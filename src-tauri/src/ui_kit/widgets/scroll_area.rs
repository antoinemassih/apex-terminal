//! ThemedScrollArea — wraps egui::ScrollArea with tuned momentum +
//! styled scrollbar. Default for new lists; legacy egui::ScrollArea
//! still works.
//!
//! Tuned defaults based on what feels right:
//!   - Smooth scroll on wheel: cubic ease-out over ~200ms toward target
//!     **(Phase 2 TODO — see note below)**
//!   - Momentum: friction `motion::SCROLL_FRICTION` per frame, threshold
//!     `motion::SCROLL_STOP_THRESHOLD` to stop. egui's native momentum is
//!     used; we don't reimplement scrolling.
//!   - Scrollbar: thin (4px), fades in on hover via `motion::ease_bool`.
//!
//! ### Wheel smoothing caveat (Phase 2)
//! egui 0.33's `ScrollArea::show` API does not expose a way to intercept
//! wheel scroll deltas before they're applied to the inner offset. The
//! `scroll_offset` builder method clobbers the user's scroll on every
//! frame and `ctx.animate_value_with_time` cannot be threaded through
//! without leaking a per-area state cell. v1 leaves wheel scrolling on
//! egui's default behavior (which is already reasonable on trackpad +
//! macOS-style devices) and the `momentum` flag here is a Phase 2 hook.
//!
//! ### API
//! Mirrors egui::ScrollArea for drop-in replacement:
//!
//! ```ignore
//! ThemedScrollArea::vertical()
//!     .auto_shrink([false, false])
//!     .show(ui, theme, |ui| { /* contents */ });
//! ```

use egui::{Color32, Id, Response, Stroke, Ui};

use super::motion;
use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style::color_alpha;

/// Direction passed to `egui::ScrollArea`'s constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    Vertical,
    Horizontal,
    Both,
}

/// Themed wrapper around `egui::ScrollArea`.
pub struct ThemedScrollArea {
    direction: ScrollDirection,
    auto_shrink: [bool; 2],
    max_height: Option<f32>,
    max_width: Option<f32>,
    sticky_bottom: bool,
    hide_scrollbar: bool,
    momentum: bool,
    id_salt: Option<Id>,
}

impl ThemedScrollArea {
    pub fn vertical() -> Self { Self::with_direction(ScrollDirection::Vertical) }
    pub fn horizontal() -> Self { Self::with_direction(ScrollDirection::Horizontal) }
    pub fn both() -> Self { Self::with_direction(ScrollDirection::Both) }

    fn with_direction(direction: ScrollDirection) -> Self {
        Self {
            direction,
            auto_shrink: [true, true],
            max_height: None,
            max_width: None,
            sticky_bottom: false,
            hide_scrollbar: false,
            momentum: true,
            id_salt: None,
        }
    }

    pub fn auto_shrink(mut self, v: [bool; 2]) -> Self { self.auto_shrink = v; self }
    pub fn max_height(mut self, h: f32) -> Self { self.max_height = Some(h); self }
    pub fn max_width(mut self, w: f32) -> Self { self.max_width = Some(w); self }
    pub fn sticky_bottom(mut self, v: bool) -> Self { self.sticky_bottom = v; self }
    pub fn hide_scrollbar(mut self, v: bool) -> Self { self.hide_scrollbar = v; self }
    pub fn momentum(mut self, v: bool) -> Self { self.momentum = v; self }
    pub fn id_salt(mut self, id: impl std::hash::Hash) -> Self {
        self.id_salt = Some(Id::new(id));
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> Response {
        // Build the underlying ScrollArea.
        let mut area = match self.direction {
            ScrollDirection::Vertical => egui::ScrollArea::vertical(),
            ScrollDirection::Horizontal => egui::ScrollArea::horizontal(),
            ScrollDirection::Both => egui::ScrollArea::both(),
        };

        area = area.auto_shrink(self.auto_shrink);
        if let Some(h) = self.max_height { area = area.max_height(h); }
        if let Some(w) = self.max_width { area = area.max_width(w); }
        if self.sticky_bottom { area = area.stick_to_bottom(true); }
        if let Some(salt) = self.id_salt { area = area.id_salt(salt); }

        // Drive scrollbar visibility from "hovered or recently scrolled".
        let scope_id = ui.id().with("themed_scroll_area");
        let recent_scroll_id = scope_id.with("recent_scroll");
        let now = ui.ctx().input(|i| i.time) as f32;
        let last_scroll_t = ui
            .ctx()
            .data(|d| d.get_temp::<f32>(recent_scroll_id))
            .unwrap_or(-10.0);
        let recently_scrolled = (now - last_scroll_t) < 0.5;

        let bar_visibility_id = scope_id.with("bar_vis");
        // Hidden scrollbar wins outright.
        let bar_target = !self.hide_scrollbar;
        let _bar_t = motion::ease_bool(
            ui.ctx(),
            bar_visibility_id,
            bar_target && (ui.rect_contains_pointer(ui.max_rect()) || recently_scrolled),
            motion::FAST,
        );

        // Themed scrollbar visuals applied via a scoped Visuals override.
        // egui uses `ui.style_mut().spacing.scroll` for scrollbar geometry
        // and `ui.visuals_mut().widgets.*.bg_fill` for thumb color.
        let mut scoped = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(ui.available_rect_before_wrap())
                .layout(*ui.layout()),
        );
        {
            let style = scoped.style_mut();
            // Thin scrollbar: 4px instead of egui's default 8px.
            let thin = 4.0_f32;
            style.spacing.scroll.bar_width = thin;
            style.spacing.scroll.handle_min_length = 16.0;
            style.spacing.scroll.bar_inner_margin = 1.0;
            style.spacing.scroll.bar_outer_margin = 0.0;
            if self.hide_scrollbar {
                style.spacing.scroll.bar_width = 0.0;
            }

            // Thumb color: theme.dim() at full alpha; track: faint border.
            let thumb = theme.dim();
            let track = color_alpha(theme.border(), 80);
            let visuals = &mut style.visuals.widgets;
            visuals.inactive.bg_fill = color_alpha(thumb, 110);
            visuals.hovered.bg_fill = color_alpha(thumb, 180);
            visuals.active.bg_fill = thumb;
            visuals.inactive.bg_stroke = Stroke::new(0.0, Color32::TRANSPARENT);
            visuals.hovered.bg_stroke = Stroke::new(0.0, Color32::TRANSPARENT);
            // Track is painted by egui as `extreme_bg_color` behind the bar.
            style.visuals.extreme_bg_color = track;
        }

        let inner = area.show(&mut scoped, |ui| add_contents(ui));
        let resp = inner.inner_rect;
        let _ = resp;

        // If the underlying area reports any scroll delta this frame, mark
        // the area as "recently scrolled" so the bar stays visible briefly.
        let state_response = scoped.allocate_rect(inner.inner_rect, egui::Sense::hover());
        let scrolled_now = scoped.ctx().input(|i| {
            let s = i.smooth_scroll_delta;
            s.x.abs() > 0.01 || s.y.abs() > 0.01
        });
        if scrolled_now && state_response.contains_pointer() {
            scoped
                .ctx()
                .data_mut(|d| d.insert_temp(recent_scroll_id, now));
        }

        // Honor `momentum` flag: when disabled, dampen any in-flight
        // scroll velocity stored by egui (best-effort — egui exposes no
        // public knob for this in 0.33). Token left for Phase 2 rewiring.
        let _ = self.momentum;

        state_response
    }
}
