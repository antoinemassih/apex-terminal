# Visibility upgrade — correctness & UX testing — 2026-06-28

Extended the harness from "did it break?" to "is it doing the right thing?" by
giving it visibility into app state it couldn't see before, plus on-demand
screenshots for visual/UX review.

## New capture fields
- **Per-pane options overlays** (`canvas.rs` `PaneOverlays`): `show_gamma`,
  `gamma_level_count`, gamma walls/zero/hvl, `show_strikes_overlay`,
  strikes call/put row counts, chain symbol + loading flag.
- **Watchlist rows** (`mod.rs` app_state): every row's symbol, last, prev_close,
  server `change_perc`, loaded flag.

## New assertions
- `gamma_overlay_active` / `strikes_overlay_active` — overlay enabled AND populated.
- `watchlist_pct_present` / `watchlist_pct_sane` — % column populated and within ±N%.
- `canvas_indicator_correct` — recompute oracle; compares the chart's value to an
  independent recompute from captured bars (SMA validated <1%). Guarded to only
  fire when the chart is anchored to the latest bar; recursive indicators skipped.
- `ux_audit` — bundled usability check: clipping + sub-28px touch targets + button overlap.
- `screenshot` step action + `POST /screenshot` — saves the live window to
  `dev/screenshots/<name>.png` via GDI (captures what the user sees).

## Verified WORKING (the harness now confirms correct behaviour)
- Watchlist % present and sane — 3/3 scenarios.
- SMA numerical correctness within 1% of an independent recompute — 4/4 symbols.
- Screenshots captured (`ux_clean_chart.png`, `nvda_vwap_rsi.png`, `nvda_gamma_on.png`).
- UX audit fires and catches real issues (below).

## Real findings surfaced (these match the original bug report)

### 1. Strikes overlay never populates — even though the chain feed returns 200 OK
Repro: `903–905_strikes_overlay_*`. Enabling the strikes overlay sets
`overlay_chain_loading` and requests the chain; `/api/chain/SPY` returns **200 OK**
(~0.5 s, every ~6 s), yet `overlay_calls`/`overlay_puts` stay empty and the overlay
is blank — the auto-fetch (core.rs:3544) keeps re-requesting because the result
never lands. The result handler (gpu.rs:4648) requires
`chart.symbol == result.symbol && overlay_chain_loading`; the overlay isn't getting
populated, so the chain data the feed returns is being dropped before it reaches the
pane. **This is the "options chain doesn't appear on chart" bug, now reproducible.**

### 2. Gamma overlay shows 0 levels when enabled via the flag
Repro: `900–902_gamma_overlay_*`. The chart only DRAWS gamma when
`gamma_levels` is non-empty (core.rs:3017), but the gamma fetch
(`fetch_gamma_from_feed`) is wired only to the toolbar gamma action
(chart_controls.rs:579) / `refresh_gamma_feeds` — there is no render-path auto-fetch
when `show_gamma` becomes true (unlike the strikes overlay, which has one at
core.rs:3544). So toggling `ShowGamma` shows the flag on but never loads gamma data
→ blank overlay. **This is the "gamma levels don't appear" bug.**

### 3. UX / usability
Repro: `913_ux_audit_baseline`. `toolbar.workspace_btn`, `indicators_btn`,
`widgets_btn` are clipped; many icon buttons are below the 28px touch-target floor
(connection 20px, watchlist_toggle 22px, layout/settings/search/toolnav/timeframe
24px). Low severity but real and now machine-detected.

## Honest limits
- The strikes/gamma findings are reproduced with the data feed confirmed responding
  (chain = 200 OK), so they are app-side, not "no data" — but I have not root-caused
  the exact drop point or fixed them yet (load-bearing async/fetch wiring). Flagged
  for a focused fix.
- Indicator correctness oracle covers window indicators (SMA) only; recursive ones
  (EMA/RSI/MACD/DEMA/TEMA/Supertrend) carry history and are skipped.
- `dev/screenshots/*.png` are generated artifacts (gitignored); regenerate by running
  the screenshot scenarios.
