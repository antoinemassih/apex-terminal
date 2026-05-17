//! ToggleRow — settings row primitive: label / description / switch.
//!
//! Zed's standard settings row pattern. Used for any boolean toggle
//! in a settings list. Pairs bold label + muted description on the
//! left with a Switch on the right.
//!
//! API:
//!   ui.add(ToggleRow::new(&mut wl.compact_mode)
//!       .label("Compact mode")
//!       .description("Trade-off vertical density for visual breathing room.")
//!   );
//!
//!   // Without description (just label + switch):
//!   ui.add(ToggleRow::new(&mut wl.broadcast_mode).label("Broadcast mode"));
//!
//!   // With info icon:
//!   ToggleRow::new(&mut wl.trust_all)
//!     .label("Trust All Projects By Default")
//!     .info(|ui, theme| { /* tooltip body */ })
//!     .show(ui, theme);

use egui::{Color32, FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use super::tokens::Size;
use super::{Switch, Tooltip};
use crate::chart::renderer::ui::style as st;

type InfoFn<'a> = Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>;

#[must_use = "ToggleRow does nothing until `.show(ui, theme)` or `ui.add(row)` is called"]
pub struct ToggleRow<'a> {
    value: &'a mut bool,
    label: String,
    description: Option<String>,
    info: Option<InfoFn<'a>>,
    disabled: bool,
}

impl<'a> ToggleRow<'a> {
    pub fn new(value: &'a mut bool) -> Self {
        Self {
            value,
            label: String::new(),
            description: None,
            info: None,
            disabled: false,
        }
    }

    pub fn label(mut self, text: impl Into<String>) -> Self {
        self.label = text.into();
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn info(mut self, f: impl FnOnce(&mut Ui, &dyn ComponentTheme) + 'a) -> Self {
        self.info = Some(Box::new(f));
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        paint_toggle_row(ui, theme, self)
    }
}

impl<'a> Widget for ToggleRow<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, theme)
    }
}

fn paint_toggle_row<'a>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    row: ToggleRow<'a>,
) -> Response {
    let ToggleRow {
        value,
        label,
        description,
        info,
        disabled,
    } = row;

    let pad_v = st::gap_sm();
    let gap_label_desc = st::gap_2xs();
    let gap_label_info = st::gap_xs();

    // Switch dims (Md): 32 wide, 20 tall — match switch.rs `track_dims`.
    let switch_w = 32.0_f32;
    let switch_h = 20.0_f32;

    let avail_w = ui.available_width();
    // Reserve right side for switch + a small gap.
    let right_reserve = switch_w + st::gap_sm();
    let left_max_w = (avail_w - right_reserve).max(80.0);

    // Lay out left side text to measure heights.
    let label_font = FontId::new(st::font_sm(), egui::FontFamily::Name("inter_semibold".into()));
    let desc_font = FontId::proportional(st::font_xs());

    let label_galley = ui.fonts(|f| {
        f.layout_no_wrap(label.clone(), label_font.clone(), theme.text())
    });
    let label_h = label_galley.rect.height();
    let label_w = label_galley.rect.width();

    let desc_galley = description.as_ref().map(|s| {
        ui.fonts(|f| {
            f.layout(s.clone(), desc_font.clone(), theme.dim(), left_max_w)
        })
    });
    let desc_h = desc_galley.as_ref().map(|g| g.rect.height()).unwrap_or(0.0);

    let left_h = if desc_h > 0.0 {
        label_h + gap_label_desc + desc_h
    } else {
        label_h
    };
    let content_h = left_h.max(switch_h);
    let total_h = content_h + pad_v * 2.0;
    let total_w = avail_w;

    let sense = if disabled { Sense::hover() } else { Sense::click() };
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(total_w, total_h), sense);
    if !disabled { st::cursor::clickable(ui, &response); }

    if response.clicked() && !disabled {
        *value = !*value;
        response.mark_changed();
    }

    if !ui.is_rect_visible(rect) {
        return response;
    }

    // Hover tint background — covers full row.
    if response.hovered() && !disabled {
        // TODO: switch to theme.element_hover() once the trait getter lands
        let tint = st::color_alpha(theme.text(), 12);
        ui.painter().rect_filled(rect, 4.0, tint);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Paint label + (info icon) + description on the left.
    let content_top = rect.top() + pad_v;
    // Vertically center the left stack within content area.
    let left_top = content_top + (content_h - left_h) * 0.5;

    let alpha = if disabled { 0.5 } else { 1.0 };
    let label_color = scale_alpha(theme.text(), alpha);
    let desc_color = scale_alpha(st::color_alpha(theme.dim(), 200), alpha);

    let painter = ui.painter_at(rect);

    // Label.
    let label_pos = Pos2::new(rect.left(), left_top);
    painter.galley(label_pos, label_galley, label_color);

    // Info icon (right of label).
    if let Some(info_fn) = info {
        let info_size = st::font_sm();
        let info_x = rect.left() + label_w + gap_label_info + info_size * 0.5;
        let info_y = left_top + label_h * 0.5;
        let info_color = scale_alpha(theme.dim(), alpha);
        painter.text(
            Pos2::new(info_x, info_y),
            egui::Align2::CENTER_CENTER,
            "?",
            FontId::proportional(info_size),
            info_color,
        );
        // Hit-test a small rect around the glyph for the tooltip trigger.
        let info_rect = egui::Rect::from_center_size(
            Pos2::new(info_x, info_y),
            Vec2::splat(info_size + 4.0),
        );
        let info_resp = ui.interact(info_rect, response.id.with("info"), Sense::hover());
        Tooltip::rich(info_fn).show(ui, &info_resp, theme);
    }

    // Description.
    if let Some(galley) = desc_galley {
        let desc_pos = Pos2::new(rect.left(), left_top + label_h + gap_label_desc);
        painter.galley(desc_pos, galley, desc_color);
    }

    // Switch on the right.
    let switch_x = rect.right() - switch_w;
    let switch_y = content_top + (content_h - switch_h) * 0.5;
    let switch_rect = egui::Rect::from_min_size(
        Pos2::new(switch_x, switch_y),
        Vec2::new(switch_w, switch_h),
    );
    let mut switch_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(switch_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let mut switch = Switch::new(value).size(Size::Md);
    if disabled {
        switch = switch.disabled(true);
    }
    let switch_resp = switch.show(&mut switch_ui, theme);
    if switch_resp.changed() {
        response.mark_changed();
    }

    response
}

#[inline]
fn scale_alpha(c: Color32, s: f32) -> Color32 {
    if (s - 1.0).abs() < f32::EPSILON {
        return c;
    }
    Color32::from_rgba_premultiplied(
        c.r(),
        c.g(),
        c.b(),
        ((c.a() as f32) * s.clamp(0.0, 1.0)).round() as u8,
    )
}
