use crate::*;
use serde_json::{json, Value};

#[allow(dead_code)]
pub(crate) fn execute_get_trailing_config(
    state: &Value,
    cfg: &StrategyConfig,
) -> Result<Value, String> {
    let symbol = get_string(state, "symbol", "UNKNOWN");
    let trailing_cfg = cfg.trailing_runtime_config(&symbol);
    let enabled_score: f64 = if trailing_cfg.enabled { 1.0 } else { 0.0 };
    let activation_score = if trailing_cfg.activate_after_profit_pct > 0.0 {
        (1.0 - trailing_cfg.activate_after_profit_pct / 0.05).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let distance_score = if trailing_cfg.initial_trail_pct > 0.0 {
        (1.0 - trailing_cfg.initial_trail_pct / 0.03).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let trailing_score = clamp_i32(
        (enabled_score * 40.0).round() as i32
            + (activation_score * 30.0).round() as i32
            + (distance_score * 30.0).round() as i32,
        0,
        100,
    );

    Ok(json!({
        "enabled": trailing_cfg.enabled,
        "activate_after_profit_pct": trailing_cfg.activate_after_profit_pct,
        "initial_trail_pct": trailing_cfg.initial_trail_pct,
        "move_to_break_even_at": trailing_cfg.move_to_break_even_at,
        "min_move_threshold_pct": trailing_cfg.min_move_threshold_pct,
        "ratchet_level_count": trailing_cfg.ratchet_levels.len(),
        "trailing_score": trailing_score,
        "reason": format!("trailing_configured_enabled_{}_activate_{:.4}_trail_{:.4}_ratchets_{}",
            trailing_cfg.enabled, trailing_cfg.activate_after_profit_pct,
            trailing_cfg.initial_trail_pct, trailing_cfg.ratchet_levels.len()
        )
    }))
}

pub(crate) fn step_get_trailing_config(input: &StrategyInput) -> Value {
    if let Some(cfg) = real_cfg() {
        return run_steps_through(input, &cfg, "get_trailing_config");
    }
    json!({
        "enabled": false, "activate_after_profit_pct": 0.0, "initial_trail_pct": 0.0,
        "move_to_break_even_at": 0.0, "min_move_threshold_pct": 0.0,
        "ratchet_level_count": 0, "trailing_score": 0.0, "reason": "trailing_pending_runtime"
    })
}
