//! Trading-related types and standalone functions extracted from gpu.rs.
//! Orders, account data, positions, alerts, triggers, and market session helpers.

pub mod order_manager;

use std::sync::{Mutex, OnceLock};

/// ApexIB endpoint configuration (duplicated from gpu.rs where it's also used)
pub(crate) const APEXIB_URL: &str = "https://apexib-dev.xllio.com";

// ─── Orders ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum OrderSide { Buy, Sell, Stop, OcoTarget, OcoStop, TriggerBuy, TriggerSell }

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum OrderStatus { Draft, Placed, Executed, Cancelled }

#[derive(Debug, Clone)]
pub(crate) struct OrderLevel {
    pub id: u32,
    pub side: OrderSide,
    pub price: f32,
    pub qty: u32,
    pub status: OrderStatus,
    pub pair_id: Option<u32>, // linked order (OCO target↔stop, trigger buy↔sell)
    // Option trigger metadata (only for TriggerBuy/TriggerSell on underlying chart)
    pub option_symbol: Option<String>,  // e.g. "SPY 560C 0DTE"
    pub option_con_id: Option<i64>,
    // Trailing stop visualization
    pub trail_amount: Option<f32>,
    pub trail_percent: Option<f32>,
}

impl OrderLevel {
    pub fn color(&self, bull: egui::Color32, bear: egui::Color32) -> egui::Color32 {
        match self.side {
            OrderSide::Buy | OrderSide::TriggerBuy => bull,
            OrderSide::Sell | OrderSide::Stop | OrderSide::OcoStop | OrderSide::TriggerSell => bear,
            OrderSide::OcoTarget => egui::Color32::from_rgb(167, 139, 250), // purple
        }
    }
    pub fn label(&self) -> &'static str {
        match self.side {
            OrderSide::Buy => "BUY", OrderSide::Sell => "SELL", OrderSide::Stop => "STOP",
            OrderSide::OcoTarget => "OCO\u{2191}", OrderSide::OcoStop => "OCO\u{2193}",
            OrderSide::TriggerBuy => "TRIG\u{2191}", OrderSide::TriggerSell => "TRIG\u{2193}",
        }
    }
    pub fn notional(&self) -> f32 { self.price * self.qty as f32 }
}

/// Cancel an order and its paired leg (OCO/Trigger).
pub(crate) fn cancel_order_with_pair(orders: &mut Vec<OrderLevel>, id: u32) {
    let pair_id = orders.iter().find(|o| o.id == id).and_then(|o| o.pair_id);
    if let Some(o) = orders.iter_mut().find(|o| o.id == id) {
        o.status = OrderStatus::Cancelled;
    }
    if let Some(pid) = pair_id {
        if let Some(o) = orders.iter_mut().find(|o| o.id == pid) {
            if o.status == OrderStatus::Draft || o.status == OrderStatus::Placed {
                o.status = OrderStatus::Cancelled;
            }
        }
    }
}

pub(crate) fn fmt_notional(v: f32) -> String {
    if v >= 1_000_000.0 { format!("${:.1}M", v / 1_000_000.0) }
    else if v >= 1_000.0 { format!("${:.1}K", v / 1_000.0) }
    else { format!("${:.0}", v) }
}

// ─── Account & Positions (from ApexIB) ──────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub(crate) struct AccountSummary {
    pub nav: f64,
    pub buying_power: f64,
    pub excess_liquidity: f64,
    pub initial_margin: f64,
    pub maintenance_margin: f64,
    pub daily_pnl: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub gross_position_value: f64,
    pub connected: bool,
    pub last_update: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct IbOrder {
    pub symbol: String,
    pub side: String,
    pub qty: i32,
    pub filled_qty: i32,
    pub order_type: String,
    pub limit_price: f64,
    pub avg_fill_price: f64,
    pub status: String,
    pub strike: f64,
    pub option_type: String,
    pub submitted_at: i64, // unix ms
}

#[derive(Debug, Clone)]
pub(crate) struct Position {
    pub symbol: String,
    pub qty: i32,         // positive=long, negative=short
    pub avg_price: f32,
    pub current_price: f32,
    pub market_value: f64,
    pub unrealized_pnl: f64,
    pub con_id: i64,
}

impl Position {
    pub fn pnl(&self) -> f32 { self.unrealized_pnl as f32 }
    pub fn pnl_pct(&self) -> f32 {
        if self.avg_price == 0.0 { return 0.0; }
        ((self.current_price - self.avg_price) / self.avg_price) * 100.0
    }
}

// Shared account data — written by background worker, read by render thread
pub(crate) static ACCOUNT_DATA: OnceLock<Mutex<Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>>> = OnceLock::new();

/// Start the account polling worker (call once). Polls ApexIB every 5 seconds.
pub(crate) fn start_account_poller() {
    static STARTED: OnceLock<bool> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = ACCOUNT_DATA.get_or_init(|| Mutex::new(None));
        std::thread::spawn(|| {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(3))
                .build().unwrap_or_else(|_| reqwest::blocking::Client::new());
            loop {
                let mut summary = AccountSummary::default();
                let mut positions = Vec::new();

                // Fetch account summary
                if let Ok(resp) = client.get(format!("{}/account/summary", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        summary.connected = true;
                        summary.nav = json["netLiquidation"].as_f64().unwrap_or(0.0);
                        summary.buying_power = json["buyingPower"].as_f64().unwrap_or(0.0);
                        summary.excess_liquidity = json["excessLiquidity"].as_f64().unwrap_or(0.0);
                        summary.initial_margin = json["initMarginReq"].as_f64().unwrap_or(0.0);
                        summary.maintenance_margin = json["maintMarginReq"].as_f64().unwrap_or(0.0);
                        summary.gross_position_value = json["grossPositionValue"].as_f64().unwrap_or(0.0);
                        // Account summary also has unrealized/realized P&L
                        if summary.unrealized_pnl == 0.0 {
                            summary.unrealized_pnl = json["unrealizedPnL"].as_f64().unwrap_or(0.0);
                        }
                        if summary.realized_pnl == 0.0 {
                            summary.realized_pnl = json["realizedPnL"].as_f64().unwrap_or(0.0);
                        }
                    }
                }

                // Fetch P&L
                if let Ok(resp) = client.get(format!("{}/account/pnl", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        summary.daily_pnl = json["dailyPnL"].as_f64().unwrap_or(0.0);
                        summary.unrealized_pnl = json["unrealizedPnL"].as_f64().unwrap_or(0.0);
                        summary.realized_pnl = json["realizedPnL"].as_f64().unwrap_or(0.0);
                    }
                }

                // Fetch positions
                if let Ok(resp) = client.get(format!("{}/positions", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        if let Some(pos_arr) = json["positions"].as_array() {
                            for p in pos_arr {
                                positions.push(Position {
                                    symbol: p["symbol"].as_str().unwrap_or("").into(),
                                    qty: p["quantity"].as_i64().unwrap_or(0) as i32,
                                    avg_price: p["avgCost"].as_f64().unwrap_or(0.0) as f32,
                                    current_price: p["marketPrice"].as_f64().unwrap_or(0.0) as f32,
                                    market_value: p["marketValue"].as_f64().unwrap_or(0.0),
                                    unrealized_pnl: p["unrealizedPnl"].as_f64().unwrap_or(0.0),
                                    con_id: p["conId"].as_i64().unwrap_or(0),
                                });
                            }
                        }
                    }
                }

                // Fetch executions + pending + cancelled orders
                let mut ib_orders = Vec::new();
                // Only show orders from the last 24 hours
                let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
                let cutoff_ms = now_ms - 86_400_000; // 24 hours ago

                let parse_orders = |json: &serde_json::Value, key: &str, orders: &mut Vec<IbOrder>, cutoff: i64| {
                    if let Some(arr) = json[key].as_array() {
                        for o in arr {
                            let ts = o["submittedAt"].as_i64().or_else(|| o["time"].as_i64()).unwrap_or(0);
                            if ts > 0 && ts < cutoff { continue; } // skip old orders
                            orders.push(IbOrder {
                                symbol: o["symbol"].as_str().unwrap_or("").into(),
                                side: o["side"].as_str().or_else(|| o["action"].as_str()).unwrap_or("").into(),
                                qty: o["quantity"].as_i64().or_else(|| o["shares"].as_i64()).unwrap_or(0) as i32,
                                filled_qty: o["filledQty"].as_i64().or_else(|| o["shares"].as_i64()).unwrap_or(0) as i32,
                                order_type: o["orderType"].as_str().unwrap_or("").into(),
                                limit_price: o["limitPrice"].as_f64().or_else(|| o["price"].as_f64()).unwrap_or(0.0),
                                avg_fill_price: o["avgFillPrice"].as_f64().or_else(|| o["avgPrice"].as_f64()).or_else(|| o["price"].as_f64()).unwrap_or(0.0),
                                status: o["status"].as_str().unwrap_or(if key == "executions" { "filled" } else { "" }).into(),
                                strike: o["strike"].as_f64().unwrap_or(0.0),
                                option_type: o["optionType"].as_str().unwrap_or("").into(),
                                submitted_at: ts,
                            });
                        }
                    }
                };
                // Executions (filled trades)
                if let Ok(resp) = client.get(format!("{}/executions", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        parse_orders(&json, "executions", &mut ib_orders, cutoff_ms);
                    }
                }
                // Pending/submitted orders
                if let Ok(resp) = client.get(format!("{}/orders?status=submitted", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        parse_orders(&json, "orders", &mut ib_orders, cutoff_ms);
                    }
                }
                // Cancelled orders
                if let Ok(resp) = client.get(format!("{}/orders?status=cancelled", APEXIB_URL)).send() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        parse_orders(&json, "orders", &mut ib_orders, cutoff_ms);
                    }
                }

                summary.last_update = Some(std::time::Instant::now());

                if let Some(data) = ACCOUNT_DATA.get() {
                    if let Ok(mut d) = data.lock() { *d = Some((summary, positions, ib_orders)); }
                }

                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
        true
    });
}

/// Read latest account data (non-blocking)
pub(crate) fn read_account_data() -> Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)> {
    ACCOUNT_DATA.get()?.lock().ok()?.clone()
}

// ─── Alerts ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct Alert {
    pub id: u32,
    pub symbol: String,
    pub price: f32,
    pub above: bool, // true = alert when price goes above, false = below
    pub triggered: bool,
    pub message: String,
}

// ─── Trigger order (options on underlying price level) ──────────────────────

/// A placed trigger level — like an order level but for conditional options trades.
/// Lives on the underlying chart. Draggable, double-clickable.
#[derive(Debug, Clone)]
pub(crate) struct TriggerLevel {
    pub id: u32,
    pub side: OrderSide,         // BUY or SELL the option
    pub trigger_price: f32,      // underlying price that triggers the order
    pub above: bool,             // true = trigger when underlying >= price
    // Option contract
    pub symbol: String,           // underlying symbol
    pub option_type: String,      // "C" or "P"
    pub strike: f32,              // 0 = ATM
    pub expiry: String,           // "" = 0DTE
    pub qty: u32,
    pub submitted: bool,          // true = sent to IB
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TriggerPhase { Idle, Picking }

#[derive(Debug, Clone)]
pub(crate) struct TriggerSetup {
    pub phase: TriggerPhase,
    pub pending_side: OrderSide,  // which side we're placing
    pub option_type: String,
    pub strike: f32,
    pub expiry: String,
    pub qty: u32,
    // Pane management
    pub source_pane: usize,       // pane where the order panel is
    pub target_pane: Option<usize>, // pane with the underlying chart
}

impl Default for TriggerSetup {
    fn default() -> Self {
        Self {
            phase: TriggerPhase::Idle, pending_side: OrderSide::Buy,
            option_type: "C".into(), strike: 0.0, expiry: String::new(), qty: 1,
            source_pane: 0, target_pane: None,
        }
    }
}

// ─── Market session ───────────────────────────────────────────────────────────

pub(crate) fn market_session() -> (&'static str, egui::Color32) {
    use chrono::Timelike;
    let now = chrono::Utc::now();
    let h = now.hour(); let m = now.minute();
    let mins = h * 60 + m;
    if mins >= 13*60+30 && mins < 20*60 { ("OPEN", egui::Color32::from_rgb(46, 204, 113)) }
    else if mins >= 9*60 && mins < 13*60+30 { ("PRE", egui::Color32::from_rgb(255, 193, 37)) }
    else if mins >= 20*60 { ("POST", egui::Color32::from_rgb(100, 150, 255)) }
    else { ("CLOSED", egui::Color32::from_rgb(100, 100, 110)) }
}

pub(crate) fn contracts_for_notional(notional: f32, premium: f32, multiplier: f32) -> i32 {
    if premium <= 0.0 { return 0; }
    let cost_per_contract = premium * multiplier;
    (notional / cost_per_contract).floor() as i32
}

// ─── Bracket order templates ──────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct FloatingOrderPane {
    pub id: u32,
    pub title: String,      // e.g. "SPY 600C 0DTE"
    pub symbol: String,      // underlying
    pub strike: f32,
    pub is_call: bool,
    pub qty: u32,
    pub pos: egui::Pos2,     // window position (relative to pane)
}

#[derive(Clone)]
pub(crate) struct BracketTemplate {
    pub name: String,
    pub target_pct: f32,
    pub stop_pct: f32,
}

// ─── Price Alert (per-pane) ──────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct PriceAlert {
    pub id: u32,
    pub price: f32,
    pub above: bool,      // true = alert when price goes above, false = below
    pub triggered: bool,  // has been fired
    pub draft: bool,      // drafted via right-click; not armed until user places it (like order drafts)
    pub symbol: String,
}
