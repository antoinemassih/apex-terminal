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
async fn open_native_chart(symbol: String, timeframe: String) -> Result<(), String> {
    let handle = chart_renderer::spawn(&format!("{} — {}", symbol, timeframe), 1400, 900);

    // Fetch real historical data via yfinance sidecar
    let interval_map: std::collections::HashMap<&str, (&str, &str)> = [
        ("1m", ("1m", "1d")), ("2m", ("2m", "5d")), ("5m", ("5m", "5d")),
        ("15m", ("15m", "1mo")), ("30m", ("30m", "1mo")), ("1h", ("60m", "3mo")),
        ("4h", ("60m", "6mo")), ("1d", ("1d", "1y")), ("1wk", ("1wk", "5y")),
    ].into_iter().collect();

    let (interval, period) = interval_map.get(timeframe.as_str()).copied().unwrap_or(("5m", "5d"));

    // Try OCOCO InfluxDB first, fallback to yfinance
    let bars = fetch_bars_for_native(&symbol, interval, period).await;

    let gpu_bars: Vec<chart_renderer::Bar> = bars.iter().map(|b| chart_renderer::Bar {
        open: b.open as f32, high: b.high as f32, low: b.low as f32,
        close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
    }).collect();

    handle.send(chart_renderer::ChartCommand::LoadBars {
        symbol: symbol.clone(),
        timeframe: timeframe.clone(),
        bars: gpu_bars,
    });

    // Store handle for tick forwarding
    // In production: forward IB ticks to handle.send(UpdateLastBar)
    std::mem::forget(handle);
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
