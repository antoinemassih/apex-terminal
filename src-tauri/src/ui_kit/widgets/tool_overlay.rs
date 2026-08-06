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
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::{Size as KitSize, Variant};
use super::{Button, Tooltip};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::tokens as st;
use crate::ui_kit::text_style::TextStyle;

#[derive(Default)]
pub struct ToolOverlayResponse {
    /// `true` for the frame on which the user clicked the close button or
    /// the underlying egui::Window's X. Hosts treat this as "request close".
    pub closed: bool,
    /// Pointer drag delta on the header strip THIS frame (only populated
    /// when the overlay is `.controlled_pos()` — host-managed position).
    /// Host adds this to its stored position to follow the drag.
    pub drag_delta: egui::Vec2,
    /// Full window rect for the frame (None on the first frame before egui
    /// has laid out the window, or when the overlay is fully faded out).
    pub rect: Option<egui::Rect>,
}

/// Position-management mode for the overlay window.
#[derive(Clone, Copy)]
enum PositionMode {
    /// egui::Window owns the position. `.pos()` is the default on first show;
    /// the user can drag the window and egui persists the new position in
    /// its own memory. Best for ephemeral / one-shot tool windows.
    EguiManaged,
    /// Host owns the position. ToolOverlay places the window at `pos` every
    /// frame (fixed_pos), captures drag delta on the header strip, and
    /// returns it via `ToolOverlayResponse::drag_delta`. Host writes the
    /// new position back to its own state. Best for tools that persist
    /// position to disk / per-document state (e.g. floating order ticket
    /// stored on Chart::order_panel_pos).
    HostManaged,
}

#[must_use = "ToolOverlay does nothing until `.show(ctx, theme, body)` is called"]
pub struct ToolOverlay<'a> {
    title:           &'a str,
    id:              &'a str,
    width:           f32,
    pos:             Option<Pos2>,
    position_mode:   PositionMode,
    accent_dot:      Option<Color32>,
    closable:        bool,
    draggable:       bool,
    body_pad_x:      f32,
    body_pad_y:      f32,
    header_leading:  Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    header_trailing: Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    footer:          Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    /// Caller's open flag. When false the overlay animates OUT and stops
    /// rendering once hidden. Default true. Pass via `.open()` and call
    /// `.show()` every frame (don't gate on the flag) to get an exit animation.
    open: bool,
}

impl<'a> ToolOverlay<'a> {
    /// Required: the overlay's title (rendered in the header strip).
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            id: title, // host should override with a stable id via .id()
            width: 260.0,
            pos: None,
            position_mode: PositionMode::EguiManaged,
            accent_dot: None,
            closable: true,
            draggable: true,
            body_pad_x: 14.0,
            body_pad_y: 8.0,
            header_leading: None,
            header_trailing: None,
            footer: None,
            open: true,
        }
    }

    /// Extra widgets injected at the LEFT side of the header strip, right
    /// after the accent dot + title. Used by tools that surface a state
    /// indicator (armed icon, mode toggle) inline with the title.
    /// The body runs inside a child UI sized to the header height — fit
    /// small icon-buttons or labels here, not large content.
    pub fn header_leading(mut self, body: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.header_leading = Some(Box::new(body));
        self
    }

    /// Extra widgets injected on the RIGHT, left of the close button. For
    /// position pills, DOM toggles, expand/collapse, etc.
    pub fn header_trailing(mut self, body: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.header_trailing = Some(Box::new(body));
        self
    }

    /// Stable egui id for the underlying window. REQUIRED for any overlay
    /// where two instances could coexist (e.g. per-indicator editor).
    pub fn id(mut self, id: &'a str) -> Self { self.id = id; self }

    /// Window width. Default 260 px. Height auto-fits content.
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }

    /// Default position on first show (egui-managed mode). The user can
    /// drag and egui remembers the new position in its own memory.
    pub fn pos(mut self, p: Pos2) -> Self { self.pos = Some(p); self }

    /// Host-managed position: ToolOverlay paints the window at `p` EVERY
    /// frame (fixed_pos, no egui-persisted drag state). Drag delta is
    /// surfaced via `ToolOverlayResponse::drag_delta` so the host can
    /// update its own state. Use this when the position lives in your
    /// app's state, not in egui's memory (e.g. floating order ticket
    /// persisted to disk per Chart).
    pub fn controlled_pos(mut self, p: Pos2) -> Self {
        self.pos = Some(p);
        self.position_mode = PositionMode::HostManaged;
        self
    }

    /// Render a small color dot just left of the title text — used by
    /// indicator-editor (one dot per indicator color) and similar
    /// color-coded tools.
    pub fn accent_dot(mut self, c: Color32) -> Self { self.accent_dot = Some(c); self }

    /// Show / hide the X button in the header right corner. Default: shown.
    pub fn closable(mut self, on: bool) -> Self { self.closable = on; self }

    /// Allow / disallow window drag via header. Default: draggable.
    pub fn draggable(mut self, on: bool) -> Self { self.draggable = on; self }

    /// Drive the show/hide animation from the caller's open flag. Pass your
    /// `*_open` bool and call `.show()` EVERY frame (do NOT gate on the flag):
    /// the overlay fades in when true, fades out when false, and stops
    /// rendering once fully hidden. Default true (always shown when called).
    pub fn open(mut self, open: bool) -> Self { self.open = open; self }

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

        // Entrance fade: ease 0→1 on a FRESH open. The caller gates rendering on
        // its own open flag (so there's no "closing" frame to animate an exit),
        // but we can still give a clean fade-IN with no API change: a >100ms gap
        // since the last frame this overlay was shown means it was just opened, so
        // we snap the animation to 0 and let it ease up. During a continuous
        // session the gap is one frame (~16ms) → no re-trigger.
        let appear = {
            let now = ctx.input(|i| i.time);
            let seen_id = egui::Id::new(("tool_overlay_seen", self.id));
            let last: Option<f64> = ctx.memory(|m| m.data.get_temp(seen_id));
            let fresh = last.map_or(true, |l| now - l > 0.1);
            ctx.memory_mut(|m| m.data.insert_temp(seen_id, now));
            let appear_id = egui::Id::new(("tool_overlay_appear", self.id));
            if fresh { ctx.animate_bool_with_time(appear_id, false, 0.0); }
            // Driven by the caller's open flag: eases up when open, down when the
            // caller flips it false (the caller must keep calling show() to play
            // the fade-out). Gated callers leave open=true and rely on the
            // fresh-open snap above for their entrance.
            crate::ui_kit::widgets::motion::ease_bool(
                ctx, appear_id, self.open, crate::ui_kit::widgets::motion::FAST)
        };
        // Fully hidden — the caller set open=false and the fade-out finished.
        // Stop rendering (and don't run the body) so it fully disappears.
        if !self.open && appear < 0.01 {
            return response;
        }

        let radius = st::r_md_cr();
        let bg = palette_ct(theme).base(Tone::Surface);
        let border = palette_ct(theme).base(Tone::Border);

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
        let host_managed = matches!(self.position_mode, PositionMode::HostManaged);
        win = match self.position_mode {
            PositionMode::EguiManaged if self.draggable => {
                win.default_pos(self.pos.unwrap_or(egui::pos2(200.0, 80.0)))
                   .default_size(egui::vec2(self.width, 0.0))
                   .movable(true)
            }
            PositionMode::EguiManaged => {
                let p = self.pos.unwrap_or(egui::pos2(200.0, 80.0));
                win.fixed_pos(p).fixed_size(egui::vec2(self.width, 0.0)).movable(false)
            }
            PositionMode::HostManaged => {
                // Always paint at the host-supplied pos; drag is captured
                // manually below from header sense and reported back.
                let p = self.pos.unwrap_or(egui::pos2(200.0, 80.0));
                win.fixed_pos(p).fixed_size(egui::vec2(self.width, 0.0)).movable(false)
            }
        };

        let title       = self.title;
        let accent_dot  = self.accent_dot;
        let closable    = self.closable;
        let body_pad_x  = self.body_pad_x;
        let body_pad_y  = self.body_pad_y;
        let mut footer  = self.footer;
        let mut header_leading  = self.header_leading;
        let mut header_trailing = self.header_trailing;
        let dim         = palette_ct(theme).base(Tone::Dim);
        let text_color  = palette_ct(theme).base(Tone::Text);
        let border_col  = border;

        // Drop shadow — match dialog elevation. Painted at last frame's window
        // rect (Order::Middle, behind the window) so floating tools sit on the
        // same depth plane as Modal dialogs (ToolOverlay had no shadow before).
        let shadow_mem_id = egui::Id::new(("tool_overlay_shadow_rect", self.id));
        if let Some(r) = ctx.memory(|m| m.data.get_temp::<Rect>(shadow_mem_id)) {
            let _ = egui::Area::new(egui::Id::new(("tool_overlay_shadow", self.id)))
                .order(egui::Order::Middle)
                .fixed_pos(r.min)
                .interactable(false)
                .show(ctx, |ui| {
                    ui.set_opacity(appear);
                    super::paint_shadow_gpu(ui.painter(), r, super::ShadowSpec::md_themed(theme));
                });
        }

        let tool_resp = win.show(ctx, |ui| {
            ui.set_opacity(appear);
            ui.set_min_width(self.width);

            // ── Header strip ──────────────────────────────────────────────
            // 28 px tall, accent-border-tint fill, hairline divider under.
            const HEADER_H: f32 = 28.0;
            let header_rect = {
                let r = ui.available_rect_before_wrap();
                Rect::from_min_size(r.min, egui::vec2(r.width(), HEADER_H))
            };
            // Allocate the rect so the body lays out below. Sense depends on
            // position mode: egui-managed → hover only (movable(true) handles
            // drag), host-managed → click_and_drag so we capture the delta.
            let _hdr_resp = if host_managed {
                ui.interact(
                    header_rect,
                    egui::Id::new(("tool_overlay_hdr_drag", self.id)),
                    egui::Sense::click_and_drag(),
                )
            } else {
                ui.allocate_rect(header_rect, egui::Sense::hover())
            };
            if host_managed && _hdr_resp.dragged() {
                response.drag_delta = _hdr_resp.drag_delta();
            }
            // REUSES `section.header` for the FILL only — the asymmetric
            // radius is load-bearing here (top corners follow the panel, bottom
            // must stay square against the body). Same as ToolPopover.
            let hdr_default = color_alpha(border_col, st::alpha_tint());
            let (_, hdr_fill, _) = crate::ui_kit::widgets::theme::resolve_control_chrome(
                ui.ctx(), theme, "section.header",
                0.0, hdr_default, hdr_default, 0.0,
            );
            ui.painter().rect_filled(
                header_rect,
                CornerRadius { nw: radius.nw, ne: radius.ne, sw: 0, se: 0 },
                hdr_fill,
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
            // Measure title width so we can position the leading slot after it.
            let title_galley = ui.fonts(|f| f.layout_no_wrap(
                title.to_string(),
                TextStyle::MonoSm.font_id_in(ui),
                text_color,
            ));
            ui.painter().galley(
                egui::pos2(text_x, header_rect.center().y - title_galley.size().y / 2.0),
                title_galley.clone(),
                text_color,
            );

            // ── Optional header_leading slot (right of title text) ──────
            if let Some(lh) = header_leading.take() {
                let leading_x = text_x + title_galley.size().x + st::gap_sm();
                let leading_rect = Rect::from_min_max(
                    egui::pos2(leading_x, header_rect.top() + 2.0),
                    egui::pos2(header_rect.right() - 28.0, header_rect.bottom() - 2.0),
                );
                let mut lh_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(leading_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center))
                );
                lh(&mut lh_ui);
            }

            // Reserve close-button slot width (24 px) so trailing/close don't overlap.
            let close_slot_w = if closable { 28.0_f32 } else { 0.0 };

            // ── Optional header_trailing slot (left of close button) ────
            if let Some(th) = header_trailing.take() {
                let trailing_right = header_rect.right() - close_slot_w;
                let trailing_left  = trailing_right - 140.0; // up to 140 px reserved
                let trailing_rect = Rect::from_min_max(
                    egui::pos2(trailing_left.max(header_rect.left() + 80.0), header_rect.top() + 2.0),
                    egui::pos2(trailing_right, header_rect.bottom() - 2.0),
                );
                let mut th_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(trailing_rect)
                        .layout(egui::Layout::right_to_left(egui::Align::Center))
                );
                th(&mut th_ui);
            }

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

        // Track the live window rect so next frame's shadow follows it (incl.
        // while it's being dragged).
        if let Some(ir) = tool_resp {
            ctx.memory_mut(|m| m.data.insert_temp(shadow_mem_id, ir.response.rect));
            response.rect = Some(ir.response.rect);
        }

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
