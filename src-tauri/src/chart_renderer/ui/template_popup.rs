//! Template popup — floating panel opened from the pane header ★ button.
//! Shows saved chart templates with apply + delete, and "Save Current" input.

use egui;
use super::style::*;
use super::super::gpu::{self, Watchlist, Chart, Theme, CandleMode, VolumeProfileMode, Indicator, IndicatorType, INDICATOR_COLORS};
use crate::ui_kit::icons::Icon;

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    _ap: usize,
    t: &Theme,
) {
    for pi in 0..panes.len() {
        if !panes[pi].template_popup_open { continue; }

        let pos = panes[pi].template_popup_pos;
        let mut close_popup = false;
        let mut apply_idx: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;

        let win_resp = egui::Area::new(egui::Id::new(("template_popup", pi)))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(&ctx.style())
                    .fill(t.toolbar_bg)
                    .stroke(egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_HEAVY)))
                    .inner_margin(egui::Margin::same(GAP_LG as i8))
                    .corner_radius(RADIUS_LG)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4], blur: 14, spread: 0,
                        color: egui::Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| {
                        ui.set_width(220.0);

                        // Header
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("TEMPLATES").monospace().size(FONT_LG).strong().color(t.accent));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if close_button(ui, t.dim) { close_popup = true; }
                            });
                        });
                        ui.add_space(GAP_SM);

                        // ── Pane Type selector ──
                        section_label(ui, "PANE TYPE", t.dim);
                        ui.add_space(GAP_XS);
                        ui.horizontal(|ui| {
                            for (ptype, label, icon) in [
                                (super::super::gpu::PaneType::Chart, "Chart", Icon::CHART_LINE),
                                (super::super::gpu::PaneType::Portfolio, "Portfolio", Icon::LIST),
                                (super::super::gpu::PaneType::Dashboard, "Dashboard", "\u{2637}"),
                                (super::super::gpu::PaneType::Heatmap, "Heatmap", "\u{2593}"),
                            ] {
                                let active = panes[pi].pane_type == ptype;
                                let fg = if active { t.accent } else { t.dim.gamma_multiply(0.5) };
                                let bg = if active { color_alpha(t.accent, ALPHA_TINT) } else { egui::Color32::TRANSPARENT };
                                if ui.add(egui::Button::new(egui::RichText::new(format!("{} {}", icon, label))
                                    .monospace().size(FONT_XS).color(fg))
                                    .fill(bg).corner_radius(RADIUS_SM)
                                    .stroke(egui::Stroke::new(if active { STROKE_THIN } else { 0.0 },
                                        if active { color_alpha(t.accent, ALPHA_LINE) } else { egui::Color32::TRANSPARENT }))
                                    .min_size(egui::vec2(0.0, 20.0))).clicked() {
                                    panes[pi].pane_type = ptype;
                                }
                            }
                        });
                        ui.add_space(GAP_SM);
                        separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
                        ui.add_space(GAP_SM);

                        // Template list
                        if watchlist.pane_templates.is_empty() {
                            ui.label(egui::RichText::new("No saved templates")
                                .monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
                            ui.add_space(GAP_SM);
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt(("tmpl_scroll", pi))
                                .max_height(200.0)
                                .show(ui, |ui| {
                                    for (i, (name, tmpl)) in watchlist.pane_templates.iter().enumerate() {
                                        // Build indicator summary
                                        let summary: String = tmpl.get("indicators")
                                            .and_then(|v| v.as_array())
                                            .map(|arr| {
                                                let names: Vec<&str> = arr.iter()
                                                    .filter_map(|ind| ind.get("kind").and_then(|v| v.as_str()))
                                                    .take(4)
                                                    .collect();
                                                let s = names.join(", ");
                                                if arr.len() > 4 { format!("{} +{}", s, arr.len() - 4) } else { s }
                                            })
                                            .unwrap_or_else(|| "—".into());

                                        let row_resp = ui.horizontal(|ui| {
                                            // Click the whole row to apply
                                            let row_rect = ui.available_rect_before_wrap();
                                            let full_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(ui.available_width(), 32.0));

                                            ui.vertical(|ui| {
                                                ui.set_min_height(28.0);
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new(Icon::STAR)
                                                        .size(FONT_SM).color(t.accent));
                                                    ui.label(egui::RichText::new(name)
                                                        .monospace().size(FONT_SM).strong().color(TEXT_PRIMARY));
                                                });
                                                ui.label(egui::RichText::new(&summary)
                                                    .monospace().size(FONT_XS).color(t.dim.gamma_multiply(0.6)));
                                            });

                                            // Delete button (right side)
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if icon_btn(ui, Icon::TRASH, t.dim.gamma_multiply(0.4), FONT_SM)
                                                    .on_hover_text("Delete template").clicked()
                                                {
                                                    delete_idx = Some(i);
                                                }
                                            });
                                        });

                                        // Make the row clickable for apply
                                        let row_rect = row_resp.response.rect;
                                        let click_resp = ui.interact(row_rect,
                                            egui::Id::new(("tmpl_apply", pi, i)), egui::Sense::click());
                                        if click_resp.hovered() {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                            ui.painter().rect_filled(row_rect, RADIUS_SM,
                                                color_alpha(t.accent, ALPHA_FAINT));
                                        }
                                        if click_resp.clicked() {
                                            apply_idx = Some(i);
                                        }

                                        ui.add_space(GAP_XS);
                                    }
                                });

                            ui.add_space(GAP_SM);
                            separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
                            ui.add_space(GAP_SM);
                        }

                        // Save Current section
                        ui.horizontal(|ui| {
                            let te = egui::TextEdit::singleline(&mut panes[pi].template_save_name)
                                .hint_text("Template name…")
                                .desired_width(160.0)
                                .font(egui::FontId::monospace(FONT_SM));
                            ui.add(te);
                            let can_save = !panes[pi].template_save_name.trim().is_empty();
                            if can_save {
                                if small_action_btn(ui, "Save", t.accent) {
                                    let name = panes[pi].template_save_name.trim().to_string();
                                    let p = &panes[pi];
                                    let indicators: Vec<serde_json::Value> = p.indicators.iter().map(|ind| serde_json::json!({
                                        "kind": ind.kind.label(), "period": ind.period, "color": ind.color,
                                        "visible": ind.visible, "thickness": ind.thickness,
                                        "param2": ind.param2, "param3": ind.param3, "param4": ind.param4,
                                        "source": ind.source, "offset": ind.offset,
                                        "ob_level": ind.ob_level, "os_level": ind.os_level,
                                        "source_tf": ind.source_tf,
                                        "line_style": match ind.line_style { super::super::LineStyle::Solid => "solid", super::super::LineStyle::Dashed => "dashed", super::super::LineStyle::Dotted => "dotted" },
                                        "upper_color": ind.upper_color, "lower_color": ind.lower_color,
                                        "fill_color_hex": ind.fill_color_hex,
                                        "upper_thickness": ind.upper_thickness, "lower_thickness": ind.lower_thickness,
                                    })).collect();
                                    let tmpl = serde_json::json!({
                                        "show_volume": p.show_volume, "show_oscillators": p.show_oscillators,
                                        "ohlc_tooltip": p.ohlc_tooltip, "magnet": p.magnet, "log_scale": p.log_scale,
                                        "show_vwap_bands": p.show_vwap_bands, "show_cvd": p.show_cvd,
                                        "show_delta_volume": p.show_delta_volume, "show_rvol": p.show_rvol,
                                        "show_ma_ribbon": p.show_ma_ribbon, "show_prev_close": p.show_prev_close,
                                        "show_auto_sr": p.show_auto_sr, "show_auto_fib": p.show_auto_fib,
                                        "show_footprint": p.show_footprint, "show_gamma": p.show_gamma,
                                        "show_darkpool": p.show_darkpool, "show_events": p.show_events,
                                        "hit_highlight": p.hit_highlight, "show_pnl_curve": p.show_pnl_curve,
                                        "show_pattern_labels": p.show_pattern_labels,
                                        "candle_mode": match p.candle_mode {
                                            CandleMode::Standard => "std", CandleMode::Violin => "vln",
                                            CandleMode::Gradient => "grd", CandleMode::ViolinGradient => "vg",
                                            CandleMode::HeikinAshi => "ha", CandleMode::Line => "line", CandleMode::Area => "area",
                                            CandleMode::Renko => "rnk", CandleMode::RangeBar => "rng", CandleMode::TickBar => "tck",
                                        },
                                        "indicators": indicators,
                                    });
                                    watchlist.pane_templates.retain(|(n, _)| n != &name);
                                    watchlist.pane_templates.push((name, tmpl));
                                    panes[pi].template_save_name.clear();
                                    gpu::save_templates(&watchlist.pane_templates);
                                }
                            }
                        });
                    });
            });

        // Close on click outside
        if !close_popup {
            let popup_rect = win_resp.response.rect;
            if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !popup_rect.contains(p) { close_popup = true; }
                }
            }
        }

        if close_popup { panes[pi].template_popup_open = false; }

        // Apply template to this pane
        if let Some(i) = apply_idx {
            let tmpl = watchlist.pane_templates[i].1.clone();
            apply_template_to_chart(&mut panes[pi], &tmpl);
            panes[pi].template_popup_open = false;
        }

        // Delete template
        if let Some(i) = delete_idx {
            if i < watchlist.pane_templates.len() {
                watchlist.pane_templates.remove(i);
                gpu::save_templates(&watchlist.pane_templates);
            }
        }
    }
}

/// Apply a template JSON value to a chart pane (used by popup + context menu).
pub(crate) fn apply_template_to_chart(chart: &mut Chart, tmpl: &serde_json::Value) {
    let gb = |key: &str, def: bool| -> bool { tmpl.get(key).and_then(|v| v.as_bool()).unwrap_or(def) };
    chart.show_volume = gb("show_volume", true);
    chart.show_oscillators = gb("show_oscillators", true);
    chart.ohlc_tooltip = gb("ohlc_tooltip", true);
    chart.magnet = gb("magnet", true);
    chart.log_scale = gb("log_scale", false);
    chart.show_vwap_bands = gb("show_vwap_bands", true);
    chart.show_cvd = gb("show_cvd", false);
    chart.show_delta_volume = gb("show_delta_volume", false);
    chart.show_rvol = gb("show_rvol", true);
    chart.show_ma_ribbon = gb("show_ma_ribbon", false);
    chart.show_prev_close = gb("show_prev_close", true);
    chart.show_auto_sr = gb("show_auto_sr", false);
    chart.show_auto_fib = gb("show_auto_fib", false);
    chart.show_footprint = gb("show_footprint", false);
    chart.show_gamma = gb("show_gamma", false);
    chart.show_darkpool = gb("show_darkpool", false);
    chart.show_events = gb("show_events", false);
    chart.hit_highlight = gb("hit_highlight", false);
    chart.show_pnl_curve = gb("show_pnl_curve", false);
    chart.show_pattern_labels = gb("show_pattern_labels", true);
    chart.candle_mode = match tmpl.get("candle_mode").and_then(|v| v.as_str()).unwrap_or("std") {
        "vln" => CandleMode::Violin, "grd" => CandleMode::Gradient, "vg" => CandleMode::ViolinGradient,
        "ha" => CandleMode::HeikinAshi, "line" => CandleMode::Line, "area" => CandleMode::Area,
        "rnk" => CandleMode::Renko, "rng" => CandleMode::RangeBar, "tck" => CandleMode::TickBar,
        _ => CandleMode::Standard,
    };
    if let Some(inds) = tmpl.get("indicators").and_then(|v| v.as_array()) {
        chart.indicators.clear();
        for (idx, ind_json) in inds.iter().enumerate() {
            let kind_label = ind_json.get("kind").and_then(|v| v.as_str()).unwrap_or("SMA");
            let kind = gpu::indicator_type_from_label(kind_label);
            let period = ind_json.get("period").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let color = ind_json.get("color").and_then(|v| v.as_str()).unwrap_or(INDICATOR_COLORS[idx % INDICATOR_COLORS.len()]);
            let id = chart.next_indicator_id; chart.next_indicator_id += 1;
            let mut ind = gpu::Indicator::new(id, kind, period, color);
            ind.visible = ind_json.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
            ind.thickness = ind_json.get("thickness").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32;
            ind.param2 = ind_json.get("param2").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            ind.param3 = ind_json.get("param3").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            ind.param4 = ind_json.get("param4").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            ind.source = ind_json.get("source").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            ind.offset = ind_json.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as i16;
            ind.upper_color = ind_json.get("upper_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
            ind.lower_color = ind_json.get("lower_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
            ind.fill_color_hex = ind_json.get("fill_color_hex").and_then(|v| v.as_str()).unwrap_or("").to_string();
            ind.upper_thickness = ind_json.get("upper_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            ind.lower_thickness = ind_json.get("lower_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            ind.line_style = match ind_json.get("line_style").and_then(|v| v.as_str()).unwrap_or("solid") {
                "dashed" => super::super::LineStyle::Dashed, "dotted" => super::super::LineStyle::Dotted, _ => super::super::LineStyle::Solid,
            };
            chart.indicators.push(ind);
        }
        chart.indicator_bar_count = 0; // force recompute
    }
}
