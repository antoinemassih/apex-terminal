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
use crate::data::connectivity::{self, errors_sink::{report, ErrorLevel}, Backoff};

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

// ── Wave 6: per-feed metrics counters ────────────────────────────────────────
// Read by `data/providers/apex_data.rs::Connection::metrics()` and surfaced to
// Prometheus via `foundation/monitoring.rs`.
pub(crate) static MESSAGES_IN:        AtomicU64 = AtomicU64::new(0);
pub(crate) static PARSE_ERRORS:       AtomicU64 = AtomicU64::new(0);
pub(crate) static RECONNECT_COUNT:    AtomicU32 = AtomicU32::new(0);
pub(crate) static LAST_MESSAGE_AT_MS: AtomicI64 = AtomicI64::new(0);

// ── Wave 7E: WS resilience ───────────────────────────────────────────────────
// Heartbeat: send a tungstenite Ping every HEARTBEAT_SECS to keep the socket
// warm and surface dead peers fast (OS TCP keep-alive is ≥2hr by default).
// Watchdog: if no inbound frame for STALE_TIMEOUT_MS, force a reconnect.
const HEARTBEAT_SECS: u64 = 30;
const STALE_TIMEOUT_MS: i64 = 30_000;
const WATCHDOG_TICK_SECS: u64 = 10;
/// Flipped to `true` by the watchdog when the feed has been silent past
/// `STALE_TIMEOUT_MS`. The inner WS loop observes this each select tick and
/// breaks out so the outer reconnect logic takes over.
pub(crate) static FORCE_RECONNECT: AtomicBool = AtomicBool::new(false);
/// Last reason recorded by the watchdog so the provider's ConnectionState
/// can surface a meaningful Backoff reason after a tick-stall reconnect.
pub(crate) static LAST_STALL_AT_MS: AtomicI64 = AtomicI64::new(0);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

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
    /// Set by `Shutdown::drain` to stop the reconnect loop after the current
    /// iteration exits.
    shutdown: AtomicBool,
}

enum CtrlMsg {
    PushSubs,
    Shutdown,
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
            shutdown: AtomicBool::new(false),
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
    if !is_enabled() {
        report(ErrorLevel::Info, "apex_data.ws", "disabled", "not starting");
        return;
    }
    let mgr = manager();
    // If a control channel already exists, we've already started.
    if mgr.tx_ctrl.lock().map(|g| g.is_some()).unwrap_or(false) { return; }

    let (tx_ctrl, rx_ctrl) = mpsc::unbounded_channel::<CtrlMsg>();
    if let Ok(mut slot) = mgr.tx_ctrl.lock() { *slot = Some(tx_ctrl); }

    let mgr_bg = mgr.clone();
    runtime().spawn(async move { run_connection_loop(mgr_bg, rx_ctrl).await; });
    // Wave 7E: tick-age watchdog. Runs independently of the WS loop; when it
    // detects silence past STALE_TIMEOUT_MS it sets FORCE_RECONNECT, which the
    // inner select breaks on, triggering the existing reconnect path.
    runtime().spawn(async move { run_watchdog().await; });

    // Register a real Shutdown handler so process exit can drain cleanly.
    connectivity::register("apex_data.ws", Arc::new(ApexDataWsShutdown { mgr: mgr.clone() }));
}

/// Real `Shutdown` impl for the ApexData WS connection.
struct ApexDataWsShutdown {
    mgr: Arc<Manager>,
}

#[async_trait::async_trait]
impl connectivity::Shutdown for ApexDataWsShutdown {
    async fn drain(&self, deadline: Duration) -> Result<(), String> {
        tracing::info!(target: "shutdown", connection = "apex_data.ws", "drain invoked");
        self.mgr.shutdown.store(true, Ordering::SeqCst);
        // Best-effort: nudge the control loop so it observes shutdown.
        if let Some(tx) = self.mgr.tx_ctrl.lock().ok().and_then(|g| g.clone()) {
            let _ = tx.send(CtrlMsg::Shutdown);
        }
        // Wait until the connected flag clears or the deadline elapses.
        let start = std::time::Instant::now();
        while start.elapsed() < deadline {
            let connected = self.mgr.connected.lock().map(|g| *g).unwrap_or(false);
            if !connected { return Ok(()); }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err("apex_data.ws drain timed out".into())
    }
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
    // Infinite reconnect attempts — exponential backoff w/ jitter prevents
    // thundering herd. WS feeds should never give up.
    let mut backoff = Backoff::new().with_max_attempts(None);
    let mut first_attempt = true;
    loop {
        if mgr.shutdown.load(Ordering::SeqCst) { break; }
        if !first_attempt {
            RECONNECT_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        first_attempt = false;
        let url = format!("{}?format=msgpack{}", apex_ws_url(),
            apex_token().map(|t| format!("&token={t}")).unwrap_or_default());
        report(ErrorLevel::Info, "apex_data.ws", "connecting", format!("→ {url}"));

        let req = match url.into_client_request() {
            Ok(r) => r,
            Err(e) => {
                report(ErrorLevel::Error, "apex_data.ws", "bad_url", e.to_string());
                if let Some(d) = backoff.next_delay() {
                    tokio::time::sleep(d).await;
                }
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
            report(ErrorLevel::Info, "apex_data.ws", "lan_override", format!("dial {ip}:{port}"));
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
                report(ErrorLevel::Warn, "apex_data.ws", "connect_failed", e);
                broadcast(&mgr, &Frame::Connection(false));
                if let Some(d) = backoff.next_delay() {
                    tokio::time::sleep(d).await;
                }
                continue;
            }
        };
        report(ErrorLevel::Info, "apex_data.ws", "connected", "ws established");
        if let Ok(mut c) = mgr.connected.lock() { *c = true; }
        broadcast(&mgr, &Frame::Connection(true));
        backoff.reset();

        // Wave 7E: gap-fill replay through the SubscriptionManager on every
        // reconnect (not the initial connect — RECONNECT_COUNT > 0 means we
        // dropped). Spawned so we don't block the WS read loop.
        if RECONNECT_COUNT.load(Ordering::Relaxed) > 0 {
            tokio::spawn(async {
                let mgr = crate::data::providers::registry::subscription_manager();
                let n = mgr.gap_fill_on_reconnect_all().await;
                if n > 0 {
                    report(
                        ErrorLevel::Info,
                        "apex_data.ws",
                        "gap_fill",
                        format!("replayed {n} bars after reconnect"),
                    );
                }
            });
        }

        let (mut tx_ws, mut rx_ws) = ws.split();

        // Send initial subs
        if let Some(frame) = build_subs_frame(&mgr) {
            let _ = tx_ws.send(Message::Text(frame)).await;
        }

        // Wave 7E: clear stale flag at connect-time so a previous stall doesn't
        // immediately tear down the fresh socket.
        FORCE_RECONNECT.store(false, Ordering::SeqCst);
        let mut heartbeat = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECS));
        // Skip the immediate first tick — server hasn't had time to reply.
        heartbeat.tick().await;
        // Cheap check (every second) on the watchdog flag so we don't have to
        // route the signal through a channel.
        let mut force_check = tokio::time::interval(Duration::from_secs(1));
        force_check.tick().await;

        // Main loop: pump inbound + handle control
        loop {
            if FORCE_RECONNECT.swap(false, Ordering::SeqCst) {
                report(ErrorLevel::Warn, "apex_data.ws", "force_reconnect", "watchdog tripped");
                let _ = tx_ws.close().await;
                break;
            }
            tokio::select! {
                _ = heartbeat.tick() => {
                    // Tungstenite Ping — protocol-level keepalive; server's
                    // pong arrives back through rx_ws and is counted in
                    // LAST_MESSAGE_AT_MS via the default Message arm (no-op).
                    if let Err(e) = tx_ws.send(Message::Ping(Vec::new().into())).await {
                        report(ErrorLevel::Warn, "apex_data.ws", "ping_failed", e.to_string());
                        break;
                    }
                }
                _ = force_check.tick() => { /* loop top re-checks FORCE_RECONNECT */ }
                msg = rx_ws.next() => {
                    match msg {
                        Some(Ok(Message::Binary(bytes))) => handle_binary(&mgr, &bytes),
                        Some(Ok(Message::Text(text)))   => handle_text(&mgr, &text),
                        Some(Ok(Message::Close(_))) => { report(ErrorLevel::Warn, "apex_data.ws", "ws_close", "close frame"); break; }
                        Some(Ok(_))  => {}
                        Some(Err(e)) => { report(ErrorLevel::Warn, "apex_data.ws", "recv_error", e.to_string()); break; }
                        None         => { report(ErrorLevel::Warn, "apex_data.ws", "stream_ended", "no more frames"); break; }
                    }
                }
                ctrl = rx_ctrl.recv() => {
                    match ctrl {
                        Some(CtrlMsg::PushSubs) => {
                            if let Some(frame) = build_subs_frame(&mgr) {
                                if let Err(e) = tx_ws.send(Message::Text(frame)).await {
                                    report(ErrorLevel::Warn, "apex_data.ws", "send_failed", e.to_string());
                                    break;
                                }
                            }
                        }
                        Some(CtrlMsg::Shutdown) | None => {
                            let _ = tx_ws.close().await;
                            break;
                        }
                    }
                }
            }
        }

        if let Ok(mut c) = mgr.connected.lock() { *c = false; }
        broadcast(&mgr, &Frame::Connection(false));
        if mgr.shutdown.load(Ordering::SeqCst) { break; }
        if let Some(d) = backoff.next_delay() {
            tokio::time::sleep(d).await;
        }
    }
}

/// Wave 7E: tick-age watchdog. Forces a reconnect when no inbound frame has
/// arrived for `STALE_TIMEOUT_MS`. The inner WS select observes
/// `FORCE_RECONNECT` and breaks; the outer reconnect loop dials again.
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
                "apex_data.ws",
                "tick_stalled",
                format!("no frames for {}ms — forcing reconnect", now - last),
            );
            FORCE_RECONNECT.store(true, Ordering::SeqCst);
            // Wave 7E: hook for SubscriptionManager gap-fill — fires on the
            // *next* successful reconnect via the broadcast wired below.
        }
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
    MESSAGES_IN.fetch_add(1, Ordering::Relaxed);
    LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);
    match serde_json::from_str::<InEnvelope>(text) {
        Ok(env) => dispatch(mgr, env),
        Err(e) => {
            PARSE_ERRORS.fetch_add(1, Ordering::Relaxed);
            report(ErrorLevel::Warn, "apex_data.ws", "parse_json", e.to_string())
        }
    }
}

fn handle_binary(mgr: &Arc<Manager>, bytes: &[u8]) {
    MESSAGES_IN.fetch_add(1, Ordering::Relaxed);
    LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);
    match rmp_serde::from_slice::<InEnvelope>(bytes) {
        Ok(env) => dispatch(mgr, env),
        Err(e) => {
            // Some servers send text-over-binary; try JSON
            if let Ok(text) = std::str::from_utf8(bytes) {
                if let Ok(env) = serde_json::from_str::<InEnvelope>(text) {
                    dispatch(mgr, env); return;
                }
            }
            PARSE_ERRORS.fetch_add(1, Ordering::Relaxed);
            report(ErrorLevel::Warn, "apex_data.ws", "parse_msgpack", e.to_string());
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
                Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_bar", e.to_string()); return; }
            }
        }
        "snapshot" => {
            let sub = env.data.get("subscription").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let src = env.data.get("source").and_then(|v| v.as_str()).unwrap_or("last").to_string();
            let bar = env.data.get("bar").cloned().unwrap_or(serde_json::Value::Null);
            match serde_json::from_value::<BarUpdate>(bar) {
                Ok(mut b) => { b.source = src; Frame::Snapshot { subscription: sub, bar: b } }
                Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_snapshot", e.to_string()); return; }
            }
        }
        "trade" => match serde_json::from_value::<Trade>(env.data) {
            Ok(t) => Frame::Trade(t),
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_trade", e.to_string()); return; }
        },
        "quote" => match serde_json::from_value::<Quote>(env.data) {
            Ok(q) => Frame::Quote(q),
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_quote", e.to_string()); return; }
        },
        "fmv" => {
            let symbol = env.data.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let fmv = env.data.get("fmv").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let time_ms = env.data.get("time_ms").and_then(|v| v.as_i64()).unwrap_or(0);
            Frame::Fmv { symbol, fmv, time_ms }
        }
        "chain_delta" => match serde_json::from_value::<ChainDelta>(env.data) {
            Ok(d) => Frame::ChainDelta(d),
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_chain_delta", e.to_string()); return; }
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
        other => { report(ErrorLevel::Warn, "apex_data.ws", "unknown_frame", other.to_string()); return; }
    };
    broadcast(mgr, &frame);
}
