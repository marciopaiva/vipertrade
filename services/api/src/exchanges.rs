//! Cross-exchange symbol availability. A token only works in the multi-exchange
//! consensus if it's tradeable on all three venues market-data polls (Bybit,
//! Binance, OKX), so adding a token is gated on this check.

use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize)]
pub struct SymbolAvailability {
    pub bybit: bool,
    pub binance: bool,
    pub okx: bool,
}

impl SymbolAvailability {
    pub fn available_on_all(&self) -> bool {
        self.bybit && self.binance && self.okx
    }

    /// Names of the venues the symbol is NOT listed on (for error messages).
    pub fn missing(&self) -> Vec<&'static str> {
        let mut m = Vec::new();
        if !self.bybit {
            m.push("bybit");
        }
        if !self.binance {
            m.push("binance");
        }
        if !self.okx {
            m.push("okx");
        }
        m
    }
}

/// OKX uses `BASE-USDT-SWAP` ids (mirrors market-data's okx_inst_id).
fn okx_inst_id(symbol: &str) -> String {
    let base = symbol.strip_suffix("USDT").unwrap_or(symbol);
    format!("{base}-USDT-SWAP")
}

fn bybit_base() -> String {
    std::env::var("BYBIT_REST_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "https://api.bybit.com".to_string())
}

async fn check_bybit(http: &Client, symbol: &str) -> bool {
    let url = format!(
        "{}/v5/market/instruments-info?category=linear&symbol={symbol}",
        bybit_base()
    );
    let Ok(resp) = http.get(&url).timeout(HTTP_TIMEOUT).send().await else {
        return false;
    };
    let Ok(v) = resp.json::<Value>().await else {
        return false;
    };
    v.get("retCode").and_then(Value::as_i64) == Some(0)
        && v.pointer("/result/list")
            .and_then(Value::as_array)
            .map(|a| !a.is_empty())
            .unwrap_or(false)
}

async fn check_binance(http: &Client, symbol: &str) -> bool {
    // exchangeInfo?symbol=X is NOT strict — an unknown symbol still returns 200 with
    // the full list, so confirm the symbol is actually present and TRADING.
    let url = format!("https://fapi.binance.com/fapi/v1/exchangeInfo?symbol={symbol}");
    let Ok(resp) = http.get(&url).timeout(HTTP_TIMEOUT).send().await else {
        return false;
    };
    let Ok(v) = resp.json::<Value>().await else {
        return false;
    };
    v.get("symbols")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter().any(|s| {
                s.get("symbol").and_then(Value::as_str) == Some(symbol)
                    && s.get("status").and_then(Value::as_str) == Some("TRADING")
            })
        })
        .unwrap_or(false)
}

async fn check_okx(http: &Client, symbol: &str) -> bool {
    let url = format!(
        "https://www.okx.com/api/v5/public/instruments?instType=SWAP&instId={}",
        okx_inst_id(symbol)
    );
    let Ok(resp) = http.get(&url).timeout(HTTP_TIMEOUT).send().await else {
        return false;
    };
    let Ok(v) = resp.json::<Value>().await else {
        return false;
    };
    v.get("code").and_then(Value::as_str) == Some("0")
        && v.get("data")
            .and_then(Value::as_array)
            .map(|a| !a.is_empty())
            .unwrap_or(false)
}

/// Check the symbol on all three venues concurrently.
pub async fn symbol_availability(http: &Client, symbol: &str) -> SymbolAvailability {
    let (bybit, binance, okx) = tokio::join!(
        check_bybit(http, symbol),
        check_binance(http, symbol),
        check_okx(http, symbol),
    );
    SymbolAvailability {
        bybit,
        binance,
        okx,
    }
}

/// Normalize + validate an operator-supplied symbol to a linear USDT perp id.
pub fn normalize_symbol(raw: &str) -> Option<String> {
    let up = raw.trim().to_uppercase();
    let ok = up.len() >= 5 && up.ends_with("USDT") && up.chars().all(|c| c.is_ascii_alphanumeric());
    ok.then_some(up)
}
