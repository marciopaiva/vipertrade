use futures_util::StreamExt;
use hmac::{Hmac, Mac};
use redis::AsyncCommands;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::{watch, Mutex};
use viper_domain::{StrategyDecision, StrategyDecisionEvent};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
struct ExecutorConfig {
    redis_url: String,
    db_url: String,
    bybit_env: String,
    bybit_api_key: String,
    bybit_api_secret: String,
    recv_window: String,
    bybit_account_type: String,
    live_orders_enabled: bool,
    live_symbol_allowlist: HashSet<String>,
}

#[derive(Clone)]
struct ExecutorState {
    db_pool: Option<PgPool>,
    processed_in_memory: Arc<Mutex<HashSet<String>>>,
}

impl ExecutorConfig {
    fn from_env() -> Self {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://vipertrade-redis:6379".to_string());

        let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            let host = std::env::var("DB_HOST").unwrap_or_else(|_| "postgres".to_string());
            let port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
            let name = std::env::var("DB_NAME").unwrap_or_else(|_| "vipertrade".to_string());
            let user = std::env::var("DB_USER").unwrap_or_else(|_| "viper".to_string());
            let pass = std::env::var("DB_PASSWORD")
                .unwrap_or_else(|_| "viper_secret_password".to_string());
            format!("postgres://{}:{}@{}:{}/{}", user, pass, host, port, name)
        });

        let bybit_env = std::env::var("BYBIT_ENV").unwrap_or_else(|_| "testnet".to_string());
        let bybit_api_key = std::env::var("BYBIT_API_KEY").unwrap_or_default();
        let bybit_api_secret = std::env::var("BYBIT_API_SECRET").unwrap_or_default();
        let recv_window = std::env::var("BYBIT_RECV_WINDOW").unwrap_or_else(|_| "5000".to_string());
        let bybit_account_type =
            std::env::var("BYBIT_ACCOUNT_TYPE").unwrap_or_else(|_| "UNIFIED".to_string());
        let live_orders_enabled = std::env::var("EXECUTOR_ENABLE_LIVE_ORDERS")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let live_symbol_allowlist = parse_allowlist(
            std::env::var("EXECUTOR_LIVE_SYMBOL_ALLOWLIST")
                .unwrap_or_else(|_| "DOGEUSDT".to_string())
                .as_str(),
        );

        Self {
            redis_url,
            db_url,
            bybit_env,
            bybit_api_key,
            bybit_api_secret,
            recv_window,
            bybit_account_type,
            live_orders_enabled,
            live_symbol_allowlist,
        }
    }

    fn bybit_base_url(&self) -> &'static str {
        if self.bybit_env.eq_ignore_ascii_case("mainnet") {
            "https://api.bybit.com"
        } else {
            "https://api-testnet.bybit.com"
        }
    }

    fn is_symbol_allowed_live(&self, symbol: &str) -> bool {
        if self.live_symbol_allowlist.is_empty() {
            return true;
        }
        self.live_symbol_allowlist.contains(&symbol.to_uppercase())
    }
}

fn parse_allowlist(raw: &str) -> HashSet<String> {
    raw.split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect()
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

fn now_ms() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    ms.to_string()
}

fn bybit_sign(secret: &str, payload: &str) -> Result<String, Box<dyn Error>> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(payload.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn action_to_side(action: &str) -> Option<&'static str> {
    match action {
        "ENTER_LONG" | "CLOSE_SHORT" => Some("Buy"),
        "ENTER_SHORT" | "CLOSE_LONG" => Some("Sell"),
        _ => None,
    }
}

fn is_close_action(action: &str) -> bool {
    matches!(action, "CLOSE_LONG" | "CLOSE_SHORT")
}

fn close_action_to_position_side(action: &str) -> Option<&'static str> {
    match action {
        "CLOSE_LONG" => Some("Long"),
        "CLOSE_SHORT" => Some("Short"),
        _ => None,
    }
}

#[derive(Debug)]
enum CloseReconcileResult {
    NoLocalOpen,
    Partial {
        trade_id: String,
        remaining_qty: f64,
    },
    Closed {
        trade_id: String,
    },
    CloseQtyExceedsOpen {
        trade_id: String,
        open_qty: f64,
        close_qty: f64,
    },
}

fn decision_hash(event: &StrategyDecisionEvent) -> String {
    let mut hasher = Sha256::new();
    let payload = serde_json::to_vec(event).unwrap_or_default();
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

async fn bybit_public_get(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    path: &str,
) -> Result<Value, Box<dyn Error>> {
    let url = format!("{}{}", cfg.bybit_base_url(), path);
    let res = http.get(url).send().await?;
    let status = res.status();
    let value: Value = res.json().await?;
    if !status.is_success() {
        return Err(format!("bybit public http={} body={}", status, value).into());
    }
    Ok(value)
}

async fn bybit_private_get(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    path: &str,
    query: &str,
) -> Result<Value, Box<dyn Error>> {
    let ts = now_ms();
    let sign_payload = format!("{}{}{}{}", ts, cfg.bybit_api_key, cfg.recv_window, query);
    let sign = bybit_sign(&cfg.bybit_api_secret, &sign_payload)?;

    let mut url = format!("{}{}", cfg.bybit_base_url(), path);
    if !query.is_empty() {
        url = format!("{}?{}", url, query);
    }

    let res = http
        .get(url)
        .header("X-BAPI-API-KEY", &cfg.bybit_api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", ts)
        .header("X-BAPI-RECV-WINDOW", &cfg.recv_window)
        .send()
        .await?;

    let status = res.status();
    let value: Value = res.json().await?;
    if !status.is_success() {
        return Err(format!("bybit private http={} body={}", status, value).into());
    }
    Ok(value)
}

async fn run_bybit_sanity_checks(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
) -> Result<(), String> {
    let time_value = bybit_public_get(http, cfg, "/v5/market/time")
        .await
        .map_err(|e| format!("market/time failed: {}", e))?;

    let time_ret = time_value
        .get("retCode")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    if time_ret != 0 {
        return Err(format!(
            "market/time retCode={} body={}",
            time_ret, time_value
        ));
    }

    println!("Bybit sanity check: market/time OK");

    if cfg.bybit_api_key.is_empty() || cfg.bybit_api_secret.is_empty() {
        if cfg.live_orders_enabled {
            return Err("live orders enabled but BYBIT_API_KEY/SECRET missing".to_string());
        }
        println!("Bybit sanity check: wallet skipped (no API credentials)");
        return Ok(());
    }

    let query = format!("accountType={}", cfg.bybit_account_type);
    let wallet_value = bybit_private_get(http, cfg, "/v5/account/wallet-balance", &query)
        .await
        .map_err(|e| format!("wallet-balance failed: {}", e))?;

    let wallet_ret = wallet_value
        .get("retCode")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    if wallet_ret != 0 {
        return Err(format!(
            "wallet-balance retCode={} body={}",
            wallet_ret, wallet_value
        ));
    }

    println!(
        "Bybit sanity check: wallet-balance OK (accountType={})",
        cfg.bybit_account_type
    );

    Ok(())
}

async fn already_processed(
    state: &ExecutorState,
    source_event_id: &str,
) -> Result<bool, sqlx::Error> {
    if let Some(pool) = &state.db_pool {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1
                FROM system_events
                WHERE event_type = 'executor_event_processed'
                  AND data->>'source_event_id' = $1
            )",
        )
        .bind(source_event_id)
        .fetch_one(pool)
        .await?;

        if exists {
            return Ok(true);
        }
    }

    let seen = state.processed_in_memory.lock().await;
    Ok(seen.contains(source_event_id))
}

async fn remember_processed(state: &ExecutorState, source_event_id: &str) {
    let mut seen = state.processed_in_memory.lock().await;
    seen.insert(source_event_id.to_string());
}

async fn mark_processed(
    state: &ExecutorState,
    source_event_id: &str,
    event: &StrategyDecisionEvent,
    status: &str,
    bybit_order_id: Option<&str>,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    if let Some(pool) = &state.db_pool {
        let data = json!({
            "source_event_id": source_event_id,
            "decision_event_id": event.event_id,
            "action": event.decision.action,
            "symbol": event.decision.symbol,
            "status": status,
            "bybit_order_id": bybit_order_id,
            "error": error,
        });

        sqlx::query(
            "INSERT INTO system_events (event_type, severity, category, data, symbol, pipeline_version, decision_hash)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind("executor_event_processed")
        .bind(if status == "error" { "error" } else { "info" })
        .bind("trade")
        .bind(data)
        .bind(&event.decision.symbol)
        .bind(&event.schema_version)
        .bind(decision_hash(event))
        .execute(pool)
        .await?;
    }

    remember_processed(state, source_event_id).await;
    Ok(())
}

async fn persist_trade(
    state: &ExecutorState,
    event: &StrategyDecisionEvent,
    bybit_order_id: &str,
) -> Result<(), sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(());
    };

    let side = if event.decision.action == "ENTER_LONG" {
        "Long"
    } else {
        "Short"
    };

    let hash = decision_hash(event);

    sqlx::query(
        "INSERT INTO trades (
            order_link_id,
            bybit_order_id,
            symbol,
            side,
            quantity,
            entry_price,
            leverage,
            status,
            decision_hash,
            smart_copy_compatible,
            pipeline_version,
            paper_trade
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,'open',$8,$9,$10,$11)
        ON CONFLICT (order_link_id) DO NOTHING",
    )
    .bind(&event.event_id)
    .bind(bybit_order_id)
    .bind(&event.decision.symbol)
    .bind(side)
    .bind(event.decision.quantity)
    .bind(event.decision.entry_price)
    .bind(event.decision.leverage)
    .bind(hash)
    .bind(event.decision.smart_copy_compatible)
    .bind(&event.schema_version)
    .bind(false)
    .execute(pool)
    .await?;

    Ok(())
}

async fn close_open_trade(
    state: &ExecutorState,
    event: &StrategyDecisionEvent,
) -> Result<CloseReconcileResult, sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(CloseReconcileResult::NoLocalOpen);
    };

    let Some(side) = close_action_to_position_side(&event.decision.action) else {
        return Ok(CloseReconcileResult::NoLocalOpen);
    };

    let open_trade: Option<(String, f64)> = sqlx::query_as(
        "SELECT trade_id::text, quantity::double precision
         FROM trades
         WHERE symbol = $1
           AND side = $2
           AND status = 'open'
         ORDER BY opened_at DESC
         LIMIT 1",
    )
    .bind(&event.decision.symbol)
    .bind(side)
    .fetch_optional(pool)
    .await?;

    let Some((trade_id, open_qty)) = open_trade else {
        return Ok(CloseReconcileResult::NoLocalOpen);
    };

    let close_qty = event.decision.quantity;
    let eps = 1e-9_f64;

    if close_qty + eps < open_qty {
        sqlx::query(
            "UPDATE trades
             SET quantity = quantity - $2,
                 updated_at = NOW()
             WHERE trade_id::text = $1",
        )
        .bind(&trade_id)
        .bind(close_qty)
        .execute(pool)
        .await?;

        return Ok(CloseReconcileResult::Partial {
            trade_id,
            remaining_qty: open_qty - close_qty,
        });
    }

    sqlx::query(
        "UPDATE trades
         SET status = 'closed',
             close_reason = 'manual',
             closed_at = NOW(),
             updated_at = NOW()
         WHERE trade_id::text = $1",
    )
    .bind(&trade_id)
    .execute(pool)
    .await?;

    if close_qty > open_qty + eps {
        return Ok(CloseReconcileResult::CloseQtyExceedsOpen {
            trade_id,
            open_qty,
            close_qty,
        });
    }

    Ok(CloseReconcileResult::Closed { trade_id })
}

async fn submit_market_order(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    event: &StrategyDecisionEvent,
) -> Result<String, Box<dyn Error>> {
    let side = action_to_side(&event.decision.action).ok_or("unsupported action for order")?;

    let close_action = is_close_action(&event.decision.action);

    let body = json!({
        "category": "linear",
        "symbol": event.decision.symbol,
        "side": side,
        "orderType": "Market",
        "qty": format!("{}", event.decision.quantity),
        "orderLinkId": event.event_id,
        "reduceOnly": close_action,
        "closeOnTrigger": close_action,
    });

    let body_str = serde_json::to_string(&body)?;
    let ts = now_ms();
    let sign_payload = format!("{}{}{}{}", ts, cfg.bybit_api_key, cfg.recv_window, body_str);
    let sign = bybit_sign(&cfg.bybit_api_secret, &sign_payload)?;

    let url = format!("{}/v5/order/create", cfg.bybit_base_url());
    let res = http
        .post(url)
        .header("X-BAPI-API-KEY", &cfg.bybit_api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", ts)
        .header("X-BAPI-RECV-WINDOW", &cfg.recv_window)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .await?;

    let status = res.status();
    let value: Value = res.json().await?;

    if !status.is_success() {
        return Err(format!("bybit http={} body={}", status, value).into());
    }

    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("bybit retCode={} retMsg={}", ret_code, ret_msg).into());
    }

    let order_id = value
        .get("result")
        .and_then(|r| r.get("orderId"))
        .and_then(Value::as_str)
        .ok_or("missing result.orderId")?
        .to_string();

    Ok(order_id)
}

async fn handle_decision_event(
    state: &ExecutorState,
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    event: StrategyDecisionEvent,
) -> Result<(), Box<dyn Error>> {
    event
        .validate()
        .map_err(|e| format!("invalid event contract: {e}"))?;

    if already_processed(state, &event.event_id).await? {
        println!("Skipping duplicate decision event_id={}", event.event_id);
        return Ok(());
    }

    if event.decision.action == "HOLD" {
        mark_processed(state, &event.event_id, &event, "ignored_hold", None, None).await?;
        return Ok(());
    }

    if action_to_side(&event.decision.action).is_none() {
        let err = format!("unsupported action {}", event.decision.action);
        mark_processed(state, &event.event_id, &event, "error", None, Some(&err)).await?;
        return Ok(());
    }

    if cfg.live_orders_enabled && !cfg.is_symbol_allowed_live(&event.decision.symbol) {
        println!(
            "Live order blocked by allowlist event_id={} symbol={} allowlist={:?}",
            event.event_id, event.decision.symbol, cfg.live_symbol_allowlist
        );
        mark_processed(
            state,
            &event.event_id,
            &event,
            "blocked_symbol_allowlist",
            None,
            None,
        )
        .await?;
        return Ok(());
    }

    if !cfg.live_orders_enabled {
        println!(
            "Live orders disabled; dry-run for event_id={} action={} symbol={}",
            event.event_id, event.decision.action, event.decision.symbol
        );
        mark_processed(state, &event.event_id, &event, "dry_run", None, None).await?;
        return Ok(());
    }

    if cfg.bybit_api_key.is_empty() || cfg.bybit_api_secret.is_empty() {
        let err = "missing BYBIT_API_KEY/BYBIT_API_SECRET".to_string();
        mark_processed(state, &event.event_id, &event, "error", None, Some(&err)).await?;
        return Ok(());
    }

    match submit_market_order(http, cfg, &event).await {
        Ok(order_id) => {
            println!(
                "Submitted Bybit order event_id={} order_id={} symbol={} action={}",
                event.event_id, order_id, event.decision.symbol, event.decision.action
            );

            let mut status = "submitted";

            if is_close_action(&event.decision.action) {
                match close_open_trade(state, &event).await {
                    Ok(CloseReconcileResult::Closed { trade_id }) => {
                        println!(
                            "Reconciled local close event_id={} trade_id={} symbol={} action={}",
                            event.event_id, trade_id, event.decision.symbol, event.decision.action
                        );
                        status = "submitted_close";
                    }
                    Ok(CloseReconcileResult::Partial {
                        trade_id,
                        remaining_qty,
                    }) => {
                        println!(
                            "Reconciled partial close event_id={} trade_id={} symbol={} remaining_qty={}",
                            event.event_id, trade_id, event.decision.symbol, remaining_qty
                        );
                        status = "submitted_close_partial";
                    }
                    Ok(CloseReconcileResult::CloseQtyExceedsOpen {
                        trade_id,
                        open_qty,
                        close_qty,
                    }) => {
                        eprintln!(
                            "Close qty exceeds local open qty event_id={} trade_id={} symbol={} close_qty={} open_qty={}",
                            event.event_id, trade_id, event.decision.symbol, close_qty, open_qty
                        );
                        status = "submitted_close_qty_exceeds_open";
                    }
                    Ok(CloseReconcileResult::NoLocalOpen) => {
                        eprintln!(
                            "No local open trade to close event_id={} symbol={} action={}",
                            event.event_id, event.decision.symbol, event.decision.action
                        );
                        status = "submitted_close_no_local_open";
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to reconcile local close event_id={} order_id={} err={}",
                            event.event_id, order_id, e
                        );
                        status = "submitted_close_no_persist";
                    }
                }
            } else if let Err(e) = persist_trade(state, &event, &order_id).await {
                eprintln!(
                    "Failed to persist trade for event_id={} order_id={} err={}",
                    event.event_id, order_id, e
                );
                status = "submitted_no_persist";
            }

            mark_processed(
                state,
                &event.event_id,
                &event,
                status,
                Some(&order_id),
                None,
            )
            .await?;
        }
        Err(e) => {
            let err = e.to_string();
            eprintln!(
                "Failed to submit Bybit order event_id={} action={} symbol={} err={}",
                event.event_id, event.decision.action, event.decision.symbol, err
            );
            mark_processed(state, &event.event_id, &event, "error", None, Some(&err)).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-executor");

    let cfg = ExecutorConfig::from_env();
    println!(
        "Executor mode env={} live_orders_enabled={} base_url={} allowlist={:?}",
        cfg.bybit_env,
        cfg.live_orders_enabled,
        cfg.bybit_base_url(),
        cfg.live_symbol_allowlist
    );

    let http = reqwest::Client::new();
    match run_bybit_sanity_checks(&http, &cfg).await {
        Ok(_) => {}
        Err(err) => {
            if cfg.live_orders_enabled {
                return Err(format!(
                    "Bybit sanity checks failed with live orders enabled: {}",
                    err
                )
                .into());
            }
            eprintln!("Bybit sanity checks warning (continuing dry-run): {}", err);
        }
    }

    let db_pool = match PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.db_url)
        .await
    {
        Ok(pool) => {
            println!("Executor database connection: enabled");
            Some(pool)
        }
        Err(err) => {
            eprintln!(
                "Executor database connection unavailable (running with in-memory idempotency only): {}",
                err
            );
            None
        }
    };

    let state = ExecutorState {
        db_pool,
        processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
    };

    let listener = TcpListener::bind("0.0.0.0:8083").await?;
    println!("Health check server running on :8083");

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let mut health_shutdown_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = health_shutdown_rx.changed() => {
                    break;
                }
                accept_result = listener.accept() => {
                    if let Ok((mut socket, _)) = accept_result {
                        tokio::spawn(async move {
                            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                            if let Err(e) = socket.write_all(response.as_bytes()).await {
                                eprintln!("failed to write to socket; err = {:?}", e);
                            }
                        });
                    }
                }
            }
        }
    });

    println!("Connecting to Redis at {}", cfg.redis_url);
    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut ack_conn = redis_client.get_multiplexed_async_connection().await?;
    #[allow(deprecated)]
    let mut pubsub = redis_client.get_async_connection().await?.into_pubsub();
    pubsub.subscribe("viper:decisions").await?;
    println!("Subscribed to viper:decisions");

    let mut messages = pubsub.on_message();

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                println!("Received shutdown signal, stopping viper-executor");
                break;
            }
            maybe_msg = messages.next() => {
                let Some(msg) = maybe_msg else {
                    println!("Decision stream ended, stopping viper-executor");
                    break;
                };

                let payload: String = msg.get_payload()?;

                if let Ok(event) = serde_json::from_str::<StrategyDecisionEvent>(&payload) {
                    if let Err(e) = handle_decision_event(&state, &http, &cfg, event.clone()).await {
                        eprintln!("Executor failed handling event_id={} err={}", event.event_id, e);
                    }

                    let _ = ack_conn.publish::<_, _, ()>("viper:executor_events", payload).await;
                    continue;
                }

                if let Ok(decision) = serde_json::from_str::<StrategyDecision>(&payload) {
                    if let Err(err) = decision.validate() {
                        eprintln!("Executor rejected invalid legacy decision err={}", err);
                        continue;
                    }

                    eprintln!(
                        "Executor received legacy decision without event envelope; ignored action={} symbol={}",
                        decision.action, decision.symbol
                    );
                    continue;
                }

                eprintln!("Executor failed to parse decision payload");
            }
        }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_action_to_side() {
        assert_eq!(action_to_side("ENTER_LONG"), Some("Buy"));
        assert_eq!(action_to_side("ENTER_SHORT"), Some("Sell"));
        assert_eq!(action_to_side("CLOSE_LONG"), Some("Sell"));
        assert_eq!(action_to_side("CLOSE_SHORT"), Some("Buy"));
        assert_eq!(action_to_side("HOLD"), None);
    }

    #[test]
    fn detects_close_actions() {
        assert!(is_close_action("CLOSE_LONG"));
        assert!(is_close_action("CLOSE_SHORT"));
        assert!(!is_close_action("ENTER_LONG"));
        assert!(!is_close_action("HOLD"));
    }

    #[test]
    fn maps_close_action_to_position_side() {
        assert_eq!(close_action_to_position_side("CLOSE_LONG"), Some("Long"));
        assert_eq!(close_action_to_position_side("CLOSE_SHORT"), Some("Short"));
        assert_eq!(close_action_to_position_side("ENTER_LONG"), None);
    }

    #[test]
    fn close_reconcile_result_debug() {
        let result = CloseReconcileResult::Partial {
            trade_id: "t1".to_string(),
            remaining_qty: 2.5,
        };
        let text = format!("{:?}", result);
        assert!(text.contains("Partial"));
    }

    #[test]
    fn signs_payload() {
        let sig = bybit_sign("secret", "payload").expect("must sign");
        assert!(!sig.is_empty());
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn parses_allowlist() {
        let set = parse_allowlist("dogeusdt, xrpusdt ,, TRXUSDT");
        assert!(set.contains("DOGEUSDT"));
        assert!(set.contains("XRPUSDT"));
        assert!(set.contains("TRXUSDT"));
        assert_eq!(set.len(), 3);
    }
}
