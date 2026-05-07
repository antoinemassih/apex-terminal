//! Select / Dropdown / Combobox.
//!
//! Three modes:
//!   - single: pick one of N options
//!   - multi: pick a subset
//!   - searchable: single or multi with a filter input
//!
//! API:
//!   let mut tif: usize = 0;
//!   Select::new(&mut tif, &["DAY", "GTC", "IOC"]).show(ui, theme);
//!
//!   let mut symbols: Vec<usize> = vec![];
//!   Select::multi(&mut symbols, &available_symbols)
//!       .searchable(true)
//!       .placeholder("Pick symbols...")
//!       .show(ui, theme);
//!
//!   // With rich item rendering:
//!   Select::new_with(&mut sel_idx, sectors, |sec| sec.display_name.clone())
//!       .item_render(|ui, theme, sec, selected| { /* ... */ })
//!       .show(ui, theme);

use egui::{
    Color32, CornerRadius, FontId, Id, Key, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

use super::motion;
use super::placement::{Align as PAlign, Placement, Side};
use super::popover::Popover;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

/// Selection mode + storage.
enum Mode<'a> {
    Single(&'a mut usize),
    Multi(&'a mut Vec<usize>),
}

/// Display strategy for an option at index `i`.
enum Display<'a, T> {
    Strs(&'a [&'a str]),
    Custom {
        items: &'a [T],
        to_string: Box<dyn Fn(&T) -> String + 'a>,
    },
}

impl<'a, T> Display<'a, T> {
    fn len(&self) -> usize {
        match self {
            Display::Strs(s) => s.len(),
            Display::Custom { items, .. } => items.len(),
        }
    }

    fn label(&self, i: usize) -> String {
        match self {
            Display::Strs(s) => s[i].to_string(),
            Display::Custom { items, to_string } => to_string(&items[i]),
        }
    }
}

type ItemRenderFn<'a, T> = Box<dyn Fn(&mut Ui, &dyn ComponentTheme, &T, bool) + 'a>;

#[must_use = "Select does nothing until `.show(ui, theme)` is called"]
pub struct Select<'a, T> {
    mode: Mode<'a>,
    display: Display<'a, T>,
    placeholder: Option<String>,
    searchable: bool,
    size: Size,
    full_width: bool,
    min_width: Option<f32>,
    disabled: bool,
    invalid: bool,
    item_render: Option<ItemRenderFn<'a, T>>,
    empty_state: Option<String>,
}

pub struct SelectResponse {
    pub response: Response,
    pub changed: bool,
    pub opened: bool,
}

// ─── Constructors ─────────────────────────────────────────────────────────

impl<'a> Select<'a, &'a str> {
    pub fn new(value: &'a mut usize, options: &'a [&'a str]) -> Select<'a, &'a str> {
        Select {
            mode: Mode::Single(value),
            display: Display::Strs(options),
            placeholder: None,
            searchable: false,
            size: Size::Md,
            full_width: false,
            min_width: None,
            disabled: false,
            invalid: false,
            item_render: None,
            empty_state: None,
        }
    }

    pub fn multi(value: &'a mut Vec<usize>, options: &'a [&'a str]) -> Select<'a, &'a str> {
        Select {
            mode: Mode::Multi(value),
            display: Display::Strs(options),
            placeholder: None,
            searchable: false,
            size: Size::Md,
            full_width: false,
            min_width: None,
            disabled: false,
            invalid: false,
            item_render: None,
            empty_state: None,
        }
    }
}

impl<'a, T: 'a> Select<'a, T> {
    pub fn new_with<F>(value: &'a mut usize, options: &'a [T], display: F) -> Self
    where
        F: Fn(&T) -> String + 'a,
    {
        Self {
            mode: Mode::Single(value),
            display: Display::Custom {
                items: options,
                to_string: Box::new(display),
            },
            placeholder: None,
            searchable: false,
            size: Size::Md,
            full_width: false,
            min_width: None,
            disabled: false,
            invalid: false,
            item_render: None,
            empty_state: None,
        }
    }

    pub fn multi_with<F>(value: &'a mut Vec<usize>, options: &'a [T], display: F) -> Self
    where
        F: Fn(&T) -> String + 'a,
    {
        Self {
            mode: Mode::Multi(value),
            display: Display::Custom {
                items: options,
                to_string: Box::new(display),
            },
            placeholder: None,
            searchable: false,
            size: Size::Md,
            full_width: false,
            min_width: None,
            disabled: false,
            invalid: false,
            item_render: None,
            empty_state: None,
        }
    }

    pub fn placeholder(mut self, hint: impl Into<String>) -> Self {
        self.placeholder = Some(hint.into());
        self
    }
    pub fn searchable(mut self, v: bool) -> Self {
        self.searchable = v;
        self
    }
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }
    pub fn min_width(mut self, px: f32) -> Self {
        self.min_width = Some(px);
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn invalid(mut self, v: bool) -> Self {
        self.invalid = v;
        self
    }
    pub fn item_render(
        mut self,
        f: impl Fn(&mut Ui, &dyn ComponentTheme, &T, bool) + 'a,
    ) -> Self {
        self.item_render = Some(Box::new(f));
        self
    }
    pub fn empty_state(mut self, text: impl Into<String>) -> Self {
        self.empty_state = Some(text.into());
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> SelectResponse {
        paint_select(ui, theme, self)
    }
}

// ─── Per-widget memory ────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct SelectMem {
    open: bool,
    filter: String,
    highlight: usize,
    type_buf: String,
    type_last: f64,
}

fn load_mem(ui: &Ui, id: Id) -> SelectMem {
    ui.memory(|m| m.data.get_temp::<SelectMem>(id).unwrap_or_default())
}

fn save_mem(ui: &Ui, id: Id, mem: SelectMem) {
    ui.memory_mut(|m| m.data.insert_temp(id, mem));
}

fn current_first_index(mode: &Mode<'_>) -> usize {
    match mode {
        Mode::Single(v) => **v,
        Mode::Multi(v) => v.first().copied().unwrap_or(0),
    }
}

fn is_selected(mode: &Mode<'_>, idx: usize) -> bool {
    match mode {
        Mode::Single(v) => **v == idx,
        Mode::Multi(v) => v.contains(&idx),
    }
}

/// Apply a click. Returns (changed, should_close_dropdown).
fn apply_click(mode: &mut Mode<'_>, idx: usize) -> (bool, bool) {
    match mode {
        Mode::Single(v) => {
            if **v != idx {
                **v = idx;
                (true, true)
            } else {
                (false, true)
            }
        }
        Mode::Multi(v) => {
            if let Some(p) = v.iter().position(|&x| x == idx) {
                v.remove(p);
            } else {
                v.push(idx);
            }
            (true, false)
        }
    }
}

fn matches_filter(label: &str, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    label.to_lowercase().contains(&filter.to_lowercase())
}

fn filtered_indices<T>(display: &Display<'_, T>, filter: &str) -> Vec<usize> {
    (0..display.len())
        .filter(|&i| matches_filter(&display.label(i), filter))
        .collect()
}

// ─── Painter ──────────────────────────────────────────────────────────────

fn paint_select<'a, T: 'a>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    sel: Select<'a, T>,
) -> SelectResponse {
    let Select {
        mut mode,
        display,
        placeholder,
        searchable,
        size,
        full_width,
        min_width,
        disabled,
        invalid,
        item_render,
        empty_state,
    } = sel;

    let h = size.height();
    let pad_x = size.padding_x();
    let font_size = size.font_size();
    let icon_gap = st::gap_2xs();

    // Measure the widest label once (used for both trigger and popup widths).
    let label_font = FontId::proportional(font_size);
    let mut widest_label: f32 = 0.0;
    for i in 0..display.len() {
        let label = display.label(i);
        let g = ui.fonts(|f| f.layout_no_wrap(label, label_font.clone(), Color32::WHITE));
        widest_label = widest_label.max(g.rect.width());
    }
    if let Some(ph) = &placeholder {
        let g = ui.fonts(|f| f.layout_no_wrap(ph.clone(), label_font.clone(), Color32::WHITE));
        widest_label = widest_label.max(g.rect.width());
    }
    let caret_w = font_size * 0.6;
    // gap_sm (~8px) extra breathing room between the label and the caret so
    // the trigger doesn't feel cramped at content-width.
    let trigger_extra_pad = st::gap_sm();

    // Default trigger width fits the longest option (label + caret + padding +
    // breathing room). `full_width()` stretches to available width;
    // `min_width(px)` acts as a floor.
    let desired_w = if full_width {
        ui.available_width()
    } else {
        let natural = pad_x * 2.0 + widest_label + icon_gap + caret_w + trigger_extra_pad;
        let floor = min_width.unwrap_or(0.0);
        natural.max(floor)
    };

    let row_size = Vec2::new(desired_w, h);
    let (rect, mut response) = ui.allocate_exact_size(row_size, Sense::click());
    let id = response.id;

    let mut mem = load_mem(ui, id);
    let mut changed = false;
    let mut opened_this_frame = false;

    // ─── Toggle on click ──
    if response.clicked() && !disabled {
        mem.open = !mem.open;
        if mem.open {
            opened_this_frame = true;
            mem.filter.clear();
            mem.highlight = current_first_index(&mode);
        }
    }

    let hovered = response.hovered() && !disabled;
    let hover_t = motion::ease_bool(ui.ctx(), id.with("hover"), hovered, motion::FAST);
    let open_t = motion::ease_bool(ui.ctx(), id.with("open"), mem.open, motion::FAST);

    // ─── Border + bg ──
    let border_idle = theme.border();
    let border_hover = theme.dim();
    let border_focus = theme.accent();
    let mut border_col = motion::lerp_color(border_idle, border_hover, hover_t);
    border_col = motion::lerp_color(border_col, border_focus, open_t);
    if invalid {
        border_col = theme.bear();
    }
    let bg_fill = if disabled {
        st::color_alpha(theme.surface(), 128)
    } else {
        theme.surface()
    };

    let radius = CornerRadius::same(4);

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, radius, bg_fill);
        painter.rect_stroke(rect, radius, Stroke::new(1.0, border_col), StrokeKind::Inside);
    }

    // ─── Trigger content ──
    let cy = rect.center().y;
    let mut left_x = rect.left() + pad_x;
    let right_edge = rect.right() - pad_x;

    // Down/up chevron.
    let chev_size = font_size * 1.0;
    let chev_center = Pos2::new(right_edge - chev_size * 0.5, cy);
    let chev_color = motion::lerp_color(theme.dim(), theme.accent(), open_t);
    {
        let painter = ui.painter_at(rect);
        let glyph = if open_t > 0.5 { "\u{25B2}" } else { "\u{25BC}" };
        painter.text(
            chev_center,
            egui::Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(chev_size * 0.8),
            chev_color,
        );
    }
    let content_right = chev_center.x - chev_size - icon_gap;

    // Render trigger label / multi tags / placeholder.
    let n = display.len();
    let muted_ph = st::color_alpha(theme.dim(), 160);
    let text_col = if disabled {
        st::color_alpha(theme.text(), 128)
    } else {
        theme.text()
    };

    // Multi-mode chip removals: we collect indices to remove, then mutate after.
    let mut multi_remove: Vec<usize> = Vec::new();

    match &mode {
        Mode::Single(v) => {
            let painter = ui.painter_at(rect);
            let label_pos = Pos2::new(left_x, cy);
            if **v < n {
                let label = display.label(**v);
                painter.text(
                    label_pos,
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::monospace(font_size),
                    text_col,
                );
            } else if let Some(ph) = &placeholder {
                painter.text(
                    label_pos,
                    egui::Align2::LEFT_CENTER,
                    ph,
                    FontId::monospace(font_size),
                    muted_ph,
                );
            }
        }
        Mode::Multi(v) => {
            if v.is_empty() {
                if let Some(ph) = &placeholder {
                    let painter = ui.painter_at(rect);
                    painter.text(
                        Pos2::new(left_x, cy),
                        egui::Align2::LEFT_CENTER,
                        ph,
                        FontId::monospace(font_size),
                        muted_ph,
                    );
                }
            } else {
                let max_inline = if v.len() > 3 { 2 } else { v.len() };
                for (slot, &idx) in v.iter().enumerate().take(max_inline) {
                    if idx >= n {
                        continue;
                    }
                    let label = display.label(idx);
                    let chip_id = id.with(("chip", slot, idx));
                    if let Some((w, removed)) = chip_paint(
                        ui,
                        theme,
                        chip_id,
                        &label,
                        left_x,
                        cy,
                        font_size,
                        content_right,
                    ) {
                        if removed {
                            multi_remove.push(idx);
                        }
                        left_x += w + icon_gap;
                    } else {
                        break;
                    }
                }
                if v.len() > max_inline {
                    let extra = format!("+{} more", v.len() - max_inline);
                    let painter = ui.painter_at(rect);
                    painter.text(
                        Pos2::new(left_x, cy),
                        egui::Align2::LEFT_CENTER,
                        extra,
                        FontId::monospace(font_size - 1.0),
                        theme.dim(),
                    );
                }
            }
        }
    }

    // Apply chip removals.
    if !multi_remove.is_empty() {
        if let Mode::Multi(v) = &mut mode {
            for idx in multi_remove {
                if let Some(p) = v.iter().position(|&x| x == idx) {
                    v.remove(p);
                    changed = true;
                }
            }
        }
    }

    // ─── Popover panel ──
    // Popup width = widest label + per-row padding + a gap_sm cushion on each
    // side. Floor at the trigger's width so the popup is never narrower than
    // the trigger it dropped from. The old hard `.max(200.0)` floor created a
    // visibly over-wide popup for compact dropdowns (Solid/Dashed/Dotted etc.).
    let popup_pad = st::gap_sm();
    let panel_w = (widest_label + popup_pad * 2.0 + caret_w).max(desired_w);
    let mut click_idx: Option<usize> = None;
    if mem.open {
        click_idx = render_panel(
            ui,
            theme,
            id,
            rect,
            panel_w,
            searchable,
            &display,
            &mode,
            &item_render,
            empty_state.as_deref(),
            &mut mem,
        );
    }

    // ─── Type-ahead (non-searchable, when open) ──
    if mem.open && !searchable {
        type_ahead(ui, &mut mem, &display);
    }

    // ─── Keyboard navigation when open ──
    if mem.open {
        let kb = keyboard_nav(ui, &mut mem, &display);
        if let KbAction::Commit(i) = kb {
            click_idx = Some(i);
        } else if let KbAction::Close = kb {
            mem.open = false;
        }
    }

    // Apply panel/keyboard click.
    if let Some(i) = click_idx {
        let (ch, close) = apply_click(&mut mode, i);
        if ch {
            changed = true;
        }
        if close {
            mem.open = false;
        }
    }

    if changed {
        response.mark_changed();
    }

    save_mem(ui, id, mem);

    SelectResponse {
        response,
        changed,
        opened: opened_this_frame,
    }
}

fn chip_paint(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    chip_id: Id,
    label: &str,
    left_x: f32,
    cy: f32,
    font_size: f32,
    right_limit: f32,
) -> Option<(f32, bool)> {
    let pad = st::gap_2xs();
    let close_size = font_size * 0.85;
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            label.to_string(),
            FontId::monospace(font_size - 1.0),
            theme.text(),
        )
    });
    let label_w = galley.rect.width();
    let chip_w = pad + label_w + pad + close_size + pad;
    let chip_h = font_size + 4.0;

    if left_x + chip_w > right_limit {
        return None;
    }

    let chip_rect = Rect::from_min_size(
        Pos2::new(left_x, cy - chip_h * 0.5),
        Vec2::new(chip_w, chip_h),
    );
    let painter = ui.painter_at(chip_rect);
    painter.rect_filled(
        chip_rect,
        CornerRadius::same(3),
        st::color_alpha(theme.accent(), st::ALPHA_GHOST + 10),
    );
    painter.text(
        Pos2::new(chip_rect.left() + pad, cy),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::monospace(font_size - 1.0),
        theme.text(),
    );

    let close_center = Pos2::new(chip_rect.right() - pad - close_size * 0.5, cy);
    let close_hit = Rect::from_center_size(close_center, Vec2::splat(close_size + 4.0));
    let close_resp = ui.interact(close_hit, chip_id, Sense::click());
    let col = if close_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        theme.text()
    } else {
        theme.dim()
    };
    painter.text(
        close_center,
        egui::Align2::CENTER_CENTER,
        Icon::X,
        FontId::proportional(close_size),
        col,
    );

    Some((chip_w, close_resp.clicked()))
}

enum KbAction {
    None,
    Commit(usize),
    Close,
}

fn type_ahead<T>(ui: &Ui, mem: &mut SelectMem, display: &Display<'_, T>) {
    let now = ui.ctx().input(|i| i.time);
    let mut typed = String::new();
    ui.ctx().input(|i| {
        for ev in &i.events {
            if let egui::Event::Text(t) = ev {
                typed.push_str(t);
            }
        }
    });
    if typed.is_empty() {
        return;
    }
    if now - mem.type_last > 0.5 {
        mem.type_buf.clear();
    }
    mem.type_buf.push_str(&typed);
    mem.type_last = now;
    let needle = mem.type_buf.to_lowercase();
    for i in 0..display.len() {
        if display.label(i).to_lowercase().starts_with(&needle) {
            mem.highlight = i;
            break;
        }
    }
}

fn keyboard_nav<T>(ui: &Ui, mem: &mut SelectMem, display: &Display<'_, T>) -> KbAction {
    let visible: Vec<usize> = filtered_indices(display, &mem.filter);
    if visible.is_empty() {
        return KbAction::None;
    }
    if !visible.contains(&mem.highlight) {
        mem.highlight = visible[0];
    }
    let pos = visible
        .iter()
        .position(|&i| i == mem.highlight)
        .unwrap_or(0);

    let down = ui.ctx().input(|i| i.key_pressed(Key::ArrowDown));
    let up = ui.ctx().input(|i| i.key_pressed(Key::ArrowUp));
    let enter = ui.ctx().input(|i| i.key_pressed(Key::Enter));
    let tab = ui.ctx().input(|i| i.key_pressed(Key::Tab));

    if down {
        let np = (pos + 1) % visible.len();
        mem.highlight = visible[np];
    }
    if up {
        let np = if pos == 0 { visible.len() - 1 } else { pos - 1 };
        mem.highlight = visible[np];
    }
    if enter {
        return KbAction::Commit(mem.highlight);
    }
    if tab {
        return KbAction::Close;
    }
    KbAction::None
}

#[allow(clippy::too_many_arguments)]
fn render_panel<'a, T>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    id: Id,
    anchor: Rect,
    width: f32,
    searchable: bool,
    display: &Display<'_, T>,
    mode: &Mode<'a>,
    item_render: &Option<ItemRenderFn<'a, T>>,
    empty_state: Option<&str>,
    mem: &mut SelectMem,
) -> Option<usize> {
    let placement = Placement {
        side: Side::Bottom,
        align: PAlign::Start,
        offset: st::gap_2xs(),
    };

    let mut open = mem.open;
    let popover_id = id.with("pop");

    // We need to extract a click index out of the popover closure.
    let mut clicked: Option<usize> = None;

    Popover::new()
        .open(&mut open)
        .anchor(anchor)
        .placement(placement)
        .id(popover_id)
        .show(ui, theme, |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width.max(280.0));

            // Search row.
            if searchable {
                let search_h = 24.0;
                let (s_rect, s_resp) =
                    ui.allocate_exact_size(Vec2::new(width, search_h), Sense::click());
                let painter = ui.painter_at(s_rect);
                painter.rect_filled(
                    s_rect,
                    CornerRadius::same(3),
                    st::color_alpha(theme.bg(), 200),
                );
                painter.rect_stroke(
                    s_rect,
                    CornerRadius::same(3),
                    Stroke::new(1.0, st::color_alpha(theme.border(), st::alpha_line())),
                    StrokeKind::Inside,
                );
                let cy = s_rect.center().y;
                let pad = st::gap_xs();
                painter.text(
                    Pos2::new(s_rect.left() + pad, cy),
                    egui::Align2::LEFT_CENTER,
                    Icon::MAGNIFYING_GLASS,
                    FontId::proportional(13.0),
                    theme.dim(),
                );
                let edit_id = id.with("filter_edit");
                let edit_left = s_rect.left() + pad + 16.0;
                let edit_rect = Rect::from_min_max(
                    Pos2::new(edit_left, s_rect.top() + 1.0),
                    Pos2::new(s_rect.right() - pad, s_rect.bottom() - 1.0),
                );
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(edit_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                child.spacing_mut().item_spacing = Vec2::ZERO;
                let te = egui::TextEdit::singleline(&mut mem.filter)
                    .id(edit_id)
                    .desired_width(edit_rect.width())
                    .margin(egui::Margin::ZERO)
                    .frame(false)
                    .text_color(theme.text())
                    .font(egui::FontSelection::FontId(FontId::monospace(12.0)));
                let _ = child.add(te);
                if s_resp.clicked() {
                    ui.memory_mut(|m| m.request_focus(edit_id));
                }
                if !ui.memory(|m| m.has_focus(edit_id)) && mem.filter.is_empty() {
                    ui.memory_mut(|m| m.request_focus(edit_id));
                }
                ui.add_space(st::gap_2xs());
            }

            // Filtered list.
            let visible: Vec<usize> = filtered_indices(display, &mem.filter);

            if visible.is_empty() {
                let empty_text = empty_state.unwrap_or("No matches");
                ui.add_space(st::gap_xs());
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(empty_text)
                            .monospace()
                            .size(st::font_xs())
                            .color(theme.dim()),
                    );
                });
                ui.add_space(st::gap_xs());
                return;
            }

            let max_panel_h = 320.0;
            egui::ScrollArea::vertical()
                .max_height(max_panel_h)
                .show(ui, |ui| {
                    for &i in &visible {
                        let selected = is_selected(mode, i);
                        let highlighted = i == mem.highlight;
                        if render_row(
                            ui,
                            theme,
                            id.with(("row", i)),
                            display,
                            i,
                            selected,
                            highlighted,
                            mode,
                            item_render,
                            width,
                        ) {
                            clicked = Some(i);
                        }
                    }
                });
        });

    mem.open = open;
    clicked
}

#[allow(clippy::too_many_arguments)]
fn render_row<'a, T>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    row_id: Id,
    display: &Display<'_, T>,
    idx: usize,
    selected: bool,
    highlighted: bool,
    mode: &Mode<'a>,
    item_render: &Option<ItemRenderFn<'a, T>>,
    width: f32,
) -> bool {
    let h = 28.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, h), Sense::click());
    let hovered = resp.hovered();

    let hover_t = motion::ease_bool(
        ui.ctx(),
        row_id.with("h"),
        hovered || highlighted,
        motion::FAST,
    );
    let sel_t = motion::ease_bool(ui.ctx(), row_id.with("s"), selected, motion::FAST);

    let painter = ui.painter_at(rect);
    let bg_hover = st::color_alpha(theme.text(), 18);
    let bg_sel = st::color_alpha(theme.accent(), st::ALPHA_GHOST);
    let mut bg = motion::lerp_color(Color32::TRANSPARENT, bg_hover, hover_t);
    bg = motion::lerp_color(bg, bg_sel, sel_t);
    painter.rect_filled(rect, CornerRadius::same(3), bg);

    let cy = rect.center().y;
    let pad = st::gap_xs();
    let mut left_x = rect.left() + pad;
    let right_x = rect.right() - pad;
    let font_size = 12.0;

    if matches!(mode, Mode::Multi(_)) {
        let bs = 12.0;
        let bx = Rect::from_min_size(Pos2::new(left_x, cy - bs * 0.5), Vec2::splat(bs));
        let border = motion::lerp_color(theme.border(), theme.accent(), sel_t);
        let fill = motion::lerp_color(Color32::TRANSPARENT, theme.accent(), sel_t);
        painter.rect_filled(bx, CornerRadius::same(2), fill);
        painter.rect_stroke(
            bx,
            CornerRadius::same(2),
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
        if selected {
            let p1 = Pos2::new(bx.center().x - bs * 0.25, bx.center().y + bs * 0.02);
            let p2 = Pos2::new(bx.center().x - bs * 0.05, bx.center().y + bs * 0.20);
            let p3 = Pos2::new(bx.center().x + bs * 0.28, bx.center().y - bs * 0.18);
            let s = Stroke::new(1.4, Color32::WHITE);
            painter.line_segment([p1, p2], s);
            painter.line_segment([p2, p3], s);
        }
        left_x += bs + st::gap_2xs();
    }

    if let (Some(render), Display::Custom { items, .. }) = (item_render.as_ref(), display) {
        let inner_rect =
            Rect::from_min_max(Pos2::new(left_x, rect.top()), Pos2::new(right_x, rect.bottom()));
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        render(&mut child, theme, &items[idx], selected);
    } else {
        let label = display.label(idx);
        painter.text(
            Pos2::new(left_x, cy),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::monospace(font_size),
            theme.text(),
        );
    }

    if matches!(mode, Mode::Single(_)) && selected {
        painter.text(
            Pos2::new(right_x, cy),
            egui::Align2::RIGHT_CENTER,
            Icon::CHECK,
            FontId::proportional(font_size * 1.1),
            theme.accent(),
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    resp.clicked()
}
