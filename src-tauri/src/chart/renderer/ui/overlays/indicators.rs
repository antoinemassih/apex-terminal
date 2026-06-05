//! Indicator math for on-chart overlay widgets — pure functions over price
//! `Bar`s. Extracted from `chart_widgets.rs` so data computation lives apart
//! from rendering (the UI file should not compute RSI/ATR/breadth/…).

use crate::chart_renderer::types::Bar;

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

pub(crate) fn compute_rsi(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let n = bars.len();
    if n < period + 1 { return 50.0; }
    let mut gain_sum = 0.0f32;
    let mut loss_sum = 0.0f32;
    for i in (n - period)..n {
        let diff = bars[i].close - bars[i - 1].close;
        if diff > 0.0 { gain_sum += diff; } else { loss_sum += -diff; }
    }
    let avg_gain = gain_sum / period as f32;
    let avg_loss = loss_sum / period as f32;
    if avg_loss < 0.0001 { return 100.0; }
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

pub(crate) fn compute_atr(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let n = bars.len();
    if n < period + 1 { return 0.0; }
    let mut sum = 0.0f32;
    for i in (n - period)..n {
        let tr = (bars[i].high - bars[i].low)
            .max((bars[i].high - bars[i - 1].close).abs())
            .max((bars[i].low - bars[i - 1].close).abs());
        sum += tr;
    }
    sum / period as f32
}

pub(crate) fn compute_trend_grid(bars: &[crate::chart_renderer::types::Bar]) -> [[bool; 4]; 7] {
    let n = bars.len();
    let periods = [7, 10, 14, 21, 42, 70, 140]; // map to 7 timeframes
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

