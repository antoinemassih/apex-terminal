//! OrderRow — order book entry: side pill, symbol, qty, price, status, age.
//! Migrated to `RowShell` (painter mode). Cancel-button click is captured
//! through a `Cell<bool>` shared with the painter body.

#![allow(dead_code, unused_imports)]

use std::cell::Cell;
use egui::{Color32, Response, Ui};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use super::super::super::style::*;
use crate::chart::renderer::ui::foundation::{
    interaction::InteractionState,
    shell::RowShell,
};
use crate::ui_kit::widgets::RowVariant;
use crate::ui_kit::widgets::tokens::Size;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;

type Theme = crate::chart_renderer::gpu::Theme;

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
            age: None, selected: false, height: crate::chart_renderer::ui::style::style_row_height(), show_cancel: false,
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
        let theme_ref: &Theme = self.theme.expect("OrderRow requires a theme — call `.theme(t)` before `.show()`");
        let bull = self.theme_bull.unwrap_or(theme_ref.bull);
        let bear = self.theme_bear.unwrap_or(theme_ref.bear);
        let dim = self.theme_dim.unwrap_or(theme_ref.dim);
        let fg = self.theme_fg.unwrap_or(theme_ref.text);

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
                // Hot path (one row per order, many rows per frame): resolve each
                // tier through the cascade ONCE and clone per string.
                let f_txt = TextStyle::MonoSm.font_id_in(ui);   // symbol / status / age / side pill
                let f_num = TextStyle::Numeric.font_id_in(ui);  // qty @ price
                let painter = ui.painter();
                let cy = rect.center().y;
                let side_col = match side { OrderSideTag::Buy => bull, OrderSideTag::Sell => bear };
                let side_lbl = match side { OrderSideTag::Buy => "B", OrderSideTag::Sell => "S" };

                // Side pill.
                let pill = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 6.0, cy - 7.0),
                    egui::vec2(14.0, 14.0));
                painter.rect_filled(pill, radius_xs(), color_alpha(side_col, alpha_subtle()));
                painter.text(pill.center(), egui::Align2::CENTER_CENTER,
                    side_lbl, f_txt.clone(), side_col);

                // Symbol — clip to prevent long option tickers bleeding into qty@price column.
                let sym_x = pill.right() + 6.0;
                painter.with_clip_rect(painter.clip_rect().intersect(
                    egui::Rect::from_x_y_ranges(sym_x..=(rect.center().x - 4.0), rect.y_range())
                )).text(egui::pos2(sym_x, cy), egui::Align2::LEFT_CENTER, symbol, f_txt.clone(), fg);

                // Qty @ price — clip between symbol and status columns.
                //
                // M5, two coupled defects here, both from pinned geometry:
                //
                // 1. The right reserve was a pinned `84.0`, while the status
                //    label below is RIGHT-anchored at `right - 80.0` and grows
                //    LEFTWARD. So the reserve cleared the status ANCHOR by 4px
                //    but not the status TEXT — any status wider than 4px shared
                //    pixels with this column.
                // 2. The text is centred on the ROW (`rect.center().x`) while
                //    its band was NOT centred on the row (`left+30` vs
                //    `right-84`). The band's centre sits 27px to the left, so
                //    the text ran out of room on the right while ~54px of band
                //    went unused on the left — it clipped early, and
                //    asymmetrically. Budget was `W - 168`; at Width::Medium
                //    (~270 usable) that is 102px, and "1000 @ 4523.75" needs
                //    ~109px once font_body is 13 (cadence, glass) — a hard clip
                //    mid-glyph, with no ellipsis.
                //
                // Derive both ends instead: measure the status label, end the
                // band before the text actually starts, and centre the value in
                // the band it really has.
                let status_w = ui.fonts(|f| {
                    f.layout_no_wrap(status.to_string(), f_txt.clone(), dim).rect.width()
                });
                let band_l = rect.left() + 30.0;
                let band_r = (rect.right() - 80.0 - status_w - gap_xs()).max(band_l);
                let band = egui::Rect::from_x_y_ranges(band_l..=band_r, rect.y_range());
                painter.with_clip_rect(painter.clip_rect().intersect(band))
                    .text(egui::pos2(band.center().x, cy), egui::Align2::CENTER_CENTER,
                        &format!("{} @ {:.2}", qty, price), f_num, fg);

                painter.text(egui::pos2(rect.right() - 80.0, cy), egui::Align2::RIGHT_CENTER,
                    status, f_txt.clone(), dim);

                if let Some(a) = age {
                    let x = if show_cancel { rect.right() - 28.0 } else { rect.right() - 6.0 };
                    ui.painter().text(egui::pos2(x, cy), egui::Align2::RIGHT_CENTER,
                        a, f_txt.clone(), color_subtle(dim));
                }

                // Embedded cancel button.
                // TODO: migrate row to PanelListRow + TrailingBtn for a cleaner slice API.
                if show_cancel {
                    let cb = egui::Rect::from_min_size(
                        egui::pos2(rect.right() - 22.0, cy - 8.0),
                        egui::vec2(icon_sm(), icon_sm()));
                    let painter = ui.painter_at(cb);
                    let cb_resp = crate::ui_kit::widgets::Button::close()
                        .placement(IconPlacement::ListRow)
                        .show_at(ui, &painter, cb, theme_ref)
                        .on_hover_text("Cancel order");
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
