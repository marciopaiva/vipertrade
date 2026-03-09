use redis::AsyncCommands;
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::watch;
use viper_domain::{MarketSignal, MarketSignalEvent};

fn parse_trading_pairs() -> Vec<String> {
    let raw = std::env::var("TRADING_PAIRS")
        .unwrap_or_else(|_| "DOGEUSDT,XRPUSDT,TRXUSDT,XLMUSDT".to_string());
    let pairs: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    if pairs.is_empty() {
        vec![
            "DOGEUSDT".to_string(),
            "XRPUSDT".to_string(),
            "TRXUSDT".to_string(),
            "XLMUSDT".to_string(),
        ]
    } else {
        pairs
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
    println!("Starting viper-market-data");

    let listener = TcpListener::bind("0.0.0.0:8081").await?;
    println!("Health check server running on :8081");

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
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    println!("Connecting to Redis at {}", redis_url);

    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    println!("Connected to Redis. Starting market data loop...");

    let bybit_env = std::env::var("BYBIT_ENV").unwrap_or_else(|_| "testnet".to_string());
    let symbols = parse_trading_pairs();
    println!(
        "Market-data running in BYBIT_ENV={} with pairs={}",
        bybit_env,
        symbols.join(",")
    );

    let mut prices: HashMap<String, f64> = symbols
        .iter()
        .map(|s| (s.clone(), 100.0 + rand::random::<f64>() * 50.0))
        .collect();

    loop {
        if *shutdown_rx.borrow() {
            println!("Received shutdown signal, stopping viper-market-data");
            break;
        }

        for symbol in &symbols {
            let entry = prices.entry(symbol.clone()).or_insert(100.0);
            let change = (rand::random::<f64>() - 0.5) * 100.0;
            *entry = (*entry + change).max(0.0001);

            let signal = MarketSignal {
                symbol: symbol.to_string(),
                current_price: *entry,
                atr_14: *entry * 0.02,
                volume_24h: 1_000_000,
                funding_rate: 0.0001,
                trend_score: 0.8,
                spread_pct: 0.001,
            };

            let event = MarketSignalEvent::new(signal);
            let json = serde_json::to_string(&event)?;
            if let Err(e) = conn.publish::<_, _, ()>("viper:market_data", json).await {
                eprintln!("Failed to publish market data: {}", e);
                break;
            }
            println!(
                "Published market event {} for {}",
                event.event_id, event.signal.symbol
            );
        }

        tokio::select! {
            _ = shutdown_rx.changed() => {
                println!("Received shutdown signal, stopping viper-market-data");
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}
