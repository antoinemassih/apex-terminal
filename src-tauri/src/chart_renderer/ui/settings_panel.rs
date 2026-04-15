//! Settings panel — appearance, axes, font scale, session shading.

use egui;
use super::style::{color_alpha, hex_to_color, dialog_window_themed, dialog_header, dialog_section, tab_bar, FONT_LG, ALPHA_MUTED};
use super::super::gpu::{Watchlist, Theme, Chart};

/// Which tab is active in the settings dialog.
#[derive(Clone, Copy, PartialEq)]
enum SettingsTab { General, Shortcuts }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, chart: &mut Chart, t: &Theme) {
// ── Settings panel ──────────────────────────────────────────────────────
if watchlist.settings_open {
    let screen = ctx.screen_rect();
    // Widen slightly to accommodate Shortcuts tab content (560px)
    let dialog_w = 560.0_f32;
    dialog_window_themed(ctx, "settings_panel", egui::pos2(screen.center().x - dialog_w / 2.0, 60.0), dialog_w, t.toolbar_bg, t.toolbar_border, None)
        .show(ctx, |ui| {
            if dialog_header(ui, "SETTINGS", t.dim) { watchlist.settings_open = false; }

            // ── Tab bar ──
            let settings_tab_id = egui::Id::new("settings_active_tab");
            let mut active_tab: SettingsTab = ui.data_mut(|d| *d.get_temp_mut_or(settings_tab_id, SettingsTab::General));
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                tab_bar(ui, &mut active_tab, &[
                    (SettingsTab::General, "General"),
                    (SettingsTab::Shortcuts, "Shortcuts"),
                ], t.accent, t.dim);
            });
            ui.data_mut(|d| d.insert_temp(settings_tab_id, active_tab));
            // Separator below tab bar
            let rect = ui.available_rect_before_wrap();
            ui.painter().line_segment(
                [egui::pos2(rect.left(), ui.cursor().min.y), egui::pos2(rect.right(), ui.cursor().min.y)],
                egui::Stroke::new(0.5, color_alpha(t.toolbar_border, ALPHA_MUTED)));
            ui.add_space(1.0);

            match active_tab {
                SettingsTab::Shortcuts => {
                    ui.add_space(8.0);
                    super::hotkey_editor::draw_content(ui, watchlist, t);
                }
                SettingsTab::General => {
            ui.add_space(8.0);
            let m = 10.0;
            let _ = FONT_LG; // suppress unused import

            // ── Appearance section ──
            dialog_section(ui, "APPEARANCE", m, t.dim.gamma_multiply(0.5));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Font Scale").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    // Display 60-160% maps to 0.96-2.56 ppp. 100% = 1.6 (baseline)
                    let display_pct = ((watchlist.font_scale - 0.96) / 0.016).round() as i32 + 60;
                    let mut dp = display_pct.clamp(60, 160);
                    if ui.add(egui::DragValue::new(&mut dp).range(60..=160).suffix("%").speed(1)
                        .custom_formatter(|v, _| format!("{}%", v as i32))).changed() {
                        watchlist.font_scale = 0.96 + (dp - 60) as f32 * 0.016;
                    }
                });
            });
            // Preset buttons (display % → internal ppp)
            ui.horizontal(|ui| {
                ui.add_space(m);
                // 100% = 1.6 ppp (baseline), 20% steps = 0.32 ppp each
                for (label, ppp) in [(60, 0.96_f32), (80, 1.28), (100, 1.6), (120, 1.92), (140, 2.24), (160, 2.56)] {
                    let active = (watchlist.font_scale - ppp).abs() < 0.05;
                    let fg = if active { t.accent } else { t.dim.gamma_multiply(0.6) };
                    let bg = if active { color_alpha(t.accent, 25) } else { egui::Color32::TRANSPARENT };
                    if ui.add(egui::Button::new(egui::RichText::new(format!("{}%", label)).monospace().size(9.0).color(fg))
                        .fill(bg).corner_radius(3.0).min_size(egui::vec2(32.0, 18.0))).clicked() {
                        watchlist.font_scale = ppp;
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Compact Mode").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.compact_mode;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.compact_mode = val;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Auto-Hide Toolbar").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.toolbar_auto_hide;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.toolbar_auto_hide = val;
                        if !val { watchlist.toolbar_hover_time = None; }
                    }
                });
            });
            ui.add_space(8.0);

            // ── Axes section ──
            dialog_section(ui, "AXES", m, t.dim.gamma_multiply(0.5));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Show X-Axis (time)").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.show_x_axis;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.show_x_axis = val;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Show Y-Axis (price)").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.show_y_axis;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.show_y_axis = val;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Shared X-Axis").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.shared_x_axis;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.shared_x_axis = val;
                    }
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Shared Y-Axis").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut val = watchlist.shared_y_axis;
                    if ui.add(egui::Checkbox::without_text(&mut val)).changed() {
                        watchlist.shared_y_axis = val;
                    }
                });
            });
            ui.add_space(8.0);

            // ── Sessions section (per-pane session shading) ──
            let is_crypto = crate::data::is_crypto(&chart.symbol);
            dialog_section(ui, "SESSIONS", m, t.dim.gamma_multiply(0.5));
            ui.add_space(4.0);
            if is_crypto {
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("N/A for crypto (24/7 market)").monospace().size(9.0).color(t.dim.gamma_multiply(0.5)));
                });
            } else {
                // Master toggle: Session Shading
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("Session Shading").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        ui.add(egui::Checkbox::without_text(&mut chart.session_shading));
                    });
                });
                if chart.session_shading {
                    ui.add_space(2.0);
                    // Extended Hours Opacity slider (0-100%)
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("ETH Bar Opacity").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(m);
                            let mut pct = (chart.eth_bar_opacity * 100.0).round() as i32;
                            if ui.add(egui::DragValue::new(&mut pct).range(0..=100).suffix("%").speed(1)).changed() {
                                chart.eth_bar_opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
                            }
                        });
                    });
                    ui.add_space(4.0);
                    // Background Tint toggle
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Background Tint").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(m);
                            ui.add(egui::Checkbox::without_text(&mut chart.session_bg_tint));
                        });
                    });
                    if chart.session_bg_tint {
                        ui.add_space(2.0);
                        // Background color preview + hex input
                        ui.horizontal(|ui| {
                            ui.add_space(m + 10.0);
                            let preview_c = hex_to_color(&chart.session_bg_color, 1.0);
                            let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            ui.painter().rect_filled(r, 3.0, preview_c);
                            ui.painter().rect_stroke(r, 3.0, egui::Stroke::new(0.5, egui::Color32::from_white_alpha(60)), egui::StrokeKind::Outside);
                            ui.label(egui::RichText::new("Color").monospace().size(9.0).color(egui::Color32::from_white_alpha(140)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(m);
                                let mut hex = chart.session_bg_color.clone();
                                let r = ui.add(egui::TextEdit::singleline(&mut hex)
                                    .desired_width(70.0)
                                    .font(egui::FontId::monospace(10.0)));
                                if r.changed() {
                                    chart.session_bg_color = hex;
                                }
                            });
                        });
                        // Color preset buttons
                        ui.horizontal(|ui| {
                            ui.add_space(m + 10.0);
                            for (label, hex) in [("Navy", "#1a1a2e"), ("Purple", "#2d1b4e"), ("Green", "#1a2e1a"), ("Red", "#2e1a1a"), ("Blue", "#1a2e3e")] {
                                let active = chart.session_bg_color == hex;
                                let c = hex_to_color(hex, 1.0);
                                let fg = if active { t.accent } else { egui::Color32::from_white_alpha(120) };
                                let bg = if active { color_alpha(c, 80) } else { color_alpha(c, 40) };
                                if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(8.0).color(fg))
                                    .fill(bg).corner_radius(3.0).min_size(egui::vec2(36.0, 16.0))).clicked() {
                                    chart.session_bg_color = hex.to_string();
                                }
                            }
                        });
                        ui.add_space(2.0);
                        // Background opacity slider
                        ui.horizontal(|ui| {
                            ui.add_space(m + 10.0);
                            ui.label(egui::RichText::new("Opacity").monospace().size(9.0).color(egui::Color32::from_white_alpha(140)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(m);
                                let mut pct = (chart.session_bg_opacity * 100.0).round() as i32;
                                if ui.add(egui::DragValue::new(&mut pct).range(0..=100).suffix("%").speed(1)).changed() {
                                    chart.session_bg_opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
                                }
                            });
                        });
                    }
                    ui.add_space(4.0);
                    // Session Break Lines toggle
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Session Break Lines").monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(m);
                            ui.add(egui::Checkbox::without_text(&mut chart.session_break_lines));
                        });
                    });
                    ui.add_space(4.0);
                    // RTH time display (informational)
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        let start_h = chart.rth_start_minutes / 60;
                        let start_m = chart.rth_start_minutes % 60;
                        let end_h = chart.rth_end_minutes / 60;
                        let end_m = chart.rth_end_minutes % 60;
                        ui.label(egui::RichText::new(format!("RTH: {:02}:{:02} - {:02}:{:02} ET", start_h, start_m, end_h, end_m))
                            .monospace().size(9.0).color(t.dim.gamma_multiply(0.5)));
                    });
                }
            }
            ui.add_space(8.0);

            // ── Order Defaults section ──
            dialog_section(ui, "ORDER DEFAULTS", m, t.dim.gamma_multiply(0.5));
            ui.add_space(4.0);

            // Default stock qty
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Stock Qty").monospace().size(9.0).color(t.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut v = watchlist.default_stock_qty as i32;
                    if ui.add(egui::DragValue::new(&mut v).range(1..=100_000).speed(10)
                        .custom_formatter(|v, _| format!("{} shares", v as i32))).changed() {
                        watchlist.default_stock_qty = v.max(1) as u32;
                    }
                });
            });

            // Default options qty
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Options Qty").monospace().size(9.0).color(t.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut v = watchlist.default_options_qty as i32;
                    if ui.add(egui::DragValue::new(&mut v).range(1..=10_000).speed(1)
                        .custom_formatter(|v, _| format!("{} contracts", v as i32))).changed() {
                        watchlist.default_options_qty = v.max(1) as u32;
                    }
                });
            });

            // Default order type
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Order Type").monospace().size(9.0).color(t.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (i, label) in ["MKT", "LMT", "STP"].iter().enumerate() {
                        let sel = watchlist.default_order_type == i;
                        let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.6) };
                        let bg = if sel { color_alpha(t.accent, 50) } else { color_alpha(t.toolbar_border, 20) };
                        let rounding = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                            else if i == 2 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                            else { egui::CornerRadius::ZERO };
                        if ui.add(egui::Button::new(egui::RichText::new(*label).monospace().size(8.0).color(fg))
                            .fill(bg).corner_radius(rounding).min_size(egui::vec2(28.0, 18.0))
                            .stroke(egui::Stroke::new(0.5, if sel { color_alpha(t.accent, 80) } else { color_alpha(t.toolbar_border, 40) })))
                            .clicked() { watchlist.default_order_type = i; }
                    }
                });
            });

            // Default TIF
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Time in Force").monospace().size(9.0).color(t.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (i, label) in ["DAY", "GTC", "IOC"].iter().enumerate() {
                        let sel = watchlist.default_tif == i;
                        let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.6) };
                        let bg = if sel { color_alpha(t.accent, 50) } else { color_alpha(t.toolbar_border, 20) };
                        let rounding = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                            else if i == 2 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                            else { egui::CornerRadius::ZERO };
                        if ui.add(egui::Button::new(egui::RichText::new(*label).monospace().size(8.0).color(fg))
                            .fill(bg).corner_radius(rounding).min_size(egui::vec2(28.0, 18.0))
                            .stroke(egui::Stroke::new(0.5, if sel { color_alpha(t.accent, 80) } else { color_alpha(t.toolbar_border, 40) })))
                            .clicked() { watchlist.default_tif = i; }
                    }
                });
            });

            // Outside RTH
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Outside RTH").monospace().size(9.0).color(t.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut v = watchlist.default_outside_rth;
                    if ui.checkbox(&mut v, "").changed() { watchlist.default_outside_rth = v; }
                });
            });

            // Paper mode
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Paper Trading").monospace().size(9.0).color(t.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(m);
                    let mut paper = crate::chart_renderer::trading::order_manager::is_paper_mode();
                    if ui.checkbox(&mut paper, "").changed() {
                        crate::chart_renderer::trading::order_manager::set_paper_mode(paper);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add_space(m + 4.0);
                let paper = crate::chart_renderer::trading::order_manager::is_paper_mode();
                let (label, color) = if paper {
                    ("Paper mode active — orders go to simulated account", egui::Color32::from_rgb(46, 204, 113))
                } else {
                    ("LIVE mode — real money at risk", egui::Color32::from_rgb(230, 70, 70))
                };
                ui.label(egui::RichText::new(label).monospace().size(7.5).color(color));
            });

            ui.add_space(8.0);

            // ── Risk Management section ──
            dialog_section(ui, "RISK MANAGEMENT", m, t.dim.gamma_multiply(0.5));
            ui.add_space(4.0);
            {
                use crate::chart_renderer::trading::order_manager;
                let mut limits = order_manager::get_risk_limits();

                // Max order qty
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("Max Order Qty").monospace().size(9.0).color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let mut v = limits.max_order_qty as i32;
                        if ui.add(egui::DragValue::new(&mut v).range(1..=100_000).speed(10)).changed() {
                            limits.max_order_qty = v.max(1) as u32;
                        }
                    });
                });

                // Max position qty
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("Max Position").monospace().size(9.0).color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let mut v = limits.max_position_qty as i32;
                        if ui.add(egui::DragValue::new(&mut v).range(1..=500_000).speed(100)).changed() {
                            limits.max_position_qty = v.max(1) as u32;
                        }
                    });
                });

                // Max notional
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("Max Notional $").monospace().size(9.0).color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let mut v = limits.max_notional as i64;
                        if ui.add(egui::DragValue::new(&mut v).range(0..=10_000_000).speed(1000)
                            .custom_formatter(|v, _| if v as i64 == 0 { "OFF".to_string() } else { format!("${}", v as i64) })).changed() {
                            limits.max_notional = v.max(0) as f64;
                        }
                    });
                });

                // Fat finger %
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("Fat Finger %").monospace().size(9.0).color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let mut v = limits.fat_finger_pct;
                        if ui.add(egui::DragValue::new(&mut v).range(0.0..=50.0).speed(0.5).suffix("%")
                            .custom_formatter(|v, _| if v < 0.1 { "OFF".to_string() } else { format!("{:.1}%", v) })).changed() {
                            limits.fat_finger_pct = v.max(0.0);
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.add_space(m + 4.0);
                    ui.label(egui::RichText::new("(only on opening orders, exits unrestricted)")
                        .monospace().size(7.5).color(t.dim.gamma_multiply(0.4)));
                });

                // Max open orders
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("Max Open Orders").monospace().size(9.0).color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let mut v = limits.max_open_orders as i32;
                        if ui.add(egui::DragValue::new(&mut v).range(1..=1000).speed(1)).changed() {
                            limits.max_open_orders = v.max(1) as usize;
                        }
                    });
                });

                // Max daily loss
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("Max Daily Loss $").monospace().size(9.0).color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let mut v = limits.max_daily_loss as i64;
                        if ui.add(egui::DragValue::new(&mut v).range(0..=1_000_000).speed(500)
                            .custom_formatter(|v, _| if v as i64 == 0 { "OFF".to_string() } else { format!("${}", v as i64) })).changed() {
                            limits.max_daily_loss = v.max(0) as f64;
                        }
                    });
                });

                // Dedup cooldown
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("Dedup Cooldown").monospace().size(9.0).color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let mut v = limits.dedup_cooldown_ms as i32;
                        if ui.add(egui::DragValue::new(&mut v).range(100..=5000).speed(50).suffix("ms")).changed() {
                            limits.dedup_cooldown_ms = v.max(100) as u64;
                        }
                    });
                });

                order_manager::update_risk_limits(limits);
            }

            ui.add_space(8.0);
            } // end General arm
            } // end match active_tab
        });
}


}
