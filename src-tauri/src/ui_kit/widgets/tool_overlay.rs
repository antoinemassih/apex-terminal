//! `ToolOverlay` — standardised chrome for floating tool panels.
//!
//! Replaces the hand-rolled header + body + close-button + drag-handle code
//! that every tool overlay used to reinvent (`indicator_editor`,
//! `order_entry_panel`, `overlay_manager`, `trendline_filter`, etc.). Each
//! site previously had its own ~30-line `header_painter` closure with magic
//! offsets for the title, a manually-positioned color dot, and a close
//! button that often broke because of a wrapping `ui.interact` swallowing
//! the click.
//!
//! ## What it provides
//!
//! - A draggable floating window anchored at a default position (the host
//!   tells us where).
//! - A header strip with: optional accent dot, title text, close button.
//!   The close button always works (it lives inside its own child UI, not
//!   under an interact wrapper).
//! - A body frame with consistent horizontal + vertical padding.
//! - Optional pre-wired sections via `.section(label, body)` so the
//!   caller doesn't reach for `dialog_section` / hand-rolled labels.
//! - Optional footer slot with a top divider.
//!
//! ## Quick start
//!
//! ```ignore
//! use crate::ui_kit::widgets::ToolOverlay;
//!
//! let resp = ToolOverlay::new("AAPL — RSI(14)")
//!     .id("rsi_editor_42")
//!     .width(260.0)
//!     .accent_dot(t.warn)          // optional color dot beside title
//!     .pos(egui::pos2(200.0, 80.0))
//!     .show(ctx, &t, |ui| {
//!         // body content — already inside the padded frame.
//!         ui.label("…");
//!     });
//! if resp.closed { /* host closes the overlay */ }
//! ```
//!
//! ## Slots
//!
//! `.footer(body)` adds a separated footer strip below the body (for
//! visibility-toggle + delete buttons, etc.). Footer body runs inside a
//! frame with the same horizontal padding as the main body, but slightly
//! tighter vertical to keep the action row compact.

use egui::{Color32, Context, CornerRadius, Pos2, Rect, Stroke, Ui};

use super::theme::ComponentTheme;
use super::tokens::{Size as KitSize, Variant};
use super::{Button, Tooltip};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::tokens as st;

#[derive(Default)]
pub struct ToolOverlayResponse {
    /// `true` for the frame on which the user clicked the close button or
    /// the underlying egui::Window's X. Hosts treat this as "request close".
    pub closed: bool,
}

#[must_use = "ToolOverlay does nothing until `.show(ctx, theme, body)` is called"]
pub struct ToolOverlay<'a> {
    title:       &'a str,
    id:          &'a str,
    width:       f32,
    pos:         Option<Pos2>,
    accent_dot:  Option<Color32>,
    closable:    bool,
    draggable:   bool,
    body_pad_x:  f32,
    body_pad_y:  f32,
    footer:      Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
}

impl<'a> ToolOverlay<'a> {
    /// Required: the overlay's title (rendered in the header strip).
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            id: title, // host should override with a stable id via .id()
            width: 260.0,
            pos: None,
            accent_dot: None,
            closable: true,
            draggable: true,
            body_pad_x: 14.0,
            body_pad_y: 8.0,
            footer: None,
        }
    }

    /// Stable egui id for the underlying window. REQUIRED for any overlay
    /// where two instances could coexist (e.g. per-indicator editor).
    pub fn id(mut self, id: &'a str) -> Self { self.id = id; self }

    /// Window width. Default 260 px. Height auto-fits content.
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }

    /// Default position on first show. Host may persist drag offset
    /// separately if it wants the position to stick across sessions.
    pub fn pos(mut self, p: Pos2) -> Self { self.pos = Some(p); self }

    /// Render a small color dot just left of the title text — used by
    /// indicator-editor (one dot per indicator color) and similar
    /// color-coded tools.
    pub fn accent_dot(mut self, c: Color32) -> Self { self.accent_dot = Some(c); self }

    /// Show / hide the X button in the header right corner. Default: shown.
    pub fn closable(mut self, on: bool) -> Self { self.closable = on; self }

    /// Allow / disallow window drag via header. Default: draggable.
    pub fn draggable(mut self, on: bool) -> Self { self.draggable = on; self }

    /// Override body padding. Defaults: 14 horizontal, 8 vertical.
    pub fn body_padding(mut self, x: f32, y: f32) -> Self {
        self.body_pad_x = x; self.body_pad_y = y; self
    }

    /// Attach a footer slot. Renders below the body with a hairline
    /// divider above. Use for visibility + delete action rows.
    pub fn footer(mut self, body: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.footer = Some(Box::new(body));
        self
    }

    /// Render the overlay. Returns whether the close button was clicked
    /// this frame.
    pub fn show<T: ComponentTheme>(
        self,
        ctx: &Context,
        theme: &T,
        body: impl FnOnce(&mut Ui),
    ) -> ToolOverlayResponse {
        let mut response = ToolOverlayResponse::default();
        let radius = st::r_md_cr();
        let bg = theme.surface();
        let border = theme.border();

        // Build the underlying egui::Window. Movable when draggable, fixed
        // otherwise. title_bar(false) — we paint our own header strip.
        let frame = egui::Frame::NONE
            .fill(bg)
            .stroke(Stroke::new(st::stroke_std(), border))
            .corner_radius(radius)
            .inner_margin(egui::Margin::ZERO);

        let mut win = egui::Window::new(self.id)
            .id(egui::Id::new(self.id))
            .title_bar(false)
            .resizable(false)
            .frame(frame);
        win = if self.draggable {
            win.default_pos(self.pos.unwrap_or(egui::pos2(200.0, 80.0)))
               .default_size(egui::vec2(self.width, 0.0))
               .movable(true)
        } else {
            let p = self.pos.unwrap_or(egui::pos2(200.0, 80.0));
            win.fixed_pos(p).fixed_size(egui::vec2(self.width, 0.0)).movable(false)
        };

        let title       = self.title;
        let accent_dot  = self.accent_dot;
        let closable    = self.closable;
        let body_pad_x  = self.body_pad_x;
        let body_pad_y  = self.body_pad_y;
        let mut footer  = self.footer;
        let dim         = theme.dim();
        let text_color  = theme.text();
        let border_col  = border;

        win.show(ctx, |ui| {
            ui.set_min_width(self.width);

            // ── Header strip ──────────────────────────────────────────────
            // 28 px tall, accent-border-tint fill, hairline divider under.
            const HEADER_H: f32 = 28.0;
            let header_rect = {
                let r = ui.available_rect_before_wrap();
                Rect::from_min_size(r.min, egui::vec2(r.width(), HEADER_H))
            };
            // Allocate the rect so the body lays out below. Header is non-
            // interactable at the frame level — egui::Window's movable(true)
            // gives us the drag; the close button gets its own child UI so
            // its click doesn't fight any wrapping sense.
            let _hdr_resp = ui.allocate_rect(header_rect, egui::Sense::hover());
            ui.painter().rect_filled(
                header_rect,
                CornerRadius { nw: radius.nw, ne: radius.ne, sw: 0, se: 0 },
                color_alpha(border_col, st::alpha_tint()),
            );
            ui.painter().hline(
                header_rect.x_range(),
                header_rect.bottom(),
                Stroke::new(st::stroke_thin(), color_alpha(border_col, st::alpha_strong())),
            );

            // Drag-hover cursor on the header (excluding the close-button area).
            if _hdr_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }

            // ── Optional accent dot at fixed offset ──────────────────────
            let mut text_x = header_rect.left() + st::gap_md();
            if let Some(dot) = accent_dot {
                let dot_center = egui::pos2(text_x + 4.0, header_rect.center().y);
                ui.painter().circle_filled(dot_center, 4.5, dot);
                text_x += 16.0;
            }
            ui.painter().text(
                egui::pos2(text_x, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                title,
                egui::FontId::monospace(st::font_sm()),
                text_color,
            );

            // ── Close button (own child UI — click never eaten by parent) ─
            if closable {
                let close_size = 22.0_f32;
                let close_rect = Rect::from_min_size(
                    egui::pos2(
                        header_rect.right() - close_size - st::gap_xs(),
                        header_rect.center().y - close_size / 2.0,
                    ),
                    egui::vec2(close_size, close_size),
                );
                let mut close_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(close_rect)
                        .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown))
                );
                let close_resp = Button::icon(Icon::X)
                    .variant(Variant::Ghost)
                    .size(KitSize::Sm)
                    .show(&mut close_ui, theme);
                Tooltip::new("Close").show(ui, &close_resp, theme);
                if close_resp.clicked() { response.closed = true; }
            }

            // ── Body frame ───────────────────────────────────────────────
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left:   body_pad_x as i8,
                    right:  body_pad_x as i8,
                    top:    body_pad_y as i8,
                    bottom: body_pad_y as i8,
                })
                .show(ui, |ui| {
                    body(ui);
                });

            // ── Optional footer ──────────────────────────────────────────
            if let Some(f) = footer.take() {
                // Hairline divider above the footer.
                let avail = ui.available_rect_before_wrap();
                ui.painter().hline(
                    avail.x_range(),
                    avail.top(),
                    Stroke::new(st::stroke_thin(), color_alpha(border_col, st::alpha_muted())),
                );
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left:   body_pad_x as i8,
                        right:  body_pad_x as i8,
                        top:    st::gap_xs() as i8,
                        bottom: st::gap_xs() as i8,
                    })
                    .show(ui, f);
            }

            let _ = dim;
        });

        response
    }
}

#[inline]
fn color_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::widgets::theme::PortableTheme;

    #[test]
    fn builder_defaults() {
        let t = ToolOverlay::new("Test");
        assert_eq!(t.width, 260.0);
        assert!(t.closable);
        assert!(t.draggable);
        assert!(t.accent_dot.is_none());
        assert!(t.footer.is_none());
    }

    #[test]
    fn builder_chain() {
        let t = ToolOverlay::new("Test")
            .id("the_id")
            .width(300.0)
            .accent_dot(Color32::RED)
            .closable(false)
            .draggable(false);
        assert_eq!(t.id, "the_id");
        assert_eq!(t.width, 300.0);
        assert_eq!(t.accent_dot, Some(Color32::RED));
        assert!(!t.closable);
        assert!(!t.draggable);
    }

    #[test]
    fn show_returns_default_response() {
        // Smoke test: construct + render via a fake ctx returns a non-closed
        // response on the first frame (close button not clicked).
        let ctx = egui::Context::default();
        let theme = PortableTheme::default();
        let mut response = ToolOverlayResponse::default();
        let _ = ctx.run(Default::default(), |ctx| {
            response = ToolOverlay::new("Test")
                .id("test_overlay")
                .show(ctx, &theme, |ui| {
                    ui.label("hello");
                });
        });
        assert!(!response.closed);
    }
}
