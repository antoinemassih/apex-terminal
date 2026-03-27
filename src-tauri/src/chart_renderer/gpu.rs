//! Native wgpu render loop — winit window + GPU candlestick rendering.
//!
//! Runs on a dedicated thread, receives commands via mpsc channel.
//! Uses the same WGSL shaders as the WebGPU frontend.

use std::sync::{mpsc, Arc};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId, WindowAttributes},
    dpi::PhysicalSize,
};

use super::{Bar, CandleUniforms, ChartCommand};

/// Include the candle shader source at compile time (same shader used by WebGPU frontend)
const CANDLE_SHADER: &str = include_str!("../../../src/renderer/shaders/candles_gpu.wgsl");

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    candle_pipeline: wgpu::RenderPipeline,
    bar_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bgl: wgpu::BindGroupLayout,

    // Data
    bars: Vec<Bar>,
    bar_count: u32,
    bar_capacity: u32,

    // Viewport
    view_start: u32,
    view_count: u32,

    // Theme
    background: [f32; 4],
    bull_color: [f32; 4],
    bear_color: [f32; 4],
}

impl GpuState {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window).expect("Failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("No GPU adapter found");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("chart-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .expect("Failed to create GPU device");

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo, // Vsync
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1, // Minimize input lag
        };
        surface.configure(&device, &surface_config);

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("candle-shader"),
            source: wgpu::ShaderSource::Wgsl(CANDLE_SHADER.into()),
        });

        // Bind group layout — matches WebGPU frontend: storage(bars) + uniform(viewport)
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("candle-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("candle-pipeline-layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let candle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("candle-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Initial buffers
        let initial_cap: u32 = 2048;
        let bar_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bar-storage"),
            size: (initial_cap as u64) * std::mem::size_of::<Bar>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("candle-uniform"),
            size: std::mem::size_of::<CandleUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("candle-bind"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: bar_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: uniform_buffer.as_entire_binding() },
            ],
        });

        Self {
            device,
            queue,
            surface,
            surface_config,
            candle_pipeline,
            bar_buffer,
            uniform_buffer,
            bind_group,
            bgl,
            bars: Vec::new(),
            bar_count: 0,
            bar_capacity: initial_cap,
            view_start: 0,
            view_count: 200,
            background: [0.05, 0.05, 0.11, 1.0], // #0d0d1c
            bull_color: [0.033, 0.600, 0.506, 1.0],
            bear_color: [0.949, 0.212, 0.271, 1.0],
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    fn process_command(&mut self, cmd: ChartCommand) {
        match cmd {
            ChartCommand::LoadBars { bars, .. } => {
                self.bars = bars;
                self.bar_count = self.bars.len() as u32;
                self.ensure_bar_buffer();
                self.queue.write_buffer(&self.bar_buffer, 0, bytemuck::cast_slice(&self.bars));
            }
            ChartCommand::AppendBar { bar, .. } => {
                self.bars.push(bar);
                self.bar_count = self.bars.len() as u32;
                self.ensure_bar_buffer();
                let offset = (self.bar_count as u64 - 1) * std::mem::size_of::<Bar>() as u64;
                self.queue.write_buffer(&self.bar_buffer, offset, bytemuck::bytes_of(&bar));
            }
            ChartCommand::UpdateLastBar { bar, .. } => {
                if let Some(last) = self.bars.last_mut() {
                    *last = bar;
                    let offset = (self.bar_count as u64 - 1) * std::mem::size_of::<Bar>() as u64;
                    self.queue.write_buffer(&self.bar_buffer, offset, bytemuck::bytes_of(&bar));
                }
            }
            ChartCommand::SetViewport { view_start, view_count, .. } => {
                self.view_start = view_start;
                self.view_count = view_count;
            }
            ChartCommand::SetTheme { background, bull_color, bear_color } => {
                self.background = background;
                self.bull_color = bull_color;
                self.bear_color = bear_color;
            }
            ChartCommand::Resize { width, height } => self.resize(width, height),
            ChartCommand::Shutdown => {} // handled by event loop
        }
    }

    fn ensure_bar_buffer(&mut self) {
        if self.bar_count <= self.bar_capacity { return; }
        let new_cap = (self.bar_count * 2).max(2048);
        self.bar_capacity = new_cap;
        self.bar_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bar-storage"),
            size: (new_cap as u64) * std::mem::size_of::<Bar>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Rebuild bind group with new buffer
        self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("candle-bind"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.bar_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.uniform_buffer.as_entire_binding() },
            ],
        });
        // Re-upload all bars
        self.queue.write_buffer(&self.bar_buffer, 0, bytemuck::cast_slice(&self.bars));
    }

    fn render(&mut self) {
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => {
                self.surface.configure(&self.device, &self.surface_config);
                return;
            }
        };

        let view = output.texture.create_view(&Default::default());
        let w = self.surface_config.width as f32;
        let h = self.surface_config.height as f32;

        // Compute viewport uniforms — same math as TypeScript computeCs
        let padding_right: f32 = 80.0;
        let padding_top: f32 = 20.0;
        let padding_bottom: f32 = 40.0;
        let chart_width = w - padding_right;
        let right_margin_bars: u32 = 8;
        let total_bars = self.view_count + right_margin_bars;
        let step_px = (chart_width / total_bars as f32).floor().max(1.0);
        let half_step_px = (step_px / 2.0).floor();

        // Price range from visible bars
        let end = (self.view_start + self.view_count).min(self.bar_count);
        let (mut min_p, mut max_p) = (f32::MAX, f32::MIN);
        for i in self.view_start..end {
            if let Some(bar) = self.bars.get(i as usize) {
                if bar.low < min_p { min_p = bar.low; }
                if bar.high > max_p { max_p = bar.high; }
            }
        }
        if min_p >= max_p { min_p -= 0.5; max_p += 0.5; }
        let pad = (max_p - min_p) * 0.05;
        min_p -= pad;
        max_p += pad;

        let chart_height = h - padding_top - padding_bottom;
        let price_a = 1.0 - 2.0 * padding_top / h - (max_p / (max_p - min_p)) * (2.0 * chart_height / h);
        let price_b = (2.0 * chart_height / h) / (max_p - min_p);

        let uniforms = CandleUniforms {
            view_start: self.view_start,
            view_count: self.view_count.min(end - self.view_start),
            _pad0: 0,
            _pad1: 0,
            step_px,
            half_step_px,
            price_a,
            price_b,
            offset_px: 0.0,
            _pad2: 0.0,
            canvas_width: w,
            canvas_height: h,
            up_color: self.bull_color,
            down_color: self.bear_color,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("candle-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.background[0] as f64,
                            g: self.background[1] as f64,
                            b: self.background[2] as f64,
                            a: self.background[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            let draw_count = uniforms.view_count;
            if draw_count > 0 {
                pass.set_pipeline(&self.candle_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.draw(0..18, 0..draw_count); // 18 vertices per candle instance
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

/// winit application handler
struct ChartApp {
    rx: mpsc::Receiver<ChartCommand>,
    title: String,
    initial_width: u32,
    initial_height: u32,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    should_close: bool,
}

impl ApplicationHandler for ChartApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }

        let attrs = WindowAttributes::default()
            .with_title(&self.title)
            .with_inner_size(PhysicalSize::new(self.initial_width, self.initial_height));

        let window = Arc::new(event_loop.create_window(attrs).expect("Failed to create window"));
        let gpu = GpuState::new(Arc::clone(&window));
        self.window = Some(window);
        self.gpu = Some(gpu);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.should_close = true;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                // Process all pending commands before rendering
                while let Ok(cmd) = self.rx.try_recv() {
                    if matches!(cmd, ChartCommand::Shutdown) {
                        self.should_close = true;
                        event_loop.exit();
                        return;
                    }
                    if let Some(gpu) = &mut self.gpu {
                        gpu.process_command(cmd);
                    }
                }

                if let Some(gpu) = &mut self.gpu {
                    gpu.render();
                }

                // Request next frame immediately — continuous render loop
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraw to keep the render loop spinning
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// Main render loop — called from the dedicated chart renderer thread
pub fn run_render_loop(title: &str, width: u32, height: u32, rx: mpsc::Receiver<ChartCommand>) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");

    let mut app = ChartApp {
        rx,
        title: title.to_string(),
        initial_width: width,
        initial_height: height,
        window: None,
        gpu: None,
        should_close: false,
    };

    let _ = event_loop.run_app(&mut app);
}
