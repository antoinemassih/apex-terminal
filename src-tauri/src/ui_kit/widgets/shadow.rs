//! GPU-blurred drop shadow for floating panels.
//!
//! ### NOTE: foundation primitive, not a widget
//! This module is a paint primitive consumed by widgets (Modal, Popover,
//! Sheet, ContextMenu, Tooltip). It exposes a free `paint()` function and
//! `ShadowPaint` presets — there is no builder + `show(ui, theme)` because
//! shadows are painted *underneath* a widget's own rect, not as a
//! standalone interactive element. The "Builder + show()" rule in
//! `CLAUDE.md` does not apply here.
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
//!   shadow::paint(painter, rect, ShadowPaint { radius, offset, color });
//!
//! ShadowPaint presets:
//!   ShadowPaint::sm()  // 8px radius, 0,2 offset  — tooltips
//!   ShadowPaint::md()  // 16px radius, 0,4 offset — popovers, context menus
//!   ShadowPaint::lg()  // 24px radius, 0,8 offset — modals
//!   ShadowPaint::xl()  // 32px radius, 0,12 offset — sheets
//
// FUTURE: replace with a true two-pass separable Gaussian via
// `egui_wgpu::CallbackTrait` for radii > 24px. The stacked-rect path
// looks acceptable up to ~24px; past that, the seams between layers
// can become visible at extreme zoom or on very dark backgrounds. A
// real two-pass blur on a small offscreen texture is the right
// answer — see `GPU_BLUR_NOTES.md` at the repo root for the pipeline
// sketch (texture pool, bind groups, shader source).

use egui::{Color32, Painter, Rect, Vec2};
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Tone};

/// What the shadow painter consumes.
///
/// Named `ShadowPaint`, not `ShadowSpec`, because `design_system::style_system`
/// has its own `ShadowSpec` and the two are NOT the same shape: that one is a
/// role's authored geometry (`blur`/`spread`/`offset_x`/`offset_y` + a 0.0–1.0
/// alpha multiplier); this one is paint input (Gaussian `radius`, a `Vec2`
/// offset, and a resolved `Color32`). Two different records under one name in
/// one crate is how three mechanisms ended up describing the same four
/// elevations with different numbers — see `ShadowTiers`.
#[derive(Clone, Copy, Debug)]
pub struct ShadowPaint {
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

impl ShadowPaint {
    /// Build a themed-color shadow from a base spec.
    #[inline]
    fn themed_color(t: &dyn crate::ui_kit::widgets::theme::ComponentTheme, alpha: u8) -> Color32 {
        let s = t.shadow_color();
        crate::ui_kit::style::color_alpha(s, alpha)
    }

    // P6.2 — deleted the four deprecated `sm/md/lg/xl()` constructors.
    // They hardcoded a black shadow tint which broke light themes.
    // All callers had already migrated to the `*_themed(theme)` variants
    // below (audit confirmed 0 non-doc call sites remain).

    /// Tooltips, resting cards — short, low-rise. Themed tint.
    pub fn sm_themed(t: &dyn crate::ui_kit::widgets::theme::ComponentTheme) -> Self {
        Self::from_tier(t, crate::ui_kit::style::elev_sm())
    }

    /// Popovers, context menus, dropdowns. Themed tint.
    pub fn md_themed(t: &dyn crate::ui_kit::widgets::theme::ComponentTheme) -> Self {
        Self::from_tier(t, crate::ui_kit::style::elev_md())
    }

    /// Modals. Themed tint.
    pub fn lg_themed(t: &dyn crate::ui_kit::widgets::theme::ComponentTheme) -> Self {
        Self::from_tier(t, crate::ui_kit::style::elev_lg())
    }

    /// Sheets, full-window overlays. Themed tint.
    pub fn xl_themed(t: &dyn crate::ui_kit::widgets::theme::ComponentTheme) -> Self {
        Self::from_tier(t, crate::ui_kit::style::elev_xl())
    }

    /// Build a spec from an authored rung.
    ///
    /// The four constructors above used to hold `radius: 8.0`, `Vec2::new(0.0,
    /// 2.0)` and `64` as bare literals. That made the elevation ladder the one
    /// part of the design system no theme could touch — and it was ALSO
    /// described, differently, by `StyleSystem.shadows` roles and by the
    /// `shadow_preset` tokens, so three mechanisms disagreed about how deep a
    /// modal sits. One ladder now, authored in `shadows.tiers`.
    fn from_tier(
        t: &dyn crate::ui_kit::widgets::theme::ComponentTheme,
        tier: crate::design_system::style_system::ShadowTier,
    ) -> Self {
        Self {
            radius: tier.radius,
            offset: Vec2::new(0.0, tier.offset_y),
            color: Self::themed_color(t, tier.alpha),
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
pub fn paint(painter: &Painter, target_rect: Rect, spec: ShadowPaint) {
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
        let color = crate::ui_kit::style::color_alpha(spec.color, alpha,);
        painter.rect_filled(r, cr, color);
    }
}

/// Paint a GPU-blurred shadow when the radius is large enough to benefit
/// from a real two-pass Gaussian; otherwise fall back to the stacked-rect
/// path. Same call signature as `paint`.
///
/// Routing:
///   - radius <= 16   →  stacked-rect (paint())
///   - radius >  16   →  GPU two-pass blur via `egui_wgpu::CallbackTrait`
///
/// If the GPU pipeline isn't initialised yet (very first frame, or the
/// chart renderer hasn't published the surface format), this also falls
/// back to the stacked-rect path. So callers can always use this function
/// safely.
pub fn paint_gpu(painter: &Painter, target_rect: Rect, spec: ShadowPaint) {
    let radius = spec.radius.clamp(2.0, 64.0);
    if radius <= 16.0 || !crate::ui_kit::widgets::shadow_pipeline::is_available() {
        return paint(painter, target_rect, spec);
    }

    let ppp = painter.ctx().pixels_per_point();
    let shadow_rect_pts = target_rect.translate(spec.offset).expand(spec.spread);
    let cb_rect_pts = shadow_rect_pts.expand(radius * 2.0);

    let to_px = |r: Rect| -> [f32; 4] {
        [
            r.min.x * ppp,
            r.min.y * ppp,
            r.max.x * ppp,
            r.max.y * ppp,
        ]
    };

    // Sigma in physical pixels = (radius pts) * ppp / 3, per the Gaussian
    // rule of thumb (kernel covers ~3 sigma each side).
    let sigma_px = (radius * ppp) / 3.0;
    // Corner radius matches the panel — pick something close to what the
    // existing stacked path uses so visual identity is preserved.
    let base_corner_pts = (target_rect.width().min(target_rect.height()) * 0.5).min(8.0);
    let corner_px = base_corner_pts * ppp;

    let r = spec.color.r() as f32 / 255.0;
    let g = spec.color.g() as f32 / 255.0;
    let b = spec.color.b() as f32 / 255.0;
    let a = spec.color.a() as f32 / 255.0;

    let cb = match crate::ui_kit::widgets::shadow_pipeline::ShadowCallback::try_new(
        to_px(shadow_rect_pts),
        to_px(cb_rect_pts),
        sigma_px,
        [r, g, b, a],
        corner_px,
    ) {
        Some(c) => c,
        None => return paint(painter, target_rect, spec),
    };
    let paint_cb = egui_wgpu::Callback::new_paint_callback(cb_rect_pts, cb);
    painter.add(egui::epaint::Shape::Callback(paint_cb));
}

/// Smoke-test gallery — paints all four shadow presets behind sample
/// rounded tiles. Drop into any panel for visual inspection.
pub fn show_shadow_gallery(
    ui: &mut egui::Ui,
    theme: &dyn crate::ui_kit::widgets::theme::ComponentTheme,
) {
    let presets: [(&str, ShadowPaint); 4] = [
        ("sm", ShadowPaint::sm_themed(theme)),
        ("md", ShadowPaint::md_themed(theme)),
        ("lg", ShadowPaint::lg_themed(theme)),
        ("xl", ShadowPaint::xl_themed(theme)),
    ];

    let tile_size = Vec2::new(120.0, 80.0);
    let gap = 48.0;
    let surface = palette_ct(theme).base(Tone::Surface);
    let text = palette_ct(theme).base(Tone::Text);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (name, spec) in presets {
            let (rect, _resp) = ui.allocate_exact_size(tile_size, egui::Sense::hover());
            let painter = ui.painter();
            paint(painter, rect, spec);
            painter.rect_filled(rect, st::radius_md(), surface);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                name,
                crate::ui_kit::style::prop_at(st::font_md_plus()),
                text,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::design_system::style_system::{ShadowTier, ShadowTiers};

    /// The four rungs must be strictly ordered on every axis.
    ///
    /// A ladder whose rungs are not monotonic is not a ladder — a modal would
    /// sit shallower than a tooltip and no call site could tell, because each
    /// constructor reads only its own rung. The three mechanisms this replaced
    /// were each internally plausible and mutually inconsistent; ordering is
    /// the property that makes one ladder checkable.
    #[test]
    fn the_elevation_ladder_is_strictly_ordered() {
        let t = ShadowTiers::default();
        let rungs = [("sm", t.sm), ("md", t.md), ("lg", t.lg), ("xl", t.xl)];
        for w in rungs.windows(2) {
            let (lo_name, lo) = w[0];
            let (hi_name, hi) = w[1];
            assert!(hi.radius > lo.radius, "{hi_name}.radius must exceed {lo_name}");
            assert!(hi.offset_y > lo.offset_y, "{hi_name}.offset_y must exceed {lo_name}");
            assert!(hi.alpha > lo.alpha, "{hi_name}.alpha must exceed {lo_name}");
        }
    }

    /// `from_tier` must carry the authored rung through unchanged.
    ///
    /// This is the seam where the literals used to live. If a constructor
    /// silently kept its old hardcoded 8.0/2.0/64, everything would still
    /// compile, render identically at the default ladder, and ignore every
    /// theme that authored its own depth — the same failure the cascade gate
    /// exists to catch one layer up.
    #[test]
    fn from_tier_uses_the_authored_rung_not_a_literal() {
        let theme = crate::ui_kit::widgets::theme::PortableTheme::dark();
        let tier = ShadowTier { radius: 41.0, offset_y: 13.0, alpha: 200 };
        let spec = super::ShadowPaint::from_tier(&theme, tier);
        assert_eq!(spec.radius, 41.0);
        assert_eq!(spec.offset.y, 13.0);
        assert_eq!(spec.color.a(), 200, "authored alpha must reach the paint spec");
    }
}
