//! GPU chart pipeline — dedicated wgpu render pass replacing egui candle rendering.
//!
//! Phase 2: instanced candle rendering. The pipeline runs BEFORE egui's pass so
//! that egui chrome (grid lines, axes, crosshair, drawings) composites on top.
//!
//! Each visible bar is one instance; the vertex shader expands it to 12 vertices
//! (6 for the body quad, 6 for the wick quad). CPU uploads only the visible window
//! (~200–500 instances) on each frame; the pre-allocated instance buffer holds up
//! to 100 k bars.
//!
//! Phase 3+ will add indicators, drawings, and multi-pane support.

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
    pub flags:    u32,  // bit 0: bull (close >= open)
}

/// Chart parameters populated by `render_chart_pane` during egui's run and
/// consumed by `ChartPipeline::upload` before the render passes.
pub struct ChartRenderParams {
    pub instances:  Vec<CandleInstance>,
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
}

impl Default for ChartRenderParams {
    fn default() -> Self {
        Self {
            instances:  Vec::new(),
            vs_frac:    0.0,
            vc_total:   200.0,
            price_low:  0.0,
            price_high: 1.0,
            chart_rect: [0.0; 4],
            bg:   [0.0, 0.0, 0.0, 1.0],
            bull: [0.0, 0.78, 0.35, 1.0],
            bear: [0.88, 0.22, 0.22, 1.0],
        }
    }
}

// ── Internal GPU types ───────────────────────────────────────────────────────

/// Uniform block matching the WGSL `ViewUniform` struct (80 bytes, std140).
#[repr(C)]
struct ViewUniform {
    vs_frac:        f32,
    vc_total:       f32,
    price_low:      f32,
    price_high:     f32,
    chart_x_min:    f32,  // NDC x of chart area left
    chart_x_max:    f32,  // NDC x of chart area right
    chart_y_min:    f32,  // NDC y of chart area bottom (smaller value)
    chart_y_max:    f32,  // NDC y of chart area top (larger value)
    body_half_slot: f32,  // body half-width as fraction of one bar slot (0.35)
    wick_half_ndc:  f32,  // 1px half-width in NDC = 1.0 / surface_width
    _pad:           [f32; 2],
    bull:           [f32; 4],
    bear:           [f32; 4],
}

const MAX_INSTANCES: u64 = 100_000;

fn instances_as_bytes(s: &[CandleInstance]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            s.as_ptr() as *const u8,
            s.len() * std::mem::size_of::<CandleInstance>(),
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
    pipeline:       wgpu::RenderPipeline,
    instance_buf:   wgpu::Buffer,
    uniform_buf:    wgpu::Buffer,
    bind_group:     wgpu::BindGroup,
    instance_count: u32,
    /// Clear color for the chart render pass — set each frame from the active
    /// pane's theme bg so the GPU canvas matches the theme.
    clear_color:    wgpu::Color,
    /// Scissor rect (x, y, w, h) in pixels — confines candle draws to the active
    /// pane's chart area so partial/edge bars can't bleed into adjacent panes.
    scissor:        (u32, u32, u32, u32),
}

impl ChartPipeline {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("chart_pipeline.candle.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("candle.wgsl").into()),
        });

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

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("chart_pipeline.layout"),
            bind_group_layouts:   &[&bgl],
            push_constant_ranges: &[],
        });

        let attrs = [
            wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 4,  shader_location: 1, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 8,  shader_location: 2, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 12, shader_location: 3, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 16, shader_location: 4, format: wgpu::VertexFormat::Float32 },
            wgpu::VertexAttribute { offset: 20, shader_location: 5, format: wgpu::VertexFormat::Uint32  },
        ];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("chart_pipeline.candle"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module:      &shader,
                entry_point: Some("vs_main"),
                buffers:     &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CandleInstance>() as u64,
                    step_mode:    wgpu::VertexStepMode::Instance,
                    attributes:   &attrs,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     surface_format,
                    blend:      None,  // opaque — egui composites on top
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

        eprintln!("[gpu] chart_pipeline: WGSL validated OK (phase=2_candle, active=true)");

        Self {
            pipeline, instance_buf, uniform_buf, bind_group,
            instance_count: 0,
            clear_color: wgpu::Color::BLACK,
            scissor: (0, 0, 0, 0),
        }
    }

    /// Upload the visible bar instances and view uniform for the active pane.
    /// Must be called after `egui_ctx.run()` (params populated) and before render.
    ///
    /// `surface_w/h` are physical pixels (the wgpu surface size). `chart_rect`
    /// inside `params` is in egui logical pixels — `pixels_per_point` is the
    /// DPI scale needed to bring them onto the same scale.
    pub fn upload(
        &mut self,
        queue:    &wgpu::Queue,
        params:   &ChartRenderParams,
        surface_w: f32,
        surface_h: f32,
        pixels_per_point: f32,
    ) {
        let count = params.instances.len().min(MAX_INSTANCES as usize) as u32;
        self.instance_count = count;

        if count > 0 {
            queue.write_buffer(
                &self.instance_buf,
                0,
                instances_as_bytes(&params.instances[..count as usize]),
            );
        }

        // Guard against degenerate price ranges (e.g. no bars loaded yet).
        let price_high = if (params.price_high - params.price_low).abs() < 1e-6 {
            params.price_low + 1.0
        } else {
            params.price_high
        };

        // chart_rect is in logical pixels — scale to physical to match the surface.
        let scale = pixels_per_point.max(0.0001);
        let [cl_lp, ct_lp, cr_lp, cb_lp] = params.chart_rect;
        let cl = cl_lp * scale;
        let ct = ct_lp * scale;
        let cr = cr_lp * scale;
        let cb = cb_lp * scale;

        // Pixel → NDC helpers.  NDC Y is inverted relative to pixel Y.
        let px_to_ndc_x = |px: f32| px / surface_w * 2.0 - 1.0;
        let px_to_ndc_y = |py: f32| 1.0 - py / surface_h * 2.0;
        let uniform = ViewUniform {
            vs_frac:        params.vs_frac,
            vc_total:       params.vc_total.max(1.0),
            price_low:      params.price_low,
            price_high,
            chart_x_min:    px_to_ndc_x(cl),
            chart_x_max:    px_to_ndc_x(cr),
            chart_y_min:    px_to_ndc_y(cb),  // bottom pixel → lower NDC y
            chart_y_max:    px_to_ndc_y(ct),  // top pixel    → higher NDC y
            body_half_slot: 0.35,
            wick_half_ndc:  1.0 / surface_w.max(1.0),
            _pad:           [0.0; 2],
            bull:           params.bull,
            bear:           params.bear,
        };
        queue.write_buffer(&self.uniform_buf, 0, uniform_as_bytes(&uniform));

        // Match the chart pass clear to the theme bg so the GPU canvas blends with
        // the egui chrome painted on top (axes, grid, indicators, drawings).
        self.clear_color = wgpu::Color {
            r: params.bg[0] as f64,
            g: params.bg[1] as f64,
            b: params.bg[2] as f64,
            a: params.bg[3] as f64,
        };

        // Scissor rect in pixels — clamped to surface bounds so wgpu validation
        // never trips on a partially-offscreen chart_rect.
        let sw_u = surface_w.max(1.0) as u32;
        let sh_u = surface_h.max(1.0) as u32;
        let sx = (cl.max(0.0) as u32).min(sw_u);
        let sy = (ct.max(0.0) as u32).min(sh_u);
        let sw = ((cr.max(cl) as u32).min(sw_u)).saturating_sub(sx);
        let sh = ((cb.max(ct) as u32).min(sh_u)).saturating_sub(sy);
        self.scissor = (sx, sy, sw, sh);

        crate::monitoring::set_chart_pipeline_active(true);
        crate::monitoring::set_chart_visible_bars(count);
    }

    /// Run the chart render pass. Uses `LoadOp::Clear(BLACK)` — must run before
    /// egui's pass so that egui chrome composites on top (`LoadOp::Load`).
    ///
    /// Returns CPU-side encode+submit time in microseconds.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue:  &wgpu::Queue,
        view:   &wgpu::TextureView,
    ) -> u64 {
        let t0 = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("chart_pass.encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("chart_pass.candle"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes:         None,
                occlusion_query_set:      None,
            });

            if self.instance_count > 0 && self.scissor.2 > 0 && self.scissor.3 > 0 {
                // Scissor only applies to draws (not LoadOp::Clear), so the surface
                // still gets cleared to bg edge-to-edge while candles are confined
                // to the active pane's chart area.
                pass.set_scissor_rect(self.scissor.0, self.scissor.1, self.scissor.2, self.scissor.3);
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instance_buf.slice(..));
                // 12 vertices per instance: body (0..6) + wick (6..12)
                pass.draw(0..12, 0..self.instance_count);
            }
        }
        queue.submit(std::iter::once(encoder.finish()));
        t0.elapsed().as_micros() as u64
    }
}
