use serde_json::json;
use serde_json::Value;

#[allow(dead_code)]
pub fn trailing_status(input: Value) -> Value {
    let enabled = input
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let trail_pct = input
        .get("trail_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let trailing_stop_price = input
        .get("trailing_stop_price")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    json!({
        "enabled": enabled,
        "trail_pct": trail_pct,
        "trailing_stop_price": trailing_stop_price,
        "trailing_score": if enabled { 100.0 } else { 0.0 }
    })
}

#[allow(dead_code)]
pub fn position_sizing(input: Value) -> Value {
    let equity = input
        .get("equity")
        .and_then(|v| v.as_f64())
        .unwrap_or(1000.0);
    let risk_pct = input
        .get("risk_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.01);
    let price = input.get("price").and_then(|v| v.as_f64()).unwrap_or(100.0);
    let max_leverage = input
        .get("max_leverage")
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0);
    let quantity = (equity * risk_pct * max_leverage) / price;
    json!({
        "quantity": quantity,
        "desired_usdt": equity * risk_pct * max_leverage,
        "risk_budget_usdt": equity * risk_pct,
        "proposal_score": 100.0,
        "reason": "size_proposed"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trailing_status_extension() {
        let result = trailing_status(json!({"enabled": true, "trail_pct": 0.5, "trailing_stop_price": 95.0}));
        assert!(result.is_object());
        assert_eq!(result["enabled"], true);
        assert_eq!(result["trail_pct"], 0.5);
        assert_eq!(result["trailing_stop_price"], 95.0);
        assert_eq!(result["trailing_score"], 100.0);

        let result = trailing_status(json!({"enabled": false, "trail_pct": 0.0, "trailing_stop_price": 0.0}));
        assert_eq!(result["enabled"], false);
        assert_eq!(result["trailing_score"], 0.0);
    }

    #[test]
    fn test_position_sizing_extension() {
        let result = position_sizing(json!({"equity": 10000.0, "risk_pct": 0.02, "price": 100.0, "max_leverage": 5.0}));
        assert!((result["quantity"].as_f64().unwrap() - 10.0).abs() < f64::EPSILON);
        assert!((result["desired_usdt"].as_f64().unwrap() - 1000.0).abs() < f64::EPSILON);
        assert!((result["risk_budget_usdt"].as_f64().unwrap() - 200.0).abs() < f64::EPSILON);
        assert_eq!(result["proposal_score"], 100.0);
        assert_eq!(result["reason"], "size_proposed");
    }
}