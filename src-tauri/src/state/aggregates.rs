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

// ─── TradingDefaults sub-types ──────────────────────────────────────────────

/// Default order type for new orders.
///
/// Maps to `Watchlist::default_order_type: usize` (0=MKT, 1=LMT, 2=STP).
/// Typed enum replaces the raw index so downstream code gets exhaustive
/// matching and no out-of-range panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultOrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

impl Default for DefaultOrderType {
    fn default() -> Self { Self::Market }
}

/// Default time-in-force for new orders.
///
/// Maps to `Watchlist::default_tif: usize` (0=DAY, 1=GTC, 2=IOC).
/// Typed enum replaces the raw index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultTimeInForce {
    Day,
    Gtc,
    Ioc,
    Fok,
}

impl Default for DefaultTimeInForce {
    fn default() -> Self { Self::Day }
}

// ─── TradingDefaults aggregate ───────────────────────────────────────────────

/// Trading defaults — order qty, order type, TIF, and bracket distances.
///
/// **P2 Round 1** populates this aggregate. Fields are sourced from the
/// scattered ad-hoc storage on `Watchlist` (the legacy god-object). Call
/// sites are **not** migrated in this PR; the struct is landed first so
/// follow-up waves can migrate one field at a time without arguing about
/// destination or versioning.
///
/// Field sources (Watchlist field → this aggregate field):
/// - `Watchlist::default_stock_qty: u32`   → `default_stock_qty`
/// - `Watchlist::default_options_qty: u32` → `default_options_qty`
/// - `Watchlist::default_order_type: usize`→ `default_order_type` (typed enum)
/// - `Watchlist::default_tif: usize`       → `default_tif` (typed enum)
/// - `Watchlist::default_outside_rth: bool`→ `default_outside_rth`
///
/// Bracket TP/SL distances — optional percentage offsets for bracket orders.
/// The per-pane `BracketTemplate` list lives on `ChartState`; these are the
/// *default* distances pre-filled when the user opens a new bracket ticket.
/// No existing Watchlist field directly stores these scalars (they are folded
/// into the per-pane template list); these fields are stubbed with TODO until
/// a follow-up wave decides on the canonical representation.
///
/// Fields NOT included here:
/// - `hotkeys: Vec<HotKey>` — `HotKey` embeds `egui::Key` which is not
///   `Serialize/Deserialize`; hotkeys need their own serialization shim.
/// - `hotkey_editor_open`, `hotkey_editing_id` — UI-only scratch state;
///   belongs in `SidebarState`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradingDefaults {
    /// Default quantity for equity/ETF orders (shares).
    /// Source: `Watchlist::default_stock_qty` (u32, default 100).
    #[serde(default = "TradingDefaults::default_stock_qty")]
    pub default_stock_qty: u32,

    /// Default quantity for options orders (contracts).
    /// Source: `Watchlist::default_options_qty` (u32, default 1).
    #[serde(default = "TradingDefaults::default_options_qty")]
    pub default_options_qty: u32,

    /// Default order type (Market / Limit / Stop / StopLimit).
    /// Source: `Watchlist::default_order_type` (usize index, default 0 = MKT).
    #[serde(default)]
    pub default_order_type: DefaultOrderType,

    /// Default time-in-force (Day / GTC / IOC / FOK).
    /// Source: `Watchlist::default_tif` (usize index, default 0 = DAY).
    #[serde(default)]
    pub default_tif: DefaultTimeInForce,

    /// Whether to allow trading outside regular trading hours by default.
    /// Source: `Watchlist::default_outside_rth` (bool, default false).
    #[serde(default)]
    pub default_outside_rth: bool,

    // TODO(P2-follow-up): migrate global bracket TP/SL default distances from
    // the per-pane BracketTemplate list. The per-pane templates live on
    // ChartState (new_bracket_target / new_bracket_stop strings + bracket_templates
    // Vec<BracketTemplate>). Decide whether the global default is a percentage
    // offset (f32) or an absolute Price(i64) distance before landing the field.
    // Placeholder fields:
    //   pub default_bracket_tp_pct: Option<f32>,
    //   pub default_bracket_sl_pct: Option<f32>,
}

impl TradingDefaults {
    fn default_stock_qty() -> u32 { 100 }
    fn default_options_qty() -> u32 { 1 }
}

impl Default for TradingDefaults {
    fn default() -> Self {
        Self {
            default_stock_qty: Self::default_stock_qty(),
            default_options_qty: Self::default_options_qty(),
            default_order_type: DefaultOrderType::default(),
            default_tif: DefaultTimeInForce::default(),
            default_outside_rth: false,
        }
    }
}

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
    fn trading_defaults_default_values_are_sane() {
        let d = TradingDefaults::default();
        assert_eq!(d.default_stock_qty, 100, "stock qty default should be 100");
        assert_eq!(d.default_options_qty, 1, "options qty default should be 1 contract");
        assert_eq!(d.default_order_type, DefaultOrderType::Market);
        assert_eq!(d.default_tif, DefaultTimeInForce::Day);
        assert!(!d.default_outside_rth, "outside RTH should be off by default");
    }

    #[test]
    fn trading_defaults_round_trips_through_persistable() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_trading_defaults_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("trading_defaults.json");
        let v = TradingDefaults {
            default_stock_qty: 200,
            default_options_qty: 5,
            default_order_type: DefaultOrderType::Limit,
            default_tif: DefaultTimeInForce::Gtc,
            default_outside_rth: true,
        };
        save(&path, &v).unwrap();
        let loaded: TradingDefaults = load(&path).unwrap();
        assert_eq!(loaded.default_stock_qty, 200);
        assert_eq!(loaded.default_options_qty, 5);
        assert_eq!(loaded.default_order_type, DefaultOrderType::Limit);
        assert_eq!(loaded.default_tif, DefaultTimeInForce::Gtc);
        assert!(loaded.default_outside_rth);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trading_defaults_v1_envelope_round_trips() {
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_trading_defaults_v1_envelope");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trading_defaults.json");
        let envelope = serde_json::json!({
            "key": "trading_defaults",
            "version": 1,
            "payload": {
                "default_stock_qty": 50,
                "default_options_qty": 2,
                "default_order_type": "stop",
                "default_tif": "gtc",
                "default_outside_rth": false,
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: TradingDefaults = load(&path).expect("v1 envelope should load");
        assert_eq!(loaded.default_stock_qty, 50);
        assert_eq!(loaded.default_options_qty, 2);
        assert_eq!(loaded.default_order_type, DefaultOrderType::Stop);
        assert_eq!(loaded.default_tif, DefaultTimeInForce::Gtc);
        assert!(!loaded.default_outside_rth);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trading_defaults_missing_fields_fall_back_to_defaults() {
        // A persisted payload with only some fields set must still deserialize,
        // using serde(default) for absent fields.
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_trading_defaults_partial");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trading_defaults.json");
        let envelope = serde_json::json!({
            "key": "trading_defaults",
            "version": 1,
            "payload": {
                "default_stock_qty": 300,
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: TradingDefaults = load(&path).expect("partial payload should load");
        assert_eq!(loaded.default_stock_qty, 300);
        // Fields not present should fall back to their serde(default)
        assert_eq!(loaded.default_options_qty, 1);
        assert_eq!(loaded.default_order_type, DefaultOrderType::Market);
        assert_eq!(loaded.default_tif, DefaultTimeInForce::Day);
        assert!(!loaded.default_outside_rth);
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
