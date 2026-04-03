//! Native GPU chart renderer — winit (any_thread) + egui for all rendering.
//! egui handles UI + chart painting. winit handles window on non-main thread.

use std::sync::{mpsc, Arc, Mutex};
use std::fmt::Write as FmtWrite;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
    dpi::PhysicalSize,
};

use super::{Bar, ChartCommand, Drawing, DrawingKind, DrawingGroup, LineStyle};

// Thread-local to pass window ref into draw_chart (which doesn't have access to ChartWindow)
std::thread_local! {
    static CURRENT_WINDOW: std::cell::RefCell<Option<Arc<Window>>> = const { std::cell::RefCell::new(None) };
    static CLOSE_REQUESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PENDING_ALERT: std::cell::RefCell<Option<(String, f32, bool)>> = const { std::cell::RefCell::new(None) };
    static PENDING_TOASTS: std::cell::RefCell<Vec<(String, f32, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
    static CONN_PANEL_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
use crate::ui_kit::{self, icons::Icon};

// ─── Themes ───────────────────────────────────────────────────────────────────

struct Theme {
    name: &'static str,
    bg: egui::Color32, bull: egui::Color32, bear: egui::Color32, dim: egui::Color32,
    toolbar_bg: egui::Color32, toolbar_border: egui::Color32, accent: egui::Color32,
}
const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }
const THEMES: &[Theme] = &[
    Theme { name: "Midnight",    bg: rgb(14,16,21),   bull: rgb(62,120,180),  bear: rgb(180,65,58),   dim: rgb(100,105,115), toolbar_bg: rgb(10,12,17),  toolbar_border: rgb(28,32,40),  accent: rgb(62,120,180) },
    Theme { name: "Nord",        bg: rgb(38,44,56),   bull: rgb(163,190,140), bear: rgb(191,97,106),  dim: rgb(129,161,193), toolbar_bg: rgb(32,38,50),  toolbar_border: rgb(50,56,70),  accent: rgb(136,192,208) },
    Theme { name: "Monokai",     bg: rgb(39,40,34),   bull: rgb(166,226,46),  bear: rgb(249,38,114),  dim: rgb(165,159,133), toolbar_bg: rgb(33,34,28),  toolbar_border: rgb(55,54,44),  accent: rgb(230,219,116) },
    Theme { name: "Solarized",   bg: rgb(0,43,54),    bull: rgb(133,153,0),   bear: rgb(220,50,47),   dim: rgb(131,148,150), toolbar_bg: rgb(0,37,48),   toolbar_border: rgb(7,54,66),   accent: rgb(42,161,152) },
    Theme { name: "Dracula",     bg: rgb(40,42,54),   bull: rgb(80,250,123),  bear: rgb(255,85,85),   dim: rgb(189,147,249), toolbar_bg: rgb(34,36,48),  toolbar_border: rgb(52,55,70),  accent: rgb(255,121,198) },
    Theme { name: "Gruvbox",     bg: rgb(40,40,40),   bull: rgb(184,187,38),  bear: rgb(251,73,52),   dim: rgb(213,196,161), toolbar_bg: rgb(34,34,34),  toolbar_border: rgb(55,52,50),  accent: rgb(254,128,25) },
    Theme { name: "Catppuccin",  bg: rgb(30,30,46),   bull: rgb(166,227,161), bear: rgb(243,139,168), dim: rgb(180,190,254), toolbar_bg: rgb(24,24,38),  toolbar_border: rgb(49,50,68),  accent: rgb(203,166,247) },
    Theme { name: "Tokyo Night", bg: rgb(26,27,38),   bull: rgb(158,206,106), bear: rgb(247,118,142), dim: rgb(122,162,247), toolbar_bg: rgb(21,22,32),  toolbar_border: rgb(36,40,59),  accent: rgb(125,207,255) },
    // ── Additional themes ──
    Theme { name: "Kanagawa",    bg: rgb(22,22,29),   bull: rgb(118,169,130), bear: rgb(195,64,67),   dim: rgb(84,88,104),   toolbar_bg: rgb(18,18,24),  toolbar_border: rgb(34,34,46),  accent: rgb(127,180,202) },
    Theme { name: "Everforest",  bg: rgb(39,46,38),   bull: rgb(167,192,128), bear: rgb(230,126,128), dim: rgb(157,169,140), toolbar_bg: rgb(33,40,32),  toolbar_border: rgb(52,60,50),  accent: rgb(131,165,152) },
    Theme { name: "Vesper",      bg: rgb(16,16,16),   bull: rgb(166,218,149), bear: rgb(238,130,98),  dim: rgb(120,120,120), toolbar_bg: rgb(11,11,11),  toolbar_border: rgb(36,36,36),  accent: rgb(255,199,119) },
    Theme { name: "Rosé Pine",   bg: rgb(25,23,36),   bull: rgb(156,207,216), bear: rgb(235,111,146), dim: rgb(110,106,134), toolbar_bg: rgb(20,18,30),  toolbar_border: rgb(38,35,53),  accent: rgb(196,167,231) },
];

const PRESET_COLORS: &[&str] = &["#4a9eff","#e74c3c","#2ecc71","#f39c12","#9b59b6","#1abc9c","#e67e22","#3498db","#e91e63","#00bcd4","#8bc34a","#ff5722","#607d8b","#795548","#cddc39","#ff9800"];

// ─── Simulation constants ────────────────────────────────────────────────────
const SIM_TICK_FRAMES: u64 = 5;           // Update price every N frames (~12 ticks/sec at 60fps)
const SIM_CANDLE_MS: u128 = 3000;         // New simulated candle every 3s
const SIM_VOLATILITY: f32 = 0.0005;       // Per-tick price change magnitude (~0.05%)
const SIM_REVERSION: f32 = 0.003;         // Mean-reversion strength toward candle open
const SIM_VOL_BASE: f32 = 1000.0;         // Minimum volume per tick
const SIM_VOL_RANGE: f32 = 8000.0;        // Random volume range above base
const SIM_DEFAULT_INTERVAL: i64 = 300;    // Default bar interval (5 min) when no timestamps
const AUTO_SCROLL_RESUME_SECS: u64 = 5;   // Resume auto-scroll after N seconds of inactivity
const MAX_RECENT_SYMBOLS: usize = 20;     // Max entries in recent symbols list
const MAX_SEARCH_RESULTS: usize = 15;     // Max Yahoo/static search results

// Shared helpers
use super::ui::style::{hex_to_color, dashed_line, draw_line_rgba, section_label, dim_label, color_alpha, separator, status_badge, order_card, action_btn, trade_btn, close_button, dialog_window_themed, dialog_header, dialog_separator_shadow, dialog_section};
use super::compute::{compute_sma, compute_ema, compute_rsi, compute_macd, compute_stochastic, compute_vwap, detect_divergences, bs_price, strike_interval, atm_strike, get_iv, sim_oi};

// compute_sma, compute_ema — now in compute.rs

// ─── Layout ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum Layout { One, Two, TwoH, Three, Four, Six, SixH, Nine }

impl Layout {
    fn max_panes(self) -> usize { match self { Layout::One=>1, Layout::Two|Layout::TwoH=>2, Layout::Three=>3, Layout::Four=>4, Layout::Six|Layout::SixH=>6, Layout::Nine=>9 } }
    fn label(self) -> &'static str { match self { Layout::One=>"1", Layout::Two=>"2", Layout::TwoH=>"2H", Layout::Three=>"3", Layout::Four=>"4", Layout::Six=>"6", Layout::SixH=>"6H", Layout::Nine=>"9" } }
    /// Returns (col, row) grid dimensions for each pane in the layout, given the total rect.
    /// For Layout::Three, returns a custom arrangement: 1 full-width top (60%) + 2 bottom (40%).
    fn pane_rects(self, rect: egui::Rect, count: usize) -> Vec<egui::Rect> {
        if count == 0 { return vec![]; }
        let gap = 1.0;
        match self {
            Layout::Three if count >= 2 => {
                let top_h = (rect.height() * 0.6) - gap * 0.5;
                let bot_h = rect.height() - top_h - gap;
                let top = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), top_h));
                let bot_count = (count - 1).min(2);
                let bw = (rect.width() - gap * (bot_count as f32 - 1.0).max(0.0)) / bot_count as f32;
                let mut rects = vec![top];
                for i in 0..bot_count {
                    rects.push(egui::Rect::from_min_size(
                        egui::pos2(rect.left() + i as f32 * (bw + gap), rect.top() + top_h + gap),
                        egui::vec2(bw, bot_h),
                    ));
                }
                rects
            }
            _ => {
                let (cols, rows) = match self {
                    Layout::One => (1, 1),
                    Layout::Two => (2, 1),
                    Layout::TwoH => (1, 2),
                    Layout::Three => (2, 2), // fallback if count < 2
                    Layout::Four => (2, 2),
                    Layout::Six => (3, 2),
                    Layout::SixH => (2, 3),
                    Layout::Nine => (3, 3),
                };
                let cw = (rect.width() - gap * (cols as f32 - 1.0).max(0.0)) / cols as f32;
                let rh = (rect.height() - gap * (rows as f32 - 1.0).max(0.0)) / rows as f32;
                let mut rects = Vec::new();
                for r in 0..rows {
                    for c in 0..cols {
                        if rects.len() >= count { break; }
                        rects.push(egui::Rect::from_min_size(
                            egui::pos2(rect.left() + c as f32 * (cw + gap), rect.top() + r as f32 * (rh + gap)),
                            egui::vec2(cw, rh),
                        ));
                    }
                }
                rects
            }
        }
    }
}

const ALL_LAYOUTS: &[Layout] = &[Layout::One, Layout::Two, Layout::TwoH, Layout::Three, Layout::Four, Layout::Six, Layout::SixH, Layout::Nine];

// ─── Indicators ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum IndicatorType { SMA, EMA, WMA, DEMA, TEMA, VWAP, RSI, MACD, Stochastic }

#[derive(Debug, Clone, Copy, PartialEq)]
enum IndicatorCategory { Overlay, Oscillator }

impl IndicatorType {
    fn label(self) -> &'static str {
        match self {
            Self::SMA => "SMA", Self::EMA => "EMA", Self::WMA => "WMA",
            Self::DEMA => "DEMA", Self::TEMA => "TEMA", Self::VWAP => "VWAP",
            Self::RSI => "RSI", Self::MACD => "MACD", Self::Stochastic => "STOCH",
        }
    }
    fn all() -> &'static [Self] { &[Self::SMA, Self::EMA, Self::WMA, Self::DEMA, Self::TEMA, Self::VWAP, Self::RSI, Self::MACD, Self::Stochastic] }
    #[allow(dead_code)]
    fn overlays() -> &'static [Self] { &[Self::SMA, Self::EMA, Self::WMA, Self::DEMA, Self::TEMA, Self::VWAP] }
    #[allow(dead_code)]
    fn oscillators() -> &'static [Self] { &[Self::RSI, Self::MACD, Self::Stochastic] }
    fn category(self) -> IndicatorCategory {
        match self { Self::RSI | Self::MACD | Self::Stochastic => IndicatorCategory::Oscillator, _ => IndicatorCategory::Overlay }
    }

    fn compute(self, closes: &[f32], period: usize) -> Vec<f32> {
        match self {
            Self::SMA => compute_sma(closes, period),
            Self::EMA => compute_ema(closes, period),
            Self::WMA => super::compute::compute_wma(closes, period),
            Self::DEMA => super::compute::compute_dema(closes, period),
            Self::TEMA => super::compute::compute_tema(closes, period),
            Self::VWAP => vec![f32::NAN; closes.len()], // computed separately with volume
            Self::RSI => compute_rsi(closes, period),
            Self::MACD => compute_ema(closes, period), // primary=MACD line, signal/histogram set separately
            Self::Stochastic => vec![f32::NAN; closes.len()], // computed separately with high/low
        }
    }
}

#[derive(Debug, Clone)]
struct Indicator {
    id: u32,
    kind: IndicatorType,
    period: usize,
    source_tf: String,
    color: String,
    thickness: f32,
    line_style: LineStyle,
    visible: bool,
    values: Vec<f32>,         // primary line (same length as chart bars)
    values2: Vec<f32>,        // secondary line: MACD signal, Stochastic %D
    histogram: Vec<f32>,      // MACD histogram
    divergences: Vec<i8>,     // 1=bullish divergence, -1=bearish, 0=none
    // Cross-timeframe state
    source_bars: Vec<Bar>,
    source_timestamps: Vec<i64>,
    source_loaded: bool,
}

const INDICATOR_TIMEFRAMES: &[&str] = &["", "1m", "5m", "15m", "30m", "1h", "4h", "1d", "1wk"];

#[allow(dead_code)]
impl Indicator {
    fn new(id: u32, kind: IndicatorType, period: usize, color: &str) -> Self {
        Self { id, kind, period, source_tf: String::new(), color: color.into(), thickness: 1.2,
               line_style: LineStyle::Solid, visible: true, values: vec![], values2: vec![], histogram: vec![], divergences: vec![],
               source_bars: vec![], source_timestamps: vec![], source_loaded: false }
    }
    fn display_name(&self) -> String {
        let tf = if self.source_tf.is_empty() { "Chart" } else { &self.source_tf };
        format!("{} {} ({})", self.kind.label(), self.period, tf)
    }
    fn source_label(&self) -> &str {
        if self.source_tf.is_empty() { "Chart" } else { &self.source_tf }
    }
}

static INDICATOR_COLORS: &[&str] = &["#00bef0", "#f0961a", "#f0d732", "#b266e6", "#1abc9c", "#e74c3c", "#3498db", "#e67e22"];

// compute_rsi, compute_macd, compute_stochastic, compute_vwap, detect_divergences — now in compute.rs

// ─── Signal drawings (auto-generated trendlines from analysis server) ────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SignalDrawing {
    id: String,
    symbol: String,
    drawing_type: String, // "trendline", "hline", "hzone"
    points: Vec<(i64, f32)>, // (unix_timestamp, price)
    color: String,
    opacity: f32,
    thickness: f32,
    line_style: LineStyle,
    strength: f32, // 0.0-1.0, how confident the analysis is
    timeframe: String,
}

impl SignalDrawing {
    /// Convert timestamp to fractional bar index using the chart's timestamp array.
    fn time_to_bar(ts: i64, timestamps: &[i64]) -> f32 {
        if timestamps.is_empty() { return 0.0; }
        // Binary search for the closest bar
        let pos = timestamps.partition_point(|&t| t < ts);
        if pos == 0 { return 0.0; }
        if pos >= timestamps.len() { return timestamps.len() as f32 - 1.0; }
        // Interpolate between bars
        let t0 = timestamps[pos - 1];
        let t1 = timestamps[pos];
        if t1 == t0 { return pos as f32; }
        let frac = (ts - t0) as f32 / (t1 - t0) as f32;
        (pos - 1) as f32 + frac
    }
}

/// Convert a fractional bar index to a timestamp using interpolation.
/// Convert DTE (trading days) to calendar date, skipping weekends
fn trading_date(dte: i32) -> (u32, u32, u32) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let mut days_added = 0i32;
    let mut offset_days = 0i64;
    while days_added < dte {
        offset_days += 1;
        let ts = now as i64 + offset_days * 86400;
        let dow = ((ts / 86400 + 4) % 7) as u32;
        if dow != 0 && dow != 6 { days_added += 1; }
    }
    let total_secs = now as i64 + offset_days * 86400;
    let days_since_epoch = total_secs / 86400;
    let mut y = 1970i32; let mut remaining = days_since_epoch;
    loop {
        let diy = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < diy { break; }
        remaining -= diy; y += 1;
    }
    let md = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
        [31,29,31,30,31,30,31,31,30,31,30,31]
    } else { [31,28,31,30,31,30,31,31,30,31,30,31] };
    let mut m = 0u32;
    for d in &md { if remaining < *d as i64 { break; } remaining -= *d as i64; m += 1; }
    (y as u32, m + 1, remaining as u32 + 1)
}

fn trading_month_name(m: u32) -> &'static str {
    match m { 1=>"Jan",2=>"Feb",3=>"Mar",4=>"Apr",5=>"May",6=>"Jun",7=>"Jul",8=>"Aug",9=>"Sep",10=>"Oct",11=>"Nov",12=>"Dec",_=>"" }
}

fn dte_label(dte: i32) -> String {
    if dte == 0 { return "0DTE Today".into(); }
    let (_, m, d) = trading_date(dte);
    format!("{}DTE {} {}", dte, trading_month_name(m), d)
}

fn bar_to_time(bar: f32, timestamps: &[i64]) -> i64 {
    let idx = bar as usize;
    if timestamps.is_empty() { return 0; }
    if idx >= timestamps.len() { return *timestamps.last().unwrap_or(&0); }
    let frac = bar - idx as f32;
    if frac < 0.01 || idx + 1 >= timestamps.len() { return timestamps[idx]; }
    // Interpolate
    let t0 = timestamps[idx];
    let t1 = timestamps[idx + 1];
    t0 + ((t1 - t0) as f32 * frac) as i64
}

/// Fetch signal annotations from OCOCO API for a symbol.
fn fetch_signal_drawings(symbol: String) {
    let txs: Vec<std::sync::mpsc::Sender<super::ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }
    std::thread::spawn(move || {
        let url = format!("http://192.168.1.60:30300/api/annotations?symbol={}&source=signal", symbol);
        let client = reqwest::blocking::Client::builder().user_agent("apex-native").build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(3)).send() {
            if let Ok(json) = resp.json::<Vec<serde_json::Value>>() {
                let drawings: Vec<SignalDrawing> = json.iter().filter_map(|a| {
                    let id = a.get("id")?.as_str()?.to_string();
                    let sym = a.get("symbol")?.as_str()?.to_string();
                    let dtype = a.get("type")?.as_str().unwrap_or("trendline").to_string();
                    let points: Vec<(i64, f32)> = a.get("points")?.as_array()?.iter().filter_map(|p| {
                        Some((p.get("time")?.as_i64()?, p.get("price")?.as_f64()? as f32))
                    }).collect();
                    let style = a.get("style");
                    let color = style.and_then(|s| s.get("color")).and_then(|c| c.as_str()).unwrap_or("#4a9eff").to_string();
                    let opacity = style.and_then(|s| s.get("opacity")).and_then(|o| o.as_f64()).unwrap_or(0.7) as f32;
                    let thickness = style.and_then(|s| s.get("thickness")).and_then(|t| t.as_f64()).unwrap_or(1.0) as f32;
                    let ls_str = style.and_then(|s| s.get("lineStyle")).and_then(|l| l.as_str()).unwrap_or("dashed");
                    let line_style = match ls_str { "solid" => LineStyle::Solid, "dotted" => LineStyle::Dotted, _ => LineStyle::Dashed };
                    let strength = a.get("strength").and_then(|s| s.as_f64()).unwrap_or(0.5) as f32;
                    let timeframe = a.get("timeframe").and_then(|t| t.as_str()).unwrap_or("5m").to_string();
                    Some(SignalDrawing { id, symbol: sym, drawing_type: dtype, points, color, opacity, thickness, line_style, strength, timeframe })
                }).collect();

                if !drawings.is_empty() {
                    eprintln!("[signal] Fetched {} signal drawings for {}", drawings.len(), symbol);
                }
                // Send via command channel
                let cmd = super::ChartCommand::SignalDrawings { symbol, drawings_json: serde_json::to_string(&json).unwrap_or_default() };
                for tx in &txs { let _ = tx.send(cmd.clone()); }
            }
        }
    });
}

// ─── Orders ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum OrderSide { Buy, Sell, Stop, OcoTarget, OcoStop, TriggerBuy, TriggerSell }

#[derive(Debug, Clone, Copy, PartialEq)]
enum OrderStatus { Draft, Placed, Executed, Cancelled }

#[derive(Debug, Clone)]
struct OrderLevel {
    id: u32,
    side: OrderSide,
    price: f32,
    qty: u32,
    status: OrderStatus,
    pair_id: Option<u32>, // linked order (OCO target↔stop, trigger buy↔sell)
    // Option trigger metadata (only for TriggerBuy/TriggerSell on underlying chart)
    option_symbol: Option<String>,  // e.g. "SPY 560C 0DTE"
    option_con_id: Option<i64>,
}

impl OrderLevel {
    fn color(&self, t: &Theme) -> egui::Color32 {
        match self.side {
            OrderSide::Buy | OrderSide::TriggerBuy => t.bull,
            OrderSide::Sell | OrderSide::Stop | OrderSide::OcoStop | OrderSide::TriggerSell => t.bear,
            OrderSide::OcoTarget => egui::Color32::from_rgb(167, 139, 250), // purple
        }
    }
    fn label(&self) -> &'static str {
        match self.side {
            OrderSide::Buy => "BUY", OrderSide::Sell => "SELL", OrderSide::Stop => "STOP",
            OrderSide::OcoTarget => "OCO\u{2191}", OrderSide::OcoStop => "OCO\u{2193}",
            OrderSide::TriggerBuy => "TRIG\u{2191}", OrderSide::TriggerSell => "TRIG\u{2193}",
        }
    }
    fn notional(&self) -> f32 { self.price * self.qty as f32 }
}

/// Cancel an order and its paired leg (OCO/Trigger).
fn cancel_order_with_pair(orders: &mut Vec<OrderLevel>, id: u32) {
    let pair_id = orders.iter().find(|o| o.id == id).and_then(|o| o.pair_id);
    if let Some(o) = orders.iter_mut().find(|o| o.id == id) {
        o.status = OrderStatus::Cancelled;
    }
    if let Some(pid) = pair_id {
        if let Some(o) = orders.iter_mut().find(|o| o.id == pid) {
            if o.status == OrderStatus::Draft || o.status == OrderStatus::Placed {
                o.status = OrderStatus::Cancelled;
            }
        }
    }
}

fn fmt_notional(v: f32) -> String {
    if v >= 1_000_000.0 { format!("${:.1}M", v / 1_000_000.0) }
    else if v >= 1_000.0 { format!("${:.1}K", v / 1_000.0) }
    else { format!("${:.0}", v) }
}

// ─── Account & Positions (from ApexIB) ──────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct AccountSummary {
    nav: f64,
    buying_power: f64,
    excess_liquidity: f64,
    initial_margin: f64,
    maintenance_margin: f64,
    daily_pnl: f64,
    unrealized_pnl: f64,
    realized_pnl: f64,
    gross_position_value: f64,
    connected: bool,
    last_update: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
struct IbOrder {
    symbol: String,
    side: String,
    qty: i32,
    filled_qty: i32,
    order_type: String,
    limit_price: f64,
    avg_fill_price: f64,
    status: String,
    strike: f64,
    option_type: String,
    submitted_at: i64, // unix ms
}

#[derive(Debug, Clone)]
struct Position {
    symbol: String,
    qty: i32,         // positive=long, negative=short
    avg_price: f32,
    current_price: f32,
    market_value: f64,
    unrealized_pnl: f64,
    con_id: i64,
}

impl Position {
    fn pnl(&self) -> f32 { self.unrealized_pnl as f32 }
    fn pnl_pct(&self) -> f32 {
        if self.avg_price == 0.0 { return 0.0; }
        ((self.current_price - self.avg_price) / self.avg_price) * 100.0
    }
}

/// ApexIB endpoint configuration
const APEXIB_URL: &str = "https://apexib-dev.xllio.com";

// Shared account data — written by background worker, read by render thread
static ACCOUNT_DATA: std::sync::OnceLock<std::sync::Mutex<Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>>> = std::sync::OnceLock::new();

/// Start the account polling worker (call once). Polls ApexIB every 5 seconds.
fn start_account_poller() {
    use std::sync::OnceLock;
    static STARTED: OnceLock<bool> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = ACCOUNT_DATA.get_or_init(|| std::sync::Mutex::new(None));
        std::thread::spawn(|| {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(3))
                .build().unwrap_or_else(|_| reqwest::blocking::Client::new());
            loop {
                let mut summary = AccountSummary::default();
                let mut positions = Vec::new();

                // Fetch account summary
                if let Ok(resp) = client.get(format!("{}/account/summary", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        summary.connected = true;
                        summary.nav = json["netLiquidation"].as_f64().unwrap_or(0.0);
                        summary.buying_power = json["buyingPower"].as_f64().unwrap_or(0.0);
                        summary.excess_liquidity = json["excessLiquidity"].as_f64().unwrap_or(0.0);
                        summary.initial_margin = json["initMarginReq"].as_f64().unwrap_or(0.0);
                        summary.maintenance_margin = json["maintMarginReq"].as_f64().unwrap_or(0.0);
                        summary.gross_position_value = json["grossPositionValue"].as_f64().unwrap_or(0.0);
                        // Account summary also has unrealized/realized P&L
                        if summary.unrealized_pnl == 0.0 {
                            summary.unrealized_pnl = json["unrealizedPnL"].as_f64().unwrap_or(0.0);
                        }
                        if summary.realized_pnl == 0.0 {
                            summary.realized_pnl = json["realizedPnL"].as_f64().unwrap_or(0.0);
                        }
                    }
                }

                // Fetch P&L
                if let Ok(resp) = client.get(format!("{}/account/pnl", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        summary.daily_pnl = json["dailyPnL"].as_f64().unwrap_or(0.0);
                        summary.unrealized_pnl = json["unrealizedPnL"].as_f64().unwrap_or(0.0);
                        summary.realized_pnl = json["realizedPnL"].as_f64().unwrap_or(0.0);
                    }
                }

                // Fetch positions
                if let Ok(resp) = client.get(format!("{}/positions", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        if let Some(pos_arr) = json["positions"].as_array() {
                            for p in pos_arr {
                                positions.push(Position {
                                    symbol: p["symbol"].as_str().unwrap_or("").into(),
                                    qty: p["quantity"].as_i64().unwrap_or(0) as i32,
                                    avg_price: p["avgCost"].as_f64().unwrap_or(0.0) as f32,
                                    current_price: p["marketPrice"].as_f64().unwrap_or(0.0) as f32,
                                    market_value: p["marketValue"].as_f64().unwrap_or(0.0),
                                    unrealized_pnl: p["unrealizedPnl"].as_f64().unwrap_or(0.0),
                                    con_id: p["conId"].as_i64().unwrap_or(0),
                                });
                            }
                        }
                    }
                }

                // Fetch executions + pending + cancelled orders
                let mut ib_orders = Vec::new();
                // Only show orders from the last 24 hours
                let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
                let cutoff_ms = now_ms - 86_400_000; // 24 hours ago

                let parse_orders = |json: &serde_json::Value, key: &str, orders: &mut Vec<IbOrder>, cutoff: i64| {
                    if let Some(arr) = json[key].as_array() {
                        for o in arr {
                            let ts = o["submittedAt"].as_i64().or_else(|| o["time"].as_i64()).unwrap_or(0);
                            if ts > 0 && ts < cutoff { continue; } // skip old orders
                            orders.push(IbOrder {
                                symbol: o["symbol"].as_str().unwrap_or("").into(),
                                side: o["side"].as_str().or_else(|| o["action"].as_str()).unwrap_or("").into(),
                                qty: o["quantity"].as_i64().or_else(|| o["shares"].as_i64()).unwrap_or(0) as i32,
                                filled_qty: o["filledQty"].as_i64().or_else(|| o["shares"].as_i64()).unwrap_or(0) as i32,
                                order_type: o["orderType"].as_str().unwrap_or("").into(),
                                limit_price: o["limitPrice"].as_f64().or_else(|| o["price"].as_f64()).unwrap_or(0.0),
                                avg_fill_price: o["avgFillPrice"].as_f64().or_else(|| o["avgPrice"].as_f64()).or_else(|| o["price"].as_f64()).unwrap_or(0.0),
                                status: o["status"].as_str().unwrap_or(if key == "executions" { "filled" } else { "" }).into(),
                                strike: o["strike"].as_f64().unwrap_or(0.0),
                                option_type: o["optionType"].as_str().unwrap_or("").into(),
                                submitted_at: ts,
                            });
                        }
                    }
                };
                // Executions (filled trades)
                if let Ok(resp) = client.get(format!("{}/executions", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        parse_orders(&json, "executions", &mut ib_orders, cutoff_ms);
                    }
                }
                // Pending/submitted orders
                if let Ok(resp) = client.get(format!("{}/orders?status=submitted", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        parse_orders(&json, "orders", &mut ib_orders, cutoff_ms);
                    }
                }
                // Cancelled orders
                if let Ok(resp) = client.get(format!("{}/orders?status=cancelled", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        parse_orders(&json, "orders", &mut ib_orders, cutoff_ms);
                    }
                }

                summary.last_update = Some(std::time::Instant::now());

                if let Some(data) = ACCOUNT_DATA.get() {
                    if let Ok(mut d) = data.lock() { *d = Some((summary, positions, ib_orders)); }
                }

                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
        true
    });
}

/// Read latest account data (non-blocking)
fn read_account_data() -> Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)> {
    ACCOUNT_DATA.get()?.lock().ok()?.clone()
}

#[derive(Debug, Clone)]
struct Alert {
    id: u32,
    symbol: String,
    price: f32,
    above: bool, // true = alert when price goes above, false = below
    triggered: bool,
    message: String,
}

// ─── Trigger order (options on underlying price level) ──────────────────────

/// A placed trigger level — like an order level but for conditional options trades.
/// Lives on the underlying chart. Draggable, double-clickable.
#[derive(Debug, Clone)]
struct TriggerLevel {
    id: u32,
    side: OrderSide,         // BUY or SELL the option
    trigger_price: f32,      // underlying price that triggers the order
    above: bool,             // true = trigger when underlying >= price
    // Option contract
    symbol: String,           // underlying symbol
    option_type: String,      // "C" or "P"
    strike: f32,              // 0 = ATM
    expiry: String,           // "" = 0DTE
    qty: u32,
    submitted: bool,          // true = sent to IB
}

#[derive(Debug, Clone, PartialEq)]
enum TriggerPhase { Idle, Picking }

#[derive(Debug, Clone)]
struct TriggerSetup {
    phase: TriggerPhase,
    pending_side: OrderSide,  // which side we're placing
    option_type: String,
    strike: f32,
    expiry: String,
    qty: u32,
    // Pane management
    source_pane: usize,       // pane where the order panel is
    target_pane: Option<usize>, // pane with the underlying chart
}

impl Default for TriggerSetup {
    fn default() -> Self {
        Self {
            phase: TriggerPhase::Idle, pending_side: OrderSide::Buy,
            option_type: "C".into(), strike: 0.0, expiry: String::new(), qty: 1,
            source_pane: 0, target_pane: None,
        }
    }
}

// ─── Chart state ──────────────────────────────────────────────────────────────

struct Chart {
    symbol: String, timeframe: String,
    // Option chart metadata
    is_option: bool,
    underlying: String,       // e.g. "SPY" when this chart shows an option
    option_type: String,      // "C" or "P"
    option_strike: f32,
    option_expiry: String,    // "20260402"
    option_con_id: i64,
    bars: Vec<Bar>, timestamps: Vec<i64>, drawings: Vec<Drawing>,
    indicators: Vec<Indicator>,
    indicator_bar_count: usize, // bar count when indicators were last computed
    next_indicator_id: u32,
    editing_indicator: Option<u32>, // id of indicator being edited
    vs: f32, vc: u32, price_lock: Option<(f32,f32)>,
    auto_scroll: bool, last_input: std::time::Instant,
    history_loading: bool, // true while fetching older bars
    history_exhausted: bool, // true if no more history available
    tick_counter: u64, last_candle_time: std::time::Instant, sim_price: f32, sim_seed: u64,
    theme_idx: usize,
    draw_tool: String, // "", "hline", "trendline", "hzone", "barmarker"
    pending_pt: Option<(f32,f32)>,
    selected_id: Option<String>,
    selected_ids: Vec<String>, // multi-select with shift
    dragging_drawing: Option<(String, i32)>,
    drag_start_price: f32, drag_start_bar: f32,
    groups: Vec<DrawingGroup>,
    hidden_groups: Vec<String>,
    signal_drawings: Vec<SignalDrawing>, // auto-generated trendlines from server
    hide_signal_drawings: bool,
    drawings_requested: bool, // prevents duplicate fetch_drawings_background calls
    last_signal_fetch: std::time::Instant,
    hide_all_drawings: bool,
    hide_all_indicators: bool,
    show_volume: bool,
    show_oscillators: bool, // toggle oscillator sub-panel
    draw_color: String, // current drawing color
    zoom_selecting: bool, zoom_start: egui::Pos2,
    // Symbol picker
    picker_open: bool, picker_query: String,
    picker_results: Vec<(String, String, String)>, // (symbol, name, exchange/type)
    picker_last_query: String, // debounce: only search when query changes
    picker_searching: bool, // true while background search is in flight
    picker_rx: Option<mpsc::Receiver<Vec<(String, String, String)>>>, // receives search results from bg thread
    picker_pos: egui::Pos2, // anchor position for the popup
    recent_symbols: Vec<(String, String)>, // (symbol, name) — most recent first, max 20
    // Group management
    group_manager_open: bool,
    new_group_name: String,
    // Orders
    orders: Vec<OrderLevel>,
    next_order_id: u32,
    order_qty: u32,
    order_market: bool, // true=market, false=limit
    order_limit_price: String, // limit price as editable text
    order_type_idx: usize, // 0=MKT, 1=LMT, 2=STP, 3=STP-LMT, 4=TRAIL
    order_tif_idx: usize, // 0=DAY, 1=GTC, 2=IOC
    order_advanced: bool, // expanded mode
    order_bracket: bool, // bracket mode: entry + TP + SL
    order_stop_price: String, // stop trigger price (for STP, STP-LMT)
    order_trail_amt: String, // trailing amount (for TRAIL)
    order_tp_price: String, // take profit price (bracket)
    order_sl_price: String, // stop loss price (bracket)
    order_panel_pos: egui::Pos2, // draggable position (relative to chart rect)
    order_panel_dragging: bool,
    order_collapsed: bool, // true = show as pill, double-click to expand
    dragging_order: Option<u32>, // order id being dragged
    editing_order: Option<u32>,
    edit_order_qty: String,
    edit_order_price: String,
    armed: bool, // skip confirmation, fire orders immediately
    pending_confirms: Vec<(u32, std::time::Instant)>, // order ids awaiting user confirm from panel
    // ── Trigger orders (options on underlying price) ──
    trigger_setup: TriggerSetup,
    trigger_levels: Vec<TriggerLevel>,
    pending_und_order: Option<OrderSide>, // deferred: activate underlying crosshair
    next_trigger_id: u32,
    dragging_trigger: Option<u32>,
    editing_trigger: Option<u32>,
    // Measure tool (shift+drag)
    measuring: bool,
    measure_start: Option<(f32, f32)>, // (bar, price) start point
    measure_active: bool, // context menu activated measure mode
    // Symbol/timeframe change request — signals the App to reload data
    pending_symbol_change: Option<String>,
    pending_timeframe_change: Option<String>,
    // Cached formatted strings — updated only when data changes, not every frame
    #[allow(dead_code)] cached_ohlc: String,
    #[allow(dead_code)] cached_ohlc_bar_count: usize,
    // Reusable buffers to avoid per-frame allocations
    indicator_pts_buf: Vec<egui::Pos2>,
    fmt_buf: String, // reusable format buffer
}

impl Chart {
    fn new_with(symbol: &str, timeframe: &str) -> Self {
        let mut c = Self::new();
        c.symbol = symbol.into();
        c.timeframe = timeframe.into();
        c
    }
    fn new() -> Self {
        Self { symbol: "AAPL".into(), timeframe: "5m".into(),
            is_option: false, underlying: String::new(), option_type: String::new(),
            option_strike: 0.0, option_expiry: String::new(), option_con_id: 0,
            bars: vec![], timestamps: vec![], drawings: vec![], indicator_bar_count: 0,
            next_indicator_id: 5, editing_indicator: None,
            indicators: vec![
                Indicator::new(1, IndicatorType::SMA, 20, "#00bef0"),
                Indicator::new(2, IndicatorType::SMA, 50, "#f0961a"),
                Indicator::new(3, IndicatorType::EMA, 12, "#f0d732"),
                Indicator::new(4, IndicatorType::EMA, 26, "#b266e6"),
            ],
            vs: 0.0, vc: 200, price_lock: None, auto_scroll: true, history_loading: false, history_exhausted: false,
            last_input: std::time::Instant::now(), tick_counter: 0,
            last_candle_time: std::time::Instant::now(), sim_price: 0.0,
            sim_seed: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42),
            theme_idx: 5, // Gruvbox
            draw_tool: String::new(), pending_pt: None,
            selected_id: None, selected_ids: vec![], dragging_drawing: None,
            drag_start_price: 0.0, drag_start_bar: 0.0,
            groups: vec![DrawingGroup { id: "default".into(), name: "Temp".into(), color: None }],
            hidden_groups: vec![], hide_all_drawings: false, hide_all_indicators: false, show_volume: true, show_oscillators: true,
            signal_drawings: vec![], hide_signal_drawings: false, last_signal_fetch: std::time::Instant::now(), drawings_requested: false,
            draw_color: "#4a9eff".into(), group_manager_open: false, new_group_name: String::new(),
            zoom_selecting: false, zoom_start: egui::Pos2::ZERO,
            picker_open: false, picker_query: String::new(), picker_results: vec![],
            picker_last_query: String::new(), picker_searching: false, picker_rx: None, picker_pos: egui::Pos2::ZERO,
            recent_symbols: vec![("AAPL".into(), "Apple".into()), ("SPY".into(), "S&P 500 ETF".into()), ("TSLA".into(), "Tesla".into()), ("NVDA".into(), "Nvidia".into()), ("MSFT".into(), "Microsoft".into())],
            orders: vec![], next_order_id: 1, order_qty: 100, order_market: true, order_limit_price: String::new(),
            order_type_idx: 0, order_tif_idx: 0, order_advanced: false, order_bracket: false,
            order_stop_price: String::new(), order_trail_amt: String::new(),
            order_tp_price: String::new(), order_sl_price: String::new(),
            order_panel_pos: egui::pos2(8.0, -80.0), order_panel_dragging: false, order_collapsed: false,
            dragging_order: None, editing_order: None, edit_order_qty: String::new(), edit_order_price: String::new(),
            armed: false, pending_confirms: vec![],
            trigger_setup: TriggerSetup::default(), trigger_levels: vec![], next_trigger_id: 1, dragging_trigger: None, editing_trigger: None, pending_und_order: None,
            measuring: false, measure_start: None, measure_active: false,
            pending_symbol_change: None, pending_timeframe_change: None,
            cached_ohlc: String::new(), cached_ohlc_bar_count: 0,
            indicator_pts_buf: Vec::with_capacity(512), fmt_buf: String::with_capacity(256) }
    }
    fn process(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, timestamps, symbol, timeframe, .. } => {
                // Skip if this pane is an option chart and the LoadBars is for the underlying
                if self.is_option && symbol != self.symbol { return; }
                let is_new_symbol = self.symbol != symbol;
                self.symbol = symbol; self.timeframe = timeframe;
                self.bars = bars; self.timestamps = timestamps;
                self.vs = (self.bars.len() as f32 - self.vc as f32 + 8.0).max(0.0);
                self.sim_price = 0.0;
                self.last_candle_time = std::time::Instant::now();
                self.indicator_bar_count = 0; // force recompute
                // Drawings: fetch asynchronously via single worker thread
                if is_new_symbol { self.drawings_requested = false; self.drawings.clear(); }
                if !self.drawings_requested {
                    self.drawings_requested = true;
                    fetch_drawings_background(self.symbol.clone());
                }

                // Fetch signal drawings for new symbol
                self.signal_drawings.clear();
                self.last_signal_fetch = std::time::Instant::now();
                fetch_signal_drawings(self.symbol.clone());

                // Reload cross-timeframe indicator sources for new symbol
                for ind in &mut self.indicators {
                    if !ind.source_tf.is_empty() {
                        ind.source_loaded = false;
                        ind.source_bars.clear();
                        ind.source_timestamps.clear();
                        fetch_indicator_source(self.symbol.clone(), ind.source_tf.clone(), ind.id);
                    }
                }
            }
            ChartCommand::PrependBars { symbol, timeframe, bars, timestamps } => {
                self.history_loading = false;
                if symbol == self.symbol && timeframe == self.timeframe {
                    if bars.is_empty() {
                        // No data returned — no more history available
                        self.history_exhausted = true;
                        eprintln!("[history] exhausted for {} {}", symbol, timeframe);
                    } else {
                        // Deduplicate: only keep bars older than our earliest
                        let earliest_existing = self.timestamps.first().copied().unwrap_or(i64::MAX);
                        let new_count = timestamps.iter().take_while(|&&t| t < earliest_existing).count();
                        if new_count == 0 {
                            self.history_exhausted = true;
                            eprintln!("[history] no new unique bars for {} {} — exhausted", symbol, timeframe);
                        } else {
                            let mut new_bars: Vec<Bar> = bars[..new_count].to_vec();
                            let mut new_ts: Vec<i64> = timestamps[..new_count].to_vec();
                            new_bars.append(&mut self.bars);
                            new_ts.append(&mut self.timestamps);
                            self.bars = new_bars;
                            self.timestamps = new_ts;
                            self.vs += new_count as f32;
                            self.indicator_bar_count = 0;
                            eprintln!("[history] prepended {} bars for {} {} (total: {})", new_count, symbol, timeframe, self.bars.len());
                        }
                    }
                }
            }
            ChartCommand::AppendBar { bar, timestamp, .. } => {
                self.bars.push(bar); self.timestamps.push(timestamp);
                if self.auto_scroll { self.vs = (self.bars.len() as f32 - self.vc as f32 + 8.0).max(0.0); }
            }
            ChartCommand::UpdateLastBar { symbol, bar, .. } => {
                // Only update if tick is for the currently displayed symbol
                if symbol == self.symbol {
                    if let Some(l) = self.bars.last_mut() {
                        // Properly update candle — don't replace open
                        l.close = bar.close;
                        l.high = l.high.max(bar.close);
                        l.low = l.low.min(bar.close);
                        l.volume += bar.volume;
                        // Keep sim in sync with real ticks
                        self.sim_price = bar.close;
                    }
                }
            }
            ChartCommand::SetDrawing(d) => { self.drawings.retain(|x| x.id != d.id); self.drawings.push(d); }
            ChartCommand::RemoveDrawing { id } => { self.drawings.retain(|x| x.id != id); }
            ChartCommand::ClearDrawings => { self.drawings.clear(); }
            ChartCommand::LoadDrawings { symbol, drawings, groups } => {
                if symbol == self.symbol {
                    self.drawings = drawings;
                    self.groups = groups.into_iter().map(|g| super::DrawingGroup { id: g.id, name: g.name, color: g.color }).collect();
                }
            }
            ChartCommand::SignalDrawings { symbol, drawings_json } => {
                if symbol == self.symbol {
                    // Parse signal drawings from JSON
                    if let Ok(annotations) = serde_json::from_str::<Vec<serde_json::Value>>(&drawings_json) {
                        self.signal_drawings.clear();
                        for a in &annotations {
                            let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let dtype = a.get("type").and_then(|v| v.as_str()).unwrap_or("trendline").to_string();
                            let points: Vec<(i64, f32)> = a.get("points").and_then(|v| v.as_array()).map(|arr| {
                                arr.iter().filter_map(|p| Some((p.get("time")?.as_i64()?, p.get("price")?.as_f64()? as f32))).collect()
                            }).unwrap_or_default();
                            let style = a.get("style");
                            let color = style.and_then(|s| s.get("color")).and_then(|c| c.as_str()).unwrap_or("#4a9eff").to_string();
                            let opacity = style.and_then(|s| s.get("opacity")).and_then(|o| o.as_f64()).unwrap_or(0.7) as f32;
                            let thickness = style.and_then(|s| s.get("thickness")).and_then(|t| t.as_f64()).unwrap_or(1.0) as f32;
                            let ls = match style.and_then(|s| s.get("lineStyle")).and_then(|l| l.as_str()).unwrap_or("dashed") {
                                "solid" => LineStyle::Solid, "dotted" => LineStyle::Dotted, _ => LineStyle::Dashed,
                            };
                            let strength = a.get("strength").and_then(|s| s.as_f64()).unwrap_or(0.5) as f32;
                            let tf = a.get("timeframe").and_then(|t| t.as_str()).unwrap_or("5m").to_string();
                            self.signal_drawings.push(SignalDrawing { id, symbol: symbol.clone(), drawing_type: dtype, points, color, opacity, thickness, line_style: ls, strength, timeframe: tf });
                        }
                    }
                }
            }
            ChartCommand::IndicatorSourceBars { indicator_id, timeframe, bars, timestamps } => {
                if let Some(ind) = self.indicators.iter_mut().find(|i| i.id == indicator_id && i.source_tf == timeframe) {
                    ind.source_bars = bars;
                    ind.source_timestamps = timestamps;
                    ind.source_loaded = true;
                    self.indicator_bar_count = 0; // force recompute
                }
            }
            _ => {}
        }
    }
    /// Recompute all indicator values from bar data.
    fn recompute_indicators(&mut self) {
        let chart_closes: Vec<f32> = self.bars.iter().map(|b| b.close).collect();
        let chart_highs: Vec<f32> = self.bars.iter().map(|b| b.high).collect();
        let chart_lows: Vec<f32> = self.bars.iter().map(|b| b.low).collect();
        let chart_volumes: Vec<f32> = self.bars.iter().map(|b| b.volume).collect();

        for ind in &mut self.indicators {
            let closes = if ind.source_tf.is_empty() { &chart_closes } else if ind.source_loaded && !ind.source_bars.is_empty() {
                // For cross-timeframe, we'd need to map — for now use chart closes
                &chart_closes
            } else {
                ind.values = vec![f32::NAN; self.bars.len()];
                ind.values2 = vec![];
                ind.histogram = vec![];
                continue;
            };

            match ind.kind {
                IndicatorType::VWAP => {
                    ind.values = compute_vwap(closes, &chart_volumes, &chart_highs, &chart_lows);
                }
                IndicatorType::RSI => {
                    ind.values = compute_rsi(closes, ind.period);
                    ind.divergences = detect_divergences(closes, &ind.values, 5);
                }
                IndicatorType::MACD => {
                    let (macd, signal, hist) = compute_macd(closes, 12, 26, 9);
                    ind.values = macd;
                    ind.values2 = signal;
                    ind.histogram = hist;
                    ind.divergences = detect_divergences(closes, &ind.values, 5);
                }
                IndicatorType::Stochastic => {
                    let (k, d) = compute_stochastic(&chart_highs, &chart_lows, closes, ind.period.max(2), 3);
                    ind.values = k;
                    ind.values2 = d;
                    ind.divergences = detect_divergences(closes, &ind.values, 5);
                }
                _ => {
                    ind.values = ind.kind.compute(closes, ind.period);
                    ind.values2 = vec![];
                    ind.histogram = vec![];
                }
            }
        }
        self.indicator_bar_count = self.bars.len();
    }

    /// Update indicators — full recompute on data load or config change,
    /// incremental for single-bar appends (simulation).
    fn update_indicators(&mut self) {
        let n = self.bars.len();
        if n == self.indicator_bar_count { return; }

        // Full recompute needed
        if self.indicator_bar_count == 0 || n < self.indicator_bar_count || (n - self.indicator_bar_count) > 5 {
            self.recompute_indicators();
            return;
        }

        // Incremental: extend each indicator for newly added bars
        let old = self.indicator_bar_count;
        self.indicator_bar_count = n;
        for idx in old..n {
            let close = self.bars[idx].close;
            for ind in &mut self.indicators {
                match ind.kind {
                    IndicatorType::SMA | IndicatorType::WMA => {
                        if idx >= ind.period {
                            if ind.kind == IndicatorType::SMA {
                                let sum: f32 = self.bars[idx+1-ind.period..=idx].iter().map(|b| b.close).sum();
                                ind.values.push(sum / ind.period as f32);
                            } else {
                                let denom = (ind.period * (ind.period + 1)) / 2;
                                let mut s = 0.0;
                                for j in 0..ind.period { s += self.bars[idx + 1 - ind.period + j].close * (j + 1) as f32; }
                                ind.values.push(s / denom as f32);
                            }
                        } else { ind.values.push(f32::NAN); }
                    }
                    IndicatorType::EMA => {
                        let k = 2.0 / (ind.period as f32 + 1.0);
                        let prev = ind.values.last().copied().unwrap_or(f32::NAN);
                        let v = if prev.is_nan() {
                            if idx >= ind.period - 1 {
                                self.bars[idx+1-ind.period..=idx].iter().map(|b| b.close).sum::<f32>() / ind.period as f32
                            } else { f32::NAN }
                        } else { close * k + prev * (1.0 - k) };
                        ind.values.push(v);
                    }
                    _ => {
                        // DEMA, TEMA, VWAP, RSI, MACD, Stochastic — need full recompute
                        ind.values.push(f32::NAN);
                    }
                }
            }
        }
    }
    fn price_range(&self) -> (f32,f32) {
        if let Some(r) = self.price_lock { return r; }
        let s = self.vs as u32; let e = (s+self.vc).min(self.bars.len() as u32);
        let (mut lo,mut hi) = (f32::MAX,f32::MIN);
        for i in s..e { if let Some(b) = self.bars.get(i as usize) { lo=lo.min(b.low); hi=hi.max(b.high); } }
        if lo>=hi { lo-=0.5; hi+=0.5; }
        let p=(hi-lo)*0.05; (lo-p,hi+p)
    }
}

// ─── egui rendering ──────────────────────────────────────────────────────────

/// Run one tick of price simulation for a single pane.
fn new_uuid() -> String { uuid::Uuid::new_v4().to_string() }

/// Generate a 32x32 RGBA window icon — Apex triangle in orange on transparent bg.
fn make_window_icon() -> Option<winit::window::Icon> {
    let s: u32 = 32;
    let mut rgba = vec![0u8; (s * s * 4) as usize];
    let color = [254u8, 128, 25, 255]; // Gruvbox accent orange

    // Draw triangle outline: top-center to bottom-left to bottom-right
    let m = 3.0_f32; // margin
    let cx = s as f32 / 2.0;
    let top = (cx, m);
    let bl = (m, s as f32 - m);
    let br = (s as f32 - m, s as f32 - m);

    // Triangle sides
    draw_line_rgba(&mut rgba, s, top.0, top.1, bl.0, bl.1, 1.0, color);
    draw_line_rgba(&mut rgba, s, bl.0, bl.1, br.0, br.1, 1.0, color);
    draw_line_rgba(&mut rgba, s, br.0, br.1, top.0, top.1, 1.0, color);
    // Horizontal bar
    let bar_y = cx + 2.0;
    draw_line_rgba(&mut rgba, s, cx - 7.0, bar_y, cx + 7.0, bar_y, 1.0, color);

    winit::window::Icon::from_rgba(rgba, s, s).ok()
}

/// Create HICON in memory using CreateIconIndirect — no file needed.
#[cfg(target_os = "windows")]
fn make_window_icon_hicon() -> Option<isize> {
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    let s: i32 = 32;
    // Build BGRA pixel data (pre-multiplied alpha)
    let mut bgra = vec![0u8; (s * s * 4) as usize];
    let color_bgra = [25u8, 128, 254, 255]; // BGRA for orange #FE8019

    let m = 3.0_f32;
    let cx = s as f32 / 2.0;
    draw_line_rgba(&mut bgra, s as u32, cx, m, m, s as f32 - m, 1.0, color_bgra);
    draw_line_rgba(&mut bgra, s as u32, m, s as f32 - m, s as f32 - m, s as f32 - m, 1.0, color_bgra);
    draw_line_rgba(&mut bgra, s as u32, s as f32 - m, s as f32 - m, cx, m, 1.0, color_bgra);
    draw_line_rgba(&mut bgra, s as u32, cx - 7.0, cx + 2.0, cx + 7.0, cx + 2.0, 1.0, color_bgra);

    unsafe {
        // Create a DIB section for the color bitmap
        let hdc = GetDC(std::ptr::null_mut());
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = s;
        bmi.bmiHeader.biHeight = -(s); // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = 0; // BI_RGB

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbm_color = CreateDIBSection(hdc, &bmi, 0, &mut bits, std::ptr::null_mut(), 0);
        if !hbm_color.is_null() && !bits.is_null() {
            std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
        }

        // Create monochrome mask (all zeros = fully opaque where color has alpha)
        let hbm_mask = CreateBitmap(s, s, 1, 1, std::ptr::null());

        let mut ii: ICONINFO = std::mem::zeroed();
        ii.fIcon = 1; // TRUE = icon
        ii.hbmMask = hbm_mask;
        ii.hbmColor = hbm_color;

        let hicon = CreateIconIndirect(&ii);

        // Cleanup bitmaps (icon keeps its own copy)
        if !hbm_color.is_null() { DeleteObject(hbm_color as _); }
        if !hbm_mask.is_null() { DeleteObject(hbm_mask as _); }
        ReleaseDC(std::ptr::null_mut(), hdc);

        if !hicon.is_null() {
            eprintln!("[native-chart] Icon created via CreateIconIndirect");
            Some(hicon as isize)
        } else {
            eprintln!("[native-chart] Warning: CreateIconIndirect failed");
            None
        }
    }
}

/// Convert a native Drawing to DbDrawing for persistence.
fn drawing_to_db(d: &Drawing, symbol: &str, timeframe: &str) -> crate::drawing_db::DbDrawing {
    let (drawing_type, points) = match &d.kind {
        DrawingKind::HLine { price } => ("hline".into(), vec![(0.0, *price as f64)]),
        DrawingKind::TrendLine { price0, time0, price1, time1 } => ("trendline".into(), vec![(*time0 as f64, *price0 as f64), (*time1 as f64, *price1 as f64)]),
        DrawingKind::HZone { price0, price1 } => ("hzone".into(), vec![(0.0, *price0 as f64), (0.0, *price1 as f64)]),
        DrawingKind::BarMarker { time, price, up } => ("barmarker".into(), vec![(*time as f64, *price as f64), (if *up { 1.0 } else { 0.0 }, 0.0)]),
    };
    let ls = match d.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" };
    crate::drawing_db::DbDrawing {
        id: d.id.clone(), symbol: symbol.into(), timeframe: timeframe.into(),
        drawing_type, points, color: d.color.clone(), opacity: d.opacity,
        line_style: ls.into(), thickness: d.thickness, group_id: d.group_id.clone(),
    }
}

/// Convert a DbDrawing to native Drawing.
fn db_to_drawing(d: &crate::drawing_db::DbDrawing) -> Option<Drawing> {
    let kind = match d.drawing_type.as_str() {
        "hline" => DrawingKind::HLine { price: d.points.first()?.1 as f32 },
        "trendline" => {
            let p0 = d.points.get(0)?;
            let p1 = d.points.get(1)?;
            DrawingKind::TrendLine { time0: p0.0 as i64, price0: p0.1 as f32, time1: p1.0 as i64, price1: p1.1 as f32 }
        }
        "hzone" => DrawingKind::HZone { price0: d.points.get(0)?.1 as f32, price1: d.points.get(1)?.1 as f32 },
        "barmarker" => DrawingKind::BarMarker { time: d.points.get(0)?.0 as i64, price: d.points.get(0)?.1 as f32, up: d.points.get(1).map(|p| p.0 > 0.5).unwrap_or(true) },
        _ => return None,
    };
    let ls = match d.line_style.as_str() { "dashed" => LineStyle::Dashed, "dotted" => LineStyle::Dotted, _ => LineStyle::Solid };
    let mut drawing = Drawing::new(d.id.clone(), kind);
    drawing.color = d.color.clone();
    drawing.opacity = d.opacity;
    drawing.line_style = ls;
    drawing.thickness = d.thickness;
    drawing.group_id = d.group_id.clone();
    Some(drawing)
}

fn tick_simulation(chart: &mut Chart) {
    if !chart.bars.is_empty() {
        // Init sim_price from last bar's close — and immediately create a new
        // candle so the simulation never overwrites historical data.
        if chart.sim_price == 0.0 {
            chart.sim_price = chart.bars.last().map(|b| b.close).unwrap_or(100.0);
            chart.last_candle_time = std::time::Instant::now();
            // Create first sim candle so ticks don't touch real bars
            let last_ts = chart.timestamps.last().copied().unwrap_or(0);
            let interval = if chart.timestamps.len() > 1 {
                chart.timestamps[chart.timestamps.len()-1] - chart.timestamps[chart.timestamps.len()-2]
            } else { SIM_DEFAULT_INTERVAL };
            chart.bars.push(Bar {
                open: chart.sim_price, high: chart.sim_price, low: chart.sim_price,
                close: chart.sim_price, volume: 0.0, _pad: 0.0,
            });
            chart.timestamps.push(last_ts + interval);
        }

        chart.tick_counter += 1;

        let rng = |seed: &mut u64| -> f32 {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (*seed >> 33) as f32 / u32::MAX as f32
        };
        let r1 = rng(&mut chart.sim_seed);
        let r2 = rng(&mut chart.sim_seed);

        // Tick every ~5 frames (~12x/sec) — update last (simulated) bar
        if chart.tick_counter % SIM_TICK_FRAMES == 0 {
            let normal = (-2.0 * r1.max(0.0001).ln()).sqrt() * (2.0 * std::f32::consts::PI * r2).cos();
            let base_open = chart.bars.last().map(|b| b.open).unwrap_or(chart.sim_price);
            let reversion = (base_open - chart.sim_price) * SIM_REVERSION;
            let change = normal * chart.sim_price * SIM_VOLATILITY + reversion;
            chart.sim_price += change;
            let volume_tick = (r1 * SIM_VOL_RANGE + SIM_VOL_BASE) * (1.0 + normal.abs());

            if let Some(last) = chart.bars.last_mut() {
                last.close = chart.sim_price;
                last.high = last.high.max(chart.sim_price);
                last.low = last.low.min(chart.sim_price);
                last.volume += volume_tick;
            }
        }

        // New candle every ~3 seconds (cap at 10K bars to prevent unbounded growth)
        if chart.last_candle_time.elapsed().as_millis() >= SIM_CANDLE_MS && chart.bars.len() < 10_000 {
            chart.last_candle_time = std::time::Instant::now();
            let last_ts = chart.timestamps.last().copied().unwrap_or(0);
            let interval = if chart.timestamps.len() > 1 {
                chart.timestamps[chart.timestamps.len()-1] - chart.timestamps[chart.timestamps.len()-2]
            } else { SIM_DEFAULT_INTERVAL };
            chart.bars.push(Bar {
                open: chart.sim_price, high: chart.sim_price, low: chart.sim_price,
                close: chart.sim_price, volume: 0.0, _pad: 0.0,
            });
            chart.timestamps.push(last_ts + interval);
        }

        if chart.auto_scroll {
            chart.vs = (chart.bars.len() as f32 - chart.vc as f32 + 8.0).max(0.0);
        }

    }

    if !chart.auto_scroll && chart.last_input.elapsed().as_secs() >= AUTO_SCROLL_RESUME_SECS {
        chart.auto_scroll = true; chart.price_lock = None;
        chart.vs = (chart.bars.len() as f32 - chart.vc as f32 + 8.0).max(0.0);
    }
}

fn draw_chart(ctx: &egui::Context, panes: &mut Vec<Chart>, active_pane: &mut usize, layout: &mut Layout, watchlist: &mut Watchlist, toasts: &[(String, f32, std::time::Instant, bool)], conn_panel_open: &mut bool, rx: &mpsc::Receiver<ChartCommand>) {
    use crate::monitoring::{span_begin, span_end};

    // ── Watchlist divider drag (handled at top level to avoid panel interference) ──
    if watchlist.divider_y > 0.0 && watchlist.options_visible {
        let pointer_pos = ctx.input(|i| i.pointer.latest_pos());
        let primary_pressed = ctx.input(|i| i.pointer.primary_pressed());
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let primary_released = ctx.input(|i| i.pointer.primary_released());

        // Start drag on press near divider
        if primary_pressed {
            if let Some(pos) = pointer_pos {
                if (pos.y - watchlist.divider_y).abs() < 10.0 {
                    watchlist.divider_dragging = true;
                }
            }
        }
        // During drag, compute split from absolute Y position
        if watchlist.divider_dragging && primary_down {
            if let Some(pos) = pointer_pos {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                // divider_y is the absolute screen Y of the divider
                // divider_total_h is the total height available for stocks+options
                // The stocks area starts at divider_y - stocks_h and ends at divider_y
                // We want: new divider_y = pos.y, solve for split
                let stocks_start_y = watchlist.divider_y - watchlist.divider_total_h * watchlist.options_split;
                let new_split = (pos.y - stocks_start_y) / watchlist.divider_total_h;
                watchlist.options_split = new_split.clamp(0.15, 0.85);
            }
        }
        // End drag
        if primary_released && watchlist.divider_dragging {
            watchlist.divider_dragging = false;
        }
    }

    // Route incoming commands to the matching pane (by symbol), or watchlist
    span_begin("cmd_routing");
    while let Ok(cmd) = rx.try_recv() {
        match &cmd {
            // Pane-targeted commands: route by symbol
            ChartCommand::LoadBars { symbol, .. } | ChartCommand::UpdateLastBar { symbol, .. } | ChartCommand::AppendBar { symbol, .. } | ChartCommand::PrependBars { symbol, .. } | ChartCommand::LoadDrawings { symbol, .. } => {
                let s = symbol.clone();
                if let Some(p) = panes.iter_mut().find(|p| p.symbol == s) { p.process(cmd); }
                else if let Some(p) = panes.get_mut(*active_pane) {
                    if !p.is_option { p.process(cmd); }
                }
            }
            // Watchlist-targeted commands: handle directly
            ChartCommand::WatchlistPrice { symbol, price, prev_close } => {
                watchlist.set_price(symbol, *price);
                watchlist.set_prev_close(symbol, *prev_close);
            }
            ChartCommand::ChainData { symbol, dte, calls, puts } => {
                if *symbol == watchlist.chain_symbol {
                    let to_rows = |data: &[(f32,f32,f32,f32,i32,i32,f32,bool,String)]| -> Vec<OptionRow> {
                        data.iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                            strike: *strike, last: *last, bid: *bid, ask: *ask,
                            volume: *vol, oi: *oi, iv: *iv, itm: *itm, contract: contract.clone(),
                        }).collect()
                    };
                    if *dte == 0 {
                        watchlist.chain_0dte = (to_rows(calls), to_rows(puts));
                    } else {
                        watchlist.chain_far = (to_rows(calls), to_rows(puts));
                    }
                    watchlist.chain_loading = false;
                    eprintln!("[chain] Loaded {} calls + {} puts for {} dte={}",
                        if *dte == 0 { watchlist.chain_0dte.0.len() } else { watchlist.chain_far.0.len() },
                        if *dte == 0 { watchlist.chain_0dte.1.len() } else { watchlist.chain_far.1.len() },
                        symbol, dte);
                }
            }
            ChartCommand::SearchResults { query, results, source } => {
                if source == "watchlist" && !query.is_empty()
                    && watchlist.search_query.to_lowercase().starts_with(&query.to_lowercase()) {
                    for (sym, name) in results {
                        if !watchlist.search_results.iter().any(|(s, _)| s == sym) {
                            watchlist.search_results.push((sym.clone(), name.clone()));
                        }
                    }
                } else if source == "chain" && !query.is_empty()
                    && watchlist.chain_sym_input.to_lowercase().starts_with(&query.to_lowercase()) {
                    for (sym, name) in results {
                        if !watchlist.search_results.iter().any(|(s, _)| s == sym) {
                            watchlist.search_results.push((sym.clone(), name.clone()));
                        }
                    }
                }
            }
            // Everything else goes to active pane
            _ => {
                if let Some(p) = panes.get_mut(*active_pane) { p.process(cmd); }
            }
        }
    }
    if *active_pane >= panes.len() { *active_pane = 0; }

    // ── History pagination check (active pane only) ──
    {
        let ap = *active_pane;
        if ap < panes.len() {
            let chart = &mut panes[ap];
            // Trigger when left edge of viewport is within 30 bars of start of data
            let threshold = 30.0;
            if !chart.auto_scroll && chart.vs < threshold && !chart.history_loading && !chart.history_exhausted
                && !chart.bars.is_empty() && chart.timestamps.len() > 1 {
                chart.history_loading = true;
                let sym = chart.symbol.clone();
                let tf = chart.timeframe.clone();
                let earliest_ts = chart.timestamps[0];
                eprintln!("[history] TRIGGERED for {} {} (vs={:.1}, bars={})", sym, tf, chart.vs, chart.bars.len());
                fetch_history_background(sym, tf, earliest_ts);
            }
        }
    }

    // Simulation + indicators for all panes
    span_begin("simulation_indicators");
    for chart in panes.iter_mut() {
        chart.update_indicators();
        tick_simulation(chart);
    }
    span_end();

    let theme_idx = panes[*active_pane].theme_idx;
    let t = &THEMES[theme_idx];
    let ap = *active_pane;
    // Store window ref for drag/minimize/maximize/close
    let win_ref: Option<Arc<Window>> = {
        // Find the window that's currently rendering (first visible window)
        // We pass it through a thread-local since draw_chart doesn't have access to ChartWindow
        CURRENT_WINDOW.with(|w| w.borrow().clone())
    };

    span_begin("top_panel");

    // Toolbar button helper — matches WebView's btnStyle: 11px monospace, 3px radius, 2px 8px padding
    let tb_btn = |ui: &mut egui::Ui, label: &str, active: bool, t: &Theme| -> egui::Response {
        super::ui::style::tb_btn(ui, label, active, t.accent, t.dim, t.toolbar_bg, t.toolbar_border)
    };

    egui::TopBottomPanel::top("tb")
        .frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin { left: 10, right: 0, top: 0, bottom: 0 }))
        .exact_height(36.0)
        .show(ctx, |ui| {
        let tb_rect = ui.max_rect();
        // Bottom border line
        ui.painter().line_segment(
            [egui::pos2(tb_rect.left(), tb_rect.bottom()), egui::pos2(tb_rect.right(), tb_rect.bottom())],
            egui::Stroke::new(1.0, t.toolbar_border),
        );

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // ── Logo ──
            let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::hover());
            let lp = ui.painter_at(logo_rect);
            let lc = logo_rect.center();
            lp.add(egui::Shape::line(vec![
                egui::pos2(lc.x, lc.y - 6.0), egui::pos2(lc.x + 6.0, lc.y + 5.0),
                egui::pos2(lc.x - 6.0, lc.y + 5.0), egui::pos2(lc.x, lc.y - 6.0),
            ], egui::Stroke::new(1.3, t.accent)));
            lp.line_segment([egui::pos2(lc.x - 3.5, lc.y + 1.0), egui::pos2(lc.x + 3.5, lc.y + 1.0)], egui::Stroke::new(1.3, t.accent));

            ui.add_space(2.0);

            // ── Symbol ticker ──
            let sym_label = format!("{} \u{25BE}", panes[ap].symbol); // ▾ dropdown arrow
            let sym_btn = ui.add(egui::Button::new(
                egui::RichText::new(&sym_label).monospace().size(12.0).strong().color(t.accent)
            ).frame(false));
            if sym_btn.clicked() {
                panes[ap].picker_open = !panes[ap].picker_open;
                panes[ap].picker_query.clear();
                panes[ap].picker_results.clear();
                panes[ap].picker_last_query.clear();
                panes[ap].picker_pos = egui::pos2(sym_btn.rect.left(), sym_btn.rect.bottom());
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Scrollable middle section ──
            // Calculate available width: total - logo(25) - symbol(~70) - right section(~350)
            let right_width = 110.0; // only window controls (3 × 34px + separator)
            let middle_width = (ui.available_width() - right_width).max(60.0);
            egui::ScrollArea::horizontal().max_width(middle_width).show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // ── Timeframes ──
            for &tf in &["1m","5m","15m","30m","1h","4h","1d","1wk"] {
                let is_active_tf = panes[ap].timeframe == tf;
                if tb_btn(ui, tf, is_active_tf, t).clicked() && !is_active_tf {
                    panes[ap].pending_timeframe_change = Some(tf.to_string());
                }
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Volume + Oscillator toggles ──
            if tb_btn(ui, "VOL", panes[ap].show_volume, t).clicked() { panes[ap].show_volume = !panes[ap].show_volume; }
            if tb_btn(ui, "OSC", panes[ap].show_oscillators, t).clicked() { panes[ap].show_oscillators = !panes[ap].show_oscillators; }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Drawing tools ──
            for (tool, label) in [("trendline", "trend"), ("hline", "hline"), ("hzone", "zone"), ("barmarker", "mark")] {
                let active = panes[ap].draw_tool == tool;
                if tb_btn(ui, label, active, t).clicked() {
                    panes[ap].draw_tool = if active { String::new() } else { tool.into() };
                    panes[ap].pending_pt = None;
                }
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Layouts ──
            for &ly in ALL_LAYOUTS {
                let is_cur = *layout == ly;
                if tb_btn(ui, ly.label(), is_cur, t).clicked() && !is_cur {
                    let max = ly.max_panes();
                    while panes.len() < max {
                        let syms = ["SPY","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOG","AMD"];
                        let sym = syms.get(panes.len()).unwrap_or(&"SPY");
                        let mut p = Chart::new_with(sym, &panes[0].timeframe);
                        p.theme_idx = panes[0].theme_idx;
                        p.recent_symbols = panes[0].recent_symbols.clone();
                        p.pending_symbol_change = Some(sym.to_string());
                        panes.push(p);
                    }
                    *layout = ly;
                    if *active_pane >= max { *active_pane = 0; }
                }
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Theme dropdown ──
            {
                let mut ti = panes[ap].theme_idx;
                egui::ComboBox::from_id_salt("thm").selected_text(
                    egui::RichText::new(THEMES[ti].name).monospace().size(11.0).color(t.dim)
                ).width(90.0).show_ui(ui, |ui| {
                    for (i, th) in THEMES.iter().enumerate() { ui.selectable_value(&mut ti, i, th.name); }
                });
                if ti != panes[ap].theme_idx { for p in panes.iter_mut() { p.theme_idx = ti; } }
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // Placeholder buttons for future features
            tb_btn(ui, "triangulator", false, t);
            tb_btn(ui, "auto target", false, t);

            // Trendline filter button
            if tb_btn(ui, &format!("{} filters", Icon::FUNNEL), watchlist.trendline_filter_open, t).clicked() {
                watchlist.trendline_filter_open = !watchlist.trendline_filter_open;
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // Indicator dropdown (add new indicator from toolbar)
            let ind_resp = tb_btn(ui, &format!("{} ind", Icon::PLUS), false, t);
            if ind_resp.clicked() {
                // Show indicator type menu below the button
                ui.memory_mut(|m| m.toggle_popup(egui::Id::new("ind_add_popup")));
            }
            egui::popup_below_widget(ui, egui::Id::new("ind_add_popup"), &ind_resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                ui.set_min_width(160.0);
                section_label(ui, "OVERLAYS", t.accent);
                for &kind in IndicatorType::overlays() {
                    if ui.button(egui::RichText::new(kind.label()).monospace().size(10.0)).clicked() {
                        let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                        let color = INDICATOR_COLORS[panes[ap].indicators.len() % INDICATOR_COLORS.len()];
                        panes[ap].indicators.push(Indicator::new(id, kind, if kind == IndicatorType::RSI { 14 } else { 20 }, color));
                        panes[ap].indicator_bar_count = 0;
                        panes[ap].editing_indicator = Some(id);
                        ui.close_menu();
                    }
                }
                ui.separator();
                section_label(ui, "OSCILLATORS", t.accent);
                for &kind in IndicatorType::oscillators() {
                    let default_period = match kind { IndicatorType::RSI => 14, IndicatorType::MACD => 12, IndicatorType::Stochastic => 14, _ => 20 };
                    if ui.button(egui::RichText::new(kind.label()).monospace().size(10.0)).clicked() {
                        let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                        let color = INDICATOR_COLORS[panes[ap].indicators.len() % INDICATOR_COLORS.len()];
                        panes[ap].indicators.push(Indicator::new(id, kind, default_period, color));
                        panes[ap].indicator_bar_count = 0;
                        panes[ap].editing_indicator = Some(id);
                        ui.close_menu();
                    }
                }
            });

            ui.add(egui::Separator::default().spacing(4.0));

            }); // end scrollable middle

            // ── Fixed right: panels + window controls ──
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                // Window control button helper using Phosphor icons
                let win_ctrl = |ui: &mut egui::Ui, icon: &str, danger: bool| -> bool {
                    let fg = t.dim;
                    let resp = ui.add(
                        egui::Button::new(egui::RichText::new(icon).size(12.0).color(fg))
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(34.0, 28.0))
                            .corner_radius(0.0)
                    );
                    if resp.hovered() {
                        let bg = if danger { rgb(224, 85, 96) } else { t.toolbar_border };
                        let hover_fg = if danger { egui::Color32::WHITE } else { egui::Color32::from_rgb(200, 200, 210) };
                        ui.painter().rect_filled(resp.rect, 0.0, bg);
                        ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(12.0), hover_fg);
                    }
                    resp.clicked()
                };

                // Close
                if win_ctrl(ui, Icon::X, true) {
                    save_state(panes, *layout);
                    watchlist.persist();
                    CLOSE_REQUESTED.with(|f| f.set(true));
                }
                // Maximize
                if win_ctrl(ui, Icon::SQUARE, false) {
                    if let Some(w) = &win_ref { let m = w.is_maximized(); w.set_maximized(!m); }
                }
                // Minimize
                if win_ctrl(ui, Icon::MINUS, false) {
                    if let Some(w) = &win_ref { w.set_minimized(true); }
                }

                // Separator between window controls and panel toggles
                ui.add(egui::Separator::default().spacing(4.0));

                // Panel toggle buttons (right-to-left, so ordered right→left)
                ui.spacing_mut().item_spacing.x = 4.0;

                // Connection status
                {
                    let conn_resp = tb_btn(ui, Icon::SPARKLE, *conn_panel_open, t);
                    // Green dot overlay
                    let dot_color = rgb(46, 204, 113);
                    ui.painter().circle_filled(egui::pos2(conn_resp.rect.right() - 3.0, conn_resp.rect.top() + 5.0), 2.5, dot_color);
                    if conn_resp.clicked() { *conn_panel_open = !*conn_panel_open; }
                }

                // Orders book panel
                if tb_btn(ui, Icon::ARTICLE, watchlist.orders_panel_open, t).clicked() {
                    watchlist.orders_panel_open = !watchlist.orders_panel_open;
                }

                // Order entry toggle
                if tb_btn(ui, Icon::CURRENCY_DOLLAR, watchlist.order_entry_open, t).clicked() {
                    watchlist.order_entry_open = !watchlist.order_entry_open;
                }

                // Account strip toggle
                if tb_btn(ui, Icon::PULSE, watchlist.account_strip_open, t).clicked() {
                    watchlist.account_strip_open = !watchlist.account_strip_open;
                }

                // Watchlist toggle
                if tb_btn(ui, Icon::LIST, watchlist.open, t).clicked() { watchlist.open = !watchlist.open; }

                ui.add(egui::Separator::default().spacing(4.0));

                // Keyboard shortcuts help
                if tb_btn(ui, Icon::QUESTION, watchlist.shortcuts_open, t).clicked() {
                    watchlist.shortcuts_open = !watchlist.shortcuts_open;
                }

                // New window
                if tb_btn(ui, &format!("{} Window", Icon::PLUS), false, t).clicked() {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let sym = panes[ap].symbol.clone();
                    let tf = panes[ap].timeframe.clone();
                    let initial = super::ChartCommand::LoadBars {
                        symbol: sym.clone(), timeframe: tf.clone(), bars: vec![], timestamps: vec![],
                    };
                    {
                        let global = crate::NATIVE_CHART_TXS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
                        global.lock().unwrap().push(tx);
                    }
                    open_window(rx, initial, None);
                    fetch_bars_background(sym, tf);
                }

                ui.add(egui::Separator::default().spacing(4.0));
            });
        });
    });

    // ── Account summary strip (below toolbar) ──
    if watchlist.account_strip_open {
        let account_data = read_account_data();
        egui::TopBottomPanel::top("account_strip")
            .exact_height(32.0)
            .frame(egui::Frame::NONE.fill(t.toolbar_bg)
                .inner_margin(egui::Margin { left: 0, right: 0, top: 4, bottom: 4 })
                .stroke(egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 60))))
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                ui.horizontal(|ui| {
                    // Calculate total width of content to center it
                    let avail = ui.available_width();
                    ui.spacing_mut().item_spacing.x = 16.0;
                    if let Some((acct, _positions, _orders)) = &account_data {
                        if acct.connected {
                            // Estimate content width and add left padding to center
                            let content_w = 680.0; // approximate
                            let pad = ((avail - content_w) / 2.0).max(0.0);
                            ui.add_space(pad);

                            // NAV
                            ui.label(egui::RichText::new("NAV").monospace().size(11.0).color(t.dim.gamma_multiply(0.5)));
                            ui.label(egui::RichText::new(format!("${:.0}", acct.nav)).monospace().size(13.0).strong()
                                .color(egui::Color32::from_rgb(220, 220, 230)));

                            ui.add(egui::Separator::default().spacing(8.0));

                            // Buying Power
                            ui.label(egui::RichText::new("BP").monospace().size(11.0).color(t.dim.gamma_multiply(0.5)));
                            ui.label(egui::RichText::new(format!("${:.0}", acct.buying_power)).monospace().size(13.0)
                                .color(egui::Color32::from_rgb(200, 200, 210)));

                            ui.add(egui::Separator::default().spacing(8.0));

                            // Daily P&L
                            let daily_color = if acct.daily_pnl >= 0.0 { t.bull } else { t.bear };
                            ui.label(egui::RichText::new("Day P&L").monospace().size(11.0).color(t.dim.gamma_multiply(0.5)));
                            ui.label(egui::RichText::new(format!("{:+.0}", acct.daily_pnl)).monospace().size(13.0).strong()
                                .color(daily_color));

                            ui.add(egui::Separator::default().spacing(8.0));

                            // Unrealized P&L
                            let unr_color = if acct.unrealized_pnl >= 0.0 { t.bull } else { t.bear };
                            ui.label(egui::RichText::new("Unr P&L").monospace().size(11.0).color(t.dim.gamma_multiply(0.5)));
                            ui.label(egui::RichText::new(format!("{:+.0}", acct.unrealized_pnl)).monospace().size(13.0)
                                .color(unr_color));

                            ui.add(egui::Separator::default().spacing(8.0));

                            // Margin
                            ui.label(egui::RichText::new("Margin").monospace().size(11.0).color(t.dim.gamma_multiply(0.5)));
                            ui.label(egui::RichText::new(format!("${:.0}", acct.initial_margin)).monospace().size(13.0)
                                .color(t.dim));

                            ui.add(egui::Separator::default().spacing(8.0));

                            // Excess Liquidity
                            ui.label(egui::RichText::new("Excess").monospace().size(11.0).color(t.dim.gamma_multiply(0.5)));
                            ui.label(egui::RichText::new(format!("${:.0}", acct.excess_liquidity)).monospace().size(13.0)
                                .color(t.dim));

                            ui.add(egui::Separator::default().spacing(8.0));

                            // Realized P&L
                            let rpnl_color = if acct.realized_pnl >= 0.0 { t.bull } else { t.bear };
                            ui.label(egui::RichText::new("Real P&L").monospace().size(11.0).color(t.dim.gamma_multiply(0.5)));
                            ui.label(egui::RichText::new(format!("{:+.0}", acct.realized_pnl)).monospace().size(13.0).strong()
                                .color(rpnl_color));
                        } else {
                            // Not connected
                            ui.label(egui::RichText::new("IB Disconnected").monospace().size(10.0).color(t.dim.gamma_multiply(0.5)));
                            ui.label(egui::RichText::new(format!("connecting to {}...", APEXIB_URL)).monospace().size(9.0).color(t.dim.gamma_multiply(0.3)));
                        }
                    } else {
                        ui.label(egui::RichText::new("Loading account...").monospace().size(11.0).color(t.dim.gamma_multiply(0.4)));
                    }
                });
                });
            });
    }

    // ── Drag to move window ──
    // If a click/drag starts in the toolbar zone (y < 36) and egui didn't use the pointer
    // (no button was clicked), initiate window drag.
    if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
        if pos.y < 36.0 {
            let pointer_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
            let egui_using = ctx.is_using_pointer();
            if pointer_pressed && !egui_using {
                if let Some(w) = &win_ref { let _ = w.drag_window(); }
            }
            // Double-click to maximize
            let double_clicked = ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
            if double_clicked && !egui_using {
                if let Some(w) = &win_ref { let m = w.is_maximized(); w.set_maximized(!m); }
            }
        }
    }

    // ── Keyboard shortcuts help panel ──────────────────────────────────────
    if watchlist.shortcuts_open {
        dialog_window_themed(ctx, "shortcuts_help", egui::pos2(ctx.screen_rect().center().x - 150.0, 50.0), 300.0, t.toolbar_bg, t.toolbar_border, None)
            .show(ctx, |ui| {
                if dialog_header(ui, "KEYBOARD SHORTCUTS", t.dim) { watchlist.shortcuts_open = false; }
                ui.add_space(8.0);
                let m = 10.0;
                let shortcut_row = |ui: &mut egui::Ui, key: &str, desc: &str| {
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.allocate_ui(egui::vec2(90.0, 16.0), |ui| {
                            ui.label(egui::RichText::new(key).monospace().size(9.0).strong().color(egui::Color32::from_rgb(200,200,210)));
                        });
                        ui.label(egui::RichText::new(desc).monospace().size(9.0).color(t.dim.gamma_multiply(0.7)));
                    });
                };
                dialog_section(ui, "NAVIGATION", m, t.accent);
                shortcut_row(ui, "Scroll", "Zoom in/out");
                shortcut_row(ui, "Drag", "Pan chart");
                shortcut_row(ui, "Drag Y-axis", "Vertical zoom");
                shortcut_row(ui, "Drag X-axis", "Horizontal zoom");
                shortcut_row(ui, "Dbl-click Y", "Reset Y zoom");
                ui.add_space(6.0);
                dialog_section(ui, "DRAWING", m, t.accent);
                shortcut_row(ui, "Middle-click", "Cycle drawing tools");
                shortcut_row(ui, "Escape", "Cancel / deselect");
                shortcut_row(ui, "Delete", "Delete selected drawing");
                shortcut_row(ui, "Shift+Drag", "Measure tool");
                shortcut_row(ui, "Dbl-click line", "Edit indicator/order");
                ui.add_space(6.0);
                dialog_section(ui, "ORDERS", m, t.accent);
                shortcut_row(ui, "Right-click", "Place order at price");
                shortcut_row(ui, "Drag order", "Adjust order price");
                shortcut_row(ui, "Dbl-click order", "Edit order details");
                ui.add_space(8.0);
            });
    }

    // ── Trendline filter dropdown ────────────────────────────────────────────
    if watchlist.trendline_filter_open {
        dialog_window_themed(ctx, "trendline_filter", egui::pos2(300.0, 40.0), 210.0, t.toolbar_bg, t.toolbar_border, None)
            .show(ctx, |ui| {
                if dialog_header(ui, "DRAWING FILTERS", t.dim) { watchlist.trendline_filter_open = false; }
                ui.add_space(8.0);
                let m = 10.0;
                let chart = &mut panes[ap];

                // Per-type visibility toggles
                dialog_section(ui, "BY TYPE", m, t.dim.gamma_multiply(0.5));
                let types = [("trendline", "Trendlines"), ("hline", "H-Lines"), ("hzone", "Zones"), ("barmarker", "Markers")];
                for (dtype, label) in &types {
                    let count = chart.drawings.iter().filter(|d| {
                        match (dtype, &d.kind) {
                            (&"trendline", DrawingKind::TrendLine{..}) => true,
                            (&"hline", DrawingKind::HLine{..}) => true,
                            (&"hzone", DrawingKind::HZone{..}) => true,
                            (&"barmarker", DrawingKind::BarMarker{..}) => true,
                            _ => false,
                        }
                    }).count();
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new(format!("{} ({})", label, count)).monospace().size(10.0).color(egui::Color32::from_rgb(200,200,210)));
                    });
                }

                ui.add_space(8.0);
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, 50));
                ui.add_space(8.0);

                // Visibility toggles
                dialog_section(ui, "VISIBILITY", m, t.dim.gamma_multiply(0.5));
                let vis_btn = |ui: &mut egui::Ui, hidden: bool, label: &str, count: usize| -> bool {
                    let icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                    let fg = if hidden { t.dim.gamma_multiply(0.4) } else { t.dim };
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.add(egui::Button::new(egui::RichText::new(format!("{} {} ({})", icon, label, count))
                            .monospace().size(9.0).color(fg)).frame(false))
                            .clicked()
                    }).inner
                };
                let sig_count = chart.signal_drawings.len();
                if vis_btn(ui, chart.hide_signal_drawings, "Signals", sig_count) {
                    chart.hide_signal_drawings = !chart.hide_signal_drawings;
                }
                if vis_btn(ui, chart.hide_all_drawings, "All Drawings", chart.drawings.len()) {
                    chart.hide_all_drawings = !chart.hide_all_drawings;
                }

                // Groups
                if !chart.groups.is_empty() {
                    ui.add_space(8.0);
                    dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, 50));
                    ui.add_space(8.0);
                    dialog_section(ui, "GROUPS", m, t.dim.gamma_multiply(0.5));
                    for g in chart.groups.clone() {
                        let hidden = chart.hidden_groups.contains(&g.id);
                        let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                        if vis_btn(ui, hidden, &g.name, count) {
                            if hidden { chart.hidden_groups.retain(|x| x != &g.id); }
                            else { chart.hidden_groups.push(g.id.clone()); }
                        }
                    }
                }
                ui.add_space(8.0);
            });
    }

    // Symbol picker popup — render for any pane that has it open
    span_begin("symbol_picker");
    for picker_pane_idx in 0..panes.len() {
    let chart = &mut panes[picker_pane_idx];
    if chart.picker_open {
        let mut close_picker = false;
        let mut new_symbol: Option<(String, String)> = None; // (symbol, name)

        // Check for background search results
        if let Some(rx) = &chart.picker_rx {
            if let Ok(results) = rx.try_recv() {
                chart.picker_results = results;
                chart.picker_searching = false;
            }
        }

        // Launch search when query changes
        if chart.picker_query != chart.picker_last_query {
            chart.picker_last_query = chart.picker_query.clone();
            let q = chart.picker_query.trim().to_string();

            if q.is_empty() {
                // Empty query: show recents + popular from static list
                chart.picker_results.clear();
                chart.picker_searching = false;
                chart.picker_rx = None;
            } else {
                // Immediate: show static matches while Yahoo search runs
                let static_results: Vec<(String, String, String)> = ui_kit::symbols::search_symbols(&q, 10)
                    .iter().map(|s| (s.symbol.to_string(), s.name.to_string(), String::new())).collect();
                chart.picker_results = static_results;

                // Fire background search: ApexIB first, Yahoo fallback
                chart.picker_searching = true;
                let (tx, rx) = mpsc::channel();
                chart.picker_rx = Some(rx);
                let query = q.clone();
                std::thread::spawn(move || {
                    let client = reqwest::blocking::Client::builder()
                        .user_agent("Mozilla/5.0")
                        .timeout(std::time::Duration::from_secs(3))
                        .build().unwrap_or_else(|_| reqwest::blocking::Client::new());
                    let mut results: Vec<(String, String, String)> = Vec::new();

                    // Try ApexIB search first
                    let apexib_url = format!("{}/search/{}", APEXIB_URL, query);
                    if let Ok(resp) = client.get(&apexib_url).send() {
                        if resp.status().is_success() {
                            if let Ok(json) = resp.json::<serde_json::Value>() {
                                if let Some(arr) = json.as_array() {
                                    for item in arr.iter().take(MAX_SEARCH_RESULTS) {
                                        if let Some(sym) = item.get("symbol").and_then(|v| v.as_str()) {
                                            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                            let sec_type = item.get("secType").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                            results.push((sym.to_string(), name, sec_type));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fallback: Yahoo Finance search API
                    if results.is_empty() {
                        let url = format!(
                            "https://query2.finance.yahoo.com/v1/finance/search?q={}&quotesCount=15&newsCount=0",
                            query
                        );
                        if let Ok(resp) = client.get(&url).send() {
                            if let Ok(json) = resp.json::<serde_json::Value>() {
                                if let Some(quotes) = json.get("quotes").and_then(|q| q.as_array()) {
                                    for q in quotes.iter().take(MAX_SEARCH_RESULTS) {
                                        if let Some(sym) = q.get("symbol").and_then(|s| s.as_str()) {
                                            let name = q.get("shortname").or_else(|| q.get("longname"))
                                                .and_then(|n| n.as_str()).unwrap_or("").to_string();
                                            let exchange = q.get("exchDisp").and_then(|e| e.as_str()).unwrap_or("").to_string();
                                            let type_disp = q.get("typeDisp").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                            let tag = if !exchange.is_empty() && !type_disp.is_empty() {
                                                format!("{} · {}", exchange, type_disp)
                                            } else if !exchange.is_empty() { exchange }
                                            else { type_disp };
                                            results.push((sym.to_string(), name, tag));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // If both returned nothing, use static
                    if results.is_empty() {
                        results = ui_kit::symbols::search_symbols(&query, MAX_SEARCH_RESULTS)
                            .iter().map(|s| (s.symbol.to_string(), s.name.to_string(), String::new())).collect();
                    }
                    let _  = tx.send(results);
                });
            }
        }

        egui::Window::new(format!("picker_{}", picker_pane_idx))
            .fixed_pos(chart.picker_pos)
            .fixed_size(egui::vec2(340.0, 420.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style()).fill(egui::Color32::from_rgb(28,28,32)))
            .show(ctx, |ui| {
                let input = ui.add(
                    egui::TextEdit::singleline(&mut chart.picker_query)
                        .hint_text("Search any stock, ETF, index...")
                        .desired_width(320.0)
                        .font(egui::FontId::monospace(13.0))
                );
                input.request_focus();

                if chart.picker_searching {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new("Searching...").small().color(t.dim));
                    });
                }

                ui.separator();

                egui::ScrollArea::vertical().max_height(370.0).show(ui, |ui| {
                    let show_recents = chart.picker_query.trim().is_empty();

                    if show_recents && !chart.recent_symbols.is_empty() {
                        ui.label(egui::RichText::new("RECENT").small().strong().color(t.dim));
                        ui.add_space(2.0);
                        for (sym, name) in chart.recent_symbols.clone() {
                            let is_current = sym == chart.symbol;
                            let resp = ui.horizontal(|ui| {
                                let sym_text = egui::RichText::new(&sym).strong().monospace()
                                    .color(if is_current { t.bull } else { egui::Color32::from_rgb(220,220,230) });
                                let r = ui.add(egui::Button::new(sym_text).frame(false).min_size(egui::vec2(65.0, 22.0)));
                                ui.label(egui::RichText::new(&name).small().color(t.dim));
                                r
                            }).inner;
                            if resp.clicked() {
                                new_symbol = Some((sym.clone(), name.clone()));
                                close_picker = true;
                            }
                        }
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("POPULAR").small().strong().color(t.dim));
                        ui.add_space(2.0);
                        // Show popular symbols from static catalog
                        for s in ui_kit::symbols::search_symbols("", 20) {
                            if chart.recent_symbols.iter().any(|(r, _)| r == s.symbol) { continue; }
                            let is_current = s.symbol == chart.symbol;
                            let resp = ui.horizontal(|ui| {
                                let sym_text = egui::RichText::new(s.symbol).strong().monospace()
                                    .color(if is_current { t.bull } else { egui::Color32::from_rgb(200,200,210) });
                                let r = ui.add(egui::Button::new(sym_text).frame(false).min_size(egui::vec2(65.0, 22.0)));
                                ui.label(egui::RichText::new(s.name).small().color(t.dim));
                                r
                            }).inner;
                            if resp.clicked() {
                                new_symbol = Some((s.symbol.to_string(), s.name.to_string()));
                                close_picker = true;
                            }
                        }
                    } else {
                        // Search results
                        for (sym, name, tag) in &chart.picker_results {
                            let is_current = sym == &chart.symbol;
                            let resp = ui.horizontal(|ui| {
                                let sym_text = egui::RichText::new(sym).strong().monospace()
                                    .color(if is_current { t.bull } else { egui::Color32::from_rgb(220,220,230) });
                                let r = ui.add(egui::Button::new(sym_text).frame(false).min_size(egui::vec2(65.0, 22.0)));
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(name).small().color(egui::Color32::from_rgb(180,180,190)));
                                    if !tag.is_empty() {
                                        ui.label(egui::RichText::new(tag).small().color(egui::Color32::from_rgb(100,100,120)));
                                    }
                                });
                                r
                            }).inner;
                            if resp.clicked() {
                                new_symbol = Some((sym.clone(), name.clone()));
                                close_picker = true;
                            }
                        }
                        if chart.picker_results.is_empty() && !chart.picker_searching && !chart.picker_query.trim().is_empty() {
                            ui.label(egui::RichText::new("No results").color(t.dim));
                        }
                    }
                });

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close_picker = true; }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some((sym, name, _)) = chart.picker_results.first() {
                        new_symbol = Some((sym.clone(), name.clone()));
                        close_picker = true;
                    }
                }
            });

        if close_picker { chart.picker_open = false; }

        if let Some((sym, name)) = new_symbol {
            // Add to recents (move to front if already there)
            chart.recent_symbols.retain(|(s, _)| s != &sym);
            chart.recent_symbols.insert(0, (sym.clone(), name));
            if chart.recent_symbols.len() > MAX_RECENT_SYMBOLS { chart.recent_symbols.truncate(MAX_RECENT_SYMBOLS); }
            chart.pending_symbol_change = Some(sym);
        }
    }
    } // end for picker_pane_idx
    span_end();

    // Style toolbar — active pane
    let chart = &mut panes[ap];

    // Style toolbar — compact, centered at top of chart
    if !chart.selected_ids.is_empty() {
        let screen = ctx.screen_rect();
        let ids = chart.selected_ids.clone();
        // Extract current style values (avoids borrow conflict with mutable drawing access)
        let (cur_color, cur_ls, cur_th, cur_op, cur_group) = chart.drawings.iter().find(|d| ids.contains(&d.id))
            .map(|d| (d.color.clone(), d.line_style, d.thickness, d.opacity, d.group_id.clone()))
            .unwrap_or(("#4a9eff".into(), LineStyle::Solid, 1.5, 1.0, "default".into()));
        let cur_group_name = chart.groups.iter().find(|g| g.id == cur_group)
            .map(|g| g.name.clone()).unwrap_or("default".into());
        let groups_snapshot: Vec<(String, String)> = {
            let mut gs = vec![("default".into(), "default".into())];
            for g in &chart.groups {
                if g.id != "default" { gs.push((g.id.clone(), g.name.clone())); }
            }
            gs
        };

        let bar_w = 580.0;
        egui::Window::new("style_bar")
            .fixed_pos(egui::pos2(screen.center().x - bar_w / 2.0, 32.0))
            .fixed_size(egui::vec2(bar_w, 24.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style()).fill(egui::Color32::from_rgb(35,35,40)).inner_margin(4.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;

                    // Color circles
                    for &c in PRESET_COLORS {
                        let col = hex_to_color(c, 1.0);
                        let is_cur = cur_color.as_str() == c;
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                        ui.painter().circle_filled(r.center(), if is_cur { 7.0 } else { 5.5 }, col);
                        if is_cur { ui.painter().circle_stroke(r.center(), 8.0, egui::Stroke::new(1.5, egui::Color32::WHITE)); }
                        if resp.clicked() { for d in &mut chart.drawings { if ids.contains(&d.id) { d.color = c.to_string(); } } }
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Line style dropdown
                    let ls_label = match cur_ls { LineStyle::Solid => "____", LineStyle::Dashed => "- - -", LineStyle::Dotted => ". . ." };
                    egui::ComboBox::from_id_salt("ls").selected_text(ls_label).width(65.0).show_ui(ui, |ui| {
                        if ui.selectable_label(cur_ls == LineStyle::Solid, "_____ Solid").clicked() { for d in &mut chart.drawings { if ids.contains(&d.id) { d.line_style = LineStyle::Solid; } } }
                        if ui.selectable_label(cur_ls == LineStyle::Dashed, "- - - -  Dash").clicked() { for d in &mut chart.drawings { if ids.contains(&d.id) { d.line_style = LineStyle::Dashed; } } }
                        if ui.selectable_label(cur_ls == LineStyle::Dotted, ". . . . .  Dot").clicked() { for d in &mut chart.drawings { if ids.contains(&d.id) { d.line_style = LineStyle::Dotted; } } }
                    });

                    // Width dropdown
                    egui::ComboBox::from_id_salt("th").selected_text(format!("{:.1}px", cur_th)).width(52.0).show_ui(ui, |ui| {
                        for &th in &[0.5_f32, 1.0, 1.5, 2.5] {
                            if ui.selectable_label((cur_th - th).abs() < 0.1, format!("{:.1}px", th)).clicked() {
                                for d in &mut chart.drawings { if ids.contains(&d.id) { d.thickness = th; } }
                            }
                        }
                    });

                    // Opacity dropdown
                    egui::ComboBox::from_id_salt("op").selected_text(format!("{}%", (cur_op * 100.0) as u32)).width(48.0).show_ui(ui, |ui| {
                        for &op in &[1.0_f32, 0.75, 0.5, 0.25] {
                            if ui.selectable_label((cur_op - op).abs() < 0.01, format!("{}%", (op * 100.0) as u32)).clicked() {
                                for d in &mut chart.drawings { if ids.contains(&d.id) { d.opacity = op; } }
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Group dropdown
                    egui::ComboBox::from_id_salt("grp").selected_text(
                        egui::RichText::new(format!("{} {}", Icon::FOLDER, cur_group_name)).monospace().size(10.0)
                    ).width(80.0).show_ui(ui, |ui| {
                        // Existing groups
                        for (gid, gname) in &groups_snapshot {
                            let is_cur = *gid == cur_group;
                            if ui.selectable_label(is_cur, egui::RichText::new(gname).monospace().size(10.0)).clicked() && !is_cur {
                                let sym = chart.symbol.clone();
                                let tf = chart.timeframe.clone();
                                for d in &mut chart.drawings {
                                    if ids.contains(&d.id) {
                                        d.group_id = gid.clone();
                                        crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                                    }
                                }
                            }
                        }
                        ui.separator();
                        // + New Group
                        if ui.selectable_label(false, egui::RichText::new(format!("{} New Group...", Icon::PLUS)).monospace().size(10.0).color(t.accent)).clicked() {
                            chart.group_manager_open = true;
                        }
                    });

                    // Apply style to entire group
                    if cur_group != "default" {
                        let group_count = chart.drawings.iter().filter(|d| d.group_id == cur_group).count();
                        if group_count > 1 {
                            let tip = format!("Apply style to all {} drawings in {}", group_count, cur_group_name);
                            let resp = ui.add(egui::Button::new(egui::RichText::new(Icon::PALETTE).size(14.0).color(t.accent))
                                .frame(false).min_size(egui::vec2(20.0, 20.0)));
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                egui::show_tooltip(ui.ctx(), ui.layer_id(), egui::Id::new("group_style_tip"), |ui| {
                                    ui.label(egui::RichText::new(tip).monospace().size(9.0));
                                });
                            }
                            if resp.clicked() {
                                let sym = chart.symbol.clone();
                                let tf = chart.timeframe.clone();
                                let target_group = cur_group.clone();
                                for d in &mut chart.drawings {
                                    if d.group_id == target_group {
                                        d.color = cur_color.clone();
                                        d.line_style = cur_ls;
                                        d.thickness = cur_th;
                                        d.opacity = cur_op;
                                        crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                                    }
                                }
                            }
                        }
                    }

                    ui.add_space(4.0);
                    // Delete icon
                    if Icon::button_colored(ui, Icon::TRASH, egui::Color32::from_rgb(224,85,96), "Delete").clicked() {
                        chart.drawings.retain(|d| !ids.contains(&d.id));
                        chart.selected_ids.clear(); chart.selected_id = None;
                    }
                });
            });
    }

    // ── Indicator editor popup ─────────────────────────────────────────────────
    let t = &THEMES[panes[ap].theme_idx];
    if let Some(edit_id) = panes[ap].editing_indicator {
        let mut close_editor = false;
        let mut delete_id: Option<u32> = None;
        let mut needs_recompute = false;
        let mut needs_source_fetch: Option<(String, String, u32)> = None;
        let pane_symbol = panes[ap].symbol.clone(); // clone to avoid borrow conflict

        {
        dialog_window_themed(ctx, &format!("ind_editor_{}", edit_id), egui::pos2(200.0, 80.0), 280.0, t.toolbar_bg, t.toolbar_border, None)
            .show(ctx, |ui| {
                if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.id == edit_id) {
                    if dialog_header(ui, &ind.display_name(), t.dim) { close_editor = true; }

                    // ── Body ──
                    ui.add_space(8.0);
                    let m = 10.0; // body margin

                    // Type selector — full-width segmented control
                    dialog_section(ui, "TYPE", m, t.dim.gamma_multiply(0.5));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (i, &kind) in IndicatorType::all().iter().enumerate() {
                            let selected = ind.kind == kind;
                            let fg = if selected { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                            let bg = if selected { color_alpha(t.accent, 60) } else { color_alpha(t.toolbar_border, 25) };
                            let rounding = if i == 0 {
                                egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }
                            } else if i == IndicatorType::all().len() - 1 {
                                egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 }
                            } else {
                                egui::CornerRadius::ZERO
                            };
                            if ui.add(egui::Button::new(egui::RichText::new(kind.label()).monospace().size(9.0).color(fg))
                                .fill(bg).corner_radius(rounding).min_size(egui::vec2(0.0, 22.0))
                                .stroke(egui::Stroke::new(0.5, if selected { color_alpha(t.accent, 120) } else { color_alpha(t.toolbar_border, 50) })))
                                .clicked() && !selected {
                                ind.kind = kind;
                                needs_recompute = true;
                            }
                        }
                    });

                    ui.add_space(8.0);

                    // Period — drag value + quick presets
                    dialog_section(ui, "PERIOD", m, t.dim.gamma_multiply(0.5));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        let mut period = ind.period as i32;
                        if ui.add(egui::DragValue::new(&mut period).range(1..=500).speed(0.5)
                            .custom_formatter(|v, _| format!("{}", v as i32))).changed() {
                            ind.period = (period as usize).max(1);
                            needs_recompute = true;
                        }
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for &p in &[9, 12, 20, 26, 50, 100, 200] {
                            let sel = ind.period == p;
                            let fg = if sel { t.accent } else { t.dim.gamma_multiply(0.5) };
                            let bg = if sel { color_alpha(t.accent, 20) } else { egui::Color32::TRANSPARENT };
                            if ui.add(egui::Button::new(egui::RichText::new(format!("{}", p)).monospace().size(8.0).color(fg))
                                .fill(bg).corner_radius(2.0).min_size(egui::vec2(22.0, 18.0))
                                .stroke(if sel { egui::Stroke::new(0.5, color_alpha(t.accent, 60)) } else { egui::Stroke::NONE }))
                                .clicked() && !sel {
                                ind.period = p;
                                needs_recompute = true;
                            }
                        }
                    });

                    ui.add_space(8.0);

                    // Source interval — segmented control
                    dialog_section(ui, "SOURCE", m, t.dim.gamma_multiply(0.5));
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let tfs = INDICATOR_TIMEFRAMES;
                        for (i, &tf) in tfs.iter().enumerate() {
                            let label = if tf.is_empty() { "Chart" } else { tf };
                            let sel = ind.source_tf == tf;
                            let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                            let bg = if sel { color_alpha(t.accent, 60) } else { color_alpha(t.toolbar_border, 25) };
                            let rounding = if i == 0 {
                                egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }
                            } else if i == tfs.len() - 1 {
                                egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 }
                            } else {
                                egui::CornerRadius::ZERO
                            };
                            if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(9.0).color(fg))
                                .fill(bg).corner_radius(rounding).min_size(egui::vec2(0.0, 22.0))
                                .stroke(egui::Stroke::new(0.5, if sel { color_alpha(t.accent, 120) } else { color_alpha(t.toolbar_border, 50) })))
                                .clicked() && !sel {
                                ind.source_tf = tf.to_string();
                                ind.source_loaded = tf.is_empty();
                                ind.source_bars.clear();
                                ind.source_timestamps.clear();
                                needs_recompute = true;
                                if !tf.is_empty() {
                                    needs_source_fetch = Some((pane_symbol.clone(), tf.to_string(), ind.id));
                                }
                            }
                        }
                    });

                    ui.add_space(10.0);
                    dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, 50));
                    ui.add_space(10.0);

                    // APPEARANCE section
                    dialog_section(ui, "APPEARANCE", m, t.dim.gamma_multiply(0.5));
                    ui.add_space(3.0);

                    // Color picker
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Color").monospace().size(9.0).color(t.dim));
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for &c in INDICATOR_COLORS {
                            let color = hex_to_color(c, 1.0);
                            let is_cur = ind.color == c;
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                            if is_cur {
                                ui.painter().rect_filled(r, 3.0, color_alpha(color, 30));
                                ui.painter().rect_stroke(r, 3.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Outside);
                            }
                            ui.painter().circle_filled(r.center(), if is_cur { 5.5 } else { 4.5 }, color);
                            if resp.clicked() { ind.color = c.to_string(); }
                        }
                    });
                    ui.add_space(4.0);

                    // Width — segmented
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Width").monospace().size(9.0).color(t.dim));
                        ui.add_space(6.0);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let widths = [0.5_f32, 1.0, 1.5, 2.0, 3.0];
                        for (i, &th) in widths.iter().enumerate() {
                            let sel = (ind.thickness - th).abs() < 0.1;
                            let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                            let bg = if sel { color_alpha(t.accent, 60) } else { color_alpha(t.toolbar_border, 25) };
                            let rounding = if i == 0 {
                                egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }
                            } else if i == widths.len() - 1 {
                                egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 }
                            } else {
                                egui::CornerRadius::ZERO
                            };
                            if ui.add(egui::Button::new(egui::RichText::new(format!("{:.1}", th)).monospace().size(9.0).color(fg))
                                .fill(bg).corner_radius(rounding).min_size(egui::vec2(30.0, 20.0))
                                .stroke(egui::Stroke::new(0.5, if sel { color_alpha(t.accent, 120) } else { color_alpha(t.toolbar_border, 50) })))
                                .clicked() { ind.thickness = th; }
                        }
                    });
                    ui.add_space(4.0);

                    // Style — segmented
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new("Style").monospace().size(9.0).color(t.dim));
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let styles = [(LineStyle::Solid, "Solid"), (LineStyle::Dashed, "Dash"), (LineStyle::Dotted, "Dot")];
                        for (i, (ls, label)) in styles.iter().enumerate() {
                            let sel = ind.line_style == *ls;
                            let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                            let bg = if sel { color_alpha(t.accent, 60) } else { color_alpha(t.toolbar_border, 25) };
                            let rounding = if i == 0 {
                                egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }
                            } else if i == styles.len() - 1 {
                                egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 }
                            } else {
                                egui::CornerRadius::ZERO
                            };
                            if ui.add(egui::Button::new(egui::RichText::new(*label).monospace().size(9.0).color(fg))
                                .fill(bg).corner_radius(rounding).min_size(egui::vec2(42.0, 20.0))
                                .stroke(egui::Stroke::new(0.5, if sel { color_alpha(t.accent, 120) } else { color_alpha(t.toolbar_border, 50) })))
                                .clicked() { ind.line_style = *ls; }
                        }
                    });

                    ui.add_space(10.0);
                    dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, 50));
                    ui.add_space(8.0);

                    // ── Footer actions ──
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        // Visibility toggle
                        let vis_icon = if ind.visible { Icon::EYE } else { Icon::EYE_SLASH };
                        let vis_fg = if ind.visible { t.dim } else { t.dim.gamma_multiply(0.4) };
                        let vis_bg = if ind.visible { color_alpha(t.toolbar_border, 20) } else { egui::Color32::TRANSPARENT };
                        if ui.add(egui::Button::new(egui::RichText::new(format!("{} {}", vis_icon, if ind.visible { "Visible" } else { "Hidden" }))
                            .monospace().size(9.0).color(vis_fg))
                            .fill(vis_bg).corner_radius(3.0).min_size(egui::vec2(0.0, 22.0))).clicked() {
                            ind.visible = !ind.visible;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(m);
                            let del_color = egui::Color32::from_rgb(224, 85, 96);
                            if ui.add(egui::Button::new(egui::RichText::new(format!("{} Delete", Icon::TRASH))
                                .monospace().size(9.0).color(del_color))
                                .fill(color_alpha(del_color, 15)).corner_radius(3.0)
                                .stroke(egui::Stroke::new(0.5, color_alpha(del_color, 60)))
                                .min_size(egui::vec2(0.0, 22.0))).clicked() {
                                delete_id = Some(edit_id);
                                close_editor = true;
                            }
                        });
                    });
                    ui.add_space(8.0);
                } else {
                    close_editor = true;
                }
            });
        }

        if close_editor { panes[ap].editing_indicator = None; }
        if let Some(id) = delete_id { panes[ap].indicators.retain(|i| i.id != id); }
        if needs_recompute { panes[ap].indicator_bar_count = 0; }
        if let Some((sym, tf, ind_id)) = needs_source_fetch {
            fetch_indicator_source(sym, tf, ind_id);
        }
    }

    // ── Group manager popup ────────────────────────────────────────────────────
    if panes[ap].group_manager_open {
        let mut close_gm = false;
        dialog_window_themed(ctx, "group_manager", egui::pos2(200.0, 100.0), 250.0, t.toolbar_bg, t.toolbar_border, None)
            .show(ctx, |ui| {
                if dialog_header(ui, "NEW GROUP", t.dim) { close_gm = true; }
                ui.add_space(10.0);
                let m = 10.0;
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let resp = ui.add(egui::TextEdit::singleline(&mut panes[ap].new_group_name)
                        .hint_text("Group name...").desired_width(230.0 - m * 2.0).font(egui::FontId::monospace(12.0)));
                    resp.request_focus();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let can_create = !panes[ap].new_group_name.trim().is_empty();
                    if action_btn(ui, &format!("{} Create", Icon::PLUS), t.accent, can_create) {
                        let name = panes[ap].new_group_name.trim().to_string();
                        let id = new_uuid();
                        crate::drawing_db::save_group(&id, &name, None);
                        panes[ap].groups.push(super::DrawingGroup { id, name, color: None });
                        panes[ap].new_group_name.clear();
                        close_gm = true;
                    }
                });
                ui.add_space(8.0);
            });
        if close_gm { panes[ap].group_manager_open = false; }
    }

    // ── Connection panel popup ──────────────────────────────────────────────
    if *conn_panel_open {
        dialog_window_themed(ctx, "conn_panel", egui::pos2(ctx.screen_rect().right() - 260.0, 40.0), 240.0, t.toolbar_bg, t.toolbar_border, None)
            .show(ctx, |ui| {
                if dialog_header(ui, "CONNECTIONS", t.dim) { *conn_panel_open = false; }
                ui.add_space(8.0);
                let m = 10.0;

                dialog_section(ui, "SERVICES", m, t.dim.gamma_multiply(0.5));
                let svc_row = |ui: &mut egui::Ui, name: &str, status: &str, ok: bool, detail: &str| {
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        let dot_color = if ok { rgb(46,204,113) } else { rgb(231,76,60) };
                        ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 7.0), 3.5, dot_color);
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(name).monospace().size(10.0).strong().color(egui::Color32::from_rgb(200,200,210)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(m);
                            status_badge(ui, status, if ok { t.bull } else { t.bear });
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(m + 12.0);
                        ui.label(egui::RichText::new(detail).monospace().size(8.0).color(t.dim.gamma_multiply(0.45)));
                    });
                    ui.add_space(3.0);
                };

                let redis_ok = crate::bar_cache::get("__ping_test", "").is_none();
                let ib_ok = read_account_data().map(|(a, _, _)| a.connected).unwrap_or(false);
                svc_row(ui, "ApexIB", if ib_ok { "OK" } else { "OFF" }, ib_ok, APEXIB_URL);
                svc_row(ui, "Redis Cache", if redis_ok { "OK" } else { "OFF" }, redis_ok, "192.168.1.89:6379");
                svc_row(ui, "GPU Engine", "DX12", true, "wgpu + egui");
                svc_row(ui, "Data Feed", "OK", true, "query1.finance.yahoo.com");
                svc_row(ui, "OCOCO", "OK", true, "192.168.1.60:30300");

                ui.add_space(4.0);
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, 40));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label(egui::RichText::new("apexib:5000 \u{00B7} redis:6379 \u{00B7} ococo:30300 \u{00B7} yahoo").monospace().size(8.0).color(t.dim.gamma_multiply(0.3)));
                });
                ui.add_space(8.0);
            });
    }

    // ── Order execution toasts ───────────────────────────────────────────────
    if !toasts.is_empty() {
        let screen = ctx.screen_rect();
        for (i, (msg, _price, created, is_buy)) in toasts.iter().enumerate() {
            let age = created.elapsed().as_secs_f32();
            let alpha = ((5.0 - age) / 1.0).min(1.0).max(0.0); // fade out in last second
            if alpha <= 0.0 { continue; }
            let color = if *is_buy { t.bull } else { t.bear };
            let y_offset = screen.top() + 44.0 + i as f32 * 28.0;

            egui::Window::new(format!("toast_{}", i))
                .fixed_pos(egui::pos2(screen.center().x - 100.0, y_offset))
                .fixed_size(egui::vec2(200.0, 20.0))
                .title_bar(false)
                .frame(egui::Frame::popup(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (40.0 * alpha) as u8))
                    .inner_margin(4.0))
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(format!("{} {}", Icon::CHECK, msg)).monospace().size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (255.0 * alpha) as u8)));
                });
        }
    }

    // ── Watchlist side panel ───────────────────────────────────────────────────
    if watchlist.open {
        egui::SidePanel::right("watchlist")
            .default_width(260.0)
            .min_width(200.0)
            .max_width(400.0)
            .frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin { left: 6, right: 6, top: 6, bottom: 6 }))
            .show(ctx, |ui| {
                let mut wl_switch_to: Option<usize> = None;
                let mut wl_fetch_syms: Vec<String> = Vec::new();
                let mut wl_rename_idx: Option<usize> = None;
                let mut wl_delete_idx: Option<usize> = None;
                let mut wl_dup_idx: Option<usize> = None;

                // ── A) Tabs at the very top with X button ──
                let tab_row_resp = ui.horizontal(|ui| {
                    ui.set_min_height(22.0);
                    for (tab, label) in [(WatchlistTab::Stocks, "LIST"), (WatchlistTab::Chain, "CHAIN")] {
                        let active = watchlist.tab == tab;
                        let color = if active { t.accent } else { t.dim };
                        let tab_resp = ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(11.0).strong().color(color)).frame(false));
                        if tab_resp.clicked() {
                            watchlist.tab = tab;
                        }
                        // Draw 2px accent border under active tab
                        if active {
                            let r = tab_resp.rect;
                            ui.painter().rect_filled(
                                egui::Rect::from_min_max(egui::pos2(r.left(), r.max.y - 2.0), egui::pos2(r.right(), r.max.y)),
                                0.0, t.accent);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim)).frame(false)).clicked() {
                            watchlist.open = false;
                        }
                    });
                });
                // 1px line below tabs
                let line_y = tab_row_resp.response.rect.max.y + 1.0;
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left(), line_y), egui::pos2(ui.min_rect().right(), line_y)],
                    egui::Stroke::new(1.0, t.toolbar_border),
                );
                ui.add_space(4.0);

                let mut open_option_chart: Option<(String, f32, bool, String)> = None;

                match watchlist.tab {
                    // ── STOCKS TAB (LIST) ──────────────────────────────────────────
                    WatchlistTab::Stocks => {
                        // ── B) Watchlist selector + options toggle ──
                        ui.horizontal(|ui| {
                            ui.set_min_height(20.0);
                            // Inline rename mode
                            if watchlist.watchlist_name_editing {
                                let resp = ui.add(egui::TextEdit::singleline(&mut watchlist.watchlist_name_buf)
                                    .desired_width(ui.available_width() - 50.0)
                                    .font(egui::FontId::monospace(10.0)));
                                if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    let new_name = watchlist.watchlist_name_buf.trim().to_string();
                                    if !new_name.is_empty() {
                                        if let Some(wl) = watchlist.saved_watchlists.get_mut(watchlist.active_watchlist_idx) {
                                            wl.name = new_name;
                                        }
                                    }
                                    watchlist.watchlist_name_editing = false;
                                    watchlist.persist();
                                } else {
                                    resp.request_focus();
                                }
                            } else {
                                // Snapshot names and count for the dropdown to avoid borrow conflicts
                                let wl_names: Vec<String> = watchlist.saved_watchlists.iter().map(|w| w.name.clone()).collect();
                                let wl_count = wl_names.len();
                                let active_idx = watchlist.active_watchlist_idx;
                                let active_name = wl_names.get(active_idx).cloned().unwrap_or_else(|| "Default".into());
                                let combo_resp = egui::ComboBox::from_id_salt("wl_selector")
                                    .selected_text(egui::RichText::new(&active_name).monospace().size(10.0).color(t.accent))
                                    .width(ui.available_width() - 60.0)
                                    .show_ui(ui, |ui| {
                                        for (i, name) in wl_names.iter().enumerate() {
                                            let is_active = i == active_idx;
                                            let label_color = if is_active { t.accent } else { egui::Color32::from_rgb(200, 200, 210) };
                                            let resp = ui.selectable_label(is_active,
                                                egui::RichText::new(name).monospace().size(10.0).color(label_color));
                                            if resp.clicked() && !is_active {
                                                wl_switch_to = Some(i);
                                            }
                                            // Right-click context menu on each watchlist entry
                                            resp.context_menu(|ui| {
                                                if ui.button(egui::RichText::new("Rename").monospace().size(10.0)).clicked() {
                                                    wl_rename_idx = Some(i);
                                                    ui.close_menu();
                                                }
                                                if ui.button(egui::RichText::new("Duplicate").monospace().size(10.0)).clicked() {
                                                    wl_dup_idx = Some(i);
                                                    ui.close_menu();
                                                }
                                                if wl_count > 1 {
                                                    ui.separator();
                                                    if ui.button(egui::RichText::new("Delete").monospace().size(10.0)
                                                        .color(egui::Color32::from_rgb(224, 85, 96))).clicked() {
                                                        wl_delete_idx = Some(i);
                                                        ui.close_menu();
                                                    }
                                                }
                                            });
                                        }
                                    });
                                // Right-click the combo box header for rename/dup/delete
                                combo_resp.response.context_menu(|ui| {
                                    if ui.button(egui::RichText::new("Rename").monospace().size(10.0)).clicked() {
                                        wl_rename_idx = Some(active_idx);
                                        ui.close_menu();
                                    }
                                    if ui.button(egui::RichText::new("Duplicate").monospace().size(10.0)).clicked() {
                                        wl_dup_idx = Some(active_idx);
                                        ui.close_menu();
                                    }
                                    if wl_count > 1 {
                                        ui.separator();
                                        if ui.button(egui::RichText::new("Delete").monospace().size(10.0)
                                            .color(egui::Color32::from_rgb(224, 85, 96))).clicked() {
                                            wl_delete_idx = Some(active_idx);
                                            ui.close_menu();
                                        }
                                    }
                                });
                                // "+" button to create new watchlist
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::PLUS).size(12.0).color(t.dim)).frame(false)).clicked() {
                                    let n = watchlist.saved_watchlists.len() + 1;
                                    let syms = watchlist.create_watchlist(&format!("Watchlist {}", n));
                                    if !syms.is_empty() { wl_fetch_syms = syms; }
                                }
                            }
                            // Options toggle (circle icon) — right-aligned
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let opt_icon = if watchlist.options_visible { Icon::RADIO_BUTTON } else { Icon::DOT };
                                let opt_color = if watchlist.options_visible { t.accent } else { t.dim };
                                let opt_resp = ui.add(egui::Button::new(egui::RichText::new(opt_icon).size(11.0).color(opt_color))
                                    .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(18.0, 18.0)));
                                if opt_resp.clicked() { watchlist.options_visible = !watchlist.options_visible; }
                                if opt_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                            });
                        });
                        // Handle deferred rename
                        if let Some(idx) = wl_rename_idx {
                            if idx != watchlist.active_watchlist_idx {
                                wl_switch_to = Some(idx);
                            }
                            watchlist.watchlist_name_buf = watchlist.saved_watchlists.get(idx).map(|w| w.name.clone()).unwrap_or_default();
                            watchlist.watchlist_name_editing = true;
                        }
                        // Handle deferred duplicate
                        if let Some(dup_idx) = wl_dup_idx {
                            let syms = watchlist.duplicate_watchlist(dup_idx);
                            if !syms.is_empty() { wl_fetch_syms = syms; }
                        }
                        // Handle deferred delete
                        if let Some(del_idx) = wl_delete_idx {
                            let syms = watchlist.delete_watchlist(del_idx);
                            if !syms.is_empty() { wl_fetch_syms = syms; }
                        }
                        // Handle watchlist switch
                        if let Some(idx) = wl_switch_to {
                            let syms = watchlist.switch_to(idx);
                            if !syms.is_empty() { wl_fetch_syms = syms; }
                        }
                        // Trigger price fetches for new watchlist
                        if !wl_fetch_syms.is_empty() {
                            fetch_watchlist_prices(wl_fetch_syms);
                        }
                        ui.add_space(2.0);

                        // ── C) Search field ──
                        let search_id = egui::Id::new("wl_search_input");
                        let has_focus = ui.ctx().memory(|m| m.has_focus(search_id));
                        let search_bg = if has_focus {
                            egui::Color32::TRANSPARENT // normal background on focus (frame default)
                        } else {
                            color_alpha(t.toolbar_border, 15) // subtle background normally
                        };
                        let search_resp = egui::Frame::NONE.fill(search_bg).corner_radius(3.0).show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut watchlist.search_query)
                                    .id(search_id)
                                    .hint_text("Add symbol...").desired_width(ui.available_width()).font(egui::FontId::monospace(11.0))
                            )
                        }).inner;
                        // Refocus after adding a symbol
                        if watchlist.search_refocus {
                            watchlist.search_refocus = false;
                            search_resp.request_focus();
                        }
                        if search_resp.changed() {
                            watchlist.search_sel = -1; // reset selection on text change
                            if !watchlist.search_query.is_empty() {
                                // Immediate: static results
                                watchlist.search_results = ui_kit::symbols::search_symbols(&watchlist.search_query, 8)
                                    .iter().map(|s| (s.symbol.to_string(), s.name.to_string())).collect();
                                // Background: ApexIB search (results merge via SearchResults command)
                                fetch_search_background(watchlist.search_query.clone(), "watchlist".to_string());
                            } else {
                                watchlist.search_results.clear();
                            }
                        }
                        // Arrow key navigation + Enter to select
                        let has_results = !watchlist.search_query.is_empty() && !watchlist.search_results.is_empty();
                        if has_results && search_resp.has_focus() {
                            let max = watchlist.search_results.len() as i32;
                            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                                watchlist.search_sel = (watchlist.search_sel + 1).min(max - 1);
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                                watchlist.search_sel = (watchlist.search_sel - 1).max(-1);
                            }
                        }
                        // Enter: add highlighted or typed symbol
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !watchlist.search_query.is_empty() {
                            let sym = if watchlist.search_sel >= 0 && (watchlist.search_sel as usize) < watchlist.search_results.len() {
                                watchlist.search_results[watchlist.search_sel as usize].0.clone()
                            } else {
                                watchlist.search_query.trim().to_uppercase()
                            };
                            watchlist.add_symbol(&sym);
                            fetch_watchlist_prices(vec![sym]);
                            watchlist.search_query.clear();
                            watchlist.search_results.clear();
                            watchlist.search_sel = -1;
                            watchlist.search_refocus = true;
                            watchlist.persist();
                        }
                        // Escape clears search
                        if search_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            watchlist.search_query.clear();
                            watchlist.search_results.clear();
                            watchlist.search_sel = -1;
                        }
                        // Suggestion dropdown
                        if has_results {
                            egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(28, 28, 34)).corner_radius(4.0).show(ui, |ui| {
                                for (i, (sym, name)) in watchlist.search_results.clone().iter().enumerate() {
                                    let is_sel = i as i32 == watchlist.search_sel;
                                    let bg = if is_sel { color_alpha(t.accent, 30) } else { egui::Color32::TRANSPARENT };
                                    let fg = if is_sel { egui::Color32::from_rgb(230, 230, 240) } else { t.dim };
                                    let resp = ui.add(egui::Button::new(
                                        egui::RichText::new(format!("{:6} {}", sym, name)).monospace().size(10.0).color(fg))
                                        .fill(bg).frame(false).min_size(egui::vec2(ui.available_width(), 20.0)));
                                    if resp.clicked() {
                                        watchlist.add_symbol(sym);
                                        fetch_watchlist_prices(vec![sym.clone()]);
                                        watchlist.search_query.clear();
                                        watchlist.search_results.clear();
                                        watchlist.search_sel = -1;
                                        watchlist.search_refocus = true;
                                        watchlist.persist();
                                    }
                                    if resp.hovered() {
                                        watchlist.search_sel = i as i32;
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                }
                            });
                        }
                        ui.add_space(4.0);

                        // Symbol list with sections and drag-and-drop
                        let active_sym = panes[ap].symbol.clone();
                        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
                        let pointer_released = ui.ctx().input(|i| i.pointer.any_released());
                        let pointer_down = ui.ctx().input(|i| i.pointer.any_down());

                        // Mark which sections are option sections
                        let option_section_ids: Vec<u32> = watchlist.sections.iter()
                            .filter(|s| s.title.contains("Options"))
                            .map(|s| s.id).collect();

                        // Options section always visible when toggled on (even if empty)
                        let show_opts = watchlist.options_visible;
                        let total_avail = ui.available_height();
                        let stocks_h = if show_opts { (total_avail * watchlist.options_split).max(60.0) } else { total_avail };

                        egui::ScrollArea::vertical().id_salt("wl_stocks").max_height(stocks_h).show(ui, |ui| {
                            let mut remove_sym: Option<String> = None;
                            let mut click_sym: Option<String> = None;
                            let mut click_opt: Option<(String, f32, bool, String)> = None; // option click -> open chart
                            let mut toggle_collapse: Option<usize> = None;
                            let mut remove_section: Option<usize> = None;
                            let full_w = ui.available_width();

                            // Collect row rects for drop target calculation
                            let mut row_rects: Vec<(usize, usize, egui::Rect)> = Vec::new(); // (sec_idx, item_idx, rect)
                            let mut section_header_rects: Vec<(usize, egui::Rect)> = Vec::new();

                            let section_count = watchlist.sections.len();
                            let dragging = watchlist.dragging;
                            let drag_confirmed = watchlist.drag_confirmed;

                            // Section color presets for the color picker
                            let color_presets = ["#4a9eff","#e74c3c","#2ecc71","#f39c12","#9b59b6","#1abc9c","#e67e22","#3498db","#e91e63","#00bcd4","#8bc34a","#ff5722","#607d8b","#795548","#cddc39","#ff9800"];


                            for si in 0..section_count {
                                let sec_id = watchlist.sections[si].id;
                                let is_option_section = option_section_ids.contains(&sec_id);

                                // Option sections render in the bottom options scroll, not here
                                if is_option_section { continue; }

                                let sec_title = watchlist.sections[si].title.clone();
                                let sec_color = watchlist.sections[si].color.clone();
                                let sec_collapsed = watchlist.sections[si].collapsed;
                                let sec_item_count = watchlist.sections[si].items.len();

                                // ── Section divider line (skip if thick options divider just drawn) ──
                                if si > 0 {
                                    ui.add_space(2.0);
                                    let cursor_y = ui.cursor().min.y;
                                    ui.painter().line_segment(
                                        [egui::pos2(ui.min_rect().left(), cursor_y),
                                         egui::pos2(ui.min_rect().left() + full_w, cursor_y)],
                                        egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80)));
                                    ui.add_space(2.0);
                                }

                                // ── Track section start for continuous background ──
                                let section_block_start_y = ui.cursor().min.y;

                                // Remove item_spacing.y within section for flush rows
                                let prev_item_spacing_y = ui.spacing().item_spacing.y;
                                ui.spacing_mut().item_spacing.y = 0.0;

                                // ── Section header (only if title is non-empty) ──
                                if !sec_title.is_empty() && watchlist.renaming_section != Some(sec_id) {
                                    let header_resp = ui.horizontal(|ui| {
                                        ui.set_min_width(full_w);
                                        ui.set_min_height(20.0);

                                        // Collapse chevron
                                        let chevron = if sec_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                                        if ui.add(egui::Button::new(egui::RichText::new(chevron).size(10.0).color(t.dim.gamma_multiply(0.6))).frame(false)).clicked() {
                                            toggle_collapse = Some(si);
                                        }

                                        // Title
                                        ui.label(egui::RichText::new(&sec_title).monospace().size(9.0).strong()
                                            .color(t.dim.gamma_multiply(0.6)));

                                        // Item count when collapsed
                                        if sec_collapsed {
                                            ui.label(egui::RichText::new(format!("({})", sec_item_count)).monospace().size(8.0)
                                                .color(t.dim.gamma_multiply(0.3)));
                                        }

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Delete section (only if empty)
                                            if sec_item_count == 0 {
                                                if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(8.0).color(t.dim.gamma_multiply(0.3))).frame(false)).clicked() {
                                                    remove_section = Some(si);
                                                }
                                            }
                                        });
                                    });
                                    section_header_rects.push((si, header_resp.response.rect));

                                    // Right-click context menu on section header
                                    header_resp.response.context_menu(|ui| {
                                        // Rename
                                        if ui.button(egui::RichText::new("Rename").monospace().size(10.0)).clicked() {
                                            watchlist.renaming_section = Some(sec_id);
                                            watchlist.rename_buf = sec_title.clone();
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        // Color presets
                                        ui.label(egui::RichText::new("Color").monospace().size(9.0).color(t.dim));
                                        for row in color_presets.chunks(8) {
                                            ui.horizontal(|ui| {
                                                for hex in row {
                                                    let c = hex_to_color(hex, 1.0);
                                                    if ui.add(egui::Button::new(egui::RichText::new("\u{25CF}").size(14.0).color(c)).frame(false)).clicked() {
                                                        if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                            sec.color = Some(hex.to_string());
                                                        }
                                                        watchlist.persist();
                                                        ui.close_menu();
                                                    }
                                                }
                                            });
                                        }
                                        if ui.button(egui::RichText::new("No color").monospace().size(10.0).color(t.dim)).clicked() {
                                            if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                sec.color = None;
                                            }
                                            watchlist.persist();
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if sec_item_count == 0 {
                                            if ui.button(egui::RichText::new("Delete section").monospace().size(10.0).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                                                remove_section = Some(si);
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                }

                                // ── Inline rename editor (replaces title in header row) ──
                                if watchlist.renaming_section == Some(sec_id) {
                                    ui.horizontal(|ui| {
                                        ui.set_min_width(full_w);
                                        ui.set_min_height(20.0);

                                        // Collapse chevron (keep visible during rename)
                                        let chevron = if sec_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                                        ui.add(egui::Button::new(egui::RichText::new(chevron).size(10.0).color(t.dim.gamma_multiply(0.6))).frame(false));

                                        let te = ui.add(egui::TextEdit::singleline(&mut watchlist.rename_buf)
                                            .desired_width(full_w - 40.0).font(egui::FontId::monospace(9.0)));
                                        if te.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                sec.title = watchlist.rename_buf.clone();
                                            }
                                            watchlist.renaming_section = None;
                                            watchlist.persist();
                                        }
                                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                            watchlist.renaming_section = None;
                                        }
                                        te.request_focus();
                                    });
                                }

                                // ── Section items (skip if collapsed) ──
                                if !sec_collapsed {
                                    for ii in 0..sec_item_count {
                                        let item = &watchlist.sections[si].items[ii];
                                        let item_sym = item.symbol.clone();
                                        let item_price = item.price;
                                        let item_prev_close = item.prev_close;
                                        let item_loaded = item.loaded;
                                        let item_is_option = item.is_option;
                                        let item_underlying = item.underlying.clone();
                                        let item_option_type = item.option_type.clone();
                                        let item_strike = item.strike;
                                        let item_expiry = item.expiry.clone();
                                        let item_bid = item.bid;
                                        let item_ask = item.ask;
                                        let is_dragged = drag_confirmed && dragging == Some((si, ii));

                                        // Skip rendering the dragged item in-place (it's shown as floating)
                                        if is_dragged {
                                            // Reserve space so layout doesn't shift
                                            let placeholder = ui.allocate_space(egui::vec2(full_w, 24.0));
                                            row_rects.push((si, ii, placeholder.1));
                                            continue;
                                        }

                                        let is_active = item_sym == active_sym;

                                        if item_is_option {
                                            // ── Option item rendering ──
                                            let opt_color = if item_option_type == "C" { t.bull } else { t.bear };
                                            let price_str = if item_bid > 0.0 || item_ask > 0.0 {
                                                format!("{:.2} \u{00D7} {:.2}", item_bid, item_ask)
                                            } else if item_price > 0.0 {
                                                format!("{:.2}", item_price)
                                            } else {
                                                "---".into()
                                            };
                                            let row_bg = if is_active { color_alpha(t.accent, 18) } else { egui::Color32::TRANSPARENT };

                                            let resp = ui.horizontal(|ui| {
                                                ui.set_min_width(full_w);
                                                ui.set_min_height(24.0);
                                                ui.painter().rect_filled(ui.max_rect(), 0.0, row_bg);
                                                if is_active {
                                                    let r = ui.max_rect();
                                                    ui.painter().rect_filled(
                                                        egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + 2.5, r.max.y)),
                                                        1.0, t.accent);
                                                }
                                                ui.add_space(if is_active { 8.0 } else { 4.0 });
                                                // Drag grip
                                                ui.label(egui::RichText::new(Icon::DOTS_SIX_VERTICAL).size(9.0).color(t.dim.gamma_multiply(0.2)));
                                                ui.add_space(2.0);
                                                // C/P badge
                                                let badge_bg = color_alpha(opt_color, 35);
                                                let badge_resp = ui.add(egui::Button::new(
                                                    egui::RichText::new(&item_option_type).monospace().size(9.0).strong().color(opt_color))
                                                    .fill(badge_bg).corner_radius(2.0).stroke(egui::Stroke::NONE)
                                                    .min_size(egui::vec2(16.0, 16.0)));
                                                let _ = badge_resp;
                                                ui.add_space(2.0);
                                                // Full option name (e.g. "SPY 560C 0DTE")
                                                let sym_color = if is_active { egui::Color32::from_rgb(240, 240, 245) } else { egui::Color32::from_rgb(200, 200, 210) };
                                                ui.label(egui::RichText::new(&item_sym).monospace().size(10.5).strong().color(sym_color));
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    // X button
                                                    if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.3))).frame(false)).clicked() {
                                                        remove_sym = Some(item_sym.clone());
                                                    }
                                                    // Bid x Ask (or price fallback)
                                                    ui.label(egui::RichText::new(&price_str).monospace().size(11.0).color(opt_color));
                                                });
                                            });

                                            let row_rect = resp.response.rect;
                                            row_rects.push((si, ii, row_rect));

                                            let drag_resp = resp.response.interact(egui::Sense::click_and_drag());
                                            if drag_resp.drag_started() {
                                                watchlist.dragging = Some((si, ii));
                                                watchlist.drag_start_pos = pointer_pos;
                                                watchlist.drag_confirmed = false;
                                            }
                                            // Click opens option chart (not stock symbol change)
                                            if drag_resp.clicked() && !drag_confirmed {
                                                let is_call = item_option_type == "C";
                                                click_opt = Some((item_underlying.clone(), item_strike, is_call, item_expiry.clone()));
                                            }
                                            if drag_resp.hovered() && !drag_confirmed {
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                if !is_active {
                                                    ui.painter().rect_filled(row_rect, 0.0, color_alpha(t.toolbar_border, 25));
                                                }
                                            }
                                        } else {
                                            // ── Stock item rendering — column-aligned ──
                                            let change_pct = if item_prev_close > 0.0 { ((item_price - item_prev_close) / item_prev_close) * 100.0 } else { 0.0 };
                                            let color = if change_pct >= 0.0 { t.bull } else { t.bear };
                                            let price_str = if item_price > 0.0 { format!("{:.2}", item_price) } else { "---".into() };
                                            let change_str = if item_loaded { format!("{:+.2}%", change_pct) } else { "".into() };

                                            let row_bg = if is_active { color_alpha(t.accent, 18) } else { egui::Color32::TRANSPARENT };
                                            let row_h = 28.0;

                                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(full_w, row_h), egui::Sense::click_and_drag());
                                            let painter = ui.painter();

                                            // Background
                                            painter.rect_filled(rect, 0.0, row_bg);
                                            // Active indicator bar
                                            if is_active {
                                                painter.rect_filled(
                                                    egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 2.5, rect.max.y)),
                                                    1.0, t.accent);
                                            }

                                            let y_c = rect.center().y;
                                            let left = rect.left();

                                            // Grip dots
                                            painter.text(egui::pos2(left + 6.0, y_c), egui::Align2::LEFT_CENTER,
                                                Icon::DOTS_SIX_VERTICAL, egui::FontId::proportional(9.0), t.dim.gamma_multiply(0.2));

                                            // Symbol (left-aligned)
                                            let sym_color = if is_active { egui::Color32::from_rgb(245, 245, 250) } else { egui::Color32::from_rgb(225, 225, 235) };
                                            painter.text(egui::pos2(left + 18.0, y_c), egui::Align2::LEFT_CENTER,
                                                &item_sym, egui::FontId::monospace(14.0), sym_color);

                                            // Change % (center-left, prominent)
                                            let mid_x = rect.left() + full_w * 0.38;
                                            painter.text(egui::pos2(mid_x, y_c), egui::Align2::LEFT_CENTER,
                                                &change_str, egui::FontId::monospace(14.0), color);

                                            // Price (right-aligned, leave room for X button)
                                            painter.text(egui::pos2(rect.right() - 24.0, y_c), egui::Align2::RIGHT_CENTER,
                                                &price_str, egui::FontId::monospace(14.0), color.gamma_multiply(0.6));

                                            // Faint row separator line
                                            painter.line_segment(
                                                [egui::pos2(rect.left() + 16.0, rect.bottom() - 0.5), egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5)],
                                                egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 40)));

                                            // X button zone (far right)
                                            if resp.hovered() {
                                                painter.text(egui::pos2(rect.right() - 6.0, y_c), egui::Align2::RIGHT_CENTER,
                                                    Icon::X, egui::FontId::proportional(9.0), t.dim.gamma_multiply(0.4));
                                                let x_zone = egui::Rect::from_min_max(egui::pos2(rect.right() - 16.0, rect.top()), rect.max);
                                                if ui.interact(x_zone, egui::Id::new(("wl_x", si, ii)), egui::Sense::click()).clicked() {
                                                    remove_sym = Some(item_sym.clone());
                                                }
                                            }

                                            // Hover highlight
                                            if resp.hovered() && !is_active {
                                                painter.rect_filled(rect, 0.0, color_alpha(t.toolbar_border, 20));
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                            }

                                            let row_rect = rect;
                                            row_rects.push((si, ii, row_rect));

                                            // Drag-and-drop + click handling
                                            if resp.drag_started() {
                                                watchlist.dragging = Some((si, ii));
                                                watchlist.drag_start_pos = pointer_pos;
                                                watchlist.drag_confirmed = false;
                                            }
                                            if resp.clicked() && !drag_confirmed {
                                                click_sym = Some(item_sym.clone());
                                            }

                                            // (hover already handled above in painter section)
                                        }
                                    }
                                }

                                // Restore item_spacing.y
                                ui.spacing_mut().item_spacing.y = prev_item_spacing_y;

                                // ── Paint continuous section background tint (header + all items) ──
                                if let Some(ref hex) = sec_color {
                                    let section_block_end_y = ui.cursor().min.y;
                                    if section_block_end_y > section_block_start_y {
                                        let left = ui.min_rect().left();
                                        let block_rect = egui::Rect::from_min_max(
                                            egui::pos2(left, section_block_start_y),
                                            egui::pos2(left + full_w, section_block_end_y));
                                        // Items area: low opacity tint (~18 alpha)
                                        ui.painter().rect_filled(block_rect, 0.0, hex_to_color(hex, 0.07));
                                        // Header area: darker tint overlay (~35 alpha)
                                        if let Some(&(_, header_rect)) = section_header_rects.iter().find(|&&(s, _)| s == si) {
                                            let header_tint_rect = egui::Rect::from_min_max(
                                                egui::pos2(left, header_rect.min.y),
                                                egui::pos2(left + full_w, header_rect.max.y));
                                            ui.painter().rect_filled(header_tint_rect, 0.0, hex_to_color(hex, 0.07));
                                        }
                                    }
                                }
                            } // end sections loop

                            // ── Drag-and-drop logic ──
                            // Confirm drag after mouse moves enough (5px threshold)
                            if let (Some(start), Some(cur)) = (watchlist.drag_start_pos, pointer_pos) {
                                if watchlist.dragging.is_some() && !watchlist.drag_confirmed {
                                    if (cur - start).length() > 5.0 {
                                        watchlist.drag_confirmed = true;
                                    }
                                }
                            }

                            // Calculate drop target from mouse position
                            if watchlist.drag_confirmed {
                                if let Some(mouse) = pointer_pos {
                                    let mut best: Option<(usize, usize, f32)> = None; // (sec, insert_idx, dist)
                                    for &(si, ii, rect) in &row_rects {
                                        let mid_y = rect.center().y;
                                        let dist = (mouse.y - mid_y).abs();
                                        // Insert before this item if mouse is above midpoint
                                        let insert_idx = if mouse.y < mid_y { ii } else { ii + 1 };
                                        if best.is_none() || dist < best.unwrap().2 {
                                            best = Some((si, insert_idx, dist));
                                        }
                                    }
                                    // Also consider dropping at the end of each section
                                    for &(si, rect) in &section_header_rects {
                                        if mouse.y > rect.max.y && watchlist.sections[si].items.is_empty() {
                                            best = Some((si, 0, 0.0));
                                        }
                                    }
                                    watchlist.drop_target = best.map(|(s, i, _)| (s, i));
                                }

                                // Draw insertion indicator line
                                if let Some((dt_sec, dt_idx)) = watchlist.drop_target {
                                    // Find the Y position for the indicator
                                    let indicator_y = if let Some(&(_, _, rect)) = row_rects.iter().find(|&&(s, i, _)| s == dt_sec && i == dt_idx) {
                                        rect.min.y
                                    } else if dt_idx > 0 {
                                        // Insert after last item
                                        row_rects.iter().filter(|&&(s, _, _)| s == dt_sec)
                                            .last().map(|&(_, _, rect)| rect.max.y)
                                            .unwrap_or(0.0)
                                    } else {
                                        // Empty section — use header rect bottom
                                        section_header_rects.iter().find(|&&(s, _)| s == dt_sec)
                                            .map(|&(_, rect)| rect.max.y + 2.0)
                                            .unwrap_or(0.0)
                                    };
                                    if indicator_y > 0.0 {
                                        let left = ui.min_rect().left();
                                        ui.painter().line_segment(
                                            [egui::pos2(left, indicator_y), egui::pos2(left + full_w, indicator_y)],
                                            egui::Stroke::new(2.0, t.accent));
                                        // Small circles at endpoints
                                        ui.painter().circle_filled(egui::pos2(left + 2.0, indicator_y), 3.0, t.accent);
                                        ui.painter().circle_filled(egui::pos2(left + full_w - 2.0, indicator_y), 3.0, t.accent);
                                    }
                                }

                                // Draw floating label at cursor
                                if let (Some((src_sec, src_idx)), Some(mouse)) = (watchlist.dragging, pointer_pos) {
                                    if src_sec < watchlist.sections.len() && src_idx < watchlist.sections[src_sec].items.len() {
                                        let drag_sym = &watchlist.sections[src_sec].items[src_idx].symbol;
                                        let float_rect = egui::Rect::from_min_size(
                                            egui::pos2(mouse.x - 30.0, mouse.y - 10.0), egui::vec2(80.0, 20.0));
                                        ui.painter().rect_filled(float_rect, 4.0, color_alpha(t.accent, 40));
                                        ui.painter().rect_stroke(float_rect, 4.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Outside);
                                        ui.painter().text(float_rect.center(), egui::Align2::CENTER_CENTER,
                                            drag_sym, egui::FontId::monospace(11.0), egui::Color32::from_rgb(240, 240, 245));
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    }
                                }
                            }

                            // Drop: on pointer release while dragging
                            if pointer_released && watchlist.drag_confirmed {
                                if let (Some((src_sec, src_idx)), Some((dst_sec, dst_idx))) = (watchlist.dragging, watchlist.drop_target) {
                                    // Adjust destination index if same section and source is before target
                                    let adj_dst = if src_sec == dst_sec && src_idx < dst_idx { dst_idx - 1 } else { dst_idx };
                                    watchlist.move_item(src_sec, src_idx, dst_sec, adj_dst);
                                    watchlist.persist();
                                }
                                watchlist.dragging = None;
                                watchlist.drag_start_pos = None;
                                watchlist.drop_target = None;
                                watchlist.drag_confirmed = false;
                            }
                            // Cancel drag if pointer released without confirming
                            if pointer_released && watchlist.dragging.is_some() && !watchlist.drag_confirmed {
                                watchlist.dragging = None;
                                watchlist.drag_start_pos = None;
                                watchlist.drop_target = None;
                            }
                            // Cancel drag if pointer is no longer down (safety)
                            if !pointer_down && watchlist.dragging.is_some() {
                                watchlist.dragging = None;
                                watchlist.drag_start_pos = None;
                                watchlist.drop_target = None;
                                watchlist.drag_confirmed = false;
                            }

                            // ── Add section button ──
                            // "+ Section" always at bottom of stocks scroll
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui.add(egui::Button::new(egui::RichText::new(format!("{} Section", Icon::PLUS)).monospace().size(9.0).color(t.dim.gamma_multiply(0.4)))
                                    .frame(false)).clicked() {
                                    watchlist.add_section("New Section");
                                    watchlist.persist();
                                }
                            });

                            if let Some(sym) = click_sym {
                                panes[ap].pending_symbol_change = Some(sym.clone());
                                panes[ap].is_option = false; // reset option flag when switching to stock
                            }
                            if let Some(opt_info) = click_opt {
                                open_option_chart = Some(opt_info);
                            }
                            if let Some(sym) = remove_sym { watchlist.remove_symbol(&sym); watchlist.persist(); }
                            if let Some(si) = toggle_collapse {
                                watchlist.sections[si].collapsed = !watchlist.sections[si].collapsed;
                                watchlist.persist();
                            }
                            if let Some(si) = remove_section {
                                if si < watchlist.sections.len() && watchlist.sections[si].items.is_empty() {
                                    watchlist.sections.remove(si);
                                    watchlist.persist();
                                }
                            }
                        }); // end stocks scroll

                        // ── Draggable divider + Options scroll ──
                        if show_opts {
                            // Divider bar — allocate a draggable strip, decoupled from egui interaction
                            ui.add_space(2.0);
                            let div_r = ui.available_rect_before_wrap();
                            let div_y = ui.cursor().min.y;
                            let div_rect = egui::Rect::from_min_max(
                                egui::pos2(div_r.left(), div_y),
                                egui::pos2(div_r.right(), div_y + 6.0));
                            ui.painter().rect_filled(
                                egui::Rect::from_min_max(
                                    egui::pos2(div_r.left(), div_y + 1.0),
                                    egui::pos2(div_r.right(), div_y + 4.0)),
                                0.0, color_alpha(t.toolbar_border, 160));
                            // Store divider Y position for drag handling outside the panel
                            watchlist.divider_y = div_rect.center().y;
                            watchlist.divider_total_h = total_avail;
                            // Show resize cursor on hover
                            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                                if div_rect.expand(6.0).contains(pos) || watchlist.divider_dragging {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                                }
                            }
                            ui.add_space(6.0);

                            // OPTIONS label
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("OPTIONS").monospace().size(9.0).strong().color(t.accent.gamma_multiply(0.7)));
                                let opt_count: usize = watchlist.sections.iter()
                                    .filter(|s| s.title.contains("Options"))
                                    .map(|s| s.items.len()).sum();
                                if opt_count > 0 {
                                    ui.label(egui::RichText::new(format!("({})", opt_count)).monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                                }
                            });
                            ui.add_space(2.0);

                            egui::ScrollArea::vertical().id_salt("wl_options").show(ui, |ui| {
                                let active_sym = panes[ap].symbol.clone();
                                let mut click_opt: Option<(String, f32, bool, String)> = None;
                                let mut remove_sym: Option<String> = None;
                                let mut opt_remove_section: Option<usize> = None;
                                let mut opt_toggle_collapse: Option<usize> = None;
                                let color_presets = ["#4a9eff","#e74c3c","#2ecc71","#f39c12","#9b59b6","#1abc9c","#e67e22","#3498db","#e91e63","#00bcd4","#8bc34a","#ff5722","#607d8b","#795548","#cddc39","#ff9800"];

                                for si in 0..watchlist.sections.len() {
                                    if !option_section_ids.contains(&watchlist.sections[si].id) { continue; }
                                    let sec_id = watchlist.sections[si].id;
                                    let sec_title = watchlist.sections[si].title.clone();
                                    let sec_color = watchlist.sections[si].color.clone();
                                    let sec_collapsed = watchlist.sections[si].collapsed;
                                    let sec_item_count = watchlist.sections[si].items.len();
                                    let full_w = ui.available_width();

                                    let section_block_start_y = ui.cursor().min.y;

                                    // Section header with collapse chevron
                                    let header_resp = ui.horizontal(|ui| {
                                        ui.set_min_width(full_w);
                                        ui.set_min_height(20.0);
                                        let chevron = if sec_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                                        if ui.add(egui::Button::new(egui::RichText::new(chevron).size(10.0).color(t.dim.gamma_multiply(0.6))).frame(false)).clicked() {
                                            opt_toggle_collapse = Some(si);
                                        }
                                        ui.label(egui::RichText::new(&sec_title).monospace().size(9.0).strong().color(t.dim.gamma_multiply(0.6)));
                                        if sec_collapsed {
                                            ui.label(egui::RichText::new(format!("({})", sec_item_count)).monospace().size(8.0).color(t.dim.gamma_multiply(0.3)));
                                        }
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if sec_item_count == 0 {
                                                if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(8.0).color(t.dim.gamma_multiply(0.3))).frame(false)).clicked() {
                                                    opt_remove_section = Some(si);
                                                }
                                            }
                                        });
                                    });

                                    // Right-click context menu on option section header (same as stock sections)
                                    header_resp.response.context_menu(|ui| {
                                        // Rename
                                        if ui.button(egui::RichText::new("Rename").monospace().size(10.0)).clicked() {
                                            watchlist.renaming_section = Some(sec_id);
                                            watchlist.rename_buf = sec_title.clone();
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        // Color presets
                                        ui.label(egui::RichText::new("Color").monospace().size(9.0).color(t.dim));
                                        for row in color_presets.chunks(8) {
                                            ui.horizontal(|ui| {
                                                for hex in row {
                                                    let c = hex_to_color(hex, 1.0);
                                                    if ui.add(egui::Button::new(egui::RichText::new("\u{25CF}").size(14.0).color(c)).frame(false)).clicked() {
                                                        if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                            sec.color = Some(hex.to_string());
                                                        }
                                                        watchlist.persist();
                                                        ui.close_menu();
                                                    }
                                                }
                                            });
                                        }
                                        if ui.button(egui::RichText::new("No color").monospace().size(10.0).color(t.dim)).clicked() {
                                            if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                                                sec.color = None;
                                            }
                                            watchlist.persist();
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if sec_item_count == 0 {
                                            if ui.button(egui::RichText::new("Delete section").monospace().size(10.0).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                                                opt_remove_section = Some(si);
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                    ui.add_space(2.0);

                                    if !sec_collapsed {
                                        for ii in 0..sec_item_count {
                                            let item = &watchlist.sections[si].items[ii];
                                            let item_sym = item.symbol.clone();
                                            let item_underlying = item.underlying.clone();
                                            let item_option_type = item.option_type.clone();
                                            let item_strike = item.strike;
                                            let item_expiry = item.expiry.clone();
                                            let item_bid = item.bid;
                                            let item_ask = item.ask;
                                            let is_call = item_option_type == "C";
                                            let color = if is_call { t.bull } else { t.bear };
                                            let is_active = item_sym == active_sym;
                                            let row_bg = if is_active { color_alpha(t.accent, 18) } else { egui::Color32::TRANSPARENT };

                                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(full_w, 28.0), egui::Sense::click());
                                            let painter = ui.painter();
                                            painter.rect_filled(rect, 0.0, row_bg);
                                            if resp.hovered() {
                                                painter.rect_filled(rect, 0.0, color_alpha(t.toolbar_border, 25));
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                            }

                                            let badge = if is_call { "C" } else { "P" };
                                            let y_c = rect.center().y;
                                            // C/P badge
                                            painter.text(egui::pos2(rect.left() + 6.0, y_c), egui::Align2::LEFT_CENTER,
                                                badge, egui::FontId::monospace(11.0), color);
                                            // Contract name
                                            painter.text(egui::pos2(rect.left() + 22.0, y_c), egui::Align2::LEFT_CENTER,
                                                &format!("{} {:.0} {}", item_underlying, item_strike, item_expiry),
                                                egui::FontId::monospace(14.0), egui::Color32::from_rgb(225, 225, 235));
                                            // Bid x Ask (right-aligned)
                                            if item_bid > 0.0 || item_ask > 0.0 {
                                                painter.text(egui::pos2(rect.right() - 6.0, y_c), egui::Align2::RIGHT_CENTER,
                                                    &format!("{:.2} x {:.2}", item_bid, item_ask),
                                                    egui::FontId::monospace(14.0), color.gamma_multiply(0.7));
                                            }
                                            // Faint separator
                                            painter.line_segment(
                                                [egui::pos2(rect.left() + 16.0, rect.bottom() - 0.5), egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5)],
                                                egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 40)));

                                            if resp.clicked() {
                                                click_opt = Some((item_underlying.clone(), item_strike, is_call, item_expiry.clone()));
                                            }

                                            // X button to remove
                                            let x_rect = egui::Rect::from_min_size(egui::pos2(rect.right() - 16.0, rect.top()), egui::vec2(16.0, 22.0));
                                            if resp.hovered() {
                                                let x_resp = ui.interact(x_rect, egui::Id::new(("opt_x", si, ii, "opt_item")), egui::Sense::click());
                                                if x_resp.clicked() { remove_sym = Some(item_sym.clone()); }
                                            }
                                        }
                                    }

                                    // Paint continuous section background tint
                                    let section_block_end_y = ui.cursor().min.y;
                                    if let Some(ref hex) = sec_color {
                                        if section_block_end_y > section_block_start_y {
                                            let left = ui.min_rect().left();
                                            let block_rect = egui::Rect::from_min_max(
                                                egui::pos2(left, section_block_start_y),
                                                egui::pos2(left + full_w, section_block_end_y));
                                            ui.painter().rect_filled(block_rect, 0.0, hex_to_color(hex, 0.07));
                                        }
                                    }
                                    ui.add_space(4.0);
                                }

                                // Empty state
                                if option_section_ids.is_empty() {
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("No options saved").monospace().size(10.0).color(t.dim.gamma_multiply(0.35)));
                                    ui.label(egui::RichText::new("Shift+click contracts in the CHAIN tab").monospace().size(8.0).color(t.dim.gamma_multiply(0.25)));
                                    ui.add_space(8.0);
                                }

                                // "+ Section" button at bottom of options area
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    if ui.add(egui::Button::new(egui::RichText::new(format!("{} Section", Icon::PLUS)).monospace().size(9.0).color(t.dim.gamma_multiply(0.4)))
                                        .frame(false)).clicked() {
                                        watchlist.add_option_section("New Options");
                                        watchlist.persist();
                                    }
                                });

                                if let Some(opt_info) = click_opt {
                                    open_option_chart = Some(opt_info);
                                }
                                if let Some(sym) = remove_sym {
                                    watchlist.remove_symbol(&sym);
                                    watchlist.persist();
                                }
                                if let Some(si) = opt_toggle_collapse {
                                    watchlist.sections[si].collapsed = !watchlist.sections[si].collapsed;
                                    watchlist.persist();
                                }
                                if let Some(si) = opt_remove_section {
                                    if si < watchlist.sections.len() && watchlist.sections[si].items.is_empty() {
                                        watchlist.sections.remove(si);
                                        watchlist.persist();
                                    }
                                }
                            });
                        }
                    }

                    // ── CHAIN TAB ───────────────────────────────────────────
                    WatchlistTab::Chain => {
                        // Rebuild chain price
                        let chain_price = watchlist.find_item(&watchlist.chain_symbol).map(|i| i.price)
                            .or_else(|| panes.iter().find(|p| p.symbol == watchlist.chain_symbol).and_then(|p| p.bars.last().map(|b| b.close)))
                            .unwrap_or(100.0);
                        if chain_price > 0.0 && watchlist.chain_0dte.0.is_empty() && !watchlist.chain_loading {
                            let ns = watchlist.chain_num_strikes;
                            let sym = watchlist.chain_symbol.clone();
                            let far_dte = watchlist.chain_far_dte;
                            watchlist.chain_loading = true;
                            watchlist.chain_last_fetch = Some(std::time::Instant::now());
                            fetch_chain_background(sym.clone(), ns, 0, chain_price);
                            fetch_chain_background(sym, ns, far_dte, chain_price);
                        }

                        // ── Controls FIRST: strikes ± | DTE selector | sel toggle ──
                        ui.horizontal(|ui| {
                            dim_label(ui, "strikes", t.dim);
                            if ui.add(egui::Button::new(egui::RichText::new("-").monospace().size(10.0)).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                watchlist.chain_num_strikes = watchlist.chain_num_strikes.saturating_sub(1).max(1);
                                let sym = watchlist.chain_symbol.clone();
                                let ns = watchlist.chain_num_strikes;
                                let far_dte = watchlist.chain_far_dte;
                                watchlist.chain_loading = true;
                                fetch_chain_background(sym.clone(), ns, 0, chain_price);
                                fetch_chain_background(sym, ns, far_dte, chain_price);
                            }
                            ui.label(egui::RichText::new(format!("{}", watchlist.chain_num_strikes)).monospace().size(10.0).color(egui::Color32::from_rgb(200,200,210)));
                            if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(10.0)).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                watchlist.chain_num_strikes += 1;
                                let sym = watchlist.chain_symbol.clone();
                                let ns = watchlist.chain_num_strikes;
                                let far_dte = watchlist.chain_far_dte;
                                watchlist.chain_loading = true;
                                fetch_chain_background(sym.clone(), ns, 0, chain_price);
                                fetch_chain_background(sym, ns, far_dte, chain_price);
                            }

                            // Trading day functions: trading_date(), trading_month_name(), dte_label() — defined at module level

                            // DTE dropdown
                            let dte_values = [1, 2, 3, 5, 7, 10];
                            let cur_label = dte_label(watchlist.chain_far_dte);
                            egui::ComboBox::from_id_salt("far_dte").selected_text(egui::RichText::new(&cur_label).monospace().size(9.0).color(t.dim)).width(100.0)
                                .show_ui(ui, |ui| {
                                    for &d in &dte_values {
                                        let label = dte_label(d);
                                        if ui.selectable_value(&mut watchlist.chain_far_dte, d, &label).changed() {
                                            let sym = watchlist.chain_symbol.clone();
                                            watchlist.chain_loading = true;
                                            fetch_chain_background(sym, watchlist.chain_num_strikes, d, chain_price);
                                        }
                                    }
                                });

                            // Select mode toggle
                            let sel_label = if watchlist.chain_select_mode { format!("{} sel", Icon::CHECK) } else { "sel".to_string() };
                            let sel_active = watchlist.chain_select_mode;
                            if ui.add(egui::Button::new(egui::RichText::new(sel_label).monospace().size(9.0)
                                .color(if sel_active { t.accent } else { t.dim }))
                                .fill(if sel_active { egui::Color32::from_rgba_unmultiplied(t.accent.r(),t.accent.g(),t.accent.b(),51) } else { t.toolbar_bg })
                                .stroke(egui::Stroke::new(1.0, if sel_active { t.accent } else { t.toolbar_border }))
                                .corner_radius(2.0)).clicked() {
                                watchlist.chain_select_mode = !watchlist.chain_select_mode;
                            }
                        });

                        ui.add_space(4.0);

                        // ── Symbol selector + price ──
                        ui.horizontal(|ui| {
                            let has_focus = ui.memory(|m| m.has_focus(egui::Id::new("chain_sym_edit")));
                            let input_bg = if has_focus { color_alpha(t.toolbar_border, 60) } else { color_alpha(t.toolbar_border, 15) };
                            let sym_resp = ui.add(egui::TextEdit::singleline(&mut watchlist.chain_sym_input)
                                .id(egui::Id::new("chain_sym_edit"))
                                .hint_text(&watchlist.chain_symbol)
                                .desired_width(70.0)
                                .font(egui::FontId::monospace(14.0))
                                .text_color(t.accent)
                                .background_color(input_bg)
                                .margin(egui::Margin::symmetric(4, 3)));
                            if !has_focus {
                                let display_text = if watchlist.chain_sym_input.is_empty() { &watchlist.chain_symbol } else { &watchlist.chain_sym_input };
                                let r = sym_resp.rect;
                                ui.painter().text(egui::pos2(r.left() + 6.0, r.center().y), egui::Align2::LEFT_CENTER,
                                    display_text, egui::FontId::monospace(14.0), t.accent);
                            }
                            // Price + freeze toggle + arrows
                            if chain_price > 0.0 {
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new(format!("${:.2}", chain_price)).monospace().size(14.0).color(egui::Color32::from_rgb(220, 220, 230)));
                                ui.add_space(4.0);
                                // Freeze toggle
                                let freeze_icon = if watchlist.chain_frozen { Icon::PAUSE } else { Icon::PLAY };
                                let freeze_color = if watchlist.chain_frozen { t.accent } else { t.dim.gamma_multiply(0.4) };
                                if ui.add(egui::Button::new(egui::RichText::new(freeze_icon).size(10.0).color(freeze_color))
                                    .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(14.0, 14.0))).clicked() {
                                    watchlist.chain_frozen = !watchlist.chain_frozen;
                                    if !watchlist.chain_frozen { watchlist.chain_center_offset = 0; }
                                }
                                // Up/down arrows (only when frozen)
                                if watchlist.chain_frozen {
                                    let max_offset = 50i32;
                                    let needs_refetch = |wl: &Watchlist| -> bool {
                                        // Check if we're near the edge of loaded data
                                        let calls = &wl.chain_0dte.0;
                                        let puts = &wl.chain_0dte.1;
                                        let total = calls.len() + puts.len();
                                        let offset_abs = wl.chain_center_offset.unsigned_abs() as usize;
                                        total > 0 && offset_abs + wl.chain_num_strikes >= total / 2
                                    };
                                    if ui.add(egui::Button::new(egui::RichText::new(Icon::ARROW_FAT_UP).size(10.0).color(t.dim))
                                        .fill(color_alpha(t.toolbar_border, 15)).corner_radius(2.0).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                        watchlist.chain_center_offset = (watchlist.chain_center_offset + 1).min(max_offset);
                                    }
                                    if ui.add(egui::Button::new(egui::RichText::new(Icon::ARROW_FAT_DOWN).size(10.0).color(t.dim))
                                        .fill(color_alpha(t.toolbar_border, 15)).corner_radius(2.0).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                        watchlist.chain_center_offset = (watchlist.chain_center_offset - 1).max(-max_offset);
                                    }
                                    // Show current offset
                                    if watchlist.chain_center_offset != 0 {
                                        ui.label(egui::RichText::new(format!("{:+}", watchlist.chain_center_offset)).monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
                                    }
                                }
                            }
                            // Search — static immediate + ApexIB background
                            if sym_resp.changed() && !watchlist.chain_sym_input.is_empty() {
                                watchlist.search_results = ui_kit::symbols::search_symbols(&watchlist.chain_sym_input, 5)
                                    .iter().map(|s| (s.symbol.to_string(), s.name.to_string())).collect();
                                // Also fire ApexIB search in background
                                fetch_search_background(watchlist.chain_sym_input.clone(), "chain".to_string());
                            }
                            if sym_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !watchlist.chain_sym_input.is_empty() {
                                watchlist.chain_symbol = watchlist.chain_sym_input.trim().to_uppercase();
                                watchlist.chain_sym_input.clear();
                                watchlist.search_results.clear();
                                watchlist.chain_0dte = (vec![], vec![]);
                                watchlist.chain_loading = false; // reset loading on symbol change
                            }
                        });
                        // Search suggestions popup
                        if !watchlist.chain_sym_input.is_empty() && !watchlist.search_results.is_empty() {
                            egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(28, 28, 34)).corner_radius(4.0).show(ui, |ui| {
                                for (sym, name) in watchlist.search_results.clone() {
                                    if ui.add(egui::Button::new(egui::RichText::new(format!("{} {}", sym, name)).monospace().size(11.0).color(t.dim))
                                        .frame(false).min_size(egui::vec2(ui.available_width(), 20.0))).clicked() {
                                        watchlist.chain_symbol = sym;
                                        watchlist.chain_sym_input.clear();
                                        watchlist.search_results.clear();
                                        watchlist.chain_0dte = (vec![], vec![]);
                                        watchlist.chain_loading = false; // reset on symbol change
                                    }
                                }
                            });
                        }

                        ui.add_space(4.0);
                        // Separator before chain data
                        let sep_r = ui.available_rect_before_wrap();
                        ui.painter().line_segment(
                            [egui::pos2(sep_r.left(), ui.cursor().min.y), egui::pos2(sep_r.right(), ui.cursor().min.y)],
                            egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 40)));
                        ui.add_space(4.0);

                        // Loading indicator
                        if watchlist.chain_loading {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(egui::RichText::new("Loading chain...").monospace().size(10.0).color(t.dim));
                            });
                        }

                        // ── Column layout ──
                        // Each data column needs space for ~8 chars of monospace 10px (~6.5px each = ~52px)
                        // Plus 8px gap between columns
                        let full_w = ui.available_width();
                        let gap = 8.0;
                        let col_chk = 14.0;
                        let col_stk = 44.0;
                        let col_bid = 56.0;
                        let col_ask = 56.0;
                        let col_oi  = 56.0;
                        // If panel is wide enough, expand proportionally
                        let used = col_chk + col_stk + col_bid + col_ask + col_oi + gap * 4.0;
                        let scale = if full_w > used { full_w / used } else { 1.0 };
                        let col_stk = col_stk * scale;
                        let col_bid = col_bid * scale;
                        let col_ask = col_ask * scale;
                        let col_oi = col_oi * scale;

                        // Column headers
                        ui.horizontal(|ui| {
                            ui.set_min_width(full_w);
                            ui.spacing_mut().item_spacing.x = gap;
                            let hdr_color = t.dim.gamma_multiply(0.4);
                            ui.add_space(col_chk);
                            ui.allocate_ui(egui::vec2(col_stk, 14.0), |ui| { dim_label(ui, "STK", hdr_color); });
                            ui.allocate_ui(egui::vec2(col_bid, 14.0), |ui| { dim_label(ui, "BID", hdr_color); });
                            ui.allocate_ui(egui::vec2(col_ask, 14.0), |ui| { dim_label(ui, "ASK", hdr_color); });
                            ui.allocate_ui(egui::vec2(col_oi, 14.0), |ui| { dim_label(ui, "OI", hdr_color); });
                        });

                        // ── Helper to render one option row ──
                        // Track clicked contract for opening chart (normal click)
                        let clicked_contract: std::cell::Cell<Option<(String, f32, bool, String)>> = std::cell::Cell::new(None);
                        // Track shift-clicked contract for adding to watchlist (select mode / shift+click)
                        let watchlist_add: std::cell::Cell<Option<(String, f32, bool, String, f32, f32)>> = std::cell::Cell::new(None);
                        let render_row = |ui: &mut egui::Ui, row: &OptionRow, is_call: bool, exp_label: &str, sym: &str, saved: &mut Vec<SavedOption>, select_mode: bool, w: f32| {
                            let is_saved = saved.iter().any(|s| s.contract == row.contract);
                            let color = if is_call { t.bull } else { t.bear };
                            let base_tint = if is_call { color_alpha(t.bull, 8) } else { color_alpha(t.bear, 8) };
                            let itm_bg = if row.itm { color.gamma_multiply(0.06) } else { base_tint };
                            let saved_bg = if is_saved { color_alpha(t.accent, 40) } else { itm_bg };

                            // Reserve a clickable rect for the whole row
                            let (rect, row_resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());

                            // Paint background
                            let bg = if row_resp.hovered() { color_alpha(t.toolbar_border, 50) } else { saved_bg };
                            ui.painter().rect_filled(rect, 0.0, bg);
                            if row_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

                            let mut x = rect.left();
                            let y_center = rect.center().y;
                            let painter = ui.painter();

                            // Check mark
                            if is_saved {
                                painter.text(egui::pos2(x + col_chk * 0.5, y_center), egui::Align2::CENTER_CENTER,
                                    Icon::CHECK, egui::FontId::proportional(12.0), t.accent);
                            }
                            x += col_chk + gap;

                            // Strike
                            painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                                &format!("{:.0}", row.strike), egui::FontId::monospace(14.0), egui::Color32::from_rgb(225, 225, 235));
                            x += col_stk + gap;

                            // Bid
                            painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                                &format!("{:.2}", row.bid), egui::FontId::monospace(14.0), color);
                            x += col_bid + gap;

                            // Ask
                            painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                                &format!("{:.2}", row.ask), egui::FontId::monospace(14.0), t.dim);
                            x += col_ask + gap;

                            // OI
                            let oi_str = if row.oi >= 1_000_000 { format!("{:.1}M", row.oi as f32 / 1_000_000.0) }
                                else if row.oi >= 1_000 { format!("{},{:03}", row.oi / 1000, row.oi % 1000) }
                                else { format!("{}", row.oi) };
                            painter.text(egui::pos2(x, y_center), egui::Align2::LEFT_CENTER,
                                &oi_str, egui::FontId::monospace(12.0), t.dim.gamma_multiply(0.5));

                            // Faint row separator
                            painter.line_segment(
                                [egui::pos2(rect.left() + 4.0, rect.bottom() - 0.5), egui::pos2(rect.right() - 4.0, rect.bottom() - 0.5)],
                                egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 30)));

                            // Click handling
                            if row_resp.clicked() {
                                if select_mode || ui.input(|i| i.modifiers.shift) {
                                    if is_saved { saved.retain(|s| s.contract != row.contract); }
                                    else { saved.push(SavedOption { contract: row.contract.clone(), symbol: sym.into(), strike: row.strike, is_call, expiry: exp_label.into(), last: row.last }); }
                                    watchlist_add.set(Some((sym.into(), row.strike, is_call, exp_label.into(), row.bid, row.ask)));
                                } else {
                                    clicked_contract.set(Some((sym.into(), row.strike, is_call, exp_label.into())));
                                }
                            }
                        };

                        // ── Helper to render one expiry block ──
                        let chain_frozen = watchlist.chain_frozen;
                        let chain_center_offset = watchlist.chain_center_offset;
                        let num_strikes = watchlist.chain_num_strikes;

                        let render_block = |ui: &mut egui::Ui, dte: i32, calls: &[OptionRow], puts: &[OptionRow], sym: &str, price: f32, saved: &mut Vec<SavedOption>, select_mode: bool, w: f32| {
                            let exp_label = format!("{}DTE", dte);
                            let date_str = if dte == 0 {
                                "Today".to_string()
                            } else {
                                let (_, m, d) = trading_date(dte);
                                format!("{} {}", trading_month_name(m), d)
                            };
                            // Expiry header
                            ui.horizontal(|ui| {
                                ui.set_min_width(w);
                                ui.label(egui::RichText::new(&exp_label).monospace().size(12.0).strong().color(t.accent));
                                ui.label(egui::RichText::new(&date_str).monospace().size(11.0).color(t.dim.gamma_multiply(0.6)));
                            });
                            ui.add_space(2.0);

                            // Collect all unique strikes from calls + puts, sorted ascending
                            let mut all_strikes: Vec<f32> = calls.iter().chain(puts.iter())
                                .map(|r| r.strike).collect();
                            all_strikes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            all_strikes.dedup();

                            // Find the ATM index (closest strike to price)
                            let atm_idx = all_strikes.iter().enumerate()
                                .min_by(|(_, a), (_, b)| ((**a - price).abs()).partial_cmp(&((**b - price).abs())).unwrap_or(std::cmp::Ordering::Equal))
                                .map(|(i, _)| i).unwrap_or(0);

                            // The offset shifts the center. The price badge always shows real price.
                            // We select num_strikes above the shifted center and num_strikes below.
                            let shifted_center_idx = (atm_idx as i32 + chain_center_offset).clamp(0, all_strikes.len() as i32 - 1) as usize;
                            let start = shifted_center_idx.saturating_sub(num_strikes);
                            let end = (shifted_center_idx + num_strikes).min(all_strikes.len());
                            let visible_strikes: Vec<f32> = all_strikes[start..end].to_vec();
                            // Use shifted center for calls/puts split (not actual price)
                            let split_price = all_strikes[shifted_center_idx];

                            // Split: calls = visible strikes above split, puts = visible strikes at/below split
                            let sorted_calls: Vec<&OptionRow> = {
                                let mut v: Vec<&OptionRow> = calls.iter()
                                    .filter(|r| visible_strikes.contains(&r.strike) && r.strike > split_price)
                                    .collect();
                                v.sort_by(|a, b| b.strike.partial_cmp(&a.strike).unwrap_or(std::cmp::Ordering::Equal));
                                v
                            };
                            let sorted_puts: Vec<&OptionRow> = {
                                let mut v: Vec<&OptionRow> = puts.iter()
                                    .filter(|r| visible_strikes.contains(&r.strike) && r.strike <= split_price)
                                    .collect();
                                v.sort_by(|a, b| b.strike.partial_cmp(&a.strike).unwrap_or(std::cmp::Ordering::Equal));
                                v
                            };

                            // Calls (OTM at top, ATM at bottom)
                            for row in &sorted_calls { render_row(ui, row, true, &exp_label, sym, saved, select_mode, w); }

                            // ── ATM price badge divider ──
                            ui.add_space(3.0);
                            {
                                let r = ui.available_rect_before_wrap();
                                let y = ui.cursor().min.y;
                                let badge_w = 80.0;
                                let center_x = r.left() + r.width() / 2.0;
                                // Lines on either side of the badge
                                ui.painter().line_segment(
                                    [egui::pos2(r.left() + 4.0, y + 10.0), egui::pos2(center_x - badge_w / 2.0 - 4.0, y + 10.0)],
                                    egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80)));
                                ui.painter().line_segment(
                                    [egui::pos2(center_x + badge_w / 2.0 + 4.0, y + 10.0), egui::pos2(r.right() - 4.0, y + 10.0)],
                                    egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80)));
                                // Badge background
                                let badge_rect = egui::Rect::from_center_size(egui::pos2(center_x, y + 10.0), egui::vec2(badge_w, 18.0));
                                ui.painter().rect_filled(badge_rect, 9.0, color_alpha(t.toolbar_border, 40));
                                ui.painter().rect_stroke(badge_rect, 9.0, egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 80)), egui::StrokeKind::Outside);
                                // Price text
                                let badge_text = if chain_frozen && chain_center_offset != 0 {
                                    format!("${:.2} ({:+})", price, chain_center_offset)
                                } else {
                                    format!("${:.2}", price)
                                };
                                ui.painter().text(badge_rect.center(), egui::Align2::CENTER_CENTER,
                                    &badge_text, egui::FontId::monospace(11.0),
                                    egui::Color32::from_rgb(220, 220, 230));
                            }
                            ui.add_space(22.0);

                            // Puts (ATM at top, OTM at bottom)
                            for row in &sorted_puts { render_row(ui, row, false, &exp_label, sym, saved, select_mode, w); }
                            ui.add_space(4.0);
                        };

                        // ── Scroll area with two expiry blocks ──
                        let scroll_w = ui.available_width();
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_min_width(scroll_w);
                            let sym = watchlist.chain_symbol.clone();
                            let sel = watchlist.chain_select_mode;
                            let calls_0 = watchlist.chain_0dte.0.clone();
                            let puts_0 = watchlist.chain_0dte.1.clone();
                            let calls_f = watchlist.chain_far.0.clone();
                            let puts_f = watchlist.chain_far.1.clone();
                            let far_dte = watchlist.chain_far_dte;

                            render_block(ui, 0, &calls_0, &puts_0, &sym, chain_price, &mut watchlist.saved_options, sel, scroll_w);
                            ui.add_space(4.0);
                            // DTE separator
                            let sep_r = ui.available_rect_before_wrap();
                            ui.painter().line_segment(
                                [egui::pos2(sep_r.left() + 4.0, ui.cursor().min.y), egui::pos2(sep_r.right() - 4.0, ui.cursor().min.y)],
                                egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 50)));
                            ui.add_space(4.0);
                            render_block(ui, far_dte, &calls_f, &puts_f, &sym, chain_price, &mut watchlist.saved_options, sel, scroll_w);
                        });
                        // Normal click: just open option chart (no watchlist add)
                        if let Some(info) = clicked_contract.take() {
                            open_option_chart = Some(info);
                        }
                        // Select mode / shift+click: add to watchlist + persist
                        if let Some((ref sym, strike, is_call, ref expiry, bid, ask)) = watchlist_add.take() {
                            watchlist.add_option_to_watchlist(sym, strike, is_call, expiry, bid, ask);
                            watchlist.persist();
                        }
                    }

                }

                // ── Handle option chart opening (from any tab) ──
                // Delegate to deferred handler which always replaces active pane
                if let Some(info) = open_option_chart {
                    watchlist.pending_opt_chart = Some(info);
                }
            });
    }

    // ── Orders / Positions / Alerts side panel (left of watchlist) ─────────────
    if watchlist.orders_panel_open {
        egui::SidePanel::right("orders_panel")
            .default_width(270.0)
            .min_width(220.0)
            .max_width(350.0)
            .frame(egui::Frame::NONE.fill(t.toolbar_bg)
                .inner_margin(egui::Margin { left: 8, right: 8, top: 8, bottom: 6 })
                .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80))))
            .show(ctx, |ui| {
                // ── Panel close button ──
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("BOOK").monospace().size(11.0).strong().color(t.accent));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if close_button(ui, t.dim) { watchlist.orders_panel_open = false; }
                    });
                });
                ui.add_space(4.0);

                // ══════════════════════════════════════════════════════
                // ── POSITIONS SECTION (top half of book) ──
                // ══════════════════════════════════════════════════════
                {
                    let (ib_positions, ib_orders) = read_account_data().map(|(_, p, o)| (p, o)).unwrap_or_default();
                    let has_positions = !ib_positions.is_empty();

                    // Header + Close All
                    ui.horizontal(|ui| {
                        section_label(ui, "POSITIONS", t.accent);
                        if has_positions {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let del_color = t.bear;
                                if ui.add(egui::Button::new(egui::RichText::new("Close All")
                                    .monospace().size(8.0).color(del_color))
                                    .fill(color_alpha(del_color, 15)).corner_radius(2.0)
                                    .stroke(egui::Stroke::new(0.5, color_alpha(del_color, 50)))
                                    .min_size(egui::vec2(0.0, 16.0))).clicked() {
                                    // Fire close-all via ApexIB
                                    std::thread::spawn(|| {
                                        let _ = reqwest::blocking::Client::new()
                                            .post(format!("{}/risk/flatten", APEXIB_URL))
                                            .timeout(std::time::Duration::from_secs(5))
                                            .send();
                                    });
                                }
                            });
                        }
                    });
                    ui.add_space(4.0);

                    if has_positions {
                        let mut total_pnl: f64 = 0.0;
                        egui::ScrollArea::vertical().id_salt("positions_scroll").max_height(ui.available_height() * 0.45).show(ui, |ui| {
                            for pos in &ib_positions {
                                total_pnl += pos.unrealized_pnl;
                                let pnl_color = if pos.unrealized_pnl >= 0.0 { t.bull } else { t.bear };
                                order_card(ui, pnl_color, color_alpha(t.toolbar_border, 10), |ui| {
                                    // Row 1: symbol, qty@price, close buttons
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(&pos.symbol).monospace().size(10.0).strong()
                                            .color(egui::Color32::from_rgb(220,220,230)));
                                        ui.label(egui::RichText::new(format!("{}@{:.2}", pos.qty, pos.avg_price))
                                            .monospace().size(9.0).color(t.dim.gamma_multiply(0.6)));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Close button
                                            let close_color = t.bear;
                                            if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(close_color))
                                                .fill(color_alpha(close_color, 12)).corner_radius(2.0)
                                                .min_size(egui::vec2(18.0, 16.0))).clicked() {
                                                // Close full position via ApexIB
                                                let sym = pos.symbol.clone();
                                                let qty = pos.qty;
                                                let con_id = pos.con_id;
                                                std::thread::spawn(move || {
                                                    let side = if qty > 0 { "SELL" } else { "BUY" };
                                                    let _ = reqwest::blocking::Client::new()
                                                        .post(format!("{}/orders", APEXIB_URL))
                                                        .json(&serde_json::json!({
                                                            "conId": con_id, "side": side,
                                                            "quantity": qty.unsigned_abs(),
                                                            "orderType": "market"
                                                        }))
                                                        .timeout(std::time::Duration::from_secs(5))
                                                        .send();
                                                });
                                            }
                                            // Close half button
                                            if pos.qty.abs() > 1 {
                                                if ui.add(egui::Button::new(egui::RichText::new("\u{00BD}").size(9.0).color(t.dim))
                                                    .fill(color_alpha(t.toolbar_border, 15)).corner_radius(2.0)
                                                    .min_size(egui::vec2(18.0, 16.0))).clicked() {
                                                    let sym = pos.symbol.clone();
                                                    let half = (pos.qty.abs() / 2).max(1);
                                                    let con_id = pos.con_id;
                                                    let qty = pos.qty;
                                                    std::thread::spawn(move || {
                                                        let side = if qty > 0 { "SELL" } else { "BUY" };
                                                        let _ = reqwest::blocking::Client::new()
                                                            .post(format!("{}/orders", APEXIB_URL))
                                                            .json(&serde_json::json!({
                                                                "conId": con_id, "side": side,
                                                                "quantity": half,
                                                                "orderType": "market"
                                                            }))
                                                            .timeout(std::time::Duration::from_secs(5))
                                                            .send();
                                                    });
                                                }
                                            }
                                        });
                                    });
                                    // Row 2: P&L + market value
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(format!("{:+.2}", pos.unrealized_pnl))
                                            .monospace().size(12.0).strong().color(pnl_color));
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(format!("({:+.1}%)", pos.pnl_pct()))
                                            .monospace().size(9.0).color(pnl_color));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new(format!("${:.0}", pos.market_value))
                                                .monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                                        });
                                    });
                                });
                            }
                        });
                        // Total P&L row
                        let total_color = if total_pnl >= 0.0 { t.bull } else { t.bear };
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Total P&L").monospace().size(9.0).color(t.dim));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(format!("{:+.2}", total_pnl)).monospace().size(11.0).strong().color(total_color));
                            });
                        });
                    } else {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("No open positions").monospace().size(10.0).color(t.dim.gamma_multiply(0.4)));
                        ui.add_space(8.0);
                    }

                    ui.add_space(4.0);
                }

                // ══════════════════════════════════════════════════════
                // ── THICK DIVIDER between positions and orders ──
                // ══════════════════════════════════════════════════════
                ui.add_space(4.0);
                {
                    let r = ui.available_rect_before_wrap();
                    let y = ui.cursor().min.y;
                    // 2px solid line like sidebar border
                    ui.painter().rect_filled(
                        egui::Rect::from_min_max(egui::pos2(r.left(), y), egui::pos2(r.right(), y + 2.0)),
                        0.0, color_alpha(t.toolbar_border, 120));
                    ui.add_space(6.0);
                }

                // ══════════════════════════════════════════════════════
                // ── ORDERS SECTION (bottom half of book) ──
                // ══════════════════════════════════════════════════════

                // Orders header + action bar
                ui.horizontal(|ui| {
                    section_label(ui, "ORDERS", t.accent);
                    let draft_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft).count()).sum();
                    let active_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).count()).sum();
                    if active_count > 0 || draft_count > 0 {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(format!("{}d {}a", draft_count, active_count - draft_count))
                            .monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let draft_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft).count()).sum();
                    let active_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).count()).sum();
                    let history_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Executed || o.status == OrderStatus::Cancelled).count()).sum();
                    if action_btn(ui, &format!("Place All ({})", draft_count), t.accent, draft_count > 0) {
                        for pane in panes.iter_mut() {
                            for o in &mut pane.orders { if o.status == OrderStatus::Draft { o.status = OrderStatus::Placed; } }
                        }
                    }
                    if action_btn(ui, "Cancel All", t.bear, active_count > 0) {
                        for pane in panes.iter_mut() {
                            for o in &mut pane.orders {
                                if o.status == OrderStatus::Draft || o.status == OrderStatus::Placed { o.status = OrderStatus::Cancelled; }
                            }
                        }
                    }
                    if action_btn(ui, "Clear", t.dim, history_count > 0) {
                        for pane in panes.iter_mut() {
                            pane.orders.retain(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed);
                        }
                    }
                });
                ui.add_space(4.0);

                // ── Group selection bar ──
                let sel_count = watchlist.selected_order_ids.len();
                if sel_count > 0 {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(egui::RichText::new(format!("{} selected", sel_count)).monospace().size(9.0).strong().color(t.accent));
                        action_btn(ui, "Place", t.accent, true).then(|| {
                            for (pi, oid) in &watchlist.selected_order_ids {
                                if let Some(o) = panes.get_mut(*pi).and_then(|p| p.orders.iter_mut().find(|o| o.id == *oid)) {
                                    if o.status == OrderStatus::Draft { o.status = OrderStatus::Placed; }
                                    if let Some(pid) = o.pair_id {
                                        if let Some(p) = panes.get_mut(*pi).and_then(|p| p.orders.iter_mut().find(|o| o.id == pid)) {
                                            if p.status == OrderStatus::Draft { p.status = OrderStatus::Placed; }
                                        }
                                    }
                                }
                            }
                            watchlist.selected_order_ids.clear();
                        });
                        action_btn(ui, "Cancel", t.bear, true).then(|| {
                            for (pi, oid) in &watchlist.selected_order_ids {
                                if *pi < panes.len() { cancel_order_with_pair(&mut panes[*pi].orders, *oid); }
                            }
                            watchlist.selected_order_ids.clear();
                        });
                        if ui.add(egui::Button::new(egui::RichText::new("Deselect").monospace().size(8.0).color(t.dim)).frame(false)).clicked() {
                            watchlist.selected_order_ids.clear();
                        }
                    });
                    ui.add_space(4.0);
                }

                // ── Select all toggle ──
                {
                    let active_orders: Vec<(usize, u32)> = panes.iter().enumerate()
                        .flat_map(|(pi, p)| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).map(move |o| (pi, o.id)))
                        .collect();
                    let all_selected = !active_orders.is_empty() && active_orders.iter().all(|(pi, oid)| watchlist.selected_order_ids.iter().any(|(p, id)| p == pi && id == oid));
                    if !active_orders.is_empty() {
                        ui.horizontal(|ui| {
                            let check_icon = if all_selected { Icon::CHECK_SQUARE } else { Icon::SQUARE_EMPTY };
                            let check_color = if all_selected { t.accent } else { t.dim.gamma_multiply(0.4) };
                            if ui.add(egui::Button::new(egui::RichText::new(check_icon).size(11.0).color(check_color))
                                .frame(false).min_size(egui::vec2(14.0, 14.0))).clicked() {
                                if all_selected {
                                    watchlist.selected_order_ids.clear();
                                } else {
                                    watchlist.selected_order_ids = active_orders;
                                }
                            }
                            ui.label(egui::RichText::new("Select all").monospace().size(9.0).color(t.dim.gamma_multiply(0.6)));
                        });
                        ui.add_space(2.0);
                    }
                }

                // ── Order cards ──
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut cancel_order: Option<(usize, u32)> = None;
                    let mut toggle_select: Option<(usize, u32)> = None;

                    for (pi, pane) in panes.iter().enumerate() {
                        for order in &pane.orders {
                            let color = order.color(t);
                            let status_text = match order.status {
                                OrderStatus::Draft => "DRAFT", OrderStatus::Placed => "PLACED",
                                OrderStatus::Executed => "EXEC", OrderStatus::Cancelled => "CXL",
                            };
                            let status_color = match order.status {
                                OrderStatus::Draft => t.dim, OrderStatus::Placed => t.accent,
                                OrderStatus::Executed => t.bull, OrderStatus::Cancelled => t.bear,
                            };
                            let is_active = order.status == OrderStatus::Draft || order.status == OrderStatus::Placed;
                            let is_selected = watchlist.selected_order_ids.iter().any(|(p, id)| *p == pi && *id == order.id);
                            let card_bg = if is_selected { color_alpha(t.accent, 12) } else { color_alpha(t.toolbar_border, 15) };

                            let card_clicked = order_card(ui, color, card_bg, |ui| {
                                // Card header: checkbox + type + symbol + status + close
                                ui.horizontal(|ui| {
                                    // Selection checkbox (visual only — click handled by card)
                                    if is_active {
                                        let check_icon = if is_selected { Icon::CHECK_SQUARE } else { Icon::SQUARE_EMPTY };
                                        let check_color = if is_selected { t.accent } else { t.dim.gamma_multiply(0.4) };
                                        ui.label(egui::RichText::new(check_icon).size(11.0).color(check_color));
                                    }
                                    ui.label(egui::RichText::new(order.label()).monospace().size(10.0).strong().color(color));
                                    ui.label(egui::RichText::new(format!("{} {}", &pane.symbol, &pane.timeframe))
                                        .monospace().size(9.0).color(egui::Color32::from_rgba_unmultiplied(200, 200, 210, 180)));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if is_active {
                                            if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.5)))
                                                .frame(false)).clicked() {
                                                cancel_order = Some((pi, order.id));
                                            }
                                        }
                                        status_badge(ui, status_text, status_color);
                                    });
                                });

                                // Card body: price | qty | notional
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("{:.2}", order.price)).monospace().size(13.0).strong().color(color));
                                    ui.add_space(6.0);
                                    ui.label(egui::RichText::new(format!("\u{00D7}{}", order.qty)).monospace().size(10.0).color(t.dim.gamma_multiply(0.6)));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(fmt_notional(order.notional())).monospace().size(9.0).color(t.dim.gamma_multiply(0.4)));
                                    });
                                });
                            });

                            // Toggle selection on card click (for active orders)
                            if card_clicked && is_active {
                                toggle_select = Some((pi, order.id));
                            }
                        }
                    }

                    if let Some((pi, oid)) = cancel_order {
                        cancel_order_with_pair(&mut panes[pi].orders, oid);
                    }
                    if let Some((pi, oid)) = toggle_select {
                        let already = watchlist.selected_order_ids.iter().any(|(p, id)| *p == pi && *id == oid);
                        if already {
                            watchlist.selected_order_ids.retain(|(p, id)| !(*p == pi && *id == oid));
                        } else {
                            watchlist.selected_order_ids.push((pi, oid));
                        }
                    }

                    // Positions are now shown above orders via ApexIB live data

                    // ── IB Order History ──
                    let ib_orders = read_account_data().map(|(_, _, o)| o).unwrap_or_default();
                    if !ib_orders.is_empty() {
                        ui.add_space(4.0);
                        separator(ui, color_alpha(t.toolbar_border, 40));
                        ui.add_space(4.0);
                        section_label(ui, "IB ORDERS", t.accent);
                        ui.add_space(4.0);
                        for o in &ib_orders {
                            let is_fill = o.status == "filled";
                            let is_cancel = o.status == "cancelled";
                            let side_color = if o.side == "BUY" { t.bull } else { t.bear };
                            let status_color = if is_fill { t.bull } else if is_cancel { t.dim.gamma_multiply(0.4) } else { t.accent };
                            let opt_label = if !o.option_type.is_empty() { format!(" {:.0}{}", o.strike, o.option_type) } else { String::new() };
                            order_card(ui, side_color, color_alpha(t.toolbar_border, if is_cancel { 5 } else { 10 }), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&o.side).monospace().size(9.0).strong().color(side_color));
                                    ui.label(egui::RichText::new(format!("{}{}", o.symbol, opt_label)).monospace().size(10.0).strong()
                                        .color(egui::Color32::from_rgb(220, 220, 230)));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        status_badge(ui, &o.status.to_uppercase(), status_color);
                                    });
                                });
                                ui.horizontal(|ui| {
                                    if o.avg_fill_price > 0.0 {
                                        ui.label(egui::RichText::new(format!("{:.2}", o.avg_fill_price)).monospace().size(11.0).strong().color(side_color));
                                    } else if o.limit_price > 0.0 {
                                        ui.label(egui::RichText::new(format!("{:.2}", o.limit_price)).monospace().size(11.0).color(t.dim));
                                    }
                                    ui.label(egui::RichText::new(format!("\u{00D7}{}", o.qty)).monospace().size(9.0).color(t.dim.gamma_multiply(0.6)));
                                    if o.filled_qty > 0 && o.filled_qty != o.qty {
                                        ui.label(egui::RichText::new(format!("filled {}", o.filled_qty)).monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                                    }
                                    let notional = if o.avg_fill_price > 0.0 { o.avg_fill_price * o.qty as f64 } else { o.limit_price * o.qty as f64 };
                                    if notional > 0.0 {
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new(format!("${:.0}", notional)).monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                                        });
                                    }
                                });
                            });
                        }
                    }

                    // ── Alerts ──
                    if !watchlist.alerts.is_empty() {
                        ui.add_space(4.0);
                        separator(ui, color_alpha(t.toolbar_border, 40));
                        ui.add_space(4.0);
                        section_label(ui, "ALERTS", t.dim);
                        ui.add_space(4.0);
                        let mut remove_alert: Option<u32> = None;
                        for alert in &watchlist.alerts {
                            let dir = if alert.above { "\u{2191}" } else { "\u{2193}" };
                            let alert_color = if alert.triggered { t.accent } else { t.dim };
                            order_card(ui, alert_color, color_alpha(t.toolbar_border, 10), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong().color(egui::Color32::from_rgb(220,220,230)));
                                    ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price)).monospace().size(10.0).color(alert_color));
                                    if alert.triggered {
                                        status_badge(ui, "TRIGGERED", t.accent);
                                    }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.5))).frame(false)).clicked() {
                                            remove_alert = Some(alert.id);
                                        }
                                    });
                                });
                                if !alert.message.is_empty() {
                                    ui.label(egui::RichText::new(&alert.message).monospace().size(9.0).color(t.dim.gamma_multiply(0.6)));
                                }
                            });
                        }
                        if let Some(id) = remove_alert { watchlist.alerts.retain(|a| a.id != id); }
                    }
                });
            });
    }

    // NOTE: Order execution is NOT simulated locally — fills come from the brokerage API.
    // The chart only displays order levels; execution status changes are signaled externally.

    // Update position current prices from chart data
    for pos in &mut watchlist.positions {
        if let Some(pane) = panes.iter().find(|p| p.symbol == pos.symbol) {
            if let Some(bar) = pane.bars.last() {
                pos.current_price = bar.close;
            }
        }
    }

    // ── Alert checking — run every frame, check if any alert prices were crossed ──
    {
        let active_prices: Vec<(String, f32)> = panes.iter()
            .filter_map(|p| p.bars.last().map(|b| (p.symbol.clone(), b.close)))
            .collect();
        for alert in &mut watchlist.alerts {
            if alert.triggered { continue; }
            if let Some((_, price)) = active_prices.iter().find(|(s, _)| *s == alert.symbol) {
                if (alert.above && *price >= alert.price) || (!alert.above && *price <= alert.price) {
                    alert.triggered = true;
                }
            }
        }
    }

    span_begin("chart_panes");
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.bg)).show(ctx, |ui| {
        let full_rect = ui.available_rect_before_wrap();
        let visible_count = layout.max_panes().min(panes.len());
        let pane_rects = layout.pane_rects(full_rect, visible_count);

        for pane_idx in 0..visible_count {
        let pane_rect = pane_rects[pane_idx];
        let chart = &mut panes[pane_idx];
        let is_active = pane_idx == *active_pane;
        let t = &THEMES[chart.theme_idx];
        let n = chart.bars.len();

        // Draw pane border (highlight active pane)
        if visible_count > 1 {
            let border_color = if is_active { t.bull.gamma_multiply(0.8) } else { t.dim.gamma_multiply(0.3) };
            let border_width = if is_active { 1.5 } else { 0.5 };
            ui.painter().rect_stroke(pane_rect, 0.0, egui::Stroke::new(border_width, border_color), egui::StrokeKind::Inside);
        }

        // Pane header (symbol + timeframe + per-pane selector) for multi-pane layouts
        let pane_top_offset = if visible_count > 1 { 18.0 } else { 0.0 };
        if visible_count > 1 {
            let header_rect = egui::Rect::from_min_size(pane_rect.min, egui::vec2(pane_rect.width(), pane_top_offset));
            ui.allocate_rect(header_rect, egui::Sense::hover()); // reserve space
            let header_painter = ui.painter_at(header_rect);
            header_painter.rect_filled(header_rect, 0.0, t.bg.gamma_multiply(1.2));

            // Clickable symbol — opens this pane's picker
            let sym_rect = egui::Rect::from_min_size(
                egui::pos2(header_rect.left() + 2.0, header_rect.top()),
                egui::vec2(header_rect.width() * 0.5, pane_top_offset),
            );
            let sym_resp = ui.allocate_rect(sym_rect, egui::Sense::click());
            let label_color = if is_active { t.bull } else { egui::Color32::from_rgb(180,180,190) };
            header_painter.text(
                egui::pos2(header_rect.left() + 6.0, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!("{} {}", chart.symbol, chart.timeframe),
                egui::FontId::monospace(10.0),
                label_color,
            );
            if sym_resp.clicked() {
                *active_pane = pane_idx;
                chart.picker_open = !chart.picker_open;
                chart.picker_query.clear();
                chart.picker_results.clear();
                chart.picker_last_query.clear();
                chart.picker_pos = egui::pos2(sym_rect.left(), sym_rect.bottom());
            }

            // Rest of header — click to activate pane
            let rest_rect = egui::Rect::from_min_size(
                egui::pos2(header_rect.left() + header_rect.width() * 0.5, header_rect.top()),
                egui::vec2(header_rect.width() * 0.5, pane_top_offset),
            );
            let rest_resp = ui.allocate_rect(rest_rect, egui::Sense::click());
            if rest_resp.clicked() { *active_pane = pane_idx; }
        }

        let rect = egui::Rect::from_min_size(
            egui::pos2(pane_rect.left(), pane_rect.top() + pane_top_offset),
            egui::vec2(pane_rect.width(), pane_rect.height() - pane_top_offset),
        );
        let (w,h) = (rect.width(), rect.height());
        let (pr,pt,pb) = (42.0_f32, 4.0_f32, 0.0_f32);
        // Reserve space for oscillator sub-panel if any oscillator indicators are active
        let has_oscillators = chart.show_oscillators && chart.indicators.iter().any(|i| i.visible && i.kind.category() == IndicatorCategory::Oscillator);
        let osc_h = if has_oscillators { (h * 0.22).min(120.0) } else { 0.0 };
        let (cw,ch) = (w-pr, h-pt-pb-osc_h);
        if n==0 || cw<=0.0 || ch<=0.0 { continue; }

        // Only set cursors for the pane the pointer is actually over
        let pointer_in_pane = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| pane_rect.contains(p));

        let (min_p,max_p) = chart.price_range();
        let total = chart.vc+8;
        let bs = cw/total as f32;
        let vs = chart.vs;
        let end = ((vs as u32)+chart.vc).min(n as u32);
        let frac = vs-vs.floor();
        let off = frac*bs;

        let py = |p:f32| rect.top()+pt+(max_p-p)/(max_p-min_p)*ch;
        let py_inv = |y:f32| max_p - (y - rect.top() - pt) / ch * (max_p - min_p);
        let bx = |i:f32| rect.left()+(i-vs)*bs+bs*0.5-off;
        let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
        let painter = ui.painter_at(rect);

        // Grid + price labels
        let rng=max_p-min_p; let rs=rng/8.0; let mg=10.0_f32.powf(rs.log10().floor());
        let ns=[1.0,2.0,2.5,5.0,10.0]; let step=ns.iter().map(|&s|s*mg).find(|&s|s>=rs).unwrap_or(rs);
        let mut p=(min_p/step).ceil()*step;
        while p<=max_p { let y=py(p);
            painter.line_segment([egui::pos2(rect.left(),y),egui::pos2(rect.left()+cw,y)], egui::Stroke::new(0.5,t.dim.gamma_multiply(0.3)));
            let d=if p>=10.0{2}else{4};
            chart.fmt_buf.clear(); let _ = write!(chart.fmt_buf, "{:.1$}", p, d);
            painter.text(egui::pos2(rect.left()+cw+3.0,y),egui::Align2::LEFT_CENTER,&chart.fmt_buf,egui::FontId::monospace(8.5),t.dim);
            p+=step;
        }

        // Time labels on bottom axis
        if !chart.timestamps.is_empty() && end > vs as u32 {
            let candle_sec = if chart.timestamps.len() > 1 { (chart.timestamps[1] - chart.timestamps[0]).max(60) } else { 86400 };
            let nice_int: &[i64] = &[60,300,900,1800,3600,7200,14400,28800,86400,172800,604800,2592000];
            let min_label_px = 70.0;
            let bars_per_label = (min_label_px / bs).ceil() as i64;
            let min_interval = bars_per_label * candle_sec;
            let time_interval = nice_int.iter().copied().find(|&i| i >= min_interval).unwrap_or(86400);

            if let Some(&first_ts) = chart.timestamps.get(vs as usize) {
                let first_label = ((first_ts / time_interval) + 1) * time_interval;
                let mut ti = first_label;
                let last_ts = chart.timestamps.get((end-1) as usize).copied().unwrap_or(first_ts);
                while ti <= last_ts {
                    let bar_idx = chart.timestamps.partition_point(|&ts| ts < ti);
                    if bar_idx >= vs as usize && bar_idx < end as usize {
                        let x = bx(bar_idx as f32);
                        if x > rect.left()+20.0 && x < rect.left()+cw-40.0 {
                            chart.fmt_buf.clear();
                            if time_interval >= 86400 {
                                let days = (ti / 86400) as i32; let y2k = days - 10957;
                                let month = ((y2k % 365) / 30 + 1).min(12).max(1);
                                let day = ((y2k % 365) % 30 + 1).min(31).max(1);
                                let _ = write!(chart.fmt_buf, "{:02}/{:02}", month, day);
                            } else {
                                let h = ((ti % 86400) / 3600) as u32;
                                let m = ((ti % 3600) / 60) as u32;
                                let _ = write!(chart.fmt_buf, "{:02}:{:02}", h, m);
                            };
                            let y = rect.top() + pt + ch - 10.0;
                            painter.text(egui::pos2(x, y), egui::Align2::CENTER_BOTTOM, &chart.fmt_buf, egui::FontId::monospace(8.0), t.dim.gamma_multiply(0.6));
                        }
                    }
                    ti += time_interval;
                }
            }
        }

        // Volume + candles + indicators + oscillators + drawings
        span_begin("pane_render");

        // Volume bars (gated by show_volume)
        if chart.show_volume {
            let mut mv: f32 = 0.0;
            for i in (vs as u32)..end { if let Some(b) = chart.bars.get(i as usize) { mv = mv.max(b.volume); } }
            if mv == 0.0 { mv = 1.0; }
            for i in (vs as u32)..end { if let Some(b) = chart.bars.get(i as usize) {
                let x = bx(i as f32); let vh = (b.volume / mv) * ch * 0.2;
                let c = if b.close >= b.open { t.bull.gamma_multiply(0.2) } else { t.bear.gamma_multiply(0.2) };
                let bw = (bs * 0.4).max(1.0);
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(x-bw, rect.top()+pt+ch-vh), egui::pos2(x+bw, rect.top()+pt+ch)), 0.0, c);
            }}
        }

        // Candles — no rounding (0.0) for fast tessellation
        for i in (vs as u32)..end { if let Some(b)=chart.bars.get(i as usize) {
            let x=bx(i as f32); let c=if b.close>=b.open{t.bull}else{t.bear};
            let bt=py(b.open.max(b.close)); let bb=py(b.open.min(b.close));
            let wt=py(b.high); let wb=py(b.low); let bw=(bs*0.35).max(1.0);
            painter.line_segment([egui::pos2(x,wt),egui::pos2(x,wb)],egui::Stroke::new(1.0,c));
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(x-bw,bt),egui::vec2(bw*2.0,(bb-bt).max(1.0))),0.0,c);
        }}

        // Indicators
        if !chart.hide_all_indicators {
            for ind in &chart.indicators {
                if !ind.visible { continue; }
                chart.indicator_pts_buf.clear();
                for i in (vs as u32)..end {
                    if let Some(&v) = ind.values.get(i as usize) {
                        if !v.is_nan() { chart.indicator_pts_buf.push(egui::pos2(bx(i as f32), py(v))); }
                    }
                }
                if chart.indicator_pts_buf.len() > 1 {
                    let color = hex_to_color(&ind.color, 1.0);
                    let stroke = egui::Stroke::new(ind.thickness, color);
                    match ind.line_style {
                        LineStyle::Solid => { painter.add(egui::Shape::line(chart.indicator_pts_buf.clone(), stroke)); }
                        LineStyle::Dashed | LineStyle::Dotted => {
                            let (dash, gap) = if ind.line_style == LineStyle::Dashed { (6.0, 3.0) } else { (2.0, 2.0) };
                            for w in chart.indicator_pts_buf.windows(2) {
                                let a = w[0]; let b = w[1];
                                let dir = b - a; let len = dir.length();
                                if len < 1.0 { continue; }
                                let norm = dir / len;
                                let mut d = 0.0;
                                while d < len {
                                    let p0 = a + norm * d;
                                    let p1 = a + norm * (d + dash).min(len);
                                    painter.line_segment([p0, p1], stroke);
                                    d += dash + gap;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Drawings (with selection highlight + endpoint handles)
        // Clamp helper — prevents extreme coordinates from causing massive tessellation allocations
        // ── Trigger level lines (options conditional orders on underlying) ──
        for tl in &chart.trigger_levels {
            let y = py(tl.trigger_price);
            if !y.is_finite() || y.abs() > 50000.0 { continue; }
            let is_buy = tl.side == OrderSide::Buy;
            let color = if is_buy { t.bull } else { t.bear };
            let alpha = if tl.submitted { 180 } else { 255 };
            let label = format!("{} {} {} {:.2} x{}", Icon::LIGHTNING,
                if is_buy { "BUY" } else { "SELL" }, tl.option_type, tl.trigger_price, tl.qty);
            let status = if tl.submitted { " LIVE" } else { " DRAFT" };
            // Dashed line
            dashed_line(&painter, egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y),
                egui::Stroke::new(1.5, color_alpha(color, alpha)), LineStyle::Dashed);
            // Label on the left
            painter.text(egui::pos2(rect.left() + 4.0, y - 12.0), egui::Align2::LEFT_BOTTOM,
                &label, egui::FontId::monospace(9.0), color_alpha(color, alpha));
            // Status badge on the right
            painter.text(egui::pos2(rect.left() + cw - 4.0, y - 12.0), egui::Align2::RIGHT_BOTTOM,
                status, egui::FontId::monospace(8.0), color_alpha(color, 120));
            // Y-axis price tag
            let tag_w = 54.0;
            let tag_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + cw, y - 8.0), egui::vec2(tag_w, 16.0));
            painter.rect_filled(tag_rect, 2.0, color_alpha(color, alpha));
            painter.text(tag_rect.center(), egui::Align2::CENTER_CENTER,
                &format!("{:.2}", tl.trigger_price), egui::FontId::monospace(9.0), egui::Color32::WHITE);
        }

        let clamp_pt = |p: egui::Pos2| -> egui::Pos2 {
            let margin = 10000.0;
            egui::pos2(p.x.clamp(-margin, margin), p.y.clamp(-margin, margin))
        };
        let in_bounds = |p: egui::Pos2| -> bool { p.x.is_finite() && p.y.is_finite() && p.x.abs() < 50000.0 && p.y.abs() < 50000.0 };

        for d in &chart.drawings {
            if chart.hide_all_drawings { break; }
            if chart.hidden_groups.contains(&d.group_id) { continue; }
            let is_sel = chart.selected_ids.contains(&d.id);
            let dc = hex_to_color(&d.color, d.opacity);
            let sc = egui::Stroke::new(if is_sel { d.thickness + 1.0 } else { d.thickness }, if is_sel { egui::Color32::WHITE } else { dc });
            let ls = d.line_style;
            match &d.kind {
                DrawingKind::HLine{price}=>{
                    let y=py(*price);
                    if y.is_finite() && y.abs() < 50000.0 {
                        dashed_line(&painter, egui::pos2(rect.left(),y), egui::pos2(rect.left()+cw,y), sc, ls);
                        if is_sel {
                            painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y), 4.0, egui::Color32::from_rgb(74,158,255));
                        }
                    }
                }
                DrawingKind::TrendLine{price0,time0,price1,time1}=>{
                    let p0=clamp_pt(egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)),py(*price0)));
                    let p1=clamp_pt(egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)),py(*price1)));
                    if in_bounds(p0) && in_bounds(p1) {
                        dashed_line(&painter, p0, p1, sc, ls);
                        if is_sel {
                            painter.circle_filled(p0, 5.0, egui::Color32::from_rgb(74,158,255));
                            painter.circle_stroke(p0, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                            painter.circle_filled(p1, 5.0, egui::Color32::from_rgb(74,158,255));
                            painter.circle_stroke(p1, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                        }
                    }
                }
                DrawingKind::HZone{price0,price1}=>{
                    let(y0,y1)=(py(*price0),py(*price1));
                    if y0.is_finite() && y1.is_finite() && y0.abs() < 50000.0 && y1.abs() < 50000.0 {
                        let fill = hex_to_color(&d.color, d.opacity * 0.1);
                        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(),y0.min(y1)),egui::pos2(rect.left()+cw,y0.max(y1))),0.0,fill);
                        dashed_line(&painter, egui::pos2(rect.left(),y0), egui::pos2(rect.left()+cw,y0), sc, ls);
                        dashed_line(&painter, egui::pos2(rect.left(),y1), egui::pos2(rect.left()+cw,y1), sc, ls);
                        if is_sel {
                            painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y0), 4.0, egui::Color32::from_rgb(74,158,255));
                            painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y1), 4.0, egui::Color32::from_rgb(74,158,255));
                        }
                    }
                }
                DrawingKind::BarMarker{time,price,up}=>{
                    let x=bx(SignalDrawing::time_to_bar(*time, &chart.timestamps)); let y=py(*price);
                    if !in_bounds(egui::pos2(x, y)) { continue; }
                    let dir = if *up { -1.0 } else { 1.0 };
                    let sz = 6.0;
                    let pts = vec![
                        egui::pos2(x, y + dir*2.0),
                        egui::pos2(x - sz, y + dir*(sz+4.0)),
                        egui::pos2(x + sz, y + dir*(sz+4.0)),
                    ];
                    painter.add(egui::Shape::convex_polygon(pts, dc, egui::Stroke::NONE));
                    if is_sel {
                        painter.circle_stroke(egui::pos2(x, y), 8.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                    }
                }
            }
        }

        // ── Oscillator sub-panel (RSI, MACD, Stochastic) ─────────────────────
        if has_oscillators && osc_h > 10.0 {
            let osc_top = rect.top() + pt + ch + 2.0;
            let osc_bottom = osc_top + osc_h - 4.0;
            let osc_height = osc_bottom - osc_top;

            // Separator line
            painter.line_segment([egui::pos2(rect.left(), osc_top - 1.0), egui::pos2(rect.left() + cw, osc_top - 1.0)],
                egui::Stroke::new(1.0, t.toolbar_border));

            for ind in &chart.indicators {
                if !ind.visible || ind.kind.category() != IndicatorCategory::Oscillator { continue; }
                let color = hex_to_color(&ind.color, 1.0);

                // Determine value range for this oscillator
                let (osc_min, osc_max) = match ind.kind {
                    IndicatorType::RSI => (0.0_f32, 100.0),
                    IndicatorType::Stochastic => (0.0, 100.0),
                    IndicatorType::MACD => {
                        let mut lo = f32::MAX; let mut hi = f32::MIN;
                        for i in (vs as u32)..end {
                            if let Some(&v) = ind.values.get(i as usize) { if !v.is_nan() { lo = lo.min(v); hi = hi.max(v); } }
                            if let Some(&v) = ind.histogram.get(i as usize) { if !v.is_nan() { lo = lo.min(v); hi = hi.max(v); } }
                        }
                        if lo >= hi { lo -= 1.0; hi += 1.0; }
                        let pad = (hi - lo) * 0.1;
                        (lo - pad, hi + pad)
                    }
                    _ => (0.0, 100.0),
                };

                let osc_y = |v: f32| -> f32 { osc_top + (osc_max - v) / (osc_max - osc_min) * osc_height };

                // Reference lines for RSI/Stochastic (30/70 or 20/80)
                if ind.kind == IndicatorType::RSI || ind.kind == IndicatorType::Stochastic {
                    let (low_ref, high_ref) = if ind.kind == IndicatorType::RSI { (30.0, 70.0) } else { (20.0, 80.0) };
                    for &level in &[low_ref, 50.0, high_ref] {
                        let y = osc_y(level);
                        painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)],
                            egui::Stroke::new(0.3, t.dim.gamma_multiply(0.3)));
                    }
                    // Overbought/oversold zones
                    painter.rect_filled(egui::Rect::from_min_max(
                        egui::pos2(rect.left(), osc_y(high_ref)), egui::pos2(rect.left() + cw, osc_y(osc_max))),
                        0.0, t.bear.gamma_multiply(0.04));
                    painter.rect_filled(egui::Rect::from_min_max(
                        egui::pos2(rect.left(), osc_y(osc_min)), egui::pos2(rect.left() + cw, osc_y(low_ref))),
                        0.0, t.bull.gamma_multiply(0.04));
                }

                // Zero line for MACD
                if ind.kind == IndicatorType::MACD {
                    let y = osc_y(0.0);
                    painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)],
                        egui::Stroke::new(0.5, t.dim.gamma_multiply(0.3)));
                }

                // MACD histogram bars
                if ind.kind == IndicatorType::MACD && !ind.histogram.is_empty() {
                    let zero_y = osc_y(0.0);
                    for i in (vs as u32)..end {
                        if let Some(&h) = ind.histogram.get(i as usize) {
                            if !h.is_nan() {
                                let x = bx(i as f32);
                                let y = osc_y(h);
                                let bw = (bs * 0.3).max(1.0);
                                let c = if h >= 0.0 { t.bull.gamma_multiply(0.4) } else { t.bear.gamma_multiply(0.4) };
                                painter.rect_filled(egui::Rect::from_min_max(
                                    egui::pos2(x - bw, y.min(zero_y)), egui::pos2(x + bw, y.max(zero_y))), 0.0, c);
                            }
                        }
                    }
                }

                // Primary line
                let mut pts = Vec::new();
                for i in (vs as u32)..end {
                    if let Some(&v) = ind.values.get(i as usize) {
                        if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), osc_y(v))); }
                    }
                }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(ind.thickness, color))); }

                // Secondary line (MACD signal, Stochastic %D)
                if !ind.values2.is_empty() {
                    let mut pts2 = Vec::new();
                    for i in (vs as u32)..end {
                        if let Some(&v) = ind.values2.get(i as usize) {
                            if !v.is_nan() { pts2.push(egui::pos2(bx(i as f32), osc_y(v))); }
                        }
                    }
                    if pts2.len() > 1 {
                        let c2 = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 140);
                        painter.add(egui::Shape::line(pts2, egui::Stroke::new(1.0, c2)));
                    }
                }

                // Divergence markers
                for i in (vs as u32)..end {
                    if let Some(&d) = ind.divergences.get(i as usize) {
                        if d != 0 {
                            let x = bx(i as f32);
                            if let Some(&v) = ind.values.get(i as usize) {
                                if !v.is_nan() {
                                    let y = osc_y(v);
                                    let div_color = if d > 0 { t.bull } else { t.bear };
                                    // Small triangle marker
                                    let dir = if d > 0 { -1.0 } else { 1.0 };
                                    painter.add(egui::Shape::convex_polygon(vec![
                                        egui::pos2(x, y + dir * 2.0),
                                        egui::pos2(x - 4.0, y + dir * 7.0),
                                        egui::pos2(x + 4.0, y + dir * 7.0),
                                    ], div_color, egui::Stroke::NONE));
                                }
                            }
                        }
                    }
                }

                // Clickable label — click to edit, shows [x] delete on hover
                let label_text = ind.display_name();
                let label_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 4.0, osc_top + 2.0),
                    egui::vec2(label_text.len() as f32 * 6.0 + 20.0, 14.0),
                );
                let label_hovered = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| label_rect.contains(p));
                let label_bg = if label_hovered { t.toolbar_border.gamma_multiply(0.5) } else { egui::Color32::TRANSPARENT };
                painter.rect_filled(label_rect, 2.0, label_bg);
                painter.text(egui::pos2(label_rect.left() + 2.0, label_rect.center().y), egui::Align2::LEFT_CENTER,
                    &label_text, egui::FontId::monospace(9.0), color.gamma_multiply(if label_hovered { 1.0 } else { 0.7 }));
                // [x] delete button at end of label
                if label_hovered {
                    let x_rect = egui::Rect::from_min_size(
                        egui::pos2(label_rect.right() - 12.0, label_rect.top()),
                        egui::vec2(12.0, 14.0),
                    );
                    painter.text(x_rect.center(), egui::Align2::CENTER_CENTER, Icon::X,
                        egui::FontId::proportional(8.0), t.bear);
                }
            }

            // Oscillator click interaction — allocate rect over the whole panel
            let osc_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), osc_top), egui::vec2(cw, osc_height));
            let osc_resp = ui.allocate_rect(osc_rect, egui::Sense::click());

            if osc_resp.clicked() {
                if let Some(pos) = osc_resp.interact_pointer_pos() {
                    // Check if clicked on a label's [x] delete button
                    let mut deleted_id: Option<u32> = None;
                    let mut label_y_offset = 0.0_f32;
                    for ind in &chart.indicators {
                        if !ind.visible || ind.kind.category() != IndicatorCategory::Oscillator { continue; }
                        let label_text = ind.display_name();
                        let label_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.left() + 4.0, osc_top + 2.0 + label_y_offset),
                            egui::vec2(label_text.len() as f32 * 6.0 + 20.0, 14.0),
                        );
                        let x_rect = egui::Rect::from_min_size(
                            egui::pos2(label_rect.right() - 12.0, label_rect.top()),
                            egui::vec2(12.0, 14.0),
                        );
                        if x_rect.contains(pos) {
                            deleted_id = Some(ind.id);
                            break;
                        }
                        if label_rect.contains(pos) {
                            chart.editing_indicator = Some(ind.id);
                            break;
                        }
                        label_y_offset += 16.0;
                    }
                    if let Some(id) = deleted_id {
                        chart.indicators.retain(|i| i.id != id);
                        chart.indicator_bar_count = 0;
                    }
                }
            }

            // Double-click on oscillator line to edit
            if osc_resp.double_clicked() {
                if let Some(pos) = osc_resp.interact_pointer_pos() {
                    for ind in &chart.indicators {
                        if !ind.visible || ind.kind.category() != IndicatorCategory::Oscillator { continue; }
                        // Check proximity to the oscillator's primary line
                        let (osc_min, osc_max) = match ind.kind {
                            IndicatorType::RSI | IndicatorType::Stochastic => (0.0_f32, 100.0),
                            _ => {
                                let mut lo = f32::MAX; let mut hi = f32::MIN;
                                for &v in &ind.values { if !v.is_nan() { lo = lo.min(v); hi = hi.max(v); } }
                                if lo >= hi { lo -= 1.0; hi += 1.0; }
                                let pad = (hi - lo) * 0.1; (lo - pad, hi + pad)
                            }
                        };
                        let osc_y = |v: f32| -> f32 { osc_top + (osc_max - v) / (osc_max - osc_min) * osc_height };
                        let bar_at_x = ((pos.x - rect.left() + off - bs * 0.5) / bs + vs) as usize;
                        for di in 0..3 {
                            let idx = if di == 0 { bar_at_x } else if di == 1 { bar_at_x.saturating_sub(1) } else { bar_at_x + 1 };
                            if let Some(&v) = ind.values.get(idx) {
                                if !v.is_nan() && (pos.y - osc_y(v)).abs() < 10.0 {
                                    chart.editing_indicator = Some(ind.id);
                                    break;
                                }
                            }
                        }
                        if chart.editing_indicator.is_some() { break; }
                    }
                }
            }

            if osc_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        // ── Signal drawings (auto-generated trendlines from server) ──────────
        if !chart.hide_signal_drawings && !chart.signal_drawings.is_empty() {
            for sd in &chart.signal_drawings {
                let color = hex_to_color(&sd.color, sd.opacity);
                let stroke = egui::Stroke::new(sd.thickness, color);
                match sd.drawing_type.as_str() {
                    "trendline" if sd.points.len() >= 2 => {
                        let b0 = SignalDrawing::time_to_bar(sd.points[0].0, &chart.timestamps);
                        let b1 = SignalDrawing::time_to_bar(sd.points[1].0, &chart.timestamps);
                        let p0 = egui::pos2(bx(b0), py(sd.points[0].1));
                        let p1 = egui::pos2(bx(b1), py(sd.points[1].1));
                        match sd.line_style {
                            LineStyle::Solid => { painter.line_segment([p0, p1], stroke); }
                            _ => {
                                let (dash, gap) = if sd.line_style == LineStyle::Dashed { (6.0, 3.0) } else { (2.0, 2.0) };
                                let dir = p1 - p0; let len = dir.length();
                                if len > 1.0 { let norm = dir / len; let mut d = 0.0;
                                    while d < len { let a = p0 + norm * d; let b = p0 + norm * (d+dash).min(len);
                                        painter.line_segment([a, b], stroke); d += dash + gap; }
                                }
                            }
                        }
                        // Strength indicator — small dot at midpoint, size = strength
                        if sd.strength > 0.0 {
                            let mid = egui::pos2((p0.x+p1.x)/2.0, (p0.y+p1.y)/2.0);
                            painter.circle_filled(mid, 2.0 + sd.strength * 3.0, color);
                        }
                    }
                    "hline" if !sd.points.is_empty() => {
                        let y = py(sd.points[0].1);
                        match sd.line_style {
                            LineStyle::Solid => { painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y)], stroke); }
                            _ => {
                                let mut dx = rect.left(); while dx < rect.left()+cw {
                                    painter.line_segment([egui::pos2(dx, y), egui::pos2((dx+6.0).min(rect.left()+cw), y)], stroke); dx += 10.0;
                                }
                            }
                        }
                    }
                    "hzone" if sd.points.len() >= 2 => {
                        let y0 = py(sd.points[0].1); let y1 = py(sd.points[1].1);
                        let fill = hex_to_color(&sd.color, sd.opacity * 0.15);
                        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(), y0.min(y1)), egui::pos2(rect.left()+cw, y0.max(y1))), 0.0, fill);
                        painter.line_segment([egui::pos2(rect.left(), y0), egui::pos2(rect.left()+cw, y0)], stroke);
                        painter.line_segment([egui::pos2(rect.left(), y1), egui::pos2(rect.left()+cw, y1)], stroke);
                    }
                    _ => {}
                }
            }
        }

        // ── Periodic signal fetch (every 30s) ────────────────────────────────
        if chart.last_signal_fetch.elapsed().as_secs() >= 30 {
            chart.last_signal_fetch = std::time::Instant::now();
            fetch_signal_drawings(chart.symbol.clone());
        }

        // ── OCO/Trigger bracket bands ─────────────────────────────────────────
        {
            let active_orders: Vec<&OrderLevel> = chart.orders.iter().filter(|o| o.status != OrderStatus::Cancelled && o.status != OrderStatus::Executed).collect();
            for order in &active_orders {
                if let Some(pair_id) = order.pair_id {
                    if let Some(pair) = active_orders.iter().find(|o| o.id == pair_id) {
                        // Only draw band once (from the higher-id order to avoid double-draw)
                        if order.id > pair.id {
                            let y1 = py(order.price);
                            let y2 = py(pair.price);
                            let band_color = match order.side {
                                OrderSide::OcoTarget | OrderSide::OcoStop => egui::Color32::from_rgba_unmultiplied(167, 139, 250, 15),
                                OrderSide::TriggerBuy | OrderSide::TriggerSell => egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 12),
                                _ => egui::Color32::TRANSPARENT,
                            };
                            painter.rect_filled(egui::Rect::from_min_max(
                                egui::pos2(rect.left(), y1.min(y2)), egui::pos2(rect.left() + cw, y1.max(y2))),
                                0.0, band_color);
                        }
                    }
                }
            }
        }

        // ── Order lines on chart ──────────────────────────────────────────────
        for order in &chart.orders {
            if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
            let y = py(order.price);
            if y < rect.top() + pt || y > rect.top() + pt + ch { continue; }
            let color = order.color(t);
            let dash_color = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200);

            // Dashed line across full width
            let mut dx = rect.left();
            while dx < rect.left() + cw {
                let end = (dx + 6.0).min(rect.left() + cw);
                painter.line_segment([egui::pos2(dx, y), egui::pos2(end, y)], egui::Stroke::new(1.0, dash_color));
                dx += 10.0;
            }

            // Label badge — LEFT aligned, opaque, with submit button for drafts
            let status_tag = match order.status { OrderStatus::Draft => " DRAFT", OrderStatus::Placed => "", _ => "" };
            let label = format!("{} x{} @ {:.2} {}{}", order.label(), order.qty, order.price, fmt_notional(order.notional()), status_tag);
            let extra_w = if order.status == OrderStatus::Draft { 50.0 } else { 0.0 }; // space for submit btn
            let badge_w = label.len() as f32 * 5.8 + 16.0 + extra_w;
            let badge_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 6.0, y - 10.0),
                egui::vec2(badge_w, 20.0),
            );
            painter.rect_filled(badge_rect, 3.0, t.toolbar_bg);
            painter.rect_stroke(badge_rect, 3.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Outside);
            painter.text(
                egui::pos2(badge_rect.left() + 6.0, badge_rect.center().y),
                egui::Align2::LEFT_CENTER, &label, egui::FontId::monospace(9.0), color,
            );
            // Submit button for draft orders (rendered as a clickable rect at badge end)
            if order.status == OrderStatus::Draft {
                let btn_rect = egui::Rect::from_min_size(
                    egui::pos2(badge_rect.right() - 46.0, badge_rect.top() + 2.0),
                    egui::vec2(42.0, 16.0),
                );
                painter.rect_filled(btn_rect, 2.0, egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 60));
                painter.rect_stroke(btn_rect, 2.0, egui::Stroke::new(0.5, t.accent), egui::StrokeKind::Outside);
                painter.text(btn_rect.center(), egui::Align2::CENTER_CENTER, "SUBMIT", egui::FontId::monospace(8.0), t.accent);
                // We'll detect clicks on this in the interaction section below
            }

            // Price label on y-axis — opaque
            let axis_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + cw + 1.0, y - 8.0), egui::vec2(pr - 2.0, 16.0));
            painter.rect_filled(axis_rect, 2.0, t.toolbar_bg);
            painter.rect_stroke(axis_rect, 2.0, egui::Stroke::new(0.5, color), egui::StrokeKind::Outside);
            let d = if order.price >= 10.0 { 2 } else { 4 };
            painter.text(egui::pos2(rect.left() + cw + 3.0, y), egui::Align2::LEFT_CENTER,
                {
                    chart.fmt_buf.clear(); let _ = write!(chart.fmt_buf, "{:.1$}", order.price, d);
                    &chart.fmt_buf
                }, egui::FontId::monospace(8.5), color);
        }

        // ── Order edit popup (double-click) ──────────────────────────────────
        if let Some(edit_id) = chart.editing_order {
            // Extract order data to avoid borrow conflict
            let order_data = chart.orders.iter().find(|o| o.id == edit_id)
                .map(|o| (o.price, o.color(t), o.label(), o.option_symbol.clone(), o.side));

            if let Some((order_price, color, order_label, opt_sym, side)) = order_data {
                let is_trigger = matches!(side, OrderSide::TriggerBuy | OrderSide::TriggerSell);
                let y = py(order_price);
                let popup_pos = egui::pos2(rect.left() + 10.0, y + 14.0);
                let dialog_w = if is_trigger { 260.0 } else { 210.0 };
                let mut close_editor = false;
                let mut apply_price: Option<f32> = None;
                let mut apply_qty: Option<u32> = None;
                let mut cancel_it = false;

                let title = if is_trigger {
                    format!("EDIT {} TRIGGER", if side == OrderSide::TriggerBuy { "BUY" } else { "SELL" })
                } else {
                    format!("EDIT {}", order_label)
                };

                dialog_window_themed(ctx, &format!("order_edit_{}", edit_id), popup_pos, dialog_w, t.toolbar_bg, t.toolbar_border, Some(color))
                    .show(ctx, |ui| {
                        if dialog_header(ui, &title, t.dim) { close_editor = true; }
                        ui.add_space(8.0);
                        let m = 10.0;

                        // Show option contract info for trigger orders
                        if is_trigger {
                            if let Some(ref opt) = opt_sym {
                                ui.horizontal(|ui| {
                                    ui.add_space(m);
                                    ui.label(egui::RichText::new(Icon::LIGHTNING).size(11.0).color(t.accent));
                                    ui.label(egui::RichText::new(opt).monospace().size(11.0).strong()
                                        .color(egui::Color32::from_rgb(220, 220, 230)));
                                });
                                ui.add_space(2.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(m);
                                    let action = if side == OrderSide::TriggerBuy { "Buy option" } else { "Sell option" };
                                    ui.label(egui::RichText::new(format!("{} when {} reaches trigger price", action, chart.symbol))
                                        .monospace().size(8.0).color(t.dim.gamma_multiply(0.6)));
                                });
                                ui.add_space(6.0);
                                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, 40));
                                ui.add_space(4.0);
                            }
                        }

                        // Trigger/Limit price
                        let price_label = if is_trigger { "Trigger" } else { "Price" };
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new(format!("{:6}", price_label)).monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let resp = ui.add(egui::TextEdit::singleline(&mut chart.edit_order_price)
                                .desired_width(if is_trigger { 130.0 } else { 110.0 }).font(egui::FontId::monospace(12.0))
                                .horizontal_align(egui::Align::RIGHT));
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                if let Ok(p) = chart.edit_order_price.parse::<f32>() { apply_price = Some(p); }
                            }
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            ui.label(egui::RichText::new("Qty   ").monospace().size(9.0).color(t.dim));
                            ui.add_space(4.0);
                            let resp = ui.add(egui::TextEdit::singleline(&mut chart.edit_order_qty)
                                .desired_width(if is_trigger { 130.0 } else { 110.0 }).font(egui::FontId::monospace(12.0))
                                .horizontal_align(egui::Align::RIGHT));
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                if let Ok(q) = chart.edit_order_qty.parse::<u32>() { apply_qty = Some(q.max(1)); }
                            }
                        });

                        ui.add_space(8.0);
                        dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, 50));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add_space(m);
                            let del_color = t.bear;
                            if ui.add(egui::Button::new(egui::RichText::new(format!("{} Cancel Order", Icon::TRASH))
                                .monospace().size(9.0).color(del_color))
                                .fill(color_alpha(del_color, 15)).corner_radius(3.0)
                                .stroke(egui::Stroke::new(0.5, color_alpha(del_color, 60)))
                                .min_size(egui::vec2(0.0, 22.0))).clicked() {
                                cancel_it = true;
                            }
                        });
                        ui.add_space(8.0);
                    });

                // Apply deferred changes
                if let Some(p) = apply_price {
                    if let Some(o) = chart.orders.iter_mut().find(|o| o.id == edit_id) { o.price = p; }
                }
                if let Some(q) = apply_qty {
                    if let Some(o) = chart.orders.iter_mut().find(|o| o.id == edit_id) { o.qty = q; }
                }
                if cancel_it {
                    cancel_order_with_pair(&mut chart.orders, edit_id);
                    chart.editing_order = None;
                }
                if close_editor { chart.editing_order = None; }
            } else {
                chart.editing_order = None;
            }
        }

        // ── Order entry panel (bottom-left of pane) ─────────────────────────
        if watchlist.order_entry_open {
            // Auto-expand advanced mode for option charts (UND is there)
            if chart.is_option && !chart.order_advanced {
                chart.order_advanced = true;
                chart.order_type_idx = 5; // default to UND for options
            }
            let adv = chart.order_advanced;
            let panel_w = if adv { 300.0 } else { 230.0 };
            // Position: relative to chart rect, default bottom-left. Negative Y = from bottom.
            let abs_pos = if chart.order_panel_pos.y < 0.0 {
                egui::pos2(rect.left() + chart.order_panel_pos.x, rect.top() + pt + ch + chart.order_panel_pos.y)
            } else {
                egui::pos2(rect.left() + chart.order_panel_pos.x, rect.top() + pt + chart.order_panel_pos.y)
            };

            let order_types: Vec<&str> = if chart.is_option {
                vec!["MKT", "LMT", "STP", "STP-LMT", "TRAIL", "UND"]
            } else {
                vec!["MKT", "LMT", "STP", "STP-LMT", "TRAIL"]
            };
            let tifs = ["DAY", "GTC", "IOC"];

            // Collapsed mode: pill with double-click to expand
            if chart.order_collapsed {
                let pill_w = 90.0;
                egui::Window::new(format!("order_pill_{}", pane_idx))
                    .fixed_pos(abs_pos)
                    .fixed_size(egui::vec2(pill_w, 24.0))
                    .title_bar(false)
                    .frame(egui::Frame::popup(&ctx.style())
                        .fill(color_alpha(t.toolbar_bg, 235))
                        .inner_margin(egui::Margin { left: 8, right: 8, top: 4, bottom: 4 })
                        .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 100)))
                        .corner_radius(12.0))
                    .show(ctx, |ui| {
                        let resp = ui.horizontal(|ui| {
                            let armed_dot = if chart.armed { t.accent } else { t.dim.gamma_multiply(0.3) };
                            ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 8.0), 3.5, armed_dot);
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new("ORDER").monospace().size(10.0).strong().color(t.dim.gamma_multiply(0.7)));
                        });
                        // Single interaction: click_and_drag on the whole pill
                        let pill_resp = ui.interact(resp.response.rect, egui::Id::new(("order_pill_interact", pane_idx)), egui::Sense::click_and_drag());
                        if pill_resp.double_clicked() { chart.order_collapsed = false; }
                        if pill_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                        if pill_resp.dragged() {
                            let delta = pill_resp.drag_delta();
                            chart.order_panel_pos.x += delta.x;
                            chart.order_panel_pos.y += delta.y;
                        }
                    });
            } else {
            egui::Window::new(format!("order_entry_{}", pane_idx))
                .fixed_pos(abs_pos)
                .fixed_size(egui::vec2(panel_w, 0.0))
                .title_bar(false)
                .frame(egui::Frame::popup(&ctx.style())
                    .fill(color_alpha(t.toolbar_bg, 245))
                    .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
                    .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 100)))
                    .corner_radius(4.0))
                .show(ctx, |ui| {
                    let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
                    let spread = (last_price * 0.0001).max(0.01);

                    // ── Header bar: armed toggle | ORDER | separator | +/- ──
                    let header_resp = ui.horizontal(|ui| {
                        ui.set_min_width(panel_w);
                        let header_rect = ui.max_rect();
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(header_rect.min, egui::vec2(panel_w, 22.0)),
                            egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 },
                            color_alpha(t.toolbar_border, 30));
                        ui.add_space(4.0);
                        // Armed toggle (inline in header)
                        let armed_icon = if chart.armed { Icon::SHIELD_WARNING } else { Icon::PLAY };
                        let armed_color = if chart.armed { t.accent } else { t.dim.gamma_multiply(0.4) };
                        let armed_resp = ui.add(egui::Button::new(egui::RichText::new(armed_icon).size(11.0).color(armed_color))
                            .fill(if chart.armed { color_alpha(t.accent, 25) } else { egui::Color32::TRANSPARENT })
                            .stroke(egui::Stroke::NONE).min_size(egui::vec2(18.0, 18.0)).corner_radius(2.0));
                        if armed_resp.clicked() { chart.armed = !chart.armed; }
                        if armed_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            egui::show_tooltip(ui.ctx(), ui.layer_id(), egui::Id::new(("armed_tip", pane_idx)), |ui| {
                                ui.label(egui::RichText::new(if chart.armed { "Armed — sends to IB" } else { "Unarmed — drafts only" }).monospace().size(9.0));
                            });
                        }
                        // Label
                        ui.label(egui::RichText::new("ORDER").monospace().size(9.0).strong().color(t.dim.gamma_multiply(0.6)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(4.0);
                            // Expand/collapse advanced toggle
                            let exp_icon = if adv { Icon::MINUS } else { Icon::PLUS };
                            let exp_resp = ui.add(egui::Button::new(egui::RichText::new(exp_icon).size(10.0).color(t.dim.gamma_multiply(0.5)))
                                .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(20.0, 18.0)).corner_radius(2.0));
                            if exp_resp.clicked() { chart.order_advanced = !chart.order_advanced; }
                            if exp_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                            // Separator
                            ui.add(egui::Separator::default().spacing(2.0));
                        });
                    });
                    // Drag + double-click on middle zone only (between armed button and +/- button)
                    let hdr_min = header_resp.response.rect.min;
                    let mid_rect = egui::Rect::from_min_size(
                        egui::pos2(hdr_min.x + 26.0, hdr_min.y),
                        egui::vec2(panel_w - 56.0, 22.0));
                    let drag_resp = ui.interact(mid_rect, egui::Id::new(("order_panel_drag", pane_idx)), egui::Sense::click_and_drag());
                    if drag_resp.double_clicked() { chart.order_collapsed = true; }
                    if drag_resp.dragged() {
                        let delta = drag_resp.drag_delta();
                        chart.order_panel_pos.x += delta.x;
                        chart.order_panel_pos.y += delta.y;
                        // Clamp to chart area
                        chart.order_panel_pos.x = chart.order_panel_pos.x.clamp(0.0, (cw - panel_w).max(0.0));
                        if chart.order_panel_pos.y < 0.0 {
                            chart.order_panel_pos.y = chart.order_panel_pos.y.clamp(-(ch - 30.0), -30.0);
                        } else {
                            chart.order_panel_pos.y = chart.order_panel_pos.y.clamp(0.0, (ch - 30.0).max(0.0));
                        }
                    }
                    if drag_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }

                    // ── Body ──
                    let pad = 8.0;
                    ui.add_space(4.0);

                    // Advanced: Order type + TIF selectors
                    if adv {
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (i, &ot) in order_types.iter().enumerate() {
                                let sel = chart.order_type_idx == i;
                                let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
                                let bg = if sel { color_alpha(t.accent, 60) } else { color_alpha(t.toolbar_border, 25) };
                                let rounding = if i == 0 {
                                    egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }
                                } else if i == order_types.len() - 1 {
                                    egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 }
                                } else { egui::CornerRadius::ZERO };
                                if ui.add(egui::Button::new(egui::RichText::new(ot).monospace().size(8.0).color(fg))
                                    .fill(bg).corner_radius(rounding).min_size(egui::vec2(0.0, 18.0))
                                    .stroke(egui::Stroke::new(0.5, if sel { color_alpha(t.accent, 120) } else { color_alpha(t.toolbar_border, 50) })))
                                    .clicked() {
                                    chart.order_type_idx = i;
                                    chart.order_market = i == 0;
                                }
                            }
                            ui.add_space(8.0);
                            // TIF
                            for (i, &tf) in tifs.iter().enumerate() {
                                let sel = chart.order_tif_idx == i;
                                let fg = if sel { t.accent } else { t.dim.gamma_multiply(0.5) };
                                if ui.add(egui::Button::new(egui::RichText::new(tf).monospace().size(8.0).color(fg))
                                    .frame(false).min_size(egui::vec2(24.0, 18.0))).clicked() {
                                    chart.order_tif_idx = i;
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }

                    // Row 1: [-] qty [+]
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let step = if chart.order_qty >= 100 { 10 } else if chart.order_qty >= 10 { 5 } else { 1 };
                        if ui.add(egui::Button::new(egui::RichText::new("-").monospace().size(11.0))
                            .min_size(egui::vec2(20.0, 20.0)).corner_radius(2.0)
                            .fill(color_alpha(t.toolbar_border, 40))).clicked() {
                            chart.order_qty = chart.order_qty.saturating_sub(step).max(1);
                        }
                        let _ = ui.add(egui::TextEdit::singleline(&mut format!("{}", chart.order_qty))
                            .desired_width(44.0).font(egui::FontId::monospace(12.0))
                            .horizontal_align(egui::Align::Center).interactive(false));
                        if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(11.0))
                            .min_size(egui::vec2(20.0, 20.0)).corner_radius(2.0)
                            .fill(color_alpha(t.toolbar_border, 40))).clicked() {
                            chart.order_qty += step;
                        }
                        ui.add_space(4.0);
                        let cursor = ui.cursor().min;
                        ui.painter().line_segment(
                            [egui::pos2(cursor.x, cursor.y), egui::pos2(cursor.x, cursor.y + 20.0)],
                            egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80)));
                        ui.add_space(6.0);

                        if !adv {
                            // Compact mode: single price + MKT/LMT toggle
                            if chart.order_market {
                                ui.label(egui::RichText::new(format!("{:.2}", last_price)).monospace().size(12.0).color(t.dim));
                            } else {
                                ui.add(egui::TextEdit::singleline(&mut chart.order_limit_price)
                                    .desired_width(68.0).font(egui::FontId::monospace(12.0)).hint_text("Price")
                                    .horizontal_align(egui::Align::RIGHT));
                            }
                            ui.add_space(2.0);
                            let mkt_label = if chart.order_market { "MKT" } else { "LMT" };
                            if ui.add(egui::Button::new(egui::RichText::new(mkt_label).monospace().size(9.0).strong()
                                .color(if chart.order_market { t.accent } else { t.dim }))
                                .fill(if chart.order_market { color_alpha(t.accent, 35) } else { t.toolbar_bg })
                                .stroke(egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 90))).corner_radius(2.0)
                                .min_size(egui::vec2(30.0, 20.0))).clicked() {
                                chart.order_market = !chart.order_market;
                                if !chart.order_market && chart.order_limit_price.is_empty() {
                                    chart.order_limit_price = format!("{:.2}", last_price);
                                }
                            }
                        } else {
                            // Advanced: show last price as reference
                            ui.label(egui::RichText::new(format!("Last {:.2}", last_price)).monospace().size(10.0).color(t.dim.gamma_multiply(0.6)));
                        }
                    });

                    // Advanced: price fields per order type
                    if adv {
                        let oti = chart.order_type_idx;
                        ui.add_space(2.0);
                        // LMT, STP-LMT: Limit price
                        if oti == 1 || oti == 3 {
                            ui.horizontal(|ui| {
                                ui.add_space(pad);
                                ui.label(egui::RichText::new("Limit").monospace().size(9.0).color(t.dim));
                                ui.add_space(4.0);
                                ui.add(egui::TextEdit::singleline(&mut chart.order_limit_price)
                                    .desired_width(80.0).font(egui::FontId::monospace(11.0)).hint_text("Limit price")
                                    .horizontal_align(egui::Align::RIGHT));
                            });
                        }
                        // STP, STP-LMT: Stop price
                        if oti == 2 || oti == 3 {
                            ui.horizontal(|ui| {
                                ui.add_space(pad);
                                ui.label(egui::RichText::new("Stop ").monospace().size(9.0).color(t.bear));
                                ui.add_space(4.0);
                                ui.add(egui::TextEdit::singleline(&mut chart.order_stop_price)
                                    .desired_width(80.0).font(egui::FontId::monospace(11.0)).hint_text("Stop price")
                                    .horizontal_align(egui::Align::RIGHT));
                            });
                        }
                        // TRAIL: Trailing amount
                        if oti == 4 {
                            ui.horizontal(|ui| {
                                ui.add_space(pad);
                                ui.label(egui::RichText::new("Trail").monospace().size(9.0).color(t.accent));
                                ui.add_space(4.0);
                                ui.add(egui::TextEdit::singleline(&mut chart.order_trail_amt)
                                    .desired_width(80.0).font(egui::FontId::monospace(11.0)).hint_text("Trail amt")
                                    .horizontal_align(egui::Align::RIGHT));
                            });
                        }
                    }

                    // Advanced: Bracket mode (TP + SL)
                    if adv {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            let brk_color = if chart.order_bracket { t.accent } else { t.dim.gamma_multiply(0.5) };
                            if ui.add(egui::Button::new(egui::RichText::new("Bracket").monospace().size(9.0).color(brk_color))
                                .fill(if chart.order_bracket { color_alpha(t.accent, 25) } else { egui::Color32::TRANSPARENT })
                                .stroke(egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 60))).corner_radius(2.0)
                                .min_size(egui::vec2(0.0, 18.0))).clicked() {
                                chart.order_bracket = !chart.order_bracket;
                            }
                            if chart.order_bracket {
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new("TP").monospace().size(9.0).color(t.bull));
                                ui.add(egui::TextEdit::singleline(&mut chart.order_tp_price)
                                    .desired_width(52.0).font(egui::FontId::monospace(10.0)).hint_text("Take")
                                    .horizontal_align(egui::Align::RIGHT));
                                ui.label(egui::RichText::new("SL").monospace().size(9.0).color(t.bear));
                                ui.add(egui::TextEdit::singleline(&mut chart.order_sl_price)
                                    .desired_width(52.0).font(egui::FontId::monospace(10.0)).hint_text("Stop")
                                    .horizontal_align(egui::Align::RIGHT));
                            }
                        });

                        // (C/P + trigger buttons removed — UND BUY/SELL in main buttons handles this)
                    }

                    ui.add_space(4.0);

                    // Row 2: BUY | SELL | armed toggle
                    let buy_price = if chart.order_market { last_price + spread } else {
                        chart.order_limit_price.parse::<f32>().unwrap_or(last_price)
                    };
                    let sell_price = if chart.order_market { last_price - spread } else {
                        chart.order_limit_price.parse::<f32>().unwrap_or(last_price)
                    };

                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let btn_w = (panel_w - pad * 2.0 - 8.0) / 2.0;
                        let is_und = adv && chart.order_type_idx == 5 && chart.is_option;
                        let buy_label = if is_und { format!("BUY {} on UND", chart.option_type) } else { format!("BUY {:.2}", buy_price) };
                        let sell_label = if is_und { format!("SELL {} on UND", chart.option_type) } else { format!("SELL {:.2}", sell_price) };
                        // BUY
                        if trade_btn(ui, &buy_label, t.bull, btn_w) {
                            if is_und {
                                // Place a TriggerBuy order level on the underlying pane
                                chart.pending_und_order = Some(OrderSide::TriggerBuy);
                            } else if chart.armed && adv {
                                let sym = chart.symbol.clone();
                                let qty = chart.order_qty;
                                let ot_idx = chart.order_type_idx;
                                let tif_idx = chart.order_tif_idx;
                                let price = buy_price;
                                let bracket = chart.order_bracket;
                                let tp = chart.order_tp_price.parse::<f32>().ok();
                                let sl = chart.order_sl_price.parse::<f32>().ok();
                                std::thread::spawn(move || {
                                    submit_ib_order(&sym, "BUY", qty, ot_idx, tif_idx, price, bracket, tp, sl);
                                });
                            } else {
                                let id = chart.next_order_id; chart.next_order_id += 1;
                                let s = if chart.armed { OrderStatus::Placed } else { OrderStatus::Draft };
                                chart.orders.push(OrderLevel { id, side: OrderSide::Buy, price: buy_price, qty: chart.order_qty, status: s, pair_id: None, option_symbol: None, option_con_id: None });
                                if !chart.armed { chart.pending_confirms.push((id, std::time::Instant::now())); }
                            }
                        }
                        // SELL
                        if trade_btn(ui, &sell_label, t.bear, btn_w) {
                            if is_und {
                                chart.pending_und_order = Some(OrderSide::TriggerSell);
                            } else if chart.armed && adv {
                                let sym = chart.symbol.clone();
                                let qty = chart.order_qty;
                                let ot_idx = chart.order_type_idx;
                                let tif_idx = chart.order_tif_idx;
                                let price = sell_price;
                                let bracket = chart.order_bracket;
                                let tp = chart.order_tp_price.parse::<f32>().ok();
                                let sl = chart.order_sl_price.parse::<f32>().ok();
                                std::thread::spawn(move || {
                                    submit_ib_order(&sym, "SELL", qty, ot_idx, tif_idx, price, bracket, tp, sl);
                                });
                            } else {
                                let id = chart.next_order_id; chart.next_order_id += 1;
                                let s = if chart.armed { OrderStatus::Placed } else { OrderStatus::Draft };
                                chart.orders.push(OrderLevel { id, side: OrderSide::Sell, price: sell_price, qty: chart.order_qty, status: s, pair_id: None, option_symbol: None, option_con_id: None });
                                if !chart.armed { chart.pending_confirms.push((id, std::time::Instant::now())); }
                            }
                        }
                    });
                    ui.add_space(6.0);
                });
            } // end if !collapsed

            // ── Pending confirm toasts (above order entry panel) ─────────
            if !chart.pending_confirms.is_empty() {
                let mut confirm_ids: Vec<u32> = Vec::new();
                let mut cancel_ids: Vec<u32> = Vec::new();
                let base_y = rect.top() + pt + ch - 120.0 - 28.0; // above the panel (raised 20px)

                for (ci, (oid, _created)) in chart.pending_confirms.iter().enumerate() {
                    let order_data = chart.orders.iter().find(|o| o.id == *oid)
                        .map(|o| (o.label(), o.price, o.qty, o.color(t)));
                    if let Some((label, price, qty, color)) = order_data {
                        let toast_y = base_y - ci as f32 * 34.0;
                        egui::Window::new(format!("confirm_toast_{}_{}", pane_idx, oid))
                            .fixed_pos(egui::pos2(rect.left() + 8.0, toast_y))
                            .fixed_size(egui::vec2(180.0, 26.0))
                            .title_bar(false)
                            .frame(egui::Frame::popup(&ctx.style()).fill(t.toolbar_bg).inner_margin(4.0)
                                .stroke(egui::Stroke::new(1.0, color)))
                            .show(ctx, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("{} x{} @ {:.2}", label, qty, price)).monospace().size(10.0).color(color));
                                    if ui.add(egui::Button::new(egui::RichText::new(Icon::CHECK).size(12.0).color(t.bull))
                                        .fill(egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 40))
                                        .corner_radius(2.0).min_size(egui::vec2(24.0, 20.0))).clicked() {
                                        confirm_ids.push(*oid);
                                    }
                                    if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.bear))
                                        .corner_radius(2.0).min_size(egui::vec2(24.0, 20.0))).clicked() {
                                        cancel_ids.push(*oid);
                                    }
                                });
                            });
                    } else {
                        cancel_ids.push(*oid); // order was deleted
                    }
                }

                // Apply confirms — place the orders
                for id in &confirm_ids {
                    if let Some(o) = chart.orders.iter_mut().find(|o| o.id == *id) {
                        o.status = OrderStatus::Placed;
                    }
                }
                // Apply cancels (with pair cancellation)
                for id in &cancel_ids {
                    cancel_order_with_pair(&mut chart.orders, *id);
                }
                chart.pending_confirms.retain(|(id, _)| !confirm_ids.contains(id) && !cancel_ids.contains(id));
            }
        }

        // Middle-click cycles through drawing tools
        if ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Middle)) {
            let tools = ["", "trendline", "hline", "hzone", "barmarker"];
            let cur = tools.iter().position(|&t| t == chart.draw_tool).unwrap_or(0);
            chart.draw_tool = tools[(cur + 1) % tools.len()].to_string();
            chart.pending_pt = None;
        }

        // Drawing preview + custom cursors (only in hovered pane)
        if pointer_in_pane { if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            if chart.draw_tool == "hzone" { if let Some((_b0, p0)) = chart.pending_pt {
                let y0 = py(p0);
                // Zone preview fill
                painter.rect_filled(
                    egui::Rect::from_min_max(egui::pos2(rect.left(), y0.min(pos.y)), egui::pos2(rect.left()+cw, y0.max(pos.y))),
                    0.0, egui::Color32::from_rgba_unmultiplied(100,160,255,25));
                // Border lines
                painter.line_segment([egui::pos2(rect.left(),y0),egui::pos2(rect.left()+cw,y0)], egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100,160,255,120)));
                painter.line_segment([egui::pos2(rect.left(),pos.y),egui::pos2(rect.left()+cw,pos.y)], egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100,160,255,120)));
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } }
            else if chart.draw_tool == "trendline" {
                if let Some((b0, p0)) = chart.pending_pt {
                    let start = egui::pos2(bx(b0), py(p0));
                    let dir = pos - start;
                    let len = dir.length();
                    if len > 2.0 {
                        let dash_len = 6.0; let gap_len = 4.0; let step = dash_len + gap_len;
                        let norm = dir / len;
                        let mut d = 0.0;
                        while d < len {
                            let a = start + norm * d;
                            let b = start + norm * (d + dash_len).min(len);
                            painter.line_segment([a, b], egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100,160,255,180)));
                            d += step;
                        }
                    }
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if chart.draw_tool == "hline" {
                if pos.y >= rect.top()+pt && pos.y < rect.top()+pt+ch {
                    painter.line_segment(
                        [egui::pos2(rect.left(), pos.y), egui::pos2(rect.left()+cw, pos.y)],
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100,180,255,120)),
                    );
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if chart.draw_tool == "barmarker" {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        } } // end pointer_in_pane + hover_pos

        // Crosshair (only when not in drawing mode, only in hovered pane)
        if pointer_in_pane && chart.draw_tool.is_empty() {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                if pos.x >= rect.left() && pos.x < rect.left()+cw && pos.y >= rect.top()+pt && pos.y < rect.top()+pt+ch {
                    painter.line_segment([egui::pos2(rect.left(),pos.y),egui::pos2(rect.left()+cw,pos.y)],egui::Stroke::new(0.5,egui::Color32::from_white_alpha(50)));
                    painter.line_segment([egui::pos2(pos.x,rect.top()+pt),egui::pos2(pos.x,rect.top()+pt+ch)],egui::Stroke::new(0.5,egui::Color32::from_white_alpha(50)));
                    let hp = min_p+(max_p-min_p)*(1.0-(pos.y-rect.top()-pt)/ch);
                    let d = if hp>=10.0{2}else{4};
                    chart.fmt_buf.clear(); let _ = write!(chart.fmt_buf, "{:.1$}", hp, d);
                    painter.text(egui::pos2(rect.left()+cw+3.0,pos.y),egui::Align2::LEFT_CENTER,&chart.fmt_buf,egui::FontId::monospace(8.5),egui::Color32::WHITE);
                }
            }
        }

        span_end(); // pane_render

        // ── Interaction ───────────────────────────────────────────────────────
        span_begin("interaction");
        // Chart interaction area — only the chart body, not the axis strips
        let chart_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top()+pt), egui::vec2(cw, ch));
        let resp = ui.allocate_rect(chart_rect, egui::Sense::click_and_drag());

        // Activate pane on any interaction
        if visible_count > 1 && (resp.clicked() || resp.drag_started()) {
            *active_pane = pane_idx;
        }

        // Check for submit button clicks on draft order badges
        if resp.clicked() && chart.draw_tool.is_empty() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let mut submitted = Vec::new();
                for order in &chart.orders {
                    if order.status != OrderStatus::Draft { continue; }
                    let oy = py(order.price);
                    // Submit button is at badge right side
                    let status_tag = " DRAFT";
                    let label = format!("{} x{} @ {:.2} {}{}", order.label(), order.qty, order.price, fmt_notional(order.notional()), status_tag);
                    let badge_w = label.len() as f32 * 5.8 + 16.0 + 50.0;
                    let btn_left = rect.left() + 6.0 + badge_w - 46.0;
                    let btn_top = oy - 10.0 + 2.0;
                    let btn_rect = egui::Rect::from_min_size(egui::pos2(btn_left, btn_top), egui::vec2(42.0, 16.0));
                    if btn_rect.contains(pos) {
                        submitted.push(order.id);
                    }
                }
                for id in &submitted {
                    // Submit this order and its pair
                    if let Some(o) = chart.orders.iter_mut().find(|o| o.id == *id) {
                        o.status = OrderStatus::Placed;
                        if let Some(pid) = o.pair_id {
                            if let Some(p) = chart.orders.iter_mut().find(|o| o.id == pid && o.status == OrderStatus::Draft) {
                                p.status = OrderStatus::Placed;
                            }
                        }
                    }
                }
            }
        }

        let pos_to_bar = |pos: egui::Pos2| -> f32 { (pos.x - rect.left() + off - bs*0.5) / bs + vs };
        let pos_to_price = |pos: egui::Pos2| -> f32 { min_p + (max_p-min_p) * (1.0 - (pos.y - rect.top() - pt) / ch) };

        // ── Measure tool (shift+drag or context menu) ────────────────────────
        let shift_held = ui.input(|i| i.modifiers.shift);
        if (shift_held || chart.measure_active) && pointer_in_pane && chart.draw_tool.is_empty() {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                let bar_f = pos_to_bar(pos);
                let price_f = pos_to_price(pos);

                if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                    chart.measure_start = Some((bar_f, price_f));
                    chart.measuring = true;
                }

                if chart.measuring {
                    if let Some((sb, sp)) = chart.measure_start {
                        let start_pos = egui::pos2(bx(sb), py(sp));

                        // Dashed line
                        let dir = pos - start_pos;
                        let len = dir.length();
                        if len > 2.0 {
                            let norm = dir / len;
                            let mut dd = 0.0;
                            while dd < len {
                                let a = start_pos + norm * dd;
                                let b_pt = start_pos + norm * (dd + 4.0).min(len);
                                painter.line_segment([a, b_pt], egui::Stroke::new(1.0, t.accent));
                                dd += 7.0;
                            }
                        }

                        painter.circle_filled(start_pos, 3.0, t.accent);
                        painter.circle_filled(pos, 3.0, t.accent);

                        // Measurement label
                        let price_diff = price_f - sp;
                        let bar_diff = (bar_f - sb).abs();
                        let pct = if sp != 0.0 { (price_diff / sp) * 100.0 } else { 0.0 };
                        let candle_sec = if chart.timestamps.len() > 1 { (chart.timestamps[1] - chart.timestamps[0]).max(60) } else { 300 };
                        let time_secs = (bar_diff * candle_sec as f32) as i64;
                        let time_str = if time_secs >= 86400 { format!("{}d {}h", time_secs / 86400, (time_secs % 86400) / 3600) }
                            else if time_secs >= 3600 { format!("{}h {}m", time_secs / 3600, (time_secs % 3600) / 60) }
                            else { format!("{}m", time_secs / 60) };

                        let label = format!("{:+.2} ({:+.2}%)  {} bars  {}", price_diff, pct, bar_diff.round() as i32, time_str);
                        let label_pos = egui::pos2((start_pos.x + pos.x) / 2.0, (start_pos.y + pos.y) / 2.0 - 14.0);
                        let label_color = if price_diff >= 0.0 { t.bull } else { t.bear };

                        let galley = painter.layout_no_wrap(label.clone(), egui::FontId::monospace(10.0), label_color);
                        let label_rect = egui::Rect::from_center_size(label_pos, galley.size() + egui::vec2(8.0, 4.0));
                        painter.rect_filled(label_rect, 3.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 220));
                        painter.text(label_pos, egui::Align2::CENTER_CENTER, &label, egui::FontId::monospace(10.0), label_color);
                    }

                    if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                        chart.measuring = false;
                        chart.measure_start = None;
                        chart.measure_active = false;
                    }
                }

                if pointer_in_pane { ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair); }
            }
        }

        // Pre-compute hit test (generous radii for easy interaction)
        let hover_hit: Option<(String, i32)> = ui.input(|i| i.pointer.hover_pos()).and_then(|pos| {
            for d in chart.drawings.iter().rev() {
                match &d.kind {
                    DrawingKind::HLine{price} => {
                        if (pos.y - py(*price)).abs() < 12.0 && pos.x < rect.left()+cw { return Some((d.id.clone(), -1)); }
                    }
                    DrawingKind::TrendLine{price0,time0,price1,time1} => {
                        let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0)); let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), py(*price1));
                        if p0.distance(pos) < 14.0 { return Some((d.id.clone(), 0)); }
                        if p1.distance(pos) < 14.0 { return Some((d.id.clone(), 1)); }
                        let dx=p1.x-p0.x; let dy=p1.y-p0.y; let len2=dx*dx+dy*dy;
                        if len2>0.0 { let t=((pos.x-p0.x)*dx+(pos.y-p0.y)*dy)/len2; let t=t.max(0.0).min(1.0);
                            if egui::pos2(p0.x+t*dx,p0.y+t*dy).distance(pos)<10.0 { return Some((d.id.clone(),-1)); }
                        }
                    }
                    DrawingKind::HZone{price0,price1} => {
                        if (pos.y-py(*price0)).abs()<10.0 { return Some((d.id.clone(),0)); }
                        if (pos.y-py(*price1)).abs()<10.0 { return Some((d.id.clone(),1)); }
                    }
                    DrawingKind::BarMarker{time,price,..} => {
                        if egui::pos2(bx(SignalDrawing::time_to_bar(*time, &chart.timestamps)),py(*price)).distance(pos) < 12.0 { return Some((d.id.clone(), -1)); }
                    }
                }
            }
            None
        });
        // Show move/grab cursor when hovering over a drawing or order line (only in this pane)
        if pointer_in_pane && chart.draw_tool.is_empty() {
            if let Some((_, ep)) = &hover_hit {
                ui.ctx().set_cursor_icon(if *ep >= 0 { egui::CursorIcon::Grab } else { egui::CursorIcon::Move });
            } else if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                // Check if hovering over an order line
                for order in &chart.orders {
                    if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
                    if (pos.y - py(order.price)).abs() < 18.0 && pos.x < rect.left() + cw {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        break;
                    }
                }
            }
        }

        let ts_ref = &chart.timestamps;
        let hit_at = |px: f32, py_pos: f32, drawings: &[Drawing]| -> Option<(String, i32)> {
            for d in drawings.iter().rev() {
                match &d.kind {
                    DrawingKind::HLine{price} => {
                        if (py_pos - py(*price)).abs() < 12.0 { return Some((d.id.clone(), -1)); }
                    }
                    DrawingKind::TrendLine{price0,time0,price1,time1} => {
                        let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                        let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                        if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                        if p1.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 1)); }
                        let dx = p1.x-p0.x; let dy = p1.y-p0.y; let len2 = dx*dx+dy*dy;
                        if len2 > 0.0 { let t = ((px-p0.x)*dx+(py_pos-p0.y)*dy)/len2;
                            let t = t.max(0.0).min(1.0);
                            if egui::pos2(p0.x+t*dx, p0.y+t*dy).distance(egui::pos2(px, py_pos)) < 10.0 { return Some((d.id.clone(), -1)); }
                        }
                    }
                    DrawingKind::HZone{price0,price1} => {
                        if (py_pos - py(*price0)).abs() < 10.0 { return Some((d.id.clone(), 0)); }
                        if (py_pos - py(*price1)).abs() < 10.0 { return Some((d.id.clone(), 1)); }
                    }
                    DrawingKind::BarMarker{time,price,..} => {
                        if egui::pos2(bx(SignalDrawing::time_to_bar(*time, ts_ref)),py(*price)).distance(egui::pos2(px,py_pos)) < 12.0 { return Some((d.id.clone(), -1)); }
                    }
                }
            }
            None
        };

        // Drawing tool: click to place
        if !chart.draw_tool.is_empty() && resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let bar = pos_to_bar(pos);
                let price = pos_to_price(pos);
                let sym = chart.symbol.clone();
                let tf = chart.timeframe.clone();
                match chart.draw_tool.as_str() {
                    "hline" => {
                        let mut d = Drawing::new(new_uuid(), DrawingKind::HLine { price });
                        d.color = chart.draw_color.clone(); d.line_style = LineStyle::Dashed;
                        crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                        chart.drawings.push(d); chart.draw_tool.clear();
                    }
                    "trendline" => {
                        if let Some((b0, p0)) = chart.pending_pt {
                            let t0 = bar_to_time(b0, &chart.timestamps);
                            let t1 = bar_to_time(bar, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::TrendLine { price0: p0, time0: t0, price1: price, time1: t1 });
                            d.color = chart.draw_color.clone();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            chart.drawings.push(d); chart.pending_pt = None; chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "hzone" => {
                        if let Some((_b0, p0)) = chart.pending_pt {
                            let mut d = Drawing::new(new_uuid(), DrawingKind::HZone { price0: p0, price1: price });
                            d.color = chart.draw_color.clone();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            chart.drawings.push(d); chart.pending_pt = None; chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "barmarker" => {
                        let bar_idx = bar.round() as usize;
                        if let Some(b) = chart.bars.get(bar_idx) {
                            let mid = (b.open + b.close) / 2.0;
                            let up = price > mid;
                            let snap_price = if up { b.high } else { b.low };
                            let ts = chart.timestamps.get(bar_idx).copied().unwrap_or(0);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::BarMarker { time: ts, price: snap_price, up });
                            d.color = chart.draw_color.clone();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            chart.drawings.push(d); chart.draw_tool.clear();
                        }
                    }
                    _ => {}
                }
            }
        }
        // No tool: click selects drawing (shift for multi-select), or deselects
        // Skip if egui is using pointer (style popup, dropdown, etc.)
        else if chart.draw_tool.is_empty() && resp.clicked() && !ctx.is_using_pointer() && !ctx.is_pointer_over_area() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let shift = ui.input(|i| i.modifiers.shift);
                if let Some((id, _)) = hit_at(pos.x, pos.y, &chart.drawings) {
                    if shift {
                        if chart.selected_ids.contains(&id) { chart.selected_ids.retain(|x| x != &id); }
                        else { chart.selected_ids.push(id.clone()); }
                    } else {
                        chart.selected_ids = vec![id.clone()];
                    }
                    chart.selected_id = Some(id);
                } else {
                    chart.selected_id = None;
                    chart.selected_ids.clear();
                }
            }
        }

        // Drag: pan chart OR move drawing OR move order
        if chart.draw_tool.is_empty() && resp.drag_started_by(egui::PointerButton::Primary) && !ctx.is_pointer_over_area() {
            if let Some(pos) = resp.interact_pointer_pos() {
                // Check order lines first
                let mut hit_order = false;
                for order in &chart.orders {
                    if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
                    if (pos.y - py(order.price)).abs() < 18.0 && pos.x < rect.left() + cw {
                        chart.dragging_order = Some(order.id);
                        hit_order = true;
                        break;
                    }
                }
                if !hit_order {
                    if let Some((id, ep)) = hit_at(pos.x, pos.y, &chart.drawings) {
                        chart.dragging_drawing = Some((id, ep));
                        chart.drag_start_price = pos_to_price(pos);
                        chart.drag_start_bar = pos_to_bar(pos);
                    }
                }
            }
        }
        // Order dragging
        if let Some(order_id) = chart.dragging_order {
            if resp.dragged_by(egui::PointerButton::Primary) {
                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let new_price = pos_to_price(pos);
                    if let Some(o) = chart.orders.iter_mut().find(|o| o.id == order_id) {
                        o.price = new_price;
                    }
                }
                if pointer_in_pane { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical); }
            }
            if resp.drag_stopped() { chart.dragging_order = None; }
        }
        if let Some((ref id, ep)) = chart.dragging_drawing.clone() {
            if resp.dragged_by(egui::PointerButton::Primary) {
                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let new_p = pos_to_price(pos);
                    let new_b = pos_to_bar(pos);
                    let dp = new_p - chart.drag_start_price;
                    let db = new_b - chart.drag_start_bar;
                    if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == *id) {
                        match &mut d.kind {
                            DrawingKind::HLine{price} => *price += dp,
                            DrawingKind::TrendLine{price0,time0,price1,time1} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::HZone{price0,price1} => match ep {
                                0 => *price0 = new_p,
                                1 => *price1 = new_p,
                                _ => { *price0 += dp; *price1 += dp; }
                            },
                            DrawingKind::BarMarker{time,price,..} => {
                                let b = SignalDrawing::time_to_bar(*time, &chart.timestamps) + db;
                                *time = bar_to_time(b, &chart.timestamps);
                                *price += dp;
                            },
                        }
                    }
                    chart.drag_start_price = new_p;
                    chart.drag_start_bar = new_b;
                }
            }
            if resp.drag_stopped() {
                // Save the dragged drawing to DB
                if let Some((ref did, _)) = chart.dragging_drawing {
                    if let Some(d) = chart.drawings.iter().find(|d| d.id == *did) {
                        crate::drawing_db::save(&drawing_to_db(d, &chart.symbol, &chart.timeframe));
                    }
                }
                chart.dragging_drawing = None;
            }
        }
        // Trigger order crosshair mode — click to place ONE trigger level
        else if chart.trigger_setup.phase == TriggerPhase::Picking {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            if let Some(mouse) = ui.input(|i| i.pointer.hover_pos()) {
                if rect.contains(mouse) {
                    let price_at_mouse = py_inv(mouse.y);
                    let is_buy = chart.trigger_setup.pending_side == OrderSide::Buy;
                    let line_color = if is_buy { t.bull } else { t.bear };
                    let side_label = if is_buy { "BUY" } else { "SELL" };
                    let opt_label = &chart.trigger_setup.option_type;
                    // Horizontal line at cursor
                    painter.line_segment(
                        [egui::pos2(rect.left(), mouse.y), egui::pos2(rect.left() + cw, mouse.y)],
                        egui::Stroke::new(1.5, color_alpha(line_color, 200)));
                    // Label
                    painter.text(egui::pos2(rect.left() + cw - 120.0, mouse.y - 14.0), egui::Align2::LEFT_BOTTOM,
                        &format!("{} {} {} @ {:.2}", Icon::LIGHTNING, side_label, opt_label, price_at_mouse),
                        egui::FontId::monospace(10.0), line_color);
                    // Click to place
                    if resp.clicked() {
                        let id = chart.next_trigger_id; chart.next_trigger_id += 1;
                        let above = price_at_mouse > last_price;
                        chart.trigger_levels.push(TriggerLevel {
                            id, side: chart.trigger_setup.pending_side.clone(),
                            trigger_price: price_at_mouse, above,
                            symbol: chart.symbol.clone(),
                            option_type: chart.trigger_setup.option_type.clone(),
                            strike: chart.trigger_setup.strike,
                            expiry: chart.trigger_setup.expiry.clone(),
                            qty: chart.trigger_setup.qty,
                            submitted: false,
                        });
                        chart.trigger_setup.phase = TriggerPhase::Idle;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        chart.trigger_setup.phase = TriggerPhase::Idle;
                    }
                }
            }
        }
        // Pan chart (only when not dragging a drawing and no tool active)
        else if chart.draw_tool.is_empty() && resp.dragged_by(egui::PointerButton::Primary) {
            let d = resp.drag_delta();
            chart.vs = (chart.vs - d.x/bs).max(0.0).min(n as f32 + 200.0);
            chart.auto_scroll = false;
            chart.last_input = std::time::Instant::now();
        }

        // Scroll zoom
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 && resp.hovered() {
            let f = if scroll > 0.0 { 0.9 } else { 1.1 };
            let old = chart.vc;
            let nw = ((old as f32*f).round() as u32).max(20).min(n as u32);
            let d = (old as i32 - nw as i32) / 2;
            chart.vc = nw;
            chart.vs = (chart.vs + d as f32).max(0.0);
            chart.auto_scroll = false;
            chart.last_input = std::time::Instant::now();
        }

        // X-axis drag (bottom 18px overlay strip) — horizontal zoom
        let xaxis_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top()+pt+ch-18.0), egui::vec2(cw, 18.0));
        let xaxis_resp = ui.allocate_rect(xaxis_rect, egui::Sense::click_and_drag());
        if xaxis_resp.dragged_by(egui::PointerButton::Primary) {
            let dx = xaxis_resp.drag_delta().x;
            if dx.abs() > 1.0 {
                let f = if dx > 0.0 { 1.05_f32 } else { 0.95 };
                let old = chart.vc;
                let nw = ((old as f32*f).round() as u32).max(20).min(n as u32);
                let d = (old as i32 - nw as i32) / 2;
                chart.vc = nw; chart.vs = (chart.vs + d as f32).max(0.0);
                chart.auto_scroll = false; chart.last_input = std::time::Instant::now();
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if xaxis_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        // Y-axis drag (right strip) — vertical zoom
        let yaxis_rect = egui::Rect::from_min_size(egui::pos2(rect.left()+cw, rect.top()+pt), egui::vec2(pr, ch));
        let yaxis_resp = ui.allocate_rect(yaxis_rect, egui::Sense::click_and_drag());
        if yaxis_resp.dragged_by(egui::PointerButton::Primary) {
            let dy = yaxis_resp.drag_delta().y;
            if dy.abs() > 1.0 {
                let f = if dy > 0.0 { 1.05_f32 } else { 0.95 };
                let (lo, hi) = chart.price_range();
                let center = (lo + hi) / 2.0;
                let half = ((hi - lo) / 2.0) * f;
                chart.price_lock = Some((center - half, center + half));
                chart.auto_scroll = false; chart.last_input = std::time::Instant::now();
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        } else if yaxis_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        // Double-click Y-axis to reset price zoom
        if yaxis_resp.double_clicked() { chart.price_lock = None; }

        // Zoom selection — two phases:
        // Phase 1: zoom_start == ZERO → show magnifier cursor, wait for first click
        // Phase 2: zoom_start set → draw selection rect, complete on second click/drag-stop
        if chart.zoom_selecting {
            let has_start = chart.zoom_start != egui::Pos2::ZERO;

            if pointer_in_pane { ui.ctx().set_cursor_icon(egui::CursorIcon::ZoomIn); }

            if !has_start {
                // Phase 1: waiting for first click to set start point
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        chart.zoom_start = pos;
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) { chart.zoom_selecting = false; }
            } else {
                // Phase 2: draw selection rectangle
                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let zr = egui::Rect::from_two_pos(chart.zoom_start, pos);
                    painter.rect_filled(zr, 0.0, egui::Color32::from_rgba_unmultiplied(110,190,255,20));
                    painter.rect_stroke(zr, 0.0, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(110,190,255,180)), egui::StrokeKind::Outside);
                }
                // Complete on click or drag-stop
                if resp.clicked() || resp.drag_stopped() {
                    if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                        let sx = chart.zoom_start.x; let sy = chart.zoom_start.y;
                        if (pos.x-sx).abs() > 10.0 && (pos.y-sy).abs() > 10.0 {
                            let b_left = pos_to_bar(egui::pos2(sx.min(pos.x), 0.0));
                            let b_right = pos_to_bar(egui::pos2(sx.max(pos.x), 0.0));
                            let p_top = pos_to_price(egui::pos2(0.0, sy.min(pos.y)));
                            let p_bot = pos_to_price(egui::pos2(0.0, sy.max(pos.y)));
                            chart.vs = b_left.max(0.0);
                            chart.vc = ((b_right-b_left).ceil() as u32).max(5);
                            chart.price_lock = Some((p_bot.min(p_top), p_bot.max(p_top)));
                            chart.auto_scroll = false; chart.last_input = std::time::Instant::now();
                        }
                    }
                    chart.zoom_selecting = false;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) { chart.zoom_selecting = false; }
            }
        }

        // Delete selected drawing
        if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            if let Some(id) = chart.selected_id.take() {
                chart.drawings.retain(|d| d.id != id);
            }
        }

        // Context menu
        resp.context_menu(|ui| {
            // Get price at click position for order placement
            let click_price = ui.input(|i| i.pointer.latest_pos()).map(|p| pos_to_price(p)).unwrap_or(0.0);

            // Orders section
            ui.label(egui::RichText::new(format!("ORDERS @ {:.2}", click_price)).small().color(t.dim));
            if ui.button(egui::RichText::new(format!("{} Buy Order", Icon::ARROW_FAT_UP)).color(t.bull)).clicked() {
                let id = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id, side: OrderSide::Buy, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None });
                ui.close_menu();
            }
            if ui.button(egui::RichText::new(format!("{} Sell Order", Icon::ARROW_FAT_DOWN)).color(t.bear)).clicked() {
                let id = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id, side: OrderSide::Sell, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None });
                ui.close_menu();
            }
            if ui.button(egui::RichText::new(format!("{} Stop Loss", Icon::SHIELD_WARNING)).color(t.bear)).clicked() {
                let id = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id, side: OrderSide::Stop, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None });
                ui.close_menu();
            }
            // OCO Bracket (target +1%, stop -1%)
            if ui.button(egui::RichText::new(format!("\u{21C5} OCO Bracket")).color(egui::Color32::from_rgb(167,139,250))).clicked() {
                let target_price = click_price * 1.01;
                let stop_price = click_price * 0.99;
                let id1 = chart.next_order_id; chart.next_order_id += 1;
                let id2 = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id: id1, side: OrderSide::OcoTarget, price: target_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id2), option_symbol: None, option_con_id: None });
                chart.orders.push(OrderLevel { id: id2, side: OrderSide::OcoStop, price: stop_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id1), option_symbol: None, option_con_id: None });
                ui.close_menu();
            }
            // Trigger Order (buy entry at click, sell target +2%)
            if ui.button(egui::RichText::new(format!("\u{27F2} Trigger Order")).color(t.accent)).clicked() {
                let target_price = click_price * 1.02;
                let id1 = chart.next_order_id; chart.next_order_id += 1;
                let id2 = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id: id1, side: OrderSide::TriggerBuy, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id2), option_symbol: None, option_con_id: None });
                chart.orders.push(OrderLevel { id: id2, side: OrderSide::TriggerSell, price: target_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id1), option_symbol: None, option_con_id: None });
                ui.close_menu();
            }
            if !chart.orders.is_empty() {
                if ui.button(egui::RichText::new(format!("{} Cancel All Orders", Icon::TRASH)).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                    chart.orders.clear(); ui.close_menu();
                }
            }
            ui.separator();
            // Alerts
            ui.label(egui::RichText::new(format!("ALERTS @ {:.2}", click_price)).small().color(t.dim));
            if ui.button(format!("{} Alert Above {:.2}", Icon::ARROW_FAT_UP, click_price)).clicked() {
                // Need to access watchlist — use thread-local
                PENDING_ALERT.with(|a| *a.borrow_mut() = Some((chart.symbol.clone(), click_price, true)));
                ui.close_menu();
            }
            if ui.button(format!("{} Alert Below {:.2}", Icon::ARROW_FAT_DOWN, click_price)).clicked() {
                PENDING_ALERT.with(|a| *a.borrow_mut() = Some((chart.symbol.clone(), click_price, false)));
                ui.close_menu();
            }
            ui.separator();
            ui.label(egui::RichText::new("DRAWING TOOLS").small().color(t.dim));
            if ui.button("Draw HLine").clicked() { chart.draw_tool="hline".into(); chart.pending_pt=None; ui.close_menu(); }
            if ui.button("Draw Trendline").clicked() { chart.draw_tool="trendline".into(); chart.pending_pt=None; ui.close_menu(); }
            if ui.button("Draw Zone").clicked() { chart.draw_tool="hzone".into(); chart.pending_pt=None; ui.close_menu(); }
            if ui.button("Place Marker").clicked() { chart.draw_tool="barmarker".into(); chart.pending_pt=None; ui.close_menu(); }
            ui.separator();
            if ui.button(format!("{} Drag Zoom", Icon::MAGNIFYING_GLASS_PLUS)).clicked() { chart.zoom_selecting=true; chart.zoom_start=egui::Pos2::ZERO; ui.close_menu(); }
            if ui.button(format!("{} Measure (Shift+Drag)", Icon::RULER)).clicked() { chart.measure_active=true; chart.measure_start=None; ui.close_menu(); }
            if ui.button(format!("{} Reset View", Icon::ARROW_COUNTER_CLOCKWISE)).clicked() { chart.auto_scroll=true; chart.price_lock=None; chart.vs=(n as f32-chart.vc as f32+8.0).max(0.0); ui.close_menu(); }
            ui.separator();
            // Visibility toggles
            {
                let draw_icon = if chart.hide_all_drawings { Icon::EYE_SLASH } else { Icon::EYE };
                let draw_label = if chart.hide_all_drawings { "Show All Drawings" } else { "Hide All Drawings" };
                if ui.button(format!("{} {}", draw_icon, draw_label)).clicked() {
                    chart.hide_all_drawings = !chart.hide_all_drawings;
                    ui.close_menu();
                }
                let ind_icon = if chart.hide_all_indicators { Icon::EYE_SLASH } else { Icon::EYE };
                let ind_label = if chart.hide_all_indicators { "Show All Indicators" } else { "Hide All Indicators" };
                if ui.button(format!("{} {}", ind_icon, ind_label)).clicked() {
                    chart.hide_all_indicators = !chart.hide_all_indicators;
                    ui.close_menu();
                }
                let sig_icon = if chart.hide_signal_drawings { Icon::EYE_SLASH } else { Icon::EYE };
                let sig_label = if chart.hide_signal_drawings { "Show Signal Lines" } else { "Hide Signal Lines" };
                if ui.button(format!("{} {}", sig_icon, sig_label)).clicked() {
                    chart.hide_signal_drawings = !chart.hide_signal_drawings;
                    ui.close_menu();
                }
                if ui.button(format!("{} Add Indicator", Icon::PLUS)).clicked() {
                    let id = chart.next_indicator_id; chart.next_indicator_id += 1;
                    let color = INDICATOR_COLORS[(chart.indicators.len()) % INDICATOR_COLORS.len()];
                    chart.indicators.push(Indicator::new(id, IndicatorType::SMA, 20, color));
                    chart.indicator_bar_count = 0; // force recompute
                    chart.editing_indicator = Some(id);
                    ui.close_menu();
                }
            }
            ui.separator();
            // Groups
            ui.label(egui::RichText::new("GROUPS").small().color(t.dim));
            for g in chart.groups.clone() {
                let hidden = chart.hidden_groups.contains(&g.id);
                let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                let vis_icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                ui.horizontal(|ui| {
                    // Toggle visibility
                    if ui.add(egui::Button::new(egui::RichText::new(vis_icon).size(10.0).color(t.dim)).frame(false)).clicked() {
                        if hidden { chart.hidden_groups.retain(|x| x != &g.id); }
                        else { chart.hidden_groups.push(g.id.clone()); }
                    }
                    // Group name + count
                    ui.label(egui::RichText::new(format!("{} ({})", g.name, count)).monospace().size(10.0).color(if hidden { t.dim } else { egui::Color32::from_rgb(200,200,210) }));
                });
                // Assign selected drawings to this group
                if !chart.selected_ids.is_empty() {
                    let assign_label = format!("  {} Assign {} to {}", Icon::ARROW_FAT_DOWN, chart.selected_ids.len(), g.name);
                    if ui.button(egui::RichText::new(&assign_label).monospace().size(9.0).color(t.accent)).clicked() {
                        let ids = chart.selected_ids.clone();
                        let gid = g.id.clone();
                        let sym = chart.symbol.clone();
                        let tf = chart.timeframe.clone();
                        for d in &mut chart.drawings {
                            if ids.contains(&d.id) {
                                d.group_id = gid.clone();
                                crate::drawing_db::save(&drawing_to_db(d, &sym, &tf));
                            }
                        }
                        ui.close_menu();
                    }
                }
            }
            // Create new group
            if ui.button(format!("{} New Group...", Icon::PLUS)).clicked() {
                chart.group_manager_open = true;
                ui.close_menu();
            }
            // Delete non-default groups
            for g in chart.groups.clone() {
                if g.id != "default" {
                    let del_label = format!("  {} Delete '{}'", Icon::TRASH, g.name);
                    if ui.button(egui::RichText::new(&del_label).monospace().size(9.0).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                        // Move drawings to default, remove group
                        for d in &mut chart.drawings { if d.group_id == g.id { d.group_id = "default".into(); } }
                        chart.groups.retain(|gg| gg.id != g.id);
                        crate::drawing_db::remove_group(&g.id);
                        ui.close_menu();
                    }
                }
            }
            ui.separator();
            // Delete
            if !chart.selected_ids.is_empty() {
                if ui.button(egui::RichText::new(format!("{} Delete Selected", Icon::TRASH)).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                    let ids = chart.selected_ids.clone();
                    for id in &ids { crate::drawing_db::remove(id); }
                    chart.drawings.retain(|d| !ids.contains(&d.id));
                    chart.selected_ids.clear(); chart.selected_id=None; ui.close_menu();
                }
            }
            if !chart.drawings.is_empty() {
                if ui.button(egui::RichText::new(format!("{} Delete All Drawings", Icon::TRASH)).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                    for d in &chart.drawings { crate::drawing_db::remove(&d.id); }
                    chart.drawings.clear(); chart.selected_ids.clear(); chart.selected_id=None; ui.close_menu();
                }
                let temp_count = chart.drawings.iter().filter(|d| d.group_id == "default").count();
                if temp_count > 0 {
                    if ui.button(egui::RichText::new(format!("{} Delete All Temporary ({})", Icon::TRASH, temp_count)).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                        let to_remove: Vec<String> = chart.drawings.iter().filter(|d| d.group_id == "default").map(|d| d.id.clone()).collect();
                        for id in &to_remove { crate::drawing_db::remove(id); }
                        chart.drawings.retain(|d| d.group_id != "default");
                        chart.selected_ids.clear(); chart.selected_id = None; ui.close_menu();
                    }
                }
            }
        });

        // Double-click on order line to edit it
        if resp.double_clicked() && chart.draw_tool.is_empty() && chart.editing_order.is_none() {
            if let Some(pos) = resp.interact_pointer_pos() {
                for order in &chart.orders {
                    if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
                    if (pos.y - py(order.price)).abs() < 18.0 && pos.x < rect.left() + cw {
                        chart.editing_order = Some(order.id);
                        chart.edit_order_price = format!("{:.2}", order.price);
                        chart.edit_order_qty = format!("{}", order.qty);
                        break;
                    }
                }
            }
        }

        // Double-click on indicator line to edit it
        if resp.double_clicked() && chart.draw_tool.is_empty() && chart.editing_order.is_none() {
            if let Some(pos) = resp.interact_pointer_pos() {
                for ind in &chart.indicators {
                    if !ind.visible { continue; }
                    // Check proximity to indicator line
                    let bar_i = ((pos.x - rect.left() + off - bs * 0.5) / bs + vs) as usize;
                    for di in 0..3 {
                        let idx = if di == 0 { bar_i } else if di == 1 { bar_i.saturating_sub(1) } else { bar_i + 1 };
                        if let Some(&v) = ind.values.get(idx) {
                            if !v.is_nan() && (pos.y - py(v)).abs() < 8.0 {
                                chart.editing_indicator = Some(ind.id);
                                break;
                            }
                        }
                    }
                    if chart.editing_indicator.is_some() { break; }
                }
            }
        }

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { chart.draw_tool.clear(); chart.pending_pt = None; chart.selected_id = None; chart.editing_indicator = None; chart.editing_order = None; }

        span_end(); // interaction
        } // end for pane_idx
    });
    span_end(); // chart_panes

    // ── Handle deferred option chart open ──
    // Replaces the CURRENT (active) pane with the option chart
    if let Some((sym, strike, is_call, expiry)) = watchlist.pending_opt_chart.take() {
        let ap = *active_pane;
        let opt_sym = format!("{} {:.0}{} {}", sym, strike, if is_call { "C" } else { "P" }, expiry);
        let target = ap;
        panes[target].symbol = opt_sym;
        panes[target].is_option = true;
        panes[target].underlying = sym.clone();
        panes[target].option_type = if is_call { "C".into() } else { "P".into() };
        panes[target].option_strike = strike;
        panes[target].option_expiry = expiry;
        // Generate simulated option bars
        let underlying_bars = panes[ap].bars.clone();
        let underlying_ts = panes[ap].timestamps.clone();
        let mut opt_bars = Vec::new();
        for (i, bar) in underlying_bars.iter().enumerate() {
            let mid = (bar.open + bar.close) / 2.0;
            let intrinsic = if is_call { (mid - strike).max(0.0) } else { (strike - mid).max(0.0) };
            let time_pct = 1.0 - (i as f32 / underlying_bars.len().max(1) as f32);
            let time_val = strike * 0.005 * time_pct.max(0.1);
            let opt_mid = (intrinsic + time_val).max(0.01);
            let vol = 0.3 + (1.0 - time_pct) * 0.2; // increasing vol near expiry
            let noise = ((i as f32 * 7.3).sin() * 0.3 + (i as f32 * 13.7).cos() * 0.2) * opt_mid * 0.05;
            let o = opt_mid + noise * 0.5;
            let c = opt_mid - noise * 0.3;
            let h = o.max(c) + opt_mid * vol * 0.02;
            let l = (o.min(c) - opt_mid * vol * 0.02).max(0.01);
            opt_bars.push(Bar { open: o, high: h, low: l, close: c, volume: bar.volume * 0.1, _pad: 0.0 });
        }
        panes[target].bars = opt_bars;
        panes[target].timestamps = underlying_ts;
        panes[target].vs = (panes[target].bars.len() as f32 - panes[target].vc as f32 + 8.0).max(0.0);
        panes[target].auto_scroll = true;
        panes[target].indicator_bar_count = 0;
        *active_pane = target;
    }

    // ── Handle deferred underlying order actions ──
    // Check if any option pane requested to place an order on its underlying
    let mut und_action: Option<(usize, OrderSide, String, String, f32, String, u32)> = None;
    for (pi, pane) in panes.iter_mut().enumerate() {
        if let Some(side) = pane.pending_und_order.take() {
            und_action = Some((pi, side, pane.underlying.clone(), pane.option_type.clone(), pane.option_strike, pane.option_expiry.clone(), pane.order_qty));
        }
    }
    if let Some((source_pi, side, underlying, opt_type, strike, expiry, qty)) = und_action {
        let opt_sym = panes[source_pi].symbol.clone();
        let source_sym = panes[source_pi].symbol.clone();
        let tf = panes[0].timeframe.clone();
        let theme = panes[0].theme_idx;

        // Find or create the underlying pane
        let und_pane = panes.iter().position(|p| p.symbol == underlying && !p.is_option);
        let target_pi = if let Some(pi) = und_pane {
            pi
        } else if panes.len() <= 1 {
            *layout = Layout::TwoH;
            let mut p = Chart::new_with(&underlying, &tf);
            p.theme_idx = theme;
            p.pending_symbol_change = Some(underlying.clone());
            panes.push(p);
            panes.len() - 1
        } else {
            let other = panes.iter().position(|p| !p.is_option && p.symbol != source_sym);
            let pi = other.unwrap_or((source_pi + 1) % panes.len());
            panes[pi].pending_symbol_change = Some(underlying.clone());
            panes[pi].is_option = false;
            pi
        };

        // Place a draft order level on the underlying pane — same as regular orders
        let last = panes[target_pi].bars.last().map(|b| b.close).unwrap_or(0.0);
        let id = panes[target_pi].next_order_id;
        panes[target_pi].next_order_id += 1;
        panes[target_pi].orders.push(OrderLevel {
            id, side, price: last, qty, status: OrderStatus::Draft, pair_id: None,
            option_symbol: Some(opt_sym), option_con_id: None,
        });
        panes[target_pi].pending_confirms.push((id, std::time::Instant::now()));
        *active_pane = target_pi;
    }

    ctx.request_repaint();
}

// ─── winit + egui integration ─────────────────────────────────────────────────

/// A single native chart window with its own GPU context, panes, and layout.
// ─── Watchlist ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct WatchlistItem {
    symbol: String,
    price: f32,
    prev_close: f32,
    loaded: bool,
    // Option fields (defaults for stocks)
    is_option: bool,
    underlying: String,    // e.g. "SPY"
    option_type: String,   // "C" or "P"
    strike: f32,
    expiry: String,        // "0DTE", "5DTE" etc.
    bid: f32,
    ask: f32,
}

#[derive(Clone)]
struct WatchlistSection {
    id: u32,
    title: String,           // optional label, empty = no header shown
    color: Option<String>,   // hex bg tint, None = default
    collapsed: bool,
    items: Vec<WatchlistItem>,
}

#[derive(Clone)]
struct SavedWatchlist {
    name: String,
    sections: Vec<WatchlistSection>,
    next_section_id: u32,
}

// ─── Options chain ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct OptionRow {
    strike: f32,
    last: f32,
    bid: f32,
    ask: f32,
    volume: i32,
    oi: i32,
    iv: f32,
    itm: bool,
    contract: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SavedOption {
    contract: String,
    symbol: String,
    strike: f32,
    is_call: bool,
    expiry: String,
    last: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum WatchlistTab { Stocks, Chain }

struct Watchlist {
    open: bool,
    tab: WatchlistTab,
    sections: Vec<WatchlistSection>,
    next_section_id: u32,
    // Multi-watchlist
    saved_watchlists: Vec<SavedWatchlist>,
    active_watchlist_idx: usize,
    watchlist_name_editing: bool,
    watchlist_name_buf: String,
    #[allow(dead_code)]
    watchlist_ctx_menu_idx: Option<usize>,   // which watchlist index has context menu open
    search_query: String,
    search_results: Vec<(String, String)>,
    search_sel: i32, // -1 = none, 0+ = highlighted suggestion index
    search_refocus: bool, // request refocus on search bar after adding
    options_visible: bool, // toggle options section below stocks
    options_split: f32,    // fraction of height for stocks (0.3..0.9), rest for options
    divider_dragging: bool, // true while dragging the stocks/options divider
    divider_y: f32,        // screen Y of divider (set during render)
    divider_total_h: f32,  // total available height for split calculation
    // Drag-and-drop state
    dragging: Option<(usize, usize)>,       // (section_idx, item_idx) being dragged
    drag_start_pos: Option<egui::Pos2>,      // mouse position when drag started
    drop_target: Option<(usize, usize)>,     // (section_idx, insert_before_item_idx)
    drag_confirmed: bool,                    // true once mouse moved enough to confirm drag
    // Section editing
    renaming_section: Option<u32>,           // section id being renamed
    rename_buf: String,
    #[allow(dead_code)]
    color_picking_section: Option<u32>,      // section id picking color
    // Toolbar
    #[allow(dead_code)] toolbar_scroll: f32,
    shortcuts_open: bool, // keyboard shortcuts help panel
    trendline_filter_open: bool, // trendline filter dropdown
    account_strip_open: bool, // account summary bar below toolbar
    pending_opt_chart: Option<(String, f32, bool, String)>, // deferred option chart open
    // Orders
    orders_panel_open: bool,
    order_entry_open: bool,
    selected_order_ids: Vec<(usize, u32)>, // (pane_idx, order_id) for multi-select
    // Positions
    positions: Vec<Position>,
    // Alerts
    alerts: Vec<Alert>,
    next_alert_id: u32,
    #[allow(dead_code)]
    alert_query: String,
    // Options chain
    chain_symbol: String,
    chain_sym_input: String,
    chain_num_strikes: usize,
    chain_far_dte: i32,
    chain_0dte: (Vec<OptionRow>, Vec<OptionRow>), // (calls, puts) for 0DTE
    chain_far: (Vec<OptionRow>, Vec<OptionRow>),   // (calls, puts) for far DTE
    chain_select_mode: bool,
    chain_loading: bool,       // true while fetching chain from ApexIB
    chain_frozen: bool,        // freeze the strike window (don't move with price)
    chain_center_offset: i32,  // manual offset in strikes when frozen
    chain_last_fetch: Option<std::time::Instant>, // debounce chain refetches
    // Saved options
    saved_options: Vec<SavedOption>,
    dte_filter: i32,
}

const DEFAULT_WATCHLIST: &[&str] = &["SPY","QQQ","IWM","DIA","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOGL","GLD"];

impl Watchlist {
    fn new() -> Self {
        let (saved_watchlists, active_idx) = load_watchlists();
        let active = &saved_watchlists[active_idx];
        let sections = active.sections.clone();
        let next_section_id = active.next_section_id;
        Self { open: false, tab: WatchlistTab::Stocks, sections, next_section_id,
               saved_watchlists, active_watchlist_idx: active_idx,
               watchlist_name_editing: false, watchlist_name_buf: String::new(), watchlist_ctx_menu_idx: None,
               search_query: String::new(), search_results: vec![], search_sel: -1, search_refocus: false,
               options_visible: true, options_split: 0.6, divider_dragging: false, divider_y: 0.0, divider_total_h: 0.0,
               dragging: None, drag_start_pos: None, drop_target: None, drag_confirmed: false,
               renaming_section: None, rename_buf: String::new(), color_picking_section: None,
               toolbar_scroll: 0.0, shortcuts_open: false, trendline_filter_open: false, account_strip_open: false, pending_opt_chart: None,
               orders_panel_open: false, order_entry_open: false, selected_order_ids: vec![], positions: vec![], alerts: vec![], next_alert_id: 1, alert_query: String::new(),
               chain_symbol: "SPY".into(), chain_sym_input: String::new(), chain_num_strikes: 10, chain_far_dte: 1,
               chain_0dte: (vec![], vec![]), chain_far: (vec![], vec![]),
               chain_select_mode: false, chain_loading: false, chain_last_fetch: None, chain_frozen: false, chain_center_offset: 0,
               saved_options: vec![], dte_filter: -1 }
    }

    /// Add symbol to the last section (creates one if none exist).
    fn add_symbol(&mut self, sym: &str) {
        let s = sym.to_uppercase();
        // Check all sections for duplicates
        if self.sections.iter().any(|sec| sec.items.iter().any(|i| i.symbol == s)) { return; }
        // Find the last non-option section, or create one
        let target = self.sections.iter().rposition(|sec| !sec.title.contains("Options"));
        let target_idx = if let Some(idx) = target {
            idx
        } else {
            let id = self.next_section_id; self.next_section_id += 1;
            self.sections.insert(0, WatchlistSection { id, title: String::new(), color: None, collapsed: false, items: vec![] });
            0
        };
        self.sections[target_idx].items.push(WatchlistItem { symbol: s, price: 0.0, prev_close: 0.0, loaded: false, is_option: false, underlying: String::new(), option_type: String::new(), strike: 0.0, expiry: String::new(), bid: 0.0, ask: 0.0 });
    }

    /// Remove symbol from all sections.
    fn remove_symbol(&mut self, sym: &str) {
        for sec in &mut self.sections {
            sec.items.retain(|i| i.symbol != sym);
        }
    }

    fn set_price(&mut self, sym: &str, price: f32) {
        for sec in &mut self.sections {
            if let Some(item) = sec.items.iter_mut().find(|i| i.symbol == sym) {
                item.price = price;
            }
        }
    }

    fn set_prev_close(&mut self, sym: &str, prev_close: f32) {
        for sec in &mut self.sections {
            if let Some(item) = sec.items.iter_mut().find(|i| i.symbol == sym) {
                item.prev_close = prev_close;
                item.loaded = true;
            }
        }
    }

    /// Collect all symbols across all sections.
    fn all_symbols(&self) -> Vec<String> {
        self.sections.iter().flat_map(|s| s.items.iter().map(|i| i.symbol.clone())).collect()
    }

    /// Find an item by symbol across all sections.
    fn find_item(&self, sym: &str) -> Option<&WatchlistItem> {
        self.sections.iter().flat_map(|s| s.items.iter()).find(|i| i.symbol == sym)
    }

    /// Add a new empty section (stocks area — inserted before any options sections).
    fn add_section(&mut self, title: &str) {
        let id = self.next_section_id; self.next_section_id += 1;
        let new_sec = WatchlistSection { id, title: title.to_string(), color: None, collapsed: false, items: vec![] };
        // Insert before the first options section (so new sections go in the stocks area)
        let first_opt = self.sections.iter().position(|s| s.title.contains("Options"));
        if let Some(pos) = first_opt {
            self.sections.insert(pos, new_sec);
        } else {
            self.sections.push(new_sec);
        }
    }

    /// Add a new empty section in the options area (title contains "Options").
    fn add_option_section(&mut self, title: &str) {
        let id = self.next_section_id; self.next_section_id += 1;
        let full_title = if title.contains("Options") { title.to_string() } else { format!("{} Options", title) };
        let new_sec = WatchlistSection { id, title: full_title, color: None, collapsed: false, items: vec![] };
        self.sections.push(new_sec);
    }

    /// Add an option contract to the "Options" section (auto-creates if needed).
    /// Returns false if already present (duplicate check by symbol string).
    fn add_option_to_watchlist(&mut self, underlying: &str, strike: f32, is_call: bool, expiry: &str, bid: f32, ask: f32) -> bool {
        let type_str = if is_call { "C" } else { "P" };
        let opt_sym = format!("{} {:.0}{} {}", underlying, strike, type_str, expiry);
        // Duplicate check across all sections
        if self.sections.iter().any(|sec| sec.items.iter().any(|i| i.symbol == opt_sym)) {
            return false;
        }
        // Find or create section named after underlying (e.g. "SPY Options")
        let section_title = format!("{} Options", underlying);
        let sec_idx = if let Some(idx) = self.sections.iter().position(|s| s.title == section_title) {
            idx
        } else {
            let id = self.next_section_id; self.next_section_id += 1;
            self.sections.push(WatchlistSection {
                id, title: section_title, color: None, collapsed: false, items: vec![],
            });
            self.sections.len() - 1
        };
        self.sections[sec_idx].items.push(WatchlistItem {
            symbol: opt_sym, price: 0.0, prev_close: 0.0, loaded: false,
            is_option: true, underlying: underlying.to_string(), option_type: type_str.to_string(), strike, expiry: expiry.to_string(), bid, ask,
        });
        true
    }

    /// Move an item from (src_sec, src_idx) to (dst_sec, dst_idx).
    fn move_item(&mut self, src_sec: usize, src_idx: usize, dst_sec: usize, dst_idx: usize) {
        if src_sec >= self.sections.len() { return; }
        if src_idx >= self.sections[src_sec].items.len() { return; }
        let item = self.sections[src_sec].items.remove(src_idx);
        let dst_sec = dst_sec.min(self.sections.len() - 1);
        let clamped = dst_idx.min(self.sections[dst_sec].items.len());
        self.sections[dst_sec].items.insert(clamped, item);
    }

    /// Sync current live sections back into saved_watchlists at active index.
    fn sync_to_saved(&mut self) {
        if self.active_watchlist_idx < self.saved_watchlists.len() {
            self.saved_watchlists[self.active_watchlist_idx].sections = self.sections.clone();
            self.saved_watchlists[self.active_watchlist_idx].next_section_id = self.next_section_id;
        }
    }

    /// Save current state and persist to disk.
    fn persist(&mut self) {
        self.sync_to_saved();
        save_watchlists(self);
    }

    /// Switch to a different watchlist by index. Returns symbols needing price fetch.
    fn switch_to(&mut self, idx: usize) -> Vec<String> {
        if idx >= self.saved_watchlists.len() || idx == self.active_watchlist_idx { return vec![]; }
        // Save current
        self.sync_to_saved();
        // Load new
        self.active_watchlist_idx = idx;
        let wl = &self.saved_watchlists[idx];
        self.sections = wl.sections.clone();
        self.next_section_id = wl.next_section_id;
        // Clear prices
        for sec in &mut self.sections {
            for item in &mut sec.items {
                item.price = 0.0;
                item.prev_close = 0.0;
                item.loaded = false;
            }
        }
        save_watchlists(self);
        self.all_symbols()
    }

    /// Create a new watchlist and switch to it. Returns symbols needing price fetch.
    fn create_watchlist(&mut self, name: &str) -> Vec<String> {
        self.sync_to_saved();
        let new_wl = SavedWatchlist {
            name: name.to_string(),
            sections: vec![WatchlistSection { id: 1, title: String::new(), color: None, collapsed: false, items: vec![] }],
            next_section_id: 2,
        };
        self.saved_watchlists.push(new_wl);
        let new_idx = self.saved_watchlists.len() - 1;
        self.switch_to(new_idx)
    }

    /// Duplicate watchlist at given index. Returns symbols needing price fetch.
    fn duplicate_watchlist(&mut self, idx: usize) -> Vec<String> {
        if idx >= self.saved_watchlists.len() { return vec![]; }
        self.sync_to_saved();
        let mut dup = self.saved_watchlists[idx].clone();
        dup.name = format!("{} (copy)", dup.name);
        self.saved_watchlists.push(dup);
        let new_idx = self.saved_watchlists.len() - 1;
        self.switch_to(new_idx)
    }

    /// Delete watchlist at given index (only if more than 1 exists). Returns symbols needing price fetch if active changed.
    fn delete_watchlist(&mut self, idx: usize) -> Vec<String> {
        if self.saved_watchlists.len() <= 1 || idx >= self.saved_watchlists.len() { return vec![]; }
        self.saved_watchlists.remove(idx);
        // Adjust active index
        if self.active_watchlist_idx == idx {
            let new_idx = if idx > 0 { idx - 1 } else { 0 };
            self.active_watchlist_idx = new_idx;
            let wl = &self.saved_watchlists[new_idx];
            self.sections = wl.sections.clone();
            self.next_section_id = wl.next_section_id;
            for sec in &mut self.sections {
                for item in &mut sec.items {
                    item.price = 0.0; item.prev_close = 0.0; item.loaded = false;
                }
            }
            save_watchlists(self);
            return self.all_symbols();
        } else if self.active_watchlist_idx > idx {
            self.active_watchlist_idx -= 1;
        }
        save_watchlists(self);
        vec![]
    }

    /// Get name of the active watchlist.
    #[allow(dead_code)]
    fn active_name(&self) -> &str {
        self.saved_watchlists.get(self.active_watchlist_idx).map(|w| w.name.as_str()).unwrap_or("Default")
    }
}

// Black-Scholes, strike_interval, atm_strike, get_iv, sim_oi — now in compute.rs

fn build_chain(underlying: f32, num_strikes: usize, dte: i32) -> (Vec<OptionRow>, Vec<OptionRow>) {
    let r = 0.05_f32;
    let t = if dte == 0 { 0.5 / 252.0 } else { dte as f32 / 252.0 };
    let interval = strike_interval(underlying);
    let atm = atm_strike(underlying);

    let make_row = |k: f32, is_call: bool, _is_atm: bool| -> OptionRow {
        let iv = get_iv(underlying, k, dte);
        let raw = bs_price(underlying, k, t, r, iv, is_call);
        let spread = (raw * 0.04 + 0.005).max(0.01);
        let mid = raw.max(0.0);
        let contract = format!("{}{}{}{}",
            if is_call { "C" } else { "P" }, k as i32, if dte == 0 { "0D" } else { "1D+" }, dte);
        OptionRow {
            strike: k, last: mid, bid: (raw - spread / 2.0).max(0.0), ask: raw + spread / 2.0,
            volume: sim_oi(underlying, k, dte) / 10, oi: sim_oi(underlying, k, dte),
            iv, itm: if is_call { underlying > k } else { underlying < k }, contract,
        }
    };

    let mut calls = Vec::new();
    for i in (1..=num_strikes).rev() { calls.push(make_row(atm + i as f32 * interval, true, false)); }
    calls.push(make_row(atm, true, true));

    let mut puts = Vec::new();
    puts.push(make_row(atm, false, true));
    for i in 1..=num_strikes { puts.push(make_row(atm - i as f32 * interval, false, false)); }

    (calls, puts)
}

/// Fetch options chain from ApexIB in background. Sends ChainData command when done.
/// Falls back to simulated build_chain if the API is unreachable.
/// Shared HTTP client for ApexIB — avoids TLS handshake per request
fn apexib_client() -> &'static reqwest::blocking::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent("apex-native")
            .timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(2)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
}

fn fetch_chain_background(symbol: String, num_strikes: usize, dte: i32, underlying_price: f32) {
    std::thread::spawn(move || {
        let client = apexib_client();

        // Build expiration query param: for 0DTE use today, otherwise offset by dte days
        // Request as many strikes as the API allows (IB typically caps at ~25 per side)
        let api_strikes = 50; // request 50, API will return what it can
        let url = format!("{}/options/{}?strikeCount={}&dte={}", APEXIB_URL, symbol, api_strikes, dte);

        let send_chain = |calls: Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)>,
                          puts: Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)>| {
            let cmd = ChartCommand::ChainData {
                symbol: symbol.clone(),
                dte,
                calls,
                puts,
            };
            crate::send_to_native_chart(cmd);
        };

        match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    let parse_rows = |key: &str| -> Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)> {
                        json.get(key).and_then(|v| v.as_array()).map(|arr| {
                            arr.iter().filter_map(|row| {
                                let strike = row.get("strike")?.as_f64()? as f32;
                                let last = row.get("lastPrice").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let bid = row.get("bid").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let ask = row.get("ask").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let vol = row.get("volume").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                                let oi = row.get("openInterest").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                                let iv = row.get("iv").and_then(|v| v.as_f64())
                                    .or_else(|| row.get("impliedVolatility").and_then(|v| v.as_f64()))
                                    .unwrap_or(0.0) as f32;
                                let itm = row.get("inTheMoney").and_then(|v| v.as_bool()).unwrap_or(false);
                                let contract = row.get("contractSymbol").and_then(|v| v.as_str())
                                    .or_else(|| row.get("conId").and_then(|v| v.as_i64()).map(|_| ""))
                                    .unwrap_or("").to_string();
                                let contract = if contract.is_empty() {
                                    row.get("conId").and_then(|v| v.as_i64()).map(|id| format!("{}", id)).unwrap_or_default()
                                } else { contract };
                                Some((strike, last, bid, ask, vol, oi, iv, itm, contract))
                            }).collect()
                        }).unwrap_or_default()
                    };

                    let calls = parse_rows("calls");
                    let puts = parse_rows("puts");

                    if !calls.is_empty() || !puts.is_empty() {
                        eprintln!("[apexib] Fetched chain for {} dte={}: {} calls, {} puts", symbol, dte, calls.len(), puts.len());
                        send_chain(calls, puts);
                        return;
                    }
                }
            }
            Ok(resp) => {
                eprintln!("[apexib] Chain fetch for {} dte={} returned status {}", symbol, dte, resp.status());
            }
            Err(e) => {
                eprintln!("[apexib] Chain fetch for {} dte={} failed: {}", symbol, dte, e);
            }
        }

        // Fallback: use simulated chain
        eprintln!("[apexib] Falling back to simulated chain for {} dte={}", symbol, dte);
        if underlying_price > 0.0 {
            let (sim_calls, sim_puts) = build_chain(underlying_price, num_strikes, dte);
            let calls: Vec<_> = sim_calls.iter().map(|r| (r.strike, r.last, r.bid, r.ask, r.volume, r.oi, r.iv, r.itm, r.contract.clone())).collect();
            let puts: Vec<_> = sim_puts.iter().map(|r| (r.strike, r.last, r.bid, r.ask, r.volume, r.oi, r.iv, r.itm, r.contract.clone())).collect();
            send_chain(calls, puts);
        }
    });
}

/// Fetch symbol search results from ApexIB in background.
fn fetch_search_background(query: String, source: String) {
    std::thread::spawn(move || {
        let client = apexib_client();

        let url = format!("{}/search/{}", APEXIB_URL, query);
        let mut results: Vec<(String, String)> = Vec::new();

        if let Ok(resp) = client.get(&url).send() {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(arr) = json.as_array() {
                        for item in arr.iter().take(16) {
                            if let Some(sym) = item.get("symbol").and_then(|v| v.as_str()) {
                                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                results.push((sym.to_string(), name));
                            }
                        }
                    }
                }
            }
        }

        if !results.is_empty() {
            let cmd = ChartCommand::SearchResults {
                query,
                results,
                source,
            };
            crate::send_to_native_chart(cmd);
        }
    });
}

/// Fetch daily previous close for all watchlist symbols (background thread).
/// Tries ApexIB first (bars endpoint), falls back to Yahoo Finance.
fn fetch_watchlist_prices(symbols: Vec<String>) {
    std::thread::spawn(move || {
        let ib_client = apexib_client();
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0")
            .timeout(std::time::Duration::from_secs(5))
            .build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        for sym in &symbols {
            // Try Redis cache first
            if let Some(bars) = crate::bar_cache::get(sym, "1d") {
                if bars.len() >= 2 {
                    let price = bars.last().map(|b| b.close as f32).unwrap_or(0.0);
                    let prev = bars[bars.len()-2].close as f32;
                    let cmd = ChartCommand::WatchlistPrice { symbol: sym.clone(), price, prev_close: prev };
                    crate::send_to_native_chart(cmd);
                    continue;
                }
            }

            // Try ApexIB bars endpoint
            let apexib_ok = (|| -> Option<()> {
                let url = format!("{}/bars/{}?interval=1d&limit=2", APEXIB_URL, sym);
                let resp = client.get(&url).send().ok()?;
                if !resp.status().is_success() { return None; }
                let json = resp.json::<serde_json::Value>().ok()?;
                let bars = json.as_array()?;
                if bars.len() < 2 { return None; }
                let prev = bars[0].get("close").and_then(|v| v.as_f64())? as f32;
                let price = bars[1].get("close").and_then(|v| v.as_f64())? as f32;
                let cmd = ChartCommand::WatchlistPrice { symbol: sym.clone(), price, prev_close: prev };
                crate::send_to_native_chart(cmd);
                Some(())
            })();

            if apexib_ok.is_some() { continue; }

            // Fallback: Yahoo Finance
            let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d", sym);
            if let Ok(resp) = client.get(&url).send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                        crate::bar_cache::set(sym, "1d", &bars);
                        if bars.len() >= 2 {
                            let price = bars.last().map(|b| b.close as f32).unwrap_or(0.0);
                            let prev = bars[bars.len()-2].close as f32;
                            let cmd = ChartCommand::WatchlistPrice { symbol: sym.clone(), price, prev_close: prev };
                            crate::send_to_native_chart(cmd);
                        }
                    }
                }
            }
        }
    });
}

struct ChartWindow {
    id: winit::window::WindowId,
    win: Arc<Window>,
    gpu: GpuCtx,
    rx: mpsc::Receiver<ChartCommand>,
    panes: Vec<Chart>,
    active_pane: usize,
    layout: Layout,
    close_requested: bool,
    watchlist: Watchlist,
    // Order execution toasts
    toasts: Vec<(String, f32, std::time::Instant, bool)>, // (message, price, created, is_buy)
    // Connection panel
    conn_panel_open: bool,
}

/// Request to spawn a new window (sent from Tauri command thread).
struct SpawnRequest {
    rx: mpsc::Receiver<ChartCommand>,
    initial_cmd: ChartCommand,
}

/// Top-level app managing multiple chart windows on a single EventLoop.
struct App {
    app_handle: Option<tauri::AppHandle>,
    iw: u32, ih: u32,
    windows: Vec<ChartWindow>,
    spawn_rx: mpsc::Receiver<SpawnRequest>,
}

struct GpuCtx {
    device: wgpu::Device, queue: wgpu::Queue,
    surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl GpuCtx {
    fn new(window: Arc<Window>) -> Option<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..Default::default() });
        let surface = instance.create_surface(Arc::clone(&window)).ok()?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), force_fallback_adapter: false,
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("chart"), memory_hints: wgpu::MemoryHints::Performance, ..Default::default()
        }, None)).ok()?;
        let caps = surface.get_capabilities(&adapter);
        let fmt = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
        // Fifo (vsync) + frame latency 2 = smooth consistent frame pacing.
        // Latency 2 lets us pipeline: CPU prepares frame N+1 while GPU presents frame N.
        // This eliminates the 10ms acquire stalls we had with latency 1.
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            wgpu::PresentMode::AutoVsync
        };
        eprintln!("[native-chart] PresentMode::{:?}, frame latency 2", present_mode);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format: fmt,
            width: size.width.max(1), height: size.height.max(1),
            present_mode, alpha_mode: caps.alpha_modes[0],
            view_formats: vec![], desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
        ui_kit::icons::init_icons(&egui_ctx);
        start_account_poller();
        let egui_state = egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, &*window, Some(window.scale_factor() as f32), None, None);
        let egui_renderer = egui_wgpu::Renderer::new(&device, fmt, None, 1, false);

        Some(Self { device, queue, surface, config, egui_ctx, egui_state, egui_renderer })
    }

    fn render(&mut self, window: &Window, panes: &mut Vec<Chart>, active_pane: &mut usize, layout: &mut Layout, watchlist: &mut Watchlist, toasts: &[(String, f32, std::time::Instant, bool)], conn_panel_open: &mut bool, rx: &mpsc::Receiver<ChartCommand>) {
        crate::monitoring::frame_begin();

        // Phase 1: Acquire surface texture
        let t0 = std::time::Instant::now();
        let output = match self.surface.get_current_texture() {
            Ok(t) => t, Err(_) => { self.surface.configure(&self.device, &self.config); return; }
        };
        let view = output.texture.create_view(&Default::default());
        let acquire_us = t0.elapsed().as_micros() as u64;

        // Phase 2: egui layout + draw_chart logic
        let t1 = std::time::Instant::now();
        let raw_input = self.egui_state.take_egui_input(window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| { draw_chart(ctx, panes, active_pane, layout, watchlist, toasts, conn_panel_open, rx); });
        self.egui_state.handle_platform_output(window, full_output.platform_output);
        let layout_us = t1.elapsed().as_micros() as u64;

        // Phase 3: Tessellation
        let t2 = std::time::Instant::now();
        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let tessellate_us = t2.elapsed().as_micros() as u64;

        // Collect render stats
        let num_paint_jobs = paint_jobs.len() as u32;
        let mut total_vertices = 0u32;
        let mut total_indices = 0u32;
        for job in &paint_jobs {
            if let egui::epaint::Primitive::Mesh(mesh) = &job.primitive {
                total_vertices += mesh.vertices.len() as u32;
                total_indices += mesh.indices.len() as u32;
            }
        }
        let texture_uploads = full_output.textures_delta.set.len() as u32;
        let texture_frees = full_output.textures_delta.free.len() as u32;

        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [self.config.width, self.config.height], pixels_per_point: full_output.pixels_per_point };

        // Phase 4: GPU upload (textures + buffers)
        let t3 = std::time::Instant::now();
        for (id, delta) in &full_output.textures_delta.set { self.egui_renderer.update_texture(&self.device, &self.queue, *id, delta); }
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut enc, &paint_jobs, &sd);
        self.queue.submit(std::iter::once(enc.finish()));
        let upload_us = t3.elapsed().as_micros() as u64;

        // Phase 5: Render pass
        let t4 = std::time::Instant::now();
        let mut enc2 = self.device.create_command_encoder(&Default::default());
        let mut pass = enc2.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view, resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
        }).forget_lifetime();
        self.egui_renderer.render(&mut pass, &paint_jobs, &sd);
        drop(pass);
        self.queue.submit(std::iter::once(enc2.finish()));
        let render_us = t4.elapsed().as_micros() as u64;

        // Phase 6: Present
        let t5 = std::time::Instant::now();
        for id in &full_output.textures_delta.free { self.egui_renderer.free_texture(id); }
        output.present();
        let present_us = t5.elapsed().as_micros() as u64;

        // Report all phase timings + render stats
        crate::monitoring::frame_end_detailed(crate::monitoring::FramePhases {
            acquire_us, layout_us, tessellate_us, upload_us, render_us, present_us,
            paint_jobs: num_paint_jobs, vertices: total_vertices, indices: total_indices,
            texture_uploads, texture_frees,
        });
    }
}

impl App {
    fn spawn_window(&mut self, el: &ActiveEventLoop, rx: mpsc::Receiver<ChartCommand>, initial_cmd: Option<ChartCommand>) {
        let w = match el.create_window(WindowAttributes::default()
            .with_title("Apex Terminal")
            .with_inner_size(PhysicalSize::new(self.iw, self.ih))
            .with_min_inner_size(PhysicalSize::new(960, 540))
            .with_decorations(false)
            .with_window_icon(make_window_icon())
            .with_active(true)
            .with_maximized(true))
        {
            Ok(w) => {
                // Enable rounded corners on Windows 11 (DWM)
                #[cfg(target_os = "windows")]
                {
                    use winit::raw_window_handle::HasWindowHandle;
                    if let Ok(handle) = w.window_handle() {
                        if let winit::raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
                            unsafe {
                                let hwnd = h.hwnd.get() as *mut std::ffi::c_void;

                                // Ensure WS_EX_APPWINDOW so taskbar shows our icon
                                let ex_style = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(hwnd, -20);
                                windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongW(hwnd, -20, ex_style | 0x00040000);

                                // DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2
                                let preference: u32 = 2;
                                let _ = windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                                    hwnd,
                                    33,
                                    &preference as *const u32 as *const _,
                                    std::mem::size_of::<u32>() as u32,
                                );
                            }
                        }
                    }
                }
                // Set window icon (taskbar + alt-tab)
                if let Some(icon) = make_window_icon() {
                    w.set_window_icon(Some(icon));
                }
                // Also set via Win32 WM_SETICON for reliable taskbar display
                #[cfg(target_os = "windows")]
                {
                    use winit::raw_window_handle::HasWindowHandle;
                    if let Ok(handle) = w.window_handle() {
                        if let winit::raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
                            if let Some(hicon) = make_window_icon_hicon() {
                                unsafe {
                                    let hwnd_msg = h.hwnd.get() as *mut std::ffi::c_void;
                                    // WM_SETICON: ICON_BIG=1, ICON_SMALL=0
                                    windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(hwnd_msg, 0x0080, 1, hicon);
                                    windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(hwnd_msg, 0x0080, 0, hicon);
                                    // Set on window CLASS — this is what Win11 taskbar reads
                                    // GCLP_HICON = -14, GCLP_HICONSM = -34
                                    windows_sys::Win32::UI::WindowsAndMessaging::SetClassLongPtrW(hwnd_msg, -14, hicon as _);
                                    windows_sys::Win32::UI::WindowsAndMessaging::SetClassLongPtrW(hwnd_msg, -34, hicon as _);
                                }
                            }
                        }
                    }
                }

                Arc::new(w)
            }
            Err(e) => { eprintln!("[native-chart] Window creation failed: {e}"); return; }
        };
        let gpu = match GpuCtx::new(Arc::clone(&w)) {
            Some(g) => g,
            None => { eprintln!("[native-chart] GPU init failed"); return; }
        };
        let id = w.id();
        let (panes, layout) = load_state();
        let wl = Watchlist::new();
        let wl_syms: Vec<String> = wl.all_symbols();
        let mut cw = ChartWindow { id, win: w, gpu, rx, panes, active_pane: 0, layout, close_requested: false, watchlist: wl, toasts: vec![], conn_panel_open: false };
        // Fetch prices for default watchlist symbols
        fetch_watchlist_prices(wl_syms);
        if let Some(cmd) = initial_cmd {
            // Route initial LoadBars to first pane
            if let Some(p) = cw.panes.first_mut() { p.process(cmd); }
        }
        self.windows.push(cw);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        // On first resume, check for pending spawn request
        if self.windows.is_empty() {
            if let Ok(req) = self.spawn_rx.try_recv() {
                self.spawn_window(el, req.rx, Some(req.initial_cmd));
            }
        }
    }
    fn window_event(&mut self, _el: &ActiveEventLoop, wid: winit::window::WindowId, ev: WindowEvent) {
        let cw = match self.windows.iter_mut().find(|w| w.id == wid) { Some(w) => w, None => return };
        let _ = cw.gpu.egui_state.on_window_event(&cw.win, &ev);
        match ev {
            WindowEvent::CloseRequested => {
                save_state(&cw.panes, cw.layout);
                cw.watchlist.persist();
                self.windows.retain(|w| w.id != wid);
            }
            WindowEvent::Resized(s) => {
                if s.width>0&&s.height>0 {
                    cw.gpu.config.width=s.width; cw.gpu.config.height=s.height;
                    cw.gpu.surface.configure(&cw.gpu.device, &cw.gpu.config);
                    cw.win.request_redraw(); // immediate redraw on resize
                }
            }
            WindowEvent::RedrawRequested => {
                // Drain watchlist price updates before render
                // (these come via the broadcast channel from fetch_watchlist_prices)
                let mut cmds_to_requeue = Vec::new();
                while let Ok(cmd) = cw.rx.try_recv() {
                    match cmd {
                        ChartCommand::WatchlistPrice { ref symbol, price, prev_close } => {
                            cw.watchlist.set_price(symbol, price);
                            cw.watchlist.set_prev_close(symbol, prev_close);
                        }
                        ChartCommand::ChainData { ref symbol, dte, ref calls, ref puts } => {
                            if *symbol == cw.watchlist.chain_symbol {
                                let to_rows = |data: &[(f32,f32,f32,f32,i32,i32,f32,bool,String)]| -> Vec<OptionRow> {
                                    data.iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                                        strike: *strike, last: *last, bid: *bid, ask: *ask,
                                        volume: *vol, oi: *oi, iv: *iv, itm: *itm, contract: contract.clone(),
                                    }).collect()
                                };
                                if dte == 0 {
                                    cw.watchlist.chain_0dte = (to_rows(calls), to_rows(puts));
                                } else {
                                    cw.watchlist.chain_far = (to_rows(calls), to_rows(puts));
                                }
                                cw.watchlist.chain_loading = false;
                            }
                        }
                        ChartCommand::SearchResults { ref query, ref results, ref source } => {
                            // Only apply if query still matches current search
                            if source == "watchlist" && !query.is_empty()
                                && cw.watchlist.search_query.to_lowercase().starts_with(&query.to_lowercase()) {
                                // Merge: keep static results and append API results that aren't already present
                                for (sym, name) in results {
                                    if !cw.watchlist.search_results.iter().any(|(s, _)| s == sym) {
                                        cw.watchlist.search_results.push((sym.clone(), name.clone()));
                                    }
                                }
                            } else if source == "chain" && !query.is_empty()
                                && cw.watchlist.chain_sym_input.to_lowercase().starts_with(&query.to_lowercase()) {
                                for (sym, name) in results {
                                    if !cw.watchlist.search_results.iter().any(|(s, _)| s == sym) {
                                        cw.watchlist.search_results.push((sym.clone(), name.clone()));
                                    }
                                }
                            }
                        }
                        other => cmds_to_requeue.push(other),
                    }
                }
                // Re-inject non-watchlist commands (they'll be picked up by draw_chart)
                // Can't re-send to rx since we own the receiver. Use a temp buffer approach:
                // Actually, draw_chart also drains rx. So we need to pass these through.
                // Simpler: just process ALL commands here and pass pane commands to the right pane.
                for cmd in cmds_to_requeue {
                    let sym = match &cmd {
                        ChartCommand::LoadBars { symbol, .. } | ChartCommand::UpdateLastBar { symbol, .. } | ChartCommand::AppendBar { symbol, .. } | ChartCommand::PrependBars { symbol, .. } | ChartCommand::LoadDrawings { symbol, .. } => Some(symbol.clone()),
                        ChartCommand::IndicatorSourceBars { .. } => None,
                        _ => None,
                    };
                    if let Some(s) = sym {
                        if let Some(p) = cw.panes.iter_mut().find(|p| p.symbol == s) { p.process(cmd); }
                        else if let Some(p) = cw.panes.get_mut(cw.active_pane) { p.process(cmd); }
                    } else if let Some(p) = cw.panes.get_mut(cw.active_pane) { p.process(cmd); }
                }

                // Also update watchlist from tick data (UpdateLastBar contains current price)
                for sec in &mut cw.watchlist.sections {
                    for item in &mut sec.items {
                        // Check if any pane has this symbol and get its latest price
                        if let Some(pane) = cw.panes.iter().find(|p| p.symbol == item.symbol) {
                            if let Some(bar) = pane.bars.last() {
                                item.price = bar.close;
                            }
                        }
                    }
                }

                CURRENT_WINDOW.with(|w| *w.borrow_mut() = Some(Arc::clone(&cw.win)));
                CLOSE_REQUESTED.with(|f| f.set(false));
                cw.gpu.render(&cw.win, &mut cw.panes, &mut cw.active_pane, &mut cw.layout, &mut cw.watchlist, &cw.toasts, &mut cw.conn_panel_open, &cw.rx);
                CURRENT_WINDOW.with(|w| *w.borrow_mut() = None);
                if CLOSE_REQUESTED.with(|f| f.get()) {
                    cw.close_requested = true;
                }
                // Process pending alerts from context menu
                if let Some((sym, price, above)) = PENDING_ALERT.with(|a| a.borrow_mut().take()) {
                    let id = cw.watchlist.next_alert_id; cw.watchlist.next_alert_id += 1;
                    cw.watchlist.alerts.push(Alert { id, symbol: sym, price, above, triggered: false, message: String::new() });
                }
                // Collect order execution toasts
                let new_toasts = PENDING_TOASTS.with(|ts| {
                    let mut v = ts.borrow_mut();
                    let r = v.drain(..).collect::<Vec<_>>();
                    r
                });
                for (msg, price, is_buy) in new_toasts {
                    cw.toasts.push((msg, price, std::time::Instant::now(), is_buy));
                }
                // Remove expired toasts (>5 seconds)
                cw.toasts.retain(|(_, _, created, _)| created.elapsed().as_secs() < 5);
            }
            // Don't request_redraw on every input — about_to_wait drives the frame loop
            _ => {}
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // Check for new window spawn requests
        while let Ok(req) = self.spawn_rx.try_recv() {
            self.spawn_window(el, req.rx, Some(req.initial_cmd));
        }

        // Remove windows that requested close
        self.windows.retain(|w| !w.close_requested);

        // Handle symbol/timeframe changes + frame rate for ALL windows
        for cw in &mut self.windows {
            for pane in &mut cw.panes {
                let sym_change = pane.pending_symbol_change.take();
                let tf_change = pane.pending_timeframe_change.take();
                if sym_change.is_some() || tf_change.is_some() {
                    if let Some(sym) = sym_change { pane.symbol = sym; }
                    if let Some(tf) = tf_change { pane.timeframe = tf; }

                    let sym = pane.symbol.clone();
                    let tf = pane.timeframe.clone();
                    eprintln!("[native-chart] Loading {} {}", sym, tf);

                    pane.bars.clear();
                    pane.timestamps.clear();
                    pane.indicators.clear();
                    pane.drawings.clear(); // cleared here, reloaded when LoadBars arrives
                    pane.drawings_requested = false; // allow re-fetch for new timeframe
                    pane.history_loading = false;
                    pane.history_exhausted = false;
                    pane.sim_price = 0.0;
                    pane.last_candle_time = std::time::Instant::now();

                    if let Some(handle) = &self.app_handle {
                        use tauri::Emitter;
                        let _ = handle.emit("native-chart-load", serde_json::json!({
                            "symbol": sym, "timeframe": tf,
                        }));
                    }

                    fetch_bars_background(sym, tf);
                }
            }

            // Request redraw — Fifo vsync naturally caps at display refresh rate
            cw.win.request_redraw();
        }

        // Poll mode so about_to_wait fires again immediately after each frame.
        // Fifo present mode blocks in get_current_texture() until vsync,
        // so this won't spin — it gives us: about_to_wait → redraw → vsync wait → repeat.
        el.set_control_flow(winit::event_loop::ControlFlow::Poll);
    }
}

/// Fetch source bars for a cross-timeframe indicator on a background thread.
fn fetch_indicator_source(sym: String, tf: String, indicator_id: u32) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }
    std::thread::spawn(move || {
        // Try Redis cache first
        if let Some(bars) = crate::bar_cache::get(&sym, &tf) {
            if !bars.is_empty() {
                let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                    open: b.open as f32, high: b.high as f32, low: b.low as f32,
                    close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                }).collect();
                let cmd = ChartCommand::IndicatorSourceBars { indicator_id, timeframe: tf.clone(), bars: gpu_bars, timestamps };
                for tx in &txs { let _ = tx.send(cmd.clone()); }
                return;
            }
        }
        // Fetch from Yahoo Finance
        let (yf_interval, yf_range) = match tf.as_str() {
            "1m" => ("1m","5d"), "5m" => ("5m","5d"), "15m" => ("15m","60d"),
            "30m" => ("30m","60d"), "1h" => ("60m","60d"), "4h" => ("1h","730d"),
            "1d" => ("1d","5y"), "1wk" => ("1wk","10y"), _ => ("5m","5d"),
        };
        let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}", sym, yf_interval, yf_range);
        let client = reqwest::blocking::Client::builder().user_agent("Mozilla/5.0").build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(5)).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                    crate::bar_cache::set(&sym, &tf, &bars);
                    let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                    let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                        open: b.open as f32, high: b.high as f32, low: b.low as f32,
                        close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                    }).collect();
                    let cmd = ChartCommand::IndicatorSourceBars { indicator_id, timeframe: tf, bars: gpu_bars, timestamps };
                    for tx in &txs { let _ = tx.send(cmd.clone()); }
                }
            }
        }
    });
}

/// Fetch older historical bars before `before_ts` and deliver as PrependBars.
/// Submit an order to ApexIB. Called from background thread.
fn submit_ib_order(symbol: &str, side: &str, qty: u32, order_type_idx: usize, tif_idx: usize, price: f32, bracket: bool, tp: Option<f32>, sl: Option<f32>) {
    let order_type = match order_type_idx { 0 => "market", 1 => "limit", 2 => "stop", 3 => "stop_limit", 4 => "trailing_stop", _ => "market" };
    let tif = match tif_idx { 0 => "day", 1 => "gtc", 2 => "ioc", _ => "day" };
    let client = reqwest::blocking::Client::new();

    // First resolve symbol to conId
    let con_id = match client.get(format!("{}/contract/{}", APEXIB_URL, symbol))
        .timeout(std::time::Duration::from_secs(5)).send()
        .and_then(|r| r.json::<serde_json::Value>()) {
        Ok(json) => json["conId"].as_i64().unwrap_or(0),
        Err(e) => { eprintln!("[order] contract resolve failed: {e}"); return; }
    };
    if con_id == 0 { eprintln!("[order] no conId for {}", symbol); return; }

    if bracket {
        if let (Some(tp_price), Some(sl_price)) = (tp, sl) {
            let body = serde_json::json!({
                "conId": con_id, "side": side, "quantity": qty,
                "orderType": order_type, "limitPrice": price, "tif": tif,
                "takeProfitPrice": tp_price, "stopLossPrice": sl_price
            });
            match client.post(format!("{}/orders/bracket", APEXIB_URL))
                .json(&body).timeout(std::time::Duration::from_secs(5)).send() {
                Ok(resp) => eprintln!("[order] bracket {} {} x{} → {}", side, symbol, qty, resp.status()),
                Err(e) => eprintln!("[order] bracket failed: {e}"),
            }
        }
    } else {
        let mut body = serde_json::json!({
            "conId": con_id, "side": side, "quantity": qty,
            "orderType": order_type, "tif": tif
        });
        if order_type != "market" {
            body["limitPrice"] = serde_json::json!(price);
        }
        match client.post(format!("{}/orders", APEXIB_URL))
            .json(&body).timeout(std::time::Duration::from_secs(5)).send() {
            Ok(resp) => eprintln!("[order] {} {} x{} @ {:.2} → {}", side, symbol, qty, price, resp.status()),
            Err(e) => eprintln!("[order] submit failed: {e}"),
        }
    }
}

fn fetch_history_background(sym: String, tf: String, before_ts: i64) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }

    std::thread::spawn(move || {
        // Calculate how far back to fetch based on timeframe
        let page_seconds: i64 = match tf.as_str() {
            "1m" => 86400 * 2,        // 2 days
            "2m" => 86400 * 3,        // 3 days
            "5m" => 86400 * 5,        // 5 days
            "15m" => 86400 * 30,      // 30 days
            "30m" => 86400 * 30,      // 30 days
            "1h" | "60m" => 86400 * 60, // 60 days
            "4h" => 86400 * 180,      // 6 months
            "1d" => 86400 * 365 * 2,  // 2 years
            "1wk" => 86400 * 365 * 5, // 5 years
            _ => 86400 * 5,
        };

        let period2 = before_ts;
        let period1 = before_ts - page_seconds;
        let yf_interval = match tf.as_str() {
            "1h" => "60m", "4h" => "1h",
            other => other,
        };

        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&period1={}&period2={}",
            sym, yf_interval, period1, period2
        );
        eprintln!("[history] fetching {} {} before {} ({}..{})", sym, tf, before_ts, period1, period2);

        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0")
            .build().unwrap_or_else(|_| reqwest::blocking::Client::new());

        match client.get(&url).timeout(std::time::Duration::from_secs(10)).send() {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                        let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                            open: b.open as f32, high: b.high as f32, low: b.low as f32,
                            close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                        }).collect();
                        let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                        eprintln!("[history] got {} bars for {} {} (oldest: {})", gpu_bars.len(), sym, tf,
                            timestamps.first().copied().unwrap_or(0));

                        let cmd = ChartCommand::PrependBars {
                            symbol: sym, timeframe: tf, bars: gpu_bars, timestamps,
                        };
                        for tx in &txs { let _ = tx.send(cmd.clone()); }
                        return;
                    }
                }
            }
            Err(e) => eprintln!("[history] fetch error: {e}"),
        }

        // On failure, send empty to clear loading flag and mark exhausted
        let cmd = ChartCommand::PrependBars {
            symbol: sym, timeframe: tf, bars: vec![], timestamps: vec![],
        };
        for tx in &txs { let _ = tx.send(cmd.clone()); }
    });
}

/// Fetch bars from Redis cache → OCOCO → yfinance sidecar → Yahoo Finance v8 on a background thread.
/// Sends LoadBars command via the global NATIVE_CHART_TXS channels (all windows).
/// Results are cached in Redis for subsequent requests.
/// Load drawings from DB — uses the single DB worker thread, no per-call runtime.
fn fetch_drawings_background(sym: String) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }

    // Spawn a thread that sends requests to the DB worker and waits for replies.
    // The DB worker is a single thread with a single tokio runtime — no pool exhaustion.
    std::thread::spawn(move || {
        let db_drawings = crate::drawing_db::load_symbol(&sym);
        let drawings: Vec<Drawing> = db_drawings.iter().filter_map(|dd| db_to_drawing(dd)).collect();
        let db_groups = crate::drawing_db::load_groups();
        let groups: Vec<super::DrawingGroup> = db_groups.into_iter()
            .map(|(id, name, color)| super::DrawingGroup { id, name, color }).collect();
        let cmd = ChartCommand::LoadDrawings { symbol: sym, drawings, groups };
        for tx in &txs { let _ = tx.send(cmd.clone()); }
    });
}

/// Public entry point for standalone binary to trigger initial data load.
pub fn fetch_bars_background_pub(sym: String, tf: String) { fetch_bars_background(sym, tf); }

fn fetch_bars_background(sym: String, tf: String) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.clone())
        .unwrap_or_default();
    if txs.is_empty() { return; }
    std::thread::spawn(move || {
        let send_bars = |bars: &[crate::data::Bar], src: &str| -> bool {
            if bars.is_empty() { return false; }
            let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                open: b.open as f32, high: b.high as f32, low: b.low as f32,
                close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
            }).collect();
            let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
            eprintln!("[native-chart] {} bars for {} {} from {}", gpu_bars.len(), sym, tf, src);
            let cmd = ChartCommand::LoadBars { symbol: sym.clone(), timeframe: tf.clone(), bars: gpu_bars, timestamps };
            for tx in &txs { let _ = tx.send(cmd.clone()); }
            true
        };

        // 0. Redis cache — instant
        if let Some(cached) = crate::bar_cache::get(&sym, &tf) {
            if send_bars(&cached, "Redis cache") { return; }
        }

        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0")
            .build().unwrap_or_else(|_| reqwest::blocking::Client::new());

        // 1. OCOCO (InfluxDB cache)
        let ococo_url = format!("http://192.168.1.60:30300/api/bars?symbol={}&interval={}&limit=500", sym, tf);
        if let Ok(resp) = client.get(&ococo_url).timeout(std::time::Duration::from_secs(2)).send() {
            if let Ok(bars) = resp.json::<Vec<crate::data::Bar>>() {
                if !bars.is_empty() {
                    crate::bar_cache::set(&sym, &tf, &bars);
                    if send_bars(&bars, "OCOCO") { return; }
                }
            }
        }

        // 2. yfinance sidecar
        let (yf_interval, yf_range) = match tf.as_str() {
            "1m" => ("1m","5d"), "2m" => ("2m","5d"), "5m" => ("5m","5d"),
            "15m" => ("15m","60d"), "30m" => ("30m","60d"), "1h" => ("60m","60d"),
            "4h" => ("1h","730d"), "1d" => ("1d","5y"), "1wk" => ("1wk","10y"),
            _ => ("5m","5d"),
        };
        let yf_url = format!("http://127.0.0.1:8777/bars?symbol={}&interval={}&period={}", sym, yf_interval, yf_range);
        if let Ok(resp) = client.get(&yf_url).timeout(std::time::Duration::from_secs(3)).send() {
            if let Ok(bars) = resp.json::<Vec<crate::data::Bar>>() {
                if !bars.is_empty() {
                    crate::bar_cache::set(&sym, &tf, &bars);
                    if send_bars(&bars, "yfinance-sidecar") { return; }
                }
            }
        }

        // 3. Direct Yahoo Finance v8 API — universal fallback
        let yahoo_url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}",
            sym, yf_interval, yf_range
        );
        if let Ok(resp) = client.get(&yahoo_url).timeout(std::time::Duration::from_secs(5)).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                    crate::bar_cache::set(&sym, &tf, &bars);
                    send_bars(&bars, "Yahoo Finance");
                }
            }
        }
    });
}

// ─── State persistence ───────────────────────────────────────────────────────

fn state_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push("native-chart-state.json");
    p
}

fn save_state(panes: &[Chart], layout: Layout) {
    // Don't persist option chart panes — they use placeholder data
    let pane_data: Vec<serde_json::Value> = panes.iter().filter(|p| !p.is_option).map(|p| serde_json::json!({
        "symbol": p.symbol, "timeframe": p.timeframe,
    })).collect();
    let state = serde_json::json!({
        "layout": layout.label(),
        "theme_idx": panes.first().map(|p| p.theme_idx).unwrap_or(5),
        "panes": pane_data,
        "recent_symbols": panes.first().map(|p| &p.recent_symbols).cloned().unwrap_or_default(),
    });
    let _ = std::fs::write(state_path(), serde_json::to_string_pretty(&state).unwrap_or_default());
}

fn load_state() -> (Vec<Chart>, Layout) {
    let path = state_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return (vec![Chart::new()], Layout::One),
    };
    let json: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return (vec![Chart::new()], Layout::One),
    };

    let layout = match json.get("layout").and_then(|v| v.as_str()).unwrap_or("1") {
        "2" => Layout::Two, "2H" => Layout::TwoH, "3" => Layout::Three, "4" => Layout::Four,
        "6" => Layout::Six, "6H" => Layout::SixH, "9" => Layout::Nine, _ => Layout::One,
    };
    let theme_idx = json.get("theme_idx").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let recents: Vec<(String, String)> = json.get("recent_symbols").and_then(|v| v.as_array()).map(|arr| {
        arr.iter().filter_map(|v| {
            let a = v.as_array()?;
            Some((a.first()?.as_str()?.to_string(), a.get(1)?.as_str()?.to_string()))
        }).collect()
    }).unwrap_or_default();

    let pane_arr = json.get("panes").and_then(|v| v.as_array());
    let mut panes = Vec::new();
    if let Some(arr) = pane_arr {
        for p in arr {
            let sym = p.get("symbol").and_then(|v| v.as_str()).unwrap_or("AAPL");
            let tf = p.get("timeframe").and_then(|v| v.as_str()).unwrap_or("5m");
            let mut chart = Chart::new_with(sym, tf);
            chart.theme_idx = theme_idx;
            chart.recent_symbols = recents.clone();
            chart.pending_symbol_change = Some(sym.to_string());
            panes.push(chart);
        }
    }
    if panes.is_empty() { panes.push(Chart::new()); }
    // Trim excess panes to match layout capacity
    let max = layout.max_panes();
    panes.truncate(max);

    (panes, layout)
}

fn watchlists_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push("watchlists.json");
    p
}

fn save_watchlists(watchlist: &Watchlist) {
    let wls: Vec<serde_json::Value> = watchlist.saved_watchlists.iter().map(|wl| {
        let sections: Vec<serde_json::Value> = wl.sections.iter().map(|sec| {
            let items: Vec<serde_json::Value> = sec.items.iter().map(|item| {
                if item.is_option {
                    serde_json::json!({ "symbol": item.symbol, "is_option": true, "underlying": item.underlying, "option_type": item.option_type, "strike": item.strike, "expiry": item.expiry, "bid": item.bid, "ask": item.ask })
                } else {
                    serde_json::json!({ "symbol": item.symbol })
                }
            }).collect();
            serde_json::json!({
                "id": sec.id,
                "title": sec.title,
                "color": sec.color,
                "collapsed": sec.collapsed,
                "items": items,
            })
        }).collect();
        serde_json::json!({
            "name": wl.name,
            "sections": sections,
            "next_section_id": wl.next_section_id,
        })
    }).collect();
    let state = serde_json::json!({
        "watchlists": wls,
        "active_idx": watchlist.active_watchlist_idx,
    });
    let _ = std::fs::write(watchlists_path(), serde_json::to_string_pretty(&state).unwrap_or_default());
}

fn load_watchlists() -> (Vec<SavedWatchlist>, usize) {
    let path = watchlists_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return default_watchlists(),
    };
    let json: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return default_watchlists(),
    };
    let active_idx = json.get("active_idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let wl_arr = match json.get("watchlists").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return default_watchlists(),
    };
    let mut watchlists: Vec<SavedWatchlist> = Vec::new();
    for wl_val in wl_arr {
        let name = wl_val.get("name").and_then(|v| v.as_str()).unwrap_or("Untitled").to_string();
        let next_section_id = wl_val.get("next_section_id").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
        let mut sections: Vec<WatchlistSection> = Vec::new();
        if let Some(sec_arr) = wl_val.get("sections").and_then(|v| v.as_array()) {
            for sec_val in sec_arr {
                let id = sec_val.get("id").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let title = sec_val.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let color = sec_val.get("color").and_then(|v| v.as_str()).map(|s| s.to_string());
                let collapsed = sec_val.get("collapsed").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut items: Vec<WatchlistItem> = Vec::new();
                if let Some(item_arr) = sec_val.get("items").and_then(|v| v.as_array()) {
                    for item_val in item_arr {
                        let symbol = item_val.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !symbol.is_empty() {
                            let is_option = item_val.get("is_option").and_then(|v| v.as_bool()).unwrap_or(false);
                            let underlying = item_val.get("underlying").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let option_type = item_val.get("option_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let strike = item_val.get("strike").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let expiry = item_val.get("expiry").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let bid = item_val.get("bid").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let ask = item_val.get("ask").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            items.push(WatchlistItem { symbol, price: 0.0, prev_close: 0.0, loaded: false, is_option, underlying, option_type, strike, expiry, bid, ask });
                        }
                    }
                }
                sections.push(WatchlistSection { id, title, color, collapsed, items });
            }
        }
        watchlists.push(SavedWatchlist { name, sections, next_section_id });
    }
    if watchlists.is_empty() { return default_watchlists(); }
    let idx = active_idx.min(watchlists.len() - 1);
    (watchlists, idx)
}

fn default_watchlists() -> (Vec<SavedWatchlist>, usize) {
    let items: Vec<WatchlistItem> = DEFAULT_WATCHLIST.iter().map(|&s| WatchlistItem {
        symbol: s.into(), price: 0.0, prev_close: 0.0, loaded: false, is_option: false, underlying: String::new(), option_type: String::new(), strike: 0.0, expiry: String::new(), bid: 0.0, ask: 0.0,
    }).collect();
    let default_section = WatchlistSection {
        id: 1, title: String::new(), color: None, collapsed: false, items,
    };
    let wl = SavedWatchlist { name: "Default".into(), sections: vec![default_section], next_section_id: 2 };
    (vec![wl], 0)
}

/// Global sender for spawning new windows on the persistent render thread.
static SPAWN_TX: std::sync::OnceLock<Mutex<Option<mpsc::Sender<SpawnRequest>>>> = std::sync::OnceLock::new();

/// Called from Tauri command thread to open a new native chart window.
/// First call starts the render thread; subsequent calls send spawn requests.
pub fn open_window(rx: mpsc::Receiver<ChartCommand>, initial_cmd: ChartCommand, app_handle: Option<tauri::AppHandle>) {
    let spawn_tx_lock = SPAWN_TX.get_or_init(|| Mutex::new(None));
    let mut guard = spawn_tx_lock.lock().unwrap();

    // Try sending to existing render thread
    let req = SpawnRequest { rx, initial_cmd };
    let req = if let Some(tx) = guard.as_ref() {
        match tx.send(req) {
            Ok(()) => return, // success — render thread got it
            Err(mpsc::SendError(r)) => r, // thread died — get req back, restart below
        }
    } else { req };

    // First call or render thread died — start the render thread
    let (spawn_tx, spawn_rx) = mpsc::channel();
    let _ = spawn_tx.send(req);
    *guard = Some(spawn_tx);

    let handle = app_handle.clone();
    std::thread::spawn(move || {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        let el = EventLoop::builder().with_any_thread(true).build().unwrap();
        let mut app = App {
            app_handle: handle, iw: 1920, ih: 1080,
            windows: Vec::new(), spawn_rx,
        };
        let _ = el.run_app(&mut app);
        // All windows closed — clear the spawn sender so next call restarts
        if let Some(lock) = SPAWN_TX.get() {
            *lock.lock().unwrap() = None;
        }
    });
}

