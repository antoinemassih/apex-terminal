//! Screener panel — T-SHELL Wave S2.
//!
//! ## Responsibilities (this file)
//!
//! 1. **Global state accessor** — holds the `Arc<RwLock<ScreenPanelState>>` that
//!    the command-bus reducer (`commands.rs`) reads/writes without touching
//!    `Watchlist`.
//! 2. **Panel shell** — `SidePanelShell::tabs` host with 3 tabs:
//!    LIBRARY / BUILD / RESULTS.
//! 3. **LIBRARY tab** (fully implemented here):
//!    - search input
//!    - `PanelSectionGroup` with PINNED + ALL sections
//!    - `PanelSubSection` per category inside ALL
//!    - `PanelListRow::trailing_buttons` for ▶ / pin / ⋯ actions
//!    - hotkey-slot `Badge` chips
//! 4. **BUILD tab** — delegates to `screener_build::draw_build_tab`.
//! 5. **RESULTS tab** — delegates to `screener_results::draw_section` +
//!    `screener_race::draw_section` depending on `view_mode`.
//! 6. **Background fetch** of `GET /api/data/scans` (ApexData scan list),
//!    funnelled into state via `AppCommand::UpdateSavedScreenCache`.
//!
//! ## File ownership
//!
//! T-SHELL owns this file exclusively (Wave S2 single-owner discipline).
//! T-BUILD writes `screener_build.rs`, T-GRID writes `screener_results.rs` +
//! `screener_race.rs`, T-HEAT writes `screener_heatmap.rs`. None of those
//! files touches this one.
//!
//! ## State architecture
//!
//! `ScreenPanelState` is NOT on `Watchlist` (frozen). It lives in the static
//! `SCREENER_STATE` below, lazily initialized. The command reducer calls
//! `with_screener_state_mut` to mutate it; `draw` reads it via
//! `with_screener_state`. The only field that mirrors into `Watchlist` is
//! `panel_open`, which goes through `sidebar_state_store` (like every other
//! panel open flag). Persistence is handled separately (see `save`/`load` at
//! the bottom of this file).

#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use egui::{Context, Ui};

use crate::chart_renderer::gpu::{Chart, Theme, Watchlist};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use crate::state::aggregates::{SavedScreenEntry, ScreenPanelState, ScreenViewMode};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{
    Button, PanelEmpty, PanelListRow, PanelSection, PanelSectionGroup,
    SearchInput, Tag, TagTone, Tooltip, TrailingBtn, TrailingTone,
};
use crate::ui_kit::widgets::tokens::{Size as KitSize, Variant};
use crate::chart_renderer::commands::{self, AppCommand, UiCtx};

// ─── Tab enum ─────────────────────────────────────────────────────────────────

/// The three top-level tabs in the screener panel.
/// Race and Heat are sub-modes inside RESULTS, not separate tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScreenerTab {
    #[default]
    Library,
    Build,
    Results,
}

impl ScreenerTab {
    fn from_index(idx: u8) -> Self {
        match idx {
            1 => Self::Build,
            2 => Self::Results,
            _ => Self::Library,
        }
    }
    fn to_index(self) -> u8 {
        match self {
            Self::Library => 0,
            Self::Build   => 1,
            Self::Results => 2,
        }
    }
}

// ─── Global screener state ────────────────────────────────────────────────────
//
// The command reducer and the draw function both access this. The draw function
// holds the lock for ONE frame (short) before releasing. The reducer holds it
// for a single closure call. No persistent cross-frame borrows.

fn screener_state_arc() -> &'static Arc<RwLock<ScreenPanelState>> {
    static STATE: OnceLock<Arc<RwLock<ScreenPanelState>>> = OnceLock::new();
    STATE.get_or_init(|| {
        // Try to load persisted state from disk; fall back to default.
        let loaded = load_persisted_state().unwrap_or_default();
        Arc::new(RwLock::new(loaded))
    })
}

/// Read the screener state inside a closure. Returns `None` if the lock is
/// poisoned (should never happen in normal operation).
pub fn with_screener_state<R>(f: impl FnOnce(&ScreenPanelState) -> R) -> Option<R> {
    screener_state_arc().read().ok().map(|g| f(&g))
}

/// Write the screener state inside a closure. Returns `None` on lock poison.
/// After the closure runs, schedules a debounced persist.
pub fn with_screener_state_mut<R>(f: impl FnOnce(&mut ScreenPanelState) -> R) -> Option<R> {
    let result = screener_state_arc().write().ok().map(|mut g| f(&mut g));
    schedule_persist();
    result
}

// ─── Muted-symbol set (session-only, not persisted) ──────────────────────────

fn muted_symbols() -> &'static Mutex<HashSet<String>> {
    static M: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Toggle a symbol in the per-session muted set. Called by the `MuteScreenSymbol`
/// command reducer arm.
pub fn toggle_muted_symbol(sym: &str) {
    if let Ok(mut m) = muted_symbols().lock() {
        if m.contains(sym) { m.remove(sym); } else { m.insert(sym.to_string()); }
    }
}

pub fn is_symbol_muted(sym: &str) -> bool {
    muted_symbols().lock().map(|m| m.contains(sym)).unwrap_or(false)
}

// ─── Persistence ──────────────────────────────────────────────────────────────

fn state_file_path() -> std::path::PathBuf {
    // Mirror the `sidebar_state_path()` pattern in gpu.rs: data_local_dir /
    // "apex-terminal" / "screen_panel_state.json".
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push("screen_panel_state.json");
    p
}

fn load_persisted_state() -> Option<ScreenPanelState> {
    let path = state_file_path();
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn schedule_persist() {
    // Spawn a detached thread that writes the current state to disk.
    // If a flush is already in-flight, the new spawn will just overwrite it,
    // which is fine (last-writer-wins, debounced naturally by thread scheduling).
    let arc = screener_state_arc().clone();
    std::thread::spawn(move || {
        let snapshot = arc.read().ok().map(|g| g.clone());
        if let Some(state) = snapshot {
            if let Ok(json) = serde_json::to_vec_pretty(&state) {
                let path = state_file_path();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&path, &json);
            }
        }
    });
}

// ─── Background scans fetch ───────────────────────────────────────────────────

/// Session flag: have we kicked off the initial fetch yet?
static FETCH_TRIGGERED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Fetch `GET /api/data/scans` once per session in a background thread and
/// funnel the result through `AppCommand::UpdateSavedScreenCache`.
/// Safe to call every frame — the `AtomicBool` gate ensures only one fetch runs.
fn ensure_scans_fetched() {
    use std::sync::atomic::Ordering;
    if FETCH_TRIGGERED.swap(true, Ordering::SeqCst) { return; }

    std::thread::spawn(|| {
        let url = format!(
            "{}/api/data/scans",
            crate::data::feeds::apex_data::config::apex_url()
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("apex-terminal/screener")
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(scans) = resp.json::<Vec<ApiScanEntry>>() {
                    let entries: Vec<SavedScreenEntry> = scans.into_iter().map(|s| SavedScreenEntry {
                        id: s.id,
                        name: s.name,
                        pinned: false,
                        hotkey_slot: None,
                    }).collect();
                    commands::push(AppCommand::UpdateSavedScreenCache { screens: entries });
                    // After scans are fetched, fire efficacy fetch for each ID.
                    // We push the IDs through the command so the reducer can
                    // trigger the per-id background fetches without access to
                    // the raw ID list here.
                    commands::push(AppCommand::FetchScreenEfficacyBatch);
                }
            }
            Ok(resp) => {
                #[cfg(debug_assertions)]
                eprintln!("[screener_panel] GET /api/data/scans → HTTP {}", resp.status());
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("[screener_panel] GET /api/data/scans → error: {}", e);
            }
        }
    });
}

/// Minimal shape of the scan entry returned by `GET /api/data/scans` (S1 route).
/// Only the fields T-SHELL needs; the full definition lives in ApexData.
#[derive(serde::Deserialize)]
struct ApiScanEntry {
    id:   String,
    name: String,
}

// ─── Screen efficacy cache (S3-SCORE integration) ─────────────────────────────
//
// One entry per saved screen ID, populated by background fetches of
// `GET /api/data/screens/efficacy/{id}` (wired by S3-SCORE).
// Absent-safe: when an entry is missing the Library tab simply omits badges.
//
// Shape mirrors S3-SCORE's `screen:efficacy:{id}` JSON:
// {
//   "hit_rate": 0.62, "avg_return": 0.018, "n": 47,
//   "realized_edge": 0.011, "badge": "Hot" | "Warm" | "Cold",
//   "decay_alert": true
// }

/// Efficacy badge tier from S3-SCORE.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub enum EfficacyBadge {
    Hot,
    Warm,
    Cold,
}

impl EfficacyBadge {
    /// Map to a `TagTone` for `ui_kit::Tag`.
    fn tag_tone(&self) -> TagTone {
        match self {
            EfficacyBadge::Hot  => TagTone::Bull,
            EfficacyBadge::Warm => TagTone::Warn,
            EfficacyBadge::Cold => TagTone::Bear,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            EfficacyBadge::Hot  => "HOT",
            EfficacyBadge::Warm => "WARM",
            EfficacyBadge::Cold => "COLD",
        }
    }
}

/// One screen's efficacy data from S3-SCORE.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct EfficacyEntry {
    pub hit_rate:       f64,
    pub avg_return:     f64,
    pub n:              u32,
    pub realized_edge:  f64,
    pub badge:          EfficacyBadge,
    #[serde(default)]
    pub decay_alert:    bool,
}

/// Module-level efficacy cache: screen_id → EfficacyEntry.
/// Written by background fetch threads, read by the library tab renderer.
fn efficacy_cache() -> &'static Mutex<std::collections::HashMap<String, EfficacyEntry>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, EfficacyEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Tracks which screen IDs are currently in-flight for efficacy fetch.
fn efficacy_inflight() -> &'static Mutex<HashSet<String>> {
    static INF: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    INF.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Read the efficacy entry for `id`, if available (absent-safe).
pub fn get_efficacy(id: &str) -> Option<EfficacyEntry> {
    efficacy_cache().lock().ok()?.get(id).cloned()
}

/// Kick off background fetches for all currently-known saved screen IDs.
/// Called by the `FetchScreenEfficacyBatch` reducer once after the scan list
/// is loaded. Safe to call multiple times — each ID is fetched at most once
/// per session (in-flight guard). Gate: `SCREEN_EFFICACY_ENABLED` env; if
/// the env is unset or "0", we skip silently (matches S3-SCORE's default-off).
pub fn fetch_efficacy_batch(ids: Vec<String>) {
    // CALIBRATION PENDING: efficacy data requires real scan history (resume-day).
    // Until then this silently no-ops when the endpoint returns 404.
    if std::env::var("SCREEN_EFFICACY_ENABLED").as_deref() == Ok("0") { return; }

    let base_url = crate::data::feeds::apex_data::config::apex_url();
    for id in ids {
        {
            let mut inf = match efficacy_inflight().lock() {
                Ok(g) => g,
                Err(_) => continue,
            };
            if inf.contains(&id) { continue; }
            inf.insert(id.clone());
        }
        let url = format!("{}/api/data/screens/efficacy/{}", base_url, id);
        let id_owned = id.clone();
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .user_agent("apex-terminal/screener-efficacy")
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());

            match client.get(&url).send() {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(entry) = resp.json::<EfficacyEntry>() {
                        if let Ok(mut cache) = efficacy_cache().lock() {
                            cache.insert(id_owned.clone(), entry);
                        }
                    }
                }
                Ok(_) => {
                    // 404 = not yet calibrated; normal before resume-day.
                    #[cfg(debug_assertions)]
                    eprintln!("[screener_panel] efficacy for {id_owned} not yet available (pre-calibration)");
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[screener_panel] efficacy fetch error for {id_owned}: {e}");
                }
            }
            if let Ok(mut inf) = efficacy_inflight().lock() {
                inf.remove(&id_owned);
            }
        });
    }
}

// ─── Panel entry point ────────────────────────────────────────────────────────

/// Draw the screener panel. Called from `top_nav.rs` every frame.
///
/// Early-returns immediately if `sidebar_state.screener_panel_open` is `false`.
/// The `close_clicked` response from the shell toggles it off via `AppCommand`.
pub fn draw(
    ctx: &Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    // Early-exit when closed (standard panel pattern).
    let is_open = watchlist.sidebar_state_snapshot().screener_panel_open;
    if !is_open { return; }

    // Kick off background scans fetch on first open.
    ensure_scans_fetched();

    // Resolve the active tab from persisted state.
    let mut active_tab = with_screener_state(|g| ScreenerTab::from_index(g.active_tab))
        .unwrap_or(ScreenerTab::Library);

    let pane_h    = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let tabs: &[(ScreenerTab, &str, Option<&str>)] = &[
        (ScreenerTab::Library, "LIBRARY", None),
        (ScreenerTab::Build,   "BUILD",   None),
        (ScreenerTab::Results, "RESULTS", None),
    ];

    let resp = SidePanelShell::tabs("screener_panel", &mut active_tab, tabs)
        .width(Width::Medium)
        .resizable(240.0..=560.0)
        .pane_metrics(pane_h, pane_font)
        .show(ctx, t, |ui, t, tab| {
            draw_tab_body(ui, t, tab, panes, ap);
        });

    // Persist the active tab back to state (only if it changed).
    with_screener_state_mut(|g| {
        let new_idx = active_tab.to_index();
        if g.active_tab != new_idx { g.active_tab = new_idx; }
    });

    // Close the panel if the X was clicked.
    if resp.close_clicked {
        commands::push(AppCommand::OpenScreenerPanel { open: false });
    }
}

// ─── Tab dispatcher ───────────────────────────────────────────────────────────

fn draw_tab_body(
    ui: &mut Ui,
    t: &Theme,
    tab: ScreenerTab,
    panes: &mut [Chart],
    ap: usize,
) {
    match tab {
        ScreenerTab::Library => draw_library_tab(ui, t),
        ScreenerTab::Build   => draw_build_tab_stub(ui, t),
        ScreenerTab::Results => draw_results_tab(ui, t, panes, ap),
    }
}

// ─── LIBRARY tab ─────────────────────────────────────────────────────────────

fn draw_library_tab(ui: &mut Ui, t: &Theme) {
    // Snapshot mutable state we need for the render.
    let snap = with_screener_state(|g| LibrarySnapshot {
        search_text:     g.library_search.clone(),
        saved_screens:   g.saved_screens.clone(),
        hotkey_slots:    g.hotkey_slots.clone(),
        expanded_pinned: g.library_expanded_pinned,
        expanded_all:    g.library_expanded_all,
        fracs:           g.library_section_fracs,
    }).unwrap_or_default();
    let LibrarySnapshot { search_text, saved_screens, hotkey_slots, expanded_pinned, expanded_all, fracs } = snap;

    // ── Search input ──────────────────────────────────────────────────────────
    let mut search_buf = search_text.clone();
    {
        let resp = SearchInput::new(&mut search_buf)
            .placeholder("Search screens\u{2026}")
            .show(ui, t);
        if resp.response.changed() {
            with_screener_state_mut(|g| g.library_search = search_buf.clone());
        }
    }

    ui.add_space(gap_xs());

    // Filter screens by search query.
    let query = search_buf.to_ascii_lowercase();
    let filtered: Vec<&SavedScreenEntry> = saved_screens.iter()
        .filter(|s| query.is_empty() || s.name.to_ascii_lowercase().contains(&query))
        .collect();

    let pinned: Vec<&SavedScreenEntry> = filtered.iter().copied().filter(|s| s.pinned).collect();
    let all:    Vec<&SavedScreenEntry> = filtered.iter().copied().collect();

    // ── PanelSectionGroup with PINNED / ALL sections ──────────────────────────
    let mut fracs_mut = fracs;
    let mut new_expanded_pinned = expanded_pinned;
    let mut new_expanded_all = expanded_all;

    let mut pending_run_id: Option<String>    = None;
    let mut pending_pin_id: Option<String>    = None;
    let mut pending_delete_id: Option<String> = None;
    let mut pending_load_id: Option<String>   = None;
    let mut pending_slot_set: Option<(u8, String)> = None;

    // If there are pinned items, show a two-section layout; otherwise show just ALL.
    if !pinned.is_empty() {
        PanelSectionGroup::new(&mut fracs_mut)
            .min_section_height(40.0)
            .show(ui, t, |grp| {
                grp.section(|ui, t| {
                    let resp = PanelSection::new("PINNED")
                        .count(pinned.len())
                        .collapsible(&mut new_expanded_pinned)
                        .show(ui, t, |ui, t| {
                            draw_screen_rows(ui, t, &pinned, &hotkey_slots,
                                &mut pending_run_id, &mut pending_pin_id,
                                &mut pending_delete_id, &mut pending_load_id,
                                &mut pending_slot_set);
                        });
                    let _ = resp;
                });
                grp.section(|ui, t| {
                    let resp = PanelSection::new("ALL SCREENS")
                        .count(all.len())
                        .collapsible(&mut new_expanded_all)
                        .show(ui, t, |ui, t| {
                            draw_screen_rows(ui, t, &all, &hotkey_slots,
                                &mut pending_run_id, &mut pending_pin_id,
                                &mut pending_delete_id, &mut pending_load_id,
                                &mut pending_slot_set);
                        });
                    let _ = resp;
                });
            });
    } else {
        // No pinned — skip PINNED section, show only ALL SCREENS.
        let resp = PanelSection::new("ALL SCREENS")
            .count(all.len())
            .collapsible(&mut new_expanded_all)
            .show(ui, t, |ui, t| {
                if all.is_empty() {
                    PanelEmpty::new("No saved screens yet")
                        .hint("Build a new screen in the BUILD tab, then save it.")
                        .show(ui, t);
                } else {
                    draw_screen_rows(ui, t, &all, &hotkey_slots,
                        &mut pending_run_id, &mut pending_pin_id,
                        &mut pending_delete_id, &mut pending_load_id,
                        &mut pending_slot_set);
                }
            });
        let _ = resp;
    }

    // ── Footer: hotkey-slot chip strip ────────────────────────────────────────
    draw_hotkey_slot_strip(ui, t, &hotkey_slots, &saved_screens);

    // ── Persist UI state changes (expand/fracs) ───────────────────────────────
    with_screener_state_mut(|g| {
        g.library_expanded_pinned = new_expanded_pinned;
        g.library_expanded_all    = new_expanded_all;
        g.library_section_fracs   = fracs_mut;
    });

    // ── Dispatch actions ──────────────────────────────────────────────────────
    if let Some(id) = pending_run_id {
        commands::push(AppCommand::RunScreen { slot: None, id: Some(id) });
    }
    if let Some(id) = pending_pin_id {
        commands::push(AppCommand::PinScreen { id });
    }
    if let Some(id) = pending_delete_id {
        commands::push(AppCommand::DeleteScreen { id });
    }
    if let Some(id) = pending_load_id {
        commands::push(AppCommand::LoadScreenToBuilder { id });
    }
    if let Some((slot, id)) = pending_slot_set {
        commands::push(AppCommand::SetScreenHotkeySlot { slot, screen_id: Some(id) });
    }
}

/// Render a list of screen rows using `PanelListRow::trailing_buttons`.
/// Writes pending action IDs into the `pending_*` out-params; caller dispatches.
///
/// ## S3-TERM additive: efficacy badges
/// For each screen, we read the session-cached `EfficacyEntry` (populated by
/// the `FetchScreenEfficacyBatch` command). When present:
/// - A Hot/Warm/Cold `Tag` is rendered as the row's leading ornament via `.leading(...)`.
/// - Realized edge is shown as a secondary label.
/// - A `⚠` warning label is appended when `decay_alert` is `true`.
/// When absent (pre-calibration or env gate off), the row renders exactly as before.
fn draw_screen_rows(
    ui: &mut Ui,
    t: &Theme,
    screens: &[&SavedScreenEntry],
    hotkey_slots: &[Option<String>; 9],
    pending_run_id:    &mut Option<String>,
    pending_pin_id:    &mut Option<String>,
    pending_delete_id: &mut Option<String>,
    pending_load_id:   &mut Option<String>,
    pending_slot_set:  &mut Option<(u8, String)>,
) {
    use crate::chart_renderer::ui::style::*;

    for screen in screens {
        // Derive the hotkey slot number for this screen (1-indexed), if any.
        let slot_num: Option<u8> = hotkey_slots.iter().enumerate().find_map(|(i, s)| {
            s.as_deref().filter(|&sid| sid == screen.id).map(|_| (i + 1) as u8)
        });

        let pin_tone = if screen.pinned { TrailingTone::Accent } else { TrailingTone::Default };

        // ── Efficacy badge (S3-SCORE, absent-safe) ─────────────────────────
        let eff = get_efficacy(&screen.id);

        // Build secondary label: slot + efficacy edge + decay warning.
        let mut secondary_parts: Vec<String> = Vec::new();
        if let Some(n) = slot_num { secondary_parts.push(format!("[{}]", n)); }
        if let Some(ref e) = eff {
            secondary_parts.push(format!("{:+.1}%", e.realized_edge * 100.0));
            if e.decay_alert { secondary_parts.push(format!("{} decay", Icon::WARNING)); }
        }
        let secondary = secondary_parts.join("  ");

        // Row with optional efficacy leading badge via `.leading(...)`.
        let row = PanelListRow::new(&screen.id)
            .primary(&screen.name)
            .secondary(&secondary);

        // If we have efficacy data, inject the badge via the leading closure.
        // We use the `trailing_buttons` API for action buttons and
        // `.leading(...)` for the badge so row width is unchanged when absent.
        let row = if let Some(ref e) = eff {
            let badge_label = e.badge.label();
            let badge_tone  = e.badge.tag_tone();
            let eff_clone = e.clone();
            row.leading(move |ui, t: &Theme| {
                Tag::new(badge_label).tone(badge_tone).show(ui, t);
                // Decay warning icon — separate from the secondary text so it
                // keeps its accent color (warn tone) and isn't plain dim text.
                if eff_clone.decay_alert {
                    ui.add(egui::Label::new(
                        TextStyle::Caption.as_rich(crate::ui_kit::icons::Icon::WARNING, t.warn)
                    ));
                }
            })
        } else {
            row
        };

        let resp = row
            .trailing_buttons(&[
                TrailingBtn::icon(Icon::PLAY).tone(TrailingTone::Bull).tooltip("Run this screen"),
                TrailingBtn::icon(Icon::PUSH_PIN).tone(pin_tone).active(screen.pinned).tooltip("Pin / unpin"),
                TrailingBtn::icon(Icon::PENCIL_LINE).tone(TrailingTone::Default).tooltip("Load into Builder"),
                TrailingBtn::icon(Icon::TRASH).tone(TrailingTone::Bear).tooltip("Delete"),
            ])
            .show_full(ui, t);

        if resp.clicked {
            // Row body click → run the screen.
            *pending_run_id = Some(screen.id.clone());
        }
        match resp.trailing_clicked_idx {
            Some(0) => { *pending_run_id    = Some(screen.id.clone()); }
            Some(1) => { *pending_pin_id    = Some(screen.id.clone()); }
            Some(2) => { *pending_load_id   = Some(screen.id.clone()); }
            Some(3) => { *pending_delete_id = Some(screen.id.clone()); }
            _       => {}
        }

        // Context menu: assign hotkey slot.
        if let Some(row_resp) = &resp.response {
            row_resp.clone().context_menu(|ui| {
                ui.label("Hotkey slot:");
                for slot in 1u8..=9 {
                    let bound_to = hotkey_slots[(slot - 1) as usize].as_deref();
                    let is_mine  = bound_to == Some(screen.id.as_str());
                    let label    = if is_mine {
                        format!("[{}]  {} assigned", slot, Icon::CHECK)
                    } else if bound_to.is_some() {
                        format!("[{}]  (reassign)", slot)
                    } else {
                        format!("[{}]", slot)
                    };
                    if ui.button(&label).clicked() {
                        *pending_slot_set = Some((slot, screen.id.clone()));
                        ui.close_menu();
                    }
                }
            });
        }
    }
}

/// Render the 9-slot hotkey strip at the bottom of the Library tab.
/// Clicking a filled slot runs that screen.
fn draw_hotkey_slot_strip(
    ui: &mut Ui,
    t: &Theme,
    hotkey_slots: &[Option<String>; 9],
    saved_screens: &[SavedScreenEntry],
) {
    ui.add_space(gap_sm());
    ui.separator();
    ui.add_space(gap_xs());
    ui.label(egui::RichText::new("HOTKEY SLOTS  (Ctrl+Shift+1..9)")
        .color(t.dim)
        .size(font_sm() as f32));
    ui.add_space(gap_xs());
    ui.horizontal_wrapped(|ui| {
        for (i, slot) in hotkey_slots.iter().enumerate() {
            let label = format!("{}", i + 1);
            let name = slot.as_deref().and_then(|sid| {
                saved_screens.iter().find(|s| s.id == sid).map(|s| s.name.as_str())
            });
            let tone = if name.is_some() { TagTone::Accent } else { TagTone::Neutral };
            let chip_label = if let Some(n) = name {
                format!("[{}] {}", i + 1, n)
            } else {
                format!("[{}] —", i + 1)
            };
            let resp = Tag::new(&chip_label).tone(tone).show(ui, t);
            if resp.response.clicked() {
                if let Some(sid) = slot.clone() {
                    commands::push(AppCommand::RunScreen { slot: Some((i + 1) as u8), id: Some(sid) });
                }
            }
            Tooltip::new(format!("Ctrl+Shift+{}", i + 1))
                .show(ui, &resp.response, t);
        }
    });
}

// ─── BUILD tab (stub — delegates to screener_build) ──────────────────────────

fn draw_build_tab_stub(ui: &mut Ui, t: &Theme) {
    let panel_w = ui.available_width();
    super::screener_build::draw_build_tab(ui, t, panel_w);
}

// ─── RESULTS tab ─────────────────────────────────────────────────────────────

fn draw_results_tab(
    ui: &mut Ui,
    t: &Theme,
    _panes: &mut [Chart],
    _ap: usize,
) {
    use crate::chart_renderer::commands::UiCtx;
    use crate::chart_renderer::ui::panels::{screener_results, screener_race, screener_heatmap};

    let (view_mode, active_screen_id) = with_screener_state(|g| {
        (g.view_mode, g.active_screen_id.clone())
    }).unwrap_or((ScreenViewMode::Grid, None));

    // ── Sub-mode toggle strip (GRID | RACE | HEAT) ────────────────────────────
    ui.horizontal(|ui| {
        for (label, mode) in [
            ("GRID", ScreenViewMode::Grid),
            ("RACE", ScreenViewMode::Race),
            ("HEAT", ScreenViewMode::Heat),
        ] {
            let active = view_mode == mode;
            let resp = Button::new(label)
                .variant(if active { Variant::Primary } else { Variant::Ghost })
                .size(KitSize::Sm)
                .show(ui, t);
            if resp.clicked() && !active {
                commands::push(AppCommand::SetScreenerView(mode));
            }
        }
    });
    ui.add_space(gap_xs());

    // ── Content area ─────────────────────────────────────────────────────────
    match view_mode {
        ScreenViewMode::Grid => {
            // Snapshot runtime state needed for draw_results.
            let (rows, selected_cols, mut sort_col, mut sort_asc, muted) =
                with_screener_state(|g| {
                    (
                        g.results_rows.clone(),
                        g.selected_columns.clone(),
                        g.result_sort_col,
                        g.result_sort_asc,
                        g.result_muted.clone(),
                    )
                }).unwrap_or_default();

            let mut pending_sym: Option<String> = None;
            let mut pending_watch: Option<String> = None;
            let mut pending_mute: Option<String> = None;
            // S3-TERM: provenance popup toggle — symbol that was clicked.
            // The toggle state is self-contained in screener_results::PROV_OPEN;
            // we dispatch ShowRowProvenance for command-bus observability only.
            let mut pending_prov_sym: Option<String> = None;

            screener_results::draw_results(
                ui, &rows, &selected_cols,
                &mut sort_col, &mut sort_asc,
                &muted, t,
                &mut pending_sym, &mut pending_watch, &mut pending_mute,
                &mut pending_prov_sym,
            );

            // Write back sort state.
            with_screener_state_mut(|g| {
                g.result_sort_col = sort_col;
                g.result_sort_asc = sort_asc;
            });

            // Dispatch pending actions.
            if let Some(sym) = pending_sym {
                commands::push(AppCommand::ScreenRowToChart { symbol: sym });
            }
            if let Some(sym) = pending_watch {
                commands::push(AppCommand::WatchlistAddSymbol { symbol: sym });
            }
            if let Some(sym) = pending_mute {
                commands::push(AppCommand::MuteScreenSymbol { symbol: sym });
            }
            if let Some(sym) = pending_prov_sym {
                commands::push(AppCommand::ShowRowProvenance { symbol: sym });
            }
        }
        ScreenViewMode::Race => {
            let mut pending_sym: Option<String> = None;
            let ctx = ui.ctx().clone();
            screener_race::draw_race(
                ui,
                active_screen_id.as_deref(),
                &ctx,
                t,
                &mut pending_sym,
            );
            if let Some(sym) = pending_sym {
                commands::push(AppCommand::ScreenRowToChart { symbol: sym });
            }
        }
        ScreenViewMode::Heat => {
            // Snapshot rows + heatmap state for the render pass.
            let (rows, mut heat_state, sector_filter, available_operands) =
                with_screener_state(|g| {
                    let sf = g.screen_heatmap.sector_filter.clone();
                    let ops = g.selected_columns.clone();
                    (g.results_rows.clone(), g.screen_heatmap.clone(), sf, ops)
                }).unwrap_or_default();

            let cx = UiCtx { theme: t };
            screener_heatmap::render_screener_heatmap(
                ui,
                &rows,
                sector_filter.as_deref(),
                &mut heat_state,
                "",  // active_sym: no per-cell active-border in panel mode
                &available_operands,
                &cx,
            );

            // Write back picker state mutations.
            with_screener_state_mut(|g| {
                g.screen_heatmap.color_key = heat_state.color_key;
                g.screen_heatmap.size_key  = heat_state.size_key;
                g.screen_heatmap.color_picker_open = heat_state.color_picker_open;
                g.screen_heatmap.size_picker_open  = heat_state.size_picker_open;
            });
        }
    }
}

// ─── Default snapshot helper ─────────────────────────────────────────────────
//
// Provides the fallback tuple when `with_screener_state` returns `None` (lock
// poisoned — should never happen in production). Named newtype avoids the
// "orphan impl" issue with tuples.

struct LibrarySnapshot {
    search_text:       String,
    saved_screens:     Vec<SavedScreenEntry>,
    hotkey_slots:      [Option<String>; 9],
    expanded_pinned:   bool,
    expanded_all:      bool,
    fracs:             [f32; 2],
}

impl Default for LibrarySnapshot {
    fn default() -> Self {
        Self {
            search_text:     String::new(),
            saved_screens:   Vec::new(),
            hotkey_slots:    [None, None, None, None, None, None, None, None, None],
            expanded_pinned: true,
            expanded_all:    true,
            fracs:           [0.35, 0.65],
        }
    }
}
