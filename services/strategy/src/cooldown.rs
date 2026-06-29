use crate::StrategyInput;
use serde_json::{json, Value};

pub(crate) fn step_cooldown_guard(_input: &StrategyInput) -> Value {
    json!({ "blocked": false, "remaining_ticks": 0, "reason": "cooldown_clear" })
}
