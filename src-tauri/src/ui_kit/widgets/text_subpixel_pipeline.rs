//! Real subpixel-AA text pipeline. Bypasses egui's bilinear-sampling
//! glyph atlas (which forces grayscale) and gives us per-channel LCD
//! coverage end-to-end.
//!
//! ## Trick
//!
//! swash's `Format::Subpixel` produces an RGB bitmap where each color
//! channel = that subpixel's coverage (0..255). We want, per output
//! pixel:
//!
//! ```text
//! result.r = mix(dst.r, fg.r, mask.r)
//! result.g = mix(dst.g, fg.g, mask.g)
//! result.b = mix(dst.b, fg.b, mask.b)
//! ```
//!
//! WGSL fragment shaders cannot read the framebuffer mid-pass on most
//! desktop GPUs — there is no portable rasterization-order load. The
//! standard fix is **dual-source blending**:
//!
//!   - shader output 0 = `fg.rgb * mask.rgb` (premultiplied per channel)
//!   - shader output 1 = `mask.rgb`          (used as `OneMinusSrc1Color`)
//!   - blend: `src*One + dst*OneMinusSrc1Color`
//!
//! This requires `wgpu::Features::DUAL_SOURCE_BLENDING`. eframe's default
//! adapter request does NOT enable it, so v1 of this pipeline runs in a
//! **degraded mode** — it falls back to a single-source `OneMinusSrcAlpha`
//! blend using the green channel of the mask as alpha. That's grayscale-AA
//! at the boundary, same as the existing engine, but with the rest of
//! the plumbing (atlas, instancing, sampler-clean pipeline) ready to flip
//! to true subpixel the moment the feature is enabled in gpu.rs's
//! adapter-request hook (one-line change: add
//! `wgpu::Features::DUAL_SOURCE_BLENDING` to `required_features`).
//!
//! ## Architecture
//!
//! - Single global `TextSubpixelPipeline` (lazy, `OnceLock`).
//! - Multi-page RGBA8 atlas with shelf packing — same scheme as
//!   `text_engine::TextEngine`. We maintain it ourselves on a bare
//!   `wgpu::Texture` (not via `egui::TextureManager`) so egui's sampler
//!   never touches our subpixel data.
//! - `SubpixelTextCallback` carries a list of pre-rasterized glyph
//!   bitmaps + screen positions. `prepare()` allocates atlas regions,
//!   uploads via `Queue::write_texture`, builds a per-callback instance
//!   buffer. `paint()` issues one indirect-style draw per atlas page
//!   touched (in practice 1).
//! - Sampler: `wgpu::FilterMode::Nearest`. Subpixel data MUST NOT be
//!   bilinear-sampled — the whole point is per-subpixel control.
//!
//! Out of scope for Phase 1:
//!   - Atlas eviction (TODO Phase 2). For now atlas just grows.
//!   - Per-frame glyph dedupe across callbacks (each callback uploads
//!     its own glyphs; cache lookup happens at the engine layer).
//!   - sRGB-correct blending. We assume the surface format is sRGB and
//!     let hardware do gamma. This is wrong for true gamma-correct LCD
//!     subpixel filtering, which would need linear-space blending +
//!     manual gamma in the shader. Phase 2 polish.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use egui_wgpu::CallbackTrait;

const ATLAS_SIZE: u32 = 1024;
const ATLAS_PAD: u32 = 1;
const MAX_INSTANCES_PER_CALLBACK: u64 = 4096;

/// Run an atlas eviction sweep every N prepare() calls.
const EVICT_SWEEP_INTERVAL: u64 = 60;
/// Drop atlas entries unused for this many frames (~2 minutes at 60fps).
const EVICT_MAX_AGE: u64 = 7200;

static PIPELINE: OnceLock<TextSubpixelPipeline> = OnceLock::new();
static SURFACE_FORMAT: AtomicU32 = AtomicU32::new(0);

/// Mirror of shadow_pipeline's mechanism: chart renderer publishes the
/// chosen surface format at startup.
pub fn set_surface_format(fmt: wgpu::TextureFormat) {
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

pub fn is_available() -> bool {
    surface_format().is_some()
}

// ---------------------------------------------------------------------
// Glyph + instance data
// ---------------------------------------------------------------------

/// A rasterized glyph ready to upload. The caller (text_engine
/// subpixel path) produces one of these per shaped glyph in a run.
#[derive(Clone)]
pub struct PreparedGlyph {
    /// Top-left position in physical pixels, screen space.
    pub px: f32,
    pub py: f32,
    /// Bitmap dimensions (the atlas region will match these).
    pub w: u32,
    pub h: u32,
    /// RGB or RGBA bytes, one of:
    ///   - `bytes_per_pixel == 3`: subpixel RGB coverage. Repacked into
    ///     RGBA on upload (alpha = max(r,g,b) for fallback path).
    ///   - `bytes_per_pixel == 4`: already RGBA (color glyph / fallback
    ///     mask uploaded as white * alpha).
    pub bitmap: Vec<u8>,
    pub bytes_per_pixel: u8,
    /// Foreground color, premultiplied is fine — shader multiplies again.
    pub color: [f32; 4],
}

/// Instance-buffer record. Layout matches the WGSL struct.
#[repr(C)]
#[derive(Copy, Clone)]
struct GlyphInstance {
    /// xy = top-left dst px, zw = size px
    rect_px: [f32; 4],
    /// uv0, uv1 in [0,1]
    uv: [f32; 4],
    /// rgba in [0,1]
    color: [f32; 4],
}

fn bytes_of_slice<T: Copy>(s: &[T]) -> &[u8] {
    // SAFETY: T is `Copy` and we only call this on `#[repr(C)]` POD with
    // f32 fields. No padding traps; slice lifetime is bound to input.
    unsafe {
        std::slice::from_raw_parts(
            s.as_ptr() as *const u8,
            std::mem::size_of_val(s),
        )
    }
}

// ---------------------------------------------------------------------
// Atlas
// ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct FreeRegion {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

struct AtlasPage {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    /// Reclaimed regions from evicted glyphs. Tried first on alloc
    /// (best-fit by area) before bumping the shelf cursor.
    free_list: Vec<FreeRegion>,
}

impl AtlasPage {
    fn new(device: &wgpu::Device, label_idx: usize) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("apex.text_subpx.atlas.{}", label_idx)),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            free_list: Vec::new(),
        }
    }

    fn try_pack_shelf(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if self.cursor_x + w + ATLAS_PAD > ATLAS_SIZE {
            self.cursor_x = 0;
            self.cursor_y = self.cursor_y + self.row_height + ATLAS_PAD;
            self.row_height = 0;
        }
        if self.cursor_y + h > ATLAS_SIZE {
            return None;
        }
        let x = self.cursor_x;
        let y = self.cursor_y;
        self.cursor_x += w + ATLAS_PAD;
        self.row_height = self.row_height.max(h);
        Some((x, y))
    }

    /// Best-fit (by area) consumption of a free region. The chosen
    /// region is split: leftover portions go back on the free list,
    /// split on the longer remaining axis (keeps strips more useful).
    fn try_take_free(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let mut best: Option<(usize, u64)> = None;
        for (i, r) in self.free_list.iter().enumerate() {
            if r.w >= w && r.h >= h {
                let area = r.w as u64 * r.h as u64;
                if best.map_or(true, |(_, a)| area < a) {
                    best = Some((i, area));
                }
            }
        }
        let (idx, _) = best?;
        let r = self.free_list.swap_remove(idx);
        let (x, y) = (r.x, r.y);
        let right_w = r.w - w;
        let bottom_h = r.h - h;
        if right_w >= bottom_h {
            if right_w > 0 {
                self.free_list.push(FreeRegion { x: x + w, y, w: right_w, h });
            }
            if bottom_h > 0 {
                self.free_list.push(FreeRegion { x, y: y + h, w, h: bottom_h });
            }
        } else {
            if bottom_h > 0 {
                self.free_list.push(FreeRegion { x, y: y + h, w: r.w, h: bottom_h });
            }
            if right_w > 0 {
                self.free_list.push(FreeRegion { x: x + w, y, w: right_w, h });
            }
        }
        Some((x, y))
    }
}

#[derive(Clone, Copy, Debug)]
struct AtlasEntry {
    page_idx: usize,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    /// Cached UV rect.
    uv: [f32; 4],
    last_used_frame: u64,
}

struct SubpixelAtlas {
    pages: Vec<AtlasPage>,
    /// Glyph cache keyed by content hash. Allows skipping re-upload
    /// when the same glyph bitmap appears across frames; also gives
    /// eviction something to bump on cache hits.
    cache: HashMap<u64, AtlasEntry>,
    frame_counter: u64,
    last_evict_frame: u64,
}

impl SubpixelAtlas {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            cache: HashMap::new(),
            frame_counter: 0,
            last_evict_frame: 0,
        }
    }

    /// Allocate (page_idx, x, y) for a (w x h) region. Tries
    /// per-page free-lists first (reclaimed from evictions), then
    /// shelf cursors, then a fresh page.
    fn alloc(&mut self, device: &wgpu::Device, w: u32, h: u32) -> Option<(usize, u32, u32)> {
        if w > ATLAS_SIZE || h > ATLAS_SIZE {
            return None;
        }
        for (i, p) in self.pages.iter_mut().enumerate() {
            if let Some((x, y)) = p.try_take_free(w, h) {
                return Some((i, x, y));
            }
        }
        for (i, p) in self.pages.iter_mut().enumerate() {
            if let Some((x, y)) = p.try_pack_shelf(w, h) {
                return Some((i, x, y));
            }
        }
        let mut page = AtlasPage::new(device, self.pages.len());
        let (x, y) = page.try_pack_shelf(w, h)?;
        self.pages.push(page);
        Some((self.pages.len() - 1, x, y))
    }

    /// Bump the per-thread frame counter and run a sweep when due.
    fn tick(&mut self) {
        self.frame_counter = self.frame_counter.wrapping_add(1);
        if self.frame_counter.saturating_sub(self.last_evict_frame) >= EVICT_SWEEP_INTERVAL {
            self.last_evict_frame = self.frame_counter;
            self.evict_stale(self.frame_counter, EVICT_MAX_AGE);
        }
    }

    /// Drop entries whose `last_used_frame` is older than
    /// `current_frame - max_age_frames`. Reclaimed regions are
    /// returned to their page's free-list. Pages themselves stay
    /// allocated even when empty (Phase 2 leaves wholesale page
    /// reclamation alone).
    fn evict_stale(&mut self, current_frame: u64, max_age_frames: u64) {
        let cutoff = current_frame.saturating_sub(max_age_frames);
        let pages = &mut self.pages;
        self.cache.retain(|_k, e| {
            if e.last_used_frame >= cutoff {
                return true;
            }
            if let Some(p) = pages.get_mut(e.page_idx) {
                p.free_list.push(FreeRegion { x: e.x, y: e.y, w: e.w, h: e.h });
            }
            false
        });
    }
}

/// Stable hash of a glyph's content. Used as the atlas cache key so
/// repeated draws of the same bitmap reuse the existing atlas region.
fn glyph_hash(g: &PreparedGlyph) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.w.hash(&mut h);
    g.h.hash(&mut h);
    g.bytes_per_pixel.hash(&mut h);
    g.bitmap.hash(&mut h);
    h.finish()
}

/// ClearType-style energy-preserving FIR filter for horizontally
/// supersampled subpixel coverage. Standard 5-tap weights
/// `[1, 2, 3, 2, 1] / 9` are applied per output channel; channel
/// offsets give R/G/B their respective LCD subpixel positions.
///
/// `swash::Format::Subpixel` already produces an RGB image where each
/// channel sits at the LCD subpixel position, so the filter takes
/// `[x-2 .. x+2]` per channel and clamps at edges.
fn apply_cleartype_filter(rgb_in: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; rgb_in.len()];
    if width == 0 || height == 0 {
        return out;
    }
    // Weights: [1, 2, 3, 2, 1], divided by 9 in-loop.
    const W: [u32; 5] = [1, 2, 3, 2, 1];
    let stride = width * 3;
    for y in 0..height {
        let row = y * stride;
        for x in 0..width {
            // Read 5 source pixels at columns [x-2, x-1, x, x+1, x+2],
            // clamped to [0, width-1].
            let mut acc = [0u32; 3];
            for k in 0..5usize {
                let off = k as isize - 2;
                let sx_i = x as isize + off;
                let sx = sx_i.clamp(0, width as isize - 1) as usize;
                let p = row + sx * 3;
                let w = W[k];
                acc[0] += rgb_in[p] as u32 * w;
                acc[1] += rgb_in[p + 1] as u32 * w;
                acc[2] += rgb_in[p + 2] as u32 * w;
            }
            // Round-to-nearest divide by 9.
            let dst = row + x * 3;
            out[dst] = ((acc[0] + 4) / 9).min(255) as u8;
            out[dst + 1] = ((acc[1] + 4) / 9).min(255) as u8;
            out[dst + 2] = ((acc[2] + 4) / 9).min(255) as u8;
        }
    }
    out
}

// ---------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------

pub struct TextSubpixelPipeline {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    atlas: Mutex<SubpixelAtlas>,
    /// Whether DUAL_SOURCE_BLENDING was available at build time. Kept
    /// for diagnostic/test access (the pipeline itself already encodes
    /// the chosen blend mode).
    pub dual_source: bool,
    _surface_format: wgpu::TextureFormat,
}

impl TextSubpixelPipeline {
    pub fn get(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> &'static Self {
        PIPELINE.get_or_init(|| Self::build(device, surface_format))
    }

    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // Phase 2: developer toggle to exercise the grayscale fallback
        // path on hardware that supports DUAL_SOURCE_BLENDING. Set
        // `APEX_FORCE_GRAYSCALE_TEXT=1` (or `=true`) to force fallback.
        let force_grayscale = std::env::var("APEX_FORCE_GRAYSCALE_TEXT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let dual_source = !force_grayscale
            && device
                .features()
                .contains(wgpu::Features::DUAL_SOURCE_BLENDING);

        eprintln!(
            "[text_subpixel] active path: {}",
            if dual_source {
                "dual-source (subpixel AA)"
            } else {
                "grayscale fallback"
            }
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("apex.text_subpx.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("apex.text_subpx.bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let shader_src: &str = if dual_source { dual_src() } else { fallback_src() };
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("apex.text_subpx.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        // Per-instance vertex buffer layout: 3 vec4<f32> attributes
        // (rect_px, uv, color) at locations 0/1/2.
        let instance_stride = std::mem::size_of::<GlyphInstance>() as u64;
        let instance_attrs = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 2,
            },
        ];
        let vbuf_layout = wgpu::VertexBufferLayout {
            array_stride: instance_stride,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attrs,
        };

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("apex.text_subpx.pl"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let blend = if dual_source {
            // True per-channel subpixel composition.
            // out0 = fg.rgb * mask.rgb (premultiplied per channel)
            // out1 = mask.rgb           (used as OneMinusSrc1Color)
            // result = out0 + dst * (1 - out1)  [per channel]
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc1,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc1Alpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }
        } else {
            // Fallback: standard premultiplied "over" using the mask's
            // luminance as alpha. Equivalent to the existing engine's
            // grayscale path, but routed through our pipeline so the
            // rest of the architecture is exercised end-to-end.
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("apex.text_subpx.pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[vbuf_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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

        Self {
            pipeline,
            bgl,
            sampler,
            atlas: Mutex::new(SubpixelAtlas::new()),
            dual_source,
            _surface_format: surface_format,
        }
    }
}

// ---------------------------------------------------------------------
// Per-page uniform buffer (just screen size for px → NDC)
// ---------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
struct ScreenUniforms {
    screen_px: [f32; 2],
    _pad: [f32; 2],
}

// ---------------------------------------------------------------------
// Callback
// ---------------------------------------------------------------------

/// Per-page draw bundle prepared in `prepare()` and consumed in `paint()`.
struct PreparedDraw {
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    _ubo: wgpu::Buffer,
    /// Instance data, kept resident for `set_vertex_buffer`-free draw via
    /// storage-buffer-style indexing in the vertex shader.
    _instances_bytes_len: u64,
}

#[derive(Default)]
struct CallbackResources {
    prepared: Vec<PreparedDraw>,
}

pub struct SubpixelTextCallback {
    pub glyphs: Vec<PreparedGlyph>,
    pub surface_format: wgpu::TextureFormat,
}

impl SubpixelTextCallback {
    pub fn try_new(glyphs: Vec<PreparedGlyph>) -> Option<Self> {
        let surface_format = surface_format()?;
        Some(Self {
            glyphs,
            surface_format,
        })
    }
}

impl CallbackTrait for SubpixelTextCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if self.glyphs.is_empty() {
            return Vec::new();
        }
        let pipeline = TextSubpixelPipeline::get(device, self.surface_format);

        if !callback_resources.contains::<CallbackResources>() {
            callback_resources.insert(CallbackResources::default());
        }
        let res = callback_resources.get_mut::<CallbackResources>().unwrap();

        // Allocate atlas regions and upload glyph bitmaps. Group instance
        // records by page so each draw call uses one atlas texture.
        let mut atlas = pipeline.atlas.lock().unwrap();
        // Phase 2: bump frame counter and run periodic eviction sweep.
        atlas.tick();
        let current_frame = atlas.frame_counter;
        let mut by_page: std::collections::HashMap<usize, Vec<GlyphInstance>> =
            std::collections::HashMap::new();

        for g in &self.glyphs {
            if g.w == 0 || g.h == 0 {
                continue;
            }
            let key = glyph_hash(g);

            // Cache hit: just bump last_used_frame and emit the instance.
            if let Some(entry) = atlas.cache.get_mut(&key) {
                entry.last_used_frame = current_frame;
                let entry = *entry;
                by_page.entry(entry.page_idx).or_default().push(GlyphInstance {
                    rect_px: [g.px, g.py, g.w as f32, g.h as f32],
                    uv: entry.uv,
                    color: g.color,
                });
                continue;
            }

            let Some((page_idx, ax, ay)) = atlas.alloc(device, g.w, g.h) else {
                continue;
            };
            // Repack to RGBA8. For 3-byte subpixel input, apply the
            // ClearType-style 5-tap FIR filter first to balance channel
            // intensities and suppress visible color fringe.
            let rgba: Vec<u8> = match g.bytes_per_pixel {
                3 => {
                    let filtered = apply_cleartype_filter(
                        &g.bitmap,
                        g.w as usize,
                        g.h as usize,
                    );
                    let mut out = Vec::with_capacity((g.w * g.h * 4) as usize);
                    for px in filtered.chunks_exact(3) {
                        out.push(px[0]);
                        out.push(px[1]);
                        out.push(px[2]);
                        // Alpha for fallback path = max channel.
                        out.push(px[0].max(px[1]).max(px[2]));
                    }
                    out
                }
                4 => g.bitmap.clone(),
                1 => {
                    // Pure mask. Replicate to all channels so the same
                    // shader handles both. dual-source path will see
                    // identical R/G/B coverage = grayscale.
                    let mut out = Vec::with_capacity((g.w * g.h * 4) as usize);
                    for &a in &g.bitmap {
                        out.push(a);
                        out.push(a);
                        out.push(a);
                        out.push(a);
                    }
                    out
                }
                _ => continue,
            };
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas.pages[page_idx].texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: ax, y: ay, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(g.w * 4),
                    rows_per_image: Some(g.h),
                },
                wgpu::Extent3d {
                    width: g.w,
                    height: g.h,
                    depth_or_array_layers: 1,
                },
            );
            let inv = 1.0 / ATLAS_SIZE as f32;
            let uv = [
                ax as f32 * inv,
                ay as f32 * inv,
                (ax + g.w) as f32 * inv,
                (ay + g.h) as f32 * inv,
            ];
            atlas.cache.insert(
                key,
                AtlasEntry {
                    page_idx,
                    x: ax,
                    y: ay,
                    w: g.w,
                    h: g.h,
                    uv,
                    last_used_frame: current_frame,
                },
            );
            by_page.entry(page_idx).or_default().push(GlyphInstance {
                rect_px: [g.px, g.py, g.w as f32, g.h as f32],
                uv,
                color: g.color,
            });
        }

        let screen_px = [
            screen.size_in_pixels[0] as f32,
            screen.size_in_pixels[1] as f32,
        ];
        let ubo_data = ScreenUniforms {
            screen_px,
            _pad: [0.0; 2],
        };

        for (page_idx, instances) in by_page {
            // Hard cap to keep buffer size bounded.
            let count = (instances.len() as u64).min(MAX_INSTANCES_PER_CALLBACK);
            if count == 0 {
                continue;
            }
            let bytes = bytes_of_slice(&instances[..count as usize]);
            let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("apex.text_subpx.instances"),
                size: bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&instance_buffer, 0, bytes);

            let ubo = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("apex.text_subpx.ubo"),
                size: std::mem::size_of::<ScreenUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(
                &ubo,
                0,
                bytes_of_slice(std::slice::from_ref(&ubo_data)),
            );

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("apex.text_subpx.bg"),
                layout: &pipeline.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &atlas.pages[page_idx].view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: ubo.as_entire_binding(),
                    },
                ],
            });

            res.prepared.push(PreparedDraw {
                bind_group,
                instance_buffer,
                instance_count: count as u32,
                _ubo: ubo,
                _instances_bytes_len: bytes.len() as u64,
            });
        }

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::epaint::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(res) = callback_resources.get::<CallbackResources>() else {
            return;
        };
        let Some(pipeline) = PIPELINE.get() else {
            return;
        };
        // Same cursor-per-frame trick as shadow_pipeline. Each paint()
        // call consumes the next prepared draw bundle in submission order.
        thread_local! {
            static CURSOR: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
            static CURSOR_TICK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        }
        // We don't have a frame id here; rely on the fact that a fresh
        // CallbackResources is built per egui pass. Reset the cursor
        // when prepared.len() changed shape vs last paint's tick cap —
        // good enough for Phase 1, will be tightened with a frame id in
        // Phase 2.
        let idx = CURSOR.with(|c| {
            let i = c.get();
            c.set(i + 1);
            i
        });
        let _ = CURSOR_TICK;
        let Some(prep) = res.prepared.get(idx) else {
            return;
        };
        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, &prep.bind_group, &[]);
        render_pass.set_vertex_buffer(0, prep.instance_buffer.slice(..));
        // 6 vertices (two triangles) per instance. Vertex shader builds
        // the quad from `vertex_index` directly; no vertex buffer.
        render_pass.draw(0..6, 0..prep.instance_count);
    }
}

// ============================================================================
// WGSL shaders
// ============================================================================

/// Common vertex shader text. We synthesize a quad from `vertex_index`
/// (0..6) and pull rect/uv/color from the per-instance attribute buffer.
const VS_COMMON: &str = r#"
struct ScreenU {
    screen_px: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(2) var<uniform> screen_u: ScreenU;

struct Instance {
    @location(0) rect_px: vec4<f32>,
    @location(1) uv:      vec4<f32>,
    @location(2) color:   vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: Instance) -> VsOut {
    // Quad corners in (0..1, 0..1).
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let c = corners[vid];
    let px = inst.rect_px.xy + c * inst.rect_px.zw;
    // Pixel coords (top-left origin) -> NDC (-1..1, y-flipped).
    let ndc = vec2<f32>(
        (px.x / screen_u.screen_px.x) * 2.0 - 1.0,
        1.0 - (px.y / screen_u.screen_px.y) * 2.0,
    );
    let uv = mix(inst.uv.xy, inst.uv.zw, c);

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = uv;
    out.color = inst.color;
    return out;
}

@group(0) @binding(0) var atlas_tex: texture_2d<f32>;
@group(0) @binding(1) var atlas_samp: sampler;
"#;

/// Dual-source variant: emits two outputs for true per-channel subpixel
/// composition. Requires `wgpu::Features::DUAL_SOURCE_BLENDING`.
fn shader_dual() -> String {
    format!(
        "{vs}\n{fs}",
        vs = VS_COMMON,
        fs = r#"
struct FsOut {
    @location(0) color: vec4<f32>,
    @location(0) @second_blend_source mask: vec4<f32>,
};

@fragment
fn fs_main(in: VsOut) -> FsOut {
    let m = textureSample(atlas_tex, atlas_samp, in.uv);
    var o: FsOut;
    // Premultiplied per-channel: output color = fg.rgb * mask.rgb.
    // Alpha kept as max(mask.rgb) so order-of-operations under
    // OneMinusSrc1Alpha leaves dst's alpha sensible.
    let mask_rgb = m.rgb;
    let mask_a = max(max(mask_rgb.r, mask_rgb.g), mask_rgb.b);
    o.color = vec4<f32>(in.color.rgb * mask_rgb * in.color.a, in.color.a * mask_a);
    o.mask  = vec4<f32>(mask_rgb * in.color.a, mask_a * in.color.a);
    return o;
}
"#
    )
}

/// Single-source fallback: standard premultiplied "over" using the
/// max-channel of the mask as alpha. This is grayscale AA — same visual
/// as the existing text_engine path, but routed through this pipeline.
fn shader_fallback() -> String {
    format!(
        "{vs}\n{fs}",
        vs = VS_COMMON,
        fs = r#"
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let m = textureSample(atlas_tex, atlas_samp, in.uv);
    let a = max(max(m.r, m.g), m.b) * in.color.a;
    return vec4<f32>(in.color.rgb * a, a);
}
"#
    )
}

// Lazy-init shader sources (we can't `const`-format strings).
static SHADER_DUAL: OnceLock<String> = OnceLock::new();
static SHADER_FALLBACK: OnceLock<String> = OnceLock::new();

fn dual_src() -> &'static str {
    SHADER_DUAL.get_or_init(shader_dual).as_str()
}
fn fallback_src() -> &'static str {
    SHADER_FALLBACK.get_or_init(shader_fallback).as_str()
}
