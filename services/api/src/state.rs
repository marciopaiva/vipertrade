use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::position_config::PositionConfigStore;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: Option<PgPool>,
    pub trading_mode: String,
    pub trading_profile: String,
    pub trade_profile_label: String,
    pub initial_capital_usd: f64,
    pub operator_auth_mode: String,
    pub operator_api_token: Option<String>,
    pub executor_default_enabled: bool,
    pub default_max_daily_loss_pct: f64,
    pub default_max_leverage: f64,
    pub default_risk_per_trade_pct: f64,
    pub position_config: PositionConfigStore,
}

#[derive(Serialize)]
pub struct ApiError {
    pub error: &'static str,
    pub message: String,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub db_connected: bool,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub service: &'static str,
    pub trading_mode: String,
    pub trading_profile: String,
    pub trade_profile_label: String,
    pub db_connected: bool,
    pub operator_auth_mode: String,
    pub operator_controls_enabled: bool,
    pub risk_status: String,
    pub critical_reconciliation_events_15m: i64,
    pub kill_switch: KillSwitchStatus,
    pub executor: ExecutorControlStatus,
    pub risk_limits: RiskLimitsStatus,
}

#[derive(Serialize, Clone)]
pub struct KillSwitchStatus {
    pub enabled: bool,
    pub reason: Option<String>,
    pub actor: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Clone)]
pub struct ExecutorControlStatus {
    pub enabled: bool,
    pub reason: Option<String>,
    pub actor: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Clone)]
pub struct RiskLimitsStatus {
    pub max_daily_loss_pct: f64,
    pub max_leverage: f64,
    pub risk_per_trade_pct: f64,
    pub reason: Option<String>,
    pub actor: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct KillSwitchRequest {
    pub enabled: bool,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct ExecutorControlRequest {
    pub enabled: bool,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct RiskLimitsRequest {
    pub max_daily_loss_pct: Option<f64>,
    pub max_leverage: Option<f64>,
    pub risk_per_trade_pct: Option<f64>,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct TradesQuery {
    pub limit: Option<u32>,
}

#[derive(Deserialize)]
pub struct EventsQuery {
    pub limit: Option<u32>,
}

#[derive(Deserialize)]
pub struct SymbolPnlQuery {
    pub days: Option<i64>,
}

#[derive(Serialize)]
pub struct SymbolPnlItem {
    pub symbol: String,
    pub realized_pnl: f64,
    pub trades: i64,
    pub wins: i64,
    pub win_rate: f64,
    pub avg_pnl_pct: f64,
}

#[derive(Serialize)]
pub struct SymbolPnlResponse {
    pub window_days: i64,
    // Ordered worst-first (lowest realized PnL) so callers can prune the tail.
    pub items: Vec<SymbolPnlItem>,
}

#[derive(Serialize)]
pub struct PositionItem {
    pub trade_id: String,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub notional_usdt: f64,
    pub entry_price: f64,
    pub opened_at: DateTime<Utc>,
    pub trailing_stop_activated: bool,
    pub trailing_stop_peak_price: Option<f64>,
    pub trailing_stop_final_distance_pct: Option<f64>,
    pub stop_loss_price: Option<f64>,
    pub trailing_activation_price: Option<f64>,
    pub fixed_take_profit_price: Option<f64>,
    pub break_even_price: Option<f64>,
}

#[derive(Serialize)]
pub struct PositionsResponse {
    pub items: Vec<PositionItem>,
}

#[derive(Serialize)]
pub struct TradesResponse {
    pub items: Vec<TradeItem>,
}

#[derive(Deserialize)]
pub struct DecisionsQuery {
    pub limit: Option<u32>,
}

/// Latest strategy decision per symbol, with the multi-exchange consensus
/// indicators that drove it — powers the web "Strategy Cockpit".
#[derive(Serialize)]
pub struct DecisionItem {
    pub symbol: String,
    pub action: String,
    pub consensus_side: Option<String>,
    pub consensus_count: Option<i64>,
    pub exchanges_available: Option<i64>,
    pub bullish_exchanges: Option<i64>,
    pub bearish_exchanges: Option<i64>,
    pub consensus_rsi_14: Option<f64>,
    pub consensus_bollinger_percent_b: Option<f64>,
    pub consensus_trend_score: Option<f64>,
    pub consensus_macd_histogram: Option<f64>,
    pub consensus_adx_14: Option<f64>,
    pub current_price: Option<f64>,
    pub executed_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct DecisionsResponse {
    pub items: Vec<DecisionItem>,
}

#[derive(Serialize)]
pub struct EventsResponse {
    pub items: Vec<EventItem>,
}

#[derive(Serialize)]
pub struct PerformanceResponse {
    pub last_24h: PerformanceWindow,
    pub last_7d: PerformanceWindow,
    pub last_30d: PerformanceWindow,
    pub max_drawdown_30d: Option<f64>,
}

#[derive(Serialize)]
pub struct RiskKpisResponse {
    pub rejected_orders_24h: i64,
    pub open_exposure_usdt: f64,
    pub realized_pnl_24h: f64,
    pub critical_events_24h: i64,
    pub circuit_breaker_triggers_24h: i64,
}

#[derive(Serialize)]
pub struct BybitPrivateHealthResponse {
    pub name: &'static str,
    pub ok: bool,
    pub status: u16,
    pub latency_ms: i64,
    pub url: String,
    pub error: Option<String>,
    pub ret_code: Option<i64>,
    pub ret_msg: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct BybitWalletResponse {
    pub ok: bool,
    pub status: u16,
    pub url: String,
    pub error: Option<String>,
    pub ret_code: Option<i64>,
    pub ret_msg: Option<String>,
    pub checked_at: DateTime<Utc>,
    pub account_type: String,
    pub total_equity: Option<f64>,
    pub wallet_balance: Option<f64>,
    pub margin_balance: Option<f64>,
    pub available_balance: Option<f64>,
    pub unrealized_pnl: Option<f64>,
    pub initial_margin: Option<f64>,
    pub maintenance_margin: Option<f64>,
    pub account_im_rate: Option<f64>,
    pub account_mm_rate: Option<f64>,
}

#[derive(Serialize)]
pub struct DailyTradesSummaryResponse {
    pub ok: bool,
    pub source: String,
    pub count: i64,
    pub window_start_utc: DateTime<Utc>,
    pub window_end_utc: DateTime<Utc>,
    pub checked_at: DateTime<Utc>,
    pub status: u16,
    pub url: String,
    pub error: Option<String>,
    pub ret_code: Option<i64>,
    pub ret_msg: Option<String>,
}

pub struct BybitWalletFetchResult {
    pub checked_at: DateTime<Utc>,
    pub account_type: String,
    pub url: String,
    pub status: u16,
    pub latency_ms: i64,
    pub ret_code: Option<i64>,
    pub ret_msg: Option<String>,
    pub body: Value,
    pub error: Option<String>,
}

pub struct BybitOrderHistoryFetchResult {
    pub checked_at: DateTime<Utc>,
    pub url: String,
    pub status: u16,
    pub ret_code: Option<i64>,
    pub ret_msg: Option<String>,
    pub body: Value,
    pub error: Option<String>,
}

pub struct BybitClosedPnlFetchResult {
    pub status: u16,
    pub ret_code: Option<i64>,
    pub ret_msg: Option<String>,
    pub body: Value,
    pub error: Option<String>,
}

pub struct BybitPositionFetchResult {
    pub status: u16,
    pub ret_code: Option<i64>,
    pub ret_msg: Option<String>,
    pub body: Value,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct TradeItem {
    pub trade_id: String,
    pub symbol: String,
    pub side: String,
    pub status: String,
    pub quantity: f64,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub pnl: Option<f64>,
    pub close_reason: Option<String>,
    pub duration_seconds: Option<i64>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct EventItem {
    pub event_id: String,
    pub event_type: String,
    pub severity: String,
    pub category: Option<String>,
    pub symbol: Option<String>,
    pub trade_id: Option<String>,
    pub data: Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct PerformanceWindow {
    pub window_start_utc: DateTime<Utc>,
    pub window_end_utc: DateTime<Utc>,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub win_rate: f64,
    pub total_pnl: f64,
}

#[derive(Serialize, Clone)]
pub struct KillSwitchResponse {
    pub updated: bool,
    pub kill_switch: KillSwitchStatus,
}

#[derive(Serialize, Clone)]
pub struct ExecutorControlResponse {
    pub updated: bool,
    pub executor: ExecutorControlStatus,
}

#[derive(Serialize, Clone)]
pub struct RiskLimitsResponse {
    pub updated: bool,
    pub risk_limits: RiskLimitsStatus,
}

#[derive(Serialize)]
pub struct ControlStateResponse {
    pub operator_auth_mode: String,
    pub operator_controls_enabled: bool,
    pub kill_switch: KillSwitchStatus,
    pub executor: ExecutorControlStatus,
    pub risk_limits: RiskLimitsStatus,
}
