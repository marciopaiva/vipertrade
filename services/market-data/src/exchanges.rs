use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::consensus::{aggregate_signals, ExchangeSignal};
use crate::indicators::{
    classify_regime, composite_trend_score, compute_adx14, compute_atr14,
    compute_indicator_bundle_complete, Candle, REQUIRED_CANDLE_COUNT,
};

pub(crate) const FETCH_CANDLE_LIMIT: usize = 220;
pub(crate) const CANDLE_INTERVAL_MS: i64 = 60_000;

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

#[derive(Debug, Clone)]
pub(crate) struct RawExchangeSnapshot {
    pub(crate) source: &'static str,
    pub(crate) current_price: f64,
    pub(crate) volume_24h: i64,
    pub(crate) funding_rate: f64,
    pub(crate) spread_pct: f64,
    pub(crate) candles: Vec<Candle>,
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

pub(crate) fn unix_time_ms() -> i64 {
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

pub(crate) fn align_exchange_candles(
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

pub(crate) fn parse_candles(rows: Vec<Vec<String>>) -> Vec<Candle> {
    rows.into_iter()
        .filter_map(|row| {
            if row.len() < 5 {
                return None;
            }
            let open_time_ms = row[0].parse::<i64>().ok()?;
            let high = row[2].parse::<f64>().unwrap_or(0.0).max(0.0);
            let low = row[3].parse::<f64>().unwrap_or(0.0).max(0.0);
            let close = row[4].parse::<f64>().unwrap_or(0.0).max(0.0);
            let volume_quote = row
                .get(5)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
                .max(0.0);
            if close <= 0.0 {
                return None;
            }
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

pub(crate) fn parse_candles_binance(rows: Vec<Vec<Value>>) -> Vec<Candle> {
    rows.into_iter()
        .filter_map(|row| {
            if row.len() < 5 {
                return None;
            }
            let open_time_ms = row[0].as_i64()?;
            let high = row[2].as_f64().unwrap_or(0.0).max(0.0);
            let low = row[3].as_f64().unwrap_or(0.0).max(0.0);
            let close = row[4].as_f64().unwrap_or(0.0).max(0.0);
            let volume_quote = row[5].as_f64().unwrap_or(0.0).max(0.0);
            if close <= 0.0 {
                return None;
            }
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

pub(crate) fn parse_candles_okx(rows: Vec<Vec<String>>) -> Vec<Candle> {
    rows.into_iter()
        .filter_map(|row| {
            if row.len() < 5 {
                return None;
            }
            let open_time_ms = row[0].parse::<i64>().ok()?;
            let high = row[2].parse::<f64>().unwrap_or(0.0).max(0.0);
            let low = row[3].parse::<f64>().unwrap_or(0.0).max(0.0);
            let close = row[4].parse::<f64>().unwrap_or(0.0).max(0.0);
            let volume_quote = row
                .get(7)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
                .max(0.0);
            if close <= 0.0 {
                return None;
            }
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

pub(crate) fn okx_inst_id(symbol: &str) -> String {
    let base = symbol.strip_suffix("USDT").unwrap_or(symbol);
    format!("{}-USDT-SWAP", base)
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

pub(crate) fn build_exchange_signal(
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
    let adx_14 = compute_adx14(&snapshot.candles).unwrap_or(0.0);
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
        adx_14,
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
    let current_price = viper_domain::config::parse_f64(&ticker.last_price)
        .unwrap_or(fallback_price)
        .max(0.0);

    let bid = viper_domain::config::parse_f64(&ticker.bid1_price).unwrap_or(0.0);
    let ask = viper_domain::config::parse_f64(&ticker.ask1_price).unwrap_or(0.0);
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

    let turnover_24h = viper_domain::config::parse_f64(&ticker.turnover_24h).unwrap_or(0.0);
    let fallback_volume =
        viper_domain::config::parse_f64(&ticker.volume_24h).unwrap_or(0.0) * current_price;
    let volume_24h = turnover_24h.max(fallback_volume).round() as i64;

    let funding_rate = ticker
        .funding_rate
        .as_deref()
        .and_then(viper_domain::config::parse_f64)
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

    let bid = viper_domain::config::parse_f64(&book_ticker.bid_price).unwrap_or(0.0);
    let ask = viper_domain::config::parse_f64(&book_ticker.ask_price).unwrap_or(0.0);
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

    let volume_24h = viper_domain::config::parse_f64(&ticker_24h.quote_volume)
        .unwrap_or(0.0)
        .round() as i64;

    Ok(RawExchangeSnapshot {
        source: "binance",
        current_price,
        volume_24h,
        funding_rate: 0.0,
        spread_pct,
        candles,
    })
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
    let current_price = viper_domain::config::parse_f64(&ticker.last)
        .unwrap_or(fallback_price)
        .max(0.0);

    let bid = viper_domain::config::parse_f64(&ticker.bid_px).unwrap_or(0.0);
    let ask = viper_domain::config::parse_f64(&ticker.ask_px).unwrap_or(0.0);
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

    let volume_24h = viper_domain::config::parse_f64(&ticker.vol_ccy_24h)
        .unwrap_or(0.0)
        .round() as i64;

    Ok(RawExchangeSnapshot {
        source: "okx",
        current_price,
        volume_24h,
        funding_rate: 0.0,
        spread_pct,
        candles,
    })
}

pub(crate) async fn fetch_market_signal(
    http: &reqwest::Client,
    base_url: &str,
    symbol: &str,
    weights: &HashMap<String, f64>,
    min_exchanges: usize,
) -> Result<viper_domain::MarketSignal, String> {
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

    if raw_snapshots.len() < min_exchanges {
        return Err(format!(
            "incomplete source set for {}: got {} exchanges, need at least {}. Errors: {}",
            symbol,
            raw_snapshots.len(),
            min_exchanges,
            errors.join(" | ")
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
    let failures = if errors.is_empty() {
        "none"
    } else {
        &errors.join(" | ")
    };
    tracing::info!(
        symbol = %symbol,
        sources = %used_sources,
        failures = %failures,
        "Consensus market signal"
    );
    Ok(signal)
}
