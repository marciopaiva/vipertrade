use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketSignal {
    pub symbol: String,
    pub current_price: f64,
    #[serde(default)]
    pub bybit_price: f64,
    pub atr_14: f64,
    pub volume_24h: i64,
    pub funding_rate: f64,
    pub trend_score: f64,
    pub spread_pct: f64,
    #[serde(default)]
    pub ema_fast: f64,
    #[serde(default)]
    pub ema_slow: f64,
    #[serde(default)]
    pub rsi_14: f64,
    #[serde(default)]
    pub macd_line: f64,
    #[serde(default)]
    pub macd_signal: f64,
    #[serde(default)]
    pub macd_histogram: f64,
    #[serde(default)]
    pub volume_ratio: f64,
    #[serde(default)]
    pub btc_regime: String,
    #[serde(default)]
    pub btc_trend_score: f64,
    #[serde(default)]
    pub btc_consensus_count: i64,
    #[serde(default)]
    pub btc_volume_ratio: f64,
    #[serde(default)]
    pub regime: String,
    #[serde(default)]
    pub consensus_side: String,
    #[serde(default)]
    pub consensus_count: i64,
    #[serde(default)]
    pub exchanges_available: i64,
    #[serde(default)]
    pub consensus_ratio: f64,
    #[serde(default)]
    pub trend_slope: f64,
    #[serde(default)]
    pub bybit_regime: String,
    #[serde(default)]
    pub bullish_exchanges: i64,
    #[serde(default)]
    pub bearish_exchanges: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported market signal schema_version '{}' expected '{}'",
                self.schema_version, SCHEMA_VERSION
            ));
        }
        if self.event_id.trim().is_empty() {
            return Err("market signal event_id is empty".to_string());
        }
        if self.signal.symbol.trim().is_empty() {
            return Err("market signal symbol is empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl StrategyDecision {
    pub fn validate(&self) -> Result<(), String> {
        if self.action.trim().is_empty() {
            return Err("strategy decision action is empty".to_string());
        }
        if self.symbol.trim().is_empty() {
            return Err("strategy decision symbol is empty".to_string());
        }
        if !(self.quantity.is_finite() && self.quantity >= 0.0) {
            return Err("strategy decision quantity must be finite and >= 0".to_string());
        }
        if !(self.leverage.is_finite() && self.leverage >= 0.0) {
            return Err("strategy decision leverage must be finite and >= 0".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported strategy decision schema_version '{}' expected '{}'",
                self.schema_version, SCHEMA_VERSION
            ));
        }
        if self.event_id.trim().is_empty() {
            return Err("strategy decision event_id is empty".to_string());
        }
        if self.source_event_id.trim().is_empty() {
            return Err("strategy decision source_event_id is empty".to_string());
        }
        self.decision.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signal() -> MarketSignal {
        MarketSignal {
            symbol: "DOGEUSDT".to_string(),
            current_price: 0.17,
            bybit_price: 0.17,
            atr_14: 0.01,
            volume_24h: 100_000_000,
            funding_rate: 0.001,
            trend_score: 0.7,
            spread_pct: 0.0005,
            ema_fast: 0.171,
            ema_slow: 0.168,
            rsi_14: 61.0,
            macd_line: 0.002,
            macd_signal: 0.0015,
            macd_histogram: 0.0005,
            volume_ratio: 1.2,
            btc_regime: "bullish".to_string(),
            btc_trend_score: 0.65,
            btc_consensus_count: 3,
            btc_volume_ratio: 1.4,
            regime: "bullish".to_string(),
            consensus_side: "bullish".to_string(),
            consensus_count: 3,
            exchanges_available: 3,
            consensus_ratio: 1.0,
            trend_slope: 0.004,
            bybit_regime: "bullish".to_string(),
            bullish_exchanges: 3,
            bearish_exchanges: 0,
        }
    }

    fn sample_decision() -> StrategyDecision {
        StrategyDecision {
            action: "ENTER_LONG".to_string(),
            symbol: "DOGEUSDT".to_string(),
            quantity: 100.0,
            leverage: 2.0,
            entry_price: 0.17,
            stop_loss: 0.165,
            take_profit: 0.18,
            reason: "trend_up".to_string(),
            smart_copy_compatible: true,
        }
    }

    #[test]
    fn market_signal_event_round_trip_and_validate() {
        let event = MarketSignalEvent::new(sample_signal());
        event.validate().expect("event must validate");

        let json = serde_json::to_string(&event).expect("serialize");
        let decoded: MarketSignalEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.schema_version, SCHEMA_VERSION);
        decoded.validate().expect("decoded event must validate");
    }

    #[test]
    fn strategy_decision_event_round_trip_and_validate() {
        let event = StrategyDecisionEvent::new("src-evt-1".to_string(), sample_decision());
        event.validate().expect("event must validate");

        let json = serde_json::to_string(&event).expect("serialize");
        let decoded: StrategyDecisionEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.schema_version, SCHEMA_VERSION);
        decoded.validate().expect("decoded event must validate");
    }

    #[test]
    fn invalid_schema_version_is_rejected() {
        let mut event = StrategyDecisionEvent::new("src-evt-1".to_string(), sample_decision());
        event.schema_version = "2.0".to_string();
        assert!(event.validate().is_err());
    }
}
