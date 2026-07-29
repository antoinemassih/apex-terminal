//! Watchlist sub-context state types (WS-E E3). The 24 cohesive sub-structs
//! extracted from the 258-field `Watchlist` god-struct across E3 slices 1-21,
//! moved out of the 10k-line `gpu.rs` god-file into this dedicated state module.
//! `super::` paths resolve to `chart::renderer` exactly as before (sibling of
//! `gpu`); only the gpu-local types below need importing. Re-exported from
//! `gpu` so `Watchlist`'s field types resolve unchanged.

use super::gpu::{OptionChain, StrikeMode, SplitSection, ScannerDef, ScanResult,
                 DiscordMessage, NewsItem, JournalEntry, SavedOption, TapeRow};

/// Play-editor form state (WS-E E3, Watchlist-split strangler slice 1).
/// Grouped out of the Watchlist god-struct: 19 transient input-buffer fields
/// for the play-entry form. Not persisted (ephemeral UI). `kind` was
/// `play_editor_type` (renamed — `type` is a reserved word). The explicit
/// `Default` mirrors the original per-field seeds exactly (qty=1, pct seeds).
pub(crate) struct PlayEditorState {
    pub(crate) open: bool,
    pub(crate) symbol: String,
    pub(crate) entry: String,
    pub(crate) target: String,
    pub(crate) stop: String,
    pub(crate) notes: String,
    pub(crate) direction: super::PlayDirection,
    pub(crate) kind: super::PlayType,
    pub(crate) qty: u32,
    pub(crate) qty_str: String,
    pub(crate) tags: Vec<String>,
    pub(crate) t2: String,
    pub(crate) t2_pct: String,
    pub(crate) t3: String,
    pub(crate) t3_pct: String,
    pub(crate) has_t2: bool,
    pub(crate) has_t3: bool,
    pub(crate) custom_tag: String,
    pub(crate) target_pct: String,
}

impl Default for PlayEditorState {
    fn default() -> Self {
        Self {
            open: false,
            symbol: String::new(),
            entry: String::new(),
            target: String::new(),
            stop: String::new(),
            notes: String::new(),
            direction: super::PlayDirection::Long,
            kind: super::PlayType::Directional,
            qty: 1,
            qty_str: "1".into(),
            tags: vec![],
            t2: String::new(),
            t2_pct: "25".into(),
            t3: String::new(),
            t3_pct: "25".into(),
            has_t2: false,
            has_t3: false,
            custom_tag: String::new(),
            target_pct: "100".into(),
        }
    }
}

/// Scripting/backtesting panel state (WS-E E3, Watchlist-split slice 2).
/// Grouped out of the Watchlist god-struct: 6 fields for the script editor +
/// backtest output. Not persisted. Explicit Default mirrors the seeds
/// (result_tab = Output).
pub(crate) struct ScriptState {
    pub(crate) open: bool,
    pub(crate) source: String,
    pub(crate) output: String,
    pub(crate) ai_prompt: String,
    pub(crate) result_tab: super::ui::panels::script_panel::ScriptResultTab,
    pub(crate) backtest: Option<super::ui::panels::script_panel::BacktestResult>,
}

impl Default for ScriptState {
    fn default() -> Self {
        Self {
            open: false,
            source: String::new(),
            output: String::new(),
            ai_prompt: String::new(),
            result_tab: super::ui::panels::script_panel::ScriptResultTab::Output,
            backtest: None,
        }
    }
}

/// Scanner + custom-scanner-builder state (WS-E E3, Watchlist-split slice 3).
/// Grouped out of the Watchlist god-struct: 12 scanner_* fields (results pool,
/// fetch state, builder form, movers tab). Explicit Default mirrors the seeds
/// (3 preset defs; min/max change -999/999). `open`/`builder_open` also mirror
/// the persisted SidebarState flags (which stay flat there).
pub(crate) struct ScannerState {
    pub(crate) open: bool,
    pub(crate) defs: Vec<ScannerDef>,
    pub(crate) results: Vec<ScanResult>,
    pub(crate) last_fetch: Option<std::time::Instant>,
    pub(crate) fetching: bool,
    pub(crate) new_name: String,
    pub(crate) new_min_change: f32,
    pub(crate) new_max_change: f32,
    pub(crate) new_min_volume: String,
    pub(crate) builder_open: bool,
    pub(crate) mover_tab: usize,
    pub(crate) filter_popup_open: bool,
}

impl Default for ScannerState {
    fn default() -> Self {
        Self {
            open: false,
            defs: vec![ScannerDef::preset_gainers(), ScannerDef::preset_losers(), ScannerDef::preset_most_active()],
            results: vec![],
            last_fetch: None,
            fetching: false,
            new_name: String::new(),
            new_min_change: -999.0,
            new_max_change: 999.0,
            new_min_volume: String::new(),
            builder_open: false,
            mover_tab: 0,
            filter_popup_open: false,
        }
    }
}

/// RRG (Relative Rotation Graph) panel state (WS-E E3, Watchlist-split slice 4).
/// Grouped out of the Watchlist god-struct: 5 rrg_* panel fields. Distinct from
/// the Theme's rrg_leading/improving/weakening/lagging QUADRANT COLORS (those
/// stay on Theme). `open` mirrors the persisted SidebarState flag (kept flat).
pub(crate) struct RrgState {
    pub(crate) open: bool,
    pub(crate) sectors: Vec<super::ui::panels::rrg_panel::RRGSector>,
    pub(crate) cycle_phase: String,
    pub(crate) time_offset: f32,
    pub(crate) tail_length: usize,
}

impl Default for RrgState {
    fn default() -> Self {
        Self {
            open: false,
            sectors: vec![],
            cycle_phase: String::new(),
            time_offset: 0.0,
            tail_length: 5,
        }
    }
}

/// Discord chat-panel state (WS-E E3, Watchlist-split slice 5). Grouped out of
/// the Watchlist god-struct: 17 discord_* fields. The 9 persisted subset
/// (open/input/channel/authenticated/username/user_id/selected_guild/
/// selected_channel/last_msg_id) mirror the ChatState store (kept flat there);
/// the rest are runtime cache. All field types impl Default → derive it.
#[derive(Default)]
pub(crate) struct DiscordState {
    pub(crate) open: bool,
    pub(crate) messages: Vec<DiscordMessage>,
    pub(crate) input: String,
    pub(crate) channel: String,
    pub(crate) authenticated: bool,
    pub(crate) username: String,
    pub(crate) user_id: String,
    pub(crate) guilds: Vec<crate::discord::DiscordGuild>,
    pub(crate) selected_guild: Option<String>,
    pub(crate) channels: Vec<crate::discord::DiscordChannel>,
    pub(crate) selected_channel: Option<String>,
    pub(crate) connecting: bool,
    pub(crate) guild_icons: std::collections::HashMap<String, egui::TextureHandle>,
    pub(crate) last_msg_id: Option<String>,
    pub(crate) poll_timer: Option<std::time::Instant>,
    pub(crate) channels_loading: bool,
    pub(crate) messages_loading: bool,
}

/// Command-palette state (WS-E E3, Watchlist-split slice 6). Grouped out of the
/// Watchlist god-struct: 8 cmd_palette_* fields. `recent`+`freq` are persisted
/// separately (cmd_palette_state.json via workspace_persist — field reads only).
/// Explicit Default (sel = -1 = no selection).
pub(crate) struct CmdPaletteState {
    pub(crate) open: bool,
    pub(crate) query: String,
    pub(crate) results: Vec<(String, String, String)>,
    pub(crate) sel: i32,
    pub(crate) recent: Vec<String>,
    pub(crate) freq: std::collections::HashMap<String, u32>,
    pub(crate) ai_mode: bool,
    pub(crate) ai_input: String,
}

impl Default for CmdPaletteState {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            results: vec![],
            sel: -1,
            recent: vec![],
            freq: std::collections::HashMap::new(),
            ai_mode: false,
            ai_input: String::new(),
        }
    }
}

/// Options-chain state (WS-E E3, Watchlist-split slice 7). Grouped out of the
/// Watchlist god-struct: 24 chain_* fields. SEMANTIC RENAME (the flat names
/// couldn't strip cleanly — `chain_0dte`/`chain_0_*` start with a digit): the
/// near/0-DTE chain is `near` + `near_*`; the far-dated chain is `far` + `far_*`;
/// shared/legacy fields keep their names. Not synced/persisted (runtime + fetch).
pub(crate) struct ChainState {
    pub(crate) symbol: String,
    pub(crate) sym_input: String,
    pub(crate) num_strikes: usize, // legacy fallback
    pub(crate) far_dte: i32,
    pub(crate) near: OptionChain,  // was chain_0dte
    pub(crate) far: OptionChain,
    /// True when `near` / `far` rows are locally-synthesized Black-Scholes
    /// placeholder data (real upstream unavailable).
    pub(crate) near_placeholder: bool, // was chain_0dte_placeholder
    pub(crate) far_placeholder: bool,
    pub(crate) select_mode: bool,
    pub(crate) loading: bool, // true while fetching chain from ApexIB
    pub(crate) underlying_price: f32, // real-time underlying price from IB chain response
    pub(crate) frozen: bool, // legacy fallback
    pub(crate) center_offset: i32, // legacy fallback
    // Per-chain independent controls
    pub(crate) near_num_strikes: usize, // was chain_0_num_strikes
    pub(crate) near_frozen: bool,
    pub(crate) near_offset: i32,
    pub(crate) near_strike_mode: StrikeMode,
    pub(crate) near_nmf: u8, // 0=near, 1=mid, 2=far
    pub(crate) far_num_strikes: usize,
    pub(crate) far_frozen: bool,
    pub(crate) far_offset: i32,
    pub(crate) far_strike_mode: StrikeMode,
    pub(crate) far_nmf: u8,
    pub(crate) last_fetch: Option<std::time::Instant>, // debounce chain refetches
}

impl Default for ChainState {
    fn default() -> Self {
        Self {
            symbol: "SPY".into(), sym_input: String::new(), num_strikes: 10, far_dte: 1,
            near: OptionChain::default(), far: OptionChain::default(),
            near_placeholder: false, far_placeholder: false,
            select_mode: false, loading: false, underlying_price: 0.0,
            frozen: false, center_offset: 0,
            near_num_strikes: 10, near_frozen: false, near_offset: 0, near_strike_mode: StrikeMode::Count, near_nmf: 0,
            far_num_strikes: 10, far_frozen: false, far_offset: 0, far_strike_mode: StrikeMode::Count, far_nmf: 0,
            last_fetch: None,
        }
    }
}

/// Order-ledger panel state (WS-E E3, Watchlist-split slice 8). Grouped out of
/// the Watchlist god-struct: 6 order_ledger_* fields. `open`/`view`/`filter`
/// mirror the persisted SidebarState flags (kept flat there). All Default.
#[derive(Default)]
pub(crate) struct OrderLedgerState {
    pub(crate) open: bool,
    pub(crate) view: u8,    // 0=Active, 1=Journal, 2=All
    pub(crate) filter: u8,  // index into LedgerFilter
    pub(crate) search: String,
    /// P2: per-symbol sub-section expanded state — keys are symbol strings. Ephemeral.
    pub(crate) sym_expanded: std::collections::HashMap<String, bool>,
    /// P2: pending bulk-cancel confirmation (Some(symbol) = inline confirm row). Ephemeral.
    pub(crate) pending_bulk_cancel: Option<String>,
}

/// Heatmap display config (WS-E E3, Watchlist-split slice 9). 4 heat_* fields.
/// Not synced/persisted. Explicit Default (index "Watchlist", cols 2).
pub(crate) struct HeatState {
    pub(crate) index: String,
    pub(crate) collapsed: std::collections::HashSet<String>,
    pub(crate) cols: u8,
    pub(crate) sort: i8,
}

impl Default for HeatState {
    fn default() -> Self {
        Self { index: "Watchlist".into(), collapsed: std::collections::HashSet::new(), cols: 2, sort: 0 }
    }
}

/// ProvenancePane state (WS-E E3, Watchlist-split slice 9). 2 provenance_*
/// fields. `open` mirrors the persisted SidebarState flag (kept flat there).
#[derive(Default)]
pub(crate) struct ProvenanceState {
    pub(crate) open: bool,
    pub(crate) active_lineage: Option<String>,
}

/// Analysis sidebar state (WS-E E3, Watchlist-split slice 9). 3 analysis_*
/// fields (auto_chart_open stays a separate Watchlist field). `open` mirrors
/// the persisted SidebarState flag. Explicit Default (tab = Rrg, one split).
pub(crate) struct AnalysisState {
    pub(crate) open: bool,
    pub(crate) tab: crate::chart_renderer::AnalysisTab,
    pub(crate) splits: Vec<SplitSection<crate::chart_renderer::AnalysisTab>>,
}

impl Default for AnalysisState {
    fn default() -> Self {
        Self {
            open: false,
            tab: crate::chart_renderer::AnalysisTab::Rrg,
            splits: vec![SplitSection::new(crate::chart_renderer::AnalysisTab::Rrg, 1.0)],
        }
    }
}

/// Feed sidebar-panel state (WS-E E3, Watchlist-split slice 10). 3 fields for
/// the Feed sidebar (News/Discord/Screenshots). Named `feed_panel` (not `feed`)
/// because Watchlist already has `feed: Vec<Play>` (the plays feed — different
/// feature). `open` mirrors the persisted SidebarState flag. Default: tab News.
pub(crate) struct FeedPanelState {
    pub(crate) open: bool,
    pub(crate) tab: crate::chart_renderer::FeedTab,
    pub(crate) splits: Vec<SplitSection<crate::chart_renderer::FeedTab>>,
}

impl Default for FeedPanelState {
    fn default() -> Self {
        Self {
            open: false,
            tab: crate::chart_renderer::FeedTab::News,
            splits: vec![SplitSection::new(crate::chart_renderer::FeedTab::News, 1.0)],
        }
    }
}

/// Signals sidebar-panel state (WS-E E3, Watchlist-split slice 11). 3 fields
/// (signals_panel_open/tab/splits). `open` mirrors both the persisted
/// SidebarState flag AND the workspace JSON key "signals_panel_open" (string
/// key unchanged). Explicit Default (tab Alerts).
pub(crate) struct SignalsPanelState {
    pub(crate) open: bool,
    pub(crate) tab: crate::chart_renderer::SignalsTab,
    pub(crate) splits: Vec<SplitSection<crate::chart_renderer::SignalsTab>>,
}

impl Default for SignalsPanelState {
    fn default() -> Self {
        Self {
            open: false,
            tab: crate::chart_renderer::SignalsTab::Alerts,
            splits: vec![SplitSection::new(crate::chart_renderer::SignalsTab::Alerts, 1.0)],
        }
    }
}

/// Timeframe favorites/dropdown state (WS-E E3, Watchlist-split slice 12). 3
/// timeframe_* fields. `favorites` mirrors the persisted SidebarState list
/// (kept flat there). Explicit Default (8 preset timeframes).
pub(crate) struct TimeframeState {
    pub(crate) favorites: Vec<String>,
    pub(crate) dropdown_open: bool,
    pub(crate) dropdown_pos: egui::Pos2,
}

impl Default for TimeframeState {
    fn default() -> Self {
        Self {
            favorites: vec!["1m".into(), "5m".into(), "15m".into(), "30m".into(), "1h".into(), "4h".into(), "1d".into(), "1wk".into()],
            dropdown_open: false,
            dropdown_pos: egui::Pos2::ZERO,
        }
    }
}

/// Indicators sidebar-panel state (WS-E E3, Watchlist-split slice 13). 4
/// indicators_* fields. `panel_open` + `section_fracs` mirror the persisted
/// SidebarState (kept flat there). Explicit Default (section_fracs 0.18/0.25/0.57).
pub(crate) struct IndicatorsState {
    pub(crate) panel_open: bool,
    pub(crate) panel_search: String,
    pub(crate) lib_collapsed: std::collections::HashSet<String>,
    pub(crate) section_fracs: [f32; 3],
}

impl Default for IndicatorsState {
    fn default() -> Self {
        Self {
            panel_open: false,
            panel_search: String::new(),
            lib_collapsed: std::collections::HashSet::new(),
            section_fracs: [0.18, 0.25, 0.57],
        }
    }
}

/// Trade-journal PANEL state (WS-E E3, Watchlist-split slice 14). 3 fields
/// (journal_panel_open/entries/page). Distinct from `journal_open` (the
/// Book-tab journal toggle, which stays a flat Watchlist field). `open` mirrors
/// the persisted SidebarState flag. Default seeds placeholder journal entries.
pub(crate) struct JournalPanelState {
    pub(crate) open: bool,
    pub(crate) entries: Vec<JournalEntry>,
    pub(crate) page: usize,
}

impl Default for JournalPanelState {
    fn default() -> Self {
        // No placeholder trades — real fills arrive via `journal_feed::start()`
        // and are swapped in by `journal_panel::draw()` each frame via
        // `journal_feed::take_fresh_entries()`. The panel renders the "No trades
        // logged" empty state until the first poller result arrives.
        Self { open: false, entries: Vec::new(), page: 0 }
    }
}

/// News-feed panel state (WS-E E3, Watchlist-split slice 15). 4 news_* fields.
/// `open` mirrors the persisted SidebarState flag. Explicit Default
/// (sentiment_filter -2 = All).
pub(crate) struct NewsState {
    pub(crate) open: bool,
    pub(crate) items: Vec<NewsItem>,
    pub(crate) filter_symbol: bool,
    pub(crate) sentiment_filter: i8,
}

impl Default for NewsState {
    fn default() -> Self {
        Self { open: false, items: vec![], filter_symbol: false, sentiment_filter: -2 }
    }
}

/// Heatmap-pane data (WS-E E3, Watchlist-split slice 16). cells + last_fetch,
/// cold-started from /api/stocks/grouped. `heatmap_templates` stays flat with
/// the other *_templates lists. Not synced/persisted. All Default.
#[derive(Default)]
pub(crate) struct HeatmapState {
    pub(crate) cells: Vec<(String, f32, f64)>,
    pub(crate) last_fetch: Option<std::time::Instant>,
}

/// Time & Sales (tape) panel state (WS-E E3, Watchlist-split slice 17). 2
/// tape_* fields. `open` mirrors the persisted SidebarState flag. All Default.
#[derive(Default)]
pub(crate) struct TapeState {
    pub(crate) open: bool,
    pub(crate) entries: Vec<TapeRow>,
}

/// Pane split-ratio + divider-drag state (WS-E E3, Watchlist-split slice 18).
/// 8 split ratios (persisted to workspace + global-settings JSON under the
/// UNCHANGED "pane_split_*" / "splits" keys) + the transient `dragging` flag
/// (was `pane_divider_dragging`; not persisted/synced). The persisted subset
/// also mirrors the flat pane_split_* fields on both `SidebarState` and the
/// `LoadedSettings` apply-step — those stay flat there.
pub(crate) struct PaneSplitState {
    pub(crate) h: f32,
    pub(crate) v: f32,
    pub(crate) h2: f32,
    pub(crate) v2: f32,
    pub(crate) v3: f32,
    pub(crate) v4: f32,
    pub(crate) v5: f32,
    pub(crate) v6: f32,
    pub(crate) dragging: bool,
}

impl Default for PaneSplitState {
    fn default() -> Self {
        Self { h: 0.5, v: 0.5, h2: 0.5, v2: 0.5, v3: 0.5, v4: 0.5, v5: 0.5, v6: 0.5, dragging: false }
    }
}

/// Workspace-management state (WS-E E3, Watchlist-split slice 19). 7 fields
/// (active / save-name / pending-load / nav-expanded / pending-new-blank /
/// rename target+buf). `active` + `save_name` mirror SidebarState (flat there);
/// `nav_expanded` persists to workspace JSON under the UNCHANGED "rail_expanded"
/// key. Mixed original prefixes -> semantic field names.
pub(crate) struct WorkspaceState {
    pub(crate) active: String,          // was active_workspace
    pub(crate) save_name: String,       // was workspace_save_name
    pub(crate) pending_load: Option<String>, // was pending_workspace_load
    pub(crate) nav_expanded: bool,      // was workspace_nav_expanded
    pub(crate) pending_new_blank: bool,
    pub(crate) rename_target: Option<String>, // was workspace_rename_target
    pub(crate) rename_buf: String,      // was workspace_rename_buf
    /// U0-6: a Delete requested from the context menu is held here until the
    /// user confirms the modal (deleting a saved workspace is hard to undo).
    pub(crate) pending_delete: Option<String>,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            active: "Default".into(),
            save_name: String::new(),
            pending_load: None,
            nav_expanded: false,
            pending_new_blank: false,
            rename_target: None,
            rename_buf: String::new(),
            pending_delete: None,
        }
    }
}

/// Watchlist symbol-search state (WS-E E3, Watchlist-split slice 20). 4 fields
/// (query/results/sel/refocus) for the add-symbol search box. Not synced/
/// persisted; single consumer (watchlist_panel). Explicit Default (sel -1 = none).
pub(crate) struct SearchState {
    pub(crate) query: String,
    pub(crate) results: Vec<(String, String)>,
    pub(crate) sel: i32,
    pub(crate) refocus: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self { query: String::new(), results: vec![], sel: -1, refocus: false }
    }
}

/// Saved-options watchlist state (WS-E E3, Watchlist-split slice 21). The saved
/// option contracts + their DTE filter. Not synced/persisted; single consumer
/// (watchlist_panel). Explicit Default (dte_filter -1 = all).
pub(crate) struct SavedOptionsState {
    pub(crate) list: Vec<SavedOption>,
    pub(crate) dte_filter: i32,
}

impl Default for SavedOptionsState {
    fn default() -> Self {
        Self { list: vec![], dte_filter: -1 }
    }
}
