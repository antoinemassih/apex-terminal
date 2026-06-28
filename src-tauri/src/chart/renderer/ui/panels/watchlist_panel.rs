//! Watchlist side panel — stocks list, options chain, heatmap.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::widgets::rows::{
    WatchlistRow, WatchlistIconSet, WatchlistPinState,
};
use super::super::lists::rows::watchlist_columns::{BUILTIN as WL_COLUMNS_BUILTIN};
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::gpu::{fetch_chain_background, fetch_search_background, fetch_watchlist_prices, set_pending_wl_tooltip, WlTooltipData};
use crate::chart_renderer::trading::market_session;
use super::super::components::text::MonospaceCode;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant, Size};
use crate::ui_kit::widgets::SearchInput;
use super::super::components::frames_widget::PopupFrame;
use super::super::widgets::watchlist::NmfToggle;
use crate::ui_kit::widgets::{Input, MenuItem, PanelEmpty, PanelLoading, PanelSection, Tag, TagTone, Tooltip};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use crate::ui_kit::widgets::tokens::Size as KitSize;
use crate::ui_kit::widgets::icon_placement::IconPlacement;

/// Map between `WatchlistTab` and the rail's instance-tab `u8` (for duplicates).
fn wl_tab_to_u8(t: WatchlistTab) -> u8 {
    match t { WatchlistTab::Stocks => 0, WatchlistTab::Chain => 1, WatchlistTab::Heat => 2, WatchlistTab::Scan => 3 }
}
fn wl_tab_from_u8(v: u8) -> WatchlistTab {
    match v { 1 => WatchlistTab::Chain, 2 => WatchlistTab::Heat, 3 => WatchlistTab::Scan, _ => WatchlistTab::Stocks }
}

/// Rail registration — the watchlist's entry in the [`super::right_rail`] registry.
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "watchlist",
    is_open: |w| w.open,
    render: |cx, slot| { draw(cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot), None); },
};

/// Draw the watchlist. `instance_tab` = `Some` when rendered as a *duplicate
/// instance* in the rail (its tab lives in the rail's spawn store, independent
/// of the base panel's `watchlist.tab`); returns `true` then if the instance's
/// close-X was clicked (the rail removes it). `None` = the base panel.
pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme, slot: Option<super::side_panel_shell::RailSlot>, instance_tab: Option<&mut u8>) -> bool {
    let _z_watchlist = crate::foundation::frame_profiler::profile_zone("watchlist_panel");
    let is_spawn = instance_tab.is_some();
    let mut spawn_close = false;
// ── Watchlist side panel ───────────────────────────────────────────────────
if is_spawn || watchlist.open {
    // Active tab: from the instance store (duplicate) or the base `watchlist.tab`.
    // Snapshotted so SidePanelShell::tabs can take a &mut without conflicting
    // with `&mut watchlist` inside the body closure (orders_panel pattern).
    let mut active_tab = match instance_tab.as_deref() {
        Some(v) => wl_tab_from_u8(*v),
        None => watchlist.tab,
    };
    // Pre-resolve pane-aligned metrics outside the mutable borrow window so the
    // body can borrow &mut watchlist freely.
    let header_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let title_font_size = watchlist.pane_header_size.title_font();
    let tabs = [
        (WatchlistTab::Stocks, "LIST", None),
        (WatchlistTab::Chain,  "CHAIN", None),
        (WatchlistTab::Heat,   "HEAT", None),
        (WatchlistTab::Scan,   "SCAN", None),
    ];
    let shell_id = if is_spawn { "watchlist_inst" } else { "watchlist" };
    let shell_resp = SidePanelShell::tabs(shell_id, &mut active_tab, &tabs)
        .width(Width::Narrow)
        .resizable(180.0..=480.0)
        .pane_metrics(header_h, title_font_size)
        .rail_slot(slot)
        .on_tab_secondary(|ui, tab| {
            // Right-click a tab → spawn a duplicate watchlist instance on that tab.
            if crate::ui_kit::widgets::MenuItem::new("Open as new instance").show(ui, t).clicked() {
                super::right_rail::request_spawn("watchlist", wl_tab_to_u8(tab));
                ui.close_menu();
            }
        })
        .show(ctx, t, |ui, t, tab| {
            let mut wl_switch_to: Option<usize> = None;
            let mut wl_fetch_syms: Vec<String> = Vec::new();
            let mut wl_rename_idx: Option<usize> = None;
            let mut wl_delete_idx: Option<usize> = None;
            let mut wl_dup_idx: Option<usize> = None;

            let mut open_option_chart: Option<(String, f32, bool, String)> = None;
            // OCC ticker for the click → routed into pending_opt_chart_contract
            // so the gpu.rs consumer uses the real contract instead of falling
            // back on synthesize_occ (which is wrong for non-Friday expiries).
            let mut clicked_occ_ticker: Option<String> = None;

            match tab {
                // ── STOCKS TAB (LIST) ──────────────────────────────────────────
                WatchlistTab::Stocks => {
                    // ── B) Watchlist selector + options toggle ──
                    ui.horizontal(|ui| {
                        ui.set_min_height(20.0);
                        // Inline rename mode
                        if watchlist.watchlist_name_editing {
                            let resp = Input::new(&mut watchlist.watchlist_name_buf)
                                .width(ui.available_width() - 50.0)
                                .font_size(10.0)
                                .show(ui, t);
                            if resp.lost_focus || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let new_name = watchlist.watchlist_name_buf.trim().to_string();
                                if !new_name.is_empty() {
                                    if let Some(wl) = watchlist.saved_watchlists.get_mut(watchlist.active_watchlist_idx) {
                                        wl.name = new_name;
                                    }
                                }
                                watchlist.watchlist_name_editing = false;
                                watchlist.persist();
                            } else {
                                resp.request_focus(ui.ctx());
                            }
                        } else {
                            // Snapshot names and count for the dropdown to avoid borrow conflicts
                            let wl_names: Vec<String> = watchlist.saved_watchlists.iter().map(|w| w.name.clone()).collect();
                            let wl_count = wl_names.len();
                            let active_idx = watchlist.active_watchlist_idx;
                            let active_name = wl_names.get(active_idx).cloned().unwrap_or_else(|| "Default".into());
                            let wl_opts: Vec<(usize, String)> = wl_names.iter().enumerate()
                                .map(|(i, n)| (i, n.clone())).collect();
                            let (_, combo_resp) = super::super::inputs::select::DropdownOwned::new("wl_selector")
                                .options(wl_opts)
                                .width(ui.available_width() - 60.0)
                                .font_size(9.0)
                                .item_context_menu(|idx, ui| {
                                    let i = *idx;
                                    if MenuItem::new("Rename").show(ui, t).clicked() {
                                        wl_rename_idx = Some(i);
                                        ui.close_menu();
                                    }
                                    if MenuItem::new("Duplicate").show(ui, t).clicked() {
                                        wl_dup_idx = Some(i);
                                        ui.close_menu();
                                    }
                                    if wl_count > 1 {
                                        ui.separator();
                                        if MenuItem::new("Delete").tint(t.bear).show(ui, t).clicked() {
                                            wl_delete_idx = Some(i);
                                            ui.close_menu();
                                        }
                                    }
                                })
                                .theme(t)
                                .show_resp(ui, &mut watchlist.active_watchlist_idx);
                            if watchlist.active_watchlist_idx != active_idx {
                                wl_switch_to = Some(watchlist.active_watchlist_idx);
                                // Restore — actual switch is handled by wl_switch_to below
                                watchlist.active_watchlist_idx = active_idx;
                            }
                            // Right-click the combo header for rename/dup/delete of active
                            combo_resp.context_menu(|ui| {
                                if MenuItem::new("Rename").show(ui, t).clicked() {
                                    wl_rename_idx = Some(active_idx);
                                    ui.close_menu();
                                }
                                if MenuItem::new("Duplicate").show(ui, t).clicked() {
                                    wl_dup_idx = Some(active_idx);
                                    ui.close_menu();
                                }
                                if wl_count > 1 {
                                    ui.separator();
                                    if MenuItem::new("Delete").tint(t.bear).show(ui, t).clicked() {
                                        wl_delete_idx = Some(active_idx);
                                        ui.close_menu();
                                    }
                                }
                            });
                            // "+" button to create new watchlist
                            let r = ui.add(Button::icon(Icon::PLUS).variant(Variant::TextOnly).glyph_color(t.dim).size(Size::Md).placement(IconPlacement::PanelHeader));
                            Tooltip::new("New watchlist").show(ui, &r, t);
                            if r.clicked() {
                                let n = watchlist.saved_watchlists.len() + 1;
                                let syms = watchlist.create_watchlist(&format!("Watchlist {}", n));
                                if !syms.is_empty() { wl_fetch_syms = syms; }
                            }
                        }
                        // Right-anchored cluster: market-session badge (OPEN/PRE/POST/CLOSED)
                        // + options-visibility toggle (circle icon). The session badge lives
                        // here — beside the watchlist selector — rather than in the header,
                        // so the chart-pane-aligned header stays uncluttered.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let opt_icon = if watchlist.options_visible { Icon::RADIO_BUTTON } else { Icon::DOT };
                            let opt_resp = ui.add(Button::icon(opt_icon).variant(Variant::MutedIcon).active(watchlist.options_visible).size(Size::Sm)
                                .placement(IconPlacement::PanelHeader));
                            Tooltip::new("Show / hide options").show(ui, &opt_resp, t);
                            if opt_resp.clicked() { watchlist.options_visible = !watchlist.options_visible; }
                            if opt_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

                            ui.add_space(gap_xs());
                            let (session, session_col) = market_session();
                            let badge_bg = color_alpha(session_col, alpha_tint());
                            ui.add(Button::new(session)
                                .variant(Variant::Chrome)
                                .size(Size::Xs)
                                .fg(session_col)
                                .fill(badge_bg)
                                .corner_radius(current().r_sm as f32)
                                .stroke(egui::Stroke::NONE)
                                .min_size(egui::vec2(34.0, 14.0)));
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
                    ui.add_space(gap_xs());

                    // ── C) Search field + filter button beside it ──
                    // Use allocate_ui_with_layout to place them side by side without
                    // ui.horizontal() which reports combined min-width and forces expansion.
                    let avail = ui.available_width();
                    let btn_w = 22.0;
                    let search_w = (avail - btn_w - 4.0).max(40.0);
                    let search_h = 20.0;
                    let (full_rect, _) = ui.allocate_exact_size(egui::vec2(avail, search_h), egui::Sense::hover());
                    // Search field (left portion)
                    let search_rect = egui::Rect::from_min_size(full_rect.min, egui::vec2(search_w, search_h));
                    let search_resp = ui.allocate_ui_at_rect(search_rect, |ui| {
                        SearchInput::new(&mut watchlist.search_query)
                            .placeholder("Add symbol...")
                            .width(search_w)
                            .size(crate::ui_kit::widgets::tokens::Size::Sm)
                            .show(ui, t)
                    }).inner;
                    // Filter button (right portion)
                    let filter_active = watchlist.filter_preset != "All" || !watchlist.filter_text.is_empty();
                    let icon_col = if filter_active { t.accent } else if watchlist.filter_open { t.accent } else { color_dim(t.dim) };
                    let btn_rect = egui::Rect::from_min_size(egui::pos2(full_rect.right() - btn_w, full_rect.top()), egui::vec2(btn_w, search_h));
                    let filter_btn_rect = btn_rect; // capture for popup anchor
                    ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER, Icon::FUNNEL, egui::FontId::proportional(font_sm()), icon_col);
                    let btn_resp = ui.interact(btn_rect, egui::Id::new("wl_filter_btn"), egui::Sense::click());
                    cursor::focus_ring(ui, &btn_resp, t.accent);
                    if btn_resp.clicked() { watchlist.update_sidebar_state(|s| s.filter_open = !s.filter_open); }
                    crate::chart_renderer::ui::style::cursor::clickable(ui, &btn_resp);
                    // Columns config button (sliders icon)
                    let col_btn_rect = egui::Rect::from_min_size(egui::pos2(btn_rect.left() - btn_w, full_rect.top()), egui::vec2(btn_w, search_h));
                    let col_icon_col = if watchlist.wl_columns_open { t.accent } else { color_dim(t.dim) };
                    ui.painter().text(col_btn_rect.center(), egui::Align2::CENTER_CENTER, Icon::SLIDERS, egui::FontId::proportional(font_sm()), col_icon_col);
                    let col_resp = ui.interact(col_btn_rect, egui::Id::new("wl_columns_btn"), egui::Sense::click());
                    cursor::focus_ring(ui, &col_resp, t.accent);
                    if col_resp.clicked() { watchlist.update_sidebar_state(|s| s.wl_columns_open = !s.wl_columns_open); }
                    crate::chart_renderer::ui::style::cursor::clickable(ui, &col_resp);
                    // Refocus after adding a symbol
                    if watchlist.search_refocus {
                        watchlist.search_refocus = false;
                        search_resp.response.request_focus();
                    }
                    if search_resp.response.changed() {
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
                    if has_results && search_resp.response.has_focus() {
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
                    if search_resp.response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        watchlist.search_query.clear();
                        watchlist.search_results.clear();
                        watchlist.search_sel = -1;
                    }
                    // Suggestion dropdown
                    if has_results {
                        PopupFrame::new().colors(t.toolbar_bg, t.toolbar_border).ctx(ctx).build().show(ui, |ui| {
                            for (i, (sym, name)) in watchlist.search_results.clone().iter().enumerate() {
                                let is_sel = i as i32 == watchlist.search_sel;
                                let bg = if is_sel { tint(t, Tone::Accent, alpha_tint()) } else { egui::Color32::TRANSPARENT };
                                let fg = if is_sel { t.text } else { t.dim };
                                let lbl = format!("{:6} {}", sym, name);
                                // legacy: monospace RichText; Button uses plain text
                                let resp = ui.add(Button::new(lbl.as_str()).variant(Variant::Ghost).size(Size::Sm)
                                    .min_size(egui::vec2(ui.available_width(), row_height_compact())));
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
                                    // Only let hover claim the selection when the mouse is
                                    // actually moving — otherwise a stationary cursor sitting
                                    // over the list overwrites keyboard (arrow) navigation
                                    // every frame, so Up/Down never appears to move.
                                    if ui.input(|inp| inp.pointer.is_moving()) {
                                        watchlist.search_sel = i as i32;
                                    }
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                            }
                        });
                    }
                    ui.add_space(gap_sm());

                    // Filter indicator (show active preset name if filtering)
                    if watchlist.filter_preset != "All" || !watchlist.filter_text.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(gap_sm());
                            ui.add(MonospaceCode::new(&format!("{} {}", Icon::FUNNEL, watchlist.filter_preset)).size_px(font_xs()).color(t.accent));
                        });
                    }
                    // Column config popup
                    if watchlist.wl_columns_open {
                        ui.add_space(gap_xs());
                        crate::ui_kit::widgets::OutlinedBox::new()
                            .fill(tint(t, Tone::Border, alpha_faint()))
                            .borderless()
                            .radius_sm()
                            .padding(gap_sm())
                            .show(ui, t, |ui| {
                                ui.add(MonospaceCode::new("COLUMNS").size_px(font_2xs()).color(t.accent).gamma(0.6));
                                ui.add_space(gap_xs());
                                for s in WL_COLUMNS_BUILTIN.iter() {
                                    let visible = watchlist.wl_columns.contains(&s.id);
                                    ui.horizontal(|ui| {
                                        let icon = if visible { Icon::EYE } else { Icon::EYE_SLASH };
                                        let r = ui.add(Button::icon(icon).variant(Variant::MutedIcon).active(visible).size(Size::Sm)
                                            .placement(IconPlacement::PanelHeader));
                                        Tooltip::new("Show / hide column").show(ui, &r, t);
                                        if r.clicked() {
                                            if visible {
                                                watchlist.wl_columns.retain(|c| *c != s.id);
                                            } else {
                                                // Append, preserving order of remaining columns.
                                                watchlist.wl_columns.push(s.id);
                                            }
                                        }
                                        let lbl_col = if visible { t.text } else { color_dim(t.dim) };
                                        ui.add(MonospaceCode::new(s.label).size_px(font_xs()).color(lbl_col));
                                    });
                                }
                            });
                        ui.add_space(gap_xs());
                    }

                    if watchlist.filter_open {
                        let popup_id = egui::Id::new("wl_filter_popup");
                        let popup_pos = egui::pos2(filter_btn_rect.left(), filter_btn_rect.bottom() + 2.0);
                        egui::Area::new(popup_id)
                            .fixed_pos(popup_pos)
                            .order(egui::Order::Foreground)
                            .show(ui.ctx(), |ui| {
                                crate::ui_kit::widgets::OutlinedBox::new()
                                    .fill(t.toolbar_bg)
                                    .border(tint(t, Tone::Border, alpha_strong()))
                                    .hairline()
                                    .radius_sm()
                                    .padding(gap_md())
                                    .show(ui, t, |ui| {
                                    ui.set_min_width(180.0);
                                    // Search
                                    ui.horizontal(|ui| {
                                        Input::new(&mut watchlist.filter_text)
                                            .placeholder("Search...")
                                            .min_width((180.0_f32 - 30.0).max(40.0))
                                            .size(KitSize::Xs)
                                            .show(ui, t);
                                        if !watchlist.filter_text.is_empty() {
                                            let r = ui.add(Button::icon(Icon::X).variant(Variant::TextOnly).glyph_color(t.dim).size(Size::Xs).placement(IconPlacement::ListRow));
                                            Tooltip::new("Clear filter").show(ui, &r, t);
                                            if r.clicked() {
                                                watchlist.filter_text.clear();
                                            }
                                        }
                                    });
                                    ui.add_space(gap_xs());
                                    // Preset pills
                                    let mut close_filter_on_preset = false;
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing.x = gap_xs();
                                        let presets: Vec<(&str, f32, f32)> = {
                                            let mut p = vec![
                                                ("All", -999.0_f32, 999.0_f32),
                                                ("+2%", 2.0, 999.0), ("-2%", -999.0, -2.0),
                                                ("+5%", 5.0, 999.0), ("-5%", -999.0, -5.0),
                                                ("Big", 3.0, 999.0),
                                            ];
                                            for cf in &watchlist.custom_filters { p.push((&cf.name, cf.min_change, cf.max_change)); }
                                            p
                                        };
                                        for (name, min_chg, max_chg) in &presets {
                                            let active = watchlist.filter_preset == *name;
                                            let tone = if active { TagTone::Accent } else { TagTone::Neutral };
                                            if Tag::new(*name).tone(tone).show(ui, t).response.clicked() {
                                                watchlist.filter_preset = name.to_string();
                                                watchlist.filter_min_change = *min_chg;
                                                watchlist.filter_max_change = *max_chg;
                                                close_filter_on_preset = true;
                                            }
                                        }
                                    });
                                    if close_filter_on_preset {
                                        watchlist.update_sidebar_state(|s| s.filter_open = false);
                                    }
                                });
                            });
                        // Close popup when clicking outside
                        if ui.ctx().input(|i| i.pointer.any_click()) {
                            let popup_area = egui::Rect::from_min_size(
                                popup_pos,
                                egui::vec2(200.0, 200.0), // approximate — close on any outside click
                            );
                            if !popup_area.contains(ui.ctx().input(|i| i.pointer.interact_pos().unwrap_or(egui::Pos2::ZERO))) {
                                watchlist.update_sidebar_state(|s| s.filter_open = false);
                            }
                        }
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

                        // Compute once per frame (not per row) — avoids a syscall × N rows.
                        let frame_now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64).unwrap_or(0);
                        // Pre-uppercase the filter text once per frame so the per-row
                        // filter check only needs to uppercase the symbol (not both).
                        let filter_text_upper = watchlist.filter_text.to_uppercase();
                        // Market session, read once per frame. Drives whether the
                        // Change % column shows today's live move (RTH) or the last
                        // completed session's close-to-close (closed / pre-open), and
                        // whether the Ext-Hours column is shown (pre/after-market).
                        let mkt = crate::apex_data::live_state::market_status();
                        let mkt_rth = mkt.as_ref().map(|m| m.is_rth()).unwrap_or(false);
                        let mkt_ext = mkt.as_ref().map(|m| m.early_hours || m.after_hours
                            || m.market.eq_ignore_ascii_case("extended-hours")).unwrap_or(false);

                        // Section color presets for the color picker
                        let color_presets = ["#4a9eff","#e74c3c","#2ecc71","#f39c12","#9b59b6","#1abc9c","#e67e22","#3498db","#e91e63","#00bcd4","#8bc34a","#ff5722","#607d8b","#795548","#cddc39","#ff9800"];


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
                            // Paint section background + inset bevel directly onto the
                            // upcoming cursor rect (without allocating — rows will allocate
                            // themselves via WatchlistRow::show).
                            let section_h = pinned_items.len() as f32 * 30.0 + 6.0;
                            let sec_top = ui.cursor().min;
                            let sec_rect = egui::Rect::from_min_size(sec_top, egui::vec2(full_w, section_h));
                            {
                                let p = ui.painter();
                                p.rect_filled(sec_rect, 0.0, shadow_color_alpha(t, alpha_line()));
                                p.line_segment([egui::pos2(sec_rect.left(), sec_rect.top()), egui::pos2(sec_rect.right(), sec_rect.top())],
                                    egui::Stroke::new(stroke_std(), shadow_color_alpha(t, alpha_dim())));
                                p.line_segment([egui::pos2(sec_rect.left(), sec_rect.top() + 1.0), egui::pos2(sec_rect.right(), sec_rect.top() + 1.0)],
                                    egui::Stroke::new(stroke_thin(), shadow_color_alpha(t, alpha_tint())));
                                p.line_segment([egui::pos2(sec_rect.left(), sec_rect.bottom() - 1.0), egui::pos2(sec_rect.right(), sec_rect.bottom() - 1.0)],
                                    egui::Stroke::new(stroke_std(), tint(t, Tone::Text, 10)));
                                p.line_segment([egui::pos2(sec_rect.left(), sec_rect.bottom()), egui::pos2(sec_rect.right(), sec_rect.bottom())],
                                    egui::Stroke::new(stroke_thin(), tint(t, Tone::Text, 5)));
                            }
                            // 3px top padding so rows sit at the same position as before.
                            ui.add_space(gap_xs());
                            // Render each pinned row via the design-system WatchlistRow widget.
                            for (si, ii, pin_sym, pin_price, pin_prev, _pin_loaded, avg_range) in &pinned_items {
                                let is_active = *pin_sym == active_sym;
                                let change_pct = if *pin_prev > 0.0 { (*pin_price / *pin_prev - 1.0) * 100.0 } else { 0.0 };
                                // Active row paints over an accent-tinted bg; use the theme's
                                // text color so light themes (Bauhaus / Peach / Ivory / Newsprint)
                                // don't get unreadable white-on-light foreground.
                                let sym_fg = if is_active { t.text } else { tint(t, Tone::Text, 230) };
                                let wresp = WatchlistRow::new(pin_sym, *pin_price, change_pct)
                                    .theme(t)
                                    .height(28.0)
                                    .font_size_override(14.0)
                                    .pin_state(WatchlistPinState::Pinned)
                                    .active(is_active)
                                    .extreme_move_tint(if *pin_prev > 0.0 { Some(*avg_range) } else { None })
                                    .fg(sym_fg)
                                    .icon_set(WatchlistIconSet {
                                        drag_handle: Icon::DOTS_SIX_VERTICAL,
                                        star: Icon::SPARKLE,
                                        x: Icon::X,
                                        alert: Icon::LIGHTNING,
                                    })
                                    .sym_layout(-6.0, 12.0, 10.0)
                                    .price_right_inset(8.0)
                                    .hover_overlay(tint(t, Tone::Border, alpha_ghost()))
                                    .separator(true)
                                    .show(ui);
                                // Star click → unpin; body click → activate.
                                if wresp.star_clicked {
                                    if let Some(sec) = watchlist.sections.get_mut(*si) {
                                        if let Some(item) = sec.items.get_mut(*ii) { item.pinned = false; }
                                    }
                                } else if wresp.response.clicked() {
                                    click_sym = Some(pin_sym.clone());
                                }
                            }
                            // Small spacer after the inset section (matches original layout).
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
                                ui.add_space(gap_xs());
                                let cursor_y = ui.cursor().min.y;
                                ui.painter().line_segment(
                                    [egui::pos2(ui.min_rect().left(), cursor_y),
                                     egui::pos2(ui.min_rect().left() + full_w, cursor_y)],
                                    egui::Stroke::new(stroke_std(), tint(t, Tone::Border, alpha_strong())));
                                ui.add_space(gap_xs());
                            }

                            // ── Track section start for continuous background ──
                            let section_block_start_y = ui.cursor().min.y;

                            // Remove item_spacing.y within section for flush rows
                            let prev_item_spacing_y = ui.spacing().item_spacing.y;
                            ui.spacing_mut().item_spacing.y = 0.0;

                            // ── Section header (only if title is non-empty) ──
                            if !sec_title.is_empty() && watchlist.renaming_section != Some(sec_id) {
                                // Wave 11a: SectionHeader retired in favor of canonical PanelSection.
                                // Use a local `expanded` bool to feed `.collapsible()`; the
                                // collapse toggle is propagated via `chevron_clicked` so the
                                // existing `toggle_collapse` deferred-mutation flow still owns
                                // the persistent `sec.collapsed` write. Body is left as the
                                // empty closure — rows continue to render full-width below to
                                // preserve drag/drop hit rects + section background tinting.
                                let mut sec_expanded = !sec_collapsed;
                                let resp = PanelSection::new(&sec_title)
                                    .id_salt(sec_id)
                                    .count(sec_item_count)
                                    .collapsible(&mut sec_expanded)
                                    .delete_when_empty()
                                    .rule(false)
                                    .show(ui, t, |_ui, _t| {});
                                if resp.chevron_clicked { toggle_collapse = Some(si); }
                                if resp.delete_clicked  { remove_section  = Some(si); }
                                section_header_rects.push((si, resp.header_response.rect));
                                #[cfg(debug_assertions)]
                                crate::dev_inspector::record(
                                    crate::dev_inspector::WidgetRecord::from_response(
                                        &format!("watchlist.section.{si}.header"), "header", &sec_title,
                                        &resp.header_response, ui,
                                    )
                                );

                                // Right-click context menu on section header
                                resp.header_response.context_menu(|ui| {
                                    // Rename
                                    if MenuItem::new("Rename").show(ui, t).clicked() {
                                        watchlist.renaming_section = Some(sec_id);
                                        watchlist.rename_buf = sec_title.clone();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    // Color presets
                                    ui.add(MonospaceCode::new("Color").size_px(font_sm_tight()).color(t.dim));
                                    for row in color_presets.chunks(8) {
                                        ui.horizontal(|ui| {
                                            for hex in row {
                                                let c = hex_to_color(hex, 1.0);
                                                let r = ui.add(Button::icon("\u{25CF}").variant(Variant::TextOnly).glyph_color(c).size(Size::Lg).placement(IconPlacement::ListRow));
                                                Tooltip::new(*hex).show(ui, &r, t);
                                                if r.clicked() {
                                                    if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                        sec.color = Some(hex.to_string());
                                                    }
                                                    watchlist.persist();
                                                    ui.close_menu();
                                                }
                                            }
                                        });
                                    }
                                    if MenuItem::new("No color").tint(t.dim).show(ui, t).clicked() {
                                        if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                            sec.color = None;
                                        }
                                        watchlist.persist();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if sec_item_count == 0 {
                                        if MenuItem::new("Delete section").tint(t.bear).show(ui, t).clicked() {
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
                                    ui.add(Button::icon(chevron).variant(Variant::TextOnly).glyph_color(color_muted(t.dim)).size(Size::Sm).placement(IconPlacement::PanelHeader));

                                    let te = Input::new(&mut watchlist.rename_buf)
                                        .width((ui.available_width() - 10.0).max(40.0)).font_size(9.0).show(ui, t);
                                    if te.lost_focus || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                            sec.title = watchlist.rename_buf.clone();
                                        }
                                        watchlist.renaming_section = None;
                                        watchlist.persist();
                                    }
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        watchlist.renaming_section = None;
                                    }
                                    te.request_focus(ui.ctx());
                                });
                            }

                            // ── Section items (skip if collapsed) ──
                            if !sec_collapsed {
                                for ii in 0..sec_item_count {
                                    let item = &watchlist.sections[si].items[ii];
                                    // ── Read non-String fields by value (no allocation) ──────────
                                    let item_price = item.price;
                                    let item_prev_close = item.prev_close;
                                    let item_day_close = item.day_close; // today's regular close (0 while live)
                                    let item_loaded = item.loaded;
                                    let item_is_option = item.is_option;
                                    let item_strike = item.strike;
                                    let item_bid = item.bid;
                                    let item_ask = item.ask;
                                    let item_pinned = item.pinned;
                                    // Skip pinned items in normal sections — they render in the PINNED section above
                                    if item_pinned && has_pinned { continue; }
                                    let item_rvol = item.rvol;
                                    let item_atr = item.atr;
                                    // Populate range data from price if not set
                                    // Real 52-week range only (no synthetic price±%). 0 when no
                                    // feed yet → the 52wk readout/tooltip shows nothing rather
                                    // than a fabricated range.
                                    let item_high_52wk = item.high_52wk;
                                    let item_low_52wk = item.low_52wk;
                                    // Real intraday range only (set from the live snapshot via
                                    // set_day_range). No synthetic price±0.8% fallback — a
                                    // fabricated range is worse than none; the Day Range column
                                    // hides itself when high<=low (off-hours / not yet loaded).
                                    let item_day_high = item.day_high;
                                    let item_day_low = item.day_low;
                                    let item_avg_daily_range = item.avg_daily_range;
                                    let item_earnings_days = item.earnings_days;
                                    let item_alert_triggered = item.alert_triggered;
                                    // Borrow string fields as &str to avoid per-row heap allocations.
                                    // Owned copies are made lazily, only at the use-sites that need
                                    // owned values (click handlers, tooltip, etc.).
                                    let item_sym: &str = &item.symbol;
                                    let item_option_type: &str = &item.option_type;
                                    // price_history: borrow slice — only clone Vec if we'll pass it to spark.
                                    let item_price_history: &[f32] = &item.price_history;
                                    // Flash animation: compute tint+alpha from elapsed time since last tick.
                                    // Duration: 400 ms linear fade. Peak alpha = alpha_soft() (~20).
                                    // No flash on initial load (prev_price == 0) or options (too dense).
                                    let item_prev_price = item.prev_price;
                                    let item_price_change_at = item.price_change_at;
                                    let is_dragged = drag_confirmed && dragging.map_or(false, |d| d.section_idx == si && d.item_idx == ii);

                                    // Skip rendering the dragged item in-place (it's shown as floating)
                                    if is_dragged {
                                        // Reserve space so layout doesn't shift
                                        let placeholder = ui.allocate_space(egui::vec2(full_w, 24.0));
                                        row_rects.push((si, ii, placeholder.1));
                                        continue;
                                    }

                                    // ── Watchlist filter ──
                                    if !item_is_option {
                                        if !filter_text_upper.is_empty() && !item_sym.to_uppercase().contains(&filter_text_upper as &str) {
                                            continue;
                                        }
                                        if watchlist.filter_min_change > -999.0 || watchlist.filter_max_change < 999.0 {
                                            if item_prev_close > 0.0 {
                                                let chg = (item_price / item_prev_close - 1.0) * 100.0;
                                                if watchlist.filter_min_change > -999.0 && chg < watchlist.filter_min_change { continue; }
                                                if watchlist.filter_max_change < 999.0 && chg > watchlist.filter_max_change { continue; }
                                            }
                                            // else: prev_close not loaded yet (e.g. a symbol the
                                            // user JUST added). Keep it visible — a change-% filter
                                            // can't be evaluated without a baseline, and hiding it
                                            // makes "add" look like it silently did nothing until
                                            // the first price arrives.
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
                                        let row_bg = if is_active { tint(t, Tone::Accent, 18) } else { egui::Color32::TRANSPARENT };

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
                                            ui.add(MonospaceCode::new(Icon::DOTS_SIX_VERTICAL).size_px(font_sm_tight()).color(t.dim).gamma(0.2));
                                            ui.add_space(gap_xs());
                                            // C/P badge
                                            let badge_bg = color_alpha(opt_color, 35);
                                            // legacy: monospace+strong; Button uses plain text
                                            let badge_resp = ui.add(Button::new(item_option_type).variant(Variant::Chrome).size(Size::Sm).fg(opt_color)
                                                .fill(badge_bg).corner_radius(current().r_sm as f32).stroke(egui::Stroke::NONE)
                                                .min_size(BTN_ICON_SM));
                                            let _ = badge_resp;
                                            ui.add_space(gap_xs());
                                            // Full option name (e.g. "SPY 560C 0DTE")
                                            let sym_color = if is_active { t.text } else { t.dim };
                                            // Strip the F: futures class tag for display (F:ES → ES).
                                            let disp_sym = item_sym.strip_prefix("F:").unwrap_or(item_sym);
                                            ui.add(MonospaceCode::new(disp_sym).size_px(font_sm()).strong(true).color(sym_color));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // X button
                                                let r = ui.add(Button::icon(Icon::X).variant(Variant::TextOnly).glyph_color(color_very_dim(t.dim)).size(Size::Sm).placement(IconPlacement::ListRow));
                                                Tooltip::new("Remove option").show(ui, &r, t);
                                                if r.clicked() {
                                                    remove_sym = Some(item_sym.to_owned());
                                                }
                                                // Bid x Ask (or price fallback)
                                                ui.add(MonospaceCode::new(&price_str).size_px(font_sm()).color(opt_color));
                                            });
                                        });

                                        let row_rect = resp.response.rect;
                                        row_rects.push((si, ii, row_rect));
                                        #[cfg(debug_assertions)]
                                        crate::dev_inspector::record(
                                            crate::dev_inspector::WidgetRecord::from_response(
                                                &format!("watchlist.option.{si}.{ii}"), "button", item_sym,
                                                &resp.response, ui,
                                            )
                                        );

                                        let drag_resp = resp.response.interact(egui::Sense::click_and_drag());
                                        if drag_resp.drag_started() {
                                            watchlist.dragging = Some(crate::chart_renderer::gpu::WatchlistDragState { section_idx: si, item_idx: ii });
                                            watchlist.drag_start_pos = pointer_pos;
                                            watchlist.drag_confirmed = false;
                                        }
                                        // Click opens option chart (not stock symbol change)
                                        if drag_resp.clicked() && !drag_confirmed {
                                            let is_call = item_option_type == "C";
                                            // Re-borrow item here — item_underlying and item_expiry are
                                            // not pre-extracted since clicks are rare (not every frame).
                                            let item = &watchlist.sections[si].items[ii];
                                            click_opt = Some((item.underlying.clone(), item_strike, is_call, item.expiry.clone()));
                                        }
                                        if drag_resp.hovered() && !drag_confirmed {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                            if !is_active {
                                                ui.painter().rect_filled(row_rect, 0.0, tint(t, Tone::Border, alpha_subtle()));
                                            }
                                        }
                                    } else {
                                        // ── Stock item rendering — migrated to WatchlistRow widget ──
                                        // Live move vs the prior regular close (today's move
                                        // during RTH, or the pre/post-market move otherwise).
                                        let live_chg = if item_prev_close > 0.0 {
                                            ((item_price - item_prev_close) / item_prev_close) * 100.0
                                        } else { 0.0 };
                                        // Main Change %:
                                        //  • RTH                → today's live move (price vs prev close)
                                        //  • closed, day.c set  → last close-to-close (today's
                                        //    regular close vs prior close) — straight from the
                                        //    bulk snapshot, no slow daily-bars fetch
                                        //  • pre-open (day.c==0) → prior session's close-to-close
                                        //    via the cached daily-bars fallback
                                        let close_to_close = if item_day_close > 0.0 && item_prev_close > 0.0 {
                                            Some(((item_day_close - item_prev_close) / item_prev_close) * 100.0)
                                        } else { None };
                                        let change_pct = if mkt_rth {
                                            live_chg
                                        } else {
                                            close_to_close.or_else(||
                                                crate::chart_renderer::gpu::prev_session_change_cached(item_sym))
                                                .unwrap_or(live_chg)
                                        };
                                        // Ext-Hours column = session close → latest price. Reference
                                        // is today's regular close after the close (day.c), else the
                                        // prior close pre-open. Shown only outside RTH with a real move.
                                        let _ = mkt_ext;
                                        let ext_ref = if item_day_close > 0.0 { item_day_close } else { item_prev_close };
                                        let ext_change = if !mkt_rth && ext_ref > 0.0 {
                                            let e = ((item_price - ext_ref) / ext_ref) * 100.0;
                                            if e.abs() > 0.01 { Some(e) } else { None }
                                        } else { None };
                                        let price_str = if item_price > 0.0 { format!("{:.2}", item_price) } else { "---".into() };
                                        let row_h = if item_pinned { 34.0 } else { 28.0 };
                                        let font_sz = if item_pinned { 15.0 } else { 14.0 };

                                        // ── Price-flash tint ────────────────────────────────────────
                                        // Show a subtle bull/bear tint behind the price column for 400 ms
                                        // after a quote change. Skip if prev_price is 0 (initial load) or
                                        // if no change has occurred yet.
                                        let price_flash_tint_col: Option<egui::Color32> = (|| {
                                            let changed_at = item_price_change_at?;
                                            if item_prev_price <= 0.0 { return None; }
                                            const FLASH_MS: f32 = 400.0;
                                            let age_ms = changed_at.elapsed().as_millis() as f32;
                                            if age_ms >= FLASH_MS { return None; }
                                            let flash_alpha = ((1.0 - age_ms / FLASH_MS) * alpha_soft() as f32) as u8;
                                            if flash_alpha == 0 { return None; }
                                            // Request a repaint in ~16 ms to keep the fade smooth.
                                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
                                            if item_price >= item_prev_price {
                                                Some(tint(t, Tone::Bull, flash_alpha))
                                            } else {
                                                Some(tint(t, Tone::Bear, flash_alpha))
                                            }
                                        })();

                                        // Pinned section: slightly distinct background tint (active wins).
                                        let row_tint = if is_active {
                                            tint(t, Tone::Accent, 18)
                                        } else if item_pinned {
                                            // Two layered tints (panel previously painted both): blend into one.
                                            // 80,120,200,12 + t.text @ alpha 4 → use the bluish tint; the t.text@4
                                            // overlay was nearly invisible. Visual parity preserved within 1 alpha.
                                            tint(t, Tone::Accent, super::super::style::alpha_ghost())
                                        } else {
                                            egui::Color32::TRANSPARENT
                                        };

                                        let pin_state = if item_pinned { WatchlistPinState::Pinned } else { WatchlistPinState::NotPinned };
                                        let icons = WatchlistIconSet {
                                            drag_handle: Icon::DOTS_SIX_VERTICAL,
                                            star: Icon::SPARKLE,
                                            x: Icon::X,
                                            alert: Icon::LIGHTNING,
                                        };
                                        let row_disp_sym = item_sym.strip_prefix("F:").unwrap_or(item_sym);
                                        let mut row_b = WatchlistRow::new(row_disp_sym, item_price, change_pct)
                                            .theme(t)
                                            .height(row_h)
                                            .active(is_active)
                                            .drag_handle(true)
                                            .pin_state(pin_state)
                                            .show_star_on_hover(true)
                                            .alert_indicator(item_alert_triggered)
                                            // Real RVOL from the server endpoint
                                            // (/api/stocks/rvol = today_volume ÷ avg_volume_20d),
                                            // TTL-cached. None until it lands → column hides
                                            // rather than faking.
                                            .rvol(crate::chart_renderer::gpu::rvol_cached(item_sym))
                                            .ext_change(ext_change)
                                            .columns(&watchlist.wl_columns)
                                            .extreme_move_tint(if item_prev_close > 0.0 { Some(item_avg_daily_range) } else { None })
                                            .icon_set(icons)
                                            .sense(egui::Sense::click_and_drag())
                                            .row_tint(row_tint)
                                            .separator(true)
                                            .hover_overlay(tint(t, Tone::Border, alpha_soft()))
                                            .show_x_on_hover(true)
                                            // Panel renders its own left-side rich tooltip
                                            // (set_pending_wl_tooltip) — suppress the row's
                                            // built-in HoverCard to avoid a duplicate.
                                            .hover_card(false)
                                            .drag_confirmed(drag_confirmed)
                                            .sym_font(egui::FontId::monospace(font_sz))
                                            .chg_font(egui::FontId::proportional(font_sz))
                                            .price_font(egui::FontId::proportional(font_md()))
                                            .price_string(price_str)
                                            .price_right_inset(24.0)
                                            // Panel symbol layout: star at left+16, sym at star+10 when star
                                            // visible, else at left+18.
                                            .sym_layout(0.0, 10.0, 18.0);
                                        if let Some(flash_col) = price_flash_tint_col {
                                            row_b = row_b.price_flash_tint(flash_col);
                                        }
                                        if item_earnings_days >= 0 && (item_earnings_days as u32) <= 14 {
                                            row_b = row_b.earnings_days(Some(item_earnings_days as u32));
                                        }
                                        if !item_loaded {
                                            // Suppress change% text when not loaded by overriding chg font with
                                            // a tiny invisible color is overkill — the inline path used "" string;
                                            // simplest: leave the widget to paint formatted change_pct (0.00%).
                                            // Visual delta: pre-load shows "+0.00%" briefly. Acceptable.
                                        }
                                        // Sparkline removed per design — the Ext-Hours column
                                        // takes its slot. (item_price_history still feeds the
                                        // price-flash tint.)
                                        let _ = item_price_history;
                                        if item_day_high > item_day_low {
                                            row_b = row_b.day_range(item_day_low, item_day_high, item_price);
                                        }

                                        let wresp = row_b.show(ui);
                                        let rect = wresp.response.rect;
                                        let resp = &wresp.response;
                                        let row_hovered = resp.hovered();
                                        let y_c = rect.center().y;
                                        #[cfg(debug_assertions)]
                                        crate::dev_inspector::record(
                                            crate::dev_inspector::WidgetRecord::from_response(
                                                &format!("watchlist.item.{si}.{ii}"), "button", item_sym,
                                                resp, ui,
                                            )
                                        );

                                        // ── Corporate-actions / news badges ──
                                        // Pure-data fetch via projector caches (TTL-gated, won't
                                        // thread-storm). Painted just to the left of the X / price
                                        // cluster so they don't overlap drag handle or symbol.
                                        // Use frame_now_ms hoisted outside the row loop (one syscall/frame).
                                        let (badge_text, badge_tip) = super::watchlist_badges::badges_for_ticker(&item_sym, frame_now_ms);
                                        if !badge_text.is_empty() {
                                            let badge_id = egui::Id::new(("wl_badge", si, ii));
                                            let badge_w = 64.0_f32.min(rect.width() * 0.30);
                                            let badge_rect = egui::Rect::from_min_max(
                                                egui::pos2(rect.max.x - 84.0 - badge_w, rect.min.y + 4.0),
                                                egui::pos2(rect.max.x - 84.0, rect.max.y - 4.0),
                                            );
                                            ui.painter().text(
                                                badge_rect.right_center(),
                                                egui::Align2::RIGHT_CENTER,
                                                &badge_text,
                                                egui::FontId::proportional(font_sm()),
                                                t.dim,
                                            );
                                            let badge_resp = ui.interact(badge_rect, badge_id, egui::Sense::hover());
                                            if !badge_tip.is_empty() {
                                                Tooltip::new(badge_tip).show(ui, &badge_resp, t);
                                            }
                                        }

                                        // ── Rich tooltip — deferred ──
                                        // Clone sym + tags only when the row is actually hovered
                                        // (rare, at most one row per frame) — not every row every frame.
                                        if row_hovered && !drag_confirmed {
                                            let item = &watchlist.sections[si].items[ii];
                                            set_pending_wl_tooltip(Some(WlTooltipData {
                                                sym: item_sym.to_owned(), price: item_price, prev_close: item_prev_close,
                                                day_high: item_day_high, day_low: item_day_low,
                                                high_52wk: item_high_52wk, low_52wk: item_low_52wk,
                                                atr: item_atr, rvol: item_rvol, avg_range: item_avg_daily_range,
                                                earnings_days: item_earnings_days, tags: item.tags.clone(),
                                                alert_triggered: item_alert_triggered,
                                                anchor_y: y_c, sidebar_left: rect.left() - 10.0,
                                            }));
                                        }

                                        row_rects.push((si, ii, rect));

                                        // ── Drag start ──
                                        if wresp.response.drag_started() {
                                            watchlist.dragging = Some(crate::chart_renderer::gpu::WatchlistDragState { section_idx: si, item_idx: ii });
                                            watchlist.drag_start_pos = pointer_pos;
                                            watchlist.drag_confirmed = false;
                                        }
                                        // ── Click routing — X removes, Star toggles pin, Body activates ──
                                        if !drag_confirmed {
                                            if wresp.x_clicked {
                                                remove_sym = Some(item_sym.to_owned());
                                            } else if wresp.star_clicked {
                                                if let Some(sec) = watchlist.sections.get_mut(si) {
                                                    if let Some(item) = sec.items.get_mut(ii) { item.pinned = !item.pinned; }
                                                }
                                            } else if wresp.response.clicked() {
                                                click_sym = Some(item_sym.to_owned());
                                            }
                                        }
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
                                watchlist.drop_target = best.map(|(s, i, _)| crate::chart_renderer::gpu::WatchlistDragState { section_idx: s, item_idx: i });
                            }

                            // Draw insertion indicator line
                            if let Some(dt) = watchlist.drop_target {
                                let (dt_sec, dt_idx) = (dt.section_idx, dt.item_idx);
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
                                        egui::Stroke::new(stroke_thick(), t.accent));
                                    // Small circles at endpoints
                                    ui.painter().circle_filled(egui::pos2(left + 2.0, indicator_y), 3.0, t.accent);
                                    ui.painter().circle_filled(egui::pos2(left + full_w - 2.0, indicator_y), 3.0, t.accent);
                                }
                            }

                            // Draw floating label at cursor
                            if let (Some(drag), Some(mouse)) = (watchlist.dragging, pointer_pos) {
                                let (src_sec, src_idx) = (drag.section_idx, drag.item_idx);
                                if src_sec < watchlist.sections.len() && src_idx < watchlist.sections[src_sec].items.len() {
                                    let drag_sym = &watchlist.sections[src_sec].items[src_idx].symbol;
                                    let float_rect = egui::Rect::from_min_size(
                                        egui::pos2(mouse.x - 30.0, mouse.y - 10.0), egui::vec2(80.0, 20.0));
                                    ui.painter().rect_filled(float_rect, 4.0, tint(t, Tone::Accent, alpha_muted()));
                                    ui.painter().rect_stroke(float_rect, 4.0, egui::Stroke::new(stroke_std(), t.accent), egui::StrokeKind::Outside);
                                    ui.painter().text(float_rect.center(), egui::Align2::CENTER_CENTER,
                                        drag_sym, mono_md(), t.text);
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                }
                            }
                        }

                        // Drop: on pointer release while dragging
                        if pointer_released && watchlist.drag_confirmed {
                            if let (Some(src), Some(dst)) = (watchlist.dragging, watchlist.drop_target) {
                                let (src_sec, src_idx, dst_sec, dst_idx) = (src.section_idx, src.item_idx, dst.section_idx, dst.item_idx);
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
                        ui.add_space(gap_md());
                        ui.horizontal(|ui| {
                            let add_sec_lbl = format!("{} Section", Icon::PLUS);
                            if ui.add(Button::new(add_sec_lbl.as_str()).variant(Variant::TextOnly).size(Size::Sm).fg(color_dim(t.dim))).clicked() {
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
                        ui.add_space(gap_xs());
                        let div_r = ui.available_rect_before_wrap();
                        let div_y = ui.cursor().min.y;
                        let div_rect = egui::Rect::from_min_max(
                            egui::pos2(div_r.left(), div_y),
                            egui::pos2(div_r.right(), div_y + 6.0));
                        ui.painter().rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(div_r.left(), div_y + 1.0),
                                egui::pos2(div_r.right(), div_y + 4.0)),
                            0.0, tint(t, Tone::Border, 160));
                        // Store divider Y position for drag handling outside the panel
                        watchlist.divider_y = div_rect.center().y;
                        watchlist.divider_total_h = total_avail;
                        // Show resize cursor on hover — wired through the canonical
                        // cursor helper. Synthesize a one-frame Response over the
                        // divider rect so the helper's hover/drag transitions fire.
                        let div_hover_resp = ui.interact(
                            div_rect.expand(6.0),
                            egui::Id::new("wl_options_divider_cursor"),
                            egui::Sense::hover(),
                        );
                        crate::chart_renderer::ui::style::cursor::resize_v(ui, &div_hover_resp);
                        if watchlist.divider_dragging {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        }
                        ui.add_space(gap_md());

                        // OPTIONS sub-header — migrated to PanelSection chrome.
                        // `rule(false)` suppresses the section's hairline because the
                        // divider above already separates the LIST scroll from the
                        // options scroll. Count chip preserves the prior "(n)" badge.
                        let opt_count: usize = watchlist.sections.iter()
                            .filter(|s| s.title.contains("Options"))
                            .map(|s| s.items.len()).sum();
                        PanelSection::new("OPTIONS")
                            .count(opt_count)
                            .title_color(t.accent)
                            .rule(false)
                            .show(ui, t, |_ui, _t| {});

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

                                // Section header with collapse chevron — Wave 11a:
                                // migrated to canonical PanelSection (see stocks-loop comment).
                                let mut sec_expanded = !sec_collapsed;
                                let resp = PanelSection::new(&sec_title)
                                    .id_salt(sec_id)
                                    .count(sec_item_count)
                                    .collapsible(&mut sec_expanded)
                                    .delete_when_empty()
                                    .rule(false)
                                    .show(ui, t, |_ui, _t| {});
                                if resp.chevron_clicked { opt_toggle_collapse = Some(si); }
                                if resp.delete_clicked  { opt_remove_section  = Some(si); }

                                // Right-click context menu on option section header (same as stock sections)
                                resp.header_response.context_menu(|ui| {
                                    // Rename
                                    if MenuItem::new("Rename").show(ui, t).clicked() {
                                        watchlist.renaming_section = Some(sec_id);
                                        watchlist.rename_buf = sec_title.clone();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    // Color presets
                                    ui.add(MonospaceCode::new("Color").size_px(font_sm_tight()).color(t.dim));
                                    for row in color_presets.chunks(8) {
                                        ui.horizontal(|ui| {
                                            for hex in row {
                                                let c = hex_to_color(hex, 1.0);
                                                let r = ui.add(Button::icon("\u{25CF}").variant(Variant::TextOnly).glyph_color(c).size(Size::Lg).placement(IconPlacement::ListRow));
                                                Tooltip::new(*hex).show(ui, &r, t);
                                                if r.clicked() {
                                                    if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                        sec.color = Some(hex.to_string());
                                                    }
                                                    watchlist.persist();
                                                    ui.close_menu();
                                                }
                                            }
                                        });
                                    }
                                    if MenuItem::new("No color").tint(t.dim).show(ui, t).clicked() {
                                        if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                            sec.color = None;
                                        }
                                        watchlist.persist();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if sec_item_count == 0 {
                                        if MenuItem::new("Delete section").tint(t.bear).show(ui, t).clicked() {
                                            opt_remove_section = Some(si);
                                            ui.close_menu();
                                        }
                                    }
                                });
                                ui.add_space(gap_xs());

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
                                        let row_bg = if is_active { tint(t, Tone::Accent, 18) } else { egui::Color32::TRANSPARENT };

                                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(full_w, 28.0), egui::Sense::click());
                                        let painter = ui.painter();
                                        painter.rect_filled(rect, 0.0, row_bg);
                                        if resp.hovered() {
                                            painter.rect_filled(rect, 0.0, tint(t, Tone::Border, alpha_subtle()));
                                        }
                                        cursor::focus_ring(ui, &resp, t.accent);
                                        crate::chart_renderer::ui::style::cursor::clickable(ui, &resp);

                                        let badge = if is_call { "C" } else { "P" };
                                        let y_c = rect.center().y;
                                        // C/P badge
                                        painter.text(egui::pos2(rect.left() + 6.0, y_c), egui::Align2::LEFT_CENTER,
                                            badge, mono_md(), color);
                                        // Contract name
                                        painter.text(egui::pos2(rect.left() + 22.0, y_c), egui::Align2::LEFT_CENTER,
                                            &format!("{} {:.0} {}", item_underlying, item_strike, item_expiry),
                                            mono_lg(), t.text);
                                        // Bid x Ask (right-aligned)
                                        if item_bid > 0.0 || item_ask > 0.0 {
                                            painter.text(egui::pos2(rect.right() - 6.0, y_c), egui::Align2::RIGHT_CENTER,
                                                &format!("{:.2} x {:.2}", item_bid, item_ask),
                                                mono_lg(), color_subtle(color));
                                        }
                                        // Faint separator
                                        painter.line_segment(
                                            [egui::pos2(rect.left() + 16.0, rect.bottom() - 0.5), egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5)],
                                            egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_muted())));

                                        if resp.clicked() {
                                            click_opt = Some((item_underlying.clone(), item_strike, is_call, item_expiry.clone()));
                                        }

                                        // X button to remove
                                        let x_rect = egui::Rect::from_min_size(egui::pos2(rect.right() - 16.0, rect.top()), egui::vec2(16.0, 22.0));
                                        if resp.hovered() {
                                            let x_resp = ui.interact(x_rect, egui::Id::new(("opt_x", si, ii, "opt_item")), egui::Sense::click());
                                            crate::chart_renderer::ui::style::cursor::clickable(ui, &x_resp);
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
                                ui.add_space(gap_sm());
                            }

                            // Empty state — canonical PanelEmpty with hint.
                            if option_section_ids.is_empty() {
                                PanelEmpty::new("No options saved")
                                    .hint("Shift+click contracts in the CHAIN tab")
                                    .show(ui, t);
                            }

                            // "+ Section" button at bottom of options area
                            ui.add_space(gap_md());
                            ui.horizontal(|ui| {
                                let add_opt_sec_lbl = format!("{} Section", Icon::PLUS);
                                if ui.add(Button::new(add_opt_sec_lbl.as_str()).variant(Variant::TextOnly).size(Size::Sm).fg(color_dim(t.dim))).clicked() {
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
                    if watchlist.chain_0dte.calls.is_empty() && !watchlist.chain_loading {
                        let ns = watchlist.chain_num_strikes;
                        let sym = watchlist.chain_symbol.clone();
                        let far_dte = watchlist.chain_far_dte;
                        watchlist.chain_loading = true;
                        // Wave 5: mirror the legacy boolean into the central
                        // InFlightRegistry. The boolean stays authoritative for
                        // the other ~6 sites; this is the proof-of-concept that
                        // the registry tracks the same lifecycle. Dedup so we
                        // don't double-register if the panel re-enters before
                        // the previous fetch returns.
                        let kind = crate::state::InFlightKind::OptionsChain {
                            underlying: sym.clone(),
                        };
                        if watchlist.inflight.dedup_kind(&kind).is_none() {
                            let _ = watchlist.inflight.start(
                                kind,
                                std::time::Duration::from_secs(10),
                            );
                        }
                        watchlist.chain_last_fetch = Some(std::time::Instant::now());
                        fetch_chain_background(sym.clone(), ns, 0, chain_price);
                        fetch_chain_background(sym, ns, far_dte, chain_price);
                    }

                    // ── Placeholder-data banner (Wave: chain fallback) ──
                    // When the real upstream chain is unavailable we render a
                    // Black-Scholes synthesized chain so the panel isn't empty.
                    // Surface it loudly so the user doesn't trade off fake bids.
                    if watchlist.chain_0dte_placeholder || watchlist.chain_far_placeholder {
                        let strip_h = 18.0;
                        let avail = ui.available_width();
                        let (strip_rect, _) = ui.allocate_exact_size(
                            egui::vec2(avail, strip_h),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(
                            strip_rect, 0.0, tint(t, Tone::Warn, alpha_dim()));
                        ui.painter().text(
                            strip_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "PLACEHOLDER DATA — real chain unavailable",
                            mono_sm(),
                            t.warn,
                        );
                        ui.add_space(gap_xs());
                    }

                    // ── Controls: DTE selector | sel toggle | Spread ──
                    ui.horizontal(|ui| {
                        // DTE dropdown
                        let dte_values = [1i32, 2, 3, 5, 7, 10];
                        let dte_labels: Vec<String> = dte_values.iter().map(|&d| dte_label(d)).collect();
                        let dte_opts: Vec<(i32, &str)> = dte_values.iter().zip(dte_labels.iter())
                            .map(|(d, l)| (*d, l.as_str())).collect();
                        dim_label(ui, "DTE", t.dim);
                        let mut cur_dte = watchlist.chain_far_dte;
                        if super::super::inputs::select::Dropdown::new("far_dte")
                            .options(&dte_opts)
                            .width(100.0)
                            .theme(t)
                            .show(ui, &mut cur_dte)
                        {
                            watchlist.chain_far_dte = cur_dte;
                            let sym = watchlist.chain_symbol.clone();
                            watchlist.chain_loading = true;
                            fetch_chain_background(sym, watchlist.chain_num_strikes, cur_dte, chain_price);
                        }
                        // Select mode toggle
                        let sel_active = watchlist.chain_select_mode;
                        let sel_lbl: String = if sel_active { format!("{} sel", Icon::CHECK) } else { "sel".into() };
                        if ui.add(Button::new(sel_lbl.as_str()).variant(Variant::Chip).size(Size::Sm)
                            .active(sel_active)).clicked() {
                            watchlist.chain_select_mode = !watchlist.chain_select_mode;
                        }
                        // Spread Builder shortcut
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if Button::small_action("Spread").tint(t.dim).show(ui, t).clicked() {
                                watchlist.update_sidebar_state(|s| s.spread_open = !s.spread_open);
                            }
                        });
                    });

                    ui.add_space(gap_sm());

                    // ── Symbol selector + price ──
                    ui.horizontal(|ui| {
                        let has_focus = ui.memory(|m| m.has_focus(egui::Id::new("chain_sym_edit")));
                        let input_bg = if has_focus { tint(t, Tone::Border, alpha_dim()) } else { tint(t, Tone::Border, alpha_ghost()) };
                        let sym_resp = Input::new(&mut watchlist.chain_sym_input)
                            .id(egui::Id::new("chain_sym_edit"))
                            .placeholder(watchlist.chain_symbol.clone())
                            .width(70.0)
                            .font_size(14.0)
                            .text_color(t.accent)
                            .background_color(input_bg)
                            .margin(egui::Margin::symmetric(gap_sm() as i8, gap_xs() as i8))
                            .show(ui, t);
                        if !has_focus {
                            let display_text = if watchlist.chain_sym_input.is_empty() { &watchlist.chain_symbol } else { &watchlist.chain_sym_input };
                            let r = sym_resp.response.rect;
                            ui.painter().text(egui::pos2(r.left() + 6.0, r.center().y), egui::Align2::LEFT_CENTER,
                                display_text, mono_lg(), t.accent);
                        }
                        // Price display
                        if chain_price > 0.0 {
                            ui.add_space(gap_md());
                            ui.add(MonospaceCode::new(&format!("${:.2}", chain_price)).size_px(font_lg()).color(t.text));
                        }
                        // Search — static immediate + ApexIB background
                        if sym_resp.response.changed() && !watchlist.chain_sym_input.is_empty() {
                            watchlist.search_results = crate::ui_kit::symbols::search_symbols(&watchlist.chain_sym_input, 5)
                                .iter().map(|s| (s.symbol.to_string(), s.name.to_string())).collect();
                            // Also fire ApexIB search in background
                            fetch_search_background(watchlist.chain_sym_input.clone(), "chain".to_string());
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !watchlist.chain_sym_input.is_empty() {
                            watchlist.chain_symbol = watchlist.chain_sym_input.trim().to_uppercase();
                            watchlist.chain_sym_input.clear();
                            watchlist.search_results.clear();
                            watchlist.chain_0dte = crate::chart_renderer::gpu::OptionChain::default();
                            watchlist.chain_underlying_price = 0.0; // reset price for new symbol
                            watchlist.chain_center_offset = 0;
                            watchlist.chain_loading = false;
                        }
                    });
                    // Search suggestions popup
                    if !watchlist.chain_sym_input.is_empty() && !watchlist.search_results.is_empty() {
                        PopupFrame::new().colors(t.toolbar_bg, t.toolbar_border).ctx(ctx).build().show(ui, |ui| {
                            for (sym, name) in watchlist.search_results.clone() {
                                let chain_sugg_lbl = format!("{} {}", sym, name);
                                if ui.add(Button::new(chain_sugg_lbl.as_str()).variant(Variant::Ghost).size(Size::Sm)
                                    .full_width(true).min_size(egui::vec2(ui.available_width(), row_height_compact()))).clicked() {
                                    watchlist.chain_symbol = sym;
                                    watchlist.chain_sym_input.clear();
                                    watchlist.search_results.clear();
                                    watchlist.chain_0dte = crate::chart_renderer::gpu::OptionChain::default();
                                    watchlist.chain_underlying_price = 0.0;
                                    watchlist.chain_center_offset = 0;
                                    watchlist.chain_loading = false;
                                }
                            }
                        });
                    }

                    ui.add_space(gap_sm());
                    // Separator before chain data
                    let sep_r = ui.available_rect_before_wrap();
                    ui.painter().line_segment(
                        [egui::pos2(sep_r.left(), ui.cursor().min.y), egui::pos2(sep_r.right(), ui.cursor().min.y)],
                        egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_muted())));
                    ui.add_space(gap_sm());

                    // Loading indicator — canonical PanelLoading.
                    if watchlist.chain_loading {
                        PanelLoading::new().reason("Loading chain").show(ui, t);
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
                        let hdr_color = color_dim(t.dim);
                        ui.add_space(col_chk);
                        ui.allocate_ui(egui::vec2(col_stk, 14.0), |ui| { dim_label(ui, "STK", hdr_color); });
                        ui.allocate_ui(egui::vec2(col_bid, 14.0), |ui| { dim_label(ui, "BID", hdr_color); });
                        ui.allocate_ui(egui::vec2(col_ask, 14.0), |ui| { dim_label(ui, "ASK", hdr_color); });
                        ui.allocate_ui(egui::vec2(col_oi, 14.0), |ui| { dim_label(ui, "OI", hdr_color); });
                    });

                    // ── Helper to render one option row ──
                    // Track clicked contract for opening chart (normal click).
                    // Tuple is (underlying, strike, is_call, expiry_label, occ_ticker)
                    // — the trailing OCC is forwarded to pending_opt_chart_contract so
                    // the consumer doesn't have to fall back on synthesize_occ (which
                    // can produce a wrong ticker on weekday-incompatible expiries).
                    let clicked_contract: std::cell::Cell<Option<(String, f32, bool, String, String)>> = std::cell::Cell::new(None);
                    // Track shift-clicked contract for adding to watchlist (select mode / shift+click)
                    let watchlist_add: std::cell::Cell<Option<(String, f32, bool, String, f32, f32)>> = std::cell::Cell::new(None);
                    let render_row = |ui: &mut egui::Ui, row: &OptionRow, is_call: bool, exp_label: &str, sym: &str, saved: &mut Vec<SavedOption>, select_mode: bool, w: f32| {
                        let is_saved = saved.iter().any(|s| s.contract == row.contract);
                        let color = if is_call { t.bull } else { t.bear };
                        let base_tint = if is_call { tint(t, Tone::Bull, 8) } else { tint(t, Tone::Bear, 8) };
                        let itm_bg = if row.itm { color.gamma_multiply(0.06) } else { base_tint };
                        let saved_bg = if is_saved { tint(t, Tone::Accent, alpha_muted()) } else { itm_bg };

                        // Reserve a clickable rect for the whole row
                        let (rect, row_resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());

                        // Paint background
                        let bg = if row_resp.hovered() { tint(t, Tone::Border, alpha_line()) } else { saved_bg };
                        ui.painter().rect_filled(rect, 0.0, bg);
                        if row_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                        cursor::focus_ring(ui, &row_resp, t.accent);

                        let mut x = rect.left();
                        let y_center = rect.center().y;
                        let painter = ui.painter();

                        // Check mark
                        if is_saved {
                            painter.text(egui::pos2(x + col_chk * 0.5, y_center), egui::Align2::CENTER_CENTER,
                                Icon::CHECK, egui::FontId::proportional(font_sm()), t.accent);
                        }
                        x += col_chk + gap;

                        // Strike
                        painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                            &format!("{:.0}", row.strike), mono_lg(), t.text);
                        x += col_stk + gap;

                        // Bid
                        painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                            &format!("{:.2}", row.bid), mono_lg(), color);
                        x += col_bid + gap;

                        // Ask
                        painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                            &format!("{:.2}", row.ask), mono_lg(), t.dim);
                        x += col_ask + gap;

                        // OI
                        let oi_str = if row.oi >= 1_000_000 { format!("{:.1}M", row.oi as f32 / 1_000_000.0) }
                            else if row.oi >= 1_000 { format!("{},{:03}", row.oi / 1000, row.oi % 1000) }
                            else { format!("{}", row.oi) };
                        let oi_x = x;
                        painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                            &oi_str, mono_sm(), color_half(t.dim));

                        // IV indicator — left edge strip on the row
                        if row.iv > 0.0 {
                            let iv_color = if row.iv > 0.7 { tint(t, Tone::Bear, 180) }
                                else if row.iv > 0.5 { tint(t, Tone::Warn, 140) }
                                else if row.iv > 0.3 { tint(t, Tone::Warn, alpha_active()) }
                                else { tint(t, Tone::Bull, alpha_active()) };
                            painter.rect_filled(egui::Rect::from_min_size(
                                egui::pos2(rect.left(), rect.top()), egui::vec2(3.0, rect.height())),
                                0.0, iv_color);
                        }

                        // (Unusual-activity gold badge around OI removed —
                        // it was visually noisy. The same signal is encoded
                        // in the IV strip on the row's left edge.)
                        let _ = oi_x;

                        // Faint row separator
                        painter.line_segment(
                            [egui::pos2(rect.left() + 4.0, rect.bottom() - 0.5), egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5)],
                            egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_tint())));

                        // Click handling
                        if row_resp.clicked() {
                            if select_mode || ui.input(|i| i.modifiers.shift) {
                                if is_saved { saved.retain(|s| s.contract != row.contract); }
                                else { saved.push(SavedOption { contract: row.contract.clone(), symbol: sym.into(), strike: row.strike, is_call, expiry: exp_label.into(), last: row.last }); }
                                watchlist_add.set(Some((sym.into(), row.strike, is_call, exp_label.into(), row.bid, row.ask)));
                            } else {
                                clicked_contract.set(Some((sym.into(), row.strike, is_call, exp_label.into(), row.contract.clone())));
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
                            ui.add(MonospaceCode::new(&exp_label).size_px(12.0).strong(true).color(t.accent));
                            ui.add(MonospaceCode::new(&date_str).size_px(font_sm()).color(t.dim).gamma(0.6));
                        });
                        ui.add_space(gap_xs());

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
                            ui.add(MonospaceCode::new("No strikes available").size_px(font_sm_tight()).color(t.dim).gamma(0.4));
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
                                    // Guard against price == 0 (no feed data yet) to avoid divide-by-zero / NaN.
                                    all_strikes.iter().filter(|&&s| price > 0.0 && (s - price).abs() / price <= pct).copied().collect()
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
                                        if price <= 0.0 { false }
                                        else if s >= price { s >= call_start_price && (s - call_start_price) / price <= pct }
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
                        ui.add_space(gap_xs());
                        {
                            let r = ui.available_rect_before_wrap();
                            let y = ui.cursor().min.y;
                            let badge_w = 80.0;
                            let center_x = r.left() + r.width() / 2.0;
                            // Lines on either side of the badge
                            ui.painter().line_segment(
                                [egui::pos2(r.left() + 4.0, y + 10.0), egui::pos2(center_x - badge_w / 2.0 - 4.0, y + 10.0)],
                                egui::Stroke::new(stroke_std(), tint(t, Tone::Border, alpha_strong())));
                            ui.painter().line_segment(
                                [egui::pos2(center_x + badge_w / 2.0 + 4.0, y + 10.0), egui::pos2(r.right() - 4.0, y + 10.0)],
                                egui::Stroke::new(stroke_std(), tint(t, Tone::Border, alpha_strong())));
                            // Badge background
                            let badge_rect = egui::Rect::from_center_size(egui::pos2(center_x, y + 10.0), egui::vec2(badge_w, 18.0));
                            ui.painter().rect_filled(badge_rect, 9.0, tint(t, Tone::Border, alpha_muted()));
                            ui.painter().rect_stroke(badge_rect, 9.0, egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_strong())), egui::StrokeKind::Outside);
                            // Price text
                            let badge_text = if center_offset != 0 {
                                format!("${:.2} ({:+})", price, center_offset)
                            } else {
                                format!("${:.2}", price)
                            };
                            ui.painter().text(badge_rect.center(), egui::Align2::CENTER_CENTER,
                                &badge_text, mono_md(),
                                t.text);
                        }
                        ui.add_space(gap_xl());

                        // Puts (ATM at top, OTM at bottom)
                        for row in &sorted_puts { render_row(ui, row, false, &exp_label, sym, saved, select_mode, w); }
                        ui.add_space(gap_sm());
                    };

                    // ── Scroll area with two expiry blocks ──
                    let scroll_w = ui.available_width();
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        // min_width removed — was preventing sidebar resize
                        let sym = watchlist.chain_symbol.clone();
                        let sel = watchlist.chain_select_mode;
                        let calls_0 = watchlist.chain_0dte.calls.clone();
                        let puts_0 = watchlist.chain_0dte.puts.clone();
                        let calls_f = watchlist.chain_far.calls.clone();
                        let puts_f = watchlist.chain_far.puts.clone();
                        let far_dte = watchlist.chain_far_dte;

                        // Per-chain controls: 0DTE
                        ui.horizontal(|ui| {
                            dim_label(ui, "0DTE", t.dim);
                            // Mode dropdown (Count, %, StdDev)
                            {
                                let sm_header = match watchlist.chain_0_strike_mode {
                                    StrikeMode::Count => "Cnt".into(),
                                    StrikeMode::Pct(i) => format!("{}%", PCT_OPTIONS.get(i as usize).unwrap_or(&1.0)),
                                    StrikeMode::StdDev => "σ".into(),
                                };
                                let mut sm_opts: Vec<(StrikeMode, String)> = vec![
                                    (StrikeMode::Count, "Count".into()),
                                ];
                                for (pi, &pct) in PCT_OPTIONS.iter().enumerate() {
                                    sm_opts.push((StrikeMode::Pct(pi as u8), format!("{}%", pct)));
                                }
                                sm_opts.push((StrikeMode::StdDev, "Std Dev".into()));
                                super::super::inputs::select::DropdownOwned::new("sm_0")
                                    .options(sm_opts)
                                    .width(40.0)
                                    .font_size(8.0)
                                    .selected_text(sm_header)
                                    .theme(t)
                                    .show(ui, &mut watchlist.chain_0_strike_mode);
                            }
                            // Count ± (always visible)
                            if ui.add(Button::new("-").variant(Variant::Ghost).size(Size::Sm)).clicked() { watchlist.chain_0_num_strikes = watchlist.chain_0_num_strikes.saturating_sub(1).max(1); }
                            ui.add(MonospaceCode::new(&format!("{}", watchlist.chain_0_num_strikes)).size_px(font_xs()).color(t.dim));
                            if ui.add(Button::new("+").variant(Variant::Ghost).size(Size::Sm)).clicked() { watchlist.chain_0_num_strikes += 1; }
                            // Near / Mid / Far toggles
                            NmfToggle::new(&mut watchlist.chain_0_nmf).theme(t).show(ui);
                            // Freeze + arrows
                            let fr_icon_0 = if watchlist.chain_0_frozen { Icon::PAUSE } else { Icon::PLAY };
                            let r = ui.add(Button::icon(fr_icon_0).variant(Variant::MutedIcon).active(watchlist.chain_0_frozen).size(Size::Sm).placement(IconPlacement::PanelHeader));
                            Tooltip::new("Freeze strikes").show(ui, &r, t);
                            if r.clicked() {
                                watchlist.chain_0_frozen = !watchlist.chain_0_frozen;
                                if !watchlist.chain_0_frozen { watchlist.chain_0_offset = 0; }
                            }
                            if watchlist.chain_0_frozen {
                                let r = ui.add(Button::icon(Icon::ARROW_FAT_UP).variant(Variant::MutedIcon).size(Size::Sm).placement(IconPlacement::PanelHeader));
                                Tooltip::new("Shift strikes up").show(ui, &r, t);
                                if r.clicked() { watchlist.chain_0_offset += 1; }
                                let r = ui.add(Button::icon(Icon::ARROW_FAT_DOWN).variant(Variant::MutedIcon).size(Size::Sm).placement(IconPlacement::PanelHeader));
                                Tooltip::new("Shift strikes down").show(ui, &r, t);
                                if r.clicked() { watchlist.chain_0_offset -= 1; }
                            }
                        });
                        let ns_0 = watchlist.chain_0_num_strikes;
                        let off_0 = watchlist.chain_0_offset;
                        let sm_0 = watchlist.chain_0_strike_mode;
                        let nmf_0 = watchlist.chain_0_nmf;
                        render_block(ui, 0, &calls_0, &puts_0, &sym, chain_price, &mut watchlist.saved_options, sel, scroll_w, ns_0, off_0, sm_0, nmf_0);

                        ui.add_space(gap_md());
                        let sep_r = ui.available_rect_before_wrap();
                        ui.painter().line_segment(
                            [egui::pos2(sep_r.left() + 4.0, ui.cursor().min.y), egui::pos2(sep_r.right() - 4.0, ui.cursor().min.y)],
                            egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_line())));
                        ui.add_space(gap_sm());

                        // Per-chain controls: far DTE
                        ui.horizontal(|ui| {
                            dim_label(ui, &format!("{}DTE", far_dte), t.dim);
                            {
                                let sm_header = match watchlist.chain_far_strike_mode {
                                    StrikeMode::Count => "Cnt".into(),
                                    StrikeMode::Pct(i) => format!("{}%", PCT_OPTIONS.get(i as usize).unwrap_or(&1.0)),
                                    StrikeMode::StdDev => "σ".into(),
                                };
                                let mut sm_opts: Vec<(StrikeMode, String)> = vec![
                                    (StrikeMode::Count, "Count".into()),
                                ];
                                for (pi, &pct) in PCT_OPTIONS.iter().enumerate() {
                                    sm_opts.push((StrikeMode::Pct(pi as u8), format!("{}%", pct)));
                                }
                                sm_opts.push((StrikeMode::StdDev, "Std Dev".into()));
                                super::super::inputs::select::DropdownOwned::new("sm_f")
                                    .options(sm_opts)
                                    .width(40.0)
                                    .font_size(8.0)
                                    .selected_text(sm_header)
                                    .theme(t)
                                    .show(ui, &mut watchlist.chain_far_strike_mode);
                            }
                            if ui.add(Button::new("-").variant(Variant::Ghost).size(Size::Sm)).clicked() { watchlist.chain_far_num_strikes = watchlist.chain_far_num_strikes.saturating_sub(1).max(1); }
                            ui.add(MonospaceCode::new(&format!("{}", watchlist.chain_far_num_strikes)).size_px(font_xs()).color(t.dim));
                            if ui.add(Button::new("+").variant(Variant::Ghost).size(Size::Sm)).clicked() { watchlist.chain_far_num_strikes += 1; }
                            NmfToggle::new(&mut watchlist.chain_far_nmf).theme(t).show(ui);
                            let fr_icon_far = if watchlist.chain_far_frozen { Icon::PAUSE } else { Icon::PLAY };
                            let r = ui.add(Button::icon(fr_icon_far).variant(Variant::MutedIcon).active(watchlist.chain_far_frozen).size(Size::Sm).placement(IconPlacement::PanelHeader));
                            Tooltip::new("Freeze strikes").show(ui, &r, t);
                            if r.clicked() {
                                watchlist.chain_far_frozen = !watchlist.chain_far_frozen;
                                if !watchlist.chain_far_frozen { watchlist.chain_far_offset = 0; }
                            }
                            if watchlist.chain_far_frozen {
                                let r = ui.add(Button::icon(Icon::ARROW_FAT_UP).variant(Variant::MutedIcon).size(Size::Sm).placement(IconPlacement::PanelHeader));
                                Tooltip::new("Shift strikes up").show(ui, &r, t);
                                if r.clicked() { watchlist.chain_far_offset += 1; }
                                let r = ui.add(Button::icon(Icon::ARROW_FAT_DOWN).variant(Variant::MutedIcon).size(Size::Sm).placement(IconPlacement::PanelHeader));
                                Tooltip::new("Shift strikes down").show(ui, &r, t);
                                if r.clicked() { watchlist.chain_far_offset -= 1; }
                            }
                        });
                        let ns_f = watchlist.chain_far_num_strikes;
                        let off_f = watchlist.chain_far_offset;
                        let sm_f = watchlist.chain_far_strike_mode;
                        let nmf_f = watchlist.chain_far_nmf;
                        render_block(ui, far_dte, &calls_f, &puts_f, &sym, chain_price, &mut watchlist.saved_options, sel, scroll_w, ns_f, off_f, sm_f, nmf_f);
                    });
                    // Normal click: just open option chart (no watchlist add).
                    // Split off the OCC so we can populate both pending fields.
                    if let Some((sym, strike, is_call, exp, occ)) = clicked_contract.take() {
                        open_option_chart = Some((sym, strike, is_call, exp));
                        clicked_occ_ticker = Some(occ);
                    }
                    // Select mode / shift+click: add to watchlist + persist
                    if let Some((ref sym, strike, is_call, ref expiry, bid, ask)) = watchlist_add.take() {
                        watchlist.add_option_to_watchlist(sym, strike, is_call, expiry, bid, ask);
                        watchlist.persist();
                    }
                }

                // ── HEAT TAB ─────────────────────────────────────────────────
                WatchlistTab::Heat => {
                    let active_sym = panes[ap].symbol.clone();
                    let mut pending_symbol: Option<String> = None;
                    super::heat_panel::render_heat_panel(ui, watchlist, t, &active_sym, &mut pending_symbol);
                    if let Some(sym) = pending_symbol {
                        panes[ap].pending_symbol_change = Some(sym);
                    }
                }

                // Scanner now lives as a watchlist tab (moved from its own panel).
                WatchlistTab::Scan => {
                    let panel_w = ui.available_width();
                    let mut pending_symbol: Option<String> = None;
                    super::scanner_panel::draw_content(ui, watchlist, panes, ap, t, &mut pending_symbol, panel_w);
                    if let Some(sym) = pending_symbol {
                        if let Some(p) = panes.get_mut(ap) { p.pending_symbol_change = Some(sym); }
                    }
                }


            }

            // ── Handle option chart opening (from any tab) ──
            // Delegate to deferred handler which always replaces active pane.
            // Both pending fields are set in lockstep so the consumer takes
            // the real OCC ticker, not a synthesized guess.
            if let Some(info) = open_option_chart {
                watchlist.pending_opt_chart = Some(crate::chart_renderer::gpu::PendingOptionChart { symbol: info.0, strike: info.1, is_call: info.2, expiry: info.3 });
                watchlist.pending_opt_chart_contract = clicked_occ_ticker.take();
            }
        }); // close SidePanelShell::tabs body closure

    // Write the active tab back to its owner (instance store or base panel).
    if let Some(it) = instance_tab { *it = wl_tab_to_u8(active_tab); }
    else { watchlist.tab = active_tab; }
    // Close: a duplicate's X removes the instance (rail handles it); the base's
    // X closes the panel.
    if shell_resp.close_clicked {
        if is_spawn { spawn_close = true; }
        else { watchlist.update_sidebar_state(|s| s.watchlist_open = false); }
    }
}
spawn_close
}
