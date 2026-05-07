//! Apex Terminal — Standalone Native GPU Application
//! No Tauri, no WebView. Pure Rust + wgpu + egui.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[global_allocator]
static GLOBAL: _scaffold_lib::monitoring::CountingAlloc = _scaffold_lib::monitoring::CountingAlloc;

use std::sync::Mutex;

fn main() {
    eprintln!("╔══════════════════════════════════════╗");
    eprintln!("║  Apex Terminal — Native GPU Edition   ║");
    eprintln!("╚══════════════════════════════════════╝");

    // Initialize design-mode token store so the inspector activates.
    #[cfg(feature = "design-mode")]
    {
        let tokens: _scaffold_lib::design_tokens::DesignTokens = std::fs::read_to_string("design.toml")
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        _scaffold_lib::design_tokens::init(tokens);
        eprintln!("[design-mode] active — inspector opens on the right side of the chart window");
    }

    // Initialize Redis bar cache
    _scaffold_lib::bar_cache::init();

    // Initialize PostgreSQL drawing persistence
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
            .await
        {
            Ok(pool) => {
                eprintln!("[apex-native] PostgreSQL connected");
                // Schema is managed by `migrations/001_chart_state.sql`,
                // applied out-of-band. Just start the drawing worker.
                _scaffold_lib::drawing_db::init(pool.clone());
                _scaffold_lib::watchlist_db::init(pool);
            }
            Err(e) => eprintln!("[apex-native] PostgreSQL unavailable ({e}) — drawings won't persist"),
        }
    });

    // Initialize global chart channel (for tick broadcasting)
    _scaffold_lib::NATIVE_CHART_TXS.get_or_init(|| Mutex::new(Vec::new()));

    // Start performance monitoring — Prometheus metrics + jank detection + GPU telemetry
    _scaffold_lib::monitoring::start();

    // Discord OAuth2 — load client credentials from discord.env
    _scaffold_lib::discord::load_config();

    // Crypto real-time feed — connects to ApexCrypto WebSocket
    _scaffold_lib::crypto_feed::start();

    // Signals real-time feed — connects to ApexSignals WebSocket for patterns/alerts/trendlines
    _scaffold_lib::signals_feed::start();

    eprintln!("[apex-native] Opening chart window...");

    // Create a channel and open the first window
    let (tx, rx) = std::sync::mpsc::channel();
    let initial = _scaffold_lib::chart_renderer::ChartCommand::LoadBars {
        symbol: "SPY".into(),
        timeframe: "5m".into(),
        bars: vec![],
        timestamps: vec![],
    };

    // Register the sender for tick forwarding
    {
        let global = _scaffold_lib::NATIVE_CHART_TXS.get_or_init(|| Mutex::new(Vec::new()));
        global.lock().unwrap().push(tx);
    }

    // Fetch initial data in background
    _scaffold_lib::chart_renderer::gpu::fetch_bars_background_pub("SPY".into(), "5m".into());

    // macOS requires the event loop on the main thread — open_window_blocking blocks here.
    // On other platforms open_window spawns a render thread and returns immediately,
    // so we sleep-poll until all windows are closed.
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

    eprintln!("[apex-native] All windows closed. Exiting.");
}
