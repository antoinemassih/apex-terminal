//! StatTile / QuoteTile / SparkTile — live-tile / dashboard card patterns.
//!
//! Each tile is a `PanelCard` shell composed with the shared `LiveFlash`
//! animated-value helper so the "price changed" pulse is identical across all
//! tiles and correctly theme-tuned.
//!
//! ```ignore
//! // Track the flash state (store in your component state):
//! let flash = LiveFlash::new();
//!
//! // Each frame, update from the latest value:
//! flash.tick(ui, id, price_f64);
//!
//! // Render a tile with the current flash:
//! StatTile::new("P&L", "+$2,340.50")
//!     .delta("+2.26%", Dir::Up)
//!     .flash(flash.dir())
//!     .show(ui, t);
//! ```
//!
//! Mirror of React `tiles.tsx` (`StatTile`, `QuoteTile`, `SparkTile`) and
//! `useLiveValue.ts` (`LiveFlash` / `FlashDir`).

use egui::{Color32, CornerRadius, Id, Rect, Ui};

use crate::ui_kit::style::{color_alpha, font_xs, font_sm, font_lg, gap_xs, radius_md};
use crate::ui_kit::widgets::sparkline::Sparkline;
use crate::ui_kit::widgets::tag::{Tag, TagTone};
use crate::ui_kit::widgets::theme::ComponentTheme;

// ── LiveFlash ─────────────────────────────────────────────────────────────────

/// Direction of a live-value flash.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Dir { #[default] Flat, Up, Down }

/// Flash state for a live-updating numeric value.
///
/// Stores the previous value + an animation timestamp in `egui::Memory`.
/// Call [`LiveFlash::tick`] each frame with the latest value to update,
/// then read [`LiveFlash::dir`] to know which flash overlay to paint.
#[derive(Clone, Copy)]
pub struct LiveFlash {
    dir:       Dir,
    flash_age: f32, // 0.0 = just fired, 1.0+ = expired
}

impl Default for LiveFlash {
    fn default() -> Self { Self::new() }
}

/// Decay duration in seconds — matches `--ds-flash-decay` (0.48s default).
const FLASH_DECAY_S: f32 = 0.48;

impl LiveFlash {
    pub fn new() -> Self { Self { dir: Dir::Flat, flash_age: 1.0 } }

    /// Update the flash state by comparing `value` to the stored previous.
    ///
    /// Stores state in `egui::Memory` keyed on `id`.  Must be called once
    /// per frame (usually right before rendering the tile).
    ///
    /// Returns the updated flash direction.
    pub fn tick(ui: &mut Ui, id: Id, value: f64) -> Dir {
        let dt = ui.input(|i| i.stable_dt);
        let (prev, dir, age) = ui.memory_mut(|m| {
            let state: (f64, Dir, f32) = *m.data.get_temp_mut_or(id, (value, Dir::Flat, 1.0));
            state
        });

        let (new_dir, new_age) = if value != prev {
            let d = if value > prev { Dir::Up } else { Dir::Down };
            (d, 0.0_f32)
        } else {
            let new_age = (age + dt / FLASH_DECAY_S).min(1.5);
            (dir, new_age)
        };

        ui.memory_mut(|m| {
            m.data.insert_temp(id, (value, new_dir, new_age));
        });

        // Request repaint while the flash is still active.
        if new_age < 1.0 {
            ui.ctx().request_repaint();
        }

        if new_age < 1.0 { new_dir } else { Dir::Flat }
    }

    /// Convenience: read the current flash dir without updating.
    /// (After `tick` has been called this frame.)
    pub fn read(ui: &Ui, id: Id) -> Dir {
        ui.memory(|m| {
            let (_, dir, age): (f64, Dir, f32) = *m.data.get_temp(id).unwrap_or(&(0.0, Dir::Flat, 1.0));
            if age < 1.0 { dir } else { Dir::Flat }
        })
    }
}

// ── Flash overlay color ───────────────────────────────────────────────────────

/// Bull/bear tint for the flash overlay.  Alpha fades over the decay window.
fn flash_color(t: &dyn ComponentTheme, dir: Dir, age: f32) -> Option<Color32> {
    if matches!(dir, Dir::Flat) { return None; }
    let alpha_f = ((1.0 - age.clamp(0.0, 1.0)) * 40.0) as u8; // max 40 alpha
    if alpha_f == 0 { return None; }
    let base = match dir {
        Dir::Up   => t.bull(),
        Dir::Down => t.bear(),
        Dir::Flat => unreachable!(),
    };
    Some(color_alpha(base, alpha_f))
}

/// Read `age` from memory; returns `None` if no active flash.
fn flash_age(ui: &Ui, id: Id) -> Option<(Dir, f32)> {
    ui.memory(|m| {
        let (_, dir, age): (f64, Dir, f32) = *m.data.get_temp(id).unwrap_or(&(0.0, Dir::Flat, 1.0));
        if age < 1.0 { Some((dir, age)) } else { None }
    })
}

/// Paint the flash overlay on the tile rect.
fn paint_flash_overlay(ui: &Ui, rect: Rect, t: &dyn ComponentTheme, id: Id) {
    if let Some((dir, age)) = flash_age(ui, id) {
        if let Some(color) = flash_color(t, dir, age) {
            let radius = CornerRadius::same(radius_md() as u8);
            ui.painter().rect_filled(rect, radius, color);
        }
    }
}

// ── Delta color ───────────────────────────────────────────────────────────────

fn dir_color(t: &dyn ComponentTheme, d: Dir) -> Color32 {
    match d {
        Dir::Up   => t.bull(),
        Dir::Down => t.bear(),
        Dir::Flat => t.dim(),
    }
}

// ── StatTile ──────────────────────────────────────────────────────────────────

/// Label + big value + delta KPI card — the "footer metric" tile.
///
/// Mirrors React `StatTile` from `tiles.tsx`.
pub struct StatTile<'a> {
    label:     &'a str,
    value:     &'a str,
    delta:     Option<&'a str>,
    delta_dir: Dir,
    value_size: f32,
    min_width:  f32,
    id:        Id,
}

impl<'a> StatTile<'a> {
    pub fn new(label: &'a str, value: &'a str) -> Self {
        Self {
            label,
            value,
            delta: None,
            delta_dir: Dir::Flat,
            value_size: font_lg(),
            min_width: 120.0,
            id: Id::new(label),
        }
    }

    pub fn delta(mut self, text: &'a str, dir: Dir) -> Self {
        self.delta = Some(text);
        self.delta_dir = dir;
        self
    }

    pub fn value_size(mut self, px: f32) -> Self { self.value_size = px; self }
    pub fn min_width(mut self, px: f32)  -> Self { self.min_width = px; self }

    /// Explicit ID (default: derived from `label`). Needed when multiple tiles
    /// share the same label string.
    pub fn id(mut self, id: impl std::hash::Hash) -> Self {
        self.id = Id::new(id);
        self
    }

    pub fn show<T: ComponentTheme>(self, ui: &mut Ui, t: &T) {
        let id = self.id;
        let bg = t.color_layer_up(1);
        let radius = CornerRadius::same(radius_md() as u8);

        let tile_rect = ui.available_rect_before_wrap();
        let frame = egui::Frame::NONE
            .fill(bg)
            .corner_radius(radius)
            .inner_margin(egui::Margin::same(gap_xs() as i8 + 2));

        frame.show(ui, |ui| {
            ui.set_min_width(self.min_width);

            ui.vertical(|ui| {
                // Label (uppercase, muted, xs).
                ui.label(
                    egui::RichText::new(self.label.to_ascii_uppercase())
                        .size(font_xs())
                        .color(t.dim()),
                );

                // Value (monospace, large).
                ui.label(
                    egui::RichText::new(self.value)
                        .size(self.value_size)
                        .monospace()
                        .color(t.text()),
                );

                // Delta.
                if let Some(delta) = self.delta {
                    ui.label(
                        egui::RichText::new(delta)
                            .size(font_xs())
                            .strong()
                            .monospace()
                            .color(dir_color(t, self.delta_dir)),
                    );
                }
            });
        });

        // Flash overlay (painted on top after the frame).
        let rect = ui.min_rect().union(tile_rect.intersect(ui.clip_rect()));
        paint_flash_overlay(ui, rect, t, id);
    }
}

// ── QuoteTile ─────────────────────────────────────────────────────────────────

/// Symbol + exchange pill + price + change — the hero price card.
///
/// Mirrors React `QuoteTile` from `tiles.tsx`.
pub struct QuoteTile<'a> {
    symbol:     &'a str,
    exchange:   Option<&'a str>,
    price:      &'a str,
    change:     Option<&'a str>,
    change_dir: Dir,
    price_size: f32,
    min_width:  f32,
    id:         Id,
}

impl<'a> QuoteTile<'a> {
    pub fn new(symbol: &'a str, price: &'a str) -> Self {
        Self {
            symbol,
            exchange:   None,
            price,
            change:     None,
            change_dir: Dir::Flat,
            price_size: font_lg() + 6.0, // "xl" — slightly bigger than stat
            min_width:  150.0,
            id:         Id::new(symbol),
        }
    }

    pub fn exchange(mut self, ex: &'a str) -> Self { self.exchange = Some(ex); self }
    pub fn change(mut self, text: &'a str, dir: Dir) -> Self {
        self.change = Some(text);
        self.change_dir = dir;
        self
    }
    pub fn price_size(mut self, px: f32) -> Self { self.price_size = px; self }
    pub fn min_width(mut self,  px: f32) -> Self { self.min_width = px; self }
    pub fn id(mut self, id: impl std::hash::Hash) -> Self { self.id = Id::new(id); self }

    pub fn show<T: ComponentTheme>(self, ui: &mut Ui, t: &T) {
        let id = self.id;
        let bg = t.bg();
        let radius = CornerRadius::same(radius_md() as u8);
        let pad = gap_xs() as i8 + 2;

        let frame = egui::Frame::NONE
            .fill(bg)
            .corner_radius(radius)
            .inner_margin(egui::Margin::same(pad));

        frame.show(ui, |ui| {
            ui.set_min_width(self.min_width);
            ui.vertical(|ui| {
                // Symbol row: symbol label + exchange pill.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap_xs();
                    ui.label(
                        egui::RichText::new(self.symbol)
                            .size(font_sm())
                            .strong()
                            .monospace()
                            .color(t.text()),
                    );
                    if let Some(ex) = self.exchange {
                        Tag::new(ex)
                            .tone(TagTone::Neutral)
                            .size(crate::ui_kit::widgets::tokens::Size::Xs)
                            .show(ui, t);
                    }
                });

                // Price (large mono).
                ui.label(
                    egui::RichText::new(self.price)
                        .size(self.price_size)
                        .monospace()
                        .color(t.text()),
                );

                // Change.
                if let Some(change) = self.change {
                    ui.label(
                        egui::RichText::new(change)
                            .size(font_sm())
                            .strong()
                            .monospace()
                            .color(dir_color(t, self.change_dir)),
                    );
                }
            });
        });

        let rect = ui.min_rect();
        paint_flash_overlay(ui, rect, t, id);
    }
}

// ── SparkTile (the live tile) ─────────────────────────────────────────────────

/// Label + value + sparkline + delta — the live data tile.
///
/// Mirrors React `SparkTile` from `tiles.tsx`.
pub struct SparkTile<'a> {
    label:       &'a str,
    value:       &'a str,
    data:        &'a [f32],
    delta:       Option<&'a str>,
    change_dir:  Dir,
    spark_height: f32,
    spark_color: Option<Color32>,
    min_width:   f32,
    id:          Id,
}

impl<'a> SparkTile<'a> {
    pub fn new(label: &'a str, value: &'a str, data: &'a [f32]) -> Self {
        Self {
            label,
            value,
            data,
            delta:       None,
            change_dir:  Dir::Flat,
            spark_height: 40.0,
            spark_color: None,
            min_width:   150.0,
            id:          Id::new(label),
        }
    }

    pub fn delta(mut self, text: &'a str, dir: Dir) -> Self {
        self.delta = Some(text);
        self.change_dir = dir;
        self
    }
    pub fn spark_color(mut self, c: Color32)   -> Self { self.spark_color = Some(c); self }
    pub fn spark_height(mut self, px: f32)     -> Self { self.spark_height = px; self }
    pub fn min_width(mut self, px: f32)        -> Self { self.min_width = px; self }
    pub fn id(mut self, id: impl std::hash::Hash) -> Self { self.id = Id::new(id); self }

    pub fn show<T: ComponentTheme>(self, ui: &mut Ui, t: &T) {
        let id = self.id;
        let bg = t.color_layer_up(1);
        let radius = CornerRadius::same(radius_md() as u8);
        let pad = gap_xs() as i8 + 2;

        let spark_color = self.spark_color.unwrap_or_else(|| dir_color(t, self.change_dir));

        let frame = egui::Frame::NONE
            .fill(bg)
            .corner_radius(radius)
            .inner_margin(egui::Margin::same(pad));

        frame.show(ui, |ui| {
            ui.set_min_width(self.min_width);
            ui.vertical(|ui| {
                // Label row (left) + delta (right).
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(self.label.to_ascii_uppercase())
                            .size(font_xs())
                            .strong()
                            .color(t.dim()),
                    );
                    if let Some(delta) = self.delta {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(delta)
                                    .size(font_xs())
                                    .strong()
                                    .monospace()
                                    .color(dir_color(t, self.change_dir)),
                            );
                        });
                    }
                });

                // Value (medium mono).
                ui.label(
                    egui::RichText::new(self.value)
                        .size(font_lg())
                        .monospace()
                        .color(t.text()),
                );

                // Sparkline.
                if !self.data.is_empty() {
                    Sparkline::new(self.data)
                        .color(spark_color)
                        .size(ui.available_width(), self.spark_height)
                        .show(ui, t);
                }
            });
        });

        let rect = ui.min_rect();
        paint_flash_overlay(ui, rect, t, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_default_is_flat() {
        assert_eq!(Dir::default(), Dir::Flat);
    }

    #[test]
    fn live_flash_default() {
        let f = LiveFlash::new();
        assert_eq!(f.dir, Dir::Flat);
        assert!(f.flash_age >= 1.0, "new flash should be expired by default");
    }
}
