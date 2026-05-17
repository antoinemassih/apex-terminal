//! PanelFooter — the canonical sticky bottom action bar for side panels.
//!
//! ## What it does
//!
//! Standardizes the bottom-action pattern every panel that has primary
//! actions (Place All, Cancel, Save, …) currently hand-paints. Provides a
//! thin top hairline divider, `gap_md` inner padding, left-justified meta
//! text in muted `mono_xs`, and right-justified primary/secondary action
//! buttons.
//!
//! ## Usage
//!
//! ```ignore
//! PanelFooter::new()
//!     .meta("3 selected")
//!     .secondary("Cancel", |ui, t| Button::simple("Cancel").show(ui, t))
//!     .primary("Place All", Tone::Accent,
//!         |ui, t| Button::cta("Place All").show(ui, t))
//!     .show(ui, t);
//! ```
//!
//! ## When to use
//!
//! - Side panels that expose 1–2 primary actions tied to the panel's
//!   selection (orders, alerts, watchlist multi-select bulk actions).
//!
//! ## When NOT to use
//!
//! - Per-row trailing actions — those live inside the row itself.
//! - Single-action floating "+" buttons — those go in
//!   [`SidePanelShell::header_actions`](super::side_panel_shell::SidePanelShell).
//! - Modal action rows — use `Modal`'s own footer slot.
//!
//! ## Sister widgets
//!
//! - [`SidePanelShell`](super::side_panel_shell::SidePanelShell) — outer
//!   shell that owns the footer slot.
//! - `kit::PanelDualAction` — two opposing tone buttons inline (Buy/Sell);
//!   different intent, kept separate.

#![allow(dead_code)]

use egui::{Pos2, Sense, Stroke, Ui, Vec2};

use crate::chart::renderer::ui::style::{
    color_alpha, gap_md, mono_xs, stroke_thin,
};
use crate::chart_renderer::gpu::Theme;

/// Semantic tone for the primary slot. Kept tiny — the underlying button
/// builder owns the actual palette mapping; this enum is here so the call
/// site reads intent-first.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Tone {
    /// Accent / primary CTA.
    #[default]
    Accent,
    /// Bull / positive / confirm.
    Success,
    /// Bear / destructive.
    Danger,
    /// Neutral / muted.
    Neutral,
}

type ButtonFn<'a> = Box<dyn FnOnce(&mut Ui, &Theme) + 'a>;

/// Sticky bottom action band. See module docs.
#[must_use = "PanelFooter must be rendered with `.show(...)`"]
pub struct PanelFooter<'a> {
    primary: Option<(&'a str, Tone, ButtonFn<'a>)>,
    secondary: Option<(&'a str, ButtonFn<'a>)>,
    meta: Option<String>,
}

impl<'a> Default for PanelFooter<'a> {
    fn default() -> Self { Self::new() }
}

impl<'a> PanelFooter<'a> {
    pub fn new() -> Self {
        Self { primary: None, secondary: None, meta: None }
    }

    /// Primary (rightmost) action. `label` is informational — the actual
    /// label text is whatever the closure paints.
    pub fn primary(
        mut self,
        label: &'a str,
        tone: Tone,
        f: impl FnOnce(&mut Ui, &Theme) + 'a,
    ) -> Self {
        self.primary = Some((label, tone, Box::new(f)));
        self
    }

    /// Secondary action, painted to the LEFT of the primary action.
    pub fn secondary(
        mut self,
        label: &'a str,
        f: impl FnOnce(&mut Ui, &Theme) + 'a,
    ) -> Self {
        self.secondary = Some((label, Box::new(f)));
        self
    }

    /// Left-aligned status text rendered in muted `mono_xs`.
    pub fn meta(mut self, m: impl Into<String>) -> Self {
        self.meta = Some(m.into());
        self
    }

    /// Render the footer inline at the current cursor position.
    pub fn show(self, ui: &mut Ui, t: &Theme) {
        // Hairline top border — matches the section-divider treatment used
        // throughout the panel kit.
        let avail = ui.available_width();
        let (rule_rect, _) = ui.allocate_exact_size(Vec2::new(avail, 1.0), Sense::hover());
        ui.painter().line_segment(
            [Pos2::new(rule_rect.left(), rule_rect.center().y),
             Pos2::new(rule_rect.right(), rule_rect.center().y)],
            Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, 36)),
        );

        let frame = egui::Frame::NONE.inner_margin(egui::Margin {
            left:   gap_md() as i8,
            right:  gap_md() as i8,
            top:    gap_md() as i8,
            bottom: gap_md() as i8,
        });

        let PanelFooter { primary, secondary, meta } = self;
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(m) = meta {
                    ui.label(
                        egui::RichText::new(m)
                            .font(mono_xs())
                            .color(color_alpha(t.dim, 160)),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((_, _tone, f)) = primary { f(ui, t); }
                    if let Some((_, f)) = secondary { f(ui, t); }
                });
            });
        });

        // `Tone` is informational at the moment — the closure owns the
        // actual button palette. Marker kept in the API so a future
        // tone-aware refactor doesn't break call sites.
        let _ = Tone::default();
    }
}
