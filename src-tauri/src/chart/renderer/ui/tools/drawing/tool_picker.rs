//! Middle-click drawing tool picker — favorites grid + category flyout.

use egui::Context;
use crate::chart_renderer::gpu::{Theme, Chart};
use crate::chart_renderer::gpu::Watchlist;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::ui::style::{gap_xs, gap_sm, gap_md, gap_lg, font_xs, font_sm, font_lg};
use crate::chart_renderer::ui::widgets::frames::PopupFrame;

/// Output from the picker.
pub struct DrawingToolPickerOutput {
    /// Close the picker.
    pub close: bool,
    /// Tool chosen — call `apply_draw_tool` in gpu.rs.
    pub chosen: Option<String>,
    /// Toggle star/favorite for this tool.
    pub star_toggle: Option<String>,
}

/// Draw-tool category table (mirrors the const in gpu.rs).
pub const DRAW_CATEGORIES: &[(&str, &[(&str, &str)])] = &[
    ("LINES", &[
        ("trendline", "Trend Line"),
        ("hline", "Horizontal"),
        ("vline", "Vertical"),
        ("ray", "Ray"),
        ("channel", "Channel"),
        ("regression", "Regression"),
    ]),
    ("ZONES", &[
        ("hzone", "Horizontal Zone"),
        ("pricerange", "Price Range"),
    ]),
    ("FIBONACCI", &[
        ("fibonacci", "Retracement"),
        ("fibext", "Extension"),
        ("fibarc", "Arc"),
        ("fibchannel", "Channel"),
        ("fibtimezone", "Time Zone"),
    ]),
    ("GANN / PITCHFORK", &[
        ("pitchfork", "Pitchfork"),
        ("gannbox", "Gann Box"),
        ("gannfan", "Gann Fan"),
    ]),
    ("HARMONIC", &[
        ("xabcd", "XABCD"),
        ("elliott_corrective", "Elliott Corrective"),
        ("elliott_sub_corrective", "Elliott Sub-Corrective"),
        ("elliott_wxyxz", "Elliott WXYXZ"),
    ]),
    ("UTILITY", &[
        ("magnifier", "Magnifier (zoom)"),
        ("measure", "Measure"),
        ("avwap", "Anchored VWAP"),
        ("riskreward", "Risk / Reward"),
        ("barmarker", "Bar Marker"),
    ]),
];

fn drawing_icon(tool: &str) -> &'static str {
    match tool {
        "trendline"          => Icon::LINE_SEGMENT,
        "hline"              => Icon::MINUS,
        "vline"              => Icon::DOTS_SIX_VERTICAL,
        "hzone"              => Icon::RECTANGLE,
        "ray"                => Icon::ARROW_FAT_UP,
        "channel"            => Icon::GIT_DIFF,
        "fibonacci"          => Icon::CHART_LINE,
        "fibext"             => Icon::CHART_LINE,
        "fibarc"             => Icon::CIRCLE,
        "fibchannel"         => Icon::GIT_DIFF,
        "fibtimezone"        => Icon::LIST,
        "pitchfork"          => Icon::GIT_DIFF,
        "gannbox"            => Icon::SQUARE,
        "gannfan"            => Icon::SPARKLE,
        "regression"         => Icon::PULSE,
        "avwap"              => Icon::CHART_LINE,
        "pricerange"         => Icon::ARROWS_OUT,
        "riskreward"         => Icon::CHART_BAR,
        "barmarker"          => Icon::MAP_PIN,
        "xabcd"              => Icon::CHART_LINE,
        "elliott_corrective" => Icon::CHART_LINE,
        "elliott_sub_corrective" => Icon::CHART_LINE,
        "elliott_wxyxz"      => Icon::CHART_LINE,
        "magnifier"          => Icon::MAGNIFYING_GLASS_PLUS,
        "measure"            => Icon::RULER,
        _                    => Icon::PENCIL_LINE,
    }
}

fn drawing_label(tool: &str) -> &'static str {
    match tool {
        "trendline" => "Trend Line",
        "hline" => "Horizontal",
        "vline" => "Vertical",
        "hzone" => "H-Zone",
        "ray" => "Ray",
        "channel" => "Channel",
        "fibonacci" => "Fibonacci",
        "fibext" => "Fib Extension",
        "fibarc" => "Fib Arc",
        "fibchannel" => "Fib Channel",
        "fibtimezone" => "Fib Time Zone",
        "pitchfork" => "Pitchfork",
        "gannbox" => "Gann Box",
        "gannfan" => "Gann Fan",
        "regression" => "Regression",
        "avwap" => "Anchored VWAP",
        "pricerange" => "Price Range",
        "riskreward" => "Risk/Reward",
        "barmarker" => "Bar Marker",
        "xabcd" => "XABCD",
        "elliott_corrective" => "Elliott Corrective",
        "elliott_sub_corrective" => "Elliott Sub-Corr.",
        "elliott_wxyxz" => "Elliott WXYXZ",
        "magnifier" => "Magnifier (zoom)",
        "measure" => "Measure",
        _ => "Tool",
    }
}

fn drawing_is_active(tool: &str, chart: &Chart) -> bool {
    match tool {
        "magnifier" => chart.zoom_selecting,
        "measure" => chart.measure_active,
        _ => false,
    }
}

/// Show the drawing tool picker popup.
///
/// Returns deferred output — gpu.rs applies tool selection and star toggling.
pub fn show_drawing_tool_picker(
    ctx: &Context,
    t: &Theme,
    chart: &mut Chart,
    watchlist: &mut Watchlist,
    pane_idx: usize,
    pos: egui::Pos2,
) -> DrawingToolPickerOutput {
    let mut close = false;
    let mut chosen: Option<String> = None;
    let mut star_toggle: Option<String> = None;

    let area_resp = egui::Area::new(egui::Id::new(("draw_picker", pane_idx)))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            PopupFrame::new().theme(t).ctx(ctx).build()
                .show(ui, |ui| {
                    ui.set_width(140.0);
                    ui.label(egui::RichText::new("FAVORITES")
                        .monospace().size(font_sm()).color(t.dim));
                    ui.add_space(gap_sm());
                    let favs = watchlist.draw_favorites.clone();
                    let cols = 3usize;
                    let gap = 3.0_f32;
                    let cell_w = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32).floor();
                    let cell_h = cell_w;
                    for chunk in favs.chunks(cols) {
                        ui.horizontal(|ui| {
                            for tool in chunk {
                                let icon = drawing_icon(tool);
                                let is_cur = chart.draw_tool.as_str() == tool
                                    || drawing_is_active(tool, chart);
                                let (cell, resp) = ui.allocate_exact_size(
                                    egui::vec2(cell_w, cell_h), egui::Sense::click());
                                let hov = resp.hovered();
                                let bg = if is_cur {
                                    t.accent.gamma_multiply(0.30)
                                } else if hov {
                                    t.toolbar_border.gamma_multiply(0.55)
                                } else { t.bg };
                                let stroke_col = if is_cur || hov {
                                    t.accent.gamma_multiply(if is_cur { 0.9 } else { 0.5 })
                                } else { t.toolbar_border };
                                ui.painter().rect_filled(cell, 5.0, bg);
                                ui.painter().rect_stroke(cell, 5.0,
                                    egui::Stroke::new(if is_cur { 1.5 } else { 0.7 }, stroke_col),
                                    egui::StrokeKind::Inside);
                                let txt_col = if is_cur { t.accent }
                                    else if hov { t.text } else { t.text.gamma_multiply(0.85) };
                                ui.painter().text(cell.center(), egui::Align2::CENTER_CENTER,
                                    icon, egui::FontId::proportional((cell_w * 0.55).max(11.0)), txt_col);
                                if hov {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    resp.clone().on_hover_text(drawing_label(tool));
                                }
                                if resp.clicked() { chosen = Some(tool.clone()); }
                                if resp.secondary_clicked() { star_toggle = Some(tool.clone()); }
                            }
                        });
                        ui.add_space(gap_sm());
                    }
                    ui.add_space(gap_md());
                    ui.separator();
                    ui.add_space(gap_sm());
                    ui.label(egui::RichText::new("ALL TOOLS")
                        .monospace().size(font_sm()).color(t.dim));
                    ui.add_space(gap_xs());
                    for &(cat, _tools) in DRAW_CATEGORIES {
                        let is_hovered_cat = chart.draw_picker_hover_cat.as_deref() == Some(cat);
                        let (row_rect, resp) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 20.0),
                            egui::Sense::hover(),
                        );
                        let bg = if is_hovered_cat || resp.hovered() {
                            t.accent.gamma_multiply(0.18)
                        } else { t.toolbar_bg };
                        ui.painter().rect_filled(row_rect, 3.0, bg);
                        ui.painter().text(
                            egui::pos2(row_rect.left() + gap_lg(), row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            cat,
                            egui::FontId::monospace(font_sm()),
                            if is_hovered_cat { t.accent } else { t.text.gamma_multiply(0.9) },
                        );
                        ui.painter().text(
                            egui::pos2(row_rect.right() - 8.0, row_rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            Icon::CARET_RIGHT,
                            egui::FontId::proportional(font_sm()),
                            t.dim,
                        );
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            chart.draw_picker_hover_cat = Some(cat.to_string());
                            chart.draw_picker_hover_cat_y = row_rect.top();
                        }
                    }
                });
        });

    // Flyout submenu
    let mut flyout_rect = egui::Rect::NOTHING;
    if let Some(cat) = chart.draw_picker_hover_cat.clone() {
        if let Some(&(_, tools)) = DRAW_CATEGORIES.iter().find(|&&(c, _)| c == cat) {
            let fpos = egui::pos2(
                area_resp.response.rect.right() + 2.0,
                chart.draw_picker_hover_cat_y - 6.0,
            );
            let fly = egui::Area::new(egui::Id::new(("draw_picker_flyout", pane_idx)))
                .order(egui::Order::Foreground)
                .fixed_pos(fpos)
                .show(ctx, |ui| {
                    PopupFrame::new().theme(t).ctx(ctx).build()
                        .show(ui, |ui| {
                            ui.set_width(180.0);
                            ui.label(egui::RichText::new(cat.as_str())
                                .monospace().size(font_sm()).color(t.dim));
                            ui.add_space(gap_sm());
                            for &(tool, label) in tools {
                                let starred = watchlist.draw_favorites.iter().any(|f| f == tool);
                                let star = if starred { Icon::STAR_FILL } else { Icon::STAR };
                                let is_cur = chart.draw_tool.as_str() == tool
                                    || drawing_is_active(tool, chart);
                                let icon = drawing_icon(tool);
                                let (row_rect, resp) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 22.0),
                                    egui::Sense::click(),
                                );
                                let hov = resp.hovered();
                                let bg = if is_cur {
                                    t.accent.gamma_multiply(0.25)
                                } else if hov {
                                    t.accent.gamma_multiply(0.15)
                                } else { t.toolbar_bg };
                                ui.painter().rect_filled(row_rect, 3.0, bg);
                                let star_size = 18.0;
                                let star_rect = egui::Rect::from_min_size(
                                    egui::pos2(row_rect.left() + 2.0, row_rect.top() + 2.0),
                                    egui::vec2(star_size, row_rect.height() - 4.0),
                                );
                                let star_resp = ui.allocate_rect(star_rect, egui::Sense::click());
                                let s_col = if starred { t.accent } else { t.dim };
                                ui.painter().text(
                                    star_rect.center(), egui::Align2::CENTER_CENTER,
                                    star, egui::FontId::proportional(font_lg()), s_col);
                                if star_resp.clicked() { star_toggle = Some(tool.to_string()); }
                                let txt_x = row_rect.left() + star_size + 8.0;
                                let row_col = if is_cur { t.accent }
                                    else if hov { t.text } else { t.text.gamma_multiply(0.9) };
                                ui.painter().text(
                                    egui::pos2(txt_x, row_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{}  {}", icon, label),
                                    egui::FontId::monospace(font_sm()),
                                    row_col,
                                );
                                if hov { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                                if resp.clicked() { chosen = Some(tool.to_string()); }
                            }
                        });
                });
            flyout_rect = fly.response.rect;
        }
    }

    // Hover-cat clear logic
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    if let Some(p) = pointer {
        let main_expanded = area_resp.response.rect.expand2(egui::vec2(3.0, 0.0));
        let in_main = main_expanded.contains(p);
        let in_fly = flyout_rect != egui::Rect::NOTHING && flyout_rect.contains(p);
        if !in_main && !in_fly {
            chart.draw_picker_hover_cat = None;
        }
    }

    // Close on outside click
    let outside_lmb = ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
    let outside_rmb = ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));
    if outside_lmb || outside_rmb {
        if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
            let in_main = area_resp.response.rect.contains(p);
            let in_fly = flyout_rect.contains(p);
            if !in_main && !in_fly {
                close = true;
            }
        }
    }

    DrawingToolPickerOutput { close, chosen, star_toggle }
}
