use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, RwLock, Semaphore};
use tracing::{error, info, warn};
use viper_domain::config::*;
use warp::Filter;

#[derive(Clone, Copy)]
struct Candle {
    close: f64,
}

#[derive(Debug, Serialize, Clone, Default)]
struct ExchangeScore {
    exchange: String,
    evaluated: i64,
    hits: i64,
    hit_rate: f64,
    avg_forward_return: f64,
}

#[derive(Debug, Serialize, Clone, Default)]
struct SymbolScore {
    exchange: String,
    symbol: String,
    evaluated: i64,
    hits: i64,
    hit_rate: f64,
    avg_forward_return: f64,
}

#[derive(Debug, Serialize, Clone, Default)]
struct ScoresSnapshot {
    updated_at: String,
    horizon_minutes: i64,
    lookback_hours: i64,
    exchanges: Vec<ExchangeScore>,
    by_symbol: Vec<SymbolScore>,
}

const RSI_PERIOD: usize = 14;

fn compute_rsi_wilder(candles: &[Candle]) -> Option<f64> {
    if candles.len() < RSI_PERIOD + 1 {
        return None;
    }

    let mut avg_gain = 0.0;
    let mut avg_loss = 0.0;

    for i in 1..=RSI_PERIOD {
        let delta = candles[candles.len() - RSI_PERIOD - 1 + i].close - candles[candles.len() - RSI_PERIOD + i - 1].close;
        if delta >= 0.0 {
            avg_gain += delta;
        } else {
            avg_loss -= delta;
        }
    }

    avg_gain /= RSI_PERIOD as f64;
    avg_loss /= RSI_PERIOD as f64;

    for i in (candles.len() - RSI_PERIOD)..candles.len() {
        let delta = candles[i].close - candles[i - 1].close;
        let gain = if delta > 0.0 { delta } else { 0.0 };
        let loss = if delta < 0.0 { -delta } else { 0.0 };

        avg_gain = (avg_gain * (RSI_PERIOD - 1) as f64 + gain) / RSI_PERIOD as f64;
        avg_loss = (avg_loss * (RSI_PERIOD - 1) as f64 + loss) / RSI_PERIOD as f64;
    }

    if avg_loss < f64::EPSILON {
        return Some(100.0);
    }

    let rs = avg_gain / avg_loss;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

struct ExchangeClients {
    bybit: reqwest::Client,
    binance: reqwest::Client,
    okx: reqwest::Client,
}

impl ExchangeClients {
    fn new() -> Result<Self, reqwest::Error> {
        let bybit = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("vipertrade-analytics/0.9")
            .build()?;

        let binance = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("vipertrade-analytics/0.9")
            .build()?;

        let okx = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("vipertrade-analytics/0.9")
            .build()?;

        Ok(Self { bybit, binance, okx })
    }
}

async fn fetch_bybit_snapshot(
    http: &reqwest::Client,
    base_url: &str,
    symbol: &str,
) -> Result<(f64, f64), String> {
    let kline_url = format!(
        "{}/v5/market/kline?category=linear&symbol={}&interval=1&limit=200",
        base_url, symbol
    );

    let body = http
        .get(&kline_url)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    if !body.status().is_success() {
        return Err(format!("http {}", body.status()));
    }

    let payload = body
        .json::<Value>()
        .await
        .map_err(|e| format!("decode failed: {}", e))?;
    let rows = payload
        .get("result")
        .and_then(|v| v.get("list"))
        .and_then(Value::as_array)
        .ok_or_else(|| "missing result.list".to_string())?;

    let mut candles: Vec<Candle> = rows
        .iter()
        .filter_map(|row| {
            let arr = row.as_array()?;
            let close = arr.get(4)?.as_str().and_then(parse_f64)?;
            Some(Candle { close })
        })
        .collect();
    candles.reverse();

    let price = candles.last().map(|c| c.close).unwrap_or(0.0);
    let rsi = compute_rsi_wilder(&candles).unwrap_or(50.0);
    let trend_score = ((rsi - 50.0) / 50.0).clamp(-1.0, 1.0);
    Ok((price, trend_score))
}

async fn fetch_binance_snapshot(
    http: &reqwest::Client,
    symbol: &str,
) -> Result<(f64, f64), String> {
    let kline_url = format!(
        "https://fapi.binance.com/fapi/v1/klines?symbol={}&interval=1m&limit=200",
        symbol
    );

    let body = http
        .get(&kline_url)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    if !body.status().is_success() {
        return Err(format!("http {}", body.status()));
    }

    let rows = body
        .json::<Vec<Vec<Value>>>()
        .await
        .map_err(|e| format!("decode failed: {}", e))?;

    let candles: Vec<Candle> = rows
        .iter()
        .filter_map(|row| {
            let close = row.get(4)?.as_str().and_then(parse_f64)?;
            Some(Candle { close })
        })
        .collect();

    let price = candles.last().map(|c| c.close).unwrap_or(0.0);
    let rsi = compute_rsi_wilder(&candles).unwrap_or(50.0);
    let trend_score = ((rsi - 50.0) / 50.0).clamp(-1.0, 1.0);
    Ok((price, trend_score))
}

fn okx_inst_id(symbol: &str) -> String {
    let base = symbol.strip_suffix("USDT").unwrap_or(symbol);
    format!("{}-USDT-SWAP", base)
}

async fn fetch_okx_snapshot(http: &reqwest::Client, symbol: &str) -> Result<(f64, f64), String> {
    let inst_id = okx_inst_id(symbol);
    let kline_url = format!(
        "https://www.okx.com/api/v5/market/candles?instId={}&bar=1m&limit=200",
        inst_id
    );

    let body = http
        .get(&kline_url)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    if !body.status().is_success() {
        return Err(format!("http {}", body.status()));
    }

    let payload = body
        .json::<Value>()
        .await
        .map_err(|e| format!("decode failed: {}", e))?;
    let rows = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing data".to_string())?;

    let mut candles: Vec<Candle> = rows
        .iter()
        .filter_map(|row| {
            let arr = row.as_array()?;
            let close = arr.get(4)?.as_str().and_then(parse_f64)?;
            Some(Candle { close })
        })
        .collect();
    candles.reverse();

    let price = candles.last().map(|c| c.close).unwrap_or(0.0);
    let rsi = compute_rsi_wilder(&candles).unwrap_or(50.0);
    let trend_score = ((rsi - 50.0) / 50.0).clamp(-1.0, 1.0);
    Ok((price, trend_score))
}

async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS exchange_signal_snapshots (
            snapshot_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
            exchange TEXT NOT NULL CHECK (exchange IN ('bybit', 'binance', 'okx')),
            symbol TEXT NOT NULL,
            price NUMERIC NOT NULL CHECK (price > 0),
            trend_score NUMERIC NOT NULL,
            observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_exchange_signal_snapshots_lookup
            ON exchange_signal_snapshots(exchange, symbol, observed_at)",
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_snapshot(
    pool: &PgPool,
    exchange: &str,
    symbol: &str,
    price: f64,
    trend_score: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO exchange_signal_snapshots (exchange, symbol, price, trend_score)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(exchange)
    .bind(symbol)
    .bind(price)
    .bind(trend_score)
    .execute(pool)
    .await?;

    Ok(())
}

async fn compute_scores(
    pool: &PgPool,
    horizon_minutes: i64,
    lookback_hours: i64,
) -> Result<ScoresSnapshot, sqlx::Error> {
    let exchange_rows = sqlx::query_as::<_, (String, i64, i64, f64)>(
        "WITH base AS (
            SELECT exchange, symbol, observed_at, price::double precision AS price, trend_score::double precision AS trend_score
            FROM exchange_signal_snapshots
            WHERE observed_at <= NOW() - make_interval(mins => $1::int)
              AND observed_at >= NOW() - make_interval(hours => $2::int)
        ),
        paired AS (
            SELECT b.exchange, b.symbol, b.price, b.trend_score,
                   f.price::double precision AS future_price
            FROM base b
            JOIN LATERAL (
                SELECT s2.price
                FROM exchange_signal_snapshots s2
                WHERE s2.exchange = b.exchange
                  AND s2.symbol = b.symbol
                  AND s2.observed_at >= b.observed_at + make_interval(mins => $1::int)
                ORDER BY s2.observed_at ASC
                LIMIT 1
            ) f ON TRUE
        ),
        scored AS (
            SELECT exchange, symbol,
                CASE WHEN trend_score > 0 THEN 1 WHEN trend_score < 0 THEN -1 ELSE 0 END AS predicted_sign,
                CASE WHEN future_price > price THEN 1 WHEN future_price < price THEN -1 ELSE 0 END AS realized_sign,
                ((future_price - price) / NULLIF(price, 0))::double precision AS forward_return
            FROM paired
        )
        SELECT
            exchange,
            COUNT(*) FILTER (WHERE predicted_sign <> 0)::bigint,
            COUNT(*) FILTER (WHERE predicted_sign <> 0 AND predicted_sign = realized_sign)::bigint,
            COALESCE(AVG(forward_return) FILTER (WHERE predicted_sign <> 0), 0)::double precision
        FROM scored
        GROUP BY exchange
        ORDER BY exchange",
    )
    .bind(horizon_minutes)
    .bind(lookback_hours)
    .fetch_all(pool)
    .await?;

    let by_symbol_rows = sqlx::query_as::<_, (String, String, i64, i64, f64)>(
        "WITH base AS (
            SELECT exchange, symbol, observed_at, price::double precision AS price, trend_score::double precision AS trend_score
            FROM exchange_signal_snapshots
            WHERE observed_at <= NOW() - make_interval(mins => $1::int)
              AND observed_at >= NOW() - make_interval(hours => $2::int)
        ),
        paired AS (
            SELECT b.exchange, b.symbol, b.price, b.trend_score,
                   f.price::double precision AS future_price
            FROM base b
            JOIN LATERAL (
                SELECT s2.price
                FROM exchange_signal_snapshots s2
                WHERE s2.exchange = b.exchange
                  AND s2.symbol = b.symbol
                  AND s2.observed_at >= b.observed_at + make_interval(mins => $1::int)
                ORDER BY s2.observed_at ASC
                LIMIT 1
            ) f ON TRUE
        ),
        scored AS (
            SELECT exchange, symbol,
                CASE WHEN trend_score > 0 THEN 1 WHEN trend_score < 0 THEN -1 ELSE 0 END AS predicted_sign,
                CASE WHEN future_price > price THEN 1 WHEN future_price < price THEN -1 ELSE 0 END AS realized_sign,
                ((future_price - price) / NULLIF(price, 0))::double precision AS forward_return
            FROM paired
        )
        SELECT
            exchange,
            symbol,
            COUNT(*) FILTER (WHERE predicted_sign <> 0)::bigint,
            COUNT(*) FILTER (WHERE predicted_sign <> 0 AND predicted_sign = realized_sign)::bigint,
            COALESCE(AVG(forward_return) FILTER (WHERE predicted_sign <> 0), 0)::double precision
        FROM scored
        GROUP BY exchange, symbol
        ORDER BY exchange, symbol",
    )
    .bind(horizon_minutes)
    .bind(lookback_hours)
    .fetch_all(pool)
    .await?;

    let exchanges = exchange_rows
        .into_iter()
        .map(|(exchange, evaluated, hits, avg_forward_return)| {
            let hit_rate = if evaluated > 0 {
                hits as f64 / evaluated as f64
            } else {
                0.0
            };
            ExchangeScore {
                exchange,
                evaluated,
                hits,
                hit_rate,
                avg_forward_return,
            }
        })
        .collect();

    let by_symbol = by_symbol_rows
        .into_iter()
        .map(|(exchange, symbol, evaluated, hits, avg_forward_return)| {
            let hit_rate = if evaluated > 0 {
                hits as f64 / evaluated as f64
            } else {
                0.0
            };
            SymbolScore {
                exchange,
                symbol,
                evaluated,
                hits,
                hit_rate,
                avg_forward_return,
            }
        })
        .collect();

    Ok(ScoresSnapshot {
        updated_at: Utc::now().to_rfc3339(),
        horizon_minutes,
        lookback_hours,
        exchanges,
        by_symbol,
    })
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "viper_analytics=info".into()),
        )
        .json()
        .init();

    info!("Starting viper-analytics");

    let symbols = parse_trading_pairs();
    let horizon_minutes = std::env::var("ANALYTICS_HORIZON_MINUTES")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(5)
        .max(1);
    let lookback_hours = std::env::var("ANALYTICS_LOOKBACK_HOURS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(24)
        .max(1);
    let sample_interval_seconds = std::env::var("ANALYTICS_SAMPLE_INTERVAL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5)
        .max(2);

    let max_concurrent_requests: usize = std::env::var("ANALYTICS_MAX_CONCURRENT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let database_url = resolve_database_url().expect("DATABASE_URL or DB_* vars must be set");

    let pool = Arc::new(PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?);
    ensure_schema(&pool).await?;

    let score_cache: Arc<RwLock<ScoresSnapshot>> = Arc::new(RwLock::new(ScoresSnapshot {
        updated_at: Utc::now().to_rfc3339(),
        horizon_minutes,
        lookback_hours,
        exchanges: vec![],
        by_symbol: vec![],
    }));

    let clients = Arc::new(ExchangeClients::new()?);
    let semaphore = Arc::new(Semaphore::new(max_concurrent_requests));
    let bybit_base = Arc::new(resolve_bybit_base_url());

    let health_route = warp::path("health")
        .and(warp::get())
        .and(with_state(Arc::clone(&pool)))
        .and_then(handle_health);

    let scores_route = warp::path("scores")
        .and(warp::get())
        .and(with_cache(Arc::clone(&score_cache)))
        .and_then(handle_scores);

    let api_routes = health_route.or(scores_route);

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let score_cache_api = Arc::clone(&score_cache);
    tokio::spawn(async move {
        warp::serve(api_routes).run(([0, 0, 0, 0], 8086)).await;
        let _ = score_cache_api;
    });

    info!(
        horizon = horizon_minutes,
        lookback = lookback_hours,
        interval = sample_interval_seconds,
        max_concurrent = max_concurrent_requests,
        symbols = symbols.join(","),
        "Analytics configured"
    );

    loop {
        if *shutdown_rx.borrow() {
            info!("Received shutdown signal, stopping viper-analytics");
            break;
        }

        let mut tasks = vec![];

        for symbol in &symbols {
            let http_bybit = clients.bybit.clone();
            let http_binance = clients.binance.clone();
            let http_okx = clients.okx.clone();
            let pool_clone = pool.clone();
            let sem_clone = semaphore.clone();
            let bybit_base_clone = bybit_base.clone();
            let symbol_owned = symbol.clone();

            tasks.push(tokio::spawn(async move {
                let _permit = sem_clone.acquire().await.unwrap();

                match fetch_bybit_snapshot(&http_bybit, &bybit_base_clone, &symbol_owned).await {
                    Ok((price, trend_score)) => {
                        if let Err(e) = insert_snapshot(&pool_clone, "bybit", &symbol_owned, price, trend_score).await {
                            warn!(exchange = "bybit", symbol = %symbol_owned, error = %e, "insert failed");
                        }
                    }
                    Err(e) => warn!(exchange = "bybit", symbol = %symbol_owned, error = %e, "fetch failed"),
                }

                match fetch_binance_snapshot(&http_binance, &symbol_owned).await {
                    Ok((price, trend_score)) => {
                        if let Err(e) = insert_snapshot(&pool_clone, "binance", &symbol_owned, price, trend_score).await {
                            warn!(exchange = "binance", symbol = %symbol_owned, error = %e, "insert failed");
                        }
                    }
                    Err(e) => warn!(exchange = "binance", symbol = %symbol_owned, error = %e, "fetch failed"),
                }

                match fetch_okx_snapshot(&http_okx, &symbol_owned).await {
                    Ok((price, trend_score)) => {
                        if let Err(e) = insert_snapshot(&pool_clone, "okx", &symbol_owned, price, trend_score).await {
                            warn!(exchange = "okx", symbol = %symbol_owned, error = %e, "insert failed");
                        }
                    }
                    Err(e) => warn!(exchange = "okx", symbol = %symbol_owned, error = %e, "fetch failed"),
                }
            }));
        }

        for task in tasks {
            if let Err(e) = task.await {
                error!(error = %e, "fetch task failed");
            }
        }

        match compute_scores(&pool, horizon_minutes, lookback_hours).await {
            Ok(scores) => {
                *score_cache.write().await = scores;
            }
            Err(e) => error!(error = %e, "compute scores failed"),
        }

        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("Received shutdown signal, stopping viper-analytics");
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(sample_interval_seconds)) => {}
        }
    }

    let _ = shutdown_tx.send(true);
    info!("viper-analytics stopped");
    Ok(())
}

fn with_state(pool: Arc<PgPool>) -> impl Filter<Extract = (Arc<PgPool>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || pool.clone())
}

fn with_cache(cache: Arc<RwLock<ScoresSnapshot>>) -> impl Filter<Extract = (Arc<RwLock<ScoresSnapshot>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || cache.clone())
}

async fn handle_health(pool: Arc<PgPool>) -> Result<impl warp::Reply, warp::Rejection> {
    let db_connected = sqlx::query_scalar::<_, i64>("select 1")
        .fetch_one(&*pool)
        .await
        .is_ok();

    Ok(warp::reply::json(&serde_json::json!({
        "status": "ok",
        "db_connected": db_connected
    })))
}

async fn handle_scores(cache: Arc<RwLock<ScoresSnapshot>>) -> Result<impl warp::Reply, warp::Rejection> {
    let payload = cache.read().await.clone();
    Ok(warp::reply::json(&payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candles(prices: &[f64]) -> Vec<Candle> {
        prices.iter().map(|p| Candle { close: *p }).collect()
    }

    #[test]
    fn test_rsi_wilder_neutral() {
        let candles = make_candles(&[100.0; 20]);
        let rsi = compute_rsi_wilder(&candles);
        assert_eq!(rsi, Some(100.0));
    }

    #[test]
    fn test_rsi_wilder_insufficient_data() {
        let candles = make_candles(&[100.0, 101.0]);
        let rsi = compute_rsi_wilder(&candles);
        assert_eq!(rsi, None);
    }

    #[test]
    fn test_rsi_wilder_strong_uptrend() {
        let candles = make_candles(&[
            100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0, 109.0, 110.0, 111.0,
            112.0, 113.0, 114.0, 115.0, 116.0, 117.0, 118.0, 119.0, 120.0, 121.0, 122.0, 123.0,
        ]);
        let rsi = compute_rsi_wilder(&candles);
        assert!(rsi.unwrap() > 80.0);
    }

    #[test]
    fn test_rsi_wilder_strong_downtrend() {
        let candles = make_candles(&[
            123.0, 122.0, 121.0, 120.0, 119.0, 118.0, 117.0, 116.0, 115.0, 114.0, 113.0, 112.0,
            111.0, 110.0, 109.0, 108.0, 107.0, 106.0, 105.0, 104.0, 103.0, 102.0, 101.0, 100.0,
        ]);
        let rsi = compute_rsi_wilder(&candles);
        assert!(rsi.unwrap() < 20.0);
    }

    #[test]
    fn test_rsi_wilder_mixed() {
        let candles = make_candles(&[
            100.0, 102.0, 98.0, 103.0, 97.0, 104.0, 96.0, 105.0, 95.0, 106.0, 94.0, 107.0, 93.0,
            108.0, 92.0, 109.0, 91.0, 110.0, 90.0, 111.0, 89.0, 112.0, 88.0,
        ]);
        let rsi = compute_rsi_wilder(&candles);
        assert!(rsi.unwrap() > 40.0 && rsi.unwrap() < 70.0);
    }
}