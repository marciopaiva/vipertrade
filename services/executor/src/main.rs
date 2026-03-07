use futures_util::StreamExt;
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use viper_domain::{StrategyDecision, StrategyDecisionEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-executor");

    let listener = TcpListener::bind("0.0.0.0:8083").await?;
    println!("Health check server running on :8083");

    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                    if let Err(e) = socket.write_all(response.as_bytes()).await {
                        eprintln!("failed to write to socket; err = {:?}", e);
                    }
                });
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

    while let Some(msg) = pubsub.on_message().next().await {
        let payload: String = msg.get_payload()?;

        if let Ok(event) = serde_json::from_str::<StrategyDecisionEvent>(&payload) {
            println!(
                "Executor received decision event {} from {} action={} symbol={}",
                event.event_id, event.source_event_id, event.decision.action, event.decision.symbol
            );
            continue;
        }

        if let Ok(decision) = serde_json::from_str::<StrategyDecision>(&payload) {
            println!(
                "Executor received legacy decision action={} symbol={}",
                decision.action, decision.symbol
            );
            continue;
        }

        eprintln!("Executor failed to parse decision payload");
    }

    Ok(())
}
