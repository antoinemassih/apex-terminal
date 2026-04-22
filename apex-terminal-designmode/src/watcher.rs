//! File watcher — monitors design.toml for changes and hot-reloads tokens.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use notify::{Watcher, RecursiveMode, Event, EventKind};

use crate::tokens::{DesignTokens, update_tokens};

/// Start watching a TOML file for changes. Reloads tokens on save.
/// Returns a channel receiver that signals when tokens have been updated.
pub fn start_watcher(toml_path: PathBuf) -> mpsc::Receiver<()> {
    let (notify_tx, notify_rx) = mpsc::channel();

    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        let _ = tx.send(());
                    }
                    _ => {}
                }
            }
        }).expect("[design-mode] Failed to create file watcher");

        // Watch the parent directory (more reliable on Windows)
        if let Some(parent) = toml_path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive)
                .expect("[design-mode] Failed to watch directory");
        }

        eprintln!("[design-mode] Watching {:?} for changes", toml_path);

        let mut last_reload = Instant::now();
        let debounce = Duration::from_millis(200);

        loop {
            // Block until we get a file change event
            if rx.recv().is_err() {
                break;
            }

            // Debounce — drain rapid successive events
            std::thread::sleep(debounce);
            while rx.try_recv().is_ok() {}

            // Skip if we just reloaded
            if last_reload.elapsed() < debounce {
                continue;
            }

            // Reload tokens
            let tokens = DesignTokens::load(&toml_path);
            update_tokens(tokens);
            last_reload = Instant::now();

            eprintln!("[design-mode] Tokens reloaded from {:?}", toml_path);
            let _ = notify_tx.send(());
        }
    });

    notify_rx
}
