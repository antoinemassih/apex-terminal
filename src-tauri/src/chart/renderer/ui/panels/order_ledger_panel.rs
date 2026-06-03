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
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;

use super::super::style::{self, *};
use super::super::super::gpu::{Watchlist, Chart, Theme};
use super::super::components::text::{SectionLabel, MonospaceCode};
use crate::ui_kit::widgets::Input;
use crate::ui_kit::widgets::tokens::{Size as KitSize, Variant};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use crate::ui_kit::widgets::{Button, MenuItem, PanelSubSection, PanelListRow, PanelColumn};
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::trading::OrderSide;
use crate::chart_renderer::trading::order_manager::{self, OrderState};
use crate::chart_renderer::trading::journal::{self, JournalEvent, AttemptKind};
use crate::data::connectivity::errors_sink::{report, ErrorLevel};

/// Lightweight active-order row, derived from the lock-free snapshot.
/// Avoids cloning the full `ManagedOrder` per render frame.
pub(crate) struct ActiveRow {
    pub(crate) id: u64,
    pub(crate) symbol: String,
    pub(crate) side: OrderSide,
    pub(crate) state: OrderState,
    pub(crate) qty: u32,
    pub(crate) filled_qty: u32,
    pub(crate) price: f32,
    /// First 8 chars of the persistent client_order_id (UUID v4 hex).
    pub(crate) cid8: String,
    pub(crate) updated_at: u64,
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
/// Rail registration — see [`super::right_rail`].
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "order_ledger",
    is_open: |w| w.order_ledger_open,
    render: |cx, slot| draw(cx.ctx, cx.watchlist, cx.panes, cx.t, Some(slot)),
};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    _panes: &[Chart],
    t: &Theme,
    slot: Option<super::side_panel_shell::RailSlot>,
) {
    if !watchlist.order_ledger_open { return; }

    let pane_h    = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();
    let resp = SidePanelShell::new("order_ledger_panel", "ORDER LEDGER")
        .width(Width::Medium)
        .resizable(280.0..=560.0)
        .pane_metrics(pane_h, pane_font)
        .rail_slot(slot)
        .show(ctx, t, |ui, t| {
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
                    let bg = if active { tint(t, Tone::Accent, alpha_strong()) } else { tint(t, Tone::Border, alpha_ghost()) };
                    let fg = if active { t.accent } else { t.dim };
                    let resp = ui.add(egui::Label::new(
                        egui::RichText::new(v.label()).monospace().size(font_sm()).color(fg)
                    ).sense(egui::Sense::click()));
                    let r = resp.rect.expand2(egui::vec2(4.0, 1.0));
                    ui.painter().rect_filled(r, radius_sm(), bg);
                    if resp.clicked() { let v8 = v as u8; watchlist.update_sidebar_state(|s| s.order_ledger_view = v8); }
                }
            });
            ui.add_space(gap_xs());

            let view = match watchlist.order_ledger_view {
                0 => LedgerView::Active, 1 => LedgerView::Journal, _ => LedgerView::All,
            };

            // ── Section 1: Active Orders (grouped by symbol) ────────────
            if matches!(view, LedgerView::Active | LedgerView::All) {
                ui.add(SectionLabel::new(&format!("ACTIVE ORDERS ({})", active_count)).tiny().color(t.accent));
                ui.add_space(gap_xs());

                let max_h = if matches!(view, LedgerView::All) {
                    (ui.available_height() * 0.45).max(120.0)
                } else {
                    ui.available_height() - gap_sm()
                };

                // Pre-build ordered symbol list and per-symbol counts outside of
                // closures so the borrow checker is happy with the `watchlist`
                // mutable borrow inside the scroll area.
                let mut symbols: Vec<String> = Vec::new();
                for row in &active_orders {
                    if !symbols.contains(&row.symbol) {
                        symbols.push(row.symbol.clone());
                    }
                }
                // Seed expanded state for any new symbols before we enter the
                // scroll area closure (avoids entry API inside nested borrows).
                for sym in &symbols {
                    watchlist.order_ledger_sym_expanded
                        .entry(sym.clone())
                        .or_insert(true);
                }

                // Trampoline: "Cancel all" clicks inside header_trailing cannot
                // mutate `watchlist` directly (already borrowed by the iterator).
                // Collect the symbol that was clicked and apply it after the loop.
                use std::cell::Cell;
                let cancel_all_clicked: Cell<Option<String>> = Cell::new(None);

                egui::ScrollArea::vertical()
                    .id_salt("ledger_active")
                    .max_height(max_h)
                    .show(ui, |ui| {
                        if active_orders.is_empty() {
                            ui.add_space(gap_sm());
                            ui.add(MonospaceCode::new("No active orders").size_px(font_sm()).color(t.dim).gamma(0.5));
                            ui.add_space(gap_sm());
                        } else {
                            // Render one PanelSubSection per symbol.
                            for sym in &symbols {
                                let sym_rows: Vec<&ActiveRow> = active_orders.iter()
                                    .filter(|r| &r.symbol == sym)
                                    .collect();
                                let working_count = sym_rows.len();

                                // Safety: we pre-seeded all entries above.
                                let expanded = watchlist.order_ledger_sym_expanded
                                    .get_mut(sym)
                                    .expect("pre-seeded above");

                                let sym_for_trail = sym.clone();
                                PanelSubSection::new(sym, sym)
                                    .count(working_count)
                                    .expanded(expanded)
                                    .header_trailing(|ui, t: &crate::chart_renderer::gpu::Theme| {
                                        // Show "Cancel all" only when ≥2 working orders —
                                        // the per-row × already covers the single-order case.
                                        if working_count >= 2 {
                                            if Button::new("Cancel all")
                                                .variant(Variant::Ghost)
                                                .fg(t.bear)
                                                .show(ui, t).clicked()
                                            {
                                                cancel_all_clicked.set(Some(sym_for_trail.clone()));
                                            }
                                        }
                                    })
                                    .show(ui, t, |ui, t| {
                                        // Column header (symbol omitted — it's in the sub-section title)
                                        ui.horizontal(|ui| {
                                            ui.add(MonospaceCode::new("TIME    SIDE TYPE  QTY/FILL  PRICE   STATE      CID")
                                                .size_px(font_xs()).color(t.dim).gamma(0.4));
                                        });
                                        ui.add_space(gap_2xs());
                                        for row in &sym_rows {
                                            let time_label  = format_hms(row.updated_at);
                                            let side_text   = match row.side {
                                                OrderSide::Buy | OrderSide::TriggerBuy => "BUY",
                                                OrderSide::Sell | OrderSide::TriggerSell => "SELL",
                                                OrderSide::Stop | OrderSide::OcoStop => "STOP",
                                                OrderSide::OcoTarget => "OCO",
                                            };
                                            let side_color  = match row.side {
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
                                            let qty_fill    = format!("{:>3}/{:<2}", row.qty, row.filled_qty);
                                            let price_str   = format!("{:>7.2}", row.price);
                                            let state_str   = format!(" {:<8}", state_text);
                                            let cid_str     = format!("{:<8}", row.cid8);
                                            let order_id    = row.id;

                                            let resp = PanelListRow::new(&format!("active_row_{}", row.id))
                                                .columns(&[
                                                    PanelColumn::left(&time_label).color(color_half(t.dim)),
                                                    PanelColumn::left(&format!("{:<5}", row.symbol)).color(t.text),
                                                    PanelColumn::left(&format!("{:<4}", side_text)).color(side_color),
                                                    PanelColumn::left("LMT ").color(t.dim),
                                                    PanelColumn::right(&qty_fill).color(t.text),
                                                    PanelColumn::right(&price_str).color(t.text),
                                                    PanelColumn::left(&state_str).color(state_color),
                                                    PanelColumn::left(" UI ").color(color_half(t.dim)),
                                                    PanelColumn::left(&cid_str).color(color_half(t.dim)),
                                                ])
                                                .show(ui, t);
                                            resp.context_menu(|ui| {
                                                if MenuItem::new("Cancel").icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                                                    crate::chart_renderer::trading::order_manager::cancel_order(order_id);
                                                    ui.close_menu();
                                                }
                                            });
                                        }
                                    });
                            }
                        }
                    });

                // Apply the trampoline: a "Cancel all" button was clicked inside
                // the scroll area — set the pending confirmation symbol.
                if let Some(sym) = cancel_all_clicked.into_inner() {
                    watchlist.order_ledger_pending_bulk_cancel = Some(sym);
                }

                // ── Inline confirmation for pending bulk cancel ───────────
                if let Some(pending_sym) = watchlist.order_ledger_pending_bulk_cancel.clone() {
                    let working = order_manager::working_count_for_symbol(&pending_sym);
                    draw_bulk_cancel_confirm(ui, t, watchlist, &pending_sym, working);
                }

                ui.add_space(gap_xs());
            }

            // ── Section 2: Journal feed ──────────────────────────────────
            if matches!(view, LedgerView::Journal | LedgerView::All) {
                separator(ui, tint(t, Tone::Border, alpha_muted()));
                ui.add_space(gap_xs());
                ui.add(SectionLabel::new(&format!("JOURNAL ({})", events.len())).tiny().color(t.accent));
                ui.add_space(gap_xs());

                // Filter chips
                ui.horizontal(|ui| {
                    let cur = LedgerFilter::all()[watchlist.order_ledger_filter as usize % 5];
                    for (i, f) in LedgerFilter::all().iter().enumerate() {
                        let active = *f == cur;
                        let bg = if active { tint(t, Tone::Accent, alpha_strong()) } else { tint(t, Tone::Border, alpha_ghost()) };
                        let fg = if active { t.accent } else { t.dim };
                        let resp = ui.add(egui::Label::new(
                            egui::RichText::new(f.label()).monospace().size(font_xs()).color(fg)
                        ).sense(egui::Sense::click()));
                        ui.painter().rect_filled(resp.rect.expand2(egui::vec2(3.0, 1.0)), radius_sm(), bg);
                        if resp.clicked() { let i8 = i as u8; watchlist.update_sidebar_state(|s| s.order_ledger_filter = i8); }
                    }
                });
                ui.add_space(gap_2xs());

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
                        for (idx, ev) in rows.iter().enumerate() {
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
                            let cid_str = format!("{:<8}", cid8);

                            PanelListRow::new(&format!("journal_row_{}", idx))
                                .columns(&[
                                    PanelColumn::left(&time_label).color(color_half(t.dim)),
                                    PanelColumn::left(kind_label).color(kind_color),
                                    PanelColumn::left(&cid_str).color(t.dim),
                                    PanelColumn::left(&summary).color(t.text),
                                ])
                                .hoverable(false)
                                .show(ui, t);
                        }
                    }
                });
            }
        });
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.order_ledger_open = false); }
}

// ── Inline bulk-cancel confirmation ─────────────────────────────────────────

/// Render the inline confirmation row for a pending bulk cancel.
/// Placed directly below the scroll area, stays visible even when the
/// sub-section is collapsed.
///
/// Layout (single horizontal strip):
///   ⚠ Cancel N working orders for SYM?  [Cancel all]  [Keep]
///
/// "Cancel all" confirms; "Keep" dismisses without action.
fn draw_bulk_cancel_confirm(
    ui: &mut egui::Ui,
    t: &Theme,
    watchlist: &mut Watchlist,
    sym: &str,
    working: usize,
) {
    let warn_color = tint(t, Tone::Bear, 220);
    let bg = tint(t, Tone::Bear, 18);
    let avail = ui.available_width();

    // Outer frame with a subtle danger tint.
    crate::ui_kit::widgets::OutlinedBox::new()
        .fill(bg)
        .borderless()
        .radius_sm()
        .padding_margin(egui::Margin::symmetric(gap_sm() as i8, gap_xs() as i8))
        .show(ui, t, |ui| {
            ui.set_min_width(avail);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap_sm();

                // Warning message
                let msg = format!("Cancel {} working order{} for {}?",
                    working,
                    if working == 1 { "" } else { "s" },
                    sym,
                );
                ui.add(MonospaceCode::new(&msg)
                    .size_px(font_sm())
                    .strong(true)
                    .color(warn_color));

                // Right-align the action buttons.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = gap_xs();

                    // Keep (dismiss)
                    if Button::new("Keep")
                        .variant(Variant::Ghost)
                        .fg(t.dim)
                        .show(ui, t).clicked()
                    {
                        watchlist.order_ledger_pending_bulk_cancel = None;
                    }

                    // Cancel all (confirm — the dangerous action)
                    if Button::new("Cancel all")
                        .variant(Variant::Ghost)
                        .fg(t.bear)
                        .show(ui, t).clicked()
                    {
                        let n = order_manager::cancel_all_for_symbol(sym);
                        if n > 0 {
                            report(
                                ErrorLevel::Info,
                                "order_manager",
                                "bulk_cancel",
                                format!("Cancelled {} order{} for {}", n,
                                    if n == 1 { "" } else { "s" }, sym),
                            );
                        }
                        watchlist.order_ledger_pending_bulk_cancel = None;
                    }
                });
            });
        });
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Collect every non-terminal order globally from the lock-free snapshot.
/// Render-thread read; does not lock the order manager.
pub(crate) fn collect_active_from_snapshot() -> Vec<ActiveRow> {
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
            price: o.price.to_f32(),
            cid8: o.client_order_id.chars().take(8).collect(),
            updated_at: o.updated_at.millis() as u64,
        })
        .collect();
    // Newest first so freshly placed orders appear at the top.
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

pub(crate) fn format_hms(ts_ms: u64) -> String {
    use chrono::TimeZone;
    let secs = (ts_ms / 1000) as i64;
    let nsecs = ((ts_ms % 1000) * 1_000_000) as u32;
    match chrono::Local.timestamp_opt(secs, nsecs).single() {
        Some(dt) => dt.format("%H:%M:%S").to_string(),
        None => "--:--:--".into(),
    }
}
