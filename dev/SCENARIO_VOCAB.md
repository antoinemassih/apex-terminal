# Scenario authoring vocabulary (interactive-verified)

Scenarios are JSON files in `dev/scenarios/`. They drive the **real** window via
the dev-inspector and assert on real captured state. Use ONLY the commands and
assertions below — they are verified to work against the live interactive build.
Anything else either errors (`unknown command`) or only mutates the headless
synthetic mirror (no real effect).

## File shape
```json
{
  "name": "story_unique_snake_name",
  "description": "One sentence describing the user story.",
  "story": "Domain/SubArea",
  "tags": ["story", "..."],
  "priority": 2,
  "settle_ms": 15,
  "abort_on_failure": false,
  "steps": [ ...step objects... ]
}
```

## Step actions
- `{"action":"reset"}` — reset app to a clean state.
- `{"action":"wait_frames","count":N}` — advance N real frames.
- `{"action":"wait","ms":N}` — sleep N ms (use after a symbol/timeframe change to let the async data fetch land, e.g. 600–800ms).
- `{"action":"log","message":"..."}` — breadcrumb (prints `[scenario] ...`).
- `{"action":"cmd","cmd":"<Command>", <fields...>}` — drive the app (see commands).
- `{"action":"assert","assertions":[ ... ]}` — evaluate assertions; any false → step fails → scenario fails.
- Add `"expect_fail":true` to a `cmd` step to assert the command is REJECTED (negative test).

## Commands (real AppCommand bus — all mutate the live window)
| cmd | fields | notes |
|-----|--------|-------|
| `SwapPaneSymbol` | `pane`,`symbol` | follow with `wait` 600–800ms for data |
| `ChangeTimeframe` | `pane`,`tf` | tf ∈ 1m,5m,15m,30m,1h,4h,1d,1w |
| `ChangePaneType` | `pane`,`kind` | Chart,Portfolio,Dashboard,Heatmap,Spreadsheet,OptionsSentiment,OptionsFlow |
| `SetChartFlag` | `pane`,`flag`,`value` | flags: ShowVolume,LogScale,Magnet,OhlcTooltip,MeasureTooltip,ShowOscillators,ShowPrevClose,ShowPatternLabels,ShowFootprint,ShowGamma,ShowStrikesOverlay,HideAllIndicators,HideAllDrawings |
| `AddIndicator` | `pane`,`kind` | SMA,EMA,WMA,DEMA,TEMA,VWAP,BB,ICHI,PSAR,ST,KC,RSI,MACD,STOCH,ADX,CCI,WILLIAMSR,ATR,OBV |
| `RemoveIndicator` | `pane`,`id` | id is the numeric indicator id |
| `RecomputeIndicators` | `pane` | force recompute |
| `SetThemeIdx` | `pane`,`idx` | idx 0..20 (21 themes) |
| `SetStyleIdx` | `idx` | idx 0..8 (9 styles) |
| `AddPriceAlert` | `pane`,`price`,`above` | local draft alert |
| `CancelAllOrders` / `ClearOrderHistory` | — | SAFE. **Never** use `PlaceAllDraftOrders`/`PlaceAllDraftAlerts** (would submit). |
| `WatchlistAddSymbol`/`WatchlistRemoveSymbol` | `symbol` | |
| `WatchlistCreate`/`WatchlistRenameActive` | `name` | |
| `WatchlistSwitchActive`/`WatchlistDelete` | `idx` | |
| `WatchlistAddSection`/`WatchlistAddOptionSection` | `title` | |
| `WatchlistRemoveSection`/`WatchlistToggleSectionCollapse` | `idx` | |

Liquid symbols with reliable data: SPY,QQQ,AAPL,MSFT,NVDA,TSLA,AMZN,META,GOOGL,AMD,NFLX,IWM,DIA,GLD,XLF,XLK,AVGO,COST.

**Cross-asset symbols (verified to load real bars):**
- Futures: ES, NQ, YM, RTY (index), CL, GC, SI, HG (commodity), ZB, ZN (bond).
- Crypto: BTC-USD, ETH-USD, SOL-USD.
Use them just like stocks via `SwapPaneSymbol`. After loading, assert `pane_symbol_equals` + invariants.

**Alias display flags** (also valid for `SetChartFlag`): ExtendedHours, ShowTrades, CrosshairEnabled, AutoScale, ChartType.

**Pane types** (`ChangePaneType` kind): Chart, Portfolio, Dashboard, Heatmap, Spreadsheet, OptionsSentiment, OptionsFlow. Note: OptionsSentiment/OptionsFlow report `pane_type` as `Dashboard` — assert `{"pane_type_equals":{"pane":0,"type":"Dashboard"}}` for those.

**Order/alert state** (in app_state):
- `{"state_field_equals":{"path":"total_order_count","value":0}}` after `CancelAllOrders`.
- `{"state_field_gte":{"path":"total_alert_count","min":N}}` after adding alerts (alerts accumulate across the session, so use gte / workflow, not exact).
- `OpenIndicatorEditor`/`CloseIndicatorEditor` (pane,id) — id is the numeric indicator id (0,1,… after a reset clears indicators).

## Assertions (evaluate against real captured state)
**Invariants — include these in nearly every assert step (they only fail on real bugs):**
- `{"no_panic":true}` — no thread panicked since scenario start (crash = highest severity).
- `{"viewport_sane":true}` — price range not inverted/collapsed, densities positive.
- `{"canvas_all_finite":true}` — no NaN/Inf in bars, viewport, indicators, drawings.
- `{"canvas_bars_monotonic":true}` — bars strictly time-ordered, left→right.
- `{"canvas_bars_screen_ordered":true}` — candle geometry: high above low, body within wick.
- `{"canvas_bars_within_pane":{"pane":0}}` — bars paint inside their pane.
- `{"canvas_bar_ohlc_valid":{"pane":0}}` — high≥low, close within range.
- `{"fps_above":5.0}` — render loop not stalled (use 3.0 for stress).

**Targeted (real-state):**
- `{"pane_symbol_equals":{"pane":0,"symbol":"AAPL"}}`
- `{"pane_timeframe_equals":{"pane":0,"tf":"5m"}}`
- `{"pane_type_equals":{"pane":0,"type":"Dashboard"}}`
- `{"canvas_indicator_exists":{"pane":0,"kind":"RSI"}}`
- `{"canvas_indicator_count_equals":{"pane":0,"count":3}}`
- `{"canvas_indicator_value_in_range":{"pane":0,"kind":"RSI","min":0,"max":100}}` (RSI/STOCH 0–100, WILLIAMSR -100–0)
- `{"canvas_visible_bar_count_gte":{"pane":0,"min":1}}`
- `{"no_clipped_widgets":true}`, `{"design_audit_clean":true}`, `{"no_active_violations":true}`
- `{"widget_exists":{"role":"button"}}`, `{"widget_label_contains":{"contains":"AAPL"}}`
- `{"all_of":[...]}`, `{"any_of":[...]}`, `{"not":{"assertion":{...}}}`

## Functional-correctness & UX assertions (new — "is it working as intended")
These read app state that the capture now exposes, so the harness verifies the app
*does the right thing*, not just that it doesn't crash:
- `{"gamma_overlay_active":{"pane":0}}` — gamma overlay is ON **and** has levels to draw (fails if enabled-but-blank). Gamma loads async — settle ~2.5 s after enabling.
- `{"strikes_overlay_active":{"pane":0}}` — strikes overlay ON **and** has option-chain rows. Async; settle ~2.5 s.
- `{"watchlist_pct_present":true}` — every *loaded* watchlist row has a server `change_perc`.
- `{"watchlist_pct_sane":25.0}` — all `change_perc` values within ±25% (catches "% completely wrong").
- `{"canvas_indicator_correct":{"pane":0,"kind":"SMA","rel_tol":0.01}}` — recomputes the indicator from captured bars and checks the chart's value matches within 1%. Window indicators only (SMA, WMA); recursive ones (EMA/RSI/MACD/…) are skipped (pass).
- `{"ux_audit":true}` — bundled usability check: no clipped widgets, no overlapping buttons, no buttons/inputs below the touch floor (default 16px — desktop-appropriate; pass `{"min_touch_px":N}` to override). Clipping/overlap are hard checks; the touch floor is intentionally low because this is a dense mouse-driven terminal.

## Subsystem drivers (NEW — DOM / orders / scanner / RRG / heatmap / gamma)
These harness-only commands OPEN and OBSERVE subsystems that have no production
command (UI-only). All are broker-SAFE — they never submit orders or hit the network.
| cmd | fields | effect |
|-----|--------|--------|
| `SynthGamma` | `pane` | show gamma + populate ~31 synthetic GEX levels + walls (deterministic, no `:8412` feed needed) |
| `SetDomSidebar` | `pane`,`open` | open/close the DOM ladder; opening auto-fills a 61-row mock ladder |
| `SeedDraftOrder` | `pane`,`side`(buy/sell/stop),`price`,`qty` | push a **visual-only** DRAFT order (never touches OrderManager/broker) |
| `SetOrderPanel` | `pane`,`collapsed` | collapse/expand the order-entry panel |
| `CancelOrder` | `pane`,`id` | cancel one visual order |
| `SetScannerOpen` | `open` | open/close the scanner panel |
| `SeedScannerResults` | `count` | seed a deterministic pool of `count` scanner rows (≤500) |
| `SetRrgOpen` | `open` | open/close the RRG panel (renders 11 deterministic demo sectors) |
| `SetRrgTail` | `len` | RRG tail length (1..20) |
| `SeedHeatmapCells` | `count` | seed `count` deterministic heatmap cells (≤200) |

**Note on visual orders:** `SeedDraftOrder` adds to the pane's *visual* list. The
per-frame reconcile REMOVES cancelled local-only orders, so after `CancelAllOrders`
assert `order_count == 0` (drafts cleaned up), not a cancelled count.

## Subsystem state (in app_state — assert with `state_field_*`)
Per-pane (`panes.<i>.<field>`): `dom_sidebar_open`, `dom_level_count`, `dom_best_bid`,
`dom_best_ask`, `dom_prices_desc`, `dom_is_live`, `order_draft_count`,
`order_placed_count`, `order_cancelled_count`, `order_panel_collapsed`,
`gamma_level_count`, `gamma_call_wall`, `gamma_put_wall`, `show_gamma`,
`strikes_call_count`, `strikes_put_count`.
Global: `scanner.open`, `scanner.result_count`, `scanner.first_def_filtered_count`,
`rrg.open`, `rrg.tail_length`, `rrg.sector_count`, `heatmap.cell_count`,
`order_manager.paper_mode`, `order_manager.mgr_working_count`.

**Order-entry SAFETY invariants (put in EVERY order scenario):**
- `{"no_live_orders":true}` — asserts `paper_mode` on AND 0 orders in a submitted state (proof nothing was submitted).
- `{"dom_spread_sane":{"pane":0}}` — DOM ladder populated, prices strictly descending, best ask ≥ best bid.

## Behavioral-correctness oracles (verify it computes the RIGHT thing, not just "populated")
These derive the correct answer independently and compare — the "behaving as intended" bar:
- `{"rrg_quadrants_correct":true}` — every RRG sector's quadrant matches the standard rule from its (rs_ratio, rs_momentum): LEADING both≥100, WEAKENING ratio≥100/mom<100, LAGGING both<100, IMPROVING ratio<100/mom≥100.
- `{"gamma_structure_sane":{"pane":0}}` — synthesized GEX has correct structure: put wall < flip (zero-gamma) < call wall, all positive.
- `{"dom_ladder_correct":{"pane":0}}` — DOM is a real price ladder: ≥3 levels, strictly descending, uniform tick spacing.
- `{"order_matches":{"pane":0,"side":"Buy","price":123.45,"qty":500}}` — the pane's order list contains an order with exactly that side/price/qty (round-trip).
- `{"scanner_filter_correct":true}` — independently recompute filter+sort+truncate from the raw pool + def[0] criteria and compare symbol-for-symbol to the app's output.

Examples:
- `{"state_field_gte":{"path":"panes.0.gamma_level_count","min":1}}` after `SynthGamma`.
- `{"state_field_gte":{"path":"panes.0.dom_level_count","min":1}}` after `SetDomSidebar` open.
- `{"state_field_equals":{"path":"scanner.result_count","value":50}}` after `SeedScannerResults` count=50.
- `{"state_field_equals":{"path":"rrg.sector_count","value":11}}` after `SetRrgOpen`.

## Auto-Charting panel — front-end testing (real widget clicks)
Drive the actual panel controls and verify they change config:
- `SetAutoChartPanel` cmd `{open}` — open/close the Auto-Charting side panel.
- `{"action":"click_widget","id":"auto_chart.trendlines"}` — click a recorded panel control by id (resolves its rect from the widget-tree, injects a real move→down→up click). **Settle after opening the panel** (it animates): `wait 700ms` + `wait_frames 8` before clicking.
- Recorded control ids: `auto_chart.enabled`, `.window`, `.anchored_only`, `.trendlines`, `.channels`, `.levels`, `.patterns`, `.candles`, `.pivot.{hybrid,atr,percent}`, `.extend.{none,right,both,left}`.
- Observable (`state_field_*` on `auto_chart.*`): `open`, `enabled`, `trendlines`, `channels`, `levels`, `patterns`, `candles`, `pivot_mode`, `extend`, `window`, `anchored_only`, `methods_count`, `signal_drawing_count`.
- Assert controls render: `{"widget_exists":{"id":"auto_chart.trendlines"}}`; conditional hiding: when `enabled=false` the layer controls disappear (`{"not":{"assertion":{"widget_exists":{"id":"auto_chart.trendlines"}}}}`).
NOTE: auto-draw OUTPUT (signal_drawings) is backend-computed (ApexSignals :8100) — not produced offline. This tests the PANEL front-end (controls render + drive config), not the backend detection.

## Spreadsheet formula correctness (behavioral oracle)
- `SetCell` cmd `{pane,row,col,text}` — set a cell's raw text (grows the grid). Cell A1=(row0,col0), B1=(0,1), A3=(2,0).
- `{"spreadsheet_cell_equals":{"pane":0,"row":0,"col":3,"value":30}}` — assert the app's formula-evaluated value for a cell equals the expected (verifies the SUM/AVG/MIN/MAX/COUNT + arithmetic engine). Switch the pane to Spreadsheet first (`ChangePaneType Spreadsheet`).
- Observable: `panes.<i>.spreadsheet.computed[row][col]` (formula-evaluated numeric grid).

## Playbook (plays) correctness + panel
- `SetPlaybookPanel` cmd `{open}` — open/close the Playbook panel.
- `SeedPlay` cmd `{symbol,long,entry,target,stop}` — add a directional play (local only).
- `ClearPlays` cmd — remove all plays.
- `{"play_rr_correct":true}` — every play's `risk_reward` equals `|target-entry|/|entry-stop|`.
- Observable: `playbook.open`, `playbook.play_count`, `playbook.plays.<i>.{symbol,direction,entry,target,stop,risk_reward,status}`.

## Screenshot action (visual review)
- `{"action":"screenshot","name":"my_state"}` — saves the live window to `dev/screenshots/my_state.png` (real GDI capture of what the user sees). Use to snapshot key states for visual/UX review.

## Rules for good stories
1. Always `reset` + `wait_frames` first; after a symbol/tf change, `wait` 600–800ms then `wait_frames`.
2. End meaningful steps with an `assert` carrying the invariant block + any targeted checks.
3. One coherent user journey per file. Name and `story` should describe it.
4. Use only the vocab above. Don't invent commands/assertions.
