//! TextArea — multiline text input.
//!
//! Sibling to `Input`. Use for free-form notes, prompts, descriptions,
//! script editors. Auto-grows up to a max number of lines, then scrolls.
//!
//! ```ignore
//! TextArea::new(&mut buf)
//!     .placeholder("Add notes…")
//!     .min_rows(3)
//!     .max_rows(12)
//!     .full_width()
//!     .show(ui, theme);
//! ```

use egui::{CornerRadius, FontId, Margin, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::motion;
use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::Size;
use crate::ui_kit::tokens as st;
use crate::ui_kit::text_style::TextStyle;

/// Builder for a multiline text input. See module docs for usage.
#[must_use = "TextArea does nothing until `.show(ui, theme)` is called"]
pub struct TextArea<'a> {
    value: &'a mut String,
    placeholder: &'a str,
    label: Option<&'a str>,
    helper: Option<&'a str>,
    min_rows: usize,
    max_rows: usize,
    width: Option<f32>,
    full_width: bool,
    disabled: bool,
    invalid: bool,
    char_limit: Option<usize>,
    size: Size,
    monospace: bool,
}

impl<'a> TextArea<'a> {
    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            placeholder: "",
            label: None,
            helper: None,
            min_rows: 3,
            max_rows: 8,
            width: None,
            full_width: false,
            disabled: false,
            invalid: false,
            char_limit: None,
            size: Size::Md,
            monospace: false,
        }
    }

    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn label(mut self, l: &'a str) -> Self { self.label = Some(l); self }
    pub fn helper_text(mut self, h: &'a str) -> Self { self.helper = Some(h); self }
    pub fn min_rows(mut self, n: usize) -> Self { self.min_rows = n; self }
    pub fn max_rows(mut self, n: usize) -> Self { self.max_rows = n; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    pub fn invalid(mut self, i: bool) -> Self { self.invalid = i; self }
    pub fn char_limit(mut self, max: usize) -> Self { self.char_limit = Some(max); self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn monospace(mut self, m: bool) -> Self { self.monospace = m; self }

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
        paint_text_area(ui, theme, self)
    }
}

fn paint_text_area<'a>(ui: &mut Ui, theme: &dyn ComponentTheme, ta: TextArea<'a>) -> Response {
    let TextArea {
        value,
        placeholder,
        label,
        helper,
        min_rows,
        max_rows,
        width,
        full_width,
        disabled,
        invalid,
        char_limit,
        size,
        monospace,
    } = ta;

    let font_size = size.font_size();
    let pad_x = size.padding_x();
    let pad_y = st::gap_xs();
    let radius = crate::ui_kit::style::r_sm_cr();

    let outer = ui.vertical(|ui| {
        // ── Label ──
        if let Some(lbl) = label {
            ui.label(
                TextStyle::MonoXs.as_rich_cascading(lbl, palette_ct(theme).base(Tone::Dim)),
            );
            ui.add_space(st::gap_2xs() * 0.5);
        }

        // ── Width ──
        let desired_w = if full_width {
            ui.available_width()
        } else {
            width.unwrap_or(240.0)
        };

        // ── Allocate a rect large enough to hold the editor + padding ──
        // We use a child UI so the TextEdit can size itself naturally.
        let id = ui.next_auto_id();
        let edit_id = id.with("ta_edit");

        let focused = ui.memory(|m| m.has_focus(edit_id));

        // Probe actual row height so border wraps content tightly.
        // We allocate min_rows worth of text height plus vertical padding.
        let row_h = ui.fonts(|f| f.row_height(&crate::ui_kit::style::mono_at(font_size)));
        let min_h = row_h * min_rows as f32 + pad_y * 2.0;
        let max_h = row_h * max_rows as f32 + pad_y * 2.0;

        // Clamp to max — inner TextEdit handles scrolling beyond that.
        let avail_h = ui.available_height();
        let actual_h = min_h.max(min_h).min(max_h).min(if avail_h > 0.0 { avail_h } else { max_h });

        let (border_rect, border_resp) =
            ui.allocate_exact_size(Vec2::new(desired_w, actual_h), Sense::click());

        // ── Animated border color ──
        let hovered = border_resp.hovered() && !disabled;
        let hover_t = motion::ease_bool(ui.ctx(), id.with("hover"), hovered, motion::FAST);
        let focus_t = motion::ease_bool(ui.ctx(), id.with("focus"), focused, motion::FAST);

        let border_idle = palette_ct(theme).base(Tone::Border);
        let border_hover = palette_ct(theme).base(Tone::Dim);
        let border_focus = palette_ct(theme).base(Tone::Accent);

        let mut border_col = motion::lerp_color(border_idle, border_hover, hover_t);
        border_col = motion::lerp_color(border_col, border_focus, focus_t);

        if invalid {
            border_col = st::color_alpha(palette_ct(theme).base(Tone::Bear), st::alpha_active());
        }

        // ── Background + border ──
        if ui.is_rect_visible(border_rect) {
            let painter = ui.painter_at(border_rect);
            let bg_fill = palette_ct(theme).base(Tone::Surface);
            let bg = if disabled { st::color_alpha(bg_fill, 128) } else { bg_fill };
            // REUSES the `input` key — TextArea is an Input sibling with the
            // same chrome. Minting `textarea` would let the two drift apart in
            // a style, which is the opposite of what the recipe layer is for.
            let (radius, fill, stroke) = super::theme::resolve_control_chrome(
                ui.ctx(), theme, "input",
                st::radius_sm(), bg, border_col, st::stroke_std(),
            );
            painter.rect_filled(border_rect, radius, fill);
            painter.rect_stroke(border_rect, radius, stroke, StrokeKind::Inside);
        }

        // ── Char limit: enforce before rendering ──
        if let Some(max) = char_limit {
            if value.chars().count() > max {
                let truncated: String = value.chars().take(max).collect();
                *value = truncated;
            }
        }

        let text_col = if disabled {
            st::color_alpha(palette_ct(theme).base(Tone::Text), 128)
        } else {
            palette_ct(theme).base(Tone::Text)
        };

        // ── Inner child UI for the editor ──
        let inner_rect = border_rect.shrink2(egui::Vec2::new(pad_x, pad_y));
        let pre_value = value.clone();

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );
        child.spacing_mut().item_spacing = Vec2::ZERO;

        let font = if monospace {
            egui::FontSelection::FontId(crate::ui_kit::style::mono_at(font_size))
        } else {
            egui::FontSelection::FontId(crate::ui_kit::style::prop_at(font_size))
        };

        let mut te = egui::TextEdit::multiline(value)
            .id(edit_id)
            .desired_rows(min_rows)
            .desired_width(inner_rect.width())
            .margin(Margin::ZERO)
            .frame(false)
            .text_color(text_col)
            .font(font);
        if disabled {
            te = te.interactive(false);
        }

        let editor_resp = child.add(te);

        // ── Placeholder when empty and not focused ──
        if value.is_empty() && !focused {
            let painter = ui.painter_at(inner_rect);
            painter.text(
                inner_rect.min,
                egui::Align2::LEFT_TOP,
                placeholder,
                crate::ui_kit::style::prop_at(font_size),
                st::color_alpha(palette_ct(theme).base(Tone::Dim), 160),
            );
        }

        // Click outside the nested area but inside border → give focus.
        if border_resp.clicked() && !disabled {
            ui.memory_mut(|m| m.request_focus(edit_id));
        }
        if hovered && !disabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }

        let mut row_resp = border_resp;
        if *value != pre_value {
            row_resp.mark_changed();
        }

        st::cursor::focus_ring(ui, &row_resp, palette_ct(theme).base(Tone::Accent));

        // ── Char counter helper (overrides plain helper when limit set) ──
        let helper_str: Option<String> = if let Some(max) = char_limit {
            Some(format!("{} / {}", value.chars().count(), max))
        } else {
            helper.map(|h| h.to_string())
        };

        if let Some(h) = helper_str {
            let color = if invalid { palette_ct(theme).base(Tone::Bear) } else { palette_ct(theme).base(Tone::Dim) };
            ui.add_space(st::gap_2xs() * 0.5);
            ui.label(
                egui::RichText::new(h)
                    .italics()
                    .size(st::font_xs())
                    .color(color),
            );
        }

        row_resp
    });

    outer.inner
}

impl<'a> egui::Widget for TextArea<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
