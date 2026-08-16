//! AlertRow — single price alert row: armed/triggered glyph, symbol,
//! comparator, target price, optional note, and a delete action.
//! Migrated to `RowShell` (painter mode) — embedded delete button click is
//! captured through a `Cell<bool>` shared with the painter body.

#![allow(dead_code, unused_imports)]

use std::cell::Cell;
use egui::{Color32, Response, Ui};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use super::super::super::style::*;
use crate::ui_kit::icons::Icon;
use crate::chart::renderer::ui::foundation::{
    interaction::InteractionState,
    shell::RowShell,
};
use crate::ui_kit::widgets::RowVariant;
use crate::ui_kit::widgets::tokens::Size;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;

type Theme = crate::chart_renderer::gpu::Theme;

#[derive(Clone, Copy, PartialEq)]
pub enum AlertCmp { Above, Below, Crosses }

impl AlertCmp {
    pub fn glyph(self) -> &'static str {
        match self { AlertCmp::Above => ">", AlertCmp::Below => "<", AlertCmp::Crosses => "x" }
    }
}

#[must_use = "AlertRow must be finalized with `.show(ui)` to render"]
pub struct AlertRow<'a> {
    symbol: &'a str,
    cmp: AlertCmp,
    target: f32,
    armed: bool,
    triggered: bool,
    note: Option<&'a str>,
    selected: bool,
    height: f32,
    theme: Option<&'a Theme>,
    theme_bull: Option<Color32>,
    theme_bear: Option<Color32>,
    theme_dim: Option<Color32>,
    theme_fg: Option<Color32>,
    theme_accent: Option<Color32>,
    theme_border: Option<Color32>,
}

impl<'a> AlertRow<'a> {
    pub fn new(symbol: &'a str, cmp: AlertCmp, target: f32) -> Self {
        Self {
            symbol, cmp, target,
            armed: true, triggered: false, note: None,
            selected: false, height: crate::chart_renderer::ui::style::style_row_height(),
            theme: None,
            theme_bull: None, theme_bear: None, theme_dim: None, theme_fg: None,
            theme_accent: None, theme_border: None,
        }
    }
    pub fn armed(mut self, v: bool) -> Self { self.armed = v; self }
    pub fn triggered(mut self, v: bool) -> Self { self.triggered = v; self }
    pub fn note(mut self, n: &'a str) -> Self { self.note = Some(n); self }
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

    /// Returns (row_response, delete_clicked).
    pub fn show(self, ui: &mut Ui) -> (Response, bool) {
        let theme_ref: &Theme = self.theme.expect("AlertRow requires a theme — call `.theme(t)` before `.show()`");
        let bull = self.theme_bull.unwrap_or(theme_ref.bull);
        let bear = self.theme_bear.unwrap_or(theme_ref.bear);
        let dim = self.theme_dim.unwrap_or(theme_ref.dim);
        let fg = self.theme_fg.unwrap_or(theme_ref.text);
        let accent = self.theme_accent.unwrap_or(theme_ref.accent);

        let symbol = self.symbol;
        let cmp = self.cmp;
        let target = self.target;
        let armed = self.armed;
        let triggered = self.triggered;
        let note = self.note;

        let delete_cell: Cell<bool> = Cell::new(false);
        let delete_ref = &delete_cell;

        let resp = RowShell::new(theme_ref, "")
            .variant(RowVariant::Default)
            .size(Size::Md)
            .state(InteractionState::default().selected(self.selected))
            .painter_mode(true)
            .painter_height(self.height)
            .painter_body(|ui, rect| {
                // Hot path: one cascade lookup per tier per row, cloned per string.
                let f_txt = TextStyle::MonoSm.font_id_in(ui);   // symbol / note
                let f_num = TextStyle::Numeric.font_id_in(ui);  // comparator + target price
                let painter = ui.painter();
                let cy = rect.center().y;

                let (glyph, gcol) = if triggered {
                    (Icon::CIRCLE, accent)
                } else if armed {
                    (Icon::CIRCLE, bull)
                } else {
                    (Icon::DOT, dim)
                };
                painter.text(egui::pos2(rect.left() + crate::ui_kit::style::gap_sm(), cy), egui::Align2::LEFT_CENTER,
                    glyph, crate::ui_kit::style::prop_at(crate::ui_kit::style::font_xl()), gcol);

                painter.text(egui::pos2(rect.left() + 22.0, cy), egui::Align2::LEFT_CENTER,
                    symbol, f_txt.clone(), fg);

                let cmp_col = match cmp {
                    AlertCmp::Above => bull, AlertCmp::Below => bear, AlertCmp::Crosses => accent,
                };
                let main = format!("{} {:.2}", cmp.glyph(), target);
                painter.text(egui::pos2(rect.left() + 80.0, cy), egui::Align2::LEFT_CENTER,
                    &main, f_num, cmp_col);

                if let Some(n) = note {
                    ui.painter().text(egui::pos2(rect.center().x + 30.0, cy),
                        egui::Align2::LEFT_CENTER,
                        n, f_txt.clone(), dim);
                }

                // Embedded delete button.
                // TODO: migrate row to PanelListRow + TrailingBtn for a cleaner slice API.
                let db = egui::Rect::from_min_size(
                    egui::pos2(rect.right() - 22.0, cy - 8.0), egui::vec2(icon_sm(), icon_sm()));
                let painter = ui.painter_at(db);
                let db_resp = crate::ui_kit::widgets::Button::close()
                    .placement(IconPlacement::ListRow)
                    .show_at(ui, &painter, db, theme_ref)
                    .on_hover_text("Delete alert");
                if db_resp.clicked() { delete_ref.set(true); }
            })
            .show(ui);

        crate::design_tokens::register_hit(
            [resp.rect.min.x, resp.rect.min.y, resp.rect.width(), resp.rect.height()],
            "ALERT_ROW", "Rows");
        (resp, delete_cell.get())
    }
}
