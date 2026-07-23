//! W3-01 (audit) — indicator trait + registry.
//!
//! **Why this module exists.** `IndicatorType` is a flat 19-variant enum living
//! inside `gpu.rs`, and adding one indicator means touching ~210 references
//! across 8 files (indicators_panel 47, gpu 38, pane/core 31, indicator_editor
//! 26, dev_inspector 20, chart_controls 10, plus persistence and the context
//! menu). That cost is the single biggest barrier to the product's stated
//! identity — free and extensible — and it is what W3-02 (Rhai custom
//! indicators) has to build on. An outside contributor cannot add an indicator
//! today without a guided tour of two god files.
//!
//! **What this is.** One trait, one registry, and a `&'static dyn Indicator`
//! per built-in that wraps the EXISTING pure functions in
//! `chart::renderer::compute`. No math is reimplemented here — that would risk
//! silently changing values traders rely on. Each impl is a faithful transcription
//! of the corresponding arm of `recompute_indicators`, including its parameter
//! defaults.
//!
//! **Deliberately placed outside `gpu.rs`** (`chart/indicators/`) to start the
//! domain-extraction direction of W4-01 without waiting for it.
//!
//! **Migration posture (staged, per the plan).** This is Stage 1: the registry
//! exists and is the single source of truth for indicator METADATA — the enum
//! delegates its `label()` / `default_period()` / `category()` to it, so the
//! registry is load-bearing from day one rather than sitting orphaned next to
//! the enum. That matters: this codebase's dominant defect class is complete,
//! tested logic with zero callers, found six times in this audit. A registry
//! nothing calls would be the seventh.
//!
//! Stage 2 (separate, corpus-gated commits) routes `recompute_indicators`
//! through `compute()` per indicator. Stage 3 persists by string id instead of
//! enum ordinal. Stage 4 ratchets the enum match sites down.
//!
//! **Hot-path rule.** `compute()` is called on data change and cached, exactly
//! as today — never per frame. `PANE_RS_SPLIT_PLAN.md` rules apply.

use super::renderer::compute as c;

/// Borrowed OHLCV series an indicator computes over. Slices, not owned Vecs —
/// indicators must not copy the bar arrays.
pub struct Ohlcv<'a> {
    pub high: &'a [f32],
    pub low: &'a [f32],
    pub close: &'a [f32],
    pub volume: &'a [f32],
    pub timestamps: &'a [i64],
}

/// Indicator parameters. Mirrors the existing `Indicator.period` / `param2..4`
/// layout so a ported indicator reads its inputs exactly as before. `0.0` means
/// "unset" and the indicator substitutes its documented default — that is the
/// existing convention throughout `recompute_indicators`, preserved on purpose.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Params {
    pub period: usize,
    pub param2: f32,
    pub param3: f32,
    pub param4: f32,
}

impl Params {
    pub fn with_period(period: usize) -> Self {
        Self { period, ..Default::default() }
    }
    /// `param2` or `fallback` when unset (the `> 0.0` convention).
    fn p2_or(&self, fallback: f32) -> f32 { if self.param2 > 0.0 { self.param2 } else { fallback } }
    fn p3_or(&self, fallback: f32) -> f32 { if self.param3 > 0.0 { self.param3 } else { fallback } }
    fn p4_or(&self, fallback: f32) -> f32 { if self.param4 > 0.0 { self.param4 } else { fallback } }
}

/// Multi-lane indicator result. Lane meanings are per-indicator and documented
/// on each impl; the lane layout matches the existing `Indicator` struct so
/// Stage 2 is a direct assignment rather than a reshuffle.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IndicatorOutput {
    pub values: Vec<f32>,
    pub values2: Vec<f32>,
    pub values3: Vec<f32>,
    pub values4: Vec<f32>,
    pub values5: Vec<f32>,
    pub histogram: Vec<f32>,
    /// Supertrend direction flags; empty for everything else.
    pub flags: Vec<bool>,
}

impl IndicatorOutput {
    fn single(values: Vec<f32>) -> Self {
        Self { values, ..Default::default() }
    }
}

/// Where the indicator renders: on the price pane, or in its own sub-pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Overlay,
    Oscillator,
}

/// One indicator. Implement this + register below and the indicator exists —
/// that is the end state W3-01 is driving toward.
pub trait Indicator: Send + Sync {
    /// Stable string id. This is what Stage 3 persists instead of an enum
    /// ordinal, so it must never change once shipped — renaming one silently
    /// drops the indicator from every saved workspace.
    fn id(&self) -> &'static str;
    /// Short label shown on the chart legend and pickers.
    fn label(&self) -> &'static str;
    fn category(&self) -> Category;
    fn default_period(&self) -> usize;
    fn compute(&self, data: &Ohlcv, p: &Params) -> IndicatorOutput;
}

// ─── Built-ins ───────────────────────────────────────────────────────────────
// Each wraps the existing pure fn in `renderer::compute`. Parameter defaults are
// transcribed from the matching arm of `recompute_indicators` — if one differs,
// that is a bug in this file, not an improvement.

macro_rules! simple_ma {
    ($name:ident, $id:literal, $label:literal, $period:literal, $f:path) => {
        pub struct $name;
        impl Indicator for $name {
            fn id(&self) -> &'static str { $id }
            fn label(&self) -> &'static str { $label }
            fn category(&self) -> Category { Category::Overlay }
            fn default_period(&self) -> usize { $period }
            fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
                IndicatorOutput::single($f(d.close, p.period))
            }
        }
    };
}

simple_ma!(Sma, "sma", "SMA", 20, c::compute_sma);
simple_ma!(Ema, "ema", "EMA", 20, c::compute_ema);
simple_ma!(Wma, "wma", "WMA", 20, c::compute_wma);
simple_ma!(Dema, "dema", "DEMA", 20, c::compute_dema);
simple_ma!(Tema, "tema", "TEMA", 20, c::compute_tema);

pub struct Vwap;
impl Indicator for Vwap {
    fn id(&self) -> &'static str { "vwap" }
    fn label(&self) -> &'static str { "VWAP" }
    fn category(&self) -> Category { Category::Overlay }
    fn default_period(&self) -> usize { 1 }
    fn compute(&self, d: &Ohlcv, _p: &Params) -> IndicatorOutput {
        IndicatorOutput::single(c::compute_vwap(d.close, d.volume, d.high, d.low, d.timestamps))
    }
}

/// Lanes: mid / upper / lower.
pub struct BollingerBands;
impl Indicator for BollingerBands {
    fn id(&self) -> &'static str { "bbands" }
    fn label(&self) -> &'static str { "BB" }
    fn category(&self) -> Category { Category::Overlay }
    fn default_period(&self) -> usize { 20 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        let (mid, upper, lower) = c::compute_bollinger(d.close, p.period, p.p2_or(2.0));
        IndicatorOutput { values: mid, values2: upper, values3: lower, ..Default::default() }
    }
}

/// Lanes: tenkan / kijun / senkou_a / senkou_b / chikou.
pub struct Ichimoku;
impl Indicator for Ichimoku {
    fn id(&self) -> &'static str { "ichimoku" }
    fn label(&self) -> &'static str { "ICHI" }
    fn category(&self) -> Category { Category::Overlay }
    fn default_period(&self) -> usize { 9 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        let (tenkan, kijun, sa, sb, chikou) = c::compute_ichimoku(
            d.high, d.low, d.close,
            p.period, p.p2_or(26.0) as usize, p.p3_or(52.0) as usize,
        );
        IndicatorOutput {
            values: tenkan, values2: kijun, values3: sa, values4: sb, values5: chikou,
            ..Default::default()
        }
    }
}

pub struct ParabolicSar;
impl Indicator for ParabolicSar {
    fn id(&self) -> &'static str { "psar" }
    fn label(&self) -> &'static str { "PSAR" }
    fn category(&self) -> Category { Category::Overlay }
    fn default_period(&self) -> usize { 1 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        // NOTE the parameter order: af_start is param4, step is param2, max is
        // param3. Transcribed from recompute_indicators — surprising, but
        // changing it would silently alter every saved PSAR.
        IndicatorOutput::single(c::compute_psar(
            d.high, d.low, p.p4_or(0.02), p.p2_or(0.02), p.p3_or(0.2),
        ))
    }
}

/// Lane `values` + `flags` (trend direction per bar).
pub struct Supertrend;
impl Indicator for Supertrend {
    fn id(&self) -> &'static str { "supertrend" }
    fn label(&self) -> &'static str { "ST" }
    fn category(&self) -> Category { Category::Overlay }
    fn default_period(&self) -> usize { 10 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        let (st, dir) = c::compute_supertrend(d.high, d.low, d.close, p.period, p.p2_or(3.0));
        IndicatorOutput { values: st, flags: dir, ..Default::default() }
    }
}

/// Lanes: mid / upper / lower.
pub struct KeltnerChannels;
impl Indicator for KeltnerChannels {
    fn id(&self) -> &'static str { "keltner" }
    fn label(&self) -> &'static str { "KC" }
    fn category(&self) -> Category { Category::Overlay }
    fn default_period(&self) -> usize { 20 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        let (mid, upper, lower) = c::compute_keltner(d.high, d.low, d.close, p.period, p.p2_or(2.0));
        IndicatorOutput { values: mid, values2: upper, values3: lower, ..Default::default() }
    }
}

pub struct Rsi;
impl Indicator for Rsi {
    fn id(&self) -> &'static str { "rsi" }
    fn label(&self) -> &'static str { "RSI" }
    fn category(&self) -> Category { Category::Oscillator }
    fn default_period(&self) -> usize { 14 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        IndicatorOutput::single(c::compute_rsi(d.close, p.period))
    }
}

/// Lanes: macd / signal + histogram.
pub struct Macd;
impl Indicator for Macd {
    fn id(&self) -> &'static str { "macd" }
    fn label(&self) -> &'static str { "MACD" }
    fn category(&self) -> Category { Category::Oscillator }
    fn default_period(&self) -> usize { 12 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        let (macd, signal, hist) = c::compute_macd(
            d.close, p.period, p.p2_or(26.0) as usize, p.p3_or(9.0) as usize,
        );
        IndicatorOutput { values: macd, values2: signal, histogram: hist, ..Default::default() }
    }
}

/// Lanes: %K / %D.
pub struct Stochastic;
impl Indicator for Stochastic {
    fn id(&self) -> &'static str { "stochastic" }
    fn label(&self) -> &'static str { "STOCH" }
    fn category(&self) -> Category { Category::Oscillator }
    fn default_period(&self) -> usize { 14 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        // period.max(2) matches recompute_indicators — a 1-bar %K is degenerate.
        let (k, dline) = c::compute_stochastic(
            d.high, d.low, d.close, p.period.max(2), p.p2_or(3.0) as usize,
        );
        IndicatorOutput { values: k, values2: dline, ..Default::default() }
    }
}

/// Lanes: adx / +DI / -DI.
pub struct Adx;
impl Indicator for Adx {
    fn id(&self) -> &'static str { "adx" }
    fn label(&self) -> &'static str { "ADX" }
    fn category(&self) -> Category { Category::Oscillator }
    fn default_period(&self) -> usize { 14 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        let (adx, plus_di, minus_di) = c::compute_adx(d.high, d.low, d.close, p.period);
        IndicatorOutput { values: adx, values2: plus_di, values3: minus_di, ..Default::default() }
    }
}

pub struct Cci;
impl Indicator for Cci {
    fn id(&self) -> &'static str { "cci" }
    fn label(&self) -> &'static str { "CCI" }
    fn category(&self) -> Category { Category::Oscillator }
    fn default_period(&self) -> usize { 14 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        IndicatorOutput::single(c::compute_cci(d.high, d.low, d.close, p.period))
    }
}

pub struct WilliamsR;
impl Indicator for WilliamsR {
    fn id(&self) -> &'static str { "williams_r" }
    fn label(&self) -> &'static str { "%R" }
    fn category(&self) -> Category { Category::Oscillator }
    fn default_period(&self) -> usize { 14 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        IndicatorOutput::single(c::compute_williams_r(d.high, d.low, d.close, p.period))
    }
}

pub struct Atr;
impl Indicator for Atr {
    fn id(&self) -> &'static str { "atr" }
    fn label(&self) -> &'static str { "ATR" }
    fn category(&self) -> Category { Category::Oscillator }
    fn default_period(&self) -> usize { 14 }
    fn compute(&self, d: &Ohlcv, p: &Params) -> IndicatorOutput {
        IndicatorOutput::single(c::compute_atr(d.high, d.low, d.close, p.period))
    }
}

pub struct Obv;
impl Indicator for Obv {
    fn id(&self) -> &'static str { "obv" }
    fn label(&self) -> &'static str { "OBV" }
    fn category(&self) -> Category { Category::Oscillator }
    fn default_period(&self) -> usize { 1 }
    fn compute(&self, d: &Ohlcv, _p: &Params) -> IndicatorOutput {
        IndicatorOutput::single(c::compute_obv(d.close, d.volume))
    }
}

// ─── Registry ────────────────────────────────────────────────────────────────

/// Every built-in indicator, in picker order. **This is the registration
/// point** — Stage 4's end state is that adding an indicator means writing one
/// impl and adding one line here.
pub fn all() -> &'static [&'static dyn Indicator] {
    &[
        &Sma, &Ema, &Wma, &Dema, &Tema, &Vwap,
        &BollingerBands, &Ichimoku, &ParabolicSar, &Supertrend, &KeltnerChannels,
        &Rsi, &Macd, &Stochastic, &Adx, &Cci, &WilliamsR, &Atr, &Obv,
    ]
}

/// Look up an indicator by its stable string id.
pub fn get(id: &str) -> Option<&'static dyn Indicator> {
    all().iter().copied().find(|i| i.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<i64>) {
        let close: Vec<f32> = (0..n).map(|i| 100.0 + (i as f32 * 0.7).sin() * 5.0).collect();
        let high: Vec<f32> = close.iter().map(|c| c + 1.0).collect();
        let low: Vec<f32> = close.iter().map(|c| c - 1.0).collect();
        let volume: Vec<f32> = (0..n).map(|i| 1000.0 + i as f32).collect();
        let ts: Vec<i64> = (0..n).map(|i| 1_700_000_000_000 + i as i64 * 60_000).collect();
        (high, low, close, volume, ts)
    }

    #[test]
    fn ids_are_unique() {
        // Ids are the persistence key (Stage 3). A duplicate silently shadows
        // an indicator in `get()`.
        let mut ids: Vec<&str> = all().iter().map(|i| i.id()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate indicator id in the registry");
    }

    #[test]
    fn registry_covers_every_enum_variant() {
        // The enum has 19 variants; the registry must not drift from it while
        // both coexist during migration.
        assert_eq!(all().len(), 19, "registry lost or gained an indicator");
    }

    #[test]
    fn get_finds_every_registered_id() {
        for ind in all() {
            let found = get(ind.id()).unwrap_or_else(|| panic!("get({}) failed", ind.id()));
            assert_eq!(found.label(), ind.label());
        }
        assert!(get("no_such_indicator").is_none());
    }

    #[test]
    fn every_indicator_produces_values_for_real_input() {
        // Guards the defect this registry is meant to end: an indicator that
        // silently yields nothing (the enum's compute() returns all-NaN for
        // every OHLCV indicator, which is exactly the "silently NaN" P1).
        let (high, low, close, volume, ts) = bars(120);
        let d = Ohlcv { high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        for ind in all() {
            let out = ind.compute(&d, &Params::with_period(ind.default_period()));
            assert!(!out.values.is_empty(), "{} produced no values", ind.id());
            let finite = out.values.iter().filter(|v| v.is_finite()).count();
            assert!(finite > 0, "{} produced only NaN/inf over 120 real bars", ind.id());
        }
    }

    #[test]
    fn multi_lane_indicators_fill_their_documented_lanes() {
        let (high, low, close, volume, ts) = bars(200);
        let d = Ohlcv { high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        let p = |n| Params::with_period(n);

        let bb = BollingerBands.compute(&d, &p(20));
        assert!(!bb.values2.is_empty() && !bb.values3.is_empty(), "BB needs upper+lower");

        let macd = Macd.compute(&d, &p(12));
        assert!(!macd.values2.is_empty(), "MACD needs a signal line");
        assert!(!macd.histogram.is_empty(), "MACD needs a histogram");

        let st = Supertrend.compute(&d, &p(10));
        assert!(!st.flags.is_empty(), "Supertrend needs direction flags");

        let ichi = Ichimoku.compute(&d, &p(9));
        assert!(!ichi.values4.is_empty() && !ichi.values5.is_empty(), "Ichimoku needs senkou_b + chikou");

        let adx = Adx.compute(&d, &p(14));
        assert!(!adx.values2.is_empty() && !adx.values3.is_empty(), "ADX needs +DI/-DI");
    }

    /// Lane-by-lane equality that treats NaN as equal to NaN.
    ///
    /// Derived `PartialEq` is useless for comparing two indicator runs: every
    /// series has a NaN warmup region and `NaN != NaN`, so identical output
    /// compares unequal. Only relevant to tests — production never compares
    /// two outputs.
    fn same_output(a: &IndicatorOutput, b: &IndicatorOutput) -> bool {
        let same_lane = |x: &[f32], y: &[f32]| {
            x.len() == y.len()
                && x.iter().zip(y).all(|(p, q)| (p.is_nan() && q.is_nan()) || p == q)
        };
        same_lane(&a.values, &b.values)
            && same_lane(&a.values2, &b.values2)
            && same_lane(&a.values3, &b.values3)
            && same_lane(&a.values4, &b.values4)
            && same_lane(&a.values5, &b.values5)
            && same_lane(&a.histogram, &b.histogram)
            && a.flags == b.flags
    }

    #[test]
    fn unset_params_fall_back_to_documented_defaults() {
        // `0.0 == unset` is the existing convention. A default that drifts here
        // silently changes values on every chart that never set the param.
        let (high, low, close, volume, ts) = bars(200);
        let d = Ohlcv { high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };

        let implicit = Macd.compute(&d, &Params::with_period(12));
        let explicit = Macd.compute(&d, &Params { period: 12, param2: 26.0, param3: 9.0, param4: 0.0 });
        assert!(same_output(&implicit, &explicit), "MACD defaults must be 26/9");

        let bb_implicit = BollingerBands.compute(&d, &Params::with_period(20));
        let bb_explicit = BollingerBands.compute(&d, &Params { period: 20, param2: 2.0, ..Default::default() });
        assert!(same_output(&bb_implicit, &bb_explicit), "BB default std-dev must be 2.0");

        // And the guard must be able to FAIL — a comparison that says
        // everything is equal would pass this test while proving nothing.
        let wrong = BollingerBands.compute(&d, &Params { period: 20, param2: 3.0, ..Default::default() });
        assert!(!same_output(&bb_implicit, &wrong), "std-dev 2.0 vs 3.0 must differ");

        let st_implicit = Supertrend.compute(&d, &Params::with_period(10));
        let st_explicit = Supertrend.compute(&d, &Params { period: 10, param2: 3.0, ..Default::default() });
        assert!(same_output(&st_implicit, &st_explicit), "Supertrend default multiplier must be 3.0");

        let ichi_implicit = Ichimoku.compute(&d, &Params::with_period(9));
        let ichi_explicit = Ichimoku.compute(&d, &Params { period: 9, param2: 26.0, param3: 52.0, param4: 0.0 });
        assert!(same_output(&ichi_implicit, &ichi_explicit), "Ichimoku defaults must be 26/52");

        let psar_implicit = ParabolicSar.compute(&d, &Params::with_period(1));
        let psar_explicit = ParabolicSar.compute(&d, &Params { period: 1, param2: 0.02, param3: 0.2, param4: 0.02 });
        assert!(same_output(&psar_implicit, &psar_explicit), "PSAR defaults must be 0.02/0.02/0.2");
    }

    /// W3-01 Stage 2 guard: the registry's lane assignment must match, element
    /// for element, what the deleted `recompute_indicators` arms wrote. The
    /// direct `compute::` tuple is the oracle — if a lane is swapped (upper vs
    /// lower, +DI vs -DI, signal vs histogram) this catches it, and a swap
    /// would silently mislabel a trader's bands.
    #[test]
    fn registry_lanes_match_the_original_compute_tuples() {
        let (high, low, close, volume, ts) = bars(200);
        let d = Ohlcv { high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        let eq = |a: &[f32], b: &[f32]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(p, q)| (p.is_nan() && q.is_nan()) || p == q)
        };

        // Bollinger: (mid, upper, lower) → values/values2/values3.
        let bb = BollingerBands.compute(&d, &Params::with_period(20));
        let (mid, upper, lower) = c::compute_bollinger(&close, 20, 2.0);
        assert!(eq(&bb.values, &mid) && eq(&bb.values2, &upper) && eq(&bb.values3, &lower));

        // Keltner: same lane shape, but H/L/C inputs.
        let kc = KeltnerChannels.compute(&d, &Params::with_period(20));
        let (kmid, kup, klo) = c::compute_keltner(&high, &low, &close, 20, 2.0);
        assert!(eq(&kc.values, &kmid) && eq(&kc.values2, &kup) && eq(&kc.values3, &klo));

        // MACD: (macd, signal, hist) → values/values2/histogram.
        let macd = Macd.compute(&d, &Params::with_period(12));
        let (m, s, h) = c::compute_macd(&close, 12, 26, 9);
        assert!(eq(&macd.values, &m) && eq(&macd.values2, &s) && eq(&macd.histogram, &h));

        // ADX: (adx, +DI, -DI) → values/values2/values3 — order matters.
        let adx = Adx.compute(&d, &Params::with_period(14));
        let (a, pdi, mdi) = c::compute_adx(&high, &low, &close, 14);
        assert!(eq(&adx.values, &a) && eq(&adx.values2, &pdi) && eq(&adx.values3, &mdi));

        // Ichimoku: (tenkan, kijun, sa, sb, chikou) across all five lanes.
        let ichi = Ichimoku.compute(&d, &Params::with_period(9));
        let (t, k, sa, sb, ch) = c::compute_ichimoku(&high, &low, &close, 9, 26, 52);
        assert!(eq(&ichi.values, &t) && eq(&ichi.values2, &k) && eq(&ichi.values3, &sa)
            && eq(&ichi.values4, &sb) && eq(&ichi.values5, &ch));

        // Stochastic: period.max(2) is load-bearing — a period-1 %K is degenerate.
        let st = Stochastic.compute(&d, &Params::with_period(14));
        let (kk, dd) = c::compute_stochastic(&high, &low, &close, 14, 3);
        assert!(eq(&st.values, &kk) && eq(&st.values2, &dd));

        // Supertrend direction flags land in `flags`, not a value lane.
        let str = Supertrend.compute(&d, &Params::with_period(10));
        let (sv, sdir) = c::compute_supertrend(&high, &low, &close, 10, 3.0);
        assert!(eq(&str.values, &sv) && str.flags == sdir);
    }

    #[test]
    fn categories_match_the_enum_split() {
        // Oscillators render in a sub-pane; overlays on price. Getting this
        // wrong puts RSI on the price axis.
        let osc: Vec<&str> = all().iter()
            .filter(|i| i.category() == Category::Oscillator)
            .map(|i| i.id()).collect();
        assert_eq!(osc, vec!["rsi", "macd", "stochastic", "adx", "cci", "williams_r", "atr", "obv"]);
    }
}
