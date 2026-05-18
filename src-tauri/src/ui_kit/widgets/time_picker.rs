//! TimePicker — HH:MM (or HH:MM:SS) input with clock popup.
//!
//! Backed by `chrono::NaiveTime`. Two display modes:
//!  - **Inline trigger**: button showing the formatted time (or placeholder)
//!  - **Popup picker**: scrollable hour/minute columns + quick presets + "Now"
//!
//! ```ignore
//! TimePicker::new(&mut time)
//!     .show_seconds(false)
//!     .placeholder("09:30")
//!     .show(ui, theme);
//! ```

use chrono::{Local, NaiveTime, Timelike};
use egui::{
    Area, CornerRadius, FontId, Id, Margin, Order, Pos2, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

#[must_use = "TimePicker does nothing until `.show(ui, theme)` is called"]
pub struct TimePicker<'a> {
    value: &'a mut Option<NaiveTime>,
    placeholder: &'a str,
    show_seconds: bool,
    use_24h: bool,
    presets: &'a [(&'a str, NaiveTime)],
    width: Option<f32>,
    full_width: bool,
    size: Size,
    disabled: bool,
    id_salt: &'a str,
}

impl<'a> TimePicker<'a> {
    pub fn new(value: &'a mut Option<NaiveTime>) -> Self {
        Self {
            value,
            placeholder: "HH:MM",
            show_seconds: false,
            use_24h: true,
            presets: &[],
            width: None,
            full_width: false,
            size: Size::Md,
            disabled: false,
            id_salt: "default",
        }
    }

    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn show_seconds(mut self, s: bool) -> Self { self.show_seconds = s; self }
    pub fn use_24h(mut self, h: bool) -> Self { self.use_24h = h; self }
    pub fn presets(mut self, p: &'a [(&'a str, NaiveTime)]) -> Self { self.presets = p; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    pub fn id_salt(mut self, s: &'a str) -> Self { self.id_salt = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        paint_time_picker(ui, theme, self)
    }
}

fn format_time(t: NaiveTime, use_24h: bool, show_seconds: bool) -> String {
    if use_24h {
        if show_seconds {
            format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second())
        } else {
            format!("{:02}:{:02}", t.hour(), t.minute())
        }
    } else {
        let (pm, h12) = t.hour12();
        let ampm = if pm { "PM" } else { "AM" };
        if show_seconds {
            format!("{:02}:{:02}:{:02} {}", h12, t.minute(), t.second(), ampm)
        } else {
            format!("{:02}:{:02} {}", h12, t.minute(), ampm)
        }
    }
}

fn paint_time_picker<'a>(ui: &mut Ui, theme: &dyn ComponentTheme, tp: TimePicker<'a>) -> Response {
    let TimePicker {
        value,
        placeholder,
        show_seconds,
        use_24h,
        presets,
        width,
        full_width,
        size,
        disabled,
        id_salt,
    } = tp;

    let h = size.height();
    let pad_x = size.padding_x();
    let font_size = size.font_size();
    let icon_gap = st::gap_2xs();

    let base_id = Id::new("time_picker").with(id_salt);
    let open_id = base_id.with("open");

    let mut open: bool = ui.ctx().memory(|m| m.data.get_temp(open_id)).unwrap_or(false);

    // Format display text.
    let display_text = match *value {
        Some(t) => format_time(t, use_24h, show_seconds),
        None => placeholder.to_string(),
    };
    let is_placeholder = value.is_none();

    let desired_w = if full_width {
        ui.available_width()
    } else {
        width.unwrap_or(160.0)
    };

    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(desired_w, h), Sense::click());
    let id = response.id;

    let hovered = response.hovered() && !disabled;
    let hover_t = motion::ease_bool(ui.ctx(), id.with("hover"), hovered, motion::FAST);
    let focus_t = motion::ease_bool(ui.ctx(), id.with("focus"), open, motion::FAST);

    let mut border_col = motion::lerp_color(theme.border(), theme.dim(), hover_t);
    border_col = motion::lerp_color(border_col, theme.accent(), focus_t);

    let radius = CornerRadius::same(st::radius_sm() as u8);

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let bg = if disabled {
            st::color_alpha(theme.surface(), 128)
        } else {
            theme.surface()
        };
        painter.rect_filled(rect, radius, bg);
        painter.rect_stroke(rect, radius, Stroke::new(st::stroke_std(), border_col), StrokeKind::Inside);

        let icon_color = motion::lerp_color(theme.dim(), theme.accent(), focus_t);
        let cy = rect.center().y;

        // Leading clock icon.
        let icon_x = rect.left() + pad_x;
        painter.text(
            Pos2::new(icon_x, cy),
            egui::Align2::LEFT_CENTER,
            Icon::CLOCK,
            FontId::proportional(font_size * 1.1),
            icon_color,
        );

        let text_x = icon_x + font_size * 1.1 + icon_gap;
        let text_col = if disabled {
            st::color_alpha(theme.dim(), 128)
        } else if is_placeholder {
            st::color_alpha(theme.dim(), 160)
        } else {
            theme.text()
        };
        painter.text(
            Pos2::new(text_x, cy),
            egui::Align2::LEFT_CENTER,
            &display_text,
            FontId::monospace(font_size),
            text_col,
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if response.clicked() && !disabled {
        open = !open;
    }

    if open {
        let popup_id = base_id.with("popup");
        let popup_pos = Pos2::new(rect.left(), rect.bottom() + st::gap_2xs());

        let mut close_popup = false;
        let mut changed = false;

        // Current selection state (hour/minute for scroll columns).
        let sel_hour_id = base_id.with("sel_hour");
        let sel_min_id = base_id.with("sel_min");
        let sel_sec_id = base_id.with("sel_sec");

        let mut sel_hour: u32 = ui.ctx().memory(|m| m.data.get_temp(sel_hour_id)).unwrap_or_else(|| {
            value.map(|t| t.hour()).unwrap_or(0)
        });
        let mut sel_min: u32 = ui.ctx().memory(|m| m.data.get_temp(sel_min_id)).unwrap_or_else(|| {
            value.map(|t| t.minute()).unwrap_or(0)
        });
        let mut sel_sec: u32 = ui.ctx().memory(|m| m.data.get_temp(sel_sec_id)).unwrap_or_else(|| {
            value.map(|t| t.second()).unwrap_or(0)
        });

        let area_response = Area::new(popup_id)
            .order(Order::Foreground)
            .fixed_pos(popup_pos)
            .show(ui.ctx(), |ui| {
                let bg = theme.surface();
                let border = theme.border();
                let col_w: f32 = 52.0;
                let num_cols = if show_seconds { 3 } else { 2 };
                let popup_w = col_w * num_cols as f32 + st::gap_xs() * (num_cols - 1) as f32 + st::gap_sm() * 2.0 + 8.0;

                egui::Frame::none()
                    .fill(bg)
                    .stroke(Stroke::new(st::stroke_std(), border))
                    .corner_radius(CornerRadius::same(st::radius_sm() as u8))
                    .inner_margin(Margin::same(st::gap_sm() as i8))
                    .show(ui, |ui| {
                        ui.set_min_width(popup_w);

                        // ── Hour / Minute (/ Second) columns ──
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = st::gap_xs();

                            // Hours column.
                            let hour_max = if use_24h { 24u32 } else { 12u32 };
                            let hour_start = if use_24h { 0u32 } else { 1u32 };
                            render_scroll_column(ui, theme, "Hr", col_w, font_size, hour_start, hour_max, &mut sel_hour, &mut changed);

                            // Minutes column.
                            render_scroll_column(ui, theme, "Min", col_w, font_size, 0, 60, &mut sel_min, &mut changed);

                            // Optional seconds column.
                            if show_seconds {
                                render_scroll_column(ui, theme, "Sec", col_w, font_size, 0, 60, &mut sel_sec, &mut changed);
                            }
                        });

                        ui.add_space(st::gap_xs());
                        ui.separator();
                        ui.add_space(st::gap_xs());

                        // ── "Now" button ──
                        let now_resp = ui.add_sized(
                            Vec2::new(popup_w - st::gap_sm() * 2.0, size.height() * 0.85),
                            egui::Button::new(
                                egui::RichText::new("Now")
                                    .monospace()
                                    .size(font_size)
                                    .color(theme.accent()),
                            )
                            .fill(st::color_alpha(theme.accent(), 24))
                            .stroke(Stroke::new(st::stroke_std(), st::color_alpha(theme.accent(), 80))),
                        );
                        if now_resp.clicked() {
                            let now = Local::now().time();
                            sel_hour = now.hour();
                            sel_min = now.minute();
                            sel_sec = now.second();
                            changed = true;
                        }

                        // ── Preset rows ──
                        if !presets.is_empty() {
                            ui.add_space(st::gap_xs());
                            for (label, preset_time) in presets.iter() {
                                let pr = ui.add_sized(
                                    Vec2::new(popup_w - st::gap_sm() * 2.0, size.height() * 0.85),
                                    egui::Button::new(
                                        egui::RichText::new(*label)
                                            .monospace()
                                            .size(font_size)
                                            .color(theme.text()),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(Stroke::new(st::stroke_std(), theme.border())),
                                );
                                if pr.clicked() {
                                    sel_hour = preset_time.hour();
                                    sel_min = preset_time.minute();
                                    sel_sec = preset_time.second();
                                    changed = true;
                                }
                            }
                        }

                        // Apply selection to value when anything changed.
                        if changed {
                            *value = NaiveTime::from_hms_opt(sel_hour, sel_min, sel_sec);
                            close_popup = true;
                        }
                    });
            });

        // Outside click closes popup.
        if ui.ctx().input(|i| i.pointer.any_click()) {
            let click_pos = ui.ctx().input(|i| i.pointer.interact_pos());
            if let Some(pos) = click_pos {
                if !rect.contains(pos) && !area_response.response.rect.contains(pos) {
                    close_popup = true;
                }
            }
        }

        if changed {
            response.mark_changed();
        }

        if close_popup {
            open = false;
        }

        // Persist scroll column state.
        ui.ctx().memory_mut(|m| {
            m.data.insert_temp(sel_hour_id, sel_hour);
            m.data.insert_temp(sel_min_id, sel_min);
            m.data.insert_temp(sel_sec_id, sel_sec);
        });
    }

    ui.ctx().memory_mut(|m| m.data.insert_temp(open_id, open));

    response
}

/// Renders a single labeled scrollable column (hours, minutes, seconds).
fn render_scroll_column(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    label: &str,
    col_w: f32,
    font_size: f32,
    start: u32,
    count: u32,
    selected: &mut u32,
    changed: &mut bool,
) {
    ui.vertical(|ui| {
        ui.set_min_width(col_w);
        ui.set_max_width(col_w);

        // Column label.
        ui.label(
            egui::RichText::new(label)
                .monospace()
                .size(font_size - 1.0)
                .color(theme.dim()),
        );
        ui.add_space(2.0);

        egui::ScrollArea::vertical()
            .id_salt(label)
            .max_height(160.0)
            .show(ui, |ui| {
                ui.set_min_width(col_w);
                for i in start..start + count {
                    let is_sel = *selected == i;
                    let text_col = if is_sel { theme.accent() } else { theme.text() };
                    let bg = if is_sel {
                        st::color_alpha(theme.accent(), 30)
                    } else {
                        egui::Color32::TRANSPARENT
                    };

                    let (item_rect, item_resp) = ui.allocate_exact_size(
                        Vec2::new(col_w, font_size + 6.0),
                        Sense::click(),
                    );

                    if ui.is_rect_visible(item_rect) {
                        let painter = ui.painter_at(item_rect);
                        if is_sel || item_resp.hovered() {
                            painter.rect_filled(
                                item_rect,
                                CornerRadius::same(st::radius_sm() as u8),
                                if item_resp.hovered() && !is_sel {
                                    st::color_alpha(theme.dim(), 20)
                                } else {
                                    bg
                                },
                            );
                        }
                        painter.text(
                            item_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{:02}", i),
                            FontId::monospace(font_size),
                            text_col,
                        );
                    }

                    if item_resp.clicked() {
                        *selected = i;
                        *changed = true;
                    }
                    if item_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });
    });
}
