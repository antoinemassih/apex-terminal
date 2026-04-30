//! Section labels and text-role helpers — pane titles, subheaders, body text,
//! monospace code, numeric displays.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Ui};

// ─── Labels ───────────────────────────────────────────────────────────────────

/// Section label — small, dim, monospace, uppercased under Meridien.
/// Use above grouped controls or table sections.
pub fn section_label_widget(ui: &mut Ui, text: &str, color: Color32) -> Response {
    section_label_sized(ui, text, color, font_sm())
}

/// Sized variant — same treatment but with caller-chosen font size.
pub fn section_label_sized(ui: &mut Ui, text: &str, color: Color32, size: f32) -> Response {
    let s = style_label_case(text);
    ui.label(
        RichText::new(s)
            .monospace()
            .size(size)
            .strong()
            .color(color),
    )
}

#[inline] pub fn section_label_xs(ui: &mut Ui, text: &str, color: Color32) -> Response { section_label_sized(ui, text, color, font_xs()) }
#[inline] pub fn section_label_md(ui: &mut Ui, text: &str, color: Color32) -> Response { section_label_sized(ui, text, color, font_md()) }
#[inline] pub fn section_label_lg(ui: &mut Ui, text: &str, color: Color32) -> Response { section_label_sized(ui, text, color, font_lg()) }

// ─── Text role helpers ────────────────────────────────────────────────────────

/// Size variants for [`monospace_code`].
pub enum MonoSize {
    /// `font_xs()` — column headers, supplemental info.
    Xs,
    /// `font_sm()` — default mono text.
    Sm,
    /// `font_md()` — emphasized mono.
    Md,
}

/// Size variants for [`numeric_display`].
pub enum NumericSize {
    /// `font_lg()` — compact price / change readout.
    Lg,
    /// `font_xl()` — secondary headline.
    Xl,
    /// 30 px — hero display (portfolio total, primary price).
    Hero,
}

/// Pane heading — large title at the top of a side pane ("Watchlist", "Orders").
/// Renders `font_lg()` strong monospace.
pub fn pane_title(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.label(RichText::new(text).monospace().size(font_lg()).strong().color(color))
}

/// Sub-section heading — "Greeks", "P&L", group names below a section header.
/// Renders `font_xs()` strong monospace.
pub fn subheader(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.label(RichText::new(text).monospace().size(font_xs()).strong().color(color))
}

/// Default UI body label. Renders `font_sm()` regular monospace.
pub fn body_label(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.label(RichText::new(text).monospace().size(font_sm()).color(color))
}

/// Secondary / dim text. Applies `alpha_muted()` to `base_color`.
pub fn muted_label(ui: &mut Ui, text: &str, base_color: Color32) -> Response {
    let c = color_alpha(base_color, alpha_muted());
    ui.label(RichText::new(text).monospace().size(font_sm()).color(c))
}

/// Monospace text for tickers / prices / code at a chosen size.
pub fn monospace_code(ui: &mut Ui, text: &str, size: MonoSize, color: Color32) -> Response {
    let sz = match size {
        MonoSize::Xs => font_xs(),
        MonoSize::Sm => font_sm(),
        MonoSize::Md => font_md(),
    };
    ui.label(RichText::new(text).monospace().size(sz).color(color))
}

/// Large numeric readout (price, P&L, account values).
/// `color` should already encode bull/bear/dim semantics at the call site.
pub fn numeric_display(ui: &mut Ui, text: &str, size: NumericSize, color: Color32) -> Response {
    let sz = match size {
        NumericSize::Lg   => font_lg(),
        NumericSize::Xl   => font_xl(),
        NumericSize::Hero => 30.0,
    };
    ui.label(RichText::new(text).monospace().size(sz).strong().color(color))
}
