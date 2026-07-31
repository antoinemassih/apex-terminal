//! Disclosure story — Tabs, Breadcrumb, Pagination, Stepper, Tree, PanelSubSection.
//!
//! Widgets shown (~20 instances):
//!   Tabs: Line × 4, Segmented × 4, Filled × 4 (3 strips × 4 tabs = 12 tab renders, count as 3)
//!   Breadcrumb: chevron 3-segment, slash 3-segment (2)
//!   Pagination: 5-page full, compact (2)
//!   Stepper: horizontal 4-step (current=1), vertical 4-step (current=2) (2)
//!   Tree: 3-level folder tree (1 widget, 7 visible nodes)
//!   PanelSubSection: 2 collapsible panels (2)

use _scaffold_lib::ui_kit::widgets::{
    Pagination, PanelSubSection, Stepper, Tabs, TabTreatment,
    theme::ComponentTheme,
};
use egui::Ui;

use super::super::stories::DisclosureState;

// PanelSubSection requires a concrete T: ComponentTheme, so this story
// is generic (same pattern as panels.rs).


// ── Sample tree data ──────────────────────────────────────────────────────────

pub fn show<T: ComponentTheme>(ui: &mut Ui, theme: &T, state: &mut DisclosureState) {
    // ── Tabs ──────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Tabs");
    ui.add_space(6.0);

    const TAB_LABELS: &[&str] = &["Overview", "Positions", "Orders", "History"];

    ui.label(egui::RichText::new("Line treatment").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    Tabs::new(&mut state.tab_line, TAB_LABELS)
        .treatment(TabTreatment::Line)
        .id_salt("disc_line")
        .show(ui, theme);

    ui.add_space(8.0);
    ui.label(egui::RichText::new("Segmented treatment").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    Tabs::new(&mut state.tab_seg, TAB_LABELS)
        .treatment(TabTreatment::Segmented)
        .id_salt("disc_seg")
        .show(ui, theme);

    ui.add_space(8.0);
    ui.label(egui::RichText::new("Filled treatment").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    Tabs::new(&mut state.tab_filled, TAB_LABELS)
        .treatment(TabTreatment::Filled)
        .id_salt("disc_filled")
        .show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Pagination ────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Pagination");
    ui.add_space(6.0);

    ui.label(egui::RichText::new("Full (page 1/5)").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    Pagination::new(&mut state.page_a, 5).show(ui, theme);

    ui.add_space(8.0);
    ui.label(egui::RichText::new("Compact (page 3/12)").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    Pagination::new(&mut state.page_b, 12).compact(true).show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Stepper ───────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Stepper");
    ui.add_space(6.0);

    const WIZARD_STEPS: &[&str] = &["Order", "Pay", "Ship", "Done"];

    ui.label(egui::RichText::new("Horizontal — step 2 active").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    Stepper::new(WIZARD_STEPS, 1).show(ui, theme);

    ui.add_space(12.0);
    ui.label(egui::RichText::new("Vertical — step 3 active").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.set_width(200.0);
        Stepper::new(WIZARD_STEPS, 2).vertical(true).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── PanelSubSection (Disclosure-like collapsible) ─────────────────────────
    story_heading(ui, theme, "Disclosure (PanelSubSection)");
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Click headers to collapse/expand")
            .color(theme.dim())
            .size(10.0),
    );
    ui.add_space(4.0);

    PanelSubSection::new("disc_panel_a", "ACCOUNT DETAILS")
        .count(3)
        .expanded(&mut state.panel_a_open)
        .show(ui, theme, |ui, theme| {
            ui.label(egui::RichText::new("Account: Standard Margin").color(theme.dim()).size(11.0));
            ui.label(egui::RichText::new("Buying Power: $24,500").color(theme.dim()).size(11.0));
            ui.label(egui::RichText::new("Day Trades Left: 2").color(theme.dim()).size(11.0));
        });

    PanelSubSection::new("disc_panel_b", "RISK SETTINGS")
        .count(2)
        .expanded(&mut state.panel_b_open)
        .show(ui, theme, |ui, theme| {
            ui.label(egui::RichText::new("Max Loss / Day: $500").color(theme.dim()).size(11.0));
            ui.label(egui::RichText::new("Position Limit: 5").color(theme.dim()).size(11.0));
        });

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
