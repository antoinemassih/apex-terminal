//! Stepper — numbered step indicator for wizards and onboarding.
//!
//! API:
//!   let steps = ["Account", "Connect Broker", "Set Risk", "Done"];
//!   ui.add(Stepper::new(&steps, current_step));
//!
//!   Stepper::new(&steps, 2)
//!     .vertical(true)
//!     .show_labels(true)
//!     .show(ui, theme);

use egui::{Color32, FontId, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item, Justify as FlexJustify};
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::Size;
use crate::ui_kit::tokens as st;
use crate::ui_kit::icons::Icon;

#[must_use = "Stepper does nothing until `.show(ui, theme)` or `ui.add(stepper)` is called"]
pub struct Stepper<'a> {
    steps: &'a [&'a str],
    current: usize,
    vertical: bool,
    show_labels: bool,
    size: Size,
}

impl<'a> Stepper<'a> {
    pub fn new(steps: &'a [&'a str], current: usize) -> Self {
        Self {
            steps,
            current,
            vertical: false,
            show_labels: true,
            size: Size::Md,
        }
    }

    pub fn vertical(mut self, v: bool) -> Self { self.vertical = v; self }
    pub fn show_labels(mut self, v: bool) -> Self { self.show_labels = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
        let Stepper { steps, current, vertical, show_labels, size } = self;
        if steps.is_empty() {
            let (_, r) = ui.allocate_exact_size(Vec2::ZERO, Sense::hover());
            return r;
        }

        let circle_d = match size { Size::Xs => 18.0, Size::Sm => 22.0, Size::Md => 26.0, Size::Lg | Size::Xl => 32.0 };
        let label_font = crate::ui_kit::style::prop_at(size.font_size());
        let num_font = crate::ui_kit::style::prop_at(circle_d * 0.45);
        let line_thickness = 2.0;

        let accent = palette_ct(theme).base(Tone::Accent);
        let dim = palette_ct(theme).base(Tone::Dim);
        let text = palette_ct(theme).base(Tone::Text);
        let line_completed = accent;
        let line_future = st::color_alpha(dim, crate::ui_kit::style::alpha_strong());

        if vertical {
            paint_vertical(ui, theme, steps, current, show_labels, circle_d, label_font, num_font,
                line_thickness, accent, dim, text, line_completed, line_future)
        } else {
            paint_horizontal(ui, theme, steps, current, show_labels, circle_d, label_font, num_font,
                line_thickness, accent, dim, text, line_completed, line_future)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_horizontal(
    ui: &mut Ui,
    _theme: &dyn ComponentTheme,
    steps: &[&str],
    current: usize,
    show_labels: bool,
    circle_d: f32,
    label_font: FontId,
    num_font: FontId,
    line_thickness: f32,
    accent: Color32,
    dim: Color32,
    text: Color32,
    line_completed: Color32,
    line_future: Color32,
) -> Response {
    let n = steps.len();
    let avail_w = ui.available_width();
    let label_h = if show_labels { label_font.size + 4.0 } else { 0.0 };
    let h = circle_d + label_h + 4.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::hover());

    if !ui.is_rect_visible(rect) { return response; }
    let painter = ui.painter_at(rect);
    let cy = rect.top() + circle_d * 0.5;

    // Compute circle centers evenly distributed.
    //
    // M4.3: `let step = (right - left) / (n - 1); … left + step * i` is
    // hand-rolled `justify-content: space-between` over `circle_d`-wide
    // children with a 4 px end inset — which is what the flex row below says
    // directly. (The single-step case is `Justify::Center`, matching the old
    // `rect.center().x` branch.)
    let centers: Vec<f32> = Flex::row()
        .padding_sides(4.0, 4.0, 0.0, 0.0)
        .justify(if n == 1 { FlexJustify::Center } else { FlexJustify::SpaceBetween })
        .align(FlexAlign::Start)
        .items((0..n).map(|_| Item::fixed(circle_d)))
        .solve(rect.size())
        .iter()
        .map(|r| rect.left() + r.center().x)
        .collect();

    // Connector lines first.
    for i in 0..n.saturating_sub(1) {
        let c0 = Pos2::new(centers[i] + circle_d * 0.5, cy);
        let c1 = Pos2::new(centers[i + 1] - circle_d * 0.5, cy);
        let col = if i + 1 <= current { line_completed } else { line_future };
        painter.line_segment([c0, c1], Stroke::new(line_thickness, col));
    }

    // Circles + labels.
    for (i, label) in steps.iter().enumerate() {
        let center = Pos2::new(centers[i], cy);
        paint_circle(&painter, center, circle_d, i, current, &num_font, accent, dim);
        if show_labels {
            let col = if i == current { text } else if i < current { text } else { dim };
            // Bound the label to its fair share of the strip.
            //
            // It was painted CENTER_TOP at `center.x` with no width at all, and
            // the circles are `Item::fixed(circle_d)` in a space-between row —
            // so their spacing is driven by the CIRCLE width and the label has
            // no say in it. On a 150px strip, a four-step "Configure /
            // Validate / Deploy / Verify" put "Configure" at x = -3.2 (outside
            // the widget) overlapping "Validate" by 7px. Nothing in the layout
            // could notice, because nothing measured the label.
            //
            // `tabs.rs` already ellipsized its labels for the same reason; the
            // helper was private to that file, so this one went without.
            let max_label_w = (rect.width() / n as f32 - crate::ui_kit::style::gap_xs()).max(0.0);
            let shown = crate::ui_kit::style::ellipsize_to(
                &painter, label, &label_font, max_label_w, col);
            painter.text(
                Pos2::new(center.x, rect.top() + circle_d + 4.0),
                egui::Align2::CENTER_TOP,
                shown,
                label_font.clone(),
                col,
            );
        }
    }

    response
}

#[allow(clippy::too_many_arguments)]
fn paint_vertical(
    ui: &mut Ui,
    _theme: &dyn ComponentTheme,
    steps: &[&str],
    current: usize,
    show_labels: bool,
    circle_d: f32,
    label_font: FontId,
    num_font: FontId,
    line_thickness: f32,
    accent: Color32,
    dim: Color32,
    text: Color32,
    line_completed: Color32,
    line_future: Color32,
) -> Response {
    let n = steps.len();
    let row_h = circle_d + 8.0;
    let total_h = row_h * n as f32;
    let label_x_offset = circle_d + st::gap_sm();
    let avail_w = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(avail_w, total_h), Sense::hover());

    if !ui.is_rect_visible(rect) { return response; }
    let painter = ui.painter_at(rect);
    let cx = rect.left() + circle_d * 0.5 + 2.0;

    // M4.3: `rect.top() + row_h * i` (repeated in both loops below) is a
    // uniform column stack — one flex column of `row_h` children.
    let rows: Vec<f32> = Flex::column()
        .items((0..n).map(|_| Item::fixed(row_h)))
        .solve(rect.size())
        .iter()
        .map(|r| rect.top() + r.top())
        .collect();

    // Connector lines between rows.
    for i in 0..n.saturating_sub(1) {
        let y0 = rows[i] + circle_d + 1.0;
        let y1 = rows[i + 1] - 1.0;
        let col = if i + 1 <= current { line_completed } else { line_future };
        painter.line_segment([Pos2::new(cx, y0), Pos2::new(cx, y1)], Stroke::new(line_thickness, col));
    }

    for (i, label) in steps.iter().enumerate() {
        let cy = rows[i] + circle_d * 0.5;
        let center = Pos2::new(cx, cy);
        paint_circle(&painter, center, circle_d, i, current, &num_font, accent, dim);
        if show_labels {
            let col = if i == current { text } else if i < current { text } else { dim };
            painter.text(
                Pos2::new(rect.left() + label_x_offset, cy),
                egui::Align2::LEFT_CENTER,
                label,
                label_font.clone(),
                col,
            );
        }
    }

    response
}

fn paint_circle(
    painter: &egui::Painter,
    center: Pos2,
    diameter: f32,
    idx: usize,
    current: usize,
    num_font: &FontId,
    accent: Color32,
    dim: Color32,
) {
    let r = diameter * 0.5;
    if idx < current {
        // Completed: filled accent, white check.
        painter.circle_filled(center, r, accent);
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            Icon::CHECK,
            crate::ui_kit::style::prop_at(diameter * 0.55),
            st::contrast_fg(accent),
        );
    } else if idx == current {
        // Current: filled accent, white number, slightly larger ring.
        painter.circle_filled(center, r, accent);
        painter.circle_stroke(center, r + 1.5, Stroke::new(st::stroke_bold(), st::color_alpha(accent, crate::ui_kit::style::alpha_heavy())));
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            format!("{}", idx + 1),
            num_font.clone(),
            st::contrast_fg(accent),
        );
    } else {
        // Future: transparent fill, dim border, dim number.
        painter.circle_stroke(center, r, Stroke::new(st::stroke_std(), dim));
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            format!("{}", idx + 1),
            num_font.clone(),
            dim,
        );
    }
}

impl<'a> Widget for Stepper<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::widgets::paint_probe;
    use crate::ui_kit::widgets::theme::PortableTheme;

    /// Step labels sit under evenly-spaced circles, centred on each. The
    /// circles are `Item::fixed(circle_d)` in a `space-between` row, so their
    /// spacing is driven by the CIRCLE width — the label width has no say in
    /// it. Long labels on a narrow strip therefore run into each other, and
    /// nothing in the layout can notice.
    ///
    /// Narrow glyphs are included because a proportional measure of `i` is a
    /// sliver: a test written only with wide text would pass while the real
    /// failure sits at the other end (see `Select`, AT-192).
    /// Two widths. 360 is a comfortable dialog; 150 is the one that used to
    /// break — four word-labels there put "Configure" at x = -3.2, outside the
    /// widget, overlapping "Validate" by 7px. Testing only the comfortable
    /// width would have passed against the unbounded label paint.
    const STRIP_WIDTHS: [f32; 3] = [360.0, 220.0, 150.0];

    #[test]
    fn step_labels_do_not_collide() {
        for steps in [
            &["One", "Two", "Three"][..],
            &["Configure", "Validate", "Deploy", "Verify"][..],
            &["iiiiiiiiii", "iiiiiiiiii", "iiiiiiiiii"][..],
            &["A", "B"][..],
        ] {
            for (size, strip_w) in [Size::Xs, Size::Sm, Size::Md, Size::Lg]
                .into_iter()
                .flat_map(|s| STRIP_WIDTHS.into_iter().map(move |w| (s, w)))
            {
                // Constrained to a realistic strip width. The probe's panel is
                // effectively unbounded, and a `space-between` row spreads the
                // circles ~5000px apart there — no two labels could collide, so
                // the assertion would hold for any widget whatsoever.
                let runs = paint_probe::probe(|ui| {
                    let t = PortableTheme::dark();
                    let rect = egui::Rect::from_min_size(
                        ui.max_rect().min, egui::vec2(strip_w, 80.0));
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    Stepper::new(steps, 1).size(size).show_labels(true).show(&mut child, &t);
                });
                if runs.is_empty() {
                    continue;
                }
                paint_probe::assert_no_overlap(
                    &format!("stepper {steps:?} {size:?} w={strip_w}"), &runs);
            }
        }
    }
}
