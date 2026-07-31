//! MSG Influence Heatmap Panel (W5.6) — basket_influence signal visualizer.
//!
//! Shows how much each basket member drives the parent: sorted rows with a
//! color-scaled `dyn_share` bar (wider = more flow influence) and an `IR`
//! (information ratio) indicator. Green IR = member helps the basket trend;
//! red IR = member works against it.
//!
//! Signal source: `signals:basket_influence:{PARENT}:{window}`
//! Fetched via:   `{APEX_SIGNALS_HTTP}/api/signals/value?key=…`
//!
//! Offline / paused-service behaviour: renders a SAMPLE DATA watermark from
//! the static sample table so the panel is reviewable without live data.

use egui;
use std::sync::{OnceLock, Mutex, atomic::{AtomicBool, Ordering}};
use super::super::style::*;
use super::super::super::gpu::Theme;
use crate::ui_kit::sx::Tone;
use crate::ui_kit::widgets::{PanelSection, PanelListRow, PanelColumn};
use super::side_panel_shell::{SidePanelShell, Width, RailSlot};
use crate::chart_renderer::ui::foundation::text_style::TextStyle;

// ── Panel open flag ────────────────────────────────────────────────────────
pub(crate) static OPEN: AtomicBool = AtomicBool::new(false);
pub(crate) fn toggle() { OPEN.fetch_xor(true, Ordering::Relaxed); }

pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id:      "msg_influence",
    is_open: |_w| OPEN.load(Ordering::Relaxed),
    render:  |cx, slot| draw(cx.ctx, cx.t, Some(slot)),
};

// ── Data types ─────────────────────────────────────────────────────────────

/// Per-member influence row from basket_influence signal.
#[derive(Clone, Debug)]
pub struct InfluenceRow {
    /// Basket member ticker.
    pub symbol:    String,
    /// Dynamic flow share [0, 1] — fraction of parent's move absorbed by this member.
    pub dyn_share: f32,
    /// Information ratio (signed). Positive = aligned with basket direction.
    pub ir:        f32,
}

// ── Sample data (QQQ basket, pre-sorted by dyn_share) ─────────────────────
// CALIBRATION PENDING live data.

fn sample_rows() -> Vec<InfluenceRow> {
    let raw: &[(&str, f32, f32)] = &[
        ("XLK",   0.38,  1.22),
        ("XLC",   0.22,  0.84),
        ("XLY",   0.15, -0.31),
        ("XLRE",  0.09, -0.58),
        ("XLV",   0.07,  0.41),
        ("XLI",   0.04,  0.19),
        ("XLB",   0.03, -0.12),
        ("XLF",   0.02,  0.07),
    ];
    raw.iter().map(|(sym, ds, ir)| InfluenceRow {
        symbol: sym.to_string(), dyn_share: *ds, ir: *ir,
    }).collect()
}

// ── Signal cache ────────────────────────────────────────────────────────────

struct CacheEntry {
    rows:      Vec<InfluenceRow>,
    at:        std::time::Instant,
    is_sample: bool,
}

fn cache() -> &'static Mutex<std::collections::HashMap<String, CacheEntry>> {
    static C: OnceLock<Mutex<std::collections::HashMap<String, CacheEntry>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}
fn inflight() -> &'static Mutex<std::collections::HashSet<String>> {
    static I: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    I.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

const TTL_SECS: u64 = 30;

fn apex_signals_http() -> String {
    std::env::var("APEX_SIGNALS_HTTP")
        .unwrap_or_else(|_| "http://localhost:8100".to_string())
}
fn http_client() -> &'static reqwest::blocking::Client {
    static C: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    C.get_or_init(|| reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("apex-terminal/msg-panels")
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new()))
}

fn fetch_influence(parent: &str, window: &str) -> Option<Vec<InfluenceRow>> {
    let key = format!("signals:basket_influence:{}:{}", parent.to_uppercase(), window);
    let url = format!("{}/api/signals/value?key={}", apex_signals_http(), key);
    let resp = http_client().get(&url).send().ok()?;
    if !resp.status().is_success() { return None; }
    let v: serde_json::Value = resp.json().ok()?;
    // Parse "members" array.
    let members = v.get("members").or_else(|| v.get("data").and_then(|d| d.get("members")))?;
    let arr = members.as_array()?;
    let mut rows: Vec<InfluenceRow> = arr.iter().filter_map(|m| {
        let symbol    = m.get("symbol")?.as_str()?.to_string();
        let dyn_share = m.get("dyn_share")?.as_f64()? as f32;
        let ir        = m.get("ir").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
        Some(InfluenceRow { symbol, dyn_share, ir })
    }).collect();
    if rows.is_empty() { return None; }
    rows.sort_by(|a, b| b.dyn_share.partial_cmp(&a.dyn_share).unwrap_or(std::cmp::Ordering::Equal));
    Some(rows)
}

fn get_or_refresh(parent: &str, window: &str) -> (Vec<InfluenceRow>, bool) {
    let key = format!("{}:{}", parent.to_uppercase(), window);
    if let Ok(c) = cache().lock() {
        if let Some(e) = c.get(&key) {
            if e.at.elapsed().as_secs() < TTL_SECS { return (e.rows.clone(), e.is_sample); }
        }
    }
    let already = {
        let mut inf = match inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
        if inf.contains(&key) { true } else { inf.insert(key.clone()); false }
    };
    if !already {
        let k2 = key.clone();
        let p2 = parent.to_uppercase();
        let w2 = window.to_string();
        crate::foundation::guard::spawn_guarded("fetch", move || {
            let result = fetch_influence(&p2, &w2);
            let (rows, is_sample) = result.map_or_else(|| (sample_rows(), true), |r| (r, false));
            if let Ok(mut c) = cache().lock() {
                c.insert(k2.clone(), CacheEntry { rows, at: std::time::Instant::now(), is_sample });
            }
            if let Ok(mut inf) = inflight().lock() { inf.remove(&k2); }
            crate::wake_native_ui();
        });
    }
    if let Ok(c) = cache().lock() {
        if let Some(e) = c.get(&key) { return (e.rows.clone(), e.is_sample); }
    }
    (sample_rows(), true)
}

// ── Panel state ─────────────────────────────────────────────────────────────

std::thread_local! {
    static PARENT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static WINDOW: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}
fn current_parent() -> String {
    PARENT.with(|p| { let b = p.borrow(); if b.is_empty() { "QQQ".to_string() } else { b.clone() } })
}
fn current_window() -> String {
    WINDOW.with(|w| { let b = w.borrow(); if b.is_empty() { "30".to_string()  } else { b.clone() } })
}

const PARENTS: &[&str] = &["QQQ", "SPY", "IWM", "XLK", "SMH"];
const WINDOWS: &[&str] = &["10", "30", "60"];

// ── Rendering ──────────────────────────────────────────────────────────────

pub(crate) fn draw_content(ui: &mut egui::Ui, t: &Theme) {
    let parent = current_parent();
    let window = current_window();
    let (rows, is_sample) = get_or_refresh(&parent, &window);

    // Selector row.
    ui.horizontal(|ui| {
        ui.label(TextStyle::Caption.as_rich("BASKET", t.dim));
        egui::ComboBox::from_id_salt("msg_inf_parent")
            .selected_text(parent.as_str())
            .width(60.0)
            .show_ui(ui, |ui| {
                for &p in PARENTS {
                    let mut cur = parent.clone();
                    if ui.selectable_value(&mut cur, p.to_string(), p).changed() {
                        PARENT.with(|r| *r.borrow_mut() = cur);
                    }
                }
            });
        ui.label(TextStyle::Caption.as_rich("WIN", t.dim));
        egui::ComboBox::from_id_salt("msg_inf_window")
            .selected_text(window.as_str())
            .width(40.0)
            .show_ui(ui, |ui| {
                for &w in WINDOWS {
                    let mut cur = window.clone();
                    if ui.selectable_value(&mut cur, w.to_string(), w).changed() {
                        WINDOW.with(|r| *r.borrow_mut() = cur);
                    }
                }
            });
        if is_sample {
            ui.label(
                TextStyle::Caption.as_rich("SAMPLE DATA", tint(t, Tone::Warn, alpha_active())),
            );
        }
    });
    ui.add_space(gap_xs());

    PanelSection::new("FLOW INFLUENCE").show(ui, t, |ui, t| {
        // Header.
        PanelListRow::new("inf_hdr")
            .columns(&[
                PanelColumn::left("MEMBER").color(t.dim),
                PanelColumn::right("DYN SHARE").color(t.dim),
                PanelColumn::right("IR").color(t.dim),
            ])
            .hoverable(false)
            .show(ui, t);

        // Find max dyn_share for bar scaling.
        let max_ds = rows.iter().map(|r| r.dyn_share).fold(0.0_f32, f32::max).max(0.01);

        for (idx, row) in rows.iter().enumerate() {
            let ds_pct = format!("{:.0}%", row.dyn_share * 100.0);
            let ir_str = format!("{:+.2}", row.ir);
            let ir_color = if row.ir > 0.2 { t.bull } else if row.ir < -0.2 { t.bear } else { t.dim };

            // Draw a painter-based heatmap bar behind the row.
            let row_h = crate::chart_renderer::ui::style::style_row_height();
            let (bar_resp, bar_painter) =
                ui.allocate_painter(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());
            let bar_rect = bar_resp.rect;

            // Heatmap background: width = dyn_share / max, green-to-red by IR sign.
            let fill_w = (row.dyn_share / max_ds) * bar_rect.width();
            let fill_alpha = 28_u8;
            let fill_color = if row.ir >= 0.0 {
                tint(t, Tone::Bull, fill_alpha)
            } else {
                tint(t, Tone::Bear, fill_alpha)
            };
            bar_painter.rect_filled(
                egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, bar_rect.height())),
                0.0, fill_color,
            );

            // Row text (overlaid on the bar).
            let text_y = bar_rect.center().y;
            let x0 = bar_rect.left() + gap_xs();
            let x1 = bar_rect.right() - gap_xs();
            bar_painter.text(
                egui::pos2(x0, text_y), egui::Align2::LEFT_CENTER,
                &row.symbol, mono_sm(),
                if idx == 0 { t.text } else { t.dim },
            );
            bar_painter.text(
                egui::pos2((x0 + x1) / 2.0, text_y), egui::Align2::CENTER_CENTER,
                &ds_pct, mono_sm(), t.dim,
            );
            bar_painter.text(
                egui::pos2(x1, text_y), egui::Align2::RIGHT_CENTER,
                &ir_str, mono_sm(), ir_color,
            );

            // Subtle separator.
            bar_painter.line_segment(
                [egui::pos2(bar_rect.left(), bar_rect.bottom()),
                 egui::pos2(bar_rect.right(), bar_rect.bottom())],
                egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_subtle())),
            );
        }
    });
}

pub(crate) fn draw(ctx: &egui::Context, t: &Theme, slot: Option<RailSlot>) {
    if !OPEN.load(Ordering::Relaxed) { return; }
    let resp = SidePanelShell::new("msg_influence_panel", "INFLUENCE")
        .width(Width::Medium)
        .resizable(240.0..=500.0)
        .rail_slot(slot)
        .show(ctx, t, |ui, t| { draw_content(ui, t); });
    if resp.close_clicked { OPEN.store(false, Ordering::Relaxed); }
}
