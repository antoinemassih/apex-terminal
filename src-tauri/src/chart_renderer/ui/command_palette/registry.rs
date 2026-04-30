//! Registered actions/commands list and pretty-label helpers.

use super::{Category, Entry};
use crate::chart_renderer::gpu::*;
use crate::chart_renderer::ChartWidgetKind;

pub(super) const THEME_NAMES: &[&str] = &[
    "Midnight", "Nord", "Monokai", "Solarized", "Dracula", "Gruvbox",
    "Catppuccin", "Tokyo Night", "Kanagawa", "Everforest", "Vesper", "Rosé Pine",
    "Bauhaus", "Peach", "Ivory",
];

pub(super) const TF_IDS: &[&str] = &["1m","5m","15m","30m","1h","2h","4h","1D","1W","1M"];

pub(super) const LAYOUT_IDS: &[(&str, &str)] = &[
    ("1","Single pane"),("2","Two panes H"),("2H","Two panes V"),
    ("3","Three panes"),("3L","3 L-shape"),("4","Quad"),("4L","4 L-shape"),
    ("5C","5 centered"),("6","Six panes"),("9","Nine-up"),
];

pub(super) const OVERLAY_IDS: &[(&str, &str)] = &[
    ("vol-shelves",   "Volume Shelves"),
    ("confluence",    "Confluence Zones"),
    ("momentum",      "Momentum Heatmap"),
    ("trend-strip",   "Trend Strip"),
    ("breadth",       "Breadth Tint"),
    ("vol-cone",      "Vol Cone"),
    ("price-memory",  "Price Memory"),
    ("liquidity",     "Liquidity Voids"),
    ("corr-ribbon",   "Correlation Ribbon"),
    ("analyst",       "Analyst Targets"),
    ("pe-band",       "PE Band"),
    ("insider",       "Insider Trades"),
];

pub(super) fn widget_catalog() -> Vec<(ChartWidgetKind, &'static str, &'static str)> {
    use ChartWidgetKind::*;
    vec![
        (RsiMulti,          "rsi-multi",        "RSI Multi"),
        (TrendAlign,        "trend-align",      "TrendAlign"),
        (VolumeShelf,       "volume-shelf",     "VolumeShelf"),
        (Confluence,        "confluence",       "Confluence"),
        (FlowCompass,       "flow-compass",     "FlowCompass"),
        (VolRegime,         "vol-regime",       "VolRegime"),
        (MomentumHeat,      "momentum-heat",    "MomentumHeat"),
        (BreadthThermo,     "breadth-thermo",   "BreadthThermo"),
        (SectorRotation,    "sector-rotation",  "SectorRotation"),
        (OptionsSentiment,  "options-sent",     "OptionsSentiment"),
        (RelStrength,       "rel-strength",     "RelStrength"),
        (RiskDash,          "risk-dash",        "RiskDash"),
        (EarningsMom,       "earnings-mom",     "EarningsMom"),
        (LiquidityScore,    "liquidity-score",  "LiquidityScore"),
        (SignalRadar,       "signal-radar",     "SignalRadar"),
        (CrossAssetPulse,   "cross-asset",      "CrossAssetPulse"),
        (TapeSpeed,         "tape-speed",       "TapeSpeed"),
        (PayoffChart,       "payoff",           "PayoffChart"),
        (OptionsFlow,       "options-flow",     "OptionsFlow"),
        (EconCalendar,      "econ-cal",         "EconCalendar"),
        (Latency,           "latency",          "Latency"),
        (TrendStrength,     "trend-strength",   "Trend Strength"),
        (Momentum,          "momentum",         "Momentum"),
        (Volatility,        "volatility",       "Volatility"),
        (KeyLevels,         "key-levels",       "Key Levels"),
        (Fundamentals,      "fundamentals",     "Fundamentals"),
        (PositionPnl,       "position-pnl",     "Position P&L"),
        (DailyPnl,          "daily-pnl",        "Daily P&L"),
        (VixMonitor,        "vix",              "VIX Monitor"),
        (MarketBreadth,     "market-breadth",   "Market Breadth"),
    ]
}

pub(super) fn widget_kind_from_id(id: &str) -> Option<ChartWidgetKind> {
    widget_catalog().into_iter().find(|(_, i, _)| *i == id).map(|(k,_,_)| k)
}

pub(super) fn build_registry(watchlist: &Watchlist, active_pane_type: PaneType) -> Vec<Entry> {
    let mut v: Vec<Entry> = Vec::new();
    let mk = |id: &str, label: &str, desc: &str, cat: Category, hk: Option<&str>| -> Entry {
        Entry { id: id.into(), label: label.into(), desc: desc.into(), cat, hotkey: hk.map(|s| s.to_string()) }
    };

    // Hero
    v.push(mk("ai:chat",         "Ask Gemma",                     "Natural-language chat — scanners, alerts, context", Category::Ai, Some("Tab")));
    v.push(mk("dyn:reorganize",  "Reorganize layout (Dynamic UI)","Let Gemma 2B rearrange panels for the current task",  Category::Dynamic, None));

    // Risk / trading
    v.push(mk("cmd:flatten",   "Flatten all positions", "Close every open position via broker", Category::Command, Some("Ctrl+Shift+F")));
    v.push(mk("cmd:cancel",    "Cancel all orders",     "Cancel every working order",           Category::Command, Some("Ctrl+Shift+C")));
    v.push(mk("cmd:reverse",   "Reverse position",      "Flip side on active symbol",           Category::Command, None));
    v.push(mk("cmd:halfsize",  "Halve position",        "Reduce active position by 50%",        Category::Command, None));

    // Layout presets
    for (id, d) in LAYOUT_IDS {
        v.push(mk(&format!("layout:{id}"), &format!("Layout · {id}"), d, Category::Layout, None));
    }

    // Themes
    for name in THEME_NAMES {
        v.push(mk(&format!("theme:{}", name.to_lowercase()), &format!("Theme · {name}"), "Switch global theme", Category::Theme, None));
    }

    // Timeframes
    for tf in TF_IDS {
        v.push(mk(&format!("tf:{tf}"), &format!("Timeframe · {tf}"), "Set active chart timeframe", Category::Timeframe, None));
    }

    // Overlays
    for (id, label) in OVERLAY_IDS {
        v.push(mk(&format!("overlay:{id}"), &format!("Toggle · {label}"), "Chart overlay", Category::Overlay, None));
    }

    // Widgets (with display name)
    for (_, id, label) in widget_catalog() {
        v.push(mk(&format!("widget:{id}"), &format!("Add widget · {label}"), "Add to active pane", Category::Widget, None));
    }

    // Settings
    v.push(mk("setting:hotkeys",     "Edit hotkeys",         "Open hotkey editor",  Category::Setting, None));
    v.push(mk("setting:settings",    "Settings",             "Open settings panel", Category::Setting, None));
    v.push(mk("setting:apex-diag",   "ApexData diagnostics", "Live view of REST/WS/chain state", Category::Setting, None));
    v.push(mk("setting:workspace",   "Save workspace…",      "Save current layout", Category::Setting, None));
    v.push(mk("setting:pane-chart",     "Pane type · Chart",     "Switch active pane to Chart",     Category::Setting, None));
    v.push(mk("setting:pane-portfolio", "Pane type · Portfolio", "Switch active pane to Portfolio", Category::Setting, None));
    v.push(mk("setting:pane-dashboard", "Pane type · Dashboard", "Switch active pane to Dashboard", Category::Setting, None));
    v.push(mk("setting:pane-heatmap",   "Pane type · Heatmap",   "Switch active pane to Heatmap",   Category::Setting, None));

    // Help
    v.push(mk("help:prefixes", "Prefix modes — > @ # / ? =", "Quick reference", Category::Help, None));
    v.push(mk("help:widgets",  "List all widgets",           "`? widgets`",     Category::Help, None));
    v.push(mk("help:overlays", "List all overlays",          "`? overlays`",    Category::Help, None));

    // Dynamic: plays
    for p in watchlist.plays.iter().take(30) {
        v.push(mk(
            &format!("play:{}", p.id),
            &format!("Play · {}", p.title),
            &format!("{} · {} @ {}", p.symbol, match p.direction { crate::chart_renderer::PlayDirection::Long => "long", crate::chart_renderer::PlayDirection::Short => "short" }, p.entry_price),
            Category::Play,
            None,
        ));
    }

    // Dynamic: alerts
    for a in watchlist.alerts.iter().take(30) {
        v.push(mk(
            &format!("alert:{}", a.id),
            &format!("Alert · {} {} {}", a.symbol, if a.above {">"} else {"<"}, a.price),
            if a.triggered { "triggered" } else { "armed" },
            Category::Alert,
            None,
        ));
    }

    // Contextual boost: duplicate a few most-relevant entries at the top as "suggested"
    // (handled at display time via pane_type argument)
    let _ = active_pane_type;

    v
}

pub(super) fn hotkey_for(id: &str) -> Option<&'static str> {
    match id {
        "cmd:flatten" => Some("Ctrl+Shift+F"),
        "cmd:cancel"  => Some("Ctrl+Shift+C"),
        "ai:chat"     => Some("Tab"),
        "setting:settings" => Some("Ctrl+,"),
        _ => None,
    }
}

pub(super) fn pretty_label_for_id(id: &str, watchlist: &Watchlist) -> Option<String> {
    if let Some(sym) = id.strip_prefix("sym:") { return Some(sym.to_string()); }
    if let Some(name) = id.strip_prefix("theme:") { return Some(format!("Theme · {name}")); }
    if let Some(tf) = id.strip_prefix("tf:") { return Some(format!("Timeframe · {tf}")); }
    if let Some(ly) = id.strip_prefix("layout:") { return Some(format!("Layout · {ly}")); }
    if let Some(w) = id.strip_prefix("widget:") {
        if let Some((_, _, label)) = widget_catalog().iter().find(|(_, i, _)| *i == w) {
            return Some(format!("Add widget · {label}"));
        }
    }
    if let Some(pid) = id.strip_prefix("play:") {
        if let Some(p) = watchlist.plays.iter().find(|p| p.id == pid) {
            return Some(format!("Play · {}", p.title));
        }
    }
    None
}

pub(super) fn read_clipboard_ticker() -> Option<String> {
    // Clipboard integration requires adding `arboard` to Cargo.toml — skipped for now.
    None
}
