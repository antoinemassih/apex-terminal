//! Real-time crypto feed — connects to ApexCrypto WebSocket for live bar updates.
//!
//! Subscribes to chart timeframes + 1s for price tracking.
//! Pushes UpdateLastBar / AppendBar / WatchlistPrice to the chart renderer.

use std::sync::{Mutex, OnceLock};
use crate::chart_renderer::{self, ChartCommand, Bar};

const APEX_CRYPTO_WS: &str = "ws://192.168.1.56:30840/ws";

static FEED_RUNNING: OnceLock<Mutex<bool>> = OnceLock::new();

pub fn start() {
    let running = FEED_RUNNING.get_or_init(|| Mutex::new(false));
    let mut guard = running.lock().unwrap();
    if *guard { return; }
    *guard = true;
    drop(guard);

    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            loop {
                if let Err(e) = run_feed().await {
                    eprintln!("[crypto-feed] Error: {e} — reconnecting in 3s");
                }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        });
    });
}

async fn run_feed() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::{StreamExt, SinkExt};
    use tokio_tungstenite::connect_async;

    eprintln!("[crypto-feed] Connecting to {}", APEX_CRYPTO_WS);
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
    eprintln!("[crypto-feed] Connected — bars + tape subscribed");

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
                    });
                } else {
                    let mut tick_bar = gpu_bar;
                    tick_bar.volume = 0.0;
                    send_to_charts(ChartCommand::UpdateLastBar {
                        symbol: bar.symbol.clone(),
                        timeframe: bar.timeframe.clone(),
                        bar: tick_bar,
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
