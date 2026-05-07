//! Tree — hierarchical list view.
//!
//! Caller manages the data model (anything iterable that knows its
//! depth and children). Widget handles: indent guides, expand/collapse
//! caret, optional checkbox, hover/select highlighting.
//!
//! API:
//!   let mut state = TreeState::default();
//!   Tree::new(&mut state, &items)
//!     .item_render(|ui, theme, item, indent_x| {
//!         Label::new(&item.name).show(ui, theme);
//!     })
//!     .show(ui, theme);

use std::collections::HashSet;

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use super::motion;
use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

/// Trait implemented by caller's tree node type. The list passed to Tree
/// is a flat pre-order traversal — each node carries its own depth.
pub trait TreeNode {
    /// Stable id used for state (expanded/selected/checked).
    fn id(&self) -> u64;
    /// Indent level (0 = root).
    fn depth(&self) -> usize;
    /// Whether the node has children (caret is drawn only if true).
    fn has_children(&self) -> bool;
    /// Default label rendered when no `item_render` callback is provided.
    fn label(&self) -> &str;
}

#[derive(Clone, Default)]
pub struct TreeState {
    pub expanded: HashSet<u64>,
    pub selected: Option<u64>,
    pub checked: HashSet<u64>,
    pub hovered: Option<u64>,
}

impl TreeState {
    pub fn is_expanded(&self, id: u64) -> bool { self.expanded.contains(&id) }

    pub fn toggle_expand(&mut self, id: u64) {
        if !self.expanded.insert(id) { self.expanded.remove(&id); }
    }

    pub fn expand(&mut self, id: u64) { self.expanded.insert(id); }
    pub fn collapse(&mut self, id: u64) { self.expanded.remove(&id); }

    pub fn expand_all<T: TreeNode>(&mut self, items: &[T]) {
        for it in items {
            if it.has_children() { self.expanded.insert(it.id()); }
        }
    }

    pub fn collapse_all(&mut self) { self.expanded.clear(); }

    pub fn is_checked(&self, id: u64) -> bool { self.checked.contains(&id) }
    pub fn toggle_checked(&mut self, id: u64) -> bool {
        if !self.checked.insert(id) { self.checked.remove(&id); false } else { true }
    }
}

type ItemRenderFn<'a, T> = dyn Fn(&mut Ui, &dyn ComponentTheme, &T, f32) + 'a;
type IconForFn<'a, T> = dyn Fn(&T) -> Option<&'static str> + 'a;

#[must_use = "Tree does nothing until `.show(ui, theme)` is called"]
pub struct Tree<'a, T: TreeNode> {
    state: &'a mut TreeState,
    items: &'a [T],
    row_height: f32,
    indent_size: f32,
    show_indent_guides: bool,
    checkable: bool,
    item_render: Option<Box<ItemRenderFn<'a, T>>>,
    icon_for: Option<Box<IconForFn<'a, T>>>,
}

pub struct TreeResponse {
    pub response: Response,
    pub clicked: Option<u64>,
    pub double_clicked: Option<u64>,
    pub expanded: Option<u64>,
    pub collapsed: Option<u64>,
    pub checked: Option<u64>,
    pub unchecked: Option<u64>,
}

impl<'a, T: TreeNode> Tree<'a, T> {
    pub fn new(state: &'a mut TreeState, items: &'a [T]) -> Self {
        Self {
            state,
            items,
            row_height: 22.0,
            indent_size: 16.0,
            show_indent_guides: true,
            checkable: false,
            item_render: None,
            icon_for: None,
        }
    }

    pub fn row_height(mut self, h: f32) -> Self { self.row_height = h; self }
    pub fn indent_size(mut self, px: f32) -> Self { self.indent_size = px; self }
    pub fn show_indent_guides(mut self, v: bool) -> Self { self.show_indent_guides = v; self }
    pub fn checkable(mut self, v: bool) -> Self { self.checkable = v; self }

    pub fn item_render(mut self, f: impl Fn(&mut Ui, &dyn ComponentTheme, &T, f32) + 'a) -> Self {
        self.item_render = Some(Box::new(f));
        self
    }

    pub fn icon_for(mut self, f: impl Fn(&T) -> Option<&'static str> + 'a) -> Self {
        self.icon_for = Some(Box::new(f));
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> TreeResponse {
        // Build the visibility-filtered index list. A row is hidden if any
        // ancestor (lower depth) is collapsed. We track collapse_depth: when
        // a collapsed node at depth D is encountered, every following item
        // with depth > D is skipped until we return to depth <= D.
        let mut visible: Vec<usize> = Vec::with_capacity(self.items.len());
        let mut collapse_depth: Option<usize> = None;
        for (i, it) in self.items.iter().enumerate() {
            if let Some(cd) = collapse_depth {
                if it.depth() > cd { continue; }
                collapse_depth = None;
            }
            visible.push(i);
            if it.has_children() && !self.state.is_expanded(it.id()) {
                collapse_depth = Some(it.depth());
            }
        }

        let mut out = TreeResponse {
            response: ui.allocate_response(Vec2::ZERO, Sense::hover()),
            clicked: None,
            double_clicked: None,
            expanded: None,
            collapsed: None,
            checked: None,
            unchecked: None,
        };

        let virtualize = visible.len() > 200;

        let Tree {
            state, items, row_height, indent_size, show_indent_guides,
            checkable, item_render, icon_for, ..
        } = self;

        let cfg = Cfg {
            row_h: row_height,
            indent_size,
            show_indent_guides,
            checkable,
            item_render: item_render.as_deref(),
            icon_for: icon_for.as_deref(),
        };

        if virtualize {
            let total = visible.len();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_rows(ui, row_height, total, |ui, row_range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for vi in row_range {
                        let idx = visible[vi];
                        render_row(ui, theme, &items[idx], state, &cfg, &mut out);
                    }
                });
        } else {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for vi in &visible {
                    render_row(ui, theme, &items[*vi], state, &cfg, &mut out);
                }
            });
        }

        out
    }
}

struct Cfg<'a, T: TreeNode> {
    row_h: f32,
    indent_size: f32,
    show_indent_guides: bool,
    checkable: bool,
    item_render: Option<&'a ItemRenderFn<'a, T>>,
    icon_for: Option<&'a IconForFn<'a, T>>,
}

fn render_row<T: TreeNode>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    item: &T,
    state: &mut TreeState,
    cfg: &Cfg<'_, T>,
    out: &mut TreeResponse,
) {
    let id = item.id();
    let depth = item.depth();
    let row_h = cfg.row_h;
    let avail_w = ui.available_width().max(0.0);

    let (rect, response) = ui.allocate_exact_size(Vec2::new(avail_w, row_h), Sense::click());
    if !ui.is_rect_visible(rect) { return; }

    let painter = ui.painter_at(rect);
    let row_id = ui.id().with(("tree_row", id));

    // Hover / select backgrounds.
    let hovered = response.hovered();
    let selected = state.selected == Some(id);
    let hover_t = motion::ease_bool(ui.ctx(), row_id.with("hov"), hovered, motion::FAST);
    let cr = CornerRadius::same(3);

    if selected {
        painter.rect_filled(rect, cr, st::color_alpha(theme.accent(), st::ALPHA_GHOST));
    } else if hover_t > 0.001 {
        let bg = st::color_alpha(theme.text(), 18);
        painter.rect_filled(rect, cr, motion::lerp_color(Color32::TRANSPARENT, bg, hover_t));
    }

    // Indent guides (vertical hairlines).
    if cfg.show_indent_guides && depth > 0 {
        let guide_col = st::color_alpha(theme.border(), 100);
        for d in 0..depth {
            let x = rect.left() + (d as f32) * cfg.indent_size + cfg.indent_size * 0.5;
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, guide_col),
            );
        }
    }

    // Layout cursor.
    let mut x = rect.left() + (depth as f32) * cfg.indent_size;
    let cy = rect.center().y;

    // Caret.
    let caret_w = 14.0;
    let caret_rect = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(caret_w, row_h));
    if item.has_children() {
        let caret_resp = ui.interact(caret_rect, row_id.with("caret"), Sense::click());
        let expanded = state.is_expanded(id);
        let glyph = if expanded { Icon::CARET_DOWN } else { Icon::CARET_RIGHT };
        let caret_color = if caret_resp.hovered() { theme.text() } else { theme.dim() };
        painter.text(
            caret_rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(12.0),
            caret_color,
        );
        if caret_resp.clicked() {
            let was_expanded = expanded;
            state.toggle_expand(id);
            if was_expanded { out.collapsed = Some(id); } else { out.expanded = Some(id); }
        }
        if caret_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    x += caret_w + 2.0;

    // Optional checkbox (compact custom paint to keep row compact).
    if cfg.checkable {
        let bs = 13.0_f32;
        let cb_rect = Rect::from_min_size(Pos2::new(x, cy - bs * 0.5), Vec2::splat(bs));
        let cb_resp = ui.interact(cb_rect, row_id.with("cb"), Sense::click());
        let on = state.is_checked(id);
        let on_t = motion::ease_bool(ui.ctx(), row_id.with("cb_on"), on, motion::FAST);
        let bg = motion::lerp_color(Color32::TRANSPARENT, theme.accent(), on_t);
        painter.rect_filled(cb_rect, CornerRadius::same(2), bg);
        painter.rect_stroke(
            cb_rect,
            CornerRadius::same(2),
            Stroke::new(1.0, motion::lerp_color(theme.border(), theme.accent(), on_t)),
            egui::StrokeKind::Inside,
        );
        if on {
            let c = cb_rect.center();
            let s = bs;
            let p1 = Pos2::new(c.x - s * 0.25, c.y + s * 0.02);
            let p2 = Pos2::new(c.x - s * 0.05, c.y + s * 0.20);
            let p3 = Pos2::new(c.x + s * 0.28, c.y - s * 0.18);
            let stroke = Stroke::new(1.4, Color32::WHITE);
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p2, p3], stroke);
        }
        if cb_resp.clicked() {
            let now_on = state.toggle_checked(id);
            if now_on { out.checked = Some(id); } else { out.unchecked = Some(id); }
        }
        x += bs + 6.0;
    }

    // Optional row icon.
    if let Some(icon_for) = cfg.icon_for {
        if let Some(glyph) = icon_for(item) {
            let iw = 14.0;
            painter.text(
                Pos2::new(x + iw * 0.5, cy),
                Align2::CENTER_CENTER,
                glyph,
                FontId::proportional(st::font_sm()),
                theme.dim(),
            );
            x += iw + 4.0;
        }
    }

    // Label / custom render. We carve a child UI to the remaining width
    // so item_render callbacks can lay out normally.
    let content_rect = Rect::from_min_max(Pos2::new(x, rect.top()), rect.right_bottom());
    if let Some(render) = cfg.item_render {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        render(&mut child, theme, item, x - rect.left());
    } else {
        painter.text(
            Pos2::new(x, cy),
            Align2::LEFT_CENTER,
            item.label(),
            FontId::proportional(st::font_sm()),
            theme.text(),
        );
    }

    // Row click — only register if caret/checkbox didn't consume it (egui
    // gives clicks to the topmost interactive area; our nested interact
    // calls take precedence in their sub-rects).
    if response.clicked() {
        state.selected = Some(id);
        out.clicked = Some(id);
    }
    if response.double_clicked() {
        out.double_clicked = Some(id);
    }
    if response.hovered() {
        state.hovered = Some(id);
    }
}
