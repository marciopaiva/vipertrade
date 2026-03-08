use hmac::{Hmac, Mac};
use redis::AsyncCommands;
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::error::Error;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::watch;

type HmacSha256 = Hmac<Sha256>;
const RECON_SYMBOLS: [&str; 4] = ["DOGEUSDT", "XRPUSDT", "TRXUSDT", "XLMUSDT"];

#[derive(Debug, Clone)]
struct MonitorConfig {
    health_check_interval_sec: u64,
    reconciliation_interval_sec: u64,
    max_position_drift_notional_usdt: f64,
    alert_cooldown_sec: u64,
    redis_url: String,
    discord_webhook_critical: Option<String>,
    discord_webhook_warning: Option<String>,
    discord_webhook_info: Option<String>,
    bybit_env: String,
    bybit_api_key: String,
    bybit_api_secret: String,
    bybit_recv_window: String,
}

impl MonitorConfig {
    fn from_env() -> Self {
        let health_check_interval_sec =
            read_interval_sec("HEALTH_CHECK_INTERVAL_SEC", "HEALTH_CHECK_INTERVAL_MIN", 60);

        let reconciliation_interval_sec = read_interval_sec(
            "RECONCILIATION_INTERVAL_SEC",
            "RECONCILIATION_INTERVAL_MIN",
            300,
        );

        let max_position_drift_notional_usdt = std::env::var("MAX_POSITION_DRIFT_NOTIONAL_USDT")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(5.0);

        let alert_cooldown_sec = std::env::var("ALERT_COOLDOWN_SEC")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300);

        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://vipertrade-redis:6379".to_string());

        let discord_webhook_critical = read_non_empty_env("DISCORD_WEBHOOK_CRITICAL");
        let discord_webhook_warning = read_non_empty_env("DISCORD_WEBHOOK_WARNING");
        let discord_webhook_info = read_non_empty_env("DISCORD_WEBHOOK_INFO");

        let bybit_env = std::env::var("BYBIT_ENV").unwrap_or_else(|_| "testnet".to_string());
        let bybit_api_key = std::env::var("BYBIT_API_KEY").unwrap_or_default();
        let bybit_api_secret = std::env::var("BYBIT_API_SECRET").unwrap_or_default();
        let bybit_recv_window =
            std::env::var("BYBIT_RECV_WINDOW").unwrap_or_else(|_| "5000".to_string());

        Self {
            health_check_interval_sec,
            reconciliation_interval_sec,
            max_position_drift_notional_usdt,
            alert_cooldown_sec,
            redis_url,
            discord_webhook_critical,
            discord_webhook_warning,
            discord_webhook_info,
            bybit_env,
            bybit_api_key,
            bybit_api_secret,
            bybit_recv_window,
        }
    }

    fn has_bybit_credentials(&self) -> bool {
        !self.bybit_api_key.is_empty() && !self.bybit_api_secret.is_empty()
    }

    fn bybit_base_url(&self) -> &'static str {
        if self.bybit_env.eq_ignore_ascii_case("mainnet") {
            "https://api.bybit.com"
        } else {
            "https://api-testnet.bybit.com"
        }
    }
}

#[derive(Debug, Clone)]
struct ReconResult {
    symbol: String,
    local_notional_usdt: f64,
    bybit_notional_usdt: f64,
    drift_notional_usdt: f64,
    drift_pct: f64,
    severity: &'static str,
    reconciled: bool,
}

fn read_non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn read_interval_sec(sec_var: &str, min_var: &str, default_sec: u64) -> u64 {
    if let Some(sec) = std::env::var(sec_var)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        return sec;
    }

    if let Some(min) = std::env::var(min_var)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        return min.saturating_mul(60);
    }

    default_sec
}

fn now_ms() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    ms.to_string()
}

fn now_epoch_sec() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn bybit_sign(secret: &str, payload: &str) -> Result<String, Box<dyn Error>> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(payload.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn resolve_database_url() -> Option<String> {
    if let Ok(v) = std::env::var("DATABASE_URL") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }

    let host = std::env::var("DB_HOST").ok()?;
    let port = std::env::var("DB_PORT")
        .ok()
        .unwrap_or_else(|| "5432".to_string());
    let db = std::env::var("DB_NAME").ok()?;
    let user = std::env::var("DB_USER").ok()?;
    let pass = std::env::var("DB_PASSWORD").ok()?;

    Some(format!(
        "postgresql://{}:{}@{}:{}/{}",
        user, pass, host, port, db
    ))
}

fn compute_drift(local_notional_usdt: f64, bybit_notional_usdt: f64) -> (f64, f64) {
    let drift = (bybit_notional_usdt - local_notional_usdt).abs();
    let denom = bybit_notional_usdt.abs().max(1.0);
    let drift_pct = drift / denom;
    (drift, drift_pct)
}

fn classify_severity(drift_notional_usdt: f64, threshold: f64) -> &'static str {
    if drift_notional_usdt <= threshold {
        "info"
    } else if drift_notional_usdt <= threshold * 2.0 {
        "warning"
    } else if drift_notional_usdt <= threshold * 4.0 {
        "error"
    } else {
        "critical"
    }
}

fn parse_f64_str(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_str)
        .and_then(|v| v.parse::<f64>().ok())
}

fn parse_position_notional_usdt(value: &Value, symbol: &str) -> Result<f64, Box<dyn Error>> {
    let ret_code = value.get("retCode").and_then(Value::as_i64).unwrap_or(-1);
    if ret_code != 0 {
        let ret_msg = value
            .get("retMsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("bybit retCode={} retMsg={}", ret_code, ret_msg).into());
    }

    let target = symbol.to_uppercase();
    let list = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(Value::as_array)
        .ok_or("bybit result.list missing")?;

    let mut total = 0.0;
    for pos in list {
        let pos_symbol = pos
            .get("symbol")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_uppercase();
        if pos_symbol != target {
            continue;
        }

        let notional = parse_f64_str(pos.get("positionValue"))
            .map(f64::abs)
            .or_else(|| {
                let size = parse_f64_str(pos.get("size"))?;
                let mark = parse_f64_str(pos.get("markPrice"))
                    .or_else(|| parse_f64_str(pos.get("avgPrice")))?;
                Some((size * mark).abs())
            })
            .unwrap_or(0.0);

        total += notional;
    }

    Ok(total)
}

async fn bybit_private_get(
    http: &reqwest::Client,
    cfg: &MonitorConfig,
    path: &str,
    query: &str,
) -> Result<Value, Box<dyn Error>> {
    let ts = now_ms();
    let sign_payload = format!(
        "{}{}{}{}",
        ts, cfg.bybit_api_key, cfg.bybit_recv_window, query
    );
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
        .header("X-BAPI-RECV-WINDOW", &cfg.bybit_recv_window)
        .send()
        .await?;

    let status = res.status();
    let value: Value = res.json().await?;
    if !status.is_success() {
        return Err(format!("bybit private http={} body={}", status, value).into());
    }

    Ok(value)
}

async fn fetch_local_notional_usdt(pool: &PgPool, symbol: &str) -> Result<f64, sqlx::Error> {
    sqlx::query_scalar::<_, f64>(
        "SELECT COALESCE(SUM(quantity * entry_price), 0)::double precision \
         FROM trades WHERE status = 'open' AND symbol = $1",
    )
    .bind(symbol)
    .fetch_one(pool)
    .await
}

async fn fetch_snapshot_notional_usdt(pool: &PgPool, symbol: &str) -> Result<f64, sqlx::Error> {
    let latest = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT (bybit_data->>'notional_usdt')::double precision \
         FROM position_snapshots \
         WHERE symbol = $1 \
         ORDER BY snapshot_at DESC \
         LIMIT 1",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await?;

    Ok(latest.flatten().unwrap_or(0.0))
}

async fn fetch_live_bybit_notional_usdt(
    http: &reqwest::Client,
    cfg: &MonitorConfig,
    symbol: &str,
) -> Result<f64, Box<dyn Error>> {
    let query = format!("category=linear&symbol={}", symbol.to_uppercase());
    let value = bybit_private_get(http, cfg, "/v5/position/list", &query).await?;
    parse_position_notional_usdt(&value, symbol)
}

async fn resolve_bybit_notional_usdt(
    http: &reqwest::Client,
    pool: &PgPool,
    cfg: &MonitorConfig,
    symbol: &str,
) -> Result<f64, Box<dyn Error>> {
    if cfg.has_bybit_credentials() {
        match fetch_live_bybit_notional_usdt(http, cfg, symbol).await {
            Ok(v) => return Ok(v),
            Err(err) => {
                eprintln!(
                    "reconciliation: bybit live query failed for {}: {} (fallback snapshot)",
                    symbol, err
                );
            }
        }
    }

    Ok(fetch_snapshot_notional_usdt(pool, symbol).await?)
}

async fn persist_recon_result(pool: &PgPool, result: &ReconResult) -> Result<(), sqlx::Error> {
    let bybit_json = json!({
        "symbol": result.symbol,
        "notional_usdt": result.bybit_notional_usdt,
    });
    let local_json = json!({
        "symbol": result.symbol,
        "notional_usdt": result.local_notional_usdt,
    });

    sqlx::query(
        "INSERT INTO position_snapshots \
         (symbol, bybit_data, local_calculation, divergence, divergence_pct, reconciled, reconciliation_notes, snapshot_at) \
         VALUES ($1, $2::jsonb, $3::jsonb, $4, $5, $6, $7, NOW())",
    )
    .bind(&result.symbol)
    .bind(bybit_json.to_string())
    .bind(local_json.to_string())
    .bind(result.drift_notional_usdt)
    .bind(result.drift_pct)
    .bind(result.reconciled)
    .bind(format!(
        "severity={} threshold check for symbol {}",
        result.severity, result.symbol
    ))
    .execute(pool)
    .await?;

    let event_data = json!({
        "symbol": result.symbol,
        "local_notional_usdt": result.local_notional_usdt,
        "bybit_notional_usdt": result.bybit_notional_usdt,
        "drift_notional_usdt": result.drift_notional_usdt,
        "drift_pct": result.drift_pct,
        "reconciled": result.reconciled,
    });

    sqlx::query(
        "INSERT INTO system_events \
         (event_type, severity, category, data, symbol, timestamp) \
         VALUES ($1, $2, $3, $4::jsonb, $5, NOW())",
    )
    .bind("reconciliation_cycle")
    .bind(result.severity)
    .bind("reconciliation")
    .bind(event_data.to_string())
    .bind(&result.symbol)
    .execute(pool)
    .await?;

    Ok(())
}

async fn publish_recon_event(redis_url: &str, result: &ReconResult) {
    let payload = json!({
        "schema_version": "1.0",
        "event_type": "reconciliation",
        "symbol": result.symbol,
        "severity": result.severity,
        "reconciled": result.reconciled,
        "local_notional_usdt": result.local_notional_usdt,
        "bybit_notional_usdt": result.bybit_notional_usdt,
        "drift_notional_usdt": result.drift_notional_usdt,
        "drift_pct": result.drift_pct,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    let Ok(client) = redis::Client::open(redis_url) else {
        eprintln!("reconciliation: invalid REDIS_URL {}", redis_url);
        return;
    };

    let Ok(mut conn) = client.get_multiplexed_async_connection().await else {
        eprintln!(
            "reconciliation: failed to connect to Redis at {}",
            redis_url
        );
        return;
    };

    let publish_result: Result<(), redis::RedisError> = conn
        .publish("viper:reconciliation", payload.to_string())
        .await;

    if let Err(err) = publish_result {
        eprintln!("reconciliation: failed to publish Redis event: {}", err);
    }
}

fn discord_webhook_for<'a>(cfg: &'a MonitorConfig, severity: &str) -> Option<&'a str> {
    match severity {
        "critical" | "error" => cfg.discord_webhook_critical.as_deref(),
        "warning" => cfg.discord_webhook_warning.as_deref(),
        _ => cfg.discord_webhook_info.as_deref(),
    }
}

fn should_emit_alert(last_sent_at: Option<i64>, now_epoch_sec: i64, cooldown_sec: u64) -> bool {
    if cooldown_sec == 0 {
        return true;
    }

    match last_sent_at {
        None => true,
        Some(last) => now_epoch_sec.saturating_sub(last) >= cooldown_sec as i64,
    }
}

async fn maybe_publish_discord_alert(
    cfg: &MonitorConfig,
    result: &ReconResult,
    alert_last_sent: &mut HashMap<String, i64>,
) {
    if result.severity == "info" {
        return;
    }

    let Some(webhook) = discord_webhook_for(cfg, result.severity) else {
        return;
    };

    let now = now_epoch_sec();
    let key = format!("{}:{}", result.symbol, result.severity);
    let last = alert_last_sent.get(&key).copied();
    if !should_emit_alert(last, now, cfg.alert_cooldown_sec) {
        println!(
            "reconciliation: alert suppressed by cooldown symbol={} severity={} cooldown_sec={}",
            result.symbol, result.severity, cfg.alert_cooldown_sec
        );
        return;
    }

    let content = format!(
        "[vipertrade][reconciliation][{}] symbol={} drift_notional_usdt={:.6} drift_pct={:.6} local_notional_usdt={:.6} bybit_notional_usdt={:.6} reconciled={}",
        result.severity,
        result.symbol,
        result.drift_notional_usdt,
        result.drift_pct,
        result.local_notional_usdt,
        result.bybit_notional_usdt,
        result.reconciled
    );

    let payload = json!({ "content": content });
    let client = reqwest::Client::new();
    match client.post(webhook).json(&payload).send().await {
        Ok(_) => {
            alert_last_sent.insert(key, now);
        }
        Err(err) => {
            eprintln!(
                "reconciliation: failed to publish Discord alert for {}: {}",
                result.symbol, err
            );
        }
    }
}

async fn run_reconciliation_cycle(
    http: &reqwest::Client,
    pool: &PgPool,
    cfg: &MonitorConfig,
    alert_last_sent: &mut HashMap<String, i64>,
) {
    for symbol in RECON_SYMBOLS {
        let local_notional_usdt = match fetch_local_notional_usdt(pool, symbol).await {
            Ok(v) => v,
            Err(err) => {
                eprintln!("reconciliation: local query failed for {}: {}", symbol, err);
                continue;
            }
        };

        let bybit_notional_usdt = match resolve_bybit_notional_usdt(http, pool, cfg, symbol).await {
            Ok(v) => v,
            Err(err) => {
                eprintln!(
                    "reconciliation: failed to resolve bybit notional for {}: {}",
                    symbol, err
                );
                continue;
            }
        };

        let (drift_notional_usdt, drift_pct) =
            compute_drift(local_notional_usdt, bybit_notional_usdt);
        let severity = classify_severity(drift_notional_usdt, cfg.max_position_drift_notional_usdt);
        let reconciled = drift_notional_usdt <= cfg.max_position_drift_notional_usdt;

        let result = ReconResult {
            symbol: symbol.to_string(),
            local_notional_usdt,
            bybit_notional_usdt,
            drift_notional_usdt,
            drift_pct,
            severity,
            reconciled,
        };

        if let Err(err) = persist_recon_result(pool, &result).await {
            eprintln!("reconciliation: persist failed for {}: {}", symbol, err);
            continue;
        }

        publish_recon_event(&cfg.redis_url, &result).await;
        maybe_publish_discord_alert(cfg, &result, alert_last_sent).await;

        println!(
            "reconciliation: symbol={} local={} bybit={} drift={} severity={}",
            result.symbol,
            result.local_notional_usdt,
            result.bybit_notional_usdt,
            result.drift_notional_usdt,
            result.severity
        );
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {},
                    _ = sigterm.recv() => {},
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-monitor");

    let cfg = MonitorConfig::from_env();
    println!(
        "Monitor config: health_interval={}s reconciliation_interval={}s max_drift={} USDT cooldown={}s bybit_env={}",
        cfg.health_check_interval_sec,
        cfg.reconciliation_interval_sec,
        cfg.max_position_drift_notional_usdt,
        cfg.alert_cooldown_sec,
        cfg.bybit_env
    );

    let pool = if let Some(database_url) = resolve_database_url() {
        match PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                println!("Connected to PostgreSQL for reconciliation");
                Some(pool)
            }
            Err(err) => {
                eprintln!("monitor: failed to connect PostgreSQL: {}", err);
                None
            }
        }
    } else {
        eprintln!("monitor: database env not configured; reconciliation loop disabled");
        None
    };

    let listener = TcpListener::bind("0.0.0.0:8084").await?;
    println!("Health check server running on :8084");

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let mut health_task_shutdown = shutdown_rx.clone();
    let health_interval = cfg.health_check_interval_sec;
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(health_interval));
        loop {
            tokio::select! {
                _ = health_task_shutdown.changed() => {
                    break;
                }
                _ = ticker.tick() => {
                    println!("monitor heartbeat: health checks scheduled");
                }
            }
        }
    });

    if let Some(pool) = pool {
        let http = reqwest::Client::new();
        let mut recon_task_shutdown = shutdown_rx.clone();
        let recon_interval = cfg.reconciliation_interval_sec;
        let cfg_for_recon = cfg.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(recon_interval));
            let mut alert_last_sent: HashMap<String, i64> = HashMap::new();
            loop {
                tokio::select! {
                    _ = recon_task_shutdown.changed() => {
                        break;
                    }
                    _ = ticker.tick() => {
                        run_reconciliation_cycle(&http, &pool, &cfg_for_recon, &mut alert_last_sent).await;
                    }
                }
            }
        });
    }

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                println!("Received shutdown signal, stopping viper-monitor");
                break;
            }
            accept_result = listener.accept() => {
                let (mut socket, _) = accept_result?;
                tokio::spawn(async move {
                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                    if let Err(e) = socket.write_all(response.as_bytes()).await {
                        eprintln!("failed to write to socket; err = {:?}", e);
                    }
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_severity, compute_drift, parse_position_notional_usdt, read_interval_sec,
        should_emit_alert,
    };
    use serde_json::json;

    #[test]
    fn compute_drift_uses_absolute_delta() {
        let (drift, pct) = compute_drift(10.0, 12.5);
        assert!((drift - 2.5).abs() < f64::EPSILON);
        assert!(pct > 0.19 && pct < 0.21);
    }

    #[test]
    fn classify_severity_levels() {
        let threshold = 5.0;
        assert_eq!(classify_severity(1.0, threshold), "info");
        assert_eq!(classify_severity(8.0, threshold), "warning");
        assert_eq!(classify_severity(19.0, threshold), "error");
        assert_eq!(classify_severity(25.0, threshold), "critical");
    }

    #[test]
    fn read_interval_uses_min_fallback() {
        const SEC_VAR: &str = "VT_TEST_HEALTH_SEC";
        const MIN_VAR: &str = "VT_TEST_HEALTH_MIN";

        std::env::remove_var(SEC_VAR);
        std::env::set_var(MIN_VAR, "2");
        assert_eq!(read_interval_sec(SEC_VAR, MIN_VAR, 60), 120);
        std::env::remove_var(MIN_VAR);
    }

    #[test]
    fn parse_position_notional_uses_position_value() {
        let payload = json!({
            "retCode": 0,
            "result": {
                "list": [
                    {"symbol": "DOGEUSDT", "positionValue": "12.5", "size": "100", "markPrice": "0.12"},
                    {"symbol": "DOGEUSDT", "positionValue": "7.5", "size": "50", "markPrice": "0.15"}
                ]
            }
        });

        let v = parse_position_notional_usdt(&payload, "DOGEUSDT").expect("parse must work");
        assert!((v - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_position_notional_falls_back_to_size_x_mark() {
        let payload = json!({
            "retCode": 0,
            "result": {
                "list": [
                    {"symbol": "XRPUSDT", "size": "10", "markPrice": "2.1"}
                ]
            }
        });

        let v = parse_position_notional_usdt(&payload, "XRPUSDT").expect("parse must work");
        assert!((v - 21.0).abs() < f64::EPSILON);
    }

    #[test]
    fn should_emit_alert_respects_cooldown() {
        assert!(should_emit_alert(None, 1000, 300));
        assert!(!should_emit_alert(Some(900), 1000, 300));
        assert!(should_emit_alert(Some(600), 1000, 300));
        assert!(should_emit_alert(Some(995), 1000, 0));
    }
}
