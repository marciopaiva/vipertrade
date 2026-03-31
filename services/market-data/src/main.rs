use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml::Value as YamlValue;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{watch, RwLock};
use viper_domain::{MarketSignal, MarketSignalEvent};

#[derive(Debug, Deserialize)]
struct BybitResponse<T> {
    #[serde(rename = "retCode")]
    ret_code: i64,
    #[serde(rename = "retMsg")]
    ret_msg: String,
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct KlineResult {
    list: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TickerResult {
    list: Vec<TickerItem>,
}

#[derive(Debug, Deserialize)]
struct TickerItem {
    #[serde(rename = "lastPrice")]
    last_price: String,
    #[serde(rename = "bid1Price")]
    bid1_price: String,
    #[serde(rename = "ask1Price")]
    ask1_price: String,
    #[serde(rename = "volume24h")]
    volume_24h: String,
    #[serde(rename = "turnover24h")]
    turnover_24h: String,
    #[serde(rename = "fundingRate")]
    funding_rate: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct Candle {
    open_time_ms: i64,
    high: f64,
    low: f64,
    close: f64,
    volume_quote: f64,
}

#[derive(Debug, Clone)]
struct RawExchangeSnapshot {
    source: &'static str,
    current_price: f64,
    volume_24h: i64,
    funding_rate: f64,
    spread_pct: f64,
    candles: Vec<Candle>,
}

#[derive(Debug, Clone)]
struct ExchangeSignal {
    source: &'static str,
    current_price: f64,
    atr_14: f64,
    volume_24h: i64,
    funding_rate: f64,
    trend_score: f64,
    spread_pct: f64,
    ema_fast: f64,
    ema_slow: f64,
    bollinger_upper: f64,
    bollinger_middle: f64,
    bollinger_lower: f64,
    bollinger_bandwidth: f64,
    bollinger_percent_b: f64,
    rsi_14: f64,
    macd_line: f64,
    macd_signal: f64,
    macd_histogram: f64,
    volume_ratio: f64,
    regime: &'static str,
    trend_slope: f64,
}

#[derive(Debug, Clone, Copy)]
struct IndicatorBundle {
    ema_fast: f64,
    ema_slow: f64,
    bollinger_upper: f64,
    bollinger_middle: f64,
    bollinger_lower: f64,
    bollinger_bandwidth: f64,
    bollinger_percent_b: f64,
    rsi_14: f64,
    macd_line: f64,
    macd_signal: f64,
    macd_histogram: f64,
    volume_ratio: f64,
}

#[derive(Debug, Deserialize)]
struct AnalyticsScoresResponse {
    exchanges: Vec<AnalyticsExchangeScore>,
    by_symbol: Vec<AnalyticsSymbolScore>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsExchangeScore {
    exchange: String,
    hit_rate: f64,
    evaluated: i64,
}

#[derive(Debug, Deserialize)]
struct AnalyticsSymbolScore {
    exchange: String,
    symbol: String,
    hit_rate: f64,
    evaluated: i64,
}

#[derive(Debug, Serialize)]
struct LatestSignalsSnapshot {
    updated_at: String,
    items: HashMap<String, MarketSignal>,
}

#[derive(Debug, Clone, Serialize)]
struct InvalidSignalDrop {
    symbol: String,
    stage: String,
    reason: String,
    timestamp: String,
}

#[derive(Debug, Clone)]
struct ConsensusLatchState {
    stable_side: String,
    pending_side: String,
    pending_count: u8,
}

const CONSENSUS_SIDE_CONFIRMATION_CYCLES: u8 = 2;

fn configured_pairs_path() -> String {
    std::env::var("STRATEGY_CONFIG")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "/app/config/pairs.yaml".to_string())
}

fn parse_trading_pairs_from_config(path: &str) -> Option<Vec<String>> {
    let raw = fs::read_to_string(path).ok()?;
    let yaml: YamlValue = serde_yaml::from_str(&raw).ok()?;
    let obj = yaml.as_mapping()?;

    let mut pairs = Vec::new();
    for (key, value) in obj {
        let Some(symbol) = key.as_str() else {
            continue;
        };
        if symbol.eq_ignore_ascii_case("global") || symbol.eq_ignore_ascii_case("profiles") {
            continue;
        }

        let enabled = value
            .as_mapping()
            .and_then(|map| map.get(YamlValue::from("enabled")))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if enabled {
            pairs.push(symbol.to_uppercase());
        }
    }

    if pairs.is_empty() {
        None
    } else {
        pairs.sort();
        Some(pairs)
    }
}

fn parse_trading_pairs() -> Vec<String> {
    if let Ok(raw) = std::env::var("TRADING_PAIRS") {
        let pairs: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        if !pairs.is_empty() {
            return pairs;
        }
    }

    let config_path = configured_pairs_path();
    if let Some(pairs) = parse_trading_pairs_from_config(&config_path) {
        return pairs;
    }
    panic!(
        "no trading pairs configured: set TRADING_PAIRS or provide enabled symbols in {}",
        config_path
    );
}

fn parse_f64(raw: &str) -> Option<f64> {
    raw.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn parse_i64(raw: &str) -> Option<i64> {
    raw.parse::<i64>().ok()
}

fn parse_candles(rows: Vec<Vec<String>>) -> Vec<Candle> {
    let mut candles: Vec<Candle> = rows
        .into_iter()
        .filter_map(|row| {
            if row.len() < 5 {
                return None;
            }
            let open_time_ms = parse_i64(&row[0])?;
            let high = parse_f64(&row[2])?;
            let low = parse_f64(&row[3])?;
            let close = parse_f64(&row[4])?;
            let volume_quote = row.get(6).and_then(|v| parse_f64(v)).unwrap_or(0.0);
            Some(Candle {
                open_time_ms,
                high,
                low,
                close,
                volume_quote,
            })
        })
        .collect();

    // Bybit kline returns newest -> oldest; strategy metrics need oldest -> newest.
    candles.reverse();
    candles
}

fn parse_candles_binance(rows: Vec<Vec<Value>>) -> Vec<Candle> {
    rows.into_iter()
        .filter_map(|row| {
            if row.len() < 8 {
                return None;
            }
            let open_time_ms = row[0].as_i64()?;
            let high = row[2].as_str().and_then(parse_f64)?;
            let low = row[3].as_str().and_then(parse_f64)?;
            let close = row[4].as_str().and_then(parse_f64)?;
            let volume_quote = row
                .get(7)
                .and_then(Value::as_str)
                .and_then(parse_f64)
                .unwrap_or(0.0);
            Some(Candle {
                open_time_ms,
                high,
                low,
                close,
                volume_quote,
            })
        })
        .collect()
}

fn parse_candles_okx(rows: Vec<Vec<String>>) -> Vec<Candle> {
    let mut candles: Vec<Candle> = rows
        .into_iter()
        .filter_map(|row| {
            if row.len() < 8 {
                return None;
            }
            let open_time_ms = parse_i64(&row[0])?;
            let high = parse_f64(&row[2])?;
            let low = parse_f64(&row[3])?;
            let close = parse_f64(&row[4])?;
            let volume_quote = row.get(7).and_then(|v| parse_f64(v)).unwrap_or(0.0);
            Some(Candle {
                open_time_ms,
                high,
                low,
                close,
                volume_quote,
            })
        })
        .collect();

    // OKX returns newest -> oldest.
    candles.reverse();
    candles
}

const REQUIRED_CANDLE_COUNT: usize = 200;
const FETCH_CANDLE_LIMIT: usize = 220;
const CANDLE_INTERVAL_MS: i64 = 60_000;

fn unix_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn retain_closed_candles(candles: Vec<Candle>) -> Vec<Candle> {
    let now_ms = unix_time_ms();
    candles
        .into_iter()
        .filter(|candle| {
            candle.open_time_ms > 0 && candle.open_time_ms + CANDLE_INTERVAL_MS <= now_ms
        })
        .collect()
}

fn align_exchange_candles(
    symbol: &str,
    snapshots: &mut [RawExchangeSnapshot],
) -> Result<(), String> {
    if snapshots.is_empty() {
        return Err(format!("{} no exchange snapshots available", symbol));
    }

    for snapshot in snapshots.iter_mut() {
        snapshot.candles = retain_closed_candles(std::mem::take(&mut snapshot.candles));
        if snapshot.candles.len() < REQUIRED_CANDLE_COUNT {
            return Err(format!(
                "{} {} closed candles incomplete: expected at least {}, got {}",
                snapshot.source,
                symbol,
                REQUIRED_CANDLE_COUNT,
                snapshot.candles.len()
            ));
        }
    }

    let mut common_times: HashSet<i64> = snapshots[0]
        .candles
        .iter()
        .map(|candle| candle.open_time_ms)
        .collect();
    for snapshot in snapshots.iter().skip(1) {
        let source_times: HashSet<i64> = snapshot
            .candles
            .iter()
            .map(|candle| candle.open_time_ms)
            .collect();
        common_times.retain(|ts| source_times.contains(ts));
    }

    if common_times.len() < REQUIRED_CANDLE_COUNT {
        return Err(format!(
            "{} aligned closed candles incomplete across exchanges: expected at least {}, got {}",
            symbol,
            REQUIRED_CANDLE_COUNT,
            common_times.len()
        ));
    }

    let mut aligned_times: Vec<i64> = common_times.into_iter().collect();
    aligned_times.sort_unstable();
    let keep_times: HashSet<i64> = aligned_times[aligned_times.len() - REQUIRED_CANDLE_COUNT..]
        .iter()
        .copied()
        .collect();

    for snapshot in snapshots.iter_mut() {
        snapshot
            .candles
            .retain(|candle| keep_times.contains(&candle.open_time_ms));
        snapshot.candles.sort_by_key(|candle| candle.open_time_ms);
        if snapshot.candles.len() != REQUIRED_CANDLE_COUNT {
            return Err(format!(
                "{} {} aligned candle count mismatch: expected {}, got {}",
                snapshot.source,
                symbol,
                REQUIRED_CANDLE_COUNT,
                snapshot.candles.len()
            ));
        }
    }

    let reference_last_ts = snapshots[0]
        .candles
        .last()
        .map(|candle| candle.open_time_ms)
        .unwrap_or(0);
    if snapshots.iter().any(|snapshot| {
        snapshot.candles.last().map(|candle| candle.open_time_ms) != Some(reference_last_ts)
    }) {
        return Err(format!(
            "{} aligned candle tail mismatch across exchanges",
            symbol
        ));
    }

    Ok(())
}

fn compute_atr14(candles: &[Candle]) -> f64 {
    if candles.len() < 2 {
        return 0.0;
    }

    let mut trs: Vec<f64> = Vec::with_capacity(candles.len().saturating_sub(1));
    for idx in 1..candles.len() {
        let current = candles[idx];
        let prev_close = candles[idx - 1].close;
        let tr = (current.high - current.low)
            .max((current.high - prev_close).abs())
            .max((current.low - prev_close).abs());
        trs.push(tr.max(0.0));
    }

    let tail = trs.len().min(14);
    if tail == 0 {
        return 0.0;
    }
    let start = trs.len() - tail;
    let sum: f64 = trs[start..].iter().sum();
    sum / tail as f64
}

fn compute_rsi14(candles: &[Candle]) -> Option<f64> {
    if candles.len() < 15 {
        return None;
    }

    let start = candles.len() - 15;
    let mut gains = 0.0;
    let mut losses = 0.0;

    for idx in (start + 1)..candles.len() {
        let delta = candles[idx].close - candles[idx - 1].close;
        if delta >= 0.0 {
            gains += delta;
        } else {
            losses += -delta;
        }
    }

    let avg_gain = gains / 14.0;
    let avg_loss = losses / 14.0;

    if avg_loss == 0.0 {
        return Some(100.0);
    }

    let rs = avg_gain / avg_loss;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

fn compute_ema(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.is_empty() || period == 0 {
        return None;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let mut ema = candles[0].close;
    for candle in candles.iter().skip(1) {
        ema = (candle.close * alpha) + (ema * (1.0 - alpha));
    }
    Some(ema)
}

fn compute_macd(candles: &[Candle]) -> Option<(f64, f64, f64)> {
    if candles.len() < 35 {
        return None;
    }

    let mut macd_series = Vec::with_capacity(candles.len());
    for idx in 0..candles.len() {
        let slice = &candles[..=idx];
        let ema12 = compute_ema(slice, 12)?;
        let ema26 = compute_ema(slice, 26)?;
        macd_series.push(ema12 - ema26);
    }

    if macd_series.len() < 9 {
        return None;
    }

    let alpha = 2.0 / (9.0 + 1.0);
    let mut signal = macd_series[0];
    for value in macd_series.iter().skip(1) {
        signal = (value * alpha) + (signal * (1.0 - alpha));
    }
    let macd_line = *macd_series.last()?;
    let histogram = macd_line - signal;
    Some((macd_line, signal, histogram))
}

fn compute_bollinger(candles: &[Candle], period: usize) -> Option<(f64, f64, f64, f64, f64)> {
    if candles.len() < period || period == 0 {
        return None;
    }

    let window = &candles[candles.len() - period..];
    let mean = window.iter().map(|c| c.close).sum::<f64>() / period as f64;
    let variance = window
        .iter()
        .map(|c| {
            let delta = c.close - mean;
            delta * delta
        })
        .sum::<f64>()
        / period as f64;
    let std_dev = variance.sqrt();
    let upper = mean + (2.0 * std_dev);
    let lower = mean - (2.0 * std_dev);
    let bandwidth = if mean.abs() > f64::EPSILON {
        ((upper - lower) / mean).max(0.0)
    } else {
        0.0
    };
    let last_close = window.last()?.close;
    let percent_b = if (upper - lower).abs() > f64::EPSILON {
        (last_close - lower) / (upper - lower)
    } else {
        0.5
    };

    Some((upper, mean, lower, bandwidth, percent_b))
}

fn compute_volume_ratio(candles: &[Candle], lookback: usize) -> Option<f64> {
    if candles.len() < lookback + 1 {
        return None;
    }
    let last = candles.last()?.volume_quote;
    let start = candles.len().saturating_sub(lookback + 1);
    let baseline: Vec<f64> = candles[start..candles.len() - 1]
        .iter()
        .map(|c| c.volume_quote)
        .filter(|v| *v > 0.0)
        .collect();
    if baseline.is_empty() {
        return None;
    }
    let avg = baseline.iter().sum::<f64>() / baseline.len() as f64;
    if avg <= 0.0 {
        return None;
    }
    Some((last / avg).max(0.0))
}

fn compute_indicator_bundle_complete(
    source: &str,
    symbol: &str,
    candles: &[Candle],
) -> Result<IndicatorBundle, String> {
    if candles.len() < REQUIRED_CANDLE_COUNT {
        return Err(format!(
            "{} {} candles incomplete: expected at least {}, got {}",
            source,
            symbol,
            REQUIRED_CANDLE_COUNT,
            candles.len()
        ));
    }

    let ema_fast = compute_ema(candles, 20)
        .ok_or_else(|| format!("{} {} ema_fast incomplete", source, symbol))?;
    let ema_slow = compute_ema(candles, 50)
        .ok_or_else(|| format!("{} {} ema_slow incomplete", source, symbol))?;
    let (
        bollinger_upper,
        bollinger_middle,
        bollinger_lower,
        bollinger_bandwidth,
        bollinger_percent_b,
    ) = compute_bollinger(candles, 20)
        .ok_or_else(|| format!("{} {} bollinger incomplete", source, symbol))?;
    let rsi_14 =
        compute_rsi14(candles).ok_or_else(|| format!("{} {} rsi_14 incomplete", source, symbol))?;
    let (macd_line, macd_signal, macd_histogram) =
        compute_macd(candles).ok_or_else(|| format!("{} {} macd incomplete", source, symbol))?;
    let volume_ratio = compute_volume_ratio(candles, 20)
        .ok_or_else(|| format!("{} {} volume_ratio incomplete", source, symbol))?;

    Ok(IndicatorBundle {
        ema_fast,
        ema_slow,
        bollinger_upper,
        bollinger_middle,
        bollinger_lower,
        bollinger_bandwidth,
        bollinger_percent_b,
        rsi_14,
        macd_line,
        macd_signal,
        macd_histogram,
        volume_ratio,
    })
}

fn composite_trend_score(
    current_price: f64,
    ema_fast: f64,
    ema_slow: f64,
    rsi_14: f64,
    macd_histogram: f64,
    volume_ratio: f64,
) -> f64 {
    let ema_component = if ema_slow > 0.0 {
        ((ema_fast - ema_slow) / ema_slow / 0.003).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let rsi_component = ((rsi_14 - 50.0) / 15.0).clamp(-1.0, 1.0);
    let macd_component = if current_price > 0.0 {
        (macd_histogram / current_price / 0.0015).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let volume_component = ((volume_ratio - 1.0) / 0.5).clamp(0.0, 1.0);
    ((ema_component * 0.4) + (rsi_component * 0.25) + (macd_component * 0.25))
        * (0.8 + 0.2 * volume_component)
}

fn classify_regime(candles: &[Candle], trend_score: f64) -> (&'static str, f64) {
    if candles.len() < 55 {
        return ("neutral", 0.0);
    }

    let ema_fast = match compute_ema(candles, 20) {
        Some(v) => v,
        None => return ("neutral", 0.0),
    };
    let ema_slow = match compute_ema(candles, 50) {
        Some(v) => v,
        None => return ("neutral", 0.0),
    };
    let prev_fast = match compute_ema(&candles[..candles.len() - 1], 20) {
        Some(v) => v,
        None => return ("neutral", 0.0),
    };

    let close = candles.last().map(|c| c.close).unwrap_or(0.0);
    let slope = if prev_fast > 0.0 {
        (ema_fast - prev_fast) / prev_fast
    } else {
        0.0
    };

    let regime = if ema_fast > ema_slow && close >= ema_fast && trend_score > 0.05 && slope > 0.0 {
        "bullish"
    } else if ema_fast < ema_slow && close <= ema_fast && trend_score < -0.05 && slope < 0.0 {
        "bearish"
    } else {
        "neutral"
    };

    (regime, slope)
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn okx_inst_id(symbol: &str) -> String {
    let base = symbol.strip_suffix("USDT").unwrap_or(symbol);
    format!("{}-USDT-SWAP", base)
}

fn score_key(exchange: &str, symbol: &str) -> String {
    format!("{}:{}", exchange, symbol)
}

async fn fetch_analytics_weights(
    http: &reqwest::Client,
    analytics_scores_url: &str,
    min_evaluated: i64,
) -> HashMap<String, f64> {
    let mut weights = HashMap::<String, f64>::new();
    let response = match http.get(analytics_scores_url).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("analytics weights fetch failed: {}", e);
            return weights;
        }
    };
    if !response.status().is_success() {
        eprintln!("analytics weights http {}", response.status());
        return weights;
    }
    let payload = match response.json::<AnalyticsScoresResponse>().await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("analytics weights decode failed: {}", e);
            return weights;
        }
    };

    for row in payload.exchanges {
        if row.evaluated >= min_evaluated && row.hit_rate.is_finite() {
            // Map hit-rate to soft weight range [0.5, 1.5] around 50%.
            let w = (1.0 + ((row.hit_rate - 0.5) * 2.0)).clamp(0.5, 1.5);
            weights.insert(score_key(&row.exchange, "*"), w);
        }
    }

    for row in payload.by_symbol {
        if row.evaluated >= min_evaluated && row.hit_rate.is_finite() {
            let w = (1.0 + ((row.hit_rate - 0.5) * 2.0)).clamp(0.5, 1.5);
            weights.insert(score_key(&row.exchange, &row.symbol), w);
        }
    }

    weights
}

fn bybit_base_url() -> String {
    if let Ok(override_url) = std::env::var("BYBIT_HTTP_PUBLIC") {
        if !override_url.trim().is_empty() {
            return override_url;
        }
    }

    let env = resolve_runtime_bybit_env();
    if env.eq_ignore_ascii_case("mainnet") {
        "https://api.bybit.com".to_string()
    } else {
        "https://api-testnet.bybit.com".to_string()
    }
}

fn resolve_runtime_bybit_env() -> String {
    match std::env::var("TRADING_MODE")
        .unwrap_or_else(|_| "paper".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "testnet" => "testnet".to_string(),
        "mainnet" | "paper" | "live" => "mainnet".to_string(),
        _ => std::env::var("BYBIT_ENV").unwrap_or_else(|_| "testnet".to_string()),
    }
}

async fn fetch_json<T: for<'de> Deserialize<'de>>(
    http: &reqwest::Client,
    url: &str,
) -> Result<BybitResponse<T>, String> {
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("http {} for {}", response.status(), url));
    }

    response
        .json::<BybitResponse<T>>()
        .await
        .map_err(|e| format!("decode failed: {}", e))
}

fn build_exchange_signal(
    symbol: &str,
    snapshot: RawExchangeSnapshot,
) -> Result<ExchangeSignal, String> {
    if snapshot.candles.is_empty() {
        return Err(format!(
            "{} {} aligned candles empty",
            snapshot.source, symbol
        ));
    }

    let last_closed_price = snapshot
        .candles
        .last()
        .map(|c| c.close)
        .unwrap_or(0.0)
        .max(0.0);
    let current_price = snapshot.current_price.max(0.0);
    let atr_14 = compute_atr14(&snapshot.candles);
    let indicators = compute_indicator_bundle_complete(snapshot.source, symbol, &snapshot.candles)?;
    let trend_score = composite_trend_score(
        last_closed_price,
        indicators.ema_fast,
        indicators.ema_slow,
        indicators.rsi_14,
        indicators.macd_histogram,
        indicators.volume_ratio,
    );
    let (regime, trend_slope) = classify_regime(&snapshot.candles, trend_score);

    Ok(ExchangeSignal {
        source: snapshot.source,
        current_price,
        atr_14,
        volume_24h: snapshot.volume_24h,
        funding_rate: snapshot.funding_rate,
        trend_score,
        spread_pct: snapshot.spread_pct,
        ema_fast: indicators.ema_fast,
        ema_slow: indicators.ema_slow,
        bollinger_upper: indicators.bollinger_upper,
        bollinger_middle: indicators.bollinger_middle,
        bollinger_lower: indicators.bollinger_lower,
        bollinger_bandwidth: indicators.bollinger_bandwidth,
        bollinger_percent_b: indicators.bollinger_percent_b,
        rsi_14: indicators.rsi_14,
        macd_line: indicators.macd_line,
        macd_signal: indicators.macd_signal,
        macd_histogram: indicators.macd_histogram,
        volume_ratio: indicators.volume_ratio,
        regime,
        trend_slope,
    })
}

async fn fetch_market_signal_bybit(
    http: &reqwest::Client,
    base_url: &str,
    symbol: &str,
) -> Result<RawExchangeSnapshot, String> {
    let kline_url = format!(
        "{}/v5/market/kline?category=linear&symbol={}&interval=1&limit={}",
        base_url, symbol, FETCH_CANDLE_LIMIT
    );
    let ticker_url = format!(
        "{}/v5/market/tickers?category=linear&symbol={}",
        base_url, symbol
    );

    let kline_response = fetch_json::<KlineResult>(http, &kline_url).await?;
    if kline_response.ret_code != 0 {
        return Err(format!(
            "kline retCode={} retMsg={}",
            kline_response.ret_code, kline_response.ret_msg
        ));
    }

    let ticker_response = fetch_json::<TickerResult>(http, &ticker_url).await?;
    if ticker_response.ret_code != 0 {
        return Err(format!(
            "ticker retCode={} retMsg={}",
            ticker_response.ret_code, ticker_response.ret_msg
        ));
    }

    let candles = parse_candles(
        kline_response
            .result
            .ok_or_else(|| "missing kline result".to_string())?
            .list,
    );
    if candles.is_empty() {
        return Err("empty kline list".to_string());
    }

    let ticker = ticker_response
        .result
        .ok_or_else(|| "missing ticker result".to_string())?
        .list
        .into_iter()
        .next()
        .ok_or_else(|| "missing ticker item".to_string())?;

    let fallback_price = candles.last().map(|c| c.close).unwrap_or(0.0);
    let current_price = parse_f64(&ticker.last_price)
        .unwrap_or(fallback_price)
        .max(0.0);

    let bid = parse_f64(&ticker.bid1_price).unwrap_or(0.0);
    let ask = parse_f64(&ticker.ask1_price).unwrap_or(0.0);
    let spread_pct = if bid > 0.0 && ask > 0.0 && ask >= bid {
        let mid = (ask + bid) / 2.0;
        if mid > 0.0 {
            ((ask - bid) / mid).max(0.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    let turnover_24h = parse_f64(&ticker.turnover_24h).unwrap_or(0.0);
    let fallback_volume = parse_f64(&ticker.volume_24h).unwrap_or(0.0) * current_price;
    let volume_24h = turnover_24h.max(fallback_volume).round() as i64;

    let funding_rate = ticker
        .funding_rate
        .as_deref()
        .and_then(parse_f64)
        .unwrap_or(0.0);

    Ok(RawExchangeSnapshot {
        source: "bybit",
        current_price,
        volume_24h,
        funding_rate,
        spread_pct,
        candles,
    })
}

#[derive(Debug, Deserialize)]
struct Binance24hTicker {
    #[serde(rename = "quoteVolume")]
    quote_volume: String,
}

#[derive(Debug, Deserialize)]
struct BinanceBookTicker {
    #[serde(rename = "bidPrice")]
    bid_price: String,
    #[serde(rename = "askPrice")]
    ask_price: String,
}

async fn fetch_market_signal_binance(
    http: &reqwest::Client,
    symbol: &str,
) -> Result<RawExchangeSnapshot, String> {
    let base_url = "https://fapi.binance.com";
    let kline_url = format!(
        "{}/fapi/v1/klines?symbol={}&interval=1m&limit={}",
        base_url, symbol, FETCH_CANDLE_LIMIT
    );
    let ticker_24h_url = format!("{}/fapi/v1/ticker/24hr?symbol={}", base_url, symbol);
    let book_ticker_url = format!("{}/fapi/v1/ticker/bookTicker?symbol={}", base_url, symbol);

    let kline_rows = http
        .get(&kline_url)
        .send()
        .await
        .map_err(|e| format!("binance klines request failed: {}", e))?;
    if !kline_rows.status().is_success() {
        return Err(format!("binance klines http {}", kline_rows.status()));
    }
    let kline_rows = kline_rows
        .json::<Vec<Vec<Value>>>()
        .await
        .map_err(|e| format!("binance klines decode failed: {}", e))?;
    let candles = parse_candles_binance(kline_rows);
    if candles.is_empty() {
        return Err("empty binance kline list".to_string());
    }

    let ticker_24h = http
        .get(&ticker_24h_url)
        .send()
        .await
        .map_err(|e| format!("binance ticker24h request failed: {}", e))?;
    if !ticker_24h.status().is_success() {
        return Err(format!("binance ticker24h http {}", ticker_24h.status()));
    }
    let ticker_24h = ticker_24h
        .json::<Binance24hTicker>()
        .await
        .map_err(|e| format!("binance ticker24h decode failed: {}", e))?;

    let book_ticker = http
        .get(&book_ticker_url)
        .send()
        .await
        .map_err(|e| format!("binance bookTicker request failed: {}", e))?;
    if !book_ticker.status().is_success() {
        return Err(format!("binance bookTicker http {}", book_ticker.status()));
    }
    let book_ticker = book_ticker
        .json::<BinanceBookTicker>()
        .await
        .map_err(|e| format!("binance bookTicker decode failed: {}", e))?;

    let current_price = candles.last().map(|c| c.close).unwrap_or(0.0).max(0.0);

    let bid = parse_f64(&book_ticker.bid_price).unwrap_or(0.0);
    let ask = parse_f64(&book_ticker.ask_price).unwrap_or(0.0);
    let spread_pct = if bid > 0.0 && ask > 0.0 && ask >= bid {
        let mid = (ask + bid) / 2.0;
        if mid > 0.0 {
            ((ask - bid) / mid).max(0.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    let volume_24h = parse_f64(&ticker_24h.quote_volume).unwrap_or(0.0).round() as i64;

    Ok(RawExchangeSnapshot {
        source: "binance",
        current_price,
        volume_24h,
        funding_rate: 0.0,
        spread_pct,
        candles,
    })
}

#[derive(Debug, Deserialize)]
struct OkxResponse<T> {
    code: String,
    msg: String,
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct OkxTicker {
    last: String,
    #[serde(rename = "bidPx")]
    bid_px: String,
    #[serde(rename = "askPx")]
    ask_px: String,
    #[serde(rename = "volCcy24h")]
    vol_ccy_24h: String,
}

async fn fetch_market_signal_okx(
    http: &reqwest::Client,
    symbol: &str,
) -> Result<RawExchangeSnapshot, String> {
    let base_url = "https://www.okx.com";
    let inst_id = okx_inst_id(symbol);
    let kline_url = format!(
        "{}/api/v5/market/candles?instId={}&bar=1m&limit={}",
        base_url, inst_id, FETCH_CANDLE_LIMIT
    );
    let ticker_url = format!("{}/api/v5/market/ticker?instId={}", base_url, inst_id);

    let kline_response = http
        .get(&kline_url)
        .send()
        .await
        .map_err(|e| format!("okx candles request failed: {}", e))?;
    if !kline_response.status().is_success() {
        return Err(format!("okx candles http {}", kline_response.status()));
    }
    let kline_response = kline_response
        .json::<OkxResponse<Vec<String>>>()
        .await
        .map_err(|e| format!("okx candles decode failed: {}", e))?;
    if kline_response.code != "0" {
        return Err(format!(
            "okx candles code={} msg={}",
            kline_response.code, kline_response.msg
        ));
    }
    let candles = parse_candles_okx(kline_response.data);
    if candles.is_empty() {
        return Err("empty okx candles".to_string());
    }

    let ticker_response = http
        .get(&ticker_url)
        .send()
        .await
        .map_err(|e| format!("okx ticker request failed: {}", e))?;
    if !ticker_response.status().is_success() {
        return Err(format!("okx ticker http {}", ticker_response.status()));
    }
    let ticker_response = ticker_response
        .json::<OkxResponse<OkxTicker>>()
        .await
        .map_err(|e| format!("okx ticker decode failed: {}", e))?;
    if ticker_response.code != "0" {
        return Err(format!(
            "okx ticker code={} msg={}",
            ticker_response.code, ticker_response.msg
        ));
    }

    let ticker = ticker_response
        .data
        .into_iter()
        .next()
        .ok_or_else(|| "missing okx ticker item".to_string())?;

    let fallback_price = candles.last().map(|c| c.close).unwrap_or(0.0);
    let current_price = parse_f64(&ticker.last).unwrap_or(fallback_price).max(0.0);

    let bid = parse_f64(&ticker.bid_px).unwrap_or(0.0);
    let ask = parse_f64(&ticker.ask_px).unwrap_or(0.0);
    let spread_pct = if bid > 0.0 && ask > 0.0 && ask >= bid {
        let mid = (ask + bid) / 2.0;
        if mid > 0.0 {
            ((ask - bid) / mid).max(0.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    let volume_24h = parse_f64(&ticker.vol_ccy_24h).unwrap_or(0.0).round() as i64;

    Ok(RawExchangeSnapshot {
        source: "okx",
        current_price,
        volume_24h,
        funding_rate: 0.0,
        spread_pct,
        candles,
    })
}

fn aggregate_signals(
    symbol: &str,
    signals: &[ExchangeSignal],
    weights: &HashMap<String, f64>,
) -> Result<MarketSignal, String> {
    if signals.is_empty() {
        return Err(format!("no exchange signal available for {}", symbol));
    }

    let mut prices: Vec<f64> = signals.iter().map(|s| s.current_price).collect();
    let mut atrs: Vec<f64> = signals.iter().map(|s| s.atr_14).collect();
    let mut spreads: Vec<f64> = signals.iter().map(|s| s.spread_pct).collect();
    let bybit_regime = signals
        .iter()
        .find(|s| s.source == "bybit")
        .map(|s| s.regime)
        .unwrap_or("neutral");
    let bybit_price = signals
        .iter()
        .find(|s| s.source == "bybit")
        .map(|s| s.current_price)
        .unwrap_or_else(|| median(&mut prices.clone()));
    let bybit_signal = signals.iter().find(|s| s.source == "bybit");
    let consensus_volume_24h = signals.iter().map(|s| s.volume_24h).min().unwrap_or(0);
    let volume_24h = bybit_signal
        .map(|s| s.volume_24h)
        .unwrap_or(consensus_volume_24h);
    let consensus_funding_rate = signals
        .iter()
        .find(|s| s.source == "bybit")
        .map(|s| s.funding_rate)
        .unwrap_or(0.0);
    let funding_rate = bybit_signal.map(|s| s.funding_rate).unwrap_or(0.0);

    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;
    let mut slope_sum = 0.0;
    let mut ema_fast_sum = 0.0;
    let mut ema_slow_sum = 0.0;
    let mut bollinger_upper_sum = 0.0;
    let mut bollinger_middle_sum = 0.0;
    let mut bollinger_lower_sum = 0.0;
    let mut bollinger_bandwidth_sum = 0.0;
    let mut bollinger_percent_b_sum = 0.0;
    let mut rsi_sum = 0.0;
    let mut macd_line_sum = 0.0;
    let mut macd_signal_sum = 0.0;
    let mut macd_histogram_sum = 0.0;
    let mut volume_ratio_sum = 0.0;
    let mut bullish_count = 0_i64;
    let mut bearish_count = 0_i64;
    for s in signals {
        let symbol_w = weights
            .get(&score_key(s.source, symbol))
            .copied()
            .or_else(|| weights.get(&score_key(s.source, "*")).copied())
            .unwrap_or(1.0);
        let w = symbol_w.clamp(0.5, 1.5);
        weighted_sum += w * s.trend_score;
        slope_sum += w * s.trend_slope;
        ema_fast_sum += w * s.ema_fast;
        ema_slow_sum += w * s.ema_slow;
        bollinger_upper_sum += w * s.bollinger_upper;
        bollinger_middle_sum += w * s.bollinger_middle;
        bollinger_lower_sum += w * s.bollinger_lower;
        bollinger_bandwidth_sum += w * s.bollinger_bandwidth;
        bollinger_percent_b_sum += w * s.bollinger_percent_b;
        rsi_sum += w * s.rsi_14;
        macd_line_sum += w * s.macd_line;
        macd_signal_sum += w * s.macd_signal;
        macd_histogram_sum += w * s.macd_histogram;
        volume_ratio_sum += w * s.volume_ratio;
        weight_total += w;
        if s.regime == "bullish" {
            bullish_count += 1;
        } else if s.regime == "bearish" {
            bearish_count += 1;
        }
    }
    let consensus_trend_score = if weight_total > 0.0 {
        (weighted_sum / weight_total).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let consensus_trend_slope = if weight_total > 0.0 {
        slope_sum / weight_total
    } else {
        0.0
    };
    let consensus_ema_fast = if weight_total > 0.0 {
        ema_fast_sum / weight_total
    } else {
        0.0
    };
    let consensus_ema_slow = if weight_total > 0.0 {
        ema_slow_sum / weight_total
    } else {
        0.0
    };
    let consensus_bollinger_upper = if weight_total > 0.0 {
        bollinger_upper_sum / weight_total
    } else {
        0.0
    };
    let consensus_bollinger_middle = if weight_total > 0.0 {
        bollinger_middle_sum / weight_total
    } else {
        0.0
    };
    let consensus_bollinger_lower = if weight_total > 0.0 {
        bollinger_lower_sum / weight_total
    } else {
        0.0
    };
    let consensus_bollinger_bandwidth = if weight_total > 0.0 {
        bollinger_bandwidth_sum / weight_total
    } else {
        0.0
    };
    let consensus_bollinger_percent_b = if weight_total > 0.0 {
        bollinger_percent_b_sum / weight_total
    } else {
        0.5
    };
    let consensus_rsi_14 = if weight_total > 0.0 {
        rsi_sum / weight_total
    } else {
        50.0
    };
    let consensus_macd_line = if weight_total > 0.0 {
        macd_line_sum / weight_total
    } else {
        0.0
    };
    let consensus_macd_signal = if weight_total > 0.0 {
        macd_signal_sum / weight_total
    } else {
        0.0
    };
    let consensus_macd_histogram = if weight_total > 0.0 {
        macd_histogram_sum / weight_total
    } else {
        0.0
    };
    let consensus_volume_ratio = if weight_total > 0.0 {
        volume_ratio_sum / weight_total
    } else {
        1.0
    };
    let exchanges_available = signals.len() as i64;
    let consensus_count = bullish_count.max(bearish_count);
    let consensus_side = if bullish_count == exchanges_available && exchanges_available > 0 {
        "bullish"
    } else if bearish_count == exchanges_available && exchanges_available > 0 {
        "bearish"
    } else {
        "neutral"
    };
    let consensus_ratio = if exchanges_available > 0 {
        consensus_count as f64 / exchanges_available as f64
    } else {
        0.0
    };
    let consensus_atr_14 = median(&mut atrs);
    let consensus_spread_pct = median(&mut spreads);

    // `current_price` must stay anchored to Bybit so entry/exit logic and the
    // displayed price match the execution venue. Other exchanges contribute to
    // consensus and directional context only.
    let current_price = bybit_signal
        .map(|s| s.current_price)
        .unwrap_or_else(|| median(&mut prices));
    let atr_14 = bybit_signal
        .map(|s| s.atr_14)
        .unwrap_or_else(|| median(&mut atrs));
    let spread_pct = bybit_signal
        .map(|s| s.spread_pct)
        .unwrap_or_else(|| median(&mut spreads));
    let trend_score = bybit_signal
        .map(|s| s.trend_score)
        .unwrap_or(consensus_trend_score);
    let trend_slope = bybit_signal
        .map(|s| s.trend_slope)
        .unwrap_or(consensus_trend_slope);
    let ema_fast = bybit_signal
        .map(|s| s.ema_fast)
        .unwrap_or(consensus_ema_fast);
    let ema_slow = bybit_signal
        .map(|s| s.ema_slow)
        .unwrap_or(consensus_ema_slow);
    let bollinger_upper = bybit_signal
        .map(|s| s.bollinger_upper)
        .unwrap_or(consensus_bollinger_upper);
    let bollinger_middle = bybit_signal
        .map(|s| s.bollinger_middle)
        .unwrap_or(consensus_bollinger_middle);
    let bollinger_lower = bybit_signal
        .map(|s| s.bollinger_lower)
        .unwrap_or(consensus_bollinger_lower);
    let bollinger_bandwidth = bybit_signal
        .map(|s| s.bollinger_bandwidth)
        .unwrap_or(consensus_bollinger_bandwidth);
    let bollinger_percent_b = bybit_signal
        .map(|s| s.bollinger_percent_b)
        .unwrap_or(consensus_bollinger_percent_b);
    let rsi_14 = bybit_signal.map(|s| s.rsi_14).unwrap_or(consensus_rsi_14);
    let macd_line = bybit_signal
        .map(|s| s.macd_line)
        .unwrap_or(consensus_macd_line);
    let macd_signal = bybit_signal
        .map(|s| s.macd_signal)
        .unwrap_or(consensus_macd_signal);
    let macd_histogram = bybit_signal
        .map(|s| s.macd_histogram)
        .unwrap_or(consensus_macd_histogram);
    let volume_ratio = bybit_signal
        .map(|s| s.volume_ratio)
        .unwrap_or(consensus_volume_ratio);
    let regime = consensus_side.to_string();

    Ok(MarketSignal {
        symbol: symbol.to_string(),
        current_price,
        bybit_price,
        atr_14,
        volume_24h,
        funding_rate,
        trend_score,
        spread_pct,
        consensus_atr_14,
        consensus_volume_24h,
        consensus_funding_rate,
        consensus_trend_score,
        consensus_spread_pct,
        consensus_trend_slope,
        ema_fast,
        ema_slow,
        bollinger_upper,
        bollinger_middle,
        bollinger_lower,
        bollinger_bandwidth,
        bollinger_percent_b,
        consensus_ema_fast,
        consensus_ema_slow,
        consensus_bollinger_upper,
        consensus_bollinger_middle,
        consensus_bollinger_lower,
        consensus_bollinger_bandwidth,
        consensus_bollinger_percent_b,
        rsi_14,
        consensus_rsi_14,
        macd_line,
        macd_signal,
        macd_histogram,
        consensus_macd_line,
        consensus_macd_signal,
        consensus_macd_histogram,
        volume_ratio,
        consensus_volume_ratio,
        btc_regime: "neutral".to_string(),
        btc_trend_score: 0.0,
        btc_consensus_count: 0,
        btc_volume_ratio: 1.0,
        regime,
        consensus_side: consensus_side.to_string(),
        consensus_count,
        exchanges_available,
        consensus_ratio,
        trend_slope,
        bybit_regime: bybit_regime.to_string(),
        bullish_exchanges: bullish_count,
        bearish_exchanges: bearish_count,
    })
}

async fn fetch_market_signal(
    http: &reqwest::Client,
    base_url: &str,
    symbol: &str,
    weights: &HashMap<String, f64>,
) -> Result<MarketSignal, String> {
    let mut raw_snapshots = Vec::<RawExchangeSnapshot>::new();
    let mut errors = Vec::<String>::new();

    match fetch_market_signal_bybit(http, base_url, symbol).await {
        Ok(v) => raw_snapshots.push(v),
        Err(e) => errors.push(format!("bybit={}", e)),
    }
    match fetch_market_signal_binance(http, symbol).await {
        Ok(v) => raw_snapshots.push(v),
        Err(e) => errors.push(format!("binance={}", e)),
    }
    match fetch_market_signal_okx(http, symbol).await {
        Ok(v) => raw_snapshots.push(v),
        Err(e) => errors.push(format!("okx={}", e)),
    }

    if !errors.is_empty() {
        return Err(format!(
            "incomplete source set for {}: {}",
            symbol,
            errors.join(" | ")
        ));
    }

    if raw_snapshots.len() != 3 {
        return Err(format!(
            "incomplete source set for {}: expected 3 exchanges, got {}",
            symbol,
            raw_snapshots.len()
        ));
    }

    align_exchange_candles(symbol, &mut raw_snapshots)?;
    let exchanges = raw_snapshots
        .into_iter()
        .map(|snapshot| build_exchange_signal(symbol, snapshot))
        .collect::<Result<Vec<_>, _>>()?;

    let signal = aggregate_signals(symbol, &exchanges, weights)?;
    let used_sources = exchanges
        .iter()
        .map(|s| s.source)
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "Consensus market signal {} using sources=[{}] failures={}",
        symbol,
        used_sources,
        if errors.is_empty() {
            "none".to_string()
        } else {
            errors.join(" | ")
        }
    );
    Ok(signal)
}

fn apply_btc_context(signal: &mut MarketSignal, btc_signal: &MarketSignal) {
    if signal.symbol.eq_ignore_ascii_case("BTCUSDT") {
        return;
    }

    signal.btc_regime = btc_signal.regime.clone();
    signal.btc_trend_score = btc_signal.consensus_trend_score;
    signal.btc_consensus_count = btc_signal.consensus_count;
    signal.btc_volume_ratio = btc_signal.consensus_volume_ratio;
}

fn stabilize_consensus_side(
    signal: &mut MarketSignal,
    state: &mut HashMap<String, ConsensusLatchState>,
) {
    let symbol = signal.symbol.clone();
    let observed_side = signal.consensus_side.clone();

    let entry = state.entry(symbol).or_insert_with(|| ConsensusLatchState {
        stable_side: observed_side.clone(),
        pending_side: observed_side.clone(),
        pending_count: 0,
    });

    if observed_side == entry.stable_side {
        entry.pending_side = observed_side.clone();
        entry.pending_count = 0;
    } else {
        if observed_side == entry.pending_side {
            entry.pending_count = entry.pending_count.saturating_add(1);
        } else {
            entry.pending_side = observed_side.clone();
            entry.pending_count = 1;
        }

        if entry.pending_count >= CONSENSUS_SIDE_CONFIRMATION_CYCLES {
            entry.stable_side = observed_side.clone();
            entry.pending_side = observed_side.clone();
            entry.pending_count = 0;
        }
    }

    signal.consensus_side = entry.stable_side.clone();
    signal.regime = entry.stable_side.clone();
}

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-market-data");

    let listener = TcpListener::bind("0.0.0.0:8081").await?;
    println!("Health check server running on :8081");
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
                                eprintln!("failed to write to socket; err = {:?}", e);
                            }
                        });
                    }
                }
            }
        }
    });

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    println!("Connecting to Redis at {}", redis_url);

    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    let bybit_env = resolve_runtime_bybit_env();
    let symbols = parse_trading_pairs();
    let base_url = bybit_base_url();
    let analytics_scores_url = std::env::var("ANALYTICS_SCORES_URL")
        .unwrap_or_else(|_| "http://analytics:8086/scores".to_string());
    let analytics_min_evaluated = std::env::var("ANALYTICS_MIN_EVALUATED")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .max(1);
    let mut consensus_latch = HashMap::<String, ConsensusLatchState>::new();
    println!(
        "Market-data running in BYBIT_ENV={} base_url={} analytics_scores_url={} with pairs={}",
        bybit_env,
        base_url,
        analytics_scores_url,
        symbols.join(",")
    );

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("vipertrade-market-data/0.8")
        .build()?;

    loop {
        if *shutdown_rx.borrow() {
            println!("Received shutdown signal, stopping viper-market-data");
            break;
        }

        let weights =
            fetch_analytics_weights(&http, &analytics_scores_url, analytics_min_evaluated).await;

        let btc_context = match fetch_market_signal(&http, &base_url, "BTCUSDT", &weights).await {
            Ok(mut signal) => {
                stabilize_consensus_side(&mut signal, &mut consensus_latch);
                signal
            }
            Err(err) => {
                latest_signals.write().await.clear();
                eprintln!(
                    "Failed to refresh BTC macro context; clearing latest signals and skipping cycle: {}",
                    err
                );
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        println!("Received shutdown signal, stopping viper-market-data");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                }
                continue;
            }
        };

        let mut cycle_signals = HashMap::<String, MarketSignal>::new();
        let mut cycle_failed = false;

        for symbol in &symbols {
            match fetch_market_signal(&http, &base_url, symbol, &weights).await {
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
                        eprintln!(
                            "{}",
                            serde_json::json!({
                                "service": "market-data",
                                "event": "invalid_market_signal_dropped",
                                "symbol": drop.symbol,
                                "stage": drop.stage,
                                "reason": drop.reason,
                                "timestamp": drop.timestamp,
                            })
                        );
                        cycle_failed = true;
                        break;
                    }
                    cycle_signals.insert(symbol.clone(), signal);
                }
                Err(err) => {
                    cycle_failed = true;
                    eprintln!("Failed to fetch market data for {}: {}", symbol, err);
                    break;
                }
            }
        }

        if cycle_failed || cycle_signals.len() != symbols.len() {
            latest_signals.write().await.clear();
            eprintln!(
                "Skipping market-data publish cycle because signals are incomplete: expected {} complete symbols, got {}",
                symbols.len(),
                cycle_signals.len()
            );
        } else {
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
                    eprintln!(
                        "{}",
                        serde_json::json!({
                            "service": "market-data",
                            "event": "invalid_market_signal_dropped",
                            "symbol": drop.symbol,
                            "stage": drop.stage,
                            "reason": drop.reason,
                            "timestamp": drop.timestamp,
                        })
                    );
                    continue;
                }
                let json = serde_json::to_string(&event)?;
                if let Err(e) = conn.publish::<_, _, ()>("viper:market_data", json).await {
                    eprintln!("Failed to publish market data: {}", e);
                    break;
                }
                println!(
                    "Published real market event {} for {}",
                    event.event_id, event.signal.symbol
                );
            }
        }

        tokio::select! {
            _ = shutdown_rx.changed() => {
                println!("Received shutdown signal, stopping viper-market-data");
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}
