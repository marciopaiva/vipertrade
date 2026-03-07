use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSignal {
    pub symbol: String,
    pub current_price: f64,
    pub atr_14: f64,
    pub volume_24h: i64,
    pub funding_rate: f64,
    pub trend_score: f64,
    pub spread_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSignalEvent {
    pub schema_version: String,
    pub event_id: String,
    pub timestamp: String,
    pub signal: MarketSignal,
}

impl MarketSignalEvent {
    pub fn new(signal: MarketSignal) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            signal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDecision {
    pub action: String,
    pub symbol: String,
    pub quantity: f64,
    pub leverage: f64,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub reason: String,
    pub smart_copy_compatible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDecisionEvent {
    pub schema_version: String,
    pub event_id: String,
    pub source_event_id: String,
    pub timestamp: String,
    pub decision: StrategyDecision,
}

impl StrategyDecisionEvent {
    pub fn new(source_event_id: String, decision: StrategyDecision) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            event_id: Uuid::new_v4().to_string(),
            source_event_id,
            timestamp: Utc::now().to_rfc3339(),
            decision,
        }
    }
}
