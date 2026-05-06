//! Design tokens runtime — global storage for live-editable design values.
//!
//! When the `design-mode` feature is enabled, this module provides a global
//! DesignTokens struct that UI code reads from. When disabled, all accessors
//! return None and the compiler eliminates the branches.

#[cfg(feature = "design-mode")]
use std::sync::{OnceLock, RwLock};

#[cfg(feature = "design-mode")]
use serde::{Deserialize, Serialize};

// ─── Token struct (only compiled with design-mode) ──────────────────────────

#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignTokens {
    pub font: FontTokens,
    pub spacing: SpacingTokens,
    pub radius: RadiusTokens,
    pub stroke: StrokeTokens,
    pub alpha: AlphaTokens,
    pub shadow: ShadowTokens,
    pub toolbar: ToolbarTokens,
    pub panel: PanelTokens,
    pub dialog: DialogTokens,
    pub button: ButtonTokens,
    pub card: CardTokens,
    pub badge: BadgeTokens,
    pub tab: TabTokens,
    pub table: TableTokens,
    pub chart: ChartTokens,
    pub watchlist: WatchlistTokens,
    pub order_entry: OrderEntryTokens,
    pub pane_header: PaneHeaderTokens,
    pub segmented: SegmentedTokens,
    pub icon_button: IconButtonTokens,
    pub form: FormTokens,
    pub split_divider: SplitDividerTokens,
    pub tooltip: TooltipTokens,
    pub separator: SeparatorTokens,
    pub color: ColorTokens,
    pub status: StatusTokens,
    pub drawing: DrawingTokens,
}

#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontTokens { pub xxs: f32, pub xs: f32, pub sm_tight: f32, pub sm: f32, pub md: f32, pub input: f32, pub lg: f32, pub xl: f32, pub xxl: f32, pub display: f32, pub display_lg: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingTokens { pub xs: f32, pub sm: f32, pub md: f32, pub lg: f32, pub xl: f32, pub xxl: f32, pub xxxl: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadiusTokens { pub xs: f32, pub sm: f32, pub md: f32, pub lg: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokeTokens { pub hair: f32, pub thin: f32, pub std: f32, pub bold: f32, pub thick: f32, pub heavy: f32, pub xheavy: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlphaTokens { pub faint: u8, pub ghost: u8, pub soft: u8, pub subtle: u8, pub tint: u8, pub muted: u8, pub line: u8, pub dim: u8, pub strong: u8, pub active: u8, pub heavy: u8 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowTokens { pub offset: f32, pub alpha: u8, pub spread: f32, pub gradient: [u8; 3] }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolbarTokens { pub height: f32, pub height_compact: f32, pub btn_min_height: f32, pub btn_padding_x: f32, pub right_controls_width: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelTokens { pub margin_x: f32, pub margin_top: f32, pub margin_bottom: f32, pub compact_margin_x: f32, pub compact_margin_top: f32, pub compact_margin_bottom: f32, pub width_sm: f32, pub width_md: f32, pub width_default: f32, pub width_lg: f32, pub width_xl: f32, pub order_width_compact: f32, pub order_width_advanced: f32, pub tooltip_width_sm: f32, pub tooltip_width_md: f32, pub content_width_lg: f32, pub content_width_xl: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogTokens { pub header_darken: u8, pub header_padding_x: f32, pub header_padding_y: f32, pub section_indent: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonTokens { pub action_height: f32, pub trade_height: f32, pub small_height: f32, pub simple_height: f32, pub trade_brightness: f32, pub trade_hover_brightness: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardTokens { pub margin_left: i8, pub margin_right: i8, pub margin_y: i8, pub radius: f32, pub stripe_width: f32, pub width_sm: f32, pub width_md: f32, pub height_sm: f32, pub height_md: f32, pub height_lg: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeTokens { pub font_size: f32, pub height: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabTokens { pub underline_thickness: f32, pub close_width: f32, pub padding_x: f32, pub add_width: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableTokens { pub header_height: f32, pub row_height: f32, pub row_height_compact: f32, pub item_height: f32, pub interact_height: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartTokens { pub right_pad_bars: u32, pub padding_bottom: f32, pub padding_top: f32, pub padding_right: f32, pub replay_height: f32, pub replay_progress_height: f32, pub pnl_strip_height: f32, pub pnl_header_height: f32, pub style_bar_width: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistTokens { pub row_width: f32, pub strip_width: f32, pub strip_width_narrow: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEntryTokens { pub padding: f32, pub pill_width_sm: f32, pub pill_width_md: f32, pub pill_height: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneHeaderTokens { pub height_compact: f32, pub height: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentedTokens { pub trough_darken: u8, pub trough_expand_x: f32, pub btn_padding_x: f32, pub btn_min_height: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconButtonTokens { pub icon_padding: f32, pub min_size: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormTokens { pub label_width: f32, pub row_height: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitDividerTokens { pub height: f32, pub dot_spacing: f32, pub dot_radius: f32, pub dot_count: usize, pub active_stroke: f32, pub inactive_stroke: f32, pub inset: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TooltipTokens { pub corner_radius: f32, pub padding: f32, pub stat_label_size: f32, pub stat_value_size: f32 }
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeparatorTokens { pub after_space: f32, pub shadow_space: f32 }
#[cfg(feature = "design-mode")]
pub type Rgba = [u8; 4];
#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorTokens {
    pub text_primary: Rgba, pub text_secondary: Rgba, pub text_dim: Rgba, pub text_on_accent: Rgba,
    pub amber: Rgba, pub earnings: Rgba, pub paper_orange: Rgba, pub live_green: Rgba,
    pub danger: Rgba, pub triggered_red: Rgba, pub dark_pool: Rgba, pub info_blue: Rgba,
    pub discord: Rgba, pub dialog_fill: Rgba, pub dialog_border: Rgba,
    pub deep_bg: Rgba, pub deep_bg_alt: Rgba,
    pub pane_tints: [Rgba; 4],
}

#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusTokens {
    /// Green — active/live/filled (sampled from pills.rs DisplayChip default).
    pub ok:      Rgba,
    /// Orange/yellow — warning / pending.
    pub warn:    Rgba,
    /// Red — error / rejected.
    pub error:   Rgba,
    /// Blue/purple — informational.
    pub info:    Rgba,
}

#[cfg(feature = "design-mode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawingTokens {
    /// Four identity colors for line-group assignment (blue, green, orange, purple).
    /// Source: painter_pane.rs link_colors array.
    pub palette: [Rgba; 4],
}

// ─── Default + save ─────────────────────────────────────────────────────────

#[cfg(feature = "design-mode")]
impl Default for DesignTokens {
    fn default() -> Self {
        Self {
            font: FontTokens { xxs: 7.0, xs: 8.0, sm_tight: 9.0, sm: 10.0, md: 11.0, input: 12.0, lg: 13.0, xl: 14.0, xxl: 15.0, display: 28.0, display_lg: 36.0 },
            spacing: SpacingTokens { xs: 2.0, sm: 4.0, md: 6.0, lg: 8.0, xl: 10.0, xxl: 12.0, xxxl: 20.0 },
            radius: RadiusTokens { xs: 2.0, sm: 3.0, md: 4.0, lg: 8.0 },
            stroke: StrokeTokens { hair: 0.3, thin: 0.5, std: 1.0, bold: 1.5, thick: 2.0, heavy: 2.5, xheavy: 5.0 },
            alpha: AlphaTokens { faint: 10, ghost: 15, soft: 20, subtle: 25, tint: 30, muted: 40, line: 50, dim: 60, strong: 80, active: 100, heavy: 120 },
            shadow: ShadowTokens { offset: 2.0, alpha: 60, spread: 4.0, gradient: [20, 12, 4] },
            toolbar: ToolbarTokens { height: 36.0, height_compact: 28.0, btn_min_height: 24.0, btn_padding_x: 7.0, right_controls_width: 150.0 },
            panel: PanelTokens { margin_x: 10.0, margin_top: 10.0, margin_bottom: 8.0, compact_margin_x: 8.0, compact_margin_top: 8.0, compact_margin_bottom: 6.0, width_sm: 240.0, width_md: 260.0, width_default: 280.0, width_lg: 300.0, width_xl: 320.0, order_width_compact: 230.0, order_width_advanced: 300.0, tooltip_width_sm: 160.0, tooltip_width_md: 220.0, content_width_lg: 520.0, content_width_xl: 680.0 },
            dialog: DialogTokens { header_darken: 8, header_padding_x: 10.0, header_padding_y: 8.0, section_indent: 10.0 },
            button: ButtonTokens { action_height: 24.0, trade_height: 30.0, small_height: 18.0, simple_height: 20.0, trade_brightness: 0.55, trade_hover_brightness: 0.7 },
            card: CardTokens { margin_left: 9, margin_right: 6, margin_y: 5, radius: 4.0, stripe_width: 3.0, width_sm: 200.0, width_md: 240.0, height_sm: 48.0, height_md: 52.0, height_lg: 120.0 },
            badge: BadgeTokens { font_size: 8.0, height: 16.0 },
            tab: TabTokens { underline_thickness: 2.0, close_width: 14.0, padding_x: 10.0, add_width: 44.0 },
            table: TableTokens { header_height: 12.0, row_height: 20.0, row_height_compact: 18.0, item_height: 36.0, interact_height: 22.0 },
            chart: ChartTokens { right_pad_bars: 20, padding_bottom: 30.0, padding_top: 4.0, padding_right: 80.0, replay_height: 28.0, replay_progress_height: 6.0, pnl_strip_height: 60.0, pnl_header_height: 68.0, style_bar_width: 480.0 },
            watchlist: WatchlistTokens { row_width: 236.0, strip_width: 50.0, strip_width_narrow: 14.0 },
            order_entry: OrderEntryTokens { padding: 8.0, pill_width_sm: 90.0, pill_width_md: 130.0, pill_height: 22.0 },
            pane_header: PaneHeaderTokens { height_compact: 28.0, height: 36.0 },
            segmented: SegmentedTokens { trough_darken: 12, trough_expand_x: 4.0, btn_padding_x: 7.0, btn_min_height: 24.0 },
            icon_button: IconButtonTokens { icon_padding: 5.0, min_size: 26.0 },
            form: FormTokens { label_width: 80.0, row_height: 18.0 },
            split_divider: SplitDividerTokens { height: 6.0, dot_spacing: 8.0, dot_radius: 1.5, dot_count: 3, active_stroke: 2.0, inactive_stroke: 1.0, inset: 8.0 },
            tooltip: TooltipTokens { corner_radius: 8.0, padding: 8.0, stat_label_size: 8.0, stat_value_size: 10.0 },
            separator: SeparatorTokens { after_space: 1.0, shadow_space: 4.0 },
            color: ColorTokens {
                text_primary: [220,220,230,255], text_secondary: [200,200,210,255], text_dim: [180,180,195,255], text_on_accent: [255,255,255,255],
                amber: [255,191,0,255], earnings: [255,193,37,255], paper_orange: [255,165,0,255], live_green: [46,204,113,255],
                danger: [224,85,96,255], triggered_red: [231,76,60,255], dark_pool: [180,100,255,255], info_blue: [100,200,255,255],
                discord: [88,101,242,255], dialog_fill: [26,26,32,255], dialog_border: [60,60,70,80],
                deep_bg: [10,12,16,255], deep_bg_alt: [12,14,18,255],
                pane_tints: [[62,120,180,30], [180,100,255,30], [46,204,113,30], [255,191,0,30]],
            },
            status: StatusTokens {
                ok:    [120, 180, 120, 255], // DisplayChip default — muted green
                warn:  [255, 165,   0, 255], // paper_orange palette entry
                error: [224,  85,  96, 255], // danger palette entry
                info:  [100, 200, 255, 255], // info_blue palette entry
            },
            drawing: DrawingTokens {
                palette: [
                    [ 70, 130, 255, 255], // blue
                    [ 80, 200, 120, 255], // green
                    [255, 160,  60, 255], // orange
                    [180, 100, 255, 255], // purple
                ],
            },
        }
    }
}

#[cfg(feature = "design-mode")]
impl DesignTokens {
    /// Save current tokens to a TOML file.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, content)
    }
}

// ─── Global storage ─────────────────────────────────────────────────────────

#[cfg(feature = "design-mode")]
static DESIGN_TOKENS: OnceLock<RwLock<DesignTokens>> = OnceLock::new();

#[cfg(feature = "design-mode")]
pub fn init(tokens: DesignTokens) {
    let _ = DESIGN_TOKENS.set(RwLock::new(tokens));
}

#[cfg(feature = "design-mode")]
pub fn update(tokens: DesignTokens) {
    if let Some(lock) = DESIGN_TOKENS.get() {
        if let Ok(mut guard) = lock.write() { *guard = tokens; }
    }
}

#[cfg(feature = "design-mode")]
pub fn get() -> Option<DesignTokens> {
    DESIGN_TOKENS.get()?.read().ok().map(|g| g.clone())
}

#[cfg(feature = "design-mode")]
pub fn get_lock() -> Option<&'static RwLock<DesignTokens>> {
    DESIGN_TOKENS.get()
}

#[cfg(feature = "design-mode")]
pub fn is_active() -> bool { DESIGN_TOKENS.get().is_some() }

// ─── No-op stubs when feature is off ────────────────────────────────────────

#[cfg(not(feature = "design-mode"))]
pub fn is_active() -> bool { false }

// ─── Element hit tracking (inspect mode) ────────────────────────────────────
// Each frame, style.rs helpers register their bounding rects + family names.
// The inspector reads these to identify what's under the cursor.

/// A UI element family that was rendered this frame.
#[cfg(feature = "design-mode")]
#[derive(Clone, Debug)]
pub struct ElementHit {
    pub rect: [f32; 4], // [x, y, w, h] — avoids egui dep here
    pub family: &'static str,
    pub category: &'static str, // maps to inspector category
}

#[cfg(feature = "design-mode")]
std::thread_local! {
    static ELEMENT_HITS: std::cell::RefCell<Vec<ElementHit>> = const { std::cell::RefCell::new(Vec::new()) };
    static INSPECT_MODE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Register an element hit for this frame (called by style.rs helpers).
#[cfg(feature = "design-mode")]
pub fn register_hit(rect: [f32; 4], family: &'static str, category: &'static str) {
    ELEMENT_HITS.with(|v| v.borrow_mut().push(ElementHit { rect, family, category }));
}

/// Clear all hits (call at start of each frame).
#[cfg(feature = "design-mode")]
pub fn clear_hits() {
    ELEMENT_HITS.with(|v| v.borrow_mut().clear());
}

/// Get all hits for this frame.
#[cfg(feature = "design-mode")]
pub fn get_hits() -> Vec<ElementHit> {
    ELEMENT_HITS.with(|v| v.borrow().clone())
}

/// Check/set inspect mode.
#[cfg(feature = "design-mode")]
pub fn is_inspect_mode() -> bool {
    INSPECT_MODE.with(|m| m.get())
}

#[cfg(feature = "design-mode")]
pub fn set_inspect_mode(on: bool) {
    INSPECT_MODE.with(|m| m.set(on));
}

/// No-op stub for register_hit when feature is off.
#[cfg(not(feature = "design-mode"))]
#[inline(always)]
pub fn register_hit(_rect: [f32; 4], _family: &'static str, _category: &'static str) {}

/// No-op stub.
#[cfg(not(feature = "design-mode"))]
#[inline(always)]
pub fn is_inspect_mode() -> bool { false }

// ─── Accessor macros ────────────────────────────────────────────────────────
// Usage: `dt_f32!(font.lg, 13.0)` — reads from design tokens if active, else uses literal.

/// Read an f32 token, falling back to the provided default.
#[macro_export]
macro_rules! dt_f32 {
    ($($path:ident).+, $default:expr) => {{
        #[cfg(feature = "design-mode")]
        {
            if let Some(t) = $crate::design_tokens::get() {
                t.$($path).+
            } else {
                $default
            }
        }
        #[cfg(not(feature = "design-mode"))]
        { $default }
    }};
}

/// Read a u8 token.
#[macro_export]
macro_rules! dt_u8 {
    ($($path:ident).+, $default:expr) => {{
        #[cfg(feature = "design-mode")]
        {
            if let Some(t) = $crate::design_tokens::get() {
                t.$($path).+
            } else {
                $default
            }
        }
        #[cfg(not(feature = "design-mode"))]
        { $default }
    }};
}

/// Read a u32 token.
#[macro_export]
macro_rules! dt_u32 {
    ($($path:ident).+, $default:expr) => {{
        #[cfg(feature = "design-mode")]
        {
            if let Some(t) = $crate::design_tokens::get() {
                t.$($path).+
            } else {
                $default
            }
        }
        #[cfg(not(feature = "design-mode"))]
        { $default }
    }};
}

/// Read a usize token.
#[macro_export]
macro_rules! dt_usize {
    ($($path:ident).+, $default:expr) => {{
        #[cfg(feature = "design-mode")]
        {
            if let Some(t) = $crate::design_tokens::get() {
                t.$($path).+
            } else {
                $default
            }
        }
        #[cfg(not(feature = "design-mode"))]
        { $default }
    }};
}

/// Read an i8 token.
#[macro_export]
macro_rules! dt_i8 {
    ($($path:ident).+, $default:expr) => {{
        #[cfg(feature = "design-mode")]
        {
            if let Some(t) = $crate::design_tokens::get() {
                t.$($path).+
            } else {
                $default
            }
        }
        #[cfg(not(feature = "design-mode"))]
        { $default }
    }};
}

/// Read an `[r,g,b,a]` Rgba token, falling back to `$default: [u8;4]`.
/// Returns `egui::Color32::from_rgba_unmultiplied(r,g,b,a)`.
#[macro_export]
macro_rules! dt_rgba {
    ($($path:ident).+, $default:expr) => {{
        #[cfg(feature = "design-mode")]
        {
            let rgba: [u8; 4] = if let Some(t) = $crate::design_tokens::get() {
                t.$($path).+
            } else {
                $default
            };
            egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
        }
        #[cfg(not(feature = "design-mode"))]
        {
            let rgba: [u8; 4] = $default;
            egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
        }
    }};
}

/// Load a design.toml file into the global store.
#[cfg(feature = "design-mode")]
pub fn load_toml(path: &std::path::Path) -> DesignTokens {
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(tokens) => tokens,
            Err(e) => {
                eprintln!("[design-mode] TOML parse error: {e}");
                panic!("Failed to parse design.toml");
            }
        },
        Err(e) => {
            eprintln!("[design-mode] Cannot read {:?}: {e}", path);
            panic!("Failed to read design.toml");
        }
    }
}
