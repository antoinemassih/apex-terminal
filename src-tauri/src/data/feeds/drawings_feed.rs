//! Live auto-drawing feed trigger.
//!
//! Subscribes to apex-data `/ws/drawings` (a global add/update/remove stream).
//! When a drawing changes for the *active* chart symbol/timeframe AND the user
//! has enabled the unified feed (`AutoDrawConfig.live_feed`), this re-pulls the
//! full drawing set via the existing `fetch_apexsignals_drawings` path (which,
//! in live_feed mode, reads apex-data `/api/drawings`). So the chart's
//! auto-drawings refresh live as the backend recomputes them at bar close.
//!
//! When `live_feed` is off (default), this is inert — the tuned per-chart
//! ApexSignals fetch owns the drawings and honours the AUTO-CHARTING panel knobs.
//!
//! Refreshes are debounced (a recompute cycle emits many events) and only fire
//! for the active symbol, so an idle/background symbol never triggers a re-pull.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();
static ACTIVE: OnceLock<Mutex<Option<(String, String)>>> = OnceLock::new();
static LAST_REFRESH_MS: AtomicI64 = AtomicI64::new(0);

const DEBOUNCE_MS: i64 = 1500;

fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("drawings_feed tokio runtime")
    })
}

fn active() -> &'static Mutex<Option<(String, String)>> {
    ACTIVE.get_or_init(|| Mutex::new(None))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Point the live-drawing trigger at the active chart symbol/timeframe. Cheap;
/// just updates the filter the WS loop matches against.
pub fn set_target(symbol: &str, timeframe: &str) {
    let sym = symbol.strip_prefix("F:").unwrap_or(symbol).to_string();
    *active().lock() = Some((sym, timeframe.to_string()));
}

fn drawings_ws_url() -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    format!("{host}/ws/drawings")
}

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// LAN-aware connect (mirrors `dom_feed` / `intercepts_feed`).
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

/// Spawn the feed thread. Idempotent. Opt-out via `DRAWINGS_FEED_ENABLED=0`.
pub fn start() {
    if std::env::var("DRAWINGS_FEED_ENABLED").map(|v| v == "0").unwrap_or(false) {
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }
    std::thread::spawn(move || {
        rt().block_on(async move {
            let mut backoff = Duration::from_secs(1);
            loop {
                match run_once().await {
                    Ok(()) => backoff = Duration::from_secs(1),
                    Err(e) => tracing::warn!(target: "drawings_feed", "reconnect: {e}"),
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        });
    });
}

async fn run_once() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    let url = drawings_ws_url();
    tracing::info!(target: "drawings_feed", "connecting {url}");
    let ws = connect_lan_aware(&url).await?;
    let (_write, mut read) = ws.split();

    while let Some(msg) = read.next().await {
        let txt = match msg? {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(&txt) else { continue };
        let sym = ev.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
        let tf = ev.get("tf").and_then(|v| v.as_str()).unwrap_or("");

        // Only react to the active symbol/tf, and only when the unified feed is on.
        let matches_active = active().lock().as_ref().map(|(s, t)| s == sym && t == tf).unwrap_or(false);
        if !matches_active { continue; }
        if !crate::chart_renderer::gpu::auto_draw_config().live_feed { continue; }

        // Debounce: a recompute cycle emits many events; coalesce into one re-pull.
        let now = now_ms();
        if now - LAST_REFRESH_MS.load(Ordering::Relaxed) < DEBOUNCE_MS { continue; }
        LAST_REFRESH_MS.store(now, Ordering::Relaxed);

        // Re-pull the full set (fetch spawns its own worker + wakes the UI).
        crate::chart_renderer::gpu::fetch_apexsignals_drawings(sym.to_string(), tf.to_string());
    }
    Ok(())
}
