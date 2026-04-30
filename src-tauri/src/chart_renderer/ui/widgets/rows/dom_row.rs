//! DomRow — single price ladder rung: bid size | price | ask size, with
//! depth-bar fills behind size cells. Migrated to `RowShell` (painter mode).

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, Ui};
use super::super::super::style::*;
use super::super::foundation::{
    interaction::InteractionState,
    shell::RowShell,
    tokens::Size,
    variants::RowVariant,
};

type Theme = crate::chart_renderer::gpu::Theme;

fn fallback_theme() -> &'static Theme {
    &crate::chart_renderer::gpu::THEMES[0]
}

#[must_use = "DomRow must be finalized with `.show(ui)` to render"]
pub struct DomRow<'a> {
    price: f32,
    bid_size: u32,
    ask_size: u32,
    bid_fill: f32,
    ask_fill: f32,
    is_inside: bool,
    selected: bool,
    height: f32,
    price_fmt: &'a str,
    theme: Option<&'a Theme>,
    theme_bull: Option<Color32>,
    theme_bear: Option<Color32>,
    theme_dim: Option<Color32>,
    theme_fg: Option<Color32>,
    theme_accent: Option<Color32>,
    theme_border: Option<Color32>,
}

impl<'a> DomRow<'a> {
    pub fn new(price: f32, bid_size: u32, ask_size: u32) -> Self {
        Self {
            price, bid_size, ask_size,
            bid_fill: 0.0, ask_fill: 0.0, is_inside: false, selected: false,
            height: 18.0, price_fmt: "{:.2}",
            theme: None,
            theme_bull: None, theme_bear: None, theme_dim: None, theme_fg: None,
            theme_accent: None, theme_border: None,
        }
    }
    pub fn bid_fill(mut self, v: f32) -> Self { self.bid_fill = v.clamp(0.0, 1.0); self }
    pub fn ask_fill(mut self, v: f32) -> Self { self.ask_fill = v.clamp(0.0, 1.0); self }
    pub fn is_inside(mut self, v: bool) -> Self { self.is_inside = v; self }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.theme_bull = Some(t.bull);
        self.theme_bear = Some(t.bear);
        self.theme_dim = Some(t.dim);
        self.theme_fg = Some(t.text);
        self.theme_accent = Some(t.accent);
        self.theme_border = Some(t.toolbar_border);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme_ref: &Theme = match self.theme { Some(t) => t, None => fallback_theme() };
        let bull = self.theme_bull.unwrap_or(Color32::from_rgb(0, 200, 120));
        let bear = self.theme_bear.unwrap_or(Color32::from_rgb(220, 80, 80));
        let _dim = self.theme_dim.unwrap_or(Color32::from_gray(120));
        let fg = self.theme_fg.unwrap_or(Color32::from_gray(220));
        let accent = self.theme_accent.unwrap_or(Color32::from_rgb(80, 160, 220));

        let price = self.price;
        let bid_size = self.bid_size;
        let ask_size = self.ask_size;
        let bid_fill = self.bid_fill;
        let ask_fill = self.ask_fill;
        let is_inside = self.is_inside;

        let resp = RowShell::new(theme_ref, "")
            .variant(RowVariant::Default)
            .size(Size::Md)
            .state(InteractionState::default().selected(self.selected))
            .painter_mode(true)
            .painter_height(self.height)
            .painter_body(move |ui, rect| {
                let bid_w = rect.width() * 0.35;
                let ask_w = rect.width() * 0.35;
                let bid_rect = egui::Rect::from_min_size(rect.min, egui::vec2(bid_w, rect.height()));
                let ask_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.right() - ask_w, rect.min.y), egui::vec2(ask_w, rect.height()));
                let price_rect = egui::Rect::from_min_max(
                    egui::pos2(bid_rect.right(), rect.min.y),
                    egui::pos2(ask_rect.left(), rect.max.y));

                if bid_fill > 0.0 {
                    let fw = bid_w * bid_fill;
                    let r = egui::Rect::from_min_size(
                        egui::pos2(bid_rect.right() - fw, bid_rect.min.y),
                        egui::vec2(fw, bid_rect.height()),
                    );
                    ui.painter().rect_filled(r, 0.0, color_alpha(bull, alpha_subtle()));
                }
                if ask_fill > 0.0 {
                    let fw = ask_w * ask_fill;
                    let r = egui::Rect::from_min_size(ask_rect.min, egui::vec2(fw, ask_rect.height()));
                    ui.painter().rect_filled(r, 0.0, color_alpha(bear, alpha_subtle()));
                }

                let painter = ui.painter();
                let cy = rect.center().y;
                let f = egui::FontId::monospace(10.0);
                if bid_size > 0 {
                    painter.text(egui::pos2(bid_rect.right() - 6.0, cy), egui::Align2::RIGHT_CENTER,
                        &format!("{}", bid_size), f.clone(), bull);
                }
                let price_col = if is_inside { accent } else { fg };
                painter.text(price_rect.center(), egui::Align2::CENTER_CENTER,
                    &format!("{:.2}", price), f.clone(), price_col);
                if ask_size > 0 {
                    painter.text(egui::pos2(ask_rect.left() + 6.0, cy), egui::Align2::LEFT_CENTER,
                        &format!("{}", ask_size), f.clone(), bear);
                }
            })
            .show(ui);

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "DOM_ROW", "Rows");
        resp
    }
}
