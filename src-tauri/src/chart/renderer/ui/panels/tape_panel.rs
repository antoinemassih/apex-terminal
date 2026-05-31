//! Time & Sales panel — real-time trade tape display.
//!
//! Migrated to the canonical ui_kit side-panel primitives. The outer chrome
//! uses [`SidePanelShell`]; the body groups the column headers and scrolling
//! print list under a [`PanelSection`].
//!
//! Row primitive: `PanelListRow::columns(&[...])` (Wave 10b migration).
//! The T&S tape is a high-cadence streaming list (up to 200 visible
//! prints, refreshed every frame on live ticks); `PanelListRow.columns`
//! uses a borrowed `&[Column]` slice with no per-cell allocations, and
//! `.row_tint(side_color, 12)` paints the buy/sell directional fill
//! BEHIND any hover/selected backgrounds — matching the legacy
//! `ListRow.row_tint` look exactly.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, TapeRow, Theme};
use super::super::components::text::MonospaceCode;
use crate::ui_kit::widgets::{PanelColAlign, PanelColumn, PanelEmpty, PanelListRow, PanelSection, Skeleton};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};


/// Draw the T&S content into `ui` (used by analysis_panel as a tab).
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    let panel_w = ui.available_width();
    let section_title = format!("TIME & SALES — {}", active_symbol);

    PanelSection::new(&section_title).show(ui, t, |ui, t| {
        // Column headers
        ui.horizontal(|ui| {
            ui.add_space(gap_xs());
            let hw = (panel_w - 12.0) / 3.0;
            let hdr_color = color_muted(t.dim);
            col_header(ui, "TIME",  hw, hdr_color, false);
            col_header(ui, "PRICE", hw, hdr_color, true);
            col_header(ui, "SIZE",  hw, hdr_color, true);
        });
        separator(ui, t.toolbar_border);

        // Trade rows
        let row_h = 14.0;
        egui::ScrollArea::vertical()
            .id_salt("tape_scroll")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_min_width(panel_w - 4.0);
                let entries: Vec<&TapeRow> = watchlist.tape_entries.iter()
                    .filter(|e| e.symbol == active_symbol)
                    .collect();

                if entries.is_empty() {
                    ui.add_space(gap_sm());
                    let cw = (panel_w - 12.0) / 3.0;
                    for _ in 0..5 {
                        ui.add_space(gap_2xs());
                        ui.horizontal(|ui| {
                            ui.add_space(gap_2xs());
                            Skeleton::text(cw * 0.7).show(ui, t);
                            ui.add_space(cw - cw * 0.7);
                            Skeleton::text(cw * 0.7).show(ui, t);
                            ui.add_space(cw - cw * 0.7);
                            Skeleton::text(cw * 0.6).show(ui, t);
                        });
                    }
                    ui.add_space(gap_sm());
                    // Wave 9c: prefer registry-backed asset class (avoids
                    // mis-classifying real equities whose ticker happens to
                    // end in USDT); fall back to the legacy string heuristic
                    // for tickers the registry has never seen.
                    let active_is_crypto = crate::foundation::types::registry()
                        .get(active_symbol)
                        .map(|s| s.is_crypto())
                        .unwrap_or_else(|| crate::data::is_crypto(active_symbol));
                    if !active_is_crypto && !crate::apex_data::is_enabled() {
                        PanelEmpty::new("No prints yet")
                            .hint("Enable ApexData in settings for stock T&S")
                            .show(ui, t);
                    }
                }

                let dim_color = color_muted(t.dim);
                let qty_color = t.text;
                for (idx, entry) in entries.iter().rev().take(200).collect::<Vec<_>>().into_iter().rev().enumerate() {
                    let side_color = if entry.is_buy { COLOR_PROFIT_GREEN } else { COLOR_LOSS_RED };

                    // Build strings before the row to avoid borrow issues.
                    let secs = entry.time / 1000;
                    let h = (secs / 3600) % 24;
                    let m = (secs / 60) % 60;
                    let s = secs % 60;
                    let time_str = format!("{:02}:{:02}:{:02}", h, m, s);
                    let price_str = if entry.price >= 100.0 {
                        format!("{:.2}", entry.price)
                    } else if entry.price >= 1.0 {
                        format!("{:.4}", entry.price)
                    } else {
                        format!("{:.6}", entry.price)
                    };
                    let qty_str = if entry.qty >= 1.0 {
                        format!("{:.4}", entry.qty)
                    } else {
                        format!("{:.6}", entry.qty)
                    };

                    // Wave 10b: PanelListRow.columns + .row_tint replaces the
                    // legacy ListRow. The id_salt borrows the entry's index;
                    // the columns slice is stack-built (no heap alloc per row
                    // beyond the format!() strings, which the legacy also did).
                    let id_salt = format!("tape_row_{idx}");
                    PanelListRow::new(&id_salt)
                        .height(row_h)
                        .hoverable(false)
                        .row_tint(side_color, 12)
                        .columns(&[
                            PanelColumn { text: &time_str,  align: PanelColAlign::Left,  color: dim_color, weight: 1.0, min_width: None, mono: true },
                            PanelColumn { text: &price_str, align: PanelColAlign::Right, color: side_color, weight: 1.0, min_width: None, mono: true },
                            PanelColumn { text: &qty_str,   align: PanelColAlign::Right, color: qty_color, weight: 1.0, min_width: None, mono: true },
                        ])
                        .show(ui, t);
                }
            });
    });
}

/// Rail registration — see [`super::right_rail`].
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "tape",
    is_open: |w| w.tape_open,
    render: |cx, slot| draw(cx.ctx, cx.watchlist, &cx.symbol, cx.t, Some(slot)),
};

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme, slot: Option<super::side_panel_shell::RailSlot>) {
    if !watchlist.tape_open { return; }

    let resp = SidePanelShell::new("time_and_sales", "TIME & SALES")
        .width(Width::Narrow)
        .resizable(180.0..=350.0)
        .rail_slot(slot)
        .show(ctx, t, |ui, t| {
            // Symbol breadcrumb under the chrome header
            ui.horizontal(|ui| {
                ui.add(MonospaceCode::new(active_symbol).size_px(9.0).strong(true).color(t.accent));
            });
            draw_content(ui, watchlist, active_symbol, t);
        });
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.tape_open = false); }
}
