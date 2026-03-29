//! Icon system — Phosphor icons for consistent iconography.
//!
//! Usage: `ui.label(Icon::PENCIL);` or `Icon::button(ui, Icon::TRASH, "Delete")`

use egui_phosphor::regular as ph;

/// Icon constants — all from Phosphor Regular set.
/// Add new icons here as needed; they render via the embedded Phosphor font.
pub struct Icon;

impl Icon {
    // Drawing tools
    pub const PENCIL_LINE: &'static str = ph::PENCIL_LINE;
    pub const LINE_SEGMENT: &'static str = ph::LINE_SEGMENT;
    pub const MINUS: &'static str = ph::MINUS;
    pub const RECTANGLE: &'static str = ph::RECTANGLE;
    pub const MAP_PIN: &'static str = ph::MAP_PIN;
    pub const CURSOR: &'static str = ph::CURSOR;

    // Actions
    pub const TRASH: &'static str = ph::TRASH;
    pub const X: &'static str = ph::X;
    pub const ARROWS_OUT: &'static str = ph::ARROWS_OUT;
    pub const ARROW_COUNTER_CLOCKWISE: &'static str = ph::ARROW_COUNTER_CLOCKWISE;
    pub const PLAY: &'static str = ph::PLAY;
    pub const PAUSE: &'static str = ph::PAUSE;
    pub const EYE: &'static str = ph::EYE;
    pub const EYE_SLASH: &'static str = ph::EYE_SLASH;

    // UI
    pub const CARET_DOWN: &'static str = ph::CARET_DOWN;
    pub const CHECK: &'static str = ph::CHECK;
    pub const DOTS_THREE: &'static str = ph::DOTS_THREE;
    pub const PALETTE: &'static str = ph::PALETTE;
    pub const SLIDERS: &'static str = ph::SLIDERS;
    pub const FOLDER: &'static str = ph::FOLDER;
    pub const PLUS: &'static str = ph::PLUS;

    // Chart
    pub const CHART_LINE: &'static str = ph::CHART_LINE;
    pub const CHART_BAR: &'static str = ph::CHART_BAR;
    pub const MAGNIFYING_GLASS_PLUS: &'static str = ph::MAGNIFYING_GLASS_PLUS;

    /// Render an icon button (icon + optional tooltip)
    pub fn button(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
        let btn = ui.add(egui::Button::new(icon).frame(false));
        if !tooltip.is_empty() { btn.clone().on_hover_text(tooltip); }
        btn
    }

    /// Render an icon button with color
    pub fn button_colored(ui: &mut egui::Ui, icon: &str, color: egui::Color32, tooltip: &str) -> egui::Response {
        let btn = ui.add(egui::Button::new(egui::RichText::new(icon).color(color)).frame(false));
        if !tooltip.is_empty() { btn.clone().on_hover_text(tooltip); }
        btn
    }
}

/// Initialize Phosphor icon font. Call once during app setup.
pub fn init_icons(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Bold);
    ctx.set_fonts(fonts);
}
