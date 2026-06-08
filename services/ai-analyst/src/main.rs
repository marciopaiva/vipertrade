#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    viper_ai_analyst::run().await
}
