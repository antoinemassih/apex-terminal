//! Compact egui-style-editor side panel.
//!
//! Gated on `Ctrl+Shift+D`. Lets developers tweak the global egui style and
//! UI scale at runtime — every change propagates instantly to all widgets.
//!
//! # Usage
//! ```ignore
//! design_mode_panel::show(ctx);
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use crate::chart_renderer::ui::style::{font_xs, font_sm, font_md_plus, gap_xs, gap_sm, gap_md};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use crate::chart_renderer::theme_impl::active_theme;

static DESIGN_PANEL_OPEN: AtomicBool = AtomicBool::new(false);

/// Toggle + render the design-mode side panel.
///
/// Call once per frame from the top-level `update` / paint function.
/// The toggle shortcut (`Ctrl+Shift+D`) is handled internally so the caller
/// does not need to check for it separately.
pub fn show(ctx: &egui::Context) {
    // Toggle on Ctrl+Shift+D
    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::D)) {
        let was = DESIGN_PANEL_OPEN.load(Ordering::Relaxed);
        DESIGN_PANEL_OPEN.store(!was, Ordering::Relaxed);
    }

    if !DESIGN_PANEL_OPEN.load(Ordering::Relaxed) {
        return;
    }

    let t = active_theme(ctx);
    let resp = SidePanelShell::new("design_mode_panel", "DESIGN MODE")
        .width(Width::Medium)
        .resizable(300.0..=450.0)
        .show(ctx, &t, |ui, t| {
            ui.label(
                egui::RichText::new("Every change affects ALL widgets globally")
                    .monospace()
                    .size(font_xs())
                    .color(t.dim),
            );
            ui.add_space(gap_sm());
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // ── Global font scale ──────────────────────────────────────
                ui.add_space(gap_sm());
                ui.label(
                    egui::RichText::new("GLOBAL SCALE")
                        .monospace()
                        .size(font_sm())
                        .strong()
                        .color(t.bull),
                );
                ui.add_space(gap_xs());

                let mut pixels_per_point = ctx.pixels_per_point();
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("UI Scale")
                            .monospace()
                            .size(font_xs())
                            .color(t.dim),
                    );
                    if ui
                        .add(
                            egui::DragValue::new(&mut pixels_per_point)
                                .range(0.5..=4.0)
                                .speed(0.01)
                                .suffix("x"),
                        )
                        .changed()
                    {
                        ctx.set_pixels_per_point(pixels_per_point);
                    }
                });

                ui.add_space(gap_md());
                ui.separator();
                ui.add_space(gap_sm());

                // ── egui's built-in full style editor ─────────────────────
                // Controls: spacing, colors, rounding, stroke widths,
                // button padding, interaction sizes, text styles, visuals —
                // everything egui renders.
                ui.label(
                    egui::RichText::new("EGUI STYLE EDITOR")
                        .monospace()
                        .size(font_sm())
                        .strong()
                        .color(t.bull),
                );
                ui.label(
                    egui::RichText::new(
                        "Controls spacing, colors, rounding, padding for all widgets",
                    )
                    .monospace()
                    .size(font_xs())
                    .color(t.dim),
                );
                ui.add_space(gap_sm());

                let mut style = (*ctx.style()).clone();
                style.ui(ui);
                ctx.set_style(style);
            });
        });
    if resp.close_clicked {
        DESIGN_PANEL_OPEN.store(false, Ordering::Relaxed);
    }
}
