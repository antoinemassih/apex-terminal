//! GPU-blurred drop shadow for floating panels.
//!
//! egui's built-in `egui::epaint::Shadow` paints a feathered rectangle
//! that looks like 2010 Win32 chrome. This module paints a real
//! Gaussian-blurred-looking quad behind a target rect — used by
//! Modal / Popover / Sheet / ContextMenu / Tooltip to look like Zed.
//!
//! Strategy (v1, what's actually implemented here): stacked feathered
//! rounded rects with cubic alpha falloff. This is what shadcn-style
//! egui ports do in practice — fast, no wgpu boilerplate, visually
//! indistinguishable from a true Gaussian blur up to ~24px radii.
//!
//! Public API:
//!   shadow::paint(painter, rect, ShadowSpec { radius, offset, color });
//!
//! ShadowSpec presets:
//!   ShadowSpec::sm()  // 8px radius, 0,2 offset  — tooltips
//!   ShadowSpec::md()  // 16px radius, 0,4 offset — popovers, context menus
//!   ShadowSpec::lg()  // 24px radius, 0,8 offset — modals
//!   ShadowSpec::xl()  // 32px radius, 0,12 offset — sheets
//
// FUTURE: replace with a true two-pass separable Gaussian via
// `egui_wgpu::CallbackTrait` for radii > 24px. The stacked-rect path
// looks acceptable up to ~24px; past that, the seams between layers
// can become visible at extreme zoom or on very dark backgrounds. A
// real two-pass blur on a small offscreen texture is the right
// answer — see `GPU_BLUR_NOTES.md` at the repo root for the pipeline
// sketch (texture pool, bind groups, shader source).

use egui::{Color32, Painter, Rect, Vec2};

/// Specification for a soft drop shadow.
#[derive(Clone, Copy, Debug)]
pub struct ShadowSpec {
    /// Gaussian-equivalent sigma in pixels. Clamped to [2, 32] at paint time.
    pub radius: f32,
    /// Translation of the shadow relative to the target rect (typically downward).
    pub offset: Vec2,
    /// Shadow tint. The alpha channel is the peak shadow opacity at the centre
    /// of the blur; falloff is computed automatically.
    pub color: Color32,
    /// Extra growth beyond the target rect before blurring. 0 for a normal
    /// drop shadow; positive values give a "halo" effect.
    pub spread: f32,
}

impl ShadowSpec {
    /// Tooltips — short, low-rise, subtle.
    pub fn sm() -> Self {
        Self {
            radius: 8.0,
            offset: Vec2::new(0.0, 2.0),
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 64), // ~25% alpha
            spread: 0.0,
        }
    }

    /// Popovers, context menus.
    pub fn md() -> Self {
        Self {
            radius: 16.0,
            offset: Vec2::new(0.0, 4.0),
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 77), // ~30% alpha
            spread: 0.0,
        }
    }

    /// Modals.
    pub fn lg() -> Self {
        Self {
            radius: 24.0,
            offset: Vec2::new(0.0, 8.0),
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 89), // ~35% alpha
            spread: 0.0,
        }
    }

    /// Sheets, full-window overlays.
    pub fn xl() -> Self {
        Self {
            radius: 32.0,
            offset: Vec2::new(0.0, 12.0),
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 102), // ~40% alpha
            spread: 0.0,
        }
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = Vec2::new(x, y);
        self
    }

    pub fn color(mut self, c: Color32) -> Self {
        self.color = c;
        self
    }

    pub fn spread(mut self, s: f32) -> Self {
        self.spread = s;
        self
    }
}

/// Paint a soft drop shadow behind `target_rect`.
///
/// Call this BEFORE you paint your panel — the shadow is laid down,
/// then the caller's panel renders over it normally.
///
/// Implementation: N expanded rounded rects with decreasing alpha
/// following a cubic ease-out curve. This approximates a Gaussian
/// blur visually at small-to-medium radii.
pub fn paint(painter: &Painter, target_rect: Rect, spec: ShadowSpec) {
    let radius = spec.radius.clamp(2.0, 32.0);
    let n_steps = radius.round() as i32;
    if n_steps <= 0 {
        return;
    }

    let shadow_rect = target_rect.translate(spec.offset).expand(spec.spread);
    let max_alpha = spec.color.a() as f32;

    // Base corner radius — match a typical panel corner. Capped to half
    // the smaller side of the target so we never over-round tiny rects.
    let base_corner = (target_rect.width().min(target_rect.height()) * 0.5).min(8.0);

    // Per-step alpha weight. The 4.0 multiplier compensates for spreading
    // peak alpha across N layers; tuned by visual inspection so md() at
    // 16px reads as a soft pool rather than a faint smudge.
    let alpha_weight = 4.0;

    for step in 0..n_steps {
        let t = step as f32 / n_steps as f32;
        // Cubic ease-out — soft Zed-like falloff. A quadratic curve (egui's
        // built-in choice) reads as a harder halo; cubic feathers more.
        let alpha_factor = (1.0 - t).powi(3);
        let alpha = (max_alpha * alpha_factor / n_steps as f32 * alpha_weight)
            .clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            continue;
        }
        let expand = step as f32 + 1.0;
        let r = shadow_rect.expand(expand);
        let cr = base_corner + expand;
        let color = Color32::from_rgba_unmultiplied(
            spec.color.r(),
            spec.color.g(),
            spec.color.b(),
            alpha,
        );
        painter.rect_filled(r, cr, color);
    }
}

/// Smoke-test gallery — paints all four shadow presets behind sample
/// rounded tiles. Drop into any panel for visual inspection.
pub fn show_shadow_gallery(
    ui: &mut egui::Ui,
    theme: &dyn crate::ui_kit::widgets::theme::ComponentTheme,
) {
    let presets: [(&str, ShadowSpec); 4] = [
        ("sm", ShadowSpec::sm()),
        ("md", ShadowSpec::md()),
        ("lg", ShadowSpec::lg()),
        ("xl", ShadowSpec::xl()),
    ];

    let tile_size = Vec2::new(120.0, 80.0);
    let gap = 48.0;
    let surface = theme.surface();
    let text = theme.text();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (name, spec) in presets {
            let (rect, _resp) = ui.allocate_exact_size(tile_size, egui::Sense::hover());
            let painter = ui.painter();
            paint(painter, rect, spec);
            painter.rect_filled(rect, 8.0, surface);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                name,
                egui::FontId::proportional(14.0),
                text,
            );
        }
    });
}
