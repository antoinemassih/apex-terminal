//! Order-edit popup (double-click on an order badge).
//!
//! Call [`show_order_edit_dialog`] from gpu.rs inside the order-edit block.
//! Returns [`OrderEditOutput`] — apply the deferred mutations after the call.

use egui::Context;
use crate::chart_renderer::gpu::Theme;
use crate::chart_renderer::trading::{OrderSide, OrderLevel};
use crate::chart_renderer::ui::style::{color_alpha, dialog_header, dialog_separator_shadow};
use crate::ui_kit::icons::Icon;

/// Everything the dialog needs to read (no mutation — mutations come back via [`OrderEditOutput`]).
pub struct OrderEditCtx<'a> {
    pub ctx: &'a Context,
    pub t: &'a Theme,
    /// The egui::Window will use this id suffix so each order has a unique window.
    pub edit_id: u32,
    /// Pixel-Y of the order badge line.
    pub badge_y: f32,
    /// Horizontal centre of the badge in screen pixels.
    pub approx_badge_center: f32,
    /// Current price string (mutable — TextEdit writes into it each frame).
    pub edit_price: &'a mut String,
    /// Current qty string (mutable — TextEdit writes into it each frame).
    pub edit_qty: &'a mut String,
    /// Pre-extracted from the order.
    pub order_price: f32,
    pub order_label: String,
    pub order_color: egui::Color32,
    pub order_side: OrderSide,
    pub opt_sym: Option<String>,
    pub symbol: String,
}

/// Mutations to apply after [`show_order_edit_dialog`] returns.
pub struct OrderEditOutput {
    pub close_editor: bool,
    pub apply_price: Option<f32>,
    pub apply_qty: Option<u32>,
    pub cancel_it: bool,
}

/// Show the order-edit dialog and return the requested mutations.
///
/// gpu.rs owns all mutable state; this function only renders.
pub fn show_order_edit_dialog(c: OrderEditCtx<'_>) -> OrderEditOutput {
    let is_trigger = matches!(c.order_side, OrderSide::TriggerBuy | OrderSide::TriggerSell);
    let dialog_w = if is_trigger { 250.0 } else { 200.0 };
    let popup_pos = egui::pos2(c.approx_badge_center - dialog_w * 0.5, c.badge_y + 14.0);

    let mut close_editor = false;
    let mut apply_price: Option<f32> = None;
    let mut apply_qty: Option<u32> = None;
    let mut cancel_it = false;

    let title = if is_trigger {
        format!("EDIT {} TRIGGER", if c.order_side == OrderSide::TriggerBuy { "BUY" } else { "SELL" })
    } else {
        format!("EDIT {}", c.order_label)
    };

    egui::Window::new(format!("order_edit_{}", c.edit_id))
        .fixed_pos(popup_pos)
        .fixed_size(egui::vec2(dialog_w, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&c.ctx.style())
            .fill(c.t.toolbar_bg)
            .inner_margin(0.0)
            .stroke(egui::Stroke::new(0.5, color_alpha(c.t.toolbar_border, 60)))
            .corner_radius(6.0)
            .shadow(egui::epaint::Shadow {
                offset: [0, 4], blur: 12, spread: 2,
                color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 80),
            }))
        .show(c.ctx, |ui| {
            if dialog_header(ui, &title, c.t.dim) { close_editor = true; }
            ui.add_space(4.0);
            let m = 8.0;

            // Option contract info for trigger orders
            if is_trigger {
                if let Some(ref opt) = c.opt_sym {
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(egui::RichText::new(Icon::LIGHTNING).size(11.0).color(c.t.accent));
                        ui.label(egui::RichText::new(opt).monospace().size(11.0).strong().color(c.t.text));
                    });
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        let action = if c.order_side == OrderSide::TriggerBuy { "Buy option" } else { "Sell option" };
                        ui.label(egui::RichText::new(format!("{} when {} reaches trigger price", action, c.symbol))
                            .monospace().size(8.0).color(c.t.dim.gamma_multiply(0.6)));
                    });
                    ui.add_space(6.0);
                    dialog_separator_shadow(ui, m, color_alpha(c.t.toolbar_border, 40));
                    ui.add_space(4.0);
                }
            }

            // Price field
            let price_label = if is_trigger { "Trigger" } else { "Price" };
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new(format!("{:6}", price_label)).monospace().size(9.0).color(c.t.dim));
                ui.add_space(4.0);
                let resp = ui.add(egui::TextEdit::singleline(c.edit_price)
                    .desired_width(if is_trigger { 130.0 } else { 110.0 })
                    .font(egui::FontId::monospace(10.0))
                    .horizontal_align(egui::Align::RIGHT));
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(p) = c.edit_price.parse::<f32>() { apply_price = Some(p); }
                }
            });
            ui.add_space(4.0);

            // Qty stepper
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Qty   ").monospace().size(9.0).color(c.t.dim));
                ui.add_space(4.0);
                if ui.add(egui::Button::new(egui::RichText::new("-").monospace().size(12.0).color(c.t.dim))
                    .fill(color_alpha(c.t.toolbar_border, 25)).corner_radius(2.0).min_size(egui::vec2(20.0, 22.0))).clicked() {
                    if let Ok(q) = c.edit_qty.parse::<u32>() {
                        let step = if q > 100 { 10 } else if q > 10 { 5 } else { 1 };
                        let new_q = q.saturating_sub(step).max(1);
                        *c.edit_qty = format!("{}", new_q);
                        apply_qty = Some(new_q);
                    }
                }
                let resp = ui.add(egui::TextEdit::singleline(c.edit_qty)
                    .desired_width(if is_trigger { 80.0 } else { 60.0 })
                    .font(egui::FontId::monospace(10.0))
                    .horizontal_align(egui::Align::Center));
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(q) = c.edit_qty.parse::<u32>() { apply_qty = Some(q.max(1)); }
                }
                if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(12.0).color(c.t.dim))
                    .fill(color_alpha(c.t.toolbar_border, 25)).corner_radius(2.0).min_size(egui::vec2(20.0, 22.0))).clicked() {
                    if let Ok(q) = c.edit_qty.parse::<u32>() {
                        let step = if q >= 100 { 10 } else if q >= 10 { 5 } else { 1 };
                        let new_q = q + step;
                        *c.edit_qty = format!("{}", new_q);
                        apply_qty = Some(new_q);
                    }
                }
            });

            // Qty presets
            ui.horizontal(|ui| {
                ui.add_space(m + 44.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                for &preset in &[1u32, 5, 10, 25, 50, 100] {
                    let current_qty = c.edit_qty.parse::<u32>().unwrap_or(0);
                    let sel = current_qty == preset;
                    let fg = if sel { c.t.accent } else { c.t.dim.gamma_multiply(0.5) };
                    let bg = if sel { color_alpha(c.t.accent, 25) } else { egui::Color32::TRANSPARENT };
                    if ui.add(egui::Button::new(egui::RichText::new(format!("{}", preset)).monospace().size(8.0).color(fg))
                        .fill(bg).corner_radius(2.0).min_size(egui::vec2(24.0, 16.0))).clicked() {
                        *c.edit_qty = format!("{}", preset);
                        apply_qty = Some(preset);
                    }
                }
            });

            ui.add_space(4.0);
            dialog_separator_shadow(ui, m, color_alpha(c.t.toolbar_border, 40));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                let del_color = c.t.bear;
                if ui.add(egui::Button::new(egui::RichText::new(format!("{} Cancel", Icon::TRASH))
                    .monospace().size(9.0).color(del_color))
                    .fill(color_alpha(del_color, 15)).corner_radius(3.0)
                    .stroke(egui::Stroke::new(0.5, color_alpha(del_color, 60)))
                    .min_size(egui::vec2(0.0, 20.0))).clicked() {
                    cancel_it = true;
                }
            });
            ui.add_space(4.0);
        });

    OrderEditOutput { close_editor, apply_price, apply_qty, cancel_it }
}
