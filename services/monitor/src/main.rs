use std::error::Error;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::watch;

#[derive(Debug, Clone)]
struct MonitorConfig {
    health_check_interval_sec: u64,
    reconciliation_interval_sec: u64,
    max_position_drift_notional_usdt: f64,
}

impl MonitorConfig {
    fn from_env() -> Self {
        let health_check_interval_sec = std::env::var("HEALTH_CHECK_INTERVAL_SEC")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);

        let reconciliation_interval_sec = std::env::var("RECONCILIATION_INTERVAL_SEC")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300);

        let max_position_drift_notional_usdt = std::env::var("MAX_POSITION_DRIFT_NOTIONAL_USDT")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(5.0);

        Self {
            health_check_interval_sec,
            reconciliation_interval_sec,
            max_position_drift_notional_usdt,
        }
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
                    // Phase 2 baseline: periodic monitor heartbeat.
                    println!("monitor heartbeat: health checks scheduled");
                }
            }
        }
    });

    let mut recon_task_shutdown = shutdown_rx.clone();
    let recon_interval = cfg.reconciliation_interval_sec;
    let max_drift = cfg.max_position_drift_notional_usdt;
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(recon_interval));
        loop {
            tokio::select! {
                _ = recon_task_shutdown.changed() => {
                    break;
                }
                _ = ticker.tick() => {
                    // Phase 2 baseline: reconciliation scheduler placeholder.
                    println!(
                        "reconciliation cycle: validating position drift threshold {} USDT",
                        max_drift
                    );
                }
            }
        }
    });

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
