use hmac::{Hmac, Mac};
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use sha2::Sha256;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

use crate::*;

pub(crate) fn bybit_sign(secret: &str, payload: &str) -> Result<String, Box<dyn Error>> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(payload.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

pub(crate) fn now_ms() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    ms.to_string()
}

pub(crate) async fn parse_bybit_json_response(
    res: reqwest::Response,
    context: &str,
) -> Result<Value, Box<dyn Error>> {
    let status = res.status();
    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();
    let body = res.text().await?;
    let preview = body_preview(&body);

    if body.trim().is_empty() {
        return Err(format!(
            "{} empty body http={} content_type={}",
            context, status, content_type
        )
        .into());
    }

    let value: Value = serde_json::from_str(&body).map_err(|e| {
        format!(
            "{} invalid json http={} content_type={} err={} body_preview={}",
            context, status, content_type, e, preview
        )
    })?;

    if !status.is_success() {
        return Err(format!(
            "{} http={} content_type={} body={}",
            context, status, content_type, value
        )
        .into());
    }

    Ok(value)
}

pub(crate) async fn bybit_public_get(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    path: &str,
) -> Result<Value, Box<dyn Error>> {
    let url = format!("{}{}", cfg.bybit_base_url(), path);
    let res = http.get(url).send().await?;
    parse_bybit_json_response(res, "bybit public").await
}

pub(crate) async fn bybit_private_get(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    path: &str,
    query: &str,
) -> Result<Value, Box<dyn Error>> {
    let ts = now_ms();
    let sign_payload = format!("{}{}{}{}", ts, cfg.bybit_api_key, cfg.recv_window, query);
    let sign = bybit_sign(&cfg.bybit_api_secret, &sign_payload)?;

    let mut url = format!("{}{}", cfg.bybit_base_url(), path);
    if !query.is_empty() {
        url = format!("{}?{}", url, query);
    }

    let res = http
        .get(url)
        .header("X-BAPI-API-KEY", &cfg.bybit_api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", ts)
        .header("X-BAPI-RECV-WINDOW", &cfg.recv_window)
        .send()
        .await?;

    parse_bybit_json_response(res, "bybit private").await
}

/// Fee rates for a symbol, as fractions (0.00055 == 0.055%).
#[derive(Debug, Clone, Copy)]
pub(crate) struct BybitFeeRate {
    pub(crate) maker: f64,
    pub(crate) taker: f64,
}

/// Fetch the account's real fee rate for `symbol` from Bybit.
///
/// This is the rate actually charged to *this* account, so it already accounts
/// for VIP tier and any promotional discount — which is why we prefer it over
/// the public non-VIP defaults. Requires API credentials; paper mode still gets
/// real rates as long as read-only keys are configured.
pub(crate) async fn fetch_fee_rate(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
) -> Result<BybitFeeRate, Box<dyn Error>> {
    if cfg.bybit_api_key.is_empty() || cfg.bybit_api_secret.is_empty() {
        return Err("no API credentials".into());
    }

    let query = format!("category=linear&symbol={symbol}");
    let value = bybit_private_get(http, cfg, "/v5/account/fee-rate", &query).await?;

    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        return Err(format!("fee-rate retCode={} body={}", ret_code, value).into());
    }

    let entry = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .ok_or("fee-rate response missing result.list[0]")?;

    let parse = |field: &str| -> Option<f64> {
        entry
            .get(field)
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<f64>().ok())
    };

    let maker = parse("makerFeeRate").ok_or("fee-rate missing makerFeeRate")?;
    let taker = parse("takerFeeRate").ok_or("fee-rate missing takerFeeRate")?;

    // A zero/negative taker rate would silently disable cost accounting; treat
    // it as bad data and let the caller fall back to the configured rate.
    if !(taker.is_finite() && taker > 0.0 && maker.is_finite() && maker >= 0.0) {
        return Err(format!("implausible fee rates maker={maker} taker={taker}").into());
    }

    Ok(BybitFeeRate { maker, taker })
}

/// Log which taker fee rate the executor will charge, so a misconfigured or
/// missing API key is visible at boot rather than silently costing accuracy.
pub(crate) async fn log_fee_rate_source(http: &reqwest::Client, cfg: &ExecutorConfig) {
    const PROBE_SYMBOL: &str = "BTCUSDT";
    match fetch_fee_rate(http, cfg, PROBE_SYMBOL).await {
        Ok(rate) => tracing::info!(
            probe_symbol = PROBE_SYMBOL,
            taker_pct = rate.taker * 100.0,
            maker_pct = rate.maker * 100.0,
            source = "bybit_account",
            "Fee rate in force (real account rate)"
        ),
        Err(e) => tracing::warn!(
            probe_symbol = PROBE_SYMBOL,
            error = %e,
            taker_pct = cfg.fee_taker_pct * 100.0,
            source = "configured_fallback",
            "Fee rate in force (could not read account rate)"
        ),
    }
}

pub(crate) async fn bybit_private_post(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    path: &str,
    body: &Value,
) -> Result<Value, Box<dyn Error>> {
    let body_str = serde_json::to_string(body)?;
    let ts = now_ms();
    let sign_payload = format!("{}{}{}{}", ts, cfg.bybit_api_key, cfg.recv_window, body_str);
    let sign = bybit_sign(&cfg.bybit_api_secret, &sign_payload)?;

    let url = format!("{}{}", cfg.bybit_base_url(), path);
    let res = http
        .post(url)
        .header("X-BAPI-API-KEY", &cfg.bybit_api_key)
        .header("X-BAPI-SIGN", sign)
        .header("X-BAPI-SIGN-TYPE", "2")
        .header("X-BAPI-TIMESTAMP", ts)
        .header("X-BAPI-RECV-WINDOW", &cfg.recv_window)
        .header(CONTENT_TYPE, "application/json")
        .body(body_str)
        .send()
        .await?;

    parse_bybit_json_response(res, "bybit private").await
}

pub(crate) async fn run_bybit_sanity_checks(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
) -> Result<(), String> {
    let time_value = bybit_public_get(http, cfg, "/v5/market/time")
        .await
        .map_err(|e| format!("market/time failed: {}", e))?;

    let time_ret = time_value
        .get("retCode")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    if time_ret != 0 {
        return Err(format!(
            "market/time retCode={} body={}",
            time_ret, time_value
        ));
    }

    tracing::info!("Bybit sanity check: market/time OK");

    if matches!(cfg.trading_mode, TradingMode::Paper) {
        tracing::info!("Bybit sanity check: wallet skipped (paper mode uses database simulation)");
        // Paper fills are still charged real fees, so surface at boot which rate
        // is in force — the account's own or the configured fallback.
        log_fee_rate_source(http, cfg).await;
        return Ok(());
    }

    if cfg.bybit_api_key.is_empty() || cfg.bybit_api_secret.is_empty() {
        if cfg.live_orders_enabled {
            return Err("live orders enabled but BYBIT_API_KEY/SECRET missing".to_string());
        }
        tracing::info!("Bybit sanity check: wallet skipped (no API credentials)");
        return Ok(());
    }

    let mut candidates = vec![cfg.bybit_account_type.to_uppercase()];
    for fallback in ["UNIFIED", "CONTRACT", "SPOT"] {
        if !candidates.iter().any(|v| v == fallback) {
            candidates.push(fallback.to_string());
        }
    }

    let mut wallet_errors: Vec<String> = Vec::new();
    let mut wallet_ok_account_type: Option<String> = None;

    for account_type in candidates {
        let query = format!("accountType={account_type}");
        let wallet_value =
            match bybit_private_get(http, cfg, "/v5/account/wallet-balance", &query).await {
                Ok(v) => v,
                Err(e) => {
                    wallet_errors.push(format!("accountType={account_type} request_error={e}"));
                    continue;
                }
            };

        let wallet_ret = wallet_value
            .get("retCode")
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        if wallet_ret == 0 {
            wallet_ok_account_type = Some(account_type.clone());
            break;
        }

        wallet_errors.push(format!(
            "accountType={} retCode={} body={}",
            account_type, wallet_ret, wallet_value
        ));
    }

    if let Some(ok_account_type) = wallet_ok_account_type {
        if ok_account_type != cfg.bybit_account_type.to_uppercase() {
            tracing::warn!(ok_account_type = %ok_account_type, configured = %cfg.bybit_account_type, "Bybit sanity check: wallet-balance OK with fallback");
        } else {
            tracing::info!(account_type = %cfg.bybit_account_type, "Bybit sanity check: wallet-balance OK");
        }
    } else {
        return Err(format!(
            "wallet-balance failed for all accountType candidates: {}",
            wallet_errors.join(" | ")
        ));
    }

    Ok(())
}
