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
///
/// # Mirror invariant
/// Every field listed below **must** be present in both
/// `Watchlist::push_to_ui_settings` (Watchlist → aggregate) and
/// `Watchlist::pull_from_ui_settings` (aggregate → Watchlist).
///
/// Known gap: `has_seen_welcome` and `welcome_step_resume` are NOT
/// mirrored by `push_to_ui_settings` — they are written directly into
/// `self.ui_settings` by the welcome wizard and persisted from there.
/// `pull_from_ui_settings` does not copy them back to flat fields because
/// no legacy flat fields exist for them.  This is intentional.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// Whether the user has completed (or skipped) the first-launch welcome
    /// wizard. Defaults to `false` so new installs show the wizard. Set to
    /// `true` on Finish or Skip; reset to `false` via "Show welcome again".
    #[serde(default)]
    pub(crate) has_seen_welcome: bool,
    /// Step index (0..4) where a partially-completed wizard was abandoned.
    /// Restored when the wizard re-opens so the user continues where they left off.
    #[serde(default)]
    pub(crate) welcome_step_resume: u8,
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
            has_seen_welcome: false,
            welcome_step_resume: 0,
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
///
/// # Mirror invariant
/// Every field below **must** appear in both
/// `Watchlist::push_to_trading_defaults_store` (Watchlist → aggregate) and
/// `Watchlist::sync_trading_defaults_from_store` (aggregate → Watchlist).
///
/// Known gap: `daily_loss_cap` and `max_position_pct` have no legacy
/// counterpart on `Watchlist` — they are intentionally store-only fields
/// written by the welcome wizard.  Neither push nor sync touches them.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

    /// Maximum daily loss in USD (set via the welcome wizard, step 3).
    /// 0.0 means no cap set. Written by `WelcomeWizard` on Finish.
    #[serde(default)]
    pub daily_loss_cap: f32,

    /// Maximum position size as a percentage of account equity (0..=100).
    /// 0.0 means no cap set. Written by `WelcomeWizard` on Finish.
    #[serde(default)]
    pub max_position_pct: f32,
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
            daily_loss_cap: 0.0,
            max_position_pct: 0.0,
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
///
/// # Mirror invariant
/// Every field below **must** appear in both
/// `Watchlist::push_to_alerts_store` (Watchlist → aggregate) and
/// `Watchlist::sync_from_alerts_store` (aggregate → Watchlist).
/// All four fields are currently covered by both methods.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
///
/// # Mirror invariant
/// Every field below **must** appear in both
/// `Watchlist::push_to_chat_store` (Watchlist → aggregate) and
/// `Watchlist::sync_from_chat_store` (aggregate → Watchlist).
/// All nine fields are currently covered by both methods.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

// ─── SidebarState aggregate ──────────────────────────────────────────────────

/// Which sidebar / side-panel slots are open, plus their per-tab indices and
/// section-split fractions.
///
/// **P2 Round 2** populates this aggregate.
///
/// # Mirror invariant
/// Every field below **must** appear in both
/// `Watchlist::push_to_sidebar_store` (Watchlist → aggregate) and
/// `Watchlist::sync_from_sidebar_store` (aggregate → Watchlist).
/// All 33 fields are currently covered by both methods.
///
/// Field sources (Watchlist field → this aggregate field):
/// - `Watchlist::open: bool`                    → `watchlist_open`
/// - `Watchlist::settings_open: bool`           → `settings_open`
/// - `Watchlist::orders_panel_open: bool`       → `orders_panel_open`
/// - `Watchlist::order_entry_open: bool`        → `order_entry_open`
/// - `Watchlist::order_ledger_open: bool`       → `order_ledger_open`
/// - `Watchlist::order_ledger_view: u8`         → `order_ledger_view`
/// - `Watchlist::order_ledger_filter: u8`       → `order_ledger_filter`
/// - `Watchlist::order_health_open: bool`       → `order_health_open`
/// - `Watchlist::account_strip_open: bool`      → `account_strip_open`
/// - `Watchlist::object_tree_open: bool`        → `object_tree_open`
/// - `Watchlist::trendline_filter_open: bool`   → `trendline_filter_open`
/// - `Watchlist::apex_diag_open: bool`          → `apex_diag_open`
/// - `Watchlist::widget_gallery_open: bool`     → `widget_gallery_open`
/// - `Watchlist::filter_open: bool`             → `filter_open`
/// - `Watchlist::wl_columns_open: bool`         → `wl_columns_open`
/// - `Watchlist::tape_open: bool`               → `tape_open`
/// - `Watchlist::news_open: bool`               → `news_open`
/// - `Watchlist::journal_open: bool`            → `journal_open`
/// - `Watchlist::scanner_open: bool`            → `scanner_open`
/// - `Watchlist::scanner_builder_open: bool`    → `scanner_builder_open`
/// - `Watchlist::spread_open: bool`             → `spread_open`
/// - `Watchlist::script_open: bool`             → `script_open`
/// - `Watchlist::screenshot_open: bool`         → `screenshot_open`
/// - `Watchlist::rrg_open: bool`                → `rrg_open`
/// - `Watchlist::analysis_open: bool`           → `analysis_open`
/// - `Watchlist::signals_panel_open: bool`      → `signals_panel_open`
/// - `Watchlist::indicators_panel_open: bool`   → `indicators_panel_open`
/// - `Watchlist::indicators_section_fracs: [f32;3]` → `indicators_section_fracs`
/// - `Watchlist::feed_panel_open: bool`         → `feed_panel_open`
/// - `Watchlist::playbook_panel_open: bool`     → `playbook_panel_open`
/// - `Watchlist::journal_panel_open: bool`      → `journal_panel_open`
/// - `Watchlist::provenance_open: bool`         → `provenance_open`
/// - `Watchlist::replay_pane_open: bool`        → `replay_pane_open`
/// - `Watchlist::hotkey_editor_open: bool`      → `hotkey_editor_open`
///
/// Fields NOT included here (with reason):
/// - `cmd_palette_open`, `cmd_palette_query`, `cmd_palette_results`,
///   `cmd_palette_sel`, `cmd_palette_recent`, `cmd_palette_freq`,
///   `cmd_palette_ai_mode`, `cmd_palette_ai_input` — the palette is
///   ephemeral; state should not survive a restart.
/// - `hotkey_editing_id: Option<u32>` — transient UI scratch, not worth
///   persisting.
/// - `layout_dropdown_open`, `layout_dropdown_pos`,
///   `timeframe_dropdown_open`, `timeframe_dropdown_pos` — transient
///   dropdown gates; always closed on launch.
/// - `discord_open` — lives in `ChatState` to keep all Discord state
///   co-located; not duplicated here.
/// - `analysis_tab`, `signals_tab`, `feed_tab`, `signals_splits`,
///   `analysis_splits`, `feed_splits` — typed enums that reference
///   crate-internal types not currently `Serialize`; deferred to follow-up.
/// - `order_ledger_search: String` — transient search buffer, resets on open.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SidebarState {
    /// Whether the main watchlist side-panel is open.
    /// Source: `Watchlist::open`.
    #[serde(default)]
    pub watchlist_open: bool,

    /// Whether the Settings modal is open.
    /// Source: `Watchlist::settings_open`.
    #[serde(default)]
    pub settings_open: bool,

    // ── Orders / trading panels ────────────────────────────────────────────

    /// Orders panel (active orders list).
    /// Source: `Watchlist::orders_panel_open`.
    #[serde(default)]
    pub orders_panel_open: bool,

    /// Order entry ticket.
    /// Source: `Watchlist::order_entry_open`.
    #[serde(default)]
    pub order_entry_open: bool,

    /// Order ledger panel (order history).
    /// Source: `Watchlist::order_ledger_open`.
    #[serde(default)]
    pub order_ledger_open: bool,

    /// Active tab in the order ledger (0=Active, 1=Journal, 2=All).
    /// Source: `Watchlist::order_ledger_view: u8`.
    #[serde(default)]
    pub order_ledger_view: u8,

    /// Active filter in the order ledger.
    /// Source: `Watchlist::order_ledger_filter: u8`.
    #[serde(default)]
    pub order_ledger_filter: u8,

    /// Order system health panel (Ctrl+Shift+O).
    /// Source: `Watchlist::order_health_open`.
    #[serde(default)]
    pub order_health_open: bool,

    /// Active tab in the bottom dock (0=Orders, 1=Positions, 2=Account, 3=Notifications).
    /// Source: `Watchlist::bottom_dock_tab: u8`. (Footer *visibility* is a
    /// style default + session override, not persisted here — see `footer_visible()`.)
    #[serde(default)]
    pub bottom_dock_tab: u8,

    /// Persisted right-rail column width (px). Source: `Watchlist::rail_col_width`.
    #[serde(default = "SidebarState::default_rail_col_width")]
    pub rail_col_width: f32,

    /// Persisted bottom-dock (footer) height (px). Source: `Watchlist::bottom_dock_height`.
    #[serde(default = "SidebarState::default_bottom_dock_height")]
    pub bottom_dock_height: f32,

    // ── Chart-adjacent panels ──────────────────────────────────────────────

    /// Account summary bar below the toolbar.
    /// Source: `Watchlist::account_strip_open`.
    #[serde(default)]
    pub account_strip_open: bool,

    /// Object tree panel (drawings, indicators, overlays).
    /// Source: `Watchlist::object_tree_open`.
    #[serde(default)]
    pub object_tree_open: bool,

    /// Trendline filter dropdown.
    /// Source: `Watchlist::trendline_filter_open`.
    #[serde(default)]
    pub trendline_filter_open: bool,

    // ── Developer / debug panels ───────────────────────────────────────────

    /// Apex diagnostics panel.
    /// Source: `Watchlist::apex_diag_open`.
    #[serde(default)]
    pub apex_diag_open: bool,

    /// Widget gallery panel (Ctrl+Shift+G).
    /// Source: `Watchlist::widget_gallery_open`.
    #[serde(default)]
    pub widget_gallery_open: bool,

    // ── Watchlist panel internals ──────────────────────────────────────────

    /// Watchlist row filter dropdown.
    /// Source: `Watchlist::filter_open`.
    #[serde(default)]
    pub filter_open: bool,

    /// Column config popup.
    /// Source: `Watchlist::wl_columns_open`.
    #[serde(default)]
    pub wl_columns_open: bool,

    // ── Data-feed side-panels ──────────────────────────────────────────────

    /// Time & Sales tape panel.
    /// Source: `Watchlist::tape_open`.
    #[serde(default)]
    pub tape_open: bool,

    /// News feed panel.
    /// Source: `Watchlist::news_open`.
    #[serde(default)]
    pub news_open: bool,

    /// Trade journal floating panel (legacy; separate from `journal_panel_open`).
    /// Source: `Watchlist::journal_open`.
    #[serde(default)]
    pub journal_open: bool,

    // ── Tool panels ────────────────────────────────────────────────────────

    /// Scanner panel.
    /// Source: `Watchlist::scanner_open`.
    #[serde(default)]
    pub scanner_open: bool,

    /// Custom scanner builder panel.
    /// Source: `Watchlist::scanner_builder_open`.
    #[serde(default)]
    pub scanner_builder_open: bool,

    /// Spread builder panel.
    /// Source: `Watchlist::spread_open`.
    #[serde(default)]
    pub spread_open: bool,

    /// Scripting / backtesting panel.
    /// Source: `Watchlist::script_open`.
    #[serde(default)]
    pub script_open: bool,

    /// Screenshot library panel.
    /// Source: `Watchlist::screenshot_open`.
    #[serde(default)]
    pub screenshot_open: bool,

    /// Relative Rotation Graph panel.
    /// Source: `Watchlist::rrg_open`.
    #[serde(default)]
    pub rrg_open: bool,

    // ── Multi-section side panels ──────────────────────────────────────────

    /// Analysis sidebar.
    /// Source: `Watchlist::analysis_open`.
    #[serde(default)]
    pub analysis_open: bool,
    /// Source: `Watchlist::auto_chart_open`.
    #[serde(default)]
    pub auto_chart_open: bool,

    /// Signals sidebar.
    /// Source: `Watchlist::signals_panel_open`.
    #[serde(default)]
    pub signals_panel_open: bool,

    /// Indicators sidebar.
    /// Source: `Watchlist::indicators_panel_open`.
    #[serde(default)]
    pub indicators_panel_open: bool,

    /// Fractional heights for the three sections in the indicators panel
    /// (TOOLS / ACTIVE / LIBRARY). Sum is always 1.0.
    /// Source: `Watchlist::indicators_section_fracs: [f32; 3]`.
    #[serde(default = "SidebarState::default_indicators_section_fracs")]
    pub indicators_section_fracs: [f32; 3],

    /// Feed sidebar (News / Tape / Scanner tabs).
    /// Source: `Watchlist::feed_panel_open`.
    #[serde(default)]
    pub feed_panel_open: bool,

    /// Playbook sidebar.
    /// Source: `Watchlist::playbook_panel_open`.
    #[serde(default)]
    pub playbook_panel_open: bool,

    /// Trade Journal sidebar.
    /// Source: `Watchlist::journal_panel_open`.
    #[serde(default)]
    pub journal_panel_open: bool,

    // ── Misc panels ────────────────────────────────────────────────────────

    /// ProvenancePane (evidence DAG, right side panel).
    /// Source: `Watchlist::provenance_open`.
    #[serde(default)]
    pub provenance_open: bool,

    /// Historical replay scrubber panel.
    /// Source: `Watchlist::replay_pane_open`.
    #[serde(default)]
    pub replay_pane_open: bool,

    /// Hotkey editor dialog.
    /// Source: `Watchlist::hotkey_editor_open`.
    #[serde(default)]
    pub hotkey_editor_open: bool,
}

impl SidebarState {
    fn default_indicators_section_fracs() -> [f32; 3] { [0.18, 0.25, 0.57] }
    fn default_rail_col_width() -> f32 { 400.0 }
    fn default_bottom_dock_height() -> f32 { 240.0 }
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            watchlist_open: false,
            settings_open: false,
            orders_panel_open: false,
            order_entry_open: false,
            order_ledger_open: false,
            order_ledger_view: 0,
            order_ledger_filter: 0,
            order_health_open: false,
            bottom_dock_tab: 0,
            rail_col_width: 400.0,
            bottom_dock_height: 240.0,
            account_strip_open: false,
            object_tree_open: false,
            trendline_filter_open: false,
            apex_diag_open: false,
            widget_gallery_open: false,
            filter_open: false,
            wl_columns_open: false,
            tape_open: false,
            news_open: false,
            journal_open: false,
            scanner_open: false,
            scanner_builder_open: false,
            spread_open: false,
            script_open: false,
            screenshot_open: false,
            rrg_open: false,
            analysis_open: false,
            auto_chart_open: false,
            signals_panel_open: false,
            indicators_panel_open: false,
            indicators_section_fracs: Self::default_indicators_section_fracs(),
            feed_panel_open: false,
            playbook_panel_open: false,
            journal_panel_open: false,
            provenance_open: false,
            replay_pane_open: false,
            hotkey_editor_open: false,
        }
    }
}

impl Persistable for SidebarState {
    const KEY: &'static str = "sidebar_state";
    const VERSION: u32 = 1;
}

// ─── LayoutState aggregate ───────────────────────────────────────────────────

/// A single named, colored pane link group.
///
/// Mirrors `chart::renderer::gpu::LinkGroup`, which uses `egui::Color32`
/// (not serializable).  Here the color is stored as `[u8; 4]` (RGBA) so
/// the aggregate can be persisted without pulling in egui as a serialization
/// dependency.
///
/// Source: `src-tauri/src/chart/renderer/gpu.rs`, `LinkGroup`, line 4347.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PersistedLinkGroup {
    /// Human-readable label ("Group 1", etc.).
    pub name: String,
    /// RGBA color bytes — maps to/from `egui::Color32::to_array()`.
    pub color_rgba: [u8; 4],
}

impl Default for PersistedLinkGroup {
    fn default() -> Self {
        Self {
            name: String::new(),
            color_rgba: [70, 130, 255, 255],
        }
    }
}

/// Layout state — grid choice, pane split ratios, link groups, broadcast mode,
/// workspace management, and favorites.
///
/// **P2 Round 2** populates this aggregate.
///
/// Field sources (Watchlist field → this aggregate field):
/// - `Watchlist::link_groups: Vec<LinkGroup>`          → `link_groups` (via `PersistedLinkGroup`)
/// - `Watchlist::broadcast_mode: bool`                 → `broadcast_mode`
/// - `Watchlist::pane_split_h: f32`                    → `pane_split_h`
/// - `Watchlist::pane_split_v: f32`                    → `pane_split_v`
/// - `Watchlist::pane_split_h2: f32`                   → `pane_split_h2`
/// - `Watchlist::pane_split_v2: f32`                   → `pane_split_v2`
/// - `Watchlist::pane_split_v3: f32`                   → `pane_split_v3`
/// - `Watchlist::pane_split_v4: f32`                   → `pane_split_v4`
/// - `Watchlist::pane_split_v5: f32`                   → `pane_split_v5`
/// - `Watchlist::pane_split_v6: f32`                   → `pane_split_v6`
/// - `Watchlist::layout_favorites: Vec<String>`        → `layout_favorites`
/// - `Watchlist::timeframe_favorites: Vec<String>`     → `timeframe_favorites`
/// - `Watchlist::maximized_pane: Option<usize>`        → `maximized_pane`
/// - `Watchlist::pane_templates: Vec<(String, …)>`     → `pane_template_names` (names only)
/// - `Watchlist::portfolio_templates: Vec<String>`     → `portfolio_templates`
/// - `Watchlist::dashboard_templates: Vec<String>`     → `dashboard_templates`
/// - `Watchlist::heatmap_templates: Vec<String>`       → `heatmap_templates`
/// - `Watchlist::spreadsheet_templates: Vec<String>`   → `spreadsheet_templates`
/// - `Watchlist::active_workspace: String`             → `active_workspace`
/// - `Watchlist::workspace_save_name: String`          → `workspace_save_name`
///
/// Fields NOT included here (with reason):
/// - `pane_divider_dragging: bool` — runtime-only, always `false` on launch.
/// - `dragging_tab: Option<TabDragState>` — runtime drag state; `TabDragState`
///   holds `egui::Response` internals that are not serializable.
/// - `pending_workspace_load: Option<String>` — transient task queue signal;
///   not meaningful across restarts.
/// - Full `pane_templates` payloads — the `serde_json::Value` inner type
///   is intentionally opaque and large; names are persisted here so the
///   UI can enumerate them; the payloads remain in the chart save/load flow.
///
/// # Mirror invariant
/// Every field below **must** appear in both
/// `Watchlist::push_to_layout_store` (Watchlist → aggregate) and
/// `Watchlist::sync_from_layout_store` (aggregate → Watchlist).
/// All 20 fields are currently covered by both methods.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LayoutState {
    /// Named, colored link groups shared across all panes.
    /// Source: `Watchlist::link_groups: Vec<LinkGroup>`.
    #[serde(default = "LayoutState::default_link_groups")]
    pub link_groups: Vec<PersistedLinkGroup>,

    /// When true, toolbar actions (symbol change, timeframe, etc.) apply to
    /// all panes simultaneously.
    /// Source: `Watchlist::broadcast_mode: bool`.
    #[serde(default)]
    pub broadcast_mode: bool,

    // ── Pane split ratios ──────────────────────────────────────────────────
    // All default to 0.5 (equal split).

    /// Primary left/right (vertical-bar) split ratio.
    /// Source: `Watchlist::pane_split_h: f32`.
    #[serde(default = "LayoutState::default_split")]
    pub pane_split_h: f32,

    /// Primary top/bottom (horizontal-bar) split ratio.
    /// Source: `Watchlist::pane_split_v: f32`.
    #[serde(default = "LayoutState::default_split")]
    pub pane_split_v: f32,

    /// Secondary left/right split (3-column layouts).
    /// Source: `Watchlist::pane_split_h2: f32`.
    #[serde(default = "LayoutState::default_split")]
    pub pane_split_h2: f32,

    /// Secondary top/bottom split (3-row layouts).
    /// Source: `Watchlist::pane_split_v2: f32`.
    #[serde(default = "LayoutState::default_split")]
    pub pane_split_v2: f32,

    /// Extra horizontal divider 3 (FiveL / SixL / EightH layouts).
    /// Source: `Watchlist::pane_split_v3: f32`.
    #[serde(default = "LayoutState::default_split")]
    pub pane_split_v3: f32,

    /// Extra horizontal divider 4.
    /// Source: `Watchlist::pane_split_v4: f32`.
    #[serde(default = "LayoutState::default_split")]
    pub pane_split_v4: f32,

    /// Extra horizontal divider 5.
    /// Source: `Watchlist::pane_split_v5: f32`.
    #[serde(default = "LayoutState::default_split")]
    pub pane_split_v5: f32,

    /// Extra horizontal divider 6.
    /// Source: `Watchlist::pane_split_v6: f32`.
    #[serde(default = "LayoutState::default_split")]
    pub pane_split_v6: f32,

    /// Phase 1 PaneGrid topology — recursive split tree. `None` means
    /// "fall back to the legacy 8-fraction path above". When `Some` it is
    /// the canonical source of pane geometry; the fractions are kept
    /// alongside for older builds and emergency fallback.
    #[serde(default)]
    pub pane_layout: Option<crate::chart_renderer::pane_layout::PaneLayout>,

    // ── Favorites ─────────────────────────────────────────────────────────

    /// Layout preset names pinned to the toolbar (e.g. `["1", "2", "2H", "4"]`).
    /// Source: `Watchlist::layout_favorites: Vec<String>`.
    #[serde(default = "LayoutState::default_layout_favorites")]
    pub layout_favorites: Vec<String>,

    /// Timeframe strings pinned to the toolbar (e.g. `["1m", "5m", "1d"]`).
    /// Source: `Watchlist::timeframe_favorites: Vec<String>`.
    #[serde(default = "LayoutState::default_timeframe_favorites")]
    pub timeframe_favorites: Vec<String>,

    // ── Pane state ─────────────────────────────────────────────────────────

    /// Index of the pane currently shown fullscreen, or `None`.
    /// Source: `Watchlist::maximized_pane: Option<usize>`.
    #[serde(default)]
    pub maximized_pane: Option<usize>,

    // ── Templates ─────────────────────────────────────────────────────────

    /// Names of saved pane templates (indicator + toggle configs).
    /// The full payloads live in the chart save/load flow; only names
    /// are persisted here for enumeration.
    /// Source: `Watchlist::pane_templates: Vec<(String, serde_json::Value)>` (names only).
    #[serde(default)]
    pub pane_template_names: Vec<String>,

    /// Saved portfolio view templates.
    /// Source: `Watchlist::portfolio_templates: Vec<String>`.
    #[serde(default = "LayoutState::default_portfolio_templates")]
    pub portfolio_templates: Vec<String>,

    /// Saved dashboard templates.
    /// Source: `Watchlist::dashboard_templates: Vec<String>`.
    #[serde(default = "LayoutState::default_dashboard_templates")]
    pub dashboard_templates: Vec<String>,

    /// Saved heatmap templates.
    /// Source: `Watchlist::heatmap_templates: Vec<String>`.
    #[serde(default = "LayoutState::default_heatmap_templates")]
    pub heatmap_templates: Vec<String>,

    /// Saved spreadsheet templates.
    /// Source: `Watchlist::spreadsheet_templates: Vec<String>`.
    #[serde(default = "LayoutState::default_spreadsheet_templates")]
    pub spreadsheet_templates: Vec<String>,

    // ── Workspace ─────────────────────────────────────────────────────────

    /// Name of the currently active workspace.
    /// Source: `Watchlist::active_workspace: String`.
    #[serde(default = "LayoutState::default_workspace")]
    pub active_workspace: String,

    /// Buffer for the "save workspace as…" input field.
    /// Source: `Watchlist::workspace_save_name: String`.
    #[serde(default)]
    pub workspace_save_name: String,
}

impl LayoutState {
    fn default_split() -> f32 { 0.5 }
    fn default_workspace() -> String { "Default".into() }

    fn default_layout_favorites() -> Vec<String> {
        vec!["1".into(), "2".into(), "2H".into(), "3".into(), "4".into()]
    }
    fn default_timeframe_favorites() -> Vec<String> {
        vec![
            "1m".into(), "5m".into(), "15m".into(), "30m".into(),
            "1h".into(), "4h".into(), "1d".into(), "1wk".into(),
        ]
    }
    fn default_link_groups() -> Vec<PersistedLinkGroup> {
        vec![
            PersistedLinkGroup { name: "Group 1".into(), color_rgba: [70, 130, 255, 255] },
            PersistedLinkGroup { name: "Group 2".into(), color_rgba: [80, 200, 120, 255] },
            PersistedLinkGroup { name: "Group 3".into(), color_rgba: [255, 160, 60, 255] },
            PersistedLinkGroup { name: "Group 4".into(), color_rgba: [180, 100, 255, 255] },
        ]
    }
    fn default_portfolio_templates() -> Vec<String> { vec!["Default".into()] }
    fn default_dashboard_templates() -> Vec<String> { vec!["Default".into()] }
    fn default_heatmap_templates() -> Vec<String> { vec!["Default".into()] }
    fn default_spreadsheet_templates() -> Vec<String> { vec!["Default".into()] }
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            link_groups: Self::default_link_groups(),
            broadcast_mode: false,
            pane_split_h: 0.5,
            pane_split_v: 0.5,
            pane_split_h2: 0.5,
            pane_split_v2: 0.5,
            pane_split_v3: 0.5,
            pane_split_v4: 0.5,
            pane_split_v5: 0.5,
            pane_split_v6: 0.5,
            pane_layout: None,
            layout_favorites: Self::default_layout_favorites(),
            timeframe_favorites: Self::default_timeframe_favorites(),
            maximized_pane: None,
            pane_template_names: Vec::new(),
            portfolio_templates: Self::default_portfolio_templates(),
            dashboard_templates: Self::default_dashboard_templates(),
            heatmap_templates: Self::default_heatmap_templates(),
            spreadsheet_templates: Self::default_spreadsheet_templates(),
            active_workspace: Self::default_workspace(),
            workspace_save_name: String::new(),
        }
    }
}

impl Persistable for LayoutState {
    const KEY: &'static str = "layout_state";
    const VERSION: u32 = 1;
}

// ─── CmdPaletteState aggregate ───────────────────────────────────────────────

/// Frecency data for the command palette — recent items list and per-item
/// use-count.  Persisted so that power users retain their "SPY first" ordering
/// across restarts.
///
/// **P2 (command-palette-frecency)** introduces this aggregate.
///
/// Field sources (Watchlist field → this aggregate field):
/// - `Watchlist::cmd_palette_recent: Vec<String>` → `recent`
/// - `Watchlist::cmd_palette_freq: HashMap<String, u32>` → `freq`
///
/// Bounds enforced at runtime (NOT in serde):
/// - `recent`: last 50 entries, most-recent first, deduped.
/// - `freq`: top 200 by count; entries beyond 200 are pruned when saving.
///
/// Fields NOT included here:
/// - `cmd_palette_open`, `cmd_palette_query`, `cmd_palette_results`,
///   `cmd_palette_sel`, `cmd_palette_ai_mode`, `cmd_palette_ai_input`
///   — ephemeral UI state; always reset on open.
///
/// # Mirror invariant
/// Every field below **must** appear in any future
/// `Watchlist::push_to_cmd_palette_store` and
/// `Watchlist::sync_from_cmd_palette_store` methods (not yet landed).
/// Until those methods exist, writes go directly through
/// `Watchlist::cmd_palette_state_store.update(...)`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CmdPaletteState {
    /// Most-recently-used command/symbol IDs, newest first (max 50).
    /// Source: `Watchlist::cmd_palette_recent`.
    #[serde(default)]
    pub recent: Vec<String>,

    /// Use-count per command/symbol ID (max 200 entries by count).
    /// Source: `Watchlist::cmd_palette_freq`.
    #[serde(default)]
    pub freq: std::collections::HashMap<String, u32>,
}

impl Default for CmdPaletteState {
    fn default() -> Self {
        Self {
            recent: Vec::new(),
            freq: std::collections::HashMap::new(),
        }
    }
}

impl Persistable for CmdPaletteState {
    const KEY: &'static str = "cmd_palette_state";
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
            CmdPaletteState::KEY,
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
            has_seen_welcome: true,
            welcome_step_resume: 2,
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
        assert!(loaded.has_seen_welcome);
        assert_eq!(loaded.welcome_step_resume, 2);
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
            daily_loss_cap: 0.0,
            max_position_pct: 0.0,
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
        // New risk fields default to 0.0 when absent in persisted payload
        assert_eq!(loaded.daily_loss_cap, 0.0);
        assert_eq!(loaded.max_position_pct, 0.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ui_settings_welcome_fields_round_trip() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_ui_settings_welcome_rt");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ui_settings.json");
        let v = UiSettings { has_seen_welcome: true, welcome_step_resume: 3, ..UiSettings::default() };
        save(&path, &v).unwrap();
        let loaded: UiSettings = load(&path).unwrap();
        assert!(loaded.has_seen_welcome, "has_seen_welcome must survive round-trip");
        assert_eq!(loaded.welcome_step_resume, 3, "welcome_step_resume must survive round-trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ui_settings_old_json_without_welcome_fields_defaults_to_not_seen() {
        // Existing installs' saved JSON won't have these keys — they must
        // deserialize cleanly with has_seen_welcome=false (wizard runs again).
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_ui_settings_compat");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ui_settings.json");
        let envelope = serde_json::json!({
            "key": "ui_settings",
            "version": 1,
            "payload": {
                "font_scale": 1.6,
                "style_idx": 0,
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: UiSettings = load(&path).expect("compat payload must load");
        assert!(!loaded.has_seen_welcome, "old saves should default has_seen_welcome=false");
        assert_eq!(loaded.welcome_step_resume, 0, "old saves should default welcome_step_resume=0");
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

    // ── SidebarState tests ───────────────────────────────────────────────────

    #[test]
    fn sidebar_state_default_values_are_sane() {
        let s = SidebarState::default();
        assert!(!s.watchlist_open);
        assert!(!s.orders_panel_open);
        assert!(!s.indicators_panel_open);
        assert_eq!(s.indicators_section_fracs, [0.18, 0.25, 0.57]);
        assert_eq!(s.order_ledger_view, 0);
        assert_eq!(s.order_ledger_filter, 0);
    }

    #[test]
    fn sidebar_state_round_trips_through_persistable() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_sidebar_state_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sidebar_state.json");
        let mut v = SidebarState::default();
        v.watchlist_open = true;
        v.orders_panel_open = true;
        v.order_ledger_open = true;
        v.order_ledger_view = 2;
        v.indicators_panel_open = true;
        v.indicators_section_fracs = [0.20, 0.30, 0.50];
        v.signals_panel_open = true;
        save(&path, &v).unwrap();
        let loaded: SidebarState = load(&path).unwrap();
        assert!(loaded.watchlist_open);
        assert!(loaded.orders_panel_open);
        assert!(loaded.order_ledger_open);
        assert_eq!(loaded.order_ledger_view, 2);
        assert!(loaded.indicators_panel_open);
        assert_eq!(loaded.indicators_section_fracs, [0.20, 0.30, 0.50]);
        assert!(loaded.signals_panel_open);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidebar_state_missing_fields_fall_back_to_defaults() {
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_sidebar_state_partial");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sidebar_state.json");
        let envelope = serde_json::json!({
            "key": "sidebar_state",
            "version": 1,
            "payload": {
                "tape_open": true,
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: SidebarState = load(&path).expect("partial payload should load");
        assert!(loaded.tape_open);
        assert!(!loaded.watchlist_open);
        // fracs should fall back to the explicit default
        assert_eq!(loaded.indicators_section_fracs, [0.18, 0.25, 0.57]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── LayoutState tests ────────────────────────────────────────────────────

    #[test]
    fn layout_state_default_values_are_sane() {
        let s = LayoutState::default();
        assert_eq!(s.link_groups.len(), 4, "four default link groups");
        assert_eq!(s.link_groups[0].name, "Group 1");
        assert_eq!(s.pane_split_h, 0.5);
        assert_eq!(s.pane_split_v6, 0.5);
        assert!(!s.broadcast_mode);
        assert!(s.maximized_pane.is_none());
        assert_eq!(s.active_workspace, "Default");
        assert!(!s.layout_favorites.is_empty());
        assert!(!s.timeframe_favorites.is_empty());
    }

    #[test]
    fn layout_state_round_trips_through_persistable() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_layout_state_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("layout_state.json");
        let mut v = LayoutState::default();
        v.broadcast_mode = true;
        v.pane_split_h = 0.35;
        v.pane_split_v = 0.65;
        v.maximized_pane = Some(2);
        v.active_workspace = "MyWorkspace".into();
        v.layout_favorites = vec!["1".into(), "4".into()];
        v.link_groups[0].name = "Blue".into();
        save(&path, &v).unwrap();
        let loaded: LayoutState = load(&path).unwrap();
        assert!(loaded.broadcast_mode);
        assert_eq!(loaded.pane_split_h, 0.35);
        assert_eq!(loaded.pane_split_v, 0.65);
        assert_eq!(loaded.maximized_pane, Some(2));
        assert_eq!(loaded.active_workspace, "MyWorkspace");
        assert_eq!(loaded.layout_favorites, &["1", "4"]);
        assert_eq!(loaded.link_groups[0].name, "Blue");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_state_missing_fields_fall_back_to_defaults() {
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_layout_state_partial");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("layout_state.json");
        let envelope = serde_json::json!({
            "key": "layout_state",
            "version": 1,
            "payload": {
                "broadcast_mode": true,
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: LayoutState = load(&path).expect("partial payload should load");
        assert!(loaded.broadcast_mode);
        // All other fields should be defaults
        assert_eq!(loaded.pane_split_h, 0.5);
        assert_eq!(loaded.link_groups.len(), 4);
        assert_eq!(loaded.active_workspace, "Default");
        assert!(!loaded.layout_favorites.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn persisted_link_group_rgba_round_trips() {
        let group = PersistedLinkGroup {
            name: "Alpha".into(),
            color_rgba: [255, 80, 30, 200],
        };
        let json = serde_json::to_string(&group).unwrap();
        let back: PersistedLinkGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "Alpha");
        assert_eq!(back.color_rgba, [255, 80, 30, 200]);
    }

    // ── PartialEq round-trip equality assertions (whole-struct ==) ───────────

    #[test]
    fn ui_settings_partial_eq_round_trip() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_ui_settings_eq_rt");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ui_settings.json");
        let v = UiSettings {
            font_scale: 2.5,
            font_idx: 1,
            compact_mode: true,
            pane_header_size: crate::chart_renderer::PaneHeaderSize::Normal,
            toolbar_auto_hide: true,
            show_x_axis: false,
            show_y_axis: false,
            shared_x_axis: true,
            shared_y_axis: true,
            style_idx: 7,
            has_seen_welcome: true,
            welcome_step_resume: 4,
        };
        save(&path, &v).unwrap();
        let loaded: UiSettings = load(&path).unwrap();
        assert_eq!(loaded, v, "UiSettings must round-trip with full field equality");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ui_settings_default_stability() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_ui_settings_default_stability");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ui_settings.json");
        let v = UiSettings::default();
        save(&path, &v).unwrap();
        let loaded: UiSettings = load(&path).unwrap();
        assert_eq!(loaded, v, "UiSettings::default() must be stable under round-trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trading_defaults_partial_eq_round_trip() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_trading_defaults_eq_rt");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("trading_defaults.json");
        let v = TradingDefaults {
            default_stock_qty: 500,
            default_options_qty: 10,
            default_order_type: DefaultOrderType::StopLimit,
            default_tif: DefaultTimeInForce::Fok,
            default_outside_rth: true,
            daily_loss_cap: 1500.0,
            max_position_pct: 25.0,
        };
        save(&path, &v).unwrap();
        let loaded: TradingDefaults = load(&path).unwrap();
        assert_eq!(loaded, v, "TradingDefaults must round-trip with full field equality");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trading_defaults_default_stability() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_trading_defaults_default_stability");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("trading_defaults.json");
        let v = TradingDefaults::default();
        save(&path, &v).unwrap();
        let loaded: TradingDefaults = load(&path).unwrap();
        assert_eq!(loaded, v, "TradingDefaults::default() must be stable under round-trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn alerts_state_partial_eq_round_trip() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_alerts_state_eq_rt");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("alerts_state.json");
        let v = AlertsState {
            alerts: vec![
                PersistedAlert {
                    id: 42,
                    symbol: "TSLA".into(),
                    price: 300.50,
                    above: false,
                    triggered: true,
                    message: "TSLA below 300.50".into(),
                },
                PersistedAlert {
                    id: 43,
                    symbol: "NVDA".into(),
                    price: 900.0,
                    above: true,
                    triggered: false,
                    message: "NVDA breakout".into(),
                },
            ],
            next_alert_id: 44,
            alert_query: "breakout".into(),
            alerts_panel_open: true,
        };
        save(&path, &v).unwrap();
        let loaded: AlertsState = load(&path).unwrap();
        assert_eq!(loaded, v, "AlertsState must round-trip with full field equality");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn alerts_state_default_stability() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_alerts_state_default_stability");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("alerts_state.json");
        let v = AlertsState::default();
        save(&path, &v).unwrap();
        let loaded: AlertsState = load(&path).unwrap();
        assert_eq!(loaded, v, "AlertsState::default() must be stable under round-trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chat_state_partial_eq_round_trip() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_chat_state_eq_rt");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("chat_state.json");
        let v = ChatState {
            discord_open: true,
            discord_input: "gm everyone!".into(),
            discord_channel: "signals".into(),
            discord_authenticated: true,
            discord_username: "apex_user".into(),
            discord_user_id: "987654321".into(),
            discord_selected_guild: Some("guild_apex".into()),
            discord_selected_channel: Some("ch_signals".into()),
            discord_last_msg_id: Some("msg_12345".into()),
        };
        save(&path, &v).unwrap();
        let loaded: ChatState = load(&path).unwrap();
        assert_eq!(loaded, v, "ChatState must round-trip with full field equality");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chat_state_default_stability() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_chat_state_default_stability");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("chat_state.json");
        let v = ChatState::default();
        save(&path, &v).unwrap();
        let loaded: ChatState = load(&path).unwrap();
        assert_eq!(loaded, v, "ChatState::default() must be stable under round-trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidebar_state_partial_eq_round_trip() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_sidebar_state_eq_rt");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sidebar_state.json");
        // Set every bool field to a non-default value so any serde omission fails.
        let v = SidebarState {
            watchlist_open: true,
            settings_open: true,
            orders_panel_open: true,
            order_entry_open: true,
            order_ledger_open: true,
            order_ledger_view: 2,
            order_ledger_filter: 3,
            order_health_open: true,
            bottom_dock_tab: 2,
            rail_col_width: 420.0,
            bottom_dock_height: 260.0,
            account_strip_open: true,
            object_tree_open: true,
            trendline_filter_open: true,
            apex_diag_open: true,
            widget_gallery_open: true,
            filter_open: true,
            wl_columns_open: true,
            tape_open: true,
            news_open: true,
            journal_open: true,
            scanner_open: true,
            scanner_builder_open: true,
            spread_open: true,
            script_open: true,
            screenshot_open: true,
            rrg_open: true,
            analysis_open: true,
            signals_panel_open: true,
            indicators_panel_open: true,
            indicators_section_fracs: [0.30, 0.40, 0.30],
            feed_panel_open: true,
            playbook_panel_open: true,
            journal_panel_open: true,
            provenance_open: true,
            replay_pane_open: true,
            hotkey_editor_open: true,
            auto_chart_open: true,
        };
        save(&path, &v).unwrap();
        let loaded: SidebarState = load(&path).unwrap();
        assert_eq!(loaded, v, "SidebarState must round-trip with full field equality");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidebar_state_default_stability() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_sidebar_state_default_stability");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sidebar_state.json");
        let v = SidebarState::default();
        save(&path, &v).unwrap();
        let loaded: SidebarState = load(&path).unwrap();
        assert_eq!(loaded, v, "SidebarState::default() must be stable under round-trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_state_partial_eq_round_trip() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_layout_state_eq_rt");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("layout_state.json");
        let v = LayoutState {
            link_groups: vec![
                PersistedLinkGroup { name: "Cyan".into(), color_rgba: [0, 255, 255, 200] },
                PersistedLinkGroup { name: "Magenta".into(), color_rgba: [255, 0, 255, 128] },
            ],
            broadcast_mode: true,
            pane_split_h: 0.33,
            pane_split_v: 0.67,
            pane_split_h2: 0.25,
            pane_split_v2: 0.75,
            pane_split_v3: 0.40,
            pane_split_v4: 0.60,
            pane_split_v5: 0.45,
            pane_split_v6: 0.55,
            pane_layout: None,
            layout_favorites: vec!["2H".into(), "4".into()],
            timeframe_favorites: vec!["5m".into(), "1h".into()],
            maximized_pane: Some(1),
            pane_template_names: vec!["My Template".into()],
            portfolio_templates: vec!["Custom".into()],
            dashboard_templates: vec!["Trading".into()],
            heatmap_templates: vec!["Sector".into()],
            spreadsheet_templates: vec!["PnL".into()],
            active_workspace: "Scalping".into(),
            workspace_save_name: "Scalp Draft".into(),
        };
        save(&path, &v).unwrap();
        let loaded: LayoutState = load(&path).unwrap();
        assert_eq!(loaded, v, "LayoutState must round-trip with full field equality");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layout_state_default_stability() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_layout_state_default_stability");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("layout_state.json");
        let v = LayoutState::default();
        save(&path, &v).unwrap();
        let loaded: LayoutState = load(&path).unwrap();
        assert_eq!(loaded, v, "LayoutState::default() must be stable under round-trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── CmdPaletteState tests ────────────────────────────────────────────────

    #[test]
    fn cmd_palette_state_default_values_are_sane() {
        let s = CmdPaletteState::default();
        assert!(s.recent.is_empty(), "no recent entries by default");
        assert!(s.freq.is_empty(), "no freq entries by default");
    }

    #[test]
    fn cmd_palette_state_round_trips_through_persistable() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_cmd_palette_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("cmd_palette_state.json");
        let mut freq = std::collections::HashMap::new();
        freq.insert("SPY".to_string(), 42u32);
        freq.insert("AAPL".to_string(), 17u32);
        freq.insert("theme:dark".to_string(), 5u32);
        let v = CmdPaletteState {
            recent: vec!["SPY".into(), "AAPL".into(), "theme:dark".into()],
            freq,
        };
        save(&path, &v).unwrap();
        let loaded: CmdPaletteState = load(&path).unwrap();
        assert_eq!(loaded, v, "CmdPaletteState must round-trip with full field equality");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cmd_palette_state_default_stability() {
        use super::super::persistence::{load, save};
        let dir = std::env::temp_dir().join("apex_state_cmd_palette_default_stability");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("cmd_palette_state.json");
        let v = CmdPaletteState::default();
        save(&path, &v).unwrap();
        let loaded: CmdPaletteState = load(&path).unwrap();
        assert_eq!(loaded, v, "CmdPaletteState::default() must be stable under round-trip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cmd_palette_state_missing_fields_fall_back_to_defaults() {
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_cmd_palette_partial");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cmd_palette_state.json");
        // Minimal payload: only `recent` is present; `freq` must default to empty map.
        let envelope = serde_json::json!({
            "key": "cmd_palette_state",
            "version": 1,
            "payload": {
                "recent": ["QQQ", "IWM"],
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: CmdPaletteState = load(&path).expect("partial payload should load");
        assert_eq!(loaded.recent, &["QQQ", "IWM"]);
        assert!(loaded.freq.is_empty(), "absent freq field must default to empty map");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cmd_palette_state_empty_payload_uses_all_defaults() {
        use super::super::persistence::load;
        let dir = std::env::temp_dir().join("apex_state_cmd_palette_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cmd_palette_state.json");
        let envelope = serde_json::json!({
            "key": "cmd_palette_state",
            "version": 1,
            "payload": {}
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: CmdPaletteState = load(&path).expect("empty payload should load");
        assert_eq!(loaded, CmdPaletteState::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
