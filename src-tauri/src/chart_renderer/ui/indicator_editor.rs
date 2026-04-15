//! Indicator Editor UI component.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::LineStyle;
const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Indicator editor popup (per-type properties panel) ──────────────────
let t = &THEMES[panes[ap].theme_idx];
if let Some(edit_id) = panes[ap].editing_indicator {
    let mut close_editor = false;
    let mut delete_id: Option<u32> = None;
    let mut needs_recompute = false;
    let mut needs_source_fetch: Option<(String, String, u32)> = None;
    let pane_symbol = panes[ap].symbol.clone();

    // Determine panel width based on indicator complexity
    let ind_kind = panes[ap].indicators.iter().find(|i| i.id == edit_id).map(|i| i.kind);
    let panel_w = match ind_kind {
        Some(IndicatorType::MACD) | Some(IndicatorType::Ichimoku) => 310.0,
        _ => 270.0,
    };

    egui::Window::new(format!("ind_editor_{}", edit_id))
        .default_pos(egui::pos2(200.0, 80.0))
        .default_size(egui::vec2(panel_w, 0.0))
        .resizable(false)
        .movable(true)
        .title_bar(false)
        .interactable(true)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
            .stroke(egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_HEAVY)))
            .corner_radius(6.0))
        .show(ctx, |ui| {
            if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.id == edit_id) {
                let ind_color = hex_to_color(&ind.color, 1.0);
                let m = 10.0;

                // ── Header: color dot + name + X ──
                let header_resp = ui.horizontal(|ui| {
                    ui.set_min_width(panel_w);
                    let hr = ui.max_rect();
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(hr.min, egui::vec2(panel_w, 26.0)),
                        egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 },
                        color_alpha(t.toolbar_border, ALPHA_TINT));
                    ui.add_space(8.0);
                    ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 10.0), 4.0, ind_color);
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(ind.display_name()).monospace().size(10.0).strong().color(TEXT_PRIMARY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);
                        let xr = ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim.gamma_multiply(0.5)))
                            .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(20.0, 20.0)).corner_radius(2.0));
                        if xr.on_hover_text("Close").clicked() { close_editor = true; }
                    });
                });
                // Make header draggable
                let hdr_rect = header_resp.response.rect;
                let drag_resp = ui.interact(hdr_rect, egui::Id::new(("ind_editor_drag", edit_id)), egui::Sense::drag());
                if drag_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }

                ui.add_space(6.0);

                // ── Per-type parameters ──
                let is_ma = matches!(ind.kind, IndicatorType::SMA | IndicatorType::EMA | IndicatorType::WMA | IndicatorType::DEMA | IndicatorType::TEMA);

                // MA type switcher (only for moving averages)
                if is_ma {
                    dialog_section(ui, "TYPE", m, t.dim.gamma_multiply(0.5));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let ma_kinds = [IndicatorType::SMA, IndicatorType::EMA, IndicatorType::WMA, IndicatorType::DEMA, IndicatorType::TEMA];
                        for (i, &kind) in ma_kinds.iter().enumerate() {
                            let sel = ind.kind == kind;
                            let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                            let bg = if sel { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_SUBTLE) };
                            let rounding = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                                else if i == ma_kinds.len() - 1 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                                else { egui::CornerRadius::ZERO };
                            if ui.add(egui::Button::new(egui::RichText::new(kind.label()).monospace().size(9.0).color(fg))
                                .fill(bg).corner_radius(rounding).min_size(egui::vec2(0.0, 22.0))
                                .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(t.accent, ALPHA_HEAVY) } else { color_alpha(t.toolbar_border, ALPHA_LINE) })))
                                .clicked() && !sel { ind.kind = kind; needs_recompute = true; }
                        }
                    });
                    ui.add_space(6.0);
                }

                // Band type switcher (BB ↔ Keltner)
                if matches!(ind.kind, IndicatorType::BollingerBands | IndicatorType::KeltnerChannels) {
                    dialog_section(ui, "TYPE", m, t.dim.gamma_multiply(0.5));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (i, &kind) in [IndicatorType::BollingerBands, IndicatorType::KeltnerChannels].iter().enumerate() {
                            let sel = ind.kind == kind;
                            let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                            let bg = if sel { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_SUBTLE) };
                            let rounding = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                                else { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } };
                            if ui.add(egui::Button::new(egui::RichText::new(kind.label()).monospace().size(9.0).color(fg))
                                .fill(bg).corner_radius(rounding).min_size(egui::vec2(0.0, 22.0))
                                .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(t.accent, ALPHA_HEAVY) } else { color_alpha(t.toolbar_border, ALPHA_LINE) })))
                                .clicked() && !sel { ind.kind = kind; needs_recompute = true; }
                        }
                    });
                    ui.add_space(6.0);
                }

                // ── PARAMETERS section ──
                dialog_section(ui, "PARAMETERS", m, t.dim.gamma_multiply(0.5));

                // Period (for most types except VWAP)
                if !matches!(ind.kind, IndicatorType::VWAP) {
                    let period_label = match ind.kind {
                        IndicatorType::MACD => "Fast",
                        IndicatorType::Ichimoku => "Tenkan",
                        _ => "Period",
                    };
                    let presets: &[usize] = match ind.kind {
                        IndicatorType::RSI => &[7, 9, 14, 21],
                        IndicatorType::MACD => &[8, 12, 16],
                        IndicatorType::Stochastic => &[5, 9, 14, 21],
                        IndicatorType::ADX => &[7, 14, 21],
                        IndicatorType::Ichimoku => &[7, 9, 13],
                        IndicatorType::Supertrend => &[7, 10, 14],
                        _ => &[9, 20, 50, 100, 200],
                    };
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new(period_label).monospace().size(9.0).color(t.dim));
                        ui.add_space(4.0);
                        let mut p = ind.period as i32;
                        if ui.add(egui::DragValue::new(&mut p).range(1..=500).speed(0.5)
                            .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                            ind.period = (p as usize).max(1); needs_recompute = true;
                        }
                        ui.add_space(6.0);
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for &pr in presets {
                            let sel = ind.period == pr;
                            let fg = if sel { t.accent } else { t.dim.gamma_multiply(0.5) };
                            if ui.add(egui::Button::new(egui::RichText::new(format!("{}", pr)).monospace().size(8.0).color(fg))
                                .fill(if sel { color_alpha(t.accent, ALPHA_SOFT) } else { egui::Color32::TRANSPARENT })
                                .corner_radius(2.0).min_size(egui::vec2(22.0, 18.0))).clicked() && !sel {
                                ind.period = pr; needs_recompute = true;
                            }
                        }
                    });
                }

                // Type-specific additional parameters
                match ind.kind {
                    IndicatorType::MACD => {
                        // Slow period (param2, default 26)
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Slow  ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 26.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(2.0..=200.0).speed(0.5)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        // Signal period (param3, default 9)
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Signal").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param3 > 0.0 { ind.param3 } else { 9.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=50.0).speed(0.3)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param3 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::Stochastic => {
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("%D    ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 3.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=20.0).speed(0.3)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::BollingerBands => {
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Std σ ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 2.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(0.5..=4.0).speed(0.05)
                                .custom_formatter(|v, _| format!("{:.1}", v))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                            ui.add_space(4.0);
                            for &s in &[1.0_f32, 1.5, 2.0, 2.5, 3.0] {
                                let cur = if ind.param2 > 0.0 { ind.param2 } else { 2.0 };
                                let sel = (cur - s).abs() < 0.01;
                                if ui.add(egui::Button::new(egui::RichText::new(format!("{:.1}", s)).monospace().size(8.0)
                                    .color(if sel { t.accent } else { t.dim.gamma_multiply(0.5) }))
                                    .fill(if sel { color_alpha(t.accent, ALPHA_SOFT) } else { egui::Color32::TRANSPARENT })
                                    .corner_radius(2.0).min_size(egui::vec2(22.0, 18.0))).clicked() {
                                    ind.param2 = s; needs_recompute = true;
                                }
                            }
                        });
                    }
                    IndicatorType::KeltnerChannels | IndicatorType::Supertrend => {
                        let def = if ind.kind == IndicatorType::Supertrend { 3.0 } else { 2.0 };
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Mult  ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { def };
                            if ui.add(egui::DragValue::new(&mut v).range(0.5..=6.0).speed(0.05)
                                .custom_formatter(|v, _| format!("{:.1}", v))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::Ichimoku => {
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Kijun ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 26.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=200.0).speed(0.5)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Senkou").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param3 > 0.0 { ind.param3 } else { 52.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=200.0).speed(0.5)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param3 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::ParabolicSAR => {
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Start ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param4 > 0.0 { ind.param4 } else { 0.02 };
                            if ui.add(egui::DragValue::new(&mut v).range(0.001..=0.1).speed(0.001)
                                .custom_formatter(|v, _| format!("{:.3}", v))).changed() {
                                ind.param4 = v; needs_recompute = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Step  ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 0.02 };
                            if ui.add(egui::DragValue::new(&mut v).range(0.001..=0.1).speed(0.001)
                                .custom_formatter(|v, _| format!("{:.3}", v))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Max   ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let mut v = if ind.param3 > 0.0 { ind.param3 } else { 0.2 };
                            if ui.add(egui::DragValue::new(&mut v).range(0.05..=0.5).speed(0.005)
                                .custom_formatter(|v, _| format!("{:.2}", v))).changed() {
                                ind.param3 = v; needs_recompute = true;
                            }
                        });
                    }
                    _ => {} // RSI, ADX, CCI, WilliamsR, ATR, VWAP — period only
                }

                // Source selection (for MAs, RSI, CCI)
                if is_ma || matches!(ind.kind, IndicatorType::RSI | IndicatorType::CCI | IndicatorType::BollingerBands) {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Source").monospace().size(9.0).color(t.dim));
                        ui.add_space(4.0);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let sources = [(0u8, "C"), (1, "O"), (2, "H"), (3, "L"), (4, "HL"), (5, "OHLC")];
                        for (i, &(src_id, lbl)) in sources.iter().enumerate() {
                            let sel = ind.source == src_id;
                            let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                            let bg = if sel { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_SUBTLE) };
                            let rounding = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                                else if i == sources.len() - 1 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                                else { egui::CornerRadius::ZERO };
                            if ui.add(egui::Button::new(egui::RichText::new(lbl).monospace().size(8.0).color(fg))
                                .fill(bg).corner_radius(rounding).min_size(egui::vec2(0.0, 20.0))
                                .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(t.accent, ALPHA_HEAVY) } else { color_alpha(t.toolbar_border, ALPHA_LINE) })))
                                .clicked() && !sel { ind.source = src_id; needs_recompute = true; }
                        }
                    });
                }

                // Timeframe source
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("TF    ").monospace().size(9.0).color(t.dim));
                    ui.add_space(4.0);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let tfs = INDICATOR_TIMEFRAMES;
                    for (i, &tf) in tfs.iter().enumerate() {
                        let label = if tf.is_empty() { "Chart" } else { tf };
                        let sel = ind.source_tf == tf;
                        let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                        let bg = if sel { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_SUBTLE) };
                        let rounding = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                            else if i == tfs.len() - 1 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                            else { egui::CornerRadius::ZERO };
                        if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(9.0).color(fg))
                            .fill(bg).corner_radius(rounding).min_size(egui::vec2(0.0, 20.0))
                            .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(t.accent, ALPHA_HEAVY) } else { color_alpha(t.toolbar_border, ALPHA_LINE) })))
                            .clicked() && !sel {
                            ind.source_tf = tf.to_string();
                            ind.source_loaded = tf.is_empty();
                            ind.source_bars.clear(); ind.source_timestamps.clear();
                            needs_recompute = true;
                            if !tf.is_empty() { needs_source_fetch = Some((pane_symbol.clone(), tf.to_string(), ind.id)); }
                        }
                    }
                });

                ui.add_space(8.0);
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, ALPHA_MUTED));
                ui.add_space(6.0);

                // ── APPEARANCE ──
                dialog_section(ui, "APPEARANCE", m, t.dim.gamma_multiply(0.5));
                ui.add_space(2.0);
                // Color
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for &c in INDICATOR_COLORS {
                        let color = hex_to_color(c, 1.0);
                        let is_cur = ind.color == c;
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                        if is_cur {
                            ui.painter().rect_filled(r, 3.0, color_alpha(color, ALPHA_TINT));
                            ui.painter().rect_stroke(r, 3.0, egui::Stroke::new(STROKE_STD, color), egui::StrokeKind::Outside);
                        }
                        ui.painter().circle_filled(r.center(), if is_cur { 5.0 } else { 4.0 }, color);
                        if resp.clicked() { ind.color = c.to_string(); }
                    }
                });
                ui.add_space(3.0);
                // Width + Style on one row
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let widths = [0.5_f32, 1.0, 1.5, 2.0, 3.0];
                    for (i, &th) in widths.iter().enumerate() {
                        let sel = (ind.thickness - th).abs() < 0.1;
                        let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                        let bg = if sel { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_SUBTLE) };
                        let rounding = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                            else if i == widths.len() - 1 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                            else { egui::CornerRadius::ZERO };
                        if ui.add(egui::Button::new(egui::RichText::new(format!("{:.1}", th)).monospace().size(8.0).color(fg))
                            .fill(bg).corner_radius(rounding).min_size(egui::vec2(26.0, 18.0))
                            .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(t.accent, ALPHA_HEAVY) } else { color_alpha(t.toolbar_border, ALPHA_LINE) })))
                            .clicked() { ind.thickness = th; }
                    }
                    ui.add_space(6.0);
                    let styles = [(LineStyle::Solid, "━"), (LineStyle::Dashed, "╌"), (LineStyle::Dotted, "┈")];
                    for (i, (ls, sym)) in styles.iter().enumerate() {
                        let sel = ind.line_style == *ls;
                        let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                        let bg = if sel { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_SUBTLE) };
                        let rounding = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                            else if i == styles.len() - 1 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                            else { egui::CornerRadius::ZERO };
                        if ui.add(egui::Button::new(egui::RichText::new(*sym).monospace().size(11.0).color(fg))
                            .fill(bg).corner_radius(rounding).min_size(egui::vec2(28.0, 18.0))
                            .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(t.accent, ALPHA_HEAVY) } else { color_alpha(t.toolbar_border, ALPHA_LINE) })))
                            .clicked() { ind.line_style = *ls; }
                    }
                });

                // ── BAND STYLING (BB / KC only) ──
                if matches!(ind.kind, IndicatorType::BollingerBands | IndicatorType::KeltnerChannels) {
                    ui.add_space(6.0);
                    dialog_section(ui, "BAND COLORS", m, t.dim.gamma_multiply(0.5));
                    ui.add_space(2.0);

                    let band_row = |ui: &mut egui::Ui, label: &str, color_field: &mut String, thickness_field: &mut f32, t: &Theme| {
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new(label).monospace().size(8.0).color(t.dim));
                            ui.add_space(4.0);
                            ui.spacing_mut().item_spacing.x = 3.0;
                            // Color swatches
                            for &c in INDICATOR_COLORS {
                                let col = hex_to_color(c, 1.0);
                                let is_cur = *color_field == c;
                                let (r, resp) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::click());
                                if is_cur {
                                    ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(STROKE_STD, col), egui::StrokeKind::Outside);
                                }
                                ui.painter().circle_filled(r.center(), if is_cur { 4.0 } else { 3.0 }, col);
                                if resp.clicked() { *color_field = c.to_string(); }
                            }
                            // "Auto" button (inherit from main color)
                            let is_auto = color_field.is_empty();
                            if ui.add(egui::Button::new(egui::RichText::new("auto").monospace().size(7.0)
                                .color(if is_auto { t.accent } else { t.dim.gamma_multiply(0.5) }))
                                .fill(if is_auto { color_alpha(t.accent, ALPHA_SOFT) } else { egui::Color32::TRANSPARENT })
                                .corner_radius(2.0).min_size(egui::vec2(24.0, 12.0))).clicked() {
                                *color_field = String::new();
                            }
                        });
                        // Thickness
                        ui.horizontal(|ui| {
                            ui.add_space(m + 44.0);
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (i, &th) in [0.5_f32, 0.8, 1.0, 1.5, 2.0].iter().enumerate() {
                                let cur = if *thickness_field > 0.0 { *thickness_field } else { 0.8 };
                                let sel = (cur - th).abs() < 0.1;
                                let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.6) };
                                let bg = if sel { color_alpha(t.accent, ALPHA_LINE) } else { color_alpha(t.toolbar_border, ALPHA_SOFT) };
                                let rounding = if i == 0 { egui::CornerRadius { nw: 2, sw: 2, ne: 0, se: 0 } }
                                    else if i == 4 { egui::CornerRadius { nw: 0, sw: 0, ne: 2, se: 2 } }
                                    else { egui::CornerRadius::ZERO };
                                if ui.add(egui::Button::new(egui::RichText::new(format!("{:.1}", th)).monospace().size(7.0).color(fg))
                                    .fill(bg).corner_radius(rounding).min_size(egui::vec2(22.0, 14.0))
                                    .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(t.accent, 80) } else { color_alpha(t.toolbar_border, ALPHA_MUTED) })))
                                    .clicked() { *thickness_field = th; }
                            }
                        });
                        ui.add_space(2.0);
                    };

                    band_row(ui, "Upper ", &mut ind.upper_color, &mut ind.upper_thickness, t);
                    band_row(ui, "Lower ", &mut ind.lower_color, &mut ind.lower_thickness, t);

                    // Fill color
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Fill  ").monospace().size(8.0).color(t.dim));
                        ui.add_space(4.0);
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for &c in INDICATOR_COLORS {
                            let col = hex_to_color(c, 1.0);
                            let is_cur = ind.fill_color_hex == c;
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::click());
                            if is_cur {
                                ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(STROKE_STD, col), egui::StrokeKind::Outside);
                            }
                            ui.painter().circle_filled(r.center(), if is_cur { 4.0 } else { 3.0 }, color_alpha(col, 80));
                            if resp.clicked() { ind.fill_color_hex = c.to_string(); }
                        }
                        let is_auto = ind.fill_color_hex.is_empty();
                        if ui.add(egui::Button::new(egui::RichText::new("auto").monospace().size(7.0)
                            .color(if is_auto { t.accent } else { t.dim.gamma_multiply(0.5) }))
                            .fill(if is_auto { color_alpha(t.accent, ALPHA_SOFT) } else { egui::Color32::TRANSPARENT })
                            .corner_radius(2.0).min_size(egui::vec2(24.0, 12.0))).clicked() {
                            ind.fill_color_hex = String::new();
                        }
                    });
                }

                ui.add_space(8.0);
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, ALPHA_MUTED));
                ui.add_space(4.0);

                // ── Footer: visibility + delete ──
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let vis_icon = if ind.visible { Icon::EYE } else { Icon::EYE_SLASH };
                    let vis_fg = if ind.visible { t.dim } else { t.dim.gamma_multiply(0.4) };
                    let vr = ui.add(egui::Button::new(egui::RichText::new(vis_icon).size(11.0).color(vis_fg))
                        .fill(if ind.visible { color_alpha(t.toolbar_border, ALPHA_SOFT) } else { egui::Color32::TRANSPARENT })
                        .corner_radius(3.0).min_size(egui::vec2(24.0, 22.0)));
                    if vr.on_hover_text("Toggle Visibility").clicked() { ind.visible = !ind.visible; }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let del_color = egui::Color32::from_rgb(224, 85, 96);
                        let dr = ui.add(egui::Button::new(egui::RichText::new(Icon::TRASH).size(11.0).color(del_color))
                            .fill(color_alpha(del_color, ALPHA_GHOST)).corner_radius(3.0)
                            .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(del_color, ALPHA_DIM)))
                            .min_size(egui::vec2(24.0, 22.0)));
                        if dr.on_hover_text("Delete Indicator").clicked() {
                            delete_id = Some(edit_id); close_editor = true;
                        }
                    });
                });
                ui.add_space(6.0);
            } else {
                close_editor = true;
            }
        });

    if close_editor { panes[ap].editing_indicator = None; }
    if let Some(id) = delete_id { panes[ap].indicators.retain(|i| i.id != id); }
    if needs_recompute { panes[ap].indicator_bar_count = 0; }
    if let Some((sym, tf, ind_id)) = needs_source_fetch {
        fetch_indicator_source(sym, tf, ind_id);
    }
}


}
