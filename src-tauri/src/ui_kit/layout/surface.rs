//! `Surface` — the styled container primitive.
//!
//! The composable unit the rest of the UI should be built from: one type that
//! owns **layout** (via [`Flex`]/Taffy), **elevation**, **border** and
//! **radius**, all expressed on the typed scales in [`crate::ui_kit::scale`].
//! Think of it as the design system's `<div className="flex gap-2 p-3 rounded-md
//! bg-panel border">`.
//!
//! ```ignore
//! Surface::row()
//!     .gap(Space::Sm)
//!     .pad(Space::Md)
//!     .bg(Level::Panel)
//!     .border(Weight::Thin)
//!     .radius(Radius::Style)      // rounded on Aperture, sharp on Meridien
//!     .item(Item::fixed(16.0))
//!     .item(Item::grow(1.0))
//!     .show(ui, t, |idx, ui| { /* children */ });
//! ```
//!
//! ## Why this instead of `egui::Frame` + a Flex
//!
//! Three reasons, all learned from the 2026-07-31 audit:
//!
//! 1. **The scales are closed.** `pad(Space::Md)` cannot become `pad(13.7)`.
//!    Tokens alone did not prevent drift — 903 off-token primitives got in
//!    while tokens existed — because every API took a bare `f32`.
//! 2. **`Radius::Style` resolves per-STYLE.** One spec renders rounded on
//!    tiled styles and sharp on flush ones, so call sites stop hardcoding a
//!    rounding that is wrong on half the styles (87 sites were pinned to 0.0
//!    and 68 to non-zero — broken in both directions).
//! 3. **Elevation goes through the theme ladder**, so it follows all 21
//!    palettes including the 5 light ones, instead of freezing a dark value.
//!
//! ## What it does NOT own
//!
//! Text. Type comes from the `TextStyle` tiers, which cascade through egui's
//! inherited `text_styles` table — a `Surface` child `Ui` inherits `Style`, so
//! tiers reach it automatically.

use egui::{Rect, Ui};

use super::flex::{Align, Flex, Item, Justify};
use crate::ui_kit::scale::{Level, Radius, Space, Weight};
use crate::ui_kit::widgets::theme::ComponentTheme;

/// A styled, laid-out container.
#[must_use = "Surface does nothing until `.show(...)` is called"]
pub struct Surface {
    flex: Flex,
    bg: Level,
    border: Weight,
    /// Border colour override; defaults to the theme's surface border.
    border_color: Option<egui::Color32>,
    radius: Radius,
    pad: Space,
}

impl Surface {
    /// Horizontal container.
    pub fn row() -> Self { Self::new(Flex::row()) }
    /// Vertical container.
    pub fn column() -> Self { Self::new(Flex::column()) }

    fn new(flex: Flex) -> Self {
        Self {
            flex,
            bg: Level::None,
            border: Weight::None,
            border_color: None,
            radius: Radius::None,
            pad: Space::None,
        }
    }

    // ── Layout (typed scales only) ──────────────────────────────────────────

    /// Gutter between children.
    pub fn gap(mut self, s: Space) -> Self {
        self.flex = self.flex.gap(s.px());
        self
    }

    /// Inner padding on all sides.
    pub fn pad(mut self, s: Space) -> Self {
        self.pad = s;
        self.flex = self.flex.padding(s.px());
        self
    }

    pub fn align(mut self, a: Align) -> Self { self.flex = self.flex.align(a); self }
    pub fn justify(mut self, j: Justify) -> Self { self.flex = self.flex.justify(j); self }
    pub fn wrap(mut self, on: bool) -> Self { self.flex = self.flex.wrap(on); self }
    pub fn item(mut self, i: Item) -> Self { self.flex = self.flex.item(i); self }
    pub fn items(mut self, it: impl IntoIterator<Item = Item>) -> Self {
        self.flex = self.flex.items(it);
        self
    }

    // ── Painting (typed scales only) ────────────────────────────────────────

    /// Which surface level to fill with. [`Level::None`] paints nothing.
    pub fn bg(mut self, l: Level) -> Self { self.bg = l; self }

    /// Border weight. [`Weight::None`] draws no border.
    pub fn border(mut self, w: Weight) -> Self { self.border = w; self }

    /// Override the border colour (defaults to the theme's surface border).
    pub fn border_color(mut self, c: egui::Color32) -> Self {
        self.border_color = Some(c);
        self
    }

    /// Corner radius. Prefer [`Radius::Style`] for panel/card chrome so the
    /// active style decides rounded-vs-sharp.
    pub fn radius(mut self, r: Radius) -> Self { self.radius = r; self }

    /// Render: paint the surface, then lay out and render the children.
    ///
    /// Returns each child's value, plus the container rect (for callers that
    /// need to anchor something to it).
    pub fn show<T: ComponentTheme, R>(
        self,
        ui: &mut Ui,
        t: &T,
        add_item: impl FnMut(usize, &mut Ui) -> R,
    ) -> SurfaceResponse<R> {
        let Surface { flex, bg, border, border_color, radius, pad: _ } = self;

        // Solve first so we know the rect to paint BEHIND the children.
        let avail = ui.available_size_before_wrap();
        let rects = flex.solve(avail);
        let origin = ui.cursor().min;
        let content = rects
            .iter()
            .fold(Rect::NOTHING, |acc, r| acc.union(*r))
            .translate(origin.to_vec2());

        // The painted box is the content union grown back out by the padding —
        // children are already inset by it, so the union alone would clip the
        // background to the content and lose the padding band.
        let outer = if content.is_positive() {
            content.expand(self_pad_px(&rects, avail))
        } else {
            Rect::from_min_size(origin, egui::vec2(avail.x.max(0.0), 0.0))
        };

        if outer.is_positive() {
            let cr = radius.cr();
            if let Some(fill) = bg.color(t) {
                ui.painter().rect_filled(outer, cr, fill);
            }
            if border != Weight::None {
                let col = border_color.unwrap_or_else(|| t.surface_border());
                ui.painter().rect_stroke(
                    outer,
                    cr,
                    egui::Stroke::new(border.px(), col),
                    egui::StrokeKind::Inside,
                );
            }
        }

        let inner = flex_show_into(ui, &rects, origin, add_item);
        SurfaceResponse { rect: outer, inner }
    }
}

/// Padding actually applied, inferred from the solved geometry. Taffy already
/// inset the children, so re-deriving it here keeps a single source of truth
/// instead of storing it twice and risking drift.
fn self_pad_px(rects: &[Rect], _avail: egui::Vec2) -> f32 {
    rects.first().map(|r| r.left().max(0.0)).unwrap_or(0.0)
}

/// Render children into pre-solved rects. Split out so `Surface` can paint its
/// background *between* solving and rendering.
fn flex_show_into<R>(
    ui: &mut Ui,
    rects: &[Rect],
    origin: egui::Pos2,
    mut add_item: impl FnMut(usize, &mut Ui) -> R,
) -> Vec<R> {
    let mut out = Vec::with_capacity(rects.len());
    let mut used = Rect::NOTHING;
    for (idx, r) in rects.iter().enumerate() {
        let rect = r.translate(origin.to_vec2());
        used = used.union(rect);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        out.push(add_item(idx, &mut child));
    }
    if used.is_positive() {
        ui.allocate_rect(used, egui::Sense::hover());
    }
    out
}

/// Result of [`Surface::show`].
pub struct SurfaceResponse<R> {
    /// The painted container rect (content + padding).
    pub rect: Rect,
    /// Each child's returned value, in item order.
    pub inner: Vec<R>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::widgets::theme::PortableTheme;

    fn approx(a: f32, b: f32) -> bool { (a - b).abs() < 0.01 }

    /// The typed scale must reach Taffy — `Space::Sm` has to produce the same
    /// geometry as the raw token, or the constraint layer is cosmetic.
    #[test]
    fn typed_gap_matches_the_raw_token() {
        let typed = Surface::row()
            .gap(Space::Sm)
            .item(Item::grow(1.0))
            .item(Item::grow(1.0));
        let raw = Flex::row()
            .gap(crate::ui_kit::style::gap_sm())
            .item(Item::grow(1.0))
            .item(Item::grow(1.0));
        let a = typed.flex.solve(egui::vec2(200.0, 40.0));
        let b = raw.solve(egui::vec2(200.0, 40.0));
        assert_eq!(a.len(), b.len());
        assert!(approx(a[1].left(), b[1].left()), "{} vs {}", a[1].left(), b[1].left());
    }

    #[test]
    fn typed_padding_insets_children() {
        let s = Surface::row().pad(Space::Md).item(Item::grow(1.0));
        let r = s.flex.solve(egui::vec2(200.0, 40.0));
        assert!(approx(r[0].left(), crate::ui_kit::style::gap_md()));
    }

    /// `Level::None` must paint nothing — the default has to be inert so
    /// `Surface` is usable as a pure layout container.
    #[test]
    fn level_none_resolves_to_no_fill() {
        let t = PortableTheme::dark();
        assert!(Level::None.color(&t).is_none());
        assert!(Level::Panel.color(&t).is_some());
    }

    /// Radius::Style is the per-style rung — it must resolve to a real value,
    /// since panel chrome depends on it.
    #[test]
    fn style_radius_resolves() {
        assert!(Radius::Style.px() >= 0.0);
    }

    #[test]
    fn empty_surface_does_not_panic() {
        let s = Surface::row();
        assert!(s.flex.solve(egui::vec2(100.0, 20.0)).is_empty());
    }
}
