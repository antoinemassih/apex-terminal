//! `PlayCard` — playbook entry summary with status pill, R:R bar, multi-target
//! rows, tags, notes, accent stripe, shadow, and hover delete/activate buttons.
//!
//! Wave 5: rich painter-driven card extracted from `plays_panel::draw_play_card`.
//! Sits alongside `TradeCard` in the cards module. The visual layout is
//! preserved exactly — this widget is a clean façade over the existing painter
//! pixels with a typed response struct for action wiring.

#![allow(dead_code, unused_imports)]

use egui::{self, Ui};

use super::super::super::style::*;
use crate::chart_renderer::{Play, PlayDirection, PlayStatus, PlayType};
use crate::chart_renderer::gpu::Theme;
use crate::ui_kit::icons::Icon;

/// Result of rendering a `PlayCard`. Action booleans are surfaced for the
/// caller to wire into mutation/state — no callbacks, keeps borrows clean.
pub struct PlayCardResponse {
    pub response:         egui::Response,
    pub clicked:          bool,
    pub delete_clicked:   bool,
    pub activate_clicked: bool,
}

#[must_use = "PlayCard must be rendered with `.show(ui)`"]
pub struct PlayCard<'a> {
    play:  &'a Play,
    theme: &'a Theme,
}

impl<'a> PlayCard<'a> {
    pub fn new(play: &'a Play, theme: &'a Theme) -> Self {
        Self { play, theme }
    }

    pub fn show(self, ui: &mut Ui) -> PlayCardResponse {
        let play = self.play;
        let t = self.theme;

        let is_long = play.direction == PlayDirection::Long;
        let dir_color = if is_long { t.bull } else { t.bear };
        let card_w = ui.available_width();
        let has_tags = !play.tags.is_empty();
        let has_targets = play.targets.len() > 1;
        let card_h = 76.0
            + if !play.notes.is_empty() { 14.0 } else { 0.0 }
            + if has_tags { 14.0 } else { 0.0 }
            + if has_targets { play.targets.len() as f32 * 10.0 } else { 0.0 };

        let (card_rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
        let p = ui.painter();

        // Shadow
        p.rect_filled(card_rect.translate(egui::vec2(0.0, 2.0)).expand(1.0), RADIUS_LG, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25));
        p.rect_filled(card_rect.translate(egui::vec2(0.0, 1.0)), RADIUS_LG, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 15));

        let bg = if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            egui::Color32::from_rgba_unmultiplied(
                t.toolbar_border.r().saturating_add(15), t.toolbar_border.g().saturating_add(15),
                t.toolbar_border.b().saturating_add(15), 255)
        } else {
            egui::Color32::from_rgb(t.toolbar_bg.r().saturating_add(8), t.toolbar_bg.g().saturating_add(8), t.toolbar_bg.b().saturating_add(8))
        };
        p.rect_filled(card_rect, RADIUS_LG, bg);

        // Top bevel highlight
        p.rect_filled(egui::Rect::from_min_max(card_rect.min, egui::pos2(card_rect.right(), card_rect.top() + 1.0)),
            egui::CornerRadius { nw: RADIUS_LG as u8, ne: RADIUS_LG as u8, sw: 0, se: 0 },
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, if t.is_light() { 40 } else { 8 }));

        p.rect_stroke(card_rect, RADIUS_LG, egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_STRONG)), egui::StrokeKind::Outside);

        // Accent stripe
        p.rect_filled(egui::Rect::from_min_max(
            egui::pos2(card_rect.left(), card_rect.top() + 4.0),
            egui::pos2(card_rect.left() + 3.0, card_rect.bottom() - 4.0)),
            egui::CornerRadius { nw: 2, sw: 2, ne: 0, se: 0 }, dir_color);

        let cx = card_rect.left() + 12.0;
        let mut cy = card_rect.top() + 10.0;

        // Row 1: Direction pill + type icon + symbol + status + R:R
        {
            let pill_w = 42.0;
            let pill_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 1.0), egui::vec2(pill_w, 16.0));
            p.rect_filled(pill_rect, 3.0, color_alpha(dir_color, ALPHA_TINT));
            p.rect_stroke(pill_rect, 3.0, egui::Stroke::new(STROKE_THIN, color_alpha(dir_color, ALPHA_DIM)), egui::StrokeKind::Outside);
            p.text(pill_rect.center(), egui::Align2::CENTER_CENTER, play.direction.label(), egui::FontId::monospace(FONT_XS), dir_color);

            p.text(egui::pos2(cx + pill_w + 6.0, cy + 6.0), egui::Align2::LEFT_CENTER,
                play.play_type.icon(), egui::FontId::proportional(FONT_SM), t.dim.gamma_multiply(0.5));
            p.text(egui::pos2(cx + pill_w + 22.0, cy + 6.0), egui::Align2::LEFT_CENTER,
                &play.symbol, egui::FontId::monospace(FONT_LG), t.text);

            let status_color = match play.status {
                PlayStatus::Draft => t.dim, PlayStatus::Published => t.accent,
                PlayStatus::Active => egui::Color32::from_rgb(255, 191, 0),
                PlayStatus::Won => t.bull, PlayStatus::Lost => t.bear, _ => t.dim.gamma_multiply(0.5),
            };
            let status_x = card_rect.right() - 60.0;
            let sr = egui::Rect::from_min_size(egui::pos2(status_x, cy - 1.0), egui::vec2(48.0, 16.0));
            p.rect_filled(sr, 3.0, color_alpha(status_color, ALPHA_SUBTLE));
            p.text(sr.center(), egui::Align2::CENTER_CENTER, play.status.label(), egui::FontId::monospace(7.0), status_color);

            if play.risk_reward > 0.0 {
                p.text(egui::pos2(status_x - 8.0, cy + 6.0), egui::Align2::RIGHT_CENTER,
                    &format!("{:.1}R", play.risk_reward), egui::FontId::monospace(FONT_SM), t.accent);
            }
            cy += 22.0;
        }

        // Row 2: Entry / Target / Stop
        {
            let col_w = (card_w - 24.0) / 3.0;
            p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER, "ENTRY", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
            p.text(egui::pos2(cx + col_w * 0.6, cy + 4.0), egui::Align2::LEFT_CENTER,
                &format!("${:.2}", play.entry_price), egui::FontId::monospace(FONT_SM), t.text);
            let tx = cx + col_w;
            p.text(egui::pos2(tx, cy + 4.0), egui::Align2::LEFT_CENTER, "TARGET", egui::FontId::monospace(7.0), t.bull.gamma_multiply(0.6));
            p.text(egui::pos2(tx + col_w * 0.6, cy + 4.0), egui::Align2::LEFT_CENTER,
                &format!("${:.2}", play.target_price), egui::FontId::monospace(FONT_SM), t.bull);
            if play.play_type != PlayType::Scalp {
                let sx = cx + col_w * 2.0;
                p.text(egui::pos2(sx, cy + 4.0), egui::Align2::LEFT_CENTER, "STOP", egui::FontId::monospace(7.0), t.bear.gamma_multiply(0.6));
                p.text(egui::pos2(sx + col_w * 0.5, cy + 4.0), egui::Align2::LEFT_CENTER,
                    &format!("${:.2}", play.stop_price), egui::FontId::monospace(FONT_SM), t.bear);
            }
            cy += 20.0;
        }

        // Row 2b: Additional targets with allocations
        if has_targets {
            for tgt in &play.targets {
                p.text(egui::pos2(cx + 8.0, cy + 4.0), egui::Align2::LEFT_CENTER,
                    &tgt.label, egui::FontId::monospace(7.0), t.bull.gamma_multiply(0.5));
                p.text(egui::pos2(cx + 30.0, cy + 4.0), egui::Align2::LEFT_CENTER,
                    &format!("${:.2}", tgt.price), egui::FontId::monospace(FONT_XS), t.bull);
                p.text(egui::pos2(cx + 100.0, cy + 4.0), egui::Align2::LEFT_CENTER,
                    &format!("{}%", (tgt.pct * 100.0) as i32), egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
                cy += 12.0;
            }
        }

        // R:R bar
        {
            let bar_x = cx;
            let bar_w = card_w - 24.0;
            let bar_rect = egui::Rect::from_min_size(egui::pos2(bar_x, cy), egui::vec2(bar_w, 4.0));
            p.rect_filled(bar_rect, 2.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
            let total_range = (play.target_price - play.stop_price).abs();
            let risk = (play.entry_price - play.stop_price).abs();
            let risk_pct = if total_range > 0.0 { (risk / total_range).min(1.0) } else { 0.5 };
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, cy), egui::vec2(bar_w * risk_pct, 4.0)), 2.0, color_alpha(t.bear, ALPHA_DIM));
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x + bar_w * risk_pct, cy), egui::vec2(bar_w * (1.0 - risk_pct), 4.0)), 2.0, color_alpha(t.bull, ALPHA_DIM));
            p.circle_filled(egui::pos2(bar_x + bar_w * risk_pct, cy + 2.0), 3.0, t.text);
            cy += 10.0;
        }

        // Tags
        if has_tags {
            let mut tx = cx;
            for tag in &play.tags {
                p.text(egui::pos2(tx, cy + 4.0), egui::Align2::LEFT_CENTER,
                    &format!("#{}", tag), egui::FontId::monospace(7.0), t.accent.gamma_multiply(0.5));
                tx += tag.len() as f32 * 5.0 + 12.0;
            }
            cy += 12.0;
        }

        // Notes
        if !play.notes.is_empty() {
            p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
                &play.notes, egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.5));
        }

        // Hover buttons
        let mut delete_clicked = false;
        let mut activate_clicked = false;
        let mut btn_clicked = false;
        if resp.hovered() {
            let del_rect = egui::Rect::from_min_size(egui::pos2(card_rect.right() - 18.0, card_rect.top() + 4.0), egui::vec2(14.0, 14.0));
            let del_resp = ui.interact(del_rect, egui::Id::new(("play_del", &play.id[..8])), egui::Sense::click());
            if del_resp.hovered() {
                ui.painter().rect_filled(del_rect, 2.0, color_alpha(t.bear, ALPHA_GHOST));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.painter().text(del_rect.center(), egui::Align2::CENTER_CENTER, "\u{00D7}", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.5));
            if del_resp.clicked() { delete_clicked = true; btn_clicked = true; }

            if play.status == PlayStatus::Draft {
                let act_rect = egui::Rect::from_min_size(egui::pos2(card_rect.right() - 60.0, card_rect.bottom() - 18.0), egui::vec2(52.0, 14.0));
                let act_resp = ui.interact(act_rect, egui::Id::new(("play_act", &play.id[..8])), egui::Sense::click());
                let act_bg = if act_resp.hovered() { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.accent, ALPHA_GHOST) };
                ui.painter().rect_filled(act_rect, 3.0, act_bg);
                ui.painter().text(act_rect.center(), egui::Align2::CENTER_CENTER, "Activate", egui::FontId::monospace(7.0), t.accent);
                if act_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                if act_resp.clicked() { activate_clicked = true; btn_clicked = true; }
            }
        }

        let clicked = resp.clicked() && !btn_clicked;

        ui.add_space(GAP_MD);

        // Suppress unused-import warning for Icon (kept for parity with panel).
        let _ = Icon::STAR;

        PlayCardResponse { response: resp, clicked, delete_clicked, activate_clicked }
    }
}
