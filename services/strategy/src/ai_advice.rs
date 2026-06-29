use std::collections::HashMap;
use std::time::{Duration, Instant};

use viper_domain::StrategyDecision;

use crate::{
    apply_hold_block, ActivePositionAdviceSnapshot, AiAnalystAdviceSnapshot, EntryGuardState,
    ExecutionAdviceSnapshot,
};

pub(crate) fn apply_execution_advice_veto(
    symbol: &str,
    mut decision: StrategyDecision,
    entry_score: f64,
    rank_score: f64,
    advice: Option<&ExecutionAdviceSnapshot>,
) -> StrategyDecision {
    if !matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
        return decision;
    }

    let Some(advice) = advice else {
        return decision;
    };

    let intended_side = if decision.action == "ENTER_LONG" {
        "long"
    } else {
        "short"
    };

    let avoid_symbol = advice
        .avoid_symbols
        .iter()
        .any(|item| item.eq_ignore_ascii_case(symbol));
    let score_floor = match (advice.market_state.as_str(), advice.entry_action.as_str()) {
        ("defensive", "avoid_marginal_entries") => 88.0,
        ("observation_mode", "only_best_setups") => 90.0,
        (_, "allow_biased_entries") => 0.0,
        _ => 0.0,
    };
    let bias_mismatch = advice.preferred_side != "neutral"
        && !advice.preferred_side.eq_ignore_ascii_case(intended_side);
    let low_entry_score = score_floor > 0.0 && entry_score < score_floor;
    let severe_size_mode = matches!(
        advice.size_action.as_str(),
        "minimal_size_33" | "reduced_size_50" | "minimal_size"
    );
    let low_rank_score = if severe_size_mode {
        rank_score < 82.0
    } else {
        false
    };

    let veto_reason = if advice.market_state == "defensive" && avoid_symbol {
        Some(format!(
            "ai_veto_fragile_symbol_{}_{}_{}",
            symbol.to_lowercase(),
            advice.market_state,
            advice.entry_action
        ))
    } else if avoid_symbol && low_entry_score {
        Some(format!(
            "ai_veto_low_entry_score_{:.0}_floor_{:.0}_{}_{}",
            entry_score, score_floor, advice.market_state, advice.entry_action
        ))
    } else if advice.market_state == "defensive" && low_entry_score && bias_mismatch {
        Some(format!(
            "ai_veto_bias_mismatch_{}_preferred_{}_score_{:.0}",
            intended_side, advice.preferred_side, entry_score
        ))
    } else if advice.market_state == "observation_mode" && low_entry_score && low_rank_score {
        Some(format!(
            "ai_veto_low_rank_score_{:.0}_{}_{}",
            rank_score, advice.market_state, advice.size_action
        ))
    } else if avoid_symbol && bias_mismatch {
        Some(format!(
            "ai_veto_fragile_bias_mismatch_{}_preferred_{}_score_{:.0}",
            intended_side, advice.preferred_side, entry_score
        ))
    } else {
        None
    };

    if let Some(reason) = veto_reason {
        apply_hold_block(&mut decision, reason);
    }

    decision
}

pub(crate) fn execution_advice_size_multiplier(advice: &ExecutionAdviceSnapshot) -> f64 {
    match advice.size_action.as_str() {
        "full_size" | "normal_size" => 1.0,
        "reduced_size_75" | "reduced_size" => 0.75,
        "reduced_size_50" => 0.5,
        "minimal_size_33" => 0.33,
        "minimal_size" => 0.5,
        _ => 1.0,
    }
}

pub(crate) fn apply_execution_advice_sizing(
    mut decision: StrategyDecision,
    advice: Option<&ExecutionAdviceSnapshot>,
    entry_score: f64,
    rank_score: f64,
    min_position_usdt: f64,
) -> StrategyDecision {
    if !matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
        return decision;
    }

    let Some(advice) = advice else {
        return decision;
    };

    let multiplier = execution_advice_size_multiplier(advice);
    if multiplier >= 0.999 || decision.quantity <= 0.0 || decision.entry_price <= 0.0 {
        return decision;
    }

    let adjusted_quantity = decision.quantity * multiplier;
    let adjusted_notional = adjusted_quantity * decision.entry_price;
    if min_position_usdt > 0.0 && adjusted_notional < min_position_usdt {
        return decision;
    }

    decision.quantity = adjusted_quantity;
    decision.reason = format!(
        "{}_ai_size_{}pct_market_{}_rank_{:.0}_entry_{:.0}",
        decision.reason,
        (multiplier * 100.0).round() as i32,
        advice.market_state,
        rank_score,
        entry_score
    );
    decision
}

pub(crate) fn execution_advice_quarantine_minutes(advice: &ExecutionAdviceSnapshot) -> i64 {
    match advice.market_state.as_str() {
        "defensive" => 30,
        "selective" => 20,
        "observation_mode" => 15,
        _ => 0,
    }
}

pub(crate) fn sync_execution_advice_guards(
    entry_guards: &mut HashMap<String, EntryGuardState>,
    advice: Option<&ExecutionAdviceSnapshot>,
) {
    let Some(advice) = advice else {
        return;
    };

    let cooldown_minutes = execution_advice_quarantine_minutes(advice);
    if cooldown_minutes <= 0 {
        return;
    }

    let cooldown_until = Instant::now() + Duration::from_secs((cooldown_minutes * 60) as u64);
    for symbol in &advice.avoid_symbols {
        let should_insert = match entry_guards.get(symbol) {
            Some(existing) if Instant::now() < existing.cooldown_until => {
                existing.cooldown_reason == "ai_quarantine"
                    && existing.cooldown_until < cooldown_until
            }
            _ => true,
        };

        if should_insert {
            entry_guards.insert(
                symbol.to_uppercase(),
                EntryGuardState {
                    blocked_side: "Both".to_string(),
                    cooldown_until,
                    cooldown_minutes,
                    cooldown_reason: "ai_quarantine".to_string(),
                    awaiting_flip: false,
                },
            );
        }
    }
}

#[allow(dead_code)]
pub(crate) fn active_position_advice_for_symbol<'a>(
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
