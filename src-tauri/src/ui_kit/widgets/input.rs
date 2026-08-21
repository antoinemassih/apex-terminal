//! Input — single-line (and opt-in multi-line) text input.
//!
//! Replaces ad-hoc `egui::TextEdit::singleline` setups across the app
//! with a token-aligned, themed, animated builder.
//!
//! Default mode is single-line monospace with a framed surface; the
//! builder exposes opt-in knobs (`frameless`, `multiline`, `proportional`,
//! `horizontal_align`, explicit `text_color`/`background_color`/`margin`/
//! `id`/`width`/`font_size`) for the handful of call sites that need to
//! override the canonical look (spreadsheet cells, command palette,
//! right-aligned trading inputs, etc).
//!
//! API:
//!   let mut buf = String::new();
//!   ui.add(Input::new(&mut buf).placeholder("Symbol"));
//!
//!   Input::new(&mut buf)
//!     .leading_icon(Icon::MAGNIFYING_GLASS)
//!     .clearable(true)
//!     .placeholder("Search...")
//!     .full_width()
//!     .size(Size::Md)
//!     .show(ui, theme);
//!
//!   Input::new(&mut password).password(true).show(ui, theme);
//!
//!   // Right-aligned, frameless price field (used in order_edit_dialog):
//!   Input::new(&mut price_buf)
//!     .width(110.0)
//!     .horizontal_align(egui::Align::RIGHT)
//!     .frameless(true)
//!     .show(ui, theme);

use egui::{
    Key, Margin, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use super::motion;
use super::theme::ComponentTheme;
use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::Size;
use crate::ui_kit::tokens as st;
use crate::ui_kit::text_style::TextStyle;

/// Builder for a single-line text input. See module docs for usage.
#[must_use = "Input does nothing until `.show(ui, theme)` is called"]
pub struct Input<'a> {
    value: &'a mut String,
    placeholder: Option<String>,
    leading_icon: Option<&'a str>,
    trailing_icon: Option<&'a str>,
    prefix: Option<String>,
    suffix: Option<String>,
    clearable: bool,
    password: bool,
    invalid: bool,
    warning: bool,
    disabled: bool,
    full_width: bool,
    min_width: Option<f32>,
    width: Option<f32>,
    size: Size,
    label: Option<String>,
    helper_text: Option<String>,
    char_limit: Option<usize>,
    // Extended knobs (mirroring the legacy chart/renderer/ui/inputs::TextInput
    // builder so callers can converge here).
    font_size: Option<f32>,
    horizontal_align: Option<egui::Align>,
    frameless: bool,
    proportional: bool,
    multiline: bool,
    text_color_override: Option<egui::Color32>,
    background_color_override: Option<egui::Color32>,
    margin_override: Option<Margin>,
    explicit_id: Option<egui::Id>,
}

/// Result of showing an [`Input`]. The inner [`Response`] is for the
/// outer row (so `.changed()` fires when the text changed).
pub struct InputResponse {
    pub response: Response,
    pub clear_clicked: bool,
    pub submitted: bool,
    /// `true` when the inner editor lost keyboard focus this frame.
    /// Mirrors `egui::Response::lost_focus` for the inner `TextEdit`.
    pub lost_focus: bool,
    /// `true` when the inner editor currently has keyboard focus.
    pub has_focus: bool,
    /// `egui::Id` of the internal `TextEdit`. Use this with
    /// `ui.memory_mut(|m| m.request_focus(resp.editor_id))` when a
    /// caller needs to programmatically focus the field (e.g. when
    /// the input first appears inside a popup / dialog).
    pub editor_id: egui::Id,
}

impl InputResponse {
    /// Convenience: focus the inner editor on the next frame.
    pub fn request_focus(&self, ctx: &egui::Context) {
        let id = self.editor_id;
        ctx.memory_mut(|m| m.request_focus(id));
    }
}

impl<'a> Input<'a> {
    /// Numeric-style input: right-aligned, no frame, sensible defaults for
    /// price/qty/share entry. Replaces the
    /// `Input::new(...).width(...).horizontal_align(RIGHT).frameless(true)`
    /// repeated pattern found across order_edit_dialog and trade panels
    /// (P6.6 variant gap).
    ///
    /// Caller still chains `.prefix("$")` / `.suffix(" shares")` / `.width(N)`
    /// as needed — this just sets the right-align + frameless defaults.
    pub fn number(value: &'a mut String) -> Self {
        Self::new(value)
            .horizontal_align(egui::Align::RIGHT)
            .frameless(true)
    }

    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            placeholder: None,
            leading_icon: None,
            trailing_icon: None,
            prefix: None,
            suffix: None,
            clearable: false,
            password: false,
            invalid: false,
            warning: false,
            disabled: false,
            full_width: false,
            min_width: None,
            width: None,
            size: Size::Md,
            label: None,
            helper_text: None,
            char_limit: None,
            font_size: None,
            horizontal_align: None,
            frameless: false,
            proportional: false,
            multiline: false,
            text_color_override: None,
            background_color_override: None,
            margin_override: None,
            explicit_id: None,
        }
    }

    pub fn placeholder(mut self, hint: impl Into<String>) -> Self { self.placeholder = Some(hint.into()); self }
    pub fn leading_icon(mut self, icon: &'a str) -> Self { self.leading_icon = Some(icon); self }
    pub fn trailing_icon(mut self, icon: &'a str) -> Self { self.trailing_icon = Some(icon); self }
    pub fn prefix(mut self, text: impl Into<String>) -> Self { self.prefix = Some(text.into()); self }
    pub fn suffix(mut self, text: impl Into<String>) -> Self { self.suffix = Some(text.into()); self }
    pub fn clearable(mut self, v: bool) -> Self { self.clearable = v; self }
    pub fn password(mut self, v: bool) -> Self { self.password = v; self }
    pub fn invalid(mut self, v: bool) -> Self { self.invalid = v; self }
    pub fn warning(mut self, v: bool) -> Self { self.warning = v; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    pub fn min_width(mut self, px: f32) -> Self { self.min_width = Some(px); self }
    /// Exact fixed width for the row. Overrides `min_width` / `full_width`.
    pub fn width(mut self, px: f32) -> Self { self.width = Some(px); self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn label(mut self, text: impl Into<String>) -> Self { self.label = Some(text.into()); self }
    pub fn helper_text(mut self, text: impl Into<String>) -> Self { self.helper_text = Some(text.into()); self }
    pub fn char_limit(mut self, max: usize) -> Self { self.char_limit = Some(max); self }

    // ── Extended knobs ─────────────────────────────────────────────────────
    /// Override the auto-derived font size for the inner editor.
    pub fn font_size(mut self, px: f32) -> Self { self.font_size = Some(px); self }
    /// Set the horizontal text alignment inside the inner `TextEdit`
    /// (`Align::LEFT`, `Align::Center`, `Align::RIGHT`). Used by trading
    /// inputs that want right-aligned prices / centred quantities.
    pub fn horizontal_align(mut self, a: egui::Align) -> Self { self.horizontal_align = Some(a); self }
    /// Disable the surrounding frame + bg fill (for inline cell editors
    /// and contexts that paint their own chrome around the input).
    pub fn frameless(mut self, v: bool) -> Self { self.frameless = v; self }
    /// Use proportional font instead of monospace (default).
    pub fn proportional(mut self, v: bool) -> Self { self.proportional = v; self }
    /// Enable multiline mode (uses `egui::TextEdit::multiline`).
    /// Disables most chrome (leading/trailing icons, prefix/suffix, clear
    /// button) — caller is responsible for sizing.
    pub fn multiline(mut self, v: bool) -> Self { self.multiline = v; self }
    /// Override the editor text color (instead of `theme.text()`).
    pub fn text_color(mut self, c: egui::Color32) -> Self { self.text_color_override = Some(c); self }
    /// Override the input's background fill (instead of `theme.surface_raised()`).
    pub fn background_color(mut self, c: egui::Color32) -> Self { self.background_color_override = Some(c); self }
    /// Override the editor's inner margin (frameless multiline / cell editor).
    pub fn margin(mut self, m: Margin) -> Self { self.margin_override = Some(m); self }
    /// Explicit `egui::Id` for the input row (and its derived editor id).
    pub fn id(mut self, id: egui::Id) -> Self { self.explicit_id = Some(id); self }

    #[track_caller]
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> InputResponse {
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
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> InputResponse {
        let theme = sctx.theme();
        let r = if self.frameless || self.multiline {
            paint_input_bare(ui, theme, self)
        } else {
            paint_input(ui, theme, self)
        };
        crate::ui_kit::inspect::mark(std::panic::Location::caller(), "input", r.response.rect);
        r
    }
}

fn paint_input<'a>(ui: &mut Ui, theme: &dyn ComponentTheme, input: Input<'a>) -> InputResponse {
    let Input {
        value,
        placeholder,
        leading_icon,
        trailing_icon,
        prefix,
        suffix,
        clearable,
        password,
        invalid,
        warning,
        disabled,
        full_width,
        min_width,
        width,
        size,
        label,
        helper_text,
        char_limit,
        font_size: font_size_override,
        horizontal_align,
        frameless: _,
        proportional,
        multiline: _,
        text_color_override,
        background_color_override,
        margin_override: _,
        explicit_id: _,
    } = input;

    let h = size.height();
    let pad_x = size.padding_x().max(st::gap_md());
    let font_size = font_size_override.unwrap_or_else(|| size.font_size().max(st::font_md()));
    let icon_gap = st::gap_sm();

    let mut clear_clicked = false;

    let outer = ui.vertical(|ui| {
        if let Some(lbl) = &label {
            ui.label(
                TextStyle::MonoXs.as_rich_cascading(lbl, palette_ct(theme).base(Tone::Dim)),
            );
            ui.add_space(st::gap_2xs() * 0.5);
        }

        let desired_w = if let Some(w) = width {
            w
        } else if full_width {
            ui.available_width()
        } else {
            min_width.unwrap_or(160.0)
        };
        let row_size = Vec2::new(desired_w, h);
        let (rect, response) = ui.allocate_exact_size(row_size, Sense::click());

        let id = response.id;
        let edit_id = id.with("input_edit");

        let focused = ui.memory(|m| m.has_focus(edit_id));
        let hovered = response.hovered() && !disabled;

        let hover_t = motion::ease_bool(ui.ctx(), id.with("hover"), hovered, motion::FAST);
        let focus_t = motion::ease_bool(ui.ctx(), id.with("focus"), focused, motion::FAST);

        let border_idle = palette_ct(theme).base(Tone::Border);
        let border_hover = palette_ct(theme).base(Tone::Dim);
        let border_focus = palette_ct(theme).base(Tone::Accent);

        let mut border_col = motion::lerp_color(border_idle, border_hover, hover_t);
        border_col = motion::lerp_color(border_col, border_focus, focus_t);

        if warning && !focused {
            border_col = palette_ct(theme).base(Tone::Warn);
        }
        if invalid {
            border_col = palette_ct(theme).base(Tone::Bear);
        }
        if disabled {
            // U1-4: dim the border too when disabled (the fill is dimmed at paint
            // time below) so it doesn't read as a full-strength box around dim content.
            border_col = st::color_alpha(border_col, 128);
        }

        let bg_fill = background_color_override.unwrap_or_else(|| theme.surface_raised());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let bg = if disabled { st::color_alpha(bg_fill, 128) } else { bg_fill };
            // Chrome through the recipe layer — `input` key. Defaults encode
            // today's look, so an unauthored style is byte-identical.
            let (radius, fill, stroke) = super::theme::resolve_control_chrome(
                ui.ctx(), theme, "input",
                st::radius_sm(), bg, border_col, st::stroke_std(),
            );
            painter.rect_filled(rect, radius, fill);
            painter.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
        }

        let cy = rect.center().y;

        // ── Inline chrome geometry (flexbox) ────────────────────────────────
        //
        // M4.3: this was a two-headed cursor walk — `left_x` marching right
        // past the icon/divider/prefix while `right_x` marched LEFT past the
        // trailing icon, suffix and clear button — with the text edit taking
        // whatever was left between them. Six `+=`/`-=` statements, each
        // repeating `icon_gap`, and the edit column defined only implicitly.
        //
        // In flex it is one row: `icon · prefix · edit(grows) · × · suffix ·
        // icon`, `icon_gap` gutter, `pad_x` inset. The leading icon's seam is
        // DOUBLE (`icon_gap`, hairline divider, `icon_gap`) — that is the
        // `margin_start` on the item after it, which is exactly the case
        // `Item::margin_start` exists for.
        let icon_w = font_size * 1.1;
        let prefix_w = prefix.as_ref().map(|p| {
            ui.fonts(|f| f.layout_no_wrap(p.clone(), crate::ui_kit::style::mono_at(font_size), palette_ct(theme).base(Tone::Dim)))
                .rect
                .width()
        });
        let suffix_w = suffix.as_ref().map(|s| {
            ui.fonts(|f| f.layout_no_wrap(s.clone(), crate::ui_kit::style::mono_at(font_size), palette_ct(theme).base(Tone::Dim)))
                .rect
                .width()
        });
        let show_clear = clearable && !value.is_empty() && !disabled;

        let mut f = Flex::row()
            .padding_sides(pad_x, pad_x, 0.0, 0.0)
            .gap(icon_gap)
            .align(FlexAlign::Center);
        if leading_icon.is_some() {
            f = f.item(Item::fixed(icon_w));
        }
        if let Some(w) = prefix_w {
            // The extra gutter the old code spent on the divider band.
            let mut it = Item::content(w).shrink(0.0);
            if leading_icon.is_some() {
                it = it.margin_start(icon_gap);
            }
            f = f.item(it);
        }
        {
            let mut it = Item::grow(1.0).align_self(FlexAlign::Stretch);
            if leading_icon.is_some() && prefix_w.is_none() {
                it = it.margin_start(icon_gap);
            }
            f = f.item(it);
        }
        if show_clear {
            f = f.item(Item::fixed(icon_w));
        }
        if let Some(w) = suffix_w {
            f = f.item(Item::content(w).shrink(0.0));
        }
        if trailing_icon.is_some() {
            f = f.item(Item::fixed(icon_w));
        }

        let chrome_off = rect.min.to_vec2();
        let mut chrome = f
            .solve(rect.size())
            .into_iter()
            .map(|r| r.translate(chrome_off));
        let leading_slot = leading_icon.map(|_| chrome.next().unwrap_or(rect));
        let prefix_slot = prefix_w.map(|_| chrome.next().unwrap_or(rect));
        let edit_slot = chrome.next().unwrap_or(rect);
        let clear_slot = if show_clear { chrome.next() } else { None };
        let suffix_slot = suffix_w.map(|_| chrome.next().unwrap_or(rect));
        let trailing_slot = trailing_icon.map(|_| chrome.next().unwrap_or(rect));

        let icon_color_idle = palette_ct(theme).base(Tone::Dim);
        let icon_color_focus = palette_ct(theme).base(Tone::Accent);
        let icon_color = motion::lerp_color(icon_color_idle, icon_color_focus, focus_t);
        let muted = palette_ct(theme).base(Tone::Dim);
        let text_col = text_color_override.unwrap_or_else(|| {
            if disabled { st::color_alpha(palette_ct(theme).base(Tone::Text), 128) } else { palette_ct(theme).base(Tone::Text) }
        });

        let painter = ui.painter_at(rect);

        if let (Some(ic), Some(slot)) = (leading_icon, leading_slot) {
            painter.text(
                Pos2::new(slot.left(), cy),
                egui::Align2::LEFT_CENTER,
                ic,
                crate::ui_kit::style::prop_at(icon_w),
                icon_color,
            );
            // Hairline sits one gutter after the icon — the midpoint of the
            // doubled seam declared by the `margin_start` above.
            let div_x = (slot.right() + icon_gap).round() + 0.5;
            painter.line_segment(
                [Pos2::new(div_x, rect.top() + st::gap_xs()), Pos2::new(div_x, rect.bottom() - st::gap_xs())],
                Stroke::new(st::stroke_thin(), st::color_alpha(palette_ct(theme).base(Tone::Border), st::alpha_strong())),
            );
        }

        if let (Some(p), Some(slot)) = (&prefix, prefix_slot) {
            painter.text(
                Pos2::new(slot.left(), cy),
                egui::Align2::LEFT_CENTER,
                p,
                crate::ui_kit::style::mono_at(font_size),
                muted,
            );
        }

        if let (Some(ic), Some(slot)) = (trailing_icon, trailing_slot) {
            painter.text(
                Pos2::new(slot.right(), cy),
                egui::Align2::RIGHT_CENTER,
                ic,
                crate::ui_kit::style::prop_at(icon_w),
                icon_color,
            );
        }

        if let (Some(s), Some(slot)) = (&suffix, suffix_slot) {
            painter.text(
                Pos2::new(slot.right(), cy),
                egui::Align2::RIGHT_CENTER,
                s,
                crate::ui_kit::style::mono_at(font_size),
                muted,
            );
        }

        let mut clear_rect: Option<Rect> = None;
        if let Some(slot) = clear_slot {
            let r = Rect::from_center_size(Pos2::new(slot.right() - icon_w * 0.5, cy), Vec2::splat(icon_w));
            painter.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                crate::ui_kit::icons::Icon::X,
                crate::ui_kit::style::prop_at(icon_w),
                muted,
            );
            clear_rect = Some(r);
        }

        let edit_w = edit_slot.width().max(0.0);
        let edit_rect = Rect::from_min_max(
            Pos2::new(edit_slot.left(), rect.top() + 1.0),
            Pos2::new(edit_slot.right(), rect.bottom() - 1.0),
        );

        let pre_value = value.clone();
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(edit_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.spacing_mut().item_spacing = Vec2::ZERO;
        child.spacing_mut().button_padding = Vec2::ZERO;
        // CLIP the edit to the slot the row gave it.
        //
        // `max_rect` bounds LAYOUT, not painting: the child inherits the
        // parent's clip rect, so a value longer than the field painted straight
        // over whatever came next. The flex row is not at fault — it hands the
        // clear button its own `Item::fixed(icon_w)` and shrinks the edit's
        // `Item::grow(1.0)` accordingly — the text simply ignored the slot.
        //
        // Measured at width 240 with prefix, suffix and both icons: the value
        // painted 67.6..147.8 while the clear `x` sat at 133.2..148.2, so the
        // glyphs ran 15px under the button.
        //
        // Clipping is what this codebase already does for the same problem —
        // see `paint_change_chip`, which clips "so a value too long for the
        // slot is cut at the chip edge rather than escaping it".
        child.set_clip_rect(edit_rect.intersect(ui.clip_rect()));

        let font_id = if proportional {
            crate::ui_kit::style::prop_at(font_size)
        } else {
            crate::ui_kit::style::mono_at(font_size)
        };
        let mut te = egui::TextEdit::singleline(value)
            .id(edit_id)
            .desired_width(edit_w)
            .margin(Margin::ZERO)
            .frame(false)
            .password(password)
            .text_color(text_col)
            .font(egui::FontSelection::FontId(font_id.clone()));
        if let Some(a) = horizontal_align {
            te = te.horizontal_align(a);
        }
        if disabled {
            te = te.interactive(false);
        }
        let editor_resp = child.add(te);

        if value.is_empty() && !focused {
            if let Some(ph) = &placeholder {
                let painter2 = ui.painter_at(rect);
                painter2.text(
                    Pos2::new(edit_slot.left(), cy),
                    egui::Align2::LEFT_CENTER,
                    ph,
                    font_id.clone(),
                    st::color_alpha(palette_ct(theme).base(Tone::Dim), crate::ui_kit::style::alpha_dense()),
                );
            }
        }

        if let Some(cr) = clear_rect {
            let click_resp = ui.interact(cr, id.with("clear"), Sense::click());
            if click_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if click_resp.clicked() {
                value.clear();
                clear_clicked = true;
            }
        }

        if hovered && !disabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }

        if response.clicked() && !disabled {
            ui.memory_mut(|m| m.request_focus(edit_id));
        }

        if let Some(max) = char_limit {
            if value.chars().count() > max {
                let truncated: String = value.chars().take(max).collect();
                *value = truncated;
            }
        }

        let lost_focus = editor_resp.lost_focus();
        let has_focus = editor_resp.has_focus();
        let submitted = lost_focus
            && ui.ctx().input(|i| i.key_pressed(Key::Enter));

        // Focus ring — painted outside the border so it layers behind
        // the input frame chrome, not over the text edit content.
        if focused {
            use egui::{CornerRadius, Stroke, StrokeKind};
            // P5b: read focus-ring knobs from TokenSnapshot (chart-app
            // populates them per frame) instead of the chart-only current().
            let snap = crate::ui_kit::style::frame_tokens();
            let ring_color = st::color_alpha(palette_ct(theme).base(Tone::Accent), snap.focus_ring_alpha);
            let ring_radius = CornerRadius::same((st::radius_sm() as u8).saturating_add(1));
            ui.painter().rect_stroke(
                rect.expand(2.0),
                ring_radius,
                Stroke::new(snap.focus_ring_width, ring_color),
                StrokeKind::Outside,
            );
        }

        let mut row_resp = response;
        if *value != pre_value {
            row_resp.mark_changed();
        }

        (row_resp, submitted, edit_id, lost_focus, has_focus)
    });

    let (row_resp, submitted, editor_id, lost_focus, has_focus) = outer.inner;

    if let Some(helper) = &helper_text {
        let color = if invalid {
            palette_ct(theme).base(Tone::Bear)
        } else if warning {
            palette_ct(theme).base(Tone::Warn)
        } else {
            palette_ct(theme).base(Tone::Dim)
        };
        ui.add_space(st::gap_2xs() * 0.5);
        ui.label(
            TextStyle::MonoXs.as_rich_cascading(helper, color),
        );
    }

    InputResponse {
        response: row_resp,
        clear_clicked,
        submitted,
        lost_focus,
        has_focus,
        editor_id,
    }
}

/// "Bare" path used when `frameless(true)` or `multiline(true)` — skips
/// the painted frame/border/icon chrome and just hands an `egui::TextEdit`
/// to the parent UI. Used by inline cell editors, command palette text
/// boxes, and right-aligned trading inputs that paint their own chrome.
fn paint_input_bare<'a>(ui: &mut Ui, theme: &dyn ComponentTheme, input: Input<'a>) -> InputResponse {
    let Input {
        value,
        placeholder,
        leading_icon: _,
        trailing_icon: _,
        prefix: _,
        suffix: _,
        clearable: _,
        password,
        invalid: _,
        warning: _,
        disabled,
        full_width,
        min_width,
        width,
        size,
        label,
        helper_text,
        char_limit,
        font_size: font_size_override,
        horizontal_align,
        frameless,
        proportional,
        multiline,
        text_color_override,
        background_color_override,
        margin_override,
        explicit_id,
    } = input;

    let font_size = font_size_override.unwrap_or_else(|| size.font_size().max(st::font_md()));
    let text_col = text_color_override.unwrap_or_else(|| {
        if disabled { st::color_alpha(palette_ct(theme).base(Tone::Text), 128) } else { palette_ct(theme).base(Tone::Text) }
    });
    let font_id = if proportional {
        crate::ui_kit::style::prop_at(font_size)
    } else {
        crate::ui_kit::style::mono_at(font_size)
    };

    if let Some(lbl) = &label {
        ui.label(
            TextStyle::MonoXs.as_rich_cascading(lbl, palette_ct(theme).base(Tone::Dim)),
        );
        ui.add_space(st::gap_2xs() * 0.5);
    }

    let edit_id = explicit_id.unwrap_or_else(|| ui.next_auto_id().with("input_edit"));

    let desired_w = if let Some(w) = width {
        w
    } else if full_width {
        ui.available_width()
    } else {
        min_width.unwrap_or_else(|| ui.available_width())
    };

    let pre_value = value.clone();

    let editor_resp = if frameless {
        let base = if multiline {
            egui::TextEdit::multiline(value)
        } else {
            egui::TextEdit::singleline(value)
        };
        let mut te = base
            .id(edit_id)
            .desired_width(desired_w)
            .margin(margin_override.unwrap_or(Margin::ZERO))
            .frame(false)
            .password(password)
            .text_color(text_col)
            .font(egui::FontSelection::FontId(font_id.clone()));
        if let Some(ph) = &placeholder { te = te.hint_text(ph.as_str()); }
        if let Some(a) = horizontal_align { te = te.horizontal_align(a); }
        if disabled { te = te.interactive(false); }
        ui.add(te)
    } else {
        // Frameless was off (so this branch fires only for multiline).
        let bg_fill = background_color_override.unwrap_or_else(|| theme.surface_raised());
        let bg = if disabled { st::color_alpha(bg_fill, 128) } else { bg_fill };
        let inner_margin = margin_override.unwrap_or_else(|| Margin::same(st::gap_sm() as i8));
        let frame = egui::Frame::NONE
            .fill(bg)
            .stroke(Stroke::new(st::stroke_std(), palette_ct(theme).base(Tone::Border)))
            .inner_margin(inner_margin)
            .corner_radius(crate::ui_kit::style::r_sm_cr());
        let mut out: Option<Response> = None;
        frame.show(ui, |ui| {
            let base = if multiline {
                egui::TextEdit::multiline(value)
            } else {
                egui::TextEdit::singleline(value)
            };
            let mut te = base
                .id(edit_id)
                .desired_width(desired_w)
                .margin(Margin::ZERO)
                .frame(false)
                .password(password)
                .text_color(text_col)
                .font(egui::FontSelection::FontId(font_id.clone()));
            if let Some(ph) = &placeholder { te = te.hint_text(ph.as_str()); }
            if let Some(a) = horizontal_align { te = te.horizontal_align(a); }
            if disabled { te = te.interactive(false); }
            out = Some(ui.add(te));
        });
        out.expect("multiline editor response")
    };

    if let Some(max) = char_limit {
        if value.chars().count() > max {
            let truncated: String = value.chars().take(max).collect();
            *value = truncated;
        }
    }

    if let Some(helper) = &helper_text {
        ui.add_space(st::gap_2xs() * 0.5);
        ui.label(
            TextStyle::MonoXs.as_rich_cascading(helper, palette_ct(theme).base(Tone::Dim)),
        );
    }

    let lost_focus = editor_resp.lost_focus();
    let has_focus = editor_resp.has_focus();
    let submitted = lost_focus && ui.ctx().input(|i| i.key_pressed(Key::Enter));
    let mut response = editor_resp;
    if *value != pre_value {
        response.mark_changed();
    }

    InputResponse {
        response,
        clear_clicked: false,
        submitted,
        lost_focus,
        has_focus,
        editor_id: edit_id,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    /// Smoke: verify that `paint_input` contains a focus ring paint call.
    /// This guards against accidental removal of the focus ring path.
    /// A UI-level test that programs focus and observes a paint call would
    /// require a full egui harness; source scanning is the pragmatic alternative.
    #[test]
    fn input_focus_ring_paints_when_focused() {
        let src = include_str!("input.rs");
        assert!(
            src.contains("focus_ring_alpha") && src.contains("rect.expand(2.0)"),
            "paint_input must contain a focus ring paint block (focus_ring_alpha + rect.expand)"
        );
    }

    /// Smoke: verify `InputResponse` exposes `has_focus` so callers can
    /// gate actions on whether the input is currently focused.
    #[test]
    fn input_response_exposes_has_focus() {
        let src = include_str!("input.rs");
        assert!(
            src.contains("pub has_focus: bool"),
            "InputResponse must expose `pub has_focus: bool`"
        );
    }
}

#[cfg(test)]
mod overlap_tests {
    use super::*;
    use crate::ui_kit::widgets::paint_probe;
    use crate::ui_kit::widgets::theme::PortableTheme;

    /// The densest row in the kit: leading icon, prefix, the edit field, a
    /// clear `x`, suffix, trailing icon — six segments competing for one width.
    ///
    /// `Input` is exempt from the element-tree migration (it drives a live
    /// `TextEdit` and has geometry the tree would have to reproduce exactly),
    /// so the one thing that CAN be checked cheaply is that its segments never
    /// land on each other. Every widget in this kit that placed two things in
    /// one rect without one knowing about the other turned out to collide —
    /// seven of them, closed in AT-203.
    ///
    /// Widths are constrained deliberately: an unbounded probe panel hands the
    /// row so much space that nothing can collide, which is how two earlier
    /// probes passed against broken widgets (AT-199).
    #[test]
    fn input_segments_never_overlap() {
        for width in [360.0f32, 240.0, 150.0, 96.0] {
            for (prefix, suffix, lead, trail, clearable) in [
                (None, None, None, None, false),
                (Some("$"), Some("USD"), None, None, true),
                (Some("$"), Some(" shares"), Some("\u{1F50D}"), Some("\u{2715}"), true),
                (None, Some("%"), None, None, true),
            ] {
                let mut value = "1234567890".to_string();
                let runs = paint_probe::probe(|ui| {
                    let t = PortableTheme::dark();
                    let mut i = Input::new(&mut value).width(width);
                    if let Some(p) = prefix { i = i.prefix(p); }
                    if let Some(s) = suffix { i = i.suffix(s); }
                    if let Some(l) = lead { i = i.leading_icon(l); }
                    if let Some(tr) = trail { i = i.trailing_icon(tr); }
                    i.clearable(clearable).show(ui, &t);
                });
                if runs.is_empty() {
                    continue;
                }
                paint_probe::assert_no_overlap(
                    &format!("input w={width} prefix={prefix:?} suffix={suffix:?} \
                              lead={} trail={} clear={clearable}",
                             lead.is_some(), trail.is_some()),
                    &runs,
                );
            }
        }
    }
}
