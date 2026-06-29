use crate::*;
use serde_json::{json, Value};

#[allow(dead_code)]
pub(crate) fn execute_decision(state: &Value, cfg: &StrategyConfig) -> Result<Value, String> {
    let symbol = get_string(state, "symbol", "UNKNOWN");
    let can_enter = get_bool(state, "check_daily_loss", false)
        && get_bool(state, "check_consecutive_losses", false)
        && get_record_bool(state, "validate_entry", "passed", false)
        && get_record_bool(state, "check_funding", "passed", false)
        && get_record_bool(state, "validate_size", "passed", false);

    let entry_price = get_f64(state, "current_price", 0.0);
    let quantity = get_record_f64(state, "calc_smart_size", "quantity", 0.0);
    let entry_side = get_record_string(state, "validate_entry", "side", "long");
    let entry_reason = get_record_string(
        state,
        "validate_entry",
        "reason",
        "risk_constraints_not_met",
    );
    let entry_score = get_record_f64(state, "validate_entry", "entry_score", 0.0);
    let entry_breakdown_summary = crate::summarize_entry_breakdown(state);
    let entry_reason_with_breakdown = if let Some(ref summary) = entry_breakdown_summary {
        format!("{}_{}", entry_reason, summary)
    } else {
        entry_reason.clone()
    };
    let size_reason =
        get_record_string(state, "validate_size", "reason", "size_constraints_not_met");
    let size_score = get_record_f64(state, "validate_size", "size_score", 0.0);
    let size_proposal_reason = get_record_string(
        state,
        "calc_smart_size",
        "reason",
        "size_proposal_not_available",
    );
    let size_proposal_score = get_record_f64(state, "calc_smart_size", "proposal_score", 0.0);
    let funding_reason = get_record_string(
        state,
        "check_funding",
        "reason",
        "funding_constraints_not_met",
    );
    let funding_score = get_record_f64(state, "check_funding", "funding_score", 0.0);
    let trailing_reason = get_record_string(
        state,
        "get_trailing_config",
        "reason",
        "trailing_config_not_available",
    );
    let trailing_score = get_record_f64(state, "get_trailing_config", "trailing_score", 0.0);

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

    let decision_raw_score: i32 = components.iter().map(|c| c.contribution).sum();
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
            "symbol": symbol, "quantity": quantity, "leverage": cfg.max_leverage(),
            "entry_price": entry_price, "stop_loss": stop_loss, "take_profit": take_profit,
            "reason": format!("entry_confirmed_score_{:.3}_funding_{:.3}_proposal_{:.3}_size_{:.3}_trailing_{:.3}_{}_{}_{}_{}_{}_{}",
                entry_score, funding_score, size_proposal_score, size_score, trailing_score,
                entry_reason_with_breakdown, funding_reason, size_proposal_reason,
                size_reason, trailing_reason, decision_summary
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
            "action": "HOLD", "symbol": symbol, "quantity": 0.0, "leverage": 0.0,
            "entry_price": 0.0, "stop_loss": 0.0, "take_profit": 0.0,
            "reason": format!("{}_{}_{}_{}_{}_{}",
                entry_reason_with_breakdown, funding_reason, size_proposal_reason,
                size_reason, trailing_reason, decision_summary
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

pub(crate) fn step_decision(input: &StrategyInput) -> Value {
    if let Some(cfg) = real_cfg() {
        return run_steps_through(input, &cfg, "decision");
    }
    json!({
        "action": "HOLD", "symbol": input.symbol, "quantity": 0.0, "leverage": 0.0,
        "entry_price": 0.0, "stop_loss": 0.0, "take_profit": 0.0,
        "reason": "decision_pending_runtime", "smart_copy_compatible": false,
        "decision_score": 100.0,
        "decision_breakdown": { "raw_score": 100, "clamped_score": 100, "components": [] }
    })
}

pub(crate) fn push_weighted_decision_component(
    components: &mut Vec<DecisionPolicyComponent>,
    reason: &'static str,
    score: f64,
    weight: f64,
) {
    let contribution = weighted_contribution(score, weight);
    if contribution != 0 {
        components.push(DecisionPolicyComponent {
            reason,
            score,
            weight,
            contribution,
        });
    }
}

pub(crate) fn decision_policy_summary(breakdown: &DecisionPolicyBreakdown) -> String {
    let mut components = breakdown.components.clone();
    components.sort_by_key(|c| c.contribution.abs());
    components.reverse();
    let reasons: Vec<String> = components
        .into_iter()
        .take(5)
        .map(|c| {
            format!(
                "{}:{:.3}x{:.1}={}",
                c.reason, c.score, c.weight, c.contribution
            )
        })
        .collect();
    if reasons.is_empty() {
        format!(
            "decision_raw_{}_clamped_{}",
            breakdown.raw_score, breakdown.clamped_score
        )
    } else {
        format!(
            "decision_raw_{}_clamped_{}_{}",
            breakdown.raw_score,
            breakdown.clamped_score,
            reasons.join("__")
        )
    }
}
