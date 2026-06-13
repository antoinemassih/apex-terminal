//! Activation bridge — wires a [`ThemePack`] into the live renderer registries.
//!
//! ## What `activate_theme_pack` does
//!
//! 1. Converts `pack.color_scheme` → [`gpu::Theme`] via `color_scheme_to_theme`
//!    and **upserts** it into the live THEMES store (append once, overwrite on
//!    re-activate).  The resulting index is applied to every pane via
//!    `AppCommand::SetThemeIdx` and stashed in egui memory so the ambient-theme
//!    path picks it up immediately.
//!
//! 2. Converts `pack.style_system` → `StyleSettings` via
//!    `style_system_to_style_settings`, registers it via `add_style_preset`, and
//!    selects it via `set_active_style`.  Re-activating the same pack reuses the
//!    same slot (slot detected by matching preset name; no new slot is appended).
//!
//! 3. Calls `set_ambient_recipes(ctx, Arc::new(pack.recipes.clone()))` so
//!    ui_kit widgets built via `StyleCtx::from_ctx` pick up recipe overrides.
//!
//! 4. Loads fonts via `init_fonts_from_typography` when the pack's style system
//!    declares non-default font families.
//!
//! ## Idempotency
//!
//! Re-activating the same pack does NOT append new slots:
//! - Theme slot: `upsert_installed_themes` overwrites by name.
//! - Style slot: searched by name; `set_style_settings` overwrites in place.
//! - Recipes / fonts: stateless replacement each call.
//!
//! ## Global registry
//!
//! `PACK_REGISTRY` is a process-wide `OnceLock<RwLock<PackRegistry>>` so
//! the settings UI (called per-frame inside egui) can query it without a
//! separate lifetime / reference chain.  Initialised by `init_registry`.

use std::sync::{Arc, OnceLock, RwLock};

use egui::Context;

use crate::{
    chart_renderer::{
        commands::{self, AppCommand},
        theme_adapter::color_scheme_to_theme,
    },
    design_system::theme_pack::{PackRegistry, RegistryError, ThemePack},
};

// ─── Global PackRegistry ──────────────────────────────────────────────────────

static PACK_REGISTRY: OnceLock<RwLock<PackRegistry>> = OnceLock::new();

/// Initialise the global `PackRegistry` from `themes_dir`.
///
/// This is the pre-egui phase: opens (or creates) the registry directory,
/// scans installed packs, and reads the persisted active selection.
/// **No egui `Context` is required here** — the actual pack activation
/// happens on the first UI frame via [`apply_startup_active_pack`].
///
/// Safe to call multiple times — subsequent calls after the first are no-ops.
pub fn open_registry(themes_dir: &std::path::Path) {
    // Sub-directory: <themes_dir>/pack-registry/ keeps pack bundles separate
    // from the DTCG JSON files the hot-reload watcher manages.
    let registry_dir = themes_dir.join("pack-registry");
    PACK_REGISTRY.get_or_init(|| {
        let reg = PackRegistry::open(&registry_dir).unwrap_or_else(|e| {
            eprintln!("[theme-pack-bridge] registry open error ({e}); using in-memory fallback");
            let tmp = std::env::temp_dir().join("apex-theme-registry-fallback");
            PackRegistry::open(&tmp)
                .expect("fallback temp-dir registry must succeed")
        });
        RwLock::new(reg)
    });
}

/// Apply the persisted active pack on the first egui frame.
///
/// Call this once from the main render loop after egui is initialised.
/// It is idempotent: a `OnceLock<()>` guards against double-application.
pub fn apply_startup_active_pack(ctx: &Context) {
    static APPLIED: OnceLock<()> = OnceLock::new();
    APPLIED.get_or_init(|| {
        let active_id = with_registry_read(|reg| reg.active().map(str::to_owned));
        if let Some(id) = active_id.flatten() {
            if let Err(e) = activate_by_id(ctx, &id) {
                eprintln!("[theme-pack-bridge] startup activate '{id}' failed: {e}");
            }
        }
    });
}

// ─── Read-only helpers (safe for per-frame UI calls) ─────────────────────────

/// Return the manifests of all registered packs (built-in + installed).
/// Returns an empty `Vec` when the registry has not been initialised yet.
pub fn list_pack_manifests() -> Vec<crate::design_system::theme_pack::manifest::ThemeManifest> {
    with_registry_read(|reg| reg.list().into_iter().cloned().collect())
        .unwrap_or_default()
}

/// Currently-active pack id, or `None`.
pub fn active_pack_id() -> Option<String> {
    with_registry_read(|reg| reg.active().map(str::to_owned))?
}

/// Returns `true` if `id` is a built-in (cannot be uninstalled).
pub fn is_builtin(id: &str) -> bool {
    with_registry_read(|reg| reg.is_builtin(id))
        .unwrap_or(false)
}

// ─── Mutating operations ──────────────────────────────────────────────────────

/// Install a `.apextheme` bundle into the registry.
///
/// Runs the S9 validation gate inside `PackRegistry::install`.
pub fn install_pack(
    bundle_path: &std::path::Path,
) -> Result<crate::design_system::theme_pack::manifest::ThemeManifest, RegistryError> {
    with_registry_write(|reg| reg.install(bundle_path))
        .ok_or_else(|| RegistryError::Io(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "PackRegistry not initialised",
        )))?
}

/// Uninstall a pack by id.
///
/// If the pack was active the active selection is cleared, but the visual
/// theme is NOT reset (stays until the user picks another).
pub fn uninstall_pack(id: &str) -> Result<(), RegistryError> {
    with_registry_write(|reg| reg.uninstall(id))
        .ok_or_else(|| RegistryError::Io(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "PackRegistry not initialised",
        )))?
}

// ─── Activation ──────────────────────────────────────────────────────────────

/// Look up `id` in the registry and activate it.
///
/// - If `id` is an installed pack, the full ThemePack is loaded and all four
///   axes (color, style, recipes, fonts) are applied.
/// - If `id` is a built-in, only the color axis is applied (built-ins have no
///   custom style / recipe / font overrides).
pub fn activate_by_id(ctx: &Context, id: &str) -> Result<(), RegistryError> {
    // Load the pack if it's an installed (non-builtin) bundle.
    let pack_opt: Option<ThemePack> = with_registry_read(|reg| {
        match reg.load_pack(id) {
            Some(Ok(p)) => Some(p),
            Some(Err(e)) => {
                eprintln!("[theme-pack-bridge] failed to read bundle for '{id}': {e}");
                None
            }
            None => None, // builtin or not found
        }
    })
    .unwrap_or(None);

    if let Some(pack) = pack_opt {
        activate_theme_pack(ctx, &pack);
    } else {
        // Builtin: activate only the color axis.
        let schemes = crate::design_system::builtin_color_schemes();
        if let Some(cs) = schemes.iter().find(|cs| cs.meta.id == id || cs.meta.name == id) {
            let theme = color_scheme_to_theme(cs);
            let idx = upsert_theme_into_live_store(theme);
            commands::push(AppCommand::SetThemeIdx { pane: 0, idx });
        }
        // If not found as a builtin either, we still persist the intent (maybe
        // the pack will appear after a hot-reload).
    }

    // Persist the active selection.
    with_registry_write(|reg| reg.set_active(id))
        .ok_or_else(|| RegistryError::Io(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "PackRegistry not initialised",
        )))?
}

/// Activate a fully-loaded [`ThemePack`] into all live renderer registries.
///
/// **Idempotent**: calling with the same pack name reuses the same slots.
pub fn activate_theme_pack(ctx: &Context, pack: &ThemePack) {
    // ── 1. Color axis → THEMES store ─────────────────────────────────────────
    let theme = color_scheme_to_theme(&pack.color_scheme);
    let theme_idx = upsert_theme_into_live_store(theme);

    // Apply to every pane via the command queue.
    commands::push(AppCommand::SetThemeIdx { pane: 0, idx: theme_idx });

    // Also stash in egui data immediately (avoids a one-frame flash).
    let resolved = crate::chart_renderer::gpu::get_theme(theme_idx);
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("apex_active_theme_idx"), theme_idx);
        d.insert_temp(egui::Id::new("apex_ambient_theme"), resolved);
    });

    // ── 2. Dimension axis → style store ──────────────────────────────────────
    use crate::chart_renderer::ui::style::{
        add_style_preset, list_style_presets, set_active_style, set_style_settings,
        style_system_to_style_settings,
    };
    let settings = style_system_to_style_settings(&pack.style_system);
    let pack_name = pack.manifest.name.clone();

    let presets = list_style_presets();
    let style_idx: u8 = if let Some((id, _)) = presets.iter().find(|(_, n)| n == &pack_name) {
        set_style_settings(*id, settings);
        *id
    } else {
        add_style_preset(&pack_name, settings)
    };
    set_active_style(style_idx);

    // ── 3. Recipes → ambient egui stash ──────────────────────────────────────
    crate::ui_kit::widgets::theme::set_ambient_recipes(
        ctx,
        Arc::new(pack.recipes.clone()),
    );

    // ── 4. Typography → font loader ───────────────────────────────────────────
    let typo = &pack.style_system.typography;
    let default_ui      = "Inter";
    let default_mono    = "JetBrains Mono";
    let default_display = "Inter";
    // Only reload fonts if the pack specifies different families from the
    // compiled-in defaults; avoids a full atlas rebuild for no-op changes.
    let needs_font_load = typo.family_ui      != default_ui
        || typo.family_mono    != default_mono
        || typo.family_display != default_display;
    if needs_font_load {
        crate::ui_kit::icons::init_fonts_from_typography(
            ctx,
            &typo.family_ui,
            &typo.family_mono,
            &typo.family_display,
        );
    }
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn with_registry_read<F, T>(f: F) -> Option<T>
where
    F: FnOnce(&PackRegistry) -> T,
{
    let lock = PACK_REGISTRY.get()?;
    let guard = lock.read().unwrap_or_else(|e| e.into_inner());
    Some(f(&guard))
}

fn with_registry_write<F, T>(f: F) -> Option<T>
where
    F: FnOnce(&mut PackRegistry) -> T,
{
    let lock = PACK_REGISTRY.get()?;
    let mut guard = lock.write().unwrap_or_else(|e| e.into_inner());
    Some(f(&mut guard))
}

/// Insert or overwrite `theme` in the live THEMES store by name.
/// Returns the store index of the (possibly-new) slot.
fn upsert_theme_into_live_store(theme: crate::chart_renderer::gpu::Theme) -> usize {
    use crate::chart_renderer::gpu::{get_all_themes, set_theme};

    let all = get_all_themes();
    if let Some(idx) = all.iter().position(|t| t.name == theme.name) {
        set_theme(idx, theme);
        idx
    } else {
        // Append via the existing public helper by constructing a single-element
        // ColorScheme that round-trips to an identical Theme.
        let cs = reverse_theme_to_color_scheme(&theme);
        crate::chart_renderer::gpu::upsert_installed_themes(vec![cs]);
        // Find the index of the newly-appended slot.
        let all2 = get_all_themes();
        all2.iter()
            .position(|t| t.name == theme.name)
            .unwrap_or(0)
    }
}

/// Minimal reverse map of a [`Theme`] back to a [`ColorScheme`] for use as
/// the `upsert_installed_themes` payload.
///
/// Only the base fields that `color_scheme_to_theme` reads are populated;
/// derived fields (toolbar_border, element_hover, …) are recomputed by the
/// adapter on the next round-trip so the result is lossless for what matters.
fn reverse_theme_to_color_scheme(
    t: &crate::chart_renderer::gpu::Theme,
) -> crate::design_system::color_scheme::ColorScheme {
    use crate::design_system::color_scheme::{ColorScheme, Meta, CMD_PALETTE_DEFAULT};

    let c = |c: egui::Color32| -> [u8; 4] { [c.r(), c.g(), c.b(), c.a()] };

    ColorScheme {
        meta: Meta::new(
            t.name.to_lowercase().replace(' ', "-"),
            t.name.to_string(),
            !t.is_light(),
        ),
        bg:      c(t.bg),
        surface: c(t.toolbar_bg),
        text:    c(t.text),
        dim:     c(t.dim),
        border:  c(t.toolbar_border),
        accent:  c(t.accent),
        bull:    c(t.bull),
        bear:    c(t.bear),
        warn:    c(t.warn),
        success: None,
        danger:  None,
        warning: None,
        info:    None,
        pane_gap_color: None,
        shadow:  c(t.shadow_color),
        notification_red: c(t.notification_red),
        gold:             c(t.gold),
        overlay_text:     c(t.overlay_text),
        rrg_leading:      c(t.rrg_leading),
        rrg_improving:    c(t.rrg_improving),
        rrg_weakening:    c(t.rrg_weakening),
        rrg_lagging:      c(t.rrg_lagging),
        pinned_row_tint:  c(t.pinned_row_tint),
        text_muted:       c(t.text_muted),
        hud_bg:           c(t.hud_bg),
        hud_border:       c(t.hud_border),
        cmd_palette: t.cmd_palette.map(|col| [col.r(), col.g(), col.b(), col.a()]),
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::{
        builtin::builtin_color_schemes,
        color_scheme::builtin_dark,
    };

    /// Verify that `upsert_theme_into_live_store` is idempotent:
    /// calling it twice with the same theme name returns the same index and
    /// does not grow the store.
    #[test]
    fn upsert_is_idempotent() {
        let cs = builtin_color_schemes().into_iter().next().unwrap();
        let theme = color_scheme_to_theme(&cs);
        let name = theme.name;

        let idx1 = upsert_theme_into_live_store(theme.clone());
        let idx2 = upsert_theme_into_live_store(theme);

        assert_eq!(idx1, idx2, "same theme name must reuse the same slot");

        let all = crate::chart_renderer::gpu::get_all_themes();
        let count = all.iter().filter(|t| t.name == name).count();
        assert_eq!(count, 1, "no duplicate entries for the same theme name");
    }

    /// Verify that `reverse_theme_to_color_scheme` round-trips the key semantic
    /// colours (bull / bear / accent) correctly.
    #[test]
    fn reverse_theme_colour_round_trip() {
        let cs = builtin_dark();
        let theme = color_scheme_to_theme(&cs);
        let back  = reverse_theme_to_color_scheme(&theme);

        let c = |c: egui::Color32| -> [u8; 4] { [c.r(), c.g(), c.b(), c.a()] };
        assert_eq!(back.bull,   c(theme.bull));
        assert_eq!(back.bear,   c(theme.bear));
        assert_eq!(back.accent, c(theme.accent));
    }
}
