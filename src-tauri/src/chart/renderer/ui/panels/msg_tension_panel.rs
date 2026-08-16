//! MSG Scenario Level-Ladder Panel (W5.6) — basket_tension + range signal
//! visualizer.
//!
//! Renders the tension solver's scenario payoff as a vertical price ladder.
//! Each absorber (e.g. QQQ, SMH, IGV) gets a row showing:
//!   • Current price anchor
//!   • Predicted move band [move_lo, move_hi] as a horizontal range bar
//!   • Target price marker
//!   • Attainability heat (from the RangeModel)
//!   • Invalidation level (red dashed line: if price breaks this, scenario off)
//!
//! The scenario path (ROTATE / DISPERSE / BREAK) is shown at the top.
//! Classic example: "QQQ −1% / SMH −5% / IGV +8%" rotation trade.
//!
//! Signal sources:
//!   `signals:basket_tension:{PARENT}:{theme}`
//!   `signals:range:{SYM}`  (for per-absorber attainability bands)
//! Fetched via: `{APEX_SIGNALS_HTTP}/api/signals/value?key=…`
//!
//! Offline: renders SAMPLE DATA with the QQQ−1%/SMH−5%/IGV+8% payoff picture.

use egui;
use std::sync::{OnceLock, Mutex, atomic::{AtomicBool, Ordering}};
use super::super::style::*;
use super::super::super::gpu::Theme;
use crate::ui_kit::sx::Tone;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use crate::ui_kit::widgets::PanelSection;
use super::side_panel_shell::{SidePanelShell, Width, RailSlot};

// ── Panel open flag ────────────────────────────────────────────────────────
pub(crate) static OPEN: AtomicBool = AtomicBool::new(false);
pub(crate) fn toggle() { OPEN.fetch_xor(true, Ordering::Relaxed); }

pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id:      "msg_tension",
    is_open: |_w| OPEN.load(Ordering::Relaxed),
    render:  |cx, slot| draw(cx.ctx, cx.t, Some(slot)),
};

// ── Data types ─────────────────────────────────────────────────────────────

/// Solver scenario from basket_tension signal.
#[derive(Clone, Debug)]
pub struct TensionScenario {
    /// Rotation path: "ROTATE" | "DISPERSE" | "BREAK".
    pub path:          String,
    /// Confidence in this scenario [0, 1].
    pub confidence:    f32,
    /// Invalidation price level for the parent — scenario breaks above/below this.
    pub invalidation:  f32,
    /// Whether invalidation fires on a breach ABOVE (true) or BELOW (false).
    pub inval_is_above: bool,
    /// Moves for each absorber.
    pub absorbers:     Vec<AbsorberMove>,
    /// Whether calibrated bands are present (W5.3 output).
    pub has_bands:     bool,
}

/// Per-absorber predicted move from the solver.
#[derive(Clone, Debug)]
pub struct AbsorberMove {
    pub symbol:        String,
    /// Predicted move as signed %. Negative = expected to fall.
    pub move_pct:      f32,
    /// Calibrated lower bound (bias-adjusted). Present after W5.3 ships.
    pub move_lo:       f32,
    /// Calibrated upper bound. Present after W5.3 ships.
    pub move_hi:       f32,
    /// Current price (used to compute target).
    pub current_price: f32,
    /// RangeModel attainability [0, 1] — soft feasibility weight.
    pub attainability: f32,
    /// Whether this move is in the bullish direction for this absorber.
    pub is_bull:       bool,
}

impl AbsorberMove {
    pub fn target_price(&self) -> f32 {
        self.current_price * (1.0 + self.move_pct / 100.0)
    }
}

// ── Sample data (QQQ−1%/SMH−5%/IGV+8% sector rotation) ────────────────────
// CALIBRATION PENDING live data.

fn sample_scenario() -> TensionScenario {
    TensionScenario {
        path:            "ROTATE".to_string(),
        confidence:      0.74,
        invalidation:    488.0,
        inval_is_above:  true,
        has_bands:       true,
        absorbers:       vec![
            AbsorberMove {
                symbol:        "QQQ".to_string(),
                move_pct:      -1.0,
                move_lo:       -2.1,
                move_hi:       -0.2,
                current_price: 480.0,
                attainability: 0.72,
                is_bull:       false,
            },
            AbsorberMove {
                symbol:        "SMH".to_string(),
                move_pct:      -5.0,
                move_lo:       -7.8,
                move_hi:       -2.4,
                current_price: 240.0,
                attainability: 0.65,
                is_bull:       false,
            },
            AbsorberMove {
                symbol:        "IGV".to_string(),
                move_pct:      8.0,
                move_lo:       4.2,
                move_hi:       12.1,
                current_price: 85.0,
                attainability: 0.45,
                is_bull:       true,
            },
            AbsorberMove {
                symbol:        "XLK".to_string(),
                move_pct:      -0.5,
                move_lo:       -1.8,
                move_hi:       0.4,
                current_price: 220.0,
                attainability: 0.68,
                is_bull:       false,
            },
        ],
    }
}

// ── Signal cache ────────────────────────────────────────────────────────────

struct CacheEntry {
    scenario:  TensionScenario,
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

const TTL_SECS: u64 = 20;

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

/// Fetch basket_tension scenario. Returns `None` when service is unreachable.
fn fetch_tension(parent: &str, theme: &str) -> Option<TensionScenario> {
    let key = format!("signals:basket_tension:{}:{}", parent.to_uppercase(), theme);
    let url = format!("{}/api/signals/value?key={}", apex_signals_http(), key);
    let resp = http_client().get(&url).send().ok()?;
    if !resp.status().is_success() { return None; }
    let v: serde_json::Value = resp.json().ok()?;
    let data = v.get("data").unwrap_or(&v);

    let path = data.get("path").and_then(|x| x.as_str()).unwrap_or("ROTATE").to_string();
    let confidence   = data.get("confidence").and_then(|x| x.as_f64()).unwrap_or(0.5) as f32;
    let invalidation = data.get("invalidation").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    let inval_above  = data.get("inval_is_above").and_then(|x| x.as_bool()).unwrap_or(true);
    // move_lo/hi presence indicates W5.3 calibrated bands.
    let has_bands = data.get("absorbers")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|m| m.get("move_lo"))
        .is_some();

    let absorbers = data.get("absorbers")?.as_array()?
        .iter().filter_map(|m| {
            let symbol       = m.get("symbol")?.as_str()?.to_string();
            let move_pct     = m.get("move_pct")?.as_f64()? as f32;
            let move_lo      = m.get("move_lo").and_then(|x| x.as_f64()).unwrap_or(move_pct as f64) as f32;
            let move_hi      = m.get("move_hi").and_then(|x| x.as_f64()).unwrap_or(move_pct as f64) as f32;
            let current_price= m.get("current_price").and_then(|x| x.as_f64()).unwrap_or(100.0) as f32;
            let attainability= m.get("attainability").and_then(|x| x.as_f64()).unwrap_or(0.5) as f32;
            let is_bull      = move_pct >= 0.0;
            Some(AbsorberMove { symbol, move_pct, move_lo, move_hi, current_price, attainability, is_bull })
        }).collect::<Vec<_>>();

    if absorbers.is_empty() { return None; }
    Some(TensionScenario { path, confidence, invalidation, inval_is_above: inval_above, has_bands, absorbers })
}

fn get_or_refresh(parent: &str, theme: &str) -> (TensionScenario, bool) {
    let key = format!("{}:{}", parent.to_uppercase(), theme);
    if let Ok(c) = cache().lock() {
        if let Some(e) = c.get(&key) {
            if e.at.elapsed().as_secs() < TTL_SECS { return (e.scenario.clone(), e.is_sample); }
        }
    }
    let already = {
        let mut inf = match inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
        if inf.contains(&key) { true } else { inf.insert(key.clone()); false }
    };
    if !already {
        let k2 = key.clone();
        let p2 = parent.to_uppercase();
        let th = theme.to_string();
        crate::foundation::guard::spawn_guarded("fetch", move || {
            let result = fetch_tension(&p2, &th);
            let (scenario, is_sample) = result.map_or_else(|| (sample_scenario(), true), |s| (s, false));
            if let Ok(mut c) = cache().lock() {
                c.insert(k2.clone(), CacheEntry { scenario, at: std::time::Instant::now(), is_sample });
            }
            if let Ok(mut inf) = inflight().lock() { inf.remove(&k2); }
            crate::wake_native_ui();
        });
    }
    if let Ok(c) = cache().lock() {
        if let Some(e) = c.get(&key) { return (e.scenario.clone(), e.is_sample); }
    }
    (sample_scenario(), true)
}

// ── Panel state ─────────────────────────────────────────────────────────────

std::thread_local! {
    static PARENT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static THEME:  std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}
fn current_parent() -> String {
    PARENT.with(|p| { let b = p.borrow(); if b.is_empty() { "QQQ".to_string() } else { b.clone() } })
}
fn current_theme() -> String {
    THEME.with(|t| { let b = t.borrow(); if b.is_empty() { "sector_rotation".to_string() } else { b.clone() } })
}

const PARENTS: &[&str] = &["QQQ", "SPY", "IWM"];
const THEMES:  &[&str] = &["sector_rotation", "rate_shock", "risk_off"];

// ── Rendering ──────────────────────────────────────────────────────────────

/// Path label color by scenario type.
fn path_tone(path: &str) -> Tone {
    match path {
        "ROTATE"   => Tone::Accent,
        "DISPERSE" => Tone::Warn,
        "BREAK"    => Tone::Bear,
        _          => Tone::Dim,
    }
}

pub(crate) fn draw_content(ui: &mut egui::Ui, t: &Theme) {
    let parent = current_parent();
    let theme  = current_theme();
    let (sc, is_sample) = get_or_refresh(&parent, &theme);

    // Selector + scenario header.
    ui.horizontal(|ui| {
        ui.label(TextStyle::Caption.as_rich_cascading("BASKET", t.dim));
        egui::ComboBox::from_id_salt("msg_ten_parent")
            .selected_text(parent.as_str())
            .width(58.0)
            .show_ui(ui, |ui| {
                for &p in PARENTS {
                    let mut cur = parent.clone();
                    if ui.selectable_value(&mut cur, p.to_string(), p).changed() {
                        PARENT.with(|r| *r.borrow_mut() = cur);
                    }
                }
            });
        ui.label(TextStyle::Caption.as_rich_cascading("THEME", t.dim));
        egui::ComboBox::from_id_salt("msg_ten_theme")
            .selected_text(theme.replace('_', " ").to_uppercase())
            .width(110.0)
            .show_ui(ui, |ui| {
                for &th in THEMES {
                    let mut cur = theme.clone();
                    if ui.selectable_value(&mut cur, th.to_string(),
                        th.replace('_', " ").to_uppercase()).changed() {
                        THEME.with(|r| *r.borrow_mut() = cur);
                    }
                }
            });
        if is_sample {
            ui.label(
                TextStyle::Caption.as_rich_cascading("SAMPLE DATA", tint(t, Tone::Warn, alpha_active())),
            );
        }
    });

    // Scenario headline.
    ui.horizontal(|ui| {
        let pton = path_tone(&sc.path);
        ui.label(TextStyle::Body.as_rich_cascading(&sc.path, tint(t, pton, alpha_near_opaque())).strong());
        ui.label(TextStyle::Caption.as_rich_cascading(&format!("conf {:.0}%", sc.confidence * 100.0), t.dim));
        if sc.invalidation > 0.0 {
            let above = if sc.inval_is_above { "↑" } else { "↓" };
            ui.label(TextStyle::Caption.as_rich_cascading(&format!("inval {above}{:.1}", sc.invalidation), tint(t, Tone::Bear, alpha_active())));
        }
    });
    ui.add_space(gap_xs());

    PanelSection::new("LEVEL LADDER").show(ui, t, |ui, t| {
        let avail_w = ui.available_width();
        draw_ladder(ui, t, avail_w, &sc);
    });
}

/// Draw the vertical price-ladder chart inline.
fn draw_ladder(ui: &mut egui::Ui, t: &Theme, avail_w: f32, sc: &TensionScenario) {
    let n = sc.absorbers.len();
    if n == 0 { return; }

    // Each absorber gets a fixed-height row.
    let row_h: f32 = 52.0;
    let total_h = row_h * n as f32 + 8.0;

    let (resp, painter) = ui.allocate_painter(
        egui::vec2(avail_w, total_h),
        egui::Sense::hover(),
    );
    let area = resp.rect;

    // Column layout:
    // [label 36px] [price ladder bar fills the rest] [attain 30px]
    let label_w  = 36.0_f32;
    let attain_w = 30.0_f32;
    let bar_w    = (avail_w - label_w - attain_w - gap_sm() * 2.0).max(60.0);

    // Compute % range across all absorbers for scaling the bar.
    let max_abs_pct = sc.absorbers.iter()
        .map(|a| a.move_lo.abs().max(a.move_hi.abs()))
        .fold(1.0_f32, f32::max)
        .max(0.5);

    for (idx, abs) in sc.absorbers.iter().enumerate() {
        let row_top  = area.top() + idx as f32 * row_h;
        let row_rect = egui::Rect::from_min_size(
            egui::pos2(area.left(), row_top),
            egui::vec2(avail_w, row_h - 2.0),
        );

        // Alternating row background.
        if idx % 2 == 1 {
            painter.rect_filled(row_rect, 0.0, tint(t, Tone::Surface, 6));
        }

        let mid_y = row_rect.center().y;
        let bar_origin_x = area.left() + label_w + gap_sm();
        let bar_center_x = bar_origin_x + bar_w / 2.0;

        // ── Label ──────────────────────────────────────────────────────────
        painter.text(
            egui::pos2(area.left() + crate::ui_kit::style::gap_2xs(), mid_y),
            egui::Align2::LEFT_CENTER,
            &abs.symbol,
            mono_sm(),
            t.text,
        );

        // ── Central axis (0% = center of bar) ─────────────────────────────
        painter.line_segment(
            [egui::pos2(bar_center_x, row_rect.top() + crate::ui_kit::style::gap_2xs()),
             egui::pos2(bar_center_x, row_rect.bottom() - crate::ui_kit::style::gap_2xs())],
            egui::Stroke::new(stroke_thin(), tint(t, Tone::Dim, alpha_subtle())),
        );

        // ── Move band bar ([move_lo, move_hi]) ────────────────────────────
        let half_w  = bar_w / 2.0;
        let pct_to_x = |pct: f32| bar_center_x + pct / max_abs_pct * half_w;

        let lo_x = pct_to_x(abs.move_lo.min(abs.move_hi));
        let hi_x = pct_to_x(abs.move_lo.max(abs.move_hi));
        let bar_top    = mid_y - 6.0;
        let bar_bottom = mid_y + 6.0;
        let tone = if abs.is_bull { Tone::Bull } else { Tone::Bear };
        // Band fill.
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(lo_x, bar_top), egui::pos2(hi_x, bar_bottom)),
            radius_xs(),
            tint(t, tone, crate::ui_kit::style::alpha_hint()),
        );
        // Band border.
        painter.rect_stroke(
            egui::Rect::from_min_max(egui::pos2(lo_x, bar_top), egui::pos2(hi_x, bar_bottom)),
            radius_xs(),
            egui::Stroke::new(stroke_thin(), tint(t, tone, alpha_active())),
            egui::StrokeKind::Outside,
        );

        // ── Target price marker (center of band) ───────────────────────────
        let target_x = pct_to_x(abs.move_pct);
        painter.line_segment(
            [egui::pos2(target_x, bar_top - 3.0), egui::pos2(target_x, bar_bottom + 3.0)],
            egui::Stroke::new(stroke_std(), tint(t, tone, alpha_near_opaque())),
        );

        // Current price (dot at 0%).
        painter.circle_filled(egui::pos2(bar_center_x, mid_y), 3.0, t.dim);

        // ── Labels: move% + target price ──────────────────────────────────
        let move_label = format!("{:+.1}%", abs.move_pct);
        let price_label = format!("${:.1}", abs.target_price());
        let move_color = tint(t, tone, alpha_near_opaque());
        // Place label outside the band.
        let lbl_x = if abs.move_pct >= 0.0 {
            (hi_x + 4.0).min(bar_origin_x + bar_w - 30.0)
        } else {
            (lo_x - 4.0).max(bar_origin_x + 2.0)
        };
        let lbl_align = if abs.move_pct >= 0.0 { egui::Align2::LEFT_CENTER } else { egui::Align2::RIGHT_CENTER };
        painter.text(egui::pos2(lbl_x, mid_y - 7.0), lbl_align, &move_label,  mono_sm(), move_color);
        painter.text(egui::pos2(lbl_x, mid_y + 7.0), lbl_align, &price_label, mono_sm(), t.dim);

        // ── Attainability bar (right column) ───────────────────────────────
        let att_x0 = area.left() + label_w + gap_sm() + bar_w + gap_sm();
        let att_x1 = att_x0 + attain_w;
        let att_inner = attain_w * abs.attainability.clamp(0.0, 1.0);
        // Background.
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(att_x0, mid_y - 4.0), egui::pos2(att_x1, mid_y + 4.0)),
            radius_xs(),
            tint(t, Tone::Surface, crate::ui_kit::style::alpha_faint()),
        );
        // Fill.
        let att_tone = if abs.attainability > 0.6 { Tone::Bull } else if abs.attainability > 0.35 { Tone::Warn } else { Tone::Bear };
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(att_x0, mid_y - 4.0), egui::pos2(att_x0 + att_inner, mid_y + 4.0)),
            radius_xs(),
            tint(t, att_tone, alpha_muted()),
        );
        // % label.
        painter.text(
            egui::pos2((att_x0 + att_x1) / 2.0, mid_y + 10.0),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}%", abs.attainability * 100.0),
            mono_sm(),
            t.dim,
        );

        // ── Row separator ─────────────────────────────────────────────────
        painter.line_segment(
            [egui::pos2(area.left(), row_rect.bottom()),
             egui::pos2(area.right(), row_rect.bottom())],
            egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_subtle())),
        );
    }

    // ── Invalidation level overlay (red dashed line through the QQQ row) ──
    if sc.invalidation > 0.0 {
        // Draw a dashed red rule at the top of the first absorber row as a
        // cross-symbol invalidation indicator.  (A proper price-axis mapping
        // across different symbols would need a unified price scale, which is
        // out of scope for a side panel.)
        let iv_y = area.top() + crate::ui_kit::style::gap_2xs();
        // A dashed rule, drawn as an INDEXED period rather than a walked `x`.
        //
        // This one is deliberately NOT an element tree. A dash pattern is
        // stroke decoration, not layout: declaring ~30 sibling nodes to draw
        // one hairline would cost a Taffy solve per repaint to express
        // something a multiply already says exactly. The accumulator went
        // anyway, because `x` stepping by a stride is an index in disguise and
        // reads better as one.
        let dash = 6.0_f32;
        let period = dash * 2.0;
        let dashes = (area.width() / period).ceil().max(0.0) as usize;
        for i in 0..dashes {
            let x0 = area.left() + i as f32 * period;
            let x1 = (x0 + dash).min(area.right());
            painter.line_segment(
                [egui::pos2(x0, iv_y), egui::pos2(x1, iv_y)],
                egui::Stroke::new(stroke_bold(), tint(t, Tone::Bear, alpha_muted())),
            );
        }
        painter.text(
            egui::pos2(area.right() - crate::ui_kit::style::gap_2xs(), iv_y + 1.0),
            egui::Align2::RIGHT_TOP,
            format!("inval ${:.1}", sc.invalidation),
            mono_sm(),
            tint(t, Tone::Bear, alpha_active()),
        );
    }

    // Column headers above the chart.
    // (painted separately because allocate_painter claims the area above too)
    // We embed these as a quick label row before the painter area.
}

pub(crate) fn draw(ctx: &egui::Context, t: &Theme, slot: Option<RailSlot>) {
    if !OPEN.load(Ordering::Relaxed) { return; }
    let resp = SidePanelShell::new("msg_tension_panel", "SCENARIO")
        .width(Width::Medium)
        .resizable(260.0..=560.0)
        .rail_slot(slot)
        .show(ctx, t, |ui, t| { draw_content(ui, t); });
    if resp.close_clicked { OPEN.store(false, Ordering::Relaxed); }
}
