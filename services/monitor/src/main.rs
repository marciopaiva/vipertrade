use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-monitor");

    // Start health check server on port 8084
    let listener = TcpListener::bind("0.0.0.0:8084").await?;
    println!("Health check server running on :8084");

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
