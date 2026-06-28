//! Real-time signals feed — patterns, alerts, trendlines, significance.
//!
//! Connects to ApexSignals WS and subscribes to the signal channels, pushing
//! PatternLabels / AlertTriggered / AutoTrendlines / SignificanceUpdate to the
//! chart renderer.
//!
//! Transport (connect / subscribe / heartbeat / tick-watchdog + stall toast /
//! jittered backoff / shutdown / ConnectionState + metrics) is handled by
//! [`resilient_ws::spawn_subscribed`]; this file keeps its `state_tx()` +
//! metric statics (the `signals` provider subscribes to them for the connection
//! panel) and the channel parse/route.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use crate::chart_renderer::{ChartCommand, PatternLabel};
use crate::data::connectivity::ConnectionState;
use crate::data::feeds::resilient_ws::{self, FeedMetrics, SubscribedWsConfig};

// ── Per-feed metrics (read by the `signals` provider / diagnostics) ──────────
pub(crate) static MESSAGES_IN:        AtomicU64 = AtomicU64::new(0);
pub(crate) static PARSE_ERRORS:       AtomicU64 = AtomicU64::new(0);
pub(crate) static RECONNECT_COUNT:    AtomicU32 = AtomicU32::new(0);
pub(crate) static LAST_MESSAGE_AT_MS: AtomicI64 = AtomicI64::new(0);
/// Vestigial: staleness/reconnect is now owned by the `resilient_ws` harness.
/// Kept (always false) because the `signals` provider's `state()` fallback seed
/// still reads it; the authoritative state flows via `state_tx()`.
pub(crate) static FORCE_RECONNECT: AtomicBool = AtomicBool::new(false);

// ── ConnectionState push stream (the `signals` provider subscribes here) ─────
static STATE_TX: OnceLock<tokio::sync::broadcast::Sender<ConnectionState>> = OnceLock::new();
pub fn state_tx() -> &'static tokio::sync::broadcast::Sender<ConnectionState> {
    STATE_TX.get_or_init(|| tokio::sync::broadcast::channel(64).0)
}
pub(crate) fn publish_state(s: ConnectionState) {
    let _ = state_tx().send(s);
}

static STARTED: OnceLock<()> = OnceLock::new();

/// ApexSignals WebSocket URL. ApexSignals binds REST + WS on one port
/// (`SIGNALS_API_PORT`, default 8100). Override with `APEX_SIGNALS_WS`.
fn apex_signals_ws() -> String {
    std::env::var("APEX_SIGNALS_WS").unwrap_or_else(|_| "ws://localhost:8100/ws".to_string())
}

pub fn start() {
    if STARTED.set(()).is_err() {
        return;
    }
    resilient_ws::spawn_subscribed(SubscribedWsConfig {
        name: "signals_feed",
        url: Box::new(apex_signals_ws),
        subscribe_msg: serde_json::json!({
            "subscribe": ["patterns", "alerts", "trendlines", "chart_patterns", "significance"]
        }).to_string(),
        subscribed_count: 4,
        // Signals traffic is bursty (events on bar-close); ping unconditionally.
        heartbeat: Some(Duration::from_secs(30)),
        conditional_heartbeat: false,
        // Signals can be legitimately quiet for minutes.
        stale_timeout: Some(Duration::from_secs(60)),
        gap_fill_on_reconnect: true,
        metrics: FeedMetrics {
            messages_in: &MESSAGES_IN,
            reconnect_count: &RECONNECT_COUNT,
            last_message_at_ms: &LAST_MESSAGE_AT_MS,
        },
        publish_state: Box::new(publish_state),
        on_text: Box::new(on_text),
    });
}

/// Parse one signal frame and route it by channel.
fn on_text(text: &str) {
    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => { PARSE_ERRORS.fetch_add(1, Ordering::Relaxed); return; }
    };

    let channel = json.get("channel").and_then(|c| c.as_str()).unwrap_or("");
    let symbol = match json.get("symbol").and_then(|s| s.as_str()) {
        Some(s) => s.to_string(),
        None => return,
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
            resilient_ws::send_to_charts(ChartCommand::PatternLabels { symbol, labels });
        }
        "alerts" => {
            let alert_id = json.get("alert_id").and_then(|a| a.as_str()).unwrap_or("").to_string();
            let price = json.get("price").and_then(|p| p.as_f64()).unwrap_or(0.0) as f32;
            let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("Alert triggered").to_string();
            resilient_ws::send_to_charts(ChartCommand::AlertTriggered { symbol, alert_id, price, message });
        }
        "trendlines" | "chart_patterns" => {
            let drawings_json = json.get("drawings")
                .map(|d| d.to_string())
                .unwrap_or_else(|| "[]".to_string());
            // `source` scopes replacement so trendlines and chart patterns
            // don't wipe each other; fall back to the channel name.
            let source = json.get("source").and_then(|s| s.as_str()).unwrap_or(channel).to_string();
            resilient_ws::send_to_charts(ChartCommand::AutoTrendlines { symbol, drawings_json, source });
        }
        "significance" => {
            let drawing_id = json.get("drawing_id").and_then(|d| d.as_str()).unwrap_or("").to_string();
            let score = json.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
            let touches = json.get("touches").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
            let strength = json.get("strength").and_then(|s| s.as_str()).unwrap_or("WEAK").to_string();
            resilient_ws::send_to_charts(ChartCommand::SignificanceUpdate { symbol, drawing_id, score, touches, strength });
        }
        _ => {} // unknown channel — ignore
    }
}
