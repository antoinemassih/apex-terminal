//! Left workspace navigation rail — a persistent, collapsible vertical strip
//! for switching between, creating, saving, and managing workspaces.
//!
//! Backed entirely by the existing workspace file system (`workspaces/*.json`)
//! via `gpu::{list_workspaces, save_workspace, rename_workspace,
//! delete_workspace}` — this module is purely a new UI over it.
//!
//! - **Collapsed** (default): a narrow column of square buttons showing each
//!   workspace's initials; hovering shows the full name as a tooltip.
//! - **Expanded**: a wider menu listing full workspace names with an active
//!   marker, plus create / save / save-as controls in the footer.
//!
//! Expand/collapse is driven by `watchlist.workspace.nav_expanded`, toggled
//! from the button immediately left of the Paper/Live `$` toggle in the top
//! toolbar. Right-click a workspace for Rename / Delete.

use crate::chart_renderer::gpu::{
    Chart, Watchlist, Layout, Theme,
    list_workspaces, save_workspace, rename_workspace, delete_workspace,
};
use crate::ui_kit::widgets::{Button as KitButton, MenuItem, Tooltip, Input};
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::sx::Tone;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use crate::chart_renderer::ui::style::{
    tint, font_xs, font_sm, gap_xs, gap_sm, stroke_std, radius_sm, contrast_fg,
};

const COLLAPSED_W: f32 = 52.0;
const EXPANDED_W:  f32 = 216.0;
const ROW_H:       f32 = 30.0;
const CHIP:        f32 = 34.0; // collapsed initials button (square)

/// Up to two uppercase initials drawn from word boundaries in the name.
fn initials(name: &str) -> String {
    let mut out = String::new();
    for word in name.split(|c: char| c == ' ' || c == '_' || c == '-').filter(|w| !w.is_empty()) {
        if let Some(c) = word.chars().next() { out.push(c.to_ascii_uppercase()); }
        if out.chars().count() >= 2 { break; }
    }
    if out.is_empty() {
        out = name.chars().take(2).collect::<String>().to_uppercase();
    }
    out
}

/// Render the workspace rail as a left `SidePanel`. Must be called between the
/// toolbar and the `CentralPanel` so egui reserves the left strip.
pub(crate) fn render_workspace_rail(
    ctx: &egui::Context,
    panes: &[Chart],
    layout: Layout,
    watchlist: &mut Watchlist,
    t: &Theme,
) {
    let expanded = watchlist.workspace.nav_expanded;
    let width = if expanded { EXPANDED_W } else { COLLAPSED_W };

    let names = list_workspaces();
    let active = watchlist.workspace.active.clone();
    let active_is_saved = names.contains(&active);
    let renaming = watchlist.workspace.rename_target.clone();

    // ── Deferred actions (applied after the panel closure releases borrows) ──
    let mut switch_to:    Option<String> = None;
    let mut delete_req:   Option<String> = None;
    let mut rename_start: Option<String> = None;
    let mut commit_rename = false;
    let mut cancel_rename = false;
    let mut save_active   = false;
    let mut save_as       = false;
    let mut new_blank     = false;

    let frame = egui::Frame::NONE
        .fill(t.toolbar_bg)
        .inner_margin(egui::Margin::ZERO)
        .stroke(egui::Stroke::new(stroke_std(), tint(t, Tone::Border, crate::ui_kit::style::alpha_dim())));

    egui::SidePanel::left("workspace_nav_rail")
        .exact_width(width)
        .resizable(false)
        .frame(frame)
        .show(ctx, |ui| {
            // ── Footer (create / save) pinned to the bottom ──
            egui::TopBottomPanel::bottom("workspace_rail_footer")
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(gap_xs() as i8)))
                .show_separator_line(false)
                .show_inside(ui, |ui| {
                    ui.add_space(gap_xs());
                    // Hairline above the footer.
                    let y = ui.cursor().min.y;
                    ui.painter().hline(ui.min_rect().x_range(), y,
                        egui::Stroke::new(stroke_std(), tint(t, Tone::Border, crate::ui_kit::style::alpha_subtle())));
                    ui.add_space(gap_sm());

                    if expanded {
                        // Rename row takes over the footer while active.
                        if let Some(ref target) = renaming {
                            ui.label(egui::RichText::new(format!("Rename “{target}”"))
                                .monospace().size(font_xs()).color(tint(t, Tone::Dim, crate::ui_kit::style::alpha_solid())));
                            ui.add_space(gap_xs());
                            Input::new(&mut watchlist.workspace.rename_buf)
                                .placeholder("New name…")
                                .min_width(EXPANDED_W - 24.0)
                                .size(KitSize::Sm)
                                .show(ui, t);
                            ui.add_space(gap_xs());
                            ui.horizontal(|ui| {
                                if KitButton::new("Rename").variant(KitVariant::Primary)
                                    .size(KitSize::Sm).tint(t.accent).show(ui, t).clicked() {
                                    commit_rename = true;
                                }
                                if KitButton::new("Cancel").variant(KitVariant::Ghost)
                                    .size(KitSize::Sm).show(ui, t).clicked() {
                                    cancel_rename = true;
                                }
                            });
                        } else {
                            // New blank workspace.
                            if KitButton::new("New workspace").leading_icon(Icon::PLUS)
                                .variant(KitVariant::Ghost).size(KitSize::Sm)
                                .min_size(egui::vec2(EXPANDED_W - 16.0, ROW_H))
                                .show(ui, t).clicked() {
                                new_blank = true;
                            }
                            ui.add_space(gap_xs());
                            // Save the active workspace (overwrite, or first save
                            // of an Untitled — both write `<active>.json`).
                            let save_enabled = !active.trim().is_empty();
                            if KitButton::new("Save workspace").leading_icon(Icon::CHECK)
                                .variant(KitVariant::Ghost).size(KitSize::Sm)
                                .fg(if save_enabled { t.accent } else { tint(t, Tone::Dim, crate::ui_kit::style::alpha_dense()) })
                                .min_size(egui::vec2(EXPANDED_W - 16.0, ROW_H))
                                .show(ui, t).clicked() && save_enabled {
                                save_active = true;
                            }
                            ui.add_space(gap_sm());
                            // Save-as (name + button).
                            Input::new(&mut watchlist.workspace.save_name)
                                .placeholder("Save as…")
                                .min_width(EXPANDED_W - 24.0)
                                .size(KitSize::Sm)
                                .show(ui, t);
                            ui.add_space(gap_xs());
                            if !watchlist.workspace.save_name.trim().is_empty()
                                && KitButton::new("Save As").variant(KitVariant::Primary)
                                    .size(KitSize::Sm).tint(t.accent)
                                    .min_size(egui::vec2(EXPANDED_W - 16.0, ROW_H))
                                    .show(ui, t).clicked() {
                                save_as = true;
                            }
                        }
                    } else {
                        // Collapsed: just a "+" to add a new blank workspace.
                        ui.vertical_centered(|ui| {
                            let resp = KitButton::icon(Icon::PLUS).variant(KitVariant::Ghost)
                                .size(KitSize::Sm).min_size(egui::vec2(CHIP, CHIP)).show(ui, t);
                            Tooltip::new("New workspace").show(ui, &resp, t);
                            if resp.clicked() { new_blank = true; }
                        });
                    }
                    ui.add_space(gap_xs());
                });

            // ── Workspace list (fills remaining space above the footer) ──
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(gap_xs() as i8)))
                .show_inside(ui, |ui| {
                    if expanded {
                        ui.add_space(gap_xs());
                        ui.label(egui::RichText::new("WORKSPACES")
                            .monospace().size(font_xs()).color(tint(t, Tone::Dim, crate::ui_kit::style::alpha_solid())));
                        ui.add_space(gap_xs());
                        // Active workspace header — surfaces the current workspace,
                        // including an unsaved "Untitled" that has no list row yet.
                        if !active.trim().is_empty() {
                            ui.horizontal(|ui| {
                                ui.label(TextStyle::Caption.as_rich_cascading(Icon::CIRCLE_FILL, t.accent));
                                ui.label(TextStyle::BodySm.as_rich_cascading(active.as_str(), t.text).strong());
                                if !active_is_saved {
                                    ui.label(egui::RichText::new("·unsaved").monospace().size(font_xs()).color(t.warn));
                                }
                            });
                            ui.add_space(gap_xs());
                        }
                    } else {
                        ui.add_space(gap_sm());
                    }

                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if expanded {
                            for name in &names {
                                let is_active = *name == active;
                                let resp = expanded_row(ui, t, name, is_active);
                                if resp.clicked() && !is_active { switch_to = Some(name.clone()); }
                                resp.context_menu(|ui| {
                                    if MenuItem::new("Rename").icon(Icon::PENCIL_LINE).show(ui, t).clicked() {
                                        rename_start = Some(name.clone()); ui.close_menu();
                                    }
                                    if MenuItem::new("Delete").icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                                        delete_req = Some(name.clone()); ui.close_menu();
                                    }
                                });
                            }
                            if names.is_empty() {
                                ui.add_space(gap_sm());
                                ui.label(egui::RichText::new("No saved workspaces")
                                    .monospace().size(font_xs()).color(tint(t, Tone::Dim, crate::ui_kit::style::alpha_scrim())));
                            }
                        } else {
                            ui.vertical_centered(|ui| {
                                ui.spacing_mut().item_spacing.y = gap_xs();
                                for name in &names {
                                    let is_active = *name == active;
                                    let resp = collapsed_chip(ui, t, name, is_active);
                                    Tooltip::new(name.clone()).show(ui, &resp, t);
                                    if resp.clicked() && !is_active { switch_to = Some(name.clone()); }
                                    resp.context_menu(|ui| {
                                        if MenuItem::new("Rename").icon(Icon::PENCIL_LINE).show(ui, t).clicked() {
                                            rename_start = Some(name.clone()); ui.close_menu();
                                        }
                                        if MenuItem::new("Delete").icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                                            delete_req = Some(name.clone()); ui.close_menu();
                                        }
                                    });
                                }
                            });
                        }
                    });
                });
        });

    // ── Apply deferred actions (panes / layout / watchlist are free here) ──
    let autosave_current = |wl: &Watchlist| {
        // Only auto-save real, already-named workspaces. A brand-new unsaved
        // "Untitled" is scratch until the user explicitly saves it.
        if active_is_saved && !active.trim().is_empty() {
            save_workspace(&active, panes, layout, wl);
        }
    };

    if new_blank {
        autosave_current(watchlist);
        watchlist.workspace.pending_new_blank = true;
    }
    if let Some(name) = switch_to {
        autosave_current(watchlist);
        watchlist.workspace.active = name.clone();
        watchlist.workspace.pending_load = Some(name);
    }
    if save_active {
        save_workspace(&active, panes, layout, watchlist);
    }
    if save_as {
        let name = watchlist.workspace.save_name.trim().to_string();
        if !name.is_empty() {
            save_workspace(&name, panes, layout, watchlist);
            watchlist.workspace.active = name;
            watchlist.workspace.save_name.clear();
        }
    }
    if let Some(name) = rename_start {
        watchlist.workspace.rename_buf = name.clone();
        watchlist.workspace.rename_target = Some(name);
        watchlist.workspace.nav_expanded = true; // expand so the rename field shows
    }
    if commit_rename {
        if let Some(old) = watchlist.workspace.rename_target.take() {
            let new = watchlist.workspace.rename_buf.trim().to_string();
            if !new.is_empty() && new != old {
                rename_workspace(&old, &new);
                if watchlist.workspace.active == old { watchlist.workspace.active = new; }
            }
            watchlist.workspace.rename_buf.clear();
        }
    }
    if cancel_rename {
        watchlist.workspace.rename_target = None;
        watchlist.workspace.rename_buf.clear();
    }
    if let Some(name) = delete_req {
        // U0-6: defer to a confirmation modal — deleting a saved workspace is
        // hard to undo. The actual delete happens on Confirm below.
        watchlist.workspace.pending_delete = Some(name);
    }

    // ── Delete-workspace confirmation modal (U0-6) ──
    if let Some(name) = watchlist.workspace.pending_delete.clone() {
        use crate::ui_kit::widgets::{ConfirmDialog, ConfirmOutcome, ConfirmTone};
        let resp = ConfirmDialog::new("Delete workspace?")
            .id("confirm_delete_workspace")
            .body(&format!("\u{201c}{name}\u{201d} and its saved layout will be permanently deleted."))
            .confirm("Delete", ConfirmTone::Danger)
            .show(ctx, t);
        match resp.outcome {
            ConfirmOutcome::Confirmed => {
                delete_workspace(&name);
                if watchlist.workspace.active == name { watchlist.workspace.active.clear(); }
                if watchlist.workspace.rename_target.as_deref() == Some(name.as_str()) {
                    watchlist.workspace.rename_target = None;
                }
                watchlist.workspace.pending_delete = None;
            }
            ConfirmOutcome::Cancelled => watchlist.workspace.pending_delete = None,
            ConfirmOutcome::Open => {}
        }
    }
}

/// Expanded list row: active-stripe + tint, initials chip, full name.
fn expanded_row(ui: &mut egui::Ui, t: &Theme, name: &str, is_active: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, ROW_H), egui::Sense::click());
    let hovered = resp.hovered();
    let radius = crate::ui_kit::style::r_sm_cr();

    if is_active {
        ui.painter().rect_filled(rect, radius, tint(t, Tone::Accent, 28));
        // Left accent stripe.
        let stripe = egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height()));
        ui.painter().rect_filled(stripe, egui::CornerRadius::ZERO, t.accent);
    } else if hovered {
        ui.painter().rect_filled(rect, radius, tint(t, Tone::Border, 36));
    }

    // Initials chip.
    let chip = egui::Rect::from_min_size(
        egui::pos2(rect.left() + gap_sm(), rect.center().y - 9.0),
        egui::vec2(crate::ui_kit::style::icon_md(), crate::ui_kit::style::icon_md()));
    ui.painter().rect_filled(chip, crate::ui_kit::style::r_sm_cr(),
        if is_active { t.accent } else { tint(t, Tone::Border, 70) });
    ui.painter().text(chip.center(), egui::Align2::CENTER_CENTER, initials(name),
        crate::ui_kit::style::mono_xs(),
        if is_active { contrast_fg(t.accent) } else { t.text });

    // Name.
    let name_col = if is_active { t.text } else if hovered { t.text } else { tint(t, Tone::Dim, 220) };
    ui.painter().text(
        egui::pos2(chip.right() + gap_sm(), rect.center().y),
        egui::Align2::LEFT_CENTER, name,
        crate::ui_kit::style::prop_at(crate::ui_kit::style::font_sm()), name_col);

    if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp
}

/// Collapsed square chip with initials + active ring.
fn collapsed_chip(ui: &mut egui::Ui, t: &Theme, name: &str, is_active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(CHIP, CHIP), egui::Sense::click());
    let hovered = resp.hovered();
    let radius = crate::ui_kit::style::r_sm_cr();

    let fill = if is_active { t.accent }
        else if hovered { tint(t, Tone::Border, crate::ui_kit::style::alpha_strong()) }
        else { tint(t, Tone::Border, crate::ui_kit::style::alpha_tint()) };
    ui.painter().rect_filled(rect, radius, fill);
    if is_active {
        ui.painter().rect_stroke(rect, radius,
            egui::Stroke::new(crate::ui_kit::style::stroke_bold(), t.accent), egui::StrokeKind::Outside);
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initials(name),
        crate::ui_kit::style::mono_sm(),
        if is_active { contrast_fg(t.accent) } else { t.text });

    if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    resp
}
