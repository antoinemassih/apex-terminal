//! Alerts management panel — shows all active and triggered alerts with controls.

use egui;
use super::style::{close_button, separator, color_alpha, section_label, status_badge, order_card};
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::trading::PriceAlert;

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
if !watchlist.alerts_panel_open { return; }

egui::SidePanel::right("alerts_panel")
    .default_width(260.0)
    .min_width(200.0)
    .max_width(320.0)
    .frame(egui::Frame::NONE.fill(t.toolbar_bg)
        .inner_margin(egui::Margin { left: 8, right: 8, top: 8, bottom: 6 })
        .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80))))
    .show(ctx, |ui| {
        // ── Panel header ──
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{} ALERTS", Icon::BELL)).monospace().size(11.0).strong().color(t.accent));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if close_button(ui, t.dim) { watchlist.alerts_panel_open = false; }
            });
        });
        ui.add_space(4.0);
        separator(ui, color_alpha(t.toolbar_border, 40));
        ui.add_space(4.0);

        // ── Add Alert section ──
        {
            let chart = &panes[ap];
            let current_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
            let sym = chart.symbol.clone();

            ui.horizontal(|ui| {
                section_label(ui, "ADD ALERT", t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("{} @ {:.2}", sym, current_price))
                        .monospace().size(9.0).color(t.dim.gamma_multiply(0.5)));
                });
            });
            ui.add_space(4.0);

            // Price input
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Price:").monospace().size(9.0).color(t.dim));
                let te = egui::TextEdit::singleline(&mut panes[ap].alert_input_price)
                    .desired_width(80.0)
                    .font(egui::FontId::monospace(10.0))
                    .text_color(egui::Color32::WHITE)
                    .hint_text(format!("{:.2}", current_price));
                ui.add(te);
            });
            ui.add_space(3.0);

            let input_price = panes[ap].alert_input_price.parse::<f32>().unwrap_or(current_price);

            ui.horizontal(|ui| {
                let above_color = t.bull;
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Above {:.2}", Icon::ARROW_FAT_UP, input_price))
                        .monospace().size(9.0).color(above_color))
                    .fill(color_alpha(above_color, 15))
                    .corner_radius(3.0)
                    .stroke(egui::Stroke::new(0.5, color_alpha(above_color, 50)))
                    .min_size(egui::vec2(0.0, 20.0))).clicked()
                {
                    // Add to watchlist alerts
                    let id = watchlist.next_alert_id; watchlist.next_alert_id += 1;
                    watchlist.alerts.push(crate::chart_renderer::trading::Alert {
                        id, symbol: sym.clone(), price: input_price, above: true,
                        triggered: false, message: String::new(),
                    });
                    // Also add per-pane chart alert line
                    let pid = panes[ap].next_alert_id; panes[ap].next_alert_id += 1;
                    panes[ap].price_alerts.push(PriceAlert {
                        id: pid, price: input_price, above: true,
                        triggered: false, symbol: sym.clone(),
                    });
                    panes[ap].alert_input_price.clear();
                }

                let below_color = t.bear;
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Below {:.2}", Icon::ARROW_FAT_DOWN, input_price))
                        .monospace().size(9.0).color(below_color))
                    .fill(color_alpha(below_color, 15))
                    .corner_radius(3.0)
                    .stroke(egui::Stroke::new(0.5, color_alpha(below_color, 50)))
                    .min_size(egui::vec2(0.0, 20.0))).clicked()
                {
                    let id = watchlist.next_alert_id; watchlist.next_alert_id += 1;
                    watchlist.alerts.push(crate::chart_renderer::trading::Alert {
                        id, symbol: sym.clone(), price: input_price, above: false,
                        triggered: false, message: String::new(),
                    });
                    let pid = panes[ap].next_alert_id; panes[ap].next_alert_id += 1;
                    panes[ap].price_alerts.push(PriceAlert {
                        id: pid, price: input_price, above: false,
                        triggered: false, symbol: sym.clone(),
                    });
                    panes[ap].alert_input_price.clear();
                }
            });
        }

        ui.add_space(6.0);
        separator(ui, color_alpha(t.toolbar_border, 40));
        ui.add_space(4.0);

        // ── Active Alerts ──
        let active_alerts: Vec<_> = watchlist.alerts.iter()
            .filter(|a| !a.triggered).cloned().collect();
        let triggered_alerts: Vec<_> = watchlist.alerts.iter()
            .filter(|a| a.triggered).cloned().collect();

        // Per-pane alerts (from chart lines)
        let pane_active: Vec<_> = panes.iter().flat_map(|p|
            p.price_alerts.iter().filter(|a| !a.triggered).cloned()
        ).collect();
        let pane_triggered: Vec<_> = panes.iter().flat_map(|p|
            p.price_alerts.iter().filter(|a| a.triggered).cloned()
        ).collect();

        let total_active = active_alerts.len() + pane_active.len();
        let total_triggered = triggered_alerts.len() + pane_triggered.len();

        // Active section
        ui.horizontal(|ui| {
            section_label(ui, &format!("ACTIVE ({})", total_active), t.accent);
            if total_active > 0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(
                        egui::RichText::new("Clear All").monospace().size(8.0).color(t.bear))
                        .fill(color_alpha(t.bear, 15)).corner_radius(2.0)
                        .stroke(egui::Stroke::new(0.5, color_alpha(t.bear, 50)))
                        .min_size(egui::vec2(0.0, 14.0))).clicked()
                    {
                        watchlist.alerts.retain(|a| a.triggered);
                        for p in panes.iter_mut() {
                            p.price_alerts.retain(|a| a.triggered);
                        }
                    }
                });
            }
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical().id_salt("alerts_scroll").max_height(ui.available_height() * 0.6).show(ui, |ui| {
            let mut remove_watchlist_alert: Option<u32> = None;
            let mut remove_pane_alert: Option<(usize, u32)> = None; // (pane_idx, alert_id)

            if active_alerts.is_empty() && pane_active.is_empty() {
                ui.label(egui::RichText::new("No active alerts").monospace().size(9.0).color(t.dim.gamma_multiply(0.4)));
            }

            // Watchlist-level alerts
            for alert in &active_alerts {
                let dir = if alert.above { "\u{25B2}" } else { "\u{25BC}" }; // up/down arrow
                let dir_color = if alert.above { t.bull } else { t.bear };
                order_card(ui, dir_color, color_alpha(t.toolbar_border, 10), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong()
                            .color(egui::Color32::from_rgb(220,220,230)));
                        ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price))
                            .monospace().size(10.0).color(dir_color));
                        status_badge(ui, "ACTIVE", egui::Color32::from_rgb(255, 191, 0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(
                                egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.5)))
                                .frame(false)).clicked()
                            {
                                remove_watchlist_alert = Some(alert.id);
                            }
                        });
                    });
                });
            }

            // Per-pane chart alerts
            for (pi, pane) in panes.iter().enumerate() {
                for alert in pane.price_alerts.iter().filter(|a| !a.triggered) {
                    let dir = if alert.above { "\u{25B2}" } else { "\u{25BC}" };
                    let dir_color = if alert.above { t.bull } else { t.bear };
                    order_card(ui, dir_color, color_alpha(t.toolbar_border, 10), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong()
                                .color(egui::Color32::from_rgb(220,220,230)));
                            ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price))
                                .monospace().size(10.0).color(dir_color));
                            status_badge(ui, "ACTIVE", egui::Color32::from_rgb(255, 191, 0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(
                                    egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.5)))
                                    .frame(false)).clicked()
                                {
                                    remove_pane_alert = Some((pi, alert.id));
                                }
                            });
                        });
                    });
                }
            }

            if let Some(id) = remove_watchlist_alert { watchlist.alerts.retain(|a| a.id != id); }
            if let Some((pi, id)) = remove_pane_alert {
                if pi < panes.len() { panes[pi].price_alerts.retain(|a| a.id != id); }
            }
        });

        // ── Triggered section ──
        if total_triggered > 0 {
            ui.add_space(6.0);
            separator(ui, color_alpha(t.toolbar_border, 40));
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                section_label(ui, &format!("TRIGGERED ({})", total_triggered), t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(
                        egui::RichText::new("Dismiss All").monospace().size(8.0).color(t.dim.gamma_multiply(0.5)))
                        .fill(egui::Color32::TRANSPARENT).corner_radius(2.0)
                        .min_size(egui::vec2(0.0, 14.0))).clicked()
                    {
                        watchlist.alerts.retain(|a| !a.triggered);
                        for p in panes.iter_mut() {
                            p.price_alerts.retain(|a| !a.triggered);
                        }
                    }
                });
            });
            ui.add_space(4.0);

            egui::ScrollArea::vertical().id_salt("triggered_scroll").max_height(ui.available_height()).show(ui, |ui| {
                let mut dismiss_watchlist: Option<u32> = None;
                let mut dismiss_pane: Option<(usize, u32)> = None;

                for alert in &triggered_alerts {
                    let dir = if alert.above { "\u{25B2}" } else { "\u{25BC}" };
                    order_card(ui, t.accent, color_alpha(t.toolbar_border, 8), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong()
                                .color(egui::Color32::from_rgb(200,200,210)));
                            ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price))
                                .monospace().size(10.0).color(t.accent));
                            status_badge(ui, "TRIGGERED", t.accent);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(
                                    egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.4)))
                                    .frame(false)).clicked()
                                {
                                    dismiss_watchlist = Some(alert.id);
                                }
                            });
                        });
                    });
                }

                for (pi, pane) in panes.iter().enumerate() {
                    for alert in pane.price_alerts.iter().filter(|a| a.triggered) {
                        let dir = if alert.above { "\u{25B2}" } else { "\u{25BC}" };
                        order_card(ui, t.accent, color_alpha(t.toolbar_border, 8), |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong()
                                    .color(egui::Color32::from_rgb(200,200,210)));
                                ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price))
                                    .monospace().size(10.0).color(t.accent));
                                status_badge(ui, "TRIGGERED", t.accent);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(
                                        egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.4)))
                                        .frame(false)).clicked()
                                    {
                                        dismiss_pane = Some((pi, alert.id));
                                    }
                                });
                            });
                        });
                    }
                }

                if let Some(id) = dismiss_watchlist { watchlist.alerts.retain(|a| a.id != id); }
                if let Some((pi, id)) = dismiss_pane {
                    if pi < panes.len() { panes[pi].price_alerts.retain(|a| a.id != id); }
                }
            });
        }
    });
}
