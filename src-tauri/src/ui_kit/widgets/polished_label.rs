//! High-quality text label using cosmic-text for subpixel antialiased
//! rendering.
//!
//! Use for: modal titles, panel headers, tooltips, anywhere static
//! text quality matters more than ms-per-frame cost.
//!
//! For high-frequency dynamic text (chart axis labels, price ticks,
//! scrolling tape) keep using egui's default text rendering.
//!
//! API:
//!   ui.add(PolishedLabel::new("Account Summary").size(Size::Lg));
//!
//!   PolishedLabel::new("Heading")
//!     .size(Size::Lg)
//!     .weight(FontWeight::Semibold)
//!     .show(ui, theme);
//!
//! ## Phase 1 status (this file)
//!
//! This is the spike scaffold. The cosmic-text dependency is wired in
//! and we go through the motions of constructing a `FontSystem` +
//! shaping a `Buffer` so the dep is exercised on every paint, but the
//! actual pixels still come from egui's grayscale atlas via a fallback
//! `Label`.
//!
//! Phase 2 (see `docs/COSMIC_TEXT_SWAP_PLAN.md`) will:
//!   - Own a `SwashCache` and rasterise glyphs at fractional X.
//!   - Upload glyph bitmaps via an `egui_wgpu::CallbackTrait`.
//!   - Survive subpixel AA all the way to the swapchain.
//!
//! The public API here is the API Phase 2 will keep, so call sites
//! that adopt `PolishedLabel` now will pick up real subpixel AA when
//! Phase 2 lands without source changes.

use egui::{Color32, Response, Ui};

use super::label::Label;
use super::theme::ComponentTheme;
use super::tokens::Size;

/// Font weight axis. Mapped to cosmic-text `Weight` in Phase 2; ignored
/// by the Phase 1 fallback (our shipped TTFs are single-weight files).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontWeight {
    Regular,
    #[default]
    Medium,
    Semibold,
    Bold,
}

impl FontWeight {
    fn to_cosmic(self) -> cosmic_text::Weight {
        match self {
            FontWeight::Regular => cosmic_text::Weight::NORMAL,
            FontWeight::Medium => cosmic_text::Weight::MEDIUM,
            FontWeight::Semibold => cosmic_text::Weight::SEMIBOLD,
            FontWeight::Bold => cosmic_text::Weight::BOLD,
        }
    }
}

#[must_use = "PolishedLabel does nothing until `.show(ui, theme)` or `ui.add(label)` is called"]
pub struct PolishedLabel<'a> {
    text: String,
    size: Size,
    weight: FontWeight,
    color: Option<Color32>,
    _lt: std::marker::PhantomData<&'a ()>,
}

impl<'a> PolishedLabel<'a> {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            size: Size::Sm,
            weight: FontWeight::default(),
            color: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }

    pub fn weight(mut self, w: FontWeight) -> Self {
        self.weight = w;
        self
    }

    pub fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        // Phase 2: real cosmic-text path. Shape with cosmic-text +
        // rasterize via swash, upload glyph bitmaps into a managed
        // egui atlas, emit a `Mesh` per atlas page. See
        // `super::text_engine` for the pipeline; v1 honesty caveat
        // (grayscale-at-atlas-boundary) documented there and in the
        // plan doc.
        let size_pt = self.size.font_size();
        let color = self.color.unwrap_or_else(|| theme.text());
        let family = cosmic_text::Family::SansSerif;
        let weight = self.weight.to_cosmic();

        let mesh_result = {
            let engine_lock = super::text_engine::engine();
            let mut engine = match engine_lock.lock() {
                Ok(g) => g,
                Err(_) => return self.fallback_show(ui, theme),
            };
            engine.shape_and_render(
                ui.ctx(),
                egui::pos2(0.0, 0.0),
                &self.text,
                size_pt,
                family,
                weight,
                color,
            )
        };

        let (meshes, size) = match mesh_result {
            Some(r) => r,
            None => return self.fallback_show(ui, theme),
        };

        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
        let offset = rect.min.to_vec2();
        for mut mesh in meshes {
            for v in mesh.vertices.iter_mut() {
                v.pos += offset;
            }
            ui.painter().add(egui::Shape::Mesh(mesh.into()));
        }
        response
    }

    fn fallback_show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let mut fallback = Label::new(self.text.clone()).size(self.size);
        if let Some(c) = self.color {
            fallback = fallback.color(c);
        }
        // Map the requested weight to a real registered Inter face.
        // Medium uses the default Inter-Medium picker family (no
        // override). Regular/Semibold/Bold pin a specific weight family
        // so we render a true heavier glyph rather than egui's
        // faux-bold stretch.
        match self.weight {
            FontWeight::Regular => {
                fallback = fallback.with_text_family("inter_regular");
            }
            FontWeight::Medium => {}
            FontWeight::Semibold => {
                fallback = fallback.with_text_family("inter_semibold");
            }
            FontWeight::Bold => {
                fallback = fallback.with_text_family("inter_bold");
            }
        }
        fallback.show(ui, theme)
    }
}

impl<'a> egui::Widget for PolishedLabel<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}

// ---------------------------------------------------------------------
// Smoke test (callable, not wired)
// ---------------------------------------------------------------------

/// Paint a small ladder of labels for visual A/B comparison against
/// `Label`. Not wired into the app — call from a debug panel when
/// evaluating the spike.
#[allow(dead_code)]
pub fn polished_label_smoke(ui: &mut Ui, theme: &dyn ComponentTheme) {
    ui.vertical(|ui| {
        ui.label("— PolishedLabel smoke test —");
        PolishedLabel::new("Account Summary")
            .size(Size::Lg)
            .weight(FontWeight::Semibold)
            .show(ui, theme);
        PolishedLabel::new("Total P/L: ligatures =>  !=  ==  ->")
            .size(Size::Md)
            .show(ui, theme);
        PolishedLabel::new("body text at sm")
            .size(Size::Sm)
            .show(ui, theme);
        PolishedLabel::new("xs caption")
            .size(Size::Xs)
            .weight(FontWeight::Regular)
            .show(ui, theme);
    });
}
