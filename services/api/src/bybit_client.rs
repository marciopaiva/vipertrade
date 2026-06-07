use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::Sha256;

pub struct BybitClient {
    pub api_key: String,
    api_secret: String,
    recv_window: String,
    pub base_url: String,
}

impl BybitClient {
    pub fn from_env() -> Self {
        let bybit_url = Self::resolve_bybit_rest_url();
        let recv_window = std::env::var("BYBIT_RECV_WINDOW")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "5000".to_string());
        let api_key = std::env::var("BYBIT_API_KEY").unwrap_or_default();
        let api_secret = std::env::var("BYBIT_API_SECRET").unwrap_or_default();
        Self {
            api_key,
            api_secret,
            recv_window,
            base_url: bybit_url,
        }
    }

    fn resolve_bybit_rest_url() -> String {
        std::env::var("BYBIT_REST_URL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "https://api.bybit.com".to_string())
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.api_secret.is_empty()
    }

    fn compute_signature(&self, timestamp: &str, query_string: &str) -> Result<String, String> {
        let payload = format!(
            "{}{}{}{}",
            timestamp, self.api_key, self.recv_window, query_string
        );
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret.as_bytes())
            .map_err(|e| format!("failed to initialize signature: {}", e))?;
        mac.update(payload.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    async fn get_with_auth(&self, endpoint: &str, query_string: &str) -> BybitResponse {
        if !self.is_configured() {
            return BybitResponse {
                status: 0,
                latency_ms: 0,
                ret_code: None,
                ret_msg: None,
                body: Value::Object(Default::default()),
                error: Some("missing BYBIT_API_KEY or BYBIT_API_SECRET in api runtime".to_string()),
            };
        }

        let timestamp = Utc::now().timestamp_millis().to_string();
        let sign = match self.compute_signature(&timestamp, query_string) {
            Ok(s) => s,
            Err(e) => {
                return BybitResponse {
                    status: 0,
                    latency_ms: 0,
                    ret_code: None,
                    ret_msg: None,
                    body: Value::Object(Default::default()),
                    error: Some(e),
                };
            }
        };

        let url = format!("{}{}?{}", self.base_url, endpoint, query_string);
        let client = match Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return BybitResponse {
                    status: 0,
                    latency_ms: 0,
                    ret_code: None,
                    ret_msg: None,
                    body: Value::Object(Default::default()),
                    error: Some(format!("failed to build http client: {}", e)),
                };
            }
        };

        let started = std::time::Instant::now();
        match client
            .get(&url)
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-SIGN", sign)
            .header("X-BAPI-SIGN-TYPE", "2")
            .header("X-BAPI-TIMESTAMP", timestamp)
            .header("X-BAPI-RECV-WINDOW", &self.recv_window)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let parsed = resp
                    .json::<Value>()
                    .await
                    .unwrap_or_else(|_| Value::Object(Default::default()));
                BybitResponse {
                    status,
                    latency_ms: started.elapsed().as_millis() as i64,
                    ret_code: parsed.get("retCode").and_then(|v| v.as_i64()),
                    ret_msg: parsed
                        .get("retMsg")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    body: parsed,
                    error: None,
                }
            }
            Err(e) => BybitResponse {
                status: 0,
                latency_ms: started.elapsed().as_millis() as i64,
                ret_code: None,
                ret_msg: None,
                body: Value::Object(Default::default()),
                error: Some(format!("request failed: {}", e)),
            },
        }
    }

    pub async fn wallet_balance(&self, account_type: &str) -> BybitResponse {
        let query = format!("accountType={}", account_type);
        self.get_with_auth("/v5/account/wallet-balance", &query)
            .await
    }

    pub async fn order_history(
        &self,
        category: &str,
        settle_coin: &str,
        start_time: i64,
        end_time: i64,
        limit: usize,
    ) -> BybitResponse {
        let query = format!(
            "category={}&settleCoin={}&startTime={}&endTime={}&limit={}",
            category,
            settle_coin,
            start_time,
            end_time,
            limit.clamp(1, 50)
        );
        self.get_with_auth("/v5/order/history", &query).await
    }

    pub async fn closed_pnl(
        &self,
        category: &str,
        settle_coin: &str,
        start_time: i64,
        end_time: i64,
        limit: usize,
        cursor: Option<&str>,
    ) -> BybitResponse {
        let mut query = format!(
            "category={}&settleCoin={}&startTime={}&endTime={}&limit={}",
            category,
            settle_coin,
            start_time,
            end_time,
            limit.clamp(1, 100)
        );
        if let Some(c) = cursor.filter(|v| !v.is_empty()) {
            query.push_str("&cursor=");
            query.push_str(c);
        }
        self.get_with_auth("/v5/position/closed-pnl", &query).await
    }

    pub async fn position_list(
        &self,
        category: &str,
        settle_coin: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> BybitResponse {
        let mut query = format!(
            "category={}&settleCoin={}&limit={}",
            category,
            settle_coin,
            limit.clamp(1, 200)
        );
        if let Some(c) = cursor.filter(|v| !v.is_empty()) {
            query.push_str("&cursor=");
            query.push_str(c);
        }
        self.get_with_auth("/v5/position/list", &query).await
    }
}

pub struct BybitResponse {
    pub status: u16,
    pub latency_ms: i64,
    pub ret_code: Option<i64>,
    pub ret_msg: Option<String>,
    pub body: Value,
    pub error: Option<String>,
}
