//! OptionChainRow — call side | strike | put side, with optional greeks columns.
//!
//! Migrated to `RowShell` (painter mode). Shell paints base fill + hover/
//! selected/focus; body paints ITM tint halves, columns, strike center.

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

#[derive(Clone, Copy, Default)]
pub struct ChainSide {
    pub bid: f32,
    pub ask: f32,
    pub last: f32,
    pub volume: u64,
    pub open_interest: u64,
    pub iv: f32,
    pub delta: f32,
    pub gamma: f32,
    pub theta: f32,
    pub vega: f32,
    pub itm: bool,
}

#[must_use = "OptionChainRow must be finalized with `.show(ui)` to render"]
pub struct OptionChainRow<'a> {
    strike: f32,
    call: ChainSide,
    put: ChainSide,
    show_greeks: bool,
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

impl<'a> OptionChainRow<'a> {
    pub fn new(strike: f32, call: ChainSide, put: ChainSide) -> Self {
        Self {
            strike, call, put,
            show_greeks: false, selected: false, height: 20.0,
            theme: None,
            theme_bg: None, theme_border: None, theme_accent: None,
            theme_bull: None, theme_bear: None, theme_dim: None, theme_fg: None,
        }
    }
    pub fn show_greeks(mut self, v: bool) -> Self { self.show_greeks = v; self }
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
        let ft = fallback_theme();
        let accent = self.theme_accent.unwrap_or(ft.accent);
        let bull = self.theme_bull.unwrap_or(ft.bull);
        let bear = self.theme_bear.unwrap_or(ft.bear);
        let dim = self.theme_dim.unwrap_or(ft.dim);
        let fg = self.theme_fg.unwrap_or(ft.text);
        let strike = self.strike;
        let call = self.call;
        let put = self.put;
        let show_greeks = self.show_greeks;

        let resp = RowShell::new(theme_ref, "")
            .variant(RowVariant::Default)
            .size(Size::Md)
            .state(InteractionState::default().selected(self.selected))
            .painter_mode(true)
            .painter_height(self.height)
            .painter_body(move |ui, rect| {
                let half = rect.width() * 0.5;
                let call_rect = egui::Rect::from_min_size(rect.min, egui::vec2(half, rect.height()));
                let put_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x + half, rect.min.y), egui::vec2(half, rect.height()));
                if call.itm { ui.painter().rect_filled(call_rect, 0.0, color_alpha(bull, alpha_ghost())); }
                if put.itm  { ui.painter().rect_filled(put_rect,  0.0, color_alpha(bear, alpha_ghost())); }

                let painter = ui.painter();
                let cy = rect.center().y;
                let f = egui::FontId::monospace(11.0);

                painter.text(egui::pos2(call_rect.left() + 8.0, cy), egui::Align2::LEFT_CENTER,
                    &format!("{:.2}", call.bid), f.clone(), bull);
                painter.text(egui::pos2(call_rect.left() + call_rect.width() * 0.4, cy),
                    egui::Align2::CENTER_CENTER, &format!("{:.2}", call.ask), f.clone(), fg);
                painter.text(egui::pos2(call_rect.right() - 8.0, cy), egui::Align2::RIGHT_CENTER,
                    &format!("{}", call.volume), f.clone(), dim);

                let strike_x = rect.center().x;
                painter.text(egui::pos2(strike_x, cy), egui::Align2::CENTER_CENTER,
                    &format!("{:.2}", strike),
                    egui::FontId::monospace(11.0), accent);

                painter.text(egui::pos2(put_rect.left() + 8.0, cy), egui::Align2::LEFT_CENTER,
                    &format!("{}", put.volume), f.clone(), dim);
                painter.text(egui::pos2(put_rect.left() + put_rect.width() * 0.6, cy),
                    egui::Align2::CENTER_CENTER, &format!("{:.2}", put.bid), f.clone(), fg);
                painter.text(egui::pos2(put_rect.right() - 8.0, cy), egui::Align2::RIGHT_CENTER,
                    &format!("{:.2}", put.ask), f.clone(), bear);

                if show_greeks {
                    painter.text(egui::pos2(rect.right() - 4.0, rect.bottom() - 4.0),
                        egui::Align2::RIGHT_BOTTOM,
                        &format!("Δ{:+.2}", call.delta),
                        egui::FontId::monospace(11.0), dim);
                }
            })
            .show(ui);

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "OPTION_CHAIN_ROW", "Rows");
        resp
    }
}
