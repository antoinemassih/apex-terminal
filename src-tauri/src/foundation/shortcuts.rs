//! Centralized keyboard-shortcut registry.
//!
//! Every global shortcut should be registered here so we can:
//!   - Detect conflicts at startup
//!   - Generate the help screen automatically
//!   - Surface bindings via the `Kbd` widget consistently
//!
//! Per-widget local shortcuts (text input, list nav) stay local. This
//! registry is for *global* and *panel-level* shortcuts.

use std::collections::HashMap;
use std::sync::OnceLock;
use egui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub modifiers: Modifiers,
    pub key: Key,
}

#[derive(Clone, Debug)]
pub struct ShortcutEntry {
    pub shortcut: Shortcut,
    pub action: &'static str,    // stable id, e.g. "command_palette.open"
    pub description: &'static str,
    pub category: &'static str,  // for grouping in help
}

pub struct ShortcutRegistry {
    entries: Vec<ShortcutEntry>,
}

impl ShortcutRegistry {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn register(&mut self, entry: ShortcutEntry) -> Result<(), String> {
        // detect conflict
        for existing in &self.entries {
            if existing.shortcut == entry.shortcut {
                return Err(format!(
                    "Shortcut conflict: {:?} already bound to '{}', tried to bind '{}'",
                    entry.shortcut, existing.action, entry.action,
                ));
            }
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn lookup(&self, action: &str) -> Option<&ShortcutEntry> {
        self.entries.iter().find(|e| e.action == action)
    }

    pub fn matches(&self, ctx: &egui::Context) -> Option<&'static str> {
        ctx.input(|i| {
            for e in &self.entries {
                if i.key_pressed(e.shortcut.key) && i.modifiers == e.shortcut.modifiers {
                    return Some(e.action);
                }
            }
            None
        })
    }

    pub fn all(&self) -> &[ShortcutEntry] { &self.entries }

    pub fn by_category(&self) -> HashMap<&'static str, Vec<&ShortcutEntry>> {
        let mut by_cat: HashMap<&'static str, Vec<&ShortcutEntry>> = HashMap::new();
        for e in &self.entries {
            by_cat.entry(e.category).or_default().push(e);
        }
        by_cat
    }

    /// Format a shortcut for display via `Kbd` widget.
    pub fn format_for_kbd(s: Shortcut) -> String {
        let mut parts: Vec<String> = Vec::new();
        if s.modifiers.command  { parts.push((if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" }).to_string()); }
        if s.modifiers.shift    { parts.push("Shift".to_string()); }
        if s.modifiers.alt      { parts.push("Alt".to_string()); }
        parts.push(format!("{:?}", s.key));
        parts.join("+")
    }
}

static REGISTRY: OnceLock<std::sync::RwLock<ShortcutRegistry>> = OnceLock::new();

pub fn registry() -> &'static std::sync::RwLock<ShortcutRegistry> {
    REGISTRY.get_or_init(|| std::sync::RwLock::new(ShortcutRegistry::new()))
}

/// Convenience: register a shortcut, logging on conflict (intended for
/// startup-time registration where conflicts are programmer errors).
pub fn register(entry: ShortcutEntry) {
    if let Err(msg) = registry().write().unwrap().register(entry) {
        eprintln!("[shortcuts] {}", msg);
    }
}

/// Helper builder for a Shortcut.
pub fn shortcut(key: Key) -> Shortcut {
    Shortcut { modifiers: Modifiers::NONE, key }
}

pub fn shortcut_cmd(key: Key) -> Shortcut {
    Shortcut { modifiers: Modifiers::COMMAND, key }
}

pub fn shortcut_cmd_shift(key: Key) -> Shortcut {
    Shortcut { modifiers: Modifiers { command: true, shift: true, ..Modifiers::NONE }, key }
}
