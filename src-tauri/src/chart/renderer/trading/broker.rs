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
//! Wave 4 connectivity refactor: multi-leg variants (bracket, OCO,
//! conditional, combo, options-trigger) are now ALSO behind the trait. The
//! trait surface is complete — `OrderManager` no longer calls `reqwest`
//! inline for any submit path. Each multi-leg shape has its own typed args
//! struct because the HTTP bodies diverge enough that a single `submit`
//! signature can't carry them.

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

// ─── Multi-leg arg/response shapes (Wave 4) ────────────────────────────────

/// Inputs for a 3-leg bracket order (entry + take-profit + stop-loss).
/// All three legs go to the backend atomically through `POST /orders/bracket`.
#[derive(Debug, Clone)]
pub(crate) struct BracketSubmitArgs {
    pub symbol: String,
    pub side: String,                  // "buy" / "sell" (entry side)
    pub qty: u32,
    pub entry_order_type: String,      // "market" | "limit" | "stop" | "stop_limit"
    pub entry_price: f32,              // limit/stop price for entry leg; 0 for market
    pub take_profit_price: f32,
    pub stop_loss_price: f32,
    pub idempotency_key: String,
}

/// Backend IDs returned from a successful bracket submission. The TP/SL ids
/// are optional because some brokers only echo the parent on the synchronous
/// reply and the children come back through reconciliation.
#[derive(Debug, Clone, Default)]
pub(crate) struct BracketSubmitResponse {
    pub parent_backend_id: Option<String>,
    pub take_profit_backend_id: Option<String>,
    pub stop_loss_backend_id: Option<String>,
}

/// A single leg of an OCO (one-cancels-other) group, in wire shape.
#[derive(Debug, Clone)]
pub(crate) struct OcoLeg {
    pub symbol: String,
    pub side: String,         // "BUY" / "SELL"
    pub qty: u32,
    pub order_type: String,   // "market" | "limit" | "stop" | "stop_limit"
    pub price: f32,
    pub stop_price: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct OcoSubmitArgs {
    pub legs: Vec<OcoLeg>,
    pub oca_group: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct OcoSubmitResponse {
    /// Backend ids per leg, in the same order as `args.legs`. May be shorter
    /// than `legs.len()` if the server skipped legs (e.g. failed conId lookup).
    pub leg_backend_ids: Vec<String>,
}

/// One price-trigger condition watched by a conditional order.
#[derive(Debug, Clone)]
pub(crate) struct ConditionalCondition {
    pub con_id: i64,
    pub exchange: String,
    pub is_more: bool,
    pub price: Price,
}

#[derive(Debug, Clone)]
pub(crate) struct ConditionalSubmitArgs {
    pub symbol: String,
    pub side: String,                  // "BUY" / "SELL"
    pub qty: u32,
    pub order_type: String,            // "market" | "limit" | ...
    pub price: f32,                    // limit price (0 for non-limit)
    pub conditions: Vec<ConditionalCondition>,
    pub conditions_logic: String,      // "and" | "or"
    pub conditions_cancel_order: bool,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub(crate) struct OptionsTriggerArgs {
    pub underlying: String,
    pub option_type: String,           // "call" / "put"
    pub strike: f32,
    pub expiration: String,
    pub qty: u32,
    pub entry_price: f32,
    pub entry_direction: String,
    pub exit_price: f32,
    pub exit_direction: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct OptionsTriggerResponse {
    pub entry_backend_id: Option<String>,
    pub option_con_id: Option<i64>,
}

/// One leg of a multi-leg combo / spread / pairs order.
#[derive(Debug, Clone)]
pub(crate) struct ComboLeg {
    pub con_id: i64,
    pub ratio: i32,
    pub side: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ComboSubmitArgs {
    pub symbol: String,
    pub side: String,                  // "buy" / "sell" on the combo as a whole
    pub qty: u32,
    pub order_type: String,            // "market" | "limit"
    pub limit_price: Option<f32>,
    pub legs: Vec<ComboLeg>,
    pub idempotency_key: String,
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

    // ── Multi-leg paths (Wave 4) ────────────────────────────────────────────

    /// Submit a 3-leg bracket (entry + TP + SL). The entry's backend id is
    /// returned in `parent_backend_id`; TP/SL ids come back when the broker
    /// echoes them on the synchronous reply (may be `None` for brokers that
    /// only return the parent up-front).
    fn submit_bracket(&self, args: &BracketSubmitArgs) -> Result<BracketSubmitResponse, String>;

    /// Submit an OCO (one-cancels-other) group. Returns the backend id of
    /// each leg, in the same order as `args.legs`.
    fn submit_oco(&self, args: &OcoSubmitArgs) -> Result<OcoSubmitResponse, String>;

    /// Submit a conditional order — primary order that activates only when
    /// one or more price triggers fire on watched contracts.
    fn submit_conditional(&self, args: &ConditionalSubmitArgs) -> Result<String, String>;

    /// Submit an options-trigger order — entry + exit on an option contract
    /// driven by underlying price levels.
    fn submit_options_trigger(&self, args: &OptionsTriggerArgs) -> Result<OptionsTriggerResponse, String>;

    /// Submit a multi-leg combo / spread / pairs order. All legs fill
    /// atomically via the broker's native combo mechanism.
    fn submit_combo(&self, args: &ComboSubmitArgs) -> Result<String, String>;
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

    // ── Multi-leg paths (Wave 4) ────────────────────────────────────────────

    fn submit_bracket(&self, args: &BracketSubmitArgs) -> Result<BracketSubmitResponse, String> {
        let client = reqwest::blocking::Client::new();
        let con_id = Self::resolve_con_id(&client, &args.symbol)
            .ok_or_else(|| format!("[bracket] no conId for {}", args.symbol))?;

        let mut entry_leg = serde_json::json!({"orderType": args.entry_order_type});
        match args.entry_order_type.as_str() {
            "limit" => { entry_leg["limitPrice"] = serde_json::json!(args.entry_price); }
            "stop"  => { entry_leg["stopPrice"]  = serde_json::json!(args.entry_price); }
            "stop_limit" => {
                entry_leg["limitPrice"] = serde_json::json!(args.entry_price);
                entry_leg["stopPrice"]  = serde_json::json!(args.entry_price);
            }
            _ => {} // market
        }
        let body = serde_json::json!({
            "conId": con_id, "side": args.side, "quantity": args.qty,
            "entry": entry_leg,
            "takeProfit": {"orderType": "limit", "limitPrice": args.take_profit_price},
            "stopLoss":   {"orderType": "stop",  "stopPrice":  args.stop_loss_price},
            "tif": "day",
            "idempotencyKey": args.idempotency_key,
        });
        let resp = client.post(format!("{}/orders/bracket", APEXIB_URL))
            .json(&body).timeout(std::time::Duration::from_secs(5)).send()
            .map_err(|e| format!("bracket http: {e}"))?;
        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("bracket json: {e}"))?;
        let pick = |k: &str| -> Option<String> {
            json[k].as_str().map(|s| s.to_string())
                .or_else(|| json[k].as_i64().map(|n| n.to_string()))
        };
        Ok(BracketSubmitResponse {
            parent_backend_id:      pick("parentOrderId"),
            take_profit_backend_id: pick("takeProfitOrderId"),
            stop_loss_backend_id:   pick("stopLossOrderId"),
        })
    }

    fn submit_oco(&self, args: &OcoSubmitArgs) -> Result<OcoSubmitResponse, String> {
        let client = reqwest::blocking::Client::new();
        let mut oco_orders = Vec::new();
        for leg in &args.legs {
            let con_id = match Self::resolve_con_id(&client, &leg.symbol) {
                Some(c) => c, None => continue,
            };
            let mut order_json = serde_json::json!({
                "conId": con_id, "side": leg.side, "quantity": leg.qty,
                "orderType": leg.order_type, "tif": "day",
            });
            match leg.order_type.as_str() {
                "limit" => { order_json["limitPrice"] = serde_json::json!(leg.price); }
                "stop"  => {
                    order_json["stopPrice"] = serde_json::json!(
                        if leg.stop_price != 0.0 { leg.stop_price } else { leg.price }
                    );
                }
                "stop_limit" => {
                    order_json["limitPrice"] = serde_json::json!(leg.price);
                    order_json["stopPrice"]  = serde_json::json!(leg.stop_price);
                }
                _ => {}
            }
            oco_orders.push(order_json);
        }
        let body = serde_json::json!({ "orders": oco_orders, "ocaGroup": args.oca_group });
        let resp = client.post(format!("{}/orders/oco", APEXIB_URL))
            .json(&body).timeout(std::time::Duration::from_secs(5)).send()
            .map_err(|e| format!("oco http: {e}"))?;
        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("oco json: {e}"))?;
        let mut leg_backend_ids = Vec::new();
        if let Some(arr) = json["orderIds"].as_array() {
            for bid in arr {
                if let Some(s) = bid.as_str() { leg_backend_ids.push(s.to_string()); }
                else if let Some(n) = bid.as_i64() { leg_backend_ids.push(n.to_string()); }
            }
        }
        Ok(OcoSubmitResponse { leg_backend_ids })
    }

    fn submit_conditional(&self, args: &ConditionalSubmitArgs) -> Result<String, String> {
        let client = reqwest::blocking::Client::new();
        let con_id = Self::resolve_con_id(&client, &args.symbol)
            .ok_or_else(|| format!("[conditional] no conId for {}", args.symbol))?;
        let conds_json: Vec<serde_json::Value> = args.conditions.iter().map(|c| {
            serde_json::json!({
                "type": "price",
                "conId": c.con_id,
                "exchange": c.exchange,
                "isMore": c.is_more,
                "price": c.price.to_f32(),
                "triggerMethod": "default",
            })
        }).collect();
        let mut body = serde_json::json!({
            "conId": con_id, "side": args.side, "quantity": args.qty,
            "orderType": args.order_type, "tif": "day",
            "conditions": conds_json,
            "conditionsLogic": args.conditions_logic,
            "conditionsCancelOrder": args.conditions_cancel_order,
            "outsideRth": true,
            "idempotencyKey": args.idempotency_key,
        });
        if args.order_type == "limit" {
            body["limitPrice"] = serde_json::json!(args.price);
        }
        let resp = client.post(format!("{}/orders/conditional", APEXIB_URL))
            .json(&body).timeout(std::time::Duration::from_secs(5)).send()
            .map_err(|e| format!("conditional http: {e}"))?;
        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("conditional json: {e}"))?;
        Self::extract_order_id(&json)
            .ok_or_else(|| "conditional: broker returned no orderId".into())
    }

    fn submit_options_trigger(&self, args: &OptionsTriggerArgs) -> Result<OptionsTriggerResponse, String> {
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({
            "underlying": args.underlying,
            "optionType": args.option_type,
            "strike": args.strike,
            "expiration": args.expiration,
            "quantity": args.qty,
            "entryPrice": args.entry_price,
            "entryDirection": args.entry_direction,
            "exitPrice": args.exit_price,
            "exitDirection": args.exit_direction,
            "exitOrderType": "market",
            "idempotencyKey": args.idempotency_key,
        });
        let resp = client.post(format!("{}/orders/options-trigger", APEXIB_URL))
            .json(&body).timeout(std::time::Duration::from_secs(5)).send()
            .map_err(|e| format!("options-trigger http: {e}"))?;
        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("options-trigger json: {e}"))?;
        Ok(OptionsTriggerResponse {
            entry_backend_id: json["entryOrderId"].as_str().map(|s| s.to_string()),
            option_con_id:    json["optionConId"].as_i64(),
        })
    }

    fn submit_combo(&self, args: &ComboSubmitArgs) -> Result<String, String> {
        let client = reqwest::blocking::Client::new();
        let legs_json: Vec<serde_json::Value> = args.legs.iter().map(|l| {
            serde_json::json!({
                "conId": l.con_id,
                "ratio": l.ratio,
                "side":  l.side,
            })
        }).collect();
        let mut url = format!(
            "{}/orders/combo?symbol={}&side={}&quantity={}&orderType={}&tif=day&idempotencyKey={}",
            APEXIB_URL, args.symbol, args.side, args.qty, args.order_type, args.idempotency_key
        );
        if let Some(lp) = args.limit_price {
            url.push_str(&format!("&limitPrice={}", lp));
        }
        let resp = client.post(&url).json(&legs_json)
            .timeout(std::time::Duration::from_secs(5)).send()
            .map_err(|e| format!("combo http: {e}"))?;
        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("combo json: {e}"))?;
        Self::extract_order_id(&json)
            .ok_or_else(|| "combo: broker returned no orderId".into())
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

    // ── Multi-leg paths (Wave 4): synthetic acks, no HTTP ──────────────────

    fn submit_bracket(&self, args: &BracketSubmitArgs) -> Result<BracketSubmitResponse, String> {
        Ok(BracketSubmitResponse {
            parent_backend_id:      Some(format!("paper:{}:entry", args.idempotency_key)),
            take_profit_backend_id: Some(format!("paper:{}:tp",    args.idempotency_key)),
            stop_loss_backend_id:   Some(format!("paper:{}:sl",    args.idempotency_key)),
        })
    }

    fn submit_oco(&self, args: &OcoSubmitArgs) -> Result<OcoSubmitResponse, String> {
        let ids = (0..args.legs.len())
            .map(|i| format!("paper:{}:{}", args.oca_group, i))
            .collect();
        Ok(OcoSubmitResponse { leg_backend_ids: ids })
    }

    fn submit_conditional(&self, args: &ConditionalSubmitArgs) -> Result<String, String> {
        Ok(format!("paper:{}", args.idempotency_key))
    }

    fn submit_options_trigger(&self, args: &OptionsTriggerArgs) -> Result<OptionsTriggerResponse, String> {
        Ok(OptionsTriggerResponse {
            entry_backend_id: Some(format!("paper:{}", args.idempotency_key)),
            option_con_id: None,
        })
    }

    fn submit_combo(&self, args: &ComboSubmitArgs) -> Result<String, String> {
        Ok(format!("paper:{}", args.idempotency_key))
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
    // Multi-leg paths (Wave 4)
    Bracket(BracketSubmitArgs),
    Oco(OcoSubmitArgs),
    Conditional(ConditionalSubmitArgs),
    OptionsTrigger(OptionsTriggerArgs),
    Combo(ComboSubmitArgs),
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

    // ── Multi-leg paths (Wave 4): record + synthetic Ok ─────────────────────

    fn submit_bracket(&self, args: &BracketSubmitArgs) -> Result<BracketSubmitResponse, String> {
        self.record(MockCall::Bracket(args.clone()));
        Ok(BracketSubmitResponse {
            parent_backend_id:      Some(format!("mock:{}:entry", args.idempotency_key)),
            take_profit_backend_id: Some(format!("mock:{}:tp",    args.idempotency_key)),
            stop_loss_backend_id:   Some(format!("mock:{}:sl",    args.idempotency_key)),
        })
    }

    fn submit_oco(&self, args: &OcoSubmitArgs) -> Result<OcoSubmitResponse, String> {
        let ids = (0..args.legs.len())
            .map(|i| format!("mock:{}:{}", args.oca_group, i))
            .collect();
        self.record(MockCall::Oco(args.clone()));
        Ok(OcoSubmitResponse { leg_backend_ids: ids })
    }

    fn submit_conditional(&self, args: &ConditionalSubmitArgs) -> Result<String, String> {
        self.record(MockCall::Conditional(args.clone()));
        Ok(format!("mock:{}", args.idempotency_key))
    }

    fn submit_options_trigger(&self, args: &OptionsTriggerArgs) -> Result<OptionsTriggerResponse, String> {
        self.record(MockCall::OptionsTrigger(args.clone()));
        Ok(OptionsTriggerResponse {
            entry_backend_id: Some(format!("mock:{}", args.idempotency_key)),
            option_con_id: None,
        })
    }

    fn submit_combo(&self, args: &ComboSubmitArgs) -> Result<String, String> {
        self.record(MockCall::Combo(args.clone()));
        Ok(format!("mock:{}", args.idempotency_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bracket_args() -> BracketSubmitArgs {
        BracketSubmitArgs {
            symbol: "AAPL".into(),
            side: "buy".into(),
            qty: 10,
            entry_order_type: "limit".into(),
            entry_price: 100.0,
            take_profit_price: 105.0,
            stop_loss_price: 95.0,
            idempotency_key: "bk-1".into(),
        }
    }

    #[test]
    fn mock_broker_records_bracket() {
        let mock = MockBroker::new();
        let args = bracket_args();
        let resp = mock.submit_bracket(&args).expect("bracket ok");
        assert!(resp.parent_backend_id.is_some());
        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
        assert!(matches!(calls[0], MockCall::Bracket(_)));
    }

    #[test]
    fn mock_broker_records_oco() {
        let mock = MockBroker::new();
        let args = OcoSubmitArgs {
            legs: vec![
                OcoLeg { symbol: "AAPL".into(), side: "SELL".into(), qty: 10,
                         order_type: "limit".into(), price: 110.0, stop_price: 0.0 },
                OcoLeg { symbol: "AAPL".into(), side: "SELL".into(), qty: 10,
                         order_type: "stop".into(),  price: 0.0,   stop_price: 90.0 },
            ],
            oca_group: "oca-1".into(),
        };
        let resp = mock.submit_oco(&args).expect("oco ok");
        assert_eq!(resp.leg_backend_ids.len(), 2);
        let calls = mock.calls();
        assert!(matches!(calls[0], MockCall::Oco(_)));
    }

    #[test]
    fn mock_broker_records_conditional() {
        let mock = MockBroker::new();
        let args = ConditionalSubmitArgs {
            symbol: "AAPL".into(), side: "BUY".into(), qty: 5,
            order_type: "limit".into(), price: 99.0,
            conditions: vec![ConditionalCondition {
                con_id: 12345, exchange: "SMART".into(),
                is_more: true, price: Price::from_f32(100.0),
            }],
            conditions_logic: "and".into(),
            conditions_cancel_order: false,
            idempotency_key: "cond-1".into(),
        };
        let oid = mock.submit_conditional(&args).expect("cond ok");
        assert!(oid.starts_with("mock:"));
        assert!(matches!(mock.calls()[0], MockCall::Conditional(_)));
    }

    #[test]
    fn mock_broker_records_options_trigger() {
        let mock = MockBroker::new();
        let args = OptionsTriggerArgs {
            underlying: "SPY".into(), option_type: "call".into(),
            strike: 500.0, expiration: "20260101".into(),
            qty: 1, entry_price: 495.0, entry_direction: "above".into(),
            exit_price: 505.0, exit_direction: "above".into(),
            idempotency_key: "opt-1".into(),
        };
        let resp = mock.submit_options_trigger(&args).expect("opt ok");
        assert!(resp.entry_backend_id.is_some());
        assert!(matches!(mock.calls()[0], MockCall::OptionsTrigger(_)));
    }

    #[test]
    fn mock_broker_records_combo() {
        let mock = MockBroker::new();
        let args = ComboSubmitArgs {
            symbol: "SPY".into(), side: "buy".into(), qty: 1,
            order_type: "limit".into(), limit_price: Some(2.50),
            legs: vec![
                ComboLeg { con_id: 1, ratio: 1,  side: "BUY".into() },
                ComboLeg { con_id: 2, ratio: -1, side: "SELL".into() },
            ],
            idempotency_key: "combo-1".into(),
        };
        let oid = mock.submit_combo(&args).expect("combo ok");
        assert!(oid.starts_with("mock:"));
        assert!(matches!(mock.calls()[0], MockCall::Combo(_)));
    }

    #[test]
    fn paper_broker_returns_synthetic_bracket_ids() {
        let p = PaperBroker;
        let resp = p.submit_bracket(&bracket_args()).expect("paper bracket ok");
        assert_eq!(resp.parent_backend_id.as_deref(), Some("paper:bk-1:entry"));
        assert_eq!(resp.take_profit_backend_id.as_deref(), Some("paper:bk-1:tp"));
        assert_eq!(resp.stop_loss_backend_id.as_deref(), Some("paper:bk-1:sl"));
    }
}
