//! Builder primitives — layout family.
//!
//! Wraps existing layout helpers (`style::split_divider`,
//! `components::empty_state_panel`, etc.) as chained-setter builders. The
//! legacy free-functions are NOT modified — these builders delegate to the
//! same paint code so output is byte-for-byte identical.
//!
//! Visual rule: CHART PAINT IS SACRED. None of these touch the chart canvas;
//! they only compose UI chrome around it.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, Sense, Ui, Vec2};
use super::super::style::{
    self, split_divider, gap_sm, gap_md, gap_lg, GAP_SM, GAP_MD, GAP_LG,
};
use super::super::components::empty_state_panel;

type Theme = crate::chart_renderer::gpu::Theme;

// ─── Splitter ─────────────────────────────────────────────────────────────────

/// Orientation for `Splitter`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SplitOrientation {
    Horizontal, // divider runs horizontally; drag is vertical (default)
    Vertical,   // divider runs vertically; drag is horizontal
}

/// Wrap `style::split_divider` as a builder. Returns the drag delta along the
/// drag-axis (positive = down for Horizontal, positive = right for Vertical).
///
/// ```ignore
/// let dy = Splitter::new("sdiv_0").theme(t).show(ui);
/// ```
pub struct Splitter<'a> {
    id_salt: &'a str,
    dim: Color32,
    orient: SplitOrientation,
}

impl<'a> Splitter<'a> {
    pub fn new(id_salt: &'a str) -> Self {
        Self {
            id_salt,
            dim: Color32::GRAY,
            orient: SplitOrientation::Horizontal,
        }
    }

    pub fn dim(mut self, dim: Color32) -> Self { self.dim = dim; self }
    pub fn theme(mut self, t: &Theme) -> Self { self.dim = t.dim; self }
    pub fn horizontal(mut self) -> Self { self.orient = SplitOrientation::Horizontal; self }
    pub fn vertical(mut self) -> Self { self.orient = SplitOrientation::Vertical; self }
    pub fn orientation(mut self, o: SplitOrientation) -> Self { self.orient = o; self }

    /// Run the splitter and return drag delta along the drag axis.
    pub fn show(self, ui: &mut Ui) -> f32 {
        match self.orient {
            // 1:1 with style::split_divider for the horizontal case.
            SplitOrientation::Horizontal => split_divider(ui, self.id_salt, self.dim),
            // Vertical variant: same look, rotated. Mirrors split_divider body.
            SplitOrientation::Vertical => vertical_split_divider(ui, self.id_salt, self.dim),
        }
    }
}

/// Vertical-divider sibling of `style::split_divider` — mirrors its body but
/// the divider runs vertically and the drag axis is horizontal.
fn vertical_split_divider(ui: &mut Ui, _id_salt: &str, dim: Color32) -> f32 {
    use egui::Stroke;
    let div_w = crate::dt_f32!(split_divider.height, 6.0);
    let inset = crate::dt_f32!(split_divider.inset, 8.0);
    let dot_r = crate::dt_f32!(split_divider.dot_radius, 1.5);
    let dot_sp = crate::dt_f32!(split_divider.dot_spacing, 8.0);
    let active_sw = crate::dt_f32!(split_divider.active_stroke, 2.0);
    let inactive_sw = crate::dt_f32!(split_divider.inactive_stroke, 1.0);

    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(div_w, ui.available_height()),
        Sense::drag(),
    );
    let p = ui.painter();
    let active = resp.hovered() || resp.dragged();
    let color = if active {
        dim.gamma_multiply(0.6)
    } else {
        style::color_alpha(dim, style::alpha_faint())
    };

    p.line_segment(
        [
            egui::pos2(rect.center().x, rect.top() + inset),
            egui::pos2(rect.center().x, rect.bottom() - inset),
        ],
        Stroke::new(if active { active_sw } else { inactive_sw }, color),
    );

    if active {
        let cx = rect.center().x;
        let cy = rect.center().y;
        for dy in [-dot_sp, 0.0, dot_sp] {
            p.circle_filled(egui::pos2(cx, cy + dy), dot_r, dim.gamma_multiply(0.4));
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    if resp.dragged() { resp.drag_delta().x } else { 0.0 }
}

// ─── ResizableSplit ───────────────────────────────────────────────────────────

/// A two-pane split container with a draggable divider that mutates a
/// fraction in `[0.05, 0.95]`.
///
/// ```ignore
/// ResizableSplit::horizontal(&mut state.frac, "watchlist_split")
///     .theme(t)
///     .show(ui, |ui| draw_top(ui), |ui| draw_bottom(ui));
/// ```
pub struct ResizableSplit<'a> {
    frac: &'a mut f32,
    id_salt: &'a str,
    dim: Color32,
    orient: SplitOrientation,
    min_frac: f32,
    max_frac: f32,
}

impl<'a> ResizableSplit<'a> {
    pub fn horizontal(frac: &'a mut f32, id_salt: &'a str) -> Self {
        Self {
            frac, id_salt,
            dim: Color32::GRAY,
            orient: SplitOrientation::Horizontal,
            min_frac: 0.05, max_frac: 0.95,
        }
    }
    pub fn vertical(frac: &'a mut f32, id_salt: &'a str) -> Self {
        Self {
            frac, id_salt,
            dim: Color32::GRAY,
            orient: SplitOrientation::Vertical,
            min_frac: 0.05, max_frac: 0.95,
        }
    }

    pub fn theme(mut self, t: &Theme) -> Self { self.dim = t.dim; self }
    pub fn dim(mut self, dim: Color32) -> Self { self.dim = dim; self }
    pub fn clamp(mut self, lo: f32, hi: f32) -> Self {
        self.min_frac = lo; self.max_frac = hi; self
    }

    pub fn show<F1, F2, R1, R2>(self, ui: &mut Ui, top: F1, bot: F2) -> (R1, R2)
    where
        F1: FnOnce(&mut Ui) -> R1,
        F2: FnOnce(&mut Ui) -> R2,
    {
        let avail = ui.available_size_before_wrap();
        match self.orient {
            SplitOrientation::Horizontal => {
                let total_h = avail.y.max(1.0);
                let top_h = (total_h * *self.frac).clamp(8.0, total_h - 8.0);
                let r1 = ui.allocate_ui(egui::vec2(avail.x, top_h), |ui| top(ui)).inner;
                let d = split_divider(ui, self.id_salt, self.dim);
                if d != 0.0 {
                    *self.frac = (*self.frac + d / total_h).clamp(self.min_frac, self.max_frac);
                }
                let r2 = ui.allocate_ui(egui::vec2(avail.x, ui.available_height()), |ui| bot(ui)).inner;
                (r1, r2)
            }
            SplitOrientation::Vertical => {
                let total_w = avail.x.max(1.0);
                let left_w = (total_w * *self.frac).clamp(8.0, total_w - 8.0);
                let mut r1: Option<R1> = None;
                let mut r2: Option<R2> = None;
                ui.horizontal(|ui| {
                    r1 = Some(ui.allocate_ui(egui::vec2(left_w, avail.y), |ui| top(ui)).inner);
                    let d = vertical_split_divider(ui, self.id_salt, self.dim);
                    if d != 0.0 {
                        *self.frac = (*self.frac + d / total_w).clamp(self.min_frac, self.max_frac);
                    }
                    r2 = Some(ui.allocate_ui(egui::vec2(ui.available_width(), avail.y), |ui| bot(ui)).inner);
                });
                (r1.expect("split top"), r2.expect("split bot"))
            }
        }
    }
}

// ─── Collapsible ──────────────────────────────────────────────────────────────

/// Single header + body with a `&mut bool` expanded state and a chevron toggle.
pub struct Collapsible<'a> {
    title: &'a str,
    expanded: &'a mut bool,
    title_color: Color32,
    rule_color: Color32,
}

impl<'a> Collapsible<'a> {
    pub fn new(title: &'a str, expanded: &'a mut bool) -> Self {
        Self { title, expanded, title_color: Color32::GRAY, rule_color: Color32::GRAY }
    }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.title_color = t.text;
        self.rule_color = t.dim;
        self
    }
    pub fn colors(mut self, title: Color32, rule: Color32) -> Self {
        self.title_color = title; self.rule_color = rule; self
    }

    pub fn show<R>(self, ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        let chev = if *self.expanded { "▾" } else { "▸" };
        let header = format!("{}  {}", chev, self.title);
        let resp = ui.add(
            egui::Label::new(
                egui::RichText::new(header)
                    .monospace()
                    .strong()
                    .color(self.title_color),
            )
            .sense(Sense::click()),
        );
        if resp.clicked() {
            *self.expanded = !*self.expanded;
        }
        ui.add_space(gap_sm());
        if *self.expanded {
            Some(body(ui))
        } else {
            None
        }
    }
}

// ─── Accordion ────────────────────────────────────────────────────────────────

/// One section of an `Accordion`. The body closure runs only when expanded.
pub struct Section<'a, F> {
    pub title: &'a str,
    pub expanded: &'a mut bool,
    pub body: F,
}

impl<'a, F> Section<'a, F> {
    pub fn new(title: &'a str, expanded: &'a mut bool, body: F) -> Self {
        Self { title, expanded, body }
    }
}

/// List-of-sections accordion. Each section gets a chevron header.
pub struct Accordion {
    title_color: Color32,
    rule_color: Color32,
}

impl Accordion {
    pub fn new() -> Self {
        Self { title_color: Color32::GRAY, rule_color: Color32::GRAY }
    }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.title_color = t.text; self.rule_color = t.dim; self
    }
    pub fn colors(mut self, title: Color32, rule: Color32) -> Self {
        self.title_color = title; self.rule_color = rule; self
    }

    /// Render a list of sections; each is collapsible independently.
    pub fn show<'a, F: FnOnce(&mut Ui)>(self, ui: &mut Ui, sections: Vec<Section<'a, F>>) {
        for s in sections {
            Collapsible::new(s.title, s.expanded)
                .colors(self.title_color, self.rule_color)
                .show(ui, s.body);
            ui.add_space(gap_sm());
        }
    }
}

impl Default for Accordion { fn default() -> Self { Self::new() } }

// ─── EmptyState ───────────────────────────────────────────────────────────────

/// Builder around `components::empty_state_panel` — icon + title + subtitle,
/// plus an optional action button rendered below.
pub struct EmptyState<'a> {
    icon: &'a str,
    title: &'a str,
    subtitle: &'a str,
    dim: Color32,
    action: Option<&'a str>,
}

impl<'a> EmptyState<'a> {
    pub fn new(icon: &'a str, title: &'a str, subtitle: &'a str) -> Self {
        Self { icon, title, subtitle, dim: Color32::GRAY, action: None }
    }
    pub fn theme(mut self, t: &Theme) -> Self { self.dim = t.dim; self }
    pub fn dim(mut self, dim: Color32) -> Self { self.dim = dim; self }
    pub fn action(mut self, label: &'a str) -> Self { self.action = Some(label); self }

    /// Returns Some(true) if the action button was clicked, Some(false) if
    /// rendered but not clicked, None if no action button was configured.
    pub fn show(self, ui: &mut Ui) -> Option<bool> {
        empty_state_panel(ui, self.icon, self.title, self.subtitle, self.dim);
        self.action.map(|label| {
            ui.add_space(gap_md());
            let mut clicked = false;
            ui.vertical_centered(|ui| {
                if ui.button(label).clicked() { clicked = true; }
            });
            clicked
        })
    }
}

// ─── Stack ────────────────────────────────────────────────────────────────────

/// Vertical stack with a consistent inter-child spacing.
pub struct Stack {
    gap: f32,
}

impl Stack {
    pub fn new() -> Self { Self { gap: GAP_MD } }
    pub fn gap(mut self, px: f32) -> Self { self.gap = px; self }
    pub fn small(mut self) -> Self { self.gap = GAP_SM; self }
    pub fn medium(mut self) -> Self { self.gap = GAP_MD; self }
    pub fn large(mut self) -> Self { self.gap = GAP_LG; self }

    /// Run a sequence of child closures, inserting `gap` space between each.
    pub fn show(self, ui: &mut Ui, children: Vec<Box<dyn FnOnce(&mut Ui) + '_>>) {
        ui.vertical(|ui| {
            let n = children.len();
            for (i, child) in children.into_iter().enumerate() {
                child(ui);
                if i + 1 < n {
                    ui.add_space(self.gap);
                }
            }
        });
    }
}

impl Default for Stack { fn default() -> Self { Self::new() } }

// ─── Cluster ──────────────────────────────────────────────────────────────────

/// Horizontal flow with wrap — used for chip/tag rows.
pub struct Cluster {
    gap: f32,
}

impl Cluster {
    pub fn new() -> Self { Self { gap: GAP_SM } }
    pub fn gap(mut self, px: f32) -> Self { self.gap = px; self }
    pub fn small(mut self) -> Self { self.gap = GAP_SM; self }
    pub fn medium(mut self) -> Self { self.gap = GAP_MD; self }
    pub fn large(mut self) -> Self { self.gap = GAP_LG; self }

    pub fn show(self, ui: &mut Ui, children: Vec<Box<dyn FnOnce(&mut Ui) + '_>>) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = self.gap;
            for child in children {
                child(ui);
            }
        });
    }
}

impl Default for Cluster { fn default() -> Self { Self::new() } }

// ─── Center ───────────────────────────────────────────────────────────────────

/// Centers a child in available space horizontally and/or vertically.
pub struct Center {
    horizontal: bool,
    vertical: bool,
}

impl Center {
    pub fn new() -> Self { Self { horizontal: true, vertical: true } }
    pub fn horizontal_only(mut self) -> Self { self.horizontal = true; self.vertical = false; self }
    pub fn vertical_only(mut self) -> Self { self.horizontal = false; self.vertical = true; self }
    pub fn both(mut self) -> Self { self.horizontal = true; self.vertical = true; self }

    pub fn show<R>(self, ui: &mut Ui, child: impl FnOnce(&mut Ui) -> R) -> R {
        match (self.horizontal, self.vertical) {
            (true, true) => {
                let avail = ui.available_size_before_wrap();
                let mut out: Option<R> = None;
                ui.allocate_ui_with_layout(
                    avail,
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| { out = Some(child(ui)); },
                );
                out.expect("Center body")
            }
            (true, false) => {
                let mut out: Option<R> = None;
                ui.vertical_centered(|ui| { out = Some(child(ui)); });
                out.expect("Center body")
            }
            (false, true) => {
                let avail = ui.available_size_before_wrap();
                let mut out: Option<R> = None;
                ui.allocate_ui_with_layout(
                    avail,
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| { out = Some(child(ui)); },
                );
                out.expect("Center body")
            }
            (false, false) => child(ui),
        }
    }
}

impl Default for Center { fn default() -> Self { Self::new() } }

// ─── Spacer ───────────────────────────────────────────────────────────────────

/// Reserves space along the current layout axis. `.size(px)` for a fixed
/// gap, `.fill()` to push siblings to opposite ends.
pub struct Spacer {
    size: Option<f32>,
    fill: bool,
}

impl Spacer {
    pub fn new() -> Self { Self { size: None, fill: false } }
    pub fn size(mut self, px: f32) -> Self { self.size = Some(px); self.fill = false; self }
    pub fn fill(mut self) -> Self { self.fill = true; self.size = None; self }

    pub fn show(self, ui: &mut Ui) {
        if self.fill {
            // Push to far end of current layout: allocate remaining space.
            let avail = ui.available_size_before_wrap();
            ui.allocate_space(avail);
        } else if let Some(px) = self.size {
            ui.add_space(px);
        }
    }
}

impl Default for Spacer { fn default() -> Self { Self::new() } }
