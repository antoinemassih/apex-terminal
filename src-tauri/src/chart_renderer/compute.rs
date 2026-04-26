//! Pure computation functions — indicators, Black-Scholes, divergences.
//! No UI dependencies. Safe to call from any thread.

// ─── Moving Averages ─────────────────────────────────────────────────────────

pub fn compute_sma(data: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; data.len()];
    if data.len() < period { return r; }
    let mut s: f32 = data[..period].iter().sum();
    r[period-1] = s / period as f32;
    for i in period..data.len() { s += data[i] - data[i-period]; r[i] = s / period as f32; }
    r
}

pub fn compute_ema(data: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; data.len()];
    if data.len() < period { return r; }
    let k = 2.0 / (period as f32 + 1.0);
    // Find the first run of `period` non-NaN values to seed the SMA
    let mut start = 0;
    while start + period <= data.len() {
        let valid = data[start..start+period].iter().all(|v| !v.is_nan());
        if valid { break; }
        start += 1;
    }
    if start + period > data.len() { return r; }
    let sma: f32 = data[start..start+period].iter().sum::<f32>() / period as f32;
    r[start+period-1] = sma;
    let mut prev = sma;
    for i in (start+period)..data.len() {
        if data[i].is_nan() { continue; }
        let v = data[i] * k + prev * (1.0 - k);
        r[i] = v; prev = v;
    }
    r
}

pub fn compute_wma(closes: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; closes.len()];
    if closes.len() < period { return r; }
    let denom = (period * (period + 1)) / 2;
    for i in (period - 1)..closes.len() {
        let mut s = 0.0;
        for j in 0..period { s += closes[i + 1 - period + j] * (j + 1) as f32; }
        r[i] = s / denom as f32;
    }
    r
}

pub fn compute_dema(closes: &[f32], period: usize) -> Vec<f32> {
    let ema1 = compute_ema(closes, period);
    let ema2 = compute_ema(&ema1, period);
    ema1.iter().zip(&ema2).map(|(&a, &b)| if a.is_nan() || b.is_nan() { f32::NAN } else { 2.0 * a - b }).collect()
}

pub fn compute_tema(closes: &[f32], period: usize) -> Vec<f32> {
    let ema1 = compute_ema(closes, period);
    let ema2 = compute_ema(&ema1, period);
    let ema3 = compute_ema(&ema2, period);
    ema1.iter().zip(ema2.iter().zip(&ema3))
        .map(|(&a, (&b, &c))| if a.is_nan() || b.is_nan() || c.is_nan() { f32::NAN } else { 3.0 * a - 3.0 * b + c })
        .collect()
}

// ─── Volatility / Trend Overlays ─────────────────────────────────────────────

pub fn compute_atr(highs: &[f32], lows: &[f32], closes: &[f32], period: usize) -> Vec<f32> {
    let n = closes.len();
    let mut atr = vec![f32::NAN; n];
    if n < period + 1 { return atr; }
    let mut sum = 0.0_f32;
    for i in 1..=period {
        let tr = (highs[i] - lows[i]).max((highs[i] - closes[i-1]).abs()).max((lows[i] - closes[i-1]).abs());
        sum += tr;
    }
    atr[period] = sum / period as f32;
    for i in (period+1)..n {
        let tr = (highs[i] - lows[i]).max((highs[i] - closes[i-1]).abs()).max((lows[i] - closes[i-1]).abs());
        atr[i] = (atr[i-1] * (period as f32 - 1.0) + tr) / period as f32;
    }
    atr
}

pub fn compute_bollinger(closes: &[f32], period: usize, num_std: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let sma = compute_sma(closes, period);
    let n = closes.len();
    let mut upper = vec![f32::NAN; n];
    let mut lower = vec![f32::NAN; n];
    for i in (period-1)..n {
        if sma[i].is_nan() { continue; }
        let mut sum_sq = 0.0_f32;
        for j in (i+1-period)..=i { sum_sq += (closes[j] - sma[i]).powi(2); }
        let std_dev = (sum_sq / period as f32).sqrt();
        upper[i] = sma[i] + num_std * std_dev;
        lower[i] = sma[i] - num_std * std_dev;
    }
    (sma, upper, lower)
}

pub fn compute_ichimoku(highs: &[f32], lows: &[f32], closes: &[f32], tenkan: usize, kijun: usize, senkou_b: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let n = closes.len();
    let period_hl = |h: &[f32], l: &[f32], period: usize| -> Vec<f32> {
        let mut r = vec![f32::NAN; n];
        for i in (period-1)..n {
            let mut hi = f32::MIN; let mut lo = f32::MAX;
            for j in (i+1-period)..=i { hi = hi.max(h[j]); lo = lo.min(l[j]); }
            r[i] = (hi + lo) / 2.0;
        }
        r
    };
    let tenkan_sen = period_hl(highs, lows, tenkan);
    let kijun_sen = period_hl(highs, lows, kijun);
    let mut senkou_a = vec![f32::NAN; n + kijun];
    for i in 0..n { if !tenkan_sen[i].is_nan() && !kijun_sen[i].is_nan() { senkou_a[i + kijun] = (tenkan_sen[i] + kijun_sen[i]) / 2.0; } }
    let span_b_raw = period_hl(highs, lows, senkou_b);
    let mut senkou_b_vals = vec![f32::NAN; n + kijun];
    for i in 0..n { if !span_b_raw[i].is_nan() { senkou_b_vals[i + kijun] = span_b_raw[i]; } }
    let mut chikou = vec![f32::NAN; n];
    for i in kijun..n { chikou[i - kijun] = closes[i]; }
    senkou_a.truncate(n); senkou_b_vals.truncate(n);
    (tenkan_sen, kijun_sen, senkou_a, senkou_b_vals, chikou)
}

pub fn compute_psar(highs: &[f32], lows: &[f32], af_start: f32, af_step: f32, af_max: f32) -> Vec<f32> {
    let n = highs.len();
    if n < 2 { return vec![f32::NAN; n]; }
    let mut sar = vec![f32::NAN; n];
    let mut is_long = true;
    let mut af = af_start;
    let mut ep = highs[0];
    sar[0] = lows[0];
    for i in 1..n {
        sar[i] = sar[i-1] + af * (ep - sar[i-1]);
        if is_long {
            if lows[i] < sar[i] {
                is_long = false; sar[i] = ep; ep = lows[i]; af = af_start;
            } else {
                if highs[i] > ep { ep = highs[i]; af = (af + af_step).min(af_max); }
                sar[i] = sar[i].min(lows[i-1]);
                if i > 1 { sar[i] = sar[i].min(lows[i-2]); }
            }
        } else {
            if highs[i] > sar[i] {
                is_long = true; sar[i] = ep; ep = highs[i]; af = af_start;
            } else {
                if lows[i] < ep { ep = lows[i]; af = (af + af_step).min(af_max); }
                sar[i] = sar[i].max(highs[i-1]);
                if i > 1 { sar[i] = sar[i].max(highs[i-2]); }
            }
        }
    }
    sar
}

/// Returns (supertrend_line, is_bullish) per bar.
pub fn compute_supertrend(highs: &[f32], lows: &[f32], closes: &[f32], period: usize, multiplier: f32) -> (Vec<f32>, Vec<bool>) {
    let n = closes.len();
    let atr = compute_atr(highs, lows, closes, period);
    let mut st = vec![f32::NAN; n];
    let mut direction = vec![true; n];
    for i in period..n {
        if atr[i].is_nan() { continue; }
        let hl2 = (highs[i] + lows[i]) / 2.0;
        let upper = hl2 + multiplier * atr[i];
        let lower = hl2 - multiplier * atr[i];
        if i == period { st[i] = lower; direction[i] = true; continue; }
        let prev_st = st[i-1];
        if prev_st.is_nan() { st[i] = lower; direction[i] = true; continue; }
        if direction[i-1] {
            let new_lower = lower.max(prev_st);
            if closes[i] < new_lower { st[i] = upper; direction[i] = false; }
            else { st[i] = new_lower; direction[i] = true; }
        } else {
            let new_upper = upper.min(prev_st);
            if closes[i] > new_upper { st[i] = lower; direction[i] = true; }
            else { st[i] = new_upper; direction[i] = false; }
        }
    }
    (st, direction)
}

pub fn compute_keltner(highs: &[f32], lows: &[f32], closes: &[f32], period: usize, multiplier: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mid = compute_ema(closes, period);
    let atr = compute_atr(highs, lows, closes, period);
    let n = closes.len();
    let mut upper = vec![f32::NAN; n];
    let mut lower = vec![f32::NAN; n];
    for i in 0..n {
        if !mid[i].is_nan() && !atr[i].is_nan() {
            upper[i] = mid[i] + multiplier * atr[i];
            lower[i] = mid[i] - multiplier * atr[i];
        }
    }
    (mid, upper, lower)
}

// ─── Oscillators ─────────────────────────────────────────────────────────────

pub fn compute_rsi(closes: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; closes.len()];
    if closes.len() <= period { return r; }
    let mut avg_gain = 0.0_f32;
    let mut avg_loss = 0.0_f32;
    for i in 1..=period {
        let d = closes[i] - closes[i-1];
        if d > 0.0 { avg_gain += d; } else { avg_loss += -d; }
    }
    avg_gain /= period as f32;
    avg_loss /= period as f32;
    let rs = if avg_loss == 0.0 { 100.0 } else { avg_gain / avg_loss };
    r[period] = 100.0 - 100.0 / (1.0 + rs);
    for i in (period+1)..closes.len() {
        let d = closes[i] - closes[i-1];
        let (gain, loss) = if d > 0.0 { (d, 0.0) } else { (0.0, -d) };
        avg_gain = (avg_gain * (period as f32 - 1.0) + gain) / period as f32;
        avg_loss = (avg_loss * (period as f32 - 1.0) + loss) / period as f32;
        let rs = if avg_loss == 0.0 { 100.0 } else { avg_gain / avg_loss };
        r[i] = 100.0 - 100.0 / (1.0 + rs);
    }
    r
}

pub fn compute_macd(closes: &[f32], fast: usize, slow: usize, signal: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let ema_fast = compute_ema(closes, fast);
    let ema_slow = compute_ema(closes, slow);
    let macd_line: Vec<f32> = ema_fast.iter().zip(&ema_slow).map(|(&f, &s)| {
        if f.is_nan() || s.is_nan() { f32::NAN } else { f - s }
    }).collect();
    let signal_line = compute_ema(&macd_line, signal);
    let histogram: Vec<f32> = macd_line.iter().zip(&signal_line).map(|(&m, &s)| {
        if m.is_nan() || s.is_nan() { f32::NAN } else { m - s }
    }).collect();
    (macd_line, signal_line, histogram)
}

pub fn compute_stochastic(highs: &[f32], lows: &[f32], closes: &[f32], k_period: usize, d_period: usize) -> (Vec<f32>, Vec<f32>) {
    let n = closes.len();
    let mut k = vec![f32::NAN; n];
    for i in (k_period-1)..n {
        let mut hi = f32::MIN;
        let mut lo = f32::MAX;
        for j in (i+1-k_period)..=i { hi = hi.max(highs[j]); lo = lo.min(lows[j]); }
        k[i] = if hi == lo { 50.0 } else { (closes[i] - lo) / (hi - lo) * 100.0 };
    }
    let d = compute_sma(&k, d_period);
    (k, d)
}

pub fn compute_vwap(closes: &[f32], volumes: &[f32], highs: &[f32], lows: &[f32]) -> Vec<f32> {
    let n = closes.len();
    let mut r = vec![f32::NAN; n];
    let mut cum_tp_vol = 0.0_f32;
    let mut cum_vol = 0.0_f32;
    for i in 0..n {
        let tp = (highs[i] + lows[i] + closes[i]) / 3.0;
        cum_tp_vol += tp * volumes[i];
        cum_vol += volumes[i];
        if cum_vol > 0.0 { r[i] = cum_tp_vol / cum_vol; }
    }
    r
}

// ─── Divergence Detection ────────────────────────────────────────────────────

pub fn detect_divergences(closes: &[f32], indicator: &[f32], lookback: usize) -> Vec<i8> {
    let n = closes.len();
    let mut div = vec![0i8; n];
    if n < lookback * 2 { return div; }
    for i in lookback..n.saturating_sub(lookback) {
        let is_price_low = (1..=lookback).all(|j| closes[i] <= closes[i.saturating_sub(j)] && closes[i] <= closes[(i+j).min(n-1)]);
        if is_price_low && !indicator[i].is_nan() {
            for k in (lookback..i).rev().take(lookback * 4) {
                let was_low = (1..=lookback.min(k)).all(|j| closes[k] <= closes[k.saturating_sub(j)]);
                if was_low && !indicator[k].is_nan() {
                    if closes[i] < closes[k] && indicator[i] > indicator[k] { div[i] = 1; }
                    if closes[i] > closes[k] && indicator[i] < indicator[k] { div[i] = -1; }
                    break;
                }
            }
        }
    }
    div
}

// ─── Black-Scholes Options Pricing ───────────────────────────────────────────

pub fn normal_cdf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let poly = t * (0.319381530 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    let phi = (-0.5 * x * x).exp() / (2.0 * std::f32::consts::PI).sqrt();
    let cdf = 1.0 - phi * poly;
    if x >= 0.0 { cdf } else { 1.0 - cdf }
}

pub fn bs_price(s: f32, k: f32, t: f32, r: f32, iv: f32, is_call: bool) -> f32 {
    if t <= 0.0 { return if is_call { (s - k).max(0.0) } else { (k - s).max(0.0) }; }
    let d1 = ((s / k).ln() + (r + 0.5 * iv * iv) * t) / (iv * t.sqrt());
    let d2 = d1 - iv * t.sqrt();
    if is_call { s * normal_cdf(d1) - k * (-r * t).exp() * normal_cdf(d2) }
    else { k * (-r * t).exp() * normal_cdf(-d2) - s * normal_cdf(-d1) }
}

#[allow(dead_code)]
pub fn bs_delta(s: f32, k: f32, t: f32, r: f32, iv: f32, is_call: bool) -> f32 {
    if t <= 0.0 { return if is_call { if s > k { 1.0 } else { 0.0 } } else { if s < k { -1.0 } else { 0.0 } }; }
    let d1 = ((s / k).ln() + (r + 0.5 * iv * iv) * t) / (iv * t.sqrt());
    if is_call { normal_cdf(d1) } else { normal_cdf(d1) - 1.0 }
}

pub fn strike_interval(price: f32) -> f32 {
    // Most liquid options (SPY, QQQ, AAPL, etc.) have $1 strikes
    // Less liquid / lower priced have wider intervals
    if price < 10.0 { 0.5 } else if price < 25.0 { 1.0 } else { 1.0 }
    // Note: real chains from IB/ApexIB have exact strike spacing per symbol.
    // This is only used for simulated/fallback data.
}

pub fn atm_strike(price: f32) -> f32 {
    let interval = strike_interval(price);
    (price / interval).round() * interval
}

pub fn get_iv(s: f32, k: f32, dte: i32) -> f32 {
    let base = 0.28;
    let moneyness = (k / s).ln();
    let smile = 0.06 * moneyness * moneyness;
    let skew = -0.05 * moneyness;
    let term = if dte <= 0 { 1.25 } else if dte == 1 { 1.10 } else { 1.0 };
    (base + smile + skew) * term
}

pub fn sim_oi(underlying: f32, strike: f32, dte: i32) -> i32 {
    let interval = strike_interval(underlying);
    let atm = atm_strike(underlying);
    let strikes_away = ((strike - atm).abs() / interval) as f32;
    let base = if dte <= 0 { 18000.0 } else if dte == 1 { 35000.0 } else { 50000.0 };
    let raw = base * (-0.35 * strikes_away * strikes_away).exp();
    let noise = 1.0 + 0.3 * (strike * 17.3 + dte as f32 * 5.7).sin();
    (raw * noise).max(100.0) as i32
}

// ═════════════════════════════════════════════════════════════════════════════
// Drawing Computation — headless-safe, no UI dependencies.
//
// These functions evaluate drawing geometry against price data. They can run
// on the GPU renderer thread OR on a headless server for alert evaluation.
//
// All functions use f64 for precision (timestamps are i64, prices are f64 in DB).
// The GPU renderer casts to f32 for rendering; the headless server uses f64 directly.
// ═════════════════════════════════════════════════════════════════════════════

/// Interpolate a trendline's price at a given timestamp.
/// Returns None if the trendline is vertical (time0 == time1).
pub fn trendline_price_at(time0: i64, price0: f64, time1: i64, price1: f64, at_time: i64) -> Option<f64> {
    let dt = time1 - time0;
    if dt == 0 { return None; }
    let t = (at_time - time0) as f64 / dt as f64;
    Some(price0 + t * (price1 - price0))
}

/// Check if a bar's price range crosses a horizontal line.
pub fn hline_crossed(price: f64, bar_low: f64, bar_high: f64) -> bool {
    bar_low <= price && bar_high >= price
}

/// Check if a bar crosses a trendline (price at the bar's timestamp crosses
/// through the bar's high-low range).
pub fn trendline_crossed(
    time0: i64, price0: f64, time1: i64, price1: f64,
    bar_time: i64, bar_low: f64, bar_high: f64,
) -> bool {
    if let Some(line_price) = trendline_price_at(time0, price0, time1, price1, bar_time) {
        bar_low <= line_price && bar_high >= line_price
    } else {
        false
    }
}

/// Compute all fibonacci retracement + extension level prices.
/// Returns Vec of (level_ratio, price) pairs.
pub fn fib_levels(price0: f64, price1: f64) -> Vec<(f64, f64)> {
    let range = price1 - price0;
    let ratios = [
        // Retracement
        0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0,
        // Extensions
        -0.272, -0.618, 1.272, 1.414, 1.618, 2.0, 2.618, 3.146,
    ];
    ratios.iter().map(|&r| (r, price0 + range * r)).collect()
}

/// Check if a bar touches any fib level (within tolerance).
/// Returns the first level crossed, or None.
pub fn fib_level_crossed(
    price0: f64, price1: f64,
    bar_low: f64, bar_high: f64,
    tolerance: f64,
) -> Option<(f64, f64)> {
    for (ratio, level_price) in fib_levels(price0, price1) {
        if (bar_low - tolerance) <= level_price && (bar_high + tolerance) >= level_price {
            return Some((ratio, level_price));
        }
    }
    None
}

/// Compute channel bounds (base line price, parallel line price) at a given timestamp.
pub fn channel_bounds_at(
    time0: i64, price0: f64, time1: i64, price1: f64, offset: f64,
    at_time: i64,
) -> Option<(f64, f64)> {
    let base = trendline_price_at(time0, price0, time1, price1, at_time)?;
    Some((base, base + offset))
}

/// Check if a bar's price is outside the channel bounds.
/// Returns: -1 if below channel, 0 if inside, 1 if above channel.
pub fn channel_position(
    time0: i64, price0: f64, time1: i64, price1: f64, offset: f64,
    bar_time: i64, bar_close: f64,
) -> i32 {
    if let Some((lo, hi)) = channel_bounds_at(time0, price0, time1, price1, offset, bar_time) {
        let (lower, upper) = if lo < hi { (lo, hi) } else { (hi, lo) };
        if bar_close < lower { -1 } else if bar_close > upper { 1 } else { 0 }
    } else {
        0
    }
}

/// Compute fib extension target prices (3-point: A→B projected from C).
/// Returns Vec of (level_ratio, target_price) pairs.
pub fn fib_extension_targets(
    price_a: f64, price_b: f64, price_c: f64,
) -> Vec<(f64, f64)> {
    let ab_range = price_b - price_a;
    let direction = if price_c < price_b { 1.0 } else { -1.0 }; // project in the trending direction
    let ratios = [0.0, 0.618, 1.0, 1.272, 1.618, 2.0, 2.618];
    ratios.iter().map(|&r| (r, price_c + direction * ab_range.abs() * r)).collect()
}

/// Linear regression over a slice of close prices.
/// Returns (slope, intercept, sigma) where:
///   predicted_price_at_index(i) = intercept + slope * i
///   sigma = standard deviation of residuals
pub fn linear_regression(closes: &[f64]) -> Option<(f64, f64, f64)> {
    let n = closes.len();
    if n < 2 { return None; }
    let nf = n as f64;
    let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
    for (i, &y) in closes.iter().enumerate() {
        let x = i as f64;
        sx += x; sy += y; sxx += x * x; sxy += x * y;
    }
    let denom = nf * sxx - sx * sx;
    if denom.abs() < 1e-12 { return None; }
    let slope = (nf * sxy - sx * sy) / denom;
    let intercept = (sy - slope * sx) / nf;
    let mut ss = 0.0;
    for (i, &y) in closes.iter().enumerate() {
        let predicted = intercept + slope * i as f64;
        ss += (y - predicted).powi(2);
    }
    let sigma = (ss / nf).sqrt();
    Some((slope, intercept, sigma))
}

/// Compute anchored VWAP from a starting index through a slice of bars.
/// Returns cumulative VWAP at each bar from start_idx to end.
pub fn anchored_vwap(
    highs: &[f64], lows: &[f64], closes: &[f64], volumes: &[f64],
    start_idx: usize,
) -> Vec<(usize, f64)> {
    let mut result = vec![];
    let mut cum_tp_vol = 0.0;
    let mut cum_vol = 0.0;
    for i in start_idx..closes.len() {
        let tp = (highs[i] + lows[i] + closes[i]) / 3.0;
        cum_tp_vol += tp * volumes[i];
        cum_vol += volumes[i];
        if cum_vol > 0.0 {
            result.push((i, cum_tp_vol / cum_vol));
        }
    }
    result
}

/// Convert a timestamp to a fractional bar index using a timestamps array.
/// Binary search for efficiency. Returns fractional index for interpolation.
pub fn time_to_bar_index(timestamps: &[i64], time: i64) -> f64 {
    if timestamps.is_empty() { return 0.0; }
    if time <= timestamps[0] { return 0.0; }
    if time >= *timestamps.last().unwrap() { return (timestamps.len() - 1) as f64; }
    match timestamps.binary_search(&time) {
        Ok(idx) => idx as f64,
        Err(idx) => {
            if idx == 0 { return 0.0; }
            let t0 = timestamps[idx - 1];
            let t1 = timestamps[idx];
            let frac = if t1 != t0 { (time - t0) as f64 / (t1 - t0) as f64 } else { 0.0 };
            (idx - 1) as f64 + frac
        }
    }
}

/// Evaluate all drawings for a symbol against the latest bar.
/// Returns a list of (drawing_id, alert_type, message) for any triggered conditions.
/// This is the main function a headless alert server would call.
pub fn evaluate_drawings_against_bar(
    drawings: &[(String, String, serde_json::Value)], // (id, drawing_type, params_json)
    bar_time: i64, bar_open: f64, bar_high: f64, bar_low: f64, bar_close: f64,
    timestamps: &[i64],
) -> Vec<(String, String, String)> {
    let mut alerts = vec![];

    for (id, dtype, params) in drawings {
        match dtype.as_str() {
            "hline" => {
                let price = params.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if hline_crossed(price, bar_low, bar_high) {
                    alerts.push((id.clone(), "cross".into(), format!("Price crossed H-Line at {:.2}", price)));
                }
            }
            "trendline" | "ray" => {
                let p0 = params.get("price0").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let t0 = params.get("time0").and_then(|v| v.as_i64()).unwrap_or(0);
                let p1 = params.get("price1").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let t1 = params.get("time1").and_then(|v| v.as_i64()).unwrap_or(0);
                if trendline_crossed(t0, p0, t1, p1, bar_time, bar_low, bar_high) {
                    let line_p = trendline_price_at(t0, p0, t1, p1, bar_time).unwrap_or(0.0);
                    alerts.push((id.clone(), "cross".into(), format!("Price crossed trendline at {:.2}", line_p)));
                }
            }
            "fibonacci" => {
                let p0 = params.get("price0").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let p1 = params.get("price1").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if let Some((ratio, level)) = fib_level_crossed(p0, p1, bar_low, bar_high, 0.01) {
                    alerts.push((id.clone(), "fib_touch".into(), format!("Price touched Fib {:.1}% at {:.2}", ratio * 100.0, level)));
                }
            }
            "channel" | "fibchannel" => {
                let p0 = params.get("price0").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let t0 = params.get("time0").and_then(|v| v.as_i64()).unwrap_or(0);
                let p1 = params.get("price1").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let t1 = params.get("time1").and_then(|v| v.as_i64()).unwrap_or(0);
                let offset = params.get("offset").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let pos = channel_position(t0, p0, t1, p1, offset, bar_time, bar_close);
                if pos != 0 {
                    let side = if pos > 0 { "above" } else { "below" };
                    alerts.push((id.clone(), "breakout".into(), format!("Price broke {} channel at {:.2}", side, bar_close)));
                }
            }
            _ => {} // Other types: no alert evaluation yet
        }
    }

    alerts
}

// ─── ADX / CCI / Williams %R ─────────────────────────────────────────────────

/// ADX with Wilder smoothing. Returns (adx, plus_di, minus_di), each
/// length == bars.len(). First (~2*period) values are NaN.
pub fn compute_adx(highs: &[f32], lows: &[f32], closes: &[f32], period: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let n = closes.len();
    if n <= period + 1 {
        return (vec![f32::NAN; n], vec![f32::NAN; n], vec![f32::NAN; n]);
    }
    let mut plus_dm_sum = 0.0_f32;
    let mut minus_dm_sum = 0.0_f32;
    let mut tr_sum = 0.0_f32;
    for i in 1..=period {
        let hi = highs[i] - highs[i-1];
        let lo = lows[i-1] - lows[i];
        plus_dm_sum += if hi > lo && hi > 0.0 { hi } else { 0.0 };
        minus_dm_sum += if lo > hi && lo > 0.0 { lo } else { 0.0 };
        let tr = (highs[i] - lows[i])
            .max((highs[i] - closes[i-1]).abs())
            .max((lows[i] - closes[i-1]).abs());
        tr_sum += tr;
    }
    let mut dx_vals = vec![f32::NAN; n];
    let k = 1.0 / period as f32;
    let mut plus_di_vals = vec![f32::NAN; n];
    let mut minus_di_vals = vec![f32::NAN; n];
    for i in period..n {
        if i > period {
            let hi = highs[i] - highs[i-1];
            let lo = lows[i-1] - lows[i];
            let pdm = if hi > lo && hi > 0.0 { hi } else { 0.0 };
            let mdm = if lo > hi && lo > 0.0 { lo } else { 0.0 };
            let tr = (highs[i] - lows[i])
                .max((highs[i] - closes[i-1]).abs())
                .max((lows[i] - closes[i-1]).abs());
            plus_dm_sum = plus_dm_sum * (1.0 - k) + pdm;
            minus_dm_sum = minus_dm_sum * (1.0 - k) + mdm;
            tr_sum = tr_sum * (1.0 - k) + tr;
        }
        let plus_di = if tr_sum > 0.0 { plus_dm_sum / tr_sum * 100.0 } else { 0.0 };
        let minus_di = if tr_sum > 0.0 { minus_dm_sum / tr_sum * 100.0 } else { 0.0 };
        plus_di_vals[i] = plus_di;
        minus_di_vals[i] = minus_di;
        let di_sum = plus_di + minus_di;
        dx_vals[i] = if di_sum > 0.0 { (plus_di - minus_di).abs() / di_sum * 100.0 } else { 0.0 };
    }
    let adx = compute_ema(&dx_vals, period);
    (adx, plus_di_vals, minus_di_vals)
}

pub fn compute_cci(highs: &[f32], lows: &[f32], closes: &[f32], period: usize) -> Vec<f32> {
    let n = closes.len();
    let mut cci = vec![f32::NAN; n];
    let tp: Vec<f32> = highs.iter().zip(lows.iter()).zip(closes.iter())
        .map(|((h, l), c)| (h + l + c) / 3.0).collect();
    if n >= period {
        for i in (period-1)..n {
            let sma: f32 = tp[i+1-period..=i].iter().sum::<f32>() / period as f32;
            let mean_dev: f32 = tp[i+1-period..=i].iter().map(|&v| (v - sma).abs()).sum::<f32>() / period as f32;
            cci[i] = if mean_dev > 0.0 { (tp[i] - sma) / (0.015 * mean_dev) } else { 0.0 };
        }
    }
    cci
}

pub fn compute_williams_r(highs: &[f32], lows: &[f32], closes: &[f32], period: usize) -> Vec<f32> {
    let n = closes.len();
    let mut wr = vec![f32::NAN; n];
    if n >= period {
        for i in (period-1)..n {
            let mut hi = f32::MIN; let mut lo = f32::MAX;
            for j in (i+1-period)..=i {
                hi = hi.max(highs[j]);
                lo = lo.min(lows[j]);
            }
            let range = hi - lo;
            wr[i] = if range > 0.0 { (hi - closes[i]) / range * -100.0 } else { -50.0 };
        }
    }
    wr
}
