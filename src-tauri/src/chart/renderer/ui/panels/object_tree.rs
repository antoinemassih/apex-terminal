//! Object tree panel — consolidated drawings, indicators, overlays management.
//!
//! Migrated to the canonical ui_kit side-panel primitives (boss #2 of 4):
//!   - `SidePanelShell` for outer chrome (was hand-rolled SidePanel + PanelFrame +
//!     PanelHeaderWithClose)
//!   - `ui_kit::PanelSection` for the four top-level sections (DRAWINGS,
//!     INDICATORS, OVERLAYS, WIDGETS)
//!   - `ui_kit::PanelSubSection` for collapsible drawing groups (was hand-rolled
//!     caret + group-header pattern, kept in `ui.data_mut` persistent state).
//!     Also used as the wrapper for the INDICATORS / OVERLAYS groups previously
//!     rendered via the `ui_kit::Tree` widget.
//!   - `ui_kit::PanelListRow` for every per-item row (drawings / indicators /
//!     overlays / widgets) — leading color swatch + primary label + trailing
//!     visibility / lock / delete affordances.
//!   - `ui_kit::PanelEmpty` for the empty states.
//!
//! Tree-widget decision: the previous code used `ui_kit::Tree` only for the
//! INDICATORS / OVERLAYS roots. The new `PanelSubSection` + `PanelListRow`
//! combination matches the canonical design system (UPPERCASE titles, stripe +
//! tint selected state, mandatory bottom hairline) so we MIGRATED OFF `Tree`
//! here. The `Tree` widget itself is unchanged and may still be used by other
//! callers — flagged in the report for follow-up.
//!
//! Drawing data structures, persistence (`drawing_db::save` / `remove`), and
//! the undo/redo stacks are untouched.

use egui;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Chart, Theme, DrawingAction, drawing_to_db, pane_tabs_header_h};
use super::super::super::DrawingKind;
use crate::ui_kit::widgets::{
    Button, PanelEmpty, PanelListRow, PanelSection, PanelSubSection, SidePanelShell, Side,
    Tooltip, TrailingBtn, TrailingTone, Width,
};
use crate::ui_kit::widgets::tokens::{Variant, Size as KitSize};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::widgets::MenuItem;
use crate::ui_kit::icons::Icon;

/// Short type key for type-level opacity mapping.
fn kind_type_key(kind: &DrawingKind) -> &'static str { kind_short_label(kind) }

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

/// Significance score badge color. Takes the active theme so the colours
/// follow the user's selection.
fn sig_color(score: f32, t: &Theme) -> egui::Color32 {
    if score >= 7.0 { t.bear }
    else if score >= 5.0 { t.warn }
    else if score >= 3.0 { t.bull }
    else { t.dim }
}

/// Thin adapter: delegates to `OpacityPicker` widget, returning `Some(new_opacity)` on click.
fn opacity_picker(ui: &mut egui::Ui, current: f32, _id_salt: &str) -> Option<f32> {
    use crate::ui_kit::widgets::OpacityPicker;
    use crate::ui_kit::widgets::theme::active_theme;
    let mut value = current;
    let resp = OpacityPicker::new(&mut value).show(ui, &active_theme(ui.ctx()));
    if resp.changed() { Some(value) } else { None }
}

/// Paint a small color swatch in the leading slot.
fn paint_swatch(ui: &mut egui::Ui, color: egui::Color32) {
    let sz = 10.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(sz, sz), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, color);
}

/// Read-or-create a persistent collapse bool keyed by id_salt.
fn read_persisted_bool(ui: &egui::Ui, id_salt: &str, default: bool) -> bool {
    let id = ui.make_persistent_id(("otree_collapse", id_salt));
    ui.data_mut(|d| *d.get_persisted_mut_or(id, default))
}

fn write_persisted_bool(ui: &egui::Ui, id_salt: &str, value: bool) {
    let id = ui.make_persistent_id(("otree_collapse", id_salt));
    ui.data_mut(|d| d.insert_persisted(id, value));
}

// ─── Public entry ───────────────────────────────────────────────────────────

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
    if !watchlist.object_tree_open || panes.is_empty() { return; }
    let ap = ap.min(panes.len() - 1);

    // Pre-resolve pane metrics so the body closure can take `&mut watchlist`
    // freely without a borrow conflict (see CLAUDE.md "Shell API contract").
    let pane_h = pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let resp = SidePanelShell::new("object_tree", "OBJECTS")
        .side(Side::Left)
        .width(Width::Narrow)
        .pane_metrics(pane_h, pane_font)
        .show(ctx, t, |ui, t| {
            draw_body(ui, watchlist, panes, ap, t);
        });
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.object_tree_open = false); }
}

fn draw_body(
    ui: &mut egui::Ui,
    _watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    let chart = &mut panes[ap];
    let sym = super::super::super::gpu::drawing_persist_key(chart);
    let tf = chart.timeframe.clone();

    // ═════════════════════════════════════════════════════════════════════
    // ── DRAWINGS section ──
    // ═════════════════════════════════════════════════════════════════════
    PanelSection::new("DRAWINGS")
        .count(chart.drawings.len())
        .show(ui, t, |ui, t| {
            draw_drawings_section(ui, chart, &sym, &tf, t);
        });

    ui.add_space(gap_xs());

    // ═════════════════════════════════════════════════════════════════════
    // ── INDICATORS section ──
    // ═════════════════════════════════════════════════════════════════════
    PanelSection::new("INDICATORS")
        .count(chart.indicators.len())
        .show(ui, t, |ui, t| {
            draw_indicators_section(ui, chart, t);
        });

    ui.add_space(gap_xs());

    // ═════════════════════════════════════════════════════════════════════
    // ── OVERLAYS section ──
    // ═════════════════════════════════════════════════════════════════════
    PanelSection::new("OVERLAYS")
        .count(chart.symbol_overlays.len())
        .show(ui, t, |ui, t| {
            draw_overlays_section(ui, chart, t);
        });

    ui.add_space(gap_xs());

    // ═════════════════════════════════════════════════════════════════════
    // ── WIDGETS section ──
    // ═════════════════════════════════════════════════════════════════════
    PanelSection::new("WIDGETS")
        .count(chart.chart_widgets.len())
        .show(ui, t, |ui, t| {
            draw_widgets_section(ui, chart, t);
        });
}

// ─── DRAWINGS section body ──────────────────────────────────────────────────

fn draw_drawings_section(ui: &mut egui::Ui, chart: &mut Chart, sym: &str, tf: &str, t: &Theme) {
    // Top toolbar (Select All / per-type fade / fade-all) — kept inline,
    // visually compact above the group list.
    if !chart.drawings.is_empty() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if Button::new("All").variant(Variant::Secondary).simple_treatment(true).fg(t.dim)
                .show(ui, t).clicked()
            {
                chart.selected_ids = chart.drawings.iter().map(|d| d.id.clone()).collect();
                chart.selected_id = chart.drawings.first().map(|d| d.id.clone());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Global "fade all drawings"
                let avg = chart.drawings.iter().map(|d| d.opacity).sum::<f32>() / chart.drawings.len() as f32;
                if let Some(op) = opacity_picker(ui, avg, "all_drawings") {
                    for d in chart.drawings.iter_mut() {
                        d.opacity = op;
                        crate::drawing_db::save(&drawing_to_db(d, sym, tf));
                    }
                }
                // Per-type fade menu
                let sym2 = sym.to_string();
                let tf2 = tf.to_string();
                Button::menu("type").show_menu(ui, t, |ui| {
                    let mut type_keys: Vec<&'static str> = chart.drawings.iter()
                        .map(|d| kind_type_key(&d.kind)).collect();
                    type_keys.sort(); type_keys.dedup();
                    for key in type_keys {
                        let count = chart.drawings.iter().filter(|d| kind_type_key(&d.kind) == key).count();
                        let cur = chart.drawings.iter().find(|d| kind_type_key(&d.kind) == key)
                            .map(|d| d.opacity).unwrap_or(1.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{key} ({count})"))
                                .monospace().size(font_sm()).color(t.text));
                            if let Some(op) = opacity_picker(ui, cur, &format!("type_{key}")) {
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
            });
        });
    }

    // Bulk actions bar (when >1 selected)
    if chart.selected_ids.len() > 1 {
        ui.add_space(gap_xs());
        draw_bulk_actions(ui, chart, sym, tf, t);
    }

    ui.add_space(gap_xs());

    if chart.drawings.is_empty() {
        PanelEmpty::new("No drawings")
            .hint("Use the drawing tools to add lines, zones, fibs…")
            .glyph(Icon::PENCIL_LINE)
            .show(ui, t);
        return;
    }

    // Build group order
    let mut groups_order: Vec<String> = vec!["default".into()];
    for g in &chart.groups {
        if g.id != "default" && !groups_order.contains(&g.id) { groups_order.push(g.id.clone()); }
    }
    for d in &chart.drawings {
        if !groups_order.contains(&d.group_id) { groups_order.push(d.group_id.clone()); }
    }

    // Deferred actions (avoid borrow issues inside per-row closures)
    let mut click_id: Option<String> = None;
    let mut shift_click_id: Option<String> = None;
    let mut delete_id: Option<String> = None;
    let mut toggle_lock_id: Option<String> = None;
    let mut toggle_vis_group: Option<String> = None;
    let mut group_op_set: Option<(String, f32)> = None;
    let mut move_drawing_to_group: Option<(String, String)> = None;
    let mut per_drawing_op_set: Option<(String, f32)> = None;
    let shift = ui.input(|i| i.modifiers.shift);

    // Snapshot drawing data for rendering (avoids borrow conflicts)
    struct DrawSnap {
        id: String, kind_label: &'static str,
        /// User label (UX-3 Fix 4) — pre-filled from drawing_kind_default_label.
        user_label: Option<String>,
        color: String, locked: bool,
        group_id: String, sig_score: Option<f32>,
    }
    let draw_snaps: Vec<DrawSnap> = chart.drawings.iter().map(|d| DrawSnap {
        id: d.id.clone(),
        kind_label: kind_short_label(&d.kind),
        user_label: d.label.clone(),
        color: d.color.clone(),
        locked: d.locked,
        group_id: d.group_id.clone(),
        sig_score: d.significance.as_ref().map(|s| s.score),
    }).collect();

    egui::ScrollArea::vertical()
        .id_salt("otree_drawings_scroll")
        .max_height(ui.available_height().min(360.0))
        .show(ui, |ui| {
        for group_id in &groups_order {
            let group_draws: Vec<&DrawSnap> = draw_snaps.iter()
                .filter(|d| d.group_id == *group_id).collect();
            if group_draws.is_empty() { continue; }

            let group_name = chart.groups.iter().find(|g| g.id == *group_id)
                .map_or(group_id.as_str(), |g| g.name.as_str()).to_string();
            let group_color = chart.groups.iter().find(|g| g.id == *group_id)
                .and_then(|g| g.color.as_ref())
                .map(|c| hex_to_color(c, 1.0));
            let is_hidden = chart.hidden_groups.contains(group_id);

            // Persistent collapse state for this group, exposed to PanelSubSection.
            let collapse_key = format!("grp_{}", group_id);
            let mut expanded = !read_persisted_bool(ui, &collapse_key, false);
            let prev_expanded = expanded;

            let group_id_for_sub = group_id.clone();
            let sub_title = group_name.to_uppercase();

            PanelSubSection::new(&collapse_key, &sub_title)
                .count(group_draws.len())
                .expanded(&mut expanded)
                .show(ui, t, |ui, t| {
                    // ── Group-level controls row (color dot + visibility + opacity) ──
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if let Some(gc) = group_color {
                            paint_swatch(ui, gc);
                        }
                        let vis_icon = if is_hidden { Icon::EYE_SLASH } else { Icon::EYE };
                        let r = Button::icon(vis_icon)
                            .variant(Variant::Ghost)
                            .glyph_color(if is_hidden { color_very_dim(t.dim) } else { t.dim })
                            .size(KitSize::Xs)
                            .placement(IconPlacement::PanelHeader)
                            .show(ui, t);
                        Tooltip::new("Show / hide group").show(ui, &r, t);
                        if r.clicked() {
                            toggle_vis_group = Some(group_id_for_sub.clone());
                        }
                        // Group opacity (avg of all)
                        let target_ids: Vec<String> = draw_snaps.iter()
                            .filter(|ds| ds.group_id == group_id_for_sub).map(|ds| ds.id.clone()).collect();
                        if !target_ids.is_empty() {
                            let avg = target_ids.iter().filter_map(|id|
                                chart.drawings.iter().find(|d| d.id == *id).map(|d| d.opacity)
                            ).sum::<f32>() / target_ids.len() as f32;
                            if let Some(op) = opacity_picker(ui, avg, &format!("grp_op_{group_id_for_sub}")) {
                                group_op_set = Some((group_id_for_sub.clone(), op));
                            }
                        }
                    });

                    // Deferred rename state (UX-3 Fix 4)
                    let mut rename_id: Option<String> = None;

                    // ── Drawing rows ──
                    for ds in &group_draws {
                        let is_sel = chart.selected_ids.contains(&ds.id);
                        let dc = hex_to_color(&ds.color, if is_hidden { 0.3 } else { 1.0 });
                        let kind_label = ds.kind_label;
                        let sig_score = ds.sig_score;
                        let locked = ds.locked;
                        let group_id_eye = ds.group_id.clone();
                        let ds_id_del = ds.id.clone();
                        let ds_id_lock = ds.id.clone();
                        let ds_id_rename = ds.id.clone();

                        let id_salt = format!("dr_{}", ds.id);

                        // UX-3 Fix 4: Use the user label if set, otherwise fall back to
                        // the kind short label.  Inline rename: double-click triggers it.
                        let display_label: String = ds.user_label
                            .as_deref()
                            .filter(|l| !l.is_empty())
                            .unwrap_or(kind_label)
                            .to_string();

                        // Inline rename: if this drawing is being renamed, show a
                        // TextEdit instead of the label row.
                        let rename_key = egui::Id::new(("drawing_rename_active", ds.id.as_str()));
                        let is_renaming = ui.data(|d| d.get_temp::<bool>(rename_key).unwrap_or(false));
                        if is_renaming {
                            let buf_key = egui::Id::new(("drawing_rename_buf", ds.id.as_str()));
                            let mut buf: String = ui.data(|d| d.get_temp::<String>(buf_key.clone()).unwrap_or(display_label.clone()));
                            let te_resp = ui.text_edit_singleline(&mut buf);
                            ui.data_mut(|d| d.insert_temp(buf_key.clone(), buf.clone()));
                            if te_resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let new_label = buf.trim().to_string();
                                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == ds_id_rename) {
                                    d.label = Some(if new_label.is_empty() { display_label.clone() } else { new_label });
                                }
                                ui.data_mut(|d| {
                                    d.insert_temp(rename_key, false);
                                    d.remove::<String>(buf_key);
                                });
                            }
                            continue;
                        }

                        let primary_text: String = if let Some(score) = sig_score {
                            format!("{}  · {:.1}", display_label, score)
                        } else {
                            display_label.clone()
                        };
                        let sig_dot_col = sig_score.map(|s| sig_color(s, t));

                        // Trailing icon strip: slice-based form replaces three
                        // Cell<bool> trampolines. Slice order is L→R visual; we
                        // keep EYE leftmost, then LOCK, then TRASH rightmost to
                        // mirror the previous right-to-left layout. The
                        // significance dot moves into the leading slot next to
                        // the swatch — it's a status indicator, not a button,
                        // and trailing_buttons + trailing(closure) are mutually
                        // exclusive.
                        let lock_icon = if locked { Icon::LOCK } else { Icon::LOCK_OPEN };
                        let eye_icon = if is_hidden { Icon::EYE_SLASH } else { Icon::EYE };
                        let btns = [
                            TrailingBtn::icon(eye_icon)
                                .tone(TrailingTone::Muted)
                                .active(!is_hidden)
                                .tooltip("Show / hide group"),
                            TrailingBtn::icon(lock_icon)
                                .tone(TrailingTone::Muted)
                                .active(locked)
                                .tooltip("Lock / unlock"),
                            TrailingBtn::icon(Icon::TRASH)
                                .tone(TrailingTone::Bear)
                                .tooltip("Delete"),
                        ];

                        let row_resp = PanelListRow::new(&id_salt)
                            .selected(is_sel)
                            .leading(move |ui, _t| {
                                paint_swatch(ui, dc);
                                if let Some(sc) = sig_dot_col {
                                    let (badge_r, _) = ui.allocate_exact_size(
                                        egui::vec2(8.0, 18.0), egui::Sense::hover());
                                    ui.painter().circle_filled(badge_r.center(), 3.0, sc);
                                }
                            })
                            .primary(&primary_text)
                            .trailing_buttons(&btns)
                            .show_full(ui, t);

                        match row_resp.trailing_clicked_idx {
                            Some(0) => toggle_vis_group = Some(group_id_eye),
                            Some(1) => toggle_lock_id = Some(ds_id_lock),
                            Some(2) => delete_id = Some(ds_id_del),
                            _ => {}
                        }

                        if row_resp.clicked {
                            if shift { shift_click_id = Some(ds.id.clone()); }
                            else { click_id = Some(ds.id.clone()); }
                        }
                        let row_resp = row_resp.response.expect("PanelListRow::show_full always sets response");

                        // Double-click triggers inline rename (UX-3 Fix 4).
                        if row_resp.double_clicked() {
                            rename_id = Some(ds.id.clone());
                        }

                        // Context menu (right-click)
                        let ds_id_for_menu = ds.id.clone();
                        let ds_id_lock_menu = ds.id.clone();
                        let ds_id_del_menu = ds.id.clone();
                        let ds_id_rename_menu = ds.id.clone();
                        let group_id_for_menu = group_id.clone();
                        row_resp.context_menu(|ui| {
                            // Per-drawing opacity
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Opacity")
                                    .monospace().size(font_sm()).color(t.dim));
                                let cur = chart.drawings.iter().find(|d| d.id == ds_id_for_menu)
                                    .map(|d| d.opacity).unwrap_or(1.0);
                                if let Some(op) = opacity_picker(ui, cur, &format!("drw_{}", ds_id_for_menu)) {
                                    per_drawing_op_set = Some((ds_id_for_menu.clone(), op));
                                }
                            });
                            ui.add(egui::Separator::default().spacing(2.0));
                            // UX-3 Fix 4: inline rename via context menu.
                            if MenuItem::new("Rename").icon(Icon::PENCIL_LINE).show(ui, t).clicked() {
                                rename_id = Some(ds_id_rename_menu);
                                ui.close_menu();
                            }
                            if MenuItem::new("Lock/Unlock").icon(Icon::LOCK).show(ui, t).clicked() {
                                toggle_lock_id = Some(ds_id_lock_menu);
                                ui.close_menu();
                            }
                            if MenuItem::new("Delete").icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                                delete_id = Some(ds_id_del_menu);
                                ui.close_menu();
                            }
                            // Move to group — cascading submenu via MenuItem::show_menu
                            let gs: Vec<(String, String)> = {
                                let mut v = vec![("default".to_string(), "default".to_string())];
                                for g in &chart.groups { if g.id != "default" { v.push((g.id.clone(), g.name.clone())); } }
                                v
                            };
                            let mut chosen_group: Option<String> = None;
                            MenuItem::new("Move to Group").icon(Icon::FOLDER).show_menu(ui, t, |ui| {
                                for (gid, gname) in &gs {
                                    if MenuItem::new(gname.as_str()).show(ui, t).clicked() {
                                        chosen_group = Some(gid.clone());
                                    }
                                }
                            });
                            if let Some(gid) = chosen_group {
                                move_drawing_to_group = Some((group_id_for_menu.clone(), gid));
                                let _ = group_id_for_menu; // ensure capture
                                ui.close_menu();
                            }
                        });
                    }

                    // Process rename activation (UX-3 Fix 4) — must happen
                    // while `ui` is in scope so temp data can be written.
                    if let Some(ref id) = rename_id {
                        let rename_key = egui::Id::new(("drawing_rename_active", id.as_str()));
                        ui.data_mut(|d| d.insert_temp(rename_key, true));
                    }
                });

            // Persist collapse state if it changed.
            if expanded != prev_expanded {
                write_persisted_bool(ui, &collapse_key, !expanded);
            }
        }
    });

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
            crate::drawing_db::save(&drawing_to_db(d, sym, tf));
        }
    }
    if let Some(gid) = toggle_vis_group {
        if chart.hidden_groups.contains(&gid) {
            chart.hidden_groups.retain(|x| x != &gid);
        } else {
            chart.hidden_groups.push(gid);
        }
    }
    if let Some((gid, op)) = group_op_set {
        let target_ids: Vec<String> = draw_snaps.iter()
            .filter(|ds| ds.group_id == gid).map(|ds| ds.id.clone()).collect();
        for d in chart.drawings.iter_mut() {
            if target_ids.contains(&d.id) {
                d.opacity = op;
                crate::drawing_db::save(&drawing_to_db(d, sym, tf));
            }
        }
    }
    if let Some((id, op)) = per_drawing_op_set {
        if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == id) {
            d.opacity = op;
            crate::drawing_db::save(&drawing_to_db(d, sym, tf));
        }
    }
    if let Some((drawing_id, new_gid)) = move_drawing_to_group {
        if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == drawing_id) {
            d.group_id = new_gid;
            crate::drawing_db::save(&drawing_to_db(d, sym, tf));
        }
    }
}

fn draw_bulk_actions(ui: &mut egui::Ui, chart: &mut Chart, sym: &str, tf: &str, t: &Theme) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let sel_label = format!("{} sel", chart.selected_ids.len());
        ui.label(egui::RichText::new(&sel_label)
            .monospace().size(font_sm()).color(t.accent));

        // Group assign dropdown
        let groups_snap: Vec<(String, String)> = {
            let mut gs = vec![("default".into(), "default".into())];
            for g in &chart.groups { if g.id != "default" { gs.push((g.id.clone(), g.name.clone())); } }
            gs
        };
        let sel_ids = chart.selected_ids.clone();
        let sym2 = sym.to_string(); let tf2 = tf.to_string();
        let mut bulk_assign_gid: Option<String> = None;
        egui::ComboBox::from_id_salt("otree_bulk_grp")
            .selected_text(egui::RichText::new(Icon::FOLDER).size(font_lg()))
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
        let r = Button::icon(lock_icon).variant(Variant::Ghost).glyph_color(t.dim).size(KitSize::Sm)
            .placement(IconPlacement::PanelHeader)
            .show(ui, t);
        Tooltip::new(lock_tip).show(ui, &r, t);
        if r.clicked() {
            let target = any_unlocked;
            let sym3 = sym.to_string(); let tf3 = tf.to_string();
            for d in &mut chart.drawings {
                if sel_ids.contains(&d.id) {
                    d.locked = target;
                    crate::drawing_db::save(&drawing_to_db(d, &sym3, &tf3));
                }
            }
        }
        // Bulk opacity
        let avg = chart.drawings.iter()
            .filter(|d| sel_ids.contains(&d.id))
            .map(|d| d.opacity).sum::<f32>() / sel_ids.len().max(1) as f32;
        if let Some(op) = opacity_picker(ui, avg, "bulk_op") {
            let sym4 = sym.to_string(); let tf4 = tf.to_string();
            for d in &mut chart.drawings {
                if sel_ids.contains(&d.id) {
                    d.opacity = op;
                    crate::drawing_db::save(&drawing_to_db(d, &sym4, &tf4));
                }
            }
        }
        // Bulk delete
        let r = Button::icon(Icon::TRASH).variant(Variant::Ghost).glyph_color(t.bear).size(KitSize::Sm)
            .placement(IconPlacement::PanelHeader).tone_destructive()
            .show(ui, t);
        Tooltip::new("Delete selected").show(ui, &r, t);
        if r.clicked() {
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

// ─── INDICATORS section body ────────────────────────────────────────────────

fn draw_indicators_section(ui: &mut egui::Ui, chart: &mut Chart, t: &Theme) {
    if chart.indicators.is_empty() {
        PanelEmpty::new("No indicators")
            .hint("Add via the Indicators panel")
            .show(ui, t);
        return;
    }

    let mut toggle_id: Option<u32> = None;
    let mut edit_id: Option<u32> = None;

    // Snapshot for borrow safety
    let snaps: Vec<(u32, egui::Color32, bool, String)> = chart.indicators.iter()
        .map(|i| (
            i.id,
            hex_to_color(&i.color, 1.0),
            i.visible,
            format!("{} {}", i.kind.label(), i.period),
        ))
        .collect();

    for (id, color, visible, label) in &snaps {
        let want_toggle = std::cell::Cell::new(false);
        let t_ref = &want_toggle;
        let id_salt = format!("ind_{}", id);
        let visible = *visible;
        let color = *color;

        let resp = PanelListRow::new(&id_salt)
            .leading(move |ui, _t: &crate::chart_renderer::gpu::Theme| { paint_swatch(ui, color); })
            .primary(label.as_str())
            .trailing(move |ui, t| {
                let eye = if visible { Icon::EYE } else { Icon::EYE_SLASH };
                let r = Button::icon(eye)
                    .variant(Variant::Ghost)
                    .glyph_color(if visible { t.dim } else { color_very_dim(t.dim) })
                    .size(KitSize::Xs)
                    .placement(IconPlacement::ListRow)
                    .show(ui, t);
                Tooltip::new("Show / hide").show(ui, &r, t);
                if r.clicked() { t_ref.set(true); }
            })
            .show(ui, t);

        if want_toggle.get() { toggle_id = Some(*id); }
        if resp.clicked() { edit_id = Some(*id); }
    }

    if let Some(id) = toggle_id {
        if let Some(ind) = chart.indicators.iter_mut().find(|i| i.id == id) {
            ind.visible = !ind.visible;
        }
    }
    if let Some(id) = edit_id { chart.editing_indicator = Some(id); }
}

// ─── OVERLAYS section body ──────────────────────────────────────────────────

fn draw_overlays_section(ui: &mut egui::Ui, chart: &mut Chart, t: &Theme) {
    if chart.symbol_overlays.is_empty() {
        PanelEmpty::new("No overlays")
            .hint("Add via the Indicators panel")
            .show(ui, t);
        return;
    }

    let mut toggle_idx: Option<usize> = None;
    let mut delete_idx: Option<usize> = None;

    let snaps: Vec<(usize, egui::Color32, bool, String)> = chart.symbol_overlays.iter().enumerate()
        .map(|(i, o)| (i, hex_to_color(&o.color, 1.0), o.visible, o.symbol.clone()))
        .collect();

    for (idx, color, visible, symbol) in &snaps {
        let id_salt = format!("ov_{}", idx);
        let visible = *visible;
        let color = *color;

        let eye_icon = if visible { Icon::EYE } else { Icon::EYE_SLASH };
        let btns = [
            TrailingBtn::icon(Icon::TRASH).tone(TrailingTone::Bear).tooltip("Remove overlay"),
            TrailingBtn::icon(eye_icon).tone(TrailingTone::Muted).active(visible).tooltip("Show / hide"),
        ];
        let resp = PanelListRow::new(&id_salt)
            .leading(move |ui, _t| { paint_swatch(ui, color); })
            .primary(symbol.as_str())
            .trailing_buttons(&btns)
            .show_full(ui, t);

        match resp.trailing_clicked_idx {
            Some(0) => delete_idx = Some(*idx),
            Some(1) => toggle_idx = Some(*idx),
            _ => {}
        }
    }

    if let Some(i) = toggle_idx {
        if i < chart.symbol_overlays.len() {
            chart.symbol_overlays[i].visible = !chart.symbol_overlays[i].visible;
        }
    }
    if let Some(i) = delete_idx {
        if i < chart.symbol_overlays.len() {
            chart.symbol_overlays.remove(i);
        }
    }
}

// ─── WIDGETS section body ───────────────────────────────────────────────────

fn draw_widgets_section(ui: &mut egui::Ui, chart: &mut Chart, t: &Theme) {
    if chart.chart_widgets.is_empty() {
        PanelEmpty::new("No widgets")
            .hint("Add via the Widgets toolbar")
            .show(ui, t);
        return;
    }

    let mut del_w: Option<usize> = None;
    let mut toggle_w: Option<usize> = None;
    let mut op_change: Option<(usize, f32)> = None;

    // Snapshot for borrow safety
    let snaps: Vec<(usize, &'static str, bool, f32, egui::Color32)> = chart.chart_widgets.iter()
        .enumerate()
        .map(|(wi, w)| {
            let dot_col = if w.visible { t.accent } else { color_very_dim(t.dim) };
            (wi, w.kind.label(), w.visible, w.opacity, dot_col)
        })
        .collect();

    for (wi, label, vis, opacity, dot_col) in snaps {
        let want_del = std::cell::Cell::new(false);
        let want_tog = std::cell::Cell::new(false);
        let want_op: std::cell::Cell<Option<f32>> = std::cell::Cell::new(None);
        let d_ref = &want_del;
        let t_ref = &want_tog;
        let o_ref = &want_op;
        let id_salt = format!("w_{}", wi);

        PanelListRow::new(&id_salt)
            .leading(move |ui, _t: &crate::chart_renderer::gpu::Theme| { paint_swatch(ui, dot_col); })
            .primary(label)
            .trailing(move |ui, t| {
                let r = Button::icon(Icon::TRASH)
                    .variant(Variant::Ghost)
                    .glyph_color(t.bear)
                    .size(KitSize::Xs)
                    .placement(IconPlacement::ListRow).tone_destructive()
                    .show(ui, t);
                Tooltip::new("Remove widget").show(ui, &r, t);
                if r.clicked() { d_ref.set(true); }
                let eye = if vis { Icon::EYE } else { Icon::EYE_SLASH };
                let r = Button::icon(eye)
                    .variant(Variant::Ghost)
                    .glyph_color(if vis { t.dim } else { color_very_dim(t.dim) })
                    .size(KitSize::Xs)
                    .placement(IconPlacement::ListRow)
                    .show(ui, t);
                Tooltip::new("Show / hide").show(ui, &r, t);
                if r.clicked() { t_ref.set(true); }
                if let Some(op) = opacity_picker(ui, opacity, &format!("wop_{wi}")) {
                    o_ref.set(Some(op));
                }
            })
            .show(ui, t);

        if want_del.get() { del_w = Some(wi); }
        if want_tog.get() { toggle_w = Some(wi); }
        if let Some(op) = want_op.get() { op_change = Some((wi, op)); }
    }

    if let Some(i) = toggle_w { chart.chart_widgets[i].visible = !chart.chart_widgets[i].visible; }
    if let Some(i) = del_w { chart.chart_widgets.remove(i); }
    if let Some((i, op)) = op_change { chart.chart_widgets[i].opacity = op; }
}

// Unused-import killer: hash_str/DefaultHasher were used by the old Tree
// integration. Keep the helpers around in case future code needs them.
#[allow(dead_code)]
fn hash_str(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
