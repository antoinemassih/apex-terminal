//! Sparkline — compact time-series mini-chart primitive.
//!
//! Two render styles:
//! - `SparkStyle::Line` — connects data points with `line_segment` calls in a
//!   single colour. Requires ≥2 points; fewer points are a no-op.
//! - `SparkStyle::Bars` — one `rect_filled` bar per value, rising from the
//!   bottom. Works with ≥1 point. An optional per-value colour callback lets
//!   callers tint bars by threshold (e.g. bull/warn/bear in the perf HUD).
//!
//! Use `show(ui, theme)` to allocate space in a layout, or `paint(painter, rect)`
//! to draw into a pre-allocated rect (e.g. from a table column renderer — no
//! per-call allocations on the paint path).
//!
//! API:
//!   Sparkline::new(&values).color(c).size(60.0, 12.0).show(ui, theme);
//!   Sparkline::new(&values).bars().bar_color(&|v| threshold_color(v)).paint(painter, rect);

use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Vec2};

use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style::stroke_std;

/// Which visual encoding to use.
pub enum SparkStyle {
    /// Connect data points with line segments (one colour).
    Line,
    /// One filled bar per value, rising from the bottom of the rect.
    Bars,
}

/// Compact time-series mini-chart primitive. Builder pattern; call
/// [`show`](Sparkline::show) or [`paint`](Sparkline::paint) to render.
#[must_use = "Sparkline does nothing until `.show(ui, theme)` or `.paint(painter, rect)` is called"]
pub struct Sparkline<'a> {
    values: &'a [f32],
    style: SparkStyle,
    color: Color32,
    bar_color: Option<&'a dyn Fn(f32) -> Color32>,
    size: Vec2,
}

impl<'a> Sparkline<'a> {
    /// Create a `Line`-style sparkline with a default 32×12 size.
    /// Use `.bars()` to switch style, `.size(w, h)` to resize.
    pub fn new(values: &'a [f32]) -> Self {
        Self {
            values,
            style: SparkStyle::Line,
            color: Color32::GRAY,
            bar_color: None,
            size: Vec2::new(32.0, 12.0),
        }
    }

    /// Switch to bars style.
    pub fn bars(mut self) -> Self {
        self.style = SparkStyle::Bars;
        self
    }

    /// Set the line colour (Line style) or the default bar colour (Bars style
    /// when no `bar_color` callback is provided).
    pub fn color(mut self, c: Color32) -> Self {
        self.color = c;
        self
    }

    /// Per-value colour callback for Bars style. The closure receives each
    /// normalised value (0.0–1.0 within the data range) and returns a colour.
    /// Ignored in Line style.
    pub fn bar_color(mut self, f: &'a dyn Fn(f32) -> Color32) -> Self {
        self.bar_color = Some(f);
        self
    }

    /// Override the default 32×12 canvas size.
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.size = Vec2::new(w, h);
        self
    }

    /// Allocate a rect inside `ui` and paint the sparkline into it.
    /// Returns the egui `Response` for the allocated area.
    pub fn show(self, ui: &mut egui::Ui, _theme: &dyn ComponentTheme) -> Response {
        let (rect, response) = ui.allocate_exact_size(self.size, Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            self.paint_in(&painter, rect);
        }
        response
    }

    /// Paint into an existing `painter` at a given `rect`.
    /// Does not allocate any heap memory during the call.
    pub fn paint(self, painter: &egui::Painter, rect: Rect) {
        self.paint_in(painter, rect);
    }

    // ── internal ──────────────────────────────────────────────────────────────

    fn paint_in(self, painter: &egui::Painter, rect: Rect) {
        let values = self.values;
        if values.is_empty() {
            return;
        }

        // Compute min/max; guard zero-range.
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for &v in values {
            if v < lo { lo = v; }
            if v > hi { hi = v; }
        }
        let span = (hi - lo).max(1e-6);

        let w = rect.width();
        let h = rect.height();

        match self.style {
            SparkStyle::Line => {
                let n = values.len();
                if n < 2 {
                    return;
                }
                let stroke = Stroke::new(stroke_std(), self.color);
                for j in 1..n {
                    let t0 = (j - 1) as f32 / (n - 1) as f32;
                    let t1 = j as f32 / (n - 1) as f32;
                    let x0 = rect.left() + t0 * w;
                    let y0 = rect.bottom() - (values[j - 1] - lo) / span * h;
                    let x1 = rect.left() + t1 * w;
                    let y1 = rect.bottom() - (values[j] - lo) / span * h;
                    painter.line_segment(
                        [Pos2::new(x0, y0), Pos2::new(x1, y1)],
                        stroke,
                    );
                }
            }
            SparkStyle::Bars => {
                let n = values.len();
                let bar_w = (w / n as f32).max(1.0);
                for (i, &v) in values.iter().enumerate() {
                    let norm = (v - lo) / span;
                    let bar_h = (norm * h).max(1.0);
                    let x = rect.left() + i as f32 * bar_w;
                    let col = match self.bar_color {
                        Some(f) => f(v),
                        None => self.color,
                    };
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(x, rect.bottom() - bar_h),
                            Vec2::new((bar_w - 0.5).max(0.5), bar_h),
                        ),
                        0.0,
                        col,
                    );
                }
            }
        }
    }
}
