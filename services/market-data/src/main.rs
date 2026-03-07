use redis::AsyncCommands;
use serde::Serialize;
use std::error::Error;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

#[derive(Debug, Serialize)]
struct MarketSignal {
    symbol: String,
    current_price: f64,
    atr_14: f64,
    volume_24h: i64,
    funding_rate: f64,
    trend_score: f64,
    spread_pct: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-market-data");

    // Start health check server on port 8081
    let listener = TcpListener::bind("0.0.0.0:8081").await?;
    println!("Health check server running on :8081");

    // Background task for health check
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

    // Redis connection
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    println!("Connecting to Redis at {}", redis_url);

    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    println!("Connected to Redis. Starting market data loop...");

    let symbols = vec!["BTCUSDT", "ETHUSDT", "SOLUSDT"];
    let mut price = 60000.0;

    loop {
        for symbol in &symbols {
            // Simple random walk
            let change = (rand::random::<f64>() - 0.5) * 100.0;
            price += change;

            let signal = MarketSignal {
                symbol: symbol.to_string(),
                current_price: price,
                atr_14: price * 0.02,
                volume_24h: 1000000,
                funding_rate: 0.0001,
                trend_score: 0.8,
                spread_pct: 0.001,
            };

            let json = serde_json::to_string(&signal)?;
            conn.publish::<_, _, ()>("viper:market_data", json).await?;
            println!("Published signal for {}", symbol);
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
