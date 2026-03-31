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
    pub consensus_atr_14: f64,
    #[serde(default)]
    pub consensus_volume_24h: i64,
    #[serde(default)]
    pub consensus_funding_rate: f64,
    #[serde(default)]
    pub consensus_trend_score: f64,
    #[serde(default)]
    pub consensus_spread_pct: f64,
    #[serde(default)]
    pub consensus_trend_slope: f64,
    #[serde(default)]
    pub ema_fast: f64,
    #[serde(default)]
    pub ema_slow: f64,
    #[serde(default)]
    pub bollinger_upper: f64,
    #[serde(default)]
    pub bollinger_middle: f64,
    #[serde(default)]
    pub bollinger_lower: f64,
    #[serde(default)]
    pub bollinger_bandwidth: f64,
    #[serde(default)]
    pub bollinger_percent_b: f64,
    #[serde(default)]
    pub consensus_ema_fast: f64,
    #[serde(default)]
    pub consensus_ema_slow: f64,
    #[serde(default)]
    pub consensus_bollinger_upper: f64,
    #[serde(default)]
    pub consensus_bollinger_middle: f64,
    #[serde(default)]
    pub consensus_bollinger_lower: f64,
    #[serde(default)]
    pub consensus_bollinger_bandwidth: f64,
    #[serde(default)]
    pub consensus_bollinger_percent_b: f64,
    #[serde(default)]
    pub rsi_14: f64,
    #[serde(default)]
    pub consensus_rsi_14: f64,
    #[serde(default)]
    pub macd_line: f64,
    #[serde(default)]
    pub macd_signal: f64,
    #[serde(default)]
    pub macd_histogram: f64,
    #[serde(default)]
    pub consensus_macd_line: f64,
    #[serde(default)]
    pub consensus_macd_signal: f64,
    #[serde(default)]
    pub consensus_macd_histogram: f64,
    #[serde(default)]
    pub volume_ratio: f64,
    #[serde(default)]
    pub consensus_volume_ratio: f64,
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

impl MarketSignal {
    pub fn validate(&self) -> Result<(), String> {
        if self.symbol.trim().is_empty() {
            return Err("market signal symbol is empty".to_string());
        }
        if !(self.current_price.is_finite() && self.current_price > 0.0) {
            return Err("market signal current_price must be finite and > 0".to_string());
        }
        if !(self.bybit_price.is_finite() && self.bybit_price >= 0.0) {
            return Err("market signal bybit_price must be finite and >= 0".to_string());
        }
        if !(self.atr_14.is_finite() && self.atr_14 >= 0.0) {
            return Err("market signal atr_14 must be finite and >= 0".to_string());
        }
        if self.volume_24h < 0 {
            return Err("market signal volume_24h must be >= 0".to_string());
        }
        if !(self.funding_rate.is_finite()) {
            return Err("market signal funding_rate must be finite".to_string());
        }
        if !(self.trend_score.is_finite()) {
            return Err("market signal trend_score must be finite".to_string());
        }
        if !(self.spread_pct.is_finite() && self.spread_pct >= 0.0) {
            return Err("market signal spread_pct must be finite and >= 0".to_string());
        }
        if !(self.consensus_atr_14.is_finite() && self.consensus_atr_14 >= 0.0) {
            return Err("market signal consensus_atr_14 must be finite and >= 0".to_string());
        }
        if self.consensus_volume_24h < 0 {
            return Err("market signal consensus_volume_24h must be >= 0".to_string());
        }
        if !(self.consensus_funding_rate.is_finite()) {
            return Err("market signal consensus_funding_rate must be finite".to_string());
        }
        if !(self.consensus_trend_score.is_finite()) {
            return Err("market signal consensus_trend_score must be finite".to_string());
        }
        if !(self.consensus_spread_pct.is_finite() && self.consensus_spread_pct >= 0.0) {
            return Err("market signal consensus_spread_pct must be finite and >= 0".to_string());
        }
        if !(self.consensus_trend_slope.is_finite()) {
            return Err("market signal consensus_trend_slope must be finite".to_string());
        }
        if !(self.ema_fast.is_finite()) {
            return Err("market signal ema_fast must be finite".to_string());
        }
        if !(self.ema_slow.is_finite()) {
            return Err("market signal ema_slow must be finite".to_string());
        }
        if !(self.bollinger_upper.is_finite()) {
            return Err("market signal bollinger_upper must be finite".to_string());
        }
        if !(self.bollinger_middle.is_finite()) {
            return Err("market signal bollinger_middle must be finite".to_string());
        }
        if !(self.bollinger_lower.is_finite()) {
            return Err("market signal bollinger_lower must be finite".to_string());
        }
        if !(self.bollinger_upper >= self.bollinger_middle
            && self.bollinger_middle >= self.bollinger_lower)
        {
            return Err(
                "market signal bollinger bands must satisfy upper >= middle >= lower".to_string(),
            );
        }
        if !(self.bollinger_bandwidth.is_finite() && self.bollinger_bandwidth >= 0.0) {
            return Err("market signal bollinger_bandwidth must be finite and >= 0".to_string());
        }
        if !(self.bollinger_percent_b.is_finite()) {
            return Err("market signal bollinger_percent_b must be finite".to_string());
        }
        if !(self.consensus_ema_fast.is_finite()) {
            return Err("market signal consensus_ema_fast must be finite".to_string());
        }
        if !(self.consensus_ema_slow.is_finite()) {
            return Err("market signal consensus_ema_slow must be finite".to_string());
        }
        if !(self.consensus_bollinger_upper.is_finite()) {
            return Err("market signal consensus_bollinger_upper must be finite".to_string());
        }
        if !(self.consensus_bollinger_middle.is_finite()) {
            return Err("market signal consensus_bollinger_middle must be finite".to_string());
        }
        if !(self.consensus_bollinger_lower.is_finite()) {
            return Err("market signal consensus_bollinger_lower must be finite".to_string());
        }
        if !(self.consensus_bollinger_upper >= self.consensus_bollinger_middle
            && self.consensus_bollinger_middle >= self.consensus_bollinger_lower)
        {
            return Err(
                "market signal consensus bollinger bands must satisfy upper >= middle >= lower"
                    .to_string(),
            );
        }
        if !(self.consensus_bollinger_bandwidth.is_finite()
            && self.consensus_bollinger_bandwidth >= 0.0)
        {
            return Err(
                "market signal consensus_bollinger_bandwidth must be finite and >= 0".to_string(),
            );
        }
        if !(self.consensus_bollinger_percent_b.is_finite()) {
            return Err("market signal consensus_bollinger_percent_b must be finite".to_string());
        }
        if !(self.rsi_14.is_finite() && (0.0..=100.0).contains(&self.rsi_14)) {
            return Err("market signal rsi_14 must be finite and between 0 and 100".to_string());
        }
        if !(self.consensus_rsi_14.is_finite() && (0.0..=100.0).contains(&self.consensus_rsi_14)) {
            return Err(
                "market signal consensus_rsi_14 must be finite and between 0 and 100".to_string(),
            );
        }
        if !(self.macd_line.is_finite()) {
            return Err("market signal macd_line must be finite".to_string());
        }
        if !(self.macd_signal.is_finite()) {
            return Err("market signal macd_signal must be finite".to_string());
        }
        if !(self.macd_histogram.is_finite()) {
            return Err("market signal macd_histogram must be finite".to_string());
        }
        if !(self.consensus_macd_line.is_finite()) {
            return Err("market signal consensus_macd_line must be finite".to_string());
        }
        if !(self.consensus_macd_signal.is_finite()) {
            return Err("market signal consensus_macd_signal must be finite".to_string());
        }
        if !(self.consensus_macd_histogram.is_finite()) {
            return Err("market signal consensus_macd_histogram must be finite".to_string());
        }
        if !(self.volume_ratio.is_finite() && self.volume_ratio >= 0.0) {
            return Err("market signal volume_ratio must be finite and >= 0".to_string());
        }
        if !(self.consensus_volume_ratio.is_finite() && self.consensus_volume_ratio >= 0.0) {
            return Err("market signal consensus_volume_ratio must be finite and >= 0".to_string());
        }
        if !(self.btc_trend_score.is_finite()) {
            return Err("market signal btc_trend_score must be finite".to_string());
        }
        if self.btc_consensus_count < 0 {
            return Err("market signal btc_consensus_count must be >= 0".to_string());
        }
        if !(self.btc_volume_ratio.is_finite() && self.btc_volume_ratio >= 0.0) {
            return Err("market signal btc_volume_ratio must be finite and >= 0".to_string());
        }
        if self.consensus_count < 0 {
            return Err("market signal consensus_count must be >= 0".to_string());
        }
        if self.exchanges_available < 0 {
            return Err("market signal exchanges_available must be >= 0".to_string());
        }
        if !(self.consensus_ratio.is_finite() && self.consensus_ratio >= 0.0) {
            return Err("market signal consensus_ratio must be finite and >= 0".to_string());
        }
        if !(self.trend_slope.is_finite()) {
            return Err("market signal trend_slope must be finite".to_string());
        }
        if self.bullish_exchanges < 0 {
            return Err("market signal bullish_exchanges must be >= 0".to_string());
        }
        if self.bearish_exchanges < 0 {
            return Err("market signal bearish_exchanges must be >= 0".to_string());
        }

        Ok(())
    }
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
        self.signal.validate()
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
            consensus_atr_14: 0.01,
            consensus_volume_24h: 100_000_000,
            consensus_funding_rate: 0.001,
            consensus_trend_score: 0.72,
            consensus_spread_pct: 0.0006,
            consensus_trend_slope: 0.0038,
            ema_fast: 0.171,
            ema_slow: 0.168,
            bollinger_upper: 0.176,
            bollinger_middle: 0.169,
            bollinger_lower: 0.162,
            bollinger_bandwidth: 0.0828,
            bollinger_percent_b: 0.5714,
            consensus_ema_fast: 0.1705,
            consensus_ema_slow: 0.1681,
            consensus_bollinger_upper: 0.1758,
            consensus_bollinger_middle: 0.1689,
            consensus_bollinger_lower: 0.1620,
            consensus_bollinger_bandwidth: 0.0817,
            consensus_bollinger_percent_b: 0.5652,
            rsi_14: 61.0,
            consensus_rsi_14: 60.5,
            macd_line: 0.002,
            macd_signal: 0.0015,
            macd_histogram: 0.0005,
            consensus_macd_line: 0.0019,
            consensus_macd_signal: 0.0014,
            consensus_macd_histogram: 0.0005,
            volume_ratio: 1.2,
            consensus_volume_ratio: 1.15,
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

    #[test]
    fn invalid_market_signal_rsi_is_rejected() {
        let mut signal = sample_signal();
        signal.rsi_14 = 101.0;
        assert!(signal.validate().is_err());
    }

    #[test]
    fn invalid_market_signal_price_is_rejected() {
        let mut signal = sample_signal();
        signal.current_price = 0.0;
        assert!(signal.validate().is_err());
    }
}
