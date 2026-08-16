//! Custom window-control buttons (Close / Maximize / Minimize) — extracted
//! from `top_nav.rs`.
//!
//! Painter-drawn rather than OS-native so they match the app chrome. Rendered
//! inside the top-nav's right-to-left fixed cluster, before the panel toggles.

use std::sync::Arc;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use winit::window::Window;

use crate::chart_renderer::gpu::{
    Chart, Layout, Watchlist, Theme,
    CLOSE_REQUESTED, TB_BTN_CLICKED, save_state,
};
use crate::chart_renderer::ui::style::{
    color_alpha, color_subtle, contrast_fg, stroke_std, BTN_ICON_LG,
};

/// Render the three custom window controls (Close, Maximize/Restore, Minimize).
///
/// Must be called inside a right-to-left layout (they paint right→left:
/// Close is drawn first and ends up rightmost).
pub(crate) fn render_window_controls(
    ui: &mut egui::Ui,
    panes: &[Chart],
    layout: Layout,
    watchlist: &mut Watchlist,
    win_ref: &Option<Arc<Window>>,
    t: &Theme,
) {
    // Shared button shell: hover fill (bear for danger/close), click flag.
    // Sense::click_AND_drag (not click) is deliberate: the full-toolbar window-drag
    // region underneath also senses drag, so with click-only egui routes any
    // micro-movement during a press to that drag region (firing a window drag /
    // un-maximize) instead of the button click — which broke the maximize button.
    // Sensing drag here makes egui assign the press to the button; we just ignore
    // the drag, so a clean click always toggles and a stray wiggle is a no-op.
    let win_btn = |ui: &mut egui::Ui, danger: bool| -> (egui::Response, egui::Rect) {
        let (r, resp) = ui.allocate_exact_size(BTN_ICON_LG, egui::Sense::click_and_drag());
        if resp.hovered() {
            let bg = if danger { t.bear } else { tint(t, Tone::Border, crate::ui_kit::style::alpha_strong()) };
            ui.painter().rect_filled(r, 0.0, bg);
        }
        crate::chart_renderer::ui::style::cursor::clickable(ui, &resp);
        if resp.clicked() { TB_BTN_CLICKED.with(|f| f.set(true)); }
        (resp, r)
    };

    // Close — draw X with lines
    {
        let (resp, r) = win_btn(ui, true);
        let c = r.center();
        let s = 4.5;
        let col = if resp.hovered() { contrast_fg(t.bear) } else { color_subtle(t.dim) };
        ui.painter().line_segment([egui::pos2(c.x - s, c.y - s), egui::pos2(c.x + s, c.y + s)], egui::Stroke::new(stroke_std(), col));
        ui.painter().line_segment([egui::pos2(c.x + s, c.y - s), egui::pos2(c.x - s, c.y + s)], egui::Stroke::new(stroke_std(), col));
        if resp.clicked() {
            save_state(panes, layout, watchlist);
            watchlist.persist();
            CLOSE_REQUESTED.with(|f| f.set(true));
        }
    }
    // Maximize — draw square outline (or overlapping squares when maximized)
    {
        let (resp, r) = win_btn(ui, false);
        let c = r.center();
        let s = 4.5;
        let col = if resp.hovered() { t.dim } else { color_subtle(t.dim) };
        let is_max = win_ref.as_ref().map_or(false, |w| w.is_maximized());
        if is_max {
            // Restore icon: two overlapping squares
            let o = 1.5;
            ui.painter().rect_stroke(egui::Rect::from_min_size(egui::pos2(c.x - s + o, c.y - s), egui::vec2(s * 2.0 - o, s * 2.0 - o)), 0.5, egui::Stroke::new(stroke_std(), col), egui::StrokeKind::Outside);
            ui.painter().rect_stroke(egui::Rect::from_min_size(egui::pos2(c.x - s, c.y - s + o), egui::vec2(s * 2.0 - o, s * 2.0 - o)), 0.5, egui::Stroke::new(stroke_std(), col), egui::StrokeKind::Outside);
        } else {
            ui.painter().rect_stroke(egui::Rect::from_center_size(c, egui::vec2(s * 2.0, s * 2.0)), 0.5, egui::Stroke::new(stroke_std(), col), egui::StrokeKind::Outside);
        }
        if resp.clicked() {
            if let Some(w) = win_ref { let m = w.is_maximized(); w.set_maximized(!m); }
        }
    }
    // Minimize — draw horizontal line
    {
        let (resp, r) = win_btn(ui, false);
        let c = r.center();
        let s = 5.0;
        let col = if resp.hovered() { t.dim } else { color_subtle(t.dim) };
        ui.painter().line_segment([egui::pos2(c.x - s, c.y), egui::pos2(c.x + s, c.y)], egui::Stroke::new(stroke_std(), col));
        if resp.clicked() {
            if let Some(w) = win_ref { w.set_minimized(true); }
        }
    }
}
