//! TagInput — free-form chip entry. Type and press Enter (or Tab) to add a tag.
//!
//! Backed by a `Vec<String>` of tags + an in-progress text buffer in egui memory.
//! Each tag renders as a closable `Tag`. Backspace at start of empty buffer
//! removes the last tag.
//!
//! ```ignore
//! let mut tags: Vec<String> = vec!["red".into(), "blue".into()];
//! TagInput::new(&mut tags)
//!     .placeholder("Add tags…")
//!     .max_tags(10)
//!     .full_width()
//!     .show(ui, theme);
//! ```

use egui::{
    Area, Color32, CornerRadius, FontId, Id, Key, Margin, Order, Pos2, Rect, Response, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};

use super::tag::{Tag, TagTone};
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;

#[must_use = "TagInput does nothing until `.show(ui, theme)` is called"]
pub struct TagInput<'a> {
    tags: &'a mut Vec<String>,
    placeholder: &'a str,
    max_tags: Option<usize>,
    full_width: bool,
    width: Option<f32>,
    size: Size,
    disabled: bool,
    suggestions: &'a [&'a str],
    tone: TagTone,
    id_salt: &'a str,
}

impl<'a> TagInput<'a> {
    pub fn new(tags: &'a mut Vec<String>) -> Self {
        Self {
            tags,
            placeholder: "Add tags…",
            max_tags: None,
            full_width: false,
            width: None,
            size: Size::Md,
            disabled: false,
            suggestions: &[],
            tone: TagTone::Neutral,
            id_salt: "default",
        }
    }

    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn max_tags(mut self, n: usize) -> Self { self.max_tags = Some(n); self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    pub fn suggestions(mut self, s: &'a [&'a str]) -> Self { self.suggestions = s; self }
    pub fn tone(mut self, t: TagTone) -> Self { self.tone = t; self }
    pub fn id_salt(mut self, s: &'a str) -> Self { self.id_salt = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let TagInput {
            tags,
            placeholder,
            max_tags,
            full_width,
            width,
            size,
            disabled,
            suggestions,
            tone,
            id_salt,
        } = self;

        let font_size = size.font_size();
        let pad_x = size.padding_x();
        let pad_y = st::gap_xs();
        let min_h = size.height();
        let gap = st::gap_2xs();

        let base_id = Id::new("tag_input").with(id_salt);
        let buf_id = base_id.with("buf");
        let edit_id = base_id.with("edit");
        let sugg_open_id = base_id.with("sugg_open");

        // Load / store the in-progress buffer from memory.
        let mut buffer: String = ui.ctx().memory(|m| m.data.get_temp(buf_id)).unwrap_or_default();
        let mut sugg_open: bool = ui.ctx().memory(|m| m.data.get_temp(sugg_open_id)).unwrap_or(false);

        let can_add = max_tags.map_or(true, |m| tags.len() < m) && !disabled;

        let desired_w = if full_width {
            ui.available_width()
        } else {
            width.unwrap_or(240.0)
        };

        // ── Measure all tags to determine row height wrapping ──
        // We paint inside a vertical layout so the outer widget can grow.
        let outer_id = base_id.with("outer");

        let mut outer_resp: Option<Response> = None;
        let focused = ui.memory(|m| m.has_focus(edit_id));
        let border_col = if focused {
            theme.accent()
        } else {
            theme.border()
        };

        let result = ui.vertical(|ui| {
            // Reserve the container rect manually via allocate_ui_with_layout.
            let container_resp = ui.allocate_ui_with_layout(
                Vec2::new(desired_w, min_h),
                egui::Layout::left_to_right(egui::Align::Min),
                |ui| {
                    ui.set_min_width(desired_w);
                    ui.set_max_width(desired_w);

                    // ── Allocate a flexible rect; we'll figure actual height after laying out.
                    // Use a flow layout inside a Frame-style manual paint.
                    let start_pos = ui.cursor().min;

                    let inner_w = desired_w - pad_x * 2.0;
                    let mut cursor_x = start_pos.x + pad_x;
                    let mut cursor_y = start_pos.y + pad_y;
                    let row_h = font_size + 8.0; // tag/edit row height

                    // We need to compute total height first (two-pass).
                    // Pass 1: compute height.
                    let mut sim_x = cursor_x;
                    let mut total_h = row_h;
                    for tag_text in tags.iter() {
                        let tag_w = measure_tag_width(ui, tag_text, font_size) + st::gap_xs() + 12.0;
                        if sim_x + tag_w > start_pos.x + pad_x + inner_w && sim_x > cursor_x {
                            sim_x = cursor_x;
                            total_h += row_h + gap;
                        }
                        sim_x += tag_w + gap;
                    }
                    // Budget for the edit field.
                    let edit_min_w: f32 = 80.0;
                    if sim_x + edit_min_w > start_pos.x + pad_x + inner_w && !tags.is_empty() {
                        total_h += row_h + gap;
                    }
                    let container_h = (total_h + pad_y * 2.0).max(min_h);

                    // Allocate the container.
                    let (container_rect, container_response) =
                        ui.allocate_exact_size(Vec2::new(desired_w, container_h), Sense::click());
                    if !disabled { st::cursor::text_input(ui, &container_response); }

                    // ── Paint border + background ──
                    if ui.is_rect_visible(container_rect) {
                        let painter = ui.painter_at(container_rect);
                        let radius = CornerRadius::same(st::radius_sm() as u8);
                        let bg = if disabled {
                            st::color_alpha(theme.surface(), 128)
                        } else {
                            theme.surface()
                        };
                        painter.rect_filled(container_rect, radius, bg);
                        painter.rect_stroke(
                            container_rect,
                            radius,
                            Stroke::new(st::stroke_std(), border_col),
                            StrokeKind::Inside,
                        );
                    }

                    // ── Tag flow + TextEdit via child UIs ──
                    let mut remove_idx: Option<usize> = None;
                    let mut flow_x = cursor_x;
                    let mut flow_y = cursor_y;

                    for (idx, tag_text) in tags.iter().enumerate() {
                        let tag_w = measure_tag_width(ui, tag_text, font_size) + st::gap_xs() + 12.0;
                        if flow_x + tag_w > container_rect.right() - pad_x && flow_x > cursor_x {
                            flow_x = cursor_x;
                            flow_y += row_h + gap;
                        }
                        let tag_rect = Rect::from_min_size(
                            Pos2::new(flow_x, flow_y + (row_h - (font_size + 8.0)) * 0.5),
                            Vec2::new(tag_w, font_size + 8.0),
                        );
                        let mut child = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(tag_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );
                        child.spacing_mut().item_spacing = Vec2::ZERO;
                        let tag_resp = Tag::new(tag_text)
                            .tone(tone)
                            .size(size)
                            .closable(!disabled)
                            .show(&mut child, theme);
                        if tag_resp.closed {
                            remove_idx = Some(idx);
                        }
                        flow_x += tag_w + gap;
                    }

                    if let Some(idx) = remove_idx {
                        tags.remove(idx);
                    }

                    // ── Inline TextEdit ──
                    if can_add {
                        // Wrap to new row if needed.
                        let remaining = container_rect.right() - pad_x - flow_x;
                        if remaining < edit_min_w && flow_x > cursor_x {
                            flow_x = cursor_x;
                            flow_y += row_h + gap;
                        }
                        let edit_w = (container_rect.right() - pad_x - flow_x).max(edit_min_w);
                        let edit_rect = Rect::from_min_size(
                            Pos2::new(flow_x, flow_y),
                            Vec2::new(edit_w, row_h),
                        );

                        let pre_buf = buffer.clone();

                        let mut child = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(edit_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );
                        child.spacing_mut().item_spacing = Vec2::ZERO;
                        child.spacing_mut().button_padding = Vec2::ZERO;

                        let text_col = if disabled {
                            st::color_alpha(theme.text(), 128)
                        } else {
                            theme.text()
                        };

                        let te = egui::TextEdit::singleline(&mut buffer)
                            .id(edit_id)
                            .desired_width(edit_w)
                            .margin(Margin::ZERO)
                            .frame(false)
                            .text_color(text_col)
                            .font(egui::FontSelection::FontId(FontId::monospace(font_size)));

                        let edit_resp = child.add(te);

                        // Placeholder when empty + unfocused.
                        if buffer.is_empty() && !focused && tags.is_empty() {
                            let painter = ui.painter_at(edit_rect);
                            painter.text(
                                Pos2::new(flow_x, flow_y + row_h * 0.5),
                                egui::Align2::LEFT_CENTER,
                                placeholder,
                                FontId::monospace(font_size),
                                st::color_alpha(theme.dim(), 160),
                            );
                        }

                        // Commit on Enter or Tab.
                        let enter_pressed = ui.ctx().input(|i| i.key_pressed(Key::Enter));
                        let tab_pressed = ui.ctx().input(|i| i.key_pressed(Key::Tab));
                        if (enter_pressed || tab_pressed) && !buffer.trim().is_empty() && can_add {
                            let new_tag = buffer.trim().to_string();
                            if !new_tag.is_empty() {
                                tags.push(new_tag);
                                buffer.clear();
                            }
                        }

                        // Backspace at empty buffer removes last tag.
                        let bs_pressed = ui.ctx().input(|i| i.key_pressed(Key::Backspace));
                        if bs_pressed && buffer.is_empty() && focused {
                            tags.pop();
                        }

                        // Toggle suggestion popup.
                        if !suggestions.is_empty() {
                            let was_empty = pre_buf.is_empty();
                            let now_non_empty = !buffer.is_empty();
                            if was_empty && now_non_empty {
                                sugg_open = true;
                            } else if buffer.is_empty() {
                                sugg_open = false;
                            }
                        }
                    }

                    // Click on container focuses the editor.
                    if container_response.clicked() && !disabled {
                        ui.memory_mut(|m| m.request_focus(edit_id));
                    }

                    if container_response.hovered() && !disabled {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                    }

                    container_response
                },
            );

            container_resp.inner
        });

        let container_resp = result.inner;
        let container_rect = container_resp.rect;

        // ── Suggestions popup ──
        if sugg_open && !suggestions.is_empty() && !buffer.is_empty() {
            let filtered: Vec<&&str> = suggestions
                .iter()
                .filter(|s| s.to_lowercase().contains(&buffer.to_lowercase()))
                .collect();

            if !filtered.is_empty() {
                let popup_id = base_id.with("sugg_popup");
                let popup_pos = Pos2::new(container_rect.left(), container_rect.bottom() + st::gap_2xs());

                Area::new(popup_id)
                    .order(Order::Foreground)
                    .fixed_pos(popup_pos)
                    .show(ui.ctx(), |ui| {
                        let popup_w = container_rect.width();
                        let bg = theme.surface();
                        let border = theme.border();

                        egui::Frame::none()
                            .fill(bg)
                            .stroke(Stroke::new(st::stroke_std(), border))
                            .corner_radius(CornerRadius::same(st::radius_sm() as u8))
                            .inner_margin(Margin::symmetric(st::gap_xs() as i8, st::gap_2xs() as i8))
                            .show(ui, |ui| {
                                ui.set_min_width(popup_w);
                                ui.set_max_width(popup_w);
                                for sug in filtered {
                                    let resp = ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(*sug)
                                                .monospace()
                                                .size(font_size)
                                                .color(theme.text()),
                                        )
                                        .sense(Sense::click()),
                                    );
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() && can_add {
                                        tags.push(sug.to_string());
                                        buffer.clear();
                                        sugg_open = false;
                                    }
                                }
                            });
                    });

                // Close on outside click.
                if ui.ctx().input(|i| i.pointer.any_click()) {
                    let click_pos = ui.ctx().input(|i| i.pointer.interact_pos());
                    if let Some(pos) = click_pos {
                        if !container_rect.contains(pos) {
                            sugg_open = false;
                        }
                    }
                }
            } else {
                sugg_open = false;
            }
        }

        // Persist buffer + sugg_open.
        ui.ctx().memory_mut(|m| {
            m.data.insert_temp(buf_id, buffer);
            m.data.insert_temp(sugg_open_id, sugg_open);
        });

        container_resp
    }
}

/// Measure the pixel width of a tag label (without padding/icon overhead).
fn measure_tag_width(ui: &Ui, text: &str, font_size: f32) -> f32 {
    ui.fonts(|f| {
        // layout-only: only `.rect.width()` is read; color is discarded.
        f.layout_no_wrap(
            text.to_string(),
            FontId::proportional(font_size - 1.0),
            Color32::WHITE,
        )
        .rect
        .width()
    })
}
