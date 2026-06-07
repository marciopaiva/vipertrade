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
