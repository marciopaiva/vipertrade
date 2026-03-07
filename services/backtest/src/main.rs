use tokio::net::TcpListener;
use tokio::io::AsyncWriteExt;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-backtest");

    // Start health check server on port 8085
    let listener = TcpListener::bind("0.0.0.0:8085").await?;
    println!("Health check server running on :8085");

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
            if let Err(e) = socket.write_all(response.as_bytes()).await {
                eprintln!("failed to write to socket; err = {:?}", e);
            }
        });
    }
}
