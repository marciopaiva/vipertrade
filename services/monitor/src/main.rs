use redis::AsyncCommands;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::error::Error;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::watch;

const RECON_SYMBOLS: [&str; 4] = ["DOGEUSDT", "XRPUSDT", "TRXUSDT", "XLMUSDT"];

#[derive(Debug, Clone)]
struct MonitorConfig {
    health_check_interval_sec: u64,
    reconciliation_interval_sec: u64,
    max_position_drift_notional_usdt: f64,
    redis_url: String,
    discord_webhook_critical: Option<String>,
    discord_webhook_warning: Option<String>,
    discord_webhook_info: Option<String>,
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

        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://vipertrade-redis:6379".to_string());

        let discord_webhook_critical = read_non_empty_env("DISCORD_WEBHOOK_CRITICAL");
        let discord_webhook_warning = read_non_empty_env("DISCORD_WEBHOOK_WARNING");
        let discord_webhook_info = read_non_empty_env("DISCORD_WEBHOOK_INFO");

        Self {
            health_check_interval_sec,
            reconciliation_interval_sec,
            max_position_drift_notional_usdt,
            redis_url,
            discord_webhook_critical,
            discord_webhook_warning,
            discord_webhook_info,
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

async fn fetch_local_notional_usdt(pool: &PgPool, symbol: &str) -> Result<f64, sqlx::Error> {
    sqlx::query_scalar::<_, f64>(
        "SELECT COALESCE(SUM(quantity * entry_price), 0)::double precision \
         FROM trades WHERE status = 'open' AND symbol = $1",
    )
    .bind(symbol)
    .fetch_one(pool)
    .await
}

async fn fetch_bybit_notional_usdt(pool: &PgPool, symbol: &str) -> Result<f64, sqlx::Error> {
    let latest = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT (bybit_data->>'notional_usdt')::double precision \
         FROM position_snapshots \
         WHERE symbol = $1 \
         ORDER BY snapshot_at DESC \
         LIMIT 1",
    )
    .bind(symbol)
    .fetch_one(pool)
    .await?;

    Ok(latest.unwrap_or(0.0))
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

async fn publish_discord_alert(cfg: &MonitorConfig, result: &ReconResult) {
    if result.severity == "info" {
        return;
    }

    let Some(webhook) = discord_webhook_for(cfg, result.severity) else {
        return;
    };

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
    if let Err(err) = client.post(webhook).json(&payload).send().await {
        eprintln!(
            "reconciliation: failed to publish Discord alert for {}: {}",
            result.symbol, err
        );
    }
}

async fn run_reconciliation_cycle(pool: &PgPool, cfg: &MonitorConfig) {
    for symbol in RECON_SYMBOLS {
        let local_notional_usdt = match fetch_local_notional_usdt(pool, symbol).await {
            Ok(v) => v,
            Err(err) => {
                eprintln!("reconciliation: local query failed for {}: {}", symbol, err);
                continue;
            }
        };

        let bybit_notional_usdt = match fetch_bybit_notional_usdt(pool, symbol).await {
            Ok(v) => v,
            Err(err) => {
                eprintln!(
                    "reconciliation: bybit snapshot query failed for {}: {}",
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
        publish_discord_alert(cfg, &result).await;

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
        "Monitor config: health_interval={}s reconciliation_interval={}s max_drift={} USDT",
        cfg.health_check_interval_sec,
        cfg.reconciliation_interval_sec,
        cfg.max_position_drift_notional_usdt
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
        let mut recon_task_shutdown = shutdown_rx.clone();
        let recon_interval = cfg.reconciliation_interval_sec;
        let cfg_for_recon = cfg.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(recon_interval));
            loop {
                tokio::select! {
                    _ = recon_task_shutdown.changed() => {
                        break;
                    }
                    _ = ticker.tick() => {
                        run_reconciliation_cycle(&pool, &cfg_for_recon).await;
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
    use super::{classify_severity, compute_drift, read_interval_sec};

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
}
