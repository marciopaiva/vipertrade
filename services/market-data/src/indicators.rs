pub(crate) const REQUIRED_CANDLE_COUNT: usize = 200;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Candle {
    pub open_time_ms: i64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume_quote: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IndicatorBundle {
    pub ema_fast: f64,
    pub ema_slow: f64,
    pub bollinger_upper: f64,
    pub bollinger_middle: f64,
    pub bollinger_lower: f64,
    pub bollinger_bandwidth: f64,
    pub bollinger_percent_b: f64,
    pub rsi_14: f64,
    pub macd_line: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub volume_ratio: f64,
}

pub(crate) fn compute_atr14(candles: &[Candle]) -> f64 {
    if candles.len() < 2 {
        return 0.0;
    }

    let mut trs: Vec<f64> = Vec::with_capacity(candles.len().saturating_sub(1));
    for idx in 1..candles.len() {
        let current = candles[idx];
        let prev_close = candles[idx - 1].close;
        let tr = (current.high - current.low)
            .max((current.high - prev_close).abs())
            .max((current.low - prev_close).abs());
        trs.push(tr.max(0.0));
    }

    let tail = trs.len().min(14);
    if tail == 0 {
        return 0.0;
    }
    let start = trs.len() - tail;
    let sum: f64 = trs[start..].iter().sum();
    sum / tail as f64
}

pub(crate) fn compute_adx14(candles: &[Candle]) -> Option<f64> {
    let period = 14usize;
    if candles.len() < period * 2 + 1 {
        return None;
    }
    let mut tr = Vec::with_capacity(candles.len());
    let mut plus_dm = Vec::with_capacity(candles.len());
    let mut minus_dm = Vec::with_capacity(candles.len());
    for i in 1..candles.len() {
        let c = candles[i];
        let p = candles[i - 1];
        let up = c.high - p.high;
        let down = p.low - c.low;
        plus_dm.push(if up > down && up > 0.0 { up } else { 0.0 });
        minus_dm.push(if down > up && down > 0.0 { down } else { 0.0 });
        let t = (c.high - c.low)
            .max((c.high - p.close).abs())
            .max((c.low - p.close).abs());
        tr.push(t.max(0.0));
    }
    if tr.len() < period * 2 {
        return None;
    }
    let dx = |atr: f64, sp: f64, sm: f64| -> f64 {
        if atr <= 0.0 {
            return 0.0;
        }
        let pdi = 100.0 * sp / atr;
        let mdi = 100.0 * sm / atr;
        let denom = pdi + mdi;
        if denom <= 0.0 {
            0.0
        } else {
            100.0 * (pdi - mdi).abs() / denom
        }
    };
    let mut atr = tr[..period].iter().sum::<f64>();
    let mut sp = plus_dm[..period].iter().sum::<f64>();
    let mut sm = minus_dm[..period].iter().sum::<f64>();
    let mut dxs = vec![dx(atr, sp, sm)];
    let pf = period as f64;
    for i in period..tr.len() {
        atr = atr - atr / pf + tr[i];
        sp = sp - sp / pf + plus_dm[i];
        sm = sm - sm / pf + minus_dm[i];
        dxs.push(dx(atr, sp, sm));
    }
    if dxs.len() < period {
        return None;
    }
    let mut adx = dxs[..period].iter().sum::<f64>() / pf;
    for d in &dxs[period..] {
        adx = (adx * (pf - 1.0) + d) / pf;
    }
    Some(adx)
}

pub(crate) fn compute_rsi14(candles: &[Candle]) -> Option<f64> {
    if candles.len() < 15 {
        return None;
    }

    let start = candles.len() - 15;
    let mut gains = 0.0;
    let mut losses = 0.0;

    for idx in (start + 1)..candles.len() {
        let delta = candles[idx].close - candles[idx - 1].close;
        if delta >= 0.0 {
            gains += delta;
        } else {
            losses += -delta;
        }
    }

    let avg_gain = gains / 14.0;
    let avg_loss = losses / 14.0;

    if avg_loss == 0.0 {
        return Some(100.0);
    }

    let rs = avg_gain / avg_loss;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

pub(crate) fn compute_ema(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.is_empty() || period == 0 {
        return None;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let mut ema = candles[0].close;
    for candle in candles.iter().skip(1) {
        ema = (candle.close * alpha) + (ema * (1.0 - alpha));
    }
    Some(ema)
}

pub(crate) fn compute_macd(candles: &[Candle]) -> Option<(f64, f64, f64)> {
    if candles.len() < 35 {
        return None;
    }

    let mut macd_series = Vec::with_capacity(candles.len());
    for idx in 0..candles.len() {
        let slice = &candles[..=idx];
        let ema12 = compute_ema(slice, 12)?;
        let ema26 = compute_ema(slice, 26)?;
        macd_series.push(ema12 - ema26);
    }

    if macd_series.len() < 9 {
        return None;
    }

    let alpha = 2.0 / (9.0 + 1.0);
    let mut signal = macd_series[0];
    for value in macd_series.iter().skip(1) {
        signal = (value * alpha) + (signal * (1.0 - alpha));
    }
    let macd_line = *macd_series.last()?;
    let histogram = macd_line - signal;
    Some((macd_line, signal, histogram))
}

pub(crate) fn compute_bollinger(candles: &[Candle], period: usize) -> Option<(f64, f64, f64, f64, f64)> {
    if candles.len() < period || period == 0 {
        return None;
    }

    let window = &candles[candles.len() - period..];
    let mean = window.iter().map(|c| c.close).sum::<f64>() / period as f64;
    let variance = window
        .iter()
        .map(|c| {
            let delta = c.close - mean;
            delta * delta
        })
        .sum::<f64>()
        / period as f64;
    let std_dev = variance.sqrt();
    let upper = mean + (2.0 * std_dev);
    let lower = mean - (2.0 * std_dev);
    let bandwidth = if mean.abs() > f64::EPSILON {
        ((upper - lower) / mean).max(0.0)
    } else {
        0.0
    };
    let last_close = window.last()?.close;
    let percent_b = if (upper - lower).abs() > f64::EPSILON {
        (last_close - lower) / (upper - lower)
    } else {
        0.5
    };

    Some((upper, mean, lower, bandwidth, percent_b))
}

pub(crate) fn compute_volume_ratio(candles: &[Candle], lookback: usize) -> Option<f64> {
    if candles.len() < lookback + 1 {
        return None;
    }
    let last = candles.last()?.volume_quote;
    let start = candles.len().saturating_sub(lookback + 1);
    let baseline: Vec<f64> = candles[start..candles.len() - 1]
        .iter()
        .map(|c| c.volume_quote)
        .filter(|v| *v > 0.0)
        .collect();
    if baseline.is_empty() {
        return None;
    }
    let avg = baseline.iter().sum::<f64>() / baseline.len() as f64;
    if avg <= 0.0 {
        return None;
    }
    Some((last / avg).max(0.0))
}

pub(crate) fn compute_indicator_bundle_complete(
    source: &str,
    symbol: &str,
    candles: &[Candle],
) -> Result<IndicatorBundle, String> {
    if candles.len() < REQUIRED_CANDLE_COUNT {
        return Err(format!(
            "{} {} candles incomplete: expected at least {}, got {}",
            source,
            symbol,
            REQUIRED_CANDLE_COUNT,
            candles.len()
        ));
    }

    let ema_fast = compute_ema(candles, 20)
        .ok_or_else(|| format!("{} {} ema_fast incomplete", source, symbol))?;
    let ema_slow = compute_ema(candles, 50)
        .ok_or_else(|| format!("{} {} ema_slow incomplete", source, symbol))?;
    let (
        bollinger_upper,
        bollinger_middle,
        bollinger_lower,
        bollinger_bandwidth,
        bollinger_percent_b,
    ) = compute_bollinger(candles, 20)
        .ok_or_else(|| format!("{} {} bollinger incomplete", source, symbol))?;
    let rsi_14 =
        compute_rsi14(candles).ok_or_else(|| format!("{} {} rsi_14 incomplete", source, symbol))?;
    let (macd_line, macd_signal, macd_histogram) =
        compute_macd(candles).ok_or_else(|| format!("{} {} macd incomplete", source, symbol))?;
    let volume_ratio = compute_volume_ratio(candles, 20)
        .ok_or_else(|| format!("{} {} volume_ratio incomplete", source, symbol))?;

    Ok(IndicatorBundle {
        ema_fast,
        ema_slow,
        bollinger_upper,
        bollinger_middle,
        bollinger_lower,
        bollinger_bandwidth,
        bollinger_percent_b,
        rsi_14,
        macd_line,
        macd_signal,
        macd_histogram,
        volume_ratio,
    })
}

pub(crate) fn composite_trend_score(
    current_price: f64,
    ema_fast: f64,
    ema_slow: f64,
    rsi_14: f64,
    macd_histogram: f64,
    volume_ratio: f64,
) -> f64 {
    let ema_component = if ema_slow > 0.0 {
        ((ema_fast - ema_slow) / ema_slow / 0.003).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let rsi_component = ((rsi_14 - 50.0) / 15.0).clamp(-1.0, 1.0);
    let macd_component = if current_price > 0.0 {
        (macd_histogram / current_price / 0.0015).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let volume_component = ((volume_ratio - 1.0) / 0.5).clamp(0.0, 1.0);
    ((ema_component * 0.4) + (rsi_component * 0.25) + (macd_component * 0.25))
        * (0.8 + 0.2 * volume_component)
}

pub(crate) fn classify_regime(candles: &[Candle], trend_score: f64) -> (&'static str, f64) {
    if candles.len() < 55 {
        return ("neutral", 0.0);
    }

    let ema_fast = match compute_ema(candles, 20) {
        Some(v) => v,
        None => return ("neutral", 0.0),
    };
    let ema_slow = match compute_ema(candles, 50) {
        Some(v) => v,
        None => return ("neutral", 0.0),
    };
    let prev_fast = match compute_ema(&candles[..candles.len() - 1], 20) {
        Some(v) => v,
        None => return ("neutral", 0.0),
    };

    let close = candles.last().map(|c| c.close).unwrap_or(0.0);
    let slope = if prev_fast > 0.0 {
        (ema_fast - prev_fast) / prev_fast
    } else {
        0.0
    };

    let regime = if ema_fast > ema_slow && close >= ema_fast && trend_score > 0.05 && slope > 0.0 {
        "bullish"
    } else if ema_fast < ema_slow && close <= ema_fast && trend_score < -0.05 && slope < 0.0 {
        "bearish"
    } else {
        "neutral"
    };

    (regime, slope)
}
