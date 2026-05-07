//! Object tree panel — consolidated drawings, indicators, overlays management.
//!
//! Replaces both the old drawing list panel (gpu.rs) and the simple object tree.
//! Opens as a left sidebar via the toolbar button next to Magnet.

use egui;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Chart, Theme, DrawingAction, drawing_to_db};
use super::super::super::DrawingKind;
use super::super::widgets::rows::ListRow;
use super::super::widgets::text::MonospaceCode;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant, Size};
use super::super::widgets::context_menu::{MenuItem, DangerMenuItem, Submenu, MenuItemWithIcon, MenuRow as _MenuRow};
use super::super::widgets::frames::SidePanelFrame;
use super::super::widgets::headers::PanelHeaderWithClose;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{Tree, TreeNode, TreeState};

// ─── Tree node for indicators / overlays sections ────────────────────────────
//
// We migrate the flat INDICATORS and OVERLAYS sections to the new Tree widget.
// Each section is a depth-0 root node ("INDICATORS" / "OVERLAYS"); the actual
// indicators / overlays are depth-1 leaves underneath. Drawings are kept on the
// legacy paint path (2-level groups + bulk header actions + complex context
// menus do not map cleanly onto Tree's `item_render` signature).
#[derive(Clone)]
enum ObjectTreeKind {
    IndicatorRoot,
    Indicator(u32),  // indicator.id
    OverlayRoot,
    Overlay(usize),  // index into chart.symbol_overlays
}

struct ObjectTreeItem {
    id: u64,
    depth: usize,
    has_children: bool,
    label: String,
    kind: ObjectTreeKind,
}

impl TreeNode for ObjectTreeItem {
    fn id(&self) -> u64 { self.id }
    fn depth(&self) -> usize { self.depth }
    fn has_children(&self) -> bool { self.has_children }
    fn label(&self) -> &str { &self.label }
}

fn hash_str(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }

// Six discrete opacity levels: fully faded (readable) up to fully opaque.
pub(crate) const OPACITY_LEVELS: [f32; 6] = [0.15, 0.30, 0.50, 0.70, 0.85, 1.0];

/// Find the closest level index for an opacity value.
fn closest_level_idx(op: f32) -> usize {
    let mut best = 0; let mut best_d = f32::MAX;
    for (i, &lv) in OPACITY_LEVELS.iter().enumerate() {
        let d = (lv - op).abs();
        if d < best_d { best_d = d; best = i; }
    }
    best
}

/// Compact 6-segment opacity picker. Returns Some(new_opacity) if user clicked a segment.
fn opacity_picker(ui: &mut egui::Ui, current: f32, accent: egui::Color32, dim: egui::Color32, id_salt: &str) -> Option<f32> {
    let cur_idx = closest_level_idx(current);
    let seg_w = 7.0;
    let seg_h = 10.0;
    let gap = 1.0;
    let total_w = (seg_w + gap) * OPACITY_LEVELS.len() as f32;
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(total_w, seg_h + 2.0),
        egui::Sense::click());
    let painter = ui.painter_at(rect);
    let mut clicked_idx: Option<usize> = None;
    for i in 0..OPACITY_LEVELS.len() {
        let x = rect.min.x + i as f32 * (seg_w + gap);
        let seg_rect = egui::Rect::from_min_size(egui::pos2(x, rect.min.y + 1.0), egui::vec2(seg_w, seg_h));
        let filled = i <= cur_idx;
        let col = if filled {
            let a = OPACITY_LEVELS[i];
            egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), (a * 220.0) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(dim.r(), dim.g(), dim.b(), 40)
        };
        painter.rect_filled(seg_rect, 1.5, col);
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if pos.x >= seg_rect.left() && pos.x <= seg_rect.right() + gap {
                    clicked_idx = Some(i);
                }
            }
        }
    }
    let _ = resp.on_hover_text_at_pointer(format!("Opacity {}%", (OPACITY_LEVELS[cur_idx] * 100.0) as i32));
    let _ = id_salt;
    clicked_idx.map(|i| OPACITY_LEVELS[i])
}

/// Short type key for type-level opacity mapping.
fn kind_type_key(kind: &DrawingKind) -> &'static str {
    kind_short_label(kind)
}

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
    if score >= 7.0 { ft().bear }                // red — critical
    else if score >= 5.0 { ft().warn }            // gold — strong
    else if score >= 3.0 { ft().bull }           // green — moderate
    else { ft().dim }                            // gray — weak
}

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
if !watchlist.object_tree_open { return; }

egui::SidePanel::left("object_tree_panel")
    .default_width(220.0)
    .min_width(180.0)
    .max_width(320.0)
    .resizable(true)
    .frame(SidePanelFrame::new().theme(t).build())
    .show(ctx, |ui| {
        let panel_w = ui.available_width();
        ui.set_max_width(panel_w);

        // ── Header — monospace "OBJECTS" title + close ──
        if PanelHeaderWithClose::new("OBJECTS")
            .title_monospace(true)
            .title_size_px(font_sm())
            .theme(t)
            .show(ui)
        {
            watchlist.object_tree_open = false;
        }
        ui.add_space(4.0);

        let chart = &mut panes[ap];
        let sym = super::super::super::gpu::drawing_persist_key(chart);
        let tf = chart.timeframe.clone();

        // ════════════════════════════════════════════════════════════════
        // ── DRAWINGS section ──
        // ════════════════════════════════════════════════════════════════
        ui.horizontal(|ui| {
            let drawings_hdr = format!("DRAWINGS ({})", chart.drawings.len());
            ui.add(MonospaceCode::new(&drawings_hdr).size_px(font_sm()).color(t.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Select all button
                if !chart.drawings.is_empty() {
                    if ui.add(Button::new("All").variant(Variant::Secondary).simple_treatment(true).fg(t.dim)).clicked() {
                        chart.selected_ids = chart.drawings.iter().map(|d| d.id.clone()).collect();
                        chart.selected_id = chart.drawings.first().map(|d| d.id.clone());
                    }
                }
                // Per-type fade menu
                if !chart.drawings.is_empty() {
                    let sym2 = sym.clone(); let tf2 = tf.clone();
                    ui.menu_button(egui::RichText::new("type").monospace().size(font_xs()).color(t.dim), |ui| {
                        let mut type_keys: Vec<&'static str> = chart.drawings.iter()
                            .map(|d| kind_type_key(&d.kind)).collect();
                        type_keys.sort(); type_keys.dedup();
                        for key in type_keys {
                            let count = chart.drawings.iter().filter(|d| kind_type_key(&d.kind) == key).count();
                            let cur = chart.drawings.iter().find(|d| kind_type_key(&d.kind) == key)
                                .map(|d| d.opacity).unwrap_or(1.0);
                            ui.horizontal(|ui| {
                                let type_entry = format!("{key} ({count})");
                            ui.add(MonospaceCode::new(&type_entry).size_px(font_sm()).color(t.text));
                                if let Some(op) = opacity_picker(ui, cur, t.accent, t.dim, &format!("type_{key}")) {
                                    let key_s = key.to_string();
                                    for d in chart.drawings.iter_mut() {
                                        if kind_type_key(&d.kind) == key_s {
                                            d.opacity = op;
                                            crate::drawing_db::save(&drawing_to_db(d, &sym2, &tf2));
                                        }
                                    }
                                }
                            });
                        }
                    });
                }
                // Global "fade all drawings"
                if !chart.drawings.is_empty() {
                    let avg = chart.drawings.iter().map(|d| d.opacity).sum::<f32>() / chart.drawings.len() as f32;
                    if let Some(op) = opacity_picker(ui, avg, t.accent, t.dim, "all_drawings") {
                        for d in chart.drawings.iter_mut() {
                            d.opacity = op;
                            crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                        }
                    }
                }
            });
        });

        // ── Bulk actions bar (when >1 selected) ──
        if chart.selected_ids.len() > 1 {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let sel_label = format!("{} sel", chart.selected_ids.len());
                ui.add(MonospaceCode::new(&sel_label).size_px(font_sm()).color(t.accent));
                // Group assign dropdown
                let groups_snap: Vec<(String, String)> = {
                    let mut gs = vec![("default".into(), "default".into())];
                    for g in &chart.groups { if g.id != "default" { gs.push((g.id.clone(), g.name.clone())); } }
                    gs
                };
                let sel_ids = chart.selected_ids.clone();
                let sym2 = sym.clone(); let tf2 = tf.clone();
                // DropdownActions requires FnOnce() + 'static but we need &mut chart
                // here, so we stay on the raw ComboBox and collect the picked group-id
                // as a local, then apply the mutation after show().
                let mut bulk_assign_gid: Option<String> = None;
                egui::ComboBox::from_id_salt("otree_bulk_grp")
                    .selected_text(egui::RichText::new(Icon::FOLDER).monospace().size(font_sm()))
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for (gid, gname) in &groups_snap {
                            if ui.selectable_label(false, egui::RichText::new(gname).monospace().size(font_sm())).clicked() {
                                bulk_assign_gid = Some(gid.clone());
                            }
                        }
                    });
                if let Some(gid) = bulk_assign_gid {
                    for d in &mut chart.drawings {
                        if sel_ids.contains(&d.id) {
                            d.group_id = gid.clone();
                            crate::drawing_db::save(&drawing_to_db(d, &sym2, &tf2));
                        }
                    }
                }
                // Bulk lock/unlock
                let any_unlocked = chart.drawings.iter().any(|d| sel_ids.contains(&d.id) && !d.locked);
                let lock_icon = if any_unlocked { Icon::LOCK } else { Icon::LOCK_OPEN };
                let lock_tip = if any_unlocked { "Lock selected" } else { "Unlock selected" };
                if icon_btn(ui, lock_icon, t.dim, FONT_MD).on_hover_text(lock_tip).clicked() {
                    let target = any_unlocked;
                    let sym3 = sym.clone(); let tf3 = tf.clone();
                    for d in &mut chart.drawings {
                        if sel_ids.contains(&d.id) {
                            d.locked = target;
                            crate::drawing_db::save(&drawing_to_db(d, &sym3, &tf3));
                        }
                    }
                }
                // Bulk opacity
                {
                    let avg = chart.drawings.iter()
                        .filter(|d| sel_ids.contains(&d.id))
                        .map(|d| d.opacity).sum::<f32>() / sel_ids.len().max(1) as f32;
                    if let Some(op) = opacity_picker(ui, avg, t.accent, t.dim, "bulk_op") {
                        let sym4 = sym.clone(); let tf4 = tf.clone();
                        for d in &mut chart.drawings {
                            if sel_ids.contains(&d.id) {
                                d.opacity = op;
                                crate::drawing_db::save(&drawing_to_db(d, &sym4, &tf4));
                            }
                        }
                    }
                }
                // Bulk delete
                if icon_btn(ui, Icon::TRASH, t.bear, FONT_MD).on_hover_text("Delete selected").clicked() {
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

        ui.add_space(4.0);

        if chart.drawings.is_empty() {
            ui.add(MonospaceCode::new("  No drawings").size_px(font_sm()).color(t.dim.gamma_multiply(0.5)));
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
                    let group_drawing_ids: Vec<String> = draw_snaps.iter()
                        .filter(|ds| ds.group_id == *group_id).map(|ds| ds.id.clone()).collect();
                    let group_avg_op: Option<f32> = if !group_drawing_ids.is_empty() {
                        Some(chart.drawings.iter()
                            .filter(|d| group_drawing_ids.contains(&d.id))
                            .map(|d| d.opacity).sum::<f32>() / group_drawing_ids.len() as f32)
                    } else { None };
                    let mut new_group_op: Option<f32> = None;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        // Collapse arrow
                        let arrow = if collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                        if ui.add(Button::icon(arrow).variant(Variant::Ghost).glyph_color(t.dim).size(Size::Xs)).clicked()
                        {
                            collapsed = !collapsed;
                            ui.data_mut(|d| d.insert_persisted(collapse_id, collapsed));
                        }
                        // Header label + dot + right-actions via ListRow
                        let header_col = if is_hidden { t.dim.gamma_multiply(0.3) } else { t.dim };
                        let label_text = format!("{} ({})", group_name, group_draws.len());
                        let group_id_for_eye = group_id.clone();
                        let group_id_for_op = group_id.clone();
                        let new_group_op_ref = &mut new_group_op;
                        let mut row = ListRow::new(18.0).theme(t).hover_enabled(false);
                        if let Some(gc) = group_color {
                            row = row.left_painter_circle(gc);
                        }
                        row.body(|ui| {
                            ui.add(MonospaceCode::new(&label_text).size_px(font_sm()).color(header_col));
                        })
                        .right_actions(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let vis_icon = if is_hidden { Icon::EYE_SLASH } else { Icon::EYE };
                            let vis_col = if is_hidden { t.dim.gamma_multiply(0.3) } else { t.dim };
                            // legacy font_2xs() — Size::Xs is closest available
                            if ui.add(Button::icon(vis_icon).variant(Variant::Ghost).glyph_color(vis_col).size(Size::Xs)).clicked() {
                                toggle_vis_group = Some(group_id_for_eye);
                            }
                            if let Some(avg) = group_avg_op {
                                if let Some(op) = opacity_picker(ui, avg, t.accent, t.dim, &format!("grp_op_{group_id_for_op}")) {
                                    *new_group_op_ref = Some(op);
                                }
                            }
                        })
                        .trailing_width(80.0)
                        .show(ui);
                    });
                    let _ = group_drawing_ids;
                    if let Some(op) = new_group_op {
                        let sym_g = sym.clone(); let tf_g = tf.clone();
                        let target_ids: Vec<String> = draw_snaps.iter()
                            .filter(|ds| ds.group_id == *group_id).map(|ds| ds.id.clone()).collect();
                        for d in &mut chart.drawings {
                            if target_ids.contains(&d.id) {
                                d.opacity = op;
                                crate::drawing_db::save(&drawing_to_db(d, &sym_g, &tf_g));
                            }
                        }
                    }

                    // ── Drawing rows (if group not collapsed) ──
                    if !collapsed {
                        for ds in &group_draws {
                            let is_sel = chart.selected_ids.contains(&ds.id);
                            let dc = hex_to_color(&ds.color, if is_hidden { 0.3 } else { 1.0 });
                            let label_col = if is_sel { egui::Color32::WHITE }
                                else if is_hidden { t.dim.gamma_multiply(0.3) }
                                else { egui::Color32::from_white_alpha(170) };
                            let kind_label = ds.kind_label;
                            let sig_score = ds.sig_score;
                            let locked = ds.locked;
                            let ds_id_eye = ds.group_id.clone();
                            let ds_id_del = ds.id.clone();

                            let row_resp = ListRow::new(20.0)
                                .theme(t)
                                .selected(is_sel)
                                .indent(14.0)
                                .left_painter_circle(dc)
                                .body(|ui| {
                                    ui.add(MonospaceCode::new(kind_label).size_px(font_sm()).color(label_col));
                                    if let Some(score) = sig_score {
                                        let sc = sig_color(score);
                                        let (badge_r, _) = ui.allocate_exact_size(egui::vec2(8.0, 18.0), egui::Sense::hover());
                                        ui.painter().circle_filled(badge_r.center(), 3.0, sc);
                                    }
                                    if locked {
                                        ui.add(Button::icon(Icon::LOCK).variant(Variant::Ghost).glyph_color(t.dim.gamma_multiply(0.6)).size(Size::Xs));
                                    }
                                })
                                .right_actions(|ui| {
                                    ui.spacing_mut().item_spacing.x = 1.0;
                                    if ui.add(Button::icon(Icon::TRASH).variant(Variant::Ghost).glyph_color(t.bear).size(Size::Xs)).clicked() {
                                        delete_id = Some(ds_id_del);
                                    }
                                    let eye_icon = if is_hidden { Icon::EYE_SLASH } else { Icon::EYE };
                                    let eye_col = if is_hidden { t.dim.gamma_multiply(0.3) } else { t.dim };
                                    if ui.add(Button::icon(eye_icon).variant(Variant::Ghost).glyph_color(eye_col).size(Size::Xs)).clicked() {
                                        toggle_vis_group = Some(ds_id_eye);
                                    }
                                })
                                .trailing_width(60.0)
                                .show(ui);

                            // Click handling
                            if row_resp.clicked() {
                                if shift { shift_click_id = Some(ds.id.clone()); }
                                else { click_id = Some(ds.id.clone()); }
                            }

                            // Context menu (right-click)
                            let ds_id_for_menu = ds.id.clone();
                            row_resp.context_menu(|ui| {
                                // Per-drawing opacity
                                ui.horizontal(|ui| {
                                    ui.add(MonospaceCode::new("Opacity").size_px(font_sm()).color(t.dim));
                                    let cur = chart.drawings.iter().find(|d| d.id == ds_id_for_menu)
                                        .map(|d| d.opacity).unwrap_or(1.0);
                                    if let Some(op) = opacity_picker(ui, cur, t.accent, t.dim, &format!("drw_{}", ds_id_for_menu)) {
                                        if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == ds_id_for_menu) {
                                            d.opacity = op;
                                            crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                                        }
                                    }
                                });
                                ui.add(egui::Separator::default().spacing(2.0));
                                let lock_label = format!("{} Lock/Unlock", Icon::LOCK);
                                let mt = super::super::widgets::context_menu::MenuTheme::from_theme(t);
                                if MenuItemWithIcon::new("Lock/Unlock", Icon::LOCK).show(ui, &mt).clicked() {
                                    toggle_lock_id = Some(ds.id.clone());
                                    ui.close_menu();
                                }
                                if DangerMenuItem::new("Delete").icon(Icon::TRASH).show(ui, &mt).clicked() {
                                    delete_id = Some(ds.id.clone());
                                    ui.close_menu();
                                }
                                let _ = lock_label;
                                // Move to group — cascading submenu via Submenu widget
                                let gs: Vec<(String, String)> = {
                                    let mut v = vec![("default".to_string(), "default".to_string())];
                                    for g in &chart.groups { if g.id != "default" { v.push((g.id.clone(), g.name.clone())); } }
                                    v
                                };
                                let mut move_to_group: Option<String> = None;
                                Submenu::new(&format!("{} Move to Group", Icon::FOLDER), |menu| {
                                    for (gid, gname) in &gs {
                                        if menu.add(MenuItem::new(gname.as_str())).clicked() {
                                            move_to_group = Some(gid.clone());
                                        }
                                    }
                                }).show(ui, &mt);
                                if let Some(gid) = move_to_group {
                                    if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == ds.id) {
                                        d.group_id = gid;
                                        crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                                    }
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                    ui.add_space(4.0);
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

        ui.add_space(8.0);
        ui.add(egui::Separator::default().spacing(2.0));
        ui.add_space(4.0);

        // ════════════════════════════════════════════════════════════════
        // ── INDICATORS + OVERLAYS sections (migrated to ui_kit Tree) ──
        // ════════════════════════════════════════════════════════════════
        //
        // Build a flat pre-order list with two roots ("INDICATORS",
        // "OVERLAYS") and the indicators / overlays as depth-1 leaves.
        // TreeState is persisted in egui memory keyed off the panel ID.
        let ind_root_id = hash_str("otree::ind_root");
        let ov_root_id = hash_str("otree::ov_root");
        let mut tree_items: Vec<ObjectTreeItem> = Vec::with_capacity(
            2 + chart.indicators.len() + chart.symbol_overlays.len(),
        );
        tree_items.push(ObjectTreeItem {
            id: ind_root_id,
            depth: 0,
            has_children: !chart.indicators.is_empty(),
            label: format!("INDICATORS ({})", chart.indicators.len()),
            kind: ObjectTreeKind::IndicatorRoot,
        });
        for ind in &chart.indicators {
            tree_items.push(ObjectTreeItem {
                id: hash_str(&format!("otree::ind::{}", ind.id)),
                depth: 1,
                has_children: false,
                label: format!("{} {}", ind.kind.label(), ind.period),
                kind: ObjectTreeKind::Indicator(ind.id),
            });
        }
        tree_items.push(ObjectTreeItem {
            id: ov_root_id,
            depth: 0,
            has_children: !chart.symbol_overlays.is_empty(),
            label: format!("OVERLAYS ({})", chart.symbol_overlays.len()),
            kind: ObjectTreeKind::OverlayRoot,
        });
        for (oi, ov) in chart.symbol_overlays.iter().enumerate() {
            tree_items.push(ObjectTreeItem {
                id: hash_str(&format!("otree::ov::{}::{}", oi, ov.symbol)),
                depth: 1,
                has_children: false,
                label: ov.symbol.clone(),
                kind: ObjectTreeKind::Overlay(oi),
            });
        }

        // Per-pane TreeState in egui memory. Roots default to expanded.
        let tree_state_id = ui.make_persistent_id(("otree_state", ap));
        let mut tree_state: TreeState = ui.data_mut(|d| {
            d.get_persisted::<TreeState>(tree_state_id).unwrap_or_else(|| {
                let mut s = TreeState::default();
                s.expand(ind_root_id);
                s.expand(ov_root_id);
                s
            })
        });

        // Snapshot indicator data we need inside the closure (color +
        // visibility) so we don't have to borrow chart.indicators while
        // mutating via deferred actions.
        let ind_snaps: Vec<(u32, egui::Color32, bool)> = chart.indicators.iter()
            .map(|i| (i.id, hex_to_color(&i.color, 1.0), i.visible))
            .collect();
        let ov_snaps: Vec<(usize, egui::Color32, bool)> = chart.symbol_overlays.iter()
            .enumerate()
            .map(|(idx, o)| (idx, hex_to_color(&o.color, 1.0), o.visible))
            .collect();

        // Deferred actions emitted from the per-row item_render closure.
        // RefCell because the closure type required by `Tree::item_render`
        // is `Fn`, not `FnMut`.
        let toggle_ind_id: RefCell<Option<u32>> = RefCell::new(None);
        let edit_ind_id: RefCell<Option<u32>> = RefCell::new(None);
        let toggle_ov_idx: RefCell<Option<usize>> = RefCell::new(None);
        let delete_ov_idx: RefCell<Option<usize>> = RefCell::new(None);

        // Capture theme colors needed inside the closure. Theme isn't Copy
        // and we only need a handful of fields, so we destructure to plain
        // Color32 values that can be moved into the Fn closure.
        let theme_dim = t.dim;
        let theme_bear = t.bear;
        let tree_resp = Tree::new(&mut tree_state, &tree_items)
            .row_height(20.0)
            .indent_size(12.0)
            .show_indent_guides(false)
            .item_render(|ui, _theme, item, _indent_x| {
                match &item.kind {
                    ObjectTreeKind::IndicatorRoot | ObjectTreeKind::OverlayRoot => {
                        ui.add(MonospaceCode::new(&item.label).size_px(font_sm()).color(theme_dim));
                    }
                    ObjectTreeKind::Indicator(ind_id) => {
                        let ind_id = *ind_id;
                        let snap = ind_snaps.iter().find(|s| s.0 == ind_id);
                        let (color, visible) = snap.map(|s| (s.1, s.2)).unwrap_or((theme_dim, true));
                        // colored dot
                        let (dot_r, _) = ui.allocate_exact_size(egui::vec2(8.0, 18.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot_r.center(), 3.0, color);
                        let label_col = if visible { egui::Color32::from_white_alpha(180) }
                            else { theme_dim.gamma_multiply(0.3) };
                        ui.add(MonospaceCode::new(&item.label).size_px(font_sm()).color(label_col));
                        // right-aligned eye toggle
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            let eye_icon = if visible { Icon::EYE } else { Icon::EYE_SLASH };
                            let eye_col = if visible { theme_dim } else { theme_dim.gamma_multiply(0.3) };
                            if ui.add(Button::icon(eye_icon).variant(Variant::Ghost).glyph_color(eye_col).size(Size::Xs)).clicked() {
                                *toggle_ind_id.borrow_mut() = Some(ind_id);
                            }
                        });
                    }
                    ObjectTreeKind::Overlay(oi) => {
                        let oi = *oi;
                        let snap = ov_snaps.iter().find(|s| s.0 == oi);
                        let (color, visible) = snap.map(|s| (s.1, s.2)).unwrap_or((theme_dim, true));
                        let (dot_r, _) = ui.allocate_exact_size(egui::vec2(8.0, 18.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot_r.center(), 3.0, color);
                        let label_col = if visible { egui::Color32::from_white_alpha(180) }
                            else { theme_dim.gamma_multiply(0.3) };
                        ui.add(MonospaceCode::new(&item.label).size_px(font_sm()).color(label_col));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            if ui.add(Button::icon(Icon::TRASH).variant(Variant::Ghost).glyph_color(theme_bear).size(Size::Xs)).clicked() {
                                *delete_ov_idx.borrow_mut() = Some(oi);
                            }
                            let eye_icon = if visible { Icon::EYE } else { Icon::EYE_SLASH };
                            let eye_col = if visible { theme_dim } else { theme_dim.gamma_multiply(0.3) };
                            if ui.add(Button::icon(eye_icon).variant(Variant::Ghost).glyph_color(eye_col).size(Size::Xs)).clicked() {
                                *toggle_ov_idx.borrow_mut() = Some(oi);
                            }
                        });
                    }
                }
            })
            .show(ui, t);

        // Row click on an indicator opens its editor; click on an overlay
        // is a no-op (overlay editing isn't a thing yet — selection only).
        if let Some(clicked) = tree_resp.clicked {
            for it in &tree_items {
                if it.id == clicked {
                    if let ObjectTreeKind::Indicator(id) = it.kind { *edit_ind_id.borrow_mut() = Some(id); }
                    break;
                }
            }
        }

        // Persist tree state.
        ui.data_mut(|d| d.insert_persisted(tree_state_id, tree_state));

        // Apply deferred mutations.
        if let Some(id) = toggle_ind_id.into_inner() {
            if let Some(ind) = chart.indicators.iter_mut().find(|i| i.id == id) {
                ind.visible = !ind.visible;
            }
        }
        if let Some(id) = edit_ind_id.into_inner() {
            chart.editing_indicator = Some(id);
        }
        if let Some(idx) = toggle_ov_idx.into_inner() {
            if idx < chart.symbol_overlays.len() {
                chart.symbol_overlays[idx].visible = !chart.symbol_overlays[idx].visible;
            }
        }
        if let Some(idx) = delete_ov_idx.into_inner() {
            if idx < chart.symbol_overlays.len() {
                chart.symbol_overlays.remove(idx);
            }
        }

        // Empty-state placeholders — Tree itself shows nothing when a root
        // has no children, so we mimic the previous "  No indicators" /
        // "  No overlays" hint inline.
        if chart.indicators.is_empty() {
            ui.add(MonospaceCode::new("  No indicators").size_px(font_sm()).color(t.dim.gamma_multiply(0.5)));
        }
        if chart.symbol_overlays.is_empty() {
            ui.add(MonospaceCode::new("  No overlays").size_px(font_sm()).color(t.dim.gamma_multiply(0.5)));
        }

        ui.add_space(8.0);
        ui.add(egui::Separator::default().spacing(2.0));
        ui.add_space(4.0);

        // ════════════════════════════════════════════════════════════════
        // ── WIDGETS section ──
        // ════════════════════════════════════════════════════════════════
        ui.horizontal(|ui| {
            let widgets_hdr = format!("WIDGETS ({})", chart.chart_widgets.len());
            ui.add(MonospaceCode::new(&widgets_hdr).size_px(font_sm()).color(t.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Global fade for all widgets
                if !chart.chart_widgets.is_empty() {
                    let avg = chart.chart_widgets.iter().map(|w| w.opacity).sum::<f32>()
                        / chart.chart_widgets.len() as f32;
                    if let Some(op) = opacity_picker(ui, avg, t.accent, t.dim, "all_widgets") {
                        for w in chart.chart_widgets.iter_mut() { w.opacity = op; }
                    }
                }
            });
        });
        ui.add_space(4.0);
        if chart.chart_widgets.is_empty() {
            ui.add(MonospaceCode::new("  No widgets").size_px(font_sm()).color(t.dim.gamma_multiply(0.5)));
        } else {
            let mut del_w: Option<usize> = None;
            let mut toggle_w: Option<usize> = None;
            let mut op_change: Option<(usize, f32)> = None;
            for (wi, w) in chart.chart_widgets.iter().enumerate() {
                let label = w.kind.label();
                let vis = w.visible;
                let dot_col = if vis { t.accent } else { t.dim.gamma_multiply(0.3) };
                let label_col = if vis { egui::Color32::from_white_alpha(180) }
                    else { t.dim.gamma_multiply(0.3) };
                let opacity = w.opacity;
                ListRow::new(18.0)
                    .theme(t)
                    .left_painter_circle(dot_col)
                    .body(|ui| {
                        ui.add(MonospaceCode::new(label).size_px(8.5).color(label_col));
                    })
                    .right_actions(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if ui.add(Button::icon(Icon::TRASH).variant(Variant::Ghost).glyph_color(t.bear).size(Size::Xs)).clicked() {
                            del_w = Some(wi);
                        }
                        let eye_icon = if vis { Icon::EYE } else { Icon::EYE_SLASH };
                        let eye_col = if vis { t.dim } else { t.dim.gamma_multiply(0.3) };
                        if ui.add(Button::icon(eye_icon).variant(Variant::Ghost).glyph_color(eye_col).size(Size::Xs)).clicked() {
                            toggle_w = Some(wi);
                        }
                        if let Some(op) = opacity_picker(ui, opacity, t.accent, t.dim, &format!("w_{wi}")) {
                            op_change = Some((wi, op));
                        }
                    })
                    .trailing_width(110.0)
                    .show(ui);
            }
            if let Some(i) = toggle_w { chart.chart_widgets[i].visible = !chart.chart_widgets[i].visible; }
            if let Some(i) = del_w { chart.chart_widgets.remove(i); }
            if let Some((i, op)) = op_change { chart.chart_widgets[i].opacity = op; }
        }
    });
}
