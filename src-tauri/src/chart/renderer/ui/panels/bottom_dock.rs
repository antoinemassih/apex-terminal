//! `bottom_dock` — the footer: a toggleable tabbed bottom dock for live trading
//! ops (Orders / Positions / Account / Notifications). Toggled via Ctrl+`.
//!
//! It docks to the bottom with `egui::TopBottomPanel::bottom` and uses
//! footer-tuned WIDE tables, but wears the same visual chrome as the side
//! panels (a `header_surface` tab band + hairline underline) so it reads as
//! one family. It floats as a rounded card under tiled styles, matching the
//! rail's region-gap treatment.
//!
//! It owns NO trading state of its own — every tab reads the same source the
//! rail panels read:
//!   * Orders        → `order_manager::orders_snapshot()` (+ `cancel_order`)
//!   * Positions     → `account_data` `Vec<Position>`
//!   * Account       → `account_data` `AccountSummary`
//!   * Notifications → `notification::history_snapshot()` (the toast log)

use egui;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use super::super::style::{self, *};
use super::super::super::gpu::{Watchlist, Theme};
use super::kit::PanelHeaderTabs;
use super::super::components::text::{MonospaceCode, SectionLabel};
use crate::ui_kit::widgets::{OutlinedBox, MenuItem, PanelListRow, PanelColumn};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::trading::{AccountSummary, Position, IbOrder, OrderSide, order_manager};
use crate::chart_renderer::trading::order_manager::OrderState;
use super::order_ledger_panel::{collect_active_from_snapshot, format_hms};
use super::split_tabs::{SplitTabs, SplitAxis};
use crate::chart_renderer::ui::tools::notification;

type AccountData = Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>;

// Self-managed dock height (session-only). egui's built-in `TopBottomPanel`
// resize didn't persist reliably here (snapped back), so we pin the height with
// `exact_height` and drive it from our own drag grip — fully deterministic.
const DOCK_MIN_H: f32 = 120.0;
const DOCK_MAX_H: f32 = 560.0;
const DOCK_GRIP_H: f32 = 6.0;
/// Height of the always-visible collapsed tab strip (header only).
const DOCK_STRIP_H: f32 = 30.0;
// Expanded height is persisted in `Watchlist::bottom_dock_height` (SidebarState),
// driven by the top-edge grip — no thread-local needed.

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab { Orders, Positions, Account, Notifications }

impl Tab {
    fn from_u8(v: u8) -> Tab {
        match v { 1 => Tab::Positions, 2 => Tab::Account, 3 => Tab::Notifications, _ => Tab::Orders }
    }
    fn as_u8(self) -> u8 {
        match self { Tab::Orders => 0, Tab::Positions => 1, Tab::Account => 2, Tab::Notifications => 3 }
    }
}

const TABS: [(Tab, &str); 4] = [
    (Tab::Orders,        "ORDERS"),
    (Tab::Positions,     "POSITIONS"),
    (Tab::Account,       "ACCOUNT"),
    (Tab::Notifications, "ALERTS"),
];

// One selected tab per split pane (session-only). Each entry is a column when
// expanded; `+` adds a column, `×` removes it. Seeded from the persisted
// `bottom_dock_tab` on first use.
thread_local! {
    static BOTTOM_SLOTS: std::cell::RefCell<Vec<Tab>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Total docked height (px) the footer occupies at the bottom of the screen:
/// the collapsed tab strip, or the expanded content height, plus the region gap.
/// The toast stack uses this to sit just above the footer instead of covering
/// its header — and to ride up automatically when the footer expands.
pub(crate) fn current_height(watchlist: &Watchlist) -> f32 {
    let rgap = region_gap();
    let content_h = if style::footer_visible() {
        watchlist.bottom_dock_height.clamp(DOCK_MIN_H, DOCK_MAX_H)
    } else {
        DOCK_STRIP_H
    };
    content_h + rgap
}

/// Render the bottom dock. Reads `account` (the same cached IB tuple the rail's
/// Orders/Positions panels use). No-op when closed.
pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, account: &AccountData, t: &Theme) {
    // The tab strip is ALWAYS pinned at the bottom (just the footer header).
    // `footer_visible()` (style default + user override) now means *expanded*:
    //   collapsed → strip only; click any tab → expand; close-X → collapse.
    let expanded = style::footer_visible();

    // Frame: panel surface; float as a rounded card when the active style tiles.
    let mut frame = egui::Frame::NONE.fill(t.panel_surface());
    let rgap = region_gap();
    if rgap > 0.0 {
        let rr = style::current().region_radius as u8;
        frame = frame
            .outer_margin(egui::Margin { left: rgap as i8, right: rgap as i8, top: 0, bottom: rgap as i8 })
            .corner_radius(egui::CornerRadius::same(rr))
            .stroke(egui::Stroke::new(1.0, tint(t, Tone::Border, 150)));
    }

    // Seed the split slots from the persisted primary tab on first use.
    BOTTOM_SLOTS.with(|s| {
        let mut sl = s.borrow_mut();
        if sl.is_empty() { sl.push(Tab::from_u8(watchlist.bottom_dock_tab)); }
    });

    let mut collapse = false;    // close-X on the last pane
    let mut want_expand = false; // a tab/strip was clicked while collapsed

    // Collapsed → just the strip; expanded → persisted height (grip-driven).
    let h = watchlist.bottom_dock_height.clamp(DOCK_MIN_H, DOCK_MAX_H);
    let content_h = if expanded { h } else { DOCK_STRIP_H };

    egui::TopBottomPanel::bottom(egui::Id::new("apex_bottom_dock"))
        .exact_height(content_h + rgap) // +rgap so the card keeps its height after the bottom outer margin.
        .frame(frame)
        .show(ctx, |ui| {
            if expanded {
                // ── Resize grip: thin drag strip at the top edge. ──
                let full = ui.max_rect();
                let grip = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), DOCK_GRIP_H));
                let gresp = ui.interact(grip, ui.id().with("dock_resize"), egui::Sense::drag());
                if gresp.hovered() || gresp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                if gresp.dragged() {
                    // Dragging up (negative dy) grows the dock (it extends upward).
                    let nh = (h - gresp.drag_delta().y).clamp(DOCK_MIN_H, DOCK_MAX_H);
                    watchlist.bottom_dock_height = nh;
                }
                if gresp.drag_stopped() {
                    let hh = watchlist.bottom_dock_height;
                    watchlist.update_sidebar_state(|s| s.bottom_dock_height = hh);
                }
                let gy = full.top() + DOCK_GRIP_H * 0.5;
                let cx = full.center().x;
                ui.painter().line_segment(
                    [egui::pos2(cx - 14.0, gy), egui::pos2(cx + 14.0, gy)],
                    egui::Stroke::new(2.0, tint(t, Tone::Dim, if gresp.hovered() { 210 } else { 90 })),
                );
                ui.add_space(DOCK_GRIP_H);

                // ── Split surface: N columns, each its own tab + (+)/(×). ──
                BOTTOM_SLOTS.with(|s| {
                    let mut slots = s.borrow_mut();
                    let resp = SplitTabs::new("bottom_dock", SplitAxis::Horizontal, &TABS, &mut slots)
                        .strip_height(DOCK_STRIP_H)
                        .max_panes(4)
                        .show(ui, t, |ui, _pane, tab| {
                            match tab {
                                Tab::Orders        => draw_orders(ui, t),
                                Tab::Positions     => draw_positions(ui, t, account),
                                Tab::Account       => draw_account(ui, t, account),
                                Tab::Notifications => draw_notifications(ui, t),
                            }
                        });
                    if resp.closed_last { collapse = true; }
                });
            } else {
                // ── Collapsed: single tab strip; click anywhere to expand. ──
                BOTTOM_SLOTS.with(|s| {
                    let mut slots = s.borrow_mut();
                    let header_resp = OutlinedBox::new()
                        .fill(t.header_surface()).borderless().square().padding(0.0)
                        .show(ui, t, |ui| {
                            PanelHeaderTabs::new(&mut slots[0], &TABS)
                                .id_salt("bottom_dock_strip")
                                .height(DOCK_STRIP_H)
                                .closable(false)
                                .show_with(ui, t, |_ui| {});
                        });
                    let strip_rect = header_resp.response.rect;
                    ui.painter().line_segment(
                        [egui::pos2(strip_rect.left(), strip_rect.bottom() - 0.5),
                         egui::pos2(strip_rect.right(), strip_rect.bottom() - 0.5)],
                        egui::Stroke::new(1.0, t.header_border()),
                    );
                    if ui.rect_contains_pointer(strip_rect) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        if ui.input(|i| i.pointer.primary_clicked()) { want_expand = true; }
                    }
                    // "expand" hint chevron on the right.
                    ui.painter().text(
                        egui::pos2(strip_rect.right() - 8.0, strip_rect.center().y),
                        egui::Align2::RIGHT_CENTER, "▴",
                        egui::FontId::proportional(11.0), tint(t, Tone::Dim, 150),
                    );
                });
            }
        });

    // Apply interactions. Close wins; otherwise a strip click expands.
    if collapse {
        style::set_footer_override(Some(false));
    } else if want_expand {
        style::set_footer_override(Some(true));
    }
    // Persist the primary (first) pane's tab.
    let primary = BOTTOM_SLOTS.with(|s| s.borrow().first().copied().unwrap_or(Tab::Orders));
    if primary.as_u8() != watchlist.bottom_dock_tab {
        watchlist.update_sidebar_state(|s| s.bottom_dock_tab = primary.as_u8());
    }
}

// ── Orders ──────────────────────────────────────────────────────────────────

fn draw_orders(ui: &mut egui::Ui, t: &Theme) {
    let orders = collect_active_from_snapshot();
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("TIME      SYMBOL  SIDE  TYPE  QTY/FILL    PRICE    STATE")
            .size_px(font_xs()).color(t.dim).gamma(0.5));
        ui.add_space(gap_md());
        ui.add(MonospaceCode::new("(right-click a row to cancel)")
            .size_px(font_xs()).color(t.dim).gamma(0.35));
    });
    ui.add_space(gap_2xs());

    if orders.is_empty() {
        ui.add_space(gap_sm());
        ui.add(MonospaceCode::new("No working orders").size_px(font_sm()).color(t.dim).gamma(0.5));
        return;
    }

    let cancel_id = std::cell::Cell::new(None::<u64>);
    egui::ScrollArea::vertical().id_salt("dock_orders").auto_shrink([false, false]).show(ui, |ui| {
        for row in &orders {
            let side_text = match row.side {
                OrderSide::Buy | OrderSide::TriggerBuy   => "BUY",
                OrderSide::Sell | OrderSide::TriggerSell => "SELL",
                OrderSide::Stop | OrderSide::OcoStop     => "STOP",
                OrderSide::OcoTarget                     => "OCO",
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
            let id = row.id;
            let resp = PanelListRow::new(&format!("dock_ord_{}", row.id))
                .columns(&[
                    PanelColumn::left(&format_hms(row.updated_at)).color(color_half(t.dim)),
                    PanelColumn::left(&format!("{:<6}", row.symbol)).color(t.text),
                    PanelColumn::left(&format!("{:<4}", side_text)).color(side_color),
                    PanelColumn::left("LMT").color(t.dim),
                    PanelColumn::right(&format!("{:>3}/{:<3}", row.qty, row.filled_qty)).color(t.text),
                    PanelColumn::right(&format!("{:>8.2}", row.price)).color(t.text),
                    PanelColumn::left(&format!("  {:<8}", state_text)).color(state_color),
                ])
                .show(ui, t);
            resp.context_menu(|ui| {
                if MenuItem::new("Cancel order").icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                    cancel_id.set(Some(id));
                    ui.close_menu();
                }
            });
        }
    });

    if let Some(id) = cancel_id.get() {
        order_manager::cancel_order(id);
    }
}

// ── Positions ─────────────────────────────────────────────────────────────

fn draw_positions(ui: &mut egui::Ui, t: &Theme, account: &AccountData) {
    let positions: &[Position] = account.as_ref().map(|(_, p, _)| p.as_slice()).unwrap_or(&[]);

    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("SYMBOL  QTY       AVG        LAST       MKT VAL      UNRL P&L    P&L%")
            .size_px(font_xs()).color(t.dim).gamma(0.5));
    });
    ui.add_space(gap_2xs());

    if positions.is_empty() {
        ui.add_space(gap_sm());
        ui.add(MonospaceCode::new("No open positions").size_px(font_sm()).color(t.dim).gamma(0.5));
        return;
    }

    egui::ScrollArea::vertical().id_salt("dock_positions").auto_shrink([false, false]).show(ui, |ui| {
        for p in positions {
            let long = p.qty >= 0;
            let qty_color = if long { t.bull } else { t.bear };
            let pnl = p.unrealized_pnl;
            let pnl_color = if pnl >= 0.0 { t.bull } else { t.bear };
            let pct = p.pnl_pct();
            let _ = PanelListRow::new(&format!("dock_pos_{}", p.symbol))
                .columns(&[
                    PanelColumn::left(&format!("{:<6}", p.symbol)).color(t.text),
                    PanelColumn::right(&format!("{:>+5}", p.qty)).color(qty_color),
                    PanelColumn::right(&format!("{:>8.2}", p.avg_price)).color(color_half(t.dim)),
                    PanelColumn::right(&format!("{:>8.2}", p.current_price)).color(t.text),
                    PanelColumn::right(&money(p.market_value)).color(t.text),
                    PanelColumn::right(&signed_money(pnl)).color(pnl_color),
                    PanelColumn::right(&format!("{:>+6.2}%", pct)).color(pnl_color),
                ])
                .show(ui, t);
        }
    });
}

// ── Account ─────────────────────────────────────────────────────────────────

fn draw_account(ui: &mut egui::Ui, t: &Theme, account: &AccountData) {
    let Some((acct, _, _)) = account.as_ref() else {
        ui.add_space(gap_sm());
        // U0-4: read as a problem (bear tint), not a neutral dim shrug. Kept as
        // an inline note (not a full PanelError block) — this is a compact
        // status row, and the OFFLINE StatusPill above already flags the state.
        ui.add(MonospaceCode::new("Broker not connected").size_px(font_sm()).color(t.bear).gamma(0.85));
        return;
    };

    // Connection chip. U0-3: route through the canonical StatusPill (was a
    // hand-rolled dot+text chip) so LIVE/OFFLINE shares one status vocabulary
    // with the rest of the app.
    let (label, tone) = if acct.connected {
        ("LIVE", crate::ui_kit::widgets::panel_section::Tone::Bull)
    } else {
        ("OFFLINE", crate::ui_kit::widgets::panel_section::Tone::Bear)
    };
    crate::ui_kit::widgets::StatusPill::new(label)
        .tone(tone)
        .dot(true)
        .size(crate::ui_kit::widgets::tokens::Size::Xs)
        .show(ui, t);
    ui.add_space(gap_sm());

    // Metric tiles, wrapped across the wide footer.
    ui.horizontal_wrapped(|ui| {
        tile(ui, t, "NET LIQ",      &money(acct.nav),                 t.text);
        tile(ui, t, "BUYING POWER", &money(acct.buying_power),        t.text);
        tile(ui, t, "EXCESS LIQ",   &money(acct.excess_liquidity),    t.text);
        tile(ui, t, "DAY P&L",      &signed_money(acct.daily_pnl),    pnl_col(t, acct.daily_pnl));
        tile(ui, t, "UNREALIZED",   &signed_money(acct.unrealized_pnl), pnl_col(t, acct.unrealized_pnl));
        tile(ui, t, "REALIZED",     &signed_money(acct.realized_pnl), pnl_col(t, acct.realized_pnl));
        tile(ui, t, "INIT MARGIN",  &money(acct.initial_margin),      color_half(t.dim));
        tile(ui, t, "MAINT MARGIN", &money(acct.maintenance_margin),  color_half(t.dim));
        tile(ui, t, "GROSS POS",    &money(acct.gross_position_value), t.text);
    });
}

fn tile(ui: &mut egui::Ui, t: &Theme, label: &str, value: &str, color: egui::Color32) {
    ui.vertical(|ui| {
        ui.add(MonospaceCode::new(label).size_px(font_xs()).color(t.dim).gamma(0.55));
        ui.add(MonospaceCode::new(value).size_px(font_md()).color(color).strong(true));
    });
    ui.add_space(gap_lg());
}

// ── Notifications ─────────────────────────────────────────────────────────

fn draw_notifications(ui: &mut egui::Ui, t: &Theme) {
    let hist = notification::history_snapshot(); // newest first
    ui.add(SectionLabel::new(&format!("RECENT ({})", hist.len())).tiny().color(t.accent));
    ui.add_space(gap_2xs());

    if hist.is_empty() {
        ui.add_space(gap_sm());
        ui.add(MonospaceCode::new("No notifications yet").size_px(font_sm()).color(t.dim).gamma(0.5));
        return;
    }

    // Virtualized: only build the rows actually visible in the viewport, not
    // all ~300. Keeps per-frame cost constant regardless of history depth
    // (fixes the bottom_dock render creep as the ring fills toward its cap).
    let row_h = font_sm() + ui.spacing().item_spacing.y;
    egui::ScrollArea::vertical().id_salt("dock_notifs").auto_shrink([false, false])
        .show_rows(ui, row_h, hist.len(), |ui, range| {
            for n in &hist[range] {
                let col = n.severity.color(t);
                ui.horizontal(|ui| {
                    ui.add(MonospaceCode::new("●").size_px(font_xs()).color(col));
                    ui.add(MonospaceCode::new(&format!("{:>6}", ago(n.created.elapsed())))
                        .size_px(font_xs()).color(color_half(t.dim)));
                    if let Some(src) = n.source {
                        ui.add(MonospaceCode::new(&format!("[{}]", src)).size_px(font_xs()).color(t.dim).gamma(0.6));
                    }
                    ui.add(MonospaceCode::new(&n.message).size_px(font_sm()).color(t.text));
                });
            }
        });
}

// ── Formatting helpers ──────────────────────────────────────────────────────

fn money(v: f64) -> String {
    let a = v.abs();
    if a >= 1_000_000.0 { format!("${:.2}M", v / 1e6) }
    else if a >= 1_000.0 { format!("${:.1}K", v / 1e3) }
    else { format!("${:.0}", v) }
}

fn signed_money(v: f64) -> String {
    if v >= 0.0 { format!("+{}", money(v)) } else { format!("-{}", money(v.abs())) }
}

fn pnl_col(t: &Theme, v: f64) -> egui::Color32 {
    if v >= 0.0 { t.bull } else { t.bear }
}

fn ago(d: std::time::Duration) -> String {
    let s = d.as_secs();
    if s < 60 { format!("{}s", s) }
    else if s < 3600 { format!("{}m", s / 60) }
    else { format!("{}h", s / 3600) }
}
