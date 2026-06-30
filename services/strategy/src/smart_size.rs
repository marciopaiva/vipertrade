use crate::*;
use serde_json::{json, Value};

#[allow(dead_code)]
pub(crate) fn execute_calc_smart_size(
    state: &Value,
    cfg: &StrategyConfig,
) -> Result<Value, String> {
    let symbol = get_string(state, "symbol", "UNKNOWN");
    let price = get_f64(state, "current_price", 0.0);
    if price <= 0.0 {
        return Ok(json!({
            "quantity": 0.0, "desired_usdt": 0.0, "risk_budget_usdt": 0.0,
            "volatility_discount": 0.0, "proposal_score": -100,
            "reason": "size_proposal_invalid_price",
            "proposal_breakdown": { "raw_score": -100, "clamped_score": -100, "components": [] }
        }));
    }
    let equity_usdt = get_f64(state, "account_equity_usdt", 1_000.0);
    let atr_14 = get_f64(state, "atr_14", 0.0);
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
    let sw = |key: &str, def: f64| cfg.size_weight(key, def);
    push_weighted_size_proposal_component(
        &mut components,
        "volatility_discount",
        discount_score,
        sw("volatility_discount", 40.0),
    );
    push_weighted_size_proposal_component(
        &mut components,
        "risk_budget",
        budget_score,
        sw("risk_budget", 30.0),
    );
    push_weighted_size_proposal_component(
        &mut components,
        "position_cap_fit",
        cap_score,
        sw("position_cap_fit", 30.0),
    );

    let raw_score: i32 = components.iter().map(|c| c.contribution).sum();
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

pub(crate) fn step_calc_smart_size(input: &StrategyInput) -> Value {
    if let Some(cfg) = real_cfg() {
        return run_steps_through(input, &cfg, "calc_smart_size");
    }
    json!({
        "quantity": 0.0, "desired_usdt": 0.0, "risk_budget_usdt": 0.0,
        "volatility_discount": 1.0, "proposal_score": 100.0, "reason": "size_proposed",
        "proposal_breakdown": { "raw_score": 100, "clamped_score": 100, "components": [] }
    })
}

pub(crate) fn push_weighted_size_proposal_component(
    components: &mut Vec<SizeProposalComponent>,
    reason: &'static str,
    score: f64,
    weight: f64,
) {
    let contribution = weighted_contribution(score, weight);
    if contribution != 0 {
        components.push(SizeProposalComponent {
            reason,
            score,
            weight,
            contribution,
        });
    }
}

pub(crate) fn size_proposal_summary(breakdown: &SizeProposalBreakdown) -> String {
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
            "proposal_raw_{}_clamped_{}",
            breakdown.raw_score, breakdown.clamped_score
        )
    } else {
        format!(
            "proposal_raw_{}_clamped_{}_{}",
            breakdown.raw_score,
            breakdown.clamped_score,
            reasons.join("__")
        )
    }
}
