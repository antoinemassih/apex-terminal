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

    egui::SidePanel::right("design_mode_panel")
        .min_width(300.0)
        .max_width(450.0)
        .default_width(340.0)
        .frame(
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(18, 18, 24))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 42, 54)))
                .inner_margin(8.0),
        )
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("DESIGN MODE")
                    .monospace()
                    .size(14.0)
                    .strong()
                    .color(egui::Color32::from_rgb(203, 166, 247)),
            );
            ui.label(
                egui::RichText::new("Every change affects ALL widgets globally")
                    .monospace()
                    .size(9.0)
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
            ui.add_space(8.0);
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // ── Global font scale ──────────────────────────────────────
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("GLOBAL SCALE")
                        .monospace()
                        .size(11.0)
                        .strong()
                        .color(egui::Color32::from_rgb(166, 227, 161)),
                );
                ui.add_space(4.0);

                let mut pixels_per_point = ctx.pixels_per_point();
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("UI Scale")
                            .monospace()
                            .size(9.0)
                            .color(egui::Color32::from_rgb(170, 170, 180)),
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

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(6.0);

                // ── egui's built-in full style editor ─────────────────────
                // Controls: spacing, colors, rounding, stroke widths,
                // button padding, interaction sizes, text styles, visuals —
                // everything egui renders.
                ui.label(
                    egui::RichText::new("EGUI STYLE EDITOR")
                        .monospace()
                        .size(11.0)
                        .strong()
                        .color(egui::Color32::from_rgb(166, 227, 161)),
                );
                ui.label(
                    egui::RichText::new(
                        "Controls spacing, colors, rounding, padding for all widgets",
                    )
                    .monospace()
                    .size(9.0)
                    .color(egui::Color32::from_rgb(120, 120, 130)),
                );
                ui.add_space(6.0);

                let mut style = (*ctx.style()).clone();
                style.ui(ui);
                ctx.set_style(style);
            });
        });
}
