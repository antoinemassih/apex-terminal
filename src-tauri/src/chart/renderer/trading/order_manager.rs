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

use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use super::{OrderSide, OrderStatus, OrderLevel, APEXIB_URL};
use super::{snapshot, inflight, paper};
use super::broker::{
    Broker, LiveBroker,
    BracketSubmitArgs, OcoSubmitArgs, OcoLeg,
    ConditionalSubmitArgs, ConditionalCondition,
    OptionsTriggerArgs,
    ComboSubmitArgs, ComboLeg as BrokerComboLeg,
};
use super::journal::{self, JournalEvent, AttemptKind, ControlKind};
use crate::foundation::types::{Price, Timestamp, TimeSource, Symbol};
use crate::foundation::types::price::price_serde;
use crate::foundation::types::timestamp::timestamp_serde;

// ─── Lock helper (panic-poison recovery) ────────────────────────────────────

fn manager() -> &'static Mutex<OrderManager> {
    ORDER_MANAGER.get_or_init(|| {
        let mut m = OrderManager::new();
        m.load_from_disk();
        // Restore kill/halt flags from sidecar — a UI crash must not silently
        // un-engage the local gate on restart.
        let path = std::env::current_exe().ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("state").join("control_flags.json");
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                m.kill_engaged = v["kill_engaged"].as_bool().unwrap_or(false);
                m.halted = v["halted"].as_bool().unwrap_or(false);
                if m.kill_engaged { eprintln!("[control] restored kill_engaged=true from disk"); }
                if m.halted { eprintln!("[control] restored halted=true from disk"); }
            }
        }
        let mgr = Mutex::new(m);
        // Wave 6a: WAL replay & broker reconciliation. Runs ONCE at startup,
        // AFTER load_from_disk and BEFORE the pending sweeper starts so we
        // don't race a recovered order into Unknown.
        // Skip when the previous run exited cleanly (Shutdown marker present).
        if journal::last_event_was_shutdown() {
            eprintln!("[recovery] previous run exited cleanly — skipping orphan recovery");
        } else {
            // Spawn the recovery work so we don't block init on HTTP. It re-acquires
            // the manager lock when it has results to apply.
            std::thread::spawn(replay_and_recover);
        }
        start_pending_sweeper();
        start_broker_watchdog();
        journal::start_wal_backup_thread();
        mgr
    })
}

/// Drop impl: emit a Shutdown marker so the next startup can skip orphan
/// recovery. Best-effort — if the process is killed mid-write we fall back
/// to the recovery path.
impl Drop for OrderManager {
    fn drop(&mut self) {
        journal::append(JournalEvent::Shutdown { ts_ms: epoch_ms() });
    }
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

/// Path for the Wave 5 versioned envelope snapshot. Written alongside
/// `orders.json` for forward-compatibility experiments. The legacy
/// reader (`load_from_disk`) still reads `orders.json` directly; this
/// snapshot is unused by the production cold-start path until a
/// follow-up wave flips the read side.
#[allow(dead_code)]
fn orders_envelope_path() -> PathBuf {
    let dir = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let state = dir.join("state");
    let _ = std::fs::create_dir_all(&state);
    state.join("orders.envelope.json")
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
    Unknown,         // Pending op timed out — broker reconcile needed
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
            Self::PendingSubmit | Self::Working | Self::PartialFill | Self::PendingModify | Self::Unknown => OrderStatus::Placed,
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
    /// Wave 3: use Price micro-units directly (i64), not "(price * 100).round()
    /// integer cents". The legacy cents bucket bucketed $145.999 and $146.001
    /// onto the same signature → legitimate orders were dropped as duplicates.
    price_micro: i64,
    qty: u32,
}

impl OrderSignature {
    fn new(symbol: &str, side: OrderSide, price: Price, qty: u32) -> Self {
        Self {
            symbol: symbol.to_uppercase(),
            side: side as u8,
            price_micro: price.0,
            qty,
        }
    }
}

// ─── Managed Order ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ManagedOrder {
    pub(crate) id: u64,
    /// Stable idempotency key sent to ApexIB on every submit/cancel/modify.
    /// Generated once at order creation; persisted; survives restarts so
    /// retried HTTP calls never duplicate broker-side.
    #[serde(default = "new_client_order_id")]
    pub(crate) client_order_id: String,
    pub(crate) symbol: String,
    /// Wave 3: typed symbol companion to `symbol`. The String stays as the
    /// authoritative serialised form so downstream code is unaffected;
    /// new sites that want the typed form can read `symbol_typed`. Skipped
    /// in serde so older `orders.json` still loads.
    #[serde(skip, default = "default_symbol")]
    pub(crate) symbol_typed: Symbol,
    pub(crate) side: OrderSide,
    pub(crate) order_type: ManagedOrderType,
    /// Wave 3: typed Price (micro-dollars). Serialised as f32 for backwards
    /// compatibility with on-disk `orders.json`.
    #[serde(with = "price_serde::as_f32")]
    pub(crate) price: Price,
    #[serde(with = "price_serde::as_f32")]
    pub(crate) stop_price: Price,     // stop trigger price
    pub(crate) qty: u32,
    pub(crate) filled_qty: u32,
    #[serde(with = "price_serde::as_f32")]
    pub(crate) avg_fill_price: Price,
    pub(crate) state: OrderState,
    pub(crate) pair_id: Option<u64>,
    // Trailing stop fields
    pub(crate) trail_amount: Option<f32>,
    pub(crate) trail_percent: Option<f32>,
    // Metadata
    pub(crate) option_symbol: Option<String>,
    pub(crate) option_con_id: Option<i64>,
    pub(crate) source: OrderSource,
    /// Wave 3: typed Timestamp. Serialised as bare u64 ms to preserve the
    /// `orders.json` wire format.
    #[serde(with = "timestamp_serde::as_u64_local")]
    pub(crate) created_at: Timestamp,
    #[serde(with = "timestamp_serde::as_u64_local")]
    pub(crate) updated_at: Timestamp,
    pub(crate) backend_order_id: Option<String>, // IB order ID once submitted
    // TIF and extended hours
    pub(crate) tif: u8,              // 0=DAY, 1=GTC, 2=IOC
    pub(crate) outside_rth: bool,    // allow trading outside regular trading hours
    // Audit
    #[serde(with = "timestamp_serde::as_pairs_u64_local")]
    pub(crate) state_history: Vec<(OrderState, Timestamp)>,
    pub(crate) rejection_reason: Option<String>,
    // ── Wave 6c: modify serialization ──
    /// Monotonic per-order modify counter. Each call to `modify_price` bumps
    /// this; the spawned PUT thread captures the value at dispatch time and
    /// only applies its result if the order's current version still matches.
    /// Persisted with `#[serde(default)]` so old `orders.json` still loads.
    #[serde(default)]
    pub(crate) modify_version: u32,
    /// True while a modify PUT is in flight to the broker. Subsequent
    /// `modify_price` calls coalesce into `modify_pending_price` instead of
    /// firing a second concurrent PUT.
    #[serde(default)]
    pub(crate) modify_inflight: bool,
    /// Latest price queued while a modify is in flight. Drained on completion.
    #[serde(default, with = "price_serde::as_f32_opt")]
    pub(crate) modify_pending_price: Option<Price>,
}

/// Default `Symbol` for `serde(skip)` ManagedOrder fields. Empty equity is
/// the safe placeholder — real call sites populate `symbol_typed` from the
/// authoritative `symbol` String during construction.
fn default_symbol() -> Symbol { Symbol::equity("") }

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

/// Generate a fresh idempotency key for a new order. UUID v4 hex.
pub(crate) fn new_client_order_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

impl ManagedOrder {
    /// Convert to legacy OrderLevel for rendering compatibility.
    /// Carries the full lifecycle `state` so the renderer can paint
    /// pending-pulse / partial-fill / unknown / ghost-filled variants.
    pub(crate) fn to_order_level(&self) -> OrderLevel {
        let filled_ratio = if self.qty > 0 {
            (self.filled_qty as f32 / self.qty as f32).clamp(0.0, 1.0)
        } else { 0.0 };
        OrderLevel {
            id: self.id as u32,
            side: self.side,
            // OrderLevel keeps f32 (it crosses into the sacred core.rs paint
            // pipeline that builds it from raw f32 click prices). Convert at
            // the boundary; ManagedOrder internally uses Price.
            price: self.price.to_f32(),
            qty: self.qty,
            status: self.state.to_legacy(),
            state: self.state,
            pair_id: self.pair_id.map(|p| p as u32),
            option_symbol: self.option_symbol.clone(),
            option_con_id: self.option_con_id,
            trail_amount: self.trail_amount,
            trail_percent: self.trail_percent,
            filled_ratio,
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
    // Wave 3 NOTE: OrderIntent's price fields stay as f32 to preserve the
    // 13+ construction sites in the sacred `core.rs` paint pipeline. Inside
    // `submit()`, intent.price is wrapped into `Price` at the manager boundary
    // and from there the typed value flows through `ManagedOrder`, dedup
    // signatures, and the broker `SubmitArgs`. The wire-format (f32) only
    // lives in the intent struct.
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
    // ── Wave 7 Task 5: strategy attribution ──
    /// Optional strategy/play identifier so the UI ledger and journal can
    /// show which automation drove a fill. Manual entry from UI sets None.
    /// Bracket/OCO legs inherit from the parent intent in the spawn helpers.
    #[allow(dead_code)] // read via journal payload + ledger panel; field still consumed across paths
    pub(crate) strategy_id: Option<String>,
    // ── Wave 7 Task 6: risk tier override ──
    /// When `true`, the manager skips the fat-finger and max-notional checks
    /// (only those two — kill, halt, position cap, oversell, dedup, qty=0
    /// remain mandatory). Default `false`. The UI MUST NEVER set this to
    /// `true` without an explicit operator confirmation dialog: this knob
    /// is the difference between "you typed the wrong price" and "I really
    /// meant 5x bigger than usual".
    #[allow(dead_code)]
    pub(crate) override_warnings: bool,
}

impl Default for OrderIntent {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            side: OrderSide::Buy,
            order_type: ManagedOrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            qty: 0,
            source: OrderSource::Api,
            pair_with: None,
            option_symbol: None,
            option_con_id: None,
            trail_amount: None,
            trail_percent: None,
            last_price: 0.0,
            tif: 0,
            outside_rth: false,
            strategy_id: None,
            override_warnings: false,
        }
    }
}

// ─── Conditional Order Intent ─────────────────────────────────────────────────

/// A price condition for conditional orders.
#[derive(Debug, Clone)]
pub(crate) struct OrderCondition {
    pub(crate) con_id: i64,       // contract to watch
    pub(crate) exchange: String,  // "SMART" default
    pub(crate) is_more: bool,     // true = trigger when >= price
    pub(crate) price: Price,
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
    /// Wave 8a: a soft-block warning gate. Fat-finger / max-notional / similar
    /// "this looks unusual" checks return this instead of a hard reject when
    /// the intent did NOT carry `override_warnings = true`. The UI is expected
    /// to surface `reason` to the operator and, on explicit confirmation,
    /// resubmit the same intent with `override_warnings = true` (which bypasses
    /// the same two gates). No managed order is created when this is returned —
    /// `intent_id` is a transient handle for telemetry, NOT an order id.
    NeedsApproval { reason: String, intent_id: u64 },
}

/// Wave 8a follow-up: a pending approval-confirmation request. UI sites that
/// receive `OrderResult::NeedsApproval` enqueue one of these (carrying the
/// original intent) and the per-frame UI loop drains the queue and renders a
/// confirmation modal. On "Override and submit", the renderer flips
/// `override_warnings = true` on the carried intent and resubmits.
#[derive(Clone, Debug)]
pub(crate) struct PendingApproval {
    pub(crate) reason: String,
    pub(crate) intent: OrderIntent,
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

/// Wave 5: versioned snapshot of the open-orders journal.
///
/// Wraps `Vec<ManagedOrder>` so we can `impl Persistable` — orphan
/// rules forbid implementing a foreign trait on `Vec<T>` directly. The
/// production cold-start path (`load_from_disk`) still reads the legacy
/// un-enveloped `orders.json`; this snapshot is written alongside as
/// `orders.envelope.json` via `save_to_disk`. A follow-up wave flips
/// the read side once the on-disk envelope has accumulated for enough
/// release cycles.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct OrdersJournalSnapshot {
    pub orders: Vec<ManagedOrder>,
}

impl crate::state::Persistable for OrdersJournalSnapshot {
    const KEY: &'static str = "orders_journal";
    const VERSION: u32 = 1;
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
    // Recent toast messages with timestamp — used to silence duplicates within
    // a short window. The reconciler can match the same logical order across
    // multiple IB endpoints (/executions, /orders?status=submitted) in a
    // single poll; without dedup the operator sees the same FILLED/CANCELLED
    // line N times. Window is 3 seconds — longer than any reasonable retry
    // burst, shorter than the operator's perception of "a new event".
    recent_toast_msgs: HashMap<String, Instant>,
    // Wave 8a follow-up: queue of soft-warning approvals awaiting operator
    // confirmation. UI sites push here on `OrderResult::NeedsApproval`; the
    // per-frame UI loop drains and renders a modal that, on "Override and
    // submit", resubmits the stashed intent with override_warnings=true.
    pending_approvals: Vec<PendingApproval>,
    // Stats
    orders_submitted: u64,
    orders_filled: u64,
    orders_rejected: u64,
    duplicates_blocked: u64,
    // ── Risk control flags (server-enforced kill / halt mirror) ──
    pub(crate) kill_engaged: bool,
    pub(crate) halted: bool,
    // ── Broker-disconnect watchdog (runtime only; not persisted) ──
    // `auto_halted_due_to_disconnect` distinguishes a halt the watchdog
    // engaged from one the user engaged, so we only auto-resume our own.
    // Note: OrderManager isn't serde-derived, so #[serde(default)] would
    // fail here. These fields default in `new()` and are not persisted.
    pub(crate) auto_halted_due_to_disconnect: bool,
    pub(crate) last_broker_contact: Option<u64>,
    // ── Reconcile startup grace ──
    // The first reconcile sweep after process start catches up to broker
    // truth — including yesterday's trades that filled while we were closed.
    // Toast emissions during that catch-up are not "new events" from the
    // operator's perspective, they're history. Silence them until first
    // sweep completes. Flips to `true` at end of `reconcile_with_ib_inner`.
    initial_reconcile_done: bool,
    // ── Wave 8a Task B: submit-rate token bucket (runtime only; not persisted) ──
    // Caps how fast a user (or a misbehaving widget) can fire submits. Cancels
    // and modifies are deliberately NOT rate-limited — those are remediations,
    // not new exposure. Defaults: 20 burst, 5/sec refill — generous enough that
    // a power user clicking quickly won't ever hit it, tight enough that a stuck
    // hotkey or a runaway loop trips at the second of sustained spam.
    submit_tokens: f32,
    submit_max_tokens: f32,
    submit_refill_per_sec: f32,
    submit_last_refill: Option<Instant>,
    // ── Wave 4: broker abstraction for multi-leg paths ──
    // All submit_bracket / submit_oco / submit_conditional / submit_options_trigger
    // / submit_combo HTTP work goes through this trait object. Defaults to
    // `LiveBroker` for production; tests inject `MockBroker` via `with_broker`.
    // Cloned (`Arc::clone`) into the background tokio/std task that performs
    // the network call so state-machine mutations on the manager itself
    // stay synchronous on the caller thread.
    pub(crate) broker: Arc<dyn Broker>,
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
            recent_toast_msgs: HashMap::new(),
            pending_approvals: Vec::new(),
            orders_submitted: 0,
            orders_filled: 0,
            orders_rejected: 0,
            duplicates_blocked: 0,
            kill_engaged: false,
            halted: false,
            auto_halted_due_to_disconnect: false,
            last_broker_contact: None,
            initial_reconcile_done: false,
            // Token bucket starts full so cold-start submits aren't penalized.
            submit_tokens: 20.0,
            submit_max_tokens: 20.0,
            submit_refill_per_sec: 5.0,
            submit_last_refill: None,
            broker: Arc::new(LiveBroker),
        }
    }

    /// Test-only constructor: build a manager wired to a caller-supplied
    /// broker (typically `MockBroker`). All other fields default — callers
    /// can mutate `paper_mode`, `armed`, etc. after construction the same
    /// way `fresh_manager()` does in the test module.
    #[cfg(test)]
    pub(crate) fn with_broker(broker: Arc<dyn Broker>) -> Self {
        let mut m = Self::new();
        m.broker = broker;
        m
    }

    /// Pull a single token from the submit bucket. Refills lazily based on
    /// wall-clock elapsed since the last attempt. Returns true if a token was
    /// consumed (caller may proceed), false if the bucket is dry (caller MUST
    /// reject). Wave 8a Task B.
    fn try_consume_submit_token(&mut self) -> bool {
        let now = Instant::now();
        if let Some(last) = self.submit_last_refill {
            let elapsed = now.duration_since(last).as_secs_f32();
            self.submit_tokens = (self.submit_tokens + elapsed * self.submit_refill_per_sec)
                .min(self.submit_max_tokens);
        }
        self.submit_last_refill = Some(now);
        if self.submit_tokens >= 1.0 {
            self.submit_tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Push a toast, skipping if the same message was pushed within the last
    /// 3 seconds. Prevents the reconciler from spamming the operator when the
    /// same logical order surfaces across multiple broker endpoints in one
    /// poll cycle (e.g., /executions AND /orders?status=submitted both
    /// referencing the same fill).
    fn push_toast_deduped(&mut self, msg: String) {
        const TOAST_DEDUP_MS: u128 = 3_000;
        // Startup catch-up: silently swallow toasts emitted while the first
        // reconcile sweep is still bringing us up to broker truth. Without
        // this, every order that was Working at shutdown but filled overnight
        // would re-toast on the next launch as if it just happened.
        if !self.initial_reconcile_done {
            return;
        }
        let now = Instant::now();
        // Opportunistic GC: drop stale entries so the map doesn't grow.
        self.recent_toast_msgs.retain(|_, t| now.duration_since(*t).as_millis() < TOAST_DEDUP_MS);
        if let Some(last) = self.recent_toast_msgs.get(&msg) {
            if now.duration_since(*last).as_millis() < TOAST_DEDUP_MS {
                return; // suppress duplicate
            }
        }
        self.recent_toast_msgs.insert(msg.clone(), now);
        self.pending_toasts.push(msg);
    }

    /// Test/dev-tool hook: override the submit rate-limit shape. `max` is the
    /// burst size (current tokens are reset to `max`); `refill` is tokens/sec.
    /// Wave 8a Task B.
    #[allow(dead_code)]
    pub(crate) fn set_submit_rate_limit(&mut self, max: f32, refill: f32) {
        self.submit_max_tokens = max;
        self.submit_refill_per_sec = refill;
        self.submit_tokens = max;
        self.submit_last_refill = None;
    }

    /// True if any submit should be refused locally (kill or halt).
    pub(crate) fn is_blocked(&self) -> bool { self.kill_engaged || self.halted }

    /// Reason string for a halt rejection — distinguishes auto-halt-due-to-
    /// disconnect from a user-engaged halt so the UI/journal carry context.
    fn halt_reason(&self) -> &'static str {
        if self.auto_halted_due_to_disconnect { "broker disconnected" } else { "trading halted" }
    }

    /// Engage the kill switch: cancel all locally + flip gate + fire broker /risk/kill.
    pub(crate) fn engage_kill(&mut self) -> Result<(), String> {
        self.cancel_all("");
        self.kill_engaged = true;
        std::thread::spawn(|| {
            let client = reqwest::blocking::Client::new();
            let _ = client.post(format!("{}/risk/kill", APEXIB_URL))
                .timeout(std::time::Duration::from_secs(10)).send();
        });
        eprintln!("[kill] ENGAGED (local + broker)");
        Ok(())
    }

    pub(crate) fn release_kill(&mut self) -> Result<(), String> {
        self.kill_engaged = false;
        eprintln!("[kill] RELEASED (local only — no broker endpoint)");
        Ok(())
    }

    pub(crate) fn halt_inner(&mut self) -> Result<(), String> {
        self.halted = true;
        std::thread::spawn(|| {
            let client = reqwest::blocking::Client::new();
            let _ = client.post(format!("{}/risk/halt", APEXIB_URL))
                .timeout(std::time::Duration::from_secs(5)).send();
        });
        eprintln!("[halt] ENGAGED (local + broker)");
        Ok(())
    }

    pub(crate) fn resume_inner(&mut self) -> Result<(), String> {
        self.halted = false;
        std::thread::spawn(|| {
            let client = reqwest::blocking::Client::new();
            let _ = client.post(format!("{}/risk/resume", APEXIB_URL))
                .timeout(std::time::Duration::from_secs(5)).send();
        });
        eprintln!("[resume] (local + broker)");
        Ok(())
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
        if self.kill_engaged { return OrderResult::Rejected("kill switch engaged".into()); }
        if self.halted { return OrderResult::Rejected(self.halt_reason().into()); }
        // Wave 8a Task B: rate-limit submits (only — cancels and modifies remain
        // unmetered since they reduce exposure). Trips BEFORE dedup so a stuck
        // hotkey that keeps re-firing the same intent gets caught at the bucket
        // rather than the dedup window (much tighter).
        if !self.try_consume_submit_token() {
            journal::append(JournalEvent::Fail {
                client_id: "rate-limit".into(),
                reason: "submit rate limit".into(),
                ts_ms: epoch_ms(),
            });
            return OrderResult::Rejected("submit rate limit exceeded".into());
        }
        let now_ms = epoch_ms();
        // Paper mode is practice — no real money at stake, so the risk limits
        // (notional, position size, fat-finger, oversell) don't apply. We still
        // enforce dedup + qty-zero + max-open-orders since those are operational
        // safety, not financial.
        let paper = self.paper_mode;

        // ── 1. Dedup check ──
        // Wave 3: dedup signature uses typed Price (micro-units, i64) so the
        // bucket distinguishes neighbouring sub-cent prices instead of
        // collapsing $145.999 and $146.001 onto the same key.
        let sig = OrderSignature::new(&intent.symbol, intent.side, Price::from_f32(intent.price), intent.qty);
        self.cleanup_expired_signatures();
        if let Some(last_time) = self.recent_signatures.get(&sig) {
            if last_time.elapsed().as_millis() < self.risk_limits.dedup_cooldown_ms as u128 {
                self.duplicates_blocked += 1;
                return OrderResult::Duplicate;
            }
        }
        self.recent_signatures.insert(sig, Instant::now());

        // ── 2. Risk validation ──
        if !paper && intent.qty > self.risk_limits.max_order_qty {
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
        if !paper && new_position.unsigned_abs() as u32 > self.risk_limits.max_position_qty {
            self.orders_rejected += 1;
            return OrderResult::Rejected(format!("Would exceed max position size {}", self.risk_limits.max_position_qty));
        }
        // In-flight aggregator: working orders in same direction count toward
        // position cap so users can't stack working orders past the limit.
        if !paper && self.risk_limits.max_position_qty > 0 {
            let working = inflight::working_qty_same_direction(&self.orders, &intent.symbol, intent.side);
            if working.saturating_add(intent.qty) > self.risk_limits.max_position_qty {
                self.orders_rejected += 1;
                return OrderResult::Rejected(format!(
                    "Working+new qty {} exceeds max position {}",
                    working.saturating_add(intent.qty), self.risk_limits.max_position_qty
                ));
            }
        }

        // ── 2.5 Oversell protection (can't sell more contracts than you hold) ──
        let is_sell = matches!(intent.side, OrderSide::Sell | OrderSide::Stop | OrderSide::OcoStop | OrderSide::TriggerSell);
        if !paper && is_sell && net_position > 0 {
            // Selling to close — can't sell more than current long position
            let sell_qty = intent.qty as i64;
            if sell_qty > net_position {
                self.orders_rejected += 1;
                return OrderResult::Rejected(format!("Can't sell {} — only holding {} contracts", intent.qty, net_position));
            }
        }
        let is_buy = matches!(intent.side, OrderSide::Buy | OrderSide::TriggerBuy);
        if !paper && is_buy && net_position < 0 {
            // Buying to close — can't buy more than current short position
            let buy_qty = intent.qty as i64;
            if buy_qty > net_position.abs() {
                self.orders_rejected += 1;
                return OrderResult::Rejected(format!("Can't buy {} — only short {} contracts", intent.qty, net_position.abs()));
            }
        }

        // ── 2.6 Fat-finger price check (ONLY on opening orders, not closing) ──
        // Wave 8a: returns NeedsApproval (soft gate) rather than Rejected when
        // override_warnings == false. The UI confirms with the operator and
        // resubmits the same intent with override_warnings=true to bypass.
        // intent_id is 0 here — the managed-order id isn't assigned until §3
        // below, and no order is created on this path anyway.
        let is_opening = (is_buy && net_position >= 0) || (is_sell && net_position <= 0);
        if !paper && !intent.override_warnings && is_opening && self.risk_limits.fat_finger_pct > 0.0
            && intent.last_price > 0.0 && intent.price > 0.0
            && intent.order_type != ManagedOrderType::Market
        {
            let deviation_pct = ((intent.price - intent.last_price) / intent.last_price * 100.0).abs();
            if deviation_pct > self.risk_limits.fat_finger_pct {
                self.orders_rejected += 1;
                return OrderResult::NeedsApproval {
                    reason: format!("fat finger: {:.1}% from last", deviation_pct),
                    intent_id: 0,
                };
            }
        }

        // ── 2.7 Max notional check (per-order + working aggregate) ──
        // Wave 8a: per-order notional cap is a soft warning gate; the working+new
        // aggregate cap remains a hard reject because once the working book is
        // already full there's nothing the user can confirm away.
        if !paper && self.risk_limits.max_notional > 0.0 {
            let order_price = if intent.price > 0.0 { intent.price } else { intent.last_price };
            let notional = order_price as f64 * intent.qty as f64;
            if notional > self.risk_limits.max_notional && !intent.override_warnings {
                self.orders_rejected += 1;
                return OrderResult::NeedsApproval {
                    reason: format!(
                        "notional ${:.0} exceeds max ${:.0}", notional, self.risk_limits.max_notional
                    ),
                    intent_id: 0,
                };
            }
            let working_notional = inflight::working_notional(&self.orders, &intent.symbol);
            if working_notional + notional > self.risk_limits.max_notional {
                self.orders_rejected += 1;
                return OrderResult::Rejected(format!(
                    "Working+new notional ${:.0} exceeds max ${:.0}",
                    working_notional + notional, self.risk_limits.max_notional
                ));
            }
        }

        // ── 2.8 Pre-submit buying-power check (Wave 6b) ──
        // Skip in paper (unlimited buying power by definition) and skip if
        // account_data not yet loaded (poller may not have fired on first
        // submit — log and proceed rather than fail-closed).
        if !paper {
            match super::read_account_data() {
                Some((summary, _positions, _ib)) if summary.connected => {
                    let inflight_notional = inflight::working_notional(&self.orders, &intent.symbol);
                    let bp = summary.buying_power;
                    let need_check = bp > 0.0;
                    if need_check {
                        if intent.order_type == ManagedOrderType::Market || intent.price <= 0.0 {
                            // Market orders: no firm price to compute notional from.
                            // Use a sanity floor — reject only when qty alone (priced
                            // at $1) would exceed 95% of buying power. This catches
                            // obvious fat-fingers without rejecting a legitimate
                            // market order whose notional we genuinely don't know.
                            // TODO: thread last-quote from chart side for a real check.
                            let qty_floor = intent.qty as f64;
                            if qty_floor >= 0.95 * bp {
                                self.orders_rejected += 1;
                                return OrderResult::Rejected(format!(
                                    "insufficient buying power for market order: qty {} >= 95% of ${:.0}",
                                    intent.qty, bp));
                            }
                        } else {
                            let candidate_notional = (intent.qty as f64) * (intent.price as f64);
                            let total = candidate_notional + inflight_notional;
                            if total > bp {
                                self.orders_rejected += 1;
                                return OrderResult::Rejected(format!(
                                    "insufficient buying power: need ${:.0}, have ${:.0}",
                                    total, bp));
                            }
                        }
                    }
                }
                _ => {
                    eprintln!("[risk] buying-power check skipped — account_data not yet loaded");
                }
            }
        }

        // Margin validation happens server-side at IB. The client-side
        // pre-check used to live here but blocked the render thread on two
        // sequential HTTP calls.

        // ── 3. Create managed order ──
        let id = self.next_id;
        self.next_id += 1;

        // Wave 8a Task 3: paper-mode UX fix — paper trading has no financial
        // risk, so the armed/Draft confirmation gate is just friction. In paper
        // mode every order goes straight to PendingSubmit regardless of `armed`.
        let initial_state = if paper || self.armed || intent.order_type == ManagedOrderType::Market {
            OrderState::PendingSubmit
        } else {
            OrderState::Draft
        };

        let order = ManagedOrder {
            id,
            client_order_id: new_client_order_id(),
            symbol: intent.symbol.clone(),
            symbol_typed: Symbol::equity(&intent.symbol),
            side: intent.side,
            order_type: intent.order_type,
            price: Price::from_f32(intent.price),
            stop_price: Price::from_f32(intent.stop_price),
            qty: intent.qty,
            filled_qty: 0,
            avg_fill_price: Price::ZERO,
            state: initial_state,
            pair_id: intent.pair_with,
            trail_amount: intent.trail_amount,
            trail_percent: intent.trail_percent,
            option_symbol: intent.option_symbol,
            option_con_id: intent.option_con_id,
            source: intent.source,
            tif: intent.tif,
            outside_rth: intent.outside_rth,
            created_at: ts_from_ms(now_ms),
            updated_at: ts_from_ms(now_ms),
            backend_order_id: None,
            state_history: vec![(initial_state, ts_from_ms(now_ms))],
            rejection_reason: None,
            modify_version: 0,
            modify_inflight: false,
            modify_pending_price: None,
        };

        self.orders.push(order);
        self.orders_submitted += 1;
        snapshot::publish(&self.orders);

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
            // OrderIntent carries f32 prices (wire-shape); submit_to_ib wants
            // the same f32 wire-shape, so pass through unchanged. The typed
            // `Price` lives on ManagedOrder.
            let price = intent.price;
            let stop_price = intent.stop_price;
            let trail_amount = intent.trail_amount;
            let trail_percent = intent.trail_percent;
            let qty = intent.qty;
            let intent_tif = intent.tif;
            let outside_rth = intent.outside_rth;
            // Use the order's stable client_order_id (UUID v4) as the idempotency
            // key. It survives retries, restarts, and double-clicks — exactly what
            // ApexIB needs to safely dedupe a re-submitted order.
            let cid = self.orders.iter().find(|o| o.id == id)
                .map(|o| o.client_order_id.clone())
                .unwrap_or_else(|| id.to_string());
            let idem_key = cid.clone();
            let order_id_copy = id;
            // Journal the attempt before dispatch so a crash is observable.
            journal::append(JournalEvent::Attempt {
                client_id: cid.clone(),
                kind: AttemptKind::Submit,
                ts_ms: now_ms,
                payload: serde_json::json!({
                    "symbol": sym, "side": side_str, "qty": qty,
                    "order_type": ot_idx, "price": price, "stop_price": stop_price,
                }),
            });
            if paper {
                // Paper-mode: short-circuit live HTTP, ack synthetically.
                let ib_oid = paper::submit_paper(&idem_key);
                if let Some(o) = self.orders.iter_mut().find(|o| o.id == id) {
                    o.backend_order_id = Some(ib_oid.clone());
                }
                journal::append(JournalEvent::Ack {
                    client_id: cid.clone(), backend_id: Some(ib_oid), ts_ms: epoch_ms(),
                });
            } else {
                // Fire async backend submission — captures IB order ID back into the manager
                let cid_thread = cid.clone();
                std::thread::spawn(move || {
                    match Self::submit_to_ib(&sym, side_str, qty, ot_idx, price, stop_price, trail_amount, trail_percent, &idem_key, intent_tif, outside_rth) {
                        Some(ib_oid) => {
                            with_mgr(|mgr| {
                                if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == order_id_copy) {
                                    o.backend_order_id = Some(ib_oid.clone());
                                }
                            });
                            journal::append(JournalEvent::Ack {
                                client_id: cid_thread, backend_id: Some(ib_oid), ts_ms: epoch_ms(),
                            });
                        }
                        None => {
                            journal::append(JournalEvent::Fail {
                                client_id: cid_thread, reason: "submit_to_ib returned None".into(),
                                ts_ms: epoch_ms(),
                            });
                        }
                    }
                });
            }
            self.transition(id, OrderState::Working); // Optimistic — will reconcile from poller
            self.pending_toasts.push(format!("{} {} x{} @ {:.2}", side_str.to_uppercase(), intent.symbol, qty, price));
            // `price` here is already f32 from the wire-boundary capture above.
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
            o.updated_at = ts_from_ms(now);
            o.state_history.push((OrderState::PendingSubmit, ts_from_ms(now)));
            o.state_history.push((OrderState::Working, ts_from_ms(now)));
            // Submit to ApexIB
            let sym = o.symbol.clone();
            let side_str = match o.side { OrderSide::Buy | OrderSide::TriggerBuy => "buy", _ => "sell" };
            let ot_idx = match o.order_type {
                ManagedOrderType::Market => 0, ManagedOrderType::Limit => 1,
                ManagedOrderType::Stop => 2, ManagedOrderType::StopLimit => 3,
                ManagedOrderType::TrailingStop => 4,
            };
            let price = o.price.to_f32(); let stop_price = o.stop_price.to_f32(); let qty = o.qty;
            let trail_amount = o.trail_amount;
            let trail_percent = o.trail_percent;
            let intent_tif = o.tif;
            let outside_rth = o.outside_rth;
            let idem_key = o.client_order_id.clone();
            let order_id_copy = order_id;
            std::thread::spawn(move || {
                if let Some(ib_oid) = Self::submit_to_ib(&sym, side_str, qty, ot_idx, price, stop_price, trail_amount, trail_percent, &idem_key, intent_tif, outside_rth) {
                    with_mgr(|mgr| {
                        if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == order_id_copy) {
                            o.backend_order_id = Some(ib_oid);
                        }
                        snapshot::publish(&mgr.orders);
                    });
                }
            });
            // Direct state mutation bypassed `transition` — publish so the
            // render-thread snapshot sees the Draft -> Working transition.
            snapshot::publish(&self.orders);
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
        if self.kill_engaged { return (OrderResult::Rejected("kill switch engaged".into()), None, None); }
        if self.halted { return (OrderResult::Rejected(self.halt_reason().into()), None, None); }
        // Wave 8a Task B: bracket is 1 logical submit (3 legs go down atomically
        // server-side), so it costs 1 token, not 3.
        if !self.try_consume_submit_token() {
            journal::append(JournalEvent::Fail {
                client_id: "rate-limit".into(),
                reason: "submit rate limit".into(),
                ts_ms: epoch_ms(),
            });
            return (OrderResult::Rejected("submit rate limit exceeded".into()), None, None);
        }
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

        // Wave 8a Task 3: paper mode skips the Draft confirmation gate.
        let initial_state = if self.paper_mode || self.armed || intent.order_type == ManagedOrderType::Market {
            OrderState::PendingSubmit
        } else {
            OrderState::Draft
        };

        let side_str = match intent.side { OrderSide::Buy | OrderSide::TriggerBuy => "buy", _ => "sell" };
        let tp_side = match intent.side { OrderSide::Buy | OrderSide::TriggerBuy => OrderSide::Sell, _ => OrderSide::Buy };

        // Entry order
        self.orders.push(ManagedOrder {
            id: entry_id, client_order_id: new_client_order_id(), symbol: intent.symbol.clone(), symbol_typed: Symbol::equity(&intent.symbol), side: intent.side,
            order_type: intent.order_type, price: Price::from_f32(intent.price), stop_price: Price::from_f32(intent.stop_price),
            qty: intent.qty, filled_qty: 0, avg_fill_price: Price::ZERO,
            state: initial_state, pair_id: None,
            trail_amount: None, trail_percent: None,
            option_symbol: intent.option_symbol.clone(), option_con_id: intent.option_con_id,
            source: intent.source, tif: intent.tif, outside_rth: intent.outside_rth,
            created_at: ts_from_ms(now_ms), updated_at: ts_from_ms(now_ms),
            backend_order_id: None, state_history: vec![(initial_state, ts_from_ms(now_ms))],
            rejection_reason: None,
            modify_version: 0, modify_inflight: false, modify_pending_price: None,
        });

        // Take profit order
        self.orders.push(ManagedOrder {
            id: tp_id, client_order_id: new_client_order_id(), symbol: intent.symbol.clone(), symbol_typed: Symbol::equity(&intent.symbol), side: tp_side,
            order_type: ManagedOrderType::Limit, price: Price::from_f32(take_profit_price), stop_price: Price::ZERO,
            qty: intent.qty, filled_qty: 0, avg_fill_price: Price::ZERO,
            state: initial_state, pair_id: Some(entry_id),
            trail_amount: None, trail_percent: None,
            option_symbol: intent.option_symbol.clone(), option_con_id: intent.option_con_id,
            source: OrderSource::Bracket, tif: intent.tif, outside_rth: intent.outside_rth,
            created_at: ts_from_ms(now_ms), updated_at: ts_from_ms(now_ms),
            backend_order_id: None, state_history: vec![(initial_state, ts_from_ms(now_ms))],
            rejection_reason: None,
            modify_version: 0, modify_inflight: false, modify_pending_price: None,
        });

        // Stop loss order
        self.orders.push(ManagedOrder {
            id: sl_id, client_order_id: new_client_order_id(), symbol: intent.symbol.clone(), symbol_typed: Symbol::equity(&intent.symbol), side: tp_side,
            order_type: ManagedOrderType::Stop, price: Price::from_f32(stop_loss_price), stop_price: Price::from_f32(stop_loss_price),
            qty: intent.qty, filled_qty: 0, avg_fill_price: Price::ZERO,
            state: initial_state, pair_id: Some(entry_id),
            trail_amount: None, trail_percent: None,
            option_symbol: intent.option_symbol.clone(), option_con_id: intent.option_con_id,
            source: OrderSource::Bracket, tif: intent.tif, outside_rth: intent.outside_rth,
            created_at: ts_from_ms(now_ms), updated_at: ts_from_ms(now_ms),
            backend_order_id: None, state_history: vec![(initial_state, ts_from_ms(now_ms))],
            rejection_reason: None,
            modify_version: 0, modify_inflight: false, modify_pending_price: None,
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
            let idem_key = self.orders.iter().find(|o| o.id == entry_id)
                .map(|o| o.client_order_id.clone())
                .unwrap_or_else(|| format!("apex_bracket_{}_{}_{}", entry_id, sym, now_ms));
            let tp_price = take_profit_price;
            let sl_price = stop_loss_price;
            let eid = entry_id; let tid = tp_id; let sid = sl_id;

            // Wave 4: HTTP work moved behind `Broker::submit_bracket`. The
            // spawned thread now calls the trait; local state mutation
            // (backend_order_id wiring on each leg) happens through `with_mgr`
            // as before so the state-machine semantics don't change.
            let broker = Arc::clone(&self.broker);
            let bargs = BracketSubmitArgs {
                symbol: sym.clone(),
                side: side_owned,
                qty,
                entry_order_type: ot_owned,
                entry_price,
                take_profit_price: tp_price,
                stop_loss_price: sl_price,
                idempotency_key: idem_key,
            };
            std::thread::spawn(move || {
                match broker.submit_bracket(&bargs) {
                    Ok(resp) => {
                        with_mgr(|mgr| {
                            if let Some(oid) = resp.parent_backend_id {
                                if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == eid) { o.backend_order_id = Some(oid); }
                            }
                            if let Some(oid) = resp.take_profit_backend_id {
                                if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == tid) { o.backend_order_id = Some(oid); }
                            }
                            if let Some(oid) = resp.stop_loss_backend_id {
                                if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == sid) { o.backend_order_id = Some(oid); }
                            }
                        });
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
        if self.kill_engaged { return vec![OrderResult::Rejected("kill switch engaged".into())]; }
        if self.halted { return vec![OrderResult::Rejected(self.halt_reason().into())]; }
        // Wave 8a Task B: OCO is one user gesture → one token.
        if !self.try_consume_submit_token() {
            journal::append(JournalEvent::Fail {
                client_id: "rate-limit".into(),
                reason: "submit rate limit".into(),
                ts_ms: epoch_ms(),
            });
            return vec![OrderResult::Rejected("submit rate limit exceeded".into())];
        }
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
                id, client_order_id: new_client_order_id(), symbol: intent.symbol.clone(), symbol_typed: Symbol::equity(&intent.symbol), side: intent.side,
                order_type: intent.order_type, price: Price::from_f32(intent.price), stop_price: Price::from_f32(intent.stop_price),
                qty: intent.qty, filled_qty: 0, avg_fill_price: Price::ZERO,
                state: initial_state, pair_id: None, // pair_ids linked after all created
                trail_amount: intent.trail_amount, trail_percent: intent.trail_percent,
                option_symbol: intent.option_symbol.clone(), option_con_id: intent.option_con_id,
                source: OrderSource::Oco, tif: intent.tif, outside_rth: intent.outside_rth,
                created_at: ts_from_ms(now_ms), updated_at: ts_from_ms(now_ms),
                backend_order_id: None, state_history: vec![(initial_state, ts_from_ms(now_ms))],
                rejection_reason: None,
                modify_version: 0, modify_inflight: false, modify_pending_price: None,
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
            // Pair-id mutation bypasses `transition`; publish so the
            // render-thread snapshot reflects the linked legs.
            snapshot::publish(&self.orders);
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

            // Wave 4: HTTP body construction moves into `LiveBroker::submit_oco`.
            // The manager just maps its (symbol, side, qty, ot, price, stop_price)
            // tuples into typed `OcoLeg`s and hands them off.
            let broker = Arc::clone(&self.broker);
            let legs: Vec<OcoLeg> = order_intents.iter().map(|(sym, side, qty, ot, price, stop_price)| {
                OcoLeg {
                    symbol: sym.clone(), side: side.clone(), qty: *qty,
                    order_type: ot.clone(), price: *price, stop_price: *stop_price,
                }
            }).collect();
            let oargs = OcoSubmitArgs { legs, oca_group: oca.clone() };
            std::thread::spawn(move || {
                match broker.submit_oco(&oargs) {
                    Ok(resp) => {
                        with_mgr(|mgr| {
                            for (i, oid) in resp.leg_backend_ids.iter().enumerate() {
                                if i < ids_copy.len() {
                                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == ids_copy[i]) {
                                        o.backend_order_id = Some(oid.clone());
                                    }
                                }
                            }
                        });
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
        if self.kill_engaged { return OrderResult::Rejected("kill switch engaged".into()); }
        if self.halted { return OrderResult::Rejected(self.halt_reason().into()); }
        // Wave 8a Task B: rate-limit before any work.
        if !self.try_consume_submit_token() {
            journal::append(JournalEvent::Fail {
                client_id: "rate-limit".into(),
                reason: "submit rate limit".into(),
                ts_ms: epoch_ms(),
            });
            return OrderResult::Rejected("submit rate limit exceeded".into());
        }
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
            id, client_order_id: new_client_order_id(), symbol: base.symbol.clone(), symbol_typed: Symbol::equity(&base.symbol), side: base.side,
            order_type: base.order_type, price: Price::from_f32(base.price), stop_price: Price::from_f32(base.stop_price),
            qty: base.qty, filled_qty: 0, avg_fill_price: Price::ZERO,
            state: initial_state, pair_id: base.pair_with,
            trail_amount: base.trail_amount, trail_percent: base.trail_percent,
            option_symbol: base.option_symbol.clone(), option_con_id: base.option_con_id,
            source: OrderSource::Conditional, tif: base.tif, outside_rth: base.outside_rth,
            created_at: ts_from_ms(now_ms), updated_at: ts_from_ms(now_ms),
            backend_order_id: None, state_history: vec![(initial_state, ts_from_ms(now_ms))],
            rejection_reason: None,
            modify_version: 0, modify_inflight: false, modify_pending_price: None,
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
        let idem_key = self.orders.iter().find(|o| o.id == id)
            .map(|o| o.client_order_id.clone())
            .unwrap_or_else(|| format!("apex_cond_{}_{}_{}", id, sym, now_ms));
        let id_copy = id;

        // Build toast before move
        let toast = format!("CONDITIONAL {} {} x{} with {} conditions",
            side_str, sym, qty, conditions.len());

        // Wave 4: HTTP body construction moves into `LiveBroker::submit_conditional`.
        let broker = Arc::clone(&self.broker);
        let cargs = ConditionalSubmitArgs {
            symbol: sym.clone(),
            side: side_str,
            qty,
            order_type: ot,
            price,
            conditions: conditions.iter().map(|c| ConditionalCondition {
                con_id: c.con_id, exchange: c.exchange.clone(),
                is_more: c.is_more, price: c.price,
            }).collect(),
            conditions_logic: logic,
            conditions_cancel_order: cancel_order,
            idempotency_key: idem_key,
        };
        std::thread::spawn(move || {
            match broker.submit_conditional(&cargs) {
                Ok(oid) => {
                    with_mgr(|mgr| {
                        if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == id_copy) {
                            o.backend_order_id = Some(oid);
                        }
                    });
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
        if self.kill_engaged { return OrderResult::Rejected("kill switch engaged".into()); }
        if self.halted { return OrderResult::Rejected(self.halt_reason().into()); }
        // Wave 8a Task B: rate-limit before any work.
        if !self.try_consume_submit_token() {
            journal::append(JournalEvent::Fail {
                client_id: "rate-limit".into(),
                reason: "submit rate limit".into(),
                ts_ms: epoch_ms(),
            });
            return OrderResult::Rejected("submit rate limit exceeded".into());
        }
        let now_ms = epoch_ms();
        if qty == 0 {
            return OrderResult::Rejected("Qty cannot be zero".into());
        }

        let id = self.next_id; self.next_id += 1;
        let initial_state = OrderState::PendingSubmit;

        self.orders.push(ManagedOrder {
            id, client_order_id: new_client_order_id(), symbol: underlying.to_string(), symbol_typed: Symbol::equity(underlying), side: OrderSide::TriggerBuy,
            order_type: ManagedOrderType::Market, price: Price::from_f32(entry_price), stop_price: Price::ZERO,
            qty, filled_qty: 0, avg_fill_price: Price::ZERO,
            state: initial_state, pair_id: None,
            trail_amount: None, trail_percent: None,
            option_symbol: Some(format!("{} {}{} {}", underlying, strike, option_type, expiration)),
            option_con_id: None,
            source: OrderSource::OptionsTrigger, tif: 0, outside_rth: false,
            created_at: ts_from_ms(now_ms), updated_at: ts_from_ms(now_ms),
            backend_order_id: None, state_history: vec![(initial_state, ts_from_ms(now_ms))],
            rejection_reason: None,
            modify_version: 0, modify_inflight: false, modify_pending_price: None,
        });
        self.orders_submitted += 1;

        let und = underlying.to_string();
        let ot = option_type.to_string();
        let exp = expiration.to_string();
        let entry_dir = entry_direction.to_string();
        let exit_dir = exit_direction.to_string();
        let idem_key = self.orders.iter().find(|o| o.id == id)
            .map(|o| o.client_order_id.clone())
            .unwrap_or_else(|| format!("apex_opttrig_{}_{}_{}", id, underlying, now_ms));
        let id_copy = id;

        // Wave 4: HTTP body moves into `LiveBroker::submit_options_trigger`.
        let broker = Arc::clone(&self.broker);
        let oargs = OptionsTriggerArgs {
            underlying: und, option_type: ot, strike, expiration: exp,
            qty, entry_price, entry_direction: entry_dir,
            exit_price, exit_direction: exit_dir,
            idempotency_key: idem_key,
        };
        std::thread::spawn(move || {
            match broker.submit_options_trigger(&oargs) {
                Ok(resp) => {
                    with_mgr(|mgr| {
                        if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == id_copy) {
                            if let Some(oid) = resp.entry_backend_id { o.backend_order_id = Some(oid); }
                            if let Some(cid) = resp.option_con_id { o.option_con_id = Some(cid); }
                        }
                    });
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
        if self.kill_engaged { return OrderResult::Rejected("kill switch engaged".into()); }
        if self.halted { return OrderResult::Rejected(self.halt_reason().into()); }
        // Wave 8a Task B: rate-limit before any work.
        if !self.try_consume_submit_token() {
            journal::append(JournalEvent::Fail {
                client_id: "rate-limit".into(),
                reason: "submit rate limit".into(),
                ts_ms: epoch_ms(),
            });
            return OrderResult::Rejected("submit rate limit exceeded".into());
        }
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
            id, client_order_id: new_client_order_id(), symbol: symbol.to_string(), symbol_typed: Symbol::equity(symbol), side: order_side,
            order_type: managed_ot, price: limit_price.map(Price::from_f32).unwrap_or(Price::ZERO), stop_price: Price::ZERO,
            qty, filled_qty: 0, avg_fill_price: Price::ZERO,
            state: initial_state, pair_id: None,
            trail_amount: None, trail_percent: None,
            option_symbol: Some(format!("{} combo {}leg", symbol, legs.len())),
            option_con_id: None,
            source: OrderSource::Combo, tif: 0, outside_rth: false,
            created_at: ts_from_ms(now_ms), updated_at: ts_from_ms(now_ms),
            backend_order_id: None, state_history: vec![(initial_state, ts_from_ms(now_ms))],
            rejection_reason: None,
            modify_version: 0, modify_inflight: false, modify_pending_price: None,
        });
        self.orders_submitted += 1;

        let sym = symbol.to_string();
        let side_owned = side.to_string();
        let ot_owned = order_type.to_string();
        let idem_key = self.orders.iter().find(|o| o.id == id)
            .map(|o| o.client_order_id.clone())
            .unwrap_or_else(|| format!("apex_combo_{}_{}_{}", id, symbol, now_ms));
        let id_copy = id;
        let num_legs = legs.len();

        // Build toast before move
        let toast = format!("COMBO {} {} x{} ({} legs)", side.to_uppercase(), symbol, qty, num_legs);

        // Wave 4: combo HTTP shape (legs in body, params in query) moves into
        // `LiveBroker::submit_combo`. Manager maps its `ComboLeg` into the
        // broker-side equivalent (same fields, different module).
        let broker = Arc::clone(&self.broker);
        let cargs = ComboSubmitArgs {
            symbol: sym, side: side_owned, qty,
            order_type: ot_owned, limit_price,
            legs: legs.iter().map(|l| BrokerComboLeg {
                con_id: l.con_id, ratio: l.ratio, side: l.side.clone(),
            }).collect(),
            idempotency_key: idem_key,
        };
        std::thread::spawn(move || {
            match broker.submit_combo(&cargs) {
                Ok(oid) => {
                    with_mgr(|mgr| {
                        if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == id_copy) {
                            o.backend_order_id = Some(oid);
                        }
                    });
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

        let paper = self.paper_mode;
        let cid = order_id.to_string();
        journal::append(JournalEvent::Attempt {
            client_id: cid.clone(), kind: AttemptKind::Cancel,
            ts_ms: epoch_ms(), payload: serde_json::json!({"order_id": order_id}),
        });

        self.transition(order_id, OrderState::Cancelled);

        // Send individual cancel to ApexIB
        if let Some(backend_id) = self.orders.iter().find(|o| o.id == order_id).and_then(|o| o.backend_order_id.clone()) {
            if paper {
                paper::cancel_paper(&backend_id);
                journal::append(JournalEvent::Ack { client_id: cid.clone(), backend_id: Some(backend_id), ts_ms: epoch_ms() });
            } else {
                let cid_thread = cid.clone();
                let bid = backend_id.clone();
                std::thread::spawn(move || {
                    let client = reqwest::blocking::Client::new();
                    let res = client.delete(format!("{}/orders/{}", APEXIB_URL, bid))
                        .timeout(std::time::Duration::from_secs(5)).send();
                    match res {
                        Ok(_) => journal::append(JournalEvent::Ack { client_id: cid_thread, backend_id: Some(bid), ts_ms: epoch_ms() }),
                        Err(e) => journal::append(JournalEvent::Fail { client_id: cid_thread, reason: format!("cancel http: {e}"), ts_ms: epoch_ms() }),
                    }
                });
            }
        } else {
            journal::append(JournalEvent::Ack { client_id: cid.clone(), backend_id: None, ts_ms: epoch_ms() });
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
        let cid = format!("cancel-all-{}", epoch_ms());
        journal::append(JournalEvent::Attempt {
            client_id: cid.clone(), kind: AttemptKind::CancelAll,
            ts_ms: epoch_ms(), payload: serde_json::json!({"symbol": symbol}),
        });
        let paper = self.paper_mode;
        // Send cancel-all to IB backend (skip in paper)
        if !paper {
            let cid_thread = cid.clone();
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                let res = client.delete(format!("{}/orders", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
                match res {
                    Ok(_) => journal::append(JournalEvent::Ack { client_id: cid_thread, backend_id: None, ts_ms: epoch_ms() }),
                    Err(e) => journal::append(JournalEvent::Fail { client_id: cid_thread, reason: format!("cancel_all http: {e}"), ts_ms: epoch_ms() }),
                }
            });
        } else {
            journal::append(JournalEvent::Ack { client_id: cid, backend_id: None, ts_ms: epoch_ms() });
        }
        let ids: Vec<u64> = self.orders.iter()
            .filter(|o| o.state.is_active() && (symbol.is_empty() || o.symbol == symbol))
            .map(|o| o.id)
            .collect();
        for id in ids {
            self.cancel(id);
        }
    }

    /// Modify an order's price (drag-to-move, etc.)
    ///
    /// Wave 6c: serialised — at most one PUT in flight per order. While
    /// `modify_inflight`, subsequent calls coalesce into `modify_pending_price`
    /// and fire after the in-flight one completes. Each PUT carries a
    /// monotonic `modify_version` so out-of-order responses are discarded.
    pub(crate) fn modify_price(&mut self, order_id: u64, new_price: Price) -> bool {
        let paper = self.paper_mode;
        let Some(o) = self.orders.iter_mut().find(|o| o.id == order_id && o.state.is_active()) else {
            return false;
        };
        // If a PUT is already in flight, queue this price and return.
        if o.modify_inflight {
            o.modify_pending_price = Some(new_price);
            return true;
        }
        let now = epoch_ms();
        o.price = new_price;
        o.updated_at = ts_from_ms(now);
        o.state_history.push((OrderState::PendingModify, ts_from_ms(now)));
        o.modify_inflight = true;
        o.modify_version = o.modify_version.wrapping_add(1);
        let version = o.modify_version;
        let cid = o.client_order_id.clone();
        let backend_id = o.backend_order_id.clone();
        let is_stop = matches!(o.order_type, ManagedOrderType::Stop);
        let is_stop_limit = matches!(o.order_type, ManagedOrderType::StopLimit);

        // Wire-boundary conversion — the broker JSON shape is unchanged.
        let new_price_f = new_price.to_f32();

        journal::append(JournalEvent::Attempt {
            client_id: cid.clone(), kind: AttemptKind::Modify,
            ts_ms: now, payload: serde_json::json!({
                "order_id": order_id, "new_price": new_price_f, "modify_version": version,
            }),
        });

        match backend_id {
            Some(bid) => {
                if paper {
                    paper::modify_paper(&bid, new_price_f);
                    journal::append(JournalEvent::Ack { client_id: cid, backend_id: Some(bid), ts_ms: epoch_ms() });
                    // Apply inline for paper — we already hold &mut self so we
                    // can't recurse through `with_mgr` (would deadlock the lock).
                    self.apply_modify_result_inner(order_id, version);
                } else {
                    let cid_thread = cid.clone();
                    let bid_thread = bid.clone();
                    let np = new_price_f;
                    std::thread::spawn(move || {
                        let client = reqwest::blocking::Client::new();
                        let body = if is_stop {
                            serde_json::json!({"stopPrice": np, "modifyVersion": version})
                        } else if is_stop_limit {
                            serde_json::json!({"limitPrice": np, "stopPrice": np, "modifyVersion": version})
                        } else {
                            serde_json::json!({"limitPrice": np, "modifyVersion": version})
                        };
                        let res = client.put(format!("{}/orders/{}", APEXIB_URL, bid_thread))
                            .json(&body)
                            .timeout(std::time::Duration::from_secs(5)).send();
                        match res {
                            Ok(_) => journal::append(JournalEvent::Ack {
                                client_id: cid_thread, backend_id: Some(bid), ts_ms: epoch_ms(),
                            }),
                            Err(e) => journal::append(JournalEvent::Fail {
                                client_id: cid_thread, reason: format!("modify http: {e}"), ts_ms: epoch_ms(),
                            }),
                        }
                        // Spawned thread does not hold the manager lock, so it
                        // can re-enter via `with_mgr` safely.
                        let pending: Option<Price> = with_mgr(|mgr| {
                            let o = mgr.orders.iter_mut().find(|o| o.id == order_id)?;
                            if o.modify_version != version {
                                return None; // superseded
                            }
                            o.modify_inflight = false;
                            o.modify_pending_price.take()
                        });
                        if let Some(p) = pending {
                            with_mgr(|mgr| { mgr.modify_price(order_id, p); });
                        }
                    });
                }
            }
            None => {
                journal::append(JournalEvent::Ack { client_id: cid, backend_id: None, ts_ms: epoch_ms() });
                self.apply_modify_result_inner(order_id, version);
            }
        }
        true
    }

    /// Inline (lock-already-held) variant of the modify completion handler.
    /// Used for paper / no-backend paths where `modify_price` hasn't released
    /// the manager lock yet.
    fn apply_modify_result_inner(&mut self, order_id: u64, version: u32) {
        let pending: Option<Price> = {
            let Some(o) = self.orders.iter_mut().find(|o| o.id == order_id) else { return; };
            if o.modify_version != version {
                return; // superseded
            }
            o.modify_inflight = false;
            o.modify_pending_price.take()
        };
        if let Some(p) = pending {
            // Reentrant — but on the same &mut self, so safe.
            self.modify_price(order_id, p);
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
                strategy_id: None,
                override_warnings: false,
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
        // Wave 5: also write a versioned envelope alongside the legacy
        // `orders.json`. The cold-start path still reads the legacy file
        // today; the envelope is being shadow-written so a future wave
        // can flip the reader without a flag day.
        let snapshot = OrdersJournalSnapshot {
            orders: active.into_iter().cloned().collect(),
        };
        if let Err(e) = crate::state::save(&orders_envelope_path(), &snapshot) {
            eprintln!("[order_manager] save envelope failed: {e}");
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
            o.updated_at = ts_from_ms(now);
            o.state_history.push((OrderState::Working, ts_from_ms(now)));
            if o.id >= self.next_id { self.next_id = o.id + 1; }
            self.orders.push(o);
        }
        if count > 0 {
            self.pending_toasts.push(format!("Restored {} open orders — verify with broker", count));
        }
        snapshot::publish(&self.orders);
    }

    // ── Internal ──

    fn transition(&mut self, order_id: u64, new_state: OrderState) {
        let mut chg: Option<(String, OrderState, OrderState, u64)> = None;
        if let Some(o) = self.orders.iter_mut().find(|o| o.id == order_id) {
            let now = epoch_ms();
            let from = o.state;
            o.state = new_state;
            o.updated_at = ts_from_ms(now);
            o.state_history.push((new_state, ts_from_ms(now)));
            chg = Some((o.id.to_string(), from, new_state, now));
        }
        snapshot::publish(&self.orders);
        if let Some((cid, from, to, ts)) = chg {
            journal::append(JournalEvent::StateChg { client_id: cid, from, to, ts_ms: ts });
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
            self.orders.retain(|o| !o.state.is_terminal() || (o.updated_at.millis() as u64) > epoch_ms() - 3_600_000);
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
    // External callers (core.rs render path) pass f32; convert to Price at
    // the boundary so the internal API stays typed.
    let r = with_mgr(|mgr| mgr.modify_price(id, Price::from_f32(new_price)));
    with_mgr(|mgr| mgr.save_to_disk());
    r
}

/// Flatten position
pub(crate) fn flatten_position(symbol: &str, current_qty: i32) {
    with_mgr(|mgr| mgr.flatten(symbol, current_qty));
    with_mgr(|mgr| mgr.save_to_disk());
}

/// Get active orders as legacy OrderLevels for rendering.
///
/// Render-thread read. Lock-free via the published `OrdersSnapshot` —
/// **does not** acquire the heavy `ORDER_MANAGER` mutex. Mutations propagate
/// via `snapshot::publish(&self.orders)` after every state change inside the
/// manager (see `transition`, `submit`, `load_from_disk`,
/// `reconcile_with_ib_inner`). Per-frame, per-pane render reads now scale
/// with pane count without contending on the writer mutex.
pub(crate) fn active_orders_for(symbol: &str) -> Vec<OrderLevel> {
    let snap = snapshot::current();
    snap.orders.iter()
        .filter(|o| o.symbol == symbol && o.state.is_active())
        .map(|o| o.to_order_level())
        .collect()
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

/// Wave 8a follow-up: enqueue a `NeedsApproval` warning for the operator to
/// confirm or cancel via the per-frame approval modal. UI sites that match on
/// `OrderResult::NeedsApproval { reason, .. }` should call this with the
/// ORIGINAL intent (not a fresh one) so the resubmit path preserves every
/// field — strategy_id, source, tif, last_price, the lot.
pub(crate) fn enqueue_approval(reason: String, intent: OrderIntent) {
    with_mgr(|m| m.pending_approvals.push(PendingApproval { reason, intent }));
}

/// Drain pending approval requests for the per-frame UI loop. Each returned
/// entry should be rendered as a confirmation dialog; on "Override and submit"
/// the caller flips `intent.override_warnings = true` and resubmits via
/// `submit_order`. Wave 8a follow-up.
pub(crate) fn drain_approvals() -> Vec<PendingApproval> {
    with_mgr(|m| m.pending_approvals.drain(..).collect())
}

/// Run GC
pub(crate) fn gc_orders() {
    with_mgr(|mgr| mgr.gc())
}

/// Get all active + recently-terminal orders as legacy OrderLevels
/// (for chart.orders sync).
///
/// Render-thread read. Lock-free via the published `OrdersSnapshot` — see
/// `active_orders_for` for the freshness contract. Each pane calls this
/// once per frame, so this is on the multi-pane hot path.
pub(crate) fn all_order_levels_for(symbol: &str) -> Vec<OrderLevel> {
    let cutoff = epoch_ms().saturating_sub(60_000); // keep terminal orders for 60s
    let snap = snapshot::current();
    snap.orders.iter()
        .filter(|o| o.symbol == symbol && (o.state.is_active() || (o.updated_at.millis() as u64) > cutoff))
        .map(|o| o.to_order_level())
        .collect()
}

/// Submit and return the new order ID (convenience for write sites that need the ID)
pub(crate) fn submit_and_get_id(intent: OrderIntent) -> Option<u64> {
    let side = intent.side;
    let price = intent.price;
    let qty = intent.qty;
    match submit_order(intent) {
        OrderResult::Accepted(id) | OrderResult::NeedsConfirmation(id) => Some(id),
        OrderResult::Duplicate => {
            eprintln!("[order-manager] Duplicate (silent): side={:?} price={:.2} qty={} (within {}ms cooldown)",
                side, price, qty, with_mgr(|m| m.risk_limits.dedup_cooldown_ms));
            None
        }
        OrderResult::Rejected(reason) => {
            eprintln!("[order-manager] Rejected: side={:?} price={:.2} qty={}: {}",
                side, price, qty, reason);
            None
        }
        OrderResult::NeedsApproval { reason, .. } => {
            // No order id is created on the soft-warning path — callers that
            // need to surface the warning to the operator must use
            // `submit_order` directly and inspect the result.
            eprintln!("[order-manager] NeedsApproval (silent at this entry point): side={:?} price={:.2} qty={}: {}",
                side, price, qty, reason);
            None
        }
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

/// Kill switch — engage local gate, cancel all locally, fire broker /risk/kill.
/// State persists across restart so a UI crash cannot un-engage silently.
pub(crate) fn kill_switch() {
    let _ = with_mgr(|mgr| mgr.engage_kill());
    with_mgr(|mgr| mgr.save_to_disk());
    save_control_flags();
}

pub(crate) fn engage_kill_switch() -> Result<(), String> {
    let r = with_mgr(|mgr| mgr.engage_kill());
    with_mgr(|mgr| mgr.save_to_disk());
    save_control_flags();
    r
}

pub(crate) fn release_kill_switch() -> Result<(), String> {
    let r = with_mgr(|mgr| mgr.release_kill());
    save_control_flags();
    r
}

/// Halt trading — local gate + broker /risk/halt.
pub(crate) fn halt_trading() -> Result<(), String> {
    let r = with_mgr(|mgr| mgr.halt_inner());
    save_control_flags();
    r
}

/// Resume trading — local gate + broker /risk/resume.
pub(crate) fn resume_trading() -> Result<(), String> {
    let r = with_mgr(|mgr| mgr.resume_inner());
    save_control_flags();
    r
}

/// Defense-in-depth: are submits currently blocked by kill or halt?
pub(crate) fn is_trading_blocked() -> bool {
    with_mgr(|mgr| mgr.is_blocked())
}

fn control_flags_path() -> std::path::PathBuf {
    let dir = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let state = dir.join("state");
    let _ = std::fs::create_dir_all(&state);
    state.join("control_flags.json")
}

fn save_control_flags() {
    let (k, h) = with_mgr(|m| (m.kill_engaged, m.halted));
    let v = serde_json::json!({ "kill_engaged": k, "halted": h });
    if let Err(e) = std::fs::write(control_flags_path(), v.to_string()) {
        eprintln!("[control] save_control_flags failed: {e}");
    }
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

/// Per-order broker-visibility tracker used by Rule 3.
/// Maps local order id -> last epoch_ms a poll observed the order at the broker.
/// Lives outside `ManagedOrder` so we don't churn the persisted struct schema.
fn last_seen() -> &'static Mutex<HashMap<u64, u64>> {
    static SEEN: OnceLock<Mutex<HashMap<u64, u64>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Map an IB status string to a local OrderState.
/// Returns None if the status is unrecognized (so the caller skips it).
/// `is_partial` lets the caller upgrade Working → PartialFill when filled_qty < qty.
fn map_ib_status(status: &str, is_partial: bool) -> Option<(OrderState, Option<String>)> {
    match status {
        "filled" | "Filled" => Some((OrderState::Filled, None)),
        "cancelled" | "Cancelled" | "ApiCancelled" => Some((OrderState::Cancelled, None)),
        "inactive" | "Inactive" | "rejected" | "Rejected" =>
            Some((OrderState::Rejected, Some(format!("IB: {}", status)))),
        "submitted" | "Submitted" | "PreSubmitted" | "working" | "Working" => {
            if is_partial { Some((OrderState::PartialFill, None)) } else { Some((OrderState::Working, None)) }
        }
        _ => None,
    }
}

/// Match a broker-reported order to a local managed order index.
/// Rule 1: prefer matching on backend_order_id; fall back to (symbol, side, qty).
fn find_local_match(orders: &[ManagedOrder], ib: &super::IbOrder) -> Option<usize> {
    // Rule 1a: id-based match wins if both sides have an id.
    if !ib.backend_id.is_empty() {
        if let Some(i) = orders.iter().position(|o| {
            o.backend_order_id.as_deref() == Some(ib.backend_id.as_str())
        }) {
            return Some(i);
        }
    }
    // Rule 1b: side-aware (symbol, side, qty) match. Prefer active orders to
    // avoid mis-binding to a stale terminal twin.
    let ib_side_buy = ib.side.eq_ignore_ascii_case("buy") || ib.side.eq_ignore_ascii_case("bot");
    let pred = |o: &ManagedOrder| -> bool {
        o.symbol.eq_ignore_ascii_case(&ib.symbol)
        && o.qty == ib.qty as u32
        && ((ib_side_buy && matches!(o.side, OrderSide::Buy | OrderSide::TriggerBuy))
            || (!ib_side_buy && matches!(o.side,
                OrderSide::Sell | OrderSide::Stop | OrderSide::OcoStop | OrderSide::OcoTarget | OrderSide::TriggerSell)))
    };
    if let Some(i) = orders.iter().position(|o| pred(o) && o.state.is_active()) {
        return Some(i);
    }
    // Rule 4 path: local Cancelled but broker still Working — match terminal too.
    orders.iter().position(pred)
}

fn reconcile_with_ib_inner(mgr: &mut OrderManager, ib_orders: &[super::IbOrder]) {
    let now = epoch_ms();

    // Track which local orders the broker still references this poll.
    let mut seen_local_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();
    // Re-fire-cancel queue for Rule 4.
    let mut to_recancel: Vec<u64> = Vec::new();

    let mut fills = 0u64;

    for ib in ib_orders {
        let matched_idx = find_local_match(&mgr.orders, ib);

        // ── Rule 5: broker has an order we don't know about → adopt as External.
        let idx = match matched_idx {
            Some(i) => i,
            None => {
                // Only adopt when the broker reports it as still actionable
                // or recently active. Skip pure historical executions that
                // arrive as "filled" with no other context (avoid spam after
                // restart re-pulling 24h of history).
                let qty_target = ib.qty.max(1) as i64;
                let qty_filled = ib.filled_qty as i64;
                let is_partial = qty_filled > 0 && qty_filled < qty_target;
                let Some((state, rejection)) = map_ib_status(&ib.status, is_partial) else { continue; };

                // Skip terminal historical rows on first sight (would just
                // pollute the ledger with `Filled` adoptions for orders the
                // user placed yesterday on another machine).
                if state.is_terminal() && ib.submitted_at > 0 && (now as i64 - ib.submitted_at) > 60_000 {
                    continue;
                }

                let id = mgr.next_id; mgr.next_id += 1;
                let side = if ib.side.eq_ignore_ascii_case("buy") || ib.side.eq_ignore_ascii_case("bot") {
                    OrderSide::Buy
                } else {
                    OrderSide::Sell
                };
                let backend_id = if ib.backend_id.is_empty() {
                    format!("ext_{}_{}", ib.symbol, ib.submitted_at.max(now as i64))
                } else {
                    ib.backend_id.clone()
                };
                let adopted = ManagedOrder {
                    id,
                    client_order_id: format!("ext-{}", backend_id),
                    symbol: ib.symbol.clone(),
                    symbol_typed: Symbol::equity(&ib.symbol),
                    side,
                    order_type: ManagedOrderType::Limit,
                    price: Price::from_dollars(ib.limit_price),
                    stop_price: Price::ZERO,
                    qty: ib.qty.max(0) as u32,
                    filled_qty: ib.filled_qty.max(0) as u32,
                    avg_fill_price: Price::from_dollars(ib.avg_fill_price),
                    state,
                    pair_id: None,
                    trail_amount: None,
                    trail_percent: None,
                    option_symbol: None,
                    option_con_id: None,
                    source: OrderSource::Api,
                    created_at: ts_from_ms(ib.submitted_at.max(0) as u64),
                    updated_at: ts_from_ms(now),
                    backend_order_id: Some(backend_id),
                    tif: 0,
                    outside_rth: false,
                    state_history: vec![(state, ts_from_ms(now))],
                    rejection_reason: rejection,
                    modify_version: 0,
                    modify_inflight: false,
                    modify_pending_price: None,
                };
                let new_idx = mgr.orders.len();
                mgr.orders.push(adopted);
                eprintln!("[reconcile] Rule 5: adopted external order id={} {} {} x{} state={:?}",
                    id, mgr.orders[new_idx].symbol,
                    if matches!(mgr.orders[new_idx].side, OrderSide::Buy) {"BUY"} else {"SELL"},
                    mgr.orders[new_idx].qty, state);
                journal::append(JournalEvent::Reconcile {
                    client_id: id.to_string(),
                    local: state,
                    broker: ib.status.clone(),
                    resolution: format!("Rule 5: adopt external (backend_id={})", mgr.orders[new_idx].backend_order_id.as_deref().unwrap_or("?")),
                    ts_ms: now,
                });
                new_idx
            }
        };

        let local_id = mgr.orders[idx].id;
        seen_local_ids.insert(local_id);

        // Snapshot fields we'll need.
        let prev_state = mgr.orders[idx].state;
        let local_filled = mgr.orders[idx].filled_qty;
        let qty_target = mgr.orders[idx].qty as i64;
        let qty_filled_broker = ib.filled_qty as i64;
        let is_partial = qty_filled_broker > 0 && qty_filled_broker < qty_target;
        let Some((broker_state, rejection)) = map_ib_status(&ib.status, is_partial) else { continue; };

        // Stash backend_order_id if the broker now gives us one and we hadn't
        // recorded it before (Rule 1 prep + Rule 2 ACK). Falls back to a
        // synthetic "ib_<created_at>" tag so cancel paths still have a token
        // to send when the broker doesn't echo an id.
        if mgr.orders[idx].backend_order_id.is_none() {
            let token = if !ib.backend_id.is_empty() {
                ib.backend_id.clone()
            } else {
                format!("ib_{}", mgr.orders[idx].created_at.millis())
            };
            mgr.orders[idx].backend_order_id = Some(token);
        }

        // ── Rule 4: local Cancelled but broker still reports Working → re-cancel.
        if matches!(prev_state, OrderState::Cancelled)
            && matches!(broker_state, OrderState::Working | OrderState::PartialFill)
        {
            eprintln!("[reconcile] Rule 4: local Cancelled but broker Working — re-firing cancel id={}", local_id);
            journal::append(JournalEvent::Reconcile {
                client_id: local_id.to_string(),
                local: prev_state,
                broker: ib.status.clone(),
                resolution: "Rule 4: re-issue cancel".into(),
                ts_ms: now,
            });
            to_recancel.push(local_id);
            continue;
        }

        // ── Rule 6: broker filled_qty > local → adopt and toast.
        // Independent of state transition (might be PartialFill→PartialFill with more shares).
        let mut filled_qty_to_set: Option<u32> = None;
        if (qty_filled_broker as u32) > local_filled {
            filled_qty_to_set = Some(qty_filled_broker as u32);
        } else if (qty_filled_broker as u32) < local_filled {
            // Rule 6 inverse: keep local; broker hasn't caught up.
            // No-op intentionally; filled_qty stays.
        }

        // ── Rule 2: pending → broker has it → ACK the pending.
        // Rule 3 (else): Working/PartialFill → broker still has it → just refresh.
        // Rule 7 (terminal-sticky): once an order is Filled/Cancelled/Rejected
        // locally, never let a different broker endpoint revert it. The
        // /executions endpoint reports `filled` while /orders?status=submitted
        // can still list the same order as `submitted` for a window — without
        // this rule, the state oscillated Working↔Filled across polls and
        // each Working→Filled flip emitted another FILLED toast.
        let new_state = match (prev_state, broker_state) {
            // Rule 7: terminal states are absorbing — broker truth doesn't undo them.
            (s, _) if s.is_terminal() => s,
            // Rule 2a: PendingSubmit + Submitted/Working/Filled → adopt broker
            (OrderState::PendingSubmit, s) if matches!(s, OrderState::Working | OrderState::PartialFill | OrderState::Filled) => s,
            // Rule 2b: PendingCancel + Cancelled → confirm cancel
            (OrderState::PendingCancel, OrderState::Cancelled) => OrderState::Cancelled,
            // Rule 2c: PendingModify + Working → confirm modify (state-wise)
            (OrderState::PendingModify, OrderState::Working) => OrderState::Working,
            // Existing flow: Working → PartialFill / Filled / Cancelled / Rejected
            (_, s) => s,
        };

        // Reject rule: handle reasons regardless of state path.
        if let Some(reason) = rejection.clone() {
            mgr.orders[idx].rejection_reason = Some(reason);
        }

        let mut state_changed = false;
        if mgr.orders[idx].state != new_state {
            mgr.orders[idx].state = new_state;
            mgr.orders[idx].updated_at = ts_from_ms(now);
            mgr.orders[idx].state_history.push((new_state, ts_from_ms(now)));
            state_changed = true;
        }

        // Apply filled_qty / avg_fill_price updates.
        let mut filled_changed = false;
        if let Some(fq) = filled_qty_to_set {
            mgr.orders[idx].filled_qty = fq;
            mgr.orders[idx].avg_fill_price = Price::from_dollars(ib.avg_fill_price);
            filled_changed = true;
        } else if matches!(new_state, OrderState::Filled) && mgr.orders[idx].filled_qty == 0 && qty_filled_broker > 0 {
            mgr.orders[idx].filled_qty = qty_filled_broker as u32;
            mgr.orders[idx].avg_fill_price = Price::from_dollars(ib.avg_fill_price);
            filled_changed = true;
        }

        // Toasts + journal — only when something actually changed.
        if state_changed || filled_changed {
            let is_buy = matches!(mgr.orders[idx].side, OrderSide::Buy | OrderSide::TriggerBuy);
            let side_str = if is_buy { "BUY" } else { "SELL" };
            let sym = mgr.orders[idx].symbol.clone();
            let qty = mgr.orders[idx].qty;
            let fq = mgr.orders[idx].filled_qty;
            let avg = mgr.orders[idx].avg_fill_price.to_f32();
            // Toast firing rules — surgical to prevent repeated emissions
            // when the broker streams multiple execution records (one per
            // fill chunk) for the same trade. Each record bumps filled_qty
            // and used to re-toast even though state didn't transition.
            match new_state {
                OrderState::Filled => {
                    // FILLED is terminal — toast ONLY on the actual
                    // transition (state_changed). Subsequent filled_qty
                    // updates while already Filled (broker re-asserting the
                    // same trade from /executions) must be silent.
                    if state_changed {
                        fills += 1;
                        // Display fill qty: prefer the broker-reported filled_qty;
                        // fall back to the order's own qty if the broker didn't
                        // populate `filledQty` (some /executions shapes only
                        // carry price + symbol).
                        let display_qty = if fq > 0 { fq } else { qty };
                        mgr.push_toast_deduped(format!("FILLED: {} {} x{} @ {:.2}", side_str, sym, display_qty, avg));
                    }
                }
                OrderState::PartialFill => {
                    // PartialFill is genuinely re-fireable — each new chunk
                    // is informative. But still honor the dedup window so
                    // identical reads from /executions across polls don't
                    // spam.
                    if state_changed || filled_changed {
                        let display_fq = if fq > 0 { fq } else { qty };
                        mgr.push_toast_deduped(format!("PARTIAL: {} {} {}/{} @ {:.2}", side_str, sym, display_fq, qty, avg));
                    }
                }
                OrderState::Cancelled => {
                    if state_changed {
                        mgr.push_toast_deduped(format!("CANCELLED: {} {} x{}", side_str, sym, qty));
                    }
                }
                OrderState::Rejected => {
                    if state_changed {
                        let reason = mgr.orders[idx].rejection_reason.clone().unwrap_or_default();
                        mgr.push_toast_deduped(format!("REJECTED: {} {} x{} ({})", side_str, sym, qty, reason));
                    }
                }
                _ => {}
            }
            // Naming the rule that drove this transition for journal clarity.
            let resolution = match (prev_state, new_state) {
                (OrderState::PendingSubmit, OrderState::Working)
                | (OrderState::PendingSubmit, OrderState::PartialFill)
                | (OrderState::PendingSubmit, OrderState::Filled) => "Rule 2: ACK pending submit",
                (OrderState::PendingCancel, OrderState::Cancelled) => "Rule 2: confirm cancel",
                (OrderState::PendingModify, OrderState::Working) => "Rule 2: confirm modify",
                _ if filled_changed => "Rule 6: adopt broker filled_qty",
                _ => "Rule 3: refresh from broker",
            };
            journal::append(JournalEvent::Reconcile {
                client_id: local_id.to_string(),
                local: prev_state,
                broker: format!("{:?}", new_state),
                resolution: resolution.into(),
                ts_ms: now,
            });
        }
    }

    mgr.orders_filled += fills;

    // ── Rule 3: local Working but broker reports nothing for >15s → Unknown.
    {
        let mut seen = last_seen().lock().unwrap_or_else(|e| e.into_inner());
        // Mark every local order we matched this poll as "seen now".
        for id in &seen_local_ids { seen.insert(*id, now); }
        // For each active local order with a backend_order_id we previously
        // recorded, if the broker dropped it from this poll AND the gap
        // exceeds 15s AND we've already seen it at least once, flip to Unknown.
        let mut to_unknown: Vec<u64> = Vec::new();
        for o in &mgr.orders {
            if !matches!(o.state, OrderState::Working | OrderState::PartialFill) { continue; }
            if seen_local_ids.contains(&o.id) { continue; }
            let last = match seen.get(&o.id) { Some(t) => *t, None => continue };
            if now.saturating_sub(last) > 15_000 {
                to_unknown.push(o.id);
            }
        }
        // Drop the lock before mutating mgr.orders.
        drop(seen);
        for id in to_unknown {
            if let Some(o) = mgr.orders.iter_mut().find(|o| o.id == id) {
                let prev = o.state;
                if !matches!(prev, OrderState::Unknown) {
                    eprintln!("[reconcile] Rule 3: order {} not seen by broker for >15s — marking Unknown", id);
                    o.state = OrderState::Unknown;
                    o.updated_at = ts_from_ms(now);
                    o.state_history.push((OrderState::Unknown, ts_from_ms(now)));
                    journal::append(JournalEvent::Reconcile {
                        client_id: id.to_string(),
                        local: prev,
                        broker: "absent".into(),
                        resolution: "Rule 3: gap > 15s → Unknown".into(),
                        ts_ms: now,
                    });
                }
            }
            // Clear from seen so subsequent reappearance restarts the timer.
            if let Ok(mut s) = last_seen().lock() { s.remove(&id); }
        }
    }

    // ── Wave 4c: pair integrity sweep.
    // If one leg of a bracket/OCO is now terminal but its partner is still
    // active, cancel the partner. Catches the case where the broker fills
    // one leg out-of-band and never sends us the cancel for the other.
    let mut to_cancel_partners: Vec<u64> = Vec::new();
    for o in &mgr.orders {
        if let Some(pid) = o.pair_id {
            if o.state.is_terminal() {
                if let Some(p) = mgr.orders.iter().find(|x| x.id == pid) {
                    if p.state.is_active() {
                        to_cancel_partners.push(p.id);
                    }
                }
            }
        }
    }
    for cid in to_cancel_partners {
        eprintln!("[reconcile] orphan pair: cancelling partner id={}", cid);
        journal::append(JournalEvent::Reconcile {
            client_id: cid.to_string(),
            local: mgr.orders.iter().find(|o| o.id == cid).map(|o| o.state).unwrap_or(OrderState::Unknown),
            broker: "n/a".into(),
            resolution: "Wave 4c: orphan pair cancel".into(),
            ts_ms: now,
        });
        mgr.cancel(cid);
    }

    // ── Rule 4 follow-through: re-issue broker DELETE for orders the user
    // already cancelled locally that the broker still reports Working.
    // We can't go through `mgr.cancel()` — the order is already Cancelled
    // (terminal), so cancel() short-circuits via its `is_active` guard.
    // Fire the DELETE directly using the recorded backend id (idempotent
    // server-side).
    let paper = mgr.paper_mode;
    for id in to_recancel {
        let backend_id = mgr.orders.iter().find(|o| o.id == id)
            .and_then(|o| o.backend_order_id.clone());
        let Some(bid) = backend_id else { continue; };
        let cid_str = id.to_string();
        if paper {
            paper::cancel_paper(&bid);
            journal::append(JournalEvent::Ack { client_id: cid_str, backend_id: Some(bid), ts_ms: now });
        } else {
            let cid_thread = cid_str;
            let bid_thread = bid.clone();
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                let res = client.delete(format!("{}/orders/{}", APEXIB_URL, bid_thread))
                    .timeout(std::time::Duration::from_secs(5)).send();
                match res {
                    Ok(_) => journal::append(JournalEvent::Ack { client_id: cid_thread, backend_id: Some(bid_thread), ts_ms: epoch_ms() }),
                    Err(e) => journal::append(JournalEvent::Fail { client_id: cid_thread, reason: format!("rule4 recancel: {e}"), ts_ms: epoch_ms() }),
                }
            });
        }
    }

    snapshot::publish(&mgr.orders);
    // First sweep complete — from this point on, state transitions are
    // genuine new events the operator should see, not catch-up to yesterday.
    mgr.initial_reconcile_done = true;
}

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Wave 3: typed wall-clock timestamp for `ManagedOrder.created_at` /
/// `updated_at` / `state_history`. Local wall-clock — order lifecycle ts are
/// recorded by this process, not the exchange.
fn epoch_ts() -> Timestamp {
    Timestamp::from_millis(epoch_ms() as i64, TimeSource::Local)
}

/// Wrap an existing `u64` wall-clock ms into a typed `Timestamp`. Used at
/// construction sites that already captured `now_ms = epoch_ms()` for
/// downstream arithmetic and want a parallel typed value for the struct fields.
fn ts_from_ms(ms: u64) -> Timestamp {
    Timestamp::from_millis(ms as i64, TimeSource::Local)
}

/// Lock-free snapshot accessor for render path.
#[allow(dead_code)]
pub(crate) fn orders_snapshot() -> std::sync::Arc<crate::chart::renderer::trading::snapshot::OrdersSnapshot> {
    crate::chart::renderer::trading::snapshot::current()
}

// ─── Wave 6a: WAL replay & broker reconciliation ─────────────────────────────

/// Broker-side view of an order, returned by `query_broker_by_client_id`.
#[derive(Debug, Clone)]
pub(crate) enum BrokerOrderState {
    Working { backend_id: String, status: String },
    Filled { backend_id: String, fill_price: f32, qty: u32 },
    Cancelled { backend_id: String },
    Rejected { reason: String },
    NotFound,
}

/// Query ApexIB for the order with the given client_order_id (idempotency key).
/// Returns `None` if the HTTP request itself failed (network/timeout) — the
/// caller should leave the orphan alone and retry next startup.
///
/// TODO: confirm with backend — assumed endpoint shape is
/// `GET {APEXIB_URL}/orders/by-client-id/{cid}` returning a JSON object with
/// `status`, `orderId`, `avgFillPrice`, `filledQty`. If the endpoint isn't
/// available the request returns 404 → `Some(NotFound)`, which is the safe
/// outcome (we'll mark the local order Rejected with a clear reason).
fn query_broker_by_client_id(cid: &str) -> Option<BrokerOrderState> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/orders/by-client-id/{}", APEXIB_URL, cid);
    let resp = client.get(&url)
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .ok()?;
    let status = resp.status();
    if status.as_u16() == 404 {
        return Some(BrokerOrderState::NotFound);
    }
    if !status.is_success() {
        // Any other non-2xx is treated as transient — don't draw conclusions.
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    let backend_id = json["orderId"].as_str().map(|s| s.to_string())
        .or_else(|| json["orderId"].as_i64().map(|n| n.to_string()))
        .unwrap_or_default();
    let raw_status = json["status"].as_str().unwrap_or("").to_string();
    let lower = raw_status.to_ascii_lowercase();
    Some(match lower.as_str() {
        "filled" => BrokerOrderState::Filled {
            backend_id,
            fill_price: json["avgFillPrice"].as_f64().unwrap_or(0.0) as f32,
            qty: json["filledQty"].as_i64().unwrap_or(0).max(0) as u32,
        },
        "cancelled" | "apicancelled" => BrokerOrderState::Cancelled { backend_id },
        "rejected" | "inactive" => BrokerOrderState::Rejected {
            reason: format!("broker: {}", raw_status),
        },
        "submitted" | "presubmitted" | "working" | "" =>
            BrokerOrderState::Working { backend_id, status: raw_status },
        _ => BrokerOrderState::Working { backend_id, status: raw_status },
    })
}

/// Wave 6a: replay the WAL, find Attempts with no Ack/Fail, and ask the broker
/// what really happened. Adopts working/filled/cancelled state when broker has
/// it; marks the local row Rejected with `orphan-not-at-broker` when broker
/// never saw it; leaves orphans untouched on transport failure (retried next
/// startup or by the live reconciler).
pub(crate) fn replay_and_recover() {
    let orphans = journal::find_orphan_attempts();
    if orphans.is_empty() { return; }
    eprintln!("[recovery] {} orphan attempts — querying broker", orphans.len());

    for (cid, kind, ts, _payload) in orphans {
        // Cancel/CancelAll attempts are best handled by the live reconciler;
        // skip here to avoid double-action.
        if !matches!(kind, journal::AttemptKind::Submit) {
            continue;
        }
        match query_broker_by_client_id(&cid) {
            Some(BrokerOrderState::Working { backend_id, status }) => {
                with_mgr(|mgr| {
                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.client_order_id == cid) {
                        o.backend_order_id = Some(backend_id.clone());
                        if !matches!(o.state, OrderState::Working | OrderState::PartialFill) {
                            let now = epoch_ms();
                            o.state = OrderState::Working;
                            o.updated_at = ts_from_ms(now);
                            o.state_history.push((OrderState::Working, ts_from_ms(now)));
                        }
                    }
                    journal::append(JournalEvent::Reconcile {
                        client_id: cid.clone(),
                        local: OrderState::PendingSubmit,
                        broker: status,
                        resolution: format!("Wave 6a: adopt working (backend_id={})", backend_id),
                        ts_ms: epoch_ms(),
                    });
                    journal::append(JournalEvent::Ack {
                        client_id: cid.clone(),
                        backend_id: Some(backend_id),
                        ts_ms: epoch_ms(),
                    });
                    snapshot::publish(&mgr.orders);
                });
            }
            Some(BrokerOrderState::Filled { backend_id, fill_price, qty }) => {
                with_mgr(|mgr| {
                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.client_order_id == cid) {
                        let now = epoch_ms();
                        o.backend_order_id = Some(backend_id.clone());
                        o.state = OrderState::Filled;
                        o.filled_qty = qty;
                        o.avg_fill_price = Price::from_f32(fill_price);
                        o.updated_at = ts_from_ms(now);
                        o.state_history.push((OrderState::Filled, ts_from_ms(now)));
                    }
                    journal::append(JournalEvent::Reconcile {
                        client_id: cid.clone(),
                        local: OrderState::PendingSubmit,
                        broker: "filled".into(),
                        resolution: format!("Wave 6a: adopt filled (backend_id={})", backend_id),
                        ts_ms: epoch_ms(),
                    });
                    journal::append(JournalEvent::Ack {
                        client_id: cid.clone(),
                        backend_id: Some(backend_id),
                        ts_ms: epoch_ms(),
                    });
                    snapshot::publish(&mgr.orders);
                });
            }
            Some(BrokerOrderState::Cancelled { backend_id }) => {
                with_mgr(|mgr| {
                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.client_order_id == cid) {
                        let now = epoch_ms();
                        o.backend_order_id = Some(backend_id.clone());
                        o.state = OrderState::Cancelled;
                        o.updated_at = ts_from_ms(now);
                        o.state_history.push((OrderState::Cancelled, ts_from_ms(now)));
                    }
                    journal::append(JournalEvent::Reconcile {
                        client_id: cid.clone(),
                        local: OrderState::PendingSubmit,
                        broker: "cancelled".into(),
                        resolution: format!("Wave 6a: adopt cancelled (backend_id={})", backend_id),
                        ts_ms: epoch_ms(),
                    });
                    journal::append(JournalEvent::Ack {
                        client_id: cid.clone(),
                        backend_id: Some(backend_id),
                        ts_ms: epoch_ms(),
                    });
                    snapshot::publish(&mgr.orders);
                });
            }
            Some(BrokerOrderState::Rejected { reason }) => {
                with_mgr(|mgr| {
                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.client_order_id == cid) {
                        let now = epoch_ms();
                        o.state = OrderState::Rejected;
                        o.rejection_reason = Some(reason.clone());
                        o.updated_at = ts_from_ms(now);
                        o.state_history.push((OrderState::Rejected, ts_from_ms(now)));
                    }
                    journal::append(JournalEvent::Fail {
                        client_id: cid.clone(),
                        reason: reason.clone(),
                        ts_ms: epoch_ms(),
                    });
                    snapshot::publish(&mgr.orders);
                });
            }
            Some(BrokerOrderState::NotFound) => {
                with_mgr(|mgr| {
                    if let Some(o) = mgr.orders.iter_mut().find(|o| o.client_order_id == cid) {
                        let now = epoch_ms();
                        o.state = OrderState::Rejected;
                        o.rejection_reason = Some("orphan-not-at-broker".into());
                        o.updated_at = ts_from_ms(now);
                        o.state_history.push((OrderState::Rejected, ts_from_ms(now)));
                    }
                    journal::append(JournalEvent::Reconcile {
                        client_id: cid.clone(),
                        local: OrderState::PendingSubmit,
                        broker: "not-found".into(),
                        resolution: "orphan: never reached broker".into(),
                        ts_ms: epoch_ms(),
                    });
                    journal::append(JournalEvent::Fail {
                        client_id: cid.clone(),
                        reason: "orphan-not-at-broker".into(),
                        ts_ms: epoch_ms(),
                    });
                    snapshot::publish(&mgr.orders);
                });
            }
            None => {
                // Broker query failed (transport / timeout). Leave as orphan;
                // next startup or live reconciler will retry.
                eprintln!("[recovery] broker query failed for cid={} (kind={:?} ts={}) — leaving as orphan",
                    cid, kind, ts);
            }
        }
    }
    with_mgr(|mgr| mgr.save_to_disk());
}

// ─── Wave 6d: position drift reconciliation ───────────────────────────────────

/// Per-symbol toast cooldown so a persistent drift doesn't spam toasts every 5s.
fn drift_toast_cooldown() -> &'static Mutex<HashMap<String, Instant>> {
    static COOLDOWN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    COOLDOWN.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Wave 6d: compare broker positions to summed local Filled qty.
/// Logs a Reconcile journal event + toast on drift; does NOT auto-correct
/// (broker is authoritative for the account but we don't know which local
/// fill is "wrong" — surface to operator instead).
pub(crate) fn reconcile_positions(positions: &[super::Position]) {
    with_mgr(|mgr| {
        for pos in positions {
            let local_signed: i64 = mgr.orders.iter()
                .filter(|o| o.symbol.eq_ignore_ascii_case(&pos.symbol) && o.state == OrderState::Filled)
                .map(|o| {
                    let q = o.filled_qty as i64;
                    if matches!(o.side, OrderSide::Buy | OrderSide::TriggerBuy) { q } else { -q }
                })
                .sum();
            let broker_signed = pos.qty as i64;
            if local_signed != broker_signed {
                eprintln!("[reconcile-pos] {}: local={} broker={} drift={}",
                    pos.symbol, local_signed, broker_signed, broker_signed - local_signed);
                journal::append(JournalEvent::Reconcile {
                    client_id: format!("position:{}", pos.symbol),
                    local: OrderState::Filled,
                    broker: format!("pos={}", broker_signed),
                    resolution: format!("position drift: local={} broker={}", local_signed, broker_signed),
                    ts_ms: epoch_ms(),
                });
                // Rate-limit toasts: at most one per 30s per symbol.
                let mut emit = true;
                if let Ok(mut cd) = drift_toast_cooldown().lock() {
                    let key = pos.symbol.to_uppercase();
                    if let Some(last) = cd.get(&key) {
                        if last.elapsed() < std::time::Duration::from_secs(30) {
                            emit = false;
                        }
                    }
                    if emit {
                        cd.insert(key, Instant::now());
                    }
                }
                if emit {
                    mgr.pending_toasts.push(format!(
                        "POSITION DRIFT: {} local={} broker={}",
                        pos.symbol, local_signed, broker_signed
                    ));
                }
            }
        }
    });
}

/// Spawn a background sweeper that flips orders stuck in pending states
/// (PendingSubmit / PendingCancel / PendingModify) for >10s into `Unknown`,
/// so stale "in-flight" rows don't lie about being live.
fn start_pending_sweeper() {
    static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let now = epoch_ms();
                with_mgr(|mgr| {
                    let mut to_unknown: Vec<u64> = Vec::new();
                    for o in mgr.orders.iter() {
                        let pending = matches!(o.state, OrderState::PendingSubmit | OrderState::PendingCancel | OrderState::PendingModify);
                        if pending && now.saturating_sub(o.updated_at.millis() as u64) > 10_000 {
                            to_unknown.push(o.id);
                        }
                    }
                    for id in to_unknown {
                        eprintln!("[sweeper] order {} stuck pending — marking Unknown", id);
                        mgr.transition(id, OrderState::Unknown);
                    }
                });
            }
        });
    });
}

// ─── Broker-disconnect watchdog ─────────────────────────────────────────────
//
// The slow account poller calls `mark_broker_contact()` after every successful
// HTTP response. A dedicated 1s watchdog thread checks `last_broker_contact`
// and engages a protective auto-halt if we've gone >10s without contact. When
// contact is restored, the auto-halt is lifted — but only if WE engaged it
// (a user-engaged halt is preserved).

/// Stamp the most-recent successful broker contact. Called from the account
/// poller after any successful response. Cheap — single mutex grab + store.
pub(crate) fn mark_broker_contact() {
    with_mgr(|m| m.last_broker_contact = Some(epoch_ms()));
}

/// Seconds since the most recent successful broker contact, or `None` if
/// we've never seen one (cold start). Used by the Order System Health panel
/// to flag stale broker connectivity.
#[allow(dead_code)]
pub(crate) fn broker_contact_age_secs() -> Option<u64> {
    with_mgr(|m| m.last_broker_contact.map(|ts| epoch_ms().saturating_sub(ts) / 1000))
}

/// Watchdog tick — engages protective auto-halt when broker contact has been
/// stale for >10s, and lifts it only if WE engaged it (preserves user halts).
pub(crate) fn check_broker_health() {
    with_mgr(|m| {
        let now = epoch_ms();
        let stale = match m.last_broker_contact {
            None => false,  // never seen contact — startup; give it a chance
            Some(ts) => now.saturating_sub(ts) > 10_000,
        };
        if stale && !m.halted {
            // Engage protective halt. We don't fire /risk/halt to the broker
            // here — broker is unreachable; the local gate is what matters.
            m.halted = true;
            m.auto_halted_due_to_disconnect = true;
            m.pending_toasts.push("BROKER DISCONNECTED \u{2014} trading auto-halted".into());
            eprintln!("[broker-watchdog] disconnect >10s, halting");
            journal::append(JournalEvent::Control { kind: ControlKind::Halt, ts_ms: now });
        }
        if !stale && m.auto_halted_due_to_disconnect {
            // Contact restored, lift our auto-halt only.
            m.halted = false;
            m.auto_halted_due_to_disconnect = false;
            m.pending_toasts.push("BROKER RECONNECTED \u{2014} trading resumed".into());
            eprintln!("[broker-watchdog] contact restored, resuming");
            journal::append(JournalEvent::Control { kind: ControlKind::Resume, ts_ms: now });
        }
    });
}

/// Reason the trading gate is currently blocked. Returned as a tuple so the
/// UI can pick the right banner copy without re-locking the manager.
/// `(auto_halted_due_to_disconnect, kill_engaged, halted)`
pub(crate) fn trading_block_reason() -> (bool, bool, bool) {
    with_mgr(|m| (m.auto_halted_due_to_disconnect, m.kill_engaged, m.halted))
}

/// Spawn the disconnect watchdog. 1s cadence — not called from the poller
/// threads to avoid lock re-entry.
fn start_broker_watchdog() {
    static STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    STARTED.get_or_init(|| {
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                check_broker_health();
            }
        });
    });
}

// ─── Tests ────────────────────────────────────────────────────────────────────
//
// All tests instantiate a fresh `OrderManager` directly and never touch the
// global `ORDER_MANAGER` singleton — that path runs disk persistence side
// effects which would break test isolation. Tests run in paper mode so live
// HTTP submission is short-circuited; they accept the WAL disk writes since
// no clean knob exists to disable journaling in-test.
//
// Tests that need a live broker / live HTTP / mockable clock are marked
// `#[ignore]` with a TODO comment.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::renderer::trading::OrderSide;

    fn fresh_manager() -> OrderManager {
        let mut m = OrderManager::new();
        // Paper mode short-circuits the HTTP path on submit/cancel/modify.
        m.paper_mode = true;
        // Armed: limit orders go straight to PendingSubmit (else they sit in
        // Draft and submit() returns NeedsConfirmation, which makes happy-path
        // tests harder to express). The kill/halt/dedup paths we exercise here
        // don't depend on Draft semantics.
        m.armed = true;
        // Tighten dedup cooldown so cooldown-elapsed tests run fast (default
        // is 500ms; tests bring it down to 50ms).
        m.risk_limits.dedup_cooldown_ms = 50;
        // Skip the startup catch-up gate so tests that assert on toast pushes
        // (or future ones) see immediate emission. Production goes through
        // reconcile_with_ib_inner which flips this flag at end-of-sweep.
        m.initial_reconcile_done = true;
        m
    }

    fn limit_intent(symbol: &str, side: OrderSide, price: f32, qty: u32) -> OrderIntent {
        OrderIntent {
            symbol: symbol.to_string(),
            side,
            order_type: ManagedOrderType::Limit,
            price,
            stop_price: 0.0,
            qty,
            source: OrderSource::OrderPanel,
            pair_with: None,
            option_symbol: None,
            option_con_id: None,
            trail_amount: None,
            trail_percent: None,
            last_price: 0.0,
            tif: 0,
            outside_rth: false,
            strategy_id: None,
            override_warnings: false,
        }
    }

    fn order_id(result: &OrderResult) -> Option<u64> {
        match result {
            OrderResult::Accepted(id) | OrderResult::NeedsConfirmation(id) => Some(*id),
            // NeedsApproval intentionally yields None — no order was created.
            OrderResult::Rejected(_) | OrderResult::Duplicate | OrderResult::NeedsApproval { .. } => None,
        }
    }

    // ── A. Idempotency / dedup ───────────────────────────────────────────────

    #[test]
    fn submit_duplicate_within_cooldown_returns_duplicate() {
        let mut m = fresh_manager();
        // Bump cooldown well above the time between two submits in a tight
        // loop so the dedup window can't accidentally elapse between calls.
        m.risk_limits.dedup_cooldown_ms = 5_000;
        let i1 = limit_intent("AAPL", OrderSide::Buy, 100.0, 10);
        let i2 = i1.clone();
        let r1 = m.submit(i1);
        assert!(matches!(r1, OrderResult::Accepted(_)), "first submit should accept, got {:?}", r1);
        let r2 = m.submit(i2);
        assert!(matches!(r2, OrderResult::Duplicate), "duplicate within cooldown should return Duplicate, got {:?}", r2);
    }

    #[test]
    fn submit_duplicate_after_cooldown_succeeds() {
        let mut m = fresh_manager();
        m.risk_limits.dedup_cooldown_ms = 30;
        let i1 = limit_intent("AAPL", OrderSide::Buy, 100.0, 10);
        let i2 = i1.clone();
        let r1 = m.submit(i1);
        assert!(matches!(r1, OrderResult::Accepted(_)));
        std::thread::sleep(std::time::Duration::from_millis(60));
        let r2 = m.submit(i2);
        assert!(matches!(r2, OrderResult::Accepted(_)),
                "second submit after cooldown should accept, got {:?}", r2);
    }

    // ── B. Risk gate ─────────────────────────────────────────────────────────

    #[test]
    fn submit_qty_zero_is_rejected() {
        let mut m = fresh_manager();
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 0));
        match r {
            OrderResult::Rejected(reason) => {
                assert!(reason.to_lowercase().contains("qty"),
                        "expected qty rejection, got: {}", reason);
            }
            other => panic!("expected Rejected for qty=0, got {:?}", other),
        }
    }

    #[test]
    fn submit_qty_above_max_order_qty_is_rejected_in_live_mode() {
        // The risk-limit gate (`qty > max_order_qty`) only fires outside paper
        // mode. We exercise it by flipping paper_mode off — note this still
        // tries to spawn a live HTTP thread for submit_to_ib, but that's fine
        // because the rejection happens BEFORE submit_to_ib runs.
        let mut m = fresh_manager();
        m.paper_mode = false;
        m.risk_limits.max_order_qty = 100;
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 200));
        match r {
            OrderResult::Rejected(reason) => {
                assert!(reason.to_lowercase().contains("max"),
                        "expected max-qty rejection, got: {}", reason);
            }
            other => panic!("expected Rejected for qty>max, got {:?}", other),
        }
    }

    // ── C. In-flight cap (live mode only) ────────────────────────────────────

    #[test]
    fn working_plus_new_qty_exceeds_max_position_is_rejected() {
        // The in-flight cap (`working+new > max_position_qty`) only fires
        // outside paper. We push a Working order directly into orders[] so we
        // don't have to go through submit() (which would require live HTTP).
        let mut m = fresh_manager();
        m.paper_mode = false;
        m.risk_limits.max_position_qty = 1_000;
        // Pre-load 900 working buys in AAPL.
        let now = epoch_ms();
        m.orders.push(ManagedOrder {
            id: 1, client_order_id: "pre-1".into(),
            symbol: "AAPL".into(), symbol_typed: Symbol::equity("AAPL"), side: OrderSide::Buy,
            order_type: ManagedOrderType::Limit, price: Price::from_f32(100.0), stop_price: Price::ZERO,
            qty: 900, filled_qty: 0, avg_fill_price: Price::ZERO,
            state: OrderState::Working, pair_id: None,
            trail_amount: None, trail_percent: None,
            option_symbol: None, option_con_id: None,
            source: OrderSource::OrderPanel,
            created_at: ts_from_ms(now), updated_at: ts_from_ms(now), backend_order_id: None,
            tif: 0, outside_rth: false,
            state_history: vec![(OrderState::Working, ts_from_ms(now))],
            rejection_reason: None,
            modify_version: 0, modify_inflight: false, modify_pending_price: None,
        });
        m.next_id = 2;

        // 900 working + 200 new = 1100 > 1000 cap → reject.
        // Note the price is set above last_price=0 so the fat-finger check
        // does NOT fire (it requires last_price > 0).
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 200));
        match r {
            OrderResult::Rejected(reason) => {
                let lower = reason.to_lowercase();
                assert!(lower.contains("position") || lower.contains("working"),
                        "expected in-flight cap rejection, got: {}", reason);
            }
            other => panic!("expected Rejected for in-flight cap breach, got {:?}", other),
        }
    }

    // ── D. Kill switch ───────────────────────────────────────────────────────

    #[test]
    fn submit_while_kill_engaged_is_rejected() {
        let mut m = fresh_manager();
        m.kill_engaged = true; // direct flip — engage_kill() spawns broker HTTP.
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        match r {
            OrderResult::Rejected(reason) => {
                assert!(reason.contains("kill"), "expected 'kill' in reason, got: {}", reason);
            }
            other => panic!("expected Rejected, got {:?}", other),
        }
    }

    #[test]
    fn submit_after_release_kill_succeeds() {
        let mut m = fresh_manager();
        m.kill_engaged = true;
        let r1 = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        assert!(matches!(r1, OrderResult::Rejected(_)));
        m.kill_engaged = false;
        let r2 = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        assert!(matches!(r2, OrderResult::Accepted(_)),
                "after release_kill, submit should succeed, got {:?}", r2);
    }

    // ── E. Halt ──────────────────────────────────────────────────────────────

    #[test]
    fn submit_while_halted_is_rejected() {
        let mut m = fresh_manager();
        m.halted = true;
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        match r {
            OrderResult::Rejected(reason) => {
                assert!(reason.contains("halt") || reason.contains("disconnect"),
                        "expected halt-related reason, got: {}", reason);
            }
            other => panic!("expected Rejected, got {:?}", other),
        }
    }

    #[test]
    fn submit_after_resume_succeeds() {
        let mut m = fresh_manager();
        m.halted = true;
        let r1 = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        assert!(matches!(r1, OrderResult::Rejected(_)));
        m.halted = false;
        let r2 = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        assert!(matches!(r2, OrderResult::Accepted(_)),
                "after resume, submit should succeed, got {:?}", r2);
    }

    #[test]
    fn cancel_works_while_halted() {
        let mut m = fresh_manager();
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        let id = order_id(&r).expect("submit should accept");
        m.halted = true;
        let cancelled = m.cancel(id);
        assert!(cancelled, "cancel must work while halted");
        let o = m.orders.iter().find(|o| o.id == id).expect("order present");
        assert_eq!(o.state, OrderState::Cancelled);
    }

    // ── F. State machine ─────────────────────────────────────────────────────

    #[test]
    fn freshly_submitted_order_is_working_in_paper_mode() {
        // submit() unconditionally calls `transition(id, Working)` after the
        // PendingSubmit insert in paper mode, so the OBSERVABLE post-state
        // is Working. We verify the historical PendingSubmit was recorded too.
        let mut m = fresh_manager();
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        let id = order_id(&r).expect("submit should accept");
        let o = m.orders.iter().find(|o| o.id == id).expect("order present");
        assert_eq!(o.state, OrderState::Working,
                   "paper-mode submit short-circuits PendingSubmit -> Working");
        // History should contain at least the PendingSubmit and Working entries.
        let states: Vec<_> = o.state_history.iter().map(|(s, _)| *s).collect();
        assert!(states.contains(&OrderState::PendingSubmit),
                "state_history should record PendingSubmit, got {:?}", states);
        assert!(states.contains(&OrderState::Working),
                "state_history should record Working, got {:?}", states);
    }

    #[test]
    fn cancel_working_order_transitions_to_cancelled() {
        // In paper mode the cancel ack is synthesized inline, so the final
        // observable state is Cancelled (no PendingCancel hop visible).
        let mut m = fresh_manager();
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        let id = order_id(&r).expect("submit should accept");
        assert!(m.cancel(id));
        let o = m.orders.iter().find(|o| o.id == id).expect("order present");
        assert_eq!(o.state, OrderState::Cancelled);
    }

    // ── G. Modify version / coalescing ───────────────────────────────────────

    #[test]
    fn modify_price_twice_quickly_coalesces_pending_in_paper() {
        // In paper mode, `modify_price` calls `apply_modify_result_inner`
        // synchronously via the same &mut self. That means by the time
        // `modify_price` returns, modify_inflight has already been cleared
        // and any pending_price has been drained. So we can't observe the
        // mid-flight `modify_pending_price` set without breaking apart the
        // method. Instead, we verify the FINAL price equals the second call,
        // which is the correctness guarantee that matters.
        let mut m = fresh_manager();
        let r = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        let id = order_id(&r).expect("submit should accept");
        assert!(m.modify_price(id, Price::from_f32(101.0)));
        assert!(m.modify_price(id, Price::from_f32(102.0)));
        let o = m.orders.iter().find(|o| o.id == id).expect("order present");
        assert!((o.price.to_f32() - 102.0).abs() < 0.001,
                "after two modify_price calls, price should equal latest: got {}", o.price);
        // After the synchronous paper path settles, no inflight should remain.
        assert!(!o.modify_inflight, "modify_inflight should be cleared after paper modify settles");
        assert!(o.modify_pending_price.is_none(),
                "modify_pending_price should be drained after paper modify settles");
        // Two modify calls bumped the version twice.
        assert_eq!(o.modify_version, 2);
    }

    #[test]
    #[ignore = "modify_inflight is observable only with a non-paper broker that doesn't ack inline; needs a Broker trait or async pump to test"]
    fn modify_inflight_queues_second_call() {
        // TODO: introduce a Broker trait so we can stall the PUT and observe
        // modify_pending_price between the two modify_price calls.
    }

    // ── H. Pair integrity in reconcile ───────────────────────────────────────

    #[test]
    fn reconcile_partner_cancelled_when_one_pair_leg_fills() {
        // Manually create two paired managed orders (simulating an OCO) with
        // pair_id pointing at each other. Simulate the fill by setting one to
        // Filled directly (skips state machine), then run reconcile_with_ib_inner
        // with no IB orders — the pair-integrity sweep should cancel the partner.
        let mut m = fresh_manager();
        let now = epoch_ms();
        let mk = |id: u64, side: OrderSide, state: OrderState, pair: u64| ManagedOrder {
            id, client_order_id: format!("oco-{}", id),
            symbol: "AAPL".into(), symbol_typed: Symbol::equity("AAPL"), side,
            order_type: ManagedOrderType::Limit, price: Price::from_f32(100.0), stop_price: Price::ZERO,
            qty: 10, filled_qty: if matches!(state, OrderState::Filled) { 10 } else { 0 },
            avg_fill_price: Price::ZERO, state, pair_id: Some(pair),
            trail_amount: None, trail_percent: None,
            option_symbol: None, option_con_id: None,
            source: OrderSource::Oco,
            created_at: ts_from_ms(now), updated_at: ts_from_ms(now), backend_order_id: None,
            tif: 0, outside_rth: false,
            state_history: vec![(state, ts_from_ms(now))],
            rejection_reason: None,
            modify_version: 0, modify_inflight: false, modify_pending_price: None,
        };
        m.orders.push(mk(1, OrderSide::Buy,  OrderState::Filled,  2));
        m.orders.push(mk(2, OrderSide::Sell, OrderState::Working, 1));
        m.next_id = 3;

        // Empty IB feed — pair-integrity sweep still runs.
        reconcile_with_ib_inner(&mut m, &[]);

        let o2 = m.orders.iter().find(|o| o.id == 2).expect("partner present");
        assert_eq!(o2.state, OrderState::Cancelled,
                   "partner of a Filled leg should be cancelled by pair-integrity sweep");
    }

    // ── I. Reconciliation rules ──────────────────────────────────────────────

    mod reconcile_tests {
        use super::*;
        use crate::chart::renderer::trading::IbOrder;

        fn ib(symbol: &str, side: &str, qty: i32, status: &str) -> IbOrder {
            IbOrder {
                symbol: symbol.into(), side: side.into(),
                qty, filled_qty: 0,
                order_type: "limit".into(),
                limit_price: 100.0, avg_fill_price: 0.0,
                status: status.into(),
                strike: 0.0, option_type: "".into(),
                submitted_at: epoch_ms() as i64,
                backend_id: "".into(),
            }
        }

        #[test]
        fn rule1_id_match_wins_over_attribute_match() {
            // Local A has backend_order_id="abc", symbol AAPL qty 10 buy.
            // Local B has backend_order_id=None,  symbol AAPL qty 10 buy.
            // IB reports backend_id="abc" — must bind to A even though B is also
            // a candidate by (symbol, side, qty).
            let mut m = fresh_manager();
            let now = epoch_ms();
            let mut a = ManagedOrder {
                id: 10, client_order_id: "ka".into(),
                symbol: "AAPL".into(), symbol_typed: Symbol::equity("AAPL"), side: OrderSide::Buy,
                order_type: ManagedOrderType::Limit, price: Price::from_f32(100.0), stop_price: Price::ZERO,
                qty: 10, filled_qty: 0, avg_fill_price: Price::ZERO,
                state: OrderState::PendingSubmit, pair_id: None,
                trail_amount: None, trail_percent: None,
                option_symbol: None, option_con_id: None,
                source: OrderSource::OrderPanel,
                created_at: ts_from_ms(now), updated_at: ts_from_ms(now),
                backend_order_id: Some("abc".into()),
                tif: 0, outside_rth: false,
                state_history: vec![(OrderState::PendingSubmit, ts_from_ms(now))],
                rejection_reason: None,
                modify_version: 0, modify_inflight: false, modify_pending_price: None,
            };
            let mut b = a.clone();
            b.id = 11; b.client_order_id = "kb".into(); b.backend_order_id = None;
            m.orders.push(a.clone()); m.orders.push(b.clone());
            m.next_id = 12;

            let mut feed = ib("AAPL", "buy", 10, "submitted");
            feed.backend_id = "abc".into();
            reconcile_with_ib_inner(&mut m, &[feed]);

            // A should be Working (Rule 2 ACK), B untouched.
            let oa = m.orders.iter().find(|o| o.id == 10).unwrap();
            let ob = m.orders.iter().find(|o| o.id == 11).unwrap();
            assert_eq!(oa.state, OrderState::Working, "A bound by id should be ACKed Working");
            assert_eq!(ob.state, OrderState::PendingSubmit, "B not bound — must not transition");
        }

        #[test]
        fn rule2_pending_submit_acks_to_working() {
            let mut m = fresh_manager();
            let now = epoch_ms();
            m.orders.push(ManagedOrder {
                id: 1, client_order_id: "k".into(),
                symbol: "AAPL".into(), symbol_typed: Symbol::equity("AAPL"), side: OrderSide::Buy,
                order_type: ManagedOrderType::Limit, price: Price::from_f32(100.0), stop_price: Price::ZERO,
                qty: 10, filled_qty: 0, avg_fill_price: Price::ZERO,
                state: OrderState::PendingSubmit, pair_id: None,
                trail_amount: None, trail_percent: None,
                option_symbol: None, option_con_id: None,
                source: OrderSource::OrderPanel,
                created_at: ts_from_ms(now), updated_at: ts_from_ms(now), backend_order_id: None,
                tif: 0, outside_rth: false,
                state_history: vec![(OrderState::PendingSubmit, ts_from_ms(now))],
                rejection_reason: None,
                modify_version: 0, modify_inflight: false, modify_pending_price: None,
            });
            m.next_id = 2;

            reconcile_with_ib_inner(&mut m, &[ib("AAPL", "buy", 10, "submitted")]);

            let o = m.orders.iter().find(|o| o.id == 1).unwrap();
            assert_eq!(o.state, OrderState::Working);
        }

        #[test]
        #[ignore = "Rule 3 requires a mockable clock to fast-forward past the 15s gap; not yet supported"]
        fn rule3_gap_marks_unknown() {
            // TODO: thread an Instant source through last_seen() so we can
            // simulate "this order hasn't been seen in 15s" deterministically.
        }

        #[test]
        fn rule4_local_cancelled_broker_working_queues_recancel() {
            // Local Cancelled, IB reports same order Working → reconciler should
            // re-fire DELETE for the backend id. We can't observe the HTTP call
            // (paper mode skips it via paper::cancel_paper), but we can assert
            // that:
            //   - the local state stays Cancelled (Rule 4 short-circuits before
            //     the normal state-update path), and
            //   - a Reconcile journal event was appended (covered by inspecting
            //     the order's state_history is unchanged but a journal write
            //     happens; we don't have an in-test journal sink so we just
            //     assert state stability — the broker DELETE is fire-and-forget
            //     paper::cancel_paper which is a no-op).
            let mut m = fresh_manager();
            let now = epoch_ms();
            m.orders.push(ManagedOrder {
                id: 1, client_order_id: "k".into(),
                symbol: "AAPL".into(), symbol_typed: Symbol::equity("AAPL"), side: OrderSide::Buy,
                order_type: ManagedOrderType::Limit, price: Price::from_f32(100.0), stop_price: Price::ZERO,
                qty: 10, filled_qty: 0, avg_fill_price: Price::ZERO,
                state: OrderState::Cancelled, pair_id: None,
                trail_amount: None, trail_percent: None,
                option_symbol: None, option_con_id: None,
                source: OrderSource::OrderPanel,
                created_at: ts_from_ms(now), updated_at: ts_from_ms(now),
                backend_order_id: Some("abc".into()),
                tif: 0, outside_rth: false,
                state_history: vec![(OrderState::Cancelled, ts_from_ms(now))],
                rejection_reason: None,
                modify_version: 0, modify_inflight: false, modify_pending_price: None,
            });
            m.next_id = 2;

            let mut feed = ib("AAPL", "buy", 10, "submitted");
            feed.backend_id = "abc".into();
            reconcile_with_ib_inner(&mut m, &[feed]);

            let o = m.orders.iter().find(|o| o.id == 1).unwrap();
            // Rule 4 should NOT flip the local state — it stays Cancelled.
            assert_eq!(o.state, OrderState::Cancelled,
                       "Rule 4 must leave local state Cancelled (broker recancel only)");
        }

        #[test]
        fn rule5_adopts_external_order() {
            let mut m = fresh_manager();
            let mut feed = ib("AAPL", "buy", 10, "submitted");
            feed.backend_id = "ext-7".into();
            feed.limit_price = 99.5;
            reconcile_with_ib_inner(&mut m, &[feed]);

            assert_eq!(m.orders.len(), 1, "Rule 5 should adopt the external order");
            let o = &m.orders[0];
            assert_eq!(o.symbol, "AAPL");
            assert_eq!(o.qty, 10);
            assert!(matches!(o.side, OrderSide::Buy));
            assert!(matches!(o.source, OrderSource::Api),
                    "adopted order should have OrderSource::Api, got {:?}", o.source);
            assert_eq!(o.backend_order_id.as_deref(), Some("ext-7"));
        }

        #[test]
        fn rule6_filled_qty_divergence_local_catches_up_to_broker() {
            // Local PartialFill 50/100 at 100.0, broker reports filled_qty=80.
            // Local should adopt 80.
            let mut m = fresh_manager();
            let now = epoch_ms();
            m.orders.push(ManagedOrder {
                id: 1, client_order_id: "k".into(),
                symbol: "AAPL".into(), symbol_typed: Symbol::equity("AAPL"), side: OrderSide::Buy,
                order_type: ManagedOrderType::Limit, price: Price::from_f32(100.0), stop_price: Price::ZERO,
                qty: 100, filled_qty: 50, avg_fill_price: Price::from_f32(100.0),
                state: OrderState::PartialFill, pair_id: None,
                trail_amount: None, trail_percent: None,
                option_symbol: None, option_con_id: None,
                source: OrderSource::OrderPanel,
                created_at: ts_from_ms(now), updated_at: ts_from_ms(now),
                backend_order_id: Some("abc".into()),
                tif: 0, outside_rth: false,
                state_history: vec![(OrderState::PartialFill, ts_from_ms(now))],
                rejection_reason: None,
                modify_version: 0, modify_inflight: false, modify_pending_price: None,
            });
            m.next_id = 2;

            let mut feed = ib("AAPL", "buy", 100, "submitted");
            feed.backend_id = "abc".into();
            feed.filled_qty = 80;
            feed.avg_fill_price = 100.5;
            reconcile_with_ib_inner(&mut m, &[feed]);

            let o = m.orders.iter().find(|o| o.id == 1).unwrap();
            assert_eq!(o.filled_qty, 80, "local should catch up to broker filled_qty");
            assert!((o.avg_fill_price.to_f32() - 100.5).abs() < 0.001,
                    "avg_fill_price should be updated alongside filled_qty");
        }
    }

    // ── J. Buying-power check ────────────────────────────────────────────────

    #[test]
    #[ignore = "Buying-power check reads global ACCOUNT_DATA via super::read_account_data(); injecting a mock account is too invasive without a refactor. Skipped per task brief."]
    fn submit_with_insufficient_buying_power_is_rejected() {
        // TODO: introduce a small AccountSource trait or pass the snapshot
        // explicitly into submit() so this test can inject a low-bp account
        // without touching the global ACCOUNT_DATA OnceLock.
    }

    // ── K. WAL roundtrip ─────────────────────────────────────────────────────

    #[test]
    #[ignore = "wal::append writes to a hardcoded `state/orders.wal` next to the executable; testing roundtrip cleanly requires a path override (env var or feature flag) to avoid trampling the live WAL on developer machines."]
    fn wal_roundtrip_replays_in_order() {
        // TODO: thread a path override (env var `APEX_WAL_PATH` or a
        // feature flag) through wal::wal_path() so tests can write to a
        // temp dir.
    }

    // ── L. Wave 8a Task B: submit rate limit ────────────────────────────────

    #[test]
    fn submit_rate_limit_blocks_after_burst() {
        // 3-token burst, no refill: first three submits succeed, fourth must
        // reject with a rate-limit reason. We vary price across submits so
        // the dedup window (50ms in fresh_manager) doesn't catch them first —
        // this isolates the bucket gate from the dedup gate.
        let mut m = fresh_manager();
        m.set_submit_rate_limit(3.0, 0.0);

        let r1 = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        assert!(matches!(r1, OrderResult::Accepted(_)), "1st submit should accept, got {:?}", r1);
        let r2 = m.submit(limit_intent("AAPL", OrderSide::Buy, 101.0, 10));
        assert!(matches!(r2, OrderResult::Accepted(_)), "2nd submit should accept, got {:?}", r2);
        let r3 = m.submit(limit_intent("AAPL", OrderSide::Buy, 102.0, 10));
        assert!(matches!(r3, OrderResult::Accepted(_)), "3rd submit should accept, got {:?}", r3);

        let r4 = m.submit(limit_intent("AAPL", OrderSide::Buy, 103.0, 10));
        match r4 {
            OrderResult::Rejected(reason) => {
                assert!(reason.to_lowercase().contains("rate"),
                        "expected rate-limit rejection, got: {}", reason);
            }
            other => panic!("expected Rejected for 4th submit past burst cap, got {:?}", other),
        }
    }

    // ── M. Wave 8a Task C: paper-mode skips armed gate ──────────────────────

    #[test]
    fn paper_mode_skips_armed_confirmation() {
        // In paper mode with armed=false, a limit submit should still go
        // straight to Accepted (no armed-confirm gate). Practice trading is
        // friction-free by design — there's no financial risk to confirm.
        let mut m = fresh_manager();
        m.paper_mode = true;
        m.armed = false;
        let result = m.submit(limit_intent("AAPL", OrderSide::Buy, 100.0, 10));
        assert!(matches!(result, OrderResult::Accepted(_)),
                "paper-mode armed=false limit submit should be Accepted (no Draft gate), got {:?}",
                result);
    }

    // ── N. Wave 8a Task A: NeedsApproval soft gates ─────────────────────────

    #[test]
    fn fat_finger_returns_needs_approval_when_not_overridden() {
        // Live mode + a 50% deviation from last price should trip the
        // fat-finger gate. With override_warnings=false (default), the gate
        // returns NeedsApproval rather than a hard Rejected.
        let mut m = fresh_manager();
        m.paper_mode = false; // fat-finger is live-only
        let mut intent = limit_intent("AAPL", OrderSide::Buy, 150.0, 10);
        intent.last_price = 100.0; // 50% deviation — well past the 5% default
        let r = m.submit(intent);
        match r {
            OrderResult::NeedsApproval { reason, .. } => {
                assert!(reason.to_lowercase().contains("fat"),
                        "expected fat-finger reason, got: {}", reason);
            }
            other => panic!("expected NeedsApproval for fat-finger, got {:?}", other),
        }
    }

    #[test]
    fn fat_finger_override_warnings_bypasses_gate() {
        // Same intent, but override_warnings=true must clear the soft gate.
        // The submit will then proceed (Accepted in paper-mode-equivalent path
        // since we still have paper_mode=false here, but with bp check skipped
        // — buying_power is 0 in fresh test state so the bp branch logs and
        // proceeds). End state: Accepted, not NeedsApproval.
        let mut m = fresh_manager();
        m.paper_mode = false;
        // Crank max_order_qty so the live-mode qty cap doesn't trip first.
        m.risk_limits.max_order_qty = 1_000_000;
        let mut intent = limit_intent("AAPL", OrderSide::Buy, 150.0, 10);
        intent.last_price = 100.0;
        intent.override_warnings = true;
        let r = m.submit(intent);
        assert!(matches!(r, OrderResult::Accepted(_)),
                "override_warnings=true should clear fat-finger soft gate, got {:?}", r);
    }

    // ── O. Wave 3 regression: sub-cent dedup signature collision ────────────
    //
    // The legacy `OrderSignature` keyed on `(price * 100.0).round() as i64`
    // (integer cents). That bucketed $145.999 and $146.001 onto the same
    // signature — legitimate orders one tick apart were silently dropped as
    // duplicates. With `Price` micro-units the signature uses the full i64
    // and distinguishes them.

    #[test]
    fn wave3_dedup_signature_distinguishes_sub_cent_prices() {
        // Both prices round to the same integer-cents value (14600) but are
        // genuinely different micro-prices ($145.999 vs $146.001).
        let a = OrderSignature::new("AAPL", OrderSide::Buy, Price::from_dollars(145.999), 10);
        let b = OrderSignature::new("AAPL", OrderSide::Buy, Price::from_dollars(146.001), 10);
        assert_ne!(a, b,
                   "Wave 3: $145.999 and $146.001 must produce DIFFERENT signatures \
                    (legacy cents-rounding collapsed them onto the same bucket)");
        // Spell the underlying micro-unit difference for clarity:
        assert_eq!(a.price_micro, 145_999_000);
        assert_eq!(b.price_micro, 146_001_000);
    }

    #[test]
    fn wave3_dedup_does_not_block_legitimate_neighbouring_order() {
        // Real-world reproduction: submit at $145.999, then at $146.001 within
        // the cooldown window. The second must NOT be reported as Duplicate.
        let mut m = fresh_manager();
        m.risk_limits.dedup_cooldown_ms = 5_000;
        let i1 = limit_intent("AAPL", OrderSide::Buy, 145.999, 10);
        let i2 = limit_intent("AAPL", OrderSide::Buy, 146.001, 10);
        let r1 = m.submit(i1);
        let r2 = m.submit(i2);
        assert!(matches!(r1, OrderResult::Accepted(_)),
                "first submit must accept, got {:?}", r1);
        assert!(matches!(r2, OrderResult::Accepted(_)),
                "Wave 3: second submit one tick apart must NOT be Duplicate, got {:?}", r2);
    }

    // ── Wave 4: multi-leg paths now go through Broker trait ─────────────────

    #[test]
    fn wave4_submit_bracket_creates_three_linked_orders() {
        use super::super::broker::MockBroker;
        // We can't easily swap the *global* manager, so build a standalone
        // OrderManager wired to a MockBroker. submit_bracket mutates `self`
        // synchronously for the local-state half (creating 3 ManagedOrders
        // with pair_id set), then spawns a thread for the HTTP half. The
        // local-state assertions don't depend on the spawn completing.
        let mock = std::sync::Arc::new(MockBroker::new());
        let mut m = OrderManager::with_broker(mock.clone());
        m.paper_mode = true;
        m.armed = true;
        m.initial_reconcile_done = true;

        let intent = limit_intent("AAPL", OrderSide::Buy, 100.0, 10);
        let (entry, tp, sl) = m.submit_bracket(intent, 105.0, 95.0);
        assert!(matches!(entry, OrderResult::Accepted(_)),
                "entry leg should accept, got {:?}", entry);
        assert!(tp.is_some(), "tp leg id should be set");
        assert!(sl.is_some(), "sl leg id should be set");

        // 3 ManagedOrders, both child legs linked back to the entry id via pair_id.
        let snap: Vec<_> = m.orders.iter().filter(|o| o.symbol == "AAPL").collect();
        assert_eq!(snap.len(), 3, "bracket should create 3 local orders");
        let entry_id = match entry { OrderResult::Accepted(id) => id, _ => unreachable!() };
        let children: Vec<_> = snap.iter().filter(|o| o.pair_id == Some(entry_id)).collect();
        assert_eq!(children.len(), 2, "TP + SL must both pair back to entry");
    }
}
