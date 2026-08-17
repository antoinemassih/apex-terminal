//! `toolnav` — the second chrome row (tools + alert feed).
//!
//! A first-class shell region, rendered as a `TopBottomPanel` between the main
//! top-nav and the workspace. Visibility gated by `style::toolnav_visible()`.
//!
//! Layout:
//!   LEFT  — chart controls (interval, drawing, object-tree, indicators,
//!            widgets, magnet, hit-alert), relocated here from the top-nav row.
//!   RIGHT — alert badge feed, starting at 40 % of the toolbar width and
//!            running to the right edge. Displays order fills, price alerts,
//!            signals, and errors as dismissible coloured pills.
//!
//! The ORDER button has moved to the top-nav right cluster (left of Search).
//! When the toolnav is toggled off, the chart controls do NOT reappear in the
//! top-nav row — they are toolbar-only.

use egui::Margin;
use super::super::super::style::{self as style, region_gap, gap_xs, gap_sm};

type Theme    = crate::chart_renderer::gpu::Theme;
type Watchlist = crate::chart_renderer::gpu::Watchlist;
type Chart     = crate::chart_renderer::gpu::Chart;

/// Render the toolnav region. No-op when toggled off. Must be called after the
/// main "tb" panel and before the central workspace.
pub(crate) fn render_toolnav(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !style::toolnav_visible() { return; }
    let h    = style::toolnav_resolved_height();
    let rgap = region_gap();

    // Padding only; the card itself is painted inside `show`. See the same
    // change on the "tb" panel in top_nav.rs.
    let frame = egui::Frame::NONE.inner_margin(Margin {
        left: (gap_xs() + rgap) as i8, right: rgap as i8,
        top: rgap as i8, bottom: rgap as i8,
    });

    egui::TopBottomPanel::top("toolnav")
        .frame(frame)
        // The CARD height. `region_frame`'s `outer_margin` adds the surrounding
        // `rgap` outside this box — adding it here too spent the gap twice.
        // See the note on the "tb" panel in top_nav.rs.
        .exact_height(h + 2.0 * rgap)
        .show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing.x = gap_sm();
            let tb_rect = ui.max_rect();
            {
                let sw = ctx.screen_rect().width();
                let card = style::region_card_rect(tb_rect, sw);
                style::paint_region_card_filled(ui.painter(), card, t, t.toolbar_bg);
            }
            ui.horizontal_centered(|ui| {
                // ── Left: chart controls (interval / drawing / object-tree /
                //    indicators / widgets / alt-bar settings / magnet / hit). ──
                super::top_nav::render_chart_controls(ui, watchlist, panes, ap, t, tb_rect);

                // ── Ticker strip: fills the gap between the controls and the
                //    alert feed. The Aperture reference puts `SYM price +chg%`
                //    quotes here, and the widget existed, fully styled, with
                //    nothing rendering it (AT-158).
                //
                //    It takes only the space that was previously `add_space`,
                //    so the alert feed still starts at the same 40 % mark and
                //    the controls are untouched. The strip draws as many WHOLE
                //    quotes as fit and stops — it does not clip one mid-glyph.
                let cursor_x   = ui.cursor().left();
                let target_x   = tb_rect.left() + tb_rect.width() * 0.40;
                if cursor_x < target_x {
                    let gap_w = target_x - cursor_x;
                    let entries = ticker_entries(watchlist);
                    if entries.is_empty() {
                        ui.add_space(gap_w);
                    } else {
                        let resp = ui.allocate_ui_with_layout(
                            egui::vec2(gap_w, ui.available_height()),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| super::ticker_strip::ticker_strip(ui, t, &entries),
                        ).inner;
                        if let Some(sym) = resp.clicked_symbol {
                            // Same path the command palette uses for `sym:` —
                            // the centralized publisher in `App::about_to_wait`
                            // consumes `pending_symbol_change`.
                            let tf = panes[ap].timeframe.clone();
                            panes[ap].symbol = sym.clone();
                            panes[ap].symbol_meta = crate::foundation::types::symbol_or_guess(&sym);
                            panes[ap].pending_symbol_change = Some(sym.clone());
                            crate::chart_renderer::gpu::fetch_bars_background(sym, tf, 0);
                        }
                    }
                }
                super::alert_feed::render_badge_feed(ui, t);
            });
        });
}

/// Loaded watchlist items as ticker quotes.
///
/// Only `loaded` items with a usable `prev_close` are included — a quote whose
/// change% cannot be computed would render `+0.00%` and read as "flat" rather
/// than "unknown", which is fabricated data, not a placeholder.
fn ticker_entries(wl: &Watchlist) -> Vec<super::ticker_strip::TickerEntry> {
    wl.sections
        .iter()
        .flat_map(|sec| sec.items.iter())
        .filter(|i| i.loaded && i.prev_close > 0.0)
        .map(|i| super::ticker_strip::TickerEntry {
            symbol: i.symbol.clone(),
            price: i.price,
            change_pct: (i.price - i.prev_close) / i.prev_close * 100.0,
        })
        .collect()
}
