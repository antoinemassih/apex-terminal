//! Live futures bar feed.
//!
//! Futures are an IBKR source (not on the Polygon `/ws` firehose), so their
//! live candles come from ApexIB's 5-second `reqRealTimeBars`, proxied by
//! apex-data at `/ws/futures?symbol=ES`. This client folds each 5s print into
//! the chart's current candle (`UpdateLastBar`) and updates the watchlist
//! (`WatchlistPrice`).
//!
//! Activates ONLY for `F:`-tagged futures symbols; any non-futures target
//! deactivates it. Re-dials on symbol change, like `dom_feed`.

use std::sync::{Arc, OnceLock};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::chart_renderer::{Bar, ChartCommand};

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
/// Active target: `(canonical_symbol_with_F_tag, timeframe)`. `None` = idle.
static TARGET: OnceLock<Mutex<Option<(String, String)>>> = OnceLock::new();
static RECONNECT: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();

fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("futures_feed tokio runtime")
    })
}
fn target() -> &'static Mutex<Option<(String, String)>> { TARGET.get_or_init(|| Mutex::new(None)) }
fn reconnect_flag() -> &'static Arc<AtomicBool> { RECONNECT.get_or_init(|| Arc::new(AtomicBool::new(false))) }

/// Point the futures feed at `(symbol, timeframe)`. Only `F:`-tagged symbols
/// activate the feed; anything else deactivates it (stocks/options stream via
/// the Polygon `/ws`). Idempotent. Call on every chart symbol/timeframe change.
pub fn set_target(symbol: &str, timeframe: &str) {
    let mut g = target().lock();
    let next = if symbol.starts_with("F:") {
        Some((symbol.to_string(), timeframe.to_string()))
    } else {
        None
    };
    if *g == next {
        return;
    }
    *g = next;
    reconnect_flag().store(true, Ordering::SeqCst);
}

/// Build the `/ws/futures` URL from the shared apex-data base. Strips the `F:`
/// class tag → the backend wants the bare root (`F:ES` → `ES`).
fn futures_ws_url(symbol: &str) -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    let root = symbol.strip_prefix("F:").unwrap_or(symbol);
    format!("{host}/ws/futures?symbol={root}")
}

type FutWsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// LAN-aware connect — mirrors `dom_feed::connect_lan_aware`.
async fn connect_lan_aware(url: &str)
    -> Result<FutWsStream, Box<dyn std::error::Error + Send + Sync>>
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

/// Spawn the feed thread. Idempotent — safe to call once at startup.
pub fn start() {
    if STARTED.set(()).is_err() {
        return;
    }
    let reconnect = reconnect_flag().clone();
    std::thread::spawn(move || {
        rt().block_on(async move {
            loop {
                let tgt = target().lock().clone();
                let Some((sym, tf)) = tgt else {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                };
                reconnect.store(false, Ordering::SeqCst);
                if let Err(e) = run_one(&sym, &tf, &reconnect).await {
                    tracing::warn!(target: "futures_feed", "{sym}: {e}");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        });
    });
}

async fn run_one(
    symbol: &str,
    timeframe: &str,
    reconnect: &AtomicBool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::StreamExt;

    let url = futures_ws_url(symbol);
    tracing::info!(target: "futures_feed", "connecting {url}");
    let ws = connect_lan_aware(&url).await?;
    let (_w, mut read) = ws.split();

    let mut tick = tokio::time::interval(Duration::from_millis(250));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if reconnect.load(Ordering::SeqCst) {
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
                    Some("bar") => emit_bar(&v, symbol, timeframe),
                    Some("error") => tracing::warn!(target: "futures_feed", "server error: {v}"),
                    _ => {}
                }
            }
        }
    }
}

/// Fold a 5s `bar` frame into the chart's current candle + the watchlist.
fn emit_bar(v: &serde_json::Value, symbol: &str, timeframe: &str) {
    let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
    let (open, high, low, close, volume) = (f("open"), f("high"), f("low"), f("close"), f("volume"));
    if close <= 0.0 {
        return;
    }
    let time_sec = v.get("time").and_then(|x| x.as_i64()).unwrap_or(0);

    // Watchlist price (keyed by the canonical F: symbol the watchlist stores).
    send(ChartCommand::WatchlistPrice {
        symbol: symbol.to_string(),
        price: close as f32,
        prev_close: open as f32,
        day_close: 0.0, // futures: 24h, no equity-style regular close
    });

    // Fold into the current chart candle. `cumulative: false` = incremental
    // tick model (volume +=, high/low via close) — the IB/crypto path. Volume
    // is carried so the building candle accrues it.
    let bar = Bar {
        open: open as f32,
        high: high as f32,
        low: low as f32,
        close: close as f32,
        volume: volume as f32,
        _pad: 0.0,
    };
    send(ChartCommand::UpdateLastBar {
        symbol: symbol.to_string(),
        timeframe: timeframe.to_string(),
        bar,
        timestamp: time_sec,
        mark: false,
        cumulative: false,
    });
}

fn send(cmd: ChartCommand) {
    if let Some(lock) = crate::NATIVE_CHART_TXS.get() {
        if let Ok(mut g) = lock.lock() {
            g.retain(|tx| tx.send(cmd.clone()).is_ok());
        }
    }
    crate::wake_native_ui();
}
