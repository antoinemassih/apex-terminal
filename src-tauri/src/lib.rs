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
async fn open_native_chart(symbol: String, timeframe: String) -> Result<String, String> {
    eprintln!("[native-chart] Opening for {} {}", symbol, timeframe);

    // Everything runs on a detached thread — command returns instantly
    std::thread::spawn(move || {
        eprintln!("[native-chart] Thread started, fetching data...");

        // Map timeframe to yfinance interval/period
        let (interval, period) = match timeframe.as_str() {
            "1m" => ("1m", "1d"), "2m" => ("2m", "5d"), "5m" => ("5m", "5d"),
            "15m" => ("15m", "1mo"), "30m" => ("30m", "1mo"), "1h" => ("60m", "3mo"),
            "4h" => ("60m", "6mo"), "1d" => ("1d", "1y"), "1wk" => ("1wk", "5y"),
            _ => ("5m", "5d"),
        };

        // Blocking HTTP fetch — try OCOCO then yfinance
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build().unwrap();

        let bars: Vec<chart_renderer::Bar> = (|| -> Option<Vec<chart_renderer::Bar>> {
            // Try OCOCO/InfluxDB
            let ococo_url = format!(
                "http://192.168.1.60:30300/api/bars?symbol={}&interval={}&start=-365d",
                symbol, interval
            );
            if let Ok(resp) = client.get(&ococo_url).timeout(std::time::Duration::from_secs(3)).send() {
                if let Ok(raw) = resp.json::<Vec<data::Bar>>() {
                    if raw.len() > 10 {
                        eprintln!("[native-chart] Loaded {} bars from OCOCO", raw.len());
                        return Some(raw.iter().map(|b| chart_renderer::Bar {
                            open: b.open as f32, high: b.high as f32, low: b.low as f32,
                            close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                        }).collect());
                    }
                }
            }

            // Try yfinance
            let yf_url = format!(
                "http://127.0.0.1:8777/bars?symbol={}&interval={}&period={}",
                symbol, interval, period
            );
            if let Ok(resp) = client.get(&yf_url).send() {
                if let Ok(raw) = resp.json::<Vec<data::Bar>>() {
                    if !raw.is_empty() {
                        eprintln!("[native-chart] Loaded {} bars from yfinance", raw.len());
                        return Some(raw.iter().map(|b| chart_renderer::Bar {
                            open: b.open as f32, high: b.high as f32, low: b.low as f32,
                            close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                        }).collect());
                    }
                }
            }

            eprintln!("[native-chart] No data sources available, using test data");
            None
        })().unwrap_or_else(|| {
            // Fallback test data
            let mut v = Vec::new();
            let mut p = 180.0_f32;
            let mut s: u64 = 12345;
            for _ in 0..500 {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let r = (s >> 33) as f32 / (u32::MAX as f32);
                let o = p; let c = p + (r - 0.48) * 3.0;
                let h = o.max(c) + r * 1.5; let l = o.min(c) - r * 1.0;
                v.push(chart_renderer::Bar { open: o, high: h, low: l, close: c, volume: r * 500000.0, _pad: 0.0 });
                p = c.max(50.0);
            }
            v
        });

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
