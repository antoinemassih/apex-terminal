//! Indicator Editor UI component.

use egui;
use super::style::*;
use super::super::gpu::*;
use super::widgets::buttons::ChromeBtn;
use super::widgets::form::{FormRow, IndicatorParamRow, IndicatorParamRowF};
use super::widgets::inputs::{ColorSwatchPicker, ThicknessPicker};
use super::widgets::modal::{Modal, Anchor, FrameKind, HeaderStyle};
use super::widgets::select::SegmentedControl;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::LineStyle;

/// Danger red — delete / destructive actions.
const COLOR_DANGER: egui::Color32 = egui::Color32::from_rgb(224, 85, 96);

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
        Some(IndicatorType::MACD) | Some(IndicatorType::Ichimoku) => 290.0,
        _ => 250.0,
    };

    let frame = egui::Frame::popup(&ctx.style())
        .fill(t.toolbar_bg)
        .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
        .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_heavy())))
        .corner_radius(r_md_cr());

    let id_str = format!("ind_editor_{}", edit_id);

    // Pre-compute header data so the painter closure doesn't need to
    // borrow `panes` (the body closure borrows it mutably).
    let (hdr_color, hdr_name) = panes[ap].indicators.iter().find(|i| i.id == edit_id)
        .map(|i| (hex_to_color(&i.color, 1.0), i.display_name()))
        .unwrap_or((egui::Color32::WHITE, String::new()));

    let modal_resp = Modal::new(&id_str)
        .ctx(ctx)
        .theme(t)
        .id(&id_str)
        .anchor(Anchor::Window { pos: Some(egui::pos2(200.0, 80.0)) })
        .size(egui::vec2(panel_w, 0.0))
        .frame_kind(FrameKind::Custom(frame))
        .header_style(HeaderStyle::None)
        .separator(false)
        .draggable_header(true)
        .header_painter(|ui| {
            // Custom header strip — preserved byte-for-byte from the original.
            let mut hdr_close = false;
            let header_resp = ui.horizontal(|ui| {
                ui.set_min_width(panel_w);
                let hr = ui.max_rect();
                let r_top = current().r_md;
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(hr.min, egui::vec2(panel_w, 26.0)),
                    egui::CornerRadius { nw: r_top, ne: r_top, sw: 0, se: 0 },
                    color_alpha(t.toolbar_border, alpha_tint()));
                ui.add_space(6.0);
                // Color dot — uses the editing indicator's color (pre-fetched).
                ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 10.0), 4.0, hdr_color);
                ui.add_space(10.0);
                ui.label(egui::RichText::new(&hdr_name).monospace().size(font_sm()).strong().color(TEXT_PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    if icon_btn(ui, Icon::X, t.dim.gamma_multiply(0.7), FONT_LG).on_hover_text("Close").clicked() {
                        hdr_close = true;
                    }
                });
            });
            // Make header draggable — interact for grab cursor; egui::Window
            // movable(true) (set via Modal::draggable_header) handles motion.
            let hdr_rect = header_resp.response.rect;
            let drag_resp = ui.interact(hdr_rect, egui::Id::new(("ind_editor_drag", edit_id)), egui::Sense::drag());
            if drag_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }
            hdr_close
        })
        .show(|ui| {
            if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.id == edit_id) {
                let m = 8.0;

                ui.add_space(6.0);

                // ── Per-type parameters ──
                let is_ma = matches!(ind.kind, IndicatorType::SMA | IndicatorType::EMA | IndicatorType::WMA | IndicatorType::DEMA | IndicatorType::TEMA);

                // MA type switcher (only for moving averages)
                if is_ma {
                    dialog_section(ui, "TYPE", m, t.dim.gamma_multiply(0.5));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        const MA_KINDS: &[(IndicatorType, &str)] = &[
                            (IndicatorType::SMA, "SMA"), (IndicatorType::EMA, "EMA"),
                            (IndicatorType::WMA, "WMA"), (IndicatorType::DEMA, "DEMA"),
                            (IndicatorType::TEMA, "TEMA"),
                        ];
                        if SegmentedControl::new().options(MA_KINDS).connected_pills(true).compact(true)
                            .height(22.0).theme(t).show(ui, &mut ind.kind) {
                            needs_recompute = true;
                        }
                    });
                    ui.add_space(6.0);
                }

                // Band type switcher (BB ↔ Keltner)
                if matches!(ind.kind, IndicatorType::BollingerBands | IndicatorType::KeltnerChannels) {
                    dialog_section(ui, "TYPE", m, t.dim.gamma_multiply(0.5));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        const BAND_KINDS: &[(IndicatorType, &str)] = &[
                            (IndicatorType::BollingerBands, "BB"),
                            (IndicatorType::KeltnerChannels, "KC"),
                        ];
                        if SegmentedControl::new().options(BAND_KINDS).connected_pills(true).compact(true)
                            .height(22.0).theme(t).show(ui, &mut ind.kind) {
                            needs_recompute = true;
                        }
                    });
                    ui.add_space(6.0);
                }

                // ── PARAMETERS section ──
                dialog_section(ui, "PARAMETERS", m, t.dim.gamma_multiply(0.5));

                // Period (for most types except VWAP) — label + DragValue + presets.
                // FormRow handles indent + label gutter; body contains DragValue + presets.
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
                    if IndicatorParamRow::new(period_label, &mut ind.period)
                        .indent(m).presets(presets).range(1, 500).speed(0.5)
                        .theme(t).show(ui)
                    {
                        needs_recompute = true;
                    }
                }

                // Type-specific additional parameters — simple label+DragValue rows
                // migrated to FormRow with explicit indent. DragValue stays inline.
                match ind.kind {
                    IndicatorType::MACD => {
                        // Slow period (param2, default 26)
                        FormRow::new("Slow  ").indent(m).label_width(40.0).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 26.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(2.0..=200.0).speed(0.5)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        // Signal period (param3, default 9)
                        FormRow::new("Signal").indent(m).label_width(40.0).show(ui, t, |ui| {
                            let mut v = if ind.param3 > 0.0 { ind.param3 } else { 9.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=50.0).speed(0.3)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param3 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::Stochastic => {
                        FormRow::new("%D    ").indent(m).label_width(40.0).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 3.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=20.0).speed(0.3)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::BollingerBands => {
                        const BB_STD_PRESETS: &[f32] = &[1.0, 1.5, 2.0, 2.5, 3.0];
                        if IndicatorParamRowF::new("Std σ ", &mut ind.param2, 2.0)
                            .indent(m).presets(BB_STD_PRESETS).range(0.5, 4.0).speed(0.05).decimals(1)
                            .theme(t).show(ui)
                        {
                            needs_recompute = true;
                        }
                    }
                    IndicatorType::KeltnerChannels | IndicatorType::Supertrend => {
                        let def = if ind.kind == IndicatorType::Supertrend { 3.0 } else { 2.0 };
                        FormRow::new("Mult  ").indent(m).label_width(40.0).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { def };
                            if ui.add(egui::DragValue::new(&mut v).range(0.5..=6.0).speed(0.05)
                                .custom_formatter(|v, _| format!("{:.1}", v))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::Ichimoku => {
                        FormRow::new("Kijun ").indent(m).label_width(48.0).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 26.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=200.0).speed(0.5)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        FormRow::new("Senkou").indent(m).label_width(48.0).show(ui, t, |ui| {
                            let mut v = if ind.param3 > 0.0 { ind.param3 } else { 52.0 };
                            if ui.add(egui::DragValue::new(&mut v).range(1.0..=200.0).speed(0.5)
                                .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                                ind.param3 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::ParabolicSAR => {
                        FormRow::new("Start ").indent(m).label_width(44.0).show(ui, t, |ui| {
                            let mut v = if ind.param4 > 0.0 { ind.param4 } else { 0.02 };
                            if ui.add(egui::DragValue::new(&mut v).range(0.001..=0.1).speed(0.001)
                                .custom_formatter(|v, _| format!("{:.3}", v))).changed() {
                                ind.param4 = v; needs_recompute = true;
                            }
                        });
                        FormRow::new("Step  ").indent(m).label_width(44.0).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 0.02 };
                            if ui.add(egui::DragValue::new(&mut v).range(0.001..=0.1).speed(0.001)
                                .custom_formatter(|v, _| format!("{:.3}", v))).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        FormRow::new("Max   ").indent(m).label_width(44.0).show(ui, t, |ui| {
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
                        ui.label(egui::RichText::new("Source").monospace().size(font_sm()).color(t.dim));
                        ui.add_space(4.0);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        const SOURCES: &[(u8, &str)] = &[
                            (0, "C"), (1, "O"), (2, "H"), (3, "L"), (4, "HL"), (5, "OHLC"),
                        ];
                        if SegmentedControl::new().options(SOURCES).connected_pills(true).compact(true)
                            .height(20.0).theme(t).show(ui, &mut ind.source) {
                            needs_recompute = true;
                        }
                    });
                }

                // Timeframe source
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("TF    ").monospace().size(font_sm()).color(t.dim));
                    ui.add_space(gap_sm());
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let tfs = INDICATOR_TIMEFRAMES;
                    let n = tfs.len();
                    let r_sm = current().r_sm;
                    for (i, &tf) in tfs.iter().enumerate() {
                        let label = if tf.is_empty() { "Chart" } else { tf };
                        let sel = ind.source_tf == tf;
                        let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                        let bg = if sel { color_alpha(t.accent, alpha_dim()) } else { color_alpha(t.toolbar_border, alpha_subtle()) };
                        let rounding = if i == 0 {
                            egui::CornerRadius { nw: r_sm, sw: r_sm, ne: 0, se: 0 }
                        } else if i == n - 1 {
                            egui::CornerRadius { nw: 0, sw: 0, ne: r_sm, se: r_sm }
                        } else {
                            egui::CornerRadius::ZERO
                        };
                        let stroke_col = if sel { color_alpha(t.accent, alpha_heavy()) } else { color_alpha(t.toolbar_border, alpha_line()) };
                        if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(font_sm()).color(fg))
                            .fill(bg).corner_radius(rounding).min_size(egui::vec2(0.0, 20.0))
                            .stroke(egui::Stroke::new(stroke_thin(), stroke_col)))
                            .clicked() && !sel
                        {
                            ind.source_tf = tf.to_string();
                            ind.source_loaded = tf.is_empty();
                            ind.source_bars.clear(); ind.source_timestamps.clear();
                            needs_recompute = true;
                            if !tf.is_empty() { needs_source_fetch = Some((pane_symbol.clone(), tf.to_string(), ind.id)); }
                        }
                    }
                });

                ui.add_space(6.0);
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, alpha_muted()));
                ui.add_space(6.0);

                // ── APPEARANCE ──
                dialog_section(ui, "APPEARANCE", m, t.dim.gamma_multiply(0.5));
                ui.add_space(2.0);
                // Color
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ColorSwatchPicker::new(&mut ind.color)
                        .palette(INDICATOR_COLORS)
                        .swatch_size(16.0).dot_radius(4.0)
                        .theme(t).show(ui);
                });
                ui.add_space(3.0);
                // Width + Style on one row
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    const MAIN_WIDTHS: &[f32] = &[0.5, 1.0, 1.5, 2.0, 3.0];
                    ThicknessPicker::new(&mut ind.thickness)
                        .values(MAIN_WIDTHS).height(18.0).min_btn_w(26.0)
                        .theme(t).show(ui);
                    ui.add_space(gap_md());
                    const LINE_STYLES: &[(LineStyle, &str)] = &[
                        (LineStyle::Solid, "━"), (LineStyle::Dashed, "╌"), (LineStyle::Dotted, "┈"),
                    ];
                    SegmentedControl::new().options(LINE_STYLES).connected_pills(true).compact(true)
                        .height(18.0).theme(t).show(ui, &mut ind.line_style);
                });

                // ── BAND STYLING (BB / KC only) ──
                if matches!(ind.kind, IndicatorType::BollingerBands | IndicatorType::KeltnerChannels) {
                    ui.add_space(6.0);
                    dialog_section(ui, "BAND COLORS", m, t.dim.gamma_multiply(0.5));
                    ui.add_space(2.0);

                    const BAND_WIDTHS: &[f32] = &[0.5, 0.8, 1.0, 1.5, 2.0];
                    let mut band_row = |ui: &mut egui::Ui, label: &str, color_field: &mut String, thickness_field: &mut f32| {
                        // Color swatch row
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new(label).monospace().size(font_xs()).color(t.dim));
                            ui.add_space(gap_sm());
                            ColorSwatchPicker::new(color_field)
                                .palette(INDICATOR_COLORS)
                                .swatch_size(12.0).dot_radius(3.0).auto_button(true)
                                .theme(t).show(ui);
                        });
                        // Thickness row (indented to align under swatches)
                        ui.horizontal(|ui| {
                            ui.add_space(m + 44.0);
                            // Normalise 0.0 sentinel to default
                            if *thickness_field <= 0.0 { *thickness_field = 0.8; }
                            ThicknessPicker::new(thickness_field)
                                .values(BAND_WIDTHS).height(14.0).font_size(7.0).min_btn_w(22.0)
                                .theme(t).show(ui);
                        });
                        ui.add_space(2.0);
                    };

                    band_row(ui, "Upper ", &mut ind.upper_color, &mut ind.upper_thickness);
                    band_row(ui, "Lower ", &mut ind.lower_color, &mut ind.lower_thickness);

                    // Fill color (semi-transparent dots to hint alpha fill)
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Fill  ").monospace().size(font_xs()).color(t.dim));
                        ui.add_space(gap_sm());
                        ColorSwatchPicker::new(&mut ind.fill_color_hex)
                            .palette(INDICATOR_COLORS)
                            .swatch_size(12.0).dot_radius(3.0).fill_alpha(80).auto_button(true)
                            .theme(t).show(ui);
                    });
                }

                ui.add_space(6.0);
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, alpha_muted()));
                ui.add_space(4.0);

                // ── Footer: visibility + delete ──
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let vis_icon = if ind.visible { Icon::EYE } else { Icon::EYE_SLASH };
                    let vis_fg = if ind.visible { t.dim } else { t.dim.gamma_multiply(0.4) };
                    let vr = ui.add(ChromeBtn::new(egui::RichText::new(vis_icon).size(font_sm()).color(vis_fg))
                        .fill(if ind.visible { color_alpha(t.toolbar_border, alpha_soft()) } else { egui::Color32::TRANSPARENT })
                        .corner_radius(r_sm_cr()).min_size(egui::vec2(24.0, 22.0)));
                    if vr.on_hover_text("Toggle Visibility").clicked() { ind.visible = !ind.visible; }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let del_color = COLOR_DANGER;
                        let dr = ui.add(ChromeBtn::new(egui::RichText::new(Icon::TRASH).size(font_sm()).color(del_color))
                            .fill(color_alpha(del_color, alpha_ghost())).corner_radius(r_sm_cr())
                            .stroke(egui::Stroke::new(stroke_thin(), color_alpha(del_color, alpha_dim())))
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

    if modal_resp.closed { close_editor = true; }

    if close_editor { panes[ap].editing_indicator = None; }
    if let Some(id) = delete_id { panes[ap].indicators.retain(|i| i.id != id); }
    if needs_recompute { panes[ap].indicator_bar_count = 0; }
    if let Some((sym, tf, ind_id)) = needs_source_fetch {
        fetch_indicator_source(sym, tf, ind_id);
    }
}


}

