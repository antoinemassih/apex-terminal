mod data;
mod drawings;
mod ib_ws;
mod chart_renderer;
mod ui_kit;

use drawings::DbPool;
use sqlx::postgres::PgPoolOptions;
use tauri::Manager;
use tauri::async_runtime;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::Mutex;
use std::time::Duration;

/// Global sender for forwarding ticks to native chart renderer
static NATIVE_CHART_TX: std::sync::OnceLock<Mutex<Option<std::sync::mpsc::Sender<chart_renderer::ChartCommand>>>> = std::sync::OnceLock::new();

pub fn send_to_native_chart(cmd: chart_renderer::ChartCommand) {
    if let Some(lock) = NATIVE_CHART_TX.get() {
        if let Ok(guard) = lock.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(cmd);
            }
        }
    }
}

/// Bar data passed from WebView
#[derive(serde::Deserialize, Debug)]
struct JsBar {
    open: f64, high: f64, low: f64, close: f64, volume: f64, time: i64,
}

#[tauri::command]
async fn open_native_chart(symbol: String, timeframe: String, bars: Option<Vec<JsBar>>) -> Result<String, String> {
    eprintln!("[native-chart] Opening for {} {} (bars from WebView: {})", symbol, timeframe, bars.as_ref().map_or(0, |b| b.len()));

    // Everything runs on a detached thread — command returns instantly
    std::thread::spawn(move || {
        eprintln!("[native-chart] Thread started");

        // Convert WebView bars to native format
        let (gpu_bars, timestamps) = if let Some(ref js_bars) = bars {
            if !js_bars.is_empty() {
                eprintln!("[native-chart] Using {} bars from WebView", js_bars.len());
                let b: Vec<chart_renderer::Bar> = js_bars.iter().map(|b| chart_renderer::Bar {
                    open: b.open as f32, high: b.high as f32, low: b.low as f32,
                    close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                }).collect();
                let t: Vec<i64> = js_bars.iter().map(|b| b.time).collect();
                (b, t)
            } else {
                eprintln!("[native-chart] WebView sent empty bars");
                (vec![], vec![])
            }
        } else {
            eprintln!("[native-chart] No bars from WebView");
            (vec![], vec![])
        };
        let bars = gpu_bars;

        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(chart_renderer::ChartCommand::LoadBars {
            symbol, timeframe, bars, timestamps,
        });

        // Store sender globally for tick forwarding
        {
            let global = NATIVE_CHART_TX.get_or_init(|| Mutex::new(None));
            *global.lock().unwrap() = Some(tx);
        }

        eprintln!("[native-chart] Starting render loop");
        chart_renderer::gpu::run_render_loop(
            &format!("Apex Chart — Native GPU"),
            1400, 900, rx,
        );
        eprintln!("[native-chart] Render loop exited");
    });

    eprintln!("[native-chart] Command returning");
    Ok("spawned".to_string())
}

async fn fetch_bars_for_native(symbol: &str, interval: &str, period: &str) -> Vec<data::Bar> {
    // Try OCOCO/InfluxDB first (fast, deep history)
    let ococo_url = format!(
        "http://192.168.1.60:30300/api/bars?symbol={}&interval={}&start=-365d",
        symbol, interval
    );
    if let Ok(resp) = reqwest::Client::new().get(&ococo_url).timeout(std::time::Duration::from_secs(3)).send().await {
        if let Ok(bars) = resp.json::<Vec<data::Bar>>().await {
            if bars.len() > 10 { return bars; }
        }
    }

    // Fallback: yfinance sidecar
    let url = format!("http://127.0.0.1:8777/bars?symbol={}&interval={}&period={}", symbol, interval, period);
    match reqwest::get(&url).await {
        Ok(resp) => resp.json::<Vec<data::Bar>>().await.unwrap_or_default(),
        Err(_) => Vec::new(),
    }
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
                app.manage(DbPool(pool));
            }

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
