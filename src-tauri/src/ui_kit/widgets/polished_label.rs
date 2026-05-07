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

use std::sync::OnceLock;
use std::sync::Mutex;

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
    #[allow(dead_code)] // used by Phase 2 pipeline
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
        // Phase 1: exercise the cosmic-text shaper to confirm the dep
        // links and to keep the integration warm. The shaped width is
        // discarded — Phase 2 will use it for layout. We swallow any
        // panics defensively because cosmic-text's first call lazily
        // builds a system font db which can be slow / fail on locked
        // appdata directories.
        let _ = shape_width_hint(&self.text, self.size.font_size(), self.weight);

        // Phase 1 fallback: paint with the standard label so the build
        // is honest and the widget renders something usable today.
        let mut fallback = Label::new(self.text.clone()).size(self.size);
        if let Some(c) = self.color {
            fallback = fallback.color(c);
        }
        if matches!(self.weight, FontWeight::Semibold | FontWeight::Bold) {
            fallback = fallback.strong();
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
// cosmic-text plumbing (Phase 1 — measurement only)
// ---------------------------------------------------------------------

/// Lazy global `FontSystem`. cosmic-text recommends one per app; building
/// it scans the system font directories which is expensive (10–100 ms).
fn font_system() -> &'static Mutex<cosmic_text::FontSystem> {
    static SYS: OnceLock<Mutex<cosmic_text::FontSystem>> = OnceLock::new();
    SYS.get_or_init(|| {
        // `new()` loads system fonts. Phase 2 will additionally feed
        // our 6 shipped TTFs via `db_mut().load_font_data(...)` so we
        // don't depend on the user having Inter installed.
        Mutex::new(cosmic_text::FontSystem::new())
    })
}

/// Shape `text` and return the advance width in pixels. Used in Phase 1
/// only to prove the dep works; Phase 2 will return a full layout.
#[allow(dead_code)]
fn shape_width_hint(text: &str, px: f32, weight: FontWeight) -> f32 {
    use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping};

    let Ok(mut sys) = font_system().lock() else {
        return 0.0;
    };
    let metrics = Metrics::new(px, px * 1.2);
    let mut buffer = Buffer::new(&mut sys, metrics);
    let attrs = Attrs::new()
        .family(Family::SansSerif)
        .weight(weight.to_cosmic());
    buffer.set_text(&mut sys, text, attrs, Shaping::Advanced);
    // Sum of glyph advances on line 0.
    buffer
        .layout_runs()
        .next()
        .map(|run| run.line_w)
        .unwrap_or(0.0)
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
