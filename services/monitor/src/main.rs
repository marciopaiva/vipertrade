#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    viper_monitor::run().await
}
