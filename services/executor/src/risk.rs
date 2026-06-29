use serde_json::Value;
use std::error::Error;
use std::time::{Duration, Instant};

use crate::*;

pub(crate) fn parse_positive_f64(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_str)
        .and_then(|x| x.parse::<f64>().ok())
        .filter(|x| *x > 0.0)
}

pub(crate) async fn fetch_symbol_constraints(
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
) -> Result<BybitSymbolConstraints, Box<dyn Error>> {
    let path = format!(
        "/v5/market/instruments-info?category=linear&symbol={}",
        symbol.to_uppercase()
    );
    let value = bybit_public_get(http, cfg, &path).await?;

    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("bybit retCode={} retMsg={}", ret_code, ret_msg).into());
    }

    let instrument = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .ok_or("missing instrument metadata")?;

    let lot = instrument
        .get("lotSizeFilter")
        .ok_or("missing lotSizeFilter")?;
    let price_filter = instrument.get("priceFilter").ok_or("missing priceFilter")?;

    let min_order_qty = parse_positive_f64(lot.get("minOrderQty")).ok_or("missing minOrderQty")?;
    let qty_step = parse_positive_f64(lot.get("qtyStep")).ok_or("missing qtyStep")?;
    let min_notional = parse_positive_f64(lot.get("minNotionalValue"));
    let tick_size = parse_positive_f64(price_filter.get("tickSize")).ok_or("missing tickSize")?;

    Ok(BybitSymbolConstraints {
        min_order_qty,
        qty_step,
        min_notional,
        tick_size,
    })
}

pub(crate) async fn get_symbol_constraints(
    state: &ExecutorState,
    http: &reqwest::Client,
    cfg: &ExecutorConfig,
    symbol: &str,
) -> Result<BybitSymbolConstraints, Box<dyn Error>> {
    let key = symbol.to_uppercase();

    {
        let cache = state.constraints_cache.lock().await;
        if let Some((cached_at, constraints)) = cache.get(&key) {
            if cached_at.elapsed() < Duration::from_secs(CONSTRAINTS_CACHE_TTL_SECS) {
                return Ok(*constraints);
            }
        }
    }

    let fetched = fetch_symbol_constraints(http, cfg, symbol).await?;
    let mut cache = state.constraints_cache.lock().await;
    cache.insert(key, (Instant::now(), fetched));
    Ok(fetched)
}
