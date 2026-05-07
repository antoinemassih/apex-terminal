//! OrderRow — order book entry: side pill, symbol, qty, price, status, age.
//! Migrated to `RowShell` (painter mode). Cancel-button click is captured
//! through a `Cell<bool>` shared with the painter body.

#![allow(dead_code, unused_imports)]

use std::cell::Cell;
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

#[derive(Clone, Copy, PartialEq)]
pub enum OrderSideTag { Buy, Sell }

#[must_use = "OrderRow must be finalized with `.show(ui)` to render"]
pub struct OrderRow<'a> {
    side: OrderSideTag,
    symbol: &'a str,
    qty: i64,
    price: f32,
    status: &'a str,
    age: Option<&'a str>,
    selected: bool,
    height: f32,
    show_cancel: bool,
    theme: Option<&'a Theme>,
    theme_bull: Option<Color32>,
    theme_bear: Option<Color32>,
    theme_dim: Option<Color32>,
    theme_fg: Option<Color32>,
    theme_accent: Option<Color32>,
    theme_border: Option<Color32>,
}

impl<'a> OrderRow<'a> {
    pub fn new(side: OrderSideTag, symbol: &'a str, qty: i64, price: f32, status: &'a str) -> Self {
        Self {
            side, symbol, qty, price, status,
            age: None, selected: false, height: 22.0, show_cancel: false,
            theme: None,
            theme_bull: None, theme_bear: None, theme_dim: None, theme_fg: None,
            theme_accent: None, theme_border: None,
        }
    }
    pub fn age(mut self, s: &'a str) -> Self { self.age = Some(s); self }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn show_cancel(mut self, v: bool) -> Self { self.show_cancel = v; self }
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

    /// Returns (row_response, cancel_clicked).
    pub fn show(self, ui: &mut Ui) -> (Response, bool) {
        let theme_ref: &Theme = match self.theme { Some(t) => t, None => fallback_theme() };
        let ft = fallback_theme();
        let bull = self.theme_bull.unwrap_or(ft.bull);
        let bear = self.theme_bear.unwrap_or(ft.bear);
        let dim = self.theme_dim.unwrap_or(ft.dim);
        let fg = self.theme_fg.unwrap_or(ft.text);

        let side = self.side;
        let symbol = self.symbol;
        let qty = self.qty;
        let price = self.price;
        let status = self.status;
        let age = self.age;
        let show_cancel = self.show_cancel;

        let cancel_cell: Cell<bool> = Cell::new(false);
        let cancel_ref = &cancel_cell;

        let resp = RowShell::new(theme_ref, "")
            .variant(RowVariant::Default)
            .size(Size::Md)
            .state(InteractionState::default().selected(self.selected))
            .painter_mode(true)
            .painter_height(self.height)
            .painter_body(|ui, rect| {
                let painter = ui.painter();
                let cy = rect.center().y;
                let side_col = match side { OrderSideTag::Buy => bull, OrderSideTag::Sell => bear };
                let side_lbl = match side { OrderSideTag::Buy => "B", OrderSideTag::Sell => "S" };

                // Side pill.
                let pill = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 6.0, cy - 7.0),
                    egui::vec2(14.0, 14.0));
                painter.rect_filled(pill, 2.0, color_alpha(side_col, alpha_subtle()));
                painter.text(pill.center(), egui::Align2::CENTER_CENTER,
                    side_lbl, egui::FontId::monospace(11.0), side_col);

                painter.text(egui::pos2(pill.right() + 6.0, cy), egui::Align2::LEFT_CENTER,
                    symbol, egui::FontId::monospace(11.0), fg);

                painter.text(egui::pos2(rect.center().x, cy), egui::Align2::CENTER_CENTER,
                    &format!("{} @ {:.2}", qty, price),
                    egui::FontId::monospace(11.0), fg);

                painter.text(egui::pos2(rect.right() - 80.0, cy), egui::Align2::RIGHT_CENTER,
                    status, egui::FontId::monospace(11.0), dim);

                if let Some(a) = age {
                    let x = if show_cancel { rect.right() - 28.0 } else { rect.right() - 6.0 };
                    ui.painter().text(egui::pos2(x, cy), egui::Align2::RIGHT_CENTER,
                        a, egui::FontId::monospace(11.0), dim.gamma_multiply(0.7));
                }

                // Embedded cancel button.
                if show_cancel {
                    let cb = egui::Rect::from_min_size(
                        egui::pos2(rect.right() - 22.0, cy - 8.0),
                        egui::vec2(16.0, 16.0));
                    let cb_resp = ui.allocate_rect(cb, egui::Sense::click());
                    let col = if cb_resp.hovered() { bear } else { dim };
                    ui.painter().text(cb.center(), egui::Align2::CENTER_CENTER,
                        "×", egui::FontId::monospace(11.0), col);
                    if cb_resp.clicked() { cancel_ref.set(true); }
                }
            })
            .show(ui);

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "ORDER_ROW", "Rows");
        (resp, cancel_cell.get())
    }
}
