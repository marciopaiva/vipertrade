use futures_util::StreamExt;
use redis::AsyncCommands;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tupa_codegen::execution_plan::{codegen_pipeline, ExecutionPlan};
use tupa_parser::{parse_program, Item, PipelineDecl, Program};
use tupa_runtime::Runtime;
use tupa_typecheck::typecheck_program;
use viper_domain::{MarketSignal, MarketSignalEvent, StrategyDecision, StrategyDecisionEvent};

#[derive(Debug, Clone)]
struct StrategyConfig {
    profile: String,
    global: Value,
    pairs: HashMap<String, Value>,
    profiles: Value,
}

impl StrategyConfig {
    fn from_files(
        pairs_path: &str,
        profiles_path: &str,
        profile: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let pairs_raw = fs::read_to_string(pairs_path)?;
        let profiles_raw = fs::read_to_string(profiles_path)?;

        let pairs_yaml: serde_yaml::Value = serde_yaml::from_str(&pairs_raw)?;
        let profiles_yaml: serde_yaml::Value = serde_yaml::from_str(&profiles_raw)?;

        let pairs_json = serde_json::to_value(pairs_yaml)?;
        let profiles_json = serde_json::to_value(profiles_yaml)?;

        let global = pairs_json
            .get("global")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let mut pairs = HashMap::new();
        if let Some(obj) = pairs_json.as_object() {
            for (name, cfg) in obj {
                if name != "global" {
                    pairs.insert(name.to_uppercase(), cfg.clone());
                }
            }
        }

        Ok(Self {
            profile: profile.to_uppercase(),
            global,
            pairs,
            profiles: profiles_json,
        })
    }

    fn profile_cfg(&self) -> Option<&Value> {
        self.profiles.get(&self.profile)
    }

    fn pair_cfg(&self, symbol: &str) -> Option<&Value> {
        self.pairs.get(&symbol.to_uppercase())
    }

    fn max_daily_loss_pct(&self) -> f64 {
        if let Some(profile) = self.profile_cfg() {
            return cfg_f64(profile, &["trading_parameters", "max_daily_loss_pct"], 0.03);
        }
        cfg_f64(&self.global, &["risk", "max_daily_loss_pct"], 0.03)
    }

    fn max_consecutive_losses(&self) -> i64 {
        if let Some(profile) = self.profile_cfg() {
            return cfg_i64(profile, &["circuit_breaker", "consecutive_losses_limit"], 3);
        }
        cfg_i64(&self.global, &["risk", "max_consecutive_losses"], 3)
    }

    fn risk_per_trade_fraction(&self) -> f64 {
        let pct = if let Some(profile) = self.profile_cfg() {
            cfg_f64(profile, &["trading_parameters", "risk_per_trade_pct"], 1.0)
        } else {
            1.0
        };
        if pct > 1.0 {
            pct / 100.0
        } else {
            pct
        }
    }

    fn max_leverage(&self) -> f64 {
        if let Some(profile) = self.profile_cfg() {
            return cfg_f64(profile, &["trading_parameters", "max_leverage"], 2.0);
        }
        2.0
    }

    fn min_position_usdt(&self) -> f64 {
        cfg_f64(&self.global, &["smart_copy", "min_position_usdt"], 5.0)
    }

    fn max_position_usdt(&self, symbol: &str) -> f64 {
        let global_max = cfg_f64(&self.global, &["smart_copy", "max_position_usdt"], 30.0);
        let pair_max = self
            .pair_cfg(symbol)
            .map(|v| cfg_f64(v, &["risk", "max_position_usdt"], global_max))
            .unwrap_or(global_max);
        pair_max.min(global_max)
    }

    fn atr_multiplier(&self, symbol: &str) -> f64 {
        self.pair_cfg(symbol)
            .map(|v| cfg_f64(v, &["risk", "atr_multiplier"], 1.0))
            .unwrap_or(1.0)
    }

    fn max_spread_pct(&self, symbol: &str) -> f64 {
        self.pair_cfg(symbol)
            .map(|v| cfg_f64(v, &["liquidity", "max_spread_pct"], 0.001))
            .unwrap_or_else(|| cfg_f64(&self.global, &["entry_filters", "max_spread_pct"], 0.001))
    }

    fn min_volume_24h_usdt(&self, symbol: &str) -> i64 {
        self.pair_cfg(symbol)
            .map(|v| cfg_i64(v, &["liquidity", "min_24h_volume_usdt"], 30_000_000))
            .unwrap_or_else(|| {
                cfg_i64(
                    &self.global,
                    &["entry_filters", "min_volume_24h_usdt"],
                    30_000_000,
                )
            })
    }

    fn max_funding_rate_pct(&self) -> f64 {
        cfg_f64(
            &self.global,
            &["entry_filters", "max_funding_rate_pct"],
            0.015,
        )
    }

    fn stop_loss_pct(&self, symbol: &str) -> f64 {
        if let Some(pair) = self.pair_cfg(symbol) {
            return cfg_f64(pair, &["risk", "stop_loss_pct"], 0.015);
        }
        if let Some(profile) = self.profile_cfg() {
            return cfg_f64(profile, &["trading_parameters", "stop_loss_pct"], 0.015);
        }
        0.015
    }

    fn take_profit_pct(&self, symbol: &str) -> f64 {
        if let Some(pair) = self.pair_cfg(symbol) {
            return cfg_f64(pair, &["risk", "take_profit_pct"], 0.03);
        }
        if let Some(profile) = self.profile_cfg() {
            return cfg_f64(profile, &["trading_parameters", "take_profit_pct"], 0.03);
        }
        0.03
    }
    fn trailing_config(&self, symbol: &str) -> Value {
        if let Some(pair) = self.pair_cfg(symbol) {
            if let Some(by_profile) = cfg_get(pair, &["trailing_stop", "by_profile", &self.profile])
            {
                return by_profile.clone();
            }
        }
        if let Some(profile) = self.profile_cfg() {
            if let Some(ts) = cfg_get(profile, &["trailing_stop"]) {
                return ts.clone();
            }
        }
        json!({
            "activate_after_profit_pct": 0.015,
            "initial_trail_pct": 0.008,
            "ratchet_levels": [],
            "move_to_break_even_at": 0.02
        })
    }
}

fn cfg_get<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = value;
    for part in path {
        cur = cur.get(*part)?;
    }
    Some(cur)
}

fn cfg_f64(value: &Value, path: &[&str], default: f64) -> f64 {
    cfg_get(value, path)
        .and_then(Value::as_f64)
        .unwrap_or(default)
}

fn cfg_i64(value: &Value, path: &[&str], default: i64) -> i64 {
    cfg_get(value, path)
        .and_then(Value::as_i64)
        .unwrap_or(default)
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

fn execute_strategy_step(
    step_name: &str,
    state: Value,
    cfg: &StrategyConfig,
) -> Result<Value, String> {
    let symbol = get_string(&state, "symbol", "UNKNOWN");

    match step_name {
        "check_daily_loss" => {
            let current_daily_loss = get_f64(&state, "current_daily_loss", 0.0);
            Ok(json!(current_daily_loss <= cfg.max_daily_loss_pct()))
        }
        "check_consecutive_losses" => {
            let losses = get_i64(&state, "consecutive_losses", 0);
            Ok(json!(losses <= cfg.max_consecutive_losses()))
        }
        "validate_entry" => {
            let spread_pct = get_f64(&state, "spread_pct", 1.0);
            let volume_24h = get_i64(&state, "volume_24h", 0);
            let trend_score = get_f64(&state, "trend_score", 0.0).abs();
            Ok(json!(
                spread_pct <= cfg.max_spread_pct(&symbol)
                    && volume_24h >= cfg.min_volume_24h_usdt(&symbol)
                    && trend_score >= 0.15
            ))
        }
        "check_funding" => {
            let funding_rate = get_f64(&state, "funding_rate", 0.0).abs();
            Ok(json!(funding_rate <= cfg.max_funding_rate_pct()))
        }
        "calc_smart_size" => {
            let price = get_f64(&state, "current_price", 0.0);
            if price <= 0.0 {
                return Ok(json!(0.0));
            }

            let equity_usdt = get_f64(&state, "account_equity_usdt", 1_000.0);
            let atr_14 = get_f64(&state, "atr_14", 0.0);
            let volatility_discount =
                (1.0 - (atr_14 * cfg.atr_multiplier(&symbol) / price)).clamp(0.2, 1.0);

            let desired_usdt = (equity_usdt * cfg.risk_per_trade_fraction() * volatility_discount)
                .clamp(cfg.min_position_usdt(), cfg.max_position_usdt(&symbol));

            Ok(json!(desired_usdt / price))
        }
        "validate_size" => {
            let quantity = get_f64(&state, "calc_smart_size", 0.0);
            let price = get_f64(&state, "current_price", 0.0);
            let position_usdt = quantity * price;
            Ok(json!(
                position_usdt >= cfg.min_position_usdt()
                    && position_usdt <= cfg.max_position_usdt(&symbol)
            ))
        }
        "get_trailing_config" => Ok(cfg.trailing_config(&symbol)),
        "decision" => {
            let can_enter = get_bool(&state, "check_daily_loss", false)
                && get_bool(&state, "check_consecutive_losses", false)
                && get_bool(&state, "validate_entry", false)
                && get_bool(&state, "check_funding", false)
                && get_bool(&state, "validate_size", false);

            let entry_price = get_f64(&state, "current_price", 0.0);
            let quantity = get_f64(&state, "calc_smart_size", 0.0);
            let trend = get_f64(&state, "trend_score", 0.0);

            if can_enter && quantity > 0.0 && entry_price > 0.0 {
                let is_long = trend >= 0.0;
                let sl_pct = cfg.stop_loss_pct(&symbol);
                let tp_pct = cfg.take_profit_pct(&symbol);

                let stop_loss = if is_long {
                    entry_price * (1.0 - sl_pct)
                } else {
                    entry_price * (1.0 + sl_pct)
                };
                let take_profit = if is_long {
                    entry_price * (1.0 + tp_pct)
                } else {
                    entry_price * (1.0 - tp_pct)
                };

                Ok(json!({
                    "action": if is_long { "ENTER_LONG" } else { "ENTER_SHORT" },
                    "symbol": symbol,
                    "quantity": quantity,
                    "leverage": cfg.max_leverage(),
                    "entry_price": entry_price,
                    "stop_loss": stop_loss,
                    "take_profit": take_profit,
                    "reason": "in_process_runtime_profiled",
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

fn register_strategy_steps(runtime: &Runtime, plan: &ExecutionPlan, cfg: Arc<StrategyConfig>) {
    for step in &plan.steps {
        let function_ref = step.function_ref.clone();
        let fallback_step_name = step.name.clone();
        let step_name = function_ref
            .split("::step_")
            .nth(1)
            .unwrap_or(&fallback_step_name)
            .to_string();
        let cfg_for_step = Arc::clone(&cfg);

        runtime.register_step(&function_ref, move |state| {
            execute_strategy_step(&step_name, state, cfg_for_step.as_ref())
        });
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

    let pipeline_path = std::env::var("TUPA_PIPELINE_PATH")
        .or_else(|_| std::env::var("VIPER_PIPELINE_PATH"))
        .unwrap_or_else(|_| "config/strategies/viper_smart_copy.tp".to_string());
    let strategy_config_path = std::env::var("STRATEGY_CONFIG")
        .unwrap_or_else(|_| "config/trading/pairs.yaml".to_string());
    let profile_config_path = std::env::var("PROFILE_CONFIG")
        .unwrap_or_else(|_| "config/system/profiles.yaml".to_string());
    let trading_profile = std::env::var("TRADING_PROFILE").unwrap_or_else(|_| "MEDIUM".to_string());

    let cfg = Arc::new(StrategyConfig::from_files(
        &strategy_config_path,
        &profile_config_path,
        &trading_profile,
    )?);

    let execution_plan = load_execution_plan(&pipeline_path)?;

    let runtime = Runtime::new();
    register_strategy_steps(&runtime, &execution_plan, Arc::clone(&cfg));
    println!(
        "Loaded in-process plan '{}' with {} step(s) and profile {}",
        execution_plan.name,
        execution_plan.steps.len(),
        cfg.profile
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

        let signal_event: MarketSignalEvent = match serde_json::from_str(&payload) {
            Ok(evt) => evt,
            Err(_) => {
                let legacy_signal: MarketSignal = match serde_json::from_str(&payload) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to parse market signal event: {}", e);
                        continue;
                    }
                };
                MarketSignalEvent::new(legacy_signal)
            }
        };

        let input = serde_json::to_value(&signal_event.signal)?;
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
                let decision_event =
                    StrategyDecisionEvent::new(signal_event.event_id.clone(), decision);
                let decision_json = serde_json::to_string(&decision_event)?;
                publish_conn
                    .publish::<_, _, ()>("viper:decisions", decision_json)
                    .await?;
                println!(
                    "Published decision event {} for {}",
                    decision_event.event_id, signal_event.signal.symbol
                );
            }
            Err(_) => {
                let decision_json = serde_json::to_string(&decision_value)?;
                publish_conn
                    .publish::<_, _, ()>("viper:decisions", decision_json)
                    .await?;
                println!(
                    "Published decision (raw) for {}",
                    signal_event.signal.symbol
                );
            }
        }
    }

    Ok(())
}
