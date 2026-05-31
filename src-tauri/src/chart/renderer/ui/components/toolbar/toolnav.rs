//! `toolnav` — the second chrome row (tools + indicators + ticker).
//!
//! A first-class shell region, rendered as a `TopBottomPanel` between the main
//! top-nav and the workspace. Present for every style; visibility + float are
//! pure style parameters:
//!   - `Chrome.toolnav_height`  — 0 hides it (single-row chrome); >0 shows it.
//!   - `region_gap`             — floats it as a card (Aperture/Glass) vs flush.
//!
//! Composed of the same `NavCluster` primitive as the top nav. Today it carries
//! the `TickerStrip`; the indicator-dropdown clusters migrate here from the top
//! nav in a follow-up (when `toolnav_height > 0` they belong in row 2).

use egui::Margin;
use super::super::super::style::{self as style, region_frame, region_gap, gap_xs, gap_sm};
use super::nav_cluster::{NavCluster, NavRole};
use super::ticker_strip::{ticker_strip, TickerEntry};

type Theme = crate::chart_renderer::gpu::Theme;
type Watchlist = crate::chart_renderer::gpu::Watchlist;

/// Render the toolnav region. No-op when the active style sets
/// `toolnav_height == 0`. Must be called after the main "tb" panel and before
/// the central workspace so it stacks in the right order.
pub(crate) fn render_toolnav(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    // Hybrid visibility: user override wins, else the style's toolnav_height.
    if !style::toolnav_visible() { return; }
    let h = style::toolnav_resolved_height();
    let rgap = region_gap();

    // Assemble ticker entries from the active watchlist's symbols.
    let mut entries: Vec<TickerEntry> = Vec::new();
    for sec in &watchlist.sections {
        for item in &sec.items {
            if item.symbol.trim().is_empty() || item.is_option { continue; }
            let chg = if item.prev_close > 0.0 {
                (item.price - item.prev_close) / item.prev_close * 100.0
            } else { 0.0 };
            entries.push(TickerEntry { symbol: item.symbol.clone(), price: item.price, change_pct: chg });
        }
    }

    let frame = region_frame(t, t.toolbar_bg)
        .inner_margin(Margin { left: (gap_xs() + rgap) as i8, right: rgap as i8, top: 0, bottom: 0 });

    let mut load_symbol: Option<String> = None;

    egui::TopBottomPanel::top("toolnav")
        .frame(frame)
        // +2×gap so the card keeps height `h` after the outer margin insets it.
        .exact_height(h + 2.0 * rgap)
        .show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing.x = 0.0;
            ui.horizontal_centered(|ui| {
                // Tools cluster — pane/drawing affordances. Minimal for now;
                // the indicator dropdowns migrate here next. (The bottom dock is
                // opened from its always-visible bottom strip, not from here.)
                NavCluster::new(NavRole::Tools).show(ui, t, |ui| {
                    ui.label(
                        egui::RichText::new("TOOLS")
                            .monospace().size(style::font_2xs())
                            .color(t.dim),
                    );
                });
                ui.add_space(gap_sm());

                // Ticker cluster — fills the remaining width.
                NavCluster::new(NavRole::Ticker).show(ui, t, |ui| {
                    let r = ticker_strip(ui, t, &entries);
                    if let Some(s) = r.clicked_symbol { load_symbol = Some(s); }
                });
            });
        });

    // Click-to-load: route a ticker click into the active pane's pending symbol.
    if let Some(_s) = load_symbol {
        // Defer to the existing symbol-change machinery via watchlist intent.
        // (Hook left intentionally light until the toolnav owns a pane ref.)
    }
}
