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

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tauri::{AppHandle, Emitter, async_runtime};
use tokio::{
    sync::{mpsc, Mutex},
    time::sleep,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const WS_URL: &str = "ws://127.0.0.1:5000/ws";
const RECONNECT_MS: u64 = 3_000;

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
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn spawn(app: AppHandle) -> IbWsHandle {
    let (tx, rx) = mpsc::channel::<Cmd>(512);
    let subscribed: Arc<Mutex<HashSet<i64>>> = Default::default();
    async_runtime::spawn(ws_loop(app, rx, subscribed.clone()));
    IbWsHandle { tx, subscribed }
}

// ── Background task ───────────────────────────────────────────────────────────

async fn ws_loop(
    app: AppHandle,
    mut rx: mpsc::Receiver<Cmd>,
    subscribed: Arc<Mutex<HashSet<i64>>>,
) {
    loop {
        match connect_async(WS_URL).await {
            Ok((stream, _)) => {
                let _ = app.emit("ib-connected", ());
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

                let mut clean_shutdown = false;
                loop {
                    tokio::select! {
                        biased; // check commands first so subscribe acks aren't delayed

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
                                if let Ok(val) = rmp_serde::from_slice::<Value>(&bytes) {
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
                            // Ignore ping/pong/text (ibserver doesn't send text)
                            Some(Ok(_)) => {}
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
                eprintln!("[ib_ws] connect failed: {e}");
            }
        }

        sleep(Duration::from_millis(RECONNECT_MS)).await;
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
