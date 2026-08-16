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

use egui::{CornerRadius, FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

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

        // Layout: leading icon, then label — DECLARED, not walked.
        //
        // `solve_rect` because this is a painter-only surface: there is no
        // child `Ui` to allocate into, only a rect and a painter. The tree is
        // the whole statement of the row's geometry — the icon holds its
        // width, the gap applies only between siblings (so a row with no icon
        // gets no gap, which the `x += icon_w + icon_gap` form got right only
        // because the increment lived inside the `if`), and the label takes
        // the rest.
        let cy = rect.center().y;
        let solved = El::row()
            .pad_x(pad_x)
            .gap(icon_gap)
            .child_if(
                leading_icon.is_some(),
                El::slot("icon", egui::vec2(icon_w, 0.0)),
            )
            .child(El::slot("label", egui::Vec2::ZERO).grow(1.0))
            .solve_rect(rect);

        if let Some(ic) = leading_icon {
            painter.text(
                Pos2::new(solved.rect("icon").left(), cy),
                egui::Align2::LEFT_CENTER,
                ic,
                icon_font,
                icon_color,
            );
        }

        painter.galley(
            Pos2::new(solved.rect("label").left(), cy - label_h * 0.5),
            label_galley,
            text_color,
        );

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

