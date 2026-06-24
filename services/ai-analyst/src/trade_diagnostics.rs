use serde::{Deserialize, Serialize};
use tupa_core::pipeline;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalystSnapshot {
    pub lookback_hours: i64,
    pub summary: SnapshotSummary,
    pub expectancy: ExpectancyMetrics,
    pub exits: SnapshotExitMetrics,
    pub sides: SnapshotSideMetrics,
    pub blockers: SnapshotBlockerMetrics,
    pub thesis: SnapshotThesisMetrics,
    pub symbols: SnapshotSymbolMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub closed_trades: i64,
    pub total_pnl_usdt: f64,
    pub avg_pnl_pct: f64,
    pub avg_duration_s: f64,
    pub win_rate_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectancyMetrics {
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub neutral_trades: i64,
    pub avg_win_usdt: f64,
    pub avg_win_pct: f64,
    pub avg_loss_usdt: f64,
    pub avg_loss_pct: f64,
    pub payoff_ratio: f64,
    pub expectancy_usdt: f64,
    pub expectancy_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotExitMetrics {
    pub thesis_invalidated_pct: f64,
    pub thesis_invalidated_avg_pnl_pct: f64,
    pub trailing_stop_pct: f64,
    pub trailing_stop_avg_pnl_pct: f64,
    pub dynamic_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSideMetrics {
    pub long_trade_share_pct: f64,
    pub short_trade_share_pct: f64,
    pub long_avg_pnl_pct: f64,
    pub short_avg_pnl_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotBlockerMetrics {
    pub top_reason: String,
    pub top_reason_hits: i64,
    pub consensus_blocks: i64,
    pub volume_blocks: i64,
    pub macd_blocks: i64,
    pub blocker_density: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotThesisMetrics {
    pub total_closes: i64,
    pub top_reason: String,
    pub top_reason_hits: i64,
    pub positive_close_pct: f64,
    pub long_avg_pnl_pct: f64,
    pub short_avg_pnl_pct: f64,
    pub no_alignment_hits: i64,
    pub health_threshold_hits: i64,
    pub opposite_side_hits: i64,
    pub consensus_trend_hits: i64,
    pub price_vs_fast_ema_hits: i64,
    pub btc_regime_hits: i64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSymbolMetrics {
    pub worst_symbol: String,
    pub worst_symbol_pnl_usdt: f64,
    pub best_symbol: String,
    pub best_symbol_pnl_usdt: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExitPressureResult {
    pub severity: String,
    pub reason: String,
    pub thesis_invalidated_pct: f64,
    pub trailing_stop_pct: f64,
    pub dynamic_threshold: f64,
}

pub fn evaluate_exit_pressure(input: &AnalystSnapshot) -> ExitPressureResult {
    let long_avg_pnl = input.sides.long_avg_pnl_pct;
    let short_avg_pnl = input.sides.short_avg_pnl_pct;
    let closed_trades = input.summary.closed_trades as f64;

    let high_threshold = if long_avg_pnl.abs() > 0.3 || short_avg_pnl.abs() > 0.3 {
        70.0
    } else {
        80.0
    };
    let elevated_threshold = if closed_trades < 20.0 { 55.0 } else { 65.0 };

    let (severity, reason, dynamic_threshold) = if input.exits.thesis_invalidated_pct
        >= high_threshold
        && input.exits.trailing_stop_pct <= 12.0
    {
        ("fail", "exit_pressure_high", high_threshold)
    } else if input.exits.thesis_invalidated_pct >= elevated_threshold {
        ("warn", "exit_pressure_elevated", elevated_threshold)
    } else {
        ("pass", "exit_pressure_stable", elevated_threshold)
    };

    ExitPressureResult {
        severity: severity.to_string(),
        reason: reason.to_string(),
        thesis_invalidated_pct: input.exits.thesis_invalidated_pct,
        trailing_stop_pct: input.exits.trailing_stop_pct,
        dynamic_threshold,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectionalBiasResult {
    pub score: f64,
    pub weight: f64,
    pub reason: String,
    pub confidence: f64,
}

pub fn evaluate_directional_bias(input: &AnalystSnapshot) -> DirectionalBiasResult {
    let long_val = input.sides.long_avg_pnl_pct;
    let short_val = input.sides.short_avg_pnl_pct;
    let closed_trades = input.summary.closed_trades as f64;

    let max_abs = (long_val.abs().max(short_val.abs())).max(f64::EPSILON);
    let continuous_score = (long_val - short_val) / max_abs;
    let confidence = (closed_trades / 50.0).min(1.0);

    let score = if long_val >= short_val {
        continuous_score.max(0.0)
    } else {
        continuous_score.abs().min(1.0)
    };

    let reason = if long_val >= short_val {
        "directional_bias_long"
    } else {
        "directional_bias_short"
    };

    DirectionalBiasResult {
        score,
        weight: 100.0,
        reason: reason.to_string(),
        confidence,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryPressureResult {
    pub severity: String,
    pub reason: String,
    pub dominant_gate: String,
    pub blocker_density: f64,
}

pub fn evaluate_entry_pressure(input: &AnalystSnapshot) -> EntryPressureResult {
    let consensus_blocks = input.blockers.consensus_blocks;
    let volume_blocks = input.blockers.volume_blocks;
    let macd_blocks = input.blockers.macd_blocks;
    let closed_trades = input.summary.closed_trades as f64;

    let total_blockers = consensus_blocks + volume_blocks + macd_blocks;
    let blocker_density = if closed_trades > 0.0 {
        total_blockers as f64 / closed_trades
    } else {
        0.0
    };

    let severity = if blocker_density > 1.5 {
        "fail"
    } else if blocker_density > 0.5 {
        "warn"
    } else {
        "pass"
    };

    let (reason, dominant_gate) =
        if consensus_blocks >= volume_blocks && consensus_blocks >= macd_blocks {
            ("entry_pressure_consensus", "consensus")
        } else if volume_blocks >= macd_blocks {
            ("entry_pressure_volume", "volume")
        } else {
            ("entry_pressure_macd", "macd")
        };

    EntryPressureResult {
        severity: severity.to_string(),
        reason: reason.to_string(),
        dominant_gate: dominant_gate.to_string(),
        blocker_density,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ThesisQualityResult {
    pub severity: String,
    pub reason: String,
    pub recommendation: String,
}

pub fn evaluate_thesis_quality(input: &AnalystSnapshot) -> ThesisQualityResult {
    if input.thesis.long_avg_pnl_pct <= -0.20
        && input.thesis.no_alignment_hits >= input.thesis.health_threshold_hits
    {
        return ThesisQualityResult {
            severity: "fail".to_string(),
            reason: "thesis_quality_long_fragile".to_string(),
            recommendation: "harden_long_invalidation_inputs".to_string(),
        };
    }
    if input.thesis.positive_close_pct >= 25.0 {
        return ThesisQualityResult {
            severity: "pass".to_string(),
            reason: "thesis_quality_profit_protective".to_string(),
            recommendation: "preserve_trailing_capture".to_string(),
        };
    }
    if input.thesis.health_threshold_hits > input.thesis.no_alignment_hits {
        return ThesisQualityResult {
            severity: "warn".to_string(),
            reason: "thesis_quality_threshold_driven".to_string(),
            recommendation: "review_health_threshold_balance".to_string(),
        };
    }
    if input.thesis.short_avg_pnl_pct >= input.thesis.long_avg_pnl_pct {
        return ThesisQualityResult {
            severity: "warn".to_string(),
            reason: "thesis_quality_directionally_asymmetric".to_string(),
            recommendation: "review_long_side_guard".to_string(),
        };
    }
    ThesisQualityResult {
        severity: "pass".to_string(),
        reason: "thesis_quality_stable".to_string(),
        recommendation: "keep_current_thesis_policy".to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolRiskResult {
    pub severity: String,
    pub reason: String,
    pub symbol: String,
}

pub fn evaluate_symbol_risk(input: &AnalystSnapshot) -> SymbolRiskResult {
    let worst_symbol = &input.symbols.worst_symbol;
    let worst_symbol_pnl_usdt = input.symbols.worst_symbol_pnl_usdt;

    let (severity, reason) = if worst_symbol_pnl_usdt <= -0.30 {
        ("fail", "symbol_risk_high")
    } else if worst_symbol_pnl_usdt < 0.0 {
        ("warn", "symbol_risk_elevated")
    } else {
        ("pass", "symbol_risk_stable")
    };

    SymbolRiskResult {
        severity: severity.to_string(),
        reason: reason.to_string(),
        symbol: worst_symbol.clone(),
    }
}

pipeline! {
    name: TradeDiagnostics,
    input: AnalystSnapshot,
    steps: [
        step("exit_pressure") { evaluate_exit_pressure(input) },
        step("directional_bias") { evaluate_directional_bias(input) },
        step("entry_pressure") { evaluate_entry_pressure(input) },
        step("thesis_quality") { evaluate_thesis_quality(input) },
        step("symbol_risk") { evaluate_symbol_risk(input) },
    ],
    constraints: []
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_snapshot() -> AnalystSnapshot {
        serde_json::from_value(serde_json::json!({
            "lookback_hours": 24,
            "summary": {
                "closed_trades": 120,
                "total_pnl_usdt": 2.4323,
                "avg_pnl_pct": 0.1716,
                "avg_duration_s": 753.9,
                "win_rate_pct": 44.17
            },
            "expectancy": {
                "winning_trades": 53,
                "losing_trades": 60,
                "neutral_trades": 7,
                "avg_win_usdt": 0.45,
                "avg_win_pct": 1.2,
                "avg_loss_usdt": -0.3,
                "avg_loss_pct": -0.8,
                "payoff_ratio": 1.5,
                "expectancy_usdt": 0.02,
                "expectancy_pct": 0.05
            },
            "exits": {
                "thesis_invalidated_pct": 87.5,
                "thesis_invalidated_avg_pnl_pct": -0.1219,
                "trailing_stop_pct": 10.83,
                "trailing_stop_avg_pnl_pct": 3.0054,
                "dynamic_threshold": 70.0
            },
            "sides": {
                "long_trade_share_pct": 47.5,
                "short_trade_share_pct": 52.5,
                "long_avg_pnl_pct": 0.2952,
                "short_avg_pnl_pct": 0.0553
            },
            "blockers": {
                "top_reason": "long_block_consensus_regime_neutral",
                "top_reason_hits": 1392,
                "consensus_blocks": 2516,
                "volume_blocks": 1476,
                "macd_blocks": 1004,
                "blocker_density": 41.63
            },
            "thesis": {
                "total_closes": 105,
                "top_reason": "thesis_invalidated_no_bullish_alignment",
                "top_reason_hits": 31,
                "positive_close_pct": 18.1,
                "long_avg_pnl_pct": -0.284,
                "short_avg_pnl_pct": -0.041,
                "no_alignment_hits": 67,
                "health_threshold_hits": 28,
                "opposite_side_hits": 10,
                "consensus_trend_hits": 74,
                "price_vs_fast_ema_hits": 42,
                "btc_regime_hits": 19,
                "confidence": 0.45
            },
            "symbols": {
                "worst_symbol": "NEARUSDT",
                "worst_symbol_pnl_usdt": -0.3864,
                "best_symbol": "XRPUSDT",
                "best_symbol_pnl_usdt": 1.1962
            }
        }))
        .expect("base snapshot")
    }

    fn with_sides(
        snapshot: &AnalystSnapshot,
        long_avg_pnl: f64,
        short_avg_pnl: f64,
    ) -> AnalystSnapshot {
        let mut s = snapshot.clone();
        s.sides.long_avg_pnl_pct = long_avg_pnl;
        s.sides.short_avg_pnl_pct = short_avg_pnl;
        s
    }

    fn with_exits(
        snapshot: &AnalystSnapshot,
        thesis_pct: f64,
        trailing_pct: f64,
    ) -> AnalystSnapshot {
        let mut s = snapshot.clone();
        s.exits.thesis_invalidated_pct = thesis_pct;
        s.exits.trailing_stop_pct = trailing_pct;
        s
    }

    fn with_blockers(
        snapshot: &AnalystSnapshot,
        consensus: i64,
        volume: i64,
        macd: i64,
    ) -> AnalystSnapshot {
        let mut s = snapshot.clone();
        s.blockers.consensus_blocks = consensus;
        s.blockers.volume_blocks = volume;
        s.blockers.macd_blocks = macd;
        s
    }

    fn with_thesis(
        snapshot: &AnalystSnapshot,
        long_pnl: f64,
        no_align: i64,
        health: i64,
    ) -> AnalystSnapshot {
        let mut s = snapshot.clone();
        s.thesis.long_avg_pnl_pct = long_pnl;
        s.thesis.no_alignment_hits = no_align;
        s.thesis.health_threshold_hits = health;
        s
    }

    fn with_worst_symbol(snapshot: &AnalystSnapshot, symbol: &str, pnl: f64) -> AnalystSnapshot {
        let mut s = snapshot.clone();
        s.symbols.worst_symbol = symbol.to_string();
        s.symbols.worst_symbol_pnl_usdt = pnl;
        s
    }

    fn with_closed_trades(snapshot: &AnalystSnapshot, n: i64) -> AnalystSnapshot {
        let mut s = snapshot.clone();
        s.summary.closed_trades = n;
        s
    }

    fn with_positive_close(snapshot: &AnalystSnapshot, pct: f64) -> AnalystSnapshot {
        let mut s = snapshot.clone();
        s.thesis.positive_close_pct = pct;
        s
    }

    // ── exit_pressure ──────────────────────────────────────────────────
    #[test]
    fn exit_pressure_high_when_thesis_dominant_and_trailing_low() {
        let snap = with_exits(&base_snapshot(), 85.0, 8.0);
        let result = evaluate_exit_pressure(&snap);
        assert_eq!(result.severity, "fail");
        assert_eq!(result.reason, "exit_pressure_high");
    }

    #[test]
    fn exit_pressure_elevated_when_thesis_elevated_but_trailing_ok() {
        let snap = with_exits(&base_snapshot(), 70.0, 20.0);
        let result = evaluate_exit_pressure(&snap);
        assert_eq!(result.severity, "warn");
        assert_eq!(result.reason, "exit_pressure_elevated");
    }

    #[test]
    fn exit_pressure_stable_when_thesis_low() {
        let snap = with_exits(&base_snapshot(), 40.0, 30.0);
        let result = evaluate_exit_pressure(&snap);
        assert_eq!(result.severity, "pass");
        assert_eq!(result.reason, "exit_pressure_stable");
    }

    #[test]
    fn exit_pressure_high_threshold_lowered_when_large_avg_pnl() {
        let snap = with_sides(&base_snapshot(), 0.5, 0.1);
        let snap = with_exits(&snap, 75.0, 8.0);
        let result = evaluate_exit_pressure(&snap);
        // high_threshold = 70 (because abs(long)=0.5 > 0.3), thesis=75 >= 70, trailing=8 <= 12
        assert_eq!(result.severity, "fail");
        assert_eq!(result.dynamic_threshold, 70.0);
    }

    #[test]
    fn exit_pressure_elevated_threshold_lowered_when_few_trades() {
        let snap = with_closed_trades(&base_snapshot(), 10);
        let snap = with_exits(&snap, 60.0, 15.0);
        let result = evaluate_exit_pressure(&snap);
        // elevated_threshold = 55 (closed_trades < 20), thesis=60 >= 55
        assert_eq!(result.severity, "warn");
        assert_eq!(result.dynamic_threshold, 55.0);
    }

    // ── directional_bias ───────────────────────────────────────────────
    #[test]
    fn directional_bias_long_when_long_outperforms() {
        let snap = with_sides(&base_snapshot(), 1.0, 0.2);
        let result = evaluate_directional_bias(&snap);
        assert_eq!(result.reason, "directional_bias_long");
        assert!(result.score > 0.0);
    }

    #[test]
    fn directional_bias_short_when_short_outperforms() {
        let snap = with_sides(&base_snapshot(), 0.1, 1.0);
        let result = evaluate_directional_bias(&snap);
        assert_eq!(result.reason, "directional_bias_short");
        assert!(result.score > 0.0);
    }

    #[test]
    fn directional_bias_confidence_scales_with_trades() {
        let snap_high = with_closed_trades(&base_snapshot(), 50);
        let snap_low = with_closed_trades(&base_snapshot(), 5);
        assert!(
            evaluate_directional_bias(&snap_high).confidence
                > evaluate_directional_bias(&snap_low).confidence,
            "more trades = higher confidence"
        );
    }

    // ── entry_pressure ─────────────────────────────────────────────────
    #[test]
    fn entry_pressure_fail_when_density_high() {
        let snap = with_blockers(&base_snapshot(), 200, 100, 50);
        let snap = with_closed_trades(&snap, 100);
        let result = evaluate_entry_pressure(&snap);
        // density = (200+100+50)/100 = 3.5 > 1.5
        assert_eq!(result.severity, "fail");
        assert!((result.blocker_density - 3.5).abs() < 1e-9);
    }

    #[test]
    fn entry_pressure_warn_when_density_moderate() {
        let snap = with_blockers(&base_snapshot(), 40, 20, 10);
        let snap = with_closed_trades(&snap, 100);
        let result = evaluate_entry_pressure(&snap);
        // density = (40+20+10)/100 = 0.7, between 0.5 and 1.5
        assert_eq!(result.severity, "warn");
    }

    #[test]
    fn entry_pressure_pass_when_density_low() {
        let snap = with_blockers(&base_snapshot(), 5, 3, 2);
        let snap = with_closed_trades(&snap, 100);
        let result = evaluate_entry_pressure(&snap);
        // density = (5+3+2)/100 = 0.1 < 0.5
        assert_eq!(result.severity, "pass");
    }

    #[test]
    fn entry_pressure_zero_density_when_no_trades() {
        let snap = with_blockers(&base_snapshot(), 10, 5, 3);
        let snap = with_closed_trades(&snap, 0);
        let result = evaluate_entry_pressure(&snap);
        assert_eq!(result.blocker_density, 0.0);
    }

    #[test]
    fn entry_pressure_dominant_gate_consensus_when_highest() {
        let snap = with_blockers(&base_snapshot(), 100, 50, 30);
        let result = evaluate_entry_pressure(&snap);
        assert_eq!(result.dominant_gate, "consensus");
        assert_eq!(result.reason, "entry_pressure_consensus");
    }

    #[test]
    fn entry_pressure_dominant_gate_volume_when_highest() {
        let snap = with_blockers(&base_snapshot(), 30, 100, 50);
        let result = evaluate_entry_pressure(&snap);
        assert_eq!(result.dominant_gate, "volume");
        assert_eq!(result.reason, "entry_pressure_volume");
    }

    #[test]
    fn entry_pressure_dominant_gate_macd_when_highest() {
        let snap = with_blockers(&base_snapshot(), 30, 50, 100);
        let result = evaluate_entry_pressure(&snap);
        assert_eq!(result.dominant_gate, "macd");
        assert_eq!(result.reason, "entry_pressure_macd");
    }

    // ── thesis_quality ─────────────────────────────────────────────────
    #[test]
    fn thesis_quality_long_fragile_when_bad_long_pnl_and_no_alignment_dominant() {
        let snap = with_thesis(&base_snapshot(), -0.25, 50, 20);
        let result = evaluate_thesis_quality(&snap);
        assert_eq!(result.severity, "fail");
        assert_eq!(result.reason, "thesis_quality_long_fragile");
    }

    #[test]
    fn thesis_quality_profit_protective_when_positive_close_high() {
        let mut snap = base_snapshot();
        snap.thesis.long_avg_pnl_pct = -0.1; // avoid long_fragile trigger
        snap.thesis.no_alignment_hits = 5;
        snap.thesis.health_threshold_hits = 10;
        snap.thesis.positive_close_pct = 30.0;
        let result = evaluate_thesis_quality(&snap);
        assert_eq!(result.severity, "pass");
        assert_eq!(result.reason, "thesis_quality_profit_protective");
    }

    #[test]
    fn thesis_quality_threshold_driven_when_health_exceeds_no_alignment() {
        let snap = with_thesis(&base_snapshot(), -0.1, 10, 30);
        let snap = with_positive_close(&snap, 10.0);
        let result = evaluate_thesis_quality(&snap);
        assert_eq!(result.severity, "warn");
        assert_eq!(result.reason, "thesis_quality_threshold_driven");
    }

    #[test]
    fn thesis_quality_asymmetric_when_short_exceeds_long() {
        let mut snap = base_snapshot();
        snap.thesis.long_avg_pnl_pct = -0.05;
        snap.thesis.short_avg_pnl_pct = 0.10;
        snap.thesis.no_alignment_hits = 10;
        snap.thesis.health_threshold_hits = 5;
        snap.thesis.positive_close_pct = 10.0;
        let result = evaluate_thesis_quality(&snap);
        assert_eq!(result.severity, "warn");
        assert_eq!(result.reason, "thesis_quality_directionally_asymmetric");
    }

    #[test]
    fn thesis_quality_stable_when_no_condition_triggers() {
        let mut snap = base_snapshot();
        snap.thesis.long_avg_pnl_pct = 0.05;
        snap.thesis.short_avg_pnl_pct = 0.02;
        snap.thesis.no_alignment_hits = 5;
        snap.thesis.health_threshold_hits = 3;
        snap.thesis.positive_close_pct = 10.0;
        let result = evaluate_thesis_quality(&snap);
        assert_eq!(result.severity, "pass");
        assert_eq!(result.reason, "thesis_quality_stable");
    }

    // ── symbol_risk ────────────────────────────────────────────────────
    #[test]
    fn symbol_risk_fail_when_worst_symbol_pnl_very_negative() {
        let snap = with_worst_symbol(&base_snapshot(), "SOLUSDT", -0.50);
        let result = evaluate_symbol_risk(&snap);
        assert_eq!(result.severity, "fail");
        assert_eq!(result.reason, "symbol_risk_high");
        assert_eq!(result.symbol, "SOLUSDT");
    }

    #[test]
    fn symbol_risk_warn_when_worst_symbol_pnl_moderately_negative() {
        let snap = with_worst_symbol(&base_snapshot(), "SOLUSDT", -0.15);
        let result = evaluate_symbol_risk(&snap);
        assert_eq!(result.severity, "warn");
        assert_eq!(result.reason, "symbol_risk_elevated");
    }

    #[test]
    fn symbol_risk_pass_when_worst_symbol_pnl_non_negative() {
        let snap = with_worst_symbol(&base_snapshot(), "SOLUSDT", 0.10);
        let result = evaluate_symbol_risk(&snap);
        assert_eq!(result.severity, "pass");
        assert_eq!(result.reason, "symbol_risk_stable");
    }
}
