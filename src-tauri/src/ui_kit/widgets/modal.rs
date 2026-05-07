//! Modal widget — re-homed from `chart::renderer::ui::chrome::modal`.
//!
//! Builder for the recurring "centered modal with title bar + close + body"
//! pattern. See the original module docs (now mirrored below) for design
//! rationale. The migration into ui_kit:
//!   * Public theme handle is `&dyn ComponentTheme` instead of `&Theme`,
//!     so the kit can be lifted out as a standalone crate.
//!   * Show/hide motion: scrim + panel ease over `motion::MED`. The
//!     widget reads its open state from a per-id memory flag and animates
//!     `ease_bool`.
//!
//! API surface (builder method names, ModalResponse) is unchanged from the
//! legacy struct so callers keep compiling via the chrome::modal re-export.

#![allow(dead_code)]

use egui::{Color32, Context, Id, Pos2, Stroke, Ui, Vec2};

use super::theme::ComponentTheme;
use super::motion;

use crate::chart_renderer::ui::components::{DialogHeaderWithClose, PaneHeaderWithClose, PopupFrame};
use crate::chart_renderer::ui::style::{self, alpha_line, color_alpha, gap_sm, r_lg_cr};

/// How the modal is anchored on screen.
#[derive(Clone, Copy)]
pub enum Anchor {
    /// `egui::Window` anchored via `fixed_pos` (or screen-center if `None`).
    Window { pos: Option<Pos2> },
    /// `egui::Area` pinned to a screen position (popups, dropdowns).
    Area { pos: Pos2 },
}

/// Header style.
#[derive(Clone, Copy)]
pub enum HeaderStyle {
    /// Compact pane-style title bar with X close (PaneHeaderWithClose).
    Pane,
    /// Full dialog title bar with X close (style::dialog_header).
    Dialog,
    /// No auto-header — caller renders its own inside the body closure.
    None,
}

/// Frame style.
pub enum FrameKind {
    /// Themed popup (`widgets::frames::PopupFrame`).
    Popup,
    /// Themed dialog window (`style::dialog_window_themed`'s frame).
    DialogWindow,
    /// Caller-supplied frame for byte-exact preservation of legacy modals.
    Custom(egui::Frame),
}

/// Result of `Modal::show`.
pub struct ModalResponse<R> {
    /// Inner closure return value. `None` if the modal didn't render this frame.
    pub inner: Option<R>,
    /// `true` if the user requested close (X clicked or click-away).
    pub closed: bool,
}

/// Custom header painter — receives the body Ui, returns true if the user
/// requested close (e.g. clicked an X). Boxed for object-safety.
type HeaderPainter<'a> = Box<dyn FnOnce(&mut Ui) -> bool + 'a>;

/// Builder for a centered modal with title bar + close + body.
#[must_use = "Modal does nothing until `.show()` is called"]
pub struct Modal<'a> {
    title: &'a str,
    id: Option<&'a str>,
    ctx: Option<&'a Context>,
    theme: Option<&'a dyn ComponentTheme>,
    size: Vec2,
    anchor: Anchor,
    header_style: HeaderStyle,
    header_color: Option<Color32>,
    frame_kind: FrameKind,
    separator: bool,
    close_on_click_outside: bool,
    draggable_header: bool,
    header_painter: Option<HeaderPainter<'a>>,
}

impl<'a> Modal<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            id: None,
            ctx: None,
            theme: None,
            size: Vec2::new(420.0, 0.0),
            anchor: Anchor::Window { pos: None },
            header_style: HeaderStyle::Pane,
            header_color: None,
            frame_kind: FrameKind::Popup,
            separator: true,
            close_on_click_outside: false,
            draggable_header: false,
            header_painter: None,
        }
    }

    pub fn id(mut self, id: &'a str) -> Self { self.id = Some(id); self }
    pub fn ctx(mut self, ctx: &'a Context) -> Self { self.ctx = Some(ctx); self }
    /// Accepts any `&T: ComponentTheme` (concrete `Theme`, etc.).
    pub fn theme<T: ComponentTheme>(mut self, t: &'a T) -> Self {
        self.theme = Some(t);
        self
    }
    pub fn size(mut self, sz: Vec2) -> Self { self.size = sz; self }
    pub fn anchor(mut self, a: Anchor) -> Self { self.anchor = a; self }
    pub fn header_style(mut self, s: HeaderStyle) -> Self { self.header_style = s; self }
    pub fn header_color(mut self, c: Color32) -> Self { self.header_color = Some(c); self }
    pub fn frame_kind(mut self, f: FrameKind) -> Self { self.frame_kind = f; self }
    pub fn frame(mut self, f: egui::Frame) -> Self { self.frame_kind = FrameKind::Custom(f); self }
    pub fn separator(mut self, on: bool) -> Self { self.separator = on; self }
    pub fn close_on_click_outside(mut self, on: bool) -> Self {
        self.close_on_click_outside = on; self
    }
    /// When true and using `Anchor::Window`, the modal is movable (the title
    /// bar / header acts as a drag handle) instead of pinned to a fixed pos.
    pub fn draggable_header(mut self, on: bool) -> Self {
        self.draggable_header = on; self
    }
    /// Optional escape hatch: caller paints a fully custom header strip. The
    /// closure runs in place of the auto-header (regardless of `header_style`)
    /// and should return `true` if the user clicked close. The default
    /// separator after the auto-header is suppressed when this is set.
    pub fn header_painter(
        mut self,
        f: impl FnOnce(&mut Ui) -> bool + 'a,
    ) -> Self {
        self.header_painter = Some(Box::new(f));
        self
    }

    /// Render. The body closure runs inside the framed region, after the
    /// (optional) header and (optional) separator.
    pub fn show<R>(self, body: impl FnOnce(&mut Ui) -> R) -> ModalResponse<R> {
        let ctx = self.ctx.expect("Modal::show requires .ctx(ctx)");
        let t   = self.theme.expect("Modal::show requires .theme(t)");
        let id  = self.id.unwrap_or(self.title);

        // Drive a presence animation keyed on this modal's id. While the
        // builder is invoked the modal is "open" (t -> 1). The fade-in
        // happens on first show. Used for scrim/panel alpha in Area mode.
        let anim_id = Id::new(("apex_modal_anim", id));
        let appear_t = motion::ease_bool(ctx, anim_id, true, motion::MED);

        let bg = t.surface();
        let border = t.border();

        let frame = match self.frame_kind {
            FrameKind::Popup => PopupFrame::new()
                .colors(bg, border)
                .ctx(ctx)
                .build(),
            FrameKind::DialogWindow => dialog_window_frame(ctx, bg, border, None),
            FrameKind::Custom(f) => f,
        };

        let header_style = self.header_style;
        let header_color = self.header_color;
        let title = self.title;
        let separator = self.separator;
        let toolbar_border = border;
        let accent = t.accent();
        let dim = t.dim();
        let header_painter = self.header_painter;
        let had_painter = header_painter.is_some();
        let draggable = self.draggable_header;

        // Inner render closure: header + separator + body. Returns
        // (closed_from_header, body_return_value).
        let render = move |ui: &mut Ui| -> (bool, R) {
            let (header_close, has_header) = if let Some(hp) = header_painter {
                (hp(ui), true)
            } else {
                let hc = match header_style {
                    HeaderStyle::Pane => {
                        let mut open = true;
                        let title_color = header_color.unwrap_or(accent);
                        let _ = PaneHeaderWithClose::new(title)
                            .title_color(title_color)
                            .show(ui, &mut open);
                        !open
                    }
                    HeaderStyle::Dialog => {
                        let d = header_color.unwrap_or(dim);
                        DialogHeaderWithClose::new(title).dim(d).show(ui)
                    }
                    HeaderStyle::None => false,
                };
                (hc, !matches!(header_style, HeaderStyle::None))
            };
            // Suppress auto-separator when caller painted a custom header —
            // they own the full visual fidelity.
            if separator && has_header && !had_painter {
                ui.add_space(gap_sm());
                style::dialog_separator(ui, 0.0, color_alpha(toolbar_border, alpha_line()));
                ui.add_space(gap_sm());
            }
            let r = body(ui);
            (header_close, r)
        };

        let mut closed = false;
        let mut inner: Option<R> = None;

        match self.anchor {
            Anchor::Window { pos } => {
                let screen = ctx.screen_rect();
                let win_pos = pos.unwrap_or_else(|| {
                    egui::pos2(
                        screen.center().x - self.size.x * 0.5,
                        (screen.center().y - self.size.y * 0.5).max(40.0),
                    )
                });
                let win = egui::Window::new(id.to_string())
                    .resizable(false)
                    .title_bar(false)
                    .frame(frame);
                let win = if draggable {
                    win.default_pos(win_pos).default_size(self.size).movable(true)
                } else {
                    win.fixed_pos(win_pos).fixed_size(self.size)
                };

                let render_cell = std::cell::Cell::new(Some(render));
                win.show(ctx, |ui| {
                    if let Some(r) = render_cell.take() {
                        let (hc, val) = r(ui);
                        if hc { closed = true; }
                        inner = Some(val);
                    }
                });
            }
            Anchor::Area { pos } => {
                let render_cell = std::cell::Cell::new(Some(render));
                let mut popup_rect = egui::Rect::NOTHING;
                let _ = egui::Area::new(Id::new(("apex_modal", id)))
                    .order(egui::Order::Foreground)
                    .fixed_pos(pos)
                    .show(ctx, |ui| {
                        // Apply fade-in alpha multiplier while appearing.
                        ui.set_opacity(appear_t);
                        let resp = frame.show(ui, |ui| {
                            if self.size.x > 0.0 { ui.set_width(self.size.x); }
                            if let Some(r) = render_cell.take() {
                                let (hc, val) = r(ui);
                                if hc { closed = true; }
                                inner = Some(val);
                            }
                        });
                        popup_rect = resp.response.rect;
                    });

                if self.close_on_click_outside && !closed {
                    if ctx.input(|i| i.pointer.any_pressed()) {
                        if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                            if !popup_rect.contains(p) { closed = true; }
                        }
                    }
                }
            }
        }

        ModalResponse { inner, closed }
    }
}

/// Build the same `egui::Frame` that `style::dialog_window_themed` uses.
/// Body mirrors that helper byte-for-byte.
fn dialog_window_frame(
    ctx: &Context,
    toolbar_bg: Color32,
    toolbar_border: Color32,
    border_color: Option<Color32>,
) -> egui::Frame {
    let border = border_color.unwrap_or(color_alpha(toolbar_border, 80));
    egui::Frame::popup(&ctx.style())
        .fill(toolbar_bg)
        .inner_margin(0.0)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(r_lg_cr())
        .shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur:   28,
            spread: 2,
            color:  Color32::from_black_alpha(80),
        })
}
