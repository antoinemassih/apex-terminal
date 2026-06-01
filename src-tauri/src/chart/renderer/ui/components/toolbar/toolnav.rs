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

    let frame = region_frame(t, t.toolbar_bg)
        .inner_margin(Margin { left: (gap_xs() + rgap) as i8, right: rgap as i8, top: 0, bottom: 0 });

    egui::TopBottomPanel::top("toolnav")
        .frame(frame)
        .exact_height(h + 2.0 * rgap)
        .show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing.x = gap_sm();
            let tb_rect = ui.max_rect();
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
