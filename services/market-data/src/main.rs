use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
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

#[derive(Clone, Copy)]
struct Candle {
    high: f64,
    low: f64,
    close: f64,
    volume_quote: f64,
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
    rsi_14: f64,
    macd_line: f64,
    macd_signal: f64,
    macd_histogram: f64,
    volume_ratio: f64,
    regime: &'static str,
    trend_slope: f64,
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

fn parse_trading_pairs() -> Vec<String> {
    let raw = std::env::var("TRADING_PAIRS")
        .unwrap_or_else(|_| "DOGEUSDT,XRPUSDT,ADAUSDT,XLMUSDT".to_string());
    let pairs: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    if pairs.is_empty() {
        vec![
            "DOGEUSDT".to_string(),
            "XRPUSDT".to_string(),
            "ADAUSDT".to_string(),
            "XLMUSDT".to_string(),
        ]
    } else {
        pairs
    }
}

fn parse_f64(raw: &str) -> Option<f64> {
    raw.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn parse_candles(rows: Vec<Vec<String>>) -> Vec<Candle> {
    let mut candles: Vec<Candle> = rows
        .into_iter()
        .filter_map(|row| {
            if row.len() < 5 {
                return None;
            }
            let high = parse_f64(&row[2])?;
            let low = parse_f64(&row[3])?;
            let close = parse_f64(&row[4])?;
            let volume_quote = row.get(6).and_then(|v| parse_f64(v)).unwrap_or(0.0);
            Some(Candle {
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
            if row.len() < 5 {
                return None;
            }
            let high = row[2].as_str().and_then(parse_f64)?;
            let low = row[3].as_str().and_then(parse_f64)?;
            let close = row[4].as_str().and_then(parse_f64)?;
            let volume_quote = row
                .get(7)
                .and_then(Value::as_str)
                .and_then(parse_f64)
                .unwrap_or(0.0);
            Some(Candle {
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
            if row.len() < 5 {
                return None;
            }
            let high = parse_f64(&row[2])?;
            let low = parse_f64(&row[3])?;
            let close = parse_f64(&row[4])?;
            let volume_quote = row.get(7).and_then(|v| parse_f64(v)).unwrap_or(0.0);
            Some(Candle {
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

fn compute_indicator_bundle(candles: &[Candle]) -> (f64, f64, f64, f64, f64, f64, f64) {
    let ema_fast = compute_ema(candles, 20).unwrap_or(0.0);
    let ema_slow = compute_ema(candles, 50).unwrap_or(0.0);
    let rsi_14 = compute_rsi14(candles).unwrap_or(50.0);
    let (macd_line, macd_signal, macd_histogram) = compute_macd(candles).unwrap_or((0.0, 0.0, 0.0));
    let volume_ratio = compute_volume_ratio(candles, 20).unwrap_or(1.0);
    (
        ema_fast,
        ema_slow,
        rsi_14,
        macd_line,
        macd_signal,
        macd_histogram,
        volume_ratio,
    )
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

fn prefer_bybit_for_decisions() -> bool {
    matches!(
        std::env::var("TRADING_MODE")
            .unwrap_or_else(|_| "paper".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "testnet"
    )
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

async fn fetch_market_signal_bybit(
    http: &reqwest::Client,
    base_url: &str,
    symbol: &str,
) -> Result<ExchangeSignal, String> {
    let kline_url = format!(
        "{}/v5/market/kline?category=linear&symbol={}&interval=1&limit=200",
        base_url, symbol
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

    let atr_14 = compute_atr14(&candles);

    let (ema_fast, ema_slow, rsi_14, macd_line, macd_signal, macd_histogram, volume_ratio) =
        compute_indicator_bundle(&candles);
    let trend_score = composite_trend_score(
        current_price,
        ema_fast,
        ema_slow,
        rsi_14,
        macd_histogram,
        volume_ratio,
    );
    let (regime, trend_slope) = classify_regime(&candles, trend_score);

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

    Ok(ExchangeSignal {
        source: "bybit",
        current_price,
        atr_14,
        volume_24h,
        funding_rate,
        trend_score,
        spread_pct,
        ema_fast,
        ema_slow,
        rsi_14,
        macd_line,
        macd_signal,
        macd_histogram,
        volume_ratio,
        regime,
        trend_slope,
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
) -> Result<ExchangeSignal, String> {
    let base_url = "https://fapi.binance.com";
    let kline_url = format!(
        "{}/fapi/v1/klines?symbol={}&interval=1m&limit=200",
        base_url, symbol
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
    let atr_14 = compute_atr14(&candles);
    let (ema_fast, ema_slow, rsi_14, macd_line, macd_signal, macd_histogram, volume_ratio) =
        compute_indicator_bundle(&candles);
    let trend_score = composite_trend_score(
        current_price,
        ema_fast,
        ema_slow,
        rsi_14,
        macd_histogram,
        volume_ratio,
    );
    let (regime, trend_slope) = classify_regime(&candles, trend_score);

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

    Ok(ExchangeSignal {
        source: "binance",
        current_price,
        atr_14,
        volume_24h,
        funding_rate: 0.0,
        trend_score,
        spread_pct,
        ema_fast,
        ema_slow,
        rsi_14,
        macd_line,
        macd_signal,
        macd_histogram,
        volume_ratio,
        regime,
        trend_slope,
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
) -> Result<ExchangeSignal, String> {
    let base_url = "https://www.okx.com";
    let inst_id = okx_inst_id(symbol);
    let kline_url = format!(
        "{}/api/v5/market/candles?instId={}&bar=1m&limit=200",
        base_url, inst_id
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
    let atr_14 = compute_atr14(&candles);
    let (ema_fast, ema_slow, rsi_14, macd_line, macd_signal, macd_histogram, volume_ratio) =
        compute_indicator_bundle(&candles);
    let trend_score = composite_trend_score(
        current_price,
        ema_fast,
        ema_slow,
        rsi_14,
        macd_histogram,
        volume_ratio,
    );
    let (regime, trend_slope) = classify_regime(&candles, trend_score);

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

    Ok(ExchangeSignal {
        source: "okx",
        current_price,
        atr_14,
        volume_24h,
        funding_rate: 0.0,
        trend_score,
        spread_pct,
        ema_fast,
        ema_slow,
        rsi_14,
        macd_line,
        macd_signal,
        macd_histogram,
        volume_ratio,
        regime,
        trend_slope,
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
    let volume_24h = signals.iter().map(|s| s.volume_24h).min().unwrap_or(0);
    let funding_rate = signals
        .iter()
        .find(|s| s.source == "bybit")
        .map(|s| s.funding_rate)
        .unwrap_or(0.0);
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

    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;
    let mut slope_sum = 0.0;
    let mut ema_fast_sum = 0.0;
    let mut ema_slow_sum = 0.0;
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
    let trend_score = if weight_total > 0.0 {
        (weighted_sum / weight_total).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let trend_slope = if weight_total > 0.0 {
        slope_sum / weight_total
    } else {
        0.0
    };
    let ema_fast = if weight_total > 0.0 {
        ema_fast_sum / weight_total
    } else {
        0.0
    };
    let ema_slow = if weight_total > 0.0 {
        ema_slow_sum / weight_total
    } else {
        0.0
    };
    let rsi_14 = if weight_total > 0.0 {
        rsi_sum / weight_total
    } else {
        50.0
    };
    let macd_line = if weight_total > 0.0 {
        macd_line_sum / weight_total
    } else {
        0.0
    };
    let macd_signal = if weight_total > 0.0 {
        macd_signal_sum / weight_total
    } else {
        0.0
    };
    let macd_histogram = if weight_total > 0.0 {
        macd_histogram_sum / weight_total
    } else {
        0.0
    };
    let volume_ratio = if weight_total > 0.0 {
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

    let prefer_bybit = prefer_bybit_for_decisions();
    let current_price = if prefer_bybit {
        bybit_signal
            .map(|s| s.current_price)
            .unwrap_or_else(|| median(&mut prices))
    } else {
        median(&mut prices)
    };
    let atr_14 = if prefer_bybit {
        bybit_signal
            .map(|s| s.atr_14)
            .unwrap_or_else(|| median(&mut atrs))
    } else {
        median(&mut atrs)
    };
    let spread_pct = if prefer_bybit {
        bybit_signal
            .map(|s| s.spread_pct)
            .unwrap_or_else(|| median(&mut spreads))
    } else {
        median(&mut spreads)
    };
    let trend_score = if prefer_bybit {
        bybit_signal.map(|s| s.trend_score).unwrap_or(trend_score)
    } else {
        trend_score
    };
    let trend_slope = if prefer_bybit {
        bybit_signal.map(|s| s.trend_slope).unwrap_or(trend_slope)
    } else {
        trend_slope
    };
    let ema_fast = if prefer_bybit {
        bybit_signal.map(|s| s.ema_fast).unwrap_or(ema_fast)
    } else {
        ema_fast
    };
    let ema_slow = if prefer_bybit {
        bybit_signal.map(|s| s.ema_slow).unwrap_or(ema_slow)
    } else {
        ema_slow
    };
    let rsi_14 = if prefer_bybit {
        bybit_signal.map(|s| s.rsi_14).unwrap_or(rsi_14)
    } else {
        rsi_14
    };
    let macd_line = if prefer_bybit {
        bybit_signal.map(|s| s.macd_line).unwrap_or(macd_line)
    } else {
        macd_line
    };
    let macd_signal = if prefer_bybit {
        bybit_signal.map(|s| s.macd_signal).unwrap_or(macd_signal)
    } else {
        macd_signal
    };
    let macd_histogram = if prefer_bybit {
        bybit_signal
            .map(|s| s.macd_histogram)
            .unwrap_or(macd_histogram)
    } else {
        macd_histogram
    };
    let volume_ratio = if prefer_bybit {
        bybit_signal.map(|s| s.volume_ratio).unwrap_or(volume_ratio)
    } else {
        volume_ratio
    };
    let regime = if prefer_bybit {
        bybit_signal
            .map(|s| s.regime)
            .unwrap_or(consensus_side)
            .to_string()
    } else {
        consensus_side.to_string()
    };

    Ok(MarketSignal {
        symbol: symbol.to_string(),
        current_price,
        bybit_price,
        atr_14,
        volume_24h,
        funding_rate,
        trend_score,
        spread_pct,
        ema_fast,
        ema_slow,
        rsi_14,
        macd_line,
        macd_signal,
        macd_histogram,
        volume_ratio,
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
    let mut exchanges = Vec::<ExchangeSignal>::new();
    let mut errors = Vec::<String>::new();

    match fetch_market_signal_bybit(http, base_url, symbol).await {
        Ok(v) => exchanges.push(v),
        Err(e) => errors.push(format!("bybit={}", e)),
    }
    match fetch_market_signal_binance(http, symbol).await {
        Ok(v) => exchanges.push(v),
        Err(e) => errors.push(format!("binance={}", e)),
    }
    match fetch_market_signal_okx(http, symbol).await {
        Ok(v) => exchanges.push(v),
        Err(e) => errors.push(format!("okx={}", e)),
    }

    if exchanges.is_empty() {
        return Err(format!("all sources failed: {}", errors.join(" | ")));
    }

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
    signal.btc_trend_score = btc_signal.trend_score;
    signal.btc_consensus_count = btc_signal.consensus_count;
    signal.btc_volume_ratio = btc_signal.volume_ratio;
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
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = health_shutdown_rx.changed() => {
                    break;
                }
                accept_result = listener.accept() => {
                    if let Ok((mut socket, _)) = accept_result {
                        let latest_signals_for_conn = Arc::clone(&latest_signals_for_health);
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
                                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_string()
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
            Ok(signal) => Some(signal),
            Err(err) => {
                eprintln!("Failed to refresh BTC macro context: {}", err);
                None
            }
        };

        for symbol in &symbols {
            match fetch_market_signal(&http, &base_url, symbol, &weights).await {
                Ok(mut signal) => {
                    if let Some(btc_signal) = &btc_context {
                        apply_btc_context(&mut signal, btc_signal);
                    }
                    latest_signals
                        .write()
                        .await
                        .insert(symbol.clone(), signal.clone());

                    let event = MarketSignalEvent::new(signal);
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
                Err(err) => {
                    eprintln!("Failed to fetch market data for {}: {}", symbol, err);
                }
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
