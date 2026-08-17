//! SelectableRow — a clickable row for menus and dropdowns.
//!
//! Renders as a left-aligned label with optional leading icon, a hover tint,
//! an accent background and accent text when selected, and a dimmed look
//! when disabled. Replaces ad-hoc `ui.selectable_label(...)` callsites in
//! menus so visuals (font size, padding, hover/selected/disabled states)
//! are consistent across the app.
//!
//! API:
//!   ui.add(SelectableRow::new("Triangulator", false));
//!   ui.add(SelectableRow::new("Auto Target", true).disabled(true));
//!   ui.add(SelectableRow::new("RSI", on).leading_icon(Icon::CHART_LINE));
//!
//! Behavior:
//! - Row height derives from the theme's density-aware `row_height()` token
//!   and is nudged per `Size` (previously it was hardcoded off `gap_2xl()`,
//!   a SPACING token, which ignored the active style's density).
//! - Idle: transparent background, `pal.base(Tone::Text)` label.
//! - Selected: `color_alpha(pal.base(Tone::Accent), alpha_soft())` background,
//!   `pal.base(Tone::Accent)` label.
//! - Hover: `color_alpha(pal.base(Tone::Text), alpha_faint())` background tint.
//! - Disabled: `st::color_dim(pal.base(Tone::Text))` label, hover-only sense.
//! - Optional leading icon: `pal.base(Tone::Dim)` color, `font_sm()` size,
//!   `gap_xs()` from text.
//!
//! Returns a normal `Response` so callers use `.clicked()` exactly like
//! `ui.selectable_label(...)`.

use egui::{Response, Sense, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use crate::ui_kit::cascade::El;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::Size;
use crate::ui_kit::tokens as st;
use crate::ui_kit::interaction::{apply_interaction, InteractionState, InteractionTokens};

#[must_use = "SelectableRow does nothing until `.show(ui, theme)` or `ui.add(row)` is called"]
pub struct SelectableRow<'a> {
    label: &'a str,
    selected: bool,
    disabled: bool,
    leading_icon: Option<&'a str>,
    size: Size,
}

impl<'a> SelectableRow<'a> {
    pub fn new(label: &'a str, selected: bool) -> Self {
        Self {
            label,
            selected,
            disabled: false,
            leading_icon: None,
            size: Size::Sm,
        }
    }

    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn leading_icon(mut self, icon: &'a str) -> Self { self.leading_icon = Some(icon); self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    #[track_caller]
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
        let pal = palette_ct(theme); // bind once: avoid per-read Palette copies in this hot row
        let bug_loc = std::panic::Location::caller();
        let SelectableRow { label, selected, disabled, leading_icon, size } = self;

        // Sizing. Row height comes from the per-style, density-aware
        // `row_height()` token; Size nudges it a tier up/down.
        let base_row_h = theme.row_height();
        let pad_x = st::gap_sm();
        let pad_y = st::gap_2xs();
        let icon_gap = st::gap_xs();

        let font_size = match size {
            Size::Xs => st::font_xs(),
            Size::Sm => st::font_sm(),
            Size::Md => st::font_sm(),
            Size::Lg => st::font_md(),
            Size::Xl => st::font_md(),
        };
        let row_h = match size {
            Size::Xs => (base_row_h - 6.0).max(16.0),
            Size::Sm => base_row_h,
            Size::Md => base_row_h,
            Size::Lg => base_row_h + 4.0,
            Size::Xl => base_row_h + 4.0,
        };

        // Resolve colors.
        let text_color = if disabled {
            st::color_dim(pal.base(Tone::Text))
        } else if selected {
            pal.base(Tone::Accent)
        } else {
            pal.base(Tone::Text)
        };
        let icon_color = if disabled { st::color_half(pal.base(Tone::Dim)) } else { pal.base(Tone::Dim) };

        // Measure label.
        let label_font = crate::ui_kit::style::mono_at(font_size);
        // Measured HERE and not by the tree, deliberately: the row must know
        // its own width before it can allocate a rect, and the tree can only
        // measure inside one. egui caches galleys, so the node laying the same
        // string out again at paint time costs a hash lookup, not a shaping
        // pass.
        let label_galley = ui.fonts(|f| {
            f.layout_no_wrap(label.to_string(), label_font.clone(), text_color)
        });
        let label_w = label_galley.rect.width();
        let label_h = label_galley.rect.height();

        // Optional leading icon measurement.
        let icon_font = crate::ui_kit::style::prop_at(font_size);
        let (icon_w, icon_h) = if let Some(ic) = leading_icon {
            let g = ui.fonts(|f| f.layout_no_wrap(ic.to_string(), icon_font.clone(), icon_color));
            (g.rect.width(), g.rect.height())
        } else {
            (0.0, 0.0)
        };

        // Allocate full available width so rows align in a vertical menu.
        let mut content_w = label_w;
        if leading_icon.is_some() { content_w += icon_w + icon_gap; }
        let min_w = content_w + pad_x * 2.0;
        let avail_w = ui.available_width().max(min_w);
        let h = row_h.max(label_h.max(icon_h) + pad_y * 2.0);

        let sense = if disabled { Sense::hover() } else { Sense::click() };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(avail_w, h), sense);

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter_at(rect);
        let cr = crate::ui_kit::style::r_sm_cr();

        // Background fill — M3.3: one call to the interaction table replaces the
        // selected / hover / disabled branch ladder. Selected reads as an accent
        // tint; hover as a neutral text tint; disabled suppresses hover.
        let ix = apply_interaction(
            rect,
            InteractionState::new()
                .selected(selected)
                .hovered(response.hovered())
                .disabled(disabled),
            if selected { pal.base(Tone::Accent) } else { pal.base(Tone::Text) },
            &InteractionTokens::borderless()
                .selected_alpha(st::alpha_soft())
                .hover_alpha(st::alpha_faint()),
        );
        if ix.fill != egui::Color32::TRANSPARENT {
            // REUSES `row.list` — SelectableRow is a PanelListRow sibling.
            // A separate key would let a style round one list and not the
            // other, which is the drift the recipe layer exists to prevent.
            let (row_cr, row_fill, _) = super::theme::resolve_control_chrome(
                ui.ctx(), theme, "row.list",
                cr.nw as f32, ix.fill, ix.fill, 0.0,
            );
            painter.rect_filled(rect, row_cr, row_fill);
        }

        // Cursor affordance.
        if response.hovered() && !disabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Focus ring for keyboard navigation — egui's Sense::click() already
        // fires clicked() on Enter/Space when the widget has keyboard focus.
        if !disabled {
            st::cursor::focus_ring(ui, &response, pal.base(Tone::Accent));
        }

        // Content: leading icon, then label — a DECLARED tree that PAINTS.
        //
        // Not `solve_rect` + hand-painting: the two text nodes measure and draw
        // themselves, so the widget no longer states each string's geometry
        // twice (once to size it, once to place it). The gap applies only
        // between siblings, so a row with no icon pays no seam — which the
        // `x += icon_w + icon_gap` form got right only because the increment
        // happened to live inside the `if`.
        //
        // `text_with_font` rather than a tier: this row's fonts are
        // `prop_at(font_size)`, size-dependent and pixel-locked to the menus it
        // appears in. Moving its layout into the tree must not silently re-tier
        // its type.
        //
        // Colour is declared ONCE on the row and inherits — the icon and the
        // label were passing the same resolved colour separately, which is the
        // repetition the cascade exists to remove. The icon still overrides,
        // because a disabled or accent icon is a real distinction.
        El::row()
            .pad_x(pad_x)
            .gap(icon_gap)
            .color(text_color)
            .child_if(
                leading_icon.is_some(),
                El::text_with_font(leading_icon.unwrap_or(""), icon_font.clone())
                    .color(icon_color),
            )
            .child(El::text_with_font(label, label_font.clone()).grow(1.0))
            .show_in(ui, theme, rect);

        crate::ui_kit::inspect::mark(bug_loc, "row", response.rect);
        response
    }
}

impl<'a> Widget for SelectableRow<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}


#[cfg(test)]
mod painted_geometry_tests {
    //! `SelectableRow` is the first widget whose CONTENT is painted by the
    //! element tree rather than by hand. These assert the painted result, not
    //! the solve, because the whole risk of that change is that the tree draws
    //! somewhere the hand-written code did not.
    use super::*;

    /// x positions of every text run painted by one row, left to right.
    /// The harness is shared — see `widgets::paint_probe`.
    fn painted_text_xs(icon: Option<&'static str>) -> Vec<f32> {
        let icon = std::cell::Cell::new(icon);
        crate::ui_kit::widgets::paint_probe::probe(|ui| {
            let theme = super::super::theme::PortableTheme::dark();
            let mut row = SelectableRow::new("Triangulator", false);
            if let Some(ic) = icon.get() {
                row = row.leading_icon(ic);
            }
            row.show(ui, &theme);
        })
        .into_iter()
        .map(|r| r.left)
        .collect()
    }

    /// With no icon the label starts at the row's left padding — the tree must
    /// not insert the icon gap for a child that is not there.
    #[test]
    fn a_row_without_an_icon_pays_no_icon_gap() {
        let xs = painted_text_xs(None);
        assert_eq!(xs.len(), 1, "one text node expected, got {xs:?}");
    }

    /// With an icon there are two text nodes and the label sits to its RIGHT.
    #[test]
    fn an_icon_pushes_the_label_right() {
        let with = painted_text_xs(Some("*"));
        let without = painted_text_xs(None);
        assert_eq!(with.len(), 2, "icon + label expected, got {with:?}");
        assert!(
            with[0] < with[1],
            "icon must be left of the label: {with:?}"
        );
        assert!(
            with[1] > without[0],
            "the icon must displace the label: with={:?}, without={:?}",
            with,
            without
        );
        // The icon itself starts where the label would have — same left pad.
        assert!(
            (with[0] - without[0]).abs() < 0.01,
            "the leading child must start at the same left pad either way: {:?} vs {:?}",
            with[0],
            without[0]
        );
    }
}
