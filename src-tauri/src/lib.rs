pub mod data;
pub mod drawings;
mod ib_ws;
pub mod chart_renderer;
pub mod ui_kit;
pub mod bar_cache;
pub mod drawing_db;
pub mod monitoring;

use drawings::DbPool;
use sqlx::postgres::PgPoolOptions;
use tauri::Manager;
use tauri::async_runtime;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::Mutex;
use std::time::Duration;

/// Global senders for forwarding ticks/data to ALL native chart windows
pub static NATIVE_CHART_TXS: std::sync::OnceLock<Mutex<Vec<std::sync::mpsc::Sender<chart_renderer::ChartCommand>>>> = std::sync::OnceLock::new();

/// Send bar data from WebView to native chart (called when WebView loads data for requested symbol)
#[tauri::command]
fn native_chart_data(symbol: String, timeframe: String, bars: Vec<JsBar>) {
    // Cache in Redis for future use
    let cache_bars: Vec<data::Bar> = bars.iter().map(|b| data::Bar {
        time: b.time, open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
    }).collect();
    bar_cache::set(&symbol, &timeframe, &cache_bars);

    let (gpu_bars, timestamps) = convert_js_bars(&bars);
    eprintln!("[native-chart] Received {} bars for {} from WebView", gpu_bars.len(), symbol);
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
    });
}

pub fn send_to_native_chart(cmd: chart_renderer::ChartCommand) {
    if let Some(lock) = NATIVE_CHART_TXS.get() {
        if let Ok(mut guard) = lock.lock() {
            // Broadcast to all windows, remove dead senders
            guard.retain(|tx| tx.send(cmd.clone()).is_ok());
        }
    }
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
async fn open_native_chart(app: tauri::AppHandle, symbol: String, timeframe: String, bars: Option<Vec<JsBar>>) -> Result<String, String> {
    eprintln!("[native-chart] Opening for {} {} (bars from WebView: {})", symbol, timeframe, bars.as_ref().map_or(0, |b| b.len()));

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // PostgreSQL pool — optional, app starts without it if DB is unreachable.
            // acquire_timeout caps the initial connection attempt at 3 s instead of
            // blocking the setup thread indefinitely (which leaves the window blank).
            let pool_opt = async_runtime::block_on(async {
                let connect = PgPoolOptions::new()
                    .max_connections(5)
                    .acquire_timeout(Duration::from_secs(3))
                    .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
                    .await;
                match connect {
                    Err(e) => {
                        eprintln!("[apex] PostgreSQL unavailable ({e}) — drawings use fallback");
                        None
                    }
                    Ok(p) => match drawings::ensure_schema(&p).await {
                        Err(e) => {
                            eprintln!("[apex] DB schema migration failed ({e}) — drawings use fallback");
                            None
                        }
                        Ok(()) => Some(p),
                    },
                }
            });
            if let Some(pool) = pool_opt {
                drawing_db::init(pool.clone());
                app.manage(DbPool(pool));
            }

            // Redis bar cache — optional, app works without it
            bar_cache::init();

            // System monitoring — GPU, CPU, memory, frame timing → :9091/metrics
            monitoring::start();

            // IB WebSocket hot path — Rust-native, msgpack binary
            let ib_handle = ib_ws::spawn(app.handle().clone());
            app.manage(ib_handle);

            // Spawn ococo-api sidecar — bundled Node.js server
            match app.shell().sidecar("ococo-api") {
                Err(e) => eprintln!("[apex] ococo-api sidecar not found: {e}"),
                Ok(cmd) => match cmd.spawn() {
                    Err(e) => eprintln!("[apex] Failed to spawn ococo-api: {e}"),
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
                                        eprintln!("[ococo] error: {e}");
                                    }
                                    CommandEvent::Terminated(status) => {
                                        eprintln!("[ococo] exited: {:?}", status);
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
            drawings::drawings_load_all,
            drawings::drawings_load_symbol,
            drawings::drawings_save,
            drawings::drawings_update_points,
            drawings::drawings_update_style,
            drawings::drawings_remove,
            drawings::drawings_clear,
            drawings::groups_load_all,
            drawings::groups_save,
            drawings::groups_remove,
            drawings::groups_update_style,
            drawings::drawings_apply_group_style,
            ib_ws::ib_ws_send,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // Kill ococo-api cleanly when the app exits
            if let tauri::RunEvent::Exit = event {
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
