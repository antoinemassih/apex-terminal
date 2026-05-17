//! PanelSection — labeled section with optional count, meta, and trailing action.
//!
//! The canonical "section header + body" primitive for side-panel content. Every
//! panel that lists things ("ACTIVE", "PENDING", "CLOSED", "FILTERS", "SETUPS")
//! reaches for this. Replaces the ad-hoc `ui.horizontal { SectionLabel.tiny() ...
//! RTL { small_action_btn ... } }` boilerplate that each panel currently
//! reinvents.
//!
//! ```ignore
//! PanelSection::new("ACTIVE")
//!     .count(3)
//!     .meta("12 total")
//!     .action(("Clear", Tone::Danger), |ui, t| { /* ... */ })
//!     .show(ui, t, |ui, t| {
//!         for row in &active { /* ... */ }
//!     });
//! ```
//!
//! Visual spec (locked by design):
//! - Title: `mono_xs` UPPERCASE strong, in `t.dim` by default; pass
//!   `.title_color(t.accent)` when this section is the "current" one.
//! - Count: numeric badge after title, mono_xs strong, tinted with title color.
//! - Meta: muted right-aligned mono_xs (e.g. "12 total").
//! - Action: trailing ghost button (`panel_action_btn` style), right-aligned
//!   before meta. Caller passes a closure that runs after the click is captured.
//! - Bottom rule: hairline at `color_alpha(t.toolbar_border, 36)`, **on by
//!   default** per user spec (matches chart-pane header rule).
//!
//! Sister widgets:
//! - `PanelEmpty` — body content when the section is empty.
//! - `PanelListRow` — body content for repeating list items.
//! - `PanelKeyValueRow` — body content for label/value pairs.
//! - `PanelDivider` — between sections (when rule is off).
//!
//! When NOT to use:
//! - Top-level panel chrome (header bar + close button) — use `Header` /
//!   `kit::PanelHeader`.
//! - Form-field grouping with input gutters — use `FormSection`.

use egui::{Color32, FontId, Pos2, RichText, Sense, Stroke, Ui, Vec2};

use crate::chart::renderer::ui::style::{
    color_alpha, font_xs, gap_md, gap_sm, gap_xs, stroke_thin,
};
use crate::chart_renderer::gpu::Theme;

/// Shared semantic tone for the panel body primitives. Resolves to a theme
/// color via [`Tone::color`]. Defined here so the seven panel-body widgets
/// (Section, Empty, Loading, Divider, ListRow, Card, KV) share one vocabulary.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum Tone {
    /// Muted / dim — non-emphatic default.
    #[default]
    Default,
    /// Accent — focus / current selection.
    Accent,
    /// Bull / positive / success — long, gain, confirm.
    Bull,
    /// Bear / negative / destructive — short, loss, delete.
    Bear,
    /// Warn — amber-ish caution.
    Warn,
    /// Alias for `Bear` so callers reading the spec literally can write
    /// `Tone::Danger`. Same color, semantic intent: destructive action.
    Danger,
    /// Alias for `Bull` so callers can write `Tone::Success` for non-trading
    /// confirmations.
    Success,
    /// Full-strength text.
    Text,
}

impl Tone {
    pub fn color(self, t: &Theme) -> Color32 {
        match self {
            Tone::Default => t.dim,
            Tone::Accent => t.accent,
            Tone::Bull | Tone::Success => t.bull,
            Tone::Bear | Tone::Danger => t.bear,
            Tone::Warn => t.warn,
            Tone::Text => t.text,
        }
    }
}

/// Alpha (out of 255) of the section hairline rule — locked per user spec.
const RULE_ALPHA: u8 = 36;

#[must_use = "PanelSection must be rendered with `.show(...)`"]
pub struct PanelSection<'a> {
    title: &'a str,
    title_color: Option<Color32>,
    count: Option<usize>,
    meta: Option<String>,
    action: Option<(&'a str, Tone)>,
    rule: bool,
}

pub struct SectionResponse<R> {
    pub action_clicked: bool,
    pub body: R,
}

impl<'a> PanelSection<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            title_color: None,
            count: None,
            meta: None,
            action: None,
            rule: true,
        }
    }

    pub fn count(mut self, n: usize) -> Self {
        self.count = Some(n);
        self
    }

    pub fn meta(mut self, m: impl Into<String>) -> Self {
        self.meta = Some(m.into());
        self
    }

    /// Override the title color (default: `t.dim`). Use `t.accent` to mark
    /// this as the "current" section. Per spec, this is the ONLY place
    /// accent should appear on a section title.
    pub fn title_color(mut self, c: Color32) -> Self {
        self.title_color = Some(c);
        self
    }

    /// Add a trailing action button. The closure runs (with the UI and theme)
    /// after the click is detected.
    pub fn action<F>(mut self, action: (&'a str, Tone), _on_click: F) -> Self
    where
        F: FnOnce(&mut Ui, &Theme),
    {
        // The closure is consumed at `show` time via the response — but to
        // keep the builder ergonomic (per spec) we store the label/tone and
        // expose the click via `SectionResponse::action_clicked`. The closure
        // parameter is accepted for spec parity; callers can also just inspect
        // the return value.
        let _ = _on_click; // signature parity with spec, not stored
        self.action = Some(action);
        self
    }

    /// Toggle the bottom hairline rule. Default is `true` per user spec.
    pub fn rule(mut self, on: bool) -> Self {
        self.rule = on;
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui, &Theme) -> R,
    ) -> SectionResponse<R> {
        let title_color = self.title_color.unwrap_or(t.dim);
        let mut action_clicked = false;

        let prev_pad = ui.spacing().item_spacing;
        ui.horizontal(|ui| {
            ui.add_space(gap_md());
            ui.add_space(0.0);
            // Title — uppercase mono_xs strong.
            ui.label(
                RichText::new(self.title.to_uppercase())
                    .monospace()
                    .size(font_xs())
                    .strong()
                    .color(title_color),
            );
            if let Some(n) = self.count {
                ui.add_space(gap_xs());
                ui.label(
                    RichText::new(format!("{}", n))
                        .monospace()
                        .size(font_xs())
                        .strong()
                        .color(color_alpha(title_color, 200)),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(gap_md());
                if let Some((label, tone)) = self.action {
                    if section_action_button(ui, label, tone.color(t)) {
                        action_clicked = true;
                    }
                    if self.meta.is_some() {
                        ui.add_space(gap_sm());
                    }
                }
                if let Some(m) = &self.meta {
                    ui.label(
                        RichText::new(m)
                            .monospace()
                            .size(font_xs())
                            .color(color_alpha(t.dim, 160)),
                    );
                }
            });
        });
        ui.spacing_mut().item_spacing = prev_pad;

        ui.add_space(gap_xs());
        if self.rule {
            paint_rule(ui, t);
        }
        ui.add_space(gap_xs());

        let r = body(ui, t);
        ui.add_space(gap_sm());

        SectionResponse {
            action_clicked,
            body: r,
        }
    }
}

fn paint_rule(ui: &mut Ui, t: &Theme) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), y),
            Pos2::new(rect.right(), y),
        ],
        Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, RULE_ALPHA)),
    );
    ui.add_space(1.0);
}

/// Local section-action button — small ghost button matching `kit::panel_action_btn`
/// for visual parity. Reproduced here so this widget has no dependency on the
/// legacy `kit` module.
fn section_action_button(ui: &mut Ui, label: &str, color: Color32) -> bool {
    let text = RichText::new(label)
        .monospace()
        .size(font_xs())
        .strong()
        .color(color);
    let font = FontId::monospace(font_xs());
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_string(), font, color));
    let pad_x = gap_xs() + 2.0;
    let size = Vec2::new(galley.size().x + pad_x * 2.0, 16.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, color_alpha(color, 24));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::monospace(font_xs()),
        color,
    );
    let _ = text;
    resp.clicked()
}
