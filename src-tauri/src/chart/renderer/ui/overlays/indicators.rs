//! Indicator math for on-chart overlay widgets — pure functions over price
//! `Bar`s. Extracted from `chart_widgets.rs` so data computation lives apart
//! from rendering (the UI file should not compute RSI/ATR/breadth/…).


pub(crate) fn compute_autocorrelation(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let n = bars.len();
    if n < period + 2 { return 0.0; }
    let mut returns: Vec<f32> = Vec::with_capacity(period);
    for i in (n - period)..n {
        if bars[i - 1].close > 0.0 {
            returns.push((bars[i].close - bars[i - 1].close) / bars[i - 1].close);
        }
    }
    if returns.len() < 4 { return 0.0; }
    let mean = returns.iter().sum::<f32>() / returns.len() as f32;
    let var: f32 = returns.iter().map(|r| (r - mean).powi(2)).sum::<f32>() / returns.len() as f32;
    if var < 1e-10 { return 0.0; }
    let mut cov = 0.0f32;
    for i in 1..returns.len() {
        cov += (returns[i] - mean) * (returns[i - 1] - mean);
    }
    cov /= (returns.len() - 1) as f32;
    (cov / var).clamp(-1.0, 1.0)
}

// AUDIT 2026-08-02 (AT-017, P1): these two used to carry their OWN math —
// a simple mean over the trailing `period` bars — while
// `chart_renderer::compute` used Wilder smoothing for the same indicators.
//
// Both were on screen at the same time. `chart_widgets.rs:29` does
// `use super::overlays::indicators::*`, so the glob silently bound the
// non-Wilder scalars for the on-chart readout, while the indicator pane
// plotted the Wilder series from `compute.rs`. Same symbol, same period, two
// different numbers, no indication which was which.
//
// Wilder is the correct convention (it is what TradingView, Bloomberg and every
// other platform mean by "RSI 14" / "ATR 14"), and `compute.rs` already
// implements it, so these now delegate. The scalar shape is kept because
// `chart_widgets` genuinely wants a single latest value, not a series — the
// duplication was in the MATH, not in the API.

/// Latest RSI, Wilder-smoothed. Delegates to the canonical series
/// implementation and takes the most recent finite value.
///
/// Returns the neutral 50.0 when there is not enough data, matching the
/// previous contract for callers that render it directly.
pub(crate) fn compute_rsi(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let closes: Vec<f32> = bars.iter().map(|b| b.close).collect();
    crate::chart_renderer::compute::compute_rsi(&closes, period)
        .iter().rev().copied()
        .find(|v| !v.is_nan())
        .unwrap_or(50.0)
}

/// Latest ATR, Wilder-smoothed. Delegates to the canonical series
/// implementation and takes the most recent finite value.
///
/// Returns 0.0 when there is not enough data, matching the previous contract.
pub(crate) fn compute_atr(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let highs:  Vec<f32> = bars.iter().map(|b| b.high).collect();
    let lows:   Vec<f32> = bars.iter().map(|b| b.low).collect();
    let closes: Vec<f32> = bars.iter().map(|b| b.close).collect();
    crate::chart_renderer::compute::compute_atr(&highs, &lows, &closes, period)
        .iter().rev().copied()
        .find(|v| !v.is_nan())
        .unwrap_or(0.0)
}

/// Lookback periods behind the seven trend-grid / RSI-multi rows.
///
/// AUDIT 2026-08-02 (AT-018, P1): these are periods on the PANE'S OWN
/// timeframe, not seven different timeframes. The widgets used to label the
/// rows `["5m","15m","30m","1h","4h","1D","1W"]` regardless, which is wrong in
/// both directions: on a 5m chart the "1W" row is period 140 x 5m ≈ 11.7 hours,
/// and on a 1D chart the "5m" row is period 7 x 1D ≈ 7 days. A trader reading
/// "1W bullish" was being shown something between half a day and seven months
/// depending on the pane.
///
/// Kept as periods-on-current-timeframe (computing seven real timeframes would
/// mean seven source fetches per widget). The labels are now DERIVED from the
/// actual horizon instead of asserted — see [`horizon_label`].
pub(crate) const TREND_GRID_PERIODS: [usize; 7] = [7, 10, 14, 21, 42, 70, 140];

/// Minutes per bar for a timeframe string. `None` for anything unrecognised,
/// so callers can fall back rather than invent a number.
pub(crate) fn timeframe_minutes(tf: &str) -> Option<f32> {
    Some(match tf {
        "1m" => 1.0, "2m" => 2.0, "3m" => 3.0, "5m" => 5.0, "10m" => 10.0,
        "15m" => 15.0, "30m" => 30.0,
        "1h" => 60.0, "2h" => 120.0, "4h" => 240.0,
        "1d" | "1D" => 1440.0,
        "1wk" | "1W" => 10080.0,
        "1mo" | "1M" => 43200.0,
        _ => return None,
    })
}

/// Human label for "`period` bars of `tf`" — the real lookback horizon.
///
/// AT-018: this replaces the hardcoded timeframe labels. It tells the trader
/// what the row actually covers, which is both honest and more useful than a
/// bare period count.
pub(crate) fn horizon_label(tf: &str, period: usize) -> String {
    let Some(per_bar) = timeframe_minutes(tf) else {
        // Unknown timeframe: state the period rather than guess a duration.
        return format!("P{period}");
    };
    let mins = per_bar * period as f32;
    if mins < 60.0 {
        format!("{}m", mins.round() as i64)
    } else if mins < 1440.0 {
        let h = mins / 60.0;
        if (h - h.round()).abs() < 0.05 { format!("{}h", h.round() as i64) }
        else { format!("{h:.1}h") }
    } else if mins < 10080.0 {
        let d = mins / 1440.0;
        if (d - d.round()).abs() < 0.05 { format!("{}D", d.round() as i64) }
        else { format!("{d:.1}D") }
    } else {
        let w = mins / 10080.0;
        if (w - w.round()).abs() < 0.05 { format!("{}W", w.round() as i64) }
        else { format!("{w:.1}W") }
    }
}

pub(crate) fn compute_trend_grid(bars: &[crate::chart_renderer::types::Bar]) -> [[bool; 4]; 7] {
    let n = bars.len();
    let periods = TREND_GRID_PERIODS;
    let mut grid = [[false; 4]; 7];
    for (ti, &p) in periods.iter().enumerate() {
        if n < p + 5 { continue; }
        // Col 0: EMA slope positive
        let ema_now = bars[n-1..n].iter().map(|b| b.close).sum::<f32>();
        let ema_prev = bars[n-3..n-2].iter().map(|b| b.close).sum::<f32>();
        grid[ti][0] = ema_now > ema_prev;
        // Col 1: Close > SMA
        let sma: f32 = bars[n.saturating_sub(p)..n].iter().map(|b| b.close).sum::<f32>() / p.min(n) as f32;
        grid[ti][1] = bars[n-1].close > sma;
        // Col 2: RSI > 50
        grid[ti][2] = compute_rsi(bars, p) > 50.0;
        // Col 3: Higher high
        if n > p + 1 {
            grid[ti][3] = bars[n-1].high > bars[n.saturating_sub(p/2+1)].high;
        }
    }
    grid
}

pub(crate) fn compute_roc_bars(bars: &[crate::chart_renderer::types::Bar]) -> [f32; 8] {
    let n = bars.len();
    let lookbacks = [1, 2, 5, 10, 20, 60, 120, 252];
    let mut roc = [0.0f32; 8];
    for (i, &lb) in lookbacks.iter().enumerate() {
        if n > lb && bars[n - lb - 1].close > 0.0 {
            roc[i] = (bars[n-1].close - bars[n-lb-1].close) / bars[n-lb-1].close * 100.0;
        }
    }
    roc
}

pub(crate) fn compute_vol_shelves(bars: &[crate::chart_renderer::types::Bar]) -> Vec<(f32, f32, bool)> {
    let n = bars.len();
    if n < 20 { return vec![]; }
    let recent = &bars[n.saturating_sub(100)..n];
    let lo = recent.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
    let hi = recent.iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max);
    let range = (hi - lo).max(0.01);
    let bins = 10;
    let mut vol = vec![0.0f32; bins];
    for b in recent {
        let mid = (b.high + b.low) / 2.0;
        let idx = ((mid - lo) / range * (bins - 1) as f32) as usize;
        vol[idx.min(bins - 1)] += b.volume;
    }
    let max_vol = vol.iter().cloned().fold(0.0f32, f32::max).max(1.0);
    let last = bars[n-1].close;
    let mut shelves: Vec<(f32, f32, bool)> = vol.iter().enumerate()
        .filter(|(_, &v)| v > max_vol * 0.3)
        .map(|(i, &v)| {
            let price = lo + (i as f32 + 0.5) * range / bins as f32;
            (price, v / max_vol, price < last)
        }).collect();
    shelves.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    shelves.truncate(5);
    shelves
}

pub(crate) fn compute_confluence(bars: &[crate::chart_renderer::types::Bar], last: f32) -> Vec<(f32, u8, f32)> {
    let n = bars.len();
    if n < 20 || last < 0.01 { return vec![]; }
    let mut levels: Vec<f32> = Vec::new();
    // SMAs
    for p in [20, 50, 100, 200] {
        if n >= p { levels.push(bars[n.saturating_sub(p)..n].iter().map(|b| b.close).sum::<f32>() / p as f32); }
    }
    // Pivots
    let (h, l) = (bars[n.saturating_sub(20)..n].iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max),
                  bars[n.saturating_sub(20)..n].iter().map(|b| b.low).fold(f32::INFINITY, f32::min));
    let pp = (h + l + last) / 3.0;
    levels.extend_from_slice(&[pp, 2.0 * pp - l, 2.0 * pp - h]);
    // Previous highs/lows
    if n > 1 { levels.push(bars[n-2].high); levels.push(bars[n-2].low); }
    // Cluster: group levels within 0.3% of each other
    levels.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut zones: Vec<(f32, u8, f32)> = Vec::new();
    let mut i = 0;
    while i < levels.len() {
        let base = levels[i];
        let mut count = 1u8;
        let mut sum = base;
        while i + (count as usize) < levels.len() && (levels[i + (count as usize)] - base).abs() / last < 0.003 {
            sum += levels[i + (count as usize)]; count += 1;
        }
        if count >= 2 {
            let avg = sum / count as f32;
            zones.push((avg, count, (avg - last).abs() / last * 100.0));
        }
        i += count as usize;
    }
    zones.sort_by(|a, b| b.1.cmp(&a.1));
    zones.truncate(5);
    zones
}

pub(crate) fn compute_bb_width(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 20 { return 0.05; }
    let p = 20;
    let sma: f32 = bars[n-p..n].iter().map(|b| b.close).sum::<f32>() / p as f32;
    let var: f32 = bars[n-p..n].iter().map(|b| (b.close - sma).powi(2)).sum::<f32>() / p as f32;
    let std = var.sqrt();
    if sma > 0.0 { (4.0 * std) / sma } else { 0.05 }
}

pub(crate) fn compute_atr_percentile(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 100 { return 50.0; }
    let current_atr = compute_atr(bars, 14);
    let mut atrs: Vec<f32> = Vec::new();
    for i in 14..n.min(100) {
        atrs.push(compute_atr(&bars[..i+1], 14));
    }
    atrs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = atrs.partition_point(|&a| a < current_atr);
    (rank as f32 / atrs.len().max(1) as f32 * 100.0).clamp(0.0, 100.0)
}

pub(crate) fn compute_breadth(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 50 { return 50.0; }
    // Simulate breadth from % of recent bars closing above various MAs
    let mut score = 0.0f32;
    let last = bars[n-1].close;
    for p in [10, 20, 50] {
        if n >= p {
            let sma: f32 = bars[n-p..n].iter().map(|b| b.close).sum::<f32>() / p as f32;
            if last > sma { score += 33.3; }
        }
    }
    score.clamp(0.0, 100.0)
}

pub(crate) fn compute_rs_rank(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 60 { return 50.0; }
    // RS approximation: relative performance vs its own history
    let ret_20 = if bars[n-21].close > 0.0 { (bars[n-1].close / bars[n-21].close - 1.0) * 100.0 } else { 0.0 };
    let ret_60 = if n > 60 && bars[n-61].close > 0.0 { (bars[n-1].close / bars[n-61].close - 1.0) * 100.0 } else { 0.0 };
    let composite = ret_20 * 0.6 + ret_60 * 0.4;
    (50.0 + composite * 5.0).clamp(0.0, 100.0)
}

pub(crate) fn compute_liquidity(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 20 { return 50.0; }
    let recent = &bars[n-20..n];
    let avg_vol: f32 = recent.iter().map(|b| b.volume).sum::<f32>() / 20.0;
    let vol_std: f32 = (recent.iter().map(|b| (b.volume - avg_vol).powi(2)).sum::<f32>() / 20.0).sqrt();
    let cv = if avg_vol > 0.0 { vol_std / avg_vol } else { 1.0 }; // coefficient of variation
    let spread_proxy = recent.iter().map(|b| (b.high - b.low) / b.close.max(0.01)).sum::<f32>() / 20.0;
    let vol_score = (avg_vol / 1_000_000.0).min(1.0) * 40.0;
    let consistency_score = (1.0 - cv).max(0.0) * 30.0;
    let spread_score = (1.0 - spread_proxy * 20.0).max(0.0) * 30.0;
    (vol_score + consistency_score + spread_score).clamp(0.0, 100.0)
}


#[cfg(test)]
mod at012_tests {
    use crate::chart_renderer::types::Bar;

    fn bars(closes: &[f32]) -> Vec<Bar> {
        closes.iter().enumerate().map(|(i, &c)| {
            // Give high/low a little spread so ATR has real true-range to chew on.
            let prev = if i == 0 { c } else { closes[i - 1] };
            Bar {
                open: prev, high: c.max(prev) + 0.5, low: c.min(prev) - 0.5,
                close: c, volume: 1000.0, _pad: 0.0,
            }
        }).collect()
    }

    /// AUDIT 2026-08-02 (AT-017, P1): the widget scalars and the chart series
    /// used to compute the SAME indicator with different math — a simple
    /// trailing mean here, Wilder smoothing in `compute.rs` — and both were on
    /// screen at once via the glob import in chart_widgets.rs.
    ///
    /// This pins the invariant that matters: the scalar a widget shows must be
    /// the last value of the series the chart plots. It fails if either side
    /// drifts back to its own math.
    #[test]
    fn widget_scalars_match_the_canonical_series() {
        let closes: Vec<f32> = (0..60).map(|i| 100.0 + (i as f32 * 0.37).sin() * 5.0).collect();
        let b = bars(&closes);
        let period = 14;

        let series_rsi = crate::chart_renderer::compute::compute_rsi(&closes, period);
        let last_rsi = series_rsi.iter().rev().copied().find(|v| !v.is_nan()).unwrap();
        let scalar_rsi = super::compute_rsi(&b, period);
        assert!((scalar_rsi - last_rsi).abs() < 1e-4,
            "widget RSI {scalar_rsi} must equal the chart series' last value {last_rsi} \
             — two different numbers for the same indicator is the defect");

        let highs:  Vec<f32> = b.iter().map(|x| x.high).collect();
        let lows:   Vec<f32> = b.iter().map(|x| x.low).collect();
        let series_atr = crate::chart_renderer::compute::compute_atr(&highs, &lows, &closes, period);
        let last_atr = series_atr.iter().rev().copied().find(|v| !v.is_nan()).unwrap();
        let scalar_atr = super::compute_atr(&b, period);
        assert!((scalar_atr - last_atr).abs() < 1e-4,
            "widget ATR {scalar_atr} must equal the chart series' last value {last_atr}");
    }

    /// Insufficient data must keep the documented fallbacks, not NaN — these
    /// values are rendered directly into widgets.
    #[test]
    fn insufficient_data_keeps_the_neutral_fallbacks() {
        let b = bars(&[100.0, 101.0, 102.0]);
        assert_eq!(super::compute_rsi(&b, 14), 50.0, "neutral RSI when data is short");
        assert_eq!(super::compute_atr(&b, 14), 0.0, "zero ATR when data is short");
    }
}

#[cfg(test)]
mod at014_tests {
    use super::{horizon_label, timeframe_minutes, TREND_GRID_PERIODS};

    /// AUDIT 2026-08-02 (AT-018, P1): the seven trend-grid rows are periods on
    /// the PANE'S timeframe, but were labelled with fixed timeframe names. The
    /// labels were wrong in both directions.
    #[test]
    fn labels_reflect_the_real_horizon_not_a_fixed_timeframe() {
        // The old hardcoded label for the last row was "1W".
        // On a 5m chart that row is period 140 → 700 minutes ≈ 11.7 hours.
        assert_eq!(horizon_label("5m", 140), "11.7h",
            "the last row on a 5m chart is under half a day, not a week");
        // On a 1D chart the FIRST row was labelled "5m"; 7 daily bars is a week.
        assert_eq!(horizon_label("1d", 7), "1W",
            "the first row on a daily chart is a week, not five minutes");
        // And the row the old labels called "1W" is really seven months there.
        assert_eq!(horizon_label("1d", 140), "20W",
            "the last row on a daily chart is ~5 months, not one week");
    }

    #[test]
    fn label_units_step_sensibly() {
        assert_eq!(horizon_label("1m", 30), "30m");
        assert_eq!(horizon_label("1m", 120), "2h");
        assert_eq!(horizon_label("1h", 24), "1D");
        assert_eq!(horizon_label("1d", 14), "2W");
    }

    /// An unrecognised timeframe must not invent a duration.
    #[test]
    fn unknown_timeframe_states_the_period_instead_of_guessing() {
        assert!(timeframe_minutes("3wk").is_none());
        assert_eq!(horizon_label("3wk", 14), "P14");
    }

    #[test]
    fn every_grid_row_gets_a_label() {
        for &p in TREND_GRID_PERIODS.iter() {
            assert!(!horizon_label("5m", p).is_empty());
        }
        assert_eq!(TREND_GRID_PERIODS.len(), 7, "the widgets render exactly 7 rows");
    }
}
