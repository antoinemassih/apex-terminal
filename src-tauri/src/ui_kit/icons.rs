//! Icon system — Phosphor Bold icons for consistent iconography.
//!
//! All icons use the Bold weight at 24px. Use `Icon::button` / `Icon::button_colored`.
//! Usage: `ui.label(Icon::PENCIL_LINE);` or `Icon::button(ui, Icon::TRASH, "Delete")`
//!
//! ## Close affordances — design rule
//!
//! Close affordances MUST use [`Icon::X`] (or its [`Icon::CLOSE`] alias).
//! Never use `Icon::X_CIRCLE`, `Icon::X_SQUARE`, or any filled close variants —
//! design rule per owner. Filled close icons read as "destroy", not "dismiss",
//! and break the visual language.
//!
//! The `X_CIRCLE` and `X_SQUARE` constants are deprecated for close use; they
//! remain available for cases where the shape itself is intentional (e.g.
//! error indicators), but should never be used as dismiss / close buttons.

use egui_phosphor::bold as ph;
use egui_phosphor::fill as ph_fill;

/// Icon constants — all from Phosphor Bold set.
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
    /// Semantic alias for `Icon::X`. Use for all dismiss / close affordances.
    /// See module-level docs for the design rule on close variants.
    pub const CLOSE: &'static str = Self::X;
    /// Phosphor Bold "arrow-left" (U+E058).
    pub const ARROW_LEFT: &'static str = "\u{E058}";
    /// Phosphor Bold "arrow-right" (U+E06C).
    pub const ARROW_RIGHT: &'static str = "\u{E06C}";
    /// Phosphor Bold "x-circle". Reads as "destroy" — NOT a close button.
    /// Use `Icon::X` / `Icon::CLOSE` for dismiss affordances.
    #[deprecated(note = "use Icon::X for close affordances; X_CIRCLE reads as 'destroy'")]
    pub const X_CIRCLE: &'static str = ph::X_CIRCLE;
    /// Phosphor Bold "x-square". Reads as "destroy" — NOT a close button.
    /// Use `Icon::X` / `Icon::CLOSE` for dismiss affordances.
    #[deprecated(note = "use Icon::X for close affordances; X_SQUARE reads as 'destroy'")]
    pub const X_SQUARE: &'static str = ph::X_SQUARE;
    pub const SQUARE: &'static str = ph::SQUARE;
    pub const LIST: &'static str = ph::LIST;
    pub const ARROW_FAT_UP: &'static str = ph::ARROW_FAT_UP;
    pub const ARROW_FAT_DOWN: &'static str = ph::ARROW_FAT_DOWN;
    pub const ARROW_FAT_LINES_UP: &'static str = ph::ARROW_FAT_LINES_UP;
    pub const ARROW_FAT_LINES_DOWN: &'static str = ph::ARROW_FAT_LINES_DOWN;
    pub const ARROW_ELBOW_RIGHT: &'static str = ph::ARROW_ELBOW_RIGHT;
    pub const HAND_PALM: &'static str = ph::HAND_PALM;
    pub const BRACKETS_CURLY: &'static str = ph::BRACKETS_CURLY;
    pub const SHIELD_WARNING: &'static str = ph::SHIELD_WARNING;
    /// Phosphor Bold "info" — used as the icon for Info-severity toasts.
    pub const INFO: &'static str = ph::INFO;
    /// Phosphor Bold "check-circle" — used as the icon for Success-severity toasts.
    pub const CHECK_CIRCLE: &'static str = ph::CHECK_CIRCLE;
    /// Phosphor Bold "warning" — used as the icon for Warning-severity toasts.
    pub const WARNING: &'static str = ph::WARNING;
    /// Phosphor Bold "push-pin" — used to indicate a pinned toast.
    pub const PUSH_PIN: &'static str = ph::PUSH_PIN;
    /// Phosphor Fill "shield-warning" — stronger critical indicator for Critical-severity toasts.
    pub const SHIELD_WARNING_FILL: &'static str = ph_fill::SHIELD_WARNING;
    pub const RULER: &'static str = ph::RULER;
    pub const ARROWS_OUT: &'static str = ph::ARROWS_OUT;
    pub const ARROWS_OUT_SIMPLE: &'static str = ph::ARROWS_OUT_SIMPLE;
    pub const ARROW_COUNTER_CLOCKWISE: &'static str = ph::ARROW_COUNTER_CLOCKWISE;
    pub const PLAY: &'static str = ph::PLAY;
    pub const PAUSE: &'static str = ph::PAUSE;
    pub const EYE: &'static str = ph::EYE;
    pub const EYE_SLASH: &'static str = ph::EYE_SLASH;

    // UI
    pub const CARET_DOWN: &'static str = ph::CARET_DOWN;
    pub const CARET_RIGHT: &'static str = ph::CARET_RIGHT;
    pub const CARET_LEFT: &'static str = ph::CARET_LEFT;
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
    pub const CIRCLES_THREE_PLUS: &'static str = ph::CIRCLES_THREE_PLUS;
    pub const LADDER: &'static str = ph::LADDER;
    pub const BROWSERS: &'static str = ph::BROWSERS;
    pub const SIDEBAR: &'static str = ph::SIDEBAR;
    pub const TAG: &'static str = ph::TAG;
    pub const CROSSHAIR: &'static str = ph::CROSSHAIR;
    pub const MAGNET: &'static str = ph::MAGNET;
    pub const BROADCAST: &'static str = ph::BROADCAST;
    pub const TREE_STRUCTURE: &'static str = ph::TREE_STRUCTURE;
    pub const STACK: &'static str = ph::STACK;
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

    // Fill variants — for use inside dropdowns (16px)
    pub const CHECK_FILL: &'static str = ph_fill::CHECK;
    pub const CIRCLE_FILL: &'static str = ph_fill::CIRCLE;
    pub const CARET_RIGHT_FILL: &'static str = ph_fill::CARET_RIGHT;
    pub const CARET_DOWN_FILL: &'static str = ph_fill::CARET_DOWN;
    pub const CHART_LINE_FILL: &'static str = ph_fill::CHART_LINE;
    pub const CHART_BAR_FILL: &'static str = ph_fill::CHART_BAR;
    pub const TREE_STRUCTURE_FILL: &'static str = ph_fill::TREE_STRUCTURE;
    pub const BROADCAST_FILL: &'static str = ph_fill::BROADCAST;
    pub const CHART_LINE_UP_FILL: &'static str = ph_fill::CHART_LINE_UP;

    pub const CHAT_DOTS: &'static str = ph::CHAT_DOTS;
    pub const NEWSPAPER: &'static str = ph::NEWSPAPER;
    pub const CODE: &'static str = ph::CODE;
    pub const TERMINAL: &'static str = ph::TERMINAL;
    pub const CAMERA: &'static str = ph::CAMERA;
    pub const CALENDAR_BLANK: &'static str = ph::CALENDAR_BLANK;
    pub const CLOCK: &'static str = ph::CLOCK;

    // Chart
    pub const CHART_LINE: &'static str = ph::CHART_LINE;
    pub const CHART_BAR: &'static str = ph::CHART_BAR;
    pub const MAGNIFYING_GLASS: &'static str = ph::MAGNIFYING_GLASS;
    pub const MAGNIFYING_GLASS_PLUS: &'static str = ph::MAGNIFYING_GLASS_PLUS;

    // Media / Replay controls
    pub const SKIP_BACK: &'static str = ph::SKIP_BACK;
    pub const SKIP_FORWARD: &'static str = ph::SKIP_FORWARD;
    pub const FAST_FORWARD: &'static str = ph::FAST_FORWARD;
    pub const REWIND: &'static str = ph::REWIND;

    /// Standard icon button (24px bold)
    pub fn button(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
        let btn = ui.add(egui::Button::new(egui::RichText::new(icon).size(24.0)).frame(false));
        if !tooltip.is_empty() { btn.clone().on_hover_text(tooltip); }
        btn
    }

    /// Icon button with color (24px bold)
    pub fn button_colored(ui: &mut egui::Ui, icon: &str, color: egui::Color32, tooltip: &str) -> egui::Response {
        let btn = ui.add(egui::Button::new(egui::RichText::new(icon).size(24.0).color(color)).frame(false));
        if !tooltip.is_empty() { btn.clone().on_hover_text(tooltip); }
        btn
    }

    /// Large icon button — kept for call-site compatibility, same 24px bold
    pub fn button_large(ui: &mut egui::Ui, icon: &str, tooltip: &str) -> egui::Response {
        Self::button(ui, icon, tooltip)
    }
}

// ── IconSet — pluggable glyph resolution ─────────────────────────────────────

/// A mapping from [`Icon`] name constants to glyph strings (typically
/// single-character Unicode from the active icon font).
///
/// ## Design
///
/// The `Icon::*` constant names (e.g. `Icon::TRASH`, `Icon::EYE`) remain the
/// stable lookup keys used at every call site — they don't change. What changes
/// is *which glyph* those names map to. A theme can supply an `IconSet` that
/// maps the same keys to different codepoints from a different icon font.
///
/// ## Built-in set
///
/// [`IconSet::builtin`] returns the current Phosphor Bold/Fill mapping — the
/// same glyphs used today. A theme that supplies no icon set gets the built-in
/// (zero visual change, consistent with the "no regressions by default" rule).
///
/// ## Usage
///
/// Widgets that currently write `Icon::TRASH` (a `&'static str`) can continue
/// to do so — those are the built-in glyphs. To support theme-overridable icons
/// the widget should instead call `icon_set.glyph(Icon::TRASH)`, which
/// falls back to the built-in constant when the set has no override entry.
///
/// The active set is accessed via the process-wide ambient accessor
/// [`active_icon_set`]. It starts as the built-in and can be replaced by the
/// theme-pack loader.
pub struct IconSet {
    /// Map from glyph key string to replacement glyph string.
    /// Keys are the `&'static str` values of the `Icon::*` constants
    /// (i.e. the Phosphor codepoints used as the default).
    overrides: std::collections::HashMap<&'static str, String>,
}

impl IconSet {
    /// The built-in Phosphor icon set — zero overrides, all glyphs resolve to
    /// the `Icon::*` constants directly. Calling `glyph(Icon::TRASH)` on this
    /// returns `Icon::TRASH` unchanged.
    pub fn builtin() -> Self {
        Self { overrides: std::collections::HashMap::new() }
    }

    /// Build an icon set from a map of `(Icon::CONSTANT, replacement_glyph)`.
    ///
    /// Example:
    /// ```
    /// let set = IconSet::from_overrides(&[
    ///     (Icon::TRASH,   "\u{F1C0}"),  // custom trash codepoint
    ///     (Icon::EYE,     "\u{F0D1}"),
    /// ]);
    /// ```
    pub fn from_overrides(pairs: &[(&'static str, &str)]) -> Self {
        let overrides = pairs.iter()
            .map(|(key, glyph)| (*key, (*glyph).to_owned()))
            .collect();
        Self { overrides }
    }

    /// Resolve an icon constant to its active glyph string.
    ///
    /// If this set has no override for `icon_const`, the constant itself is
    /// returned (the Phosphor codepoint — the current built-in behavior).
    pub fn glyph<'s>(&'s self, icon_const: &'static str) -> &'s str {
        self.overrides
            .get(icon_const)
            .map(|s| s.as_str())
            .unwrap_or(icon_const)
    }

    /// Returns `true` if this set has an override for the given icon constant.
    pub fn has_override(&self, icon_const: &'static str) -> bool {
        self.overrides.contains_key(icon_const)
    }

    /// Returns the number of override entries in this set.
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }
}

// ── Process-wide ambient icon set ─────────────────────────────────────────────

use std::sync::{Arc, RwLock};

static AMBIENT_ICON_SET: std::sync::OnceLock<RwLock<Arc<IconSet>>> =
    std::sync::OnceLock::new();

fn ambient_lock() -> &'static RwLock<Arc<IconSet>> {
    AMBIENT_ICON_SET.get_or_init(|| RwLock::new(Arc::new(IconSet::builtin())))
}

/// Get the process-wide active [`IconSet`].
///
/// Returns the built-in set until a theme pack replaces it via
/// [`set_active_icon_set`].  This is an `Arc` clone — cheap to call per frame.
pub fn active_icon_set() -> Arc<IconSet> {
    ambient_lock().read().unwrap().clone()
}

/// Replace the process-wide active [`IconSet`].
///
/// Called by the theme-pack loader when a pack with a custom icon set is
/// activated, or when reverting to the built-in.  Takes effect on the next
/// frame — no restart required.
pub fn set_active_icon_set(set: IconSet) {
    *ambient_lock().write().unwrap() = Arc::new(set);
}

/// Reset the process-wide icon set to the built-in Phosphor set.
pub fn reset_icon_set() {
    set_active_icon_set(IconSet::builtin());
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_set_passthrough() {
        let set = IconSet::builtin();
        // The built-in set must return the constant unchanged (zero visual change).
        assert_eq!(set.glyph(Icon::TRASH), Icon::TRASH);
        assert_eq!(set.glyph(Icon::EYE),   Icon::EYE);
        assert_eq!(set.override_count(), 0);
    }

    #[test]
    fn override_set_replaces_known_and_falls_back() {
        let set = IconSet::from_overrides(&[(Icon::TRASH, "\u{1F5D1}")]);
        assert_eq!(set.glyph(Icon::TRASH), "\u{1F5D1}"); // overridden
        assert_eq!(set.glyph(Icon::EYE),   Icon::EYE);   // falls back to built-in constant
    }
}

/// Initialize fonts + Phosphor icons. Call once during app setup.
pub const FONT_NAMES: &[&str] = &[
    "JetBrains Mono",   // 0 — monospace, default
    "Inter",            // 1 — clean geometric sans
    "Plus Jakarta",     // 2 — modern rounded sans
    "Space Grotesk",    // 3 — geometric wide sans
    "DM Sans",          // 4 — clean dashboard sans
    "Geist",            // 5 — Vercel's app font
];

/// Initialize fonts.
///
/// `font_idx` selects which of the 6 fonts to use as the **proportional** primary
/// (UI chrome, labels, prose). The **monospace** family is ALWAYS pinned to
/// JetBrains Mono regardless of the picker — financial data needs tabular digit
/// alignment for prices, quantities, OCC tickers, and anything that has to line
/// up in columns. This is non-negotiable; the font picker controls proportional
/// UI chrome only.
pub fn init_fonts(ctx: &egui::Context, font_idx: usize) {
    let mut fonts = egui::FontDefinitions::default();

    let tweak_mono = egui::FontTweak {
        scale: 1.0,
        y_offset_factor: -0.02,
        y_offset: 0.0,
        baseline_offset_factor: 0.0,
    };
    let tweak_sans = egui::FontTweak {
        scale: 1.02,
        y_offset_factor: -0.01,
        y_offset: 0.0,
        baseline_offset_factor: 0.0,
    };
    // Source Serif 4 — registered as the named "serif" family used by
    // `hero_font_id()` when `StyleSettings::serif_headlines` is true (#14).
    fonts.font_data.insert("source_serif4_regular".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("SourceSerif4-Regular.ttf"))));
    fonts.font_data.insert("source_serif4_bold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("SourceSerif4-Bold.ttf"))));
    fonts.families.insert(
        egui::FontFamily::Name("serif".into()),
        vec!["source_serif4_bold".into(), "source_serif4_regular".into()],
    );

    fonts.font_data.insert("jetbrains_mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("JetBrainsMono-Regular.ttf")).tweak(tweak_mono)));
    fonts.font_data.insert("inter".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("Inter-Medium.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("plus_jakarta".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("PlusJakartaSans-Medium.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("space_grotesk".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("SpaceGrotesk-Medium.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("dm_sans".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("DMSans-Medium.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("geist".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("Geist-Medium.ttf")).tweak(tweak_sans)));
    // IBM Plex Sans + Mono — the authentic Alto/Mariner UI font (ported from
    // the React mockup's `--ds-font-ui: 'IBM Plex Sans'` / `--ds-font-mono: 'IBM Plex Mono'`).
    fonts.font_data.insert("ibm_plex_sans".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("IBMPlexSans-Regular.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("ibm_plex_sans_sb".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("IBMPlexSans-SemiBold.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("ibm_plex_mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("IBMPlexMono-Regular.ttf")).tweak(tweak_mono)));
    fonts.font_data.insert("ibm_plex_mono_bold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("IBMPlexMono-Bold.ttf")).tweak(tweak_mono)));

    // Multi-weight Inter + JetBrains Mono Bold for real (non-faux) bold/semibold rendering.
    fonts.font_data.insert("inter_regular".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("Inter-Regular.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("inter_semibold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("Inter-SemiBold.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("inter_bold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("Inter-Bold.ttf")).tweak(tweak_sans)));
    fonts.font_data.insert("jetbrains_mono_bold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("JetBrainsMono-Bold.ttf")).tweak(tweak_mono)));

    // Named families so call sites can opt into a specific weight via
    // `RichText::family(FontFamily::Name("inter_semibold".into()))`.
    fonts.families.insert(
        egui::FontFamily::Name("inter_regular".into()),
        vec!["inter_regular".into()],
    );
    fonts.families.insert(
        egui::FontFamily::Name("inter_semibold".into()),
        vec!["inter_semibold".into()],
    );
    fonts.families.insert(
        egui::FontFamily::Name("inter_bold".into()),
        vec!["inter_bold".into()],
    );
    fonts.families.insert(
        egui::FontFamily::Name("jetbrains_mono_bold".into()),
        vec!["jetbrains_mono_bold".into()],
    );

    let primary = match font_idx {
        1 => "inter",
        2 => "plus_jakarta",
        3 => "space_grotesk",
        4 => "dm_sans",
        5 => "geist",
        6 => "ibm_plex_sans",
        _ => "jetbrains_mono",
    };

    // When IBM Plex is active, also slot IBM Plex Mono into the Monospace family
    // for Alto/Mariner's instrument-panel character (source: `--ds-font-mono: 'IBM Plex Mono'`).
    // NOTE: JetBrains Mono is still the fallback so tabular-digit alignment is preserved
    // for financial data; IBM Plex Mono just leads the family for that theme.
    if font_idx == 6 {
        if let Some(mono_keys) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            mono_keys.insert(0, "ibm_plex_mono".into());
        }
    }

    // Proportional: picker font wins (user's choice for UI chrome).
    if let Some(prop_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        prop_keys.insert(0, primary.into());
    }

    // Monospace: ALWAYS JetBrains Mono, regardless of picker.
    // Financial data needs tabular digit alignment; this is non-negotiable.
    if let Some(mono_keys) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        mono_keys.insert(0, "jetbrains_mono".into());
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
