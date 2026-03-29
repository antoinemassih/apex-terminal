//! Chart theme system — 8 themes matching the WebView application.

use egui::Color32;

/// Complete chart theme with all colors needed for rendering.
#[derive(Clone)]
pub struct ChartTheme {
    pub name: &'static str,
    pub bg: Color32,
    pub toolbar_bg: Color32,
    pub toolbar_border: Color32,
    pub bull: Color32,
    pub bear: Color32,
    pub bull_volume: Color32,
    pub bear_volume: Color32,
    pub grid: Color32,
    pub axis_text: Color32,
    pub ohlc_label: Color32,
    pub accent: Color32,
    pub crosshair: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 { Color32::from_rgb(r, g, b) }
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 { Color32::from_rgba_premultiplied(r, g, b, a) }

pub const THEMES: &[ChartTheme] = &[
    ChartTheme { name: "Midnight",
        bg: rgb(13,13,13), toolbar_bg: rgb(17,17,17), toolbar_border: rgb(34,34,34),
        bull: rgb(46,204,113), bear: rgb(231,76,60),
        bull_volume: rgba(46,204,113,64), bear_volume: rgba(231,76,60,64),
        grid: rgba(38,38,38,100), axis_text: rgb(102,102,102), ohlc_label: rgb(204,204,204),
        accent: rgb(42,100,150), crosshair: rgba(255,255,255,50),
    },
    ChartTheme { name: "Nord",
        bg: rgb(46,52,64), toolbar_bg: rgb(46,52,64), toolbar_border: rgb(59,66,82),
        bull: rgb(163,190,140), bear: rgb(191,97,106),
        bull_volume: rgba(163,190,140,64), bear_volume: rgba(191,97,106,64),
        grid: rgba(59,66,82,100), axis_text: rgb(129,161,193), ohlc_label: rgb(216,222,233),
        accent: rgb(136,192,208), crosshair: rgba(255,255,255,50),
    },
    ChartTheme { name: "Monokai",
        bg: rgb(39,40,34), toolbar_bg: rgb(30,31,28), toolbar_border: rgb(62,61,50),
        bull: rgb(166,226,46), bear: rgb(249,38,114),
        bull_volume: rgba(166,226,46,64), bear_volume: rgba(249,38,114,64),
        grid: rgba(62,61,50,100), axis_text: rgb(165,159,133), ohlc_label: rgb(248,248,242),
        accent: rgb(230,219,116), crosshair: rgba(255,255,255,50),
    },
    ChartTheme { name: "Solarized",
        bg: rgb(0,43,54), toolbar_bg: rgb(0,43,54), toolbar_border: rgb(7,54,66),
        bull: rgb(133,153,0), bear: rgb(220,50,47),
        bull_volume: rgba(133,153,0,64), bear_volume: rgba(220,50,47,64),
        grid: rgba(7,54,66,100), axis_text: rgb(131,148,150), ohlc_label: rgb(147,161,161),
        accent: rgb(42,161,152), crosshair: rgba(255,255,255,50),
    },
    ChartTheme { name: "Dracula",
        bg: rgb(40,42,54), toolbar_bg: rgb(33,34,44), toolbar_border: rgb(52,55,70),
        bull: rgb(80,250,123), bear: rgb(255,85,85),
        bull_volume: rgba(80,250,123,64), bear_volume: rgba(255,85,85,64),
        grid: rgba(52,55,70,100), axis_text: rgb(189,147,249), ohlc_label: rgb(248,248,242),
        accent: rgb(255,121,198), crosshair: rgba(255,255,255,50),
    },
    ChartTheme { name: "Gruvbox",
        bg: rgb(40,40,40), toolbar_bg: rgb(29,32,33), toolbar_border: rgb(60,56,54),
        bull: rgb(184,187,38), bear: rgb(251,73,52),
        bull_volume: rgba(184,187,38,64), bear_volume: rgba(251,73,52,64),
        grid: rgba(60,56,54,100), axis_text: rgb(213,196,161), ohlc_label: rgb(235,219,178),
        accent: rgb(254,128,25), crosshair: rgba(255,255,255,50),
    },
    ChartTheme { name: "Catppuccin",
        bg: rgb(30,30,46), toolbar_bg: rgb(24,24,37), toolbar_border: rgb(49,50,68),
        bull: rgb(166,227,161), bear: rgb(243,139,168),
        bull_volume: rgba(166,227,161,64), bear_volume: rgba(243,139,168,64),
        grid: rgba(49,50,68,100), axis_text: rgb(180,190,254), ohlc_label: rgb(205,214,244),
        accent: rgb(203,166,247), crosshair: rgba(255,255,255,50),
    },
    ChartTheme { name: "Tokyo Night",
        bg: rgb(26,27,38), toolbar_bg: rgb(22,22,30), toolbar_border: rgb(36,40,59),
        bull: rgb(158,206,106), bear: rgb(247,118,142),
        bull_volume: rgba(158,206,106,64), bear_volume: rgba(247,118,142,64),
        grid: rgba(36,40,59,100), axis_text: rgb(122,162,247), ohlc_label: rgb(192,202,245),
        accent: rgb(125,207,255), crosshair: rgba(255,255,255,50),
    },
];

/// Preset drawing colors
pub const DRAW_COLORS: &[(&str, Color32)] = &[
    ("#4a9eff", rgb(74,158,255)),
    ("#e74c3c", rgb(231,76,60)),
    ("#2ecc71", rgb(46,204,113)),
    ("#f39c12", rgb(243,156,18)),
    ("#9b59b6", rgb(155,89,182)),
    ("#1abc9c", rgb(26,188,156)),
    ("#ffffff", rgb(255,255,255)),
    ("#e67e22", rgb(230,126,34)),
];

/// Spacing constants
pub const TOOLBAR_HEIGHT: f32 = 28.0;
pub const PADDING_RIGHT: f32 = 80.0;
pub const PADDING_TOP: f32 = 4.0;
pub const PADDING_BOTTOM: f32 = 30.0;
pub const STYLE_BAR_WIDTH: f32 = 480.0;
