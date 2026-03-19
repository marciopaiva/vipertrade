use futures_util::StreamExt;
use hmac::{Hmac, Mac};
use redis::AsyncCommands;
use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};
use serde_yaml::Value as YamlValue;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::{watch, Mutex};
use viper_domain::{StrategyDecision, StrategyDecisionEvent};

type HmacSha256 = Hmac<Sha256>;
const CONSTRAINTS_CACHE_TTL_SECS: u64 = 60;

#[derive(Debug, Clone)]
struct ExecutorConfig {
    redis_url: String,
    db_url: String,
    trading_mode: TradingMode,
    bybit_env: String,
    bybit_api_key: String,
    bybit_api_secret: String,
    recv_window: String,
    bybit_account_type: String,
    executor_default_enabled: bool,
    live_orders_enabled: bool,
    live_symbol_allowlist: HashSet<String>,
    reconcile_fix: bool,
    paper_max_open_positions: i64,
    strategy_config_path: String,
    trading_profile: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TradingMode {
    Paper,
    Testnet,
    Mainnet,
}

impl TradingMode {
    fn from_env() -> Self {
        match std::env::var("TRADING_MODE")
            .unwrap_or_else(|_| "paper".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "testnet" => Self::Testnet,
            "mainnet" | "live" => Self::Mainnet,
            _ => Self::Paper,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "PAPER",
            Self::Testnet => "TESTNET",
            Self::Mainnet => "MAINNET",
        }
    }

    fn bybit_env(self) -> &'static str {
        match self {
            Self::Testnet => "testnet",
            Self::Paper | Self::Mainnet => "mainnet",
        }
    }

    fn executes_exchange_orders(self) -> bool {
        !matches!(self, Self::Paper)
    }
}

#[derive(Clone)]
struct ExecutorState {
    db_pool: Option<PgPool>,
    processed_in_memory: Arc<Mutex<HashSet<String>>>,
    constraints_cache: Arc<Mutex<HashMap<String, (Instant, BybitSymbolConstraints)>>>,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeControls {
    executor_enabled: bool,
    kill_switch_enabled: bool,
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

        let trading_mode = TradingMode::from_env();
        let bybit_env = trading_mode.bybit_env().to_string();
        let (bybit_api_key, bybit_api_secret) = resolve_bybit_credentials();
        let recv_window = std::env::var("BYBIT_RECV_WINDOW").unwrap_or_else(|_| "5000".to_string());
        let bybit_account_type =
            std::env::var("BYBIT_ACCOUNT_TYPE").unwrap_or_else(|_| "UNIFIED".to_string());
        let executor_default_enabled = std::env::var("EXECUTOR_DEFAULT_ENABLED")
            .map(|v| !matches!(v.as_str(), "0" | "false" | "FALSE" | "no" | "NO"))
            .unwrap_or(true);
        let live_orders_enabled = trading_mode.executes_exchange_orders();
        let live_symbol_allowlist = parse_allowlist(
            std::env::var("EXECUTOR_LIVE_SYMBOL_ALLOWLIST")
                .unwrap_or_else(|_| "DOGEUSDT".to_string())
                .as_str(),
        );
        let reconcile_fix = std::env::var("EXECUTOR_RECONCILE_FIX")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let paper_max_open_positions = std::env::var("EXECUTOR_PAPER_MAX_OPEN_POSITIONS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(2);
        let strategy_config_path = std::env::var("STRATEGY_CONFIG")
            .unwrap_or_else(|_| "config/trading/pairs.yaml".to_string());
        let trading_profile =
            std::env::var("TRADING_PROFILE").unwrap_or_else(|_| "MEDIUM".to_string());

        Self {
            redis_url,
            db_url,
            trading_mode,
            bybit_env,
            bybit_api_key,
            bybit_api_secret,
            recv_window,
            bybit_account_type,
            executor_default_enabled,
            live_orders_enabled,
            live_symbol_allowlist,
            reconcile_fix,
            paper_max_open_positions,
            strategy_config_path,
            trading_profile,
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

async fn connect_executor_db(cfg: &ExecutorConfig) -> Result<Option<PgPool>, Box<dyn Error>> {
    let attempts = if matches!(cfg.trading_mode, TradingMode::Paper) {
        10
    } else {
        5
    };
    let retry_delay = Duration::from_secs(2);
    let mut last_err: Option<sqlx::Error> = None;

    for attempt in 1..=attempts {
        match PgPoolOptions::new()
            .max_connections(5)
            .connect(&cfg.db_url)
            .await
        {
            Ok(pool) => {
                println!("Executor database connection: enabled");
                return Ok(Some(pool));
            }
            Err(err) => {
                eprintln!(
                    "Executor database connection attempt {}/{} failed: {}",
                    attempt, attempts, err
                );
                last_err = Some(err);
                if attempt < attempts {
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
    }

    if matches!(cfg.trading_mode, TradingMode::Paper) {
        let err = last_err
            .map(|err| err.to_string())
            .unwrap_or_else(|| "unknown database connection error".to_string());
        return Err(format!(
            "Executor requires Postgres in PAPER mode but could not connect after {} attempts: {}",
            attempts, err
        )
        .into());
    }

    if let Some(err) = last_err {
        eprintln!(
            "Executor database connection unavailable (running with in-memory idempotency only): {}",
            err
        );
    }

    Ok(None)
}

fn parse_allowlist(raw: &str) -> HashSet<String> {
    raw.split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn resolve_bybit_credentials() -> (String, String) {
    let from = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
    let scoped = match TradingMode::from_env() {
        TradingMode::Testnet => (
            from("BYBIT_TESTNET_API_KEY"),
            from("BYBIT_TESTNET_API_SECRET"),
        ),
        TradingMode::Paper | TradingMode::Mainnet => (
            from("BYBIT_MAINNET_API_KEY"),
            from("BYBIT_MAINNET_API_SECRET"),
        ),
    };

    (
        scoped
            .0
            .or_else(|| from("BYBIT_API_KEY"))
            .unwrap_or_default(),
        scoped
            .1
            .or_else(|| from("BYBIT_API_SECRET"))
            .unwrap_or_default(),
    )
}

fn yaml_get<'a>(value: &'a YamlValue, path: &[&str]) -> Option<&'a YamlValue> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn yaml_f64(value: &YamlValue, path: &[&str]) -> Option<f64> {
    yaml_get(value, path).and_then(|v| match v {
        YamlValue::Number(n) => n.as_f64(),
        YamlValue::String(s) => s.parse::<f64>().ok(),
        _ => None,
    })
}

fn yaml_bool(value: &YamlValue, path: &[&str]) -> Option<bool> {
    yaml_get(value, path).and_then(|v| match v {
        YamlValue::Bool(b) => Some(*b),
        YamlValue::String(s) => match s.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

fn load_native_trailing_config(cfg: &ExecutorConfig, symbol: &str) -> Option<NativeTrailingConfig> {
    let raw = std::fs::read_to_string(&cfg.strategy_config_path).ok()?;
    let root: YamlValue = serde_yaml::from_str(&raw).ok()?;
    let mode_key = cfg.trading_mode.as_str();
    let symbol_key = symbol.to_uppercase();
    let profile_key = cfg.trading_profile.to_ascii_uppercase();

    let mode_cfg = yaml_get(&root, &["global", "mode_profiles", mode_key]);
    let pair_mode_cfg = yaml_get(&root, &[&symbol_key, "mode_profiles", mode_key]);
    let pair_profile_cfg = yaml_get(
        &root,
        &[&symbol_key, "trailing_stop", "by_profile", &profile_key],
    );

    let enabled = pair_mode_cfg
        .and_then(|v| yaml_bool(v, &["trailing_enabled"]))
        .or_else(|| mode_cfg.and_then(|v| yaml_bool(v, &["trailing_enabled"])))
        .or_else(|| pair_mode_cfg.and_then(|v| yaml_bool(v, &["trailing_stop", "enabled"])))
        .or_else(|| pair_profile_cfg.and_then(|v| yaml_bool(v, &["enabled"])))
        .unwrap_or(false);

    if !enabled {
        return None;
    }

    let activate_after_profit_pct = pair_mode_cfg
        .and_then(|v| yaml_f64(v, &["trailing_stop", "activate_after_profit_pct"]))
        .or_else(|| {
            mode_cfg.and_then(|v| yaml_f64(v, &["trailing_stop", "activate_after_profit_pct"]))
        })
        .or_else(|| pair_profile_cfg.and_then(|v| yaml_f64(v, &["activate_after_profit_pct"])))?;

    let initial_trail_pct = pair_mode_cfg
        .and_then(|v| yaml_f64(v, &["trailing_stop", "initial_trail_pct"]))
        .or_else(|| mode_cfg.and_then(|v| yaml_f64(v, &["trailing_stop", "initial_trail_pct"])))
        .or_else(|| pair_profile_cfg.and_then(|v| yaml_f64(v, &["initial_trail_pct"])))?;

    Some(NativeTrailingConfig {
        enabled,
        activate_after_profit_pct,
        initial_trail_pct,
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

fn close_reason_from_decision(reason: &str) -> String {
    let normalized = reason.to_ascii_lowercase();
    if normalized.contains("trailing_stop") {
        "trailing_stop".to_string()
    } else if normalized.contains("stop_loss") {
        "stop_loss".to_string()
    } else if normalized.contains("take_profit") {
        "take_profit".to_string()
    } else if normalized.contains("time_exit") {
        "time_exit".to_string()
    } else if normalized.contains("circuit_breaker") {
        "circuit_breaker".to_string()
    } else if normalized.contains("thesis_invalidated") {
        "thesis_invalidated".to_string()
    } else if normalized.trim().is_empty() {
        "manual".to_string()
    } else {
        normalized
    }
}

#[derive(Debug)]
enum CloseReconcileResult {
    NoLocalOpen,
    Partial {
        trade_id: String,
        remaining_qty: f64,
        realized_pnl: f64,
    },
    Closed {
        trade_id: String,
        realized_pnl: f64,
    },
    CloseQtyExceedsOpen {
        trade_id: String,
        open_qty: f64,
        close_qty: f64,
        realized_pnl: f64,
    },
}

#[derive(Debug, Clone, Copy)]
struct BybitSymbolConstraints {
    min_order_qty: f64,
    qty_step: f64,
    min_notional: Option<f64>,
    tick_size: f64,
}

#[derive(Debug, Clone, Copy)]
struct NativeTrailingConfig {
    enabled: bool,
    activate_after_profit_pct: f64,
    initial_trail_pct: f64,
}

#[derive(Debug, Clone)]
struct OrderExecutionMeta {
    avg_price: Option<f64>,
    fee: Option<f64>,
    executed_qty: Option<f64>,
    fills: Vec<BybitFill>,
}

#[derive(Debug, Clone)]
struct BybitFill {
    execution_id: String,
    order_id: String,
    side: Option<String>,
    exec_qty: f64,
    exec_price: Option<f64>,
    exec_fee: f64,
    fee_currency: Option<String>,
    is_maker: Option<bool>,
    exec_time_ms: Option<i64>,
    raw_data: Value,
}

fn realized_pnl(
    side: &str,
    entry_price: f64,
    exit_price: f64,
    quantity: f64,
    leverage: f64,
) -> f64 {
    let signed_delta = if side == "Long" {
        exit_price - entry_price
    } else {
        entry_price - exit_price
    };

    signed_delta * quantity * leverage
}

fn parse_positive_f64(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_str)
        .and_then(|x| x.parse::<f64>().ok())
        .filter(|x| *x > 0.0)
}

fn format_order_qty(qty: f64, qty_step: f64) -> String {
    let precision = qty_step_precision(qty_step);

    if precision == 0 {
        format!("{:.0}", qty)
    } else {
        let raw = format!("{qty:.precision$}");
        raw.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn format_price_value(price: f64, tick_size: f64) -> String {
    let precision = qty_step_precision(tick_size);
    if precision == 0 {
        format!("{:.0}", price)
    } else {
        let raw = format!("{price:.precision$}");
        raw.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn snap_price_to_tick(price: f64, tick_size: f64) -> f64 {
    if tick_size <= 0.0 {
        return price;
    }
    let precision = qty_step_precision(tick_size);
    let steps = (price / tick_size).round();
    round_with_precision(steps * tick_size, precision)
}

fn qty_step_precision(qty_step: f64) -> usize {
    if qty_step >= 1.0 {
        0
    } else {
        let step_repr = format!("{:.12}", qty_step);
        step_repr
            .trim_end_matches('0')
            .split('.')
            .nth(1)
            .map(|d| d.len())
            .unwrap_or(0)
    }
}

fn round_with_precision(value: f64, precision: usize) -> f64 {
    if precision == 0 {
        value.round()
    } else {
        let factor = 10_f64.powi(precision as i32);
        (value * factor).round() / factor
    }
}

fn normalize_order_quantity(qty: f64, c: BybitSymbolConstraints) -> Result<f64, String> {
    if qty <= 0.0 {
        return Err("quantity must be > 0".to_string());
    }

    let eps = 1e-8_f64;
    let mut normalized = qty;
    let precision = qty_step_precision(c.qty_step).max(qty_step_precision(c.min_order_qty));

    if c.qty_step > 0.0 {
        let raw_steps = qty / c.qty_step;
        let rounded_steps = raw_steps.round();
        let snapped_steps = if (raw_steps - rounded_steps).abs() <= eps {
            rounded_steps
        } else {
            raw_steps.floor()
        };
        normalized = snapped_steps.max(0.0) * c.qty_step;
    }
    normalized = round_with_precision(normalized, precision);

    if normalized + eps < c.min_order_qty {
        let min_order_qty = round_with_precision(c.min_order_qty, precision);
        if qty + eps >= min_order_qty {
            return Ok(min_order_qty);
        }
        return Err(format!(
            "quantity {} below minOrderQty {} after qtyStep normalization",
            normalized, c.min_order_qty
        ));
    }

    Ok(normalized)
}

fn ensure_min_notional(
    action: &str,
    qty: f64,
    decision_price: f64,
    c: BybitSymbolConstraints,
) -> Result<(), String> {
    if is_close_action(action) {
        return Ok(());
    }

    let Some(min_notional) = c.min_notional else {
        return Ok(());
    };

    if decision_price <= 0.0 {
        return Err("decision entry_price must be > 0 for min-notional validation".to_string());
    }

    let notional = qty * decision_price;
    if notional + 1e-9 < min_notional {
        return Err(format!(
            "order notional {} below minNotionalValue {}",
            notional, min_notional
        ));
    }

    Ok(())
}

async fn fetch_symbol_constraints(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
) -> Result<BybitSymbolConstraints, Box<dyn Error>> {
    let path = format!(
        "/v5/market/instruments-info?category=linear&symbol={}",
        symbol.to_uppercase()
    );
    let value = bybit_public_get(http, cfg, &path).await?;

    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("bybit retCode={} retMsg={}", ret_code, ret_msg).into());
    }

    let instrument = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .ok_or("missing instrument metadata")?;

    let lot = instrument
        .get("lotSizeFilter")
        .ok_or("missing lotSizeFilter")?;
    let price_filter = instrument.get("priceFilter").ok_or("missing priceFilter")?;

    let min_order_qty = parse_positive_f64(lot.get("minOrderQty")).ok_or("missing minOrderQty")?;
    let qty_step = parse_positive_f64(lot.get("qtyStep")).ok_or("missing qtyStep")?;
    let min_notional = parse_positive_f64(lot.get("minNotionalValue"));
    let tick_size = parse_positive_f64(price_filter.get("tickSize")).ok_or("missing tickSize")?;

    Ok(BybitSymbolConstraints {
        min_order_qty,
        qty_step,
        min_notional,
        tick_size,
    })
}

async fn get_symbol_constraints(
    state: &ExecutorState,
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
) -> Result<BybitSymbolConstraints, Box<dyn Error>> {
    let key = symbol.to_uppercase();

    {
        let cache = state.constraints_cache.lock().await;
        if let Some((cached_at, constraints)) = cache.get(&key) {
            if cached_at.elapsed() < Duration::from_secs(CONSTRAINTS_CACHE_TTL_SECS) {
                return Ok(*constraints);
            }
        }
    }

    let fetched = fetch_symbol_constraints(http, cfg, symbol).await?;
    let mut cache = state.constraints_cache.lock().await;
    cache.insert(key, (Instant::now(), fetched));
    Ok(fetched)
}

fn idempotency_key(event: &StrategyDecisionEvent) -> &str {
    if event.source_event_id.trim().is_empty() {
        &event.event_id
    } else {
        &event.source_event_id
    }
}

fn decision_hash(event: &StrategyDecisionEvent) -> String {
    let mut hasher = Sha256::new();
    let payload = serde_json::to_vec(event).unwrap_or_default();
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

fn body_preview(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    let max = 280usize;
    if trimmed.len() <= max {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..max])
    }
}

async fn parse_bybit_json_response(
    res: reqwest::Response,
    context: &str,
) -> Result<Value, Box<dyn Error>> {
    let status = res.status();
    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();
    let body = res.text().await?;
    let preview = body_preview(&body);

    if body.trim().is_empty() {
        return Err(format!(
            "{} empty body http={} content_type={}",
            context, status, content_type
        )
        .into());
    }

    let value: Value = serde_json::from_str(&body).map_err(|e| {
        format!(
            "{} invalid json http={} content_type={} err={} body_preview={}",
            context, status, content_type, e, preview
        )
    })?;

    if !status.is_success() {
        return Err(format!(
            "{} http={} content_type={} body={}",
            context, status, content_type, value
        )
        .into());
    }

    Ok(value)
}

async fn bybit_public_get(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    path: &str,
) -> Result<Value, Box<dyn Error>> {
    let url = format!("{}{}", cfg.bybit_base_url(), path);
    let res = http.get(url).send().await?;
    parse_bybit_json_response(res, "bybit public").await
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

    parse_bybit_json_response(res, "bybit private").await
}

async fn bybit_private_post(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    path: &str,
    body: &Value,
) -> Result<Value, Box<dyn Error>> {
    let body_str = serde_json::to_string(body)?;
    let ts = now_ms();
    let sign_payload = format!("{}{}{}{}", ts, cfg.bybit_api_key, cfg.recv_window, body_str);
    let sign = bybit_sign(&cfg.bybit_api_secret, &sign_payload)?;

    let url = format!("{}{}", cfg.bybit_base_url(), path);
    let res = http
        .post(url)
        .header("X-BAPI-API-KEY", &cfg.bybit_api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", ts)
        .header("X-BAPI-RECV-WINDOW", &cfg.recv_window)
        .header(CONTENT_TYPE, "application/json")
        .body(body_str)
        .send()
        .await?;

    parse_bybit_json_response(res, "bybit private").await
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

    if matches!(cfg.trading_mode, TradingMode::Paper) {
        println!("Bybit sanity check: wallet skipped (paper mode uses database simulation)");
        return Ok(());
    }

    if cfg.bybit_api_key.is_empty() || cfg.bybit_api_secret.is_empty() {
        if cfg.live_orders_enabled {
            return Err("live orders enabled but BYBIT_API_KEY/SECRET missing".to_string());
        }
        println!("Bybit sanity check: wallet skipped (no API credentials)");
        return Ok(());
    }

    let mut candidates = vec![cfg.bybit_account_type.to_uppercase()];
    for fallback in ["UNIFIED", "CONTRACT", "SPOT"] {
        if !candidates.iter().any(|v| v == fallback) {
            candidates.push(fallback.to_string());
        }
    }

    let mut wallet_errors: Vec<String> = Vec::new();
    let mut wallet_ok_account_type: Option<String> = None;

    for account_type in candidates {
        let query = format!("accountType={account_type}");
        let wallet_value =
            match bybit_private_get(http, cfg, "/v5/account/wallet-balance", &query).await {
                Ok(v) => v,
                Err(e) => {
                    wallet_errors.push(format!("accountType={account_type} request_error={e}"));
                    continue;
                }
            };

        let wallet_ret = wallet_value
            .get("retCode")
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        if wallet_ret == 0 {
            wallet_ok_account_type = Some(account_type.clone());
            break;
        }

        wallet_errors.push(format!(
            "accountType={} retCode={} body={}",
            account_type, wallet_ret, wallet_value
        ));
    }

    if let Some(ok_account_type) = wallet_ok_account_type {
        if ok_account_type != cfg.bybit_account_type.to_uppercase() {
            eprintln!(
                "Bybit sanity check: wallet-balance OK with fallback accountType={} (configured={})",
                ok_account_type, cfg.bybit_account_type
            );
        } else {
            println!(
                "Bybit sanity check: wallet-balance OK (accountType={})",
                cfg.bybit_account_type
            );
        }
    } else {
        return Err(format!(
            "wallet-balance failed for all accountType candidates: {}",
            wallet_errors.join(" | ")
        ));
    }

    Ok(())
}

async fn remember_processed(state: &ExecutorState, source_event_id: &str) {
    let mut seen = state.processed_in_memory.lock().await;
    seen.insert(source_event_id.to_string());
}

async fn claim_processed_event(
    state: &ExecutorState,
    source_event_id: &str,
    event: &StrategyDecisionEvent,
) -> Result<bool, sqlx::Error> {
    if let Some(pool) = &state.db_pool {
        let data = json!({
            "source_event_id": source_event_id,
            "decision_event_id": event.event_id,
            "action": event.decision.action,
            "symbol": event.decision.symbol,
            "status": "claimed",
            "bybit_order_id": null,
            "error": null,
        });

        let result = sqlx::query(
            "INSERT INTO system_events (event_type, severity, category, data, symbol, pipeline_version, decision_hash)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT DO NOTHING",
        )
        .bind("executor_event_processed")
        .bind("info")
        .bind("trade")
        .bind(data)
        .bind(&event.decision.symbol)
        .bind(&event.schema_version)
        .bind(decision_hash(event))
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(false);
        }

        remember_processed(state, source_event_id).await;
        return Ok(true);
    }

    let mut seen = state.processed_in_memory.lock().await;
    Ok(seen.insert(source_event_id.to_string()))
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
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (event_type, (data->>'source_event_id'))
             WHERE event_type = 'executor_event_processed'
               AND COALESCE(data->>'source_event_id', '') <> ''
             DO UPDATE SET
                severity = EXCLUDED.severity,
                category = EXCLUDED.category,
                data = EXCLUDED.data,
                symbol = EXCLUDED.symbol,
                pipeline_version = EXCLUDED.pipeline_version,
                decision_hash = EXCLUDED.decision_hash,
                timestamp = NOW()",
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

async fn fetch_latest_control_flag(
    pool: &PgPool,
    event_type: &str,
    default_enabled: bool,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_scalar::<_, Option<bool>>(
        "SELECT (data->>'enabled')::boolean
         FROM system_events
         WHERE event_type = $1
         ORDER BY timestamp DESC
         LIMIT 1",
    )
    .bind(event_type)
    .fetch_optional(pool)
    .await?;

    Ok(row.flatten().unwrap_or(default_enabled))
}

async fn fetch_runtime_controls(
    state: &ExecutorState,
    cfg: &ExecutorConfig,
) -> Result<RuntimeControls, sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(RuntimeControls {
            executor_enabled: true,
            kill_switch_enabled: false,
        });
    };

    let executor_enabled =
        fetch_latest_control_flag(pool, "api_executor_state_set", cfg.executor_default_enabled)
            .await?;
    let kill_switch_enabled = fetch_latest_control_flag(pool, "api_kill_switch_set", false).await?;

    Ok(RuntimeControls {
        executor_enabled,
        kill_switch_enabled,
    })
}

async fn persist_trade(
    state: &ExecutorState,
    event: &StrategyDecisionEvent,
    bybit_order_id: &str,
    entry_qty: f64,
    entry_price: f64,
    fees: f64,
    paper_trade: bool,
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
            fees,
            leverage,
            status,
            decision_hash,
            smart_copy_compatible,
            pipeline_version,
            paper_trade,
            trailing_stop_activated,
            trailing_stop_peak_price,
            trailing_stop_final_distance_pct
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'open',$9,$10,$11,$12,$13,$14,$15)
        ON CONFLICT (order_link_id) DO NOTHING",
    )
    .bind(&event.event_id)
    .bind(bybit_order_id)
    .bind(&event.decision.symbol)
    .bind(side)
    .bind(entry_qty)
    .bind(entry_price)
    .bind(fees)
    .bind(event.decision.leverage)
    .bind(hash)
    .bind(event.decision.smart_copy_compatible)
    .bind(&event.schema_version)
    .bind(paper_trade)
    .bind(false)
    .bind(entry_price)
    .bind(0.0_f64)
    .execute(pool)
    .await?;

    Ok(())
}

async fn count_open_trades(state: &ExecutorState) -> Result<i64, sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(0);
    };

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM trades WHERE status = 'open'")
            .fetch_one(pool)
            .await?;

    Ok(count)
}

async fn has_open_trade_for_symbol_side(
    state: &ExecutorState,
    symbol: &str,
    side: &str,
) -> Result<bool, sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(false);
    };

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM trades WHERE status = 'open' AND symbol = $1 AND side = $2",
    )
    .bind(symbol)
    .bind(side)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

async fn persist_bybit_fills(
    state: &ExecutorState,
    event: &StrategyDecisionEvent,
    bybit_order_id: &str,
    fills: &[BybitFill],
) -> Result<(), sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(());
    };

    for fill in fills {
        sqlx::query(
            "INSERT INTO bybit_fills (
                bybit_execution_id,
                bybit_order_id,
                order_link_id,
                symbol,
                side,
                exec_qty,
                exec_price,
                exec_fee,
                fee_currency,
                is_maker,
                exec_time,
                raw_data
            ) VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,
                CASE WHEN $11 IS NULL THEN NULL ELSE to_timestamp(($11::double precision)/1000.0) END,
                $12
            )
            ON CONFLICT (bybit_execution_id) DO NOTHING",
        )
        .bind(&fill.execution_id)
        .bind(if fill.order_id.is_empty() { bybit_order_id } else { &fill.order_id })
        .bind(&event.event_id)
        .bind(&event.decision.symbol)
        .bind(fill.side.as_deref())
        .bind(fill.exec_qty)
        .bind(fill.exec_price)
        .bind(fill.exec_fee)
        .bind(fill.fee_currency.as_deref())
        .bind(fill.is_maker)
        .bind(fill.exec_time_ms)
        .bind(&fill.raw_data)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn close_open_trade(
    state: &ExecutorState,
    event: &StrategyDecisionEvent,
    close_qty: f64,
    close_price: f64,
    close_fee: f64,
) -> Result<CloseReconcileResult, sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(CloseReconcileResult::NoLocalOpen);
    };

    let Some(side) = close_action_to_position_side(&event.decision.action) else {
        return Ok(CloseReconcileResult::NoLocalOpen);
    };

    let open_trade: Option<(String, f64, f64, f64)> = sqlx::query_as(
        "SELECT trade_id::text,
                quantity::double precision,
                entry_price::double precision,
                leverage::double precision
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

    let Some((trade_id, open_qty, entry_price, leverage)) = open_trade else {
        return Ok(CloseReconcileResult::NoLocalOpen);
    };

    let eps = 1e-9_f64;
    let effective_close_qty = if close_qty > open_qty {
        open_qty
    } else {
        close_qty
    };
    let pnl_delta = realized_pnl(
        side,
        entry_price,
        close_price,
        effective_close_qty,
        leverage,
    );
    let close_reason = close_reason_from_decision(&event.decision.reason);

    if close_qty + eps < open_qty {
        sqlx::query(
            "UPDATE trades
             SET quantity = quantity - $2,
                 pnl = COALESCE(pnl, 0) + $3,
                 fees = COALESCE(fees, 0) + $4,
                 exit_price = $5,
                 updated_at = NOW()
             WHERE trade_id::text = $1",
        )
        .bind(&trade_id)
        .bind(close_qty)
        .bind(pnl_delta)
        .bind(close_fee)
        .bind(close_price)
        .execute(pool)
        .await?;

        return Ok(CloseReconcileResult::Partial {
            trade_id,
            remaining_qty: open_qty - close_qty,
            realized_pnl: pnl_delta,
        });
    }

    sqlx::query(
        "UPDATE trades
         SET status = 'closed',
             close_reason = $5,
             closed_at = NOW(),
             pnl = COALESCE(pnl, 0) + $2,
             fees = COALESCE(fees, 0) + $3,
             pnl_pct = CASE
                 WHEN entry_price > 0 THEN (((COALESCE(pnl, 0) + $2 - COALESCE(fees, 0) - $3) / (entry_price * quantity)) * 100)
                 ELSE NULL
             END,
             exit_price = $4,
             updated_at = NOW()
         WHERE trade_id::text = $1",
    )
    .bind(&trade_id)
    .bind(pnl_delta)
    .bind(close_fee)
    .bind(close_price)
    .bind(close_reason)
    .execute(pool)
    .await?;

    if close_qty > open_qty + eps {
        return Ok(CloseReconcileResult::CloseQtyExceedsOpen {
            trade_id,
            open_qty,
            close_qty,
            realized_pnl: pnl_delta,
        });
    }

    Ok(CloseReconcileResult::Closed {
        trade_id,
        realized_pnl: pnl_delta,
    })
}

async fn submit_market_order(
    state: &ExecutorState,
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    event: &StrategyDecisionEvent,
) -> Result<String, Box<dyn Error>> {
    let side = action_to_side(&event.decision.action).ok_or("unsupported action for order")?;

    let close_action = is_close_action(&event.decision.action);
    let constraints = get_symbol_constraints(state, http, cfg, &event.decision.symbol)
        .await
        .map_err(|e| {
            format!(
                "symbol constraints unavailable for {} (live-safe block): {}",
                event.decision.symbol, e
            )
        })?;
    let normalized_qty = normalize_order_quantity(event.decision.quantity, constraints)
        .map_err(|e| format!("quantity validation failed: {e}"))?;

    ensure_min_notional(
        &event.decision.action,
        normalized_qty,
        event.decision.entry_price,
        constraints,
    )
    .map_err(|e| format!("notional validation failed: {e}"))?;

    if (normalized_qty - event.decision.quantity).abs() > 1e-9 {
        println!(
            "Adjusted order quantity event_id={} symbol={} action={} original_qty={} normalized_qty={}",
            event.event_id,
            event.decision.symbol,
            event.decision.action,
            event.decision.quantity,
            normalized_qty
        );
    }

    let qty_str = format_order_qty(normalized_qty, constraints.qty_step);

    let body = json!({
        "category": "linear",
        "symbol": event.decision.symbol,
        "side": side,
        "orderType": "Market",
        "qty": qty_str,
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

async fn fetch_order_execution_price(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
    order_id: &str,
) -> Result<Option<f64>, Box<dyn Error>> {
    let query = format!(
        "category=linear&symbol={}&orderId={}",
        symbol.to_uppercase(),
        order_id
    );

    let value = bybit_private_get(http, cfg, "/v5/order/realtime", &query).await?;
    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("bybit retCode={} retMsg={}", ret_code, ret_msg).into());
    }

    let maybe_avg = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .and_then(|order| order.get("avgPrice"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|p| *p > 0.0);

    Ok(maybe_avg)
}

async fn fetch_order_execution_fills(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
    order_id: &str,
) -> Result<Vec<BybitFill>, Box<dyn Error>> {
    let query = format!(
        "category=linear&symbol={}&orderId={}",
        symbol.to_uppercase(),
        order_id
    );

    let value = bybit_private_get(http, cfg, "/v5/execution/list", &query).await?;
    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("bybit retCode={} retMsg={}", ret_code, ret_msg).into());
    }

    let fills = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|fill| {
                    let execution_id = fill
                        .get("execId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if execution_id.is_empty() {
                        return None;
                    }

                    let order_id = fill
                        .get("orderId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();

                    let exec_qty = fill
                        .get("execQty")
                        .and_then(Value::as_str)
                        .and_then(|x| x.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    if exec_qty <= 0.0 {
                        return None;
                    }

                    let exec_price = fill
                        .get("execPrice")
                        .and_then(Value::as_str)
                        .and_then(|x| x.parse::<f64>().ok())
                        .filter(|v| *v > 0.0);

                    let exec_fee = fill
                        .get("execFee")
                        .and_then(Value::as_str)
                        .and_then(|x| x.parse::<f64>().ok())
                        .unwrap_or(0.0);

                    let fee_currency = fill
                        .get("feeCurrency")
                        .and_then(Value::as_str)
                        .map(str::to_string);

                    let side = fill.get("side").and_then(Value::as_str).map(str::to_string);
                    let is_maker = fill.get("isMaker").and_then(Value::as_bool);
                    let exec_time_ms = fill
                        .get("execTime")
                        .and_then(Value::as_str)
                        .and_then(|x| x.parse::<i64>().ok());

                    Some(BybitFill {
                        execution_id,
                        order_id,
                        side,
                        exec_qty,
                        exec_price,
                        exec_fee,
                        fee_currency,
                        is_maker,
                        exec_time_ms,
                        raw_data: fill.clone(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(fills)
}

async fn fetch_order_execution_meta(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
    order_id: &str,
) -> Result<OrderExecutionMeta, Box<dyn Error>> {
    let avg_price_from_order = fetch_order_execution_price(http, cfg, symbol, order_id).await?;
    let fills = match fetch_order_execution_fills(http, cfg, symbol, order_id).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Failed to fetch Bybit fills symbol={} order_id={} err={}",
                symbol, order_id, e
            );
            Vec::new()
        }
    };

    let total_fee: f64 = fills.iter().map(|f| f.exec_fee).sum();
    let total_qty: f64 = fills.iter().map(|f| f.exec_qty).sum();
    let weighted_notional: f64 = fills
        .iter()
        .filter_map(|f| f.exec_price.map(|p| p * f.exec_qty))
        .sum();

    let avg_price_from_fills = if total_qty > 0.0 && weighted_notional > 0.0 {
        Some(weighted_notional / total_qty)
    } else {
        None
    };

    Ok(OrderExecutionMeta {
        avg_price: avg_price_from_order.or(avg_price_from_fills),
        fee: if total_fee.abs() < 1e-12 {
            None
        } else {
            Some(total_fee)
        },
        executed_qty: if total_qty.abs() < 1e-12 {
            None
        } else {
            Some(total_qty)
        },
        fills,
    })
}

async fn fetch_bybit_position_qty(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
    side: &str,
) -> Result<f64, Box<dyn Error>> {
    let query = format!("category=linear&symbol={}", symbol.to_uppercase());
    let value = bybit_private_get(http, cfg, "/v5/position/list", &query).await?;

    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("bybit retCode={} retMsg={}", ret_code, ret_msg).into());
    }

    let bybit_side = if side == "Long" { "Buy" } else { "Sell" };
    let qty = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter(|pos| pos.get("side").and_then(Value::as_str) == Some(bybit_side))
                .filter_map(|pos| pos.get("size"))
                .filter_map(Value::as_str)
                .filter_map(|x| x.parse::<f64>().ok())
                .fold(0.0, |acc, v| acc + v)
        })
        .unwrap_or(0.0);

    Ok(qty)
}

async fn fetch_bybit_last_price(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
) -> Result<f64, Box<dyn Error>> {
    let path = format!(
        "/v5/market/tickers?category=linear&symbol={}",
        symbol.to_uppercase()
    );
    let value = bybit_public_get(http, cfg, &path).await?;

    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("bybit retCode={} retMsg={}", ret_code, ret_msg).into());
    }

    let price = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .and_then(|ticker| ticker.get("lastPrice"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|p| *p > 0.0)
        .ok_or("missing result.list[0].lastPrice")?;

    Ok(price)
}

async fn set_bybit_trailing_stop(
    state: &ExecutorState,
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    event: &StrategyDecisionEvent,
    entry_price: f64,
) -> Result<(), Box<dyn Error>> {
    if !matches!(event.decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
        return Ok(());
    }

    let Some(native_cfg) = load_native_trailing_config(cfg, &event.decision.symbol) else {
        return Ok(());
    };

    if !native_cfg.enabled || entry_price <= 0.0 {
        return Ok(());
    }

    let constraints = get_symbol_constraints(state, http, cfg, &event.decision.symbol).await?;
    let is_long = event.decision.action == "ENTER_LONG";
    let trailing_distance_raw = entry_price * native_cfg.initial_trail_pct;
    let trailing_distance = snap_price_to_tick(trailing_distance_raw, constraints.tick_size);

    if trailing_distance <= 0.0 {
        return Err("computed trailing distance is not positive".into());
    }

    let mut last_error: Option<String> = None;

    for attempt in 1..=4 {
        let last_price = fetch_bybit_last_price(http, cfg, &event.decision.symbol)
            .await
            .unwrap_or(entry_price);
        let active_price_target = if is_long {
            (entry_price * (1.0 + native_cfg.activate_after_profit_pct))
                .max(last_price + constraints.tick_size)
        } else {
            (entry_price * (1.0 - native_cfg.activate_after_profit_pct))
                .min((last_price - constraints.tick_size).max(constraints.tick_size))
        };
        let active_price = snap_price_to_tick(active_price_target, constraints.tick_size);

        let body = json!({
            "category": "linear",
            "symbol": event.decision.symbol,
            "tpslMode": "Full",
            "positionIdx": 0,
            "activePrice": format_price_value(active_price, constraints.tick_size),
            "trailingStop": format_price_value(trailing_distance, constraints.tick_size),
        });

        let value = bybit_private_post(http, cfg, "/v5/position/trading-stop", &body).await?;
        let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
        if ret_code == 0 {
            println!(
                "Configured Bybit trailing stop event_id={} symbol={} active_price={} trailing_distance={} attempt={}",
                event.event_id,
                event.decision.symbol,
                format_price_value(active_price, constraints.tick_size),
                format_price_value(trailing_distance, constraints.tick_size),
                attempt,
            );
            return Ok(());
        }

        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        last_error = Some(format!(
            "bybit trailing-stop retCode={} retMsg={} body={}",
            ret_code, ret_msg, value
        ));

        let retryable_zero_position = ret_msg.contains("zero position");
        let retryable_active_price = ret_msg.contains("TrailingProfit:")
            || ret_msg.contains("should greater than")
            || ret_msg.contains("should be less than");

        if !(retryable_zero_position || retryable_active_price) || attempt == 4 {
            break;
        }

        tokio::time::sleep(Duration::from_millis(350 * attempt as u64)).await;
    }

    Err(last_error
        .unwrap_or_else(|| "unknown trailing-stop error".to_string())
        .into())
}

async fn local_open_qty(pool: &PgPool, symbol: &str, side: &str) -> Result<f64, sqlx::Error> {
    let qty: Option<f64> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity)::double precision, 0)
         FROM trades
         WHERE symbol = $1 AND side = $2 AND status = 'open'",
    )
    .bind(symbol)
    .bind(side)
    .fetch_one(pool)
    .await?;
    Ok(qty.unwrap_or(0.0))
}

fn reconciliation_event_meta(fix_applied: bool) -> (&'static str, &'static str) {
    if fix_applied {
        ("executor_reconciliation_fix_applied", "info")
    } else {
        ("executor_reconciliation_detected", "warning")
    }
}

async fn record_reconciliation_event(
    pool: &PgPool,
    symbol: &str,
    side: &str,
    local_qty: f64,
    bybit_qty: f64,
    diff: f64,
    fix_applied: bool,
) -> Result<(), sqlx::Error> {
    let (event_type, severity) = reconciliation_event_meta(fix_applied);
    let data = json!({
        "symbol": symbol,
        "side": side,
        "local_qty": local_qty,
        "bybit_qty": bybit_qty,
        "diff": diff,
        "fix_applied": fix_applied,
    });

    sqlx::query(
        "INSERT INTO system_events (event_type, severity, category, data, symbol)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(event_type)
    .bind(severity)
    .bind("reconciliation")
    .bind(data)
    .bind(symbol)
    .execute(pool)
    .await?;

    Ok(())
}

async fn apply_reconciliation_reduce_local(
    pool: &PgPool,
    symbol: &str,
    side: &str,
    target_qty: f64,
) -> Result<(f64, f64), sqlx::Error> {
    let open_trades: Vec<(String, f64)> = sqlx::query_as(
        "SELECT trade_id::text, quantity::double precision
         FROM trades
         WHERE symbol = $1 AND side = $2 AND status = 'open'
         ORDER BY opened_at DESC",
    )
    .bind(symbol)
    .bind(side)
    .fetch_all(pool)
    .await?;

    let local_qty: f64 = open_trades.iter().map(|(_, q)| *q).sum();
    let mut to_reduce = (local_qty - target_qty).max(0.0);
    let eps = 1e-9_f64;

    for (trade_id, qty) in open_trades {
        if to_reduce <= eps {
            break;
        }

        if to_reduce + eps >= qty {
            sqlx::query(
                "UPDATE trades
                 SET status='closed',
                     close_reason='error',
                     closed_at=NOW(),
                     updated_at=NOW()
                 WHERE trade_id::text=$1",
            )
            .bind(&trade_id)
            .execute(pool)
            .await?;
            to_reduce -= qty;
        } else {
            let new_qty = (qty - to_reduce).max(0.0);
            sqlx::query(
                "UPDATE trades
                 SET quantity=$2,
                     updated_at=NOW()
                 WHERE trade_id::text=$1",
            )
            .bind(&trade_id)
            .bind(new_qty)
            .execute(pool)
            .await?;

            break;
        }
    }

    let final_qty = local_open_qty(pool, symbol, side).await?;
    Ok((local_qty, final_qty))
}

async fn run_reconciliation_tick(
    state: &ExecutorState,
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
) -> Result<(), Box<dyn Error>> {
    if !cfg.live_orders_enabled {
        return Ok(());
    }
    let Some(pool) = &state.db_pool else {
        return Ok(());
    };

    let symbols: Vec<String> = if cfg.live_symbol_allowlist.is_empty() {
        sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT symbol FROM trades WHERE status = 'open' ORDER BY symbol",
        )
        .fetch_all(pool)
        .await?
    } else {
        cfg.live_symbol_allowlist.iter().cloned().collect()
    };

    for symbol in symbols {
        for side in ["Long", "Short"] {
            let local_qty = local_open_qty(pool, &symbol, side).await?;
            let bybit_qty = fetch_bybit_position_qty(http, cfg, &symbol, side).await?;
            let diff = (local_qty - bybit_qty).abs();

            if diff > 1e-6 {
                eprintln!(
                    "Reconciliation diff symbol={} side={} local_qty={} bybit_qty={} diff={} fix_mode={}",
                    symbol, side, local_qty, bybit_qty, diff, cfg.reconcile_fix
                );

                if cfg.reconcile_fix {
                    if local_qty > bybit_qty + 1e-6 {
                        match apply_reconciliation_reduce_local(pool, &symbol, side, bybit_qty)
                            .await
                        {
                            Ok((before, after)) => {
                                let _ = record_reconciliation_event(
                                    pool,
                                    &symbol,
                                    side,
                                    before,
                                    bybit_qty,
                                    (before - bybit_qty).abs(),
                                    true,
                                )
                                .await;
                                println!(
                                    "Reconciliation fix applied symbol={} side={} before_local={} target_bybit={} after_local={}",
                                    symbol, side, before, bybit_qty, after
                                );
                            }
                            Err(e) => {
                                eprintln!(
                                    "Reconciliation fix failed symbol={} side={} err={}",
                                    symbol, side, e
                                );
                            }
                        }
                    } else {
                        let _ = record_reconciliation_event(
                            pool, &symbol, side, local_qty, bybit_qty, diff, false,
                        )
                        .await;
                        eprintln!(
                            "Reconciliation fix skipped symbol={} side={} reason=local_less_than_bybit",
                            symbol, side
                        );
                    }
                } else {
                    let _ = record_reconciliation_event(
                        pool, &symbol, side, local_qty, bybit_qty, diff, false,
                    )
                    .await;
                }
            }
        }
    }

    Ok(())
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

    let idem_key = idempotency_key(&event).to_string();

    if !claim_processed_event(state, &idem_key, &event).await? {
        println!(
            "Skipping duplicate decision event_id={} source_event_id={}",
            event.event_id, idem_key
        );
        return Ok(());
    }

    if event.decision.action == "HOLD" {
        mark_processed(state, &idem_key, &event, "ignored_hold", None, None).await?;
        return Ok(());
    }

    if action_to_side(&event.decision.action).is_none() {
        let err = format!("unsupported action {}", event.decision.action);
        mark_processed(state, &idem_key, &event, "error", None, Some(&err)).await?;
        return Ok(());
    }

    let is_close = is_close_action(&event.decision.action);
    let runtime_controls = fetch_runtime_controls(state, cfg).await?;

    if !runtime_controls.executor_enabled && !is_close {
        println!(
            "Executor disabled by operator control; blocking event_id={} action={} symbol={}",
            event.event_id, event.decision.action, event.decision.symbol
        );
        mark_processed(
            state,
            &idem_key,
            &event,
            "blocked_executor_disabled",
            None,
            None,
        )
        .await?;
        return Ok(());
    }

    if runtime_controls.kill_switch_enabled && !is_close {
        println!(
            "Kill switch enabled; blocking event_id={} action={} symbol={}",
            event.event_id, event.decision.action, event.decision.symbol
        );
        mark_processed(state, &idem_key, &event, "blocked_kill_switch", None, None).await?;
        return Ok(());
    }

    if cfg.live_orders_enabled && !cfg.is_symbol_allowed_live(&event.decision.symbol) {
        println!(
            "Live order blocked by allowlist event_id={} symbol={} allowlist={:?}",
            event.event_id, event.decision.symbol, cfg.live_symbol_allowlist
        );
        mark_processed(
            state,
            &idem_key,
            &event,
            "blocked_symbol_allowlist",
            None,
            None,
        )
        .await?;
        return Ok(());
    }

    if !cfg.live_orders_enabled {
        let paper_order_id = format!("paper-{}", event.event_id);
        println!(
            "Live orders disabled; paper-trade dry-run for event_id={} action={} symbol={}",
            event.event_id, event.decision.action, event.decision.symbol,
        );

        let status = if is_close_action(&event.decision.action) {
            let close_qty = event.decision.quantity;
            let close_price = event.decision.entry_price;
            match close_open_trade(state, &event, close_qty, close_price, 0.0).await {
                Ok(CloseReconcileResult::Closed { .. }) => "paper_close",
                Ok(CloseReconcileResult::Partial { .. }) => "paper_close_partial",
                Ok(CloseReconcileResult::CloseQtyExceedsOpen { .. }) => {
                    "paper_close_qty_exceeds_open"
                }
                Ok(CloseReconcileResult::NoLocalOpen) => "paper_close_no_local_open",
                Err(e) => {
                    eprintln!(
                        "Failed to reconcile paper close event_id={} err={}",
                        event.event_id, e
                    );
                    "paper_close_no_persist"
                }
            }
        } else {
            let side = if event.decision.action == "ENTER_LONG" {
                "Long"
            } else {
                "Short"
            };

            match has_open_trade_for_symbol_side(state, &event.decision.symbol, side).await {
                Ok(true) => {
                    mark_processed(
                        state,
                        &idem_key,
                        &event,
                        "paper_open_blocked_existing_open",
                        Some(&paper_order_id),
                        None,
                    )
                    .await?;
                    return Ok(());
                }
                Ok(false) => {}
                Err(e) => {
                    eprintln!(
                        "Failed checking open trade for symbol/side event_id={} symbol={} side={} err={}",
                        event.event_id, event.decision.symbol, side, e
                    );
                    mark_processed(
                        state,
                        &idem_key,
                        &event,
                        "paper_open_guard_error",
                        Some(&paper_order_id),
                        Some("paper guard query failed"),
                    )
                    .await?;
                    return Ok(());
                }
            }

            match count_open_trades(state).await {
                Ok(open_count) if open_count >= cfg.paper_max_open_positions => {
                    mark_processed(
                        state,
                        &idem_key,
                        &event,
                        "paper_open_blocked_max_open_positions",
                        Some(&paper_order_id),
                        None,
                    )
                    .await?;
                    return Ok(());
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "Failed checking global open trades event_id={} err={}",
                        event.event_id, e
                    );
                    mark_processed(
                        state,
                        &idem_key,
                        &event,
                        "paper_open_guard_error",
                        Some(&paper_order_id),
                        Some("paper max-open guard query failed"),
                    )
                    .await?;
                    return Ok(());
                }
            }

            let entry_qty = event.decision.quantity;
            let entry_price = event.decision.entry_price;
            if let Err(e) = persist_trade(
                state,
                &event,
                &paper_order_id,
                entry_qty,
                entry_price,
                0.0,
                true,
            )
            .await
            {
                eprintln!(
                    "Failed to persist paper trade for event_id={} order_id={} err={}",
                    event.event_id, paper_order_id, e
                );
                "paper_open_no_persist"
            } else {
                "paper_open"
            }
        };

        mark_processed(
            state,
            &idem_key,
            &event,
            status,
            Some(&paper_order_id),
            None,
        )
        .await?;
        return Ok(());
    }

    if cfg.bybit_api_key.is_empty() || cfg.bybit_api_secret.is_empty() {
        let err = "missing BYBIT_API_KEY/BYBIT_API_SECRET".to_string();
        mark_processed(state, &idem_key, &event, "error", None, Some(&err)).await?;
        return Ok(());
    }

    match submit_market_order(state, http, cfg, &event).await {
        Ok(order_id) => {
            println!(
                "Submitted Bybit order event_id={} order_id={} symbol={} action={}",
                event.event_id, order_id, event.decision.symbol, event.decision.action
            );

            let mut status = "submitted";

            let execution_meta = match fetch_order_execution_meta(
                http,
                cfg,
                &event.decision.symbol,
                &order_id,
            )
            .await
            {
                Ok(meta) => meta,
                Err(e) => {
                    eprintln!(
                        "Failed to fetch execution metadata event_id={} order_id={} err={}, fallback decision price={}",
                        event.event_id, order_id, e, event.decision.entry_price
                    );
                    OrderExecutionMeta {
                        avg_price: None,
                        fee: None,
                        executed_qty: None,
                        fills: Vec::new(),
                    }
                }
            };

            if let Err(e) =
                persist_bybit_fills(state, &event, &order_id, &execution_meta.fills).await
            {
                eprintln!(
                    "Failed to persist Bybit fills event_id={} order_id={} err={}",
                    event.event_id, order_id, e
                );
            }

            if is_close_action(&event.decision.action) {
                let execution_price = execution_meta
                    .avg_price
                    .unwrap_or(event.decision.entry_price);
                let close_fee = execution_meta.fee.unwrap_or(0.0);
                let close_qty = execution_meta
                    .executed_qty
                    .unwrap_or(event.decision.quantity);

                match close_open_trade(state, &event, close_qty, execution_price, close_fee).await {
                    Ok(CloseReconcileResult::Closed {
                        trade_id,
                        realized_pnl,
                    }) => {
                        println!(
                            "Reconciled local close event_id={} trade_id={} symbol={} action={} realized_pnl={}",
                            event.event_id,
                            trade_id,
                            event.decision.symbol,
                            event.decision.action,
                            realized_pnl
                        );
                        status = "submitted_close";
                    }
                    Ok(CloseReconcileResult::Partial {
                        trade_id,
                        remaining_qty,
                        realized_pnl,
                    }) => {
                        println!(
                            "Reconciled partial close event_id={} trade_id={} symbol={} remaining_qty={} realized_pnl={}",
                            event.event_id,
                            trade_id,
                            event.decision.symbol,
                            remaining_qty,
                            realized_pnl
                        );
                        status = "submitted_close_partial";
                    }
                    Ok(CloseReconcileResult::CloseQtyExceedsOpen {
                        trade_id,
                        open_qty,
                        close_qty,
                        realized_pnl,
                    }) => {
                        eprintln!(
                            "Close qty exceeds local open qty event_id={} trade_id={} symbol={} close_qty={} open_qty={} realized_pnl={}",
                            event.event_id,
                            trade_id,
                            event.decision.symbol,
                            close_qty,
                            open_qty,
                            realized_pnl
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
            } else {
                let entry_qty = execution_meta
                    .executed_qty
                    .unwrap_or(event.decision.quantity);
                let entry_price = execution_meta
                    .avg_price
                    .unwrap_or(event.decision.entry_price);
                let entry_fee = execution_meta.fee.unwrap_or(0.0);

                if let Err(e) = persist_trade(
                    state,
                    &event,
                    &order_id,
                    entry_qty,
                    entry_price,
                    entry_fee,
                    false,
                )
                .await
                {
                    eprintln!(
                        "Failed to persist trade for event_id={} order_id={} err={}",
                        event.event_id, order_id, e
                    );
                    status = "submitted_no_persist";
                }

                if let Err(e) = set_bybit_trailing_stop(state, http, cfg, &event, entry_price).await
                {
                    eprintln!(
                        "Failed to configure Bybit trailing stop event_id={} order_id={} symbol={} err={}",
                        event.event_id, order_id, event.decision.symbol, e
                    );
                    if status == "submitted" {
                        status = "submitted_trailing_not_set";
                    }
                }
            }

            mark_processed(state, &idem_key, &event, status, Some(&order_id), None).await?;
        }
        Err(e) => {
            let err = e.to_string();
            eprintln!(
                "Failed to submit Bybit order event_id={} action={} symbol={} err={}",
                event.event_id, event.decision.action, event.decision.symbol, err
            );
            mark_processed(state, &idem_key, &event, "error", None, Some(&err)).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-executor");

    let cfg = ExecutorConfig::from_env();
    println!(
        "Executor mode={} bybit_env={} executor_default_enabled={} live_orders_enabled={} reconcile_fix={} paper_max_open_positions={} base_url={} allowlist={:?}",
        cfg.trading_mode.as_str(),
        cfg.bybit_env,
        cfg.executor_default_enabled,
        cfg.live_orders_enabled,
        cfg.reconcile_fix,
        cfg.paper_max_open_positions,
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

    let db_pool = connect_executor_db(&cfg).await?;

    let state = ExecutorState {
        db_pool,
        processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
        constraints_cache: Arc::new(Mutex::new(HashMap::new())),
    };

    let listener = TcpListener::bind("0.0.0.0:8083").await?;
    println!("Health check server running on :8083");

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();

    let state_for_reconcile = state.clone();
    let cfg_for_reconcile = cfg.clone();
    let http_for_reconcile = http.clone();
    let mut reconcile_shutdown_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = reconcile_shutdown_rx.changed() => {
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    if let Err(e) = run_reconciliation_tick(&state_for_reconcile, &http_for_reconcile, &cfg_for_reconcile).await {
                        eprintln!("Reconciliation tick failed: {}", e);
                    }
                }
            }
        }
    });
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
                    eprintln!("Decision stream ended unexpectedly; exiting so container can restart");
                    return Err("decision stream ended unexpectedly".into());
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
            realized_pnl: 1.25,
        };
        let text = format!("{:?}", result);
        assert!(text.contains("Partial"));
    }

    #[test]
    fn calculates_realized_pnl() {
        let long_pnl = realized_pnl("Long", 100.0, 110.0, 2.0, 2.0);
        let short_pnl = realized_pnl("Short", 100.0, 90.0, 2.0, 2.0);
        assert_eq!(long_pnl, 40.0);
        assert_eq!(short_pnl, 40.0);
    }

    #[test]
    fn normalizes_quantity_by_step() {
        let c = BybitSymbolConstraints {
            min_order_qty: 1.0,
            qty_step: 0.1,
            tick_size: 0.0001,
            min_notional: Some(5.0),
        };
        let q = normalize_order_quantity(10.09, c).expect("normalize qty");
        assert!((q - 10.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_quantity_below_min_after_normalization() {
        let c = BybitSymbolConstraints {
            min_order_qty: 1.0,
            qty_step: 0.1,
            tick_size: 0.0001,
            min_notional: Some(5.0),
        };
        let err = normalize_order_quantity(0.95, c).expect_err("should reject");
        assert!(err.contains("below minOrderQty"));
    }

    #[test]
    fn snaps_quantity_close_to_step_boundary() {
        let c = BybitSymbolConstraints {
            min_order_qty: 0.1,
            qty_step: 0.1,
            tick_size: 0.0001,
            min_notional: Some(5.0),
        };
        let q = normalize_order_quantity(0.09999999964, c).expect("normalize qty");
        assert!((q - 0.1).abs() < 1e-9);
    }

    #[test]
    fn snaps_quantity_close_to_fractional_step_boundary() {
        let c = BybitSymbolConstraints {
            min_order_qty: 0.1,
            qty_step: 0.1,
            tick_size: 0.0001,
            min_notional: Some(5.0),
        };
        let q = normalize_order_quantity(8.1999999996, c).expect("normalize qty");
        assert!((q - 8.2).abs() < 1e-9);
    }

    #[test]
    fn formats_qty_with_step_precision() {
        assert_eq!(format_order_qty(100.0, 1.0), "100");
        assert_eq!(format_order_qty(12.34, 0.01), "12.34");
        assert_eq!(format_order_qty(12.30, 0.01), "12.3");
    }

    #[test]
    fn validates_min_notional_for_enter_and_skips_close() {
        let c = BybitSymbolConstraints {
            min_order_qty: 1.0,
            qty_step: 0.1,
            tick_size: 0.0001,
            min_notional: Some(5.0),
        };

        let low = ensure_min_notional("ENTER_LONG", 10.0, 0.4, c);
        assert!(low.is_err());

        let close = ensure_min_notional("CLOSE_LONG", 1.0, 0.1, c);
        assert!(close.is_ok());
    }

    #[tokio::test]
    async fn db_close_reconcile_partial_then_full() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };

        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let trade_key = format!("it-close-{}", now_ms());

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
            ) VALUES ($1,$2,'DOGEUSDT','Long',10,1.0,2.0,'open',$3,true,'it',true)",
        )
        .bind(&trade_key)
        .bind(format!("bybit-{}", trade_key))
        .bind(format!("hash-{}", trade_key))
        .execute(&pool)
        .await
        .expect("seed trade");

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
        };

        let partial: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": format!("{}-p", trade_key),
            "source_event_id": format!("{}-sp", trade_key),
            "timestamp": "2026-01-01T00:00:00Z",
            "decision": {
                "action": "CLOSE_LONG",
                "symbol": "DOGEUSDT",
                "quantity": 4.0,
                "leverage": 2.0,
                "entry_price": 1.1,
                "stop_loss": 0.0,
                "take_profit": 0.0,
                "reason": "it",
                "smart_copy_compatible": true
            }
        }))
        .expect("partial event");

        let res = close_open_trade(&state, &partial, 4.0, 1.1, 0.0)
            .await
            .expect("partial close should work");
        assert!(matches!(res, CloseReconcileResult::Partial { .. }));

        let (qty_after_partial, status_after_partial): (f64, String) = sqlx::query_as(
            "SELECT quantity::double precision, status FROM trades WHERE order_link_id = $1",
        )
        .bind(&trade_key)
        .fetch_one(&pool)
        .await
        .expect("query partial");
        assert!((qty_after_partial - 6.0).abs() < 1e-9);
        assert_eq!(status_after_partial, "open");

        let full: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": format!("{}-f", trade_key),
            "source_event_id": format!("{}-sf", trade_key),
            "timestamp": "2026-01-01T00:01:00Z",
            "decision": {
                "action": "CLOSE_LONG",
                "symbol": "DOGEUSDT",
                "quantity": 6.0,
                "leverage": 2.0,
                "entry_price": 1.2,
                "stop_loss": 0.0,
                "take_profit": 0.0,
                "reason": "it",
                "smart_copy_compatible": true
            }
        }))
        .expect("full event");

        let res = close_open_trade(&state, &full, 6.0, 1.2, 0.0)
            .await
            .expect("full close should work");
        assert!(matches!(res, CloseReconcileResult::Closed { .. }));

        let (status_after_full, closed_at_is_set, qty_after_full): (String, bool, f64) = sqlx::query_as(
            "SELECT status, (closed_at IS NOT NULL), quantity::double precision FROM trades WHERE order_link_id = $1",
        )
        .bind(&trade_key)
        .fetch_one(&pool)
        .await
        .expect("query full");
        assert_eq!(status_after_full, "closed");
        assert!(closed_at_is_set);
        assert!((qty_after_full - 6.0).abs() < 1e-9);

        sqlx::query("DELETE FROM trades WHERE order_link_id = $1")
            .bind(&trade_key)
            .execute(&pool)
            .await
            .expect("cleanup");
    }

    #[tokio::test]
    async fn db_persist_bybit_fills_is_idempotent() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };

        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
        };

        let key = format!("it-fill-{}", now_ms());
        let event: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": key,
            "source_event_id": format!("{}-src", key),
            "timestamp": "2026-01-01T00:00:00Z",
            "decision": {
                "action": "ENTER_LONG",
                "symbol": "DOGEUSDT",
                "quantity": 10.0,
                "leverage": 2.0,
                "entry_price": 0.2,
                "stop_loss": 0.0,
                "take_profit": 0.0,
                "reason": "it",
                "smart_copy_compatible": true
            }
        }))
        .expect("event parse");

        let fill = BybitFill {
            execution_id: format!("{}-exec", event.event_id),
            order_id: format!("{}-order", event.event_id),
            side: Some("Buy".to_string()),
            exec_qty: 10.0,
            exec_price: Some(0.2),
            exec_fee: 0.01,
            fee_currency: Some("USDT".to_string()),
            is_maker: Some(false),
            exec_time_ms: Some(1_700_000_000_000),
            raw_data: serde_json::json!({"k":"v"}),
        };

        persist_bybit_fills(&state, &event, "order-fallback", &[fill.clone()])
            .await
            .expect("insert fill");
        persist_bybit_fills(&state, &event, "order-fallback", &[fill])
            .await
            .expect("insert fill idempotent");

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM bybit_fills WHERE bybit_execution_id = $1")
                .bind(format!("{}-exec", event.event_id))
                .fetch_one(&pool)
                .await
                .expect("query fill count");

        assert_eq!(count, 1);

        sqlx::query("DELETE FROM bybit_fills WHERE bybit_execution_id = $1")
            .bind(format!("{}-exec", event.event_id))
            .execute(&pool)
            .await
            .expect("cleanup fill");
    }

    #[test]
    fn maps_reconciliation_event_meta() {
        assert_eq!(
            reconciliation_event_meta(false),
            ("executor_reconciliation_detected", "warning")
        );
        assert_eq!(
            reconciliation_event_meta(true),
            ("executor_reconciliation_fix_applied", "info")
        );
    }

    #[test]
    fn signs_payload() {
        let sig = bybit_sign("secret", "payload").expect("must sign");
        assert!(!sig.is_empty());
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn parses_allowlist() {
        let set = parse_allowlist("dogeusdt, xrpusdt ,, ADAUSDT");
        assert!(set.contains("DOGEUSDT"));
        assert!(set.contains("XRPUSDT"));
        assert!(set.contains("ADAUSDT"));
        assert_eq!(set.len(), 3);
    }
}
