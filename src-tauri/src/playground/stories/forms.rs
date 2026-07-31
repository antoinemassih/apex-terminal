//! Forms story — FormRow, FormField, FormSection, FormActionBar, Fieldset, InputGroup.
//!
//! Builds a "Settings" mock form:
//!   Section 1 "Profile" — 5 rows (Name, Email, Bio, Notifications, Theme)
//!   Section 2 "Trading" — 5 rows (Risk %, Max Loss, Order Type, Slippage, PDT Mode)
//!   FormActionBar at the bottom (Cancel + Save)
//!
//! Widgets shown (~25 instances):
//!   FormSection × 2 (2)
//!   FormRow × 10 (10 rows across 2 sections)
//!   FormField × 2 (used inline inside rows)
//!   Fieldset × 1 (Advanced group)
//!   InputGroup × 2 (prefix/suffix on numeric fields)
//!   FormActionBar × 1 (footer)
//!   Switch, Input, Select mixed in as form controls (~8 controls)

use _scaffold_lib::ui_kit::widgets::{
    FormField, FormRow, FormSection, Input, InputGroup,
    NumberStepper, Select, Switch,
    theme::ComponentTheme,
};
use egui::Ui;

use super::super::stories::FormsState;

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme, state: &mut FormsState) {
    // ── Section 1: Profile ────────────────────────────────────────────────────
    FormSection::new("Profile")
        .helper("Your personal details visible to other traders")
        .show(ui, theme, |ui| {
            // Name
            FormRow::new("Display Name")
                .required(true)
                .show(ui, theme, |ui| {
                    Input::new(&mut state.name_buf)
                        .placeholder("e.g. Antoine")
                        .width(280.0)
                        .show(ui, theme);
                });

            ui.add_space(4.0);

            // Email
            FormRow::new("Email")
                .required(true)
                .show(ui, theme, |ui| {
                    Input::new(&mut state.email_buf)
                        .placeholder("you@example.com")
                        .leading_icon(_scaffold_lib::ui_kit::icons::Icon::MAGNIFYING_GLASS)
                        .width(280.0)
                        .show(ui, theme);
                });

            ui.add_space(4.0);

            // Notifications toggle
            FormRow::new("Email Alerts")
                .helper_text("Receive trade confirmation emails")
                .show(ui, theme, |ui| {
                    let lbl = if state.notif_on { "On" } else { "Off" };
                    Switch::new(&mut state.notif_on)
                        .label(lbl)
                        .show(ui, theme);
                });

            ui.add_space(4.0);

            // FormField demo (top-label style)
            FormField::new("Notes")
                .helper("Optional — shown on your public profile")
                .show(ui, theme, |ui| {
                    Input::new(&mut state.notes_buf)
                        .placeholder("Short bio…")
                        .width(320.0)
                        .show(ui, theme);
                });
        });

    ui.add_space(16.0);
    rule(ui, theme);

    // ── Section 2: Trading ────────────────────────────────────────────────────
    FormSection::new("Trading")
        .helper("Risk parameters applied to all new orders")
        .show(ui, theme, |ui| {
            // Risk % with InputGroup suffix
            FormRow::new("Risk Per Trade")
                .required(true)
                .show(ui, theme, |ui| {
                    InputGroup::new()
                        .suffix("%")
                        .show(ui, theme, |ui| {
                            Input::new(&mut state.risk_pct_buf)
                                .placeholder("1.0")
                                .width(120.0)
                                .show(ui, theme);
                        });
                });

            ui.add_space(4.0);

            // Max loss with InputGroup prefix
            FormRow::new("Max Daily Loss")
                .show(ui, theme, |ui| {
                    InputGroup::new()
                        .prefix("$")
                        .show(ui, theme, |ui| {
                            Input::new(&mut state.max_loss_buf)
                                .placeholder("500")
                                .width(120.0)
                                .show(ui, theme);
                        });
                });

            ui.add_space(4.0);

            // Order type select
            FormRow::new("Default Order Type").show(ui, theme, |ui| {
                let options: &[&str] = &["Market", "Limit", "Stop", "Stop Limit"];
                Select::new(&mut state.order_type_idx, options)
                    .min_width(200.0)
                    .show(ui, theme);
            });

            ui.add_space(4.0);

            // Slippage stepper
            FormRow::new("Max Slippage")
                .helper_text("In basis points (1 bp = 0.01%)")
                .show(ui, theme, |ui| {
                    NumberStepper::new(&mut state.slippage_val)
                        .range(0.0..=50.0)
                        .step(1.0)
                        .suffix("bp")
                        .show(ui, theme);
                });

            ui.add_space(4.0);

            // PDT mode switch
            FormRow::new("PDT Mode").show(ui, theme, |ui| {
                Switch::new(&mut state.pdt_mode).label("Restrict to 3 day trades").show(ui, theme);
            });

            ui.add_space(8.0);

        });

    ui.add_space(8.0);
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
