//! Sidebar — vertical navigation rail or panel with sections.
//!
//! Two flavors via SidebarStyle:
//!   - Rail:  narrow (60px), icon-only, expands on hover (if hover_expand)
//!   - Panel: wider (200-280px), icon + label, fixed
//!
//! API:
//!   let mut active: usize = 0;
//!   ui.add(Sidebar::new(&mut active, &items)
//!     .style(SidebarStyle::Panel)
//!     .header(|ui, theme| { Label::heading("Apex").show(ui, theme); })
//!     .footer(|ui, theme| { /* user button */ })
//!   );

use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

use super::motion;
use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;

// ── Public types ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarStyle {
    Rail,
    #[default]
    Panel,
}

#[derive(Clone)]
pub struct SidebarItem<'a> {
    pub label: &'a str,
    pub icon: &'static str,
    pub badge: Option<u32>,
    pub disabled: bool,
}

impl<'a> SidebarItem<'a> {
    pub fn new(label: &'a str, icon: &'static str) -> Self {
        Self { label, icon, badge: None, disabled: false }
    }
    pub fn badge(mut self, n: u32) -> Self { self.badge = Some(n); self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
}

#[derive(Clone)]
pub struct SidebarSection<'a> {
    pub title: Option<&'a str>,
    pub items: Vec<SidebarItem<'a>>,
}

// ── Source enum ────────────────────────────────────────────────────────────

enum Source<'a> {
    Flat(&'a [SidebarItem<'a>]),
    Sections(&'a [SidebarSection<'a>]),
}

// ── Builder ────────────────────────────────────────────────────────────────

type SlotFn<'a> = Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>;

pub struct Sidebar<'a> {
    active: &'a mut usize,
    source: Source<'a>,
    style: SidebarStyle,
    width: Option<f32>,
    header: Option<SlotFn<'a>>,
    footer: Option<SlotFn<'a>>,
    collapsible: Option<&'a mut bool>,
}

impl<'a> Sidebar<'a> {
    pub fn new(active: &'a mut usize, items: &'a [SidebarItem<'a>]) -> Self {
        Self {
            active,
            source: Source::Flat(items),
            style: SidebarStyle::default(),
            width: None,
            header: None,
            footer: None,
            collapsible: None,
        }
    }

    pub fn sections(active: &'a mut usize, sections: &'a [SidebarSection<'a>]) -> Self {
        Self {
            active,
            source: Source::Sections(sections),
            style: SidebarStyle::default(),
            width: None,
            header: None,
            footer: None,
            collapsible: None,
        }
    }

    pub fn style(mut self, s: SidebarStyle) -> Self { self.style = s; self }
    pub fn width(mut self, px: f32) -> Self { self.width = Some(px); self }

    pub fn header(mut self, f: impl FnOnce(&mut Ui, &dyn ComponentTheme) + 'a) -> Self {
        self.header = Some(Box::new(f));
        self
    }

    pub fn footer(mut self, f: impl FnOnce(&mut Ui, &dyn ComponentTheme) + 'a) -> Self {
        self.footer = Some(Box::new(f));
        self
    }

    pub fn collapsible(mut self, state: &'a mut bool) -> Self {
        self.collapsible = Some(state);
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        paint_sidebar(self, ui, theme)
    }
}

// ── Layout constants ───────────────────────────────────────────────────────

const RAIL_WIDTH: f32 = 60.0;
const PANEL_WIDTH: f32 = 240.0;
const ITEM_HEIGHT: f32 = 32.0;
const ICON_SIZE: f32 = 16.0;
const ACTIVE_STRIPE: f32 = 2.0;
const COLLAPSE_BTN: f32 = 20.0;

// ── Painting ───────────────────────────────────────────────────────────────

fn paint_sidebar(sb: Sidebar<'_>, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
    let Sidebar {
        active,
        source,
        style: requested_style,
        width: width_override,
        header,
        footer,
        collapsible,
    } = sb;

    let outer_id = ui.make_persistent_id("ui_kit_sidebar");

    // Resolve effective style. If collapsible state is true → Rail, false → Panel.
    let effective_style = match collapsible.as_ref() {
        Some(state) if **state => SidebarStyle::Rail,
        Some(_) => SidebarStyle::Panel,
        None => requested_style,
    };

    // Animate width between Rail and Panel widths.
    let target_w = width_override.unwrap_or(match effective_style {
        SidebarStyle::Rail => RAIL_WIDTH,
        SidebarStyle::Panel => PANEL_WIDTH,
    });
    let anim_w = motion::ease_value(ui.ctx(), outer_id.with("w"), target_w, motion::MED);

    let avail_h = ui.available_height();
    let avail_h = if avail_h.is_finite() && avail_h > 0.0 { avail_h } else { 400.0 };

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(anim_w, avail_h),
        Sense::hover(),
    );

    if !ui.is_rect_visible(rect) {
        return response;
    }

    // Use the animating style: snapshot of label visibility based on width.
    let show_labels = anim_w > (RAIL_WIDTH + PANEL_WIDTH) * 0.5;

    // Outer frame.
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, theme.bg());
    painter.line_segment(
        [Pos2::new(rect.right() - 0.5, rect.top()),
         Pos2::new(rect.right() - 0.5, rect.bottom())],
        Stroke::new(1.0, theme.border()),
    );

    let pad_x = st::gap_sm();
    let pad_y = st::gap_md();

    // ── Header ─────────────────────────────────────────────────────────
    let header_inner = Rect::from_min_max(
        Pos2::new(rect.left() + pad_x, rect.top() + pad_y),
        Pos2::new(rect.right() - pad_x, rect.top() + pad_y),
    );
    let mut content_top = rect.top() + pad_y;

    if let Some(hdr) = header {
        let hdr_max = Rect::from_min_max(
            header_inner.min,
            Pos2::new(header_inner.max.x, rect.top() + pad_y + 48.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(hdr_max)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(rect);
        hdr(&mut child, theme);
        let used = child.min_rect().height();
        content_top = hdr_max.min.y + used + st::gap_xs();
        // Separator under header.
        ui.painter_at(rect).line_segment(
            [Pos2::new(rect.left() + pad_x, content_top),
             Pos2::new(rect.right() - pad_x, content_top)],
            Stroke::new(1.0, st::color_alpha(theme.border(), st::ALPHA_STRONG)),
        );
        content_top += st::gap_xs();
    }

    // ── Collapse button (top-right) ────────────────────────────────────
    if let Some(state) = collapsible {
        let btn_rect = Rect::from_min_size(
            Pos2::new(rect.right() - pad_x - COLLAPSE_BTN, rect.top() + 4.0),
            Vec2::splat(COLLAPSE_BTN),
        );
        let btn_resp = ui.interact(btn_rect, outer_id.with("collapse"), Sense::click());
        let hov = motion::ease_bool(ui.ctx(), outer_id.with("collapse_hov"),
            btn_resp.hovered(), motion::FAST);
        let bg = motion::lerp_color(
            Color32::TRANSPARENT,
            st::color_alpha(theme.text(), st::ALPHA_GHOST),
            hov,
        );
        ui.painter().rect_filled(btn_rect, CornerRadius::same(3), bg);
        let glyph = if *state {
            crate::ui_kit::icons::Icon::CARET_RIGHT
        } else {
            crate::ui_kit::icons::Icon::CARET_LEFT
        };
        ui.painter().text(
            btn_rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            FontId::proportional(st::font_sm()),
            theme.dim(),
        );
        if btn_resp.clicked() {
            *state = !*state;
        }
    }

    // ── Footer (allocate from bottom) ──────────────────────────────────
    let mut content_bottom = rect.bottom() - pad_y;
    if let Some(ftr) = footer {
        let ftr_h = 40.0;
        let ftr_rect = Rect::from_min_max(
            Pos2::new(rect.left() + pad_x, content_bottom - ftr_h),
            Pos2::new(rect.right() - pad_x, content_bottom),
        );
        // Separator above footer.
        ui.painter_at(rect).line_segment(
            [Pos2::new(rect.left() + pad_x, ftr_rect.top() - st::gap_xs()),
             Pos2::new(rect.right() - pad_x, ftr_rect.top() - st::gap_xs())],
            Stroke::new(1.0, st::color_alpha(theme.border(), st::ALPHA_STRONG)),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(ftr_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(rect);
        ftr(&mut child, theme);
        content_bottom = ftr_rect.top() - st::gap_xs() - 1.0;
    }

    // ── Items ──────────────────────────────────────────────────────────
    let items_rect = Rect::from_min_max(
        Pos2::new(rect.left(), content_top),
        Pos2::new(rect.right(), content_bottom),
    );

    let cur_active = *active;
    let mut new_active = cur_active;
    let mut idx_counter: usize = 0;

    let mut y = items_rect.top();

    // Build a flat iteration with optional section titles.
    let paint_section_title = |ui: &mut Ui, y: &mut f32, title: &str| {
        if !show_labels { return; } // hide titles in rail mode
        let title_rect = Rect::from_min_size(
            Pos2::new(rect.left() + pad_x, *y + st::gap_xs()),
            Vec2::new(rect.width() - pad_x * 2.0, 14.0),
        );
        ui.painter_at(rect).text(
            Pos2::new(title_rect.left(), title_rect.center().y),
            Align2::LEFT_CENTER,
            title.to_uppercase(),
            FontId::proportional(st::font_xs()),
            theme.dim(),
        );
        *y = title_rect.bottom() + st::gap_2xs();
    };

    let render_item = |ui: &mut Ui,
                       y: &mut f32,
                       item: &SidebarItem<'_>,
                       my_idx: usize,
                       cur_active: usize,
                       new_active: &mut usize| {
        let row_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 4.0, *y),
            Vec2::new(rect.width() - 8.0, ITEM_HEIGHT),
        );

        let id = outer_id.with(("item", my_idx));
        let sense = if item.disabled { Sense::hover() } else { Sense::click() };
        let resp = ui.interact(row_rect, id, sense);

        let is_active = my_idx == cur_active;
        let hover_t = motion::ease_bool(ui.ctx(), id.with("hov"),
            resp.hovered() && !item.disabled, motion::FAST);
        let active_t = motion::ease_bool(ui.ctx(), id.with("act"), is_active, motion::MED);

        let p = ui.painter_at(rect);

        // Background.
        if is_active {
            let bg = motion::fade_in(
                st::color_alpha(theme.accent(), st::ALPHA_GHOST),
                active_t,
            );
            p.rect_filled(row_rect, CornerRadius::same(4), bg);
            // Left stripe.
            let stripe = Rect::from_min_size(
                Pos2::new(row_rect.left(), row_rect.top() + 4.0),
                Vec2::new(ACTIVE_STRIPE, row_rect.height() - 8.0),
            );
            p.rect_filled(stripe, CornerRadius::same(1), theme.accent());
        } else if hover_t > 0.01 {
            let bg = motion::lerp_color(
                Color32::TRANSPARENT,
                st::color_alpha(theme.text(), 18),
                hover_t,
            );
            p.rect_filled(row_rect, CornerRadius::same(4), bg);
        }

        // Content layout.
        let label_color = if item.disabled {
            st::color_alpha(theme.dim(), 120)
        } else if is_active {
            theme.text()
        } else {
            motion::lerp_color(theme.dim(), theme.text(), hover_t)
        };

        // Icon: fixed square at the left.
        let icon_x = row_rect.left() + ACTIVE_STRIPE + st::gap_xs();
        let icon_center = Pos2::new(icon_x + ICON_SIZE * 0.5, row_rect.center().y);
        p.text(
            icon_center,
            Align2::CENTER_CENTER,
            item.icon,
            FontId::proportional(ICON_SIZE),
            label_color,
        );

        // Label (only when wide enough).
        if show_labels {
            let label_x = icon_center.x + ICON_SIZE * 0.5 + st::gap_xs();
            let badge_w = if item.badge.is_some() { 28.0 } else { 0.0 };
            let label_max_x = row_rect.right() - st::gap_xs() - badge_w;
            let label_w = (label_max_x - label_x).max(0.0);
            let g = ui.fonts(|f| f.layout(
                item.label.to_string(),
                FontId::proportional(st::font_sm()),
                label_color,
                label_w,
            ));
            p.galley(
                Pos2::new(label_x, row_rect.center().y - g.rect.height() * 0.5),
                g,
                label_color,
            );

            // Badge.
            if let Some(n) = item.badge {
                let s = if n > 99 { "99+".to_string() } else { n.to_string() };
                let bg_text = ui.fonts(|f| f.layout_no_wrap(
                    s.clone(), FontId::monospace(10.0), Color32::WHITE,
                ));
                let bw = (bg_text.rect.width() + 10.0).max(18.0);
                let bh = 14.0;
                let br = Rect::from_min_size(
                    Pos2::new(row_rect.right() - st::gap_xs() - bw,
                              row_rect.center().y - bh * 0.5),
                    Vec2::new(bw, bh),
                );
                p.rect_filled(br, CornerRadius::same(7), theme.bear());
                p.text(br.center(), Align2::CENTER_CENTER, &s,
                    FontId::monospace(10.0), Color32::WHITE);
            }
        } else if let Some(n) = item.badge {
            // Rail mode: tiny dot-style badge in the top-right corner of the icon.
            let s = if n > 9 { "9+".to_string() } else { n.to_string() };
            let bh = 12.0;
            let bw = 14.0;
            let br = Rect::from_min_size(
                Pos2::new(icon_center.x + 4.0, icon_center.y - ICON_SIZE * 0.5 - 2.0),
                Vec2::new(bw, bh),
            );
            p.rect_filled(br, CornerRadius::same(6), theme.bear());
            p.text(br.center(), Align2::CENTER_CENTER, &s,
                FontId::monospace(9.0), Color32::WHITE);
        }

        if resp.clicked() && !item.disabled {
            *new_active = my_idx;
        }

        *y += ITEM_HEIGHT + 2.0;
    };

    match source {
        Source::Flat(items) => {
            for item in items {
                if y + ITEM_HEIGHT > items_rect.bottom() { break; }
                render_item(ui, &mut y, item, idx_counter, cur_active, &mut new_active);
                idx_counter += 1;
            }
        }
        Source::Sections(sections) => {
            for section in sections {
                if let Some(t) = section.title {
                    if y + 18.0 > items_rect.bottom() { break; }
                    paint_section_title(ui, &mut y, t);
                }
                for item in &section.items {
                    if y + ITEM_HEIGHT > items_rect.bottom() { break; }
                    render_item(ui, &mut y, item, idx_counter, cur_active, &mut new_active);
                    idx_counter += 1;
                }
                y += st::gap_xs();
            }
        }
    }

    if new_active != cur_active {
        *active = new_active;
    }

    // Outline (debug-quiet): use StrokeKind to pin variant import.
    let _ = StrokeKind::Inside;

    response
}
