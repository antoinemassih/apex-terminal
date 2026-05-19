//! Builder + impl Widget primitives — pills family.
//! See ui/widgets/mod.rs for the rationale.
//!
//! Wave 4.5b: bodies compose `ChipShell` patterns from `widgets/foundation/`.
//! Each chip picks a `ChipVariant` + `Size` + content + optional close
//! affordance. The public API (type names + builder methods) is unchanged so
//! callers in widgets/menus, toolbar, pane chrome and headers keep working.
//!
//! **DEPRECATED in the design-system pass.** Prefer `ui_kit::Tag` (with
//! `TagTone::Normal | Muted | Success | Warning`) for label/pill use
//! cases, and `ui_kit::Badge` for counts. The builder types in this file
//! (`RemovableChip`, `DisplayChip`) will be retired during the panel
//! sweep — do not add new uses.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, Stroke, Ui, Widget};
use super::super::style::*;
use crate::chart::renderer::ui::foundation::{ChipVariant, Size, Radius};
use crate::ui_kit::widgets::Button as KitButton;
use crate::ui_kit::widgets::tokens::Variant as KitVariant;

// ─── Re-export ActionSize so callers only need one import ─────────────────────
pub use super::super::components_extra::ActionSize;

// ─── Internal: ChipShell-style body with explicit palette colors ──────────────
//
// `ChipShell` itself resolves colors through a `Theme`, but the pills/chips
// public API accepts explicit palette colors (so callers can use semantic
// colors like `t.bull` / discord brand / etc.). This helper composes the same
// shell structure (Frame + pill radius + Size padding + label) using the
// palette colors the caller supplied — matching ChipShell visually while
// preserving API parity with the pre-4.5b implementation.
struct ChipBody<'a> {
    label: &'a str,
    label_color: Color32,
    fill: Color32,
    border: Color32,
    radius: egui::CornerRadius,
    height: f32,
    pad_x: f32,
    pad_y: f32,
    sense: egui::Sense,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> ChipBody<'a> {
    fn render(self, ui: &mut Ui) -> Response {
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(self.pad_x, self.pad_y);
        // Migrated to ui_kit::Button. Note: label is rendered as proportional
        // (not monospace) since ui_kit::Button takes &str + resolves font via Size.
        // Color is preserved via .fg(); monospace styling is a known visual delta
        // in this deprecated file. fill/stroke/sense/corner_radius use the new
        // escape-hatch APIs added in Part 1 of the pills migration.
        let resp = ui.add(
            KitButton::new(self.label)
                .variant(KitVariant::Chrome)
                .fill(self.fill)
                .stroke(Stroke::new(stroke_thin(), self.border))
                .corner_radius_asymmetric(self.radius)
                .min_size(egui::vec2(0.0, self.height))
                .sense(self.sense)
                .fg(self.label_color),
        );
        ui.spacing_mut().button_padding = prev_pad;
        resp
    }
}

// ─── RemovableChip ────────────────────────────────────────────────────────────

/// Text chip with a paired ✕ remove button.
///
/// Returns `(label_resp, x_clicked)` — use `.show(ui)` instead of `ui.add(...)` since
/// the tuple return type is incompatible with `impl Widget`.
///
/// # Example
/// ```ignore
/// let (resp, removed) = RemovableChip::new("SPY").theme(&theme).show(ui);
/// if removed { tags.remove(idx); }
/// ```
#[must_use = "RemovableChip must be shown with `.show(ui)` to render"]
pub struct RemovableChip<'a> {
    text: &'a str,
    accent: Color32,
    dim: Color32,
}

impl<'a> RemovableChip<'a> {
    /// New removable chip.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            accent: Color32::from_rgb(120, 140, 220),
            dim: Color32::from_rgb(120, 120, 130),
        }
    }

    /// Supply explicit palette colors.
    pub fn palette(mut self, accent: Color32, dim: Color32) -> Self {
        self.accent = accent;
        self.dim = dim;
        self
    }

    /// Pull palette colors from a Theme.
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.palette(t.accent, t.dim)
    }

    /// Render the chip. Returns `(label_response, x_was_clicked)`.
    pub fn show(self, ui: &mut Ui) -> (Response, bool) {
        // ChipVariant::Removable, Size::Sm, pill radius split across two halves
        // to host the dismissible affordance (matches ChipShell's `closable`).
        let _variant = ChipVariant::Removable;
        let _size = Size::Sm;

        let fill = color_alpha(self.accent, alpha_faint());
        let border = color_alpha(self.dim, alpha_dim());
        let mut x_clicked = false;
        let resp = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();
            let prev_pad = ui.spacing().button_padding;
            ui.spacing_mut().button_padding = egui::vec2(gap_md(), 0.0);
            // Body label (left half of pill) — migrated to ui_kit::Button.
            // Asymmetric CornerRadius: nw+sw rounded (outer), ne+se=0 (inner join).
            // Note: rendered as proportional font (not monospace) — known visual
            // delta in this deprecated file. Use ui_kit::Tag for new call sites.
            let body = ui.add(
                KitButton::new(self.text)
                    .variant(KitVariant::Chrome)
                    .fill(fill)
                    .stroke(Stroke::new(stroke_thin(), border))
                    .corner_radius_asymmetric(egui::CornerRadius { nw: 99, sw: 99, ne: 0, se: 0 })
                    .min_size(egui::vec2(0.0, row_height_dense()))
                    .fg(self.dim),
            );
            // ✕ remove button (right half of pill — the closable affordance).
            // Icon::X replaces the raw \u{00D7} glyph; pill corner radii are
            // preserved because this must visually be the right half of the chip.
            // Migrated to ui_kit::Button with asymmetric CornerRadius: ne+se rounded.
            let x = ui.add(
                KitButton::new(crate::ui_kit::icons::Icon::X)
                    .variant(KitVariant::Chrome)
                    .fill(fill)
                    .stroke(Stroke::new(stroke_thin(), border))
                    .corner_radius_asymmetric(egui::CornerRadius { nw: 0, sw: 0, ne: 99, se: 99 })
                    .min_size(egui::vec2(18.0, row_height_dense()))
                    .fg(self.dim),
            );
            ui.spacing_mut().button_padding = prev_pad;
            if x.clicked() { x_clicked = true; }
            if x.hovered() && !crate::design_tokens::is_inspect_mode() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let _ = x.on_hover_text("Remove");
            body
        }).inner;
        (resp, x_clicked)
    }
}

// ─── DisplayChip ──────────────────────────────────────────────────────────────

/// Non-interactive status chip — the builder equivalent of `components_extra::display_chip`.
///
/// # Example
/// ```ignore
/// ui.add(DisplayChip::new("LIVE").color(live_green));
/// ```
#[must_use = "DisplayChip must be added with `ui.add(...)` to render"]
pub struct DisplayChip<'a> {
    label: &'a str,
    color: Color32,
}

impl<'a> DisplayChip<'a> {
    /// New display chip. You must call `.color(c)` to set the semantic color.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            color: status_ok(),
        }
    }

    /// Set the semantic color (fill tint + border + text).
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
}

impl<'a> Widget for DisplayChip<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // ChipVariant::Subtle/Outline (semantic-tinted, non-interactive), Size::Xs.
        let _variant = ChipVariant::Subtle;
        ChipBody {
            label: self.label,
            label_color: self.color,
            fill: color_alpha(self.color, alpha_tint()),
            border: color_alpha(self.color, alpha_dim()),
            radius: Radius::Pill.corner(),
            height: 14.0,
            pad_x: gap_md(),
            pad_y: 0.0,
            sense: egui::Sense::hover(),
            _marker: std::marker::PhantomData,
        }.render(ui)
    }
}

// ─── StatusBadge ──────────────────────────────────────────────────────────────

/// Status badge — small filled pill for things like DRAFT, ACTIVE, FILLED.
///
/// Returns `Response` and supports `ui.add(...)`. The legacy `style::status_badge`
/// returns `()` (calls `hit` internally); this builder version returns `Response`
/// so callers can inspect clicks if needed.
///
/// # Example
/// ```ignore
/// ui.add(StatusBadge::new("FILLED").color(t.bull));
/// ```
#[must_use = "StatusBadge must be added with `ui.add(...)` to render"]
pub struct StatusBadge<'a> {
    text: &'a str,
    color: Color32,
}

impl<'a> StatusBadge<'a> {
    /// New status badge.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            // TODO(design-tokens): semantic status color — replace with a theme token when available.
            color: Color32::from_rgb(100, 180, 120),
        }
    }

    /// Set the badge color (text + fill tint or border depending on style).
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
}

impl<'a> Widget for StatusBadge<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // ChipVariant::Subtle (semantic-tinted small badge), Size::Xs, pill radius.
        // Hairline-border mode flips to an Outline-like treatment.
        let _variant = ChipVariant::Subtle;
        let s = current();
        let (fill, stroke_w, stroke_col) = if s.hairline_borders {
            (Color32::TRANSPARENT, s.stroke_std, self.color)
        } else {
            (
                color_alpha(self.color, alpha_subtle()),
                stroke_thin(),
                color_alpha(self.color, alpha_dim()),
            )
        };
        let txt = if s.uppercase_section_labels {
            style_label_case(self.text)
        } else {
            self.text.to_string()
        };
        // Migrated to ui_kit::Button with Sense::hover() (display-only badge).
        // Note: rendered as proportional font (not monospace) — known visual delta
        // in this deprecated file. Use ui_kit::Tag/Badge for new call sites.
        let prev_pad = ui.spacing().button_padding;
        let resp = ui.add(
            KitButton::new(txt.as_str())
                .variant(KitVariant::Chrome)
                .fill(fill)
                .stroke(Stroke::new(stroke_w, stroke_col))
                .corner_radius_asymmetric(Radius::Pill.corner())
                .min_size(egui::vec2(0.0, crate::dt_f32!(badge.height, 16.0)))
                .sense(egui::Sense::hover())
                .fg(self.color),
        );
        ui.spacing_mut().button_padding = prev_pad;
        resp
    }
}

// ─── KeybindChip ──────────────────────────────────────────────────────────────

/// Keyboard shortcut hint chip — the builder equivalent of `components_extra::keybind_chip`.
///
/// # Example
/// ```ignore
/// ui.add(KeybindChip::new("Cmd+K").fg(t.dim).border(t.dim));
/// ```
#[must_use = "KeybindChip must be added with `ui.add(...)` to render"]
pub struct KeybindChip<'a> {
    hint: &'a str,
    fg: Color32,
    bg_border: Color32,
}

impl<'a> KeybindChip<'a> {
    /// New keybind chip.
    pub fn new(hint: &'a str) -> Self {
        Self {
            hint,
            fg: Color32::from_rgb(120, 120, 130),
            bg_border: Color32::from_rgb(120, 120, 130),
        }
    }

    /// Set foreground text color.
    pub fn fg(mut self, c: Color32) -> Self { self.fg = c; self }

    /// Set border/bg tint color.
    pub fn border(mut self, c: Color32) -> Self { self.bg_border = c; self }

    /// Convenience: set both fg and border from a single dim color.
    pub fn palette(mut self, fg: Color32, bg_border: Color32) -> Self {
        self.fg = fg;
        self.bg_border = bg_border;
        self
    }

    /// Pull colors from a Theme — fg and border both use `t.dim`.
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.palette(t.dim, t.dim)
    }
}

impl<'a> Widget for KeybindChip<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // ChipVariant::Outline (small monospace, Xs radius — non-pill keybind hint).
        let _variant = ChipVariant::Outline;
        let st = current();
        let cr = egui::CornerRadius::same(st.r_xs as u8);
        let stroke = if st.hairline_borders {
            Stroke::new(st.stroke_std, color_alpha(self.bg_border, alpha_strong()))
        } else {
            Stroke::new(st.stroke_thin, color_alpha(self.bg_border, alpha_muted()))
        };
        // Migrated to ui_kit::Button with Sense::hover() (display-only keybind chip).
        // Note: rendered as proportional font (not monospace) — known visual delta
        // in this deprecated file. Use ui_kit::Tag/Badge for new call sites.
        ui.add(
            KitButton::new(self.hint)
                .variant(KitVariant::Chrome)
                .fill(Color32::TRANSPARENT)
                .stroke(stroke)
                .corner_radius_asymmetric(cr)
                .min_size(egui::vec2(0.0, 14.0))
                .sense(egui::Sense::hover())
                .fg(self.fg),
        )
    }
}
