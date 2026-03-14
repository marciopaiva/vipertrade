use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use redis::AsyncCommands;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::{watch, RwLock};
use tupa_codegen::execution_plan::{codegen_pipeline, ExecutionPlan};
use tupa_parser::{parse_program, Item, PipelineDecl, Program};
use tupa_runtime::Runtime;
use tupa_typecheck::typecheck_program;
use viper_domain::{MarketSignal, MarketSignalEvent, StrategyDecision, StrategyDecisionEvent};

#[derive(Debug, Clone)]
struct StrategyConfig {
    profile: String,
    trading_mode: String,
    global: Value,
    pairs: HashMap<String, Value>,
    profiles: Value,
}

#[derive(Debug, Clone)]
struct RatchetLevel {
    at_profit_pct: f64,
    trail_pct: f64,
}

#[derive(Debug, Clone)]
struct TrailingRuntimeConfig {
    enabled: bool,
    activate_after_profit_pct: f64,
    initial_trail_pct: f64,
    ratchet_levels: Vec<RatchetLevel>,
    move_to_break_even_at: f64,
    min_move_threshold_pct: f64,
}

#[derive(Debug, Clone)]
struct OpenTradeSnapshot {
    trade_id: String,
    side: String,
    quantity: f64,
    entry_price: f64,
    opened_at: DateTime<Utc>,
    trailing_stop_activated: bool,
    trailing_stop_peak_price: f64,
    trailing_stop_final_distance_pct: f64,
}

#[derive(Debug, Clone)]
struct TrailingEval {
    activated: bool,
    peak_price: f64,
    trail_pct: f64,
    trailing_stop_price: f64,
}

#[derive(Debug, Clone)]
struct EntryGuardState {
    blocked_side: String,
    cooldown_until: Instant,
    awaiting_flip: bool,
}

#[derive(Debug, Clone)]
struct SignalConfirmationState {
    side: String,
    consecutive_valid_ticks: usize,
}

#[derive(Debug, Clone)]
struct InvalidSignalDrop {
    symbol: String,
    stage: String,
    reason: String,
    timestamp: String,
}

#[derive(Debug, serde::Deserialize)]
struct WalletSizingResponse {
    total_equity: Option<f64>,
    margin_balance: Option<f64>,
    wallet_balance: Option<f64>,
    available_balance: Option<f64>,
}

impl StrategyConfig {
    fn from_files(
        pairs_path: &str,
        profiles_path: &str,
        profile: &str,
        trading_mode: &str,
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
            trading_mode: trading_mode.to_uppercase(),
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

    fn pair_mode_cfg(&self, symbol: &str) -> Option<&Value> {
        self.pair_cfg(symbol)
            .and_then(|value| cfg_get(value, &["mode_profiles", self.trading_mode.as_str()]))
    }

    fn mode_cfg(&self) -> Option<&Value> {
        cfg_get(&self.global, &["mode_profiles", self.trading_mode.as_str()])
    }

    fn mode_flag(&self, key: &str, default: bool) -> bool {
        self.mode_cfg()
            .and_then(|value| cfg_get(value, &[key]))
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }

    fn mode_f64(&self, key: &str) -> Option<f64> {
        self.mode_cfg()
            .and_then(|value| cfg_get(value, &[key]))
            .and_then(Value::as_f64)
    }

    fn mode_i64(&self, key: &str) -> Option<i64> {
        self.mode_cfg()
            .and_then(|value| cfg_get(value, &[key]))
            .and_then(Value::as_i64)
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
        let mode_pair_max = self
            .pair_mode_cfg(symbol)
            .and_then(|v| cfg_get(v, &["risk", "max_position_usdt"]))
            .and_then(Value::as_f64);
        let pair_max = self
            .pair_cfg(symbol)
            .map(|v| cfg_f64(v, &["risk", "max_position_usdt"], global_max))
            .unwrap_or(global_max);
        mode_pair_max.unwrap_or(pair_max).min(global_max)
    }

    fn max_position_wallet_pct(&self, symbol: &str) -> Option<f64> {
        let mode_pair_pct = self
            .pair_mode_cfg(symbol)
            .and_then(|v| cfg_get(v, &["risk", "max_position_wallet_pct"]))
            .and_then(Value::as_f64);
        let pair_pct = self
            .pair_cfg(symbol)
            .and_then(|v| cfg_get(v, &["risk", "max_position_wallet_pct"]))
            .and_then(Value::as_f64);
        mode_pair_pct.or(pair_pct)
    }

    fn atr_multiplier(&self, symbol: &str) -> f64 {
        self.pair_cfg(symbol)
            .map(|v| cfg_f64(v, &["risk", "atr_multiplier"], 1.0))
            .unwrap_or(1.0)
    }

    fn max_position_cap_usdt(&self, symbol: &str, equity_usdt: f64) -> f64 {
        self.max_position_wallet_pct(symbol)
            .map(|pct| equity_usdt * pct)
            .unwrap_or_else(|| self.max_position_usdt(symbol))
    }

    fn max_spread_pct(&self, symbol: &str) -> f64 {
        self.mode_f64("max_spread_pct").unwrap_or_else(|| {
            self.pair_cfg(symbol)
                .map(|v| cfg_f64(v, &["liquidity", "max_spread_pct"], 0.001))
                .unwrap_or_else(|| {
                    cfg_f64(&self.global, &["entry_filters", "max_spread_pct"], 0.001)
                })
        })
    }

    fn max_atr_pct(&self, symbol: &str) -> f64 {
        self.mode_f64("max_atr_pct").unwrap_or_else(|| {
            self.pair_cfg(symbol)
                .map(|v| cfg_f64(v, &["entry_filters", "max_atr_pct"], 0.05))
                .unwrap_or_else(|| cfg_f64(&self.global, &["entry_filters", "max_atr_pct"], 0.05))
        })
    }

    fn min_trend_score_for_side(&self, symbol: &str, side: &str) -> f64 {
        let side_key = if side.eq_ignore_ascii_case("short") {
            "min_trend_score_short"
        } else {
            "min_trend_score_long"
        };

        if let Some(value) = self.mode_f64(side_key) {
            return value;
        }

        self.pair_cfg(symbol)
            .map(|v| {
                cfg_f64(
                    v,
                    &["entry_filters", side_key],
                    cfg_f64(v, &["entry_filters", "min_trend_score"], 0.25),
                )
            })
            .unwrap_or_else(|| {
                cfg_f64(
                    &self.global,
                    &["entry_filters", side_key],
                    cfg_f64(&self.global, &["entry_filters", "min_trend_score"], 0.25),
                )
            })
    }

    fn allow_long(&self, symbol: &str) -> bool {
        self.pair_cfg(symbol)
            .map(|v| cfg_bool(v, &["entry_filters", "allow_long"], true))
            .unwrap_or(true)
    }

    fn allow_short(&self, symbol: &str) -> bool {
        self.pair_cfg(symbol)
            .map(|v| cfg_bool(v, &["entry_filters", "allow_short"], true))
            .unwrap_or(true)
    }

    fn min_signal_confirmation_ticks(&self, symbol: &str) -> usize {
        self.pair_cfg(symbol)
            .and_then(|v| cfg_get(v, &["entry_filters", "min_signal_confirmation_ticks"]))
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or_else(|| {
                cfg_get(
                    &self.global,
                    &["entry_filters", "min_signal_confirmation_ticks"],
                )
                .and_then(Value::as_u64)
                .map(|v| v as usize)
                .unwrap_or(2)
            })
    }

    fn min_signal_confirmation_ticks_for_side(&self, symbol: &str, side: &str) -> usize {
        let side_key = if side.eq_ignore_ascii_case("short") {
            "min_signal_confirmation_ticks_short"
        } else {
            "min_signal_confirmation_ticks_long"
        };

        if let Some(value) = self.mode_i64(side_key) {
            return value.max(1) as usize;
        }

        self.pair_cfg(symbol)
            .and_then(|v| cfg_get(v, &["entry_filters", side_key]))
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or_else(|| {
                cfg_get(&self.global, &["entry_filters", side_key])
                    .and_then(Value::as_u64)
                    .map(|v| v as usize)
                    .unwrap_or_else(|| self.min_signal_confirmation_ticks(symbol))
            })
    }

    fn stop_loss_cooldown_minutes_for_side(&self, symbol: &str, side: &str) -> i64 {
        let side_key = if side.eq_ignore_ascii_case("short") {
            "stop_loss_cooldown_minutes_short"
        } else {
            "stop_loss_cooldown_minutes_long"
        };

        if let Some(value) = self.mode_i64(side_key) {
            return value.max(0);
        }

        self.pair_cfg(symbol)
            .map(|v| {
                cfg_i64(
                    v,
                    &["entry_filters", side_key],
                    cfg_i64(v, &["entry_filters", "stop_loss_cooldown_minutes"], 3),
                )
            })
            .unwrap_or_else(|| {
                cfg_i64(
                    &self.global,
                    &["entry_filters", side_key],
                    cfg_i64(
                        &self.global,
                        &["entry_filters", "stop_loss_cooldown_minutes"],
                        3,
                    ),
                )
            })
    }

    fn min_volume_ratio_for_side(&self, symbol: &str, side: &str) -> f64 {
        let side_key = if side.eq_ignore_ascii_case("short") {
            "min_volume_ratio_short"
        } else {
            "min_volume_ratio_long"
        };

        if let Some(value) = self.mode_f64(side_key) {
            return value;
        }

        self.pair_cfg(symbol)
            .map(|v| {
                cfg_f64(
                    v,
                    &["entry_filters", side_key],
                    cfg_f64(v, &["entry_filters", "min_volume_ratio"], 1.0),
                )
            })
            .unwrap_or_else(|| {
                cfg_f64(
                    &self.global,
                    &["entry_filters", side_key],
                    cfg_f64(&self.global, &["entry_filters", "min_volume_ratio"], 1.0),
                )
            })
    }

    fn rsi_bounds_for_side(&self, symbol: &str, side: &str) -> (f64, f64) {
        let (min_key, max_key, default_min, default_max) = if side.eq_ignore_ascii_case("short") {
            ("rsi_short_min", "rsi_short_max", 32.0, 50.0)
        } else {
            ("rsi_long_min", "rsi_long_max", 50.0, 68.0)
        };

        if let (Some(min_value), Some(max_value)) = (self.mode_f64(min_key), self.mode_f64(max_key))
        {
            return (min_value, max_value);
        }

        let min_value = self
            .pair_cfg(symbol)
            .map(|v| {
                cfg_f64(
                    v,
                    &["entry_filters", min_key],
                    cfg_f64(&self.global, &["entry_filters", min_key], default_min),
                )
            })
            .unwrap_or_else(|| cfg_f64(&self.global, &["entry_filters", min_key], default_min));
        let max_value = self
            .pair_cfg(symbol)
            .map(|v| {
                cfg_f64(
                    v,
                    &["entry_filters", max_key],
                    cfg_f64(&self.global, &["entry_filters", max_key], default_max),
                )
            })
            .unwrap_or_else(|| cfg_f64(&self.global, &["entry_filters", max_key], default_max));
        (min_value, max_value)
    }

    fn btc_macro_penalty_for_side(
        &self,
        symbol: &str,
        side: &str,
        btc_regime: &str,
        btc_trend_score: f64,
        btc_consensus_count: i64,
    ) -> Option<f64> {
        if symbol.eq_ignore_ascii_case("BTCUSDT") {
            return Some(0.0);
        }

        let aligned = if side.eq_ignore_ascii_case("short") {
            btc_regime.eq_ignore_ascii_case("bearish")
                && btc_trend_score <= -0.05
                && btc_consensus_count >= 2
        } else {
            btc_regime.eq_ignore_ascii_case("bullish")
                && btc_trend_score >= 0.05
                && btc_consensus_count >= 2
        };

        if aligned {
            return Some(0.0);
        }

        let neutral = btc_regime.eq_ignore_ascii_case("neutral") && btc_consensus_count >= 2;
        if neutral {
            return Some(0.05);
        }

        None
    }

    fn min_volume_24h_usdt(&self, symbol: &str) -> i64 {
        self.mode_i64("min_volume_24h_usdt").unwrap_or_else(|| {
            self.pair_cfg(symbol)
                .map(|v| cfg_i64(v, &["liquidity", "min_24h_volume_usdt"], 30_000_000))
                .unwrap_or_else(|| {
                    cfg_i64(
                        &self.global,
                        &["entry_filters", "min_volume_24h_usdt"],
                        30_000_000,
                    )
                })
        })
    }

    fn max_funding_rate_pct(&self) -> f64 {
        self.mode_f64("max_funding_rate_pct").unwrap_or_else(|| {
            cfg_f64(
                &self.global,
                &["entry_filters", "max_funding_rate_pct"],
                0.015,
            )
        })
    }

    fn require_multi_exchange_consensus(&self) -> bool {
        self.mode_flag("require_multi_exchange_consensus", true)
    }

    fn require_btc_macro_alignment(&self) -> bool {
        self.mode_flag("require_btc_macro_alignment", true)
    }

    fn permissive_entry(&self) -> bool {
        self.mode_flag("permissive_entry", false)
    }

    fn min_hold_seconds(&self) -> i64 {
        self.mode_i64("min_hold_seconds").unwrap_or(0).max(0)
    }

    fn stop_loss_pct(&self, symbol: &str) -> f64 {
        if let Some(value) = self.mode_f64("stop_loss_pct") {
            return value;
        }
        if let Some(pair) = self.pair_cfg(symbol) {
            return cfg_f64(pair, &["risk", "stop_loss_pct"], 0.015);
        }
        if let Some(profile) = self.profile_cfg() {
            return cfg_f64(profile, &["trading_parameters", "stop_loss_pct"], 0.015);
        }
        0.015
    }

    fn take_profit_pct(&self, symbol: &str) -> f64 {
        if let Some(value) = self.mode_f64("take_profit_pct") {
            return value;
        }
        if let Some(pair) = self.pair_cfg(symbol) {
            return cfg_f64(pair, &["risk", "take_profit_pct"], 0.03);
        }
        if let Some(profile) = self.profile_cfg() {
            return cfg_f64(profile, &["trading_parameters", "take_profit_pct"], 0.03);
        }
        0.03
    }
    fn trailing_config(&self, symbol: &str) -> Value {
        if let Some(mode_cfg) = self.mode_cfg().and_then(|v| cfg_get(v, &["trailing_stop"])) {
            return mode_cfg.clone();
        }
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

    fn trailing_enabled(&self, symbol: &str) -> bool {
        if let Some(enabled) = self
            .mode_cfg()
            .and_then(|v| cfg_get(v, &["trailing_enabled"]))
            .and_then(Value::as_bool)
        {
            return enabled;
        }
        let pair_enabled = self
            .pair_cfg(symbol)
            .and_then(|v| cfg_get(v, &["trailing_stop", "enabled"]))
            .and_then(Value::as_bool);
        pair_enabled.unwrap_or_else(|| {
            cfg_get(&self.global, &["trailing_stop", "enabled"])
                .and_then(Value::as_bool)
                .unwrap_or(true)
        })
    }

    fn fixed_take_profit_enabled(&self) -> bool {
        self.mode_cfg()
            .and_then(|v| cfg_get(v, &["fixed_take_profit_enabled"]))
            .and_then(Value::as_bool)
            .unwrap_or(true)
    }

    fn trailing_min_move_threshold_pct(&self) -> f64 {
        cfg_f64(
            &self.global,
            &["trailing_stop", "min_move_threshold_pct"],
            0.002,
        )
    }

    fn trailing_runtime_config(&self, symbol: &str) -> TrailingRuntimeConfig {
        let cfg = self.trailing_config(symbol);
        let mut ratchet_levels = cfg
            .get("ratchet_levels")
            .and_then(Value::as_array)
            .map(|levels| {
                levels
                    .iter()
                    .filter_map(|level| {
                        let at_profit_pct = level.get("at_profit_pct")?.as_f64()?;
                        let trail_pct = level.get("trail_pct")?.as_f64()?;
                        Some(RatchetLevel {
                            at_profit_pct,
                            trail_pct,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ratchet_levels.sort_by(|a, b| {
            a.at_profit_pct
                .partial_cmp(&b.at_profit_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        TrailingRuntimeConfig {
            enabled: self.trailing_enabled(symbol),
            activate_after_profit_pct: cfg
                .get("activate_after_profit_pct")
                .and_then(Value::as_f64)
                .unwrap_or(0.015),
            initial_trail_pct: cfg
                .get("initial_trail_pct")
                .and_then(Value::as_f64)
                .unwrap_or(0.008),
            ratchet_levels,
            move_to_break_even_at: cfg
                .get("move_to_break_even_at")
                .and_then(Value::as_f64)
                .unwrap_or(0.02),
            min_move_threshold_pct: self.trailing_min_move_threshold_pct(),
        }
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

fn cfg_bool(value: &Value, path: &[&str], default: bool) -> bool {
    cfg_get(value, path)
        .and_then(Value::as_bool)
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

fn side_from_trend(trend: f64) -> &'static str {
    if trend >= 0.0 {
        "Long"
    } else {
        "Short"
    }
}

fn is_same_direction(side: &str, trend: f64) -> bool {
    side.eq_ignore_ascii_case(side_from_trend(trend))
}

fn resolve_database_url() -> Option<String> {
    if let Ok(v) = std::env::var("DATABASE_URL") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }

    let host = std::env::var("DB_HOST").ok()?;
    let port = std::env::var("DB_PORT")
        .ok()
        .unwrap_or_else(|| "5432".to_string());
    let db = std::env::var("DB_NAME").ok()?;
    let user = std::env::var("DB_USER").ok()?;
    let pass = std::env::var("DB_PASSWORD").ok()?;

    Some(format!(
        "postgresql://{}:{}@{}:{}/{}",
        user, pass, host, port, db
    ))
}

fn resolve_wallet_api_base_url() -> String {
    std::env::var("WALLET_API_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://api:8080".to_string())
}

async fn fetch_account_equity_usdt(
    http: &reqwest::Client,
    wallet_api_base_url: &str,
    fallback_equity_usdt: f64,
) -> f64 {
    let url = format!(
        "{}/api/v1/external/bybit-wallet",
        wallet_api_base_url.trim_end_matches('/')
    );
    match http.get(url).send().await {
        Ok(response) => match response.json::<WalletSizingResponse>().await {
            Ok(body) => body
                .total_equity
                .or(body.margin_balance)
                .or(body.wallet_balance)
                .or(body.available_balance)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(fallback_equity_usdt),
            Err(_) => fallback_equity_usdt,
        },
        Err(_) => fallback_equity_usdt,
    }
}

async fn fetch_open_trade_for_symbol(
    pool: &PgPool,
    symbol: &str,
) -> Result<Option<OpenTradeSnapshot>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, f64, f64, DateTime<Utc>, bool, f64, f64)>(
        "SELECT
            trade_id::text,
            side,
            quantity::double precision,
            entry_price::double precision,
            opened_at,
            COALESCE(trailing_stop_activated, false),
            COALESCE(trailing_stop_peak_price::double precision, entry_price::double precision),
            COALESCE(trailing_stop_final_distance_pct::double precision, 0)
        FROM trades
        WHERE status = 'open' AND symbol = $1
        ORDER BY opened_at ASC
        LIMIT 1",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            trade_id,
            side,
            quantity,
            entry_price,
            opened_at,
            trailing_stop_activated,
            trailing_stop_peak_price,
            trailing_stop_final_distance_pct,
        )| OpenTradeSnapshot {
            trade_id,
            side,
            quantity,
            entry_price,
            opened_at,
            trailing_stop_activated,
            trailing_stop_peak_price,
            trailing_stop_final_distance_pct,
        },
    ))
}

async fn update_trade_trailing_state(
    pool: &PgPool,
    trade_id: &str,
    activated: bool,
    peak_price: f64,
    trail_pct: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE trades
         SET trailing_stop_activated = $2,
             trailing_stop_peak_price = $3,
             trailing_stop_final_distance_pct = $4
         WHERE trade_id::text = $1",
    )
    .bind(trade_id)
    .bind(activated)
    .bind(peak_price)
    .bind(trail_pct)
    .execute(pool)
    .await?;

    Ok(())
}

async fn has_recent_stop_loss_for_symbol(
    pool: &PgPool,
    symbol: &str,
    cooldown_minutes: i64,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM trades
         WHERE symbol = $1
           AND status = 'closed'
           AND close_reason = 'stop_loss'
           AND closed_at >= NOW() - make_interval(mins => $2::int)",
    )
    .bind(symbol)
    .bind(cooldown_minutes)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

fn create_hold_decision(symbol: &str, reason: &str) -> StrategyDecision {
    StrategyDecision {
        action: "HOLD".to_string(),
        symbol: symbol.to_string(),
        quantity: 0.0,
        leverage: 0.0,
        entry_price: 0.0,
        stop_loss: 0.0,
        take_profit: 0.0,
        reason: reason.to_string(),
        smart_copy_compatible: false,
    }
}

fn create_close_decision(
    symbol: &str,
    side: &str,
    quantity: f64,
    close_price: f64,
    reason: &str,
) -> Option<StrategyDecision> {
    let action = match side {
        "Long" => "CLOSE_LONG",
        "Short" => "CLOSE_SHORT",
        _ => return None,
    };

    Some(StrategyDecision {
        action: action.to_string(),
        symbol: symbol.to_string(),
        quantity,
        leverage: 0.0,
        entry_price: close_price,
        stop_loss: 0.0,
        take_profit: 0.0,
        reason: reason.to_string(),
        smart_copy_compatible: true,
    })
}

fn current_profit_pct(side: &str, entry: f64, current: f64) -> f64 {
    if entry <= 0.0 || current <= 0.0 {
        return 0.0;
    }
    if side == "Long" {
        (current - entry) / entry
    } else {
        (entry - current) / entry
    }
}

fn evaluate_trailing(
    open: &OpenTradeSnapshot,
    current_price: f64,
    trailing: &TrailingRuntimeConfig,
) -> Option<TrailingEval> {
    if !trailing.enabled || current_price <= 0.0 || open.entry_price <= 0.0 {
        return None;
    }

    let profit_pct = current_profit_pct(&open.side, open.entry_price, current_price);
    let mut activated =
        open.trailing_stop_activated || profit_pct >= trailing.activate_after_profit_pct;
    if !activated {
        return None;
    }

    let mut peak_price = if open.trailing_stop_peak_price > 0.0 {
        open.trailing_stop_peak_price
    } else {
        open.entry_price
    };

    if open.side == "Long" {
        peak_price = peak_price.max(current_price);
    } else {
        peak_price = peak_price.min(current_price);
    }

    let mut trail_pct = trailing.initial_trail_pct;
    for level in &trailing.ratchet_levels {
        if profit_pct >= level.at_profit_pct {
            trail_pct = level.trail_pct;
        }
    }

    // Preserve ratcheted progress already persisted for this trade.
    if open.trailing_stop_final_distance_pct > 0.0 {
        trail_pct = trail_pct.max(open.trailing_stop_final_distance_pct);
    }

    let mut trailing_stop_price = if open.side == "Long" {
        peak_price * (1.0 - trail_pct)
    } else {
        peak_price * (1.0 + trail_pct)
    };

    if profit_pct >= trailing.move_to_break_even_at {
        if open.side == "Long" {
            trailing_stop_price = trailing_stop_price.max(open.entry_price);
        } else {
            trailing_stop_price = trailing_stop_price.min(open.entry_price);
        }
    }

    activated = true;
    Some(TrailingEval {
        activated,
        peak_price,
        trail_pct,
        trailing_stop_price,
    })
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
            let raw_trend_score = get_f64(&state, "trend_score", 0.0);
            let consensus_raw_trend_score =
                get_f64(&state, "consensus_trend_score", raw_trend_score);
            let consensus_trend_score = consensus_raw_trend_score.abs();
            let current_price = get_f64(&state, "current_price", 0.0);
            let atr_14 = get_f64(&state, "atr_14", 0.0);
            let trend_slope = get_f64(&state, "trend_slope", 0.0);
            let consensus_trend_slope = get_f64(&state, "consensus_trend_slope", trend_slope);
            let ema_fast = get_f64(&state, "ema_fast", 0.0);
            let ema_slow = get_f64(&state, "ema_slow", 0.0);
            let consensus_ema_fast = get_f64(&state, "consensus_ema_fast", ema_fast);
            let consensus_ema_slow = get_f64(&state, "consensus_ema_slow", ema_slow);
            let rsi_14 = get_f64(&state, "rsi_14", 50.0);
            let consensus_rsi_14 = get_f64(&state, "consensus_rsi_14", rsi_14);
            let macd_line = get_f64(&state, "macd_line", 0.0);
            let macd_signal = get_f64(&state, "macd_signal", 0.0);
            let macd_histogram = get_f64(&state, "macd_histogram", 0.0);
            let consensus_macd_line = get_f64(&state, "consensus_macd_line", macd_line);
            let consensus_macd_signal = get_f64(&state, "consensus_macd_signal", macd_signal);
            let consensus_macd_histogram =
                get_f64(&state, "consensus_macd_histogram", macd_histogram);
            let volume_ratio = get_f64(&state, "volume_ratio", 0.0);
            let consensus_volume_ratio = get_f64(&state, "consensus_volume_ratio", volume_ratio);
            let btc_regime = get_string(&state, "btc_regime", "neutral");
            let btc_trend_score = get_f64(&state, "btc_trend_score", 0.0);
            let btc_consensus_count = get_i64(&state, "btc_consensus_count", 0);
            let regime = get_string(&state, "regime", "neutral");
            let exchanges_available = get_i64(&state, "exchanges_available", 0);
            let bybit_regime = get_string(&state, "bybit_regime", "neutral");
            let bullish_exchanges = get_i64(&state, "bullish_exchanges", 0);
            let bearish_exchanges = get_i64(&state, "bearish_exchanges", 0);
            let entry_side = if raw_trend_score >= 0.0 {
                "long"
            } else {
                "short"
            };
            let (rsi_min, rsi_max) = cfg.rsi_bounds_for_side(&symbol, entry_side);
            let btc_macro_penalty = if cfg.require_btc_macro_alignment() {
                let Some(penalty) = cfg.btc_macro_penalty_for_side(
                    &symbol,
                    entry_side,
                    &btc_regime,
                    btc_trend_score,
                    btc_consensus_count,
                ) else {
                    return Ok(json!(false));
                };
                penalty
            } else {
                0.0
            };
            let atr_pct = if current_price > 0.0 {
                atr_14 / current_price
            } else {
                1.0
            };
            let consensus_long_ok = if cfg.require_multi_exchange_consensus() {
                bullish_exchanges >= 2 && bearish_exchanges == 0 && exchanges_available >= 3
            } else {
                bybit_regime.eq_ignore_ascii_case("bullish")
                    || regime.eq_ignore_ascii_case("bullish")
            };
            let consensus_short_ok = if cfg.require_multi_exchange_consensus() {
                bearish_exchanges >= 2 && bullish_exchanges == 0 && exchanges_available >= 3
            } else {
                bybit_regime.eq_ignore_ascii_case("bearish")
                    || regime.eq_ignore_ascii_case("bearish")
            };
            let strict_long_ok = cfg.allow_long(&symbol)
                && regime.eq_ignore_ascii_case("bullish")
                && bybit_regime.eq_ignore_ascii_case("bullish")
                && consensus_long_ok
                && consensus_trend_slope > 0.0
                && consensus_ema_fast > consensus_ema_slow
                && current_price >= ema_fast
                && consensus_rsi_14 >= rsi_min
                && consensus_rsi_14 <= rsi_max
                && consensus_macd_line > consensus_macd_signal
                && consensus_macd_histogram > 0.0
                && consensus_volume_ratio >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                && consensus_trend_score
                    >= (cfg.min_trend_score_for_side(&symbol, entry_side) + btc_macro_penalty);

            let directional_ok = if raw_trend_score >= 0.0 {
                cfg.allow_long(&symbol)
                    && if cfg.permissive_entry() {
                        (bybit_regime.eq_ignore_ascii_case("bullish")
                            || regime.eq_ignore_ascii_case("bullish")
                            || consensus_raw_trend_score >= 0.0)
                            && consensus_trend_slope >= 0.0
                            && consensus_ema_fast >= consensus_ema_slow
                            && current_price > 0.0
                            && current_price >= ema_slow
                            && consensus_rsi_14 >= rsi_min
                            && consensus_rsi_14 <= rsi_max
                            && consensus_macd_line >= consensus_macd_signal
                            && consensus_macd_histogram >= 0.0
                            && consensus_volume_ratio
                                >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                            && consensus_trend_score
                                >= (cfg.min_trend_score_for_side(&symbol, entry_side)
                                    + btc_macro_penalty)
                    } else {
                        strict_long_ok
                    }
            } else {
                cfg.allow_short(&symbol)
                    && if cfg.permissive_entry() {
                        (bybit_regime.eq_ignore_ascii_case("bearish")
                            || regime.eq_ignore_ascii_case("bearish")
                            || consensus_raw_trend_score < 0.0)
                            && consensus_trend_slope <= 0.0
                            && consensus_ema_fast <= consensus_ema_slow
                            && current_price > 0.0
                            && current_price <= ema_slow
                            && consensus_rsi_14 >= rsi_min
                            && consensus_rsi_14 <= rsi_max
                            && consensus_macd_line <= consensus_macd_signal
                            && consensus_macd_histogram <= 0.0
                            && consensus_volume_ratio
                                >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                            && consensus_trend_score
                                >= (cfg.min_trend_score_for_side(&symbol, entry_side)
                                    + btc_macro_penalty)
                    } else {
                        regime.eq_ignore_ascii_case("bearish")
                            && bybit_regime.eq_ignore_ascii_case("bearish")
                            && consensus_short_ok
                            && consensus_trend_slope < 0.0
                            && consensus_ema_fast < consensus_ema_slow
                            && current_price <= ema_fast
                            && consensus_rsi_14 >= rsi_min
                            && consensus_rsi_14 <= rsi_max
                            && consensus_macd_line < consensus_macd_signal
                            && consensus_macd_histogram < 0.0
                            && consensus_volume_ratio
                                >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                            && consensus_trend_score
                                >= (cfg.min_trend_score_for_side(&symbol, entry_side)
                                    + btc_macro_penalty)
                    }
            };
            Ok(json!(
                spread_pct <= cfg.max_spread_pct(&symbol)
                    && volume_24h >= cfg.min_volume_24h_usdt(&symbol)
                    && atr_pct <= cfg.max_atr_pct(&symbol)
                    && directional_ok
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
            let stop_loss_pct = cfg.stop_loss_pct(&symbol).max(0.0001);
            let risk_budget_usdt = equity_usdt * cfg.risk_per_trade_fraction();
            let risk_sized_notional = (risk_budget_usdt / stop_loss_pct) * volatility_discount;
            let max_position_usdt = cfg.max_position_cap_usdt(&symbol, equity_usdt);
            let desired_usdt = risk_sized_notional.clamp(
                cfg.min_position_usdt(),
                max_position_usdt.max(cfg.min_position_usdt()),
            );

            Ok(json!(desired_usdt / price))
        }
        "validate_size" => {
            let quantity = get_f64(&state, "calc_smart_size", 0.0);
            let price = get_f64(&state, "current_price", 0.0);
            let equity_usdt = get_f64(&state, "account_equity_usdt", 1_000.0);
            let position_usdt = quantity * price;
            Ok(json!(
                position_usdt >= cfg.min_position_usdt()
                    && position_usdt <= cfg.max_position_cap_usdt(&symbol, equity_usdt)
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

async fn publish_decision_event(
    publish_conn: &mut redis::aio::MultiplexedConnection,
    source_event_id: &str,
    decision: StrategyDecision,
) -> Result<(), Box<dyn Error>> {
    let decision_event = StrategyDecisionEvent::new(source_event_id.to_string(), decision);
    decision_event.validate()?;

    let decision_json = serde_json::to_string(&decision_event)?;
    publish_conn
        .publish::<_, _, ()>("viper:decisions", decision_json)
        .await?;

    println!(
        "Published decision event {} for {} action={}",
        decision_event.event_id, decision_event.decision.symbol, decision_event.decision.action
    );
    Ok(())
}

fn should_persist_trailing_update(
    open: &OpenTradeSnapshot,
    eval: &TrailingEval,
    min_move_threshold_pct: f64,
) -> bool {
    if open.trailing_stop_activated != eval.activated {
        return true;
    }

    let peak_base = open.trailing_stop_peak_price.abs().max(1e-9);
    let peak_move_pct = (eval.peak_price - open.trailing_stop_peak_price).abs() / peak_base;
    if peak_move_pct >= min_move_threshold_pct {
        return true;
    }

    (eval.trail_pct - open.trailing_stop_final_distance_pct).abs() >= 1e-9
}

fn evaluate_open_trade_exit(
    symbol: &str,
    current_price: f64,
    open: &OpenTradeSnapshot,
    cfg: &StrategyConfig,
) -> (Option<StrategyDecision>, Option<TrailingEval>) {
    if current_price <= 0.0 || open.entry_price <= 0.0 {
        return (
            Some(create_hold_decision(symbol, "open_position_invalid_price")),
            None,
        );
    }

    let min_hold_seconds = cfg.min_hold_seconds();
    if min_hold_seconds > 0 {
        let held_for = Utc::now()
            .signed_duration_since(open.opened_at)
            .num_seconds()
            .max(0);
        if held_for < min_hold_seconds {
            return (
                Some(create_hold_decision(
                    symbol,
                    &format!("min_hold_{}s", min_hold_seconds),
                )),
                None,
            );
        }
    }

    let side = open.side.as_str();
    let sl_pct = cfg.stop_loss_pct(symbol);
    let hard_stop = if side == "Long" {
        open.entry_price * (1.0 - sl_pct)
    } else {
        open.entry_price * (1.0 + sl_pct)
    };

    if (side == "Long" && current_price <= hard_stop)
        || (side == "Short" && current_price >= hard_stop)
    {
        return (
            create_close_decision(
                symbol,
                side,
                open.quantity,
                current_price,
                "stop_loss_triggered",
            ),
            None,
        );
    }

    if cfg.fixed_take_profit_enabled() {
        let tp_pct = cfg.take_profit_pct(symbol);
        let fixed_take_profit = if side == "Long" {
            open.entry_price * (1.0 + tp_pct)
        } else {
            open.entry_price * (1.0 - tp_pct)
        };
        if (side == "Long" && current_price >= fixed_take_profit)
            || (side == "Short" && current_price <= fixed_take_profit)
        {
            return (
                create_close_decision(
                    symbol,
                    side,
                    open.quantity,
                    current_price,
                    "take_profit_triggered",
                ),
                None,
            );
        }
    }

    let trailing_cfg = cfg.trailing_runtime_config(symbol);
    if let Some(eval) = evaluate_trailing(open, current_price, &trailing_cfg) {
        let trailing_hit = if side == "Long" {
            current_price <= eval.trailing_stop_price
        } else {
            current_price >= eval.trailing_stop_price
        };

        if trailing_hit {
            return (
                create_close_decision(
                    symbol,
                    side,
                    open.quantity,
                    current_price,
                    "trailing_stop_triggered",
                ),
                Some(eval),
            );
        }

        return (None, Some(eval));
    }

    (None, None)
}

#[allow(clippy::too_many_arguments)]
fn enforce_entry_guards(
    symbol: &str,
    trend: f64,
    mut decision: StrategyDecision,
    entry_guards: &mut HashMap<String, EntryGuardState>,
    cooldown_minutes: i64,
    recent_stop_loss_same_symbol: bool,
    signal_confirmations: &mut HashMap<String, SignalConfirmationState>,
    min_confirmation_ticks: usize,
) -> StrategyDecision {
    if !matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
        return decision;
    }

    let proposed_side = if decision.action == "ENTER_LONG" {
        "Long"
    } else {
        "Short"
    };

    if recent_stop_loss_same_symbol {
        decision.action = "HOLD".to_string();
        decision.quantity = 0.0;
        decision.leverage = 0.0;
        decision.entry_price = 0.0;
        decision.stop_loss = 0.0;
        decision.take_profit = 0.0;
        decision.reason = format!("cooldown_stop_loss_{}m", cooldown_minutes);
        decision.smart_copy_compatible = false;
        return decision;
    }

    let confirmation = signal_confirmations
        .entry(symbol.to_string())
        .or_insert_with(|| SignalConfirmationState {
            side: proposed_side.to_string(),
            consecutive_valid_ticks: 0,
        });

    if !confirmation.side.eq_ignore_ascii_case(proposed_side) {
        confirmation.side = proposed_side.to_string();
        confirmation.consecutive_valid_ticks = 1;
    } else {
        confirmation.consecutive_valid_ticks += 1;
    }

    if confirmation.consecutive_valid_ticks < min_confirmation_ticks {
        decision.action = "HOLD".to_string();
        decision.quantity = 0.0;
        decision.leverage = 0.0;
        decision.entry_price = 0.0;
        decision.stop_loss = 0.0;
        decision.take_profit = 0.0;
        decision.reason = format!(
            "awaiting_signal_confirmation_{}/{}",
            confirmation.consecutive_valid_ticks, min_confirmation_ticks
        );
        decision.smart_copy_compatible = false;
        return decision;
    }

    if let Some(guard) = entry_guards.get_mut(symbol) {
        if Instant::now() < guard.cooldown_until {
            decision.action = "HOLD".to_string();
            decision.quantity = 0.0;
            decision.leverage = 0.0;
            decision.entry_price = 0.0;
            decision.stop_loss = 0.0;
            decision.take_profit = 0.0;
            decision.reason = format!("cooldown_stop_loss_{}m", cooldown_minutes);
            decision.smart_copy_compatible = false;
            return decision;
        }

        if !guard.awaiting_flip {
            return decision;
        }

        if !is_same_direction(&guard.blocked_side, trend) {
            guard.awaiting_flip = false;
            return decision;
        }

        if guard.blocked_side.eq_ignore_ascii_case(proposed_side) {
            decision.action = "HOLD".to_string();
            decision.quantity = 0.0;
            decision.leverage = 0.0;
            decision.entry_price = 0.0;
            decision.stop_loss = 0.0;
            decision.take_profit = 0.0;
            decision.reason = "blocked_until_trend_flip".to_string();
            decision.smart_copy_compatible = false;
        }
    }

    decision
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {},
                    _ = sigterm.recv() => {},
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting viper-strategy");

    let listener = TcpListener::bind("0.0.0.0:8082").await?;
    println!("Health check server running on :8082");

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_signal_tx.send(true);
    });

    let mut health_shutdown_rx = shutdown_rx.clone();
    let invalid_signal_count = Arc::new(AtomicU64::new(0));
    let invalid_signal_count_for_health = Arc::clone(&invalid_signal_count);
    let last_invalid_signal = Arc::new(RwLock::new(None::<InvalidSignalDrop>));
    let last_invalid_signal_for_health = Arc::clone(&last_invalid_signal);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = health_shutdown_rx.changed() => {
                    break;
                }
                accept_result = listener.accept() => {
                    if let Ok((mut socket, _)) = accept_result {
                        let invalid_signal_count_for_conn =
                            Arc::clone(&invalid_signal_count_for_health);
                        let last_invalid_signal_for_conn =
                            Arc::clone(&last_invalid_signal_for_health);
                        tokio::spawn(async move {
                            let last_invalid = last_invalid_signal_for_conn.read().await.clone();
                            let body = serde_json::json!({
                                "status": "ok",
                                "invalid_market_signals_dropped": invalid_signal_count_for_conn.load(Ordering::Relaxed),
                                "last_invalid_market_signal_drop": last_invalid.as_ref().map(|drop| json!({
                                    "symbol": drop.symbol,
                                    "stage": drop.stage,
                                    "reason": drop.reason,
                                    "timestamp": drop.timestamp,
                                })),
                            })
                            .to_string();
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            if let Err(e) = socket.write_all(response.as_bytes()).await {
                                eprintln!("failed to write to socket; err = {:?}", e);
                            }
                        });
                    }
                }
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
    let trading_mode = std::env::var("TRADING_MODE").unwrap_or_else(|_| "paper".to_string());

    let cfg = Arc::new(StrategyConfig::from_files(
        &strategy_config_path,
        &profile_config_path,
        &trading_profile,
        &trading_mode,
    )?);

    let execution_plan = load_execution_plan(&pipeline_path)?;

    let runtime = Runtime::new();
    register_strategy_steps(&runtime, &execution_plan, Arc::clone(&cfg));
    println!(
        "Loaded in-process plan '{}' with {} step(s) and profile {}/{}",
        execution_plan.name,
        execution_plan.steps.len(),
        cfg.trading_mode,
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
    let mut messages = pubsub.on_message();
    let mut entry_guards = HashMap::<String, EntryGuardState>::new();
    let mut signal_confirmations = HashMap::<String, SignalConfirmationState>::new();
    let default_stop_loss_cooldown_minutes = 3_i64;
    let wallet_api_base_url = resolve_wallet_api_base_url();
    let wallet_http = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let fallback_equity_usdt = std::env::var("INITIAL_CAPITAL_USD")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1_000.0);
    let mut cached_account_equity_usdt = fallback_equity_usdt;
    let mut last_wallet_fetch_at = Instant::now() - Duration::from_secs(60);

    let db_pool = match resolve_database_url() {
        Some(database_url) => match PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                println!("Strategy database connection: enabled");
                Some(pool)
            }
            Err(err) => {
                eprintln!(
                    "Strategy database unavailable (open-position trailing disabled): {}",
                    err
                );
                None
            }
        },
        None => {
            println!("Strategy database connection: disabled (missing DB_* env)");
            None
        }
    };

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                println!("Received shutdown signal, stopping viper-strategy");
                break;
            }
            maybe_msg = messages.next() => {
                let Some(msg) = maybe_msg else {
                    eprintln!("Market data stream ended unexpectedly; exiting so container can restart");
                    return Err("market data stream ended unexpectedly".into());
                };

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

                if let Err(err) = signal_event.validate() {
                    invalid_signal_count.fetch_add(1, Ordering::Relaxed);
                    let drop = InvalidSignalDrop {
                        symbol: signal_event.signal.symbol.clone(),
                        stage: "pre_decision".to_string(),
                        reason: err.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    };
                    *last_invalid_signal.write().await = Some(drop.clone());
                    eprintln!(
                        "{}",
                        serde_json::json!({
                            "service": "strategy",
                            "event": "invalid_market_signal_dropped",
                            "symbol": drop.symbol,
                            "stage": drop.stage,
                            "reason": drop.reason,
                            "timestamp": drop.timestamp,
                        })
                    );
                    continue;
                }

                let symbol = signal_event.signal.symbol.to_uppercase();
                let trend = signal_event.signal.trend_score;

                if let Some(guard) = entry_guards.get_mut(&symbol) {
                    if Instant::now() >= guard.cooldown_until {
                        // Cooldown expiration alone is not enough; same-direction reentry stays blocked
                        // until the market bias flips once.
                    }
                    if guard.awaiting_flip && !is_same_direction(&guard.blocked_side, trend) {
                        guard.awaiting_flip = false;
                    }
                }

                if let Some(pool) = &db_pool {
                    match fetch_open_trade_for_symbol(pool, &symbol).await {
                        Ok(Some(open)) => {
                            let current_price = signal_event.signal.current_price;
                            let (close_decision, trailing_eval) =
                                evaluate_open_trade_exit(&symbol, current_price, &open, cfg.as_ref());
                            if let Some(eval) = trailing_eval {
                                let trailing_cfg = cfg.trailing_runtime_config(&symbol);
                                if should_persist_trailing_update(
                                    &open,
                                    &eval,
                                    trailing_cfg.min_move_threshold_pct,
                                ) {
                                    if let Err(err) = update_trade_trailing_state(
                                        pool,
                                        &open.trade_id,
                                        eval.activated,
                                        eval.peak_price,
                                        eval.trail_pct,
                                    )
                                    .await
                                    {
                                        eprintln!(
                                            "Failed to persist trailing state trade_id={} err={}",
                                            open.trade_id, err
                                        );
                                    }
                                }
                            }
                            let decision = close_decision.unwrap_or_else(|| {
                                create_hold_decision(&symbol, "open_position_monitoring")
                            });

                            if let Err(err) =
                                publish_decision_event(&mut publish_conn, &signal_event.event_id, decision)
                                    .await
                            {
                                eprintln!("Failed to publish open-position decision: {}", err);
                            }
                            continue;
                        }
                        Ok(None) => {}
                        Err(err) => {
                            eprintln!(
                                "Failed to query open trade for symbol={} err={}",
                                symbol, err
                            );
                        }
                    }
                }

                if last_wallet_fetch_at.elapsed() >= Duration::from_secs(15) {
                    cached_account_equity_usdt = fetch_account_equity_usdt(
                        &wallet_http,
                        &wallet_api_base_url,
                        fallback_equity_usdt,
                    )
                    .await;
                    last_wallet_fetch_at = Instant::now();
                }

                let mut input = serde_json::to_value(&signal_event.signal)?;
                if let Some(obj) = input.as_object_mut() {
                    obj.insert(
                        "account_equity_usdt".to_string(),
                        json!(cached_account_equity_usdt),
                    );
                }
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
                        let intended_side = if decision.action == "ENTER_SHORT" {
                            "short"
                        } else {
                            "long"
                        };
                        let min_confirmation_ticks =
                            cfg.min_signal_confirmation_ticks_for_side(&symbol, intended_side);
                        let stop_loss_cooldown_minutes = cfg
                            .stop_loss_cooldown_minutes_for_side(&symbol, intended_side)
                            .max(default_stop_loss_cooldown_minutes);
                        let recent_stop_loss_same_symbol = if let Some(pool) = &db_pool {
                            if matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
                                match has_recent_stop_loss_for_symbol(
                                    pool,
                                    &symbol,
                                    stop_loss_cooldown_minutes,
                                )
                                .await
                                {
                                    Ok(v) => v,
                                    Err(err) => {
                                        eprintln!(
                                            "Failed to check stop-loss cooldown symbol={} err={}",
                                            symbol, err
                                        );
                                        false
                                    }
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        let decision = enforce_entry_guards(
                            &symbol,
                            trend,
                            decision,
                            &mut entry_guards,
                            stop_loss_cooldown_minutes,
                            recent_stop_loss_same_symbol,
                            &mut signal_confirmations,
                            min_confirmation_ticks,
                        );

                        if matches!(decision.action.as_str(), "ENTER_LONG" | "ENTER_SHORT") {
                            signal_confirmations.remove(&symbol);
                        }

                        if decision.reason == "stop_loss_triggered" {
                            let blocked_side = if decision.action == "CLOSE_LONG" {
                                "Long"
                            } else if decision.action == "CLOSE_SHORT" {
                                "Short"
                            } else {
                                ""
                            };
                            if !blocked_side.is_empty() {
                                let cooldown_minutes = cfg
                                    .stop_loss_cooldown_minutes_for_side(
                                        &symbol,
                                        &blocked_side.to_lowercase(),
                                    )
                                    .max(default_stop_loss_cooldown_minutes);
                                entry_guards.insert(
                                    symbol.clone(),
                                    EntryGuardState {
                                        blocked_side: blocked_side.to_string(),
                                        cooldown_until: Instant::now()
                                            + Duration::from_secs(
                                                (cooldown_minutes * 60) as u64,
                                            ),
                                        awaiting_flip: true,
                                    },
                                );
                                signal_confirmations.remove(&symbol);
                            }
                        }

                        if let Err(err) =
                            publish_decision_event(&mut publish_conn, &signal_event.event_id, decision)
                                .await
                        {
                            eprintln!(
                                "Invalid strategy decision event contract for {}: {}",
                                signal_event.signal.symbol, err
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to parse strategy decision for {}: {}",
                            signal_event.signal.symbol, e
                        );
                    }
                }
            }
        }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}
