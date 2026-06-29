use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use std::collections::HashMap;
use std::time::Instant;
use viper_domain::{MarketSignalEvent, StrategyDecision};

#[cfg(test)]
use crate::StrategyConfig;

#[derive(Debug, Clone)]
pub(crate) struct RatchetLevel {
    pub(crate) at_profit_pct: f64,
    pub(crate) trail_pct: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct TrailingRuntimeConfig {
    pub(crate) enabled: bool,
    pub(crate) activate_after_profit_pct: f64,
    pub(crate) initial_trail_pct: f64,
    pub(crate) ratchet_levels: Vec<RatchetLevel>,
    pub(crate) move_to_break_even_at: f64,
    pub(crate) min_move_threshold_pct: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct OpenTradeSnapshot {
    pub(crate) trade_id: String,
    pub(crate) side: String,
    pub(crate) quantity: f64,
    pub(crate) entry_price: f64,
    pub(crate) opened_at: DateTime<Utc>,
    pub(crate) trailing_stop_activated: bool,
    pub(crate) trailing_stop_peak_price: f64,
    pub(crate) trailing_stop_final_distance_pct: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct TrailingEval {
    pub(crate) activated: bool,
    pub(crate) peak_price: f64,
    pub(crate) trail_pct: f64,
    pub(crate) trailing_stop_price: f64,
    pub(crate) trailing_score: i32,
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TrailingPolicyComponent {
    pub(crate) reason: &'static str,
    pub(crate) score: f64,
    pub(crate) weight: f64,
    pub(crate) contribution: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct TrailingPolicyBreakdown {
    pub(crate) raw_score: i32,
    pub(crate) clamped_score: i32,
    pub(crate) components: Vec<TrailingPolicyComponent>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExitEvaluation {
    pub(crate) decision: Option<StrategyDecision>,
    pub(crate) trailing: Option<TrailingEval>,
    pub(crate) trigger: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct EntryGuardState {
    pub(crate) blocked_side: String,
    pub(crate) cooldown_until: Instant,
    pub(crate) cooldown_minutes: i64,
    pub(crate) cooldown_reason: String,
    pub(crate) awaiting_flip: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SignalConfirmationState {
    pub(crate) side: String,
    pub(crate) consecutive_valid_ticks: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ThesisInvalidationState {
    pub(crate) side: String,
    pub(crate) consecutive_invalid_ticks: usize,
    pub(crate) consecutive_degrading_ticks: usize,
    pub(crate) bollinger_invalidated: bool,
    pub(crate) bollinger_consecutive_hits: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct InvalidSignalDrop {
    pub(crate) symbol: String,
    pub(crate) stage: String,
    pub(crate) reason: String,
    pub(crate) timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EntryPolicyComponent {
    pub(crate) reason: &'static str,
    pub(crate) score: f64,
    pub(crate) weight: f64,
    pub(crate) contribution: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct EntryPolicyBreakdown {
    pub(crate) raw_score: i32,
    pub(crate) clamped_score: i32,
    pub(crate) components: Vec<EntryPolicyComponent>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SizePolicyComponent {
    pub(crate) reason: &'static str,
    pub(crate) score: f64,
    pub(crate) weight: f64,
    pub(crate) contribution: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct SizePolicyBreakdown {
    pub(crate) raw_score: i32,
    pub(crate) clamped_score: i32,
    pub(crate) components: Vec<SizePolicyComponent>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FundingPolicyComponent {
    pub(crate) reason: &'static str,
    pub(crate) score: f64,
    pub(crate) weight: f64,
    pub(crate) contribution: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct FundingPolicyBreakdown {
    pub(crate) raw_score: i32,
    pub(crate) clamped_score: i32,
    pub(crate) components: Vec<FundingPolicyComponent>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SizeProposalComponent {
    pub(crate) reason: &'static str,
    pub(crate) score: f64,
    pub(crate) weight: f64,
    pub(crate) contribution: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct SizeProposalBreakdown {
    pub(crate) raw_score: i32,
    pub(crate) clamped_score: i32,
    pub(crate) components: Vec<SizeProposalComponent>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DecisionPolicyComponent {
    pub(crate) reason: &'static str,
    pub(crate) score: f64,
    pub(crate) weight: f64,
    pub(crate) contribution: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct DecisionPolicyBreakdown {
    pub(crate) raw_score: i32,
    pub(crate) clamped_score: i32,
    pub(crate) components: Vec<DecisionPolicyComponent>,
}

#[derive(Debug, Clone)]
pub(crate) struct HealthScoreComponent {
    pub(crate) reason: &'static str,
    pub(crate) score: f64,
    pub(crate) weight: f64,
    pub(crate) contribution: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct PositionHealthBreakdown {
    pub(crate) raw_score: i32,
    pub(crate) clamped_score: i32,
    pub(crate) components: Vec<HealthScoreComponent>,
}

#[derive(Debug, Clone)]
pub(crate) struct ThesisInvalidationEvaluation {
    pub(crate) stage: &'static str,
    pub(crate) reason: String,
    pub(crate) health_score: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct EntryGuardEvaluation {
    pub(crate) blocked: bool,
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ThesisGuardEvaluation {
    pub(crate) confirmed: bool,
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingEntryCandidate {
    pub(crate) signal_event: MarketSignalEvent,
    pub(crate) decision: StrategyDecision,
    pub(crate) pipeline_input: Value,
    pub(crate) runtime_output: Value,
    pub(crate) execution_time_ms: i32,
    pub(crate) rank_score: f64,
    pub(crate) entry_score: f64,
    pub(crate) created_at: Instant,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ExecutionAdviceSnapshot {
    pub(crate) market_state: String,
    pub(crate) entry_action: String,
    pub(crate) exit_action: String,
    pub(crate) size_action: String,
    pub(crate) directional_bias: String,
    pub(crate) preferred_side: String,
    pub(crate) confidence: String,
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) evidence: Vec<String>,
    #[serde(default)]
    pub(crate) avoid_symbols: Vec<String>,
    #[serde(default)]
    pub(crate) priority_actions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ActivePositionAdviceSnapshot {
    pub(crate) symbol: String,
    pub(crate) side: String,
    pub(crate) action: String,
    pub(crate) confidence: String,
    #[serde(default)]
    pub(crate) maintenance_score: i32,
    pub(crate) market_state: String,
    pub(crate) pnl_pct_estimate: f64,
    pub(crate) duration_minutes: i64,
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) evidence: Vec<String>,
    #[serde(default)]
    pub(crate) risk_flags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct AiAnalystAdviceSnapshot {
    pub(crate) execution_advice: ExecutionAdviceSnapshot,
    #[serde(default)]
    pub(crate) active_position_advice: Vec<ActivePositionAdviceSnapshot>,
}

pub(crate) struct FinalizeDecisionContext<'a> {
    pub(crate) signal_event: &'a MarketSignalEvent,
    pub(crate) pipeline_input: &'a Value,
    pub(crate) runtime_output: &'a Value,
    pub(crate) execution_time_ms: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WalletSizingResponse {
    pub(crate) total_equity: Option<f64>,
    pub(crate) margin_balance: Option<f64>,
    pub(crate) wallet_balance: Option<f64>,
    pub(crate) available_balance: Option<f64>,
}

#[cfg(test)]
impl StrategyConfig {
    pub(crate) fn sample_for_tests() -> Self {
        StrategyConfig {
            profile: "TEST".to_string(),
            trading_mode: "PAPER".to_string(),
            global: serde_json::json!({
                "mode_profiles": { "PAPER": {
                    "stop_loss_pct": 0.01,
                    "fixed_take_profit_enabled": true,
                    "take_profit_pct": 0.02,
                    "min_hold_seconds": 0,
                    "trailing_enabled": true,
                    "trailing_stop": {
                        "activate_after_profit_pct": 0.01,
                        "initial_trail_pct": 0.005,
                        "move_to_break_even_at": 0.015
                    }
                }},
                "trailing_stop": { "min_move_threshold_pct": 0.002 }
            }),
            pairs: HashMap::new(),
            bollinger_std_dev_multiplier: 2.0,
            bollinger_invalidation_threshold: 0.7,
        }
    }
}
