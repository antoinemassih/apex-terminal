//! Button story — showcases every Variant × Size × State combination plus
//! the role-preset constructors.
//!
//! Widgets shown (~38 instances):
//!   Variants × Sizes: Primary/Secondary/Ghost/Danger/Link × Xs/Sm/Md/Lg  (20)
//!   States: disabled, active, loading (3)
//!   Role presets: toolbar, action, trade/buy/sell, cta, small_action, icon (8)
//!   Chip / Tab / Toggle / MutedIcon / NeutralAction / TextOnly (7)

use _scaffold_lib::ui_kit::widgets::{Button, Size, Variant};
use _scaffold_lib::ui_kit::widgets::theme::ComponentTheme;
use _scaffold_lib::ui_kit::widgets::button_style::{
    OutlinedButtonStyle, SoftButtonStyle, ElevatedButtonStyle,
    MutedButtonStyle, AccentEmphasisStyle,
};
use _scaffold_lib::ui_kit::icons::Icon;
use egui::Ui;

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme) {
    story_heading(ui, theme, "Variant × Size grid");
    ui.add_space(6.0);

    const VARIANTS: &[(Variant, &str)] = &[
        (Variant::Primary,   "Primary"),
        (Variant::Secondary, "Secondary"),
        (Variant::Ghost,     "Ghost"),
        (Variant::Danger,    "Danger"),
        (Variant::Link,      "Link"),
    ];

    const SIZES: &[(Size, &str)] = &[
        (Size::Xs, "Xs"),
        (Size::Sm, "Sm"),
        (Size::Md, "Md"),
        (Size::Lg, "Lg"),
    ];

    // Header row
    ui.horizontal(|ui| {
        ui.add_space(90.0);
        for (_, slabel) in SIZES {
            ui.add_sized([70.0, 18.0], egui::Label::new(
                egui::RichText::new(*slabel).color(theme.dim()).size(10.0),
            ));
        }
    });

    for (variant, vlabel) in VARIANTS {
        ui.horizontal(|ui| {
            ui.add_sized([90.0, 28.0], egui::Label::new(
                egui::RichText::new(*vlabel).color(theme.dim()).size(10.0),
            ));
            for (size, _) in SIZES {
                Button::new(*vlabel)
                    .variant(*variant)
                    .size(*size)
                    .show(ui, theme);
                ui.add_space(4.0);
            }
        });
        ui.add_space(4.0);
    }

    ui.add_space(16.0);
    rule(ui, theme);

    // ── States ────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "States");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Button::new("Disabled").variant(Variant::Primary).disabled(true).show(ui, theme);
        ui.add_space(8.0);
        Button::new("Active").variant(Variant::Secondary).active(true).show(ui, theme);
        ui.add_space(8.0);
        Button::new("Loading").variant(Variant::Primary).loading(true).show(ui, theme);
        ui.add_space(8.0);
        Button::new("Disabled Ghost").variant(Variant::Ghost).disabled(true).show(ui, theme);
        ui.add_space(8.0);
        Button::new("Disabled Danger").variant(Variant::Danger).disabled(true).show(ui, theme);
    });

    ui.add_space(16.0);
    rule(ui, theme);

    // ── Role presets ─────────────────────────────────────────────────────────
    story_heading(ui, theme, "Role presets");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Button::toolbar("Toolbar").show(ui, theme);
        ui.add_space(6.0);
        Button::action("Action").show(ui, theme);
        ui.add_space(6.0);
        Button::trade("Trade").show(ui, theme);
        ui.add_space(6.0);
        Button::buy("Buy").show(ui, theme);
        ui.add_space(6.0);
        Button::sell("Sell").show(ui, theme);
        ui.add_space(6.0);
        Button::cta("CTA").show(ui, theme);
        ui.add_space(6.0);
        Button::small_action("SmAction").show(ui, theme);
        ui.add_space(6.0);
        Button::simple("Simple").show(ui, theme);
    });

    ui.add_space(16.0);
    rule(ui, theme);

    // ── Icon-only buttons ─────────────────────────────────────────────────────
    story_heading(ui, theme, "Icon-only");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for icon in &[Icon::PLUS, Icon::TRASH, Icon::PENCIL_LINE, Icon::X, Icon::BELL, Icon::GEAR] {
            Button::icon(icon).variant(Variant::Ghost).show(ui, theme);
            ui.add_space(4.0);
        }
        ui.add_space(12.0);
        // Danger icon
        Button::icon(Icon::TRASH).variant(Variant::Danger).show(ui, theme);
        ui.add_space(4.0);
        // Primary icon
        Button::icon(Icon::PLUS).variant(Variant::Primary).show(ui, theme);
    });

    ui.add_space(16.0);
    rule(ui, theme);

    // ── Extended variants ─────────────────────────────────────────────────────
    story_heading(ui, theme, "Extended variants (Chip / Tab / Toggle / MutedIcon / NeutralAction / TextOnly)");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Button::new("Chip Off").variant(Variant::Chip).show(ui, theme);
        ui.add_space(4.0);
        Button::new("Chip On").variant(Variant::Chip).active(true).show(ui, theme);
        ui.add_space(8.0);
        Button::new("Tab Off").variant(Variant::Tab).show(ui, theme);
        ui.add_space(4.0);
        Button::new("Tab On").variant(Variant::Tab).active(true).show(ui, theme);
        ui.add_space(8.0);
        Button::new("Toggle Off").variant(Variant::Toggle).show(ui, theme);
        ui.add_space(4.0);
        Button::new("Toggle On").variant(Variant::Toggle).active(true).show(ui, theme);
        ui.add_space(8.0);
        Button::icon(Icon::PENCIL_LINE).variant(Variant::MutedIcon).show(ui, theme);
        ui.add_space(8.0);
        Button::new("Flatten").variant(Variant::NeutralAction).show(ui, theme);
        ui.add_space(8.0);
        Button::new("Text only").variant(Variant::TextOnly).show(ui, theme);
    });

    ui.add_space(8.0);

    ui.add_space(16.0);
    rule(ui, theme);

    // ── Custom Stylesheets ────────────────────────────────────────────────────
    story_heading(ui, theme, "CUSTOM STYLESHEETS");
    ui.add_space(4.0);
    ui.label(egui::RichText::new(
        "Same Button::new(\"Sample\") × 4 variants, each row using a different ButtonStyle impl."
    ).color(theme.dim()).size(10.0));
    ui.add_space(8.0);

    const DEMO_VARIANTS: &[(Variant, &str)] = &[
        (Variant::Primary,   "Primary"),
        (Variant::Secondary, "Secondary"),
        (Variant::Ghost,     "Ghost"),
        (Variant::Danger,    "Danger"),
    ];

    // Row helper: render 4 variants with a given style + label.
    macro_rules! style_row {
        ($label:expr, $style_expr:expr) => {{
            ui.label(egui::RichText::new($label).color(theme.dim()).size(10.0).italics());
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let style = $style_expr;
                for (variant, vlabel) in DEMO_VARIANTS {
                    Button::new(*vlabel)
                        .variant(*variant)
                        .size(Size::Md)
                        .show_styled(ui, theme, &style);
                    ui.add_space(6.0);
                }
            });
            ui.add_space(8.0);
        }};
    }

    style_row!("OutlinedButtonStyle — transparent fill, colored border (Material Outlined)",
        OutlinedButtonStyle::new(theme));

    style_row!("SoftButtonStyle — soft alpha tinted fill, no border (friendly modern)",
        SoftButtonStyle::new(theme));

    style_row!("ElevatedButtonStyle — ghost fill + visible border, raised card feel",
        ElevatedButtonStyle::new(theme));

    style_row!("MutedButtonStyle — all variants dim fg + theme border, still clickable",
        MutedButtonStyle::new(theme));

    style_row!("AccentEmphasisStyle — every variant renders as accent, cohesive group",
        AccentEmphasisStyle::new(theme));

    ui.add_space(8.0);
}

// ── Helpers ────────────────────────────────────────────────────────────────────

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
