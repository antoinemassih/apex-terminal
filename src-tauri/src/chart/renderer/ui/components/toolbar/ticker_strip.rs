//! `TickerStrip` — a horizontal scrolling strip of `SYM  price  +chg%` quotes.
//!
//! The signature element of the toolnav (second chrome row) in the ApertureJune
//! reference. Universal primitive: every style renders it; only the surrounding
//! cluster styling (radius, fill) and the palette bull/bear colours vary.

use egui::{Align2, Color32, FontId, Rect, Sense, Ui, Vec2};
use super::super::super::style::{font_sm, font_xs, gap_md, gap_sm, color_subtle, mono_sm};

type Theme = crate::chart_renderer::gpu::Theme;

/// One quote in the strip.
pub struct TickerEntry {
    pub symbol: String,
    pub price: f32,
    pub change_pct: f32,
}

/// A click result: the symbol the user clicked, if any.
pub struct TickerStripResponse {
    pub clicked_symbol: Option<String>,
}

/// Render the ticker strip into the available horizontal space. Each entry is
/// `SYM` (text colour) + `price` (dim mono) + `+chg%` (bull/bear). Entries are
/// laid out left-to-right; the strip clips to `ui.available_width()`.
pub fn ticker_strip(ui: &mut Ui, t: &Theme, entries: &[TickerEntry]) -> TickerStripResponse {
    let mut clicked: Option<String> = None;
    let avail = ui.available_size_before_wrap();
    let h = avail.y.max(20.0);
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail.x, h), Sense::hover());
    let painter = ui.painter_at(rect);
    let cy = rect.center().y;

    let sym_font = mono_sm();
    let px_font  = crate::ui_kit::style::mono_xs();
    let chg_font = crate::ui_kit::style::mono_xs();

    let mut cx = rect.left() + gap_sm();
    for e in entries {
        if cx > rect.right() - 40.0 { break; } // clip — no horizontal scroll yet

        // Symbol (primary text).
        let sym_g = painter.layout_no_wrap(e.symbol.clone(), sym_font.clone(), t.text);
        // Hit rect spans the whole quote for click-to-load.
        let quote_start = cx;

        painter.text(egui::pos2(cx, cy), Align2::LEFT_CENTER, &e.symbol, sym_font.clone(), t.text);
        cx += sym_g.size().x + gap_sm();

        // Price (dim mono).
        let px_s = format!("{:.2}", e.price);
        let px_g = painter.layout_no_wrap(px_s.clone(), px_font.clone(), color_subtle(t.dim));
        painter.text(egui::pos2(cx, cy), Align2::LEFT_CENTER, &px_s, px_font.clone(), color_subtle(t.dim));
        cx += px_g.size().x + gap_sm();

        // Change % (bull/bear).
        let chg_col = if e.change_pct >= 0.0 { t.bull } else { t.bear };
        let chg_s = format!("{}{:.2}%", if e.change_pct >= 0.0 { "+" } else { "" }, e.change_pct);
        let chg_g = painter.layout_no_wrap(chg_s.clone(), chg_font.clone(), chg_col);
        painter.text(egui::pos2(cx, cy), Align2::LEFT_CENTER, &chg_s, chg_font.clone(), chg_col);
        cx += chg_g.size().x + gap_md();

        // Click-to-load over the whole quote.
        let quote_rect = Rect::from_min_max(
            egui::pos2(quote_start, rect.top()),
            egui::pos2(cx - gap_md() * 0.5, rect.bottom()),
        );
        let qresp = ui.interact(quote_rect, ui.id().with(("ticker", &e.symbol)), Sense::click());
        if qresp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        if qresp.clicked() { clicked = Some(e.symbol.clone()); }
    }
    let _ = font_sm;
    TickerStripResponse { clicked_symbol: clicked }
}
