//! Deterministic backtest harness (#37).
//!
//! Replays the recorded `StrategyInput` corpus (`tupa_audit_logs.input_data`)
//! through the live decision (`run_steps_through`) and exit
//! (`evaluate_open_trade_exit`) logic, simulating the position lifecycle to
//! produce reproducible performance metrics for a given config. Because it
//! reads the exact inputs the strategy consumed and runs the real code paths,
//! two configs can be compared on identical data (unlike noisy live A/B).
//!
//! The core `simulate` is pure (no I/O) and unit-tested; `run_backtest_cli`
//! wraps it with DB fetch + config load and prints a report. Invoked via
//! `viper backtest` / `VIPER_ROLE=backtest`.

use crate::{
    evaluate_open_trade_exit, run_steps_through, OpenTradeSnapshot, StrategyConfig, StrategyInput,
};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

/// One replay tick: a timestamped market input for a single symbol.
pub struct Tick {
    pub ts: DateTime<Utc>,
    pub input: StrategyInput,
}

#[derive(Debug, Default, Clone)]
pub struct BacktestReport {
    pub ticks: usize,
    pub opened: usize,
    pub closed: usize,
    pub net_pnl: f64,
    pub wins: usize,
    pub losses: usize,
    /// close_reason -> (count, net pnl)
    pub by_reason: BTreeMap<String, (usize, f64)>,
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
pub(crate) fn simulate(ticks: &[Tick], cfg: &StrategyConfig) -> BacktestReport {
    let mut rep = BacktestReport::default();
    // One open position per symbol (mirrors the single-position-per-symbol live model).
    let mut open: BTreeMap<String, OpenTradeSnapshot> = BTreeMap::new();

    for t in ticks {
        rep.ticks += 1;
        let symbol = t.input.symbol.clone();
        let price = tick_price(&t.input);
        if price <= 0.0 {
            continue;
        }

        if let Some(pos) = open.get_mut(&symbol) {
            let eval = evaluate_open_trade_exit(&symbol, price, pos, cfg, None);
            let is_close = eval
                .decision
                .as_ref()
                .map(|d| d.action.starts_with("CLOSE_"))
                .unwrap_or(false);
            if is_close {
                let pnl = realized_pnl(&pos.side, pos.entry_price, price, pos.quantity);
                rep.closed += 1;
                rep.net_pnl += pnl;
                if pnl > 0.0 {
                    rep.wins += 1;
                } else if pnl < 0.0 {
                    rep.losses += 1;
                }
                let entry = rep.by_reason.entry(eval.trigger.clone()).or_default();
                entry.0 += 1;
                entry.1 += pnl;
                open.remove(&symbol);
            } else if let Some(tr) = eval.trailing {
                // Thread trailing state forward so the stop ratchets like live.
                pos.trailing_stop_activated = tr.activated;
                pos.trailing_stop_peak_price = tr.peak_price;
                pos.trailing_stop_final_distance_pct = tr.trail_pct;
            }
        } else {
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

fn print_report(rep: &BacktestReport, cfg_label: &str) {
    println!("─ Backtest report ({cfg_label}) ───────────────────────");
    println!("  ticks replayed : {}", rep.ticks);
    println!("  opened         : {}", rep.opened);
    println!(
        "  closed         : {}  (wins {}, losses {})",
        rep.closed, rep.wins, rep.losses
    );
    let wr = if rep.closed > 0 {
        100.0 * rep.wins as f64 / rep.closed as f64
    } else {
        0.0
    };
    println!("  win rate       : {wr:.1}%");
    println!("  net PnL        : {:.4}", rep.net_pnl);
    println!("  by close_reason:");
    for (reason, (n, pnl)) in &rep.by_reason {
        println!("    {reason:<20} n={n:<4} net={pnl:.4}");
    }
}

/// CLI entrypoint (role `backtest`). Reads the input corpus from the DB, loads
/// the same config the strategy uses, runs the simulation and prints a report.
///
/// Env: `DATABASE_URL` (or DB_HOST/DB_PORT/DB_NAME/DB_USER/DB_PASSWORD),
/// `BACKTEST_LIMIT` (default 5000), plus the usual STRATEGY_CONFIG /
/// PROFILE_CONFIG / TRADING_PROFILE / TRADING_MODE.
pub async fn run_backtest_cli() -> Result<(), Box<dyn std::error::Error>> {
    let strategy_config = std::env::var("STRATEGY_CONFIG")
        .unwrap_or_else(|_| "config/trading/pairs.yaml".to_string());
    let profile_config = std::env::var("PROFILE_CONFIG")
        .unwrap_or_else(|_| "config/system/profiles.yaml".to_string());
    let trading_profile = std::env::var("TRADING_PROFILE").unwrap_or_else(|_| "MEDIUM".to_string());
    let trading_mode = std::env::var("TRADING_MODE").unwrap_or_else(|_| "paper".to_string());
    let cfg = StrategyConfig::from_files(
        &strategy_config,
        &profile_config,
        &trading_profile,
        &trading_mode,
    )?;

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let host = std::env::var("DB_HOST").unwrap_or_else(|_| "postgres".to_string());
        let port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
        let name = std::env::var("DB_NAME").unwrap_or_else(|_| "vipertrade".to_string());
        let user = std::env::var("DB_USER").unwrap_or_else(|_| "viper".to_string());
        let pass = std::env::var("DB_PASSWORD").unwrap_or_default();
        format!("postgres://{user}:{pass}@{host}:{port}/{name}")
    });
    let limit: i64 = std::env::var("BACKTEST_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await?;

    // input_data fetched as text (sqlx here has no `json` feature).
    let rows: Vec<(DateTime<Utc>, String)> = sqlx::query_as(
        "SELECT executed_at, input_data::text FROM tupa_audit_logs ORDER BY executed_at ASC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?;

    let ticks: Vec<Tick> = rows
        .into_iter()
        .filter_map(|(ts, json)| {
            serde_json::from_str::<StrategyInput>(&json)
                .ok()
                .map(|input| Tick { ts, input })
        })
        .collect();

    println!("Loaded {} replay ticks from tupa_audit_logs", ticks.len());
    let report = simulate(&ticks, &cfg);
    print_report(&report, &format!("{trading_profile}/{trading_mode}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        let rep = simulate(&[mk(0.0), mk(-1.0)], &cfg);
        assert_eq!(rep.ticks, 2);
        assert_eq!(rep.opened, 0);
        assert_eq!(rep.closed, 0);
    }
}
