//! Native GPU chart renderer — winit (any_thread) + egui for all rendering.
//! egui handles UI + chart painting. winit handles window on non-main thread.

use std::sync::{mpsc, Arc, Mutex};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
    dpi::PhysicalSize,
};

use super::{Bar, ChartCommand, Drawing, DrawingKind, DrawingGroup, LineStyle};
use crate::ui_kit::{self, icons::Icon};

// ─── Themes ───────────────────────────────────────────────────────────────────

struct Theme { name: &'static str, bg: egui::Color32, bull: egui::Color32, bear: egui::Color32, dim: egui::Color32 }
const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }
const THEMES: &[Theme] = &[
    Theme { name: "Midnight",   bg: rgb(13,13,13),  bull: rgb(46,204,113),  bear: rgb(231,76,60),  dim: rgb(102,102,102) },
    Theme { name: "Nord",       bg: rgb(46,52,64),  bull: rgb(163,190,140), bear: rgb(191,97,106), dim: rgb(129,161,193) },
    Theme { name: "Monokai",    bg: rgb(39,40,34),  bull: rgb(166,226,46),  bear: rgb(249,38,114), dim: rgb(165,159,133) },
    Theme { name: "Solarized",  bg: rgb(0,43,54),   bull: rgb(133,153,0),   bear: rgb(220,50,47),  dim: rgb(131,148,150) },
    Theme { name: "Dracula",    bg: rgb(40,42,54),  bull: rgb(80,250,123),  bear: rgb(255,85,85),  dim: rgb(189,147,249) },
    Theme { name: "Gruvbox",    bg: rgb(40,40,40),  bull: rgb(184,187,38),  bear: rgb(251,73,52),  dim: rgb(213,196,161) },
    Theme { name: "Catppuccin", bg: rgb(30,30,46),  bull: rgb(166,227,161), bear: rgb(243,139,168),dim: rgb(180,190,254) },
    Theme { name: "Tokyo Night",bg: rgb(26,27,38),  bull: rgb(158,206,106), bear: rgb(247,118,142),dim: rgb(122,162,247) },
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

// ─── Chart state ──────────────────────────────────────────────────────────────

struct Chart {
    symbol: String, timeframe: String,
    bars: Vec<Bar>, timestamps: Vec<i64>, drawings: Vec<Drawing>,
    indicators: Vec<(Vec<f32>, egui::Color32, String)>,
    indicator_bar_count: usize, // bar count when indicators were last computed
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
    hide_all_drawings: bool,
    hide_all_indicators: bool,
    draw_color: String, // current drawing color
    next_draw_id: u32,
    zoom_selecting: bool, zoom_start: egui::Pos2,
    // Symbol picker
    picker_open: bool, picker_query: String,
    picker_results: Vec<(String, String, String)>, // (symbol, name, exchange/type)
    picker_last_query: String, // debounce: only search when query changes
    picker_searching: bool, // true while background search is in flight
    picker_rx: Option<mpsc::Receiver<Vec<(String, String, String)>>>, // receives search results from bg thread
    picker_pos: egui::Pos2, // anchor position for the popup
    recent_symbols: Vec<(String, String)>, // (symbol, name) — most recent first, max 20
    // Symbol/timeframe change request — signals the App to reload data
    pending_symbol_change: Option<String>,
    pending_timeframe_change: Option<String>,
    // Reusable buffers to avoid per-frame allocations
    indicator_pts_buf: Vec<egui::Pos2>,
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
            bars: vec![], timestamps: vec![], drawings: vec![], indicators: vec![], indicator_bar_count: 0,
            vs: 0.0, vc: 200, price_lock: None, auto_scroll: true,
            last_input: std::time::Instant::now(), tick_counter: 0,
            last_candle_time: std::time::Instant::now(), sim_price: 0.0,
            sim_seed: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42),
            theme_idx: 5, // Gruvbox
            draw_tool: String::new(), pending_pt: None,
            selected_id: None, selected_ids: vec![], dragging_drawing: None,
            drag_start_price: 0.0, drag_start_bar: 0.0,
            groups: vec![DrawingGroup { id: "default".into(), name: "Temp".into(), color: None }],
            hidden_groups: vec![], hide_all_drawings: false, hide_all_indicators: false,
            draw_color: "#4a9eff".into(), next_draw_id: 0,
            zoom_selecting: false, zoom_start: egui::Pos2::ZERO,
            picker_open: false, picker_query: String::new(), picker_results: vec![],
            picker_last_query: String::new(), picker_searching: false, picker_rx: None, picker_pos: egui::Pos2::ZERO,
            recent_symbols: vec![("AAPL".into(), "Apple".into()), ("SPY".into(), "S&P 500 ETF".into()), ("TSLA".into(), "Tesla".into()), ("NVDA".into(), "Nvidia".into()), ("MSFT".into(), "Microsoft".into())],
            pending_symbol_change: None, pending_timeframe_change: None,
            indicator_pts_buf: Vec::with_capacity(512) }
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
            _ => {}
        }
    }
    /// Update indicators. Full recompute when indicator_bar_count == 0 (data reload),
    /// incremental append when a single bar was added (simulation tick).
    fn update_indicators(&mut self) {
        let n = self.bars.len();
        if n == self.indicator_bar_count { return; }

        // Full recompute on data load (indicator_bar_count reset to 0)
        if self.indicator_bar_count == 0 || n < self.indicator_bar_count {
            self.indicator_bar_count = n;
            let closes: Vec<f32> = self.bars.iter().map(|b| b.close).collect();
            self.indicators.clear();
            if closes.len() >= 20 { self.indicators.push((compute_sma(&closes, 20), egui::Color32::from_rgba_unmultiplied(0,190,240,200), "SMA20".into())); }
            if closes.len() >= 50 { self.indicators.push((compute_sma(&closes, 50), egui::Color32::from_rgba_unmultiplied(240,150,25,180), "SMA50".into())); }
            self.indicators.push((compute_ema(&closes, 12), egui::Color32::from_rgba_unmultiplied(240,215,50,170), "EMA12".into()));
            self.indicators.push((compute_ema(&closes, 26), egui::Color32::from_rgba_unmultiplied(178,102,230,170), "EMA26".into()));
            return;
        }

        // Incremental: extend each indicator for newly added bars
        let old = self.indicator_bar_count;
        self.indicator_bar_count = n;
        for idx in old..n {
            let close = self.bars[idx].close;
            for (vals, _, name) in &mut self.indicators {
                let period = match name.as_str() { "SMA20" => 20, "SMA50" => 50, "EMA12" => 12, "EMA26" => 26, _ => continue };
                if name.starts_with("SMA") {
                    if idx >= period {
                        let sum: f32 = self.bars[idx+1-period..=idx].iter().map(|b| b.close).sum();
                        vals.push(sum / period as f32);
                    } else { vals.push(f32::NAN); }
                } else {
                    // EMA: use previous EMA value
                    let k = 2.0 / (period as f32 + 1.0);
                    let prev = if vals.is_empty() { close } else { *vals.last().unwrap() };
                    let v = if prev.is_nan() {
                        if idx >= period - 1 {
                            self.bars[idx+1-period..=idx].iter().map(|b| b.close).sum::<f32>() / period as f32
                        } else { f32::NAN }
                    } else {
                        close * k + prev * (1.0 - k)
                    };
                    vals.push(v);
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

fn draw_chart(ctx: &egui::Context, panes: &mut Vec<Chart>, active_pane: &mut usize, layout: &mut Layout, rx: &mpsc::Receiver<ChartCommand>) {
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
    let n = panes[*active_pane].bars.len();

    let ap = *active_pane;
    span_begin("top_panel");
    egui::TopBottomPanel::top("tb").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Symbol ticker — click to open picker
            let sym_text = egui::RichText::new(&panes[ap].symbol).strong().size(14.0).color(t.bull);
            let sym_btn = ui.add(egui::Button::new(sym_text).frame(false));
            if sym_btn.clicked() {
                panes[ap].picker_open = !panes[ap].picker_open;
                panes[ap].picker_query.clear();
                panes[ap].picker_results.clear();
                panes[ap].picker_last_query.clear();
                panes[ap].picker_pos = egui::pos2(sym_btn.rect.left(), sym_btn.rect.bottom());
            }
            ui.separator();
            for &tf in &["1m","5m","15m","30m","1h","4h","1d","1wk"] {
                let is_active_tf = panes[ap].timeframe == tf;
                let text = egui::RichText::new(tf).small()
                    .color(if is_active_tf { t.bull } else { t.dim });
                if ui.add(egui::Button::new(text).frame(false).min_size(egui::vec2(24.0, 18.0)))
                    .clicked() && !is_active_tf
                {
                    panes[ap].pending_timeframe_change = Some(tf.to_string());
                }
            }
            ui.separator();
            if let Some(b) = panes[ap].bars.last() {
                let c = if b.close>=b.open { t.bull } else { t.bear };
                ui.label(egui::RichText::new(format!("O{:.2} H{:.2} L{:.2} C{:.2} V{:.0}",b.open,b.high,b.low,b.close,b.volume)).monospace().size(11.0).color(c));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Layout selector
                for &ly in ALL_LAYOUTS {
                    let is_cur = *layout == ly;
                    let txt = egui::RichText::new(ly.label()).small()
                        .color(if is_cur { t.bull } else { t.dim });
                    if ui.add(egui::Button::new(txt).frame(false).min_size(egui::vec2(20.0, 18.0))).clicked() && !is_cur {
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
                ui.separator();
                // Theme dropdown
                {
                    let mut ti = panes[ap].theme_idx;
                    egui::ComboBox::from_id_salt("thm").selected_text(THEMES[ti].name).width(100.0).show_ui(ui, |ui| {
                        for (i,th) in THEMES.iter().enumerate() { ui.selectable_value(&mut ti, i, th.name); }
                    });
                    if ti != panes[ap].theme_idx {
                        for p in panes.iter_mut() { p.theme_idx = ti; }
                    }
                }
                // Drawing tools with icons
                for (tool, icon, label) in [
                    ("trendline", Icon::LINE_SEGMENT_BOLD, "Trend"),
                    ("hline", Icon::MINUS_BOLD, "HLine"),
                    ("hzone", Icon::RECTANGLE_BOLD, "Zone"),
                    ("barmarker", Icon::MAP_PIN_BOLD, "Mark"),
                ] {
                    let _ = label;
                    if ui.selectable_label(panes[ap].draw_tool==tool, format!("{} {}", icon, label)).clicked() {
                        panes[ap].draw_tool = if panes[ap].draw_tool==tool { String::new() } else { tool.into() };
                        panes[ap].pending_pt = None;
                    }
                }
                ui.separator();
                for &c in PRESET_COLORS {
                    let color = hex_to_color(c, 1.0);
                    let is_cur = panes[ap].draw_color == c;
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                    ui.painter().circle_filled(r.center(), if is_cur { 6.0 } else { 5.0 }, color);
                    if is_cur { ui.painter().circle_stroke(r.center(), 7.0, egui::Stroke::new(1.5, egui::Color32::WHITE)); }
                    if resp.clicked() { panes[ap].draw_color = c.to_string(); }
                }
                if !panes[ap].auto_scroll { if ui.button(format!("{} LIVE", Icon::PLAY)).clicked() { panes[ap].auto_scroll=true; panes[ap].price_lock=None; panes[ap].vs=(n as f32-panes[ap].vc as f32+8.0).max(0.0); } }
                else { ui.label(egui::RichText::new(format!("{} LIVE", Icon::CHART_LINE)).color(t.bull).small()); }
            });
        });
    });
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

    // Status bar and style toolbar use active pane
    let chart = &mut panes[ap];
    if !chart.draw_tool.is_empty() {
        egui::TopBottomPanel::bottom("st").show(ctx, |ui| {
            let h = match chart.draw_tool.as_str() { "hline"=>"Click to place HLine (Esc cancel)", "trendline" if chart.pending_pt.is_some()=>"Click 2nd point (Esc cancel)", "trendline"=>"Click 1st point (Esc cancel)", _=>"" };
            ui.label(egui::RichText::new(h).color(egui::Color32::from_rgb(255,200,50)));
        });
    }

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

    let t = &THEMES[panes[ap].theme_idx];
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
        let (pr,pt,pb) = (80.0_f32, 4.0_f32, 30.0_f32);
        let (cw,ch) = (w-pr, h-pt-pb);
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
            let d=if p>=10.0{2}else{4}; painter.text(egui::pos2(rect.left()+cw+4.0,y),egui::Align2::LEFT_CENTER,format!("{:.1$}",p,d),egui::FontId::monospace(10.0),t.dim);
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
                            // Vertical grid line
                            painter.line_segment([egui::pos2(x,rect.top()+pt),egui::pos2(x,rect.top()+pt+ch)],egui::Stroke::new(0.3,t.dim.gamma_multiply(0.2)));
                            // Time label
                            let txt = if time_interval >= 86400 {
                                let days = (ti / 86400) as i32; let y2k = days - 10957;
                                let month = ((y2k % 365) / 30 + 1).min(12).max(1);
                                let day = ((y2k % 365) % 30 + 1).min(31).max(1);
                                format!("{:02}/{:02}", month, day)
                            } else {
                                let h = ((ti % 86400) / 3600) as u32;
                                let m = ((ti % 3600) / 60) as u32;
                                format!("{:02}:{:02}", h, m)
                            };
                            painter.text(egui::pos2(x,rect.top()+pt+ch+8.0),egui::Align2::CENTER_TOP,txt,egui::FontId::monospace(9.0),t.dim);
                        }
                    }
                    ti += time_interval;
                }
            }
        }

        // Volume + candles + indicators + drawings
        span_begin("pane_render");
        let mut mv:f32=0.0;
        for i in (vs as u32)..end { if let Some(b)=chart.bars.get(i as usize) { mv=mv.max(b.volume); } }
        if mv==0.0{mv=1.0;}
        for i in (vs as u32)..end { if let Some(b)=chart.bars.get(i as usize) {
            let x=bx(i as f32); let vh=(b.volume/mv)*ch*0.2;
            let c=if b.close>=b.open{t.bull.gamma_multiply(0.2)}else{t.bear.gamma_multiply(0.2)};
            let bw=(bs*0.4).max(1.0);
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(x-bw,rect.top()+pt+ch-vh),egui::pos2(x+bw,rect.top()+pt+ch)),0.0,c);
        }}

        // Candles
        for i in (vs as u32)..end { if let Some(b)=chart.bars.get(i as usize) {
            let x=bx(i as f32); let c=if b.close>=b.open{t.bull}else{t.bear};
            let bt=py(b.open.max(b.close)); let bb=py(b.open.min(b.close));
            let wt=py(b.high); let wb=py(b.low); let bw=(bs*0.35).max(1.0);
            painter.line_segment([egui::pos2(x,wt),egui::pos2(x,wb)],egui::Stroke::new(1.0,c));
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(x-bw,bt),egui::vec2(bw*2.0,(bb-bt).max(1.0))),1.0,c);
        }}

        // Indicators (reuse buffer to avoid per-frame Vec allocations)
        if !chart.hide_all_indicators { for (vals,color,_) in &chart.indicators {
            chart.indicator_pts_buf.clear();
            for i in (vs as u32)..end { if let Some(&v)=vals.get(i as usize) { if !v.is_nan() { chart.indicator_pts_buf.push(egui::pos2(bx(i as f32),py(v))); }}}
            if chart.indicator_pts_buf.len()>1 { painter.add(egui::Shape::line(chart.indicator_pts_buf.clone(),egui::Stroke::new(1.2,*color))); }
        } }

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
                    painter.text(egui::pos2(rect.left()+cw+4.0,pos.y),egui::Align2::LEFT_CENTER,format!("{:.1$}",hp,d),egui::FontId::monospace(10.0),egui::Color32::WHITE);
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

        let pos_to_bar = |pos: egui::Pos2| -> f32 { (pos.x - rect.left() + off - bs*0.5) / bs + vs };
        let pos_to_price = |pos: egui::Pos2| -> f32 { min_p + (max_p-min_p) * (1.0 - (pos.y - rect.top() - pt) / ch) };

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
        // Show move/grab cursor when hovering over a drawing (only in this pane)
        if pointer_in_pane && chart.draw_tool.is_empty() {
            if let Some((_, ep)) = &hover_hit {
                ui.ctx().set_cursor_icon(if *ep >= 0 { egui::CursorIcon::Grab } else { egui::CursorIcon::Move });
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
                match chart.draw_tool.as_str() {
                    "hline" => {
                        chart.next_draw_id+=1;
                        let mut d=Drawing::new(format!("d{}",chart.next_draw_id),DrawingKind::HLine{price});
                        d.color=chart.draw_color.clone(); d.line_style=LineStyle::Dashed;
                        chart.drawings.push(d); chart.draw_tool.clear();
                    }
                    "trendline" => {
                        if let Some((b0,p0)) = chart.pending_pt {
                            chart.next_draw_id+=1;
                            let mut d=Drawing::new(format!("d{}",chart.next_draw_id),DrawingKind::TrendLine{price0:p0,bar0:b0,price1:price,bar1:bar});
                            d.color=chart.draw_color.clone();
                            chart.drawings.push(d); chart.pending_pt=None; chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "hzone" => {
                        if let Some((_b0,p0)) = chart.pending_pt {
                            chart.next_draw_id+=1;
                            let mut d=Drawing::new(format!("d{}",chart.next_draw_id),DrawingKind::HZone{price0:p0,price1:price});
                            d.color=chart.draw_color.clone();
                            chart.drawings.push(d); chart.pending_pt=None; chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "barmarker" => {
                        // Snap to nearest bar, determine up/down based on click vs bar midpoint
                        let bar_idx = bar.round() as usize;
                        if let Some(b) = chart.bars.get(bar_idx) {
                            let mid = (b.open + b.close) / 2.0;
                            let up = price > mid;
                            let snap_price = if up { b.high } else { b.low };
                            chart.next_draw_id+=1;
                            let mut d=Drawing::new(format!("d{}",chart.next_draw_id),DrawingKind::BarMarker{bar:bar_idx as f32,price:snap_price,up});
                            d.color=chart.draw_color.clone();
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

        // Drag: pan chart OR move drawing
        if chart.draw_tool.is_empty() && resp.drag_started_by(egui::PointerButton::Primary) && !ctx.is_pointer_over_area() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some((id, ep)) = hit_at(pos.x, pos.y, &chart.drawings) {
                    chart.dragging_drawing = Some((id, ep));
                    chart.drag_start_price = pos_to_price(pos);
                    chart.drag_start_bar = pos_to_bar(pos);
                }
            }
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
            if resp.drag_stopped() { chart.dragging_drawing = None; }
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

        // X-axis drag (bottom strip) — horizontal zoom
        let xaxis_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top()+pt+ch), egui::vec2(cw, pb));
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
            ui.label(egui::RichText::new("DRAWING TOOLS").small().color(t.dim));
            if ui.button("Draw HLine").clicked() { chart.draw_tool="hline".into(); chart.pending_pt=None; ui.close_menu(); }
            if ui.button("Draw Trendline").clicked() { chart.draw_tool="trendline".into(); chart.pending_pt=None; ui.close_menu(); }
            if ui.button("Draw Zone").clicked() { chart.draw_tool="hzone".into(); chart.pending_pt=None; ui.close_menu(); }
            if ui.button("Place Marker").clicked() { chart.draw_tool="barmarker".into(); chart.pending_pt=None; ui.close_menu(); }
            ui.separator();
            if ui.button(format!("{} Drag Zoom", Icon::MAGNIFYING_GLASS_PLUS)).clicked() { chart.zoom_selecting=true; chart.zoom_start=egui::Pos2::ZERO; ui.close_menu(); }
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
            }
            ui.separator();
            // Groups
            if !chart.groups.is_empty() {
                ui.label(egui::RichText::new("GROUPS").small().color(t.dim));
                for g in &chart.groups {
                    let hidden = chart.hidden_groups.contains(&g.id);
                    let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                    let vis_icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                    let label = format!("{} {} ({})", vis_icon, g.name, count);
                    if ui.button(&label).clicked() {
                        if hidden { chart.hidden_groups.retain(|x| x != &g.id); }
                        else { chart.hidden_groups.push(g.id.clone()); }
                    }
                }
                ui.separator();
            }
            // Delete
            if !chart.selected_ids.is_empty() {
                if ui.button(egui::RichText::new(format!("{} Delete Selected", Icon::TRASH)).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                    let ids = chart.selected_ids.clone();
                    chart.drawings.retain(|d| !ids.contains(&d.id));
                    chart.selected_ids.clear(); chart.selected_id=None; ui.close_menu();
                }
            }
            if !chart.drawings.is_empty() {
                if ui.button(egui::RichText::new(format!("{} Delete All Drawings", Icon::TRASH)).color(egui::Color32::from_rgb(224,85,96))).clicked() {
                    chart.drawings.clear(); chart.selected_ids.clear(); chart.selected_id=None; ui.close_menu();
                }
            }
        });

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { chart.draw_tool.clear(); chart.pending_pt = None; chart.selected_id = None; }

        span_end(); // interaction
        } // end for pane_idx
    });
    span_end(); // chart_panes
    ctx.request_repaint();
}

// ─── winit + egui integration ─────────────────────────────────────────────────

/// A single native chart window with its own GPU context, panes, and layout.
struct ChartWindow {
    id: winit::window::WindowId,
    win: Arc<Window>,
    gpu: GpuCtx,
    rx: mpsc::Receiver<ChartCommand>,
    panes: Vec<Chart>,
    active_pane: usize,
    layout: Layout,
    last_redraw: std::time::Instant,
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
        // Prefer Mailbox (non-blocking, drops stale frames) over Fifo (vsync-blocking).
        // Fifo was causing 5.9ms avg acquire stalls (65% of frame time).
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            eprintln!("[native-chart] PresentMode::Mailbox (non-blocking)");
            wgpu::PresentMode::Mailbox
        } else {
            eprintln!("[native-chart] PresentMode::Fifo (Mailbox unavailable)");
            wgpu::PresentMode::Fifo
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format: fmt,
            width: size.width.max(1), height: size.height.max(1),
            present_mode, alpha_mode: caps.alpha_modes[0],
            view_formats: vec![], desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
        ui_kit::icons::init_icons(&egui_ctx);
        let egui_state = egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, &*window, Some(window.scale_factor() as f32), None, None);
        let egui_renderer = egui_wgpu::Renderer::new(&device, fmt, None, 1, false);

        Some(Self { device, queue, surface, config, egui_ctx, egui_state, egui_renderer })
    }

    fn render(&mut self, window: &Window, panes: &mut Vec<Chart>, active_pane: &mut usize, layout: &mut Layout, rx: &mpsc::Receiver<ChartCommand>) {
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
        let full_output = self.egui_ctx.run(raw_input, |ctx| { draw_chart(ctx, panes, active_pane, layout, rx); });
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
            .with_title("Apex Chart — Native GPU")
            .with_inner_size(PhysicalSize::new(self.iw, self.ih))
            .with_active(true))
        {
            Ok(w) => Arc::new(w),
            Err(e) => { eprintln!("[native-chart] Window creation failed: {e}"); return; }
        };
        let gpu = match GpuCtx::new(Arc::clone(&w)) {
            Some(g) => g,
            None => { eprintln!("[native-chart] GPU init failed"); return; }
        };
        let id = w.id();
        let (panes, layout) = load_state();
        let mut cw = ChartWindow { id, win: w, gpu, rx, panes, active_pane: 0, layout, last_redraw: std::time::Instant::now() };
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
                // Don't exit the event loop — other windows may be open
            }
            WindowEvent::Resized(s) => { if s.width>0&&s.height>0 { cw.gpu.config.width=s.width; cw.gpu.config.height=s.height; cw.gpu.surface.configure(&cw.gpu.device, &cw.gpu.config); } }
            WindowEvent::RedrawRequested => { cw.gpu.render(&cw.win, &mut cw.panes, &mut cw.active_pane, &mut cw.layout, &cw.rx); }
            _ => { cw.win.request_redraw(); }
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // Check for new window spawn requests
        while let Ok(req) = self.spawn_rx.try_recv() {
            self.spawn_window(el, req.rx, Some(req.initial_cmd));
        }

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

            // Cap at ~60fps per window
            let now = std::time::Instant::now();
            let target = cw.last_redraw + std::time::Duration::from_millis(16);
            if now >= target {
                cw.win.request_redraw();
                cw.last_redraw = now;
            }
        }

        el.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16)
        ));
    }
}

/// Fetch bars from Redis cache → OCOCO → yfinance sidecar → Yahoo Finance v8 on a background thread.
/// Sends LoadBars command via the global NATIVE_CHART_TXS channels (all windows).
/// Results are cached in Redis for subsequent requests.
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
            app_handle: handle, iw: 1400, ih: 900,
            windows: Vec::new(), spawn_rx,
        };
        let _ = el.run_app(&mut app);
        // All windows closed — clear the spawn sender so next call restarts
        if let Some(lock) = SPAWN_TX.get() {
            *lock.lock().unwrap() = None;
        }
    });
}

