use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use serde_yaml::Value as YamlValue;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::error::Error;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{watch, RwLock};

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

fn bybit_base_url() -> String {
    if let Ok(override_url) = std::env::var("BYBIT_HTTP_PUBLIC") {
        if !override_url.trim().is_empty() {
            return override_url;
        }
    }

    let env = match std::env::var("TRADING_MODE")
        .unwrap_or_else(|_| "paper".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "testnet" => "testnet".to_string(),
        "mainnet" | "paper" | "live" => "mainnet".to_string(),
        _ => std::env::var("BYBIT_ENV").unwrap_or_else(|_| "testnet".to_string()),
    };
    if env.eq_ignore_ascii_case("mainnet") {
        "https://api.bybit.com".to_string()
    } else {
        "https://api-testnet.bybit.com".to_string()
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
    let rsi = compute_rsi14(&candles).unwrap_or(50.0);
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
    let rsi = compute_rsi14(&candles).unwrap_or(50.0);
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
    let rsi = compute_rsi14(&candles).unwrap_or(50.0);
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
    println!("Starting viper-analytics");

    let listener = TcpListener::bind("0.0.0.0:8086").await?;
    println!("Health/metrics server running on :8086");

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

    let db_host = std::env::var("DB_HOST").unwrap_or_else(|_| "postgres".to_string());
    let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "vipertrade".to_string());
    let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "viper".to_string());
    let db_password = std::env::var("DB_PASSWORD").unwrap_or_default();
    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        db_user, db_password, db_host, db_port, db_name
    );

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    ensure_schema(&pool).await?;

    let score_cache = Arc::new(RwLock::new(ScoresSnapshot {
        updated_at: Utc::now().to_rfc3339(),
        horizon_minutes,
        lookback_hours,
        exchanges: vec![],
        by_symbol: vec![],
    }));

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let mut health_shutdown_rx = shutdown_rx.clone();
    let score_cache_for_health = Arc::clone(&score_cache);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = health_shutdown_rx.changed() => {
                    break;
                }
                accept_result = listener.accept() => {
                    if let Ok((mut socket, _)) = accept_result {
                        let score_cache_for_conn = Arc::clone(&score_cache_for_health);
                        tokio::spawn(async move {
                            let mut request_buf = [0_u8; 2048];
                            let bytes_read = socket.read(&mut request_buf).await.unwrap_or(0);
                            let request = String::from_utf8_lossy(&request_buf[..bytes_read]);

                            let response = if request.starts_with("GET /scores") {
                                let payload = score_cache_for_conn.read().await.clone();
                                let body = serde_json::to_string(&payload)
                                    .unwrap_or_else(|_| "{\"error\":\"encode_failed\"}".to_string());
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

    let bybit_base = bybit_base_url();
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("vipertrade-analytics/0.8")
        .build()?;

    println!(
        "Analytics enabled with horizon={}m lookback={}h sample={}s symbols={}",
        horizon_minutes,
        lookback_hours,
        sample_interval_seconds,
        symbols.join(",")
    );

    loop {
        if *shutdown_rx.borrow() {
            println!("Received shutdown signal, stopping viper-analytics");
            break;
        }

        for symbol in &symbols {
            match fetch_bybit_snapshot(&http, &bybit_base, symbol).await {
                Ok((price, trend_score)) => {
                    if let Err(e) =
                        insert_snapshot(&pool, "bybit", symbol, price, trend_score).await
                    {
                        eprintln!("insert bybit snapshot failed {}: {}", symbol, e);
                    }
                }
                Err(e) => eprintln!("fetch bybit failed {}: {}", symbol, e),
            }

            match fetch_binance_snapshot(&http, symbol).await {
                Ok((price, trend_score)) => {
                    if let Err(e) =
                        insert_snapshot(&pool, "binance", symbol, price, trend_score).await
                    {
                        eprintln!("insert binance snapshot failed {}: {}", symbol, e);
                    }
                }
                Err(e) => eprintln!("fetch binance failed {}: {}", symbol, e),
            }

            match fetch_okx_snapshot(&http, symbol).await {
                Ok((price, trend_score)) => {
                    if let Err(e) = insert_snapshot(&pool, "okx", symbol, price, trend_score).await
                    {
                        eprintln!("insert okx snapshot failed {}: {}", symbol, e);
                    }
                }
                Err(e) => eprintln!("fetch okx failed {}: {}", symbol, e),
            }
        }

        match compute_scores(&pool, horizon_minutes, lookback_hours).await {
            Ok(scores) => {
                *score_cache.write().await = scores;
            }
            Err(e) => eprintln!("compute scores failed: {}", e),
        }

        tokio::select! {
            _ = shutdown_rx.changed() => {
                println!("Received shutdown signal, stopping viper-analytics");
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(sample_interval_seconds)) => {}
        }
    }

    let _ = shutdown_tx.send(true);
    Ok(())
}
