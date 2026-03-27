//! Native wgpu render loop — winit + GPU candles + volume + grid.

use std::sync::{mpsc, Arc};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId, WindowAttributes},
    dpi::PhysicalSize,
};

use glyphon::{
    FontSystem, SwashCache, TextAtlas, TextRenderer as GlyphonRenderer,
    Cache as GlyphonCache, Viewport as GlyphonViewport,
    TextArea, TextBounds, Buffer as TextBuffer, Metrics, Attrs, Family, Color as GColor,
    Shaping, Resolution,
};

use super::{Bar, CandleUniforms, VolumeUniforms, GridVertex, ChartCommand};

const CANDLE_SHADER: &str = include_str!("../../../src/renderer/shaders/candles_gpu.wgsl");
const VOLUME_SHADER: &str = include_str!("../../../src/renderer/shaders/volume_gpu.wgsl");
const GRID_SHADER: &str = include_str!("../../../src/renderer/shaders/grid.wgsl");
const OVERLAY_SHADER: &str = include_str!("../../../src/renderer/shaders/overlay.wgsl");

const RIGHT_MARGIN_BARS: u32 = 8;
const PR: f32 = 80.0;
const PT: f32 = 20.0;
const PB: f32 = 40.0;
const MAX_GRID_VERTS: usize = 512;
const MAX_OVERLAY_LINES: usize = 128;
const OVERLAY_FLOATS_PER_LINE: usize = 12; // matches overlay.wgsl Line struct

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
}

impl Mouse {
    fn new() -> Self { Self { dragging: false, zone: DragZone::Chart, last_x: 0.0, last_y: 0.0, cx: 0.0, cy: 0.0 } }
    fn detect_zone(&self, w: f32, h: f32) -> DragZone {
        let (x, y) = (self.cx as f32, self.cy as f32);
        if x >= w - PR && y < h - PB { DragZone::YAxis }
        else if y >= h - PB { DragZone::XAxis }
        else { DragZone::Chart }
    }
}

// ─── GPU State ────────────────────────────────────────────────────────────────

struct Gpu {
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

    // Text rendering (glyphon)
    font_system: FontSystem,
    swash_cache: SwashCache,
    text_atlas: TextAtlas,
    text_renderer: GlyphonRenderer,
    glyphon_viewport: GlyphonViewport,

    mouse: Mouse,
    dirty: bool,
    grid_vert_count: u32,
    grid_cpu: Vec<GridVertex>,
}

impl Gpu {
    fn new(window: Arc<Window>) -> Self {
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

        // Text rendering
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let glyphon_cache = GlyphonCache::new(&device);
        let mut text_atlas = TextAtlas::new(&device, &queue, &glyphon_cache, fmt);
        let text_renderer = GlyphonRenderer::new(&mut text_atlas, &device, wgpu::MultisampleState::default(), None);
        let glyphon_viewport = GlyphonViewport::new(&device, &glyphon_cache);

        Self {
            device, queue, surface, config,
            candle_pl, candle_ubuf, volume_pl, volume_ubuf, grid_pl, grid_vbuf,
            bar_buf, bgl, candle_bg, volume_bg,
            overlay_pl, overlay_buf, overlay_bg, overlay_bgl,
            overlay_cpu: vec![0.0; MAX_OVERLAY_LINES * OVERLAY_FLOATS_PER_LINE],
            overlay_count: 0,
            font_system, swash_cache, text_atlas, text_renderer, glyphon_viewport,
            bars: Vec::new(), bar_count: 0, bar_cap: cap,
            vs: 0.0, vc: 200, price_lock: None,
            bg_color: [0.05, 0.05, 0.11, 1.0],
            bull: [0.15, 0.65, 0.6, 1.0], bear: [0.94, 0.33, 0.31, 1.0],
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

    fn process(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, .. } => {
                self.bars = bars;
                self.bar_count = self.bars.len() as u32;
                self.vs = (self.bar_count as f32 - self.vc as f32 + RIGHT_MARGIN_BARS as f32).max(0.0);
                self.price_lock = None;
                self.ensure_bar_buf();
                self.queue.write_buffer(&self.bar_buf, 0, bytemuck::cast_slice(&self.bars));
                self.dirty = true;
            }
            ChartCommand::AppendBar { bar, .. } => {
                self.bars.push(bar);
                self.bar_count = self.bars.len() as u32;
                self.ensure_bar_buf();
                self.queue.write_buffer(&self.bar_buf, (self.bar_count as u64 - 1) * 24, bytemuck::bytes_of(&bar));
                // Auto-scroll if near end
                let max_vs = self.bar_count as f32 - self.vc as f32 + RIGHT_MARGIN_BARS as f32;
                if self.vs >= max_vs - 2.0 { self.vs = max_vs.max(0.0); }
                self.dirty = true;
            }
            ChartCommand::UpdateLastBar { bar, .. } => {
                if let Some(last) = self.bars.last_mut() {
                    *last = bar;
                    self.queue.write_buffer(&self.bar_buf, (self.bar_count as u64 - 1) * 24, bytemuck::bytes_of(&bar));
                    self.dirty = true;
                }
            }
            ChartCommand::SetViewport { view_start, view_count, .. } => {
                self.vs = view_start as f32; self.vc = view_count; self.dirty = true;
            }
            ChartCommand::SetTheme { background, bull_color, bear_color } => {
                self.bg_color = background; self.bull = bull_color; self.bear = bear_color; self.dirty = true;
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
    }

    fn mouse_up(&mut self) { self.mouse.dragging = false; }

    fn mouse_move(&mut self, x: f64, y: f64) {
        self.mouse.cx = x; self.mouse.cy = y;
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
            // Overlay (crosshair, drawing lines) — on top of everything
            if self.overlay_count > 0 {
                pass.set_pipeline(&self.overlay_pl);
                pass.set_bind_group(0, &self.overlay_bg, &[]);
                pass.draw(0..6, 0..self.overlay_count);
            }
        }

        // ── Text rendering (price labels, OHLC, crosshair price) ──────────────
        self.glyphon_viewport.update(&self.queue, Resolution { width: w as u32, height: h as u32 });

        let mono = Attrs::new().family(Family::Monospace);
        let dim_color = GColor::rgba(160, 160, 170, 200);
        let bright_color = GColor::rgba(255, 255, 255, 255);

        // Build text buffers for all labels
        let mut text_buffers: Vec<(TextBuffer, f32, f32, GColor)> = Vec::new();

        // Price labels on right axis
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
                let txt = format!("{:.1$}", p, dec);
                let mut buf = TextBuffer::new(&mut self.font_system, Metrics::new(11.0, 13.0));
                buf.set_size(&mut self.font_system, Some(PR - 8.0), Some(14.0));
                buf.set_text(&mut self.font_system, &txt, mono.color(dim_color), Shaping::Basic);
                buf.shape_until_scroll(&mut self.font_system, false);
                text_buffers.push((buf, w - PR + 4.0, py - 6.0, dim_color));
            }
            p += price_step;
        }

        // Crosshair price label
        let mx = self.mouse.cx as f32;
        let my = self.mouse.cy as f32;
        if !self.mouse.dragging && mx >= 0.0 && mx < w - PR && my >= PT && my < h - PB {
            let ch_price = min_p + (max_p - min_p) * (1.0 - (my - PT) / ch);
            let dec = if ch_price >= 10.0 { 2usize } else { 4 };
            let txt = format!("{:.1$}", ch_price, dec);
            let mut buf = TextBuffer::new(&mut self.font_system, Metrics::new(11.0, 13.0));
            buf.set_size(&mut self.font_system, Some(PR - 8.0), Some(14.0));
            buf.set_text(&mut self.font_system, &txt, mono.color(bright_color), Shaping::Basic);
            buf.shape_until_scroll(&mut self.font_system, false);
            text_buffers.push((buf, w - PR + 4.0, my - 6.0, bright_color));
        }

        // OHLC label (top-left)
        if dc > 0 {
            if let Some(bar) = self.bars.get((end - 1) as usize) {
                let c = if bar.close >= bar.open { GColor::rgba(46, 204, 113, 220) } else { GColor::rgba(231, 76, 60, 220) };
                let txt = format!("O {:.2}  H {:.2}  L {:.2}  C {:.2}", bar.open, bar.high, bar.low, bar.close);
                let mut buf = TextBuffer::new(&mut self.font_system, Metrics::new(11.0, 13.0));
                buf.set_size(&mut self.font_system, Some(500.0), Some(14.0));
                buf.set_text(&mut self.font_system, &txt, mono.color(c), Shaping::Basic);
                buf.shape_until_scroll(&mut self.font_system, false);
                text_buffers.push((buf, 8.0, 4.0, c));
            }
        }

        // Build TextArea references
        let text_areas: Vec<TextArea> = text_buffers.iter().map(|(buf, left, top, color)| {
            TextArea {
                buffer: buf, left: *left, top: *top, scale: 1.0,
                bounds: TextBounds { left: 0, top: 0, right: w as i32, bottom: h as i32 },
                default_color: *color,
                custom_glyphs: &[],
            }
        }).collect();

        let _ = self.text_renderer.prepare(
            &self.device, &self.queue, &mut self.font_system, &mut self.text_atlas,
            &self.glyphon_viewport, text_areas, &mut self.swash_cache,
        );

        {
            let mut text_pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
            });
            let _ = self.text_renderer.render(&self.text_atlas, &self.glyphon_viewport, &mut text_pass);
        }

        self.queue.submit(std::iter::once(enc.finish()));
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
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => gpu.resize(s.width, s.height),
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
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
                gpu.mouse_up();
                if let Some(win) = &self.win { win.set_cursor(winit::window::CursorIcon::Crosshair); }
            }
            WindowEvent::CursorMoved { position, .. } => {
                gpu.mouse_move(position.x, position.y);
                gpu.dirty = true;
                gpu.render(); // Immediate render — crosshair follows cursor, drag updates viewport
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta { MouseScrollDelta::LineDelta(_, y) => y, MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0 };
                gpu.scroll(dy);
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
