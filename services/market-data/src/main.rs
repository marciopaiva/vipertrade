#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    viper_market_data::run().await
}
