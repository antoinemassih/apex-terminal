//! Tabs — horizontal tab strip with multiple visual treatments.
//!
//! Treatments:
//!   - Line:       active tab gets a 2px underline, inactive tabs are flat
//!   - Segmented:  tab strip looks like a pill-grouped button bar
//!   - Filled:     active tab gets a surface fill, inactive transparent
//!
//! Optional features:
//!   - closable: each tab has an X icon (chart panes use this)
//!   - reorderable: drag to reorder
//!   - addable: trailing + button
//!
//! API:
//! ```ignore
//!   let mut active: usize = 0;
//!   let labels = ["AAPL", "SPY", "QQQ"];
//!   Tabs::new(&mut active, &labels).show(ui, theme);
//!
//!   let mut items: Vec<TabItem> = vec![TabItem::new("AAPL")];
//!   let mut active: usize = 0;
//!   let resp = Tabs::with_items(&mut active, &mut items)
//!       .treatment(TabTreatment::Line)
//!       .closable(true)
//!       .reorderable(true)
//!       .addable(true)
//!       .show(ui, theme);
//!   if resp.add_clicked { items.push(TabItem::new("New")); }
//!   for closed_idx in resp.closed.iter().rev() { items.remove(*closed_idx); }
//! ```

use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2,
};

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabTreatment {
    #[default]
    Line,
    Segmented,
    Filled,
    /// Browser-tab look: active tab gets top + left + right hairline borders
    /// and a subtle surface fill. No bottom border, so the tab visually merges
    /// with the content panel below. Inactive tabs are flat/transparent.
    Card,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Debug)]
pub struct TabItem {
    pub label: String,
    pub icon: Option<&'static str>,
    pub badge: Option<u32>,
    pub modified: bool,
    /// Per-item override for the closable flag. `None` defers to the Tabs builder.
    pub closable: Option<bool>,
    pub disabled: bool,
}

impl TabItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            badge: None,
            modified: false,
            closable: None,
            disabled: false,
        }
    }
    pub fn icon(mut self, icon: &'static str) -> Self { self.icon = Some(icon); self }
    pub fn badge(mut self, n: u32) -> Self { self.badge = Some(n); self }
    pub fn modified(mut self, v: bool) -> Self { self.modified = v; self }
    pub fn closable(mut self, v: bool) -> Self { self.closable = Some(v); self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
}

pub struct TabsResponse {
    pub response: Response,
    pub changed: bool,
    pub closed: Vec<usize>,
    pub add_clicked: bool,
    pub reordered: Option<(usize, usize)>,
}

// ── Internal source enum (labels vs items) ─────────────────────────────────────

enum Source<'a> {
    Labels(&'a [&'a str]),
    Items(&'a mut Vec<TabItem>),
}

impl<'a> Source<'a> {
    fn len(&self) -> usize {
        match self {
            Source::Labels(s) => s.len(),
            Source::Items(v) => v.len(),
        }
    }
    fn snapshot(&self) -> Vec<TabItem> {
        match self {
            Source::Labels(s) => s.iter().map(|l| TabItem::new(*l)).collect(),
            Source::Items(v) => v.iter().cloned().collect(),
        }
    }
}

// ── Builder ────────────────────────────────────────────────────────────────────

pub struct Tabs<'a> {
    active: &'a mut usize,
    source: Source<'a>,
    treatment: TabTreatment,
    size: Size,
    closable: bool,
    reorderable: bool,
    addable: bool,
    full_width: bool,
    align: TabAlign,
    id_salt: Option<&'a str>,
}

impl<'a> Tabs<'a> {
    pub fn new(active: &'a mut usize, labels: &'a [&'a str]) -> Self {
        Self {
            active,
            source: Source::Labels(labels),
            treatment: TabTreatment::default(),
            size: Size::Md,
            closable: false,
            reorderable: false,
            addable: false,
            full_width: false,
            align: TabAlign::default(),
            id_salt: None,
        }
    }

    pub fn with_items(active: &'a mut usize, items: &'a mut Vec<TabItem>) -> Self {
        Self {
            active,
            source: Source::Items(items),
            treatment: TabTreatment::default(),
            size: Size::Md,
            closable: false,
            reorderable: false,
            addable: false,
            full_width: false,
            align: TabAlign::default(),
            id_salt: None,
        }
    }

    pub fn treatment(mut self, t: TabTreatment) -> Self { self.treatment = t; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn reorderable(mut self, v: bool) -> Self { self.reorderable = v; self }
    pub fn addable(mut self, v: bool) -> Self { self.addable = v; self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    pub fn align(mut self, a: TabAlign) -> Self { self.align = a; self }
    pub fn id_salt(mut self, s: &'a str) -> Self { self.id_salt = Some(s); self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> TabsResponse {
        paint_tabs(self, ui, theme)
    }
}

// ── Drag state stored in egui memory ───────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default)]
struct DragState {
    /// Index of tab currently being dragged.
    from: usize,
    /// Pointer x at drag start, relative to strip origin.
    start_x: f32,
    /// Live current pointer x.
    current_x: f32,
    /// True once threshold crossed and we're actually dragging.
    active: bool,
}

// ── Painting ───────────────────────────────────────────────────────────────────

const DRAG_THRESHOLD: f32 = 8.0;
// Min squeeze width — small enough that short labels (LIST, HEAT, Chart, etc.)
// don't get ellipsized when the tab strip overflows. Long labels still get
// truncated when there's truly not enough room.
const TAB_MIN_WIDTH: f32 = 40.0;
const CLOSE_HIT: f32 = 16.0;
const CLOSE_VIS: f32 = 11.0;
const MOD_DOT_R: f32 = 3.0;

fn paint_tabs(
    tabs: Tabs<'_>,
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
) -> TabsResponse {
    let Tabs {
        active,
        mut source,
        treatment,
        size,
        closable: closable_default,
        reorderable,
        addable,
        full_width,
        align,
        id_salt,
    } = tabs;

    let mut resp_out = TabsResponse {
        response: ui.allocate_response(Vec2::ZERO, Sense::hover()),
        changed: false,
        closed: Vec::new(),
        add_clicked: false,
        reordered: None,
    };

    let n = source.len();
    let snapshot = source.snapshot();

    let row_h = size.height();
    let pad_x = st::gap_sm();
    let inner_gap = st::gap_xs();
    let font_label = FontId::proportional(size.font_size());
    let font_icon = FontId::proportional(st::font_sm());

    // Outer id for stable animation/drag keys.
    let outer_id = ui.make_persistent_id(("ui_kit_tabs", id_salt.unwrap_or("default")));

    // Pre-compute each tab's natural width.
    let widths: Vec<f32> = (0..n)
        .map(|i| measure_tab_width(ui, &snapshot[i], &font_label, &font_icon,
            tab_is_closable(&snapshot[i], closable_default), inner_gap, pad_x))
        .collect();

    // Allocate strip rect.
    let avail = ui.available_rect_before_wrap();
    let row_w = avail.width();
    let total_natural: f32 = widths.iter().sum::<f32>()
        + if addable { row_h } else { 0.0 };

    // Tabs always render at their natural label width — never squeeze. If the
    // strip overflows the available row, the parent layout (a horizontal split,
    // panel, or scroll area) is responsible for clipping or scrolling. This
    // preserves full readable labels at the cost of potential horizontal
    // overflow when many tabs are present.
    let _must_scroll = full_width || addable;
    let effective_widths = widths.clone();

    // Wrap in a horizontal scroll when overflow.
    let strip_total: f32 = effective_widths.iter().sum::<f32>()
        + if addable { row_h } else { 0.0 };
    let need_scroll = strip_total > row_w;

    // We render directly into the parent ui (no ScrollArea complication for now);
    // the active-tab auto-scroll-into-view is best-effort handled by egui's
    // scroll-to-rect when the active changes. If overflow happens without
    // scroll, tabs are simply clipped by parent; this matches existing tabs.rs.
    let _ = need_scroll;

    // Reserve full row for layout.
    let strip_w = if full_width { row_w } else { strip_total.min(row_w) };
    let (strip_rect, strip_resp) = ui.allocate_exact_size(
        Vec2::new(strip_w, row_h),
        Sense::click_and_drag(),
    );
    resp_out.response = strip_resp.clone();

    // Compute alignment offset.
    let mut x = match align {
        TabAlign::Start => strip_rect.left(),
        TabAlign::Center => strip_rect.left() + (strip_rect.width() - strip_total).max(0.0) * 0.5,
        TabAlign::End => strip_rect.right() - strip_total,
    };

    // Drag state load.
    let drag_id = outer_id.with("drag");
    let mut drag: Option<DragState> = ui.ctx().data(|d| d.get_temp::<DragState>(drag_id));

    let pointer = ui.ctx().pointer_latest_pos();
    let primary_down = ui.ctx().input(|i| i.pointer.primary_down());
    let primary_released = ui.ctx().input(|i| i.pointer.any_released() && !i.pointer.primary_down());

    // Compute base rects (un-shifted).
    let mut base_rects: Vec<Rect> = Vec::with_capacity(n);
    {
        let mut cx = x;
        for w in &effective_widths {
            let r = Rect::from_min_size(Pos2::new(cx, strip_rect.top()), Vec2::new(*w, row_h));
            base_rects.push(r);
            cx += *w;
        }
        x = cx; // x now points to end-of-tabs (where + button goes)
    }

    // Drag-reorder live displacement: figure the "drop index" based on pointer x.
    let mut drop_index: Option<usize> = None;
    if let (Some(state), Some(pp)) = (drag.as_ref(), pointer) {
        if state.active {
            let px = pp.x;
            // Drop index = first base rect whose center is right of pointer.
            let mut idx = n;
            for (i, r) in base_rects.iter().enumerate() {
                if px < r.center().x { idx = i; break; }
            }
            if idx > state.from { idx -= 1; } // collapse self-slot
            drop_index = Some(idx.min(n.saturating_sub(1)));
        }
    }

    // Painted rects (with displacement for non-dragged tabs).
    let displaced_rects: Vec<Rect> = base_rects
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut rect = *r;
            if let (Some(state), Some(drop_i)) = (drag.as_ref(), drop_index) {
                if state.active && i != state.from {
                    let from = state.from;
                    let to = drop_i;
                    let w = r.width();
                    // Animate sliding using ease_value for smoothness.
                    let target_dx: f32 = if from < i && i <= to {
                        -widths[from]
                    } else if to <= i && i < from {
                        widths[from]
                    } else { 0.0 };
                    let dx = motion::ease_value(
                        ui.ctx(),
                        outer_id.with(("slide", i)),
                        target_dx,
                        motion::FAST,
                    );
                    rect = rect.translate(Vec2::new(dx, 0.0));
                    let _ = w;
                } else if i != state.from {
                    // settle to 0 when no drop change
                    let dx = motion::ease_value(
                        ui.ctx(),
                        outer_id.with(("slide", i)),
                        0.0,
                        motion::FAST,
                    );
                    rect = rect.translate(Vec2::new(dx, 0.0));
                }
            } else {
                // settle to 0 when drag ends
                let dx = motion::ease_value(
                    ui.ctx(),
                    outer_id.with(("slide", i)),
                    0.0,
                    motion::FAST,
                );
                rect = rect.translate(Vec2::new(dx, 0.0));
            }
            rect
        })
        .collect();

    // Treatment-level wrapper background (Segmented).
    if matches!(treatment, TabTreatment::Segmented) && n > 0 {
        let total_rect = Rect::from_min_max(
            base_rects[0].min,
            base_rects[base_rects.len() - 1].max,
        );
        ui.painter().rect_filled(
            total_rect,
            CornerRadius::same(6),
            st::color_alpha(theme.surface(), 200),
        );
    }

    // Per-tab paint + interactions.
    let cur_active = (*active).min(n.saturating_sub(1));
    let mut new_active = cur_active;

    for i in 0..n {
        let item = &snapshot[i];
        let rect = displaced_rects[i];
        let is_active = i == cur_active;
        let is_dragging = drag.as_ref().map(|d| d.active && d.from == i).unwrap_or(false);

        let tab_id = outer_id.with(("tab", i));
        let tab_resp = ui.interact(rect, tab_id, Sense::click_and_drag());

        let hover_t = motion::ease_bool(ui.ctx(), tab_id.with("hov"),
            tab_resp.hovered() && !item.disabled, motion::FAST);
        let active_t = motion::ease_bool(ui.ctx(), tab_id.with("act"),
            is_active, motion::MED);

        // Click selection.
        if tab_resp.clicked() && !item.disabled {
            new_active = i;
        }

        // Drag start.
        if reorderable && !item.disabled && tab_resp.drag_started() {
            if let Some(p) = pointer {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(drag_id, DragState {
                        from: i,
                        start_x: p.x,
                        current_x: p.x,
                        active: false,
                    });
                });
                drag = Some(DragState { from: i, start_x: p.x, current_x: p.x, active: false });
            }
        }

        // Paint (unless this tab is being dragged — that's painted last as floating).
        if !is_dragging {
            paint_one_tab(
                ui, theme, treatment, rect, item, is_active, hover_t, active_t,
                &font_label, &font_icon, inner_gap, pad_x,
            );

            // Close button hit-test.
            let closable = tab_is_closable(item, closable_default);
            if closable {
                let close_visible = is_active || tab_resp.hovered();
                let close_t = motion::ease_bool(
                    ui.ctx(), tab_id.with("close"), close_visible, motion::FAST,
                );
                let close_center = Pos2::new(rect.right() - pad_x - CLOSE_VIS * 0.5,
                    rect.center().y);
                let close_rect = Rect::from_center_size(close_center, Vec2::splat(CLOSE_HIT));
                let close_resp = ui.interact(close_rect, tab_id.with("close_btn"), Sense::click());
                if close_t > 0.01 {
                    let base = if close_resp.hovered() { theme.text() } else { theme.dim() };
                    let col = Color32::from_rgba_premultiplied(
                        base.r(), base.g(), base.b(),
                        (base.a() as f32 * close_t).round() as u8,
                    );
                    ui.painter().text(
                        close_center,
                        Align2::CENTER_CENTER,
                        Icon::X,
                        font_icon.clone(),
                        col,
                    );
                }
                if close_resp.clicked() {
                    resp_out.closed.push(i);
                }
            }
        }
    }

    // Update drag state from input.
    if let (Some(state), Some(p)) = (drag.as_mut(), pointer) {
        state.current_x = p.x;
        if !state.active && (p.x - state.start_x).abs() > DRAG_THRESHOLD {
            state.active = true;
        }
    }

    // Render the dragged tab as floating (above others, semi-transparent).
    if let (Some(state), Some(p)) = (drag.as_ref(), pointer) {
        if state.active && state.from < n {
            let i = state.from;
            let item = &snapshot[i];
            let w = effective_widths[i];
            let rect = Rect::from_min_size(
                Pos2::new(p.x - w * 0.5, strip_rect.top()),
                Vec2::new(w, row_h),
            );
            let layer = egui::LayerId::new(egui::Order::Tooltip, outer_id.with("drag_layer"));
            let painter = ui.ctx().layer_painter(layer);
            // Semi-transparent overlay
            paint_one_tab_painter(
                &painter, theme, treatment, rect, item, true, 1.0, 1.0,
                &font_label, &font_icon, inner_gap, pad_x, 70,
            );
        }
    }

    // Drag end: commit reorder.
    if drag.is_some() && (primary_released || !primary_down) {
        let was = drag.unwrap();
        if was.active {
            if let Some(to) = drop_index {
                let from = was.from;
                if from != to {
                    if let Source::Items(items) = &mut source {
                        if from < items.len() && to < items.len() {
                            let item = items.remove(from);
                            items.insert(to, item);
                        }
                    }
                    // Adjust active index for caller.
                    if cur_active == from {
                        new_active = to;
                    } else if from < cur_active && to >= cur_active {
                        new_active = cur_active.saturating_sub(1);
                    } else if from > cur_active && to <= cur_active {
                        new_active = cur_active + 1;
                    }
                    resp_out.reordered = Some((from, to));
                }
            }
        }
        ui.ctx().data_mut(|d| d.remove::<DragState>(drag_id));
    }

    // Add (+) button.
    if addable {
        let plus_rect = Rect::from_min_size(
            Pos2::new(x, strip_rect.top()),
            Vec2::new(row_h, row_h),
        );
        let plus_resp = ui.interact(plus_rect, outer_id.with("add"), Sense::click());
        let hover_t = motion::ease_bool(ui.ctx(), outer_id.with("add_hov"),
            plus_resp.hovered(), motion::FAST);
        let bg = motion::lerp_color(
            Color32::TRANSPARENT,
            st::color_alpha(theme.surface(), 200),
            hover_t,
        );
        ui.painter().rect_filled(plus_rect, CornerRadius::same(4), bg);
        ui.painter().text(
            plus_rect.center(),
            Align2::CENTER_CENTER,
            Icon::PLUS,
            font_icon.clone(),
            theme.dim(),
        );
        if plus_resp.clicked() {
            resp_out.add_clicked = true;
        }
    }

    // ── Card treatment: post-loop hairline separators ──
    // Vertical hairline between every adjacent tab pair (active included),
    // plus a horizontal hairline below the strip — except where the active
    // tab sits, so the active tab's "open bottom" merges with the content
    // panel below.
    if matches!(treatment, TabTreatment::Card) {
        let sep_color = st::color_alpha(theme.border(), st::alpha_muted());
        let stroke = Stroke::new(st::stroke_thin(), sep_color);
        // Vertical separators between every adjacent tab pair.
        for i in 1..n {
            let r = displaced_rects[i];
            ui.painter().line_segment(
                [Pos2::new(r.left(), r.top() + 4.0),
                 Pos2::new(r.left(), r.bottom() - 4.0)],
                stroke,
            );
        }
        // Bottom hairline — full width minus the active tab's footprint
        // (the active tab's open bottom sits flush with the content panel).
        let bottom_y = strip_rect.bottom() - 0.5;
        let active_rect = displaced_rects.get(cur_active).copied();
        let segments: Vec<(f32, f32)> = match active_rect {
            Some(a) => vec![
                (strip_rect.left(), a.left()),
                (a.right(), strip_rect.right()),
            ],
            None => vec![(strip_rect.left(), strip_rect.right())],
        };
        for (x0, x1) in segments {
            if x1 > x0 + 0.5 {
                ui.painter().line_segment(
                    [Pos2::new(x0, bottom_y), Pos2::new(x1, bottom_y)],
                    stroke,
                );
            }
        }
    }

    if new_active != cur_active {
        *active = new_active;
        resp_out.changed = true;
        // Best-effort: ask egui to scroll the new active tab's rect into view.
        if new_active < displaced_rects.len() {
            ui.scroll_to_rect(displaced_rects[new_active], None);
        }
    }

    resp_out
}

fn tab_is_closable(item: &TabItem, default: bool) -> bool {
    item.closable.unwrap_or(default)
}

/// Width of all tab content laid out horizontally.
fn measure_tab_width(
    ui: &Ui,
    item: &TabItem,
    font_label: &FontId,
    font_icon: &FontId,
    closable: bool,
    inner_gap: f32,
    pad_x: f32,
) -> f32 {
    let mut w = pad_x * 2.0;
    let mut first = true;
    let mut add_segment = |seg_w: f32, w: &mut f32, first: &mut bool| {
        if !*first { *w += inner_gap; }
        *w += seg_w;
        *first = false;
    };
    if let Some(ic) = item.icon {
        let g = ui.fonts(|f| f.layout_no_wrap(ic.to_string(), font_icon.clone(), Color32::WHITE));
        add_segment(g.rect.width(), &mut w, &mut first);
    }
    let g = ui.fonts(|f| f.layout_no_wrap(item.label.clone(), font_label.clone(), Color32::WHITE));
    add_segment(g.rect.width().max(20.0), &mut w, &mut first);
    if let Some(n) = item.badge {
        let s = if n > 99 { "99+".to_string() } else { n.to_string() };
        let g = ui.fonts(|f| f.layout_no_wrap(s, FontId::monospace(10.0), Color32::WHITE));
        add_segment((g.rect.width() + 10.0).max(14.0), &mut w, &mut first);
    }
    if item.modified {
        add_segment(MOD_DOT_R * 2.0, &mut w, &mut first);
    }
    if closable {
        add_segment(CLOSE_VIS, &mut w, &mut first);
    }
    w
}

#[allow(clippy::too_many_arguments)]
fn paint_one_tab(
    ui: &Ui,
    theme: &dyn ComponentTheme,
    treatment: TabTreatment,
    rect: Rect,
    item: &TabItem,
    is_active: bool,
    hover_t: f32,
    active_t: f32,
    font_label: &FontId,
    font_icon: &FontId,
    inner_gap: f32,
    pad_x: f32,
) {
    paint_one_tab_painter(
        &ui.painter().clone(),
        theme,
        treatment,
        rect,
        item,
        is_active,
        hover_t,
        active_t,
        font_label,
        font_icon,
        inner_gap,
        pad_x,
        255,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_one_tab_painter(
    painter: &egui::Painter,
    theme: &dyn ComponentTheme,
    treatment: TabTreatment,
    rect: Rect,
    item: &TabItem,
    is_active: bool,
    hover_t: f32,
    active_t: f32,
    font_label: &FontId,
    font_icon: &FontId,
    inner_gap: f32,
    pad_x: f32,
    alpha_mul: u8,
) {
    let alpha = |c: Color32| -> Color32 {
        if alpha_mul == 255 { c } else {
            let a = (c.a() as f32 * (alpha_mul as f32 / 255.0)).round() as u8;
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), a)
        }
    };

    // Background per treatment.
    match treatment {
        TabTreatment::Line => {
            // Inactive: transparent. Hover: subtle dim tint.
            if hover_t > 0.01 && !is_active {
                let bg = motion::lerp_color(
                    Color32::TRANSPARENT,
                    st::color_alpha(theme.surface(), 80),
                    hover_t,
                );
                painter.rect_filled(rect, CornerRadius::ZERO, alpha(bg));
            }
        }
        TabTreatment::Segmented => {
            if is_active {
                let inset = rect.shrink2(Vec2::new(2.0, 2.0));
                let bg = motion::fade_in(theme.bg(), active_t);
                painter.rect_filled(inset, CornerRadius::same(4), alpha(bg));
            } else if hover_t > 0.01 {
                let bg = motion::lerp_color(
                    Color32::TRANSPARENT,
                    st::color_alpha(theme.bg(), 100),
                    hover_t,
                );
                painter.rect_filled(rect.shrink2(Vec2::new(2.0, 2.0)),
                    CornerRadius::same(4), alpha(bg));
            }
        }
        TabTreatment::Filled => {
            if is_active {
                let bg = motion::fade_in(theme.surface(), active_t);
                painter.rect_filled(rect, CornerRadius::same(4), alpha(bg));
            } else if hover_t > 0.01 {
                let bg = motion::lerp_color(
                    Color32::TRANSPARENT,
                    st::color_alpha(theme.surface(), 120),
                    hover_t,
                );
                painter.rect_filled(rect, CornerRadius::same(4), alpha(bg));
            }
        }
        TabTreatment::Card => {
            // Active: subtle surface fill + 2px top accent indicator + hairline
            // borders on top, left, and right. NO bottom border so the tab
            // visually merges with the content panel below. Inactive tabs are
            // flat; hover paints a faint surface tint. Inter-tab vertical
            // separators + the full-width hairline below the strip are painted
            // post-loop in `paint_tabs`.
            if is_active {
                let bg = motion::fade_in(st::color_alpha(theme.surface(), 220), active_t);
                painter.rect_filled(rect, CornerRadius::ZERO, alpha(bg));
                let accent_col = motion::fade_in(theme.accent(), active_t);
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.left(), rect.top()),
                        Vec2::new(rect.width(), 2.0),
                    ),
                    CornerRadius::ZERO,
                    alpha(accent_col),
                );
                let border = motion::fade_in(
                    st::color_alpha(theme.border(), st::alpha_strong()),
                    active_t,
                );
                let bs = Stroke::new(st::stroke_thin(), alpha(border));
                // Top
                painter.line_segment(
                    [Pos2::new(rect.left(), rect.top()),
                     Pos2::new(rect.right(), rect.top())],
                    bs,
                );
                // Left
                painter.line_segment(
                    [Pos2::new(rect.left(), rect.top()),
                     Pos2::new(rect.left(), rect.bottom())],
                    bs,
                );
                // Right
                painter.line_segment(
                    [Pos2::new(rect.right(), rect.top()),
                     Pos2::new(rect.right(), rect.bottom())],
                    bs,
                );
            } else if hover_t > 0.01 {
                let bg = motion::lerp_color(
                    Color32::TRANSPARENT,
                    st::color_alpha(theme.surface(), 100),
                    hover_t,
                );
                painter.rect_filled(rect, CornerRadius::ZERO, alpha(bg));
            }
        }
    }

    // Text color: dim → text on hover/active.
    let label_col = if item.disabled {
        st::color_alpha(theme.dim(), 120)
    } else if is_active {
        theme.text()
    } else {
        motion::lerp_color(theme.dim(), theme.text(), hover_t)
    };
    let label_col = alpha(label_col);

    // Layout content left-to-right.
    let mut cx = rect.left() + pad_x;
    let cy = rect.center().y;

    if let Some(ic) = item.icon {
        let g = painter.layout_no_wrap(ic.to_string(), font_icon.clone(), label_col);
        let w = g.rect.width();
        painter.galley(Pos2::new(cx, cy - g.rect.height() * 0.5), g, label_col);
        cx += w + inner_gap;
    }

    // Label (with optional ellipsis). Reserve trailing space only for the
    // bits actually present on this item — the previous `|| true` clause
    // forced every tab to reserve close-button space even on non-closable
    // tabs, eating ~15px and triggering false-positive ellipsis.
    let max_label_w = rect.right() - pad_x - cx
        - if item.badge.is_some() { 18.0 + inner_gap } else { 0.0 }
        - if item.modified { MOD_DOT_R * 2.0 + inner_gap } else { 0.0 }
        - if tab_is_closable(item, false) { CLOSE_VIS + inner_gap } else { 0.0 };
    let max_label_w = max_label_w.max(8.0);

    let text = ellipsize(painter, &item.label, font_label, max_label_w, label_col);
    let g = painter.layout_no_wrap(text, font_label.clone(), label_col);
    let lw = g.rect.width();
    let lh = g.rect.height();
    painter.galley(Pos2::new(cx, cy - lh * 0.5), g, label_col);
    cx += lw + inner_gap;

    // Active underline (Line treatment).
    if matches!(treatment, TabTreatment::Line) && is_active {
        let half = (rect.width() * 0.5) * active_t;
        let center_x = rect.center().x;
        let y = rect.bottom() - 1.0;
        let col = alpha(theme.accent());
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(center_x - half, y - 1.0),
                Pos2::new(center_x + half, y + 1.0),
            ),
            CornerRadius::ZERO,
            col,
        );
    }

    // Optional dim underline on hover (Line treatment, inactive only).
    if matches!(treatment, TabTreatment::Line) && !is_active && hover_t > 0.01 {
        let col = motion::lerp_color(
            Color32::TRANSPARENT,
            st::color_alpha(theme.dim(), 80),
            hover_t,
        );
        painter.line_segment(
            [Pos2::new(rect.left() + pad_x, rect.bottom() - 0.5),
             Pos2::new(rect.right() - pad_x, rect.bottom() - 0.5)],
            Stroke::new(1.0, alpha(col)),
        );
    }

    // Badge.
    if let Some(n) = item.badge {
        let s = if n > 99 { "99+".to_string() } else { n.to_string() };
        let bg = painter.layout_no_wrap(s.clone(), FontId::monospace(10.0), Color32::WHITE);
        let bw = (bg.rect.width() + 10.0).max(14.0);
        let bh = 14.0;
        let br = Rect::from_min_size(Pos2::new(cx, cy - bh * 0.5), Vec2::new(bw, bh));
        painter.rect_filled(br, CornerRadius::same(7), alpha(theme.bear()));
        painter.text(br.center(), Align2::CENTER_CENTER, &s,
            FontId::monospace(10.0), Color32::WHITE);
        cx += bw + inner_gap;
    }

    // Modified dot.
    if item.modified {
        painter.circle_filled(
            Pos2::new(cx + MOD_DOT_R, cy),
            MOD_DOT_R,
            alpha(theme.accent()),
        );
        // close-button drawn separately by caller via interact()
    }

    // Outline for active in Filled treatment? Match shadcn: subtle border.
    if matches!(treatment, TabTreatment::Filled) && is_active {
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, alpha(st::color_alpha(theme.border(), 180))),
            StrokeKind::Inside,
        );
    }
}

fn ellipsize(
    painter: &egui::Painter,
    text: &str,
    font: &FontId,
    max_w: f32,
    color: Color32,
) -> String {
    let g = painter.layout_no_wrap(text.to_string(), font.clone(), color);
    if g.rect.width() <= max_w {
        return text.to_string();
    }
    let ell = "…";
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>() + ell;
        let g = painter.layout_no_wrap(candidate.clone(), font.clone(), color);
        if g.rect.width() <= max_w { return candidate; }
    }
    ell.to_string()
}
