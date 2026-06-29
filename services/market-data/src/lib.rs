pub(crate) mod consensus;
pub(crate) mod exchanges;
pub(crate) mod indicators;

use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{watch, RwLock};
use viper_domain::config::*;
use viper_domain::{stream_publish, MarketSignal, MarketSignalEvent, REDIS_STREAM_MARKET_DATA};

use crate::consensus::{
    apply_btc_context, fetch_analytics_weights, stabilize_consensus_side, ConsensusLatchState,
    InvalidSignalDrop, LatestSignalsSnapshot,
};
use crate::exchanges::fetch_market_signal;

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {},
                    _ = sigterm.recv() => {},
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

pub async fn run() -> Result<(), Box<dyn Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "viper_market_data=info".into()),
        )
        .json()
        .try_init();

    tracing::info!("Starting viper-market-data");

    let listener = TcpListener::bind("0.0.0.0:8081").await?;
    tracing::info!("Health check server running on :8081");
    let latest_signals = Arc::new(RwLock::new(HashMap::<String, MarketSignal>::new()));

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let mut health_shutdown_rx = shutdown_rx.clone();
    let latest_signals_for_health = Arc::clone(&latest_signals);
    let invalid_signal_count = Arc::new(AtomicU64::new(0));
    let invalid_signal_count_for_health = Arc::clone(&invalid_signal_count);
    let last_invalid_signal = Arc::new(RwLock::new(None::<InvalidSignalDrop>));
    let last_invalid_signal_for_health = Arc::clone(&last_invalid_signal);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = health_shutdown_rx.changed() => {
                    break;
                }
                accept_result = listener.accept() => {
                    if let Ok((mut socket, _)) = accept_result {
                        let latest_signals_for_conn = Arc::clone(&latest_signals_for_health);
                        let invalid_signal_count_for_conn =
                            Arc::clone(&invalid_signal_count_for_health);
                        let last_invalid_signal_for_conn =
                            Arc::clone(&last_invalid_signal_for_health);
                        tokio::spawn(async move {
                            let mut request_buf = [0_u8; 2048];
                            let bytes_read = socket.read(&mut request_buf).await.unwrap_or(0);
                            let request = String::from_utf8_lossy(&request_buf[..bytes_read]);

                            let response = if request.starts_with("GET /latest-signals")
                                || request.starts_with("GET /latest_signals")
                            {
                                let items = latest_signals_for_conn.read().await.clone();
                                let body = serde_json::to_string(&LatestSignalsSnapshot {
                                    updated_at: chrono::Utc::now().to_rfc3339(),
                                    items,
                                })
                                .unwrap_or_else(|_| "{\"updated_at\":null,\"items\":{}}".to_string());

                                format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
                                    body.len(),
                                    body
                                )
                            } else if request.starts_with("GET /health") {
                                let last_invalid = last_invalid_signal_for_conn.read().await.clone();
                                let body = serde_json::json!({
                                    "status": "ok",
                                    "invalid_market_signals_dropped": invalid_signal_count_for_conn.load(Ordering::Relaxed),
                                    "last_invalid_market_signal_drop": last_invalid,
                                })
                                .to_string();
                                format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
                                    body.len(),
                                    body
                                )
                            } else {
                                "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_string()
                            };

                            if let Err(e) = socket.write_all(response.as_bytes()).await {
                                tracing::warn!(error = ?e, "Failed to write to socket");
                            }
                        });
                    }
                }
            }
        }
    });

    let redis_url = resolve_redis_url();
    tracing::info!(redis_url = %redis_url, "Connecting to Redis");

    let client = redis::Client::open(redis_url.clone())?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    let bybit_env = TradingMode::from_env().bybit_env();

    let universe: Vec<String> = parse_trading_pairs();

    let base_url = resolve_bybit_base_url();
    let analytics_scores_url = std::env::var("ANALYTICS_SCORES_URL")
        .unwrap_or_else(|_| "http://analytics:8086/scores".to_string());
    let analytics_min_evaluated = std::env::var("ANALYTICS_MIN_EVALUATED")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .max(1);
    let min_exchanges = std::env::var("MARKET_DATA_MIN_EXCHANGES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(2)
        .clamp(1, 3);
    let mut consensus_latch = HashMap::<String, ConsensusLatchState>::new();
    tracing::info!(
        bybit_env = %bybit_env,
        base_url = %base_url,
        analytics_scores_url = %analytics_scores_url,
        min_exchanges = min_exchanges,
        pairs = %universe.join(","),
        "Market-data config"
    );

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("vipertrade-market-data/0.8")
        .build()?;

    loop {
        if *shutdown_rx.borrow() {
            tracing::info!("Received shutdown signal, stopping viper-market-data");
            break;
        }

        let weights =
            fetch_analytics_weights(&http, &analytics_scores_url, analytics_min_evaluated).await;

        let btc_context = match fetch_market_signal(
            &http,
            &base_url,
            "BTCUSDT",
            &weights,
            min_exchanges,
        )
        .await
        {
            Ok(mut signal) => {
                stabilize_consensus_side(&mut signal, &mut consensus_latch);
                signal
            }
            Err(err) => {
                latest_signals.write().await.clear();
                tracing::error!(error = %err, "Failed to refresh BTC macro context; clearing latest signals and skipping cycle");
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        tracing::info!("Received shutdown signal, stopping viper-market-data");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                }
                continue;
            }
        };

        let mut cycle_signals = HashMap::<String, MarketSignal>::new();

        for symbol in universe.iter() {
            match fetch_market_signal(&http, &base_url, symbol, &weights, min_exchanges).await {
                Ok(mut signal) => {
                    stabilize_consensus_side(&mut signal, &mut consensus_latch);
                    apply_btc_context(&mut signal, &btc_context);
                    if let Err(err) = signal.validate() {
                        invalid_signal_count.fetch_add(1, Ordering::Relaxed);
                        let drop = InvalidSignalDrop {
                            symbol: symbol.clone(),
                            stage: "pre_publish_signal".to_string(),
                            reason: err.clone(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        *last_invalid_signal.write().await = Some(drop.clone());
                        tracing::warn!(
                            symbol = %drop.symbol,
                            stage = %drop.stage,
                            reason = %drop.reason,
                            "Invalid market signal dropped"
                        );
                        continue;
                    }
                    cycle_signals.insert(symbol.clone(), signal);
                }
                Err(err) => {
                    tracing::warn!(symbol = %symbol, error = %err, "Failed to fetch market data");
                    continue;
                }
            }
        }

        if cycle_signals.is_empty() {
            latest_signals.write().await.clear();
            tracing::warn!(
                expected_symbols = universe.len(),
                got_symbols = 0,
                skipped_symbols = universe.len(),
                "Skipping market-data publish cycle because no symbols produced a valid signal"
            );
        } else {
            let processed = cycle_signals.len();
            let skipped = universe.len().saturating_sub(processed);
            if skipped > 0 {
                tracing::warn!(
                    processed_symbols = processed,
                    skipped_symbols = skipped,
                    expected_symbols = universe.len(),
                    "Publishing partial market-data cycle; some symbols were skipped"
                );
            }
            *latest_signals.write().await = cycle_signals.clone();
            for (symbol, signal) in cycle_signals {
                let event = MarketSignalEvent::new(signal);
                if let Err(err) = event.validate() {
                    invalid_signal_count.fetch_add(1, Ordering::Relaxed);
                    let drop = InvalidSignalDrop {
                        symbol: symbol.clone(),
                        stage: "pre_publish_event".to_string(),
                        reason: err.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    };
                    *last_invalid_signal.write().await = Some(drop.clone());
                    tracing::warn!(
                        symbol = %drop.symbol,
                        stage = %drop.stage,
                        reason = %drop.reason,
                        "Invalid market signal dropped"
                    );
                    continue;
                }
                let json = serde_json::to_string(&event)?;
                if let Err(e) = stream_publish(&mut conn, REDIS_STREAM_MARKET_DATA, &json).await {
                    tracing::warn!(error = %e, "Failed to publish market data");
                    break;
                }
                tracing::info!(event_id = %event.event_id, symbol = %event.signal.symbol, "Published real market event");
            }
        }

        tokio::select! {
            _ = shutdown_rx.changed() => {
                tracing::info!("Received shutdown signal, stopping viper-market-data");
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::consensus::*;
    use super::exchanges::*;
    use super::indicators::*;
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn c(close: f64) -> Candle {
        Candle {
            open_time_ms: 0,
            high: close,
            low: close,
            close,
            volume_quote: 1.0,
        }
    }
    fn chl(high: f64, low: f64, close: f64) -> Candle {
        Candle {
            open_time_ms: 0,
            high,
            low,
            close,
            volume_quote: 1.0,
        }
    }
    fn cv(close: f64, vol: f64) -> Candle {
        Candle {
            open_time_ms: 0,
            high: close,
            low: close,
            close,
            volume_quote: vol,
        }
    }
    fn series(closes: &[f64]) -> Vec<Candle> {
        closes.iter().map(|&x| c(x)).collect()
    }
    fn ramp(n: usize, start: f64, step: f64) -> Vec<Candle> {
        (0..n).map(|i| c(start + step * i as f64)).collect()
    }
    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    // ── median ────────────────────────────────────────────────────────────
    #[test]
    fn median_odd_even_empty_and_unsorted() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(median(&mut []), 0.0);
        assert_eq!(median(&mut [7.0]), 7.0);
    }

    // ── EMA ───────────────────────────────────────────────────────────────
    #[test]
    fn ema_guards_and_constant_series() {
        assert_eq!(compute_ema(&[], 10), None);
        assert_eq!(compute_ema(&series(&[5.0; 10]), 0), None);
        let v = compute_ema(&series(&[100.0; 20]), 10).unwrap();
        assert!(approx(v, 100.0, 1e-9), "ema={v}");
    }

    #[test]
    fn ema_period_one_is_last_close() {
        let v = compute_ema(&series(&[1.0, 2.0, 3.0]), 1).unwrap();
        assert!(approx(v, 3.0, 1e-12), "ema={v}");
    }

    // ── Bollinger / %B ────────────────────────────────────────────────────
    #[test]
    fn bollinger_needs_full_period() {
        assert!(compute_bollinger(&series(&[1.0, 2.0, 3.0]), 4).is_none());
    }

    #[test]
    fn bollinger_constant_series_is_neutral() {
        let (upper, mean, lower, bw, pb) = compute_bollinger(&series(&[50.0; 20]), 20).unwrap();
        assert!(approx(mean, 50.0, 1e-9));
        assert!(approx(upper, 50.0, 1e-9));
        assert!(approx(lower, 50.0, 1e-9));
        assert!(approx(bw, 0.0, 1e-9));
        assert!(approx(pb, 0.5, 1e-12));
    }

    #[test]
    fn bollinger_known_window() {
        let (upper, mean, lower, bw, pb) =
            compute_bollinger(&series(&[1.0, 2.0, 3.0, 4.0]), 4).unwrap();
        let std = 1.25_f64.sqrt();
        assert!(approx(mean, 2.5, 1e-12));
        assert!(approx(upper, 2.5 + 2.0 * std, 1e-9));
        assert!(approx(lower, 2.5 - 2.0 * std, 1e-9));
        let pb_expected = (4.0 - (2.5 - 2.0 * std)) / (4.0 * std);
        assert!(approx(pb, pb_expected, 1e-12), "pb={pb}");
        assert!(approx(bw, 4.0 * std / 2.5, 1e-9));
    }

    // ── RSI ───────────────────────────────────────────────────────────────
    #[test]
    fn rsi_needs_15_candles() {
        assert!(compute_rsi14(&series(&[1.0; 14])).is_none());
    }

    #[test]
    fn rsi_monotonic_extremes() {
        assert!(approx(
            compute_rsi14(&ramp(20, 1.0, 1.0)).unwrap(),
            100.0,
            1e-9
        ));
        assert!(approx(
            compute_rsi14(&ramp(20, 100.0, -1.0)).unwrap(),
            0.0,
            1e-9
        ));
        assert!(approx(
            compute_rsi14(&series(&[42.0; 20])).unwrap(),
            100.0,
            1e-9
        ));
    }

    // ── ATR ───────────────────────────────────────────────────────────────
    #[test]
    fn atr_guard_and_known_value() {
        assert_eq!(compute_atr14(&[c(10.0)]), 0.0);
        let candles = vec![
            chl(10.0, 10.0, 10.0),
            chl(12.0, 9.0, 11.0),
            chl(13.0, 10.0, 12.0),
        ];
        assert!(approx(compute_atr14(&candles), 3.0, 1e-12));
    }

    // ── ADX ───────────────────────────────────────────────────────────────
    #[test]
    fn adx_needs_29_candles() {
        assert!(compute_adx14(&ramp(28, 100.0, 0.5)).is_none());
    }

    #[test]
    fn adx_flat_is_zero_trend_is_max() {
        assert!(approx(
            compute_adx14(&series(&[100.0; 40])).unwrap(),
            0.0,
            1e-9
        ));
        let adx = compute_adx14(&ramp(40, 100.0, 0.5)).unwrap();
        assert!(approx(adx, 100.0, 1e-6), "adx={adx}");
        assert!((0.0..=100.0).contains(&adx));
    }

    // ── MACD ──────────────────────────────────────────────────────────────
    #[test]
    fn macd_needs_35_and_is_flat_on_constant() {
        assert!(compute_macd(&series(&[1.0; 34])).is_none());
        let (line, signal, hist) = compute_macd(&series(&[100.0; 40])).unwrap();
        assert!(approx(line, 0.0, 1e-9));
        assert!(approx(signal, 0.0, 1e-9));
        assert!(approx(hist, 0.0, 1e-9));
    }

    // ── volume ratio ──────────────────────────────────────────────────────
    #[test]
    fn volume_ratio_guard_constant_and_doubled() {
        assert!(compute_volume_ratio(&series(&[1.0; 3]), 4).is_none());
        let constant: Vec<Candle> = (0..6).map(|_| cv(1.0, 100.0)).collect();
        assert!(approx(
            compute_volume_ratio(&constant, 4).unwrap(),
            1.0,
            1e-12
        ));
        let spike = vec![
            cv(1.0, 1.0),
            cv(1.0, 1.0),
            cv(1.0, 1.0),
            cv(1.0, 1.0),
            cv(1.0, 2.0),
        ];
        assert!(approx(compute_volume_ratio(&spike, 4).unwrap(), 2.0, 1e-12));
    }

    // ── composite trend score ─────────────────────────────────────────────
    #[test]
    fn composite_trend_score_neutral_bull_bear() {
        assert!(approx(
            composite_trend_score(100.0, 50.0, 50.0, 50.0, 0.0, 1.0),
            0.0,
            1e-12
        ));
        assert!(approx(
            composite_trend_score(100.0, 51.0, 50.0, 65.0, 0.5, 2.0),
            0.9,
            1e-9
        ));
        assert!(approx(
            composite_trend_score(100.0, 49.0, 50.0, 35.0, -0.5, 1.0),
            -0.72,
            1e-9
        ));
    }

    // ── regime classification ─────────────────────────────────────────────
    #[test]
    fn classify_regime_short_series_is_neutral() {
        let (r, slope) = classify_regime(&ramp(54, 100.0, 0.1), 0.5);
        assert_eq!(r, "neutral");
        assert_eq!(slope, 0.0);
    }

    #[test]
    fn classify_regime_uptrend_is_bullish_flat_is_neutral() {
        let (r, slope) = classify_regime(&ramp(60, 100.0, 0.5), 0.5);
        assert_eq!(r, "bullish");
        assert!(slope > 0.0);
        let (r2, _) = classify_regime(&series(&[100.0; 60]), 0.0);
        assert_eq!(r2, "neutral");
    }

    // ── full bundle (integration of the pure math) ────────────────────────
    #[test]
    fn bundle_rejects_short_history() {
        let err =
            compute_indicator_bundle_complete("test", "BTCUSDT", &series(&[1.0; 50])).unwrap_err();
        assert!(err.contains("incomplete"), "err={err}");
    }

    #[test]
    fn bundle_on_constant_history_is_neutral() {
        let candles: Vec<Candle> = (0..REQUIRED_CANDLE_COUNT)
            .map(|_| cv(100.0, 1000.0))
            .collect();
        let b = compute_indicator_bundle_complete("test", "BTCUSDT", &candles).unwrap();
        assert!(approx(b.ema_fast, 100.0, 1e-6));
        assert!(approx(b.ema_slow, 100.0, 1e-6));
        assert!(approx(b.bollinger_percent_b, 0.5, 1e-9));
        assert!(approx(b.bollinger_bandwidth, 0.0, 1e-9));
        assert!(approx(b.rsi_14, 100.0, 1e-9));
        assert!(approx(b.macd_histogram, 0.0, 1e-9));
        assert!(approx(b.volume_ratio, 1.0, 1e-9));
    }

    // ── consensus side latch (state machine) ───────────────────────────────
    fn msig(symbol: &str, side: &str) -> MarketSignal {
        MarketSignal {
            symbol: symbol.to_string(),
            current_price: 100.0,
            bybit_price: 100.0,
            atr_14: 1.0,
            adx_14: 25.0,
            volume_24h: 1_000_000,
            funding_rate: 0.01,
            trend_score: 0.5,
            spread_pct: 0.05,
            consensus_atr_14: 1.0,
            consensus_adx_14: 25.0,
            consensus_volume_24h: 1_000_000,
            consensus_funding_rate: 0.01,
            consensus_trend_score: 0.5,
            consensus_spread_pct: 0.05,
            consensus_trend_slope: 0.0,
            ema_fast: 100.0,
            ema_slow: 99.0,
            bollinger_upper: 110.0,
            bollinger_middle: 100.0,
            bollinger_lower: 90.0,
            bollinger_bandwidth: 0.2,
            bollinger_percent_b: 0.5,
            consensus_ema_fast: 100.0,
            consensus_ema_slow: 99.0,
            consensus_bollinger_upper: 110.0,
            consensus_bollinger_middle: 100.0,
            consensus_bollinger_lower: 90.0,
            consensus_bollinger_bandwidth: 0.2,
            consensus_bollinger_percent_b: 0.5,
            rsi_14: 50.0,
            consensus_rsi_14: 50.0,
            macd_line: 0.0,
            macd_signal: 0.0,
            macd_histogram: 0.0,
            consensus_macd_line: 0.0,
            consensus_macd_signal: 0.0,
            consensus_macd_histogram: 0.0,
            volume_ratio: 1.0,
            consensus_volume_ratio: 1.0,
            btc_regime: "neutral".to_string(),
            btc_trend_score: 0.0,
            btc_consensus_count: 0,
            btc_volume_ratio: 1.0,
            regime: side.to_string(),
            consensus_side: side.to_string(),
            consensus_count: 1,
            exchanges_available: 1,
            consensus_ratio: 1.0,
            trend_slope: 0.0,
            bybit_regime: side.to_string(),
            bullish_exchanges: 0,
            bearish_exchanges: 0,
        }
    }

    #[test]
    fn latch_initial_side_is_applied_immediately() {
        let mut signal = msig("BTCUSDT", "bullish");
        let mut state = HashMap::new();
        stabilize_consensus_side(&mut signal, &mut state);
        assert_eq!(signal.consensus_side, "bullish");
        assert_eq!(signal.regime, "bullish");
    }

    #[test]
    fn latch_needs_two_cycles_to_confirm_switch() {
        let mut state = HashMap::new();
        let mut signal = msig("BTCUSDT", "bullish");
        stabilize_consensus_side(&mut signal, &mut state);
        assert_eq!(signal.consensus_side, "bullish");

        signal.consensus_side = "bearish".to_string();
        stabilize_consensus_side(&mut signal, &mut state);
        assert_eq!(
            signal.consensus_side, "bullish",
            "still latched after 1 bearish cycle"
        );

        signal.consensus_side = "bearish".to_string();
        stabilize_consensus_side(&mut signal, &mut state);
        assert_eq!(signal.consensus_side, "bearish", "confirmed after 2 cycles");
    }

    #[test]
    fn latch_switch_before_confirmation_resets_counter() {
        let mut state = HashMap::new();
        let mut signal = msig("BTCUSDT", "bullish");
        stabilize_consensus_side(&mut signal, &mut state);
        assert_eq!(signal.consensus_side, "bullish");

        signal.consensus_side = "bearish".to_string();
        stabilize_consensus_side(&mut signal, &mut state);
        assert_eq!(
            signal.consensus_side, "bullish",
            "still latched after 1 bearish"
        );

        signal.consensus_side = "neutral".to_string();
        stabilize_consensus_side(&mut signal, &mut state);
        assert_eq!(
            signal.consensus_side, "bullish",
            "still latched after 1 neutral"
        );

        signal.consensus_side = "neutral".to_string();
        stabilize_consensus_side(&mut signal, &mut state);
        assert_eq!(
            signal.consensus_side, "neutral",
            "neutral confirmed after 2 cycles"
        );
    }

    #[test]
    fn latch_multiple_symbols_independent() {
        let mut state = HashMap::new();
        let mut btc = msig("BTCUSDT", "bullish");
        let mut eth = msig("ETHUSDT", "bearish");

        stabilize_consensus_side(&mut btc, &mut state);
        stabilize_consensus_side(&mut eth, &mut state);
        assert_eq!(btc.consensus_side, "bullish");
        assert_eq!(eth.consensus_side, "bearish");

        eth.consensus_side = "bullish".to_string();
        eth.bybit_regime = "bullish".to_string();
        stabilize_consensus_side(&mut eth, &mut state);
        assert_eq!(
            eth.consensus_side, "bearish",
            "ETH still latched after 1 bullish"
        );
        assert_eq!(btc.consensus_side, "bullish", "BTC unchanged");
    }

    // ── aggregate_signals (weighted consensus) ─────────────────────────────
    fn esig(source: &'static str, price: f64, trend: f64, regime: &'static str) -> ExchangeSignal {
        ExchangeSignal {
            source,
            current_price: price,
            atr_14: 1.0,
            adx_14: 25.0,
            volume_24h: 1_000_000,
            funding_rate: 0.01,
            trend_score: trend,
            spread_pct: 0.05,
            ema_fast: 100.0,
            ema_slow: 99.0,
            bollinger_upper: 110.0,
            bollinger_middle: 100.0,
            bollinger_lower: 90.0,
            bollinger_bandwidth: 0.2,
            bollinger_percent_b: 0.5,
            rsi_14: 50.0,
            macd_line: 0.0,
            macd_signal: 0.0,
            macd_histogram: 0.0,
            volume_ratio: 1.0,
            regime,
            trend_slope: 0.0,
        }
    }

    #[test]
    fn aggregate_empty_signals_returns_error() {
        let err = aggregate_signals("BTCUSDT", &[], &HashMap::new()).unwrap_err();
        assert!(err.contains("no exchange signal"));
    }

    #[test]
    fn aggregate_single_exchange_with_default_weight() {
        let signals = vec![esig("bybit", 100.0, 0.5, "bullish")];
        let result = aggregate_signals("BTCUSDT", &signals, &HashMap::new()).unwrap();
        assert!(approx(result.consensus_trend_score, 0.5, 1e-9));
        assert_eq!(result.consensus_side, "bullish");
        assert_eq!(result.current_price, 100.0);
    }

    #[test]
    fn aggregate_three_exchanges_equal_weight() {
        let signals = vec![
            esig("bybit", 100.0, 0.6, "bullish"),
            esig("binance", 101.0, 0.4, "bullish"),
            esig("okx", 99.0, 0.2, "bullish"),
        ];
        let result = aggregate_signals("BTCUSDT", &signals, &HashMap::new()).unwrap();
        assert!(approx(result.consensus_trend_score, 0.4, 1e-9));
        assert_eq!(result.consensus_side, "bullish");
        assert_eq!(result.current_price, 100.0);
    }

    #[test]
    fn aggregate_custom_weights_affect_consensus() {
        let signals = vec![
            esig("bybit", 100.0, 0.8, "bullish"),
            esig("binance", 101.0, 0.2, "bearish"),
        ];
        let mut weights = HashMap::new();
        weights.insert("bybit:BTCUSDT".to_string(), 1.5);
        weights.insert("binance:BTCUSDT".to_string(), 0.5);
        let result = aggregate_signals("BTCUSDT", &signals, &weights).unwrap();
        assert!(approx(result.consensus_trend_score, 0.65, 1e-9));
    }

    #[test]
    fn aggregate_mixed_regime_is_neutral_consensus() {
        let signals = vec![
            esig("bybit", 100.0, 0.5, "bullish"),
            esig("binance", 101.0, 0.5, "bearish"),
            esig("okx", 99.0, 0.5, "bullish"),
        ];
        let result = aggregate_signals("BTCUSDT", &signals, &HashMap::new()).unwrap();
        assert_eq!(result.consensus_side, "neutral");
    }

    #[test]
    fn aggregate_all_bearish_consensus() {
        let signals = vec![
            esig("bybit", 100.0, -0.6, "bearish"),
            esig("binance", 101.0, -0.4, "bearish"),
        ];
        let result = aggregate_signals("BTCUSDT", &signals, &HashMap::new()).unwrap();
        assert_eq!(result.consensus_side, "bearish");
    }

    #[test]
    fn aggregate_bybit_price_takes_precedence() {
        let signals = vec![
            esig("bybit", 100.0, 0.5, "neutral"),
            esig("binance", 200.0, 0.5, "neutral"),
        ];
        let result = aggregate_signals("BTCUSDT", &signals, &HashMap::new()).unwrap();
        assert_eq!(result.current_price, 100.0);
        assert_eq!(result.bybit_price, 100.0);
        assert_eq!(result.bybit_regime, "neutral");
    }

    #[test]
    fn aggregate_missing_weight_falls_back_to_global_and_default() {
        let signals = vec![esig("bybit", 100.0, 0.5, "bullish")];
        let result = aggregate_signals("BTCUSDT", &signals, &HashMap::new()).unwrap();
        assert!(approx(result.consensus_trend_score, 0.5, 1e-9));
    }

    #[test]
    fn aggregate_global_wildcard_weight_applied() {
        let signals = vec![esig("bybit", 100.0, 0.3, "bullish")];
        let mut weights = HashMap::new();
        weights.insert("bybit:*".to_string(), 0.5);
        let result = aggregate_signals("BTCUSDT", &signals, &weights).unwrap();
        let expected = 0.5_f64.clamp(0.5, 1.5) * 0.3 / 0.5_f64.clamp(0.5, 1.5);
        assert!(
            approx(result.consensus_trend_score, expected, 1e-9),
            "got {} expected {}",
            result.consensus_trend_score,
            expected
        );
    }

    // ── apply_btc_context ──────────────────────────────────────────────────
    #[test]
    fn btc_context_skips_btcusdt_itself() {
        let mut btc = msig("BTCUSDT", "neutral");
        let btc_signal = msig("BTCUSDT", "bullish");
        btc.btc_regime = "original".to_string();

        apply_btc_context(&mut btc, &btc_signal);
        assert_eq!(btc.btc_regime, "original");
    }

    #[test]
    fn btc_context_injects_into_altcoin() {
        let mut alt = msig("ETHUSDT", "neutral");
        let btc_signal = MarketSignal {
            regime: "bearish".to_string(),
            consensus_trend_score: -0.6,
            consensus_count: 3,
            consensus_volume_ratio: 0.8,
            ..msig("BTCUSDT", "bearish")
        };

        apply_btc_context(&mut alt, &btc_signal);
        assert_eq!(alt.btc_regime, "bearish");
        assert!(approx(alt.btc_trend_score, -0.6, 1e-9));
        assert_eq!(alt.btc_consensus_count, 3);
        assert!(approx(alt.btc_volume_ratio, 0.8, 1e-9));
    }

    #[test]
    fn btc_context_case_insensitive_skip() {
        let mut btc = msig("btcusdt", "neutral");
        apply_btc_context(&mut btc, &msig("BTCUSDT", "bullish"));
        assert_eq!(btc.btc_regime, "neutral", "case-insensitive skip");
    }

    // ── align_exchange_candles (cross-exchange alignment) ──────────────────
    fn candle_ts(ts: i64, price: f64) -> Candle {
        Candle {
            open_time_ms: ts,
            high: price,
            low: price,
            close: price,
            volume_quote: 1.0,
        }
    }

    fn make_snapshot(source: &'static str, timestamps: &[i64], price: f64) -> RawExchangeSnapshot {
        let candles: Vec<Candle> = timestamps.iter().map(|ts| candle_ts(*ts, price)).collect();
        RawExchangeSnapshot {
            source,
            current_price: price,
            volume_24h: 1_000_000,
            funding_rate: 0.01,
            spread_pct: 0.05,
            candles,
        }
    }

    #[test]
    fn align_empty_snapshots_returns_error() {
        let err = align_exchange_candles("BTCUSDT", &mut []).unwrap_err();
        assert!(err.contains("no exchange snapshots"));
    }

    #[test]
    fn align_partial_overlap_produces_exact_count() {
        let interval = 60_000i64;
        let n = 210usize;
        let start = unix_time_ms() - (n as i64 * interval) - 3_600_000;

        let bybit_ts: Vec<i64> = (0..n).map(|i| start + i as i64 * interval).collect();
        let binance_ts: Vec<i64> = (1..=n).map(|i| start + i as i64 * interval).collect();
        let okx_ts: Vec<i64> = (2..=(n + 1)).map(|i| start + i as i64 * interval).collect();

        let mut snapshots = vec![
            make_snapshot("bybit", &bybit_ts, 100.0),
            make_snapshot("binance", &binance_ts, 101.0),
            make_snapshot("okx", &okx_ts, 102.0),
        ];

        align_exchange_candles("BTCUSDT", &mut snapshots).expect("alignment");

        for snap in &snapshots {
            assert_eq!(
                snap.candles.len(),
                REQUIRED_CANDLE_COUNT,
                "{} candle count",
                snap.source
            );
        }

        let ref_ts: Vec<i64> = snapshots[0]
            .candles
            .iter()
            .map(|c| c.open_time_ms)
            .collect();
        for snap in &snapshots[1..] {
            let ts: Vec<i64> = snap.candles.iter().map(|c| c.open_time_ms).collect();
            assert_eq!(ts, ref_ts, "{} timestamps must match bybit", snap.source);
        }
    }

    #[test]
    fn align_identical_timestamps_keeps_last_200() {
        let interval = 60_000i64;
        let n = 220usize;
        let start = unix_time_ms() - (n as i64 * interval) - 3_600_000;

        let ts: Vec<i64> = (0..n).map(|i| start + i as i64 * interval).collect();
        let mut snapshots = vec![
            make_snapshot("bybit", &ts, 100.0),
            make_snapshot("binance", &ts, 101.0),
        ];

        align_exchange_candles("BTCUSDT", &mut snapshots).expect("alignment");

        for snap in &snapshots {
            assert_eq!(snap.candles.len(), REQUIRED_CANDLE_COUNT);
        }
        let last_ts: Vec<i64> = ts[ts.len() - REQUIRED_CANDLE_COUNT..].to_vec();
        let aligned_ts: Vec<i64> = snapshots[0]
            .candles
            .iter()
            .map(|c| c.open_time_ms)
            .collect();
        assert_eq!(aligned_ts, last_ts);
    }

    #[test]
    fn align_insufficient_overlap_rejected() {
        let interval = 60_000i64;
        let start = unix_time_ms() - (250 * interval) - 3_600_000;

        let bybit_ts: Vec<i64> = (0..210).map(|i| start + i as i64 * interval).collect();
        let binance_ts: Vec<i64> = (150..360).map(|i| start + i as i64 * interval).collect();

        let mut snapshots = vec![
            make_snapshot("bybit", &bybit_ts, 100.0),
            make_snapshot("binance", &binance_ts, 101.0),
        ];

        let err = align_exchange_candles("BTCUSDT", &mut snapshots).unwrap_err();
        assert!(
            err.contains("incomplete"),
            "expected alignment error: {err}"
        );
    }

    // ── score_key ────────────────────────────────────────────────
    #[test]
    fn score_key_joins_exchange_and_symbol() {
        assert_eq!(score_key("bybit", "BTCUSDT"), "bybit:BTCUSDT");
        assert_eq!(score_key("binance", "ETHUSDT"), "binance:ETHUSDT");
        assert_eq!(score_key("", ""), ":");
    }

    // ── unix_time_ms ─────────────────────────────────────────────
    #[test]
    fn unix_time_ms_is_recent_and_positive() {
        let ts = unix_time_ms();
        assert!(ts > 1_700_000_000_000, "ts={ts} looks prehistoric");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        assert!(
            (ts - now).abs() < 1000,
            "ts={ts} should be near current time"
        );
    }

    // ── okx_inst_id ──────────────────────────────────────────────
    #[test]
    fn okx_inst_id_transforms_symbol() {
        assert_eq!(okx_inst_id("BTCUSDT"), "BTC-USDT-SWAP");
        assert_eq!(okx_inst_id("ETHUSDT"), "ETH-USDT-SWAP");
        assert_eq!(okx_inst_id("1000PEPEUSDT"), "1000PEPE-USDT-SWAP");
    }

    #[test]
    fn okx_inst_id_passes_through_non_usdt_suffix() {
        // strip_suffix only removes "USDT" when it appears at the end
        assert_eq!(okx_inst_id("BTC-USD"), "BTC-USD-USDT-SWAP");
        assert_eq!(okx_inst_id("SOL"), "SOL-USDT-SWAP");
    }

    // ── parse_candles (Bybit format) ─────────────────────────────
    #[test]
    fn parse_candles_valid_rows() {
        let rows = vec![vec![
            "1000000".into(),
            "open".into(),
            "50000.0".into(),
            "49900.0".into(),
            "50100.0".into(),
            "100.5".into(),
        ]];
        let candles = parse_candles(rows);
        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].open_time_ms, 1_000_000);
        assert!((candles[0].high - 50000.0).abs() < 1e-6);
        assert!((candles[0].low - 49900.0).abs() < 1e-6);
        assert!((candles[0].close - 50100.0).abs() < 1e-6);
        assert!((candles[0].volume_quote - 100.5).abs() < 1e-6);
    }

    #[test]
    fn parse_candles_skips_too_few_columns() {
        let rows = vec![vec!["1000000".into(), "open".into()]];
        let candles = parse_candles(rows);
        assert!(candles.is_empty(), "should skip rows with <5 columns");
    }

    #[test]
    fn parse_candles_skips_zero_close() {
        let rows = vec![vec![
            "1000000".into(),
            "open".into(),
            "0".into(),
            "0".into(),
            "0".into(),
            "0".into(),
        ]];
        let candles = parse_candles(rows);
        assert!(candles.is_empty(), "should skip rows where close <= 0");
    }

    #[test]
    fn parse_candles_handles_missing_volume() {
        // volume is in column 5 (0-indexed); if missing, defaults to 0
        let rows = vec![vec![
            "1000000".into(),
            "open".into(),
            "100.0".into(),
            "99.0".into(),
            "101.0".into(),
        ]];
        let candles = parse_candles(rows);
        assert_eq!(candles.len(), 1);
        assert!((candles[0].volume_quote - 0.0).abs() < 1e-6);
    }

    // ── parse_candles_binance ────────────────────────────────────
    #[test]
    fn parse_candles_binance_valid_rows() {
        use serde_json::json;
        let rows = vec![vec![
            json!(1000000),
            json!("open"),
            json!(50000.0),
            json!(49900.0),
            json!(50100.0),
            json!(200.5),
        ]];
        let candles = parse_candles_binance(rows);
        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].open_time_ms, 1_000_000);
        assert!((candles[0].volume_quote - 200.5).abs() < 1e-6);
    }

    #[test]
    fn parse_candles_binance_skips_invalid_timestamp() {
        use serde_json::json;
        let rows = vec![vec![
            json!("not_a_number"),
            json!("open"),
            json!(100.0),
            json!(99.0),
            json!(101.0),
        ]];
        let candles = parse_candles_binance(rows);
        assert!(
            candles.is_empty(),
            "should skip rows with non-numeric timestamp"
        );
    }

    // ── parse_candles_okx ────────────────────────────────────────
    #[test]
    fn parse_candles_okx_valid_rows() {
        let rows = vec![vec![
            "1000000".into(),
            "open".into(),
            "50000.0".into(),
            "49900.0".into(),
            "50100.0".into(),
            "vol".into(),
            "volCcy".into(),
            "300.5".into(),
        ]];
        let candles = parse_candles_okx(rows);
        assert_eq!(candles.len(), 1);
        assert!(
            (candles[0].volume_quote - 300.5).abs() < 1e-6,
            "OKX volume at index 7"
        );
    }

    #[test]
    fn parse_candles_okx_missing_volume_defaults_zero() {
        let rows = vec![vec![
            "1000000".into(),
            "open".into(),
            "100.0".into(),
            "99.0".into(),
            "101.0".into(),
        ]];
        let candles = parse_candles_okx(rows);
        assert_eq!(candles.len(), 1);
        assert!((candles[0].volume_quote - 0.0).abs() < 1e-6);
    }

    // ── build_exchange_signal ────────────────────────────────────
    #[test]
    fn build_exchange_signal_rejects_empty_candles() {
        let snap = RawExchangeSnapshot {
            source: "bybit",
            current_price: 100.0,
            volume_24h: 1_000_000,
            funding_rate: 0.01,
            spread_pct: 0.05,
            candles: vec![],
        };
        let err = build_exchange_signal("BTCUSDT", snap).unwrap_err();
        assert!(err.contains("empty"), "error: {err}");
    }

    #[test]
    fn build_exchange_signal_computes_indicators() {
        let n = REQUIRED_CANDLE_COUNT + 10;
        let candles: Vec<Candle> = (0..n)
            .map(|i| Candle {
                open_time_ms: i as i64 * 60_000,
                high: 100.0 + (i % 5) as f64,
                low: 99.0,
                close: 100.0 + (i % 10) as f64,
                volume_quote: 1000.0,
            })
            .collect();
        let snap = RawExchangeSnapshot {
            source: "bybit",
            current_price: 105.0,
            volume_24h: 1_000_000,
            funding_rate: 0.01,
            spread_pct: 0.05,
            candles,
        };
        let signal = build_exchange_signal("BTCUSDT", snap).expect("build signal");
        assert_eq!(signal.source, "bybit");
        assert!((signal.current_price - 105.0).abs() < 1e-6);
        assert!(signal.rsi_14 > 0.0, "rsi should be computed");
        assert!(signal.trend_score != 0.0, "trend_score should be computed");
    }
}
