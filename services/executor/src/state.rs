use serde_json::json;
use sqlx::PgPool;
use viper_domain::StrategyDecisionEvent;

use crate::*;

pub(crate) async fn remember_processed(state: &ExecutorState, source_event_id: &str) {
    let mut seen = state.processed_in_memory.lock().await;
    seen.insert(source_event_id.to_string());
}

pub(crate) async fn claim_processed_event(
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

        upsert_decision_audit(pool, source_event_id, event, "claimed", None, None).await?;
        remember_processed(state, source_event_id).await;
        return Ok(true);
    }

    let mut seen = state.processed_in_memory.lock().await;
    Ok(seen.insert(source_event_id.to_string()))
}

pub(crate) async fn upsert_decision_audit(
    pool: &PgPool,
    source_event_id: &str,
    event: &StrategyDecisionEvent,
    status: &str,
    bybit_order_id: Option<&str>,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let payload = serde_json::to_value(event).unwrap_or_else(|_| json!({}));

    sqlx::query(
        "INSERT INTO strategy_decision_audit (
            source_event_id,
            decision_event_id,
            schema_version,
            symbol,
            action,
            reason,
            smart_copy_compatible,
            decision_hash,
            executor_status,
            bybit_order_id,
            error,
            payload
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        ON CONFLICT (decision_event_id) DO UPDATE SET
            source_event_id = EXCLUDED.source_event_id,
            schema_version = EXCLUDED.schema_version,
            symbol = EXCLUDED.symbol,
            action = EXCLUDED.action,
            reason = EXCLUDED.reason,
            smart_copy_compatible = EXCLUDED.smart_copy_compatible,
            decision_hash = EXCLUDED.decision_hash,
            executor_status = EXCLUDED.executor_status,
            bybit_order_id = EXCLUDED.bybit_order_id,
            error = EXCLUDED.error,
            payload = EXCLUDED.payload,
            updated_at = NOW()",
    )
    .bind(source_event_id)
    .bind(&event.event_id)
    .bind(&event.schema_version)
    .bind(&event.decision.symbol)
    .bind(&event.decision.action)
    .bind(&event.decision.reason)
    .bind(event.decision.smart_copy_compatible)
    .bind(decision_hash(event))
    .bind(status)
    .bind(bybit_order_id)
    .bind(error)
    .bind(payload)
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn mark_processed(
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

        upsert_decision_audit(pool, source_event_id, event, status, bybit_order_id, error).await?;
    }

    remember_processed(state, source_event_id).await;
    Ok(())
}

pub(crate) async fn fetch_latest_control_flag(
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

pub(crate) async fn fetch_runtime_controls(
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

pub(crate) async fn persist_trade(
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

/// Margem livre da carteira simulada, em USDT.
///
/// Espelha a conta que a API mostra ao operador: capital inicial, mais o
/// resultado já realizado, menos taxas e funding, menos a margem presa nas
/// posições abertas. Uma conta real recusa a ordem quando isso não cobre a
/// margem exigida — sem a checagem, o paper abria posição com saldo que não
/// existe e media uma estratégia impossível de executar.
pub(crate) async fn available_margin_usdt(
    state: &ExecutorState,
    initial_capital_usd: f64,
) -> Result<f64, sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(initial_capital_usd);
    };

    let (realized_pnl, fees, funding_paid) = sqlx::query_as::<_, (f64, f64, f64)>(
        "SELECT
            COALESCE(SUM(pnl), 0)::double precision,
            COALESCE(SUM(fees), 0)::double precision,
            COALESCE(SUM(funding_paid), 0)::double precision
         FROM trades
         WHERE status <> 'open'",
    )
    .fetch_one(pool)
    .await?;

    let used_margin = sqlx::query_scalar::<_, f64>(
        "SELECT COALESCE(SUM((quantity * entry_price) / NULLIF(leverage, 0)), 0)::double precision
         FROM trades
         WHERE status = 'open'",
    )
    .fetch_one(pool)
    .await?;

    Ok(initial_capital_usd + realized_pnl - fees - funding_paid - used_margin)
}

/// Queda percentual do equity desde o seu ponto mais alto (high-water mark).
///
/// Mede sobre resultado REALIZADO — a curva de equity fechada. Posições abertas
/// ficam de fora de propósito: incluí-las faria o disjuntor oscilar com o preço
/// e disparar por ruído, quando o que ele precisa detectar é perda consolidada.
///
/// Retorna `(drawdown_pct, pico, equity_atual)`, com o drawdown em FRAÇÃO.
pub(crate) async fn equity_drawdown(
    state: &ExecutorState,
    initial_capital_usd: f64,
) -> Result<(f64, f64, f64), sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok((0.0, initial_capital_usd, initial_capital_usd));
    };

    // A curva começa no capital inicial (antes do primeiro trade), senão o pico
    // seria o primeiro fechamento e uma conta que só perdeu teria drawdown zero.
    let (peak, current) = sqlx::query_as::<_, (f64, f64)>(
        "WITH curve AS (
            SELECT closed_at,
                   trade_id,
                   $1::double precision + SUM(
                       COALESCE(pnl, 0) - COALESCE(fees, 0) - COALESCE(funding_paid, 0)
                   ) OVER (ORDER BY closed_at, trade_id) AS equity
            FROM trades
            WHERE status <> 'open' AND closed_at IS NOT NULL
         )
         SELECT
            GREATEST(
                $1::double precision,
                COALESCE((SELECT MAX(equity) FROM curve), $1::double precision)
            ),
            COALESCE(
                (SELECT equity FROM curve ORDER BY closed_at DESC, trade_id DESC LIMIT 1),
                $1::double precision
            )",
    )
    .bind(initial_capital_usd)
    .fetch_one(pool)
    .await?;

    if peak <= 0.0 {
        return Ok((0.0, peak, current));
    }
    let drawdown = ((peak - current) / peak).max(0.0);
    Ok((drawdown, peak, current))
}

/// Registra o disparo do disjuntor para auditoria.
pub(crate) async fn record_circuit_breaker(
    state: &ExecutorState,
    breaker_type: &str,
    trigger_value: f64,
    threshold_value: f64,
    action_taken: &str,
    metadata: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO circuit_breaker_events
             (breaker_type, trigger_value, threshold_value, action_taken, metadata)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(breaker_type)
    .bind(trigger_value)
    .bind(threshold_value)
    .bind(action_taken)
    .bind(metadata)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn count_open_trades(state: &ExecutorState) -> Result<i64, sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(0);
    };

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM trades WHERE status = 'open'")
            .fetch_one(pool)
            .await?;

    Ok(count)
}

pub(crate) async fn open_trade_side_for_symbol(
    state: &ExecutorState,
    symbol: &str,
) -> Result<Option<String>, sqlx::Error> {
    let Some(pool) = &state.db_pool else {
        return Ok(None);
    };

    let side: Option<String> = sqlx::query_scalar(
        "SELECT side FROM trades
         WHERE status = 'open' AND symbol = $1
         ORDER BY opened_at ASC
         LIMIT 1",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await?;

    Ok(side)
}
