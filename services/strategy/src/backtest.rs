//! Deterministic backtest harness (#37).
//!
//! Replays the recorded `StrategyInput` corpus (`tupa_audit_logs.input_data`)
//! through the live decision (`run_steps_through`) and exit
//! (`evaluate_open_trade_exit`) logic, simulating the position lifecycle to
//! produce reproducible performance metrics for a given config. Because it
//! reads the exact inputs the strategy consumed and runs the real code paths,
//! two configs can be compared on identical data (unlike noisy live A/B).
//!
//! The core `simulate` is pure (no I/O) and unit-tested. Exposed via the
//! ai-analyst `/sweep` endpoint and the `SweepResult` / `SweepVariant` API.

use crate::{
    enforce_open_position_thesis_guard, evaluate_open_trade_exit, run_steps_through,
    OpenTradeSnapshot, StrategyConfig, StrategyInput, ThesisInvalidationState,
};
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};
use viper_domain::MarketSignal;

/// One replay tick: a timestamped market input for a single symbol.
pub struct Tick {
    pub ts: DateTime<Utc>,
    pub input: StrategyInput,
    /// True for full entry-context rows (StrategyInput); false for raw
    /// open-position price ticks, which may only drive exits, never entries.
    pub entry_eligible: bool,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct BacktestReport {
    pub ticks: usize,
    pub opened: usize,
    pub closed: usize,
    pub net_pnl: f64,
    pub wins: usize,
    pub losses: usize,
    /// close_reason -> (count, net pnl)
    pub by_reason: BTreeMap<String, (usize, f64)>,
    /// symbol -> (count, net pnl, wins) — to spot symbols that bleed under the
    /// current config (the stop-loss tail is concentrated by symbol).
    pub by_symbol: BTreeMap<String, (usize, f64, usize)>,
}

/// Absolute realized PnL — price move over the full position quantity (no
/// leverage; matches the corrected executor formula, #36).
fn realized_pnl(side: &str, entry: f64, exit: f64, qty: f64) -> f64 {
    let delta = if side == "Long" {
        exit - entry
    } else {
        entry - exit
    };
    delta * qty
}

fn tick_price(input: &StrategyInput) -> f64 {
    input
        .signal
        .get("current_price")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

/// Deterministic position-lifecycle simulation over chronological ticks.
/// Pure: same ticks + config => same report.
pub fn simulate(ticks: &[Tick], cfg: &StrategyConfig) -> BacktestReport {
    let mut rep = BacktestReport::default();
    // One open position per symbol (mirrors the single-position-per-symbol live model).
    let mut open: BTreeMap<String, OpenTradeSnapshot> = BTreeMap::new();
    // Per-symbol thesis-degradation state, threaded across ticks like the live loop.
    let mut thesis_state: HashMap<String, ThesisInvalidationState> = HashMap::new();

    for t in ticks {
        rep.ticks += 1;
        let symbol = t.input.symbol.clone();
        let price = tick_price(&t.input);
        if price <= 0.0 {
            continue;
        }

        if open.contains_key(&symbol) {
            let pos = &open[&symbol];
            // Capture fields up front so the close path can mutate `open` later.
            let (p_side, p_entry, p_qty) = (pos.side.clone(), pos.entry_price, pos.quantity);

            // Exit precedence mirrors live: SL/TP/trailing first (replay tick time
            // so min_hold is faithful), then stateful thesis invalidation.
            let eval = evaluate_open_trade_exit(&symbol, price, pos, cfg, None);
            let close_reason = match eval.decision.as_ref() {
                // SL/TP/trailing fired.
                Some(d) if d.action.starts_with("CLOSE_") => Some(eval.trigger.clone()),
                // Explicit hold (min_hold / invalid_price): keep the position,
                // and crucially do NOT run thesis — mirrors live, where a Some
                // exit decision takes priority over the thesis guard.
                Some(_) => None,
                // No exit opinion (no_exit / trailing_monitoring): now check thesis.
                None => match serde_json::from_value::<MarketSignal>(t.input.signal.clone()) {
                    Ok(signal) => match enforce_open_position_thesis_guard(
                        &symbol,
                        &signal,
                        pos,
                        cfg,
                        &mut thesis_state,
                    ) {
                        Some(d) if d.action.starts_with("CLOSE_") => {
                            Some("thesis_invalidated".to_string())
                        }
                        _ => None,
                    },
                    Err(_) => None,
                },
            };

            if let Some(reason) = close_reason {
                let pnl = realized_pnl(&p_side, p_entry, price, p_qty);
                rep.closed += 1;
                rep.net_pnl += pnl;
                if pnl > 0.0 {
                    rep.wins += 1;
                } else if pnl < 0.0 {
                    rep.losses += 1;
                }
                let entry = rep.by_reason.entry(reason).or_default();
                entry.0 += 1;
                entry.1 += pnl;
                let sym = rep.by_symbol.entry(symbol.clone()).or_default();
                sym.0 += 1;
                sym.1 += pnl;
                if pnl > 0.0 {
                    sym.2 += 1;
                }
                open.remove(&symbol);
                thesis_state.remove(&symbol);
            } else if let Some(tr) = eval.trailing {
                if let Some(pos) = open.get_mut(&symbol) {
                    // Thread trailing state forward so the stop ratchets like live.
                    pos.trailing_stop_activated = tr.activated;
                    pos.trailing_stop_peak_price = tr.peak_price;
                    pos.trailing_stop_final_distance_pct = tr.trail_pct;
                }
            }
        } else if t.entry_eligible {
            let decision = run_steps_through(&t.input, cfg, "decision");
            let action = decision
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("HOLD");
            if action == "ENTER_LONG" || action == "ENTER_SHORT" {
                let qty = decision
                    .get("quantity")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                if qty > 0.0 {
                    let side = if action == "ENTER_LONG" {
                        "Long"
                    } else {
                        "Short"
                    };
                    open.insert(
                        symbol.clone(),
                        OpenTradeSnapshot {
                            trade_id: format!("bt-{}", rep.opened),
                            side: side.to_string(),
                            quantity: qty,
                            entry_price: price,
                            opened_at: t.ts,
                            trailing_stop_activated: false,
                            trailing_stop_peak_price: price,
                            trailing_stop_final_distance_pct: 0.0,
                        },
                    );
                    rep.opened += 1;
                }
            }
        }
    }
    rep
}

/// Load the replay corpus from `tupa_audit_logs` in chronological order.
/// Reusable by any caller (the CLI, the ai-analyst `/sweep` endpoint) so the
/// corpus-fetch + parse logic lives in one place. `since` restricts to rows
/// at/after a timestamp (e.g. an era cutover); `limit` caps the row count.
pub async fn load_corpus(
    pool: &sqlx::PgPool,
    since: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<Tick>, sqlx::Error> {
    // input_data fetched as text (sqlx here has no `json` feature).
    let rows: Vec<(DateTime<Utc>, String)> = if let Some(since) = since {
        sqlx::query_as(
            "SELECT executed_at, input_data::text FROM tupa_audit_logs \
             WHERE executed_at >= $1 ORDER BY executed_at ASC LIMIT $2",
        )
        .bind(since)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        // No window given: take the MOST RECENT `limit` rows (DESC), then restore
        // chronological order for replay. (ASC + LIMIT would grab the OLDEST rows
        // — the pre-ADX era with no entries — making every sweep empty.)
        let mut rows: Vec<(DateTime<Utc>, String)> = sqlx::query_as(
            "SELECT executed_at, input_data::text FROM tupa_audit_logs ORDER BY executed_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        rows.reverse();
        rows
    };
    Ok(rows
        .into_iter()
        .filter_map(|(ts, json)| parse_tick(ts, &json))
        .collect())
}

/// Clone `base` and apply each `(dotted.path, value)` override to BOTH the
/// global mode profile and every per-symbol block. Lookup precedence varies per
/// parameter (some read global first, some the per-symbol block), so patching
/// everywhere guarantees the override takes effect instead of silently no-op'ing.
pub fn apply_overrides(base: &StrategyConfig, overrides: &[(String, String)]) -> StrategyConfig {
    let mut variant = base.clone();
    for (path, raw) in overrides {
        set_json_path(&mut variant.global, path, raw);
        for pair in variant.pairs.values_mut() {
            set_json_path(pair, path, raw);
        }
    }
    variant
}

/// One config variant in a sweep: its label, its report, and the net-PnL delta
/// versus the baseline run on the same corpus.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SweepVariant {
    pub label: String,
    pub report: BacktestReport,
    pub delta_net_pnl: f64,
}

/// A baseline report plus one report per config variant, all on the same corpus.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SweepResult {
    pub baseline: BacktestReport,
    pub variants: Vec<SweepVariant>,
}

/// Run the baseline config plus one variant per override-set on the same corpus,
/// returning structured results. Deterministic: same ticks + configs => same
/// result. Each variant's label is derived from its overrides (`path=value, …`).
pub fn run_sweep(
    ticks: &[Tick],
    base_cfg: &StrategyConfig,
    variant_overrides: &[Vec<(String, String)>],
) -> SweepResult {
    let baseline = simulate(ticks, base_cfg);
    let variants = variant_overrides
        .iter()
        .map(|overrides| {
            let label = overrides
                .iter()
                .map(|(p, v)| format!("{p}={v}"))
                .collect::<Vec<_>>()
                .join(", ");
            let report = simulate(ticks, &apply_overrides(base_cfg, overrides));
            let delta_net_pnl = report.net_pnl - baseline.net_pnl;
            SweepVariant {
                label,
                report,
                delta_net_pnl,
            }
        })
        .collect();
    SweepResult { baseline, variants }
}

/// Parse an audit `input_data` row into a replay tick. Entry rows are a full
/// `StrategyInput` (nested `signal`); open-position rows store the raw
/// `MarketSignal` — wrap those so the price tick is still captured.
fn parse_tick(ts: DateTime<Utc>, json: &str) -> Option<Tick> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let (input, entry_eligible) = if v.get("signal").is_some() {
        (serde_json::from_value::<StrategyInput>(v).ok()?, true)
    } else {
        // Raw MarketSignal (open-position row): a price tick for exits only.
        (
            StrategyInput {
                symbol: v
                    .get("symbol")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string(),
                temporal: serde_json::Value::Null,
                account_equity_usdt: 0.0,
                config: serde_json::Value::Null,
                signal: v,
            },
            false,
        )
    };
    Some(Tick {
        ts,
        input,
        entry_eligible,
    })
}

/// Set a dotted-path value inside a JSON object, creating intermediate objects.
/// The raw value is parsed as i64, then f64, then bool, else kept as a string.
fn set_json_path(root: &mut serde_json::Value, path: &str, raw: &str) {
    let parsed = raw
        .parse::<i64>()
        .map(serde_json::Value::from)
        .or_else(|_| raw.parse::<f64>().map(serde_json::Value::from))
        .or_else(|_| raw.parse::<bool>().map(serde_json::Value::from))
        .unwrap_or_else(|_| serde_json::Value::from(raw));
    let parts: Vec<&str> = path.split('.').collect();
    let Some((last, parents)) = parts.split_last() else {
        return;
    };
    let mut cur = root;
    for part in parents {
        if !cur.is_object() {
            *cur = serde_json::json!({});
        }
        cur = cur
            .as_object_mut()
            .unwrap()
            .entry(part.to_string())
            .or_insert_with(|| serde_json::json!({}));
    }
    if !cur.is_object() {
        *cur = serde_json::json!({});
    }
    cur.as_object_mut()
        .unwrap()
        .insert(last.to_string(), parsed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_json_path_creates_nested_and_typed_value() {
        let mut root = serde_json::json!({});
        set_json_path(&mut root, "a.b.thesis_ticks", "8");
        assert_eq!(root["a"]["b"]["thesis_ticks"], serde_json::json!(8));
        // overwrites + parses float
        set_json_path(&mut root, "a.b.thesis_ticks", "1.5");
        assert_eq!(root["a"]["b"]["thesis_ticks"], serde_json::json!(1.5));
    }

    #[test]
    fn parse_tick_handles_both_corpus_shapes() {
        // Full StrategyInput row (nested "signal").
        let full = r#"{"symbol":"BTCUSDT","signal":{"current_price":42.0}}"#;
        let t = parse_tick(Utc::now(), full).unwrap();
        assert_eq!(t.input.symbol, "BTCUSDT");
        assert!((tick_price(&t.input) - 42.0).abs() < 1e-9);
        assert!(t.entry_eligible, "full rows may open entries");
        // Raw MarketSignal row (open-position path) — wrapped so price survives,
        // but exit-only (must not open entries).
        let raw = r#"{"symbol":"ETHUSDT","current_price":99.0}"#;
        let t = parse_tick(Utc::now(), raw).unwrap();
        assert_eq!(t.input.symbol, "ETHUSDT");
        assert!((tick_price(&t.input) - 99.0).abs() < 1e-9);
        assert!(!t.entry_eligible, "raw exit ticks must not open entries");
    }

    #[test]
    fn realized_pnl_has_no_leverage_factor() {
        // Long: profit on the up move; Short: profit on the down move.
        assert!((realized_pnl("Long", 100.0, 110.0, 2.0) - 20.0).abs() < 1e-9);
        assert!((realized_pnl("Short", 100.0, 90.0, 2.0) - 20.0).abs() < 1e-9);
        assert!((realized_pnl("Long", 100.0, 90.0, 2.0) + 20.0).abs() < 1e-9);
    }

    #[test]
    fn empty_corpus_yields_empty_report() {
        let cfg = StrategyConfig::sample_for_tests();
        let rep = simulate(&[], &cfg);
        assert_eq!(rep.ticks, 0);
        assert_eq!(rep.opened, 0);
        assert_eq!(rep.closed, 0);
        assert_eq!(rep.net_pnl, 0.0);
    }

    #[test]
    fn ticks_with_invalid_price_are_skipped_without_opening() {
        let cfg = StrategyConfig::sample_for_tests();
        let mk = |price: f64| Tick {
            ts: Utc::now(),
            input: StrategyInput {
                symbol: "BTCUSDT".to_string(),
                temporal: serde_json::json!({}),
                account_equity_usdt: 1000.0,
                config: serde_json::json!({}),
                signal: serde_json::json!({ "current_price": price }),
            },
            entry_eligible: true,
        };
        let rep = simulate(&[mk(0.0), mk(-1.0)], &cfg);
        assert_eq!(rep.ticks, 2);
        assert_eq!(rep.opened, 0);
        assert_eq!(rep.closed, 0);
    }
}
