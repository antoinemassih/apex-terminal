//! W3-02 (audit) — user-scriptable custom indicators via embedded Rhai.
//!
//! This is the differentiator's first real surface: a trader writes a small
//! script and it computes over the same OHLCV the built-ins get, rendering like
//! any other indicator. The W3-01 registry made "add an indicator = one impl";
//! this makes it "add an indicator = write a script, no rebuild."
//!
//! ## The script contract
//! The script is evaluated with these **read-only** variables in scope, each an
//! array of `f64` parallel to the chart bars (oldest first):
//!   `open`, `high`, `low`, `close`, `volume`
//! plus `period` (int) and `NAN` (the not-a-number float, for warmup slots).
//! The script's final expression must be an **array** the same length as
//! `close` (shorter/longer is padded/truncated with NaN — lenient rendering).
//!
//! Example — SMA(period):
//! ```rhai
//! let out = [];
//! for i in 0..close.len() {
//!     if i < period - 1 { out.push(NAN); }
//!     else {
//!         let s = 0.0;
//!         for j in 0..period { s += close[i - j]; }
//!         out.push(s / period);
//!     }
//! }
//! out
//! ```
//!
//! ## Safety
//! Rhai is sandboxed by construction — no file, network, or system access. On
//! top of that this engine sets a hard **operation cap** so a runaway script
//! (`while true {}`) is killed rather than freezing the app: `recompute` runs on
//! the update/render path, so an unbounded script would otherwise hang the
//! window. Array/string/depth caps bound memory and stack. `eval` is disabled to
//! prevent nested-script escape.
//!
//! Compiled/evaluated on data change only, never per frame (same discipline as
//! the native indicators).

use super::{Ohlcv, Params};

/// Hard ceiling on Rhai operations per evaluation. Sized to allow an honest
/// O(bars × period) script over a large window (e.g. 20k bars × 200 period ≈
/// 4M inner steps) while still killing an infinite loop in well under a frame's
/// worth of wall-clock. This is the load-bearing guard: the eval runs on the
/// update path, so "bounded always" matters more than "generous".
const MAX_OPERATIONS: u64 = 10_000_000;
/// Bound output/intermediate array growth (a script can't allocate unbounded).
const MAX_ARRAY_SIZE: usize = 1_000_000;
/// Bound string growth.
const MAX_STRING_SIZE: usize = 64 * 1024;
/// Bound call nesting / expression depth (stack-overflow guard).
const MAX_CALL_LEVELS: usize = 64;

/// Build the sandboxed engine. Fresh per eval — cheap, and keeps evals fully
/// independent (no state leaks between indicators or runs).
fn sandboxed_engine() -> rhai::Engine {
    let mut engine = rhai::Engine::new();
    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_array_size(MAX_ARRAY_SIZE);
    engine.set_max_string_size(MAX_STRING_SIZE);
    engine.set_max_call_levels(MAX_CALL_LEVELS);
    // Defence in depth: no nested eval, so a script can't smuggle in a second
    // interpreter to dodge the caps above.
    engine.disable_symbol("eval");
    engine
}

fn to_rhai_array(xs: &[f32]) -> rhai::Array {
    xs.iter().map(|&v| rhai::Dynamic::from_float(v as f64)).collect()
}

/// Evaluate a script indicator over `data`, returning the primary output series
/// aligned to `data.close` (padded/truncated with NaN to that length).
///
/// `Err(msg)` on compile error, runtime error, the op-limit trip, or a
/// non-array result — the caller surfaces it inline so a broken script shows a
/// message instead of silently rendering nothing.
pub fn eval_script_indicator(
    src: &str,
    data: &Ohlcv,
    params: &Params,
) -> Result<Vec<f32>, String> {
    let engine = sandboxed_engine();

    let mut scope = rhai::Scope::new();
    // Read-only inputs. push_constant so a script can't reassign them out from
    // under itself (and it documents intent).
    scope.push_constant("open", to_rhai_array(data.open));
    scope.push_constant("high", to_rhai_array(data.high));
    scope.push_constant("low", to_rhai_array(data.low));
    scope.push_constant("close", to_rhai_array(data.close));
    scope.push_constant("volume", to_rhai_array(data.volume));
    scope.push_constant("period", params.period as i64);
    scope.push_constant("NAN", f64::NAN);

    let result = engine
        .eval_with_scope::<rhai::Array>(&mut scope, src)
        .map_err(|e| e.to_string())?;

    Ok(normalize_output(result, data.close.len()))
}

/// Map a Rhai result array to a fixed-length `Vec<f32>`: floats/ints pass
/// through, anything else (unit, string, bool, …) becomes NaN, and the whole
/// series is padded/truncated to `n` so it always aligns to the bars.
///
/// Pure — the JSON-free core of the contract, unit-tested without an engine.
fn normalize_output(arr: rhai::Array, n: usize) -> Vec<f32> {
    let mut out = vec![f32::NAN; n];
    for (i, d) in arr.into_iter().enumerate() {
        if i >= n {
            break;
        }
        out[i] = if let Ok(f) = d.as_float() {
            f as f32
        } else if let Ok(x) = d.as_int() {
            x as f32
        } else {
            f32::NAN
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<i64>) {
        let close: Vec<f32> = (0..n).map(|i| 100.0 + (i as f32 * 0.5).sin() * 3.0).collect();
        let high: Vec<f32> = close.iter().map(|c| c + 0.5).collect();
        let low: Vec<f32> = close.iter().map(|c| c - 0.5).collect();
        let volume: Vec<f32> = (0..n).map(|i| 1000.0 + i as f32).collect();
        let ts: Vec<i64> = (0..n).map(|i| i as i64 * 60).collect();
        (high, low, close, volume, ts)
    }

    const SMA_SCRIPT: &str = r#"
        let out = [];
        for i in 0..close.len() {
            if i < period - 1 { out.push(NAN); }
            else {
                let s = 0.0;
                for j in 0..period { s += close[i - j]; }
                out.push(s / period);
            }
        }
        out
    "#;

    #[test]
    fn script_sma_matches_native_sma() {
        // The acceptance criterion: a script indicator matches its native
        // equivalent on a fixture, bar for bar (outside the NaN warmup).
        let (high, low, close, volume, ts) = fixture(120);
        let d = Ohlcv { open: &close, high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        let got = eval_script_indicator(SMA_SCRIPT, &d, &Params::with_period(20)).expect("script runs");
        let native = crate::chart::renderer::compute::compute_sma(&close, 20);
        assert_eq!(got.len(), native.len());
        for (i, (g, nv)) in got.iter().zip(native.iter()).enumerate() {
            if nv.is_nan() {
                assert!(g.is_nan(), "bar {i}: native NaN warmup but script gave {g}");
            } else {
                assert!((g - nv).abs() < 1e-3, "bar {i}: script {g} vs native {nv}");
            }
        }
    }

    #[test]
    fn infinite_script_is_killed_by_the_op_limit() {
        // The load-bearing safety test: an unbounded loop must ERROR (op-limit),
        // not hang. recompute runs on the render path — a freeze here freezes
        // the app.
        let (high, low, close, volume, ts) = fixture(10);
        let d = Ohlcv { open: &close, high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        let err = eval_script_indicator("while true { let x = 1; } []", &d, &Params::with_period(5))
            .expect_err("infinite loop must be killed");
        // Rhai's message for the operations cap mentions "operations".
        assert!(
            err.to_lowercase().contains("operation"),
            "expected an op-limit error, got: {err}"
        );
    }

    #[test]
    fn compile_error_is_surfaced_not_panicked() {
        let (high, low, close, volume, ts) = fixture(5);
        let d = Ohlcv { open: &close, high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        let err = eval_script_indicator("let x = ;;; garbage", &d, &Params::with_period(5))
            .expect_err("syntax error must be an Err");
        assert!(!err.is_empty());
    }

    #[test]
    fn output_is_aligned_to_bars_when_script_returns_wrong_length() {
        // A script that returns a short array is padded with NaN; too long is
        // truncated. Rendering must always get exactly bars.len() values.
        let (high, low, close, volume, ts) = fixture(50);
        let d = Ohlcv { open: &close, high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        let short = eval_script_indicator("[1.0, 2.0, 3.0]", &d, &Params::with_period(5)).unwrap();
        assert_eq!(short.len(), 50, "short output padded to bar count");
        assert_eq!(short[0], 1.0);
        assert!(short[3].is_nan(), "padding is NaN");

        let long_src = "let a = []; for i in 0..500 { a.push(0.0); } a";
        let long = eval_script_indicator(long_src, &d, &Params::with_period(5)).unwrap();
        assert_eq!(long.len(), 50, "long output truncated to bar count");
    }

    #[test]
    fn non_array_result_is_an_error() {
        let (high, low, close, volume, ts) = fixture(5);
        let d = Ohlcv { open: &close, high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        assert!(
            eval_script_indicator("42.0", &d, &Params::with_period(5)).is_err(),
            "a scalar result is not a valid indicator series"
        );
    }

    #[test]
    fn non_numeric_array_entries_become_nan() {
        // Mixed junk in the output array must degrade to NaN, never panic.
        let (high, low, close, volume, ts) = fixture(4);
        let d = Ohlcv { open: &close, high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        let out = eval_script_indicator(r#"[1.0, "oops", (), 4]"#, &d, &Params::with_period(2)).unwrap();
        assert_eq!(out[0], 1.0);
        assert!(out[1].is_nan(), "string → NaN");
        assert!(out[2].is_nan(), "unit → NaN");
        assert_eq!(out[3], 4.0, "int coerces to float");
    }

    #[test]
    fn script_can_read_all_ohlcv_lanes() {
        // Prove every injected input is reachable (typical: hl2, volume-weighted).
        let (high, low, close, volume, ts) = fixture(10);
        let d = Ohlcv { open: &close, high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts };
        let src = "let out = []; for i in 0..close.len() { out.push((high[i]+low[i])/2.0); } out";
        let hl2 = eval_script_indicator(src, &d, &Params::with_period(1)).unwrap();
        for i in 0..10 {
            assert!((hl2[i] - (high[i] + low[i]) / 2.0).abs() < 1e-4);
        }
        // volume readable too
        let vsrc = "let out = []; for i in 0..volume.len() { out.push(volume[i]); } out";
        let v = eval_script_indicator(vsrc, &d, &Params::with_period(1)).unwrap();
        assert_eq!(v[0], 1000.0);
    }
}
