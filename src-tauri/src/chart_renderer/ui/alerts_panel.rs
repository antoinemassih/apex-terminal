//! Alerts management panel — shows all active and triggered alerts with controls.
//!
//! Wave 5 migration: re-established `UiCtx` plumbing (Phase 3 of the design-system
//! roadmap), adopted `AlertRow` for per-alert rendering, and replaced inline
//! mutations with `AppCommand` dispatch through `cx.dispatch(...)`.

use egui;
use super::style::*;
use super::widgets;
use super::widgets::inputs::TextInput;
use super::widgets::rows::alert_row::{AlertRow, AlertCmp};
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::trading::PriceAlert;
use crate::chart_renderer::commands::{AppCommand, UiCtx};

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
        .frame(widgets::frames::PanelFrame::new(cx.toolbar_bg, cx.toolbar_border).theme(&cx).build())
        .show(ctx, |ui| {
            if panel_header(ui, &format!("{} ALERTS", Icon::BELL), cx.accent, cx.dim) {
                watchlist.alerts_panel_open = false;
            }
            ui.add_space(4.0);
            separator(ui, color_alpha(cx.toolbar_border, ALPHA_MUTED));
            ui.add_space(4.0);
            draw_content_cx(ui, watchlist, panes, ap, &cx);
        });
}

/// Tab body content (no SidePanel wrapper, no header). Used by signals_panel.
/// Public API still takes `&Theme`; internally builds `UiCtx`.
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
    // ── Add Alert section ──
    {
        let chart = &panes[ap];
        let current_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
        let sym = chart.symbol.clone();

        ui.horizontal(|ui| {
            ui.add(widgets::text::SectionLabel::new("ADD ALERT").tiny().color(cx.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(widgets::text::MonospaceCode::new(&format!("{} @ {:.2}", sym, current_price)).xs().color(cx.dim).gamma(0.5));
            });
        });
        ui.add_space(4.0);

        // Price input
        ui.horizontal(|ui| {
            ui.add(widgets::text::MonospaceCode::new("Price:").xs().color(cx.dim));
            TextInput::new(&mut panes[ap].alert_input_price)
                .width(80.0)
                .font_size(10.0)
                .text_color(egui::Color32::WHITE)
                .placeholder(&format!("{:.2}", current_price))
                .theme(cx.theme)
                .show(ui);
        });
        ui.add_space(3.0);

        let input_price = panes[ap].alert_input_price.parse::<f32>().unwrap_or(current_price);

        ui.horizontal(|ui| {
            let above_color = cx.bull;
            if ui.add(widgets::buttons::ChromeBtn::new(
                egui::RichText::new(format!("{} Above {:.2}", Icon::ARROW_FAT_UP, input_price))
                    .monospace().size(9.0).color(above_color))
                .fill(color_alpha(above_color, ALPHA_GHOST))
                .corner_radius(r_md_cr())
                .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(above_color, ALPHA_LINE)))
                .min_size(egui::vec2(0.0, 20.0))).clicked()
            {
                cx.dispatch(AppCommand::AddPriceAlert { pane: ap, price: input_price, above: true });
            }

            let below_color = cx.bear;
            if ui.add(widgets::buttons::ChromeBtn::new(
                egui::RichText::new(format!("{} Below {:.2}", Icon::ARROW_FAT_DOWN, input_price))
                    .monospace().size(9.0).color(below_color))
                .fill(color_alpha(below_color, ALPHA_GHOST))
                .corner_radius(r_md_cr())
                .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(below_color, ALPHA_LINE)))
                .min_size(egui::vec2(0.0, 20.0))).clicked()
            {
                cx.dispatch(AppCommand::AddPriceAlert { pane: ap, price: input_price, above: false });
            }
        });
    }

    ui.add_space(6.0);
    separator(ui, color_alpha(cx.toolbar_border, ALPHA_MUTED));
    ui.add_space(4.0);

    // ── Draft Alerts (context-menu created, pending user Place) ──
    let pane_drafts: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| a.draft).cloned().map(move |a| (pi, a))
    ).collect();
    if !pane_drafts.is_empty() {
        ui.horizontal(|ui| {
            ui.add(widgets::text::SectionLabel::new(&format!("DRAFT ({})", pane_drafts.len())).tiny().color(cx.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if small_action_btn(ui, "Place All", cx.accent) {
                    cx.dispatch(AppCommand::PlaceAllDraftAlerts);
                }
            });
        });
        ui.add_space(4.0);
        for (pi, alert) in &pane_drafts {
            ui.horizontal(|ui| {
                let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
                let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                    .armed(false)
                    .triggered(false)
                    .note("DRAFT")
                    .theme(cx.theme)
                    .show(ui);
                if delete_clicked {
                    cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: alert.id });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_action_btn(ui, "Place", cx.accent) {
                        cx.dispatch(AppCommand::PlaceDraftAlert { pane: *pi, id: alert.id });
                    }
                });
            });
        }
        ui.add_space(6.0);
        separator(ui, color_alpha(cx.toolbar_border, ALPHA_MUTED));
        ui.add_space(4.0);
    }

    // ── Active Alerts ──
    let active_alerts: Vec<_> = watchlist.alerts.iter()
        .filter(|a| !a.triggered).cloned().collect();
    let triggered_alerts: Vec<_> = watchlist.alerts.iter()
        .filter(|a| a.triggered).cloned().collect();

    // Per-pane alerts (from chart lines) — exclude drafts
    let pane_active: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| !a.triggered && !a.draft).cloned().map(move |a| (pi, a))
    ).collect();
    let pane_triggered: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| a.triggered).cloned().map(move |a| (pi, a))
    ).collect();

    let total_active = active_alerts.len() + pane_active.len();
    let total_triggered = triggered_alerts.len() + pane_triggered.len();

    // Active section
    ui.horizontal(|ui| {
        ui.add(widgets::text::SectionLabel::new(&format!("ACTIVE ({})", total_active)).tiny().color(cx.accent));
        if total_active > 0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if small_action_btn(ui, "Clear All", cx.bear) {
                    // Bulk clear: dispatch one cancel per active alert.
                    for a in &active_alerts {
                        cx.dispatch(AppCommand::CancelWatchlistAlert { id: a.id });
                    }
                    for (pi, a) in &pane_active {
                        cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: a.id });
                    }
                }
            });
        }
    });
    ui.add_space(4.0);

    egui::ScrollArea::vertical().id_salt("alerts_scroll").max_height(ui.available_height() * 0.6).show(ui, |ui| {
        if active_alerts.is_empty() && pane_active.is_empty() {
            ui.add(widgets::text::MonospaceCode::new("No active alerts").xs().color(cx.dim).gamma(0.4));
        }

        // Watchlist-level alerts
        for alert in &active_alerts {
            let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
            let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                .armed(true)
                .triggered(false)
                .theme(cx.theme)
                .show(ui);
            if delete_clicked {
                cx.dispatch(AppCommand::CancelWatchlistAlert { id: alert.id });
            }
        }

        // Per-pane chart alerts
        for (pi, alert) in &pane_active {
            let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
            let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                .armed(true)
                .triggered(false)
                .theme(cx.theme)
                .show(ui);
            if delete_clicked {
                cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: alert.id });
            }
        }
    });

    // ── Triggered section ──
    if total_triggered > 0 {
        ui.add_space(6.0);
        separator(ui, color_alpha(cx.toolbar_border, ALPHA_MUTED));
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.add(widgets::text::SectionLabel::new(&format!("TRIGGERED ({})", total_triggered)).tiny().color(cx.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if small_action_btn(ui, "Dismiss All", cx.dim.gamma_multiply(0.5)) {
                    for a in &triggered_alerts {
                        cx.dispatch(AppCommand::CancelWatchlistAlert { id: a.id });
                    }
                    for (pi, a) in &pane_triggered {
                        cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: a.id });
                    }
                }
            });
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical().id_salt("triggered_scroll").max_height(ui.available_height()).show(ui, |ui| {
            for alert in &triggered_alerts {
                let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
                let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                    .armed(false)
                    .triggered(true)
                    .theme(cx.theme)
                    .show(ui);
                if delete_clicked {
                    cx.dispatch(AppCommand::CancelWatchlistAlert { id: alert.id });
                }
            }

            for (pi, alert) in &pane_triggered {
                let cmp = if alert.above { AlertCmp::Above } else { AlertCmp::Below };
                let (_resp, delete_clicked) = AlertRow::new(&alert.symbol, cmp, alert.price)
                    .armed(false)
                    .triggered(true)
                    .theme(cx.theme)
                    .show(ui);
                if delete_clicked {
                    cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: alert.id });
                }
            }
        });
    }
}
