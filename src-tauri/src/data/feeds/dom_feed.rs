//! Live DOM (L2 depth-of-market) feed.
//!
//! Connects to apex-data `/ws/dom?symbol=X&rows=30` and pushes a merged,
//! price-keyed ladder into the chart renderer via `ChartCommand::DomLevels`.
//!
//! Unlike `signals_feed` (one global wildcard socket), the DOM endpoint is
//! per-symbol — the symbol lives in the URL — so this feed re-dials whenever
//! the active chart symbol changes. Call `set_symbol()` on every symbol switch
//! (and once on startup); the connect loop observes the change and reconnects.

use std::sync::{Arc, OnceLock};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::chart_renderer::{ChartCommand, ui::panels::dom_panel::DomLevel};

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static ACTIVE_SYMBOL: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static RECONNECT: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();

fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("dom_feed tokio runtime")
    })
}

fn active() -> &'static Mutex<Option<String>> {
    ACTIVE_SYMBOL.get_or_init(|| Mutex::new(None))
}

fn reconnect_flag() -> &'static Arc<AtomicBool> {
    RECONNECT.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

/// Epoch milliseconds. Shared with the renderer's live-vs-mock gate so the
/// staleness window uses one clock.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Point the DOM feed at `symbol` (the chart symbol). Trips a reconnect only if
/// it actually changed. The connect loop re-dials the new symbol's `/ws/dom`.
pub fn set_symbol(symbol: &str) {
    let mut g = active().lock();
    if g.as_deref() == Some(symbol) {
        return;
    }
    *g = Some(symbol.to_string());
    reconnect_flag().store(true, Ordering::SeqCst);
}

/// Build the `/ws/dom` URL from the shared apex-data base. `apex_ws_url()`
/// returns `ws(s)://host/ws?format=json`; we keep the scheme+host and swap the
/// path so DOM honours the same `APEX_DATA_URL` override as the main feed.
fn dom_ws_url(symbol: &str) -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-dev.xllio.com");
    // Strip the F: futures class tag → backend wants the bare root (F:ES → ES).
    let symbol = symbol.strip_prefix("F:").unwrap_or(symbol);
    // rows is clamped 1..20 server-side; ask for the max.
    format!("{host}/ws/dom?symbol={symbol}&rows=20")
}

type DomWsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Connect to `url`, honouring the same LAN-IP override the main apex_data WS
/// uses. When `APEX_DATA_LAN_IP` is set, public DNS resolves the apex-data host
/// to a non-routable IP on the LAN, so dial the homelab Traefik IP directly
/// (plain ws) and hand the socket to `client_async` with the original
/// Host-bearing request — keeping Traefik ingress routing intact. Without this
/// the DOM socket silently fails to connect from inside the LAN.
async fn connect_lan_aware(url: &str)
    -> Result<DomWsStream, Box<dyn std::error::Error + Send + Sync>>
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
        tracing::info!(target: "dom_feed", "lan_override dial {ip}:{port}");
        let stream = TcpStream::connect((ip.as_str(), port)).await?;
        let (ws, _) = client_async(req, MaybeTlsStream::Plain(stream)).await?;
        Ok(ws)
    } else {
        let (ws, _) = connect_async(req).await?;
        Ok(ws)
    }
}

/// Spawn the feed thread. Idempotent — safe to call once at startup.
pub fn start() {
    if STARTED.set(()).is_err() {
        return;
    }
    let reconnect = reconnect_flag().clone();
    std::thread::spawn(move || {
        rt().block_on(async move {
            loop {
                let sym = active().lock().clone();
                let Some(sym) = sym else {
                    // No symbol selected yet — idle until one arrives.
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                };
                reconnect.store(false, Ordering::SeqCst);
                if let Err(e) = run_one(&sym, &reconnect).await {
                    tracing::warn!(target: "dom_feed", "{sym}: {e}");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        });
    });
}

async fn run_one(
    symbol: &str,
    reconnect: &AtomicBool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::StreamExt;

    let url = dom_ws_url(symbol);
    tracing::info!(target: "dom_feed", "connecting {url}");
    let ws = connect_lan_aware(&url).await?;
    let (_w, mut read) = ws.split();

    // 250ms tick lets us notice a symbol switch promptly even on a quiet book.
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if reconnect.load(Ordering::SeqCst) {
                    // Symbol changed — outer loop re-dials the new one.
                    return Ok(());
                }
            }
            frame = read.next() => {
                let Some(msg) = frame else { return Ok(()); };
                let msg = msg?;
                if !msg.is_text() { continue; }
                let v: serde_json::Value = match serde_json::from_str(msg.to_text()?) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("type").and_then(|t| t.as_str()) {
                    Some("dom") => {
                        let levels = parse_dom(&v);
                        send(ChartCommand::DomLevels { symbol: symbol.to_string(), levels });
                    }
                    Some("error") => {
                        tracing::warn!(target: "dom_feed", "server error: {v}");
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Merge the `bids` + `asks` arrays into a single price-keyed ladder, sorted
/// high → low (the order the DOM panel renders). L2 depth carries no traded
/// volume, so `volume` stays 0 and `delta` is the resting bid/ask imbalance.
fn parse_dom(v: &serde_json::Value) -> Vec<DomLevel> {
    use std::collections::BTreeMap;

    // Key by price*1000 (integer) so floats sort deterministically.
    let mut book: BTreeMap<i64, DomLevel> = BTreeMap::new();
    let empty = Vec::new();

    let read_side = |side: &str| -> Vec<serde_json::Value> {
        v.get(side)
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_else(|| empty.clone())
    };

    for lvl in read_side("bids") {
        let p = lvl.get("price").and_then(|x| x.as_f64()).unwrap_or(0.0);
        // Sizes arrive as JSON floats (e.g. `12.0`); as_u64() returns None on
        // those, so read as f64 and round.
        let s = lvl.get("size").and_then(|x| x.as_f64()).unwrap_or(0.0).round() as u32;
        let e = book.entry((p * 1000.0) as i64).or_insert(DomLevel {
            price: p as f32, bid_size: 0, ask_size: 0, volume: 0, delta: 0,
        });
        e.bid_size = s;
    }
    for lvl in read_side("asks") {
        let p = lvl.get("price").and_then(|x| x.as_f64()).unwrap_or(0.0);
        // Sizes arrive as JSON floats (e.g. `12.0`); as_u64() returns None on
        // those, so read as f64 and round.
        let s = lvl.get("size").and_then(|x| x.as_f64()).unwrap_or(0.0).round() as u32;
        let e = book.entry((p * 1000.0) as i64).or_insert(DomLevel {
            price: p as f32, bid_size: 0, ask_size: 0, volume: 0, delta: 0,
        });
        e.ask_size = s;
    }

    // BTreeMap iterates ascending; the ladder draws high → low, so reverse.
    book.into_values()
        .rev()
        .map(|mut l| {
            l.delta = l.bid_size as i64 - l.ask_size as i64;
            l
        })
        .collect()
}

fn send(cmd: ChartCommand) {
    if let Some(lock) = crate::NATIVE_CHART_TXS.get() {
        if let Ok(mut g) = lock.lock() {
            g.retain(|tx| tx.send(cmd.clone()).is_ok());
        }
    }
    crate::wake_native_ui();
}
