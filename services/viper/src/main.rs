//! Unified ViperTrade entrypoint. Runs a single service selected by role, so the
//! whole suite ships as one binary/image. Role comes from the `VIPER_ROLE` env
//! var or the first CLI argument.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let role = std::env::var("VIPER_ROLE")
        .ok()
        .or_else(|| std::env::args().nth(1))
        .unwrap_or_default();

    match role.as_str() {
        "market-data" => viper_market_data::run().await,
        "analytics" => viper_analytics::run().await,
        "ai-analyst" => viper_ai_analyst::run().await,
        "strategy" => viper_strategy::run().await,
        "executor" => viper_executor::run().await,
        "monitor" => viper_monitor::run().await,
        "api" => {
            viper_api::run().await;
            Ok(())
        }
        other => {
            eprintln!(
                "unknown role {other:?}; set VIPER_ROLE or pass a role arg: \
                 market-data | analytics | ai-analyst | strategy | executor | monitor | api"
            );
            std::process::exit(2);
        }
    }
}
