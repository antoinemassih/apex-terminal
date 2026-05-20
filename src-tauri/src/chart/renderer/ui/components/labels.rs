//! Section labels and text-role helpers — pane titles, subheaders, body text,
//! monospace code, numeric displays.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Ui};

// ─── Labels ───────────────────────────────────────────────────────────────────

/// Section label — small, dim, monospace, uppercased under Meridien.
/// Use above grouped controls or table sections.
pub fn section_label_widget(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let s = style_label_case(text);
    ui.label(
        RichText::new(s)
            .monospace()
            .size(font_sm())
            .strong()
            .color(color),
    )
}

#[inline] pub fn section_label_xs(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let s = style_label_case(text);
    ui.label(RichText::new(s).monospace().size(font_xs()).strong().color(color))
}
#[inline] pub fn section_label_md(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let s = style_label_case(text);
    ui.label(RichText::new(s).monospace().size(font_md()).strong().color(color))
}
#[inline] pub fn section_label_lg(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let s = style_label_case(text);
    ui.label(RichText::new(s).monospace().size(font_lg()).strong().color(color))
}

// Re-export size enums from text.rs (canonical home).
pub use super::text::{MonoSize, NumericSize};

/// Pane heading — large title at the top of a side pane ("Watchlist", "Orders").
/// Renders `font_lg()` strong monospace.
pub fn pane_title(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.label(RichText::new(text).monospace().size(font_lg()).strong().color(color))
}
