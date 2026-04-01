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
    Theme { name: "Midnight",    bg: rgb(13,13,13),   bull: rgb(46,204,113),  bear: rgb(231,76,60),   dim: rgb(102,102,102), toolbar_bg: rgb(17,17,17),  toolbar_border: rgb(34,34,34),  accent: rgb(42,100,150) },
    Theme { name: "Nord",        bg: rgb(46,52,64),   bull: rgb(163,190,140), bear: rgb(191,97,106),  dim: rgb(129,161,193), toolbar_bg: rgb(46,52,64),  toolbar_border: rgb(59,66,82),  accent: rgb(136,192,208) },
    Theme { name: "Monokai",     bg: rgb(39,40,34),   bull: rgb(166,226,46),  bear: rgb(249,38,114),  dim: rgb(165,159,133), toolbar_bg: rgb(30,31,28),  toolbar_border: rgb(62,61,50),  accent: rgb(230,219,116) },
    Theme { name: "Solarized",   bg: rgb(0,43,54),    bull: rgb(133,153,0),   bear: rgb(220,50,47),   dim: rgb(131,148,150), toolbar_bg: rgb(0,43,54),   toolbar_border: rgb(7,54,66),   accent: rgb(42,161,152) },
    Theme { name: "Dracula",     bg: rgb(40,42,54),   bull: rgb(80,250,123),  bear: rgb(255,85,85),   dim: rgb(189,147,249), toolbar_bg: rgb(33,34,44),  toolbar_border: rgb(52,55,70),  accent: rgb(255,121,198) },
    Theme { name: "Gruvbox",     bg: rgb(40,40,40),   bull: rgb(184,187,38),  bear: rgb(251,73,52),   dim: rgb(213,196,161), toolbar_bg: rgb(29,32,33),  toolbar_border: rgb(60,56,54),  accent: rgb(254,128,25) },
    Theme { name: "Catppuccin",  bg: rgb(30,30,46),   bull: rgb(166,227,161), bear: rgb(243,139,168), dim: rgb(180,190,254), toolbar_bg: rgb(24,24,37),  toolbar_border: rgb(49,50,68),  accent: rgb(203,166,247) },
    Theme { name: "Tokyo Night", bg: rgb(26,27,38),   bull: rgb(158,206,106), bear: rgb(247,118,142), dim: rgb(122,162,247), toolbar_bg: rgb(22,22,30),  toolbar_border: rgb(36,40,59),  accent: rgb(125,207,255) },
];

const PRESET_COLORS: &[&str] = &["#4a9eff","#e74c3c","#2ecc71","#f39c12","#9b59b6","#1abc9c","#ffffff","#e67e22"];

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

fn hex_to_color(hex: &str, opacity: f32) -> egui::Color32 {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(128);
    egui::Color32::from_rgba_unmultiplied(r, g, b, (opacity * 255.0) as u8)
}

fn compute_sma(data: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; data.len()];
    if data.len() < period { return r; }
    let mut s: f32 = data[..period].iter().sum();
    r[period-1] = s / period as f32;
    for i in period..data.len() { s += data[i] - data[i-period]; r[i] = s / period as f32; }
    r
}
fn compute_ema(data: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; data.len()];
    if data.len() < period { return r; }
    let k = 2.0/(period as f32+1.0);
    let sma: f32 = data[..period].iter().sum::<f32>() / period as f32;
    r[period-1] = sma; let mut prev = sma;
    for i in period..data.len() { let v = data[i]*k + prev*(1.0-k); r[i] = v; prev = v; }
    r
}

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
            Self::WMA => {
                let mut r = vec![f32::NAN; closes.len()];
                if closes.len() < period { return r; }
                let denom = (period * (period + 1)) / 2;
                for i in (period - 1)..closes.len() {
                    let mut s = 0.0;
                    for j in 0..period { s += closes[i + 1 - period + j] * (j + 1) as f32; }
                    r[i] = s / denom as f32;
                }
                r
            }
            Self::DEMA => {
                let ema1 = compute_ema(closes, period);
                let ema2 = compute_ema(&ema1, period);
                ema1.iter().zip(&ema2).map(|(&a, &b)| if a.is_nan() || b.is_nan() { f32::NAN } else { 2.0 * a - b }).collect()
            }
            Self::TEMA => {
                let ema1 = compute_ema(closes, period);
                let ema2 = compute_ema(&ema1, period);
                let ema3 = compute_ema(&ema2, period);
                ema1.iter().zip(ema2.iter().zip(&ema3))
                    .map(|(&a, (&b, &c))| if a.is_nan() || b.is_nan() || c.is_nan() { f32::NAN } else { 3.0 * a - 3.0 * b + c })
                    .collect()
            }
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

fn compute_rsi(closes: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; closes.len()];
    if closes.len() <= period { return r; }
    let mut avg_gain = 0.0_f32;
    let mut avg_loss = 0.0_f32;
    for i in 1..=period {
        let d = closes[i] - closes[i-1];
        if d > 0.0 { avg_gain += d; } else { avg_loss += -d; }
    }
    avg_gain /= period as f32;
    avg_loss /= period as f32;
    let rs = if avg_loss == 0.0 { 100.0 } else { avg_gain / avg_loss };
    r[period] = 100.0 - 100.0 / (1.0 + rs);
    for i in (period+1)..closes.len() {
        let d = closes[i] - closes[i-1];
        let (gain, loss) = if d > 0.0 { (d, 0.0) } else { (0.0, -d) };
        avg_gain = (avg_gain * (period as f32 - 1.0) + gain) / period as f32;
        avg_loss = (avg_loss * (period as f32 - 1.0) + loss) / period as f32;
        let rs = if avg_loss == 0.0 { 100.0 } else { avg_gain / avg_loss };
        r[i] = 100.0 - 100.0 / (1.0 + rs);
    }
    r
}

fn compute_macd(closes: &[f32], fast: usize, slow: usize, signal: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let ema_fast = compute_ema(closes, fast);
    let ema_slow = compute_ema(closes, slow);
    let macd_line: Vec<f32> = ema_fast.iter().zip(&ema_slow).map(|(&f, &s)| {
        if f.is_nan() || s.is_nan() { f32::NAN } else { f - s }
    }).collect();
    let signal_line = compute_ema(&macd_line, signal);
    let histogram: Vec<f32> = macd_line.iter().zip(&signal_line).map(|(&m, &s)| {
        if m.is_nan() || s.is_nan() { f32::NAN } else { m - s }
    }).collect();
    (macd_line, signal_line, histogram)
}

fn compute_stochastic(highs: &[f32], lows: &[f32], closes: &[f32], k_period: usize, d_period: usize) -> (Vec<f32>, Vec<f32>) {
    let n = closes.len();
    let mut k = vec![f32::NAN; n];
    for i in (k_period-1)..n {
        let mut hi = f32::MIN;
        let mut lo = f32::MAX;
        for j in (i+1-k_period)..=i { hi = hi.max(highs[j]); lo = lo.min(lows[j]); }
        k[i] = if hi == lo { 50.0 } else { (closes[i] - lo) / (hi - lo) * 100.0 };
    }
    let d = compute_sma(&k, d_period);
    (k, d)
}

fn compute_vwap(closes: &[f32], volumes: &[f32], highs: &[f32], lows: &[f32]) -> Vec<f32> {
    let n = closes.len();
    let mut r = vec![f32::NAN; n];
    let mut cum_tp_vol = 0.0_f32;
    let mut cum_vol = 0.0_f32;
    for i in 0..n {
        let tp = (highs[i] + lows[i] + closes[i]) / 3.0;
        cum_tp_vol += tp * volumes[i];
        cum_vol += volumes[i];
        if cum_vol > 0.0 { r[i] = cum_tp_vol / cum_vol; }
    }
    r
}

/// Detect bullish/bearish divergences between price and indicator
fn detect_divergences(closes: &[f32], indicator: &[f32], lookback: usize) -> Vec<i8> {
    let n = closes.len();
    let mut div = vec![0i8; n];
    if n < lookback * 2 { return div; }
    // Find local swing lows/highs in both price and indicator
    for i in lookback..n.saturating_sub(lookback) {
        // Check for swing low in price
        let is_price_low = (1..=lookback).all(|j| closes[i] <= closes[i.saturating_sub(j)] && closes[i] <= closes[(i+j).min(n-1)]);
        if is_price_low && !indicator[i].is_nan() {
            // Look back for a previous swing low
            for k in (lookback..i).rev().take(lookback * 4) {
                let was_low = (1..=lookback.min(k)).all(|j| closes[k] <= closes[k.saturating_sub(j)]);
                if was_low && !indicator[k].is_nan() {
                    // Bullish: price makes lower low but indicator makes higher low
                    if closes[i] < closes[k] && indicator[i] > indicator[k] { div[i] = 1; }
                    // Bearish: price makes higher low but indicator makes lower low
                    if closes[i] > closes[k] && indicator[i] < indicator[k] { div[i] = -1; }
                    break;
                }
            }
        }
    }
    div
}

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

// ─── Positions & Alerts ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Position {
    symbol: String,
    qty: i32,         // positive=long, negative=short
    avg_price: f32,
    current_price: f32,
}

impl Position {
    fn pnl(&self) -> f32 { (self.current_price - self.avg_price) * self.qty as f32 }
    fn pnl_pct(&self) -> f32 {
        if self.avg_price == 0.0 { return 0.0; }
        ((self.current_price - self.avg_price) / self.avg_price) * 100.0
    }
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

// ─── Chart state ──────────────────────────────────────────────────────────────

struct Chart {
    symbol: String, timeframe: String,
    bars: Vec<Bar>, timestamps: Vec<i64>, drawings: Vec<Drawing>,
    indicators: Vec<Indicator>,
    indicator_bar_count: usize, // bar count when indicators were last computed
    next_indicator_id: u32,
    editing_indicator: Option<u32>, // id of indicator being edited
    vs: f32, vc: u32, price_lock: Option<(f32,f32)>,
    auto_scroll: bool, last_input: std::time::Instant,
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
    dragging_order: Option<u32>, // order id being dragged
    editing_order: Option<u32>,
    edit_order_qty: String,
    edit_order_price: String,
    armed: bool, // skip confirmation, fire orders immediately
    pending_confirms: Vec<(u32, std::time::Instant)>, // order ids awaiting user confirm from panel
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
            bars: vec![], timestamps: vec![], drawings: vec![], indicator_bar_count: 0,
            next_indicator_id: 5, editing_indicator: None,
            indicators: vec![
                Indicator::new(1, IndicatorType::SMA, 20, "#00bef0"),
                Indicator::new(2, IndicatorType::SMA, 50, "#f0961a"),
                Indicator::new(3, IndicatorType::EMA, 12, "#f0d732"),
                Indicator::new(4, IndicatorType::EMA, 26, "#b266e6"),
            ],
            vs: 0.0, vc: 200, price_lock: None, auto_scroll: true,
            last_input: std::time::Instant::now(), tick_counter: 0,
            last_candle_time: std::time::Instant::now(), sim_price: 0.0,
            sim_seed: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42),
            theme_idx: 5, // Gruvbox
            draw_tool: String::new(), pending_pt: None,
            selected_id: None, selected_ids: vec![], dragging_drawing: None,
            drag_start_price: 0.0, drag_start_bar: 0.0,
            groups: vec![DrawingGroup { id: "default".into(), name: "Temp".into(), color: None }],
            hidden_groups: vec![], hide_all_drawings: false, hide_all_indicators: false, show_volume: true, show_oscillators: true,
            signal_drawings: vec![], hide_signal_drawings: false, last_signal_fetch: std::time::Instant::now(),
            draw_color: "#4a9eff".into(), group_manager_open: false, new_group_name: String::new(),
            zoom_selecting: false, zoom_start: egui::Pos2::ZERO,
            picker_open: false, picker_query: String::new(), picker_results: vec![],
            picker_last_query: String::new(), picker_searching: false, picker_rx: None, picker_pos: egui::Pos2::ZERO,
            recent_symbols: vec![("AAPL".into(), "Apple".into()), ("SPY".into(), "S&P 500 ETF".into()), ("TSLA".into(), "Tesla".into()), ("NVDA".into(), "Nvidia".into()), ("MSFT".into(), "Microsoft".into())],
            orders: vec![], next_order_id: 1, order_qty: 100, order_market: true, order_limit_price: String::new(),
            dragging_order: None, editing_order: None, edit_order_qty: String::new(), edit_order_price: String::new(),
            armed: false, pending_confirms: vec![],
            measuring: false, measure_start: None, measure_active: false,
            pending_symbol_change: None, pending_timeframe_change: None,
            cached_ohlc: String::new(), cached_ohlc_bar_count: 0,
            indicator_pts_buf: Vec::with_capacity(512), fmt_buf: String::with_capacity(256) }
    }
    fn process(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, timestamps, symbol, timeframe, .. } => {
                self.symbol = symbol; self.timeframe = timeframe;
                self.bars = bars; self.timestamps = timestamps;
                self.vs = (self.bars.len() as f32 - self.vc as f32 + 8.0).max(0.0);
                self.sim_price = 0.0;
                self.last_candle_time = std::time::Instant::now();
                self.indicator_bar_count = 0; // force recompute
                // Load persisted drawings for this symbol
                self.drawings.clear();
                let db_drawings = crate::drawing_db::load_symbol(&self.symbol);
                for dd in &db_drawings {
                    if let Some(d) = db_to_drawing(dd) { self.drawings.push(d); }
                }
                // Load groups from DB
                let db_groups = crate::drawing_db::load_groups();
                self.groups.clear();
                for (id, name, color) in db_groups {
                    self.groups.push(super::DrawingGroup { id, name, color });
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

    let draw_line = |rgba: &mut Vec<u8>, x0: f32, y0: f32, x1: f32, y1: f32, w: f32| {
        let len = ((x1-x0)*(x1-x0) + (y1-y0)*(y1-y0)).sqrt();
        let steps = (len * 2.0) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let px = x0 + (x1-x0) * t;
            let py = y0 + (y1-y0) * t;
            for dy in -(w as i32)..=(w as i32) {
                for dx in -(w as i32)..=(w as i32) {
                    let ix = (px + dx as f32) as i32;
                    let iy = (py + dy as f32) as i32;
                    if ix >= 0 && ix < s as i32 && iy >= 0 && iy < s as i32 {
                        let idx = ((iy as u32 * s + ix as u32) * 4) as usize;
                        rgba[idx..idx+4].copy_from_slice(&color);
                    }
                }
            }
        }
    };

    // Triangle sides
    draw_line(&mut rgba, top.0, top.1, bl.0, bl.1, 1.0);
    draw_line(&mut rgba, bl.0, bl.1, br.0, br.1, 1.0);
    draw_line(&mut rgba, br.0, br.1, top.0, top.1, 1.0);
    // Horizontal bar
    let bar_y = cx + 2.0;
    draw_line(&mut rgba, cx - 7.0, bar_y, cx + 7.0, bar_y, 1.0);

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

    let set_px = |buf: &mut Vec<u8>, x: i32, y: i32| {
        if x >= 0 && x < s && y >= 0 && y < s {
            let idx = ((y * s + x) * 4) as usize;
            buf[idx..idx+4].copy_from_slice(&color_bgra);
        }
    };

    let draw_line = |buf: &mut Vec<u8>, x0: f32, y0: f32, x1: f32, y1: f32| {
        let len = ((x1-x0)*(x1-x0) + (y1-y0)*(y1-y0)).sqrt();
        let steps = (len * 3.0) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps.max(1) as f32;
            let px = (x0 + (x1-x0)*t) as i32;
            let py = (y0 + (y1-y0)*t) as i32;
            for dy in -1..=1 { for dx in -1..=1 { set_px(buf, px+dx, py+dy); } }
        }
    };

    let m = 3.0_f32;
    let cx = s as f32 / 2.0;
    // Triangle
    draw_line(&mut bgra, cx, m, m, s as f32 - m);
    draw_line(&mut bgra, m, s as f32 - m, s as f32 - m, s as f32 - m);
    draw_line(&mut bgra, s as f32 - m, s as f32 - m, cx, m);
    // Horizontal bar
    draw_line(&mut bgra, cx - 7.0, cx + 2.0, cx + 7.0, cx + 2.0);

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
        DrawingKind::TrendLine { price0, bar0, price1, bar1 } => ("trendline".into(), vec![(*bar0 as f64, *price0 as f64), (*bar1 as f64, *price1 as f64)]),
        DrawingKind::HZone { price0, price1 } => ("hzone".into(), vec![(0.0, *price0 as f64), (0.0, *price1 as f64)]),
        DrawingKind::BarMarker { bar, price, up } => ("barmarker".into(), vec![(*bar as f64, *price as f64), (if *up { 1.0 } else { 0.0 }, 0.0)]),
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
            DrawingKind::TrendLine { bar0: p0.0 as f32, price0: p0.1 as f32, bar1: p1.0 as f32, price1: p1.1 as f32 }
        }
        "hzone" => DrawingKind::HZone { price0: d.points.get(0)?.1 as f32, price1: d.points.get(1)?.1 as f32 },
        "barmarker" => DrawingKind::BarMarker { bar: d.points.get(0)?.0 as f32, price: d.points.get(0)?.1 as f32, up: d.points.get(1).map(|p| p.0 > 0.5).unwrap_or(true) },
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

    // Route incoming commands to the matching pane (by symbol), or active pane as fallback
    span_begin("cmd_routing");
    while let Ok(cmd) = rx.try_recv() {
        let sym = match &cmd {
            ChartCommand::LoadBars { symbol, .. } | ChartCommand::UpdateLastBar { symbol, .. } | ChartCommand::AppendBar { symbol, .. } => Some(symbol.clone()),
            _ => None,
        };
        if let Some(s) = sym {
            if let Some(p) = panes.iter_mut().find(|p| p.symbol == s) { p.process(cmd); }
            else if let Some(p) = panes.get_mut(*active_pane) { p.process(cmd); }
        } else if let Some(p) = panes.get_mut(*active_pane) { p.process(cmd); }
    }
    if *active_pane >= panes.len() { *active_pane = 0; }

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
        let bg = if active { egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 51) } else { t.toolbar_bg };
        let fg = if active { t.accent } else { t.dim };
        let border = if active { egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 136) } else { t.toolbar_border };
        let btn = egui::Button::new(egui::RichText::new(label).monospace().size(11.0).color(fg))
            .fill(bg).stroke(egui::Stroke::new(1.0, border)).corner_radius(3.0)
            .min_size(egui::vec2(0.0, 22.0));
        ui.add(btn)
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
            if tb_btn(ui, "filters", watchlist.trendline_filter_open, t).clicked() {
                watchlist.trendline_filter_open = !watchlist.trendline_filter_open;
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // Indicator dropdown (add new indicator from toolbar)
            let ind_resp = tb_btn(ui, "+ ind", false, t);
            if ind_resp.clicked() {
                // Show indicator type menu below the button
                ui.memory_mut(|m| m.toggle_popup(egui::Id::new("ind_add_popup")));
            }
            egui::popup_below_widget(ui, egui::Id::new("ind_add_popup"), &ind_resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                ui.set_min_width(160.0);
                ui.label(egui::RichText::new("OVERLAYS").monospace().size(9.0).strong().color(t.accent));
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
                ui.label(egui::RichText::new("OSCILLATORS").monospace().size(9.0).strong().color(t.accent));
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

            // Keyboard shortcuts help
            if tb_btn(ui, "?", watchlist.shortcuts_open, t).clicked() {
                watchlist.shortcuts_open = !watchlist.shortcuts_open;
            }

            // New window button
            if tb_btn(ui, "+ Window", false, t).clicked() {
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

            // Watchlist toggle
            if tb_btn(ui, Icon::LIST, watchlist.open, t).clicked() { watchlist.open = !watchlist.open; }
            // Order entry toggle
            if tb_btn(ui, "orders", watchlist.order_entry_open, t).clicked() { watchlist.order_entry_open = !watchlist.order_entry_open; }
            // Orders book panel toggle
            if tb_btn(ui, "book", watchlist.orders_panel_open, t).clicked() { watchlist.orders_panel_open = !watchlist.orders_panel_open; }
            // Connection status
            {
                let dot_color = rgb(46, 204, 113);
                let conn_resp = ui.add(egui::Button::new(egui::RichText::new("CONN").monospace().size(9.0).color(t.dim))
                    .fill(if *conn_panel_open { egui::Color32::from_rgba_unmultiplied(t.accent.r(),t.accent.g(),t.accent.b(),30) } else { t.toolbar_bg })
                    .stroke(egui::Stroke::new(1.0, t.toolbar_border)).corner_radius(2.0));
                ui.painter().circle_filled(egui::pos2(conn_resp.rect.left() - 6.0, conn_resp.rect.center().y), 3.0, dot_color);
                if conn_resp.clicked() { *conn_panel_open = !*conn_panel_open; }
            }

            // LIVE indicator
            if !panes[ap].auto_scroll {
                if tb_btn(ui, "LIVE", false, t).clicked() {
                    panes[ap].auto_scroll=true; panes[ap].price_lock=None;
                    let bar_count = panes[ap].bars.len();
                    panes[ap].vs=(bar_count as f32-panes[ap].vc as f32+8.0).max(0.0);
                }
            } else {
                ui.label(egui::RichText::new("LIVE").monospace().size(11.0).color(t.accent));
            }

            }); // end scrollable middle

            // ── Fixed right: window controls only ──
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

                // Separator between window controls and scrollable content
                ui.add(egui::Separator::default().spacing(4.0));
            });
        });
    });

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
        egui::Window::new("shortcuts_help")
            .fixed_pos(egui::pos2(ctx.screen_rect().center().x - 140.0, 50.0))
            .fixed_size(egui::vec2(280.0, 0.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style()).fill(egui::Color32::from_rgb(28, 28, 34)).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("KEYBOARD SHORTCUTS").monospace().size(10.0).strong().color(t.accent));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim)).frame(false)).clicked() {
                            watchlist.shortcuts_open = false;
                        }
                    });
                });
                ui.add_space(8.0);
                let row = |ui: &mut egui::Ui, key: &str, desc: &str| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(key).monospace().size(10.0).strong().color(egui::Color32::from_rgb(200,200,210)));
                        ui.label(egui::RichText::new(desc).monospace().size(9.0).color(t.dim));
                    });
                };
                ui.label(egui::RichText::new("NAVIGATION").monospace().size(9.0).color(t.accent));
                row(ui, "Scroll", "Zoom in/out");
                row(ui, "Drag", "Pan chart");
                row(ui, "Drag Y-axis", "Vertical zoom");
                row(ui, "Drag X-axis", "Horizontal zoom");
                row(ui, "Dbl-click Y", "Reset Y zoom");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("DRAWING").monospace().size(9.0).color(t.accent));
                row(ui, "Middle-click", "Cycle drawing tools");
                row(ui, "Escape", "Cancel / deselect");
                row(ui, "Delete", "Delete selected drawing");
                row(ui, "Shift+Drag", "Measure tool");
                row(ui, "Dbl-click line", "Edit indicator/order");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("ORDERS").monospace().size(9.0).color(t.accent));
                row(ui, "Right-click", "Place order at price");
                row(ui, "Drag order", "Adjust order price");
                row(ui, "Dbl-click order", "Edit order details");
            });
    }

    // ── Trendline filter dropdown ────────────────────────────────────────────
    if watchlist.trendline_filter_open {
        egui::Window::new("trendline_filter")
            .fixed_pos(egui::pos2(300.0, 40.0))
            .fixed_size(egui::vec2(200.0, 0.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style()).fill(egui::Color32::from_rgb(28, 28, 34)).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DRAWING FILTERS").monospace().size(10.0).strong().color(t.accent));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim)).frame(false)).clicked() {
                            watchlist.trendline_filter_open = false;
                        }
                    });
                });
                ui.add_space(4.0);

                let chart = &mut panes[ap];
                // Per-type visibility toggles
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
                        ui.label(egui::RichText::new(format!("{} ({})", label, count)).monospace().size(10.0).color(egui::Color32::from_rgb(200,200,210)));
                    });
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // Signal drawings toggle
                let sig_count = chart.signal_drawings.len();
                let sig_icon = if chart.hide_signal_drawings { Icon::EYE_SLASH } else { Icon::EYE };
                if ui.button(format!("{} Signals ({})", sig_icon, sig_count)).clicked() {
                    chart.hide_signal_drawings = !chart.hide_signal_drawings;
                }

                // All drawings toggle
                let draw_icon = if chart.hide_all_drawings { Icon::EYE_SLASH } else { Icon::EYE };
                if ui.button(format!("{} All Drawings", draw_icon)).clicked() {
                    chart.hide_all_drawings = !chart.hide_all_drawings;
                }

                // Groups
                ui.add_space(4.0);
                ui.label(egui::RichText::new("GROUPS").monospace().size(9.0).color(t.dim));
                for g in chart.groups.clone() {
                    let hidden = chart.hidden_groups.contains(&g.id);
                    let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                    let icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                    if ui.button(format!("{} {} ({})", icon, g.name, count)).clicked() {
                        if hidden { chart.hidden_groups.retain(|x| x != &g.id); }
                        else { chart.hidden_groups.push(g.id.clone()); }
                    }
                }
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

                // Fire background Yahoo Finance search
                chart.picker_searching = true;
                let (tx, rx) = mpsc::channel();
                chart.picker_rx = Some(rx);
                let query = q.clone();
                std::thread::spawn(move || {
                    let mut results: Vec<(String, String, String)> = Vec::new();
                    // Yahoo Finance search API
                    let url = format!(
                        "https://query2.finance.yahoo.com/v1/finance/search?q={}&quotesCount=15&newsCount=0",
                        query
                    );
                    if let Ok(resp) = reqwest::blocking::Client::builder()
                        .user_agent("Mozilla/5.0").build().unwrap_or_else(|_| reqwest::blocking::Client::new())
                        .get(&url).timeout(std::time::Duration::from_secs(3)).send()
                    {
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
                    // If Yahoo returned nothing, use static
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
        let (cur_color, cur_ls, cur_th, cur_op) = chart.drawings.iter().find(|d| ids.contains(&d.id))
            .map(|d| (d.color.clone(), d.line_style, d.thickness, d.opacity))
            .unwrap_or(("#4a9eff".into(), LineStyle::Solid, 1.5, 1.0));

        egui::Window::new("style_bar")
            .fixed_pos(egui::pos2(screen.center().x - 230.0, 32.0))
            .fixed_size(egui::vec2(460.0, 24.0))
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

                    // Line style dropdown with visual previews
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

        egui::Window::new(format!("ind_editor_{}", edit_id))
            .fixed_pos(egui::pos2(200.0, 80.0))
            .fixed_size(egui::vec2(280.0, 0.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style()).fill(egui::Color32::from_rgb(30, 30, 36)).inner_margin(10.0))
            .show(ctx, |ui| {
                if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.id == edit_id) {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("EDIT INDICATOR").strong().color(t.bull));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").clicked() { close_editor = true; }
                        });
                    });
                    ui.add_space(4.0);

                    // Type selector
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Type").small().color(t.dim));
                        for &kind in IndicatorType::all() {
                            let selected = ind.kind == kind;
                            let text = egui::RichText::new(kind.label()).small()
                                .color(if selected { t.bull } else { egui::Color32::from_rgb(180,180,190) });
                            if ui.add(egui::Button::new(text).frame(false).min_size(egui::vec2(32.0, 18.0))).clicked() && !selected {
                                ind.kind = kind;
                                needs_recompute = true;
                            }
                        }
                    });

                    // Period
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Period").small().color(t.dim));
                        let mut period = ind.period as i32;
                        if ui.add(egui::DragValue::new(&mut period).range(1..=500).speed(0.5)).changed() {
                            ind.period = (period as usize).max(1);
                            needs_recompute = true;
                        }
                        // Quick presets
                        for &p in &[9, 12, 20, 26, 50, 100, 200] {
                            let sel = ind.period == p;
                            let txt = egui::RichText::new(format!("{}", p)).small()
                                .color(if sel { t.bull } else { t.dim });
                            if ui.add(egui::Button::new(txt).frame(false).min_size(egui::vec2(22.0, 16.0))).clicked() && !sel {
                                ind.period = p;
                                needs_recompute = true;
                            }
                        }
                    });

                    // Source interval
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Interval").small().color(t.dim));
                        for &tf in INDICATOR_TIMEFRAMES {
                            let label = if tf.is_empty() { "Chart" } else { tf };
                            let sel = ind.source_tf == tf;
                            let txt = egui::RichText::new(label).small()
                                .color(if sel { t.bull } else { t.dim });
                            if ui.add(egui::Button::new(txt).frame(false).min_size(egui::vec2(28.0, 16.0))).clicked() && !sel {
                                ind.source_tf = tf.to_string();
                                ind.source_loaded = tf.is_empty(); // chart TF is always loaded
                                ind.source_bars.clear();
                                ind.source_timestamps.clear();
                                needs_recompute = true;
                                // Fetch cross-timeframe data if needed
                                if !tf.is_empty() {
                                    let sym = pane_symbol.clone();
                                    let ind_id = ind.id;
                                    let ind_tf = tf.to_string();
                                    // Defer fetch to after the borrow ends
                                    needs_source_fetch = Some((sym, ind_tf, ind_id));
                                }
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Color
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Color").small().color(t.dim));
                        for &c in INDICATOR_COLORS {
                            let color = hex_to_color(c, 1.0);
                            let is_cur = ind.color == c;
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                            ui.painter().circle_filled(r.center(), if is_cur { 6.0 } else { 5.0 }, color);
                            if is_cur { ui.painter().circle_stroke(r.center(), 7.0, egui::Stroke::new(1.5, egui::Color32::WHITE)); }
                            if resp.clicked() { ind.color = c.to_string(); }
                        }
                    });

                    // Thickness
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Width").small().color(t.dim));
                        for &th in &[0.5, 1.0, 1.5, 2.0, 3.0] {
                            let sel = (ind.thickness - th).abs() < 0.1;
                            let txt = egui::RichText::new(format!("{:.1}", th)).small()
                                .color(if sel { t.bull } else { t.dim });
                            if ui.add(egui::Button::new(txt).frame(false)).clicked() { ind.thickness = th; }
                        }
                    });

                    // Line style
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Style").small().color(t.dim));
                        for (ls, label) in [(LineStyle::Solid, "Solid"), (LineStyle::Dashed, "Dash"), (LineStyle::Dotted, "Dot")] {
                            let sel = ind.line_style == ls;
                            let txt = egui::RichText::new(label).small()
                                .color(if sel { t.bull } else { t.dim });
                            if ui.add(egui::Button::new(txt).frame(false)).clicked() { ind.line_style = ls; }
                        }
                    });

                    // Visibility toggle
                    ui.horizontal(|ui| {
                        let vis_icon = if ind.visible { Icon::EYE } else { Icon::EYE_SLASH };
                        if ui.button(format!("{} {}", vis_icon, if ind.visible { "Visible" } else { "Hidden" })).clicked() {
                            ind.visible = !ind.visible;
                        }
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);

                    // Delete
                    if ui.button(egui::RichText::new(format!("{} Delete", Icon::TRASH)).color(egui::Color32::from_rgb(224, 85, 96))).clicked() {
                        delete_id = Some(edit_id);
                        close_editor = true;
                    }
                } else {
                    close_editor = true; // indicator was deleted
                }
            });

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
        egui::Window::new("group_manager")
            .fixed_pos(egui::pos2(200.0, 100.0))
            .fixed_size(egui::vec2(240.0, 0.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style()).fill(egui::Color32::from_rgb(30, 30, 36)).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("NEW GROUP").monospace().size(10.0).strong().color(t.accent));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim)).frame(false)).clicked() {
                            close_gm = true;
                        }
                    });
                });
                ui.add_space(6.0);
                let resp = ui.add(egui::TextEdit::singleline(&mut panes[ap].new_group_name)
                    .hint_text("Group name...").desired_width(220.0).font(egui::FontId::monospace(11.0)));
                resp.request_focus();
                ui.add_space(6.0);
                let can_create = !panes[ap].new_group_name.trim().is_empty();
                if ui.add_enabled(can_create, egui::Button::new(
                    egui::RichText::new(format!("{} Create", Icon::PLUS)).monospace().size(11.0)
                ).min_size(egui::vec2(220.0, 24.0))).clicked() || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can_create) {
                    let name = panes[ap].new_group_name.trim().to_string();
                    let id = new_uuid();
                    crate::drawing_db::save_group(&id, &name, None);
                    panes[ap].groups.push(super::DrawingGroup { id, name, color: None });
                    panes[ap].new_group_name.clear();
                    close_gm = true;
                }
            });
        if close_gm { panes[ap].group_manager_open = false; }
    }

    // ── Connection panel popup ──────────────────────────────────────────────
    if *conn_panel_open {
        egui::Window::new("conn_panel")
            .fixed_pos(egui::pos2(ctx.screen_rect().right() - 250.0, 40.0))
            .fixed_size(egui::vec2(230.0, 0.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style()).fill(egui::Color32::from_rgb(28, 28, 34)).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("CONNECTIONS").monospace().size(10.0).strong().color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim)).frame(false)).clicked() {
                            *conn_panel_open = false;
                        }
                    });
                });
                ui.add_space(6.0);

                let svc_row = |ui: &mut egui::Ui, name: &str, status: &str, ok: bool, detail: &str| {
                    ui.horizontal(|ui| {
                        let color = if ok { rgb(46,204,113) } else { rgb(231,76,60) };
                        ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 8.0), 3.0, color);
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(name).monospace().size(10.0).strong().color(egui::Color32::from_rgb(200,200,210)));
                        ui.label(egui::RichText::new(status).monospace().size(9.0).color(if ok { t.bull } else { t.bear }));
                    });
                    ui.label(egui::RichText::new(format!("  {}", detail)).monospace().size(8.0).color(t.dim));
                    ui.add_space(4.0);
                };

                let redis_ok = crate::bar_cache::get("__ping_test", "").is_none(); // tests connection
                svc_row(ui, "Redis Cache", if redis_ok { "connected" } else { "offline" }, redis_ok, "192.168.1.89:6379");
                svc_row(ui, "GPU Engine", "DX12 active", true, "wgpu + egui");
                svc_row(ui, "Data Feed", "Yahoo Finance", true, "query1.finance.yahoo.com");
                svc_row(ui, "OCOCO", "cache", true, "192.168.1.60:30300");

                ui.add_space(4.0);
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left(), ui.cursor().min.y), egui::pos2(ui.min_rect().right(), ui.cursor().min.y)],
                    egui::Stroke::new(0.5, t.toolbar_border));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Endpoints: redis:6379 · ococo:30300 · yahoo").monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
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
                    ui.label(egui::RichText::new(format!("\u{2713} {}", msg)).monospace().size(10.0)
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
                // Header with tabs
                ui.horizontal(|ui| {
                    for (tab, label) in [(WatchlistTab::Stocks, "STOCKS"), (WatchlistTab::Chain, "CHAIN"), (WatchlistTab::Saved, "SAVED")] {
                        let active = watchlist.tab == tab;
                        let color = if active { t.accent } else { t.dim };
                        if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(9.0).strong().color(color)).frame(false)).clicked() {
                            watchlist.tab = tab;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim)).frame(false)).clicked() {
                            watchlist.open = false;
                        }
                    });
                });
                ui.add_space(2.0);
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left(), ui.cursor().min.y), egui::pos2(ui.min_rect().right(), ui.cursor().min.y)],
                    egui::Stroke::new(1.0, t.toolbar_border),
                );
                ui.add_space(4.0);

                match watchlist.tab {
                    // ── STOCKS TAB ──────────────────────────────────────────
                    WatchlistTab::Stocks => {
                        // Search input
                        let search_resp = ui.add(
                            egui::TextEdit::singleline(&mut watchlist.search_query)
                                .hint_text("Add symbol...").desired_width(ui.available_width()).font(egui::FontId::monospace(11.0))
                        );
                        if search_resp.changed() && !watchlist.search_query.is_empty() {
                            watchlist.search_results = ui_kit::symbols::search_symbols(&watchlist.search_query, 5)
                                .iter().map(|s| (s.symbol.to_string(), s.name.to_string())).collect();
                        }
                        if !watchlist.search_query.is_empty() && !watchlist.search_results.is_empty() {
                            egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(30, 30, 36)).show(ui, |ui| {
                                for (sym, name) in watchlist.search_results.clone() {
                                    if ui.add(egui::Button::new(egui::RichText::new(format!("{} {}", sym, name)).monospace().size(10.0).color(t.dim))
                                        .frame(false).min_size(egui::vec2(ui.available_width(), 18.0))).clicked() {
                                        watchlist.add_symbol(&sym);
                                        watchlist.search_query.clear(); watchlist.search_results.clear();
                                        fetch_watchlist_prices(vec![sym]);
                                    }
                                }
                            });
                        }
                        if search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !watchlist.search_query.is_empty() {
                            let sym = watchlist.search_query.trim().to_uppercase();
                            watchlist.add_symbol(&sym); fetch_watchlist_prices(vec![sym.clone()]);
                            watchlist.search_query.clear(); watchlist.search_results.clear();
                        }
                        ui.add_space(4.0);

                        // Symbol list
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut remove_sym: Option<String> = None;
                            let mut click_sym: Option<String> = None;
                            let full_w = ui.available_width();
                            for item in &watchlist.items {
                                let change_pct = if item.prev_close > 0.0 { ((item.price - item.prev_close) / item.prev_close) * 100.0 } else { 0.0 };
                                let color = if change_pct >= 0.0 { t.bull } else { t.bear };
                                let price_str = if item.price > 0.0 { format!("{:.2}", item.price) } else { "---".into() };
                                let change_str = if item.loaded { format!("{:+.2}%", change_pct) } else { "".into() };
                                let resp = ui.horizontal(|ui| {
                                    ui.set_min_width(full_w);
                                    let sym_resp = ui.add(egui::Button::new(egui::RichText::new(&item.symbol).monospace().size(11.0).strong().color(egui::Color32::from_rgb(220,220,230))).frame(false));
                                    if sym_resp.clicked() { click_sym = Some(item.symbol.clone()); }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.5))).frame(false)).clicked() { remove_sym = Some(item.symbol.clone()); }
                                        ui.label(egui::RichText::new(&change_str).monospace().size(10.0).color(color));
                                        ui.label(egui::RichText::new(&price_str).monospace().size(11.0).color(color));
                                    });
                                });
                                if resp.response.hovered() { ui.painter().rect_filled(resp.response.rect, 0.0, t.toolbar_border.gamma_multiply(0.3)); }
                            }
                            if let Some(sym) = click_sym { panes[ap].pending_symbol_change = Some(sym); }
                            if let Some(sym) = remove_sym { watchlist.remove_symbol(&sym); }
                        });
                    }

                    // ── CHAIN TAB ───────────────────────────────────────────
                    WatchlistTab::Chain => {
                        // ── Symbol input ──
                        let sym_resp = ui.add(egui::TextEdit::singleline(&mut watchlist.chain_sym_input)
                            .hint_text(&watchlist.chain_symbol)
                            .desired_width(ui.available_width())
                            .font(egui::FontId::monospace(13.0)));
                        // Search suggestions
                        if sym_resp.changed() && !watchlist.chain_sym_input.is_empty() {
                            watchlist.search_results = ui_kit::symbols::search_symbols(&watchlist.chain_sym_input, 5)
                                .iter().map(|s| (s.symbol.to_string(), s.name.to_string())).collect();
                        }
                        if !watchlist.chain_sym_input.is_empty() && !watchlist.search_results.is_empty() && sym_resp.has_focus() {
                            egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(30, 30, 36)).show(ui, |ui| {
                                for (sym, name) in watchlist.search_results.clone() {
                                    if ui.add(egui::Button::new(egui::RichText::new(format!("{} {}", sym, name)).monospace().size(10.0).color(t.dim))
                                        .frame(false).min_size(egui::vec2(ui.available_width(), 18.0))).clicked() {
                                        watchlist.chain_symbol = sym;
                                        watchlist.chain_sym_input.clear();
                                        watchlist.search_results.clear();
                                        watchlist.chain_0dte = (vec![], vec![]); // force rebuild
                                    }
                                }
                            });
                        }
                        if sym_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !watchlist.chain_sym_input.is_empty() {
                            watchlist.chain_symbol = watchlist.chain_sym_input.trim().to_uppercase();
                            watchlist.chain_sym_input.clear();
                            watchlist.search_results.clear();
                            watchlist.chain_0dte = (vec![], vec![]); // force rebuild
                        }
                        // Rebuild chain from current price
                        let chain_price = watchlist.items.iter().find(|i| i.symbol == watchlist.chain_symbol).map(|i| i.price)
                            .or_else(|| panes.iter().find(|p| p.symbol == watchlist.chain_symbol).and_then(|p| p.bars.last().map(|b| b.close)))
                            .unwrap_or(100.0);
                        if chain_price > 0.0 && watchlist.chain_0dte.0.is_empty() {
                            let ns = watchlist.chain_num_strikes;
                            watchlist.chain_0dte = build_chain(chain_price, ns, 0);
                            watchlist.chain_far = build_chain(chain_price, ns, watchlist.chain_far_dte);
                        }

                        // ── Controls: strikes ± | DTE selector | sel toggle ──
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("strikes").monospace().size(9.0).color(t.dim));
                            if ui.add(egui::Button::new(egui::RichText::new("-").monospace().size(10.0)).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                watchlist.chain_num_strikes = watchlist.chain_num_strikes.saturating_sub(1).max(1);
                                watchlist.chain_0dte = build_chain(chain_price, watchlist.chain_num_strikes, 0);
                                watchlist.chain_far = build_chain(chain_price, watchlist.chain_num_strikes, watchlist.chain_far_dte);
                            }
                            ui.label(egui::RichText::new(format!("{}", watchlist.chain_num_strikes)).monospace().size(10.0).color(egui::Color32::from_rgb(200,200,210)));
                            if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(10.0)).min_size(egui::vec2(16.0, 16.0))).clicked() {
                                watchlist.chain_num_strikes += 1;
                                watchlist.chain_0dte = build_chain(chain_price, watchlist.chain_num_strikes, 0);
                                watchlist.chain_far = build_chain(chain_price, watchlist.chain_num_strikes, watchlist.chain_far_dte);
                            }

                            // DTE dropdown
                            let dte_labels = [(1,"1DTE"),(2,"2DTE"),(3,"3DTE"),(5,"5DTE"),(7,"7DTE"),(10,"10DTE")];
                            let cur_label = dte_labels.iter().find(|(d,_)| *d == watchlist.chain_far_dte).map(|(_,l)| *l).unwrap_or("1DTE");
                            egui::ComboBox::from_id_salt("far_dte").selected_text(egui::RichText::new(cur_label).monospace().size(9.0).color(t.dim)).width(60.0)
                                .show_ui(ui, |ui| {
                                    for (d, label) in &dte_labels {
                                        if ui.selectable_value(&mut watchlist.chain_far_dte, *d, *label).changed() {
                                            watchlist.chain_far = build_chain(chain_price, watchlist.chain_num_strikes, *d);
                                        }
                                    }
                                });

                            // Select mode toggle
                            let sel_label = if watchlist.chain_select_mode { "\u{2713} sel" } else { "sel" };
                            let sel_active = watchlist.chain_select_mode;
                            if ui.add(egui::Button::new(egui::RichText::new(sel_label).monospace().size(9.0)
                                .color(if sel_active { t.accent } else { t.dim }))
                                .fill(if sel_active { egui::Color32::from_rgba_unmultiplied(t.accent.r(),t.accent.g(),t.accent.b(),51) } else { t.toolbar_bg })
                                .stroke(egui::Stroke::new(1.0, if sel_active { t.accent } else { t.toolbar_border }))
                                .corner_radius(2.0)).clicked() {
                                watchlist.chain_select_mode = !watchlist.chain_select_mode;
                            }
                        });

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
                            ui.allocate_ui(egui::vec2(col_stk, 14.0), |ui| { ui.label(egui::RichText::new("STK").monospace().size(9.0).color(hdr_color)); });
                            ui.allocate_ui(egui::vec2(col_bid, 14.0), |ui| { ui.label(egui::RichText::new("BID").monospace().size(9.0).color(hdr_color)); });
                            ui.allocate_ui(egui::vec2(col_ask, 14.0), |ui| { ui.label(egui::RichText::new("ASK").monospace().size(9.0).color(hdr_color)); });
                            ui.allocate_ui(egui::vec2(col_oi, 14.0), |ui| { ui.label(egui::RichText::new("OI").monospace().size(9.0).color(hdr_color)); });
                        });

                        // ── Helper to render one option row ──
                        let render_row = |ui: &mut egui::Ui, row: &OptionRow, is_call: bool, exp_label: &str, sym: &str, saved: &mut Vec<SavedOption>, select_mode: bool, w: f32| {
                            let is_saved = saved.iter().any(|s| s.contract == row.contract);
                            let color = if is_call { t.bull } else { t.bear };
                            let hovered = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| {
                                let cursor_y = p.y;
                                let row_top = ui.cursor().min.y;
                                cursor_y >= row_top && cursor_y < row_top + 20.0 && p.x >= ui.min_rect().left() && p.x <= ui.min_rect().right()
                            });
                            let bg = if is_saved { egui::Color32::from_rgba_unmultiplied(t.accent.r(),t.accent.g(),t.accent.b(),40) }
                                else if hovered { t.toolbar_border.gamma_multiply(0.4) }
                                else if row.itm { color.gamma_multiply(0.06) }
                                else { egui::Color32::TRANSPARENT };

                            let resp = ui.horizontal(|ui| {
                                ui.set_min_width(w);
                                ui.set_min_height(20.0);
                                ui.spacing_mut().item_spacing.x = gap;
                                ui.painter().rect_filled(ui.max_rect(), 0.0, bg);

                                // Check mark column
                                ui.allocate_ui(egui::vec2(col_chk, 20.0), |ui| {
                                    if is_saved { ui.label(egui::RichText::new("\u{2713}").size(10.0).color(t.accent)); }
                                });
                                // Strike
                                ui.allocate_ui(egui::vec2(col_stk, 20.0), |ui| {
                                    ui.label(egui::RichText::new(format!("{:.0}", row.strike)).monospace().size(11.0).strong().color(egui::Color32::from_rgb(210,210,220)));
                                });
                                // Bid
                                ui.allocate_ui(egui::vec2(col_bid, 20.0), |ui| {
                                    ui.label(egui::RichText::new(format!("{:.2}", row.bid)).monospace().size(10.0).color(color));
                                });
                                // Ask
                                ui.allocate_ui(egui::vec2(col_ask, 20.0), |ui| {
                                    ui.label(egui::RichText::new(format!("{:.2}", row.ask)).monospace().size(10.0).color(t.dim));
                                });
                                // OI
                                ui.allocate_ui(egui::vec2(col_oi, 20.0), |ui| {
                                    let oi_str = if row.oi >= 1_000_000 { format!("{:.1}M", row.oi as f32 / 1_000_000.0) }
                                        else if row.oi >= 1_000 { format!("{},{:03}", row.oi / 1000, row.oi % 1000) }
                                        else { format!("{}", row.oi) };
                                    ui.label(egui::RichText::new(oi_str).monospace().size(10.0).color(t.dim.gamma_multiply(0.5)));
                                });
                            });

                            // Hover cursor + click handling
                            let row_resp = resp.response.interact(egui::Sense::click());
                            if row_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if (select_mode || ui.input(|i| i.modifiers.shift)) && row_resp.clicked() {
                                if is_saved { saved.retain(|s| s.contract != row.contract); }
                                else { saved.push(SavedOption { contract: row.contract.clone(), symbol: sym.into(), strike: row.strike, is_call, expiry: exp_label.into(), last: row.last }); }
                            }
                        };

                        // ── Helper to render one expiry block ──
                        let render_block = |ui: &mut egui::Ui, dte: i32, calls: &[OptionRow], puts: &[OptionRow], sym: &str, price: f32, saved: &mut Vec<SavedOption>, select_mode: bool, w: f32| {
                            let exp_label = format!("{}DTE", dte);
                            // Expiry header bar
                            ui.horizontal(|ui| {
                                ui.set_min_width(w);
                                ui.label(egui::RichText::new(&exp_label).monospace().size(11.0).strong().color(t.accent));
                            });
                            ui.add_space(1.0);

                            ui.label(egui::RichText::new("CALLS").monospace().size(9.0).strong().color(t.bull));
                            for row in calls { render_row(ui, row, true, &exp_label, sym, saved, select_mode, w); }

                            // Underlying divider
                            ui.horizontal(|ui| {
                                ui.set_min_width(w);
                                let divider_rect = ui.max_rect();
                                ui.painter().rect_filled(divider_rect, 0.0, t.toolbar_border.gamma_multiply(0.15));
                                ui.label(egui::RichText::new(format!("{}  ${:.2}", sym, price)).monospace().size(10.0).color(t.dim.gamma_multiply(0.5)));
                            });

                            ui.label(egui::RichText::new("PUTS").monospace().size(9.0).strong().color(t.bear));
                            for row in puts { render_row(ui, row, false, &exp_label, sym, saved, select_mode, w); }
                            ui.add_space(6.0);
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
                            render_block(ui, far_dte, &calls_f, &puts_f, &sym, chain_price, &mut watchlist.saved_options, sel, scroll_w);
                        });
                    }

                    // ── SAVED TAB ───────────────────────────────────────────
                    WatchlistTab::Saved => {
                        // DTE filter
                        ui.horizontal(|ui| {
                            for (f, label) in [(-1, "All"), (0, "0DTE"), (1, "1+DTE")] {
                                let active = watchlist.dte_filter == f;
                                let color = if active { t.accent } else { t.dim };
                                if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(9.0).color(color)).frame(false)).clicked() {
                                    watchlist.dte_filter = f;
                                }
                            }
                        });
                        ui.add_space(4.0);

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut remove_idx: Option<usize> = None;
                            for (i, opt) in watchlist.saved_options.iter().enumerate() {
                                let type_label = if opt.is_call { "C" } else { "P" };
                                let color = if opt.is_call { t.bull } else { t.bear };
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&opt.symbol).monospace().size(10.0).strong().color(egui::Color32::from_rgb(220,220,230)));
                                    ui.label(egui::RichText::new(format!("{:.0}{}", opt.strike, type_label)).monospace().size(10.0).color(color));
                                    ui.label(egui::RichText::new(format!("{:.2}", opt.last)).monospace().size(10.0).color(color));
                                    ui.label(egui::RichText::new(&opt.expiry).monospace().size(8.0).color(t.dim));
                                    if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim)).frame(false)).clicked() {
                                        remove_idx = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = remove_idx { watchlist.saved_options.remove(i); }
                            if watchlist.saved_options.is_empty() {
                                ui.label(egui::RichText::new("Click a contract in\nthe CHAIN tab to save it").monospace().size(9.0).color(t.dim));
                            }
                        });
                    }
                }
            });
    }

    // ── Orders / Positions / Alerts side panel (left of watchlist) ─────────────
    if watchlist.orders_panel_open {
        egui::SidePanel::right("orders_panel")
            .default_width(220.0)
            .min_width(180.0)
            .max_width(320.0)
            .frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin { left: 6, right: 6, top: 6, bottom: 6 }))
            .show(ctx, |ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("ORDERS").monospace().size(10.0).strong().color(t.accent));

                    // Count active orders across all panes
                    let total_orders: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).count()).sum();
                    if total_orders > 0 {
                        ui.label(egui::RichText::new(format!("({})", total_orders)).monospace().size(9.0).color(t.dim));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim)).frame(false)).clicked() {
                            watchlist.orders_panel_open = false;
                        }
                    });
                });

                // Action buttons
                ui.horizontal(|ui| {
                    let draft_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft).count()).sum();
                    let active_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).count()).sum();
                    let history_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Executed || o.status == OrderStatus::Cancelled).count()).sum();

                    if draft_count > 0 {
                        if ui.add(egui::Button::new(egui::RichText::new(format!("Place All ({})", draft_count)).monospace().size(9.0).color(t.accent))
                            .fill(egui::Color32::from_rgba_unmultiplied(t.accent.r(),t.accent.g(),t.accent.b(),30))
                            .stroke(egui::Stroke::new(1.0, t.accent)).corner_radius(2.0)).clicked() {
                            for pane in panes.iter_mut() {
                                for o in &mut pane.orders { if o.status == OrderStatus::Draft { o.status = OrderStatus::Placed; } }
                            }
                        }
                    }
                    if active_count > 0 {
                        if ui.add(egui::Button::new(egui::RichText::new("Cancel All").monospace().size(9.0).color(t.bear))
                            .corner_radius(2.0)).clicked() {
                            for pane in panes.iter_mut() {
                                for o in &mut pane.orders {
                                    if o.status == OrderStatus::Draft || o.status == OrderStatus::Placed { o.status = OrderStatus::Cancelled; }
                                }
                            }
                        }
                    }
                    if history_count > 0 {
                        if ui.add(egui::Button::new(egui::RichText::new("Clear").monospace().size(9.0).color(t.dim))
                            .corner_radius(2.0)).clicked() {
                            for pane in panes.iter_mut() {
                                pane.orders.retain(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed);
                            }
                        }
                    }
                });

                ui.add_space(4.0);
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left(), ui.cursor().min.y), egui::pos2(ui.min_rect().right(), ui.cursor().min.y)],
                    egui::Stroke::new(1.0, t.toolbar_border),
                );
                ui.add_space(4.0);

                // Group selection actions
                let sel_count = watchlist.selected_order_ids.len();
                if sel_count > 0 {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("{} selected", sel_count)).monospace().size(9.0).color(t.accent));
                        if ui.add(egui::Button::new(egui::RichText::new("Place").monospace().size(9.0).color(t.accent))
                            .stroke(egui::Stroke::new(0.5, t.accent)).corner_radius(2.0)).clicked() {
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
                        }
                        if ui.add(egui::Button::new(egui::RichText::new("Cancel").monospace().size(9.0).color(t.bear))
                            .corner_radius(2.0)).clicked() {
                            for (pi, oid) in &watchlist.selected_order_ids {
                                if *pi < panes.len() { cancel_order_with_pair(&mut panes[*pi].orders, *oid); }
                            }
                            watchlist.selected_order_ids.clear();
                        }
                        if ui.add(egui::Button::new(egui::RichText::new("Unarm").monospace().size(9.0).color(t.dim))
                            .corner_radius(2.0)).clicked() {
                            for (pi, oid) in &watchlist.selected_order_ids {
                                if let Some(o) = panes.get_mut(*pi).and_then(|p| p.orders.iter_mut().find(|o| o.id == *oid)) {
                                    if o.status == OrderStatus::Placed { o.status = OrderStatus::Draft; }
                                }
                            }
                            watchlist.selected_order_ids.clear();
                        }
                        if ui.add(egui::Button::new(egui::RichText::new("Deselect").monospace().size(8.0).color(t.dim)).frame(false)).clicked() {
                            watchlist.selected_order_ids.clear();
                        }
                    });
                    ui.add_space(2.0);
                }

                // Orders list — compact cards with checkboxes
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut cancel_order: Option<(usize, u32)> = None;
                    let full_w = ui.available_width();

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

                            // Row with checkbox
                            let resp = ui.horizontal(|ui| {
                                ui.set_min_width(full_w);

                                // Selection checkbox (only for active orders)
                                if is_active {
                                    let check_icon = if is_selected { "\u{25C9}" } else { "\u{25CB}" }; // ◉ / ○
                                    let check_color = if is_selected { t.accent } else { t.dim.gamma_multiply(0.4) };
                                    if ui.add(egui::Button::new(egui::RichText::new(check_icon).size(11.0).color(check_color)).frame(false).min_size(egui::vec2(14.0, 16.0))).clicked() {
                                        if is_selected {
                                            watchlist.selected_order_ids.retain(|(p, id)| !(*p == pi && *id == order.id));
                                        } else {
                                            watchlist.selected_order_ids.push((pi, order.id));
                                        }
                                    }
                                } else {
                                    ui.add_space(14.0);
                                }

                                ui.label(egui::RichText::new(order.label()).monospace().size(10.0).strong().color(color));
                                ui.label(egui::RichText::new(&pane.symbol).monospace().size(10.0).color(egui::Color32::from_rgb(200,200,210)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if is_active {
                                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim)).frame(false)).clicked() {
                                            cancel_order = Some((pi, order.id));
                                        }
                                    }
                                    ui.label(egui::RichText::new(status_text).monospace().size(9.0).color(status_color));
                                });
                            });
                            // Selected highlight
                            if is_selected {
                                ui.painter().rect_filled(resp.response.rect, 0.0, egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 15));
                            }

                            ui.horizontal(|ui| {
                                ui.add_space(14.0); // align with checkbox
                                ui.label(egui::RichText::new(format!("{:.2}", order.price)).monospace().size(10.0).color(color));
                                ui.label(egui::RichText::new(format!("x{}", order.qty)).monospace().size(9.0).color(t.dim));
                                ui.label(egui::RichText::new(fmt_notional(order.notional())).monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
                            });
                            ui.add_space(2.0);
                        }
                    }

                    if let Some((pi, oid)) = cancel_order {
                        cancel_order_with_pair(&mut panes[pi].orders, oid);
                    }

                    // Positions section
                    if !watchlist.positions.is_empty() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("POSITIONS").monospace().size(9.0).strong().color(t.dim));
                        ui.add_space(2.0);
                        let total_pnl: f32 = watchlist.positions.iter().map(|p| p.pnl()).sum();
                        for pos in &watchlist.positions {
                            let pnl = pos.pnl();
                            let pnl_pct = pos.pnl_pct();
                            let color = if pnl >= 0.0 { t.bull } else { t.bear };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&pos.symbol).monospace().size(10.0).strong().color(egui::Color32::from_rgb(220,220,230)));
                                ui.label(egui::RichText::new(format!("{}@{:.2}", pos.qty, pos.avg_price)).monospace().size(10.0).color(t.dim));
                                ui.label(egui::RichText::new(format!("{:+.2}", pnl)).monospace().size(10.0).color(color));
                                ui.label(egui::RichText::new(format!("({:+.1}%)", pnl_pct)).monospace().size(9.0).color(color));
                            });
                        }
                        ui.add_space(2.0);
                        let total_color = if total_pnl >= 0.0 { t.bull } else { t.bear };
                        ui.label(egui::RichText::new(format!("Total P&L: {:+.2}", total_pnl)).monospace().size(10.0).strong().color(total_color));
                    }

                    // Alerts section
                    if !watchlist.alerts.is_empty() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("ALERTS").monospace().size(9.0).strong().color(t.dim));
                        ui.add_space(2.0);
                        let mut remove_alert: Option<u32> = None;
                        for alert in &watchlist.alerts {
                            let dir = if alert.above { "\u{2191}" } else { "\u{2193}" }; // ↑ ↓
                            let color = if alert.triggered { t.accent } else { t.dim };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).color(egui::Color32::from_rgb(220,220,230)));
                                ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price)).monospace().size(10.0).color(color));
                                if alert.triggered {
                                    ui.label(egui::RichText::new("TRIGGERED").monospace().size(9.0).color(t.accent));
                                }
                                if !alert.message.is_empty() {
                                    ui.label(egui::RichText::new(&alert.message).monospace().size(9.0).color(t.dim));
                                }
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim)).frame(false)).clicked() {
                                    remove_alert = Some(alert.id);
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
        let bx = |i:f32| rect.left()+(i-vs)*bs+bs*0.5-off;
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
        // Helper: draw a line with optional dash pattern
        let draw_line = |painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: egui::Stroke, style: LineStyle| {
            match style {
                LineStyle::Solid => { painter.line_segment([a, b], stroke); }
                LineStyle::Dashed | LineStyle::Dotted => {
                    let (dash_l, gap_l) = if style == LineStyle::Dashed { (8.0, 4.0) } else { (2.0, 3.0) };
                    let dir = b - a;
                    let len = dir.length();
                    if len < 1.0 { return; }
                    let norm = dir / len;
                    let step = dash_l + gap_l;
                    let mut d = 0.0;
                    while d < len {
                        let p0 = a + norm * d;
                        let p1 = a + norm * (d + dash_l).min(len);
                        painter.line_segment([p0, p1], stroke);
                        d += step;
                    }
                }
            }
        };

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
                    draw_line(&painter, egui::pos2(rect.left(),y), egui::pos2(rect.left()+cw,y), sc, ls);
                    if is_sel {
                        painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y), 4.0, egui::Color32::from_rgb(74,158,255));
                    }
                }
                DrawingKind::TrendLine{price0,bar0,price1,bar1}=>{
                    let p0=egui::pos2(bx(*bar0),py(*price0)); let p1=egui::pos2(bx(*bar1),py(*price1));
                    draw_line(&painter, p0, p1, sc, ls);
                    if is_sel {
                        painter.circle_filled(p0, 5.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_stroke(p0, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                        painter.circle_filled(p1, 5.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_stroke(p1, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                    }
                }
                DrawingKind::HZone{price0,price1}=>{
                    let(y0,y1)=(py(*price0),py(*price1));
                    let fill = hex_to_color(&d.color, d.opacity * 0.1);
                    painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(),y0.min(y1)),egui::pos2(rect.left()+cw,y0.max(y1))),0.0,fill);
                    draw_line(&painter, egui::pos2(rect.left(),y0), egui::pos2(rect.left()+cw,y0), sc, ls);
                    draw_line(&painter, egui::pos2(rect.left(),y1), egui::pos2(rect.left()+cw,y1), sc, ls);
                    if is_sel {
                        painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y0), 4.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y1), 4.0, egui::Color32::from_rgb(74,158,255));
                    }
                }
                DrawingKind::BarMarker{bar,price,up}=>{
                    let x=bx(*bar); let y=py(*price);
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
                .map(|o| (o.price, o.color(t), o.label()));

            if let Some((order_price, color, order_label)) = order_data {
                let y = py(order_price);
                let popup_pos = egui::pos2(rect.left() + 10.0, y + 14.0);
                let mut close_editor = false;
                let mut apply_price: Option<f32> = None;
                let mut apply_qty: Option<u32> = None;
                let mut cancel_it = false;

                egui::Window::new(format!("order_edit_{}", edit_id))
                    .fixed_pos(popup_pos)
                    .fixed_size(egui::vec2(180.0, 0.0))
                    .title_bar(false)
                    .frame(egui::Frame::popup(&ctx.style()).fill(t.toolbar_bg).inner_margin(8.0)
                        .stroke(egui::Stroke::new(1.0, color)))
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("EDIT {}", order_label)).monospace().size(10.0).strong().color(color));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(10.0).color(t.dim)).frame(false)).clicked() {
                                    close_editor = true;
                                }
                            });
                        });
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Price").monospace().size(9.0).color(t.dim));
                            let resp = ui.add(egui::TextEdit::singleline(&mut chart.edit_order_price)
                                .desired_width(80.0).font(egui::FontId::monospace(11.0)));
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                if let Ok(p) = chart.edit_order_price.parse::<f32>() { apply_price = Some(p); }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Qty  ").monospace().size(9.0).color(t.dim));
                            let resp = ui.add(egui::TextEdit::singleline(&mut chart.edit_order_qty)
                                .desired_width(80.0).font(egui::FontId::monospace(11.0)));
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                if let Ok(q) = chart.edit_order_qty.parse::<u32>() { apply_qty = Some(q.max(1)); }
                            }
                        });

                        ui.add_space(4.0);
                        if ui.add(egui::Button::new(egui::RichText::new(format!("{} Cancel Order", Icon::TRASH)).monospace().size(10.0).color(t.bear))
                            .min_size(egui::vec2(164.0, 22.0)).corner_radius(3.0)).clicked() {
                            cancel_it = true;
                        }
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
            let panel_w = 190.0;
            let panel_h = 56.0;
            let panel_pos = egui::pos2(rect.left() + 8.0, rect.top() + pt + ch - panel_h - 8.0);

            egui::Window::new(format!("order_entry_{}", pane_idx))
                .fixed_pos(panel_pos)
                .fixed_size(egui::vec2(panel_w, panel_h))
                .title_bar(false)
                .frame(egui::Frame::popup(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 235))
                    .inner_margin(6.0))
                .show(ctx, |ui| {
                    let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
                    let spread = (last_price * 0.0001).max(0.01);

                    // Row 1: [-] qty [+] | price/last | MKT/LMT
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let step = if chart.order_qty >= 100 { 10 } else if chart.order_qty >= 10 { 5 } else { 1 };
                        if ui.add(egui::Button::new(egui::RichText::new("-").monospace().size(10.0)).min_size(egui::vec2(16.0, 18.0)).corner_radius(2.0)).clicked() {
                            chart.order_qty = chart.order_qty.saturating_sub(step).max(1);
                        }
                        ui.label(egui::RichText::new(format!("{}", chart.order_qty)).monospace().size(10.0).strong().color(egui::Color32::from_rgb(220,220,230)));
                        if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(10.0)).min_size(egui::vec2(16.0, 18.0)).corner_radius(2.0)).clicked() {
                            chart.order_qty += step;
                        }

                        ui.add_space(4.0);

                        // Price input or last price display
                        if chart.order_market {
                            ui.label(egui::RichText::new(format!("{:.2}", last_price)).monospace().size(10.0).color(t.dim));
                        } else {
                            ui.add(egui::TextEdit::singleline(&mut chart.order_limit_price)
                                .desired_width(58.0).font(egui::FontId::monospace(10.0)).hint_text("Price"));
                        }

                        // MKT/LMT toggle
                        let mkt_label = if chart.order_market { "MKT" } else { "LMT" };
                        let mkt_active = chart.order_market;
                        if ui.add(egui::Button::new(egui::RichText::new(mkt_label).monospace().size(9.0).strong()
                            .color(if mkt_active { t.accent } else { t.dim }))
                            .fill(if mkt_active { egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 35) } else { t.toolbar_bg })
                            .stroke(egui::Stroke::new(0.5, t.toolbar_border)).corner_radius(2.0)
                            .min_size(egui::vec2(28.0, 18.0))).clicked() {
                            chart.order_market = !chart.order_market;
                            if !chart.order_market && chart.order_limit_price.is_empty() {
                                chart.order_limit_price = format!("{:.2}", last_price);
                            }
                        }
                    });

                    ui.add_space(2.0);

                    // Row 2: BUY | SELL | armed glyph
                    let buy_price = if chart.order_market { last_price + spread } else {
                        chart.order_limit_price.parse::<f32>().unwrap_or(last_price)
                    };
                    let sell_price = if chart.order_market { last_price - spread } else {
                        chart.order_limit_price.parse::<f32>().unwrap_or(last_price)
                    };

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        // BUY
                        if ui.add(egui::Button::new(egui::RichText::new(format!("BUY {:.2}", buy_price)).monospace().size(9.0).strong().color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 180))
                            .min_size(egui::vec2(76.0, 20.0)).corner_radius(3.0)).clicked() {
                            let id = chart.next_order_id; chart.next_order_id += 1;
                            let s = if chart.armed { OrderStatus::Placed } else { OrderStatus::Draft };
                            chart.orders.push(OrderLevel { id, side: OrderSide::Buy, price: buy_price, qty: chart.order_qty, status: s, pair_id: None });
                            if !chart.armed { chart.pending_confirms.push((id, std::time::Instant::now())); }
                        }
                        // SELL
                        if ui.add(egui::Button::new(egui::RichText::new(format!("SELL {:.2}", sell_price)).monospace().size(9.0).strong().color(egui::Color32::WHITE))
                            .fill(egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 180))
                            .min_size(egui::vec2(76.0, 20.0)).corner_radius(3.0)).clicked() {
                            let id = chart.next_order_id; chart.next_order_id += 1;
                            let s = if chart.armed { OrderStatus::Placed } else { OrderStatus::Draft };
                            chart.orders.push(OrderLevel { id, side: OrderSide::Sell, price: sell_price, qty: chart.order_qty, status: s, pair_id: None });
                            if !chart.armed { chart.pending_confirms.push((id, std::time::Instant::now())); }
                        }
                        // Armed toggle — glyph only, tooltip on hover
                        let armed_icon = if chart.armed { Icon::SHIELD_WARNING } else { Icon::PLAY };
                        let armed_color = if chart.armed { t.accent } else { t.dim.gamma_multiply(0.5) };
                        let armed_resp = ui.add(egui::Button::new(egui::RichText::new(armed_icon).size(12.0).color(armed_color))
                            .fill(if chart.armed { egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 30) } else { egui::Color32::TRANSPARENT })
                            .stroke(egui::Stroke::NONE).min_size(egui::vec2(20.0, 20.0)).corner_radius(2.0));
                        if armed_resp.clicked() { chart.armed = !chart.armed; }
                        if armed_resp.hovered() {
                            egui::show_tooltip(ui.ctx(), ui.layer_id(), egui::Id::new("armed_tip"), |ui| {
                                ui.label(egui::RichText::new(if chart.armed { "Armed — orders fire immediately" } else { "Unarmed — orders need confirmation" }).monospace().size(9.0));
                            });
                        }
                    });
                });

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
                                    if ui.add(egui::Button::new(egui::RichText::new("\u{2713}").size(11.0).color(t.bull))
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
                    DrawingKind::TrendLine{price0,bar0,price1,bar1} => {
                        let p0 = egui::pos2(bx(*bar0), py(*price0)); let p1 = egui::pos2(bx(*bar1), py(*price1));
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
                    DrawingKind::BarMarker{bar,price,..} => {
                        if egui::pos2(bx(*bar),py(*price)).distance(pos) < 12.0 { return Some((d.id.clone(), -1)); }
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

        let hit_at = |px: f32, py_pos: f32, drawings: &[Drawing]| -> Option<(String, i32)> {
            for d in drawings.iter().rev() {
                match &d.kind {
                    DrawingKind::HLine{price} => {
                        if (py_pos - py(*price)).abs() < 12.0 { return Some((d.id.clone(), -1)); }
                    }
                    DrawingKind::TrendLine{price0,bar0,price1,bar1} => {
                        let p0 = egui::pos2(bx(*bar0), py(*price0));
                        let p1 = egui::pos2(bx(*bar1), py(*price1));
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
                    DrawingKind::BarMarker{bar,price,..} => {
                        if egui::pos2(bx(*bar),py(*price)).distance(egui::pos2(px,py_pos)) < 12.0 { return Some((d.id.clone(), -1)); }
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
                            let mut d = Drawing::new(new_uuid(), DrawingKind::TrendLine { price0: p0, bar0: b0, price1: price, bar1: bar });
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
                            let mut d = Drawing::new(new_uuid(), DrawingKind::BarMarker { bar: bar_idx as f32, price: snap_price, up });
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
                            DrawingKind::TrendLine{price0,bar0,price1,bar1} => match ep {
                                0 => { *price0 = new_p; *bar0 = new_b; }
                                1 => { *price1 = new_p; *bar1 = new_b; }
                                _ => { *price0 += dp; *price1 += dp; *bar0 += db; *bar1 += db; }
                            },
                            DrawingKind::HZone{price0,price1} => match ep {
                                0 => *price0 = new_p,
                                1 => *price1 = new_p,
                                _ => { *price0 += dp; *price1 += dp; }
                            },
                            DrawingKind::BarMarker{bar,price,..} => { *bar += db; *price += dp; },
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
                chart.orders.push(OrderLevel { id, side: OrderSide::Buy, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None });
                ui.close_menu();
            }
            if ui.button(egui::RichText::new(format!("{} Sell Order", Icon::ARROW_FAT_DOWN)).color(t.bear)).clicked() {
                let id = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id, side: OrderSide::Sell, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None });
                ui.close_menu();
            }
            if ui.button(egui::RichText::new(format!("{} Stop Loss", Icon::SHIELD_WARNING)).color(t.bear)).clicked() {
                let id = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id, side: OrderSide::Stop, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None });
                ui.close_menu();
            }
            // OCO Bracket (target +1%, stop -1%)
            if ui.button(egui::RichText::new(format!("\u{21C5} OCO Bracket")).color(egui::Color32::from_rgb(167,139,250))).clicked() {
                let target_price = click_price * 1.01;
                let stop_price = click_price * 0.99;
                let id1 = chart.next_order_id; chart.next_order_id += 1;
                let id2 = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id: id1, side: OrderSide::OcoTarget, price: target_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id2) });
                chart.orders.push(OrderLevel { id: id2, side: OrderSide::OcoStop, price: stop_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id1) });
                ui.close_menu();
            }
            // Trigger Order (buy entry at click, sell target +2%)
            if ui.button(egui::RichText::new(format!("\u{27F2} Trigger Order")).color(t.accent)).clicked() {
                let target_price = click_price * 1.02;
                let id1 = chart.next_order_id; chart.next_order_id += 1;
                let id2 = chart.next_order_id; chart.next_order_id += 1;
                chart.orders.push(OrderLevel { id: id1, side: OrderSide::TriggerBuy, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id2) });
                chart.orders.push(OrderLevel { id: id2, side: OrderSide::TriggerSell, price: target_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id1) });
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
enum WatchlistTab { Stocks, Chain, Saved }

struct Watchlist {
    open: bool,
    tab: WatchlistTab,
    items: Vec<WatchlistItem>,
    search_query: String,
    search_results: Vec<(String, String)>,
    // Toolbar
    #[allow(dead_code)] toolbar_scroll: f32,
    shortcuts_open: bool, // keyboard shortcuts help panel
    trendline_filter_open: bool, // trendline filter dropdown
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
    // Saved options
    saved_options: Vec<SavedOption>,
    dte_filter: i32,
}

const DEFAULT_WATCHLIST: &[&str] = &["SPY","QQQ","IWM","DIA","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOGL","GLD"];

impl Watchlist {
    fn new() -> Self {
        let items = DEFAULT_WATCHLIST.iter().map(|&s| WatchlistItem {
            symbol: s.into(), price: 0.0, prev_close: 0.0, loaded: false,
        }).collect();
        Self { open: false, tab: WatchlistTab::Stocks, items, search_query: String::new(), search_results: vec![],
               toolbar_scroll: 0.0, shortcuts_open: false, trendline_filter_open: false,
               orders_panel_open: false, order_entry_open: false, selected_order_ids: vec![], positions: vec![], alerts: vec![], next_alert_id: 1, alert_query: String::new(),
               chain_symbol: "SPY".into(), chain_sym_input: String::new(), chain_num_strikes: 10, chain_far_dte: 1,
               chain_0dte: (vec![], vec![]), chain_far: (vec![], vec![]),
               chain_select_mode: false, saved_options: vec![], dte_filter: -1 }
    }

    fn add_symbol(&mut self, sym: &str) {
        let s = sym.to_uppercase();
        if !self.items.iter().any(|i| i.symbol == s) {
            self.items.push(WatchlistItem { symbol: s, price: 0.0, prev_close: 0.0, loaded: false });
        }
    }

    fn remove_symbol(&mut self, sym: &str) {
        self.items.retain(|i| i.symbol != sym);
    }

    fn set_price(&mut self, sym: &str, price: f32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.symbol == sym) {
            item.price = price;
        }
    }

    fn set_prev_close(&mut self, sym: &str, prev_close: f32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.symbol == sym) {
            item.prev_close = prev_close;
            item.loaded = true;
        }
    }
}

// ─── Black-Scholes options pricing (matches WebView's optionsSim.ts) ────────

fn normal_cdf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let poly = t * (0.319381530 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    let phi = (-0.5 * x * x).exp() / (2.0 * std::f32::consts::PI).sqrt();
    let cdf = 1.0 - phi * poly;
    if x >= 0.0 { cdf } else { 1.0 - cdf }
}

fn bs_price(s: f32, k: f32, t: f32, r: f32, iv: f32, is_call: bool) -> f32 {
    if t <= 0.0 { return if is_call { (s - k).max(0.0) } else { (k - s).max(0.0) }; }
    let d1 = ((s / k).ln() + (r + 0.5 * iv * iv) * t) / (iv * t.sqrt());
    let d2 = d1 - iv * t.sqrt();
    if is_call { s * normal_cdf(d1) - k * (-r * t).exp() * normal_cdf(d2) }
    else { k * (-r * t).exp() * normal_cdf(-d2) - s * normal_cdf(-d1) }
}

#[allow(dead_code)]
fn bs_delta(s: f32, k: f32, t: f32, r: f32, iv: f32, is_call: bool) -> f32 {
    if t <= 0.0 { return if is_call { if s > k { 1.0 } else { 0.0 } } else { if s < k { -1.0 } else { 0.0 } }; }
    let d1 = ((s / k).ln() + (r + 0.5 * iv * iv) * t) / (iv * t.sqrt());
    if is_call { normal_cdf(d1) } else { normal_cdf(d1) - 1.0 }
}

fn strike_interval(price: f32) -> f32 {
    if price < 20.0 { 0.5 } else if price < 50.0 { 1.0 } else if price < 100.0 { 2.5 }
    else if price < 200.0 { 5.0 } else if price < 500.0 { 10.0 } else { 25.0 }
}

fn atm_strike(price: f32) -> f32 {
    let interval = strike_interval(price);
    (price / interval).round() * interval
}

fn get_iv(s: f32, k: f32, dte: i32) -> f32 {
    let base = 0.28;
    let moneyness = (k / s).ln();
    let smile = 0.06 * moneyness * moneyness;
    let skew = -0.05 * moneyness;
    let term = if dte <= 0 { 1.25 } else if dte == 1 { 1.10 } else { 1.0 };
    (base + smile + skew) * term
}

fn sim_oi(underlying: f32, strike: f32, dte: i32) -> i32 {
    let interval = strike_interval(underlying);
    let atm = atm_strike(underlying);
    let strikes_away = ((strike - atm).abs() / interval) as f32;
    let base = if dte <= 0 { 18000.0 } else if dte == 1 { 35000.0 } else { 50000.0 };
    let raw = base * (-0.35 * strikes_away * strikes_away).exp();
    let noise = 1.0 + 0.3 * (strike * 17.3 + dte as f32 * 5.7).sin();
    (raw * noise).max(100.0) as i32
}

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

/// Fetch daily previous close for all watchlist symbols (background thread).
fn fetch_watchlist_prices(symbols: Vec<String>) {
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0").build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        for sym in &symbols {
            // Try Redis cache first, then Yahoo
            if let Some(bars) = crate::bar_cache::get(sym, "1d") {
                if bars.len() >= 2 {
                    let price = bars.last().map(|b| b.close as f32).unwrap_or(0.0);
                    let prev = bars[bars.len()-2].close as f32;
                    // Send via broadcast channel
                    let cmd = ChartCommand::WatchlistPrice { symbol: sym.clone(), price, prev_close: prev };
                    crate::send_to_native_chart(cmd);
                    continue;
                }
            }
            let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d", sym);
            if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(5)).send() {
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
            .with_active(true))
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
        let wl_syms: Vec<String> = wl.items.iter().map(|i| i.symbol.clone()).collect();
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
                        other => cmds_to_requeue.push(other),
                    }
                }
                // Re-inject non-watchlist commands (they'll be picked up by draw_chart)
                // Can't re-send to rx since we own the receiver. Use a temp buffer approach:
                // Actually, draw_chart also drains rx. So we need to pass these through.
                // Simpler: just process ALL commands here and pass pane commands to the right pane.
                for cmd in cmds_to_requeue {
                    let sym = match &cmd {
                        ChartCommand::LoadBars { symbol, .. } | ChartCommand::UpdateLastBar { symbol, .. } | ChartCommand::AppendBar { symbol, .. } => Some(symbol.clone()),
                        ChartCommand::IndicatorSourceBars { .. } => None,
                        _ => None,
                    };
                    if let Some(s) = sym {
                        if let Some(p) = cw.panes.iter_mut().find(|p| p.symbol == s) { p.process(cmd); }
                        else if let Some(p) = cw.panes.get_mut(cw.active_pane) { p.process(cmd); }
                    } else if let Some(p) = cw.panes.get_mut(cw.active_pane) { p.process(cmd); }
                }

                // Also update watchlist from tick data (UpdateLastBar contains current price)
                for item in &mut cw.watchlist.items {
                    // Check if any pane has this symbol and get its latest price
                    if let Some(pane) = cw.panes.iter().find(|p| p.symbol == item.symbol) {
                        if let Some(bar) = pane.bars.last() {
                            item.price = bar.close;
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

/// Fetch bars from Redis cache → OCOCO → yfinance sidecar → Yahoo Finance v8 on a background thread.
/// Sends LoadBars command via the global NATIVE_CHART_TXS channels (all windows).
/// Results are cached in Redis for subsequent requests.
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
    let pane_data: Vec<serde_json::Value> = panes.iter().map(|p| serde_json::json!({
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

    (panes, layout)
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

