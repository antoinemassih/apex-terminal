//! ApexData WebSocket client — subscription manager + frame routing.
//!
//! Contract: §6 of `FRONTEND_INTEGRATION.md`.
//!
//! Design:
//! - One tokio runtime thread owns the WS connection.
//! - Callers interact through a synchronous API (`add_bar_sub`, `remove_bar_sub`,
//!   `set_tape`, `set_quotes`, `subscribe_to_frames`) — the manager diffs against
//!   current state and sends a single replace-set frame.
//! - Inbound frames are dispatched to registered listeners (crossbeam channels).
//! - Auto-reconnect with 2s backoff; re-sends subscription state on reopen.
//! - Encoding: MessagePack preferred; JSON fallback.

use super::config::{apex_ws_url, apex_token, is_enabled};
use super::types::*;

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio_tungstenite::{connect_async, client_async, MaybeTlsStream};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

// ────────────────────────────────────────────────────────────────────────────
// Global state
// ────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct SubState {
    bars: HashSet<String>,        // "SYM:TF" — last-source bar subs
    bars_mark: HashSet<String>,   // "SYM:TF" — mark-source bar subs (MARK_BARS_PROTOCOL)
    tape: HashSet<String>,
    quotes: HashSet<String>,
    chain: HashSet<String>,  // underlyings to stream chain_delta for
}

#[derive(Clone)]
pub enum Frame {
    Hello   { server: String, encoding: String },
    Bar     (BarUpdate),
    Snapshot{ subscription: String, bar: BarUpdate },
    Trade   (Trade),
    Quote   (Quote),
    Fmv     { symbol: String, fmv: f64, time_ms: i64 },
    ChainDelta(ChainDelta),
    Resync  { reason: String },
    Error   { code: String, message: String },
    /// Transport-level: WS connected or disconnected.
    Connection(bool),
}

type Listener = Arc<dyn Fn(&Frame) + Send + Sync>;

struct Manager {
    subs: Mutex<SubState>,
    listeners: Mutex<Vec<Listener>>,
    tx_ctrl: Mutex<Option<mpsc::UnboundedSender<CtrlMsg>>>,
    connected: Mutex<bool>,
}

enum CtrlMsg {
    PushSubs,
}

static MANAGER: OnceLock<Arc<Manager>> = OnceLock::new();
static RT:      OnceLock<Runtime>      = OnceLock::new();

fn manager() -> Arc<Manager> {
    MANAGER.get_or_init(|| {
        Arc::new(Manager {
            subs: Mutex::new(SubState::default()),
            listeners: Mutex::new(Vec::new()),
            tx_ctrl: Mutex::new(None),
            connected: Mutex::new(false),
        })
    }).clone()
}

fn runtime() -> &'static Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("apex-data-ws")
            .build()
            .expect("apex-data tokio runtime")
    })
}

// ────────────────────────────────────────────────────────────────────────────
// Wire frames
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OutMsg<'a> {
    #[serde(skip_serializing_if = "Option::is_none")] subscribe:      Option<&'a [String]>,
    /// MARK_BARS_PROTOCOL §"WebSocket — Subscribe frame": parallel array for
    /// NBBO-mid bars. Atomic-replace semantics — both arrays sent every push.
    #[serde(skip_serializing_if = "Option::is_none")] subscribe_mark: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")] tape:           Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")] quotes:         Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")] chain:          Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")] format:         Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct InEnvelope {
    #[allow(dead_code)]
    #[serde(default)] v: u32,
    #[serde(rename = "type")] kind: String,
    #[serde(default)] data: serde_json::Value,
}

// ────────────────────────────────────────────────────────────────────────────
// Public API
// ────────────────────────────────────────────────────────────────────────────

/// Boot the client once at startup. Safe to call multiple times — first wins.
pub fn start() {
    if !is_enabled() { eprintln!("[apex_data.ws] disabled, not starting"); return; }
    let mgr = manager();
    // If a control channel already exists, we've already started.
    if mgr.tx_ctrl.lock().map(|g| g.is_some()).unwrap_or(false) { return; }

    let (tx_ctrl, rx_ctrl) = mpsc::unbounded_channel::<CtrlMsg>();
    if let Ok(mut slot) = mgr.tx_ctrl.lock() { *slot = Some(tx_ctrl); }

    let mgr_bg = mgr.clone();
    runtime().spawn(async move { run_connection_loop(mgr_bg, rx_ctrl).await; });
}

pub fn is_connected() -> bool {
    MANAGER.get().and_then(|m| m.connected.lock().ok().map(|g| *g)).unwrap_or(false)
}

/// Register a frame listener. Call **once** during startup; fans out every frame.
pub fn subscribe_to_frames<F: Fn(&Frame) + Send + Sync + 'static>(f: F) {
    let mgr = manager();
    if let Ok(mut ls) = mgr.listeners.lock() { ls.push(Arc::new(f)); };
}

pub fn add_bar_sub(symbol: &str, tf: &str) {
    with_subs(|s| { s.bars.insert(format!("{symbol}:{tf}")); });
    push_subs();
}
pub fn remove_bar_sub(symbol: &str, tf: &str) {
    with_subs(|s| { s.bars.remove(&format!("{symbol}:{tf}")); });
    push_subs();
}

/// Mark-source equivalent (MARK_BARS_PROTOCOL §WebSocket). Adds to the
/// parallel `subscribe_mark` array — independent of last-source subs.
pub fn add_mark_bar_sub(symbol: &str, tf: &str) {
    with_subs(|s| { s.bars_mark.insert(format!("{symbol}:{tf}")); });
    push_subs();
}
pub fn remove_mark_bar_sub(symbol: &str, tf: &str) {
    with_subs(|s| { s.bars_mark.remove(&format!("{symbol}:{tf}")); });
    push_subs();
}

pub fn set_tape(symbols: &[String]) {
    with_subs(|s| { s.tape = symbols.iter().cloned().collect(); });
    push_subs();
}
pub fn set_quotes(symbols: &[String]) {
    with_subs(|s| { s.quotes = symbols.iter().cloned().collect(); });
    push_subs();
}
/// Replace the chain-delta underlying set. `"*"` as sole entry = wildcard.
pub fn set_chain(underlyings: &[String]) {
    with_subs(|s| { s.chain = underlyings.iter().cloned().collect(); });
    push_subs();
}

fn with_subs(f: impl FnOnce(&mut SubState)) {
    if let Ok(mut g) = manager().subs.lock() { f(&mut g); }
}

fn push_subs() {
    if let Some(tx) = manager().tx_ctrl.lock().ok().and_then(|g| g.clone()) {
        let _ = tx.send(CtrlMsg::PushSubs);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Connection loop
// ────────────────────────────────────────────────────────────────────────────

async fn run_connection_loop(mgr: Arc<Manager>, mut rx_ctrl: mpsc::UnboundedReceiver<CtrlMsg>) {
    loop {
        let url = format!("{}?format=msgpack{}", apex_ws_url(),
            apex_token().map(|t| format!("&token={t}")).unwrap_or_default());
        eprintln!("[apex_data.ws] connecting → {url}");

        let req = match url.into_client_request() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[apex_data.ws] bad url: {e}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // LAN-IP override: when set, connect TCP directly to the homelab
        // Traefik IP and hand the socket to `client_async` with the original
        // Host-bearing request. Keeps Traefik ingress routing intact.
        let lan_override = match (super::config::apex_lan_ip(), super::config::apex_host_port()) {
            (Some(ip), Some((_host, port))) => Some((ip, port)),
            _ => None,
        };

        let conn_result = if let Some((ip, port)) = lan_override.as_ref() {
            eprintln!("[apex_data.ws] LAN override: dial {ip}:{port}");
            match TcpStream::connect((ip.as_str(), *port)).await {
                Ok(stream) => match client_async(req, MaybeTlsStream::Plain(stream)).await {
                    Ok((ws, _)) => Ok(ws),
                    Err(e) => Err(format!("handshake: {e}")),
                },
                Err(e) => Err(format!("tcp connect {ip}:{port}: {e}")),
            }
        } else {
            match connect_async(req).await {
                Ok((ws, _)) => Ok(ws),
                Err(e) => Err(format!("{e}")),
            }
        };

        let ws = match conn_result {
            Ok(ws) => ws,
            Err(e) => {
                eprintln!("[apex_data.ws] connect failed: {e}");
                broadcast(&mgr, &Frame::Connection(false));
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };
        eprintln!("[apex_data.ws] connected");
        if let Ok(mut c) = mgr.connected.lock() { *c = true; }
        broadcast(&mgr, &Frame::Connection(true));

        let (mut tx_ws, mut rx_ws) = ws.split();

        // Send initial subs
        if let Some(frame) = build_subs_frame(&mgr) {
            let _ = tx_ws.send(Message::Text(frame)).await;
        }

        // Main loop: pump inbound + handle control
        loop {
            tokio::select! {
                msg = rx_ws.next() => {
                    match msg {
                        Some(Ok(Message::Binary(bytes))) => handle_binary(&mgr, &bytes),
                        Some(Ok(Message::Text(text)))   => handle_text(&mgr, &text),
                        Some(Ok(Message::Close(_))) => { eprintln!("[apex_data.ws] close"); break; }
                        Some(Ok(_))  => {}
                        Some(Err(e)) => { eprintln!("[apex_data.ws] recv error: {e}"); break; }
                        None         => { eprintln!("[apex_data.ws] stream ended"); break; }
                    }
                }
                ctrl = rx_ctrl.recv() => {
                    match ctrl {
                        Some(CtrlMsg::PushSubs) => {
                            if let Some(frame) = build_subs_frame(&mgr) {
                                if let Err(e) = tx_ws.send(Message::Text(frame)).await {
                                    eprintln!("[apex_data.ws] send failed: {e}");
                                    break;
                                }
                            }
                        }
                        None => break,
                    }
                }
            }
        }

        if let Ok(mut c) = mgr.connected.lock() { *c = false; }
        broadcast(&mgr, &Frame::Connection(false));
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn build_subs_frame(mgr: &Arc<Manager>) -> Option<String> {
    let s = mgr.subs.lock().ok()?.clone();
    let bars: Vec<String>      = s.bars.iter().cloned().collect();
    let bars_mark: Vec<String> = s.bars_mark.iter().cloned().collect();
    let tape: Vec<String>      = s.tape.iter().cloned().collect();
    let quotes: Vec<String>    = s.quotes.iter().cloned().collect();
    let chain: Vec<String>     = s.chain.iter().cloned().collect();
    // MARK_BARS_PROTOCOL: always send BOTH arrays atomically — server treats
    // a missing array as empty.
    let msg = OutMsg {
        subscribe:      Some(&bars),
        subscribe_mark: Some(&bars_mark),
        tape:           Some(&tape),
        quotes:         Some(&quotes),
        chain:          Some(&chain),
        format:         None,
    };
    serde_json::to_string(&msg).ok()
}

fn broadcast(mgr: &Arc<Manager>, f: &Frame) {
    let ls = match mgr.listeners.lock() { Ok(g) => g.clone(), Err(_) => return };
    for l in ls.iter() { l(f); }
}

fn handle_text(mgr: &Arc<Manager>, text: &str) {
    match serde_json::from_str::<InEnvelope>(text) {
        Ok(env) => dispatch(mgr, env),
        Err(e) => eprintln!("[apex_data.ws] bad json frame: {e}"),
    }
}

fn handle_binary(mgr: &Arc<Manager>, bytes: &[u8]) {
    match rmp_serde::from_slice::<InEnvelope>(bytes) {
        Ok(env) => dispatch(mgr, env),
        Err(e) => {
            // Some servers send text-over-binary; try JSON
            if let Ok(text) = std::str::from_utf8(bytes) {
                if let Ok(env) = serde_json::from_str::<InEnvelope>(text) {
                    dispatch(mgr, env); return;
                }
            }
            eprintln!("[apex_data.ws] bad msgpack frame: {e}");
        }
    }
}

fn dispatch(mgr: &Arc<Manager>, env: InEnvelope) {
    let frame = match env.kind.as_str() {
        "hello" => {
            let server   = env.data.get("server").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let encoding = env.data.get("encoding").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Frame::Hello { server, encoding }
        }
        "bar" => {
            // MARK_BARS_PROTOCOL: `source` lives on the envelope alongside
            // `bar`. Default to "last" for back-compat with servers that
            // don't emit it yet.
            let src = env.data.get("source").and_then(|v| v.as_str()).unwrap_or("last").to_string();
            match serde_json::from_value::<BarUpdate>(env.data) {
                Ok(mut b) => { b.source = src; Frame::Bar(b) }
                Err(e) => { eprintln!("[apex_data.ws] bad bar: {e}"); return; }
            }
        }
        "snapshot" => {
            let sub = env.data.get("subscription").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let src = env.data.get("source").and_then(|v| v.as_str()).unwrap_or("last").to_string();
            let bar = env.data.get("bar").cloned().unwrap_or(serde_json::Value::Null);
            match serde_json::from_value::<BarUpdate>(bar) {
                Ok(mut b) => { b.source = src; Frame::Snapshot { subscription: sub, bar: b } }
                Err(e) => { eprintln!("[apex_data.ws] bad snapshot: {e}"); return; }
            }
        }
        "trade" => match serde_json::from_value::<Trade>(env.data) {
            Ok(t) => Frame::Trade(t),
            Err(e) => { eprintln!("[apex_data.ws] bad trade: {e}"); return; }
        },
        "quote" => match serde_json::from_value::<Quote>(env.data) {
            Ok(q) => Frame::Quote(q),
            Err(e) => { eprintln!("[apex_data.ws] bad quote: {e}"); return; }
        },
        "fmv" => {
            let symbol = env.data.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let fmv = env.data.get("fmv").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let time_ms = env.data.get("time_ms").and_then(|v| v.as_i64()).unwrap_or(0);
            Frame::Fmv { symbol, fmv, time_ms }
        }
        "chain_delta" => match serde_json::from_value::<ChainDelta>(env.data) {
            Ok(d) => Frame::ChainDelta(d),
            Err(e) => { eprintln!("[apex_data.ws] bad chain_delta: {e}"); return; }
        },
        "resync" => {
            let reason = env.data.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Frame::Resync { reason }
        }
        "error" => {
            let code    = env.data.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let message = env.data.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Frame::Error { code, message }
        }
        other => { eprintln!("[apex_data.ws] unknown frame: {other}"); return; }
    };
    broadcast(mgr, &frame);
}
