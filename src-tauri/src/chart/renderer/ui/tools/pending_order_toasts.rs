//! Pending order confirm/cancel toast notifications.

use egui::Context;
use crate::chart_renderer::gpu::{Theme, Chart};
use crate::chart_renderer::trading::OrderStatus;
use crate::chart_renderer::ui::style::{font_sm};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{Button as KitButton, Tooltip, OutlinedBox, tokens::Variant as KitVariant};
use crate::ui_kit::widgets::icon_placement::IconPlacement;

pub struct PendingOrderToastsCtx<'a> {
    pub ctx: &'a Context,
    pub t: &'a Theme,
    pub chart: &'a mut Chart,
    pub pane_idx: usize,
    /// Absolute Y of the bottom of the chart area (used to position toasts above panel).
    pub base_y: f32,
    /// Left edge of the chart rect.
    pub rect_left: f32,
}

pub fn show_pending_order_toasts(c: PendingOrderToastsCtx<'_>) {
    if c.chart.pending_confirms.is_empty() { return; }

    let mut confirm_ids: Vec<u32> = Vec::new();
    let mut cancel_ids: Vec<u32> = Vec::new();

    for (ci, (oid, _created)) in c.chart.pending_confirms.iter().enumerate() {
        let order_data = c.chart.orders.iter().find(|o| o.id == *oid)
            .map(|o| (o.label(), o.price, o.qty, o.color(c.t.bull, c.t.bear, c.t.accent)));
        if let Some((label, price, qty, color)) = order_data {
            let toast_y = c.base_y - ci as f32 * 34.0;
            // P6.6 migration: was raw `egui::Window::new(...)` with hand-rolled
            // `Frame::popup(...).fill(...).inner_margin(...).stroke(...)`.
            // The chart-relative stack position still needs egui::Area, but
            // the frame chrome is now `OutlinedBox` (with .border() carrying
            // the per-order side color — the API that audit P6.6 unblocked).
            egui::Area::new(egui::Id::new(format!("confirm_toast_{}_{}", c.pane_idx, oid)))
                .fixed_pos(egui::pos2(c.rect_left + 8.0, toast_y))
                .order(egui::Order::Foreground)
                .show(c.ctx, |ui| {
                    OutlinedBox::new()
                        .fill(c.t.toolbar_bg)
                        .border(color) // per-order side color (bull green / bear red)
                        .show(ui, c.t, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("{} x{} @ {:.2}", label, qty, price)).monospace().size(font_sm()).color(color));
                                let r = KitButton::icon(Icon::CHECK).variant(KitVariant::Ghost).placement(IconPlacement::Modal).tone_affirmative().show(ui, c.t);
                                Tooltip::new("Confirm order").show(ui, &r, c.t);
                                if r.clicked() { confirm_ids.push(*oid); }
                                let r = KitButton::icon(Icon::X).variant(KitVariant::Ghost).placement(IconPlacement::Modal).tone_destructive().show(ui, c.t);
                                Tooltip::new("Cancel order").show(ui, &r, c.t);
                                if r.clicked() { cancel_ids.push(*oid); }
                            });
                        });
                });
        } else {
            cancel_ids.push(*oid); // order was deleted
        }
    }

    // Apply confirms
    for id in &confirm_ids {
        crate::chart_renderer::trading::order_manager::confirm_order(*id as u64);
        if let Some(o) = c.chart.orders.iter_mut().find(|o| o.id == *id) {
            o.status = OrderStatus::Placed;
        }
    }
    // Apply cancels
    for id in &cancel_ids {
        crate::chart_renderer::trading::order_manager::cancel_order(*id as u64);
        crate::chart_renderer::trading::cancel_order_with_pair(&mut c.chart.orders, *id);
    }
    c.chart.pending_confirms.retain(|(id, _)| !confirm_ids.contains(id) && !cancel_ids.contains(id));
}
