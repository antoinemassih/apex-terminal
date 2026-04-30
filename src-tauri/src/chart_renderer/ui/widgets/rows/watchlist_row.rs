//! WatchlistRow — symbol + price + change% [+ optional sparkline].
//!
//! Migrated to `RowShell` (painter mode). The shell owns the hit-rect, base
//! fill, and hover/selected/focus overlay; this body paints the row's
//! domain-specific content (symbol, prices, sparkline middle slot).

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, Stroke, Ui, Widget};
use super::super::super::style::*;
use super::super::foundation::{
    interaction::InteractionState,
    shell::RowShell,
    tokens::Size,
    variants::RowVariant,
};
use super::ListRow;

type Theme = crate::chart_renderer::gpu::Theme;

/// Fallback theme for theme-less callers — first registered project theme.
fn fallback_theme() -> &'static Theme {
    &crate::chart_renderer::gpu::THEMES[0]
}

#[must_use = "WatchlistRow must be finalized with `.show(ui)` to render"]
pub struct WatchlistRow<'a> {
    symbol: &'a str,
    price: f32,
    change_pct: f32,
    spark: Option<&'a [f32]>,
    selected: bool,
    height: f32,
    theme: Option<&'a Theme>,
    theme_bg: Option<Color32>,
    theme_border: Option<Color32>,
    theme_accent: Option<Color32>,
    theme_bull: Option<Color32>,
    theme_bear: Option<Color32>,
    theme_dim: Option<Color32>,
    theme_fg: Option<Color32>,
}

impl<'a> WatchlistRow<'a> {
    pub fn new(symbol: &'a str, price: f32, change_pct: f32) -> Self {
        Self {
            symbol, price, change_pct,
            spark: None, selected: false, height: 22.0,
            theme: None,
            theme_bg: None, theme_border: None, theme_accent: None,
            theme_bull: None, theme_bear: None, theme_dim: None, theme_fg: None,
        }
    }
    pub fn spark(mut self, s: &'a [f32]) -> Self { self.spark = Some(s); self }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.theme_bg = Some(t.toolbar_bg);
        self.theme_border = Some(t.toolbar_border);
        self.theme_accent = Some(t.accent);
        self.theme_bull = Some(t.bull);
        self.theme_bear = Some(t.bear);
        self.theme_dim = Some(t.dim);
        self.theme_fg = Some(t.text);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme_ref: &Theme = match self.theme { Some(t) => t, None => fallback_theme() };
        let bull = self.theme_bull.unwrap_or(Color32::from_rgb(0, 200, 120));
        let bear = self.theme_bear.unwrap_or(Color32::from_rgb(220, 80, 80));
        let dim = self.theme_dim.unwrap_or(Color32::from_gray(120));
        let fg = self.theme_fg.unwrap_or(Color32::from_gray(220));
        let symbol = self.symbol;
        let price = self.price;
        let change_pct = self.change_pct;
        let spark = self.spark;

        let resp = RowShell::new(theme_ref, "")
            .variant(RowVariant::Default)
            .size(Size::Md)
            .state(InteractionState::default().selected(self.selected))
            .painter_mode(true)
            .painter_height(self.height)
            .painter_body(move |ui, rect| {
                let painter = ui.painter();
                let cy = rect.center().y;
                painter.text(
                    egui::pos2(rect.left() + 8.0, cy), egui::Align2::LEFT_CENTER,
                    symbol, egui::FontId::monospace(10.0), fg,
                );
                let chg_col = if change_pct >= 0.0 { bull } else { bear };
                let chg_str = format!("{:+.2}%", change_pct);
                painter.text(
                    egui::pos2(rect.right() - 8.0, cy), egui::Align2::RIGHT_CENTER,
                    &chg_str, egui::FontId::monospace(9.5), chg_col,
                );
                let price_str = format!("{:.2}", price);
                painter.text(
                    egui::pos2(rect.right() - 60.0, cy), egui::Align2::RIGHT_CENTER,
                    &price_str, egui::FontId::monospace(10.0), fg,
                );
                if let Some(s) = spark {
                    if s.len() >= 2 {
                        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                        for &v in s { if v < lo { lo = v; } if v > hi { hi = v; } }
                        let span = (hi - lo).max(1e-6);
                        let sx0 = rect.left() + 60.0;
                        let sx1 = rect.right() - 110.0;
                        if sx1 > sx0 + 6.0 {
                            let sw = sx1 - sx0;
                            let sy0 = rect.top() + 4.0;
                            let sy1 = rect.bottom() - 4.0;
                            let sh = sy1 - sy0;
                            let n = s.len();
                            let mut prev: Option<egui::Pos2> = None;
                            for (i, &v) in s.iter().enumerate() {
                                let x = sx0 + sw * (i as f32) / ((n - 1) as f32);
                                let y = sy1 - sh * (v - lo) / span;
                                let p = egui::pos2(x, y);
                                if let Some(pp) = prev {
                                    painter.line_segment([pp, p],
                                        Stroke::new(stroke_thin(),
                                            color_alpha(dim, alpha_dim())));
                                }
                                prev = Some(p);
                            }
                        }
                    }
                }
            })
            .show(ui);

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "WATCHLIST_ROW", "Rows",
        );
        resp
    }
}
