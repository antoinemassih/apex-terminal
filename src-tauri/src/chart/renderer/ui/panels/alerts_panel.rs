//! Alerts management panel — shows all active and triggered alerts with controls.
//!
//! Migrated to the canonical ui_kit side-panel primitives:
//!   - `SidePanelShell` for outer chrome (was hand-rolled SidePanel + PanelFrame + PanelHeader)
//!   - `ui_kit::PanelSection` for section headers (default `.rule(true)` hairline)
//!   - `ui_kit::PanelListRow` for alert rows (leading status dot + symbol +
//!     secondary cmp/price line + trailing close button)
//!   - `ui_kit::PanelEmpty` for the "no alerts" state
//!
//! `kit::PanelInputRow` and `kit::PanelDualAction` are still used inside the
//! ADD ALERT section — those primitives have not yet been promoted to ui_kit.

use egui;
use super::super::style::*;
use crate::ui_kit::widgets::Input;
use crate::ui_kit::widgets::tokens::Size as KitSize;
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::widgets::{Button, PanelEmpty, PanelListRow, PanelSection, PanelTone, Tooltip};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use super::super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::trading::PriceAlert;
use crate::chart_renderer::commands::{AppCommand, UiCtx};
use super::kit::{PanelInputRow, PanelDualAction, Tone};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.alerts_panel_open { return; }
    let resp = SidePanelShell::new("alerts_panel", "ALERTS")
        .width(Width::Narrow)
        .pane_metrics(
            crate::chart_renderer::gpu::pane_tabs_header_h(watchlist),
            watchlist.pane_header_size.title_font(),
        )
        .show(ctx, t, |ui, t| {
            let cx = UiCtx::new(t);
            draw_content_cx(ui, watchlist, panes, ap, &cx);
        });
    if resp.close_clicked { watchlist.update_alerts_state(|s| s.alerts_panel_open = false); }
}

/// Tab body content (no SidePanel wrapper, no header). Used by signals_panel.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    let cx = UiCtx::new(t);
    draw_content_cx(ui, watchlist, panes, ap, &cx);
}

fn draw_content_cx(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    cx: &UiCtx<'_>,
) {
    let t = cx.theme;

    // ── Add Alert ──
    let current_price = panes[ap].bars.last().map(|b| b.close).unwrap_or(0.0);
    let sym = panes[ap].symbol.clone();
    let meta = format!("{} @ {:.2}", sym, current_price);

    PanelSection::new("ADD ALERT")
        .meta(meta)
        .show(ui, t, |ui, t| {
            PanelInputRow::new("Price").show(ui, t, |ui| {
                Input::new(&mut panes[ap].alert_input_price)
                    .min_width(80.0)
                    .size(KitSize::Sm)
                    .placeholder(format!("{:.2}", current_price))
                    .show(ui, t);
            });
            ui.add_space(gap_xs());

            let input_price = panes[ap].alert_input_price.parse::<f32>().unwrap_or(current_price);
            let above_label = format!("{} Above {:.2}", Icon::ARROW_FAT_UP, input_price);
            let below_label = format!("{} Below {:.2}", Icon::ARROW_FAT_DOWN, input_price);
            match PanelDualAction::new(
                (above_label.as_str(), Tone::Success),
                (below_label.as_str(), Tone::Danger),
            ).show(ui, t) {
                Some(0) => cx.dispatch(AppCommand::AddPriceAlert { pane: ap, price: input_price, above: true }),
                Some(1) => cx.dispatch(AppCommand::AddPriceAlert { pane: ap, price: input_price, above: false }),
                _ => {}
            }
        });

    // ── Drafts ──
    let pane_drafts: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| a.draft).cloned().map(move |a| (pi, a))
    ).collect();
    if !pane_drafts.is_empty() {
        let r = PanelSection::new("DRAFT")
            .count(pane_drafts.len())
            .action("Place All", PanelTone::Accent)
            .show(ui, t, |ui, t| {
                for (pi, alert) in &pane_drafts {
                    draft_row(ui, t, cx, *pi, alert);
                }
            });
        if r.action_clicked {
            cx.dispatch(AppCommand::PlaceAllDraftAlerts);
        }
    }

    // ── Active ──
    let active_alerts: Vec<_> = watchlist.alerts.iter().filter(|a| !a.triggered).cloned().collect();
    let triggered_alerts: Vec<_> = watchlist.alerts.iter().filter(|a| a.triggered).cloned().collect();

    let pane_active: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| !a.triggered && !a.draft).cloned().map(move |a| (pi, a))
    ).collect();
    let pane_triggered: Vec<(usize, PriceAlert)> = panes.iter().enumerate().flat_map(|(pi, p)|
        p.price_alerts.iter().filter(|a| a.triggered).cloned().map(move |a| (pi, a))
    ).collect();

    let total_active = active_alerts.len() + pane_active.len();
    let total_triggered = triggered_alerts.len() + pane_triggered.len();

    let mut active_section = PanelSection::new("ACTIVE")
        .title_color(t.accent)
        .count(total_active);
    if total_active > 0 {
        active_section = active_section.action("Clear All", PanelTone::Danger);
    }
    let active_resp = active_section.show(ui, t, |ui, t| {
        egui::ScrollArea::vertical()
            .id_salt("alerts_scroll")
            .max_height(ui.available_height() * 0.6)
            .show(ui, |ui| {
                if active_alerts.is_empty() && pane_active.is_empty() {
                    PanelEmpty::new("No active alerts").show(ui, t);
                }
                for alert in &active_alerts {
                    if alert_row(ui, t, &alert.symbol, alert.id, alert.price, alert.above, AlertState::Armed) {
                        cx.dispatch(AppCommand::CancelWatchlistAlert { id: alert.id });
                    }
                }
                for (pi, alert) in &pane_active {
                    if alert_row(ui, t, &alert.symbol, alert.id, alert.price, alert.above, AlertState::Armed) {
                        cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: alert.id });
                    }
                }
            });
    });
    if active_resp.action_clicked {
        for a in &active_alerts {
            cx.dispatch(AppCommand::CancelWatchlistAlert { id: a.id });
        }
        for (pi, a) in &pane_active {
            cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: a.id });
        }
    }

    // ── Triggered ──
    if total_triggered > 0 {
        let r = PanelSection::new("TRIGGERED")
            .count(total_triggered)
            .action("Dismiss All", PanelTone::Default)
            .show(ui, t, |ui, t| {
                egui::ScrollArea::vertical()
                    .id_salt("triggered_scroll")
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        for alert in &triggered_alerts {
                            if alert_row(ui, t, &alert.symbol, alert.id, alert.price, alert.above, AlertState::Triggered) {
                                cx.dispatch(AppCommand::CancelWatchlistAlert { id: alert.id });
                            }
                        }
                        for (pi, alert) in &pane_triggered {
                            if alert_row(ui, t, &alert.symbol, alert.id, alert.price, alert.above, AlertState::Triggered) {
                                cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: alert.id });
                            }
                        }
                    });
            });
        if r.action_clicked {
            for a in &triggered_alerts {
                cx.dispatch(AppCommand::CancelWatchlistAlert { id: a.id });
            }
            for (pi, a) in &pane_triggered {
                cx.dispatch(AppCommand::CancelPaneAlert { pane: *pi, id: a.id });
            }
        }
    }
}

// ── Row helpers ──────────────────────────────────────────────────────────────

#[derive(Copy, Clone)]
enum AlertState { Armed, Triggered, Draft }

impl AlertState {
    fn dot_color(self, t: &Theme) -> egui::Color32 {
        match self {
            AlertState::Armed => t.bull,
            AlertState::Triggered => t.accent,
            AlertState::Draft => t.dim,
        }
    }
}

/// Render a single alert row via `PanelListRow`. Returns `true` if the
/// trailing close button was clicked (caller dispatches the cancel command).
/// Accepts decomposed fields so it can render both `Alert` (watchlist) and
/// `PriceAlert` (pane) without a shared trait.
fn alert_row(
    ui: &mut egui::Ui,
    t: &Theme,
    symbol: &str,
    id: u32,
    price: f32,
    above: bool,
    state: AlertState,
) -> bool {
    let cmp = if above { ">" } else { "<" };
    let secondary = format!("{} {:.2}", cmp, price);
    let id_salt = format!("alert_{}", id);
    let dot_col = state.dot_color(t);
    let delete = std::cell::Cell::new(false);
    let delete_ref = &delete;

    PanelListRow::new(&id_salt)
        .leading(move |ui, _t| {
            ui.label(
                egui::RichText::new(Icon::CIRCLE)
                    .font(egui::FontId::proportional(font_md()))
                    .color(dot_col),
            );
        })
        .primary(symbol)
        .secondary(&secondary)
        .trailing(move |ui, t| {
            let r = Button::close().placement(IconPlacement::ListRow).show(ui, t);
            Tooltip::new("Delete alert").show(ui, &r, t);
            if r.clicked() { delete_ref.set(true); }
        })
        .dense(false)
        .show(ui, t);

    delete.get()
}

/// Draft row — like `alert_row` but with a leading "Place" action in the
/// trailing slot alongside the close button.
fn draft_row(
    ui: &mut egui::Ui,
    t: &Theme,
    cx: &UiCtx<'_>,
    pi: usize,
    alert: &PriceAlert,
) {
    let cmp = if alert.above { ">" } else { "<" };
    let secondary = format!("{} {:.2}  DRAFT", cmp, alert.price);
    let id_salt = format!("draft_{}_{}", pi, alert.id);
    let dot_col = AlertState::Draft.dot_color(t);

    let delete = std::cell::Cell::new(false);
    let place = std::cell::Cell::new(false);
    let delete_ref = &delete;
    let place_ref = &place;
    let accent = t.accent;

    PanelListRow::new(&id_salt)
        .leading(move |ui, _t| {
            ui.label(
                egui::RichText::new(Icon::CIRCLE)
                    .font(egui::FontId::proportional(font_md()))
                    .color(dot_col),
            );
        })
        .primary(&alert.symbol)
        .secondary(&secondary)
        .trailing(move |ui, t| {
            let r = Button::close().placement(IconPlacement::ListRow).show(ui, t);
            Tooltip::new("Discard draft").show(ui, &r, t);
            if r.clicked() { delete_ref.set(true); }
            if Button::small_action("Place").tint(accent).show(ui, t).clicked() {
                place_ref.set(true);
            }
        })
        .dense(false)
        .show(ui, t);

    if delete.get() {
        cx.dispatch(AppCommand::CancelPaneAlert { pane: pi, id: alert.id });
    }
    if place.get() {
        cx.dispatch(AppCommand::PlaceDraftAlert { pane: pi, id: alert.id });
    }
}
