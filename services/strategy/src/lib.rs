use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch, RwLock};
use tracing::{error, info, warn};
use tupa_core::pipeline;
use tupa_engine::Executor;
use viper_domain::config::*;
use viper_domain::{
    stream_ensure_group, stream_publish, MarketSignal, MarketSignalEvent, StrategyDecision,
    StrategyDecisionEvent, REDIS_STREAM_DECISIONS, REDIS_STREAM_MARKET_DATA, STREAM_GROUP_STRATEGY,
};

pub mod backtest;
mod helpers;
mod thesis;
mod types;

pub(crate) use helpers::*;
pub(crate) use thesis::*;
pub(crate) use types::*;
mod config;

pub use config::StrategyConfig;
pub(crate) use config::*;
// Step modules (extracted)
mod ai_advice;
mod audit;
mod consecutive_losses;
mod cooldown;
mod daily_loss;
mod db;
mod decision;
mod equity_floor;
mod fetch;
mod filters;
mod funding;
mod signal_confirmation;
mod smart_size;
mod thesis_confirmation;
mod trailing;
mod trailing_config;
mod validate_entry;
mod validate_size;

pub(crate) use ai_advice::*;
pub(crate) use audit::*;
pub(crate) use consecutive_losses::*;
pub(crate) use cooldown::*;
pub(crate) use daily_loss::*;
pub(crate) use db::*;
pub(crate) use decision::*;
pub(crate) use equity_floor::*;
pub(crate) use fetch::*;
pub(crate) use filters::*;
pub(crate) use funding::*;
pub(crate) use signal_confirmation::*;
pub(crate) use smart_size::*;
pub(crate) use thesis_confirmation::*;
pub(crate) use trailing::*;
pub(crate) use trailing_config::*;
pub(crate) use validate_entry::*;
pub(crate) use validate_size::*;
mod tupa_extensions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyInput {
    pub symbol: String,
    #[serde(default)]
    pub temporal: Value,
    #[serde(default)]
    pub account_equity_usdt: f64,
    #[serde(default)]
    pub config: Value,
    /// Serialized `MarketSignal` (flat market features). Populated by the live
    /// loop; consumed by the real decision logic when it is enabled.
    #[serde(default)]
    pub signal: Value,
}

impl StrategyInput {
    fn max_daily_loss_pct(&self) -> f64 {
        self.config["risk"]["max_daily_loss_pct"]
            .as_f64()
            .unwrap_or(0.03)
    }

    fn max_consecutive_losses(&self) -> f64 {
        self.config["risk"]["max_consecutive_losses"]
            .as_f64()
            .unwrap_or(3.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Real strategy decision logic (Phase 1) — gated behind STRATEGY_REAL_DECISIONS.
//
// The pipeline! steps are independent fns of `&StrategyInput`, but the legacy
// `execute_strategy_step` logic accumulates state (later steps read earlier
// results). To adapt it to the pipeline while keeping TupaLang as the decision
// layer: each step rebuilds its base state from the input and calls
// execute_strategy_step for its own step; the `decision` step re-runs the
// prerequisite steps to assemble the state it aggregates over.
//
// Default OFF: when disabled, the original stub behavior is preserved.
// ─────────────────────────────────────────────────────────────────────────

/// Assemble the flat `state` the legacy logic reads: market signal features
/// (flat) + symbol + account equity + temporal state + safe account-history
/// defaults.
fn build_base_state(input: &StrategyInput) -> Value {
    let mut state = if input.signal.is_object() {
        input.signal.clone()
    } else {
        json!({})
    };
    if let Some(obj) = state.as_object_mut() {
        obj.insert("symbol".to_string(), json!(input.symbol));
        obj.insert(
            "account_equity_usdt".to_string(),
            json!(input.account_equity_usdt),
        );
        // TODO(phase-1): source these from trade history (DB). Safe defaults
        // for now — the daily-loss / consecutive-losses guards pass trivially.
        obj.entry("current_daily_loss".to_string())
            .or_insert(json!(0.0));
        obj.entry("consecutive_losses".to_string())
            .or_insert(json!(0));
        if let Some(temporal) = input.temporal.as_object() {
            for (k, v) in temporal {
                obj.entry(k.clone()).or_insert(v.clone());
            }
        }
    }
    state
}

/// Run a single legacy step, returning its result (or a HOLD/false-ish marker
/// Canonical step order. Later steps read earlier results from `state`
/// (e.g. `validate_size` reads `calc_smart_size.quantity`, `decision` reads all),
/// so a step must be run with its predecessors already accumulated.
const STEP_ORDER: &[&str] = &[
    "check_daily_loss",
    "check_consecutive_losses",
    "validate_entry",
    "check_funding",
    "calc_smart_size",
    "validate_size",
    "get_trailing_config",
    "decision",
];

/// Run the canonical sequence accumulating each result into `state`, and return
/// the result of `target` — with all of its prerequisites already in `state`.
/// This is what lets independent `pipeline!` steps reproduce the legacy logic's
/// sequential-state behavior.
fn run_steps_through(input: &StrategyInput, cfg: &StrategyConfig, target: &str) -> Value {
    let mut state = build_base_state(input);
    for &step in STEP_ORDER {
        let res = execute_strategy_step(step, state.clone(), cfg).unwrap_or_else(|e| {
            error!(step = step, err = %e, "real strategy step failed");
            json!({ "passed": false, "severity": "error", "reason": format!("step_error_{}", e) })
        });
        if step == target {
            return res;
        }
        if let Some(obj) = state.as_object_mut() {
            obj.insert(step.to_string(), res);
        }
    }
    json!({ "passed": false, "reason": format!("unknown_step_{}", target) })
}

pipeline! {
    name: ViperSmartCopy,
    input: StrategyInput,
    steps: [
        step("check_daily_loss") { step_check_daily_loss(input) },
        step("check_consecutive_losses") { step_check_consecutive_losses(input) },
        step("validate_entry") { step_validate_entry(input) },
        step("check_funding") { step_check_funding(input) },
        step("calc_smart_size") { step_calc_smart_size(input) },
        step("validate_size") { step_validate_size(input) },
        step("get_trailing_config") { step_get_trailing_config(input) },
        step("signal_confirmation") { step_signal_confirmation(input) },
        step("cooldown_guard") { step_cooldown_guard(input) },
        step("thesis_confirmation") { step_thesis_confirmation(input) },
        step("current_daily_loss") { step_current_daily_loss(input) },
        step("consecutive_losses") { step_consecutive_losses(input) },
        step("equity_floor") { step_equity_floor(input) },
        step("decision") { step_decision(input) },
        step("audit") { step_audit(input) },
    ],
    constraints: [
        metric("current_daily_loss").le(input.max_daily_loss_pct()).fail_fast(),
        metric("consecutive_losses").le(input.max_consecutive_losses()).fail_fast(),
        metric("equity_floor").ge(0.0)
    ]
}

fn active_position_advice_for_symbol<'a>(
    advice: Option<&'a AiAnalystAdviceSnapshot>,
    symbol: &str,
    side: &str,
) -> Option<&'a ActivePositionAdviceSnapshot> {
    let intended_side = if side.eq_ignore_ascii_case("Long") {
        "long"
    } else {
        "short"
    };

    advice.and_then(|snapshot| {
        snapshot.active_position_advice.iter().find(|item| {
            item.symbol.eq_ignore_ascii_case(symbol)
                && item.side.eq_ignore_ascii_case(intended_side)
        })
    })
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
fn decision_rank_score(runtime_output: &Value) -> f64 {
    get_record_f64(runtime_output, "decision", "decision_score", 0.0)
}

fn decision_entry_score(runtime_output: &Value) -> f64 {
    get_record_f64(runtime_output, "validate_entry", "entry_score", 0.0)
}

fn build_constraints_results(runtime_output: &Value) -> Value {
    json!({
        "check_daily_loss": runtime_output.get("check_daily_loss").cloned(),
        "check_consecutive_losses": runtime_output.get("check_consecutive_losses").cloned(),
        "validate_entry": runtime_output.get("validate_entry").cloned(),
        "check_funding": runtime_output.get("check_funding").cloned(),
        "validate_size": runtime_output.get("validate_size").cloned(),
        "signal_confirmation": runtime_output.get("signal_confirmation").cloned(),
        "cooldown_guard": runtime_output.get("cooldown_guard").cloned(),
        "thesis_confirmation": runtime_output.get("thesis_confirmation").cloned(),
        "audit": runtime_output.get("audit").cloned(),
    })
}

async fn finalize_strategy_decision(
    publish_conn: &mut redis::aio::MultiplexedConnection,
    db_pool: Option<&PgPool>,
    ctx: FinalizeDecisionContext<'_>,
    decision: StrategyDecision,
) -> Result<(), Box<dyn Error>> {
    if let Some(pool) = db_pool {
        let mut audit_output = ctx.runtime_output.clone();
        if let Some(obj) = audit_output.as_object_mut() {
            obj.insert(
                "final_decision".to_string(),
                serde_json::to_value(&decision).unwrap_or_else(|_| json!({})),
            );
        }
        let constraints_results = build_constraints_results(ctx.runtime_output);
        if let Err(err) = persist_tupa_audit_log(
            pool,
            ctx.signal_event,
            "viper_smart_copy",
            "1.0.0",
            ctx.pipeline_input,
            &audit_output,
            &constraints_results,
            &decision,
            ctx.execution_time_ms,
        )
        .await
        {
            error!(
                symbol = %ctx.signal_event.signal.symbol,
                err = %err,
                "Failed to persist Tupa audit log"
            );
        }
    }

    publish_decision_event(publish_conn, &ctx.signal_event.event_id, decision).await
}

async fn flush_pending_entry_candidates(
    publish_conn: &mut redis::aio::MultiplexedConnection,
    db_pool: Option<&PgPool>,
    pending_candidates: &mut HashMap<String, PendingEntryCandidate>,
    max_open_positions: i64,
) -> Result<(), Box<dyn Error>> {
    if pending_candidates.is_empty() {
        return Ok(());
    }

    let current_open_count = match db_pool {
        Some(pool) => count_open_trades(pool).await.unwrap_or_else(|err| {
            warn!(err = %err, "Failed to count open trades for portfolio selection");
            0
        }),
        None => 0,
    };
    let open_slots = (max_open_positions - current_open_count).max(0) as usize;

    let mut candidates: Vec<PendingEntryCandidate> =
        pending_candidates.drain().map(|(_, c)| c).collect();
    candidates.sort_by(|a, b| {
        b.rank_score
            .partial_cmp(&a.rank_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.entry_score
                    .partial_cmp(&a.entry_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.created_at.cmp(&b.created_at))
    });

    for (index, candidate) in candidates.into_iter().enumerate() {
        let final_decision = if index < open_slots {
            candidate.decision.clone()
        } else {
            create_hold_decision(
                &candidate.decision.symbol,
                &format!(
                    "portfolio_selection_not_selected_rank_{}_slots_{}_score_{:.0}",
                    index + 1,
                    open_slots,
                    candidate.rank_score
                ),
            )
        };

        finalize_strategy_decision(
            publish_conn,
            db_pool,
            FinalizeDecisionContext {
                signal_event: &candidate.signal_event,
                pipeline_input: &candidate.pipeline_input,
                runtime_output: &candidate.runtime_output,
                execution_time_ms: candidate.execution_time_ms,
            },
            final_decision,
        )
        .await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
fn execute_strategy_step(
    step_name: &str,
    state: Value,
    cfg: &StrategyConfig,
) -> Result<Value, String> {
    let symbol = get_string(&state, "symbol", "UNKNOWN");

    match step_name {
        "check_daily_loss" => {
            let current_daily_loss = get_f64(&state, "current_daily_loss", 0.0);
            Ok(json!(current_daily_loss <= cfg.max_daily_loss_pct()))
        }
        "check_consecutive_losses" => {
            let losses = get_i64(&state, "consecutive_losses", 0);
            Ok(json!(losses <= cfg.max_consecutive_losses()))
        }
        "validate_entry" => {
            let spread_pct = get_f64(&state, "spread_pct", 1.0);
            let volume_24h = get_i64(&state, "volume_24h", 0);
            let raw_trend_score = get_f64(&state, "trend_score", 0.0);
            let consensus_raw_trend_score =
                get_f64(&state, "consensus_trend_score", raw_trend_score);
            let consensus_trend_score = consensus_raw_trend_score.abs();
            let current_price = get_f64(&state, "current_price", 0.0);
            let atr_14 = get_f64(&state, "atr_14", 0.0);
            let trend_slope = get_f64(&state, "trend_slope", 0.0);
            let consensus_trend_slope = get_f64(&state, "consensus_trend_slope", trend_slope);
            let ema_fast = get_f64(&state, "ema_fast", 0.0);
            let ema_slow = get_f64(&state, "ema_slow", 0.0);
            let consensus_ema_fast = get_f64(&state, "consensus_ema_fast", ema_fast);
            let consensus_ema_slow = get_f64(&state, "consensus_ema_slow", ema_slow);
            let bollinger_percent_b = get_f64(&state, "bollinger_percent_b", 0.5);
            let consensus_bollinger_percent_b =
                get_f64(&state, "consensus_bollinger_percent_b", bollinger_percent_b);
            let consensus_adx_14 = get_f64(&state, "consensus_adx_14", 0.0);
            let bollinger_bandwidth = get_f64(&state, "bollinger_bandwidth", 0.0);
            let consensus_bollinger_bandwidth =
                get_f64(&state, "consensus_bollinger_bandwidth", bollinger_bandwidth);
            let rsi_14 = get_f64(&state, "rsi_14", 50.0);
            let consensus_rsi_14 = get_f64(&state, "consensus_rsi_14", rsi_14);
            let macd_line = get_f64(&state, "macd_line", 0.0);
            let macd_signal = get_f64(&state, "macd_signal", 0.0);
            let macd_histogram = get_f64(&state, "macd_histogram", 0.0);
            let consensus_macd_line = get_f64(&state, "consensus_macd_line", macd_line);
            let consensus_macd_signal = get_f64(&state, "consensus_macd_signal", macd_signal);
            let consensus_macd_histogram =
                get_f64(&state, "consensus_macd_histogram", macd_histogram);
            let volume_ratio = get_f64(&state, "volume_ratio", 0.0);
            let consensus_volume_ratio = get_f64(&state, "consensus_volume_ratio", volume_ratio);
            let btc_regime = get_string(&state, "btc_regime", "neutral");
            let btc_trend_score = get_f64(&state, "btc_trend_score", 0.0);
            let btc_consensus_count = get_i64(&state, "btc_consensus_count", 0);
            let regime = get_string(&state, "regime", "neutral");
            let exchanges_available = get_i64(&state, "exchanges_available", 0);
            let bybit_regime = get_string(&state, "bybit_regime", "neutral");
            let bullish_exchanges = get_i64(&state, "bullish_exchanges", 0);
            let bearish_exchanges = get_i64(&state, "bearish_exchanges", 0);
            let entry_side = if raw_trend_score >= 0.0 {
                "long"
            } else {
                "short"
            };
            let (rsi_min, rsi_max) = cfg.rsi_bounds_for_side(&symbol, entry_side);
            let btc_macro_penalty = if cfg.require_btc_macro_alignment() {
                let Some(penalty) = cfg.btc_macro_penalty_for_side(
                    &symbol,
                    entry_side,
                    &btc_regime,
                    btc_trend_score,
                    btc_consensus_count,
                ) else {
                    return Ok(json!(false));
                };
                penalty
            } else {
                0.0
            };
            let atr_pct = if current_price > 0.0 {
                atr_14 / current_price
            } else {
                1.0
            };
            let consensus_long_ok = if cfg.require_multi_exchange_consensus() {
                bullish_exchanges >= 2 && bearish_exchanges == 0 && exchanges_available >= 3
            } else {
                bybit_regime.eq_ignore_ascii_case("bullish")
                    || regime.eq_ignore_ascii_case("bullish")
            };
            let consensus_short_ok = if cfg.require_multi_exchange_consensus() {
                bearish_exchanges >= 2 && bullish_exchanges == 0 && exchanges_available >= 3
            } else {
                bybit_regime.eq_ignore_ascii_case("bearish")
                    || regime.eq_ignore_ascii_case("bearish")
            };
            let strict_long_ok = cfg.allow_long(&symbol)
                && regime.eq_ignore_ascii_case("bullish")
                && bybit_regime.eq_ignore_ascii_case("bullish")
                && consensus_long_ok
                && consensus_trend_slope > 0.0
                && consensus_ema_fast > consensus_ema_slow
                && current_price >= ema_fast
                && consensus_rsi_14 >= rsi_min
                && consensus_rsi_14 <= rsi_max
                && consensus_macd_line > consensus_macd_signal
                && consensus_macd_histogram > 0.0
                && consensus_volume_ratio >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                && consensus_trend_score
                    >= (cfg.min_trend_score_for_side(&symbol, entry_side) + btc_macro_penalty);

            let directional_ok = if raw_trend_score >= 0.0 {
                cfg.allow_long(&symbol)
                    && if cfg.permissive_entry() {
                        (bybit_regime.eq_ignore_ascii_case("bullish")
                            || regime.eq_ignore_ascii_case("bullish")
                            || consensus_raw_trend_score >= 0.0)
                            && consensus_trend_slope >= 0.0
                            && consensus_ema_fast >= consensus_ema_slow
                            && current_price > 0.0
                            && current_price >= ema_slow
                            && consensus_rsi_14 >= rsi_min
                            && consensus_rsi_14 <= rsi_max
                            && consensus_macd_line >= consensus_macd_signal
                            && consensus_macd_histogram >= 0.0
                            && consensus_volume_ratio
                                >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                            && consensus_trend_score
                                >= (cfg.min_trend_score_for_side(&symbol, entry_side)
                                    + btc_macro_penalty)
                    } else {
                        strict_long_ok
                    }
            } else {
                cfg.allow_short(&symbol)
                    && if cfg.permissive_entry() {
                        (bybit_regime.eq_ignore_ascii_case("bearish")
                            || regime.eq_ignore_ascii_case("bearish")
                            || consensus_raw_trend_score < 0.0)
                            && consensus_trend_slope <= 0.0
                            && consensus_ema_fast <= consensus_ema_slow
                            && current_price > 0.0
                            && current_price <= ema_slow
                            && consensus_rsi_14 >= rsi_min
                            && consensus_rsi_14 <= rsi_max
                            && consensus_macd_line <= consensus_macd_signal
                            && consensus_macd_histogram <= 0.0
                            && consensus_volume_ratio
                                >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                            && consensus_trend_score
                                >= (cfg.min_trend_score_for_side(&symbol, entry_side)
                                    + btc_macro_penalty)
                    } else {
                        regime.eq_ignore_ascii_case("bearish")
                            && bybit_regime.eq_ignore_ascii_case("bearish")
                            && consensus_short_ok
                            && consensus_trend_slope < 0.0
                            && consensus_ema_fast < consensus_ema_slow
                            && current_price <= ema_fast
                            && consensus_rsi_14 >= rsi_min
                            && consensus_rsi_14 <= rsi_max
                            && consensus_macd_line < consensus_macd_signal
                            && consensus_macd_histogram < 0.0
                            && consensus_volume_ratio
                                >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                            && consensus_trend_score
                                >= (cfg.min_trend_score_for_side(&symbol, entry_side)
                                    + btc_macro_penalty)
                    }
            };
            // Bollinger %B guard: avoid entering at price-band extremes (longs
            // above the ceiling, shorts below the floor). Neutral by default.
            let percent_b_limit = cfg.percent_b_limit_for_side(&symbol, entry_side);
            let percent_b_ok = if entry_side.eq_ignore_ascii_case("short") {
                consensus_bollinger_percent_b >= percent_b_limit
            } else {
                consensus_bollinger_percent_b <= percent_b_limit
            };
            let directional_ok = directional_ok && percent_b_ok;
            // ADX entry guard: require a minimum trend strength (skip chop).
            // Neutral by default (min_adx=0). consensus_adx_14=0 (missing data)
            // only blocks when min_adx>0, i.e. once ADX is actually populated.
            let adx_ok = consensus_adx_14 >= cfg.min_adx(&symbol);
            let directional_ok = directional_ok && adx_ok;
            let max_spread_pct = cfg.max_spread_pct(&symbol);
            let min_volume_24h = cfg.min_volume_24h_usdt(&symbol);
            let max_atr_pct = cfg.max_atr_pct(&symbol);
            let min_volume_ratio = cfg.min_volume_ratio_for_side(&symbol, entry_side);
            let min_trend_score =
                cfg.min_trend_score_for_side(&symbol, entry_side) + btc_macro_penalty;

            let mut components = Vec::new();
            let directional_bias = if entry_side == "long" { 1.0 } else { -1.0 };
            let consensus_regime_score = directional_points(
                &regime,
                if entry_side == "long" {
                    "bullish"
                } else {
                    "bearish"
                },
                if entry_side == "long" {
                    "bearish"
                } else {
                    "bullish"
                },
                1,
            ) as f64;
            push_weighted_entry_component(
                &mut components,
                "consensus_regime",
                consensus_regime_score,
                20.0,
            );

            let bybit_regime_score = directional_points(
                &bybit_regime,
                if entry_side == "long" {
                    "bullish"
                } else {
                    "bearish"
                },
                if entry_side == "long" {
                    "bearish"
                } else {
                    "bullish"
                },
                1,
            ) as f64;
            push_weighted_entry_component(
                &mut components,
                "bybit_regime",
                bybit_regime_score,
                20.0,
            );

            let consensus_score = if entry_side == "long" {
                if consensus_long_ok {
                    1.0
                } else {
                    -1.0
                }
            } else if consensus_short_ok {
                1.0
            } else {
                -1.0
            };
            push_weighted_entry_component(
                &mut components,
                "exchange_consensus",
                consensus_score,
                20.0,
            );

            let trend_slope_score = (consensus_trend_slope * directional_bias).clamp(-1.0, 1.0);
            push_weighted_entry_component(&mut components, "trend_slope", trend_slope_score, 10.0);

            let ema_alignment_score = if entry_side == "long" {
                if consensus_ema_fast > consensus_ema_slow {
                    1.0
                } else if consensus_ema_fast < consensus_ema_slow {
                    -1.0
                } else {
                    0.0
                }
            } else if consensus_ema_fast < consensus_ema_slow {
                1.0
            } else if consensus_ema_fast > consensus_ema_slow {
                -1.0
            } else {
                0.0
            };
            push_weighted_entry_component(
                &mut components,
                "ema_alignment",
                ema_alignment_score,
                10.0,
            );

            let rsi_quality_score = rsi_quality_score_for_side(entry_side, consensus_rsi_14);
            push_weighted_entry_component(&mut components, "rsi_quality", rsi_quality_score, 6.0);

            let bollinger_extension_score =
                bollinger_quality_score_for_side(entry_side, consensus_bollinger_percent_b);
            push_weighted_entry_component(
                &mut components,
                "bollinger_extension",
                bollinger_extension_score,
                8.0,
            );

            let bollinger_bandwidth_score =
                ((consensus_bollinger_bandwidth - 0.003) / 0.003).clamp(-1.0, 1.0);
            push_weighted_entry_component(
                &mut components,
                "bollinger_bandwidth",
                bollinger_bandwidth_score,
                5.0,
            );

            let macd_score = if entry_side == "long" {
                if consensus_macd_line > consensus_macd_signal {
                    1.0
                } else if consensus_macd_line < consensus_macd_signal {
                    -1.0
                } else {
                    0.0
                }
            } else if consensus_macd_line < consensus_macd_signal {
                1.0
            } else if consensus_macd_line > consensus_macd_signal {
                -1.0
            } else {
                0.0
            };
            push_weighted_entry_component(&mut components, "macd_cross", macd_score, 10.0);

            let macd_hist_score = (consensus_macd_histogram * directional_bias).clamp(-1.0, 1.0);
            push_weighted_entry_component(&mut components, "macd_histogram", macd_hist_score, 5.0);

            let macd_quality_score = macd_quality_score_for_side(
                entry_side,
                consensus_macd_line,
                consensus_macd_signal,
                consensus_macd_histogram,
            );
            push_weighted_entry_component(&mut components, "macd_quality", macd_quality_score, 6.0);

            let entry_confluence_score =
                ((rsi_quality_score + macd_quality_score + bollinger_extension_score) / 3.0)
                    .clamp(-1.0, 1.0);
            push_weighted_entry_component(
                &mut components,
                "entry_confluence",
                entry_confluence_score,
                8.0,
            );

            let volume_ratio_score = if min_volume_ratio > 0.0 {
                (consensus_volume_ratio / min_volume_ratio - 1.0).clamp(-1.0, 1.0)
            } else {
                0.0
            };
            push_weighted_entry_component(&mut components, "volume_ratio", volume_ratio_score, 5.0);

            let trend_score_ratio = if min_trend_score > 0.0 {
                (consensus_trend_score / min_trend_score - 1.0).clamp(-1.0, 1.0)
            } else {
                0.0
            };
            push_weighted_entry_component(&mut components, "trend_score", trend_score_ratio, 10.0);

            let entry_raw_score = components
                .iter()
                .map(|component| component.contribution)
                .sum();
            let entry_clamped_score = clamp_i32(entry_raw_score, -100, 100);
            let breakdown = EntryPolicyBreakdown {
                raw_score: entry_raw_score,
                clamped_score: entry_clamped_score,
                components,
            };
            let passed = spread_pct <= max_spread_pct
                && volume_24h >= min_volume_24h
                && atr_pct <= max_atr_pct
                && directional_ok;
            let reason = if passed {
                format!("entry_validated_{}", entry_policy_summary(&breakdown))
            } else if cfg.require_btc_macro_alignment()
                && cfg
                    .btc_macro_penalty_for_side(
                        &symbol,
                        entry_side,
                        &btc_regime,
                        btc_trend_score,
                        btc_consensus_count,
                    )
                    .is_none()
            {
                format!("{}_block_btc_macro_misaligned", entry_side)
            } else if spread_pct > max_spread_pct {
                format!("{}_block_spread", entry_side)
            } else if volume_24h < min_volume_24h {
                format!("{}_block_volume_24h", entry_side)
            } else if atr_pct > max_atr_pct {
                format!("{}_block_atr_pct", entry_side)
            } else if raw_trend_score >= 0.0 && !cfg.allow_long(&symbol) {
                "long_block_disabled".to_string()
            } else if raw_trend_score < 0.0 && !cfg.allow_short(&symbol) {
                "short_block_disabled".to_string()
            } else if raw_trend_score >= 0.0 && !regime.eq_ignore_ascii_case("bullish") {
                format!("long_block_consensus_regime_{}", regime.to_lowercase())
            } else if raw_trend_score < 0.0 && !regime.eq_ignore_ascii_case("bearish") {
                format!("short_block_consensus_regime_{}", regime.to_lowercase())
            } else if raw_trend_score >= 0.0 && !bybit_regime.eq_ignore_ascii_case("bullish") {
                format!("long_block_bybit_regime_{}", bybit_regime.to_lowercase())
            } else if raw_trend_score < 0.0 && !bybit_regime.eq_ignore_ascii_case("bearish") {
                format!("short_block_bybit_regime_{}", bybit_regime.to_lowercase())
            } else if raw_trend_score >= 0.0 && !consensus_long_ok {
                format!(
                    "long_block_consensus_{}_of_{}",
                    bullish_exchanges, exchanges_available
                )
            } else if raw_trend_score < 0.0 && !consensus_short_ok {
                format!(
                    "short_block_consensus_{}_of_{}",
                    bearish_exchanges, exchanges_available
                )
            } else if raw_trend_score >= 0.0 && consensus_trend_slope <= 0.0 {
                format!("long_block_trend_slope_{:.5}_lte_0", consensus_trend_slope)
            } else if raw_trend_score < 0.0 && consensus_trend_slope >= 0.0 {
                format!("short_block_trend_slope_{:.5}_gte_0", consensus_trend_slope)
            } else if raw_trend_score >= 0.0 && consensus_ema_fast <= consensus_ema_slow {
                "long_block_ema_alignment".to_string()
            } else if raw_trend_score < 0.0 && consensus_ema_fast >= consensus_ema_slow {
                "short_block_ema_alignment".to_string()
            } else if raw_trend_score >= 0.0 && current_price < ema_fast {
                format!(
                    "long_block_price_{:.5}_lt_fast_ema_{:.5}",
                    current_price, ema_fast
                )
            } else if raw_trend_score < 0.0 && current_price > ema_fast {
                format!(
                    "short_block_price_{:.5}_gt_fast_ema_{:.5}",
                    current_price, ema_fast
                )
            } else if raw_trend_score >= 0.0
                && consensus_bollinger_percent_b > 1.08
                && consensus_bollinger_bandwidth < 0.012
            {
                format!(
                    "long_block_bollinger_overstretch_pb_{:.3}_bw_{:.4}",
                    consensus_bollinger_percent_b, consensus_bollinger_bandwidth
                )
            } else if raw_trend_score < 0.0
                && consensus_bollinger_percent_b < -0.08
                && consensus_bollinger_bandwidth < 0.012
            {
                format!(
                    "short_block_bollinger_overstretch_pb_{:.3}_bw_{:.4}",
                    consensus_bollinger_percent_b, consensus_bollinger_bandwidth
                )
            } else if consensus_rsi_14 < rsi_min || consensus_rsi_14 > rsi_max {
                format!(
                    "{}_block_rsi_{:.2}_outside_{:.2}_{:.2}",
                    entry_side, consensus_rsi_14, rsi_min, rsi_max
                )
            } else if raw_trend_score >= 0.0 && consensus_macd_line <= consensus_macd_signal {
                "long_block_macd_cross".to_string()
            } else if raw_trend_score < 0.0 && consensus_macd_line >= consensus_macd_signal {
                "short_block_macd_cross".to_string()
            } else if raw_trend_score >= 0.0 && consensus_macd_histogram <= 0.0 {
                format!("long_block_macd_hist_{:.6}_lte_0", consensus_macd_histogram)
            } else if raw_trend_score < 0.0 && consensus_macd_histogram >= 0.0 {
                format!(
                    "short_block_macd_hist_{:.6}_gte_0",
                    consensus_macd_histogram
                )
            } else if consensus_volume_ratio < min_volume_ratio {
                format!(
                    "{}_block_volume_ratio_{:.2}_lt_{:.2}",
                    entry_side, consensus_volume_ratio, min_volume_ratio
                )
            } else if consensus_trend_score < min_trend_score {
                format!(
                    "{}_block_trend_score_{:.3}_lt_{:.3}",
                    entry_side, consensus_trend_score, min_trend_score
                )
            } else if !directional_ok {
                format!("{}_block_directional_checks", entry_side)
            } else {
                "risk_constraints_not_met".to_string()
            };
            Ok(json!({
                "passed": passed,
                "severity": if passed { "info" } else { "error" },
                "reason": reason,
                "side": entry_side,
                "entry_score": breakdown.clamped_score,
                "entry_breakdown": {
                    "raw_score": breakdown.raw_score,
                    "clamped_score": breakdown.clamped_score,
                    "components": breakdown.components
                }
            }))
        }
        "check_funding" => {
            let funding_rate = get_f64(&state, "funding_rate", 0.0).abs();
            let max_funding_rate_pct = cfg.max_funding_rate_pct();
            let funding_score = if max_funding_rate_pct > 0.0 {
                (1.0 - funding_rate / max_funding_rate_pct).clamp(-1.0, 1.0)
            } else {
                0.0
            };

            let mut components = Vec::new();
            push_weighted_funding_component(
                &mut components,
                "funding_rate_limit",
                funding_score,
                100.0,
            );

            let raw_score = components
                .iter()
                .map(|component| component.contribution)
                .sum();
            let clamped_score = clamp_i32(raw_score, -100, 100);
            let breakdown = FundingPolicyBreakdown {
                raw_score,
                clamped_score,
                components,
            };

            let passed = funding_rate <= max_funding_rate_pct;
            let reason = if passed {
                format!("funding_validated_{}", funding_policy_summary(&breakdown))
            } else {
                format!(
                    "funding_above_limit_{:.6}_gt_{:.6}_{}",
                    funding_rate,
                    max_funding_rate_pct,
                    funding_policy_summary(&breakdown)
                )
            };

            Ok(json!({
                "passed": passed,
                "severity": if passed { "info" } else { "error" },
                "reason": reason,
                "funding_rate": funding_rate,
                "funding_score": breakdown.clamped_score,
                "funding_breakdown": {
                    "raw_score": breakdown.raw_score,
                    "clamped_score": breakdown.clamped_score,
                    "components": breakdown.components
                }
            }))
        }
        "calc_smart_size" => {
            let price = get_f64(&state, "current_price", 0.0);
            if price <= 0.0 {
                return Ok(json!({
                    "quantity": 0.0,
                    "desired_usdt": 0.0,
                    "risk_budget_usdt": 0.0,
                    "volatility_discount": 0.0,
                    "proposal_score": -100,
                    "reason": "size_proposal_invalid_price",
                    "proposal_breakdown": {
                        "raw_score": -100,
                        "clamped_score": -100,
                        "components": []
                    }
                }));
            }

            let equity_usdt = get_f64(&state, "account_equity_usdt", 1_000.0);
            let atr_14 = get_f64(&state, "atr_14", 0.0);
            let volatility_discount =
                (1.0 - (atr_14 * cfg.atr_multiplier(&symbol) / price)).clamp(0.2, 1.0);
            let stop_loss_pct = cfg.stop_loss_pct(&symbol).max(0.0001);
            let risk_budget_usdt = equity_usdt * cfg.risk_per_trade_fraction();
            let risk_sized_notional = (risk_budget_usdt / stop_loss_pct) * volatility_discount;
            let max_position_usdt = cfg.max_position_cap_usdt(&symbol, equity_usdt);
            let desired_usdt = risk_sized_notional.clamp(
                cfg.min_position_usdt(),
                max_position_usdt.max(cfg.min_position_usdt()),
            );
            let quantity = desired_usdt / price;

            let discount_score = ((volatility_discount - 0.2) / 0.8).clamp(0.0, 1.0);
            let budget_score = if equity_usdt > 0.0 {
                (risk_budget_usdt / equity_usdt / cfg.risk_per_trade_fraction()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let cap_score = if max_position_usdt > 0.0 {
                (desired_usdt / max_position_usdt).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let mut components = Vec::new();
            push_weighted_size_proposal_component(
                &mut components,
                "volatility_discount",
                discount_score,
                40.0,
            );
            push_weighted_size_proposal_component(
                &mut components,
                "risk_budget",
                budget_score,
                30.0,
            );
            push_weighted_size_proposal_component(
                &mut components,
                "position_cap_fit",
                cap_score,
                30.0,
            );

            let raw_score = components
                .iter()
                .map(|component| component.contribution)
                .sum();
            let clamped_score = clamp_i32(raw_score, -100, 100);
            let breakdown = SizeProposalBreakdown {
                raw_score,
                clamped_score,
                components,
            };

            Ok(json!({
                "quantity": quantity,
                "desired_usdt": desired_usdt,
                "risk_budget_usdt": risk_budget_usdt,
                "volatility_discount": volatility_discount,
                "proposal_score": breakdown.clamped_score,
                "reason": format!("size_proposed_{}", size_proposal_summary(&breakdown)),
                "proposal_breakdown": {
                    "raw_score": breakdown.raw_score,
                    "clamped_score": breakdown.clamped_score,
                    "components": breakdown.components
                }
            }))
        }
        "validate_size" => {
            let quantity = get_record_f64(&state, "calc_smart_size", "quantity", 0.0);
            let price = get_f64(&state, "current_price", 0.0);
            let equity_usdt = get_f64(&state, "account_equity_usdt", 1_000.0);
            let position_usdt = quantity * price;
            let min_position_usdt = cfg.min_position_usdt();
            let max_position_usdt = cfg.max_position_cap_usdt(&symbol, equity_usdt);
            let min_size_score = if min_position_usdt > 0.0 {
                (position_usdt / min_position_usdt - 1.0).clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let max_size_score = if max_position_usdt > 0.0 {
                (1.0 - position_usdt / max_position_usdt).clamp(-1.0, 1.0)
            } else {
                0.0
            };

            let mut components = Vec::new();
            push_weighted_size_component(&mut components, "min_position", min_size_score, 50.0);
            push_weighted_size_component(&mut components, "max_position_cap", max_size_score, 50.0);

            let raw_score = components
                .iter()
                .map(|component| component.contribution)
                .sum();
            let clamped_score = clamp_i32(raw_score, -100, 100);
            let breakdown = SizePolicyBreakdown {
                raw_score,
                clamped_score,
                components,
            };

            let passed = position_usdt >= min_position_usdt && position_usdt <= max_position_usdt;
            let reason = if passed {
                format!("size_validated_{}", size_policy_summary(&breakdown))
            } else if position_usdt < min_position_usdt {
                format!(
                    "size_below_min_{:.2}_lt_{:.2}_{}",
                    position_usdt,
                    min_position_usdt,
                    size_policy_summary(&breakdown)
                )
            } else {
                format!(
                    "size_above_cap_{:.2}_gt_{:.2}_{}",
                    position_usdt,
                    max_position_usdt,
                    size_policy_summary(&breakdown)
                )
            };

            Ok(json!({
                "passed": passed,
                "severity": if passed { "info" } else { "error" },
                "reason": reason,
                "position_usdt": position_usdt,
                "size_score": breakdown.clamped_score,
                "size_breakdown": {
                    "raw_score": breakdown.raw_score,
                    "clamped_score": breakdown.clamped_score,
                    "components": breakdown.components
                }
            }))
        }
        "get_trailing_config" => {
            let trailing_cfg = cfg.trailing_runtime_config(&symbol);
            let enabled_score: f64 = if trailing_cfg.enabled { 1.0 } else { 0.0 };
            let activation_score = if trailing_cfg.activate_after_profit_pct > 0.0 {
                (1.0 - trailing_cfg.activate_after_profit_pct / 0.05).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance_score = if trailing_cfg.initial_trail_pct > 0.0 {
                (1.0 - trailing_cfg.initial_trail_pct / 0.03).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let trailing_score = clamp_i32(
                (enabled_score * 40.0).round() as i32
                    + (activation_score * 30.0).round() as i32
                    + (distance_score * 30.0).round() as i32,
                0,
                100,
            );

            Ok(json!({
                "enabled": trailing_cfg.enabled,
                "activate_after_profit_pct": trailing_cfg.activate_after_profit_pct,
                "initial_trail_pct": trailing_cfg.initial_trail_pct,
                "move_to_break_even_at": trailing_cfg.move_to_break_even_at,
                "min_move_threshold_pct": trailing_cfg.min_move_threshold_pct,
                "ratchet_level_count": trailing_cfg.ratchet_levels.len(),
                "trailing_score": trailing_score,
                "reason": format!(
                    "trailing_configured_enabled_{}_activate_{:.4}_trail_{:.4}_ratchets_{}",
                    trailing_cfg.enabled,
                    trailing_cfg.activate_after_profit_pct,
                    trailing_cfg.initial_trail_pct,
                    trailing_cfg.ratchet_levels.len()
                )
            }))
        }
        "decision" => {
            let can_enter = get_bool(&state, "check_daily_loss", false)
                && get_bool(&state, "check_consecutive_losses", false)
                && get_record_bool(&state, "validate_entry", "passed", false)
                && get_record_bool(&state, "check_funding", "passed", false)
                && get_record_bool(&state, "validate_size", "passed", false);

            let entry_price = get_f64(&state, "current_price", 0.0);
            let quantity = get_record_f64(&state, "calc_smart_size", "quantity", 0.0);
            let entry_side = get_record_string(&state, "validate_entry", "side", "long");
            let entry_reason = get_record_string(
                &state,
                "validate_entry",
                "reason",
                "risk_constraints_not_met",
            );
            let entry_score = get_record_f64(&state, "validate_entry", "entry_score", 0.0);
            let entry_breakdown_summary = summarize_entry_breakdown(&state);
            let entry_reason_with_breakdown = if let Some(summary) = entry_breakdown_summary {
                format!("{}_{}", entry_reason, summary)
            } else {
                entry_reason.clone()
            };
            let size_reason = get_record_string(
                &state,
                "validate_size",
                "reason",
                "size_constraints_not_met",
            );
            let size_score = get_record_f64(&state, "validate_size", "size_score", 0.0);
            let size_proposal_reason = get_record_string(
                &state,
                "calc_smart_size",
                "reason",
                "size_proposal_not_available",
            );
            let size_proposal_score =
                get_record_f64(&state, "calc_smart_size", "proposal_score", 0.0);
            let funding_reason = get_record_string(
                &state,
                "check_funding",
                "reason",
                "funding_constraints_not_met",
            );
            let funding_score = get_record_f64(&state, "check_funding", "funding_score", 0.0);
            let trailing_reason = get_record_string(
                &state,
                "get_trailing_config",
                "reason",
                "trailing_config_not_available",
            );
            let trailing_score =
                get_record_f64(&state, "get_trailing_config", "trailing_score", 0.0);

            let mut components = Vec::new();
            push_weighted_decision_component(
                &mut components,
                "entry_policy",
                (entry_score / 100.0).clamp(-1.0, 1.0),
                40.0,
            );
            push_weighted_decision_component(
                &mut components,
                "funding_policy",
                (funding_score / 100.0).clamp(-1.0, 1.0),
                20.0,
            );
            push_weighted_decision_component(
                &mut components,
                "size_proposal",
                (size_proposal_score / 100.0).clamp(-1.0, 1.0),
                20.0,
            );
            push_weighted_decision_component(
                &mut components,
                "size_policy",
                (size_score / 100.0).clamp(-1.0, 1.0),
                10.0,
            );
            push_weighted_decision_component(
                &mut components,
                "trailing_policy",
                (trailing_score / 100.0).clamp(0.0, 1.0),
                10.0,
            );
            let decision_raw_score = components
                .iter()
                .map(|component| component.contribution)
                .sum();
            let decision_clamped_score = clamp_i32(decision_raw_score, -100, 100);
            let decision_breakdown = DecisionPolicyBreakdown {
                raw_score: decision_raw_score,
                clamped_score: decision_clamped_score,
                components,
            };
            let decision_summary = decision_policy_summary(&decision_breakdown);

            if can_enter && quantity > 0.0 && entry_price > 0.0 {
                let is_long = entry_side.eq_ignore_ascii_case("long");
                let sl_pct = cfg.stop_loss_pct(&symbol);
                let tp_pct = cfg.take_profit_pct(&symbol);

                let stop_loss = if is_long {
                    entry_price * (1.0 - sl_pct)
                } else {
                    entry_price * (1.0 + sl_pct)
                };
                let take_profit = if is_long {
                    entry_price * (1.0 + tp_pct)
                } else {
                    entry_price * (1.0 - tp_pct)
                };

                Ok(json!({
                    "action": if is_long { "ENTER_LONG" } else { "ENTER_SHORT" },
                    "symbol": symbol,
                    "quantity": quantity,
                    "leverage": cfg.max_leverage(),
                    "entry_price": entry_price,
                    "stop_loss": stop_loss,
                    "take_profit": take_profit,
                    "reason": format!(
                        "entry_confirmed_score_{:.3}_funding_{:.3}_proposal_{:.3}_size_{:.3}_trailing_{:.3}_{}_{}_{}_{}_{}_{}",
                        entry_score,
                        funding_score,
                        size_proposal_score,
                        size_score,
                        trailing_score,
                        entry_reason_with_breakdown,
                        funding_reason,
                        size_proposal_reason,
                        size_reason,
                        trailing_reason,
                        decision_summary
                    ),
                    "smart_copy_compatible": true,
                    "decision_score": decision_breakdown.clamped_score,
                    "decision_breakdown": {
                        "raw_score": decision_breakdown.raw_score,
                        "clamped_score": decision_breakdown.clamped_score,
                        "components": decision_breakdown.components
                    }
                }))
            } else {
                Ok(json!({
                    "action": "HOLD",
                    "symbol": symbol,
                    "quantity": 0.0,
                    "leverage": 0.0,
                    "entry_price": 0.0,
                    "stop_loss": 0.0,
                    "take_profit": 0.0,
                    "reason": format!(
                        "{}_{}_{}_{}_{}_{}",
                        entry_reason_with_breakdown,
                        funding_reason,
                        size_proposal_reason,
                        size_reason,
                        trailing_reason,
                        decision_summary
                    ),
                    "smart_copy_compatible": false,
                    "decision_score": decision_breakdown.clamped_score,
                    "decision_breakdown": {
                        "raw_score": decision_breakdown.raw_score,
                        "clamped_score": decision_breakdown.clamped_score,
                        "components": decision_breakdown.components
                    }
                }))
            }
        }
        "audit" => {
            let decision_action = get_record_string(&state, "decision", "action", "UNKNOWN");
            let decision_reason = get_record_string(
                &state,
                "decision",
                "reason",
                "audit_missing_decision_reason",
            );
            let decision_score = get_record_f64(&state, "decision", "decision_score", 0.0);
            let smart_copy_compatible =
                get_record_bool(&state, "decision", "smart_copy_compatible", false);
            let temporal_reason = structured_hold_reason_from_state(&state);

            Ok(json!({
                "ok": true,
                "reason": format!(
                    "audit_action_{}_score_{:.3}_smart_copy_{}_{}_{}",
                    decision_action,
                    decision_score,
                    smart_copy_compatible,
                    decision_reason,
                    temporal_reason
                ),
                "decision_action": decision_action,
                "decision_score": decision_score,
                "smart_copy_compatible": smart_copy_compatible
            }))
        }
        _ => Ok(Value::Null),
    }
}

async fn publish_decision_event(
    publish_conn: &mut redis::aio::MultiplexedConnection,
    source_event_id: &str,
    decision: StrategyDecision,
) -> Result<(), Box<dyn Error>> {
    let decision_event = StrategyDecisionEvent::new(source_event_id.to_string(), decision);
    decision_event.validate()?;

    let decision_json = serde_json::to_string(&decision_event)?;
    stream_publish(publish_conn, REDIS_STREAM_DECISIONS, &decision_json).await?;

    info!(
        event_id = %decision_event.event_id,
        symbol = %decision_event.decision.symbol,
        action = %decision_event.decision.action,
        "Published decision event"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
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

pub async fn run() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "viper_strategy=info".into()),
        )
        .json()
        .init();

    info!("Starting viper-strategy");

    let listener = TcpListener::bind("0.0.0.0:8082").await?;
    info!("Health check server running on :8082");

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let mut health_shutdown_rx = shutdown_rx.clone();
    let invalid_signal_count = Arc::new(AtomicU64::new(0));
    let invalid_signal_count_for_health = Arc::clone(&invalid_signal_count);
    let last_invalid_signal = Arc::new(RwLock::new(None::<InvalidSignalDrop>));
    let last_invalid_signal_for_health = Arc::clone(&last_invalid_signal);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                            _ = health_shutdown_rx.changed() => {
                                break;
                            }
                            accept_result = listener.accept() => {
                                if let Ok((mut socket, _)) = accept_result {
                                    let invalid_signal_count_for_conn =
                                        Arc::clone(&invalid_signal_count_for_health);
                                    let last_invalid_signal_for_conn =
                                        Arc::clone(&last_invalid_signal_for_health);
                                    tokio::spawn(async move {
                                        let last_invalid = last_invalid_signal_for_conn.read().await.clone();
                                        let body = serde_json::json!({
                                            "status": "ok",
                                            "invalid_market_signals_dropped": invalid_signal_count_for_conn.load(Ordering::Relaxed),
                                            "last_invalid_market_signal_drop": last_invalid.as_ref().map(|drop| json!({
                                                "symbol": drop.symbol,
                                                "stage": drop.stage,
                                                "reason": drop.reason,
                                                "timestamp": drop.timestamp,
                                            })),
                                        })
                                        .to_string();
                                        let response = format!(
                                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
                                            body.len(),
                                            body
                                        );
            if let Err(e) = socket.write_all(response.as_bytes()).await {
                                                error!(err = ?e, "failed to write to socket");
                                            }
                                    });
                                }
                            }
                        }
        }
    });

    let strategy_config_path = std::env::var("STRATEGY_CONFIG")
        .unwrap_or_else(|_| "config/trading/pairs.yaml".to_string());
    let trading_profile = std::env::var("TRADING_PROFILE").unwrap_or_else(|_| "MEDIUM".to_string());
    let trading_mode = std::env::var("TRADING_MODE").unwrap_or_else(|_| "paper".to_string());

    // Pool is created before the config load so the config can come from the DB
    // (active version, seeded from YAML on first boot). Falls back to the baked
    // file when there is no DB (local/CLI use).
    let db_pool = match resolve_database_url() {
        Some(database_url) => match PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                info!("Strategy database connection: enabled");
                Some(pool)
            }
            Err(err) => {
                warn!(err = %err, "Strategy database unavailable (open-position trailing disabled)");
                None
            }
        },
        None => {
            info!("Strategy database connection: disabled (missing DB_* env)");
            None
        }
    };

    // Config is the baked pairs.yaml — the single source of truth (git). The
    // db_pool below is still used for open-position trailing, not for config.
    let cfg = Arc::new(StrategyConfig::from_files(
        &strategy_config_path,
        &trading_profile,
        &trading_mode,
    )?);
    // Expose config to the pipeline steps for the real (flag-gated) decision logic.
    let _ = STRATEGY_CFG.set(cfg.clone());
    if real_cfg().is_some() {
        info!("STRATEGY_REAL_DECISIONS enabled — using real decision logic");
    }

    let executor = Executor::new();
    let pipeline = ViperSmartCopy::new();
    info!(
        profile = %cfg.profile,
        mode = %cfg.trading_mode,
        "Initialized ViperSmartCopy pipeline"
    );

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://vipertrade-redis:6379".to_string());
    info!("Connecting to Redis at {}", redis_url);

    let client = redis::Client::open(redis_url.clone())?;
    let mut publish_conn = client.get_multiplexed_async_connection().await?;
    stream_ensure_group(
        &mut publish_conn,
        REDIS_STREAM_MARKET_DATA,
        STREAM_GROUP_STRATEGY,
    )
    .await;

    let (signal_tx, mut signal_rx) = mpsc::unbounded_channel::<String>();
    let signal_consumer = format!("strategy-{}", std::process::id());
    let stream_tx = signal_tx.clone();
    let mut stream_shutdown_rx = shutdown_rx.clone();
    let redis_url_clone = redis_url.clone();
    tokio::spawn(async move {
        loop {
            let c = match redis::Client::open(redis_url_clone.as_str()) {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "Failed to create Redis client for stream reader");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
            let mut conn = match c.get_multiplexed_async_connection().await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "Failed to connect Redis for stream reader");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
            info!("Starting market data stream reader");
            loop {
                tokio::select! {
                    _ = stream_shutdown_rx.changed() => return,
                    result = async {
                        let result: redis::RedisResult<viper_domain::StreamEntries> = redis::cmd("XREADGROUP")
                            .arg("GROUP")
                            .arg(STREAM_GROUP_STRATEGY)
                            .arg(&signal_consumer)
                            .arg("BLOCK")
                            .arg(2000)
                            .arg("COUNT")
                            .arg(1)
                            .arg("STREAMS")
                            .arg(REDIS_STREAM_MARKET_DATA)
                            .arg(">")
                            .query_async(&mut conn)
                            .await;
                        result
                    } => {
                        match result {
                            Ok(entries) => {
                                for (_stream, messages) in entries {
                                    for (msg_id, fields) in messages {
                                        for (k, v) in fields {
                                            if k == "payload" {
                                                let _ = stream_tx.send(v);
                                                let _: Result<String, _> = redis::cmd("XACK")
                                                    .arg(REDIS_STREAM_MARKET_DATA)
                                                    .arg(STREAM_GROUP_STRATEGY)
                                                    .arg(&msg_id)
                                                    .query_async(&mut conn)
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Market data stream read failed");
                                break;
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
    info!("Subscribed to market data stream");
    let mut pending_candidates = HashMap::<String, PendingEntryCandidate>::new();
    let mut entry_guards = HashMap::<String, EntryGuardState>::new();
    let mut signal_confirmations = HashMap::<String, SignalConfirmationState>::new();
    let mut thesis_invalidations = HashMap::<String, ThesisInvalidationState>::new();
    let default_stop_loss_cooldown_minutes = 3_i64;
    let selection_window_ms = std::env::var("STRATEGY_ENTRY_SELECTION_WINDOW_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3_000);
    let selection_window = Duration::from_millis(selection_window_ms);
    let paper_max_open_positions = std::env::var("EXECUTOR_PAPER_MAX_OPEN_POSITIONS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(4);
    let mut selection_tick = tokio::time::interval(Duration::from_millis(500));
    selection_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let wallet_api_base_url = resolve_wallet_api_base_url();
    let wallet_http = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let ai_analyst_base_url = resolve_ai_analyst_base_url();
    let ai_analyst_http = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let fallback_equity_usdt = std::env::var("INITIAL_CAPITAL_USD")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1_000.0);
    let mut cached_account_equity_usdt = fallback_equity_usdt;
    let mut last_wallet_fetch_at = Instant::now() - Duration::from_secs(60);
    let mut cached_execution_advice: Option<AiAnalystAdviceSnapshot> = None;
    let mut last_execution_advice_fetch_at = Instant::now() - Duration::from_secs(60);
    let execution_advice_refresh = Duration::from_secs(
        std::env::var("AI_EXECUTION_ADVICE_REFRESH_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(30),
    );
    let execution_advice_lookback_hours = std::env::var("AI_EXECUTION_ADVICE_LOOKBACK_HOURS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(6);

    loop {
        tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if let Err(err) = flush_pending_entry_candidates(
                            &mut publish_conn,
                            db_pool.as_ref(),
                            &mut pending_candidates,
                            paper_max_open_positions,
                        ).await {
                            error!(err = %err, "Failed to flush pending entry candidates during shutdown");
                        }
                        info!("Received shutdown signal, stopping viper-strategy");
                        break;
                    }
                    _ = selection_tick.tick() => {
                        let should_flush = pending_candidates
                            .values()
                            .map(|candidate| candidate.created_at.elapsed() >= selection_window)
                            .any(|ready| ready);
                        if should_flush {
                            if let Err(err) = flush_pending_entry_candidates(
                                &mut publish_conn,
                                db_pool.as_ref(),
                                &mut pending_candidates,
                                paper_max_open_positions,
                            ).await {
                                error!(err = %err, "Failed to flush pending entry candidates");
                            }
                        }
                    }
                    maybe_msg = signal_rx.recv() => {
                        let Some(payload) = maybe_msg else {
                            error!("Market data stream ended unexpectedly; exiting so container can restart");
                            return Err("market data stream ended unexpectedly".into());
                        };

                        let signal_event: MarketSignalEvent = match serde_json::from_str(&payload) {
                            Ok(evt) => evt,
                            Err(_) => {
                                let legacy_signal: MarketSignal = match serde_json::from_str(&payload) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        error!(err = %e, "Failed to parse market signal event");
                                        continue;
                                    }
                                };
                                MarketSignalEvent::new(legacy_signal)
                            }
                        };

                        if let Err(err) = signal_event.validate() {
                            invalid_signal_count.fetch_add(1, Ordering::Relaxed);
                            let drop = InvalidSignalDrop {
                                symbol: signal_event.signal.symbol.clone(),
                                stage: "pre_decision".to_string(),
                                reason: err.clone(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            };
                            *last_invalid_signal.write().await = Some(drop.clone());
                            warn!(
                                symbol = %drop.symbol,
                                stage = %drop.stage,
                                reason = %drop.reason,
                                "Invalid market signal dropped"
                            );
                            continue;
                        }

                        let symbol = signal_event.signal.symbol.to_uppercase();
                        let trend = signal_event.signal.trend_score;

                        if cached_execution_advice.is_none()
                            || last_execution_advice_fetch_at.elapsed() >= execution_advice_refresh
                        {
                            if let Some(advice) = fetch_execution_advice(
                                &ai_analyst_http,
                                &ai_analyst_base_url,
                                execution_advice_lookback_hours,
                            )
                            .await
                            {
                                cached_execution_advice = Some(advice);
                            }
                            last_execution_advice_fetch_at = Instant::now();
                        }

                        sync_execution_advice_guards(
                            &mut entry_guards,
                            cached_execution_advice
                                .as_ref()
                                .map(|snapshot| &snapshot.execution_advice),
                        );

                        if let Some(guard) = entry_guards.get_mut(&symbol) {
                            if Instant::now() >= guard.cooldown_until {
                                // Cooldown expiration alone is not enough; same-direction reentry stays blocked
                                // until the market bias flips once.
                            }
                            if guard.awaiting_flip && !is_same_direction(&guard.blocked_side, trend) {
                                guard.awaiting_flip = false;
                            }
                        }

                        if let Some(pool) = &db_pool {
                            match fetch_open_trade_for_symbol(pool, &symbol).await {
                                Ok(Some(open)) => {
                                    let current_price = signal_event.signal.current_price;
                                    let exit_evaluation =
                                        evaluate_open_trade_exit(
                                            &symbol,
                                            current_price,
                                            &open,
                                            cfg.as_ref(),
                                            active_position_advice_for_symbol(
                                                cached_execution_advice.as_ref(),
                                                &symbol,
                                                &open.side,
                                            ),
                                        );
                                    let close_decision = exit_evaluation.decision.clone();
                                    let trailing_eval = exit_evaluation.trailing.clone();
                                    if let Some(eval) = trailing_eval {
                                        let trailing_cfg = cfg.trailing_runtime_config(&symbol);
                                        if should_persist_trailing_update(
                                            &open,
                                            &eval,
                                            trailing_cfg.min_move_threshold_pct,
                                        ) {
                                            if let Err(err) = update_trade_trailing_state(
                                                pool,
                                                &open.trade_id,
                                                eval.activated,
                                                eval.peak_price,
                                                eval.trail_pct,
                                            )
                                            .await
                                            {
                                                warn!(trade_id = %open.trade_id, err = %err, "Failed to persist trailing state");
                                            }
                                        }
                                    }
                                    // min_hold defers ONLY the discretionary thesis exit
                                    // (give a fresh position room before cutting on a
                                    // consensus/regime blip). Protective stops (SL/TP/
                                    // trailing) already fired in evaluate_open_trade_exit
                                    // regardless of age.
                                    let min_hold_secs = cfg.min_hold_seconds();
                                    let min_hold_active = min_hold_secs > 0
                                        && Utc::now()
                                            .signed_duration_since(open.opened_at)
                                            .num_seconds()
                                            .max(0)
                                            < min_hold_secs;
                                    let thesis_decision = if min_hold_active {
                                        None
                                    } else {
                                        enforce_open_position_thesis_guard(
                                            &symbol,
                                            &signal_event.signal,
                                            &open,
                                            cfg.as_ref(),
                                            &mut thesis_invalidations,
                                        )
                                    };
                                    let decision = if let Some(decision) = close_decision {
                                        thesis_invalidations.remove(&symbol);
                                        decision
                                    } else if let Some(decision) = thesis_decision {
                                        if decision.action != "HOLD" {
                                            thesis_invalidations.remove(&symbol);
                                        }
                                        decision
                                    } else {
                                        create_hold_decision(
                                            &symbol,
                                            &format!(
                                                "exit_{}_{}",
                                                exit_evaluation.trigger, exit_evaluation.reason
                                            ),
                                        )
                                    };

                                    // Guard against duplicate exits: the position is still open here,
                                    // so if we already emitted a CLOSE for it that the executor has not
                                    // processed yet (e.g. after a strategy restart), re-emitting would
                                    // send a duplicate close. Downgrade to HOLD until it is executed.
                                    let decision = if decision.action.starts_with("CLOSE_") {
                                        match has_recent_close_decision_for_symbol(pool, &symbol, 5)
                                            .await
                                        {
                                            Ok(true) => {
                                                info!(
                                                    symbol = %symbol,
                                                    action = %decision.action,
                                                    "Suppressing duplicate close: a recent CLOSE for this open position is still pending execution"
                                                );
                                                create_hold_decision(
                                                    &symbol,
                                                    &format!("close_already_pending_{}", decision.reason),
                                                )
                                            }
                                            Ok(false) => decision,
                                            Err(err) => {
                                                warn!(
                                                    symbol = %symbol,
                                                    err = %err,
                                                    "Failed to check for a pending close; emitting close decision"
                                                );
                                                decision
                                            }
                                        }
                                    } else {
                                        decision
                                    };

                                    if let Err(err) = finalize_strategy_decision(
                                        &mut publish_conn,
                                        db_pool.as_ref(),
                                        FinalizeDecisionContext {
                                            signal_event: &signal_event,
                                            pipeline_input: &json!(&signal_event.signal),
                                            runtime_output: &json!({
                                                "open_trade_exit_trigger": exit_evaluation.trigger,
                                                "open_trade_exit_reason": exit_evaluation.reason,
                                                "active_position_advice": active_position_advice_for_symbol(
                                                    cached_execution_advice.as_ref(),
                                                    &symbol,
                                                    &open.side,
                                                ),
                                            }),
                                            execution_time_ms: 0,
                                        },
                                        decision,
                                    )
                                    .await
                                    {
                                        error!(err = %err, "Failed to publish open-position decision");
                                    }
                                    continue;
                                }
                                Ok(None) => {
                                    thesis_invalidations.remove(&symbol);
                                }
                                Err(err) => {
                                    error!(symbol = %symbol, err = %err, "Failed to query open trade");
                                }
                            }
                        }

                        if last_wallet_fetch_at.elapsed() >= Duration::from_secs(15) {
                            cached_account_equity_usdt = fetch_account_equity_usdt(
                                &wallet_http,
                                &wallet_api_base_url,
                                fallback_equity_usdt,
                            )
                            .await;
                            last_wallet_fetch_at = Instant::now();
                        }

                        let input_value = serde_json::to_value(&StrategyInput {
                            symbol: signal_event.signal.symbol.clone(),
                            temporal: build_temporal_pipeline_state(
                                &symbol,
                                trend,
                                cfg.as_ref(),
                                &entry_guards,
                                &signal_confirmations,
                                &thesis_invalidations,
                            ),
                            account_equity_usdt: cached_account_equity_usdt,
                            config: json!({
                                "bollinger": {
                                    "std_dev_multiplier": cfg.bollinger_std_dev_multiplier,
                                    "invalidation_threshold": cfg.bollinger_invalidation_threshold
                                },
                                "risk": {
                                    "max_daily_loss_pct": cfg.max_daily_loss_pct(),
                                    "max_consecutive_losses": cfg.max_consecutive_losses()
                                }
                            }),
                            signal: serde_json::to_value(&signal_event.signal)
                                .unwrap_or_else(|_| json!({})),
                        })?;
                        let pipeline_input = input_value.clone();
                        let strategy_input: StrategyInput = serde_json::from_value(input_value.clone())?;
                        let pipeline_started_at = Instant::now();
                        let (constraint_passed, constraint_failures, mut runtime_output) =
                            match executor.run_parallel(&pipeline, &strategy_input).await {
                                Ok(result) => {
                                    let passed = result.passed;
                                    let failures = result.failures;
                                    let mut values = result.values;
                                    values.insert("execution_advice".to_string(), json!({}));
                                    (passed, failures, json!(values))
                                }
                                Err(e) => {
                                    error!(err = %e, "Pipeline execution failed");
                                    continue;
                                }
                            };

                        if let Some(obj) = runtime_output.as_object_mut() {
                            obj.insert(
                                "execution_advice".to_string(),
                                serde_json::to_value(cached_execution_advice.clone())
                                    .unwrap_or_else(|_| json!({})),
                            );
                        }

                        let decision_value = runtime_output.get("decision").cloned();
                        let Some(decision_value) = decision_value else {
                            error!(symbol = %symbol, "Pipeline output missing 'decision' step result");
                            continue;
                        };

                        match serde_json::from_value::<StrategyDecision>(decision_value.clone()) {
                            Ok(decision) => {
                                // Gate entries when pipeline invariants are violated.
                                let decision = if !constraint_passed
                                    && matches!(
                                        decision.action.as_str(),
                                        "ENTER_LONG" | "ENTER_SHORT"
                                    )
                                {
                                    let reason = format!(
                                        "constraint_gate_{}",
                                        constraint_failures
                                            .iter()
                                            .map(|f| f.metric.as_str())
                                            .collect::<Vec<_>>()
                                            .join("_")
                                    );
                                    warn!(
                                        symbol = %symbol,
                                        failure_count = constraint_failures.len(),
                                        reason = %reason,
                                        "Pipeline constraints failed — blocking entry"
                                    );
                                    create_hold_decision(&symbol, &reason)
                                } else {
                                    decision
                                };
                                let intended_side = if decision.action == "ENTER_SHORT" {
                                    "short"
                                } else {
                                    "long"
                                };
                                let min_confirmation_ticks =
                                    cfg.min_signal_confirmation_ticks_for_side(&symbol, intended_side);
                                let stop_loss_cooldown_minutes = cfg
                                    .stop_loss_cooldown_minutes_for_side(&symbol, intended_side)
                                    .max(default_stop_loss_cooldown_minutes);
                                let recent_stop_loss_same_symbol = if let Some(pool) = &db_pool {
                                    if matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
                                        match has_recent_stop_loss_for_symbol(
                                            pool,
                                            &symbol,
                                            stop_loss_cooldown_minutes,
                                        )
                                        .await
                                        {
                                            Ok(v) => v,
        Err(err) => {
                                                    warn!(symbol = %symbol, err = %err, "Failed to check stop-loss cooldown");
                                                    false
                                                }
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                };

                                let decision = enforce_entry_guards(
                                    &symbol,
                                    trend,
                                    decision,
                                    &mut entry_guards,
                                    stop_loss_cooldown_minutes,
                                    recent_stop_loss_same_symbol,
                                    &mut signal_confirmations,
                                    min_confirmation_ticks,
                                );

                                let mut decision = decision;
                                if matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
                                    let rank_score = decision_rank_score(&runtime_output);
                                    let entry_score = decision_entry_score(&runtime_output);
                                    decision = apply_execution_advice_veto(
                                        &symbol,
                                        decision,
                                        entry_score,
                                        rank_score,
                                        cached_execution_advice
                                            .as_ref()
                                            .map(|snapshot| &snapshot.execution_advice),
                                    );
                                    decision = apply_execution_advice_sizing(
                                        decision,
                                        cached_execution_advice
                                            .as_ref()
                                            .map(|snapshot| &snapshot.execution_advice),
                                        entry_score,
                                        rank_score,
                                        cfg.min_position_usdt(),
                                    );
                                }
                                if decision.action == "HOLD" && decision.reason == "risk_constraints_not_met"
                                {
                                    decision.reason = structured_hold_reason_from_state(&runtime_output);
                                } else if decision.action == "HOLD"
                                    && (decision.reason.starts_with("awaiting_signal_confirmation_")
                                        || decision.reason.starts_with("cooldown_stop_loss_")
                                        || decision.reason.starts_with("cooldown_thesis_invalidated_")
                                        || decision.reason.starts_with("blocked_until_trend_flip_"))
                                {
                                    if let Some(temporal_reason) =
                                        structured_temporal_reason_from_state(&runtime_output)
                                    {
                                        decision.reason =
                                            format!("{}_{}", decision.reason, temporal_reason);
                                    }
                                }

                                if decision.reason == "stop_loss_triggered" {
                                    let blocked_side = if decision.action == "CLOSE_LONG" {
                                        "Long"
                                    } else if decision.action == "CLOSE_SHORT" {
                                        "Short"
                                    } else {
                                        ""
                                    };
                                    if !blocked_side.is_empty() {
                                        let cooldown_minutes = cfg
                                            .stop_loss_cooldown_minutes_for_side(
                                                &symbol,
                                                &blocked_side.to_lowercase(),
                                            )
                                            .max(default_stop_loss_cooldown_minutes);
                                        entry_guards.insert(
                                            symbol.clone(),
                                            EntryGuardState {
                                                blocked_side: blocked_side.to_string(),
                                                cooldown_until: Instant::now()
                                                    + Duration::from_secs(
                                                        (cooldown_minutes * 60) as u64,
                                                    ),
                                                cooldown_minutes,
                                                cooldown_reason: "stop_loss".to_string(),
                                                awaiting_flip: true,
                                            },
                                        );
                                        signal_confirmations.remove(&symbol);
                                    }
                                } else if decision.reason.starts_with("thesis_invalidated") {
                                    let blocked_side = if decision.action == "CLOSE_LONG" {
                                        "Long"
                                    } else if decision.action == "CLOSE_SHORT" {
                                        "Short"
                                    } else {
                                        ""
                                    };
                                    if !blocked_side.is_empty() {
                                        let cooldown_minutes = cfg
                                            .thesis_invalidation_cooldown_minutes_for_side(
                                                &symbol,
                                                &blocked_side.to_lowercase(),
                                            );
                                        if cooldown_minutes > 0 {
                                            entry_guards.insert(
                                                symbol.clone(),
                                                EntryGuardState {
                                                    blocked_side: blocked_side.to_string(),
                                                    cooldown_until: Instant::now()
                                                        + Duration::from_secs(
                                                            (cooldown_minutes * 60) as u64,
                                                        ),
                                                    cooldown_minutes,
                                                    cooldown_reason: "thesis_invalidated".to_string(),
                                                    awaiting_flip: true,
                                                },
                                            );
                                            signal_confirmations.remove(&symbol);
                                        }
                                    }
                                }

                                let execution_time_ms =
                                    pipeline_started_at.elapsed().as_millis().min(i32::MAX as u128) as i32;

                                if matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
                                    let rank_score = decision_rank_score(&runtime_output);
                                    let entry_score = decision_entry_score(&runtime_output);
                                    let candidate = PendingEntryCandidate {
                                        signal_event: signal_event.clone(),
                                        decision,
                                        pipeline_input,
                                        runtime_output,
                                        execution_time_ms,
                                        rank_score,
                                        entry_score,
                                        created_at: Instant::now(),
                                    };
                                    match pending_candidates.get(&symbol) {
                                        Some(existing)
                                            if existing.rank_score > candidate.rank_score
                                                || (existing.rank_score == candidate.rank_score
                                                    && existing.entry_score >= candidate.entry_score) => {}
                                        _ => {
                                            pending_candidates.insert(symbol.clone(), candidate);
                                        }
                                    }
                                } else if let Err(err) = finalize_strategy_decision(
                                    &mut publish_conn,
                                    db_pool.as_ref(),
                                    FinalizeDecisionContext {
                                        signal_event: &signal_event,
                                        pipeline_input: &pipeline_input,
                                        runtime_output: &runtime_output,
                                        execution_time_ms,
                                    },
                                    decision,
                                )
                                .await
                                {
                                    error!(symbol = %signal_event.signal.symbol, err = %err, "Invalid strategy decision event contract");
                                }
                            }
                            Err(e) => {
                                error!(symbol = %signal_event.signal.symbol, err = %e, "Failed to parse strategy decision");
                            }
                        }
                    }
                }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn sample_cfg() -> StrategyConfig {
        StrategyConfig {
            profile: "TEST".to_string(),
            trading_mode: "PAPER".to_string(),
            global: json!({
                "mode_profiles": {
                    "PAPER": {
                        "stop_loss_pct": 0.01,
                        "fixed_take_profit_enabled": true,
                        "take_profit_pct": 0.02,
                        "min_hold_seconds": 0,
                        "trailing_enabled": true,
                        "trailing_stop": {
                            "activate_after_profit_pct": 0.01,
                            "initial_trail_pct": 0.005,
                            "ratchet_levels": [
                                { "at_profit_pct": 0.02, "trail_pct": 0.004 }
                            ],
                            "move_to_break_even_at": 0.015
                        }
                    }
                },
                "trailing_stop": {
                    "min_move_threshold_pct": 0.002
                }
            }),
            pairs: HashMap::new(),
            bollinger_std_dev_multiplier: 2.0,
            bollinger_invalidation_threshold: 0.7,
        }
    }

    // Sizing single-source-of-truth: a token with no per-symbol risk override
    // inherits the global mode-profile default; an override still wins.
    fn sizing_cfg() -> StrategyConfig {
        StrategyConfig {
            profile: "TEST".to_string(),
            trading_mode: "PAPER".to_string(),
            global: json!({
                "smart_copy": { "max_position_usdt": 30 },
                "mode_profiles": {
                    "PAPER": {
                        "risk": {
                            "max_position_wallet_pct": 0.08,
                            "atr_multiplier": 0.65,
                            "max_position_usdt": 18
                        }
                    }
                }
            }),
            pairs: HashMap::from([(
                "OVERRIDEUSDT".to_string(),
                json!({
                    "enabled": true,
                    "mode_profiles": { "PAPER": { "risk": { "max_position_wallet_pct": 0.12 } } },
                    "risk": { "atr_multiplier": 0.5, "max_position_usdt": 20 }
                }),
            )]),
            bollinger_std_dev_multiplier: 2.0,
            bollinger_invalidation_threshold: 0.7,
        }
    }

    #[test]
    fn new_token_inherits_global_sizing_defaults() {
        let cfg = sizing_cfg();
        // NEWUSDT has no per-symbol block (a freshly-added token) → global default.
        assert_eq!(cfg.max_position_wallet_pct("NEWUSDT"), Some(0.08));
        assert_eq!(cfg.atr_multiplier("NEWUSDT"), 0.65);
        assert_eq!(cfg.max_position_usdt("NEWUSDT"), 18.0);
    }

    #[test]
    fn per_symbol_sizing_override_wins_over_global_default() {
        let cfg = sizing_cfg();
        assert_eq!(cfg.max_position_wallet_pct("OVERRIDEUSDT"), Some(0.12));
        assert_eq!(cfg.atr_multiplier("OVERRIDEUSDT"), 0.5);
        assert_eq!(cfg.max_position_usdt("OVERRIDEUSDT"), 20.0);
    }

    fn sample_open_trade() -> OpenTradeSnapshot {
        OpenTradeSnapshot {
            trade_id: "trade-1".to_string(),
            side: "Long".to_string(),
            quantity: 10.0,
            entry_price: 100.0,
            opened_at: Utc::now() - ChronoDuration::seconds(600),
            trailing_stop_activated: false,
            trailing_stop_peak_price: 0.0,
            trailing_stop_final_distance_pct: 0.0,
        }
    }

    fn sample_market_signal() -> MarketSignal {
        MarketSignal {
            symbol: "DOGEUSDT".to_string(),
            current_price: 100.0,
            bybit_price: 100.0,
            atr_14: 1.0,
            adx_14: 25.0,
            volume_24h: 100_000_000,
            funding_rate: 0.0,
            trend_score: 0.0,
            spread_pct: 0.0,
            consensus_atr_14: 1.0,
            consensus_adx_14: 25.0,
            consensus_volume_24h: 100_000_000,
            consensus_funding_rate: 0.0,
            consensus_trend_score: 0.0,
            consensus_spread_pct: 0.0,
            consensus_trend_slope: 0.0,
            ema_fast: 100.0,
            ema_slow: 99.0,
            bollinger_upper: 103.0,
            bollinger_middle: 100.0,
            bollinger_lower: 97.0,
            bollinger_bandwidth: 0.06,
            bollinger_percent_b: 0.5,
            consensus_ema_fast: 100.0,
            consensus_ema_slow: 99.0,
            consensus_bollinger_upper: 102.5,
            consensus_bollinger_middle: 99.8,
            consensus_bollinger_lower: 97.1,
            consensus_bollinger_bandwidth: 0.0541,
            consensus_bollinger_percent_b: 0.5370,
            rsi_14: 50.0,
            consensus_rsi_14: 50.0,
            macd_line: 0.0,
            macd_signal: 0.0,
            macd_histogram: 0.0,
            consensus_macd_line: 0.0,
            consensus_macd_signal: 0.0,
            consensus_macd_histogram: 0.0,
            volume_ratio: 1.0,
            consensus_volume_ratio: 1.0,
            btc_regime: "neutral".to_string(),
            btc_trend_score: 0.0,
            btc_consensus_count: 0,
            btc_volume_ratio: 1.0,
            regime: "bullish".to_string(),
            consensus_side: "bullish".to_string(),
            consensus_count: 3,
            exchanges_available: 3,
            consensus_ratio: 1.0,
            trend_slope: 0.0,
            bybit_regime: "bullish".to_string(),
            bullish_exchanges: 3,
            bearish_exchanges: 0,
        }
    }

    #[test]
    fn evaluate_trailing_returns_score_and_reason_summary() {
        let open = sample_open_trade();
        let trailing = sample_cfg().trailing_runtime_config("DOGEUSDT");

        let eval = evaluate_trailing(&open, 103.0, &trailing).expect("trailing evaluation");

        assert!(eval.activated);
        assert!(eval.trailing_score > 0);
        assert!(eval.reason.contains("trailing_raw_"));
        assert!(eval.reason.contains("activation_progress"));
        assert!(eval.reason.contains("break_even"));
    }

    #[test]
    fn persists_small_favorable_peak_move_below_old_threshold() {
        // A Short whose peak improves ~0.05% — far below the old 0.2% gate that used
        // to freeze the persisted peak (and degrade the trail to break-even only).
        // With the geometry-matched threshold it must persist so the trail ratchets.
        let open = OpenTradeSnapshot {
            trade_id: "t".to_string(),
            side: "Short".to_string(),
            quantity: 1.0,
            entry_price: 100.0,
            opened_at: Utc::now(),
            trailing_stop_activated: true,
            trailing_stop_peak_price: 100.0,
            trailing_stop_final_distance_pct: 0.0006,
        };
        let eval = TrailingEval {
            activated: true,
            peak_price: 99.95, // favorable for a short: lower price = more profit
            trail_pct: 0.0006,
            trailing_stop_price: 99.95 * 1.0006,
            trailing_score: 50,
            reason: String::new(),
        };
        assert!(should_persist_trailing_update(&open, &eval, 0.0002));
        // The old coarse gate would have dropped this peak update.
        assert!(!should_persist_trailing_update(&open, &eval, 0.002));
    }

    #[test]
    fn skips_persist_when_peak_unchanged_and_no_ratchet() {
        let open = OpenTradeSnapshot {
            trade_id: "t".to_string(),
            side: "Short".to_string(),
            quantity: 1.0,
            entry_price: 100.0,
            opened_at: Utc::now(),
            trailing_stop_activated: true,
            trailing_stop_peak_price: 99.95,
            trailing_stop_final_distance_pct: 0.0006,
        };
        let eval = TrailingEval {
            activated: true,
            peak_price: 99.95,
            trail_pct: 0.0006,
            trailing_stop_price: 99.95 * 1.0006,
            trailing_score: 50,
            reason: String::new(),
        };
        assert!(!should_persist_trailing_update(&open, &eval, 0.0002));
    }

    #[test]
    fn active_position_advice_only_tightens_trailing() {
        // Invariant: advice must never make the live trail looser than the tuned
        // config (the backtest validates the config trail with advice = None, so a
        // looser live trail bleeds profit the backtest never predicts).
        let base = sample_cfg().trailing_runtime_config("DOGEUSDT");
        let advice = |action: &str| ActivePositionAdviceSnapshot {
            symbol: "DOGEUSDT".to_string(),
            side: "Long".to_string(),
            action: action.to_string(),
            confidence: "high".to_string(),
            maintenance_score: 50,
            market_state: "trending".to_string(),
            pnl_pct_estimate: 0.2,
            duration_minutes: 5,
            summary: String::new(),
            evidence: vec![],
            risk_flags: vec![],
        };
        for action in ["hold_but_tighten", "reduce_risk"] {
            let a = advice(action);
            let tuned = apply_active_position_advice_to_trailing(base.clone(), Some(&a));
            assert!(
                tuned.initial_trail_pct <= base.initial_trail_pct + 1e-12,
                "{action} loosened initial_trail_pct"
            );
            assert!(tuned.initial_trail_pct > 0.0);
            assert!(tuned.move_to_break_even_at <= base.move_to_break_even_at + 1e-12);
            for (t, b) in tuned.ratchet_levels.iter().zip(base.ratchet_levels.iter()) {
                assert!(
                    t.trail_pct <= b.trail_pct + 1e-12,
                    "{action} loosened ratchet trail_pct"
                );
                assert!(t.trail_pct > 0.0);
            }
        }
    }

    #[test]
    fn percent_b_limit_defaults_are_neutral() {
        let cfg = sample_cfg();
        // No %B keys configured => ±infinity limits that never block an entry.
        assert_eq!(
            cfg.percent_b_limit_for_side("BTCUSDT", "long"),
            f64::INFINITY
        );
        assert_eq!(
            cfg.percent_b_limit_for_side("BTCUSDT", "short"),
            f64::NEG_INFINITY
        );
    }

    #[test]
    fn evaluate_open_trade_exit_returns_take_profit_trigger() {
        let open = sample_open_trade();
        let cfg = sample_cfg();

        let eval = evaluate_open_trade_exit("DOGEUSDT", 102.5, &open, &cfg, None);

        assert_eq!(eval.trigger, "take_profit");
        assert!(eval.reason.contains("take_profit_triggered"));
        let decision = eval.decision.expect("close decision");
        assert_eq!(decision.action, "CLOSE_LONG");
        assert!(decision.reason.contains("take_profit_triggered"));
    }

    #[test]
    fn trailing_stop_fires_during_min_hold() {
        // A protective trailing-stop exit must fire even WITHIN the min_hold window —
        // min_hold defers only the discretionary thesis exit, never the risk stops.
        // (The bug: an armed trail's locked profit bled back during min_hold because
        // the exit was suppressed; e.g. WIF peaked +0.71%, fell through the lock, no SL.)
        let cfg = StrategyConfig {
            profile: "TEST".to_string(),
            trading_mode: "PAPER".to_string(),
            global: json!({
                "mode_profiles": { "PAPER": {
                    "stop_loss_pct": 0.012,
                    "fixed_take_profit_enabled": false,
                    "trailing_enabled": true,
                    "min_hold_seconds": 300,
                    "trailing_stop": {
                        "activate_after_profit_pct": 0.001,
                        "initial_trail_pct": 0.0006,
                        "ratchet_levels": [],
                        "move_to_break_even_at": 0.0016
                    }
                }},
                "trailing_stop": { "min_move_threshold_pct": 0.0002 }
            }),
            pairs: HashMap::new(),
            bollinger_std_dev_multiplier: 2.0,
            bollinger_invalidation_threshold: 0.7,
        };
        // Long opened 10s ago (inside the 300s hold), trailing already armed at a
        // +0.5% peak; price has fallen below the trailing stop (100.5*(1-0.001)=100.3995).
        let open = OpenTradeSnapshot {
            trade_id: "t".to_string(),
            side: "Long".to_string(),
            quantity: 1.0,
            entry_price: 100.0,
            opened_at: Utc::now() - ChronoDuration::seconds(10),
            trailing_stop_activated: true,
            trailing_stop_peak_price: 100.5,
            trailing_stop_final_distance_pct: 0.001,
        };
        let eval = evaluate_open_trade_exit("SOLUSDT", 100.20, &open, &cfg, None);
        // Inside min_hold, but the trailing stop still triggers the close.
        assert_eq!(eval.trigger, "trailing_stop");
        assert_eq!(eval.decision.expect("close decision").action, "CLOSE_LONG");
    }

    #[test]
    fn audit_step_summarizes_decision_state() {
        let input = StrategyInput {
            symbol: "DOGEUSDT".to_string(),
            temporal: json!({
                "decision": {
                    "action": "ENTER_LONG",
                    "reason": "entry_confirmed",
                    "decision_score": 87.0,
                    "smart_copy_compatible": true
                }
            }),
            account_equity_usdt: 1000.0,
            config: json!({}),
            signal: json!({}),
        };

        let audit = step_audit(&input);

        assert_eq!(audit["ok"], json!(true));
        assert_eq!(audit["decision_action"], json!("HOLD"));
        assert_eq!(audit["decision_score"], json!(100.0));
        assert_eq!(audit["smart_copy_compatible"], json!(false));
    }

    #[test]
    fn structured_hold_reason_from_state_uses_non_default_reasons() {
        let state = json!({
            "validate_entry": { "reason": "entry_blocked_low_volume" },
            "check_funding": { "reason": "funding_validated" },
            "calc_smart_size": { "reason": "size_proposed_proposal_raw_100_clamped_100" },
            "validate_size": { "reason": "size_validated_size_raw_100_clamped_100" },
            "get_trailing_config": { "reason": "trailing_pending_runtime" },
            "signal_confirmation": { "reason": "signal_confirmation" },
            "cooldown_guard": { "reason": "cooldown_guard" },
            "thesis_confirmation": { "reason": "thesis_confirmation" }
        });

        let reason = structured_hold_reason_from_state(&state);

        assert!(reason.contains("entry_blocked_low_volume"));
        assert!(reason.contains("funding_validated"));
        assert!(reason.contains("size_proposed_proposal_raw_100_clamped_100"));
        assert!(reason.contains("size_validated_size_raw_100_clamped_100"));
        assert!(reason.contains("signal_confirmation"));
        assert!(reason.contains("cooldown_guard"));
        assert!(reason.contains("thesis_confirmation"));
        assert!(!reason.contains("risk_constraints_not_met"));
    }

    #[test]
    fn structured_temporal_reason_from_state_uses_temporal_steps() {
        let state = json!({
            "signal_confirmation": { "reason": "signal_confirmation" },
            "cooldown_guard": { "reason": "cooldown_guard" },
            "thesis_confirmation": { "reason": "thesis_confirmation" }
        });

        let reason =
            structured_temporal_reason_from_state(&state).expect("temporal reason should exist");

        assert!(reason.contains("signal_confirmation"));
        assert!(reason.contains("cooldown_guard"));
        assert!(reason.contains("thesis_confirmation"));
    }

    #[test]
    fn temporal_confirmation_reason_reports_remaining_hits() {
        let reason = temporal_confirmation_reason("thesis_confirmation", 1, 3, "base_reason");

        assert!(reason.contains("thesis_confirmation_pending_1"));
        assert!(reason.contains("remaining_2"));
        assert!(reason.contains("base_reason"));
    }

    #[test]
    fn thesis_guard_policy_uses_temporal_confirmation_reason() {
        let evaluation = ThesisInvalidationEvaluation {
            stage: "invalidated",
            reason: "thesis_invalidated_health_threshold".to_string(),
            health_score: -42,
        };
        let mut invalidations = HashMap::new();

        let guard =
            evaluate_thesis_guard_policy("DOGEUSDT", "Long", &evaluation, &mut invalidations, 3);

        assert!(!guard.confirmed);
        assert!(guard.reason.contains("thesis_confirmation_pending_1"));
        assert!(guard.reason.contains("remaining_2"));
        assert!(guard.reason.contains("health_-42"));
    }

    #[test]
    fn long_thesis_does_not_invalidate_on_weak_alignment_noise() {
        let open = sample_open_trade();
        let mut signal = sample_market_signal();
        signal.consensus_side = "neutral".to_string();
        signal.bybit_regime = "bullish".to_string();
        signal.btc_regime = "neutral".to_string();
        signal.current_price = 100.0;
        signal.bybit_price = 100.0;
        signal.consensus_ema_fast = 100.0;
        signal.consensus_ema_slow = 99.0;
        signal.consensus_macd_histogram = -0.01;
        signal.consensus_bollinger_percent_b = 0.40;

        let evaluation = evaluate_thesis_invalidation(&signal, &open, &sample_cfg());

        assert_eq!(evaluation.stage, "valid");
        assert!(evaluation.reason.starts_with("thesis_valid_"));
    }

    #[test]
    fn long_thesis_invalidates_when_multiple_core_components_break() {
        let open = sample_open_trade();
        let mut signal = sample_market_signal();
        signal.consensus_side = "neutral".to_string();
        signal.bybit_regime = "neutral".to_string();
        signal.consensus_trend_score = -0.62;
        signal.trend_score = -0.58;
        signal.btc_trend_score = -0.55;
        signal.current_price = 99.0;
        signal.bybit_price = 99.0;
        signal.consensus_ema_fast = 100.0;
        signal.consensus_ema_slow = 99.0;
        signal.consensus_bollinger_percent_b = 0.10;

        let evaluation = evaluate_thesis_invalidation(&signal, &open, &sample_cfg());

        assert_eq!(evaluation.stage, "invalidated");
        assert!(evaluation
            .reason
            .contains("thesis_invalidated_no_bullish_alignment"));
    }

    #[test]
    fn long_thesis_enters_degrading_before_invalidation() {
        let open = sample_open_trade();
        let mut signal = sample_market_signal();
        signal.consensus_side = "neutral".to_string();
        signal.bybit_regime = "neutral".to_string();
        signal.btc_regime = "bearish".to_string();
        signal.trend_score = -0.12;
        signal.current_price = 100.0;
        signal.bybit_price = 100.0;
        signal.consensus_ema_fast = 100.0;
        signal.consensus_ema_slow = 99.0;
        signal.consensus_bollinger_percent_b = 0.20;
        signal.consensus_macd_histogram = -1.0;

        let evaluation = evaluate_thesis_invalidation(&signal, &open, &sample_cfg());

        assert_eq!(evaluation.stage, "degrading_soft");
        assert!(evaluation
            .reason
            .contains("thesis_degrading_soft_long_alignment"));
    }

    #[test]
    fn long_thesis_enters_hard_degrading_before_invalidation() {
        let open = sample_open_trade();
        let mut signal = sample_market_signal();
        signal.consensus_side = "neutral".to_string();
        signal.bybit_regime = "neutral".to_string();
        signal.consensus_trend_score = -0.22;
        signal.trend_score = -0.24;
        signal.consensus_macd_histogram = -1.0;
        signal.current_price = 99.0;
        signal.bybit_price = 99.0;
        signal.consensus_ema_fast = 100.0;
        signal.consensus_ema_slow = 99.0;
        signal.consensus_bollinger_percent_b = 0.20;

        let evaluation = evaluate_thesis_invalidation(&signal, &open, &sample_cfg());

        assert_eq!(evaluation.stage, "degrading_hard");
        assert!(evaluation
            .reason
            .contains("thesis_degrading_hard_long_alignment"));
    }

    #[test]
    fn short_thesis_uses_degrading_before_invalidation() {
        let mut open = sample_open_trade();
        open.side = "Short".to_string();
        let mut signal = sample_market_signal();
        signal.consensus_side = "neutral".to_string();
        signal.bybit_regime = "bearish".to_string();
        signal.consensus_trend_score = -0.22;
        signal.trend_score = -0.18;
        signal.current_price = 99.0;
        signal.bybit_price = 99.0;
        signal.consensus_ema_fast = 100.0;
        signal.consensus_ema_slow = 101.0;
        signal.consensus_bollinger_percent_b = 0.35;

        let evaluation = evaluate_thesis_invalidation(&signal, &open, &sample_cfg());

        assert_eq!(evaluation.stage, "degrading_hard");
        assert!(evaluation
            .reason
            .contains("thesis_degrading_hard_short_alignment"));
    }

    fn strategy_input_with_equity(equity: f64) -> StrategyInput {
        StrategyInput {
            symbol: "BTCUSDT".to_string(),
            temporal: json!({}),
            account_equity_usdt: equity,
            config: json!({}),
            signal: json!({}),
        }
    }

    fn strategy_input_with_signal(equity: f64) -> StrategyInput {
        let sig = sample_market_signal();
        StrategyInput {
            symbol: sig.symbol.clone(),
            temporal: json!({ "cooldown_guard": { "active": false } }),
            account_equity_usdt: equity,
            config: json!({}),
            signal: serde_json::to_value(&sig).unwrap(),
        }
    }

    #[test]
    fn build_base_state_flattens_signal_and_account() {
        let sig = sample_market_signal();
        let input = strategy_input_with_signal(1234.0);
        let state = build_base_state(&input);
        // market feature from the signal is flattened to the top level
        assert_eq!(get_f64(&state, "trend_score", f64::NAN), sig.trend_score);
        assert_eq!(get_string(&state, "symbol", ""), sig.symbol);
        assert_eq!(get_f64(&state, "account_equity_usdt", 0.0), 1234.0);
        // safe defaults for unsourced account-history guards
        assert_eq!(get_f64(&state, "current_daily_loss", -1.0), 0.0);
        assert_eq!(get_i64(&state, "consecutive_losses", -1), 0);
        // temporal merged
        assert!(state.get("cooldown_guard").is_some());
    }

    #[test]
    fn real_decision_runs_end_to_end_and_returns_valid_action() {
        let input = strategy_input_with_signal(1000.0);
        let cfg = sample_cfg();
        let decision = run_steps_through(&input, &cfg, "decision");
        let action = get_string(&decision, "action", "");
        assert!(
            matches!(action.as_str(), "ENTER_LONG" | "ENTER_SHORT" | "HOLD"),
            "unexpected decision action: {action}"
        );
    }

    #[test]
    fn decision_step_defaults_to_hold_when_real_logic_disabled() {
        // STRATEGY_CFG is unset in tests, so real_cfg() is None → stub path.
        let decision = step_decision(&strategy_input_with_signal(1000.0));
        assert_eq!(get_string(&decision, "action", ""), "HOLD");
    }

    #[tokio::test]
    async fn equity_floor_constraint_flags_negative_equity() {
        let executor = Executor::new();
        let pipeline = ViperSmartCopy::new();

        // Negative equity is a data fault → the typed constraint must fail.
        let res = executor
            .run_parallel(&pipeline, &strategy_input_with_equity(-5.0))
            .await
            .expect("pipeline should run");
        assert!(!res.passed, "negative equity must violate the constraint");
        assert!(
            res.failures.iter().any(|f| f.metric == "equity_floor"),
            "expected an equity_floor failure, got {:?}",
            res.failures
        );
    }

    #[tokio::test]
    async fn equity_floor_constraint_passes_for_non_negative_equity() {
        let executor = Executor::new();
        let pipeline = ViperSmartCopy::new();

        let res = executor
            .run_parallel(&pipeline, &strategy_input_with_equity(1000.0))
            .await
            .expect("pipeline should run");
        assert!(
            res.passed,
            "non-negative equity must satisfy the constraint"
        );
        assert!(
            res.failures.is_empty(),
            "unexpected failures: {:?}",
            res.failures
        );
    }
}
