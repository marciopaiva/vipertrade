use crate::*;
use serde_json::json;

#[allow(dead_code)]
pub(crate) fn execute_check_daily_loss(
    state: &Value,
    cfg: &StrategyConfig,
) -> Result<Value, String> {
    let current_daily_loss = get_f64(state, "current_daily_loss", 0.0);
    Ok(json!(current_daily_loss <= cfg.max_daily_loss_pct()))
}

pub(crate) fn step_check_daily_loss(input: &StrategyInput) -> Value {
    if let Some(cfg) = real_cfg() {
        return run_steps_through(input, &cfg, "check_daily_loss");
    }
    json!({ "passed": true, "severity": "info", "reason": "daily_loss_valid", "symbol": input.symbol })
}

pub(crate) fn step_current_daily_loss(input: &StrategyInput) -> Value {
    json!(input
        .temporal
        .get("current_daily_loss")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0))
}
