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
use super::super::super::style::{self as style, region_frame, region_gap, gap_xs, gap_sm};

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

                // ── Alert feed: occupies from 40 % of the toolbar width to the
                //    right edge, with a minimum gap so it never collides with
                //    the controls even when they are wide. ──
                let cursor_x   = ui.cursor().left();
                let target_x   = tb_rect.left() + tb_rect.width() * 0.40;
                if cursor_x < target_x {
                    ui.add_space(target_x - cursor_x);
                }
                super::alert_feed::render_badge_feed(ui, t);
            });
        });
}
