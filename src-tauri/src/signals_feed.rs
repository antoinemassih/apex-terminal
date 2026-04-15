//! Real-time signals feed — connects to ApexSignals WebSocket for patterns, alerts, and trendlines.
//!
//! Subscribes to patterns/alerts/trendlines/significance channels.
//! Pushes PatternLabels / AlertTriggered / AutoTrendlines / SignificanceUpdate to the chart renderer.

use std::sync::{Mutex, OnceLock};
use crate::chart_renderer::{ChartCommand, PatternLabel};

const APEX_SIGNALS_WS: &str = "ws://localhost:8200/ws";

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
                    eprintln!("[signals-feed] Error: {e} — reconnecting in 5s");
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
    });
}

async fn run_feed() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::{StreamExt, SinkExt};
    use tokio_tungstenite::connect_async;

    eprintln!("[signals-feed] Connecting to {}", APEX_SIGNALS_WS);
    let (ws, _) = connect_async(APEX_SIGNALS_WS).await?;
    let (mut write, mut read) = ws.split();

    // Subscribe to all signal channels
    let sub_msg = serde_json::json!({
        "subscribe": ["patterns", "alerts", "trendlines", "significance"]
    });
    write.send(tokio_tungstenite::tungstenite::Message::Text(
        sub_msg.to_string().into()
    )).await?;
    eprintln!("[signals-feed] Connected — subscribed to patterns/alerts/trendlines/significance");

    while let Some(msg) = read.next().await {
        let msg = msg?;
        if !msg.is_text() { continue; }
        let text = msg.to_text()?;

        let json: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let channel = json.get("channel").and_then(|c| c.as_str()).unwrap_or("");
        let symbol = match json.get("symbol").and_then(|s| s.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        match channel {
            "patterns" => {
                let labels: Vec<PatternLabel> = json.get("labels")
                    .and_then(|l| l.as_array())
                    .map(|arr| {
                        arr.iter().filter_map(|item| {
                            Some(PatternLabel {
                                time: item.get("time")?.as_i64()?,
                                label: item.get("label")?.as_str()?.to_string(),
                                bullish: item.get("bullish").and_then(|b| b.as_bool()).unwrap_or(true),
                                confidence: item.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.5) as f32,
                            })
                        }).collect()
                    })
                    .unwrap_or_default();
                send_to_charts(ChartCommand::PatternLabels { symbol, labels });
            }
            "alerts" => {
                let alert_id = json.get("alert_id").and_then(|a| a.as_str()).unwrap_or("").to_string();
                let price = json.get("price").and_then(|p| p.as_f64()).unwrap_or(0.0) as f32;
                let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("Alert triggered").to_string();
                send_to_charts(ChartCommand::AlertTriggered { symbol, alert_id, price, message });
            }
            "trendlines" => {
                let drawings_json = json.get("drawings")
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "[]".to_string());
                send_to_charts(ChartCommand::AutoTrendlines { symbol, drawings_json });
            }
            "significance" => {
                let drawing_id = json.get("drawing_id").and_then(|d| d.as_str()).unwrap_or("").to_string();
                let score = json.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
                let touches = json.get("touches").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                let strength = json.get("strength").and_then(|s| s.as_str()).unwrap_or("WEAK").to_string();
                send_to_charts(ChartCommand::SignificanceUpdate { symbol, drawing_id, score, touches, strength });
            }
            _ => {} // unknown channel — ignore
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
