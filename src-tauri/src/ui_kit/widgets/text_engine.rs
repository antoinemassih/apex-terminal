//! Cosmic-text rasterization pipeline. Owns a global FontSystem +
//! SwashCache + an egui-managed glyph atlas. Used by `PolishedLabel`.
//!
//! Lifecycle: lazy-init on first request. Atlas grows on demand;
//! eviction when full not yet implemented (acceptable for v1 since
//! polished labels are rare).
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
use std::sync::{Mutex, OnceLock};

use cosmic_text::{Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use egui::epaint::{ColorImage, ImageDelta, Mesh};
use egui::{Color32, TextureId, TextureOptions};

const ATLAS_SIZE: usize = 512;
const ATLAS_PAD: u32 = 1;

#[derive(Clone, Copy, Debug)]
struct AtlasEntry {
    page_idx: usize,
    /// UV rect in normalized [0,1] coords (u0, v0, u1, v1).
    uv_rect: [f32; 4],
    /// Pixel offset from the glyph origin to the bitmap's top-left,
    /// matching `swash::Image::placement.{left, top}`.
    bearing: [f32; 2],
    pixel_size: [u32; 2],
}

struct AtlasPage {
    tex_id: TextureId,
    /// Shelf-packing cursor: current row's left edge and top edge,
    /// plus the row height.
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

pub struct TextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
    pages: Vec<AtlasPage>,
    cache: HashMap<CacheKey, Option<AtlasEntry>>,
    fonts_loaded: bool,
}

impl TextEngine {
    fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            pages: Vec::new(),
            cache: HashMap::new(),
            fonts_loaded: false,
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
        });
        self.pages.len() - 1
    }

    /// Try to allocate a (w x h) region in some existing page or a
    /// fresh page. Returns (page_idx, x, y).
    fn pack(&mut self, ctx: &egui::Context, w: u32, h: u32) -> Option<(usize, u32, u32)> {
        if w as usize > ATLAS_SIZE || h as usize > ATLAS_SIZE {
            return None;
        }
        // Try the most-recent page first.
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
        if let Some(entry) = self.cache.get(&key) {
            return *entry;
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
        if text.is_empty() {
            return Some((Vec::new(), egui::vec2(0.0, size_pt * 1.2)));
        }
        self.ensure_fonts();

        let metrics = Metrics::new(size_pt, size_pt * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs::new().family(family).weight(weight);
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
