//! Order-edit popup (double-click on an order badge).
//!
//! Call [`show_order_edit_dialog`] from gpu.rs inside the order-edit block.
//! Returns [`OrderEditOutput`] — apply the deferred mutations after the call.

use egui::Context;
use crate::chart_renderer::gpu::Theme;
use crate::chart_renderer::trading::OrderSide;
use crate::chart_renderer::ui::style::{color_alpha, color_half, color_muted, dialog_separator_shadow, gap_xs, gap_sm, gap_md, gap_lg, font_xs, font_sm, font_md, row_height_default};
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use crate::ui_kit::widgets::Input;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{Button as KitButton, tokens::{Variant as KitVariant, Size as KitSize}};

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

    // Migrated to ToolPopover (2026-05-26).
    let portable_t = crate::chart_renderer::theme_impl::theme_to_portable(c.t);
    let dialog_id = format!("order_edit_{}", c.edit_id);
    let resp = crate::ui_kit::widgets::ToolPopover::new()
        .id(&dialog_id)
        .width(dialog_w)
        .pos(popup_pos)
        .title(&title)
        .show(c.ctx, &portable_t, |ui| {
            let m = gap_lg();

            // Option contract info for trigger orders
            if is_trigger {
                if let Some(ref opt) = c.opt_sym {
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        ui.label(TextStyle::Body.as_rich(Icon::LIGHTNING, c.t.accent));
                        ui.label(egui::RichText::new(opt).monospace().size(font_md()).strong().color(c.t.text));
                    });
                    ui.add_space(gap_xs());
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        let action = if c.order_side == OrderSide::TriggerBuy { "Buy option" } else { "Sell option" };
                        ui.label(egui::RichText::new(format!("{} when {} reaches trigger price", action, c.symbol))
                            .monospace().size(font_xs()).color(color_muted(c.t.dim)));
                    });
                    ui.add_space(gap_md());
                    dialog_separator_shadow(ui, m, color_alpha(c.t.toolbar_border, 40));
                    ui.add_space(gap_xs());
                }
            }

            // Price field
            let price_label = if is_trigger { "Trigger" } else { "Price" };
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new(format!("{:6}", price_label)).monospace().size(font_sm()).color(c.t.dim));
                ui.add_space(gap_sm());
                let resp = Input::new(c.edit_price)
                    .width(if is_trigger { 130.0 } else { 110.0 })
                    .font_size(font_sm())
                    .horizontal_align(egui::Align::RIGHT)
                    .frameless(true)
                    .show(ui, c.t);
                if resp.lost_focus && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(p) = c.edit_price.parse::<f32>() {
                        if p.is_finite() && p > 0.0 { apply_price = Some(p); }
                    }
                }
            });
            ui.add_space(gap_sm());

            // Qty stepper
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("Qty   ").monospace().size(font_sm()).color(c.t.dim));
                ui.add_space(gap_sm());
                if KitButton::new("-").variant(KitVariant::Ghost).size(KitSize::Sm)
                    .fg(c.t.dim).min_size(egui::vec2(20.0, row_height_default())).show(ui, c.t).clicked() {
                    if let Ok(q) = c.edit_qty.parse::<u32>() {
                        let step = if q > 100 { 10 } else if q > 10 { 5 } else { 1 };
                        let new_q = q.saturating_sub(step).max(1);
                        *c.edit_qty = format!("{}", new_q);
                        apply_qty = Some(new_q);
                    }
                }
                let resp = Input::new(c.edit_qty)
                    .width(if is_trigger { 80.0 } else { 60.0 })
                    .font_size(font_sm())
                    .horizontal_align(egui::Align::Center)
                    .frameless(true)
                    .show(ui, c.t);
                if resp.lost_focus && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(q) = c.edit_qty.parse::<u32>() { apply_qty = Some(q.clamp(1, 99_999)); }
                }
                if KitButton::new("+").variant(KitVariant::Ghost).size(KitSize::Sm)
                    .fg(c.t.dim).min_size(egui::vec2(20.0, row_height_default())).show(ui, c.t).clicked() {
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
                ui.spacing_mut().item_spacing.x = gap_xs();
                for &preset in &[1u32, 5, 10, 25, 50, 100] {
                    let current_qty = c.edit_qty.parse::<u32>().unwrap_or(0);
                    let sel = current_qty == preset;
                    let fg = if sel { c.t.accent } else { color_half(c.t.dim) };
                    let bg = if sel { color_alpha(c.t.accent, 25) } else { egui::Color32::TRANSPARENT };
                    let preset_lbl = format!("{}", preset);
                    if KitButton::toggle(preset_lbl.as_str(), sel).size(KitSize::Xs)
                        .show(ui, c.t).clicked() {
                        *c.edit_qty = format!("{}", preset);
                        apply_qty = Some(preset);
                    }
                }
            });

            ui.add_space(gap_sm());
            dialog_separator_shadow(ui, m, color_alpha(c.t.toolbar_border, 40));
            ui.add_space(gap_sm());
            ui.horizontal(|ui| {
                ui.add_space(m);
                let del_color = c.t.bear;
                if KitButton::new("Cancel").leading_icon(Icon::TRASH).variant(KitVariant::Ghost).fg(del_color).show(ui, c.t).clicked() {
                    cancel_it = true;
                }
            });
            ui.add_space(gap_sm());
        });
    if resp.dismissed { close_editor = true; }

    OrderEditOutput { close_editor, apply_price, apply_qty, cancel_it }
}
