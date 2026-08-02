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
use crate::data::connectivity::{self, errors_sink::{report, ErrorLevel}, Backoff, ConnectionState};

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

// ── Wave 6: per-feed metrics counters ────────────────────────────────────────
// Read by `data/providers/apex_data.rs::Connection::metrics()` and surfaced to
// Prometheus via `foundation/monitoring.rs`.
pub(crate) static MESSAGES_IN:        AtomicU64 = AtomicU64::new(0);
pub(crate) static PARSE_ERRORS:       AtomicU64 = AtomicU64::new(0);
/// Subscription frames actually written to the socket.
pub(crate) static SUBS_PUSH_SENT:       AtomicU64 = AtomicU64::new(0);
/// Identical subscription pushes suppressed before hitting the wire.
///
/// Expected to be LARGE and to dwarf `SUBS_PUSH_SENT` — `set_quotes` is driven
/// from the per-frame render loop, so this counts the ~60/s no-op pushes that
/// used to reach ApexData and write-lock its fanout index for every client.
pub(crate) static SUBS_PUSH_SUPPRESSED: AtomicU64 = AtomicU64::new(0);
pub(crate) static RECONNECT_COUNT:    AtomicU32 = AtomicU32::new(0);
pub(crate) static LAST_MESSAGE_AT_MS: AtomicI64 = AtomicI64::new(0);

/// Last time a DATA frame (binary/text) arrived, as opposed to any frame.
///
/// AUDIT 2026-08-02 (AT-011): the watchdog used to reconnect on
/// `LAST_MESSAGE_AT_MS` alone — but that atomic is only written by
/// `handle_binary`/`handle_text`, never by the pong arm, despite the comment at
/// the heartbeat claiming pongs are "counted in LAST_MESSAGE_AT_MS via the
/// default Message arm". They are not: that arm is `Some(Ok(_)) => {}`.
///
/// So a socket that was perfectly healthy but simply quiet — a thin symbol, or
/// any time outside RTH — was force-reconnected every 30s, forever.
///
/// Splitting the two signals is the fix, and it is not merely cosmetic:
/// reconnecting cannot cure "socket alive, no data". That condition means the
/// subscription or the entitlement is wrong upstream, and churning the
/// connection just hides it behind a reconnect loop. Socket death forces a
/// reconnect; data starvation is reported and left alone.
pub(crate) static LAST_DATA_AT_MS: AtomicI64 = AtomicI64::new(0);

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
/// Timestamp (ms) of the last stale-feed toast emitted. Used to suppress
/// duplicate toasts during a sustained outage (60s cooldown).
static LAST_STALL_TOAST_AT: AtomicI64 = AtomicI64::new(0);
/// Minimum gap between consecutive stale-feed toasts (60 seconds).
const STALL_TOAST_COOLDOWN_MS: i64 = 60_000;

// ── Wave 11c: ConnectionState push-notification stream ───────────────────────
// Module-level broadcast channel — `ApexDataProvider::subscribe_state()` and
// every diagnostic consumer hold receivers. Capacity 64 gives slow subscribers
// some slack before they observe `RecvError::Lagged`. Send is best-effort —
// failures (no receivers) are silently ignored.
static STATE_TX: OnceLock<tokio::sync::broadcast::Sender<ConnectionState>> = OnceLock::new();

pub fn state_tx() -> &'static tokio::sync::broadcast::Sender<ConnectionState> {
    STATE_TX.get_or_init(|| tokio::sync::broadcast::channel(64).0)
}

/// Publish a state transition to all current subscribers. Call from any
/// state-mutation site in the WS loop. Send failure (no subscribers) is
/// silently swallowed — broadcast channels keep working after every receiver
/// drops.
pub(crate) fn publish_state(s: ConnectionState) {
    let _ = state_tx().send(s);
}

fn now_ms() -> i64 {
    crate::foundation::time::now_ms()
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
    /// SOTA §4.4 — calibrated trade plan v2.
    TradePlan(TradePlanV2),
    /// SOTA §4.5 — spike-explanation toast payload.
    Spike(SpikeExplanation),
    /// Wave 10 — LULD trading halt / resume / near-band warning.
    Halt    (HaltReading),
    Resync  { reason: String },
    Error   { code: String, message: String },
    /// Transport-level: WS connected or disconnected.
    Connection(bool),
    // ── SOTA UX §3.3 frames (Agent A) ──────────────────────────────────────
    /// `Signal::Combined` — per-symbol composite signal with calibrated
    /// contributors and provenance. Drives SignalsPanel + ProvenancePane.
    Combined(CombinedSignalV2),
    /// `Signal::Regime` — 4-axis classification published every 5min +
    /// on axis change. Drives the always-visible RegimeTape top strip.
    Regime(RegimeFrame),
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
    /// The last subscription frame actually written to the socket.
    ///
    /// Exists to make re-sending an identical set impossible. `set_quotes` is
    /// driven from the per-frame render loop, so without this the terminal
    /// pushed a full `{quotes:[...]}` frame ~60x/second. ApexData takes a WRITE
    /// LOCK on its fanout index per subscription frame, so that starves
    /// dispatch for EVERY client on the account, not just this one — it is the
    /// single most damaging thing a client can do to the shared feed.
    ///
    /// Cleared on disconnect so the first push after a reconnect always goes
    /// out (the server has no memory of what we wanted).
    last_sent_subs: Mutex<Option<String>>,
}

enum CtrlMsg {
    /// `force` bypasses the identical-frame suppression. Required after a
    /// `resync` or a reconnect: the server has dropped its subscription state
    /// and is waiting for us to re-declare, so "unchanged" must still be sent.
    PushSubs { force: bool },
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
            last_sent_subs: Mutex::new(None),
        })
    }).clone()
}

pub(crate) fn runtime() -> &'static Runtime {
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
/// How many quote SEATS we are currently asking ApexData for.
///
/// The number that matters for shared-pool etiquette: the account has ~1000
/// subscription strings with half reserved, so this is the client's draw on a
/// resource every other client shares.
pub fn quotes_requested() -> usize {
    manager().subs.lock().map(|g| g.quotes.len()).unwrap_or(0)
}

/// Of those seats, how many are OPTION contracts (OCC "O:" prefix).
///
/// Split out because stocks and options draw on the same request but behave
/// completely differently: a handful of stock symbols is unavoidable and cheap,
/// while options are where a client can quietly consume the whole shared pool.
/// Reporting only the total hid that distinction — the first measurement after
/// capping the strike seats read HIGHER than before, purely because stock
/// symbols dominate the number.
pub fn option_quotes_requested() -> usize {
    manager().subs.lock()
        .map(|g| g.quotes.iter().filter(|s| s.starts_with("O:")).count())
        .unwrap_or(0)
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

/// Re-send the full current subscription set to the server. Call this when the
/// server emits a `resync` frame (it has dropped/rebuilt its subscription state
/// and expects clients to re-declare what they want). Cheap — just replays the
/// existing `SubState`, no local subscription is lost.
pub fn resubscribe() {
    push_subs_forced();
}

fn with_subs(f: impl FnOnce(&mut SubState)) {
    if let Ok(mut g) = manager().subs.lock() { f(&mut g); }
}

/// Per-candidate TCP connect budget when walking LAN nodes.
///
/// A node that is down but not actively refusing (firewalled, or mid-reboot)
/// black-holes the SYN and the connect hangs until the OS gives up — which is
/// tens of seconds. With a whole candidate list that would be worse than the
/// single-IP behaviour it replaces, so each attempt is capped.
const LAN_DIAL_TIMEOUT: Duration = Duration::from_millis(1500);

/// Connect to the first reachable LAN candidate, in preference order.
///
/// The WS path cannot use reqwest's `resolve_to_addrs` (it dials a raw TCP
/// socket and hands it to `client_async` with the original Host-bearing
/// request, to keep Traefik ingress routing intact), so failover is explicit
/// here. Same rationale as the REST side: losing one K3s node must cost a
/// reconnect, not drop the terminal onto synthesized option prices.
pub(crate) async fn dial_lan_candidates(port: u16) -> Result<TcpStream, String> {
    let ips = super::config::apex_lan_ips();
    if ips.is_empty() {
        return Err("no LAN candidates configured".into());
    }
    let mut errors: Vec<String> = Vec::new();
    for ip in &ips {
        match tokio::time::timeout(LAN_DIAL_TIMEOUT, TcpStream::connect((ip.as_str(), port))).await {
            Ok(Ok(stream)) => {
                if !errors.is_empty() {
                    // Worth a line: a silent failover hides a node that is down.
                    report(ErrorLevel::Warn, "apex_data.ws", "lan_failover",
                        format!("connected {ip}:{port} after {} failed candidate(s): {}",
                            errors.len(), errors.join("; ")));
                }
                return Ok(stream);
            }
            Ok(Err(e)) => errors.push(format!("{ip}: {e}")),
            Err(_)     => errors.push(format!("{ip}: timeout after {}ms", LAN_DIAL_TIMEOUT.as_millis())),
        }
    }
    Err(format!("all {} LAN candidate(s) failed: {}", ips.len(), errors.join("; ")))
}

fn push_subs() { push_subs_inner(false) }

/// Push even if the set is unchanged — for resync/reconnect, where the server
/// has forgotten what we asked for.
fn push_subs_forced() { push_subs_inner(true) }

fn push_subs_inner(force: bool) {
    if let Some(tx) = manager().tx_ctrl.lock().ok().and_then(|g| g.clone()) {
        let _ = tx.send(CtrlMsg::PushSubs { force });
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
        // Token goes in the Authorization header, NOT the URL query — a `?token=`
        // leaks the secret into proxy / server / app logs. Mirrors the REST
        // client's bearer discipline. Safe to log this URL (no secret in it).
        let url = format!("{}?format=msgpack", apex_ws_url());
        report(ErrorLevel::Info, "apex_data.ws", "connecting", format!("→ {url}"));
        publish_state(ConnectionState::Connecting {
            attempt: RECONNECT_COUNT.load(Ordering::Relaxed) + 1,
        });

        let mut req = match url.into_client_request() {
            Ok(r) => r,
            Err(e) => {
                report(ErrorLevel::Error, "apex_data.ws", "bad_url", e.to_string());
                if let Some(d) = backoff.next_delay() {
                    tokio::time::sleep(d).await;
                }
                continue;
            }
        };
        if let Some(tok) = apex_token() {
            use tokio_tungstenite::tungstenite::http::{header::AUTHORIZATION, HeaderValue};
            if let Ok(v) = HeaderValue::from_str(&format!("Bearer {tok}")) {
                req.headers_mut().insert(AUTHORIZATION, v);
            }
        }

        // LAN-IP override: when set, connect TCP directly to the homelab
        // Traefik IP and hand the socket to `client_async` with the original
        // Host-bearing request. Keeps Traefik ingress routing intact.
        let lan_override = match (super::config::apex_lan_ips().is_empty(), super::config::apex_host_port()) {
            (false, Some((_host, port))) => Some(port),
            _ => None,
        };

        let conn_result = if let Some(port) = lan_override {
            report(ErrorLevel::Info, "apex_data.ws", "lan_override", format!("dial :{port}"));
            match dial_lan_candidates(port).await {
                Ok(stream) => match client_async(req, MaybeTlsStream::Plain(stream)).await {
                    Ok((ws, _)) => Ok(ws),
                    Err(e) => Err(format!("handshake: {e}")),
                },
                Err(e) => Err(e),
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
                report(ErrorLevel::Warn, "apex_data.ws", "connect_failed", e.clone());
                broadcast(&mgr, &Frame::Connection(false));
                let next_attempt = RECONNECT_COUNT.load(Ordering::Relaxed) + 1;
                if let Some(d) = backoff.next_delay() {
                    publish_state(ConnectionState::Backoff {
                        until: std::time::Instant::now() + d,
                        attempt: next_attempt,
                        reason: format!("connect_failed: {e}"),
                    });
                    tokio::time::sleep(d).await;
                }
                continue;
            }
        };
        report(ErrorLevel::Info, "apex_data.ws", "connected", "ws established");
        if let Ok(mut c) = mgr.connected.lock() { *c = true; }
        broadcast(&mgr, &Frame::Connection(true));
        backoff.reset();
        // Wave 11c: handshake done — push Authenticated, then a Subscribed
        // snapshot reflecting whatever subs were re-sent below. The route
        // table is the source of truth for live sub count.
        publish_state(ConnectionState::Authenticated);
        let sub_count = mgr.subs.lock().ok().map(|s|
            s.bars.len() + s.bars_mark.len() + s.tape.len() + s.quotes.len() + s.chain.len()
        ).unwrap_or(0);
        if sub_count > 0 {
            publish_state(ConnectionState::Subscribed { count: sub_count });
        }

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
                // Wave 11c: surface the stall as a Backoff transition so push
                // subscribers see the same reason the poll-path synthesizes.
                publish_state(ConnectionState::Backoff {
                    until: std::time::Instant::now() + Duration::from_secs(1),
                    attempt: RECONNECT_COUNT.load(Ordering::Relaxed),
                    reason: "tick_stalled".into(),
                });
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
                        // AT-011: Pong / Ping / Frame. These prove the SOCKET is
                        // alive even when no market data is flowing, so they must
                        // count toward socket liveness — previously this arm was
                        // an empty no-op and a quiet feed reconnected every 30s.
                        Some(Ok(_))  => { LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed); }
                        Some(Err(e)) => { report(ErrorLevel::Warn, "apex_data.ws", "recv_error", e.to_string()); break; }
                        None         => { report(ErrorLevel::Warn, "apex_data.ws", "stream_ended", "no more frames"); break; }
                    }
                }
                ctrl = rx_ctrl.recv() => {
                    match ctrl {
                        Some(CtrlMsg::PushSubs { force }) => {
                            // Suppress identical re-sends. ApexData write-locks
                            // its fanout index per subscription frame, so a
                            // no-op push starves dispatch for every client on
                            // the account. `force` is set after resync/reconnect
                            // where the server genuinely needs re-declaring.
                            let frame = build_subs_frame(&mgr);
                            let unchanged = !force && matches!(
                                (&frame, mgr.last_sent_subs.lock().ok().as_deref()),
                                (Some(f), Some(Some(prev))) if f == prev
                            );
                            if unchanged {
                                SUBS_PUSH_SUPPRESSED.fetch_add(1, Ordering::Relaxed);
                            } else if let Some(frame) = frame {
                                if let Ok(mut g) = mgr.last_sent_subs.lock() {
                                    *g = Some(frame.clone());
                                }
                                SUBS_PUSH_SENT.fetch_add(1, Ordering::Relaxed);
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
        // Forget what we last declared. The server keeps no subscription state
        // across a connection, so the first push after reconnect MUST go out
        // even if our local set is byte-identical — otherwise suppression would
        // leave us silently subscribed to nothing.
        if let Ok(mut g) = mgr.last_sent_subs.lock() { *g = None; }
        broadcast(&mgr, &Frame::Connection(false));
        if mgr.shutdown.load(Ordering::SeqCst) {
            publish_state(ConnectionState::ShuttingDown);
            break;
        }
        if let Some(d) = backoff.next_delay() {
            publish_state(ConnectionState::Backoff {
                until: std::time::Instant::now() + d,
                attempt: RECONNECT_COUNT.load(Ordering::Relaxed) + 1,
                reason: "disconnected".into(),
            });
            tokio::time::sleep(d).await;
        } else {
            // Backoff exhausted — terminal failure.
            publish_state(ConnectionState::Failed {
                reason: connectivity::ConnectionError::MaxRetriesExceeded(
                    RECONNECT_COUNT.load(Ordering::Relaxed),
                ),
            });
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
        let now = now_ms();

        // AT-011: data starvation on a LIVE socket is reported, never
        // reconnected. If pongs are arriving but no market data is, the
        // subscription or entitlement is wrong upstream — churning the
        // connection cannot fix that and only hides it behind a reconnect loop.
        // This is the condition that previously masqueraded as a dead feed.
        let last_data = LAST_DATA_AT_MS.load(Ordering::Relaxed);
        let last_any = LAST_MESSAGE_AT_MS.load(Ordering::Relaxed);
        if last_data > 0
            && now - last_data > STALE_TIMEOUT_MS
            && now - last_any <= STALE_TIMEOUT_MS
        {
            let quiet_secs = (now - last_data) / 1000;
            let last_toast = LAST_STALL_TOAST_AT.load(Ordering::Relaxed);
            if now - last_toast >= STALL_TOAST_COOLDOWN_MS {
                LAST_STALL_TOAST_AT.store(now, Ordering::Relaxed);
                report(
                    ErrorLevel::Warn,
                    "apex_data.ws",
                    "data_starved",
                    format!(
                        "socket alive but no market data for {quiet_secs}s — \
                         check subscriptions/entitlement, not the connection"
                    ),
                );
            }
        }

        let last = last_any;
        if last > 0 && now - last > STALE_TIMEOUT_MS && !FORCE_RECONNECT.load(Ordering::Relaxed) {
            let stall_secs = (now - last) / 1000;
            LAST_STALL_AT_MS.store(now, Ordering::Relaxed);
            report(
                ErrorLevel::Warn,
                "apex_data.ws",
                "tick_stalled",
                format!("no frames for {}ms — forcing reconnect", now - last),
            );
            // P1.11: emit a user-visible toast if we haven't toasted recently.
            // Gate on 60s cooldown so a sustained outage doesn't spam the user.
            let last_toast = LAST_STALL_TOAST_AT.load(Ordering::Relaxed);
            if now - last_toast >= STALL_TOAST_COOLDOWN_MS {
                LAST_STALL_TOAST_AT.store(now, Ordering::Relaxed);
                report(
                    ErrorLevel::Warn,
                    "apex_data.ws",
                    "feed_stalled",
                    format!("ApexData feed silent for >{stall_secs}s — reconnecting"),
                );
            }
            FORCE_RECONNECT.store(true, Ordering::SeqCst);
            // Wave 7E: hook for SubscriptionManager gap-fill — fires on the
            // *next* successful reconnect via the broadcast wired below.
        }
    }
}

fn build_subs_frame(mgr: &Arc<Manager>) -> Option<String> {
    let s = mgr.subs.lock().ok()?.clone();
    // SORTED, not just collected. These are HashSets, which iterate in a
    // different order every time, so an unchanged subscription set serialized
    // to different bytes on every frame — defeating any dedup and making the
    // wire traffic undiffable. Sorting makes the frame a pure function of the
    // set, which is what lets `last_sent_subs` suppress no-op pushes.
    let sorted = |set: &std::collections::HashSet<String>| -> Vec<String> {
        let mut v: Vec<String> = set.iter().cloned().collect();
        v.sort_unstable();
        v
    };
    let bars      = sorted(&s.bars);
    let bars_mark = sorted(&s.bars_mark);
    let tape      = sorted(&s.tape);
    let quotes    = sorted(&s.quotes);
    let chain     = sorted(&s.chain);
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
    // AT-011: a data frame proves BOTH that the socket is alive and that the
    // feed is actually delivering. Pongs only prove the former.
    LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);
    LAST_DATA_AT_MS.store(now_ms(), Ordering::Relaxed);
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
    // AT-011: see handle_text.
    LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);
    LAST_DATA_AT_MS.store(now_ms(), Ordering::Relaxed);
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

// ────────────────────────────────────────────────────────────────────────────
// Replay subscription (SOTA §4.2) — dedicated WS, separate from the live data
// session above. Replay frames carry their own `replay_id` in the envelope so
// callers can correlate but we route them straight to the user callback rather
// than mixing them with live state.
// ────────────────────────────────────────────────────────────────────────────

/// Handle returned by [`subscribe_replay`]. Drop or call [`ReplaySubscription::stop`]
/// to close the underlying socket. Cloning the handle is cheap (Arc<AtomicBool>).
#[derive(Clone)]
pub struct ReplaySubscription {
    pub replay_id: String,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl ReplaySubscription {
    /// Signal the background task to close the WS on the next loop iteration.
    /// Idempotent — safe to call multiple times. Doesn't block.
    pub fn stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn is_stopped(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for ReplaySubscription {
    fn drop(&mut self) {
        // Only the *last* clone going out of scope actually shuts down the
        // socket — Arc handles refcount. The stop flag is read by the task
        // on every recv loop iteration so the close is bounded by one frame
        // (or one ping interval) of latency.
        if Arc::strong_count(&self.stop) == 1 {
            self.stop();
        }
    }
}

/// Open a dedicated WS to `/api/replay/:id/stream`. Frames are passed verbatim
/// to `on_frame` on the tokio worker thread, so the callback **must be**
/// `Send + Sync + 'static` and should hand off via a channel (don't block).
///
/// The returned [`ReplaySubscription`] keeps the socket alive; drop it (or
/// call [`ReplaySubscription::stop`]) to tear down.
///
/// Uses the same WS URL base as the live data session (`apex_ws_url()`) but
/// rewrites the path to `/api/replay/:id/stream`. Auth token, if set, is sent
/// via the Authorization header (not the URL) to mirror the live WS.
pub fn subscribe_replay<F>(replay_id: impl Into<String>, on_frame: F) -> ReplaySubscription
where
    F: Fn(super::types::ReplayEvent) + Send + Sync + 'static,
{
    let replay_id = replay_id.into();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_bg = stop.clone();
    let id_bg = replay_id.clone();
    let cb = Arc::new(on_frame);

    runtime().spawn(async move {
        run_replay_stream(id_bg, stop_bg, cb).await;
    });

    ReplaySubscription { replay_id, stop }
}

async fn run_replay_stream<F>(
    replay_id: String,
    stop: Arc<std::sync::atomic::AtomicBool>,
    on_frame: Arc<F>,
)
where
    F: Fn(super::types::ReplayEvent) + Send + Sync + 'static,
{
    // Derive the per-session URL from the live WS base. The live URL points at
    // `/api/ws` (or similar) — replace the path with `/api/replay/:id/stream`.
    let base = apex_ws_url();
    let url = build_replay_url(&base, &replay_id);
    report(ErrorLevel::Info, "apex_data.ws.replay", "replay_connect", format!("connect {url}"));

    let mut req = match url.clone().into_client_request() {
        Ok(r) => r,
        Err(e) => {
            report(ErrorLevel::Error, "apex_data.ws.replay", "replay_bad_url", format!("bad url: {e}"));
            on_frame(super::types::ReplayEvent::new_error(format!("bad url: {e}")));
            return;
        }
    };
    if let Some(tok) = apex_token() {
        use tokio_tungstenite::tungstenite::http::{header::AUTHORIZATION, HeaderValue};
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {tok}")) {
            req.headers_mut().insert(AUTHORIZATION, v);
        }
    }

    // Reuse the LAN-IP override path (same trick the live WS uses) so the
    // replay socket also bypasses public DNS in the homelab.
    let lan_override = match (super::config::apex_lan_ips().is_empty(), super::config::apex_host_port()) {
        (false, Some((_host, port))) => Some(port),
        _ => None,
    };

    let conn_result = if let Some(port) = lan_override {
        match dial_lan_candidates(port).await {
            Ok(stream) => match client_async(req, MaybeTlsStream::Plain(stream)).await {
                Ok((ws, _)) => Ok(ws),
                Err(e) => Err(format!("handshake: {e}")),
            },
            Err(e) => Err(e),
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
            report(ErrorLevel::Warn, "apex_data.ws.replay", "replay_connect_failed", format!("connect failed: {e}"));
            on_frame(super::types::ReplayEvent::new_error(format!("connect: {e}")));
            return;
        }
    };
    report(ErrorLevel::Info, "apex_data.ws.replay", "replay_connected", format!("connected ({replay_id})"));

    let (_tx_ws, mut rx_ws) = ws.split();

    loop {
        if stop.load(std::sync::atomic::Ordering::SeqCst) {
            report(ErrorLevel::Info, "apex_data.ws.replay", "replay_stop", format!("stop requested ({replay_id})"));
            break;
        }
        // Bound the recv so the stop flag is observed within ~250ms even
        // when the server is idle — important when the user clicks Stop on a
        // replay that has finished server-side but hasn't sent a Close yet.
        let next = tokio::time::timeout(Duration::from_millis(250), rx_ws.next()).await;
        match next {
            Err(_) => continue,                                   // timeout — re-check stop flag
            Ok(None) => { report(ErrorLevel::Info, "apex_data.ws.replay", "replay_stream_end", "stream ended"); break; }
            Ok(Some(Err(e))) => {
                report(ErrorLevel::Warn, "apex_data.ws.replay", "replay_recv_error", format!("recv error: {e}"));
                on_frame(super::types::ReplayEvent::new_error(format!("recv: {e}")));
                break;
            }
            Ok(Some(Ok(msg))) => match msg {
                Message::Close(_) => { report(ErrorLevel::Info, "apex_data.ws.replay", "replay_close", "close"); break; }
                Message::Binary(bytes) => handle_replay_bytes(&bytes, &on_frame),
                Message::Text(text)    => handle_replay_text(&text, &on_frame),
                _ => {}
            },
        }
    }
    report(ErrorLevel::Info, "apex_data.ws.replay", "replay_closed", format!("closed ({replay_id})"));
}

/// Build the replay stream URL by reusing the live WS host:port and swapping
/// the path. `apex_ws_url()` may end in `/api/ws` or similar — we strip the
/// path and append `/api/replay/:id/stream?format=msgpack`. The auth token is
/// sent via the Authorization header by the caller, not the URL.
fn build_replay_url(base: &str, replay_id: &str) -> String {
    // The base URL is constructed elsewhere; rather than parse it fully we
    // do a cheap split on `://` + first `/` of the authority portion.
    let (scheme_auth, _path) = match base.find("://") {
        Some(i) => {
            let rest = &base[i + 3..];
            match rest.find('/') {
                Some(j) => (&base[..i + 3 + j], &rest[j..]),
                None    => (base, ""),
            }
        }
        None => (base, ""),
    };
    format!("{scheme_auth}/api/replay/{replay_id}/stream?format=msgpack")
}

fn handle_replay_text<F>(text: &str, on_frame: &Arc<F>)
where F: Fn(super::types::ReplayEvent) + Send + Sync + 'static {
    match serde_json::from_str::<InEnvelope>(text) {
        Ok(env) => dispatch_replay(env, on_frame),
        Err(e) => report(ErrorLevel::Warn, "apex_data.ws.replay", "replay_bad_json", format!("bad json frame: {e}")),
    }
}

fn handle_replay_bytes<F>(bytes: &[u8], on_frame: &Arc<F>)
where F: Fn(super::types::ReplayEvent) + Send + Sync + 'static {
    match rmp_serde::from_slice::<InEnvelope>(bytes) {
        Ok(env) => dispatch_replay(env, on_frame),
        Err(_) => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                if let Ok(env) = serde_json::from_str::<InEnvelope>(text) {
                    dispatch_replay(env, on_frame); return;
                }
            }
            report(ErrorLevel::Warn, "apex_data.ws.replay", "replay_bad_msgpack", "bad msgpack frame");
        }
    }
}

/// Replay-side dispatch: map envelope `type` → lightweight ReplayEvent and
/// hand it to the user callback. We deliberately keep the event shape
/// minimal — the chart pane already owns the heavy bar/trade structs — so
/// the events log stays cheap to retain.
/// W1-09: build a REAL overlay bar from a replay `bar` payload.
///
/// Returns `None` unless every one of open/high/low/close is present. This is
/// the load-bearing rule of the whole item: an overlay bar reconstructed from
/// the close alone (or with zeros standing in for missing legs) would be
/// FABRICATED order data rendered as if it were real — the exact defect class
/// W0-12 removed from the footprint chart. Volume is optional and defaults to
/// 0.0 because a missing volume understates a bar; a missing PRICE leg would
/// misstate it.
///
/// Pure → unit-testable with no socket, chart, or replay session.
fn replay_overlay_bar(bar: &serde_json::Value) -> Option<(crate::chart_renderer::Bar, i64)> {
    let f = |k: &str| bar.get(k).and_then(|v| v.as_f64());
    let (o, h, l, c) = (f("open")?, f("high")?, f("low")?, f("close")?);
    let t_ms = bar.get("time").and_then(|v| v.as_i64())?;
    Some((
        crate::chart_renderer::Bar {
            open: o as f32,
            high: h as f32,
            low: l as f32,
            close: c as f32,
            volume: f("volume").unwrap_or(0.0) as f32,
            _pad: 0.0,
        },
        t_ms,
    ))
}

fn dispatch_replay<F>(env: InEnvelope, on_frame: &Arc<F>)
where F: Fn(super::types::ReplayEvent) + Send + Sync + 'static {
    use super::types::ReplayEvent;
    let evt: ReplayEvent = match env.kind.as_str() {
        "bar" => {
            // bar payload is a BarUpdate; pull (symbol, t, close).
            let bar = env.data.get("bar").unwrap_or(&env.data);
            let symbol = bar.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let t_ms   = bar.get("time").and_then(|v| v.as_i64()).unwrap_or(0);
            let close  = bar.get("close").and_then(|v| v.as_f64()).unwrap_or(0.0);
            // W1-09: the full OHLCV is right here in the envelope and used to be
            // discarded. Route it to the chart overlay on its own path — the
            // events log keeps the minimal ReplayEvent it was designed for.
            // Panes with no overlay installed drop these, so this costs nothing
            // when the user hasn't asked for the overlay.
            if let Some((ovl_bar, ovl_t)) = replay_overlay_bar(bar) {
                crate::send_to_native_chart(
                    crate::chart_renderer::ChartCommand::ReplayOverlayBar {
                        symbol: symbol.clone(),
                        bar: ovl_bar,
                        t_ms: ovl_t,
                    },
                );
            }
            ReplayEvent::new_bar(symbol, t_ms, close)
        }
        "trade" => {
            let symbol = env.data.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let t_ms   = env.data.get("time").and_then(|v| v.as_i64()).unwrap_or(0);
            let price  = env.data.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
            ReplayEvent::new_trade(symbol, t_ms, price)
        }
        "snapshot" => {
            let bar = env.data.get("bar").unwrap_or(&env.data);
            let symbol = bar.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let t_ms   = bar.get("time").and_then(|v| v.as_i64()).unwrap_or(0);
            let close  = bar.get("close").and_then(|v| v.as_f64()).unwrap_or(0.0);
            ReplayEvent { kind: "snapshot", symbol, t_ms, price: Some(close) }
        }
        "error" => {
            let message = env.data.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            ReplayEvent::new_error(message)
        }
        other => {
            // Unknown frame type — keep it in the log as an "info" so the
            // user can see something happened, but don't choke.
            ReplayEvent::new_info(format!("frame: {other}"), 0)
        }
    };
    on_frame(evt);
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
        // ApexData emits the type as "chaindelta" (no underscore — see FE brief
        // frame list); accept both spellings so live chain updates aren't dropped
        // as unknown_frame.
        "chaindelta" | "chain_delta" => match serde_json::from_value::<ChainDelta>(env.data) {
            Ok(d) => Frame::ChainDelta(d),
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_chain_delta", e.to_string()); return; }
        },
        // SOTA §4.4 — calibrated trade plan v2. Every non-legacy field on
        // TradePlanV2 uses #[serde(default)], so older servers that emit
        // only `entry/target/stop` still deserialize and just render as
        // point-form plans.
        "trade_plan" => match serde_json::from_value::<TradePlanV2>(env.data) {
            Ok(p) => Frame::TradePlan(p),
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_trade_plan", e.to_string()); return; }
        },
        // SOTA §4.5 — spike-explanation toast. Derive the dedup id from
        // (symbol, t_ms) after deserialization — the server doesn't have to
        // know about our client-side dedup scheme.
        "spike" => match serde_json::from_value::<SpikeExplanation>(env.data) {
            Ok(mut s) => {
                s.id = SpikeExplanation::derive_id(&s.symbol, s.t_ms);
                Frame::Spike(s)
            }
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_spike", e.to_string()); return; }
        },
        "halt" => match serde_json::from_value::<HaltReading>(env.data) {
            Ok(h) => Frame::Halt(h),
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_halt", e.to_string()); return; }
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
        // ── SOTA UX §3.3 — combined signal ────────────────────────────────
        // Routes into `live_state.latest_combined[symbol]` so SignalsPanel
        // can render rows with calibrated + lineage_id without re-fetching.
        "combined" => match serde_json::from_value::<CombinedSignalV2>(env.data) {
            Ok(c) => {
                super::live_state::push_combined(c.clone());
                Frame::Combined(c)
            }
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_combined", e.to_string()); return; }
        },
        // ── SOTA UX §3.3 — regime frame ───────────────────────────────────
        // Routes into `live_state.latest_regime` so the always-visible
        // RegimeTape top strip can render without re-fetching.
        "regime" => match serde_json::from_value::<RegimeFrame>(env.data) {
            Ok(r) => {
                super::live_state::push_regime(r.clone());
                Frame::Regime(r)
            }
            Err(e) => { report(ErrorLevel::Warn, "apex_data.ws", "parse_regime", e.to_string()); return; }
        },
        other => { report(ErrorLevel::Warn, "apex_data.ws", "unknown_frame", other.to_string()); return; }
    };
    broadcast(mgr, &frame);
}

#[cfg(test)]
mod w1_09_replay_overlay_tests {
    use super::replay_overlay_bar;
    use serde_json::json;

    #[test]
    fn extracts_the_real_ohlcv_the_envelope_already_carries() {
        let bar = json!({
            "symbol": "SPY", "time": 1_700_000_000_000i64,
            "open": 400.0, "high": 405.5, "low": 399.25, "close": 404.0,
            "volume": 12345.0
        });
        let (b, t) = replay_overlay_bar(&bar).expect("complete bar → overlay bar");
        assert_eq!(t, 1_700_000_000_000i64);
        assert_eq!(b.open, 400.0);
        assert_eq!(b.high, 405.5);
        assert_eq!(b.low, 399.25);
        assert_eq!(b.close, 404.0);
        assert_eq!(b.volume, 12345.0);
    }

    // THE load-bearing rule of W1-09. A bar rebuilt from the close alone is
    // fabricated order data rendered as if it were real — the W0-12 footprint
    // defect. Refusing to emit is the only correct answer.
    #[test]
    fn refuses_to_fabricate_when_a_price_leg_is_missing() {
        for missing in ["open", "high", "low", "close"] {
            let mut bar = json!({
                "symbol": "SPY", "time": 1i64,
                "open": 1.0, "high": 2.0, "low": 0.5, "close": 1.5
            });
            bar.as_object_mut().unwrap().remove(missing);
            assert!(
                replay_overlay_bar(&bar).is_none(),
                "missing {missing} must yield NO overlay bar, never a synthesized one"
            );
        }
    }

    #[test]
    fn a_close_only_payload_yields_nothing() {
        // Exactly what ReplayEvent carries. If this ever returns Some, the
        // overlay has started inventing OHLC from a single price.
        let bar = json!({ "symbol": "SPY", "time": 1i64, "close": 404.0 });
        assert!(replay_overlay_bar(&bar).is_none());
    }

    #[test]
    fn missing_time_yields_nothing() {
        // Without a timestamp the bar cannot be aligned to the time axis, so
        // it would render at an arbitrary x position.
        let bar = json!({ "open": 1.0, "high": 2.0, "low": 0.5, "close": 1.5 });
        assert!(replay_overlay_bar(&bar).is_none());
    }

    #[test]
    fn absent_volume_defaults_to_zero_but_prices_are_never_defaulted() {
        // Asymmetry is deliberate: a missing volume understates a bar, a
        // missing price leg misstates it.
        let bar = json!({
            "symbol": "SPY", "time": 7i64,
            "open": 1.0, "high": 2.0, "low": 0.5, "close": 1.5
        });
        let (b, _) = replay_overlay_bar(&bar).expect("prices complete → bar");
        assert_eq!(b.volume, 0.0);
    }
}

#[cfg(test)]
mod subs_frame_stability_tests {
    use super::*;

    fn state(quotes: &[&str]) -> SubState {
        let mut s = SubState::default();
        s.quotes = quotes.iter().map(|x| x.to_string()).collect();
        s
    }

    /// Serialize exactly as `build_subs_frame` does, without needing a Manager.
    fn frame_of(s: &SubState) -> String {
        let sorted = |set: &std::collections::HashSet<String>| -> Vec<String> {
            let mut v: Vec<String> = set.iter().cloned().collect();
            v.sort_unstable();
            v
        };
        let (bars, bars_mark, tape, quotes, chain) =
            (sorted(&s.bars), sorted(&s.bars_mark), sorted(&s.tape), sorted(&s.quotes), sorted(&s.chain));
        serde_json::to_string(&OutMsg {
            subscribe: Some(&bars), subscribe_mark: Some(&bars_mark),
            tape: Some(&tape), quotes: Some(&quotes), chain: Some(&chain), format: None,
        }).unwrap()
    }

    #[test]
    fn the_same_contract_set_always_serializes_identically() {
        // THE bug this guards: HashSet iteration order varies run to run, so an
        // unchanged subscription produced different bytes every frame. That
        // defeats dedup entirely — every no-op push looks like a real change
        // and reaches ApexData, which write-locks its fanout index per frame.
        let a = frame_of(&state(&["O:SPY260804C00744000", "O:SPY260804P00744000", "O:QQQ260804C00500000"]));
        for _ in 0..64 {
            // Rebuilt from scratch each time: fresh HashSet, fresh iteration order.
            let b = frame_of(&state(&["O:QQQ260804C00500000", "O:SPY260804P00744000", "O:SPY260804C00744000"]));
            assert_eq!(a, b, "identical sets must produce identical bytes regardless of insertion order");
        }
    }

    #[test]
    fn a_genuinely_different_set_produces_a_different_frame() {
        // Suppression must not be so aggressive that a real change is dropped —
        // that would leave the user staring at unseated contracts forever.
        let a = frame_of(&state(&["O:SPY260804C00744000"]));
        let b = frame_of(&state(&["O:SPY260804C00744000", "O:SPY260804P00744000"]));
        assert_ne!(a, b, "adding a visible contract MUST reach the wire");
    }

    #[test]
    fn quotes_are_sorted_in_the_payload() {
        let f = frame_of(&state(&["O:C", "O:A", "O:B"]));
        let a = f.find("O:A").expect("A present");
        let b = f.find("O:B").expect("B present");
        let c = f.find("O:C").expect("C present");
        assert!(a < b && b < c, "payload must be deterministically ordered: {f}");
    }

    #[test]
    fn an_empty_set_still_serializes_all_arrays() {
        // MARK_BARS_PROTOCOL: the server treats a MISSING array as empty, so
        // every array must be present even when empty — otherwise clearing a
        // subscription silently fails to clear.
        let f = frame_of(&SubState::default());
        for key in ["subscribe", "subscribe_mark", "tape", "quotes", "chain"] {
            assert!(f.contains(key), "{key} must be present in {f}");
        }
    }
}

#[cfg(test)]
mod lan_dial_tests {
    use super::*;

    #[test]
    fn the_per_candidate_budget_is_bounded() {
        // A node that black-holes the SYN (firewalled, mid-reboot) hangs the
        // connect until the OS gives up — tens of seconds. Unbounded, walking a
        // candidate list would be WORSE than the single IP it replaced, because
        // each dead node adds its own full stall before the next is tried.
        assert!(LAN_DIAL_TIMEOUT <= Duration::from_secs(3),
            "per-candidate dial budget must stay small enough that walking the \
             whole list beats one hung connect");
    }

    #[test]
    fn walking_every_candidate_stays_within_a_reconnect_budget() {
        // Worst case: every candidate times out. That total is what a reconnect
        // costs before falling back — it must not exceed the chain fetch's own
        // patience, or the terminal drops to a synthetic chain while the dialer
        // is still working through the list.
        let worst = LAN_DIAL_TIMEOUT * crate::apex_data::config::default_lan_ip_count() as u32;
        assert!(worst <= Duration::from_secs(8),
            "worst-case dial walk {worst:?} would outlast the chain retry window");
    }

    #[tokio::test]
    async fn an_empty_candidate_list_fails_fast_and_explains_itself() {
        use crate::apex_data::config;
        // Save/restore: this mutates PROCESS-WIDE config, and the suite runs
        // tests in parallel. Leaving it clobbered would silently strip the LAN
        // candidates out from under any test that reads them later — a flake
        // that would look like a networking bug rather than a dirty fixture.
        let prev = config::apex_lan_ips().join(",");
        config::set_apex_lan_ip(Some(String::new()));
        let err = dial_lan_candidates(65535).await.unwrap_err();
        config::set_apex_lan_ip(if prev.is_empty() { None } else { Some(prev) });
        assert!(err.contains("no LAN candidates"), "unhelpful error: {err}");
    }
}
