//! Broker abstraction — Live HTTP, Paper simulator, and Mock for tests.
//!
//! Wave 7 Task 1: HTTP calls for the single-order submit/cancel/modify path
//! used to be scattered as inline `reqwest::blocking::Client::new().post(...)`
//! blocks inside `order_manager.rs`. That made tests gate on `paper_mode` to
//! avoid hitting the network and leaked broker concerns into the manager.
//!
//! `OrderManager` now holds an `Arc<dyn Broker>`. Submit/cancel/modify
//! delegate to the broker; the manager does state, dedup, and risk only.
//!
//! Multi-leg variants (bracket, OCO, conditional, combo, options-trigger)
//! still call `reqwest` inline — their HTTP shapes diverge enough that a
//! single `submit` signature can't cover them, and tests don't exercise
//! those paths through the trait. They remain available for a follow-up.

use std::sync::Mutex;

use super::APEXIB_URL;
use crate::foundation::types::Price;

/// What the broker reports back when we ask "do you have this order?"
/// Used by orphan-recovery (`replay_and_recover`) to learn the real state of
/// an order whose Attempt journal entry was never matched by an Ack/Fail.
#[derive(Debug, Clone)]
pub(crate) enum BrokerOrderState {
    Working { backend_id: String, status: String },
    Filled { backend_id: String, fill_price: f32, qty: u32 },
    Cancelled { backend_id: String },
    Rejected { reason: String },
    NotFound,
}

/// All inputs needed to submit a single working order. Borrows everything
/// because callers usually construct this from already-owned fields and
/// don't need the broker to take ownership.
#[derive(Debug)]
pub(crate) struct SubmitArgs<'a> {
    pub symbol: &'a str,
    pub side: &'a str,         // "buy" / "sell"
    pub qty: u32,
    pub order_type_idx: usize, // 0=market 1=limit 2=stop 3=stop_limit 4=trailing_stop
    /// Wave 3: typed Price (micro-dollars). Converted to f32 at the wire
    /// boundary in `LiveBroker::submit` so the JSON shape stays the same.
    pub price: Price,
    pub stop_price: Price,
    pub trail_amount: Option<f32>,
    pub trail_percent: Option<f32>,
    pub client_order_id: &'a str, // idempotency key
    pub tif: u8,                  // 0=day 1=gtc 2=ioc
    pub outside_rth: bool,
}

/// Broker contract.
///
/// All methods are synchronous and may block on HTTP — callers spawn threads
/// when they want to fire-and-forget. The manager's submit/cancel/modify
/// path does exactly that: stage local state, then `thread::spawn(move || broker.submit(...))`.
pub(crate) trait Broker: Send + Sync {
    /// Submit a single order; returns the broker-side id on success.
    fn submit(&self, args: &SubmitArgs) -> Result<String, String>;

    /// Cancel by broker order id.
    fn cancel(&self, backend_id: &str, client_order_id: &str) -> Result<(), String>;

    /// Modify an existing order's price (and optionally quantity).
    fn modify(
        &self,
        backend_id: &str,
        client_order_id: &str,
        new_price: Price,
        new_qty: u32,
    ) -> Result<(), String>;

    /// Lookup by client_order_id. Used by `replay_and_recover` after an
    /// Attempt was journaled but no Ack/Fail arrived.
    fn lookup_by_client_id(&self, client_order_id: &str) -> BrokerOrderState;

    /// Server-side kill / halt / resume. Default impls no-op since not all
    /// brokers expose these endpoints.
    fn kill(&self) -> Result<(), String> { Ok(()) }
    fn halt(&self) -> Result<(), String> { Ok(()) }
    fn resume(&self) -> Result<(), String> { Ok(()) }
}

// ─── LiveBroker — real HTTP ─────────────────────────────────────────────────

pub(crate) struct LiveBroker;

impl LiveBroker {
    fn resolve_con_id(client: &reqwest::blocking::Client, symbol: &str) -> Option<i64> {
        client.get(format!("{}/contract/{}", APEXIB_URL, symbol))
            .timeout(std::time::Duration::from_secs(5)).send()
            .and_then(|r| r.json::<serde_json::Value>())
            .ok()
            .and_then(|j| j["conId"].as_i64())
    }

    fn extract_order_id(json: &serde_json::Value) -> Option<String> {
        json["orderId"].as_str().map(|s| s.to_string())
            .or_else(|| json["orderId"].as_i64().map(|n| n.to_string()))
    }
}

impl Broker for LiveBroker {
    fn submit(&self, args: &SubmitArgs) -> Result<String, String> {
        let client = reqwest::blocking::Client::new();
        let con_id = Self::resolve_con_id(&client, args.symbol)
            .ok_or_else(|| format!("conId lookup failed for {}", args.symbol))?;

        let order_type = match args.order_type_idx {
            0 => "market", 1 => "limit", 2 => "stop",
            3 => "stop_limit", 4 => "trailing_stop", _ => "market",
        };
        let tif = match args.tif { 0 => "day", 1 => "gtc", 2 => "ioc", _ => "day" };
        let mut body = serde_json::json!({
            "conId": con_id, "side": args.side, "quantity": args.qty,
            "orderType": order_type, "tif": tif,
            "idempotencyKey": args.client_order_id,
        });
        if args.outside_rth { body["outsideRth"] = serde_json::json!(true); }
        // Convert typed Price to f32 at the wire boundary; broker JSON shape
        // is unchanged from before Wave 3.
        let price_f = args.price.to_f32();
        let stop_f = args.stop_price.to_f32();
        match order_type {
            "limit" => { body["limitPrice"] = serde_json::json!(price_f); }
            "stop" => {
                body["stopPrice"] = serde_json::json!(
                    if !args.stop_price.is_zero() { stop_f } else { price_f }
                );
            }
            "stop_limit" => {
                body["limitPrice"] = serde_json::json!(price_f);
                body["stopPrice"] = serde_json::json!(stop_f);
            }
            "trailing_stop" => {
                if let Some(amt) = args.trail_amount { body["trailAmount"] = serde_json::json!(amt); }
                if let Some(pct) = args.trail_percent { body["trailPercent"] = serde_json::json!(pct); }
                if !args.stop_price.is_zero() { body["stopPrice"] = serde_json::json!(stop_f); }
            }
            _ => {}
        }

        let resp = client.post(format!("{}/orders", APEXIB_URL))
            .json(&body).timeout(std::time::Duration::from_secs(5)).send()
            .map_err(|e| format!("submit http: {e}"))?;
        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("submit json: {e}"))?;
        Self::extract_order_id(&json).ok_or_else(|| "broker returned no orderId".into())
    }

    fn cancel(&self, backend_id: &str, _client_order_id: &str) -> Result<(), String> {
        let client = reqwest::blocking::Client::new();
        client.delete(format!("{}/orders/{}", APEXIB_URL, backend_id))
            .timeout(std::time::Duration::from_secs(5)).send()
            .map(|_| ())
            .map_err(|e| format!("cancel http: {e}"))
    }

    fn modify(&self, backend_id: &str, _client_order_id: &str, new_price: Price, _new_qty: u32) -> Result<(), String> {
        // Caller is responsible for choosing limitPrice vs stopPrice — we
        // can't know the order type without re-fetching. Use limitPrice as
        // the default and let the manager call back through with the right
        // body shape if needed (current call sites only modify limit price).
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({"limitPrice": new_price.to_f32()});
        client.put(format!("{}/orders/{}", APEXIB_URL, backend_id))
            .json(&body).timeout(std::time::Duration::from_secs(5)).send()
            .map(|_| ())
            .map_err(|e| format!("modify http: {e}"))
    }

    fn lookup_by_client_id(&self, client_order_id: &str) -> BrokerOrderState {
        let client = reqwest::blocking::Client::new();
        let url = format!("{}/orders/by-client-id/{}", APEXIB_URL, client_order_id);
        let resp = match client.get(&url)
            .timeout(std::time::Duration::from_secs(3)).send() {
            Ok(r) => r,
            Err(_) => return BrokerOrderState::NotFound, // transient — caller leaves orphan
        };
        let status = resp.status();
        if status.as_u16() == 404 { return BrokerOrderState::NotFound; }
        if !status.is_success() { return BrokerOrderState::NotFound; }
        let json: serde_json::Value = match resp.json() {
            Ok(j) => j,
            Err(_) => return BrokerOrderState::NotFound,
        };
        let backend_id = json["orderId"].as_str().map(|s| s.to_string())
            .or_else(|| json["orderId"].as_i64().map(|n| n.to_string()))
            .unwrap_or_default();
        let raw_status = json["status"].as_str().unwrap_or("").to_string();
        let lower = raw_status.to_ascii_lowercase();
        match lower.as_str() {
            "filled" => BrokerOrderState::Filled {
                backend_id,
                fill_price: json["avgFillPrice"].as_f64().unwrap_or(0.0) as f32,
                qty: json["filledQty"].as_i64().unwrap_or(0).max(0) as u32,
            },
            "cancelled" | "apicancelled" => BrokerOrderState::Cancelled { backend_id },
            "rejected" | "inactive" => BrokerOrderState::Rejected {
                reason: format!("broker: {}", raw_status),
            },
            _ => BrokerOrderState::Working { backend_id, status: raw_status },
        }
    }

    fn kill(&self) -> Result<(), String> {
        let client = reqwest::blocking::Client::new();
        client.post(format!("{}/risk/kill", APEXIB_URL))
            .timeout(std::time::Duration::from_secs(10)).send()
            .map(|_| ())
            .map_err(|e| format!("kill http: {e}"))
    }
    fn halt(&self) -> Result<(), String> {
        let client = reqwest::blocking::Client::new();
        client.post(format!("{}/risk/halt", APEXIB_URL))
            .timeout(std::time::Duration::from_secs(5)).send()
            .map(|_| ())
            .map_err(|e| format!("halt http: {e}"))
    }
    fn resume(&self) -> Result<(), String> {
        let client = reqwest::blocking::Client::new();
        client.post(format!("{}/risk/resume", APEXIB_URL))
            .timeout(std::time::Duration::from_secs(5)).send()
            .map(|_| ())
            .map_err(|e| format!("resume http: {e}"))
    }
}

// ─── PaperBroker — local simulator ──────────────────────────────────────────

pub(crate) struct PaperBroker;

impl Broker for PaperBroker {
    fn submit(&self, args: &SubmitArgs) -> Result<String, String> {
        // Paper-trading: orders ack immediately and stay Working until cancel.
        Ok(format!("paper:{}", args.client_order_id))
    }

    fn cancel(&self, _backend_id: &str, _client_order_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn modify(&self, _backend_id: &str, _client_order_id: &str, _new_price: Price, _new_qty: u32) -> Result<(), String> {
        Ok(())
    }

    fn lookup_by_client_id(&self, _client_order_id: &str) -> BrokerOrderState {
        // Paper has no out-of-band state — orphan recovery should never need
        // to hit this in paper mode (paper acks inline) but if it does,
        // return NotFound so the manager marks the orphan rejected.
        BrokerOrderState::NotFound
    }
}

// ─── MockBroker — for unit tests ────────────────────────────────────────────

/// Recorded call to a `MockBroker` method. Tests assert against the queue.
#[derive(Debug, Clone)]
#[allow(dead_code)] // fields read via Debug formatting in tests
pub(crate) enum MockCall {
    Submit { symbol: String, side: String, qty: u32, price: Price, client_order_id: String },
    Cancel { backend_id: String, client_order_id: String },
    Modify { backend_id: String, client_order_id: String, new_price: Price, new_qty: u32 },
    Lookup { client_order_id: String },
}

/// Pre-canned response a test wants the next `submit` call to return.
/// `cancel`/`modify` always succeed; if a test needs them to fail, it can
/// add a similar enum + queue.
#[derive(Debug, Clone)]
pub(crate) enum MockResponse {
    SubmitOk(String),         // backend_id to return
    SubmitErr(String),        // error reason
    Lookup(BrokerOrderState),
}

pub(crate) struct MockBroker {
    calls: Mutex<Vec<MockCall>>,
    /// FIFO queue of canned responses. Tests `enqueue_response` before
    /// triggering the path; the broker pops as it answers.
    responses: Mutex<Vec<MockResponse>>,
}

impl MockBroker {
    pub(crate) fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            responses: Mutex::new(Vec::new()),
        }
    }

    /// Snapshot of recorded calls, oldest first.
    pub(crate) fn calls(&self) -> Vec<MockCall> {
        self.calls.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Pre-load a response for the next matching call. Submit pops a
    /// `SubmitOk`/`SubmitErr`; lookup pops a `Lookup`. If the queue is empty
    /// for a given call type, the broker returns a sensible default (Submit
    /// → Ok with `mock:{cid}`, Lookup → NotFound).
    pub(crate) fn enqueue_response(&self, response: MockResponse) {
        if let Ok(mut g) = self.responses.lock() { g.push(response); }
    }

    fn record(&self, call: MockCall) {
        if let Ok(mut g) = self.calls.lock() { g.push(call); }
    }

    /// Pop the first response of the given variant, leaving others in place.
    fn take_submit_response(&self) -> Option<MockResponse> {
        if let Ok(mut g) = self.responses.lock() {
            let pos = g.iter().position(|r| matches!(r, MockResponse::SubmitOk(_) | MockResponse::SubmitErr(_)));
            return pos.map(|i| g.remove(i));
        }
        None
    }
    fn take_lookup_response(&self) -> Option<BrokerOrderState> {
        if let Ok(mut g) = self.responses.lock() {
            let pos = g.iter().position(|r| matches!(r, MockResponse::Lookup(_)));
            return pos.map(|i| match g.remove(i) {
                MockResponse::Lookup(s) => s,
                _ => unreachable!(),
            });
        }
        None
    }
}

impl Broker for MockBroker {
    fn submit(&self, args: &SubmitArgs) -> Result<String, String> {
        self.record(MockCall::Submit {
            symbol: args.symbol.into(), side: args.side.into(),
            qty: args.qty, price: args.price,
            client_order_id: args.client_order_id.into(),
        });
        match self.take_submit_response() {
            Some(MockResponse::SubmitOk(id)) => Ok(id),
            Some(MockResponse::SubmitErr(reason)) => Err(reason),
            _ => Ok(format!("mock:{}", args.client_order_id)),
        }
    }

    fn cancel(&self, backend_id: &str, client_order_id: &str) -> Result<(), String> {
        self.record(MockCall::Cancel {
            backend_id: backend_id.into(),
            client_order_id: client_order_id.into(),
        });
        Ok(())
    }

    fn modify(&self, backend_id: &str, client_order_id: &str, new_price: Price, new_qty: u32) -> Result<(), String> {
        self.record(MockCall::Modify {
            backend_id: backend_id.into(),
            client_order_id: client_order_id.into(),
            new_price, new_qty,
        });
        Ok(())
    }

    fn lookup_by_client_id(&self, client_order_id: &str) -> BrokerOrderState {
        self.record(MockCall::Lookup { client_order_id: client_order_id.into() });
        self.take_lookup_response().unwrap_or(BrokerOrderState::NotFound)
    }
}
