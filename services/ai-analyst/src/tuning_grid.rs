//! Deterministic tuning-grid analysis for `POST /analyze/tuning` (Format A).
//!
//! The sweep grid is hardcoded with FULL `mode_profiles.PAPER.` override paths so a
//! path can never be silently truncated to a no-op (the trap that makes a wrong path
//! return delta 0.0000 and read as "no impact"), and each variant is pre-classified
//! `alpha` vs `exposure` IN CODE. The narration layer (OpenRouter) is handed these
//! numbers; it never builds a path, runs a sweep, or invents a PnL — which eliminates
//! the path/sign errors a free-form agent makes.

use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use viper_strategy::backtest::{self, SweepResult, Tick};
use viper_strategy::StrategyConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantClass {
    /// Changes entry/exit STRUCTURE (filters, trailing) — a genuine edge change.
    Alpha,
    /// Only scales position size — on a net-losing book this "improves" net by
    /// reducing exposure, not by improving the edge. Never recommend as tuning.
    Exposure,
}

struct GridAxis {
    label: &'static str,
    /// FULL dotted path including the `mode_profiles.PAPER.` prefix.
    path: &'static str,
    class: VariantClass,
    values: &'static [&'static str],
}

/// The canonical tuning grid. Values are strings (the /sweep override contract) and
/// paths are absolute from the profile root — never truncate these.
const GRID: &[GridAxis] = &[
    GridAxis {
        label: "trailing activate_after_profit_pct",
        path: "mode_profiles.PAPER.trailing_stop.activate_after_profit_pct",
        class: VariantClass::Alpha,
        values: &["0.001", "0.0015", "0.002"],
    },
    GridAxis {
        label: "trailing initial_trail_pct",
        path: "mode_profiles.PAPER.trailing_stop.initial_trail_pct",
        class: VariantClass::Alpha,
        values: &["0.0004", "0.0006", "0.0008"],
    },
    GridAxis {
        label: "min_trend_score_short",
        path: "mode_profiles.PAPER.min_trend_score_short",
        class: VariantClass::Alpha,
        values: &["0.5", "0.55", "0.6"],
    },
    GridAxis {
        label: "min_adx",
        path: "mode_profiles.PAPER.min_adx",
        class: VariantClass::Alpha,
        values: &["18", "20", "22", "25"],
    },
    GridAxis {
        label: "risk.max_position_wallet_pct",
        path: "mode_profiles.PAPER.risk.max_position_wallet_pct",
        class: VariantClass::Exposure,
        values: &["0.06", "0.08", "0.16"],
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct GridVariant {
    pub axis: String,
    pub path: String,
    pub value: String,
    pub class: VariantClass,
    pub delta_net_pnl: f64,
    pub net_pnl: f64,
    pub closed: usize,
    pub wins: usize,
    pub losses: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolPerf {
    pub symbol: String,
    pub trades: usize,
    pub net_pnl: f64,
    pub wins: usize,
    pub win_rate_pct: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Substitution {
    pub drop_candidate: Option<String>,
    pub drop_reason: Option<String>,
    /// Disabled candidates — NO corpus, so a substitute is a hypothesis to validate,
    /// never a backtested PnL claim.
    pub pool: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BaselineSummary {
    pub net_pnl: f64,
    pub closed: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate_pct: f64,
    pub by_reason: BTreeMap<String, (usize, f64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuningGridResult {
    pub corpus_ticks: usize,
    pub baseline: BaselineSummary,
    /// Sorted by delta_net_pnl descending (best first).
    pub variants: Vec<GridVariant>,
    /// Sorted by net_pnl ascending (worst first) — the pruning scoreboard.
    pub by_symbol: Vec<SymbolPerf>,
    pub substitution: Substitution,
    /// The authoritative recommendation, decided in code: the best ALPHA variant with
    /// a positive delta. `None` when no alpha variant improves on the corpus. The LLM
    /// narrates this — it does not pick it (and exposure variants are never picked).
    pub recommended: Option<GridVariant>,
}

/// One override-set per grid value, in GRID iteration order so results map back to
/// their axis/class metadata.
fn grid_overrides() -> Vec<Vec<(String, String)>> {
    let mut out = Vec::new();
    for axis in GRID {
        for value in axis.values {
            out.push(vec![(axis.path.to_string(), value.to_string())]);
        }
    }
    out
}

/// Run the full deterministic grid and assemble the structured result. Pure (no I/O):
/// the caller loads the corpus + config. Same corpus + config => same result.
pub fn run(ticks: &[Tick], cfg: &StrategyConfig) -> TuningGridResult {
    let overrides = grid_overrides();
    let sweep: SweepResult = backtest::run_sweep(ticks, cfg, &overrides);

    // Replay GRID order to recover each variant's axis/class/value metadata.
    let meta: Vec<(&'static str, &'static str, VariantClass, &'static str)> = GRID
        .iter()
        .flat_map(|axis| {
            axis.values
                .iter()
                .map(move |value| (axis.label, axis.path, axis.class, *value))
        })
        .collect();

    let mut variants: Vec<GridVariant> = sweep
        .variants
        .iter()
        .zip(meta.iter())
        .map(|(sv, (label, path, class, value))| GridVariant {
            axis: label.to_string(),
            path: path.to_string(),
            value: value.to_string(),
            class: *class,
            delta_net_pnl: sv.delta_net_pnl,
            net_pnl: sv.report.net_pnl,
            closed: sv.report.closed,
            wins: sv.report.wins,
            losses: sv.report.losses,
        })
        .collect();
    variants.sort_by(|a, b| {
        b.delta_net_pnl
            .partial_cmp(&a.delta_net_pnl)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let universe: HashMap<String, bool> = cfg.symbol_universe().into_iter().collect();
    let mut by_symbol: Vec<SymbolPerf> = sweep
        .baseline
        .by_symbol
        .iter()
        .map(|(symbol, (trades, net, wins))| SymbolPerf {
            symbol: symbol.clone(),
            trades: *trades,
            net_pnl: *net,
            wins: *wins,
            win_rate_pct: if *trades > 0 {
                *wins as f64 / *trades as f64 * 100.0
            } else {
                0.0
            },
            enabled: universe.get(symbol).copied().unwrap_or(true),
        })
        .collect();
    by_symbol.sort_by(|a, b| {
        a.net_pnl
            .partial_cmp(&b.net_pnl)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Drop candidate = worst realized PnL with a non-trivial trade count.
    let drop = by_symbol
        .iter()
        .find(|s| s.trades >= 3 && s.net_pnl < 0.0)
        .cloned();
    let pool: Vec<String> = cfg
        .symbol_universe()
        .into_iter()
        .filter(|(_, enabled)| !enabled)
        .map(|(symbol, _)| symbol)
        .collect();
    let substitution = Substitution {
        drop_candidate: drop.as_ref().map(|s| s.symbol.clone()),
        drop_reason: drop.as_ref().map(|s| {
            format!(
                "worst realized PnL: {:.4} over {} trades ({:.0}% win)",
                s.net_pnl, s.trades, s.win_rate_pct
            )
        }),
        pool,
    };

    // Authoritative recommendation: best alpha with a positive delta (variants are
    // already sorted by delta desc, so the first qualifying one is the best).
    let recommended = variants
        .iter()
        .find(|v| v.class == VariantClass::Alpha && v.delta_net_pnl > 0.0)
        .cloned();

    let b = &sweep.baseline;
    TuningGridResult {
        corpus_ticks: ticks.len(),
        baseline: BaselineSummary {
            net_pnl: b.net_pnl,
            closed: b.closed,
            wins: b.wins,
            losses: b.losses,
            win_rate_pct: if b.closed > 0 {
                b.wins as f64 / b.closed as f64 * 100.0
            } else {
                0.0
            },
            by_reason: b.by_reason.clone(),
        },
        variants,
        by_symbol,
        substitution,
        recommended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_paths_are_fully_qualified() {
        // The whole point of Format A: a path can never be truncated to a no-op.
        let overrides = grid_overrides();
        assert_eq!(overrides.len(), 16, "3+3+3+4+3 grid values");
        for set in &overrides {
            for (path, _value) in set {
                assert!(
                    path.starts_with("mode_profiles.PAPER."),
                    "path not fully qualified: {path}"
                );
            }
        }
    }

    #[test]
    fn sizing_is_exposure_trailing_is_alpha() {
        let sizing = GRID
            .iter()
            .find(|a| a.path.contains("max_position_wallet_pct"))
            .expect("sizing axis");
        assert_eq!(sizing.class, VariantClass::Exposure);
        let trailing = GRID
            .iter()
            .find(|a| a.path.contains("activate_after_profit_pct"))
            .expect("trailing axis");
        assert_eq!(trailing.class, VariantClass::Alpha);
    }
}
