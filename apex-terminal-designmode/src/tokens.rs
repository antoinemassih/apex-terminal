//! DesignTokens — every visual property in the application as a named, editable value.
//!
//! Organized by component family so editing one token updates every instance
//! of that family across the entire app.

use serde::{Deserialize, Serialize};

/// Color as [r, g, b, a] for TOML compatibility.
pub type Rgba = [u8; 4];

pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba { [r, g, b, a] }
pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba { [r, g, b, 255] }

/// Convert Rgba to egui::Color32.
pub fn to_color32(c: Rgba) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

// ─────────────────────────────────────────────────────────────────────────────
// Top-level token struct
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignTokens {
    pub font: FontTokens,
    pub spacing: SpacingTokens,
    pub radius: RadiusTokens,
    pub stroke: StrokeTokens,
    pub alpha: AlphaTokens,
    pub shadow: ShadowTokens,
    pub color: ColorTokens,
    pub toolbar: ToolbarTokens,
    pub panel: PanelTokens,
    pub dialog: DialogTokens,
    pub button: ButtonTokens,
    pub card: CardTokens,
    pub badge: BadgeTokens,
    pub tab: TabTokens,
    pub table: TableTokens,
    pub separator: SeparatorTokens,
    pub tooltip: TooltipTokens,
    pub chart: ChartTokens,
    pub watchlist: WatchlistTokens,
    pub order_entry: OrderEntryTokens,
    pub pane_header: PaneHeaderTokens,
    pub segmented: SegmentedTokens,
    pub icon_button: IconButtonTokens,
    pub form: FormTokens,
    pub split_divider: SplitDividerTokens,
}

// ─────────────────────────────────────────────────────────────────────────────
// Font family
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontTokens {
    /// 2xs — micro labels on chart painter, tick labels (currently 7.0)
    pub xxs: f32,
    /// xs — column headers, status chips, tiny badges (8.0)
    pub xs: f32,
    /// sm-tight — compact rows, watchlist rows (9.0, currently no token)
    pub sm_tight: f32,
    /// sm — body text, labels, most buttons (10.0)
    pub sm: f32,
    /// md — panel section headers (11.0)
    pub md: f32,
    /// input — form inputs, quantity/price fields (12.0, currently no token)
    pub input: f32,
    /// lg — primary headings, toolbar buttons (13.0)
    pub lg: f32,
    /// xl — large price values in cards (14.0)
    pub xl: f32,
    /// 2xl — featured prices, big display (15.0)
    pub xxl: f32,
    /// display — hero numbers, large gauges (28.0)
    pub display: f32,
    /// display_lg — biggest display text (36.0)
    pub display_lg: f32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Spacing family
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingTokens {
    pub xs: f32,   // 2.0
    pub sm: f32,   // 4.0
    pub md: f32,   // 6.0
    pub lg: f32,   // 8.0
    pub xl: f32,   // 10.0
    pub xxl: f32,  // 12.0
    pub xxxl: f32, // 20.0
}

// ─────────────────────────────────────────────────────────────────────────────
// Radius family
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadiusTokens {
    pub xs: f32,  // 2.0 — hairline rects in painter
    pub sm: f32,  // 3.0 — small buttons, badges, chips
    pub md: f32,  // 4.0 — primary buttons, cards
    pub lg: f32,  // 8.0 — dialogs, panels, modals
}

// ─────────────────────────────────────────────────────────────────────────────
// Stroke family
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokeTokens {
    pub hair: f32,   // 0.3 — ultra-fine grid/DOM separators
    pub thin: f32,   // 0.5 — separators, card borders, badges
    pub std: f32,    // 1.0 — panel frames, dialog windows
    pub bold: f32,   // 1.5 — emphasis outlines
    pub thick: f32,  // 2.0 — tab underlines, accent stripes
    pub heavy: f32,  // 2.5 — extra bold chart elements
    pub xheavy: f32, // 5.0 — maximum emphasis
}

// ─────────────────────────────────────────────────────────────────────────────
// Alpha family
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlphaTokens {
    pub faint: u8,   // 10
    pub ghost: u8,   // 15
    pub soft: u8,    // 20
    pub subtle: u8,  // 25
    pub tint: u8,    // 30
    pub muted: u8,   // 40
    pub line: u8,    // 50
    pub dim: u8,     // 60
    pub strong: u8,  // 80
    pub active: u8,  // 100
    pub heavy: u8,   // 120
}

// ─────────────────────────────────────────────────────────────────────────────
// Shadow family
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowTokens {
    pub offset: f32,  // 2.0
    pub alpha: u8,    // 60
    pub spread: f32,  // 4.0
    /// Fading shadow gradient alphas (3 tiers below separators)
    pub gradient: [u8; 3], // [20, 12, 4]
}

// ─────────────────────────────────────────────────────────────────────────────
// Semantic color family (theme-independent fixed colors)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorTokens {
    /// Primary text on dark bg
    pub text_primary: Rgba,      // [220, 220, 230, 255]
    /// Secondary text on dark bg
    pub text_secondary: Rgba,    // [200, 200, 210, 255]
    /// Dimmed text
    pub text_dim: Rgba,          // [180, 180, 195, 255]
    /// White (for high-contrast on colored bg)
    pub text_on_accent: Rgba,    // [255, 255, 255, 255]

    /// Amber — warnings, neutral tier, EXT indicator
    pub amber: Rgba,             // [255, 191, 0, 255]
    /// Earnings alert color
    pub earnings: Rgba,          // [255, 193, 37, 255]
    /// Paper trading badge
    pub paper_orange: Rgba,      // [255, 165, 0, 255]
    /// Live status green
    pub live_green: Rgba,        // [46, 204, 113, 255]
    /// Cancel/danger red
    pub danger: Rgba,            // [224, 85, 96, 255]
    /// Triggered alert red
    pub triggered_red: Rgba,     // [231, 76, 60, 255]
    /// Dark pool purple
    pub dark_pool: Rgba,         // [180, 100, 255, 255]
    /// Blue accent (greeks, breadth)
    pub info_blue: Rgba,         // [100, 200, 255, 255]
    /// Discord blurple
    pub discord: Rgba,           // [88, 101, 242, 255]

    /// Dialog fill background (dark)
    pub dialog_fill: Rgba,       // [26, 26, 32, 255]
    /// Dialog border
    pub dialog_border: Rgba,     // [60, 60, 70, 80]
    /// Near-black background (DOM panel)
    pub deep_bg: Rgba,           // [10, 12, 16, 255]
    /// Slightly lighter deep bg
    pub deep_bg_alt: Rgba,       // [12, 14, 18, 255]

    /// Pane tint colors (4 presets for multi-pane identification)
    pub pane_tints: [Rgba; 4],
}

// ─────────────────────────────────────────────────────────────────────────────
// Component families
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolbarTokens {
    pub height: f32,         // 28.0 (compact) or 36.0
    pub height_compact: f32, // 28.0
    pub btn_min_height: f32, // 24.0
    pub btn_padding_x: f32,  // 7.0 (button_padding.x)
    pub right_controls_width: f32, // 150.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelTokens {
    /// Standard side panel inner margin (left/right)
    pub margin_x: f32,       // 10.0 (GAP_XL)
    /// Standard side panel inner margin (top)
    pub margin_top: f32,     // 10.0
    /// Standard side panel inner margin (bottom)
    pub margin_bottom: f32,  // 8.0
    /// Compact panel margin (left/right)
    pub compact_margin_x: f32, // 8.0
    /// Compact panel margin (top)
    pub compact_margin_top: f32, // 8.0
    /// Compact panel margin (bottom)
    pub compact_margin_bottom: f32, // 6.0
    /// Default panel width (general)
    pub width_sm: f32,        // 240.0
    pub width_md: f32,        // 260.0
    pub width_default: f32,   // 280.0
    pub width_lg: f32,        // 300.0
    pub width_xl: f32,        // 320.0
    /// Order entry panel width (compact)
    pub order_width_compact: f32, // 230.0
    /// Order entry panel width (advanced)
    pub order_width_advanced: f32, // 300.0
    /// Tooltip width
    pub tooltip_width_sm: f32, // 160.0
    pub tooltip_width_md: f32, // 220.0
    /// Floating content width
    pub content_width_lg: f32, // 520.0
    pub content_width_xl: f32, // 680.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogTokens {
    /// Header darkening offset (subtracted from bg)
    pub header_darken: u8,    // 8
    /// Header inner padding
    pub header_padding_x: f32, // 10.0
    pub header_padding_y: f32, // 8.0
    /// Section margin indent
    pub section_indent: f32,    // varies
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonTokens {
    /// Action button min height
    pub action_height: f32,     // 24.0
    /// Trade button min height
    pub trade_height: f32,      // 30.0
    /// Small action button min height
    pub small_height: f32,      // 18.0
    /// Simple button min height
    pub simple_height: f32,     // 20.0
    /// Trade button brightness factor (normal)
    pub trade_brightness: f32,  // 0.55
    /// Trade button brightness factor (hover)
    pub trade_hover_brightness: f32, // 0.7
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardTokens {
    /// Order card inner margin (left)
    pub margin_left: i8,     // 9
    /// Order card inner margin (right)
    pub margin_right: i8,    // 6
    /// Order card inner margin (top/bottom)
    pub margin_y: i8,        // 5
    /// Order card corner radius
    pub radius: f32,         // 4.0
    /// Left accent stripe width
    pub stripe_width: f32,   // 3.0
    /// Card widths
    pub width_sm: f32,       // 200.0
    pub width_md: f32,       // 240.0
    /// Card heights
    pub height_sm: f32,      // 48.0
    pub height_md: f32,      // 52.0
    pub height_lg: f32,      // 120.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeTokens {
    /// Badge font size
    pub font_size: f32,      // 8.0
    /// Badge min height
    pub height: f32,         // 16.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabTokens {
    /// Active tab underline thickness
    pub underline_thickness: f32, // 2.0
    /// Close button width (pane tabs)
    pub close_width: f32,         // 14.0
    /// Tab horizontal padding
    pub padding_x: f32,           // 10.0
    /// Add-tab (+) button width
    pub add_width: f32,           // 44.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableTokens {
    /// Column header height
    pub header_height: f32,   // 12.0
    /// Standard row height
    pub row_height: f32,      // 20.0
    /// Compact row height
    pub row_height_compact: f32, // 18.0
    /// Item height (list items, search results)
    pub item_height: f32,     // 36.0
    /// Interaction target height (egui interact_size.y)
    pub interact_height: f32, // 22.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeparatorTokens {
    /// Space added after separator
    pub after_space: f32, // 1.0
    /// Shadow gradient space
    pub shadow_space: f32, // GAP_SM (4.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TooltipTokens {
    pub corner_radius: f32,  // RADIUS_LG (8.0)
    pub padding: f32,        // GAP_LG (8.0)
    /// Stat row label font size
    pub stat_label_size: f32,  // FONT_XS (8.0)
    /// Stat row value font size
    pub stat_value_size: f32,  // FONT_SM (10.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartTokens {
    /// Right padding in bars (empty space after latest)
    pub right_pad_bars: u32,       // 20
    /// Chart bottom padding (price axis)
    pub padding_bottom: f32,       // 30.0
    /// Chart top padding
    pub padding_top: f32,          // 4.0
    /// Chart right padding (price label area)
    pub padding_right: f32,        // 80.0
    /// Replay control bar height
    pub replay_height: f32,        // 28.0
    /// Replay progress bar height
    pub replay_progress_height: f32, // 6.0
    /// PnL strip height
    pub pnl_strip_height: f32,    // 60.0
    /// PnL strip header height
    pub pnl_header_height: f32,   // 68.0
    /// Style bar width
    pub style_bar_width: f32,     // 480.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistTokens {
    /// Watchlist row width
    pub row_width: f32,       // 236.0
    /// Row strip width (mini indicator)
    pub strip_width: f32,     // 50.0
    /// Narrow strip width (spark lines)
    pub strip_width_narrow: f32, // 14.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEntryTokens {
    /// Order entry inner padding
    pub padding: f32,         // 8.0
    /// Pill width (compact)
    pub pill_width_sm: f32,   // 90.0
    /// Pill width (standard)
    pub pill_width_md: f32,   // 130.0
    /// Pill height
    pub pill_height: f32,     // 22.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneHeaderTokens {
    /// Pane header height (compact)
    pub height_compact: f32,  // 28.0
    /// Pane header height (standard)
    pub height: f32,          // 36.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentedTokens {
    /// Trough darkening (subtracted from toolbar bg)
    pub trough_darken: u8,    // 12
    /// Trough expand padding
    pub trough_expand_x: f32, // 4.0
    /// Segment button padding x
    pub btn_padding_x: f32,   // 7.0
    /// Segment min height
    pub btn_min_height: f32,  // 24.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconButtonTokens {
    /// Padding around icon (each side)
    pub icon_padding: f32,    // 5.0
    /// Minimum square size
    pub min_size: f32,        // 26.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormTokens {
    /// Label column width
    pub label_width: f32,     // varies, param
    /// Row height
    pub row_height: f32,      // 18.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitDividerTokens {
    /// Divider hit area height
    pub height: f32,          // 6.0
    /// Dot spacing
    pub dot_spacing: f32,     // 8.0
    /// Dot radius
    pub dot_radius: f32,      // 1.5
    /// Number of dots
    pub dot_count: usize,     // 3
    /// Active stroke width
    pub active_stroke: f32,   // 2.0
    /// Inactive stroke width
    pub inactive_stroke: f32, // 1.0
    /// Inset from edges
    pub inset: f32,           // 8.0
}

// ─────────────────────────────────────────────────────────────────────────────
// Defaults (current hardcoded values)
// ─────────────────────────────────────────────────────────────────────────────

impl Default for DesignTokens {
    fn default() -> Self {
        Self {
            font: FontTokens {
                xxs: 7.0, xs: 8.0, sm_tight: 9.0, sm: 10.0, md: 11.0,
                input: 12.0, lg: 13.0, xl: 14.0, xxl: 15.0,
                display: 28.0, display_lg: 36.0,
            },
            spacing: SpacingTokens {
                xs: 2.0, sm: 4.0, md: 6.0, lg: 8.0, xl: 10.0, xxl: 12.0, xxxl: 20.0,
            },
            radius: RadiusTokens {
                xs: 2.0, sm: 3.0, md: 4.0, lg: 8.0,
            },
            stroke: StrokeTokens {
                hair: 0.3, thin: 0.5, std: 1.0, bold: 1.5, thick: 2.0, heavy: 2.5, xheavy: 5.0,
            },
            alpha: AlphaTokens {
                faint: 10, ghost: 15, soft: 20, subtle: 25, tint: 30,
                muted: 40, line: 50, dim: 60, strong: 80, active: 100, heavy: 120,
            },
            shadow: ShadowTokens {
                offset: 2.0, alpha: 60, spread: 4.0, gradient: [20, 12, 4],
            },
            color: ColorTokens {
                text_primary: rgb(220, 220, 230),
                text_secondary: rgb(200, 200, 210),
                text_dim: rgb(180, 180, 195),
                text_on_accent: rgb(255, 255, 255),
                amber: rgb(255, 191, 0),
                earnings: rgb(255, 193, 37),
                paper_orange: rgb(255, 165, 0),
                live_green: rgb(46, 204, 113),
                danger: rgb(224, 85, 96),
                triggered_red: rgb(231, 76, 60),
                dark_pool: rgb(180, 100, 255),
                info_blue: rgb(100, 200, 255),
                discord: rgb(88, 101, 242),
                dialog_fill: rgb(26, 26, 32),
                dialog_border: rgba(60, 60, 70, 80),
                deep_bg: rgb(10, 12, 16),
                deep_bg_alt: rgb(12, 14, 18),
                pane_tints: [
                    rgba(62, 120, 180, 30),
                    rgba(180, 100, 255, 30),
                    rgba(46, 204, 113, 30),
                    rgba(255, 191, 0, 30),
                ],
            },
            toolbar: ToolbarTokens {
                height: 36.0, height_compact: 28.0, btn_min_height: 24.0,
                btn_padding_x: 7.0, right_controls_width: 150.0,
            },
            panel: PanelTokens {
                margin_x: 10.0, margin_top: 10.0, margin_bottom: 8.0,
                compact_margin_x: 8.0, compact_margin_top: 8.0, compact_margin_bottom: 6.0,
                width_sm: 240.0, width_md: 260.0, width_default: 280.0,
                width_lg: 300.0, width_xl: 320.0,
                order_width_compact: 230.0, order_width_advanced: 300.0,
                tooltip_width_sm: 160.0, tooltip_width_md: 220.0,
                content_width_lg: 520.0, content_width_xl: 680.0,
            },
            dialog: DialogTokens {
                header_darken: 8, header_padding_x: 10.0, header_padding_y: 8.0,
                section_indent: 10.0,
            },
            button: ButtonTokens {
                action_height: 24.0, trade_height: 30.0,
                small_height: 18.0, simple_height: 20.0,
                trade_brightness: 0.55, trade_hover_brightness: 0.7,
            },
            card: CardTokens {
                margin_left: 9, margin_right: 6, margin_y: 5,
                radius: 4.0, stripe_width: 3.0,
                width_sm: 200.0, width_md: 240.0,
                height_sm: 48.0, height_md: 52.0, height_lg: 120.0,
            },
            badge: BadgeTokens { font_size: 8.0, height: 16.0 },
            tab: TabTokens {
                underline_thickness: 2.0, close_width: 14.0,
                padding_x: 10.0, add_width: 44.0,
            },
            table: TableTokens {
                header_height: 12.0, row_height: 20.0, row_height_compact: 18.0,
                item_height: 36.0, interact_height: 22.0,
            },
            separator: SeparatorTokens { after_space: 1.0, shadow_space: 4.0 },
            tooltip: TooltipTokens {
                corner_radius: 8.0, padding: 8.0,
                stat_label_size: 8.0, stat_value_size: 10.0,
            },
            chart: ChartTokens {
                right_pad_bars: 20, padding_bottom: 30.0, padding_top: 4.0,
                padding_right: 80.0, replay_height: 28.0, replay_progress_height: 6.0,
                pnl_strip_height: 60.0, pnl_header_height: 68.0,
                style_bar_width: 480.0,
            },
            watchlist: WatchlistTokens {
                row_width: 236.0, strip_width: 50.0, strip_width_narrow: 14.0,
            },
            order_entry: OrderEntryTokens {
                padding: 8.0, pill_width_sm: 90.0, pill_width_md: 130.0, pill_height: 22.0,
            },
            pane_header: PaneHeaderTokens { height_compact: 28.0, height: 36.0 },
            segmented: SegmentedTokens {
                trough_darken: 12, trough_expand_x: 4.0,
                btn_padding_x: 7.0, btn_min_height: 24.0,
            },
            icon_button: IconButtonTokens { icon_padding: 5.0, min_size: 26.0 },
            form: FormTokens { label_width: 80.0, row_height: 18.0 },
            split_divider: SplitDividerTokens {
                height: 6.0, dot_spacing: 8.0, dot_radius: 1.5, dot_count: 3,
                active_stroke: 2.0, inactive_stroke: 1.0, inset: 8.0,
            },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TOML serialization
// ─────────────────────────────────────────────────────────────────────────────

impl DesignTokens {
    /// Load from a TOML file, falling back to defaults for missing fields.
    pub fn load(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(tokens) => tokens,
                Err(e) => {
                    eprintln!("[design-mode] TOML parse error: {e}");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Save to a TOML file.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, content)
    }

    /// Generate the initial design.toml with all current defaults.
    pub fn write_defaults(path: &std::path::Path) -> std::io::Result<()> {
        Self::default().save(path)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Global accessor (for the core app to read tokens at runtime)
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::{OnceLock, RwLock};

static DESIGN_TOKENS: OnceLock<RwLock<DesignTokens>> = OnceLock::new();

/// Initialize the global design tokens (call once at startup).
pub fn init_tokens(tokens: DesignTokens) {
    let _ = DESIGN_TOKENS.set(RwLock::new(tokens));
}

/// Update the global design tokens (called by file watcher on TOML change).
pub fn update_tokens(tokens: DesignTokens) {
    if let Some(lock) = DESIGN_TOKENS.get() {
        if let Ok(mut guard) = lock.write() {
            *guard = tokens;
        }
    }
}

/// Read a value from the global design tokens.
/// Returns None if design mode is not active.
pub fn get_tokens() -> Option<DesignTokens> {
    DESIGN_TOKENS.get()?.read().ok().map(|g| g.clone())
}

/// Check if design mode is active.
pub fn is_active() -> bool {
    DESIGN_TOKENS.get().is_some()
}
