//! GPU chart pipeline — dedicated wgpu render pass replacing egui candle and
//! indicator rendering. Runs BEFORE egui's pass so chrome composites on top.
//!
//! Phase 2: instanced candle rendering (12 verts/candle, body + wick).
//! Phase 4a: instanced thick-line rendering for single-line MA overlays
//!   (SMA, EMA, WMA, DEMA, TEMA, VWAP). Each segment is one instance, expanded
//!   to a 6-vertex quad in the vertex shader with screen-space thickness.
//!
//! Both pipelines share one ViewUniform (pan/zoom transform + theme colors)
//! and one bind group, so frame upload writes the uniform once.

use std::time::Instant;

// ── Public data types (used by pane.rs) ─────────────────────────────────────

/// Per-bar instance data uploaded to the GPU. 24 bytes; `bar_slot` is the
/// 0-based index within the visible window starting at `floor(chart.vs)`.
#[repr(C)]
pub struct CandleInstance {
    pub bar_slot: f32,
    pub open:     f32,
    pub high:     f32,
    pub low:      f32,
    pub close:    f32,
    /// bit 0: bull (close >= open)
    /// bit 1: extended hours (pre-market or after-hours) — body + wick get
    ///        their alpha multiplied by `view.eth_alpha` so ETH bars render
    ///        dimmer than regular-session candles.
    pub flags:    u32,
}

/// One segment of an indicator line. 36 bytes. The CPU side splits a polyline
/// into consecutive segments and pushes one of these per gap-free pair so NaN
/// gaps in the indicator history skip cleanly.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct LineSegment {
    pub start_slot: f32,
    pub start_val:  f32,
    pub end_slot:   f32,
    pub end_val:    f32,
    pub color:      [f32; 4],  // linear RGBA
    pub thickness:  f32,       // physical pixels — full line width
}

/// One quad of a band fill (e.g. Bollinger Bands area between upper and lower).
/// 40 bytes. CPU pushes one per consecutive bar pair where both top and bottom
/// values are non-NaN.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FillQuad {
    pub start_slot: f32,
    pub start_top:  f32,    // upper band value at start
    pub start_bot:  f32,    // lower band value at start
    pub end_slot:   f32,
    pub end_top:    f32,
    pub end_bot:    f32,
    pub color:      [f32; 4],  // linear RGBA (alpha-blended)
}

/// Chart parameters populated by `render_chart_pane` during egui's run and
/// consumed by `ChartPipeline::upload` before the render passes.
pub struct ChartRenderParams {
    pub instances:      Vec<CandleInstance>,
    pub line_segments:  Vec<LineSegment>,
    pub fill_quads:     Vec<FillQuad>,
    /// Fractional part of `chart.vs` (sub-bar pan offset, 0..1).
    pub vs_frac:    f32,
    /// `chart.vc + dynamic_pad` — total bar slots across the chart width.
    pub vc_total:   f32,
    /// Visible price range bottom.
    pub price_low:  f32,
    /// Visible price range top.
    pub price_high: f32,
    /// Chart area in window pixels: `[left, top, right, bottom]`.
    pub chart_rect: [f32; 4],
    /// Theme background color `[r, g, b, a]` (0..1) — used as the chart pass clear color
    /// so the GPU canvas matches the theme behind the candles.
    pub bg:         [f32; 4],
    /// Bull candle color `[r, g, b, a]` (0..1).
    pub bull:       [f32; 4],
    /// Bear candle color `[r, g, b, a]` (0..1).
    pub bear:       [f32; 4],
    /// Multiplier for the alpha of candles flagged as extended-hours (bit 1
    /// of `CandleInstance.flags`). 1.0 = no dim. Typical: 0.18..0.4.
    pub eth_alpha:  f32,
}

impl Default for ChartRenderParams {
    fn default() -> Self {
        Self {
            instances:     Vec::new(),
            line_segments: Vec::new(),
            fill_quads:    Vec::new(),
            vs_frac:    0.0,
            vc_total:   200.0,
            price_low:  0.0,
            price_high: 1.0,
            chart_rect: [0.0; 4],
            bg:   [0.0, 0.0, 0.0, 1.0],
            bull: [0.0, 0.78, 0.35, 1.0],
            bear: [0.88, 0.22, 0.22, 1.0],
            eth_alpha: 1.0,
        }
    }
}

// ── Internal GPU types ───────────────────────────────────────────────────────

/// Uniform block matching the WGSL `ViewUniform` struct (96 bytes, std140).
/// Shared between candle.wgsl, line.wgsl and fill.wgsl.
#[repr(C)]
struct ViewUniform {
    vs_frac:        f32,
    vc_total:       f32,
    price_low:      f32,
    price_high:     f32,
    chart_x_min:    f32,
    chart_x_max:    f32,
    chart_y_min:    f32,
    chart_y_max:    f32,
    body_half_slot: f32,
    wick_half_ndc:  f32,
    surface_w:      f32,
    surface_h:      f32,
    bull:           [f32; 4],
    bear:           [f32; 4],
    /// alpha multiplier for candles flagged extended-hours (1.0 = no dim).
    eth_alpha:      f32,
    _pad:           [f32; 3],
}

const MAX_INSTANCES:      u64 = 100_000;
const MAX_LINE_SEGMENTS:  u64 =  20_000;
const MAX_FILL_QUADS:     u64 =  10_000;

fn instances_as_bytes(s: &[CandleInstance]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            s.as_ptr() as *const u8,
            s.len() * std::mem::size_of::<CandleInstance>(),
        )
    }
}

fn segments_as_bytes(s: &[LineSegment]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            s.as_ptr() as *const u8,
            s.len() * std::mem::size_of::<LineSegment>(),
        )
    }
}

fn fills_as_bytes(s: &[FillQuad]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            s.as_ptr() as *const u8,
            s.len() * std::mem::size_of::<FillQuad>(),
        )
    }
}

fn uniform_as_bytes(u: &ViewUniform) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            u as *const ViewUniform as *const u8,
            std::mem::size_of::<ViewUniform>(),
        )
    }
}

// ── Pipeline ─────────────────────────────────────────────────────────────────

pub struct ChartPipeline {
    candle_pipeline: wgpu::RenderPipeline,
    line_pipeline:   wgpu::RenderPipeline,
    fill_pipeline:   wgpu::RenderPipeline,

    instance_buf: wgpu::Buffer,  // candle instances
    line_buf:     wgpu::Buffer,  // line segments
    fill_buf:     wgpu::Buffer,  // fill quads (band areas)
    uniform_buf:  wgpu::Buffer,  // shared view uniform
    bind_group:   wgpu::BindGroup,

    instance_count:     u32,
    line_segment_count: u32,
    fill_quad_count:    u32,

    /// Clear color for the chart render pass — set each frame from the active
    /// pane's theme bg so the GPU canvas matches the theme.
    clear_color: wgpu::Color,
    /// Scissor rect (x, y, w, h) in pixels — confines all chart draws to the
    /// active pane's chart area so partial bars / line segments don't bleed.
    scissor:     (u32, u32, u32, u32),
}

impl ChartPipeline {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let candle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("chart_pipeline.candle.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("candle.wgsl").into()),
        });
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("chart_pipeline.line.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("line.wgsl").into()),
        });
        let fill_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("chart_pipeline.fill.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fill.wgsl").into()),
        });

        // Both pipelines bind the same single uniform — one BGL covers both.
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("chart_pipeline.bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty:                 wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("chart_pipeline.instance_buf"),
            size:               MAX_INSTANCES * std::mem::size_of::<CandleInstance>() as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("chart_pipeline.line_buf"),
            size:               MAX_LINE_SEGMENTS * std::mem::size_of::<LineSegment>() as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let fill_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("chart_pipeline.fill_buf"),
            size:               MAX_FILL_QUADS * std::mem::size_of::<FillQuad>() as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("chart_pipeline.uniform_buf"),
            size:               std::mem::size_of::<ViewUniform>() as u64,
            usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("chart_pipeline.bind_group"),
            layout:  &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("chart_pipeline.layout"),
            bind_group_layouts:   &[&bgl],
            push_constant_ranges: &[],
        });

        // ── Candle pipeline ──────────────────────────────────────────────────
        let candle_attrs = [
            wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 4,  shader_location: 1, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 8,  shader_location: 2, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 12, shader_location: 3, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 16, shader_location: 4, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 20, shader_location: 5, format: wgpu::VertexFormat::Uint32  },
        ];
        let candle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("chart_pipeline.candle"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:      &candle_shader,
                entry_point: Some("vs_main"),
                buffers:     &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CandleInstance>() as u64,
                    step_mode:    wgpu::VertexStepMode::Instance,
                    attributes:   &candle_attrs,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &candle_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     surface_format,
                    // Alpha-blended so the extended-hours dim factor (carried
                    // through the shader as `out.color.a *= view.eth_alpha`)
                    // actually composites against the chart bg. Was `None`
                    // (opaque) which discarded any alpha < 1 — ETH bars
                    // appeared full-opacity, ignoring the session shading.
                    blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:    wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:  wgpu::MultisampleState::default(),
            multiview:    None,
            cache:        None,
        });

        // ── Line pipeline ────────────────────────────────────────────────────
        // Layout: 4 floats endpoints, vec4 color, 1 float thickness = 36 bytes.
        let line_attrs = [
            wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 4,  shader_location: 1, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 8,  shader_location: 2, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 12, shader_location: 3, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 16, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 5, format: wgpu::VertexFormat::Float32   },
        ];
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("chart_pipeline.line"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:      &line_shader,
                entry_point: Some("vs_main"),
                buffers:     &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LineSegment>() as u64,
                    step_mode:    wgpu::VertexStepMode::Instance,
                    attributes:   &line_attrs,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &line_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     surface_format,
                    // Lines blend on top of candles for sub-pixel AA at edges.
                    blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:    wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:  wgpu::MultisampleState::default(),
            multiview:    None,
            cache:        None,
        });

        // ── Fill pipeline (band areas under multi-line indicators) ──────────
        // Layout: 6 floats endpoints + vec4 color = 40 bytes.
        let fill_attrs = [
            wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 4,  shader_location: 1, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 8,  shader_location: 2, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 12, shader_location: 3, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 16, shader_location: 4, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 20, shader_location: 5, format: wgpu::VertexFormat::Float32   },
            wgpu::VertexAttribute { offset: 24, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
        ];
        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("chart_pipeline.fill"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:      &fill_shader,
                entry_point: Some("vs_main"),
                buffers:     &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<FillQuad>() as u64,
                    step_mode:    wgpu::VertexStepMode::Instance,
                    attributes:   &fill_attrs,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &fill_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     surface_format,
                    blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:    wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:  wgpu::MultisampleState::default(),
            multiview:    None,
            cache:        None,
        });

        eprintln!("[gpu] chart_pipeline: WGSL validated OK (phase=4b_candle+line+fill, active=true)");

        Self {
            candle_pipeline, line_pipeline, fill_pipeline,
            instance_buf, line_buf, fill_buf, uniform_buf, bind_group,
            instance_count: 0, line_segment_count: 0, fill_quad_count: 0,
            clear_color: wgpu::Color::BLACK,
            scissor: (0, 0, 0, 0),
        }
    }

    /// Upload visible candle instances + indicator line segments + view uniform
    /// for the active pane. Call after egui_ctx.run(), before render.
    ///
    /// `surface_w/h` are physical pixels; `chart_rect` inside `params` is in
    /// egui logical pixels — `pixels_per_point` bridges the two.
    pub fn upload(
        &mut self,
        queue:    &wgpu::Queue,
        params:   &ChartRenderParams,
        surface_w: f32,
        surface_h: f32,
        pixels_per_point: f32,
    ) {
        // ── Candle instances ────────────────────────────────────────────────
        let icount = params.instances.len().min(MAX_INSTANCES as usize) as u32;
        self.instance_count = icount;
        if icount > 0 {
            queue.write_buffer(
                &self.instance_buf, 0,
                instances_as_bytes(&params.instances[..icount as usize]),
            );
        }

        // ── Line segments ───────────────────────────────────────────────────
        let scount = params.line_segments.len().min(MAX_LINE_SEGMENTS as usize) as u32;
        self.line_segment_count = scount;
        if scount > 0 {
            queue.write_buffer(
                &self.line_buf, 0,
                segments_as_bytes(&params.line_segments[..scount as usize]),
            );
        }

        // ── Fill quads (band areas) ─────────────────────────────────────────
        let fcount = params.fill_quads.len().min(MAX_FILL_QUADS as usize) as u32;
        self.fill_quad_count = fcount;
        if fcount > 0 {
            queue.write_buffer(
                &self.fill_buf, 0,
                fills_as_bytes(&params.fill_quads[..fcount as usize]),
            );
        }

        // ── View uniform ────────────────────────────────────────────────────
        let price_high = if (params.price_high - params.price_low).abs() < 1e-6 {
            params.price_low + 1.0
        } else {
            params.price_high
        };

        let scale = pixels_per_point.max(0.0001);
        let [cl_lp, ct_lp, cr_lp, cb_lp] = params.chart_rect;
        let cl = cl_lp * scale;
        let ct = ct_lp * scale;
        let cr = cr_lp * scale;
        let cb = cb_lp * scale;

        let px_to_ndc_x = |px: f32| px / surface_w * 2.0 - 1.0;
        let px_to_ndc_y = |py: f32| 1.0 - py / surface_h * 2.0;
        let uniform = ViewUniform {
            vs_frac:        params.vs_frac,
            vc_total:       params.vc_total.max(1.0),
            price_low:      params.price_low,
            price_high,
            chart_x_min:    px_to_ndc_x(cl),
            chart_x_max:    px_to_ndc_x(cr),
            chart_y_min:    px_to_ndc_y(cb),
            chart_y_max:    px_to_ndc_y(ct),
            body_half_slot: 0.35,
            // Wick total width should be 1 LOGICAL pixel to match egui's
            // hw = 0.5 (logical). On HiDPI that's pixels_per_point physical
            // pixels — so half-width physical = ppp/2, which in NDC is
            // (ppp/2) / (surface_w/2) = ppp / surface_w. Previously hardcoded
            // to 1.0/surface_w which made wicks 1 physical pixel and looked
            // ~33% thinner than egui's wicks at ppp=1.5.
            wick_half_ndc:  scale / surface_w.max(1.0),
            surface_w:      surface_w.max(1.0),
            surface_h:      surface_h.max(1.0),
            bull:           params.bull,
            bear:           params.bear,
            eth_alpha:      params.eth_alpha,
            _pad:           [0.0; 3],
        };
        queue.write_buffer(&self.uniform_buf, 0, uniform_as_bytes(&uniform));

        self.clear_color = wgpu::Color {
            r: params.bg[0] as f64,
            g: params.bg[1] as f64,
            b: params.bg[2] as f64,
            a: params.bg[3] as f64,
        };

        let sw_u = surface_w.max(1.0) as u32;
        let sh_u = surface_h.max(1.0) as u32;
        let sx = (cl.max(0.0) as u32).min(sw_u);
        let sy = (ct.max(0.0) as u32).min(sh_u);
        let sw = ((cr.max(cl) as u32).min(sw_u)).saturating_sub(sx);
        let sh = ((cb.max(ct) as u32).min(sh_u)).saturating_sub(sy);
        self.scissor = (sx, sy, sw, sh);

        // Aggregated `chart_visible_bars` and `chart_pipeline_active` are
        // updated by GpuCtx::render across all panes — setting them here
        // would just overwrite each upload with the last pane's count.
    }

    /// Run a chart render pass for a single pane: maybe-clear, draw candles,
    /// draw fills, draw lines. Caller controls the load op so the multi-pane
    /// loop can clear on the first call and load on subsequent calls.
    /// All draws use the scissor rect set by the most recent `upload`.
    /// Returns CPU encode + submit time in microseconds.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue:  &wgpu::Queue,
        view:   &wgpu::TextureView,
        load_op: wgpu::LoadOp<wgpu::Color>,
    ) -> u64 {
        let t0 = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("chart_pass.encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("chart_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  load_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes:         None,
                occlusion_query_set:      None,
            });

            let have_scissor = self.scissor.2 > 0 && self.scissor.3 > 0;
            if have_scissor {
                pass.set_scissor_rect(self.scissor.0, self.scissor.1, self.scissor.2, self.scissor.3);
            }

            // Bind group is shared between candle and line pipelines.
            pass.set_bind_group(0, &self.bind_group, &[]);

            if have_scissor && self.instance_count > 0 {
                pass.set_pipeline(&self.candle_pipeline);
                pass.set_vertex_buffer(0, self.instance_buf.slice(..));
                // 12 vertices per instance: body (0..6) + wick (6..12)
                pass.draw(0..12, 0..self.instance_count);
            }

            // Fills draw between candles and lines so band tints sit on top
            // of candles but under indicator strokes.
            if have_scissor && self.fill_quad_count > 0 {
                pass.set_pipeline(&self.fill_pipeline);
                pass.set_vertex_buffer(0, self.fill_buf.slice(..));
                pass.draw(0..6, 0..self.fill_quad_count);
            }

            if have_scissor && self.line_segment_count > 0 {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, self.line_buf.slice(..));
                // 6 vertices per segment (one quad)
                pass.draw(0..6, 0..self.line_segment_count);
            }
        }
        queue.submit(std::iter::once(encoder.finish()));
        t0.elapsed().as_micros() as u64
    }
}
