use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use tupa_runtime::TupaExtension;

pub struct ViperExtensions;

#[cfg(test)]
mod tests {
    use super::*;
    use tupa_runtime::Runtime;

    #[test]
    fn test_viper_extension_name() {
        let ext = ViperExtensions;
        assert_eq!(ext.name(), "vipertrade");
    }

    #[test]
    fn test_viper_trailing_status_extension() {
        let runtime = Runtime::new();
        runtime.register_extension(Arc::new(ViperExtensions));

        // Test enabled case
        let result = runtime.call_step_function(
            "viper::trailing_status",
            json!({"enabled": true, "trail_pct": 0.5, "trailing_stop_price": 95.0})
        );
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output["enabled"], true);
        assert_eq!(output["trail_pct"], 0.5);
        assert_eq!(output["trailing_stop_price"], 95.0);
        assert_eq!(output["trailing_score"], 100.0);

        // Test disabled case
        let result = runtime.call_step_function(
            "viper::trailing_status",
            json!({"enabled": false, "trail_pct": 0.0, "trailing_stop_price": 0.0})
        );
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output["enabled"], false);
        assert_eq!(output["trailing_score"], 0.0);
    }

    #[test]
    fn test_viper_position_sizing_extension() {
        let runtime = Runtime::new();
        runtime.register_extension(Arc::new(ViperExtensions));

        let result = runtime.call_step_function(
            "viper::position_sizing",
            json!({"equity": 10000.0, "risk_pct": 0.02, "price": 100.0, "max_leverage": 5.0})
        );
        assert!(result.is_ok());
        let output = result.unwrap();
        // quantity = (10000 * 0.02 * 5.0) / 100 = 10.0
        assert!((output["quantity"].as_f64().unwrap() - 10.0).abs() < f64::EPSILON);
        assert!((output["desired_usdt"].as_f64().unwrap() - 1000.0).abs() < f64::EPSILON);
        assert!((output["risk_budget_usdt"].as_f64().unwrap() - 200.0).abs() < f64::EPSILON);
        assert_eq!(output["proposal_score"], 100.0);
        assert_eq!(output["reason"], "size_proposed");
    }
}

impl TupaExtension for ViperExtensions {
    fn name(&self) -> &'static str {
        "vipertrade"
    }

    fn register(&self, runtime: &tupa_runtime::Runtime) {
        runtime.register_step("viper::trailing_status", |input: Value| {
            let enabled = input.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let trail_pct = input.get("trail_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let trailing_stop_price = input.get("trailing_stop_price").and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(json!({
                "enabled": enabled,
                "trail_pct": trail_pct,
                "trailing_stop_price": trailing_stop_price,
                "trailing_score": if enabled { 100.0 } else { 0.0 }
            }))
        });

        runtime.register_step("viper::position_sizing", |input: Value| {
            let equity = input.get("equity").and_then(|v| v.as_f64()).unwrap_or(1000.0);
            let risk_pct = input.get("risk_pct").and_then(|v| v.as_f64()).unwrap_or(0.01);
            let price = input.get("price").and_then(|v| v.as_f64()).unwrap_or(100.0);
            let max_leverage = input.get("max_leverage").and_then(|v| v.as_f64()).unwrap_or(5.0);
            let quantity = (equity * risk_pct * max_leverage) / price;
            Ok(json!({
                "quantity": quantity,
                "desired_usdt": equity * risk_pct * max_leverage,
                "risk_budget_usdt": equity * risk_pct,
                "proposal_score": 100.0,
                "reason": "size_proposed"
            }))
        });
    }
}

pub fn get_extension() -> Arc<dyn TupaExtension> {
    Arc::new(ViperExtensions)
}