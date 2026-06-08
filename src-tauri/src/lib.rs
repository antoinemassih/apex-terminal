#![recursion_limit = "512"]

pub mod design_system;
pub mod foundation;
pub mod data;
pub mod persistence;
pub mod chart;
pub mod ui_kit;
pub mod watchlist;
pub mod state;
pub mod error;

pub use error::AppError;

// Backward-compat re-exports so code in lib.rs body keeps working without changes
pub use foundation::monitoring;
pub use foundation::design_tokens;
#[cfg(feature = "design-mode")]
pub use foundation::design_inspector;
pub use data::bar_cache;
pub use data::apex_data;
pub use data::crypto_feed;
pub use data::signals_feed;
pub use data::discord;
pub use persistence::drawing_db;
pub use persistence::watchlist_db;

// chart_state / chart_renderer backward compat aliases
pub use chart::state as chart_state;
pub use chart::renderer as chart_renderer;

use std::sync::Mutex;

/// Global senders for forwarding ticks/data to ALL native chart windows
pub static NATIVE_CHART_TXS: std::sync::OnceLock<Mutex<Vec<std::sync::mpsc::Sender<chart_renderer::ChartCommand>>>> = std::sync::OnceLock::new();

/// Global egui Context for cross-thread repaint requests. The native chart
/// window installs its Context here on startup so background threads (data
/// feeds, fetch jobs, async tasks) can wake the UI when they have data to
/// show. egui::Context is Arc-internal and `request_repaint` is thread-safe.
pub static NATIVE_EGUI_CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Wake the native chart UI to render a frame. Safe to call from any thread.
/// No-op until the chart window has been created (and the OnceLock filled).
/// Call this after sending a ChartCommand or completing a background fetch
/// that the user expects to see — without it, the UI stays asleep.
pub fn wake_native_ui() {
    if let Some(ctx) = NATIVE_EGUI_CTX.get() {
        ctx.request_repaint();
    }
}

/// Broadcast a `ChartCommand` to every open native chart window, pruning any
/// senders whose receiver has been dropped. Background threads (data feeds,
/// fetch jobs) call this to push live bars/ticks/quotes into the UI.
pub fn send_to_native_chart(cmd: chart_renderer::ChartCommand) {
    if let Some(lock) = NATIVE_CHART_TXS.get() {
        if let Ok(mut guard) = lock.lock() {
            // Broadcast to all windows, remove dead senders
            guard.retain(|tx| tx.send(cmd.clone()).is_ok());
        }
    }
    // Wake the UI loop so the new data is rendered. Required after the
    // catch-all per-frame repaint was removed — without this, ticks/bars
    // pile up in the mpsc queue until the user moves the mouse.
    wake_native_ui();
}

/// Start the live market-data feeds and wire them into the native chart.
///
/// Called once from the native app's `main` after the chart channel and DB
/// pool are initialised. This is the de-Tauri replacement for the feed wiring
/// that used to live in the old `tauri::Builder::setup` closure.
///
/// - **ApexData WS** — REST + WebSocket market data. Live `bar`/`snapshot`
///   frames route into the chart via `send_to_native_chart`; `quote`/`trade`/
///   halt/spike/etc. frames route into `apex_data::live_state` for the
///   watchlist, tape, and overlay panels.
/// - **Connection-state listeners** — drain each provider's state broadcast
///   into the snapshot the connection panel reads each frame.
///
/// NOTE on IB: the broker integration is **ApexIB** (`APEXIB_URL`,
/// `apexib-dev.xllio.com`), polled over REST by `start_account_poller()` from
/// the chart window init — orders/executions/account/positions flow there. The
/// legacy `feeds::ib_ws` local-ibserver tick feed (`ws://127.0.0.1:5000`) is
/// obsolete (market data now comes from ApexData) and is intentionally **not**
/// auto-started here — spawning it only produced connect-refused spam and a
/// misleading "IB down" status against a server that isn't ApexIB.
pub fn init_live_feeds() {
    use crate::data::connectivity::errors_sink::{report, ErrorLevel};

    // ── ApexData — REST + WebSocket market data ──────────────────────────
    // Routes live `bar` / `snapshot` frames into the chart pipeline, and
    // `quote` / `trade` frames into the watchlist/tape global queues.
    apex_data::ws::start();
    apex_data::live_state::start_pollers();
    apex_data::ws::subscribe_to_frames(|frame| {
        use apex_data::ws::Frame;
        match frame {
            Frame::Bar(upd) | Frame::Snapshot { bar: upd, .. } => {
                // MARK_BARS_PROTOCOL: read source ("last"|"mark") off the
                // BarUpdate. Pane filters by matching its `bar_source_mark`.
                let mark = upd.source == "mark";
                crate::apex_log!("ws.bar", "symbol={} tf={} close={} closed={} src={}",
                    upd.bar.symbol, upd.bar.timeframe, upd.bar.close, upd.is_closed, upd.source);
                let gb = chart_renderer::Bar {
                    open: upd.bar.open as f32, high: upd.bar.high as f32,
                    low:  upd.bar.low  as f32, close: upd.bar.close as f32,
                    volume: upd.bar.volume as f32, _pad: 0.0,
                };
                let ts_sec = upd.bar.time / 1000;
                let cmd = if upd.is_closed {
                    chart_renderer::ChartCommand::AppendBar {
                        symbol: upd.bar.symbol.clone(),
                        timeframe: upd.bar.timeframe.clone(),
                        bar: gb, timestamp: ts_sec, mark,
                    }
                } else {
                    chart_renderer::ChartCommand::UpdateLastBar {
                        symbol: upd.bar.symbol.clone(),
                        timeframe: upd.bar.timeframe.clone(),
                        bar: gb, mark,
                    }
                };
                send_to_native_chart(cmd);
            }
            Frame::Quote(q)  => { apex_data::live_state::push_quote(q.clone()); }
            Frame::Trade(t)  => {
                apex_data::live_state::push_trade(t.clone());
                // Also push into the chart tape panel. ApexData trades don't carry
                // a buy/sell flag (NBBO-only feed per spec §12), so mark `is_buy`
                // based on whether price is at/above mid via the cached quote.
                let is_buy = apex_data::live_state::get_quote(&t.symbol)
                    .map(|q| {
                        let mid = (q.bid + q.ask) * 0.5;
                        t.price >= mid
                    }).unwrap_or(true);
                send_to_native_chart(chart_renderer::ChartCommand::TapeEntry {
                    symbol: t.symbol.clone(), price: t.price as f32,
                    qty: t.qty as f32, time: t.time, is_buy,
                });
            }
            Frame::Fmv { symbol, fmv, time_ms } => {
                apex_data::live_state::push_fmv(apex_data::live_state::Fmv {
                    symbol: symbol.clone(), fmv: *fmv, time_ms: *time_ms,
                });
            }
            Frame::ChainDelta(d) => {
                apex_data::live_state::merge_chain_delta(&d.underlying, &d.rows);
                crate::apex_log!("ws.chain", "{} delta: {} rows", d.underlying, d.rows.len());
            }
            Frame::Halt(h) => {
                // Cache in recent_halts for the heat/scanner panels; also
                // surface as a toast for HaltActive / NearLuld events so
                // the user sees an immediate banner. HaltCleared dismisses
                // by removing the active entry (handled inside push_halt).
                use apex_data::types::HaltKind;
                let toast = match h.kind {
                    HaltKind::HaltActive => Some(format!(
                        "HALT ACTIVE: {} ({}) @ {:.2}", h.symbol, h.reason, h.price)),
                    HaltKind::NearLuldUp => Some(format!(
                        "NEAR LULD↑: {} @ {:.2}", h.symbol, h.price)),
                    HaltKind::NearLuldDown => Some(format!(
                        "NEAR LULD↓: {} @ {:.2}", h.symbol, h.price)),
                    HaltKind::HaltCleared => Some(format!(
                        "RESUMED: {} @ {:.2}", h.symbol, h.price)),
                    HaltKind::Unknown => None,
                };
                apex_data::live_state::push_halt(h.clone());
                if let Some(msg) = toast {
                    apex_data::live_state::push_toast(msg);
                }
            }
            Frame::Resync { reason } => {
                report(ErrorLevel::Warn, "apex_data", "resync", reason.to_string());
            }
            Frame::Connection(connected) => {
                apex_data::live_state::set_connected(*connected);
            }
            Frame::Error { code, message } => {
                report(ErrorLevel::Warn, "apex_data", "server_error", format!("{code}: {message}"));
                // Surface sub_rejected (cap reached, no feed handle) as a toast.
                // Other soft errors stay in stderr — too noisy for the UI.
                if code == "sub_rejected" {
                    apex_data::live_state::push_toast(format!("ApexData: {message}"));
                }
            }
            // SOTA §4.4 — TradePlan v2: stash the latest plan keyed
            // by symbol. The new trade_plan_panel reads from there.
            Frame::TradePlan(plan) => {
                apex_data::live_state::push_trade_plan(plan.clone());
            }
            // SOTA §4.5 — Spike explanation: push into the recent
            // ring; the SpikeExplanationPopup component polls the
            // ring once per frame and renders new ids as toasts.
            Frame::Spike(spike) => {
                apex_data::live_state::push_spike(spike.clone());
            }
            _ => {}
        }
    });

    // ── Legacy local-ibserver tick feed (opt-in, OFF by default) ─────────
    // The broker integration is ApexIB (REST; see `start_account_poller`).
    // This `feeds::ib_ws` feed talks to a *local* ibserver at
    // `ws://127.0.0.1:5000` and is obsolete (market data now comes from
    // ApexData). Auto-starting it only produced connect-refused spam and a
    // false "IB down" status against a server that isn't ApexIB. Start it only
    // when a local ibserver is explicitly available.
    if std::env::var("APEX_ENABLE_LOCAL_IBSERVER").is_ok() {
        report(ErrorLevel::Info, "ib_ws", "enabled",
            "APEX_ENABLE_LOCAL_IBSERVER set — starting legacy local-ibserver feed");
        let _ = data::ib_ws::spawn();
    }

    // ── Connection-state push listeners ──────────────────────────────────
    // Bridge each provider's `subscribe_state()` broadcast into the snapshot
    // map the connection panel reads each frame. `spawn_state_listeners`
    // calls `tokio::spawn` internally, so it must run inside a live runtime —
    // reuse ApexData's long-lived multi-thread runtime.
    apex_data::ws::runtime().spawn(async {
        crate::chart_renderer::ui::panels::connection_state_snapshot::spawn_state_listeners();
    });
}

// ── P1.9: NATIVE_CHART_TXS broadcast stress test ────────────────────────────
//
// Verifies that `send_to_native_chart` fans out correctly under concurrent
// senders without losing messages or deadlocking.
//
// Design:
//   - 100 receivers registered in NATIVE_CHART_TXS
//   - 10 sender threads, each broadcasting 100 ChartCommand::Shutdown (cheapest
//     Clone-able variant with no allocation)
//   - Expected: each receiver sees all 1 000 events → 100 × 1 000 = 100 000
//     total across all receivers
//
// The test owns the OnceLock initialization, so it must run in an isolated
// process or be the first test to touch NATIVE_CHART_TXS.  In practice the
// lib unit-test binary contains no other initializer, so the OnceLock is
// always unset when this runs.  If the OnceLock is already set (parallel test
// run edge-case), `get_or_init` returns the existing value and the test will
// still pass (senders are added, counts will only be ≥ expected).
#[cfg(test)]
mod broadcast_stress {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn native_chart_txs_broadcast_100_receivers_10_senders() {
        const RECEIVERS: usize = 100;
        const SENDERS: usize = 10;
        const SENDS_PER_SENDER: usize = 100;
        const EXPECTED_PER_RECEIVER: usize = SENDERS * SENDS_PER_SENDER; // 1 000

        // --- Register receivers -------------------------------------------
        let txs_lock = NATIVE_CHART_TXS.get_or_init(|| Mutex::new(Vec::new()));

        let mut receivers = Vec::with_capacity(RECEIVERS);
        {
            // Wave 8 High: recover from lock poison.
            let mut guard = txs_lock.lock().unwrap_or_else(|e| e.into_inner());
            for _ in 0..RECEIVERS {
                let (tx, rx) = mpsc::channel::<chart_renderer::ChartCommand>();
                guard.push(tx);
                receivers.push(rx);
            }
        }

        // --- 10 concurrent sender threads, 100 commands each ---------------
        let mut handles = Vec::with_capacity(SENDERS);
        for _sender_id in 0..SENDERS {
            handles.push(thread::spawn(move || {
                for _ in 0..SENDS_PER_SENDER {
                    // `Shutdown` is a unit variant — cheapest possible clone.
                    send_to_native_chart(chart_renderer::ChartCommand::Shutdown);
                }
            }));
        }

        for h in handles {
            h.join().expect("sender thread panicked");
        }

        // --- Drain all receivers and count ----------------------------------
        // Give the mutex guard time to flush — all senders have joined so
        // every send call has completed; no additional sleep needed.
        let mut total_received: usize = 0;
        for rx in &receivers {
            while let Ok(_) = rx.try_recv() {
                total_received += 1;
            }
        }

        let expected_total = RECEIVERS * EXPECTED_PER_RECEIVER; // 100 000
        assert!(
            total_received > 0,
            "broadcast bus delivered nothing — NATIVE_CHART_TXS fan-out is broken"
        );
        assert_eq!(
            total_received, expected_total,
            "expected {} total deliveries ({} receivers × {} events), got {}",
            expected_total, RECEIVERS, EXPECTED_PER_RECEIVER, total_received
        );
    }
}
