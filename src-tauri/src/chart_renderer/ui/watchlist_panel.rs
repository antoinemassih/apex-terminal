//! Watchlist side panel — stocks list, options chain, heatmap.

use egui;
use super::style::*;
use super::super::gpu::*;
use super::super::{Drawing, DrawingKind, ChartCommand};
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::gpu::{fetch_chain_background, fetch_search_background, fetch_watchlist_prices, set_pending_wl_tooltip, WlTooltipData};
use crate::chart_renderer::trading::market_session;

const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Watchlist side panel ───────────────────────────────────────────────────
if watchlist.open {
    egui::SidePanel::right("watchlist")
        .default_width(260.0)
        .min_width(140.0)
        .max_width(500.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin { left: 6, right: 6, top: 6, bottom: 6 }))
        .show(ctx, |ui| {
            // Force content to never exceed the panel's actual width
            let panel_w = ui.available_width();
            ui.set_min_width(0.0);
            ui.set_max_width(panel_w);
            let mut wl_switch_to: Option<usize> = None;
            let mut wl_fetch_syms: Vec<String> = Vec::new();
            let mut wl_rename_idx: Option<usize> = None;
            let mut wl_delete_idx: Option<usize> = None;
            let mut wl_dup_idx: Option<usize> = None;

            // ── A) Tabs at the very top with X button ──
            let tab_row_resp = ui.horizontal(|ui| {
                ui.set_min_height(22.0);
                tab_bar(ui, &mut watchlist.tab, &[
                    (WatchlistTab::Stocks, "LIST"),
                    (WatchlistTab::Chain, "CHAIN"),
                    (WatchlistTab::Heat, "HEAT"),
                ], t.accent, t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.open = false; }
                    // Market session badge
                    let (session, session_col) = market_session();
                    ui.add_space(4.0);
                    let badge_bg = color_alpha(session_col, ALPHA_TINT);
                    ui.add(egui::Button::new(
                        egui::RichText::new(session).monospace().size(8.5).strong().color(session_col))
                        .fill(badge_bg).corner_radius(RADIUS_SM).stroke(egui::Stroke::NONE)
                        .min_size(egui::vec2(34.0, 14.0)));
                });
            });
            // 1px line below tabs
            let line_y = tab_row_resp.response.rect.max.y + 1.0;
            ui.painter().line_segment(
                [egui::pos2(ui.min_rect().left(), line_y), egui::pos2(ui.min_rect().right(), line_y)],
                egui::Stroke::new(STROKE_STD, t.toolbar_border),
            );
            ui.add_space(4.0);

            let mut open_option_chart: Option<(String, f32, bool, String)> = None;

            match watchlist.tab {
                // ── STOCKS TAB (LIST) ──────────────────────────────────────────
                WatchlistTab::Stocks => {
                    // ── B) Watchlist selector + options toggle ──
                    ui.horizontal(|ui| {
                        ui.set_min_height(20.0);
                        // Inline rename mode
                        if watchlist.watchlist_name_editing {
                            let resp = ui.add(egui::TextEdit::singleline(&mut watchlist.watchlist_name_buf)
                                .desired_width(ui.available_width() - 50.0)
                                .font(egui::FontId::monospace(10.0)));
                            if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let new_name = watchlist.watchlist_name_buf.trim().to_string();
                                if !new_name.is_empty() {
                                    if let Some(wl) = watchlist.saved_watchlists.get_mut(watchlist.active_watchlist_idx) {
                                        wl.name = new_name;
                                    }
                                }
                                watchlist.watchlist_name_editing = false;
                                watchlist.persist();
                            } else {
                                resp.request_focus();
                            }
                        } else {
                            // Snapshot names and count for the dropdown to avoid borrow conflicts
                            let wl_names: Vec<String> = watchlist.saved_watchlists.iter().map(|w| w.name.clone()).collect();
                            let wl_count = wl_names.len();
                            let active_idx = watchlist.active_watchlist_idx;
                            let active_name = wl_names.get(active_idx).cloned().unwrap_or_else(|| "Default".into());
                            let combo_resp = egui::ComboBox::from_id_salt("wl_selector")
                                .selected_text(egui::RichText::new(&active_name).monospace().size(10.0).color(t.accent))
                                .width(ui.available_width() - 60.0)
                                .show_ui(ui, |ui| {
                                    for (i, name) in wl_names.iter().enumerate() {
                                        let is_active = i == active_idx;
                                        let label_color = if is_active { t.accent } else { egui::Color32::from_rgb(200, 200, 210) };
                                        let resp = ui.selectable_label(is_active,
                                            egui::RichText::new(name).monospace().size(10.0).color(label_color));
                                        if resp.clicked() && !is_active {
                                            wl_switch_to = Some(i);
                                        }
                                        // Right-click context menu on each watchlist entry
                                        resp.context_menu(|ui| {
                                            if ui.button(egui::RichText::new("Rename").monospace().size(10.0)).clicked() {
                                                wl_rename_idx = Some(i);
                                                ui.close_menu();
                                            }
                                            if ui.button(egui::RichText::new("Duplicate").monospace().size(10.0)).clicked() {
                                                wl_dup_idx = Some(i);
                                                ui.close_menu();
                                            }
                                            if wl_count > 1 {
                                                ui.separator();
                                                if ui.button(egui::RichText::new("Delete").monospace().size(10.0)
                                                    .color(egui::Color32::from_rgb(224, 85, 96))).clicked() {
                                                    wl_delete_idx = Some(i);
                                                    ui.close_menu();
                                                }
                                            }
                                        });
                                    }
                                });
                            // Right-click the combo box header for rename/dup/delete
                            combo_resp.response.context_menu(|ui| {
                                if ui.button(egui::RichText::new("Rename").monospace().size(10.0)).clicked() {
                                    wl_rename_idx = Some(active_idx);
                                    ui.close_menu();
                                }
                                if ui.button(egui::RichText::new("Duplicate").monospace().size(10.0)).clicked() {
                                    wl_dup_idx = Some(active_idx);
                                    ui.close_menu();
                                }
                                if wl_count > 1 {
                                    ui.separator();
                                    if ui.button(egui::RichText::new("Delete").monospace().size(10.0)
                                        .color(egui::Color32::from_rgb(224, 85, 96))).clicked() {
                                        wl_delete_idx = Some(active_idx);
                                        ui.close_menu();
                                    }
                                }
                            });
                            // "+" button to create new watchlist
                            if ui.add(egui::Button::new(egui::RichText::new(Icon::PLUS).size(12.0).color(t.dim)).frame(false)).clicked() {
                                let n = watchlist.saved_watchlists.len() + 1;
                                let syms = watchlist.create_watchlist(&format!("Watchlist {}", n));
                                if !syms.is_empty() { wl_fetch_syms = syms; }
                            }
                        }
                        // Options toggle (circle icon) — right-aligned
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let opt_icon = if watchlist.options_visible { Icon::RADIO_BUTTON } else { Icon::DOT };
                            let opt_color = if watchlist.options_visible { t.accent } else { t.dim };
                            let opt_resp = ui.add(egui::Button::new(egui::RichText::new(opt_icon).size(11.0).color(opt_color))
                                .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(18.0, 18.0)));
                            if opt_resp.clicked() { watchlist.options_visible = !watchlist.options_visible; }
                            if opt_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                        });
                    });
                    // Handle deferred rename
                    if let Some(idx) = wl_rename_idx {
                        if idx != watchlist.active_watchlist_idx {
                            wl_switch_to = Some(idx);
                        }
                        watchlist.watchlist_name_buf = watchlist.saved_watchlists.get(idx).map(|w| w.name.clone()).unwrap_or_default();
                        watchlist.watchlist_name_editing = true;
                    }
                    // Handle deferred duplicate
                    if let Some(dup_idx) = wl_dup_idx {
                        let syms = watchlist.duplicate_watchlist(dup_idx);
                        if !syms.is_empty() { wl_fetch_syms = syms; }
                    }
                    // Handle deferred delete
                    if let Some(del_idx) = wl_delete_idx {
                        let syms = watchlist.delete_watchlist(del_idx);
                        if !syms.is_empty() { wl_fetch_syms = syms; }
                    }
                    // Handle watchlist switch
                    if let Some(idx) = wl_switch_to {
                        let syms = watchlist.switch_to(idx);
                        if !syms.is_empty() { wl_fetch_syms = syms; }
                    }
                    // Trigger price fetches for new watchlist
                    if !wl_fetch_syms.is_empty() {
                        fetch_watchlist_prices(wl_fetch_syms);
                    }
                    ui.add_space(2.0);

                    // ── C) Search field + filter button beside it ──
                    // Use allocate_ui_with_layout to place them side by side without
                    // ui.horizontal() which reports combined min-width and forces expansion.
                    let search_id = egui::Id::new("wl_search_input");
                    let avail = ui.available_width();
                    let btn_w = 22.0;
                    let search_w = (avail - btn_w - 4.0).max(40.0);
                    let search_h = 20.0;
                    let (full_rect, _) = ui.allocate_exact_size(egui::vec2(avail, search_h), egui::Sense::hover());
                    // Search field (left portion)
                    let search_rect = egui::Rect::from_min_size(full_rect.min, egui::vec2(search_w, search_h));
                    let search_resp = ui.put(search_rect, egui::TextEdit::singleline(&mut watchlist.search_query)
                        .id(search_id)
                        .hint_text("Add symbol...").desired_width(search_w).font(egui::FontId::monospace(11.0)));
                    // Filter button (right portion)
                    let filter_active = watchlist.filter_preset != "All" || !watchlist.filter_text.is_empty();
                    let icon_col = if filter_active { t.accent } else if watchlist.filter_open { t.accent } else { t.dim.gamma_multiply(0.4) };
                    let btn_rect = egui::Rect::from_min_size(egui::pos2(full_rect.right() - btn_w, full_rect.top()), egui::vec2(btn_w, search_h));
                    ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER, Icon::FUNNEL, egui::FontId::proportional(11.0), icon_col);
                    let btn_resp = ui.interact(btn_rect, egui::Id::new("wl_filter_btn"), egui::Sense::click());
                    if btn_resp.clicked() { watchlist.filter_open = !watchlist.filter_open; }
                    if btn_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    // Refocus after adding a symbol
                    if watchlist.search_refocus {
                        watchlist.search_refocus = false;
                        search_resp.request_focus();
                    }
                    if search_resp.changed() {
                        watchlist.search_sel = -1; // reset selection on text change
                        if !watchlist.search_query.is_empty() {
                            // Immediate: static results
                            watchlist.search_results = crate::ui_kit::symbols::search_symbols(&watchlist.search_query, 8)
                                .iter().map(|s| (s.symbol.to_string(), s.name.to_string())).collect();
                            // Background: ApexIB search (results merge via SearchResults command)
                            fetch_search_background(watchlist.search_query.clone(), "watchlist".to_string());
                        } else {
                            watchlist.search_results.clear();
                        }
                    }
                    // Arrow key navigation + Enter to select
                    let has_results = !watchlist.search_query.is_empty() && !watchlist.search_results.is_empty();
                    if has_results && search_resp.has_focus() {
                        let max = watchlist.search_results.len() as i32;
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                            watchlist.search_sel = (watchlist.search_sel + 1).min(max - 1);
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                            watchlist.search_sel = (watchlist.search_sel - 1).max(-1);
                        }
                    }
                    // Enter: add highlighted or typed symbol
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !watchlist.search_query.is_empty() {
                        let sym = if watchlist.search_sel >= 0 && (watchlist.search_sel as usize) < watchlist.search_results.len() {
                            watchlist.search_results[watchlist.search_sel as usize].0.clone()
                        } else {
                            watchlist.search_query.trim().to_uppercase()
                        };
                        watchlist.add_symbol(&sym);
                        fetch_watchlist_prices(vec![sym]);
                        watchlist.search_query.clear();
                        watchlist.search_results.clear();
                        watchlist.search_sel = -1;
                        watchlist.search_refocus = true;
                        watchlist.persist();
                    }
                    // Escape clears search
                    if search_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        watchlist.search_query.clear();
                        watchlist.search_results.clear();
                        watchlist.search_sel = -1;
                    }
                    // Suggestion dropdown
                    if has_results {
                        egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(28, 28, 34)).corner_radius(4.0).show(ui, |ui| {
                            for (i, (sym, name)) in watchlist.search_results.clone().iter().enumerate() {
                                let is_sel = i as i32 == watchlist.search_sel;
                                let bg = if is_sel { color_alpha(t.accent, ALPHA_TINT) } else { egui::Color32::TRANSPARENT };
                                let fg = if is_sel { t.text } else { t.dim };
                                let resp = ui.add(egui::Button::new(
                                    egui::RichText::new(format!("{:6} {}", sym, name)).monospace().size(10.0).color(fg))
                                    .fill(bg).frame(false).min_size(egui::vec2(ui.available_width(), 20.0)));
                                if resp.clicked() {
                                    watchlist.add_symbol(sym);
                                    fetch_watchlist_prices(vec![sym.clone()]);
                                    watchlist.search_query.clear();
                                    watchlist.search_results.clear();
                                    watchlist.search_sel = -1;
                                    watchlist.search_refocus = true;
                                    watchlist.persist();
                                }
                                if resp.hovered() {
                                    watchlist.search_sel = i as i32;
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                            }
                        });
                    }
                    ui.add_space(4.0);

                    // Filter indicator (show active preset name if filtering)
                    if watchlist.filter_preset != "All" || !watchlist.filter_text.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(format!("{} {}", Icon::FUNNEL, watchlist.filter_preset)).monospace().size(8.0).color(t.accent));
                        });
                    }
                    if watchlist.filter_open {
                        ui.add_space(2.0);
                        // Search
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut watchlist.filter_text)
                                .hint_text("Search...").desired_width((ui.available_width() - 30.0).max(40.0)).font(egui::FontId::monospace(9.0)));
                            if !watchlist.filter_text.is_empty() {
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(8.0).color(t.dim)).frame(false)).clicked() {
                                    watchlist.filter_text.clear();
                                }
                            }
                        });
                        // Preset buttons
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            let presets: Vec<(&str, f32, f32)> = {
                                let mut p = vec![
                                    ("All", -999.0_f32, 999.0_f32),
                                    ("+2%", 2.0, 999.0), ("-2%", -999.0, -2.0),
                                    ("+5%", 5.0, 999.0), ("-5%", -999.0, -5.0),
                                    ("Big", 3.0, 999.0),
                                ];
                                for cf in &watchlist.custom_filters { p.push((&cf.0, cf.1, cf.2)); }
                                p
                            };
                            for (name, min_chg, max_chg) in &presets {
                                let active = watchlist.filter_preset == *name;
                                let col = if active { t.accent } else { t.dim };
                                let bg = if active { color_alpha(t.accent, ALPHA_SUBTLE) } else { egui::Color32::TRANSPARENT };
                                if ui.add(egui::Button::new(egui::RichText::new(*name).monospace().size(8.0).color(col))
                                    .fill(bg).corner_radius(RADIUS_MD).min_size(egui::vec2(0.0, 16.0))).clicked() {
                                    watchlist.filter_preset = name.to_string();
                                    watchlist.filter_min_change = *min_chg;
                                    watchlist.filter_max_change = *max_chg;
                                }
                            }
                        });
                        // Create custom filter (inline)
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            ui.label(egui::RichText::new("+").monospace().size(9.0).color(t.accent));
                            // Quick create: just type a name and min% threshold
                            static mut NEW_FILTER_NAME: String = String::new();
                            static mut NEW_FILTER_MIN: String = String::new();
                            unsafe {
                                ui.add(egui::TextEdit::singleline(&mut NEW_FILTER_NAME).hint_text("name").desired_width(50.0).font(egui::FontId::monospace(8.0)));
                                ui.label(egui::RichText::new(">").monospace().size(8.0).color(t.dim));
                                ui.add(egui::TextEdit::singleline(&mut NEW_FILTER_MIN).hint_text("%").desired_width(30.0).font(egui::FontId::monospace(8.0)));
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::CHECK).size(8.0).color(t.accent)).frame(false)).clicked() {
                                    let name = NEW_FILTER_NAME.trim().to_string();
                                    let min_val: f32 = NEW_FILTER_MIN.parse().unwrap_or(0.0);
                                    if !name.is_empty() {
                                        watchlist.custom_filters.push((name, min_val, 999.0));
                                        NEW_FILTER_NAME.clear();
                                        NEW_FILTER_MIN.clear();
                                    }
                                }
                            }
                        });
                        ui.add_space(2.0);
                    }

                    // Symbol list with sections and drag-and-drop
                    let active_sym = panes[ap].symbol.clone();
                    let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
                    let pointer_released = ui.ctx().input(|i| i.pointer.any_released());
                    let pointer_down = ui.ctx().input(|i| i.pointer.any_down());

                    // Mark which sections are option sections
                    let option_section_ids: Vec<u32> = watchlist.sections.iter()
                        .filter(|s| s.title.contains("Options"))
                        .map(|s| s.id).collect();

                    // Options section always visible when toggled on (even if empty)
                    let show_opts = watchlist.options_visible;
                    let total_avail = ui.available_height();
                    let stocks_h = if show_opts { (total_avail * watchlist.options_split).max(60.0) } else { total_avail };

                    egui::ScrollArea::vertical().id_salt("wl_stocks").max_height(stocks_h).show(ui, |ui| {
                        let mut remove_sym: Option<String> = None;
                        let mut click_sym: Option<String> = None;
                        let mut click_opt: Option<(String, f32, bool, String)> = None; // option click -> open chart
                        let mut toggle_collapse: Option<usize> = None;
                        let mut remove_section: Option<usize> = None;
                        let full_w = ui.available_width();

                        // Collect row rects for drop target calculation
                        let mut row_rects: Vec<(usize, usize, egui::Rect)> = Vec::new(); // (sec_idx, item_idx, rect)
                        let mut section_header_rects: Vec<(usize, egui::Rect)> = Vec::new();

                        let section_count = watchlist.sections.len();
                        let dragging = watchlist.dragging;
                        let drag_confirmed = watchlist.drag_confirmed;

                        // Section color presets for the color picker
                        let color_presets = ["#4a9eff","#e74c3c","#2ecc71","#f39c12","#9b59b6","#1abc9c","#e67e22","#3498db","#e91e63","#00bcd4","#8bc34a","#ff5722","#607d8b","#795548","#cddc39","#ff9800"];


                        let mut active_sym_change: Option<String> = None;
                        // ── PINNED section at the top (no title, darker background) ──
                        let has_pinned = watchlist.sections.iter().any(|s| s.items.iter().any(|i| i.pinned));
                        if has_pinned {
                            // Collect pinned items first
                            let mut pinned_items: Vec<(usize, usize, String, f32, f32, bool, f32)> = vec![]; // (si, ii, sym, price, prev, loaded, avg_range)
                            for si in 0..watchlist.sections.len() {
                                for ii in 0..watchlist.sections[si].items.len() {
                                    let item = &watchlist.sections[si].items[ii];
                                    if item.pinned && !item.is_option {
                                        pinned_items.push((si, ii, item.symbol.clone(), item.price, item.prev_close, item.loaded, item.avg_daily_range));
                                    }
                                }
                            }
                            // Darker section background
                            let section_h = pinned_items.len() as f32 * 30.0 + 6.0;
                            let (sec_rect, _) = ui.allocate_exact_size(egui::vec2(full_w, section_h), egui::Sense::hover());
                            let p = ui.painter();
                            // Darker section background + inset bevel
                            p.rect_filled(sec_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, ALPHA_LINE));
                            // Inset effect: dark line at top, light line at bottom
                            p.line_segment([egui::pos2(sec_rect.left(), sec_rect.top()), egui::pos2(sec_rect.right(), sec_rect.top())],
                                egui::Stroke::new(STROKE_STD, egui::Color32::from_rgba_unmultiplied(0, 0, 0, ALPHA_DIM)));
                            p.line_segment([egui::pos2(sec_rect.left(), sec_rect.top() + 1.0), egui::pos2(sec_rect.right(), sec_rect.top() + 1.0)],
                                egui::Stroke::new(STROKE_THIN, egui::Color32::from_rgba_unmultiplied(0, 0, 0, ALPHA_TINT)));
                            p.line_segment([egui::pos2(sec_rect.left(), sec_rect.bottom() - 1.0), egui::pos2(sec_rect.right(), sec_rect.bottom() - 1.0)],
                                egui::Stroke::new(STROKE_STD, color_alpha(t.text,10)));
                            p.line_segment([egui::pos2(sec_rect.left(), sec_rect.bottom()), egui::pos2(sec_rect.right(), sec_rect.bottom())],
                                egui::Stroke::new(STROKE_THIN, color_alpha(t.text,5)));
                            // Render each pinned row
                            for (idx, (si, ii, pin_sym, pin_price, pin_prev, pin_loaded, avg_range)) in pinned_items.iter().enumerate() {
                                let row_y = sec_rect.top() + 3.0 + idx as f32 * 30.0;
                                let row_rect = egui::Rect::from_min_size(egui::pos2(sec_rect.left(), row_y), egui::vec2(full_w, 28.0));
                                let yc = row_rect.center().y;
                                let is_active = *pin_sym == active_sym;
                                let change_pct = if *pin_prev > 0.0 { (*pin_price / *pin_prev - 1.0) * 100.0 } else { 0.0 };
                                let col = if change_pct >= 0.0 { t.bull } else { t.bear };
                                // Extreme movement background
                                if change_pct.abs() > *avg_range * 1.5 {
                                    let extreme_bg = if change_pct >= 0.0 { egui::Color32::from_rgba_unmultiplied(46, 204, 113, ALPHA_GHOST) }
                                        else { egui::Color32::from_rgba_unmultiplied(231, 76, 60, ALPHA_GHOST) };
                                    p.rect_filled(row_rect, 0.0, extreme_bg);
                                }
                                // Active indicator
                                if is_active {
                                    p.rect_filled(egui::Rect::from_min_max(row_rect.min, egui::pos2(row_rect.min.x + 2.5, row_rect.max.y)), 1.0, t.accent);
                                }
                                // Gold star (always visible for pinned)
                                p.text(egui::pos2(row_rect.left() + 10.0, yc), egui::Align2::CENTER_CENTER,
                                    Icon::SPARKLE, egui::FontId::proportional(9.0), egui::Color32::from_rgb(255, 193, 37));
                                // Symbol
                                let sym_col = if is_active { egui::Color32::WHITE } else { color_alpha(t.text,230) };
                                p.text(egui::pos2(row_rect.left() + 22.0, yc), egui::Align2::LEFT_CENTER,
                                    pin_sym, egui::FontId::monospace(14.0), sym_col);
                                // Change %
                                p.text(egui::pos2(row_rect.left() + full_w * 0.45, yc), egui::Align2::LEFT_CENTER,
                                    &format!("{:+.2}%", change_pct), egui::FontId::monospace(14.0), col);
                                // Price
                                p.text(egui::pos2(row_rect.right() - 8.0, yc), egui::Align2::RIGHT_CENTER,
                                    &format!("{:.2}", pin_price), egui::FontId::monospace(14.0), col.gamma_multiply(0.6));
                                // Row separator
                                p.line_segment([egui::pos2(row_rect.left() + 18.0, row_rect.bottom() - 0.5), egui::pos2(row_rect.right() - 4.0, row_rect.bottom() - 0.5)],
                                    egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_TINT)));
                            }
                            // Hover + click detection over the entire pinned section
                            if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                                if sec_rect.contains(hover_pos) {
                                    let row_idx = ((hover_pos.y - sec_rect.top() - 3.0) / 30.0).floor() as usize;
                                    if row_idx < pinned_items.len() {
                                        let row_y = sec_rect.top() + 3.0 + row_idx as f32 * 30.0;
                                        let row_rect = egui::Rect::from_min_size(egui::pos2(sec_rect.left(), row_y), egui::vec2(full_w, 28.0));
                                        p.rect_filled(row_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_GHOST));
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                                        if row_idx < pinned_items.len() {
                                            let (si, ii, ref sym, ..) = pinned_items[row_idx];
                                            if hover_pos.x < sec_rect.left() + 20.0 {
                                                // Unpin
                                                if let Some(sec) = watchlist.sections.get_mut(si) {
                                                    if let Some(item) = sec.items.get_mut(ii) { item.pinned = false; }
                                                }
                                            } else {
                                                active_sym_change = Some(sym.clone());
                                            }
                                        }
                                    }
                                }
                            }
                            // Small spacer after the inset section
                            ui.allocate_exact_size(egui::vec2(full_w, 3.0), egui::Sense::hover());
                        }
                        for si in 0..section_count {
                            let sec_id = watchlist.sections[si].id;
                            let is_option_section = option_section_ids.contains(&sec_id);

                            // Option sections render in the bottom options scroll, not here
                            if is_option_section { continue; }

                            let sec_title = watchlist.sections[si].title.clone();
                            let sec_color = watchlist.sections[si].color.clone();
                            let sec_collapsed = watchlist.sections[si].collapsed;
                            let sec_item_count = watchlist.sections[si].items.len();

                            // ── Section divider line (skip if thick options divider just drawn) ──
                            if si > 0 {
                                ui.add_space(2.0);
                                let cursor_y = ui.cursor().min.y;
                                ui.painter().line_segment(
                                    [egui::pos2(ui.min_rect().left(), cursor_y),
                                     egui::pos2(ui.min_rect().left() + full_w, cursor_y)],
                                    egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_STRONG)));
                                ui.add_space(2.0);
                            }

                            // ── Track section start for continuous background ──
                            let section_block_start_y = ui.cursor().min.y;

                            // Remove item_spacing.y within section for flush rows
                            let prev_item_spacing_y = ui.spacing().item_spacing.y;
                            ui.spacing_mut().item_spacing.y = 0.0;

                            // ── Section header (only if title is non-empty) ──
                            if !sec_title.is_empty() && watchlist.renaming_section != Some(sec_id) {
                                let header_resp = ui.horizontal(|ui| {
                                    // ui.set_min_width removed — was preventing sidebar resize
                                    ui.set_min_height(20.0);

                                    // Collapse chevron
                                    let chevron = if sec_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                                    if ui.add(egui::Button::new(egui::RichText::new(chevron).size(10.0).color(t.dim.gamma_multiply(0.6))).frame(false)).clicked() {
                                        toggle_collapse = Some(si);
                                    }

                                    // Title
                                    ui.label(egui::RichText::new(&sec_title).monospace().size(9.0).strong()
                                        .color(t.dim.gamma_multiply(0.6)));

                                    // Item count when collapsed
                                    if sec_collapsed {
                                        ui.label(egui::RichText::new(format!("({})", sec_item_count)).monospace().size(8.0)
                                            .color(t.dim.gamma_multiply(0.3)));
                                    }

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Delete section (only if empty)
                                        if sec_item_count == 0 {
                                            if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(8.0).color(t.dim.gamma_multiply(0.3))).frame(false)).clicked() {
                                                remove_section = Some(si);
                                            }
                                        }
                                    });
                                });
                                section_header_rects.push((si, header_resp.response.rect));

                                // Right-click context menu on section header
                                header_resp.response.context_menu(|ui| {
                                    // Rename
                                    if ui.button(egui::RichText::new("Rename").monospace().size(10.0)).clicked() {
                                        watchlist.renaming_section = Some(sec_id);
                                        watchlist.rename_buf = sec_title.clone();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    // Color presets
                                    ui.label(egui::RichText::new("Color").monospace().size(9.0).color(t.dim));
                                    for row in color_presets.chunks(8) {
                                        ui.horizontal(|ui| {
                                            for hex in row {
                                                let c = hex_to_color(hex, 1.0);
                                                if ui.add(egui::Button::new(egui::RichText::new("\u{25CF}").size(14.0).color(c)).frame(false)).clicked() {
                                                    if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                        sec.color = Some(hex.to_string());
                                                    }
                                                    watchlist.persist();
                                                    ui.close_menu();
                                                }
                                            }
                                        });
                                    }
                                    if ui.button(egui::RichText::new("No color").monospace().size(10.0).color(t.dim)).clicked() {
                                        if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                            sec.color = None;
                                        }
                                        watchlist.persist();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if sec_item_count == 0 {
                                        if ui.button(egui::RichText::new("Delete section").monospace().size(10.0).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                                            remove_section = Some(si);
                                            ui.close_menu();
                                        }
                                    }
                                });
                            }

                            // ── Inline rename editor (replaces title in header row) ──
                            if watchlist.renaming_section == Some(sec_id) {
                                ui.horizontal(|ui| {
                                    // ui.set_min_width removed — was preventing sidebar resize
                                    ui.set_min_height(20.0);

                                    // Collapse chevron (keep visible during rename)
                                    let chevron = if sec_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                                    ui.add(egui::Button::new(egui::RichText::new(chevron).size(10.0).color(t.dim.gamma_multiply(0.6))).frame(false));

                                    let te = ui.add(egui::TextEdit::singleline(&mut watchlist.rename_buf)
                                        .desired_width((ui.available_width() - 10.0).max(40.0)).font(egui::FontId::monospace(9.0)));
                                    if te.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                            sec.title = watchlist.rename_buf.clone();
                                        }
                                        watchlist.renaming_section = None;
                                        watchlist.persist();
                                    }
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        watchlist.renaming_section = None;
                                    }
                                    te.request_focus();
                                });
                            }

                            // ── Section items (skip if collapsed) ──
                            if !sec_collapsed {
                                for ii in 0..sec_item_count {
                                    let item = &watchlist.sections[si].items[ii];
                                    let item_sym = item.symbol.clone();
                                    let item_price = item.price;
                                    let item_prev_close = item.prev_close;
                                    let item_loaded = item.loaded;
                                    let item_is_option = item.is_option;
                                    let item_underlying = item.underlying.clone();
                                    let item_option_type = item.option_type.clone();
                                    let item_strike = item.strike;
                                    let item_expiry = item.expiry.clone();
                                    let item_bid = item.bid;
                                    let item_ask = item.ask;
                                    let item_pinned = item.pinned;
                                    // Skip pinned items in normal sections — they render in the PINNED section above
                                    if item_pinned && has_pinned { continue; }
                                    let item_tags = item.tags.clone();
                                    let item_rvol = item.rvol;
                                    let item_atr = item.atr;
                                    // Populate range data from price if not set
                                    let item_high_52wk = if item.high_52wk > 0.0 { item.high_52wk } else { item.price * 1.15 };
                                    let item_low_52wk = if item.low_52wk > 0.0 { item.low_52wk } else { item.price * 0.70 };
                                    let item_day_high = if item.day_high > 0.0 { item.day_high } else { item.price * 1.008 };
                                    let item_day_low = if item.day_low > 0.0 { item.day_low } else { item.price * 0.992 };
                                    let item_avg_daily_range = item.avg_daily_range;
                                    let item_earnings_days = item.earnings_days;
                                    let item_alert_triggered = item.alert_triggered;
                                    let _item_price_history = item.price_history.clone();
                                    let is_dragged = drag_confirmed && dragging == Some((si, ii));

                                    // Skip rendering the dragged item in-place (it's shown as floating)
                                    if is_dragged {
                                        // Reserve space so layout doesn't shift
                                        let placeholder = ui.allocate_space(egui::vec2(full_w, 24.0));
                                        row_rects.push((si, ii, placeholder.1));
                                        continue;
                                    }

                                    // ── Watchlist filter ──
                                    if !item_is_option {
                                        let ft = &watchlist.filter_text;
                                        if !ft.is_empty() && !item_sym.to_uppercase().contains(&ft.to_uppercase()) {
                                            continue;
                                        }
                                        if watchlist.filter_min_change > -999.0 || watchlist.filter_max_change < 999.0 {
                                            if item_prev_close > 0.0 {
                                                let chg = (item_price / item_prev_close - 1.0) * 100.0;
                                                if watchlist.filter_min_change > -999.0 && chg < watchlist.filter_min_change { continue; }
                                                if watchlist.filter_max_change < 999.0 && chg > watchlist.filter_max_change { continue; }
                                            } else {
                                                // price not loaded yet — only skip if a strict filter is active
                                                if watchlist.filter_min_change > -999.0 || watchlist.filter_max_change < 999.0 { continue; }
                                            }
                                        }
                                    }

                                    let is_active = item_sym == active_sym;

                                    if item_is_option {
                                        // ── Option item rendering ──
                                        let opt_color = if item_option_type == "C" { t.bull } else { t.bear };
                                        let price_str = if item_bid > 0.0 || item_ask > 0.0 {
                                            format!("{:.2} \u{00D7} {:.2}", item_bid, item_ask)
                                        } else if item_price > 0.0 {
                                            format!("{:.2}", item_price)
                                        } else {
                                            "---".into()
                                        };
                                        let row_bg = if is_active { color_alpha(t.accent, 18) } else { egui::Color32::TRANSPARENT };

                                        let resp = ui.horizontal(|ui| {
                                            // ui.set_min_width removed — was preventing sidebar resize
                                            ui.set_min_height(24.0);
                                            ui.painter().rect_filled(ui.max_rect(), 0.0, row_bg);
                                            if is_active {
                                                let r = ui.max_rect();
                                                ui.painter().rect_filled(
                                                    egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + 2.5, r.max.y)),
                                                    1.0, t.accent);
                                            }
                                            ui.add_space(if is_active { 8.0 } else { 4.0 });
                                            // Drag grip
                                            ui.label(egui::RichText::new(Icon::DOTS_SIX_VERTICAL).size(9.0).color(t.dim.gamma_multiply(0.2)));
                                            ui.add_space(2.0);
                                            // C/P badge
                                            let badge_bg = color_alpha(opt_color, 35);
                                            let badge_resp = ui.add(egui::Button::new(
                                                egui::RichText::new(&item_option_type).monospace().size(9.0).strong().color(opt_color))
                                                .fill(badge_bg).corner_radius(RADIUS_SM).stroke(egui::Stroke::NONE)
                                                .min_size(egui::vec2(16.0, 16.0)));
                                            let _ = badge_resp;
                                            ui.add_space(2.0);
                                            // Full option name (e.g. "SPY 560C 0DTE")
                                            let sym_color = if is_active { t.text } else { egui::Color32::from_rgb(200, 200, 210) };
                                            ui.label(egui::RichText::new(&item_sym).monospace().size(10.5).strong().color(sym_color));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // X button
                                                if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.3))).frame(false)).clicked() {
                                                    remove_sym = Some(item_sym.clone());
                                                }
                                                // Bid x Ask (or price fallback)
                                                ui.label(egui::RichText::new(&price_str).monospace().size(11.0).color(opt_color));
                                            });
                                        });

                                        let row_rect = resp.response.rect;
                                        row_rects.push((si, ii, row_rect));

                                        let drag_resp = resp.response.interact(egui::Sense::click_and_drag());
                                        if drag_resp.drag_started() {
                                            watchlist.dragging = Some((si, ii));
                                            watchlist.drag_start_pos = pointer_pos;
                                            watchlist.drag_confirmed = false;
                                        }
                                        // Click opens option chart (not stock symbol change)
                                        if drag_resp.clicked() && !drag_confirmed {
                                            let is_call = item_option_type == "C";
                                            click_opt = Some((item_underlying.clone(), item_strike, is_call, item_expiry.clone()));
                                        }
                                        if drag_resp.hovered() && !drag_confirmed {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                            if !is_active {
                                                ui.painter().rect_filled(row_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_SUBTLE));
                                            }
                                        }
                                    } else {
                                        // ── Stock item rendering — column-aligned ──
                                        let change_pct = if item_prev_close > 0.0 { ((item_price - item_prev_close) / item_prev_close) * 100.0 } else { 0.0 };
                                        let color = if change_pct >= 0.0 { t.bull } else { t.bear };
                                        let price_str = if item_price > 0.0 { format!("{:.2}", item_price) } else { "---".into() };
                                        let change_str = if item_loaded { format!("{:+.2}%", change_pct) } else { "".into() };

                                        // Pinned section: slightly distinct background tint
                                        let row_bg = if is_active {
                                            color_alpha(t.accent, 18)
                                        } else if item_pinned {
                                            egui::Color32::from_rgba_unmultiplied(80, 120, 200, 12)
                                        } else {
                                            egui::Color32::TRANSPARENT
                                        };
                                        let row_h = if item_pinned { 34.0 } else { 28.0 };
                                        let sym_font_sz = if item_pinned { 15.0 } else { 14.0 };
                                        let chg_font_sz = if item_pinned { 15.0 } else { 14.0 };

                                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(full_w, row_h), egui::Sense::click_and_drag());
                                        let painter = ui.painter();

                                        // Background
                                        painter.rect_filled(rect, 0.0, row_bg);
                                        // Pinned row: very subtle darker tint
                                        if item_pinned {
                                            painter.rect_filled(rect, 0.0, color_alpha(t.text,4));
                                        }

                                        // Extreme movement background tint (only when move > avg)
                                        if item_prev_close > 0.0 && change_pct.abs() > item_avg_daily_range * 1.5 {
                                            let extreme_bg = if item_price >= item_prev_close {
                                                egui::Color32::from_rgba_unmultiplied(46, 204, 113, ALPHA_GHOST)
                                            } else {
                                                egui::Color32::from_rgba_unmultiplied(231, 76, 60, ALPHA_GHOST)
                                            };
                                            painter.rect_filled(rect, 0.0, extreme_bg);
                                        }

                                        // Active indicator bar
                                        if is_active {
                                            painter.rect_filled(
                                                egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 2.5, rect.max.y)),
                                                1.0, t.accent);
                                        }

                                        let y_c = rect.center().y;
                                        let left = rect.left();
                                        let row_left = left;
                                        let row_y = rect.top();
                                        let row_h_val = rect.height();

                                        // ── RVOL left border strip ──
                                        let (rvol_color, rvol_width) = if item_rvol > 3.0 {
                                            (egui::Color32::from_rgba_unmultiplied(240, 160, 40, 220), 4.0_f32)
                                        } else if item_rvol > 2.0 {
                                            (egui::Color32::from_rgba_unmultiplied(240, 160, 40, 160), 3.0_f32)
                                        } else if item_rvol > 0.8 {
                                            (egui::Color32::from_rgba_unmultiplied(46, 204, 113, ALPHA_ACTIVE), 2.0_f32)
                                        } else {
                                            (egui::Color32::from_rgba_unmultiplied(100, 150, 255, ALPHA_STRONG), 2.0_f32)
                                        };
                                        painter.rect_filled(
                                            egui::Rect::from_min_size(egui::pos2(row_left, row_y), egui::vec2(rvol_width, row_h_val)),
                                            0.0, rvol_color);

                                        // Grip dots
                                        painter.text(egui::pos2(left + 6.0, y_c), egui::Align2::LEFT_CENTER,
                                            Icon::DOTS_SIX_VERTICAL, egui::FontId::proportional(9.0), t.dim.gamma_multiply(0.2));

                                        // Star pin (left of ticker, visible on hover or when pinned)
                                        let row_hovered = resp.hovered();
                                        let star_x = left + 16.0;
                                        if row_hovered || item_pinned {
                                            let star_col = if item_pinned { egui::Color32::from_rgb(255, 193, 37) } else { t.dim.gamma_multiply(0.3) };
                                            painter.text(egui::pos2(star_x, y_c), egui::Align2::CENTER_CENTER,
                                                Icon::SPARKLE, egui::FontId::proportional(9.0), star_col);
                                        }

                                        // Symbol (shifts right when star is showing)
                                        let sym_x = if row_hovered || item_pinned { star_x + 10.0 } else { left + 18.0 };
                                        let sym_color = if is_active { t.text } else { t.text };
                                        painter.text(egui::pos2(sym_x, y_c), egui::Align2::LEFT_CENTER,
                                            &item_sym, egui::FontId::monospace(sym_font_sz), sym_color);

                                        // ── Indicator column (right of ticker name) ──
                                        let mut ind_x = sym_x + item_sym.len() as f32 * 8.5 + 6.0; // after symbol text
                                        // Earnings pill: "E:5"
                                        if item_earnings_days >= 0 && item_earnings_days <= 14 {
                                            let e_text = format!("E:{}", item_earnings_days);
                                            let e_galley = painter.layout_no_wrap(e_text.clone(), egui::FontId::monospace(7.0), egui::Color32::BLACK);
                                            let pw = e_galley.size().x + 6.0;
                                            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(ind_x, y_c - 6.0), egui::vec2(pw, 12.0)),
                                                6.0, egui::Color32::from_rgb(255, 193, 37));
                                            painter.text(egui::pos2(ind_x + pw / 2.0, y_c), egui::Align2::CENTER_CENTER,
                                                &e_text, egui::FontId::monospace(7.0), egui::Color32::BLACK);
                                            ind_x += pw + 3.0;
                                        }
                                        // Alert bell (red)
                                        if item_alert_triggered {
                                            painter.circle_filled(egui::pos2(ind_x + 5.0, y_c), 5.5, egui::Color32::from_rgb(231, 76, 60));
                                            painter.text(egui::pos2(ind_x + 5.0, y_c), egui::Align2::CENTER_CENTER,
                                                Icon::LIGHTNING, egui::FontId::proportional(6.0), egui::Color32::WHITE);
                                            ind_x += 14.0;
                                        }
                                        // Correlation dot (placeholder — green=with market, red=diverging)
                                        // TODO: compute real correlation from price data
                                        // For now show a dim neutral dot
                                        // painter.circle_filled(egui::pos2(ind_x + 5.0, y_c), 3.0, color_alpha(t.text,30));

                                        // Change % (center-left, prominent)
                                        let mid_x = rect.left() + full_w * 0.45;
                                        painter.text(egui::pos2(mid_x, y_c), egui::Align2::LEFT_CENTER,
                                            &change_str, egui::FontId::monospace(chg_font_sz), color);

                                        // Price (right-aligned, leave room for X button)
                                        let price_x = rect.right() - 24.0;
                                        painter.text(egui::pos2(price_x, y_c), egui::Align2::RIGHT_CENTER,
                                            &price_str, egui::FontId::monospace(14.0), color.gamma_multiply(0.6));

                                        // Faint row separator line
                                        painter.line_segment(
                                            [egui::pos2(rect.left() + 16.0, rect.bottom() - 0.5), egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5)],
                                            egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_MUTED)));

                                        let is_hovered = resp.hovered();

                                        // Hover actions: X (remove) on right, star click on left
                                        if row_hovered {
                                            // X button (far right)
                                            painter.text(egui::pos2(rect.right() - 8.0, y_c), egui::Align2::CENTER_CENTER,
                                                Icon::X, egui::FontId::proportional(10.0), t.dim.gamma_multiply(0.5));
                                            // Detect click position
                                            if resp.clicked() {
                                                if let Some(pos) = resp.interact_pointer_pos() {
                                                    if pos.x > rect.right() - 16.0 {
                                                        remove_sym = Some(item_sym.clone());
                                                    } else if pos.x < left + 26.0 {
                                                        // Star zone (left side) — toggle pin
                                                        if let Some(sec) = watchlist.sections.get_mut(si) {
                                                            if let Some(item) = sec.items.get_mut(ii) { item.pinned = !item.pinned; }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Hover highlight
                                        if row_hovered && !is_active {
                                            painter.rect_filled(rect, 0.0, color_alpha(t.toolbar_border, ALPHA_SOFT));
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }

                                        // ── Rich tooltip — deferred to render outside panel ──
                                        if row_hovered && !drag_confirmed {
                                            // Store tooltip data for deferred rendering after the panel
                                            set_pending_wl_tooltip(Some(WlTooltipData {
                                                sym: item_sym.clone(), price: item_price, prev_close: item_prev_close,
                                                day_high: item_day_high, day_low: item_day_low,
                                                high_52wk: item_high_52wk, low_52wk: item_low_52wk,
                                                atr: item_atr, rvol: item_rvol, avg_range: item_avg_daily_range,
                                                earnings_days: item_earnings_days, tags: item_tags.clone(),
                                                alert_triggered: item_alert_triggered,
                                                anchor_y: y_c, sidebar_left: rect.left() - 10.0,
                                            }));
                                        }
                                        // (tooltip rendered outside panel via PENDING_WL_TOOLTIP)

                                        let row_rect = rect;
                                        row_rects.push((si, ii, row_rect));

                                        // Drag-and-drop + click handling
                                        if resp.drag_started() {
                                            watchlist.dragging = Some((si, ii));
                                            watchlist.drag_start_pos = pointer_pos;
                                            watchlist.drag_confirmed = false;
                                        }
                                        if resp.clicked() && !drag_confirmed {
                                            click_sym = Some(item_sym.clone());
                                        }

                                        // (hover already handled above in painter section)
                                    }
                                }
                            }

                            // Restore item_spacing.y
                            ui.spacing_mut().item_spacing.y = prev_item_spacing_y;

                            // ── Paint continuous section background tint (header + all items) ──
                            if let Some(ref hex) = sec_color {
                                let section_block_end_y = ui.cursor().min.y;
                                if section_block_end_y > section_block_start_y {
                                    let left = ui.min_rect().left();
                                    let block_rect = egui::Rect::from_min_max(
                                        egui::pos2(left, section_block_start_y),
                                        egui::pos2(left + full_w, section_block_end_y));
                                    // Items area: low opacity tint (~18 alpha)
                                    ui.painter().rect_filled(block_rect, 0.0, hex_to_color(hex, 0.07));
                                    // Header area: darker tint overlay (~35 alpha)
                                    if let Some(&(_, header_rect)) = section_header_rects.iter().find(|&&(s, _)| s == si) {
                                        let header_tint_rect = egui::Rect::from_min_max(
                                            egui::pos2(left, header_rect.min.y),
                                            egui::pos2(left + full_w, header_rect.max.y));
                                        ui.painter().rect_filled(header_tint_rect, 0.0, hex_to_color(hex, 0.07));
                                    }
                                }
                            }
                        } // end sections loop

                        // ── Drag-and-drop logic ──
                        // Confirm drag after mouse moves enough (5px threshold)
                        if let (Some(start), Some(cur)) = (watchlist.drag_start_pos, pointer_pos) {
                            if watchlist.dragging.is_some() && !watchlist.drag_confirmed {
                                if (cur - start).length() > 5.0 {
                                    watchlist.drag_confirmed = true;
                                }
                            }
                        }

                        // Calculate drop target from mouse position
                        if watchlist.drag_confirmed {
                            if let Some(mouse) = pointer_pos {
                                let mut best: Option<(usize, usize, f32)> = None; // (sec, insert_idx, dist)
                                for &(si, ii, rect) in &row_rects {
                                    let mid_y = rect.center().y;
                                    let dist = (mouse.y - mid_y).abs();
                                    // Insert before this item if mouse is above midpoint
                                    let insert_idx = if mouse.y < mid_y { ii } else { ii + 1 };
                                    if best.is_none() || dist < best.unwrap().2 {
                                        best = Some((si, insert_idx, dist));
                                    }
                                }
                                // Also consider dropping at the end of each section
                                for &(si, rect) in &section_header_rects {
                                    if mouse.y > rect.max.y && watchlist.sections[si].items.is_empty() {
                                        best = Some((si, 0, 0.0));
                                    }
                                }
                                watchlist.drop_target = best.map(|(s, i, _)| (s, i));
                            }

                            // Draw insertion indicator line
                            if let Some((dt_sec, dt_idx)) = watchlist.drop_target {
                                // Find the Y position for the indicator
                                let indicator_y = if let Some(&(_, _, rect)) = row_rects.iter().find(|&&(s, i, _)| s == dt_sec && i == dt_idx) {
                                    rect.min.y
                                } else if dt_idx > 0 {
                                    // Insert after last item
                                    row_rects.iter().filter(|&&(s, _, _)| s == dt_sec)
                                        .last().map(|&(_, _, rect)| rect.max.y)
                                        .unwrap_or(0.0)
                                } else {
                                    // Empty section — use header rect bottom
                                    section_header_rects.iter().find(|&&(s, _)| s == dt_sec)
                                        .map(|&(_, rect)| rect.max.y + 2.0)
                                        .unwrap_or(0.0)
                                };
                                if indicator_y > 0.0 {
                                    let left = ui.min_rect().left();
                                    ui.painter().line_segment(
                                        [egui::pos2(left, indicator_y), egui::pos2(left + full_w, indicator_y)],
                                        egui::Stroke::new(STROKE_THICK, t.accent));
                                    // Small circles at endpoints
                                    ui.painter().circle_filled(egui::pos2(left + 2.0, indicator_y), 3.0, t.accent);
                                    ui.painter().circle_filled(egui::pos2(left + full_w - 2.0, indicator_y), 3.0, t.accent);
                                }
                            }

                            // Draw floating label at cursor
                            if let (Some((src_sec, src_idx)), Some(mouse)) = (watchlist.dragging, pointer_pos) {
                                if src_sec < watchlist.sections.len() && src_idx < watchlist.sections[src_sec].items.len() {
                                    let drag_sym = &watchlist.sections[src_sec].items[src_idx].symbol;
                                    let float_rect = egui::Rect::from_min_size(
                                        egui::pos2(mouse.x - 30.0, mouse.y - 10.0), egui::vec2(80.0, 20.0));
                                    ui.painter().rect_filled(float_rect, 4.0, color_alpha(t.accent, ALPHA_MUTED));
                                    ui.painter().rect_stroke(float_rect, 4.0, egui::Stroke::new(STROKE_STD, t.accent), egui::StrokeKind::Outside);
                                    ui.painter().text(float_rect.center(), egui::Align2::CENTER_CENTER,
                                        drag_sym, egui::FontId::monospace(11.0), t.text);
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                }
                            }
                        }

                        // Drop: on pointer release while dragging
                        if pointer_released && watchlist.drag_confirmed {
                            if let (Some((src_sec, src_idx)), Some((dst_sec, dst_idx))) = (watchlist.dragging, watchlist.drop_target) {
                                // Adjust destination index if same section and source is before target
                                let adj_dst = if src_sec == dst_sec && src_idx < dst_idx { dst_idx - 1 } else { dst_idx };
                                watchlist.move_item(src_sec, src_idx, dst_sec, adj_dst);
                                watchlist.persist();
                            }
                            watchlist.dragging = None;
                            watchlist.drag_start_pos = None;
                            watchlist.drop_target = None;
                            watchlist.drag_confirmed = false;
                        }
                        // Cancel drag if pointer released without confirming
                        if pointer_released && watchlist.dragging.is_some() && !watchlist.drag_confirmed {
                            watchlist.dragging = None;
                            watchlist.drag_start_pos = None;
                            watchlist.drop_target = None;
                        }
                        // Cancel drag if pointer is no longer down (safety)
                        if !pointer_down && watchlist.dragging.is_some() {
                            watchlist.dragging = None;
                            watchlist.drag_start_pos = None;
                            watchlist.drop_target = None;
                            watchlist.drag_confirmed = false;
                        }

                        // ── Add section button ──
                        // "+ Section" always at bottom of stocks scroll
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.add(egui::Button::new(egui::RichText::new(format!("{} Section", Icon::PLUS)).monospace().size(9.0).color(t.dim.gamma_multiply(0.4)))
                                .frame(false)).clicked() {
                                watchlist.add_section("New Section");
                                watchlist.persist();
                            }
                        });

                        if let Some(sym) = click_sym {
                            panes[ap].pending_symbol_change = Some(sym.clone());
                            panes[ap].is_option = false; // reset option flag when switching to stock
                        }
                        if let Some(opt_info) = click_opt {
                            open_option_chart = Some(opt_info);
                        }
                        if let Some(sym) = remove_sym { watchlist.remove_symbol(&sym); watchlist.persist(); }
                        if let Some(si) = toggle_collapse {
                            watchlist.sections[si].collapsed = !watchlist.sections[si].collapsed;
                            watchlist.persist();
                        }
                        if let Some(si) = remove_section {
                            if si < watchlist.sections.len() && watchlist.sections[si].items.is_empty() {
                                watchlist.sections.remove(si);
                                watchlist.persist();
                            }
                        }
                    }); // end stocks scroll

                    // ── Draggable divider + Options scroll ──
                    if show_opts {
                        // Divider bar — allocate a draggable strip, decoupled from egui interaction
                        ui.add_space(2.0);
                        let div_r = ui.available_rect_before_wrap();
                        let div_y = ui.cursor().min.y;
                        let div_rect = egui::Rect::from_min_max(
                            egui::pos2(div_r.left(), div_y),
                            egui::pos2(div_r.right(), div_y + 6.0));
                        ui.painter().rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(div_r.left(), div_y + 1.0),
                                egui::pos2(div_r.right(), div_y + 4.0)),
                            0.0, color_alpha(t.toolbar_border, 160));
                        // Store divider Y position for drag handling outside the panel
                        watchlist.divider_y = div_rect.center().y;
                        watchlist.divider_total_h = total_avail;
                        // Show resize cursor on hover
                        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                            if div_rect.expand(6.0).contains(pos) || watchlist.divider_dragging {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                            }
                        }
                        ui.add_space(6.0);

                        // OPTIONS label
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("OPTIONS").monospace().size(9.0).strong().color(t.accent.gamma_multiply(0.7)));
                            let opt_count: usize = watchlist.sections.iter()
                                .filter(|s| s.title.contains("Options"))
                                .map(|s| s.items.len()).sum();
                            if opt_count > 0 {
                                ui.label(egui::RichText::new(format!("({})", opt_count)).monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                            }
                        });
                        ui.add_space(2.0);

                        egui::ScrollArea::vertical().id_salt("wl_options").show(ui, |ui| {
                            let active_sym = panes[ap].symbol.clone();
                            let mut click_opt: Option<(String, f32, bool, String)> = None;
                            let mut remove_sym: Option<String> = None;
                            let mut opt_remove_section: Option<usize> = None;
                            let mut opt_toggle_collapse: Option<usize> = None;
                            let color_presets = ["#4a9eff","#e74c3c","#2ecc71","#f39c12","#9b59b6","#1abc9c","#e67e22","#3498db","#e91e63","#00bcd4","#8bc34a","#ff5722","#607d8b","#795548","#cddc39","#ff9800"];

                            for si in 0..watchlist.sections.len() {
                                if !option_section_ids.contains(&watchlist.sections[si].id) { continue; }
                                let sec_id = watchlist.sections[si].id;
                                let sec_title = watchlist.sections[si].title.clone();
                                let sec_color = watchlist.sections[si].color.clone();
                                let sec_collapsed = watchlist.sections[si].collapsed;
                                let sec_item_count = watchlist.sections[si].items.len();
                                let full_w = ui.available_width();

                                let section_block_start_y = ui.cursor().min.y;

                                // Section header with collapse chevron
                                let header_resp = ui.horizontal(|ui| {
                                    // ui.set_min_width removed — was preventing sidebar resize
                                    ui.set_min_height(20.0);
                                    let chevron = if sec_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                                    if ui.add(egui::Button::new(egui::RichText::new(chevron).size(10.0).color(t.dim.gamma_multiply(0.6))).frame(false)).clicked() {
                                        opt_toggle_collapse = Some(si);
                                    }
                                    ui.label(egui::RichText::new(&sec_title).monospace().size(9.0).strong().color(t.dim.gamma_multiply(0.6)));
                                    if sec_collapsed {
                                        ui.label(egui::RichText::new(format!("({})", sec_item_count)).monospace().size(8.0).color(t.dim.gamma_multiply(0.3)));
                                    }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if sec_item_count == 0 {
                                            if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(8.0).color(t.dim.gamma_multiply(0.3))).frame(false)).clicked() {
                                                opt_remove_section = Some(si);
                                            }
                                        }
                                    });
                                });

                                // Right-click context menu on option section header (same as stock sections)
                                header_resp.response.context_menu(|ui| {
                                    // Rename
                                    if ui.button(egui::RichText::new("Rename").monospace().size(10.0)).clicked() {
                                        watchlist.renaming_section = Some(sec_id);
                                        watchlist.rename_buf = sec_title.clone();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    // Color presets
                                    ui.label(egui::RichText::new("Color").monospace().size(9.0).color(t.dim));
                                    for row in color_presets.chunks(8) {
                                        ui.horizontal(|ui| {
                                            for hex in row {
                                                let c = hex_to_color(hex, 1.0);
                                                if ui.add(egui::Button::new(egui::RichText::new("\u{25CF}").size(14.0).color(c)).frame(false)).clicked() {
                                                    if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                        sec.color = Some(hex.to_string());
                                                    }
                                                    watchlist.persist();
                                                    ui.close_menu();
                                                }
                                            }
                                        });
                                    }
                                    if ui.button(egui::RichText::new("No color").monospace().size(10.0).color(t.dim)).clicked() {
                                        if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                            sec.color = None;
                                        }
                                        watchlist.persist();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if sec_item_count == 0 {
                                        if ui.button(egui::RichText::new("Delete section").monospace().size(10.0).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                                            opt_remove_section = Some(si);
                                            ui.close_menu();
                                        }
                                    }
                                });
                                ui.add_space(2.0);

                                if !sec_collapsed {
                                    for ii in 0..sec_item_count {
                                        let item = &watchlist.sections[si].items[ii];
                                        let item_sym = item.symbol.clone();
                                        let item_underlying = item.underlying.clone();
                                        let item_option_type = item.option_type.clone();
                                        let item_strike = item.strike;
                                        let item_expiry = item.expiry.clone();
                                        let item_bid = item.bid;
                                        let item_ask = item.ask;
                                        let is_call = item_option_type == "C";
                                        let color = if is_call { t.bull } else { t.bear };
                                        let is_active = item_sym == active_sym;
                                        let row_bg = if is_active { color_alpha(t.accent, 18) } else { egui::Color32::TRANSPARENT };

                                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(full_w, 28.0), egui::Sense::click());
                                        let painter = ui.painter();
                                        painter.rect_filled(rect, 0.0, row_bg);
                                        if resp.hovered() {
                                            painter.rect_filled(rect, 0.0, color_alpha(t.toolbar_border, ALPHA_SUBTLE));
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }

                                        let badge = if is_call { "C" } else { "P" };
                                        let y_c = rect.center().y;
                                        // C/P badge
                                        painter.text(egui::pos2(rect.left() + 6.0, y_c), egui::Align2::LEFT_CENTER,
                                            badge, egui::FontId::monospace(11.0), color);
                                        // Contract name
                                        painter.text(egui::pos2(rect.left() + 22.0, y_c), egui::Align2::LEFT_CENTER,
                                            &format!("{} {:.0} {}", item_underlying, item_strike, item_expiry),
                                            egui::FontId::monospace(14.0), t.text);
                                        // Bid x Ask (right-aligned)
                                        if item_bid > 0.0 || item_ask > 0.0 {
                                            painter.text(egui::pos2(rect.right() - 6.0, y_c), egui::Align2::RIGHT_CENTER,
                                                &format!("{:.2} x {:.2}", item_bid, item_ask),
                                                egui::FontId::monospace(14.0), color.gamma_multiply(0.7));
                                        }
                                        // Faint separator
                                        painter.line_segment(
                                            [egui::pos2(rect.left() + 16.0, rect.bottom() - 0.5), egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5)],
                                            egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_MUTED)));

                                        if resp.clicked() {
                                            click_opt = Some((item_underlying.clone(), item_strike, is_call, item_expiry.clone()));
                                        }

                                        // X button to remove
                                        let x_rect = egui::Rect::from_min_size(egui::pos2(rect.right() - 16.0, rect.top()), egui::vec2(16.0, 22.0));
                                        if resp.hovered() {
                                            let x_resp = ui.interact(x_rect, egui::Id::new(("opt_x", si, ii, "opt_item")), egui::Sense::click());
                                            if x_resp.clicked() { remove_sym = Some(item_sym.clone()); }
                                        }
                                    }
                                }

                                // Paint continuous section background tint
                                let section_block_end_y = ui.cursor().min.y;
                                if let Some(ref hex) = sec_color {
                                    if section_block_end_y > section_block_start_y {
                                        let left = ui.min_rect().left();
                                        let block_rect = egui::Rect::from_min_max(
                                            egui::pos2(left, section_block_start_y),
                                            egui::pos2(left + full_w, section_block_end_y));
                                        ui.painter().rect_filled(block_rect, 0.0, hex_to_color(hex, 0.07));
                                    }
                                }
                                ui.add_space(4.0);
                            }

                            // Empty state
                            if option_section_ids.is_empty() {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("No options saved").monospace().size(10.0).color(t.dim.gamma_multiply(0.35)));
                                ui.label(egui::RichText::new("Shift+click contracts in the CHAIN tab").monospace().size(8.0).color(t.dim.gamma_multiply(0.25)));
                                ui.add_space(8.0);
                            }

                            // "+ Section" button at bottom of options area
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui.add(egui::Button::new(egui::RichText::new(format!("{} Section", Icon::PLUS)).monospace().size(9.0).color(t.dim.gamma_multiply(0.4)))
                                    .frame(false)).clicked() {
                                    watchlist.add_option_section("New Options");
                                    watchlist.persist();
                                }
                            });

                            if let Some(opt_info) = click_opt {
                                open_option_chart = Some(opt_info);
                            }
                            if let Some(sym) = remove_sym {
                                watchlist.remove_symbol(&sym);
                                watchlist.persist();
                            }
                            if let Some(si) = opt_toggle_collapse {
                                watchlist.sections[si].collapsed = !watchlist.sections[si].collapsed;
                                watchlist.persist();
                            }
                            if let Some(si) = opt_remove_section {
                                if si < watchlist.sections.len() && watchlist.sections[si].items.is_empty() {
                                    watchlist.sections.remove(si);
                                    watchlist.persist();
                                }
                            }
                        });
                    }
                }

                // ── CHAIN TAB ───────────────────────────────────────────
                WatchlistTab::Chain => {
                    // Chain price: prefer IB underlying price, then watchlist, then chart, then fallback
                    let chain_price = if watchlist.chain_underlying_price > 0.0 {
                        watchlist.chain_underlying_price
                    } else {
                        watchlist.find_item(&watchlist.chain_symbol).map(|i| i.price)
                            .or_else(|| panes.iter().find(|p| p.symbol == watchlist.chain_symbol).and_then(|p| p.bars.last().map(|b| b.close)))
                            .unwrap_or(0.0)
                    };
                    if watchlist.chain_0dte.0.is_empty() && !watchlist.chain_loading {
                        let ns = watchlist.chain_num_strikes;
                        let sym = watchlist.chain_symbol.clone();
                        let far_dte = watchlist.chain_far_dte;
                        watchlist.chain_loading = true;
                        watchlist.chain_last_fetch = Some(std::time::Instant::now());
                        fetch_chain_background(sym.clone(), ns, 0, chain_price);
                        fetch_chain_background(sym, ns, far_dte, chain_price);
                    }

                    // ── Controls: DTE selector | sel toggle | Spread ──
                    ui.horizontal(|ui| {
                        // DTE dropdown
                        let dte_values = [1, 2, 3, 5, 7, 10];
                        let cur_label = dte_label(watchlist.chain_far_dte);
                        dim_label(ui, "DTE", t.dim);
                        egui::ComboBox::from_id_salt("far_dte").selected_text(egui::RichText::new(&cur_label).monospace().size(9.0).color(t.dim)).width(100.0)
                            .show_ui(ui, |ui| {
                                for &d in &dte_values {
                                    let label = dte_label(d);
                                    if ui.selectable_value(&mut watchlist.chain_far_dte, d, &label).changed() {
                                        let sym = watchlist.chain_symbol.clone();
                                        watchlist.chain_loading = true;
                                        fetch_chain_background(sym, watchlist.chain_num_strikes, d, chain_price);
                                    }
                                }
                            });
                        // Select mode toggle
                        let sel_active = watchlist.chain_select_mode;
                        if ui.add(egui::Button::new(egui::RichText::new(if sel_active { format!("{} sel", Icon::CHECK) } else { "sel".into() }).monospace().size(9.0)
                            .color(if sel_active { t.accent } else { t.dim }))
                            .fill(if sel_active { egui::Color32::from_rgba_unmultiplied(t.accent.r(),t.accent.g(),t.accent.b(),51) } else { t.toolbar_bg })
                            .stroke(egui::Stroke::new(STROKE_STD, if sel_active { t.accent } else { t.toolbar_border }))
                            .corner_radius(RADIUS_SM)).clicked() {
                            watchlist.chain_select_mode = !watchlist.chain_select_mode;
                        }
                        // Spread Builder shortcut
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if small_action_btn(ui, "Spread", t.dim) {
                                watchlist.spread_open = !watchlist.spread_open;
                            }
                        });
                    });

                    ui.add_space(4.0);

                    // ── Symbol selector + price ──
                    ui.horizontal(|ui| {
                        let has_focus = ui.memory(|m| m.has_focus(egui::Id::new("chain_sym_edit")));
                        let input_bg = if has_focus { color_alpha(t.toolbar_border, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_GHOST) };
                        let sym_resp = ui.add(egui::TextEdit::singleline(&mut watchlist.chain_sym_input)
                            .id(egui::Id::new("chain_sym_edit"))
                            .hint_text(&watchlist.chain_symbol)
                            .desired_width(70.0)
                            .font(egui::FontId::monospace(14.0))
                            .text_color(t.accent)
                            .background_color(input_bg)
                            .margin(egui::Margin::symmetric(4, 3)));
                        if !has_focus {
                            let display_text = if watchlist.chain_sym_input.is_empty() { &watchlist.chain_symbol } else { &watchlist.chain_sym_input };
                            let r = sym_resp.rect;
                            ui.painter().text(egui::pos2(r.left() + 6.0, r.center().y), egui::Align2::LEFT_CENTER,
                                display_text, egui::FontId::monospace(14.0), t.accent);
                        }
                        // Price display
                        if chain_price > 0.0 {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(format!("${:.2}", chain_price)).monospace().size(14.0).color(TEXT_PRIMARY));
                        }
                        // Search — static immediate + ApexIB background
                        if sym_resp.changed() && !watchlist.chain_sym_input.is_empty() {
                            watchlist.search_results = crate::ui_kit::symbols::search_symbols(&watchlist.chain_sym_input, 5)
                                .iter().map(|s| (s.symbol.to_string(), s.name.to_string())).collect();
                            // Also fire ApexIB search in background
                            fetch_search_background(watchlist.chain_sym_input.clone(), "chain".to_string());
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !watchlist.chain_sym_input.is_empty() {
                            watchlist.chain_symbol = watchlist.chain_sym_input.trim().to_uppercase();
                            watchlist.chain_sym_input.clear();
                            watchlist.search_results.clear();
                            watchlist.chain_0dte = (vec![], vec![]);
                            watchlist.chain_underlying_price = 0.0; // reset price for new symbol
                            watchlist.chain_center_offset = 0;
                            watchlist.chain_loading = false;
                        }
                    });
                    // Search suggestions popup
                    if !watchlist.chain_sym_input.is_empty() && !watchlist.search_results.is_empty() {
                        egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(28, 28, 34)).corner_radius(4.0).show(ui, |ui| {
                            for (sym, name) in watchlist.search_results.clone() {
                                if ui.add(egui::Button::new(egui::RichText::new(format!("{} {}", sym, name)).monospace().size(11.0).color(t.dim))
                                    .frame(false).min_size(egui::vec2(ui.available_width(), 20.0))).clicked() {
                                    watchlist.chain_symbol = sym;
                                    watchlist.chain_sym_input.clear();
                                    watchlist.search_results.clear();
                                    watchlist.chain_0dte = (vec![], vec![]);
                                    watchlist.chain_underlying_price = 0.0;
                                    watchlist.chain_center_offset = 0;
                                    watchlist.chain_loading = false;
                                }
                            }
                        });
                    }

                    ui.add_space(4.0);
                    // Separator before chain data
                    let sep_r = ui.available_rect_before_wrap();
                    ui.painter().line_segment(
                        [egui::pos2(sep_r.left(), ui.cursor().min.y), egui::pos2(sep_r.right(), ui.cursor().min.y)],
                        egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_MUTED)));
                    ui.add_space(4.0);

                    // Loading indicator
                    if watchlist.chain_loading {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new("Loading chain...").monospace().size(10.0).color(t.dim));
                        });
                    }

                    // ── Column layout ──
                    // Each data column needs space for ~8 chars of monospace 10px (~6.5px each = ~52px)
                    // Plus 8px gap between columns
                    let full_w = ui.available_width();
                    let gap = 8.0;
                    let col_chk = 14.0;
                    let col_stk = 44.0;
                    let col_bid = 56.0;
                    let col_ask = 56.0;
                    let col_oi  = 56.0;
                    // If panel is wide enough, expand proportionally
                    let used = col_chk + col_stk + col_bid + col_ask + col_oi + gap * 4.0;
                    let scale = if full_w > used { full_w / used } else { 1.0 };
                    let col_stk = col_stk * scale;
                    let col_bid = col_bid * scale;
                    let col_ask = col_ask * scale;
                    let col_oi = col_oi * scale;

                    // Column headers
                    ui.horizontal(|ui| {
                        // ui.set_min_width removed — was preventing sidebar resize
                        ui.spacing_mut().item_spacing.x = gap;
                        let hdr_color = t.dim.gamma_multiply(0.4);
                        ui.add_space(col_chk);
                        ui.allocate_ui(egui::vec2(col_stk, 14.0), |ui| { dim_label(ui, "STK", hdr_color); });
                        ui.allocate_ui(egui::vec2(col_bid, 14.0), |ui| { dim_label(ui, "BID", hdr_color); });
                        ui.allocate_ui(egui::vec2(col_ask, 14.0), |ui| { dim_label(ui, "ASK", hdr_color); });
                        ui.allocate_ui(egui::vec2(col_oi, 14.0), |ui| { dim_label(ui, "OI", hdr_color); });
                    });

                    // ── Helper to render one option row ──
                    // Track clicked contract for opening chart (normal click)
                    let clicked_contract: std::cell::Cell<Option<(String, f32, bool, String)>> = std::cell::Cell::new(None);
                    // Track shift-clicked contract for adding to watchlist (select mode / shift+click)
                    let watchlist_add: std::cell::Cell<Option<(String, f32, bool, String, f32, f32)>> = std::cell::Cell::new(None);
                    let render_row = |ui: &mut egui::Ui, row: &OptionRow, is_call: bool, exp_label: &str, sym: &str, saved: &mut Vec<SavedOption>, select_mode: bool, w: f32| {
                        let is_saved = saved.iter().any(|s| s.contract == row.contract);
                        let color = if is_call { t.bull } else { t.bear };
                        let base_tint = if is_call { color_alpha(t.bull, 8) } else { color_alpha(t.bear, 8) };
                        let itm_bg = if row.itm { color.gamma_multiply(0.06) } else { base_tint };
                        let saved_bg = if is_saved { color_alpha(t.accent, ALPHA_MUTED) } else { itm_bg };

                        // Reserve a clickable rect for the whole row
                        let (rect, row_resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());

                        // Paint background
                        let bg = if row_resp.hovered() { color_alpha(t.toolbar_border, ALPHA_LINE) } else { saved_bg };
                        ui.painter().rect_filled(rect, 0.0, bg);
                        if row_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

                        let mut x = rect.left();
                        let y_center = rect.center().y;
                        let painter = ui.painter();

                        // Check mark
                        if is_saved {
                            painter.text(egui::pos2(x + col_chk * 0.5, y_center), egui::Align2::CENTER_CENTER,
                                Icon::CHECK, egui::FontId::proportional(12.0), t.accent);
                        }
                        x += col_chk + gap;

                        // Strike
                        painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                            &format!("{:.0}", row.strike), egui::FontId::monospace(14.0), t.text);
                        x += col_stk + gap;

                        // Bid
                        painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                            &format!("{:.2}", row.bid), egui::FontId::monospace(14.0), color);
                        x += col_bid + gap;

                        // Ask
                        painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                            &format!("{:.2}", row.ask), egui::FontId::monospace(14.0), t.dim);
                        x += col_ask + gap;

                        // OI
                        let oi_str = if row.oi >= 1_000_000 { format!("{:.1}M", row.oi as f32 / 1_000_000.0) }
                            else if row.oi >= 1_000 { format!("{},{:03}", row.oi / 1000, row.oi % 1000) }
                            else { format!("{}", row.oi) };
                        let oi_x = x;
                        painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                            &oi_str, egui::FontId::monospace(12.0), t.dim.gamma_multiply(0.5));

                        // IV indicator — left edge strip on the row
                        if row.iv > 0.0 {
                            let iv_color = if row.iv > 0.7 { egui::Color32::from_rgba_unmultiplied(231, 76, 60, 180) }
                                else if row.iv > 0.5 { egui::Color32::from_rgba_unmultiplied(240, 160, 40, 140) }
                                else if row.iv > 0.3 { egui::Color32::from_rgba_unmultiplied(255, 193, 37, ALPHA_ACTIVE) }
                                else { egui::Color32::from_rgba_unmultiplied(46, 204, 113, ALPHA_ACTIVE) };
                            painter.rect_filled(egui::Rect::from_min_size(
                                egui::pos2(rect.left(), rect.top()), egui::vec2(3.0, rect.height())),
                                0.0, iv_color);
                        }

                        // Unusual activity — badge background around OI number
                        let is_unusual = row.volume > row.oi && row.volume > 100;
                        if is_unusual {
                            // Highlight the OI text area with a gold badge background
                            let oi_badge_rect = egui::Rect::from_min_size(
                                egui::pos2(oi_x - 2.0, rect.top() + 1.0), egui::vec2(col_oi + 4.0, rect.height() - 2.0));
                            painter.rect_filled(oi_badge_rect, 3.0, egui::Color32::from_rgba_unmultiplied(255, 193, 37, ALPHA_TINT));
                            painter.rect_stroke(oi_badge_rect, 3.0, egui::Stroke::new(STROKE_THIN, egui::Color32::from_rgba_unmultiplied(255, 193, 37, ALPHA_STRONG)), egui::StrokeKind::Outside);
                        }

                        // Faint row separator
                        painter.line_segment(
                            [egui::pos2(rect.left() + 4.0, rect.bottom() - 0.5), egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5)],
                            egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_TINT)));

                        // Click handling
                        if row_resp.clicked() {
                            if select_mode || ui.input(|i| i.modifiers.shift) {
                                if is_saved { saved.retain(|s| s.contract != row.contract); }
                                else { saved.push(SavedOption { contract: row.contract.clone(), symbol: sym.into(), strike: row.strike, is_call, expiry: exp_label.into(), last: row.last }); }
                                watchlist_add.set(Some((sym.into(), row.strike, is_call, exp_label.into(), row.bid, row.ask)));
                            } else {
                                clicked_contract.set(Some((sym.into(), row.strike, is_call, exp_label.into())));
                            }
                        }
                    };

                    // ── Helper to render one expiry block ──
                    let chain_frozen = watchlist.chain_frozen;
                    // Per-chain controls passed as parameters to render_block

                    let render_block = |ui: &mut egui::Ui, dte: i32, calls: &[OptionRow], puts: &[OptionRow], sym: &str, price: f32, saved: &mut Vec<SavedOption>, select_mode: bool, w: f32, num_strikes: usize, center_offset: i32, strike_mode: StrikeMode, nmf: u8| {
                        let exp_label = format!("{}DTE", dte);
                        let date_str = if dte == 0 {
                            "Today".to_string()
                        } else {
                            let (_, m, d) = trading_date(dte);
                            format!("{} {}", trading_month_name(m), d)
                        };
                        // Expiry header
                        ui.horizontal(|ui| {
                            // min_width removed — was preventing sidebar resize
                            ui.label(egui::RichText::new(&exp_label).monospace().size(12.0).strong().color(t.accent));
                            ui.label(egui::RichText::new(&date_str).monospace().size(11.0).color(t.dim.gamma_multiply(0.6)));
                        });
                        ui.add_space(2.0);

                        // Collect all unique strikes from calls + puts, sorted ascending
                        let mut all_strikes: Vec<f32> = calls.iter().chain(puts.iter())
                            .map(|r| r.strike).collect();
                        all_strikes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                        all_strikes.dedup();

                        // Find the ATM index (closest strike to price)
                        let atm_idx = all_strikes.iter().enumerate()
                            .min_by(|(_, a), (_, b)| ((**a - price).abs()).partial_cmp(&((**b - price).abs())).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(i, _)| i).unwrap_or(0);

                        // The offset shifts the center. The price badge always shows real price.
                        // We select num_strikes above the shifted center and num_strikes below.
                        if all_strikes.is_empty() {
                            ui.label(egui::RichText::new("No strikes available").monospace().size(10.0).color(t.dim.gamma_multiply(0.4)));
                            return;
                        }

                        // Window: offset shifts which strikes are visible, but divider stays at real price
                        let max_idx = (all_strikes.len() as i32 - 1).max(0);
                        // σ approximated as 1.5% of price until real HV data
                        let sigma = price * 0.015;

                        // Near/Mid/Far: determines where calls and puts START
                        // Near (0): calls/puts start right at ATM
                        // Mid (1): calls start from price+1σ upward, puts from price-1σ downward
                        // Far (2): calls start from price+2σ upward, puts from price-2σ downward
                        let nmf_sigma = nmf as f32; // 0, 1, or 2
                        let call_start_price = price + nmf_sigma * sigma;
                        let put_start_price = price - nmf_sigma * sigma;

                        // For Near: single symmetric window (classic behavior)
                        // For Mid/Far: calls start from +Nσ, puts from -Nσ
                        let visible_strikes: Vec<f32> = if nmf == 0 {
                            // NEAR: symmetric window centered on ATM, same as original
                            match strike_mode {
                                StrikeMode::Count => {
                                    let window_center = (atm_idx as i32 + center_offset).clamp(0, max_idx) as usize;
                                    let start = window_center.saturating_sub(num_strikes);
                                    let end = (window_center + num_strikes).min(all_strikes.len());
                                    all_strikes[start..end].to_vec()
                                }
                                StrikeMode::Pct(pct_idx) => {
                                    let pct = PCT_OPTIONS.get(pct_idx as usize).copied().unwrap_or(1.0) / 100.0;
                                    all_strikes.iter().filter(|&&s| (s - price).abs() / price <= pct).copied().collect()
                                }
                                StrikeMode::StdDev => {
                                    all_strikes.iter().filter(|&&s| (s - price).abs() <= sigma * 2.0).copied().collect()
                                }
                            }
                        } else {
                            // MID/FAR: calls start from +Nσ, puts from -Nσ
                            let call_start_idx = all_strikes.iter().position(|&s| s >= call_start_price).unwrap_or(all_strikes.len());
                            let put_end_idx = all_strikes.iter().rposition(|&s| s <= put_start_price).unwrap_or(0);
                            // Arrow offset shifts both in the same direction
                            let call_start = (call_start_idx as i32 + center_offset).clamp(0, all_strikes.len() as i32) as usize;
                            let put_end = (put_end_idx as i32 + center_offset).clamp(0, max_idx) as usize;
                            match strike_mode {
                                StrikeMode::Count => {
                                    let call_end = (call_start + num_strikes).min(all_strikes.len());
                                    let put_begin = put_end.saturating_sub(num_strikes.saturating_sub(1));
                                    let mut strikes = Vec::new();
                                    for i in put_begin..=put_end.min(all_strikes.len().saturating_sub(1)) { strikes.push(all_strikes[i]); }
                                    for i in call_start..call_end { if !strikes.contains(&all_strikes[i]) { strikes.push(all_strikes[i]); } }
                                    strikes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                                    strikes
                                }
                                StrikeMode::Pct(pct_idx) => {
                                    let pct = PCT_OPTIONS.get(pct_idx as usize).copied().unwrap_or(1.0) / 100.0;
                                    all_strikes.iter().filter(|&&s| {
                                        if s >= price { s >= call_start_price && (s - call_start_price) / price <= pct }
                                        else { s <= put_start_price && (put_start_price - s) / price <= pct }
                                    }).copied().collect()
                                }
                                StrikeMode::StdDev => {
                                    all_strikes.iter().filter(|&&s| {
                                        if s >= price { s >= call_start_price && s <= call_start_price + sigma }
                                        else { s <= put_start_price && s >= put_start_price - sigma }
                                    }).copied().collect()
                                }
                            }
                        };

                        // ALWAYS split at the real price — divider never moves with arrows
                        // Calls: visible strikes ABOVE the real price
                        let sorted_calls: Vec<&OptionRow> = {
                            let mut v: Vec<&OptionRow> = calls.iter()
                                .filter(|r| visible_strikes.contains(&r.strike) && r.strike > price)
                                .collect();
                            v.sort_by(|a, b| b.strike.partial_cmp(&a.strike).unwrap_or(std::cmp::Ordering::Equal));
                            v
                        };
                        // Puts: visible strikes AT or BELOW the real price
                        let sorted_puts: Vec<&OptionRow> = {
                            let mut v: Vec<&OptionRow> = puts.iter()
                                .filter(|r| visible_strikes.contains(&r.strike) && r.strike <= price)
                                .collect();
                            v.sort_by(|a, b| b.strike.partial_cmp(&a.strike).unwrap_or(std::cmp::Ordering::Equal));
                            v
                        };

                        // Calls (OTM at top, ATM at bottom)
                        for row in &sorted_calls { render_row(ui, row, true, &exp_label, sym, saved, select_mode, w); }

                        // ── ATM price badge divider ──
                        ui.add_space(3.0);
                        {
                            let r = ui.available_rect_before_wrap();
                            let y = ui.cursor().min.y;
                            let badge_w = 80.0;
                            let center_x = r.left() + r.width() / 2.0;
                            // Lines on either side of the badge
                            ui.painter().line_segment(
                                [egui::pos2(r.left() + 4.0, y + 10.0), egui::pos2(center_x - badge_w / 2.0 - 4.0, y + 10.0)],
                                egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_STRONG)));
                            ui.painter().line_segment(
                                [egui::pos2(center_x + badge_w / 2.0 + 4.0, y + 10.0), egui::pos2(r.right() - 4.0, y + 10.0)],
                                egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_STRONG)));
                            // Badge background
                            let badge_rect = egui::Rect::from_center_size(egui::pos2(center_x, y + 10.0), egui::vec2(badge_w, 18.0));
                            ui.painter().rect_filled(badge_rect, 9.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
                            ui.painter().rect_stroke(badge_rect, 9.0, egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_STRONG)), egui::StrokeKind::Outside);
                            // Price text
                            let badge_text = if center_offset != 0 {
                                format!("${:.2} ({:+})", price, center_offset)
                            } else {
                                format!("${:.2}", price)
                            };
                            ui.painter().text(badge_rect.center(), egui::Align2::CENTER_CENTER,
                                &badge_text, egui::FontId::monospace(11.0),
                                TEXT_PRIMARY);
                        }
                        ui.add_space(22.0);

                        // Puts (ATM at top, OTM at bottom)
                        for row in &sorted_puts { render_row(ui, row, false, &exp_label, sym, saved, select_mode, w); }
                        ui.add_space(4.0);
                    };

                    // ── Scroll area with two expiry blocks ──
                    let scroll_w = ui.available_width();
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        // min_width removed — was preventing sidebar resize
                        let sym = watchlist.chain_symbol.clone();
                        let sel = watchlist.chain_select_mode;
                        let calls_0 = watchlist.chain_0dte.0.clone();
                        let puts_0 = watchlist.chain_0dte.1.clone();
                        let calls_f = watchlist.chain_far.0.clone();
                        let puts_f = watchlist.chain_far.1.clone();
                        let far_dte = watchlist.chain_far_dte;

                        // Per-chain controls: 0DTE
                        ui.horizontal(|ui| {
                            dim_label(ui, "0DTE", t.dim);
                            // Mode dropdown (Count, %, StdDev)
                            let mode_label = match watchlist.chain_0_strike_mode {
                                StrikeMode::Count => "Cnt".into(),
                                StrikeMode::Pct(i) => format!("{}%", PCT_OPTIONS.get(i as usize).unwrap_or(&1.0)),
                                StrikeMode::StdDev => "σ".into(),
                            };
                            egui::ComboBox::from_id_salt("sm_0").selected_text(egui::RichText::new(&mode_label).monospace().size(8.0)).width(40.0).show_ui(ui, |ui| {
                                if ui.selectable_label(matches!(watchlist.chain_0_strike_mode, StrikeMode::Count), "Count").clicked() { watchlist.chain_0_strike_mode = StrikeMode::Count; }
                                for (pi, &pct) in PCT_OPTIONS.iter().enumerate() {
                                    if ui.selectable_label(watchlist.chain_0_strike_mode == StrikeMode::Pct(pi as u8), format!("{}%", pct)).clicked() { watchlist.chain_0_strike_mode = StrikeMode::Pct(pi as u8); }
                                }
                                if ui.selectable_label(matches!(watchlist.chain_0_strike_mode, StrikeMode::StdDev), "Std Dev").clicked() { watchlist.chain_0_strike_mode = StrikeMode::StdDev; }
                            });
                            // Count ± (always visible)
                            if ui.add(egui::Button::new(egui::RichText::new("-").monospace().size(9.0)).min_size(egui::vec2(14.0, 14.0))).clicked() { watchlist.chain_0_num_strikes = watchlist.chain_0_num_strikes.saturating_sub(1).max(1); }
                            ui.label(egui::RichText::new(format!("{}", watchlist.chain_0_num_strikes)).monospace().size(8.0).color(t.dim));
                            if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(9.0)).min_size(egui::vec2(14.0, 14.0))).clicked() { watchlist.chain_0_num_strikes += 1; }
                            // Near / Mid / Far toggles
                            for (lvl, label) in [(0u8, "N"), (1, "M"), (2, "F")] {
                                let active = watchlist.chain_0_nmf == lvl;
                                let col = if active { t.accent } else { t.dim.gamma_multiply(0.4) };
                                if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(8.0).color(col))
                                    .fill(if active { color_alpha(t.accent, ALPHA_SUBTLE) } else { egui::Color32::TRANSPARENT })
                                    .min_size(egui::vec2(14.0, 14.0)).corner_radius(RADIUS_SM)).clicked() { watchlist.chain_0_nmf = lvl; }
                            }
                            // Freeze + arrows
                            let fr_col = if watchlist.chain_0_frozen { t.accent } else { t.dim.gamma_multiply(0.4) };
                            if ui.add(egui::Button::new(egui::RichText::new(if watchlist.chain_0_frozen { Icon::PAUSE } else { Icon::PLAY }).size(9.0).color(fr_col)).fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(14.0, 14.0))).clicked() {
                                watchlist.chain_0_frozen = !watchlist.chain_0_frozen;
                                if !watchlist.chain_0_frozen { watchlist.chain_0_offset = 0; }
                            }
                            if watchlist.chain_0_frozen {
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::ARROW_FAT_UP).size(9.0).color(t.dim)).fill(color_alpha(t.toolbar_border, ALPHA_GHOST)).min_size(egui::vec2(14.0, 14.0))).clicked() { watchlist.chain_0_offset += 1; }
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::ARROW_FAT_DOWN).size(9.0).color(t.dim)).fill(color_alpha(t.toolbar_border, ALPHA_GHOST)).min_size(egui::vec2(14.0, 14.0))).clicked() { watchlist.chain_0_offset -= 1; }
                            }
                        });
                        let ns_0 = watchlist.chain_0_num_strikes;
                        let off_0 = watchlist.chain_0_offset;
                        let sm_0 = watchlist.chain_0_strike_mode;
                        let nmf_0 = watchlist.chain_0_nmf;
                        render_block(ui, 0, &calls_0, &puts_0, &sym, chain_price, &mut watchlist.saved_options, sel, scroll_w, ns_0, off_0, sm_0, nmf_0);

                        ui.add_space(6.0);
                        let sep_r = ui.available_rect_before_wrap();
                        ui.painter().line_segment(
                            [egui::pos2(sep_r.left() + 4.0, ui.cursor().min.y), egui::pos2(sep_r.right() - 4.0, ui.cursor().min.y)],
                            egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_LINE)));
                        ui.add_space(4.0);

                        // Per-chain controls: far DTE
                        ui.horizontal(|ui| {
                            dim_label(ui, &format!("{}DTE", far_dte), t.dim);
                            let mode_label = match watchlist.chain_far_strike_mode {
                                StrikeMode::Count => "Cnt".into(),
                                StrikeMode::Pct(i) => format!("{}%", PCT_OPTIONS.get(i as usize).unwrap_or(&1.0)),
                                StrikeMode::StdDev => "σ".into(),
                            };
                            egui::ComboBox::from_id_salt("sm_f").selected_text(egui::RichText::new(&mode_label).monospace().size(8.0)).width(40.0).show_ui(ui, |ui| {
                                if ui.selectable_label(matches!(watchlist.chain_far_strike_mode, StrikeMode::Count), "Count").clicked() { watchlist.chain_far_strike_mode = StrikeMode::Count; }
                                for (pi, &pct) in PCT_OPTIONS.iter().enumerate() {
                                    if ui.selectable_label(watchlist.chain_far_strike_mode == StrikeMode::Pct(pi as u8), format!("{}%", pct)).clicked() { watchlist.chain_far_strike_mode = StrikeMode::Pct(pi as u8); }
                                }
                                if ui.selectable_label(matches!(watchlist.chain_far_strike_mode, StrikeMode::StdDev), "Std Dev").clicked() { watchlist.chain_far_strike_mode = StrikeMode::StdDev; }
                            });
                            if ui.add(egui::Button::new(egui::RichText::new("-").monospace().size(9.0)).min_size(egui::vec2(14.0, 14.0))).clicked() { watchlist.chain_far_num_strikes = watchlist.chain_far_num_strikes.saturating_sub(1).max(1); }
                            ui.label(egui::RichText::new(format!("{}", watchlist.chain_far_num_strikes)).monospace().size(8.0).color(t.dim));
                            if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(9.0)).min_size(egui::vec2(14.0, 14.0))).clicked() { watchlist.chain_far_num_strikes += 1; }
                            for (lvl, label) in [(0u8, "N"), (1, "M"), (2, "F")] {
                                let active = watchlist.chain_far_nmf == lvl;
                                let col = if active { t.accent } else { t.dim.gamma_multiply(0.4) };
                                if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(8.0).color(col))
                                    .fill(if active { color_alpha(t.accent, ALPHA_SUBTLE) } else { egui::Color32::TRANSPARENT })
                                    .min_size(egui::vec2(14.0, 14.0)).corner_radius(RADIUS_SM)).clicked() { watchlist.chain_far_nmf = lvl; }
                            }
                            let fr_col = if watchlist.chain_far_frozen { t.accent } else { t.dim.gamma_multiply(0.4) };
                            if ui.add(egui::Button::new(egui::RichText::new(if watchlist.chain_far_frozen { Icon::PAUSE } else { Icon::PLAY }).size(9.0).color(fr_col)).fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(14.0, 14.0))).clicked() {
                                watchlist.chain_far_frozen = !watchlist.chain_far_frozen;
                                if !watchlist.chain_far_frozen { watchlist.chain_far_offset = 0; }
                            }
                            if watchlist.chain_far_frozen {
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::ARROW_FAT_UP).size(9.0).color(t.dim)).fill(color_alpha(t.toolbar_border, ALPHA_GHOST)).min_size(egui::vec2(14.0, 14.0))).clicked() { watchlist.chain_far_offset += 1; }
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::ARROW_FAT_DOWN).size(9.0).color(t.dim)).fill(color_alpha(t.toolbar_border, ALPHA_GHOST)).min_size(egui::vec2(14.0, 14.0))).clicked() { watchlist.chain_far_offset -= 1; }
                            }
                        });
                        let ns_f = watchlist.chain_far_num_strikes;
                        let off_f = watchlist.chain_far_offset;
                        let sm_f = watchlist.chain_far_strike_mode;
                        let nmf_f = watchlist.chain_far_nmf;
                        render_block(ui, far_dte, &calls_f, &puts_f, &sym, chain_price, &mut watchlist.saved_options, sel, scroll_w, ns_f, off_f, sm_f, nmf_f);
                    });
                    // Normal click: just open option chart (no watchlist add)
                    if let Some(info) = clicked_contract.take() {
                        open_option_chart = Some(info);
                    }
                    // Select mode / shift+click: add to watchlist + persist
                    if let Some((ref sym, strike, is_call, ref expiry, bid, ask)) = watchlist_add.take() {
                        watchlist.add_option_to_watchlist(sym, strike, is_call, expiry, bid, ask);
                        watchlist.persist();
                    }
                }

                // ── HEAT TAB ─────────────────────────────────────────────────
                WatchlistTab::Heat => {
                    // Index preset dropdown + expand/collapse
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("heat_idx")
                            .selected_text(egui::RichText::new(&watchlist.heat_index).monospace().size(10.0))
                            .width(100.0)
                            .show_ui(ui, |ui| {
                                for idx in &["Watchlist", "S&P 500", "Dow 30", "Nasdaq 100"] {
                                    if ui.selectable_label(watchlist.heat_index == *idx, *idx).clicked() {
                                        watchlist.heat_index = idx.to_string();
                                        watchlist.heat_collapsed.clear();
                                    }
                                }
                            });
                        // Expand / Collapse / Columns / Sort — all with hover cursor
                        let hbtn = |ui: &mut egui::Ui, label: &str, col: egui::Color32, tip: &str| -> bool {
                            let resp = ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(10.0).color(col))
                                .min_size(egui::vec2(20.0, 18.0)).corner_radius(RADIUS_MD));
                            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                            resp.on_hover_text(tip).clicked()
                        };
                        if hbtn(ui, Icon::PLUS, t.dim, "Expand all") { watchlist.heat_collapsed.clear(); }
                        if hbtn(ui, Icon::MINUS, t.dim, "Collapse all") { watchlist.heat_collapsed.insert("__collapse_all__".into()); }
                        let col_label = format!("{}c", watchlist.heat_cols);
                        if hbtn(ui, &col_label, t.dim, "Toggle 1/2/3 columns") { watchlist.heat_cols = match watchlist.heat_cols { 1 => 2, 2 => 3, _ => 1 }; }
                        let sort_label = match watchlist.heat_sort { 1 => Icon::ARROW_FAT_UP, -1 => Icon::ARROW_FAT_DOWN, _ => Icon::DOTS_THREE };
                        let sort_col = if watchlist.heat_sort != 0 { t.accent } else { t.dim };
                        if hbtn(ui, sort_label, sort_col, "Sort: gainers / losers / default") { watchlist.heat_sort = match watchlist.heat_sort { 0 => 1, 1 => -1, _ => 0 }; }
                    });
                    ui.add_space(2.0);

                    // Sector ETF mapping for S&P
                    // TODO: Fetch constituent lists from ApexIB API for live, up-to-date data.
                    // These are static approximations of index holdings as of early 2025.
                    let sp500_sectors: &[(&str, &[&str])] = &[
                        ("XLK Technology", &["AAPL","MSFT","NVDA","AVGO","CRM","ADBE","AMD","INTC","CSCO","ORCL","ACN","IBM","NOW","QCOM","TXN","AMAT","INTU","ADI","LRCX","MU","SNPS","CDNS","KLAC","MCHP","FTNT","MSI","ANSS","NXPI","KEYS","GEN"]),
                        ("XLF Financials", &["BRK.B","JPM","V","MA","BAC","WFC","GS","MS","AXP","BLK","SCHW","SPGI","C","CB","MMC","PGR","ICE","CME","AON","MET","AIG","TFC","USB","PNC","MCO","MSCI","AJG","AFL","FIS","TROW"]),
                        ("XLV Healthcare", &["UNH","JNJ","LLY","PFE","ABT","TMO","MRK","ABBV","DHR","BMY","AMGN","MDT","ELV","CI","ISRG","SYK","GILD","VRTX","REGN","ZTS","BDX","BSX","HCA","IDXX","IQV","EW","A","DXCM","MTD","ALGN"]),
                        ("XLY Consumer Disc.", &["AMZN","TSLA","HD","MCD","NKE","SBUX","LOW","TJX","BKNG","CMG","ORLY","AZO","ROST","MAR","HLT","DHI","LEN","GM","F","EBAY","POOL","ULTA","GPC","DRI","BBY","MGM","WYNN","LVS","YUM","DPZ"]),
                        ("XLC Communication", &["META","GOOGL","GOOG","DIS","NFLX","CMCSA","T","VZ","TMUS","CHTR","EA","TTWO","WBD","PARA","OMC","IPG","MTCH","LYV","FOXA","FOX","NWSA","NWS","LUMN","DISH"]),
                        ("XLI Industrials", &["GE","CAT","UNP","HON","UPS","RTX","BA","LMT","DE","MMM","ETN","ITW","EMR","WM","RSG","CSX","NSC","FDX","GD","NOC","TDG","CARR","OTIS","JCI","PCAR","CTAS","ROK","FAST","GWW","IR"]),
                        ("XLE Energy", &["XOM","CVX","COP","SLB","EOG","MPC","PSX","VLO","OXY","HES","WMB","KMI","DVN","HAL","FANG","BKR","TRGP","MRO","APA","CTRA","OVV","EQT","MTDR","PR","DINO"]),
                        ("XLP Consumer Staples", &["PG","KO","PEP","COST","WMT","PM","MO","CL","MDLZ","KHC","GIS","SYY","STZ","KMB","HSY","K","MKC","TSN","HRL","CAG","SJM","CLX","CHD","TAP","CPB","BG","ADM","EL","KDP","MNST"]),
                        ("XLU Utilities", &["NEE","DUK","SO","D","AEP","SRE","EXC","XEL","ED","WEC","ES","AWK","DTE","CMS","FE","AES","ATO","NI","PNW","LNT","EVRG","CNP","PPL","NRG","CEG"]),
                        ("XLRE Real Estate", &["PLD","AMT","CCI","EQIX","PSA","SPG","O","WELL","DLR","AVB","EQR","VTR","ARE","MAA","UDR","PEAK","ESS","CPT","REG","HST","KIM","BXP","SLG","VNO","CBRE","IRM","WY","INVH","SUI","ELS"]),
                        ("XLB Materials", &["LIN","APD","SHW","ECL","FCX","NEM","NUE","DOW","DD","VMC","MLM","PPG","CE","CF","IFF","ALB","BALL","PKG","IP","EMN","AVY","FMC","MOS","SEE","WRK"]),
                    ];
                    let dow30: &[&str] = &["AAPL","AMGN","AXP","BA","CAT","CRM","CSCO","CVX","DIS","DOW","GS","HD","HON","IBM","INTC","JNJ","JPM","KO","MCD","MMM","MRK","MSFT","NKE","PG","TRV","UNH","V","VZ","WBA","WMT"];
                    let qqq100: &[&str] = &[
                        "AAPL","ABNB","ADBE","ADI","ADP","ADSK","AEP","AMAT","AMGN","AMZN",
                        "ANSS","ARM","ASML","AVGO","AZN","BIIB","BKNG","BKR","CDNS","CDW",
                        "CEG","CHTR","CMCSA","COST","CPRT","CRWD","CSCO","CSGP","CSX","CTAS",
                        "CTSH","DASH","DDOG","DLTR","DXCM","EA","EXC","FANG","FAST","FTNT",
                        "GEHC","GFS","GILD","GOOG","GOOGL","HON","IDXX","ILMN","INTC","INTU",
                        "ISRG","KDP","KHC","KLAC","LRCX","LULU","MAR","MCHP","MDB","MDLZ",
                        "MELI","META","MNST","MRNA","MRVL","MSFT","MU","NFLX","NVDA","NXPI",
                        "ODFL","ON","ORLY","PANW","PAYX","PCAR","PDD","PEP","PYPL","QCOM",
                        "REGN","RIVN","ROST","SBUX","SNPS","SPLK","TEAM","TMUS","TSLA","TTD",
                        "TTWO","TXN","VRSK","VRTX","WBA","WBD","WDAY","XEL","ZM","ZS",
                    ];

                    // Pre-build price lookup from watchlist
                    type HeatItem = (String, f32, String); // (symbol, change%, sector)
                    let price_map: std::collections::HashMap<String, f32> = watchlist.sections.iter()
                        .flat_map(|sec| sec.items.iter())
                        .filter(|i| i.price > 0.0 && i.prev_close > 0.0)
                        .map(|i| (i.symbol.clone(), (i.price / i.prev_close - 1.0) * 100.0))
                        .collect();
                    let lookup = |s: &str| -> f32 { price_map.get(s).copied().unwrap_or(0.0) };

                    let heat_items: Vec<HeatItem> = if watchlist.heat_index == "S&P 500" {
                        sp500_sectors.iter().flat_map(|(sector, syms)| {
                            syms.iter().map(|s| (s.to_string(), lookup(s), sector.to_string())).collect::<Vec<_>>()
                        }).collect()
                    } else if watchlist.heat_index == "Dow 30" {
                        dow30.iter().map(|s| (s.to_string(), lookup(s), "Dow".into())).collect()
                    } else if watchlist.heat_index == "Nasdaq 100" {
                        qqq100.iter().map(|s| (s.to_string(), lookup(s), "QQQ".into())).collect()
                    } else {
                        watchlist.sections.iter().flat_map(|sec| sec.items.iter())
                            .filter(|i| !i.is_option && i.loaded && i.price > 0.0)
                            .map(|i| {
                                let chg = if i.prev_close > 0.0 { (i.price / i.prev_close - 1.0) * 100.0 } else { 0.0 };
                                (i.symbol.clone(), chg, "Watchlist".into())
                            }).collect()
                    };

                    if heat_items.is_empty() {
                        ui.add_space(24.0);
                        ui.label(egui::RichText::new("No data — add symbols to watchlist").monospace().size(10.0).color(t.dim));
                    } else {
                        let mut heat_click_sym_outer: Option<String> = None;
                        egui::ScrollArea::vertical().show(ui, |ui| {

                            // Group by sector and render with dividers
                            let mut current_sector = String::new();
                            let mut tile_idx = 0;
                            let mut sector_items: Vec<&HeatItem> = vec![];

                            // Configurable N-column layout with click-to-chart
                            let num_cols = watchlist.heat_cols.max(1) as usize;
                            let heat_sort = watchlist.heat_sort;
                            let active_sym = panes[ap].symbol.clone();
                            let render_sector_items = |ui: &mut egui::Ui, items_unsorted: &[&HeatItem], t: &Theme, _pm: &std::collections::HashMap<String, f32>, num_cols: usize, sort: i8, click_sym: &mut Option<String>, active_sym: &str| {
                                let mut items: Vec<&HeatItem> = items_unsorted.to_vec();
                                if sort == 1 { items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)); }
                                else if sort == -1 { items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)); }
                                let avail_w = ui.available_width();
                                let gap = 3.0;
                                let col_w = (avail_w - gap * (num_cols - 1) as f32) / num_cols as f32;
                                let cell_h = if num_cols == 1 { 26.0 } else { 28.0 };
                                let font_sz = if num_cols >= 3 { 10.0 } else { 12.0 };
                                let max_pct = items.iter().map(|i| i.1.abs()).fold(1.0_f32, f32::max);
                                let rows = (items.len() + num_cols - 1) / num_cols;
                                let total_h = rows as f32 * cell_h;
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(avail_w, total_h), egui::Sense::click());
                                let painter = ui.painter();
                                // Hover detection — find which cell the mouse is over
                                let hover_idx: Option<usize> = ui.input(|i| i.pointer.hover_pos()).and_then(|pos| {
                                    if !rect.contains(pos) { return None; }
                                    let col = ((pos.x - rect.left()) / (col_w + gap)).floor() as usize;
                                    let row = ((pos.y - rect.top()) / cell_h).floor() as usize;
                                    let idx = row * num_cols + col;
                                    if idx < items.len() { Some(idx) } else { None }
                                });
                                if hover_idx.is_some() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                                // Click detection
                                if resp.clicked() {
                                    if let Some(pos) = resp.interact_pointer_pos() {
                                        let col = ((pos.x - rect.left()) / (col_w + gap)).floor() as usize;
                                        let row = ((pos.y - rect.top()) / cell_h).floor() as usize;
                                        let idx = row * num_cols + col;
                                        if let Some(item) = items.get(idx) { *click_sym = Some(item.0.clone()); }
                                    }
                                }
                                for (i, item) in items.iter().enumerate() {
                                    let col = i % num_cols;
                                    let row = i / num_cols;
                                    let cx = rect.left() + col as f32 * (col_w + gap);
                                    let cy = rect.top() + row as f32 * cell_h;
                                    let intensity = (item.1.abs() / 5.0).min(1.0);
                                    let is_up = item.1 >= 0.0;
                                    let is_active = item.0 == active_sym;
                                    let is_hovered = hover_idx == Some(i);
                                    // Hover highlight
                                    if is_hovered {
                                        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(cx, cy), egui::vec2(col_w, cell_h)),
                                            2.0, color_alpha(t.text,12));
                                    }
                                    // Active symbol border
                                    if is_active {
                                        painter.rect_stroke(egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(col_w, cell_h - 2.0)),
                                            2.0, egui::Stroke::new(STROKE_BOLD, t.accent), egui::StrokeKind::Outside);
                                    }
                                    // Background bar
                                    let bar_frac = if max_pct > 0.0 { item.1.abs() / max_pct } else { 0.0 };
                                    let bar_w = bar_frac * col_w * 0.6;
                                    let bar_col = if is_up {
                                        egui::Color32::from_rgba_unmultiplied(46, 204, 113, (25.0 + intensity * 55.0) as u8)
                                    } else {
                                        egui::Color32::from_rgba_unmultiplied(231, 76, 60, (25.0 + intensity * 55.0) as u8)
                                    };
                                    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(bar_w, cell_h - 2.0)), 2.0, bar_col);
                                    // Edge strip
                                    let edge_a = (120.0 + intensity * 135.0) as u8;
                                    let edge_col = if is_up { egui::Color32::from_rgba_unmultiplied(46, 204, 113, edge_a) } else { egui::Color32::from_rgba_unmultiplied(231, 76, 60, edge_a) };
                                    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(3.0, cell_h - 2.0)), 0.0, edge_col);
                                    // Symbol (bright white for active, slightly dimmer for others)
                                    let sym_col = if is_active { egui::Color32::WHITE } else if is_hovered { color_alpha(t.text,230) } else { color_alpha(t.text,190) };
                                    painter.text(egui::pos2(cx + 7.0, cy + cell_h / 2.0), egui::Align2::LEFT_CENTER,
                                        &item.0, egui::FontId::monospace(font_sz), sym_col);
                                    // Change%
                                    let chg_col = if is_up { t.bull } else { t.bear };
                                    painter.text(egui::pos2(cx + col_w - 3.0, cy + cell_h / 2.0), egui::Align2::RIGHT_CENTER,
                                        &format!("{:+.1}%", item.1), egui::FontId::monospace(font_sz), chg_col);
                                }
                            };

                            // Render grouped by sector
                            let mut groups: Vec<(String, Vec<&HeatItem>)> = vec![];
                            for item in &heat_items {
                                if groups.last().map_or(true, |(s, _)| *s != item.2) {
                                    groups.push((item.2.clone(), vec![]));
                                }
                                groups.last_mut().unwrap().1.push(item);
                            }
                            // Handle collapse-all
                            if watchlist.heat_collapsed.contains("__collapse_all__") {
                                watchlist.heat_collapsed.remove("__collapse_all__");
                                for (s, _) in &groups { watchlist.heat_collapsed.insert(s.clone()); }
                            }
                            for (sector, items) in &groups {
                                let is_collapsed = watchlist.heat_collapsed.contains(sector);
                                // Sector avg change
                                let avg_chg: f32 = if items.is_empty() { 0.0 } else {
                                    items.iter().map(|i| i.1).sum::<f32>() / items.len() as f32
                                };
                                let sector_col = if avg_chg >= 0.0 { t.bull } else { t.bear };

                                if groups.len() > 1 {
                                    ui.add_space(3.0);
                                    // Colored sector header — single clickable button
                                    let caret = if is_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                                    let header_text = format!("{} {}  ({})  {:+.2}%", caret, sector, items.len(), avg_chg);
                                    let header_btn = ui.add(egui::Button::new(
                                        egui::RichText::new(&header_text).monospace().size(11.0).color(sector_col)
                                    ).fill(color_alpha(sector_col, ALPHA_FAINT)).corner_radius(RADIUS_MD).min_size(egui::vec2(ui.available_width(), 22.0)));
                                    if header_btn.clicked() {
                                        if is_collapsed { watchlist.heat_collapsed.remove(sector); }
                                        else { watchlist.heat_collapsed.insert(sector.clone()); }
                                    }
                                    ui.add_space(1.0);
                                }
                                if !is_collapsed {
                                    render_sector_items(ui, items, t, &price_map, num_cols, heat_sort, &mut heat_click_sym_outer, &active_sym);
                                }
                            }
                        });
                        // Handle click-to-chart
                        if let Some(sym) = heat_click_sym_outer {
                            panes[ap].pending_symbol_change = Some(sym);
                        }
                    }
                }

            }

            // ── Handle option chart opening (from any tab) ──
            // Delegate to deferred handler which always replaces active pane
            if let Some(info) = open_option_chart {
                watchlist.pending_opt_chart = Some(info);
            }
        });
}


}
