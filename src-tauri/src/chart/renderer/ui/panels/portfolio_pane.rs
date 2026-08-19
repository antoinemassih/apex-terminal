//! Portfolio pane — real account, positions, exposure and risk from the
//! dedicated ApexIB trading API (`/account/summary`, `/positions`, `/risk/*`).
//!
//! Layout (responsive, egui-laid-out — no absolute magic offsets):
//!   ┌ header (Portfolio · account/connection badge) ─────────────────────────┐
//!   │ metric cards: Net Liq · Day P&L · Unrealized · Realized · Buying Power …│
//!   ├───────────────────────────────┬────────────────────────────────────────┤
//!   │ POSITIONS table (real)         │ EXPOSURE (computed)                     │
//!   │ sym qty avg last mv pnl % wt   │ MARGIN (real)                           │
//!   │ … + totals row                 │ RISK (real limits + circuit breaker)    │
//!   └───────────────────────────────┴────────────────────────────────────────┘
//! Everything is real or computed from real positions; the right column hides
//! when the pane is too narrow.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use crate::chart_renderer::trading::{AccountSummary, Position, read_risk_data};
use super::super::components::headers_widget::PaneHeader;
use crate::ui_kit::widgets::{MetricRow, MetricTone, PanelKeyValueRow, PanelSection};
use crate::ui_kit::widgets::panel_section::Tone as PTone;

pub(crate) fn render(
    ui: &mut egui::Ui, _ctx: &egui::Context,
    _panes: &mut [Chart], pane_idx: usize, active_pane: &mut usize,
    _visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    _watchlist: &mut Watchlist,
    account_data: &Option<(AccountSummary, Vec<Position>, Vec<crate::chart_renderer::trading::IbOrder>)>,
) {
    let t_owned = crate::chart_renderer::gpu::get_theme(theme_idx); let t = &t_owned;
    if pane_rects.is_empty() { return; }
    let rect = pane_rects[0];
    ui.painter_at(rect).rect_filled(rect, 0.0, t.bg);
    if let Some(p) = ui.ctx().pointer_hover_pos() { if rect.contains(p) { *active_pane = pane_idx; } }

    let margin = 16.0;
    let inner = rect.shrink(margin);
    let mut col = ui.new_child(egui::UiBuilder::new()
        .max_rect(inner)
        .layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut col;
    ui.spacing_mut().item_spacing = egui::vec2(gap_md(), gap_sm());

    // ── Real data (no placeholders) ──────────────────────────────────────────
    let connected = account_data.as_ref().map_or(false, |(s, _, _)| s.connected);
    let summary = account_data.as_ref().map(|(s, _, _)| s.clone()).unwrap_or_default();
    let positions: Vec<Position> = account_data.as_ref().map(|(_, p, _)| p.clone()).unwrap_or_default();
    let risk = read_risk_data();

    // ── Header + connection badge ────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add(PaneHeader::new("Portfolio").theme(t));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (label, tone) = if connected {
                ("BROKER LIVE", crate::ui_kit::widgets::panel_section::Tone::Bull)
            } else {
                ("NOT CONNECTED", crate::ui_kit::widgets::panel_section::Tone::Default)
            };
            crate::ui_kit::widgets::StatusPill::new(label).tone(tone).dot(true)
                .size(crate::ui_kit::widgets::tokens::Size::Xs).show(ui, t);
        });
    });
    separator(ui, tint(t, Tone::Border, alpha_muted()));

    if !connected && positions.is_empty() {
        ui.add_space(gap_xl() * 2.0);
        ui.vertical_centered(|ui| {
            ui.add(crate::chart_renderer::ui::components::text::MonospaceCode::new(
                "No broker connection").size_px(font_md()).color(color_muted(t.dim)));
            ui.add_space(gap_xs());
            ui.add(crate::chart_renderer::ui::components::text::MonospaceCode::new(
                "Connect a broker (Connections panel) to see positions & P&L").size_px(font_xs()).color(color_dim(t.dim)));
        });
        return;
    }

    // ── Derived totals ───────────────────────────────────────────────────────
    let gross: f64 = positions.iter().map(|p| p.market_value.abs()).sum();
    let net: f64 = positions.iter().map(|p| p.market_value).sum();
    let long_val: f64 = positions.iter().filter(|p| p.qty > 0).map(|p| p.market_value).sum();
    let short_val: f64 = positions.iter().filter(|p| p.qty < 0).map(|p| p.market_value.abs()).sum();
    let n_long = positions.iter().filter(|p| p.qty > 0).count();
    let n_short = positions.iter().filter(|p| p.qty < 0).count();
    let total_unreal: f64 = positions.iter().map(|p| p.unrealized_pnl).sum();
    let net_liq = if summary.nav > 0.0 { summary.nav } else { gross };

    // ── Metric cards (real account summary) ──────────────────────────────────
    let pnl_tone = |v: f64| if v > 0.0 { MetricTone::Bull } else if v < 0.0 { MetricTone::Bear } else { MetricTone::Default };
    let money = |v: f64| if v.abs() >= 1_000_000.0 { format!("${:+.2}M", v / 1e6) }
        else if v.abs() >= 10_000.0 { format!("${:+.1}K", v / 1e3) } else { format!("${:+.0}", v) };
    let money0 = |v: f64| if v >= 1_000_000.0 { format!("${:.2}M", v / 1e6) }
        else if v >= 10_000.0 { format!("${:.1}K", v / 1e3) } else { format!("${:.0}", v) };

    let cards: [(&str, String, MetricTone); 8] = [
        ("NET LIQ",       money0(net_liq),               MetricTone::Default),
        ("DAY P&L",       money(summary.daily_pnl),      pnl_tone(summary.daily_pnl)),
        ("UNREALIZED",    money(if summary.unrealized_pnl != 0.0 { summary.unrealized_pnl } else { total_unreal }), pnl_tone(if summary.unrealized_pnl != 0.0 { summary.unrealized_pnl } else { total_unreal })),
        ("REALIZED",      money(summary.realized_pnl),   pnl_tone(summary.realized_pnl)),
        ("BUYING POWER",  money0(summary.buying_power),   MetricTone::Accent),
        ("EXCESS LIQ",    money0(summary.excess_liquidity), MetricTone::Default),
        ("GROSS EXP",     money0(if summary.gross_position_value > 0.0 { summary.gross_position_value } else { gross }), MetricTone::Default),
        ("MAINT MARGIN",  money0(summary.maintenance_margin), MetricTone::Warn),
    ];
    let card_w = 150.0_f32;
    ui.horizontal_wrapped(|ui| {
        for (label, value, tone) in cards {
            ui.allocate_ui(egui::vec2(card_w, 46.0), |ui| {
                MetricRow::new(label).value(value).tone(tone)
                    .value_font(font_lg() + 2.0).label_font(font_2xs()).proportional()
                    .show(ui, t);
            });
        }
    });
    ui.add_space(gap_sm());
    separator(ui, tint(t, Tone::Border, alpha_muted()));
    ui.add_space(gap_sm());

    // ── Body: positions table (left) + analytics column (right, if room) ─────
    let avail = ui.available_size();
    let show_side = avail.x > 720.0;
    let side_w = if show_side { 250.0 } else { 0.0 };
    let table_w = (avail.x - side_w - if show_side { gap_lg() } else { 0.0 }).max(120.0);
    let body_h = avail.y.max(0.0);

    ui.horizontal_top(|ui| {
        // ── Positions table ──────────────────────────────────────────────────
        ui.allocate_ui_with_layout(egui::vec2(table_w, body_h), egui::Layout::top_down(egui::Align::Min), |ui| {
            positions_table(ui, t, &positions, net_liq, table_w);
        });

        if show_side {
            ui.add_space(gap_lg());
            ui.allocate_ui_with_layout(egui::vec2(side_w, body_h), egui::Layout::top_down(egui::Align::Min), |ui| {
                // EXPOSURE (computed from real positions)
                PanelSection::new("EXPOSURE").rule(true).show(ui, t, |ui, t| {
                    PanelKeyValueRow::new("Long",  &money0(long_val)).tone(PTone::Bull).show(ui, t);
                    PanelKeyValueRow::new("Short", &money0(short_val)).tone(PTone::Bear).show(ui, t);
                    PanelKeyValueRow::new("Gross", &money0(gross)).tone(PTone::Default).show(ui, t);
                    PanelKeyValueRow::new("Net",   &money(net)).tone(if net >= 0.0 { PTone::Bull } else { PTone::Bear }).show(ui, t);
                    PanelKeyValueRow::new("Positions", &format!("{} long · {} short", n_long, n_short)).tone(PTone::Default).show(ui, t);
                });
                ui.add_space(gap_md());

                // MARGIN (real)
                PanelSection::new("MARGIN").rule(true).show(ui, t, |ui, t| {
                    PanelKeyValueRow::new("Maint",  &money0(summary.maintenance_margin)).tone(PTone::Default).show(ui, t);
                    PanelKeyValueRow::new("Initial", &money0(summary.initial_margin)).tone(PTone::Default).show(ui, t);
                    PanelKeyValueRow::new("Excess Liq", &money0(summary.excess_liquidity)).tone(PTone::Default).show(ui, t);
                });
                // `net_liq == 0.0` means the account snapshot has not arrived,
                // not that the account is empty. This used to fall back to
                // `0.0`, which rendered "0%" in BULL green with an empty bar —
                // the most reassuring possible display of "we have no idea",
                // on the one figure that tells a trader how close they are to
                // a margin call.
                let margin_util = if net_liq > 0.0 {
                    Some((summary.maintenance_margin / net_liq) as f32)
                } else {
                    None
                };
                let mtone = match margin_util {
                    Some(u) if u > 0.7 => MetricTone::Bear,
                    Some(u) if u > 0.5 => MetricTone::Warn,
                    Some(_) => MetricTone::Bull,
                    None => MetricTone::Muted,
                };
                ui.add_space(gap_xs());
                let mut margin_row = MetricRow::new("Margin used")
                    .value(match margin_util {
                        Some(u) => format!("{:.0}%", u * 100.0),
                        None => "\u{2014}".to_string(),
                    })
                    .tone(mtone);
                // No bar at all when unknown — an empty bar reads as "none used".
                if let Some(u) = margin_util {
                    margin_row = margin_row.bar(u);
                }
                margin_row.show(ui, t);
                ui.add_space(gap_md());

                // RISK (real limits + circuit breaker)
                PanelSection::new("RISK").rule(true).show(ui, t, |ui, t| {
                    if let Some(r) = &risk {
                        let (st, stone) = if r.manually_halted { ("HALTED", PTone::Bear) }
                            else if r.circuit_tripped { ("TRIPPED", PTone::Bear) }
                            else if !r.connected { ("DISCONNECTED", PTone::Warn) }
                            else { ("OK", PTone::Bull) };
                        PanelKeyValueRow::new("Circuit breaker", st).tone(stone).show(ui, t);
                        PanelKeyValueRow::new("Rejections", &format!("{}/{}", r.recent_rejections, r.max_rejections))
                            .tone(if r.recent_rejections > 0 { PTone::Warn } else { PTone::Default }).show(ui, t);
                        if r.max_gross_exposure > 0.0 {
                            let used = (gross / r.max_gross_exposure) as f32;
                            PanelKeyValueRow::new("Gross limit", &money0(r.max_gross_exposure)).tone(PTone::Default).show(ui, t);
                            ui.add_space(gap_xs());
                            MetricRow::new("Gross used").value(format!("{:.1}%", used * 100.0))
                                .tone(if used > 0.8 { MetricTone::Bear } else { MetricTone::Default })
                                .bar(used.min(1.0)).show(ui, t);
                        }
                    } else {
                        // U0-4: risk feed down reads as a problem (bear tint), not a
                        // neutral dim note; kept inline (compact section body).
                        ui.add(crate::chart_renderer::ui::components::text::MonospaceCode::new(
                            "risk feed not connected").size_px(font_2xs()).color(color_dim(t.bear)));
                    }
                });
            });
        }
    });
}

/// Real positions table — egui-laid-out rows (sym / qty / avg / last / mkt val /
/// unreal P&L / P&L% / weight%) with a totals footer. Scrolls when tall.
fn positions_table(ui: &mut egui::Ui, t: &Theme, positions: &[Position], net_liq: f64, width: f32) {
    use crate::chart_renderer::ui::components::text::{SectionLabel, MonospaceCode};
    // Proportional column weights → pixel widths from available `width`.
    let weights = [0.20, 0.10, 0.13, 0.13, 0.16, 0.14, 0.08, 0.06]; // sum 1.0
    let headers = ["SYMBOL", "QTY", "AVG", "LAST", "MKT VAL", "UNREAL", "P&L%", "WT%"];
    let aligns = [egui::Align::Min, egui::Align::Max, egui::Align::Max, egui::Align::Max, egui::Align::Max, egui::Align::Max, egui::Align::Max, egui::Align::Max];
    let colw: Vec<f32> = weights.iter().map(|w| (w * width).max(28.0)).collect();

    let cell = |ui: &mut egui::Ui, w: f32, align: egui::Align, text: String, color: egui::Color32, strong: bool| {
        ui.allocate_ui_with_layout(egui::vec2(w, 18.0),
            egui::Layout::right_to_left(egui::Align::Center).with_main_align(if align == egui::Align::Min { egui::Align::Min } else { egui::Align::Max }),
            |ui| {
                let mut mc = MonospaceCode::new(&text).size_px(font_xs()).color(color);
                if strong { mc = mc.strong(true); }
                ui.add(mc);
            });
    };

    // Header row
    ui.horizontal(|ui| {
        for (i, h) in headers.iter().enumerate() {
            ui.allocate_ui_with_layout(egui::vec2(colw[i], 14.0),
                egui::Layout::right_to_left(egui::Align::Center).with_main_align(if aligns[i] == egui::Align::Min { egui::Align::Min } else { egui::Align::Max }),
                |ui| { ui.add(SectionLabel::new(h).xs().color(color_dim(t.dim))); });
        }
    });
    separator(ui, tint(t, Tone::Border, alpha_faint()));

    if positions.is_empty() {
        ui.add_space(gap_md());
        ui.add(MonospaceCode::new("No open positions").size_px(font_xs()).color(color_muted(t.dim)));
        return;
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 22.0).show(ui, |ui| {
        for (ri, p) in positions.iter().enumerate() {
            let pnl_c = if p.unrealized_pnl >= 0.0 { t.bull } else { t.bear };
            let dir_c = if p.qty >= 0 { t.bull } else { t.bear };
            let pnl_pct = p.pnl_pct();
            // Same story as margin: an unknown net liquidation value makes
            // every position's portfolio weight unknowable, not 0%.
            let wt = if net_liq > 0.0 { Some(p.market_value.abs() / net_liq * 100.0) } else { None };
            let row = ui.horizontal(|ui| {
                cell(ui, colw[0], egui::Align::Min, p.symbol.clone(), t.text, true);
                cell(ui, colw[1], egui::Align::Max, format!("{}{}", if p.qty > 0 { "+" } else { "" }, p.qty), dir_c, false);
                cell(ui, colw[2], egui::Align::Max, format!("{:.2}", p.avg_price), color_half(t.dim), false);
                cell(ui, colw[3], egui::Align::Max, format!("{:.2}", p.current_price), t.text, false);
                cell(ui, colw[4], egui::Align::Max, fmt_money(p.market_value), t.dim, false);
                cell(ui, colw[5], egui::Align::Max, format!("{:+.0}", p.unrealized_pnl), pnl_c, true);
                cell(ui, colw[6], egui::Align::Max,
                    pnl_pct.map_or_else(|| "\u{2014}".to_string(), |v| format!("{v:+.1}")),
                    pnl_c, false);
                cell(ui, colw[7], egui::Align::Max,
                    wt.map_or_else(|| "\u{2014}".to_string(), |v| format!("{v:.0}")),
                    color_half(t.dim), false);
            });
            if ri % 2 == 1 {
                ui.painter().rect_filled(row.response.rect.expand2(egui::vec2(2.0, 0.0)), 0.0, tint(t, Tone::Border, alpha_faint()));
            }
        }
    });

    // Totals footer
    separator(ui, tint(t, Tone::Border, alpha_muted()));
    let tot_mv: f64 = positions.iter().map(|p| p.market_value).sum();
    let tot_pnl: f64 = positions.iter().map(|p| p.unrealized_pnl).sum();
    ui.horizontal(|ui| {
        cell(ui, colw[0], egui::Align::Min, "TOTAL".into(), color_dim(t.dim), true);
        cell(ui, colw[1] + colw[2] + colw[3], egui::Align::Max, String::new(), t.dim, false);
        cell(ui, colw[4], egui::Align::Max, fmt_money(tot_mv), t.text, true);
        cell(ui, colw[5], egui::Align::Max, format!("{:+.0}", tot_pnl), if tot_pnl >= 0.0 { t.bull } else { t.bear }, true);
        cell(ui, colw[6] + colw[7], egui::Align::Max, String::new(), t.dim, false);
    });
}

/// Delegates to the ONE compact-number renderer.
///
/// The visible change is the K threshold: this alone used **10_000**, so
/// `4_500` rendered `4500` here and `4.5K` in every other panel. There was no
/// stated reason for the difference and it read as an inconsistency, not a
/// choice.
fn fmt_money(v: f64) -> String {
    crate::foundation::num_format::plain(v)
}
