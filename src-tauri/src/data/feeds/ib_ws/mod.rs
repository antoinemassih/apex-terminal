//! ib_ws — Rust-native IB WebSocket client (hot path)
//!
//! Replaces the TypeScript WebSocket in IBKRProvider.
//! Connects to ibserver ws://127.0.0.1:5000/ws, decodes MessagePack binary
//! frames in Rust, and emits `ib-tick` Tauri events to the React frontend
//! via direct IPC (no TCP round-trip on the hot path).
//!
//! Control messages (subscribe/unsubscribe) sent as JSON text — ibserver
//! receive side stays unchanged.

pub mod aggregator;
pub mod resolver;

use std::{collections::{HashMap, HashSet}, sync::Arc, sync::OnceLock, time::Duration};
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64};
use parking_lot::Mutex as StdMutex;
use crate::data::connectivity::ConnectionState;
use crate::data::feeds::apex_data::types::{AssetClass, BarWire, Quote, Trade};

// ── Wave 12a: per-symbol mpsc fanout hubs ────────────────────────────────────
//
// `feeds::ib_ws::ws_loop` historically emitted decoded ticks as `ib-tick`
// Tauri events directly to the WebView. Wave 12a additionally fans every
// observed tick into per-symbol Rust-side streams so `IbProvider`'s
// `MarketDataProvider::subscribe_*` methods can return honest streams.
//
// Storage: `Mutex<HashMap<String, Vec<UnboundedSender<T>>>>`. The IB hub is
// bounded by active symbols (typically a few dozen) and the hot path holds
// the lock only long enough to clone+retain the sender vec, so contention is
// negligible. Avoiding `dashmap` keeps the dependency surface lean (matches
// `resolver` rationale).
//
// Subscribers receive an `UnboundedReceiver<T>`. Dropping the receiver causes
// the corresponding `UnboundedSender::send` to fail, and the next tick that
// touches that symbol prunes the dead sender via `retain`.
type SenderVec<T> = Vec<tokio::sync::mpsc::UnboundedSender<T>>;
type Hub<T> = StdMutex<HashMap<String, SenderVec<T>>>;

// Wave 14b: bar hub keyed by (symbol, tf) so subscribers only see frames
// for their own timeframe — no receive-side filter needed. Quote/Trade
// hubs stay symbol-keyed: there is no TF concept on a tick stream.
type BarHub = StdMutex<HashMap<(String, String), SenderVec<BarWire>>>;

static BAR_HUB:   OnceLock<BarHub>      = OnceLock::new();
static QUOTE_HUB: OnceLock<Hub<Quote>>  = OnceLock::new();
static TRADE_HUB: OnceLock<Hub<Trade>>  = OnceLock::new();

pub fn bar_hub()   -> &'static BarHub       { BAR_HUB.get_or_init(Default::default) }
pub fn quote_hub() -> &'static Hub<Quote>   { QUOTE_HUB.get_or_init(Default::default) }
pub fn trade_hub() -> &'static Hub<Trade>   { TRADE_HUB.get_or_init(Default::default) }

// ── Wave 14b: active timeframes per symbol ───────────────────────────────────
//
// The ws_loop tick path iterates this set to know which `(sym, tf)` buckets
// to fold each tick into. Populated by `bar_hub_subscribe`, pruned in
// `bar_hub_fanout` when the last subscriber for `(sym, tf)` drops.
//
// Storage: `Mutex<HashMap<String, HashSet<String>>>`. Bounded by active
// subscriptions (small); lock is held only for the duration of a clone()
// on the hot path. Lives next to the hubs because its lifecycle is
// inseparable from `bar_hub_subscribe` / `bar_hub_fanout`.
static ACTIVE_TFS: OnceLock<StdMutex<HashMap<String, HashSet<String>>>> = OnceLock::new();

fn active_tfs() -> &'static StdMutex<HashMap<String, HashSet<String>>> {
    ACTIVE_TFS.get_or_init(Default::default)
}

/// Snapshot of timeframes currently subscribed for `symbol`. Cloned out so
/// the tick path can iterate without holding the mutex.
pub(crate) fn active_tfs_for(symbol: &str) -> Vec<String> {
    let map = active_tfs().lock();
    map.get(symbol).map(|s| s.iter().cloned().collect()).unwrap_or_default()
}

/// Register a new subscriber for `symbol` on a symbol-keyed hub
/// (Quote / Trade). Returns the receiver end; senders are stored in the
/// hub for the next tick fanout.
pub(crate) fn hub_subscribe<T>(hub: &'static Hub<T>, symbol: &str)
    -> tokio::sync::mpsc::UnboundedReceiver<T>
{
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut map = hub.lock();
    map.entry(symbol.to_string()).or_default().push(tx);
    rx
}

/// Register a new subscriber for `(symbol, tf)` on the bar hub. Also
/// records `tf` in the per-symbol active-TF set so `ws_loop` knows to
/// fold each tick through that timeframe's aggregator bucket.
pub(crate) fn bar_hub_subscribe(symbol: &str, tf: &str)
    -> tokio::sync::mpsc::UnboundedReceiver<BarWire>
{
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    {
        let mut map = bar_hub().lock();
        map.entry((symbol.to_string(), tf.to_string())).or_default().push(tx);
    }
    {
        let mut tfs = active_tfs().lock();
        tfs.entry(symbol.to_string()).or_default().insert(tf.to_string());
    }
    rx
}

/// Fan `value` out to every live subscriber for `symbol`, pruning closed
/// senders in the same pass. Cheap when nobody is subscribed (early return
/// on empty entry; lock held for the duration of the inner `retain`).
fn hub_fanout<T: Clone>(hub: &'static Hub<T>, symbol: &str, value: T) {
    let mut map = hub.lock();
    let Some(senders) = map.get_mut(symbol) else { return };
    if senders.is_empty() { return; }
    senders.retain(|s| s.send(value.clone()).is_ok());
    if senders.is_empty() {
        map.remove(symbol);
    }
}

/// Bar-hub fanout for the `(symbol, tf)` key. Mirrors `hub_fanout` but
/// also prunes the entry's TF from `active_tfs` when the last subscriber
/// for that (sym, tf) goes away — so the ws_loop tick path stops folding
/// ticks into a bucket nobody is listening for.
pub(crate) fn bar_hub_fanout(symbol: &str, tf: &str, value: BarWire) {
    let became_empty = {
        let mut map = bar_hub().lock();
        let key = (symbol.to_string(), tf.to_string());
        let Some(senders) = map.get_mut(&key) else { return };
        if senders.is_empty() { return; }
        senders.retain(|s| s.send(value.clone()).is_ok());
        let empty = senders.is_empty();
        if empty { map.remove(&key); }
        empty
    };
    if became_empty {
        let mut tfs = active_tfs().lock();
        if let Some(set) = tfs.get_mut(symbol) {
            set.remove(tf);
            if set.is_empty() { tfs.remove(symbol); }
        }
    }
}

// ── Wave 6: per-feed metrics counters ────────────────────────────────────────
pub(crate) static MESSAGES_IN:        AtomicU64 = AtomicU64::new(0);
pub(crate) static PARSE_ERRORS:       AtomicU64 = AtomicU64::new(0);
pub(crate) static RECONNECT_COUNT:    AtomicU32 = AtomicU32::new(0);
pub(crate) static LAST_MESSAGE_AT_MS: AtomicI64 = AtomicI64::new(0);

// ── Wave 7E: WS resilience ───────────────────────────────────────────────────
const HEARTBEAT_SECS: u64 = 30;
const STALE_TIMEOUT_MS: i64 = 30_000;
const WATCHDOG_TICK_SECS: u64 = 10;
pub(crate) static FORCE_RECONNECT: AtomicBool = AtomicBool::new(false);
pub(crate) static LAST_STALL_AT_MS: AtomicI64 = AtomicI64::new(0);

// ── Wave 11c: ConnectionState push-notification stream ───────────────────────
static STATE_TX: OnceLock<tokio::sync::broadcast::Sender<ConnectionState>> = OnceLock::new();

pub fn state_tx() -> &'static tokio::sync::broadcast::Sender<ConnectionState> {
    STATE_TX.get_or_init(|| tokio::sync::broadcast::channel(64).0)
}

pub(crate) fn publish_state(s: ConnectionState) {
    let _ = state_tx().send(s);
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tauri::{AppHandle, Emitter, async_runtime};
use tokio::{
    sync::{mpsc, Mutex},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crate::data::connectivity::{self, errors_sink::{report, ErrorLevel}, Backoff};
use std::sync::atomic::{AtomicBool, Ordering};

const WS_URL: &str = "ws://127.0.0.1:5000/ws";

// ── Command channel ───────────────────────────────────────────────────────────

#[allow(dead_code)]
pub enum Cmd {
    /// JSON text frame to forward to ibserver (subscribe/unsubscribe)
    Send(String),
    Shutdown,
}

// ── Public handle (managed by Tauri state) ───────────────────────────────────

pub struct IbWsHandle {
    pub tx: mpsc::Sender<Cmd>,
    /// conIds currently active — restored verbatim on reconnect
    pub subscribed: Arc<Mutex<HashSet<i64>>>,
    /// Set to true by `Shutdown::drain` to stop the reconnect loop.
    pub shutdown: Arc<AtomicBool>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn spawn(app: AppHandle) -> IbWsHandle {
    let (tx, rx) = mpsc::channel::<Cmd>(512);
    let subscribed: Arc<Mutex<HashSet<i64>>> = Default::default();
    let shutdown = Arc::new(AtomicBool::new(false));
    async_runtime::spawn(ws_loop(app, rx, subscribed.clone(), shutdown.clone()));
    // Wave 7E: tick-age watchdog — flips FORCE_RECONNECT when silent past STALE_TIMEOUT_MS.
    async_runtime::spawn(run_watchdog());
    let handle = IbWsHandle { tx: tx.clone(), subscribed, shutdown: shutdown.clone() };
    connectivity::register("ib_ws", Arc::new(IbWsShutdown { tx, shutdown }));
    handle
}

struct IbWsShutdown {
    tx: mpsc::Sender<Cmd>,
    shutdown: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl connectivity::Shutdown for IbWsShutdown {
    async fn drain(&self, deadline: Duration) -> Result<(), String> {
        tracing::info!(target: "shutdown", connection = "ib_ws", "drain invoked");
        self.shutdown.store(true, Ordering::SeqCst);
        // Best-effort: send Shutdown so the loop exits the inner select.
        let _ = self.tx.send(Cmd::Shutdown).await;
        // Give the loop a chance to flush + close cleanly.
        tokio::time::sleep(deadline.min(Duration::from_millis(500))).await;
        Ok(())
    }
}

// ── Background task ───────────────────────────────────────────────────────────

async fn ws_loop(
    app: AppHandle,
    mut rx: mpsc::Receiver<Cmd>,
    subscribed: Arc<Mutex<HashSet<i64>>>,
    shutdown: Arc<AtomicBool>,
) {
    let mut backoff = Backoff::new().with_max_attempts(None);
    let mut first = true;
    loop {
        if shutdown.load(Ordering::SeqCst) {
            publish_state(ConnectionState::ShuttingDown);
            return;
        }
        if !first { RECONNECT_COUNT.fetch_add(1, Ordering::Relaxed); }
        first = false;
        publish_state(ConnectionState::Connecting {
            attempt: RECONNECT_COUNT.load(Ordering::Relaxed) + 1,
        });
        match connect_async(WS_URL).await {
            Ok((stream, _)) => {
                backoff.reset();
                let _ = app.emit("ib-connected", ());
                publish_state(ConnectionState::Authenticated);
                {
                    let n = subscribed.lock().await.len();
                    if n > 0 {
                        publish_state(ConnectionState::Subscribed { count: n });
                    }
                }
                // Wave 7E: trigger SubscriptionManager gap-fill on reconnect
                // (skip the initial connect — only run when we recovered).
                if RECONNECT_COUNT.load(Ordering::Relaxed) > 0 {
                    tokio::spawn(async {
                        let mgr = crate::data::providers::registry::subscription_manager();
                        let n = mgr.gap_fill_on_reconnect_all().await;
                        if n > 0 {
                            report(
                                ErrorLevel::Info,
                                "ib_ws",
                                "gap_fill",
                                format!("replayed {n} bars after reconnect"),
                            );
                        }
                    });
                }
                let (mut write, mut read) = stream.split();

                // Re-subscribe after reconnect
                {
                    let ids: Vec<i64> = subscribed.lock().await.iter().copied().collect();
                    if !ids.is_empty() {
                        let text =
                            serde_json::json!({"action": "subscribe", "conIds": ids}).to_string();
                        let _ = write.send(Message::Text(text)).await;
                    }
                }

                // Wave 7E: app-level heartbeat — ibserver does not emit text
                // chatter, so without this a stalled TCP socket goes unnoticed
                // for hours.
                FORCE_RECONNECT.store(false, Ordering::SeqCst);
                let mut heartbeat = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECS));
                heartbeat.tick().await; // skip immediate first tick
                let mut force_check = tokio::time::interval(Duration::from_secs(1));
                force_check.tick().await;

                let mut clean_shutdown = false;
                loop {
                    if FORCE_RECONNECT.swap(false, Ordering::SeqCst) {
                        report(ErrorLevel::Warn, "ib_ws", "force_reconnect", "watchdog tripped");
                        publish_state(ConnectionState::Backoff {
                            until: std::time::Instant::now() + Duration::from_secs(1),
                            attempt: RECONNECT_COUNT.load(Ordering::Relaxed),
                            reason: "tick_stalled".into(),
                        });
                        let _ = write.close().await;
                        break;
                    }
                    tokio::select! {
                        biased; // check commands first so subscribe acks aren't delayed

                        _ = heartbeat.tick() => {
                            if let Err(e) = write.send(Message::Ping(Vec::new().into())).await {
                                report(ErrorLevel::Warn, "ib_ws", "ping_failed", e.to_string());
                                break;
                            }
                        }
                        _ = force_check.tick() => { /* loop top re-checks FORCE_RECONNECT */ }

                        cmd = rx.recv() => match cmd {
                            Some(Cmd::Send(text)) => {
                                if write.send(Message::Text(text)).await.is_err() {
                                    break;
                                }
                            }
                            Some(Cmd::Shutdown) | None => {
                                let _ = write.close().await;
                                clean_shutdown = true;
                                break;
                            }
                        },

                        frame = read.next() => match frame {
                            // ── Hot path: binary msgpack tick data ──────────
                            Some(Ok(Message::Binary(bytes))) => {
                                MESSAGES_IN.fetch_add(1, Ordering::Relaxed);
                                LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);
                                let parsed = rmp_serde::from_slice::<Value>(&bytes);
                                if parsed.is_err() { PARSE_ERRORS.fetch_add(1, Ordering::Relaxed); }
                                if let Ok(val) = parsed {
                                    // Forward to native chart renderer if active
                                    if let Value::Object(ref map) = val {
                                        if let (Some(price), Some(volume)) = (
                                            map.get("price").and_then(|v| v.as_f64()),
                                            map.get("volume").and_then(|v| v.as_f64()),
                                        ) {
                                            let p = price as f32;
                                            let v = volume as f32;
                                            let sym = map.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                            // Wave 9b: every tick carries both `symbol` and
                                            // `conId` — observe the pair so the resolver
                                            // fills passively as the user streams symbols.
                                            // Accept either casing defensively; ibserver
                                            // is camelCase today but msgpack producers vary.
                                            if !sym.is_empty() {
                                                if let Some(cid) = map
                                                    .get("conId")
                                                    .and_then(|v| v.as_i64())
                                                    .or_else(|| map.get("conid").and_then(|v| v.as_i64()))
                                                {
                                                    resolver::resolver().observe(&sym, cid);
                                                }
                                            }
                                            // Wave 8c: bump SubscriptionManager so IB-fed
                                            // (sym, "5m") subs become visible to gap-fill.
                                            // IB subscribes per-conId (not per-symbol), so
                                            // the bumper at frame ingress is the only
                                            // symbol-keyed integration point. Time field
                                            // is best-effort: prefer payload `time` (ms),
                                            // otherwise fall back to wall-clock now.
                                            let ts_ms = map.get("time")
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or_else(now_ms);
                                            crate::data::providers::registry::subscription_manager()
                                                .bump_last_seen_bar(
                                                    &sym,
                                                    "5m",
                                                    crate::data::providers::subscription_manager::BarSource::Last,
                                                    ts_ms,
                                                );
                                            // Wave 13b/14b: fold tick into the OHLC aggregator
                                            // for EVERY active timeframe on this symbol and fan
                                            // out a live (not-yet-closed) bar per TF. When a tick
                                            // crosses a bucket boundary the previous bucket is
                                            // also fanned out with `closed=true` (as a separate
                                            // frame, AFTER the live bar) so downstream
                                            // resampler/storage can finalize it.
                                            //
                                            // Active TFs come from `active_tfs_for(sym)`, which is
                                            // driven by `bar_hub_subscribe`. If nobody subscribes,
                                            // we still fold at "1m" so resolver/SubscriptionManager
                                            // wiring and the native chart UpdateLastBar below stay
                                            // identical to Wave 13b behavior.
                                            //
                                            // Quote fanout still fires only when the payload
                                            // actually carries bid/ask fields (defensive — current
                                            // ibserver doesn't emit those on this code path, but
                                            // future quote-tick frames will land here too).
                                            let ac = AssetClass::from_symbol(&sym);
                                            let mut tfs = active_tfs_for(&sym);
                                            if tfs.is_empty() {
                                                tfs.push("1m".to_string());
                                            }
                                            for tf in &tfs {
                                                let agg_res = aggregator::aggregator()
                                                    .process_tick(&sym, tf, ts_ms, p as f64, v as f64);
                                                let live_bar = BarWire {
                                                    symbol: sym.clone(),
                                                    asset_class: ac,
                                                    timeframe: tf.clone(),
                                                    time: agg_res.current.bucket_start_ms,
                                                    open:   agg_res.current.open,
                                                    high:   agg_res.current.high,
                                                    low:    agg_res.current.low,
                                                    close:  agg_res.current.close,
                                                    volume: agg_res.current.volume,
                                                    vwap: 0.0,
                                                    trades: agg_res.current.tick_count as u64,
                                                    closed: false,
                                                };
                                                bar_hub_fanout(&sym, tf, live_bar);
                                                if let Some(prev) = agg_res.closed {
                                                    let closed_bar = BarWire {
                                                        symbol: sym.clone(),
                                                        asset_class: ac,
                                                        timeframe: tf.clone(),
                                                        time: prev.bucket_start_ms,
                                                        open:   prev.open,
                                                        high:   prev.high,
                                                        low:    prev.low,
                                                        close:  prev.close,
                                                        volume: prev.volume,
                                                        vwap: 0.0,
                                                        trades: prev.tick_count as u64,
                                                        closed: true,
                                                    };
                                                    bar_hub_fanout(&sym, tf, closed_bar);
                                                }
                                            }
                                            let trade = Trade {
                                                symbol: sym.clone(),
                                                asset_class: ac,
                                                price: p as f64,
                                                qty: v as f64,
                                                time: ts_ms,
                                            };
                                            hub_fanout(trade_hub(), &sym, trade);
                                            // Quote fanout only when bid/ask actually present.
                                            let bid = map.get("bid").and_then(|v| v.as_f64());
                                            let ask = map.get("ask").and_then(|v| v.as_f64());
                                            if bid.is_some() || ask.is_some() {
                                                let quote = Quote {
                                                    symbol: sym.clone(),
                                                    asset_class: ac,
                                                    bid: bid.unwrap_or(0.0),
                                                    ask: ask.unwrap_or(0.0),
                                                    bid_size: map.get("bid_size").or_else(|| map.get("bidSize"))
                                                        .and_then(|v| v.as_f64()).unwrap_or(0.0),
                                                    ask_size: map.get("ask_size").or_else(|| map.get("askSize"))
                                                        .and_then(|v| v.as_f64()).unwrap_or(0.0),
                                                    spread: 0.0,
                                                    time: ts_ms,
                                                };
                                                hub_fanout(quote_hub(), &sym, quote);
                                            }
                                            crate::send_to_native_chart(crate::chart_renderer::ChartCommand::UpdateLastBar {
                                                symbol: sym,
                                                timeframe: "5m".to_string(),
                                                bar: crate::chart_renderer::Bar {
                                                    open: p, high: p, low: p, close: p, volume: v, _pad: 0.0,
                                                },
                                                mark: false,
                                            });
                                        }
                                    }
                                    let _ = app.emit("ib-tick", val);
                                }
                            }
                            // Ping/pong/text — refresh liveness so the watchdog
                            // sees the pong from our own ping when no data flows.
                            Some(Ok(_)) => {
                                LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);
                            }
                            // Socket closed or error → report and reconnect
                            Some(Ok(Message::Close(_))) => {
                                report(ErrorLevel::Warn, "ib_ws", "ws_close", "close frame received");
                                break;
                            }
                            Some(Err(e)) => {
                                report(ErrorLevel::Warn, "ib_ws", "recv_error", e.to_string());
                                break;
                            }
                            None => {
                                report(ErrorLevel::Warn, "ib_ws", "stream_ended", "no more frames");
                                break;
                            }
                        },
                    }
                }

                if clean_shutdown {
                    return;
                }
                let _ = app.emit("ib-disconnected", ());
            }
            Err(e) => {
                report(ErrorLevel::Warn, "ib_ws", "connect_failed", e.to_string());
            }
        }

        if shutdown.load(Ordering::SeqCst) {
            publish_state(ConnectionState::ShuttingDown);
            return;
        }
        if let Some(d) = backoff.next_delay() {
            publish_state(ConnectionState::Backoff {
                until: std::time::Instant::now() + d,
                attempt: RECONNECT_COUNT.load(Ordering::Relaxed) + 1,
                reason: "disconnected".into(),
            });
            tokio::time::sleep(d).await;
        } else {
            publish_state(ConnectionState::Failed {
                reason: crate::data::connectivity::ConnectionError::MaxRetriesExceeded(
                    RECONNECT_COUNT.load(Ordering::Relaxed),
                ),
            });
        }
    }
}

// ── Wave 7E: tick-age watchdog ───────────────────────────────────────────────

async fn run_watchdog() {
    let mut tick = tokio::time::interval(Duration::from_secs(WATCHDOG_TICK_SECS));
    loop {
        tick.tick().await;
        let last = LAST_MESSAGE_AT_MS.load(Ordering::Relaxed);
        let now = now_ms();
        if last > 0 && now - last > STALE_TIMEOUT_MS && !FORCE_RECONNECT.load(Ordering::Relaxed) {
            LAST_STALL_AT_MS.store(now, Ordering::Relaxed);
            report(
                ErrorLevel::Warn,
                "ib_ws",
                "tick_stalled",
                format!("no frames for {}ms — forcing reconnect", now - last),
            );
            FORCE_RECONNECT.store(true, Ordering::SeqCst);
        }
    }
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Forward any WS message to ibserver. Also tracks subscribe/unsubscribe
/// conIds in `subscribed` so they can be restored after reconnect.
#[tauri::command]
pub async fn ib_ws_send(
    msg: Value,
    state: tauri::State<'_, IbWsHandle>,
) -> Result<(), crate::error::AppError> {
    use crate::error::AppError;
    // Wave 8c/9b: IB subscribes by numeric conId, not by (symbol, timeframe).
    // The resolver in `feeds::ib_ws::resolver` now provides bidirectional
    // conId ↔ symbol mapping (populated passively from tick frames in
    // ws_loop). No Rust-side subscribe callers exist today — all IB
    // subscribes flow through this Tauri command from the WebView, which
    // already knows both the conId and symbol, so SubscriptionManager
    // bumping happens at tick ingress (symbol-keyed) rather than at
    // subscribe time (conId-keyed). The resolver primarily benefits
    // future Rust-side subscribe paths (see `ib_subscribe_symbol` helper
    // sketched in the Wave 9b task notes).

    // Track subscription state for reconnect restoration
    if let Value::Object(ref map) = msg {
        let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(Value::Array(ids)) = map.get("conIds") {
            let ids: Vec<i64> = ids.iter().filter_map(|v| v.as_i64()).collect();
            let mut subs = state.subscribed.lock().await;
            match action {
                "subscribe" => {
                    subs.extend(ids.iter().copied());
                }
                "unsubscribe" => {
                    for id in &ids {
                        subs.remove(id);
                    }
                }
                "unsubscribe_all" => {
                    subs.clear();
                }
                _ => {}
            }
        }
    }

    let text = serde_json::to_string(&msg).map_err(AppError::from)?;
    state
        .tx
        .send(Cmd::Send(text))
        .await
        .map_err(|e| AppError::internal(e))
}

// ── Wave 12a: hub fanout tests ────────────────────────────────────────────────

#[cfg(test)]
mod hub_tests {
    use super::*;
    use std::time::Duration;

    fn mk_bar(sym: &str, price: f64) -> BarWire {
        BarWire {
            symbol: sym.to_string(),
            asset_class: AssetClass::Stock,
            timeframe: "1m".to_string(),
            time: 1_700_000_000_000,
            open: price, high: price, low: price, close: price,
            volume: 100.0, vwap: 0.0, trades: 0, closed: false,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bar_hub_fans_out_to_subscribers() {
        // Use a uniquely-named symbol so this test can't be poisoned by
        // sibling tests or by real tick traffic in a Tauri-launched harness.
        let sym = "TEST_FANOUT_AAPL";
        let mut rx1 = bar_hub_subscribe(sym, "1m");
        let mut rx2 = bar_hub_subscribe(sym, "1m");

        let bar = mk_bar(sym, 145.0);
        bar_hub_fanout(sym, "1m", bar.clone());

        let r1 = tokio::time::timeout(Duration::from_millis(50), rx1.recv())
            .await.expect("rx1 timed out").expect("rx1 closed");
        let r2 = tokio::time::timeout(Duration::from_millis(50), rx2.recv())
            .await.expect("rx2 timed out").expect("rx2 closed");
        assert_eq!(r1.close, bar.close);
        assert_eq!(r2.close, bar.close);
        assert_eq!(r1.symbol, sym);
        assert_eq!(r2.symbol, sym);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unsubscribe_drops_sender_count() {
        let sym = "TEST_FANOUT_MSFT";
        let rx = bar_hub_subscribe(sym, "1m");
        // Before any fanout, sender is parked in the hub.
        let key = (sym.to_string(), "1m".to_string());
        assert_eq!(bar_hub().lock().get(&key).map(|v| v.len()), Some(1));

        drop(rx);
        // Trigger a fanout — `retain` should prune the dead sender and the
        // empty entry is removed in the same pass.
        bar_hub_fanout(sym, "1m", mk_bar(sym, 300.0));
        assert!(bar_hub().lock().get(&key).is_none());
        // And the active-TF set entry is gone too.
        assert!(active_tfs().lock().get(sym).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bar_hub_segregates_by_timeframe() {
        // Subscribers on different TFs of the same symbol must NOT see
        // each other's bars — they live in separate (sym, tf) slots.
        let sym = "TEST_FANOUT_TFSEG";
        let mut rx1 = bar_hub_subscribe(sym, "1m");
        let mut rx5 = bar_hub_subscribe(sym, "5m");

        let mut b1 = mk_bar(sym, 100.0); b1.timeframe = "1m".into();
        let mut b5 = mk_bar(sym, 200.0); b5.timeframe = "5m".into();
        bar_hub_fanout(sym, "1m", b1.clone());
        bar_hub_fanout(sym, "5m", b5.clone());

        let r1 = tokio::time::timeout(Duration::from_millis(50), rx1.recv())
            .await.expect("rx1 timed out").expect("rx1 closed");
        let r5 = tokio::time::timeout(Duration::from_millis(50), rx5.recv())
            .await.expect("rx5 timed out").expect("rx5 closed");
        assert_eq!(r1.timeframe, "1m");
        assert_eq!(r1.close, 100.0);
        assert_eq!(r5.timeframe, "5m");
        assert_eq!(r5.close, 200.0);

        // No cross-talk: each receiver got exactly one frame.
        assert!(tokio::time::timeout(Duration::from_millis(20), rx1.recv()).await.is_err());
        assert!(tokio::time::timeout(Duration::from_millis(20), rx5.recv()).await.is_err());

        // active_tfs should know both TFs are live for this sym.
        let tfs = active_tfs_for(sym);
        assert!(tfs.contains(&"1m".to_string()));
        assert!(tfs.contains(&"5m".to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn quote_and_trade_hubs_independent() {
        let sym = "TEST_FANOUT_NVDA";
        let mut qrx = hub_subscribe(quote_hub(), sym);
        let mut trx = hub_subscribe(trade_hub(), sym);

        hub_fanout(trade_hub(), sym, Trade {
            symbol: sym.into(), asset_class: AssetClass::Stock,
            price: 800.0, qty: 10.0, time: 1,
        });
        hub_fanout(quote_hub(), sym, Quote {
            symbol: sym.into(), asset_class: AssetClass::Stock,
            bid: 799.5, ask: 800.5, bid_size: 1.0, ask_size: 2.0,
            spread: 1.0, time: 1,
        });

        let t = tokio::time::timeout(Duration::from_millis(50), trx.recv())
            .await.expect("trx timed out").expect("trx closed");
        let q = tokio::time::timeout(Duration::from_millis(50), qrx.recv())
            .await.expect("qrx timed out").expect("qrx closed");
        assert_eq!(t.price, 800.0);
        assert_eq!(q.bid, 799.5);
        assert_eq!(q.ask, 800.5);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn aggregator_drives_bar_hub_live_then_closed_on_boundary() {
        // Three ticks in minute 100 + one tick in minute 101.
        // Expect: 4 live bars (one per tick) + 1 closed bar (when the
        // 4th tick rolls into minute 101) on the bar_hub stream.
        let sym = "TEST_AGG_TSLA";
        let mut rx = bar_hub_subscribe(sym, "1m");

        // Drive the global singleton directly (mirrors ws_loop wiring).
        let agg = super::aggregator::aggregator();

        // ts_ms in minute 100 == 100 * 60_000 = 6_000_000
        let m100 = 100_i64 * 60_000;
        let m101 = 101_i64 * 60_000;

        let r1 = agg.process_tick(sym, "1m", m100 + 0,     100.0, 1.0);
        let r2 = agg.process_tick(sym, "1m", m100 + 1_000, 102.0, 2.0);
        let r3 = agg.process_tick(sym, "1m", m100 + 2_000,  99.0, 3.0);
        let r4 = agg.process_tick(sym, "1m", m101 + 500,   101.0, 4.0);

        // Mirror the ws_loop fanout for each ProcessResult.
        for r in [r1, r2, r3, r4] {
            let live = BarWire {
                symbol: sym.to_string(),
                asset_class: AssetClass::Stock,
                timeframe: "1m".into(),
                time: r.current.bucket_start_ms,
                open: r.current.open, high: r.current.high,
                low: r.current.low,   close: r.current.close,
                volume: r.current.volume,
                vwap: 0.0, trades: r.current.tick_count as u64, closed: false,
            };
            bar_hub_fanout(sym, "1m", live);
            if let Some(prev) = r.closed {
                let cb = BarWire {
                    symbol: sym.to_string(),
                    asset_class: AssetClass::Stock,
                    timeframe: "1m".into(),
                    time: prev.bucket_start_ms,
                    open: prev.open, high: prev.high,
                    low: prev.low,   close: prev.close,
                    volume: prev.volume,
                    vwap: 0.0, trades: prev.tick_count as u64, closed: true,
                };
                bar_hub_fanout(sym, "1m", cb);
            }
        }

        let mut live_seen = 0;
        let mut closed_seen = 0;
        let mut closed_bar: Option<BarWire> = None;
        for _ in 0..5 {
            let bw = tokio::time::timeout(Duration::from_millis(50), rx.recv())
                .await.expect("rx timed out").expect("rx closed");
            if bw.closed { closed_seen += 1; closed_bar = Some(bw); }
            else         { live_seen += 1; }
        }
        assert_eq!(live_seen,   4, "expected 4 live bars (one per tick)");
        assert_eq!(closed_seen, 1, "expected exactly 1 closed bar on minute rollover");

        let cb = closed_bar.expect("closed bar missing");
        assert_eq!(cb.time,   m100);
        assert_eq!(cb.open,   100.0);
        assert_eq!(cb.high,   102.0);
        assert_eq!(cb.low,     99.0);
        assert_eq!(cb.close,   99.0);
        assert_eq!(cb.volume,   6.0);
        assert_eq!(cb.trades,   3);
    }

    #[test]
    fn fanout_with_no_subscribers_is_a_noop() {
        // No panic, no hub entry created.
        bar_hub_fanout("TEST_FANOUT_GHOST", "1m", mk_bar("TEST_FANOUT_GHOST", 1.0));
        let key = ("TEST_FANOUT_GHOST".to_string(), "1m".to_string());
        assert!(bar_hub().lock().get(&key).is_none());
    }
}
