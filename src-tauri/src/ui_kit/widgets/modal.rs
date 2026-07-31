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
//!
//! ### Signature note
//! `Modal::show(body)` does NOT take `ui: &mut Ui` or `theme: &dyn ComponentTheme`
//! as parameters. A modal is a top-level floating Area on the egui Context
//! (not a child of any parent `Ui`), so it takes a `Context` via `.ctx(ctx)`
//! and a theme via `.theme(t)` as builder methods. The `.show(body)` call
//! then runs the body inside the modal's own internal Ui. This is an explicit,
//! documented variance from the "Builder + show(ui, theme)" rule in `CLAUDE.md`.

#![allow(dead_code)]

use egui::{Color32, Context, Id, Pos2, Rect, Stroke, Ui, Vec2};

use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::motion;

use crate::ui_kit::widgets::DialogHeader;
use crate::ui_kit::widgets::frames::{
    PaneHeaderWithClose, PanelHeaderWithClose, PopupFrame,
};
use crate::ui_kit::widgets::frames::SectionLabelSize;
use crate::ui_kit::tokens::{self, alpha_line, color_alpha, gap_sm, r_lg_cr};

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
    /// Panel-style header (PanelHeaderWithClose) — title via SectionLabel,
    /// optional subtitle, close button, and optional leading/trailing actions.
    /// The title is taken from `Modal::new(title)`. Use `.subtitle(s)`,
    /// `.panel_title_actions(f)`, `.panel_actions(f)` to configure further.
    Panel {
        /// Title `SectionLabel` size variant (default: `Sm`).
        title_size:      SectionLabelSize,
        /// Escape-hatch pixel size that overrides the variant.
        title_size_px:   Option<f32>,
        /// Force monospace (or proportional) title rendering.
        title_monospace: Option<bool>,
        /// Pixels of horizontal space inserted before the title.
        leading_space:   f32,
        /// Pixels of horizontal space inserted after the close button.
        trailing_space:  f32,
    },
    /// No auto-header — caller renders its own inside the body closure.
    None,
}

impl HeaderStyle {
    /// Convenience constructor for a default Panel header. Mirrors
    /// `PanelHeaderWithClose::new(title)` defaults — `Sm` SectionLabel, no
    /// pixel override, default monospace, no edge padding.
    pub const fn panel() -> Self {
        Self::Panel {
            title_size:      SectionLabelSize::Sm,
            title_size_px:   None,
            title_monospace: None,
            leading_space:   0.0,
            trailing_space:  0.0,
        }
    }
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

/// Trailing/leading action painter for `HeaderStyle::Panel` — runs inside the
/// header strip alongside the auto-title and close button.
type PanelActionFn<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

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
    /// Whether to paint a full-viewport scrim behind the modal. Defaults to
    /// `true` for `Anchor::Window` modals. The scrim fades with `appear_t`
    /// and uses a theme-aware semi-transparent color (`~50 % bg overlay`).
    scrim: bool,
    // ── HeaderStyle::Panel side-channel state ─────────────────────────────
    /// Optional subtitle rendered next to the title (monospace, dim).
    header_subtitle: Option<&'a str>,
    /// Override accent (title color) for the panel header.
    panel_accent: Option<Color32>,
    /// Override dim color (subtitle / close button) for the panel header.
    panel_dim: Option<Color32>,
    /// Inline actions placed immediately to the RIGHT of the title (LTR).
    panel_title_actions: Option<PanelActionFn<'a>>,
    /// Trailing actions placed to the LEFT of the close button (RTL).
    panel_actions: Option<PanelActionFn<'a>>,
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
            scrim: true, // Window-anchored modals show a scrim by default.
            header_subtitle: None,
            panel_accent: None,
            panel_dim: None,
            panel_title_actions: None,
            panel_actions: None,
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
    /// Control the scrim (background dim overlay) for `Anchor::Window` modals.
    /// Default is `true`. Pass `false` to suppress the scrim for floating
    /// palettes or non-blocking overlays that shouldn't dim the background.
    pub fn scrim(mut self, on: bool) -> Self { self.scrim = on; self }
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

    /// Subtitle for `HeaderStyle::Panel` — rendered next to the title in
    /// monospace `font_sm` using the dim color.
    pub fn subtitle(mut self, s: &'a str) -> Self {
        self.header_subtitle = Some(s);
        self
    }
    /// Override the accent (title) color for `HeaderStyle::Panel`. Defaults
    /// to `palette_ct(theme).base(Tone::Accent)`.
    pub fn panel_accent(mut self, c: Color32) -> Self {
        self.panel_accent = Some(c);
        self
    }
    /// Override the dim (subtitle / close-button) color for `HeaderStyle::Panel`.
    /// Defaults to `palette_ct(theme).base(Tone::Dim)`.
    pub fn panel_dim(mut self, c: Color32) -> Self {
        self.panel_dim = Some(c);
        self
    }
    /// Trailing actions placed to the LEFT of the close button (RTL flow).
    /// Mirrors `PanelHeaderWithClose::show_with`. Used for filter chips,
    /// toolbar buttons, etc.
    pub fn panel_actions(mut self, f: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.panel_actions = Some(Box::new(f));
        self
    }
    /// Inline actions placed immediately to the RIGHT of the title (LTR flow).
    /// Mirrors `PanelHeaderWithClose::show_with_title_actions`. Used for
    /// next-to-title chips, badges, etc.
    pub fn panel_title_actions(mut self, f: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.panel_title_actions = Some(Box::new(f));
        self
    }

    /// Render. The body closure runs inside the framed region, after the
    /// (optional) header and (optional) separator.
    pub fn show<R>(self, body: impl FnOnce(&mut Ui) -> R) -> ModalResponse<R> {
        let ctx = self.ctx.expect("Modal::show requires .ctx(ctx)");
        let t   = self.theme.expect("Modal::show requires .theme(t)");
        let id  = self.id.unwrap_or(self.title);

        // ── Exit-animation state ─────────────────────────────────────────
        // `closing_id` stores a bool in egui temp memory: true once the
        // header X has been clicked. While closing, we keep rendering the
        // modal for one MED cycle so the slide-down + fade-out plays before
        // we report `closed = true` to the caller.
        let closing_id    = Id::new(("apex_modal_closing", id));
        let closing_val_id = Id::new(("apex_modal_closing_t", id));

        // Read persistent closing flag from temp memory (survives across frames).
        let already_closing: bool = ctx.memory(|m| {
            m.data.get_temp(closing_id).unwrap_or(false)
        });
        // Animate closing_t: 0.0 at rest, eases to 1.0 over MED once closing.
        let closing_t = motion::ease_value(ctx, closing_val_id,
            if already_closing { 1.0 } else { 0.0 },
            motion::MED,
        );

        // Drive a presence animation keyed on this modal's id. While the
        // builder is invoked the modal is "open" (t -> 1). The fade-in
        // happens on first show. Used for scrim/panel alpha in Area mode.
        // During exit, appear_t is overridden to 1.0 - closing_t so the
        // modal fades/slides out.
        let anim_id = Id::new(("apex_modal_anim", id));
        let appear_t = if already_closing {
            // Exit path: override the open animation with the inverse of
            // closing_t so opacity and Y-offset reverse cleanly.
            (1.0 - closing_t).max(0.0)
        } else {
            motion::ease_bool(ctx, anim_id, true, motion::MED)
        };

        // elevation_3: modal is the deepest overlay layer (bg × 0.85).
        // Inlined from style::elevation_3 — ComponentTheme exposes bg() so we
        // don't need the concrete &Theme to replicate the gamma-multiply.
        let bg = palette_ct(t).base(Tone::Bg).gamma_multiply(0.85);
        let border = palette_ct(t).base(Tone::Border);

        let frame = match self.frame_kind {
            FrameKind::Popup => PopupFrame::new()
                .colors(bg, border)
                .ctx(ctx)
                .build(),
            FrameKind::DialogWindow => dialog_window_frame(ctx, bg, border, None, t.shadow_color()),
            FrameKind::Custom(f) => f,
        };

        let header_style = self.header_style;
        let header_color = self.header_color;
        let title = self.title;
        let separator = self.separator;
        let toolbar_border = border;
        let accent = palette_ct(t).base(Tone::Accent);
        let dim = palette_ct(t).base(Tone::Dim);
        let header_painter = self.header_painter;
        let had_painter = header_painter.is_some();
        // DialogWindow-framed modals are draggable by default so every dialog
        // (settings, connections, news, spread, diagnostics, replay, order
        // health, welcome, …) can be moved by grabbing it — one consistent,
        // movable shell. Popup/Custom frames keep the explicit opt-in.
        let draggable = self.draggable_header || matches!(self.frame_kind, FrameKind::DialogWindow);
        let header_subtitle = self.header_subtitle;
        let panel_accent = self.panel_accent;
        let panel_dim = self.panel_dim;
        let panel_title_actions = self.panel_title_actions;
        let panel_actions = self.panel_actions;

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
                        // P6 inversion: was the legacy
                        // `chart_renderer::…::DialogHeaderWithClose`, which
                        // re-resolved the chart-app's ambient Theme internally.
                        // The ui_kit builder takes the modal's own
                        // `&dyn ComponentTheme` — same palette, no chart import.
                        DialogHeader::new(title).dim(d).show_with(ui, t)
                    }
                    HeaderStyle::Panel {
                        title_size,
                        title_size_px,
                        title_monospace,
                        leading_space,
                        trailing_space,
                    } => {
                        let acc = panel_accent.or(header_color).unwrap_or(accent);
                        let d   = panel_dim.unwrap_or(dim);
                        let mut hb = PanelHeaderWithClose::new(title)
                            .accent(acc)
                            .dim(d)
                            .title_size(title_size)
                            .leading_space(leading_space)
                            .trailing_space(trailing_space);
                        if let Some(s)  = header_subtitle { hb = hb.subtitle(s); }
                        if let Some(px) = title_size_px   { hb = hb.title_size_px(px); }
                        if let Some(m)  = title_monospace { hb = hb.title_monospace(m); }
                        // Dispatch to the most specific show_* method based
                        // on which action closures were supplied.
                        match (panel_title_actions, panel_actions) {
                            (Some(ta), Some(ba)) => hb.show_full(ui, ta, ba),
                            (Some(ta), None)     => hb.show_with_title_actions(ui, ta),
                            (None,     Some(ba)) => hb.show_with(ui, ba),
                            (None,     None)     => hb.show(ui),
                        }
                    }
                    HeaderStyle::None => false,
                };
                (hc, !matches!(header_style, HeaderStyle::None))
            };
            // Suppress auto-separator when caller painted a custom header —
            // they own the full visual fidelity.
            if separator && has_header && !had_painter {
                ui.add_space(gap_sm());
                tokens::dialog_separator(ui, 0.0, color_alpha(toolbar_border, alpha_line()));
                ui.add_space(gap_sm());
            }
            let r = body(ui);
            (header_close, r)
        };

        let mut closed = false;
        let mut inner: Option<R> = None;
        let mut modal_rect = egui::Rect::NOTHING; // for the bug-report anchor

        // If closing_t has fully settled at 1.0, the exit animation is done —
        // report closed and clear the temp state so the next show starts fresh.
        if already_closing && closing_t >= 0.999 {
            ctx.memory_mut(|m| {
                m.data.remove::<bool>(closing_id);
                // The closing_val_id animate_value_with_time entry lives in egui's
                // own animation map and will decay naturally; we just clean our flag.
            });
            return ModalResponse { inner: None, closed: true };
        }

        match self.anchor {
            Anchor::Window { pos } => {
                let screen = ctx.screen_rect();
                let win_pos_base = pos.unwrap_or_else(|| {
                    egui::pos2(
                        screen.center().x - self.size.x * 0.5,
                        (screen.center().y - self.size.y * 0.5).max(40.0),
                    )
                });
                // Slide-in: eases from 8px below the final position while
                // appear_t goes 0 → 1. No vertical offset once fully open.
                let win_pos = egui::pos2(
                    win_pos_base.x,
                    win_pos_base.y + (1.0 - appear_t) * 8.0,
                );
                // ── Scrim ────────────────────────────────────────────────
                // Paint a full-viewport semi-transparent overlay behind the
                // modal. Order::Background puts it below all other egui
                // layers (panels, windows, tooltips) but above the app's
                // native content. Alpha tracks appear_t so it fades with the
                // modal. Clicking the scrim does NOT dismiss the modal —
                // caller controls dismiss via ModalResponse::closed.
                if self.scrim {
                    let scrim_id = Id::new(("apex_modal_scrim", id));
                    let viewport  = ctx.screen_rect();
                    // Theme-aware scrim — base color inherits from palette_ct(t).base(Tone::Bg) (dark
                    // for dark themes, light for light themes). Alpha sourced
                    // from the `alpha_scrim` token (140/255 ≈ 55%) — between
                    // heavy (120) and solid (200). Tokenized so a single
                    // design-mode slider tunes scrim density app-wide.
                    let scrim_base = color_alpha(palette_ct(t).base(Tone::Bg), crate::ui_kit::style::alpha_scrim());
                    let scrim_alpha = (scrim_base.a() as f32 * appear_t) as u8;
                    let scrim_color = Color32::from_rgba_unmultiplied(
                        scrim_base.r(), scrim_base.g(), scrim_base.b(), scrim_alpha,
                    );
                    let _ = egui::Area::new(scrim_id)
                        .order(egui::Order::Background)
                        .fixed_pos(viewport.min)
                        .interactable(false)
                        .show(ctx, |ui| {
                            ui.painter().rect_filled(viewport, 0.0, scrim_color);
                        });
                }

                // Paint a soft drop shadow behind fixed-position windows.
                // We can't easily track the rect of a movable window across
                // frames, so the shadow only goes on pinned modals.
                //
                // Use the md-tier (radius 16px) rather than lg (24px) here.
                // The modal frame already carries an egui::epaint::Shadow from
                // PopupFrame/themed_popup_frame, so this GPU layer is additive.
                // lg (radius 24) routes to the GPU silhouette path and painted
                // a ~48-pt halo on each side — visibly too wide. md (radius 16)
                // stays in the fast CPU stacked-rect path, is visually
                // indistinguishable from the prior CPU-only behaviour, and does
                // not double up on the GPU compositor.
                // Drop shadow. For a fixed modal the rect is known; for a
                // DRAGGABLE one we paint at the window's tracked last-frame rect
                // so the shadow follows it as it moves (previously the shadow was
                // disabled entirely while draggable).
                let win_rect_id = Id::new(("apex_modal_win_rect", id));
                let shadow_rect = if draggable {
                    ctx.memory(|m| m.data.get_temp::<Rect>(win_rect_id))
                } else if self.size.x > 0.0 && self.size.y > 0.0 {
                    Some(Rect::from_min_size(win_pos, self.size))
                } else {
                    None
                };
                if let Some(sr) = shadow_rect {
                    let shadow_id = Id::new(("apex_modal_shadow", id));
                    let _ = egui::Area::new(shadow_id)
                        .order(egui::Order::Middle)
                        .fixed_pos(sr.min)
                        .interactable(false)
                        .show(ctx, |ui| {
                            ui.set_opacity(appear_t);
                            super::paint_shadow_gpu(
                                ui.painter(),
                                sr,
                                super::ShadowSpec::md_themed(t),
                            );
                        });
                }

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
                let win_resp = win.show(ctx, |ui| {
                    // Apply fade alpha so the Window content fades in/out.
                    ui.set_opacity(appear_t);
                    if let Some(r) = render_cell.take() {
                        let (hc, val) = r(ui);
                        if hc && !already_closing {
                            // Header X clicked — start exit animation.
                            // Don't report closed immediately; wait for
                            // closing_t to reach 1.0 (handled above).
                            ctx.memory_mut(|m| {
                                m.data.insert_temp(closing_id, true);
                            });
                        }
                        inner = Some(val);
                    }
                });
                // Track the live window rect so next frame's shadow follows a
                // dragged window.
                if let Some(ir) = win_resp {
                    modal_rect = ir.response.rect;
                    if draggable {
                        ctx.memory_mut(|m| m.data.insert_temp(win_rect_id, ir.response.rect));
                    }
                }
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
                        // Drop shadow behind the panel — uses prior-frame rect.
                        let prior_rect: Option<Rect> = ctx.memory(|m| {
                            m.data.get_temp(Id::new(("apex_modal_rect", id)))
                        });
                        if let Some(r) = prior_rect {
                            // md-tier (16px) keeps the shadow in the CPU
                            // stacked-rect path. lg (24px) was triggering the
                            // GPU silhouette and creating an oversized ~48pt
                            // halo around every floating modal.
                            super::paint_shadow_gpu(
                                ui.painter(),
                                r,
                                super::ShadowSpec::md_themed(t),
                            );
                        }
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

                if popup_rect.width() > 0.0 && popup_rect.height() > 0.0 {
                    modal_rect = popup_rect;
                    ctx.memory_mut(|m| {
                        m.data.insert_temp(Id::new(("apex_modal_rect", id)), popup_rect);
                    });
                }

                if self.close_on_click_outside && !closed {
                    if ctx.input(|i| i.pointer.any_pressed()) {
                        if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                            if !popup_rect.contains(p) { closed = true; }
                        }
                    }
                }
            }
        }

        // Bug-report anchor for the whole dialog (no-op unless Inspect is on).
        crate::ui_kit::inspect::register(
            &format!("dialog/{}", crate::ui_kit::inspect::slug(title)),
            modal_rect,
            file!(),
            line!(),
        );

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
    shadow_tint: Color32,
) -> egui::Frame {
    let border = border_color.unwrap_or(color_alpha(toolbar_border, 80));
    egui::Frame::popup(&ctx.style())
        .fill(toolbar_bg)
        .inner_margin(0.0)
        .stroke(Stroke::new(tokens::stroke_std(), border))
        .corner_radius(r_lg_cr())
        .shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur:   28,
            spread: 2,
            color:  Color32::from_rgba_unmultiplied(
                shadow_tint.r(), shadow_tint.g(), shadow_tint.b(), 80,
            ),
        })
}
