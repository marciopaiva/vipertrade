use crate::types::{AiAnalystAdviceSnapshot, WalletSizingResponse};

pub(crate) fn resolve_wallet_api_base_url() -> String {
    std::env::var("WALLET_API_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://api:8080".to_string())
}

pub(crate) fn resolve_ai_analyst_base_url() -> String {
    std::env::var("AI_ANALYST_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://ai-analyst:8087".to_string())
}

pub(crate) async fn fetch_account_equity_usdt(
    http: &reqwest::Client,
    wallet_api_base_url: &str,
    fallback_equity_usdt: f64,
) -> f64 {
    let url = format!(
        "{}/api/v1/external/bybit-wallet",
        wallet_api_base_url.trim_end_matches('/')
    );
    match http.get(url).send().await {
        Ok(response) => match response.json::<WalletSizingResponse>().await {
            Ok(body) => body
                .total_equity
                .or(body.margin_balance)
                .or(body.wallet_balance)
                .or(body.available_balance)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(fallback_equity_usdt),
            Err(_) => fallback_equity_usdt,
        },
        Err(_) => fallback_equity_usdt,
    }
}

pub(crate) async fn fetch_execution_advice(
    http: &reqwest::Client,
    ai_analyst_base_url: &str,
    hours: i64,
) -> Option<AiAnalystAdviceSnapshot> {
    let url = format!(
        "{}/analyze/recent?hours={}",
        ai_analyst_base_url.trim_end_matches('/'),
        hours
    );
    match http.get(url).send().await {
        Ok(response) => match response.json::<AiAnalystAdviceSnapshot>().await {
            Ok(body) => Some(body),
            Err(_) => None,
        },
        Err(_) => None,
    }
}
