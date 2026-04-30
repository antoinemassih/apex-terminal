//! Native GPU chart renderer — wgpu + winit, zero browser overhead.
//!
//! Architecture:
//! - Spawns a native OS window (winit) on a dedicated thread
//! - Renders candlesticks, volume, indicators via wgpu (same WGSL shaders as WebGPU frontend)
//! - Receives bar data + viewport commands from Tauri WebView via channels
//! - Runs its own render loop at monitor vsync — no rAF, no compositor, no DOM
//!
//! Communication with WebView:
//! - ChartCommand enum sent via crossbeam channel from Tauri commands
//! - Render thread processes commands between frames (non-blocking)

pub mod gpu;
mod types;
pub mod ui;
pub mod compute;
pub mod trading;
pub mod commands;

pub use types::*;

/// Tab selector for the unified Analysis sidebar.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum AnalysisTab { Rrg, TimeSales, Scanner, Scripts, Seasonality, Research }

/// Tab selector for the unified Signals sidebar (Alerts + Signals).
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SignalsTab { Alerts, Signals }

/// Tab selector for the unified Feed sidebar (News + Discord + Screenshots).
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum FeedTab { News, Discord, Screenshots }

// ─── Chart Widgets (floating info cards on the chart canvas) ─────────────────

/// Widget type — what kind of data/visualization the widget shows.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ChartWidgetKind {
    TrendStrength,    // gauge: trend health score + direction
    Momentum,         // gauge: momentum oscillator composite
    Volatility,       // gauge: ATR / VIX / realized vol
    VolumeProfile,    // mini volume-at-price heatmap
    SessionTimer,     // countdown to market open/close
    KeyLevels,        // S/R levels with distance from price
    OptionGreeks,     // delta/gamma/theta/vega for active option
    RiskReward,       // current position R:R ratio
    MarketBreadth,    // advance/decline, new highs/lows
    Correlation,      // live correlation vs SPY/QQQ/sector
    DarkPool,         // unusual volume / dark pool prints
    PositionPnl,      // live unrealized P&L for active position
    EarningsBadge,    // countdown chip for upcoming earnings
    NewsTicker,       // scrolling headline strip, sentiment-colored
    // ── ApexSignals widgets ──
    ExitGauge,        // position exit urgency (hold → exit_now)
    PrecursorAlert,   // smart money / unusual options detection
    TradePlan,        // entry / target / stop suggestion card
    ChangePoints,     // regime shift detection timeline
    ZoneStrength,     // supply/demand zone health summary
    PatternScanner,   // recent candlestick patterns with confidence
    VixMonitor,       // VIX term structure / convergence
    SignalDashboard,  // aggregate all active signals overview
    DivergenceMonitor,// active indicator divergences
    ConvictionMeter,  // aggregate conviction score from all signals
    RsiMulti,         // concentric rings RSI across 7 timeframes
    TrendAlign,       // multi-TF trend alignment dot grid
    VolumeShelf,      // volume shelf map — ranked S/R levels
    Confluence,       // S/R confluence meter — clustered levels
    FlowCompass,      // institutional flow compass — directional bias
    VolRegime,        // volatility regime panel — squeeze/expansion
    MomentumHeat,     // momentum heatmap — ROC across lookbacks
    BreadthThermo,    // breadth thermometer — dot matrix market health
    SectorRotation,   // sector rotation radar — 2x2 quadrant
    OptionsSentiment, // options sentiment composite gauge
    RelStrength,      // relative strength radar — vs peers/market
    RiskDash,         // risk dashboard — position sizing helper
    EarningsMom,      // earnings momentum — EPS/revenue trends
    LiquidityScore,   // liquidity score gauge
    SignalRadar,      // radial map of all active ApexSignals
    CrossAssetPulse,  // compact multi-asset dashboard grid
    TapeSpeed,        // trade velocity speedometer
    Fundamentals,     // key fundamental metrics card
    EconCalendar,     // upcoming economic events countdown
    Latency,          // frame time + data feed latency
    PayoffChart,      // options payoff curve diagram
    OptionsFlow,      // unusual options activity feed
    PositionsPanel,   // all account positions with P&L + close buttons
    DailyPnl,         // hero daily P&L number + close all button
    #[serde(other)]
    Custom,           // user-defined (future: extension widget / fallback for unknown)
}

impl ChartWidgetKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::TrendStrength  => "Trend Strength",
            Self::Momentum       => "Momentum",
            Self::Volatility     => "Volatility",
            Self::VolumeProfile  => "Volume Profile",
            Self::SessionTimer   => "Session Timer",
            Self::KeyLevels      => "Key Levels",
            Self::OptionGreeks   => "Option Greeks",
            Self::RiskReward     => "Risk / Reward",
            Self::MarketBreadth  => "Market Breadth",
            Self::Correlation    => "Correlation",
            Self::DarkPool       => "Dark Pool",
            Self::PositionPnl    => "Position P&L",
            Self::EarningsBadge  => "Earnings",
            Self::NewsTicker     => "News Ticker",
            Self::ExitGauge      => "Exit Gauge",
            Self::PrecursorAlert => "Precursor",
            Self::TradePlan      => "Trade Plan",
            Self::ChangePoints   => "Change Points",
            Self::ZoneStrength   => "Zone Strength",
            Self::PatternScanner => "Patterns",
            Self::VixMonitor     => "VIX Monitor",
            Self::SignalDashboard=> "Signal Dashboard",
            Self::DivergenceMonitor => "Divergences",
            Self::ConvictionMeter=> "Conviction",
            Self::RsiMulti       => "RSI Multi",
            Self::TrendAlign     => "Trend Align",
            Self::VolumeShelf    => "Vol Shelves",
            Self::Confluence     => "Confluence",
            Self::FlowCompass    => "Flow",
            Self::VolRegime      => "Vol Regime",
            Self::MomentumHeat   => "Momentum",
            Self::BreadthThermo  => "Breadth",
            Self::SectorRotation => "Sectors",
            Self::OptionsSentiment => "Options",
            Self::RelStrength    => "Rel Strength",
            Self::RiskDash       => "Risk",
            Self::EarningsMom    => "Earnings",
            Self::LiquidityScore => "Liquidity",
            Self::SignalRadar    => "Signal Radar",
            Self::CrossAssetPulse => "Market Pulse",
            Self::TapeSpeed      => "Tape Speed",
            Self::Fundamentals   => "Fundamentals",
            Self::EconCalendar   => "Calendar",
            Self::Latency        => "Latency",
            Self::PayoffChart    => "Payoff",
            Self::OptionsFlow    => "Flow",
            Self::PositionsPanel => "Positions",
            Self::DailyPnl       => "Daily P&L",
            Self::Custom         => "Custom",
        }
    }
    pub(crate) fn icon(self) -> &'static str {
        match self {
            Self::TrendStrength  => "\u{2191}",  // ↑
            Self::Momentum       => "\u{21C5}",  // ⇅
            Self::Volatility     => "\u{2248}",  // ≈
            Self::VolumeProfile  => "\u{2593}",  // ▓
            Self::SessionTimer   => "\u{23F1}",  // ⏱
            Self::KeyLevels      => "\u{2550}",  // ═
            Self::OptionGreeks   => "\u{0394}",  // Δ
            Self::RiskReward     => "\u{2696}",  // ⚖
            Self::MarketBreadth  => "\u{2637}",  // ☷
            Self::Correlation    => "\u{2194}",  // ↔
            Self::DarkPool       => "\u{2588}",  // █
            Self::PositionPnl    => "\u{0024}",  // $
            Self::EarningsBadge  => "\u{1F4C5}", // 📅
            Self::NewsTicker     => "\u{1F4F0}", // 📰
            Self::ExitGauge      => "\u{23F9}",  // ⏹
            Self::PrecursorAlert => "\u{26A1}",  // ⚡
            Self::TradePlan      => "\u{1F3AF}", // 🎯
            Self::ChangePoints   => "\u{21BA}",  // ↺
            Self::ZoneStrength   => "\u{2591}",  // ░
            Self::PatternScanner => "\u{25E2}",  // ◢
            Self::VixMonitor     => "\u{26A0}",  // ⚠
            Self::SignalDashboard=> "\u{2630}",  // ☰
            Self::DivergenceMonitor => "\u{21C5}",// ⇅
            Self::ConvictionMeter=> "\u{2605}",  // ★
            Self::RsiMulti       => "\u{25CE}",  // ◎
            Self::TrendAlign     => "\u{2593}",  // ▓
            Self::VolumeShelf    => "\u{2586}",  // ▆
            Self::Confluence     => "\u{2261}",  // ≡
            Self::FlowCompass    => "\u{29BF}",  // ⦿
            Self::VolRegime      => "\u{224B}",  // ≋
            Self::MomentumHeat   => "\u{2588}",  // █
            Self::BreadthThermo  => "\u{25A3}",  // ▣
            Self::SectorRotation => "\u{25D4}",  // ◔
            Self::OptionsSentiment => "\u{25D5}", // ◕
            Self::RelStrength    => "\u{29BE}",  // ⦾
            Self::RiskDash       => "\u{26A0}",  // ⚠
            Self::EarningsMom    => "\u{2197}",  // ↗
            Self::LiquidityScore => "\u{25C9}",  // ◉
            Self::SignalRadar    => "\u{25C8}",  // ◈
            Self::CrossAssetPulse => "\u{229E}", // ⊞
            Self::TapeSpeed      => "\u{23F1}",  // ⏱
            Self::Fundamentals   => "\u{1F4CA}", // 📊
            Self::EconCalendar   => "\u{1F4C5}", // 📅
            Self::Latency        => "\u{23F1}",  // ⏱
            Self::PayoffChart    => "\u{1F4C8}", // 📈
            Self::OptionsFlow    => "\u{1F4A7}", // 💧
            Self::PositionsPanel => "\u{1F4BC}", // 💼
            Self::DailyPnl       => "\u{1F4B0}", // 💰
            Self::Custom         => "\u{2699}",  // ⚙
        }
    }
    pub(crate) fn all() -> &'static [Self] {
        &[Self::TrendStrength, Self::Momentum, Self::Volatility, Self::VolumeProfile,
          Self::SessionTimer, Self::KeyLevels, Self::OptionGreeks, Self::RiskReward,
          Self::MarketBreadth, Self::Correlation, Self::DarkPool, Self::PositionPnl,
          Self::EarningsBadge, Self::NewsTicker, Self::ExitGauge, Self::PrecursorAlert,
          Self::TradePlan, Self::ChangePoints, Self::ZoneStrength, Self::PatternScanner,
          Self::VixMonitor, Self::SignalDashboard, Self::DivergenceMonitor, Self::ConvictionMeter,
          Self::RsiMulti, Self::TrendAlign, Self::VolumeShelf, Self::Confluence,
          Self::FlowCompass, Self::VolRegime, Self::MomentumHeat, Self::BreadthThermo,
          Self::SectorRotation, Self::OptionsSentiment, Self::RelStrength,
          Self::RiskDash, Self::EarningsMom, Self::LiquidityScore,
          Self::SignalRadar, Self::CrossAssetPulse, Self::TapeSpeed,
          Self::Fundamentals, Self::EconCalendar, Self::Latency,
          Self::PayoffChart, Self::OptionsFlow,
          Self::PositionsPanel, Self::DailyPnl]
    }

    /// Subtle background tint color by widget category (for dashboard tiles).
    /// Returns (r, g, b, alpha) — the alpha is very low for a subtle wash.
    pub(crate) fn category_tint(self) -> (u8, u8, u8, u8) {
        match self {
            // Gauges — warm cream
            Self::TrendStrength | Self::Momentum | Self::Volatility | Self::ConvictionMeter
            | Self::LiquidityScore | Self::RsiMulti => (200, 180, 140, 15),
            // Analytics — sage green
            Self::TrendAlign | Self::VolumeShelf | Self::Confluence | Self::MomentumHeat
            | Self::VolRegime | Self::BreadthThermo | Self::RelStrength => (140, 180, 140, 12),
            // Market — peach/salmon
            Self::Correlation | Self::DarkPool | Self::FlowCompass | Self::SectorRotation
            | Self::OptionsSentiment | Self::SignalRadar | Self::CrossAssetPulse | Self::TapeSpeed
            => (200, 160, 140, 12),
            // Position/P&L — no tint (clean white)
            Self::PositionPnl | Self::PositionsPanel | Self::DailyPnl | Self::RiskDash
            | Self::RiskReward => (0, 0, 0, 0),
            // Signals — light blue
            Self::ExitGauge | Self::PrecursorAlert | Self::TradePlan | Self::ChangePoints
            | Self::ZoneStrength | Self::PatternScanner | Self::VixMonitor | Self::SignalDashboard
            | Self::DivergenceMonitor => (140, 160, 200, 12),
            // Info — light grey
            _ => (160, 160, 160, 8),
        }
    }
}

/// Widget display density — how much chrome surrounds the data.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum WidgetDisplayMode {
    Card,    // solid background card
    Hud,     // transparent — just the visualization
    #[serde(other)]
    Minimal, // legacy fallback → treated as HUD
}

impl WidgetDisplayMode {
    pub fn label(self) -> &'static str {
        match self { Self::Card => "Card", Self::Hud | Self::Minimal => "HUD" }
    }
    pub fn cycle(self) -> Self {
        match self { Self::Card => Self::Hud, Self::Hud | Self::Minimal => Self::Card }
    }
}

/// Where a widget docks on the chart.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum WidgetDock {
    Float,  // free-floating, positioned by x/y fractions
    Top,    // snapped to top strip, auto-laid out left-to-right
    Bottom, // snapped to bottom strip, auto-laid out left-to-right
}

impl WidgetDock {
    pub fn label(self) -> &'static str {
        match self { Self::Float => "Float", Self::Top => "Top", Self::Bottom => "Bottom" }
    }
}

fn default_opacity() -> f32 { 1.0 }

/// A floating info widget placed on the chart canvas.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ChartWidget {
    pub kind: ChartWidgetKind,
    pub x: f32,          // position as fraction of chart width (0.0-1.0)
    pub y: f32,          // position as fraction of chart height (0.0-1.0)
    pub w: f32,          // width in pixels
    pub h: f32,          // height in pixels
    pub visible: bool,
    pub collapsed: bool,        // just show title bar
    pub locked: bool,           // prevent accidental dragging
    pub display: WidgetDisplayMode, // Card / HUD / Minimal
    pub dock: WidgetDock,           // Float / Top / Bottom
    pub dock_x: f32,               // horizontal pos in dock strip (pixels from chart left)
    #[serde(default = "default_opacity")]
    pub opacity: f32,              // 0.0-1.0 — fades widget toward background
    // ── Animation state (transient, not meaningful to persist but serde-friendly) ──
    pub anim_x: f32,  // current animated screen x
    pub anim_y: f32,  // current animated screen y
    pub anim_init: bool, // false until first frame positions are seeded
}

impl ChartWidget {
    pub fn new(kind: ChartWidgetKind, x: f32, y: f32) -> Self {
        // Standard grid sizes: S=160², M=200², L=240², W=240×160
        const S: (f32, f32) = (160.0, 160.0);
        const M: (f32, f32) = (200.0, 200.0);
        const W: (f32, f32) = (240.0, 160.0); // wide
        const T: (f32, f32) = (160.0, 120.0); // compact
        let (w, h) = match kind {
            // Gauges — square, donut ring centered
            ChartWidgetKind::TrendStrength  => S,
            ChartWidgetKind::Momentum       => S,
            ChartWidgetKind::Volatility     => S,
            ChartWidgetKind::ConvictionMeter=> S,
            ChartWidgetKind::LiquidityScore => S,
            ChartWidgetKind::ExitGauge      => S,
            ChartWidgetKind::OptionsSentiment => S,
            ChartWidgetKind::SessionTimer   => S,
            ChartWidgetKind::TapeSpeed      => S,
            ChartWidgetKind::Latency        => T,
            // Concentric rings — medium square
            ChartWidgetKind::RsiMulti       => M,
            ChartWidgetKind::VolRegime      => M,
            ChartWidgetKind::RelStrength    => M,
            ChartWidgetKind::FlowCompass    => M,
            ChartWidgetKind::SignalRadar    => M,
            // Grids/matrices — medium square
            ChartWidgetKind::TrendAlign     => M,
            ChartWidgetKind::BreadthThermo  => M,
            ChartWidgetKind::SectorRotation => M,
            ChartWidgetKind::EarningsMom    => M,
            ChartWidgetKind::CrossAssetPulse => M,
            // Data lists — wide
            ChartWidgetKind::KeyLevels      => W,
            ChartWidgetKind::VolumeShelf    => W,
            ChartWidgetKind::Confluence     => W,
            ChartWidgetKind::Fundamentals   => W,
            ChartWidgetKind::OptionsFlow    => W,
            ChartWidgetKind::EconCalendar   => W,
            // Charts/bars — wide
            ChartWidgetKind::MomentumHeat   => W,
            ChartWidgetKind::VolumeProfile  => (160.0, 200.0),
            ChartWidgetKind::PayoffChart    => W,
            ChartWidgetKind::DarkPool       => W,
            // Compact info cards
            ChartWidgetKind::Correlation    => S,
            ChartWidgetKind::PositionPnl    => S,
            ChartWidgetKind::EarningsBadge  => T,
            ChartWidgetKind::DailyPnl       => W,
            ChartWidgetKind::RiskReward     => S,
            ChartWidgetKind::RiskDash       => W,
            ChartWidgetKind::MarketBreadth  => S,
            ChartWidgetKind::VixMonitor     => S,
            // Signal cards — standard
            ChartWidgetKind::PrecursorAlert => S,
            ChartWidgetKind::TradePlan      => W,
            ChartWidgetKind::ChangePoints   => S,
            ChartWidgetKind::ZoneStrength   => S,
            ChartWidgetKind::PatternScanner => S,
            ChartWidgetKind::SignalDashboard=> M,
            ChartWidgetKind::DivergenceMonitor => S,
            // Special
            ChartWidgetKind::NewsTicker     => (280.0, 60.0),
            ChartWidgetKind::PositionsPanel => (240.0, 200.0),
            ChartWidgetKind::OptionGreeks   => S,
            ChartWidgetKind::SignalRadar    => (210.0, 210.0),
            ChartWidgetKind::CrossAssetPulse => (200.0, 160.0),
            ChartWidgetKind::TapeSpeed      => (170.0, 140.0),
            ChartWidgetKind::Fundamentals   => (200.0, 180.0),
            ChartWidgetKind::EconCalendar   => (210.0, 160.0),
            ChartWidgetKind::Latency        => (160.0, 100.0),
            ChartWidgetKind::PayoffChart    => (220.0, 160.0),
            ChartWidgetKind::OptionsFlow    => (200.0, 180.0),
            ChartWidgetKind::PositionsPanel => (220.0, 200.0),
            ChartWidgetKind::DailyPnl       => (260.0, 80.0),
            ChartWidgetKind::Custom         => (150.0, 90.0),
        };
        Self { kind, x, y, w, h, visible: true, collapsed: false, locked: false,
               display: WidgetDisplayMode::Card, dock: WidgetDock::Float,
               dock_x: 0.0, anim_x: 0.0, anim_y: 0.0, anim_init: false, opacity: 1.0 }
    }
}

/// A saved widget layout — restores all widgets for a chart pane.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WidgetPreset {
    pub name: String,
    pub widgets: Vec<ChartWidget>,
}

impl WidgetPreset {
    pub fn from_chart(name: &str, widgets: &[ChartWidget]) -> Self {
        Self { name: name.into(), widgets: widgets.to_vec() }
    }
}

// ─── Plays / Playbook (shareable strategy cards) ─────────────────────────────

/// Play type — determines which chart elements are spawned and which form fields appear.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PlayType {
    Directional,  // simple long/short: entry + target + stop
    Bracket,      // entry + TP + SL as unified bracket
    Scalp,        // entry + tight target, no stop, time-limited
    Swing,        // entry + T1/T2/T3 partial exits
    Spread,       // two legs with net debit/credit
    Event,        // catalyst-tied with pre/post levels
}

impl PlayType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Directional => "Directional",
            Self::Bracket     => "Bracket",
            Self::Scalp       => "Scalp",
            Self::Swing       => "Swing",
            Self::Spread      => "Spread",
            Self::Event       => "Event",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Directional => "\u{2191}",  // ↑
            Self::Bracket     => "\u{2696}",  // ⚖
            Self::Scalp       => "\u{26A1}",  // ⚡
            Self::Swing       => "\u{301C}",  // 〜
            Self::Spread      => "\u{2194}",  // ↔
            Self::Event       => "\u{1F4C5}", // 📅
        }
    }
    pub fn all() -> &'static [Self] {
        &[Self::Directional, Self::Bracket, Self::Scalp, Self::Swing, Self::Spread, Self::Event]
    }
}

/// A target level in a multi-target play (Swing).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PlayTarget {
    pub price: f32,
    pub pct: f32,       // allocation % (0.0-1.0), all targets must sum to 1.0
    pub label: String,  // "T1", "T2", "T3"
}

/// A leg in a spread play.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SpreadLeg {
    pub contract: String,       // "450C 0DTE"
    pub side: PlayDirection,    // buy or sell this leg
    pub price: f32,
    pub quantity: u32,
}

/// Play line kind — which price level this line represents on the chart.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PlayLineKind {
    Entry,
    Target,   // T1 / primary target
    Stop,
    Target2,
    Target3,
}

impl PlayLineKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Entry   => "ENTRY",
            Self::Target  => "TARGET",
            Self::Stop    => "STOP",
            Self::Target2 => "T2",
            Self::Target3 => "T3",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Self::Entry   => "E",
            Self::Target  => "T",
            Self::Stop    => "S",
            Self::Target2 => "T2",
            Self::Target3 => "T3",
        }
    }
}

/// A draggable price line on the chart, synced with the play editor form.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PlayLine {
    pub id: u32,
    pub kind: PlayLineKind,
    pub price: f32,
}

/// A saved play template (preset).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PlayTemplate {
    pub name: String,
    pub play_type: PlayType,
    pub direction: PlayDirection,
    pub rr_ratio: f32,          // auto-compute stop from entry+target
    pub default_tags: Vec<String>,
}

/// A Play is a shareable one-click trade card: ticker + entry + exit + stop.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Play {
    pub id: String,
    pub title: String,         // "AAPL Bull Call Spread" or "SPY Breakout Long"
    pub symbol: String,
    pub direction: PlayDirection,
    pub play_type: PlayType,
    pub entry_price: f32,
    pub target_price: f32,
    pub stop_price: f32,
    pub targets: Vec<PlayTarget>,     // T2/T3 for Swing plays
    pub spread_legs: Vec<SpreadLeg>,  // legs for Spread plays
    pub contract: String,      // empty for equity, "450C 0DTE" for options
    pub quantity: u32,
    pub status: PlayStatus,
    pub author: String,        // who created it
    pub notes: String,         // strategy rationale
    pub created_at: i64,       // unix timestamp
    pub risk_reward: f32,      // computed: (target - entry) / (entry - stop)
    pub tags: Vec<String>,     // "momentum", "breakout", "earnings", etc.
    pub template_name: Option<String>,
}

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PlayDirection { Long, Short }

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PlayStatus {
    Draft,       // being composed
    Published,   // shared / visible in feed
    Active,      // order placed, in progress
    Won,         // hit target
    Lost,        // hit stop
    Expired,     // time-based expiry
    Cancelled,   // manually cancelled
}

impl PlayDirection {
    pub fn label(self) -> &'static str { match self { Self::Long => "LONG", Self::Short => "SHORT" } }
}

impl PlayStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT", Self::Published => "PUBLISHED", Self::Active => "ACTIVE",
            Self::Won => "WON", Self::Lost => "LOST", Self::Expired => "EXPIRED", Self::Cancelled => "CANCELLED",
        }
    }
}

impl Play {
    pub fn new(symbol: &str, direction: PlayDirection, play_type: PlayType, entry: f32, target: f32, stop: f32) -> Self {
        let rr = if (entry - stop).abs() > 0.001 {
            (target - entry).abs() / (entry - stop).abs()
        } else { 0.0 };
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: format!("{} {}", symbol, direction.label()),
            symbol: symbol.into(),
            direction, play_type, entry_price: entry, target_price: target, stop_price: stop,
            targets: vec![], spread_legs: vec![],
            contract: String::new(), quantity: 1,
            status: PlayStatus::Draft,
            author: String::new(), notes: String::new(),
            created_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
            risk_reward: rr,
            tags: vec![],
            template_name: None,
        }
    }
}

/// Tab selector for the Book pane (Positions/Orders + Journal).
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum BookTab { Book, Journal }

/// Pane header density mode — controls header height + title font size.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PaneHeaderSize { Compact, Normal, Expanded }

impl PaneHeaderSize {
    pub(crate) fn header_h(self) -> f32 {
        match self { Self::Compact => 22.0, Self::Normal => 26.0, Self::Expanded => 32.0 }
    }
    pub(crate) fn tabs_header_h(self) -> f32 {
        // Tabs header — only slightly bigger than non-tabs so it doesn't dominate.
        match self { Self::Compact => 28.0, Self::Normal => 32.0, Self::Expanded => 38.0 }
    }
    pub(crate) fn title_font(self) -> f32 {
        match self { Self::Compact => 11.0, Self::Normal => 12.0, Self::Expanded => 14.0 }
    }
    pub(crate) fn label(self) -> &'static str {
        match self { Self::Compact => "Compact", Self::Normal => "Normal", Self::Expanded => "Expanded" }
    }
}

/// Line style
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineStyle { Solid, Dashed, Dotted }

/// Drawing on the chart
/// Significance data for a drawing (populated by backend/ApexSignals)
#[derive(Debug, Clone)]
pub struct DrawingSignificance {
    pub score: f32,            // 0.0-10.0 overall score
    pub touches: u32,          // number of price touches
    pub timeframe: String,     // source timeframe e.g. "1D", "4H"
    pub age_days: u32,         // how old the drawing is
    pub volume_index: f32,     // avg volume at touches relative to normal (1.0 = average)
    pub last_tested_bars: u32, // how many bars ago price last touched it
    pub strength: String,      // "WEAK", "MODERATE", "STRONG", "CRITICAL"
}

impl DrawingSignificance {
    /// Generate placeholder significance from basic bar analysis
    pub fn estimate(kind: &DrawingKind, timestamps: &[i64], bars: &[Bar]) -> Option<Self> {
        if bars.is_empty() || timestamps.is_empty() { return None; }
        let n = bars.len();
        let threshold_pct = 0.003; // 0.3% proximity = touch

        // Get the price function for this drawing at each bar
        let price_at_bar = |i: usize| -> Option<f32> {
            match kind {
                DrawingKind::HLine { price } => Some(*price),
                DrawingKind::TrendLine { price0, time0, price1, time1 } => {
                    let t0 = *time0 as f64; let t1 = *time1 as f64;
                    if (t1 - t0).abs() < 1.0 { return None; }
                    let tc = timestamps.get(i).copied()? as f64;
                    let frac = (tc - t0) / (t1 - t0);
                    Some(*price0 + (*price1 - *price0) * frac as f32)
                }
                _ => None,
            }
        };

        let mut touches = 0u32;
        let mut last_touch_bar = 0usize;
        let mut vol_sum = 0.0f32;
        let mut vol_count = 0u32;
        let avg_vol: f32 = bars.iter().map(|b| b.volume).sum::<f32>() / n as f32;

        for i in 0..n {
            if let Some(level_price) = price_at_bar(i) {
                if level_price <= 0.0 { continue; }
                let threshold = level_price * threshold_pct;
                let bar = &bars[i];
                // Touch = bar high/low within threshold of the level
                if (bar.high - level_price).abs() < threshold || (bar.low - level_price).abs() < threshold {
                    touches += 1;
                    last_touch_bar = i;
                    vol_sum += bar.volume;
                    vol_count += 1;
                }
            }
        }

        if touches == 0 { return None; }

        let volume_index = if vol_count > 0 && avg_vol > 0.0 { (vol_sum / vol_count as f32) / avg_vol } else { 1.0 };
        let last_tested_bars = (n - 1).saturating_sub(last_touch_bar) as u32;

        // Age in days (approximate from timestamps)
        let age_days = if timestamps.len() >= 2 {
            let first = timestamps[0]; let last = *timestamps.last().unwrap();
            ((last - first) as f64 / 86400.0).ceil() as u32
        } else { 0 };

        // Score: weighted combination
        let touch_score = (touches as f32 * 1.5).min(5.0);
        let vol_score = (volume_index * 1.5).min(2.5);
        let recency_score = if last_tested_bars < 5 { 2.0 } else if last_tested_bars < 20 { 1.0 } else { 0.5 };
        let score = (touch_score + vol_score + recency_score).min(10.0);

        let strength = if score >= 7.0 { "CRITICAL" } else if score >= 5.0 { "STRONG" } else if score >= 3.0 { "MODERATE" } else { "WEAK" };

        Some(Self {
            score,
            touches,
            timeframe: String::new(), // filled by backend
            age_days,
            volume_index,
            last_tested_bars,
            strength: strength.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Drawing {
    pub id: String,
    pub kind: DrawingKind,
    pub color: String,      // hex color like "#4a9eff"
    pub opacity: f32,       // 0.0-1.0
    pub line_style: LineStyle,
    pub thickness: f32,     // pixels
    pub group_id: String,   // "default" or group UUID
    pub extend_left: bool,  // extend trendline/ray to left chart edge
    pub extend_right: bool, // extend trendline/ray to right chart edge
    pub locked: bool,       // prevent accidental moves
    pub alert_enabled: bool, // show alert bell indicator
    pub significance: Option<DrawingSignificance>, // backend-populated or estimated
}

impl Drawing {
    pub fn new(id: String, kind: DrawingKind) -> Self {
        Self { id, kind, color: "#4a9eff".into(), opacity: 1.0, line_style: LineStyle::Solid, thickness: 1.5, group_id: "default".into(), extend_left: false, extend_right: false, locked: false, alert_enabled: false, significance: None }
    }
}

/// Drawing group
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DrawingGroup {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DrawingKind {
    HLine { price: f32 },
    /// Trendline — timestamps (i64) resolve to fractional bar positions on any timeframe.
    TrendLine { price0: f32, time0: i64, price1: f32, time1: i64 },
    HZone { price0: f32, price1: f32 },
    BarMarker { time: i64, price: f32, up: bool },
    /// Fibonacci retracement — two anchor points define the range.
    /// Levels drawn: 0%, 23.6%, 38.2%, 50%, 61.8%, 78.6%, 100%
    Fibonacci { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Parallel channel — base trendline (p0→p1) + price offset for the parallel line.
    Channel { price0: f32, time0: i64, price1: f32, time1: i64, offset: f32 },
    /// Fibonacci channel — same anchors as channel, internal lines at fib ratios.
    FibChannel { price0: f32, time0: i64, price1: f32, time1: i64, offset: f32 },
    /// Pitchfork — pivot + two reaction points. variant: 0=Standard, 1=Schiff, 2=Modified Schiff
    Pitchfork { price0: f32, time0: i64, price1: f32, time1: i64, price2: f32, time2: i64 },
    /// Gann Fan — origin + scale point defines 1x1 angle, radiating lines
    GannFan { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Regression Channel — time range, regression + σ bands computed from bars
    RegressionChannel { time0: i64, time1: i64 },
    /// XABCD Harmonic pattern — 5 points stored as (time, price) pairs
    XABCD { points: Vec<(i64, f32)> },
    /// Elliott Wave — labeled wave points, wave_type: 0=impulse(5pt), 1=corrective(3pt)
    ElliottWave { points: Vec<(i64, f32)>, wave_type: u8 },
    /// Anchored VWAP — single anchor timestamp, line computed from bars
    AnchoredVWAP { time: i64 },
    /// Price Range — persistent measurement rectangle
    PriceRange { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Risk/Reward — entry + stop + target
    RiskReward { entry_price: f32, entry_time: i64, stop_price: f32, target_price: f32 },
    /// Vertical line at a specific timestamp
    VerticalLine { time: i64 },
    /// Ray — trendline that extends forward from second point to chart edge
    Ray { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Fib Extension (3-point) — A→B move projected from C
    FibExtension { price0: f32, time0: i64, price1: f32, time1: i64, price2: f32, time2: i64 },
    /// Fib Time Zones — vertical lines at fibonacci bar intervals from anchor
    FibTimeZone { time: i64 },
    /// Fib Arcs — semicircular arcs at fib ratios between two points
    FibArc { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Gann Box — price/time grid with diagonal angles
    GannBox { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Text annotation — placed at a price/time coordinate
    TextNote { price: f32, time: i64, text: String, font_size: f32 },
}

/// Candlestick pattern label from ApexSignals
#[derive(Debug, Clone)]
pub struct PatternLabel {
    pub time: i64,
    pub label: String,
    pub bullish: bool,
    pub confidence: f32, // 0.0–1.0
}

/// Commands sent from Tauri/WebView to the native chart renderer
#[derive(Debug, Clone)]
pub enum ChartCommand {
    /// Load OHLCV bar data for a symbol
    LoadBars {
        symbol: String,
        timeframe: String,
        bars: Vec<Bar>,
        timestamps: Vec<i64>,
    },
    /// Prepend historical bars (pagination — older data loaded on scroll-left)
    PrependBars {
        symbol: String,
        timeframe: String,
        bars: Vec<Bar>,
        timestamps: Vec<i64>,
    },
    /// Append a single new bar + timestamp.
    /// `mark`: MARK_BARS_PROTOCOL source — `false` = last (trade prints, default),
    /// `true` = mark (NBBO mid). Panes drop frames whose source doesn't match their
    /// current `bar_source_mark` (race window during toggle).
    AppendBar {
        symbol: String,
        timeframe: String,
        bar: Bar,
        timestamp: i64,
        mark: bool,
    },
    /// Update the last bar (tick). See `AppendBar` for `mark` semantics.
    UpdateLastBar {
        symbol: String,
        timeframe: String,
        bar: Bar,
        mark: bool,
    },
    /// Set viewport (from pan/zoom in WebView)
    SetViewport {
        view_start: u32,
        view_count: u32,
        width: u32,
        height: u32,
    },
    /// Set theme colors
    SetTheme {
        background: [f32; 4],
        bull_color: [f32; 4],
        bear_color: [f32; 4],
    },
    /// Add/update a drawing
    SetDrawing(Drawing),
    /// Remove a drawing
    RemoveDrawing { id: String },
    /// Clear all drawings
    ClearDrawings,
    /// Bulk-load drawings from DB (async delivery)
    LoadDrawings {
        symbol: String,
        drawings: Vec<Drawing>,
        groups: Vec<DrawingGroup>,
    },
    /// Resize the window
    Resize { width: u32, height: u32 },
    /// Close the renderer
    Shutdown,
    /// Show/reactivate the window (sent when GPU button clicked again)
    Show,
    /// Watchlist price update
    WatchlistPrice {
        symbol: String,
        price: f32,
        prev_close: f32,
    },
    /// Time & Sales tape entry
    TapeEntry {
        symbol: String,
        price: f32,
        qty: f32,
        time: i64,
        is_buy: bool,
    },
    /// Scanner bulk price update (symbol, price, prev_close, volume)
    ScannerPrice {
        symbol: String,
        price: f32,
        prev_close: f32,
        volume: u64,
    },
    /// Signal drawings from analysis server
    SignalDrawings {
        symbol: String,
        drawings_json: String, // raw JSON — parsed in gpu.rs
    },
    /// Deliver source bars for a cross-timeframe indicator
    IndicatorSourceBars {
        indicator_id: u32,
        timeframe: String,
        bars: Vec<Bar>,
        timestamps: Vec<i64>,
    },
    /// Options chain data fetched from ApexIB
    ChainData {
        symbol: String,
        dte: i32,
        underlying_price: f32, // real-time price from IB
        calls: Vec<(f32, f32, f32, f32, i32, i32, f32, bool, String)>, // strike, last, bid, ask, vol, oi, iv, itm, contract
        puts: Vec<(f32, f32, f32, f32, i32, i32, f32, bool, String)>,
    },
    /// Event markers (earnings, dividends, splits, economic) for overlay display
    EventData {
        symbol: String,
        events: Vec<(i64, String, String, String, i8)>, // (timestamp, event_type, label, details, impact)
    },
    /// Options chain data for the chart overlay (independent of sidebar chain tab)
    OverlayChainData {
        symbol: String,
        calls: Vec<(f32, f32, f32, f32, i32, i32, f32, bool, String)>,
        puts: Vec<(f32, f32, f32, f32, i32, i32, f32, bool, String)>,
    },
    /// Symbol search results from ApexIB
    SearchResults {
        query: String,
        results: Vec<(String, String)>, // (symbol, name)
        source: String, // "watchlist" or "chain"
    },
    /// Overlay bars for a secondary symbol overlay
    OverlayBars {
        symbol: String,
        bars: Vec<Bar>,
        timestamps: Vec<i64>,
    },
    /// Dark pool / off-exchange print data
    DarkPoolData {
        symbol: String,
        prints: Vec<(f32, u64, i64, i8)>, // (price, size, timestamp, side: 1=buy/-1=sell/0=unknown)
    },
    /// Candlestick pattern labels from ApexSignals (via signals feed)
    PatternLabels {
        symbol: String,
        labels: Vec<PatternLabel>,
    },
    /// Alert triggered notification from ApexSignals
    AlertTriggered {
        symbol: String,
        alert_id: String,
        price: f32,
        message: String,
    },
    /// Auto trendlines pushed from ApexSignals (replaces signal_drawings for this symbol)
    AutoTrendlines {
        symbol: String,
        drawings_json: String, // same JSON format as SignalDrawings
    },
    /// Significance score update for a drawing from ApexSignals
    SignificanceUpdate {
        symbol: String,
        drawing_id: String,
        score: f32,
        touches: u32,
        strength: String, // "WEAK", "MODERATE", "STRONG", "CRITICAL"
    },
    /// Trend health score from ApexSignals (0-100 composite)
    TrendHealthUpdate {
        symbol: String,
        timeframe: String,
        score: f32,            // 0-100
        direction: i8,         // 1=bullish, -1=bearish, 0=neutral
        exhaustion_count: u8,  // number of active exhaustion signals
        regime: String,        // "strong_trend", "weakening", "exhausted", "reversal"
    },
    /// Exit gauge score from ApexSignals (0-100 master exit signal)
    ExitGaugeUpdate {
        symbol: String,
        score: f32,            // 0-100
        urgency: String,       // "hold", "tighten", "partial", "close", "exit_now"
        components: Vec<(String, f32)>, // (engine_name, contribution)
    },
    /// Supply/demand zones from ApexSignals
    SupplyDemandZones {
        symbol: String,
        timeframe: String,
        zones: Vec<SignalZone>,
    },
    /// Precursor alert from ApexSignals (smart money front-running detected)
    PrecursorAlert {
        symbol: String,
        score: f32,            // 0-100
        direction: i8,         // 1=bullish, -1=bearish
        surge_ratio: f32,      // volume / baseline
        lead_minutes: f32,     // estimated time to move
        description: String,
    },
    /// Change-point detection — exact moment of regime shift
    ChangePointMarker {
        symbol: String,
        time: i64,             // bar timestamp of change
        change_type: String,   // "volume", "directional", "volatility", "institutional"
        confidence: f32,       // 0-1
    },
    /// Trade plan suggestion from ApexSignals
    TradePlanUpdate {
        symbol: String,
        direction: i8,         // 1=long, -1=short
        entry_price: f32,
        target_price: f32,
        stop_price: f32,
        contract_name: String, // e.g. "AAPL 195C 5DTE"
        contract_entry: f32,
        contract_target: f32,
        risk_reward: f32,
        conviction: f32,       // 0-100
        summary: String,
    },
    /// Divergence visual overlay from ApexSignals
    DivergenceOverlay {
        symbol: String,
        timeframe: String,
        divergences: Vec<DivergenceMarker>,
    },
}

/// A supply/demand zone from the signal engine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignalZone {
    pub zone_type: String,     // "supply", "demand", "order_block", "fvg", "breaker"
    pub price_high: f32,
    pub price_low: f32,
    pub start_time: i64,
    pub strength: f32,         // 0-10
    pub touches: u32,
    pub fresh: bool,           // untested zone
}

/// A divergence marker for chart overlay.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DivergenceMarker {
    pub indicator: String,     // "RSI", "MACD", etc.
    pub div_type: String,      // "regular_bullish", "hidden_bearish", etc.
    pub start_bar: u32,        // bar index of first point
    pub end_bar: u32,          // bar index of second point
    pub start_price: f32,
    pub end_price: f32,
    pub confidence: f32,       // 0-1
}

