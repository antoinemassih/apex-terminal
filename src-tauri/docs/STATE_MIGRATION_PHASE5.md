# Phase 5 — Canonical Model Migration Plan
# `Chart` → `ChartState`

**Status:** Active planning document  
**Companion:** `docs/STATE_ROADMAP.md` §4 Phase 5, `docs/adr/0001-canonical-state-model.md`  
**Constraint:** `core.rs` is sacred — no mechanical sweeps, ever.

---

## 1. Overview

`ChartState` (`chart/state/mod.rs`) is the declared target model for all
per-chart state. `Chart` (`chart/renderer/gpu.rs`, ~200 fields) is the live
god-object. Phase 5 migrates Chart → ChartState incrementally, one field group
at a time, using a read-through shim so the app stays compilable and runnable
after every step.

This document is the executable playbook: field inventory, equivalence table,
migration order, shim contract, per-step verification, and risk flags.

---

## 2. Field-Group Inventory

### Group A — Symbol/Timeframe Identity (LOW RISK)

| Chart field | ChartState equivalent | Notes |
|---|---|---|
| `symbol: String` | `symbol.canonical: ArcStr` | `String` → `ArcStr` at codec boundary |
| `timeframe: String` | `timeframe: Timeframe` | String → typed enum |
| `symbol_meta: Symbol` | `symbol.asset_class`, `symbol.provider_hints` | Fold into ChartState::Symbol |
| `is_option: bool` | — | No equivalent yet; add to ChartState or ExtensionBag |
| `underlying: String` | — | Option-specific; ExtensionBag candidate |
| `option_type: String` | — | ExtensionBag |
| `option_strike: f32` | — | ExtensionBag |
| `option_expiry: String` | — | ExtensionBag |
| `option_con_id: i64` | — | ExtensionBag |
| `option_contract: String` | — | ExtensionBag |
| `bar_source_mark: bool` | — | ExtensionBag candidate |
| `pane_type: PaneType` | — | Workspace-level; not a chart property |
| `theme_idx: usize` | `theme: ThemeOverride` | Typed enum; index is renderer detail |

**ChartState coverage:** Partial. Symbol + Timeframe have typed homes; option
metadata and pane type do not.

---

### Group B — Viewport / Scroll (MEDIUM RISK — hot paint path reads)

| Chart field | ChartState equivalent |
|---|---|
| `vs: f32` (scroll bar offset) | `viewport.from_ts_ns` (after conversion) |
| `vc: u32` (visible bar count) | derived from `viewport.from_ts_ns` / `viewport.to_ts_ns` |
| `price_lock: Option<(f32,f32)>` | `viewport.price_low` / `viewport.price_high` |
| `log_scale: bool` | `viewport.log_scale` |
| `auto_scroll: bool` | — |
| `drag_zoom_active: bool` | — |
| `drag_zoom_start: Option<Pos2>` | — |
| `draw_price_freeze: Option<(f32,f32)>` | — |
| `axis_drag_mode: u8` | — |
| `zoom_selecting: bool` | — |
| `zoom_start: Pos2` | — |
| `vc_target: u32` | — |
| `price_range_animated: Option<(f32,f32)>` | — |

**ChartState coverage:** The four persisted viewport scalars (`from_ts_ns`,
`to_ts_ns`, `price_low`, `price_high`, `log_scale`) have a clean home. The
transient interaction state (drag booleans, animation targets) is UI-only and
belongs on a separate `ViewportInteraction` struct, not persisted.

**Risk:** `vs` and `vc` are read every frame in `core.rs` for bar → pixel
projection. Migrating these changes the hot path. **Flag: Needs supervised
execution with live frame-rate verification.**

---

### Group C — Drawings (MEDIUM RISK)

| Chart field | ChartState equivalent |
|---|---|
| `drawings: Vec<Drawing>` (legacy) | `drawings: SlotMap<DrawingId, Drawing>` |
| `selected_id: Option<String>` | — (UI ephemera) |
| `selected_ids: Vec<String>` | — |
| `dragging_drawing: Option<(String, i32)>` | — |
| `drag_start_price: f32` | — |
| `drag_start_bar: f32` | — |
| `groups: Vec<DrawingGroup>` | — (group model not in ChartState yet) |
| `hidden_groups: Vec<String>` | — |
| `hide_all_drawings: bool` | — |
| `undo_stack: Vec<DrawingAction>` | — |
| `redo_stack: Vec<DrawingAction>` | — |
| `drag_drawing_snapshot: Option<Drawing>` | — |
| `text_edit_id: Option<String>` | — |
| `text_edit_buf: String` | — |
| `draw_tool: String` | — |
| `draw_picker_open: bool` | — |
| `draw_picker_pos: Pos2` | — |
| `draw_picker_hover_cat: Option<String>` | — |
| `draw_picker_hover_cat_y: f32` | — |
| `pending_pt: Option<(f32,f32)>` | — |
| `pending_pt2: Option<(f32,f32)>` | — |
| `pending_pts: Vec<(f32,f32)>` | — |
| `magnet: bool` | — |
| `draw_color: String` | — |
| `drawings_requested: bool` | — |

**ChartState coverage:** The SlotMap `drawings` collection with typed
`Drawing`/`DrawingId` is the target. All selection, drag, undo, and tool-picker
state is ephemeral UI state — it belongs on a `DrawingInteraction` struct
alongside `ChartState`, not inside it.

**Note:** `Chart::drawings` uses the _legacy_ `Drawing` type (defined earlier
in `gpu.rs`) while `ChartState::drawings` uses the _canonical_ `Drawing`
(`chart/state/drawings.rs`). These types share a name but are different structs.
The migration here is not a rename — it requires a real type-system change and
a codec path from the legacy DB rows to `DrawingId`-keyed SlotMap entries.

---

### Group D — Indicators (MEDIUM RISK)

| Chart field | ChartState equivalent |
|---|---|
| `indicators: Vec<Indicator>` (legacy) | `indicators: SmallVec<[IndicatorRef; 8]>` |
| `indicator_bar_count: usize` | — (renderer cache) |
| `next_indicator_id: u32` | — (IDs now from SlotMap / UUID) |
| `editing_indicator: Option<u32>` | — (UI ephemera) |
| `hide_all_indicators: bool` | — (UI ephemera) |
| `indicator_pts_buf: Vec<Pos2>` | — (renderer scratch buffer) |

**ChartState coverage:** `IndicatorRef` is the typed target; it carries
`ref_id`, `params`, `style`, `pane`, and `param_schema_version`. The legacy
`Indicator` struct uses `IndicatorType` enum + raw color string. The migration
requires a `Indicator → IndicatorRef` converter.

---

### Group E — Annotations (LOW-MEDIUM RISK)

`ChartState::annotations: SlotMap<AnnotationId, Annotation>` is fully designed.
`Chart` has no direct annotation collection — annotations live in the legacy
drawing DB alongside drawings. The migration here is about ensuring new
annotations flow through `ChartState` rather than the legacy path.

---

### Group F — Signal Engine State (MEDIUM RISK)

All signal fields (`trend_health_score`, `exit_gauge_score`, `signal_zones`,
`precursor_*`, `change_points`, `trade_plan`, `divergence_markers`, and their
visibility toggles, `show_*`) are runtime-computed from the signal feed. They
are not persisted per-chart. These belong on a `ChartSignals` struct that lives
beside `ChartState`, not inside it. No migration to `ChartState` required.

---

### Group G — VIX Alert State (LOW RISK — leaf node)

Fields: `vix_expiry_active`, `vix_expiry_days`, `vix_expiry_date`, `vix_spot`,
`vix_expiring_future`, `vix_realized_vol`, `vix_gap_pct`,
`vix_convergence_score`.

These are runtime data from the VIX feed, refreshed on each signal cycle. Not
persisted per-chart. Belong on a `VixAlert` data struct held by the app layer,
not ChartState. No migration to ChartState required.

---

### Group H — Order Entry Scratch (HIGH RISK — interactive trading surface)

~28 fields: `orders`, `order_qty`, `order_is_buy`, `order_market`,
`order_limit_price`, `order_type_idx`, `order_tif_idx`, `order_outside_rth`,
`order_advanced`, `order_bracket`, `order_stop_price`, `order_trail_amt`,
`order_tp_price`, `order_sl_price`, `order_panel_pos`, `order_panel_dragging`,
`order_collapsed`, `dragging_order`, `dragging_alert`, `editing_order`,
`edit_order_qty`, `edit_order_price`, `armed`, `pending_confirms`,
`order_notional_mode`, `order_notional_amount`, `bracket_templates`, +
trigger fields.

**No ChartState equivalent.** This is entirely UI/interaction state for the
order entry panel. Per ADR 0001: "the trading order engine is already at the
target standard and is explicitly out of scope of the decomposition." Order
entry *scratch* fields are not trading engine state — they should live on an
`OrderEntryPanel` struct owned by the renderer, not on `ChartState`.

---

### Group I — Replay (LOW-MEDIUM RISK)

Fields: `replay_mode`, `replay_bar_count`, `replay_playing`, `replay_speed`,
`replay_last_step`, `replay_overlay`.

No ChartState equivalent. Not persisted per-chart in the canonical model (it
is a session mode, not a chart property). Belongs on a `ReplaySession` struct.
`replay_overlay` is a render-side concern.

---

### Group J — DOM / Price Ladder (MEDIUM RISK)

~14 fields: `dom_open`, `dom_sidebar_open`, `dom_levels`, `dom_tick_size`,
`dom_center_price`, `dom_width`, `dom_selected_price`, `dom_order_type`,
`dom_armed`, `dom_col_mode`, `dom_dragging`, `dom_position`, `dom_fullscreen`.

No ChartState equivalent. Pure UI panel state — belongs on a `DomPanel` struct
owned by the renderer.

---

### Group K — Symbol Picker & Navigation (LOW RISK — leaf node)

Fields: `picker_open`, `picker_query`, `picker_results`, `picker_last_query`,
`picker_searching`, `picker_rx`, `picker_pos`, `recent_symbols`,
`pending_symbol_change`, `pending_timeframe_change`, `symbol_history`,
`symbol_history_idx`, `symbol_nav_in_progress`.

No ChartState equivalent needed — this is app-level navigation, not
per-chart state. Should migrate to a `SymbolNav` or `AppNav` struct in the
`state/` aggregates layer, not ChartState.

---

### Group L — Tabs (MEDIUM RISK)

Fields: `tab_symbols`, `tab_timeframes`, `tab_changes`, `tab_prices`,
`tab_active`, `tab_hovered`, `tab_cache`.

No ChartState equivalent. Tabs are a workspace-layout concern. They map to
multiple `ChartState` instances (one per tab), managed by a `TabbedPane` struct
in the workspace aggregate.

---

### Group M — Session Shading (LOW RISK — leaf node, render toggle)

Fields: `session_shading`, `rth_start_minutes`, `rth_end_minutes`,
`eth_bar_opacity`, `session_bg_tint`, `session_bg_color`,
`session_bg_opacity`, `session_break_lines`.

These are user-configurable rendering preferences, should persist with the
chart. ChartState can absorb these via `ExtensionBag` in v1, or via a dedicated
`SessionShadingPrefs` field added to `ChartState` in Phase 5.

---

### Group N — Volume Analytics (MEDIUM RISK — computed cache)

Fields: `vwap_data`, `vwap_upper1/2`, `vwap_lower1/2`, `cvd_data`,
`delta_data`, `rvol_data`, `vol_analytics_computed`, `vp_data`, `vp_last_vs`,
`vp_last_vc`, `vp_mode`, `show_vwap_bands`, `show_cvd`, `show_delta_volume`,
`show_rvol`, and many `show_*` toggle flags.

Computed data (`vwap_data`, etc.) are renderer scratch — never persisted to
ChartState. The `show_*` toggles are user preferences that should persist —
they can live in `ExtensionBag` in v1 or a dedicated field group in Phase 5.

---

### Group O — Analytics Overlays (LOW RISK — render toggles)

Fields: `show_vol_shelves`, `show_confluence`, `show_momentum_heat`,
`show_trend_strip`, `show_breadth_tint`, `show_vol_cone`, `show_price_memory`,
`show_liquidity_voids`, `show_corr_ribbon`.

Boolean toggles for overlay rendering. Same treatment as Group N show flags —
persist via ExtensionBag in v1.

---

### Group P — Options Overlay (LOW RISK — leaf node)

Fields: `show_strikes_overlay`, `overlay_calls`, `overlay_puts`,
`overlay_chain_symbol`, `overlay_chain_loading`, `overlay_chain_placeholder`,
`show_gamma`, `gamma_levels`, `gamma_call_wall/put_wall/zero/hvl`.

Runtime data fetched from the options feed. The `show_*` toggles persist;
the data arrays are scratch. Same treatment as Group N.

---

### Group Q — Fundamental Data (LOW RISK — leaf node)

Fields: `fundamentals: FundamentalData`, `show_analyst_targets`, `show_pe_band`,
`show_insider_trades`, `insider_trades`, `econ_calendar`, `show_darkpool`,
`darkpool_prints`.

Data is fetched at runtime. `show_*` toggles persist. Same treatment as Group N.

---

### Group R — Miscellaneous UI (LOW RISK — leaf node)

Fields: `template_popup_open/pos`, `template_save_name`,
`option_quick_open/pos/dte_idx`, `group_manager_open`, `new_group_name`,
`draw_picker_open/pos/hover_cat/hover_cat_y`, `link_group`,
`price_alerts/next_alert_id/alert_input_price`, `chart_widgets/dragging_widget`,
`show_pnl_curve`, `measuring/measure_start/measure_active`,
`widget_cache/widget_cache_bar_count`, `play_lines/…/play_click_to_set`,
`floating_order_panes`, `spreadsheet_*`, `pane_picker_*`, `pane_template_name`,
`candle_mode`, `renko_*`, `range_bar_size`, `tick_bar_count`, `alt_bars/*`,
`show_footprint`, `swing_leg_mode`, `symbol_overlays/*`, `hit_highlight/*`,
`show_events/event_markers`, `fmt_buf`, `cached_ohlc/*`.

Mix of persisted prefs, UI interaction state, and renderer scratch buffers.
Each should be audited individually in Phase 5.

---

## 3. Equivalence Summary

| Group | ChartState has equivalent? | Migration target |
|---|---|---|
| A — Symbol/Timeframe | Partial | `ChartState::symbol`, `::timeframe` |
| B — Viewport | Partial (persisted scalars only) | `ChartState::viewport` + new `ViewportInteraction` |
| C — Drawings | Partial (collection; not interaction) | `ChartState::drawings` + new `DrawingInteraction` |
| D — Indicators | Partial (IndicatorRef; not legacy Indicator) | `ChartState::indicators` |
| E — Annotations | Yes (SlotMap) | `ChartState::annotations` |
| F — Signal state | No | New `ChartSignals` struct (not in ChartState) |
| G — VIX state | No | App-layer data, not ChartState |
| H — Order entry | No | New `OrderEntryPanel` (renderer-owned) |
| I — Replay | No | New `ReplaySession` (renderer-owned) |
| J — DOM | No | New `DomPanel` (renderer-owned) |
| K — Symbol picker | No | `state/` aggregate `SymbolNav` |
| L — Tabs | No | Workspace aggregate `TabbedPane` |
| M — Session shading | No (but small) | `ChartState` prefs field or ExtensionBag |
| N — Vol analytics | No (toggles only) | ExtensionBag or dedicated field |
| O–Q — Overlays | No (toggles only) | ExtensionBag or dedicated field |
| R — Misc UI | No | Per-field audit |

---

## 4. Migration Order (Leaf-First)

### Tier 0 — Already done (no action needed)
- `ChartState` struct, `drawings` SlotMap, `annotations` SlotMap,
  `indicators` SmallVec, `viewport` scalars, `style_table`, `extension_bag`,
  XOL codec, DB codec stubs, file I/O — all fully designed and tested.

### Tier 1 — Safe infrastructure (can do now without touching core.rs)

**Step 1.1 — Attach `ChartState` to `Chart` as a side-field (the shim anchor)**

Add `chart_state: Option<chart::state::ChartState>` to `Chart`. Initialize it
as `None` in `Chart::new()`. This is zero-cost until populated. No render path
changes. Build verifies: `cargo check --bin apex-native`.

**Step 1.2 — Populate `chart_state` on symbol load**

When the symbol/timeframe is committed (after bars load), construct and assign
a `ChartState` with the correct `symbol`, `timeframe`, and an empty
`viewport`/`drawings`/etc. This makes `ChartState` live (not `None`) without
any read-through yet.

**Step 1.3 — Session shading prefs (Group M) into ChartState ExtensionBag**

When the chart is serialized/saved, write Group M fields into
`chart_state.unknown_extensions` (keyed as `"session_shading"`, etc.). On load,
read them back and apply to `Chart`. This is a one-directional sync (Chart →
ExtensionBag on save; ExtensionBag → Chart on load). Provides real persistence
value with zero renderer risk.

### Tier 2 — Requires type coordination but no hot-path change

**Step 2.1 — Indicator sync (Group D)**

Convert `Chart::indicators: Vec<Indicator>` → `ChartState::indicators:
SmallVec<[IndicatorRef]>` on each indicator add/remove. Keep both in sync via a
`sync_indicators_to_state(chart: &mut Chart)` helper. The renderer keeps
reading `chart.indicators`; `chart_state.indicators` is the write-through copy
for persistence.

**Step 2.2 — Drawing sync (Group C)**

The legacy `Chart::drawings: Vec<Drawing>` (gpu.rs type) feeds the renderer.
Build a converter from `chart::state::Drawing` (SlotMap canonical) → legacy
`Drawing` (gpu.rs). When drawings are loaded from the DB via the XOL/DB codec,
populate `chart_state.drawings` AND convert + fill `chart.drawings`. This makes
ChartState the read source for loaded charts.

### Tier 3 — Viewport (HOT PATH — supervised only)

**Step 3.1 — Viewport: persisted scalars**

After `chart_state` is always populated (Tier 1), sync `Chart::vs`, `Chart::vc`,
`Chart::price_lock`, `Chart::log_scale` → `chart_state.viewport` on save. On
load, read from `chart_state.viewport` and set `Chart` fields. No renderer read
change.

**Step 3.2 — Viewport: live write-through (DANGER)**

Change `core.rs` to read `chart.chart_state.as_ref().unwrap().viewport` instead
of `chart.vs`/`chart.vc`. This is the hot-path change. Requires:
- A micro-benchmark showing frame time delta < 0.5ms
- A live smoke test: pan, zoom, auto-scroll all work
- Code review by a single owner before merge

**This step must NOT be done in an automated sweep. It is a supervised,
measured, single-owner change.**

### Tier 4 — Full god-object decomposition (Future phases)

Once Tiers 1–3 are done and `ChartState` is the live source of truth for
symbol/timeframe/viewport/drawings/indicators:

- Extract signal state → `ChartSignals` struct
- Extract order entry → `OrderEntryPanel`
- Extract DOM → `DomPanel`
- Extract replay → `ReplaySession`
- Remove the `chart_state: Option<ChartState>` shim field; `ChartState` becomes
  the top-level type passed to the renderer
- Delete `Chart` god-object

---

## 5. Shim Strategy

During migration, `Chart` reads through to `ChartState` via a thin accessor
layer rather than replacing fields directly:

```rust
// In Chart
pub(crate) chart_state: Option<chart::state::ChartState>,

// Accessor helpers (not in core.rs)
impl Chart {
    pub(crate) fn state(&self) -> Option<&chart::state::ChartState> {
        self.chart_state.as_ref()
    }
    pub(crate) fn state_mut(&mut self) -> Option<&mut chart::state::ChartState> {
        self.chart_state.as_mut()
    }
}
```

Old code continues to read/write `chart.vs`, `chart.drawings`, etc. New code
(persistence, codec) reads/writes `chart.chart_state`. The sync helpers keep
them in agreement. Once a group is fully migrated and `chart_state` is the
sole source of truth for that group, the legacy field is removed and all
call sites are updated.

---

## 6. Per-Step Verification

Each step must pass:
1. `cargo check --bin apex-native` — no new errors
2. `cargo test --lib chart::state` — all tests pass
3. Smoke test: launch apex-native, open a chart, verify the affected feature
   (drawings visible, indicators shown, scroll works, etc.)

Hot-path steps (Tier 3.2) additionally require:
4. Frame time measurement: `perf_hud` before and after; delta < 0.5ms
5. Single named reviewer signs off before merge

---

## 7. Risk Register

| Risk | Severity | Mitigation |
|---|---|---|
| `core.rs` viewport read change causes frame drop | HIGH | Benchmark requirement; single owner; revert gate |
| Legacy `Drawing` vs canonical `Drawing` type confusion | MEDIUM | Separate type aliases; explicit conversion functions |
| `chart_state` shim field left as `None` when renderer tries to use it | MEDIUM | `unwrap_or_else(|| panic!("chart_state not populated"))` + test that `chart_state.is_some()` after new() |
| Indicators double-written (both `chart.indicators` and `state.indicators`) diverge | LOW | Single write path; sync helper has unit test |
| Option metadata lost (no ChartState field) | LOW | ExtensionBag with round-trip test |
| Tier 4 scope creep into Tier 1/2 sessions | LOW | This document; each PR scoped to its step only |

---

## 8. Proof-of-Concept: Step 1.1 in this PR

Step 1.1 (adding `chart_state: Option<ChartState>` to `Chart`) is the only
migration step executed in this PR. See implementation in `gpu.rs`. The field
is initialized to `None` in `Chart::new()` and is not yet read by the renderer.
This establishes the shim anchor with zero render-path risk.

**Why no further step was safely auto-migratable:**

Steps 1.2+ require wiring the field population to the data-load path
(`io/fetch.rs` → app loop), which touches the data flow outside the `chart/state/`
module boundary. Step 2.x requires a type-incompatible conversion between the
legacy `Drawing` type and the canonical `Drawing` type — that conversion needs
a dedicated `chart/state/compat.rs` module and test coverage before it is safe
to wire to the renderer. Step 3+ is explicitly flagged as supervised-only.

The proof-of-concept value of Step 1.1 is establishing that `ChartState` is a
real field on `Chart` (not dead code), and that the compiler accepts it
alongside the existing ~200 fields.
