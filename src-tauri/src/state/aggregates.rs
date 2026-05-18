//! Skeleton aggregate structs — each represents a focused slice of state
//! that *should* live outside `Watchlist` once Wave 5+ has had a chance
//! to migrate the field references.
//!
//! Wave 5 deliberately does **not** move fields. The aggregates exist so
//! follow-up waves can move one slice at a time without first having to
//! invent a destination, decide on serialization, or argue about
//! versioning. Each aggregate:
//!
//! - is `Default + Serialize + Deserialize` so it slots into
//!   `state::persistence::Persistable` immediately;
//! - has a stable `KEY` and `VERSION = 1`;
//! - documents the specific `Watchlist` fields that should eventually
//!   migrate into it (grep the audit notes in each doc comment).
//!
//! When migrating a field into an aggregate, the rule of thumb is:
//! introduce the aggregate field on `Watchlist`, add a delegating
//! accessor for the old name, then sweep call sites in a separate PR.
//! Never delete a field on `Watchlist` in the same PR that introduces
//! the replacement — `core.rs` is sacred and must keep compiling.

use super::persistence::Persistable;

/// UI display preferences — theme, font, density, panel chrome toggles.
///
/// **Wave 14c** populates this aggregate. Fields are mirrored from the
/// matching `Watchlist::*` fields via
/// `Watchlist::push_to_ui_settings` (write before save) and
/// `Watchlist::pull_from_ui_settings` (write after load). Until a
/// follow-up wave can flip the source-of-truth, the legacy `Watchlist`
/// fields remain authoritative for reads (notably the sacred
/// `core.rs` paint pipeline reads them directly).
///
/// Excluded from this aggregate:
/// - `native_dpi_scale: f32` — derived from `Window::scale_factor()` at
///   runtime, must not be persisted.
/// - `toolbar_hover_time: Option<Instant>` — runtime-only animation
///   state; `Instant` is not serializable.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct UiSettings {
    #[serde(default = "default_font_scale")]
    pub(crate) font_scale: f32,
    #[serde(default)]
    pub(crate) font_idx: usize,
    #[serde(default)]
    pub(crate) compact_mode: bool,
    #[serde(default = "default_pane_header_size")]
    pub(crate) pane_header_size: crate::chart_renderer::PaneHeaderSize,
    #[serde(default)]
    pub(crate) toolbar_auto_hide: bool,
    #[serde(default = "default_true")]
    pub(crate) show_x_axis: bool,
    #[serde(default = "default_true")]
    pub(crate) show_y_axis: bool,
    #[serde(default)]
    pub(crate) shared_x_axis: bool,
    #[serde(default)]
    pub(crate) shared_y_axis: bool,
    #[serde(default)]
    pub(crate) style_idx: usize,
}

fn default_font_scale() -> f32 { 1.6 }
fn default_true() -> bool { true }
fn default_pane_header_size() -> crate::chart_renderer::PaneHeaderSize {
    crate::chart_renderer::PaneHeaderSize::Compact
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            font_scale: default_font_scale(),
            font_idx: 0,
            compact_mode: false,
            pane_header_size: default_pane_header_size(),
            toolbar_auto_hide: false,
            show_x_axis: true,
            show_y_axis: true,
            shared_x_axis: false,
            shared_y_axis: false,
            style_idx: 0,
        }
    }
}

impl Persistable for UiSettings {
    const KEY: &'static str = "ui_settings";
    const VERSION: u32 = 1;
}

/// Trading defaults — order qty, TIF, hotkeys, the armed flag.
///
/// Migrate from `Watchlist`:
/// - `default_stock_qty: u32`
/// - `default_options_qty: u32`
/// - `default_order_type: usize`
/// - `default_tif: usize`
/// - `default_outside_rth: bool`
/// - `hotkeys: Vec<HotKey>`
/// - `hotkey_editor_open: bool` (UI-only — could go in SidebarState instead)
/// - `hotkey_editing_id: Option<u32>`
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradingDefaults {}

impl Persistable for TradingDefaults {
    const KEY: &'static str = "trading_defaults";
    const VERSION: u32 = 1;
}

/// Alerts list + their state.
///
/// Migrate from `Watchlist`:
/// - `alerts: Vec<Alert>`
/// - `next_alert_id: u32`
/// - `alert_query: String`
/// - `alerts_panel_open: bool` (UI-only — could go in SidebarState)
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AlertsState {}

impl Persistable for AlertsState {
    const KEY: &'static str = "alerts_state";
    const VERSION: u32 = 1;
}

/// Discord / signals chat state.
///
/// Migrate from `Watchlist`:
/// - `discord_open: bool` (UI — SidebarState candidate)
/// - `discord_messages: Vec<DiscordMessage>`
/// - `discord_input: String`
/// - `discord_channel: String`
/// - `discord_authenticated: bool`
/// - `discord_username: String`, `discord_user_id: String`
/// - `discord_guilds`, `discord_selected_guild`
/// - `discord_channels`, `discord_selected_channel`
/// - `discord_connecting: bool`
/// - `discord_guild_icons: HashMap<String, TextureHandle>` (runtime only — exclude)
/// - `discord_last_msg_id: Option<String>`
/// - `discord_poll_timer: Option<Instant>` (runtime only — exclude)
/// - `discord_channels_loading: bool` → migrate to `InFlightRegistry`
/// - `discord_messages_loading: bool` → migrate to `InFlightRegistry`
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatState {}

impl Persistable for ChatState {
    const KEY: &'static str = "chat_state";
    const VERSION: u32 = 1;
}

/// Sidebar / side-panel open state, widths, focus.
///
/// Migrate from `Watchlist` (the `*_panel_open: bool` / `*_open: bool` family):
/// - `open: bool` (watchlist itself)
/// - `orders_panel_open`, `order_entry_open`
/// - `order_ledger_open`, `order_ledger_view`, `order_ledger_filter`, `order_ledger_search`
/// - `order_health_open`
/// - `account_strip_open`, `object_tree_open`, `trendline_filter_open`
/// - `apex_diag_open`, `widget_gallery_open`
/// - `filter_open`, `wl_columns_open`
/// - `cmd_palette_open` + the whole cmd_palette_* family (UI scratch state)
/// - `layout_dropdown_open`, `timeframe_dropdown_open`
/// - `tape_open`, `news_open`, `journal_open`
/// - `scanner_open`, `scanner_builder_open`
/// - `spread_open`, `script_open`, `screenshot_open`, `rrg_open`
/// - `analysis_open`, `signals_panel_open`, `indicators_panel_open`
/// - `feed_panel_open`, `playbook_panel_open`, `journal_panel_open`
/// - `settings_open`, `discord_open` (overlap with ChatState — pick one)
/// - the split-section fraction arrays + tab indices for each side panel
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SidebarState {}

impl Persistable for SidebarState {
    const KEY: &'static str = "sidebar_state";
    const VERSION: u32 = 1;
}

/// Layout state — grid choice, pane split ratios, link groups, broadcast mode.
///
/// Migrate from `Watchlist`:
/// - `link_groups: Vec<LinkGroup>`
/// - `broadcast_mode: bool`
/// - `pane_split_h`, `pane_split_v`, `pane_split_h2`, `pane_split_v2`
/// - `pane_split_v3`..`pane_split_v6`
/// - `pane_divider_dragging: bool` (runtime only — exclude)
/// - `layout_favorites: Vec<String>`
/// - `timeframe_favorites: Vec<String>`
/// - `maximized_pane: Option<usize>`
/// - `pane_templates`, `portfolio_templates`, `dashboard_templates`,
///   `heatmap_templates`, `spreadsheet_templates`
/// - `dragging_tab: Option<TabDragState>` (runtime only — exclude)
/// - `active_workspace`, `pending_workspace_load`, `workspace_save_name`
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LayoutState {}

impl Persistable for LayoutState {
    const KEY: &'static str = "layout_state";
    const VERSION: u32 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_have_distinct_keys() {
        let keys = [
            UiSettings::KEY,
            TradingDefaults::KEY,
            AlertsState::KEY,
            ChatState::KEY,
            SidebarState::KEY,
            LayoutState::KEY,
        ];
        let mut sorted = keys.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len(), "aggregate KEYs must be unique");
    }

    #[test]
    fn aggregates_default_round_trip_through_persistence() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_aggregates_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ui.json");
        let v = UiSettings::default();
        save(&path, &v).unwrap();
        let loaded: UiSettings = load(&path).unwrap();
        assert_eq!(loaded.font_scale, v.font_scale);
        assert_eq!(loaded.font_idx, v.font_idx);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ui_settings_round_trips_through_persistable() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_ui_settings_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ui_settings.json");
        let v = UiSettings {
            font_scale: 2.0,
            font_idx: 3,
            compact_mode: true,
            pane_header_size: crate::chart_renderer::PaneHeaderSize::Expanded,
            toolbar_auto_hide: true,
            show_x_axis: false,
            show_y_axis: true,
            shared_x_axis: true,
            shared_y_axis: false,
            style_idx: 4,
        };
        save(&path, &v).unwrap();
        let loaded: UiSettings = load(&path).unwrap();
        assert_eq!(loaded.font_scale, 2.0);
        assert_eq!(loaded.font_idx, 3);
        assert!(loaded.compact_mode);
        assert!(loaded.toolbar_auto_hide);
        assert!(!loaded.show_x_axis);
        assert!(loaded.show_y_axis);
        assert!(loaded.shared_x_axis);
        assert!(!loaded.shared_y_axis);
        assert_eq!(loaded.style_idx, 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ui_settings_version_migration_from_v1_passes_through() {
        // VERSION == 1 currently. A future bump pairs with a migrate arm;
        // this test pins the contract that an envelope marked v1 round-trips
        // through `migrate` (default pass-through) without dropping fields.
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_ui_settings_v1_passthrough");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ui_settings.json");
        let envelope = serde_json::json!({
            "key": "ui_settings",
            "version": 1,
            "payload": {
                "font_scale": 1.8,
                "font_idx": 2,
                "compact_mode": false,
                "pane_header_size": "Normal",
                "toolbar_auto_hide": false,
                "show_x_axis": true,
                "show_y_axis": true,
                "shared_x_axis": false,
                "shared_y_axis": false,
                "style_idx": 1,
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: UiSettings = load(&path).expect("v1 envelope should load");
        assert_eq!(loaded.font_scale, 1.8);
        assert_eq!(loaded.font_idx, 2);
        assert_eq!(loaded.style_idx, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
