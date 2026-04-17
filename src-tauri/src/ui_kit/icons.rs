//! Icon system — Phosphor icons for consistent iconography.
//!
//! Usage: `ui.label(Icon::PENCIL);` or `Icon::button(ui, Icon::TRASH, "Delete")`

use egui_phosphor::regular as ph;
use egui_phosphor::bold as ph_bold;
use egui_phosphor::fill as ph_fill;

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
    pub const SQUARE: &'static str = ph::SQUARE;
    pub const LIST: &'static str = ph::LIST;
    pub const ARROW_FAT_UP: &'static str = ph::ARROW_FAT_UP;
    pub const ARROW_FAT_DOWN: &'static str = ph::ARROW_FAT_DOWN;
    pub const SHIELD_WARNING: &'static str = ph::SHIELD_WARNING;
    pub const RULER: &'static str = ph::RULER;
    pub const ARROWS_OUT: &'static str = ph::ARROWS_OUT;
    pub const ARROW_COUNTER_CLOCKWISE: &'static str = ph::ARROW_COUNTER_CLOCKWISE;
    pub const PLAY: &'static str = ph::PLAY;
    pub const PAUSE: &'static str = ph::PAUSE;
    pub const EYE: &'static str = ph::EYE;
    pub const EYE_SLASH: &'static str = ph::EYE_SLASH;

    // UI
    pub const CARET_DOWN: &'static str = ph::CARET_DOWN;
    pub const CARET_RIGHT: &'static str = ph::CARET_RIGHT;
    pub const DOTS_SIX_VERTICAL: &'static str = ph::DOTS_SIX_VERTICAL;
    pub const CHECK: &'static str = ph::CHECK;
    pub const CHECK_SQUARE: &'static str = ph::CHECK_SQUARE;
    pub const SQUARE_EMPTY: &'static str = ph::SQUARE;
    pub const DOTS_THREE: &'static str = ph::DOTS_THREE;
    pub const PALETTE: &'static str = ph::PALETTE;
    pub const SLIDERS: &'static str = ph::SLIDERS;
    pub const FOLDER: &'static str = ph::FOLDER;
    pub const PLUS: &'static str = ph::PLUS;
    pub const QUESTION: &'static str = ph::QUESTION;
    pub const GEAR: &'static str = ph::GEAR;
    pub const FUNNEL: &'static str = ph::FUNNEL;
    pub const PLUGS_CONNECTED: &'static str = ph::PLUGS_CONNECTED;
    pub const BOOK_OPEN: &'static str = ph::BOOK_OPEN;
    pub const SHOPPING_CART: &'static str = ph::SHOPPING_CART;
    pub const CIRCLES_FOUR: &'static str = ph::CIRCLES_FOUR;
    pub const BROWSERS: &'static str = ph::BROWSERS;
    pub const SIDEBAR: &'static str = ph::SIDEBAR;
    pub const TAG: &'static str = ph::TAG;
    pub const CROSSHAIR: &'static str = ph::CROSSHAIR;
    pub const LOCK: &'static str = ph::LOCK;
    pub const LOCK_OPEN: &'static str = ph::LOCK_OPEN;
    pub const LIGHTNING: &'static str = ph::LIGHTNING;
    pub const BELL: &'static str = ph::BELL;
    pub const BELL_RINGING: &'static str = ph::BELL_RINGING;
    pub const RADIO_BUTTON: &'static str = ph::RADIO_BUTTON;
    pub const MEGAPHONE: &'static str = ph::MEGAPHONE;
    pub const DOT: &'static str = ph::DOT_OUTLINE;
    pub const CIRCLE: &'static str = ph::CIRCLE;
    pub const CURRENCY_DOLLAR: &'static str = ph::CURRENCY_DOLLAR;
    pub const GIT_DIFF: &'static str = ph::GIT_DIFF;
    pub const ARTICLE: &'static str = ph::ARTICLE;
    pub const SPARKLE: &'static str = ph::SPARKLE;
    pub const PULSE: &'static str = ph::PULSE;
    pub const NOTEBOOK: &'static str = ph::NOTEBOOK;
    pub const STAR: &'static str = ph::STAR;
    pub const STAR_FILL: &'static str = ph_fill::STAR;
    pub const CHAT_DOTS: &'static str = ph::CHAT_DOTS;
    pub const NEWSPAPER: &'static str = ph::NEWSPAPER;
    pub const CODE: &'static str = ph::CODE;
    pub const TERMINAL: &'static str = ph::TERMINAL;
    pub const CAMERA: &'static str = ph::CAMERA;

    // Chart
    pub const CHART_LINE: &'static str = ph::CHART_LINE;
    pub const CHART_BAR: &'static str = ph::CHART_BAR;
    pub const MAGNIFYING_GLASS: &'static str = ph::MAGNIFYING_GLASS;
    pub const MAGNIFYING_GLASS_PLUS: &'static str = ph::MAGNIFYING_GLASS_PLUS;

    // Bold variants for toolbar (more visible at small sizes)
    pub const PENCIL_LINE_BOLD: &'static str = ph_bold::PENCIL_LINE;
    pub const LINE_SEGMENT_BOLD: &'static str = ph_bold::LINE_SEGMENT;
    pub const MINUS_BOLD: &'static str = ph_bold::MINUS;
    pub const RECTANGLE_BOLD: &'static str = ph_bold::RECTANGLE;
    pub const MAP_PIN_BOLD: &'static str = ph_bold::MAP_PIN;
    pub const TRASH_BOLD: &'static str = ph_bold::TRASH;
    pub const PLAY_BOLD: &'static str = ph_bold::PLAY;
    pub const X_BOLD: &'static str = ph_bold::X;

    // Media / Replay controls
    pub const SKIP_BACK: &'static str = ph::SKIP_BACK;
    pub const SKIP_FORWARD: &'static str = ph::SKIP_FORWARD;
    pub const CARET_LEFT: &'static str = ph::CARET_LEFT;
    pub const FAST_FORWARD: &'static str = ph::FAST_FORWARD;
    pub const REWIND: &'static str = ph::REWIND;

    /// Render an icon button at standard size (16px)
    pub fn button(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
        let btn = ui.add(egui::Button::new(egui::RichText::new(icon).size(16.0)).frame(false));
        if !tooltip.is_empty() { btn.clone().on_hover_text(tooltip); }
        btn
    }

    /// Render an icon button with color at standard size
    pub fn button_colored(ui: &mut egui::Ui, icon: &str, color: egui::Color32, tooltip: &str) -> egui::Response {
        let btn = ui.add(egui::Button::new(egui::RichText::new(icon).size(16.0).color(color)).frame(false));
        if !tooltip.is_empty() { btn.clone().on_hover_text(tooltip); }
        btn
    }

    /// Render a large icon button (20px) for prominent actions
    pub fn button_large(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
        let btn = ui.add(egui::Button::new(egui::RichText::new(icon).size(20.0)).frame(false));
        if !tooltip.is_empty() { btn.clone().on_hover_text(tooltip); }
        btn
    }
}

/// Available font choices for the UI.
pub const FONT_NAMES: &[&str] = &[
    "JetBrains Mono",  // 0 — modern monospace, default
    "Roboto Mono",     // 1 — clean Google monospace
    "IBM Plex Mono",   // 2 — professional IBM monospace
    "Inter",           // 3 — best-practice sans-serif, widely loved
    "Plus Jakarta",    // 4 — modern fintech sans-serif
    "Space Grotesk",   // 5 — geometric modern sans-serif
];

/// Initialize fonts + Phosphor icons. Call once during app setup.
/// `font_idx` selects which of the 6 fonts to use as primary.
pub fn init_fonts(ctx: &egui::Context, font_idx: usize) {
    let mut fonts = egui::FontDefinitions::default();

    // Load all 6 fonts — use FontTweak for baseline alignment and crispness
    let tweak_mono = egui::FontTweak {
        scale: 1.0,
        y_offset_factor: -0.02, // lift monospace slightly for better vertical centering
        y_offset: 0.0,
        baseline_offset_factor: 0.0,
    };
    let tweak_sans = egui::FontTweak {
        scale: 1.02, // slight upscale for sans-serif to match mono visual weight
        y_offset_factor: -0.01,
        y_offset: 0.0,
        baseline_offset_factor: 0.0,
    };
    fonts.font_data.insert("jetbrains_mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("JetBrainsMono-Regular.ttf")).tweak(tweak_mono)));
    fonts.font_data.insert("roboto_mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("RobotoMono-Regular.ttf")).tweak(tweak_mono)));
    fonts.font_data.insert("ibm_plex_mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("IBMPlexMono-Regular.ttf")).tweak(tweak_mono)));
    fonts.font_data.insert("inter".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("Inter-Medium.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("plus_jakarta".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("PlusJakartaSans-Medium.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("space_grotesk".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("SpaceGrotesk-Medium.ttf")).tweak(tweak_sans)));

    // Pick the primary font based on selection
    let primary = match font_idx {
        1 => "roboto_mono",
        2 => "ibm_plex_mono",
        3 => "inter",
        4 => "plus_jakarta",
        5 => "space_grotesk",
        _ => "jetbrains_mono",
    };

    // Set as primary for both font families
    if let Some(mono_keys) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        mono_keys.insert(0, primary.into());
    }
    if let Some(prop_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        prop_keys.insert(0, primary.into());
    }

    // Add Phosphor icon fonts
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Bold);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Fill);

    // Ensure phosphor is a fallback for Monospace too
    if let Some(mono_keys) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        if !mono_keys.contains(&"phosphor".to_string()) {
            mono_keys.push("phosphor".into());
        }
    }

    ctx.set_fonts(fonts);
}

/// Legacy alias — calls init_fonts with default font.
pub fn init_icons(ctx: &egui::Context) { init_fonts(ctx, 0); }
