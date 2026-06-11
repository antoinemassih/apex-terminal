//! Apex Terminal — Standalone Native GPU Application
//! No Tauri, no WebView. Pure Rust + wgpu + egui.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[global_allocator]
static GLOBAL: _scaffold_lib::monitoring::CountingAlloc = _scaffold_lib::monitoring::CountingAlloc;

use std::sync::Mutex;

#[cfg(target_os = "macos")]
fn set_macos_dock_icon() {
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};

    // PNG bytes embedded at compile time so we don't depend on cwd / bundle layout.
    const ICON_PNG: &[u8] = include_bytes!("../icons/128x128@2x.png");

    let mtm = match MainThreadMarker::new() {
        Some(m) => m,
        None => return, // can't touch AppKit off the main thread
    };
    unsafe {
        let data = NSData::with_bytes(ICON_PNG);
        let alloc = objc2::msg_send_id![objc2::class!(NSImage), alloc];
        let image: Option<objc2::rc::Retained<NSImage>> = NSImage::initWithData(alloc, &data);
        if let Some(image) = image {
            let app = NSApplication::sharedApplication(mtm);
            app.setApplicationIconImage(Some(&image));
        }
    }
}

fn main() {
    eprintln!("╔══════════════════════════════════════╗");
    eprintln!("║  Apex Terminal — Native GPU Edition   ║");
    eprintln!("╚══════════════════════════════════════╝");

    // Initialize tracing FIRST so feed/connectivity diagnostics — every
    // `errors_sink::report(...)` (WS connect/reconnect, IB status, server
    // errors) plus `apex_data=debug` — surface on stderr and persist to
    // <app-data>/apex-terminal/logs/apex.log. The WorkerGuard MUST outlive the
    // program; bind it for the lifetime of `main`, which blocks on the event
    // loop below. This is the de-Tauri replacement for the tracing init that
    // used to live in `_scaffold_lib::run`.
    let _tracing_guard = {
        let log_dir = dirs::data_local_dir()
            .map(|p| p.join("apex-terminal").join("logs"))
            .unwrap_or_else(|| std::env::temp_dir().join("apex-terminal-logs"));
        let g = _scaffold_lib::data::connectivity::init_tracing(&log_dir);
        tracing::info!(target: "apex", log_dir = %log_dir.display(), "tracing initialized");
        g
    };

    // Set the dock icon early so macOS picks it up before the first frame.
    #[cfg(target_os = "macos")]
    set_macos_dock_icon();

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

    // Initialize Redis bar cache. URL comes from APEX_REDIS_URL env
    // (defaults to the homelab dev Redis).
    _scaffold_lib::bar_cache::init(&_scaffold_lib::data::apex_data::config::apex_redis_url());

    // Initialize PostgreSQL drawing persistence
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let pg_url = _scaffold_lib::data::apex_data::config::apex_pg_url();
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&pg_url)
            .await
        {
            Ok(pool) => {
                eprintln!("[apex-native] PostgreSQL connected");
                // Schema is managed by `migrations/001_chart_state.sql`,
                // applied out-of-band. Just start the drawing worker.
                _scaffold_lib::drawing_db::init(pool.clone());
                _scaffold_lib::watchlist_db::init(pool.clone());
                // Wave 7A fix (Bug 2): register the pool for shutdown so
                // drain_all closes it cleanly on exit.
                use std::sync::Arc;
                use _scaffold_lib::data::connectivity::{register, shutdown::PgPoolShutdown};
                register("postgres", Arc::new(PgPoolShutdown { name: "postgres", pool }));
            }
            Err(e) => eprintln!("[apex-native] PostgreSQL unavailable ({e}) — drawings won't persist"),
        }
    });

    // Initialize global chart channel (for tick broadcasting)
    _scaffold_lib::NATIVE_CHART_TXS.get_or_init(|| Mutex::new(Vec::new()));

    // ── Theme hot-reload ──────────────────────────────────────────────────────
    // Resolve the themes directory: <app-data>/apex-terminal/themes.
    // Falls back to a `themes/` dir next to the binary when dirs::data_dir()
    // is unavailable (CI, sandboxed environments).
    {
        let themes_dir = dirs::data_dir()
            .map(|d| d.join("apex-terminal").join("themes"))
            .unwrap_or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join("themes")))
                    .unwrap_or_else(|| std::path::PathBuf::from("themes"))
            });

        // Seed the directory with editable copies of every built-in theme so
        // the user has files to edit on first launch.
        match _scaffold_lib::design_system::export_builtin_themes(&themes_dir) {
            Ok(n)  => eprintln!("[theme-watcher] seeded {n} theme files in {:?}", themes_dir),
            Err(e) => eprintln!("[theme-watcher] could not seed themes: {e}"),
        }

        // Scan for user-installed colour-scheme JSON files and register them
        // with the live theme list BEFORE the first frame so the picker and
        // command palette see them immediately.  Built-in indices 0–15 are
        // never disturbed; installed themes append at 16+.
        {
            let (installed, _styles) = _scaffold_lib::design_system::scan_theme_dir(&themes_dir);
            // Filter out built-ins (the seed step just wrote them, so they
            // appear in the scan result). `append_installed_themes` dedupes by
            // name, but filtering first avoids an unnecessary write-lock cycle.
            let builtin_names: std::collections::HashSet<String> =
                _scaffold_lib::design_system::builtin_color_schemes()
                    .iter()
                    .map(|cs| cs.meta.name.clone())
                    .collect();
            let user_schemes: Vec<_> = installed
                .into_iter()
                .filter(|cs| !builtin_names.contains(&cs.meta.name))
                .collect();
            if !user_schemes.is_empty() {
                eprintln!("[theme-watcher] loading {} installed theme(s)", user_schemes.len());
                _scaffold_lib::chart_renderer::gpu::append_installed_themes(user_schemes);
            }
        }

        // Wire the scheme-reload callback so design_system can dispatch live
        // scheme updates without depending on chart_renderer directly (P13).
        _scaffold_lib::design_system::hot_reload::set_scheme_update_hook(
            |schemes| _scaffold_lib::chart_renderer::gpu::upsert_installed_themes(schemes),
        );

        // Start the background watcher — ZERO per-frame main-thread cost.
        _scaffold_lib::design_system::start_theme_watcher(themes_dir);
    }

    // Wire the chart-app's frame_profiler into ui_kit's portable motion
    // module (P5b extraction Step 4). Without this registration ui_kit's
    // ease_bool/ease_value would lose the in-flight animation telemetry
    // that drives `foundation::frame_profiler` counters. With it, the
    // primitives can live in ui_kit without depending on chart-app code.
    _scaffold_lib::ui_kit::widgets::motion::set_animation_hooks(
        _scaffold_lib::foundation::frame_profiler::animation_started,
        _scaffold_lib::foundation::frame_profiler::animation_finished,
    );

    // Start performance monitoring — Prometheus metrics + jank detection + GPU telemetry
    _scaffold_lib::monitoring::start();

    // Discord OAuth2 — load client credentials from discord.env
    _scaffold_lib::discord::load_config();

    // Crypto real-time feed — connects to ApexCrypto WebSocket
    _scaffold_lib::crypto_feed::start();

    // Signals real-time feed — connects to ApexSignals WebSocket for patterns/alerts/trendlines
    _scaffold_lib::signals_feed::start();

    // DOM (L2 depth) feed — connects to apex-data /ws/dom for the active symbol
    _scaffold_lib::dom_feed::start();

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

    // Start the live market-data feeds (ApexData WS + pollers, IB WS,
    // connection-state listeners) and wire them into the chart. This is the
    // de-Tauri replacement for the old `tauri::Builder::setup` feed wiring.
    // Done after the chart sender is registered so the first frames have a
    // live receiver.
    _scaffold_lib::init_live_feeds();

    // Fetch initial data in background
    _scaffold_lib::chart_renderer::gpu::fetch_bars_background_pub("SPY".into(), "5m".into());

    // macOS requires the event loop on the main thread — open_window_blocking blocks here.
    // On other platforms open_window spawns a render thread and returns immediately,
    // so we sleep-poll until all windows are closed.
    #[cfg(target_os = "macos")]
    _scaffold_lib::chart_renderer::gpu::open_window_blocking(rx, initial);

    #[cfg(not(target_os = "macos"))]
    {
        _scaffold_lib::chart_renderer::gpu::open_window(rx, initial);
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
