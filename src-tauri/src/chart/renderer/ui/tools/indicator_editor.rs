//! Indicator Editor UI component.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use crate::ui_kit::widgets::{Button, Tooltip};
use crate::ui_kit::widgets::tokens::{Variant, Size as KitSize};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::widgets::NumberStepper;
use super::super::inputs::form::{IndicatorParamRow, IndicatorParamRowF};
use crate::ui_kit::widgets::FormRow;
use super::super::inputs::inputs::{ColorSwatchPicker, ThicknessPicker};
use super::super::chrome::modal::{Modal, Anchor, FrameKind, HeaderStyle};
use super::super::components::frames_widget::PopupFrame;
use super::super::inputs::select::SegmentedControl;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::LineStyle;

// P6.1 — was `const COLOR_DANGER` hardcoded (224,85,96); destructive
// action color now reads from `t.bear` at each call site so the picker
// respects the active palette's destructive semantic colour.

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Indicator editor popup (per-type properties panel) ──────────────────
// NOTE: do NOT shadow `t` here — the caller already passes the correct theme.
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

    let frame = PopupFrame::new()
        .colors(t.toolbar_bg, t.toolbar_border)
        .ctx(ctx)
        .no_inner_margin()
        .corner_radius(current().r_md as f32)
        .build();

    let id_str = format!("ind_editor_{}", edit_id);

    // Pre-compute header data so the painter closure doesn't need to
    // borrow `panes` (the body closure borrows it mutably).
    // WHITE is a never-rendered placeholder: the closing branch (no indicator
    // found) sets close_editor = true and the dot is never painted.
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
            // Redesigned header (2026-05-26):
            //   • Full-width fill (ui.max_rect()), not the stale panel_w.
            //   • No ui.interact() wrapper — that was blocking BOTH the close
            //     button (click eaten by drag-sense) AND egui::Window's own
            //     movable(true) handler. Window drag now works as designed;
            //     cursor hint is set only when hovering empty header area.
            //   • Color dot painted via allocate_space, not magic cursor math.
            //   • Header strip is 28 px tall — taller hit target.
            let mut hdr_close = false;
            const HEADER_H: f32 = 28.0;
            let avail_w = ui.available_width();
            let (header_rect, header_resp) = ui.allocate_exact_size(
                egui::vec2(avail_w, HEADER_H),
                egui::Sense::hover(),
            );
            // Background fill — accent-tinted strip across the full inner width.
            let r_top = current().r_md;
            ui.painter().rect_filled(
                header_rect,
                egui::CornerRadius { nw: r_top, ne: r_top, sw: 0, se: 0 },
                color_alpha(t.toolbar_border, alpha_tint()),
            );
            // Hairline divider beneath the header — separates from body.
            ui.painter().hline(
                header_rect.x_range(),
                header_rect.bottom(),
                egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_strong())),
            );
            // Hover anywhere on the strip (outside the close button) → drag cursor.
            // egui::Window's movable(true) consumes the drag event itself.
            if header_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            // Color dot at a fixed offset from header_rect.left().
            let dot_center = egui::pos2(
                header_rect.left() + gap_md() + 4.0,
                header_rect.center().y,
            );
            ui.painter().circle_filled(dot_center, 4.5, hdr_color);
            // Title — centered vertically, just right of the dot.
            ui.painter().text(
                egui::pos2(header_rect.left() + gap_md() + 16.0, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &hdr_name,
                egui::FontId::monospace(font_sm()),
                t.text,
            );
            // Close button — right-anchored in its own child UI so the
            // Button widget gets a proper interactable region INSIDE the
            // header rect WITHOUT a wrapping interact swallowing the click.
            let close_size = 22.0_f32;
            let close_rect = egui::Rect::from_min_size(
                egui::pos2(header_rect.right() - close_size - gap_xs(),
                           header_rect.center().y - close_size / 2.0),
                egui::vec2(close_size, close_size),
            );
            let mut close_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(close_rect)
                    .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown))
            );
            let close_resp = Button::icon(Icon::X)
                .variant(Variant::Ghost)
                .placement(IconPlacement::Modal)
                .show(&mut close_ui, t);
            Tooltip::new("Close").show(ui, &close_resp, t);
            if close_resp.clicked() { hdr_close = true; }
            hdr_close
        })
        .show(|ui| {
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
                        if SegmentedControl::new().options(MA_KINDS).connected_pills(true).compact(true)
                            .height(row_height_compact()).theme(t).show(ui, &mut ind.kind) {
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
                        if SegmentedControl::new().options(BAND_KINDS).connected_pills(true).compact(true)
                            .height(row_height_compact()).theme(t).show(ui, &mut ind.kind) {
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
                        if SegmentedControl::new().options(SOURCES).connected_pills(true).compact(true)
                            .height(row_height_compact()).theme(t).show(ui, &mut ind.source) {
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
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, alpha_muted()));
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
                    SegmentedControl::new().options(LINE_STYLES).connected_pills(true).compact(true)
                        .height(row_height_compact()).theme(t).show(ui, &mut ind.line_style);
                });

                // ── BAND STYLING (BB / KC only) ──
                if matches!(ind.kind, IndicatorType::BollingerBands | IndicatorType::KeltnerChannels) {
                    ui.add_space(gap_sm());
                    dialog_section(ui, "BAND COLORS", m, color_half(t.dim));
                    ui.add_space(gap_xs());

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
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, alpha_muted()));
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

    if close_editor { panes[ap].editing_indicator = None; }
    if let Some(id) = delete_id { panes[ap].indicators.retain(|i| i.id != id); }
    if needs_recompute { panes[ap].indicator_bar_count = 0; }
    if let Some((sym, tf, ind_id)) = needs_source_fetch {
        fetch_indicator_source(sym, tf, ind_id);
    }
}


}

