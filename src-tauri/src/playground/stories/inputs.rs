//! Inputs story — Input, TextArea, SearchInput, NumberStepper.
//!
//! Widgets shown (~16 instances):
//!   Input: default, placeholder, leading icon, password, disabled, invalid (6)
//!   TextArea: default, disabled (2)
//!   SearchInput: default, full_width (2)
//!   NumberStepper: default, with range/suffix, disabled (3)
//!   Bonus: clearable, warning state (2 extra Input variants)

use _scaffold_lib::ui_kit::widgets::{
    Input, TextArea, SearchInput, NumberStepper, Size,
    theme::ComponentTheme,
};
use _scaffold_lib::ui_kit::icons::Icon;
use egui::Ui;

use super::super::stories::InputsState;

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme, state: &mut InputsState) {
    // ── Input ─────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Input");
    ui.add_space(6.0);

    ui.horizontal_wrapped(|ui| {
        // Default
        Input::new(&mut state.text_buf)
            .placeholder("Default placeholder…")
            .width(200.0)
            .show(ui, theme);

        ui.add_space(8.0);

        // Leading icon
        Input::new(&mut state.text_buf)
            .placeholder("With icon")
            .leading_icon(Icon::MAGNIFYING_GLASS)
            .width(200.0)
            .show(ui, theme);

        ui.add_space(8.0);

        // Password
        let mut pw = "secret".to_string();
        Input::new(&mut pw)
            .placeholder("Password")
            .password(true)
            .width(200.0)
            .show(ui, theme);
    });

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        // Clearable
        Input::new(&mut state.text_buf)
            .placeholder("Clearable")
            .clearable(true)
            .width(200.0)
            .show(ui, theme);

        ui.add_space(8.0);

        // Disabled
        let mut dummy = "Disabled value".to_string();
        Input::new(&mut dummy)
            .disabled(true)
            .width(200.0)
            .show(ui, theme);

        ui.add_space(8.0);

        // Invalid
        let mut invalid_buf = "bad value".to_string();
        Input::new(&mut invalid_buf)
            .invalid(true)
            .placeholder("Invalid")
            .width(200.0)
            .show(ui, theme);

        ui.add_space(8.0);

        // Warning
        let mut warn_buf = "warn value".to_string();
        Input::new(&mut warn_buf)
            .warning(true)
            .placeholder("Warning")
            .width(200.0)
            .show(ui, theme);
    });

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        // Size Xs
        Input::new(&mut state.text_buf)
            .placeholder("Xs")
            .size(Size::Xs)
            .width(100.0)
            .show(ui, theme);

        ui.add_space(6.0);

        // Size Sm
        Input::new(&mut state.text_buf)
            .placeholder("Sm")
            .size(Size::Sm)
            .width(100.0)
            .show(ui, theme);

        ui.add_space(6.0);

        // Size Md
        Input::new(&mut state.text_buf)
            .placeholder("Md")
            .size(Size::Md)
            .width(100.0)
            .show(ui, theme);

        ui.add_space(6.0);

        // Size Lg
        Input::new(&mut state.text_buf)
            .placeholder("Lg")
            .size(Size::Lg)
            .width(100.0)
            .show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── TextArea ──────────────────────────────────────────────────────────────
    story_heading(ui, theme, "TextArea");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        TextArea::new(&mut state.textarea_buf)
            .placeholder("Type some notes here…")
            .min_rows(3)
            .max_rows(8)
            .show(ui, theme);

        ui.add_space(12.0);

        let mut dummy = "Read-only value\nspanning multiple lines.".to_string();
        TextArea::new(&mut dummy)
            .placeholder("Disabled")
            .disabled(true)
            .min_rows(3)
            .show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── SearchInput ───────────────────────────────────────────────────────────
    story_heading(ui, theme, "SearchInput");
    ui.add_space(6.0);
    SearchInput::new(&mut state.search_buf)
        .placeholder("Search symbols…")
        .show(ui, theme);
    ui.add_space(6.0);
    SearchInput::new(&mut state.search_buf)
        .placeholder("Compact Md size")
        .size(Size::Md)
        .show(ui, theme);
    ui.add_space(6.0);
    SearchInput::new(&mut state.search_buf)
        .placeholder("Disabled search")
        .disabled(true)
        .show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── NumberStepper ─────────────────────────────────────────────────────────
    story_heading(ui, theme, "NumberStepper");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        NumberStepper::new(&mut state.num_val)
            .show(ui, theme);

        ui.add_space(12.0);

        NumberStepper::new(&mut state.num_val)
            .range(1.0..=100.0)
            .step(0.5)
            .suffix("px")
            .show(ui, theme);

        ui.add_space(12.0);

        let mut disabled_val = 10.0_f64;
        NumberStepper::new(&mut disabled_val)
            .disabled(true)
            .suffix("%")
            .show(ui, theme);

        ui.add_space(12.0);

        NumberStepper::new(&mut state.num_val)
            .decimals(2)
            .prefix("$")
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
