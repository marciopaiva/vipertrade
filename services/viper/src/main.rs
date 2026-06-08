//! Unified ViperTrade entrypoint. Runs a single service selected by role, so the
//! whole suite ships as one binary/image. Role comes from the `VIPER_ROLE` env
//! var or the first CLI argument.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Role resolution order: VIPER_ROLE env, then `--role <x>`, then first positional arg.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let role = std::env::var("VIPER_ROLE")
        .ok()
        .or_else(|| {
            args.iter()
                .position(|a| a == "--role")
                .and_then(|i| args.get(i + 1).cloned())
                .or_else(|| args.first().filter(|a| !a.starts_with('-')).cloned())
        })
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
        // One-shot deterministic backtest over the recorded input corpus (#37).
        "backtest" => viper_strategy::backtest::run_backtest_cli().await,
        other => {
            eprintln!(
                "unknown role {other:?}; set VIPER_ROLE or pass a role arg: \
                 market-data | analytics | ai-analyst | strategy | executor | monitor | api | backtest"
            );
            std::process::exit(2);
        }
    }
}
