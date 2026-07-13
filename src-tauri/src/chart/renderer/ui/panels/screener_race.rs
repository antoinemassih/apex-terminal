//! Screener Race panel — T-GRID wave S2.
//!
//! Renders the live-race view (view 4 of the 5-view screener UX defined in
//! `docs/audit/screener-next/06-terminal-ux.md`).
//!
//! # Data source
//! `GET /api/scan/race/:id?limit=50` — SSE stream served by ApexData S1.
//! Each SSE frame carries:
//! ```text
//! event: race
//! data: {"scan_id":"…","ts_ms":…,"rows":[{"symbol":"…","score":…,"values":{…}},…]}
//! ```
//!
//! ## SSE / poll strategy
//! A single background thread per active scan_id maintains the connection:
//! - **SSE path**: `reqwest::blocking::Client` with `Accept: text/event-stream`,
//!   reads line-by-line via `BufReader`. On each complete event (blank separator),
//!   parses `data:` line, updates `RaceState` under `Mutex`, and calls
//!   `ctx.request_repaint_after(100ms)` to animate.
//! - **Poll fallback**: if SSE fails (connection refused, non-200, stream closed),
//!   falls back to a plain `GET /api/scan/race/:id?limit=50` polled every 5 s.
//!   ApexData returns the current ranked snapshot on that REST path.
//!
//! Switching `scan_id` stops the old thread (it detects the id mismatch) and
//! starts a new one.
//!
//! # Visual features
//! - Rolling ranked list via `PanelListRow::columns` (no-alloc per row).
//! - **Entry flash** (`RowAnim::Entry`): new entry → `row_tint(t.bull, alpha)`
//!   fading over `FLASH_MS` ms.
//! - **Exit fade** (`RowAnim::Exit`): dropped symbol → one additional frame at
//!   decaying alpha and `row_tint(t.bear, alpha)`.
//! - **Rank-change arrows**: ▲ / ▼ / — in the rank column.
//! - `ctx.request_repaint_after(100ms)` keeps animation smooth.
//!
//! # State contract (T-SHELL owns ScreenPanelState)
//! This module reads:
//! - `screen_state.active_screen_id: Option<String>` — SSE URL param.
//!
//! Animation state (`Instant` per entry/exit) lives inside `RaceState` global,
//! NOT in T-SHELL's `ScreenPanelState`, to avoid mid-wave structural changes.
//!
//! # AppCommand needs (request to T-SHELL)
//! - `AppCommand::ScreenRowToChart(sym: String)` — row click loads chart.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::Color32;
use serde::Deserialize;

use super::super::style::*;
use super::super::super::gpu::Theme;
use crate::ui_kit::widgets::{PanelColAlign, PanelColumn, PanelEmpty, PanelListRow, PanelSection};

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RaceRowWire {
    pub symbol: String,
    pub score: Option<f64>,
    #[serde(default)]
    pub values: HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct RaceFrameWire {
    #[allow(dead_code)]
    pub scan_id: String,
    #[allow(dead_code)]
    pub ts_ms: i64,
    pub rows: Vec<RaceRowWire>,
}

// ── Per-row animation ─────────────────────────────────────────────────────────

#[derive(Clone)]
enum RowAnim {
    Entry(Instant),
    Exit(Instant),
    Stable,
}

const FLASH_MS: u128 = 800;
const FADE_MS: u128  = 600;

impl RowAnim {
    fn alpha_mult(&self) -> f32 {
        match self {
            RowAnim::Entry(t) => {
                let e = t.elapsed().as_millis();
                if e >= FLASH_MS { 1.0 }
                else { 1.0 - 0.6 * (1.0 - e as f32 / FLASH_MS as f32) }
            }
            RowAnim::Exit(t) => {
                let e = t.elapsed().as_millis();
                if e >= FADE_MS { 0.0 } else { 1.0 - e as f32 / FADE_MS as f32 }
            }
            RowAnim::Stable => 1.0,
        }
    }
    fn is_expired(&self) -> bool {
        matches!(self, RowAnim::Exit(t) if t.elapsed().as_millis() >= FADE_MS)
    }
    fn is_animating(&self) -> bool {
        !matches!(self, RowAnim::Stable)
    }
}

// ── In-memory race state ──────────────────────────────────────────────────────

struct LiveRow {
    symbol: String,
    score: Option<f64>,
    rank: usize,
    prev_rank: Option<usize>,
    anim: RowAnim,
}

#[derive(Default)]
struct RaceState {
    scan_id: String,
    rows: Vec<LiveRow>,
    prior_symbols: HashMap<String, usize>,
    last_frame_at: Option<Instant>,
    running: bool,
    poll_fallback: bool,
}

static RACE: OnceLock<Mutex<RaceState>> = OnceLock::new();

fn race() -> &'static Mutex<RaceState> {
    RACE.get_or_init(|| Mutex::new(RaceState::default()))
}

// ── Background connection thread ──────────────────────────────────────────────

fn ensure_thread(scan_id: &str, ctx: egui::Context) {
    {
        let state = race().lock().unwrap();
        if state.running && state.scan_id == scan_id { return; }
    }
    {
        let mut state = race().lock().unwrap();
        state.scan_id = scan_id.to_string();
        state.running = true;
        state.rows.clear();
        state.prior_symbols.clear();
        state.poll_fallback = false;
    }
    let id = scan_id.to_string();
    std::thread::spawn(move || connection_loop(id, ctx));
}

fn connection_loop(scan_id: String, ctx: egui::Context) {
    use crate::apex_data::config::{apex_url, apex_token};
    use reqwest::blocking::Client;

    let base = apex_url();
    let token = apex_token();
    let sse_url  = format!("{base}/api/scan/race/{scan_id}?limit=50");
    let poll_url = sse_url.clone();

    // Build a client with a long timeout for SSE streaming.
    let client = {
        let b = Client::builder()
            .timeout(Duration::from_secs(35))
            .connect_timeout(Duration::from_secs(3))
            .user_agent("apex-terminal/0.9");
        match b.build() { Ok(c) => c, Err(_) => Client::new() }
    };

    let mut use_poll = false;

    loop {
        // Exit if scan_id was replaced.
        {
            if race().lock().unwrap().scan_id != scan_id { break; }
        }

        if use_poll {
            // ── Poll fallback every 5s ─────────────────────────────────────
            std::thread::sleep(Duration::from_secs(5));
            {
                if race().lock().unwrap().scan_id != scan_id { break; }
            }
            let mut req = client.get(&poll_url);
            if let Some(ref tok) = token { req = req.bearer_auth(tok); }
            if let Ok(resp) = req.send() {
                if let Ok(frame) = resp.json::<RaceFrameWire>() {
                    apply_frame(frame);
                    ctx.request_repaint();
                }
            }
        } else {
            // ── SSE attempt ───────────────────────────────────────────────
            let mut req = client.get(&sse_url)
                .header("Accept", "text/event-stream")
                .header("Cache-Control", "no-cache");
            if let Some(ref tok) = token { req = req.bearer_auth(tok); }

            match req.send() {
                Ok(resp) if resp.status().is_success() => {
                    {
                        let mut s = race().lock().unwrap();
                        s.poll_fallback = false;
                    }
                    let reader = BufReader::new(resp);
                    let mut pending_data: Option<String> = None;

                    for line_res in reader.lines() {
                        // Guard against scan_id change mid-stream.
                        { if race().lock().unwrap().scan_id != scan_id { return; } }

                        let line = match line_res { Ok(l) => l, Err(_) => break };

                        if let Some(data) = line.strip_prefix("data:") {
                            pending_data = Some(data.trim().to_string());
                        } else if line.is_empty() {
                            if let Some(data) = pending_data.take() {
                                if let Ok(frame) = serde_json::from_str::<RaceFrameWire>(&data) {
                                    apply_frame(frame);
                                    ctx.request_repaint_after(Duration::from_millis(100));
                                }
                            }
                        }
                    }
                    // Stream ended → fall back.
                    use_poll = true;
                    race().lock().unwrap().poll_fallback = true;
                }
                _ => {
                    use_poll = true;
                    race().lock().unwrap().poll_fallback = true;
                }
            }
        }
    }

    // Mark thread as done.
    if let Ok(mut s) = race().lock() {
        if s.scan_id == scan_id { s.running = false; }
    }
}

fn apply_frame(frame: RaceFrameWire) {
    let Ok(mut state) = race().lock() else { return; };
    state.last_frame_at = Some(Instant::now());

    let new_syms: HashMap<String, usize> = frame.rows.iter()
        .enumerate()
        .map(|(i, r)| (r.symbol.clone(), i))
        .collect();

    let mut rows: Vec<LiveRow> = frame.rows.iter().enumerate().map(|(rank, wire)| {
        let prev_rank = state.prior_symbols.get(&wire.symbol).copied();
        let anim = if prev_rank.is_none() { RowAnim::Entry(Instant::now()) }
                   else { RowAnim::Stable };
        LiveRow { symbol: wire.symbol.clone(), score: wire.score, rank, prev_rank, anim }
    }).collect();

    // Carry forward exit-fading rows for symbols that just dropped off.
    for (sym, prior_rank) in &state.prior_symbols {
        if !new_syms.contains_key(sym) {
            let already_fading = state.rows.iter()
                .any(|r| r.symbol == *sym && matches!(r.anim, RowAnim::Exit(_)));
            if !already_fading {
                if let Some(prev) = state.rows.iter().find(|r| r.symbol == *sym) {
                    rows.push(LiveRow {
                        symbol: sym.clone(),
                        score: prev.score,
                        rank: rows.len(),
                        prev_rank: Some(*prior_rank),
                        anim: RowAnim::Exit(Instant::now()),
                    });
                }
            }
        }
    }

    rows.retain(|r| !r.anim.is_expired());
    state.prior_symbols = new_syms;
    state.rows = rows;
}

// ── Rank-change arrow ─────────────────────────────────────────────────────────

fn rank_arrow(rank: usize, prev: Option<usize>) -> (&'static str, Color32) {
    // Returns (glyph, color) — color resolved against Theme at call site.
    let _ = rank; // used for comparison below
    match prev {
        None           => ("·",  Color32::TRANSPARENT), // placeholder; caller overrides
        Some(p) if p > rank => ("▲", Color32::TRANSPARENT),
        Some(p) if p < rank => ("▼", Color32::TRANSPARENT),
        _              => ("—",  Color32::TRANSPARENT),
    }
}

/// Resolve rank-arrow color from theme.
fn arrow_color(rank: usize, prev: Option<usize>, t: &Theme) -> Color32 {
    match prev {
        None => t.dim,
        Some(p) if p > rank => t.bull,
        Some(p) if p < rank => t.bear,
        _ => t.dim,
    }
}

// ── Public draw entry ─────────────────────────────────────────────────────────

/// Draw the race view into `ui`.
///
/// Called by T-SHELL's tab dispatch when `view_mode == ScreenerView::Race`.
pub fn draw_race(
    ui: &mut egui::Ui,
    scan_id: Option<&str>,
    ctx: &egui::Context,
    t: &Theme,
    pending_sym_to_chart: &mut Option<String>,
) {
    let Some(sid) = scan_id else {
        PanelEmpty::new("No screen selected")
            .hint("Open a screen in the Library tab first")
            .show(ui, t);
        return;
    };

    ensure_thread(sid, ctx.clone());

    let panel_w = ui.available_width();

    // Read current snapshot (avoid holding lock during UI paint).
    struct Snap {
        rows: Vec<(String, Option<f64>, usize, Option<usize>, f32, bool)>,
        poll_fallback: bool,
        stale: bool,
        any_animating: bool,
    }
    let snap = {
        let s = race().lock().unwrap();
        let stale = s.last_frame_at.map(|t| t.elapsed().as_secs() > 10).unwrap_or(true);
        let any_animating = s.rows.iter().any(|r| r.anim.is_animating());
        let rows = s.rows.iter().map(|r| (
            r.symbol.clone(),
            r.score,
            r.rank,
            r.prev_rank,
            r.anim.alpha_mult(),
            matches!(r.anim, RowAnim::Exit(_)),
        )).collect();
        Snap { rows, poll_fallback: s.poll_fallback, stale, any_animating }
    };

    // Status chip.
    let status_str = if snap.stale && snap.rows.is_empty() { "Connecting…" }
        else if snap.poll_fallback { "Poll mode (SSE unavailable)" }
        else { "Live" };
    ui.add(egui::Label::new(
        egui::RichText::new(status_str)
            .monospace().size(font_2xs())
            .color(if snap.stale && snap.rows.is_empty() { t.warn } else { color_muted(t.dim) })
    ));
    separator(ui, t.toolbar_border);

    if snap.rows.is_empty() {
        PanelEmpty::new("Waiting for race data")
            .hint("Score appears when the scan evaluates")
            .show(ui, t);
        ctx.request_repaint_after(Duration::from_millis(500));
        return;
    }

    // Column widths.
    let rank_w  = 22.0_f32;
    let sym_w   = 56.0_f32;
    let arr_w   = 16.0_f32;
    let score_w = (panel_w - rank_w - sym_w - arr_w - gap_sm() * 2.0).max(40.0);

    // Header.
    ui.horizontal(|ui| {
        ui.add_space(gap_2xs());
        let hdr = color_muted(t.dim);
        col_header(ui, "#",     rank_w,  hdr, true);
        col_header(ui, "SYM",   sym_w,   hdr, false);
        col_header(ui, "",      arr_w,   hdr, false);
        col_header(ui, "SCORE", score_w, hdr, true);
    });
    separator(ui, t.toolbar_border);

    // Rows.
    egui::ScrollArea::vertical()
        .id_salt("screener_race_scroll")
        .show(ui, |ui| {
            ui.set_min_width(panel_w - 4.0);

            for (row_idx, (sym, score, rank, prev_rank, alpha, is_exit)) in snap.rows.iter().enumerate() {
                if *alpha < 0.01 { continue; }

                let id_salt = format!("race_row_{row_idx}");

                let rank_str  = format!("{}", rank + 1);
                let score_str = score.map(|s| format!("{:.3}", s)).unwrap_or_else(|| "—".into());
                let (arrow_glyph, _) = rank_arrow(*rank, *prev_rank);
                let arr_color = arrow_color(*rank, *prev_rank, t);

                // Tint: entry (alpha fading in) = bull, exit (alpha fading out) = bear.
                let (tint_color, tint_alpha) = if *is_exit {
                    (t.bear, ((*alpha * 20.0) as u8).min(20))
                } else if *alpha < 1.0 {
                    (t.bull, ((1.0 - alpha) * 20.0) as u8)
                } else {
                    (t.dim, 0u8)
                };

                let resp = PanelListRow::new(&id_salt)
                    .row_tint(tint_color, tint_alpha)
                    .columns(&[
                        PanelColumn {
                            text: &rank_str,
                            align: PanelColAlign::Right,
                            color: color_muted(t.dim),
                            weight: rank_w / panel_w.max(1.0),
                            min_width: Some(rank_w),
                            mono: true,
                        },
                        PanelColumn {
                            text: sym,
                            align: PanelColAlign::Left,
                            color: t.text,
                            weight: sym_w / panel_w.max(1.0),
                            min_width: Some(sym_w),
                            mono: true,
                        },
                        PanelColumn {
                            text: arrow_glyph,
                            align: PanelColAlign::Left,
                            color: arr_color,
                            weight: arr_w / panel_w.max(1.0),
                            min_width: Some(arr_w),
                            mono: false,
                        },
                        PanelColumn {
                            text: &score_str,
                            align: PanelColAlign::Right,
                            color: t.text,
                            weight: score_w / panel_w.max(1.0),
                            min_width: Some(score_w),
                            mono: true,
                        },
                    ])
                    .show_full(ui, t);

                if resp.clicked {
                    *pending_sym_to_chart = Some(sym.clone());
                }
            }
        });

    // Keep repaint loop alive while animating.
    if snap.any_animating {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

/// Convenience wrapper: draw inside a `PanelSection`.
pub fn draw_section(
    ui: &mut egui::Ui,
    scan_id: Option<&str>,
    ctx: &egui::Context,
    t: &Theme,
    pending_sym_to_chart: &mut Option<String>,
) {
    let count = race().lock().map(|s| s.rows.len()).unwrap_or(0);
    let title = if count > 0 { format!("RACE ({count})") } else { "RACE".to_string() };
    PanelSection::new(&title).show(ui, t, |ui, t| {
        draw_race(ui, scan_id, ctx, t, pending_sym_to_chart);
    });
}
