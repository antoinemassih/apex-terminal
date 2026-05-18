//! Real-time crypto feed — connects to ApexCrypto WebSocket for live bar updates.
//!
//! Subscribes to chart timeframes + 1s for price tracking.
//! Pushes UpdateLastBar / AppendBar / WatchlistPrice to the chart renderer.

use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

// ── Wave 6: per-feed metrics counters ────────────────────────────────────────
pub(crate) static MESSAGES_IN:        AtomicU64 = AtomicU64::new(0);
pub(crate) static PARSE_ERRORS:       AtomicU64 = AtomicU64::new(0);
pub(crate) static RECONNECT_COUNT:    AtomicU32 = AtomicU32::new(0);
pub(crate) static LAST_MESSAGE_AT_MS: AtomicI64 = AtomicI64::new(0);

// ── Wave 7E: WS resilience ───────────────────────────────────────────────────
// Crypto carries steady-state tick traffic in healthy operation, so a strict
// "ping every 30s" wastes a frame. The select-loop sends a Ping only when no
// inbound frame has arrived for HEARTBEAT_SECS.
const HEARTBEAT_SECS: u64 = 30;
const STALE_TIMEOUT_MS: i64 = 30_000;
const WATCHDOG_TICK_SECS: u64 = 10;
pub(crate) static FORCE_RECONNECT: AtomicBool = AtomicBool::new(false);
pub(crate) static LAST_STALL_AT_MS: AtomicI64 = AtomicI64::new(0);

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
use crate::chart_renderer::{self, ChartCommand, Bar};
use crate::data::connectivity::{self, errors_sink::{report, ErrorLevel}, Backoff, ConnectionState};

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
            // Wave 7E: tick-age watchdog runs alongside the reconnect loop on
            // the same single-thread runtime.
            tokio::spawn(run_watchdog());
            // Infinite reconnect — never give up on crypto.
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
                        report(ErrorLevel::Warn, "crypto_feed", "reconnect", e.to_string());
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

struct CryptoFeedShutdown {
    shutdown: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl connectivity::Shutdown for CryptoFeedShutdown {
    async fn drain(&self, deadline: Duration) -> Result<(), String> {
        tracing::info!(target: "shutdown", connection = "crypto_feed", "drain invoked");
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

    // Wave 7E: trigger gap-fill on every reconnect (skip initial connect).
    if RECONNECT_COUNT.load(Ordering::Relaxed) > 0 {
        tokio::spawn(async {
            let mgr = crate::data::providers::registry::subscription_manager();
            let n = mgr.gap_fill_on_reconnect_all().await;
            if n > 0 {
                report(
                    ErrorLevel::Info,
                    "crypto_feed",
                    "gap_fill",
                    format!("replayed {n} bars after reconnect"),
                );
            }
        });
    }

    // Subscribe to chart timeframes + tape for T&S
    let sub_msg = serde_json::json!({
        "subscribe": ["*:1s", "*:1m", "*:5m", "*:15m", "*:30m", "*:1h", "*:4h", "*:1d"],
        "tape": ["*"]
    });
    write.send(tokio_tungstenite::tungstenite::Message::Text(
        sub_msg.to_string().into()
    )).await?;
    report(ErrorLevel::Info, "crypto_feed", "connected", "bars + tape subscribed");
    publish_state(ConnectionState::Authenticated);
    // Wildcard subs — exact count isn't meaningful; surface 1 so the UI shows
    // green-with-load rather than just Authenticated.
    publish_state(ConnectionState::Subscribed { count: 1 });

    let mut chart_updates: u64 = 0;
    let mut price_updates: u64 = 0;
    let mut tape_updates: u64 = 0;
    let mut last_log = std::time::Instant::now();

    // Wave 7E: heartbeat + force-reconnect plumbing.
    FORCE_RECONNECT.store(false, Ordering::SeqCst);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECS));
    heartbeat.tick().await; // skip immediate first
    let mut force_check = tokio::time::interval(Duration::from_secs(1));
    force_check.tick().await;

    loop {
        if FORCE_RECONNECT.swap(false, Ordering::SeqCst) {
            report(ErrorLevel::Warn, "crypto_feed", "force_reconnect", "watchdog tripped");
            publish_state(ConnectionState::Backoff {
                until: std::time::Instant::now() + Duration::from_secs(1),
                attempt: RECONNECT_COUNT.load(Ordering::Relaxed),
                reason: "tick_stalled".into(),
            });
            return Err("watchdog forced reconnect".into());
        }
        tokio::select! {
            _ = heartbeat.tick() => {
                // Conditional ping: skip if a frame arrived in the last
                // HEARTBEAT_SECS — steady-state crypto traffic is its own
                // keepalive. Only ping during the (rare) quiet stretches.
                let age = now_ms() - LAST_MESSAGE_AT_MS.load(Ordering::Relaxed);
                if age > (HEARTBEAT_SECS as i64) * 1000 {
                    if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Ping(Vec::new().into())).await {
                        report(ErrorLevel::Warn, "crypto_feed", "ping_failed", e.to_string());
                        return Err(e.into());
                    }
                }
                continue;
            }
            _ = force_check.tick() => { continue; }
            frame = read.next() => {
                let msg = match frame {
                    Some(m) => m?,
                    None => break,
                };

        if !msg.is_text() {
            // Pong / binary / close — still proof of life.
            LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);
            continue;
        }
        let text = msg.to_text()?;

        MESSAGES_IN.fetch_add(1, Ordering::Relaxed);
        LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed);

        // Parse trade tape entries
        let json: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => { PARSE_ERRORS.fetch_add(1, Ordering::Relaxed); continue; }
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

            // Wave 8c: bump SubscriptionManager so gap-fill on reconnect
            // has accurate replay anchors for crypto symbols too. Crypto
            // subscribes by wildcard (no per-symbol subscribe site), so
            // the bumper is the only useful integration point — entries
            // are a no-op when no one has subscribed to this (sym, tf).
            // Crypto bars are always last-source.
            crate::data::providers::registry::subscription_manager()
                .bump_last_seen_bar(
                    &bar.symbol,
                    &bar.timeframe,
                    crate::data::providers::subscription_manager::BarSource::Last,
                    bar.time,
                );

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
                tracing::debug!(target: "crypto_feed", chart = chart_updates, prices = price_updates, tape = tape_updates, "30s tick metrics");
                chart_updates = 0;
                price_updates = 0;
                tape_updates = 0;
                last_log = std::time::Instant::now();
            }
        }
            } // frame arm
        } // tokio::select!
    } // outer loop

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
                "crypto_feed",
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
