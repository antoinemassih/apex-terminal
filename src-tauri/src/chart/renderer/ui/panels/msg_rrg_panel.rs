//! MSG Weighted RRG Panel (W5.6) — basket_rrg signal visualizer.
//!
//! Displays basket members (e.g. QQQ holdings: XLK, XLC, …) as points on a
//! JdK Relative Rotation Graph. Marker radius scales with basket weight so the
//! largest constituents dominate visually. Heading vectors show recent momentum
//! direction. Quadrant backgrounds follow the standard RRG palette.
//!
//! Signal source: `signals:basket_rrg:{PARENT}:{tf}`
//! Fetched via: `{APEX_SIGNALS_HTTP}/api/signals/value?key=…`
//!
//! Offline / paused-service behaviour: renders a SAMPLE DATA watermark from
//! the static `SAMPLE_EDGES` table so the panel is reviewable without the live
//! ApexSignals service.

use egui;
use std::sync::{OnceLock, Mutex, atomic::{AtomicBool, Ordering}};
use super::super::style::*;
use super::super::super::gpu::Theme;
use crate::ui_kit::sx::Tone;
use crate::ui_kit::widgets::PanelSection;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use super::side_panel_shell::{SidePanelShell, Width, RailSlot};

// ── Panel open flag (no Watchlist field needed) ────────────────────────────
pub(crate) static OPEN: AtomicBool = AtomicBool::new(false);
pub(crate) fn toggle() { OPEN.fetch_xor(true, Ordering::Relaxed); }

/// Rail registration. `is_open` reads the module-level static; no Watchlist
/// field required (additive, zero impact on frozen Watchlist/Chart structs).
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id:       "msg_rrg",
    is_open:  |_w| OPEN.load(Ordering::Relaxed),
    render:   |cx, slot| draw(cx.ctx, cx.t, Some(slot)),
};

// ── Data types ─────────────────────────────────────────────────────────────

/// One basket member on the RRG.
#[derive(Clone, Debug)]
pub struct MsgRrgEdge {
    /// Member ticker (e.g. "XLK").
    pub symbol:   String,
    /// JdK RS-Ratio centered at 100 (>100 = outperforming parent).
    pub ratio:    f32,
    /// JdK RS-Momentum centered at 100 (>100 = accelerating outperformance).
    pub momentum: f32,
    /// Basket weight [0, 1] — drives marker size.
    pub weight:   f32,
    /// Prior-period (ratio, momentum) for the heading vector.
    pub prev:     Option<(f32, f32)>,
}

impl MsgRrgEdge {
    fn quadrant(&self) -> &'static str {
        match (self.ratio >= 100.0, self.momentum >= 100.0) {
            (true,  true)  => "LEADING",
            (true,  false) => "WEAKENING",
            (false, false) => "LAGGING",
            (false, true)  => "IMPROVING",
        }
    }
}

// ── Sample data (QQQ basket, synthetic but realistic weights) ──────────────
// CALIBRATION PENDING live data — values chosen to show all four quadrants.

/// Sample basket_rrg payload for QQQ (used when live service is unavailable).
fn sample_edges() -> Vec<MsgRrgEdge> {
    let raw: &[(&str, f32, f32, f32, f32, f32)] = &[
        // (sym,  ratio,  mom,   weight, prev_ratio, prev_mom)
        ("XLK",  103.2,  102.1, 0.32,   102.8,  101.9),
        ("XLC",  101.8,  101.2, 0.17,   101.5,  101.0),
        ("XLV",   99.5,  100.8, 0.07,    99.2,  100.5),
        ("XLY",  100.8,   98.7, 0.06,   101.0,   99.1),
        ("XLI",   98.8,  100.2, 0.05,    98.5,   99.9),
        ("XLRE",  97.5,   98.3, 0.03,    97.3,   98.0),
        ("XLP",   98.2,   99.5, 0.04,    98.0,   99.2),
        ("XLF",   97.9,   98.1, 0.03,    98.1,   98.4),
        ("XLB",   99.1,   97.8, 0.02,    99.3,   98.0),
        ("XLU",   96.8,   99.2, 0.02,    96.5,   98.8),
        ("XLE",   97.2,   97.5, 0.02,    97.4,   97.8),
    ];
    raw.iter().map(|(sym, ratio, mom, weight, pr, pm)| MsgRrgEdge {
        symbol:   sym.to_string(),
        ratio:    *ratio,
        momentum: *mom,
        weight:   *weight,
        prev:     Some((*pr, *pm)),
    }).collect()
}

// ── Signal cache ────────────────────────────────────────────────────────────

struct CacheEntry {
    edges:  Vec<MsgRrgEdge>,
    at:     std::time::Instant,
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

/// Shared cached HTTP client for ApexSignals REST calls.
fn apex_signals_http() -> String {
    std::env::var("APEX_SIGNALS_HTTP")
        .unwrap_or_else(|_| "http://localhost:8100".to_string())
}

fn http_client() -> &'static reqwest::blocking::Client {
    static C: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .user_agent("apex-terminal/msg-panels")
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
}

/// Fetch basket_rrg from ApexSignals REST. Returns `None` if unreachable.
fn fetch_rrg(parent: &str, tf: &str) -> Option<Vec<MsgRrgEdge>> {
    let key = format!("signals:basket_rrg:{}:{}", parent.to_uppercase(), tf);
    let url = format!("{}/api/signals/value?key={}", apex_signals_http(), key);
    let resp = http_client().get(&url).send().ok()?;
    if !resp.status().is_success() { return None; }
    let v: serde_json::Value = resp.json().ok()?;
    // Try "members" array first, then fall back to flat field scanning.
    let members = v.get("members").or_else(|| v.get("data").and_then(|d| d.get("members")))?;
    let arr = members.as_array()?;
    let mut edges: Vec<MsgRrgEdge> = arr.iter().filter_map(|m| {
        let symbol   = m.get("symbol")?.as_str()?.to_string();
        let ratio    = m.get("ratio")?.as_f64()? as f32;
        let momentum = m.get("momentum")?.as_f64()? as f32;
        let weight   = m.get("weight").and_then(|w| w.as_f64()).unwrap_or(1.0 / arr.len() as f64) as f32;
        let prev = m.get("prev_ratio").and_then(|pr| pr.as_f64())
            .zip(m.get("prev_momentum").and_then(|pm| pm.as_f64()))
            .map(|(pr, pm)| (pr as f32, pm as f32));
        Some(MsgRrgEdge { symbol, ratio, momentum, weight, prev })
    }).collect();
    if edges.is_empty() { return None; }
    // Sort by weight descending so heaviest members are drawn on top.
    edges.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
    Some(edges)
}

/// Non-blocking refresh: returns cached data (or sample) and spawns a
/// background fetch when the cache is missing or stale.
fn get_or_refresh(parent: &str, tf: &str) -> (Vec<MsgRrgEdge>, bool) {
    let key = format!("{}:{}", parent.to_uppercase(), tf);
    // Check cache.
    if let Ok(c) = cache().lock() {
        if let Some(e) = c.get(&key) {
            let stale = e.at.elapsed().as_secs() >= TTL_SECS;
            if !stale { return (e.edges.clone(), e.is_sample); }
        }
    }
    // Spawn background fetch (deduped).
    let already_fetching = {
        let mut inf = match inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
        if inf.contains(&key) { true } else { inf.insert(key.clone()); false }
    };
    if !already_fetching {
        let k2 = key.clone();
        let parent = parent.to_uppercase();
        let tf = tf.to_string();
        crate::foundation::guard::spawn_guarded("fetch", move || {
            let result = fetch_rrg(&parent, &tf);
            let (edges, is_sample) = match result {
                Some(e) => (e, false),
                None    => (sample_edges(), true),
            };
            if let Ok(mut c) = cache().lock() {
                c.insert(k2.clone(), CacheEntry { edges, at: std::time::Instant::now(), is_sample });
            }
            if let Ok(mut inf) = inflight().lock() { inf.remove(&k2); }
            crate::wake_native_ui();
        });
    }
    // Return sample while waiting.
    if let Ok(c) = cache().lock() {
        if let Some(e) = c.get(&key) {
            return (e.edges.clone(), e.is_sample);
        }
    }
    (sample_edges(), true)
}

// ── Rendering ──────────────────────────────────────────────────────────────

/// Panel state: selected parent + timeframe (thread-local; render thread is single).
std::thread_local! {
    static PARENT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static TF:     std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn current_parent() -> String {
    PARENT.with(|p| {
        let b = p.borrow();
        if b.is_empty() { "QQQ".to_string() } else { b.clone() }
    })
}
fn current_tf() -> String {
    TF.with(|t| {
        let b = t.borrow();
        if b.is_empty() { "1d".to_string() } else { b.clone() }
    })
}

const PARENTS: &[&str] = &["QQQ", "SPY", "IWM", "XLK", "SMH"];
const TFS:     &[&str] = &["1d", "1w"];

/// Draw the panel content (called by `draw` inside the `SidePanelShell`).
pub(crate) fn draw_content(ui: &mut egui::Ui, t: &Theme) {
    // Selector row.
    let parent = current_parent();
    let tf     = current_tf();
    let (edges, is_sample) = get_or_refresh(&parent, &tf);

    ui.horizontal(|ui| {
        ui.label(TextStyle::Caption.as_rich_cascading("BASKET", t.dim));
        egui::ComboBox::from_id_salt("msg_rrg_parent")
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
        ui.label(TextStyle::Caption.as_rich_cascading("TF", t.dim));
        egui::ComboBox::from_id_salt("msg_rrg_tf")
            .selected_text(tf.as_str())
            .width(42.0)
            .show_ui(ui, |ui| {
                for &f in TFS {
                    let mut cur = tf.clone();
                    if ui.selectable_value(&mut cur, f.to_string(), f).changed() {
                        TF.with(|r| *r.borrow_mut() = cur);
                    }
                }
            });
        if is_sample {
            ui.label(
                TextStyle::Caption.as_rich_cascading("SAMPLE DATA", tint(t, Tone::Warn, alpha_active())),
            );
        }
    });
    ui.add_space(gap_xs());

    PanelSection::new("WEIGHTED RRG").show(ui, t, |ui, t| {
        let avail = ui.available_size();
        let plot_size = avail.x.min(avail.y - 30.0).max(80.0);
        let (response, painter) = ui.allocate_painter(
            egui::vec2(plot_size, plot_size),
            egui::Sense::hover(),
        );
        draw_rrg_scatter(&painter, response.rect, &edges, t);
    });

    // Legend.
    PanelSection::new("LEGEND").show(ui, t, |ui, t| {
        let n = edges.len();
        let half = (n + 1) / 2;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                for e in edges.iter().take(half) {
                    let c = member_color(t, &e.symbol, e.quadrant());
                    ui.horizontal(|ui| {
                        let (r, painter) = ui.allocate_painter(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        painter.circle_filled(r.rect.center(), 3.5, c);
                        ui.label(TextStyle::Caption.as_rich_cascading(&format!("{} {:.0}%", e.symbol, e.weight * 100.0), t.text));
                    });
                }
            });
            ui.vertical(|ui| {
                for e in edges.iter().skip(half) {
                    let c = member_color(t, &e.symbol, e.quadrant());
                    ui.horizontal(|ui| {
                        let (r, painter) = ui.allocate_painter(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        painter.circle_filled(r.rect.center(), 3.5, c);
                        ui.label(TextStyle::Caption.as_rich_cascading(&format!("{} {:.0}%", e.symbol, e.weight * 100.0), t.text));
                    });
                }
            });
        });
    });
}

/// Map quadrant to a theme-aware color tone.
///
/// Now actually theme-aware: the four RRG quadrant colours live on `Theme`
/// (`rrg_leading/improving/weakening/lagging`) and are tuned for every palette.
/// This fn used to hardcode them, so the panel rendered identical colours on
/// all ~19 themes — including the light ones, where they clashed.
fn member_color(t: &Theme, symbol: &str, quadrant: &str) -> egui::Color32 {
    // Deterministic hue shift from symbol hash for easy visual discrimination.
    let h = symbol.bytes().fold(0u32, |a, b| a.wrapping_add(b as u32));
    let base = match quadrant {
        "LEADING"   => t.rrg_leading,
        "IMPROVING" => t.rrg_improving,
        "WEAKENING" => t.rrg_weakening,
        "LAGGING"   => t.rrg_lagging,
        _           => t.dim,
    };
    // Slightly vary brightness by symbol hash to distinguish members.
    let shift = (h % 40) as i32 - 20;
    let adj = |v: u8| (v as i32 + shift).clamp(60, 240) as u8;
    egui::Color32::from_rgb(adj(base.r()), adj(base.g()), adj(base.b()))
}

/// Core painter routine — quadrant backgrounds, axes, dots, vectors.
fn draw_rrg_scatter(
    painter: &egui::Painter,
    rect: egui::Rect,
    edges: &[MsgRrgEdge],
    t: &Theme,
) {
    // Data range — symmetric around 100.
    let padding = 1.5_f32;
    let mut dx: f32 = 2.0 + padding;
    let mut dy: f32 = 2.0 + padding;
    for e in edges {
        dx = dx.max((e.ratio - 100.0).abs() + padding);
        dy = dy.max((e.momentum - 100.0).abs() + padding);
        if let Some((pr, pm)) = e.prev {
            dx = dx.max((pr - 100.0).abs() + padding);
            dy = dy.max((pm - 100.0).abs() + padding);
        }
    }
    let margin = 18.0_f32;
    let pr = rect.shrink(margin);

    let to_screen = |ratio: f32, mom: f32| -> egui::Pos2 {
        let x = pr.left()   + (ratio - (100.0 - dx)) / (2.0 * dx) * pr.width();
        let y = pr.bottom() - (mom   - (100.0 - dy)) / (2.0 * dy) * pr.height();
        egui::pos2(x, y)
    };

    // Background.
    painter.rect_filled(rect, 0.0, t.bg);

    // Quadrant fills.
    let center = to_screen(100.0, 100.0);
    let fill_alpha = 8_u8;
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(center.x, pr.top()), egui::pos2(pr.right(), center.y)),
        0.0, tint(t, Tone::Bull, fill_alpha),
    );
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(center.x, center.y), pr.right_bottom().into()),
        0.0, tint(t, Tone::Warn, fill_alpha),
    );
    painter.rect_filled(
        egui::Rect::from_min_max(pr.left_bottom().into(), egui::pos2(center.x, center.y)),
        0.0, tint(t, Tone::Bear, fill_alpha),
    );
    painter.rect_filled(
        egui::Rect::from_min_max(pr.left_top().into(), center),
        0.0, tint(t, Tone::Accent, fill_alpha),
    );

    // Axis lines.
    let ax = tint(t, Tone::Dim, alpha_tint());
    let ax_stroke = egui::Stroke::new(stroke_std(), ax);
    painter.line_segment([egui::pos2(center.x, pr.top()), egui::pos2(center.x, pr.bottom())], ax_stroke);
    painter.line_segment([egui::pos2(pr.left(), center.y), egui::pos2(pr.right(), center.y)], ax_stroke);

    // Quadrant labels.
    let qa = 36_u8;
    let qf = mono_sm();
    painter.text(egui::pos2(pr.right()-3.0, pr.top()+3.0), egui::Align2::RIGHT_TOP,
        "LEADING",   qf.clone(), tint(t, Tone::Bull,   qa));
    painter.text(egui::pos2(pr.right()-3.0, pr.bottom()-3.0), egui::Align2::RIGHT_BOTTOM,
        "WEAKENING", qf.clone(), tint(t, Tone::Warn,   qa));
    painter.text(egui::pos2(pr.left()+3.0, pr.bottom()-3.0), egui::Align2::LEFT_BOTTOM,
        "LAGGING",   qf.clone(), tint(t, Tone::Bear,   qa));
    painter.text(egui::pos2(pr.left()+3.0, pr.top()+3.0), egui::Align2::LEFT_TOP,
        "IMPROVING", qf.clone(), tint(t, Tone::Accent, qa));

    // Draw heading vectors then dots (heaviest weight on top).
    for edge in edges {
        let pos = to_screen(edge.ratio, edge.momentum);
        let c   = member_color(t, &edge.symbol, edge.quadrant());
        // Heading vector from previous position.
        if let Some((pr, pm)) = edge.prev {
            let prev_pos = to_screen(pr, pm);
            painter.arrow(
                prev_pos,
                egui::vec2(pos.x - prev_pos.x, pos.y - prev_pos.y),
                egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha_active())),
            );
        }
        // Marker radius proportional to weight (min 4, max 12 px).
        let r = (4.0 + edge.weight.clamp(0.0, 1.0) * 30.0).min(12.0);
        painter.circle_filled(pos, r + 2.0,
            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha_subtle()));
        painter.circle_filled(pos, r, c);
        // Label.
        let lx = if pos.x + r + 26.0 > pr.right() { pos.x - r - 2.0 } else { pos.x + r + 2.0 };
        let la = if pos.x + r + 26.0 > pr.right() { egui::Align2::RIGHT_CENTER } else { egui::Align2::LEFT_CENTER };
        painter.text(egui::pos2(lx, pos.y), la, &edge.symbol, mono_sm(),
            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha_near_opaque()));
    }

    // Border.
    painter.rect_stroke(pr, 0.0,
        egui::Stroke::new(stroke_std(), tint(t, Tone::Border, alpha_muted())),
        egui::StrokeKind::Outside);
}

/// Outer panel shell. Called by the rail dispatcher.
pub(crate) fn draw(ctx: &egui::Context, t: &Theme, slot: Option<RailSlot>) {
    if !OPEN.load(Ordering::Relaxed) { return; }
    let resp = SidePanelShell::new("msg_rrg_panel", "MSG RRG")
        .width(Width::Medium)
        .resizable(240.0..=500.0)
        .rail_slot(slot)
        .show(ctx, t, |ui, t| { draw_content(ui, t); });
    if resp.close_clicked { OPEN.store(false, Ordering::Relaxed); }
}
