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

    // Wave 1: unified tracing FIRST. Bind the WorkerGuard to a local that
    // lives until the bottom of `main` so the non-blocking writer thread
    // stays alive for the program's duration.
    let log_dir = std::env::var("APEX_LOG_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("apex-terminal-logs"));
    let _tracing_guard = _scaffold_lib::data::connectivity::init_tracing(&log_dir);
    tracing::info!(target: "apex_native", log_dir = %log_dir.display(), "tracing initialized");

    // Initialize Redis bar cache. URL comes from APEX_REDIS_URL env
    // (defaults to the homelab dev Redis).
    _scaffold_lib::bar_cache::init(&_scaffold_lib::data::apex_data::config::apex_redis_url());

    // Initialize PostgreSQL drawing persistence
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let pg_url = _scaffold_lib::data::apex_data::config::apex_pg_url();
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect(&pg_url)
            .await
        {
            Ok(pool) => {
                eprintln!("[apex-native] PostgreSQL connected");
                // Schema is managed externally; lib.rs (Tauri path) also skips
                // an ensure_schema call. drawing_db::init is the contract.
                _scaffold_lib::drawing_db::init(pool.clone());
                // Wave 7A fix (Bug 2): register the pool for shutdown.
                use std::sync::Arc;
                use _scaffold_lib::data::connectivity::{register, shutdown::PgPoolShutdown};
                register("postgres", Arc::new(PgPoolShutdown { name: "postgres", pool }));
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

    // macOS requires winit::EventLoop on the main thread — use the blocking
    // variant that owns the main thread until all windows close.
    #[cfg(target_os = "macos")]
    _scaffold_lib::chart_renderer::gpu::open_window_blocking(rx, initial, None);

    #[cfg(not(target_os = "macos"))]
    {
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
    }

    // Wave 1: drain registered connections before exit. Uses the
    // already-built tokio runtime from above. 3s deadline mirrors the Tauri
    // entry point.
    rt.block_on(async {
        _scaffold_lib::data::connectivity::drain_all(std::time::Duration::from_secs(3)).await;
    });

    eprintln!("[apex-native] All windows closed. Exiting.");
    drop(_tracing_guard);
}
