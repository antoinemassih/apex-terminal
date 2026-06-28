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
    use futures_util::StreamExt;
    let ws = connect_lan_aware(url).await?;
    let (_w, mut read) = ws.split();
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
