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
use super::CardVariant;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::text_style::TextStyle;

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
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
        use st::{
            color_alpha, color_subtle, color_half, color_dim,
            alpha_subtle, radius_sm, gap_xs,
        };

        let entry = self.data;
        let card_w = ui.available_width();
        let card_h = if entry.notes.is_empty() { 52.0_f32 } else { 66.0_f32 };
        let (card_rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
        let p = ui.painter();
        // Cascade-aware mono tiers (== mono_sm / mono_xs today, subtree-overridable).
        let f_mono_sm = TextStyle::MonoSm.font_id_in(ui);
        let f_mono_xs = TextStyle::MonoXs.font_id_in(ui);

        let pal = palette_ct(theme);
        let is_win = entry.pnl > 0.0;
        let pnl_col = if is_win { pal.base(Tone::Bull) } else { pal.base(Tone::Bear) };
        let dir_col = if entry.side == "Long" { pal.base(Tone::Bull) } else { pal.base(Tone::Bear) };

        // REUSES the `card` key — a TradeCard is a card. PanelCard already
        // resolves it, so both surfaces move together when a style restyles
        // cards instead of one of them being forgotten.
        let base_fill = if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            color_alpha(CardVariant::Elevated.fill_color(theme), alpha_subtle())
        } else {
            CardVariant::Elevated.fill_color(theme)
        };
        let (card_cr, card_fill, card_stroke) = super::theme::resolve_control_chrome(
            ui.ctx(), theme, "card",
            radius_sm(), base_fill,
            CardVariant::Elevated.border_color(theme), st::stroke_thin(),
        );
        p.rect_filled(card_rect, card_cr, card_fill);
        p.rect_stroke(card_rect, card_cr, card_stroke, egui::StrokeKind::Outside);

        // Left P&L accent stripe
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(card_rect.left(), card_rect.top() + 3.0),
                egui::pos2(card_rect.left() + 3.0, card_rect.bottom() - 3.0),
            ),
            st::radius_pill(), pnl_col);

        let cx = card_rect.left() + 8.0;
        let mut cy = card_rect.top() + 8.0;

        // Row 1: symbol · side · P&L
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            entry.symbol, f_mono_sm.clone(), pal.base(Tone::Text));
        p.text(egui::pos2(cx + 50.0, cy + 4.0), egui::Align2::LEFT_CENTER,
            entry.side, f_mono_xs.clone(), dir_col);
        let sign = if entry.pnl >= 0.0 { "+" } else { "" };
        p.text(egui::pos2(card_rect.right() - 8.0, cy + 4.0), egui::Align2::RIGHT_CENTER,
            &format!("{}${:.0} ({:+.1}%)", sign, entry.pnl, entry.pnl_pct),
            f_mono_sm.clone(), pnl_col);
        cy += 16.0;

        // Row 2: setup · duration · R-multiple
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            entry.setup_type, f_mono_sm.clone(), color_subtle(pal.base(Tone::Accent)));
        let dur = if entry.duration_mins >= 1440 {
            format!("{:.0}d", entry.duration_mins as f64 / 1440.0)
        } else if entry.duration_mins >= 60 {
            format!("{:.0}h", entry.duration_mins as f64 / 60.0)
        } else {
            format!("{}m", entry.duration_mins)
        };
        p.text(egui::pos2(cx + 60.0, cy + 4.0), egui::Align2::LEFT_CENTER,
            &dur, f_mono_sm.clone(), color_half(pal.base(Tone::Dim)));
        let r_col = if entry.r_multiple > 0.0 { pal.base(Tone::Bull) } else { pal.base(Tone::Bear) };
        p.text(egui::pos2(cx + 90.0, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("{:+.1}R", entry.r_multiple), f_mono_sm.clone(), r_col);
        cy += 14.0;

        // Row 3: entry→exit prices · timeframe
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("{:.2} \u{2192} {:.2}", entry.entry_price, entry.exit_price),
            f_mono_sm.clone(), color_dim(pal.base(Tone::Dim)));
        p.text(egui::pos2(card_rect.right() - 8.0, cy + 4.0), egui::Align2::RIGHT_CENTER,
            entry.timeframe, f_mono_sm.clone(), color_dim(pal.base(Tone::Dim)));

        // Optional notes row
        if !entry.notes.is_empty() {
            cy += 14.0;
            p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
                entry.notes, f_mono_sm.clone(), st::color_dim(pal.base(Tone::Dim)));
        }

        ui.add_space(gap_xs());
        resp
    }
}

impl<'a> egui::Widget for TradeCard<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
