//! ib_ws — Rust-native IB WebSocket client (hot path)
//!
//! Replaces the TypeScript WebSocket in IBKRProvider.
//! Connects to ibserver ws://127.0.0.1:5000/ws, decodes MessagePack binary
//! frames in Rust, and emits `ib-tick` Tauri events to the React frontend
//! via direct IPC (no TCP round-trip on the hot path).
//!
//! Control messages (subscribe/unsubscribe) sent as JSON text — ibserver
//! receive side stays unchanged.

use std::{collections::HashSet, sync::Arc, time::Duration};
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64};

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
        if shutdown.load(Ordering::SeqCst) { return; }
        if !first { RECONNECT_COUNT.fetch_add(1, Ordering::Relaxed); }
        first = false;
        match connect_async(WS_URL).await {
            Ok((stream, _)) => {
                backoff.reset();
                let _ = app.emit("ib-connected", ());
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
                                            crate::send_to_native_chart(crate::chart_renderer::ChartCommand::UpdateLastBar {
                                                symbol: map.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string(),
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
                            // Socket closed or error → reconnect
                            _ => break,
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

        if shutdown.load(Ordering::SeqCst) { return; }
        if let Some(d) = backoff.next_delay() {
            tokio::time::sleep(d).await;
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
) -> Result<(), String> {
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

    let text = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
    state
        .tx
        .send(Cmd::Send(text))
        .await
        .map_err(|e| e.to_string())
}
