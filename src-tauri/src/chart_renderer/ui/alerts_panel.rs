//! Alerts management panel — shows all active and triggered alerts with controls.

use egui;
use super::style::*;
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
    .frame(panel_frame(t.toolbar_bg, t.toolbar_border))
    .show(ctx, |ui| {
        if panel_header(ui, &format!("{} ALERTS", Icon::BELL), t.accent, t.dim) {
            watchlist.alerts_panel_open = false;
        }
        ui.add_space(4.0);
        separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
        ui.add_space(4.0);
        draw_content(ui, watchlist, panes, ap, t);
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
    {
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
                    .fill(color_alpha(above_color, ALPHA_GHOST))
                    .corner_radius(RADIUS_MD)
                    .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(above_color, ALPHA_LINE)))
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
                        triggered: false, draft: false, symbol: sym.clone(),
                    });
                    panes[ap].alert_input_price.clear();
                }

                let below_color = t.bear;
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Below {:.2}", Icon::ARROW_FAT_DOWN, input_price))
                        .monospace().size(9.0).color(below_color))
                    .fill(color_alpha(below_color, ALPHA_GHOST))
                    .corner_radius(RADIUS_MD)
                    .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(below_color, ALPHA_LINE)))
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
                        triggered: false, draft: false, symbol: sym.clone(),
                    });
                    panes[ap].alert_input_price.clear();
                }
            });
        }

        ui.add_space(6.0);
        separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
        ui.add_space(4.0);

        // ── Draft Alerts (context-menu created, pending user Place) ──
        let pane_drafts: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
            p.price_alerts.iter().filter(|a| a.draft).cloned().map(move |a| (pi, a))
        ).collect();
        if !pane_drafts.is_empty() {
            ui.horizontal(|ui| {
                section_label(ui, &format!("DRAFT ({})", pane_drafts.len()), t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_action_btn(ui, "Place All", t.accent) {
                        for p in panes.iter_mut() {
                            for a in p.price_alerts.iter_mut() { if a.draft { a.draft = false; } }
                        }
                    }
                });
            });
            ui.add_space(4.0);
            let mut place_id: Option<(usize, u32)> = None;
            let mut cancel_id: Option<(usize, u32)> = None;
            for (pi, alert) in &pane_drafts {
                let dir = if alert.above { "\u{25B2}" } else { "\u{25BC}" };
                let dir_color = if alert.above { t.bull } else { t.bear };
                order_card(ui, t.dim, color_alpha(t.toolbar_border, ALPHA_FAINT), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong().color(TEXT_PRIMARY));
                        ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price))
                            .monospace().size(10.0).color(dir_color));
                        status_badge(ui, "DRAFT", t.dim);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_btn(ui, Icon::X, t.dim.gamma_multiply(0.5), FONT_MD).on_hover_text("Cancel").clicked() {
                                cancel_id = Some((*pi, alert.id));
                            }
                            if small_action_btn(ui, "Place", t.accent) {
                                place_id = Some((*pi, alert.id));
                            }
                        });
                    });
                });
            }
            if let Some((pi, id)) = place_id {
                if let Some(p) = panes.get_mut(pi) {
                    if let Some(a) = p.price_alerts.iter_mut().find(|a| a.id == id) { a.draft = false; }
                }
            }
            if let Some((pi, id)) = cancel_id {
                if let Some(p) = panes.get_mut(pi) { p.price_alerts.retain(|a| a.id != id); }
            }
            ui.add_space(6.0);
            separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
            ui.add_space(4.0);
        }

        // ── Active Alerts ──
        let active_alerts: Vec<_> = watchlist.alerts.iter()
            .filter(|a| !a.triggered).cloned().collect();
        let triggered_alerts: Vec<_> = watchlist.alerts.iter()
            .filter(|a| a.triggered).cloned().collect();

        // Per-pane alerts (from chart lines) — exclude drafts
        let pane_active: Vec<_> = panes.iter().flat_map(|p|
            p.price_alerts.iter().filter(|a| !a.triggered && !a.draft).cloned()
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
                    if small_action_btn(ui, "Clear All", t.bear) {
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
                order_card(ui, dir_color, color_alpha(t.toolbar_border, ALPHA_FAINT), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong()
                            .color(TEXT_PRIMARY));
                        ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price))
                            .monospace().size(10.0).color(dir_color));
                        status_badge(ui, "ACTIVE", egui::Color32::from_rgb(255, 191, 0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_btn(ui, Icon::X, t.dim.gamma_multiply(0.5), FONT_MD).clicked()
                            {
                                remove_watchlist_alert = Some(alert.id);
                            }
                        });
                    });
                });
            }

            // Per-pane chart alerts
            for (pi, pane) in panes.iter().enumerate() {
                for alert in pane.price_alerts.iter().filter(|a| !a.triggered && !a.draft) {
                    let dir = if alert.above { "\u{25B2}" } else { "\u{25BC}" };
                    let dir_color = if alert.above { t.bull } else { t.bear };
                    order_card(ui, dir_color, color_alpha(t.toolbar_border, ALPHA_FAINT), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong()
                                .color(TEXT_PRIMARY));
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
            separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                section_label(ui, &format!("TRIGGERED ({})", total_triggered), t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_action_btn(ui, "Dismiss All", t.dim.gamma_multiply(0.5)) {
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
                                .color(TEXT_SECONDARY));
                            ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price))
                                .monospace().size(10.0).color(t.accent));
                            status_badge(ui, "TRIGGERED", t.accent);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if icon_btn(ui, Icon::X, t.dim.gamma_multiply(0.4), FONT_MD).clicked()
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
                                    .color(TEXT_SECONDARY));
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
    }
}
