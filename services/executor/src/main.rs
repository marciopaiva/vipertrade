#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    viper_executor::run().await
}
