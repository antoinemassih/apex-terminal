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
    use crate::ui_kit::layout::{Flex, Item};

    let mut clicked: Option<String> = None;
    let avail = ui.available_size_before_wrap();
    let h = avail.y.max(20.0);
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail.x, h), Sense::hover());
    let painter = ui.painter_at(rect);

    let sym_font = mono_sm();
    let px_font  = crate::ui_kit::style::mono_xs();
    let chg_font = crate::ui_kit::style::mono_xs();

    let mut cx = rect.left() + gap_sm();
    for e in entries {
        let px_s = format!("{:.2}", e.price);
        let chg_col = if e.change_pct >= 0.0 { t.bull } else { t.bear };
        let chg_s = format!("{}{:.2}%", if e.change_pct >= 0.0 { "+" } else { "" }, e.change_pct);

        // One quote = `SYM · price · +chg%`, three content-sized items with
        // their own seam tokens. This was three `cx += galley.width + gap`
        // steps; the flex row states the same layout as a shape instead of a
        // sequence of mutations, which is what makes the width below knowable
        // BEFORE anything is painted.
        let quote = Flex::row()
            .slot("sym", Item::text(ui, e.symbol.clone(), sym_font.clone()))
            .slot("px",  Item::text(ui, px_s.clone(), px_font.clone()).margin_start(gap_sm()))
            .slot("chg", Item::text(ui, chg_s.clone(), chg_font.clone()).margin_start(gap_sm()));

        // Measure the quote and stop when it would not FIT — the previous test
        // was `if cx > rect.right() - 40.0 { break }`, a fixed guess unrelated
        // to the quote about to be drawn. A quote wider than 40px started
        // inside the strip and ran past its right edge, where `painter_at`
        // clipped it mid-glyph. The strip's own width is now the test.
        let quote_w = quote.solve_in(Rect::from_min_size(
            egui::pos2(0.0, 0.0), Vec2::new(f32::INFINITY, h),
        )).rect("chg").right();
        if cx + quote_w > rect.right() { break; }

        let solved = quote.solve_in(Rect::from_min_size(
            egui::pos2(cx, rect.top()), Vec2::new(quote_w, h),
        ));
        let cy = rect.center().y;
        let put = |r: Rect, text: &str, font: FontId, col: Color32| {
            painter.text(egui::pos2(r.left(), cy), Align2::LEFT_CENTER, text, font, col);
        };
        put(solved.rect("sym"), &e.symbol, sym_font.clone(), t.text);
        put(solved.rect("px"),  &px_s,     px_font.clone(),  color_subtle(t.dim));
        put(solved.rect("chg"), &chg_s,    chg_font.clone(), chg_col);

        // Click-to-load spans the whole quote — exactly the solved extent, no
        // half-gap fudge (`cx - gap_md() * 0.5`) needed.
        let quote_rect = Rect::from_min_max(
            egui::pos2(cx, rect.top()),
            egui::pos2(cx + quote_w, rect.bottom()),
        );
        let qresp = ui.interact(quote_rect, ui.id().with(("ticker", &e.symbol)), Sense::click());
        if qresp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        if qresp.clicked() { clicked = Some(e.symbol.clone()); }

        cx += quote_w + gap_md();
    }
    let _ = font_sm;
    TickerStripResponse { clicked_symbol: clicked }
}
