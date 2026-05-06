//! Tab strip, pane header bar, and panel header row (title + close × button).

use super::super::style::*;
use super::labels::section_label_widget;
use egui::{self, Color32, Pos2, RichText, Sense, Stroke, Ui, Vec2};

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

