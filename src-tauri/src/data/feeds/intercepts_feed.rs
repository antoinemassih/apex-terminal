//! Live drawing-interception feed.
//!
//! Connects to apex-data `/ws/intercepts` and surfaces backend-computed
//! interception events (price breaking / retesting an auto-drawn line) in the
//! terminal's alert-badge feed. This is the read side of the auto-drawing feed:
//! the backend (apex-data `InterceptEngine`) computes interceptions across the
//! whole universe — independent of whether a chart is open — and streams them;
//! here we just render the significant ones as notifications.
//!
//! Global socket (no per-symbol dialing), mirroring `dom_feed`'s LAN-aware
//! connect. Noisy phases (approach/touch/bounce/cross) are filtered out by
//! default — only `break` and `retest` reach the badge feed (override via
//! `INTERCEPTS_FEED_EVENTS`, a CSV of event names).

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::chart_renderer::ui::components::toolbar::alert_feed;
use crate::chart_renderer::ui::tools::notification::AlertKind;

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();
/// Only surface interceptions newer than this (epoch ms) — set at startup so the
/// WS replay of recent history doesn't spam the feed, and reconnects don't
/// re-push already-seen events.
static SINCE_MS: AtomicI64 = AtomicI64::new(0);

fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("intercepts_feed tokio runtime")
    })
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Which interception events to surface as badges (default: the actionable ones).
fn surfaced_events() -> Vec<String> {
    std::env::var("INTERCEPTS_FEED_EVENTS")
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_else(|| vec!["break".into(), "retest".into()])
}

fn intercepts_ws_url() -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    format!("{host}/ws/intercepts")
}

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// LAN-aware connect, mirroring `dom_feed`: when `APEX_DATA_LAN_IP` is set,
/// public DNS points at a non-routable IP, so dial the homelab Traefik IP
/// directly (plain ws) with the original Host-bearing request.
async fn connect_lan_aware(url: &str)
    -> Result<WsStream, Box<dyn std::error::Error + Send + Sync>>
{
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::{client_async, connect_async, MaybeTlsStream};
    use tokio::net::TcpStream;
    use crate::data::feeds::apex_data::config;

    let req = url.into_client_request()?;
    let lan = match (config::apex_lan_ip(), config::apex_host_port()) {
        (Some(ip), Some((_h, port))) => Some((ip, port)),
        _ => None,
    };
    if let Some((ip, port)) = lan {
        let stream = TcpStream::connect((ip.as_str(), port)).await?;
        let (ws, _) = client_async(req, MaybeTlsStream::Plain(stream)).await?;
        Ok(ws)
    } else {
        let (ws, _) = connect_async(req).await?;
        Ok(ws)
    }
}

/// Spawn the feed thread. Idempotent — safe to call once at startup. Opt-out via
/// `INTERCEPTS_FEED_ENABLED=0`.
pub fn start() {
    if std::env::var("INTERCEPTS_FEED_ENABLED").map(|v| v == "0").unwrap_or(false) {
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }
    SINCE_MS.store(now_ms(), Ordering::SeqCst);
    let surfaced = surfaced_events();
    std::thread::spawn(move || {
        rt().block_on(async move {
            let mut backoff = Duration::from_secs(1);
            loop {
                match run_once(&surfaced).await {
                    Ok(()) => backoff = Duration::from_secs(1),
                    Err(e) => {
                        tracing::warn!(target: "intercepts_feed", "reconnect: {e}");
                    }
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        });
    });
}

async fn run_once(surfaced: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    let url = intercepts_ws_url();
    tracing::info!(target: "intercepts_feed", "connecting {url}");
    let ws = connect_lan_aware(&url).await?;
    let (_write, mut read) = ws.split();

    while let Some(msg) = read.next().await {
        let txt = match msg? {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(&txt) else { continue };
        let event = ev.get("event").and_then(|v| v.as_str()).unwrap_or("");
        if !surfaced.iter().any(|e| e == &event.to_lowercase()) {
            continue;
        }
        let ts = ev.get("ts_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        if ts <= SINCE_MS.load(Ordering::SeqCst) {
            continue; // historical replay / already past — don't badge
        }
        let symbol = ev.get("symbol").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        let dtype = ev.get("dtype").and_then(|v| v.as_str()).unwrap_or("line");
        let tf = ev.get("tf").and_then(|v| v.as_str()).unwrap_or("");
        let price = ev.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let kind = match event {
            "break" => AlertKind::Warning,
            _ => AlertKind::Signal,
        };
        let msg = format!("{event} {dtype} {tf} @ {price:.2}");
        alert_feed::push(kind, Some(symbol), msg);
        crate::wake_native_ui();
    }
    Ok(())
}
