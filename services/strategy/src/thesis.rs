use std::collections::HashMap;

use viper_domain::{MarketSignal, StrategyDecision};

use crate::{
    clamp_i32, create_close_decision, create_hold_decision, current_profit_pct, directional_points,
    push_weighted_health_component, temporal_confirmation_reason, OpenTradeSnapshot,
    PositionHealthBreakdown, StrategyConfig, ThesisGuardEvaluation, ThesisInvalidationEvaluation,
    ThesisInvalidationState,
};

#[derive(Debug, Clone)]
pub(crate) struct ThesisHealthCfg {
    long_invalidate: i32,
    long_invalidate_confirmed: i32,
    long_no_alignment: i32,
    long_degrading_hard: i32,
    long_degrading_hard_unaligned: i32,
    long_degrading_soft: i32,
    long_degrading_soft_unaligned: i32,
    short_invalidate: i32,
    short_no_alignment: i32,
    short_degrading_hard: i32,
    short_degrading_hard_unaligned: i32,
    short_degrading_soft: i32,
    short_degrading_soft_unaligned: i32,
    in_profit_pct: f64,
    required_components_in_profit: i32,
    required_components: i32,
    opposite_side_exit: String,
}

impl StrategyConfig {
    pub(crate) fn thesis_health(&self) -> ThesisHealthCfg {
        let n = |key: &str, def: i32| -> i32 {
            self.mode_cfg()
                .and_then(|m| crate::cfg_get(m, &["thesis_health", key]))
                .and_then(serde_json::Value::as_i64)
                .or_else(|| {
                    crate::cfg_get(&self.global, &["thesis_health", key])
                        .and_then(serde_json::Value::as_i64)
                })
                .map(|v| v as i32)
                .unwrap_or(def)
        };
        let in_profit_pct = self
            .mode_cfg()
            .and_then(|m| crate::cfg_get(m, &["thesis_health", "in_profit_pct"]))
            .and_then(serde_json::Value::as_f64)
            .or_else(|| {
                crate::cfg_get(&self.global, &["thesis_health", "in_profit_pct"])
                    .and_then(serde_json::Value::as_f64)
            })
            .unwrap_or(0.002);
        let opposite_side_exit = self
            .mode_cfg()
            .and_then(|m| crate::cfg_get(m, &["thesis_health", "opposite_side_exit"]))
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                crate::cfg_get(&self.global, &["thesis_health", "opposite_side_exit"])
                    .and_then(serde_json::Value::as_str)
            })
            .unwrap_or("any")
            .to_string();
        ThesisHealthCfg {
            long_invalidate: n("long_invalidate", -60),
            long_invalidate_confirmed: n("long_invalidate_confirmed", -50),
            long_no_alignment: n("long_no_alignment", -35),
            long_degrading_hard: n("long_degrading_hard", -35),
            long_degrading_hard_unaligned: n("long_degrading_hard_unaligned", -20),
            long_degrading_soft: n("long_degrading_soft", -20),
            long_degrading_soft_unaligned: n("long_degrading_soft_unaligned", -12),
            short_invalidate: n("short_invalidate", 55),
            short_no_alignment: n("short_no_alignment", 35),
            short_degrading_hard: n("short_degrading_hard", 40),
            short_degrading_hard_unaligned: n("short_degrading_hard_unaligned", 25),
            short_degrading_soft: n("short_degrading_soft", 25),
            short_degrading_soft_unaligned: n("short_degrading_soft_unaligned", 15),
            in_profit_pct,
            required_components_in_profit: n("required_components_in_profit", 3),
            required_components: n("required_components", 2),
            opposite_side_exit,
        }
    }
}

pub(crate) fn position_health_breakdown(
    signal: &MarketSignal,
    open: &OpenTradeSnapshot,
) -> PositionHealthBreakdown {
    let is_long = open.side.eq_ignore_ascii_case("Long");
    let favorable = if is_long { "bullish" } else { "bearish" };
    let unfavorable = if is_long { "bearish" } else { "bullish" };
    let sign = if is_long { 1.0 } else { -1.0 };
    let price = if signal.bybit_price > 0.0 {
        signal.bybit_price
    } else {
        signal.current_price
    };

    let mut components = Vec::new();

    let consensus_side = directional_points(&signal.consensus_side, favorable, unfavorable, 1);
    push_weighted_health_component(
        &mut components,
        "consensus_side",
        consensus_side as f64,
        30.0,
    );

    let bybit_regime = directional_points(&signal.bybit_regime, favorable, unfavorable, 1);
    push_weighted_health_component(&mut components, "bybit_regime", bybit_regime as f64, 20.0);

    let btc_regime = directional_points(&signal.btc_regime, favorable, unfavorable, 1);
    push_weighted_health_component(&mut components, "btc_regime", btc_regime as f64, 10.0);

    let consensus_trend_score = (signal.consensus_trend_score * sign).clamp(-0.4, 0.4);
    push_weighted_health_component(
        &mut components,
        "consensus_trend_score",
        consensus_trend_score,
        50.0,
    );

    let bybit_trend_score = (signal.trend_score * sign).clamp(-10.0 / 30.0, 10.0 / 30.0);
    push_weighted_health_component(
        &mut components,
        "bybit_trend_score",
        bybit_trend_score,
        30.0,
    );

    let btc_trend_score = (signal.btc_trend_score * sign).clamp(-10.0 / 25.0, 10.0 / 25.0);
    push_weighted_health_component(&mut components, "btc_trend_score", btc_trend_score, 25.0);

    let macd_histogram = if signal.consensus_macd_histogram * sign > 0.0 {
        1.0
    } else if signal.consensus_macd_histogram * sign < 0.0 {
        -1.0
    } else {
        0.0
    };
    push_weighted_health_component(
        &mut components,
        "consensus_macd_histogram",
        macd_histogram,
        5.0,
    );

    let consensus_bollinger_percent_b = signal.consensus_bollinger_percent_b;
    let bollinger_position = if is_long {
        if consensus_bollinger_percent_b >= 0.55 {
            1.0
        } else if consensus_bollinger_percent_b >= 0.25 {
            0.0
        } else if consensus_bollinger_percent_b >= 0.15 {
            -0.5
        } else {
            -1.0
        }
    } else if consensus_bollinger_percent_b <= 0.45 {
        1.0
    } else if consensus_bollinger_percent_b <= 0.75 {
        0.0
    } else if consensus_bollinger_percent_b <= 0.85 {
        -0.5
    } else {
        -1.0
    };
    push_weighted_health_component(
        &mut components,
        "bollinger_position",
        bollinger_position,
        5.0,
    );

    if is_long {
        let ema_alignment = if signal.consensus_ema_fast > signal.consensus_ema_slow {
            1.0
        } else if signal.consensus_ema_fast < signal.consensus_ema_slow {
            -1.0
        } else {
            0.0
        };
        push_weighted_health_component(&mut components, "ema_alignment", ema_alignment, 5.0);

        let price_vs_fast_ema = if price >= signal.consensus_ema_fast {
            1.0
        } else {
            -1.0
        };
        push_weighted_health_component(
            &mut components,
            "price_vs_fast_ema",
            price_vs_fast_ema,
            5.0,
        );
    } else {
        let ema_alignment = if signal.consensus_ema_fast < signal.consensus_ema_slow {
            1.0
        } else if signal.consensus_ema_fast > signal.consensus_ema_slow {
            -1.0
        } else {
            0.0
        };
        push_weighted_health_component(&mut components, "ema_alignment", ema_alignment, 5.0);

        let price_vs_fast_ema = if price <= signal.consensus_ema_fast {
            1.0
        } else {
            -1.0
        };
        push_weighted_health_component(
            &mut components,
            "price_vs_fast_ema",
            price_vs_fast_ema,
            5.0,
        );
    }

    let raw_score = components
        .iter()
        .map(|component| component.contribution)
        .sum();
    let clamped_score = clamp_i32(raw_score, -100, 100);

    PositionHealthBreakdown {
        raw_score,
        clamped_score,
        components,
    }
}

pub(crate) fn position_health_summary(breakdown: &PositionHealthBreakdown) -> String {
    let mut components = breakdown.components.clone();
    components.sort_by_key(|component| component.contribution.abs());
    components.reverse();

    let reasons = components
        .into_iter()
        .take(3)
        .map(|component| {
            format!(
                "{}:{:.3}x{:.1}={}",
                component.reason, component.score, component.weight, component.contribution
            )
        })
        .collect::<Vec<_>>();

    if reasons.is_empty() {
        format!(
            "health_raw_{}_clamped_{}",
            breakdown.raw_score, breakdown.clamped_score
        )
    } else {
        format!(
            "health_raw_{}_clamped_{}_{}",
            breakdown.raw_score,
            breakdown.clamped_score,
            reasons.join("__")
        )
    }
}

pub(crate) fn adverse_long_thesis_components(breakdown: &PositionHealthBreakdown) -> usize {
    breakdown
        .components
        .iter()
        .filter(|component| {
            component.contribution < 0
                && matches!(
                    component.reason,
                    "consensus_trend_score"
                        | "bybit_trend_score"
                        | "btc_trend_score"
                        | "price_vs_fast_ema"
                )
        })
        .count()
}

pub(crate) fn thesis_degrading_confirmation_reason(
    current_hits: usize,
    required_hits: usize,
    base_reason: &str,
) -> String {
    temporal_confirmation_reason(
        "thesis_degrading_confirmation",
        current_hits,
        required_hits,
        base_reason,
    )
}

pub(crate) fn evaluate_thesis_invalidation(
    signal: &MarketSignal,
    open: &OpenTradeSnapshot,
    cfg: &StrategyConfig,
) -> ThesisInvalidationEvaluation {
    let breakdown = position_health_breakdown(signal, open);
    let health_score = breakdown.clamped_score;
    let th = cfg.thesis_health();

    if open.side.eq_ignore_ascii_case("Long") {
        let profit_pct = current_profit_pct(&open.side, open.entry_price, signal.current_price);
        let in_profit = profit_pct > th.in_profit_pct;
        let opposite_side = match th.opposite_side_exit.as_str() {
            "off" => false,
            "both" => {
                signal.consensus_side.eq_ignore_ascii_case("bearish")
                    && signal.bybit_regime.eq_ignore_ascii_case("bearish")
            }
            _ => {
                signal.consensus_side.eq_ignore_ascii_case("bearish")
                    || signal.bybit_regime.eq_ignore_ascii_case("bearish")
            }
        };
        let both_not_bullish = !signal.consensus_side.eq_ignore_ascii_case("bullish")
            && !signal.bybit_regime.eq_ignore_ascii_case("bullish");
        let adverse_components = adverse_long_thesis_components(&breakdown);
        let required_invalid_components = (if in_profit {
            th.required_components_in_profit
        } else {
            th.required_components
        })
        .max(0) as usize;

        let (stage, reason) = if opposite_side {
            ("invalidated", "thesis_invalidated_opposite_side")
        } else if health_score <= th.long_invalidate
            || (health_score <= th.long_invalidate_confirmed
                && adverse_components >= required_invalid_components)
        {
            ("invalidated", "thesis_invalidated_health_threshold")
        } else if both_not_bullish
            && health_score <= th.long_no_alignment
            && adverse_components >= required_invalid_components
        {
            ("invalidated", "thesis_invalidated_no_bullish_alignment")
        } else if (health_score <= th.long_degrading_hard && adverse_components >= 2)
            || (both_not_bullish
                && health_score <= th.long_degrading_hard_unaligned
                && adverse_components >= 2)
        {
            ("degrading_hard", "thesis_degrading_hard_long_alignment")
        } else if health_score <= th.long_degrading_soft
            || (both_not_bullish
                && health_score <= th.long_degrading_soft_unaligned
                && adverse_components >= 1)
        {
            ("degrading_soft", "thesis_degrading_soft_long_alignment")
        } else {
            ("valid", "thesis_valid")
        };

        ThesisInvalidationEvaluation {
            stage,
            reason: format!("{}_{}", reason, position_health_summary(&breakdown)),
            health_score,
        }
    } else if open.side.eq_ignore_ascii_case("Short") {
        let opposite_side = match th.opposite_side_exit.as_str() {
            "off" => false,
            "both" => {
                signal.consensus_side.eq_ignore_ascii_case("bullish")
                    && signal.bybit_regime.eq_ignore_ascii_case("bullish")
            }
            _ => {
                signal.consensus_side.eq_ignore_ascii_case("bullish")
                    || signal.bybit_regime.eq_ignore_ascii_case("bullish")
            }
        };
        let both_not_bearish = !signal.consensus_side.eq_ignore_ascii_case("bearish")
            && !signal.bybit_regime.eq_ignore_ascii_case("bearish");

        let (stage, reason) = if opposite_side {
            ("invalidated", "thesis_invalidated_opposite_side")
        } else if health_score >= th.short_invalidate {
            ("invalidated", "thesis_invalidated_health_threshold")
        } else if both_not_bearish && health_score >= th.short_no_alignment {
            ("invalidated", "thesis_invalidated_no_bearish_alignment")
        } else if health_score >= th.short_degrading_hard
            || (both_not_bearish && health_score >= th.short_degrading_hard_unaligned)
        {
            ("degrading_hard", "thesis_degrading_hard_short_alignment")
        } else if health_score >= th.short_degrading_soft
            || (both_not_bearish && health_score >= th.short_degrading_soft_unaligned)
        {
            ("degrading_soft", "thesis_degrading_soft_short_alignment")
        } else {
            ("valid", "thesis_valid")
        };

        ThesisInvalidationEvaluation {
            stage,
            reason: format!("{}_{}", reason, position_health_summary(&breakdown)),
            health_score,
        }
    } else {
        ThesisInvalidationEvaluation {
            stage: "valid",
            reason: "thesis_valid_unknown_side".to_string(),
            health_score,
        }
    }
}

pub(crate) fn evaluate_thesis_guard_policy(
    symbol: &str,
    open_side: &str,
    evaluation: &ThesisInvalidationEvaluation,
    thesis_invalidations: &mut HashMap<String, ThesisInvalidationState>,
    required_ticks: usize,
) -> ThesisGuardEvaluation {
    let state = thesis_invalidations
        .entry(symbol.to_string())
        .or_insert_with(|| ThesisInvalidationState {
            side: open_side.to_string(),
            consecutive_invalid_ticks: 0,
            consecutive_degrading_ticks: 0,
            bollinger_invalidated: false,
            bollinger_consecutive_hits: 0,
        });

    if !state.side.eq_ignore_ascii_case(open_side) {
        state.side = open_side.to_string();
        state.consecutive_invalid_ticks = 1;
        state.consecutive_degrading_ticks = 0;
    } else {
        state.consecutive_invalid_ticks += 1;
        state.consecutive_degrading_ticks = 0;
    }

    if state.consecutive_invalid_ticks < required_ticks {
        ThesisGuardEvaluation {
            confirmed: false,
            reason: format!(
                "{}_health_{}_{}",
                temporal_confirmation_reason(
                    "thesis_confirmation",
                    state.consecutive_invalid_ticks,
                    required_ticks,
                    "thesis_invalidation"
                ),
                evaluation.health_score,
                evaluation.reason
            ),
        }
    } else {
        ThesisGuardEvaluation {
            confirmed: true,
            reason: evaluation.reason.clone(),
        }
    }
}

pub(crate) fn evaluate_thesis_degrading_policy(
    symbol: &str,
    open_side: &str,
    evaluation: &ThesisInvalidationEvaluation,
    thesis_invalidations: &mut HashMap<String, ThesisInvalidationState>,
    required_ticks: usize,
) -> ThesisGuardEvaluation {
    let state = thesis_invalidations
        .entry(symbol.to_string())
        .or_insert_with(|| ThesisInvalidationState {
            side: open_side.to_string(),
            consecutive_invalid_ticks: 0,
            consecutive_degrading_ticks: 0,
            bollinger_invalidated: false,
            bollinger_consecutive_hits: 0,
        });

    if !state.side.eq_ignore_ascii_case(open_side) {
        state.side = open_side.to_string();
        state.consecutive_invalid_ticks = 0;
        state.consecutive_degrading_ticks = 1;
    } else {
        state.consecutive_invalid_ticks = 0;
        state.consecutive_degrading_ticks += 1;
    }

    if state.consecutive_degrading_ticks < required_ticks {
        ThesisGuardEvaluation {
            confirmed: false,
            reason: format!(
                "{}_health_{}_{}",
                thesis_degrading_confirmation_reason(
                    state.consecutive_degrading_ticks,
                    required_ticks,
                    "thesis_degrading"
                ),
                evaluation.health_score,
                evaluation.reason
            ),
        }
    } else {
        ThesisGuardEvaluation {
            confirmed: true,
            reason: format!(
                "thesis_invalidated_degrading_persisted_{}",
                evaluation.reason
            ),
        }
    }
}

pub(crate) fn enforce_open_position_thesis_guard(
    symbol: &str,
    signal: &MarketSignal,
    open: &OpenTradeSnapshot,
    cfg: &StrategyConfig,
    thesis_invalidations: &mut HashMap<String, ThesisInvalidationState>,
) -> Option<StrategyDecision> {
    if !cfg.thesis_invalidation_enabled() {
        thesis_invalidations.remove(symbol);
        return None;
    }

    let evaluation = evaluate_thesis_invalidation(signal, open, cfg);

    if evaluation.stage == "valid" {
        thesis_invalidations.remove(symbol);
        return None;
    }

    if evaluation.stage == "degrading_soft" {
        thesis_invalidations.remove(symbol);
        return Some(create_hold_decision(symbol, &evaluation.reason));
    }

    if evaluation.stage == "degrading_hard" {
        let required_ticks = cfg.thesis_degrading_confirmation_ticks(symbol);
        let guard_evaluation = evaluate_thesis_degrading_policy(
            symbol,
            &open.side,
            &evaluation,
            thesis_invalidations,
            required_ticks,
        );

        if !guard_evaluation.confirmed {
            return Some(create_hold_decision(symbol, &guard_evaluation.reason));
        }

        thesis_invalidations.remove(symbol);
        return create_close_decision(
            symbol,
            &open.side,
            open.quantity,
            signal.current_price,
            &guard_evaluation.reason,
        );
    }

    let required_ticks = cfg.thesis_invalidation_confirmation_ticks(symbol);
    let guard_evaluation = evaluate_thesis_guard_policy(
        symbol,
        &open.side,
        &evaluation,
        thesis_invalidations,
        required_ticks,
    );

    if !guard_evaluation.confirmed {
        return Some(create_hold_decision(symbol, &guard_evaluation.reason));
    }

    thesis_invalidations.remove(symbol);
    create_close_decision(
        symbol,
        &open.side,
        open.quantity,
        signal.current_price,
        &guard_evaluation.reason,
    )
}
