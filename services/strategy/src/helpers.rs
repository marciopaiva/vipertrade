use serde_json::Value;
use viper_domain::StrategyDecision;

use crate::HealthScoreComponent;

pub(crate) fn cfg_get<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = value;
    for part in path {
        cur = cur.get(*part)?;
    }
    Some(cur)
}

pub(crate) fn cfg_f64(value: &Value, path: &[&str], default: f64) -> f64 {
    cfg_get(value, path)
        .and_then(Value::as_f64)
        .unwrap_or(default)
}

pub(crate) fn cfg_i64(value: &Value, path: &[&str], default: i64) -> i64 {
    cfg_get(value, path)
        .and_then(Value::as_i64)
        .unwrap_or(default)
}

pub(crate) fn cfg_bool(value: &Value, path: &[&str], default: bool) -> bool {
    cfg_get(value, path)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

pub(crate) fn get_f64(state: &Value, key: &str, default: f64) -> f64 {
    state.get(key).and_then(Value::as_f64).unwrap_or(default)
}

pub(crate) fn get_i64(state: &Value, key: &str, default: i64) -> i64 {
    state.get(key).and_then(Value::as_i64).unwrap_or(default)
}

pub(crate) fn get_bool(state: &Value, key: &str, default: bool) -> bool {
    state.get(key).and_then(Value::as_bool).unwrap_or(default)
}

pub(crate) fn get_string(state: &Value, key: &str, default: &str) -> String {
    state
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

pub(crate) fn get_record_field<'a>(state: &'a Value, key: &str, field: &str) -> Option<&'a Value> {
    state.get(key).and_then(|value| value.get(field))
}

pub(crate) fn get_record_bool(state: &Value, key: &str, field: &str, default: bool) -> bool {
    get_record_field(state, key, field)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

pub(crate) fn get_record_f64(state: &Value, key: &str, field: &str, default: f64) -> f64 {
    get_record_field(state, key, field)
        .and_then(Value::as_f64)
        .unwrap_or(default)
}

pub(crate) fn get_record_string(state: &Value, key: &str, field: &str, default: &str) -> String {
    get_record_field(state, key, field)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

pub(crate) fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
}

pub(crate) fn weighted_contribution(score: f64, weight: f64) -> i32 {
    (score * weight).round() as i32
}

pub(crate) fn directional_points(
    state: &str,
    favorable: &str,
    unfavorable: &str,
    weight: i32,
) -> i32 {
    if state.eq_ignore_ascii_case(favorable) {
        weight
    } else if state.eq_ignore_ascii_case(unfavorable) {
        -weight
    } else {
        0
    }
}

pub(crate) fn temporal_confirmation_reason(
    prefix: &str,
    consecutive_hits: usize,
    required_hits: usize,
    base_reason: &str,
) -> String {
    let remaining_hits = required_hits.saturating_sub(consecutive_hits);
    format!(
        "{}_pending_{}_remaining_{}_{}",
        prefix, consecutive_hits, remaining_hits, base_reason
    )
}

pub(crate) fn side_from_trend(trend: f64) -> &'static str {
    if trend >= 0.0 {
        "Long"
    } else {
        "Short"
    }
}

pub(crate) fn is_same_direction(side: &str, trend: f64) -> bool {
    side.eq_ignore_ascii_case(side_from_trend(trend))
}

pub(crate) fn summarize_entry_breakdown(state: &Value) -> Option<String> {
    let breakdown = get_record_field(state, "validate_entry", "entry_breakdown")?;
    let raw_score = breakdown
        .get("raw_score")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let clamped_score = breakdown
        .get("clamped_score")
        .and_then(Value::as_i64)
        .unwrap_or(raw_score);

    let mut components = breakdown
        .get("components")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    components.sort_by_key(|component| {
        component
            .get("contribution")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .abs()
    });
    components.reverse();

    let reasons = components
        .into_iter()
        .take(3)
        .map(|component| {
            let reason = component
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let score = component
                .get("score")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let weight = component
                .get("weight")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let contribution = component
                .get("contribution")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            format!("{reason}:{score:.3}x{weight:.1}={contribution}")
        })
        .collect::<Vec<_>>();

    Some(if reasons.is_empty() {
        format!("entry_raw_{raw_score}_clamped_{clamped_score}")
    } else {
        format!(
            "entry_raw_{raw_score}_clamped_{clamped_score}_{}",
            reasons.join("__")
        )
    })
}

pub(crate) fn create_hold_decision(symbol: &str, reason: &str) -> StrategyDecision {
    StrategyDecision {
        action: "HOLD".to_string(),
        symbol: symbol.to_string(),
        quantity: 0.0,
        leverage: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        reason: reason.to_string(),
        smart_copy_compatible: false,
    }
}

pub(crate) fn apply_hold_block(decision: &mut StrategyDecision, reason: String) {
    decision.action = "HOLD".to_string();
    decision.quantity = 0.0;
    decision.leverage = 0.0;
    decision.entry_price = 0.0;
    decision.stop_loss = 0.0;
    decision.take_profit = 0.0;
    decision.reason = reason;
    decision.smart_copy_compatible = false;
}

pub(crate) fn push_weighted_health_component(
    components: &mut Vec<HealthScoreComponent>,
    reason: &'static str,
    score: f64,
    weight: f64,
) {
    let contribution = weighted_contribution(score, weight);
    if contribution != 0 {
        components.push(HealthScoreComponent {
            reason,
            score,
            weight,
            contribution,
        });
    }
}
