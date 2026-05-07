//! NewsRow — timestamp + headline + source badge + symbol + tag chips.
//! Migrated to `RowShell` (painter mode) so the meta-strip's tag-chip
//! wrapping retains exact pixel control while sharing shell-level
//! hover/selected/focus paint.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, Ui};
use super::super::super::style::*;
use crate::chart::renderer::ui::foundation::{
    interaction::InteractionState,
    shell::RowShell,
    tokens::Size,
    variants::RowVariant,
};

type Theme = crate::chart_renderer::gpu::Theme;

fn fallback_theme() -> &'static Theme {
    &crate::chart_renderer::gpu::THEMES[0]
}


#[must_use = "NewsRow must be finalized with `.show(ui)` to render"]
pub struct NewsRow<'a> {
    headline: &'a str,
    timestamp: &'a str,
    source: &'a str,
    symbol: &'a str,
    sentiment: i8,
    tags: &'a [&'a str],
    selected: bool,
    height: f32,
    theme: Option<&'a Theme>,
    theme_bull: Option<Color32>,
    theme_bear: Option<Color32>,
    theme_dim: Option<Color32>,
    theme_accent: Option<Color32>,
    theme_border: Option<Color32>,
}

impl<'a> NewsRow<'a> {
    pub fn new(headline: &'a str, timestamp: &'a str, source: &'a str, symbol: &'a str) -> Self {
        Self {
            headline, timestamp, source, symbol,
            sentiment: 0, tags: &[], selected: false, height: 52.0,
            theme: None,
            theme_bull: None, theme_bear: None, theme_dim: None,
            theme_accent: None, theme_border: None,
        }
    }
    pub fn sentiment(mut self, s: i8) -> Self { self.sentiment = s; self }
    pub fn tags(mut self, t: &'a [&'a str]) -> Self { self.tags = t; self }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.theme_bull = Some(t.bull);
        self.theme_bear = Some(t.bear);
        self.theme_dim = Some(t.dim);
        self.theme_accent = Some(t.accent);
        self.theme_border = Some(t.toolbar_border);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme_ref: &Theme = match self.theme { Some(t) => t, None => fallback_theme() };
        let ft = fallback_theme();
        let bull = self.theme_bull.unwrap_or(ft.bull);
        let bear = self.theme_bear.unwrap_or(ft.bear);
        let dim = self.theme_dim.unwrap_or(ft.dim);
        let accent = self.theme_accent.unwrap_or(ft.accent);
        let headline_fg = theme_ref.text;

        let headline = self.headline;
        let timestamp = self.timestamp;
        let source = self.source;
        let symbol = self.symbol;
        let sentiment = self.sentiment;
        let tags = self.tags;

        let resp = RowShell::new(theme_ref, "")
            .variant(RowVariant::Default)
            .size(Size::Md)
            .state(InteractionState::default().selected(self.selected))
            .painter_mode(true)
            .painter_height(self.height)
            .painter_body(move |ui, rect| {
                let m = 8.0;
                let painter = ui.painter();

                let headline_pos = egui::pos2(rect.min.x + m, rect.min.y + 4.0);
                painter.text(headline_pos, egui::Align2::LEFT_TOP,
                    headline, egui::FontId::monospace(11.0),
                    headline_fg);

                let meta_y = rect.min.y + 30.0;

                let source_col = match source {
                    "Reuters" => Color32::from_rgb(255, 140, 0),
                    "Bloomberg" => Color32::from_rgb(100, 180, 255),
                    "CNBC" => Color32::from_rgb(0, 180, 120),
                    "Benzinga" => Color32::from_rgb(180, 100, 255),
                    _ => dim,
                };
                let source_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x + m, meta_y), egui::vec2(50.0, 14.0));
                painter.rect_filled(source_rect, 2.0, color_alpha(source_col, alpha_subtle()));
                painter.text(source_rect.center(), egui::Align2::CENTER_CENTER,
                    source, egui::FontId::monospace(11.0), source_col);

                painter.text(egui::pos2(rect.min.x + m + 55.0, meta_y + 7.0),
                    egui::Align2::LEFT_CENTER, timestamp,
                    egui::FontId::monospace(11.0), dim.gamma_multiply(0.5));

                let sym_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x + m + 95.0, meta_y), egui::vec2(36.0, 14.0));
                painter.rect_filled(sym_rect, 2.0, color_alpha(accent, alpha_ghost()));
                painter.text(sym_rect.center(), egui::Align2::CENTER_CENTER,
                    symbol, egui::FontId::monospace(11.0), accent);

                let mut chip_x = sym_rect.right() + 4.0;
                for tag in tags.iter() {
                    let tw = (tag.len() as f32) * 5.0 + 8.0;
                    let tr = egui::Rect::from_min_size(egui::pos2(chip_x, meta_y), egui::vec2(tw, 14.0));
                    if tr.right() > rect.right() - 16.0 { break; }
                    painter.rect_filled(tr, 2.0, color_alpha(dim, alpha_ghost()));
                    painter.text(tr.center(), egui::Align2::CENTER_CENTER,
                        tag, egui::FontId::monospace(11.0), dim);
                    chip_x = tr.right() + 3.0;
                }

                let dot_col = match sentiment {
                    1 => bull, -1 => bear, _ => dim.gamma_multiply(0.4),
                };
                painter.circle_filled(
                    egui::pos2(rect.right() - m - 4.0, meta_y + 7.0), 3.5, dot_col);
            })
            .show(ui);

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "NEWS_ROW", "Rows");
        resp
    }
}
