//! Native wgpu render loop — winit + GPU candles + volume + grid.

use std::sync::{mpsc, Arc};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId, WindowAttributes},
    dpi::PhysicalSize,
};

use super::{Bar, CandleUniforms, VolumeUniforms, LineUniforms, GridVertex, ChartCommand, Drawing, DrawingKind};

const CANDLE_SHADER: &str = include_str!("../../../src/renderer/shaders/candles_gpu.wgsl");
const VOLUME_SHADER: &str = include_str!("../../../src/renderer/shaders/volume_gpu.wgsl");
const GRID_SHADER: &str = include_str!("../../../src/renderer/shaders/grid.wgsl");
const OVERLAY_SHADER: &str = include_str!("../../../src/renderer/shaders/overlay.wgsl");
const LINE_SHADER: &str = include_str!("../../../src/renderer/shaders/line_gpu.wgsl");

const RIGHT_MARGIN_BARS: u32 = 8;
const PR: f32 = 80.0;
const PT: f32 = 20.0;
const PB: f32 = 40.0;
const MAX_GRID_VERTS: usize = 512;
const MAX_OVERLAY_LINES: usize = 128;
const OVERLAY_FLOATS_PER_LINE: usize = 12; // matches overlay.wgsl Line struct

fn compute_sma(data: &[f32], period: usize) -> Vec<f32> {
    let mut result = vec![f32::NAN; data.len()];
    if data.len() < period { return result; }
    let mut sum: f32 = data[..period].iter().sum();
    result[period - 1] = sum / period as f32;
    for i in period..data.len() {
        sum += data[i] - data[i - period];
        result[i] = sum / period as f32;
    }
    result
}

fn compute_ema(data: &[f32], period: usize) -> Vec<f32> {
    let mut result = vec![f32::NAN; data.len()];
    if data.len() < period { return result; }
    let k = 2.0 / (period as f32 + 1.0);
    let sma: f32 = data[..period].iter().sum::<f32>() / period as f32;
    result[period - 1] = sma;
    let mut prev = sma;
    for i in period..data.len() {
        let val = data[i] * k + prev * (1.0 - k);
        result[i] = val;
        prev = val;
    }
    result
}

// ─── Drawing tool state ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum DrawTool { None, HLine, TrendLine }

struct DrawState {
    tool: DrawTool,
    pending_point: Option<(f32, f32)>, // (bar_idx, price)
    // Drag-move or endpoint-edit
    dragging_id: Option<String>,
    drag_endpoint: i32, // -1 = whole drawing, 0 = first point, 1 = second point
    drag_start_price: f32,
    drag_start_bar: f32,
    // Zoom selection box
    zoom_selecting: bool,
    zoom_start: (f32, f32),
}

impl DrawState {
    fn new() -> Self {
        Self {
            tool: DrawTool::None, pending_point: None,
            dragging_id: None, drag_endpoint: -1, drag_start_price: 0.0, drag_start_bar: 0.0,
            zoom_selecting: false, zoom_start: (0.0, 0.0),
        }
    }
}

// ─── Theme presets ────────────────────────────────────────────────────────────

struct ThemePreset {
    name: &'static str,
    bg: [f32; 4],
    toolbar_bg: [f32; 4],
    toolbar_border: [f32; 4],
    bull: [f32; 4],
    bear: [f32; 4],
    grid: [f32; 4],
    axis_text: [f32; 4],
    ohlc_label: [f32; 4],
    accent: [f32; 4],
}

const fn hex(r: u8, g: u8, b: u8) -> [f32; 4] { [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0] }
const fn hexa(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] { [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a] }

const THEMES: &[ThemePreset] = &[
    ThemePreset { name: "Midnight",       bg: hex(13,13,13),   toolbar_bg: hex(17,17,17), toolbar_border: hex(34,34,34), bull: hex(46,204,113), bear: hex(231,76,60),  grid: hexa(38,38,38,0.4), axis_text: hexa(102,102,102,1.0), ohlc_label: hexa(204,204,204,1.0), accent: hex(42,100,150) },
    ThemePreset { name: "Nord",           bg: hex(46,52,64),   toolbar_bg: hex(46,52,64), toolbar_border: hex(59,66,82), bull: hex(163,190,140), bear: hex(191,97,106), grid: hexa(59,66,82,0.4), axis_text: hexa(129,161,193,1.0), ohlc_label: hexa(216,222,233,1.0), accent: hex(136,192,208) },
    ThemePreset { name: "Monokai",        bg: hex(39,40,34),   toolbar_bg: hex(30,31,28), toolbar_border: hex(62,61,50), bull: hex(166,226,46), bear: hex(249,38,114),  grid: hexa(62,61,50,0.4), axis_text: hexa(165,159,133,1.0), ohlc_label: hexa(248,248,242,1.0), accent: hex(230,219,116) },
    ThemePreset { name: "Solarized Dark", bg: hex(0,43,54),    toolbar_bg: hex(0,43,54),  toolbar_border: hex(7,54,66),  bull: hex(133,153,0), bear: hex(220,50,47),    grid: hexa(7,54,66,0.4),  axis_text: hexa(131,148,150,1.0), ohlc_label: hexa(147,161,161,1.0), accent: hex(42,161,152) },
    ThemePreset { name: "Dracula",        bg: hex(40,42,54),   toolbar_bg: hex(33,34,44), toolbar_border: hex(52,55,70), bull: hex(80,250,123), bear: hex(255,85,85),   grid: hexa(52,55,70,0.4), axis_text: hexa(189,147,249,1.0), ohlc_label: hexa(248,248,242,1.0), accent: hex(255,121,198) },
    ThemePreset { name: "Gruvbox",        bg: hex(40,40,40),   toolbar_bg: hex(29,32,33), toolbar_border: hex(60,56,54), bull: hex(184,187,38), bear: hex(251,73,52),   grid: hexa(60,56,54,0.4), axis_text: hexa(213,196,161,1.0), ohlc_label: hexa(235,219,178,1.0), accent: hex(254,128,25) },
    ThemePreset { name: "Catppuccin",     bg: hex(30,30,46),   toolbar_bg: hex(24,24,37), toolbar_border: hex(49,50,68), bull: hex(166,227,161), bear: hex(243,139,168), grid: hexa(49,50,68,0.4), axis_text: hexa(180,190,254,1.0), ohlc_label: hexa(205,214,244,1.0), accent: hex(203,166,247) },
    ThemePreset { name: "Tokyo Night",    bg: hex(26,27,38),   toolbar_bg: hex(22,22,30), toolbar_border: hex(36,40,59), bull: hex(158,206,106), bear: hex(247,118,142), grid: hexa(36,40,59,0.4), axis_text: hexa(122,162,247,1.0), ohlc_label: hexa(192,202,245,1.0), accent: hex(125,207,255) },
];

// ─── Mouse ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum DragZone { Chart, XAxis, YAxis }

struct Mouse {
    dragging: bool,
    zone: DragZone,
    last_x: f64,
    last_y: f64,
    cx: f64,
    cy: f64,
    right_click: Option<(f32, f32)>,
}

impl Mouse {
    fn new() -> Self { Self { dragging: false, zone: DragZone::Chart, last_x: 0.0, last_y: 0.0, cx: 0.0, cy: 0.0, right_click: None } }
    fn detect_zone(&self, w: f32, h: f32) -> DragZone {
        let (x, y) = (self.cx as f32, self.cy as f32);
        if x >= w - PR && y < h - PB { DragZone::YAxis }
        else if y >= h - PB { DragZone::XAxis }
        else { DragZone::Chart }
    }
}

struct IndicatorLine {
    values_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    color: [f32; 4],
    width: f32,
    values: Vec<f32>,
    name: String,
}

// ─── GPU State ────────────────────────────────────────────────────────────────

struct Gpu {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,

    candle_pl: wgpu::RenderPipeline,
    candle_ubuf: wgpu::Buffer,
    volume_pl: wgpu::RenderPipeline,
    volume_ubuf: wgpu::Buffer,
    grid_pl: wgpu::RenderPipeline,
    grid_vbuf: wgpu::Buffer,

    bar_buf: wgpu::Buffer,
    bgl: wgpu::BindGroupLayout,
    candle_bg: wgpu::BindGroup,
    volume_bg: wgpu::BindGroup,

    bars: Vec<Bar>,
    timestamps: Vec<i64>,  // unix timestamps for time axis labels
    bar_count: u32,
    bar_cap: u32,

    vs: f32,        // view start (float for sub-bar pan)
    vc: u32,        // view count
    price_lock: Option<(f32, f32)>,

    bg_color: [f32; 4],
    bull: [f32; 4],
    bear: [f32; 4],

    // Overlay (crosshair, drawing lines)
    overlay_pl: wgpu::RenderPipeline,
    overlay_buf: wgpu::Buffer,
    overlay_bg: wgpu::BindGroup,
    overlay_bgl: wgpu::BindGroupLayout,
    overlay_cpu: Vec<f32>,
    overlay_count: u32,

    // Indicator lines
    line_pl: wgpu::RenderPipeline,
    indicators: Vec<IndicatorLine>,

    // egui UI
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    show_context_menu: bool,
    context_menu_pos: egui::Pos2,

    // Drawings + tools
    drawings: Vec<Drawing>,
    draw_state: DrawState,
    theme_idx: usize,

    // Auto-scroll
    auto_scroll: bool,
    interaction_time: std::time::Instant,

    mouse: Mouse,
    dirty: bool,
    grid_vert_count: u32,
    grid_cpu: Vec<GridVertex>,
}

impl Gpu {
    fn new(window: Arc<Window>) -> Self {
        let win_clone = window.clone();
        let size = window.inner_size();
        // Force DX12 on Windows — Vulkan conflicts with WebView2's GPU context
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });
        let surface = instance.create_surface(window).expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })).expect("adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("chart"), memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            },
            None,
        )).expect("device");

        let caps = surface.get_capabilities(&adapter);
        let fmt = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format: fmt,
            width: size.width.max(1), height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0], view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        // Shared BGL for candle + volume (storage + uniform)
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let blend = Some(wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] });

        let make_pl = |src: &str, lbl: &str| {
            let m = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(lbl), source: wgpu::ShaderSource::Wgsl(src.into()) });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(lbl), layout: Some(&layout),
                vertex: wgpu::VertexState { module: &m, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &m, entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState { format: fmt, blend, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
                depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
            })
        };
        let candle_pl = make_pl(CANDLE_SHADER, "candle");
        let volume_pl = make_pl(VOLUME_SHADER, "volume");

        // Grid pipeline — line-list with vertex buffer
        let grid_module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("grid"), source: wgpu::ShaderSource::Wgsl(GRID_SHADER.into()) });
        let grid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[], push_constant_ranges: &[] });
        let grid_pl = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid"), layout: Some(&grid_layout),
            vertex: wgpu::VertexState {
                module: &grid_module, entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GridVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 8, shader_location: 1 },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState { module: &grid_module, entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format: fmt, blend, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::LineList, ..Default::default() },
            depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
        });

        let cap: u32 = 4096;
        let bar_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bars"), size: (cap as u64) * 24, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let candle_ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None, size: 80, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let volume_ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None, size: 80, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let grid_vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grid-verts"), size: (MAX_GRID_VERTS * std::mem::size_of::<GridVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

        let candle_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &bgl, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: bar_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: candle_ubuf.as_entire_binding() },
        ]});
        let volume_bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &bgl, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: bar_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: volume_ubuf.as_entire_binding() },
        ]});

        // Overlay pipeline (crosshair, drawing lines) — storage buffer of Line structs
        let overlay_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("overlay-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let overlay_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&overlay_bgl], push_constant_ranges: &[] });
        let overlay_module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("overlay"), source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()) });
        let overlay_pl = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay"), layout: Some(&overlay_layout),
            vertex: wgpu::VertexState { module: &overlay_module, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &overlay_module, entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState { format: fmt, blend, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
        });
        let overlay_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("overlay-lines"),
            size: (MAX_OVERLAY_LINES * OVERLAY_FLOATS_PER_LINE * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        let overlay_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &overlay_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: overlay_buf.as_entire_binding() }],
        });

        // Indicator line pipeline — same BGL as candles (storage + uniform)
        let line_pl = make_pl(LINE_SHADER, "line");

        // egui UI
        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
        let egui_state = egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, &*win_clone, Some(win_clone.scale_factor() as f32), None, None);
        let egui_renderer = egui_wgpu::Renderer::new(&device, fmt, None, 1, false);

        Self {
            window: win_clone,
            device, queue, surface, config,
            candle_pl, candle_ubuf, volume_pl, volume_ubuf, grid_pl, grid_vbuf,
            bar_buf, bgl, candle_bg, volume_bg,
            overlay_pl, overlay_buf, overlay_bg, overlay_bgl,
            overlay_cpu: vec![0.0; MAX_OVERLAY_LINES * OVERLAY_FLOATS_PER_LINE],
            overlay_count: 0,
            line_pl, indicators: Vec::new(),
            egui_ctx, egui_state, egui_renderer,
            show_context_menu: false, context_menu_pos: egui::Pos2::ZERO,
            bars: Vec::new(), timestamps: Vec::new(), bar_count: 0, bar_cap: cap,
            vs: 0.0, vc: 200, price_lock: None,
            bg_color: [0.05, 0.05, 0.11, 1.0],
            bull: [0.15, 0.65, 0.6, 1.0], bear: [0.94, 0.33, 0.31, 1.0],
            drawings: Vec::new(),
            draw_state: DrawState::new(),
            theme_idx: 0,
            auto_scroll: true,
            interaction_time: std::time::Instant::now(),
            mouse: Mouse::new(), dirty: true,
            grid_vert_count: 0, grid_cpu: vec![GridVertex { pos: [0.0; 2], color: [0.0; 4] }; MAX_GRID_VERTS],
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 { return; }
        self.config.width = w; self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.dirty = true;
    }

    fn ensure_bar_buf(&mut self) {
        if self.bar_count <= self.bar_cap { return; }
        let new = (self.bar_count * 2).max(4096);
        self.bar_cap = new;
        self.bar_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bars"), size: (new as u64) * 24, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        self.candle_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &self.bgl, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: self.bar_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: self.candle_ubuf.as_entire_binding() },
        ]});
        self.volume_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &self.bgl, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: self.bar_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: self.volume_ubuf.as_entire_binding() },
        ]});
        self.queue.write_buffer(&self.bar_buf, 0, bytemuck::cast_slice(&self.bars));
    }

    fn compute_indicators(&mut self) {
        self.indicators.clear();
        let n = self.bars.len();
        if n < 20 { return; }

        let closes: Vec<f32> = self.bars.iter().map(|b| b.close).collect();

        // SMA 20 — cyan
        let sma20 = compute_sma(&closes, 20);
        self.add_indicator("SMA20", &sma20, [0.0, 0.75, 0.95, 0.85], 1.5);

        // SMA 50 — orange
        if n >= 50 {
            let sma50 = compute_sma(&closes, 50);
            self.add_indicator("SMA50", &sma50, [0.95, 0.6, 0.1, 0.75], 1.5);
        }

        // EMA 12 — yellow
        let ema12 = compute_ema(&closes, 12);
        self.add_indicator("EMA12", &ema12, [0.95, 0.85, 0.2, 0.7], 1.0);

        // EMA 26 — purple
        let ema26 = compute_ema(&closes, 26);
        self.add_indicator("EMA26", &ema26, [0.7, 0.4, 0.9, 0.7], 1.0);
    }

    fn add_indicator(&mut self, name: &str, values: &[f32], color: [f32; 4], width: f32) {
        let f32_data: Vec<f32> = values.to_vec();
        let values_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(name), size: (f32_data.len() * 4).max(64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        self.queue.write_buffer(&values_buf, 0, bytemuck::cast_slice(&f32_data));
        let uniform_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None, size: 80, // padded to match shared bind group layout
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: values_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            ],
        });
        self.indicators.push(IndicatorLine {
            values_buf, uniform_buf, bind_group,
            color, width, values: f32_data, name: name.to_string(),
        });
    }

    fn process(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, timestamps, .. } => {
                self.bars = bars;
                self.timestamps = timestamps;
                self.bar_count = self.bars.len() as u32;
                self.vs = (self.bar_count as f32 - self.vc as f32 + RIGHT_MARGIN_BARS as f32).max(0.0);
                self.price_lock = None;
                self.ensure_bar_buf();
                self.queue.write_buffer(&self.bar_buf, 0, bytemuck::cast_slice(&self.bars));
                self.compute_indicators();
                self.dirty = true;
            }
            ChartCommand::AppendBar { bar, timestamp, .. } => {
                self.bars.push(bar);
                self.timestamps.push(timestamp);
                self.bar_count = self.bars.len() as u32;
                self.ensure_bar_buf();
                self.queue.write_buffer(&self.bar_buf, (self.bar_count as u64 - 1) * 24, bytemuck::bytes_of(&bar));
                // Auto-scroll if enabled and near end
                if self.auto_scroll {
                    let max_vs = self.bar_count as f32 - self.vc as f32 + RIGHT_MARGIN_BARS as f32;
                    self.vs = max_vs.max(0.0);
                }
                self.dirty = true;
            }
            ChartCommand::UpdateLastBar { bar, .. } => {
                if let Some(last) = self.bars.last_mut() {
                    *last = bar;
                    self.queue.write_buffer(&self.bar_buf, (self.bar_count as u64 - 1) * 24, bytemuck::bytes_of(&bar));
                    // Auto-scroll keeps viewport at end
                    if self.auto_scroll {
                        let max_vs = self.bar_count as f32 - self.vc as f32 + RIGHT_MARGIN_BARS as f32;
                        self.vs = max_vs.max(0.0);
                    }
                    self.dirty = true;
                }
            }
            ChartCommand::SetViewport { view_start, view_count, .. } => {
                self.vs = view_start as f32; self.vc = view_count; self.dirty = true;
            }
            ChartCommand::SetTheme { background, bull_color, bear_color } => {
                self.bg_color = background; self.bull = bull_color; self.bear = bear_color; self.dirty = true;
            }
            ChartCommand::SetDrawing(d) => {
                self.drawings.retain(|x| x.id != d.id);
                self.drawings.push(d);
                self.dirty = true;
            }
            ChartCommand::RemoveDrawing { id } => {
                self.drawings.retain(|x| x.id != id);
                self.dirty = true;
            }
            ChartCommand::ClearDrawings => {
                self.drawings.clear();
                self.dirty = true;
            }
            ChartCommand::Resize { width, height } => self.resize(width, height),
            ChartCommand::Shutdown => {}
        }
    }

    // ── Mouse ─────────────────────────────────────────────────────────────────

    fn mouse_down(&mut self) {
        let w = self.config.width as f32;
        let h = self.config.height as f32;
        self.mouse.dragging = true;
        self.mouse.zone = self.mouse.detect_zone(w, h);
        self.mouse.last_x = self.mouse.cx;
        self.mouse.last_y = self.mouse.cy;
        // Pause auto-scroll on any interaction
        self.auto_scroll = false;
        self.interaction_time = std::time::Instant::now();
    }

    fn mouse_up(&mut self) {
        self.mouse.dragging = false;
        self.draw_state.dragging_id = None;
    }

    fn mouse_move(&mut self, x: f64, y: f64) {
        self.mouse.cx = x; self.mouse.cy = y;

        // Handle drawing drag (whole or endpoint)
        if let Some(ref id) = self.draw_state.dragging_id.clone() {
            let new_price = self.y_to_price(y as f32);
            let new_bar = self.x_to_bar(x as f32);
            let dp = new_price - self.draw_state.drag_start_price;
            let db = new_bar - self.draw_state.drag_start_bar;
            let ep = self.draw_state.drag_endpoint;
            if let Some(d) = self.drawings.iter_mut().find(|d| d.id == *id) {
                match &mut d.kind {
                    DrawingKind::HLine { price } => *price += dp,
                    DrawingKind::TrendLine { price0, bar0, price1, bar1 } => {
                        match ep {
                            0 => { *price0 = new_price; *bar0 = new_bar; } // edit first endpoint
                            1 => { *price1 = new_price; *bar1 = new_bar; } // edit second endpoint
                            _ => { *price0 += dp; *price1 += dp; *bar0 += db; *bar1 += db; } // move whole
                        }
                    }
                    DrawingKind::HZone { price0, price1 } => {
                        match ep {
                            0 => *price0 = new_price,
                            1 => *price1 = new_price,
                            _ => { *price0 += dp; *price1 += dp; }
                        }
                    }
                }
            }
            self.draw_state.drag_start_price = new_price;
            self.draw_state.drag_start_bar = new_bar;
            self.dirty = true;
            return;
        }

        if !self.mouse.dragging { return; }
        let dx = x - self.mouse.last_x;
        let dy = y - self.mouse.last_y;
        self.mouse.last_x = x; self.mouse.last_y = y;

        let w = self.config.width as f32;
        let cw = w - PR;
        let total = self.vc + RIGHT_MARGIN_BARS;
        let step = cw / total as f32;

        match self.mouse.zone {
            DragZone::Chart => {
                let d = dx as f32 / step;
                if d.abs() < 0.0001 { return; }
                let max = self.bar_count as f32 - self.vc as f32 + 200.0;
                self.vs = (self.vs - d).max(0.0).min(max);
                self.dirty = true;
            }
            DragZone::XAxis => {
                if dx.abs() <= 1.0 { return; }
                let f = if dx > 0.0 { 1.05_f32 } else { 0.95 };
                let old = self.vc;
                let new = ((old as f32 * f).round() as u32).max(20).min(self.bar_count);
                if new == old { return; }
                let delta = (old as i32 - new as i32) / 2;
                self.vc = new;
                self.vs = (self.vs + delta as f32).max(0.0);
                self.price_lock = None;
                self.dirty = true;
            }
            DragZone::YAxis => {
                if dy.abs() <= 1.0 { return; }
                let f = if dy > 0.0 { 1.05_f32 } else { 0.95 };
                let (lo, hi) = self.price_range();
                let c = (lo + hi) / 2.0;
                let half = ((hi - lo) / 2.0) * f;
                self.price_lock = Some((c - half, c + half));
                self.dirty = true;
            }
        }
    }

    fn scroll(&mut self, dy: f32) {
        let f = if dy > 0.0 { 1.1_f32 } else { 0.9 };
        let old = self.vc;
        let new = ((old as f32 * f).round() as u32).max(20).min(self.bar_count);
        if new == old { return; }
        let d = (old as i32 - new as i32) / 2;
        self.vc = new;
        self.vs = (self.vs + d as f32).max(0.0);
        self.price_lock = None;
        self.dirty = true;
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn price_range(&self) -> (f32, f32) {
        if let Some(r) = self.price_lock { return r; }
        let s = self.vs as u32;
        let e = (s + self.vc).min(self.bar_count);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in s..e { if let Some(b) = self.bars.get(i as usize) { lo = lo.min(b.low); hi = hi.max(b.high); } }
        if lo >= hi { lo -= 0.5; hi += 0.5; }
        let p = (hi - lo) * 0.05;
        (lo - p, hi + p)
    }

    fn apply_theme(&mut self) {
        let t = &THEMES[self.theme_idx];
        self.bg_color = t.bg;
        self.bull = t.bull;
        self.bear = t.bear;
    }

    /// Convert pixel Y to price
    fn y_to_price(&self, py: f32) -> f32 {
        let h = self.config.height as f32;
        let ch = h - PT - PB;
        let (min_p, max_p) = self.price_range();
        min_p + (max_p - min_p) * (1.0 - (py - PT) / ch)
    }

    /// Convert pixel X to bar index (float)
    fn x_to_bar(&self, px: f32) -> f32 {
        let w = self.config.width as f32;
        let cw = w - PR;
        let total = self.vc + RIGHT_MARGIN_BARS;
        let step = cw / total as f32;
        let frac = self.vs - self.vs.floor();
        let offset = (frac * step).round();
        (px + offset - step * 0.5) / step + self.vs
    }

    /// Hit-test drawings — returns (drawing id, endpoint index: -1=whole, 0=first, 1=second)
    fn hit_test_drawing(&self, px: f32, py: f32) -> Option<(String, i32)> {
        let w = self.config.width as f32;
        let h = self.config.height as f32;
        let cw = w - PR;
        let ch = h - PT - PB;
        let (min_p, max_p) = self.price_range();

        let price_to_y = |p: f32| -> f32 { PT + (max_p - p) / (max_p - min_p) * ch };
        let bar_to_x = |bar: f32| -> f32 {
            let total = self.vc + RIGHT_MARGIN_BARS;
            let step = cw / total as f32;
            let frac = self.vs - self.vs.floor();
            let offset = (frac * step).round();
            (bar - self.vs) * step + step * 0.5 - offset
        };

        for d in self.drawings.iter().rev() {
            match &d.kind {
                DrawingKind::HLine { price } => {
                    let y = price_to_y(*price);
                    if (py - y).abs() < 5.0 && px < cw { return Some((d.id.clone(), -1)); }
                }
                DrawingKind::TrendLine { price0, bar0, price1, bar1 } => {
                    let x0 = bar_to_x(*bar0); let y0 = price_to_y(*price0);
                    let x1 = bar_to_x(*bar1); let y1 = price_to_y(*price1);
                    // Check endpoints first (for editing)
                    if ((px - x0).powi(2) + (py - y0).powi(2)).sqrt() < 8.0 { return Some((d.id.clone(), 0)); }
                    if ((px - x1).powi(2) + (py - y1).powi(2)).sqrt() < 8.0 { return Some((d.id.clone(), 1)); }
                    // Then check line segment
                    let dx = x1 - x0; let dy = y1 - y0;
                    let len2 = dx * dx + dy * dy;
                    if len2 > 0.0 {
                        let t = ((px - x0) * dx + (py - y0) * dy) / len2;
                        let t = t.max(0.0).min(1.0);
                        let nx = x0 + t * dx; let ny = y0 + t * dy;
                        let dist = ((px - nx).powi(2) + (py - ny).powi(2)).sqrt();
                        if dist < 6.0 { return Some((d.id.clone(), -1)); }
                    }
                }
                DrawingKind::HZone { price0, price1 } => {
                    let y0 = price_to_y(*price0); let y1 = price_to_y(*price1);
                    // Check edges for endpoint editing
                    if (py - y0).abs() < 5.0 && px < cw { return Some((d.id.clone(), 0)); }
                    if (py - y1).abs() < 5.0 && px < cw { return Some((d.id.clone(), 1)); }
                    let top = y0.min(y1); let bot = y0.max(y1);
                    if py >= top - 4.0 && py <= bot + 4.0 && px < cw { return Some((d.id.clone(), -1)); }
                }
            }
        }
        None
    }

    fn build_grid(&mut self, w: f32, h: f32, min_p: f32, max_p: f32) {
        let mut n = 0usize;
        let cw = w - PR;
        let ch = h - PT - PB;
        let axis_color = [0.35, 0.35, 0.4, 0.15];
        let border_color = [0.35, 0.35, 0.4, 0.4];

        // price→clip helper
        let py = |price: f32| -> f32 { 1.0 - 2.0 * (PT + (max_p - price) / (max_p - min_p) * ch) / h };

        // Horizontal price grid
        let range = max_p - min_p;
        let raw_step = range / 8.0;
        let mag = 10.0_f32.powf(raw_step.log10().floor());
        let nice = [1.0, 2.0, 2.5, 5.0, 10.0];
        let step = nice.iter().map(|&s| s * mag).find(|&s| s >= raw_step).unwrap_or(raw_step);
        let first = (min_p / step).ceil() * step;
        let mut p = first;
        while p <= max_p && n + 2 <= MAX_GRID_VERTS {
            let cy = py(p);
            let lx = -1.0;
            let rx = (cw / w) * 2.0 - 1.0;
            self.grid_cpu[n] = GridVertex { pos: [lx, cy], color: axis_color };
            self.grid_cpu[n + 1] = GridVertex { pos: [rx, cy], color: axis_color };
            n += 2;
            p += step;
        }

        // Chart border box
        let l = -1.0_f32;
        let r = (cw / w) * 2.0 - 1.0;
        let t = 1.0 - 2.0 * PT / h;
        let b = 1.0 - 2.0 * (h - PB) / h;
        if n + 8 <= MAX_GRID_VERTS {
            // Right border (y-axis)
            self.grid_cpu[n] = GridVertex { pos: [r, t], color: border_color };
            self.grid_cpu[n + 1] = GridVertex { pos: [r, b], color: border_color };
            // Bottom border (x-axis)
            self.grid_cpu[n + 2] = GridVertex { pos: [l, b], color: border_color };
            self.grid_cpu[n + 3] = GridVertex { pos: [r, b], color: border_color };
            // Top border
            self.grid_cpu[n + 4] = GridVertex { pos: [l, t], color: border_color };
            self.grid_cpu[n + 5] = GridVertex { pos: [r, t], color: border_color };
            // Left border
            self.grid_cpu[n + 6] = GridVertex { pos: [l, t], color: border_color };
            self.grid_cpu[n + 7] = GridVertex { pos: [l, b], color: border_color };
            n += 8;
        }

        self.grid_vert_count = n as u32;
        if n > 0 {
            self.queue.write_buffer(&self.grid_vbuf, 0, bytemuck::cast_slice(&self.grid_cpu[..n]));
        }
    }

    fn build_overlay(&mut self, w: f32, h: f32) {
        let mut n = 0usize;
        let mx = self.mouse.cx as f32;
        let my = self.mouse.cy as f32;

        // Only draw crosshair if mouse is in chart area and not dragging
        if !self.mouse.dragging && mx >= 0.0 && mx < w - PR && my >= PT && my < h - PB {
            let clip_x = (mx / w) * 2.0 - 1.0;
            let clip_y = 1.0 - (my / h) * 2.0;
            let clip_left = -1.0;
            let clip_right = ((w - PR) / w) * 2.0 - 1.0;
            let clip_top = 1.0 - (PT / h) * 2.0;
            let clip_bottom = 1.0 - ((h - PB) / h) * 2.0;
            // Line widths: billboard extends perpendicular, so horizontal lines need height-based width
            let lw_h = 1.5 / h * 2.0;  // horizontal line — billboard extends vertically
            let lw_v = 1.5 / w * 2.0;  // vertical line — billboard extends horizontally

            // Horizontal crosshair
            if n + OVERLAY_FLOATS_PER_LINE <= self.overlay_cpu.len() {
                let o = n;
                let dash = 8.0 / w * 2.0;
                let gap = 4.0 / w * 2.0;
                self.overlay_cpu[o] = clip_left; self.overlay_cpu[o+1] = clip_y;
                self.overlay_cpu[o+2] = clip_right; self.overlay_cpu[o+3] = clip_y;
                self.overlay_cpu[o+4] = 1.0; self.overlay_cpu[o+5] = 1.0; self.overlay_cpu[o+6] = 1.0; self.overlay_cpu[o+7] = 0.25;
                self.overlay_cpu[o+8] = dash; self.overlay_cpu[o+9] = gap;
                self.overlay_cpu[o+10] = lw_h; self.overlay_cpu[o+11] = 0.0;
                n += OVERLAY_FLOATS_PER_LINE;
            }
            // Vertical crosshair
            if n + OVERLAY_FLOATS_PER_LINE <= self.overlay_cpu.len() {
                let o = n;
                let dash_v = 8.0 / h * 2.0;
                let gap_v = 4.0 / h * 2.0;
                self.overlay_cpu[o] = clip_x; self.overlay_cpu[o+1] = clip_top;
                self.overlay_cpu[o+2] = clip_x; self.overlay_cpu[o+3] = clip_bottom;
                self.overlay_cpu[o+4] = 1.0; self.overlay_cpu[o+5] = 1.0; self.overlay_cpu[o+6] = 1.0; self.overlay_cpu[o+7] = 0.25;
                self.overlay_cpu[o+8] = dash_v; self.overlay_cpu[o+9] = gap_v;
                self.overlay_cpu[o+10] = lw_v; self.overlay_cpu[o+11] = 0.0;
                n += OVERLAY_FLOATS_PER_LINE;
            }
        }

        // Drawings (hlines, trendlines, hzones)
        let cw = w - PR;
        let ch = h - PT - PB;
        let (min_p, max_p) = self.price_range();
        let price_to_clip_y = |p: f32| -> f32 {
            1.0 - 2.0 * (PT + (max_p - p) / (max_p - min_p) * ch) / h
        };
        let bar_to_clip_x = |bar_idx: f32| -> f32 {
            let step = cw / (self.vc + RIGHT_MARGIN_BARS) as f32;
            let view_idx = bar_idx - self.vs;
            let px = view_idx * step + step * 0.5;
            (px / w) * 2.0 - 1.0
        };
        let clip_left = -1.0_f32;
        let clip_right = (cw / w) * 2.0 - 1.0;
        let lw_draw_h = 1.5 / h * 2.0;
        let lw_draw_v = 1.5 / w * 2.0;

        for d in &self.drawings {
            if n + OVERLAY_FLOATS_PER_LINE * 2 > self.overlay_cpu.len() { break; }
            let dash = if d.dashed { 8.0 / w * 2.0 } else { 0.0 };
            let gap = if d.dashed { 4.0 / w * 2.0 } else { 0.0 };

            match &d.kind {
                DrawingKind::HLine { price } => {
                    let cy = price_to_clip_y(*price);
                    if cy > -1.0 && cy < 1.0 {
                        let o = n;
                        self.overlay_cpu[o] = clip_left; self.overlay_cpu[o+1] = cy;
                        self.overlay_cpu[o+2] = clip_right; self.overlay_cpu[o+3] = cy;
                        self.overlay_cpu[o+4] = d.color[0]; self.overlay_cpu[o+5] = d.color[1];
                        self.overlay_cpu[o+6] = d.color[2]; self.overlay_cpu[o+7] = d.color[3];
                        self.overlay_cpu[o+8] = dash; self.overlay_cpu[o+9] = gap;
                        self.overlay_cpu[o+10] = lw_draw_h * d.width; self.overlay_cpu[o+11] = 0.0;
                        n += OVERLAY_FLOATS_PER_LINE;
                    }
                }
                DrawingKind::TrendLine { price0, bar0, price1, bar1 } => {
                    let x0 = bar_to_clip_x(*bar0);
                    let y0 = price_to_clip_y(*price0);
                    let x1 = bar_to_clip_x(*bar1);
                    let y1 = price_to_clip_y(*price1);
                    let lw = (lw_draw_h + lw_draw_v) / 2.0 * d.width;
                    // Main line
                    let o = n;
                    self.overlay_cpu[o]=x0; self.overlay_cpu[o+1]=y0; self.overlay_cpu[o+2]=x1; self.overlay_cpu[o+3]=y1;
                    self.overlay_cpu[o+4]=d.color[0]; self.overlay_cpu[o+5]=d.color[1]; self.overlay_cpu[o+6]=d.color[2]; self.overlay_cpu[o+7]=d.color[3];
                    self.overlay_cpu[o+8]=dash; self.overlay_cpu[o+9]=gap; self.overlay_cpu[o+10]=lw; self.overlay_cpu[o+11]=0.0;
                    n += OVERLAY_FLOATS_PER_LINE;
                    // Endpoint handles (small horizontal marks)
                    let handle_size = 5.0 / w * 2.0;
                    let handle_h = 5.0 / h * 2.0;
                    for (ex, ey) in [(x0, y0), (x1, y1)] {
                        if n + OVERLAY_FLOATS_PER_LINE > self.overlay_cpu.len() { break; }
                        let o = n;
                        self.overlay_cpu[o]=ex-handle_size; self.overlay_cpu[o+1]=ey; self.overlay_cpu[o+2]=ex+handle_size; self.overlay_cpu[o+3]=ey;
                        self.overlay_cpu[o+4]=1.0; self.overlay_cpu[o+5]=1.0; self.overlay_cpu[o+6]=1.0; self.overlay_cpu[o+7]=0.7;
                        self.overlay_cpu[o+8]=0.0; self.overlay_cpu[o+9]=0.0; self.overlay_cpu[o+10]=handle_h; self.overlay_cpu[o+11]=0.0;
                        n += OVERLAY_FLOATS_PER_LINE;
                    }
                }
                DrawingKind::HZone { price0, price1 } => {
                    let cy0 = price_to_clip_y(*price0);
                    let cy1 = price_to_clip_y(*price1);
                    // Two border lines
                    for cy in [cy0, cy1] {
                        if n + OVERLAY_FLOATS_PER_LINE > self.overlay_cpu.len() { break; }
                        let o = n;
                        self.overlay_cpu[o] = clip_left; self.overlay_cpu[o+1] = cy;
                        self.overlay_cpu[o+2] = clip_right; self.overlay_cpu[o+3] = cy;
                        self.overlay_cpu[o+4] = d.color[0]; self.overlay_cpu[o+5] = d.color[1];
                        self.overlay_cpu[o+6] = d.color[2]; self.overlay_cpu[o+7] = d.color[3] * 0.5;
                        self.overlay_cpu[o+8] = dash; self.overlay_cpu[o+9] = gap;
                        self.overlay_cpu[o+10] = lw_draw_h * d.width; self.overlay_cpu[o+11] = 0.0;
                        n += OVERLAY_FLOATS_PER_LINE;
                    }
                }
            }
        }

        // Zoom selection rectangle
        if self.draw_state.zoom_selecting {
            let (sx, sy) = self.draw_state.zoom_start;
            let mx = self.mouse.cx as f32;
            let my = self.mouse.cy as f32;
            if n + OVERLAY_FLOATS_PER_LINE * 4 <= self.overlay_cpu.len() {
                let x0 = (sx.min(mx) / w) * 2.0 - 1.0;
                let x1 = (sx.max(mx) / w) * 2.0 - 1.0;
                let y0 = 1.0 - (sy.min(my) / h) * 2.0;
                let y1 = 1.0 - (sy.max(my) / h) * 2.0;
                let dash = 4.0 / w * 2.0;
                let gap = 3.0 / w * 2.0;
                let lw_sel_h = 1.0 / h * 2.0;
                let lw_sel_v = 1.0 / w * 2.0;
                let sc = [0.5, 0.7, 1.0, 0.7_f32]; // selection color
                // Top
                let o = n;
                self.overlay_cpu[o]=x0; self.overlay_cpu[o+1]=y0; self.overlay_cpu[o+2]=x1; self.overlay_cpu[o+3]=y0;
                self.overlay_cpu[o+4]=sc[0]; self.overlay_cpu[o+5]=sc[1]; self.overlay_cpu[o+6]=sc[2]; self.overlay_cpu[o+7]=sc[3];
                self.overlay_cpu[o+8]=dash; self.overlay_cpu[o+9]=gap; self.overlay_cpu[o+10]=lw_sel_h; self.overlay_cpu[o+11]=0.0;
                n += OVERLAY_FLOATS_PER_LINE;
                // Bottom
                let o = n;
                self.overlay_cpu[o]=x0; self.overlay_cpu[o+1]=y1; self.overlay_cpu[o+2]=x1; self.overlay_cpu[o+3]=y1;
                self.overlay_cpu[o+4]=sc[0]; self.overlay_cpu[o+5]=sc[1]; self.overlay_cpu[o+6]=sc[2]; self.overlay_cpu[o+7]=sc[3];
                self.overlay_cpu[o+8]=dash; self.overlay_cpu[o+9]=gap; self.overlay_cpu[o+10]=lw_sel_h; self.overlay_cpu[o+11]=0.0;
                n += OVERLAY_FLOATS_PER_LINE;
                // Left
                let o = n;
                self.overlay_cpu[o]=x0; self.overlay_cpu[o+1]=y0; self.overlay_cpu[o+2]=x0; self.overlay_cpu[o+3]=y1;
                self.overlay_cpu[o+4]=sc[0]; self.overlay_cpu[o+5]=sc[1]; self.overlay_cpu[o+6]=sc[2]; self.overlay_cpu[o+7]=sc[3];
                self.overlay_cpu[o+8]=dash; self.overlay_cpu[o+9]=gap; self.overlay_cpu[o+10]=lw_sel_v; self.overlay_cpu[o+11]=0.0;
                n += OVERLAY_FLOATS_PER_LINE;
                // Right
                let o = n;
                self.overlay_cpu[o]=x1; self.overlay_cpu[o+1]=y0; self.overlay_cpu[o+2]=x1; self.overlay_cpu[o+3]=y1;
                self.overlay_cpu[o+4]=sc[0]; self.overlay_cpu[o+5]=sc[1]; self.overlay_cpu[o+6]=sc[2]; self.overlay_cpu[o+7]=sc[3];
                self.overlay_cpu[o+8]=dash; self.overlay_cpu[o+9]=gap; self.overlay_cpu[o+10]=lw_sel_v; self.overlay_cpu[o+11]=0.0;
                n += OVERLAY_FLOATS_PER_LINE;
            }
        }

        // Trendline preview (pending point to mouse)
        if self.draw_state.tool == DrawTool::TrendLine {
            if let Some((b0, p0)) = self.draw_state.pending_point {
                let mx = self.mouse.cx as f32;
                let my = self.mouse.cy as f32;
                if mx >= 0.0 && mx < w - PR && my >= PT && my < h - PB && n + OVERLAY_FLOATS_PER_LINE <= self.overlay_cpu.len() {
                    let x0 = bar_to_clip_x(b0);
                    let y0 = price_to_clip_y(p0);
                    let x1 = (mx / w) * 2.0 - 1.0;
                    let y1 = 1.0 - (my / h) * 2.0;
                    let lw = (lw_draw_h + lw_draw_v) / 2.0;
                    let o = n;
                    self.overlay_cpu[o] = x0; self.overlay_cpu[o+1] = y0;
                    self.overlay_cpu[o+2] = x1; self.overlay_cpu[o+3] = y1;
                    self.overlay_cpu[o+4] = 0.3; self.overlay_cpu[o+5] = 0.6; self.overlay_cpu[o+6] = 1.0; self.overlay_cpu[o+7] = 0.5;
                    self.overlay_cpu[o+8] = 6.0 / w * 2.0; self.overlay_cpu[o+9] = 4.0 / w * 2.0;
                    self.overlay_cpu[o+10] = lw; self.overlay_cpu[o+11] = 0.0;
                    n += OVERLAY_FLOATS_PER_LINE;
                }
            }
        }

        self.overlay_count = (n / OVERLAY_FLOATS_PER_LINE) as u32;
        if self.overlay_count > 0 {
            self.queue.write_buffer(&self.overlay_buf, 0, bytemuck::cast_slice(&self.overlay_cpu[..n]));
        }
    }

    // ── Render ────────────────────────────────────────────────────────────────

    fn render(&mut self) {
        self.dirty = false;
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => { self.surface.configure(&self.device, &self.config); return; }
        };
        let view = output.texture.create_view(&Default::default());
        let w = self.config.width as f32;
        let h = self.config.height as f32;
        let cw = w - PR;
        let ch = h - PT - PB;
        let total = self.vc + RIGHT_MARGIN_BARS;
        let step_px = (cw / total as f32).floor().max(1.0);
        let half_step = (step_px / 2.0).floor();
        let frac = self.vs - self.vs.floor();
        let offset_px = (frac * step_px).round();

        let vs = self.vs as u32;
        let end = (vs + self.vc).min(self.bar_count);
        let dc = end.saturating_sub(vs);
        if dc == 0 { output.present(); return; }

        let (min_p, max_p) = self.price_range();
        let pa = 1.0 - 2.0 * PT / h - (max_p / (max_p - min_p)) * (2.0 * ch / h);
        let pb = (2.0 * ch / h) / (max_p - min_p);

        // Candle uniform
        self.queue.write_buffer(&self.candle_ubuf, 0, bytemuck::bytes_of(&CandleUniforms {
            view_start: vs, view_count: dc, _pad0: 0, _pad1: 0,
            step_px, half_step_px: half_step, price_a: pa, price_b: pb,
            offset_px, _pad2: 0.0, canvas_width: w, canvas_height: h,
            up_color: self.bull, down_color: self.bear,
        }));

        // Volume uniform
        let bsc = step_px * 2.0 / w;
        let pof = offset_px / step_px;
        let bwc = (step_px * 0.4) * 2.0 / w;
        let mut mv: f32 = 0.0;
        for i in vs..end { if let Some(b) = self.bars.get(i as usize) { mv = mv.max(b.volume); } }
        if mv == 0.0 { mv = 1.0; }
        self.queue.write_buffer(&self.volume_ubuf, 0, bytemuck::bytes_of(&VolumeUniforms {
            view_start: vs, view_count: dc, _pad0: 0, _pad1: 0,
            bar_step_clip: bsc, pixel_offset_frac: pof, body_width_clip: bwc, max_volume: mv,
            vol_bottom_clip: -1.0, vol_height_clip: 0.3, _pad2: 0.0, _pad3: 0.0,
            up_color: [0.18, 0.78, 0.45, 0.25], down_color: [0.93, 0.27, 0.27, 0.25],
        }));

        // Grid + overlay
        self.build_grid(w, h, min_p, max_p);
        self.build_overlay(w, h);

        // Encode
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view, resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.bg_color[0] as f64, g: self.bg_color[1] as f64,
                            b: self.bg_color[2] as f64, a: self.bg_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
            });

            // Grid (behind everything)
            if self.grid_vert_count > 0 {
                pass.set_pipeline(&self.grid_pl);
                pass.set_vertex_buffer(0, self.grid_vbuf.slice(..));
                pass.draw(0..self.grid_vert_count, 0..1);
            }
            // Volume
            pass.set_pipeline(&self.volume_pl);
            pass.set_bind_group(0, &self.volume_bg, &[]);
            pass.draw(0..6, 0..dc);
            // Candles
            pass.set_pipeline(&self.candle_pl);
            pass.set_bind_group(0, &self.candle_bg, &[]);
            pass.draw(0..18, 0..dc);
            // Indicator lines
            if !self.indicators.is_empty() {
                let bsc = step_px * 2.0 / w;
                let pof = offset_px / step_px;
                pass.set_pipeline(&self.line_pl);
                for ind in &self.indicators {
                    let seg_count = dc.saturating_sub(1).min(ind.values.len().saturating_sub(vs as usize + 1) as u32);
                    if seg_count == 0 { continue; }
                    let lw_clip = ind.width * 2.0 / w;
                    let lu = LineUniforms {
                        view_start: vs, seg_count, _pad0: 0, _pad1: 0,
                        bar_step_clip: bsc, pixel_offset_frac: pof, price_a: pa, price_b: pb,
                        line_width_clip: lw_clip, _pad2: 0.0, _pad3: 0.0, _pad4: 0.0,
                        color: ind.color,
                        _extra_pad: [0.0; 4],
                    };
                    self.queue.write_buffer(&ind.uniform_buf, 0, bytemuck::bytes_of(&lu));
                    pass.set_bind_group(0, &ind.bind_group, &[]);
                    pass.draw(0..6, 0..seg_count);
                }
            }
            // Overlay (crosshair, drawing lines) — on top of everything
            if self.overlay_count > 0 {
                pass.set_pipeline(&self.overlay_pl);
                pass.set_bind_group(0, &self.overlay_bg, &[]);
                pass.draw(0..6, 0..self.overlay_count);
            }
        }

        self.queue.submit(std::iter::once(enc.finish()));

        // ── egui UI pass ──────────────────────────────────────────────────────
        let raw_input = self.egui_state.take_egui_input(&*self.window);

        // Extract values needed by the egui closure to avoid borrowing self
        let theme_idx = self.theme_idx;
        let bar_count = self.bar_count;
        let auto_scroll = self.auto_scroll;
        let mouse_cx = self.mouse.cx as f32;
        let mouse_cy = self.mouse.cy as f32;
        let mouse_dragging = self.mouse.dragging;
        let draw_tool = self.draw_state.tool;
        let pending_pt = self.draw_state.pending_point;
        let last_bar = if dc > 0 { self.bars.get((end - 1) as usize).copied() } else { None };
        let mut new_theme_idx: Option<usize> = None;
        let mut resume_scroll = false;

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            let t = &THEMES[theme_idx];
            let bull_color = egui::Color32::from_rgba_unmultiplied((t.bull[0]*255.0) as u8, (t.bull[1]*255.0) as u8, (t.bull[2]*255.0) as u8, 220);
            let bear_color = egui::Color32::from_rgba_unmultiplied((t.bear[0]*255.0) as u8, (t.bear[1]*255.0) as u8, (t.bear[2]*255.0) as u8, 220);
            let dim = egui::Color32::from_rgba_unmultiplied(140, 140, 150, 200);

            // Price labels on right axis
            let area = egui::Area::new(egui::Id::new("price_axis")).fixed_pos(egui::pos2(w - PR + 4.0, 0.0));
            area.show(ctx, |ui| {
                let range = max_p - min_p;
                let raw_step = range / 8.0;
                let mag_val = 10.0_f32.powf(raw_step.log10().floor());
                let nice = [1.0, 2.0, 2.5, 5.0, 10.0];
                let price_step = nice.iter().map(|&s| s * mag_val).find(|&s| s >= raw_step).unwrap_or(raw_step);
                let mut p = (min_p / price_step).ceil() * price_step;
                while p <= max_p {
                    let py = PT + (max_p - p) / (max_p - min_p) * ch;
                    if py >= PT && py <= h - PB {
                        let dec = if p >= 10.0 { 2usize } else { 4 };
                        ui.put(egui::Rect::from_min_size(egui::pos2(0.0, py - 6.0), egui::vec2(70.0, 14.0)),
                            egui::Label::new(egui::RichText::new(format!("{:.1$}", p, dec)).monospace().size(10.0).color(dim)));
                    }
                    p += price_step;
                }
                // Crosshair price
                if !mouse_dragging && mouse_cx >= 0.0 && mouse_cx < w - PR && mouse_cy >= PT && mouse_cy < h - PB {
                    let ch_price = min_p + (max_p - min_p) * (1.0 - (mouse_cy - PT) / ch);
                    let dec = if ch_price >= 10.0 { 2usize } else { 4 };
                    ui.put(egui::Rect::from_min_size(egui::pos2(0.0, mouse_cy - 6.0), egui::vec2(70.0, 14.0)),
                        egui::Label::new(egui::RichText::new(format!("{:.1$}", ch_price, dec)).monospace().size(10.0).color(egui::Color32::WHITE)));
                }
            });

            // OHLC label (top-left)
            if let Some(bar) = last_bar {
                let c = if bar.close >= bar.open { bull_color } else { bear_color };
                egui::Area::new(egui::Id::new("ohlc")).fixed_pos(egui::pos2(8.0, 4.0)).show(ctx, |ui| {
                    ui.label(egui::RichText::new(format!("O {:.2}  H {:.2}  L {:.2}  C {:.2}  V {:.0}", bar.open, bar.high, bar.low, bar.close, bar.volume))
                        .monospace().size(11.0).color(c));
                });
            }

            // Drawing tool hint
            if draw_tool != DrawTool::None {
                let hint = match draw_tool {
                    DrawTool::HLine => "Click to place HLine (Esc to cancel)",
                    DrawTool::TrendLine if pending_pt.is_some() => "Click second point (Esc to cancel)",
                    DrawTool::TrendLine => "Click first point (Esc to cancel)",
                    DrawTool::None => "",
                };
                if !hint.is_empty() {
                    egui::Area::new(egui::Id::new("tool_hint")).fixed_pos(egui::pos2(8.0, h - PB + 6.0)).show(ctx, |ui| {
                        ui.label(egui::RichText::new(hint).monospace().size(11.0).color(egui::Color32::from_rgb(255, 200, 50)));
                    });
                }
            }

            // Auto-scroll indicator
            if !auto_scroll {
                egui::Area::new(egui::Id::new("scroll_paused")).fixed_pos(egui::pos2(w - PR - 80.0, h - PB + 4.0)).show(ctx, |ui| {
                    if ui.button(egui::RichText::new("▶ LIVE").monospace().size(10.0)).clicked() {
                        resume_scroll = true;
                    }
                });
            }

            // Theme dropdown (top-right)
            egui::Area::new(egui::Id::new("theme_picker")).fixed_pos(egui::pos2(w - PR - 120.0, 2.0)).show(ctx, |ui| {
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(THEMES[self.theme_idx].name)
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for (i, theme) in THEMES.iter().enumerate() {
                            if ui.selectable_label(i == theme_idx, theme.name).clicked() {
                                new_theme_idx = Some(i);
                            }
                        }
                    });
            });
        });

        // Apply deferred actions from egui
        if let Some(idx) = new_theme_idx {
            self.theme_idx = idx;
            self.apply_theme();
        }
        if resume_scroll {
            self.auto_scroll = true;
            self.price_lock = None;
            self.vs = (self.bar_count as f32 - self.vc as f32 + RIGHT_MARGIN_BARS as f32).max(0.0);
        }
        self.egui_state.handle_platform_output(&*self.window, full_output.platform_output);

        // Render egui
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: 1.0,
        };
        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        // Render egui using egui_wgpu's built-in renderer which handles encoder internally
        let mut upload_enc = self.device.create_command_encoder(&Default::default());
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut upload_enc, &paint_jobs, &screen_descriptor);
        // Submit upload, then do render in a new submission
        self.queue.submit(std::iter::once(upload_enc.finish()));

        // Render egui pass — use forget_lifetime for wgpu 24 owned pass compatibility
        let mut render_enc = self.device.create_command_encoder(&Default::default());
        let mut egui_pass = render_enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view, resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
        }).forget_lifetime();
        self.egui_renderer.render(&mut egui_pass, &paint_jobs, &screen_descriptor);
        std::mem::drop(egui_pass);
        self.queue.submit(std::iter::once(render_enc.finish()));
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        output.present();
    }
}

// ─── winit App ────────────────────────────────────────────────────────────────

struct App {
    rx: mpsc::Receiver<ChartCommand>,
    title: String,
    iw: u32, ih: u32,
    win: Option<Arc<Window>>,
    gpu: Option<Gpu>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.win.is_some() { return; }
        eprintln!("[native-chart] Creating window {}x{}", self.iw, self.ih);
        let w = Arc::new(el.create_window(
            WindowAttributes::default()
                .with_title(&self.title)
                .with_inner_size(PhysicalSize::new(self.iw, self.ih))
                .with_active(true)
        ).expect("window"));
        eprintln!("[native-chart] Window created, initializing GPU...");
        let g = Gpu::new(Arc::clone(&w));
        eprintln!("[native-chart] GPU initialized, {:?} format", g.config.format);
        w.set_cursor(winit::window::CursorIcon::Crosshair);
        self.win = Some(w);
        self.gpu = Some(g);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        let gpu = match &mut self.gpu { Some(g) => g, None => return };

        // Feed to egui — if it consumes the event (clicked a UI widget), skip chart handling
        if let Some(win) = &self.win {
            let resp = gpu.egui_state.on_window_event(win, &ev);
            if resp.consumed {
                gpu.dirty = true;
                if let Some(w) = &self.win { w.request_redraw(); }
                return;
            }
        }

        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => gpu.resize(s.width, s.height),
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                // Handle drawing tool placement
                if gpu.draw_state.tool != DrawTool::None && gpu.mouse.right_click.is_none() {
                    let mx = gpu.mouse.cx as f32;
                    let my = gpu.mouse.cy as f32;
                    let w = gpu.config.width as f32;
                    if mx < w - PR && my >= PT && my < gpu.config.height as f32 - PB {
                        let price = gpu.y_to_price(my);
                        let bar = gpu.x_to_bar(mx);
                        match gpu.draw_state.tool {
                            DrawTool::HLine => {
                                gpu.drawings.push(Drawing {
                                    id: format!("hline-{}", gpu.drawings.len()),
                                    kind: DrawingKind::HLine { price },
                                    color: [0.4, 0.7, 1.0, 0.8], width: 1.0, dashed: true,
                                });
                                gpu.draw_state.tool = DrawTool::None;
                            }
                            DrawTool::TrendLine => {
                                if let Some((b0, p0)) = gpu.draw_state.pending_point {
                                    gpu.drawings.push(Drawing {
                                        id: format!("trend-{}", gpu.drawings.len()),
                                        kind: DrawingKind::TrendLine { price0: p0, bar0: b0, price1: price, bar1: bar },
                                        color: [0.3, 0.6, 1.0, 0.9], width: 1.0, dashed: false,
                                    });
                                    gpu.draw_state.pending_point = None;
                                    gpu.draw_state.tool = DrawTool::None;
                                } else {
                                    gpu.draw_state.pending_point = Some((bar, price));
                                }
                            }
                            DrawTool::None => {}
                        }
                        gpu.dirty = true;
                    }
                }
                // Check context menu item click
                else if let Some((cmx, cmy)) = gpu.mouse.right_click {
                    let mx = gpu.mouse.cx as f32;
                    let my = gpu.mouse.cy as f32;
                    let menu_w = 170.0_f32;
                    let item_h = 22.0_f32;
                    let items = 7;
                    if mx >= cmx && mx < cmx + menu_w && my >= cmy && my < cmy + items as f32 * item_h {
                        let idx = ((my - cmy) / item_h) as usize;
                        let click_price = gpu.y_to_price(cmy);
                        match idx {
                            0 => { // Set HLine
                                gpu.drawings.push(Drawing {
                                    id: format!("hline-{}", gpu.drawings.len()),
                                    kind: DrawingKind::HLine { price: click_price },
                                    color: [0.4, 0.7, 1.0, 0.8], width: 1.0, dashed: true,
                                });
                            }
                            1 => { // Draw Trendline
                                gpu.draw_state.tool = DrawTool::TrendLine;
                                gpu.draw_state.pending_point = None;
                            }
                            2 => { // Reset View
                                gpu.vs = (gpu.bar_count as f32 - gpu.vc as f32 + RIGHT_MARGIN_BARS as f32).max(0.0);
                                gpu.price_lock = None;
                                gpu.auto_scroll = true;
                            }
                            3 => gpu.drawings.clear(), // Clear Drawings
                            4 => { // Next Theme
                                gpu.theme_idx = (gpu.theme_idx + 1) % THEMES.len();
                                gpu.apply_theme();
                            }
                            5 => { // Delete drawing under cursor
                                if let Some((id, _)) = gpu.hit_test_drawing(cmx, cmy) {
                                    gpu.drawings.retain(|d| d.id != id);
                                }
                            }
                            6 => { // Zoom Selection
                                gpu.draw_state.zoom_selecting = true;
                                gpu.draw_state.zoom_start = (cmx, cmy);
                            }
                            _ => {}
                        }
                        gpu.mouse.right_click = None;
                        gpu.dirty = true;
                    } else {
                        // Check if clicking on a drawing to start drag/edit
                        let hit = gpu.hit_test_drawing(mx, my);
                        if let Some((id, endpoint)) = hit {
                            gpu.draw_state.dragging_id = Some(id);
                            gpu.draw_state.drag_endpoint = endpoint;
                            gpu.draw_state.drag_start_price = gpu.y_to_price(my);
                            gpu.draw_state.drag_start_bar = gpu.x_to_bar(mx);
                        }
                        gpu.mouse.right_click = None;
                        gpu.dirty = true;
                    }
                }
                gpu.mouse_down();
                // Set grab cursor during drag
                if let Some(win) = &self.win {
                    let cursor = match gpu.mouse.zone {
                        DragZone::Chart => winit::window::CursorIcon::Grabbing,
                        DragZone::XAxis => winit::window::CursorIcon::EwResize,
                        DragZone::YAxis => winit::window::CursorIcon::NsResize,
                    };
                    win.set_cursor(cursor);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                // Complete zoom selection
                if gpu.draw_state.zoom_selecting {
                    let (sx, sy) = gpu.draw_state.zoom_start;
                    let ex = gpu.mouse.cx as f32;
                    let ey = gpu.mouse.cy as f32;
                    let w = gpu.config.width as f32;
                    if (ex - sx).abs() > 10.0 && (ey - sy).abs() > 10.0 {
                        let bar_left = gpu.x_to_bar(sx.min(ex));
                        let bar_right = gpu.x_to_bar(sx.max(ex));
                        let price_top = gpu.y_to_price(sy.min(ey));
                        let price_bot = gpu.y_to_price(sy.max(ey));
                        gpu.vs = bar_left.max(0.0);
                        gpu.vc = ((bar_right - bar_left).ceil() as u32).max(5);
                        gpu.price_lock = Some((price_bot.min(price_top), price_bot.max(price_top)));
                        gpu.auto_scroll = false;
                        gpu.interaction_time = std::time::Instant::now();
                    }
                    gpu.draw_state.zoom_selecting = false;
                    gpu.dirty = true;
                }
                gpu.mouse_up();
                gpu.mouse.right_click = None;
                if let Some(win) = &self.win { win.set_cursor(winit::window::CursorIcon::Crosshair); }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Middle, .. } => {
                // Middle-click: place HLine at cursor price
                let mx = gpu.mouse.cx as f32;
                let my = gpu.mouse.cy as f32;
                let w = gpu.config.width as f32;
                let h = gpu.config.height as f32;
                if mx >= 0.0 && mx < w - PR && my >= PT && my < h - PB {
                    let price = gpu.y_to_price(my);
                    gpu.drawings.push(Drawing {
                        id: format!("hline-{}", gpu.drawings.len()),
                        kind: DrawingKind::HLine { price },
                        color: [0.4, 0.7, 1.0, 0.8], width: 1.0, dashed: true,
                    });
                    gpu.dirty = true;
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => {
                let mx = gpu.mouse.cx as f32;
                let my = gpu.mouse.cy as f32;
                let w = gpu.config.width as f32;
                let h = gpu.config.height as f32;
                if mx >= 0.0 && mx < w - PR && my >= PT && my < h - PB {
                    // Right-click in chart area — toggle context menu
                    if gpu.mouse.right_click.is_some() {
                        gpu.mouse.right_click = None;
                    } else {
                        gpu.mouse.right_click = Some((mx, my));
                    }
                    gpu.dirty = true;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                gpu.mouse_move(position.x, position.y);
                gpu.dirty = true;
                gpu.render(); // Immediate render — crosshair follows cursor, drag updates viewport
            }
            WindowEvent::CursorLeft { .. } => {
                gpu.mouse.cx = -1.0;
                gpu.mouse.cy = -1.0;
                gpu.dirty = true;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta { MouseScrollDelta::LineDelta(_, y) => y, MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0 };
                gpu.scroll(dy);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use winit::keyboard::{Key, NamedKey};
                if event.state == ElementState::Pressed {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => {
                            if gpu.draw_state.tool != DrawTool::None {
                                gpu.draw_state.tool = DrawTool::None;
                                gpu.draw_state.pending_point = None;
                                gpu.dirty = true;
                            } else if gpu.mouse.right_click.is_some() {
                                gpu.mouse.right_click = None;
                                gpu.dirty = true;
                            } else {
                                el.exit();
                            }
                        }
                        Key::Named(NamedKey::Home) => {
                            gpu.vs = (gpu.bar_count as f32 - gpu.vc as f32 + RIGHT_MARGIN_BARS as f32).max(0.0);
                            gpu.price_lock = None;
                            gpu.dirty = true;
                        }
                        Key::Character(c) if c.as_str() == "r" => {
                            gpu.vs = (gpu.bar_count as f32 - gpu.vc as f32 + RIGHT_MARGIN_BARS as f32).max(0.0);
                            gpu.price_lock = None;
                            gpu.dirty = true;
                        }
                        Key::Character(c) if c.as_str() == "+" || c.as_str() == "=" => gpu.scroll(-1.0),
                        Key::Character(c) if c.as_str() == "-" => gpu.scroll(1.0),
                        Key::Character(c) if c.as_str() == "h" => {
                            gpu.draw_state.tool = DrawTool::HLine;
                            gpu.draw_state.pending_point = None;
                            gpu.dirty = true;
                        }
                        Key::Character(c) if c.as_str() == "t" => {
                            gpu.draw_state.tool = DrawTool::TrendLine;
                            gpu.draw_state.pending_point = None;
                            gpu.dirty = true;
                        }
                        Key::Character(c) if c.as_str() == "d" => {
                            let mx = gpu.mouse.cx as f32;
                            let my = gpu.mouse.cy as f32;
                            if let Some((id, _)) = gpu.hit_test_drawing(mx, my) {
                                gpu.drawings.retain(|d| d.id != id);
                                gpu.dirty = true;
                            }
                        }
                        Key::Named(NamedKey::Delete) | Key::Named(NamedKey::Backspace) => {
                            // Delete last drawing
                            if !gpu.drawings.is_empty() {
                                gpu.drawings.pop();
                                gpu.dirty = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                while let Ok(cmd) = self.rx.try_recv() {
                    if matches!(cmd, ChartCommand::Shutdown) { el.exit(); return; }
                    gpu.process(cmd);
                }
                if gpu.dirty { gpu.render(); }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        // Resume auto-scroll after 5 seconds of no interaction
        if let Some(gpu) = &mut self.gpu {
            if !gpu.auto_scroll && gpu.interaction_time.elapsed().as_secs() >= 5 {
                gpu.auto_scroll = true;
                gpu.price_lock = None;
                // Snap to end
                let max_vs = gpu.bar_count as f32 - gpu.vc as f32 + RIGHT_MARGIN_BARS as f32;
                gpu.vs = max_vs.max(0.0);
                gpu.dirty = true;
            }
        }
        if let Some(w) = &self.win { w.request_redraw(); }
    }
}

pub fn run_render_loop(title: &str, width: u32, height: u32, rx: mpsc::Receiver<ChartCommand>) {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    let el = EventLoop::builder()
        .with_any_thread(true)
        .build()
        .expect("event loop");
    eprintln!("[native-chart] Event loop created");
    let _ = el.run_app(&mut App { rx, title: title.into(), iw: width, ih: height, win: None, gpu: None });
}
