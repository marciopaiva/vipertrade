use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::helpers::*;
use crate::{RatchetLevel, TrailingRuntimeConfig};

/// Process-wide strategy config, set once at startup from the baked `pairs.yaml`
/// (the single source of truth) so the (free-fn) pipeline steps can reach it.
pub(crate) static STRATEGY_CFG: std::sync::OnceLock<Arc<StrategyConfig>> =
    std::sync::OnceLock::new();

/// `Some(cfg)` when real decisions are enabled and the config is available.
pub(crate) fn real_cfg() -> Option<Arc<StrategyConfig>> {
    let enabled = std::env::var("STRATEGY_REAL_DECISIONS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled {
        return None;
    }
    STRATEGY_CFG.get().cloned()
}

#[derive(Debug, Clone)]
pub struct StrategyConfig {
    pub(crate) profile: String,
    pub(crate) trading_mode: String,
    pub(crate) global: Value,
    pub(crate) pairs: HashMap<String, Value>,
    pub(crate) bollinger_std_dev_multiplier: f64,
    pub(crate) bollinger_invalidation_threshold: f64,
}

impl StrategyConfig {
    pub fn from_files(
        pairs_path: &str,
        profile: &str,
        trading_mode: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let pairs_raw = fs::read_to_string(pairs_path)?;
        let pairs_yaml: serde_yaml::Value = serde_yaml::from_str(&pairs_raw)?;
        let pairs_json = serde_json::to_value(pairs_yaml)?;
        Ok(Self::from_pairs_json(pairs_json, profile, trading_mode))
    }

    pub fn from_pairs_json(pairs_json: Value, profile: &str, trading_mode: &str) -> Self {
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

        Self {
            profile: profile.to_uppercase(),
            trading_mode: trading_mode.to_uppercase(),
            global,
            pairs,
            bollinger_std_dev_multiplier: 2.0,
            bollinger_invalidation_threshold: 0.7,
        }
    }

    pub fn symbol_universe(&self) -> Vec<(String, bool)> {
        let mut universe: Vec<(String, bool)> = self
            .pairs
            .iter()
            .map(|(symbol, cfg)| {
                let enabled = cfg.get("enabled").and_then(Value::as_bool).unwrap_or(false);
                (symbol.clone(), enabled)
            })
            .collect();
        universe.sort_by(|a, b| a.0.cmp(&b.0));
        universe
    }

    pub(crate) fn pair_cfg(&self, symbol: &str) -> Option<&Value> {
        self.pairs.get(&symbol.to_uppercase())
    }

    pub(crate) fn pair_mode_cfg(&self, symbol: &str) -> Option<&Value> {
        self.pair_cfg(symbol)
            .and_then(|value| cfg_get(value, &["mode_profiles", self.trading_mode.as_str()]))
    }

    pub(crate) fn mode_cfg(&self) -> Option<&Value> {
        cfg_get(&self.global, &["mode_profiles", self.trading_mode.as_str()])
    }

    pub(crate) fn mode_flag(&self, key: &str, default: bool) -> bool {
        self.mode_cfg()
            .and_then(|value| cfg_get(value, &[key]))
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }

    pub(crate) fn mode_f64(&self, key: &str) -> Option<f64> {
        self.mode_cfg()
            .and_then(|value| cfg_get(value, &[key]))
            .and_then(Value::as_f64)
    }

    pub(crate) fn mode_i64(&self, key: &str) -> Option<i64> {
        self.mode_cfg()
            .and_then(|value| cfg_get(value, &[key]))
            .and_then(Value::as_i64)
    }

    pub(crate) fn max_daily_loss_pct(&self) -> f64 {
        cfg_f64(&self.global, &["risk", "max_daily_loss_pct"], 0.03)
    }

    pub(crate) fn max_consecutive_losses(&self) -> i64 {
        cfg_i64(&self.global, &["risk", "max_consecutive_losses"], 3)
    }

    pub(crate) fn risk_per_trade_fraction(&self) -> f64 {
        let pct = cfg_f64(&self.global, &["risk", "risk_per_trade_pct"], 1.0);
        if pct > 1.0 {
            pct / 100.0
        } else {
            pct
        }
    }

    pub(crate) fn max_leverage(&self) -> f64 {
        cfg_f64(&self.global, &["risk", "max_leverage"], 2.0)
    }

    pub(crate) fn min_position_usdt(&self) -> f64 {
        cfg_f64(&self.global, &["smart_copy", "min_position_usdt"], 5.0)
    }

    pub(crate) fn mode_risk_f64(&self, key: &str) -> Option<f64> {
        self.mode_cfg()
            .and_then(|v| cfg_get(v, &["risk", key]))
            .and_then(Value::as_f64)
    }

    pub(crate) fn max_position_usdt(&self, symbol: &str) -> f64 {
        let global_max = cfg_f64(&self.global, &["smart_copy", "max_position_usdt"], 30.0);
        let pair = self
            .pair_mode_cfg(symbol)
            .and_then(|v| cfg_get(v, &["risk", "max_position_usdt"]))
            .and_then(Value::as_f64)
            .or_else(|| {
                self.pair_cfg(symbol)
                    .and_then(|v| cfg_get(v, &["risk", "max_position_usdt"]))
                    .and_then(Value::as_f64)
            });
        pair.or_else(|| self.mode_risk_f64("max_position_usdt"))
            .unwrap_or(global_max)
            .min(global_max)
    }

    pub(crate) fn max_position_wallet_pct(&self, symbol: &str) -> Option<f64> {
        self.pair_mode_cfg(symbol)
            .and_then(|v| cfg_get(v, &["risk", "max_position_wallet_pct"]))
            .and_then(Value::as_f64)
            .or_else(|| {
                self.pair_cfg(symbol)
                    .and_then(|v| cfg_get(v, &["risk", "max_position_wallet_pct"]))
                    .and_then(Value::as_f64)
            })
            .or_else(|| self.mode_risk_f64("max_position_wallet_pct"))
    }

    pub(crate) fn atr_multiplier(&self, symbol: &str) -> f64 {
        self.pair_cfg(symbol)
            .and_then(|v| cfg_get(v, &["risk", "atr_multiplier"]))
            .and_then(Value::as_f64)
            .or_else(|| self.mode_risk_f64("atr_multiplier"))
            .unwrap_or(1.0)
    }

    pub(crate) fn max_position_cap_usdt(&self, symbol: &str, equity_usdt: f64) -> f64 {
        self.max_position_wallet_pct(symbol)
            .map(|pct| equity_usdt * pct)
            .unwrap_or_else(|| self.max_position_usdt(symbol))
    }

    pub(crate) fn max_spread_pct(&self, symbol: &str) -> f64 {
        self.mode_f64("max_spread_pct").unwrap_or_else(|| {
            self.pair_cfg(symbol)
                .map(|v| cfg_f64(v, &["liquidity", "max_spread_pct"], 0.001))
                .unwrap_or_else(|| {
                    cfg_f64(&self.global, &["entry_filters", "max_spread_pct"], 0.001)
                })
        })
    }

    pub(crate) fn max_atr_pct(&self, symbol: &str) -> f64 {
        self.mode_f64("max_atr_pct").unwrap_or_else(|| {
            self.pair_cfg(symbol)
                .map(|v| cfg_f64(v, &["entry_filters", "max_atr_pct"], 0.05))
                .unwrap_or_else(|| cfg_f64(&self.global, &["entry_filters", "max_atr_pct"], 0.05))
        })
    }

    pub(crate) fn min_trend_score_for_side(&self, symbol: &str, side: &str) -> f64 {
        let side_key = if side.eq_ignore_ascii_case("short") {
            "min_trend_score_short"
        } else {
            "min_trend_score_long"
        };

        // CORREÇÃO 2026-04-02: Verifica configuração por símbolo PRIMEIRO
        // Bug anterior: mode_f64 era verificado primeiro, ignorando config por símbolo
        if let Some(pair_value) = self
            .pair_cfg(symbol)
            .and_then(|v| cfg_get(v, &["entry_filters", side_key]))
            .and_then(Value::as_f64)
        {
            return pair_value;
        }

        // Depois usa o global mode como fallback
        if let Some(value) = self.mode_f64(side_key) {
            return value;
        }

        // Fallback para configuração genérica do símbolo
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

    pub(crate) fn allow_long(&self, symbol: &str) -> bool {
        self.pair_cfg(symbol)
            .map(|v| cfg_bool(v, &["entry_filters", "allow_long"], true))
            .unwrap_or(true)
    }

    pub(crate) fn allow_short(&self, symbol: &str) -> bool {
        self.pair_cfg(symbol)
            .map(|v| cfg_bool(v, &["entry_filters", "allow_short"], true))
            .unwrap_or(true)
    }

    pub(crate) fn min_signal_confirmation_ticks(&self, symbol: &str) -> usize {
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

    pub(crate) fn min_signal_confirmation_ticks_for_side(&self, symbol: &str, side: &str) -> usize {
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

    pub(crate) fn thesis_invalidation_enabled(&self) -> bool {
        self.mode_cfg()
            .and_then(|value| cfg_get(value, &["entry_filters", "exit_on_thesis_invalidation"]))
            .and_then(Value::as_bool)
            .unwrap_or_else(|| !self.permissive_entry())
    }

    pub(crate) fn thesis_invalidation_confirmation_ticks(&self, symbol: &str) -> usize {
        self.mode_cfg()
            .and_then(|value| {
                cfg_get(
                    value,
                    &["entry_filters", "thesis_invalidation_confirmation_ticks"],
                )
            })
            .and_then(Value::as_u64)
            .map(|value| value.max(1) as usize)
            .or_else(|| {
                self.pair_cfg(symbol)
                    .and_then(|value| {
                        cfg_get(
                            value,
                            &["entry_filters", "thesis_invalidation_confirmation_ticks"],
                        )
                    })
                    .and_then(Value::as_u64)
                    .map(|value| value.max(1) as usize)
            })
            .unwrap_or_else(|| {
                cfg_get(
                    &self.global,
                    &["entry_filters", "thesis_invalidation_confirmation_ticks"],
                )
                .and_then(Value::as_u64)
                .map(|value| value.max(1) as usize)
                .unwrap_or(2)
            })
    }

    pub(crate) fn thesis_degrading_confirmation_ticks(&self, symbol: &str) -> usize {
        self.thesis_invalidation_confirmation_ticks(symbol)
            .saturating_sub(1)
            .max(1)
    }

    pub(crate) fn stop_loss_cooldown_minutes_for_side(&self, symbol: &str, side: &str) -> i64 {
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

    pub(crate) fn thesis_invalidation_cooldown_minutes_for_side(
        &self,
        symbol: &str,
        side: &str,
    ) -> i64 {
        let side_key = if side.eq_ignore_ascii_case("short") {
            "thesis_invalidation_cooldown_minutes_short"
        } else {
            "thesis_invalidation_cooldown_minutes_long"
        };

        if let Some(value) = self.mode_i64(side_key) {
            return value.max(0);
        }

        self.pair_cfg(symbol)
            .map(|v| {
                cfg_i64(
                    v,
                    &["entry_filters", side_key],
                    cfg_i64(
                        v,
                        &["entry_filters", "thesis_invalidation_cooldown_minutes"],
                        3,
                    ),
                )
            })
            .unwrap_or_else(|| {
                cfg_i64(
                    &self.global,
                    &["entry_filters", side_key],
                    cfg_i64(
                        &self.global,
                        &["entry_filters", "thesis_invalidation_cooldown_minutes"],
                        3,
                    ),
                )
            })
    }

    pub(crate) fn min_volume_ratio_for_side(&self, symbol: &str, side: &str) -> f64 {
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

    pub(crate) fn rsi_bounds_for_side(&self, symbol: &str, side: &str) -> (f64, f64) {
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

    pub(crate) fn percent_b_limit_for_side(&self, symbol: &str, side: &str) -> f64 {
        // Defaults are ±infinity so an unconfigured guard never blocks an entry,
        // even at extreme %B (price far outside the bands).
        let (key, default) = if side.eq_ignore_ascii_case("short") {
            ("min_percent_b_short", f64::NEG_INFINITY)
        } else {
            ("max_percent_b_long", f64::INFINITY)
        };
        if let Some(value) = self.mode_f64(key) {
            return value;
        }
        self.pair_cfg(symbol)
            .map(|v| {
                cfg_f64(
                    v,
                    &["entry_filters", key],
                    cfg_f64(&self.global, &["entry_filters", key], default),
                )
            })
            .unwrap_or_else(|| cfg_f64(&self.global, &["entry_filters", key], default))
    }

    pub(crate) fn min_adx(&self, symbol: &str) -> f64 {
        if let Some(value) = self.mode_f64("min_adx") {
            return value;
        }
        self.pair_cfg(symbol)
            .map(|v| {
                cfg_f64(
                    v,
                    &["entry_filters", "min_adx"],
                    cfg_f64(&self.global, &["entry_filters", "min_adx"], 0.0),
                )
            })
            .unwrap_or_else(|| cfg_f64(&self.global, &["entry_filters", "min_adx"], 0.0))
    }

    pub(crate) fn btc_macro_penalty_for_side(
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

        let min_trend_score = self.btc_macro_min_trend_score_for_side(side);
        let min_consensus_count = self.btc_macro_min_consensus_count_for_side(side);

        let aligned = if side.eq_ignore_ascii_case("short") {
            btc_regime.eq_ignore_ascii_case("bearish")
                && btc_trend_score <= -min_trend_score
                && btc_consensus_count >= min_consensus_count
        } else {
            btc_regime.eq_ignore_ascii_case("bullish")
                && btc_trend_score >= min_trend_score
                && btc_consensus_count >= min_consensus_count
        };

        if aligned {
            return Some(0.0);
        }

        let neutral = btc_regime.eq_ignore_ascii_case("neutral")
            && btc_consensus_count >= min_consensus_count;
        if neutral {
            return Some(self.btc_macro_neutral_penalty());
        }

        None
    }

    pub(crate) fn btc_macro_min_trend_score_for_side(&self, side: &str) -> f64 {
        let side_key = if side.eq_ignore_ascii_case("short") {
            "btc_macro_min_trend_score_short"
        } else {
            "btc_macro_min_trend_score_long"
        };

        self.mode_f64(side_key)
            .unwrap_or_else(|| cfg_f64(&self.global, &["entry_filters", side_key], 0.05))
    }

    pub(crate) fn btc_macro_min_consensus_count_for_side(&self, side: &str) -> i64 {
        let side_key = if side.eq_ignore_ascii_case("short") {
            "btc_macro_min_consensus_count_short"
        } else {
            "btc_macro_min_consensus_count_long"
        };

        self.mode_i64(side_key)
            .unwrap_or_else(|| cfg_i64(&self.global, &["entry_filters", side_key], 2))
            .max(1)
    }

    pub(crate) fn btc_macro_neutral_penalty(&self) -> f64 {
        self.mode_f64("btc_macro_neutral_penalty")
            .unwrap_or_else(|| {
                cfg_f64(
                    &self.global,
                    &["entry_filters", "btc_macro_neutral_penalty"],
                    0.05,
                )
            })
    }

    pub(crate) fn min_volume_24h_usdt(&self, symbol: &str) -> i64 {
        self.pair_mode_cfg(symbol)
            .and_then(|v| cfg_get(v, &["entry_filters", "min_volume_24h_usdt"]))
            .and_then(Value::as_i64)
            .or_else(|| {
                self.pair_mode_cfg(symbol)
                    .and_then(|v| cfg_get(v, &["liquidity", "min_24h_volume_usdt"]))
                    .and_then(Value::as_i64)
            })
            .or_else(|| self.mode_i64("min_volume_24h_usdt"))
            .unwrap_or_else(|| {
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

    pub(crate) fn max_funding_rate_pct(&self) -> f64 {
        self.mode_f64("max_funding_rate_pct").unwrap_or_else(|| {
            cfg_f64(
                &self.global,
                &["entry_filters", "max_funding_rate_pct"],
                0.015,
            )
        })
    }

    pub(crate) fn require_multi_exchange_consensus(&self) -> bool {
        self.mode_flag("require_multi_exchange_consensus", true)
    }

    pub(crate) fn require_btc_macro_alignment(&self) -> bool {
        self.mode_flag("require_btc_macro_alignment", true)
    }

    pub(crate) fn permissive_entry(&self) -> bool {
        self.mode_flag("permissive_entry", false)
    }

    pub(crate) fn min_hold_seconds(&self) -> i64 {
        self.mode_i64("min_hold_seconds").unwrap_or(0).max(0)
    }

    pub(crate) fn stop_loss_pct(&self, symbol: &str) -> f64 {
        if let Some(value) = self.mode_f64("stop_loss_pct") {
            return value;
        }
        if let Some(pair) = self.pair_cfg(symbol) {
            return cfg_f64(pair, &["risk", "stop_loss_pct"], 0.015);
        }
        0.015
    }

    pub(crate) fn take_profit_pct(&self, symbol: &str) -> f64 {
        if let Some(value) = self.mode_f64("take_profit_pct") {
            return value;
        }
        if let Some(pair) = self.pair_cfg(symbol) {
            return cfg_f64(pair, &["risk", "take_profit_pct"], 0.03);
        }
        0.03
    }

    pub(crate) fn trailing_config(&self, symbol: &str) -> Value {
        if let Some(mode_cfg) = self.mode_cfg().and_then(|v| cfg_get(v, &["trailing_stop"])) {
            return mode_cfg.clone();
        }
        if let Some(pair) = self.pair_cfg(symbol) {
            if let Some(by_profile) = cfg_get(pair, &["trailing_stop", "by_profile", &self.profile])
            {
                return by_profile.clone();
            }
        }
        json!({
            "activate_after_profit_pct": 0.015,
            "initial_trail_pct": 0.008,
            "ratchet_levels": [],
            "move_to_break_even_at": 0.02
        })
    }

    pub(crate) fn trailing_enabled(&self, symbol: &str) -> bool {
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

    pub(crate) fn fixed_take_profit_enabled(&self) -> bool {
        self.mode_cfg()
            .and_then(|v| cfg_get(v, &["fixed_take_profit_enabled"]))
            .and_then(Value::as_bool)
            .unwrap_or(true)
    }

    pub(crate) fn trailing_min_move_threshold_pct(&self) -> f64 {
        // Read from the mode profile (where the rest of the trailing config lives),
        // falling back to the legacy global.trailing_stop block, then a small default
        // matched to the trail geometry. The old 0.002 (0.2%) was coarser than this
        // strategy's sub-0.2% profit peaks, so it froze the persisted peak and
        // degraded the trail to a break-even-only guard (see
        // should_persist_trailing_update).
        self.mode_cfg()
            .and_then(|value| cfg_get(value, &["trailing_stop", "min_move_threshold_pct"]))
            .and_then(Value::as_f64)
            .or_else(|| {
                cfg_get(&self.global, &["trailing_stop", "min_move_threshold_pct"])
                    .and_then(Value::as_f64)
            })
            .unwrap_or(0.0002)
    }

    pub(crate) fn trailing_runtime_config(&self, symbol: &str) -> TrailingRuntimeConfig {
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

    // ── Weight accessors (externalized from hardcoded constants) ──

    pub(crate) fn entry_weight(&self, key: &str, default: f64) -> f64 {
        cfg_f64(&self.global, &["weights", "entry", key], default)
    }

    pub(crate) fn decision_weight(&self, key: &str, default: f64) -> f64 {
        cfg_f64(&self.global, &["weights", "decision", key], default)
    }

    pub(crate) fn size_weight(&self, key: &str, default: f64) -> f64 {
        cfg_f64(&self.global, &["weights", "size", key], default)
    }
}
