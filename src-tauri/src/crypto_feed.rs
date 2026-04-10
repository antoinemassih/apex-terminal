//! Real-time crypto feed — connects to ApexCrypto WebSocket for live bar updates.
//!
//! Subscribes to all crypto symbols in the watchlist and pushes
//! UpdateLastBar / AppendBar commands to the chart renderer.

use std::sync::{Mutex, OnceLock};
use crate::chart_renderer::{self, ChartCommand, Bar};

const APEX_CRYPTO_WS: &str = "ws://localhost:8400/ws";

static FEED_RUNNING: OnceLock<Mutex<bool>> = OnceLock::new();

/// Start the crypto feed in a background thread. Safe to call multiple times — only starts once.
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

    // Subscribe to all timeframes for all symbols
    let sub_msg = serde_json::json!({
        "subscribe": ["*"]
    });
    write.send(tokio_tungstenite::tungstenite::Message::Text(
        sub_msg.to_string().into()
    )).await?;
    eprintln!("[crypto-feed] Connected — subscribed to all streams");

    let mut count: u64 = 0;
    let mut last_log = std::time::Instant::now();

    while let Some(msg) = read.next().await {
        let msg = msg?;
        if !msg.is_text() { continue; }
        let text = msg.to_text()?;

        // Parse BarUpdate from ApexCrypto
        if let Ok(update) = serde_json::from_str::<BarUpdateMsg>(text) {
            let bar = &update.bar;

            // Convert to chart renderer Bar (f32, time in seconds)
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
                // Bar closed → append new bar to chart
                let cmd = ChartCommand::AppendBar {
                    symbol: bar.symbol.clone(),
                    timeframe: bar.timeframe.clone(),
                    bar: gpu_bar,
                    timestamp: time_sec,
                };
                send_to_charts(cmd);
            } else {
                // Live tick → update current bar
                let cmd = ChartCommand::UpdateLastBar {
                    symbol: bar.symbol.clone(),
                    timeframe: bar.timeframe.clone(),
                    bar: gpu_bar,
                };
                send_to_charts(cmd);
            }

            // Also push watchlist price updates
            let price_cmd = ChartCommand::WatchlistPrice {
                symbol: bar.symbol.clone(),
                price: bar.close as f32,
                prev_close: bar.open as f32,
            };
            send_to_charts(price_cmd);

            count += 1;
            if last_log.elapsed().as_secs() >= 30 {
                eprintln!("[crypto-feed] {} updates/30s", count);
                count = 0;
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

/// ApexCrypto bar update message format
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
