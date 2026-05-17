//! Real-time crypto feed — connects to ApexCrypto WebSocket for live bar updates.
//!
//! Subscribes to chart timeframes + 1s for price tracking.
//! Pushes UpdateLastBar / AppendBar / WatchlistPrice to the chart renderer.

use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use crate::chart_renderer::{self, ChartCommand, Bar};
use crate::data::connectivity::{self, errors_sink::{report, ErrorLevel}, Backoff};

const APEX_CRYPTO_WS: &str = "ws://192.168.1.56:30840/ws";

static FEED_RUNNING: OnceLock<Mutex<bool>> = OnceLock::new();
static SHUTDOWN: OnceLock<Arc<AtomicBool>> = OnceLock::new();

pub fn start() {
    let running = FEED_RUNNING.get_or_init(|| Mutex::new(false));
    let mut guard = running.lock().unwrap();
    if *guard { return; }
    *guard = true;
    drop(guard);

    let shutdown = SHUTDOWN.get_or_init(|| Arc::new(AtomicBool::new(false))).clone();
    connectivity::register("crypto_feed", Arc::new(CryptoFeedShutdown { shutdown: shutdown.clone() }));

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            // Infinite reconnect — never give up on crypto.
            let mut backoff = Backoff::new().with_max_attempts(None);
            loop {
                if shutdown.load(Ordering::SeqCst) { break; }
                match run_feed().await {
                    Ok(()) => { backoff.reset(); }
                    Err(e) => {
                        report(ErrorLevel::Warn, "crypto_feed", "reconnect", e.to_string());
                    }
                }
                if shutdown.load(Ordering::SeqCst) { break; }
                if let Some(d) = backoff.next_delay() {
                    tokio::time::sleep(d).await;
                }
            }
        });
    });
}

struct CryptoFeedShutdown {
    shutdown: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl connectivity::Shutdown for CryptoFeedShutdown {
    async fn drain(&self, deadline: Duration) -> Result<(), String> {
        self.shutdown.store(true, Ordering::SeqCst);
        tokio::time::sleep(deadline.min(Duration::from_millis(200))).await;
        Ok(())
    }
}

async fn run_feed() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::{StreamExt, SinkExt};
    use tokio_tungstenite::connect_async;

    report(ErrorLevel::Info, "crypto_feed", "connecting", APEX_CRYPTO_WS);
    let (ws, _) = connect_async(APEX_CRYPTO_WS).await?;
    let (mut write, mut read) = ws.split();

    // Subscribe to chart timeframes + tape for T&S
    let sub_msg = serde_json::json!({
        "subscribe": ["*:1s", "*:1m", "*:5m", "*:15m", "*:30m", "*:1h", "*:4h", "*:1d"],
        "tape": ["*"]
    });
    write.send(tokio_tungstenite::tungstenite::Message::Text(
        sub_msg.to_string().into()
    )).await?;
    report(ErrorLevel::Info, "crypto_feed", "connected", "bars + tape subscribed");

    let mut chart_updates: u64 = 0;
    let mut price_updates: u64 = 0;
    let mut tape_updates: u64 = 0;
    let mut last_log = std::time::Instant::now();

    while let Some(msg) = read.next().await {
        let msg = msg?;
        if !msg.is_text() { continue; }
        let text = msg.to_text()?;

        // Parse trade tape entries
        let json: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(trade) = json.get("trade") {
            let symbol = trade["symbol"].as_str().unwrap_or("").to_string();
            let price = trade["price"].as_f64().unwrap_or(0.0) as f32;
            let qty = trade["qty"].as_f64().unwrap_or(0.0) as f32;
            let time = trade["time"].as_i64().unwrap_or(0);
            let is_buy = trade["side"].as_str() == Some("buy");
            send_to_charts(ChartCommand::TapeEntry { symbol, price, qty, time, is_buy });
            tape_updates += 1;
            continue;
        }

        if let Ok(update) = serde_json::from_value::<BarUpdateMsg>(json) {
            let bar = &update.bar;
            let is_1s = bar.timeframe == "1s";

            // 1s bars → watchlist price updates only (don't send to chart)
            if is_1s {
                let price_cmd = ChartCommand::WatchlistPrice {
                    symbol: bar.symbol.clone(),
                    price: bar.close as f32,
                    prev_close: bar.open as f32,
                };
                send_to_charts(price_cmd);
                price_updates += 1;
            } else {
                // >= 1m bars → chart updates
                let gpu_bar = Bar {
                    open: bar.open as f32,
                    high: bar.high as f32,
                    low: bar.low as f32,
                    close: bar.close as f32,
                    volume: bar.volume as f32,
                    _pad: 0.0,
                };
                let time_sec = bar.time / 1000;

                if update.is_closed {
                    send_to_charts(ChartCommand::AppendBar {
                        symbol: bar.symbol.clone(),
                        timeframe: bar.timeframe.clone(),
                        bar: gpu_bar,
                        timestamp: time_sec,
                        mark: false,
                    });
                } else {
                    let mut tick_bar = gpu_bar;
                    tick_bar.volume = 0.0;
                    send_to_charts(ChartCommand::UpdateLastBar {
                        symbol: bar.symbol.clone(),
                        timeframe: bar.timeframe.clone(),
                        bar: tick_bar,
                        mark: false,
                    });
                }
                chart_updates += 1;
            }

            if last_log.elapsed().as_secs() >= 30 {
                eprintln!("[crypto-feed] chart: {}/30s, prices: {}/30s, tape: {}/30s", chart_updates, price_updates, tape_updates);
                chart_updates = 0;
                price_updates = 0;
                tape_updates = 0;
                last_log = std::time::Instant::now();
            }
        }
    }

    Err("WebSocket closed".into())
}

fn send_to_charts(cmd: ChartCommand) {
    if let Some(lock) = crate::NATIVE_CHART_TXS.get() {
        if let Ok(mut guard) = lock.lock() {
            guard.retain(|tx| tx.send(cmd.clone()).is_ok());
        }
    }
    // Wake the UI from sleep so the tick is visible. With the catch-all
    // repaint removed, the egui loop only runs when something requests it.
    crate::wake_native_ui();
}

#[derive(serde::Deserialize)]
struct BarUpdateMsg {
    bar: CryptoBar,
    is_closed: bool,
}

#[derive(serde::Deserialize)]
struct CryptoBar {
    symbol: String,
    timeframe: String,
    time: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}
