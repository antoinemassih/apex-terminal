//! Real-time signals feed — connects to ApexSignals WebSocket for patterns, alerts, and trendlines.
//!
//! Subscribes to patterns/alerts/trendlines/significance channels.
//! Pushes PatternLabels / AlertTriggered / AutoTrendlines / SignificanceUpdate to the chart renderer.

use std::sync::{Arc, OnceLock};
use parking_lot::Mutex;
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
/// Timestamp (ms) of the last stale-feed toast emitted (60s cooldown).
static LAST_STALL_TOAST_AT: AtomicI64 = AtomicI64::new(0);
const STALL_TOAST_COOLDOWN_MS: i64 = 60_000;

// ── Wave 11c: ConnectionState push-notification stream ───────────────────────
static STATE_TX: OnceLock<tokio::sync::broadcast::Sender<ConnectionState>> = OnceLock::new();

pub fn state_tx() -> &'static tokio::sync::broadcast::Sender<ConnectionState> {
    STATE_TX.get_or_init(|| tokio::sync::broadcast::channel(64).0)
}

pub(crate) fn publish_state(s: ConnectionState) {
    let _ = state_tx().send(s);
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
use crate::chart_renderer::{ChartCommand, PatternLabel};
use crate::data::connectivity::{self, errors_sink::{report, ErrorLevel}, Backoff, ConnectionState};

/// ApexSignals WebSocket URL. ApexSignals binds REST + WS on one port
/// (`SIGNALS_API_PORT`, default 8100). Override here with `APEX_SIGNALS_WS`
/// when the engine runs elsewhere (e.g. K3s service DNS).
fn apex_signals_ws() -> String {
    std::env::var("APEX_SIGNALS_WS").unwrap_or_else(|_| "ws://localhost:8100/ws".to_string())
}

static FEED_RUNNING: OnceLock<Mutex<bool>> = OnceLock::new();
static SHUTDOWN: OnceLock<Arc<AtomicBool>> = OnceLock::new();
/// Audit fix Wave 8: shared single-thread runtime — created once, reused for
/// every `start()` call (safe because `start()` is guarded by FEED_RUNNING).
/// This avoids a new tokio Runtime being built on every reconnect-triggered
/// re-entry and eliminates one OS thread per invocation.
static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn feed_runtime() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("signals_feed tokio runtime")
    })
}

pub fn start() {
    let running = FEED_RUNNING.get_or_init(|| Mutex::new(false));
    let mut guard = running.lock();
    if *guard { return; }
    *guard = true;
    drop(guard);

    let shutdown = SHUTDOWN.get_or_init(|| Arc::new(AtomicBool::new(false))).clone();
    connectivity::register("signals_feed", Arc::new(SignalsFeedShutdown { shutdown: shutdown.clone() }));

    std::thread::spawn(move || {
        feed_runtime().block_on(async {
            tokio::spawn(run_watchdog());
            let mut backoff = Backoff::new().with_max_attempts(None);
            let mut first = true;
            loop {
                if shutdown.load(Ordering::SeqCst) {
                    publish_state(ConnectionState::ShuttingDown);
                    break;
                }
                if !first { RECONNECT_COUNT.fetch_add(1, Ordering::Relaxed); }
                first = false;
                publish_state(ConnectionState::Connecting {
                    attempt: RECONNECT_COUNT.load(Ordering::Relaxed) + 1,
                });
                match run_feed().await {
                    Ok(()) => { backoff.reset(); }
                    Err(e) => {
                        report(ErrorLevel::Warn, "signals_feed", "reconnect", e.to_string());
                    }
                }
                if shutdown.load(Ordering::SeqCst) {
                    publish_state(ConnectionState::ShuttingDown);
                    break;
                }
                if let Some(d) = backoff.next_delay() {
                    publish_state(ConnectionState::Backoff {
                        until: std::time::Instant::now() + d,
                        attempt: RECONNECT_COUNT.load(Ordering::Relaxed) + 1,
                        reason: "disconnected".into(),
                    });
                    tokio::time::sleep(d).await;
                } else {
                    publish_state(ConnectionState::Failed {
                        reason: connectivity::ConnectionError::MaxRetriesExceeded(
                            RECONNECT_COUNT.load(Ordering::Relaxed),
                        ),
                    });
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

    let ws_url = apex_signals_ws();
    report(ErrorLevel::Info, "signals_feed", "connecting", &ws_url);
    let (ws, _) = connect_async(&ws_url).await?;
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
    publish_state(ConnectionState::Authenticated);
    publish_state(ConnectionState::Subscribed { count: 4 });

    // Wave 7E: heartbeat + force-reconnect plumbing.
    FORCE_RECONNECT.store(false, Ordering::SeqCst);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECS));
    heartbeat.tick().await;
    let mut force_check = tokio::time::interval(Duration::from_secs(1));
    force_check.tick().await;

    loop {
        if FORCE_RECONNECT.swap(false, Ordering::SeqCst) {
            report(ErrorLevel::Warn, "signals_feed", "force_reconnect", "watchdog tripped");
            publish_state(ConnectionState::Backoff {
                until: std::time::Instant::now() + Duration::from_secs(1),
                attempt: RECONNECT_COUNT.load(Ordering::Relaxed),
                reason: "tick_stalled".into(),
            });
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
            let stall_secs = (now - last) / 1000;
            LAST_STALL_AT_MS.store(now, Ordering::Relaxed);
            report(
                ErrorLevel::Warn,
                "signals_feed",
                "tick_stalled",
                format!("no frames for {}ms — forcing reconnect", now - last),
            );
            // P1.11: user-visible toast with 60s cooldown.
            let last_toast = LAST_STALL_TOAST_AT.load(Ordering::Relaxed);
            if now - last_toast >= STALL_TOAST_COOLDOWN_MS {
                LAST_STALL_TOAST_AT.store(now, Ordering::Relaxed);
                report(
                    ErrorLevel::Warn,
                    "signals_feed",
                    "feed_stalled",
                    format!("Signals feed silent for >{stall_secs}s — reconnecting"),
                );
            }
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
