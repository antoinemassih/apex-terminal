//! `NumericStepper` — full-width stepper with `[−]` left, `[+]` right,
//! editable value centered between with optional prefix/suffix.
//!
//! Universal across the app: order ticket prices, qty, indicator params,
//! anywhere a `[−] value [+]` pattern is used.
//!
//! ```ignore
//! NumericStepper::new(&mut state.limit_price)
//!     .prefix("$")
//!     .step(0.01)
//!     .min(0.0)
//!     .theme(t)
//!     .show(ui);
//! ```

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Stroke, Ui, Vec2};
use super::super::style::*;
use crate::ui_kit::widgets::{Button, tokens::Variant};

type Theme = crate::chart_renderer::gpu::Theme;

#[must_use = "NumericStepper must be shown via `.show(ui)`"]
pub struct NumericStepper<'a, V: NumericValue> {
    value: &'a mut V,
    prefix: Option<&'a str>,
    suffix: Option<&'a str>,
    step: f64,
    min: Option<f64>,
    max: Option<f64>,
    height: f32,
    theme: Option<&'a Theme>,
    decimals: usize,
}

/// Polymorphic numeric backing — `String` (for prices) or `u32` (for qty).
pub trait NumericValue {
    fn to_display(&self, decimals: usize) -> String;
    fn add(&mut self, delta: f64, decimals: usize, min: Option<f64>, max: Option<f64>);
    fn set_from_str(&mut self, s: &str);
    fn editable_buf(&self) -> String;
}

impl NumericValue for String {
    fn to_display(&self, _d: usize) -> String { self.clone() }
    fn add(&mut self, delta: f64, decimals: usize, min: Option<f64>, max: Option<f64>) {
        let cur = self.trim().parse::<f64>().unwrap_or(0.0);
        let mut next = cur + delta;
        if let Some(mn) = min { next = next.max(mn); }
        if let Some(mx) = max { next = next.min(mx); }
        *self = format!("{:.*}", decimals, next);
    }
    fn set_from_str(&mut self, s: &str) { *self = s.to_string(); }
    fn editable_buf(&self) -> String { self.clone() }
}

impl NumericValue for u32 {
    fn to_display(&self, _d: usize) -> String { self.to_string() }
    fn add(&mut self, delta: f64, _decimals: usize, min: Option<f64>, max: Option<f64>) {
        let cur = *self as f64;
        let mut next = (cur + delta).round();
        if let Some(mn) = min { next = next.max(mn); }
        if let Some(mx) = max { next = next.min(mx); }
        *self = next.max(0.0) as u32;
    }
    fn set_from_str(&mut self, s: &str) {
        if let Ok(v) = s.trim().parse::<u32>() { *self = v; }
    }
    fn editable_buf(&self) -> String { self.to_string() }
}

impl<'a, V: NumericValue> NumericStepper<'a, V> {
    pub fn new(value: &'a mut V) -> Self {
        Self {
            value,
            prefix: None,
            suffix: None,
            step: 1.0,
            min: None,
            max: None,
            height: 28.0,
            theme: None,
            decimals: 2,
        }
    }
    pub fn prefix(mut self, p: &'a str) -> Self { self.prefix = Some(p); self }
    pub fn suffix(mut self, s: &'a str) -> Self { self.suffix = Some(s); self }
    pub fn step(mut self, s: f64) -> Self { self.step = s; self }
    pub fn min(mut self, v: f64) -> Self { self.min = Some(v); self }
    pub fn max(mut self, v: f64) -> Self { self.max = Some(v); self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn decimals(mut self, d: usize) -> Self { self.decimals = d; self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme = match self.theme { Some(t) => t, None => &crate::chart_renderer::gpu::THEMES[0] };
        let dim   = theme.dim;
        let text  = theme.text;
        let border = theme.toolbar_border;
        let h = self.height;
        let btn_size = Vec2::new(h, h);

        let avail_w = ui.available_width();
        let prefix = self.prefix;
        let suffix = self.suffix;

        let mut buf = self.value.editable_buf();
        let resp = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // [−]
            let minus = ui.add(Button::new("\u{2212}")
                .variant(Variant::Chrome)
                .min_size(btn_size)
                .corner_radius(0.0));
            if minus.clicked() {
                self.value.add(-self.step, self.decimals, self.min, self.max);
                buf = self.value.editable_buf();
            }

            // central editable value with optional prefix/suffix
            let center_w = (avail_w - h * 2.0).max(40.0);
            ui.allocate_ui_with_layout(
                Vec2::new(center_w, h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_width(center_w);
                    let frame = egui::Frame::NONE
                        .fill(color_alpha(border, alpha_subtle()))
                        .stroke(Stroke::new(stroke_std(),
                            color_alpha(border, alpha_line())));
                    frame.show(ui, |ui| {
                        ui.set_min_height(h - 2.0);
                        ui.with_layout(
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                if let Some(p) = prefix {
                                    ui.add_space(gap_sm());
                                    ui.label(RichText::new(p).monospace()
                                        .size(font_sm()).color(color_subtle(dim)));
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if let Some(sf) = suffix {
                                        ui.add_space(gap_sm());
                                        ui.label(RichText::new(sf).monospace()
                                            .size(font_xs()).color(color_subtle(dim)));
                                    }
                                    let te = egui::TextEdit::singleline(&mut buf)
                                        .font(egui::FontId::monospace(font_md()))
                                        .text_color(text)
                                        .desired_width(ui.available_width())
                                        .horizontal_align(egui::Align::Center)
                                        .frame(false);
                                    let r = ui.add(te);
                                    if r.lost_focus() {
                                        self.value.set_from_str(&buf);
                                    }
                                });
                            });
                    });
                });

            // [+]
            let plus = ui.add(Button::new("+")
                .variant(Variant::Chrome)
                .min_size(btn_size)
                .corner_radius(0.0));
            if plus.clicked() {
                self.value.add(self.step, self.decimals, self.min, self.max);
            }
        });
        resp.response
    }
}
