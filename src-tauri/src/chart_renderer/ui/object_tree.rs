//! Object tree panel — consolidated drawings, indicators, overlays management.
//!
//! Replaces both the old drawing list panel (gpu.rs) and the simple object tree.
//! Opens as a left sidebar via the toolbar button next to Magnet.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Chart, Theme, DrawingAction, drawing_to_db};
use super::super::DrawingKind;
use crate::ui_kit::icons::Icon;

/// Short label for drawing kind (compact for tree rows).
fn kind_short_label(kind: &DrawingKind) -> &'static str {
    match kind {
        DrawingKind::HLine{..} => "HL",
        DrawingKind::TrendLine{..} => "TL",
        DrawingKind::Ray{..} => "RAY",
        DrawingKind::HZone{..} => "ZN",
        DrawingKind::Fibonacci{..} => "FIB",
        DrawingKind::Channel{..} => "CH",
        DrawingKind::FibChannel{..} => "FCH",
        DrawingKind::Pitchfork{..} => "PF",
        DrawingKind::GannFan{..} => "GF",
        DrawingKind::GannBox{..} => "GB",
        DrawingKind::RegressionChannel{..} => "REG",
        DrawingKind::XABCD{..} => "XAB",
        DrawingKind::ElliottWave{..} => "EW",
        DrawingKind::AnchoredVWAP{..} => "AVW",
        DrawingKind::PriceRange{..} => "RNG",
        DrawingKind::RiskReward{..} => "RR",
        DrawingKind::BarMarker{..} => "MK",
        DrawingKind::VerticalLine{..} => "VL",
        DrawingKind::FibExtension{..} => "FX",
        DrawingKind::FibTimeZone{..} => "FT",
        DrawingKind::FibArc{..} => "FA",
        DrawingKind::TextNote{..} => "TX",
    }
}

/// Significance score badge color.
fn sig_color(score: f32) -> egui::Color32 {
    if score >= 7.0 { egui::Color32::from_rgb(224, 85, 96) }       // red — critical
    else if score >= 5.0 { egui::Color32::from_rgb(255, 193, 37) } // gold — strong
    else if score >= 3.0 { egui::Color32::from_rgb(81, 207, 102) } // green — moderate
    else { egui::Color32::from_rgb(120, 120, 120) }                // gray — weak
}

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
if !watchlist.object_tree_open { return; }

egui::SidePanel::left("object_tree_panel")
    .default_width(220.0)
    .min_width(180.0)
    .max_width(320.0)
    .resizable(true)
    .frame(egui::Frame::NONE.fill(t.toolbar_bg)
        .inner_margin(egui::Margin { left: 6, right: 6, top: 6, bottom: 6 })
        .stroke(egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_STRONG))))
    .show(ctx, |ui| {
        let panel_w = ui.available_width();
        ui.set_max_width(panel_w);

        // ── Header ──
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("OBJECTS").monospace().size(11.0).strong().color(t.accent));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if close_button(ui, t.dim) { watchlist.object_tree_open = false; }
            });
        });
        ui.add_space(4.0);

        let chart = &mut panes[ap];
        let sym = chart.symbol.clone();
        let tf = chart.timeframe.clone();

        // ════════════════════════════════════════════════════════════════
        // ── DRAWINGS section ──
        // ════════════════════════════════════════════════════════════════
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("DRAWINGS ({})", chart.drawings.len()))
                .monospace().size(9.0).color(t.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Select all button
                if !chart.drawings.is_empty() {
                    if ui.add(egui::Button::new(
                        egui::RichText::new("All").monospace().size(7.0).color(t.dim))
                        .frame(false)).clicked()
                    {
                        chart.selected_ids = chart.drawings.iter().map(|d| d.id.clone()).collect();
                        chart.selected_id = chart.drawings.first().map(|d| d.id.clone());
                    }
                }
            });
        });

        // ── Bulk actions bar (when >1 selected) ──
        if chart.selected_ids.len() > 1 {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(egui::RichText::new(format!("{} sel", chart.selected_ids.len()))
                    .monospace().size(8.0).color(t.accent));
                // Group assign dropdown
                let groups_snap: Vec<(String, String)> = {
                    let mut gs = vec![("default".into(), "default".into())];
                    for g in &chart.groups { if g.id != "default" { gs.push((g.id.clone(), g.name.clone())); } }
                    gs
                };
                let sel_ids = chart.selected_ids.clone();
                let sym2 = sym.clone(); let tf2 = tf.clone();
                egui::ComboBox::from_id_salt("otree_bulk_grp")
                    .selected_text(egui::RichText::new(Icon::FOLDER).monospace().size(9.0))
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for (gid, gname) in &groups_snap {
                            if ui.selectable_label(false, egui::RichText::new(gname).monospace().size(9.0)).clicked() {
                                for d in &mut chart.drawings {
                                    if sel_ids.contains(&d.id) {
                                        d.group_id = gid.clone();
                                        crate::drawing_db::save(&drawing_to_db(d, &sym2, &tf2));
                                    }
                                }
                            }
                        }
                    });
                // Bulk lock/unlock
                let any_unlocked = chart.drawings.iter().any(|d| sel_ids.contains(&d.id) && !d.locked);
                let lock_icon = if any_unlocked { Icon::LOCK } else { Icon::LOCK_OPEN };
                let lock_tip = if any_unlocked { "Lock selected" } else { "Unlock selected" };
                if icon_btn(ui, lock_icon, t.dim, FONT_SM).on_hover_text(lock_tip).clicked() {
                    let target = any_unlocked;
                    let sym3 = sym.clone(); let tf3 = tf.clone();
                    for d in &mut chart.drawings {
                        if sel_ids.contains(&d.id) {
                            d.locked = target;
                            crate::drawing_db::save(&drawing_to_db(d, &sym3, &tf3));
                        }
                    }
                }
                // Bulk delete
                if icon_btn(ui, Icon::TRASH, egui::Color32::from_rgb(224, 85, 96), FONT_SM).on_hover_text("Delete selected").clicked() {
                    let ids = chart.selected_ids.clone();
                    for id in &ids {
                        if let Some(d) = chart.drawings.iter().find(|d| d.id == *id) {
                            chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                        }
                        crate::drawing_db::remove(id);
                    }
                    chart.drawings.retain(|d| !ids.contains(&d.id));
                    chart.redo_stack.clear();
                    chart.selected_ids.clear();
                    chart.selected_id = None;
                }
            });
        }

        ui.add_space(3.0);

        if chart.drawings.is_empty() {
            ui.label(egui::RichText::new("  No drawings").monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
        } else {
            // Build group order
            let mut groups_order: Vec<String> = vec!["default".into()];
            for g in &chart.groups {
                if g.id != "default" && !groups_order.contains(&g.id) { groups_order.push(g.id.clone()); }
            }
            for d in &chart.drawings {
                if !groups_order.contains(&d.group_id) { groups_order.push(d.group_id.clone()); }
            }

            // Deferred actions (to avoid borrow issues)
            let mut click_id: Option<String> = None;
            let mut shift_click_id: Option<String> = None;
            let mut delete_id: Option<String> = None;
            let mut toggle_lock_id: Option<String> = None;
            let mut toggle_vis_group: Option<String> = None;
            let shift = ui.input(|i| i.modifiers.shift);

            // Snapshot drawing data for rendering (avoids borrow conflicts)
            struct DrawSnap {
                id: String, kind_label: &'static str, color: String, locked: bool,
                group_id: String, sig_score: Option<f32>,
            }
            let draw_snaps: Vec<DrawSnap> = chart.drawings.iter().map(|d| DrawSnap {
                id: d.id.clone(),
                kind_label: kind_short_label(&d.kind),
                color: d.color.clone(),
                locked: d.locked,
                group_id: d.group_id.clone(),
                sig_score: d.significance.as_ref().map(|s| s.score),
            }).collect();

            egui::ScrollArea::vertical()
                .id_salt("otree_drawings_scroll")
                .max_height(ui.available_height() * 0.6)
                .show(ui, |ui| {
                for group_id in &groups_order {
                    let group_draws: Vec<&DrawSnap> = draw_snaps.iter()
                        .filter(|d| d.group_id == *group_id).collect();
                    if group_draws.is_empty() { continue; }

                    let group_name = chart.groups.iter().find(|g| g.id == *group_id)
                        .map_or(group_id.as_str(), |g| g.name.as_str());
                    let group_color = chart.groups.iter().find(|g| g.id == *group_id)
                        .and_then(|g| g.color.as_ref())
                        .map(|c| hex_to_color(c, 1.0));
                    let is_hidden = chart.hidden_groups.contains(group_id);

                    // ── Group header row ──
                    let collapse_id = ui.make_persistent_id(format!("otgrp_{}", group_id));
                    let mut collapsed = ui.data_mut(|d| *d.get_persisted_mut_or(collapse_id, false));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        // Collapse arrow
                        let arrow = if collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                        if ui.add(egui::Button::new(
                            egui::RichText::new(arrow).size(8.0).color(t.dim))
                            .frame(false).min_size(egui::vec2(14.0, 16.0))).clicked()
                        {
                            collapsed = !collapsed;
                            ui.data_mut(|d| d.insert_persisted(collapse_id, collapsed));
                        }
                        // Group color dot (if set)
                        if let Some(gc) = group_color {
                            let (dot_r, _) = ui.allocate_exact_size(egui::vec2(8.0, 16.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot_r.center(), 3.0, gc);
                        }
                        // Group name + count
                        let header_col = if is_hidden { t.dim.gamma_multiply(0.3) } else { t.dim };
                        ui.label(egui::RichText::new(format!("{} ({})", group_name, group_draws.len()))
                            .monospace().size(8.0).color(header_col));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            // Eye toggle for group
                            let vis_icon = if is_hidden { Icon::EYE_SLASH } else { Icon::EYE };
                            let vis_col = if is_hidden { t.dim.gamma_multiply(0.3) } else { t.dim };
                            if ui.add(egui::Button::new(
                                egui::RichText::new(vis_icon).size(7.0).color(vis_col))
                                .frame(false).min_size(egui::vec2(14.0, 14.0))).clicked()
                            {
                                toggle_vis_group = Some(group_id.clone());
                            }
                        });
                    });

                    // ── Drawing rows (if group not collapsed) ──
                    if !collapsed {
                        for ds in &group_draws {
                            let is_sel = chart.selected_ids.contains(&ds.id);
                            let dc = hex_to_color(&ds.color, if is_hidden { 0.3 } else { 1.0 });
                            let bg = if is_sel { color_alpha(t.accent, ALPHA_TINT) } else { egui::Color32::TRANSPARENT };

                            let row_resp = ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.add_space(14.0); // indent under group

                                // Color dot
                                let (dot_r, _) = ui.allocate_exact_size(egui::vec2(10.0, 18.0), egui::Sense::hover());
                                ui.painter().circle_filled(dot_r.center(), 3.5, dc);

                                // Kind label
                                let label_col = if is_sel { egui::Color32::WHITE }
                                    else if is_hidden { t.dim.gamma_multiply(0.3) }
                                    else { egui::Color32::from_white_alpha(170) };
                                let row_btn = ui.add(egui::Button::new(
                                    egui::RichText::new(ds.kind_label).monospace().size(9.0).color(label_col))
                                    .fill(bg).min_size(egui::vec2(30.0, 18.0)).corner_radius(RADIUS_SM));

                                // Significance badge
                                if let Some(score) = ds.sig_score {
                                    let sc = sig_color(score);
                                    let (badge_r, _) = ui.allocate_exact_size(egui::vec2(8.0, 18.0), egui::Sense::hover());
                                    ui.painter().circle_filled(badge_r.center(), 3.0, sc);
                                }

                                // Lock icon
                                if ds.locked {
                                    ui.label(egui::RichText::new(Icon::LOCK).size(7.0).color(t.dim.gamma_multiply(0.6)));
                                }

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing.x = 1.0;
                                    // Delete button (always visible when hovered or selected)
                                    if row_btn.hovered() || is_sel {
                                        if ui.add(egui::Button::new(
                                            egui::RichText::new(Icon::TRASH).size(7.0).color(egui::Color32::from_rgb(224, 85, 96)))
                                            .frame(false).min_size(egui::vec2(14.0, 14.0))).clicked()
                                        {
                                            delete_id = Some(ds.id.clone());
                                        }
                                    }
                                    // Eye toggle for individual drawing (via group)
                                    if row_btn.hovered() || is_sel {
                                        let eye_icon = if is_hidden { Icon::EYE_SLASH } else { Icon::EYE };
                                        let eye_col = if is_hidden { t.dim.gamma_multiply(0.3) } else { t.dim };
                                        if ui.add(egui::Button::new(
                                            egui::RichText::new(eye_icon).size(7.0).color(eye_col))
                                            .frame(false).min_size(egui::vec2(14.0, 14.0))).clicked()
                                        {
                                            toggle_vis_group = Some(ds.group_id.clone());
                                        }
                                    }
                                });

                                row_btn
                            }).inner;

                            // Click handling
                            if row_resp.clicked() {
                                if shift { shift_click_id = Some(ds.id.clone()); }
                                else { click_id = Some(ds.id.clone()); }
                            }

                            // Context menu (right-click)
                            row_resp.context_menu(|ui| {
                                if ui.button(egui::RichText::new(format!("  {} Lock/Unlock", Icon::LOCK)).monospace().size(9.0)).clicked() {
                                    toggle_lock_id = Some(ds.id.clone());
                                    ui.close_menu();
                                }
                                if ui.button(egui::RichText::new(format!("  {} Delete", Icon::TRASH)).monospace().size(9.0)).clicked() {
                                    delete_id = Some(ds.id.clone());
                                    ui.close_menu();
                                }
                                // Move to group submenu
                                ui.menu_button(egui::RichText::new(format!("  {} Move to Group", Icon::FOLDER)).monospace().size(9.0), |ui| {
                                    let mut gs = vec![("default".to_string(), "default".to_string())];
                                    for g in &chart.groups { if g.id != "default" { gs.push((g.id.clone(), g.name.clone())); } }
                                    for (gid, gname) in &gs {
                                        if ui.button(egui::RichText::new(gname).monospace().size(9.0)).clicked() {
                                            if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == ds.id) {
                                                d.group_id = gid.clone();
                                                crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                                            }
                                            ui.close_menu();
                                        }
                                    }
                                });
                            });
                        }
                    }
                    ui.add_space(2.0);
                }
            }); // end ScrollArea

            // ── Process deferred actions ──
            if let Some(id) = click_id {
                chart.selected_id = Some(id.clone());
                chart.selected_ids = vec![id];
            }
            if let Some(id) = shift_click_id {
                if chart.selected_ids.contains(&id) { chart.selected_ids.retain(|x| x != &id); }
                else { chart.selected_ids.push(id.clone()); chart.selected_id = Some(id); }
            }
            if let Some(id) = delete_id {
                if let Some(d) = chart.drawings.iter().find(|d| d.id == id) {
                    chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                }
                crate::drawing_db::remove(&id);
                chart.drawings.retain(|d| d.id != id);
                chart.redo_stack.clear();
                chart.selected_ids.retain(|x| x != &id);
                if chart.selected_id.as_deref() == Some(&id) { chart.selected_id = None; }
            }
            if let Some(id) = toggle_lock_id {
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == id) {
                    d.locked = !d.locked;
                    crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                }
            }
            if let Some(gid) = toggle_vis_group {
                if chart.hidden_groups.contains(&gid) {
                    chart.hidden_groups.retain(|x| x != &gid);
                } else {
                    chart.hidden_groups.push(gid);
                }
            }
        }

        ui.add_space(6.0);
        ui.add(egui::Separator::default().spacing(2.0));
        ui.add_space(4.0);

        // ════════════════════════════════════════════════════════════════
        // ── INDICATORS section ──
        // ════════════════════════════════════════════════════════════════
        ui.label(egui::RichText::new("INDICATORS").monospace().size(9.0).color(t.dim));
        ui.add_space(2.0);
        if chart.indicators.is_empty() {
            ui.label(egui::RichText::new("  No indicators").monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
        } else {
            let mut edit_ind: Option<u32> = None;
            for ind in chart.indicators.iter_mut() {
                let ic = hex_to_color(&ind.color, 1.0);
                ui.horizontal(|ui| {
                    ui.set_height(18.0);
                    ui.spacing_mut().item_spacing.x = 2.0;
                    // Color dot
                    ui.painter().circle_filled(
                        egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, ic);
                    ui.add_space(12.0);
                    // Name + period
                    let label = format!("{} {}", ind.kind.label(), ind.period);
                    let label_resp = ui.label(egui::RichText::new(&label).monospace().size(8.0).color(
                        if ind.visible { egui::Color32::from_white_alpha(180) } else { t.dim.gamma_multiply(0.3) }));
                    if label_resp.clicked() { edit_ind = Some(ind.id); }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 1.0;
                        // Eye toggle
                        let eye_icon = if ind.visible { Icon::EYE } else { Icon::EYE_SLASH };
                        let eye_col = if ind.visible { t.dim } else { t.dim.gamma_multiply(0.3) };
                        if ui.add(egui::Button::new(
                            egui::RichText::new(eye_icon).size(8.0).color(eye_col))
                            .frame(false).min_size(egui::vec2(16.0, 16.0))).clicked() {
                            ind.visible = !ind.visible;
                        }
                    });
                });
            }
            if let Some(id) = edit_ind {
                chart.editing_indicator = Some(id);
            }
        }

        ui.add_space(6.0);
        ui.add(egui::Separator::default().spacing(2.0));
        ui.add_space(4.0);

        // ════════════════════════════════════════════════════════════════
        // ── OVERLAYS section ──
        // ════════════════════════════════════════════════════════════════
        ui.label(egui::RichText::new("OVERLAYS").monospace().size(9.0).color(t.dim));
        ui.add_space(2.0);
        if chart.symbol_overlays.is_empty() {
            ui.label(egui::RichText::new("  No overlays").monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
        } else {
            let mut del_ov: Option<usize> = None;
            let mut toggle_ov: Option<usize> = None;
            // Snapshot data for iteration to avoid borrow conflicts
            let ov_snap: Vec<(String, String, bool)> = chart.symbol_overlays.iter()
                .map(|ov| (ov.symbol.clone(), ov.color.clone(), ov.visible)).collect();
            for (oi, (sym_ov, color, vis)) in ov_snap.iter().enumerate() {
                let oc = hex_to_color(color, 1.0);
                ui.horizontal(|ui| {
                    ui.set_height(18.0);
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.painter().circle_filled(
                        egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, oc);
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(sym_ov).monospace().size(8.0).color(
                        if *vis { egui::Color32::from_white_alpha(180) } else { t.dim.gamma_multiply(0.3) }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 1.0;
                        if ui.add(egui::Button::new(
                            egui::RichText::new(Icon::TRASH).size(8.0).color(egui::Color32::from_rgb(224, 85, 96)))
                            .frame(false).min_size(egui::vec2(16.0, 16.0))).clicked() {
                            del_ov = Some(oi);
                        }
                        let eye_icon = if *vis { Icon::EYE } else { Icon::EYE_SLASH };
                        let eye_col = if *vis { t.dim } else { t.dim.gamma_multiply(0.3) };
                        if ui.add(egui::Button::new(
                            egui::RichText::new(eye_icon).size(8.0).color(eye_col))
                            .frame(false).min_size(egui::vec2(16.0, 16.0))).clicked() {
                            toggle_ov = Some(oi);
                        }
                    });
                });
            }
            if let Some(idx) = toggle_ov {
                chart.symbol_overlays[idx].visible = !chart.symbol_overlays[idx].visible;
            }
            if let Some(idx) = del_ov {
                chart.symbol_overlays.remove(idx);
            }
        }
    });
}
