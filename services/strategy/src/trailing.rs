use viper_domain::StrategyDecision;

use crate::config::StrategyConfig;
use crate::helpers::*;
use crate::{
    ActivePositionAdviceSnapshot, ExitEvaluation, OpenTradeSnapshot, TrailingEval,
    TrailingPolicyBreakdown, TrailingPolicyComponent, TrailingRuntimeConfig,
};

pub(crate) fn apply_active_position_advice_to_trailing(
    mut trailing: TrailingRuntimeConfig,
    advice: Option<&ActivePositionAdviceSnapshot>,
) -> TrailingRuntimeConfig {
    let Some(advice) = advice else {
        return trailing;
    };

    match advice.action.as_str() {
        "hold_but_tighten" => {
            trailing.move_to_break_even_at *= 0.85;
            trailing.initial_trail_pct *= 0.9;
            for level in &mut trailing.ratchet_levels {
                level.trail_pct *= 0.92;
            }
        }
        "reduce_risk" => {
            trailing.activate_after_profit_pct *= 0.6;
            trailing.move_to_break_even_at *= 0.5;
            trailing.initial_trail_pct *= 0.65;
            for level in &mut trailing.ratchet_levels {
                level.at_profit_pct *= 0.8;
                level.trail_pct *= 0.7;
            }
        }
        _ => {}
    }

    trailing
}

pub(crate) fn create_close_decision(
    symbol: &str,
    side: &str,
    quantity: f64,
    close_price: f64,
    reason: &str,
) -> Option<StrategyDecision> {
    let action = match side {
        "Long" => "CLOSE_LONG",
        "Short" => "CLOSE_SHORT",
        _ => return None,
    };

    Some(StrategyDecision {
        action: action.to_string(),
        symbol: symbol.to_string(),
        quantity,
        leverage: 0.0,
        entry_price: close_price,
        stop_loss: 0.0,
        take_profit: 0.0,
        reason: reason.to_string(),
        smart_copy_compatible: true,
    })
}

pub(crate) fn current_profit_pct(side: &str, entry: f64, current: f64) -> f64 {
    if entry <= 0.0 || current <= 0.0 {
        return 0.0;
    }
    if side == "Long" {
        (current - entry) / entry
    } else {
        (entry - current) / entry
    }
}

pub(crate) fn evaluate_trailing(
    open: &OpenTradeSnapshot,
    current_price: f64,
    trailing: &TrailingRuntimeConfig,
) -> Option<TrailingEval> {
    if !trailing.enabled || current_price <= 0.0 || open.entry_price <= 0.0 {
        return None;
    }

    let profit_pct = current_profit_pct(&open.side, open.entry_price, current_price);
    let mut activated =
        open.trailing_stop_activated || profit_pct >= trailing.activate_after_profit_pct;
    if !activated {
        return None;
    }

    let mut peak_price = if open.trailing_stop_peak_price > 0.0 {
        open.trailing_stop_peak_price
    } else {
        open.entry_price
    };

    if open.side == "Long" {
        peak_price = peak_price.max(current_price);
    } else {
        peak_price = peak_price.min(current_price);
    }

    let mut trail_pct = trailing.initial_trail_pct;
    let mut ratchet_level = 0_i32;
    for (idx, level) in trailing.ratchet_levels.iter().enumerate() {
        if profit_pct >= level.at_profit_pct {
            trail_pct = level.trail_pct;
            ratchet_level = (idx + 1) as i32;
        }
    }

    if open.trailing_stop_final_distance_pct > 0.0 {
        trail_pct = trail_pct.max(open.trailing_stop_final_distance_pct);
    }

    let mut trailing_stop_price = if open.side == "Long" {
        peak_price * (1.0 - trail_pct)
    } else {
        peak_price * (1.0 + trail_pct)
    };

    let break_even_armed = profit_pct >= trailing.move_to_break_even_at;
    if break_even_armed {
        if open.side == "Long" {
            trailing_stop_price = trailing_stop_price.max(open.entry_price);
        } else {
            trailing_stop_price = trailing_stop_price.min(open.entry_price);
        }
    }

    let activation_score = if trailing.activate_after_profit_pct > 0.0 {
        (profit_pct / trailing.activate_after_profit_pct).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let ratchet_score = if trailing.ratchet_levels.is_empty() {
        0.0
    } else {
        (ratchet_level as f64 / trailing.ratchet_levels.len() as f64).clamp(0.0, 1.0)
    };
    let break_even_score = if break_even_armed { 1.0 } else { 0.0 };
    let trail_tightness_score = if trail_pct > 0.0 {
        (1.0 - trail_pct / 0.03).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let mut components = Vec::new();
    push_weighted_trailing_component(
        &mut components,
        "activation_progress",
        activation_score,
        35.0,
    );
    push_weighted_trailing_component(&mut components, "ratchet_progress", ratchet_score, 25.0);
    push_weighted_trailing_component(&mut components, "break_even_guard", break_even_score, 20.0);
    push_weighted_trailing_component(
        &mut components,
        "trail_tightness",
        trail_tightness_score,
        20.0,
    );
    let raw_score = components
        .iter()
        .map(|component| component.contribution)
        .sum();
    let clamped_score = clamp_i32(raw_score, 0, 100);
    let breakdown = TrailingPolicyBreakdown {
        raw_score,
        clamped_score,
        components,
    };

    activated = true;
    Some(TrailingEval {
        activated,
        peak_price,
        trail_pct,
        trailing_stop_price,
        trailing_score: breakdown.clamped_score,
        reason: format!(
            "trailing_eval_profit_{:.5}_peak_{:.5}_trail_{:.5}_stop_{:.5}_ratchet_{}_breakeven_{}_{}",
            profit_pct,
            peak_price,
            trail_pct,
            trailing_stop_price,
            ratchet_level,
            break_even_armed,
            trailing_policy_summary(&breakdown)
        ),
    })
}

pub(crate) fn should_persist_trailing_update(
    open: &OpenTradeSnapshot,
    eval: &TrailingEval,
    min_move_threshold_pct: f64,
) -> bool {
    if open.trailing_stop_activated != eval.activated {
        return true;
    }

    let prior_peak = open.trailing_stop_peak_price;
    let favorable_move = if open.side == "Long" {
        eval.peak_price - prior_peak
    } else {
        prior_peak - eval.peak_price
    };
    let peak_base = prior_peak.abs().max(1e-9);
    if favorable_move > 0.0 && favorable_move / peak_base >= min_move_threshold_pct {
        return true;
    }

    (eval.trail_pct - open.trailing_stop_final_distance_pct).abs() >= 1e-9
}

pub(crate) fn push_weighted_trailing_component(
    components: &mut Vec<TrailingPolicyComponent>,
    reason: &'static str,
    score: f64,
    weight: f64,
) {
    let contribution = weighted_contribution(score, weight);
    if contribution != 0 {
        components.push(TrailingPolicyComponent {
            reason,
            score,
            weight,
            contribution,
        });
    }
}

pub(crate) fn trailing_policy_summary(breakdown: &TrailingPolicyBreakdown) -> String {
    let mut components = breakdown.components.clone();
    components.sort_by_key(|component| component.contribution.abs());
    components.reverse();

    let reasons = components
        .into_iter()
        .take(4)
        .map(|component| {
            format!(
                "{}:{:.3}x{:.1}={}",
                component.reason, component.score, component.weight, component.contribution
            )
        })
        .collect::<Vec<_>>();

    if reasons.is_empty() {
        format!(
            "trailing_raw_{}_clamped_{}",
            breakdown.raw_score, breakdown.clamped_score
        )
    } else {
        format!(
            "trailing_raw_{}_clamped_{}_{}",
            breakdown.raw_score,
            breakdown.clamped_score,
            reasons.join("__")
        )
    }
}

pub(crate) fn evaluate_open_trade_exit(
    symbol: &str,
    current_price: f64,
    open: &OpenTradeSnapshot,
    cfg: &StrategyConfig,
    active_position_advice: Option<&ActivePositionAdviceSnapshot>,
) -> ExitEvaluation {
    if current_price <= 0.0 || open.entry_price <= 0.0 {
        return ExitEvaluation {
            decision: Some(create_hold_decision(symbol, "open_position_invalid_price")),
            trailing: None,
            trigger: "invalid_price".to_string(),
            reason: "open_position_invalid_price".to_string(),
        };
    }

    let side = open.side.as_str();
    let sl_pct = cfg.stop_loss_pct(symbol);
    let hard_stop = if side == "Long" {
        open.entry_price * (1.0 - sl_pct)
    } else {
        open.entry_price * (1.0 + sl_pct)
    };

    if (side == "Long" && current_price <= hard_stop)
        || (side == "Short" && current_price >= hard_stop)
    {
        let reason = format!(
            "stop_loss_triggered_hard_stop_{:.5}_current_{:.5}",
            hard_stop, current_price
        );
        return ExitEvaluation {
            decision: create_close_decision(symbol, side, open.quantity, current_price, &reason),
            trailing: None,
            trigger: "stop_loss".to_string(),
            reason,
        };
    }

    if cfg.fixed_take_profit_enabled() {
        let tp_pct = cfg.take_profit_pct(symbol);
        let fixed_take_profit = if side == "Long" {
            open.entry_price * (1.0 + tp_pct)
        } else {
            open.entry_price * (1.0 - tp_pct)
        };
        if (side == "Long" && current_price >= fixed_take_profit)
            || (side == "Short" && current_price <= fixed_take_profit)
        {
            let reason = format!(
                "take_profit_triggered_target_{:.5}_current_{:.5}",
                fixed_take_profit, current_price
            );
            return ExitEvaluation {
                decision: create_close_decision(
                    symbol,
                    side,
                    open.quantity,
                    current_price,
                    &reason,
                ),
                trailing: None,
                trigger: "take_profit".to_string(),
                reason,
            };
        }
    }

    let trailing_cfg = apply_active_position_advice_to_trailing(
        cfg.trailing_runtime_config(symbol),
        active_position_advice,
    );
    if let Some(eval) = evaluate_trailing(open, current_price, &trailing_cfg) {
        let trailing_hit = if side == "Long" {
            current_price <= eval.trailing_stop_price
        } else {
            current_price >= eval.trailing_stop_price
        };

        if trailing_hit {
            let reason = format!(
                "trailing_stop_triggered_score_{}_{}_current_{:.5}",
                eval.trailing_score, eval.reason, current_price
            );
            return ExitEvaluation {
                decision: create_close_decision(
                    symbol,
                    side,
                    open.quantity,
                    current_price,
                    &reason,
                ),
                trailing: Some(eval),
                trigger: "trailing_stop".to_string(),
                reason,
            };
        }

        let reason = format!(
            "trailing_monitoring_score_{}_{}_ai_{}",
            eval.trailing_score,
            eval.reason,
            active_position_advice
                .map(|item| item.action.as_str())
                .unwrap_or("hold")
        );
        return ExitEvaluation {
            decision: None,
            trailing: Some(eval),
            trigger: "trailing_monitoring".to_string(),
            reason,
        };
    }

    ExitEvaluation {
        decision: None,
        trailing: None,
        trigger: "no_exit".to_string(),
        reason: "open_position_monitoring".to_string(),
    }
}
