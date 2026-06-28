use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use viper_domain::MarketSignal;

pub(crate) const CONSENSUS_SIDE_CONFIRMATION_CYCLES: u8 = 2;

#[derive(Debug, Clone)]
pub(crate) struct ExchangeSignal {
    pub source: &'static str,
    pub current_price: f64,
    pub atr_14: f64,
    pub adx_14: f64,
    pub volume_24h: i64,
    pub funding_rate: f64,
    pub trend_score: f64,
    pub spread_pct: f64,
    pub ema_fast: f64,
    pub ema_slow: f64,
    pub bollinger_upper: f64,
    pub bollinger_middle: f64,
    pub bollinger_lower: f64,
    pub bollinger_bandwidth: f64,
    pub bollinger_percent_b: f64,
    pub rsi_14: f64,
    pub macd_line: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub volume_ratio: f64,
    pub regime: &'static str,
    pub trend_slope: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct ConsensusLatchState {
    pub stable_side: String,
    pub pending_side: String,
    pub pending_count: u8,
}

#[derive(Debug, Serialize)]
pub(crate) struct LatestSignalsSnapshot {
    pub updated_at: String,
    pub items: HashMap<String, MarketSignal>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InvalidSignalDrop {
    pub symbol: String,
    pub stage: String,
    pub reason: String,
    pub timestamp: String,
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

fn score_key(exchange: &str, symbol: &str) -> String {
    format!("{}:{}", exchange, symbol)
}

pub(crate) fn median(values: &mut [f64]) -> f64 {
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

pub(crate) async fn fetch_analytics_weights(
    http: &reqwest::Client,
    analytics_scores_url: &str,
    min_evaluated: i64,
) -> HashMap<String, f64> {
    let mut weights = HashMap::<String, f64>::new();
    let response = match http.get(analytics_scores_url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "Analytics weights fetch failed");
            return weights;
        }
    };
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "Analytics weights HTTP error");
        return weights;
    }
    let payload = match response.json::<AnalyticsScoresResponse>().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "Analytics weights decode failed");
            return weights;
        }
    };

    for row in payload.exchanges {
        if row.evaluated >= min_evaluated && row.hit_rate.is_finite() {
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

pub(crate) fn aggregate_signals(
    symbol: &str,
    signals: &[ExchangeSignal],
    weights: &HashMap<String, f64>,
) -> Result<MarketSignal, String> {
    if signals.is_empty() {
        return Err(format!("no exchange signal available for {}", symbol));
    }

    let prices: Vec<f64> = signals.iter().map(|s| s.current_price).collect();
    let mut atrs: Vec<f64> = signals.iter().map(|s| s.atr_14).collect();
    let mut adxs: Vec<f64> = signals.iter().map(|s| s.adx_14).collect();
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
        0.0
    };
    let consensus_rsi_14 = if weight_total > 0.0 {
        rsi_sum / weight_total
    } else {
        0.0
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
        0.0
    };
    let exchanges_available = signals.len() as i64;
    let all_same_side = if bullish_count == exchanges_available {
        "bullish"
    } else if bearish_count == exchanges_available {
        "bearish"
    } else {
        "neutral"
    };
    let consensus_ratio = if exchanges_available > 0 {
        bullish_count.max(bearish_count) as f64 / exchanges_available as f64
    } else {
        0.0
    };

    Ok(MarketSignal {
        symbol: symbol.to_string(),
        current_price: bybit_price,
        bybit_price,
        atr_14: median(&mut atrs),
        adx_14: median(&mut adxs),
        volume_24h,
        funding_rate,
        trend_score: consensus_trend_score,
        spread_pct: median(&mut spreads),
        consensus_atr_14: median(&mut atrs),
        consensus_adx_14: median(&mut adxs),
        consensus_volume_24h,
        consensus_funding_rate,
        consensus_trend_score,
        consensus_spread_pct: median(&mut spreads),
        consensus_trend_slope,
        ema_fast: consensus_ema_fast,
        ema_slow: consensus_ema_slow,
        bollinger_upper: consensus_bollinger_upper,
        bollinger_middle: consensus_bollinger_middle,
        bollinger_lower: consensus_bollinger_lower,
        bollinger_bandwidth: consensus_bollinger_bandwidth,
        bollinger_percent_b: consensus_bollinger_percent_b,
        consensus_ema_fast,
        consensus_ema_slow,
        consensus_bollinger_upper,
        consensus_bollinger_middle,
        consensus_bollinger_lower,
        consensus_bollinger_bandwidth,
        consensus_bollinger_percent_b,
        rsi_14: consensus_rsi_14,
        consensus_rsi_14,
        macd_line: consensus_macd_line,
        macd_signal: consensus_macd_signal,
        macd_histogram: consensus_macd_histogram,
        consensus_macd_line,
        consensus_macd_signal,
        consensus_macd_histogram,
        volume_ratio: consensus_volume_ratio,
        consensus_volume_ratio,
        btc_regime: String::new(),
        btc_trend_score: 0.0,
        btc_consensus_count: 0,
        btc_volume_ratio: 0.0,
        regime: all_same_side.to_string(),
        consensus_side: all_same_side.to_string(),
        consensus_count: exchanges_available,
        exchanges_available,
        consensus_ratio,
        trend_slope: consensus_trend_slope,
        bybit_regime: bybit_regime.to_string(),
        bullish_exchanges: bullish_count,
        bearish_exchanges: bearish_count,
    })
}

pub(crate) fn apply_btc_context(signal: &mut MarketSignal, btc_signal: &MarketSignal) {
    if signal.symbol.eq_ignore_ascii_case("BTCUSDT") {
        return;
    }

    signal.btc_regime = btc_signal.regime.clone();
    signal.btc_trend_score = btc_signal.consensus_trend_score;
    signal.btc_consensus_count = btc_signal.consensus_count;
    signal.btc_volume_ratio = btc_signal.consensus_volume_ratio;
}

pub(crate) fn stabilize_consensus_side(
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
