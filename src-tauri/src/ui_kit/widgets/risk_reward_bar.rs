//! RiskRewardBar — visualizes a risk/reward ratio as a 2-segment horizontal bar.
//!
//! Renders a background track, a risk segment (bear color) on the left and a
//! reward segment (bull color) on the right, proportional to the supplied
//! values. A small divider circle marks the split point.
//!
//! ```ignore
//! RiskRewardBar::new(risk, reward).width(120.0).show(ui, theme);
//! ```

use egui::{Response, Sense, Ui, Vec2};
use super::theme::ComponentTheme;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::text_style::TextStyle;

/// Horizontal 2-segment risk/reward bar.
#[must_use = "RiskRewardBar does nothing until `.show(ui, theme)` is called"]
pub struct RiskRewardBar {
    risk: f32,
    reward: f32,
    width: f32,
    height: f32,
    show_label: bool,
}

impl RiskRewardBar {
    pub fn new(risk: f32, reward: f32) -> Self {
        Self {
            risk,
            reward,
            width: 200.0,
            height: 6.0,
            show_label: false,
        }
    }

    /// Override the total bar width (default: 200.0). Callers typically pass
    /// `ui.available_width().min(120.0)` here.
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// Override the bar height (default: 6.0).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    /// When `true`, renders an `R/R: X.XX` label overlaid on the bar.
    pub fn show_label(mut self, b: bool) -> Self {
        self.show_label = b;
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
        let (bar_rect, resp) = ui.allocate_exact_size(
            Vec2::new(self.width, self.height),
            Sense::hover(),
        );
        let p = ui.painter();
        let pal = palette_ct(theme);

        // Background track
        let border_col = st::color_alpha(
            egui::Color32::from_gray(128),
            st::alpha_muted(),
        );
        p.rect_filled(bar_rect, st::radius_xs(), border_col);

        // Risk / reward split
        let total = (self.risk + self.reward).max(f32::EPSILON);
        let risk_pct = (self.risk / total).min(1.0);

        // Risk segment (bear)
        p.rect_filled(
            egui::Rect::from_min_size(
                bar_rect.min,
                egui::vec2(self.width * risk_pct, self.height),
            ),
            st::radius_xs(),
            st::color_alpha(pal.base(Tone::Bear), st::alpha_dim()),
        );

        // Reward segment (bull)
        p.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(bar_rect.left() + self.width * risk_pct, bar_rect.top()),
                egui::vec2(self.width * (1.0 - risk_pct), self.height),
            ),
            st::radius_xs(),
            st::color_alpha(pal.base(Tone::Bull), st::alpha_dim()),
        );

        // Divider dot
        p.circle_filled(
            egui::pos2(bar_rect.left() + self.width * risk_pct, bar_rect.center().y),
            3.0,
            pal.base(Tone::Text),
        );

        if self.show_label {
            let rr = self.reward / self.risk.max(f32::EPSILON);
            p.text(
                bar_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("R/R: {:.2}", rr),
                TextStyle::MonoXs.font_id_in(ui),
                pal.base(Tone::Text),
            );
        }

        resp
    }
}

impl<'a> egui::Widget for RiskRewardBar {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
