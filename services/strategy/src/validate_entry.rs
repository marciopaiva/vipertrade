use crate::*;
use serde_json::{json, Value};

#[allow(dead_code)]
pub(crate) fn execute_validate_entry(state: &Value, cfg: &StrategyConfig) -> Result<Value, String> {
    let symbol = get_string(state, "symbol", "UNKNOWN");
    let spread_pct = get_f64(state, "spread_pct", 1.0);
    let volume_24h = get_i64(state, "volume_24h", 0);
    let raw_trend_score = get_f64(state, "trend_score", 0.0);
    let consensus_raw_trend_score = get_f64(state, "consensus_trend_score", raw_trend_score);
    let consensus_trend_score = consensus_raw_trend_score.abs();
    let current_price = get_f64(state, "current_price", 0.0);
    let atr_14 = get_f64(state, "atr_14", 0.0);
    let trend_slope = get_f64(state, "trend_slope", 0.0);
    let consensus_trend_slope = get_f64(state, "consensus_trend_slope", trend_slope);
    let ema_fast = get_f64(state, "ema_fast", 0.0);
    let ema_slow = get_f64(state, "ema_slow", 0.0);
    let consensus_ema_fast = get_f64(state, "consensus_ema_fast", ema_fast);
    let consensus_ema_slow = get_f64(state, "consensus_ema_slow", ema_slow);
    let bollinger_percent_b = get_f64(state, "bollinger_percent_b", 0.5);
    let consensus_bollinger_percent_b =
        get_f64(state, "consensus_bollinger_percent_b", bollinger_percent_b);
    let consensus_adx_14 = get_f64(state, "consensus_adx_14", 0.0);
    let bollinger_bandwidth = get_f64(state, "bollinger_bandwidth", 0.0);
    let consensus_bollinger_bandwidth =
        get_f64(state, "consensus_bollinger_bandwidth", bollinger_bandwidth);
    let rsi_14 = get_f64(state, "rsi_14", 50.0);
    let consensus_rsi_14 = get_f64(state, "consensus_rsi_14", rsi_14);
    let macd_line = get_f64(state, "macd_line", 0.0);
    let macd_signal = get_f64(state, "macd_signal", 0.0);
    let macd_histogram = get_f64(state, "macd_histogram", 0.0);
    let consensus_macd_line = get_f64(state, "consensus_macd_line", macd_line);
    let consensus_macd_signal = get_f64(state, "consensus_macd_signal", macd_signal);
    let consensus_macd_histogram = get_f64(state, "consensus_macd_histogram", macd_histogram);
    let volume_ratio = get_f64(state, "volume_ratio", 0.0);
    let consensus_volume_ratio = get_f64(state, "consensus_volume_ratio", volume_ratio);
    let btc_regime = get_string(state, "btc_regime", "neutral");
    let btc_trend_score = get_f64(state, "btc_trend_score", 0.0);
    let btc_consensus_count = get_i64(state, "btc_consensus_count", 0);
    let regime = get_string(state, "regime", "neutral");
    let exchanges_available = get_i64(state, "exchanges_available", 0);
    let bybit_regime = get_string(state, "bybit_regime", "neutral");
    let bullish_exchanges = get_i64(state, "bullish_exchanges", 0);
    let bearish_exchanges = get_i64(state, "bearish_exchanges", 0);
    let entry_side = if raw_trend_score >= 0.0 {
        "long"
    } else {
        "short"
    };
    let (rsi_min, rsi_max) = cfg.rsi_bounds_for_side(&symbol, entry_side);
    let btc_macro_penalty = if cfg.require_btc_macro_alignment() {
        let Some(penalty) = cfg.btc_macro_penalty_for_side(
            &symbol,
            entry_side,
            &btc_regime,
            btc_trend_score,
            btc_consensus_count,
        ) else {
            return Ok(json!(false));
        };
        penalty
    } else {
        0.0
    };
    let atr_pct = if current_price > 0.0 {
        atr_14 / current_price
    } else {
        1.0
    };
    let consensus_long_ok = if cfg.require_multi_exchange_consensus() {
        bullish_exchanges >= 2 && bearish_exchanges == 0 && exchanges_available >= 2
    } else {
        bybit_regime.eq_ignore_ascii_case("bullish") || regime.eq_ignore_ascii_case("bullish")
    };
    let consensus_short_ok = if cfg.require_multi_exchange_consensus() {
        bearish_exchanges >= 2 && bullish_exchanges == 0 && exchanges_available >= 2
    } else {
        bybit_regime.eq_ignore_ascii_case("bearish") || regime.eq_ignore_ascii_case("bearish")
    };
    let strict_long_ok = cfg.allow_long(&symbol)
        && regime.eq_ignore_ascii_case("bullish")
        && bybit_regime.eq_ignore_ascii_case("bullish")
        && consensus_long_ok
        && consensus_trend_slope > 0.0
        && consensus_ema_fast > consensus_ema_slow
        && current_price >= ema_fast
        && consensus_rsi_14 >= rsi_min
        && consensus_rsi_14 <= rsi_max
        && consensus_macd_line > consensus_macd_signal
        && consensus_macd_histogram > 0.0
        && consensus_volume_ratio >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
        && consensus_trend_score
            >= (cfg.min_trend_score_for_side(&symbol, entry_side) + btc_macro_penalty);

    let directional_ok = if raw_trend_score >= 0.0 {
        cfg.allow_long(&symbol)
            && if cfg.permissive_entry() {
                (bybit_regime.eq_ignore_ascii_case("bullish")
                    || regime.eq_ignore_ascii_case("bullish")
                    || consensus_raw_trend_score >= 0.0)
                    && consensus_trend_slope >= 0.0
                    && consensus_ema_fast >= consensus_ema_slow
                    && current_price > 0.0
                    && current_price >= ema_slow
                    && consensus_rsi_14 >= rsi_min
                    && consensus_rsi_14 <= rsi_max
                    && consensus_macd_line >= consensus_macd_signal
                    && consensus_macd_histogram >= 0.0
                    && consensus_volume_ratio >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                    && consensus_trend_score
                        >= (cfg.min_trend_score_for_side(&symbol, entry_side) + btc_macro_penalty)
            } else {
                strict_long_ok
            }
    } else {
        cfg.allow_short(&symbol)
            && if cfg.permissive_entry() {
                (bybit_regime.eq_ignore_ascii_case("bearish")
                    || regime.eq_ignore_ascii_case("bearish")
                    || consensus_raw_trend_score < 0.0)
                    && consensus_trend_slope <= 0.0
                    && consensus_ema_fast <= consensus_ema_slow
                    && current_price > 0.0
                    && current_price <= ema_slow
                    && consensus_rsi_14 >= rsi_min
                    && consensus_rsi_14 <= rsi_max
                    && consensus_macd_line <= consensus_macd_signal
                    && consensus_macd_histogram <= 0.0
                    && consensus_volume_ratio >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                    && consensus_trend_score
                        >= (cfg.min_trend_score_for_side(&symbol, entry_side) + btc_macro_penalty)
            } else {
                regime.eq_ignore_ascii_case("bearish")
                    && bybit_regime.eq_ignore_ascii_case("bearish")
                    && consensus_short_ok
                    && consensus_trend_slope < 0.0
                    && consensus_ema_fast < consensus_ema_slow
                    && current_price <= ema_fast
                    && consensus_rsi_14 >= rsi_min
                    && consensus_rsi_14 <= rsi_max
                    && consensus_macd_line < consensus_macd_signal
                    && consensus_macd_histogram < 0.0
                    && consensus_volume_ratio >= cfg.min_volume_ratio_for_side(&symbol, entry_side)
                    && consensus_trend_score
                        >= (cfg.min_trend_score_for_side(&symbol, entry_side) + btc_macro_penalty)
            }
    };
    let percent_b_limit = cfg.percent_b_limit_for_side(&symbol, entry_side);
    let percent_b_ok = if entry_side.eq_ignore_ascii_case("short") {
        consensus_bollinger_percent_b >= percent_b_limit
    } else {
        consensus_bollinger_percent_b <= percent_b_limit
    };
    let directional_ok = directional_ok && percent_b_ok;
    let adx_ok = consensus_adx_14 >= cfg.min_adx(&symbol);
    let directional_ok = directional_ok && adx_ok;
    let max_spread_pct = cfg.max_spread_pct(&symbol);
    let min_volume_24h = cfg.min_volume_24h_usdt(&symbol);
    let max_atr_pct = cfg.max_atr_pct(&symbol);
    let min_volume_ratio = cfg.min_volume_ratio_for_side(&symbol, entry_side);
    let min_trend_score = cfg.min_trend_score_for_side(&symbol, entry_side) + btc_macro_penalty;

    let mut components = Vec::new();
    let directional_bias = if entry_side == "long" { 1.0 } else { -1.0 };
    let consensus_regime_score = directional_points(
        &regime,
        if entry_side == "long" {
            "bullish"
        } else {
            "bearish"
        },
        if entry_side == "long" {
            "bearish"
        } else {
            "bullish"
        },
        1,
    ) as f64;
    let ew = |key: &str, def: f64| cfg.entry_weight(key, def);

    push_weighted_entry_component(
        &mut components,
        "consensus_regime",
        consensus_regime_score,
        ew("consensus_regime", 20.0),
    );

    let bybit_regime_score = directional_points(
        &bybit_regime,
        if entry_side == "long" {
            "bullish"
        } else {
            "bearish"
        },
        if entry_side == "long" {
            "bearish"
        } else {
            "bullish"
        },
        1,
    ) as f64;
    push_weighted_entry_component(&mut components, "bybit_regime", bybit_regime_score, ew("bybit_regime", 20.0));

    let consensus_score = if entry_side == "long" {
        if consensus_long_ok {
            1.0
        } else {
            -1.0
        }
    } else if consensus_short_ok {
        1.0
    } else {
        -1.0
    };
    push_weighted_entry_component(&mut components, "exchange_consensus", consensus_score, ew("exchange_consensus", 20.0));

    let trend_slope_score = (consensus_trend_slope * directional_bias).clamp(-1.0, 1.0);
    push_weighted_entry_component(&mut components, "trend_slope", trend_slope_score, ew("trend_slope", 10.0));

    let ema_alignment_score = if entry_side == "long" {
        if consensus_ema_fast > consensus_ema_slow {
            1.0
        } else if consensus_ema_fast < consensus_ema_slow {
            -1.0
        } else {
            0.0
        }
    } else if consensus_ema_fast < consensus_ema_slow {
        1.0
    } else if consensus_ema_fast > consensus_ema_slow {
        -1.0
    } else {
        0.0
    };
    push_weighted_entry_component(&mut components, "ema_alignment", ema_alignment_score, ew("ema_alignment", 10.0));

    let rsi_quality_score = rsi_quality_score_for_side(entry_side, consensus_rsi_14);
    push_weighted_entry_component(&mut components, "rsi_quality", rsi_quality_score, ew("rsi_quality", 6.0));

    let bollinger_bw_score = ((consensus_bollinger_bandwidth - 0.003) / 0.003).clamp(-1.0, 1.0);
    let bollinger_extension_score =
        (bollinger_quality_score_for_side(entry_side, consensus_bollinger_percent_b) * 8.0
            + bollinger_bw_score * 5.0)
            / 13.0;
    push_weighted_entry_component(
        &mut components,
        "bollinger_extension",
        bollinger_extension_score,
        ew("bollinger_extension", 13.0),
    );

    let macd_cross_score = if entry_side == "long" {
        if consensus_macd_line > consensus_macd_signal {
            1.0
        } else if consensus_macd_line < consensus_macd_signal {
            -1.0
        } else {
            0.0
        }
    } else if consensus_macd_line < consensus_macd_signal {
        1.0
    } else if consensus_macd_line > consensus_macd_signal {
        -1.0
    } else {
        0.0
    };
    let macd_hist_score = (consensus_macd_histogram * directional_bias).clamp(-1.0, 1.0);
    let macd_score = (macd_cross_score * 10.0 + macd_hist_score * 5.0) / 15.0;
    push_weighted_entry_component(&mut components, "macd_cross", macd_score, ew("macd_cross", 15.0));

    let macd_quality_score = macd_quality_score_for_side(
        entry_side,
        consensus_macd_line,
        consensus_macd_signal,
        consensus_macd_histogram,
    );
    push_weighted_entry_component(&mut components, "macd_quality", macd_quality_score, ew("macd_quality", 6.0));

    let entry_confluence_score =
        ((rsi_quality_score + macd_quality_score + bollinger_extension_score) / 3.0)
            .clamp(-1.0, 1.0);
    push_weighted_entry_component(
        &mut components,
        "entry_confluence",
        entry_confluence_score,
        ew("entry_confluence", 8.0),
    );

    let volume_ratio_score = if min_volume_ratio > 0.0 {
        (consensus_volume_ratio / min_volume_ratio - 1.0).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    push_weighted_entry_component(&mut components, "volume_ratio", volume_ratio_score, ew("volume_ratio", 5.0));

    let trend_score_ratio = if min_trend_score > 0.0 {
        (consensus_trend_score / min_trend_score - 1.0).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    push_weighted_entry_component(&mut components, "trend_score", trend_score_ratio, ew("trend_score", 10.0));

    let entry_raw_score: i32 = components.iter().map(|c| c.contribution).sum();
    let entry_clamped_score = clamp_i32(entry_raw_score, -100, 100);
    let breakdown = EntryPolicyBreakdown {
        raw_score: entry_raw_score,
        clamped_score: entry_clamped_score,
        components,
    };
    let passed = spread_pct <= max_spread_pct
        && volume_24h >= min_volume_24h
        && atr_pct <= max_atr_pct
        && directional_ok;
    let reason = if passed {
        format!("entry_validated_{}", entry_policy_summary(&breakdown))
    } else if cfg.require_btc_macro_alignment()
        && cfg
            .btc_macro_penalty_for_side(
                &symbol,
                entry_side,
                &btc_regime,
                btc_trend_score,
                btc_consensus_count,
            )
            .is_none()
    {
        format!("{}_block_btc_macro_misaligned", entry_side)
    } else if spread_pct > max_spread_pct {
        format!("{}_block_spread", entry_side)
    } else if volume_24h < min_volume_24h {
        format!("{}_block_volume_24h", entry_side)
    } else if atr_pct > max_atr_pct {
        format!("{}_block_atr_pct", entry_side)
    } else if raw_trend_score >= 0.0 && !cfg.allow_long(&symbol) {
        "long_block_disabled".to_string()
    } else if raw_trend_score < 0.0 && !cfg.allow_short(&symbol) {
        "short_block_disabled".to_string()
    } else if raw_trend_score >= 0.0 && !regime.eq_ignore_ascii_case("bullish") {
        format!("long_block_consensus_regime_{}", regime.to_lowercase())
    } else if raw_trend_score < 0.0 && !regime.eq_ignore_ascii_case("bearish") {
        format!("short_block_consensus_regime_{}", regime.to_lowercase())
    } else if raw_trend_score >= 0.0 && !bybit_regime.eq_ignore_ascii_case("bullish") {
        format!("long_block_bybit_regime_{}", bybit_regime.to_lowercase())
    } else if raw_trend_score < 0.0 && !bybit_regime.eq_ignore_ascii_case("bearish") {
        format!("short_block_bybit_regime_{}", bybit_regime.to_lowercase())
    } else if raw_trend_score >= 0.0 && !consensus_long_ok {
        format!(
            "long_block_consensus_{}_of_{}",
            bullish_exchanges, exchanges_available
        )
    } else if raw_trend_score < 0.0 && !consensus_short_ok {
        format!(
            "short_block_consensus_{}_of_{}",
            bearish_exchanges, exchanges_available
        )
    } else if raw_trend_score >= 0.0 && consensus_trend_slope <= 0.0 {
        format!("long_block_trend_slope_{:.5}_lte_0", consensus_trend_slope)
    } else if raw_trend_score < 0.0 && consensus_trend_slope >= 0.0 {
        format!("short_block_trend_slope_{:.5}_gte_0", consensus_trend_slope)
    } else if raw_trend_score >= 0.0 && consensus_ema_fast <= consensus_ema_slow {
        "long_block_ema_alignment".to_string()
    } else if raw_trend_score < 0.0 && consensus_ema_fast >= consensus_ema_slow {
        "short_block_ema_alignment".to_string()
    } else if raw_trend_score >= 0.0 && current_price < ema_fast {
        format!(
            "long_block_price_{:.5}_lt_fast_ema_{:.5}",
            current_price, ema_fast
        )
    } else if raw_trend_score < 0.0 && current_price > ema_fast {
        format!(
            "short_block_price_{:.5}_gt_fast_ema_{:.5}",
            current_price, ema_fast
        )
    } else if raw_trend_score >= 0.0
        && consensus_bollinger_percent_b > 1.08
        && consensus_bollinger_bandwidth < 0.012
    {
        format!(
            "long_block_bollinger_overstretch_pb_{:.3}_bw_{:.4}",
            consensus_bollinger_percent_b, consensus_bollinger_bandwidth
        )
    } else if raw_trend_score < 0.0
        && consensus_bollinger_percent_b < -0.08
        && consensus_bollinger_bandwidth < 0.012
    {
        format!(
            "short_block_bollinger_overstretch_pb_{:.3}_bw_{:.4}",
            consensus_bollinger_percent_b, consensus_bollinger_bandwidth
        )
    } else if consensus_rsi_14 < rsi_min || consensus_rsi_14 > rsi_max {
        format!(
            "{}_block_rsi_{:.2}_outside_{:.2}_{:.2}",
            entry_side, consensus_rsi_14, rsi_min, rsi_max
        )
    } else if raw_trend_score >= 0.0 && consensus_macd_line <= consensus_macd_signal {
        "long_block_macd_cross".to_string()
    } else if raw_trend_score < 0.0 && consensus_macd_line >= consensus_macd_signal {
        "short_block_macd_cross".to_string()
    } else if raw_trend_score >= 0.0 && consensus_macd_histogram <= 0.0 {
        format!("long_block_macd_hist_{:.6}_lte_0", consensus_macd_histogram)
    } else if raw_trend_score < 0.0 && consensus_macd_histogram >= 0.0 {
        format!(
            "short_block_macd_hist_{:.6}_gte_0",
            consensus_macd_histogram
        )
    } else if consensus_volume_ratio < min_volume_ratio {
        format!(
            "{}_block_volume_ratio_{:.2}_lt_{:.2}",
            entry_side, consensus_volume_ratio, min_volume_ratio
        )
    } else if consensus_trend_score < min_trend_score {
        format!(
            "{}_block_trend_score_{:.3}_lt_{:.3}",
            entry_side, consensus_trend_score, min_trend_score
        )
    } else if !directional_ok {
        format!("{}_block_directional_checks", entry_side)
    } else {
        "risk_constraints_not_met".to_string()
    };
    Ok(json!({
        "passed": passed,
        "severity": if passed { "info" } else { "error" },
        "reason": reason,
        "side": entry_side,
        "entry_score": breakdown.clamped_score,
        "entry_breakdown": {
            "raw_score": breakdown.raw_score,
            "clamped_score": breakdown.clamped_score,
            "components": breakdown.components
        }
    }))
}

pub(crate) fn step_validate_entry(input: &StrategyInput) -> Value {
    if let Some(cfg) = real_cfg() {
        return run_steps_through(input, &cfg, "validate_entry");
    }
    json!({
        "passed": true,
        "severity": "info",
        "reason": "entry_validated",
        "side": "long",
        "entry_score": 60.0,
        "symbol": input.symbol,
        "entry_breakdown": {
            "raw_score": 60, "clamped_score": 60, "components": []
        }
    })
}

pub(crate) fn push_weighted_entry_component(
    components: &mut Vec<EntryPolicyComponent>,
    reason: &'static str,
    score: f64,
    weight: f64,
) {
    let contribution = weighted_contribution(score, weight);
    if contribution != 0 {
        components.push(EntryPolicyComponent {
            reason,
            score,
            weight,
            contribution,
        });
    }
}

pub(crate) fn rsi_quality_score_for_side(side: &str, rsi: f64) -> f64 {
    if side.eq_ignore_ascii_case("long") {
        if rsi < 30.0 {
            -0.5
        } else if rsi < 45.0 {
            0.5
        } else if rsi < 60.0 {
            1.0
        } else if rsi < 70.0 {
            0.35
        } else {
            -0.5
        }
    } else if rsi > 70.0 {
        -0.5
    } else if rsi > 55.0 {
        0.5
    } else if rsi > 40.0 {
        1.0
    } else if rsi > 30.0 {
        0.35
    } else {
        -0.5
    }
}

pub(crate) fn bollinger_quality_score_for_side(side: &str, percent_b: f64) -> f64 {
    if side.eq_ignore_ascii_case("long") {
        if percent_b > 1.05 {
            -1.0
        } else if percent_b > 0.90 {
            -0.35
        } else if (0.45..=0.85).contains(&percent_b) {
            1.0
        } else if (0.25..0.45).contains(&percent_b) {
            0.45
        } else if percent_b < 0.15 {
            -0.35
        } else {
            0.0
        }
    } else if percent_b < -0.05 {
        -1.0
    } else if percent_b < 0.10 {
        -0.35
    } else if (0.15..=0.55).contains(&percent_b) {
        1.0
    } else if (0.55..=0.75).contains(&percent_b) {
        0.45
    } else if percent_b > 0.85 {
        -0.35
    } else {
        0.0
    }
}

pub(crate) fn macd_quality_score_for_side(
    side: &str,
    macd_line: f64,
    macd_signal: f64,
    macd_histogram: f64,
) -> f64 {
    let favorable_crossover = if side.eq_ignore_ascii_case("long") {
        macd_line > macd_signal
    } else {
        macd_line < macd_signal
    };
    let favorable_histogram = if side.eq_ignore_ascii_case("long") {
        macd_histogram > 0.0
    } else {
        macd_histogram < 0.0
    };
    if !favorable_crossover {
        -1.0
    } else if favorable_histogram && macd_histogram.abs() >= 0.0001 {
        1.0
    } else if favorable_histogram {
        0.6
    } else {
        0.2
    }
}

pub(crate) fn entry_policy_summary(breakdown: &EntryPolicyBreakdown) -> String {
    let mut components = breakdown.components.clone();
    components.sort_by_key(|c| c.contribution.abs());
    components.reverse();
    let reasons: Vec<String> = components
        .into_iter()
        .take(3)
        .map(|c| {
            format!(
                "{}:{:.3}x{:.1}={}",
                c.reason, c.score, c.weight, c.contribution
            )
        })
        .collect();
    if reasons.is_empty() {
        format!(
            "entry_raw_{}_clamped_{}",
            breakdown.raw_score, breakdown.clamped_score
        )
    } else {
        format!(
            "entry_raw_{}_clamped_{}_{}",
            breakdown.raw_score,
            breakdown.clamped_score,
            reasons.join("__")
        )
    }
}
