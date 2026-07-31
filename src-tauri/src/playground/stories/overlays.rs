//! Overlays story — Modal, Sheet, Popover, HoverCard, Tooltip, ContextMenu.
//!
//! Widgets shown (~12 instances):
//!   Modal: trigger button + modal dialog (1 trigger + 1 overlay)
//!   Sheet: trigger button + right-side sheet (1 trigger + 1 overlay)
//!   Popover: trigger button + content popover (1 trigger + 1 overlay)
//!   ContextMenu: trigger button + context menu (1 trigger + 1 overlay)
//!   Tooltip: anchor label + tooltip on hover (1)
//!   HoverCard: anchor label + hover card on hover (1)
//!
//! For overlays that require interaction, a trigger button is shown. Open-state
//! is stored in PlaygroundState::overlays. Tooltip and HoverCard describe how
//! to interact in adjacent labels.

use _scaffold_lib::ui_kit::widgets::{
    Button, ConfirmDialog, ConfirmOutcome, ConfirmTone,
    ContextMenu, HoverCard, Modal, Popover, Size, Tooltip,
    Variant,
    theme::ComponentTheme,
};
use _scaffold_lib::ui_kit::widgets::context_menu::{MenuItem as CxMenuItem, DangerMenuItem, MenuDivider};
use egui::Ui;

use super::super::stories::OverlaysState;

// Modal::theme<T: ComponentTheme> requires a concrete type, so this story
// is generic over T (mirrors how panels.rs works).
pub fn show<T: ComponentTheme>(ui: &mut Ui, theme: &T, state: &mut OverlaysState) {
    let ctx = ui.ctx().clone();

    // ── Modal ─────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Modal");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let open_btn = Button::new("Open Modal")
            .variant(Variant::Secondary)
            .size(Size::Sm)
            .show(ui, theme);
        if open_btn.clicked() {
            state.modal_open = true;
        }
        ui.label(
            egui::RichText::new("→ click to open a centered dialog")
                .color(theme.dim())
                .size(10.0),
        );
    });

    if state.modal_open {
        let resp = Modal::new("Example Modal")
            .ctx(&ctx)
            .theme(theme)
            .size(egui::vec2(360.0, 0.0))
            .show(|ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(
                        "This is an example modal dialog.\nClick X or press Escape to close.",
                    )
                    .color(theme.dim())
                    .size(11.0),
                );
                ui.add_space(12.0);
                if Button::new("Close")
                    .variant(Variant::Secondary)
                    .size(Size::Sm)
                    .show(ui, theme)
                    .clicked()
                {
                    // signal via ModalResponse.closed handled below
                }
            });
        if resp.closed {
            state.modal_open = false;
        }
    }

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Popover ───────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Popover");
    ui.add_space(6.0);

    let popover_resp = Button::new("Open Popover")
        .variant(Variant::Secondary)
        .size(Size::Sm)
        .show(ui, theme);
    if popover_resp.clicked() {
        state.popover_open = !state.popover_open;
    }

    let anchor_rect = popover_resp.rect;
    Popover::new()
        .open(&mut state.popover_open)
        .anchor(anchor_rect)
        .close_on_click_outside(true)
        .id("playground_popover")
        .show(ui, theme, |ui| {
            ui.label(
                egui::RichText::new("Popover content")
                    .color(theme.text())
                    .size(11.0),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Click outside to dismiss.")
                    .color(theme.dim())
                    .size(10.0),
            );
        });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── ContextMenu ───────────────────────────────────────────────────────────
    story_heading(ui, theme, "ContextMenu");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let ctx_btn = Button::new("Open Context Menu")
            .variant(Variant::Secondary)
            .size(Size::Sm)
            .show(ui, theme);
        if ctx_btn.clicked() {
            state.context_menu_open = true;
        }
        ui.label(
            egui::RichText::new("→ click to open at button position")
                .color(theme.dim())
                .size(10.0),
        );

        if state.context_menu_open {
            let pos = ctx_btn.rect.left_bottom();
            ContextMenu::new(theme)
                .id("playground_ctx_menu")
                .pos(pos)
                .show(ui, |menu| {
                    if menu.add(CxMenuItem::new("Buy")).clicked() {
                        state.context_menu_open = false;
                    }
                    if menu.add(CxMenuItem::new("Sell")).clicked() {
                        state.context_menu_open = false;
                    }
                    menu.add(MenuDivider);
                    if menu.add(DangerMenuItem::new("Cancel All Orders")).clicked() {
                        state.context_menu_open = false;
                    }
                });
            // Dismiss on click outside via pointer check.
            if ui.ctx().input(|i| i.pointer.any_pressed()) {
                state.context_menu_open = false;
            }
        }
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Tooltip ───────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Tooltip");
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Hover over the button below to see a Tooltip:")
            .color(theme.dim())
            .size(10.0),
    );
    ui.add_space(4.0);
    let tip_resp = Button::new("Hover me")
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .show(ui, theme);
    Tooltip::new("This is a tooltip card.\nShows on hover after a short delay.")
        .show(ui, &tip_resp, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── HoverCard ─────────────────────────────────────────────────────────────
    story_heading(ui, theme, "HoverCard");
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Hover over the ticker below to see a HoverCard:")
            .color(theme.dim())
            .size(10.0),
    );
    ui.add_space(4.0);
    let anchor_resp = Button::new("AAPL $182.45")
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .show(ui, theme);
    HoverCard::new().show(ui, &anchor_resp, theme, |ui| {
        ui.label(egui::RichText::new("AAPL — Apple Inc.").color(theme.text()).size(11.0).strong());
        ui.label(egui::RichText::new("Last: $182.45  +1.2%").color(theme.dim()).size(10.0));
        ui.label(egui::RichText::new("Vol: 72.4M  Mkt Cap: $2.84T").color(theme.dim()).size(10.0));
    });

    ui.add_space(8.0);

    // ── ConfirmDialog (P6.4) ──────────────────────────────────────────────
    rule(ui, theme);
    story_heading(ui, theme, "ConfirmDialog");
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        if Button::new("Open primary confirm")
            .variant(Variant::Primary)
            .size(Size::Sm)
            .show(ui, theme)
            .clicked()
        {
            state.confirm_dialog_open = true;
        }
        if Button::new("Open danger confirm")
            .variant(Variant::Danger)
            .size(Size::Sm)
            .show(ui, theme)
            .clicked()
        {
            state.confirm_danger_open = true;
        }
        if let Some(outcome) = state.last_confirm_outcome {
            ui.label(egui::RichText::new(format!("last: {outcome}")).color(theme.dim()).size(10.0));
        }
    });

    if state.confirm_dialog_open {
        let resp = ConfirmDialog::new("Save changes?")
            .body("Your unsaved changes will be written to the active workspace.")
            .confirm("Save", ConfirmTone::Primary)
            .show(ui.ctx(), theme);
        match resp.outcome {
            ConfirmOutcome::Confirmed => {
                state.confirm_dialog_open = false;
                state.last_confirm_outcome = Some("confirmed");
            }
            ConfirmOutcome::Cancelled => {
                state.confirm_dialog_open = false;
                state.last_confirm_outcome = Some("cancelled");
            }
            ConfirmOutcome::Open => {}
        }
    }

    if state.confirm_danger_open {
        let resp = ConfirmDialog::new("Delete watchlist?")
            .body("This watchlist and all its symbols will be permanently deleted.")
            .confirm("Delete", ConfirmTone::Danger)
            .show(ui.ctx(), theme);
        match resp.outcome {
            ConfirmOutcome::Confirmed => {
                state.confirm_danger_open = false;
                state.last_confirm_outcome = Some("deleted");
            }
            ConfirmOutcome::Cancelled => {
                state.confirm_danger_open = false;
                state.last_confirm_outcome = Some("cancelled");
            }
            ConfirmOutcome::Open => {}
        }
    }

    ui.add_space(8.0);
}

fn story_heading<T: ComponentTheme>(ui: &mut Ui, theme: &T, title: &str) {
    ui.label(egui::RichText::new(title).color(theme.text()).size(12.0).strong());
}

fn rule<T: ComponentTheme>(ui: &mut Ui, theme: &T) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(
            theme.border().r(), theme.border().g(), theme.border().b(), 80,
        )),
    );
    ui.add_space(12.0);
}
