use crate::StrategyInput;
use serde_json::{json, Value};

pub(crate) fn step_equity_floor(input: &StrategyInput) -> Value {
    json!(input.account_equity_usdt)
}
