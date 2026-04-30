//! Canonical chrome components — style-aware (Relay/Meridien) building blocks.
//!
//! These encapsulate paint patterns repeated across panels/dialogs. All radii,
//! strokes, and treatments route through `super::style` so a single style flip
//! propagates everywhere. Colors are passed in by callers (no `Theme` coupling).

#![allow(dead_code)]

use super::style::*;
use egui::{self, Color32, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

// ─── Labels ───────────────────────────────────────────────────────────────────

/// Section label — small, dim, monospace, uppercased under Meridien.
/// Use above grouped controls or table sections.
pub fn section_label_widget(ui: &mut Ui, text: &str, color: Color32) -> Response {
    section_label_sized(ui, text, color, font_sm())
}

/// Sized variant — same treatment but with caller-chosen font size.
pub fn section_label_sized(ui: &mut Ui, text: &str, color: Color32, size: f32) -> Response {
    let s = style_label_case(text);
    ui.label(
        RichText::new(s)
            .monospace()
            .size(size)
            .strong()
            .color(color),
    )
}

#[inline] pub fn section_label_xs(ui: &mut Ui, text: &str, color: Color32) -> Response { section_label_sized(ui, text, color, font_xs()) }
#[inline] pub fn section_label_md(ui: &mut Ui, text: &str, color: Color32) -> Response { section_label_sized(ui, text, color, font_md()) }
#[inline] pub fn section_label_lg(ui: &mut Ui, text: &str, color: Color32) -> Response { section_label_sized(ui, text, color, font_lg()) }

// ─── Pills / chips ────────────────────────────────────────────────────────────

/// Status pill — small accent-colored chip with text. Square under Meridien.
pub fn status_pill(ui: &mut Ui, text: &str, fill: Color32, fg: Color32) -> Response {
    let st = current();
    let cr = egui::CornerRadius::same(st.r_pill as u8);
    let stroke = if st.hairline_borders {
        Stroke::new(st.stroke_hair, color_alpha(fill, alpha_dim()))
    } else {
        Stroke::NONE
    };
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .strong()
                .color(fg),
        )
        .fill(fill)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 14.0)),
    )
}

/// Pill button — interactive, style-aware corner radius.
/// Under Meridien (`solid_active_fills`), active fills solid with `fill`.
/// Under Relay, active is a low-alpha tint.
///
/// **Deprecated**: use [`pill_button`] for new code (simpler signature, single source of truth).
#[deprecated(since = "0.10.0", note = "Use `pill_button(ui, text, active, accent, dim)` — see docs/DESIGN_SYSTEM.md")]
pub fn pill_btn(
    ui: &mut Ui,
    text: &str,
    active: bool,
    fill: Color32,
    fg_active: Color32,
    fg_inactive: Color32,
) -> Response {
    let st = current();
    let cr = egui::CornerRadius::same(st.r_pill as u8);

    let (bg, fg, stroke) = if active {
        if st.solid_active_fills {
            (fill, fg_active, Stroke::new(st.stroke_std, fill))
        } else {
            (
                color_alpha(fill, alpha_tint()),
                fg_active,
                Stroke::new(st.stroke_thin, color_alpha(fill, alpha_strong())),
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
                .size(font_sm())
                .strong()
                .color(fg),
        )
        .fill(bg)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 18.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

// ─── Frames ───────────────────────────────────────────────────────────────────

/// Card frame — surface with style-aware corners. Hairline border under Meridien;
/// soft border + drop shadow under Relay.
pub fn card_frame<R>(
    ui: &mut Ui,
    theme_bg: Color32,
    theme_border: Color32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let mut frame = egui::Frame::NONE
        .fill(theme_bg)
        .corner_radius(r_md_cr())
        .inner_margin(egui::Margin::same(gap_lg() as i8));

    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(
            st.stroke_std,
            color_alpha(theme_border, alpha_strong()),
        ));
    } else {
        frame = frame.stroke(Stroke::new(
            st.stroke_thin,
            color_alpha(theme_border, alpha_muted()),
        ));
    }

    if st.shadows_enabled {
        frame = frame.shadow(egui::epaint::Shadow {
            offset: [0, shadow_offset() as i8],
            blur: shadow_spread() as u8,
            spread: 0,
            color: Color32::from_black_alpha(shadow_alpha()),
        });
    }

    let mut out: Option<R> = None;
    frame.show(ui, |ui| {
        out = Some(add_contents(ui));
    });
    out.expect("card_frame contents")
}

/// Dialog frame — modal popups. Square + hairline under Meridien;
/// rounded + soft shadow under Relay.
pub fn dialog_frame<R>(
    ui: &mut Ui,
    theme_bg: Color32,
    theme_border: Color32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let mut frame = egui::Frame::popup(&ui.ctx().style())
        .fill(theme_bg)
        .corner_radius(r_lg_cr())
        .inner_margin(egui::Margin::same(gap_xl() as i8));

    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(st.stroke_std, theme_border));
    } else {
        frame = frame.stroke(Stroke::new(
            st.stroke_thin,
            color_alpha(theme_border, alpha_strong()),
        ));
    }

    if st.shadows_enabled {
        frame = frame.shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 2,
            color: Color32::from_black_alpha(80),
        });
    } else {
        // Meridien: explicitly clear any default popup shadow.
        frame = frame.shadow(egui::epaint::Shadow::NONE);
    }

    let mut out: Option<R> = None;
    frame.show(ui, |ui| {
        out = Some(add_contents(ui));
    });
    out.expect("dialog_frame contents")
}

// ─── Tab strip ────────────────────────────────────────────────────────────────

/// Horizontal tab strip. Returns the index clicked, or None.
/// Relay: pill background on active. Meridien: 1px bottom rule under active.
pub fn tab_strip(
    ui: &mut Ui,
    tabs: &[&str],
    active: usize,
    accent: Color32,
    dim: Color32,
) -> Option<usize> {
    let st = current();
    let mut clicked = None;

    ui.horizontal(|ui| {
        let prev = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_md();

        for (i, label) in tabs.iter().enumerate() {
            let is_active = i == active;
            let text = style_label_case(label);
            let fg = if is_active { accent } else { dim };

            if is_active && !st.hairline_borders {
                // Relay: pill background behind active tab.
                let resp = ui.add(
                    egui::Button::new(
                        RichText::new(text).monospace().size(font_md()).strong().color(fg),
                    )
                    .fill(color_alpha(accent, alpha_tint()))
                    .stroke(Stroke::NONE)
                    .corner_radius(r_pill())
                    .min_size(Vec2::new(0.0, 20.0)),
                );
                if resp.clicked() {
                    clicked = Some(i);
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            } else {
                let resp = ui.add(
                    egui::Button::new(
                        RichText::new(text).monospace().size(font_md()).strong().color(fg),
                    )
                    .frame(false)
                    .min_size(Vec2::new(0.0, 20.0)),
                );
                if resp.clicked() {
                    clicked = Some(i);
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if is_active && st.hairline_borders {
                    let r = resp.rect;
                    ui.painter().line_segment(
                        [
                            Pos2::new(r.left(), r.bottom() + 0.5),
                            Pos2::new(r.right(), r.bottom() + 0.5),
                        ],
                        Stroke::new(st.stroke_std, accent),
                    );
                }
            }
        }

        ui.spacing_mut().item_spacing.x = prev;
    });

    clicked
}

// ─── Pane header bar ──────────────────────────────────────────────────────────

/// Pane header bar — standard header above a pane. Honors `hairline_borders`
/// for the bottom rule.
pub fn pane_header_bar<R>(
    ui: &mut Ui,
    height: f32,
    theme_bg: Color32,
    theme_border: Color32,
    contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let avail_w = ui.available_width();
    let (rect, _resp) =
        ui.allocate_exact_size(Vec2::new(avail_w, height), Sense::hover());

    // Background fill.
    ui.painter().rect_filled(rect, r_md_cr(), theme_bg);

    // Bottom rule.
    let rule_color = if st.hairline_borders {
        color_alpha(theme_border, alpha_heavy())
    } else {
        color_alpha(theme_border, alpha_muted())
    };
    let rule_w = if st.hairline_borders {
        st.stroke_std
    } else {
        st.stroke_thin
    };
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), rect.bottom() - 0.5),
            Pos2::new(rect.right(), rect.bottom() - 0.5),
        ],
        Stroke::new(rule_w, rule_color),
    );

    // Inner ui for header contents, with horizontal layout.
    let inner_rect = rect.shrink2(Vec2::new(gap_lg(), gap_xs()));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    contents(&mut child)
}

// ─── Themed popup frame ───────────────────────────────────────────────────────

/// Pre-themed `egui::Frame` for use inside `egui::Window::frame(...)` and
/// similar contexts where the caller cannot pass a closure.
/// Replaces hand-rolled `Frame::popup(...).fill(...).stroke(...).corner_radius(...)`
/// boilerplate. Honors `hairline_borders` and `shadows_enabled`.
pub fn themed_popup_frame(
    ctx: &egui::Context,
    theme_bg: Color32,
    theme_border: Color32,
) -> egui::Frame {
    let st = current();
    // Under Meridien, popup bg is slightly LIGHTER than the canvas — picks the
    // popup off the surrounding chrome with the soft drop-shadow.
    let pop_bg = if st.hairline_borders {
        theme_bg.gamma_multiply(1.10)
    } else {
        theme_bg
    };
    let mut frame = egui::Frame::popup(&ctx.style())
        .fill(pop_bg)
        .corner_radius(r_lg_cr())
        .inner_margin(egui::Margin::same(gap_lg() as i8));

    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(st.stroke_std, theme_border));
    } else {
        frame = frame.stroke(Stroke::new(
            st.stroke_thin,
            color_alpha(theme_border, alpha_strong()),
        ));
    }

    if st.shadows_enabled {
        // Soft, diffused drop-shadow tuned to match the Meridien close-up
        // reference — low offset, generous blur, near-zero spread, faint alpha.
        frame = frame.shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 1,
            color: Color32::from_black_alpha(40),
        });
    } else {
        frame = frame.shadow(egui::epaint::Shadow::NONE);
    }

    frame
}

// ─── Panel header ─────────────────────────────────────────────────────────────

/// Standardized panel header row — title on the left, optional close button on
/// the right. Returns `true` if the close button was clicked. Common pattern in
/// almost every floating panel (object_tree, screenshot, spread, news, discord,
/// scanner, etc).
///
/// Caller passes `*open` or similar `&mut bool`; we toggle it on close.
pub fn panel_header(
    ui: &mut Ui,
    title: &str,
    title_color: Color32,
    open: &mut bool,
) -> bool {
    let mut closed = false;
    ui.horizontal(|ui| {
        section_label_widget(ui, title, title_color);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let resp = ui.add(
                egui::Button::new(
                    RichText::new("×")
                        .monospace()
                        .size(font_md())
                        .color(title_color),
                )
                .frame(false)
                .min_size(Vec2::new(16.0, 16.0)),
            );
            if resp.clicked() {
                *open = false;
                closed = true;
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
    });
    closed
}

// ─── Hairlines ────────────────────────────────────────────────────────────────

/// Horizontal hairline — width matches available width.
pub fn hairline(ui: &mut Ui, color: Color32) {
    let st = current();
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
        Stroke::new(st.stroke_std, color),
    );
    ui.add_space(1.0);
}

/// Vertical hairline divider — for inline horizontal layouts.
pub fn v_hairline(ui: &mut Ui, color: Color32, height: f32) {
    let st = current();
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(st.stroke_std, color),
    );
}

// ─── Metric / stat displays ───────────────────────────────────────────────────

/// Metric card — small label above a large colored value, with optional subtitle.
/// Common for portfolio P&L, scanner counts, journal stats.
pub fn metric_value_with_label(
    ui: &mut Ui,
    label: &str,
    value: &str,
    color: Color32,
    size: f32,
    subtitle: Option<&str>,
    label_color: Color32,
) {
    ui.vertical(|ui| {
        section_label_xs(ui, label, label_color);
        let value_text = {
            let mut t = RichText::new(value).size(size).strong().color(color);
            if current().serif_headlines {
                t = t.family(egui::FontFamily::Name("serif".into()));
            } else {
                t = t.monospace();
            }
            t
        };
        ui.label(value_text);
        if let Some(sub) = subtitle {
            ui.label(
                RichText::new(sub)
                    .monospace()
                    .size(font_xs())
                    .color(label_color),
            );
        }
    });
}

/// Label/value row — monospace label on the left, right-aligned value.
/// Used for settings rows, stat dumps, key/value displays.
pub fn monospace_label_row(
    ui: &mut Ui,
    label: &str,
    value: &str,
    value_color: Color32,
    label_color: Color32,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .monospace()
                .size(font_sm())
                .color(label_color),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .monospace()
                    .size(font_sm())
                    .color(value_color),
            );
        });
    });
}

/// Direction badge — ▲/▼ + price, colored bull/bear.
pub fn colored_direction_badge(
    ui: &mut Ui,
    above: bool,
    price: f32,
    bull_col: Color32,
    bear_col: Color32,
) -> Response {
    let (sym, col) = if above { ("\u{25B2}", bull_col) } else { ("\u{25BC}", bear_col) };
    ui.horizontal(|ui| {
        ui.label(RichText::new(sym).monospace().size(font_xs()).color(col));
        ui.label(
            RichText::new(format!("{:.2}", price))
                .monospace()
                .size(font_sm())
                .strong()
                .color(col),
        );
    })
    .response
}

// ─── Cards ────────────────────────────────────────────────────────────────────

/// Order card — card with a left accent stripe (used for orders, alerts, plays).
/// Hover background tint. Caller fills `add_contents` with the card body.
pub fn order_card<R>(
    ui: &mut Ui,
    accent: Color32,
    bg: Color32,
    border: Color32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let mut frame = egui::Frame::NONE
        .fill(bg)
        .corner_radius(r_md_cr())
        .inner_margin(egui::Margin {
            left: gap_md() as i8 + 3,
            right: gap_lg() as i8,
            top: gap_md() as i8,
            bottom: gap_md() as i8,
        });
    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(st.stroke_std, border));
    } else {
        frame = frame.stroke(Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted())));
    }

    let mut out: Option<R> = None;
    let resp = frame.show(ui, |ui| {
        // Paint the left accent stripe inside the frame.
        let max = ui.max_rect();
        ui.painter().rect_filled(
            Rect::from_min_size(max.min, Vec2::new(2.5, max.height())),
            r_xs(),
            accent,
        );
        out = Some(add_contents(ui));
    });
    let _ = resp;
    out.expect("order_card contents")
}

// ─── Status & badges ──────────────────────────────────────────────────────────

/// Status badge — small filled pill for things like DRAFT, ACTIVE, FILLED.
pub fn status_badge(ui: &mut Ui, text: &str, bg: Color32, fg: Color32) -> Response {
    let st = current();
    let cr = egui::CornerRadius::same(st.r_pill as u8);
    let stroke = if st.hairline_borders {
        Stroke::new(st.stroke_hair, color_alpha(bg, alpha_strong()))
    } else {
        Stroke::NONE
    };
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .strong()
                .color(fg),
        )
        .fill(bg)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 12.0)),
    )
}

/// Small action button — minimal, text-only, monospace; used in tight header rows.
pub fn small_action_btn(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let resp = ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .strong()
                .color(color),
        )
        .frame(false)
        .min_size(Vec2::new(0.0, 14.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Icon-only button — frame-less, hover changes cursor.
pub fn icon_btn(ui: &mut Ui, icon: &str, color: Color32, size: f32) -> Response {
    let resp = ui.add(
        egui::Button::new(RichText::new(icon).size(size).color(color))
            .frame(false)
            .min_size(Vec2::new(size + 2.0, size + 2.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

// ─── Empty state ──────────────────────────────────────────────────────────────

/// Empty state — centered icon + title + subtitle for "No data" placeholders.
pub fn empty_state_panel(
    ui: &mut Ui,
    icon: &str,
    title: &str,
    subtitle: &str,
    dim: Color32,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(gap_3xl());
        ui.label(RichText::new(icon).size(font_2xl() * 1.5).color(dim));
        ui.add_space(gap_md());
        ui.label(
            RichText::new(title)
                .monospace()
                .size(font_md())
                .strong()
                .color(dim),
        );
        ui.add_space(gap_xs());
        ui.label(
            RichText::new(subtitle)
                .monospace()
                .size(font_sm())
                .color(color_alpha(dim, alpha_muted())),
        );
    });
}

// ─── Stat bar ─────────────────────────────────────────────────────────────────

/// Insight stat bar — label, filled progress bar, count + pct.
pub fn insight_stat_bar(
    ui: &mut Ui,
    label: &str,
    pct: f32,
    count: u32,
    bar_color: Color32,
    track_color: Color32,
    label_color: Color32,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui(Vec2::new(80.0, 14.0), |ui| {
            ui.label(
                RichText::new(label)
                    .monospace()
                    .size(font_sm())
                    .color(label_color),
            );
        });

        // Bar
        let bar_w = ui.available_width() - 80.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w.max(40.0), 6.0), Sense::hover());
        ui.painter().rect_filled(rect, r_xs(), track_color);
        let fill_w = rect.width() * pct.clamp(0.0, 1.0);
        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        ui.painter().rect_filled(fill_rect, r_xs(), bar_color);

        // Right-aligned count + pct
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:>3.0}% · {}", pct * 100.0, count))
                    .monospace()
                    .size(font_xs())
                    .color(label_color),
            );
        });
    });
}

// ═══════════════════════════════════════════════════════════════════════════════
// Design-system: PillButton (canonical) + Text helpers
// Added by design-system rollout. See docs/DESIGN_SYSTEM.md.
// ═══════════════════════════════════════════════════════════════════════════════

/// Canonical pill toggle button. Replaces deprecated `pill_btn` and `filter_chip`.
///
/// - **Active**: accent-tinted fill, accent text, accent border.
/// - **Inactive**: transparent fill, dim text, dim border.
///
/// Uses `font_sm()`, `gap_md()` x-padding, `CornerRadius::same(99)` for pill shape.
pub fn pill_button(
    ui: &mut Ui,
    text: &str,
    active: bool,
    accent: Color32,
    dim: Color32,
) -> Response {
    let pill_r = egui::CornerRadius::same(99);
    let (bg, fg, border) = if active {
        (
            color_alpha(accent, alpha_muted()),
            accent,
            color_alpha(accent, alpha_active()),
        )
    } else {
        (
            Color32::TRANSPARENT,
            dim,
            color_alpha(dim, alpha_dim()),
        )
    };

    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), prev_pad.y);
    let resp = ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_sm())
                .color(fg),
        )
        .fill(bg)
        .stroke(Stroke::new(stroke_thin(), border))
        .corner_radius(pill_r)
        .min_size(egui::vec2(0.0, 18.0)),
    );
    ui.spacing_mut().button_padding = prev_pad;

    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

// ─── Brand colors (read from palette tokens) ──────────────────────────────────

/// Discord brand color — reads from `palette.discord` so brand surfaces stay
/// in sync with the design system. Falls back to the canonical Discord blurple
/// when design-mode is off.
#[inline]
pub fn discord_brand_color() -> Color32 {
    Color32::from_rgb(88, 101, 242)
}

// ─── Text role helpers ────────────────────────────────────────────────────────

/// Size variants for [`monospace_code`].
pub enum MonoSize {
    /// `font_xs()` — column headers, supplemental info.
    Xs,
    /// `font_sm()` — default mono text.
    Sm,
    /// `font_md()` — emphasized mono.
    Md,
}

/// Size variants for [`numeric_display`].
pub enum NumericSize {
    /// `font_lg()` — compact price / change readout.
    Lg,
    /// `font_xl()` — secondary headline.
    Xl,
    /// 30 px — hero display (portfolio total, primary price).
    Hero,
}

/// Pane heading — large title at the top of a side pane ("Watchlist", "Orders").
/// Renders `font_lg()` strong monospace.
pub fn pane_title(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.label(RichText::new(text).monospace().size(font_lg()).strong().color(color))
}

/// Sub-section heading — "Greeks", "P&L", group names below a section header.
/// Renders `font_xs()` strong monospace.
pub fn subheader(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.label(RichText::new(text).monospace().size(font_xs()).strong().color(color))
}

/// Default UI body label. Renders `font_sm()` regular monospace.
pub fn body_label(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.label(RichText::new(text).monospace().size(font_sm()).color(color))
}

/// Secondary / dim text. Applies `alpha_muted()` to `base_color`.
pub fn muted_label(ui: &mut Ui, text: &str, base_color: Color32) -> Response {
    let c = color_alpha(base_color, alpha_muted());
    ui.label(RichText::new(text).monospace().size(font_sm()).color(c))
}

/// Monospace text for tickers / prices / code at a chosen size.
pub fn monospace_code(ui: &mut Ui, text: &str, size: MonoSize, color: Color32) -> Response {
    let sz = match size {
        MonoSize::Xs => font_xs(),
        MonoSize::Sm => font_sm(),
        MonoSize::Md => font_md(),
    };
    ui.label(RichText::new(text).monospace().size(sz).color(color))
}

/// Large numeric readout (price, P&L, account values).
/// `color` should already encode bull/bear/dim semantics at the call site.
pub fn numeric_display(ui: &mut Ui, text: &str, size: NumericSize, color: Color32) -> Response {
    let sz = match size {
        NumericSize::Lg   => font_lg(),
        NumericSize::Xl   => font_xl(),
        NumericSize::Hero => 30.0,
    };
    ui.label(RichText::new(text).monospace().size(sz).strong().color(color))
}
