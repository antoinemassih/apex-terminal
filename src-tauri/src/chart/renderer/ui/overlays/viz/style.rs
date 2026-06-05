//! Chart **style spec** — the stylable dimensions every viz primitive reads,
//! so charts vary per theme/`StyleSystem` (stroke weight, dash/dot, fill
//! treatment, number fonts, proportions) the same way the rest of the UI does.
//!
//! For now [`ChartStyle::resolve`] derives from the existing per-theme token
//! helpers (`stroke_*`, `font_*`, `gap_*`, `alpha_*`, `radius_*`) — which already
//! shift with the active `StyleSystem` — plus a couple of chart-level choices
//! (line pattern, fill mode). Deeper per-style chart treatments (e.g. dashed in
//! one style, solid in another) can later be lifted into a `ChartStyle` substruct
//! on `StyleSystem` itself; call sites won't change.

use egui::{self, Color32, Pos2, Stroke};
use crate::ui_kit::sx::{Tone, Shade};
use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::gpu::Theme;

/// How a poly-line / outline is stroked.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum LinePattern { Solid, Dashed, Dotted }

/// How an area / region under a curve is filled.
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)] // Hollow used by outline-only chart variants
pub(crate) enum FillMode { Solid, Soft, Hollow }

/// How a stroke / arc terminates: square (`Butt`) or rounded (`Round`).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum LineCap { Butt, Round }

/// Resolved, per-theme chart style. Cheap to construct (reads cached token fns).
#[derive(Clone, Copy)]
pub(crate) struct ChartStyle {
    pub line_thin: f32,
    pub line_std: f32,
    pub line_bold: f32,
    pub pattern: LinePattern,
    pub fill: FillMode,
    /// Alpha for the primary area/fill (0..255).
    pub fill_alpha: u8,
    /// Alpha for the faint track / grid behind data.
    pub track_alpha: u8,
    /// Corner radius for bars / cells.
    pub corner: f32,
    /// Inter-element gap (bars, cells).
    pub gap: f32,
    /// How rings / arcs / lines terminate (round vs square ends).
    pub cap: LineCap,
    /// Per-theme multiplier on ring/arc stroke thickness (gauges, multi-rings).
    pub ring_scale: f32,
    /// Hero value font (big number).
    pub num_lg: f32,
    /// Secondary value / axis font.
    pub num_md: f32,
    /// Annotation font.
    pub num_sm: f32,
    /// Eyebrow / metric-name caption font.
    pub eyebrow: f32,
}

impl ChartStyle {
    /// Resolve the chart style for the active theme.
    pub(crate) fn resolve(_t: &Theme) -> Self {
        Self {
            line_thin: stroke_thin(),
            line_std: stroke_std(),
            line_bold: stroke_bold(),
            pattern: LinePattern::Solid,
            fill: FillMode::Soft,
            fill_alpha: alpha_muted(),
            track_alpha: alpha_subtle(),
            corner: radius_sm(),
            gap: gap_xs(),
            // Derived per theme from existing style tokens: rounded-radii styles
            // get round ring/line ends; sharp (radii≈0) styles get square ends.
            // Ring thickness tracks the style's stroke weight.
            cap: if radius_sm() >= 3.0 { LineCap::Round } else { LineCap::Butt },
            ring_scale: stroke_std().clamp(0.7, 1.5),
            num_lg: font_xl(),
            num_md: font_md(),
            num_sm: font_xs(),
            eyebrow: FONT_2XS,
        }
    }

    /// A copy with a specific line pattern (for previews / per-series variety).
    #[allow(dead_code)]
    pub(crate) fn with_pattern(mut self, pattern: LinePattern) -> Self {
        self.pattern = pattern;
        self
    }
}

// ── Color sequencing ─────────────────────────────────────────────────────────

/// A categorical colour for series / slice `i`, drawn from the Sx ramps so it
/// tracks the active `ColorScheme`. Distinct hues that read well together.
pub(crate) fn categorical(t: &Theme, i: usize) -> Color32 {
    const SEQ: [(Tone, Shade); 8] = [
        (Tone::Accent, Shade::S400),
        (Tone::Bull,   Shade::S400),
        (Tone::Warn,   Shade::S400),
        (Tone::Bear,   Shade::S400),
        (Tone::Accent, Shade::S600),
        (Tone::Bull,   Shade::S600),
        (Tone::Warn,   Shade::S600),
        (Tone::Bear,   Shade::S600),
    ];
    let (tone, sh) = SEQ[i % SEQ.len()];
    shade(t, tone, sh)
}

// ── Line stroking (pattern-aware) ─────────────────────────────────────────────

/// Stroke a poly-line honouring the style's [`LinePattern`].
pub(crate) fn stroke_path(p: &egui::Painter, points: &[Pos2], color: Color32, st: &ChartStyle, width: f32) {
    let stroke = Stroke::new(width, color);
    match st.pattern {
        LinePattern::Solid => {
            for w in points.windows(2) { p.line_segment([w[0], w[1]], stroke); }
        }
        LinePattern::Dashed => {
            for w in points.windows(2) { dash_segment(p, w[0], w[1], stroke, 6.0, 4.0); }
        }
        LinePattern::Dotted => {
            let r = (width * 0.85).max(0.9);
            for w in points.windows(2) { dot_segment(p, w[0], w[1], color, r, r * 3.0); }
        }
    }
}

/// Dashed straight segment `a`→`b` (phase resets per segment — fine for charts).
fn dash_segment(p: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke, dash: f32, gap: f32) {
    let v = b - a;
    let len = v.length();
    if len < 1e-3 { return; }
    let dir = v / len;
    let step = dash + gap;
    let mut d = 0.0;
    while d < len {
        let s = a + dir * d;
        let e = a + dir * (d + dash).min(len);
        p.line_segment([s, e], stroke);
        d += step;
    }
}

/// Dotted straight segment `a`→`b` as evenly-spaced filled circles.
fn dot_segment(p: &egui::Painter, a: Pos2, b: Pos2, color: Color32, r: f32, spacing: f32) {
    let v = b - a;
    let len = v.length();
    if len < 1e-3 { return; }
    let dir = v / len;
    let mut d = 0.0;
    while d <= len {
        p.circle_filled(a + dir * d, r, color);
        d += spacing.max(1.0);
    }
}
