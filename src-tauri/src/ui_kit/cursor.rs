//! Cursor helpers — portable hover/drag/resize cursor management.
//!
//! Mirrors the chart-app's `chart_renderer::ui::style::cursor` module.
//! Used by every widget that needs to set the OS cursor on hover, drag,
//! or focus. Honors design-mode inspect: when the inspector is on, none
//! of these helpers set a cursor so the design tool retains control.

use egui::{CursorIcon, Response, Ui};

#[inline]
fn inspect_mode() -> bool {
    crate::design_tokens::is_inspect_mode()
}

/// PointingHand on hover — for any clickable surface.
#[inline]
pub fn clickable(ui: &Ui, resp: &Response) {
    if resp.hovered() && !inspect_mode() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
}

/// Grab on hover, Grabbing while actively dragging.
#[inline]
pub fn draggable(ui: &Ui, resp: &Response) {
    if inspect_mode() { return; }
    if resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
}

/// Horizontal resize cursor — vertical dividers between panes/columns.
#[inline]
pub fn resize_h(ui: &Ui, resp: &Response) {
    if inspect_mode() { return; }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
}

/// Vertical resize cursor — horizontal dividers between rows/panes.
#[inline]
pub fn resize_v(ui: &Ui, resp: &Response) {
    if inspect_mode() { return; }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
    }
}

/// Diagonal NW–SE resize.
#[inline]
pub fn resize_nwse(ui: &Ui, resp: &Response) {
    if resp.hovered() && !inspect_mode() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
    }
}

/// Text I-beam.
#[inline]
pub fn text_input(ui: &Ui, resp: &Response) {
    if resp.hovered() && !inspect_mode() {
        ui.ctx().set_cursor_icon(CursorIcon::Text);
    }
}

/// Crosshair.
#[inline]
pub fn crosshair(ui: &Ui, resp: &Response) {
    if resp.hovered() && !inspect_mode() {
        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
    }
}

/// ZoomIn.
#[inline]
pub fn zoom_in(ui: &Ui, resp: &Response) {
    if resp.hovered() && !inspect_mode() {
        ui.ctx().set_cursor_icon(CursorIcon::ZoomIn);
    }
}

/// Modal cursor: set whenever a tool mode is active.
#[inline]
pub fn modal(ui: &Ui, icon: CursorIcon) {
    if !inspect_mode() {
        ui.ctx().set_cursor_icon(icon);
    }
}

/// `ui.add(widget)` + `clickable(...)` in one call.
#[inline]
pub fn click_widget<W: egui::Widget>(ui: &mut Ui, widget: W) -> Response {
    let r = ui.add(widget);
    clickable(ui, &r);
    r
}

/// Paint a keyboard-focus ring around `resp.rect`. Takes explicit
/// `focus_ring_alpha` / `focus_ring_width` so this is portable —
/// callers thread the values from their style (chart-app passes
/// `current().focus_ring_alpha` etc; portable callers pass constants).
#[inline]
pub fn focus_ring(
    ui: &Ui,
    resp: &Response,
    accent: egui::Color32,
    focus_ring_alpha: u8,
    focus_ring_width: f32,
) {
    use egui::{CornerRadius, Stroke, StrokeKind};
    if !resp.has_focus() {
        return;
    }
    let color = crate::ui_kit::style::color_alpha(accent, focus_ring_alpha);
    let radius = CornerRadius::same((crate::ui_kit::style::radius_sm() as u8).saturating_add(1));
    ui.painter().rect_stroke(
        resp.rect.expand(2.0),
        radius,
        Stroke::new(focus_ring_width, color),
        StrokeKind::Outside,
    );
}
