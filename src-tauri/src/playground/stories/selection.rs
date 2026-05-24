//! Selection story — Checkbox, Radio, Switch, ToggleGroup, SegmentedControl.
//!
//! Widgets shown (~16 instances):
//!   Checkbox: checked, unchecked, indeterminate, disabled (4)
//!   Radio: 3 options, Sm size (3)
//!   Switch: on, off, disabled (3)
//!   ToggleGroup: 3 items (3)
//!   SegmentedControl: connected + separated (2 × 3 segments = 6 renders, count as 2)

use _scaffold_lib::ui_kit::widgets::{
    Checkbox, Radio, Switch, ToggleGroup, SegmentedControl, Size,
    theme::ComponentTheme,
};
use egui::Ui;

use super::super::stories::SelectionState;

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme, state: &mut SelectionState) {
    // ── Checkbox ──────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Checkbox");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Checkbox::new(&mut state.check_a).label("Checked").show(ui, theme);
        ui.add_space(12.0);
        Checkbox::new(&mut state.check_b).label("Unchecked").show(ui, theme);
        ui.add_space(12.0);
        Checkbox::tri(&mut state.tri_state).label("Indeterminate").show(ui, theme);
        ui.add_space(12.0);
        Checkbox::new(&mut state.check_c).label("Disabled").disabled(true).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Radio ─────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Radio");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Radio::new(&mut state.radio_val, 0).label("DAY").show(ui, theme);
        ui.add_space(8.0);
        Radio::new(&mut state.radio_val, 1).label("GTC").show(ui, theme);
        ui.add_space(8.0);
        Radio::new(&mut state.radio_val, 2).label("IOC").show(ui, theme);
        ui.add_space(16.0);
        // Small variant
        Radio::new(&mut state.radio_val, 0).label("DAY Sm").size(Size::Sm).show(ui, theme);
        ui.add_space(8.0);
        Radio::new(&mut state.radio_val, 1).label("GTC Sm").size(Size::Sm).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Switch ────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Switch");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Switch::new(&mut state.switch_a).label("Enabled (on)").show(ui, theme);
        ui.add_space(12.0);
        Switch::new(&mut state.switch_b).label("Enabled (off)").show(ui, theme);
        ui.add_space(12.0);
        let mut dummy = true;
        Switch::new(&mut dummy).label("Disabled on").disabled(true).show(ui, theme);
        ui.add_space(12.0);
        let mut dummy2 = false;
        Switch::new(&mut dummy2).label("Disabled off").disabled(true).show(ui, theme);
        ui.add_space(12.0);
        // Sm size
        Switch::new(&mut state.switch_a).label("Sm").size(Size::Sm).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── ToggleGroup ───────────────────────────────────────────────────────────
    story_heading(ui, theme, "ToggleGroup");
    ui.add_space(6.0);
    const TOGGLE_ITEMS: &[(usize, &str)] = &[(0, "Normal"), (1, "Margin"), (2, "Futures")];
    ui.horizontal(|ui| {
        ToggleGroup::new(&mut state.toggle_val, TOGGLE_ITEMS)
            .size(Size::Sm)
            .show(ui, theme);
        ui.add_space(16.0);
        // Xs
        ToggleGroup::new(&mut state.toggle_val, TOGGLE_ITEMS)
            .size(Size::Xs)
            .show(ui, theme);
        ui.add_space(16.0);
        // Disabled
        ToggleGroup::new(&mut state.toggle_val, TOGGLE_ITEMS)
            .disabled(true)
            .show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── SegmentedControl ──────────────────────────────────────────────────────
    story_heading(ui, theme, "SegmentedControl");
    ui.add_space(6.0);
    const SEG_ITEMS: &[(usize, &str)] = &[(0, "MKT"), (1, "LMT"), (2, "STP")];
    ui.horizontal(|ui| {
        // Connected (default)
        SegmentedControl::new(&mut state.seg_val, SEG_ITEMS)
            .show(ui, theme);
        ui.add_space(16.0);
        // Separated
        SegmentedControl::new(&mut state.seg_val, SEG_ITEMS)
            .connected(false)
            .show(ui, theme);
        ui.add_space(16.0);
        // Sm size
        SegmentedControl::new(&mut state.seg_val, SEG_ITEMS)
            .size(Size::Xs)
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
