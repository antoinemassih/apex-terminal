//! PaneGrid — portable recursive split-pane widget inspired by iced's PaneGrid.
//!
//! Lets the user split any pane horizontally or vertically, resize splits by
//! dragging the divider bar, close panes, and swap pane contents. The widget
//! works with any `ComponentTheme`; it has no dependency on the trading-app's
//! concrete `Theme` type.
//!
//! ### Quick start
//! ```ignore
//! // In your app state:
//! let (mut pane_state, root_id) = PaneState::new(MyContent::default());
//!
//! // In your UI code each frame:
//! PaneGrid::new(&mut pane_state, |ui, _pane_id, content, theme| {
//!     ui.label("hello pane");
//! })
//! .show(ui, theme);
//! ```
//!
//! ### Options
//! - `.show_pane_chrome(false)` — strip border + header. Use when embedding
//!   inside another container that provides its own chrome.
//! - `.splitter_width(w)` — override the 6 px hit-band (keep ≥ 2 px).
//! - `.splitter_color(c)` — override the default `theme.border()` line.
//! - `.on_split(cb)` / `.on_close(cb)` — receive split/close events after the
//!   fact (useful for logging or persistence side-effects).
//!
//! ### Context menu
//! Right-clicking any pane opens a menu with "Split Horizontal",
//! "Split Vertical", and "Close". The widget uses `T::default()` as the initial
//! content for context-menu-triggered splits. Callers who need richer initial
//! content should call `state.split()` directly after receiving the `on_split`
//! callback.
//!
//! ### Serialization
//! `PaneState<T>` derives `serde::{Serialize, Deserialize}` when `T` does.
//! Persist the full state to disk to restore workspace layouts across sessions.

use egui::{
    Color32, CornerRadius, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};
use serde::{Deserialize, Serialize};

use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
// ContextMenu / MenuItem / DangerMenuItem are available for future enhancement
// (Tier B: themed context menus). For now we use egui's built-in context_menu
// with plain Button rows — removes a borrow-conflict between the closure's &mut Ui
// and the ContextMenu::show(&mut Ui) sub-call.
#[allow(unused_imports)]
use super::context_menu::{ContextMenu, DangerMenuItem, MenuItem};
use super::motion;
use crate::ui_kit::tokens as st;

// ─── Identifiers ─────────────────────────────────────────────────────────────

/// Opaque handle for a leaf pane in the layout tree.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub struct PaneId(pub u32);

/// Opaque handle for a split node in the layout tree.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub struct SplitId(pub u32);

// ─── Axis ─────────────────────────────────────────────────────────────────────

/// Direction of a binary split.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Axis {
    /// Left pane | Right pane (split line is vertical).
    Horizontal,
    /// Top pane / Bottom pane (split line is horizontal).
    Vertical,
}

// ─── Layout tree ─────────────────────────────────────────────────────────────

/// Layout-tree node — either a leaf pane or a binary split. Public so hosts
/// can construct preset templates (see chart-app `PaneLayout::from_template`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + for<'de2> Deserialize<'de2>")]
pub enum Node<T> {
    Leaf { id: PaneId, content: T },
    Split {
        id:    SplitId,
        axis:  Axis,
        /// Fraction of the available space given to child `a`; clamped to [0.1, 0.9].
        ratio: f32,
        a:     Box<Node<T>>,
        b:     Box<Node<T>>,
    },
}

impl<T> Node<T> {
    // ── Constructors (for host preset templates) ─────────────────────────────

    /// Build a leaf node. Hosts pass these into `Self::split_node` to compose
    /// preset trees, then hand the root to `PaneState::replace_root`.
    pub fn leaf(id: PaneId, content: T) -> Self {
        Node::Leaf { id, content }
    }

    /// Build a split node from two children. Ratio is clamped at render time.
    pub fn split_node(id: SplitId, axis: Axis, ratio: f32, a: Node<T>, b: Node<T>) -> Self {
        Node::Split { id, axis, ratio, a: Box::new(a), b: Box::new(b) }
    }

    // ── Counting ────────────────────────────────────────────────────────────

    fn pane_count(&self) -> usize {
        match self {
            Node::Leaf { .. } => 1,
            Node::Split { a, b, .. } => a.pane_count() + b.pane_count(),
        }
    }

    // ── Iteration ───────────────────────────────────────────────────────────

    fn collect_panes<'s>(&'s self, out: &mut Vec<(PaneId, &'s T)>) {
        match self {
            Node::Leaf { id, content } => out.push((*id, content)),
            Node::Split { a, b, .. } => {
                a.collect_panes(out);
                b.collect_panes(out);
            }
        }
    }

    fn collect_panes_mut<'s>(&'s mut self, out: &mut Vec<(PaneId, &'s mut T)>) {
        match self {
            Node::Leaf { id, content } => out.push((*id, content)),
            Node::Split { a, b, .. } => {
                a.collect_panes_mut(out);
                b.collect_panes_mut(out);
            }
        }
    }

    // ── Split ───────────────────────────────────────────────────────────────

    /// Replaces the leaf with `target` by a Split node, returning `true` on success.
    /// The old leaf content becomes child `a`; `new_content` becomes child `b`.
    fn do_split(
        &mut self,
        target: PaneId,
        axis:   Axis,
        new_content: T,
        new_pane_id:  PaneId,
        new_split_id: SplitId,
    ) -> Result<(), T> {
        // `Err(new_content)` means "target not found in this subtree".
        match self {
            Node::Leaf { id, .. } if *id == target => {
                // In-place replacement via mem::replace.
                // We swap a temporary placeholder in, then write the real node.
                let old = std::mem::replace(self, Node::Split {
                    id:    new_split_id,
                    axis,
                    ratio: 0.5,
                    // These boxes will be overwritten immediately below.
                    a: Box::new(Node::Leaf { id: target, content: unsafe { std::mem::zeroed() } }),
                    b: Box::new(Node::Leaf { id: new_pane_id, content: unsafe { std::mem::zeroed() } }),
                });
                // old is the real leaf (id == target, real content).
                let old_content = match old {
                    Node::Leaf { content, .. } => content,
                    _ => unreachable!(),
                };
                // Write the real content into the two children.
                if let Node::Split { a, b, .. } = self {
                    *a = Box::new(Node::Leaf { id: target, content: old_content });
                    *b = Box::new(Node::Leaf { id: new_pane_id, content: new_content });
                }
                Ok(())
            }
            Node::Leaf { .. } => Err(new_content), // not the target
            Node::Split { a, b, .. } => {
                match a.do_split(target, axis, new_content, new_pane_id, new_split_id) {
                    Ok(())            => Ok(()),
                    Err(new_content) => b.do_split(target, axis, new_content, new_pane_id, new_split_id),
                }
            }
        }
    }

    // ── Close ───────────────────────────────────────────────────────────────

    /// Remove the leaf with `target`, promoting the sibling into the split's
    /// position.  Returns the removed content on success.
    fn do_close(&mut self, target: PaneId) -> CloseResult<T> {
        match self {
            Node::Leaf { id, .. } => {
                if *id == target { CloseResult::IsRoot } else { CloseResult::NotFound }
            }
            Node::Split { a, b, .. } => {
                // Check if either direct child is the target leaf.
                let a_is_target = matches!(a.as_ref(), Node::Leaf { id, .. } if *id == target);
                let b_is_target = matches!(b.as_ref(), Node::Leaf { id, .. } if *id == target);

                if a_is_target {
                    // Collapse: promote `b` into `self`.
                    let b_node  = std::mem::replace(b.as_mut(), dummy_leaf());
                    let a_owned = std::mem::replace(self, b_node);
                    let content = match a_owned {
                        Node::Split { a, .. } => match *a {
                            Node::Leaf { content, .. } => content,
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    };
                    return CloseResult::Removed(content);
                }

                if b_is_target {
                    // Collapse: promote `a` into `self`.
                    let a_node  = std::mem::replace(a.as_mut(), dummy_leaf());
                    let b_owned = std::mem::replace(self, a_node);
                    let content = match b_owned {
                        Node::Split { b, .. } => match *b {
                            Node::Leaf { content, .. } => content,
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    };
                    return CloseResult::Removed(content);
                }

                // Recurse.
                match a.do_close(target) {
                    CloseResult::NotFound => b.do_close(target),
                    other => other,
                }
            }
        }
    }

    // ── Resize ──────────────────────────────────────────────────────────────

    fn do_resize(&mut self, target: SplitId, new_ratio: f32) {
        match self {
            Node::Leaf { .. } => {}
            Node::Split { id, ratio, a, b, .. } => {
                if *id == target {
                    *ratio = new_ratio.clamp(0.1, 0.9);
                } else {
                    a.do_resize(target, new_ratio);
                    b.do_resize(target, new_ratio);
                }
            }
        }
    }

    // ── Swap ────────────────────────────────────────────────────────────────

    fn find_content_mut(&mut self, target: PaneId) -> Option<&mut T> {
        match self {
            Node::Leaf { id, content } => {
                if *id == target { Some(content) } else { None }
            }
            Node::Split { a, b, .. } => {
                a.find_content_mut(target)
                    .or_else(|| b.find_content_mut(target))
            }
        }
    }
}

enum CloseResult<T> {
    IsRoot,        // target is the root leaf; caller guards against this
    NotFound,
    Removed(T),
}

/// A zeroed-out sentinel leaf used as a placeholder during in-place swaps.
/// The poison value is immediately overwritten before it can be read or dropped.
fn dummy_leaf<T>() -> Node<T> {
    Node::Leaf { id: PaneId(u32::MAX), content: unsafe { std::mem::zeroed() } }
}

// ─── PaneState ───────────────────────────────────────────────────────────────

/// Root layout state — a binary tree of splits and leaf panes.
///
/// Generic over `T`, the per-pane content type. Derives
/// `Serialize`/`Deserialize` when `T` does (opt-in via the serde bound).
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + for<'de2> Deserialize<'de2>")]
pub struct PaneState<T> {
    pub(crate) layout: Node<T>,
    next_pane:  u32,
    next_split: u32,
    focus:      Option<PaneId>,
}

impl<T> PaneState<T> {
    /// Create a new single-pane layout. Returns the state and the root pane's ID.
    pub fn new(initial: T) -> (Self, PaneId) {
        let root_id = PaneId(0);
        let state = Self {
            layout: Node::Leaf { id: root_id, content: initial },
            next_pane:  1,
            next_split: 0,
            focus:      Some(root_id),
        };
        (state, root_id)
    }

    /// Split `pane` along `axis`, inserting `new_content` as the new sibling.
    /// Returns the new pane's `PaneId`, or `None` if `pane` was not found OR
    /// if splitting it would push the tree past `MAX_SPLIT_DEPTH` (8 levels),
    /// at which point further splits produce panes too small to use.
    pub fn split(&mut self, pane: PaneId, axis: Axis, new_content: T) -> Option<PaneId> {
        const MAX_SPLIT_DEPTH: usize = 8;
        if depth_of(&self.layout, pane, 0) >= MAX_SPLIT_DEPTH {
            return None;
        }
        let new_pane_id  = PaneId(self.next_pane);
        let new_split_id = SplitId(self.next_split);
        match self.layout.do_split(pane, axis, new_content, new_pane_id, new_split_id) {
            Ok(()) => {
                self.next_pane  += 1;
                self.next_split += 1;
                self.focus = Some(new_pane_id);
                Some(new_pane_id)
            }
            Err(_) => None,
        }
    }

    /// Remove `pane` from the layout. Returns its content, or `None` if it is
    /// the only remaining pane (closing the last pane is a no-op).
    pub fn close(&mut self, pane: PaneId) -> Option<T> {
        if self.layout.pane_count() <= 1 {
            return None; // never close the last pane
        }
        match self.layout.do_close(pane) {
            CloseResult::Removed(content) => {
                if self.focus == Some(pane) {
                    // Re-collect panes after mutation; can't use iter_panes() here
                    // because that borrows self immutably while content was just moved.
                    let mut out: Vec<(PaneId, &T)> = Vec::new();
                    self.layout.collect_panes(&mut out);
                    self.focus = out.first().map(|(id, _)| *id);
                }
                Some(content)
            }
            _ => None,
        }
    }

    /// Swap the *contents* of two panes (IDs stay fixed, content moves).
    /// Returns `true` if both panes existed.
    pub fn swap(&mut self, a: PaneId, b: PaneId) -> bool {
        if a == b {
            return true; // trivially a no-op, but "succeeded"
        }
        // We need two separate mutable borrows into the tree. Collect all
        // (PaneId, &mut T) pairs first, then swap via raw pointers.
        let mut pairs: Vec<(PaneId, *mut T)> = Vec::new();
        collect_ptrs(&mut self.layout, &mut pairs);

        let pa = pairs.iter().find(|(id, _)| *id == a).map(|(_, p)| *p);
        let pb = pairs.iter().find(|(id, _)| *id == b).map(|(_, p)| *p);
        if let (Some(pa), Some(pb)) = (pa, pb) {
            // SAFETY: `pa` and `pb` point to distinct `content` fields inside
            // different `Node::Leaf` variants (enforced by unique PaneIds).
            unsafe { std::ptr::swap(pa, pb); }
            true
        } else {
            false
        }
    }

    /// Update the split ratio for the split identified by `split`. Clamped to [0.1, 0.9].
    pub fn resize(&mut self, split: SplitId, ratio: f32) {
        self.layout.do_resize(split, ratio);
    }

    /// Set the focused pane. The focused pane gets a highlighted border.
    pub fn focus(&mut self, pane: PaneId) {
        self.focus = Some(pane);
    }

    /// Currently focused pane, if any.
    pub fn focused(&self) -> Option<PaneId> {
        self.focus
    }

    /// Depth-first iterator over all (PaneId, &T) leaf pairs.
    pub fn iter_panes(&self) -> impl Iterator<Item = (PaneId, &T)> {
        let mut out = Vec::new();
        self.layout.collect_panes(&mut out);
        out.into_iter()
    }

    /// Depth-first iterator over all (PaneId, &mut T) leaf pairs.
    pub fn iter_panes_mut(&mut self) -> impl Iterator<Item = (PaneId, &mut T)> {
        let mut out = Vec::new();
        self.layout.collect_panes_mut(&mut out);
        out.into_iter()
    }

    /// Number of leaf panes.
    pub fn pane_count(&self) -> usize {
        self.layout.pane_count()
    }

    /// Walk the layout tree and yield `(PaneId, &T, Rect)` for each leaf,
    /// with rects computed from `full_rect` divided according to every
    /// `Split { axis, ratio }` along the path. Includes a `gap` between
    /// sibling panes (carved out of the split boundary).
    ///
    /// Hosts that drive their own per-pane render loop (rather than passing
    /// a closure to `PaneGrid::show`) use this to align legacy iteration
    /// with the new tree topology — see chart-app `PaneLayout::pane_rects`.
    pub fn iter_panes_with_rects(
        &self,
        full_rect: egui::Rect,
        gap: f32,
    ) -> Vec<(PaneId, &T, egui::Rect)> {
        let mut out = Vec::new();
        collect_with_rects(&self.layout, full_rect, gap, &mut out);
        out
    }

    /// Same as `iter_panes_with_rects` but yields mutable references.
    pub fn iter_panes_with_rects_mut(
        &mut self,
        full_rect: egui::Rect,
        gap: f32,
    ) -> Vec<(PaneId, &mut T, egui::Rect)> {
        let mut out = Vec::new();
        collect_with_rects_mut(&mut self.layout, full_rect, gap, &mut out);
        out
    }

    /// Walk the tree yielding `(SplitId, axis, ratio, parent_rect)` for each
    /// Split node, with rects computed from `full_rect` and `gap`. Hosts use
    /// this to attach drag-handle hit-bands at each interior split — see the
    /// chart-app `PaneLayout::show_overlays`.
    pub fn iter_splits_with_rects(
        &self,
        full_rect: egui::Rect,
        gap: f32,
    ) -> Vec<(SplitId, Axis, f32, egui::Rect)> {
        let mut out = Vec::new();
        collect_split_rects(&self.layout, full_rect, gap, &mut out);
        out
    }

    /// Replace the entire layout tree with a new one. Used by hosts to swap
    /// from one preset template to another (e.g. "2×2 grid" → "1 + 2 below")
    /// without having to drop and rebuild `PaneState`.
    ///
    /// Resets `focus` to the first pane in the new tree. ID counters
    /// continue from the high-water mark so subsequent splits keep producing
    /// fresh IDs.
    pub fn replace_root(&mut self, new_root: Node<T>) {
        let mut ids = Vec::new();
        new_root.collect_ids(&mut ids);
        let max_pane  = ids.iter().filter_map(|n| if let NodeId::Pane(p)  = n { Some(p.0) } else { None }).max().unwrap_or(0);
        let max_split = ids.iter().filter_map(|n| if let NodeId::Split(s) = n { Some(s.0) } else { None }).max().unwrap_or(0);
        self.layout    = new_root;
        self.next_pane  = self.next_pane.max(max_pane + 1);
        self.next_split = self.next_split.max(max_split + 1);
        let mut first = Vec::new();
        self.layout.collect_panes(&mut first);
        self.focus = first.first().map(|(id, _)| *id);
    }
}

/// Helper enum for `replace_root`'s ID accounting.
enum NodeId { Pane(PaneId), Split(SplitId) }

impl<T> Node<T> {
    fn collect_ids(&self, out: &mut Vec<NodeId>) {
        match self {
            Node::Leaf { id, .. } => out.push(NodeId::Pane(*id)),
            Node::Split { id, a, b, .. } => {
                out.push(NodeId::Split(*id));
                a.collect_ids(out);
                b.collect_ids(out);
            }
        }
    }
}

fn collect_split_rects<T>(
    node: &Node<T>,
    rect: egui::Rect,
    gap: f32,
    out: &mut Vec<(SplitId, Axis, f32, egui::Rect)>,
) {
    if let Node::Split { id, axis, ratio, a, b } = node {
        out.push((*id, *axis, *ratio, rect));
        let (ra, rb) = split_rect(rect, *axis, *ratio, gap);
        collect_split_rects(a, ra, gap, out);
        collect_split_rects(b, rb, gap, out);
    }
}

fn collect_with_rects<'s, T>(
    node: &'s Node<T>,
    rect: egui::Rect,
    gap: f32,
    out: &mut Vec<(PaneId, &'s T, egui::Rect)>,
) {
    match node {
        Node::Leaf { id, content } => out.push((*id, content, rect)),
        Node::Split { axis, ratio, a, b, .. } => {
            let (ra, rb) = split_rect(rect, *axis, *ratio, gap);
            collect_with_rects(a, ra, gap, out);
            collect_with_rects(b, rb, gap, out);
        }
    }
}

fn collect_with_rects_mut<'s, T>(
    node: &'s mut Node<T>,
    rect: egui::Rect,
    gap: f32,
    out: &mut Vec<(PaneId, &'s mut T, egui::Rect)>,
) {
    match node {
        Node::Leaf { id, content } => out.push((*id, content, rect)),
        Node::Split { axis, ratio, a, b, .. } => {
            let (ra, rb) = split_rect(rect, *axis, *ratio, gap);
            collect_with_rects_mut(a, ra, gap, out);
            collect_with_rects_mut(b, rb, gap, out);
        }
    }
}

/// Divide `rect` into two children along `axis`. `ratio` is the fraction
/// of `rect`'s primary dimension given to child `a`. `gap` is reserved
/// between the two children as a gutter (carved evenly from the split line).
pub fn split_rect(rect: egui::Rect, axis: Axis, ratio: f32, gap: f32) -> (egui::Rect, egui::Rect) {
    let r = ratio.clamp(0.1, 0.9);
    match axis {
        Axis::Horizontal => {
            let avail = (rect.width() - gap).max(0.0);
            let aw = avail * r;
            let bw = avail - aw;
            let ar = egui::Rect::from_min_size(rect.min, egui::vec2(aw, rect.height()));
            let br = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + aw + gap, rect.min.y),
                egui::vec2(bw, rect.height()),
            );
            (ar, br)
        }
        Axis::Vertical => {
            let avail = (rect.height() - gap).max(0.0);
            let ah = avail * r;
            let bh = avail - ah;
            let ar = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), ah));
            let br = egui::Rect::from_min_size(
                egui::pos2(rect.min.x, rect.min.y + ah + gap),
                egui::vec2(rect.width(), bh),
            );
            (ar, br)
        }
    }
}

/// P17 #7 — compute the depth of `target` in `node` (root = 0). Returns
/// `usize::MAX` if target isn't in the tree, so the depth check in
/// `PaneState::split` becomes a no-op (target not found → split returns
/// None for a different reason anyway).
fn depth_of<T>(node: &Node<T>, target: PaneId, current: usize) -> usize {
    match node {
        Node::Leaf { id, .. } => if *id == target { current } else { usize::MAX },
        Node::Split { a, b, .. } => {
            let da = depth_of(a, target, current + 1);
            if da != usize::MAX { return da; }
            depth_of(b, target, current + 1)
        }
    }
}

// Collect raw mutable pointers for the swap helper.
fn collect_ptrs<T>(node: &mut Node<T>, out: &mut Vec<(PaneId, *mut T)>) {
    match node {
        Node::Leaf { id, content } => out.push((*id, content as *mut T)),
        Node::Split { a, b, .. } => {
            collect_ptrs(a, out);
            collect_ptrs(b, out);
        }
    }
}

// ─── PaneGrid widget ──────────────────────────────────────────────────────────

/// Recursive split-pane widget. Renders all leaf panes via the user-supplied
/// `render` closure and paints interactive splitter bars between siblings.
///
/// # Type parameters
/// - `T` — per-pane content stored in `PaneState<T>`. Must implement `Default`
///   so the context-menu "Split" action can create new panes.
/// - `R` — render closure type.
#[must_use = "Widget does nothing until `.show(ui, theme)` is called"]
pub struct PaneGrid<'a, T, R> {
    state:            &'a mut PaneState<T>,
    render:           R,
    splitter_width:   f32,
    splitter_color:   Option<Color32>,
    show_pane_chrome: bool,
    on_split:         Option<Box<dyn Fn(PaneId, Axis) + 'a>>,
    on_close:         Option<Box<dyn Fn(PaneId) + 'a>>,
}

impl<'a, T, R> PaneGrid<'a, T, R>
where
    T: Default,
    R: Fn(&mut Ui, PaneId, &mut T, &dyn ComponentTheme),
{
    /// Construct with mutable access to the layout state and a per-pane render
    /// closure.
    pub fn new(state: &'a mut PaneState<T>, render: R) -> Self {
        Self {
            state,
            render,
            splitter_width:   6.0, // intentional: standard hit-band width per spec
            splitter_color:   None,
            show_pane_chrome: true,
            on_split:         None,
            on_close:         None,
        }
    }

    /// Override the splitter hit-band width (default 6 px; minimum 2 px).
    pub fn splitter_width(mut self, w: f32) -> Self {
        self.splitter_width = w.max(2.0);
        self
    }

    /// Override the splitter line color (default: `theme.border()`).
    pub fn splitter_color(mut self, c: Color32) -> Self {
        self.splitter_color = Some(c);
        self
    }

    /// Show/hide the per-pane chrome (border + header bar with close button).
    /// Default `true`. Pass `false` to embed inside an existing container.
    pub fn show_pane_chrome(mut self, b: bool) -> Self {
        self.show_pane_chrome = b;
        self
    }

    /// Callback fired after a split completes (new pane ID, axis).
    pub fn on_split(mut self, f: impl Fn(PaneId, Axis) + 'a) -> Self {
        self.on_split = Some(Box::new(f));
        self
    }

    /// Callback fired after a close completes.
    pub fn on_close(mut self, f: impl Fn(PaneId) + 'a) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }

    /// Lay out and paint all panes. Applies any pending mutations (split, close,
    /// resize, focus) that result from user interaction.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) {
        let avail = ui.available_rect_before_wrap();
        if avail.width() < 1.0 || avail.height() < 1.0 {
            return;
        }

        let PaneGrid {
            state,
            render,
            splitter_width,
            splitter_color,
            show_pane_chrome,
            on_split,
            on_close,
        } = self;

        let splitter_color = splitter_color.unwrap_or_else(|| palette_ct(theme).base(Tone::Border));

        let mut pending: Vec<PendingAction> = Vec::new();

        paint_node(
            ui,
            &mut state.layout,
            avail,
            state.focus,
            theme,
            &render,
            splitter_width,
            splitter_color,
            show_pane_chrome,
            &mut pending,
        );

        // Apply mutations collected during painting.
        for action in pending {
            match action {
                PendingAction::Split(pane_id, axis) => {
                    let new_content = T::default();
                    if let Some(new_id) = state.split(pane_id, axis, new_content) {
                        if let Some(ref cb) = on_split {
                            cb(new_id, axis);
                        }
                    }
                }
                PendingAction::Close(pane_id) => {
                    state.close(pane_id);
                    if let Some(ref cb) = on_close {
                        cb(pane_id);
                    }
                }
                PendingAction::Focus(pane_id) => {
                    state.focus = Some(pane_id);
                }
                PendingAction::Resize(split_id, ratio) => {
                    state.resize(split_id, ratio);
                }
            }
        }
    }
}

// ─── Pending mutations ────────────────────────────────────────────────────────

/// Actions queued during the paint pass and applied after painting finishes.
#[derive(Debug)]
enum PendingAction {
    Split(PaneId, Axis),
    Close(PaneId),
    Focus(PaneId),
    Resize(SplitId, f32),
}

// ─── Recursive painter ────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn paint_node<T, R>(
    ui:              &mut Ui,
    node:            &mut Node<T>,
    rect:            Rect,
    focused:         Option<PaneId>,
    theme:           &dyn ComponentTheme,
    render:          &R,
    splitter_width:  f32,
    splitter_color:  Color32,
    show_chrome:     bool,
    pending:         &mut Vec<PendingAction>,
) where
    T: Default,
    R: Fn(&mut Ui, PaneId, &mut T, &dyn ComponentTheme),
{
    match node {
        Node::Leaf { id, content } => {
            paint_leaf(
                ui, *id, content, rect, focused, theme, render,
                show_chrome, pending,
            );
        }
        Node::Split { id: split_id, axis, ratio, a, b } => {
            let (rect_a, rect_div, rect_b) = compute_split_rects(rect, *axis, *ratio, splitter_width);

            // Recurse into children.
            paint_node(ui, a, rect_a, focused, theme, render, splitter_width, splitter_color, show_chrome, pending);
            paint_node(ui, b, rect_b, focused, theme, render, splitter_width, splitter_color, show_chrome, pending);

            // Splitter interaction.
            let div_egui_id = ui.make_persistent_id(("pg_div", split_id.0));
            let div_resp = ui.interact(rect_div, div_egui_id, Sense::drag());

            let cursor = match axis {
                Axis::Horizontal => CursorIcon::ResizeHorizontal,
                Axis::Vertical   => CursorIcon::ResizeVertical,
            };
            if div_resp.hovered() || div_resp.dragged() {
                ui.ctx().set_cursor_icon(cursor);
            }

            if div_resp.dragged() {
                if let Some(ptr) = ui.ctx().pointer_latest_pos() {
                    let total = match axis {
                        Axis::Horizontal => rect.width(),
                        Axis::Vertical   => rect.height(),
                    };
                    if total > 0.0 {
                        let raw = match axis {
                            Axis::Horizontal => (ptr.x - rect.left()) / total,
                            Axis::Vertical   => (ptr.y - rect.top())  / total,
                        };
                        pending.push(PendingAction::Resize(*split_id, raw));
                    }
                }
            }

            // Animated splitter line.
            let anim_id  = div_egui_id.with("anim");
            let hover_t  = motion::ease_bool(
                ui.ctx(), anim_id,
                div_resp.hovered() || div_resp.dragged(),
                motion::FAST,
            );
            let idle_c   = st::color_alpha(splitter_color, st::alpha_strong());
            let hot_c    = st::color_alpha(palette_ct(theme).base(Tone::Accent), st::alpha_heavy());
            let line_col = motion::lerp_color(idle_c, hot_c, hover_t);
            ui.painter().rect_filled(rect_div, CornerRadius::ZERO, line_col);
        }
    }
}

fn paint_leaf<T, R>(
    ui:          &mut Ui,
    id:          PaneId,
    content:     &mut T,
    rect:        Rect,
    focused:     Option<PaneId>,
    theme:       &dyn ComponentTheme,
    render:      &R,
    show_chrome: bool,
    pending:     &mut Vec<PendingAction>,
) where
    T: Default,
    R: Fn(&mut Ui, PaneId, &mut T, &dyn ComponentTheme),
{
    let is_focused = focused == Some(id);

    // Draw optional chrome (border + header bar).
    let (content_rect, close_clicked) = if show_chrome {
        draw_pane_chrome(ui, id, rect, is_focused, theme)
    } else {
        (rect, false)
    };

    if close_clicked {
        pending.push(PendingAction::Close(id));
    }

    // Focus on click.
    let sense_id = ui.make_persistent_id(("pg_leaf", id.0));
    let leaf_resp = ui.interact(content_rect, sense_id, Sense::click());
    if leaf_resp.clicked() && !is_focused {
        pending.push(PendingAction::Focus(id));
    }

    // Right-click context menu — use egui's built-in context_menu for the
    // response, then render menu items via simple egui::Button rows.
    // We store the chosen action in egui temp memory so the deferred action
    // survives the frame boundary between the context_menu closure and the
    // pending-action push below.
    let ctx_action_key = egui::Id::new(("pg_ctx_act", id.0));
    leaf_resp.context_menu(|ctx_ui| {
        ctx_ui.spacing_mut().button_padding = egui::vec2(st::gap_sm(), st::gap_xs());
        if ctx_ui.button("Split Horizontal").clicked() {
            ctx_ui.memory_mut(|m| m.data.insert_temp::<u8>(ctx_action_key, 1));
            ctx_ui.close_menu();
        }
        if ctx_ui.button("Split Vertical").clicked() {
            ctx_ui.memory_mut(|m| m.data.insert_temp::<u8>(ctx_action_key, 2));
            ctx_ui.close_menu();
        }
        ctx_ui.separator();
        if ctx_ui.button("Close").clicked() {
            ctx_ui.memory_mut(|m| m.data.insert_temp::<u8>(ctx_action_key, 3));
            ctx_ui.close_menu();
        }
    });

    // Pick up deferred context-menu action (fires the frame *after* the menu closes).
    if let Some(act) = ui.memory(|m| m.data.get_temp::<u8>(ctx_action_key)) {
        ui.memory_mut(|m| m.data.remove::<u8>(ctx_action_key));
        match act {
            1 => pending.push(PendingAction::Split(id, Axis::Horizontal)),
            2 => pending.push(PendingAction::Split(id, Axis::Vertical)),
            3 => pending.push(PendingAction::Close(id)),
            _ => {}
        }
    }

    // Render user content inside the content rect.
    if content_rect.width() > 1.0 && content_rect.height() > 1.0 {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(content_rect);
        render(&mut child, id, content, theme);
    }
}

/// Paint a thin border + compact header bar with a close "×" button.
/// Returns (inner content rect, whether the close button was clicked this frame).
fn draw_pane_chrome(
    ui:       &mut Ui,
    id:       PaneId,
    rect:     Rect,
    focused:  bool,
    theme:    &dyn ComponentTheme,
) -> (Rect, bool) {
    // Header height = compact row token (20 px default).
    let header_h  = st::row_height_compact();
    // Border stroke uses thin token.
    let border_px = st::stroke_thin();

    // Outer border.
    let border_color = if focused {
        st::color_alpha(palette_ct(theme).base(Tone::Accent), st::alpha_subtle())
    } else {
        st::color_alpha(palette_ct(theme).base(Tone::Border), st::alpha_subtle())
    };
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(border_px, border_color),
        egui::StrokeKind::Inside,
    );

    // Header band.
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), header_h));
    ui.painter().rect_filled(
        header_rect,
        CornerRadius::ZERO,
        st::color_alpha(palette_ct(theme).base(Tone::Surface), st::alpha_subtle()),
    );

    // Title label.
    let label_x = header_rect.left() + st::gap_xs();
    ui.painter().text(
        Pos2::new(label_x, header_rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("Pane {}", id.0),
        egui::FontId::proportional(st::font_xs()),
        st::color_alpha(palette_ct(theme).base(Tone::Dim), st::alpha_strong()),
    );

    // Close "×" button — icon-sized square in the right side of header.
    // 14 px is intentional: minimum comfortable tap target for a close glyph.
    let close_sz   = 14.0_f32;
    let close_rect = Rect::from_center_size(
        Pos2::new(
            header_rect.right() - close_sz * 0.5 - st::gap_xs(),
            header_rect.center().y,
        ),
        Vec2::splat(close_sz),
    );
    let close_id   = ui.make_persistent_id(("pg_close_btn", id.0));
    let close_resp = ui.interact(close_rect, close_id, Sense::click());

    let glyph_color = if close_resp.hovered() {
        theme.danger()
    } else {
        st::color_alpha(palette_ct(theme).base(Tone::Dim), st::alpha_strong())
    };
    ui.painter().text(
        close_rect.center(),
        egui::Align2::CENTER_CENTER,
        "×",
        egui::FontId::proportional(st::font_sm()),
        glyph_color,
    );

    // Inset the content rect below the header and inside the border.
    let content_rect = Rect::from_min_max(
        Pos2::new(rect.left() + border_px, rect.top() + header_h),
        Pos2::new(rect.right() - border_px, rect.bottom() - border_px),
    );

    (content_rect, close_resp.clicked())
}

// ─── Geometry ─────────────────────────────────────────────────────────────────

/// Compute three rects: [child A] [divider] [child B].
fn compute_split_rects(rect: Rect, axis: Axis, ratio: f32, splitter_px: f32) -> (Rect, Rect, Rect) {
    let half = splitter_px * 0.5;
    match axis {
        Axis::Horizontal => {
            let x = rect.left() + rect.width() * ratio;
            let a = Rect::from_min_max(rect.min, Pos2::new(x - half, rect.bottom()));
            let d = Rect::from_min_max(Pos2::new(x - half, rect.top()), Pos2::new(x + half, rect.bottom()));
            let b = Rect::from_min_max(Pos2::new(x + half, rect.top()), rect.max);
            (a, d, b)
        }
        Axis::Vertical => {
            let y = rect.top() + rect.height() * ratio;
            let a = Rect::from_min_max(rect.min, Pos2::new(rect.right(), y - half));
            let d = Rect::from_min_max(Pos2::new(rect.left(), y - half), Pos2::new(rect.right(), y + half));
            let b = Rect::from_min_max(Pos2::new(rect.left(), y + half), rect.max);
            (a, d, b)
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn state(v: i32) -> (PaneState<i32>, PaneId) { PaneState::new(v) }

    // ── Core data-model tests ─────────────────────────────────────────────

    #[test]
    fn test_split_increases_pane_count() {
        let (mut s, root) = state(1);
        assert_eq!(s.pane_count(), 1);

        let p2 = s.split(root, Axis::Horizontal, 2).expect("split #1");
        assert_eq!(s.pane_count(), 2);

        let _p3 = s.split(p2, Axis::Vertical, 3).expect("split #2");
        assert_eq!(s.pane_count(), 3);

        let _p4 = s.split(root, Axis::Vertical, 4).expect("split #3");
        assert_eq!(s.pane_count(), 4);
    }

    #[test]
    fn test_close_decreases_pane_count() {
        let (mut s, root) = state(10);
        let p2 = s.split(root, Axis::Horizontal, 20).unwrap();
        let p3 = s.split(p2, Axis::Vertical, 30).unwrap();
        assert_eq!(s.pane_count(), 3);

        let r3 = s.close(p3);
        assert_eq!(s.pane_count(), 2);
        assert_eq!(r3, Some(30));

        let r2 = s.close(p2);
        assert_eq!(s.pane_count(), 1);
        assert_eq!(r2, Some(20));
    }

    #[test]
    fn test_close_last_pane_returns_none() {
        let (mut s, root) = state(42);
        let result = s.close(root);
        assert!(result.is_none(), "close on the only pane must return None");
        assert_eq!(s.pane_count(), 1, "pane count must stay 1");
    }

    #[test]
    fn test_swap_swaps_contents() {
        let (mut s, root) = state(100);
        let p2 = s.split(root, Axis::Horizontal, 200).unwrap();

        let ok = s.swap(root, p2);
        assert!(ok, "swap must succeed for two valid panes");

        let vals: std::collections::HashMap<PaneId, i32> =
            s.iter_panes().map(|(id, &v)| (id, v)).collect();
        assert_eq!(vals[&root], 200, "root should hold 200 after swap");
        assert_eq!(vals[&p2],   100, "p2 should hold 100 after swap");
    }

    #[test]
    fn test_resize_clamps() {
        let (mut s, root) = state(1);
        let _p2 = s.split(root, Axis::Horizontal, 2).unwrap();

        let split_id = match &s.layout {
            Node::Split { id, .. } => *id,
            _ => panic!("expected a Split at root after split"),
        };

        s.resize(split_id, 0.99); // above maximum
        let ratio_after_max = match &s.layout {
            Node::Split { ratio, .. } => *ratio,
            _ => panic!(),
        };
        assert!((ratio_after_max - 0.9).abs() < 0.001, "must clamp to 0.9, got {}", ratio_after_max);

        s.resize(split_id, 0.01); // below minimum
        let ratio_after_min = match &s.layout {
            Node::Split { ratio, .. } => *ratio,
            _ => panic!(),
        };
        assert!((ratio_after_min - 0.1).abs() < 0.001, "must clamp to 0.1, got {}", ratio_after_min);
    }

    #[test]
    fn test_iter_panes_returns_all() {
        let (mut s, root) = state(1);
        let p2  = s.split(root, Axis::Horizontal, 2).unwrap();
        let p3  = s.split(p2,   Axis::Vertical,   3).unwrap();
        let _p4 = s.split(root, Axis::Vertical,   4).unwrap();

        let ids: Vec<PaneId> = s.iter_panes().map(|(id, _)| id).collect();
        assert_eq!(ids.len(), 4);
        for id in [root, p2, p3] {
            assert!(ids.contains(&id), "{:?} not found in iter_panes", id);
        }
    }

    #[test]
    fn test_split_nonexistent_pane_returns_none() {
        let (mut s, _root) = state(1);
        let result = s.split(PaneId(999), Axis::Horizontal, 2);
        assert!(result.is_none());
        assert_eq!(s.pane_count(), 1);
    }

    #[test]
    fn test_focus_follows_new_split() {
        let (mut s, root) = state(1);
        assert_eq!(s.focused(), Some(root));
        let p2 = s.split(root, Axis::Horizontal, 2).unwrap();
        assert_eq!(s.focused(), Some(p2), "focus must move to the newly created pane");
    }

    #[test]
    fn test_iter_panes_mut_mutation() {
        let (mut s, root) = state(10);
        let _p2 = s.split(root, Axis::Horizontal, 20).unwrap();
        for (_, v) in s.iter_panes_mut() {
            *v *= 10;
        }
        let vals: Vec<i32> = s.iter_panes().map(|(_, &v)| v).collect();
        assert!(vals.contains(&100), "10 * 10 = 100 expected");
        assert!(vals.contains(&200), "20 * 10 = 200 expected");
    }

    #[test]
    fn test_close_root_sibling_is_promoted() {
        // Split root → [root | p2].  Close root. p2 should become the new root.
        let (mut s, root) = state(42);
        let p2 = s.split(root, Axis::Horizontal, 99).unwrap();
        assert_eq!(s.pane_count(), 2);
        let removed = s.close(root).expect("should be able to close non-last pane");
        assert_eq!(removed, 42);
        assert_eq!(s.pane_count(), 1);
        // The surviving pane should be p2 with value 99.
        let first = s.iter_panes().next().unwrap();
        assert_eq!(*first.1, 99);
    }
}
