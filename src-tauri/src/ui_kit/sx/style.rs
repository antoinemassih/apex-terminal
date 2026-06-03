//! `Sx` — composable utility style values (the "Tailwind layer").
//!
//! An `Sx` is a small `Copy` bag of optional style properties plus per-state
//! overrides (hover / active / disabled). It carries no heap data: the state
//! variants are flat [`SxDelta`] values, not boxed `Sx`. Resolving for a frame
//! is a few `Option` merges; painting reserves a single background shape so the
//! box renders *behind* the content (same trick as `ButtonGroupBox`). No
//! allocation, no lock, no color math on the hot path.

use egui::{Color32, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui};
use super::color::{palette, Palette, Shade, Tone};

type Theme = crate::chart_renderer::gpu::Theme;

#[inline]
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// How a box is filled.
#[derive(Clone, Copy)]
pub enum Fill {
    /// A solid ramp color.
    Shade(Tone, Shade),
    /// The tone's base color at an explicit alpha (a tinted overlay).
    Alpha(Tone, u8),
    /// A literal color.
    Solid(Color32),
}

impl Fill {
    #[inline]
    fn resolve(self, pal: &Palette) -> Color32 {
        match self {
            Fill::Shade(tone, s) => pal.shade(tone, s),
            Fill::Alpha(tone, a) => {
                let c = pal.base(tone);
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
            }
            Fill::Solid(c) => c,
        }
    }
}

/// Border spec — color resolved the same way as a [`Fill`] (so borders can be a
/// solid shade or a tinted alpha overlay).
#[derive(Clone, Copy)]
pub struct BorderSpec {
    pub color: Fill,
    pub width: f32,
}

/// Interaction state used to pick which override applies.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StyleState {
    Normal,
    Hover,
    Active,
    Disabled,
}

/// Flat style properties — no nested state. `Copy`, stack-only.
#[derive(Clone, Copy, Default)]
pub struct SxDelta {
    pub(crate) px: Option<f32>,
    pub(crate) py: Option<f32>,
    pub(crate) radius: Option<f32>,
    pub(crate) fill: Option<Fill>,
    pub(crate) border: Option<BorderSpec>,
    pub(crate) text: Option<(Tone, Shade)>,
    pub(crate) text_size: Option<f32>,
    pub(crate) gap: Option<f32>,
    pub(crate) opacity: Option<f32>,
}

impl SxDelta {
    pub fn new() -> Self { Self::default() }

    pub fn px(mut self, n: f32) -> Self { self.px = Some(n); self }
    pub fn py(mut self, n: f32) -> Self { self.py = Some(n); self }
    pub fn p(self, n: f32) -> Self { self.px(n).py(n) }
    pub fn rounded(mut self, r: f32) -> Self { self.radius = Some(r); self }
    pub fn gap(mut self, g: f32) -> Self { self.gap = Some(g); self }
    pub fn opacity(mut self, o: f32) -> Self { self.opacity = Some(o); self }

    /// Solid base (500) fill of a tone.
    pub fn bg(mut self, tone: Tone) -> Self { self.fill = Some(Fill::Shade(tone, Shade::S500)); self }
    /// Solid fill at an explicit shade.
    pub fn bg_shade(mut self, tone: Tone, s: Shade) -> Self { self.fill = Some(Fill::Shade(tone, s)); self }
    /// Tinted overlay: the tone's base color at `alpha`.
    pub fn bg_alpha(mut self, tone: Tone, alpha: u8) -> Self { self.fill = Some(Fill::Alpha(tone, alpha)); self }
    pub fn bg_color(mut self, c: Color32) -> Self { self.fill = Some(Fill::Solid(c)); self }

    pub fn border(mut self, tone: Tone, width: f32) -> Self {
        self.border = Some(BorderSpec { color: Fill::Shade(tone, Shade::S500), width });
        self
    }
    pub fn border_shade(mut self, tone: Tone, shade: Shade, width: f32) -> Self {
        self.border = Some(BorderSpec { color: Fill::Shade(tone, shade), width });
        self
    }
    /// Tinted border: the tone's base color at `alpha`.
    pub fn border_alpha(mut self, tone: Tone, alpha: u8, width: f32) -> Self {
        self.border = Some(BorderSpec { color: Fill::Alpha(tone, alpha), width });
        self
    }

    pub fn text(mut self, tone: Tone) -> Self { self.text = Some((tone, Shade::S500)); self }
    pub fn text_shade(mut self, tone: Tone, s: Shade) -> Self { self.text = Some((tone, s)); self }
    pub fn text_size(mut self, sz: f32) -> Self { self.text_size = Some(sz); self }

    /// Overlay `over`'s set fields on top of `self`.
    #[inline]
    pub(crate) fn merge(self, over: SxDelta) -> SxDelta {
        SxDelta {
            px: over.px.or(self.px),
            py: over.py.or(self.py),
            radius: over.radius.or(self.radius),
            fill: over.fill.or(self.fill),
            border: over.border.or(self.border),
            text: over.text.or(self.text),
            text_size: over.text_size.or(self.text_size),
            gap: over.gap.or(self.gap),
            opacity: over.opacity.or(self.opacity),
        }
    }

    /// The resolved text color for this delta, if any.
    #[inline]
    pub fn text_color(&self, pal: &Palette) -> Option<Color32> {
        self.text.map(|(tone, s)| pal.shade(tone, s))
    }
}

/// A utility style value with optional per-state overrides.
#[derive(Clone, Copy, Default)]
pub struct Sx {
    base: SxDelta,
    hover: Option<SxDelta>,
    active: Option<SxDelta>,
    disabled: Option<SxDelta>,
}

macro_rules! fwd {
    ($($name:ident ( $($a:ident : $ty:ty),* )),* $(,)?) => {
        $( pub fn $name(mut self, $($a: $ty),*) -> Self { self.base = self.base.$name($($a),*); self } )*
    };
}

impl Sx {
    pub fn new() -> Self { Self::default() }

    // Forward the common builders to `base` so call sites read like Tailwind.
    fwd!(
        px(n: f32), py(n: f32), p(n: f32), rounded(r: f32), gap(g: f32), opacity(o: f32),
        bg(tone: Tone), bg_shade(tone: Tone, s: Shade), bg_alpha(tone: Tone, alpha: u8), bg_color(c: Color32),
        border(tone: Tone, width: f32), border_shade(tone: Tone, shade: Shade, width: f32),
        border_alpha(tone: Tone, alpha: u8, width: f32),
        text(tone: Tone), text_shade(tone: Tone, s: Shade), text_size(sz: f32),
    );

    /// Hover override: `Sx::new().bg(Accent).hover(|d| d.bg_shade(Accent, S400))`.
    pub fn hover(mut self, f: impl FnOnce(SxDelta) -> SxDelta) -> Self {
        self.hover = Some(f(SxDelta::new())); self
    }
    pub fn active(mut self, f: impl FnOnce(SxDelta) -> SxDelta) -> Self {
        self.active = Some(f(SxDelta::new())); self
    }
    pub fn disabled(mut self, f: impl FnOnce(SxDelta) -> SxDelta) -> Self {
        self.disabled = Some(f(SxDelta::new())); self
    }

    /// Conditionally apply a delta (the `when(cond, |d| …)` utility).
    pub fn when(mut self, cond: bool, f: impl FnOnce(SxDelta) -> SxDelta) -> Self {
        if cond { self.base = f(self.base); }
        self
    }

    /// Overlay an `SxDelta` onto the base (used by `Recipe` variant merging).
    #[inline]
    pub fn with(mut self, d: SxDelta) -> Self {
        self.base = self.base.merge(d);
        self
    }

    /// Resolve the effective delta for an interaction state.
    #[inline]
    pub fn resolved(&self, st: StyleState) -> SxDelta {
        let over = match st {
            StyleState::Normal => None,
            StyleState::Hover => self.hover,
            StyleState::Active => self.active,
            StyleState::Disabled => self.disabled,
        };
        match over {
            Some(o) => self.base.merge(o),
            None => self.base,
        }
    }

    /// Paint the box for an explicit state behind `rect` into a reserved slot.
    fn paint(&self, ui: &Ui, slot: egui::layers::ShapeIdx, rect: egui::Rect, st: StyleState, pal: &Palette) {
        let d = self.resolved(st);
        let cr = CornerRadius::same(d.radius.unwrap_or(0.0) as u8);
        if let Some(fill) = d.fill {
            ui.painter().set(slot, egui::Shape::rect_filled(rect, cr, fill.resolve(pal)));
        }
        if let Some(b) = d.border {
            ui.painter().rect_stroke(
                rect, cr,
                Stroke::new(b.width, b.color.resolve(pal)),
                StrokeKind::Inside,
            );
        }
    }

    /// Paint with a motion-eased blend from `Normal` toward `Hover` by `hover_t`
    /// and toward `Active` by `active_t` (both 0..1, from egui's `animate_bool`).
    /// Fill/border colors lerp so the box fades between states like the legacy
    /// `Button`'s motion engine — closing the main Sx↔Button capability gap.
    fn paint_eased(
        &self, ui: &Ui, slot: egui::layers::ShapeIdx, rect: egui::Rect,
        hover_t: f32, active_t: f32, pal: &Palette,
    ) {
        let base = self.resolved(StyleState::Normal);
        let hov  = self.resolved(StyleState::Hover);
        let act  = self.resolved(StyleState::Active);
        let cr = CornerRadius::same(base.radius.unwrap_or(0.0) as u8);

        // Fill: lerp Normal→Hover→Active. A missing fill is treated as transparent.
        let fill_of = |d: &SxDelta| d.fill.map(|f| f.resolve(pal)).unwrap_or(Color32::TRANSPARENT);
        let mut fc = lerp_color(fill_of(&base), fill_of(&hov), hover_t);
        fc = lerp_color(fc, fill_of(&act), active_t);
        if fc.a() > 0 {
            ui.painter().set(slot, egui::Shape::rect_filled(rect, cr, fc));
        }

        // Border: lerp the same way (transparent when absent).
        let border_of = |d: &SxDelta| d.border.map(|b| (b.color.resolve(pal), b.width))
            .unwrap_or((Color32::TRANSPARENT, base.border.map(|b| b.width).unwrap_or(1.0)));
        let (bc_n, bw) = border_of(&base);
        let (bc_h, _)  = border_of(&hov);
        let (bc_a, _)  = border_of(&act);
        let mut bc = lerp_color(bc_n, bc_h, hover_t);
        bc = lerp_color(bc, bc_a, active_t);
        if bc.a() > 0 {
            ui.painter().rect_stroke(rect, cr, Stroke::new(bw, bc), StrokeKind::Inside);
        }
    }

    /// Paint this style's box at an explicit `rect` into a caller-reserved slot.
    /// For decorations whose geometry the caller already knows (e.g. a
    /// button-group enclosure spanning measured button bounds). The slot must
    /// have been reserved *before* the content was emitted so the box renders
    /// behind it.
    pub fn paint_into(
        &self,
        ui: &Ui,
        t: &Theme,
        slot: egui::layers::ShapeIdx,
        rect: egui::Rect,
        state: StyleState,
    ) {
        let pal = palette(t);
        self.paint(ui, slot, rect, state, &pal);
    }

    /// Interactive box: lays out `body` with this style's padding, auto-detects
    /// hover/active from the pointer, and paints the resolved background behind
    /// it. Returns the box `Response` (so callers can read `.clicked()` etc).
    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        sense: Sense,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> (Response, R) {
        let pal = palette(t);
        let slot = ui.painter().add(egui::Shape::Noop);
        let pad = egui::Margin::symmetric(
            self.base.px.unwrap_or(0.0) as i8,
            self.base.py.unwrap_or(0.0) as i8,
        );
        let ir = egui::Frame::NONE.inner_margin(pad).show(ui, |ui| {
            if let Some(g) = self.base.gap { ui.spacing_mut().item_spacing.x = g; }
            if let Some(col) = self.base.text_color(&pal) { ui.style_mut().visuals.override_text_color = Some(col); }
            body(ui)
        });
        let rect = ir.response.rect;
        let id = ui.id().with(("sx", rect.min.x.to_bits(), rect.min.y.to_bits()));
        let resp = ui.interact(rect, id, sense);
        // Motion-eased state blend (egui's built-in animation; no extra deps).
        let hover_t = ui.ctx().animate_bool(id.with("h"), resp.hovered());
        let active_t = ui.ctx().animate_bool(id.with("a"), resp.is_pointer_button_down_on());
        self.paint_eased(ui, slot, rect, hover_t, active_t, &pal);
        (resp, ir.inner)
    }

    /// Non-interactive decoration: reserve the slot first, run `body`, then fill
    /// the slot with the box behind the content using `state`. Use for static
    /// enclosures (e.g. a button-group box) that must not steal inner clicks.
    /// Returns `(content_rect, body_result)`.
    pub fn decorate<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        state: StyleState,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> (egui::Rect, R) {
        let pal = palette(t);
        let slot = ui.painter().add(egui::Shape::Noop);
        let pad = egui::Margin::symmetric(
            self.base.px.unwrap_or(0.0) as i8,
            self.base.py.unwrap_or(0.0) as i8,
        );
        let ir = egui::Frame::NONE.inner_margin(pad).show(ui, |ui| body(ui));
        let rect = ir.response.rect;
        self.paint(ui, slot, rect, state, &pal);
        (rect, ir.inner)
    }
}
