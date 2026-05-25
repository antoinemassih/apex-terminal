//! Panel kit — high-level patterns for side panels.
//!
//! Composes the existing primitives (`SectionLabel`, `Button`, `Input`, frames,
//! etc.) into the patterns that side panels actually want to assemble: a section
//! header with optional count + meta + inline action, an empty-state line, a
//! compact label-and-input row, and a pair of opposing colored action buttons.
//!
//! Goal: replace ~30 lines of `ui.horizontal { SectionLabel.tiny() ... RTL { small_action_btn ... } }`
//! boilerplate per section with one builder call. Visual choices (hairline rule
//! under headers, accent counts, tone palette) are made once here.
//!
//! Visual spec
//! - Section title: 11px monospace, strong, uppercase. Tone-colored (default = dim).
//! - Optional count appended in muted-tint. Optional meta in `mono_xs` dim.
//! - Optional 1px hairline rule below the row at `alpha_subtle`.
//! - Inline action: ghost button — fg = tone, faint hover fill.
//! - Empty: single muted line, optional dim glyph prefix.
//! - InputRow: leading dim label + Input, fixed compact height.
//! - DualAction: two equal-width tone-colored ghost buttons side-by-side.

#![allow(dead_code)]

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::super::style::{
    alpha_dim, alpha_ghost, alpha_line, alpha_soft, alpha_subtle, alpha_tint,
    color_alpha, color_dim, color_subtle, current, font_md, font_xs, gap_md, gap_sm, gap_xs,
    style_label_case, stroke_thin,
};
use super::super::components::text::SectionLabel;
use crate::chart_renderer::gpu::{Theme, Watchlist};
use crate::ui_kit::widgets::{Button, tokens::{Size as KitSize, Variant}};
use crate::ui_kit::widgets::icon_placement::IconPlacement;

/// Resolve the chart-pane header height + title font from a `Watchlist`. Routes
/// through `gpu::pane_tabs_header_h` so style-token adjustments
/// (`header_height_scale` etc.) propagate identically. Side-panel headers
/// pulling from this guarantee pixel-y alignment with chart-pane headers.
fn header_metrics(wl: Option<&Watchlist>) -> (f32, f32) {
    match wl {
        Some(w) => (
            crate::chart_renderer::gpu::pane_tabs_header_h(w),
            w.pane_header_size.title_font(),
        ),
        // Fallback when no watchlist is available — Normal preset.
        None => (32.0, 12.0),
    }
}

/// Vertical hairline divider matching `painter_pane`'s `header_divider_strong`
/// — `border_variant` at alpha 200, `stroke_thin`, inset 3px from top/bottom.
/// Use this between adjacent action buttons in a panel header's trailing slot
/// so they read as a grouped cluster like the chart pane's right cluster.
///
/// ```ignore
/// PanelHeader::new("FOO").show_with(ui, t, |ui| {
///     if panel_action_btn(ui, "Save", t.accent) {}
///     panel_action_divider(ui, t);
///     if panel_action_btn(ui, "Close", t.dim) {}
/// });
/// ```
pub fn panel_action_divider(ui: &mut Ui, t: &Theme) {
    // Allocate a 1px-wide vertical slot the height of the surrounding row.
    let h = ui.spacing().interact_size.y.max(20.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, h), Sense::hover());
    let col = color_alpha(t.border_variant, 200);
    ui.painter().line_segment(
        [Pos2::new(rect.center().x, rect.top() + 3.0),
         Pos2::new(rect.center().x, rect.bottom() - 3.0)],
        Stroke::new(stroke_thin(), col),
    );
}

// ── Chart-pane-parity chrome ─────────────────────────────────────────────────
//
// `panel_chrome_*` paints the SAME header chrome used by chart panes
// (`chrome::painter_pane::PainterPaneHeader`):
//   - absolute-rect, painter-mode (not layout flow)
//   - perimeter hairline using `t.text` at `header_outer_border_alpha`
//   - title in monospace `font_md` (matches chart pane's `title_font_size`)
//   - close button with painter-mode `×` glyph + bear-on-hover tint
//
// Side panels render through `PanelHeader` / `PanelHeaderTabs` which call
// these so they line up pixel-for-pixel with chart-pane headers above them.

const HEADER_CLOSE_SIZE: f32 = 18.0;
const HEADER_TAB_TOP_INSET: f32 = 1.0;
const HEADER_TAB_HEIGHT_INSET: f32 = 2.0;

fn paint_chrome_perimeter(painter: &egui::Painter, rect: Rect, t: &Theme) {
    let st = current();
    painter.rect_stroke(
        rect, 0.0,
        Stroke::new(
            st.header_outer_border_width,
            color_alpha(t.text, st.header_outer_border_alpha),
        ),
        StrokeKind::Inside,
    );
}

/// Paint the 10px linear-alpha gradient shadow that sits BELOW the header bar,
/// fading from `from_black_alpha(42)` → transparent. Mirrors `render::pane.rs:425-448`
/// exactly so chart panes and side panels share the identical drop.
fn paint_header_shadow(ui: &Ui, header_rect: Rect, t: &Theme) {
    let shadow_h = 10.0_f32;
    let shadow_top = header_rect.bottom() + 1.0;
    let shadow_rect = Rect::from_min_max(
        Pos2::new(header_rect.left(), shadow_top),
        Pos2::new(header_rect.right(), shadow_top + shadow_h),
    );
    let painter = ui.painter_at(shadow_rect);
    // Themed: pull shadow tint from the active palette so light themes
    // (Bauhaus/Peach/Ivory/Newsprint) get a soft gray gradient instead of
    // a brown-black smudge.
    let s = t.shadow_color;
    let top_col = Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 42);
    let bot_col = Color32::TRANSPARENT;
    let mut mesh = egui::Mesh::default();
    let tl = shadow_rect.left_top();
    let tr = shadow_rect.right_top();
    let bl = shadow_rect.left_bottom();
    let br = shadow_rect.right_bottom();
    mesh.colored_vertex(tl, top_col);
    mesh.colored_vertex(tr, top_col);
    mesh.colored_vertex(br, bot_col);
    mesh.colored_vertex(bl, bot_col);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_close_btn(ui: &mut Ui, painter: &egui::Painter, rect: Rect, t: &Theme) -> bool {
    crate::ui_kit::widgets::Button::close()
        .placement(IconPlacement::PanelHeader)
        .show_at(ui, painter, rect, t)
        .on_hover_text("Close")
        .clicked()
}

// ── PanelHeader ──────────────────────────────────────────────────────────────

/// Side-panel header bar that mirrors the chart-pane header chrome
/// (`chrome::painter_pane::PainterPaneHeader`) so an open side panel lines up
/// pixel-for-pixel with the pane header above it.
///
/// Visual spec (matches `PainterPaneHeader`):
/// - Height: 28px (default; chart panes use the same)
/// - Background: panel's own `egui::SidePanel` fill (no extra fill here —
///   matches inactive chart panes which inherit the canvas bg)
/// - Outer perimeter hairline: `t.text` at `header_outer_border_alpha` /
///   `header_outer_border_width` (style-token driven)
/// - Title: monospace `font_md`, color = `t.text` (matches inactive chart pane)
/// - Close: painter-mode `×` glyph, bear-tinted on hover
///
/// ```ignore
/// if PanelHeader::new("ALERTS").icon(Icon::BELL).show(ui, t) {
///     open = false;
/// }
/// ```
#[must_use = "PanelHeader must be rendered with `.show(...)`"]
pub struct PanelHeader<'a> {
    title: &'a str,
    icon: Option<&'a str>,
    height_override: Option<f32>,
    font_size_override: Option<f32>,
    closable: bool,
    watchlist: Option<&'a Watchlist>,
}

impl<'a> PanelHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title, icon: None,
            height_override: None,
            font_size_override: None,
            closable: true,
            watchlist: None,
        }
    }
    pub fn icon(mut self, icon: &'a str) -> Self { self.icon = Some(icon); self }
    /// Pin the header to the chart-pane height/font configured by this
    /// `Watchlist`'s `pane_header_size`. Side panels that pass this line up
    /// pixel-y with the chart pane header above them.
    pub fn watchlist(mut self, wl: &'a Watchlist) -> Self { self.watchlist = Some(wl); self }
    /// Override the resolved height (escape hatch for non-aligned panels).
    pub fn height(mut self, h: f32) -> Self { self.height_override = Some(h); self }
    /// Override the resolved title font size.
    pub fn font_size(mut self, px: f32) -> Self { self.font_size_override = Some(px); self }
    /// Disable the trailing close button (e.g. when the panel cannot be closed
    /// from its own header).
    pub fn closable(mut self, on: bool) -> Self { self.closable = on; self }

    /// Render. Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui, t: &Theme) -> bool {
        self.show_full(ui, t, |_| {}, |_| {})
    }

    /// Render with caller-supplied trailing actions placed to the LEFT of the
    /// close button (RTL slot). Returns `true` if the close button was clicked.
    pub fn show_with(self, ui: &mut Ui, t: &Theme, actions: impl FnOnce(&mut Ui)) -> bool {
        self.show_full(ui, t, |_| {}, actions)
    }

    /// Render with controls immediately to the right of the title (LTR flow).
    pub fn show_with_title_actions(
        self,
        ui: &mut Ui,
        t: &Theme,
        title_actions: impl FnOnce(&mut Ui),
    ) -> bool {
        self.show_full(ui, t, title_actions, |_| {})
    }

    /// Render with both leading (LTR, after title) and trailing (RTL, before
    /// close) controls. Returns `true` if the close button was clicked.
    pub fn show_full(
        self,
        ui: &mut Ui,
        t: &Theme,
        title_actions: impl FnOnce(&mut Ui),
        actions: impl FnOnce(&mut Ui),
    ) -> bool {
        let (resolved_h, resolved_font) = header_metrics(self.watchlist);
        let h = self.height_override.unwrap_or(resolved_h);
        let font_size = self.font_size_override.unwrap_or(resolved_font);

        let avail_w = ui.available_width();
        let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::hover());
        let painter = ui.painter_at(rect);

        // Chrome: perimeter hairline + 10px gradient shadow below.
        paint_chrome_perimeter(&painter, rect, t);
        paint_header_shadow(ui, rect, t);

        let title_font = FontId::monospace(font_size);

        // Paint icon + title in painter mode.
        let mut cx = rect.left() + gap_sm();
        if let Some(g) = self.icon {
            let icon_font = FontId::monospace(font_size);
            let galley = painter.layout_no_wrap(g.to_string(), icon_font.clone(), t.accent);
            painter.text(
                Pos2::new(cx, rect.center().y),
                Align2::LEFT_CENTER, g, icon_font, t.accent,
            );
            cx += galley.size().x + gap_sm();
        }

        let title_text = style_label_case(self.title);
        let title_galley = painter.layout_no_wrap(title_text.clone(), title_font.clone(), t.text);
        let title_pos = Pos2::new(cx, rect.center().y);
        // Pseudo-bold via double-paint (same trick painter_pane symbol mode uses).
        painter.text(
            Pos2::new(title_pos.x + 0.5, title_pos.y),
            Align2::LEFT_CENTER, &title_text, title_font.clone(), t.text,
        );
        painter.text(title_pos, Align2::LEFT_CENTER, &title_text, title_font, t.text);
        cx += title_galley.size().x + gap_md();

        let mut closed = false;

        // Layout-flow child UI inside the header rect for actions + close.
        let actions_rect = Rect::from_min_max(
            Pos2::new(cx, rect.top()),
            Pos2::new(rect.right() - gap_sm(), rect.bottom()),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(actions_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        title_actions(&mut child);
        child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if self.closable {
                let close_rect = Rect::from_center_size(
                    Pos2::new(
                        rect.right() - gap_sm() - HEADER_CLOSE_SIZE / 2.0,
                        rect.center().y,
                    ),
                    Vec2::splat(HEADER_CLOSE_SIZE),
                );
                if paint_close_btn(ui, &painter, close_rect, t) {
                    closed = true;
                }
                // Reserve the close-button slot in the layout flow so trailing
                // `actions` don't paint on top of it.
                ui.add_space(HEADER_CLOSE_SIZE + gap_sm());
            }
            actions(ui);
        });
        closed
    }
}

// ── PanelHeaderTabs ──────────────────────────────────────────────────────────

/// Tab-driven variant of `PanelHeader`. Same chart-pane-aligned chrome
/// (perimeter hairline, identical height), with tabs painted in absolute-rect
/// painter mode mirroring `chrome::painter_pane`'s tab strip exactly.
///
/// Tab visual spec (matches `painter_pane.rs:541-664`):
/// - Inactive: transparent fill, dim label
/// - Active: `color_dim(t.bg)` fill, top-rounded corners only,
///   `t.text` label (`t.accent` would require active-pane multipane semantics)
/// - Vertical hairline divider between adjacent tabs in `border_variant` at
///   alpha 200, `stroke_thin`, inset 4px from top/bottom
/// - Hover: faint `toolbar_border` fill
///
/// ```ignore
/// let closed = PanelHeaderTabs::new(&mut tab, &[
///     (Tab::Book, "BOOK"),
///     (Tab::Journal, "JOURNAL"),
/// ]).show(ui, t);
/// ```
#[must_use = "PanelHeaderTabs must be rendered with `.show(...)`"]
pub struct PanelHeaderTabs<'a, T: PartialEq + Copy> {
    current: &'a mut T,
    tabs: &'a [(T, &'a str)],
    height_override: Option<f32>,
    font_size_override: Option<f32>,
    closable: bool,
    salt: &'a str,
    watchlist: Option<&'a Watchlist>,
}

impl<'a, T: PartialEq + Copy> PanelHeaderTabs<'a, T> {
    pub fn new(current: &'a mut T, tabs: &'a [(T, &'a str)]) -> Self {
        Self {
            current, tabs,
            height_override: None,
            font_size_override: None,
            closable: true,
            salt: "panel_tabs",
            watchlist: None,
        }
    }
    pub fn height(mut self, h: f32) -> Self { self.height_override = Some(h); self }
    pub fn font_size(mut self, px: f32) -> Self { self.font_size_override = Some(px); self }
    pub fn closable(mut self, on: bool) -> Self { self.closable = on; self }
    /// Stable id salt — required when multiple `PanelHeaderTabs` exist in the
    /// same window so per-tab interaction state doesn't collide.
    pub fn id_salt(mut self, salt: &'a str) -> Self { self.salt = salt; self }
    /// Pin tab height + label font to the chart pane's metrics so the tab
    /// strip lines up with the strip in the pane header above.
    pub fn watchlist(mut self, wl: &'a Watchlist) -> Self { self.watchlist = Some(wl); self }

    pub fn show(self, ui: &mut Ui, t: &Theme) -> bool {
        self.show_with(ui, t, |_| {})
    }

    /// Render with trailing actions placed to the LEFT of the close button.
    pub fn show_with(self, ui: &mut Ui, t: &Theme, actions: impl FnOnce(&mut Ui)) -> bool {
        let (resolved_h, resolved_font) = header_metrics(self.watchlist);
        let h_panel = self.height_override.unwrap_or(resolved_h);
        let font_size = self.font_size_override.unwrap_or(resolved_font);

        // Namespace tab interaction state under the parent ui's id so two
        // tabbed panels in the same context don't collide on `ui.interact`.
        let scope_id = ui.id().with(("kit_panel_tabs", self.salt));

        let avail_w = ui.available_width();
        let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail_w, h_panel), Sense::hover());
        let painter = ui.painter_at(rect);

        paint_chrome_perimeter(&painter, rect, t);
        paint_header_shadow(ui, rect, t);

        let title_font = FontId::monospace(font_size);
        let h = rect.height();
        let st_settings = current();
        let r_md_corner = super::super::style::radius_md() as u8;

        // Reserve the close button rect first so tab widths can clamp short.
        let close_w_reserved = if self.closable { HEADER_CLOSE_SIZE + gap_sm() * 2.0 } else { 0.0 };

        // ── Tab strip (painter-mode, mirrors painter_pane.rs:541-664) ──
        let tab_h = h - HEADER_TAB_HEIGHT_INSET;
        let tab_y = rect.top() + HEADER_TAB_TOP_INSET;
        let tab_pad = gap_md() + 4.0;
        let mut cx = rect.left() + gap_sm();
        let mut tab_rects: Vec<Rect> = Vec::with_capacity(self.tabs.len());

        let active_idx = self.tabs.iter().position(|(v, _)| *v == *self.current).unwrap_or(0);
        let mut new_active = active_idx;

        for (ti, (_, label)) in self.tabs.iter().enumerate() {
            let is_active = ti == active_idx;
            let label_galley = painter.layout_no_wrap(
                label.to_string(), title_font.clone(), t.dim,
            );
            let tab_w = tab_pad + label_galley.size().x + tab_pad;
            // Clamp last tab so it doesn't overlap the close button.
            let max_right = rect.right() - close_w_reserved;
            if cx + tab_w > max_right { break; }

            let tab_rect = Rect::from_min_size(Pos2::new(cx, tab_y), Vec2::new(tab_w, tab_h));
            let tab_resp = ui.interact(
                tab_rect,
                scope_id.with(("tab", ti)),
                Sense::click(),
            );
            if tab_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

            // Background — animated with motion::ease_bool, mirroring
            // painter_pane.rs:583-598. Active fades over MED (180ms), hover
            // over FAST (120ms). Painted as: idle → hover → active layered.
            use super::super::components::motion;
            let active_id = scope_id.with(("active", ti));
            let hover_id  = scope_id.with(("hover", ti));
            let active_t = motion::ease_bool(ui.ctx(), active_id, is_active, motion::MED);
            let hover_t  = motion::ease_bool(ui.ctx(), hover_id,  tab_resp.hovered() && !is_active, motion::FAST);
            let corners = CornerRadius { nw: r_md_corner, ne: r_md_corner, sw: 0, se: 0 };
            let idle_bg   = Color32::TRANSPARENT;
            let hover_bg  = color_alpha(t.toolbar_border, st_settings.tab_hover_bg_alpha);
            let active_bg = color_dim(t.bg);
            let mut tab_bg = motion::lerp_color(idle_bg, hover_bg, hover_t);
            tab_bg = motion::lerp_color(tab_bg, active_bg, active_t);
            painter.rect_filled(tab_rect, corners, tab_bg);

            // Inter-tab vertical hairline divider (painter_pane.rs:609-616).
            if ti + 1 < self.tabs.len() {
                let div_col = color_alpha(t.border_variant, 200);
                painter.line_segment(
                    [Pos2::new(tab_rect.right() + 0.5, tab_rect.top() + 4.0),
                     Pos2::new(tab_rect.right() + 0.5, tab_rect.bottom() - 4.0)],
                    Stroke::new(stroke_thin(), div_col),
                );
            }

            // Label.
            let label_color = if is_active {
                t.text
            } else {
                t.dim.gamma_multiply(st_settings.tab_inactive_alpha)
            };
            painter.text(
                Pos2::new(tab_rect.left() + tab_pad, tab_rect.center().y),
                Align2::LEFT_CENTER, label, title_font.clone(), label_color,
            );

            if tab_resp.clicked() { new_active = ti; }
            tab_rects.push(tab_rect);
            cx += tab_w + 1.0;
        }

        if new_active != active_idx {
            if let Some((v, _)) = self.tabs.get(new_active) { *self.current = *v; }
        }

        // Trailing actions + close button — built in a child UI on the right.
        let mut closed = false;
        let actions_rect = Rect::from_min_max(
            Pos2::new(cx, rect.top()),
            Pos2::new(rect.right() - gap_sm(), rect.bottom()),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(actions_rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        if self.closable {
            let close_rect = Rect::from_center_size(
                Pos2::new(
                    rect.right() - gap_sm() - HEADER_CLOSE_SIZE / 2.0,
                    rect.center().y,
                ),
                Vec2::splat(HEADER_CLOSE_SIZE),
            );
            if paint_close_btn(&mut child, &painter, close_rect, t) {
                closed = true;
            }
            child.add_space(HEADER_CLOSE_SIZE + gap_sm());
        }
        actions(&mut child);
        closed
    }
}

// ── Tone ─────────────────────────────────────────────────────────────────────

/// Semantic tone — resolves to a color via the active theme. Drives section
/// accents, action buttons, and empty-state glyphs.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum Tone {
    /// Muted/dim — default for non-emphatic section headers.
    #[default]
    Default,
    /// Accent — focus / primary call-to-action.
    Accent,
    /// Bull/positive — long, gain, confirm.
    Success,
    /// Bear/negative — destructive, loss, danger.
    Danger,
    /// Warn — amber-ish caution.
    Warn,
    /// Text — full-strength foreground.
    Text,
}

impl Tone {
    pub fn color(self, t: &Theme) -> Color32 {
        match self {
            Tone::Default => t.dim,
            Tone::Accent  => t.accent,
            Tone::Success => t.bull,
            Tone::Danger  => t.bear,
            Tone::Warn    => t.warn,
            Tone::Text    => t.text,
        }
    }
}

// ── PanelSection ─────────────────────────────────────────────────────────────

/// A standardized section header followed by its body.
///
/// ```ignore
/// let r = PanelSection::new("ACTIVE")
///     .tone(Tone::Accent)
///     .count(active.len())
///     .action("Clear All", Tone::Danger)
///     .show(ui, t, |ui| {
///         for a in &active { /* row */ }
///     });
/// if r.action_clicked { /* dispatch clear */ }
/// ```
#[must_use = "PanelSection must be rendered with `.show(...)`"]
pub struct PanelSection<'a> {
    title: &'a str,
    tone: Tone,
    count: Option<usize>,
    meta: Option<String>,
    action_label: Option<&'a str>,
    action_tone: Tone,
    rule: bool,
    body_top: f32,
    body_bottom: f32,
}

pub struct SectionResponse<R> {
    pub action_clicked: bool,
    pub body: R,
}

impl<'a> PanelSection<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            tone: Tone::Default,
            count: None,
            meta: None,
            action_label: None,
            action_tone: Tone::Accent,
            rule: true,
            body_top: gap_sm(),
            body_bottom: gap_md(),
        }
    }
    pub fn tone(mut self, t: Tone) -> Self { self.tone = t; self }
    pub fn count(mut self, n: usize) -> Self { self.count = Some(n); self }
    pub fn meta(mut self, m: impl Into<String>) -> Self { self.meta = Some(m.into()); self }
    pub fn action(mut self, label: &'a str, tone: Tone) -> Self {
        self.action_label = Some(label);
        self.action_tone = tone;
        self
    }
    /// Disable the hairline rule under the header.
    pub fn rule(mut self, on: bool) -> Self { self.rule = on; self }
    /// Override vertical spacing around the body (default: gap_sm above, gap_md below).
    pub fn margins(mut self, top: f32, bottom: f32) -> Self {
        self.body_top = top;
        self.body_bottom = bottom;
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> SectionResponse<R> {
        let title_color = self.tone.color(t);
        let action_color = self.action_tone.color(t);
        let mut action_clicked = false;

        ui.horizontal(|ui| {
            ui.add(SectionLabel::new(self.title).xs().color(title_color));
            if let Some(n) = self.count {
                ui.add_space(gap_xs());
                ui.label(
                    RichText::new(format!("{}", n))
                        .monospace()
                        .size(font_xs())
                        .strong()
                        .color(color_alpha(title_color, alpha_line())),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(label) = self.action_label {
                    if panel_action_btn(ui, label, action_color) {
                        action_clicked = true;
                    }
                    if self.meta.is_some() {
                        ui.add_space(gap_sm());
                    }
                }
                if let Some(m) = &self.meta {
                    ui.label(
                        RichText::new(m)
                            .monospace()
                            .size(font_xs())
                            .color(color_alpha(t.dim, alpha_line())),
                    );
                }
            });
        });

        if self.rule {
            ui.add_space(gap_xs());
            section_rule(ui, t);
        }
        ui.add_space(self.body_top);
        let r = body(ui);
        ui.add_space(self.body_bottom);

        SectionResponse { action_clicked, body: r }
    }
}

/// Hairline section rule: ~12% opacity of the toolbar border, full panel width.
fn section_rule(ui: &mut Ui, t: &Theme) {
    let color = color_alpha(t.toolbar_border, alpha_subtle());
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        Stroke::new(stroke_thin(), color),
    );
    ui.add_space(1.0);
}

// ── Inline action button ─────────────────────────────────────────────────────

/// Compact ghost button used inline in section headers ("Clear All", "Place").
/// Replaces `style::small_action_btn` with a cleaner ghost/hover treatment.
pub fn panel_action_btn(ui: &mut Ui, label: &str, color: Color32) -> bool {
    let theme = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
    Button::new(label).variant(Variant::Ghost).size(KitSize::Xs)
        .fg(color).min_size(Vec2::new(0.0, 16.0))
        .show(ui, &theme).clicked()
}

// ── PanelEmpty ───────────────────────────────────────────────────────────────

/// Single-line muted empty state for inside a section — short, low-key.
///
/// ```ignore
/// PanelEmpty::new("No active alerts").show(ui, t);
/// ```
#[must_use = "PanelEmpty must be rendered with `.show(...)`"]
pub struct PanelEmpty<'a> {
    text: &'a str,
    glyph: Option<&'a str>,
    indent: f32,
}

impl<'a> PanelEmpty<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, glyph: None, indent: gap_sm() }
    }
    pub fn glyph(mut self, g: &'a str) -> Self { self.glyph = Some(g); self }
    pub fn indent(mut self, px: f32) -> Self { self.indent = px; self }

    pub fn show(self, ui: &mut Ui, t: &Theme) {
        let color = color_alpha(t.dim, alpha_line());
        ui.horizontal(|ui| {
            if self.indent > 0.0 { ui.add_space(self.indent); }
            if let Some(g) = self.glyph {
                ui.label(
                    RichText::new(g)
                        .monospace()
                        .size(font_xs())
                        .color(color),
                );
                ui.add_space(gap_xs());
            }
            ui.label(
                RichText::new(self.text)
                    .monospace()
                    .size(font_xs())
                    .italics()
                    .color(color),
            );
        });
    }
}

// ── PanelInputRow ────────────────────────────────────────────────────────────

/// Compact `Label: [body]` inline row. The body closure gets a `&mut Ui` to
/// place an Input or DragValue with whatever width it needs.
///
/// Distinct from `FormRow` (which uses a fixed gutter for full forms): this is
/// for *inline* compact rows where the label is short and the input width is
/// caller-driven.
///
/// ```ignore
/// PanelInputRow::new("Price").show(ui, t, |ui| {
///     Input::new(&mut state.price).min_width(80.0).size(KitSize::Sm).show(ui, t);
/// });
/// ```
#[must_use = "PanelInputRow must be rendered with `.show(...)`"]
pub struct PanelInputRow<'a> {
    label: &'a str,
    suffix: Option<&'a str>,
    label_width: Option<f32>,
}

impl<'a> PanelInputRow<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, suffix: None, label_width: None }
    }
    pub fn suffix(mut self, s: &'a str) -> Self { self.suffix = Some(s); self }
    /// Fix the label gutter for vertical alignment across multiple rows.
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = Some(w); self }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let label_color = t.dim;
        let label_text = RichText::new(self.label)
            .monospace()
            .size(font_xs())
            .color(label_color);
        let r = ui.horizontal(|ui| {
            if let Some(w) = self.label_width {
                ui.allocate_ui_with_layout(
                    Vec2::new(w, ui.spacing().interact_size.y),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| { ui.label(label_text); },
                );
            } else {
                ui.label(label_text);
            }
            ui.add_space(gap_sm());
            let inner = body(ui);
            if let Some(suf) = self.suffix {
                ui.add_space(gap_xs());
                ui.label(
                    RichText::new(suf)
                        .monospace()
                        .size(font_xs())
                        .color(color_alpha(label_color, alpha_line())),
                );
            }
            inner
        });
        r.inner
    }
}

// ── PanelDualAction ──────────────────────────────────────────────────────────

/// A row of two equal-width tone-colored action buttons, side-by-side. The
/// pattern shows up wherever a panel exposes a binary directional choice
/// (Above/Below, Buy/Sell, Long/Short, Approve/Reject). Returns the index of
/// the clicked button (0 = left, 1 = right) or `None`.
///
/// ```ignore
/// match PanelDualAction::new(
///     ("▲ Above", Tone::Success),
///     ("▼ Below", Tone::Danger),
/// ).show(ui, t) {
///     Some(0) => add_above(),
///     Some(1) => add_below(),
///     _ => {}
/// }
/// ```
#[must_use = "PanelDualAction must be rendered with `.show(...)`"]
pub struct PanelDualAction<'a> {
    left: (&'a str, Tone),
    right: (&'a str, Tone),
    height: f32,
    gap: f32,
}

impl<'a> PanelDualAction<'a> {
    pub fn new(left: (&'a str, Tone), right: (&'a str, Tone)) -> Self {
        Self { left, right, height: crate::chart_renderer::ui::style::style_row_height(), gap: gap_xs() }
    }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn gap(mut self, g: f32) -> Self { self.gap = g; self }

    pub fn show(self, ui: &mut Ui, t: &Theme) -> Option<usize> {
        let st = current();
        let avail = ui.available_width();
        let half = ((avail - self.gap) / 2.0).max(40.0);
        let mut clicked: Option<usize> = None;

        let prev_spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = self.gap;
        ui.horizontal(|ui| {
            for (i, (label, tone)) in [self.left, self.right].into_iter().enumerate() {
                let c = tone.color(t);
                let resp = Button::new(label)
                    .variant(Variant::Chrome)
                    .size(KitSize::Sm)
                    .fg(c)
                    .fill(color_alpha(c, alpha_ghost()))
                    .stroke(Stroke::new(stroke_thin(), color_alpha(c, alpha_line())))
                    .corner_radius(st.r_md as f32)
                    .min_size(Vec2::new(half, self.height))
                    .frameless(true)
                    .show(ui, t);
                if resp.hovered() {
                    let r = resp.rect;
                    ui.painter().rect_filled(r, st.r_md as f32, color_alpha(c, alpha_soft()));
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    clicked = Some(i);
                }
            }
        });
        ui.spacing_mut().item_spacing.x = prev_spacing;
        clicked
    }
}

// ── Stat (inline label-value chip) ───────────────────────────────────────────

/// Compact tabular `LABEL value` for header right-side context (e.g. `AAPL @ 200.50`).
/// Renders both pieces in mono_xs, label in muted-line, value in tone color.
#[must_use = "Stat must be added with `ui.add(...)` to render"]
pub struct Stat<'a> {
    label: &'a str,
    value: String,
    tone: Tone,
}

impl<'a> Stat<'a> {
    pub fn new(label: &'a str, value: impl Into<String>) -> Self {
        Self { label, value: value.into(), tone: Tone::Text }
    }
    pub fn tone(mut self, t: Tone) -> Self { self.tone = t; self }
}

impl<'a> Stat<'a> {
    pub fn show(self, ui: &mut Ui, t: &Theme) {
        let lc = color_alpha(t.dim, alpha_line());
        let vc = self.tone.color(t);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();
            ui.label(
                RichText::new(self.label)
                    .monospace()
                    .size(font_xs())
                    .color(lc),
            );
            ui.label(
                RichText::new(self.value)
                    .monospace()
                    .size(font_xs())
                    .strong()
                    .color(vc),
            );
        });
    }
}

// ── Standardized panel margin helpers ────────────────────────────────────────

/// Standard vertical break between unrelated sections inside a panel.
#[inline]
pub fn section_break(ui: &mut Ui) { ui.add_space(gap_md()); }

/// Standard "list row" gap between adjacent rows of the same kind.
#[inline]
pub fn row_gap(ui: &mut Ui) { ui.add_space(gap_xs()); }

/// Panel body content frame — applies the standard side-panel inner padding
/// (left/right `gap_lg`, bottom `gap_lg`, top `gap_md` to clear the header's
/// 10px gradient shadow). Use after `PanelHeader` / `PanelHeaderWithClose`
/// when the surrounding `PanelFrame` is `zero_margin` (the default since
/// we adopted chart-pane-parity headers).
///
/// ```ignore
/// PanelHeader::new("ALERTS").show(ui, t);
/// kit::panel_body(ui, |ui| {
///     // section content with normal horizontal padding
/// });
/// ```
pub fn panel_body<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    let frame = egui::Frame::NONE
        .inner_margin(egui::Margin {
            left:   super::super::style::gap_lg() as i8,
            right:  super::super::style::gap_lg() as i8,
            top:    gap_md() as i8,
            bottom: super::super::style::gap_lg() as i8,
        });
    frame.show(ui, body).inner
}

