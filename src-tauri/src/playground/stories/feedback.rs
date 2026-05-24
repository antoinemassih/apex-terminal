//! Feedback story — Alert, Toast, Indicator, StatusPill, Spinner, Skeleton, Progress.
//!
//! Widgets shown (~28 instances):
//!   Alert: info, success, warning, error; with and without title, closable (7)
//!   Toast: info, success, warning, danger (4)
//!   Indicator: dot, ring, pulsing × tones (6)
//!   StatusPill: connected, warn, disconnected, xs/sm (5)
//!   Spinner: Xs, Sm, Md, Lg (4)
//!   Skeleton: rect, text, lines, circle (4)
//!   Progress: linear determinate, indeterminate; circular determinate, indeterminate (4)

use _scaffold_lib::ui_kit::widgets::{
    Alert, Indicator, IndicatorTone, Progress, Size, Skeleton, Spinner,
    StatusPill, PanelTone,
    theme::ComponentTheme,
};
use _scaffold_lib::ui_kit::widgets::toast::{Toast, ToastVariant};
use egui::Ui;

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme) {
    // ── Alert ─────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Alert");
    ui.add_space(6.0);

    Alert::info("Informational message — new version available.")
        .show(ui, theme);
    ui.add_space(4.0);
    Alert::success("Order placed successfully. AAPL filled at $145.32.")
        .show(ui, theme);
    ui.add_space(4.0);
    Alert::warn("Market closes in 5 minutes.")
        .title("Market Close")
        .show(ui, theme);
    ui.add_space(4.0);
    Alert::error("Order rejected: insufficient buying power.")
        .title("Order Error")
        .show(ui, theme);
    ui.add_space(4.0);
    // Variants with variant() builder
    Alert::info("Info with closable=true").closable(true).show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Toast ─────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Toast (static render — no lifecycle)");
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        Toast::new("Info toast")
            .variant(ToastVariant::Info)
            .width(200.0)
            .show(ui);
        ui.add_space(8.0);
        Toast::new("Success")
            .success()
            .width(200.0)
            .show(ui);
        ui.add_space(8.0);
        Toast::new("Warning")
            .warning()
            .body("Something needs your attention.")
            .width(200.0)
            .show(ui);
        ui.add_space(8.0);
        Toast::new("Danger")
            .danger()
            .body("A critical error occurred.")
            .width(200.0)
            .show(ui);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Indicator ─────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Indicator");
    ui.add_space(6.0);
    let tones = [
        IndicatorTone::Neutral,
        IndicatorTone::Accent,
        IndicatorTone::Bull,
        IndicatorTone::Bear,
        IndicatorTone::Warn,
    ];
    ui.horizontal(|ui| {
        // Dot row
        for tone in &tones {
            Indicator::dot().tone(*tone).show(ui, theme);
            ui.add_space(2.0);
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Dot").color(theme.dim()).size(10.0));
        ui.add_space(12.0);
        // Ring row
        for tone in &tones {
            Indicator::ring().tone(*tone).show(ui, theme);
            ui.add_space(2.0);
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Ring").color(theme.dim()).size(10.0));
        ui.add_space(12.0);
        // Pulsing row
        for tone in &tones {
            Indicator::pulsing().tone(*tone).show(ui, theme);
            ui.add_space(2.0);
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Pulse").color(theme.dim()).size(10.0));
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── StatusPill ────────────────────────────────────────────────────────────
    story_heading(ui, theme, "StatusPill");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        StatusPill::new("Connected").tone(PanelTone::Bull).show(ui, theme);
        ui.add_space(6.0);
        StatusPill::new("Reconnecting").tone(PanelTone::Warn).show(ui, theme);
        ui.add_space(6.0);
        StatusPill::new("Disconnected").tone(PanelTone::Bear).show(ui, theme);
        ui.add_space(6.0);
        StatusPill::new("Pending").tone(PanelTone::Accent).show(ui, theme);
        ui.add_space(6.0);
        StatusPill::new("Xs").tone(PanelTone::Default).size(Size::Xs).show(ui, theme);
        ui.add_space(6.0);
        StatusPill::new("Sm").tone(PanelTone::Default).size(Size::Sm).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Spinner ───────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Spinner");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for (size, slabel) in &[
            (Size::Xs, "Xs"),
            (Size::Sm, "Sm"),
            (Size::Md, "Md"),
            (Size::Lg, "Lg"),
        ] {
            Spinner::new().size(*size).show(ui, theme);
            ui.add_space(4.0);
            ui.label(egui::RichText::new(*slabel).color(theme.dim()).size(10.0));
            ui.add_space(12.0);
        }
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Skeleton ──────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Skeleton");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Skeleton::rect(120.0, 16.0).show(ui, theme);
        ui.add_space(8.0);
        Skeleton::text(160.0).show(ui, theme);
        ui.add_space(8.0);
        Skeleton::circle(32.0).show(ui, theme);
    });
    ui.add_space(4.0);
    Skeleton::lines(3, 280.0).show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Progress ──────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Progress");
    ui.add_space(6.0);

    // Linear determinate
    ui.label(egui::RichText::new("Linear 0% / 40% / 70% / 100%").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    for t in &[0.0_f32, 0.4, 0.7, 1.0] {
        Progress::linear(*t).show(ui, theme);
        ui.add_space(4.0);
    }

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Linear indeterminate").color(theme.dim()).size(10.0));
    Progress::linear_indeterminate().show(ui, theme);

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Circular 50%").color(theme.dim()).size(10.0));
        ui.add_space(8.0);
        Progress::circular(0.5).size(Size::Sm).show(ui, theme);
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Circular indeterminate").color(theme.dim()).size(10.0));
        ui.add_space(8.0);
        Progress::circular_indeterminate().size(Size::Md).show(ui, theme);
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
