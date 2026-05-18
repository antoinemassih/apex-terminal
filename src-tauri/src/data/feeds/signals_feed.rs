//! Real-time signals feed — connects to ApexSignals WebSocket for patterns, alerts, and trendlines.
//!
//! Subscribes to patterns/alerts/trendlines/significance channels.
//! Pushes PatternLabels / AlertTriggered / AutoTrendlines / SignificanceUpdate to the chart renderer.

use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

// ── Wave 6: per-feed metrics counters ────────────────────────────────────────
pub(crate) static MESSAGES_IN:        AtomicU64 = AtomicU64::new(0);
pub(crate) static PARSE_ERRORS:       AtomicU64 = AtomicU64::new(0);
pub(crate) static RECONNECT_COUNT:    AtomicU32 = AtomicU32::new(0);
pub(crate) static LAST_MESSAGE_AT_MS: AtomicI64 = AtomicI64::new(0);

// ── Wave 7E: WS resilience ───────────────────────────────────────────────────
// Signals traffic is bursty (patterns + alerts on bar-close) — quiet for
// minutes is normal, so the heartbeat is unconditional, not conditional.
const HEARTBEAT_SECS: u64 = 30;
const STALE_TIMEOUT_MS: i64 = 60_000; // signals can be legitimately quiet
const WATCHDOG_TICK_SECS: u64 = 10;
pub(crate) static FORCE_RECONNECT: AtomicBool = AtomicBool::new(false);
pub(crate) static LAST_STALL_AT_MS: AtomicI64 = AtomicI64::new(0);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
use crate::chart_renderer::{ChartCommand, PatternLabel};
use crate::data::connectivity::{self, errors_sink::{report, ErrorLevel}, Backoff};

const APEX_SIGNALS_WS: &str = "ws://localhost:8200/ws";

static FEED_RUNNING: OnceLock<Mutex<bool>> = OnceLock::new();
static SHUTDOWN: OnceLock<Arc<AtomicBool>> = OnceLock::new();

pub fn start() {
    let running = FEED_RUNNING.get_or_init(|| Mutex::new(false));
    let mut guard = running.lock().unwrap();
    if *guard { return; }
    *guard = true;
    drop(guard);

    let shutdown = SHUTDOWN.get_or_init(|| Arc::new(AtomicBool::new(false))).clone();
    connectivity::register("signals_feed", Arc::new(SignalsFeedShutdown { shutdown: shutdown.clone() }));

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            tokio::spawn(run_watchdog());
            let mut backoff = Backoff::new().with_max_attempts(None);
            let mut first = true;
            loop {
                if shutdown.load(Ordering::SeqCst) { break; }
                if !first { RECONNECT_COUNT.fetch_add(1, Ordering::Relaxed); }
                first = false;
                match run_feed().await {
                    Ok(()) => { backoff.reset(); }
                    Err(e) => {
                        report(ErrorLevel::Warn, "signals_feed", "reconnect", e.to_string());
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

struct SignalsFeedShutdown {
    shutdown: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl connectivity::Shutdown for SignalsFeedShutdown {
    async fn drain(&self, deadline: Duration) -> Result<(), String> {
        tracing::info!(target: "shutdown", connection = "signals_feed", "drain invoked");
        self.shutdown.store(true, Ordering::SeqCst);
        tokio::time::sleep(deadline.min(Duration::from_millis(200))).await;
        Ok(())
    }
}

async fn run_feed() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures_util::{StreamExt, SinkExt};
    use tokio_tungstenite::connect_async;

    report(ErrorLevel::Info, "signals_feed", "connecting", APEX_SIGNALS_WS);
    let (ws, _) = connect_async(APEX_SIGNALS_WS).await?;
    let (mut write, mut read) = ws.split();

    // Wave 7E: trigger gap-fill on every reconnect (skip initial connect).
    // Signals don't push bars directly, but the SubscriptionManager fanout is
    // shared with bar subs — calling here keeps the contract uniform.
    if RECONNECT_COUNT.load(Ordering::Relaxed) > 0 {
        tokio::spawn(async {
            let mgr = crate::data::providers::registry::subscription_manager();
            let n = mgr.gap_fill_on_reconnect_all().await;
            if n > 0 {
                report(
                    ErrorLevel::Info,
                    "signals_feed",
                    "gap_fill",
                    format!("replayed {n} bars after reconnect"),
                );
            }
        });
    }

    // Subscribe to all signal channels
    let sub_msg = serde_json::json!({
        "subscribe": ["patterns", "alerts", "trendlines", "significance"]
    });
    write.send(tokio_tungstenite::tungstenite::Message::Text(
        sub_msg.to_string().into()
    )).await?;
    report(ErrorLevel::Info, "signals_feed", "connected", "patterns/alerts/trendlines/significance");

    // Wave 7E: heartbeat + force-reconnect plumbing.
    FORCE_RECONNECT.store(false, Ordering::SeqCst);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECS));
    heartbeat.tick().await;
    let mut force_check = tokio::time::interval(Duration::from_secs(1));
    force_check.tick().await;

    loop {
        if FORCE_RECONNECT.swap(false, Ordering::SeqCst) {
            report(ErrorLevel::Warn, "signals_feed", "force_reconnect", "watchdog tripped");
            return Err("watchdog forced reconnect".into());
        }
        let msg = tokio::select! {
            _ = heartbeat.tick() => {
                if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Ping(Vec::new().into())).await {
                    report(ErrorLevel::Warn, "signals_feed", "ping_failed", e.to_string());
                    return Err(e.into());
                }
                continue;
            }
            _ = force_check.tick() => { continue; }
            frame = read.next() => match frame {
                Some(m) => m?,
                None => break,
            }
        };
        if !msg.is_text() {
            LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);
            continue;
        }
        let text = msg.to_text()?;

        MESSAGES_IN.fetch_add(1, Ordering::Relaxed);
        LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);

        let json: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => { PARSE_ERRORS.fetch_add(1, Ordering::Relaxed); continue; }
        };

        // Wave 8c: signals feed is wildcard-subscribed by channel
        // ("patterns" / "alerts" / "trendlines" / "significance") and the
        // frames are pattern/alert/trendline events — not bar updates.
        // There is no (symbol, timeframe) bumper that fits here; gap-fill
        // does not apply to signals state. Intentionally no SubscriptionManager
        // bump call. The reconnect hook above is still useful because the
        // shared manager may have bar subs from other feeds.
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

// ── Wave 7E: tick-age watchdog ───────────────────────────────────────────────

async fn run_watchdog() {
    let mut tick = tokio::time::interval(Duration::from_secs(WATCHDOG_TICK_SECS));
    loop {
        tick.tick().await;
        let last = LAST_MESSAGE_AT_MS.load(Ordering::Relaxed);
        let now = now_ms();
        if last > 0 && now - last > STALE_TIMEOUT_MS && !FORCE_RECONNECT.load(Ordering::Relaxed) {
            LAST_STALL_AT_MS.store(now, Ordering::Relaxed);
            report(
                ErrorLevel::Warn,
                "signals_feed",
                "tick_stalled",
                format!("no frames for {}ms — forcing reconnect", now - last),
            );
            FORCE_RECONNECT.store(true, Ordering::SeqCst);
        }
    }
}

fn send_to_charts(cmd: ChartCommand) {
    if let Some(lock) = crate::NATIVE_CHART_TXS.get() {
        if let Ok(mut guard) = lock.lock() {
            guard.retain(|tx| tx.send(cmd.clone()).is_ok());
        }
    }
    crate::wake_native_ui();
}
