use crate::*;
use serde_json::{json, Value};

#[allow(dead_code)]
pub(crate) fn execute_audit(state: &Value) -> Result<Value, String> {
    let decision_action = get_record_string(state, "decision", "action", "UNKNOWN");
    let decision_reason =
        get_record_string(state, "decision", "reason", "audit_missing_decision_reason");
    let decision_score = get_record_f64(state, "decision", "decision_score", 0.0);
    let smart_copy_compatible = get_record_bool(state, "decision", "smart_copy_compatible", false);
    let temporal_reason = crate::filters::structured_hold_reason_from_state(state);
    Ok(json!({
        "ok": true,
        "reason": format!("audit_action_{}_score_{:.3}_smart_copy_{}_{}_{}",
            decision_action, decision_score, smart_copy_compatible, decision_reason, temporal_reason
        ),
        "decision_action": decision_action,
        "decision_score": decision_score,
        "smart_copy_compatible": smart_copy_compatible
    }))
}

pub(crate) fn step_audit(_input: &StrategyInput) -> Value {
    json!({
        "ok": true, "reason": "audit_ok", "decision_action": "HOLD",
        "decision_score": 100.0, "smart_copy_compatible": false
    })
}
