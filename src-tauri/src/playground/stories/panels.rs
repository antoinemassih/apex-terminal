//! Panels story — PanelCard, PanelListRow, PanelKeyValueRow, PanelSection.
//!
//! Widgets shown (~22 instances):
//!   PanelSection: default, with count, with action, collapsible (4)
//!   PanelCard: default, striped, each tone (6)
//!   PanelListRow: basic, with secondary, with leading/trailing, columns mode (5)
//!   PanelKeyValueRow: with tones / meta (5)
//!   PanelListRow + selected / hover states (2)

use _scaffold_lib::ui_kit::widgets::{
    PanelCard, PanelKeyValueRow, PanelListRow, PanelSection,
    PanelTone,
    theme::ComponentTheme,
};
use egui::Ui;

// PanelCard::show / PanelSection::show are generic over `T: ComponentTheme`
// (not `dyn`), so this function must be generic too.
pub fn show<T: ComponentTheme>(ui: &mut Ui, theme: &T) {
    // ── PanelSection ──────────────────────────────────────────────────────────
    story_heading(ui, theme, "PanelSection");
    ui.add_space(6.0);

    // Basic
    PanelSection::new("POSITIONS")
        .show(ui, theme, |ui, theme| {
            ui.label(egui::RichText::new("Section body content here.").color(theme.dim()).size(11.0));
        });

    ui.add_space(6.0);

    // With count
    PanelSection::new("ACTIVE ORDERS")
        .count(3)
        .show(ui, theme, |ui, theme| {
            ui.label(egui::RichText::new("3 active orders.").color(theme.dim()).size(11.0));
        });

    ui.add_space(6.0);

    // With action
    PanelSection::new("ALERTS")
        .count(5)
        .action("Clear All", PanelTone::Danger)
        .show(ui, theme, |ui, theme| {
            ui.label(egui::RichText::new("Alert list body.").color(theme.dim()).size(11.0));
        });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── PanelCard ─────────────────────────────────────────────────────────────
    story_heading(ui, theme, "PanelCard");
    ui.add_space(6.0);

    // Default
    PanelCard::new()
        .show(ui, theme, |ui, theme| {
            ui.label(egui::RichText::new("Default card — no stripe.").color(theme.text()).size(11.0));
        });

    ui.add_space(6.0);

    // Striped tones in a horizontal row
    ui.horizontal_wrapped(|ui| {
        for (tone, tlabel) in &[
            (PanelTone::Default, "Default"),
            (PanelTone::Accent,  "Accent"),
            (PanelTone::Bull,    "Bull"),
            (PanelTone::Bear,    "Bear"),
            (PanelTone::Warn,    "Warn"),
        ] {
            PanelCard::new()
                .tone(*tone)
                .stripe(true)
                .padding(8.0)
                .show(ui, theme, |ui, theme| {
                    ui.label(egui::RichText::new(*tlabel).color(theme.text()).size(10.0));
                    ui.label(egui::RichText::new("Striped card").color(theme.dim()).size(9.0));
                });
            ui.add_space(6.0);
        }
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── PanelListRow ──────────────────────────────────────────────────────────
    story_heading(ui, theme, "PanelListRow");
    ui.add_space(6.0);

    // Basic
    PanelListRow::new("row_basic")
        .primary("AAPL")
        .show(ui, theme);

    // With secondary
    PanelListRow::new("row_secondary")
        .primary("TSLA")
        .secondary("Tesla Inc.")
        .show(ui, theme);

    // Selected
    PanelListRow::new("row_selected")
        .primary("NVDA")
        .secondary("NVIDIA Corporation")
        .selected(true)
        .show(ui, theme);

    // With trailing text
    let bull_color = theme.bull();
    PanelListRow::new("row_trailing")
        .primary("MSFT")
        .secondary("Microsoft")
        .trailing(move |ui, _t| {
            ui.label(egui::RichText::new("$415.22").color(bull_color).size(11.0));
        })
        .show(ui, theme);

    // Columns mode
    PanelListRow::new("row_cols")
        .columns(&[
            _scaffold_lib::ui_kit::widgets::PanelColumn::left("09:31:42").color(theme.dim()),
            _scaffold_lib::ui_kit::widgets::PanelColumn::right("$145.32").color(theme.bull()),
            _scaffold_lib::ui_kit::widgets::PanelColumn::right("250").color(theme.text()),
        ])
        .show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── PanelKeyValueRow ──────────────────────────────────────────────────────
    story_heading(ui, theme, "PanelKeyValueRow");
    ui.add_space(6.0);

    PanelCard::new().show(ui, theme, |ui, theme| {
        PanelKeyValueRow::new("Buying Power", "$24,500.00")
            .tone(PanelTone::Default)
            .show(ui, theme);
        PanelKeyValueRow::new("Realized P&L", "+$1,340.22")
            .tone(PanelTone::Bull)
            .show(ui, theme);
        PanelKeyValueRow::new("Unrealized P&L", "-$220.50")
            .tone(PanelTone::Bear)
            .show(ui, theme);
        PanelKeyValueRow::new("Day Trades Left", "2")
            .tone(PanelTone::Warn)
            .meta("PDT rule")
            .show(ui, theme);
        PanelKeyValueRow::new("Account", "Standard Margin")
            .tone(PanelTone::Accent)
            .show(ui, theme);
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
