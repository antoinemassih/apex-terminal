//! GPU-blurred shadow pipeline. Real two-pass separable Gaussian on
//! an offscreen texture, composited via egui_wgpu::CallbackTrait.
//!
//! Used by `shadow::paint_shadow_gpu` for radii > 16. Below 16, the
//! stacked-rect path in shadow.rs is faster and visually equivalent.
//!
//! Pipeline (per shadow, per frame):
//!   1. Acquire two RGBA8 textures from the size-bucket pool.
//!   2. silhouette pass — fill a rounded rect into texture A using an SDF.
//!   3. blur_h pass — sample A horizontally with 13-tap Gaussian, write B.
//!   4. blur_v pass — sample B vertically, write A.
//!   5. composite pass — sample A, multiply by tint, draw inside egui's
//!      main render pass at the callback rect.
//!
//! The composite blend is premultiplied "over" so it lays down on top
//! of whatever has already been drawn under the panel.

use egui_wgpu::CallbackTrait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;

const KERNEL_TAPS: usize = 13;
const KERNEL_HALF: usize = 6;
const BUCKETS: [u32; 4] = [64, 128, 256, 512];
const MAX_OUTSTANDING: usize = 16;

static PIPELINE: OnceLock<ShadowPipeline> = OnceLock::new();

/// Surface format published by the chart renderer at startup so widgets
/// can construct callbacks without holding a wgpu handle. Stored as the
/// numeric discriminant of `wgpu::TextureFormat`; 0 means "unknown" and
/// `paint_shadow_gpu` falls back to the stacked-rect path until set.
static SURFACE_FORMAT: AtomicU32 = AtomicU32::new(0);
/// Monotonic frame id, bumped once per frame by the renderer so the
/// pipeline can recycle textures across frame boundaries.
static FRAME_ID: AtomicU64 = AtomicU64::new(0);

/// Called by the renderer once at startup with the chosen surface format.
pub fn set_surface_format(fmt: wgpu::TextureFormat) {
    // We can't bytemuck the enum reliably across wgpu versions, but the
    // safe path is to encode the format ourselves. We only support a
    // handful of formats here — if it's something exotic, we fall back.
    let code: u32 = match fmt {
        wgpu::TextureFormat::Rgba8Unorm => 1,
        wgpu::TextureFormat::Rgba8UnormSrgb => 2,
        wgpu::TextureFormat::Bgra8Unorm => 3,
        wgpu::TextureFormat::Bgra8UnormSrgb => 4,
        _ => 0,
    };
    SURFACE_FORMAT.store(code, Ordering::Relaxed);
}

fn surface_format() -> Option<wgpu::TextureFormat> {
    match SURFACE_FORMAT.load(Ordering::Relaxed) {
        1 => Some(wgpu::TextureFormat::Rgba8Unorm),
        2 => Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        3 => Some(wgpu::TextureFormat::Bgra8Unorm),
        4 => Some(wgpu::TextureFormat::Bgra8UnormSrgb),
        _ => None,
    }
}

/// Called once per frame by the renderer.
pub fn next_frame() {
    FRAME_ID.fetch_add(1, Ordering::Relaxed);
}

/// Read the current frame id (used by widgets when constructing callbacks).
pub fn current_frame_id() -> u64 {
    FRAME_ID.load(Ordering::Relaxed)
}

/// Whether the pipeline is ready to be used. Widgets check this to decide
/// between the GPU and stacked-rect paths.
pub fn is_available() -> bool {
    surface_format().is_some()
}

// ---------- Helpers ----------

/// Compute Gaussian weights for KERNEL_TAPS taps centred on 0, given sigma.
/// Returned weights sum to 1.
fn gaussian_weights(sigma: f32) -> [f32; KERNEL_TAPS] {
    let mut w = [0.0f32; KERNEL_TAPS];
    let half = KERNEL_HALF as i32;
    let two_sigma_sq = 2.0 * sigma * sigma;
    let mut sum = 0.0;
    for i in -half..=half {
        let v = (-(i as f32 * i as f32) / two_sigma_sq).exp();
        w[(i + half) as usize] = v;
        sum += v;
    }
    if sum > 0.0 {
        for v in &mut w {
            *v /= sum;
        }
    }
    w
}

fn pick_bucket(needed: u32) -> u32 {
    for b in BUCKETS {
        if needed <= b {
            return b;
        }
    }
    *BUCKETS.last().unwrap()
}

/// Treat any `Copy` POD-ish struct as a byte slice for buffer init. We only
/// use this with `#[repr(C)]` types containing f32 fields — no padding traps.
fn bytes_of<T: Copy>(value: &T) -> &[u8] {
    // SAFETY: T is `#[repr(C)]` and POD (only f32 fields). The lifetime is
    // tied to the input ref, so the slice can't outlive the value.
    unsafe {
        std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
    }
}

// ---------- Pool ----------

struct PoolTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl PoolTexture {
    fn new(device: &wgpu::Device, bucket: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("apex.shadow.pool"),
            size: wgpu::Extent3d {
                width: bucket,
                height: bucket,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            _texture: texture,
            view,
        }
    }
}

#[derive(Default)]
struct TexturePool {
    free: HashMap<u32, Vec<PoolTexture>>,
    outstanding: usize,
    total_allocs: HashMap<u32, usize>,
}

impl TexturePool {
    fn acquire(&mut self, device: &wgpu::Device, bucket: u32) -> Option<PoolTexture> {
        if self.outstanding >= MAX_OUTSTANDING {
            return None;
        }
        let entry = self.free.entry(bucket).or_default();
        let tex = if let Some(t) = entry.pop() {
            t
        } else {
            *self.total_allocs.entry(bucket).or_insert(0) += 1;
            PoolTexture::new(device, bucket)
        };
        self.outstanding += 1;
        Some(tex)
    }

    fn release(&mut self, bucket: u32, tex: PoolTexture) {
        self.free.entry(bucket).or_default().push(tex);
        if self.outstanding > 0 {
            self.outstanding -= 1;
        }
    }
}

// ---------- Uniforms (manual layout, std140-friendly) ----------
// All structs are repr(C), 16-byte aligned, fields padded to vec4.

#[repr(C)]
#[derive(Copy, Clone)]
struct BlurUniforms {
    direction: [f32; 2],
    _pad0: [f32; 2],
    weights: [[f32; 4]; 4], // 13 weights packed in 16 floats; last 3 are 0.
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SilhouetteUniforms {
    inset_min: [f32; 2],
    inset_max: [f32; 2],
    corner: f32,
    _pad: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CompositeUniforms {
    color: [f32; 4],
    uv_max: [f32; 2],
    _pad: [f32; 2],
}

// ---------- Resources stored in CallbackResources ----------

struct PreparedShadow {
    bucket: u32,
    blurred: PoolTexture, // released next frame
    composite_bg: wgpu::BindGroup,
    _composite_ubo: wgpu::Buffer,
}

struct ShadowResources {
    pool: TexturePool,
    /// Prepared this frame; consumed by paint().
    prepared: Vec<PreparedShadow>,
    /// Holdovers from the prior frame, to be reclaimed at start of next prepare.
    last_frame: Vec<PreparedShadow>,
    frame_id: u64,
}

impl ShadowResources {
    fn new() -> Self {
        Self {
            pool: TexturePool::default(),
            prepared: Vec::new(),
            last_frame: Vec::new(),
            frame_id: u64::MAX,
        }
    }

    fn maybe_advance_frame(&mut self, frame_id: u64) {
        if frame_id != self.frame_id {
            for ps in self.last_frame.drain(..) {
                self.pool.release(ps.bucket, ps.blurred);
            }
            self.last_frame = std::mem::take(&mut self.prepared);
            self.frame_id = frame_id;
        }
    }
}

// ---------- Pipeline singleton ----------

pub struct ShadowPipeline {
    silhouette: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    silhouette_bgl: wgpu::BindGroupLayout,
    blur_bgl: wgpu::BindGroupLayout,
    composite_bgl: wgpu::BindGroupLayout,
    _surface_format: wgpu::TextureFormat,
}

impl ShadowPipeline {
    pub fn get(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> &'static Self {
        PIPELINE.get_or_init(|| Self::build(device, surface_format))
    }

    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("apex.shadow.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let silhouette_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("apex.shadow.silhouette.bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let tex_samp_ubo = [
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        let blur_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("apex.shadow.blur.bgl"),
            entries: &tex_samp_ubo,
        });
        let composite_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("apex.shadow.composite.bgl"),
            entries: &tex_samp_ubo,
        });

        let silhouette_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("apex.shadow.silhouette.wgsl"),
            source: wgpu::ShaderSource::Wgsl(SILHOUETTE_WGSL.into()),
        });
        let blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("apex.shadow.blur.wgsl"),
            source: wgpu::ShaderSource::Wgsl(BLUR_WGSL.into()),
        });
        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("apex.shadow.composite.wgsl"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
        });

        let silhouette_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("apex.shadow.silhouette.pl"),
            bind_group_layouts: &[&silhouette_bgl],
            push_constant_ranges: &[],
        });
        let silhouette = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("apex.shadow.silhouette.pipeline"),
            layout: Some(&silhouette_layout),
            vertex: wgpu::VertexState {
                module: &silhouette_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &silhouette_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let blur_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("apex.shadow.blur.pl"),
            bind_group_layouts: &[&blur_bgl],
            push_constant_ranges: &[],
        });
        let blur = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("apex.shadow.blur.pipeline"),
            layout: Some(&blur_layout),
            vertex: wgpu::VertexState {
                module: &blur_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blur_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let composite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("apex.shadow.composite.pl"),
            bind_group_layouts: &[&composite_bgl],
            push_constant_ranges: &[],
        });
        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("apex.shadow.composite.pipeline"),
            layout: Some(&composite_layout),
            vertex: wgpu::VertexState {
                module: &composite_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &composite_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        // Premultiplied "over": shader emits color * alpha.
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
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
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            silhouette,
            blur,
            composite,
            sampler,
            silhouette_bgl,
            blur_bgl,
            composite_bgl,
            _surface_format: surface_format,
        }
    }
}

// ---------- Public Callback ----------

pub struct ShadowCallback {
    pub target_rect_px: [f32; 4], // x0, y0, x1, y1 in physical pixels
    pub callback_rect_px: [f32; 4],
    pub sigma: f32,
    pub color: [f32; 4],
    pub corner: f32,
    pub frame_id: u64,
    pub surface_format: wgpu::TextureFormat,
}

impl ShadowCallback {
    /// Build a `ShadowCallback` if the pipeline is initialised. Returns
    /// `None` if no surface format has been set yet (very first frame).
    pub fn try_new(
        target_rect_px: [f32; 4],
        callback_rect_px: [f32; 4],
        sigma: f32,
        color: [f32; 4],
        corner: f32,
    ) -> Option<Self> {
        let surface_format = surface_format()?;
        Some(Self {
            target_rect_px,
            callback_rect_px,
            sigma,
            color,
            corner,
            frame_id: current_frame_id(),
            surface_format,
        })
    }
}

impl CallbackTrait for ShadowCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let pipeline = ShadowPipeline::get(device, self.surface_format);

        if !callback_resources.contains::<ShadowResources>() {
            callback_resources.insert(ShadowResources::new());
        }
        let res = callback_resources.get_mut::<ShadowResources>().unwrap();
        res.maybe_advance_frame(self.frame_id);

        let cb_w = (self.callback_rect_px[2] - self.callback_rect_px[0]).ceil() as u32;
        let cb_h = (self.callback_rect_px[3] - self.callback_rect_px[1]).ceil() as u32;
        let needed = cb_w.max(cb_h).max(1);
        let bucket = pick_bucket(needed);

        let silh_tex = match res.pool.acquire(device, bucket) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let ping_tex = match res.pool.acquire(device, bucket) {
            Some(t) => t,
            None => {
                res.pool.release(bucket, silh_tex);
                return Vec::new();
            }
        };

        let bucket_f = bucket as f32;
        let inset_px = (self.sigma * 3.0).max(1.0);
        let rect_w = self.target_rect_px[2] - self.target_rect_px[0];
        let rect_h = self.target_rect_px[3] - self.target_rect_px[1];
        let inset_min = [inset_px / bucket_f, inset_px / bucket_f];
        let inset_max = [
            (inset_px + rect_w) / bucket_f,
            (inset_px + rect_h) / bucket_f,
        ];

        let silh_uniforms = SilhouetteUniforms {
            inset_min,
            inset_max,
            corner: self.corner / bucket_f,
            _pad: [0.0; 3],
        };
        let silh_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("apex.shadow.silh.ubo"),
            size: std::mem::size_of::<SilhouetteUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&silh_ubo, 0, bytes_of(&silh_uniforms));
        let silh_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("apex.shadow.silh.bg"),
            layout: &pipeline.silhouette_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: silh_ubo.as_entire_binding(),
            }],
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("apex.shadow.silhouette.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &silh_tex.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&pipeline.silhouette);
            rp.set_bind_group(0, &silh_bg, &[]);
            rp.draw(0..3, 0..1);
        }

        // ---------- Blur passes ----------
        let weights = gaussian_weights(self.sigma.max(0.5));
        let mut weights_padded = [[0.0f32; 4]; 4];
        for (i, w) in weights.iter().enumerate() {
            weights_padded[i / 4][i % 4] = *w;
        }

        let blur_h_uniforms = BlurUniforms {
            direction: [1.0 / bucket_f, 0.0],
            _pad0: [0.0; 2],
            weights: weights_padded,
        };
        let blur_h_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("apex.shadow.blur_h.ubo"),
            size: std::mem::size_of::<BlurUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&blur_h_ubo, 0, bytes_of(&blur_h_uniforms));
        let blur_h_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("apex.shadow.blur_h.bg"),
            layout: &pipeline.blur_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&silh_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: blur_h_ubo.as_entire_binding(),
                },
            ],
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("apex.shadow.blur_h.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &ping_tex.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&pipeline.blur);
            rp.set_bind_group(0, &blur_h_bg, &[]);
            rp.draw(0..3, 0..1);
        }

        let blur_v_uniforms = BlurUniforms {
            direction: [0.0, 1.0 / bucket_f],
            _pad0: [0.0; 2],
            weights: weights_padded,
        };
        let blur_v_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("apex.shadow.blur_v.ubo"),
            size: std::mem::size_of::<BlurUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&blur_v_ubo, 0, bytes_of(&blur_v_uniforms));
        let blur_v_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("apex.shadow.blur_v.bg"),
            layout: &pipeline.blur_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ping_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: blur_v_ubo.as_entire_binding(),
                },
            ],
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("apex.shadow.blur_v.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &silh_tex.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&pipeline.blur);
            rp.set_bind_group(0, &blur_v_bg, &[]);
            rp.draw(0..3, 0..1);
        }

        // ---------- Composite bind group ----------
        let composite_uniforms = CompositeUniforms {
            color: self.color,
            uv_max: [cb_w as f32 / bucket_f, cb_h as f32 / bucket_f],
            _pad: [0.0; 2],
        };
        let composite_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("apex.shadow.composite.ubo"),
            size: std::mem::size_of::<CompositeUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&composite_ubo, 0, bytes_of(&composite_uniforms));
        let composite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("apex.shadow.composite.bg"),
            layout: &pipeline.composite_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&silh_tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: composite_ubo.as_entire_binding(),
                },
            ],
        });

        // Free the ping-pong texture; keep silhouette_tex (now holds the
        // blurred result) until next frame.
        res.pool.release(bucket, ping_tex);
        res.prepared.push(PreparedShadow {
            bucket,
            blurred: silh_tex,
            composite_bg,
            _composite_ubo: composite_ubo,
        });
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::epaint::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(res) = callback_resources.get::<ShadowResources>() else {
            return;
        };
        let Some(pipeline) = PIPELINE.get() else {
            return;
        };
        // Each paint() invocation consumes the next prepared entry in
        // submission order. We track the cursor in a thread-local since we
        // only have a shared reference to resources here.
        thread_local! {
            static CURSOR: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
            static CURSOR_FRAME: std::cell::Cell<u64> = const { std::cell::Cell::new(u64::MAX) };
        }
        let frame = res.frame_id;
        let idx = CURSOR.with(|c| {
            CURSOR_FRAME.with(|cf| {
                if cf.get() != frame {
                    cf.set(frame);
                    c.set(0);
                }
            });
            let i = c.get();
            c.set(i + 1);
            i
        });
        let Some(prep) = res.prepared.get(idx) else {
            return;
        };
        render_pass.set_pipeline(&pipeline.composite);
        render_pass.set_bind_group(0, &prep.composite_bg, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

// ============================================================================
// WGSL shaders
// ============================================================================

/// Silhouette pass: draw a soft-AA'd rounded rect into the offscreen texture.
const SILHOUETTE_WGSL: &str = r#"
struct Uniforms {
    inset_min: vec2<f32>,
    inset_max: vec2<f32>,
    corner: f32,
    _pad: vec3<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Standard fullscreen-triangle: vertices (-1,-1), (3,-1), (-1,3) cover the
// viewport. UVs (0,0), (2,0), (0,2) so the visible quad has uv in [0,1].
@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[idx], 0.0, 1.0);
    out.uv  = uv[idx];
    return out;
}

fn sd_round_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r, r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - r;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let center = 0.5 * (u.inset_min + u.inset_max);
    let half_size = 0.5 * (u.inset_max - u.inset_min);
    let p = in.uv - center;
    let d = sd_round_box(p, half_size, u.corner);
    let aa = fwidth(d) + 1e-5;
    let alpha = 1.0 - smoothstep(-aa, aa, d);
    return vec4<f32>(alpha, alpha, alpha, alpha);
}
"#;

/// Two-pass separable Gaussian blur (13 taps).
const BLUR_WGSL: &str = r#"
struct Uniforms {
    direction: vec2<f32>,
    _pad0: vec2<f32>,
    weights: array<vec4<f32>, 4>,
};
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[idx], 0.0, 1.0);
    out.uv  = uv[idx];
    return out;
}

fn weight(i: i32) -> f32 {
    let row = i / 4;
    let col = i % 4;
    let v = u.weights[row];
    if (col == 0) { return v.x; }
    if (col == 1) { return v.y; }
    if (col == 2) { return v.z; }
    return v.w;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    let half_n: i32 = 6;
    for (var i: i32 = -half_n; i <= half_n; i = i + 1) {
        let off = u.direction * f32(i);
        let w = weight(i + half_n);
        color = color + textureSample(src, samp, in.uv + off) * w;
    }
    return color;
}
"#;

/// Composite pass: sample blurred silhouette at scaled UV (since the bucket
/// is power-of-2 but only the top-left `uv_max` fraction holds useful data),
/// modulate by tint, output premultiplied.
const COMPOSITE_WGSL: &str = r#"
struct Uniforms {
    color: vec4<f32>,
    uv_max: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[idx], 0.0, 1.0);
    out.uv  = uv[idx];
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv * u.uv_max;
    let s = textureSample(src, samp, uv).a;
    let a = u.color.a * s;
    return vec4<f32>(u.color.rgb * a, a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_normalised() {
        let w = gaussian_weights(4.0);
        let s: f32 = w.iter().sum();
        assert!((s - 1.0).abs() < 1e-4, "sum was {}", s);
    }

    #[test]
    fn buckets_pick_correctly() {
        assert_eq!(pick_bucket(50), 64);
        assert_eq!(pick_bucket(100), 128);
        assert_eq!(pick_bucket(200), 256);
        assert_eq!(pick_bucket(500), 512);
        assert_eq!(pick_bucket(900), 512);
    }
}
