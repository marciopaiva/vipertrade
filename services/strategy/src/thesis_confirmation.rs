use crate::StrategyInput;
use serde_json::{json, Value};

pub(crate) fn step_thesis_confirmation(_input: &StrategyInput) -> Value {
    json!({ "passed": true, "pending": false, "remaining_hits": 0, "reason": "thesis_clear" })
}
