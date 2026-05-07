//! Centralized Order Manager — enterprise-grade order lifecycle management.
//!
//! ALL order creation, modification, cancellation, and state transitions flow through
//! this module. No UI component directly mutates order state.
//!
//! Features:
//! - Deduplication via order signatures (prevents double-fire)
//! - State machine with audit trail
//! - Pre-submission risk validation
//! - Armed/confirmation flow
//! - Backend sync (ApexIB)
//! - Thread-safe global singleton

use std::sync::{Mutex, OnceLock};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use super::{OrderSide, OrderStatus, OrderLevel, APEXIB_URL};

// ─── Lock helper (panic-poison recovery) ────────────────────────────────────

fn manager() -> &'static Mutex<OrderManager> {
    ORDER_MANAGER.get_or_init(|| {
        let mut m = OrderManager::new();
        m.load_from_disk();
        Mutex::new(m)
    })
}

/// Run a closure with mutable access to the global OrderManager.
/// Recovers from poisoned mutexes by extracting the inner data — essential
/// for a trading app where one panic must not lock everyone out forever.
fn with_mgr<F, R>(f: F) -> R where F: FnOnce(&mut OrderManager) -> R {
    let mut g = manager().lock().unwrap_or_else(|e| {
        eprintln!("[order_manager] mutex poisoned, recovering: {e}");
        e.into_inner()
    });
    f(&mut *g)
}

/// Path for persisted open orders.
fn orders_state_path() -> PathBuf {
    let dir = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let state = dir.join("state");
    let _ = std::fs::create_dir_all(&state);
    state.join("orders.json")
}

// ─── Order State Machine ────────────────────────────────────────────────────

/// Extended order status with full lifecycle
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum OrderState {
    Draft,           // Created locally, not yet submitted (awaiting confirmation)
    PendingSubmit,   // Queued for submission to backend
    Working,         // Confirmed by backend, live in market
    PartialFill,     // Partially filled
    Filled,          // Fully filled
    PendingCancel,   // Cancel request sent to backend
    Cancelled,       // Confirmed cancelled
    Rejected,        // Rejected by backend or risk checks
    PendingModify,   // Modify request sent to backend
}

impl OrderState {
    pub(crate) fn is_active(&self) -> bool {
        matches!(self, Self::Draft | Self::PendingSubmit | Self::Working | Self::PartialFill | Self::PendingModify)
    }
    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Filled | Self::Cancelled | Self::Rejected)
    }
    /// Map to legacy OrderStatus for backward compat with rendering code
    pub(crate) fn to_legacy(&self) -> OrderStatus {
        match self {
            Self::Draft => OrderStatus::Draft,
            Self::PendingSubmit | Self::Working | Self::PartialFill | Self::PendingModify => OrderStatus::Placed,
            Self::Filled => OrderStatus::Executed,
            Self::PendingCancel | Self::Cancelled | Self::Rejected => OrderStatus::Cancelled,
        }
    }
}

// ─── Order Signature (deduplication) ────────────────────────────────────────

/// Unique signature for dedup. Two orders with same sig within the cooldown window
/// are considered duplicates and the second is rejected.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct OrderSignature {
    symbol: String,
    side: u8,       // 0=buy, 1=sell, etc.
    price_cents: i64,
    qty: u32,
}

impl OrderSignature {
    fn new(symbol: &str, side: OrderSide, price: f32, qty: u32) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            side: side as u8,
            price_cents: (price * 100.0).round() as i64,
            qty,
        }
    }
}

// ─── Managed Order ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ManagedOrder {
    pub(crate) id: u64,
    pub(crate) symbol: String,
    pub(crate) side: OrderSide,
    pub(crate) order_type: ManagedOrderType,
    pub(crate) price: f32,
    pub(crate) stop_price: f32,     // stop trigger price
    pub(crate) qty: u32,
    pub(crate) filled_qty: u32,
    pub(crate) avg_fill_price: f32,
    pub(crate) state: OrderState,
    pub(crate) pair_id: Option<u64>,
    // Trailing stop fields
    pub(crate) trail_amount: Option<f32>,
    pub(crate) trail_percent: Option<f32>,
    // Metadata
    pub(crate) option_symbol: Option<String>,
    pub(crate) option_con_id: Option<i64>,
    pub(crate) source: OrderSource,
    pub(crate) created_at: u64,     // unix ms
    pub(crate) updated_at: u64,     // unix ms
    pub(crate) backend_order_id: Option<String>, // IB order ID once submitted
    // TIF and extended hours
    pub(crate) tif: u8,              // 0=DAY, 1=GTC, 2=IOC
    pub(crate) outside_rth: bool,    // allow trading outside regular trading hours
    // Audit
    pub(crate) state_history: Vec<(OrderState, u64)>, // (state, timestamp_ms)
    pub(crate) rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ManagedOrderType { Market, Limit, Stop, StopLimit, TrailingStop }

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum OrderSource {
    ChartClick,     // Click on chart
    DomLadder,      // DOM price ladder
    DomButton,      // DOM buy/sell button
    OrderPanel,     // Order entry panel
    Hotkey,         // Keyboard shortcut
    Bracket,        // Auto-generated bracket leg
    Trigger,        // Trigger/conditional order
    Api,            // External API
    Oco,            // OCO group leg
    Combo,          // Combo/spread order
    Conditional,    // Conditional order
    OptionsTrigger, // Options trigger order
}

impl ManagedOrder {
    /// Convert to legacy OrderLevel for rendering compatibility
    pub(crate) fn to_order_level(&self) -> OrderLevel {
        OrderLevel {
            id: self.id as u32,
            side: self.side,
            price: self.price,
            qty: self.qty,
            status: self.state.to_legacy(),
            pair_id: self.pair_id.map(|p| p as u32),
            option_symbol: self.option_symbol.clone(),
            option_con_id: self.option_con_id,
            trail_amount: self.trail_amount,
            trail_percent: self.trail_percent,
        }
    }
}

// ─── Order Intent (what UI components submit) ───────────────────────────────

/// An order intent — the standardized request that ALL UI components create.
/// The OrderManager validates and processes these.
/// `last_price` is used for fat-finger checks (current market price of the symbol).
#[derive(Debug, Clone)]
pub(crate) struct OrderIntent {
    pub(crate) symbol: String,
    pub(crate) side: OrderSide,
    pub(crate) order_type: ManagedOrderType,
    pub(crate) price: f32,         // limit price (0.0 for market orders)
    pub(crate) stop_price: f32,    // stop trigger price (for stop/stop-limit orders)
    pub(crate) qty: u32,
    pub(crate) source: OrderSource,
    pub(crate) pair_with: Option<u64>, // link to another order (OCO)
    pub(crate) option_symbol: Option<String>,
    pub(crate) option_con_id: Option<i64>,
    // Trailing stop fields
    pub(crate) trail_amount: Option<f32>,
    pub(crate) trail_percent: Option<f32>,
    // Market context for risk checks
    pub(crate) last_price: f32,  // current market price (for fat-finger check, 0=skip)
    // TIF and extended hours
    pub(crate) tif: u8,           // 0=DAY, 1=GTC, 2=IOC
    pub(crate) outside_rth: bool, // allow trading outside regular trading hours
}

// ─── Conditional Order Intent ─────────────────────────────────────────────────

/// A price condition for conditional orders.
#[derive(Debug, Clone)]
pub(crate) struct OrderCondition {
    pub(crate) con_id: i64,       // contract to watch
    pub(crate) exchange: String,  // "SMART" default
    pub(crate) is_more: bool,     // true = trigger when >= price
    pub(crate) price: f32,
}

/// Intent for a conditional order (order with price conditions on any contract).
#[derive(Debug, Clone)]
pub(crate) struct ConditionalOrderIntent {
    pub(crate) base: OrderIntent,
    pub(crate) conditions: Vec<OrderCondition>,
    pub(crate) conditions_logic: String, // "and" or "or"
    pub(crate) conditions_cancel_order: bool,
}

// ─── Combo/Spread Order ───────────────────────────────────────────────────────

/// A single leg of a combo/spread order.
#[derive(Debug, Clone)]
pub(crate) struct ComboLeg {
    pub(crate) con_id: i64,
    pub(crate) ratio: i32,
    pub(crate) side: String,
}

/// Result of submitting an intent
#[derive(Debug, Clone)]
pub(crate) enum OrderResult {
    Accepted(u64),           // Order ID
    NeedsConfirmation(u64),  // Order ID, requires armed/confirm
    Rejected(String),        // Reason
    Duplicate,               // Dedup caught it
}

// ─── Risk Limits ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct RiskLimits {
    pub(crate) max_order_qty: u32,
    pub(crate) max_position_qty: u32,
    pub(crate) max_daily_loss: f64,
    pub(crate) max_open_orders: usize,
    pub(crate) max_notional: f64,          // max $ value per order (0=disabled)
    pub(crate) fat_finger_pct: f32,        // max % deviation from last price (0=disabled), only on OPENING orders
    pub(crate) dedup_cooldown_ms: u64,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_order_qty: 10_000,
            max_position_qty: 50_000,
            max_daily_loss: 50_000.0,
            max_open_orders: 100,
            max_notional: 500_000.0,
            fat_finger_pct: 5.0,     // 5% deviation from last price on opening orders
            dedup_cooldown_ms: 500,
        }
    }
}

// ─── Order Manager ──────────────────────────────────────────────────────────

pub(crate) struct OrderManager {
    orders: Vec<ManagedOrder>,
    next_id: u64,
    armed: bool,
    paper_mode: bool,
    risk_limits: RiskLimits,
    // Dedup tracking
    recent_signatures: HashMap<OrderSignature, Instant>,
    // Pending actions for the render thread to process
    pending_toasts: Vec<String>,
    // Stats
    orders_submitted: u64,
    orders_filled: u64,
    orders_rejected: u64,
    duplicates_blocked: u64,
}

static ORDER_MANAGER: OnceLock<Mutex<OrderManager>> = OnceLock::new();

impl OrderManager {
    fn new() -> Self {
        Self {
            orders: Vec::new(),
            next_id: 1,
            armed: false,
            paper_mode: true,
            risk_limits: RiskLimits::default(),
            recent_signatures: HashMap::new(),
            pending_toasts: Vec::new(),
            orders_submitted: 0,
            orders_filled: 0,
            orders_rejected: 0,
            duplicates_blocked: 0,
        }
    }

    /// Resolve a symbol to its IB conId.
    fn resolve_con_id(client: &reqwest::blocking::Client, symbol: &str) -> Option<i64> {
        client.get(format!("{}/contract/{}", APEXIB_URL, symbol))
            .timeout(std::time::Duration::from_secs(5)).send()
            .and_then(|r| r.json::<serde_json::Value>())
            .ok()
            .and_then(|j| j["conId"].as_i64())
    }

    /// Extract orderId from a JSON response.
    fn extract_order_id(json: &serde_json::Value) -> Option<String> {
        json["orderId"].as_str().map(|s| s.to_string())
            .or_else(|| json["orderId"].as_i64().map(|n| n.to_string()))
    }

    /// Submit an order to ApexIB and return the backend order ID.
    /// Handles all order types: market, limit, stop, stop_limit, trailing_stop.
    fn submit_to_ib(symbol: &str, side: &str, qty: u32, order_type_idx: usize,
                    price: f32, stop_price: f32,
                    trail_amount: Option<f32>, trail_percent: Option<f32>,
                    idempotency_key: &str, intent_tif: u8, outside_rth: bool) -> Option<String> {
        let client = reqwest::blocking::Client::new();
        let con_id = Self::resolve_con_id(&client, symbol)?;

        let order_type = match order_type_idx {
            0 => "market", 1 => "limit", 2 => "stop",
            3 => "stop_limit", 4 => "trailing_stop", _ => "market"
        };
        let tif = match intent_tif { 0 => "day", 1 => "gtc", 2 => "ioc", _ => "day" };
        let mut body = serde_json::json!({
            "conId": con_id, "side": side, "quantity": qty,
            "orderType": order_type, "tif": tif,
            "idempotencyKey": idempotency_key,
        });
        if outside_rth { body["outsideRth"] = serde_json::json!(true); }
        match order_type {
            "limit" => { body["limitPrice"] = serde_json::json!(price); }
            "stop" => { body["stopPrice"] = serde_json::json!(if stop_price != 0.0 { stop_price } else { price }); }
            "stop_limit" => {
                body["limitPrice"] = serde_json::json!(price);
                body["stopPrice"] = serde_json::json!(stop_price);
            }
            "trailing_stop" => {
                if let Some(amt) = trail_amount { body["trailAmount"] = serde_json::json!(amt); }
                if let Some(pct) = trail_percent { body["trailPercent"] = serde_json::json!(pct); }
                if stop_price != 0.0 { body["stopPrice"] = serde_json::json!(stop_price); }
            }
            _ => {} // market — no price fields
        }

        let resp = client.post(format!("{}/orders", APEXIB_URL))
            .json(&body).timeout(std::time::Duration::from_secs(5)).send().ok()?;
        let json: serde_json::Value = resp.json().ok()?;
        Self::extract_order_id(&json)
    }

    /// Submit an order intent. Returns the result.
    pub(crate) fn submit(&mut self, intent: OrderIntent) -> OrderResult {
        let now_ms = epoch_ms();

        // ── 1. Dedup check ──
        let sig = OrderSignature::new(&intent.symbol, intent.side, intent.price, intent.qty);
        self.cleanup_expired_signatures();
        if let Some(last_time) = self.recent_signatures.get(&sig) {
            if last_time.elapsed().as_millis() < self.risk_limits.dedup_cooldown_ms as u128 {
                self.duplicates_blocked += 1;
                return OrderResult::Duplicate;
            }
        }
        self.recent_signatures.insert(sig, Instant::now());

        // ── 2. Risk validation ──
        if intent.qty > self.risk_limits.max_order_qty {
            self.orders_rejected += 1;
            return OrderResult::Rejected(format!("Qty {} exceeds max {}", intent.qty, self.risk_limits.max_order_qty));
        }
        if intent.qty == 0 {
            return OrderResult::Rejected("Qty cannot be zero".into());
        }
        let active_count = self.orders.iter().filter(|o| o.state.is_active()).count();
        if active_count >= self.risk_limits.max_open_orders {
            self.orders_rejected += 1;
            return OrderResult::Rejected(format!("Max {} open orders reached", self.risk_limits.max_open_orders));
        }
        // Position limit check
        let net_position: i64 = self.orders.iter()
            .filter(|o| o.symbol == intent.symbol && o.state == OrderState::Filled)
            .map(|o| match o.side {
                OrderSide::Buy | OrderSide::TriggerBuy => o.filled_qty as i64,
                _ => -(o.filled_qty as i64),
            }).sum();
        let new_position = match intent.side {
            OrderSide::Buy | OrderSide::TriggerBuy => net_position + intent.qty as i64,
            _ => net_position - intent.qty as i64,
        };
        if new_position.unsigned_abs() as u32 > self.risk_limits.max_position_qty {
            self.orders_rejected += 1;
            return OrderResult::Rejected(format!("Would exceed max position size {}", self.risk_limits.max_position_qty));
        }

        // ── 2.5 Oversell protection (can't sell more contracts than you hold) ──
        let is_sell = matches!(intent.side, OrderSide::Sell | OrderSide::Stop | OrderSide::OcoStop | OrderSide::TriggerSell);
        if is_sell && net_position > 0 {
            // Selling to close — can't sell more than current long position
            let sell_qty = intent.qty as i64;
            if sell_qty > net_position {
                self.orders_rejected += 1;
                return OrderResult::Rejected(format!("Can't sell {} — only holding {} contracts", intent.qty, net_position));
            }
        }
        let is_buy = matches!(intent.side, OrderSide::Buy | OrderSide::TriggerBuy);
        if is_buy && net_position < 0 {
            // Buying to close — can't buy more than current short position
            let buy_qty = intent.qty as i64;
            if buy_qty > net_position.abs() {
                self.orders_rejected += 1;
                return OrderResult::Rejected(format!("Can't buy {} — only short {} contracts", intent.qty, net_position.abs()));
            }
        }

        // ── 2.6 Fat-finger price check (ONLY on opening orders, not closing) ──
        let is_opening = (is_buy && net_position >= 0) || (is_sell && net_position <= 0);
        if is_opening && self.risk_limits.fat_finger_pct > 0.0 && intent.last_price > 0.0 && intent.price > 0.0
            && intent.order_type != ManagedOrderType::Market
        {
            let deviation_pct = ((intent.price - intent.last_price) / intent.last_price * 100.0).abs();
            if deviation_pct > self.risk_limits.fat_finger_pct {
                self.orders_rejected += 1;
                return OrderResult::Rejected(format!(
                    "Fat-finger: price {:.2} is {:.1}% from market {:.2} (max {}%)",
                    intent.price, deviation_pct, intent.last_price, self.risk_limits.fat_finger_pct
                ));
            }
        }

        // ── 2.7 Max notional check ──
        if self.risk_limits.max_notional > 0.0 {
            let order_price = if intent.price > 0.0 { intent.price } else { intent.last_price };
            let notional = order_price as f64 * intent.qty as f64;
            if notional > self.risk_limits.max_notional {
                self.orders_rejected += 1;
                return OrderResult::Rejected(format!(
                    "Notional ${:.0} exceeds max ${:.0}", notional, self.risk_limits.max_notional
                ));
            }
        }

        // Margin validation happens server-side at IB. The client-side
        // pre-check used to live here but blocked the render thread on two
        // sequential HTTP calls.

        // ── 3. Create managed order ──
        let id = self.next_id;
        self.next_id += 1;

        let initial_state = if self.armed || intent.order_type == ManagedOrderType::Market {
            OrderState::PendingSubmit
        } else {
            OrderState::Draft
        };

        let order = ManagedOrder {
            id,
            symbol: intent.symbol.clone(),
            side: intent.side,
            order_type: intent.order_type,
            price: intent.price,
            stop_price: intent.stop_price,
            qty: intent.qty,
            filled_qty: 0,
            avg_fill_price: 0.0,
            state: initial_state,
            pair_id: intent.pair_with,
            trail_amount: intent.trail_amount,
            trail_percent: intent.trail_percent,
            option_symbol: intent.option_symbol,
            option_con_id: intent.option_con_id,
            source: intent.source,
            tif: intent.tif,
            outside_rth: intent.outside_rth,
            created_at: now_ms,
            updated_at: now_ms,
            backend_order_id: None,
            state_history: vec![(initial_state, now_ms)],
            rejection_reason: None,
        };

        self.orders.push(order);
        self.orders_submitted += 1;

        if initial_state == OrderState::PendingSubmit {
            // Submit to ApexIB backend
            let sym = intent.symbol.clone();
            let side_str = match intent.side {
                OrderSide::Buy | OrderSide::TriggerBuy => "buy",
                _ => "sell",
            };
            let ot_idx = match intent.order_type {
                ManagedOrderType::Market => 0,
                ManagedOrderType::Limit => 1,
                ManagedOrderType::Stop => 2,
                ManagedOrderType::StopLimit => 3,
                ManagedOrderType::TrailingStop => 4,
            };
            let price = intent.price;
            let stop_price = intent.stop_price;
            let trail_amount = intent.trail_amount;
            let trail_percent = intent.trail_percent;
            let qty = intent.qty;
            let intent_tif = intent.tif;
            let outside_rth = intent.outside_rth;
            let idem_key = format!("apex_{}_{}_{}", id, intent.symbol, now_ms);
            let order_id_copy = id;
            // Fire async backend submission — captures IB order ID back into the manager
            std::thread::spawn(move || {
                if let Some(ib_oid) = Self::submit_to_ib(&sym, side_str, qty, ot_idx, price, stop_price, trail_amount, trail_percent, &idem_key, intent_tif, outside_rth) {
                    with_mgr(|mgr| {
                        if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == order_id_copy) {
                            o.backend_order_id = Some(ib_oid);
                        }
                    });
                }
            });
            self.transition(id, OrderState::Working); // Optimistic — will reconcile from poller
            self.pending_toasts.push(format!("{} {} x{} @ {:.2}", side_str.to_uppercase(), intent.symbol, qty, price));
            OrderResult::Accepted(id)
        } else {
            OrderResult::NeedsConfirmation(id)
        }
    }

    /// Confirm a draft order (when not armed, user explicitly confirms)
    pub(crate) fn confirm(&mut self, order_id: u64) -> bool {
        if let Some(o) = self.orders.iter_mut().find(|o| o.id == order_id && o.state == OrderState::Draft) {
            let now = epoch_ms();
            o.state = OrderState::Working;
            o.updated_at = now;
            o.state_history.push((OrderState::PendingSubmit, now));
            o.state_history.push((OrderState::Working, now));
            // Submit to ApexIB
            let sym = o.symbol.clone();
            let side_str = match o.side { OrderSide::Buy | OrderSide::TriggerBuy => "buy", _ => "sell" };
            let ot_idx = match o.order_type {
                ManagedOrderType::Market => 0, ManagedOrderType::Limit => 1,
                ManagedOrderType::Stop => 2, ManagedOrderType::StopLimit => 3,
                ManagedOrderType::TrailingStop => 4,
            };
            let price = o.price; let stop_price = o.stop_price; let qty = o.qty;
            let trail_amount = o.trail_amount;
            let trail_percent = o.trail_percent;
            let intent_tif = o.tif;
            let outside_rth = o.outside_rth;
            let idem_key = format!("apex_{}_{}_{}", order_id, sym, now);
            let order_id_copy = order_id;
            std::thread::spawn(move || {
                if let Some(ib_oid) = Self::submit_to_ib(&sym, side_str, qty, ot_idx, price, stop_price, trail_amount, trail_percent, &idem_key, intent_tif, outside_rth) {
                    with_mgr(|mgr| {
                        if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == order_id_copy) {
                            o.backend_order_id = Some(ib_oid);
                        }
                    });
                }
            });
            true
        } else {
            false
        }
    }

    // ─── Bracket Orders ──────────────────────────────────────────────────────

    /// Submit a bracket order (entry + take profit + stop loss) via POST /orders/bracket.
    /// Creates 3 local managed orders and submits the bracket to the backend.
    /// Returns (entry_result, tp_order_id, sl_order_id).
    pub(crate) fn submit_bracket(&mut self, intent: OrderIntent, take_profit_price: f32, stop_loss_price: f32) -> (OrderResult, Option<u64>, Option<u64>) {
        let now_ms = epoch_ms();

        // Validate
        if intent.qty == 0 {
            return (OrderResult::Rejected("Qty cannot be zero".into()), None, None);
        }
        if intent.qty > self.risk_limits.max_order_qty {
            return (OrderResult::Rejected(format!("Qty {} exceeds max {}", intent.qty, self.risk_limits.max_order_qty)), None, None);
        }

        // Create entry order
        let entry_id = self.next_id; self.next_id += 1;
        let tp_id = self.next_id; self.next_id += 1;
        let sl_id = self.next_id; self.next_id += 1;

        let initial_state = if self.armed || intent.order_type == ManagedOrderType::Market {
            OrderState::PendingSubmit
        } else {
            OrderState::Draft
        };

        let side_str = match intent.side { OrderSide::Buy | OrderSide::TriggerBuy => "buy", _ => "sell" };
        let tp_side = match intent.side { OrderSide::Buy | OrderSide::TriggerBuy => OrderSide::Sell, _ => OrderSide::Buy };

        // Entry order
        self.orders.push(ManagedOrder {
            id: entry_id, symbol: intent.symbol.clone(), side: intent.side,
            order_type: intent.order_type, price: intent.price, stop_price: intent.stop_price,
            qty: intent.qty, filled_qty: 0, avg_fill_price: 0.0,
            state: initial_state, pair_id: None,
            trail_amount: None, trail_percent: None,
            option_symbol: intent.option_symbol.clone(), option_con_id: intent.option_con_id,
            source: intent.source, tif: intent.tif, outside_rth: intent.outside_rth,
            created_at: now_ms, updated_at: now_ms,
            backend_order_id: None, state_history: vec![(initial_state, now_ms)],
            rejection_reason: None,
        });

        // Take profit order
        self.orders.push(ManagedOrder {
            id: tp_id, symbol: intent.symbol.clone(), side: tp_side,
            order_type: ManagedOrderType::Limit, price: take_profit_price, stop_price: 0.0,
            qty: intent.qty, filled_qty: 0, avg_fill_price: 0.0,
            state: initial_state, pair_id: Some(entry_id),
            trail_amount: None, trail_percent: None,
            option_symbol: intent.option_symbol.clone(), option_con_id: intent.option_con_id,
            source: OrderSource::Bracket, tif: intent.tif, outside_rth: intent.outside_rth,
            created_at: now_ms, updated_at: now_ms,
            backend_order_id: None, state_history: vec![(initial_state, now_ms)],
            rejection_reason: None,
        });

        // Stop loss order
        self.orders.push(ManagedOrder {
            id: sl_id, symbol: intent.symbol.clone(), side: tp_side,
            order_type: ManagedOrderType::Stop, price: stop_loss_price, stop_price: stop_loss_price,
            qty: intent.qty, filled_qty: 0, avg_fill_price: 0.0,
            state: initial_state, pair_id: Some(entry_id),
            trail_amount: None, trail_percent: None,
            option_symbol: intent.option_symbol.clone(), option_con_id: intent.option_con_id,
            source: OrderSource::Bracket, tif: intent.tif, outside_rth: intent.outside_rth,
            created_at: now_ms, updated_at: now_ms,
            backend_order_id: None, state_history: vec![(initial_state, now_ms)],
            rejection_reason: None,
        });

        self.orders_submitted += 3;

        if initial_state == OrderState::PendingSubmit {
            let sym = intent.symbol.clone();
            let side_owned = side_str.to_string();
            let qty = intent.qty;
            let entry_price = intent.price;
            let order_type = match intent.order_type {
                ManagedOrderType::Market => "market", ManagedOrderType::Limit => "limit",
                ManagedOrderType::Stop => "stop", ManagedOrderType::StopLimit => "stop_limit",
                ManagedOrderType::TrailingStop => "trailing_stop",
            };
            let ot_owned = order_type.to_string();
            let idem_key = format!("apex_bracket_{}_{}_{}", entry_id, sym, now_ms);
            let tp_price = take_profit_price;
            let sl_price = stop_loss_price;
            let eid = entry_id; let tid = tp_id; let sid = sl_id;

            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                let con_id = match Self::resolve_con_id(&client, &sym) {
                    Some(c) => c, None => { eprintln!("[bracket] no conId for {}", sym); return; }
                };

                let mut entry_leg = serde_json::json!({"orderType": ot_owned});
                match ot_owned.as_str() {
                    "limit" => { entry_leg["limitPrice"] = serde_json::json!(entry_price); }
                    "stop" => { entry_leg["stopPrice"] = serde_json::json!(entry_price); }
                    "stop_limit" => { entry_leg["limitPrice"] = serde_json::json!(entry_price); entry_leg["stopPrice"] = serde_json::json!(entry_price); }
                    _ => {} // market
                }

                let body = serde_json::json!({
                    "conId": con_id, "side": side_owned, "quantity": qty,
                    "entry": entry_leg,
                    "takeProfit": {"orderType": "limit", "limitPrice": tp_price},
                    "stopLoss": {"orderType": "stop", "stopPrice": sl_price},
                    "tif": "day",
                    "idempotencyKey": idem_key,
                });

                match client.post(format!("{}/orders/bracket", APEXIB_URL))
                    .json(&body).timeout(std::time::Duration::from_secs(5)).send() {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            with_mgr(|mgr| {
                                if let Some(oid) = json["parentOrderId"].as_str().map(|s| s.to_string())
                                    .or_else(|| json["parentOrderId"].as_i64().map(|n| n.to_string())) {
                                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == eid) { o.backend_order_id = Some(oid); }
                                }
                                if let Some(oid) = json["takeProfitOrderId"].as_str().map(|s| s.to_string())
                                    .or_else(|| json["takeProfitOrderId"].as_i64().map(|n| n.to_string())) {
                                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == tid) { o.backend_order_id = Some(oid); }
                                }
                                if let Some(oid) = json["stopLossOrderId"].as_str().map(|s| s.to_string())
                                    .or_else(|| json["stopLossOrderId"].as_i64().map(|n| n.to_string())) {
                                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == sid) { o.backend_order_id = Some(oid); }
                                }
                            });
                        }
                    }
                    Err(e) => eprintln!("[bracket] submit failed: {e}"),
                }
            });
            self.transition(entry_id, OrderState::Working);
            self.transition(tp_id, OrderState::Working);
            self.transition(sl_id, OrderState::Working);
            self.pending_toasts.push(format!("BRACKET {} {} x{} entry={:.2} TP={:.2} SL={:.2}",
                side_str.to_uppercase(), intent.symbol, intent.qty, intent.price, take_profit_price, stop_loss_price));
            (OrderResult::Accepted(entry_id), Some(tp_id), Some(sl_id))
        } else {
            (OrderResult::NeedsConfirmation(entry_id), Some(tp_id), Some(sl_id))
        }
    }

    // ─── OCO Orders ────────────────────────────────────────────────────────────

    /// Submit an OCO (One-Cancels-Other) order group via POST /orders/oco.
    /// Creates all orders locally paired together, then submits to the backend.
    pub(crate) fn submit_oco(&mut self, orders: Vec<OrderIntent>) -> Vec<OrderResult> {
        let now_ms = epoch_ms();
        if orders.len() < 2 {
            return vec![OrderResult::Rejected("OCO requires at least 2 orders".into())];
        }

        let oca_group = format!("apex_oco_{}_{}", self.next_id, now_ms);
        let mut results = Vec::new();
        let mut local_ids: Vec<u64> = Vec::new();

        // Create all local orders
        for intent in &orders {
            if intent.qty == 0 {
                results.push(OrderResult::Rejected("Qty cannot be zero".into()));
                continue;
            }
            let id = self.next_id; self.next_id += 1;
            local_ids.push(id);

            let initial_state = if self.armed { OrderState::PendingSubmit } else { OrderState::Draft };

            self.orders.push(ManagedOrder {
                id, symbol: intent.symbol.clone(), side: intent.side,
                order_type: intent.order_type, price: intent.price, stop_price: intent.stop_price,
                qty: intent.qty, filled_qty: 0, avg_fill_price: 0.0,
                state: initial_state, pair_id: None, // pair_ids linked after all created
                trail_amount: intent.trail_amount, trail_percent: intent.trail_percent,
                option_symbol: intent.option_symbol.clone(), option_con_id: intent.option_con_id,
                source: OrderSource::Oco, tif: intent.tif, outside_rth: intent.outside_rth,
                created_at: now_ms, updated_at: now_ms,
                backend_order_id: None, state_history: vec![(initial_state, now_ms)],
                rejection_reason: None,
            });
            self.orders_submitted += 1;

            if initial_state == OrderState::PendingSubmit {
                self.transition(id, OrderState::Working);
                results.push(OrderResult::Accepted(id));
            } else {
                results.push(OrderResult::NeedsConfirmation(id));
            }
        }

        // Link each order to the first other order in the group (simplified pair linkage)
        if local_ids.len() >= 2 {
            for i in 0..local_ids.len() {
                let pair = if i == 0 { local_ids[1] } else { local_ids[0] };
                if let Some(o) = self.orders.iter_mut().find(|o| o.id == local_ids[i]) {
                    o.pair_id = Some(pair);
                }
            }
        }

        // Submit to backend
        if self.armed && !local_ids.is_empty() {
            let oca = oca_group.clone();
            let order_intents: Vec<(String, String, u32, String, f32, f32)> = orders.iter().map(|i| {
                let side = match i.side { OrderSide::Buy | OrderSide::TriggerBuy => "BUY", _ => "SELL" };
                let ot = match i.order_type {
                    ManagedOrderType::Market => "market", ManagedOrderType::Limit => "limit",
                    ManagedOrderType::Stop => "stop", ManagedOrderType::StopLimit => "stop_limit",
                    ManagedOrderType::TrailingStop => "trailing_stop",
                };
                (i.symbol.clone(), side.to_string(), i.qty, ot.to_string(), i.price, i.stop_price)
            }).collect();
            let ids_copy = local_ids.clone();

            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                // Build the orders array for the backend
                let mut oco_orders = Vec::new();
                for (sym, side, qty, ot, price, stop_price) in &order_intents {
                    let con_id = match Self::resolve_con_id(&client, sym) {
                        Some(c) => c, None => continue,
                    };
                    let mut order_json = serde_json::json!({
                        "conId": con_id, "side": side, "quantity": qty,
                        "orderType": ot, "tif": "day",
                    });
                    match ot.as_str() {
                        "limit" => { order_json["limitPrice"] = serde_json::json!(price); }
                        "stop" => { order_json["stopPrice"] = serde_json::json!(if *stop_price != 0.0 { *stop_price } else { *price }); }
                        "stop_limit" => { order_json["limitPrice"] = serde_json::json!(price); order_json["stopPrice"] = serde_json::json!(stop_price); }
                        _ => {}
                    }
                    oco_orders.push(order_json);
                }

                let body = serde_json::json!({ "orders": oco_orders, "ocaGroup": oca });
                match client.post(format!("{}/orders/oco", APEXIB_URL))
                    .json(&body).timeout(std::time::Duration::from_secs(5)).send() {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            if let Some(backend_ids) = json["orderIds"].as_array() {
                                with_mgr(|mgr| {
                                    for (i, bid) in backend_ids.iter().enumerate() {
                                        if i < ids_copy.len() {
                                            let oid = bid.as_str().map(|s| s.to_string())
                                                .or_else(|| bid.as_i64().map(|n| n.to_string()));
                                            if let Some(oid) = oid {
                                                if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == ids_copy[i]) {
                                                    o.backend_order_id = Some(oid);
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    }
                    Err(e) => eprintln!("[oco] submit failed: {e}"),
                }
            });
            self.pending_toasts.push(format!("OCO group {} with {} orders", oca_group, local_ids.len()));
        }

        results
    }

    // ─── Conditional Orders ────────────────────────────────────────────────────

    /// Submit a conditional order via POST /orders/conditional.
    /// The order executes when price conditions on watched contracts are met.
    pub(crate) fn submit_conditional(&mut self, intent: ConditionalOrderIntent) -> OrderResult {
        let now_ms = epoch_ms();
        let base = &intent.base;

        if base.qty == 0 {
            return OrderResult::Rejected("Qty cannot be zero".into());
        }
        if intent.conditions.is_empty() {
            return OrderResult::Rejected("Conditional order requires at least one condition".into());
        }

        let id = self.next_id; self.next_id += 1;
        let initial_state = OrderState::PendingSubmit; // conditionals always go to backend immediately

        self.orders.push(ManagedOrder {
            id, symbol: base.symbol.clone(), side: base.side,
            order_type: base.order_type, price: base.price, stop_price: base.stop_price,
            qty: base.qty, filled_qty: 0, avg_fill_price: 0.0,
            state: initial_state, pair_id: base.pair_with,
            trail_amount: base.trail_amount, trail_percent: base.trail_percent,
            option_symbol: base.option_symbol.clone(), option_con_id: base.option_con_id,
            source: OrderSource::Conditional, tif: base.tif, outside_rth: base.outside_rth,
            created_at: now_ms, updated_at: now_ms,
            backend_order_id: None, state_history: vec![(initial_state, now_ms)],
            rejection_reason: None,
        });
        self.orders_submitted += 1;

        let sym = base.symbol.clone();
        let side_str = match base.side { OrderSide::Buy | OrderSide::TriggerBuy => "BUY", _ => "SELL" }.to_string();
        let ot = match base.order_type {
            ManagedOrderType::Market => "market", ManagedOrderType::Limit => "limit",
            ManagedOrderType::Stop => "stop", ManagedOrderType::StopLimit => "stop_limit",
            ManagedOrderType::TrailingStop => "trailing_stop",
        }.to_string();
        let price = base.price;
        let qty = base.qty;
        let conditions = intent.conditions.clone();
        let logic = intent.conditions_logic.clone();
        let cancel_order = intent.conditions_cancel_order;
        let idem_key = format!("apex_cond_{}_{}_{}", id, sym, now_ms);
        let id_copy = id;

        // Build toast before move
        let toast = format!("CONDITIONAL {} {} x{} with {} conditions",
            side_str, sym, qty, conditions.len());

        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let con_id = match Self::resolve_con_id(&client, &sym) {
                Some(c) => c, None => { eprintln!("[conditional] no conId for {}", sym); return; }
            };

            let conds_json: Vec<serde_json::Value> = conditions.iter().map(|c| {
                serde_json::json!({
                    "type": "price",
                    "conId": c.con_id,
                    "exchange": c.exchange,
                    "isMore": c.is_more,
                    "price": c.price,
                    "triggerMethod": "default",
                })
            }).collect();

            let mut body = serde_json::json!({
                "conId": con_id, "side": side_str, "quantity": qty,
                "orderType": ot, "tif": "day",
                "conditions": conds_json,
                "conditionsLogic": logic,
                "conditionsCancelOrder": cancel_order,
                "outsideRth": true,
                "idempotencyKey": idem_key,
            });
            if ot == "limit" { body["limitPrice"] = serde_json::json!(price); }

            match client.post(format!("{}/orders/conditional", APEXIB_URL))
                .json(&body).timeout(std::time::Duration::from_secs(5)).send() {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        if let Some(oid) = Self::extract_order_id(&json) {
                            with_mgr(|mgr| {
                                if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == id_copy) {
                                    o.backend_order_id = Some(oid);
                                }
                            });
                        }
                    }
                }
                Err(e) => eprintln!("[conditional] submit failed: {e}"),
            }
        });

        self.transition(id, OrderState::Working);
        self.pending_toasts.push(toast);
        OrderResult::Accepted(id)
    }

    // ─── Options Trigger ───────────────────────────────────────────────────────

    /// Submit an options trigger via POST /orders/options-trigger.
    /// Creates entry + exit conditional orders on an option based on underlying price levels.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn submit_options_trigger(&mut self,
        underlying: &str, option_type: &str, strike: f32, expiration: &str,
        qty: u32, entry_price: f32, entry_direction: &str,
        exit_price: f32, exit_direction: &str,
    ) -> OrderResult {
        let now_ms = epoch_ms();
        if qty == 0 {
            return OrderResult::Rejected("Qty cannot be zero".into());
        }

        let id = self.next_id; self.next_id += 1;
        let initial_state = OrderState::PendingSubmit;

        self.orders.push(ManagedOrder {
            id, symbol: underlying.to_string(), side: OrderSide::TriggerBuy,
            order_type: ManagedOrderType::Market, price: entry_price, stop_price: 0.0,
            qty, filled_qty: 0, avg_fill_price: 0.0,
            state: initial_state, pair_id: None,
            trail_amount: None, trail_percent: None,
            option_symbol: Some(format!("{} {}{} {}", underlying, strike, option_type, expiration)),
            option_con_id: None,
            source: OrderSource::OptionsTrigger, tif: 0, outside_rth: false,
            created_at: now_ms, updated_at: now_ms,
            backend_order_id: None, state_history: vec![(initial_state, now_ms)],
            rejection_reason: None,
        });
        self.orders_submitted += 1;

        let und = underlying.to_string();
        let ot = option_type.to_string();
        let exp = expiration.to_string();
        let entry_dir = entry_direction.to_string();
        let exit_dir = exit_direction.to_string();
        let idem_key = format!("apex_opttrig_{}_{}_{}", id, underlying, now_ms);
        let id_copy = id;

        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let body = serde_json::json!({
                "underlying": und,
                "optionType": ot,
                "strike": strike,
                "expiration": exp,
                "quantity": qty,
                "entryPrice": entry_price,
                "entryDirection": entry_dir,
                "exitPrice": exit_price,
                "exitDirection": exit_dir,
                "exitOrderType": "market",
                "idempotencyKey": idem_key,
            });
            match client.post(format!("{}/orders/options-trigger", APEXIB_URL))
                .json(&body).timeout(std::time::Duration::from_secs(5)).send() {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        with_mgr(|mgr| {
                            if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == id_copy) {
                                if let Some(oid) = json["entryOrderId"].as_str().map(|s| s.to_string()) {
                                    o.backend_order_id = Some(oid);
                                }
                                if let Some(con_id) = json["optionConId"].as_i64() {
                                    o.option_con_id = Some(con_id);
                                }
                            }
                        });
                    }
                }
                Err(e) => eprintln!("[options-trigger] submit failed: {e}"),
            }
        });

        self.transition(id, OrderState::Working);
        self.pending_toasts.push(format!("OPT TRIGGER {} {}{}@{} entry={:.2} exit={:.2}",
            underlying, strike, option_type, expiration, entry_price, exit_price));
        OrderResult::Accepted(id)
    }

    // ─── Combo/Spread Orders ───────────────────────────────────────────────────

    /// Submit a combo/spread order via POST /orders/combo.
    /// All legs fill atomically via IB's native combo mechanism.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn submit_combo(&mut self, symbol: &str, legs: Vec<ComboLeg>,
        side: &str, qty: u32, order_type: &str, limit_price: Option<f32>,
    ) -> OrderResult {
        let now_ms = epoch_ms();
        if qty == 0 {
            return OrderResult::Rejected("Qty cannot be zero".into());
        }
        if legs.is_empty() {
            return OrderResult::Rejected("Combo requires at least one leg".into());
        }

        let id = self.next_id; self.next_id += 1;
        let initial_state = OrderState::PendingSubmit;

        let order_side = if side.eq_ignore_ascii_case("buy") { OrderSide::Buy } else { OrderSide::Sell };
        let managed_ot = if order_type == "limit" { ManagedOrderType::Limit } else { ManagedOrderType::Market };

        self.orders.push(ManagedOrder {
            id, symbol: symbol.to_string(), side: order_side,
            order_type: managed_ot, price: limit_price.unwrap_or(0.0), stop_price: 0.0,
            qty, filled_qty: 0, avg_fill_price: 0.0,
            state: initial_state, pair_id: None,
            trail_amount: None, trail_percent: None,
            option_symbol: Some(format!("{} combo {}leg", symbol, legs.len())),
            option_con_id: None,
            source: OrderSource::Combo, tif: 0, outside_rth: false,
            created_at: now_ms, updated_at: now_ms,
            backend_order_id: None, state_history: vec![(initial_state, now_ms)],
            rejection_reason: None,
        });
        self.orders_submitted += 1;

        let sym = symbol.to_string();
        let side_owned = side.to_string();
        let ot_owned = order_type.to_string();
        let idem_key = format!("apex_combo_{}_{}_{}", id, symbol, now_ms);
        let id_copy = id;
        let num_legs = legs.len();

        // Build toast before move
        let toast = format!("COMBO {} {} x{} ({} legs)", side.to_uppercase(), symbol, qty, num_legs);

        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let legs_json: Vec<serde_json::Value> = legs.iter().map(|l| {
                serde_json::json!({
                    "conId": l.con_id,
                    "ratio": l.ratio,
                    "side": l.side,
                })
            }).collect();

            let mut url = format!("{}/orders/combo?symbol={}&side={}&quantity={}&orderType={}&tif=day&idempotencyKey={}",
                APEXIB_URL, sym, side_owned, qty, ot_owned, idem_key);
            if let Some(lp) = limit_price {
                url.push_str(&format!("&limitPrice={}", lp));
            }

            match client.post(&url)
                .json(&legs_json)
                .timeout(std::time::Duration::from_secs(5)).send() {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        if let Some(oid) = Self::extract_order_id(&json) {
                            with_mgr(|mgr| {
                                if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == id_copy) {
                                    o.backend_order_id = Some(oid);
                                }
                            });
                        }
                    }
                }
                Err(e) => eprintln!("[combo] submit failed: {e}"),
            }
        });

        self.transition(id, OrderState::Working);
        self.pending_toasts.push(toast);
        OrderResult::Accepted(id)
    }

    /// Cancel an order
    pub(crate) fn cancel(&mut self, order_id: u64) -> bool {
        // Find the order and its pair_id
        let (is_active, pair_id) = self.orders.iter()
            .find(|o| o.id == order_id)
            .map(|o| (o.state.is_active(), o.pair_id))
            .unwrap_or((false, None));

        if !is_active { return false; }

        self.transition(order_id, OrderState::Cancelled);

        // Send individual cancel to ApexIB
        if let Some(backend_id) = self.orders.iter().find(|o| o.id == order_id).and_then(|o| o.backend_order_id.clone()) {
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                let _ = client.delete(format!("{}/orders/{}", APEXIB_URL, backend_id))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }

        // Also cancel paired order
        if let Some(pid) = pair_id {
            let pair_active = self.orders.iter().any(|o| o.id == pid && o.state.is_active());
            if pair_active {
                self.transition(pid, OrderState::Cancelled);
                // Cancel paired order on backend too
                if let Some(pair_backend_id) = self.orders.iter().find(|o| o.id == pid).and_then(|o| o.backend_order_id.clone()) {
                    std::thread::spawn(move || {
                        let client = reqwest::blocking::Client::new();
                        let _ = client.delete(format!("{}/orders/{}", APEXIB_URL, pair_backend_id))
                            .timeout(std::time::Duration::from_secs(5)).send();
                    });
                }
            }
        }
        true
    }

    /// Cancel all active orders for a symbol (or all if symbol is empty)
    /// Also sends cancel to ApexIB backend
    pub(crate) fn cancel_all(&mut self, symbol: &str) {
        // Send cancel-all to IB backend
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let _ = client.delete(format!("{}/orders", APEXIB_URL))
                .timeout(std::time::Duration::from_secs(5)).send();
        });
        let ids: Vec<u64> = self.orders.iter()
            .filter(|o| o.state.is_active() && (symbol.is_empty() || o.symbol == symbol))
            .map(|o| o.id)
            .collect();
        for id in ids {
            self.cancel(id);
        }
    }

    /// Modify an order's price (drag-to-move, etc.)
    pub(crate) fn modify_price(&mut self, order_id: u64, new_price: f32) -> bool {
        if let Some(o) = self.orders.iter_mut().find(|o| o.id == order_id && o.state.is_active()) {
            let now = epoch_ms();
            o.price = new_price;
            o.updated_at = now;
            o.state_history.push((OrderState::PendingModify, now));
            // Send modify to ApexIB backend — use the right price field for the order type
            if let Some(ref bid) = o.backend_order_id {
                let bid = bid.clone();
                let np = new_price;
                let is_stop = matches!(o.order_type, ManagedOrderType::Stop);
                let is_stop_limit = matches!(o.order_type, ManagedOrderType::StopLimit);
                std::thread::spawn(move || {
                    let client = reqwest::blocking::Client::new();
                    let body = if is_stop {
                        serde_json::json!({"stopPrice": np})
                    } else if is_stop_limit {
                        serde_json::json!({"limitPrice": np, "stopPrice": np})
                    } else {
                        serde_json::json!({"limitPrice": np})
                    };
                    let _ = client.put(format!("{}/orders/{}", APEXIB_URL, bid))
                        .json(&body)
                        .timeout(std::time::Duration::from_secs(5)).send();
                });
            }
            if o.state == OrderState::Working {
                o.state = OrderState::PendingModify;
                // Optimistic: go back to Working after modify
                o.state = OrderState::Working;
                o.state_history.push((OrderState::Working, now));
            }
            true
        } else {
            false
        }
    }

    /// Flatten: cancel all orders + submit market close for position
    pub(crate) fn flatten(&mut self, symbol: &str, current_qty: i32) {
        self.cancel_all(symbol);
        if current_qty != 0 {
            let side = if current_qty > 0 { OrderSide::Sell } else { OrderSide::Buy };
            let qty = current_qty.unsigned_abs();
            self.submit(OrderIntent {
                symbol: symbol.to_string(),
                side,
                order_type: ManagedOrderType::Market,
                price: 0.0,
                stop_price: 0.0,
                qty,
                source: OrderSource::DomButton,
                pair_with: None,
                option_symbol: None,
                option_con_id: None,
                trail_amount: None,
                trail_percent: None,
                last_price: 0.0,
                tif: 0,
                outside_rth: false,
            });
        }
    }

    /// Get all orders for a symbol (active + recent terminal)
    pub(crate) fn orders_for_symbol(&self, symbol: &str) -> Vec<&ManagedOrder> {
        self.orders.iter()
            .filter(|o| o.symbol == symbol)
            .collect()
    }

    /// Get active orders for a symbol as legacy OrderLevels (rendering compat)
    pub(crate) fn active_order_levels(&self, symbol: &str) -> Vec<OrderLevel> {
        self.orders.iter()
            .filter(|o| o.symbol == symbol && o.state.is_active())
            .map(|o| o.to_order_level())
            .collect()
    }

    /// Set armed state
    pub(crate) fn set_armed(&mut self, armed: bool) { self.armed = armed; }
    pub(crate) fn is_armed(&self) -> bool { self.armed }

    /// Get/set risk limits
    pub(crate) fn risk_limits(&self) -> &RiskLimits { &self.risk_limits }
    pub(crate) fn set_risk_limits(&mut self, limits: RiskLimits) { self.risk_limits = limits; }

    /// Drain pending toast messages
    pub(crate) fn drain_toasts(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_toasts)
    }

    /// Stats
    pub(crate) fn stats(&self) -> (u64, u64, u64, u64) {
        (self.orders_submitted, self.orders_filled, self.orders_rejected, self.duplicates_blocked)
    }

    /// Persist active orders (Working / PendingSubmit / PartialFill) to disk.
    /// Filled / Cancelled / Rejected are history and not saved.
    pub(crate) fn save_to_disk(&self) {
        let active: Vec<&ManagedOrder> = self.orders.iter().filter(|o| matches!(
            o.state,
            OrderState::Working | OrderState::PendingSubmit | OrderState::PartialFill
        )).collect();
        match serde_json::to_vec_pretty(&active) {
            Ok(bytes) => {
                let path = orders_state_path();
                if let Err(e) = std::fs::write(&path, &bytes) {
                    eprintln!("[order_manager] save_to_disk write failed: {e}");
                }
            }
            Err(e) => eprintln!("[order_manager] save_to_disk serialize failed: {e}"),
        }
    }

    /// Load persisted orders from disk and repopulate.
    /// Restored orders are marked Working — the trader must verify with the
    /// broker since we don't know if these are still live.
    pub(crate) fn load_from_disk(&mut self) {
        let path = orders_state_path();
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return, // no file = first run
        };
        let restored: Vec<ManagedOrder> = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[order_manager] load_from_disk parse failed: {e}");
                return;
            }
        };
        let count = restored.len();
        let now = epoch_ms();
        for mut o in restored {
            o.state = OrderState::Working;
            o.updated_at = now;
            o.state_history.push((OrderState::Working, now));
            if o.id >= self.next_id { self.next_id = o.id + 1; }
            self.orders.push(o);
        }
        if count > 0 {
            self.pending_toasts.push(format!("Restored {} open orders — verify with broker", count));
        }
    }

    // ── Internal ──

    fn transition(&mut self, order_id: u64, new_state: OrderState) {
        if let Some(o) = self.orders.iter_mut().find(|o| o.id == order_id) {
            let now = epoch_ms();
            o.state = new_state;
            o.updated_at = now;
            o.state_history.push((new_state, now));
        }
    }

    fn cleanup_expired_signatures(&mut self) {
        let cutoff = std::time::Duration::from_millis(self.risk_limits.dedup_cooldown_ms * 2);
        self.recent_signatures.retain(|_, t| t.elapsed() < cutoff);
    }

    /// Garbage collect old terminal orders (keep last 500)
    pub(crate) fn gc(&mut self) {
        if self.orders.len() > 600 {
            let keep = self.orders.len() - 500;
            self.orders.retain(|o| !o.state.is_terminal() || o.updated_at > epoch_ms() - 3_600_000);
            if self.orders.len() > 500 {
                self.orders.drain(0..self.orders.len().saturating_sub(500));
            }
            let _ = keep; // suppress unused
        }
    }
}

// ─── Global API ─────────────────────────────────────────────────────────────

/// Submit an order intent through the global manager
pub(crate) fn submit_order(intent: OrderIntent) -> OrderResult {
    let r = with_mgr(|mgr| mgr.submit(intent));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Confirm a draft order
pub(crate) fn confirm_order(id: u64) -> bool {
    let r = with_mgr(|mgr| mgr.confirm(id));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Cancel an order
pub(crate) fn cancel_order(id: u64) -> bool {
    let r = with_mgr(|mgr| mgr.cancel(id));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Cancel all active orders for a symbol
pub(crate) fn cancel_all_orders(symbol: &str) {
    with_mgr(|mgr| mgr.cancel_all(symbol));
    with_mgr(|mgr| mgr.save_to_disk());
}

/// Modify order price
pub(crate) fn modify_order_price(id: u64, new_price: f32) -> bool {
    let r = with_mgr(|mgr| mgr.modify_price(id, new_price));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Flatten position
pub(crate) fn flatten_position(symbol: &str, current_qty: i32) {
    with_mgr(|mgr| mgr.flatten(symbol, current_qty));
    with_mgr(|mgr| mgr.save_to_disk());
}

/// Get active orders as legacy OrderLevels for rendering
pub(crate) fn active_orders_for(symbol: &str) -> Vec<OrderLevel> {
    with_mgr(|mgr| mgr.active_order_levels(symbol))
}

/// Set armed state
pub(crate) fn set_armed(armed: bool) {
    with_mgr(|mgr| mgr.set_armed(armed))
}

/// Check armed state
pub(crate) fn is_armed() -> bool {
    with_mgr(|mgr| mgr.is_armed())
}

/// Drain toast messages
pub(crate) fn drain_order_toasts() -> Vec<String> {
    with_mgr(|mgr| mgr.drain_toasts())
}

/// Run GC
pub(crate) fn gc_orders() {
    with_mgr(|mgr| mgr.gc())
}

/// Get all active + recently-terminal orders as legacy OrderLevels (for chart.orders sync)
pub(crate) fn all_order_levels_for(symbol: &str) -> Vec<OrderLevel> {
    with_mgr(|mgr| {
        let cutoff = epoch_ms().saturating_sub(60_000); // keep terminal orders for 60s
        mgr.orders.iter()
            .filter(|o| o.symbol == symbol && (o.state.is_active() || o.updated_at > cutoff))
            .map(|o| o.to_order_level())
            .collect()
    })
}

/// Submit and return the new order ID (convenience for write sites that need the ID)
pub(crate) fn submit_and_get_id(intent: OrderIntent) -> Option<u64> {
    match submit_order(intent) {
        OrderResult::Accepted(id) | OrderResult::NeedsConfirmation(id) => Some(id),
        _ => None,
    }
}

/// Submit a bracket order through the global manager
pub(crate) fn submit_bracket_order(intent: OrderIntent, tp: f32, sl: f32) -> (OrderResult, Option<u64>, Option<u64>) {
    let r = with_mgr(|mgr| mgr.submit_bracket(intent, tp, sl));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Submit an OCO order group through the global manager
pub(crate) fn submit_oco_order(orders: Vec<OrderIntent>) -> Vec<OrderResult> {
    let r = with_mgr(|mgr| mgr.submit_oco(orders));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Submit a conditional order through the global manager
pub(crate) fn submit_conditional_order(intent: ConditionalOrderIntent) -> OrderResult {
    let r = with_mgr(|mgr| mgr.submit_conditional(intent));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Submit an options trigger through the global manager
#[allow(clippy::too_many_arguments)]
pub(crate) fn submit_options_trigger_order(
    underlying: &str, option_type: &str, strike: f32, expiration: &str,
    qty: u32, entry_price: f32, entry_direction: &str,
    exit_price: f32, exit_direction: &str,
) -> OrderResult {
    let r = with_mgr(|mgr| mgr.submit_options_trigger(
        underlying, option_type, strike, expiration,
        qty, entry_price, entry_direction,
        exit_price, exit_direction,
    ));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Submit a combo/spread order through the global manager
pub(crate) fn submit_combo_order(symbol: &str, legs: Vec<ComboLeg>,
    side: &str, qty: u32, order_type: &str, limit_price: Option<f32>,
) -> OrderResult {
    let r = with_mgr(|mgr| mgr.submit_combo(symbol, legs, side, qty, order_type, limit_price));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Get next unique order ID without creating an order
pub(crate) fn next_order_id() -> u64 {
    with_mgr(|mgr| {
        let id = mgr.next_id;
        mgr.next_id += 1;
        id
    })
}

/// What-if margin check for a proposed order
pub(crate) fn check_margin(symbol: &str, side: &str, qty: u32, order_type: &str, price: f32) -> Option<serde_json::Value> {
    let client = reqwest::blocking::Client::new();
    let con_id = client.get(format!("{}/contract/{}", APEXIB_URL, symbol))
        .timeout(std::time::Duration::from_secs(5)).send()
        .and_then(|r| r.json::<serde_json::Value>()).ok()
        .and_then(|j| j["conId"].as_i64())?;
    client.get(format!("{}/orders/0/margin?conId={}&side={}&quantity={}&orderType={}&limitPrice={}",
        APEXIB_URL, con_id, side, qty, order_type, price))
        .timeout(std::time::Duration::from_secs(5)).send().ok()
        .and_then(|r| r.json().ok())
}

/// Kill switch — cancel everything on backend and locally
pub(crate) fn kill_switch() {
    std::thread::spawn(|| {
        let client = reqwest::blocking::Client::new();
        let _ = client.post(format!("{}/risk/kill", APEXIB_URL))
            .timeout(std::time::Duration::from_secs(10)).send();
    });
    with_mgr(|mgr| mgr.cancel_all(""));
    with_mgr(|mgr| mgr.save_to_disk());
}

/// Halt trading on the backend
pub(crate) fn halt_trading() {
    std::thread::spawn(|| {
        let client = reqwest::blocking::Client::new();
        let _ = client.post(format!("{}/risk/halt", APEXIB_URL))
            .timeout(std::time::Duration::from_secs(5)).send();
    });
}

/// Resume trading on the backend
pub(crate) fn resume_trading() {
    std::thread::spawn(|| {
        let client = reqwest::blocking::Client::new();
        let _ = client.post(format!("{}/risk/resume", APEXIB_URL))
            .timeout(std::time::Duration::from_secs(5)).send();
    });
}

/// Set paper/live mode
pub(crate) fn set_paper_mode(paper: bool) {
    with_mgr(|mgr| mgr.paper_mode = paper);
}

/// Check if in paper mode
pub(crate) fn is_paper_mode() -> bool {
    with_mgr(|mgr| mgr.paper_mode)
}

/// Get current risk limits (for settings UI)
pub(crate) fn get_risk_limits() -> RiskLimits {
    with_mgr(|mgr| mgr.risk_limits.clone())
}

/// Update risk limits (from settings UI)
pub(crate) fn update_risk_limits(limits: RiskLimits) {
    with_mgr(|mgr| mgr.risk_limits = limits);
}

/// Reconcile OrderManager state with IB backend data (called each frame with account poller data)
pub(crate) fn reconcile_with_ib(ib_orders: &[super::IbOrder]) {
    with_mgr(|mgr| reconcile_with_ib_inner(mgr, ib_orders));
    with_mgr(|mgr| mgr.save_to_disk());
}

fn reconcile_with_ib_inner(mgr: &mut OrderManager, ib_orders: &[super::IbOrder]) {
    let now = epoch_ms();

    // Collect updates first to avoid double borrow
    let mut updates: Vec<(usize, OrderState, u32, f32, Option<String>)> = Vec::new(); // (idx, state, filled_qty, avg_price, rejection)

    for ib in ib_orders {
        let ib_side_buy = ib.side.eq_ignore_ascii_case("buy") || ib.side.eq_ignore_ascii_case("bot");
        let matched_idx = mgr.orders.iter().position(|o| {
            o.symbol == ib.symbol
            && o.state.is_active()
            && ((ib_side_buy && matches!(o.side, OrderSide::Buy | OrderSide::TriggerBuy))
                || (!ib_side_buy && matches!(o.side, OrderSide::Sell | OrderSide::Stop | OrderSide::OcoStop | OrderSide::OcoTarget | OrderSide::TriggerSell)))
            && o.qty == ib.qty as u32
            && o.backend_order_id.is_none()
        });

        if let Some(idx) = matched_idx {
            // Detect partial fills explicitly: any non-empty filled_qty that's
            // less than the requested qty is a PartialFill, regardless of the
            // status string (IB sometimes reports "Submitted" while shipping
            // partial fills via separate execDetails).
            let qty_target = mgr.orders[idx].qty as i64;
            let qty_filled = ib.filled_qty as i64;
            let is_partial = qty_filled > 0 && qty_filled < qty_target;
            let (new_state, rejection) = match ib.status.as_str() {
                "filled" | "Filled" => (OrderState::Filled, None),
                "cancelled" | "Cancelled" | "ApiCancelled" => (OrderState::Cancelled, None),
                "inactive" | "Inactive" | "rejected" | "Rejected" => (OrderState::Rejected, Some(format!("IB: {}", ib.status))),
                "submitted" | "Submitted" | "PreSubmitted" => {
                    if is_partial { (OrderState::PartialFill, None) } else { (OrderState::Working, None) }
                }
                _ => continue,
            };
            updates.push((idx, new_state, ib.filled_qty as u32, ib.avg_fill_price as f32, rejection));
        }
    }

    // Apply updates
    let mut fills = 0u64;
    for (idx, new_state, filled_qty, avg_price, rejection) in updates {
        if mgr.orders[idx].state == new_state { continue; }
        let is_buy = matches!(mgr.orders[idx].side, OrderSide::Buy | OrderSide::TriggerBuy);
        let side_str = if is_buy { "BUY" } else { "SELL" };
        let sym = mgr.orders[idx].symbol.clone();
        let qty = mgr.orders[idx].qty;
        if new_state == OrderState::Filled {
            mgr.orders[idx].filled_qty = filled_qty;
            mgr.orders[idx].avg_fill_price = avg_price;
            fills += 1;
            mgr.pending_toasts.push(format!("FILLED: {} {} x{} @ {:.2}",
                side_str, sym, filled_qty, avg_price));
        }
        if new_state == OrderState::PartialFill {
            mgr.orders[idx].filled_qty = filled_qty;
            mgr.orders[idx].avg_fill_price = avg_price;
            mgr.pending_toasts.push(format!("PARTIAL: {} {} {}/{} @ {:.2}",
                side_str, sym, filled_qty, qty, avg_price));
        }
        if new_state == OrderState::Cancelled {
            mgr.pending_toasts.push(format!("CANCELLED: {} {} x{}",
                side_str, sym, qty));
        }
        if let Some(ref reason) = rejection {
            mgr.pending_toasts.push(format!("REJECTED: {} {} x{} ({})",
                side_str, sym, qty, reason));
        }
        if let Some(reason) = rejection {
            mgr.orders[idx].rejection_reason = Some(reason);
        }
        mgr.orders[idx].state = new_state;
        mgr.orders[idx].updated_at = now;
        mgr.orders[idx].state_history.push((new_state, now));
        let created = mgr.orders[idx].created_at;
        mgr.orders[idx].backend_order_id = Some(format!("ib_{}", created));
    }
    mgr.orders_filled += fills;
}

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
