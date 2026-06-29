use serde_json::{json, Value};
use std::error::Error;
use std::time::Duration;
use viper_domain::StrategyDecisionEvent;

use crate::*;

pub(crate) fn realized_pnl(side: &str, entry_price: f64, exit_price: f64, quantity: f64) -> f64 {
    let signed_delta = if side == "Long" {
        exit_price - entry_price
    } else {
        entry_price - exit_price
    };

    // `quantity` is the full position size (calc_smart_size: desired_usdt / price,
    // submitted to the exchange as-is). PnL is the price move over the position;
    // leverage affects margin/ROI, NOT absolute PnL, so it must NOT be multiplied
    // in here (doing so double-counted it and inflated realized PnL by leverage).
    signed_delta * quantity
}

pub(crate) fn format_order_qty(qty: f64, qty_step: f64) -> String {
    let precision = qty_step_precision(qty_step);

    if precision == 0 {
        format!("{:.0}", qty)
    } else {
        let raw = format!("{qty:.precision$}");
        raw.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub(crate) fn format_price_value(price: f64, tick_size: f64) -> String {
    let precision = qty_step_precision(tick_size);
    if precision == 0 {
        format!("{:.0}", price)
    } else {
        let raw = format!("{price:.precision$}");
        raw.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub(crate) fn snap_price_to_tick(price: f64, tick_size: f64) -> f64 {
    if tick_size <= 0.0 {
        return price;
    }
    let precision = qty_step_precision(tick_size);
    let steps = (price / tick_size).round();
    round_with_precision(steps * tick_size, precision)
}

pub(crate) fn qty_step_precision(qty_step: f64) -> usize {
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

pub(crate) fn round_with_precision(value: f64, precision: usize) -> f64 {
    if precision == 0 {
        value.round()
    } else {
        let factor = 10_f64.powi(precision as i32);
        (value * factor).round() / factor
    }
}

pub(crate) fn normalize_order_quantity(qty: f64, c: BybitSymbolConstraints) -> Result<f64, String> {
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

pub(crate) fn ensure_min_notional(
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

pub(crate) async fn persist_bybit_fills(
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

pub(crate) async fn close_open_trade(
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

    let Some((trade_id, open_qty, entry_price, _leverage)) = open_trade else {
        return Ok(CloseReconcileResult::NoLocalOpen);
    };

    let eps = 1e-9_f64;
    let effective_close_qty = if close_qty > open_qty {
        open_qty
    } else {
        close_qty
    };
    let pnl_delta = realized_pnl(side, entry_price, close_price, effective_close_qty);
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

pub(crate) async fn submit_market_order(
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
        tracing::info!(event_id = %event.event_id, symbol = %event.decision.symbol, action = %event.decision.action, original_qty = event.decision.quantity, normalized_qty = normalized_qty, "Adjusted order quantity");
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

pub(crate) async fn fetch_order_execution_price(
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

pub(crate) async fn fetch_order_execution_fills(
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

pub(crate) async fn fetch_order_execution_meta(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
    order_id: &str,
) -> Result<OrderExecutionMeta, Box<dyn Error>> {
    let avg_price_from_order = fetch_order_execution_price(http, cfg, symbol, order_id).await?;
    let fills = match fetch_order_execution_fills(http, cfg, symbol, order_id).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(symbol = %symbol, order_id = %order_id, error = %e, "Failed to fetch Bybit fills");
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

pub(crate) async fn fetch_bybit_position_qty(
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

pub(crate) async fn fetch_bybit_last_price(
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

pub(crate) async fn set_bybit_trailing_stop(
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
            tracing::info!(
                event_id = %event.event_id,
                symbol = %event.decision.symbol,
                active_price = ?format_price_value(active_price, constraints.tick_size),
                trailing_distance = ?format_price_value(trailing_distance, constraints.tick_size),
                attempt = attempt,
                "Configured Bybit trailing stop"
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
