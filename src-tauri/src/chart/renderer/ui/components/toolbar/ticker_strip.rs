//! `TickerStrip` — a horizontal scrolling strip of `SYM  price  +chg%` quotes.
//!
//! The signature element of the toolnav (second chrome row) in the ApertureJune
//! reference. Universal primitive: every style renders it; only the surrounding
//! cluster styling (radius, fill) and the palette bull/bear colours vary.

use egui::{Color32, Rect, Sense, Ui, Vec2};
use super::super::super::style::{font_sm, gap_md, gap_sm, color_subtle, mono_sm};

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
    use crate::ui_kit::cascade::El;

    let mut clicked: Option<String> = None;
    let avail = ui.available_size_before_wrap();
    let h = avail.y.max(20.0);
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail.x, h), Sense::hover());

    let sym_font = mono_sm();
    let px_font  = crate::ui_kit::style::mono_xs();
    let chg_font = crate::ui_kit::style::mono_xs();

    // MEASURE every quote, then DECLARE the prefix that fits.
    //
    // The strip used to pack with `cx += quote_w + gap_md()`, which made the
    // seam a term in an accumulator: it appeared in the fit test and again in
    // the advance, free to disagree. Splitting the two phases fixes that and
    // is also what the problem actually is — "how many fit" is a measurement,
    // "where they go" is a layout, and only the second belongs in a tree.
    let strip_left = rect.left() + gap_sm();
    struct Quote<'q> {
        w: f32,
        e: &'q TickerEntry,
        px_s: String,
        chg_s: String,
        chg_col: Color32,
    }

    // One quote = `SYM · price · +chg%` — three text nodes that MEASURE AND
    // PAINT THEMSELVES.
    //
    // Built twice, once to measure and once to paint, because `El` consumes
    // itself on show and a quote is three `String`s and three `FontId`s — a
    // cheap closure, and cheaper than keeping a second representation of the
    // same row around to be told to agree with the first.
    let quote_el = |e: &TickerEntry, px_s: &str, chg_s: &str, chg_col: Color32| {
        El::row()
            .child(El::text_with_font(e.symbol.clone(), sym_font.clone()).color(t.text))
            .child(
                El::text_with_font(px_s.to_string(), px_font.clone())
                    .color(color_subtle(t.dim))
                    .margin_start(gap_sm()),
            )
            .child(
                El::text_with_font(chg_s.to_string(), chg_font.clone())
                    .color(chg_col)
                    .margin_start(gap_sm()),
            )
    };

    let measured: Vec<Quote> = entries
        .iter()
        .map(|e| {
            let px_s = format!("{:.2}", e.price);
            let chg_col = if e.change_pct >= 0.0 { t.bull } else { t.bear };
            let chg_s =
                format!("{}{:.2}%", if e.change_pct >= 0.0 { "+" } else { "" }, e.change_pct);
            // The quote's INTRINSIC extent, asked of the tree directly. The
            // old fit test was `cx > rect.right() - 40.0`, a fixed guess
            // unrelated to the quote about to be drawn: anything wider than
            // 40px started inside the strip and ran past its right edge, where
            // `painter_at` clipped it mid-glyph.
            let w = quote_el(e, &px_s, &chg_s, chg_col).intrinsic_width(ui);
            Quote { w, e, px_s, chg_s, chg_col }
        })
        .collect();

    // How many fit, counting the seams the declared row will insert.
    let fitting = measured
        .iter()
        .scan(strip_left, |pen, q| {
            let end = *pen + q.w;
            *pen = end + gap_md();
            Some(end)
        })
        .take_while(|end| *end <= rect.right())
        .count();

    let strip = measured
        .iter()
        .take(fitting)
        .enumerate()
        .fold(El::row().gap(gap_md()), |el, (i, q)| {
            el.child(El::slot(format!("q{i}"), Vec2::new(q.w, h)))
        })
        .solve_rect(Rect::from_min_max(
            egui::pos2(strip_left, rect.top()),
            rect.max,
        ));

    // `Theme` IS a `ComponentTheme` — `theme_impl.rs` is the chart app's bridge.
    let theme = t;
    for (i, q) in measured.iter().take(fitting).enumerate() {
        let slot = strip.rect(&format!("q{i}"));
        quote_el(q.e, &q.px_s, &q.chg_s, q.chg_col).show_in(
            ui,
            theme,
            Rect::from_min_size(slot.min, Vec2::new(q.w, h)),
        );

        // Click-to-load spans the whole quote — exactly the solved extent, no
        // half-gap fudge (`cx - gap_md() * 0.5`) needed.
        let qresp = ui.interact(
            Rect::from_min_max(slot.min, egui::pos2(slot.right(), rect.bottom())),
            ui.id().with(("ticker", &q.e.symbol)),
            Sense::click(),
        );
        if qresp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if qresp.clicked() {
            clicked = Some(q.e.symbol.clone());
        }
    }

    let _ = font_sm;
    TickerStripResponse { clicked_symbol: clicked }
}
