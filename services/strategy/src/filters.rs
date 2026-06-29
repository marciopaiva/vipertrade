use std::collections::HashMap;
use std::time::Instant;

use viper_domain::StrategyDecision;

use serde_json::{json, Value};

use crate::config::StrategyConfig;
use crate::helpers::*;
use crate::{
    apply_hold_block, is_same_direction, EntryGuardEvaluation, EntryGuardState,
    SignalConfirmationState, ThesisInvalidationState,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_entry_guard_policy(
    symbol: &str,
    trend: f64,
    proposed_side: &str,
    proposed_reason: &str,
    entry_guards: &mut HashMap<String, EntryGuardState>,
    cooldown_minutes: i64,
    recent_stop_loss_same_symbol: bool,
    signal_confirmations: &mut HashMap<String, SignalConfirmationState>,
    min_confirmation_ticks: usize,
) -> EntryGuardEvaluation {
    if recent_stop_loss_same_symbol {
        return EntryGuardEvaluation {
            blocked: true,
            reason: format!(
                "cooldown_stop_loss_{}m_{}",
                cooldown_minutes, proposed_reason
            ),
        };
    }

    let confirmation = signal_confirmations
        .entry(symbol.to_string())
        .or_insert_with(|| SignalConfirmationState {
            side: proposed_side.to_string(),
            consecutive_valid_ticks: 0,
        });

    if !confirmation.side.eq_ignore_ascii_case(proposed_side) {
        confirmation.side = proposed_side.to_string();
        confirmation.consecutive_valid_ticks = 1;
    } else {
        confirmation.consecutive_valid_ticks += 1;
    }

    if confirmation.consecutive_valid_ticks < min_confirmation_ticks {
        return EntryGuardEvaluation {
            blocked: true,
            reason: format!(
                "awaiting_signal_confirmation_{}/{}_{}",
                confirmation.consecutive_valid_ticks, min_confirmation_ticks, proposed_reason
            ),
        };
    }

    if let Some(guard) = entry_guards.get_mut(symbol) {
        if Instant::now() < guard.cooldown_until {
            return EntryGuardEvaluation {
                blocked: true,
                reason: format!(
                    "cooldown_{}_{}m_{}",
                    guard.cooldown_reason, guard.cooldown_minutes, proposed_reason
                ),
            };
        }

        if !guard.awaiting_flip {
            return EntryGuardEvaluation {
                blocked: false,
                reason: proposed_reason.to_string(),
            };
        }

        if !is_same_direction(&guard.blocked_side, trend) {
            guard.awaiting_flip = false;
            return EntryGuardEvaluation {
                blocked: false,
                reason: proposed_reason.to_string(),
            };
        }

        if guard.blocked_side.eq_ignore_ascii_case(proposed_side) {
            return EntryGuardEvaluation {
                blocked: true,
                reason: format!("blocked_until_trend_flip_{}", proposed_reason),
            };
        }
    }

    EntryGuardEvaluation {
        blocked: false,
        reason: proposed_reason.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn enforce_entry_guards(
    symbol: &str,
    trend: f64,
    mut decision: StrategyDecision,
    entry_guards: &mut HashMap<String, EntryGuardState>,
    cooldown_minutes: i64,
    recent_stop_loss_same_symbol: bool,
    signal_confirmations: &mut HashMap<String, SignalConfirmationState>,
    min_confirmation_ticks: usize,
) -> StrategyDecision {
    if !matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
        return decision;
    }

    let proposed_reason = decision.reason.clone();
    let proposed_side = if decision.action == "ENTER_LONG" {
        "Long"
    } else {
        "Short"
    };

    let evaluation = evaluate_entry_guard_policy(
        symbol,
        trend,
        proposed_side,
        &proposed_reason,
        entry_guards,
        cooldown_minutes,
        recent_stop_loss_same_symbol,
        signal_confirmations,
        min_confirmation_ticks,
    );

    if evaluation.blocked {
        apply_hold_block(&mut decision, evaluation.reason);
    }

    decision
}

pub(crate) fn structured_hold_reason_from_state(state: &Value) -> String {
    let candidate_reasons = [
        get_record_string(
            state,
            "validate_entry",
            "reason",
            "risk_constraints_not_met",
        ),
        get_record_string(
            state,
            "check_funding",
            "reason",
            "funding_constraints_not_met",
        ),
        get_record_string(
            state,
            "calc_smart_size",
            "reason",
            "size_proposal_not_available",
        ),
        get_record_string(state, "validate_size", "reason", "size_constraints_not_met"),
        get_record_string(
            state,
            "get_trailing_config",
            "reason",
            "trailing_config_not_available",
        ),
        get_record_string(
            state,
            "signal_confirmation",
            "reason",
            "signal_confirmation_not_available",
        ),
        get_record_string(
            state,
            "cooldown_guard",
            "reason",
            "cooldown_guard_not_available",
        ),
        get_record_string(
            state,
            "thesis_confirmation",
            "reason",
            "thesis_confirmation_not_available",
        ),
    ];

    let reasons = candidate_reasons
        .into_iter()
        .filter(|reason| {
            !matches!(
                reason.as_str(),
                "risk_constraints_not_met"
                    | "funding_constraints_not_met"
                    | "size_proposal_not_available"
                    | "size_constraints_not_met"
                    | "trailing_config_not_available"
                    | "signal_confirmation_not_available"
                    | "cooldown_guard_not_available"
                    | "thesis_confirmation_not_available"
            )
        })
        .collect::<Vec<_>>();

    if reasons.is_empty() {
        "risk_constraints_not_met".to_string()
    } else {
        reasons.join("_")
    }
}

pub(crate) fn structured_temporal_reason_from_state(state: &Value) -> Option<String> {
    let candidate_reasons = [
        get_record_string(
            state,
            "signal_confirmation",
            "reason",
            "signal_confirmation_not_available",
        ),
        get_record_string(
            state,
            "cooldown_guard",
            "reason",
            "cooldown_guard_not_available",
        ),
        get_record_string(
            state,
            "thesis_confirmation",
            "reason",
            "thesis_confirmation_not_available",
        ),
    ];

    let reasons = candidate_reasons
        .into_iter()
        .filter(|reason| {
            !matches!(
                reason.as_str(),
                "signal_confirmation_not_available"
                    | "cooldown_guard_not_available"
                    | "thesis_confirmation_not_available"
            )
        })
        .collect::<Vec<_>>();

    if reasons.is_empty() {
        None
    } else {
        Some(reasons.join("_"))
    }
}

pub(crate) fn inferred_confirmation_side(
    trend: f64,
    signal_confirmation: Option<&SignalConfirmationState>,
) -> &'static str {
    if let Some(state) = signal_confirmation {
        if state.side.eq_ignore_ascii_case("short") {
            "short"
        } else {
            "long"
        }
    } else if trend < 0.0 {
        "short"
    } else {
        "long"
    }
}

pub(crate) fn build_temporal_pipeline_state(
    symbol: &str,
    trend: f64,
    cfg: &StrategyConfig,
    entry_guards: &HashMap<String, EntryGuardState>,
    signal_confirmations: &HashMap<String, SignalConfirmationState>,
    thesis_invalidations: &HashMap<String, ThesisInvalidationState>,
) -> serde_json::Value {
    let signal_confirmation = signal_confirmations.get(symbol);
    let thesis_confirmation = thesis_invalidations.get(symbol);
    let cooldown_guard = entry_guards.get(symbol);
    let confirmation_side = inferred_confirmation_side(trend, signal_confirmation);

    let cooldown_active = cooldown_guard
        .map(|guard| Instant::now() < guard.cooldown_until)
        .unwrap_or(false);
    let cooldown_remaining_seconds = if cooldown_active {
        cooldown_guard
            .map(|guard| {
                guard
                    .cooldown_until
                    .saturating_duration_since(Instant::now())
                    .as_secs()
            })
            .unwrap_or(0) as i64
    } else {
        0
    };

    json!({
        "signal_confirmation": {
            "observed": signal_confirmation
                .map(|state| state.consecutive_valid_ticks > 0)
                .unwrap_or(false),
            "consecutive_hits": signal_confirmation
            .map(|state| state.consecutive_valid_ticks as i64)
            .unwrap_or(0),
            "required_hits": cfg.min_signal_confirmation_ticks_for_side(symbol, confirmation_side) as i64
        },
        "cooldown_guard": {
            "active": cooldown_active,
            "remaining_seconds": cooldown_remaining_seconds
        },
        "thesis_confirmation": {
            "observed": thesis_confirmation
                .map(|state| state.consecutive_invalid_ticks > 0)
                .unwrap_or(false),
            "consecutive_hits": thesis_confirmation
                .map(|state| state.consecutive_invalid_ticks as i64)
                .unwrap_or(0),
            "required_hits": cfg.thesis_invalidation_confirmation_ticks(symbol) as i64
        },
        "thesis_degrading": {
            "observed": thesis_confirmation
                .map(|state| state.consecutive_degrading_ticks > 0)
                .unwrap_or(false),
            "consecutive_hits": thesis_confirmation
                .map(|state| state.consecutive_degrading_ticks as i64)
                .unwrap_or(0),
            "required_hits": cfg.thesis_degrading_confirmation_ticks(symbol) as i64
        },
        "thesis_invalidation": {
            "observed": thesis_confirmation
                .map(|state| state.bollinger_invalidated)
                .unwrap_or(false),
            "consecutive_hits": thesis_confirmation
                .map(|state| state.bollinger_consecutive_hits as i64)
                .unwrap_or(0),
            "required_hits": cfg.thesis_invalidation_confirmation_ticks(symbol) as i64
        }
    })
}
