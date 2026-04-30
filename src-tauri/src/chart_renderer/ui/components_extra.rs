//! Additional canonical components — keyboard chips, search inputs, steppers,
//! toggles, filter chips, sortable headers, toasts, spinners, breadcrumbs,
//! notification badges. Style-aware via `super::style::current()`.

#![allow(dead_code)]

use super::style::*;
use egui::{self, Color32, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

// ─── Keyboard shortcut chip ───────────────────────────────────────────────────

/// Keyboard shortcut hint chip — small pill with hint text (Cmd+K, Esc).
pub fn keybind_chip(ui: &mut Ui, hint: &str, fg: Color32, bg_border: Color32) -> Response {
    let st = current();
    let cr = egui::CornerRadius::same(st.r_xs as u8);
    let stroke = if st.hairline_borders {
        Stroke::new(st.stroke_std, color_alpha(bg_border, alpha_strong()))
    } else {
        Stroke::new(st.stroke_thin, color_alpha(bg_border, alpha_muted()))
    };
    ui.add(
        egui::Button::new(
            RichText::new(hint).monospace().size(font_xs()).color(fg),
        )
        .fill(Color32::TRANSPARENT)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 14.0)),
    )
}

// ─── Search input ─────────────────────────────────────────────────────────────

/// Search input — text edit framed with a magnifier glyph.
pub fn search_input(
    ui: &mut Ui,
    buffer: &mut String,
    placeholder: &str,
    accent: Color32,
    dim: Color32,
    border: Color32,
) -> egui::Response {
    let st = current();
    let avail = ui.available_width();
    let frame = egui::Frame::NONE
        .fill(Color32::TRANSPARENT)
        .corner_radius(r_sm_cr())
        .stroke(if st.hairline_borders {
            Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
        } else {
            Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
        })
        .inner_margin(egui::Margin {
            left: gap_md() as i8,
            right: gap_md() as i8,
            top: gap_xs() as i8,
            bottom: gap_xs() as i8,
        });
    let mut resp_out: Option<egui::Response> = None;
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("\u{1F50D}").size(font_sm()).color(dim));
            let edit = egui::TextEdit::singleline(buffer)
                .desired_width(avail - 36.0)
                .hint_text(RichText::new(placeholder).color(color_alpha(dim, alpha_muted())))
                .text_color(accent)
                .frame(false);
            resp_out = Some(ui.add(edit));
        });
    });
    resp_out.expect("search_input response")
}

// ─── Numeric stepper ──────────────────────────────────────────────────────────

/// Compact stepper — 14x14 buttons with dim value (no accent). Returns delta.
/// For tight inline layouts where `numeric_stepper`'s 18x18 is too big.
pub fn compact_stepper(
    ui: &mut Ui,
    value: &str,
    dim: Color32,
    border: Color32,
) -> i32 {
    let mut delta = 0;
    ui.horizontal(|ui| {
        let prev = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();

        let st = current();
        let cr = egui::CornerRadius::same(st.r_xs as u8);
        let stroke = if st.hairline_borders {
            Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
        } else {
            Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
        };

        let mut mk_btn = |ui: &mut Ui, sym: &str| -> Response {
            ui.add(
                egui::Button::new(
                    RichText::new(sym).monospace().size(font_xs()).color(dim),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(stroke)
                .corner_radius(cr)
                .min_size(Vec2::new(14.0, 14.0)),
            )
        };

        if mk_btn(ui, "-").clicked() { delta = -1; }
        ui.label(
            RichText::new(value)
                .monospace()
                .size(font_xs())
                .color(dim),
        );
        if mk_btn(ui, "+").clicked() { delta = 1; }

        ui.spacing_mut().item_spacing.x = prev;
    });
    delta
}

/// Numeric stepper [-]value[+]. Returns delta clicks (-1, 0, +1).
pub fn numeric_stepper(
    ui: &mut Ui,
    value: &str,
    accent: Color32,
    dim: Color32,
    border: Color32,
) -> i32 {
    let mut delta = 0;
    ui.horizontal(|ui| {
        let prev = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();

        let st = current();
        let cr = egui::CornerRadius::same(st.r_xs as u8);
        let stroke = if st.hairline_borders {
            Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
        } else {
            Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
        };

        let mut mk_btn = |ui: &mut Ui, sym: &str| -> Response {
            ui.add(
                egui::Button::new(
                    RichText::new(sym).monospace().size(font_sm()).strong().color(dim),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(stroke)
                .corner_radius(cr)
                .min_size(Vec2::new(18.0, 18.0)),
            )
        };

        if mk_btn(ui, "-").clicked() { delta = -1; }
        ui.label(
            RichText::new(value)
                .monospace()
                .size(font_sm())
                .strong()
                .color(accent),
        );
        if mk_btn(ui, "+").clicked() { delta = 1; }

        ui.spacing_mut().item_spacing.x = prev;
    });
    delta
}

// ─── Toggle row ───────────────────────────────────────────────────────────────

/// Settings-style row: label on left, checkbox on right. Uppercased under Meridien.
pub fn toggle_row(
    ui: &mut Ui,
    label: &str,
    state: &mut bool,
    label_color: Color32,
) -> Response {
    let mut resp = ui.allocate_response(Vec2::ZERO, Sense::hover());
    ui.horizontal(|ui| {
        let s = style_label_case(label);
        ui.label(
            RichText::new(s)
                .monospace()
                .size(font_sm())
                .color(label_color),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            resp = ui.checkbox(state, "");
        });
    });
    resp
}

// ─── Filter chip ──────────────────────────────────────────────────────────────

/// Filter chip — togglable inline tag.
/// Filter chip toggle.
///
/// **Deprecated**: use [`super::components::pill_button`] for new code.
#[deprecated(since = "0.10.0", note = "Use `pill_button(ui, text, active, accent, dim)` — see docs/DESIGN_SYSTEM.md")]
pub fn filter_chip(
    ui: &mut Ui,
    text: &str,
    active: bool,
    accent: Color32,
    fg_inactive: Color32,
) -> Response {
    let st = current();
    let cr = egui::CornerRadius::same(st.r_pill as u8);

    let (bg, fg, stroke) = if active {
        if st.solid_active_fills {
            (accent, contrast_fg_local(accent), Stroke::NONE)
        } else {
            (
                color_alpha(accent, alpha_tint()),
                accent,
                Stroke::new(st.stroke_thin, color_alpha(accent, alpha_strong())),
            )
        }
    } else {
        (
            Color32::TRANSPARENT,
            fg_inactive,
            Stroke::new(st.stroke_thin, color_alpha(fg_inactive, alpha_muted())),
        )
    };

    let resp = ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .color(fg),
        )
        .fill(bg)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 16.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

// ─── Sortable column header ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection { None, Asc, Desc }

/// Sortable column header with sort indicator. Returns the click Response.
pub fn sortable_col_header(
    ui: &mut Ui,
    label: &str,
    width: f32,
    sort: SortDirection,
    color: Color32,
    right_align: bool,
) -> Response {
    let layout = if right_align {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    let s = style_label_case(label);
    let arrow = match sort {
        SortDirection::Asc  => " \u{25B2}",
        SortDirection::Desc => " \u{25BC}",
        SortDirection::None => "",
    };
    let text = format!("{}{}", s, arrow);
    let mut resp_out: Option<Response> = None;
    ui.allocate_ui_with_layout(Vec2::new(width, 14.0), layout, |ui| {
        let resp = ui.add(
            egui::Button::new(
                RichText::new(text).monospace().size(font_xs()).color(color),
            )
            .frame(false),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp_out = Some(resp);
    });
    resp_out.expect("sortable_col_header response")
}

// ─── Toast card ───────────────────────────────────────────────────────────────

/// Toast notification card — accent-stripe + monospace text.
pub fn toast_card(
    ui: &mut Ui,
    accent: Color32,
    bg: Color32,
    fg: Color32,
    text: &str,
) {
    let st = current();
    let mut frame = egui::Frame::NONE
        .fill(bg)
        .corner_radius(r_md_cr())
        .inner_margin(egui::Margin::same(gap_md() as i8));

    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(st.stroke_std, color_alpha(accent, alpha_strong())));
    } else {
        frame = frame.stroke(Stroke::new(st.stroke_thin, color_alpha(accent, alpha_muted())));
    }
    if st.shadows_enabled {
        frame = frame.shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 1,
            color: Color32::from_black_alpha(60),
        });
    }

    frame.show(ui, |ui| {
        let max = ui.max_rect();
        ui.painter().rect_filled(
            Rect::from_min_size(max.min, Vec2::new(2.5, max.height())),
            r_xs(),
            accent,
        );
        ui.label(
            RichText::new(text)
                .monospace()
                .size(font_sm())
                .color(fg),
        );
    });
}

// ─── Loading dots ─────────────────────────────────────────────────────────────

/// Animated three-dot loading indicator.
pub fn loading_dots(ui: &mut Ui, color: Color32) {
    let now = ui.input(|i| i.time);
    let phase = (now * 4.0) as usize % 3;
    let dot = |i: usize| if i == phase { "\u{25CF}" } else { "\u{25CB}" };
    ui.horizontal(|ui| {
        for i in 0..3 {
            ui.label(RichText::new(dot(i)).size(font_md()).color(color));
        }
    });
    ui.ctx().request_repaint();
}

// ─── Breadcrumb ───────────────────────────────────────────────────────────────

/// Path breadcrumb — segments separated by " / ". Last segment styled accent.
pub fn breadcrumb(ui: &mut Ui, segments: &[&str], accent: Color32, dim: Color32) {
    ui.horizontal(|ui| {
        let prev = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();
        let last = segments.len().saturating_sub(1);
        for (i, seg) in segments.iter().enumerate() {
            let is_last = i == last;
            let color = if is_last { accent } else { dim };
            ui.label(
                RichText::new(*seg)
                    .monospace()
                    .size(font_sm())
                    .color(color),
            );
            if !is_last {
                ui.label(
                    RichText::new("/")
                        .monospace()
                        .size(font_sm())
                        .color(color_alpha(dim, alpha_muted())),
                );
            }
        }
        ui.spacing_mut().item_spacing.x = prev;
    });
}

// ─── Notification badge ───────────────────────────────────────────────────────

/// Small filled pill with a count. Used to indicate unread items.
pub fn notification_badge(ui: &mut Ui, count: u32, accent: Color32, fg: Color32) -> Response {
    let st = current();
    let cr = egui::CornerRadius::same(st.r_pill as u8);
    let text = if count > 99 { "99+".to_string() } else { count.to_string() };
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .strong()
                .color(fg),
        )
        .fill(accent)
        .stroke(Stroke::NONE)
        .corner_radius(cr)
        .min_size(Vec2::new(14.0, 14.0)),
    )
}

// ─── Header action button ─────────────────────────────────────────────────────

/// Tiny transparent ghost glyph button for panel headers (+, ×, ⚙).
/// Frameless, dim color, hover changes cursor. Used in compact header rows
/// where a full `icon_btn` is too prominent.
pub fn header_action_btn(ui: &mut Ui, glyph: &str, dim: Color32) -> Response {
    let resp = ui.add(
        egui::Button::new(
            RichText::new(glyph)
                .monospace()
                .size(font_md())
                .color(dim),
        )
        .frame(false)
        .min_size(Vec2::new(14.0, 14.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Smaller, dimmer variant of `style::close_button` for secondary close
/// affordances inside split sections / nested headers.
pub fn secondary_close_btn(ui: &mut Ui, dim: Color32) -> bool {
    let resp = ui.add(
        egui::Button::new(
            RichText::new("\u{00D7}")
                .monospace()
                .size(font_sm())
                .color(color_alpha(dim, alpha_dim())),
        )
        .frame(false)
        .min_size(Vec2::new(14.0, 14.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

// ─── Tab bar with close ───────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    None,
    Selected(usize),
    Closed(usize),
}

/// Tab strip with a small × on each tab. Returns the action triggered.
/// Active tab visual: pill bg under Relay, hairline bottom-rule under Meridien.
pub fn tab_bar_with_close(
    ui: &mut Ui,
    tabs: &[&str],
    active: usize,
    accent: Color32,
    dim: Color32,
) -> TabAction {
    let st = current();
    let mut action = TabAction::None;

    ui.horizontal(|ui| {
        let prev_x = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();

        for (i, label) in tabs.iter().enumerate() {
            let is_active = i == active;
            let fg = if is_active { accent } else { dim };
            let s = style_label_case(label);

            // Per-tab cluster (label + ×)
            ui.horizontal(|ui| {
                let prev_inner = ui.spacing().item_spacing.x;
                ui.spacing_mut().item_spacing.x = 1.0;

                if is_active && !st.hairline_borders {
                    let resp = ui.add(
                        egui::Button::new(
                            RichText::new(s).monospace().size(font_sm()).strong().color(fg),
                        )
                        .fill(color_alpha(accent, alpha_tint()))
                        .stroke(Stroke::NONE)
                        .corner_radius(r_pill())
                        .min_size(Vec2::new(0.0, 18.0)),
                    );
                    if resp.clicked() { action = TabAction::Selected(i); }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                } else {
                    let resp = ui.add(
                        egui::Button::new(
                            RichText::new(s).monospace().size(font_sm()).strong().color(fg),
                        )
                        .frame(false)
                        .min_size(Vec2::new(0.0, 18.0)),
                    );
                    if resp.clicked() { action = TabAction::Selected(i); }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if is_active && st.hairline_borders {
                        let r = resp.rect;
                        ui.painter().line_segment(
                            [
                                egui::pos2(r.left(), r.bottom() + 0.5),
                                egui::pos2(r.right(), r.bottom() + 0.5),
                            ],
                            Stroke::new(st.stroke_std, accent),
                        );
                    }
                }

                if secondary_close_btn(ui, dim) {
                    action = TabAction::Closed(i);
                }

                ui.spacing_mut().item_spacing.x = prev_inner;
            });
        }
        ui.spacing_mut().item_spacing.x = prev_x;
    });

    action
}

// ─── Themed SidePanel frame ───────────────────────────────────────────────────

/// Pre-themed `egui::Frame` for `egui::SidePanel::frame(...)` calls. Parallel
/// to `themed_popup_frame` but tuned for docked side panels: thinner border,
/// no shadow regardless of style.
pub fn themed_side_panel_frame(
    _ctx: &egui::Context,
    theme_bg: Color32,
    theme_border: Color32,
) -> egui::Frame {
    let st = current();
    let stroke = if st.hairline_borders {
        Stroke::new(st.stroke_std, theme_border)
    } else {
        Stroke::new(st.stroke_thin, color_alpha(theme_border, alpha_strong()))
    };
    egui::Frame::NONE
        .fill(theme_bg)
        .stroke(stroke)
        .corner_radius(r_md_cr())
        .inner_margin(egui::Margin::ZERO)
}

// ─── Metric grid row ──────────────────────────────────────────────────────────

/// N-column metric row — each cell is a (label, value, value_color) triple.
/// Labels rendered above values in `font_xs` dim; values in `font_md` bold.
/// Common in dashboards / journal stats.
pub fn metric_grid_row(
    ui: &mut Ui,
    cells: &[(&str, &str, Color32)],
    label_color: Color32,
) {
    if cells.is_empty() { return; }
    let avail = ui.available_width();
    let cell_w = (avail / cells.len() as f32).max(60.0);
    ui.horizontal(|ui| {
        for (label, value, value_color) in cells {
            ui.allocate_ui(Vec2::new(cell_w, 0.0), |ui| {
                ui.vertical(|ui| {
                    let s = style_label_case(label);
                    ui.label(
                        RichText::new(s)
                            .monospace()
                            .size(font_xs())
                            .color(label_color),
                    );
                    let value_text = {
                        let mut t = RichText::new(*value)
                            .size(font_md())
                            .strong()
                            .color(*value_color);
                        if current().serif_headlines {
                            t = t.family(egui::FontFamily::Name("serif".into()));
                        } else {
                            t = t.monospace();
                        }
                        t
                    };
                    ui.label(value_text);
                });
            });
        }
    });
}

// ─── Misc utility components ──────────────────────────────────────────────────

/// Paint a row-tint behind a pre-allocated rect (for hover/selected states
/// where the row was allocated via `interact()` / `allocate_rect`).
pub fn hover_row_tint(ui: &mut Ui, rect: Rect, color: Color32) {
    ui.painter().rect_filled(rect, r_xs(), color);
}

/// Secondary caption — dim, font_xs. For URLs, timestamps, hint text under
/// primary labels.
pub fn caption_label(ui: &mut Ui, text: &str, dim: Color32) -> Response {
    ui.label(
        RichText::new(text)
            .monospace()
            .size(font_xs())
            .color(color_alpha(dim, alpha_dim())),
    )
}

/// Category section — uppercase label + body + bottom rule. Common pattern in
/// settings, hotkey editor, trendline filter.
pub fn category_section<R>(
    ui: &mut Ui,
    label: &str,
    label_color: Color32,
    rule_color: Color32,
    body: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let s = style_label_case(label);
    ui.label(
        RichText::new(s)
            .monospace()
            .size(font_xs())
            .strong()
            .color(label_color),
    );
    ui.add_space(gap_xs());
    let out = body(ui);
    ui.add_space(gap_sm());
    let stroke_w = if st.hairline_borders { st.stroke_std } else { st.stroke_thin };
    let stroke_alpha = if st.hairline_borders { alpha_strong() } else { alpha_muted() };
    let avail = ui.available_width();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [egui::pos2(ui.cursor().min.x, y), egui::pos2(ui.cursor().min.x + avail, y)],
        Stroke::new(stroke_w, color_alpha(rule_color, stroke_alpha)),
    );
    ui.add_space(gap_sm());
    out
}

/// Label + arbitrary right-side widget — like `monospace_label_row` but the
/// value slot is a closure so callers can put a status badge, button, or any
/// other widget there.
pub fn label_widget_row<R>(
    ui: &mut Ui,
    label: &str,
    label_color: Color32,
    value: impl FnOnce(&mut Ui) -> R,
) -> R {
    let mut out: Option<R> = None;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .monospace()
                .size(font_sm())
                .color(label_color),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            out = Some(value(ui));
        });
    });
    out.expect("label_widget_row value")
}

/// Symbol row — sym + name + optional tag. Used in pickers and watchlist.
pub fn symbol_row(
    ui: &mut Ui,
    sym: &str,
    name: &str,
    tag: Option<&str>,
    is_active: bool,
    accent: Color32,
    text: Color32,
    dim: Color32,
) -> Response {
    let row_h = 18.0;
    let avail = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail, row_h), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(rect, r_xs(), color_alpha(dim, alpha_subtle()));
    }
    if is_active {
        ui.painter().rect_filled(rect, r_xs(), color_alpha(accent, alpha_tint()));
    }
    let pad = gap_md();
    let baseline = rect.min.y + 4.0;
    let painter = ui.painter().clone();
    painter.text(
        egui::pos2(rect.min.x + pad, baseline),
        egui::Align2::LEFT_TOP,
        sym,
        egui::FontId::monospace(font_sm()),
        if is_active { accent } else { text },
    );
    let sym_w = (sym.len() as f32) * (font_sm() * 0.6);
    painter.text(
        egui::pos2(rect.min.x + pad + sym_w + gap_md(), baseline),
        egui::Align2::LEFT_TOP,
        name,
        egui::FontId::monospace(font_xs()),
        color_alpha(dim, alpha_strong()),
    );
    if let Some(t) = tag {
        painter.text(
            egui::pos2(rect.max.x - pad, baseline),
            egui::Align2::RIGHT_TOP,
            t,
            egui::FontId::monospace(font_xs()),
            color_alpha(accent, alpha_strong()),
        );
    }
    resp
}

// ─── Local utility ────────────────────────────────────────────────────────────

#[inline]
fn contrast_fg_local(bg: Color32) -> Color32 {
    let r = bg.r() as f32 * 0.299;
    let g = bg.g() as f32 * 0.587;
    let b = bg.b() as f32 * 0.114;
    if r + g + b > 140.0 { Color32::from_rgb(20, 20, 20) } else { Color32::from_rgb(245, 245, 245) }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Design-system: TopNav + BigAction + Menu + PaneTab + Timeframe + Inputs
// Added by design-system rollout. See docs/DESIGN_SYSTEM.md.
// ═══════════════════════════════════════════════════════════════════════════════

// ─── Helper: luminance-aware contrast color ──────────────────────────────────

#[inline]
fn ds_contrast_fg(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 140.0 { Color32::from_rgb(20, 20, 24) } else { Color32::from_rgb(240, 240, 244) }
}

// ─── TopNavButton ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopNavTreatment {
    Raised,
    Underline,
    SoftPill,
}

pub fn top_nav_btn(
    ui: &mut Ui,
    label: &str,
    active: bool,
    treatment: TopNavTreatment,
    accent: Color32,
    dim: Color32,
) -> Response {
    let fg = if active { accent } else { dim };
    let (bg, border) = match treatment {
        TopNavTreatment::Raised => {
            let b = if active { color_alpha(accent, alpha_tint()) } else { Color32::TRANSPARENT };
            let s = if active { color_alpha(accent, alpha_line()) } else { Color32::TRANSPARENT };
            (b, s)
        }
        TopNavTreatment::Underline => (Color32::TRANSPARENT, Color32::TRANSPARENT),
        TopNavTreatment::SoftPill => {
            let b = if active { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT };
            (b, Color32::TRANSPARENT)
        }
    };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_md());
    let resp = ui.add(
        egui::Button::new(RichText::new(label).size(font_md()).strong().color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_sm())
            .min_size(egui::vec2(0.0, gap_3xl())),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if active && treatment == TopNavTreatment::Underline {
        let r = resp.rect;
        ui.painter().line_segment(
            [egui::pos2(r.left() + gap_sm(), r.bottom()), egui::pos2(r.right() - gap_sm(), r.bottom())],
            Stroke::new(stroke_std(), accent),
        );
    }
    if resp.hovered() && !active && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_faint()));
    }
    resp
}

// ─── TopNavToggle ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopNavToggleSize {
    Small,
    Medium,
}

pub fn top_nav_toggle(
    ui: &mut Ui,
    icon: &str,
    active: bool,
    size: TopNavToggleSize,
    accent: Color32,
    dim: Color32,
) -> Response {
    let side = match size { TopNavToggleSize::Small => 22.0_f32, TopNavToggleSize::Medium => 28.0_f32 };
    let font = match size { TopNavToggleSize::Small => font_md(), TopNavToggleSize::Medium => font_lg() };
    let fg = if active { accent } else { dim };
    let bg = if active { color_alpha(accent, alpha_tint()) } else { Color32::TRANSPARENT };
    let border = if active { color_alpha(accent, alpha_muted()) } else { color_alpha(dim, alpha_subtle()) };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
    let resp = ui.add(
        egui::Button::new(RichText::new(icon).size(font).color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_sm())
            .min_size(egui::vec2(side, side)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        if !active {
            ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_ghost()));
        }
    }
    resp
}

// ─── BigActionButton ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTier {
    Primary,
    Destructive,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionSize { Small, Medium, Large }

/// **Canonical action button — builder + `impl Widget` API.**
///
/// This is the modern entry point. Use `ui.add(ActionButton::new("BUY").primary().large().theme(t))`
/// instead of the older `big_action_btn(...)` positional-arg helper. Adding new
/// knobs (e.g. `.icon(...)`, `.loading(true)`) becomes non-breaking, and the
/// fluent API documents the intent at the call site.
///
/// Defaults: `Primary` tier, `Medium` size, not disabled. You must call
/// `.theme(t)` (or `.palette(...)`) so the button has palette colors.
#[must_use = "ActionButton must be added with `ui.add(...)` to render"]
pub struct ActionButton<'a> {
    label: &'a str,
    tier: ActionTier,
    size: ActionSize,
    accent: Color32,
    bear: Color32,
    dim: Color32,
    disabled: bool,
    palette_set: bool,
}

impl<'a> ActionButton<'a> {
    /// New action button with the given label. Defaults to Primary/Medium/enabled.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            tier: ActionTier::Primary,
            size: ActionSize::Medium,
            accent: Color32::from_rgb(120, 140, 220),
            bear: Color32::from_rgb(220, 80, 90),
            dim: Color32::from_rgb(120, 120, 130),
            disabled: false,
            palette_set: false,
        }
    }
    pub fn tier(mut self, t: ActionTier) -> Self { self.tier = t; self }
    pub fn size(mut self, s: ActionSize) -> Self { self.size = s; self }
    pub fn primary(mut self) -> Self { self.tier = ActionTier::Primary; self }
    pub fn destructive(mut self) -> Self { self.tier = ActionTier::Destructive; self }
    pub fn secondary(mut self) -> Self { self.tier = ActionTier::Secondary; self }
    pub fn small(mut self) -> Self { self.size = ActionSize::Small; self }
    pub fn medium(mut self) -> Self { self.size = ActionSize::Medium; self }
    pub fn large(mut self) -> Self { self.size = ActionSize::Large; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    /// Supply explicit palette colors. Prefer `.theme(t)` when you have a Theme handy.
    pub fn palette(mut self, accent: Color32, bear: Color32, dim: Color32) -> Self {
        self.accent = accent; self.bear = bear; self.dim = dim;
        self.palette_set = true;
        self
    }
    /// Pull palette colors from a Theme — the common path.
    pub fn theme(self, t: &super::super::gpu::Theme) -> Self {
        self.palette(t.accent, t.bear, t.dim)
    }
}

impl<'a> egui::Widget for ActionButton<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Body is identical to `big_action_btn` — they share visual primitives.
        // Once all call sites migrate, big_action_btn will delegate here too.
        let height: f32 = match self.size {
            ActionSize::Small => 24.0, ActionSize::Medium => 32.0, ActionSize::Large => 40.0,
        };
        let font_size: f32 = match self.size {
            ActionSize::Small => font_sm(), ActionSize::Medium => font_md(), ActionSize::Large => font_lg(),
        };
        let (bg, fg, border) = if self.disabled {
            (color_alpha(self.dim, alpha_subtle()),
             color_alpha(self.dim, alpha_dim()),
             color_alpha(self.dim, alpha_line()))
        } else {
            match self.tier {
                ActionTier::Primary =>
                    (self.accent, ds_contrast_fg(self.accent), color_alpha(self.accent, alpha_active())),
                ActionTier::Destructive =>
                    (self.bear, ds_contrast_fg(self.bear), color_alpha(self.bear, alpha_active())),
                ActionTier::Secondary =>
                    (color_alpha(self.accent, alpha_faint()), self.accent, color_alpha(self.accent, alpha_muted())),
            }
        };
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_xl(), gap_xs());
        let resp = ui.add_enabled(
            !self.disabled,
            egui::Button::new(RichText::new(self.label).size(font_size).strong().color(fg))
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(radius_md())
                .min_size(egui::vec2(0.0, height)),
        );
        ui.spacing_mut().button_padding = prev_pad;
        if resp.hovered() && !self.disabled && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(resp.rect, radius_md(), color_alpha(Color32::WHITE, 12));
        }
        let _ = self.palette_set;
        resp
    }
}

/// Legacy positional-arg helper. Prefer [`ActionButton`] for new code:
/// `ui.add(ActionButton::new("BUY").primary().large().theme(t))`.
pub fn big_action_btn(
    ui: &mut Ui,
    label: &str,
    tier: ActionTier,
    size: ActionSize,
    accent: Color32,
    bear: Color32,
    dim: Color32,
    disabled: bool,
) -> Response {
    let height: f32 = match size { ActionSize::Small => 24.0, ActionSize::Medium => 32.0, ActionSize::Large => 40.0 };
    let font_size: f32 = match size { ActionSize::Small => font_sm(), ActionSize::Medium => font_md(), ActionSize::Large => font_lg() };
    let (bg, fg, border) = if disabled {
        (color_alpha(dim, alpha_subtle()), color_alpha(dim, alpha_dim()), color_alpha(dim, alpha_line()))
    } else {
        match tier {
            ActionTier::Primary => (accent, ds_contrast_fg(accent), color_alpha(accent, alpha_active())),
            ActionTier::Destructive => (bear, ds_contrast_fg(bear), color_alpha(bear, alpha_active())),
            ActionTier::Secondary => (color_alpha(accent, alpha_faint()), accent, color_alpha(accent, alpha_muted())),
        }
    };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_xl(), gap_xs());
    let resp = ui.add_enabled(
        !disabled,
        egui::Button::new(RichText::new(label).size(font_size).strong().color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_md())
            .min_size(egui::vec2(0.0, height)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !disabled && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_md(), color_alpha(Color32::WHITE, 12));
    }
    resp
}

// ─── SidePaneActionButton ────────────────────────────────────────────────────

#[allow(unused_variables)]
pub fn side_pane_action_btn(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    accent: Color32,
    dim: Color32,
) -> Response {
    let fg = accent;
    let bg = color_alpha(accent, alpha_soft());
    let border = color_alpha(accent, alpha_dim());
    let display = match icon {
        Some(ic) => format!("{} {}", ic, label),
        None => label.to_owned(),
    };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_xs());
    let resp = ui.add(
        egui::Button::new(RichText::new(display).size(font_sm()).strong().color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_sm())
            .min_size(egui::vec2(0.0, 22.0)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_faint()));
    }
    resp
}

// ─── MenuTrigger ─────────────────────────────────────────────────────────────

pub fn menu_trigger(ui: &mut Ui, label: &str, open: bool, accent: Color32, dim: Color32) -> Response {
    let fg = if open { accent } else { dim };
    let bg = if open { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT };
    let border = if open { color_alpha(accent, alpha_muted()) } else { Color32::TRANSPARENT };
    let display = format!("{} \u{25BE}", label);
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());
    let resp = ui.add(
        egui::Button::new(RichText::new(display).size(font_sm()).color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_sm())
            .min_size(egui::vec2(0.0, 20.0)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !open && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_ghost()));
    }
    resp
}

// ─── MenuItem ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MenuItemVariant {
    Default,
    Submenu,
    Checkbox(bool),
    Separator,
}

pub fn menu_item(
    ui: &mut Ui,
    label: &str,
    variant: MenuItemVariant,
    shortcut: Option<&str>,
    accent: Color32,
    dim: Color32,
) -> Response {
    if variant == MenuItemVariant::Separator {
        let (sep_rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 1.0),
            egui::Sense::hover(),
        );
        ui.painter().line_segment(
            [
                egui::pos2(sep_rect.left() + gap_sm(), sep_rect.center().y),
                egui::pos2(sep_rect.right() - gap_sm(), sep_rect.center().y),
            ],
            Stroke::new(stroke_hair(), color_alpha(dim, alpha_line())),
        );
        ui.add_space(gap_xs());
        return resp;
    }
    let prefix = match &variant {
        MenuItemVariant::Checkbox(true)  => "\u{2713} ",
        MenuItemVariant::Checkbox(false) => "  ",
        _ => "",
    };
    let suffix = match &variant {
        MenuItemVariant::Submenu => " \u{25B8}",
        _ => "",
    };
    let display = format!("{}{}{}", prefix, label, suffix);
    let fg = dim;
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_xs());
    let resp = ui.horizontal(|ui| {
        let r = ui.add(
            egui::Button::new(RichText::new(&display).size(font_sm()).color(fg))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .min_size(egui::vec2(ui.available_width().max(80.0), 20.0)),
        );
        if let Some(sc) = shortcut {
            let sc_color = color_alpha(dim, alpha_muted());
            let max_x = r.rect.right() - gap_sm();
            let y = r.rect.center().y;
            ui.painter().text(
                egui::pos2(max_x, y),
                egui::Align2::RIGHT_CENTER,
                sc,
                egui::FontId::monospace(font_xs()),
                sc_color,
            );
        }
        r
    }).inner;
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(accent, alpha_ghost()));
    }
    resp
}

// ─── PaneTabButton ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneTabStyle {
    Underline,
    Filled,
    Border,
}

pub fn pane_tab_btn(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    active: bool,
    style: PaneTabStyle,
    accent: Color32,
    dim: Color32,
) -> Response {
    let text = match icon {
        Some(ic) => format!("{} {}", ic, label),
        None => label.to_owned(),
    };
    let fg = if active { accent } else { dim };
    let (bg, border) = match (active, style) {
        (true, PaneTabStyle::Filled) => (color_alpha(accent, alpha_tint()), color_alpha(accent, alpha_active())),
        (true, PaneTabStyle::Border) => (Color32::TRANSPARENT, color_alpha(accent, alpha_active())),
        _ => (Color32::TRANSPARENT, Color32::TRANSPARENT),
    };
    let cr = egui::CornerRadius::same(radius_sm() as u8);
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());
    let resp = ui.add(
        egui::Button::new(RichText::new(&text).monospace().size(font_sm()).color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(cr)
            .min_size(egui::vec2(0.0, 22.0)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if active && style == PaneTabStyle::Underline {
        let r = resp.rect;
        ui.painter().line_segment(
            [egui::pos2(r.left() + 3.0, r.bottom() + 1.0), egui::pos2(r.right() - 3.0, r.bottom() + 1.0)],
            Stroke::new(stroke_thick(), color_alpha(accent, alpha_strong())),
        );
    }
    resp
}

// ─── TimeframeSelector ───────────────────────────────────────────────────────

pub fn timeframe_selector(
    ui: &mut Ui,
    options: &[&str],
    active_idx: usize,
    accent: Color32,
    dim: Color32,
) -> Option<usize> {
    let mut clicked = None;
    let pill_r = egui::CornerRadius::same(99);
    let prev_item_spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = gap_xs();
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_xs());
    for (i, &label) in options.iter().enumerate() {
        let active = i == active_idx;
        let fg = if active { accent } else { dim };
        let (bg, border) = if active {
            (color_alpha(accent, alpha_tint()), color_alpha(accent, alpha_dim()))
        } else {
            (Color32::TRANSPARENT, Color32::TRANSPARENT)
        };
        let resp = ui.add(
            egui::Button::new(RichText::new(label).monospace().size(font_sm()).strong().color(fg))
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(pill_r)
                .min_size(egui::vec2(0.0, 20.0)),
        );
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() && i != active_idx {
            clicked = Some(i);
        }
    }
    ui.spacing_mut().button_padding = prev_pad;
    ui.spacing_mut().item_spacing.x = prev_item_spacing;
    clicked
}

// ─── TextInput ───────────────────────────────────────────────────────────────

pub fn text_input_field(
    ui: &mut Ui,
    buffer: &mut String,
    placeholder: &str,
    accent: Color32,
    _dim: Color32,
    border: Color32,
) -> Response {
    let id = ui.next_auto_id();
    let focused = ui.memory(|m| m.has_focus(id));
    let border_color = if focused { color_alpha(accent, alpha_active()) } else { color_alpha(border, alpha_line()) };
    let frame = egui::Frame::NONE
        .stroke(Stroke::new(1.0, border_color))
        .inner_margin(gap_sm())
        .corner_radius(radius_sm());
    let mut resp_opt: Option<Response> = None;
    frame.show(ui, |ui| {
        let te = egui::TextEdit::singleline(buffer)
            .id(id)
            .hint_text(placeholder)
            .font(egui::FontSelection::FontId(egui::FontId::monospace(font_sm())))
            .frame(false)
            .desired_width(ui.available_width());
        resp_opt = Some(ui.add(te));
    });
    resp_opt.unwrap_or_else(|| ui.label(""))
}

// ─── NumericInput ────────────────────────────────────────────────────────────

pub fn numeric_input_field(
    ui: &mut Ui,
    value: &mut f32,
    placeholder: &str,
    accent: Color32,
    dim: Color32,
    border: Color32,
) -> Response {
    let buf_id = ui.next_auto_id();
    let mut buf = ui.memory_mut(|m| {
        m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()).clone()
    });
    let resp = text_input_field(ui, &mut buf, placeholder, accent, dim, border);
    ui.memory_mut(|m| {
        *m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()) = buf.clone();
    });
    if resp.lost_focus() {
        if let Ok(parsed) = buf.trim().parse::<f32>() {
            *value = parsed;
        }
        ui.memory_mut(|m| {
            *m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()) =
                value.to_string();
        });
    }
    resp
}

// ─── ToggleSwitch ────────────────────────────────────────────────────────────

pub fn toggle_switch(ui: &mut Ui, state: &mut bool, accent: Color32, dim: Color32) -> Response {
    let track_w: f32 = 32.0;
    let track_h: f32 = 16.0;
    let thumb_d: f32 = 12.0;
    let pad: f32 = 2.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(track_w, track_h), egui::Sense::click());
    if resp.clicked() {
        *state = !*state;
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let anim_id = resp.id;
    let t = ui.ctx().animate_bool_with_time(anim_id, *state, 0.15);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let cr = track_h / 2.0;
        let track_color = if *state { color_alpha(accent, alpha_active()) } else { color_alpha(dim, alpha_dim()) };
        p.rect_filled(rect, cr, track_color);
        if !*state {
            p.rect_stroke(rect, cr, Stroke::new(1.0, color_alpha(dim, alpha_muted())), egui::StrokeKind::Inside);
        }
        let thumb_travel = track_w - thumb_d - pad * 2.0;
        let thumb_cx = rect.left() + pad + thumb_d / 2.0 + thumb_travel * t;
        let thumb_cy = rect.center().y;
        p.circle_filled(egui::pos2(thumb_cx, thumb_cy), thumb_d / 2.0, Color32::WHITE);
    }
    resp
}

// ─── RadioButtonRow ──────────────────────────────────────────────────────────

pub fn radio_button_row<T: PartialEq + Clone>(
    ui: &mut Ui,
    current_val: &mut T,
    options: &[(T, &str)],
    accent: Color32,
    dim: Color32,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap_md();
        for (value, label) in options {
            let active = *current_val == *value;
            let fg = if active { accent } else { color_alpha(dim, alpha_muted()) };
            let bg = if active { color_alpha(accent, alpha_subtle()) } else { Color32::TRANSPARENT };
            let border = if active { color_alpha(accent, alpha_dim()) } else { color_alpha(dim, alpha_line()) };
            let resp = ui.add(
                egui::Button::new(RichText::new(*label).monospace().size(font_sm()).strong().color(fg))
                    .fill(bg)
                    .stroke(Stroke::new(1.0, border))
                    .corner_radius(radius_sm())
                    .min_size(egui::vec2(0.0, 20.0)),
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() && !active {
                *current_val = value.clone();
                changed = true;
            }
        }
    });
    changed
}

// ═══════════════════════════════════════════════════════════════════════════════
// Bespoke-but-shared components
//
// These look custom but share their visual primitives (border thickness, font
// sizes, corner radii, hover treatment) with the canonical buttons via the
// same token helpers. Use these instead of inline `egui::Button::new(...)` for
// the patterns they cover.
// ═══════════════════════════════════════════════════════════════════════════════

/// Brand-color CTA — like `big_action_btn` but with an explicit brand color
/// (e.g. Discord blurple from `palette.discord`). Uses the same height,
/// padding, font, radius, and border as `big_action_btn` so brand CTAs feel
/// like first-class action buttons in the same family.
pub fn brand_cta_button(
    ui: &mut Ui,
    label: &str,
    brand_color: Color32,
    fg_color: Color32,
    size: ActionSize,
    disabled: bool,
) -> Response {
    let height: f32 = match size { ActionSize::Small => 24.0, ActionSize::Medium => 32.0, ActionSize::Large => 40.0 };
    let font_size: f32 = match size { ActionSize::Small => font_sm(), ActionSize::Medium => font_md(), ActionSize::Large => font_lg() };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_xl(), gap_xs());
    let resp = ui.add_enabled(
        !disabled,
        egui::Button::new(RichText::new(label).size(font_size).strong().color(fg_color))
            .fill(brand_color)
            .stroke(Stroke::new(stroke_thin(), color_alpha(brand_color, alpha_active())))
            .corner_radius(radius_md())
            .min_size(egui::vec2(0.0, height)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    if resp.hovered() && !disabled && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, radius_md(), color_alpha(Color32::WHITE, 12));
    }
    resp
}

/// Display chip — non-interactive status indicator. Uses the same shape and
/// sizing as `pill_button`; no click behavior. Pass a single semantic color
/// (e.g. session_col, paper_orange, live_green); the chip tints its bg with
/// `alpha_tint()` and uses the color for the border + text.
pub fn display_chip(
    ui: &mut Ui,
    label: &str,
    color: Color32,
) -> Response {
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), 0.0);
    let resp = ui.add(
        egui::Button::new(
            RichText::new(label)
                .monospace()
                .size(font_xs())
                .strong()
                .color(color),
        )
        .fill(color_alpha(color, alpha_tint()))
        .stroke(Stroke::new(stroke_thin(), color_alpha(color, alpha_dim())))
        .corner_radius(egui::CornerRadius::same(99))
        .min_size(egui::vec2(0.0, 14.0))
        .sense(egui::Sense::hover()),
    );
    ui.spacing_mut().button_padding = prev_pad;
    resp
}

/// Removable chip — text + ✕ in a single pill. Returns
/// `(label_resp, x_clicked)` so the caller can act on either.
/// Visual signature matches `pill_button`.
pub fn removable_chip(
    ui: &mut Ui,
    text: &str,
    accent: Color32,
    dim: Color32,
) -> (Response, bool) {
    let mut x_clicked = false;
    let resp = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_md(), 0.0);
        // Body label (looks like a pill, no hover affordance)
        let body = ui.add(
            egui::Button::new(
                RichText::new(text)
                    .monospace()
                    .size(font_sm())
                    .color(dim),
            )
            .fill(color_alpha(accent, alpha_faint()))
            .stroke(Stroke::new(stroke_thin(), color_alpha(dim, alpha_dim())))
            .corner_radius(egui::CornerRadius { nw: 99, sw: 99, ne: 0, se: 0 })
            .min_size(egui::vec2(0.0, 18.0)),
        );
        // ✕ remove button (paired)
        let x = ui.add(
            egui::Button::new(
                RichText::new("\u{00D7}")
                    .monospace()
                    .size(font_sm())
                    .color(dim),
            )
            .fill(color_alpha(accent, alpha_faint()))
            .stroke(Stroke::new(stroke_thin(), color_alpha(dim, alpha_dim())))
            .corner_radius(egui::CornerRadius { nw: 0, sw: 0, ne: 99, se: 99 })
            .min_size(egui::vec2(18.0, 18.0)),
        );
        ui.spacing_mut().button_padding = prev_pad;
        if x.clicked() { x_clicked = true; }
        if x.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        body
    }).inner;
    (resp, x_clicked)
}

// ─── Painter-positioned bespoke components ──────────────────────────────────
//
// These look custom because they use `allocate_rect + Painter::*` instead of
// `Ui::add(Button)`, but they ARE design-system components — they read from
// the same primitives (font_*, gap_*, stroke_*, radius_*, alpha_*, palette
// bindings) so editing a token in the inspector recolors / resizes them in
// lock-step with the canonical buttons.
//
// Use these when you need a button positioned with absolute/painter coords —
// inside a column-aligned ladder (DOM), at a fixed rect inside a toolbar
// (search pill), or with column-wide hover that spans the full toolbar height
// (window controls).

/// Search / command-launcher pill. Painter-positioned because it sits inside
/// the toolbar at a fixed-width pill, not inside an `egui::Ui` layout flow.
/// Visual primitives (border thickness, corner radius, font) match the
/// canonical text-input components — same family, different layout.
pub fn paint_search_command_pill(
    ui: &mut Ui,
    rect: egui::Rect,
    panel_rect: egui::Rect,
    icon: &str,
    label: &str,
    bg: Color32,
    bg_hover: Color32,
    border: egui::Stroke,
    icon_color: Color32,
    label_color: Color32,
) -> Response {
    let resp = ui.allocate_rect(rect, egui::Sense::click());
    let p = ui.painter_at(panel_rect);
    let r_cr = egui::CornerRadius::same(crate::dt_f32!(radius.xs, 2.0) as u8);
    let actual_bg = if resp.hovered() { bg_hover } else { bg };
    p.rect_filled(rect, r_cr, actual_bg);
    p.rect_stroke(rect, r_cr, border, egui::StrokeKind::Inside);
    let icon_x = rect.left() + gap_lg();
    p.text(
        egui::pos2(icon_x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(font_md()),
        icon_color,
    );
    p.text(
        egui::pos2(icon_x + gap_2xl() + gap_xs(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::monospace(font_sm()),
        label_color,
    );
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Window control button (close / maximize / minimize) — painter-positioned
/// because hover paints the full toolbar-height column behind the icon, which
/// requires access to the panel's outer rect. Caller paints the icon glyph
/// (X / square / dash) on top of the returned rect.
///
/// `danger` = true → hover bg uses `danger_bg` (red for close); false → uses
/// `border_hover_bg` (subtle grey).
pub fn paint_window_control_button(
    ui: &mut Ui,
    button_rect: egui::Rect,
    panel_rect: egui::Rect,
    danger: bool,
    danger_bg: Color32,
    neutral_hover_bg: Color32,
) -> Response {
    let resp = ui.allocate_rect(button_rect, egui::Sense::click());
    if resp.hovered() {
        let bg = if danger { danger_bg } else { neutral_hover_bg };
        let full = egui::Rect::from_min_max(
            egui::pos2(button_rect.left(), panel_rect.top()),
            egui::pos2(button_rect.right(), panel_rect.bottom()),
        );
        let p = ui.ctx().layer_painter(ui.layer_id());
        p.rect_filled(full, egui::CornerRadius::ZERO, bg);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Visual tier for a DOM ladder action button. The DOM bottom row uses 6
/// distinct paint treatments — encode them here so the bottom row's call
/// sites become declarative.
#[derive(Debug, Clone, Copy)]
pub enum DomActionTier {
    /// Small `[-]` / `[+]` qty stepper. Subtle bg, dark text in light themes.
    QtyStepper,
    /// Static qty readout — non-interactive, looks like a text input.
    QtyReadout,
    /// `MARKET` / `LIMIT` toggle. Solid accent fill in light themes.
    SegmentChip,
    /// `[A]` armed-arm chip. Off → ghost grey, on → red-tinted.
    ArmedChip,
    /// Solid `BUY` action — bull color.
    Buy,
    /// Solid `SELL` action — bear color.
    Sell,
    /// Warning `FLATTEN` — orange.
    Warn,
    /// Subtle `CANCEL` — neutral grey.
    Subtle,
}

/// Inputs for `paint_dom_action` — bundle the theme/state once instead of
/// passing 8 parameters at every call site.
#[derive(Clone, Copy)]
pub struct DomActionContext<'a> {
    pub t: &'a super::super::gpu::Theme,
    pub is_light: bool,
    pub dark_ink: Color32,
    pub strong_text: Color32,
    pub armed: bool,
    pub mkt_active: bool,
}

/// Paint a single DOM ladder action button. Caller computes the rect (column-
/// aligned with the price ladder above) and supplies the click semantics; this
/// helper handles ALL visual primitives so every DOM button stays in sync with
/// the design system.
pub fn paint_dom_action(
    ui: &mut Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    tier: DomActionTier,
    ctx: DomActionContext,
) -> Response {
    use DomActionTier::*;
    let resp = ui.allocate_rect(rect, egui::Sense::click());
    let hover = resp.hovered();
    let r_xs = egui::CornerRadius::same(crate::dt_f32!(radius.xs, 2.0) as u8);
    let r_sm = egui::CornerRadius::same(crate::dt_f32!(radius.sm, 3.0) as u8);
    let t = ctx.t;
    let border_stroke = rule_stroke_for(t.bg, t.toolbar_border);

    let font_label = egui::FontId::monospace(font_xs());
    let font_glyph = egui::FontId::monospace(font_sm());

    match tier {
        QtyStepper => {
            let fill = if ctx.is_light {
                if hover { color_alpha(ctx.dark_ink, 60) } else { color_alpha(ctx.dark_ink, 30) }
            } else if hover { color_alpha(t.toolbar_border, alpha_dim()) }
              else { color_alpha(t.toolbar_border, alpha_soft()) };
            painter.rect_filled(rect, r_xs, fill);
            painter.rect_stroke(rect, r_xs, border_stroke, egui::StrokeKind::Inside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_glyph, ctx.strong_text);
        }
        QtyReadout => {
            let fill = if ctx.is_light { Color32::WHITE } else { color_alpha(t.bg, 180) };
            let text_col = if ctx.is_light { ctx.dark_ink } else { Color32::from_rgb(220,220,230) };
            painter.rect_filled(rect, egui::CornerRadius::ZERO, fill);
            painter.rect_stroke(rect, egui::CornerRadius::ZERO, border_stroke, egui::StrokeKind::Inside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label.clone(), text_col);
        }
        SegmentChip => {
            let (fill, text_col) = if ctx.is_light {
                (t.accent, Color32::WHITE)
            } else {
                (color_alpha(t.accent, if hover { 55 } else { 28 }), t.accent)
            };
            painter.rect_filled(rect, r_xs, fill);
            painter.rect_stroke(rect, r_xs, border_stroke, egui::StrokeKind::Inside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label, text_col);
        }
        ArmedChip => {
            let ac = if ctx.armed { Color32::from_rgb(230, 70, 70) } else { t.dim.gamma_multiply(0.4) };
            let fill = if ctx.armed { color_alpha(ac, 35) } else { color_alpha(t.toolbar_border, alpha_ghost()) };
            painter.rect_filled(rect, r_xs, fill);
            let stroke_a = if ctx.armed { 90 } else { 30 };
            painter.rect_stroke(rect, r_xs,
                egui::Stroke::new(stroke_thin(), color_alpha(ac, stroke_a)),
                egui::StrokeKind::Outside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label, ac);
        }
        Buy | Sell => {
            let semantic = if matches!(tier, Buy) { t.bull } else { t.bear };
            let (fill, text_col) = if ctx.is_light {
                (if hover { semantic } else { semantic.gamma_multiply(0.92) }, Color32::WHITE)
            } else {
                (if hover { color_alpha(semantic, 70) } else { color_alpha(semantic, alpha_tint()) }, semantic)
            };
            painter.rect_filled(rect, r_sm, fill);
            painter.rect_stroke(rect, r_sm,
                egui::Stroke::new(stroke_thin(), color_alpha(semantic, if ctx.is_light { 200 } else { 90 })),
                egui::StrokeKind::Outside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_glyph, text_col);
        }
        Warn => {
            let fc = Color32::from_rgb(200, 150, 50);
            painter.rect_filled(rect, r_xs,
                if hover { color_alpha(fc, alpha_line()) } else { color_alpha(fc, 18) });
            painter.rect_stroke(rect, r_xs,
                egui::Stroke::new(stroke_thin(), color_alpha(fc, alpha_line())),
                egui::StrokeKind::Outside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label,
                if hover { fc } else { fc.gamma_multiply(0.6) });
        }
        Subtle => {
            painter.rect_filled(rect, r_xs,
                if hover { color_alpha(t.dim, alpha_muted()) } else { color_alpha(t.toolbar_border, alpha_soft()) });
            painter.rect_stroke(rect, r_xs,
                egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_line())),
                egui::StrokeKind::Outside);
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font_label,
                if hover { t.dim } else { t.dim.gamma_multiply(0.5) });
        }
    }
    if hover && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let _ = ctx.mkt_active;
    resp
}

/// Pane-header right-cluster action button (`+ Compare`, `Order`, `DOM`,
/// `Options`). Painter-positioned because the cluster manages its own
/// right-to-left layout cursor + full-height vertical dividers, but each
/// button's visual flows through this single helper so all four stay in sync.
pub fn paint_pane_header_action(
    ui: &mut Ui,
    header_painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    active: bool,
    text_color: Color32,
    dim_color: Color32,
) -> Response {
    let resp = ui.allocate_rect(rect, egui::Sense::click());
    let fg = if active {
        text_color
    } else if resp.hovered() {
        text_color
    } else {
        dim_color.gamma_multiply(0.85)
    };
    header_painter.text(
        egui::pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::monospace(font_md()),
        fg,
    );
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Frameless interactive list-item row — for symbol-search results, watchlist
/// rows etc. Provides hover background using the canonical hover-alpha token.
/// Caller paints the row's content via the closure.
pub fn list_item_row<R>(
    ui: &mut Ui,
    accent: Color32,
    content: impl FnOnce(&mut Ui) -> R,
) -> (Response, R) {
    let row_h = 22.0;
    let avail_w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(avail_w, row_h),
        egui::Sense::click(),
    );
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(
            rect,
            radius_sm(),
            color_alpha(accent, alpha_ghost()),
        );
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let inner = content(&mut child);
    (resp, inner)
}
