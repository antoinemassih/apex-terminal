//! Alerts management panel — shows all active and triggered alerts with controls.
//!
//! Migrated to the panel kit (PanelSection, PanelEmpty, PanelInputRow,
//! PanelDualAction, Stat). Section boilerplate (header + RTL action + separator)
//! collapses to a single `PanelSection::new(...).action(...).show(ui, t, |ui|...)`
//! call, and the Above/Below buttons route through PanelDualAction.

use egui;
use super::super::style::*;
use super::super::widgets as widgets;
use crate::ui_kit::widgets::Input;
use crate::ui_kit::widgets::tokens::Size as KitSize;
use super::super::widgets::rows::alert_row::{AlertRow, AlertCmp};
use super::super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::trading::PriceAlert;
use crate::chart_renderer::commands::{AppCommand, UiCtx};
use super::kit::{PanelHeader, PanelSection, PanelEmpty, PanelInputRow, PanelDualAction, Tone};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.alerts_panel_open { return; }

    let cx = UiCtx::new(t);
    egui::SidePanel::right("alerts_panel")
        .default_width(240.0)
        .min_width(180.0)
        .max_width(300.0)
        .frame(widgets::frames::PanelFrame::new(cx.toolbar_bg, cx.toolbar_border).theme(&cx).zero_margin().build())
        .show(ctx, |ui| {
            let close_clicked = PanelHeader::new("ALERTS")
                .icon(Icon::BELL)
                .watchlist(watchlist)
                .show(ui, t);
            if close_clicked {
                watchlist.alerts_panel_open = false;
            }
            ui.add_space(gap_md());
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left:   gap_lg() as i8,
                    right:  gap_lg() as i8,
                    top:    0,
                    bottom: gap_lg() as i8,
                })
                .show(ui, |ui| {
                    draw_content_cx(ui, watchlist, panes, ap, &cx);
                });
        });
}

/// Tab body content (no SidePanel wrapper, no header). Used by signals_panel.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    let cx = UiCtx::new(t);
    draw_content_cx(ui, watchlist, panes, ap, &cx);
}

fn draw_content_cx(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    cx: &UiCtx<'_>,
) {
    let t = cx.theme;

    // ── Add Alert ──
    let current_price = panes[ap].bars.last().map(|b| b.close).unwrap_or(0.0);
    let sym = panes[ap].symbol.clone();
    let meta = format!("{} @ {:.2}", sym, current_price);

    PanelSection::new("ADD ALERT")
        .meta(meta)
        .show(ui, t, |ui| {
            PanelInputRow::new("Price").show(ui, t, |ui| {
                Input::new(&mut panes[ap].alert_input_price)
                    .min_width(80.0)
                    .size(KitSize::Sm)
                    .placeholder(format!("{:.2}", current_price))
                    .show(ui, cx.theme);
            });
            ui.add_space(gap_xs());

            let input_price = panes[ap].alert_input_price.parse::<f32>().unwrap_or(current_price);
            let above_label = format!("{} Above {:.2}", Icon::ARROW_FAT_UP, input_price);
            let below_label = format!("{} Below {:.2}", Icon::ARROW_FAT_DOWN, input_price);
            match PanelDualAction::new(
                (above_label.as_str(), Tone::Success),
                (below_label.as_str(), Tone::Danger),
            ).show(ui, t) {
                Some(0) => cx.dispatch(AppCommand::AddPriceAlert { pane: ap, price: input_price, above: true }),
                Some(1) => cx.dispatch(AppCommand::AddPriceAlert { pane: ap, price: input_price, above: false }),
                _ => {}
            }
        });

    // ── Drafts ──
    let pane_drafts: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| a.draft).cloned().map(move |a| (pi, a))
    ).collect();
    if !pane_drafts.is_empty() {
        let r = PanelSection::new("DRAFT")
            .count(pane_drafts.len())
            .action("Place All", Tone::Accent)
            .show(ui, t, |ui| {
                for (pi, alert) in &pane_drafts {
                    ui.horizontal(|ui| {
                        let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
                        let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                            .armed(false).triggered(false).note("DRAFT").theme(cx.theme).show(ui);
                        if delete_clicked {
                            cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: alert.id });
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if super::kit::panel_action_btn(ui, "Place", cx.accent) {
                                cx.dispatch(AppCommand::PlaceDraftAlert { pane: *pi, id: alert.id });
                            }
                        });
                    });
                }
            });
        if r.action_clicked {
            cx.dispatch(AppCommand::PlaceAllDraftAlerts);
        }
    }

    // ── Active ──
    let active_alerts: Vec<_> = watchlist.alerts.iter().filter(|a| !a.triggered).cloned().collect();
    let triggered_alerts: Vec<_> = watchlist.alerts.iter().filter(|a| a.triggered).cloned().collect();

    let pane_active: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| !a.triggered && !a.draft).cloned().map(move |a| (pi, a))
    ).collect();
    let pane_triggered: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| a.triggered).cloned().map(move |a| (pi, a))
    ).collect();

    let total_active = active_alerts.len() + pane_active.len();
    let total_triggered = triggered_alerts.len() + pane_triggered.len();

    let mut active_section = PanelSection::new("ACTIVE")
        .tone(Tone::Accent)
        .count(total_active);
    if total_active > 0 {
        active_section = active_section.action("Clear All", Tone::Danger);
    }
    let active_resp = active_section.show(ui, t, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("alerts_scroll")
            .max_height(ui.available_height() * 0.6)
            .show(ui, |ui| {
                if active_alerts.is_empty() && pane_active.is_empty() {
                    PanelEmpty::new("No active alerts").show(ui, t);
                }
                for alert in &active_alerts {
                    let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
                    let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                        .armed(true).triggered(false).theme(cx.theme).show(ui);
                    if delete_clicked {
                        cx.dispatch(AppCommand::CancelWatchlistAlert { id: alert.id });
                    }
                }
                for (pi, alert) in &pane_active {
                    let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
                    let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                        .armed(true).triggered(false).theme(cx.theme).show(ui);
                    if delete_clicked {
                        cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: alert.id });
                    }
                }
            });
    });
    if active_resp.action_clicked {
        for a in &active_alerts {
            cx.dispatch(AppCommand::CancelWatchlistAlert { id: a.id });
        }
        for (pi, a) in &pane_active {
            cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: a.id });
        }
    }

    // ── Triggered ──
    if total_triggered > 0 {
        let mut sec = PanelSection::new("TRIGGERED").count(total_triggered);
        sec = sec.action("Dismiss All", Tone::Default);
        let r = sec.show(ui, t, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("triggered_scroll")
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    for alert in &triggered_alerts {
                        let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
                        let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                            .armed(false).triggered(true).theme(cx.theme).show(ui);
                        if delete_clicked {
                            cx.dispatch(AppCommand::CancelWatchlistAlert { id: alert.id });
                        }
                    }
                    for (pi, alert) in &pane_triggered {
                        let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
                        let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                            .armed(false).triggered(true).theme(cx.theme).show(ui);
                        if delete_clicked {
                            cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: alert.id });
                        }
                    }
                });
        });
        if r.action_clicked {
            for a in &triggered_alerts {
                cx.dispatch(AppCommand::CancelWatchlistAlert { id: a.id });
            }
            for (pi, a) in &pane_triggered {
                cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: a.id });
            }
        }
    }

}
