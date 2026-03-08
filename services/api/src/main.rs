use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::watch;
use warp::http::StatusCode;
use warp::reply::Json as WarpJson;
use warp::Filter;

#[derive(Clone)]
struct AppState {
    db_pool: Option<PgPool>,
    trading_mode: String,
    trading_profile: String,
    operator_api_token: Option<String>,
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
    risk_status: String,
    critical_reconciliation_events_15m: i64,
    kill_switch: KillSwitchStatus,
}

#[derive(Serialize)]
struct KillSwitchStatus {
    enabled: bool,
    reason: Option<String>,
    actor: Option<String>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct PositionItem {
    symbol: String,
    side: String,
    quantity: f64,
    notional_usdt: f64,
}

#[derive(Serialize)]
struct PositionsResponse {
    items: Vec<PositionItem>,
}

#[derive(Deserialize)]
struct TradesQuery {
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
struct PerformanceWindow {
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

fn read_non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn clamp_limit(limit: Option<u32>) -> i64 {
    let raw = limit.unwrap_or(50);
    raw.clamp(1, 200) as i64
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

async fn health_handler(state: Arc<AppState>) -> impl warp::Reply {
    let payload = HealthResponse {
        status: "ok",
        db_connected: state.db_pool.is_some(),
    };
    json_ok(&payload)
}

async fn status_handler(state: Arc<AppState>) -> impl warp::Reply {
    let mut kill_switch = KillSwitchStatus {
        enabled: false,
        reason: None,
        actor: None,
        updated_at: None,
    };
    let mut critical_recon = 0_i64;

    if let Some(pool) = &state.db_pool {
        if let Ok(v) = fetch_kill_switch_status(pool).await {
            kill_switch = v;
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
        risk_status,
        critical_reconciliation_events_15m: critical_recon,
        kill_switch,
    };

    json_ok(&payload)
}

async fn positions_handler(state: Arc<AppState>) -> impl warp::Reply {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let rows = sqlx::query_as::<_, (String, String, f64, f64)>(
        "SELECT
             symbol,
             side,
             COALESCE(SUM(quantity)::double precision, 0),
             COALESCE(SUM(quantity * entry_price)::double precision, 0)
         FROM trades
         WHERE status = 'open'
         GROUP BY symbol, side
         ORDER BY symbol, side",
    )
    .fetch_all(pool)
    .await;

    match rows {
        Ok(rows) => {
            let items = rows
                .into_iter()
                .map(|(symbol, side, quantity, notional_usdt)| PositionItem {
                    symbol,
                    side,
                    quantity,
                    notional_usdt,
                })
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

async fn trades_handler(query: TradesQuery, state: Arc<AppState>) -> impl warp::Reply {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let limit = clamp_limit(query.limit);

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

async fn fetch_window(pool: &PgPool, hours: i32) -> Result<PerformanceWindow, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64, i64, f64)>(
        "SELECT
             COUNT(*)::bigint,
             COUNT(*) FILTER (WHERE COALESCE(pnl, 0) > 0)::bigint,
             COALESCE(SUM(COALESCE(pnl, 0))::double precision, 0)
         FROM trades
         WHERE status = 'closed'
           AND closed_at IS NOT NULL
           AND closed_at >= NOW() - ($1 * INTERVAL '1 hour')",
    )
    .bind(hours)
    .fetch_one(pool)
    .await?;

    let (total_trades, winning_trades, total_pnl) = row;
    let win_rate = if total_trades > 0 {
        winning_trades as f64 / total_trades as f64
    } else {
        0.0
    };

    Ok(PerformanceWindow {
        total_trades,
        winning_trades,
        win_rate,
        total_pnl,
    })
}

async fn performance_handler(state: Arc<AppState>) -> impl warp::Reply {
    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let last_24h = match fetch_window(pool, 24).await {
        Ok(v) => v,
        Err(err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "query_failed",
                format!("failed to fetch 24h performance: {}", err),
            )
        }
    };

    let last_7d = match fetch_window(pool, 24 * 7).await {
        Ok(v) => v,
        Err(err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "query_failed",
                format!("failed to fetch 7d performance: {}", err),
            )
        }
    };

    let last_30d = match fetch_window(pool, 24 * 30).await {
        Ok(v) => v,
        Err(err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "query_failed",
                format!("failed to fetch 30d performance: {}", err),
            )
        }
    };

    let max_drawdown_30d = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT MAX(max_drawdown)::double precision
         FROM daily_metrics
         WHERE date >= CURRENT_DATE - INTERVAL '30 days'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .flatten();

    let payload = PerformanceResponse {
        last_24h,
        last_7d,
        last_30d,
        max_drawdown_30d,
    };

    json_ok(&payload)
}

async fn kill_switch_handler(
    req: KillSwitchRequest,
    token_header: Option<String>,
    operator_id_header: Option<String>,
    state: Arc<AppState>,
) -> impl warp::Reply {
    let Some(configured_token) = &state.operator_api_token else {
        return json_err(
            StatusCode::FORBIDDEN,
            "auth_not_configured",
            "operator control auth is not configured",
        );
    };

    if token_header.as_deref() != Some(configured_token.as_str()) {
        return json_err(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "missing or invalid operator token",
        );
    }

    let Some(pool) = &state.db_pool else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "db_unavailable",
            "database is not connected",
        );
    };

    let actor = operator_id_header.unwrap_or_else(|| "operator".to_string());
    let reason = req
        .reason
        .clone()
        .unwrap_or_else(|| "manual_api_control".to_string());

    let event_data = json!({
        "enabled": req.enabled,
        "reason": reason,
        "actor": actor,
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

fn with_state(
    state: Arc<AppState>,
) -> impl Filter<Extract = (Arc<AppState>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
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
        trading_mode: std::env::var("TRADING_MODE").unwrap_or_else(|_| "paper".to_string()),
        trading_profile: std::env::var("TRADING_PROFILE").unwrap_or_else(|_| "MEDIUM".to_string()),
        operator_api_token: read_non_empty_env("OPERATOR_API_TOKEN"),
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

    let performance = api_v1
        .and(warp::path("performance"))
        .and(warp::path::end())
        .and(with_state(state.clone()))
        .then(performance_handler);

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

    let legacy_root = warp::path::end().map(|| "Hello, ViperTrade API!");
    let legacy_health = warp::path("health")
        .and(warp::path::end())
        .map(|| warp::reply::json(&"OK"));

    let routes = health
        .or(status)
        .or(positions)
        .or(trades)
        .or(performance)
        .or(kill_switch)
        .or(legacy_root)
        .or(legacy_health);

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
    use super::clamp_limit;

    #[test]
    fn clamp_limit_enforces_bounds() {
        assert_eq!(clamp_limit(None), 50);
        assert_eq!(clamp_limit(Some(0)), 1);
        assert_eq!(clamp_limit(Some(10)), 10);
        assert_eq!(clamp_limit(Some(500)), 200);
    }
}
