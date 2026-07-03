//! Playbook persistence, sharing, and level helpers (WS-E E2 extraction).
//!
//! Extracted verbatim from gpu.rs to shrink that god-file. gpu.rs re-exports
//! this module (`pub(crate) use super::playbook_store::*;`) so every existing
//! `gpu::plays_path()` / `gpu::author_handle()` / ... call site resolves
//! unchanged. `super::Play` / `super::ExprCtx` / `super::SnapLevel` /
//! `super::round_step` live in chart::renderer (mod.rs); `Chart` lives in the
//! sibling gpu module.

use super::gpu::Chart;

// ── Plays persistence (playbook durability) ─────────────────────────────────
// Plays were previously in-memory only (lost on restart). They now serialize to
// a versioned JSON file via the standard `state::persistence` envelope. `Play`
// and all sub-types already derive serde.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct PlaysFile {
    #[serde(default)]
    pub plays: Vec<super::Play>,
}
impl crate::state::persistence::Persistable for PlaysFile {
    const KEY: &'static str = "plays";
    const VERSION: u32 = 1;
}
fn plays_data_file(name: &str) -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push(name);
    p
}
/// Real on-disk store the app loads at startup and the editor writes to.
pub(crate) fn plays_path() -> std::path::PathBuf { plays_data_file("plays.json") }
pub(crate) fn load_plays_from(path: &std::path::Path) -> Vec<super::Play> {
    crate::state::persistence::load::<PlaysFile>(path).map(|f| f.plays).unwrap_or_default()
}
pub(crate) fn save_plays_to(path: &std::path::Path, plays: &[super::Play]) {
    let _ = crate::state::persistence::save(path, &PlaysFile { plays: plays.to_vec() });
}
/// Load/save the real playbook (startup + editor mutations).
pub(crate) fn load_plays() -> Vec<super::Play> { load_plays_from(&plays_path()) }
pub(crate) fn save_plays(plays: &[super::Play]) { save_plays_to(&plays_path(), plays) }
/// Separate store used ONLY by the debug PersistPlays/ReloadPlays commands so the
/// test harness can exercise the save/load round-trip without ever touching the
/// user's real plays.json.
#[cfg(debug_assertions)]
pub(crate) fn plays_debug_path() -> std::path::PathBuf { plays_data_file("plays.debug.json") }

// ── Play export / import (sharing foundation) ───────────────────────────────
// A single play serializes to portable JSON — the payload behind every share
// transport (file, clipboard, deep link, Discord, feed). `Play` is serde-ready.
pub(crate) fn export_play_json(play: &super::Play) -> String {
    serde_json::to_string_pretty(play).unwrap_or_default()
}
pub(crate) fn import_play_json(s: &str) -> Option<super::Play> {
    serde_json::from_str::<super::Play>(s).ok()
}
/// Write a play to a `.play.json` file (real export uses the file dialog; this is
/// the underlying transport, also used by the harness round-trip).
pub(crate) fn export_play_to_file(play: &super::Play, path: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(path, export_play_json(play))
}
pub(crate) fn import_play_from_file(path: &std::path::Path) -> Option<super::Play> {
    std::fs::read_to_string(path).ok().and_then(|s| import_play_json(&s))
}
/// Debug export target so the harness can round-trip export→import through a real
/// file without a file dialog and without touching user data.
#[cfg(debug_assertions)]
pub(crate) fn play_export_debug_path() -> std::path::PathBuf { plays_data_file("play.debug.export.json") }

/// Format a play as a shareable message (Discord embed / social post). Contains
/// the essence: symbol, direction, levels, R:R, author, and a short thesis line.
pub(crate) fn format_play_embed(play: &super::Play) -> String {
    let author = if play.author.is_empty() { "anon" } else { play.author.as_str() };
    let thesis = play.notes.lines().next().unwrap_or("");
    format!(
        "📈 {} {}  •  entry {:.2} → target {:.2}  •  stop {:.2}  •  R:R {:.2}:1  •  by @{}{}",
        play.symbol, play.direction.label(),
        play.entry_price, play.target_price, play.stop_price, play.risk_reward, author,
        if thesis.is_empty() { String::new() } else { format!("\n{thesis}") },
    )
}
/// Debug capture of the last share payload so the harness can assert the embed
/// format WITHOUT ever actually posting to Discord.
#[cfg(debug_assertions)]
static LAST_SHARE: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();
#[cfg(debug_assertions)]
pub(crate) fn set_last_share(s: String) {
    let c = LAST_SHARE.get_or_init(|| std::sync::Mutex::new(String::new()));
    if let Ok(mut g) = c.lock() { *g = s; }
}
#[cfg(debug_assertions)]
pub(crate) fn last_share() -> String {
    LAST_SHARE.get().and_then(|c| c.lock().ok().map(|g| g.clone())).unwrap_or_default()
}
/// Debug capture of the last snap result (price, label) for harness assertions.
#[cfg(debug_assertions)]
static LAST_SNAP: std::sync::OnceLock<std::sync::Mutex<(f32, String)>> = std::sync::OnceLock::new();
#[cfg(debug_assertions)]
pub(crate) fn set_last_snap(price: f32, label: &str) {
    let c = LAST_SNAP.get_or_init(|| std::sync::Mutex::new((0.0, String::new())));
    if let Ok(mut g) = c.lock() { *g = (price, label.to_string()); }
}
#[cfg(debug_assertions)]
pub(crate) fn last_snap() -> (f32, String) {
    LAST_SNAP.get().and_then(|c| c.lock().ok().map(|g| g.clone())).unwrap_or((0.0, String::new()))
}
/// Debug capture of the last options-payoff computation (for harness assertions).
#[cfg(debug_assertions)]
static LAST_PAYOFF: std::sync::OnceLock<std::sync::Mutex<f32>> = std::sync::OnceLock::new();
#[cfg(debug_assertions)]
pub(crate) fn set_last_payoff(v: f32) {
    let c = LAST_PAYOFF.get_or_init(|| std::sync::Mutex::new(0.0));
    if let Ok(mut g) = c.lock() { *g = v; }
}
#[cfg(debug_assertions)]
pub(crate) fn last_payoff() -> f32 {
    LAST_PAYOFF.get().and_then(|c| c.lock().ok().map(|g| *g)).unwrap_or(0.0)
}

/// Set a pane's play-line overlay (Entry/Target/Stop) from explicit prices — used
/// to lay a play leg's levels onto its own pane in a multi-instrument play (P3).
pub(crate) fn set_pane_play_lines(chart: &mut Chart, entry: f32, target: f32, stop: f32) {
    use super::{PlayLine, PlayLineKind};
    chart.play_lines.clear();
    let mut id = 1u32;
    chart.play_lines.push(PlayLine { id, kind: PlayLineKind::Entry, price: entry }); id += 1;
    chart.play_lines.push(PlayLine { id, kind: PlayLineKind::Target, price: target }); id += 1;
    if stop > 0.0 {
        chart.play_lines.push(PlayLine { id, kind: PlayLineKind::Stop, price: stop }); id += 1;
    }
    chart.next_play_line_id = id;
}

/// Build the inline-expression context for a play from its levels + the chart
/// showing its symbol (gamma walls, price, a simple ATR and VWAP over recent bars).
pub(crate) fn play_expr_ctx(entry: f32, stop: f32, target: f32, chart: Option<&Chart>) -> super::ExprCtx {
    let mut c = super::ExprCtx { entry, stop, target, ..Default::default() };
    if let Some(ch) = chart {
        c.callwall = ch.gamma_call_wall;
        c.putwall = ch.gamma_put_wall;
        c.flip = ch.gamma_zero;
        if let Some(b) = ch.bars.last() { c.price = b.close; }
        let n = ch.bars.len();
        if n > 0 {
            let atr_lo = n.saturating_sub(14);
            let recent = &ch.bars[atr_lo..];
            if !recent.is_empty() {
                c.atr = recent.iter().map(|b| b.high - b.low).sum::<f32>() / recent.len() as f32;
            }
            let vw_lo = n.saturating_sub(40);
            let (mut pv, mut vv) = (0.0f32, 0.0f32);
            for b in &ch.bars[vw_lo..] {
                let typ = (b.high + b.low + b.close) / 3.0;
                pv += typ * b.volume; vv += b.volume;
            }
            if vv > 0.0 { c.vwap = pv / vv; }
        }
    }
    c
}

// ── Snap candidates (magnetic level dragging) ───────────────────────────────
/// Gather the key levels a play line can snap to on this chart: gamma call/put
/// walls + flip, round numbers around the last price, and the recent swing
/// high/low. apex already computes gamma; this just surfaces it as snap targets.
pub(crate) fn snap_candidates(chart: &Chart) -> Vec<super::SnapLevel> {
    use super::SnapLevel;
    let mut v: Vec<SnapLevel> = Vec::new();
    if chart.gamma_call_wall > 0.0 { v.push(SnapLevel { price: chart.gamma_call_wall, label: "call wall".into() }); }
    if chart.gamma_put_wall  > 0.0 { v.push(SnapLevel { price: chart.gamma_put_wall,  label: "put wall".into() }); }
    if chart.gamma_zero      > 0.0 { v.push(SnapLevel { price: chart.gamma_zero,       label: "gamma flip".into() }); }
    if let Some(last) = chart.bars.last() {
        let p = last.close;
        if p > 0.0 {
            // Round numbers around the last price.
            let step = super::round_step(p);
            let base = (p / step).round() * step;
            for k in -3..=3i32 {
                let rp = base + k as f32 * step;
                if rp > 0.0 { v.push(SnapLevel { price: rp, label: "round".into() }); }
            }
        }
        // Recent swing high/low over the last ~40 visible bars.
        let n = chart.bars.len();
        let lo_idx = n.saturating_sub(40);
        let recent = &chart.bars[lo_idx..];
        if let Some(hi) = recent.iter().map(|b| b.high).fold(None, |m: Option<f32>, h| Some(m.map_or(h, |x| x.max(h)))) {
            v.push(SnapLevel { price: hi, label: "swing high".into() });
        }
        if let Some(lo) = recent.iter().map(|b| b.low).fold(None, |m: Option<f32>, l| Some(m.map_or(l, |x| x.min(l)))) {
            v.push(SnapLevel { price: lo, label: "swing low".into() });
        }
    }
    v
}

// ── Author identity (playbook attribution) ──────────────────────────────────
// The local user's handle, stamped on plays they create so shared plays are
// attributable. Persisted as a plain file; empty until the user sets one.
static PLAY_AUTHOR: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();
fn author_path() -> std::path::PathBuf { plays_data_file("author.txt") }
pub(crate) fn author_handle() -> String {
    PLAY_AUTHOR
        .get_or_init(|| std::sync::Mutex::new(
            std::fs::read_to_string(author_path()).unwrap_or_default().trim().to_string()))
        .lock().map(|g| g.clone()).unwrap_or_default()
}
/// Set the in-memory author without persisting (used by tests + do_reset so the
/// user's real author.txt is never touched).
pub(crate) fn set_author_handle_mem(handle: &str) {
    let cell = PLAY_AUTHOR.get_or_init(|| std::sync::Mutex::new(String::new()));
    if let Ok(mut g) = cell.lock() { *g = handle.to_string(); }
}
/// Set + persist the author handle (the real settings path).
#[allow(dead_code)]
pub(crate) fn set_author_handle(handle: &str) {
    set_author_handle_mem(handle);
    let _ = std::fs::write(author_path(), handle);
}
