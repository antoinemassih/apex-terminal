//! Order Ledger side panel — surfaces every order intent, state transition,
//! broker ack/fail and rejection reason so the operator has full visibility
//! into the order subsystem.
//!
//! Two stacked sections:
//!   1. Active Orders — every non-terminal order globally, sourced from the
//!      Wave 3 lock-free `orders_snapshot()`. Symbols are pulled from
//!      `ManagedOrder.symbol`; we no longer index per-pane.
//!   2. Journal feed — recent events from the WAL ring buffer
//!      (`trading::journal::tail`). Events are colored by kind and filtered
//!      via chips (All / Submits / Cancels / Modifies / Rejects) plus a
//!      symbol/client-id search box.

use egui;

use super::super::style::{self, *};
use super::super::super::gpu::{Watchlist, Chart, Theme};
use super::super::widgets::frames::PanelFrame;
use super::super::widgets::headers::PanelHeaderWithClose;
use super::super::widgets::text::{SectionLabel, MonospaceCode};
use crate::ui_kit::widgets::Input;
use crate::ui_kit::widgets::tokens::Size as KitSize;
use crate::chart_renderer::trading::OrderSide;
use crate::chart_renderer::trading::order_manager::{self, OrderState};
use crate::chart_renderer::trading::journal::{self, JournalEvent, AttemptKind};

/// Lightweight active-order row, derived from the lock-free snapshot.
/// Avoids cloning the full `ManagedOrder` per render frame.
struct ActiveRow {
    id: u64,
    symbol: String,
    side: OrderSide,
    state: OrderState,
    qty: u32,
    filled_qty: u32,
    price: f32,
    /// First 8 chars of the persistent client_order_id (UUID v4 hex).
    cid8: String,
    updated_at: u64,
}

/// Filter chip selection for the journal feed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LedgerFilter { All, Submits, Cancels, Modifies, Rejects }

impl LedgerFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "All", Self::Submits => "Submits", Self::Cancels => "Cancels",
            Self::Modifies => "Modifies", Self::Rejects => "Rejects",
        }
    }
    fn all() -> [Self; 5] { [Self::All, Self::Submits, Self::Cancels, Self::Modifies, Self::Rejects] }
    fn matches(self, ev: &JournalEvent) -> bool {
        match (self, ev) {
            (Self::All, _) => true,
            (Self::Submits,  JournalEvent::Attempt { kind: AttemptKind::Submit, .. }) => true,
            (Self::Cancels,  JournalEvent::Attempt { kind: AttemptKind::Cancel, .. })
            | (Self::Cancels, JournalEvent::Attempt { kind: AttemptKind::CancelAll, .. }) => true,
            (Self::Modifies, JournalEvent::Attempt { kind: AttemptKind::Modify, .. }) => true,
            (Self::Rejects,  JournalEvent::Fail { .. }) => true,
            _ => false,
        }
    }
}

/// View toggle: which section gets rendered.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LedgerView { Active, Journal, All }

impl LedgerView {
    fn label(self) -> &'static str {
        match self { Self::Active => "Active", Self::Journal => "Journal", Self::All => "All" }
    }
    fn all() -> [Self; 3] { [Self::Active, Self::Journal, Self::All] }
}

/// Top-level draw entry. Mirrors the shape of `journal_panel::draw` /
/// `alerts_panel::draw`. Only renders when `watchlist.order_ledger_open`.
pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    _panes: &[Chart],
    t: &Theme,
) {
    if !watchlist.order_ledger_open { return; }

    egui::SidePanel::right("order_ledger_panel")
        .default_width(380.0)
        .min_width(280.0)
        .max_width(560.0)
        .resizable(true)
        .frame(PanelFrame::new(t.toolbar_bg, t.toolbar_border).build())
        .show(ctx, |ui| {
            // ── Header + close ───────────────────────────────────────────
            if PanelHeaderWithClose::new("ORDER LEDGER").theme(t).watchlist(watchlist).show(ui) {
                watchlist.order_ledger_open = false;
            }
            separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
            ui.add_space(gap_xs());

            // Snapshot of journal + counts. Read once per frame.
            //
            // Active orders come from the lock-free `orders_snapshot()` so
            // multi-pane render doesn't contend with the manager mutex.
            // Mutations propagate via `snapshot::publish` after every state
            // change in `order_manager`.
            let events = journal::tail(200);
            let active_orders = collect_active_from_snapshot();
            let pending_count = active_orders.iter()
                .filter(|r| matches!(r.state, OrderState::Draft))
                .count();
            let active_count = active_orders.len();
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
            let one_hour_ago = now_ms.saturating_sub(60 * 60 * 1000);
            let rejected_last_hour = events.iter().filter(|e| match e {
                JournalEvent::Fail { ts_ms, .. } => *ts_ms >= one_hour_ago,
                _ => false,
            }).count();

            // Counts strip
            ui.horizontal(|ui| {
                ui.add(MonospaceCode::new(&format!("Active {}", active_count)).size_px(font_sm()).color(t.accent).strong(true));
                ui.add_space(gap_sm());
                ui.add(MonospaceCode::new(&format!("Pending {}", pending_count)).size_px(font_sm()).color(t.warn));
                ui.add_space(gap_sm());
                ui.add(MonospaceCode::new(&format!("Rejected/1h {}", rejected_last_hour)).size_px(font_sm()).color(t.bear));
            });
            ui.add_space(gap_xs());

            // View selector (Active / Journal / All)
            ui.horizontal(|ui| {
                ui.add(MonospaceCode::new("View").size_px(font_sm()).color(t.dim));
                for v in LedgerView::all() {
                    let active = watchlist.order_ledger_view == v as u8;
                    let bg = if active { color_alpha(t.accent, alpha_strong()) } else { color_alpha(t.toolbar_border, alpha_ghost()) };
                    let fg = if active { t.accent } else { t.dim };
                    let resp = ui.add(egui::Label::new(
                        egui::RichText::new(v.label()).monospace().size(font_sm()).color(fg)
                    ).sense(egui::Sense::click()));
                    let r = resp.rect.expand2(egui::vec2(4.0, 1.0));
                    ui.painter().rect_filled(r, radius_sm(), bg);
                    if resp.clicked() { watchlist.order_ledger_view = v as u8; }
                }
            });
            ui.add_space(gap_xs());

            let view = match watchlist.order_ledger_view {
                0 => LedgerView::Active, 1 => LedgerView::Journal, _ => LedgerView::All,
            };

            // ── Section 1: Active Orders ─────────────────────────────────
            if matches!(view, LedgerView::Active | LedgerView::All) {
                ui.add(SectionLabel::new(&format!("ACTIVE ORDERS ({})", active_count)).tiny().color(t.accent));
                ui.add_space(gap_xs());

                let max_h = if matches!(view, LedgerView::All) {
                    (ui.available_height() * 0.45).max(120.0)
                } else {
                    ui.available_height() - gap_sm()
                };

                egui::ScrollArea::vertical()
                    .id_salt("ledger_active")
                    .max_height(max_h)
                    .show(ui, |ui| {
                        if active_orders.is_empty() {
                            ui.add_space(gap_sm());
                            ui.add(MonospaceCode::new("No active orders").size_px(font_sm()).color(t.dim).gamma(0.5));
                            ui.add_space(gap_sm());
                        } else {
                            // Column header
                            ui.horizontal(|ui| {
                                ui.add(MonospaceCode::new("TIME    SYM   SIDE TYPE  QTY/FILL  PRICE   STATE      SRC  CID")
                                    .size_px(font_xs()).color(t.dim).gamma(0.4));
                            });
                            ui.add_space(2.0);
                            for row in &active_orders {
                                draw_active_row(ui, t, row);
                            }
                        }
                    });
                ui.add_space(gap_xs());
            }

            // ── Section 2: Journal feed ──────────────────────────────────
            if matches!(view, LedgerView::Journal | LedgerView::All) {
                separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
                ui.add_space(gap_xs());
                ui.add(SectionLabel::new(&format!("JOURNAL ({})", events.len())).tiny().color(t.accent));
                ui.add_space(gap_xs());

                // Filter chips
                ui.horizontal(|ui| {
                    let cur = LedgerFilter::all()[watchlist.order_ledger_filter as usize % 5];
                    for (i, f) in LedgerFilter::all().iter().enumerate() {
                        let active = *f == cur;
                        let bg = if active { color_alpha(t.accent, alpha_strong()) } else { color_alpha(t.toolbar_border, alpha_ghost()) };
                        let fg = if active { t.accent } else { t.dim };
                        let resp = ui.add(egui::Label::new(
                            egui::RichText::new(f.label()).monospace().size(font_xs()).color(fg)
                        ).sense(egui::Sense::click()));
                        ui.painter().rect_filled(resp.rect.expand2(egui::vec2(3.0, 1.0)), radius_sm(), bg);
                        if resp.clicked() { watchlist.order_ledger_filter = i as u8; }
                    }
                });
                ui.add_space(2.0);

                // Search box
                ui.horizontal(|ui| {
                    ui.add(MonospaceCode::new("Find").size_px(font_xs()).color(t.dim));
                    Input::new(&mut watchlist.order_ledger_search)
                        .placeholder("symbol or client-id")
                        .size(KitSize::Sm)
                        .min_width(ui.available_width().min(180.0))
                        .clearable(true)
                        .show(ui, t);
                });
                ui.add_space(gap_xs());

                let filter = LedgerFilter::all()[watchlist.order_ledger_filter as usize % 5];
                let q = watchlist.order_ledger_search.trim().to_ascii_lowercase();

                let rows: Vec<&JournalEvent> = events.iter()
                    .filter(|e| filter.matches(e))
                    .filter(|e| {
                        if q.is_empty() { return true; }
                        let cid = e.client_id().to_ascii_lowercase();
                        if cid.contains(&q) { return true; }
                        // For Attempt events the symbol may be in the JSON payload.
                        if let JournalEvent::Attempt { payload, .. } = e {
                            if let Some(s) = payload.get("symbol").and_then(|v| v.as_str()) {
                                if s.to_ascii_lowercase().contains(&q) { return true; }
                            }
                        }
                        false
                    })
                    .collect();

                let area = egui::ScrollArea::vertical()
                    .id_salt("ledger_journal")
                    .stick_to_bottom(true)
                    .auto_shrink([false; 2]);
                area.show(ui, |ui| {
                    if rows.is_empty() {
                        ui.add_space(gap_sm());
                        ui.add(MonospaceCode::new("No journal events").size_px(font_sm()).color(t.dim).gamma(0.5));
                    } else {
                        for ev in rows {
                            draw_journal_row(ui, t, ev);
                        }
                    }
                });
            }
        });
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Collect every non-terminal order globally from the lock-free snapshot.
/// Render-thread read; does not lock the order manager.
fn collect_active_from_snapshot() -> Vec<ActiveRow> {
    let snap = order_manager::orders_snapshot();
    let mut out: Vec<ActiveRow> = snap.orders.iter()
        .filter(|o| o.state.is_active())
        .map(|o| ActiveRow {
            id: o.id,
            symbol: o.symbol.clone(),
            side: o.side,
            state: o.state,
            qty: o.qty,
            filled_qty: o.filled_qty,
            price: o.price,
            cid8: o.client_order_id.chars().take(8).collect(),
            updated_at: o.updated_at,
        })
        .collect();
    // Newest first so freshly placed orders appear at the top.
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

fn draw_active_row(ui: &mut egui::Ui, t: &Theme, row: &ActiveRow) {
    let side_text = match row.side {
        OrderSide::Buy | OrderSide::TriggerBuy => "BUY",
        OrderSide::Sell | OrderSide::TriggerSell => "SELL",
        OrderSide::Stop | OrderSide::OcoStop => "STOP",
        OrderSide::OcoTarget => "OCO",
    };
    let side_color = match row.side {
        OrderSide::Buy | OrderSide::TriggerBuy | OrderSide::OcoTarget => t.bull,
        _ => t.bear,
    };
    let (state_text, state_color) = match row.state {
        OrderState::Draft         => ("Pending", t.warn),
        OrderState::PendingSubmit => ("PendSub", t.warn),
        OrderState::Working       => ("Working", t.accent),
        OrderState::PartialFill   => ("PartFil", t.accent),
        OrderState::Filled        => ("Filled",  t.bull),
        OrderState::PendingCancel => ("PendCxl", t.dim),
        OrderState::Cancelled     => ("Cancel",  t.dim),
        OrderState::Rejected      => ("Reject",  t.bear),
        OrderState::PendingModify => ("PendMod", t.warn),
        OrderState::Unknown       => ("Unknown", t.dim),
    };

    let resp = ui.horizontal(|ui| {
        let time_label = format_hms(row.updated_at);
        ui.add(MonospaceCode::new(&time_label).size_px(font_xs()).color(t.dim).gamma(0.5));
        ui.add(MonospaceCode::new(&format!("{:<5}", row.symbol)).size_px(font_xs()).color(t.text));
        ui.add(MonospaceCode::new(&format!("{:<4}", side_text)).size_px(font_xs()).strong(true).color(side_color));
        ui.add(MonospaceCode::new("LMT ").size_px(font_xs()).color(t.dim));
        ui.add(MonospaceCode::new(&format!("{:>3}/{:<2}", row.qty, row.filled_qty)).size_px(font_xs()).color(t.text));
        ui.add(MonospaceCode::new(&format!("{:>7.2}", row.price)).size_px(font_xs()).color(t.text));
        ui.add(MonospaceCode::new(&format!(" {:<8}", state_text)).size_px(font_xs()).strong(true).color(state_color));
        ui.add(MonospaceCode::new(" UI ").size_px(font_xs()).color(t.dim).gamma(0.5));
        ui.add(MonospaceCode::new(&format!("{:<8}", row.cid8)).size_px(font_xs()).color(t.dim).gamma(0.5));
    }).response.interact(egui::Sense::click());

    // Right-click → cancel via global API. The snapshot exposes the
    // canonical u64 id, so cancellation no longer relies on the legacy
    // truncated u32.
    let order_id = row.id;
    resp.context_menu(|ui| {
        if ui.button("Cancel").clicked() {
            crate::chart_renderer::trading::order_manager::cancel_order(order_id);
            ui.close_menu();
        }
    });
}

fn draw_journal_row(ui: &mut egui::Ui, t: &Theme, ev: &JournalEvent) {
    let (kind_label, kind_color, summary, ts_ms) = match ev {
        JournalEvent::Attempt { kind, ts_ms, payload, .. } => {
            let k = match kind {
                AttemptKind::Submit => "SUBMIT", AttemptKind::Cancel => "CANCEL",
                AttemptKind::CancelAll => "CXALL", AttemptKind::Modify => "MODIFY",
            };
            let sym = payload.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
            (k, t.text, format!("{}", sym), *ts_ms)
        }
        JournalEvent::Ack  { backend_id, ts_ms, .. } => {
            ("ACK   ", t.bull, backend_id.clone().unwrap_or_default(), *ts_ms)
        }
        JournalEvent::Fail { reason, ts_ms, .. } => {
            ("FAIL  ", t.bear, reason.clone(), *ts_ms)
        }
        JournalEvent::StateChg { from, to, ts_ms, .. } => {
            ("STATE ", t.dim, format!("{:?} -> {:?}", from, to), *ts_ms)
        }
        JournalEvent::Reconcile { local, broker, resolution, ts_ms, .. } => {
            ("RECON ", style::order_state_recon(),
             format!("{:?}/{} -> {}", local, broker, resolution), *ts_ms)
        }
        JournalEvent::Control { kind, ts_ms } => {
            ("CTRL  ", style::order_state_ctrl(),
             format!("{:?}", kind), *ts_ms)
        }
        JournalEvent::Shutdown { ts_ms } => {
            ("SHUTDN", t.dim, "graceful shutdown".into(), *ts_ms)
        }
    };

    let cid8: String = ev.client_id().chars().take(8).collect();
    let time_label = format_hms(ts_ms);

    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new(&time_label).size_px(font_xs()).color(t.dim).gamma(0.5));
        ui.add(MonospaceCode::new(kind_label).size_px(font_xs()).strong(true).color(kind_color));
        ui.add(MonospaceCode::new(&format!("{:<8}", cid8)).size_px(font_xs()).color(t.dim));
        ui.add(MonospaceCode::new(&summary).size_px(font_xs()).color(t.text));
    });
}

fn format_hms(ts_ms: u64) -> String {
    use chrono::TimeZone;
    let secs = (ts_ms / 1000) as i64;
    let nsecs = ((ts_ms % 1000) * 1_000_000) as u32;
    match chrono::Local.timestamp_opt(secs, nsecs).single() {
        Some(dt) => dt.format("%H:%M:%S").to_string(),
        None => "--:--:--".into(),
    }
}
