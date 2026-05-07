//! Input — single-line text input.
//!
//! Replaces ad-hoc `egui::TextEdit::singleline` setups across the app
//! with a token-aligned, themed, animated builder.
//!
//! Multi-line text and code editing are NOT this widget's job — use
//! egui's TextEdit::multiline directly for those rare cases.
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

use egui::{
    CornerRadius, FontId, Key, Margin, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;

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
    size: Size,
    label: Option<String>,
    helper_text: Option<String>,
    char_limit: Option<usize>,
}

/// Result of showing an [`Input`]. The inner [`Response`] is for the
/// outer row (so `.changed()` fires when the text changed).
pub struct InputResponse {
    pub response: Response,
    pub clear_clicked: bool,
    pub submitted: bool,
}

impl<'a> Input<'a> {
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
            size: Size::Md,
            label: None,
            helper_text: None,
            char_limit: None,
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
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn label(mut self, text: impl Into<String>) -> Self { self.label = Some(text.into()); self }
    pub fn helper_text(mut self, text: impl Into<String>) -> Self { self.helper_text = Some(text.into()); self }
    pub fn char_limit(mut self, max: usize) -> Self { self.char_limit = Some(max); self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> InputResponse {
        paint_input(ui, theme, self)
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
        size,
        label,
        helper_text,
        char_limit,
    } = input;

    let h = size.height();
    let pad_x = size.padding_x();
    let font_size = size.font_size();
    let icon_gap = st::gap_2xs();

    let mut clear_clicked = false;

    let outer = ui.vertical(|ui| {
        // ── Label above ──
        if let Some(lbl) = &label {
            ui.label(
                egui::RichText::new(lbl)
                    .monospace()
                    .size(st::font_xs())
                    .color(theme.dim()),
            );
            ui.add_space(st::gap_2xs() * 0.5);
        }

        // ── Input row ──
        let desired_w = if full_width {
            ui.available_width()
        } else {
            min_width.unwrap_or(160.0)
        };
        let row_size = Vec2::new(desired_w, h);
        let (rect, response) = ui.allocate_exact_size(row_size, Sense::click());

        let id = response.id;
        // The TextEdit gets a stable nested id so focus tracking is reliable.
        let edit_id = id.with("input_edit");

        let focused = ui.memory(|m| m.has_focus(edit_id));
        let hovered = response.hovered() && !disabled;

        // ── Border color (animated) ──
        let hover_t = motion::ease_bool(ui.ctx(), id.with("hover"), hovered, motion::FAST);
        let focus_t = motion::ease_bool(ui.ctx(), id.with("focus"), focused, motion::FAST);

        let border_idle = theme.border();
        let border_hover = theme.dim();
        let border_focus = theme.accent();

        let mut border_col = motion::lerp_color(border_idle, border_hover, hover_t);
        border_col = motion::lerp_color(border_col, border_focus, focus_t);

        if warning && !focused {
            border_col = theme.warn();
        }
        if invalid {
            border_col = theme.bear();
        }

        let bg_fill = theme.surface();

        let radius = CornerRadius::same(4);

        // ── Paint background + border ──
        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let bg = if disabled { st::color_alpha(bg_fill, 128) } else { bg_fill };
            painter.rect_filled(rect, radius, bg);
            painter.rect_stroke(rect, radius, Stroke::new(1.0, border_col), StrokeKind::Inside);
        }

        // ── Layout content left→right and right→left to compute editor span ──
        let cy = rect.center().y;
        let mut left_x = rect.left() + pad_x;
        let mut right_x = rect.right() - pad_x;

        let icon_color_idle = theme.dim();
        let icon_color_focus = theme.accent();
        let icon_color = motion::lerp_color(icon_color_idle, icon_color_focus, focus_t);
        let muted = theme.dim();
        let text_col = if disabled { st::color_alpha(theme.text(), 128) } else { theme.text() };

        let painter = ui.painter_at(rect);

        // Leading icon
        if let Some(ic) = leading_icon {
            painter.text(
                Pos2::new(left_x, cy),
                egui::Align2::LEFT_CENTER,
                ic,
                FontId::proportional(font_size * 1.1),
                icon_color,
            );
            left_x += font_size * 1.1 + icon_gap;
        }

        // Prefix
        if let Some(p) = &prefix {
            let g = ui.fonts(|f| {
                f.layout_no_wrap(p.clone(), FontId::monospace(font_size), muted)
            });
            painter.text(
                Pos2::new(left_x, cy),
                egui::Align2::LEFT_CENTER,
                p,
                FontId::monospace(font_size),
                muted,
            );
            left_x += g.rect.width() + icon_gap;
        }

        // Trailing icon (right edge)
        if let Some(ic) = trailing_icon {
            painter.text(
                Pos2::new(right_x, cy),
                egui::Align2::RIGHT_CENTER,
                ic,
                FontId::proportional(font_size * 1.1),
                icon_color,
            );
            right_x -= font_size * 1.1 + icon_gap;
        }

        // Suffix
        if let Some(s) = &suffix {
            let g = ui.fonts(|f| {
                f.layout_no_wrap(s.clone(), FontId::monospace(font_size), muted)
            });
            painter.text(
                Pos2::new(right_x, cy),
                egui::Align2::RIGHT_CENTER,
                s,
                FontId::monospace(font_size),
                muted,
            );
            right_x -= g.rect.width() + icon_gap;
        }

        // Clear button (right-most when value non-empty + clearable)
        let mut clear_rect: Option<Rect> = None;
        if clearable && !value.is_empty() && !disabled {
            let sz = font_size * 1.1;
            let r = Rect::from_center_size(Pos2::new(right_x - sz * 0.5, cy), Vec2::splat(sz));
            painter.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "\u{2715}", // ✕
                FontId::proportional(sz),
                muted,
            );
            clear_rect = Some(r);
            right_x -= sz + icon_gap;
        }

        // ── Editor area ──
        let edit_left = left_x;
        let edit_right = right_x;
        let edit_w = (edit_right - edit_left).max(0.0);
        let edit_rect = Rect::from_min_max(
            Pos2::new(edit_left, rect.top() + 1.0),
            Pos2::new(edit_right, rect.bottom() - 1.0),
        );

        // Place the TextEdit via a child UI clipped to edit_rect.
        let pre_value = value.clone();
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(edit_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        // Zero spacing inside the inner UI.
        child.spacing_mut().item_spacing = Vec2::ZERO;
        child.spacing_mut().button_padding = Vec2::ZERO;

        let mut te = egui::TextEdit::singleline(value)
            .id(edit_id)
            .desired_width(edit_w)
            .margin(Margin::ZERO)
            .frame(false)
            .password(password)
            .text_color(text_col)
            .font(egui::FontSelection::FontId(FontId::monospace(font_size)));
        if disabled {
            te = te.interactive(false);
        }
        let editor_resp = child.add(te);

        // Placeholder paint when empty + not focused.
        if value.is_empty() && !focused {
            if let Some(ph) = &placeholder {
                let painter2 = ui.painter_at(rect);
                painter2.text(
                    Pos2::new(edit_left, cy),
                    egui::Align2::LEFT_CENTER,
                    ph,
                    FontId::monospace(font_size),
                    st::color_alpha(theme.dim(), 160),
                );
            }
        }

        // Clear-button click handling.
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

        // Cursor on hover over editor area.
        if hovered && !disabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }

        // Click anywhere in the row focuses the editor.
        if response.clicked() && !disabled {
            ui.memory_mut(|m| m.request_focus(edit_id));
        }

        // ── Char limit ──
        if let Some(max) = char_limit {
            if value.chars().count() > max {
                let truncated: String = value.chars().take(max).collect();
                *value = truncated;
            }
        }

        // ── Submit detection ──
        let submitted = editor_resp.lost_focus()
            && ui.ctx().input(|i| i.key_pressed(Key::Enter));

        // Mark the outer response changed if value changed.
        let mut row_resp = response;
        if *value != pre_value {
            row_resp.mark_changed();
        }

        (row_resp, submitted)
    });

    let (row_resp, submitted) = outer.inner;

    // ── Helper text below ──
    if let Some(helper) = &helper_text {
        let color = if invalid {
            theme.bear()
        } else if warning {
            theme.warn()
        } else {
            theme.dim()
        };
        ui.add_space(st::gap_2xs() * 0.5);
        ui.label(
            egui::RichText::new(helper)
                .monospace()
                .size(st::font_xs())
                .color(color),
        );
    }

    InputResponse {
        response: row_resp,
        clear_clicked,
        submitted,
    }
}
