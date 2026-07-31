//! SOTA §4.5 — SpikeExplanationPopup overlay.
//!
//! Transient toast in the top-right corner of the chart window. Multiple
//! active toasts stack vertically (oldest first), capped at 3 visible to
//! avoid screen takeover. Each toast carries:
//!   - symbol + sigma badge
//!   - headline (bold)
//!   - explanation (smaller, dim)
//!   - source chips (clickable, up to 2 visible)
//!   - [view chart] [view provenance] [×] buttons
//!
//! State machine (per §4.5):
//!   receive → enqueue (skip if dismissed)
//!   dismiss (× or auto-timeout 30s) → mark dismissed, drop from active
//!   re-receive same id → no-op (dedup against active OR dismissed)
//!   receive new id → enqueue
//!
//! Coalescing: if a spike for the same symbol arrives within 5 minutes of an
//! existing active toast, update that toast's (sigma, pct_move, t_ms,
//! headline, explanation) in place rather than stacking a second toast.
//!
//! Rendering: drawn via `egui::Area` with absolute positioning in the
//! top-right corner of the active chart's window. Not a panel — overlays
//! the panel grid.

#![allow(dead_code)]

use std::collections::{HashSet, VecDeque};
use crate::chart_renderer::ui::style::tint;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use crate::ui_kit::sx::Tone;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui;

use crate::chart_renderer::ui::style::{
    gap_2xs, gap_xs, alpha_solid, alpha_muted,
    stroke_std,
    radius_sm,
    font_md, font_sm, font_xs_plus, font_xs,
    shadow_tooltip_themed,
};
use crate::data::apex_data::live_state;
use crate::data::apex_data::types::SpikeExplanation;
use crate::ui_kit::widgets::Button as KitButton;
use crate::ui_kit::widgets::Tooltip;
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};
use crate::ui_kit::widgets::icon_placement::IconPlacement;

/// Max active toasts displayed simultaneously (oldest first, newer ones queued
/// off-screen until an active slot frees). Anything beyond is invisible until
/// dismissed.
pub const MAX_VISIBLE: usize = 3;
/// How long a single toast lives before auto-dismissal. Per §4.5 = 30s.
pub const AUTO_DISMISS: Duration = Duration::from_secs(30);
/// Coalescing window — two spikes for the same symbol within this window are
/// merged into one toast (sigma/pct/headline updated in place).
pub const COALESCE_WINDOW: Duration = Duration::from_secs(5 * 60);

/// One active toast (lives in the popup state, not in `live_state`). Carries
/// the spike payload plus the `Instant` it was first shown (used for both
/// auto-dismiss and coalescing).
#[derive(Debug, Clone)]
pub struct ActiveToast {
    pub spike: SpikeExplanation,
    pub shown_at: Instant,
}

/// Public state container — exposed for tests; the production code path keeps
/// the singleton in `popup_state()`. The struct is split out (rather than a
/// pile of `Mutex<…>`) so tests can build a fresh instance per test without
/// fighting the global.
#[derive(Default)]
pub struct SpikeExplanationPopup {
    pub active:    VecDeque<ActiveToast>,
    pub dismissed: HashSet<String>,
    pub history:   VecDeque<SpikeExplanation>,
}

impl SpikeExplanationPopup {
    /// Apply a freshly arrived spike. Implements the full state machine:
    /// - if dismissed → silently drop (server can republish freely)
    /// - if already-active with same id → no-op
    /// - if same symbol within COALESCE_WINDOW → update in place
    /// - else → enqueue at back, append to history
    ///
    /// Returns `true` iff the call mutated `active` (test convenience).
    pub fn on_spike_received(&mut self, spike: SpikeExplanation, now: Instant) -> bool {
        if self.dismissed.contains(&spike.id) {
            return false;
        }
        // Same id already shown? No-op.
        if self.active.iter().any(|a| a.spike.id == spike.id) {
            return false;
        }
        // Coalesce by symbol within the window.
        if let Some(idx) = self.active.iter().position(|a| {
            a.spike.symbol == spike.symbol && now.duration_since(a.shown_at) < COALESCE_WINDOW
        }) {
            // Update in place — keep the original shown_at so auto-dismiss
            // doesn't reset on every micro-tick, but refresh the payload so
            // the user sees the latest sigma/headline.
            self.active[idx].spike = spike.clone();
            self.history.push_back(spike);
            while self.history.len() > MAX_VISIBLE * 4 { self.history.pop_front(); }
            return true;
        }
        self.active.push_back(ActiveToast { spike: spike.clone(), shown_at: now });
        self.history.push_back(spike);
        while self.history.len() > MAX_VISIBLE * 4 { self.history.pop_front(); }
        true
    }

    /// Mark a toast id as dismissed and drop it from `active`. Idempotent.
    pub fn dismiss(&mut self, id: &str) {
        self.dismissed.insert(id.to_string());
        self.active.retain(|a| a.spike.id != id);
    }

    /// Expire any toast past AUTO_DISMISS. Returns the number expired.
    pub fn tick(&mut self, now: Instant) -> usize {
        let before = self.active.len();
        let dismissed = &mut self.dismissed;
        self.active.retain(|a| {
            let alive = now.duration_since(a.shown_at) < AUTO_DISMISS;
            if !alive { dismissed.insert(a.spike.id.clone()); }
            alive
        });
        before - self.active.len()
    }

    /// Visible slice — clamped at MAX_VISIBLE. Oldest first.
    pub fn visible(&self) -> Vec<&ActiveToast> {
        self.active.iter().take(MAX_VISIBLE).collect()
    }
}

fn popup_state() -> &'static Mutex<SpikeExplanationPopup> {
    static S: OnceLock<Mutex<SpikeExplanationPopup>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(SpikeExplanationPopup::default()))
}

/// Pull new spikes off the live_state ring and feed them through the state
/// machine. Called once per frame from `draw()` before render.
fn pump_from_live_state(now: Instant) {
    // iter_spikes returns newest-first; reverse so oldest is enqueued first
    // and visual ordering matches arrival ordering.
    let spikes = live_state::iter_spikes();
    if spikes.is_empty() { return; }
    let Ok(mut popup) = popup_state().lock() else { return };
    for spike in spikes.into_iter().rev() {
        // Also honor the global dismiss set in live_state (the popup mirror
        // is module-local; live_state survives across `popup_state` re-init
        // in hot-reload builds).
        if live_state::is_spike_dismissed(&spike.id) { continue; }
        popup.on_spike_received(spike, now);
    }
    popup.tick(now);
}

/// Emitted by [view chart] — caller hooks this into whatever chart-control
/// bus exists at integration time. Today the trade-plan panel + spike popup
/// both want this; left as a callback slot to avoid hard-wiring a specific
/// bus type here.
pub type ChartJumpCallback = Box<dyn Fn(&str, i64) + Send + Sync>;

/// Emitted by [view provenance] — caller routes this to the provenance pane
/// (which lives in a sibling agent's branch and may not be present at
/// integration time). The callback is optional; when None the button is
/// rendered as a stub tooltip ("provenance pane not wired in this build").
pub type ProvenanceJumpCallback = Box<dyn Fn(&str) + Send + Sync>;

fn callbacks() -> &'static Mutex<Callbacks> {
    static C: OnceLock<Mutex<Callbacks>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Callbacks::default()))
}

#[derive(Default)]
struct Callbacks {
    on_jump_to_chart: Option<ChartJumpCallback>,
    on_open_provenance: Option<ProvenanceJumpCallback>,
}

/// Wire up the [view chart] callback. Safe to call multiple times; last wins.
pub fn set_chart_jump_callback(cb: ChartJumpCallback) {
    if let Ok(mut c) = callbacks().lock() { c.on_jump_to_chart = Some(cb); }
}

/// Wire up the [view provenance] callback. Safe to call multiple times;
/// last wins. Leave unset to render the button as a stub.
pub fn set_provenance_callback(cb: ProvenanceJumpCallback) {
    if let Ok(mut c) = callbacks().lock() { c.on_open_provenance = Some(cb); }
}

/// Main render entry — call once per frame from the top-level layout. Draws
/// nothing when there are no active toasts.
///
/// `screen_rect` is the chart window's outer rect; we anchor in its top-right
/// corner so the popup follows the active window.
pub fn draw(ctx: &egui::Context, screen_rect: egui::Rect) {
    pump_from_live_state(Instant::now());

    // Snapshot active set + history for rendering.
    let (visible, _history_len) = {
        let popup = match popup_state().lock() { Ok(g) => g, Err(_) => return };
        let v: Vec<ActiveToast> = popup.visible().into_iter().cloned().collect();
        (v, popup.history.len())
    };
    if visible.is_empty() { return; }

    // Top-right corner, 16px inset. Each toast is ~280px wide × ~120px tall.
    // Stack downward; oldest at top to preserve reading order.
    const TOAST_W: f32 = 280.0;
    const TOAST_H: f32 = 120.0;
    const PAD: f32 = 8.0;
    let right = screen_rect.right() - 16.0;
    let mut y = screen_rect.top() + 16.0;

    let mut dismissed_now: Vec<String> = Vec::new();
    let mut jump_now: Option<(String, i64)> = None;
    let mut provenance_now: Option<String> = None;

    for (i, toast) in visible.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            egui::pos2(right - TOAST_W, y),
            egui::vec2(TOAST_W, TOAST_H),
        );
        egui::Area::new(egui::Id::new(("spike_toast", &toast.spike.id, i)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
                ui.set_min_size(egui::vec2(TOAST_W, TOAST_H));
                egui::Frame::popup(ui.style())
                    .fill(tint(&t, Tone::Surface, alpha_solid()))
                    .stroke(egui::Stroke::new(stroke_std(), tint(&t, Tone::Border, alpha_muted())))
                    .corner_radius(radius_sm())
                    .shadow(shadow_tooltip_themed(&t))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(TOAST_W - 16.0, TOAST_H - 16.0));
                        draw_toast_body(ui, &toast.spike,
                            &mut dismissed_now, &mut jump_now, &mut provenance_now);
                    });
            });
        y += TOAST_H + PAD;
    }

    // Apply collected interactions outside the render closure so we don't
    // mutate state mid-iteration.
    if !dismissed_now.is_empty() {
        if let Ok(mut popup) = popup_state().lock() {
            for id in &dismissed_now {
                popup.dismiss(id);
                live_state::dismiss_spike(id);
            }
        }
    }
    if let Some((sym, t_ms)) = jump_now {
        if let Ok(c) = callbacks().lock() {
            if let Some(cb) = c.on_jump_to_chart.as_ref() {
                cb(&sym, t_ms);
            }
        }
    }
    if let Some(lineage_id) = provenance_now {
        if let Ok(c) = callbacks().lock() {
            if let Some(cb) = c.on_open_provenance.as_ref() {
                cb(&lineage_id);
            }
        }
    }
}

fn draw_toast_body(
    ui: &mut egui::Ui,
    spike: &SpikeExplanation,
    dismiss_buf: &mut Vec<String>,
    jump_buf: &mut Option<(String, i64)>,
    provenance_buf: &mut Option<String>,
) {
    // Header row: [SYM] [σ X.Y] [+1.7%] ··· [×]
    ui.horizontal(|ui| {
        let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
        ui.label(egui::RichText::new(&spike.symbol).monospace().strong()
            .color(t.accent).size(font_md()));
        ui.label(egui::RichText::new(format!("σ{:.1}", spike.sigma))
            .monospace().color(t.warn).size(font_sm()));
        let move_col = if spike.pct_move >= 0.0 { t.bull } else { t.bear };
        ui.label(egui::RichText::new(format!("{:+.2}%", spike.pct_move))
            .monospace().color(move_col).size(font_sm()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let spike_theme = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
            let r = crate::ui_kit::widgets::Button::close()
                .placement(IconPlacement::PanelHeader)
                .show(ui, &spike_theme);
            Tooltip::new("Dismiss").show(ui, &r, &spike_theme);
            if r.clicked() {
                dismiss_buf.push(spike.id.clone());
            }
        });
    });
    ui.add_space(gap_xs());
    {
        let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
        ui.label(TextStyle::BodySm.as_rich_cascading(&spike.headline, t.text).strong());
        ui.add_space(gap_2xs());
        // Explanation — truncate visually via egui's wrapping; the data itself is
        // unbounded. We give it a fixed height and rely on egui's clip rect.
        let explanation_h = 32.0;
        ui.allocate_ui(egui::vec2(ui.available_width(), explanation_h), |ui| {
            ui.label(egui::RichText::new(&spike.explanation).size(font_xs_plus())
                .color(t.dim));
        });
        ui.add_space(gap_2xs());
        // Source chips — first 2 only.
        if !spike.sources.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for src in spike.sources.iter().take(2) {
                    let label = shorten_source(src);
                    ui.add(egui::Hyperlink::from_label_and_url(
                        egui::RichText::new(label).size(font_xs())
                            .color(t.accent)
                            .underline(),
                        src,
                    ));
                }
            });
        }
    }
    ui.add_space(gap_xs());
    ui.horizontal(|ui| {
        if small_btn(ui, "view chart").clicked() {
            *jump_buf = Some((spike.symbol.clone(), spike.t_ms));
        }
        if small_btn(ui, "view provenance").clicked() {
            // The popup payload doesn't carry a lineage_id today (the spike
            // explainer doesn't emit Provenance yet). Fall back to the
            // synthesized dedup id so the provenance pane can at least
            // resolve "the spike with id X".
            *provenance_buf = Some(spike.id.clone());
        }
    });
}

fn small_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
    KitButton::new(label).variant(KitVariant::Ghost).size(KitSize::Xs)
        .min_size(egui::vec2(0.0, 18.0))
        .show(ui, &t)
}

fn shorten_source(url: &str) -> String {
    // "https://reuters.com/article/x" → "reuters.com"
    let mut s = url.trim_start_matches("https://").trim_start_matches("http://");
    if let Some(end) = s.find('/') { s = &s[..end]; }
    if s.starts_with("www.") { s = &s[4..]; }
    s.to_string()
}

// ── Tests ────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn s(id_sym: &str, id_t: i64, sigma: f32, pct: f32) -> SpikeExplanation {
        SpikeExplanation::new(
            id_sym.to_string(), id_t, sigma, pct,
            format!("{id_sym} headline @ {id_t}"),
            format!("explanation @ {id_t}"),
            vec!["https://reuters.com/x".to_string()],
        )
    }

    #[test]
    fn receive_one_then_dismiss_then_re_receive_no_show() {
        let mut p = SpikeExplanationPopup::default();
        let now = Instant::now();
        assert!(p.on_spike_received(s("AAPL", 1, 3.0, 1.0), now));
        assert_eq!(p.active.len(), 1);

        p.dismiss("AAPL:1");
        assert_eq!(p.active.len(), 0);

        // Server republishes same id — must NOT re-show.
        let later = now + Duration::from_secs(1);
        let mutated = p.on_spike_received(s("AAPL", 1, 3.0, 1.0), later);
        assert!(!mutated, "dismissed id must not re-show");
        assert_eq!(p.active.len(), 0);
    }

    #[test]
    fn new_id_after_dismiss_shows() {
        let mut p = SpikeExplanationPopup::default();
        let now = Instant::now();
        p.on_spike_received(s("AAPL", 1, 3.0, 1.0), now);
        p.dismiss("AAPL:1");

        // Different t_ms → different id → must show.
        assert!(p.on_spike_received(s("AAPL", 2, 3.5, 1.2), now));
        assert_eq!(p.active.len(), 1);
        assert_eq!(p.active[0].spike.id, "AAPL:2");
    }

    #[test]
    fn coalesces_same_symbol_within_window() {
        let mut p = SpikeExplanationPopup::default();
        let t0 = Instant::now();
        p.on_spike_received(s("NVDA", 1, 3.0, 1.0), t0);
        // 30s later, NEW t_ms but same symbol → coalesce.
        let t1 = t0 + Duration::from_secs(30);
        p.on_spike_received(s("NVDA", 2, 4.2, 1.8), t1);
        assert_eq!(p.active.len(), 1, "must collapse into one toast");
        assert!((p.active[0].spike.sigma - 4.2).abs() < 1e-4);
        assert!((p.active[0].spike.pct_move - 1.8).abs() < 1e-4);
        // shown_at must NOT have been reset — auto-dismiss is based on the
        // original arrival, otherwise a rapid-fire stream would never expire.
        assert_eq!(p.active[0].shown_at, t0);
    }

    #[test]
    fn does_not_coalesce_outside_window() {
        let mut p = SpikeExplanationPopup::default();
        let t0 = Instant::now();
        p.on_spike_received(s("NVDA", 1, 3.0, 1.0), t0);
        let t_far = t0 + Duration::from_secs(6 * 60); // > 5 min
        p.on_spike_received(s("NVDA", 2, 4.2, 1.8), t_far);
        // Note: the first toast WILL have auto-expired by the time tick()
        // runs at t_far, but on_spike_received itself doesn't tick — we're
        // testing the coalesce logic in isolation here.
        // Without auto-tick we should see two toasts (no coalesce).
        assert_eq!(p.active.len(), 2);
    }

    #[test]
    fn auto_dismiss_after_30s() {
        let mut p = SpikeExplanationPopup::default();
        let t0 = Instant::now();
        p.on_spike_received(s("AAPL", 1, 3.0, 1.0), t0);
        // Just under 30s — still alive.
        let expired = p.tick(t0 + Duration::from_secs(29));
        assert_eq!(expired, 0);
        assert_eq!(p.active.len(), 1);

        // Past 30s — expired.
        let expired = p.tick(t0 + Duration::from_secs(31));
        assert_eq!(expired, 1);
        assert_eq!(p.active.len(), 0);
        // Expired toasts must be added to the dismissed set so server
        // re-publishes don't reincarnate them.
        assert!(p.dismissed.contains("AAPL:1"));
    }

    #[test]
    fn visible_capped_at_max_visible() {
        let mut p = SpikeExplanationPopup::default();
        let t0 = Instant::now();
        for i in 0..(MAX_VISIBLE + 2) {
            // Distinct symbols so coalescing doesn't fire.
            let sym = format!("S{i}");
            p.on_spike_received(s(&sym, i as i64, 3.0, 1.0), t0);
        }
        assert_eq!(p.active.len(), MAX_VISIBLE + 2);
        assert_eq!(p.visible().len(), MAX_VISIBLE,
            "visible() must clamp at MAX_VISIBLE");
    }

    #[test]
    fn duplicate_id_in_active_is_noop() {
        let mut p = SpikeExplanationPopup::default();
        let t0 = Instant::now();
        assert!(p.on_spike_received(s("X", 1, 3.0, 1.0), t0));
        let mutated = p.on_spike_received(s("X", 1, 99.0, 99.0), t0 + Duration::from_secs(1));
        assert!(!mutated, "same id, active toast: no-op");
        // Original sigma should be unchanged.
        assert!((p.active[0].spike.sigma - 3.0).abs() < 1e-4);
    }

    #[test]
    fn shorten_source_strips_scheme_and_path() {
        assert_eq!(shorten_source("https://www.reuters.com/article/x"), "reuters.com");
        assert_eq!(shorten_source("http://bloomberg.com/y"), "bloomberg.com");
        assert_eq!(shorten_source("https://nytimes.com"), "nytimes.com");
    }
}
