#![recursion_limit = "512"]

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
pub(crate) use data::ib_ws;

// chart_state / chart_renderer backward compat aliases
pub use chart::state as chart_state;
pub use chart::renderer as chart_renderer;

use sqlx::postgres::PgPoolOptions;
use tauri::Manager;
use tauri::async_runtime;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::Mutex;
use std::time::Duration;
use crate::data::connectivity::errors_sink::{report, ErrorLevel};

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

/// Send bar data from WebView to native chart (called when WebView loads data for requested symbol)
#[tauri::command]
fn native_chart_data(symbol: String, timeframe: String, bars: Vec<JsBar>) {
    // Cache in Redis for future use
    let cache_bars: Vec<data::Bar> = bars.iter().map(|b| data::Bar {
        time: b.time, open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
    }).collect();
    bar_cache::set(&symbol, &timeframe, &cache_bars);

    let (gpu_bars, timestamps) = convert_js_bars(&bars);
    tracing::debug!(target: "native_chart", count = gpu_bars.len(), %symbol, "received bars from WebView");
    send_to_native_chart(chart_renderer::ChartCommand::LoadBars {
        symbol, timeframe, bars: gpu_bars, timestamps,
    });
}

/// Forward a single tick to the native chart
#[tauri::command]
fn native_chart_tick(symbol: String, price: f64, volume: f64) {
    send_to_native_chart(chart_renderer::ChartCommand::UpdateLastBar {
        symbol: symbol.clone(), timeframe: String::new(),
        bar: chart_renderer::Bar {
            open: price as f32, high: price as f32, low: price as f32,
            close: price as f32, volume: volume as f32, _pad: 0.0,
        },
        mark: false,
    });
}

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

/// Bar data passed from WebView
#[derive(serde::Deserialize, Debug)]
struct JsBar {
    open: f64, high: f64, low: f64, close: f64, volume: f64, time: i64,
}

/// Convert WebView JsBars into (gpu bars, timestamps) for the native chart renderer.
fn convert_js_bars(bars: &[JsBar]) -> (Vec<chart_renderer::Bar>, Vec<i64>) {
    let gpu: Vec<chart_renderer::Bar> = bars.iter().map(|b| chart_renderer::Bar {
        open: b.open as f32, high: b.high as f32, low: b.low as f32,
        close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
    }).collect();
    let ts: Vec<i64> = bars.iter().map(|b| b.time).collect();
    (gpu, ts)
}

#[tauri::command]
async fn open_native_chart(app: tauri::AppHandle, symbol: String, timeframe: String, bars: Option<Vec<JsBar>>) -> Result<String, AppError> {
    report(ErrorLevel::Info, "native_chart", "open",
        format!("opening for {} {} (bars from WebView: {})", symbol, timeframe, bars.as_ref().map_or(0, |b| b.len())));

    let (gpu_bars, timestamps) = bars.as_ref()
        .filter(|b| !b.is_empty())
        .map(|b| convert_js_bars(b))
        .unwrap_or_default();

    let (tx, rx) = std::sync::mpsc::channel();
    let initial = chart_renderer::ChartCommand::LoadBars {
        symbol, timeframe, bars: gpu_bars, timestamps,
    };

    // Register sender for tick broadcasting
    {
        let global = NATIVE_CHART_TXS.get_or_init(|| Mutex::new(Vec::new()));
        global.lock().unwrap().push(tx);
    }

    // Opens a new window (starts render thread on first call)
    chart_renderer::gpu::open_window(rx, initial, Some(app));

    Ok("spawned".to_string())
}



#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

struct OcocoProcess(Mutex<Option<CommandChild>>);

/// Holds the `tracing-appender` non-blocking writer guard for the program's
/// lifetime. Dropping it would stop the background log thread and silently
/// truncate buffered lines, so we stash it here instead.
static TRACING_GUARD: std::sync::OnceLock<tracing_appender::non_blocking::WorkerGuard> =
    std::sync::OnceLock::new();

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing FIRST so any startup error / log line gets captured.
    // Log dir: ~/Library/Logs/apex-terminal (macOS) or std::env::temp_dir()
    // fallback. The non-blocking writer guard MUST live for the program's
    // duration — stash it in a static.
    {
        let log_dir = dirs::data_local_dir()
            .map(|p| p.join("apex-terminal").join("logs"))
            .unwrap_or_else(|| std::env::temp_dir().join("apex-terminal-logs"));
        let guard = crate::data::connectivity::init_tracing(&log_dir);
        let _ = TRACING_GUARD.set(guard);
        tracing::info!(target: "apex", log_dir = %log_dir.display(), "tracing initialized");
    }

    // Initialize design-mode token store so is_active() returns true and
    // the inspector keyboard shortcut (Ctrl+Shift+D) becomes responsive.
    // Tries design.toml first, falls back to defaults.
    #[cfg(feature = "design-mode")]
    {
        let tokens: design_tokens::DesignTokens = std::fs::read_to_string("design.toml")
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        design_tokens::init(tokens);
        report(ErrorLevel::Info, "design_mode", "active", "active — press Ctrl+Shift+D to toggle the panel");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // PostgreSQL pool — optional, app starts without it if DB is unreachable.
            // acquire_timeout caps the initial connection attempt at 3 s instead of
            // blocking the setup thread indefinitely (which leaves the window blank).
            let pool_opt = async_runtime::block_on(async {
                let pg_url = data::apex_data::config::apex_pg_url();
                let connect = PgPoolOptions::new()
                    .max_connections(5)
                    .acquire_timeout(Duration::from_secs(3))
                    .connect(&pg_url)
                    .await;
                // Do NOT include `pg_url` in any error message — it carries
                // the password. The sqlx error already names the host:port.
                match connect {
                    Err(e) => {
                        report(ErrorLevel::Warn, "apex", "postgres_unavailable", format!("({e}) — drawings use fallback"));
                        None
                    }
                    Ok(p) => Some(p),
                }
            });
            if let Some(pool) = pool_opt {
                drawing_db::init(pool.clone());
                crate::persistence::watchlist_db::init(pool.clone());
                // Wave 7A fix (Bug 2): register the pool for shutdown so
                // `pool.close().await` runs on exit. Without this, sqlx's
                // background connections stay open and the dev DB starts
                // rejecting new connections after ~10-20 restarts.
                {
                    use std::sync::Arc;
                    use crate::data::connectivity::{register, shutdown::PgPoolShutdown};
                    register("postgres", Arc::new(PgPoolShutdown { name: "postgres", pool: pool.clone() }));
                }
                // Phase (d): refresh Polygon-backed ETF/index holdings into
                // symbol_universes on a background thread. Cold-start cache
                // is primed from the DB inside the same job.
                crate::watchlist::refresh::refresh_universes_in_background();
            }

            // Redis bar cache — optional, app works without it. URL comes
            // from APEX_REDIS_URL env (defaults to the homelab dev Redis).
            bar_cache::init(&data::apex_data::config::apex_redis_url());

            // System monitoring — GPU, CPU, memory, frame timing → :9091/metrics
            monitoring::start();

            // Discord OAuth2 — load client credentials from discord.env
            discord::load_config();

            // Crypto real-time feed — connects to ApexCrypto WebSocket
            crypto_feed::start();

            // Signals real-time feed — connects to ApexSignals WebSocket for patterns/alerts/trendlines
            signals_feed::start();

            // ApexData — REST + WebSocket market data.
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
                    _ => {}
                }
            });

            // IB WebSocket hot path — Rust-native, msgpack binary
            let ib_handle = ib_ws::spawn(app.handle().clone());
            app.manage(ib_handle);

            // Wave 12d: bridge `Connection::subscribe_state()` broadcast
            // streams into a module-level snapshot map readable by the
            // connection panel each frame. Must run after the WS feeds are
            // started above (so their broadcast senders exist) and inside the
            // tokio runtime (so `tokio::spawn` from inside the function works).
            async_runtime::spawn(async {
                crate::chart_renderer::ui::panels::connection_state_snapshot::spawn_state_listeners();
            });

            // Wave 7A fix (Bug 1): the noop pre-registrations that used to
            // live here masked the real Shutdown impls. Because `register()`
            // appends rather than replaces, the first (noop) entry won and
            // real WS close frames were never sent on exit. Each feed now
            // self-registers its real `Shutdown` when it spawns above
            // (apex_data::ws::start, ib_ws::spawn, crypto_feed::start,
            // signals_feed::start). Discord has no long-lived connection
            // yet — when it grows one, register from its module.

            // Spawn ococo-api sidecar — bundled Node.js server
            match app.shell().sidecar("ococo-api") {
                Err(e) => report(ErrorLevel::Error, "apex", "sidecar_not_found", format!("ococo-api: {e}")),
                Ok(cmd) => match cmd.spawn() {
                    Err(e) => report(ErrorLevel::Error, "apex", "sidecar_spawn_failed", format!("ococo-api: {e}")),
                    Ok((mut rx, child)) => {
                        // Drain sidecar stdout/stderr so the channel doesn't block.
                        tauri::async_runtime::spawn(async move {
                            use tauri_plugin_shell::process::CommandEvent;
                            while let Some(event) = rx.recv().await {
                                match event {
                                    CommandEvent::Stdout(line) => {
                                        if let Ok(s) = String::from_utf8(line) {
                                            print!("[ococo] {s}");
                                        }
                                    }
                                    CommandEvent::Stderr(line) => {
                                        if let Ok(s) = String::from_utf8(line) {
                                            eprint!("[ococo] {s}");
                                        }
                                    }
                                    CommandEvent::Error(e) => {
                                        report(ErrorLevel::Error, "ococo", "sidecar_error", e.to_string());
                                    }
                                    CommandEvent::Terminated(status) => {
                                        report(ErrorLevel::Warn, "ococo", "sidecar_exited", format!("{:?}", status));
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        });
                        app.manage(OcocoProcess(Mutex::new(Some(child))));
                    }
                },
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            open_native_chart,
            native_chart_data,
            native_chart_tick,
            data::get_bars,
            data::get_options_chain,
            chart::state::commands::export_chart_xol,
            chart::state::commands::import_chart_xol,
            chart::state::commands::save_chart_to_file,
            chart::state::commands::load_chart_from_file,
            ib_ws::ib_ws_send,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // Kill ococo-api cleanly when the app exits
            if let tauri::RunEvent::Exit = event {
                // Wave 1: drain all registered connections within 3 s before
                // killing sidecars. Best-effort — failures are logged via
                // tracing inside `drain_all`.
                tauri::async_runtime::block_on(async {
                    crate::data::connectivity::drain_all(std::time::Duration::from_secs(3)).await;
                });
                if let Some(state) = app.try_state::<OcocoProcess>() {
                    if let Ok(mut guard) = state.0.lock() {
                        if let Some(child) = guard.take() {
                            let _ = child.kill();
                        }
                    }
                }
            }
        });
}
