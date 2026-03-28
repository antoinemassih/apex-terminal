//! Native GPU chart renderer — winit (any_thread) + egui for all rendering.
//! egui handles UI + chart painting. winit handles window on non-main thread.

use std::sync::{mpsc, Arc};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
    dpi::PhysicalSize,
};

use super::{Bar, ChartCommand, Drawing, DrawingKind};

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

// ─── Chart state ──────────────────────────────────────────────────────────────

struct Chart {
    bars: Vec<Bar>, timestamps: Vec<i64>, drawings: Vec<Drawing>,
    indicators: Vec<(Vec<f32>, egui::Color32, String)>,
    vs: f32, vc: u32, price_lock: Option<(f32,f32)>,
    auto_scroll: bool, last_input: std::time::Instant,
    theme_idx: usize, draw_tool: String, pending_pt: Option<(f32,f32)>,
    selected_id: Option<String>,
    dragging_drawing: Option<(String, i32)>, // (id, endpoint: -1=whole, 0=first, 1=second)
    drag_start_price: f32, drag_start_bar: f32,
}

impl Chart {
    fn new() -> Self {
        Self { bars: vec![], timestamps: vec![], drawings: vec![], indicators: vec![],
            vs: 0.0, vc: 200, price_lock: None, auto_scroll: true,
            last_input: std::time::Instant::now(), theme_idx: 0,
            draw_tool: String::new(), pending_pt: None,
            selected_id: None, dragging_drawing: None, drag_start_price: 0.0, drag_start_bar: 0.0 }
    }
    fn process(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, timestamps, .. } => {
                self.bars = bars; self.timestamps = timestamps;
                self.vs = (self.bars.len() as f32 - self.vc as f32 + 8.0).max(0.0);
                let closes: Vec<f32> = self.bars.iter().map(|b| b.close).collect();
                self.indicators.clear();
                if closes.len()>=20 { self.indicators.push((compute_sma(&closes,20), egui::Color32::from_rgba_unmultiplied(0,190,240,200), "SMA20".into())); }
                if closes.len()>=50 { self.indicators.push((compute_sma(&closes,50), egui::Color32::from_rgba_unmultiplied(240,150,25,180), "SMA50".into())); }
                self.indicators.push((compute_ema(&closes,12), egui::Color32::from_rgba_unmultiplied(240,215,50,170), "EMA12".into()));
                self.indicators.push((compute_ema(&closes,26), egui::Color32::from_rgba_unmultiplied(178,102,230,170), "EMA26".into()));
            }
            ChartCommand::AppendBar { bar, timestamp, .. } => {
                self.bars.push(bar); self.timestamps.push(timestamp);
                if self.auto_scroll { self.vs = (self.bars.len() as f32 - self.vc as f32 + 8.0).max(0.0); }
            }
            ChartCommand::UpdateLastBar { bar, .. } => { if let Some(l) = self.bars.last_mut() { *l = bar; } }
            ChartCommand::SetDrawing(d) => { self.drawings.retain(|x| x.id != d.id); self.drawings.push(d); }
            ChartCommand::RemoveDrawing { id } => { self.drawings.retain(|x| x.id != id); }
            ChartCommand::ClearDrawings => { self.drawings.clear(); }
            _ => {}
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

// ──�� egui rendering ───────────────────────────────────────────────────────────

fn draw_chart(ctx: &egui::Context, chart: &mut Chart, rx: &mpsc::Receiver<ChartCommand>) {
    while let Ok(cmd) = rx.try_recv() { chart.process(cmd); }
    if !chart.auto_scroll && chart.last_input.elapsed().as_secs() >= 5 {
        chart.auto_scroll = true; chart.price_lock = None;
        chart.vs = (chart.bars.len() as f32 - chart.vc as f32 + 8.0).max(0.0);
    }
    let t = &THEMES[chart.theme_idx];
    let n = chart.bars.len();

    egui::TopBottomPanel::top("tb").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Apex Chart").strong().color(t.bull));
            ui.separator();
            if let Some(b) = chart.bars.last() {
                let c = if b.close>=b.open { t.bull } else { t.bear };
                ui.label(egui::RichText::new(format!("O{:.2} H{:.2} L{:.2} C{:.2} V{:.0}",b.open,b.high,b.low,b.close,b.volume)).monospace().size(11.0).color(c));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("thm").selected_text(t.name).width(100.0).show_ui(ui, |ui| {
                    for (i,th) in THEMES.iter().enumerate() { ui.selectable_value(&mut chart.theme_idx, i, th.name); }
                });
                if ui.selectable_label(chart.draw_tool=="hline","HLine").clicked() {
                    chart.draw_tool = if chart.draw_tool=="hline" { String::new() } else { "hline".into() }; chart.pending_pt=None;
                }
                if ui.selectable_label(chart.draw_tool=="trendline","Trend").clicked() {
                    chart.draw_tool = if chart.draw_tool=="trendline" { String::new() } else { "trendline".into() }; chart.pending_pt=None;
                }
                if !chart.auto_scroll { if ui.button("▶ LIVE").clicked() { chart.auto_scroll=true; chart.price_lock=None; chart.vs=(n as f32-chart.vc as f32+8.0).max(0.0); } }
                else { ui.label(egui::RichText::new("● LIVE").color(t.bull).small()); }
            });
        });
    });
    if !chart.draw_tool.is_empty() {
        egui::TopBottomPanel::bottom("st").show(ctx, |ui| {
            let h = match chart.draw_tool.as_str() { "hline"=>"Click to place HLine (Esc cancel)", "trendline" if chart.pending_pt.is_some()=>"Click 2nd point (Esc cancel)", "trendline"=>"Click 1st point (Esc cancel)", _=>"" };
            ui.label(egui::RichText::new(h).color(egui::Color32::from_rgb(255,200,50)));
        });
    }

    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.bg)).show(ctx, |ui| {
        let rect = ui.available_rect_before_wrap();
        let (w,h) = (rect.width(), rect.height());
        let (pr,pt,pb) = (80.0_f32, 4.0_f32, 24.0_f32);
        let (cw,ch) = (w-pr, h-pt-pb);
        if n==0 || cw<=0.0 || ch<=0.0 { return; }

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

        // Volume
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
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(x-bw,bt),egui::vec2(bw*2.0,(bb-bt).max(1.0))),0.0,c);
        }}

        // Indicators
        for (vals,color,_) in &chart.indicators {
            let mut pts=vec![];
            for i in (vs as u32)..end { if let Some(&v)=vals.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32),py(v))); }}}
            if pts.len()>1 { painter.add(egui::Shape::line(pts,egui::Stroke::new(1.2,*color))); }
        }

        // Drawings (with selection highlight + endpoint handles)
        for d in &chart.drawings {
            let is_sel = chart.selected_id.as_deref() == Some(&d.id);
            let dc = egui::Color32::from_rgba_unmultiplied((d.color[0]*255.0)as u8,(d.color[1]*255.0)as u8,(d.color[2]*255.0)as u8,(d.color[3]*255.0)as u8);
            let sc = egui::Stroke::new(if is_sel { d.width + 1.0 } else { d.width }, if is_sel { egui::Color32::WHITE } else { dc });
            match &d.kind {
                DrawingKind::HLine{price}=>{
                    let y=py(*price);
                    painter.line_segment([egui::pos2(rect.left(),y),egui::pos2(rect.left()+cw,y)],sc);
                    if is_sel {
                        painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y), 4.0, egui::Color32::from_rgb(74,158,255));
                    }
                }
                DrawingKind::TrendLine{price0,bar0,price1,bar1}=>{
                    let p0=egui::pos2(bx(*bar0),py(*price0)); let p1=egui::pos2(bx(*bar1),py(*price1));
                    painter.line_segment([p0,p1],sc);
                    if is_sel {
                        painter.circle_filled(p0, 5.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_stroke(p0, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                        painter.circle_filled(p1, 5.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_stroke(p1, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                    }
                }
                DrawingKind::HZone{price0,price1}=>{
                    let(y0,y1)=(py(*price0),py(*price1));
                    painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(),y0.min(y1)),egui::pos2(rect.left()+cw,y0.max(y1))),0.0,
                        egui::Color32::from_rgba_unmultiplied((d.color[0]*255.0)as u8,(d.color[1]*255.0)as u8,(d.color[2]*255.0)as u8,30));
                    painter.line_segment([egui::pos2(rect.left(),y0),egui::pos2(rect.left()+cw,y0)],sc);
                    painter.line_segment([egui::pos2(rect.left(),y1),egui::pos2(rect.left()+cw,y1)],sc);
                }
            }
        }

        // Drawing preview
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            if chart.draw_tool == "trendline" {
                if let Some((b0, p0)) = chart.pending_pt {
                    // Dashed preview line
                    let start = egui::pos2(bx(b0), py(p0));
                    let dir = pos - start;
                    let len = dir.length();
                    if len > 2.0 {
                        let dash_len = 6.0;
                        let gap_len = 4.0;
                        let step = dash_len + gap_len;
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
                // Crosshair cursor for drawing mode
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if chart.draw_tool == "hline" {
                // Show horizontal preview line at cursor price
                if pos.y >= rect.top()+pt && pos.y < rect.top()+pt+ch {
                    painter.line_segment(
                        [egui::pos2(rect.left(), pos.y), egui::pos2(rect.left()+cw, pos.y)],
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100,180,255,120)),
                    );
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }

        // Crosshair (only when not in drawing mode)
        if chart.draw_tool.is_empty() {
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

        // ── Interaction ───────────────────────────────────────────────────────
        let resp = ui.allocate_rect(egui::Rect::from_min_size(rect.min,egui::vec2(cw,h)), egui::Sense::click_and_drag());

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
                }
            }
            None
        });
        // Show move/grab cursor when hovering over a drawing
        if chart.draw_tool.is_empty() {
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
                        chart.drawings.push(Drawing{id:format!("h{}",chart.drawings.len()),kind:DrawingKind::HLine{price},color:[0.4,0.7,1.0,0.8],width:1.0,dashed:true});
                        chart.draw_tool.clear();
                    }
                    "trendline" => {
                        if let Some((b0,p0)) = chart.pending_pt {
                            chart.drawings.push(Drawing{id:format!("t{}",chart.drawings.len()),kind:DrawingKind::TrendLine{price0:p0,bar0:b0,price1:price,bar1:bar},color:[0.3,0.6,1.0,0.9],width:1.0,dashed:false});
                            chart.pending_pt = None;
                            chart.draw_tool.clear();
                        } else {
                            chart.pending_pt = Some((bar, price));
                        }
                    }
                    _ => {}
                }
            }
        }
        // No tool: click selects drawing, or deselects
        else if chart.draw_tool.is_empty() && resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some((id, _)) = hit_at(pos.x, pos.y, &chart.drawings) {
                    chart.selected_id = Some(id);
                } else {
                    chart.selected_id = None;
                }
            }
        }

        // Drag: pan chart OR move drawing
        if chart.draw_tool.is_empty() && resp.drag_started_by(egui::PointerButton::Primary) {
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

        // Delete selected drawing
        if chart.selected_id.is_some() && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            let id = chart.selected_id.take().unwrap();
            chart.drawings.retain(|d| d.id != id);
        }

        // Context menu
        resp.context_menu(|ui| {
            if ui.button("Draw HLine").clicked() {
                chart.draw_tool = "hline".into(); chart.pending_pt = None; ui.close_menu();
            }
            if ui.button("Draw Trendline").clicked() { chart.draw_tool = "trendline".into(); chart.pending_pt = None; ui.close_menu(); }
            ui.separator();
            if ui.button("Reset View").clicked() { chart.auto_scroll=true; chart.price_lock=None; chart.vs=(n as f32-chart.vc as f32+8.0).max(0.0); ui.close_menu(); }
            if chart.selected_id.is_some() {
                if ui.button("Delete Selected").clicked() {
                    let id = chart.selected_id.take().unwrap();
                    chart.drawings.retain(|d| d.id != id);
                    ui.close_menu();
                }
            }
            if !chart.drawings.is_empty() {
                if ui.button("Clear All Drawings").clicked() { chart.drawings.clear(); ui.close_menu(); }
            }
        });

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { chart.draw_tool.clear(); chart.pending_pt = None; chart.selected_id = None; }
    });
    ctx.request_repaint();
}

// ─── winit + egui integration ─────────────────────────────────────────────────

struct App {
    rx: mpsc::Receiver<ChartCommand>,
    title: String, iw: u32, ih: u32,
    win: Option<Arc<Window>>,
    gpu: Option<GpuCtx>,
    chart: Chart,
}

struct GpuCtx {
    device: wgpu::Device, queue: wgpu::Queue,
    surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl GpuCtx {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..Default::default() });
        let surface = instance.create_surface(Arc::clone(&window)).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), force_fallback_adapter: false,
        })).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("chart"), memory_hints: wgpu::MemoryHints::Performance, ..Default::default()
        }, None)).unwrap();
        let caps = surface.get_capabilities(&adapter);
        let fmt = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format: fmt,
            width: size.width.max(1), height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo, alpha_mode: caps.alpha_modes[0],
            view_formats: vec![], desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
        let egui_state = egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, &*window, Some(window.scale_factor() as f32), None, None);
        let egui_renderer = egui_wgpu::Renderer::new(&device, fmt, None, 1, false);

        Self { device, queue, surface, config, egui_ctx, egui_state, egui_renderer }
    }

    fn render(&mut self, window: &Window, chart: &mut Chart, rx: &mpsc::Receiver<ChartCommand>) {
        let output = match self.surface.get_current_texture() {
            Ok(t) => t, Err(_) => { self.surface.configure(&self.device, &self.config); return; }
        };
        let view = output.texture.create_view(&Default::default());

        let raw_input = self.egui_state.take_egui_input(window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| { draw_chart(ctx, chart, rx); });
        self.egui_state.handle_platform_output(window, full_output.platform_output);

        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [self.config.width, self.config.height], pixels_per_point: full_output.pixels_per_point };

        for (id, delta) in &full_output.textures_delta.set { self.egui_renderer.update_texture(&self.device, &self.queue, *id, delta); }

        let mut enc = self.device.create_command_encoder(&Default::default());
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut enc, &paint_jobs, &sd);
        self.queue.submit(std::iter::once(enc.finish()));

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

        for id in &full_output.textures_delta.free { self.egui_renderer.free_texture(id); }
        output.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.win.is_some() { return; }
        let w = Arc::new(el.create_window(WindowAttributes::default().with_title(&self.title).with_inner_size(PhysicalSize::new(self.iw, self.ih)).with_active(true)).unwrap());
        let gpu = GpuCtx::new(Arc::clone(&w));
        self.win = Some(w); self.gpu = Some(gpu);
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _: winit::window::WindowId, ev: WindowEvent) {
        let gpu = match &mut self.gpu { Some(g) => g, None => return };
        if let Some(win) = &self.win { let _ = gpu.egui_state.on_window_event(win, &ev); }
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => { if s.width>0&&s.height>0 { gpu.config.width=s.width; gpu.config.height=s.height; gpu.surface.configure(&gpu.device, &gpu.config); } }
            WindowEvent::RedrawRequested => { if let Some(win) = &self.win { gpu.render(win, &mut self.chart, &self.rx); } }
            _ => {}
        }
        if let Some(win) = &self.win { win.request_redraw(); }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) { if let Some(w) = &self.win { w.request_redraw(); } }
}

pub fn run_render_loop(title: &str, width: u32, height: u32, rx: mpsc::Receiver<ChartCommand>) {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    let el = EventLoop::builder().with_any_thread(true).build().unwrap();
    let mut app = App { rx, title: title.into(), iw: width, ih: height, win: None, gpu: None, chart: Chart::new() };
    let _ = el.run_app(&mut app);
}
