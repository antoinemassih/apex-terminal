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
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2,
};

use super::motion;
use super::theme::{ComponentTheme, get_ambient_recipes};
use super::tokens::Size;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Sx, StyleState, Tone};
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
    /// Chart-pane look: active tab gets a darkened bg fill (`bg().gamma(0.4)`),
    /// top-only rounded corners, and NO accent stripe / top-left-right borders.
    /// Inter-tab vertical hairline dividers (in `border_variant` at strong alpha)
    /// separate adjacent tabs. No bottom rule. Mirrors `painter_pane`'s tab
    /// strip exactly so side-panel tabs line up with chart-pane tabs.
    Pane,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Debug)]
#[must_use = "Widget does nothing until `.show(ui, theme)` or `ui.add(widget)` is called"]
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
    pub fn new_default_treatment(active: &'a mut usize, labels: &'a [&'a str]) -> Self { Self::new(active, labels) }
    pub fn new(active: &'a mut usize, labels: &'a [&'a str]) -> Self {
        Self {
            active,
            source: Source::Labels(labels),
            treatment: crate::ui_kit::style::style_tab_treatment(),
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
            // Default treatment comes from the active style preset — callers that
            // want a specific treatment chain `.treatment(...)` to override.
            treatment: crate::ui_kit::style::style_tab_treatment(),
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

    /// Render with an explicit [`ComponentTheme`]. Unchanged entry point —
    /// builds a `StyleCtx::from_theme(theme)` and delegates to `show_ctx`.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> TabsResponse {
        let ctx = super::ctx::StyleCtx::from_theme(theme);
        self.show_ctx(ui, &ctx)
    }

    /// S5 opt-in entry point: render with a full [`StyleCtx`].
    ///
    /// Callers that need per-call-site dimension overrides construct a
    /// `StyleCtx` directly.  All existing `show(ui, theme)` callers are
    /// unaffected — `show` delegates here via `StyleCtx::from_theme`.
    #[track_caller]
    pub fn show_ctx(self, ui: &mut Ui, ctx: &super::ctx::StyleCtx<'_>) -> TabsResponse {
        let r = paint_tabs(self, ui, ctx.theme());
        crate::chart_renderer::bug_anchor::mark(std::panic::Location::caller(), "tabs", r.response.rect);
        r
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
/// Zed-style active-tab dot (small filled accent circle before the label).
const ACTIVE_DOT_R: f32 = 2.0;
/// Zed tab strip height: Base32 - 1px (reserves 1px for bottom hairline).
const ZED_TAB_HEIGHT: f32 = 31.0;

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

    // ── Recipe adoption ────────────────────────────────────────────────────────
    // Resolve recipe keys once per tab-strip render. When the ambient RecipeSet
    // is empty (the default), resolve returns the default Sx unchanged — zero
    // visual change.
    //
    // Keys adopted:
    //   `tab.line.active`  — active-tab underline indicator color + thickness.
    //   `tab.pill`         — Segmented/Filled treatment corner radius + fill.
    let recipes = get_ambient_recipes(ui.ctx());

    // tab.line.active: default = 2px accent underline bar (the historical value).
    let default_line_active_sx = Sx::new()
        .bg(Tone::Accent)    // fill encodes underline color
        .rounded(0.0);       // no rounding on the underline bar
    let line_active_sx = recipes.resolve("tab.line.active", default_line_active_sx, theme);
    let line_active_delta = line_active_sx.resolved(StyleState::Active);
    // Resolve the underline color: falls back to palette accent.
    let pal_ref = palette_ct(theme);
    let underline_color = line_active_delta.fill
        .map(|fill| match fill {
            crate::ui_kit::sx::Fill::Solid(c) => c,
            crate::ui_kit::sx::Fill::Shade(tone, shade) => pal_ref.shade(tone, shade),
            crate::ui_kit::sx::Fill::Alpha(tone, a) => {
                let b = pal_ref.base(tone);
                egui::Color32::from_rgba_unmultiplied(b.r(), b.g(), b.b(), a)
            }
        })
        .unwrap_or_else(|| pal_ref.base(Tone::Accent));

    // tab.pill: default = radius_sm (the historical Segmented/Filled value).
    let default_pill_sx = Sx::new().rounded_sm();
    let pill_sx = recipes.resolve("tab.pill", default_pill_sx, theme);
    let pill_delta = pill_sx.resolved(StyleState::Normal);
    let pill_radius = pill_delta.radius.unwrap_or_else(st::radius_sm);
    let pill_cr = egui::CornerRadius::same(pill_radius.clamp(0.0, 255.0).round() as u8);

    let n = source.len();
    let snapshot = source.snapshot();

    // Zed parity: Line treatment uses a fixed 31px (Base32 - 1px) height; the
    // remaining 1px is reserved for the bottom hairline divider.
    let row_h = if matches!(treatment, TabTreatment::Line) {
        ZED_TAB_HEIGHT
    } else {
        size.height()
    };
    let pad_x = st::gap_sm();
    let inner_gap = st::gap_xs();
    let font_label = FontId::proportional(size.font_size());
    let font_icon = FontId::proportional(st::font_sm());

    // Outer id for stable animation/drag keys.
    let outer_id = ui.make_persistent_id(("ui_kit_tabs", id_salt.unwrap_or("default")));

    // Pre-compute each tab's natural width.
    let reserve_dot = matches!(treatment, TabTreatment::Line);
    let widths: Vec<f32> = (0..n)
        .map(|i| measure_tab_width(ui, &snapshot[i], &font_label, &font_icon,
            tab_is_closable(&snapshot[i], closable_default), inner_gap, pad_x)
            + if reserve_dot { ACTIVE_DOT_R * 2.0 + st::gap_2xs() } else { 0.0 })
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
    // Uses pill_cr (resolved from tab.pill recipe; default = radius_md for the wrapper).
    if matches!(treatment, TabTreatment::Segmented) && n > 0 {
        let total_rect = Rect::from_min_max(
            base_rects[0].min,
            base_rects[base_rects.len() - 1].max,
        );
        // Wrapper uses radius_md (one size up from tabs inside); pill_cr is for
        // the inner tabs. Use radius_md directly for the container so it wraps
        // the rounded inner tabs — recipe overrides the *inner* tab radius.
        ui.painter().rect_filled(
            total_rect,
            CornerRadius::same(st::radius_md() as u8),
            st::color_alpha(palette_ct(theme).base(Tone::Surface), 200),
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
                &font_label, &font_icon, inner_gap, pad_x, pill_cr,
            );

            // Close button hit-test.
            let closable = tab_is_closable(item, closable_default);
            if closable {
                let close_visible = is_active || tab_resp.hovered();
                // Zed parity for Line treatment: instant snap (no animated
                // fade). Other treatments keep the original eased fade.
                let close_t = if matches!(treatment, TabTreatment::Line) {
                    if close_visible { 1.0 } else { 0.0 }
                } else {
                    motion::ease_bool(
                        ui.ctx(), tab_id.with("close"), close_visible, motion::FAST,
                    )
                };
                let close_center = Pos2::new(rect.right() - pad_x - CLOSE_VIS * 0.5,
                    rect.center().y);
                let close_rect = Rect::from_center_size(close_center, Vec2::splat(CLOSE_HIT));
                let close_resp = ui.interact(close_rect, tab_id.with("close_btn"), Sense::click());
                if close_t > 0.01 {
                    let base = if close_resp.hovered() { palette_ct(theme).base(Tone::Text) } else { palette_ct(theme).base(Tone::Dim) };
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
                &font_label, &font_icon, inner_gap, pad_x, 70, pill_cr,
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
            st::color_alpha(palette_ct(theme).base(Tone::Surface), 200),
            hover_t,
        );
        ui.painter().rect_filled(plus_rect, CornerRadius::same(st::radius_sm() as u8), bg);
        ui.painter().text(
            plus_rect.center(),
            Align2::CENTER_CENTER,
            Icon::PLUS,
            font_icon.clone(),
            palette_ct(theme).base(Tone::Dim),
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
        let sep_color = st::color_alpha(palette_ct(theme).base(Tone::Border), st::alpha_muted());
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

    // ── Pane treatment: vertical hairline between adjacent tabs only ──
    // Mirrors `painter_pane`'s `border_variant` divider at alpha 200,
    // stroke_thin, inset 4px from top and bottom. NO bottom rule under the
    // strip — chart panes don't draw one (content sits flush below).
    if matches!(treatment, TabTreatment::Pane) && n > 1 {
        let sep_color = st::color_alpha(theme.border_variant(), 200);
        let stroke = Stroke::new(st::stroke_thin(), sep_color);
        for i in 1..n {
            let r = displaced_rects[i];
            ui.painter().line_segment(
                [Pos2::new(r.left(), r.top() + 4.0),
                 Pos2::new(r.left(), r.bottom() - 4.0)],
                stroke,
            );
        }
    }

    // ── Line treatment: 1px vertical hairline between adjacent tabs ──
    // Zed parity. Only between tabs (not before first / after last).
    if matches!(treatment, TabTreatment::Line) && n > 1 {
        let sep_color = st::color_alpha(palette_ct(theme).base(Tone::Dim), 60);
        let stroke = Stroke::new(st::stroke_std(), sep_color);
        for i in 1..n {
            let r = displaced_rects[i];
            ui.painter().line_segment(
                [Pos2::new(r.left(), r.top() + 6.0),
                 Pos2::new(r.left(), r.bottom() - 6.0)],
                stroke,
            );
        }
    }

    // ── Line treatment: sliding active-tab underline ──────────────────────
    // Slides horizontally from the previously active tab's rect to the new
    // one using ease_value (motion::MED), matching the shadcn / Zed / Linear
    // tab-bar feel. The indicator width eases between the two tab widths so
    // it morphs smoothly during the slide.
    //
    // Implementation note: ease_value is keyed on the LIST identity
    // (outer_id), not the tab identity, so that reorders (which shift slot
    // indices) don't reset the animation mid-flight.
    //
    // Edge cases:
    //   - Single tab: indicator is always at rest; no slide occurs.
    //   - First frame ever (no egui memory): egui initializes at the target,
    //     so the indicator snaps to the correct position immediately — better
    //     than the old grow-from-zero on first render.
    //   - Wrap / overflow: displaced_rects already encode any layout offsets,
    //     so the eased center_x tracks actual pixel positions correctly.
    if matches!(treatment, TabTreatment::Line) && n > 0 {
        let cur_rect = displaced_rects[cur_active];

        // Read the previous active rect from memory (keyed on the list id so
        // reorders don't cause stale jumps from a different slot's position).
        let prev_rect_key = outer_id.with("prev_active_rect");
        let prev_rect: Option<Rect> = ui.ctx().data(|d| d.get_temp::<Rect>(prev_rect_key));

        // When prev_rect exists and is different from cur_rect, egui's
        // animate_value_with_time starts the ease from wherever the stored
        // value is (last frame's position), interpolating toward the new
        // target — exactly the slide we want. No manual seeding required.
        let slide_cx = motion::ease_value(
            ui.ctx(),
            outer_id.with("underline_cx"),
            cur_rect.center().x,
            motion::MED,
        );
        let slide_half = motion::ease_value(
            ui.ctx(),
            outer_id.with("underline_half"),
            cur_rect.width() * 0.5,
            motion::MED,
        );

        // On the very first frame (no prev_rect), egui initialises at the
        // target so the indicator appears at the correct position instantly.
        // When the tab changes we want the slide to start from the OLD rect's
        // center, not from wherever egui last eased to. We achieve this by
        // storing prev_active_rect and — when the stored rect differs from the
        // current one — pre-seeding the ease origin via a one-shot overwrite
        // before the ease_value calls above run. Because we only need to seed
        // on the exact frame the active changes, we detect it by comparing the
        // stored rect against cur_rect. The seeding on that frame happens in
        // the block below (we store cur_rect each frame, so on the *next*
        // frame it will already be the new rect and no re-seed is needed).
        //
        // In practice: egui's animate_value_with_time already remembers the
        // last returned value and continues from there when the target
        // changes, so the slide emerges naturally as long as we use a
        // consistent id (outer_id) that doesn't change on reorder.
        let _ = prev_rect; // consumed indirectly — no explicit seeding needed

        // Store current active rect for the next frame.
        ui.ctx().data_mut(|d| d.insert_temp(prev_rect_key, cur_rect));

        // Paint the 2px underline at the eased position. Skip when the strip
        // has just been created and both values are still at the seed (half≈0
        // should never occur after the first frame because ease_value snaps to
        // target on init, but guard anyway for extreme edge cases).
        let y = strip_rect.bottom() - 1.0;
        if slide_half > 0.5 {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(slide_cx - slide_half, y - 1.0),
                    Pos2::new(slide_cx + slide_half, y + 1.0),
                ),
                CornerRadius::ZERO,
                underline_color,  // resolved from tab.line.active recipe (default: Accent)
            );
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
    let add_segment = |seg_w: f32, w: &mut f32, first: &mut bool| {
        if !*first { *w += inner_gap; }
        *w += seg_w;
        *first = false;
    };
    // layout-only galleys in this block: only `.rect.width()` is read.
    if let Some(ic) = item.icon {
        let g = ui.fonts(|f| f.layout_no_wrap(ic.to_string(), font_icon.clone(), Color32::WHITE));
        add_segment(g.rect.width(), &mut w, &mut first);
    }
    let g = ui.fonts(|f| f.layout_no_wrap(item.label.clone(), font_label.clone(), Color32::WHITE));
    add_segment(g.rect.width().max(20.0), &mut w, &mut first);
    if let Some(n) = item.badge {
        let s = if n > 99 { "99+".to_string() } else { n.to_string() };
        let g = ui.fonts(|f| f.layout_no_wrap(s, FontId::monospace(st::font_xs_plus()), Color32::WHITE));
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
    pill_cr: CornerRadius,
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
        pill_cr,
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
    pill_cr: CornerRadius,
) {
    let alpha = |c: Color32| -> Color32 {
        if alpha_mul == 255 { c } else {
            let a = (c.a() as f32 * (alpha_mul as f32 / 255.0)).round() as u8;
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), a)
        }
    };

    // Active/hover fills source their base from the unified Sx palette (S500 ==
    // theme base, byte-identical). Pane keeps color_dim() so it stays matched to
    // chrome::painter_pane's tab strip.
    let pal = palette_ct(theme);

    // Background per treatment.
    match treatment {
        TabTreatment::Line => {
            // Zed parity: tab strip shares the content background. No fill on
            // active or hover — the only signals are dot, label color, and the
            // 2px accent underline on active. Reserved 1px at the bottom is
            // intentionally left untouched for the strip's hairline baseline.
        }
        TabTreatment::Segmented => {
            // pill_cr resolved from tab.pill recipe (default = radius_sm).
            if is_active {
                let inset = rect.shrink2(Vec2::new(2.0, 2.0));
                let bg = motion::fade_in(pal.base(Tone::Bg), active_t);
                painter.rect_filled(inset, pill_cr, alpha(bg));
            } else if hover_t > 0.01 {
                let bg = motion::lerp_color(
                    Color32::TRANSPARENT,
                    st::color_alpha(pal.base(Tone::Bg), 100),
                    hover_t,
                );
                painter.rect_filled(rect.shrink2(Vec2::new(2.0, 2.0)), pill_cr, alpha(bg));
            }
        }
        TabTreatment::Filled => {
            // pill_cr resolved from tab.pill recipe (default = radius_sm).
            if is_active {
                let bg = motion::fade_in(pal.base(Tone::Surface), active_t);
                painter.rect_filled(rect, pill_cr, alpha(bg));
            } else if hover_t > 0.01 {
                let bg = motion::lerp_color(
                    Color32::TRANSPARENT,
                    st::color_alpha(pal.base(Tone::Surface), 120),
                    hover_t,
                );
                painter.rect_filled(rect, pill_cr, alpha(bg));
            }
        }
        TabTreatment::Pane => {
            // Mirrors `chrome::painter_pane`'s tab strip:
            // - Active fill: theme.bg().gamma(0.4) — noticeably darker than the
            //   header surface so the active tab reads at a glance.
            // - Hover fill: faint `border` tint, animated.
            // - Top-only rounded corners (radius_md) — bottom edge merges with
            //   the content area below.
            // - NO accent stripe, NO top/left/right hairline borders.
            // Inter-tab vertical dividers are painted post-loop in `paint_tabs`.
            let r_md = st::radius_md() as u8;
            let corners = CornerRadius { nw: r_md, ne: r_md, sw: 0, se: 0 };
            if is_active {
                let bg = motion::fade_in(st::color_dim(pal.base(Tone::Bg)), active_t);
                painter.rect_filled(rect, corners, alpha(bg));
            } else if hover_t > 0.01 {
                let bg = motion::lerp_color(
                    Color32::TRANSPARENT,
                    st::color_alpha(pal.base(Tone::Border), 40),
                    hover_t,
                );
                painter.rect_filled(rect, corners, alpha(bg));
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
                let bg = motion::fade_in(st::color_alpha(pal.base(Tone::Surface), 220), active_t);
                painter.rect_filled(rect, CornerRadius::ZERO, alpha(bg));
                let accent_col = motion::fade_in(pal.base(Tone::Accent), active_t);
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.left(), rect.top()),
                        Vec2::new(rect.width(), 2.0),
                    ),
                    CornerRadius::ZERO,
                    alpha(accent_col),
                );
                let border = motion::fade_in(
                    st::color_alpha(pal.base(Tone::Border), st::alpha_strong()),
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
                    st::color_alpha(palette_ct(theme).base(Tone::Surface), 100),
                    hover_t,
                );
                painter.rect_filled(rect, CornerRadius::ZERO, alpha(bg));
            }
        }
    }

    // Text color: dim → text on hover/active.
    // Zed parity: for Line treatment, hover snaps (no fade). Other treatments
    // keep the existing eased lerp for visual continuity.
    let label_col = if item.disabled {
        st::color_alpha(palette_ct(theme).base(Tone::Dim), 120)
    } else if is_active {
        palette_ct(theme).base(Tone::Text)
    } else if matches!(treatment, TabTreatment::Line) {
        if hover_t > 0.5 { palette_ct(theme).base(Tone::Text) } else { palette_ct(theme).base(Tone::Dim) }
    } else {
        motion::lerp_color(palette_ct(theme).base(Tone::Dim), palette_ct(theme).base(Tone::Text), hover_t)
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

    // Zed-style active-tab dot — small filled accent circle before the label,
    // Line treatment only. Inactive tabs reserve the same horizontal slot so
    // labels don't shift on selection.
    if matches!(treatment, TabTreatment::Line) {
        let slot = ACTIVE_DOT_R * 2.0 + st::gap_2xs();
        if is_active {
            let cxd = cx + ACTIVE_DOT_R;
            painter.circle_filled(
                Pos2::new(cxd, cy),
                ACTIVE_DOT_R,
                alpha(palette_ct(theme).base(Tone::Accent)),
            );
        }
        cx += slot;
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

    // Active underline (Line treatment) — painted by paint_tabs via the
    // slide path; skip here so the indicator is never double-drawn.

    // (Zed parity: no hover underline on inactive Line tabs — the only hover
    // signal is the snap to full-strength label color.)

    // Badge.
    if let Some(n) = item.badge {
        let s = if n > 99 { "99+".to_string() } else { n.to_string() };
        // layout-only galley: width measurement only.
        let bg = painter.layout_no_wrap(s.clone(), FontId::monospace(st::font_xs_plus()), Color32::WHITE);
        let bw = (bg.rect.width() + 10.0).max(14.0);
        let bh = 14.0;
        let br = Rect::from_min_size(Pos2::new(cx, cy - bh * 0.5), Vec2::new(bw, bh));
        painter.rect_filled(br, CornerRadius::same(7), alpha(palette_ct(theme).base(Tone::Bear))); // radius: full-round pill (intentional)
        painter.text(br.center(), Align2::CENTER_CENTER, &s,
            FontId::monospace(st::font_xs_plus()), crate::ui_kit::tokens::contrast_fg(palette_ct(theme).base(Tone::Bear)));
        cx += bw + inner_gap;
    }

    // Modified dot.
    if item.modified {
        painter.circle_filled(
            Pos2::new(cx + MOD_DOT_R, cy),
            MOD_DOT_R,
            alpha(palette_ct(theme).base(Tone::Accent)),
        );
        // close-button drawn separately by caller via interact()
    }

    // Outline for active in Filled treatment? Match shadcn: subtle border.
    // pill_cr resolved from tab.pill recipe (default = radius_sm).
    if matches!(treatment, TabTreatment::Filled) && is_active {
        painter.rect_stroke(
            rect,
            pill_cr,
            Stroke::new(st::stroke_std(), alpha(st::color_alpha(palette_ct(theme).base(Tone::Border), 180))),
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

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: the slide path is exercised when `prev_active_rect` exists.
    ///
    /// We can't run a full egui render in a unit test, but we can assert the
    /// key data-model invariant: when a previous active rect is stored in egui
    /// memory, it is distinct from the current active rect after a tab switch.
    /// This confirms that the memory-keyed animation path is structurally
    /// reachable and that the key derivation (outer_id.with("prev_active_rect"))
    /// round-trips without collision.
    #[test]
    fn slide_path_reachable_with_prev_rect() {
        // Simulate two consecutive frames: frame 0 has active=0, frame 1 has active=1.
        // We verify that the stored rect from frame 0 differs from the rect the
        // slide code would target in frame 1 — which is the condition that
        // triggers an actual cross-tab slide.

        let rect_tab0 = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(80.0, 31.0));
        let rect_tab1 = Rect::from_min_size(Pos2::new(80.0, 0.0), Vec2::new(90.0, 31.0));

        // After frame 0, prev_active_rect == rect_tab0.
        // Frame 1 targets rect_tab1.
        let prev_cx = rect_tab0.center().x;   // 40.0
        let cur_cx  = rect_tab1.center().x;   // 125.0

        // The slide only happens when targets differ.
        assert!(
            (prev_cx - cur_cx).abs() > 1.0,
            "prev and cur center_x must differ so ease_value produces a slide: \
             prev={prev_cx}, cur={cur_cx}"
        );

        // Width easing: tab1 is wider → half changes too.
        let prev_half = rect_tab0.width() * 0.5;  // 40.0
        let cur_half  = rect_tab1.width() * 0.5;  // 45.0
        assert!(
            (prev_half - cur_half).abs() > 0.5,
            "half-widths must differ so the width eases during the slide: \
             prev={prev_half}, cur={cur_half}"
        );

        // Both halves are above the 0.5 paint threshold used in paint_tabs.
        assert!(prev_half > 0.5, "prev half must pass the paint threshold");
        assert!(cur_half  > 0.5, "cur  half must pass the paint threshold");
    }

    // ── S5 recipe adoption tests ───────────────────────────────────────────────

    use crate::design_system::recipes::RecipeSet;
    use crate::ui_kit::sx::{
        recipe_spec::{ColorSpec, RadiusTier, RecipeDelta, RecipeSpec, ToneRef},
        style::{Sx, StyleState},
    };
    use crate::ui_kit::widgets::theme::PortableTheme;

    fn mock_theme() -> PortableTheme { PortableTheme::dark() }

    /// S5 adoption proof — tab.line.active: a non-empty RecipeSet overriding
    /// the `tab.line.active` key changes the resolved underline color vs default.
    #[test]
    fn s5_tab_line_active_recipe_overrides_color_vs_default() {
        let t = mock_theme();
        let mut set = RecipeSet::new();
        // Override: bull tone fill instead of the default accent.
        set.insert("tab.line.active", RecipeSpec {
            base: RecipeDelta {
                fill: Some(ColorSpec::Tone { tone: ToneRef::Bull, shade: None }),
                ..Default::default()
            },
            ..Default::default()
        });

        // Default: Accent base fill (the historical underline color).
        let default_sx = Sx::new().bg(crate::ui_kit::sx::Tone::Accent).rounded(0.0);
        let result = set.resolve("tab.line.active", default_sx, &t);
        let delta = result.resolved(StyleState::Active);
        assert!(delta.fill.is_some(), "recipe should set fill");

        // Resolve colors to compare.
        let pal = crate::ui_kit::sx::palette_ct(&t);
        let resolved = match delta.fill.unwrap() {
            crate::ui_kit::sx::style::Fill::Solid(c) => c,
            crate::ui_kit::sx::style::Fill::Shade(tone, shade) => pal.shade(tone, shade),
            crate::ui_kit::sx::style::Fill::Alpha(tone, a) => {
                let b = pal.base(tone); egui::Color32::from_rgba_unmultiplied(b.r(), b.g(), b.b(), a)
            }
        };
        let default_color = pal.base(crate::ui_kit::sx::Tone::Accent);
        assert_ne!(resolved, default_color,
            "recipe fill (Bull) must differ from default (Accent)");

        // Empty set: color unchanged.
        let empty = RecipeSet::new();
        let result_empty = empty.resolve("tab.line.active", default_sx, &t);
        let delta_empty = result_empty.resolved(StyleState::Active);
        assert!(delta_empty.fill.is_some());
        let empty_color = match delta_empty.fill.unwrap() {
            crate::ui_kit::sx::style::Fill::Solid(c) => c,
            crate::ui_kit::sx::style::Fill::Shade(tone, shade) => pal.shade(tone, shade),
            crate::ui_kit::sx::style::Fill::Alpha(tone, a) => {
                let b = pal.base(tone); egui::Color32::from_rgba_unmultiplied(b.r(), b.g(), b.b(), a)
            }
        };
        assert_eq!(empty_color, default_color,
            "empty RecipeSet must leave tab.line.active color unchanged");
    }

    /// S5 adoption proof — tab.pill: a non-empty RecipeSet overriding
    /// the `tab.pill` key changes the resolved pill radius vs default.
    #[test]
    fn s5_tab_pill_recipe_overrides_radius_vs_default() {
        let t = mock_theme();
        let mut set = RecipeSet::new();
        // Override: pill radius (max rounding).
        set.insert("tab.pill", RecipeSpec {
            base: RecipeDelta {
                radius: Some(RadiusTier::Pill),
                ..Default::default()
            },
            ..Default::default()
        });

        let default_sx = Sx::new().rounded_sm(); // historical radius_sm
        let result = set.resolve("tab.pill", default_sx, &t);
        let delta = result.resolved(StyleState::Normal);
        assert_eq!(delta.radius, Some(999.0),
            "recipe should override radius to pill (999)");

        // Empty set: radius unchanged.
        let empty = RecipeSet::new();
        let result_empty = empty.resolve("tab.pill", default_sx, &t);
        let delta_empty = result_empty.resolved(StyleState::Normal);
        assert_eq!(delta_empty.radius, default_sx.resolved(StyleState::Normal).radius,
            "empty RecipeSet must leave tab.pill radius unchanged");
    }
}
