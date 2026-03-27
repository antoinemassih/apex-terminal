//! Native wgpu render loop — winit window + GPU candlestick + volume rendering.
//!
//! Runs on a dedicated thread, receives commands via mpsc channel.
//! Handles mouse input directly (zero-latency pan/zoom).

use std::sync::{mpsc, Arc};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId, WindowAttributes},
    dpi::PhysicalSize,
};

use super::{Bar, CandleUniforms, ChartCommand};

const CANDLE_SHADER: &str = include_str!("../../../src/renderer/shaders/candles_gpu.wgsl");
const VOLUME_SHADER: &str = include_str!("../../../src/renderer/shaders/volume_gpu.wgsl");

const RIGHT_MARGIN_BARS: u32 = 8;
const PADDING_RIGHT: f32 = 80.0;
const PADDING_TOP: f32 = 20.0;
const PADDING_BOTTOM: f32 = 40.0;

/// Volume uniform — 80 bytes matching volume_gpu.wgsl
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct VolumeUniforms {
    view_start: u32,
    view_count: u32,
    _pad0: u32,
    _pad1: u32,
    bar_step_clip: f32,
    pixel_offset_frac: f32,
    body_width_clip: f32,
    max_volume: f32,
    vol_bottom_clip: f32,
    vol_height_clip: f32,
    _pad2: f32,
    _pad3: f32,
    up_color: [f32; 4],
    down_color: [f32; 4],
}
unsafe impl bytemuck::Pod for VolumeUniforms {}
unsafe impl bytemuck::Zeroable for VolumeUniforms {}

// ─── Mouse state ──────────────────────────────────────────────────────────────

struct MouseState {
    dragging: bool,
    drag_zone: DragZone,
    last_x: f64,
    last_y: f64,
    cursor_x: f64,
    cursor_y: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum DragZone { Chart, XAxis, YAxis }

impl MouseState {
    fn new() -> Self {
        Self { dragging: false, drag_zone: DragZone::Chart, last_x: 0.0, last_y: 0.0, cursor_x: 0.0, cursor_y: 0.0 }
    }

    fn zone(&self, w: f32, h: f32) -> DragZone {
        let x = self.cursor_x as f32;
        let y = self.cursor_y as f32;
        if x >= w - PADDING_RIGHT && y < h - PADDING_BOTTOM { DragZone::YAxis }
        else if y >= h - PADDING_BOTTOM { DragZone::XAxis }
        else { DragZone::Chart }
    }
}

// ─── GPU State ────────────────────────────────────────────────────────────────

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // Candle renderer
    candle_pipeline: wgpu::RenderPipeline,
    candle_uniform_buf: wgpu::Buffer,

    // Volume renderer
    volume_pipeline: wgpu::RenderPipeline,
    volume_uniform_buf: wgpu::Buffer,

    // Shared bar storage
    bar_buffer: wgpu::Buffer,
    bgl: wgpu::BindGroupLayout,
    candle_bind_group: wgpu::BindGroup,
    volume_bind_group: wgpu::BindGroup,

    // Data
    bars: Vec<Bar>,
    bar_count: u32,
    bar_capacity: u32,

    // Viewport — modified directly by mouse input
    view_start: f32, // float for sub-bar panning
    view_count: u32,
    price_override: Option<(f32, f32)>, // (min, max)

    // Theme
    background: [f32; 4],
    bull_color: [f32; 4],
    bear_color: [f32; 4],

    // Mouse
    mouse: MouseState,
    needs_render: bool,
}

impl GpuState {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
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
                label: Some("chart"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )).expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &surface_config);

        // Shared bind group layout (storage + uniform)
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });

        let blend = Some(wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] });

        let make_pipeline = |shader_src: &str, label: &str| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(shader_src.into()) });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label), layout: Some(&layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
                depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
            })
        };
        let candle_pipeline = make_pipeline(CANDLE_SHADER, "candle");
        let volume_pipeline = make_pipeline(VOLUME_SHADER, "volume");

        let cap: u32 = 4096;
        let bar_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bars"), size: (cap as u64) * std::mem::size_of::<Bar>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        let candle_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("candle-u"), size: std::mem::size_of::<CandleUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        let volume_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume-u"), size: std::mem::size_of::<VolumeUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });

        let candle_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: bar_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: candle_uniform_buf.as_entire_binding() },
            ],
        });
        let volume_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: bar_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: volume_uniform_buf.as_entire_binding() },
            ],
        });

        Self {
            device, queue, surface, surface_config,
            candle_pipeline, candle_uniform_buf,
            volume_pipeline, volume_uniform_buf,
            bar_buffer, bgl, candle_bind_group, volume_bind_group,
            bars: Vec::new(), bar_count: 0, bar_capacity: cap,
            view_start: 0.0, view_count: 200, price_override: None,
            background: [0.05, 0.05, 0.11, 1.0],
            bull_color: [0.033, 0.600, 0.506, 1.0],
            bear_color: [0.949, 0.212, 0.271, 1.0],
            mouse: MouseState::new(),
            needs_render: true,
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 { return; }
        self.surface_config.width = w;
        self.surface_config.height = h;
        self.surface.configure(&self.device, &self.surface_config);
        self.needs_render = true;
    }

    fn ensure_bar_buffer(&mut self) {
        if self.bar_count <= self.bar_capacity { return; }
        let new_cap = (self.bar_count * 2).max(4096);
        self.bar_capacity = new_cap;
        self.bar_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bars"), size: (new_cap as u64) * std::mem::size_of::<Bar>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        self.candle_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.bar_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.candle_uniform_buf.as_entire_binding() },
            ],
        });
        self.volume_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.bar_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.volume_uniform_buf.as_entire_binding() },
            ],
        });
        self.queue.write_buffer(&self.bar_buffer, 0, bytemuck::cast_slice(&self.bars));
    }

    fn process_command(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, .. } => {
                self.bars = bars;
                self.bar_count = self.bars.len() as u32;
                // Auto-scroll to end
                self.view_start = (self.bar_count as f32 - self.view_count as f32 + RIGHT_MARGIN_BARS as f32).max(0.0);
                self.ensure_bar_buffer();
                self.queue.write_buffer(&self.bar_buffer, 0, bytemuck::cast_slice(&self.bars));
                self.needs_render = true;
            }
            ChartCommand::AppendBar { bar, .. } => {
                self.bars.push(bar);
                self.bar_count = self.bars.len() as u32;
                self.ensure_bar_buffer();
                let off = (self.bar_count as u64 - 1) * std::mem::size_of::<Bar>() as u64;
                self.queue.write_buffer(&self.bar_buffer, off, bytemuck::bytes_of(&bar));
                self.needs_render = true;
            }
            ChartCommand::UpdateLastBar { bar, .. } => {
                if let Some(last) = self.bars.last_mut() {
                    *last = bar;
                    let off = (self.bar_count as u64 - 1) * std::mem::size_of::<Bar>() as u64;
                    self.queue.write_buffer(&self.bar_buffer, off, bytemuck::bytes_of(&bar));
                    self.needs_render = true;
                }
            }
            ChartCommand::SetViewport { view_start, view_count, .. } => {
                self.view_start = view_start as f32;
                self.view_count = view_count;
                self.needs_render = true;
            }
            ChartCommand::SetTheme { background, bull_color, bear_color } => {
                self.background = background;
                self.bull_color = bull_color;
                self.bear_color = bear_color;
                self.needs_render = true;
            }
            ChartCommand::Resize { width, height } => self.resize(width, height),
            ChartCommand::Shutdown => {}
        }
    }

    // ── Mouse interaction (immediate, zero-latency) ───────────────────────────

    fn handle_mouse_down(&mut self, w: f32, h: f32) {
        self.mouse.dragging = true;
        self.mouse.drag_zone = self.mouse.zone(w, h);
        self.mouse.last_x = self.mouse.cursor_x;
        self.mouse.last_y = self.mouse.cursor_y;
    }

    fn handle_mouse_up(&mut self) {
        self.mouse.dragging = false;
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64) {
        self.mouse.cursor_x = x;
        self.mouse.cursor_y = y;

        if !self.mouse.dragging { return; }

        let dx = x - self.mouse.last_x;
        let dy = y - self.mouse.last_y;
        self.mouse.last_x = x;
        self.mouse.last_y = y;

        let w = self.surface_config.width as f32;
        let chart_width = w - PADDING_RIGHT;
        let total_bars = self.view_count + RIGHT_MARGIN_BARS;
        let bar_step = chart_width / total_bars as f32;

        match self.mouse.drag_zone {
            DragZone::Chart => {
                let bar_delta = dx as f32 / bar_step;
                if bar_delta.abs() < 0.0001 { return; }
                let max_vs = self.bar_count as f32 - self.view_count as f32 + 200.0;
                self.view_start = (self.view_start - bar_delta).max(0.0).min(max_vs);
                self.needs_render = true;
            }
            DragZone::XAxis => {
                if dx.abs() <= 1.0 { return; }
                let factor = if dx > 0.0 { 1.05 } else { 0.95 };
                let old_vc = self.view_count;
                let new_vc = ((old_vc as f32 * factor).round() as u32).max(20).min(self.bar_count);
                if new_vc == old_vc { return; }
                let delta = (old_vc as i32 - new_vc as i32) / 2;
                self.view_count = new_vc;
                self.view_start = (self.view_start + delta as f32).max(0.0);
                self.needs_render = true;
            }
            DragZone::YAxis => {
                if dy.abs() <= 1.0 { return; }
                let factor = if dy > 0.0 { 1.05 } else { 0.95 };
                let (min_p, max_p) = self.price_range();
                let center = (min_p + max_p) / 2.0;
                let half = ((max_p - min_p) / 2.0) * factor;
                self.price_override = Some((center - half, center + half));
                self.needs_render = true;
            }
        }
    }

    fn handle_scroll(&mut self, delta: f32) {
        let factor = if delta > 0.0 { 1.1 } else { 0.9 };
        let old_vc = self.view_count;
        let new_vc = ((old_vc as f32 * factor).round() as u32).max(20).min(self.bar_count);
        if new_vc == old_vc { return; }
        let d = (old_vc as i32 - new_vc as i32) / 2;
        self.view_count = new_vc;
        self.view_start = (self.view_start + d as f32).max(0.0);
        self.price_override = None;
        self.needs_render = true;
    }

    // ── Price range ───────────────────────────────────────────────────────────

    fn price_range(&self) -> (f32, f32) {
        if let Some((min, max)) = self.price_override { return (min, max); }
        let start = self.view_start as u32;
        let end = (start + self.view_count).min(self.bar_count);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in start..end {
            if let Some(b) = self.bars.get(i as usize) {
                if b.low < lo { lo = b.low; }
                if b.high > hi { hi = b.high; }
            }
        }
        if lo >= hi { lo -= 0.5; hi += 0.5; }
        let pad = (hi - lo) * 0.05;
        (lo - pad, hi + pad)
    }

    // ── Render ────────────────────────────────────────────────────────────────

    fn render(&mut self) {
        self.needs_render = false;
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => { self.surface.configure(&self.device, &self.surface_config); return; }
        };
        let view = output.texture.create_view(&Default::default());
        let w = self.surface_config.width as f32;
        let h = self.surface_config.height as f32;

        let chart_width = w - PADDING_RIGHT;
        let chart_height = h - PADDING_TOP - PADDING_BOTTOM;
        let total_bars = self.view_count + RIGHT_MARGIN_BARS;
        let step_px = (chart_width / total_bars as f32).floor().max(1.0);
        let half_step_px = (step_px / 2.0).floor();
        let offset_frac = self.view_start - self.view_start.floor();
        let offset_px = (offset_frac * step_px).round();

        let vs = self.view_start as u32;
        let end = (vs + self.view_count).min(self.bar_count);
        let draw_count = end.saturating_sub(vs);
        if draw_count == 0 { output.present(); return; }

        let (min_p, max_p) = self.price_range();
        let price_a = 1.0 - 2.0 * PADDING_TOP / h - (max_p / (max_p - min_p)) * (2.0 * chart_height / h);
        let price_b = (2.0 * chart_height / h) / (max_p - min_p);

        // Candle uniforms
        let cu = CandleUniforms {
            view_start: vs, view_count: draw_count, _pad0: 0, _pad1: 0,
            step_px, half_step_px, price_a, price_b,
            offset_px, _pad2: 0.0, canvas_width: w, canvas_height: h,
            up_color: self.bull_color, down_color: self.bear_color,
        };
        self.queue.write_buffer(&self.candle_uniform_buf, 0, bytemuck::bytes_of(&cu));

        // Volume uniforms
        let bar_step_clip = step_px * 2.0 / w;
        let pixel_offset_frac = offset_px / step_px;
        let body_width_clip = (step_px * 0.4) * 2.0 / w; // 40% of bar width
        let mut max_vol: f32 = 0.0;
        for i in vs..end { if let Some(b) = self.bars.get(i as usize) { if b.volume > max_vol { max_vol = b.volume; } } }
        if max_vol == 0.0 { max_vol = 1.0; }

        let vu = VolumeUniforms {
            view_start: vs, view_count: draw_count, _pad0: 0, _pad1: 0,
            bar_step_clip, pixel_offset_frac, body_width_clip, max_volume: max_vol,
            vol_bottom_clip: -1.0, vol_height_clip: 0.3, _pad2: 0.0, _pad3: 0.0,
            up_color: [0.18, 0.78, 0.45, 0.25],
            down_color: [0.93, 0.27, 0.27, 0.25],
        };
        self.queue.write_buffer(&self.volume_uniform_buf, 0, bytemuck::bytes_of(&vu));

        // Encode + submit
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view, resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.background[0] as f64, g: self.background[1] as f64,
                            b: self.background[2] as f64, a: self.background[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
            });

            // Volume (behind candles)
            pass.set_pipeline(&self.volume_pipeline);
            pass.set_bind_group(0, &self.volume_bind_group, &[]);
            pass.draw(0..6, 0..draw_count);

            // Candles
            pass.set_pipeline(&self.candle_pipeline);
            pass.set_bind_group(0, &self.candle_bind_group, &[]);
            pass.draw(0..18, 0..draw_count);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

// ─── winit Application ────────────────────────────────────────────────────────

struct ChartApp {
    rx: mpsc::Receiver<ChartCommand>,
    title: String,
    initial_width: u32,
    initial_height: u32,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
}

impl ApplicationHandler for ChartApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let attrs = WindowAttributes::default()
            .with_title(&self.title)
            .with_inner_size(PhysicalSize::new(self.initial_width, self.initial_height));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        let gpu = GpuState::new(Arc::clone(&window));
        self.window = Some(window);
        self.gpu = Some(gpu);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let gpu = match &mut self.gpu { Some(g) => g, None => return };
        let w = gpu.surface_config.width as f32;
        let h = gpu.surface_config.height as f32;

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => gpu.resize(size.width, size.height),
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                gpu.handle_mouse_down(w, h);
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                gpu.handle_mouse_up();
            }
            WindowEvent::CursorMoved { position, .. } => {
                gpu.handle_mouse_move(position.x, position.y);
                // Render immediately on mouse move during drag — zero latency
                if gpu.mouse.dragging && gpu.needs_render {
                    gpu.render();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
                gpu.handle_scroll(dy);
            }
            WindowEvent::RedrawRequested => {
                // Process IPC commands
                while let Ok(cmd) = self.rx.try_recv() {
                    if matches!(cmd, ChartCommand::Shutdown) { event_loop.exit(); return; }
                    gpu.process_command(cmd);
                }
                if gpu.needs_render { gpu.render(); }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

pub fn run_render_loop(title: &str, width: u32, height: u32, rx: mpsc::Receiver<ChartCommand>) {
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = ChartApp {
        rx, title: title.to_string(), initial_width: width, initial_height: height,
        window: None, gpu: None,
    };
    let _ = event_loop.run_app(&mut app);
}
