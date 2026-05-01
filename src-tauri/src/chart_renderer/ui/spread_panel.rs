//! Spread/Combo Builder panel — build and submit multi-leg option strategies.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Theme};
use super::widgets::text::{MonospaceCode, SectionLabel};
use super::widgets::buttons::{SimpleBtn, IconBtn, TradeBtn};
use super::widgets::inputs::TextInput;
use super::widgets::cards::metric_card::MetricCard;

// ─── Data Structures ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SpreadStrategy {
    VerticalCall,
    VerticalPut,
    IronCondor,
    Straddle,
    Strangle,
    CalendarSpread,
    IronButterfly,
    Custom,
}

impl SpreadStrategy {
    fn label(&self) -> &'static str {
        match self {
            Self::VerticalCall   => "Vertical Call Spread",
            Self::VerticalPut    => "Vertical Put Spread",
            Self::IronCondor     => "Iron Condor",
            Self::Straddle       => "Straddle",
            Self::Strangle       => "Strangle",
            Self::CalendarSpread => "Calendar Spread",
            Self::IronButterfly  => "Iron Butterfly",
            Self::Custom         => "Custom",
        }
    }

    fn all() -> &'static [SpreadStrategy] {
        &[
            Self::VerticalCall, Self::VerticalPut, Self::IronCondor,
            Self::Straddle, Self::Strangle, Self::CalendarSpread,
            Self::IronButterfly, Self::Custom,
        ]
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SpreadLeg {
    pub side: String,        // "BUY" or "SELL"
    pub qty: u32,
    pub option_type: String, // "CALL" or "PUT"
    pub strike: f32,
    pub expiry: String,      // "0DTE", "1W", "2W", "1M"
}

impl Default for SpreadLeg {
    fn default() -> Self {
        Self { side: "BUY".into(), qty: 1, option_type: "CALL".into(), strike: 450.0, expiry: "0DTE".into() }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SpreadState {
    pub symbol: String,
    pub strategy: SpreadStrategy,
    pub legs: Vec<SpreadLeg>,
    pub combo_qty: u32,
    pub strategy_dropdown_open: bool,
    pub editing_leg: Option<usize>,
    pub submit_result: Option<String>,
}

impl Default for SpreadState {
    fn default() -> Self {
        Self {
            symbol: "AAPL".into(),
            strategy: SpreadStrategy::VerticalCall,
            legs: vec![
                SpreadLeg { side: "BUY".into(), qty: 1, option_type: "CALL".into(), strike: 450.0, expiry: "0DTE".into() },
                SpreadLeg { side: "SELL".into(), qty: 1, option_type: "CALL".into(), strike: 460.0, expiry: "0DTE".into() },
            ],
            combo_qty: 1,
            strategy_dropdown_open: false,
            editing_leg: None,
            submit_result: None,
        }
    }
}

impl SpreadState {
    /// Apply a strategy preset with mock ATM-based strikes.
    fn apply_strategy(&mut self, strat: SpreadStrategy, atm: f32) {
        let interval = if atm > 500.0 { 10.0 } else if atm > 100.0 { 5.0 } else { 2.5 };
        let atm_round = (atm / interval).round() * interval;

        self.strategy = strat.clone();
        self.legs.clear();
        match strat {
            SpreadStrategy::VerticalCall => {
                self.legs.push(SpreadLeg { side: "BUY".into(), qty: 1, option_type: "CALL".into(), strike: atm_round, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "SELL".into(), qty: 1, option_type: "CALL".into(), strike: atm_round + interval, expiry: "0DTE".into() });
            }
            SpreadStrategy::VerticalPut => {
                self.legs.push(SpreadLeg { side: "BUY".into(), qty: 1, option_type: "PUT".into(), strike: atm_round, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "SELL".into(), qty: 1, option_type: "PUT".into(), strike: atm_round - interval, expiry: "0DTE".into() });
            }
            SpreadStrategy::IronCondor => {
                self.legs.push(SpreadLeg { side: "SELL".into(), qty: 1, option_type: "PUT".into(),  strike: atm_round - interval, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "BUY".into(),  qty: 1, option_type: "PUT".into(),  strike: atm_round - interval * 2.0, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "SELL".into(), qty: 1, option_type: "CALL".into(), strike: atm_round + interval, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "BUY".into(),  qty: 1, option_type: "CALL".into(), strike: atm_round + interval * 2.0, expiry: "0DTE".into() });
            }
            SpreadStrategy::Straddle => {
                self.legs.push(SpreadLeg { side: "BUY".into(), qty: 1, option_type: "CALL".into(), strike: atm_round, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "BUY".into(), qty: 1, option_type: "PUT".into(),  strike: atm_round, expiry: "0DTE".into() });
            }
            SpreadStrategy::Strangle => {
                self.legs.push(SpreadLeg { side: "BUY".into(), qty: 1, option_type: "CALL".into(), strike: atm_round + interval, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "BUY".into(), qty: 1, option_type: "PUT".into(),  strike: atm_round - interval, expiry: "0DTE".into() });
            }
            SpreadStrategy::CalendarSpread => {
                self.legs.push(SpreadLeg { side: "SELL".into(), qty: 1, option_type: "CALL".into(), strike: atm_round, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "BUY".into(),  qty: 1, option_type: "CALL".into(), strike: atm_round, expiry: "1M".into() });
            }
            SpreadStrategy::IronButterfly => {
                self.legs.push(SpreadLeg { side: "SELL".into(), qty: 1, option_type: "CALL".into(), strike: atm_round, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "SELL".into(), qty: 1, option_type: "PUT".into(),  strike: atm_round, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "BUY".into(),  qty: 1, option_type: "CALL".into(), strike: atm_round + interval, expiry: "0DTE".into() });
                self.legs.push(SpreadLeg { side: "BUY".into(),  qty: 1, option_type: "PUT".into(),  strike: atm_round - interval, expiry: "0DTE".into() });
            }
            SpreadStrategy::Custom => {
                self.legs.push(SpreadLeg::default());
            }
        }
    }
}

/// Compute P&L metrics for the spread legs. Uses live NBBO when available
/// (BUY = ask, SELL = bid), falls back to a strike-based estimate otherwise.
fn compute_spread_metrics(legs: &[SpreadLeg], underlying: &str) -> (f32, f32, f32, f32) {
    fn leg_price(leg: &SpreadLeg, underlying: &str) -> f32 {
        if !underlying.is_empty() {
            let occ = crate::chart_renderer::gpu::synthesize_occ(
                underlying, leg.strike, leg.option_type == "CALL", &leg.expiry,
            );
            if let Some(q) = crate::apex_data::live_state::get_quote(&occ) {
                let p = if leg.side == "BUY" { q.ask as f32 } else { q.bid as f32 };
                if p > 0.0 { return p; }
            }
        }
        // Fallback: rough approximation when feed unavailable.
        (leg.strike * 0.005).max(0.50)
    }
    // Underlying inferred from any leg (legs share the underlying in this panel).
    let mut net_premium = 0.0_f32;
    let mut buy_strikes = Vec::new();
    let mut sell_strikes = Vec::new();
    for leg in legs {
        let p = leg_price(leg, underlying) * leg.qty as f32;
        if leg.side == "BUY" {
            net_premium -= p;
            buy_strikes.push(leg.strike);
        } else {
            net_premium += p;
            sell_strikes.push(leg.strike);
        }
    }

    let net_debit_credit = net_premium;
    let (max_profit, max_loss);

    if legs.len() == 2 {
        let width = (legs[0].strike - legs[1].strike).abs();
        if net_premium < 0.0 {
            // Debit spread
            max_loss = net_premium.abs();
            max_profit = (width - max_loss).max(0.0);
        } else {
            // Credit spread
            max_profit = net_premium;
            max_loss = (width - max_profit).max(0.0);
        }
    } else if legs.len() == 4 {
        // Iron condor / butterfly approximation
        let strikes: Vec<f32> = legs.iter().map(|l| l.strike).collect();
        let wing_width = if strikes.len() >= 4 {
            let mut sorted = strikes.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            (sorted[1] - sorted[0]).min(sorted[3] - sorted[2])
        } else { 5.0 };
        max_profit = net_premium.abs().min(wing_width);
        max_loss = (wing_width - max_profit).max(0.0);
    } else {
        max_profit = net_premium.abs() * 3.0;
        max_loss = net_premium.abs();
    }

    // Break-even uses the long strike for debit spreads (BE = long_strike + debit_paid)
    // and the short strike for credit spreads (BE = short_strike - credit_received).
    // `net_debit_credit` is negative for debits, positive for credits.
    let break_even = if net_debit_credit < 0.0 && !buy_strikes.is_empty() {
        let avg_long = buy_strikes.iter().sum::<f32>() / buy_strikes.len() as f32;
        avg_long + net_debit_credit.abs()
    } else if net_debit_credit > 0.0 && !sell_strikes.is_empty() {
        let avg_short = sell_strikes.iter().sum::<f32>() / sell_strikes.len() as f32;
        avg_short - net_debit_credit
    } else if !buy_strikes.is_empty() {
        buy_strikes.iter().sum::<f32>() / buy_strikes.len() as f32
    } else if !sell_strikes.is_empty() {
        sell_strikes.iter().sum::<f32>() / sell_strikes.len() as f32
    } else { 0.0 };

    (net_debit_credit, max_profit, max_loss, break_even)
}

const EXPIRY_OPTIONS: &[&str] = &["0DTE", "1W", "2W", "1M", "2M", "3M"];

// ─── Draw ───────────────────────────────────────────────────────────────────

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    if !watchlist.spread_open { return; }

    let mut close = false;
    let mut pending_strategy: Option<SpreadStrategy> = None;
    let mut remove_leg: Option<usize> = None;
    let mut add_leg = false;
    let mut do_submit = false;

    egui::Window::new("spread_builder")
        .default_pos(egui::pos2(400.0, 100.0))
        .default_size(egui::vec2(340.0, 520.0))
        .resizable(true)
        .movable(true)
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
            .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_heavy())))
            .corner_radius(r_lg_cr()))
        .show(ctx, |ui| {
            let w = ui.available_width();

            // ── Header ──
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.add(MonospaceCode::new("SPREAD BUILDER").size_px(10.0).strong(true).color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    if close_button(ui, t.dim) { close = true; }
                });
            });
            ui.add_space(4.0);
            // Divider
            let div_rect = egui::Rect::from_min_size(
                egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
                egui::vec2(w, 1.0),
            );
            ui.painter().rect_filled(div_rect, 0.0, color_alpha(t.toolbar_border, alpha_dim()));
            ui.add_space(6.0);

            egui::ScrollArea::vertical().id_salt("spread_body").show(ui, |ui| {
                ui.set_min_width(w - 4.0);
                let m = 8.0;

                // ── Symbol ──
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.add(MonospaceCode::new("Symbol").size_px(9.0).color(t.dim));
                    ui.add_space(4.0);
                    let resp = TextInput::new(&mut watchlist.spread_state.symbol)
                        .width(80.0)
                        .margin(egui::Margin::symmetric(4, 2))
                        .theme(t)
                        .show(ui);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        watchlist.spread_state.symbol = watchlist.spread_state.symbol.to_uppercase();
                    }
                    // "Use active" chip
                    if !active_symbol.is_empty() && active_symbol != watchlist.spread_state.symbol {
                        ui.add_space(4.0);
                        if ui.add(egui::Button::new(
                            egui::RichText::new(active_symbol).monospace().size(8.0).color(t.accent))
                            .fill(color_alpha(t.accent, alpha_ghost()))
                            .corner_radius(r_md_cr())
                            .stroke(egui::Stroke::new(stroke_thin(), color_alpha(t.accent, alpha_muted())))
                            .min_size(egui::vec2(0.0, 16.0))
                        ).on_hover_text("Use chart symbol").clicked() {
                            watchlist.spread_state.symbol = active_symbol.to_string();
                        }
                    }
                });
                ui.add_space(6.0);

                // ── Strategy selector ──
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.add(MonospaceCode::new("Strategy").size_px(9.0).color(t.dim));
                    ui.add_space(4.0);
                    let strat_opts: Vec<(SpreadStrategy, &'static str)> = SpreadStrategy::all()
                        .iter().map(|s| (*s, s.label())).collect();
                    let mut cur_strat = watchlist.spread_state.strategy;
                    if super::widgets::select::Dropdown::new("spread_strategy_combo")
                        .options(&strat_opts)
                        .width(180.0)
                        .theme(t)
                        .show(ui, &mut cur_strat)
                    {
                        pending_strategy = Some(cur_strat);
                    }
                });
                ui.add_space(6.0);

                // ── Legs ──
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.add(SectionLabel::new("LEGS").xs().color(t.dim));
                });
                ui.add_space(4.0);

                // Leg rows
                let leg_count = watchlist.spread_state.legs.len();
                for idx in 0..leg_count {
                    let leg = &mut watchlist.spread_state.legs[idx];
                    // Leg card background
                    let card_rect = egui::Rect::from_min_size(
                        egui::pos2(ui.cursor().min.x + m - 2.0, ui.cursor().min.y),
                        egui::vec2(w - m * 2.0 + 4.0, 26.0),
                    );
                    let _ = ui.allocate_rect(card_rect, egui::Sense::hover());
                    ui.painter().rect_filled(card_rect, 3.0, color_alpha(t.toolbar_border, alpha_subtle()));

                    // Draw leg label + controls
                    let leg_label_col = if leg.side == "BUY" { t.bull } else { t.bear };
                    let leg_text = format!("L{}: {} {}x {} {} {}",
                        idx + 1, leg.side, leg.qty, leg.option_type, leg.strike, leg.expiry);
                    ui.painter().text(
                        egui::pos2(card_rect.min.x + 6.0, card_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &leg_text,
                        egui::FontId::monospace(9.5),
                        leg_label_col,
                    );

                    // Remove button (right side)
                    if leg_count > 1 {
                        let x_rect = egui::Rect::from_min_size(
                            egui::pos2(card_rect.right() - 18.0, card_rect.min.y + 2.0),
                            egui::vec2(18.0, 22.0),
                        );
                        // Render an IconBtn inside the pre-allocated rect using a child UI.
                        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(x_rect));
                        if child.add(IconBtn::new("\u{00D7}").small().color(t.dim)).clicked() {
                            remove_leg = Some(idx);
                        }
                    }

                    ui.add_space(2.0);

                    // Leg editor row (inline controls)
                    ui.horizontal(|ui| {
                        ui.add_space(m + 4.0);
                        // Side toggle
                        let side_col = if leg.side == "BUY" { t.bull } else { t.bear };
                        if ui.add(SimpleBtn::new(&leg.side).color(side_col).min_width(30.0).height(14.0)).clicked() {
                            leg.side = if leg.side == "BUY" { "SELL".into() } else { "BUY".into() };
                        }
                        // Qty
                        if ui.add(IconBtn::new("-").small().color(t.dim)).clicked() && leg.qty > 1 { leg.qty -= 1; }
                        ui.add(MonospaceCode::new(&format!("{}", leg.qty)).size_px(9.0).color(egui::Color32::from_gray(220)));
                        if ui.add(IconBtn::new("+").small().color(t.dim)).clicked() { leg.qty += 1; }
                        // Option type toggle
                        let ot_col = if leg.option_type == "CALL" { t.bull } else { t.bear };
                        if ui.add(SimpleBtn::new(&leg.option_type).color(ot_col).min_width(34.0).height(14.0)).clicked() {
                            leg.option_type = if leg.option_type == "CALL" { "PUT".into() } else { "CALL".into() };
                        }
                        // Strike
                        let mut strike_str = format!("{:.0}", leg.strike);
                        let strike_resp = TextInput::new(&mut strike_str)
                            .width(44.0).margin(egui::Margin::symmetric(2, 1))
                            .theme(t)
                            .show(ui);
                        if strike_resp.changed() {
                            if let Ok(v) = strike_str.parse::<f32>() { leg.strike = v; }
                        }
                        // Expiry combo
                        {
                            let expiry_id = format!("leg_expiry_{}", idx);
                            let opts: Vec<(&'static str, &'static str)> = EXPIRY_OPTIONS.iter().map(|&s| (s, s)).collect();
                            let mut cur: &'static str = EXPIRY_OPTIONS.iter().copied().find(|&s| s == leg.expiry.as_str()).unwrap_or("0DTE");
                            if super::widgets::select::Dropdown::new(&expiry_id)
                                .options(&opts)
                                .width(52.0)
                                .theme(t)
                                .show(ui, &mut cur)
                            {
                                leg.expiry = cur.to_string();
                            }
                        }
                    });
                    ui.add_space(4.0);
                }

                // Add leg button
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    if ui.add(SimpleBtn::new("+ Add Leg").color(t.accent).min_width(w - m * 2.0).height(18.0)).clicked() {
                        add_leg = true;
                    }
                });
                ui.add_space(8.0);

                // ── Spread Metrics ──
                let (net_dc, max_profit, max_loss, break_even) = compute_spread_metrics(&watchlist.spread_state.legs, active_symbol);
                let dc_label = if net_dc < 0.0 { "Net Debit" } else { "Net Credit" };

                // Two-column grid of MetricCards
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    MetricCard::new(dc_label, format!("${:.2}", net_dc.abs()))
                        .theme(t).show(ui);
                    ui.add_space(4.0);
                    MetricCard::new("Max Profit", format!("${:.2}", max_profit))
                        .theme(t).show(ui);
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    MetricCard::new("Max Loss", format!("${:.2}", max_loss))
                        .theme(t).show(ui);
                    ui.add_space(4.0);
                    MetricCard::new("Break Even", format!("${:.2}", break_even))
                        .theme(t).show(ui);
                });
                ui.add_space(6.0);

                // ── Qty multiplier ──
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.add(MonospaceCode::new("Qty").size_px(9.0).color(t.dim));
                    ui.add_space(6.0);
                    if ui.add(IconBtn::new("-").color(t.dim)).clicked() && watchlist.spread_state.combo_qty > 1 {
                        watchlist.spread_state.combo_qty -= 1;
                    }
                    ui.add(MonospaceCode::new(&format!("{}", watchlist.spread_state.combo_qty)).size_px(12.0).strong(true).color(egui::Color32::from_gray(240)));
                    if ui.add(IconBtn::new("+").color(t.dim)).clicked() {
                        watchlist.spread_state.combo_qty += 1;
                    }
                });
                ui.add_space(8.0);

                // ── Submit button ──
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    if ui.add(TradeBtn::new("SUBMIT SPREAD").color(t.accent).width(w - m * 2.0).height(30.0)).clicked() {
                        do_submit = true;
                    }
                });
                ui.add_space(6.0);

                // ── Submit result message ──
                if let Some(ref msg) = watchlist.spread_state.submit_result {
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        let col = if msg.starts_with("OK") { t.bull } else { t.bear };
                        ui.add(MonospaceCode::new(msg).size_px(9.0).color(col));
                    });
                    ui.add_space(4.0);
                }
            });
        });

    if close { watchlist.spread_open = false; }

    // Apply deferred actions outside the closure
    if let Some(strat) = pending_strategy {
        // Use a mock ATM price based on the symbol for preset generation
        let atm = match watchlist.spread_state.symbol.as_str() {
            "SPY" => 580.0, "QQQ" => 500.0, "IWM" => 220.0,
            "AAPL" => 230.0, "MSFT" => 450.0, "NVDA" => 900.0,
            "TSLA" => 250.0, "AMZN" => 200.0, "META" => 530.0,
            "GOOGL" => 170.0, _ => 100.0,
        };
        watchlist.spread_state.apply_strategy(strat, atm);
    }
    if let Some(idx) = remove_leg {
        if idx < watchlist.spread_state.legs.len() {
            watchlist.spread_state.legs.remove(idx);
        }
    }
    if add_leg {
        let last_strike = watchlist.spread_state.legs.last().map(|l| l.strike).unwrap_or(450.0);
        let last_expiry = watchlist.spread_state.legs.last().map(|l| l.expiry.clone()).unwrap_or_else(|| "0DTE".into());
        watchlist.spread_state.legs.push(SpreadLeg {
            side: "BUY".into(), qty: 1, option_type: "CALL".into(),
            strike: last_strike + 5.0, expiry: last_expiry,
        });
    }
    if do_submit {
        submit_spread(watchlist);
    }
}

/// Submit the current spread as a combo order through the OrderManager.
fn submit_spread(watchlist: &mut Watchlist) {
    use super::super::trading::order_manager::{ComboLeg, submit_combo_order, OrderResult};

    let state = &watchlist.spread_state;
    if state.legs.is_empty() {
        watchlist.spread_state.submit_result = Some("No legs defined".into());
        return;
    }

    // Build ComboLeg entries. In MVP, con_id is 0 (mock) since we don't have live chain yet.
    // The backend will resolve con_ids from symbol + strike + expiry.
    let combo_legs: Vec<ComboLeg> = state.legs.iter().map(|leg| {
        ComboLeg {
            con_id: 0, // Mock — backend resolves from option details
            ratio: leg.qty as i32,
            side: leg.side.clone(),
        }
    }).collect();

    // Determine overall side: if first leg is BUY it's a buy combo
    let overall_side = if state.legs[0].side == "BUY" { "buy" } else { "sell" };

    // Submit through OrderManager
    let result = submit_combo_order(
        &state.symbol,
        combo_legs,
        overall_side,
        state.combo_qty,
        "limit",
        Some(0.0), // limit price placeholder — real price comes from chain
    );

    watchlist.spread_state.submit_result = Some(match result {
        OrderResult::Accepted(id) => format!("OK: Spread submitted (ID {})", id),
        OrderResult::NeedsConfirmation(id) => format!("OK: Needs confirmation (ID {})", id),
        OrderResult::Rejected(reason) => format!("Rejected: {}", reason),
        OrderResult::Duplicate => "Duplicate order blocked".into(),
    });
}
