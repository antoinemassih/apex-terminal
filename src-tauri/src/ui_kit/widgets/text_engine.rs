//! Cosmic-text rasterization pipeline. Owns a global FontSystem +
//! SwashCache + an egui-managed glyph atlas. Used by `PolishedLabel`.
//!
//! Lifecycle: lazy-init on first request. Atlas grows on demand;
//! glyphs are evicted via an LRU sweep that runs every 60 frames and
//! reclaims regions unused for the last ~2 minutes. Reclaimed regions
//! enter a per-page free-list and get re-used on next allocation
//! before the shelf cursor advances. Pages are not defragmented; if
//! a page's free-list gets fragmented and new glyphs don't fit, a
//! fresh page is allocated.
//!
//! Approach: option 1 from `docs/COSMIC_TEXT_SWAP_PLAN.md`. We hand
//! egui a managed RGBA texture and emit a `Mesh` UV-mapped to atlas
//! regions. This means glyph bitmaps still pass through egui's
//! sampler/atlas pipeline, so subpixel AA degrades to grayscale at
//! that boundary. We still get cosmic-text's *shaping* improvements
//! (real ligatures, kerning, BiDi) on top.
//!
//! cosmic-text 0.12's `SwashCache::get_image_uncached` always renders
//! with `Format::Alpha` (8-bit mask). For real subpixel we'd need to
//! call swash directly with `Format::Subpixel`, which requires
//! patching cosmic-text or duplicating its `swash_image` helper. The
//! plan doc explicitly accepts the grayscale-at-boundary tradeoff for
//! v1.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use cosmic_text::{Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use egui::epaint::{ColorImage, ImageDelta, Mesh};
use egui::{Color32, TextureId, TextureOptions};

const ATLAS_SIZE: usize = 512;
const ATLAS_PAD: u32 = 1;

/// Sweep eviction every N frames.
const EVICT_SWEEP_INTERVAL: u64 = 60;
/// Keep glyphs touched within the last N frames (~2 min @ 60 fps).
const EVICT_MAX_AGE: u64 = 7200;

/// Active proportional font idx, mirroring `Watchlist::font_idx`. The
/// renderer should bump this each frame via `set_active_font_idx`.
static ACTIVE_FONT_IDX: AtomicUsize = AtomicUsize::new(0);

/// Set the active font_idx so subsequent shape_and_render calls use
/// the matching font as the primary. Called by the renderer at the
/// top of the frame from gpu.rs before any PolishedLabel renders.
pub fn set_active_font_idx(idx: usize) {
    ACTIVE_FONT_IDX.store(idx, Ordering::Relaxed);
}

/// Internal: maps font_idx → cosmic_text::Family. Falls back to
/// SansSerif on unknown idx.
fn family_for_idx(idx: usize) -> Family<'static> {
    match idx {
        0 => Family::Name("JetBrains Mono"),
        1 => Family::Name("Inter"),
        2 => Family::Name("Plus Jakarta Sans"),
        3 => Family::Name("Space Grotesk"),
        4 => Family::Name("DM Sans"),
        5 => Family::Name("Geist"),
        _ => Family::SansSerif,
    }
}

#[derive(Clone, Copy, Debug)]
struct AtlasEntry {
    page_idx: usize,
    /// UV rect in normalized [0,1] coords (u0, v0, u1, v1).
    uv_rect: [f32; 4],
    /// Pixel offset from the glyph origin to the bitmap's top-left,
    /// matching `swash::Image::placement.{left, top}`.
    bearing: [f32; 2],
    pixel_size: [u32; 2],
    /// Pixel position within its page — kept so eviction can return
    /// the region to the page's free-list.
    page_xy: [u32; 2],
    /// Frame number of the last cache hit (or insertion).
    last_used_frame: u64,
}

#[derive(Clone, Copy, Debug)]
struct FreeRegion {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

struct AtlasPage {
    tex_id: TextureId,
    /// Shelf-packing cursor: current row's left edge and top edge,
    /// plus the row height.
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    /// Reclaimed regions from evicted glyphs. Tried first on alloc
    /// (best-fit by area) before bumping the shelf cursor.
    free_list: Vec<FreeRegion>,
}

pub struct TextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
    pages: Vec<AtlasPage>,
    cache: HashMap<CacheKey, Option<AtlasEntry>>,
    fonts_loaded: bool,
    frame_counter: u64,
    last_evict_frame: u64,
}

impl TextEngine {
    fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            pages: Vec::new(),
            cache: HashMap::new(),
            fonts_loaded: false,
            frame_counter: 0,
            last_evict_frame: 0,
        }
    }

    /// Register the same TTFs egui's font stack uses so cosmic-text
    /// can resolve `Family::SansSerif` / `Family::Monospace` to our
    /// shipped fonts even when the OS lacks Inter / JetBrains Mono.
    fn ensure_fonts(&mut self) {
        if self.fonts_loaded {
            return;
        }
        let db = self.font_system.db_mut();
        // Bytes are baked into the binary by the same `include_bytes!`
        // calls used in `ui_kit::icons::init_fonts`.
        db.load_font_data(include_bytes!("../JetBrainsMono-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../Inter-Medium.ttf").to_vec());
        db.load_font_data(include_bytes!("../PlusJakartaSans-Medium.ttf").to_vec());
        db.load_font_data(include_bytes!("../SpaceGrotesk-Medium.ttf").to_vec());
        db.load_font_data(include_bytes!("../DMSans-Medium.ttf").to_vec());
        db.load_font_data(include_bytes!("../Geist-Medium.ttf").to_vec());
        db.load_font_data(include_bytes!("../SourceSerif4-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../SourceSerif4-Bold.ttf").to_vec());
        self.fonts_loaded = true;
    }

    fn alloc_page(&mut self, ctx: &egui::Context) -> usize {
        // Allocate a blank RGBA8 atlas page and register it with egui.
        let blank = ColorImage::new([ATLAS_SIZE, ATLAS_SIZE], Color32::TRANSPARENT);
        let tex_mgr = ctx.tex_manager();
        let mut tm = tex_mgr.write();
        let tex_id = tm.alloc(
            format!("polished_label_atlas_{}", self.pages.len()),
            blank.into(),
            TextureOptions::LINEAR,
        );
        drop(tm);
        self.pages.push(AtlasPage {
            tex_id,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            free_list: Vec::new(),
        });
        self.pages.len() - 1
    }

    /// Try to allocate a (w x h) region in some existing page or a
    /// fresh page. Returns (page_idx, x, y).
    fn pack(&mut self, ctx: &egui::Context, w: u32, h: u32) -> Option<(usize, u32, u32)> {
        if w as usize > ATLAS_SIZE || h as usize > ATLAS_SIZE {
            return None;
        }
        // Try free-lists across existing pages first (most-recent first).
        for idx in (0..self.pages.len()).rev() {
            if let Some(pos) = Self::try_take_free(&mut self.pages[idx], w, h) {
                return Some((idx, pos.0, pos.1));
            }
        }
        // Then try the shelf cursor of each page.
        for idx in (0..self.pages.len()).rev() {
            if let Some(pos) = Self::try_pack_page(&mut self.pages[idx], w, h) {
                return Some((idx, pos.0, pos.1));
            }
        }
        // No page had room — allocate a new one.
        let idx = self.alloc_page(ctx);
        let pos = Self::try_pack_page(&mut self.pages[idx], w, h)?;
        Some((idx, pos.0, pos.1))
    }

    /// Best-fit (by area) consumption of a free region. The chosen
    /// region is split: we keep whatever's left after taking the
    /// (w x h) chunk from its top-left back on the free list, split
    /// on the longer remaining axis.
    fn try_take_free(page: &mut AtlasPage, w: u32, h: u32) -> Option<(u32, u32)> {
        let mut best: Option<(usize, u64)> = None;
        for (i, r) in page.free_list.iter().enumerate() {
            if r.w >= w && r.h >= h {
                let area = r.w as u64 * r.h as u64;
                if best.map_or(true, |(_, a)| area < a) {
                    best = Some((i, area));
                }
            }
        }
        let (idx, _) = best?;
        let r = page.free_list.swap_remove(idx);
        let (x, y) = (r.x, r.y);
        // Split the leftover. Choose the split that yields the larger
        // remaining rectangle (keeps strips more useful).
        let right_w = r.w - w;
        let bottom_h = r.h - h;
        if right_w >= bottom_h {
            if right_w > 0 {
                page.free_list.push(FreeRegion { x: x + w, y, w: right_w, h });
            }
            if bottom_h > 0 {
                page.free_list.push(FreeRegion { x, y: y + h, w, h: bottom_h });
            }
        } else {
            if bottom_h > 0 {
                page.free_list.push(FreeRegion { x, y: y + h, w: r.w, h: bottom_h });
            }
            if right_w > 0 {
                page.free_list.push(FreeRegion { x: x + w, y, w: right_w, h });
            }
        }
        Some((x, y))
    }

    fn try_pack_page(page: &mut AtlasPage, w: u32, h: u32) -> Option<(u32, u32)> {
        let aw = ATLAS_SIZE as u32;
        let ah = ATLAS_SIZE as u32;
        // Does it fit in the current row?
        if page.cursor_x + w + ATLAS_PAD > aw {
            // New row.
            page.cursor_x = 0;
            page.cursor_y = page.cursor_y + page.row_height + ATLAS_PAD;
            page.row_height = 0;
        }
        if page.cursor_y + h > ah {
            return None;
        }
        let x = page.cursor_x;
        let y = page.cursor_y;
        page.cursor_x += w + ATLAS_PAD;
        page.row_height = page.row_height.max(h);
        Some((x, y))
    }

    /// Upload a swash mask (one byte of alpha per pixel) into the
    /// given atlas page region as RGBA8 (white with alpha = mask).
    fn upload_mask(
        &mut self,
        ctx: &egui::Context,
        page_idx: usize,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        mask: &[u8],
    ) {
        let mut rgba = Vec::with_capacity((w * h) as usize * 4);
        for &a in mask.iter() {
            rgba.push(255);
            rgba.push(255);
            rgba.push(255);
            rgba.push(a);
        }
        let img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let delta = ImageDelta::partial([x as usize, y as usize], img, TextureOptions::LINEAR);
        let tex_id = self.pages[page_idx].tex_id;
        let tex_mgr = ctx.tex_manager();
        let mut tm = tex_mgr.write();
        tm.set(tex_id, delta);
    }

    /// Upload a colored bitmap (RGBA8) — used for color emoji.
    fn upload_color(
        &mut self,
        ctx: &egui::Context,
        page_idx: usize,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        rgba: &[u8],
    ) {
        let img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba);
        let delta = ImageDelta::partial([x as usize, y as usize], img, TextureOptions::LINEAR);
        let tex_id = self.pages[page_idx].tex_id;
        let tex_mgr = ctx.tex_manager();
        let mut tm = tex_mgr.write();
        tm.set(tex_id, delta);
    }

    fn ensure_glyph(&mut self, ctx: &egui::Context, key: CacheKey) -> Option<AtlasEntry> {
        let frame = self.frame_counter;
        if let Some(entry) = self.cache.get_mut(&key) {
            if let Some(e) = entry.as_mut() {
                e.last_used_frame = frame;
                return Some(*e);
            }
            return None;
        }
        let img = self.swash_cache.get_image_uncached(&mut self.font_system, key);
        let entry = match img {
            Some(image) if image.placement.width > 0 && image.placement.height > 0 => {
                let w = image.placement.width;
                let h = image.placement.height;
                let (page_idx, ax, ay) = self.pack(ctx, w, h)?;
                match image.content {
                    cosmic_text::SwashContent::Mask => {
                        self.upload_mask(ctx, page_idx, ax, ay, w, h, &image.data);
                    }
                    cosmic_text::SwashContent::Color => {
                        self.upload_color(ctx, page_idx, ax, ay, w, h, &image.data);
                    }
                    // SubpixelMask not handled by cosmic-text 0.12 either; fall back.
                    cosmic_text::SwashContent::SubpixelMask => return None,
                }
                let inv = 1.0 / ATLAS_SIZE as f32;
                let entry = AtlasEntry {
                    page_idx,
                    uv_rect: [
                        ax as f32 * inv,
                        ay as f32 * inv,
                        (ax + w) as f32 * inv,
                        (ay + h) as f32 * inv,
                    ],
                    bearing: [image.placement.left as f32, image.placement.top as f32],
                    pixel_size: [w, h],
                    page_xy: [ax, ay],
                    last_used_frame: frame,
                };
                Some(entry)
            }
            // Whitespace glyph or rasterization failure — record None
            // so we don't retry every frame, but no draw needed.
            _ => None,
        };
        self.cache.insert(key, entry);
        entry
    }

    /// Evict glyphs whose `last_used_frame` is older than
    /// `current_frame - max_age_frames`. Reclaimed regions are pushed
    /// onto each page's free-list for re-use.
    pub fn evict_stale(&mut self, current_frame: u64, max_age_frames: u64) {
        let cutoff = current_frame.saturating_sub(max_age_frames);
        let pages = &mut self.pages;
        self.cache.retain(|_, entry| {
            let Some(e) = entry else { return true };
            if e.last_used_frame >= cutoff {
                return true;
            }
            if let Some(page) = pages.get_mut(e.page_idx) {
                page.free_list.push(FreeRegion {
                    x: e.page_xy[0],
                    y: e.page_xy[1],
                    w: e.pixel_size[0],
                    h: e.pixel_size[1],
                });
            }
            false
        });
    }

    /// Shape `text` and return one `Mesh` per atlas page touched.
    /// `origin` is the top-left where the first line baseline should
    /// be offset down by the line height.
    pub fn shape_and_render(
        &mut self,
        ctx: &egui::Context,
        origin: egui::Pos2,
        text: &str,
        size_pt: f32,
        family: Family,
        weight: Weight,
        color: Color32,
    ) -> Option<(Vec<Mesh>, egui::Vec2)> {
        // Bump frame counter and run periodic eviction sweep.
        self.frame_counter = self.frame_counter.wrapping_add(1);
        if self.frame_counter.saturating_sub(self.last_evict_frame) >= EVICT_SWEEP_INTERVAL {
            self.last_evict_frame = self.frame_counter;
            self.evict_stale(self.frame_counter, EVICT_MAX_AGE);
        }

        if text.is_empty() {
            return Some((Vec::new(), egui::vec2(0.0, size_pt * 1.2)));
        }
        self.ensure_fonts();

        // Resolve the "use whatever the user picked" sentinel
        // (Family::SansSerif) to the active font_idx mapping.
        let resolved_family = match family {
            Family::SansSerif => family_for_idx(ACTIVE_FONT_IDX.load(Ordering::Relaxed)),
            other => other,
        };

        let metrics = Metrics::new(size_pt, size_pt * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs::new().family(resolved_family).weight(weight);
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut meshes_by_page: HashMap<usize, Mesh> = HashMap::new();
        let mut max_w = 0.0_f32;
        let mut max_y = 0.0_f32;

        // Collect physical glyphs first (avoid borrow conflict with
        // self.swash_cache during glyph upload).
        let scale = 1.0_f32; // hi-DPI handled by egui's pixels_per_point separately.
        let runs: Vec<(f32, Vec<cosmic_text::PhysicalGlyph>)> = buffer
            .layout_runs()
            .map(|run| {
                max_w = max_w.max(run.line_w);
                max_y = max_y.max(run.line_y + metrics.line_height * 0.25);
                let glyphs: Vec<_> = run
                    .glyphs
                    .iter()
                    .map(|g| g.physical((0.0, 0.0), scale))
                    .collect();
                (run.line_y, glyphs)
            })
            .collect();

        for (line_y, glyphs) in runs {
            for pg in glyphs {
                let entry = match self.ensure_glyph(ctx, pg.cache_key) {
                    Some(e) => e,
                    None => continue,
                };
                let page_idx = entry.page_idx;
                let tex_id = self.pages[page_idx].tex_id;
                let mesh = meshes_by_page
                    .entry(page_idx)
                    .or_insert_with(|| Mesh::with_texture(tex_id));

                // swash placement: `left` is the bearing X, `top` is
                // the distance from baseline to the bitmap's top edge
                // (positive when bitmap top is above baseline).
                let x0 = origin.x + pg.x as f32 + entry.bearing[0];
                let y0 = origin.y + line_y + pg.y as f32 - entry.bearing[1];
                let x1 = x0 + entry.pixel_size[0] as f32;
                let y1 = y0 + entry.pixel_size[1] as f32;
                let rect = egui::Rect::from_min_max(
                    egui::pos2(x0, y0),
                    egui::pos2(x1, y1),
                );
                let uv = egui::Rect::from_min_max(
                    egui::pos2(entry.uv_rect[0], entry.uv_rect[1]),
                    egui::pos2(entry.uv_rect[2], entry.uv_rect[3]),
                );
                mesh.add_rect_with_uv(rect, uv, color);
            }
        }

        let size = egui::vec2(max_w, max_y.max(metrics.line_height));
        Some((meshes_by_page.into_values().collect(), size))
    }
}

pub fn engine() -> &'static Mutex<TextEngine> {
    static ENGINE: OnceLock<Mutex<TextEngine>> = OnceLock::new();
    ENGINE.get_or_init(|| Mutex::new(TextEngine::new()))
}
