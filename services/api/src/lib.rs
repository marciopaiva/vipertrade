// The big `.or(...)` warp route chain nests filter types deeply; the default
// trait-resolution recursion limit (128) overflows in release builds, so raise it.
#![recursion_limit = "512"]

mod bybit_client;
mod position_config;
mod state;

use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::watch;
use viper_domain::config::*;
use warp::http::StatusCode;
use warp::reply::Json as WarpJson;
use warp::{Filter, Rejection, Reply};

use position_config::{default_trailing_profile, load_position_config};
use state::{
    ApiError, AppState, BybitClosedPnlFetchResult, BybitOrderHistoryFetchResult,
    BybitPositionFetchResult, BybitWalletFetchResult, ControlStateResponse, DecisionItem,
    DecisionsQuery, DecisionsResponse, EventItem, EventsQuery, EventsResponse,
    ExecutorControlRequest, ExecutorControlResponse, ExecutorControlStatus, HealthResponse,
    KillSwitchRequest, KillSwitchResponse, KillSwitchStatus, PerformanceResponse,
    PerformanceWindow, PositionItem, PositionsResponse, RiskLimitsRequest, RiskLimitsResponse,
    RiskLimitsStatus, StatusResponse, SymbolPnlItem, SymbolPnlQuery, SymbolPnlResponse, TradeItem,
    TradesQuery, TradesResponse,
};

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
    let mode_cfg = state
        .position_config
        .mode_profiles
        .get(&state.trading_mode.to_uppercase());
    let stop_loss_pct = mode_cfg
        .and_then(|cfg| cfg.stop_loss_pct)
        .or_else(|| pair_cfg.map(|p| p.stop_loss_pct))
        .unwrap_or(0.015);
    let take_profit_pct = mode_cfg
        .and_then(|cfg| cfg.take_profit_pct)
        .or_else(|| pair_cfg.map(|p| p.take_profit_pct))
        .unwrap_or(0.03);
    let trailing_enabled = mode_cfg
        .and_then(|cfg| cfg.trailing_enabled)
        .or_else(|| {
            pair_cfg
                .and_then(|p| p.trailing_enabled)
                .or(Some(state.position_config.global.trailing_enabled))
        })
        .unwrap_or(true);
    let fixed_take_profit_enabled = mode_cfg
        .and_then(|cfg| cfg.fixed_take_profit_enabled)
        .unwrap_or(true);
    let trailing_profile = mode_cfg
        .and_then(|cfg| cfg.trailing.clone())
        .or_else(|| {
            pair_cfg.and_then(|p| {
                p.trailing_by_profile
                    .get(&state.trading_profile.to_uppercase())
                    .cloned()
            })
        })
        .unwrap_or_else(default_trailing_profile);

    let is_long = side.eq_ignore_ascii_case("long");
    let stop_loss_price = if is_long {
        entry_price * (1.0 - stop_loss_pct)
    } else {
        entry_price * (1.0 + stop_loss_pct)
    };
    let fixed_take_profit_price = if fixed_take_profit_enabled {
        Some(if is_long {
            entry_price * (1.0 + take_profit_pct)
        } else {
            entry_price * (1.0 - take_profit_pct)
        })
    } else {
        None
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
        fixed_take_profit_price,
        break_even_price,
    )
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
        trade_profile_label: state.trade_profile_label.clone(),
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
            DateTime<Utc>,
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
             opened_at,
             COALESCE(trailing_stop_activated, false),
             trailing_stop_peak_price::double precision,
             trailing_stop_final_distance_pct::double precision
         FROM trades
         WHERE status = 'open'
           AND paper_trade = TRUE
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
                        opened_at,
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
                            opened_at,
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

async fn build_exchange_positions_response(
    state: Arc<AppState>,
) -> warp::reply::WithStatus<WarpJson> {
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

async fn build_paper_trades_response(
    query: TradesQuery,
    state: Arc<AppState>,
) -> warp::reply::WithStatus<WarpJson> {
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
            Option<String>,
            Option<i64>,
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
             close_reason,
             CASE
               WHEN closed_at IS NOT NULL THEN GREATEST(0, EXTRACT(EPOCH FROM (closed_at - opened_at))::bigint)
               ELSE NULL
             END AS duration_seconds,
             opened_at,
             closed_at
         FROM trades
         WHERE paper_trade = TRUE
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
                        close_reason,
                        duration_seconds,
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
                        close_reason,
                        duration_seconds,
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
            trades.sort_by(|a, b| {
                b.closed_at
                    .unwrap_or(b.opened_at)
                    .cmp(&a.closed_at.unwrap_or(a.opened_at))
            });
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

async fn decisions_handler(query: DecisionsQuery, state: Arc<AppState>) -> impl Reply {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let limit = clamp_limit(query.limit, 30);

    // Latest decision per symbol, with the consensus indicators from input_data.
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            DateTime<Utc>,
        ),
    >(
        "SELECT * FROM (
             SELECT DISTINCT ON (input_data->>'symbol')
                 input_data->>'symbol' AS symbol,
                 COALESCE(output_data->'decision'->>'action', 'HOLD') AS action,
                 input_data->'signal'->>'consensus_side' AS consensus_side,
                 (input_data->'signal'->>'consensus_count')::numeric::int8 AS consensus_count,
                 (input_data->'signal'->>'exchanges_available')::numeric::int8 AS exchanges_available,
                 (input_data->'signal'->>'bullish_exchanges')::numeric::int8 AS bullish_exchanges,
                 (input_data->'signal'->>'bearish_exchanges')::numeric::int8 AS bearish_exchanges,
                 (input_data->'signal'->>'consensus_rsi_14')::float8 AS consensus_rsi_14,
                 (input_data->'signal'->>'consensus_bollinger_percent_b')::float8 AS percent_b,
                 (input_data->'signal'->>'consensus_trend_score')::float8 AS consensus_trend_score,
                 (input_data->'signal'->>'consensus_macd_histogram')::float8 AS macd_hist,
                 (input_data->'signal'->>'current_price')::float8 AS current_price,
                 (input_data->'signal'->>'consensus_adx_14')::float8 AS consensus_adx_14,
                 executed_at
             FROM tupa_audit_logs
             WHERE input_data ? 'signal'
             ORDER BY input_data->>'symbol', executed_at DESC
         ) latest
         ORDER BY executed_at DESC
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
                        symbol,
                        action,
                        consensus_side,
                        consensus_count,
                        exchanges_available,
                        bullish_exchanges,
                        bearish_exchanges,
                        consensus_rsi_14,
                        consensus_bollinger_percent_b,
                        consensus_trend_score,
                        consensus_macd_histogram,
                        current_price,
                        consensus_adx_14,
                        executed_at,
                    )| DecisionItem {
                        symbol,
                        action,
                        consensus_side,
                        consensus_count,
                        exchanges_available,
                        bullish_exchanges,
                        bearish_exchanges,
                        consensus_rsi_14,
                        consensus_bollinger_percent_b,
                        consensus_trend_score,
                        consensus_macd_histogram,
                        current_price,
                        consensus_adx_14,
                        executed_at,
                    },
                )
                .collect();
            json_ok(&DecisionsResponse { items })
        }
        Err(err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "query_failed",
            format!("failed to fetch decisions: {}", err),
        ),
    }
}

async fn daily_trades_summary_handler(state: Arc<AppState>) -> impl Reply {
    let window_end_utc = Utc::now();
    let window_start_utc = start_of_day_utc(window_end_utc);

    if TradingMode::from_env().uses_simulated_wallet() {
        return json_ok(
            &build_paper_daily_trades_summary(&state, window_start_utc, window_end_utc).await,
        );
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

#[derive(serde::Deserialize)]
struct TradeQualityQuery {
    days: Option<i64>,
}

#[derive(Serialize)]
struct CloseReasonStat {
    reason: String,
    trades: i64,
    net_pnl: f64,
    wins: i64,
    avg_pnl_pct: f64,
}

#[derive(Serialize)]
struct FollowThroughStat {
    armed: bool,
    trades: i64,
    net_pnl: f64,
    wins: i64,
    avg_pnl_pct: f64,
}

#[derive(Serialize)]
struct PeakCapture {
    trailing_exits: i64,
    avg_peak_pct: f64,
    avg_realized_pct: f64,
    pct_captured: f64,
}

#[derive(Serialize)]
struct TradeQualityResponse {
    window_days: i64,
    closed_trades: i64,
    net_pnl: f64,
    win_rate: f64,
    by_close_reason: Vec<CloseReasonStat>,
    follow_through: Vec<FollowThroughStat>,
    peak_capture: PeakCapture,
    worst_symbols: Vec<SymbolPnlItem>,
}

// LIVE trade-quality metrics over realized (closed, paper) trades — NOT a backtest.
// Surfaces the diagnostics we actually validate by: close-reason attribution, entry
// follow-through (did the trade reach profit and arm the trail, or die flat?), and
// how much of the peak the trailing stop captured. Feeds the /analysis "Ao Vivo" tab.
async fn trade_quality_handler(query: TradeQualityQuery, state: Arc<AppState>) -> impl Reply {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };
    let days = query.days.unwrap_or(7).clamp(1, 365);

    // 1) close-reason attribution (and overall is derived from it).
    let by_reason = sqlx::query_as::<_, (Option<String>, i64, f64, i64, f64)>(
        "SELECT close_reason, COUNT(*)::bigint, COALESCE(SUM(pnl),0)::double precision,
                COUNT(*) FILTER (WHERE pnl > 0)::bigint, COALESCE(AVG(pnl_pct),0)::double precision
         FROM trades
         WHERE status='closed' AND paper_trade = TRUE
           AND closed_at >= NOW() - make_interval(days => $1::int)
         GROUP BY close_reason ORDER BY SUM(pnl) ASC",
    )
    .bind(days as i32)
    .fetch_all(pool)
    .await;

    // 2) entry follow-through: armed the trailing vs never (died flat/negative).
    let follow = sqlx::query_as::<_, (Option<bool>, i64, f64, i64, f64)>(
        "SELECT trailing_stop_activated, COUNT(*)::bigint, COALESCE(SUM(pnl),0)::double precision,
                COUNT(*) FILTER (WHERE pnl > 0)::bigint, COALESCE(AVG(pnl_pct),0)::double precision
         FROM trades
         WHERE status='closed' AND paper_trade = TRUE
           AND closed_at >= NOW() - make_interval(days => $1::int)
         GROUP BY trailing_stop_activated ORDER BY trailing_stop_activated",
    )
    .bind(days as i32)
    .fetch_all(pool)
    .await;

    // 3) trailing peak-capture: how much of the peak the trail locked.
    let cap = sqlx::query_as::<_, (i64, f64, f64)>(
        "WITH t AS (
            SELECT pnl_pct,
              CASE WHEN side='Long' THEN (trailing_stop_peak_price-entry_price)/entry_price*100
                   ELSE (entry_price-trailing_stop_peak_price)/entry_price*100 END AS peak_pct
            FROM trades
            WHERE status='closed' AND paper_trade = TRUE AND close_reason='trailing_stop'
              AND closed_at >= NOW() - make_interval(days => $1::int)
              AND trailing_stop_peak_price IS NOT NULL AND trailing_stop_peak_price <> entry_price)
         SELECT COUNT(*)::bigint, COALESCE(AVG(peak_pct),0)::double precision,
                COALESCE(AVG(pnl_pct),0)::double precision FROM t",
    )
    .bind(days as i32)
    .fetch_one(pool)
    .await;

    // 4) worst symbols (reuses the by-symbol ranking, windowed).
    let symbols = sqlx::query_as::<_, (String, f64, i64, i64, f64)>(
        "SELECT symbol, COALESCE(SUM(pnl),0)::double precision, COUNT(*)::bigint,
                COUNT(*) FILTER (WHERE pnl > 0)::bigint, COALESCE(AVG(pnl_pct),0)::double precision
         FROM trades
         WHERE status='closed' AND paper_trade = TRUE
           AND closed_at >= NOW() - make_interval(days => $1::int)
         GROUP BY symbol ORDER BY SUM(pnl) ASC LIMIT 12",
    )
    .bind(days as i32)
    .fetch_all(pool)
    .await;

    let (Ok(by_reason), Ok(follow), Ok(cap), Ok(symbols)) = (by_reason, follow, cap, symbols)
    else {
        return json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "query_failed",
            "failed to compute trade-quality metrics",
        );
    };

    let mut closed_trades = 0i64;
    let mut net_pnl = 0.0f64;
    let mut wins = 0i64;
    let by_close_reason: Vec<CloseReasonStat> = by_reason
        .into_iter()
        .map(|(reason, trades, net, w, avg)| {
            closed_trades += trades;
            net_pnl += net;
            wins += w;
            CloseReasonStat {
                reason: reason.unwrap_or_else(|| "(none)".to_string()),
                trades,
                net_pnl: round6(net),
                wins: w,
                avg_pnl_pct: round6(avg),
            }
        })
        .collect();

    let follow_through: Vec<FollowThroughStat> = follow
        .into_iter()
        .map(|(armed, trades, net, w, avg)| FollowThroughStat {
            armed: armed.unwrap_or(false),
            trades,
            net_pnl: round6(net),
            wins: w,
            avg_pnl_pct: round6(avg),
        })
        .collect();

    let (cap_n, cap_peak, cap_real) = cap;
    let peak_capture = PeakCapture {
        trailing_exits: cap_n,
        avg_peak_pct: round6(cap_peak),
        avg_realized_pct: round6(cap_real),
        pct_captured: if cap_peak.abs() > 1e-9 {
            round6(cap_real / cap_peak * 100.0)
        } else {
            0.0
        },
    };

    let worst_symbols: Vec<SymbolPnlItem> = symbols
        .into_iter()
        .map(|(symbol, net, trades, w, avg)| SymbolPnlItem {
            symbol,
            realized_pnl: round6(net),
            trades,
            wins: w,
            win_rate: round6(if trades > 0 {
                w as f64 / trades as f64
            } else {
                0.0
            }),
            avg_pnl_pct: round6(avg),
        })
        .collect();

    json_ok(&TradeQualityResponse {
        window_days: days,
        closed_trades,
        net_pnl: round6(net_pnl),
        win_rate: round6(if closed_trades > 0 {
            wins as f64 / closed_trades as f64
        } else {
            0.0
        }),
        by_close_reason,
        follow_through,
        peak_capture,
        worst_symbols,
    })
}

// Per-symbol realized-PnL ranking over closed trades, worst-first. Feeds the
// "broaden the universe, then prune the worst" workflow: with real decisions on,
// this is the objective scoreboard for which symbols to disable in pairs.yaml.
async fn symbol_pnl_handler(query: SymbolPnlQuery, state: Arc<AppState>) -> impl Reply {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let days = query.days.unwrap_or(14).clamp(1, 365);

    let rows = sqlx::query_as::<_, (String, f64, i64, i64, f64)>(
        "SELECT
             symbol,
             COALESCE(SUM(pnl), 0)::double precision AS realized_pnl,
             COUNT(*)::bigint AS trades,
             COUNT(*) FILTER (WHERE pnl > 0)::bigint AS wins,
             COALESCE(AVG(pnl_pct), 0)::double precision AS avg_pnl_pct
         FROM trades
         WHERE status = 'closed'
           AND closed_at >= NOW() - make_interval(days => $1::int)
         GROUP BY symbol
         ORDER BY realized_pnl ASC",
    )
    .bind(days as i32)
    .fetch_all(pool)
    .await;

    match rows {
        Ok(rows) => {
            let items = rows
                .into_iter()
                .map(|(symbol, realized_pnl, trades, wins, avg_pnl_pct)| {
                    let win_rate = if trades > 0 {
                        wins as f64 / trades as f64
                    } else {
                        0.0
                    };
                    SymbolPnlItem {
                        symbol,
                        realized_pnl: round6(realized_pnl),
                        trades,
                        wins,
                        win_rate: round6(win_rate),
                        avg_pnl_pct: round6(avg_pnl_pct),
                    }
                })
                .collect();
            json_ok(&SymbolPnlResponse {
                window_days: days,
                items,
            })
        }
        Err(err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "query_failed",
            format!("failed to fetch symbol pnl: {}", err),
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
    paper_only: bool,
) -> Result<PerformanceWindow, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64, i64, f64)>(
        "SELECT
             COUNT(*)::bigint,
             COUNT(*) FILTER (WHERE COALESCE(pnl, 0) > 0)::bigint,
             COALESCE(SUM(COALESCE(pnl, 0))::double precision, 0)
         FROM trades
         WHERE status = 'closed'
           AND (NOT $3 OR paper_trade = TRUE)
           AND closed_at IS NOT NULL
           AND closed_at >= $1
           AND closed_at < $2",
    )
    .bind(window_start_utc)
    .bind(window_end_utc)
    .bind(paper_only)
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

async fn build_paper_performance_response(
    state: Arc<AppState>,
) -> warp::reply::WithStatus<WarpJson> {
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
        true,
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
        true,
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
        true,
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
        last_24h: summarize_closed_pnl_window(
            &items,
            window_end_utc - ChronoDuration::hours(24),
            window_end_utc,
        ),
        last_7d: summarize_closed_pnl_window(
            &items,
            window_end_utc - ChronoDuration::days(7),
            window_end_utc,
        ),
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
           AND paper_trade = TRUE
           AND opened_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let open_exposure_usdt = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT COALESCE(SUM(quantity * entry_price)::double precision, 0)
         FROM trades
         WHERE status = 'open'
           AND paper_trade = TRUE",
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
           AND paper_trade = TRUE
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
        let client = bybit_client::BybitClient::from_env();
        return json_ok(&BybitPrivateHealthResponse {
            name: "bybit-private",
            ok: true,
            status: 200,
            latency_ms: 0,
            url: format!(
                "{}/v5/account/wallet-balance?accountType={}",
                client.base_url,
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
    let account_type =
        read_non_empty_env("BYBIT_ACCOUNT_TYPE").unwrap_or_else(|| "UNIFIED".to_string());

    let client = bybit_client::BybitClient::from_env();
    let url_base = client.base_url.clone();
    let result = client.wallet_balance(&account_type).await;
    let url = format!(
        "{}/v5/account/wallet-balance?accountType={}",
        url_base, account_type
    );

    if let Some(ref err) = result.error {
        if result.status != 0 && !err.contains("missing BYBIT_API") {
            tracing::warn!(service = "api", error = %err, "Bybit wallet fetch error");
        }
    }

    BybitWalletFetchResult {
        checked_at,
        account_type,
        url,
        status: result.status,
        latency_ms: result.latency_ms,
        ret_code: result.ret_code,
        ret_msg: result.ret_msg,
        body: result.body,
        error: result.error,
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
    let start_ms = window_start_utc.timestamp_millis();
    let end_ms = window_end_utc.timestamp_millis();

    let client = bybit_client::BybitClient::from_env();
    let url_base = client.base_url.clone();
    let result = client
        .order_history("linear", "USDT", start_ms, end_ms, 50)
        .await;
    let url = format!(
        "{}/v5/order/history?category=linear&settleCoin=USDT&startTime={}&endTime={}&limit=50",
        url_base, start_ms, end_ms
    );

    BybitOrderHistoryFetchResult {
        checked_at,
        url,
        status: result.status,
        ret_code: result.ret_code,
        ret_msg: result.ret_msg,
        body: result.body,
        error: result.error,
    }
}

fn extract_bybit_today_trade_count(body: &Value) -> i64 {
    body.get("result")
        .and_then(|result| result.get("list"))
        .and_then(|list| list.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("cumExecQty")
                        .and_then(json_number)
                        .map(|value| value > 0.0)
                        .unwrap_or_else(|| {
                            item.get("orderStatus")
                                .and_then(|value| value.as_str())
                                .map(|status| {
                                    matches!(status, "Filled" | "PartiallyFilledCanceled")
                                })
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
    let start_ms = window_start_utc.timestamp_millis();
    let end_ms = window_end_utc.timestamp_millis();

    let client = bybit_client::BybitClient::from_env();
    let result = client
        .closed_pnl("linear", "USDT", start_ms, end_ms, limit, cursor)
        .await;

    BybitClosedPnlFetchResult {
        status: result.status,
        ret_code: result.ret_code,
        ret_msg: result.ret_msg,
        body: result.body,
        error: result.error,
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
            let result =
                fetch_bybit_closed_pnl_page(chunk_start, chunk_end, page_size, cursor.as_deref())
                    .await;
            if result.status != 200 || result.ret_code != Some(0) {
                return Err(result.error.or(result.ret_msg).unwrap_or_else(|| {
                    format!("http={} retCode={:?}", result.status, result.ret_code)
                }));
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

async fn fetch_bybit_position_page(
    settle_coin: &str,
    cursor: Option<&str>,
) -> BybitPositionFetchResult {
    let client = bybit_client::BybitClient::from_env();
    let result = client
        .position_list("linear", settle_coin, 200, cursor)
        .await;

    BybitPositionFetchResult {
        status: result.status,
        ret_code: result.ret_code,
        ret_msg: result.ret_msg,
        body: result.body,
        error: result.error,
    }
}

async fn fetch_bybit_positions() -> Result<Vec<Value>, String> {
    let mut combined = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let result = fetch_bybit_position_page("USDT", cursor.as_deref()).await;
        if result.status != 200 || result.ret_code != Some(0) {
            return Err(result.error.or(result.ret_msg).unwrap_or_else(|| {
                format!("http={} retCode={:?}", result.status, result.ret_code)
            }));
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

    let symbol = item
        .get("symbol")
        .and_then(|value| value.as_str())?
        .to_string();
    let raw_side = item
        .get("side")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let side = map_bybit_side(Some(raw_side));
    let entry_price = item.get("avgPrice").and_then(json_number).unwrap_or(0.0);
    let notional_usdt = item
        .get("positionValue")
        .and_then(json_number)
        .or_else(|| item.get("positionIM").and_then(json_number))
        .unwrap_or(quantity * entry_price);

    let (stop_loss_price, trailing_activation_price, fixed_take_profit_price, break_even_price) =
        resolve_position_triggers(state, &symbol, &side, entry_price);

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
        opened_at: parse_bybit_timestamp_millis(item.get("createdTime")).unwrap_or_else(Utc::now),
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
    let symbol = item
        .get("symbol")
        .and_then(|value| value.as_str())?
        .to_string();
    let entry_price = item
        .get("avgEntryPrice")
        .and_then(json_number)
        .unwrap_or(0.0);
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
        close_reason: None,
        duration_seconds: Some((closed_at - opened_at).num_seconds().max(0)),
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

// ─── Live stream (WebSocket) ────────────────────────────────────────────────
// Bridges Redis pub/sub (viper:market_data, viper:decisions) to connected
// browsers so the dashboard updates by push instead of polling.

fn with_broadcast(
    tx: tokio::sync::broadcast::Sender<String>,
) -> impl Filter<Extract = (tokio::sync::broadcast::Sender<String>,), Error = std::convert::Infallible>
       + Clone {
    warp::any().map(move || tx.clone())
}

fn resolve_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| {
        let host = std::env::var("REDIS_HOST").unwrap_or_else(|_| "redis".to_string());
        let port = std::env::var("REDIS_PORT").unwrap_or_else(|_| "6379".to_string());
        format!("redis://{host}:{port}")
    })
}

/// Subscribe to the strategy/market Redis channels and fan out every message to
/// the broadcast channel (consumed by each WS client). Reconnects on error.
async fn redis_stream_subscriber(tx: tokio::sync::broadcast::Sender<String>) {
    use futures_util::StreamExt;
    let url = resolve_redis_url();
    loop {
        let result: redis::RedisResult<()> = async {
            let client = redis::Client::open(url.as_str())?;
            let mut pubsub = client.get_async_pubsub().await?;
            pubsub.subscribe("viper:market_data").await?;
            pubsub.subscribe("viper:decisions").await?;
            tracing::info!("WS bridge subscribed to Redis market/decision channels");
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                if let Ok(payload) = msg.get_payload::<String>() {
                    let _ = tx.send(payload);
                }
            }
            Ok(())
        }
        .await;
        if let Err(err) = result {
            tracing::warn!(error = %err, "WS Redis subscriber dropped; retrying in 3s");
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

async fn ws_client(ws: warp::ws::WebSocket, tx: tokio::sync::broadcast::Sender<String>) {
    use futures_util::{SinkExt, StreamExt};
    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut rx = tx.subscribe();
    let forward = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if ws_tx.send(warp::ws::Message::text(msg)).await.is_err() {
                break;
            }
        }
    });
    // Drain inbound frames (we don't expect any) so we notice the socket closing.
    while let Some(Ok(_)) = ws_rx.next().await {}
    forward.abort();
}

pub async fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "viper_api=info".into()),
        )
        .json()
        .init();

    let db_pool = if let Some(database_url) = resolve_database_url() {
        match PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                tracing::info!("Connected to PostgreSQL for API queries");
                Some(pool)
            }
            Err(err) => {
                tracing::error!(error = %err, "Failed to connect PostgreSQL");
                None
            }
        }
    } else {
        tracing::warn!("Database env not configured; DB-backed endpoints degraded");
        None
    };

    let trading_mode = TradingMode::from_env();

    let state = Arc::new(AppState {
        db_pool,
        trading_mode: trading_mode.as_status_label().to_string(),
        trading_profile: std::env::var("TRADING_PROFILE").unwrap_or_else(|_| "MEDIUM".to_string()),
        trade_profile_label: trading_mode.trade_profile_label().to_string(),
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

    // Live stream: one Redis subscriber fans out to all WS clients.
    let (market_tx, _) = tokio::sync::broadcast::channel::<String>(512);
    tokio::spawn(redis_stream_subscriber(market_tx.clone()));

    let ws_stream = warp::path("ws")
        .and(warp::ws())
        .and(with_broadcast(market_tx.clone()))
        .map(
            |ws: warp::ws::Ws, tx: tokio::sync::broadcast::Sender<String>| {
                ws.on_upgrade(move |socket| ws_client(socket, tx))
            },
        );

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

    let decisions = api_v1
        .and(warp::path("decisions"))
        .and(warp::path::end())
        .and(warp::query::<DecisionsQuery>())
        .and(with_state(state.clone()))
        .then(decisions_handler);

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

    let symbol_pnl = api_v1
        .and(warp::path("performance"))
        .and(warp::path("by-symbol"))
        .and(warp::path::end())
        .and(warp::query::<SymbolPnlQuery>())
        .and(with_state(state.clone()))
        .then(symbol_pnl_handler);

    let trade_quality = api_v1
        .and(warp::path("performance"))
        .and(warp::path("trade-quality"))
        .and(warp::path::end())
        .and(warp::query::<TradeQualityQuery>())
        .and(with_state(state.clone()))
        .then(trade_quality_handler);

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

    // Boxed sub-groups: collapsing each group's filter type with `.boxed()` keeps
    // the combined `.or(...)` tree shallow, avoiding warp's deep-type recursion
    // overflow (E0275) that a single 25-deep chain triggers in release builds.
    let public = health
        .or(status)
        .or(positions)
        .or(trades)
        .or(daily_trades_summary)
        .boxed();
    let data = decisions
        .or(events)
        .or(performance)
        .or(symbol_pnl)
        .or(trade_quality)
        .or(risk_kpis)
        .or(bybit_private_health)
        .or(bybit_wallet)
        .boxed();
    let control = control_state
        .or(kill_switch)
        .or(executor_control)
        .or(risk_limits_control)
        .boxed();
    let misc = legacy_root.or(legacy_health).or(ws_stream).boxed();

    let routes = public
        .or(data)
        .or(control)
        .or(misc)
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
            tracing::info!("Received shutdown signal, stopping viper-api");
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
