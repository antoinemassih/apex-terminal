//! Tag — colored label for categories, status, filters. Like a chip
//! but non-interactive by default. Closable variant for filter chips.
//!
//! Tones map to the 6-color palette; opacity supplies hierarchy.
//!
//! API:
//!   ui.add(Tag::new("Filled").tone(TagTone::Bull));
//!   ui.add(Tag::new("Day").tone(TagTone::Neutral).size(Size::Xs));
//!   let r = Tag::new("Tech").closable(true).show(ui, theme);
//!   if r.closed { /* remove */ }

use egui::{Color32, FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::theme::{ComponentTheme, get_ambient_recipes};
use super::tokens::Size;
use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{Sx, StyleState};
use crate::ui_kit::icons::Icon;

/// Tone palette for Tag/Badge — each tone maps to one color in the
/// project's 6-color palette (accent / bull / bear / warn / dim/text-on-surface).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TagTone {
    /// Theme dim — neutral chip on surface.
    #[default]
    Neutral,
    /// Theme accent — informational / selected.
    Accent,
    /// Theme bull — positive / filled / long.
    Bull,
    /// Theme bear — negative / rejected / short.
    Bear,
    /// Theme warn — caution / pending.
    Warn,
}

impl TagTone {
    /// Map to the unified Sx [`Tone`](crate::ui_kit::sx::Tone) vocabulary so
    /// Tag/Badge resolve through the same palette as the rest of the style
    /// system. This is the single tone vocabulary — there is no separate
    /// widget-layer color list.
    #[inline]
    pub fn to_tone(self) -> crate::ui_kit::sx::Tone {
        use crate::ui_kit::sx::Tone;
        match self {
            TagTone::Neutral => Tone::Dim,
            TagTone::Accent => Tone::Accent,
            TagTone::Bull => Tone::Bull,
            TagTone::Bear => Tone::Bear,
            TagTone::Warn => Tone::Warn,
        }
    }

    /// Resolve the base color through the unified Sx palette. Byte-identical to
    /// the previous direct `theme.dim()/accent()/…` reads (Sx `S500` == the
    /// theme base), but now routed through the one shared color authority.
    pub fn color(&self, theme: &dyn ComponentTheme) -> Color32 {
        crate::ui_kit::sx::palette_ct(theme).base(self.to_tone())
    }
}

#[must_use = "Tag does nothing until `.show(ui, theme)` or `ui.add(tag)` is called"]
pub struct Tag<'a> {
    label: String,
    tone: TagTone,
    size: Size,
    closable: bool,
    dot: bool,
    outline: bool,
    disabled: bool,
    _lt: std::marker::PhantomData<&'a ()>,
}

pub struct TagResponse {
    pub response: Response,
    pub closed: bool,
}

impl<'a> Tag<'a> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tone: TagTone::Neutral,
            size: Size::Sm,
            closable: false,
            dot: false,
            outline: false,
            disabled: false,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn tone(mut self, t: TagTone) -> Self { self.tone = t; self }
    /// Tag size: Xs/Sm only. Md/Lg are clamped to Sm — they're too chunky for chips.
    pub fn size(mut self, s: Size) -> Self {
        self.size = match s {
            Size::Xs => Size::Xs,
            _ => Size::Sm,
        };
        self
    }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn dot(mut self, v: bool) -> Self { self.dot = v; self }
    pub fn outline(mut self, v: bool) -> Self { self.outline = v; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> TagResponse {
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
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> TagResponse {
        let theme = sctx.theme();
        let dim_mul = if self.disabled { 0.5 } else { 1.0 };
        let tone_col = self.tone.color(theme).gamma_multiply(dim_mul);
        let font_size = match self.size { Size::Xs => st::font_xs() - 1.0, _ => st::font_xs() };
        let pad_x = st::gap_xs();
        let pad_y: f32 = 2.0;
        let icon_gap = st::gap_2xs();

        // Measure label.
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(self.label.clone(), crate::ui_kit::style::prop_at(font_size), tone_col)
        });
        let label_w = galley.rect.width();
        let label_h = galley.rect.height();

        let dot_size: f32 = 6.0;
        let close_size: f32 = 8.0;

        let mut content_w = label_w;
        if self.dot { content_w += dot_size + icon_gap; }
        if self.closable { content_w += icon_gap + close_size; }

        let h = (label_h + pad_y * 2.0).max(match self.size { Size::Xs => 14.0, _ => 16.0 });
        let w = content_w + pad_x * 2.0;
        let desired = Vec2::new(w, h);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

        let mut closed = false;

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            // ── Recipe adoption (tag) ────────────────────────────────────────
            // Default Sx encodes the historical pill shape + soft fill (or
            // outline). When the ambient RecipeSet is empty (the default) resolve
            // returns the default Sx unchanged — zero visual change.
            let recipes = get_ambient_recipes(ui.ctx());
            let default_fill_alpha: u8 = 32;
            let default_chip_sx = if self.outline {
                Sx::new().rounded(h * 0.5).border_color(tone_col, st::stroke_std())
            } else {
                Sx::new().rounded(h * 0.5).bg_color(st::color_alpha(tone_col, default_fill_alpha))
            };
            let chip_sx = recipes.resolve("tag", default_chip_sx, theme);
            let chip_delta = chip_sx.resolved(StyleState::Normal);

            // Extract resolved radius for the chip box (falls back to pill if unset).
            let chip_cr = egui::CornerRadius::same(
                chip_delta.radius.map(|r| r.clamp(0.0, 255.0).round() as u8)
                    .unwrap_or_else(|| { (h * 0.5).clamp(0.0, 255.0).round() as u8 })
            );

            // Paint fill or border using the resolved Sx.
            if let Some(fill) = chip_delta.fill {
                let fill_color = match fill {
                    crate::ui_kit::sx::Fill::Solid(c) => c,
                    crate::ui_kit::sx::Fill::Shade(tone, shade) => {
                        crate::ui_kit::sx::palette_ct(theme).shade(tone, shade)
                    }
                    crate::ui_kit::sx::Fill::Alpha(tone, a) => {
                        let b = crate::ui_kit::sx::palette_ct(theme).base(tone);
                        crate::ui_kit::style::color_alpha(b, a)
                    }
                };
                painter.rect_filled(rect, chip_cr, fill_color);
            }
            if let Some(border) = chip_delta.border {
                let border_color = match border.color {
                    crate::ui_kit::sx::Fill::Solid(c) => c,
                    crate::ui_kit::sx::Fill::Shade(tone, shade) => {
                        crate::ui_kit::sx::palette_ct(theme).shade(tone, shade)
                    }
                    crate::ui_kit::sx::Fill::Alpha(tone, a) => {
                        let b = crate::ui_kit::sx::palette_ct(theme).base(tone);
                        crate::ui_kit::style::color_alpha(b, a)
                    }
                };
                painter.rect_stroke(
                    rect, chip_cr,
                    egui::Stroke::new(border.width, border_color),
                    egui::StrokeKind::Inside,
                );
            }

            // M4.3: the `x += dot_size + icon_gap; … x += label_w; x += icon_gap;`
            // walk is one flex row — `dot · label · ×` on a uniform `icon_gap`
            // gutter, inset by the chip's horizontal padding.
            let mut f = Flex::row()
                .padding_sides(pad_x, pad_x, 0.0, 0.0)
                .gap(icon_gap)
                .align(FlexAlign::Center);
            if self.dot {
                f = f.item(Item::fixed(dot_size).cross(dot_size));
            }
            f = f.item(Item::content(label_w).shrink(0.0));
            if self.closable {
                f = f.item(Item::fixed(close_size).cross(close_size));
            }
            let off = rect.min.to_vec2();
            let mut slots = f.solve(rect.size()).into_iter().map(|r| r.translate(off));

            if self.dot {
                if let Some(d) = slots.next() {
                    painter.circle_filled(d.center(), dot_size * 0.5, tone_col);
                }
            }

            let label_slot = slots.next().unwrap_or(rect);
            painter.text(
                Pos2::new(label_slot.left(), label_slot.center().y),
                egui::Align2::LEFT_CENTER,
                &self.label,
                crate::ui_kit::style::prop_at(font_size),
                tone_col,
            );

            if self.closable {
                let close_slot = slots.next().unwrap_or(rect);
                let close_center = close_slot.center();
                let close_rect = egui::Rect::from_center_size(close_center, Vec2::splat(close_size + 4.0));
                let close_sense = if self.disabled { Sense::hover() } else { Sense::click() };
                let close_resp = ui.interact(close_rect, response.id.with("close"), close_sense);
                let col = if !self.disabled && close_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    st::color_alpha(tone_col, 255)
                } else {
                    st::color_alpha(tone_col, crate::ui_kit::style::alpha_solid())
                };
                painter.text(
                    close_center,
                    egui::Align2::CENTER_CENTER,
                    Icon::X,
                    crate::ui_kit::style::prop_at(close_size),
                    col,
                );
                if !self.disabled && close_resp.clicked() { closed = true; }
            }
        }

        TagResponse { response, closed }
    }
}

impl<'a> Widget for Tag<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme).response
    }
}

// ─── paint_pill — painter-level pill for absolute-rect callers ───────────────

/// Fill treatment for [`paint_pill`].
#[derive(Clone, Copy, PartialEq)]
pub enum PillStyle {
    /// Soft tinted fill — the default chip look (matches [`Tag`]).
    Soft,
    /// Fainter tinted fill — secondary / status pills.
    Subtle,
    /// Solid fill with contrast text — strong emphasis (matches [`Badge`]).
    Solid,
    /// Border only, transparent fill.
    Outline,
}

/// **Painter-level pill** — draws a labelled, fully-rounded pill into an absolute
/// `rect` with no `Ui`, for chart overlays + painter-driven cards that can't use
/// the `Ui`-based [`Tag`] / [`Badge`]. `color` is the tone; it shares the Sx box
/// renderer so it matches the chip widgets, and the label is centred in `font`.
pub fn paint_pill(
    p: &egui::Painter, rect: egui::Rect, label: &str, color: Color32,
    style: PillStyle, font: FontId, t: &dyn ComponentTheme,
) {
    let r = rect.height() * 0.5;
    match style {
        PillStyle::Soft    => Sx::new().rounded(r).bg_color(st::color_alpha(color, 32)).paint_box_at(p, rect, t),
        PillStyle::Subtle  => Sx::new().rounded(r).bg_color(st::color_alpha(color, crate::ui_kit::style::alpha_soft())).paint_box_at(p, rect, t),
        PillStyle::Solid   => Sx::new().rounded(r).bg_color(color).paint_box_at(p, rect, t),
        PillStyle::Outline => Sx::new().rounded(r).border_color(color, st::stroke_std()).paint_box_at(p, rect, t),
    }
    let text_col = if style == PillStyle::Solid { st::contrast_fg(color) } else { color };
    p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font, text_col);
}

// ─── paint_badge — notification badge: pill + left accent bar ────────────────

/// Notification badge — a pill with a 3 px left severity accent bar plus a
/// soft-tinted fill and a left-aligned label. Visually matches the hand-drawn
/// badges in `alert_feed` but reusable anywhere a painter rect is available.
///
/// The caller is responsible for any dismiss / interactive controls drawn on
/// top of `rect`; `paint_badge` only fills the visual background.
///
/// * `color`  — tone colour (accent bar + tint source); rendered at full alpha
///              for the bar and at ~7 % (`α = 18`) for the pill fill.
/// * `font`   — monospace xs recommended (matches the toolbar badge strip).
/// * `t`      — theme reference used for the label text colour (`t.text()`).
pub fn paint_badge(
    p: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    color: Color32,
    font: FontId,
    t: &dyn ComponentTheme,
) {
    const ACCENT_W: f32 = 3.0;
    const TINT_ALPHA: u8 = 18;
    const LABEL_GAP: f32 = 4.0; // gap between accent bar right-edge and text

    let r = rect.height() * 0.5;
    let cr = egui::CornerRadius::same(r as u8);

    // Soft-tinted pill background
    Sx::new().rounded(r).bg_color(st::color_alpha(color, TINT_ALPHA)).paint_box_at(p, rect, t);

    // Left accent bar — same height as the pill, clipped to the left edge.
    // Use a small corner radius equal to the pill's so the left corners match.
    let bar_rect = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(ACCENT_W, rect.height()),
    );
    p.rect_filled(bar_rect, cr, color);

    // Label — left-aligned, offset past the accent bar.
    let text_x = rect.left() + ACCENT_W + LABEL_GAP;
    p.text(
        egui::pos2(text_x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        t.text(),
    );
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::widgets::paint_probe;

    /// A closable tag is a label plus a close glyph — the same two-part shape
    /// as the `Select` trigger, which turned out to size itself from a
    /// PROPORTIONAL layout of its label and paint a MONOSPACE one, so a
    /// narrow-glyph label overran the caret by 87px and the caret was drawn on
    /// top of the text.
    ///
    /// Narrow glyphs are the case that exposes it: under a proportional
    /// measure `i` is a sliver, under a monospace paint it fills a cell. A
    /// test written with "W" passes either way, which is how that one hid.
    #[test]
    fn a_closable_tag_keeps_its_label_clear_of_the_close_glyph() {
        for label in ["iiiiiiiiiiiiiiiiiiii", "WWWWWWWWWW", "Mixed Label 123"] {
            let runs = paint_probe::probe(|ui| {
                let t = PortableTheme::dark();
                Tag::new(label).closable(true).show(ui, &t);
            });
            assert!(!runs.is_empty(), "{label:?}: the tag painted nothing");
            paint_probe::assert_no_overlap(&format!("tag {label:?}"), &runs);
        }
    }

    use crate::design_system::recipes::RecipeSet;
    use crate::ui_kit::sx::{
        recipe_spec::{ColorSpec, RadiusTier, RecipeDelta, RecipeSpec, ToneRef},
        style::{Sx, StyleState},
    };
    use crate::ui_kit::widgets::theme::{ComponentTheme, PortableTheme};

    fn mock_theme() -> PortableTheme { PortableTheme::dark() }

    /// S5 adoption proof — tag: a non-empty RecipeSet overriding the `tag` key
    /// changes the resolved box vs the default.
    #[test]
    fn s5_tag_recipe_overrides_radius_vs_default() {
        let mut set = RecipeSet::new();
        // Override: square corners (no rounding).
        set.insert("tag", RecipeSpec {
            base: RecipeDelta {
                radius: Some(RadiusTier::None),
                ..Default::default()
            },
            ..Default::default()
        });
        let t = mock_theme();
        // Default: pill (h*0.5 ≈ 8.0 for a 16px tag).
        let default_sx = Sx::new().rounded(8.0);
        let result = set.resolve("tag", default_sx, &t);
        let delta = result.resolved(StyleState::Normal);
        assert_eq!(delta.radius, Some(0.0),
            "recipe should override radius to 0 (square)");

        // Empty set: radius unchanged.
        let empty = RecipeSet::new();
        let result_empty = empty.resolve("tag", default_sx, &t);
        let delta_empty = result_empty.resolved(StyleState::Normal);
        assert_eq!(delta_empty.radius, default_sx.resolved(StyleState::Normal).radius,
            "empty RecipeSet must leave tag radius unchanged");
    }

    /// S5 adoption proof — tag fill: a non-empty RecipeSet overriding fill
    /// changes the resolved fill vs the default.
    #[test]
    fn s5_tag_recipe_overrides_fill_vs_default() {
        let mut set = RecipeSet::new();
        // Override: solid accent base fill.
        set.insert("tag", RecipeSpec {
            base: RecipeDelta {
                fill: Some(ColorSpec::Tone {
                    tone: ToneRef::Accent,
                    shade: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        });
        let t = mock_theme();
        // Default: dim at alpha 32 (soft fill).
        let dim_col = ComponentTheme::dim(&t);
        let default_sx = Sx::new().rounded(8.0)
            .bg_color(crate::ui_kit::tokens::color_alpha(dim_col, 32));
        let result = set.resolve("tag", default_sx, &t);
        let delta = result.resolved(StyleState::Normal);
        assert!(delta.fill.is_some(), "recipe should set a fill");

        // The resolved fill must differ from the default dim-at-32 fill.
        let pal = crate::ui_kit::sx::palette_ct(&t);
        let resolved_color = match delta.fill.unwrap() {
            crate::ui_kit::sx::style::Fill::Solid(c) => c,
            crate::ui_kit::sx::style::Fill::Shade(tone, shade) => pal.shade(tone, shade),
            crate::ui_kit::sx::style::Fill::Alpha(tone, a) => {
                let b = pal.base(tone);
                crate::ui_kit::style::color_alpha(b, a)
            }
        };
        let default_fill = crate::ui_kit::tokens::color_alpha(dim_col, 32);
        assert_ne!(resolved_color, default_fill,
            "recipe fill (Accent solid) should differ from default (Dim alpha 32)");

        // Empty set: fill unchanged.
        let empty = RecipeSet::new();
        let result_empty = empty.resolve("tag", default_sx, &t);
        let delta_empty = result_empty.resolved(StyleState::Normal);
        assert!(delta_empty.fill.is_some(), "empty set must preserve default fill");
        let empty_color = match delta_empty.fill.unwrap() {
            crate::ui_kit::sx::style::Fill::Solid(c) => c,
            crate::ui_kit::sx::style::Fill::Shade(tone, shade) => pal.shade(tone, shade),
            crate::ui_kit::sx::style::Fill::Alpha(tone, a) => {
                let b = pal.base(tone);
                crate::ui_kit::style::color_alpha(b, a)
            }
        };
        assert_eq!(empty_color, default_fill,
            "empty RecipeSet must leave tag fill unchanged");
    }
}
