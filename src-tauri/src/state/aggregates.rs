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

// ─── AlertsState aggregate ───────────────────────────────────────────────────

/// Serializable mirror of a price alert.
///
/// Mirrors `chart::renderer::trading::Alert` but adds `Serialize + Deserialize`
/// so the aggregate can be persisted.  The original `Alert` derives only
/// `Debug + Clone`; this copy is kept in sync manually until a follow-up
/// wave makes the trading module re-export a serializable version directly.
///
/// Source: `src-tauri/src/chart/renderer/trading/mod.rs`, line 440.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedAlert {
    /// Stable alert identifier (`Watchlist::next_alert_id` counter).
    pub id: u32,
    /// Ticker the alert watches (e.g. "AAPL").
    pub symbol: String,
    /// Price level that triggers the alert.
    pub price: f32,
    /// `true` = fire when price crosses *above*, `false` = *below*.
    pub above: bool,
    /// `true` once the alert has fired at least once this session.
    pub triggered: bool,
    /// Human-readable note shown in the alerts panel.
    pub message: String,
}

/// Alert configurations and list.
///
/// **P2 Round 2** populates this aggregate.  Fields are sourced from
/// `Watchlist` (the legacy god-object).  Call sites are **not** migrated in
/// this PR; the struct is landed so follow-up waves can migrate one field at
/// a time.
///
/// Field sources (Watchlist field → this aggregate field):
/// - `Watchlist::alerts: Vec<Alert>`         → `alerts` (via `PersistedAlert`)
/// - `Watchlist::next_alert_id: u32`         → `next_alert_id`
/// - `Watchlist::alert_query: String`        → `alert_query`
/// - `Watchlist::alerts_panel_open: bool`    → `alerts_panel_open`
///   (UI-only; `SidebarState` is the eventual home, but the field is
///   small enough to co-locate here until a dedicated sidebar sweep lands.)
///
/// Fields NOT included here:
/// - No additional `alerts_*` fields were found in gpu.rs beyond the four
///   listed above.  `alert_query` has `#[allow(dead_code)]` in gpu.rs,
///   suggesting it is stub / future use — included here for completeness.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AlertsState {
    /// The live alert list.
    /// Source: `Watchlist::alerts: Vec<Alert>`.
    #[serde(default)]
    pub alerts: Vec<PersistedAlert>,

    /// Monotonically-increasing counter used to assign stable alert IDs.
    /// Source: `Watchlist::next_alert_id: u32` (default 1).
    #[serde(default = "AlertsState::default_next_alert_id")]
    pub next_alert_id: u32,

    /// Search / filter string in the alerts panel input box.
    /// Source: `Watchlist::alert_query: String` (`#[allow(dead_code)]` in gpu.rs).
    #[serde(default)]
    pub alert_query: String,

    /// Whether the alerts side-panel is currently visible.
    /// Source: `Watchlist::alerts_panel_open: bool`.
    #[serde(default)]
    pub alerts_panel_open: bool,
}

impl AlertsState {
    fn default_next_alert_id() -> u32 { 1 }
}

impl Default for AlertsState {
    fn default() -> Self {
        Self {
            alerts: Vec::new(),
            next_alert_id: Self::default_next_alert_id(),
            alert_query: String::new(),
            alerts_panel_open: false,
        }
    }
}

impl Persistable for AlertsState {
    const KEY: &'static str = "alerts_state";
    const VERSION: u32 = 1;
}

// ─── ChatState aggregate ─────────────────────────────────────────────────────

/// Discord / signals chat panel state — selected guild/channel, cached
/// messages, input buffer, connection flags.
///
/// **P2 Round 2** populates this aggregate.
///
/// Field sources (Watchlist field → this aggregate field):
/// - `Watchlist::discord_open: bool`                      → `discord_open`
/// - `Watchlist::discord_input: String`                   → `discord_input`
/// - `Watchlist::discord_channel: String`                 → `discord_channel`
/// - `Watchlist::discord_authenticated: bool`             → `discord_authenticated`
/// - `Watchlist::discord_username: String`                → `discord_username`
/// - `Watchlist::discord_user_id: String`                 → `discord_user_id`
/// - `Watchlist::discord_selected_guild: Option<String>`  → `discord_selected_guild`
/// - `Watchlist::discord_selected_channel: Option<String>`→ `discord_selected_channel`
/// - `Watchlist::discord_last_msg_id: Option<String>`     → `discord_last_msg_id`
///
/// Fields NOT included here (with reason):
/// - `discord_messages: Vec<DiscordMessage>` — runtime cache; re-fetched on
///   open.  `DiscordMessage` is not `Serialize`; persisting would risk
///   stale messages on next launch.
/// - `discord_guilds: Vec<DiscordGuild>` / `discord_channels: Vec<DiscordChannel>`
///   — fetched fresh each session from the Discord API; not worth persisting.
/// - `discord_guild_icons: HashMap<String, TextureHandle>` — runtime GPU
///   textures, not serializable.
/// - `discord_poll_timer: Option<Instant>` — runtime timer, not serializable.
/// - `discord_connecting: bool` — transient connection state; always starts
///   `false` on launch.
/// - `discord_channels_loading: bool`, `discord_messages_loading: bool`
///   — in-flight flags; migrate to `InFlightRegistry` in a follow-up wave.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatState {
    /// Whether the Discord chat side-panel is open.
    /// Source: `Watchlist::discord_open: bool`.
    #[serde(default)]
    pub discord_open: bool,

    /// Text currently typed in the message input box (drafts across restarts).
    /// Source: `Watchlist::discord_input: String`.
    #[serde(default)]
    pub discord_input: String,

    /// Display name of the currently selected channel.
    /// Source: `Watchlist::discord_channel: String`.
    #[serde(default)]
    pub discord_channel: String,

    /// Whether the user has completed Discord OAuth and is logged in.
    /// Source: `Watchlist::discord_authenticated: bool`.
    /// NOTE: auth tokens live in the system keychain, not here.
    #[serde(default)]
    pub discord_authenticated: bool,

    /// Discord display name of the logged-in user.
    /// Source: `Watchlist::discord_username: String`.
    #[serde(default)]
    pub discord_username: String,

    /// Discord snowflake ID of the logged-in user.
    /// Source: `Watchlist::discord_user_id: String`.
    #[serde(default)]
    pub discord_user_id: String,

    /// Snowflake ID of the guild (server) the user had selected.
    /// Source: `Watchlist::discord_selected_guild: Option<String>`.
    #[serde(default)]
    pub discord_selected_guild: Option<String>,

    /// Snowflake ID of the channel the user had selected within the guild.
    /// Source: `Watchlist::discord_selected_channel: Option<String>`.
    #[serde(default)]
    pub discord_selected_channel: Option<String>,

    /// Snowflake ID of the last message received, used for incremental polling.
    /// Source: `Watchlist::discord_last_msg_id: Option<String>`.
    #[serde(default)]
    pub discord_last_msg_id: Option<String>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            discord_open: false,
            discord_input: String::new(),
            discord_channel: String::new(),
            discord_authenticated: false,
            discord_username: String::new(),
            discord_user_id: String::new(),
            discord_selected_guild: None,
            discord_selected_channel: None,
            discord_last_msg_id: None,
        }
    }
}

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

    // ── AlertsState tests ────────────────────────────────────────────────────

    #[test]
    fn alerts_state_default_values_are_sane() {
        let s = AlertsState::default();
        assert!(s.alerts.is_empty(), "no alerts by default");
        assert_eq!(s.next_alert_id, 1, "IDs start at 1");
        assert!(s.alert_query.is_empty());
        assert!(!s.alerts_panel_open);
    }

    #[test]
    fn alerts_state_round_trips_through_persistable() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_alerts_state_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("alerts_state.json");
        let v = AlertsState {
            alerts: vec![
                PersistedAlert {
                    id: 1,
                    symbol: "AAPL".into(),
                    price: 150.0,
                    above: true,
                    triggered: false,
                    message: "AAPL breakout".into(),
                },
            ],
            next_alert_id: 2,
            alert_query: "SPY".into(),
            alerts_panel_open: true,
        };
        save(&path, &v).unwrap();
        let loaded: AlertsState = load(&path).unwrap();
        assert_eq!(loaded.alerts.len(), 1);
        assert_eq!(loaded.alerts[0].symbol, "AAPL");
        assert_eq!(loaded.alerts[0].price, 150.0);
        assert!(loaded.alerts[0].above);
        assert!(!loaded.alerts[0].triggered);
        assert_eq!(loaded.next_alert_id, 2);
        assert_eq!(loaded.alert_query, "SPY");
        assert!(loaded.alerts_panel_open);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn alerts_state_missing_fields_fall_back_to_defaults() {
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_alerts_state_partial");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("alerts_state.json");
        let envelope = serde_json::json!({
            "key": "alerts_state",
            "version": 1,
            "payload": {
                "next_alert_id": 5,
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: AlertsState = load(&path).expect("partial payload should load");
        assert_eq!(loaded.next_alert_id, 5);
        assert!(loaded.alerts.is_empty());
        assert!(loaded.alert_query.is_empty());
        assert!(!loaded.alerts_panel_open);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── ChatState tests ──────────────────────────────────────────────────────

    #[test]
    fn chat_state_default_values_are_sane() {
        let s = ChatState::default();
        assert!(!s.discord_open);
        assert!(s.discord_input.is_empty());
        assert!(!s.discord_authenticated);
        assert!(s.discord_selected_guild.is_none());
        assert!(s.discord_selected_channel.is_none());
        assert!(s.discord_last_msg_id.is_none());
    }

    #[test]
    fn chat_state_round_trips_through_persistable() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_chat_state_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("chat_state.json");
        let v = ChatState {
            discord_open: true,
            discord_input: "hello".into(),
            discord_channel: "general".into(),
            discord_authenticated: true,
            discord_username: "traderbro".into(),
            discord_user_id: "123456789".into(),
            discord_selected_guild: Some("guild_abc".into()),
            discord_selected_channel: Some("ch_xyz".into()),
            discord_last_msg_id: Some("msg_999".into()),
        };
        save(&path, &v).unwrap();
        let loaded: ChatState = load(&path).unwrap();
        assert!(loaded.discord_open);
        assert_eq!(loaded.discord_input, "hello");
        assert_eq!(loaded.discord_channel, "general");
        assert!(loaded.discord_authenticated);
        assert_eq!(loaded.discord_username, "traderbro");
        assert_eq!(loaded.discord_user_id, "123456789");
        assert_eq!(loaded.discord_selected_guild.as_deref(), Some("guild_abc"));
        assert_eq!(loaded.discord_selected_channel.as_deref(), Some("ch_xyz"));
        assert_eq!(loaded.discord_last_msg_id.as_deref(), Some("msg_999"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chat_state_missing_fields_fall_back_to_defaults() {
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_chat_state_partial");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("chat_state.json");
        let envelope = serde_json::json!({
            "key": "chat_state",
            "version": 1,
            "payload": {
                "discord_authenticated": true,
                "discord_username": "alice",
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: ChatState = load(&path).expect("partial payload should load");
        assert!(loaded.discord_authenticated);
        assert_eq!(loaded.discord_username, "alice");
        assert!(!loaded.discord_open);
        assert!(loaded.discord_selected_guild.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
