use crate::*;
use serde_json::{json, Value};

#[allow(dead_code)]
pub(crate) fn execute_check_funding(state: &Value, cfg: &StrategyConfig) -> Result<Value, String> {
    let funding_rate = get_f64(state, "funding_rate", 0.0).abs();
    let max_funding_rate_pct = cfg.max_funding_rate_pct();
    let funding_score = if max_funding_rate_pct > 0.0 {
        (1.0 - funding_rate / max_funding_rate_pct).clamp(-1.0, 1.0)
    } else {
        0.0
    };

    let mut components = Vec::new();
    push_weighted_funding_component(&mut components, "funding_rate_limit", funding_score, 100.0);

    let raw_score: i32 = components.iter().map(|c| c.contribution).sum();
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

pub(crate) fn step_check_funding(input: &StrategyInput) -> Value {
    if let Some(cfg) = real_cfg() {
        return run_steps_through(input, &cfg, "check_funding");
    }
    json!({
        "passed": true, "severity": "info", "reason": "funding_validated",
        "funding_rate": 0.0, "funding_score": 100.0,
        "funding_breakdown": { "raw_score": 100, "clamped_score": 100, "components": [] }
    })
}

pub(crate) fn push_weighted_funding_component(
    components: &mut Vec<FundingPolicyComponent>,
    reason: &'static str,
    score: f64,
    weight: f64,
) {
    let contribution = weighted_contribution(score, weight);
    if contribution != 0 {
        components.push(FundingPolicyComponent {
            reason,
            score,
            weight,
            contribution,
        });
    }
}

pub(crate) fn funding_policy_summary(breakdown: &FundingPolicyBreakdown) -> String {
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
            "funding_raw_{}_clamped_{}",
            breakdown.raw_score, breakdown.clamped_score
        )
    } else {
        format!(
            "funding_raw_{}_clamped_{}_{}",
            breakdown.raw_score,
            breakdown.clamped_score,
            reasons.join("__")
        )
    }
}
