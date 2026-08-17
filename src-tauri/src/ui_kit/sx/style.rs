//! `Sx` — composable utility style values (the "Tailwind layer").
//!
//! An `Sx` is a small `Copy` bag of optional style properties plus per-state
//! overrides (hover / active / disabled). It carries no heap data: the state
//! variants are flat [`SxDelta`] values, not boxed `Sx`. Resolving for a frame
//! is a few `Option` merges; painting reserves a single background shape so the
//! box renders *behind* the content (same trick as `ButtonGroupBox`). No
//! allocation, no lock, no color math on the hot path.

use egui::{Color32, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui};
use super::color::{palette_ct, Palette, Shade, Tone};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::tokens as st;

/// Convert an `f32` radius to `u8` for `CornerRadius::same`.
///
/// The naïve `value as u8` cast wraps on values above 255 (pill radius 999
/// becomes 231) and truncates sub-pixel values below 1.0 to 0. This helper
/// clamps faithfully: values ≥ 255 become 255 (egui's u8 API maximum), values
/// in 0..255 round to the nearest integer, and negatives clamp to 0.
#[inline(always)]
fn radius_to_u8(r: f32) -> u8 {
    r.clamp(0.0, 255.0).round() as u8
}

/// UNmultiplied lerp — the sx ramp works in straight alpha.
///
/// Shares `motion::lerp_channels` so the interpolation cannot drift; only the
/// alpha treatment differs, and that difference is the whole reason this
/// wrapper exists rather than a call to `motion::lerp_color`.
#[inline]
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let [r, g, b_, al] = crate::ui_kit::widgets::motion::lerp_channels(a, b, t);
    Color32::from_rgba_unmultiplied(r, g, b_, al)
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
                crate::ui_kit::style::color_alpha(c, a)
            }
            Fill::Solid(c) => c,
        }
    }
}

/// M3.2: which EDGES a border paints. CSS `border-bottom` / `border-left`
/// were the single largest unmappable class in the recipe audit (~35 rules):
/// every tab underline, the chrome-active bottom rules on Cadence/Lucid/
/// Meridien, Mariner's 1.5px pane top-stripe and 2px DOM left edge, and all
/// the ledger hairlines. `BorderSpec` painted all four sides, so none of it
/// could be expressed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Edges {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl Edges {
    pub const ALL:    Self = Self { top: true,  right: true,  bottom: true,  left: true  };
    pub const TOP:    Self = Self { top: true,  right: false, bottom: false, left: false };
    pub const BOTTOM: Self = Self { top: false, right: false, bottom: true,  left: false };
    pub const LEFT:   Self = Self { top: false, right: false, bottom: false, left: true  };
    pub const RIGHT:  Self = Self { top: false, right: true,  bottom: false, left: false };
    #[inline] pub fn is_all(self) -> bool { self == Self::ALL }
    #[inline] pub fn any(self) -> bool { self.top || self.right || self.bottom || self.left }
}

impl Default for Edges {
    fn default() -> Self { Self::ALL }
}

/// M3.2: a 1px INSET edge line — the Sx-expressible half of CSS
/// `box-shadow: inset 0 ±1px 0 <color>`. This is Alto/Mariner's entire
/// "raised button face / sunken well" identity, Cadence's white top
/// highlight on filled surfaces, and Alto's `inset 0 -2px 0 accent` tab
/// marker (~25 rules). Full blurred inset shadows remain out of scope;
/// every inset in the six design systems is a hairline, which this covers.
#[derive(Clone, Copy)]
pub struct BevelSpec {
    /// Top inner line (the highlight on a raised face).
    pub top: Option<Fill>,
    /// Bottom inner line (the shadow on a raised face; the accent marker on tabs).
    pub bottom: Option<Fill>,
    /// Thickness in px (1.0 for hairlines, 2.0 for Alto's tab marker).
    pub width: f32,
}

/// Border spec — color resolved the same way as a [`Fill`] (so borders can be a
/// solid shade or a tinted alpha overlay).
#[derive(Clone, Copy)]
pub struct BorderSpec {
    pub color: Fill,
    pub width: f32,
    /// M3.2: which sides paint. Defaults to ALL (previous behaviour).
    pub edges: Edges,
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
    /// M3.2: inset hairline bevel (see [`BevelSpec`]) — the Zed raised face /
    /// Spotify highlight that ~25 recipe rules needed and could not express.
    pub(crate) bevel: Option<BevelSpec>,
    /// M3.2: font-weight hint (400/500/600/700). egui selects weight by FAMILY
    /// registration rather than a variable axis, so this maps to `strong` at
    /// >= 600 until per-weight families are registered (advisory, ~30 rules).
    pub(crate) weight: Option<u16>,
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
        self.border = Some(BorderSpec { color: Fill::Shade(tone, Shade::S500), width, edges: Edges::ALL });
        self
    }
    pub fn border_shade(mut self, tone: Tone, shade: Shade, width: f32) -> Self {
        self.border = Some(BorderSpec { color: Fill::Shade(tone, shade), width, edges: Edges::ALL });
        self
    }
    /// Tinted border: the tone's base color at `alpha`.
    pub fn border_alpha(mut self, tone: Tone, alpha: u8, width: f32) -> Self {
        self.border = Some(BorderSpec { color: Fill::Alpha(tone, alpha), width, edges: Edges::ALL });
        self
    }
    /// Border from an explicit raw color — for widgets that resolve their own
    /// color (e.g. a tone-enum pill). The border-side mirror of `bg_color`.
    /// M3.2: explicit colour + per-edge selection (the recipe path).
    pub fn border_color_edges(mut self, c: Color32, width: f32, edges: Edges) -> Self {
        self.border = Some(BorderSpec { color: Fill::Solid(c), width, edges });
        self
    }
    pub fn border_color(mut self, c: Color32, width: f32) -> Self {
        self.border = Some(BorderSpec { color: Fill::Solid(c), width, edges: Edges::ALL });
        self
    }

    pub fn text(mut self, tone: Tone) -> Self { self.text = Some((tone, Shade::S500)); self }
    pub fn text_shade(mut self, tone: Tone, s: Shade) -> Self { self.text = Some((tone, s)); self }
    pub fn text_size(mut self, sz: f32) -> Self { self.text_size = Some(sz); self }

    /// Border on specific EDGES only (M3.2). `border_bottom(Tone::Accent, 2.0)`
    /// is the tab-underline idiom the vocabulary previously could not express.
    pub fn border_edges(mut self, tone: Tone, width: f32, edges: Edges) -> Self {
        self.border = Some(BorderSpec { color: Fill::Shade(tone, Shade::S500), width, edges });
        self
    }
    pub fn border_bottom(self, tone: Tone, width: f32) -> Self {
        self.border_edges(tone, width, Edges::BOTTOM)
    }
    pub fn border_top(self, tone: Tone, width: f32) -> Self {
        self.border_edges(tone, width, Edges::TOP)
    }
    pub fn border_left(self, tone: Tone, width: f32) -> Self {
        self.border_edges(tone, width, Edges::LEFT)
    }

    /// Inset hairline bevel (M3.2) — the Zed raised-face / Spotify highlight.
    pub fn bevel(mut self, top: Option<Fill>, bottom: Option<Fill>, width: f32) -> Self {
        self.bevel = Some(BevelSpec { top, bottom, width });
        self
    }
    /// Raised face: light top line + dark bottom line (Alto/Mariner buttons).
    pub fn bevel_raised(self, top: Tone, top_a: u8, bottom_a: u8) -> Self {
        self.bevel(
            Some(Fill::Alpha(top, top_a)),
            Some(Fill::Alpha(Tone::Bg, bottom_a)),
            1.0,
        )
    }

    /// Font weight hint (M3.2). >= 600 renders `strong` today.
    pub fn weight(mut self, w: u16) -> Self { self.weight = Some(w); self }

    // ── Token-tier builders ─────────────────────────────────────────────────
    // These resolve through the unified token scale (`frame_tokens()` × the live
    // corner-scale / spacing-scale / border-weight overrides), NOT raw numbers —
    // so an Sx-styled dimension responds to the same inspector knobs as a
    // `st::`-styled one. This is what makes Sx the single system for *all* styles
    // (spacing, radius, typography, borders), not just color. Values are captured
    // at build time; Sx is rebuilt per frame, so live override changes propagate.

    /// Corner radius tiers (obey the CornerScale override; Sharp ⇒ 0 everywhere).
    pub fn rounded_xs(self) -> Self { self.rounded(st::radius_xs()) }
    pub fn rounded_sm(self) -> Self { self.rounded(st::radius_sm()) }
    pub fn rounded_md(self) -> Self { self.rounded(st::radius_md()) }
    pub fn rounded_lg(self) -> Self { self.rounded(st::radius_lg()) }

    /// Inter-element gap tiers (obey the SpacingScale override).
    pub fn gap_xs(self) -> Self { self.gap(st::gap_xs()) }
    pub fn gap_sm(self) -> Self { self.gap(st::gap_sm()) }
    pub fn gap_md(self) -> Self { self.gap(st::gap_md()) }
    pub fn gap_lg(self) -> Self { self.gap(st::gap_lg()) }

    /// Symmetric padding tiers (spacing scale on both axes).
    pub fn p_xs(self) -> Self { self.p(st::gap_xs()) }
    pub fn p_sm(self) -> Self { self.p(st::gap_sm()) }
    pub fn p_md(self) -> Self { self.p(st::gap_md()) }
    pub fn p_lg(self) -> Self { self.p(st::gap_lg()) }
    /// Axis padding tiers.
    pub fn px_sm(self) -> Self { self.px(st::gap_sm()) }
    pub fn px_md(self) -> Self { self.px(st::gap_md()) }
    pub fn px_lg(self) -> Self { self.px(st::gap_lg()) }
    pub fn py_xs(self) -> Self { self.py(st::gap_xs()) }
    pub fn py_sm(self) -> Self { self.py(st::gap_sm()) }
    pub fn py_md(self) -> Self { self.py(st::gap_md()) }

    /// Type-scale tiers (obey the typography token set).
    pub fn text_xs(self) -> Self { self.text_size(st::font_xs()) }
    pub fn text_sm(self) -> Self { self.text_size(st::font_sm()) }
    pub fn text_md(self) -> Self { self.text_size(st::font_md()) }
    pub fn text_lg(self) -> Self { self.text_size(st::font_lg()) }

    /// Border-weight tiers (obey the BorderWeight override). Color from a tone.
    pub fn border_hair(self, tone: Tone) -> Self { self.border(tone, st::stroke_hair()) }
    pub fn border_thin(self, tone: Tone) -> Self { self.border(tone, st::stroke_thin()) }
    pub fn border_std(self, tone: Tone) -> Self { self.border(tone, st::stroke_std()) }
    /// Tinted thin border (the common hairline-overlay case).
    pub fn border_thin_alpha(self, tone: Tone, alpha: u8) -> Self {
        self.border_alpha(tone, alpha, st::stroke_thin())
    }

    /// M3.2: paint a border honouring per-edge selection. Falls back to the
    /// single `rect_stroke` when all four edges are on, so the common path is
    /// unchanged (same shape count, same tessellation).
    pub(crate) fn paint_border_edges(
        painter: &egui::Painter,
        rect: egui::Rect,
        cr: CornerRadius,
        b: BorderSpec,
        pal: &Palette,
    ) {
        let col = b.color.resolve(pal);
        if col.a() == 0 || b.width <= 0.0 { return; }
        if b.edges.is_all() {
            painter.rect_stroke(rect, cr, Stroke::new(b.width, col), StrokeKind::Inside);
            return;
        }
        let stroke = Stroke::new(b.width, col);
        let h = b.width * 0.5;
        if b.edges.top {
            painter.line_segment(
                [egui::pos2(rect.left(), rect.top() + h), egui::pos2(rect.right(), rect.top() + h)],
                stroke,
            );
        }
        if b.edges.bottom {
            painter.line_segment(
                [egui::pos2(rect.left(), rect.bottom() - h), egui::pos2(rect.right(), rect.bottom() - h)],
                stroke,
            );
        }
        if b.edges.left {
            painter.line_segment(
                [egui::pos2(rect.left() + h, rect.top()), egui::pos2(rect.left() + h, rect.bottom())],
                stroke,
            );
        }
        if b.edges.right {
            painter.line_segment(
                [egui::pos2(rect.right() - h, rect.top()), egui::pos2(rect.right() - h, rect.bottom())],
                stroke,
            );
        }
    }

    /// M3.2: paint an inset hairline bevel INSIDE `rect` (after the fill).
    /// Corresponds to CSS `box-shadow: inset 0 ±Npx 0 <color>`.
    pub(crate) fn paint_bevel(
        painter: &egui::Painter,
        rect: egui::Rect,
        cr: CornerRadius,
        bev: BevelSpec,
        pal: &Palette,
    ) {
        if !rect.is_finite() || rect.width() < 1.0 || rect.height() < 1.0 { return; }
        // Inset past the corner arc so the line doesn't overshoot a rounded box.
        let r = cr.nw.max(cr.ne).max(cr.sw).max(cr.se) as f32;
        let inset = (r * 0.5).clamp(0.0, 3.0);
        let h = bev.width * 0.5;
        if let Some(top) = bev.top {
            let c = top.resolve(pal);
            if c.a() > 0 {
                let y = rect.top() + h;
                painter.line_segment(
                    [egui::pos2(rect.left() + inset, y), egui::pos2(rect.right() - inset, y)],
                    Stroke::new(bev.width, c),
                );
            }
        }
        if let Some(bottom) = bev.bottom {
            let c = bottom.resolve(pal);
            if c.a() > 0 {
                let y = rect.bottom() - h;
                painter.line_segment(
                    [egui::pos2(rect.left() + inset, y), egui::pos2(rect.right() - inset, y)],
                    Stroke::new(bev.width, c),
                );
            }
        }
    }

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
            bevel: over.bevel.or(self.bevel),
            weight: over.weight.or(self.weight),
        }
    }

    /// The resolved text color for this delta, if any.
    #[inline]
    pub fn text_color(&self, pal: &Palette) -> Option<Color32> {
        self.text.map(|(tone, s)| pal.shade(tone, s))
    }

    /// M3.1: resolve the fill against a palette. Was copy-pasted as a 10-line
    /// `match fill { Solid | Shade | Alpha }` at every recipe consumer
    /// (panel_list_row, tabs ×2, panel_section, tag) because `SxDelta` exposed
    /// `text_color` but no fill twin — the audit flagged the duplication.
    /// Every new recipe consumer uses this.
    pub fn fill_color(&self, pal: &Palette) -> Option<Color32> {
        self.fill.map(|fill| match fill {
            Fill::Solid(c)        => c,
            Fill::Shade(tone, s)  => pal.shade(tone, s),
            Fill::Alpha(tone, a)  => {
                let b = pal.base(tone);
                crate::ui_kit::style::color_alpha(b, a)
            }
        })
    }

    /// M3.2 accessors for the extended vocabulary.
    pub fn bevel_spec(&self) -> Option<BevelSpec> { self.bevel }
    /// Authored horizontal padding, if any. Cards read this to honour
    /// `--ds-card-pad` from the recipe layer.
    pub fn pad_x(&self) -> Option<f32> { self.px }
    /// The authored border, if any. `None` means the recipe says NO border —
    /// distinct from a zero-width one.
    pub fn border_spec(&self) -> Option<BorderSpec> { self.border }
    pub fn weight_hint(&self) -> Option<u16> { self.weight }
    /// True when the authored weight asks for a bold face (>= 600).
    pub fn is_strong(&self) -> Option<bool> { self.weight.map(|w| w >= 600) }
    /// Which edges the border paints (ALL when unauthored).
    pub fn resolved_border_edges(&self) -> Edges {
        self.border.map(|b| b.edges).unwrap_or(Edges::ALL)
    }

    /// Border colour resolved against a palette (same rationale as `fill_color`).
    /// Named `resolved_border_color` to avoid colliding with the `border_color`
    /// BUILDER on `Sx`.
    pub fn resolved_border_color(&self, pal: &Palette) -> Option<Color32> {
        self.border.map(|b| match b.color {
            Fill::Solid(c)        => c,
            Fill::Shade(tone, s)  => pal.shade(tone, s),
            Fill::Alpha(tone, a)  => {
                let base = pal.base(tone);
                crate::ui_kit::style::color_alpha(base, a)
            }
        })
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
        border_alpha(tone: Tone, alpha: u8, width: f32), border_color(c: Color32, width: f32),
        text(tone: Tone), text_shade(tone: Tone, s: Shade), text_size(sz: f32),
        // Token-tier (scale-aware) builders — the "all styles" surface.
        rounded_xs(), rounded_sm(), rounded_md(), rounded_lg(),
        gap_xs(), gap_sm(), gap_md(), gap_lg(),
        p_xs(), p_sm(), p_md(), p_lg(), px_sm(), px_md(), px_lg(), py_xs(), py_sm(), py_md(),
        text_xs(), text_sm(), text_md(), text_lg(),
        border_hair(tone: Tone), border_thin(tone: Tone), border_std(tone: Tone),
        border_thin_alpha(tone: Tone, alpha: u8),
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
        let cr = CornerRadius::same(radius_to_u8(d.radius.unwrap_or(0.0)));
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
        let cr = CornerRadius::same(radius_to_u8(base.radius.unwrap_or(0.0)));

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
            // M3.2: honour per-edge selection here too (state-driven paints).
            let edges = base.border.map(|b| b.edges).unwrap_or(Edges::ALL);
            if edges.is_all() {
                ui.painter().rect_stroke(rect, cr, Stroke::new(bw, bc), StrokeKind::Inside);
            } else {
                SxDelta::paint_border_edges(
                    ui.painter(), rect, cr,
                    BorderSpec { color: Fill::Solid(bc), width: bw, edges }, pal,
                );
            }
        }
        // M3.2: inset bevel on the state path (base spec; bevels are identity,
        // not a hover affordance, so they do not lerp).
        if let Some(bev) = base.bevel {
            SxDelta::paint_bevel(ui.painter(), rect, cr, bev, pal);
        }
    }

    /// Paint this style's Normal-state box (fill + border) directly into `rect`,
    /// resolving colors from a [`ComponentTheme`] — the ui_kit-facing paint entry.
    ///
    /// This is the DS#4 bridge: a widget DECLARES its box as an `Sx`
    /// (`Sx::new().rounded_md().bg_alpha(tone, 32).border_alpha(tone, 200, w)`)
    /// and renders it in one call, instead of hand-writing `rect_filled` +
    /// `rect_stroke`. Immediate paint (not a reserved slot) — call it before
    /// emitting the widget's own content so the box sits behind it.
    pub fn paint_box_ct(&self, ui: &Ui, rect: egui::Rect, t: &dyn ComponentTheme) {
        self.paint_box_at(ui.painter(), rect, t);
    }

    /// As [`paint_box_ct`], but into a caller-supplied `Painter` — for widgets
    /// that already hold a (clipped) painter, or paint helpers with no `Ui`.
    pub fn paint_box_at(&self, painter: &egui::Painter, rect: egui::Rect, t: &dyn ComponentTheme) {
        let pal = palette_ct(t);
        let d = self.resolved(StyleState::Normal);
        let cr = CornerRadius::same(radius_to_u8(d.radius.unwrap_or(0.0)));
        if let Some(fill) = d.fill {
            painter.rect_filled(rect, cr, fill.resolve(&pal));
        }
        if let Some(b) = d.border {
            // M3.2: per-edge borders (tab underlines, ledger hairlines, the
            // Mariner pane top-stripe). ALL-edges takes the original path.
            SxDelta::paint_border_edges(painter, rect, cr, b, &pal);
        }
        // M3.2: inset bevel last so it reads as an inner edge over the fill.
        if let Some(bev) = d.bevel {
            SxDelta::paint_bevel(painter, rect, cr, bev, &pal);
        }
    }

    // ── ComponentTheme-based core implementations ────────────────────────────
    // These are the canonical entry points for new code and the real
    // implementation for the `&Theme`-shim methods below.

    /// Interactive box using the portable [`ComponentTheme`] API.
    /// Prefer this over [`show`] in new code — `t` can be any `&dyn ComponentTheme`
    /// (including `&gpu::Theme` which already implements the trait).
    pub fn show_ct<R>(
        self,
        ui: &mut Ui,
        t: &dyn ComponentTheme,
        sense: Sense,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> (Response, R) {
        let pal = palette_ct(t);
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
        let hover_t = ui.ctx().animate_bool(id.with("h"), resp.hovered());
        let active_t = ui.ctx().animate_bool(id.with("a"), resp.is_pointer_button_down_on());
        self.paint_eased(ui, slot, rect, hover_t, active_t, &pal);
        (resp, ir.inner)
    }

    /// Non-interactive decoration using the portable [`ComponentTheme`] API.
    /// Prefer this over [`decorate`] in new code.
    pub fn decorate_ct<R>(
        self,
        ui: &mut Ui,
        t: &dyn ComponentTheme,
        state: StyleState,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> (egui::Rect, R) {
        let pal = palette_ct(t);
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

    /// Paint into a reserved slot using the portable [`ComponentTheme`] API.
    /// Prefer this over [`paint_into`] in new code.
    pub fn paint_into_ct(
        &self,
        ui: &Ui,
        t: &dyn ComponentTheme,
        slot: egui::layers::ShapeIdx,
        rect: egui::Rect,
        state: StyleState,
    ) {
        let pal = palette_ct(t);
        self.paint(ui, slot, rect, state, &pal);
    }

}
