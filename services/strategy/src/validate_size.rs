use crate::*;
use serde_json::{json, Value};

#[allow(dead_code)]
pub(crate) fn execute_validate_size(state: &Value, cfg: &StrategyConfig) -> Result<Value, String> {
    let symbol = get_string(state, "symbol", "UNKNOWN");
    let quantity = get_record_f64(state, "calc_smart_size", "quantity", 0.0);
    let price = get_f64(state, "current_price", 0.0);
    let equity_usdt = get_f64(state, "account_equity_usdt", 1_000.0);
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

    let raw_score: i32 = components.iter().map(|c| c.contribution).sum();
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

pub(crate) fn step_validate_size(input: &StrategyInput) -> Value {
    if let Some(cfg) = real_cfg() {
        return run_steps_through(input, &cfg, "validate_size");
    }
    json!({
        "passed": true, "severity": "info", "reason": "size_validated",
        "position_usdt": 0.0, "size_score": 100.0,
        "size_breakdown": { "raw_score": 100, "clamped_score": 100, "components": [] }
    })
}

pub(crate) fn push_weighted_size_component(
    components: &mut Vec<SizePolicyComponent>,
    reason: &'static str,
    score: f64,
    weight: f64,
) {
    let contribution = weighted_contribution(score, weight);
    if contribution != 0 {
        components.push(SizePolicyComponent {
            reason,
            score,
            weight,
            contribution,
        });
    }
}

pub(crate) fn size_policy_summary(breakdown: &SizePolicyBreakdown) -> String {
    let mut components = breakdown.components.clone();
    components.sort_by_key(|c| c.contribution.abs());
    components.reverse();
    let reasons: Vec<String> = components
        .into_iter()
        .take(3)
        .map(|c| {
            format!(
                "{}:{:.3}x{:.1}={}",
                c.reason, c.score, c.weight, c.contribution
            )
        })
        .collect();
    if reasons.is_empty() {
        format!(
            "size_raw_{}_clamped_{}",
            breakdown.raw_score, breakdown.clamped_score
        )
    } else {
        format!(
            "size_raw_{}_clamped_{}_{}",
            breakdown.raw_score,
            breakdown.clamped_score,
            reasons.join("__")
        )
    }
}
