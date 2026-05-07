//! Calendar — month view date picker.
//!
//! Three modes share one builder:
//!
//! API (single date):
//!   let mut selected: Option<chrono::NaiveDate> = None;
//!   ui.add(Calendar::new(&mut selected));
//!
//! API (range):
//!   let mut range: Option<(chrono::NaiveDate, chrono::NaiveDate)> = None;
//!   ui.add(Calendar::range(&mut range));
//!
//! API (multi):
//!   let mut dates: Vec<chrono::NaiveDate> = vec![];
//!   ui.add(Calendar::multi(&mut dates));

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use egui::{Color32, CornerRadius, FontId, Id, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

use super::motion;
use super::placement::{Align, Placement, Side};
use super::popover::Popover;
use super::theme::ComponentTheme;
use super::tokens::Size;
use super::button::Button;
use crate::chart::renderer::ui::style as st;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CalendarMode {
    Single,
    Range,
    Multi,
}

enum CalendarValue<'a> {
    Single(&'a mut Option<NaiveDate>),
    Range(&'a mut Option<(NaiveDate, NaiveDate)>),
    Multi(&'a mut Vec<NaiveDate>),
}

#[must_use = "Calendar does nothing until `.show(ui, theme)` is called"]
pub struct Calendar<'a> {
    value: CalendarValue<'a>,
    mode: CalendarMode,
    size: Size,
    min_date: Option<NaiveDate>,
    max_date: Option<NaiveDate>,
    disabled_date: Option<Box<dyn Fn(NaiveDate) -> bool + 'a>>,
    week_starts_on: Weekday,
    show_week_numbers: bool,
    months_visible: usize,
    id_salt: Option<Id>,
}

pub struct CalendarResponse {
    pub response: Response,
    pub changed: bool,
    pub view_month: NaiveDate,
}

impl<'a> Calendar<'a> {
    pub fn new(value: &'a mut Option<NaiveDate>) -> Self {
        Self {
            value: CalendarValue::Single(value),
            mode: CalendarMode::Single,
            size: Size::Md,
            min_date: None,
            max_date: None,
            disabled_date: None,
            week_starts_on: Weekday::Sun,
            show_week_numbers: false,
            months_visible: 1,
            id_salt: None,
        }
    }

    pub fn range(value: &'a mut Option<(NaiveDate, NaiveDate)>) -> Self {
        Self {
            value: CalendarValue::Range(value),
            mode: CalendarMode::Range,
            size: Size::Md,
            min_date: None,
            max_date: None,
            disabled_date: None,
            week_starts_on: Weekday::Sun,
            show_week_numbers: false,
            months_visible: 1,
            id_salt: None,
        }
    }

    pub fn multi(value: &'a mut Vec<NaiveDate>) -> Self {
        Self {
            value: CalendarValue::Multi(value),
            mode: CalendarMode::Multi,
            size: Size::Md,
            min_date: None,
            max_date: None,
            disabled_date: None,
            week_starts_on: Weekday::Sun,
            show_week_numbers: false,
            months_visible: 1,
            id_salt: None,
        }
    }

    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn min_date(mut self, d: NaiveDate) -> Self { self.min_date = Some(d); self }
    pub fn max_date(mut self, d: NaiveDate) -> Self { self.max_date = Some(d); self }
    pub fn disabled_date(mut self, f: impl Fn(NaiveDate) -> bool + 'a) -> Self {
        self.disabled_date = Some(Box::new(f));
        self
    }
    pub fn week_starts_on(mut self, day: Weekday) -> Self { self.week_starts_on = day; self }
    pub fn show_week_numbers(mut self, v: bool) -> Self { self.show_week_numbers = v; self }
    pub fn months_visible(mut self, n: usize) -> Self {
        self.months_visible = n.clamp(1, 2);
        self
    }
    pub fn id_salt(mut self, salt: impl std::hash::Hash) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> CalendarResponse {
        paint_calendar(ui, theme, self)
    }
}

impl<'a> Widget for Calendar<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme).response
    }
}

// ─── State helpers ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default)]
struct CalState {
    /// First-of-month for the leftmost displayed month.
    view: Option<NaiveDate>,
    /// In-progress range pick (start chosen, waiting for end).
    pending_range_start: Option<NaiveDate>,
    /// Year-picker popover open?
    year_picker_open: bool,
}

fn load_state(ui: &Ui, id: Id) -> CalState {
    ui.ctx().memory(|m| m.data.get_temp::<CalState>(id)).unwrap_or_default()
}
fn save_state(ui: &Ui, id: Id, s: CalState) {
    ui.ctx().memory_mut(|m| m.data.insert_temp(id, s));
}

fn first_of_month(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d)
}

fn add_months(base: NaiveDate, delta: i32) -> NaiveDate {
    let mut y = base.year();
    let mut m = base.month() as i32 + delta;
    while m > 12 { y += 1; m -= 12; }
    while m < 1 { y -= 1; m += 12; }
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or(base)
}

const MONTH_NAMES: [&str; 12] = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
];

fn weekday_short(w: Weekday) -> &'static str {
    match w {
        Weekday::Sun => "Su",
        Weekday::Mon => "Mo",
        Weekday::Tue => "Tu",
        Weekday::Wed => "We",
        Weekday::Thu => "Th",
        Weekday::Fri => "Fr",
        Weekday::Sat => "Sa",
    }
}

/// 0..7 — day-of-week index relative to a chosen week start.
fn day_index(d: NaiveDate, week_start: Weekday) -> u32 {
    let ds = d.weekday().num_days_from_sunday();
    let ws = week_start.num_days_from_sunday();
    (ds + 7 - ws) % 7
}

// ─── Painting ─────────────────────────────────────────────────────────────────

fn paint_calendar<'a>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    cal: Calendar<'a>,
) -> CalendarResponse {
    let Calendar {
        mut value,
        mode,
        size,
        min_date,
        max_date,
        disabled_date,
        week_starts_on,
        show_week_numbers,
        months_visible,
        id_salt,
    } = cal;

    let cell_px: f32 = match size { Size::Xs | Size::Sm => 28.0, Size::Md => 32.0, Size::Lg => 36.0 };
    let header_font = match size { Size::Lg => st::font_md(), _ => st::font_sm() };
    let day_font = match size { Size::Xs | Size::Sm => st::font_xs(), _ => st::font_sm() };

    // Stable id for state persistence.
    let id = id_salt.unwrap_or_else(|| ui.id().with(("apex_calendar", mode as u8)));
    let mut state = load_state(ui, id);

    // Determine initial view month from current value if state has none.
    if state.view.is_none() {
        let initial = match &value {
            CalendarValue::Single(v) => v.unwrap_or_else(today),
            CalendarValue::Range(v) => v.map(|(s, _)| s).unwrap_or_else(today),
            CalendarValue::Multi(v) => v.first().copied().unwrap_or_else(today),
        };
        state.view = Some(first_of_month(initial));
    }
    let view_month = state.view.unwrap_or_else(|| first_of_month(today()));

    let n_months = months_visible.max(1);
    let week_col_extra = if show_week_numbers { cell_px * 0.85 } else { 0.0 };
    let single_grid_w = cell_px * 7.0 + week_col_extra;
    let inter_month_gap = st::gap_md();
    let total_w = single_grid_w * n_months as f32 + inter_month_gap * (n_months as f32 - 1.0);

    let mut changed = false;
    let mut pending_view: Option<NaiveDate> = None;

    let outer_resp = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = st::gap_2xs();

        // ── Header strip ──────────────────────────────────────────────────────
        ui.allocate_ui_with_layout(
            Vec2::new(total_w, size.height()),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                if Button::icon(super::super::icons::Icon::CARET_LEFT)
                    .size(Size::Sm)
                    .show(ui, theme)
                    .clicked()
                {
                    pending_view = Some(add_months(view_month, -1));
                }

                // Title fills the middle.
                let title_w = total_w - cell_px * 2.0 - st::gap_xs() * 2.0;
                let title = if n_months == 1 {
                    format!("{} {}", MONTH_NAMES[(view_month.month() - 1) as usize], view_month.year())
                } else {
                    let last = add_months(view_month, (n_months as i32) - 1);
                    format!(
                        "{} {} – {} {}",
                        MONTH_NAMES[(view_month.month() - 1) as usize], view_month.year(),
                        MONTH_NAMES[(last.month() - 1) as usize], last.year()
                    )
                };
                let (title_rect, title_resp) =
                    ui.allocate_exact_size(Vec2::new(title_w.max(80.0), size.height()), Sense::click());
                let hovered = title_resp.hovered();
                let hover_t = motion::ease_bool(ui.ctx(), id.with("title_hover"), hovered, motion::FAST);
                let bg_hover = st::color_alpha(theme.text(), st::ALPHA_GHOST);
                if hover_t > 0.001 {
                    ui.painter().rect_filled(
                        title_rect,
                        CornerRadius::same(3),
                        st::color_alpha(bg_hover, (st::ALPHA_GHOST as f32 * hover_t) as u8),
                    );
                }
                ui.painter().text(
                    title_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &title,
                    FontId::proportional(header_font),
                    theme.text(),
                );
                if title_resp.clicked() {
                    state.year_picker_open = !state.year_picker_open;
                }
                if hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Year picker popover anchored to title.
                if state.year_picker_open {
                    let pop_id = id.with("year_pop");
                    let mut open = true;
                    let cur_year = view_month.year();
                    let pop_result = Popover::new()
                        .open(&mut open)
                        .anchor(title_rect)
                        .placement(Placement {
                            side: Side::Bottom,
                            align: Align::Center,
                            offset: st::gap_2xs(),
                        })
                        .id(pop_id)
                        .show(ui, theme, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::splat(st::gap_2xs());
                            let mut picked_year: Option<i32> = None;
                            let mut picked_month: Option<u32> = None;
                            ui.label(
                                RichText::new("Year")
                                    .monospace().size(st::font_xs()).color(theme.dim()),
                            );
                            // 5x5 grid of years cur ±10
                            egui::Grid::new(pop_id.with("years"))
                                .num_columns(5)
                                .spacing(Vec2::splat(2.0))
                                .show(ui, |ui| {
                                    for (i, off) in (-10..=10).enumerate() {
                                        let y = cur_year + off;
                                        let is_cur = y == cur_year;
                                        let label = format!("{}", y);
                                        let resp = ui.add(
                                            egui::Button::new(
                                                RichText::new(label)
                                                    .monospace()
                                                    .size(st::font_xs())
                                                    .color(if is_cur { Color32::WHITE } else { theme.text() }),
                                            )
                                            .fill(if is_cur { theme.accent() } else { Color32::TRANSPARENT })
                                            .min_size(Vec2::new(40.0, 20.0)),
                                        );
                                        if resp.clicked() { picked_year = Some(y); }
                                        if (i + 1) % 5 == 0 { ui.end_row(); }
                                    }
                                });
                            ui.add_space(st::gap_2xs());
                            ui.label(
                                RichText::new("Month")
                                    .monospace().size(st::font_xs()).color(theme.dim()),
                            );
                            egui::Grid::new(pop_id.with("months"))
                                .num_columns(4)
                                .spacing(Vec2::splat(2.0))
                                .show(ui, |ui| {
                                    for m in 1u32..=12 {
                                        let is_cur = m == view_month.month();
                                        let resp = ui.add(
                                            egui::Button::new(
                                                RichText::new(&MONTH_NAMES[(m - 1) as usize][..3])
                                                    .monospace()
                                                    .size(st::font_xs())
                                                    .color(if is_cur { Color32::WHITE } else { theme.text() }),
                                            )
                                            .fill(if is_cur { theme.accent() } else { Color32::TRANSPARENT })
                                            .min_size(Vec2::new(40.0, 20.0)),
                                        );
                                        if resp.clicked() { picked_month = Some(m); }
                                        if m % 4 == 0 { ui.end_row(); }
                                    }
                                });
                            (picked_year, picked_month)
                        });
                    if let Some((picked_y, picked_m)) = pop_result {
                        let new_year = picked_y.unwrap_or(view_month.year());
                        let new_month = picked_m.unwrap_or(view_month.month());
                        if picked_y.is_some() || picked_m.is_some() {
                            pending_view = Some(
                                NaiveDate::from_ymd_opt(new_year, new_month, 1)
                                    .unwrap_or(view_month),
                            );
                            state.year_picker_open = false;
                        }
                    }
                    if !open { state.year_picker_open = false; }
                }

                if Button::icon(super::super::icons::Icon::CARET_RIGHT)
                    .size(Size::Sm)
                    .show(ui, theme)
                    .clicked()
                {
                    pending_view = Some(add_months(view_month, 1));
                }
            },
        );

        // ── Day-of-week header + grid (one or more months side by side) ──────
        ui.allocate_ui_with_layout(
            Vec2::new(total_w, cell_px * 7.0 + cell_px * 0.9),
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for mi in 0..n_months {
                    if mi > 0 {
                        ui.add_space(inter_month_gap);
                    }
                    let month = add_months(view_month, mi as i32);
                    paint_one_month(
                        ui,
                        theme,
                        id.with(("month", mi)),
                        month,
                        &mut value,
                        mode,
                        &mut state,
                        &min_date,
                        &max_date,
                        disabled_date.as_deref(),
                        week_starts_on,
                        show_week_numbers,
                        cell_px,
                        day_font,
                        &mut changed,
                    );
                }
            },
        );
    });

    // Apply pending view month change after grid render.
    if let Some(pv) = pending_view {
        state.view = Some(first_of_month(pv));
    }

    save_state(ui, id, state);

    CalendarResponse {
        response: outer_resp.response,
        changed,
        view_month,
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_one_month(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    grid_id: Id,
    month_first: NaiveDate,
    value: &mut CalendarValue,
    mode: CalendarMode,
    state: &mut CalState,
    min_date: &Option<NaiveDate>,
    max_date: &Option<NaiveDate>,
    disabled_fn: Option<&dyn Fn(NaiveDate) -> bool>,
    week_start: Weekday,
    show_week_numbers: bool,
    cell_px: f32,
    day_font: f32,
    changed: &mut bool,
) {
    let week_col_extra = if show_week_numbers { cell_px * 0.85 } else { 0.0 };
    let month_w = cell_px * 7.0 + week_col_extra;
    let total_h = cell_px * 7.0; // 1 row header + 6 grid rows

    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(month_w, total_h), Sense::hover());
    let painter = ui.painter_at(rect);

    // Day-of-week header row.
    let header_y = rect.top() + cell_px * 0.5;
    let mut day_x = rect.left() + week_col_extra + cell_px * 0.5;
    let mut wd = week_start;
    for _ in 0..7 {
        painter.text(
            Pos2::new(day_x, header_y),
            egui::Align2::CENTER_CENTER,
            weekday_short(wd),
            FontId::monospace(st::font_xs()),
            theme.dim(),
        );
        day_x += cell_px;
        wd = wd.succ();
    }
    if show_week_numbers {
        painter.text(
            Pos2::new(rect.left() + week_col_extra * 0.5, header_y),
            egui::Align2::CENTER_CENTER,
            "Wk",
            FontId::monospace(st::font_xs()),
            theme.dim(),
        );
    }

    // First displayed cell = first-of-month minus its day_index.
    let leading = day_index(month_first, week_start) as i64;
    let first_cell = month_first - Duration::days(leading);

    let today_d = today();
    let hover_pos = ui.ctx().pointer_interact_pos();

    for row in 0..6 {
        let row_top = rect.top() + cell_px + row as f32 * cell_px;
        // Optional week-number column.
        if show_week_numbers {
            let center = Pos2::new(rect.left() + week_col_extra * 0.5, row_top + cell_px * 0.5);
            let week_anchor = first_cell + Duration::days(row as i64 * 7);
            let wk = week_anchor.iso_week().week();
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                format!("{}", wk),
                FontId::monospace(st::font_xs()),
                theme.dim(),
            );
        }

        for col in 0..7 {
            let day_idx = row * 7 + col;
            let d = first_cell + Duration::days(day_idx);
            let cell_min = Pos2::new(
                rect.left() + week_col_extra + col as f32 * cell_px,
                row_top,
            );
            let cell_rect = Rect::from_min_size(cell_min, Vec2::splat(cell_px));

            let in_month = d.month() == month_first.month() && d.year() == month_first.year();
            let mut disabled = false;
            if let Some(min) = min_date { if d < *min { disabled = true; } }
            if let Some(max) = max_date { if d > *max { disabled = true; } }
            if let Some(f) = disabled_fn { if f(d) { disabled = true; } }

            let is_today = d == today_d;
            let (sel_state, in_range_mid) = selection_state(&*value, mode, state, d);

            // Hover & click interaction.
            let cell_id = grid_id.with(("c", day_idx));
            let resp = ui.interact(cell_rect, cell_id, Sense::click());
            let hovered = resp.hovered() && !disabled;
            if hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let hover_t = motion::ease_bool(ui.ctx(), cell_id.with("h"), hovered, motion::FAST);

            // Range hover preview: if Range mode + pending start exists + this date >= start
            // and pointer is in this calendar grid area covering between start and hover.
            let mut preview_mid = false;
            if mode == CalendarMode::Range {
                if let Some(start) = state.pending_range_start {
                    if let Some(p) = hover_pos {
                        if rect.contains(p) {
                            // figure out which cell pointer is on
                            let col_rel = ((p.x - (rect.left() + week_col_extra)) / cell_px).floor() as i64;
                            let row_rel = ((p.y - (rect.top() + cell_px)) / cell_px).floor() as i64;
                            if (0..7).contains(&col_rel) && (0..6).contains(&row_rel) {
                                let hovered_d = first_cell + Duration::days(row_rel * 7 + col_rel);
                                let (lo, hi) = if hovered_d >= start { (start, hovered_d) } else { (hovered_d, start) };
                                if d >= lo && d <= hi && d != start && d != hovered_d {
                                    preview_mid = true;
                                }
                            }
                        }
                    }
                }
            }

            // Paint cell background.
            let radius = CornerRadius::same(3);
            let accent = theme.accent();
            let mid_bg = st::color_alpha(accent, st::ALPHA_GHOST);
            let preview_bg = st::color_alpha(accent, st::ALPHA_FAINT);

            match sel_state {
                CellSel::Selected | CellSel::RangeStart | CellSel::RangeEnd => {
                    painter.rect_filled(cell_rect.shrink(2.0), radius, accent);
                }
                CellSel::None => {
                    if in_range_mid {
                        painter.rect_filled(cell_rect.shrink(2.0), radius, mid_bg);
                    } else if preview_mid {
                        painter.rect_filled(cell_rect.shrink(2.0), radius, preview_bg);
                    } else if hover_t > 0.001 && !disabled {
                        let base = st::color_alpha(theme.text(), 18);
                        painter.rect_filled(
                            cell_rect.shrink(2.0),
                            radius,
                            st::color_alpha(base, (18.0 * hover_t) as u8),
                        );
                    }
                }
            }

            // Today marker — accent border at 2px when not selected.
            if is_today && matches!(sel_state, CellSel::None) {
                painter.rect_stroke(
                    cell_rect.shrink(2.0),
                    radius,
                    Stroke::new(1.5, accent),
                    StrokeKind::Inside,
                );
            }

            // Number text.
            let text_col = match sel_state {
                CellSel::Selected | CellSel::RangeStart | CellSel::RangeEnd => Color32::WHITE,
                CellSel::None => {
                    if !in_month {
                        st::color_alpha(theme.dim(), 80)
                    } else if disabled {
                        st::color_alpha(theme.text(), 80)
                    } else {
                        theme.text()
                    }
                }
            };
            let text = format!("{}", d.day());
            let font = if is_today {
                FontId::new(day_font, egui::FontFamily::Monospace)
            } else {
                FontId::monospace(day_font)
            };
            painter.text(
                cell_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font,
                text_col,
            );

            // Click handling.
            if resp.clicked() && !disabled {
                apply_click(&mut *value, mode, state, d, changed);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CellSel {
    None,
    Selected,
    RangeStart,
    RangeEnd,
}

fn selection_state(
    value: &CalendarValue,
    _mode: CalendarMode,
    state: &CalState,
    d: NaiveDate,
) -> (CellSel, bool) {
    match value {
        CalendarValue::Single(v) => {
            if let Some(sel) = **v { if sel == d { return (CellSel::Selected, false); } }
            (CellSel::None, false)
        }
        CalendarValue::Multi(v) => {
            if v.iter().any(|x| *x == d) { (CellSel::Selected, false) } else { (CellSel::None, false) }
        }
        CalendarValue::Range(v) => {
            if let Some((s, e)) = **v {
                if d == s { return (CellSel::RangeStart, false); }
                if d == e { return (CellSel::RangeEnd, false); }
                if d > s && d < e { return (CellSel::None, true); }
            }
            if let Some(s) = state.pending_range_start {
                if d == s { return (CellSel::RangeStart, false); }
            }
            (CellSel::None, false)
        }
    }
}

fn apply_click(
    value: &mut CalendarValue,
    _mode: CalendarMode,
    state: &mut CalState,
    d: NaiveDate,
    changed: &mut bool,
) {
    match value {
        CalendarValue::Single(v) => {
            **v = Some(d);
            *changed = true;
        }
        CalendarValue::Multi(v) => {
            if let Some(pos) = v.iter().position(|x| *x == d) {
                v.remove(pos);
            } else {
                v.push(d);
                v.sort();
            }
            *changed = true;
        }
        CalendarValue::Range(v) => {
            match state.pending_range_start {
                None => {
                    state.pending_range_start = Some(d);
                    **v = None;
                }
                Some(start) => {
                    let (lo, hi) = if d >= start { (start, d) } else { (d, start) };
                    **v = Some((lo, hi));
                    state.pending_range_start = None;
                    *changed = true;
                }
            }
        }
    }
}

/// Best-effort "today" — chrono::Local may not be available in some sandboxes.
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
