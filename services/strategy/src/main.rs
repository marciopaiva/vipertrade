use futures_util::StreamExt;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error;
use std::fs;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tupa_codegen::execution_plan::{codegen_pipeline, ExecutionPlan};
use tupa_parser::{parse_program, Item, PipelineDecl, Program};
use tupa_runtime::Runtime;
use tupa_typecheck::typecheck_program;

#[derive(Debug, Serialize, Deserialize)]
struct MarketSignal {
    symbol: String,
    current_price: f64,
    atr_14: f64,
    volume_24h: i64,
    funding_rate: f64,
    trend_score: f64,
    spread_pct: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct StrategyDecision {
    action: String,
    symbol: String,
    quantity: f64,
    leverage: f64,
    entry_price: f64,
    stop_loss: f64,
    take_profit: f64,
    reason: String,
    smart_copy_compatible: bool,
}

fn first_pipeline(program: &Program) -> Result<&PipelineDecl, Box<dyn Error>> {
    program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Pipeline(p) => Some(p),
            _ => None,
        })
        .ok_or_else(|| "no pipeline declaration found".into())
}

fn load_execution_plan(path: &str) -> Result<ExecutionPlan, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let program = parse_program(&source)?;

    if let Err(err) = typecheck_program(&program) {
        eprintln!("Typecheck warning (continuing): {}", err);
    }

    let pipeline = first_pipeline(&program)?;
    let plan_json = codegen_pipeline("vipertrade", pipeline, &program)?;
    let plan: ExecutionPlan = serde_json::from_str(&plan_json)?;
    Ok(plan)
}

fn get_f64(state: &Value, key: &str, default: f64) -> f64 {
    state.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn get_i64(state: &Value, key: &str, default: i64) -> i64 {
    state.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn get_bool(state: &Value, key: &str, default: bool) -> bool {
    state.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn get_string(state: &Value, key: &str, default: &str) -> String {
    state
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn execute_strategy_step(step_name: &str, state: Value) -> Result<Value, String> {
    match step_name {
        "check_daily_loss" => {
            let current_daily_loss = get_f64(&state, "current_daily_loss", 0.0);
            let max_daily_loss_pct = get_f64(&state, "max_daily_loss_pct", 0.05);
            Ok(json!(current_daily_loss <= max_daily_loss_pct))
        }
        "check_consecutive_losses" => {
            let losses = get_i64(&state, "consecutive_losses", 0);
            Ok(json!(losses <= 3))
        }
        "validate_entry" => {
            let spread_pct = get_f64(&state, "spread_pct", 1.0);
            let volume_24h = get_i64(&state, "volume_24h", 0);
            let trend_score = get_f64(&state, "trend_score", 0.0).abs();
            Ok(json!(spread_pct <= 0.2 && volume_24h >= 100_000 && trend_score >= 0.15))
        }
        "check_funding" => {
            let funding_rate = get_f64(&state, "funding_rate", 0.0).abs();
            Ok(json!(funding_rate <= 0.015))
        }
        "calc_smart_size" => {
            let price = get_f64(&state, "current_price", 1.0).max(1.0);
            let equity_usdt = get_f64(&state, "account_equity_usdt", 1_000.0);
            let risk_pct = get_f64(&state, "risk_per_trade_pct", 0.01).max(0.001);
            let size = ((equity_usdt * risk_pct) / price).clamp(0.001, 100.0);
            Ok(json!(size))
        }
        "validate_size" => {
            let size = get_f64(&state, "calc_smart_size", 0.0);
            let min_size = get_f64(&state, "min_position_usdt", 0.001);
            let max_size = get_f64(&state, "max_position_usdt", 100.0);
            Ok(json!(size >= min_size && size <= max_size))
        }
        "get_trailing_config" => Ok(json!({
            "activate_after_profit_pct": 0.004,
            "initial_trail_pct": 0.002,
            "ratchet_levels": [
                {"at_profit_pct": 0.008, "trail_pct": 0.003},
                {"at_profit_pct": 0.015, "trail_pct": 0.005}
            ],
            "move_to_break_even_at": 0.01
        })),
        "decision" => {
            let can_enter = get_bool(&state, "check_daily_loss", false)
                && get_bool(&state, "check_consecutive_losses", false)
                && get_bool(&state, "validate_entry", false)
                && get_bool(&state, "check_funding", false)
                && get_bool(&state, "validate_size", false);

            let symbol = get_string(&state, "symbol", "UNKNOWN");
            let entry_price = get_f64(&state, "current_price", 0.0);
            let quantity = get_f64(&state, "calc_smart_size", 0.0);
            let trend = get_f64(&state, "trend_score", 0.0);
            let leverage = 3.0;

            if can_enter && quantity > 0.0 && entry_price > 0.0 {
                let is_long = trend >= 0.0;
                let stop_loss = if is_long {
                    entry_price * 0.99
                } else {
                    entry_price * 1.01
                };
                let take_profit = if is_long {
                    entry_price * 1.02
                } else {
                    entry_price * 0.98
                };

                Ok(json!({
                    "action": if is_long { "ENTER_LONG" } else { "ENTER_SHORT" },
                    "symbol": symbol,
                    "quantity": quantity,
                    "leverage": leverage,
                    "entry_price": entry_price,
                    "stop_loss": stop_loss,
                    "take_profit": take_profit,
                    "reason": "in_process_runtime",
                    "smart_copy_compatible": true
                }))
            } else {
                Ok(json!({
                    "action": "HOLD",
                    "symbol": symbol,
                    "quantity": 0.0,
                    "leverage": 0.0,
                    "entry_price": 0.0,
                    "stop_loss": 0.0,
                    "take_profit": 0.0,
                    "reason": "risk_constraints_not_met",
                    "smart_copy_compatible": false
                }))
            }
        }
        "audit" => Ok(json!({"ok": true})),
        _ => Ok(Value::Null),
    }
}

fn register_strategy_steps(runtime: &Runtime, plan: &ExecutionPlan) {
    for step in &plan.steps {
        let function_ref = step.function_ref.clone();
        let fallback_step_name = step.name.clone();
        let step_name = function_ref
            .split("::step_")
            .nth(1)
            .unwrap_or(&fallback_step_name)
            .to_string();

        runtime.register_step(&function_ref, move |state| execute_strategy_step(&step_name, state));
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-strategy");

    let listener = TcpListener::bind("0.0.0.0:8082").await?;
    println!("Health check server running on :8082");

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
    let pipeline_path = std::env::var("VIPER_PIPELINE_PATH")
        .unwrap_or_else(|_| "config/strategies/viper_smart_copy.tp".to_string());
    let execution_plan = load_execution_plan(&pipeline_path)?;

    let runtime = Runtime::new();
    register_strategy_steps(&runtime, &execution_plan);
    println!(
        "Loaded in-process plan '{}' with {} step(s)",
        execution_plan.name,
        execution_plan.steps.len()
    );

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://vipertrade-redis:6379".to_string());
    println!("Connecting to Redis at {}", redis_url);

    let client = redis::Client::open(redis_url)?;
    #[allow(deprecated)]
    let mut pubsub = client.get_async_connection().await?.into_pubsub();

    pubsub.subscribe("viper:market_data").await?;
    println!("Subscribed to viper:market_data");

    let mut publish_conn = client.get_multiplexed_async_connection().await?;

    while let Some(msg) = pubsub.on_message().next().await {
        let payload: String = msg.get_payload()?;

        let signal: MarketSignal = match serde_json::from_str(&payload) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to parse market signal: {}", e);
                continue;
            }
        };

        let input = serde_json::to_value(&signal)?;
        let runtime_output = match runtime.run_pipeline_async(&execution_plan, input).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("In-process Tupa runtime failed: {}", e);
                continue;
            }
        };

        let decision_value = runtime_output.get("decision").cloned();
        let Some(decision_value) = decision_value else {
            eprintln!("Pipeline output missing 'decision' step result");
            continue;
        };

        match serde_json::from_value::<StrategyDecision>(decision_value.clone()) {
            Ok(decision) => {
                let decision_json = serde_json::to_string(&decision)?;
                publish_conn.publish::<_, _, ()>("viper:decisions", decision_json).await?;
                println!("Published decision for {}", signal.symbol);
            }
            Err(_) => {
                let decision_json = serde_json::to_string(&decision_value)?;
                publish_conn.publish::<_, _, ()>("viper:decisions", decision_json).await?;
                println!("Published decision (raw) for {}", signal.symbol);
            }
        }
    }

    Ok(())
}
