use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use viper_domain::StrategyDecision;

use crate::*;

pub(crate) async fn local_open_qty(
    pool: &PgPool,
    symbol: &str,
    side: &str,
) -> Result<f64, sqlx::Error> {
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

pub(crate) fn reconciliation_event_meta(fix_applied: bool) -> (&'static str, &'static str) {
    if fix_applied {
        ("executor_reconciliation_fix_applied", "info")
    } else {
        ("executor_reconciliation_detected", "warning")
    }
}

pub(crate) async fn record_reconciliation_event(
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

pub(crate) async fn apply_reconciliation_reduce_local(
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

fn check_reconcile_daily_limit(
    counts: &Arc<Mutex<HashMap<String, (String, i64)>>>,
    symbol: &str,
    side: &str,
    max_daily: i64,
) -> bool {
    let key = format!("{}:{}", symbol, side);
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let mut map = counts.blocking_lock();
    let current = map.get(&key);
    let (allowed, next_count) = match current {
        Some((date, count)) if date == &today => (*count < max_daily, count + 1),
        _ => (true, 1),
    };
    if allowed {
        map.insert(key, (today, next_count));
    }
    allowed
}

async fn submit_reconciliation_order(
    state: &ExecutorState,
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
    side: &str,
    drift: f64,
) -> Result<String, Box<dyn Error>> {
    let action = if drift > 0.0 {
        format!("ENTER_{}", side.to_uppercase())
    } else {
        format!("CLOSE_{}", side.to_uppercase())
    };

    let abs_qty = drift.abs();

    let price = fetch_bybit_last_price(http, cfg, symbol)
        .await
        .unwrap_or(0.0);

    let decision = StrategyDecision {
        action,
        symbol: symbol.to_string(),
        quantity: abs_qty,
        leverage: 1.0,
        entry_price: price,
        stop_loss: 0.0,
        take_profit: 0.0,
        reason: "reconciliation_auto_fix".to_string(),
        smart_copy_compatible: false,
    };

    let event = viper_domain::StrategyDecisionEvent::new("reconciliation".to_string(), decision);

    submit_market_order(state, http, cfg, &event).await
}

pub(crate) async fn run_reconciliation_tick(
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
            let bybit_qty = match fetch_bybit_position_qty(http, cfg, &symbol, side).await {
                Ok(q) => q,
                Err(e) => {
                    tracing::warn!(symbol = %symbol, side = %side, error = %e, "Reconciliation fetch bybit position failed");
                    continue;
                }
            };
            let diff = (local_qty - bybit_qty).abs();

            if diff <= 1e-6 {
                continue;
            }

            let drift = bybit_qty - local_qty;

            tracing::warn!(
                symbol = %symbol, side = %side,
                local_qty = local_qty, bybit_qty = bybit_qty,
                diff = diff, drift = drift,
                fix_mode = ?cfg.reconcile_fix,
                auto_fix = cfg.reconcile_auto_fix,
                "Reconciliation diff"
            );

            if cfg.reconcile_auto_fix && cfg.trading_mode == TradingMode::Mainnet {
                let max_correction = local_qty.max(bybit_qty) * cfg.reconcile_max_correction_pct;
                if diff > max_correction {
                    tracing::warn!(
                        symbol = %symbol, side = %side,
                        diff = diff, max_correction = max_correction,
                        pct = cfg.reconcile_max_correction_pct,
                        "Reconciliation diff exceeds max correction pct, skipping auto-fix"
                    );
                    let _ = record_reconciliation_event(
                        pool, &symbol, side, local_qty, bybit_qty, diff, false,
                    )
                    .await;
                    continue;
                }

                if !check_reconcile_daily_limit(
                    &state.reconcile_daily_counts,
                    &symbol,
                    side,
                    cfg.reconcile_max_daily,
                ) {
                    tracing::warn!(
                        symbol = %symbol, side = %side,
                        max_daily = cfg.reconcile_max_daily,
                        "Reconciliation daily limit reached, skipping auto-fix"
                    );
                    let _ = record_reconciliation_event(
                        pool, &symbol, side, local_qty, bybit_qty, diff, false,
                    )
                    .await;
                    continue;
                }

                let order_id_str =
                    match submit_reconciliation_order(state, http, cfg, &symbol, side, drift).await
                    {
                        Ok(id) => (Some(id), None),
                        Err(e) => (None, Some(e.to_string())),
                    };
                let (order_id, err_str) = order_id_str;
                if let Some(error_msg) = err_str {
                    tracing::error!(
                        symbol = %symbol, side = %side,
                        drift = drift, error = %error_msg,
                        "Reconciliation auto-fix order failed"
                    );
                    let _ = record_reconciliation_event(
                        pool, &symbol, side, local_qty, bybit_qty, diff, false,
                    )
                    .await;
                    continue;
                }
                let order_id = order_id.expect("Ok must produce Some order_id");
                tracing::info!(
                    symbol = %symbol, side = %side,
                    drift = drift, order_id = %order_id,
                    "Reconciliation auto-fix order submitted"
                );
                if drift < 0.0 {
                    match apply_reconciliation_reduce_local(pool, &symbol, side, bybit_qty).await {
                        Ok((before, after)) => {
                            let _ = record_reconciliation_event(
                                pool, &symbol, side, before, bybit_qty, diff, true,
                            )
                            .await;
                            tracing::info!(
                                symbol = %symbol, side = %side,
                                before_local = before, target_bybit = bybit_qty,
                                after_local = after, order_id = %order_id,
                                "Reconciliation auto-fix applied (reduce local + bybit order)"
                            );
                        }
                        Err(reduce_err) => {
                            let reduce_err_str = reduce_err.to_string();
                            tracing::warn!(
                                symbol = %symbol, side = %side,
                                error = %reduce_err_str, order_id = %order_id,
                                "Reconciliation auto-fix: local reduce failed after bybit order"
                            );
                        }
                    }
                } else {
                    let _ = record_reconciliation_event(
                        pool, &symbol, side, local_qty, bybit_qty, diff, true,
                    )
                    .await;
                    tracing::info!(
                        symbol = %symbol, side = %side,
                        local_qty = local_qty, bybit_qty = bybit_qty,
                        drift = drift, order_id = %order_id,
                        "Reconciliation auto-fix applied (bybit order only)"
                    );
                }
            } else if cfg.reconcile_fix {
                if local_qty > bybit_qty + 1e-6 {
                    match apply_reconciliation_reduce_local(pool, &symbol, side, bybit_qty).await {
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
                            tracing::info!(
                                symbol = %symbol, side = %side,
                                before_local = before, target_bybit = bybit_qty,
                                after_local = after, "Reconciliation reduce-local fix applied"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(symbol = %symbol, side = %side, error = %e, "Reconciliation reduce-local fix failed");
                        }
                    }
                } else {
                    let _ = record_reconciliation_event(
                        pool, &symbol, side, local_qty, bybit_qty, diff, false,
                    )
                    .await;
                    tracing::warn!(
                        symbol = %symbol, side = %side,
                        "Reconciliation reduce-local fix skipped (bybit > local, needs bybit order)"
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

    Ok(())
}
