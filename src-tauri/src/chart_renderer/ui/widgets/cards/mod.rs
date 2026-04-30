//! Builder primitives — cards family.
//!
//! Card surfaces extracted by visual reference from playbook_panel,
//! plays_panel, news_panel, seasonality_panel, rrg_panel, analysis_panel,
//! feed_panel and the read-only chart_widgets. **No call sites are migrated
//! here (Wave 5).** Chart paint code is sacred and untouched.
//!
//! Pattern: `Card::new().bordered().title("X").subtitle("Y").show(ui, |ui| {...})`
//!
//! Specialized cards (`PlaybookCard`, `SignalCard`, ...) layer presets on top
//! of the base `Card` and add domain-specific fields (tags, sparklines,
//! deltas, etc.).

#![allow(dead_code, unused_imports)]

pub mod earnings_card;
pub mod event_card;
pub mod metric_card;
pub mod news_card;
pub mod play_card;
pub mod playbook_card;
pub mod signal_card;
pub mod stat_card;
pub mod trade_card;

pub use earnings_card::EarningsCard;
pub use event_card::EventCard;
pub use metric_card::MetricCard;
pub use news_card::NewsCard;
pub use play_card::{PlayCard, PlayCardResponse};
pub use playbook_card::PlaybookCard;
pub use signal_card::SignalCard;
pub use stat_card::StatCard;
pub use trade_card::TradeCard;

use egui::{Color32, RichText, Stroke, Ui};
use super::super::style::*;
use super::frames::CardFrame;

type Theme = crate::chart_renderer::gpu::Theme;

/// Visual variant of a `Card`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CardVariant {
    /// Default — subtle border, panel bg.
    Bordered,
    /// Stronger fill + shadow.
    Elevated,
    /// No border, no fill — pure layout container.
    Ghost,
}

impl Default for CardVariant {
    fn default() -> Self { CardVariant::Bordered }
}

/// Base card builder. Use a body closure for content; pass an optional footer
/// closure separately. Specialized cards live in sibling modules.
///
/// ```ignore
/// Card::new().bordered().title("Plan").subtitle("ES")
///     .theme(t)
///     .show(ui, |ui| { ui.label("body"); });
/// ```
#[must_use = "Card must be rendered with `.show(ui, |ui| {...})`"]
pub struct Card<'a> {
    title:    Option<&'a str>,
    subtitle: Option<&'a str>,
    variant:  CardVariant,
    bg:       Color32,
    border:   Color32,
    fg:       Color32,
    dim:      Color32,
}

impl<'a> Card<'a> {
    pub fn new() -> Self {
        Self {
            title: None,
            subtitle: None,
            variant: CardVariant::Bordered,
            bg:     Color32::TRANSPARENT,
            border: Color32::TRANSPARENT,
            fg:     Color32::from_rgb(210, 210, 220),
            dim:    Color32::from_rgb(140, 140, 150),
        }
    }

    pub fn title(mut self, t: &'a str)    -> Self { self.title = Some(t); self }
    pub fn subtitle(mut self, s: &'a str) -> Self { self.subtitle = Some(s); self }

    pub fn bordered(mut self) -> Self { self.variant = CardVariant::Bordered; self }
    pub fn elevated(mut self) -> Self { self.variant = CardVariant::Elevated; self }
    pub fn ghost(mut self)    -> Self { self.variant = CardVariant::Ghost;    self }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.bg     = t.toolbar_bg;
        self.border = t.toolbar_border;
        self.fg     = t.text;
        self.dim    = t.dim;
        self
    }

    pub fn colors(mut self, bg: Color32, border: Color32) -> Self {
        self.bg = bg; self.border = border; self
    }

    /// Render header (if any) then body.
    pub fn show<R>(self, ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        let frame = match self.variant {
            CardVariant::Bordered => CardFrame::new().colors(self.bg, self.border).build(),
            CardVariant::Elevated => CardFrame::new().colors(self.bg, self.border).build(),
            CardVariant::Ghost    => egui::Frame::NONE
                .inner_margin(egui::Margin::same(gap_md() as i8)),
        };
        let mut out = None;
        frame.show(ui, |ui| {
            if let Some(t) = self.title {
                ui.label(RichText::new(t).monospace().size(font_md()).strong().color(self.fg));
            }
            if let Some(s) = self.subtitle {
                ui.label(RichText::new(s).monospace().size(font_sm()).color(self.dim));
            }
            if self.title.is_some() || self.subtitle.is_some() {
                ui.add_space(gap_xs());
            }
            out = Some(body(ui));
        });
        out
    }

    /// Render with both body and footer closures.
    pub fn show_with_footer(
        self,
        ui: &mut Ui,
        body:   impl FnOnce(&mut Ui),
        footer: impl FnOnce(&mut Ui),
    ) {
        self.show(ui, |ui| {
            body(ui);
            ui.add_space(gap_xs());
            ui.separator();
            ui.add_space(gap_xs());
            footer(ui);
        });
    }
}

impl<'a> Default for Card<'a> {
    fn default() -> Self { Self::new() }
}
