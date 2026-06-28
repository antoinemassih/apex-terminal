//! Toolbar dropdown popups — extracted from `top_nav.rs`.
//!
//! Owns the two floating popup windows that appear below the toolbar:
//!   - Timeframe picker  (`watchlist.timeframe_dropdown_open`)
//!   - Layout picker     (`watchlist.layout_dropdown_open`)
//!
//! Both follow the same UX pattern: a borderless `egui::Window` anchored
//! to a button rect, rows with hover-highlight, star-toggle favorites, and
//! click-to-select that closes the popup.

#![allow(unused_imports)]

use crate::ui_kit::icons::Icon;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use crate::ui_kit::widgets::{Button as KitButton, tokens::{Variant as KitVariant, Size as KitSize}, Input};
use crate::chart_renderer::gpu::{Chart, Watchlist, Theme, save_workspace, save_templates};
use crate::chart_renderer::gpu::Layout;
use crate::chart_renderer::ui::style::{
    color_alpha, color_half, color_muted, color_very_dim,
    alpha_heavy,
    font_xs, font_sm,
    mono_sm, icon_sm,
    gap_xs, gap_sm, gap_md,
    stroke_std, stroke_thin, r_md_cr,
};

/// Render the timeframe-picker dropdown popup.
///
/// No-op when `watchlist.timeframe_dropdown_open` is false.
pub(crate) fn render_timeframe_dropdown(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    use super::top_nav::{ALL_TIMEFRAMES, tf_to_secs, row_text_color};

    let appear = crate::ui_kit::widgets::motion::ease_bool(
        ctx, egui::Id::new("timeframe_dd_appear"), watchlist.timeframe_dropdown_open,
        crate::ui_kit::widgets::motion::FAST);
    if !watchlist.timeframe_dropdown_open && appear < 0.01 { return; }

    let dd_pos = watchlist.timeframe_dropdown_pos;
    let mut close_dd = false;
    let mut switch_to_tf: Option<&'static str> = None;
    let cur_tf = panes[ap].timeframe.clone();

    let dd_resp = egui::Window::new("timeframe_dropdown")
        .fixed_pos(dd_pos)
        .fixed_size(egui::vec2(220.0, 0.0))
        .title_bar(false)
        .frame(
            crate::chart_renderer::ui::components::frames_widget::PopupFrame::new()
                .theme(t)
                .ctx(ctx)
                .build()
                .inner_margin(egui::Margin::same(gap_md() as i8))
        )
        .show(ctx, |ui| {
            ui.set_opacity(appear);
            let hover_pos = ui.input(|i| i.pointer.hover_pos());
            let mut last_section = "";
            for &(tf_label, _secs, section) in ALL_TIMEFRAMES {
                if section != last_section {
                    if !last_section.is_empty() {
                        ui.add_space(gap_xs());
                        let y = ui.cursor().min.y;
                        ui.painter().line_segment(
                            [egui::pos2(ui.min_rect().left() + 8.0, y), egui::pos2(ui.min_rect().left() + 236.0, y)],
                            egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, 50)));
                        ui.add_space(gap_sm());
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(gap_md());
                        ui.label(egui::RichText::new(section).monospace().size(font_xs()).strong().color(color_half(t.dim)));
                    });
                    ui.add_space(gap_xs());
                    last_section = section;
                }
                let is_cur = cur_tf == tf_label;
                let is_fav = watchlist.timeframe_favorites.iter().any(|f| f == tf_label);
                let row_min = ui.cursor().min;
                let row_rect = egui::Rect::from_min_size(row_min, egui::vec2(236.0, 24.0));
                let hovered = hover_pos.map_or(false, |p| row_rect.contains(p));

                if hovered || is_cur {
                    let bg = if is_cur { tint(t, Tone::Accent, 25) } else { tint(t, Tone::Border, 30) };
                    ui.painter().rect_filled(row_rect, 3.0, bg);
                }
                if is_cur {
                    ui.painter().rect_filled(egui::Rect::from_min_size(row_rect.min, egui::vec2(2.0, 24.0)), 1.0, t.accent);
                }

                let lc = row_text_color(is_cur, hovered, t);
                ui.painter().text(
                    egui::pos2(row_rect.left() + 14.0, row_rect.center().y),
                    egui::Align2::LEFT_CENTER, tf_label, mono_sm(), lc,
                );

                let sr = egui::Rect::from_min_size(egui::pos2(row_rect.right() - 22.0, row_rect.center().y - 8.0), egui::vec2(icon_sm(), icon_sm()));
                let sh = hover_pos.map_or(false, |p| sr.contains(p));
                let sc = if is_fav { tint(t, Tone::Accent, alpha_heavy()) } else if sh { color_half(t.dim) } else { color_very_dim(t.dim) };
                ui.painter().text(sr.center(), egui::Align2::CENTER_CENTER, Icon::STAR_FILL, egui::FontId::proportional(font_sm()), sc);
                if sh { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                if sh && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                    if is_fav { watchlist.timeframe_favorites.retain(|f| f != tf_label); }
                    else { watchlist.timeframe_favorites.push(tf_label.to_string()); }
                }

                let rh = hover_pos.map_or(false, |p| row_rect.contains(p)) && !sh;
                if rh { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                if rh && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                    switch_to_tf = Some(tf_label);
                    close_dd = true;
                }

                ui.allocate_space(egui::vec2(236.0, 24.0));
            }
        });

    if let Some(resp) = dd_resp {
        let win_rect = resp.response.rect;
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                if !win_rect.contains(pos) { close_dd = true; }
            }
        }
    }
    if let Some(new_tf) = switch_to_tf {
        if new_tf != panes[ap].timeframe {
            let cur_secs = tf_to_secs(&panes[ap].timeframe);
            let new_secs = tf_to_secs(new_tf);
            if cur_secs > 0 && new_secs > 0 {
                let new_vc = ((panes[ap].vc as u64 * cur_secs as u64) / new_secs as u64).max(20).min(2000) as u32;
                panes[ap].vc = new_vc;
                panes[ap].vc_target = new_vc;
            }
            panes[ap].pending_timeframe_change = Some(new_tf.to_string());
        }
    }
    if close_dd { watchlist.timeframe_dropdown_open = false; }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { watchlist.timeframe_dropdown_open = false; }
}

/// Render the layout-picker dropdown popup.
///
/// No-op when `watchlist.layout_dropdown_open` is false.
pub(crate) fn render_layout_dropdown(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut Vec<Chart>,
    layout: &mut Layout,
    active_pane: &mut usize,
    t: &Theme,
) {
    use super::top_nav::row_text_color;
    use crate::chart_renderer::gpu::{ALL_LAYOUTS};

    let appear = crate::ui_kit::widgets::motion::ease_bool(
        ctx, egui::Id::new("layout_dd_appear"), watchlist.layout_dropdown_open,
        crate::ui_kit::widgets::motion::FAST);
    if !watchlist.layout_dropdown_open && appear < 0.01 { return; }

    let dd_pos = watchlist.layout_dropdown_pos;
    let mut close_dd = false;
    let mut switch_to: Option<Layout> = None;

    let dd_resp = egui::Window::new("layout_dropdown")
        .fixed_pos(dd_pos)
        .fixed_size(egui::vec2(220.0, 0.0))
        .title_bar(false)
        .frame(
            crate::chart_renderer::ui::components::frames_widget::PopupFrame::new()
                .theme(t)
                .ctx(ctx)
                .build()
                .inner_margin(egui::Margin::same(gap_md() as i8))
        )
        .show(ctx, |ui| {
            ui.set_opacity(appear);
            let hover_pos = ui.input(|i| i.pointer.hover_pos());
            let mut last_section = "";
            for &ly in ALL_LAYOUTS {
                let sec = ly.section();
                if sec != last_section {
                    if !last_section.is_empty() {
                        ui.add_space(gap_xs());
                        let y = ui.cursor().min.y;
                        ui.painter().line_segment(
                            [egui::pos2(ui.min_rect().left() + 8.0, y), egui::pos2(ui.min_rect().left() + 236.0, y)],
                            egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, 50)));
                        ui.add_space(gap_sm());
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(gap_md());
                        ui.label(egui::RichText::new(sec).monospace().size(font_xs()).strong().color(color_half(t.dim)));
                    });
                    ui.add_space(gap_xs());
                    last_section = sec;
                }
                let is_cur = *layout == ly;
                let is_fav = watchlist.layout_favorites.iter().any(|f| f == ly.label());
                let row_min = ui.cursor().min;
                let row_rect = egui::Rect::from_min_size(row_min, egui::vec2(236.0, 26.0));
                let hovered = hover_pos.map_or(false, |p| row_rect.contains(p));

                if hovered || is_cur {
                    let bg = if is_cur { tint(t, Tone::Accent, 25) } else { tint(t, Tone::Border, 30) };
                    ui.painter().rect_filled(row_rect, 3.0, bg);
                }
                if is_cur {
                    ui.painter().rect_filled(egui::Rect::from_min_size(row_rect.min, egui::vec2(2.0, 26.0)), 1.0, t.accent);
                }

                let gr = egui::Rect::from_min_size(egui::pos2(row_rect.left() + 6.0, row_rect.center().y - 9.5), egui::vec2(29.0, 19.0));
                let gc = if is_cur { t.accent } else if hovered { t.dim } else { color_half(t.dim) };
                let mini = ly.pane_rects(gr, ly.max_panes(), 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5);
                for mr in &mini {
                    let s = egui::Rect::from_min_max(egui::pos2(mr.left() + 0.5, mr.top() + 0.5), egui::pos2(mr.right() - 0.5, mr.bottom() - 0.5));
                    ui.painter().rect_filled(s, 1.0, color_alpha(gc, 80));
                    ui.painter().rect_stroke(s, 1.0, egui::Stroke::new(stroke_thin(), color_alpha(gc, 150)), egui::StrokeKind::Outside);
                }

                let lc = row_text_color(is_cur, hovered, t);
                ui.painter().text(egui::pos2(row_rect.left() + 42.0, row_rect.center().y), egui::Align2::LEFT_CENTER, ly.label(), mono_sm(), lc);
                let dc = if hovered { tint(t, Tone::Dim, alpha_heavy()) } else { color_muted(t.dim) };
                ui.painter().text(egui::pos2(row_rect.left() + 74.0, row_rect.center().y), egui::Align2::LEFT_CENTER, ly.description(), mono_sm(), dc);

                let sr = egui::Rect::from_min_size(egui::pos2(row_rect.right() - 22.0, row_rect.center().y - 8.0), egui::vec2(icon_sm(), icon_sm()));
                let sh = hover_pos.map_or(false, |p| sr.contains(p));
                let sc = if is_fav { tint(t, Tone::Accent, alpha_heavy()) } else if sh { color_half(t.dim) } else { color_very_dim(t.dim) };
                ui.painter().text(sr.center(), egui::Align2::CENTER_CENTER, Icon::STAR_FILL, egui::FontId::proportional(font_sm()), sc);
                if sh { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                if sh && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                    if is_fav { watchlist.layout_favorites.retain(|f| f != ly.label()); }
                    else { watchlist.layout_favorites.push(ly.label().to_string()); }
                }

                let rh = hover_pos.map_or(false, |p| row_rect.contains(p)) && !sh;
                if rh { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                if rh && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                    switch_to = Some(ly);
                    close_dd = true;
                }

                ui.allocate_space(egui::vec2(236.0, 26.0));
            }

            // ── Saved layout templates ──
            if !watchlist.pane_templates.is_empty() {
                ui.add_space(gap_xs());
                let y = ui.cursor().min.y;
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left() + 8.0, y), egui::pos2(ui.min_rect().left() + 236.0, y)],
                    egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, 50)));
                ui.add_space(gap_sm());
                ui.horizontal(|ui| {
                    ui.add_space(gap_md());
                    ui.label(egui::RichText::new("SAVED LAYOUTS").monospace().size(font_xs()).strong().color(color_half(t.dim)));
                });
                ui.add_space(gap_xs());
                let templates_snapshot: Vec<String> = watchlist.pane_templates.iter().map(|(n, _)| n.clone()).collect();
                for tpl_name in &templates_snapshot {
                    let row_min = ui.cursor().min;
                    let row_rect = egui::Rect::from_min_size(row_min, egui::vec2(236.0, 24.0));
                    let row_hover = hover_pos.map_or(false, |p| row_rect.contains(p));
                    if row_hover {
                        ui.painter().rect_filled(row_rect, 3.0, tint(t, Tone::Border, 30));
                    }
                    let lc = if row_hover { t.text } else { t.dim };
                    ui.painter().text(egui::pos2(row_rect.left() + 8.0, row_rect.center().y), egui::Align2::LEFT_CENTER, tpl_name.as_str(), mono_sm(), lc);
                    let apply_w = 42.0;
                    let apply_rect = egui::Rect::from_min_size(
                        egui::pos2(row_rect.right() - apply_w - 4.0, row_rect.center().y - 10.0),
                        egui::vec2(apply_w, 20.0),
                    );
                    let apply_hov = hover_pos.map_or(false, |p| apply_rect.contains(p));
                    let apply_fill = if apply_hov { tint(t, Tone::Accent, 40) } else { tint(t, Tone::Accent, 15) };
                    ui.painter().rect_filled(apply_rect, 3.0, apply_fill);
                    ui.painter().text(apply_rect.center(), egui::Align2::CENTER_CENTER, "Apply", mono_sm(),
                        if apply_hov { t.accent } else { tint(t, Tone::Accent, 180) });
                    if apply_hov { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if apply_hov && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                        watchlist.active_workspace = tpl_name.clone();
                        watchlist.pending_workspace_load = Some(tpl_name.clone());
                        close_dd = true;
                    }
                    ui.allocate_space(egui::vec2(236.0, 24.0));
                }
            }

            // ── Save current layout as… ──
            {
                ui.add_space(gap_xs());
                let y = ui.cursor().min.y;
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left() + 8.0, y), egui::pos2(ui.min_rect().left() + 236.0, y)],
                    egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, 50)));
                ui.add_space(gap_sm());
                ui.horizontal(|ui| {
                    use crate::ui_kit::widgets::Input;
                    Input::new(&mut watchlist.pane_template_name)
                        .placeholder("Save layout as…")
                        .min_width(148.0)
                        .size(KitSize::Sm)
                        .show(ui, t);
                    let can_save = !watchlist.pane_template_name.trim().is_empty();
                    if can_save {
                        if KitButton::new("Save").variant(KitVariant::Primary).size(KitSize::Sm)
                            .tint(t.accent).show(ui, t).clicked()
                        {
                            let name = watchlist.pane_template_name.trim().to_string();
                            save_workspace(&name, panes, *layout, watchlist);
                            let layout_val = serde_json::json!({
                                "kind": "layout_template",
                                "layout": layout.label(),
                                "panes": panes.iter().map(|p| serde_json::json!({
                                    "symbol": p.symbol,
                                    "timeframe": p.timeframe,
                                    "link_group": p.link_group,
                                })).collect::<Vec<_>>(),
                            });
                            watchlist.pane_templates.retain(|(n, _)| n != &name);
                            watchlist.pane_templates.push((name.clone(), layout_val));
                            save_templates(&watchlist.pane_templates);
                            watchlist.pane_template_name.clear();
                            close_dd = true;
                        }
                    }
                });
            }
        });

    if let Some(resp) = dd_resp {
        let win_rect = resp.response.rect;
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                if !win_rect.contains(pos) { close_dd = true; }
            }
        }
    }
    if let Some(ly) = switch_to {
        *layout = ly;
        let max = ly.max_panes();
        while panes.len() < max {
            let syms = ["SPY","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOG","AMD"];
            let sym = syms.get(panes.len()).copied().unwrap_or("SPY");
            let tf = panes[0].timeframe.clone();
            let mut c = Chart::new_with(sym, &tf);
            c.theme_idx = panes[0].theme_idx;
            c.recent_symbols = panes[0].recent_symbols.clone();
            c.pending_symbol_change = Some(sym.to_string());
            panes.push(c);
        }
        if *active_pane >= max { *active_pane = 0; }
        let template_max = ly.max_panes();
        if panes.len() > template_max {
            crate::chart_renderer::gpu::PENDING_PANE_CLOSE.with(|q| {
                let mut closes = q.borrow_mut();
                for idx in template_max..panes.len() { closes.push(idx); }
            });
            watchlist.maximized_pane = watchlist.maximized_pane.filter(|&m| m < template_max);
        }
        crate::chart_renderer::gpu::ensure_pane_ids_synced(watchlist, panes.len());
        let mut slot_ids: Vec<u64> = watchlist.pane_ids.iter().copied().take(template_max.max(1)).collect();
        while slot_ids.len() < template_max.max(1) { slot_ids.push(0); }
        watchlist.pane_layout = Some(
            crate::chart_renderer::pane_layout::PaneLayout::from_template(ly, &slot_ids)
        );
    }
    if close_dd { watchlist.layout_dropdown_open = false; }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { watchlist.layout_dropdown_open = false; }
}
