//! TradeCard — closed trade summary card for the journal.
//!
//! Shows symbol + side + entry→exit prices + P&L + setup tag + duration +
//! R-multiple + timeframe in a compact card with a left P&L accent stripe.
//! Hover triggers a hand cursor and a slightly brighter background fill.
//!
//! ```ignore
//! TradeCard::new(&entry).show(ui, theme);
//! ```

use egui::{Response, Ui};
use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;

// ── Data — mirrors JournalEntry fields used by draw_card ─────────────────────

pub struct TradeCardData<'a> {
    pub symbol: &'a str,
    pub side: &'a str,          // "Long" | "Short"
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub setup_type: &'a str,
    pub duration_mins: i64,
    pub r_multiple: f64,
    pub timeframe: &'a str,
    pub notes: &'a str,
}

// ── Widget ────────────────────────────────────────────────────────────────────

#[must_use = "TradeCard does nothing until `.show(ui, theme)` is called"]
pub struct TradeCard<'a> {
    data: &'a TradeCardData<'a>,
}

impl<'a> TradeCard<'a> {
    pub fn new(data: &'a TradeCardData<'a>) -> Self {
        Self { data }
    }

    /// Render the card. Mirrors draw_card() from journal_panel.rs:209-265 exactly.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        use st::{
            color_alpha, color_subtle, color_half, color_dim,
            alpha_subtle, radius_sm, gap_xs,
            font_sm, font_xs,
        };

        let entry = self.data;
        let card_w = ui.available_width();
        let card_h = if entry.notes.is_empty() { 52.0_f32 } else { 66.0_f32 };
        let (card_rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
        let p = ui.painter();

        let is_win = entry.pnl > 0.0;
        let pnl_col = if is_win { theme.bull() } else { theme.bear() };
        let dir_col = if entry.side == "Long" { theme.bull() } else { theme.bear() };

        let bg = if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            color_alpha(theme.border(), alpha_subtle())
        } else {
            color_alpha(theme.border(), 8)
        };
        p.rect_filled(card_rect, radius_sm(), bg);

        // Left P&L accent stripe
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(card_rect.left(), card_rect.top() + 3.0),
                egui::pos2(card_rect.left() + 3.0, card_rect.bottom() - 3.0),
            ),
            1.0, pnl_col);

        let cx = card_rect.left() + 8.0;
        let mut cy = card_rect.top() + 8.0;

        // Row 1: symbol · side · P&L
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            entry.symbol, egui::FontId::monospace(font_sm()), theme.text());
        p.text(egui::pos2(cx + 50.0, cy + 4.0), egui::Align2::LEFT_CENTER,
            entry.side, egui::FontId::monospace(font_xs()), dir_col);
        let sign = if entry.pnl >= 0.0 { "+" } else { "" };
        p.text(egui::pos2(card_rect.right() - 8.0, cy + 4.0), egui::Align2::RIGHT_CENTER,
            &format!("{}${:.0} ({:+.1}%)", sign, entry.pnl, entry.pnl_pct),
            egui::FontId::monospace(font_sm()), pnl_col);
        cy += 16.0;

        // Row 2: setup · duration · R-multiple
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            entry.setup_type, st::mono_sm(), color_subtle(theme.accent()));
        let dur = if entry.duration_mins >= 1440 {
            format!("{:.0}d", entry.duration_mins as f64 / 1440.0)
        } else if entry.duration_mins >= 60 {
            format!("{:.0}h", entry.duration_mins as f64 / 60.0)
        } else {
            format!("{}m", entry.duration_mins)
        };
        p.text(egui::pos2(cx + 60.0, cy + 4.0), egui::Align2::LEFT_CENTER,
            &dur, st::mono_sm(), color_half(theme.dim()));
        let r_col = if entry.r_multiple > 0.0 { theme.bull() } else { theme.bear() };
        p.text(egui::pos2(cx + 90.0, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("{:+.1}R", entry.r_multiple), st::mono_sm(), r_col);
        cy += 14.0;

        // Row 3: entry→exit prices · timeframe
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("{:.2} \u{2192} {:.2}", entry.entry_price, entry.exit_price),
            st::mono_sm(), color_dim(theme.dim()));
        p.text(egui::pos2(card_rect.right() - 8.0, cy + 4.0), egui::Align2::RIGHT_CENTER,
            entry.timeframe, st::mono_sm(), color_dim(theme.dim()));

        // Optional notes row
        if !entry.notes.is_empty() {
            cy += 14.0;
            p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
                entry.notes, st::mono_sm(), theme.dim().gamma_multiply(0.35));
        }

        ui.add_space(gap_xs());
        resp
    }
}
