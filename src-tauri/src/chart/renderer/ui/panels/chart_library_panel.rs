//! Chart Library side panel — a dedicated front-end over the existing
//! named-workspace persistence (`gpu::save_workspace` / `list_workspaces` /
//! `pending_workspace_load`). A "chart" here is a saved pane-area layout
//! (arrangement + per-pane symbol/timeframe/indicators + split ratios).
//!
//! This panel does NOT introduce a new store — it reads/writes the same
//! `workspaces/*.json` files the toolbar workspace dropdown already uses.
//! Drawings are not stored here; they live in their own per-symbol Postgres
//! store and reload automatically when a restored pane's symbol loads.
//!
//! Two tabs:
//!   - Library   — your saved charts (save current / open / delete).
//!   - Community — Phase-1 stub; the seam for a future homelab sharing API.

use egui;
use super::super::style::*;
use crate::chart_renderer::gpu::{
    Chart, Layout, Theme, Watchlist,
    list_workspaces, save_workspace, delete_workspace, pane_tabs_header_h,
};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width, RailSlot};
use crate::ui_kit::widgets::{Button, Input, PanelEmpty, PanelListRow, PanelSection, Size, Tooltip};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::icons::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
enum ChartLibTab {
    #[default]
    Library,
    Community,
}

/// Rail registration — Chart Library ("Charts") is a standard `SidePanelShell::tabs`
/// panel hosted by the rail (same header / borders / split toggle as the rest).
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "chart_library",
    is_open: |w| w.charts_library_open,
    render: |cx, slot| { draw(cx.ctx, cx.watchlist, cx.panes, cx.layout, cx.t, Some(slot), None); },
};

fn lib_tab_to_u8(t: ChartLibTab) -> u8 { match t { ChartLibTab::Library => 0, ChartLibTab::Community => 1 } }
fn lib_tab_from_u8(v: u8) -> ChartLibTab { match v { 1 => ChartLibTab::Community, _ => ChartLibTab::Library } }

/// Draw the Chart Library side panel. `panes` is read-only (snapshot on save).
/// `instance_tab` = `Some` for a rail duplicate; returns its close-X click then.
pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &[Chart],
    layout: Layout,
    t: &Theme,
    slot: Option<RailSlot>,
    instance_tab: Option<&mut u8>,
) -> bool {
    let is_spawn = instance_tab.is_some();
    let mut spawn_close = false;
    if !is_spawn && !watchlist.charts_library_open { return false; }

    // Read-only inputs resolved before the body borrows `watchlist`.
    let active_ws = watchlist.active_workspace.clone();
    let names = list_workspaces();
    let pane_h = pane_tabs_header_h(watchlist);
    let title_font = watchlist.pane_header_size.title_font();

    // Tab selection persists across frames in egui temp memory (no Watchlist
    // field needed for a transient UI toggle).
    let tab_id = egui::Id::new("chart_library_tab");
    let mut tab: ChartLibTab = match instance_tab.as_deref() {
        Some(v) => lib_tab_from_u8(*v),
        None => ctx.data_mut(|d| d.get_temp(tab_id).unwrap_or_default()),
    };

    // Intents captured inside the body, acted on after (clean &mut access).
    let mut apply: Option<String> = None;
    let mut delete: Option<String> = None;
    let mut save_now = false;

    // Tab glyphs are stripped by SidePanelShellTabs today, so pass None.
    let tabs = [
        (ChartLibTab::Library,   "LIBRARY",   None),
        (ChartLibTab::Community, "COMMUNITY", None),
    ];

    let shell_id = if is_spawn { "chart_library_inst" } else { "chart_library" };
    let resp = SidePanelShell::tabs(shell_id, &mut tab, &tabs)
        .width(Width::Narrow)
        .pane_metrics(pane_h, title_font)
        .rail_slot(slot)
        .on_tab_secondary(|ui, tab| {
            if crate::ui_kit::widgets::MenuItem::new("Open as new instance").show(ui, t).clicked() {
                super::right_rail::request_spawn("chart_library", lib_tab_to_u8(tab));
                ui.close_menu();
            }
        })
        .show(ctx, t, |ui, t, active| {
            match active {
                ChartLibTab::Library => {
                    // Save the current pane area as a new chart.
                    PanelSection::new("SAVE CURRENT").show(ui, t, |ui, t| {
                        ui.horizontal(|ui| {
                            Input::new(&mut watchlist.workspace_save_name)
                                .placeholder("Chart name…")
                                .min_width(150.0)
                                .size(Size::Sm)
                                .show(ui, t);
                            let can_save = !watchlist.workspace_save_name.trim().is_empty();
                            if can_save
                                && Button::small_action("Save").tint(t.accent).show(ui, t).clicked()
                            {
                                save_now = true;
                            }
                        });
                    });

                    PanelSection::new("MY CHARTS")
                        .count(names.len())
                        .show(ui, t, |ui, t| {
                            if names.is_empty() {
                                PanelEmpty::new("No saved charts")
                                    .glyph(Icon::CHART_LINE)
                                    .hint("Name a chart above and press Save")
                                    .show(ui, t);
                                return;
                            }
                            egui::ScrollArea::vertical()
                                .id_salt("chart_library_scroll")
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    for name in &names {
                                        let id_salt = format!("chartlib_{name}");
                                        let is_active = *name == active_ws;
                                        let name_owned = name.clone();
                                        let accent = t.accent;

                                        let open = std::cell::Cell::new(false);
                                        let del = std::cell::Cell::new(false);
                                        let open_ref = &open;
                                        let del_ref = &del;

                                        PanelListRow::new(&id_salt)
                                            .leading(move |ui, _t| {
                                                ui.label(
                                                    egui::RichText::new(Icon::CHART_BAR)
                                                        .font(egui::FontId::proportional(font_md()))
                                                        .color(accent),
                                                );
                                            })
                                            .primary(name)
                                            .secondary(if is_active { "Active" } else { "" })
                                            .trailing(move |ui, t| {
                                                let r = Button::close()
                                                    .placement(IconPlacement::ListRow)
                                                    .show(ui, t);
                                                Tooltip::new("Delete chart").show(ui, &r, t);
                                                if r.clicked() { del_ref.set(true); }
                                                if Button::small_action("Open").tint(accent).show(ui, t).clicked() {
                                                    open_ref.set(true);
                                                }
                                            })
                                            .dense(false)
                                            .show(ui, t);

                                        if open.get() { apply = Some(name_owned.clone()); }
                                        if del.get()  { delete = Some(name_owned); }
                                    }
                                });
                        });
                }
                ChartLibTab::Community => {
                    PanelSection::new("COMMUNITY").show(ui, t, |ui, t| {
                        PanelEmpty::new("Community charts coming soon")
                            .glyph(Icon::FOLDER)
                            .hint("Browse and share charts (planned)")
                            .show(ui, t);
                        // TODO(community): wire to the homelab sharing API here —
                        // list/download shared charts, upload the active chart.
                        // Backend lives separately (K3s service); this is the seam.
                    });
                }
            }
        });

    if let Some(it) = instance_tab { *it = lib_tab_to_u8(tab); }
    else { ctx.data_mut(|d| d.insert_temp(tab_id, tab)); }
    if resp.close_clicked {
        if is_spawn { spawn_close = true; }
        else { watchlist.charts_library_open = false; }
    }

    // ── Act on captured intents (full &mut access here) ──────────────────────
    if save_now {
        let name = watchlist.workspace_save_name.trim().to_string();
        if !name.is_empty() {
            save_workspace(&name, panes, layout, watchlist);
            watchlist.active_workspace = name;
            watchlist.workspace_save_name.clear();
        }
    }
    if let Some(name) = apply {
        watchlist.active_workspace = name.clone();
        watchlist.pending_workspace_load = Some(name);
    }
    if let Some(name) = delete {
        delete_workspace(&name);
        if watchlist.active_workspace == name {
            watchlist.active_workspace.clear();
        }
    }
    spawn_close
}
