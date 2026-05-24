//! Specialty story — Progress, ColorPicker, ScrollArea, ThemePreviewCard, Sidebar.
//!
//! Widgets shown (~18 instances):
//!   Progress: linear × 3 values + 1 indeterminate; circular × 3 values + 1 indeterminate (8)
//!   ColorPicker: compact swatch (2)
//!   ThemedScrollArea: 50-row list (1)
//!   ThemePreviewCard: 3 cards (3)
//!   Sidebar: Panel style with 4 items (1)

use _scaffold_lib::ui_kit::widgets::{
    ColorPicker, Progress, Sidebar, SidebarItem, SidebarStyle, Size, ThemedScrollArea,
    ThemePreviewCard,
    theme::{ComponentTheme, PortableTheme},
};
use egui::Ui;

use super::super::stories::SpecialtyState;

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme, state: &mut SpecialtyState) {
    // ── Progress ──────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Progress");
    ui.add_space(6.0);

    ui.label(egui::RichText::new("Linear — 20% / 55% / 85%").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    for val in &[0.20_f32, 0.55, 0.85] {
        Progress::linear(*val).show(ui, theme);
        ui.add_space(4.0);
    }

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Linear indeterminate").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    Progress::linear_indeterminate().show(ui, theme);

    ui.add_space(8.0);
    ui.label(egui::RichText::new("Circular — 25% / 60% / 90% + indeterminate").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for val in &[0.25_f32, 0.60, 0.90] {
            Progress::circular(*val).size(Size::Md).show(ui, theme);
            ui.add_space(8.0);
        }
        Progress::circular_indeterminate().size(Size::Md).show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── ColorPicker ───────────────────────────────────────────────────────────
    story_heading(ui, theme, "ColorPicker");
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Compact swatch — click to open the full picker popover:")
            .color(theme.dim())
            .size(10.0),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ColorPicker::new(&mut state.color_a)
            .label("Color A")
            .compact(true)
            .show(ui, theme);
        ui.add_space(16.0);
        ColorPicker::new(&mut state.color_b)
            .label("Color B")
            .compact(true)
            .with_alpha(true)
            .show(ui, theme);
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── ThemedScrollArea ──────────────────────────────────────────────────────
    story_heading(ui, theme, "ScrollArea (50 rows)");
    ui.add_space(6.0);

    ThemedScrollArea::vertical()
        .max_height(160.0)
        .auto_shrink([false, true])
        .show(ui, theme, |ui| {
            for i in 1..=50_usize {
                let color = if i % 2 == 0 { theme.dim() } else { theme.text() };
                ui.label(
                    egui::RichText::new(format!("Row {:02} — Some content label", i))
                        .color(color)
                        .size(11.0),
                );
            }
        });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── ThemePreviewCard ──────────────────────────────────────────────────────
    story_heading(ui, theme, "ThemePreviewCard");
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Three theme tiles — click to \"select\" one:")
            .color(theme.dim())
            .size(10.0),
    );
    ui.add_space(6.0);

    let dark_theme     = PortableTheme::dark();
    let light_theme    = PortableTheme::light();
    let midnight_theme = crate::playground::midnight();

    ui.horizontal(|ui| {
        let r0 = ThemePreviewCard::new("Dark", &dark_theme)
            .selected(state.selected_theme == 0)
            .show(ui, theme);
        if r0.clicked() { state.selected_theme = 0; }

        ui.add_space(8.0);

        let r1 = ThemePreviewCard::new("Light", &light_theme)
            .selected(state.selected_theme == 1)
            .show(ui, theme);
        if r1.clicked() { state.selected_theme = 1; }

        ui.add_space(8.0);

        let r2 = ThemePreviewCard::new("Midnight", &midnight_theme)
            .selected(state.selected_theme == 2)
            .show(ui, theme);
        if r2.clicked() { state.selected_theme = 2; }
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Sidebar ───────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Sidebar");
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Panel-style sidebar with 4 nav items:")
            .color(theme.dim())
            .size(10.0),
    );
    ui.add_space(4.0);

    use _scaffold_lib::ui_kit::icons::Icon;
    let items = [
        SidebarItem::new("Dashboard",  Icon::CHART_LINE),
        SidebarItem::new("Watchlist",  Icon::LIST),
        SidebarItem::new("Orders",     Icon::SHOPPING_CART).badge(3),
        SidebarItem::new("Settings",   Icon::GEAR),
    ];

    ui.allocate_ui_with_layout(
        egui::vec2(240.0, 160.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            Sidebar::new(&mut state.sidebar_active, &items)
                .style(SidebarStyle::Panel)
                .width(220.0)
                .show(ui, theme);
        },
    );

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
