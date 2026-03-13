use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::sync::Arc;
use tokio::sync::watch;
use warp::http::StatusCode;
use warp::reply::Json as WarpJson;
use warp::{Filter, Rejection, Reply};

#[derive(Clone)]
struct AppState {
    db_pool: Option<PgPool>,
    trading_mode: String,
    trading_profile: String,
    initial_capital_usd: f64,
    operator_auth_mode: String,
    operator_api_token: Option<String>,
    executor_default_enabled: bool,
    default_max_daily_loss_pct: f64,
    default_max_leverage: f64,
    default_risk_per_trade_pct: f64,
    position_config: PositionConfigStore,
}

#[derive(Serialize)]
struct ApiError {
    error: &'static str,
    message: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    db_connected: bool,
}

#[derive(Serialize)]
struct StatusResponse {
    service: &'static str,
    trading_mode: String,
    trading_profile: String,
    db_connected: bool,
    operator_auth_mode: String,
    operator_controls_enabled: bool,
    risk_status: String,
    critical_reconciliation_events_15m: i64,
    kill_switch: KillSwitchStatus,
    executor: ExecutorControlStatus,
    risk_limits: RiskLimitsStatus,
}

#[derive(Serialize)]
struct KillSwitchStatus {
    enabled: bool,
    reason: Option<String>,
    actor: Option<String>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct ExecutorControlStatus {
    enabled: bool,
    reason: Option<String>,
    actor: Option<String>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct RiskLimitsStatus {
    max_daily_loss_pct: f64,
    max_leverage: f64,
    risk_per_trade_pct: f64,
    reason: Option<String>,
    actor: Option<String>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct PositionItem {
    trade_id: String,
    symbol: String,
    side: String,
    quantity: f64,
    notional_usdt: f64,
    entry_price: f64,
    trailing_stop_activated: bool,
    trailing_stop_peak_price: Option<f64>,
    trailing_stop_final_distance_pct: Option<f64>,
    stop_loss_price: Option<f64>,
    trailing_activation_price: Option<f64>,
    fixed_take_profit_price: Option<f64>,
    break_even_price: Option<f64>,
}

#[derive(Serialize)]
struct PositionsResponse {
    items: Vec<PositionItem>,
}

#[derive(Clone, Default)]
struct PositionConfigStore {
    global: GlobalPositionConfig,
    pairs: HashMap<String, PairPositionConfig>,
}

#[derive(Clone)]
struct GlobalPositionConfig {
    trailing_enabled: bool,
    _trailing_min_move_threshold_pct: f64,
}

impl Default for GlobalPositionConfig {
    fn default() -> Self {
        Self {
            trailing_enabled: true,
            _trailing_min_move_threshold_pct: 0.002,
        }
    }
}

#[derive(Clone)]
struct PairPositionConfig {
    stop_loss_pct: f64,
    take_profit_pct: f64,
    trailing_by_profile: HashMap<String, TrailingProfileConfig>,
    trailing_enabled: Option<bool>,
}

#[derive(Clone)]
struct TrailingProfileConfig {
    activate_after_profit_pct: f64,
    move_to_break_even_at: f64,
}

#[derive(Debug, Deserialize)]
struct PairsFile {
    global: Option<PairsGlobalSection>,
    #[serde(flatten)]
    pairs: HashMap<String, PairFileSection>,
}

#[derive(Debug, Deserialize)]
struct PairsGlobalSection {
    trailing_stop: Option<GlobalTrailingSection>,
}

#[derive(Debug, Deserialize)]
struct GlobalTrailingSection {
    enabled: Option<bool>,
    min_move_threshold_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PairFileSection {
    risk: Option<PairRiskSection>,
    trailing_stop: Option<PairTrailingSection>,
}

#[derive(Debug, Deserialize)]
struct PairRiskSection {
    stop_loss_pct: Option<f64>,
    take_profit_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PairTrailingSection {
    enabled: Option<bool>,
    by_profile: Option<HashMap<String, PairTrailingProfileSection>>,
}

#[derive(Debug, Deserialize)]
struct PairTrailingProfileSection {
    activate_after_profit_pct: Option<f64>,
    move_to_break_even_at: Option<f64>,
}

#[derive(Deserialize)]
struct TradesQuery {
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct EventsQuery {
    limit: Option<u32>,
}

#[derive(Serialize)]
struct TradeItem {
    trade_id: String,
    symbol: String,
    side: String,
    status: String,
    quantity: f64,
    entry_price: f64,
    exit_price: Option<f64>,
    pnl: Option<f64>,
    opened_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct TradesResponse {
    items: Vec<TradeItem>,
}

#[derive(Serialize)]
struct EventItem {
    event_id: String,
    event_type: String,
    severity: String,
    category: Option<String>,
    symbol: Option<String>,
    trade_id: Option<String>,
    data: Value,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
struct EventsResponse {
    items: Vec<EventItem>,
}

#[derive(Serialize)]
struct PerformanceWindow {
    window_start_utc: DateTime<Utc>,
    window_end_utc: DateTime<Utc>,
    total_trades: i64,
    winning_trades: i64,
    win_rate: f64,
    total_pnl: f64,
}

#[derive(Serialize)]
struct PerformanceResponse {
    last_24h: PerformanceWindow,
    last_7d: PerformanceWindow,
    last_30d: PerformanceWindow,
    max_drawdown_30d: Option<f64>,
}

#[derive(Serialize)]
struct RiskKpisResponse {
    rejected_orders_24h: i64,
    open_exposure_usdt: f64,
    realized_pnl_24h: f64,
    critical_events_24h: i64,
    circuit_breaker_triggers_24h: i64,
}

#[derive(Serialize)]
struct BybitPrivateHealthResponse {
    name: &'static str,
    ok: bool,
    status: u16,
    latency_ms: i64,
    url: String,
    error: Option<String>,
    ret_code: Option<i64>,
    ret_msg: Option<String>,
    checked_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct BybitWalletResponse {
    ok: bool,
    status: u16,
    url: String,
    error: Option<String>,
    ret_code: Option<i64>,
    ret_msg: Option<String>,
    checked_at: DateTime<Utc>,
    account_type: String,
    total_equity: Option<f64>,
    wallet_balance: Option<f64>,
    margin_balance: Option<f64>,
    available_balance: Option<f64>,
    unrealized_pnl: Option<f64>,
    initial_margin: Option<f64>,
    maintenance_margin: Option<f64>,
    account_im_rate: Option<f64>,
    account_mm_rate: Option<f64>,
}

#[derive(Serialize)]
struct DailyTradesSummaryResponse {
    ok: bool,
    source: String,
    count: i64,
    window_start_utc: DateTime<Utc>,
    window_end_utc: DateTime<Utc>,
    checked_at: DateTime<Utc>,
    status: u16,
    url: String,
    error: Option<String>,
    ret_code: Option<i64>,
    ret_msg: Option<String>,
}

struct BybitWalletFetchResult {
    checked_at: DateTime<Utc>,
    account_type: String,
    url: String,
    status: u16,
    latency_ms: i64,
    ret_code: Option<i64>,
    ret_msg: Option<String>,
    body: Value,
    error: Option<String>,
}

struct BybitOrderHistoryFetchResult {
    checked_at: DateTime<Utc>,
    url: String,
    status: u16,
    ret_code: Option<i64>,
    ret_msg: Option<String>,
    body: Value,
    error: Option<String>,
}

struct BybitClosedPnlFetchResult {
    status: u16,
    ret_code: Option<i64>,
    ret_msg: Option<String>,
    body: Value,
    error: Option<String>,
}

struct BybitPositionFetchResult {
    status: u16,
    ret_code: Option<i64>,
    ret_msg: Option<String>,
    body: Value,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

    fn as_status_label(self) -> &'static str {
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

    fn exchange_env_label(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Testnet => "testnet",
            Self::Mainnet => "mainnet",
        }
    }

    fn uses_simulated_wallet(self) -> bool {
        matches!(self, Self::Paper)
    }
}

#[derive(Deserialize)]
struct KillSwitchRequest {
    enabled: bool,
    reason: Option<String>,
}

#[derive(Serialize)]
struct KillSwitchResponse {
    updated: bool,
    kill_switch: KillSwitchStatus,
}

#[derive(Deserialize)]
struct ExecutorControlRequest {
    enabled: bool,
    reason: Option<String>,
}

#[derive(Serialize)]
struct ExecutorControlResponse {
    updated: bool,
    executor: ExecutorControlStatus,
}

#[derive(Deserialize)]
struct RiskLimitsRequest {
    max_daily_loss_pct: Option<f64>,
    max_leverage: Option<f64>,
    risk_per_trade_pct: Option<f64>,
    reason: Option<String>,
}

#[derive(Serialize)]
struct RiskLimitsResponse {
    updated: bool,
    risk_limits: RiskLimitsStatus,
}

#[derive(Serialize)]
struct ControlStateResponse {
    operator_auth_mode: String,
    operator_controls_enabled: bool,
    kill_switch: KillSwitchStatus,
    executor: ExecutorControlStatus,
    risk_limits: RiskLimitsStatus,
}

fn resolve_database_url() -> Option<String> {
    if let Ok(v) = std::env::var("DATABASE_URL") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }

    let host = std::env::var("DB_HOST").ok()?;
    let port = std::env::var("DB_PORT")
        .ok()
        .unwrap_or_else(|| "5432".to_string());
    let db = std::env::var("DB_NAME").ok()?;
    let user = std::env::var("DB_USER").ok()?;
    let pass = std::env::var("DB_PASSWORD").ok()?;

    Some(format!(
        "postgresql://{}:{}@{}:{}/{}",
        user, pass, host, port, db
    ))
}

fn load_position_config(path: &str) -> PositionConfigStore {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => {
            eprintln!("api: failed to read position config '{}': {}", path, err);
            return PositionConfigStore::default();
        }
    };

    let parsed: PairsFile = match serde_yaml::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("api: failed to parse position config '{}': {}", path, err);
            return PositionConfigStore::default();
        }
    };

    let global = GlobalPositionConfig {
        trailing_enabled: parsed
            .global
            .as_ref()
            .and_then(|g| g.trailing_stop.as_ref())
            .and_then(|t| t.enabled)
            .unwrap_or(true),
        _trailing_min_move_threshold_pct: parsed
            .global
            .as_ref()
            .and_then(|g| g.trailing_stop.as_ref())
            .and_then(|t| t.min_move_threshold_pct)
            .unwrap_or(0.002),
    };

    let mut pairs = HashMap::new();
    for (symbol, pair) in parsed.pairs {
        let Some(risk) = pair.risk else {
            continue;
        };
        let stop_loss_pct = risk.stop_loss_pct.unwrap_or(0.015);
        let take_profit_pct = risk.take_profit_pct.unwrap_or(0.03);
        let trailing_enabled = pair.trailing_stop.as_ref().and_then(|t| t.enabled);
        let trailing_by_profile = pair
            .trailing_stop
            .and_then(|t| t.by_profile)
            .unwrap_or_default()
            .into_iter()
            .map(|(profile, cfg)| {
                (
                    profile.to_uppercase(),
                    TrailingProfileConfig {
                        activate_after_profit_pct: cfg.activate_after_profit_pct.unwrap_or(0.015),
                        move_to_break_even_at: cfg.move_to_break_even_at.unwrap_or(0.02),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        pairs.insert(
            symbol.to_uppercase(),
            PairPositionConfig {
                stop_loss_pct,
                take_profit_pct,
                trailing_by_profile,
                trailing_enabled,
            },
        );
    }

    PositionConfigStore { global, pairs }
}

fn default_trailing_profile() -> TrailingProfileConfig {
    TrailingProfileConfig {
        activate_after_profit_pct: 0.015,
        move_to_break_even_at: 0.02,
    }
}

fn resolve_position_triggers(
    state: &AppState,
    symbol: &str,
    side: &str,
    entry_price: f64,
) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    if entry_price <= 0.0 {
        return (None, None, None, None);
    }

    let pair_cfg = state.position_config.pairs.get(&symbol.to_uppercase());
    let stop_loss_pct = pair_cfg.map(|p| p.stop_loss_pct).unwrap_or(0.015);
    let take_profit_pct = pair_cfg.map(|p| p.take_profit_pct).unwrap_or(0.03);
    let trailing_enabled = pair_cfg
        .and_then(|p| p.trailing_enabled)
        .unwrap_or(state.position_config.global.trailing_enabled);
    let trailing_profile = pair_cfg
        .and_then(|p| {
            p.trailing_by_profile
                .get(&state.trading_profile.to_uppercase())
                .cloned()
        })
        .unwrap_or_else(default_trailing_profile);

    let is_long = side.eq_ignore_ascii_case("long");
    let stop_loss_price = if is_long {
        entry_price * (1.0 - stop_loss_pct)
    } else {
        entry_price * (1.0 + stop_loss_pct)
    };
    let fixed_take_profit_price = if is_long {
        entry_price * (1.0 + take_profit_pct)
    } else {
        entry_price * (1.0 - take_profit_pct)
    };

    let trailing_activation_price = if trailing_enabled {
        Some(if is_long {
            entry_price * (1.0 + trailing_profile.activate_after_profit_pct)
        } else {
            entry_price * (1.0 - trailing_profile.activate_after_profit_pct)
        })
    } else {
        None
    };

    let break_even_price = if trailing_enabled {
        Some(if is_long {
            entry_price * (1.0 + trailing_profile.move_to_break_even_at)
        } else {
            entry_price * (1.0 - trailing_profile.move_to_break_even_at)
        })
    } else {
        None
    };

    (
        Some(stop_loss_price),
        trailing_activation_price,
        Some(fixed_take_profit_price),
        break_even_price,
    )
}

fn read_non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn read_bybit_credential(key_name: &str, secret_name: &str) -> (Option<String>, Option<String>) {
    let mode = TradingMode::from_env();
    let scoped = match mode {
        TradingMode::Testnet => (
            read_non_empty_env("BYBIT_TESTNET_API_KEY"),
            read_non_empty_env("BYBIT_TESTNET_API_SECRET"),
        ),
        TradingMode::Paper | TradingMode::Mainnet => (
            read_non_empty_env("BYBIT_MAINNET_API_KEY"),
            read_non_empty_env("BYBIT_MAINNET_API_SECRET"),
        ),
    };

    (
        scoped.0.or_else(|| read_non_empty_env(key_name)),
        scoped.1.or_else(|| read_non_empty_env(secret_name)),
    )
}

fn read_f64_env(name: &str, default_value: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default_value)
}

fn read_bool_env(name: &str, default_value: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default_value)
}

fn resolve_bybit_rest_url() -> String {
    if let Some(url) = read_non_empty_env("BYBIT_REST_URL") {
        return url.trim_end_matches('/').to_string();
    }

    let bybit_env = TradingMode::from_env().bybit_env().to_string();
    if bybit_env == "mainnet" {
        "https://api.bybit.com".to_string()
    } else {
        "https://api-testnet.bybit.com".to_string()
    }
}

fn clamp_limit(limit: Option<u32>, default_value: u32) -> i64 {
    let raw = limit.unwrap_or(default_value);
    raw.clamp(1, 200) as i64
}

fn round6(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}

fn json_ok<T: Serialize>(payload: &T) -> warp::reply::WithStatus<WarpJson> {
    warp::reply::with_status(warp::reply::json(payload), StatusCode::OK)
}

fn json_err(
    code: StatusCode,
    error: &'static str,
    message: impl Into<String>,
) -> warp::reply::WithStatus<WarpJson> {
    let body = ApiError {
        error,
        message: message.into(),
    };
    warp::reply::with_status(warp::reply::json(&body), code)
}

fn with_state(
    state: Arc<AppState>,
) -> impl Filter<Extract = (Arc<AppState>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

fn ensure_operator_token(
    state: &AppState,
    token_header: Option<&str>,
) -> Result<(), warp::reply::WithStatus<WarpJson>> {
    if !state.operator_auth_mode.eq_ignore_ascii_case("token") {
        return Err(json_err(
            StatusCode::FORBIDDEN,
            "auth_not_configured",
            "operator auth mode is not configured for token controls",
        ));
    }

    let Some(configured_token) = &state.operator_api_token else {
        return Err(json_err(
            StatusCode::FORBIDDEN,
            "auth_not_configured",
            "operator control auth is not configured",
        ));
    };

    if token_header != Some(configured_token.as_str()) {
        return Err(json_err(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "missing or invalid operator token",
        ));
    }

    Ok(())
}

fn validate_risk_limits(
    max_daily_loss_pct: f64,
    max_leverage: f64,
    risk_per_trade_pct: f64,
) -> Result<(), String> {
    if !(0.0..=100.0).contains(&max_daily_loss_pct) {
        return Err("max_daily_loss_pct must be between 0 and 100".to_string());
    }
    if !(1.0..=50.0).contains(&max_leverage) {
        return Err("max_leverage must be between 1 and 50".to_string());
    }
    if !(0.0..=100.0).contains(&risk_per_trade_pct) {
        return Err("risk_per_trade_pct must be between 0 and 100".to_string());
    }
    Ok(())
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    if err.is_not_found() {
        return Ok(json_err(
            StatusCode::NOT_FOUND,
            "not_found",
            "route not found",
        ));
    }

    if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        return Ok(json_err(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "method not allowed",
        ));
    }

    if let Some(body_err) = err.find::<warp::filters::body::BodyDeserializeError>() {
        return Ok(json_err(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            format!("invalid request body: {}", body_err),
        ));
    }

    Ok(json_err(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        format!("unhandled rejection: {:?}", err),
    ))
}

async fn fetch_kill_switch_status(pool: &PgPool) -> Result<KillSwitchStatus, sqlx::Error> {
    let row = sqlx::query_as::<
        _,
        (
            Option<bool>,
            Option<String>,
            Option<String>,
            Option<DateTime<Utc>>,
        ),
    >(
        "SELECT
             (data->>'enabled')::boolean,
             data->>'reason',
             data->>'actor',
             timestamp
         FROM system_events
         WHERE event_type = 'api_kill_switch_set'
         ORDER BY timestamp DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    if let Some((enabled, reason, actor, updated_at)) = row {
        Ok(KillSwitchStatus {
            enabled: enabled.unwrap_or(false),
            reason,
            actor,
            updated_at,
        })
    } else {
        Ok(KillSwitchStatus {
            enabled: false,
            reason: None,
            actor: None,
            updated_at: None,
        })
    }
}

async fn fetch_executor_status(
    pool: &PgPool,
    default_enabled: bool,
) -> Result<ExecutorControlStatus, sqlx::Error> {
    let row = sqlx::query_as::<
        _,
        (
            Option<bool>,
            Option<String>,
            Option<String>,
            Option<DateTime<Utc>>,
        ),
    >(
        "SELECT
             (data->>'enabled')::boolean,
             data->>'reason',
             data->>'actor',
             timestamp
         FROM system_events
         WHERE event_type = 'api_executor_state_set'
         ORDER BY timestamp DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    if let Some((enabled, reason, actor, updated_at)) = row {
        Ok(ExecutorControlStatus {
            enabled: enabled.unwrap_or(default_enabled),
            reason,
            actor,
            updated_at,
        })
    } else {
        Ok(ExecutorControlStatus {
            enabled: default_enabled,
            reason: None,
            actor: None,
            updated_at: None,
        })
    }
}

async fn fetch_risk_limits_status(
    pool: &PgPool,
    defaults: &AppState,
) -> Result<RiskLimitsStatus, sqlx::Error> {
    let row = sqlx::query_as::<
        _,
        (
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<String>,
            Option<String>,
            Option<DateTime<Utc>>,
        ),
    >(
        "SELECT
             (data->>'max_daily_loss_pct')::double precision,
             (data->>'max_leverage')::double precision,
             (data->>'risk_per_trade_pct')::double precision,
             data->>'reason',
             data->>'actor',
             timestamp
         FROM system_events
         WHERE event_type = 'api_risk_limits_set'
         ORDER BY timestamp DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    if let Some((max_daily_loss_pct, max_leverage, risk_per_trade_pct, reason, actor, updated_at)) =
        row
    {
        Ok(RiskLimitsStatus {
            max_daily_loss_pct: round6(
                max_daily_loss_pct.unwrap_or(defaults.default_max_daily_loss_pct),
            ),
            max_leverage: round6(max_leverage.unwrap_or(defaults.default_max_leverage)),
            risk_per_trade_pct: round6(
                risk_per_trade_pct.unwrap_or(defaults.default_risk_per_trade_pct),
            ),
            reason,
            actor,
            updated_at,
        })
    } else {
        Ok(RiskLimitsStatus {
            max_daily_loss_pct: round6(defaults.default_max_daily_loss_pct),
            max_leverage: round6(defaults.default_max_leverage),
            risk_per_trade_pct: round6(defaults.default_risk_per_trade_pct),
            reason: None,
            actor: None,
            updated_at: None,
        })
    }
}

async fn fetch_critical_recon_15m(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)::bigint
         FROM system_events
         WHERE event_type = 'reconciliation_cycle'
           AND severity IN ('error', 'critical')
           AND timestamp >= NOW() - INTERVAL '15 minutes'",
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}

async fn health_handler(state: Arc<AppState>) -> impl Reply {
    let payload = HealthResponse {
        status: "ok",
        db_connected: state.db_pool.is_some(),
    };
    json_ok(&payload)
}

async fn status_handler(state: Arc<AppState>) -> impl Reply {
    let mut kill_switch = KillSwitchStatus {
        enabled: false,
        reason: None,
        actor: None,
        updated_at: None,
    };
    let mut executor = ExecutorControlStatus {
        enabled: state.executor_default_enabled,
        reason: None,
        actor: None,
        updated_at: None,
    };
    let mut risk_limits = RiskLimitsStatus {
        max_daily_loss_pct: round6(state.default_max_daily_loss_pct),
        max_leverage: round6(state.default_max_leverage),
        risk_per_trade_pct: round6(state.default_risk_per_trade_pct),
        reason: None,
        actor: None,
        updated_at: None,
    };
    let mut critical_recon = 0_i64;

    if let Some(pool) = &state.db_pool {
        if let Ok(v) = fetch_kill_switch_status(pool).await {
            kill_switch = v;
        }
        if let Ok(v) = fetch_executor_status(pool, state.executor_default_enabled).await {
            executor = v;
        }
        if let Ok(v) = fetch_risk_limits_status(pool, &state).await {
            risk_limits = v;
        }
        if let Ok(v) = fetch_critical_recon_15m(pool).await {
            critical_recon = v;
        }
    }

    let risk_status = if kill_switch.enabled {
        "halted".to_string()
    } else if critical_recon > 0 {
        "elevated".to_string()
    } else {
        "normal".to_string()
    };

    let payload = StatusResponse {
        service: "viper-api",
        trading_mode: state.trading_mode.clone(),
        trading_profile: state.trading_profile.clone(),
        db_connected: state.db_pool.is_some(),
        operator_auth_mode: state.operator_auth_mode.clone(),
        operator_controls_enabled: state.operator_api_token.is_some()
            && state.operator_auth_mode.eq_ignore_ascii_case("token"),
        risk_status,
        critical_reconciliation_events_15m: critical_recon,
        kill_switch,
        executor,
        risk_limits,
    };

    json_ok(&payload)
}

async fn positions_handler(state: Arc<AppState>) -> impl Reply {
    if TradingMode::from_env().uses_simulated_wallet() {
        return build_paper_positions_response(state).await;
    }

    build_exchange_positions_response(state).await
}

async fn build_paper_positions_response(state: Arc<AppState>) -> warp::reply::WithStatus<WarpJson> {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            f64,
            f64,
            bool,
            Option<f64>,
            Option<f64>,
        ),
    >(
        "SELECT
             trade_id::text,
             symbol,
             side,
             COALESCE(quantity::double precision, 0),
             COALESCE((quantity * entry_price)::double precision, 0),
             COALESCE(trailing_stop_activated, false),
             trailing_stop_peak_price::double precision,
             trailing_stop_final_distance_pct::double precision
         FROM trades
         WHERE status = 'open'
         ORDER BY opened_at DESC",
    )
    .fetch_all(pool)
    .await;

    match rows {
        Ok(rows) => {
            let items = rows
                .into_iter()
                .map(
                    |(
                        trade_id,
                        symbol,
                        side,
                        quantity,
                        notional_usdt,
                        trailing_stop_activated,
                        trailing_stop_peak_price,
                        trailing_stop_final_distance_pct,
                    )| {
                        let entry_price = if quantity > 0.0 {
                            notional_usdt / quantity
                        } else {
                            0.0
                        };
                        let (
                            stop_loss_price,
                            trailing_activation_price,
                            fixed_take_profit_price,
                            break_even_price,
                        ) = resolve_position_triggers(state.as_ref(), &symbol, &side, entry_price);

                        PositionItem {
                            trade_id,
                            symbol,
                            side,
                            quantity,
                            notional_usdt,
                            entry_price,
                            trailing_stop_activated,
                            trailing_stop_peak_price,
                            trailing_stop_final_distance_pct,
                            stop_loss_price,
                            trailing_activation_price,
                            fixed_take_profit_price,
                            break_even_price,
                        }
                    },
                )
                .collect();
            json_ok(&PositionsResponse { items })
        }
        Err(err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "query_failed",
            format!("failed to fetch positions: {}", err),
        ),
    }
}

async fn build_exchange_positions_response(state: Arc<AppState>) -> warp::reply::WithStatus<WarpJson> {
    match fetch_bybit_positions().await {
        Ok(items) => {
            let positions = items
                .into_iter()
                .filter_map(|item| build_position_item_from_bybit(state.as_ref(), &item))
                .collect::<Vec<_>>();
            json_ok(&PositionsResponse { items: positions })
        }
        Err(message) => json_err(
            StatusCode::BAD_GATEWAY,
            "exchange_fetch_failed",
            format!("failed to fetch bybit positions: {}", message),
        ),
    }
}

async fn trades_handler(query: TradesQuery, state: Arc<AppState>) -> impl Reply {
    if TradingMode::from_env().uses_simulated_wallet() {
        return build_paper_trades_response(query, state).await;
    }

    build_exchange_trades_response(query).await
}

async fn build_paper_trades_response(query: TradesQuery, state: Arc<AppState>) -> warp::reply::WithStatus<WarpJson> {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let limit = clamp_limit(query.limit, 50);

    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            f64,
            f64,
            Option<f64>,
            Option<f64>,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
        ),
    >(
        "SELECT
             trade_id::text,
             symbol,
             side,
             status,
             COALESCE(quantity::double precision, 0),
             COALESCE(entry_price::double precision, 0),
             exit_price::double precision,
             pnl::double precision,
             opened_at,
             closed_at
         FROM trades
         ORDER BY opened_at DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await;

    match rows {
        Ok(rows) => {
            let items = rows
                .into_iter()
                .map(
                    |(
                        trade_id,
                        symbol,
                        side,
                        status,
                        quantity,
                        entry_price,
                        exit_price,
                        pnl,
                        opened_at,
                        closed_at,
                    )| TradeItem {
                        trade_id,
                        symbol,
                        side,
                        status,
                        quantity,
                        entry_price,
                        exit_price,
                        pnl,
                        opened_at,
                        closed_at,
                    },
                )
                .collect();
            json_ok(&TradesResponse { items })
        }
        Err(err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "query_failed",
            format!("failed to fetch trades: {}", err),
        ),
    }
}

async fn build_exchange_trades_response(query: TradesQuery) -> warp::reply::WithStatus<WarpJson> {
    let limit = clamp_limit(query.limit, 20) as usize;
    let window_end_utc = Utc::now();
    let window_start_utc = window_end_utc - ChronoDuration::days(30);

    match fetch_bybit_closed_pnl_between(window_start_utc, window_end_utc, limit.max(50)).await {
        Ok(items) => {
            let mut trades = items
                .into_iter()
                .filter_map(|item| build_trade_item_from_closed_pnl(&item))
                .collect::<Vec<_>>();
            trades.sort_by(|a, b| b.closed_at.unwrap_or(b.opened_at).cmp(&a.closed_at.unwrap_or(a.opened_at)));
            trades.truncate(limit);
            json_ok(&TradesResponse { items: trades })
        }
        Err(message) => json_err(
            StatusCode::BAD_GATEWAY,
            "exchange_fetch_failed",
            format!("failed to fetch bybit closed pnl: {}", message),
        ),
    }
}

async fn daily_trades_summary_handler(state: Arc<AppState>) -> impl Reply {
    let window_end_utc = Utc::now();
    let window_start_utc = start_of_day_utc(window_end_utc);

    if TradingMode::from_env().uses_simulated_wallet() {
        return json_ok(&build_paper_daily_trades_summary(&state, window_start_utc, window_end_utc).await);
    }

    let result = fetch_bybit_order_history_today(window_start_utc, window_end_utc).await;
    let count = extract_bybit_today_trade_count(&result.body);
    let ok = result.status == 200 && result.ret_code == Some(0) && result.error.is_none();

    json_ok(&DailyTradesSummaryResponse {
        ok,
        source: format!(
            "bybit-{}-order-history",
            TradingMode::from_env().exchange_env_label()
        ),
        count,
        window_start_utc,
        window_end_utc,
        checked_at: result.checked_at,
        status: result.status,
        url: result.url,
        error: result.error,
        ret_code: result.ret_code,
        ret_msg: result.ret_msg,
    })
}

async fn events_handler(query: EventsQuery, state: Arc<AppState>) -> impl Reply {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let limit = clamp_limit(query.limit, 50);

    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Json<Value>,
            DateTime<Utc>,
        ),
    >(
        "SELECT
             event_id::text,
             event_type,
             severity,
             category,
             symbol,
             trade_id::text,
             data,
             timestamp
         FROM system_events
         ORDER BY timestamp DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await;

    match rows {
        Ok(rows) => {
            let items = rows
                .into_iter()
                .map(
                    |(
                        event_id,
                        event_type,
                        severity,
                        category,
                        symbol,
                        trade_id,
                        data,
                        timestamp,
                    )| {
                        EventItem {
                            event_id,
                            event_type,
                            severity,
                            category,
                            symbol,
                            trade_id,
                            data: data.0,
                            timestamp,
                        }
                    },
                )
                .collect();
            json_ok(&EventsResponse { items })
        }
        Err(err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "query_failed",
            format!("failed to fetch events: {}", err),
        ),
    }
}

async fn fetch_reference_now(pool: &PgPool) -> Result<DateTime<Utc>, sqlx::Error> {
    let now = sqlx::query_scalar::<_, DateTime<Utc>>("SELECT date_trunc('second', NOW())")
        .fetch_one(pool)
        .await?;
    Ok(now)
}

async fn fetch_window_between(
    pool: &PgPool,
    window_start_utc: DateTime<Utc>,
    window_end_utc: DateTime<Utc>,
) -> Result<PerformanceWindow, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64, i64, f64)>(
        "SELECT
             COUNT(*)::bigint,
             COUNT(*) FILTER (WHERE COALESCE(pnl, 0) > 0)::bigint,
             COALESCE(SUM(COALESCE(pnl, 0))::double precision, 0)
         FROM trades
         WHERE status = 'closed'
           AND closed_at IS NOT NULL
           AND closed_at >= $1
           AND closed_at < $2",
    )
    .bind(window_start_utc)
    .bind(window_end_utc)
    .fetch_one(pool)
    .await?;

    let (total_trades, winning_trades, total_pnl) = row;
    let win_rate = if total_trades > 0 {
        winning_trades as f64 / total_trades as f64
    } else {
        0.0
    };

    Ok(PerformanceWindow {
        window_start_utc,
        window_end_utc,
        total_trades,
        winning_trades,
        win_rate: round6(win_rate),
        total_pnl: round6(total_pnl),
    })
}

async fn performance_handler(state: Arc<AppState>) -> impl Reply {
    if TradingMode::from_env().uses_simulated_wallet() {
        return build_paper_performance_response(state).await;
    }

    build_exchange_performance_response().await
}

async fn build_paper_performance_response(state: Arc<AppState>) -> warp::reply::WithStatus<WarpJson> {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let window_end_utc = match fetch_reference_now(pool).await {
        Ok(v) => v,
        Err(err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "query_failed",
                format!("failed to fetch reference time: {}", err),
            )
        }
    };

    let last_24h = match fetch_window_between(
        pool,
        window_end_utc - ChronoDuration::hours(24),
        window_end_utc,
    )
    .await
    {
        Ok(v) => v,
        Err(err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "query_failed",
                format!("failed to fetch 24h performance: {}", err),
            )
        }
    };

    let last_7d = match fetch_window_between(
        pool,
        window_end_utc - ChronoDuration::days(7),
        window_end_utc,
    )
    .await
    {
        Ok(v) => v,
        Err(err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "query_failed",
                format!("failed to fetch 7d performance: {}", err),
            )
        }
    };

    let last_30d = match fetch_window_between(
        pool,
        window_end_utc - ChronoDuration::days(30),
        window_end_utc,
    )
    .await
    {
        Ok(v) => v,
        Err(err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "query_failed",
                format!("failed to fetch 30d performance: {}", err),
            )
        }
    };

    let end_date: NaiveDate = window_end_utc.date_naive();
    let start_date: NaiveDate = end_date - ChronoDuration::days(30);

    let max_drawdown_30d = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT MAX(max_drawdown)::double precision
         FROM daily_metrics
         WHERE date >= $1
           AND date <= $2",
    )
    .bind(start_date)
    .bind(end_date)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .flatten()
    .map(round6);

    let payload = PerformanceResponse {
        last_24h,
        last_7d,
        last_30d,
        max_drawdown_30d,
    };

    json_ok(&payload)
}

async fn build_exchange_performance_response() -> warp::reply::WithStatus<WarpJson> {
    let window_end_utc = Utc::now();
    let window_start_utc = window_end_utc - ChronoDuration::days(30);
    let items = match fetch_bybit_closed_pnl_between(window_start_utc, window_end_utc, 200).await {
        Ok(items) => items,
        Err(message) => {
            return json_err(
                StatusCode::BAD_GATEWAY,
                "exchange_fetch_failed",
                format!("failed to fetch bybit closed pnl: {}", message),
            )
        }
    };

    let payload = PerformanceResponse {
        last_24h: summarize_closed_pnl_window(&items, window_end_utc - ChronoDuration::hours(24), window_end_utc),
        last_7d: summarize_closed_pnl_window(&items, window_end_utc - ChronoDuration::days(7), window_end_utc),
        last_30d: summarize_closed_pnl_window(&items, window_start_utc, window_end_utc),
        max_drawdown_30d: None,
    };

    json_ok(&payload)
}

async fn risk_kpis_handler(state: Arc<AppState>) -> impl Reply {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let rejected_orders_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)::bigint
         FROM trades
         WHERE status = 'rejected'
           AND opened_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let open_exposure_usdt = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT COALESCE(SUM(quantity * entry_price)::double precision, 0)
         FROM trades
         WHERE status = 'open'",
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(0.0);

    let realized_pnl_24h = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT COALESCE(SUM(COALESCE(pnl, 0))::double precision, 0)
         FROM trades
         WHERE status = 'closed'
           AND closed_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(0.0);

    let critical_events_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)::bigint
         FROM system_events
         WHERE severity IN ('error', 'critical')
           AND timestamp >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let circuit_breaker_triggers_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)::bigint
         FROM circuit_breaker_events
         WHERE activated_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    json_ok(&RiskKpisResponse {
        rejected_orders_24h,
        open_exposure_usdt: round6(open_exposure_usdt),
        realized_pnl_24h: round6(realized_pnl_24h),
        critical_events_24h,
        circuit_breaker_triggers_24h,
    })
}

async fn bybit_private_health_handler(_state: Arc<AppState>) -> impl Reply {
    if TradingMode::from_env().uses_simulated_wallet() {
        return json_ok(&BybitPrivateHealthResponse {
            name: "bybit-private",
            ok: true,
            status: 200,
            latency_ms: 0,
            url: format!(
                "{}/v5/account/wallet-balance?accountType={}",
                resolve_bybit_rest_url(),
                read_non_empty_env("BYBIT_ACCOUNT_TYPE").unwrap_or_else(|| "UNIFIED".to_string())
            ),
            error: None,
            ret_code: Some(0),
            ret_msg: Some("paper mode: database-simulated wallet".to_string()),
            checked_at: Utc::now(),
        });
    }

    let result = fetch_bybit_wallet_snapshot().await;
    let ok = result.status == 200 && result.ret_code == Some(0) && result.error.is_none();
    json_ok(&BybitPrivateHealthResponse {
        name: "bybit-private",
        ok,
        status: result.status,
        latency_ms: result.latency_ms,
        url: result.url,
        error: if ok {
            None
        } else {
            Some(result.error.unwrap_or_else(|| {
                format!(
                    "retCode={} retMsg={}",
                    result
                        .ret_code
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    result
                        .ret_msg
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string())
                )
            }))
        },
        ret_code: result.ret_code,
        ret_msg: result.ret_msg,
        checked_at: result.checked_at,
    })
}

async fn bybit_wallet_handler(state: Arc<AppState>) -> impl Reply {
    if TradingMode::from_env().uses_simulated_wallet() {
        return json_ok(&build_paper_wallet_response(&state).await);
    }

    let result = fetch_bybit_wallet_snapshot().await;
    let wallet = extract_wallet_summary(&result.body);
    let ok = result.status == 200 && result.ret_code == Some(0) && result.error.is_none();

    json_ok(&BybitWalletResponse {
        ok,
        status: result.status,
        url: result.url,
        error: result.error,
        ret_code: result.ret_code,
        ret_msg: result.ret_msg,
        checked_at: result.checked_at,
        account_type: result.account_type,
        total_equity: wallet.get("totalEquity").and_then(json_number),
        wallet_balance: wallet.get("totalWalletBalance").and_then(json_number),
        margin_balance: wallet.get("totalMarginBalance").and_then(json_number),
        available_balance: wallet.get("totalAvailableBalance").and_then(json_number),
        unrealized_pnl: wallet.get("totalPerpUPL").and_then(json_number),
        initial_margin: wallet.get("totalInitialMargin").and_then(json_number),
        maintenance_margin: wallet.get("totalMaintenanceMargin").and_then(json_number),
        account_im_rate: wallet.get("accountIMRate").and_then(json_number),
        account_mm_rate: wallet.get("accountMMRate").and_then(json_number),
    })
}

async fn build_paper_wallet_response(state: &AppState) -> BybitWalletResponse {
    let checked_at = Utc::now();
    let url = "paper://database-simulated-wallet".to_string();

    let Some(pool) = &state.db_pool else {
        return BybitWalletResponse {
            ok: false,
            status: 0,
            url,
            error: Some("database unavailable for paper wallet simulation".to_string()),
            ret_code: None,
            ret_msg: None,
            checked_at,
            account_type: "PAPER".to_string(),
            total_equity: None,
            wallet_balance: None,
            margin_balance: None,
            available_balance: None,
            unrealized_pnl: None,
            initial_margin: None,
            maintenance_margin: None,
            account_im_rate: None,
            account_mm_rate: None,
        };
    };

    let (realized_pnl, fees, funding_paid) = sqlx::query_as::<_, (f64, f64, f64)>(
        "SELECT
            COALESCE(SUM(pnl), 0)::double precision,
            COALESCE(SUM(fees), 0)::double precision,
            COALESCE(SUM(funding_paid), 0)::double precision
         FROM trades
         WHERE paper_trade = TRUE
           AND status <> 'open'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0.0, 0.0, 0.0));

    let initial_margin = sqlx::query_scalar::<_, f64>(
        "SELECT
            COALESCE(SUM((quantity * entry_price) / NULLIF(leverage, 0)), 0)::double precision
         FROM trades
         WHERE paper_trade = TRUE
           AND status = 'open'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0.0);

    let maintenance_margin = initial_margin * 0.5;
    let unrealized_pnl = 0.0;
    let wallet_balance = state.initial_capital_usd + realized_pnl - fees - funding_paid;
    let margin_balance = wallet_balance;
    let available_balance = (margin_balance - initial_margin).max(0.0);
    let total_equity = margin_balance + unrealized_pnl;
    let account_im_rate = if total_equity > 0.0 {
        Some(round6(initial_margin / total_equity))
    } else {
        Some(0.0)
    };
    let account_mm_rate = if total_equity > 0.0 {
        Some(round6(maintenance_margin / total_equity))
    } else {
        Some(0.0)
    };

    BybitWalletResponse {
        ok: true,
        status: 200,
        url,
        error: None,
        ret_code: Some(0),
        ret_msg: Some("paper mode: wallet simulated from database".to_string()),
        checked_at,
        account_type: "PAPER".to_string(),
        total_equity: Some(round6(total_equity)),
        wallet_balance: Some(round6(wallet_balance)),
        margin_balance: Some(round6(margin_balance)),
        available_balance: Some(round6(available_balance)),
        unrealized_pnl: Some(round6(unrealized_pnl)),
        initial_margin: Some(round6(initial_margin)),
        maintenance_margin: Some(round6(maintenance_margin)),
        account_im_rate,
        account_mm_rate,
    }
}

async fn fetch_bybit_wallet_snapshot() -> BybitWalletFetchResult {
    let checked_at = Utc::now();
    let bybit_url = resolve_bybit_rest_url();
    let recv_window = read_non_empty_env("BYBIT_RECV_WINDOW").unwrap_or_else(|| "5000".to_string());
    let account_type =
        read_non_empty_env("BYBIT_ACCOUNT_TYPE").unwrap_or_else(|| "UNIFIED".to_string());
    let url = format!(
        "{}/v5/account/wallet-balance?accountType={}",
        bybit_url, account_type
    );

    let (api_key, api_secret) = read_bybit_credential("BYBIT_API_KEY", "BYBIT_API_SECRET");

    let Some(api_key) = api_key else {
        return BybitWalletFetchResult {
            checked_at,
            account_type,
            url,
            status: 0,
            latency_ms: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some("missing BYBIT_API_KEY in api runtime".to_string()),
        };
    };
    let Some(api_secret) = api_secret else {
        return BybitWalletFetchResult {
            checked_at,
            account_type,
            url,
            status: 0,
            latency_ms: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some("missing BYBIT_API_SECRET in api runtime".to_string()),
        };
    };

    let timestamp = Utc::now().timestamp_millis().to_string();
    let query_string = format!("accountType={}", account_type);
    let payload = format!("{}{}{}{}", timestamp, api_key, recv_window, query_string);
    let mut mac = match Hmac::<Sha256>::new_from_slice(api_secret.as_bytes()) {
        Ok(v) => v,
        Err(err) => {
            return BybitWalletFetchResult {
                checked_at,
                account_type,
                url,
                status: 0,
                latency_ms: 0,
                ret_code: None,
                ret_msg: None,
                body: json!({}),
                error: Some(format!("failed to initialize signature: {}", err)),
            };
        }
    };
    mac.update(payload.as_bytes());
    let sign = hex::encode(mac.finalize().into_bytes());

    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            return BybitWalletFetchResult {
                checked_at,
                account_type,
                url,
                status: 0,
                latency_ms: 0,
                ret_code: None,
                ret_msg: None,
                body: json!({}),
                error: Some(format!("failed to build http client: {}", err)),
            };
        }
    };

    let started = std::time::Instant::now();
    match client
        .get(&url)
        .header("X-BAPI-API-KEY", api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", timestamp)
        .header("X-BAPI-RECV-WINDOW", recv_window)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let parsed = resp.json::<Value>().await.unwrap_or_else(|_| json!({}));
            BybitWalletFetchResult {
                checked_at,
                account_type,
                url,
                status,
                latency_ms: started.elapsed().as_millis() as i64,
                ret_code: parsed.get("retCode").and_then(|v| v.as_i64()),
                ret_msg: parsed
                    .get("retMsg")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                body: parsed,
                error: None,
            }
        }
        Err(err) => BybitWalletFetchResult {
            checked_at,
            account_type,
            url,
            status: 0,
            latency_ms: started.elapsed().as_millis() as i64,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some(format!("request failed: {}", err)),
        },
    }
}

fn extract_wallet_summary(body: &Value) -> Value {
    body.get("result")
        .and_then(|result| result.get("list"))
        .and_then(|list| list.as_array())
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn json_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn start_of_day_utc(now: DateTime<Utc>) -> DateTime<Utc> {
    now.date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
        .unwrap_or(now)
}

async fn build_paper_daily_trades_summary(
    state: &AppState,
    window_start_utc: DateTime<Utc>,
    window_end_utc: DateTime<Utc>,
) -> DailyTradesSummaryResponse {
    let checked_at = Utc::now();
    let url = "paper://database-trades-today".to_string();

    let Some(pool) = &state.db_pool else {
        return DailyTradesSummaryResponse {
            ok: false,
            source: "paper-database".to_string(),
            count: 0,
            window_start_utc,
            window_end_utc,
            checked_at,
            status: 0,
            url,
            error: Some("database unavailable for paper daily trades".to_string()),
            ret_code: None,
            ret_msg: None,
        };
    };

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)::bigint
         FROM trades
         WHERE paper_trade = TRUE
           AND opened_at >= $1
           AND opened_at < $2",
    )
    .bind(window_start_utc)
    .bind(window_end_utc)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    DailyTradesSummaryResponse {
        ok: true,
        source: "paper-database".to_string(),
        count,
        window_start_utc,
        window_end_utc,
        checked_at,
        status: 200,
        url,
        error: None,
        ret_code: Some(0),
        ret_msg: Some("paper mode: daily trades summarized from database".to_string()),
    }
}

async fn fetch_bybit_order_history_today(
    window_start_utc: DateTime<Utc>,
    window_end_utc: DateTime<Utc>,
) -> BybitOrderHistoryFetchResult {
    let checked_at = Utc::now();
    let bybit_url = resolve_bybit_rest_url();
    let recv_window = read_non_empty_env("BYBIT_RECV_WINDOW").unwrap_or_else(|| "5000".to_string());
    let start_ms = window_start_utc.timestamp_millis();
    let end_ms = window_end_utc.timestamp_millis();
    let category = "linear";
    let url = format!(
        "{}/v5/order/history?category={}&settleCoin=USDT&startTime={}&endTime={}&limit=50",
        bybit_url, category, start_ms, end_ms
    );

    let (api_key, api_secret) = read_bybit_credential("BYBIT_API_KEY", "BYBIT_API_SECRET");

    let Some(api_key) = api_key else {
        return BybitOrderHistoryFetchResult {
            checked_at,
            url,
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some("missing BYBIT_API_KEY in api runtime".to_string()),
        };
    };
    let Some(api_secret) = api_secret else {
        return BybitOrderHistoryFetchResult {
            checked_at,
            url,
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some("missing BYBIT_API_SECRET in api runtime".to_string()),
        };
    };

    let timestamp = Utc::now().timestamp_millis().to_string();
    let query_string = format!(
        "category={}&settleCoin=USDT&startTime={}&endTime={}&limit=50",
        category, start_ms, end_ms
    );
    let payload = format!("{}{}{}{}", timestamp, api_key, recv_window, query_string);
    let mut mac = match Hmac::<Sha256>::new_from_slice(api_secret.as_bytes()) {
        Ok(v) => v,
        Err(err) => {
            return BybitOrderHistoryFetchResult {
                checked_at,
                url,
                status: 0,
                ret_code: None,
                ret_msg: None,
                body: json!({}),
                error: Some(format!("failed to initialize signature: {}", err)),
            };
        }
    };
    mac.update(payload.as_bytes());
    let sign = hex::encode(mac.finalize().into_bytes());

    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            return BybitOrderHistoryFetchResult {
                checked_at,
                url,
                status: 0,
                ret_code: None,
                ret_msg: None,
                body: json!({}),
                error: Some(format!("failed to build http client: {}", err)),
            };
        }
    };

    match client
        .get(&url)
        .header("X-BAPI-API-KEY", api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", timestamp)
        .header("X-BAPI-RECV-WINDOW", recv_window)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let parsed = resp.json::<Value>().await.unwrap_or_else(|_| json!({}));
            BybitOrderHistoryFetchResult {
                checked_at,
                url,
                status,
                ret_code: parsed.get("retCode").and_then(|v| v.as_i64()),
                ret_msg: parsed
                    .get("retMsg")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                body: parsed,
                error: None,
            }
        }
        Err(err) => BybitOrderHistoryFetchResult {
            checked_at,
            url,
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some(format!("request failed: {}", err)),
        },
    }
}

fn extract_bybit_today_trade_count(body: &Value) -> i64 {
    body.get("result")
        .and_then(|result| result.get("list"))
        .and_then(|list| list.as_array())
        .map(|items| {
            items.iter()
                .filter(|item| {
                    item.get("cumExecQty")
                        .and_then(json_number)
                        .map(|value| value > 0.0)
                        .unwrap_or_else(|| {
                            item.get("orderStatus")
                                .and_then(|value| value.as_str())
                                .map(|status| matches!(status, "Filled" | "PartiallyFilledCanceled"))
                                .unwrap_or(false)
                        })
                })
                .count() as i64
        })
        .unwrap_or(0)
}

async fn fetch_bybit_closed_pnl_page(
    window_start_utc: DateTime<Utc>,
    window_end_utc: DateTime<Utc>,
    limit: usize,
    cursor: Option<&str>,
) -> BybitClosedPnlFetchResult {
    let bybit_url = resolve_bybit_rest_url();
    let recv_window = read_non_empty_env("BYBIT_RECV_WINDOW").unwrap_or_else(|| "5000".to_string());
    let start_ms = window_start_utc.timestamp_millis();
    let end_ms = window_end_utc.timestamp_millis();
    let category = "linear";
    let mut query_string = format!(
        "category={}&settleCoin=USDT&startTime={}&endTime={}&limit={}",
        category,
        start_ms,
        end_ms,
        limit.clamp(1, 100)
    );
    if let Some(cursor) = cursor.filter(|value| !value.is_empty()) {
        query_string.push_str("&cursor=");
        query_string.push_str(cursor);
    }
    let url = format!("{}/v5/position/closed-pnl?{}", bybit_url, query_string);

    let (api_key, api_secret) = read_bybit_credential("BYBIT_API_KEY", "BYBIT_API_SECRET");
    let Some(api_key) = api_key else {
        return BybitClosedPnlFetchResult {
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some("missing BYBIT_API_KEY in api runtime".to_string()),
        };
    };
    let Some(api_secret) = api_secret else {
        return BybitClosedPnlFetchResult {
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some("missing BYBIT_API_SECRET in api runtime".to_string()),
        };
    };

    let timestamp = Utc::now().timestamp_millis().to_string();
    let payload = format!("{}{}{}{}", timestamp, api_key, recv_window, query_string);
    let mut mac = match Hmac::<Sha256>::new_from_slice(api_secret.as_bytes()) {
        Ok(v) => v,
        Err(err) => {
            return BybitClosedPnlFetchResult {
                status: 0,
                ret_code: None,
                ret_msg: None,
                body: json!({}),
                error: Some(format!("failed to initialize signature: {}", err)),
            };
        }
    };
    mac.update(payload.as_bytes());
    let sign = hex::encode(mac.finalize().into_bytes());

    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            return BybitClosedPnlFetchResult {
                status: 0,
                ret_code: None,
                ret_msg: None,
                body: json!({}),
                error: Some(format!("failed to build http client: {}", err)),
            };
        }
    };

    match client
        .get(&url)
        .header("X-BAPI-API-KEY", api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", timestamp)
        .header("X-BAPI-RECV-WINDOW", recv_window)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let parsed = resp.json::<Value>().await.unwrap_or_else(|_| json!({}));
            BybitClosedPnlFetchResult {
                status,
                ret_code: parsed.get("retCode").and_then(|v| v.as_i64()),
                ret_msg: parsed
                    .get("retMsg")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                body: parsed,
                error: None,
            }
        }
        Err(err) => BybitClosedPnlFetchResult {
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some(format!("request failed: {}", err)),
        },
    }
}

async fn fetch_bybit_closed_pnl_between(
    window_start_utc: DateTime<Utc>,
    window_end_utc: DateTime<Utc>,
    page_size: usize,
) -> Result<Vec<Value>, String> {
    let mut combined = Vec::new();
    let mut chunk_start = window_start_utc;

    while chunk_start < window_end_utc {
        let chunk_end = (chunk_start + ChronoDuration::days(7)).min(window_end_utc);
        let mut cursor: Option<String> = None;

        loop {
            let result = fetch_bybit_closed_pnl_page(chunk_start, chunk_end, page_size, cursor.as_deref()).await;
            if result.status != 200 || result.ret_code != Some(0) {
                return Err(
                    result
                        .error
                        .or_else(|| result.ret_msg)
                        .unwrap_or_else(|| format!("http={} retCode={:?}", result.status, result.ret_code)),
                );
            }

            let items = result
                .body
                .get("result")
                .and_then(|result| result.get("list"))
                .and_then(|list| list.as_array())
                .cloned()
                .unwrap_or_default();
            combined.extend(items);

            cursor = result
                .body
                .get("result")
                .and_then(|result| result.get("nextPageCursor"))
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            if cursor.is_none() {
                break;
            }
        }

        chunk_start = chunk_end;
    }

    Ok(combined)
}

async fn fetch_bybit_position_page(settle_coin: &str, cursor: Option<&str>) -> BybitPositionFetchResult {
    let bybit_url = resolve_bybit_rest_url();
    let recv_window = read_non_empty_env("BYBIT_RECV_WINDOW").unwrap_or_else(|| "5000".to_string());
    let mut query_string = format!("category=linear&settleCoin={}&limit=200", settle_coin);
    if let Some(cursor) = cursor.filter(|value| !value.is_empty()) {
        query_string.push_str("&cursor=");
        query_string.push_str(cursor);
    }
    let url = format!("{}/v5/position/list?{}", bybit_url, query_string);

    let (api_key, api_secret) = read_bybit_credential("BYBIT_API_KEY", "BYBIT_API_SECRET");
    let Some(api_key) = api_key else {
        return BybitPositionFetchResult {
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some("missing BYBIT_API_KEY in api runtime".to_string()),
        };
    };
    let Some(api_secret) = api_secret else {
        return BybitPositionFetchResult {
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some("missing BYBIT_API_SECRET in api runtime".to_string()),
        };
    };

    let timestamp = Utc::now().timestamp_millis().to_string();
    let payload = format!("{}{}{}{}", timestamp, api_key, recv_window, query_string);
    let mut mac = match Hmac::<Sha256>::new_from_slice(api_secret.as_bytes()) {
        Ok(v) => v,
        Err(err) => {
            return BybitPositionFetchResult {
                status: 0,
                ret_code: None,
                ret_msg: None,
                body: json!({}),
                error: Some(format!("failed to initialize signature: {}", err)),
            };
        }
    };
    mac.update(payload.as_bytes());
    let sign = hex::encode(mac.finalize().into_bytes());

    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            return BybitPositionFetchResult {
                status: 0,
                ret_code: None,
                ret_msg: None,
                body: json!({}),
                error: Some(format!("failed to build http client: {}", err)),
            };
        }
    };

    match client
        .get(&url)
        .header("X-BAPI-API-KEY", api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", timestamp)
        .header("X-BAPI-RECV-WINDOW", recv_window)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let parsed = resp.json::<Value>().await.unwrap_or_else(|_| json!({}));
            BybitPositionFetchResult {
                status,
                ret_code: parsed.get("retCode").and_then(|v| v.as_i64()),
                ret_msg: parsed
                    .get("retMsg")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                body: parsed,
                error: None,
            }
        }
        Err(err) => BybitPositionFetchResult {
            status: 0,
            ret_code: None,
            ret_msg: None,
            body: json!({}),
            error: Some(format!("request failed: {}", err)),
        },
    }
}

async fn fetch_bybit_positions() -> Result<Vec<Value>, String> {
    let mut combined = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let result = fetch_bybit_position_page("USDT", cursor.as_deref()).await;
        if result.status != 200 || result.ret_code != Some(0) {
            return Err(
                result
                    .error
                    .or_else(|| result.ret_msg)
                    .unwrap_or_else(|| format!("http={} retCode={:?}", result.status, result.ret_code)),
            );
        }

        let items = result
            .body
            .get("result")
            .and_then(|result| result.get("list"))
            .and_then(|list| list.as_array())
            .cloned()
            .unwrap_or_default();
        combined.extend(items);

        cursor = result
            .body
            .get("result")
            .and_then(|result| result.get("nextPageCursor"))
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        if cursor.is_none() {
            break;
        }
    }

    Ok(combined)
}

fn build_position_item_from_bybit(state: &AppState, item: &Value) -> Option<PositionItem> {
    let quantity = item.get("size").and_then(json_number).unwrap_or(0.0);
    if quantity <= 0.0 {
        return None;
    }

    let symbol = item.get("symbol").and_then(|value| value.as_str())?.to_string();
    let raw_side = item.get("side").and_then(|value| value.as_str()).unwrap_or("");
    let side = map_bybit_side(Some(raw_side));
    let entry_price = item.get("avgPrice").and_then(json_number).unwrap_or(0.0);
    let notional_usdt = item
        .get("positionValue")
        .and_then(json_number)
        .or_else(|| item.get("positionIM").and_then(json_number))
        .unwrap_or(quantity * entry_price);

    let (
        stop_loss_price,
        trailing_activation_price,
        fixed_take_profit_price,
        break_even_price,
    ) = resolve_position_triggers(state, &symbol, &side, entry_price);

    Some(PositionItem {
        trade_id: item
            .get("positionIdx")
            .and_then(|value| value.as_i64())
            .map(|value| format!("{}-{}", symbol, value))
            .unwrap_or_else(|| format!("{}-live", symbol)),
        symbol,
        side,
        quantity,
        notional_usdt: round6(notional_usdt),
        entry_price: round6(entry_price),
        trailing_stop_activated: false,
        trailing_stop_peak_price: None,
        trailing_stop_final_distance_pct: None,
        stop_loss_price,
        trailing_activation_price,
        fixed_take_profit_price,
        break_even_price,
    })
}

fn parse_bybit_timestamp_millis(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let raw = match value {
        Some(Value::String(text)) => text.parse::<i64>().ok(),
        Some(Value::Number(number)) => number.as_i64(),
        _ => None,
    }?;
    DateTime::<Utc>::from_timestamp_millis(raw)
}

fn map_bybit_side(value: Option<&str>) -> String {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "buy" => "LONG".to_string(),
        "sell" => "SHORT".to_string(),
        other if !other.is_empty() => other.to_ascii_uppercase(),
        _ => "UNKNOWN".to_string(),
    }
}

fn infer_position_side_from_closed_pnl(
    raw_side: Option<&str>,
    entry_price: f64,
    exit_price: Option<f64>,
    pnl: Option<f64>,
) -> String {
    if let (Some(exit_price), Some(pnl)) = (exit_price, pnl) {
        let delta = exit_price - entry_price;
        if delta.abs() > f64::EPSILON && pnl.abs() > f64::EPSILON {
            return if delta * pnl >= 0.0 {
                "LONG".to_string()
            } else {
                "SHORT".to_string()
            };
        }
    }

    map_bybit_side(raw_side)
}

fn build_trade_item_from_closed_pnl(item: &Value) -> Option<TradeItem> {
    let closed_at = parse_bybit_timestamp_millis(item.get("updatedTime"))
        .or_else(|| parse_bybit_timestamp_millis(item.get("createdTime")))?;
    let opened_at = parse_bybit_timestamp_millis(item.get("createdTime")).unwrap_or(closed_at);
    let symbol = item.get("symbol").and_then(|value| value.as_str())?.to_string();
    let entry_price = item.get("avgEntryPrice").and_then(json_number).unwrap_or(0.0);
    let exit_price = item.get("avgExitPrice").and_then(json_number);
    let pnl = item.get("closedPnl").and_then(json_number);
    let trade_id = item
        .get("orderId")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("{}-{}", symbol, closed_at.timestamp_millis()));

    Some(TradeItem {
        trade_id,
        symbol,
        side: infer_position_side_from_closed_pnl(
            item.get("side").and_then(|value| value.as_str()),
            entry_price,
            exit_price,
            pnl,
        ),
        status: "closed".to_string(),
        quantity: item
            .get("closedSize")
            .and_then(json_number)
            .or_else(|| item.get("qty").and_then(json_number))
            .unwrap_or(0.0),
        entry_price,
        exit_price,
        pnl,
        opened_at,
        closed_at: Some(closed_at),
    })
}

fn summarize_closed_pnl_window(
    items: &[Value],
    window_start_utc: DateTime<Utc>,
    window_end_utc: DateTime<Utc>,
) -> PerformanceWindow {
    let mut total_trades = 0_i64;
    let mut winning_trades = 0_i64;
    let mut total_pnl = 0.0_f64;

    for item in items {
        let Some(closed_at) = parse_bybit_timestamp_millis(item.get("updatedTime"))
            .or_else(|| parse_bybit_timestamp_millis(item.get("createdTime")))
        else {
            continue;
        };

        if closed_at < window_start_utc || closed_at >= window_end_utc {
            continue;
        }

        let pnl = item.get("closedPnl").and_then(json_number).unwrap_or(0.0);
        total_trades += 1;
        total_pnl += pnl;
        if pnl > 0.0 {
            winning_trades += 1;
        }
    }

    let win_rate = if total_trades > 0 {
        winning_trades as f64 / total_trades as f64
    } else {
        0.0
    };

    PerformanceWindow {
        window_start_utc,
        window_end_utc,
        total_trades,
        winning_trades,
        win_rate: round6(win_rate),
        total_pnl: round6(total_pnl),
    }
}

async fn control_state_handler(state: Arc<AppState>) -> impl Reply {
    let mut kill_switch = KillSwitchStatus {
        enabled: false,
        reason: None,
        actor: None,
        updated_at: None,
    };
    let mut executor = ExecutorControlStatus {
        enabled: state.executor_default_enabled,
        reason: None,
        actor: None,
        updated_at: None,
    };
    let mut risk_limits = RiskLimitsStatus {
        max_daily_loss_pct: round6(state.default_max_daily_loss_pct),
        max_leverage: round6(state.default_max_leverage),
        risk_per_trade_pct: round6(state.default_risk_per_trade_pct),
        reason: None,
        actor: None,
        updated_at: None,
    };

    if let Some(pool) = &state.db_pool {
        if let Ok(v) = fetch_kill_switch_status(pool).await {
            kill_switch = v;
        }
        if let Ok(v) = fetch_executor_status(pool, state.executor_default_enabled).await {
            executor = v;
        }
        if let Ok(v) = fetch_risk_limits_status(pool, &state).await {
            risk_limits = v;
        }
    }

    json_ok(&ControlStateResponse {
        operator_auth_mode: state.operator_auth_mode.clone(),
        operator_controls_enabled: state.operator_api_token.is_some()
            && state.operator_auth_mode.eq_ignore_ascii_case("token"),
        kill_switch,
        executor,
        risk_limits,
    })
}

async fn kill_switch_handler(
    req: KillSwitchRequest,
    token_header: Option<String>,
    operator_id_header: Option<String>,
    state: Arc<AppState>,
) -> impl Reply {
    if let Err(err) = ensure_operator_token(&state, token_header.as_deref()) {
        return err;
    }

    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let operator_id = operator_id_header.unwrap_or_else(|| "operator".to_string());
    let reason = req
        .reason
        .clone()
        .unwrap_or_else(|| "manual_api_control".to_string());

    let event_data = json!({
        "enabled": req.enabled,
        "reason": reason,
        "actor": operator_id,
        "source": "api",
    });

    let severity = if req.enabled { "warning" } else { "info" };

    let insert = sqlx::query(
        "INSERT INTO system_events (event_type, severity, category, data, timestamp)
         VALUES ($1, $2, $3, $4, NOW())",
    )
    .bind("api_kill_switch_set")
    .bind(severity)
    .bind("system")
    .bind(Json(event_data))
    .execute(pool)
    .await;

    if let Err(err) = insert {
        return json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persist_failed",
            format!("failed to persist kill-switch event: {}", err),
        );
    }

    let kill_switch = fetch_kill_switch_status(pool)
        .await
        .unwrap_or(KillSwitchStatus {
            enabled: req.enabled,
            reason: req.reason,
            actor: Some("operator".to_string()),
            updated_at: Some(Utc::now()),
        });

    json_ok(&KillSwitchResponse {
        updated: true,
        kill_switch,
    })
}

async fn executor_control_handler(
    req: ExecutorControlRequest,
    token_header: Option<String>,
    operator_id_header: Option<String>,
    state: Arc<AppState>,
) -> impl Reply {
    if let Err(err) = ensure_operator_token(&state, token_header.as_deref()) {
        return err;
    }

    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let operator_id = operator_id_header.unwrap_or_else(|| "operator".to_string());
    let reason = req
        .reason
        .clone()
        .unwrap_or_else(|| "manual_executor_control".to_string());

    let event_data = json!({
        "enabled": req.enabled,
        "reason": reason,
        "actor": operator_id,
        "source": "api",
    });

    let severity = if req.enabled { "info" } else { "warning" };

    let insert = sqlx::query(
        "INSERT INTO system_events (event_type, severity, category, data, timestamp)
         VALUES ($1, $2, $3, $4, NOW())",
    )
    .bind("api_executor_state_set")
    .bind(severity)
    .bind("system")
    .bind(Json(event_data))
    .execute(pool)
    .await;

    if let Err(err) = insert {
        return json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persist_failed",
            format!("failed to persist executor control event: {}", err),
        );
    }

    let executor = fetch_executor_status(pool, state.executor_default_enabled)
        .await
        .unwrap_or(ExecutorControlStatus {
            enabled: req.enabled,
            reason: req.reason,
            actor: Some("operator".to_string()),
            updated_at: Some(Utc::now()),
        });

    json_ok(&ExecutorControlResponse {
        updated: true,
        executor,
    })
}

async fn risk_limits_control_handler(
    req: RiskLimitsRequest,
    token_header: Option<String>,
    operator_id_header: Option<String>,
    state: Arc<AppState>,
) -> impl Reply {
    if let Err(err) = ensure_operator_token(&state, token_header.as_deref()) {
        return err;
    }

    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let current = fetch_risk_limits_status(pool, &state)
        .await
        .unwrap_or(RiskLimitsStatus {
            max_daily_loss_pct: state.default_max_daily_loss_pct,
            max_leverage: state.default_max_leverage,
            risk_per_trade_pct: state.default_risk_per_trade_pct,
            reason: None,
            actor: None,
            updated_at: None,
        });

    let max_daily_loss_pct = req.max_daily_loss_pct.unwrap_or(current.max_daily_loss_pct);
    let max_leverage = req.max_leverage.unwrap_or(current.max_leverage);
    let risk_per_trade_pct = req.risk_per_trade_pct.unwrap_or(current.risk_per_trade_pct);

    if let Err(msg) = validate_risk_limits(max_daily_loss_pct, max_leverage, risk_per_trade_pct) {
        return json_err(StatusCode::BAD_REQUEST, "invalid_limits", msg);
    }

    let operator_id = operator_id_header.unwrap_or_else(|| "operator".to_string());
    let reason = req
        .reason
        .clone()
        .unwrap_or_else(|| "manual_risk_limits_update".to_string());

    let event_data = json!({
        "max_daily_loss_pct": round6(max_daily_loss_pct),
        "max_leverage": round6(max_leverage),
        "risk_per_trade_pct": round6(risk_per_trade_pct),
        "reason": reason,
        "actor": operator_id,
        "source": "api",
    });

    let insert = sqlx::query(
        "INSERT INTO system_events (event_type, severity, category, data, timestamp)
         VALUES ($1, $2, $3, $4, NOW())",
    )
    .bind("api_risk_limits_set")
    .bind("warning")
    .bind("risk")
    .bind(Json(event_data))
    .execute(pool)
    .await;

    if let Err(err) = insert {
        return json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persist_failed",
            format!("failed to persist risk limits event: {}", err),
        );
    }

    let risk_limits = fetch_risk_limits_status(pool, &state)
        .await
        .unwrap_or(RiskLimitsStatus {
            max_daily_loss_pct: round6(max_daily_loss_pct),
            max_leverage: round6(max_leverage),
            risk_per_trade_pct: round6(risk_per_trade_pct),
            reason: req.reason,
            actor: Some("operator".to_string()),
            updated_at: Some(Utc::now()),
        });

    json_ok(&RiskLimitsResponse {
        updated: true,
        risk_limits,
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
async fn main() {
    let db_pool = if let Some(database_url) = resolve_database_url() {
        match PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                println!("Connected to PostgreSQL for API queries");
                Some(pool)
            }
            Err(err) => {
                eprintln!("api: failed to connect PostgreSQL: {}", err);
                None
            }
        }
    } else {
        eprintln!("api: database env not configured; DB-backed endpoints degraded");
        None
    };

    let state = Arc::new(AppState {
        db_pool,
        trading_mode: TradingMode::from_env().as_status_label().to_string(),
        trading_profile: std::env::var("TRADING_PROFILE").unwrap_or_else(|_| "MEDIUM".to_string()),
        initial_capital_usd: read_f64_env("INITIAL_CAPITAL_USD", 100.0),
        operator_auth_mode: std::env::var("OPERATOR_AUTH_MODE")
            .unwrap_or_else(|_| "token".to_string()),
        operator_api_token: read_non_empty_env("OPERATOR_API_TOKEN"),
        executor_default_enabled: read_bool_env("EXECUTOR_DEFAULT_ENABLED", true),
        default_max_daily_loss_pct: read_f64_env("MAX_DAILY_LOSS_PCT", 3.0),
        default_max_leverage: read_f64_env("MAX_LEVERAGE", 2.0),
        default_risk_per_trade_pct: read_f64_env("RISK_PER_TRADE_PCT", 1.25),
        position_config: load_position_config(
            &std::env::var("STRATEGY_CONFIG")
                .unwrap_or_else(|_| "config/trading/pairs.yaml".to_string()),
        ),
    });

    let api_v1 = warp::path("api").and(warp::path("v1"));

    let health = api_v1
        .and(warp::path("health"))
        .and(warp::path::end())
        .and(with_state(state.clone()))
        .then(health_handler);

    let status = api_v1
        .and(warp::path("status"))
        .and(warp::path::end())
        .and(with_state(state.clone()))
        .then(status_handler);

    let positions = api_v1
        .and(warp::path("positions"))
        .and(warp::path::end())
        .and(with_state(state.clone()))
        .then(positions_handler);

    let trades = api_v1
        .and(warp::path("trades"))
        .and(warp::path::end())
        .and(warp::query::<TradesQuery>())
        .and(with_state(state.clone()))
        .then(trades_handler);

    let daily_trades_summary = api_v1
        .and(warp::path("trades"))
        .and(warp::path("today-summary"))
        .and(warp::path::end())
        .and(with_state(state.clone()))
        .then(daily_trades_summary_handler);

    let events = api_v1
        .and(warp::path("events"))
        .and(warp::path::end())
        .and(warp::query::<EventsQuery>())
        .and(with_state(state.clone()))
        .then(events_handler);

    let performance = api_v1
        .and(warp::path("performance"))
        .and(warp::path::end())
        .and(with_state(state.clone()))
        .then(performance_handler);

    let risk_kpis = api_v1
        .and(warp::path("risk"))
        .and(warp::path("kpis"))
        .and(warp::path::end())
        .and(with_state(state.clone()))
        .then(risk_kpis_handler);

    let bybit_private_health = api_v1
        .and(warp::path("external"))
        .and(warp::path("bybit-private-health"))
        .and(warp::path::end())
        .and(warp::get())
        .and(with_state(state.clone()))
        .then(bybit_private_health_handler);

    let bybit_wallet = api_v1
        .and(warp::path("external"))
        .and(warp::path("bybit-wallet"))
        .and(warp::path::end())
        .and(warp::get())
        .and(with_state(state.clone()))
        .then(bybit_wallet_handler);

    let control_state = api_v1
        .and(warp::path("control"))
        .and(warp::path("state"))
        .and(warp::path::end())
        .and(warp::get())
        .and(with_state(state.clone()))
        .then(control_state_handler);

    let kill_switch = api_v1
        .and(warp::path("control"))
        .and(warp::path("kill-switch"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json::<KillSwitchRequest>())
        .and(warp::header::optional::<String>("x-operator-token"))
        .and(warp::header::optional::<String>("x-operator-id"))
        .and(with_state(state.clone()))
        .then(kill_switch_handler);

    let executor_control = api_v1
        .and(warp::path("control"))
        .and(warp::path("executor"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json::<ExecutorControlRequest>())
        .and(warp::header::optional::<String>("x-operator-token"))
        .and(warp::header::optional::<String>("x-operator-id"))
        .and(with_state(state.clone()))
        .then(executor_control_handler);

    let risk_limits_control = api_v1
        .and(warp::path("control"))
        .and(warp::path("risk-limits"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json::<RiskLimitsRequest>())
        .and(warp::header::optional::<String>("x-operator-token"))
        .and(warp::header::optional::<String>("x-operator-id"))
        .and(with_state(state.clone()))
        .then(risk_limits_control_handler);

    let legacy_root = warp::path::end().map(|| "Hello, ViperTrade API!");
    let legacy_health = warp::path("health")
        .and(warp::path::end())
        .map(|| warp::reply::json(&"OK"));

    let routes = health
        .or(status)
        .or(positions)
        .or(trades)
        .or(daily_trades_summary)
        .or(events)
        .or(performance)
        .or(risk_kpis)
        .or(bybit_private_health)
        .or(bybit_wallet)
        .or(control_state)
        .or(kill_switch)
        .or(executor_control)
        .or(risk_limits_control)
        .or(legacy_root)
        .or(legacy_health)
        .recover(handle_rejection);

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let (_addr, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([0, 0, 0, 0], 8080), async move {
            let _ = shutdown_rx.changed().await;
            println!("Received shutdown signal, stopping viper-api");
        });

    server.await;
}

#[cfg(test)]
mod tests {
    use super::{clamp_limit, round6, validate_risk_limits};

    #[test]
    fn clamp_limit_enforces_bounds() {
        assert_eq!(clamp_limit(None, 50), 50);
        assert_eq!(clamp_limit(Some(0), 50), 1);
        assert_eq!(clamp_limit(Some(10), 50), 10);
        assert_eq!(clamp_limit(Some(500), 50), 200);
    }

    #[test]
    fn round6_keeps_six_decimals() {
        assert_eq!(round6(1.23456789), 1.234568);
        assert_eq!(round6(1.0), 1.0);
    }

    #[test]
    fn validate_risk_limits_bounds() {
        assert!(validate_risk_limits(3.0, 2.0, 1.0).is_ok());
        assert!(validate_risk_limits(-1.0, 2.0, 1.0).is_err());
        assert!(validate_risk_limits(3.0, 0.0, 1.0).is_err());
        assert!(validate_risk_limits(3.0, 2.0, 120.0).is_err());
    }
}
