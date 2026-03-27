mod data;
mod drawings;
mod ib_ws;
mod chart_renderer;

use drawings::DbPool;
use sqlx::postgres::PgPoolOptions;
use tauri::Manager;
use tauri::async_runtime;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::Mutex;
use std::time::Duration;

struct NativeChart(Mutex<Option<chart_renderer::ChartRendererHandle>>);

#[tauri::command]
fn open_native_chart(symbol: String, timeframe: String) -> Result<(), String> {
    eprintln!("[native-chart] Opening for {} {}", symbol, timeframe);

    // Everything runs on a detached thread — command returns instantly
    std::thread::spawn(move || {
        eprintln!("[native-chart] Thread started");

        // Generate test data
        let mut bars = Vec::new();
        let mut price = 180.0_f32;
        let mut seed: u64 = 12345;
        for _ in 0..1000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r1 = (seed >> 33) as f32 / (u32::MAX as f32);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r2 = (seed >> 33) as f32 / (u32::MAX as f32);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r3 = (seed >> 33) as f32 / (u32::MAX as f32);
            let open = price;
            let close = price + (r1 - 0.48) * 3.0;
            let high = open.max(close) + r2 * 1.5;
            let low = open.min(close) - r3 * 1.5;
            let volume = (r1 * 500.0 + 200.0) * 1000.0;
            bars.push(chart_renderer::Bar { open, high, low, close, volume, _pad: 0.0 });
            price = close.max(50.0);
        }

        // Create channel and send data before starting the event loop
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(chart_renderer::ChartCommand::LoadBars {
            symbol, timeframe, bars,
        });

        eprintln!("[native-chart] Starting render loop");
        // This blocks the thread (winit event loop) — that's intentional
        chart_renderer::gpu::run_render_loop(
            &format!("Apex Chart — Native GPU"),
            1400, 900, rx,
        );
        eprintln!("[native-chart] Render loop exited");
    });

    eprintln!("[native-chart] Command returning");
    Ok(())
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
