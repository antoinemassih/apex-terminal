//! Apex Terminal — Standalone Native GPU Application
//! No Tauri, no WebView. Pure Rust + wgpu + egui.

// windows_subsystem disabled for debugging
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[global_allocator]
static GLOBAL: _scaffold_lib::monitoring::CountingAlloc = _scaffold_lib::monitoring::CountingAlloc;

use std::sync::Mutex;

fn main() {
    eprintln!("╔══════════════════════════════════════╗");
    eprintln!("║  Apex Terminal — Native GPU Edition   ║");
    eprintln!("╚══════════════════════════════════════╝");

    // Initialize Redis bar cache
    _scaffold_lib::bar_cache::init();

    // Initialize PostgreSQL drawing persistence
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
            .await
        {
            Ok(pool) => {
                eprintln!("[apex-native] PostgreSQL connected");
                if let Err(e) = _scaffold_lib::drawings::ensure_schema(&pool).await {
                    eprintln!("[apex-native] Schema migration failed: {e}");
                }
                _scaffold_lib::drawing_db::init(pool);
            }
            Err(e) => eprintln!("[apex-native] PostgreSQL unavailable ({e}) — drawings won't persist"),
        }
    });

    // Start performance monitoring
    _scaffold_lib::monitoring::start();

    // Initialize global chart channel
    _scaffold_lib::NATIVE_CHART_TXS.get_or_init(|| Mutex::new(Vec::new()));

    eprintln!("[apex-native] Opening chart window...");

    let (tx, rx) = std::sync::mpsc::channel();
    let initial = _scaffold_lib::chart_renderer::ChartCommand::LoadBars {
        symbol: "SPY".into(),
        timeframe: "5m".into(),
        bars: vec![],
        timestamps: vec![],
    };

    {
        let global = _scaffold_lib::NATIVE_CHART_TXS.get_or_init(|| Mutex::new(Vec::new()));
        global.lock().unwrap().push(tx);
    }

    _scaffold_lib::chart_renderer::gpu::fetch_bars_background_pub("SPY".into(), "5m".into());
    _scaffold_lib::chart_renderer::gpu::open_window(rx, initial, None);

    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let has_senders = _scaffold_lib::NATIVE_CHART_TXS.get()
            .and_then(|m| m.lock().ok())
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if !has_senders {
            std::thread::sleep(std::time::Duration::from_millis(200));
            break;
        }
    }

    eprintln!("[apex-native] All windows closed. Exiting.");
}
