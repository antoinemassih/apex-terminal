//! Native GPU chart renderer using eframe (egui + wgpu).
//!
//! eframe handles: window, event loop, input, wgpu setup, egui rendering.
//! We add: custom GPU chart shaders via egui::PaintCallback.

use std::sync::mpsc;
use eframe::egui;

use super::{Bar, ChartCommand, Drawing, DrawingKind};

// ─── Theme ────────────────────────────────────────────────────────────────────

struct Theme {
    name: &'static str,
    bg: egui::Color32,
    bull: egui::Color32,
    bear: egui::Color32,
    dim: egui::Color32,
}

const fn c(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

const THEMES: &[Theme] = &[
    Theme { name: "Midnight",       bg: c(13,13,13),   bull: c(46,204,113),  bear: c(231,76,60),   dim: c(102,102,102) },
    Theme { name: "Nord",           bg: c(46,52,64),   bull: c(163,190,140), bear: c(191,97,106),  dim: c(129,161,193) },
    Theme { name: "Monokai",        bg: c(39,40,34),   bull: c(166,226,46),  bear: c(249,38,114),  dim: c(165,159,133) },
    Theme { name: "Solarized",      bg: c(0,43,54),    bull: c(133,153,0),   bear: c(220,50,47),   dim: c(131,148,150) },
    Theme { name: "Dracula",        bg: c(40,42,54),   bull: c(80,250,123),  bear: c(255,85,85),   dim: c(189,147,249) },
    Theme { name: "Gruvbox",        bg: c(40,40,40),   bull: c(184,187,38),  bear: c(251,73,52),   dim: c(213,196,161) },
    Theme { name: "Catppuccin",     bg: c(30,30,46),   bull: c(166,227,161), bear: c(243,139,168), dim: c(180,190,254) },
    Theme { name: "Tokyo Night",    bg: c(26,27,38),   bull: c(158,206,106), bear: c(247,118,142), dim: c(122,162,247) },
];

// ─��─ Chart state ──────────────────────────────────────────────────────────────

fn compute_sma(data: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; data.len()];
    if data.len() < period { return r; }
    let mut sum: f32 = data[..period].iter().sum();
    r[period - 1] = sum / period as f32;
    for i in period..data.len() { sum += data[i] - data[i - period]; r[i] = sum / period as f32; }
    r
}

fn compute_ema(data: &[f32], period: usize) -> Vec<f32> {
    let mut r = vec![f32::NAN; data.len()];
    if data.len() < period { return r; }
    let k = 2.0 / (period as f32 + 1.0);
    let sma: f32 = data[..period].iter().sum::<f32>() / period as f32;
    r[period - 1] = sma;
    let mut prev = sma;
    for i in period..data.len() { let v = data[i] * k + prev * (1.0 - k); r[i] = v; prev = v; }
    r
}

struct ChartApp {
    rx: mpsc::Receiver<ChartCommand>,
    bars: Vec<Bar>,
    timestamps: Vec<i64>,
    drawings: Vec<Drawing>,
    indicators: Vec<(Vec<f32>, egui::Color32, String)>, // (values, color, name)

    // Viewport
    view_start: f32,
    view_count: u32,
    price_lock: Option<(f32, f32)>,

    // Interaction
    dragging: bool,
    drag_start: egui::Pos2,
    auto_scroll: bool,
    last_interaction: std::time::Instant,

    // UI state
    theme_idx: usize,
    draw_tool: String, // "", "hline", "trendline"
    pending_point: Option<(f32, f32)>,
}

impl ChartApp {
    fn new(rx: mpsc::Receiver<ChartCommand>) -> Self {
        Self {
            rx, bars: Vec::new(), timestamps: Vec::new(), drawings: Vec::new(), indicators: Vec::new(),
            view_start: 0.0, view_count: 200, price_lock: None,
            dragging: false, drag_start: egui::Pos2::ZERO, auto_scroll: true,
            last_interaction: std::time::Instant::now(),
            theme_idx: 0, draw_tool: String::new(), pending_point: None,
        }
    }

    fn process_commands(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                ChartCommand::LoadBars { bars, timestamps, .. } => {
                    self.bars = bars;
                    self.timestamps = timestamps;
                    self.view_start = (self.bars.len() as f32 - self.view_count as f32 + 8.0).max(0.0);
                    self.compute_indicators();
                }
                ChartCommand::AppendBar { bar, timestamp, .. } => {
                    self.bars.push(bar);
                    self.timestamps.push(timestamp);
                    if self.auto_scroll {
                        self.view_start = (self.bars.len() as f32 - self.view_count as f32 + 8.0).max(0.0);
                    }
                }
                ChartCommand::UpdateLastBar { bar, .. } => {
                    if let Some(last) = self.bars.last_mut() { *last = bar; }
                }
                ChartCommand::SetTheme { .. } => {}
                ChartCommand::SetDrawing(d) => { self.drawings.retain(|x| x.id != d.id); self.drawings.push(d); }
                ChartCommand::RemoveDrawing { id } => { self.drawings.retain(|x| x.id != id); }
                ChartCommand::ClearDrawings => { self.drawings.clear(); }
                _ => {}
            }
        }
    }

    fn compute_indicators(&mut self) {
        self.indicators.clear();
        let closes: Vec<f32> = self.bars.iter().map(|b| b.close).collect();
        if closes.len() >= 20 {
            self.indicators.push((compute_sma(&closes, 20), egui::Color32::from_rgba_unmultiplied(0, 190, 240, 200), "SMA20".into()));
        }
        if closes.len() >= 50 {
            self.indicators.push((compute_sma(&closes, 50), egui::Color32::from_rgba_unmultiplied(240, 150, 25, 180), "SMA50".into()));
        }
        self.indicators.push((compute_ema(&closes, 12), egui::Color32::from_rgba_unmultiplied(240, 215, 50, 170), "EMA12".into()));
        self.indicators.push((compute_ema(&closes, 26), egui::Color32::from_rgba_unmultiplied(178, 102, 230, 170), "EMA26".into()));
    }

    fn price_range(&self) -> (f32, f32) {
        if let Some(r) = self.price_lock { return r; }
        let s = self.view_start as u32;
        let e = (s + self.view_count).min(self.bars.len() as u32);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in s..e { if let Some(b) = self.bars.get(i as usize) { lo = lo.min(b.low); hi = hi.max(b.high); } }
        if lo >= hi { lo -= 0.5; hi += 0.5; }
        let p = (hi - lo) * 0.05;
        (lo - p, hi + p)
    }
}

impl eframe::App for ChartApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_commands();

        // Auto-scroll resume
        if !self.auto_scroll && self.last_interaction.elapsed().as_secs() >= 5 {
            self.auto_scroll = true;
            self.price_lock = None;
            self.view_start = (self.bars.len() as f32 - self.view_count as f32 + 8.0).max(0.0);
        }

        let t = &THEMES[self.theme_idx];
        let n_bars = self.bars.len();

        // Top panel — toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Apex Chart").strong().color(t.bull));
                ui.separator();

                // OHLC
                if let Some(bar) = self.bars.last() {
                    let c = if bar.close >= bar.open { t.bull } else { t.bear };
                    ui.label(egui::RichText::new(format!("O {:.2}  H {:.2}  L {:.2}  C {:.2}  V {:.0}", bar.open, bar.high, bar.low, bar.close, bar.volume))
                        .monospace().color(c));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Theme picker
                    egui::ComboBox::from_id_salt("theme").selected_text(t.name).width(100.0).show_ui(ui, |ui| {
                        for (i, theme) in THEMES.iter().enumerate() {
                            ui.selectable_value(&mut self.theme_idx, i, theme.name);
                        }
                    });

                    // Drawing tools
                    if ui.selectable_label(self.draw_tool == "hline", "HLine").clicked() {
                        self.draw_tool = if self.draw_tool == "hline" { String::new() } else { "hline".into() };
                        self.pending_point = None;
                    }
                    if ui.selectable_label(self.draw_tool == "trendline", "Trend").clicked() {
                        self.draw_tool = if self.draw_tool == "trendline" { String::new() } else { "trendline".into() };
                        self.pending_point = None;
                    }

                    // Auto-scroll
                    if !self.auto_scroll {
                        if ui.button("▶ LIVE").clicked() {
                            self.auto_scroll = true;
                            self.price_lock = None;
                            self.view_start = (n_bars as f32 - self.view_count as f32 + 8.0).max(0.0);
                        }
                    } else {
                        ui.label(egui::RichText::new("● LIVE").color(t.bull).small());
                    }
                });
            });
        });

        // Status bar
        if !self.draw_tool.is_empty() {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                let hint = match self.draw_tool.as_str() {
                    "hline" => "Click chart to place horizontal line. Esc to cancel.",
                    "trendline" if self.pending_point.is_some() => "Click second point. Esc to cancel.",
                    "trendline" => "Click first point. Esc to cancel.",
                    _ => "",
                };
                ui.label(egui::RichText::new(hint).color(egui::Color32::from_rgb(255, 200, 50)));
            });
        }

        // Central panel — chart
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.bg)).show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let w = rect.width();
            let h = rect.height();
            let pr = 80.0_f32; // right padding for price axis
            let pt = 4.0_f32;
            let pb = 24.0_f32;
            let cw = w - pr;
            let ch = h - pt - pb;

            if n_bars == 0 || cw <= 0.0 || ch <= 0.0 { return; }

            let (min_p, max_p) = self.price_range();
            let total_bars = self.view_count + 8;
            let bar_step = cw / total_bars as f32;
            let vs = self.view_start;
            let end = ((vs as u32) + self.view_count).min(n_bars as u32);
            let frac = vs - vs.floor();
            let offset = frac * bar_step;

            let price_to_y = |p: f32| -> f32 { rect.top() + pt + (max_p - p) / (max_p - min_p) * ch };
            let bar_to_x = |idx: f32| -> f32 { rect.left() + (idx - vs) * bar_step + bar_step * 0.5 - offset };

            let painter = ui.painter_at(rect);

            // Grid lines
            let range = max_p - min_p;
            let raw_step = range / 8.0;
            let mag = 10.0_f32.powf(raw_step.log10().floor());
            let nice = [1.0, 2.0, 2.5, 5.0, 10.0];
            let price_step = nice.iter().map(|&s| s * mag).find(|&s| s >= raw_step).unwrap_or(raw_step);
            let mut p = (min_p / price_step).ceil() * price_step;
            while p <= max_p {
                let y = price_to_y(p);
                painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)],
                    egui::Stroke::new(0.5, t.dim.gamma_multiply(0.3)));
                // Price label
                let dec = if p >= 10.0 { 2 } else { 4 };
                painter.text(egui::pos2(rect.left() + cw + 4.0, y), egui::Align2::LEFT_CENTER,
                    format!("{:.1$}", p, dec), egui::FontId::monospace(10.0), t.dim);
                p += price_step;
            }

            // Volume bars
            let mut max_vol: f32 = 0.0;
            for i in (vs as u32)..end { if let Some(b) = self.bars.get(i as usize) { max_vol = max_vol.max(b.volume); } }
            if max_vol == 0.0 { max_vol = 1.0; }
            for i in (vs as u32)..end {
                if let Some(bar) = self.bars.get(i as usize) {
                    let x = bar_to_x(i as f32);
                    let vol_h = (bar.volume / max_vol) * ch * 0.2;
                    let color = if bar.close >= bar.open { t.bull.gamma_multiply(0.2) } else { t.bear.gamma_multiply(0.2) };
                    let bw = (bar_step * 0.4).max(1.0);
                    painter.rect_filled(egui::Rect::from_min_max(
                        egui::pos2(x - bw, rect.top() + pt + ch - vol_h),
                        egui::pos2(x + bw, rect.top() + pt + ch),
                    ), 0.0, color);
                }
            }

            // Candlesticks
            for i in (vs as u32)..end {
                if let Some(bar) = self.bars.get(i as usize) {
                    let x = bar_to_x(i as f32);
                    let is_bull = bar.close >= bar.open;
                    let color = if is_bull { t.bull } else { t.bear };
                    let body_top = price_to_y(bar.open.max(bar.close));
                    let body_bot = price_to_y(bar.open.min(bar.close));
                    let wick_top = price_to_y(bar.high);
                    let wick_bot = price_to_y(bar.low);
                    let bw = (bar_step * 0.35).max(1.0);

                    // Wick
                    painter.line_segment([egui::pos2(x, wick_top), egui::pos2(x, wick_bot)], egui::Stroke::new(1.0, color));
                    // Body
                    let body_h = (body_bot - body_top).max(1.0);
                    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(x - bw, body_top), egui::vec2(bw * 2.0, body_h)), 0.0, color);
                }
            }

            // Indicator lines
            for (values, color, _name) in &self.indicators {
                let mut points = Vec::new();
                for i in (vs as u32)..end {
                    if let Some(&v) = values.get(i as usize) {
                        if !v.is_nan() {
                            points.push(egui::pos2(bar_to_x(i as f32), price_to_y(v)));
                        }
                    }
                }
                if points.len() > 1 {
                    painter.add(egui::Shape::line(points, egui::Stroke::new(1.2, *color)));
                }
            }

            // Drawings
            for d in &self.drawings {
                let stroke = egui::Stroke::new(d.width, egui::Color32::from_rgba_unmultiplied(
                    (d.color[0]*255.0) as u8, (d.color[1]*255.0) as u8, (d.color[2]*255.0) as u8, (d.color[3]*255.0) as u8));
                match &d.kind {
                    DrawingKind::HLine { price } => {
                        let y = price_to_y(*price);
                        painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)], stroke);
                    }
                    DrawingKind::TrendLine { price0, bar0, price1, bar1 } => {
                        painter.line_segment([egui::pos2(bar_to_x(*bar0), price_to_y(*price0)), egui::pos2(bar_to_x(*bar1), price_to_y(*price1))], stroke);
                    }
                    DrawingKind::HZone { price0, price1 } => {
                        let y0 = price_to_y(*price0); let y1 = price_to_y(*price1);
                        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(), y0.min(y1)), egui::pos2(rect.left()+cw, y0.max(y1))),
                            0.0, egui::Color32::from_rgba_unmultiplied((d.color[0]*255.0) as u8, (d.color[1]*255.0) as u8, (d.color[2]*255.0) as u8, 30));
                    }
                }
            }

            // Crosshair
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                if pos.x >= rect.left() && pos.x < rect.left() + cw && pos.y >= rect.top() + pt && pos.y < rect.top() + pt + ch {
                    // Dashed crosshair lines
                    painter.line_segment([egui::pos2(rect.left(), pos.y), egui::pos2(rect.left() + cw, pos.y)],
                        egui::Stroke::new(0.5, egui::Color32::from_white_alpha(50)));
                    painter.line_segment([egui::pos2(pos.x, rect.top() + pt), egui::pos2(pos.x, rect.top() + pt + ch)],
                        egui::Stroke::new(0.5, egui::Color32::from_white_alpha(50)));
                    // Price label
                    let hover_price = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
                    let dec = if hover_price >= 10.0 { 2 } else { 4 };
                    painter.text(egui::pos2(rect.left() + cw + 4.0, pos.y), egui::Align2::LEFT_CENTER,
                        format!("{:.1$}", hover_price, dec), egui::FontId::monospace(10.0), egui::Color32::WHITE);
                }
            }

            // Chart interaction
            let response = ui.allocate_rect(egui::Rect::from_min_size(rect.min, egui::vec2(cw, h)), egui::Sense::click_and_drag());

            // Pan
            if response.dragged_by(egui::PointerButton::Primary) && self.draw_tool.is_empty() {
                let delta = response.drag_delta();
                let bar_delta = delta.x / bar_step;
                self.view_start = (self.view_start - bar_delta).max(0.0).min((n_bars as f32) + 200.0);
                self.auto_scroll = false;
                self.last_interaction = std::time::Instant::now();
            }

            // Scroll zoom
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 && response.hovered() {
                let factor = if scroll > 0.0 { 0.9 } else { 1.1 };
                let old = self.view_count;
                let new = ((old as f32 * factor).round() as u32).max(20).min(n_bars as u32);
                let d = (old as i32 - new as i32) / 2;
                self.view_count = new;
                self.view_start = (self.view_start + d as f32).max(0.0);
                self.auto_scroll = false;
                self.last_interaction = std::time::Instant::now();
            }

            // Drawing tool clicks
            if response.clicked() && !self.draw_tool.is_empty() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let bar = (pos.x - rect.left() + offset - bar_step * 0.5) / bar_step + vs;
                    let price = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
                    match self.draw_tool.as_str() {
                        "hline" => {
                            self.drawings.push(Drawing { id: format!("h{}", self.drawings.len()), kind: DrawingKind::HLine { price }, color: [0.4, 0.7, 1.0, 0.8], width: 1.0, dashed: true });
                            self.draw_tool.clear();
                        }
                        "trendline" => {
                            if let Some((b0, p0)) = self.pending_point {
                                self.drawings.push(Drawing { id: format!("t{}", self.drawings.len()), kind: DrawingKind::TrendLine { price0: p0, bar0: b0, price1: price, bar1: bar }, color: [0.3, 0.6, 1.0, 0.9], width: 1.0, dashed: false });
                                self.pending_point = None;
                                self.draw_tool.clear();
                            } else {
                                self.pending_point = Some((bar, price));
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Context menu
            response.context_menu(|ui| {
                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let price = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
                    if ui.button("Set HLine").clicked() {
                        self.drawings.push(Drawing { id: format!("h{}", self.drawings.len()), kind: DrawingKind::HLine { price }, color: [0.4, 0.7, 1.0, 0.8], width: 1.0, dashed: true });
                        ui.close_menu();
                    }
                }
                if ui.button("Draw Trendline").clicked() {
                    self.draw_tool = "trendline".into();
                    self.pending_point = None;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Reset View").clicked() {
                    self.auto_scroll = true;
                    self.price_lock = None;
                    self.view_start = (n_bars as f32 - self.view_count as f32 + 8.0).max(0.0);
                    ui.close_menu();
                }
                if ui.button("Clear Drawings").clicked() {
                    self.drawings.clear();
                    ui.close_menu();
                }
            });

            // Escape cancels tool
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.draw_tool.clear();
                self.pending_point = None;
            }
        });

        // Always repaint for smooth crosshair + live data
        ctx.request_repaint();
    }
}

pub fn run_render_loop(title: &str, width: u32, height: u32, rx: mpsc::Receiver<ChartCommand>) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([width as f32, height as f32]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    let _ = eframe::run_native(title, options, Box::new(|_cc| {
        Ok(Box::new(ChartApp::new(rx)))
    }));
}
