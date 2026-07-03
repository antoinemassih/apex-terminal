//! ResilientWs — shared WebSocket reconnect harness for the secondary feeds.
//!
//! The apex_data firehose (`apex_data/ws.rs`) has its own full-featured manager.
//! The smaller feeds (futures, dom, intercepts, drawings, signals, crypto) each
//! used to hand-roll: connect + LAN-resolve + a reconnect sleep (often a fixed
//! 1–2s, jitter-less, watchdog-less) + a private `NATIVE_CHART_TXS` send — and
//! several never registered for Shutdown, so they didn't drain on exit. This
//! consolidates all of that into one harness:
//!   - LAN-aware connect (single copy)
//!   - jittered exponential [`Backoff`] reconnect (no thundering herd)
//!   - idle watchdog (reconnect if no frame within `idle_timeout`)
//!   - Shutdown registration (drains on process exit)
//!   - reconnect-on-signal (target/symbol change)
//!   - shared chart-command send ([`send_to_charts`])
//!
//! A feed reduces to a `url_provider` (return `None` to idle) + an `on_text`
//! handler. Pure transport plumbing — it never touches the GPU render path.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::chart_renderer::ChartCommand;
use crate::data::connectivity::{self, Backoff};

/// Send a chart command to every live native-chart window, pruning dead senders,
/// then wake the UI. Replaces the per-feed copy of this lock+retain+wake.
pub fn send_to_charts(cmd: ChartCommand) {
    if let Some(lock) = crate::NATIVE_CHART_TXS.get() {
        if let Ok(mut g) = lock.lock() {
            g.retain(|tx| tx.send(cmd.clone()).is_ok());
        }
    }
    crate::wake_native_ui();
}

// ── Unified /ws/v2 topic helpers ─────────────────────────────────────────────
//
// ApexData's /ws/v2 carries every datatype as a topic on ONE socket (bars,
// trades, quotes, depth, futures_bars, drawings, intercepts, scans, alerts,
// signals). A feed opts in via `APEX_WS_V2=1`; until then it keeps using its
// bespoke endpoint (which remains aliased server-side). See
// ApexData/docs/SPEC_DATA_ACCESS_ARCHITECTURE.md §3.

/// True when feeds should use the unified `/ws/v2` socket instead of bespoke
/// per-topic endpoints. Opt-in (default off) so the migration is verified at
/// market hours before becoming the default.
pub fn v2_enabled() -> bool {
    std::env::var("APEX_WS_V2").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false)
}

/// The unified socket URL `{scheme}://{host}/ws/v2?format=json`, derived from
/// the configured apex-data base.
pub fn v2_url() -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    format!("{host}/ws/v2?format=json")
}

/// Build a `/ws/v2` subscribe frame for the given topics. `instrument` is a
/// concrete symbol for instrument-scoped topics, or `"*"` for account-wide ones.
pub fn v2_subscribe(datatypes: &[&str], instrument: &str) -> String {
    serde_json::json!({
        "action": "subscribe",
        "instrument": instrument,
        "datatypes": datatypes,
        "time_source": { "mode": "live" },
    })
    .to_string()
}

/// Unwrap a `/ws/v2` `Frame::Event` envelope
/// (`{"v":1,"type":"event","data":{"datatype":..,"symbol":..,"data":<event>}}`)
/// into `(datatype, inner_event)`. Returns `None` for non-event frames
/// (hello/error/etc.) or a raw bespoke frame — so callers can fall back to
/// parsing the text as their legacy shape.
pub fn unwrap_v2_event(text: &str) -> Option<(String, serde_json::Value)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("type")?.as_str()? != "event" {
        return None;
    }
    let data = v.get("data")?;
    let dt = data.get("datatype")?.as_str()?.to_string();
    let inner = data.get("data")?.clone();
    Some((dt, inner))
}

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// LAN-aware connect: when an apex-data LAN IP is configured, dial it directly
/// (bypassing public DNS that may return an un-routable WAN IP) while keeping
/// the request's Host header intact for ingress routing.
pub async fn connect_lan_aware(url: &str)
    -> Result<WsStream, Box<dyn std::error::Error + Send + Sync>>
{
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::{client_async, connect_async, MaybeTlsStream};
    use tokio::net::TcpStream;
    use crate::data::feeds::apex_data::config;

    let req = url.into_client_request()?;
    match (config::apex_lan_ip(), config::apex_host_port()) {
        (Some(ip), Some((_h, port))) => {
            let stream = TcpStream::connect((ip.as_str(), port)).await?;
            let (ws, _) = client_async(req, MaybeTlsStream::Plain(stream)).await?;
            Ok(ws)
        }
        _ => {
            let (ws, _) = connect_async(req).await?;
            Ok(ws)
        }
    }
}

/// Configuration for a resilient feed connection.
pub struct WsConfig {
    /// Stable name for logging + Shutdown registration.
    pub name: &'static str,
    /// Flip to force the loop to drop the current connection and re-evaluate
    /// `url_provider` (e.g. on a target/symbol change). The helper clears it
    /// after each (re)connect.
    pub reconnect: Arc<AtomicBool>,
    /// Reconnect if no frame arrives within this window. `None` = no watchdog.
    pub idle_timeout: Option<Duration>,
    /// Returns the URL to connect to, or `None` to idle (no active target).
    pub url_provider: Box<dyn Fn() -> Option<String> + Send>,
    /// Builds an optional JSON frame sent once immediately after each
    /// (re)connect. Evaluated PER connect, so per-symbol feeds (dom/futures) can
    /// return a subscribe for the *current* symbol. Return `None` to send nothing
    /// (stream-on-connect endpoints like `/ws/drawings`) — behaviour unchanged.
    /// `Some(json)` drives the unified `/ws/v2` socket's `{"action":"subscribe"…}`.
    pub subscribe_provider: Box<dyn Fn() -> Option<String> + Send>,
    /// Called for every text frame received.
    pub on_text: Box<dyn Fn(&str) + Send>,
}

/// Spawn a dedicated thread + current-thread runtime running the reconnect loop.
/// Call once (the caller guards its own `start()`).
pub fn spawn(cfg: WsConfig) {
    let shutdown = Arc::new(AtomicBool::new(false));
    connectivity::register(cfg.name, Arc::new(WsShutdown { name: cfg.name, flag: shutdown.clone() }));
    let name = cfg.name;
    let _ = std::thread::Builder::new().name(name.into()).spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!(target: "resilient_ws", "{name}: runtime build failed: {e}");
                return;
            }
        };
        rt.block_on(run_loop(cfg, shutdown));
    });
}

async fn run_loop(cfg: WsConfig, shutdown: Arc<AtomicBool>) {
    // Never give up — feeds reconnect indefinitely; the cap bounds the delay.
    let mut backoff = Backoff::new()
        .with_max(Duration::from_secs(30))
        .with_max_attempts(None);
    loop {
        if shutdown.load(Ordering::SeqCst) { return; }
        let Some(url) = (cfg.url_provider)() else {
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        };
        cfg.reconnect.store(false, Ordering::SeqCst);
        match run_one(&url, &cfg, &shutdown).await {
            // Clean exit (reconnect signal / shutdown / EOF) — reconnect promptly.
            Ok(()) => backoff.reset(),
            Err(e) => {
                tracing::warn!(target: "resilient_ws", "{}: {e}", cfg.name);
                let d = backoff.next_delay().unwrap_or(Duration::from_secs(30));
                tokio::time::sleep(d).await;
            }
        }
    }
}

async fn run_one(url: &str, cfg: &WsConfig, shutdown: &AtomicBool)
    -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    use futures_util::{SinkExt, StreamExt};
    let ws = connect_lan_aware(url).await?;
    let (mut write, mut read) = ws.split();
    // Drive the unified /ws/v2 socket (or any subscribe-on-connect endpoint).
    // Stream-on-connect feeds return `None` and this is a no-op (write half is
    // simply held open, which does not close the connection). Evaluated per
    // connect so per-symbol feeds subscribe for the current symbol.
    if let Some(msg) = (cfg.subscribe_provider)() {
        use tokio_tungstenite::tungstenite::Message;
        write.send(Message::Text(msg.into())).await?;
    }
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    let mut last_activity = Instant::now();
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if cfg.reconnect.load(Ordering::SeqCst) || shutdown.load(Ordering::SeqCst) {
                    return Ok(());
                }
                if let Some(to) = cfg.idle_timeout {
                    if last_activity.elapsed() > to {
                        return Ok(()); // stale connection → reconnect
                    }
                }
            }
            frame = read.next() => {
                let Some(msg) = frame else { return Ok(()); };
                let msg = msg?;
                if msg.is_text() {
                    last_activity = Instant::now();
                    (cfg.on_text)(msg.to_text()?);
                }
            }
        }
    }
}

// ── Subscribed feeds (crypto / signals): connect + subscribe + heartbeat ─────
//
// crypto_feed and signals_feed are ~95% identical: connect → send a subscribe
// message over the write half → publish ConnectionState → heartbeat-ping while
// idle → tick-age watchdog with a cooldown'd stall toast → metrics. This second
// entry point captures all of that; the feed supplies its URL, subscribe JSON,
// heartbeat/staleness policy, its own metrics + state-publish handles (so the
// provider's `state_tx().subscribe()` keeps working), and an `on_text` parser.

use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64};
use crate::data::connectivity::{ConnectionState, errors_sink::{report, ErrorLevel}};

/// `&'static` handles to a feed's metric counters — the helper bumps the
/// transport-level ones; `parse_errors` is the feed's own (bumped in `on_text`).
pub struct FeedMetrics {
    pub messages_in: &'static AtomicU64,
    pub reconnect_count: &'static AtomicU32,
    pub last_message_at_ms: &'static AtomicI64,
}

pub struct SubscribedWsConfig {
    pub name: &'static str,
    /// Build the WS URL (re-evaluated each connect for env overrides).
    pub url: Box<dyn Fn() -> String + Send>,
    /// JSON subscribe frame sent immediately after connect.
    pub subscribe_msg: String,
    /// `ConnectionState::Subscribed { count }` reported after subscribing.
    pub subscribed_count: usize,
    /// Heartbeat ping interval; `None` disables pings.
    pub heartbeat: Option<Duration>,
    /// When true, skip the heartbeat ping if a frame arrived within the interval
    /// (for chatty feeds where traffic is its own keepalive).
    pub conditional_heartbeat: bool,
    /// Reconnect (with a cooldown'd toast) if no frame within this window.
    pub stale_timeout: Option<Duration>,
    /// Trigger SubscriptionManager gap-fill after each non-initial reconnect.
    pub gap_fill_on_reconnect: bool,
    pub metrics: FeedMetrics,
    /// Publish lifecycle transitions to the feed's own `state_tx()` broadcast.
    pub publish_state: Box<dyn Fn(ConnectionState) + Send>,
    /// Parse + route one text frame (also bumps the feed's `parse_errors`).
    pub on_text: Box<dyn Fn(&str) + Send>,
}

fn epoch_ms() -> i64 {
    crate::foundation::time::now_ms()
}

/// Spawn a dedicated thread + runtime for a subscribe-style feed. Call once.
pub fn spawn_subscribed(cfg: SubscribedWsConfig) {
    let shutdown = Arc::new(AtomicBool::new(false));
    connectivity::register(cfg.name, Arc::new(WsShutdown { name: cfg.name, flag: shutdown.clone() }));
    let name = cfg.name;
    let _ = std::thread::Builder::new().name(name.into()).spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => { tracing::error!(target: "resilient_ws", "{name}: runtime build failed: {e}"); return; }
        };
        rt.block_on(run_subscribed_loop(cfg, shutdown));
    });
}

async fn run_subscribed_loop(cfg: SubscribedWsConfig, shutdown: Arc<AtomicBool>) {
    let stall_toast_at = AtomicI64::new(0);
    let mut backoff = Backoff::new().with_max_attempts(None);
    let mut first = true;
    loop {
        if shutdown.load(Ordering::SeqCst) {
            (cfg.publish_state)(ConnectionState::ShuttingDown);
            return;
        }
        if !first { cfg.metrics.reconnect_count.fetch_add(1, Ordering::Relaxed); }
        first = false;
        (cfg.publish_state)(ConnectionState::Connecting {
            attempt: cfg.metrics.reconnect_count.load(Ordering::Relaxed) + 1,
        });
        match run_subscribed_one(&cfg, &shutdown, &stall_toast_at).await {
            Ok(()) => backoff.reset(),
            Err(e) => report(ErrorLevel::Warn, cfg.name, "reconnect", e.to_string()),
        }
        if shutdown.load(Ordering::SeqCst) {
            (cfg.publish_state)(ConnectionState::ShuttingDown);
            return;
        }
        if let Some(d) = backoff.next_delay() {
            (cfg.publish_state)(ConnectionState::Backoff {
                until: std::time::Instant::now() + d,
                attempt: cfg.metrics.reconnect_count.load(Ordering::Relaxed) + 1,
                reason: "disconnected".into(),
            });
            tokio::time::sleep(d).await;
        }
    }
}

async fn run_subscribed_one(
    cfg: &SubscribedWsConfig,
    shutdown: &AtomicBool,
    stall_toast_at: &AtomicI64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let url = (cfg.url)();
    report(ErrorLevel::Info, cfg.name, "connecting", &url);
    let ws = connect_lan_aware(&url).await?;
    let (mut write, mut read) = ws.split();

    // Gap-fill after a real reconnect (not the first connect).
    if cfg.gap_fill_on_reconnect && cfg.metrics.reconnect_count.load(Ordering::Relaxed) > 0 {
        let name = cfg.name;
        tokio::spawn(async move {
            let n = crate::data::providers::registry::subscription_manager()
                .gap_fill_on_reconnect_all().await;
            if n > 0 { report(ErrorLevel::Info, name, "gap_fill", format!("replayed {n} bars after reconnect")); }
        });
    }

    write.send(Message::Text(cfg.subscribe_msg.clone().into())).await?;
    (cfg.publish_state)(ConnectionState::Authenticated);
    (cfg.publish_state)(ConnectionState::Subscribed { count: cfg.subscribed_count });

    let hb = cfg.heartbeat.unwrap_or(Duration::from_secs(3600));
    let mut heartbeat = tokio::time::interval(hb);
    heartbeat.tick().await; // skip immediate first
    let mut watchdog = tokio::time::interval(Duration::from_secs(5));
    watchdog.tick().await;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if cfg.heartbeat.is_none() { continue; }
                if cfg.conditional_heartbeat {
                    let age = epoch_ms() - cfg.metrics.last_message_at_ms.load(Ordering::Relaxed);
                    if age <= hb.as_millis() as i64 { continue; } // traffic is its own keepalive
                }
                write.send(Message::Ping(Vec::new().into())).await?;
            }
            _ = watchdog.tick() => {
                if cfg.reconnect_due(shutdown) { return Ok(()); }
                if let Some(to) = cfg.stale_timeout {
                    let last = cfg.metrics.last_message_at_ms.load(Ordering::Relaxed);
                    let now = epoch_ms();
                    if last > 0 && now - last > to.as_millis() as i64 {
                        let stall_secs = (now - last) / 1000;
                        report(ErrorLevel::Warn, cfg.name, "tick_stalled",
                            format!("no frames for {}ms — reconnecting", now - last));
                        // 60s cooldown on the user-visible toast.
                        if now - stall_toast_at.load(Ordering::Relaxed) >= 60_000 {
                            stall_toast_at.store(now, Ordering::Relaxed);
                            report(ErrorLevel::Warn, cfg.name, "feed_stalled",
                                format!("{} silent for >{stall_secs}s — reconnecting", cfg.name));
                        }
                        return Ok(()); // force reconnect
                    }
                }
            }
            frame = read.next() => {
                let Some(msg) = frame else { return Ok(()); };
                let msg = msg?;
                cfg.metrics.last_message_at_ms.store(epoch_ms(), Ordering::Relaxed);
                if !msg.is_text() { continue; }
                cfg.metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                (cfg.on_text)(msg.to_text()?);
            }
        }
    }
}

impl SubscribedWsConfig {
    fn reconnect_due(&self, shutdown: &AtomicBool) -> bool {
        shutdown.load(Ordering::SeqCst)
    }
}

struct WsShutdown { name: &'static str, flag: Arc<AtomicBool> }

#[async_trait::async_trait]
impl connectivity::Shutdown for WsShutdown {
    async fn drain(&self, deadline: Duration) -> Result<(), String> {
        tracing::info!(target: "shutdown", connection = self.name, "drain invoked");
        self.flag.store(true, Ordering::SeqCst);
        // The loop polls the flag on its 250ms tick — give it a brief moment.
        let wait = deadline.min(Duration::from_millis(400));
        let start = Instant::now();
        while start.elapsed() < wait {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(())
    }
}
