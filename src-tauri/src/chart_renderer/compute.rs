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
    let sma: f32 = data[..period].iter().sum::<f32>() / period as f32;
    r[period-1] = sma;
    let mut prev = sma;
    for i in period..data.len() { let v = data[i] * k + prev * (1.0 - k); r[i] = v; prev = v; }
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
    if price < 20.0 { 0.5 } else if price < 50.0 { 1.0 } else if price < 100.0 { 2.5 }
    else if price < 200.0 { 5.0 } else if price < 500.0 { 10.0 } else { 25.0 }
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
