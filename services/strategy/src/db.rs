use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use viper_domain::{MarketSignalEvent, StrategyDecision};

use crate::types::OpenTradeSnapshot;

pub(crate) async fn fetch_open_trade_for_symbol(
    pool: &PgPool,
    symbol: &str,
) -> Result<Option<OpenTradeSnapshot>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, f64, f64, DateTime<Utc>, bool, f64, f64)>(
        "SELECT
            trade_id::text,
            side,
            quantity::double precision,
            entry_price::double precision,
            opened_at,
            COALESCE(trailing_stop_activated, false),
            COALESCE(trailing_stop_peak_price::double precision, entry_price::double precision),
            COALESCE(trailing_stop_final_distance_pct::double precision, 0)
        FROM trades
        WHERE status = 'open' AND symbol = $1
        ORDER BY opened_at ASC
        LIMIT 1",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            trade_id,
            side,
            quantity,
            entry_price,
            opened_at,
            trailing_stop_activated,
            trailing_stop_peak_price,
            trailing_stop_final_distance_pct,
        )| OpenTradeSnapshot {
            trade_id,
            side,
            quantity,
            entry_price,
            opened_at,
            trailing_stop_activated,
            trailing_stop_peak_price,
            trailing_stop_final_distance_pct,
        },
    ))
}

pub(crate) async fn update_trade_trailing_state(
    pool: &PgPool,
    trade_id: &str,
    activated: bool,
    peak_price: f64,
    trail_pct: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE trades
         SET trailing_stop_activated = $2,
             trailing_stop_peak_price = $3,
             trailing_stop_final_distance_pct = $4
         WHERE trade_id::text = $1",
    )
    .bind(trade_id)
    .bind(activated)
    .bind(peak_price)
    .bind(trail_pct)
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn has_recent_stop_loss_for_symbol(
    pool: &PgPool,
    symbol: &str,
    cooldown_minutes: i64,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM trades
         WHERE symbol = $1
           AND status = 'closed'
           AND close_reason = 'stop_loss'
           AND closed_at >= NOW() - make_interval(mins => $2::int)",
    )
    .bind(symbol)
    .bind(cooldown_minutes)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

/// Returns true when a CLOSE_* decision for `symbol` was already emitted within
/// the last `within_minutes` minutes.
///
/// The strategy persists every decision to `tupa_audit_logs` synchronously before
/// publishing it. If the strategy restarts after emitting a CLOSE but before the
/// executor processes it, the position is still `open`, so the open-trade
/// re-evaluation would emit the same CLOSE again (duplicate exit). Guarding on a
/// recently-emitted CLOSE for the symbol prevents that — there is at most one
/// open position per symbol, so matching by symbol is sufficient.
pub(crate) async fn has_recent_close_decision_for_symbol(
    pool: &PgPool,
    symbol: &str,
    within_minutes: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM tupa_audit_logs
            WHERE executed_at >= NOW() - make_interval(mins => $2::int)
              AND output_data->'final_decision'->>'symbol' = $1
              AND output_data->'final_decision'->>'action' LIKE 'CLOSE\\_%'
         )",
    )
    .bind(symbol)
    .bind(within_minutes)
    .fetch_one(pool)
    .await
}

pub(crate) async fn count_open_trades(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM trades
         WHERE status = 'open'",
    )
    .fetch_one(pool)
    .await
}

pub(crate) fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub(crate) fn sha256_hex_json(value: &Value) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256_hex_bytes(&bytes))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn persist_tupa_audit_log(
    pool: &PgPool,
    signal_event: &MarketSignalEvent,
    pipeline_name: &str,
    pipeline_version: &str,
    input_data: &Value,
    output_data: &Value,
    constraints_results: &Value,
    decision: &StrategyDecision,
    execution_time_ms: i32,
) -> Result<(), sqlx::Error> {
    let input_hash = sha256_hex_json(input_data).unwrap_or_else(|_| signal_event.event_id.clone());
    let output_hash =
        sha256_hex_json(output_data).unwrap_or_else(|_| signal_event.signal.symbol.clone());
    let decision_value = serde_json::to_value(decision).unwrap_or_else(|_| json!({}));
    let decision_hash =
        sha256_hex_json(&decision_value).unwrap_or_else(|_| signal_event.signal.symbol.clone());
    let environment = json!({
        "service": "viper-strategy",
        "strategy_version": env!("CARGO_PKG_VERSION"),
        "decision_schema_version": viper_domain::SCHEMA_VERSION,
        "runtime_mode": "in_process_tupa",
    });

    sqlx::query(
        "INSERT INTO tupa_audit_logs (
            execution_id,
            pipeline_name,
            pipeline_version,
            input_hash,
            output_hash,
            decision_hash,
            input_data,
            output_data,
            constraints_results,
            execution_time_ms,
            memory_used_kb,
            environment
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(&signal_event.event_id)
    .bind(pipeline_name)
    .bind(pipeline_version)
    .bind(input_hash)
    .bind(output_hash)
    .bind(decision_hash)
    .bind(input_data)
    .bind(output_data)
    .bind(constraints_results)
    .bind(execution_time_ms)
    .bind(Option::<i32>::None)
    .bind(environment)
    .execute(pool)
    .await?;

    Ok(())
}
