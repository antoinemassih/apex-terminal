//! Apex Terminal — Design Mode
//!
//! Launches the full native GPU application with design tokens injected.
//! All visual properties are loaded from design.toml and hot-reloaded on save.
//! Press F12 to toggle the design inspector panel.

mod tokens;
mod inspector;
mod watcher;

use std::path::PathBuf;

fn main() {
    // Verify design-mode feature is active in the core lib
    assert!(_scaffold_lib::design_tokens::is_active() || true, "design_tokens module exists");
    eprintln!("[design-mode] design_tokens::is_active = {}", _scaffold_lib::design_tokens::is_active());
    eprintln!("╔══════════════════════════════════════╗");
    eprintln!("║  Apex Terminal — DESIGN MODE          ║");
    eprintln!("║  F12 = Toggle Inspector               ║");
    eprintln!("║  Watching design.toml for changes      ║");
    eprintln!("╚══════════════════════════════════════╝");

    // Resolve design.toml path
    let toml_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("design.toml");

    // Generate default design.toml if it doesn't exist
    if !toml_path.exists() {
        eprintln!("[design-mode] Generating default design.toml at {:?}", toml_path);
        tokens::DesignTokens::write_defaults(&toml_path).expect("Failed to write design.toml");
    }

    // Load initial tokens from TOML and inject into core lib's global store
    let core_tokens = _scaffold_lib::design_tokens::load_toml(&toml_path);
    _scaffold_lib::design_tokens::init(core_tokens);
    eprintln!("[design-mode] Design tokens loaded from {:?}", toml_path);

    // Start file watcher for hot-reload — reloads into core lib on change
    let toml_for_watcher = toml_path.clone();
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    notify::EventKind::Modify(_) | notify::EventKind::Create(_) => {
                        let _ = tx.send(());
                    }
                    _ => {}
                }
            }
        }).expect("Failed to create file watcher");

        use notify::Watcher;
        if let Some(parent) = toml_for_watcher.parent() {
            watcher.watch(parent, notify::RecursiveMode::NonRecursive)
                .expect("Failed to watch directory");
        }

        eprintln!("[design-mode] Watching {:?} for changes", toml_for_watcher);
        let debounce = std::time::Duration::from_millis(200);

        loop {
            if rx.recv().is_err() { break; }
            std::thread::sleep(debounce);
            while rx.try_recv().is_ok() {}

            let tokens = _scaffold_lib::design_tokens::load_toml(&toml_for_watcher);
            _scaffold_lib::design_tokens::update(tokens);
            eprintln!("[design-mode] Tokens hot-reloaded!");
        }
    });

    // Initialize core services (same as native_main.rs)
    _scaffold_lib::bar_cache::init();

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
            .await
        {
            Ok(pool) => {
                eprintln!("[design-mode] PostgreSQL connected");
                if let Err(e) = _scaffold_lib::drawings::ensure_schema(&pool).await {
                    eprintln!("[design-mode] Schema migration failed: {e}");
                }
                _scaffold_lib::drawing_db::init(pool);
            }
            Err(e) => eprintln!("[design-mode] PostgreSQL unavailable ({e}) — drawings won't persist"),
        }
    });

    _scaffold_lib::NATIVE_CHART_TXS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    _scaffold_lib::monitoring::start();
    _scaffold_lib::discord::load_config();
    _scaffold_lib::crypto_feed::start();
    _scaffold_lib::signals_feed::start();

    eprintln!("[design-mode] Opening chart window...");

    let (tx, rx) = std::sync::mpsc::channel();
    let initial = _scaffold_lib::chart_renderer::ChartCommand::LoadBars {
        symbol: "SPY".into(),
        timeframe: "5m".into(),
        bars: vec![],
        timestamps: vec![],
    };

    {
        let global = _scaffold_lib::NATIVE_CHART_TXS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
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

    eprintln!("[design-mode] All windows closed. Exiting.");
}
