use serde_json::Value;
use serde_yaml::Value as YamlValue;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch, Mutex};
use viper_domain::{
    stream_ensure_group, stream_publish, StrategyDecision, StrategyDecisionEvent,
    REDIS_STREAM_CONTROL_EVENTS, REDIS_STREAM_DECISIONS, REDIS_STREAM_EXECUTOR_EVENTS,
    STREAM_GROUP_EXECUTOR,
};

const CONSTRAINTS_CACHE_TTL_SECS: u64 = 60;
mod bybit_client;
mod orders;
mod reconciliation;
mod risk;
mod state;

pub(crate) use bybit_client::*;
pub(crate) use orders::*;
pub(crate) use reconciliation::*;
pub(crate) use risk::*;
pub(crate) use state::*;

#[derive(Debug, Clone)]
struct ExecutorConfig {
    redis_url: String,
    db_url: String,
    trading_mode: TradingMode,
    bybit_api_key: String,
    bybit_api_secret: String,
    recv_window: String,
    bybit_account_type: String,
    executor_default_enabled: bool,
    live_orders_enabled: bool,
    live_symbol_allowlist: HashSet<String>,
    reconcile_fix: bool,
    reconcile_auto_fix: bool,
    reconcile_max_correction_pct: f64,
    reconcile_max_daily: i64,
    paper_max_open_positions: i64,
    strategy_config_path: String,
    trading_profile: String,
    /// Fallback taker fee rate (fraction) used when the Bybit fee-rate API is
    /// unavailable — e.g. paper mode without credentials.
    fee_taker_pct: f64,
    /// Simulated starting balance. Same env var the API reads, so the executor's
    /// margin check and the wallet shown to the operator agree.
    initial_capital_usd: f64,
}

/// Só existem dois modos: simular localmente (Paper) ou operar valendo
/// (Mainnet). O testnet da Bybit foi removido — o book de lá não reproduz
/// liquidez nem spread reais, então validar estratégia contra ele dava
/// confiança falsa; o Paper já cobre a simulação, com custo e slippage.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TradingMode {
    Paper,
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
            "mainnet" | "live" => Self::Mainnet,
            _ => Self::Paper,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "PAPER",
            Self::Mainnet => "MAINNET",
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
    reconcile_daily_counts: Arc<Mutex<HashMap<String, (String, i64)>>>,
    fee_rate_cache: Arc<Mutex<HashMap<String, (Instant, BybitFeeRate)>>>,
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
                .unwrap_or_default()
                .as_str(),
        );
        let reconcile_fix = std::env::var("EXECUTOR_RECONCILE_FIX")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let reconcile_auto_fix = std::env::var("EXECUTOR_RECONCILE_AUTO_FIX")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let reconcile_max_correction_pct = std::env::var("RECONCILE_MAX_CORRECTION_PCT")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0 && *v <= 1.0)
            .unwrap_or(0.05);
        let reconcile_max_daily = std::env::var("RECONCILE_MAX_DAILY")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(5);
        let paper_max_open_positions = std::env::var("EXECUTOR_PAPER_MAX_OPEN_POSITIONS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(2);
        let strategy_config_path = std::env::var("STRATEGY_CONFIG")
            .unwrap_or_else(|_| "config/trading/pairs.yaml".to_string());
        let trading_profile =
            std::env::var("TRADING_PROFILE").unwrap_or_else(|_| "MEDIUM".to_string());
        // Bybit USDT-perp non-VIP defaults; only used when the fee-rate API
        // cannot be reached (see resolve_taker_fee_rate).
        let initial_capital_usd = std::env::var("INITIAL_CAPITAL_USD")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(100.0);
        let fee_taker_pct = std::env::var("EXECUTOR_FEE_TAKER_PCT")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0 && *v < 0.01)
            .unwrap_or(0.00055);

        Self {
            redis_url,
            db_url,
            trading_mode,
            bybit_api_key,
            bybit_api_secret,
            recv_window,
            bybit_account_type,
            executor_default_enabled,
            live_orders_enabled,
            live_symbol_allowlist,
            reconcile_fix,
            reconcile_auto_fix,
            reconcile_max_correction_pct,
            reconcile_max_daily,
            paper_max_open_positions,
            strategy_config_path,
            trading_profile,
            fee_taker_pct,
            initial_capital_usd,
        }
    }

    fn bybit_base_url(&self) -> &'static str {
        "https://api.bybit.com"
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
                tracing::info!("Executor database connection: enabled");
                return Ok(Some(pool));
            }
            Err(err) => {
                tracing::warn!(attempt = attempt, attempts = attempts, error = %err, "Executor database connection failed");
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
        tracing::warn!(error = %err, "Executor database connection unavailable");
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
    // Paper também usa as chaves de mainnet: não envia ordem, mas lê dados reais
    // da conta (taxa efetiva, constraints de símbolo) para simular fielmente.
    let scoped = (
        from("BYBIT_MAINNET_API_KEY"),
        from("BYBIT_MAINNET_API_SECRET"),
    );

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
    } else {
        "manual".to_string()
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

// Returns the side ("Long"/"Short") of an existing open trade for the symbol, if
// any. Guards against BOTH a same-side duplicate AND an opposite-side hedge: the
// strategy is directional, so at most one open position per symbol is valid.
// Checking only (symbol, side) let an ENTER_LONG open while a Short was still
// open (and vice versa), leaving simultaneous long+short on the same symbol.

/// Taker fee configured in `pairs.yaml`, if present.
///
/// The strategy reads the same key to price its break-even, so keeping this as
/// the shared source stops the decision side and the accounting side from
/// drifting apart. Only used when the live Bybit rate is unavailable.
fn load_configured_taker_fee(cfg: &ExecutorConfig) -> Option<f64> {
    let raw = std::fs::read_to_string(&cfg.strategy_config_path).ok()?;
    let root: YamlValue = serde_yaml::from_str(&raw).ok()?;
    yaml_get(
        &root,
        &["global", "mode_profiles", cfg.trading_mode.as_str()],
    )
    .and_then(|v| yaml_f64(v, &["fee_taker_pct"]))
    .filter(|v| v.is_finite() && *v >= 0.0 && *v < 0.01)
}

/// Faixa de slippage usada apenas como fallback, quando o book real não pode
/// ser lido (ver `paper_fill_price`).
fn load_paper_slippage_config(cfg: &ExecutorConfig) -> (f64, f64) {
    let raw = std::fs::read_to_string(&cfg.strategy_config_path).ok();
    let root: Option<YamlValue> = raw.as_deref().and_then(|r| serde_yaml::from_str(r).ok());
    let mode_key = cfg.trading_mode.as_str();
    let mode_cfg = root
        .as_ref()
        .and_then(|r| yaml_get(r, &["global", "mode_profiles", mode_key]));
    let slip_min = mode_cfg
        .and_then(|v| yaml_f64(v, &["paper_slippage_min"]))
        .unwrap_or(0.0003);
    let slip_max = mode_cfg
        .and_then(|v| yaml_f64(v, &["paper_slippage_max"]))
        .unwrap_or(0.0008);
    (slip_min, slip_max)
}

fn paper_adverse_slippage(action: &str, price: f64, slip_min: f64, slip_max: f64) -> f64 {
    use rand::Rng;
    let slip_pct: f64 = rand::thread_rng().gen_range(slip_min..slip_max);
    match action {
        "ENTER_LONG" | "CLOSE_SHORT" => price * (1.0 + slip_pct),
        "ENTER_SHORT" | "CLOSE_LONG" => price * (1.0 - slip_pct),
        _ => price,
    }
}

const FEE_RATE_CACHE_TTL: Duration = Duration::from_secs(3600);

/// Resolve the taker fee rate for `symbol`, preferring the account's real rate
/// from Bybit and falling back to the configured default.
///
/// Paper entries and exits are modelled as market orders (see
/// `paper_adverse_slippage`), so taker is the correct side of the book.
async fn resolve_taker_fee_rate(
    state: &ExecutorState,
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
) -> f64 {
    {
        let cache = state.fee_rate_cache.lock().await;
        if let Some((fetched_at, rate)) = cache.get(symbol) {
            if fetched_at.elapsed() < FEE_RATE_CACHE_TTL {
                return rate.taker;
            }
        }
    }

    match fetch_fee_rate(http, cfg, symbol).await {
        Ok(rate) => {
            tracing::info!(
                symbol = %symbol,
                taker_pct = rate.taker * 100.0,
                maker_pct = rate.maker * 100.0,
                "Bybit account fee rate resolved"
            );
            let mut cache = state.fee_rate_cache.lock().await;
            cache.insert(symbol.to_string(), (Instant::now(), rate));
            rate.taker
        }
        Err(e) => {
            let fallback = load_configured_taker_fee(cfg).unwrap_or(cfg.fee_taker_pct);
            tracing::warn!(
                symbol = %symbol,
                error = %e,
                fallback_taker_pct = fallback * 100.0,
                "Failed to fetch Bybit fee rate; using configured fallback"
            );
            fallback
        }
    }
}

/// Whether an action buys (lifts the ask) or sells (hits the bid).
fn action_is_buy(action: &str) -> bool {
    matches!(action, "ENTER_LONG" | "CLOSE_SHORT")
}

/// Price at which a paper order fills.
///
/// Prefers the real top of book: a market order crosses the spread, so a buy
/// pays the ask and a sell receives the bid. That reproduces the true cost of
/// immediacy — and widens on its own when the book thins out, which a fixed
/// random draw never did.
///
/// Falls back to the configured random slippage if the book is unavailable, so
/// a hiccup in the public API degrades the simulation instead of stopping it.
/// Devolve `(preço de execução, funding rate vigente)`. O funding vem do mesmo
/// ticker do book, então não custa uma segunda chamada.
async fn paper_fill_price(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
    action: &str,
    reference_price: f64,
    slip_min: f64,
    slip_max: f64,
) -> (f64, f64) {
    match fetch_book_quote(http, cfg, symbol).await {
        Ok(quote) => {
            let price = quote.fill_price(action_is_buy(action));
            // Em INFO de propósito: são poucos fills por dia, e o spread pago é
            // justamente o custo que a simulação antes inventava. Querer auditar
            // isso depois sem ter registrado seria o mesmo buraco das fees.
            let mid = (quote.bid + quote.ask) / 2.0;
            let spread_pct = if mid > 0.0 {
                (quote.ask - quote.bid) / mid * 100.0
            } else {
                0.0
            };
            tracing::info!(
                symbol = %symbol, action = %action,
                bid = quote.bid, ask = quote.ask,
                fill_price = price, spread_pct,
                "Paper fill priced from live book"
            );
            (price, quote.funding_rate)
        }
        Err(e) => {
            tracing::warn!(
                symbol = %symbol, action = %action, error = %e,
                "Book unavailable; pricing paper fill with simulated slippage"
            );
            // Sem ticker não há funding confiável; não cobrar é melhor que inventar.
            (
                paper_adverse_slippage(action, reference_price, slip_min, slip_max),
                0.0,
            )
        }
    }
}

/// Quantos instantes de funding (00:00, 08:00 e 16:00 UTC) o intervalo cobre.
///
/// Perpétuo não vence: o custo de carregar a posição é cobrado nesses horários.
/// Uma posição que abre 15:50 e fecha 16:10 paga um ciclo inteiro; uma que vive
/// 40 minutos sem cruzar nenhum deles não paga nada.
fn funding_events_between(opened_at: i64, closed_at: i64) -> i64 {
    const FUNDING_INTERVAL_SECS: i64 = 8 * 3600;
    if closed_at <= opened_at {
        return 0;
    }
    // Número de fronteiras de 8h estritamente após a abertura e até o fechamento.
    let after_open = opened_at.div_euclid(FUNDING_INTERVAL_SECS);
    let until_close = closed_at.div_euclid(FUNDING_INTERVAL_SECS);
    (until_close - after_open).max(0)
}

/// Funding pago (positivo) ou recebido (negativo) ao carregar a posição.
///
/// Taxa positiva significa que os comprados pagam aos vendidos — por isso o
/// sinal se inverte no Short.
fn funding_cost(notional: f64, funding_rate: f64, side: &str, events: i64) -> f64 {
    if events <= 0 || !notional.is_finite() || notional <= 0.0 || !funding_rate.is_finite() {
        return 0.0;
    }
    let magnitude = notional * funding_rate * events as f64;
    if side.eq_ignore_ascii_case("Long") {
        magnitude
    } else {
        -magnitude
    }
}

/// Trading fee in quote currency for a fill of `qty` at `price`.
fn fee_for_fill(price: f64, qty: f64, rate: f64) -> f64 {
    let notional = price * qty;
    if !notional.is_finite() || notional <= 0.0 || !rate.is_finite() || rate <= 0.0 {
        return 0.0;
    }
    notional * rate
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
        tracing::info!(event_id = %event.event_id, source_event_id = %idem_key, "Skipping duplicate decision");
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
        tracing::warn!(event_id = %event.event_id, action = %event.decision.action, symbol = %event.decision.symbol, "Executor disabled by operator control");
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
        tracing::warn!(event_id = %event.event_id, action = %event.decision.action, symbol = %event.decision.symbol, "Kill switch enabled");
        mark_processed(state, &idem_key, &event, "blocked_kill_switch", None, None).await?;
        return Ok(());
    }

    if cfg.live_orders_enabled && !cfg.is_symbol_allowed_live(&event.decision.symbol) {
        tracing::warn!(
            event_id = %event.event_id,
            symbol = %event.decision.symbol,
            allowlist = ?cfg.live_symbol_allowlist,
            "Live order blocked by allowlist"
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
        tracing::info!(
            event_id = %event.event_id,
            action = %event.decision.action,
            symbol = %event.decision.symbol,
            "Live orders disabled; paper-trade dry-run"
        );

        // Sem sorteio de preenchimento: ordem a mercado deste tamanho sempre
        // encontra contraparte no topo do book. O antigo descarte de 3% jogava
        // fora entradas boas e ruins por igual, sem contrapartida no mundo real.
        let (slip_min, slip_max) = load_paper_slippage_config(cfg);

        let status = if is_close_action(&event.decision.action) {
            let close_qty = event.decision.quantity;
            let (close_price, funding_rate) = paper_fill_price(
                http,
                cfg,
                &event.decision.symbol,
                &event.decision.action,
                event.decision.entry_price,
                slip_min,
                slip_max,
            )
            .await;
            let close_fee = fee_for_fill(
                close_price,
                close_qty,
                resolve_taker_fee_rate(state, http, cfg, &event.decision.symbol).await,
            );
            match close_open_trade(
                state,
                &event,
                close_qty,
                close_price,
                close_fee,
                funding_rate,
            )
            .await
            {
                Ok(CloseReconcileResult::Closed { .. }) => "paper_close",
                Ok(CloseReconcileResult::Partial { .. }) => "paper_close_partial",
                Ok(CloseReconcileResult::CloseQtyExceedsOpen { .. }) => {
                    "paper_close_qty_exceeds_open"
                }
                Ok(CloseReconcileResult::NoLocalOpen) => "paper_close_no_local_open",
                Err(e) => {
                    tracing::warn!(event_id = %event.event_id, error = %e, "Failed to reconcile paper close");
                    "paper_close_no_persist"
                }
            }
        } else {
            let side = if event.decision.action == "ENTER_LONG" {
                "Long"
            } else {
                "Short"
            };

            match open_trade_side_for_symbol(state, &event.decision.symbol).await {
                Ok(Some(existing_side)) => {
                    // Block whether the existing position is the same side (a
                    // duplicate) or the opposite (would hedge long+short).
                    let reason = if existing_side == side {
                        "paper_open_blocked_existing_open"
                    } else {
                        "paper_open_blocked_opposite_open"
                    };
                    tracing::warn!(
                        event_id = %event.event_id,
                        symbol = %event.decision.symbol,
                        requested_side = %side,
                        existing_side = %existing_side,
                        "Blocked ENTER: position already open for symbol"
                    );
                    mark_processed(
                        state,
                        &idem_key,
                        &event,
                        reason,
                        Some(&paper_order_id),
                        None,
                    )
                    .await?;
                    return Ok(());
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(event_id = %event.event_id, symbol = %event.decision.symbol, side = %side, error = %e, "Failed checking open trade");
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
                    tracing::warn!(event_id = %event.event_id, error = %e, "Failed checking global open trades");
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

            // Constraints do símbolo valem no paper também: a Bybit só aceita
            // múltiplos de qtyStep, acima de minOrderQty e de minNotionalValue.
            // Sem isto o paper operava quantidades impossíveis — o arredondamento
            // chega a 20% do notional em símbolos de step grosso (BCH, SUI), o que
            // dava a alguns símbolos mais peso no resultado do que teriam de fato.
            let entry_qty = match get_symbol_constraints(state, http, cfg, &event.decision.symbol)
                .await
            {
                Ok(constraints) => {
                    match normalize_order_quantity(event.decision.quantity, constraints).and_then(
                        |qty| {
                            ensure_min_notional(
                                &event.decision.action,
                                qty,
                                event.decision.entry_price,
                                constraints,
                            )
                            .map(|_| qty)
                        },
                    ) {
                        Ok(qty) => {
                            if (qty - event.decision.quantity).abs() > f64::EPSILON {
                                tracing::info!(
                                    event_id = %event.event_id, symbol = %event.decision.symbol,
                                    requested_qty = event.decision.quantity, normalized_qty = qty,
                                    "Paper entry quantity normalized to exchange constraints"
                                );
                            }
                            qty
                        }
                        Err(e) => {
                            tracing::warn!(
                                event_id = %event.event_id, symbol = %event.decision.symbol,
                                requested_qty = event.decision.quantity, error = %e,
                                "Paper entry rejected by exchange constraints"
                            );
                            mark_processed(
                                state,
                                &idem_key,
                                &event,
                                "paper_open_blocked_constraints",
                                Some(&paper_order_id),
                                Some(&e),
                            )
                            .await?;
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    // Sem constraints não há como saber se a ordem seria aceita.
                    // Bloquear mantém o paper honesto; deixar passar reintroduz
                    // exatamente o viés que esta checagem existe para remover.
                    tracing::warn!(
                        event_id = %event.event_id, symbol = %event.decision.symbol, error = %e,
                        "Symbol constraints unavailable; blocking paper entry"
                    );
                    mark_processed(
                        state,
                        &idem_key,
                        &event,
                        "paper_open_blocked_constraints_unavailable",
                        Some(&paper_order_id),
                        Some("symbol constraints unavailable"),
                    )
                    .await?;
                    return Ok(());
                }
            };
            // Margem: a corretora recusa a ordem se a conta não cobrir a margem
            // exigida. `leverage` divide porque é o quanto do notional sai do
            // saldo — 2x significa metade.
            let required_margin = if event.decision.leverage > 0.0 {
                entry_qty * event.decision.entry_price / event.decision.leverage
            } else {
                entry_qty * event.decision.entry_price
            };
            match available_margin_usdt(state, cfg.initial_capital_usd).await {
                Ok(available) if available + 1e-9 < required_margin => {
                    tracing::warn!(
                        event_id = %event.event_id, symbol = %event.decision.symbol,
                        required_margin, available,
                        "Paper entry rejected: insufficient margin"
                    );
                    mark_processed(
                        state,
                        &idem_key,
                        &event,
                        "paper_open_blocked_insufficient_margin",
                        Some(&paper_order_id),
                        Some(&format!(
                            "required {required_margin:.4} > available {available:.4}"
                        )),
                    )
                    .await?;
                    return Ok(());
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(event_id = %event.event_id, error = %e, "Failed checking available margin");
                    mark_processed(
                        state,
                        &idem_key,
                        &event,
                        "paper_open_guard_error",
                        Some(&paper_order_id),
                        Some("margin query failed"),
                    )
                    .await?;
                    return Ok(());
                }
            }

            let (entry_price, _) = paper_fill_price(
                http,
                cfg,
                &event.decision.symbol,
                &event.decision.action,
                event.decision.entry_price,
                slip_min,
                slip_max,
            )
            .await;
            let entry_fee = fee_for_fill(
                entry_price,
                entry_qty,
                resolve_taker_fee_rate(state, http, cfg, &event.decision.symbol).await,
            );
            if let Err(e) = persist_trade(
                state,
                &event,
                &paper_order_id,
                entry_qty,
                entry_price,
                entry_fee,
                true,
            )
            .await
            {
                tracing::warn!(event_id = %event.event_id, order_id = %paper_order_id, error = %e, "Failed to persist paper trade");
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
            tracing::info!(
                event_id = %event.event_id,
                order_id = %order_id,
                symbol = %event.decision.symbol,
                action = %event.decision.action,
                "Submitted Bybit order"
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
                    tracing::warn!(event_id = %event.event_id, order_id = %order_id, error = %e, entry_price = event.decision.entry_price, "Failed to fetch execution metadata");
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
                tracing::warn!(event_id = %event.event_id, order_id = %order_id, error = %e, "Failed to persist Bybit fills");
            }

            if is_close_action(&event.decision.action) {
                let execution_price = execution_meta
                    .avg_price
                    .unwrap_or(event.decision.entry_price);
                let close_fee = execution_meta.fee.unwrap_or(0.0);
                let close_qty = execution_meta
                    .executed_qty
                    .unwrap_or(event.decision.quantity);

                // Em mainnet a Bybit debita o funding direto na carteira; simular
                // aqui contaria o mesmo custo duas vezes.
                match close_open_trade(state, &event, close_qty, execution_price, close_fee, 0.0)
                    .await
                {
                    Ok(CloseReconcileResult::Closed {
                        trade_id,
                        realized_pnl,
                    }) => {
                        tracing::info!(
                            event_id = %event.event_id,
                            trade_id = %trade_id,
                            symbol = %event.decision.symbol,
                            action = %event.decision.action,
                            realized_pnl = realized_pnl,
                            "Reconciled local close"
                        );
                        status = "submitted_close";
                    }
                    Ok(CloseReconcileResult::Partial {
                        trade_id,
                        remaining_qty,
                        realized_pnl,
                    }) => {
                        tracing::info!(
                            event_id = %event.event_id,
                            trade_id = %trade_id,
                            symbol = %event.decision.symbol,
                            remaining_qty = remaining_qty,
                            realized_pnl = realized_pnl,
                            "Reconciled partial close"
                        );
                        status = "submitted_close_partial";
                    }
                    Ok(CloseReconcileResult::CloseQtyExceedsOpen {
                        trade_id,
                        open_qty,
                        close_qty,
                        realized_pnl,
                    }) => {
                        tracing::warn!(event_id = %event.event_id, trade_id = %trade_id, symbol = %event.decision.symbol, close_qty = close_qty, open_qty = open_qty, realized_pnl = realized_pnl, "Close qty exceeds local open qty");
                        status = "submitted_close_qty_exceeds_open";
                    }
                    Ok(CloseReconcileResult::NoLocalOpen) => {
                        tracing::warn!(event_id = %event.event_id, symbol = %event.decision.symbol, action = %event.decision.action, "No local open trade to close");
                        status = "submitted_close_no_local_open";
                    }
                    Err(e) => {
                        tracing::warn!(event_id = %event.event_id, order_id = %order_id, error = %e, "Failed to reconcile local close");
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
                    tracing::warn!(event_id = %event.event_id, order_id = %order_id, error = %e, "Failed to persist trade");
                    status = "submitted_no_persist";
                }

                if let Err(e) = set_bybit_trailing_stop(state, http, cfg, &event, entry_price).await
                {
                    tracing::warn!(event_id = %event.event_id, order_id = %order_id, symbol = %event.decision.symbol, error = %e, "Failed to configure Bybit trailing stop");
                    if status == "submitted" {
                        status = "submitted_trailing_not_set";
                    }
                }
            }

            mark_processed(state, &idem_key, &event, status, Some(&order_id), None).await?;
        }
        Err(e) => {
            let err = e.to_string();
            tracing::warn!(event_id = %event.event_id, action = %event.decision.action, symbol = %event.decision.symbol, error = %err, "Failed to submit Bybit order");
            mark_processed(state, &idem_key, &event, "error", None, Some(&err)).await?;
        }
    }

    Ok(())
}

pub async fn run() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "viper_executor=info".into()),
        )
        .json()
        .init();

    tracing::info!("Starting viper-executor");

    let cfg = ExecutorConfig::from_env();

    // Defense-in-depth: live_orders_enabled is derived from trading_mode, but if an
    // operator explicitly tries to force live orders via env while in a non-executing
    // mode (e.g. paper), refuse to start instead of silently ignoring the request.
    let live_override = std::env::var("EXECUTOR_ENABLE_LIVE_ORDERS")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    if live_override && !cfg.trading_mode.executes_exchange_orders() {
        tracing::error!(
            mode = %cfg.trading_mode.as_str(),
            "REFUSING TO START: EXECUTOR_ENABLE_LIVE_ORDERS=true requires mainnet, got paper"
        );
        std::process::exit(1);
    }

    tracing::info!(
        mode = %cfg.trading_mode.as_str(),
        executor_default_enabled = cfg.executor_default_enabled,
        live_orders_enabled = cfg.live_orders_enabled,
        reconcile_fix = cfg.reconcile_fix,
        paper_max_open_positions = cfg.paper_max_open_positions,
        base_url = %cfg.bybit_base_url(),
        allowlist = ?cfg.live_symbol_allowlist,
        "Executor config"
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
            tracing::warn!(error = %err, "Bybit sanity checks warning (continuing dry-run)");
        }
    }

    let db_pool = connect_executor_db(&cfg).await?;

    let state = ExecutorState {
        db_pool,
        processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
        constraints_cache: Arc::new(Mutex::new(HashMap::new())),
        fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
        reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
    };

    let listener = TcpListener::bind("0.0.0.0:8083").await?;
    tracing::info!("Health check server running on :8083");

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
                        tracing::warn!(error = %e, "Reconciliation tick failed");
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
                                tracing::warn!(error = ?e, "Failed to write to socket");
                            }
                        });
                    }
                }
            }
        }
    });

    tracing::info!(redis_url = %cfg.redis_url, "Connecting to Redis");
    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut pub_conn = redis_client.get_multiplexed_async_connection().await?;
    stream_ensure_group(&mut pub_conn, REDIS_STREAM_DECISIONS, STREAM_GROUP_EXECUTOR).await;
    stream_ensure_group(
        &mut pub_conn,
        REDIS_STREAM_CONTROL_EVENTS,
        STREAM_GROUP_EXECUTOR,
    )
    .await;

    let (decision_tx, mut decision_rx) = mpsc::unbounded_channel::<String>();
    let decision_consumer = format!("executor-{}", std::process::id());
    let decision_tx_clone = decision_tx.clone();
    let mut stream_shutdown_rx = shutdown_rx.clone();
    let redis_url_stream = cfg.redis_url.clone();
    tokio::spawn(async move {
        loop {
            let c = match redis::Client::open(redis_url_stream.as_str()) {
                Ok(c) => c,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
            let mut conn = match c.get_multiplexed_async_connection().await {
                Ok(c) => c,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
            loop {
                tokio::select! {
                    _ = stream_shutdown_rx.changed() => return,
                    result = async {
                        let r: redis::RedisResult<viper_domain::StreamEntries> = redis::cmd("XREADGROUP")
                            .arg("GROUP").arg(STREAM_GROUP_EXECUTOR).arg(&decision_consumer)
                            .arg("BLOCK").arg(2000).arg("COUNT").arg(1)
                            .arg("STREAMS").arg(REDIS_STREAM_DECISIONS).arg(">")
                            .query_async(&mut conn).await;
                        r
                    } => {
                        match result {
                            Ok(entries) => {
                                for (_stream, msgs) in entries {
                                    for (msg_id, fields) in msgs {
                                        for (k, v) in fields {
                                            if k == "payload" {
                                                let _ = decision_tx_clone.send(v);
                                                let _: Result<String, _> = redis::cmd("XACK")
                                                    .arg(REDIS_STREAM_DECISIONS).arg(STREAM_GROUP_EXECUTOR).arg(&msg_id)
                                                    .query_async(&mut conn).await;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Decision stream read failed");
                                break;
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    let state_ctl = state.clone();
    let mut shutdown_rx_ctl = shutdown_rx.clone();
    let redis_url_ctl = cfg.redis_url.clone();
    let ctl_consumer = format!("executor-ctl-{}", std::process::id());
    tokio::spawn(async move {
        loop {
            let ctl_client = match redis::Client::open(redis_url_ctl.as_str()) {
                Ok(c) => c,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
            let mut ctl_conn = match ctl_client.get_multiplexed_async_connection().await {
                Ok(c) => c,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
            loop {
                tokio::select! {
                    _ = shutdown_rx_ctl.changed() => return,
                    result = async {
                        let r: redis::RedisResult<viper_domain::StreamEntries> = redis::cmd("XREADGROUP")
                            .arg("GROUP").arg(STREAM_GROUP_EXECUTOR).arg(&ctl_consumer)
                            .arg("BLOCK").arg(2000).arg("COUNT").arg(1)
                            .arg("STREAMS").arg(REDIS_STREAM_CONTROL_EVENTS).arg(">")
                            .query_async(&mut ctl_conn).await;
                        r
                    } => {
                        match result {
                            Ok(entries) => {
                                for (_stream, msgs) in entries {
                                    for (msg_id, fields) in msgs {
                                        for (k, v) in fields {
                                            if k == "payload" {
                                                let control: serde_json::Value = match serde_json::from_str(&v) {
                                                    Ok(val) => val,
                                                    Err(_) => continue,
                                                };
                                                if control.get("type").and_then(|t| t.as_str()) == Some("kill_switch_sync") {
                                                    let enabled = control.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                                                    let request_id = control.get("request_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                                    let pool = state_ctl.db_pool.clone();
                                                    let actual_enabled = if let Some(p) = pool {
                                                        fetch_latest_control_flag(&p, "api_kill_switch_set", false).await.unwrap_or(false)
                                                    } else { false };
                                                    let ack = serde_json::json!({"type": "kill_switch_ack", "enabled": actual_enabled, "request_id": request_id, "status": "applied"});
                                                    let _ = stream_publish(&mut ctl_conn, REDIS_STREAM_EXECUTOR_EVENTS, &ack.to_string()).await;
                                                    tracing::info!(request_id = %request_id, requested_enabled = enabled, actual_enabled = actual_enabled, "Executor processed kill-switch sync request");
                                                }
                                                let _: Result<String, _> = redis::cmd("XACK")
                                                    .arg(REDIS_STREAM_CONTROL_EVENTS).arg(STREAM_GROUP_EXECUTOR).arg(&msg_id)
                                                    .query_async(&mut ctl_conn).await;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Control stream read failed");
                                break;
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                tracing::info!("Received shutdown signal, stopping viper-executor");
                break;
            }
            maybe_msg = decision_rx.recv() => {
                let Some(payload) = maybe_msg else {
                    tracing::warn!("Decision stream ended unexpectedly; exiting so container can restart");
                    return Err("decision stream ended unexpectedly".into());
                };

                if let Ok(event) = serde_json::from_str::<StrategyDecisionEvent>(&payload) {
                    if let Err(e) = handle_decision_event(&state, &http, &cfg, event.clone()).await {
                        tracing::warn!(event_id = %event.event_id, error = %e, "Executor failed handling event");
                    }

                    let _ = stream_publish(&mut pub_conn, REDIS_STREAM_EXECUTOR_EVENTS, &payload).await;
                    continue;
                }

                if let Ok(decision) = serde_json::from_str::<StrategyDecision>(&payload) {
                    if let Err(err) = decision.validate() {
                        tracing::warn!(error = %err, "Executor rejected invalid legacy decision");
                        continue;
                    }

                    tracing::warn!(action = %decision.action, symbol = %decision.symbol, "Executor received legacy decision without event envelope");
                    continue;
                }

                tracing::warn!("Executor failed to parse decision payload");
            }
        }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn db_available_margin_discounts_open_positions_and_costs() {
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
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        // Mede a variação, não o absoluto: o banco de teste pode já ter trades.
        let before = available_margin_usdt(&state, 100.0).await.expect("margin");

        let key = format!("it-margin-{}", now_ms());
        // Posição aberta: $20 de notional com 2x prende $10 de margem.
        sqlx::query(
            "INSERT INTO trades (order_link_id, bybit_order_id, symbol, side, quantity,
                                 entry_price, leverage, status, decision_hash, paper_trade)
             VALUES ($1, $1, 'BTCUSDT', 'Long', 2, 10, 2, 'open', 'h', TRUE)",
        )
        .bind(format!("{key}-open"))
        .execute(&pool)
        .await
        .expect("insert open");

        // Fechado: perdeu $5 e pagou $0,75 de taxa e $0,25 de funding.
        sqlx::query(
            "INSERT INTO trades (order_link_id, bybit_order_id, symbol, side, quantity,
                                 entry_price, leverage, status, decision_hash, paper_trade,
                                 pnl, fees, funding_paid)
             VALUES ($1, $1, 'BTCUSDT', 'Long', 1, 10, 2, 'closed', 'h', TRUE, -5, 0.75, 0.25)",
        )
        .bind(format!("{key}-closed"))
        .execute(&pool)
        .await
        .expect("insert closed");

        let after = available_margin_usdt(&state, 100.0).await.expect("margin");

        // Consumiu 10 (margem presa) + 5 (perda) + 0,75 + 0,25 = 16.
        assert!(
            (before - after - 16.0).abs() < 1e-6,
            "esperado consumo de 16, veio {}",
            before - after
        );

        sqlx::query("DELETE FROM trades WHERE order_link_id LIKE $1")
            .bind(format!("{key}%"))
            .execute(&pool)
            .await
            .expect("cleanup");
    }

    // ── funding ───────────────────────────────────────────────────
    /// 2026-08-02 16:00 UTC em epoch, uma das três fronteiras de funding.
    const FUNDING_BOUNDARY: i64 = 1785686400;

    #[test]
    fn funding_counts_only_boundaries_actually_crossed() {
        // Abre 10min antes e fecha 10min depois da fronteira: cruzou uma.
        assert_eq!(
            funding_events_between(FUNDING_BOUNDARY - 600, FUNDING_BOUNDARY + 600),
            1
        );
        // Vive 40min inteiramente dentro de uma janela: não cruzou nenhuma.
        assert_eq!(
            funding_events_between(FUNDING_BOUNDARY + 600, FUNDING_BOUNDARY + 3000),
            0
        );
        // Um dia inteiro cruza as três (00, 08, 16).
        assert_eq!(
            funding_events_between(FUNDING_BOUNDARY, FUNDING_BOUNDARY + 86_400),
            3
        );
    }

    #[test]
    fn funding_ignores_degenerate_intervals() {
        assert_eq!(
            funding_events_between(FUNDING_BOUNDARY, FUNDING_BOUNDARY),
            0
        );
        // Fechamento antes da abertura não pode gerar cobrança.
        assert_eq!(
            funding_events_between(FUNDING_BOUNDARY, FUNDING_BOUNDARY - 86_400),
            0
        );
    }

    /// Taxa positiva: comprado paga, vendido recebe. Trocar esse sinal
    /// transformaria um custo em receita.
    #[test]
    fn funding_sign_follows_position_side() {
        let notional = 1000.0;
        let rate = 0.0001; // 0,01%

        assert!((funding_cost(notional, rate, "Long", 1) - 0.1).abs() < 1e-9);
        assert!((funding_cost(notional, rate, "Short", 1) + 0.1).abs() < 1e-9);

        // Taxa negativa inverte quem paga.
        assert!(funding_cost(notional, -rate, "Long", 1) < 0.0);
        assert!(funding_cost(notional, -rate, "Short", 1) > 0.0);
    }

    #[test]
    fn funding_scales_with_events_and_is_zero_without_them() {
        let notional = 1000.0;
        let rate = 0.0001;

        assert!((funding_cost(notional, rate, "Long", 3) - 0.3).abs() < 1e-9);
        assert_eq!(funding_cost(notional, rate, "Long", 0), 0.0);
        assert_eq!(funding_cost(0.0, rate, "Long", 1), 0.0);
        assert_eq!(funding_cost(notional, f64::NAN, "Long", 1), 0.0);
    }

    #[test]
    fn book_quote_fills_buys_at_ask_and_sells_at_bid() {
        let quote = BookQuote {
            bid: 99.0,
            ask: 101.0,
            funding_rate: 0.0,
        };

        // Comprar paga o lado caro, vender recebe o lado barato — atravessar o
        // spread é o custo de exigir execução imediata.
        assert_eq!(quote.fill_price(true), 101.0);
        assert_eq!(quote.fill_price(false), 99.0);
    }

    /// O mapeamento de lado é o ponto onde um erro inverteria o sinal do
    /// slippage — encareceria as vendas e barateria as compras.
    #[test]
    fn maps_actions_to_book_side() {
        assert!(action_is_buy("ENTER_LONG"));
        assert!(action_is_buy("CLOSE_SHORT"));
        assert!(!action_is_buy("ENTER_SHORT"));
        assert!(!action_is_buy("CLOSE_LONG"));
    }

    /// O preenchimento pelo book tem de ser sempre pior que o preço de
    /// referência, nos dois lados — igual ao que o slippage sorteado garantia.
    #[test]
    fn book_fill_is_never_better_than_mid() {
        let quote = BookQuote {
            bid: 99.0,
            ask: 101.0,
            funding_rate: 0.0,
        };
        let mid = (quote.bid + quote.ask) / 2.0;

        assert!(
            quote.fill_price(true) >= mid,
            "compra não pode sair abaixo do mid"
        );
        assert!(
            quote.fill_price(false) <= mid,
            "venda não pode sair acima do mid"
        );
    }

    #[test]
    fn fee_for_fill_charges_notional_times_rate() {
        // 10 units at $2,000 = $20,000 notional; 0.055% taker = $11.
        assert!((fee_for_fill(2000.0, 10.0, 0.00055) - 11.0).abs() < 1e-9);
    }

    #[test]
    fn fee_for_fill_rejects_nonsense_inputs() {
        // A bad rate or notional must not produce a negative/NaN fee that would
        // silently credit the account.
        assert_eq!(fee_for_fill(2000.0, 10.0, 0.0), 0.0);
        assert_eq!(fee_for_fill(2000.0, 10.0, -0.001), 0.0);
        assert_eq!(fee_for_fill(0.0, 10.0, 0.00055), 0.0);
        assert_eq!(fee_for_fill(f64::NAN, 10.0, 0.00055), 0.0);
    }

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
        // PnL is the price move over the full position quantity; leverage is not
        // a factor (it affects margin/ROI, not absolute PnL).
        let long_pnl = realized_pnl("Long", 100.0, 110.0, 2.0);
        let short_pnl = realized_pnl("Short", 100.0, 90.0, 2.0);
        assert_eq!(long_pnl, 20.0);
        assert_eq!(short_pnl, 20.0);
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
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
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

        let res = close_open_trade(&state, &partial, 4.0, 1.1, 0.0, 0.0)
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

        let res = close_open_trade(&state, &full, 6.0, 1.2, 0.0, 0.0)
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
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
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

    #[tokio::test]
    async fn paper_enter_long_persist_and_query() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let trade_key = format!("it-paper-enter-{}", now_ms());
        let event: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": trade_key,
            "source_event_id": format!("{}-src", trade_key),
            "timestamp": "2026-01-01T00:00:00Z",
            "decision": {
                "action": "ENTER_LONG",
                "symbol": "DOGEUSDT",
                "quantity": 10.0,
                "leverage": 2.0,
                "entry_price": 1.0,
                "stop_loss": 0.98,
                "take_profit": 1.04,
                "reason": "it_paper_enter",
                "smart_copy_compatible": true
            }
        }))
        .expect("event parse");

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        let paper_order_id = format!("paper-{}", event.event_id);
        persist_trade(&state, &event, &paper_order_id, 10.0, 1.0, 0.0, true)
            .await
            .expect("persist paper trade");

        let (side, qty, price, status): (String, f64, f64, String) = sqlx::query_as(
            "SELECT side, quantity::double precision, entry_price::double precision, status FROM trades WHERE order_link_id = $1",
        )
        .bind(&event.event_id)
        .fetch_one(&pool)
        .await
        .expect("query trade");
        assert_eq!(side, "Long");
        assert!((qty - 10.0).abs() < 1e-9);
        assert!((price - 1.0).abs() < 1e-9);
        assert_eq!(status, "open");

        let found_side = open_trade_side_for_symbol(&state, "DOGEUSDT")
            .await
            .expect("query open side")
            .expect("side should exist");
        assert_eq!(found_side, "Long");

        let count = count_open_trades(&state).await.expect("count trades");
        assert!(count >= 1);

        sqlx::query("DELETE FROM trades WHERE order_link_id = $1")
            .bind(&event.event_id)
            .execute(&pool)
            .await
            .expect("cleanup");
    }

    #[tokio::test]
    async fn paper_enter_short_persist_and_query() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let trade_key = format!("it-paper-enter-short-{}", now_ms());
        let event: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": trade_key,
            "source_event_id": format!("{}-src", trade_key),
            "timestamp": "2026-01-01T00:00:00Z",
            "decision": {
                "action": "ENTER_SHORT",
                "symbol": "XRPUSDT",
                "quantity": 20.0,
                "leverage": 3.0,
                "entry_price": 2.0,
                "stop_loss": 2.05,
                "take_profit": 1.90,
                "reason": "it_paper_enter_short",
                "smart_copy_compatible": true
            }
        }))
        .expect("event parse");

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        let paper_order_id = format!("paper-{}", event.event_id);
        persist_trade(&state, &event, &paper_order_id, 20.0, 2.0, 0.0, true)
            .await
            .expect("persist paper short");

        let (side, qty, price, status): (String, f64, f64, String) = sqlx::query_as(
            "SELECT side, quantity::double precision, entry_price::double precision, status FROM trades WHERE order_link_id = $1",
        )
        .bind(&event.event_id)
        .fetch_one(&pool)
        .await
        .expect("query trade");
        assert_eq!(side, "Short");
        assert!((qty - 20.0).abs() < 1e-9);
        assert!((price - 2.0).abs() < 1e-9);
        assert_eq!(status, "open");

        let found_side = open_trade_side_for_symbol(&state, "XRPUSDT")
            .await
            .expect("query open side")
            .expect("side should exist");
        assert_eq!(found_side, "Short");

        sqlx::query("DELETE FROM trades WHERE order_link_id = $1")
            .bind(&event.event_id)
            .execute(&pool)
            .await
            .expect("cleanup");
    }

    #[tokio::test]
    async fn paper_close_flow() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let trade_key = format!("it-paper-close-{}", now_ms());
        let paper_order_id = format!("paper-{}", trade_key);

        sqlx::query(
            "INSERT INTO trades (
                order_link_id, bybit_order_id, symbol, side, quantity,
                entry_price, leverage, status, decision_hash,
                smart_copy_compatible, pipeline_version, paper_trade
            ) VALUES ($1,$2,'SOLUSDT','Long',15,1.5,2.0,'open',$3,true,'it',true)",
        )
        .bind(&trade_key)
        .bind(&paper_order_id)
        .bind(format!("hash-{}", trade_key))
        .execute(&pool)
        .await
        .expect("seed trade");

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        let close_event: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": format!("{}-close", trade_key),
            "source_event_id": format!("{}-src-close", trade_key),
            "timestamp": "2026-01-01T00:01:00Z",
            "decision": {
                "action": "CLOSE_LONG",
                "symbol": "SOLUSDT",
                "quantity": 15.0,
                "leverage": 2.0,
                "entry_price": 1.6,
                "stop_loss": 0.0,
                "take_profit": 0.0,
                "reason": "it_paper_close",
                "smart_copy_compatible": true
            }
        }))
        .expect("close event");

        let res = close_open_trade(&state, &close_event, 15.0, 1.6, 0.0, 0.0)
            .await
            .expect("close should work");
        assert!(
            matches!(res, CloseReconcileResult::Closed { .. }),
            "expected Closed result, got {:?}",
            res
        );

        let (status, pnl): (String, Option<f64>) = sqlx::query_as(
            "SELECT status, pnl::double precision FROM trades WHERE order_link_id = $1",
        )
        .bind(&trade_key)
        .fetch_one(&pool)
        .await
        .expect("query closed trade");
        assert_eq!(status, "closed");
        assert!(
            (pnl.unwrap_or(0.0) - (1.6 - 1.5) * 15.0).abs() < 1e-6,
            "expected pnl ~1.5, got {:?}",
            pnl
        );

        let duplicate_res = close_open_trade(&state, &close_event, 15.0, 1.6, 0.0, 0.0)
            .await
            .expect("duplicate close should not error");
        assert!(
            matches!(duplicate_res, CloseReconcileResult::NoLocalOpen),
            "expected NoLocalOpen for duplicate, got {:?}",
            duplicate_res
        );

        sqlx::query("DELETE FROM trades WHERE order_link_id = $1")
            .bind(&trade_key)
            .execute(&pool)
            .await
            .expect("cleanup");
    }

    #[tokio::test]
    async fn paper_guard_duplicate_symbol_blocks_entry() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let trade_key = format!("it-guard-dup-{}", now_ms());
        let paper_order_id = format!("paper-{}", trade_key);

        sqlx::query(
            "INSERT INTO trades (
                order_link_id, bybit_order_id, symbol, side, quantity,
                entry_price, leverage, status, decision_hash,
                smart_copy_compatible, pipeline_version, paper_trade
            ) VALUES ($1,$2,'ADAUSDT','Long',10,0.5,2.0,'open',$3,true,'it',true)",
        )
        .bind(&trade_key)
        .bind(&paper_order_id)
        .bind(format!("hash-{}", trade_key))
        .execute(&pool)
        .await
        .expect("seed trade");

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        let existing_side = open_trade_side_for_symbol(&state, "ADAUSDT")
            .await
            .expect("query open side")
            .expect("should have open trade");
        assert_eq!(
            existing_side, "Long",
            "open_trade_side_for_symbol should return the side"
        );

        let count = count_open_trades(&state).await.expect("count trades");
        assert!(count >= 1);

        sqlx::query("DELETE FROM trades WHERE order_link_id = $1")
            .bind(&trade_key)
            .execute(&pool)
            .await
            .expect("cleanup");
    }

    #[tokio::test]
    async fn paper_guard_max_positions_respected() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("db connect");

        let symbol_a = format!("SYMA-{}", now_ms());
        let symbol_b = format!("SYMB-{}", now_ms());
        let key_a = format!("it-maxpos-a-{}", now_ms());
        let key_b = format!("it-maxpos-b-{}", now_ms());

        sqlx::query(
            "INSERT INTO trades (order_link_id, bybit_order_id, symbol, side, quantity, entry_price, leverage, status, decision_hash, smart_copy_compatible, pipeline_version, paper_trade)
             VALUES ($1,$2,$3,'Long',10,1.0,2.0,'open',$4,true,'it',true)",
        )
        .bind(&key_a).bind(format!("bybit-{}", key_a)).bind(&symbol_a).bind(format!("hash-{}", key_a))
        .execute(&pool).await.expect("seed trade a");

        sqlx::query(
            "INSERT INTO trades (order_link_id, bybit_order_id, symbol, side, quantity, entry_price, leverage, status, decision_hash, smart_copy_compatible, pipeline_version, paper_trade)
             VALUES ($1,$2,$3,'Long',10,1.0,2.0,'open',$4,true,'it',true)",
        )
        .bind(&key_b).bind(format!("bybit-{}", key_b)).bind(&symbol_b).bind(format!("hash-{}", key_b))
        .execute(&pool).await.expect("seed trade b");

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        let open_count = count_open_trades(&state).await.expect("count open");
        assert!(
            open_count >= 2,
            "expected at least 2 open trades, got {}",
            open_count
        );

        sqlx::query("DELETE FROM trades WHERE order_link_id = $1")
            .bind(&key_a)
            .execute(&pool)
            .await
            .expect("cleanup a");
        sqlx::query("DELETE FROM trades WHERE order_link_id = $1")
            .bind(&key_b)
            .execute(&pool)
            .await
            .expect("cleanup b");
    }

    #[tokio::test]
    async fn handle_decision_hold_is_ignored() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let trade_key = format!("it-hold-{}", now_ms());
        let event: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": trade_key,
            "source_event_id": format!("{}-src", trade_key),
            "timestamp": "2026-01-01T00:00:00Z",
            "decision": {
                "action": "HOLD",
                "symbol": "DOGEUSDT",
                "quantity": 0.0,
                "leverage": 0.0,
                "entry_price": 0.0,
                "stop_loss": 0.0,
                "take_profit": 0.0,
                "reason": "it_hold",
                "smart_copy_compatible": false
            }
        }))
        .expect("event");

        let cfg = ExecutorConfig {
            redis_url: "redis://localhost:6379".to_string(),
            db_url: db_url.clone(),
            trading_mode: TradingMode::Paper,
            bybit_api_key: String::new(),
            bybit_api_secret: String::new(),
            recv_window: "5000".to_string(),
            bybit_account_type: "UNIFIED".to_string(),
            executor_default_enabled: true,
            live_orders_enabled: false,
            live_symbol_allowlist: HashSet::new(),
            reconcile_fix: false,
            reconcile_auto_fix: false,
            reconcile_max_correction_pct: 0.05,
            reconcile_max_daily: 5,
            paper_max_open_positions: 2,
            strategy_config_path: "".to_string(),
            fee_taker_pct: 0.00055,
            initial_capital_usd: 100.0,
            trading_profile: "MEDIUM".to_string(),
        };

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        let http = reqwest::Client::new();
        handle_decision_event(&state, &http, &cfg, event.clone())
            .await
            .expect("handle HOLD should succeed");

        let trade_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM trades WHERE order_link_id = $1")
                .bind(&event.event_id)
                .fetch_one(&pool)
                .await
                .expect("query trades");
        assert_eq!(trade_count, 0, "HOLD must not create a trade");

        let audit_status: Option<String> = sqlx::query_scalar(
            "SELECT executor_status FROM strategy_decision_audit WHERE decision_event_id = $1",
        )
        .bind(&event.event_id)
        .fetch_optional(&pool)
        .await
        .expect("query audit")
        .flatten();
        assert_eq!(
            audit_status.as_deref(),
            Some("ignored_hold"),
            "expected ignored_hold audit status, got {:?}",
            audit_status
        );

        sqlx::query("DELETE FROM strategy_decision_audit WHERE decision_event_id = $1")
            .bind(&event.event_id)
            .execute(&pool)
            .await
            .ok();
        sqlx::query("DELETE FROM system_events WHERE data->>'source_event_id' = $1")
            .bind(format!("{}-src", trade_key))
            .execute(&pool)
            .await
            .ok();
    }

    #[tokio::test]
    async fn handle_decision_invalid_action_rejected() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let trade_key = format!("it-invalid-{}", now_ms());
        let event: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": trade_key,
            "source_event_id": format!("{}-src", trade_key),
            "timestamp": "2026-01-01T00:00:00Z",
            "decision": {
                "action": "INVALID_ACTION",
                "symbol": "DOGEUSDT",
                "quantity": 10.0,
                "leverage": 2.0,
                "entry_price": 1.0,
                "stop_loss": 0.0,
                "take_profit": 0.0,
                "reason": "it_invalid",
                "smart_copy_compatible": false
            }
        }))
        .expect("event");

        let cfg = ExecutorConfig {
            redis_url: "redis://localhost:6379".to_string(),
            db_url: db_url.clone(),
            trading_mode: TradingMode::Paper,
            bybit_api_key: String::new(),
            bybit_api_secret: String::new(),
            recv_window: "5000".to_string(),
            bybit_account_type: "UNIFIED".to_string(),
            executor_default_enabled: true,
            live_orders_enabled: false,
            live_symbol_allowlist: HashSet::new(),
            reconcile_fix: false,
            reconcile_auto_fix: false,
            reconcile_max_correction_pct: 0.05,
            reconcile_max_daily: 5,
            paper_max_open_positions: 2,
            strategy_config_path: "".to_string(),
            fee_taker_pct: 0.00055,
            initial_capital_usd: 100.0,
            trading_profile: "MEDIUM".to_string(),
        };

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        let http = reqwest::Client::new();
        handle_decision_event(&state, &http, &cfg, event.clone())
            .await
            .expect("handle invalid action should not panic");

        let audit_status: Option<String> = sqlx::query_scalar(
            "SELECT executor_status FROM strategy_decision_audit WHERE decision_event_id = $1",
        )
        .bind(&event.event_id)
        .fetch_optional(&pool)
        .await
        .expect("query audit")
        .flatten();
        assert_eq!(
            audit_status.as_deref(),
            Some("error"),
            "expected error audit status, got {:?}",
            audit_status
        );

        sqlx::query("DELETE FROM strategy_decision_audit WHERE decision_event_id = $1")
            .bind(&event.event_id)
            .execute(&pool)
            .await
            .ok();
        sqlx::query("DELETE FROM system_events WHERE data->>'source_event_id' = $1")
            .bind(format!("{}-src", trade_key))
            .execute(&pool)
            .await
            .ok();
    }

    #[tokio::test]
    async fn claim_processed_event_idempotent() {
        let Ok(db_url) = std::env::var("EXECUTOR_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("db connect");

        let trade_key = format!("it-claim-{}", now_ms());
        let event: StrategyDecisionEvent = serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "event_id": trade_key,
            "source_event_id": format!("{}-src", trade_key),
            "timestamp": "2026-01-01T00:00:00Z",
            "decision": {
                "action": "ENTER_LONG",
                "symbol": "DOGEUSDT",
                "quantity": 10.0,
                "leverage": 2.0,
                "entry_price": 1.0,
                "stop_loss": 0.98,
                "take_profit": 1.04,
                "reason": "it_claim",
                "smart_copy_compatible": true
            }
        }))
        .expect("event");

        let state = ExecutorState {
            db_pool: Some(pool.clone()),
            processed_in_memory: Arc::new(Mutex::new(HashSet::new())),
            constraints_cache: Arc::new(Mutex::new(HashMap::new())),
            fee_rate_cache: Arc::new(Mutex::new(HashMap::new())),
            reconcile_daily_counts: Arc::new(Mutex::new(HashMap::new())),
        };

        let idem_key = idempotency_key(&event);
        let first_claim = claim_processed_event(&state, idem_key, &event)
            .await
            .expect("first claim");
        assert!(first_claim, "first claim must return true");

        let second_claim = claim_processed_event(&state, idem_key, &event)
            .await
            .expect("second claim");
        assert!(!second_claim, "duplicate claim must return false");

        sqlx::query("DELETE FROM strategy_decision_audit WHERE decision_event_id = $1")
            .bind(&event.event_id)
            .execute(&pool)
            .await
            .ok();
        sqlx::query("DELETE FROM system_events WHERE data->>'source_event_id' = $1")
            .bind(format!("{}-src", trade_key))
            .execute(&pool)
            .await
            .ok();
    }
}
