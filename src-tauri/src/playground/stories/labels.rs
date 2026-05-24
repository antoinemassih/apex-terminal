//! Labels story — Label, Tag, Badge, Kbd.
//!
//! Widgets shown (~28 instances):
//!   Label: heading, body, muted, monospace, number, sizes (7)
//!   Tag: all 5 tones, closable, outline, with dot, disabled, sizes (10)
//!   Badge: count, dot, max, text tones (6)
//!   Kbd: single key, multi-key sequences (5)

use _scaffold_lib::ui_kit::widgets::{
    Badge, Kbd, Label, Size, Tag, TagTone,
    theme::ComponentTheme,
};
use egui::Ui;

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme) {
    // ── Label ─────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Label");
    ui.add_space(6.0);

    Label::heading("Heading Label — Lg, strong").show(ui, theme);
    ui.add_space(2.0);
    Label::new("Body label — Sm proportional").show(ui, theme);
    ui.add_space(2.0);
    Label::new("Muted label").muted().show(ui, theme);
    ui.add_space(2.0);
    Label::number("$12,345.67").show(ui, theme);
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        Label::new("Xs").size(Size::Xs).show(ui, theme);
        ui.add_space(8.0);
        Label::new("Sm").size(Size::Sm).show(ui, theme);
        ui.add_space(8.0);
        Label::new("Md").size(Size::Md).show(ui, theme);
        ui.add_space(8.0);
        Label::new("Lg").size(Size::Lg).show(ui, theme);
        ui.add_space(8.0);
        Label::new("Strong").strong().show(ui, theme);
        ui.add_space(8.0);
        Label::new("Accented").color(theme.accent()).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Tag ───────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Tag");
    ui.add_space(6.0);

    // Tones
    ui.horizontal(|ui| {
        for (tone, tlabel) in &[
            (TagTone::Neutral, "Neutral"),
            (TagTone::Accent,  "Accent"),
            (TagTone::Bull,    "Bull"),
            (TagTone::Bear,    "Bear"),
            (TagTone::Warn,    "Warn"),
        ] {
            Tag::new(*tlabel).tone(*tone).show(ui, theme);
            ui.add_space(4.0);
        }
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Tag::new("Outline").tone(TagTone::Accent).outline(true).show(ui, theme);
        ui.add_space(4.0);
        Tag::new("With Dot").tone(TagTone::Bull).dot(true).show(ui, theme);
        ui.add_space(4.0);
        Tag::new("Closable").tone(TagTone::Neutral).closable(true).show(ui, theme);
        ui.add_space(4.0);
        Tag::new("Disabled").tone(TagTone::Bear).disabled(true).show(ui, theme);
        ui.add_space(4.0);
        Tag::new("Xs").tone(TagTone::Accent).size(Size::Xs).show(ui, theme);
        ui.add_space(4.0);
        Tag::new("Md").tone(TagTone::Accent).size(Size::Md).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Badge ─────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Badge");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Badge::count(3).tone(TagTone::Bear).show(ui, theme);
        ui.add_space(4.0);
        Badge::count(99).tone(TagTone::Warn).show(ui, theme);
        ui.add_space(4.0);
        Badge::count(150).max(99).tone(TagTone::Bear).show(ui, theme);
        ui.add_space(4.0);
        Badge::dot().tone(TagTone::Bull).show(ui, theme);
        ui.add_space(4.0);
        Badge::dot().tone(TagTone::Warn).show(ui, theme);
        ui.add_space(4.0);
        Badge::dot().tone(TagTone::Bear).show(ui, theme);
        ui.add_space(4.0);
        Badge::text("NEW").tone(TagTone::Accent).show(ui, theme);
        ui.add_space(4.0);
        Badge::text("BETA").tone(TagTone::Warn).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Kbd ───────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Kbd");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        Kbd::new("Ctrl+K").show(ui, theme);
        ui.add_space(8.0);
        Kbd::new("Cmd+Shift+P").show(ui, theme);
        ui.add_space(8.0);
        Kbd::sequence(&["Ctrl", "Alt", "Del"]).show(ui, theme);
        ui.add_space(8.0);
        Kbd::new("Esc").size(Size::Xs).show(ui, theme);
        ui.add_space(8.0);
        Kbd::new("Enter").size(Size::Md).show(ui, theme);
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
