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

## Screenshot action (visual review)
- `{"action":"screenshot","name":"my_state"}` — saves the live window to `dev/screenshots/my_state.png` (real GDI capture of what the user sees). Use to snapshot key states for visual/UX review.

## Rules for good stories
1. Always `reset` + `wait_frames` first; after a symbol/tf change, `wait` 600–800ms then `wait_frames`.
2. End meaningful steps with an `assert` carrying the invariant block + any targeted checks.
3. One coherent user journey per file. Name and `story` should describe it.
4. Use only the vocab above. Don't invent commands/assertions.
