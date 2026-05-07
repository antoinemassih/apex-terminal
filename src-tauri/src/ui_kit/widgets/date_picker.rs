//! DatePicker — Input-style trigger that opens a Calendar in a Popover.
//!
//! API:
//!   let mut date: Option<NaiveDate> = None;
//!   ui.add(DatePicker::new(&mut date).placeholder("Pick a date"));
//!
//!   let mut range = None;
//!   ui.add(DatePicker::range(&mut range).months_visible(2));

use chrono::NaiveDate;
use egui::{
    CornerRadius, FontId, Id, Pos2, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
};

use super::calendar::Calendar;
use super::motion;
use super::placement::{Align, Placement, Side};
use super::popover::Popover;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;

enum DPValue<'a> {
    Single(&'a mut Option<NaiveDate>),
    Range(&'a mut Option<(NaiveDate, NaiveDate)>),
}

#[must_use = "DatePicker does nothing until `.show(ui, theme)` is called"]
pub struct DatePicker<'a> {
    value: DPValue<'a>,
    placeholder: Option<String>,
    format: &'a str,
    min_date: Option<NaiveDate>,
    max_date: Option<NaiveDate>,
    months_visible: usize,
    full_width: bool,
    size: Size,
    id_salt: Option<Id>,
}

pub struct DatePickerResponse {
    pub response: Response,
    pub changed: bool,
}

impl<'a> DatePicker<'a> {
    pub fn new(value: &'a mut Option<NaiveDate>) -> Self {
        Self {
            value: DPValue::Single(value),
            placeholder: None,
            format: "%Y-%m-%d",
            min_date: None,
            max_date: None,
            months_visible: 1,
            full_width: false,
            size: Size::Md,
            id_salt: None,
        }
    }

    pub fn range(value: &'a mut Option<(NaiveDate, NaiveDate)>) -> Self {
        Self {
            value: DPValue::Range(value),
            placeholder: None,
            format: "%Y-%m-%d",
            min_date: None,
            max_date: None,
            months_visible: 1,
            full_width: false,
            size: Size::Md,
            id_salt: None,
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self { self.placeholder = Some(text.into()); self }
    pub fn format(mut self, fmt: &'a str) -> Self { self.format = fmt; self }
    pub fn min_date(mut self, d: NaiveDate) -> Self { self.min_date = Some(d); self }
    pub fn max_date(mut self, d: NaiveDate) -> Self { self.max_date = Some(d); self }
    pub fn months_visible(mut self, n: usize) -> Self { self.months_visible = n.clamp(1, 2); self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn id_salt(mut self, salt: impl std::hash::Hash) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> DatePickerResponse {
        paint_date_picker(ui, theme, self)
    }
}

impl<'a> Widget for DatePicker<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme).response
    }
}

fn paint_date_picker<'a>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    dp: DatePicker<'a>,
) -> DatePickerResponse {
    let DatePicker {
        value,
        placeholder,
        format,
        min_date,
        max_date,
        months_visible,
        full_width,
        size,
        id_salt,
    } = dp;

    let h = size.height();
    let pad_x = size.padding_x();
    let font_size = size.font_size();
    let icon_gap = st::gap_2xs();

    let id_base = id_salt.unwrap_or_else(|| ui.id().with("apex_date_picker"));

    // Open state persisted in memory.
    let open_id = id_base.with("open");
    let mut open: bool = ui.ctx().memory(|m| m.data.get_temp(open_id)).unwrap_or(false);

    // Format the trigger text.
    let trigger_text: String = match &value {
        DPValue::Single(v) => match **v {
            Some(d) => d.format(format).to_string(),
            None => placeholder.clone().unwrap_or_else(|| "Pick a date".to_string()),
        },
        DPValue::Range(v) => match **v {
            Some((s, e)) => format!("{} → {}", s.format(format), e.format(format)),
            None => placeholder.clone().unwrap_or_else(|| "Pick a date range".to_string()),
        },
    };
    let is_placeholder = match &value {
        DPValue::Single(v) => v.is_none(),
        DPValue::Range(v) => v.is_none(),
    };

    let desired_w = if full_width { ui.available_width() } else { 200.0 };
    let row_size = Vec2::new(desired_w, h);
    let (rect, response) = ui.allocate_exact_size(row_size, Sense::click());
    let id = response.id;

    let hovered = response.hovered();
    let hover_t = motion::ease_bool(ui.ctx(), id.with("hover"), hovered, motion::FAST);
    let focus_t = motion::ease_bool(ui.ctx(), id.with("focus"), open, motion::FAST);

    let mut border_col = motion::lerp_color(theme.border(), theme.dim(), hover_t);
    border_col = motion::lerp_color(border_col, theme.accent(), focus_t);

    let radius = CornerRadius::same(4);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, radius, theme.surface());
        painter.rect_stroke(rect, radius, Stroke::new(1.0, border_col), StrokeKind::Inside);

        // Leading calendar icon.
        let icon_color = motion::lerp_color(theme.dim(), theme.accent(), focus_t);
        let cy = rect.center().y;
        let icon_x = rect.left() + pad_x;
        painter.text(
            Pos2::new(icon_x, cy),
            egui::Align2::LEFT_CENTER,
            super::super::icons::Icon::CALENDAR_BLANK,
            FontId::proportional(font_size * 1.1),
            icon_color,
        );

        // Trigger text.
        let text_x = icon_x + font_size * 1.1 + icon_gap;
        let text_col = if is_placeholder {
            st::color_alpha(theme.dim(), 160)
        } else {
            theme.text()
        };
        let max_text_w = rect.right() - pad_x - text_x;
        let truncated = truncate_to_width(ui, &trigger_text, font_size, max_text_w);
        painter.text(
            Pos2::new(text_x, cy),
            egui::Align2::LEFT_CENTER,
            truncated,
            FontId::monospace(font_size),
            text_col,
        );
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if response.clicked() {
        open = !open;
    }

    let mut changed = false;
    if open {
        let pop_id = id_base.with("popover");
        let pop_result = Popover::new()
            .open(&mut open)
            .anchor(rect)
            .placement(Placement {
                side: Side::Bottom,
                align: Align::Start,
                offset: st::gap_2xs(),
            })
            .id(pop_id)
            .show(ui, theme, |ui| {
                let resp = match value {
                    DPValue::Single(v) => {
                        let mut cal = Calendar::new(v).size(size).months_visible(months_visible);
                        if let Some(m) = min_date { cal = cal.min_date(m); }
                        if let Some(m) = max_date { cal = cal.max_date(m); }
                        cal.id_salt(pop_id.with("cal_single")).show(ui, theme)
                    }
                    DPValue::Range(v) => {
                        let mut cal = Calendar::range(v).size(size).months_visible(months_visible);
                        if let Some(m) = min_date { cal = cal.min_date(m); }
                        if let Some(m) = max_date { cal = cal.max_date(m); }
                        cal.id_salt(pop_id.with("cal_range")).show(ui, theme)
                    }
                };
                resp.changed
            });
        if let Some(c) = pop_result {
            changed = c;
        }
    }

    ui.ctx().memory_mut(|m| m.data.insert_temp(open_id, open));

    let mut row_resp = response;
    if changed {
        row_resp.mark_changed();
    }
    DatePickerResponse {
        response: row_resp,
        changed,
    }
}

fn truncate_to_width(ui: &Ui, text: &str, font_size: f32, max_w: f32) -> String {
    let layout = ui.fonts(|f| f.layout_no_wrap(text.to_string(), FontId::monospace(font_size), egui::Color32::WHITE));
    if layout.rect.width() <= max_w || text.is_empty() {
        return text.to_string();
    }
    // Binary chop on chars.
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let candidate: String = chars[..mid].iter().collect::<String>() + "\u{2026}";
        let w = ui.fonts(|f| f.layout_no_wrap(candidate.clone(), FontId::monospace(font_size), egui::Color32::WHITE)).rect.width();
        if w <= max_w { lo = mid; } else { hi = mid - 1; }
    }
    chars[..lo].iter().collect::<String>() + "\u{2026}"
}

