//! Plays panel — shareable strategy cards (playbook).
//! Shows in the Feed sidebar under the "Plays" tab.
//! Each play is a polished visual card with shadow, bevel, and rich layout.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::chart_renderer::{Play, PlayDirection, PlayStatus};
use crate::ui_kit::icons::Icon;

/// Draw the plays tab content inside the Feed sidebar.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    t: &Theme,
) {
    ui.add_space(GAP_SM);

    // ── Header with "New Play" button ──
    ui.horizontal(|ui| {
        section_label(ui, &format!("PLAYBOOK ({})", watchlist.plays.len()), t.accent);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if small_action_btn(ui, "+ New Play", t.accent) {
                watchlist.play_editor_open = !watchlist.play_editor_open;
            }
        });
    });
    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Play editor (inline) ──
    if watchlist.play_editor_open {
        draw_play_editor(ui, watchlist, t);
        ui.add_space(GAP_SM);
        separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
        ui.add_space(GAP_SM);
    }

    // ── Play cards ──
    if watchlist.plays.is_empty() {
        ui.add_space(GAP_3XL);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(Icon::STAR).size(28.0).color(t.dim.gamma_multiply(0.2)));
            ui.add_space(GAP_SM);
            ui.label(egui::RichText::new("No plays yet").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
            ui.label(egui::RichText::new("Create a play to share a trade idea")
                .monospace().size(FONT_XS).color(t.dim.gamma_multiply(0.3)));
        });
        return;
    }

    let mut remove_id: Option<String> = None;

    egui::ScrollArea::vertical().id_salt("plays_scroll").show(ui, |ui| {
        for play in &watchlist.plays {
            draw_play_card(ui, play, t, &mut remove_id);
        }
    });

    if let Some(id) = remove_id {
        watchlist.plays.retain(|p| p.id != id);
    }
}

/// A polished play card with shadow, rounded corners, direction stripe, and rich layout.
fn draw_play_card(ui: &mut egui::Ui, play: &Play, t: &Theme, remove_id: &mut Option<String>) {
    let is_long = play.direction == PlayDirection::Long;
    let dir_color = if is_long { t.bull } else { t.bear };
    let card_w = ui.available_width();

    // Reserve space for the card
    let card_h = if play.notes.is_empty() { 76.0 } else { 92.0 };
    let (card_rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
    let p = ui.painter();

    // ── Drop shadow (offset, blur approximated with multiple rects) ──
    let shadow_rect = card_rect.translate(egui::vec2(0.0, 2.0));
    p.rect_filled(shadow_rect.expand(1.0), RADIUS_LG, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25));
    let shadow2 = card_rect.translate(egui::vec2(0.0, 1.0));
    p.rect_filled(shadow2, RADIUS_LG, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 15));

    // ── Card background ──
    let bg = if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        egui::Color32::from_rgba_unmultiplied(
            t.toolbar_border.r().saturating_add(15),
            t.toolbar_border.g().saturating_add(15),
            t.toolbar_border.b().saturating_add(15), 255)
    } else {
        egui::Color32::from_rgb(
            t.toolbar_bg.r().saturating_add(8),
            t.toolbar_bg.g().saturating_add(8),
            t.toolbar_bg.b().saturating_add(8))
    };
    p.rect_filled(card_rect, RADIUS_LG, bg);

    // ── Subtle top highlight (bevel effect) ──
    let bevel_rect = egui::Rect::from_min_max(
        card_rect.min,
        egui::pos2(card_rect.right(), card_rect.top() + 1.0));
    p.rect_filled(bevel_rect,
        egui::CornerRadius { nw: RADIUS_LG as u8, ne: RADIUS_LG as u8, sw: 0, se: 0 },
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8));

    // ── Border ──
    p.rect_stroke(card_rect, RADIUS_LG,
        egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_STRONG)),
        egui::StrokeKind::Outside);

    // ── Left accent stripe ──
    let stripe_rect = egui::Rect::from_min_max(
        egui::pos2(card_rect.left(), card_rect.top() + 4.0),
        egui::pos2(card_rect.left() + 3.0, card_rect.bottom() - 4.0));
    p.rect_filled(stripe_rect,
        egui::CornerRadius { nw: 2, sw: 2, ne: 0, se: 0 }, dir_color);

    // ── Content layout ──
    let cx = card_rect.left() + 12.0;
    let mut cy = card_rect.top() + 10.0;

    // Row 1: Direction + Symbol + Status + R:R + Delete
    {
        // Direction pill
        let dir_label = play.direction.label();
        let pill_w = 42.0;
        let pill_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 1.0), egui::vec2(pill_w, 16.0));
        p.rect_filled(pill_rect, 3.0, color_alpha(dir_color, ALPHA_TINT));
        p.rect_stroke(pill_rect, 3.0,
            egui::Stroke::new(STROKE_THIN, color_alpha(dir_color, ALPHA_DIM)), egui::StrokeKind::Outside);
        p.text(pill_rect.center(), egui::Align2::CENTER_CENTER,
            dir_label, egui::FontId::monospace(FONT_XS), dir_color);

        // Symbol
        p.text(egui::pos2(cx + pill_w + 8.0, cy + 6.0), egui::Align2::LEFT_CENTER,
            &play.symbol, egui::FontId::monospace(FONT_LG), TEXT_PRIMARY);

        // Status badge (right side)
        let status_color = match play.status {
            PlayStatus::Draft => t.dim,
            PlayStatus::Published => t.accent,
            PlayStatus::Active => egui::Color32::from_rgb(255, 191, 0),
            PlayStatus::Won => t.bull,
            PlayStatus::Lost => t.bear,
            _ => t.dim.gamma_multiply(0.5),
        };
        let status_label = play.status.label();
        let status_x = card_rect.right() - 60.0;
        let status_rect = egui::Rect::from_min_size(egui::pos2(status_x, cy - 1.0), egui::vec2(48.0, 16.0));
        p.rect_filled(status_rect, 3.0, color_alpha(status_color, ALPHA_SUBTLE));
        p.text(status_rect.center(), egui::Align2::CENTER_CENTER,
            status_label, egui::FontId::monospace(7.0), status_color);

        // R:R (next to status)
        if play.risk_reward > 0.0 {
            p.text(egui::pos2(status_x - 8.0, cy + 6.0), egui::Align2::RIGHT_CENTER,
                &format!("{:.1}R", play.risk_reward), egui::FontId::monospace(FONT_SM), t.accent);
        }

        cy += 22.0;
    }

    // Row 2: Entry / Target / Stop — with colored labels
    {
        let col_w = (card_w - 24.0) / 3.0;

        // Entry
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            "ENTRY", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
        p.text(egui::pos2(cx + col_w * 0.6, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("${:.2}", play.entry_price), egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);

        // Target
        let tx = cx + col_w;
        p.text(egui::pos2(tx, cy + 4.0), egui::Align2::LEFT_CENTER,
            "TARGET", egui::FontId::monospace(7.0), t.bull.gamma_multiply(0.6));
        p.text(egui::pos2(tx + col_w * 0.6, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("${:.2}", play.target_price), egui::FontId::monospace(FONT_SM), t.bull);

        // Stop
        let sx = cx + col_w * 2.0;
        p.text(egui::pos2(sx, cy + 4.0), egui::Align2::LEFT_CENTER,
            "STOP", egui::FontId::monospace(7.0), t.bear.gamma_multiply(0.6));
        p.text(egui::pos2(sx + col_w * 0.5, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("${:.2}", play.stop_price), egui::FontId::monospace(FONT_SM), t.bear);

        cy += 20.0;
    }

    // Row 3: Visual R:R bar
    {
        let bar_x = cx;
        let bar_w = card_w - 24.0;
        let bar_h = 4.0;
        let bar_rect = egui::Rect::from_min_size(egui::pos2(bar_x, cy), egui::vec2(bar_w, bar_h));

        // Full bar background
        p.rect_filled(bar_rect, 2.0, color_alpha(t.toolbar_border, ALPHA_MUTED));

        // Risk portion (red)
        let total_range = (play.target_price - play.stop_price).abs();
        let risk = (play.entry_price - play.stop_price).abs();
        let risk_pct = if total_range > 0.0 { (risk / total_range).min(1.0) } else { 0.5 };
        let risk_rect = egui::Rect::from_min_size(egui::pos2(bar_x, cy), egui::vec2(bar_w * risk_pct, bar_h));
        p.rect_filled(risk_rect, 2.0, color_alpha(t.bear, ALPHA_DIM));

        // Reward portion (green)
        let reward_rect = egui::Rect::from_min_size(
            egui::pos2(bar_x + bar_w * risk_pct, cy),
            egui::vec2(bar_w * (1.0 - risk_pct), bar_h));
        p.rect_filled(reward_rect, 2.0, color_alpha(t.bull, ALPHA_DIM));

        // Entry marker
        let entry_x = bar_x + bar_w * risk_pct;
        p.circle_filled(egui::pos2(entry_x, cy + bar_h / 2.0), 3.0, TEXT_PRIMARY);

        cy += 10.0;
    }

    // Row 4: Notes (if any)
    if !play.notes.is_empty() {
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            &play.notes, egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.5));
    }

    // Delete button (top-right corner, visible on hover)
    if resp.hovered() {
        let del_rect = egui::Rect::from_min_size(
            egui::pos2(card_rect.right() - 18.0, card_rect.top() + 4.0),
            egui::vec2(14.0, 14.0));
        let del_resp = ui.interact(del_rect, egui::Id::new(("play_del", &play.id[..8])), egui::Sense::click());
        if del_resp.hovered() {
            p.rect_filled(del_rect, 2.0, color_alpha(t.bear, ALPHA_GHOST));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        p.text(del_rect.center(), egui::Align2::CENTER_CENTER,
            "\u{00D7}", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.5));
        if del_resp.clicked() {
            *remove_id = Some(play.id.clone());
        }
    }

    ui.add_space(GAP_MD);
}

/// Inline play editor with direction toggle, price inputs, notes.
fn draw_play_editor(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    egui::Frame::NONE
        .fill(color_alpha(t.toolbar_border, ALPHA_FAINT))
        .inner_margin(egui::Margin::same(GAP_LG as i8))
        .corner_radius(RADIUS_LG)
        .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_MUTED)))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("NEW PLAY").monospace().size(FONT_SM).strong().color(t.accent));
            ui.add_space(GAP_SM);

            // Direction toggle
            ui.horizontal(|ui| {
                let is_long = watchlist.play_editor_direction == PlayDirection::Long;
                for (dir, label, color) in [
                    (PlayDirection::Long, "LONG", t.bull),
                    (PlayDirection::Short, "SHORT", t.bear),
                ] {
                    let sel = watchlist.play_editor_direction == dir;
                    let fg = if sel { color } else { t.dim.gamma_multiply(0.5) };
                    let bg = if sel { color_alpha(color, ALPHA_TINT) } else { egui::Color32::TRANSPARENT };
                    if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(FONT_SM).strong().color(fg))
                        .fill(bg).corner_radius(RADIUS_SM)
                        .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(color, ALPHA_LINE) } else { egui::Stroke::NONE.color }))
                        .min_size(egui::vec2(56.0, 22.0))).clicked() {
                        watchlist.play_editor_direction = dir;
                    }
                }
            });
            ui.add_space(GAP_XS);

            // Symbol
            ui.horizontal(|ui| {
                dim_label(ui, "Symbol", t.dim);
                ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_symbol)
                    .desired_width(80.0).font(egui::FontId::monospace(FONT_SM)).hint_text("AAPL"));
            });

            // Entry + Target
            ui.horizontal(|ui| {
                dim_label(ui, "Entry", t.dim);
                ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_entry)
                    .desired_width(70.0).font(egui::FontId::monospace(FONT_SM)).hint_text("150.00"));
                ui.add_space(GAP_SM);
                dim_label(ui, "Target", t.dim);
                ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_target)
                    .desired_width(70.0).font(egui::FontId::monospace(FONT_SM)).hint_text("155.00"));
            });

            // Stop
            ui.horizontal(|ui| {
                dim_label(ui, "Stop", t.dim);
                ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_stop)
                    .desired_width(70.0).font(egui::FontId::monospace(FONT_SM)).hint_text("148.00"));
            });

            // Notes
            ui.add(egui::TextEdit::multiline(&mut watchlist.play_editor_notes)
                .desired_rows(2).desired_width(ui.available_width())
                .font(egui::FontId::monospace(FONT_SM)).hint_text("Strategy notes..."));
            ui.add_space(GAP_SM);

            // Buttons
            ui.horizontal(|ui| {
                let can_create = !watchlist.play_editor_symbol.trim().is_empty()
                    && watchlist.play_editor_entry.parse::<f32>().is_ok();
                if action_btn(ui, "Create Play", t.accent, can_create) {
                    let entry = watchlist.play_editor_entry.parse::<f32>().unwrap_or(0.0);
                    let target = watchlist.play_editor_target.parse::<f32>().unwrap_or(entry * 1.02);
                    let stop = watchlist.play_editor_stop.parse::<f32>().unwrap_or(entry * 0.98);
                    let mut play = Play::new(
                        watchlist.play_editor_symbol.trim(),
                        watchlist.play_editor_direction, entry, target, stop);
                    play.notes = watchlist.play_editor_notes.trim().to_string();
                    watchlist.plays.push(play);
                    watchlist.play_editor_symbol.clear();
                    watchlist.play_editor_entry.clear();
                    watchlist.play_editor_target.clear();
                    watchlist.play_editor_stop.clear();
                    watchlist.play_editor_notes.clear();
                    watchlist.play_editor_open = false;
                }
                if small_action_btn(ui, "Cancel", t.dim) {
                    watchlist.play_editor_open = false;
                }
            });
        });
}
