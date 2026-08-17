//! `FloatingPaneChrome` — unified header/body/footer wrapper for floating
//! modals and panes.
//!
//! Provides a consistent look for floating UI surfaces (order ticket,
//! filter pane, indicator detail modal). Square border + hairline stroke
//! matching the chart pane's chrome, slot-based composition.
//!
//! ```ignore
//! egui::Window::new("foo").title_bar(false).frame(egui::Frame::NONE)
//!     .show(ctx, |ui| {
//!         let r = FloatingPaneChrome::new(42, "Order Ticket")
//!             .subtitle("AAPL · 1m")
//!             .width(260.0)
//!             .theme(t)
//!             .trailing(|ui| { /* extra header buttons */ })
//!             .footer(|ui| { /* sticky CTA */ })
//!             .show(ui, |ui| { /* body */ });
//!         if r.close_clicked { /* close */ }
//!     });
//! ```


use egui::{Color32, CornerRadius, Pos2, Sense, Stroke, StrokeKind, Ui, Vec2};
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use crate::ui_kit::widgets::{Button, Tooltip, tokens::Variant};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;

type Theme = crate::chart_renderer::gpu::Theme;

/// Outcome of rendering a floating pane chrome.
#[derive(Default, Clone, Copy)]
pub struct FloatingPaneChromeResponse {
    pub close_clicked: bool,
    /// Drag delta this frame (zero if not dragging).
    pub drag_delta:    Vec2,
    /// Header was double-clicked — caller should toggle expand/collapse.
    pub header_double_clicked: bool,
    /// Title text was clicked — caller can use this for "click to change symbol".
    pub title_clicked: bool,
}

/// Slot-based floating pane chrome — title bar, body, optional footer.
///
/// Use inside an `egui::Window` with `title_bar(false)` and a transparent
/// frame; this widget paints its own border + fills.
#[must_use = "FloatingPaneChrome must be shown via `.show(ui, body)`"]
pub struct FloatingPaneChrome<'a> {
    id:             u32,
    title:          &'a str,
    subtitle:       Option<&'a str>,
    leading_icon:   Option<&'a str>,
    badge:          Option<(&'a str, Color32)>,
    width:          f32,
    show_close:     bool,
    trailing:       Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    footer:         Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    theme:          Option<&'a Theme>,
}

impl<'a> FloatingPaneChrome<'a> {
    pub fn new(id: u32, title: &'a str) -> Self {
        Self {
            id,
            title,
            subtitle:     None,
            leading_icon: None,
            badge:        None,
            width:        260.0,
            show_close:   true,
            trailing:     None,
            footer:       None,
            theme:        None,
        }
    }

    pub fn subtitle(mut self, s: &'a str) -> Self { self.subtitle = Some(s); self }
    pub fn leading_icon(mut self, icon: &'a str) -> Self { self.leading_icon = Some(icon); self }
    pub fn badge(mut self, text: &'a str, color: Color32) -> Self {
        self.badge = Some((text, color));
        self
    }
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }
    pub fn show_close(mut self, v: bool) -> Self { self.show_close = v; self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    /// Custom header trailing slot — rendered right-to-left, before the close button.
    pub fn trailing<F: FnOnce(&mut Ui) + 'a>(mut self, f: F) -> Self {
        let b: Box<dyn FnOnce(&mut Ui) + 'a> = Box::new(f);
        self.trailing = Some(b);
        self
    }

    /// Sticky footer slot, rendered below the body with a hairline separator.
    pub fn footer<F: FnOnce(&mut Ui) + 'a>(mut self, f: F) -> Self {
        let b: Box<dyn FnOnce(&mut Ui) + 'a> = Box::new(f);
        self.footer = Some(b);
        self
    }

    /// Render header → body → optional footer.
    pub fn show<B: FnOnce(&mut Ui)>(self, ui: &mut Ui, body: B) -> FloatingPaneChromeResponse {
        let _theme_owned;
        let theme: &Theme = match self.theme {
            Some(t) => t,
            None => { _theme_owned = crate::chart_renderer::theme_impl::active_theme(ui.ctx()); &_theme_owned },
        };
        let bg       = theme.toolbar_bg;
        let border_c = tint(theme, Tone::Border, alpha_line());
        let header_bg = tint(theme, Tone::Border, alpha_subtle());
        let dim      = theme.dim;
        let text     = theme.text;
        let accent   = theme.accent;
        let header_h = 28.0_f32;
        let pad      = gap_sm();

        let mut close_clicked = false;
        let mut drag_delta    = Vec2::ZERO;
        let mut header_double_clicked = false;
        let mut title_clicked = false;

        // Outer frame: full-width fill + hairline border + square corners.
        let outer_rect_start = ui.cursor().min;
        let inner_resp = crate::ui_kit::widgets::OutlinedBox::new()
            .fill(bg)
            .border(border_c)
            .square()
            .padding(0.0)
            .show(ui, &crate::chart_renderer::theme_impl::active_theme(ui.ctx()), |ui| {
            ui.set_min_width(self.width);
            // Use the frame's actual content rect so the header/hairlines
            // span edge-to-edge regardless of nested margins.
            let content_left  = ui.max_rect().left();
            let content_right = ui.max_rect().right();
            let avail_w = (content_right - content_left).max(self.width);

            // ── Header ─────────────────────────────────────────────
            let header_top_y = ui.cursor().min.y;
            let header_top = Pos2::new(content_left, header_top_y);
            let header_rect = egui::Rect::from_min_size(
                header_top,
                Vec2::new(avail_w, header_h),
            );
            ui.painter().rect_filled(header_rect, CornerRadius::ZERO, header_bg);

            // Drag handle interact registered FIRST so widgets in the header
            // (title click, Buy/Sell toggle, close button) take precedence
            // for click events. Egui resolves clicks last-registered-wins, so
            // anything registered after this gets priority.
            let drag_resp = ui.interact(
                egui::Rect::from_min_size(header_top, Vec2::new(avail_w, header_h)),
                egui::Id::new(("floating_pane_drag", self.id)),
                Sense::click_and_drag(),
            );
            if drag_resp.dragged() { drag_delta = drag_resp.drag_delta(); }
            if drag_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }
            if drag_resp.double_clicked() { header_double_clicked = true; }

            ui.allocate_ui_with_layout(
                Vec2::new(avail_w, header_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(pad);

                    if let Some(icon) = self.leading_icon {
                        ui.label(egui::RichText::new(icon)
                            .size(font_xs() + 2.0)
                            .color(accent));
                        ui.add_space(gap_xs());
                    }

                    // Clickable title — bound up with subtitle so the whole
                    // "SYMBOL  $price" block is one big click target. The
                    // caret hints that it opens a picker.
                    let title_resp = ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.x = gap_xs();
                        ui.label(egui::RichText::new(self.title)
                            .monospace()
                            .size(font_xs() + 1.0)
                            .strong()
                            .color(text));
                        if let Some(sub) = self.subtitle {
                            ui.label(egui::RichText::new(sub)
                                .monospace()
                                .size(font_xs())
                                .color(color_subtle(dim)));
                        }
                        ui.label(TextStyle::Caption.as_rich_cascading(Icon::CARET_DOWN, color_half(dim)));
                    }).response.interact(egui::Sense::click());
                    if title_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if title_resp.clicked() { title_clicked = true; }

                    if let Some((bt, bc)) = self.badge {
                        ui.add_space(gap_xs());
                        ui.label(egui::RichText::new(bt)
                            .monospace()
                            .size(font_xs())
                            .strong()
                            .color(bc));
                    }

                    // Allocate the rest of the header row as its own sub-region
                    // so right_to_left actually anchors to the pane's right edge.
                    let remaining_w = ui.available_width();
                    ui.allocate_ui_with_layout(
                        Vec2::new(remaining_w, header_h),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.add_space(pad);
                            if self.show_close {
                                let cr = ui.add(Button::icon(Icon::X)
                                    .variant(Variant::InlineClose)
                                    .placement(IconPlacement::PanelHeader));
                                Tooltip::new("Close").show(ui, &cr, theme);
                                if cr.clicked() { close_clicked = true; }
                                if cr.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                            }
                            if let Some(trailing) = self.trailing {
                                ui.add_space(gap_xs());
                                trailing(ui);
                            }
                        });
                });

            // Header bottom hairline.
            let hl_y = header_top.y + header_h;
            ui.painter().line_segment(
                [Pos2::new(header_top.x, hl_y),
                 Pos2::new(header_top.x + avail_w, hl_y)],
                Stroke::new(stroke_std(), border_c),
            );

            // ── Body ───────────────────────────────────────────────
            ui.add_space(pad);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = gap_xs();
                body(ui);
            });
            ui.add_space(pad);

            // ── Footer ─────────────────────────────────────────────
            if let Some(footer) = self.footer {
                let foot_y = ui.cursor().min.y;
                ui.painter().line_segment(
                    [Pos2::new(header_top.x, foot_y),
                     Pos2::new(header_top.x + avail_w, foot_y)],
                    Stroke::new(stroke_std(), border_c),
                );
                ui.add_space(pad);
                footer(ui);
                ui.add_space(pad);
            }
        });

        // Paint outer hairline border explicitly on the rect we actually
        // occupied — egui::Frame's stroke can get clipped by the Window's
        // outer margin when the content is taller than the fixed_size hint.
        let outer = inner_resp.response.rect;
        ui.painter().rect_stroke(
            outer,
            CornerRadius::ZERO,
            Stroke::new(stroke_std(), border_c),
            StrokeKind::Inside,
        );
        let _ = outer_rect_start;

        FloatingPaneChromeResponse {
            close_clicked, drag_delta, header_double_clicked, title_clicked,
        }
    }
}
