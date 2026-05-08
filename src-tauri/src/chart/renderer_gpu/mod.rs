//! GPU chart pipeline — replaces egui-based chart rendering with a dedicated
//! wgpu render pass. See `SPEC_GPU_CHART_REFACTOR.md`.
//!
//! Phase 1 (this commit): foundation only. The pipeline paints a magenta
//! debug rectangle over the surface to verify a second render pass can
//! interleave with egui's pass without breaking compositing. Pipeline build
//! happens unconditionally (so the WGSL compiles in CI on every build);
//! actual rendering is gated by the `gpu_chart_v2` Cargo feature.
//!
//! Phase 2+: real candle / indicator / drawing rendering. egui will draw
//! chrome on top.

use std::time::Instant;

pub struct ChartPipeline {
    pipeline: wgpu::RenderPipeline,
}

impl ChartPipeline {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("chart_pipeline.debug_rect.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("debug.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("chart_pipeline.debug_rect.layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("chart_pipeline.debug_rect"),
            layout: Some(&layout),
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
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self { pipeline }
    }

    /// Run the chart pass into the supplied surface view. Returns elapsed
    /// microseconds (CPU-side encode + submit; does not include GPU execution).
    ///
    /// `LoadOp::Load` is critical — egui's render pass has already drawn into
    /// this view, and clearing here would erase it. The chart pass paints on
    /// top of egui (Phase 1 demo); in Phase 2+ the painting order will flip.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
    ) -> u64 {
        let t0 = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("chart_pass.encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("chart_pass.debug_rect"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.draw(0..6, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
        t0.elapsed().as_micros() as u64
    }
}
