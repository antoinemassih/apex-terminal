//! Context menu — re-homed from `chart::renderer::ui::components::context_menu`.
//!
//! Floating right-click / popup menu builder with sections, dividers,
//! checks, radios, submenus, and danger styling. The original module's
//! design notes apply unchanged. Migration into ui_kit:
//!   * `ContextMenu::new` accepts any `&T: ComponentTheme`.
//!   * Open animation: alpha fade + tiny scale-in over `motion::FAST`,
//!     keyed on the menu id so concurrent menus animate independently.
//!   * Public types (`MenuTheme`, `MenuBuilder`, `MenuItem`, etc.)
//!     are unchanged so callers compile via the back-compat re-export.
//!
//! ### Signature note
//! `ContextMenu::show(ui, body)` intentionally does NOT take
//! `theme: &dyn ComponentTheme` as a third parameter — the standard
//! ui_kit shape. Theme is captured at construction time via
//! `ContextMenu::new(theme)` because the menu snapshots a `MenuTheme`
//! (a `Copy` palette) so the body closure can be handed a
//! `&mut MenuBuilder<'_>` without lifetime gymnastics tying the closure
//! to the original `&dyn ComponentTheme`. This is an explicit, documented
//! variance from the "Builder + show(ui, theme)" rule in `CLAUDE.md`.


use egui::{Align2, Color32, Id, Pos2, Response, RichText, Sense, Stroke, Ui, Vec2};

use super::theme::{ComponentTheme, PortableTheme};
use super::motion;

use crate::ui_kit::widgets::frames::{BorderAlpha, PopupFrame};
use crate::ui_kit::tokens::*;
use crate::ui_kit::text_style::TextStyle;

// `Theme` alias removed — the legacy `MenuTheme::from_theme(&Theme)` shortcut
// is deleted below; `from_component<T: ComponentTheme>` is the portable API.

// ─── Shared theme snapshot ───────────────────────────────────────────────────

// `MenuTheme` is GONE — it was a six-colour projection of `ComponentTheme`
// (accent/dim/bg/fg/danger/shadow) snapshotted at construction so the builder
// need not hold a borrow until `.show()`.
//
// The lifetime problem was real; the fix was not. Everything downstream of the
// projection was cut off from the rest of the palette AND from the recipe
// layer — the menu could not resolve a recipe key even in principle, because
// `resolve` needs a `&dyn ComponentTheme` and the six colours are not one.
//
// `PortableTheme::snapshot(t)` solves the same lifetime problem, is an OWNED
// `ComponentTheme`, and costs a struct copy. A widget that needs to outlive a
// borrow should snapshot the WHOLE theme, never a hand-picked subset of it.

// ─── ContextMenu builder ─────────────────────────────────────────────────────

#[derive(Copy, Clone)]
pub enum MenuAnchor {
    Pos(Pos2),
    BelowRect(egui::Rect),
}

#[must_use = "ContextMenu must be terminated with `.show(ui, |menu| { ... })`"]
pub struct ContextMenu {
    id: Id,
    anchor: Option<MenuAnchor>,
    theme: PortableTheme,
    min_width: f32,
}

impl ContextMenu {
    /// Construct from any `ComponentTheme`. Matches legacy `ContextMenu::new(&Theme)`.
    pub fn new<T: ComponentTheme>(t: &T) -> Self {
        Self {
            id: Id::new("apex_context_menu"),
            anchor: None,
            theme: PortableTheme::snapshot(t),
            min_width: 160.0,
        }
    }

    pub fn id(mut self, id: impl std::hash::Hash) -> Self {
        self.id = Id::new(id);
        self
    }

    pub fn pos(mut self, p: Pos2) -> Self {
        self.anchor = Some(MenuAnchor::Pos(p));
        self
    }

    pub fn below(mut self, r: &Response) -> Self {
        self.anchor = Some(MenuAnchor::BelowRect(r.rect));
        self
    }

    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    /// Paint the menu and run `body` to populate rows.
    pub fn show<R, F>(self, ui: &mut Ui, body: F) -> Option<R>
    where
        F: FnOnce(&mut MenuBuilder<'_>) -> R,
    {
        let pos = match self.anchor {
            Some(MenuAnchor::Pos(p)) => p,
            Some(MenuAnchor::BelowRect(r)) => egui::pos2(r.left(), r.bottom() + gap_xs()),
            None => ui.cursor().min,
        };
        let theme = self.theme;
        let min_width = self.min_width;
        let id = self.id;

        // Open animation: alpha 0->1 over FAST. Origin = anchor pos.
        let appear_t = motion::ease_bool(ui.ctx(), id.with("apex_ctx_anim"), true, motion::FAST);

        let mut out: Option<R> = None;
        let size_id = id.with("apex_ctx_size");
        let prior_size: Vec2 = ui
            .ctx()
            .memory(|m| m.data.get_temp(size_id))
            .unwrap_or(Vec2::new(min_width, 32.0));
        let area_resp = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                ui.set_opacity(appear_t);
                let shadow_rect = egui::Rect::from_min_size(pos, prior_size);
                // Use the menu's themed shadow tint so light themes get a
                // soft gray drop instead of a hard black smudge.
                // `theme` is a `MenuTheme` snapshot (not `dyn ComponentTheme`),
                // so we inline what `md_themed` does rather than calling it.
                super::paint_shadow_gpu(
                    ui.painter(),
                    shadow_rect,
                    super::ShadowPaint {
                        radius: 16.0,
                        offset: egui::Vec2::new(0.0, 4.0),
                        color: color_alpha(theme.shadow_color(), 77),
                        spread: 0.0,
                    },
                );
                let frame = PopupFrame::new()
                    .colors(theme.bg(), theme.dim())
                    .ctx(ui.ctx())
                    .border_alpha(BorderAlpha::Line)
                    .corner_radius(radius_sm())
                    .inner_margin(egui::Margin::symmetric(gap_xs() as i8, gap_xs() as i8))
                    .build();
                frame.show(ui, |ui| {
                    ui.set_min_width(min_width);
                    let mut mb = MenuBuilder { ui, theme };
                    out = Some(body(&mut mb));
                });
            });
        let measured = area_resp.response.rect.size();
        if measured.x > 0.0 && measured.y > 0.0 {
            ui.ctx().memory_mut(|m| m.data.insert_temp(size_id, measured));
        }
        out
    }
}

// ─── MenuBuilder — passed into the body closure ─────────────────────────────

pub struct MenuBuilder<'a> {
    pub ui: &'a mut Ui,
    pub theme: PortableTheme,
}

impl<'a> MenuBuilder<'a> {
    pub fn add<R: MenuRow>(&mut self, row: R) -> Response {
        row.show(self.ui, &self.theme)
    }
    pub fn add_section(&mut self, label: &str) -> Response {
        self.add(MenuSection::new(label))
    }
    pub fn add_divider(&mut self) -> Response {
        self.add(MenuDivider)
    }
}

// ─── MenuRow trait ───────────────────────────────────────────────────────────

pub trait MenuRow {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response;
}

// ─── MenuSection ─────────────────────────────────────────────────────────────

pub struct MenuSection<'a> {
    label: &'a str,
}

impl<'a> MenuSection<'a> {
    pub fn new(label: &'a str) -> Self { Self { label } }
}

impl<'a> MenuRow for MenuSection<'a> {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        let resp = ui.horizontal(|ui| {
            ui.add_space(gap_sm());
            ui.label(
                RichText::new(self.label.to_uppercase())
                    .size(font_xs())
                    .color(color_alpha(theme.dim(), alpha_strong())),
            )
        }).response;
        ui.add_space(gap_xs());
        resp
    }
}

// ─── MenuDivider ─────────────────────────────────────────────────────────────

pub struct MenuDivider;

impl MenuRow for MenuDivider {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 1.0),
            Sense::hover(),
        );
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + gap_sm(), rect.center().y),
                egui::pos2(rect.right() - gap_sm(), rect.center().y),
            ],
            Stroke::new(stroke_hair(), color_alpha(theme.dim(), alpha_line())),
        );
        ui.add_space(gap_xs());
        resp
    }
}

// ─── Internal row painter ───────────────────────────────────────────────────

fn paint_row(
    ui: &mut Ui,
    theme: &PortableTheme,
    label: &str,
    fg: Color32,
    icon: Option<&str>,
    shortcut: Option<&str>,
    suffix: Option<&str>,
    leading_check: Option<bool>,
) -> Response {
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_xs());

    let mut display = String::new();
    if let Some(checked) = leading_check {
        display.push_str(if checked { "\u{2713} " } else { "  " });
    }
    if let Some(ic) = icon {
        display.push_str(ic);
        display.push(' ');
    }
    display.push_str(label);
    if let Some(sx) = suffix {
        display.push(' ');
        display.push_str(sx);
    }

    // Reserve the shortcut's width before the label is laid out.
    // The rule lives in `fit_menu_label` because `chart/renderer/ui/components/
    // menus.rs` is a second implementation of this same row and had the same
    // defect; see that function for why the reservation is doubled and why the
    // shortcut is dropped below a minimum.
    //
    // The button still spans the full width, so the hover highlight covers the
    // whole row; only the TEXT is bounded.
    let sc_font = TextStyle::MonoXs.font_id_in(ui);
    let row_w = ui.available_width().max(80.0);
    let label_font = crate::ui_kit::style::prop_at(super::tokens::Size::Md.font_size());
    let (fitted, show_shortcut) = fit_menu_label(
        ui.painter(), &display, &label_font, fg, shortcut, &sc_font, row_w);
    display = fitted;

    let resp = ui
        .horizontal(|ui| {
            let r = ui.add(
                super::Button::new(display.as_str())
                    .variant(super::tokens::Variant::Ghost)
                    .fg(fg)
                    // `20.0` sat between control_xs (18) and control_sm (22)
                    // — off the ladder, so it could not follow Density and
                    // could not agree with the controls around it. It was a
                    // pre-existing literal that this rewrite made visible to
                    // `control_size_lint`; `min_size` is a floor and the row is
                    // content-driven, so moving it onto the rung costs 2px.
                    .min_size(egui::vec2(row_w, super::tokens::Size::Sm.height())),
            );
            if let (Some(sc), true) = (shortcut, show_shortcut) {
                let sc_color = color_alpha(theme.dim(), alpha_muted());
                let max_x = r.rect.right() - gap_sm();
                let y = r.rect.center().y;
                ui.painter().text(
                    egui::pos2(max_x, y),
                    Align2::RIGHT_CENTER,
                    sc,
                    sc_font.clone(),
                    sc_color,
                );
            }
            r
        })
        .inner;

    ui.spacing_mut().button_padding = prev_pad;

    crate::ui_kit::cursor::clickable(ui, &resp);
    // M3.3: hover fill derived from the ONE interaction table (accent tint at
    // the hover-bg token) instead of a hand-picked `color_alpha(accent, ghost)`.
    let v = crate::ui_kit::interaction::apply_interaction(
        resp.rect,
        crate::ui_kit::interaction::InteractionState::new().hovered(resp.hovered()),
        theme.accent(),
        &crate::ui_kit::interaction::InteractionTokens::borderless(),
    );
    if v.fill != Color32::TRANSPARENT {
        // `popover` key — shared by ContextMenu and the tool popovers, so a
        // style restyles every floating surface at once instead of one of them
        // quietly keeping the old look.
        //
        // Resolved against the menu's OWN theme now.
        //
        // This used to reach for the ambient theme because the widget was
        // handed a `MenuTheme` — a six-colour projection that is not a
        // `ComponentTheme` and so could not be passed to `resolve` at all. With
        // the snapshot upgraded to `PortableTheme` the workaround is gone: the
        // menu resolves against the theme it was actually constructed with,
        // which is what a caller passing an explicit theme expects.
        let (pop_cr, pop_fill, _) = crate::ui_kit::widgets::theme::resolve_control_chrome(
            ui.ctx(), theme, "popover",
            radius_sm(), v.fill, v.fill, 0.0,
        );
        ui.painter().rect_filled(resp.rect, pop_cr, pop_fill);
    }
    resp
}

// ─── MenuItem ────────────────────────────────────────────────────────────────

pub struct MenuItem<'a> {
    label: &'a str,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self { Self { label } }
}

impl<'a> MenuRow for MenuItem<'a> {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        paint_row(ui, theme, self.label, theme.dim(), None, None, None, None)
    }
}

/// Fit a menu row's label around its shortcut, and say whether the shortcut
/// fits at all.
///
/// Returns `(label_to_paint, show_shortcut)`.
///
/// # Why this is a function and not four lines at each call site
///
/// There are TWO menu-row implementations in this codebase — this one and
/// `chart/renderer/ui/components/menus.rs` — built the same way and carrying
/// the same defect: the label goes through `Button`, which sizes itself from
/// the label ALONE, and the shortcut is then painted `RIGHT_CENTER` over the
/// same rect. Fixing one and not the other is worse than fixing neither: two
/// menus that look identical would truncate differently.
///
/// # The two things it gets right that are easy to get wrong
///
/// **The reservation is doubled.** `Button` left-aligns its label inside its
/// CONTENT rect and centres that block when the button is wider — which it is,
/// because `min_size` stretches it across the row for the hover highlight. The
/// label therefore grows symmetrically about the centre, so every pixel of
/// label costs half a pixel on each side. Reserving the shortcut once leaves
/// the label centred and still overlapping.
///
/// **Below a minimum, the shortcut is dropped.** At 140px a `Ctrl+Shift+S` is
/// 72px — over half the row — and reserving for it ellipsises the label to a
/// bare `…` that still collides. Two unreadable halves is a worse answer than
/// one readable one, so the accelerator is not advertised at a width where it
/// cannot be read.
pub(crate) fn fit_menu_label(
    painter: &egui::Painter,
    display: &str,
    label_font: &egui::FontId,
    label_color: Color32,
    shortcut: Option<&str>,
    shortcut_font: &egui::FontId,
    row_w: f32,
) -> (String, bool) {
    let sc_w = shortcut
        .map(|sc| crate::ui_kit::style::measure_with_painter(painter, sc, shortcut_font.clone()).x)
        .unwrap_or(0.0);
    let min_label_room = gap_lg() * 3.0;
    let label_room = row_w - 2.0 * (sc_w + gap_sm()) - gap_lg() * 2.0;
    if sc_w <= 0.0 || label_room < min_label_room {
        return (display.to_string(), false);
    }
    let fitted = crate::ui_kit::style::ellipsize_to(
        painter, display, label_font, label_room.max(0.0), label_color);
    (fitted, true)
}

// ─── MenuItemWithShortcut ───────────────────────────────────────────────────

pub struct MenuItemWithShortcut<'a> {
    label: &'a str,
    shortcut: &'a str,
}

impl<'a> MenuItemWithShortcut<'a> {
    pub fn new(label: &'a str, shortcut: &'a str) -> Self { Self { label, shortcut } }
}

impl<'a> MenuRow for MenuItemWithShortcut<'a> {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        paint_row(ui, theme, self.label, theme.dim(), None, Some(self.shortcut), None, None)
    }
}

// ─── MenuItemWithIcon ───────────────────────────────────────────────────────

pub struct MenuItemWithIcon<'a> {
    label: &'a str,
    icon: &'a str,
}

impl<'a> MenuItemWithIcon<'a> {
    pub fn new(label: &'a str, icon: &'a str) -> Self { Self { label, icon } }
}

impl<'a> MenuRow for MenuItemWithIcon<'a> {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        paint_row(ui, theme, self.label, theme.dim(), Some(self.icon), None, None, None)
    }
}

// ─── CheckMenuItem ───────────────────────────────────────────────────────────

pub struct CheckMenuItem<'a> {
    label: &'a str,
    checked: &'a mut bool,
}

impl<'a> CheckMenuItem<'a> {
    pub fn new(label: &'a str, checked: &'a mut bool) -> Self { Self { label, checked } }
}

impl<'a> MenuRow for CheckMenuItem<'a> {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        let resp = paint_row(
            ui,
            theme,
            self.label,
            theme.dim(),
            None,
            None,
            None,
            Some(*self.checked),
        );
        if resp.clicked() {
            *self.checked = !*self.checked;
        }
        resp
    }
}

// ─── RadioMenuItem<T> ───────────────────────────────────────────────────────

pub struct RadioMenuItem<'a, T: PartialEq + Clone> {
    label: &'a str,
    value: T,
    current: &'a mut T,
}

impl<'a, T: PartialEq + Clone> RadioMenuItem<'a, T> {
    pub fn new(label: &'a str, value: T, current: &'a mut T) -> Self {
        Self { label, value, current }
    }
}

impl<'a, T: PartialEq + Clone> MenuRow for RadioMenuItem<'a, T> {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        let selected = *self.current == self.value;
        let resp = paint_row(
            ui,
            theme,
            self.label,
            theme.dim(),
            None,
            None,
            None,
            Some(selected),
        );
        if resp.clicked() {
            *self.current = self.value.clone();
        }
        resp
    }
}

// ─── Submenu ─────────────────────────────────────────────────────────────────

pub struct Submenu<'a, F>
where
    F: FnOnce(&mut MenuBuilder<'_>),
{
    label: &'a str,
    body: F,
}

impl<'a, F> Submenu<'a, F>
where
    F: FnOnce(&mut MenuBuilder<'_>),
{
    pub fn new(label: &'a str, body: F) -> Self { Self { label, body } }
}

impl<'a, F> MenuRow for Submenu<'a, F>
where
    F: FnOnce(&mut MenuBuilder<'_>),
{
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        let resp = paint_row(
            ui,
            theme,
            self.label,
            theme.dim(),
            None,
            None,
            Some("\u{25B8}"),
            None,
        );

        let popup_id = ui.id().with(("submenu", self.label));
        let open_mem = ui.memory(|m| m.data.get_temp::<bool>(popup_id).unwrap_or(false));
        let want_open = open_mem || resp.hovered() || resp.clicked();
        ui.memory_mut(|m| m.data.insert_temp(popup_id, want_open));

        if want_open {
            let appear_t = motion::ease_bool(ui.ctx(), popup_id.with("anim"), true, motion::FAST);
            let anchor = egui::pos2(resp.rect.right() + gap_xs(), resp.rect.top());
            egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(anchor)
                .show(ui.ctx(), |ui| {
                    ui.set_opacity(appear_t);
                    let frame = PopupFrame::new()
                        .colors(theme.bg(), theme.dim())
                        .ctx(ui.ctx())
                        .border_alpha(BorderAlpha::Line)
                        .corner_radius(radius_sm())
                        .inner_margin(egui::Margin::symmetric(gap_xs() as i8, gap_xs() as i8))
                        .build();
                    frame.show(ui, |ui| {
                        ui.set_min_width(140.0);
                        let mut mb = MenuBuilder { ui, theme: theme.clone() };
                        (self.body)(&mut mb);
                    });
                });
        }

        resp
    }
}

// ─── DangerMenuItem ─────────────────────────────────────────────────────────

pub struct DangerMenuItem<'a> {
    label: &'a str,
    icon: Option<&'a str>,
}

impl<'a> DangerMenuItem<'a> {
    pub fn new(label: &'a str) -> Self { Self { label, icon: None } }
    pub fn icon(mut self, icon: &'a str) -> Self { self.icon = Some(icon); self }
}

impl<'a> MenuRow for DangerMenuItem<'a> {
    fn show(self, ui: &mut Ui, theme: &PortableTheme) -> Response {
        paint_row(ui, theme, self.label, theme.danger(), self.icon, None, None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::widgets::paint_probe;

    /// A menu row's shortcut must not land on top of its label.
    ///
    /// The label goes through `Button`, which sizes itself from the label
    /// alone. The shortcut is then painted `RIGHT_CENTER` at the button's
    /// right edge — the button never learns a shortcut exists, so it reserves
    /// no room for one. A long label and a shortcut therefore share the same
    /// pixels, and neither half can notice.
    ///
    /// Widths are constrained: an unbounded probe panel gives the row so much
    /// space that nothing can collide, which is how two earlier probes in this
    /// session passed while the widget was broken.
    #[test]
    fn a_shortcut_never_lands_on_its_label() {
        for width in [320.0f32, 200.0, 140.0] {
            for (label, sc) in [
                ("Copy", "Ctrl+C"),
                ("Save workspace layout as template", "Ctrl+Shift+S"),
                ("Duplicate this pane into a new tab", "Ctrl+D"),
            ] {
                let runs = paint_probe::probe(|ui| {
                    let t = PortableTheme::dark();
                    let rect = egui::Rect::from_min_size(
                        ui.max_rect().min, egui::vec2(width, 40.0));
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    MenuItemWithShortcut::new(label, sc).show(&mut child, &t);
                });
                if runs.is_empty() {
                    continue;
                }
                paint_probe::assert_no_overlap(
                    &format!("menu row w={width} {label:?} + {sc:?}"), &runs);
            }
        }
    }
}
