//! Indicator Editor UI component.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use crate::ui_kit::widgets::{Button, Tooltip};
use crate::ui_kit::widgets::tokens::{Variant, Size as KitSize};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::widgets::NumberStepper;
use super::super::inputs::form::{IndicatorParamRow, IndicatorParamRowF};
use crate::ui_kit::widgets::FormRow;
use super::super::inputs::inputs::{ColorSwatchPicker, ThicknessPicker};
use crate::ui_kit::widgets::SegmentedControl;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::LineStyle;

// P6.1 — was `const COLOR_DANGER` hardcoded (224,85,96); destructive
// action color now reads from `t.bear` at each call site so the picker
// respects the active palette's destructive semantic colour.

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Indicator editor popup (per-type properties panel) ──────────────────
// NOTE: do NOT shadow `t` here — the caller already passes the correct theme.
// Exit animation: de-gated from the outer `if let Some` so ToolOverlay can
// fade out after `editing_indicator` is cleared. We track the last-known
// edit_id and an open flag in egui memory so the fade-out frame can still
// render the correct content.
{
    let open_key    = egui::Id::new(("ind_editor_open", ap));
    let last_id_key = egui::Id::new(("ind_editor_last_id", ap));

    if let Some(id) = panes[ap].editing_indicator {
        ctx.memory_mut(|m| {
            m.data.insert_temp(open_key, true);
            m.data.insert_temp(last_id_key, id);
        });
    }

    let editor_open: bool = ctx.memory(|m| m.data.get_temp(open_key).unwrap_or(false));
    let Some(edit_id) = ctx.memory(|m| m.data.get_temp::<u32>(last_id_key)) else { return; };

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

    let id_str = format!("ind_editor_{}", edit_id);

    // Pre-compute header data so the body closure can mutably borrow `panes`.
    let (hdr_color, hdr_name) = panes[ap].indicators.iter().find(|i| i.id == edit_id)
        .map(|i| (hex_to_color(&i.color, 1.0), i.display_name()))
        .unwrap_or((egui::Color32::WHITE, String::new()));

    // P+ — switched from Modal + hand-rolled header_painter to ToolOverlay.
    // Header chrome (color dot, title, close button, drag cursor, divider)
    // is now centralised in ui_kit and shared with every other tool panel.
    let portable_t = crate::chart_renderer::theme_impl::theme_to_portable(t);
    let modal_resp = crate::ui_kit::widgets::ToolOverlay::new(&hdr_name)
        .id(&id_str)
        .width(panel_w)
        .pos(egui::pos2(200.0, 80.0))
        .accent_dot(hdr_color)
        .open(editor_open)
        .show(ctx, &portable_t, |ui| {
            if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.id == edit_id) {
                // Redesigned body (2026-05-26):
                //   • Single `BODY_PAD` constant for left/right padding.
                //   • Single `LABEL_W` constant for FormRow gutter so every
                //     row aligns its DragValue at the same x. Eliminates the
                //     40/44/48 zigzag that made the modal look sloppy.
                //   • Labels stripped of trailing-whitespace padding hacks.
                //   • Body wrapped in a Frame so the existing per-row
                //     `ui.add_space(m)` calls become natural padding.
                const BODY_PAD: f32 = 14.0;
                const LABEL_W:  f32 = 56.0;
                let m = 0.0_f32; // legacy var kept zero; pad comes from frame
                let _ = m;

                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left:   BODY_PAD as i8,
                        right:  BODY_PAD as i8,
                        top:    gap_sm() as i8,
                        bottom: gap_sm() as i8,
                    })
                    .show(ui, |ui| {
                ui.add_space(gap_xs());

                // ── Per-type parameters ──
                let is_ma = matches!(ind.kind, IndicatorType::SMA | IndicatorType::EMA | IndicatorType::WMA | IndicatorType::DEMA | IndicatorType::TEMA);

                // MA type switcher (only for moving averages)
                if is_ma {
                    dialog_section(ui, "TYPE", m, color_half(t.dim));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        const MA_KINDS: &[(IndicatorType, &str)] = &[
                            (IndicatorType::SMA, "SMA"), (IndicatorType::EMA, "EMA"),
                            (IndicatorType::WMA, "WMA"), (IndicatorType::DEMA, "DEMA"),
                            (IndicatorType::TEMA, "TEMA"),
                        ];
                        if SegmentedControl::new(&mut ind.kind, MA_KINDS)
                            .connected(true).compact(true)
                            .size(crate::ui_kit::widgets::tokens::Size::Xs)
                            .show(ui, t).changed()
                        {
                            needs_recompute = true;
                        }
                    });
                    ui.add_space(gap_sm());
                }

                // Band type switcher (BB ↔ Keltner)
                if matches!(ind.kind, IndicatorType::BollingerBands | IndicatorType::KeltnerChannels) {
                    dialog_section(ui, "TYPE", m, color_half(t.dim));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        const BAND_KINDS: &[(IndicatorType, &str)] = &[
                            (IndicatorType::BollingerBands, "BB"),
                            (IndicatorType::KeltnerChannels, "KC"),
                        ];
                        if SegmentedControl::new(&mut ind.kind, BAND_KINDS)
                            .connected(true).compact(true)
                            .size(crate::ui_kit::widgets::tokens::Size::Xs)
                            .show(ui, t).changed()
                        {
                            needs_recompute = true;
                        }
                    });
                    ui.add_space(gap_sm());
                }

                // ── PARAMETERS section ──
                dialog_section(ui, "PARAMETERS", m, color_half(t.dim));

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
                        FormRow::new("Slow").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 26.0 };
                            if NumberStepper::new(&mut v).range(2.0..=200.0).step(0.5).integer().show(ui, t).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        // Signal period (param3, default 9)
                        FormRow::new("Signal").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param3 > 0.0 { ind.param3 } else { 9.0 };
                            if NumberStepper::new(&mut v).range(1.0..=50.0).step(0.3).integer().show(ui, t).changed() {
                                ind.param3 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::Stochastic => {
                        FormRow::new("%D").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 3.0 };
                            if NumberStepper::new(&mut v).range(1.0..=20.0).step(0.3).integer().show(ui, t).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::BollingerBands => {
                        const BB_STD_PRESETS: &[f32] = &[1.0, 1.5, 2.0, 2.5, 3.0];
                        if IndicatorParamRowF::new("Std σ", &mut ind.param2, 2.0)
                            .indent(m).presets(BB_STD_PRESETS).range(0.5, 4.0).speed(0.05).decimals(1)
                            .theme(t).show(ui)
                        {
                            needs_recompute = true;
                        }
                    }
                    IndicatorType::KeltnerChannels | IndicatorType::Supertrend => {
                        let def = if ind.kind == IndicatorType::Supertrend { 3.0 } else { 2.0 };
                        FormRow::new("Mult").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { def };
                            if NumberStepper::new(&mut v).range(0.5..=6.0).step(0.05).decimals(1).show(ui, t).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::Ichimoku => {
                        FormRow::new("Kijun").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 26.0 };
                            if NumberStepper::new(&mut v).range(1.0..=200.0).step(0.5).integer().show(ui, t).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        FormRow::new("Senkou").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param3 > 0.0 { ind.param3 } else { 52.0 };
                            if NumberStepper::new(&mut v).range(1.0..=200.0).step(0.5).integer().show(ui, t).changed() {
                                ind.param3 = v; needs_recompute = true;
                            }
                        });
                    }
                    IndicatorType::ParabolicSAR => {
                        FormRow::new("Start").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param4 > 0.0 { ind.param4 } else { 0.02 };
                            if NumberStepper::new(&mut v).range(0.001..=0.1).step(0.001).decimals(3).show(ui, t).changed() {
                                ind.param4 = v; needs_recompute = true;
                            }
                        });
                        FormRow::new("Step").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param2 > 0.0 { ind.param2 } else { 0.02 };
                            if NumberStepper::new(&mut v).range(0.001..=0.1).step(0.001).decimals(3).show(ui, t).changed() {
                                ind.param2 = v; needs_recompute = true;
                            }
                        });
                        FormRow::new("Max").leading_space(m).label_width(LABEL_W).show(ui, t, |ui| {
                            let mut v = if ind.param3 > 0.0 { ind.param3 } else { 0.2 };
                            if NumberStepper::new(&mut v).range(0.05..=0.5).step(0.005).decimals(2).show(ui, t).changed() {
                                ind.param3 = v; needs_recompute = true;
                            }
                        });
                    }
                    _ => {} // RSI, ADX, CCI, WilliamsR, ATR, VWAP — period only
                }

                // Source selection (for MAs, RSI, CCI)
                if is_ma || matches!(ind.kind, IndicatorType::RSI | IndicatorType::CCI | IndicatorType::BollingerBands) {
                    ui.add_space(gap_xs());
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Source").monospace().size(font_sm()).color(t.dim));
                        ui.add_space(gap_xs());
                        ui.spacing_mut().item_spacing.x = 0.0;
                        const SOURCES: &[(u8, &str)] = &[
                            (0, "C"), (1, "O"), (2, "H"), (3, "L"), (4, "HL"), (5, "OHLC"),
                        ];
                        if SegmentedControl::new(&mut ind.source, SOURCES)
                            .connected(true).compact(true).size(crate::ui_kit::widgets::tokens::Size::Xs)
                            .show(ui, t).changed()
                        {
                            needs_recompute = true;
                        }
                    });
                }

                // Timeframe source — Button::toggle handles its own rendering;
                // the per-item `fg / bg / rounding / stroke_col` locals were
                // dead (computed but never passed to Button::toggle).
                ui.add_space(gap_xs());
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("TF    ").monospace().size(font_sm()).color(t.dim));
                    ui.add_space(gap_sm());
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for &tf in INDICATOR_TIMEFRAMES.iter() {
                        let label = if tf.is_empty() { "Chart" } else { tf };
                        let sel = ind.source_tf == tf;
                        if Button::toggle(label, sel).size(KitSize::Sm).show(ui, t)
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

                ui.add_space(gap_sm());
                dialog_separator_shadow(ui, m, tint(t, Tone::Border, alpha_muted()));
                ui.add_space(gap_sm());

                // ── APPEARANCE ──
                dialog_section(ui, "APPEARANCE", m, color_half(t.dim));
                ui.add_space(gap_xs());
                // Color
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ColorSwatchPicker::new(&mut ind.color)
                        .palette(INDICATOR_COLORS)
                        .swatch_size(16.0).dot_radius(4.0)
                        .theme(t).show(ui);
                });
                ui.add_space(gap_xs());
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
                    SegmentedControl::new(&mut ind.line_style, LINE_STYLES)
                        .connected(true).compact(true).size(crate::ui_kit::widgets::tokens::Size::Xs)
                        .show(ui, t);
                });

                // ── BAND STYLING (BB / KC only) ──
                if matches!(ind.kind, IndicatorType::BollingerBands | IndicatorType::KeltnerChannels) {
                    ui.add_space(gap_sm());
                    dialog_section(ui, "BAND COLORS", m, color_half(t.dim));
                    ui.add_space(gap_xs());

                    const BAND_WIDTHS: &[f32] = &[0.5, 0.8, 1.0, 1.5, 2.0];
                    let band_row = |ui: &mut egui::Ui, label: &str, color_field: &mut String, thickness_field: &mut f32| {
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
                        ui.add_space(gap_xs());
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

                ui.add_space(gap_sm());
                dialog_separator_shadow(ui, m, tint(t, Tone::Border, alpha_muted()));
                ui.add_space(gap_xs());

                // ── Footer: visibility + delete ──
                // Both buttons previously had 6+ overrides each (glyph_color, fill,
                // corner_radius, stroke, min_size, frameless). Replaced with the
                // semantic primitives: Toggle for vis, tone_destructive for delete.
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let vis_icon = if ind.visible { Icon::EYE } else { Icon::EYE_SLASH };
                    let vr = Button::icon(vis_icon)
                        .variant(Variant::Toggle)
                        .active(ind.visible)
                        .size(KitSize::Sm)
                        .placement(IconPlacement::PanelHeader)
                        .show(ui, t);
                    Tooltip::new(if ind.visible { "Hide indicator" } else { "Show indicator" })
                        .show(ui, &vr, t);
                    if vr.clicked() { ind.visible = !ind.visible; }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        let dr = Button::icon(Icon::TRASH)
                            .variant(Variant::Ghost)
                            .size(KitSize::Sm)
                            .placement(IconPlacement::PanelHeader)
                            .tone_destructive()
                            .show(ui, t);
                        Tooltip::new("Delete Indicator").show(ui, &dr, t);
                        if dr.clicked() {
                            delete_id = Some(edit_id); close_editor = true;
                        }
                    });
                });
                ui.add_space(gap_sm());
                    }); // end body Frame
            } else {
                close_editor = true;
            }
        });

    if modal_resp.closed { close_editor = true; }

    #[cfg(debug_assertions)]
    if let Some(drect) = modal_resp.rect {
        crate::dev_inspector::record(crate::dev_inspector::WidgetRecord {
            id: format!("dialog.indicator_editor.{ap}"),
            role: "dialog".into(),
            synthetic: false,
            label: hdr_name.clone(),
            value: None,
            rect: drect.into(),
            clip_rect: crate::dev_inspector::SerRect::zero(),
            layer: 1, focused: false, hovered: false, enabled: true,
            is_clipped: false, style_class: None,
            ticker: false,
        });
    }

    if close_editor {
        // Clear immediately so data model is clean; ToolOverlay fades out using
        // the last-known content still in egui memory.
        ctx.memory_mut(|m| m.data.insert_temp(open_key, false));
        panes[ap].editing_indicator = None;
    }
    if let Some(id) = delete_id { panes[ap].indicators.retain(|i| i.id != id); }
    if needs_recompute { panes[ap].indicator_bar_count = 0; }
    if let Some((sym, tf, ind_id)) = needs_source_fetch {
        fetch_indicator_source(sym, tf, ind_id);
    }
}


}

