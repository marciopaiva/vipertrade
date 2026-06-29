use crate::*;
use serde_json::json;

#[allow(dead_code)]
pub(crate) fn execute_check_consecutive_losses(
    state: &Value,
    cfg: &StrategyConfig,
) -> Result<Value, String> {
    let losses = get_i64(state, "consecutive_losses", 0);
    Ok(json!(losses <= cfg.max_consecutive_losses()))
}

pub(crate) fn step_check_consecutive_losses(input: &StrategyInput) -> Value {
    if let Some(cfg) = real_cfg() {
        return run_steps_through(input, &cfg, "check_consecutive_losses");
    }
    json!({ "passed": true, "severity": "info", "reason": "consecutive_losses_valid", "symbol": input.symbol })
}

pub(crate) fn step_consecutive_losses(input: &StrategyInput) -> Value {
    json!(input
        .temporal
        .get("consecutive_losses")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0))
}
