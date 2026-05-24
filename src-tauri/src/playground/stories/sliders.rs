//! Sliders story — Slider, RangeSlider, OpacityPicker.
//!
//! Widgets shown (~12 instances):
//!   Slider: 25%, 60%, 90%, disabled (4)
//!   RangeSlider: price range $10-$90, time range 09:00-16:00 (2)
//!   OpacityPicker: accent color, dim color (2)
//!   Bonus: Slider with label + step snap (1), RangeSlider full_width (1)

use _scaffold_lib::ui_kit::widgets::{
    OpacityPicker, RangeSlider, Slider, Variant,
    theme::ComponentTheme,
};
use egui::Ui;

use super::super::stories::SlidersState;

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme, state: &mut SlidersState) {
    // ── Slider ────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Slider");
    ui.add_space(6.0);

    ui.label(egui::RichText::new("25%  /  60%  /  90%  /  Disabled").color(theme.dim()).size(10.0));
    ui.add_space(4.0);

    Slider::new(&mut state.slider_a, 0.0_f32..=100.0)
        .show_value(true)
        .label("Volume")
        .full_width()
        .show(ui, theme);
    ui.add_space(6.0);

    Slider::new(&mut state.slider_b, 0.0_f32..=100.0)
        .show_value(true)
        .label("Balance")
        .variant(Variant::Primary)
        .full_width()
        .show(ui, theme);
    ui.add_space(6.0);

    Slider::new(&mut state.slider_c, 0.0_f32..=100.0)
        .show_value(true)
        .label("Leverage ×")
        .step(5.0)
        .full_width()
        .show(ui, theme);
    ui.add_space(6.0);

    let mut disabled_val = 42.0_f32;
    Slider::new(&mut disabled_val, 0.0_f32..=100.0)
        .show_value(true)
        .label("Disabled")
        .disabled(true)
        .full_width()
        .show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── RangeSlider ───────────────────────────────────────────────────────────
    story_heading(ui, theme, "RangeSlider");
    ui.add_space(6.0);

    ui.label(egui::RichText::new("Price range (step $5)").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    RangeSlider::new(&mut state.price_range, 0.0_f32..=200.0)
        .step(5.0)
        .show_value(true)
        .label("Price Range")
        .full_width()
        .show(ui, theme);

    ui.add_space(10.0);
    ui.label(egui::RichText::new("Time range (minutes from midnight)").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    RangeSlider::new(&mut state.time_range, 0.0_f32..=1440.0)
        .step(30.0)
        .show_value(true)
        .label("Session Window")
        .full_width()
        .show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── OpacityPicker ─────────────────────────────────────────────────────────
    story_heading(ui, theme, "OpacityPicker");
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("6-segment opacity selector — click a segment to pick a level.")
            .color(theme.dim())
            .size(10.0),
    );
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Accent fill").color(theme.dim()).size(10.0));
        ui.add_space(8.0);
        OpacityPicker::new(&mut state.opacity_a).show(ui, theme);
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Second picker").color(theme.dim()).size(10.0));
        ui.add_space(8.0);
        OpacityPicker::new(&mut state.opacity_b)
            .seg_width(12.0)
            .seg_height(14.0)
            .show(ui, theme);
    });

    ui.add_space(8.0);
}

fn story_heading(ui: &mut Ui, theme: &dyn ComponentTheme, title: &str) {
    ui.label(egui::RichText::new(title).color(theme.text()).size(12.0).strong());
}

fn rule(ui: &mut Ui, theme: &dyn ComponentTheme) {
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
