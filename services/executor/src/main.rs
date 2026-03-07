use futures_util::StreamExt;
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::watch;
use viper_domain::{StrategyDecision, StrategyDecisionEvent};

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
    println!("Starting viper-executor");

    let listener = TcpListener::bind("0.0.0.0:8083").await?;
    println!("Health check server running on :8083");

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let mut health_shutdown_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = health_shutdown_rx.changed() => {
                    break;
                }
                accept_result = listener.accept() => {
                    if let Ok((mut socket, _)) = accept_result {
                        tokio::spawn(async move {
                            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                            if let Err(e) = socket.write_all(response.as_bytes()).await {
                                eprintln!("failed to write to socket; err = {:?}", e);
                            }
                        });
                    }
                }
            }
        }
    });

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://vipertrade-redis:6379".to_string());
    println!("Connecting to Redis at {}", redis_url);

    let client = redis::Client::open(redis_url)?;
    #[allow(deprecated)]
    let mut pubsub = client.get_async_connection().await?.into_pubsub();
    pubsub.subscribe("viper:decisions").await?;
    println!("Subscribed to viper:decisions");

    let mut messages = pubsub.on_message();

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                println!("Received shutdown signal, stopping viper-executor");
                break;
            }
            maybe_msg = messages.next() => {
                let Some(msg) = maybe_msg else {
                    println!("Decision stream ended, stopping viper-executor");
                    break;
                };

                let payload: String = msg.get_payload()?;

                if let Ok(event) = serde_json::from_str::<StrategyDecisionEvent>(&payload) {
                    if let Err(err) = event.validate() {
                        eprintln!(
                            "Executor rejected invalid decision event contract event_id={} err={}",
                            event.event_id, err
                        );
                        continue;
                    }

                    println!(
                        "Executor received decision event {} from {} action={} symbol={}",
                        event.event_id, event.source_event_id, event.decision.action, event.decision.symbol
                    );
                    continue;
                }

                if let Ok(decision) = serde_json::from_str::<StrategyDecision>(&payload) {
                    if let Err(err) = decision.validate() {
                        eprintln!("Executor rejected invalid legacy decision err={}", err);
                        continue;
                    }

                    println!(
                        "Executor received legacy decision action={} symbol={}",
                        decision.action, decision.symbol
                    );
                    continue;
                }

                eprintln!("Executor failed to parse decision payload");
            }
        }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}
