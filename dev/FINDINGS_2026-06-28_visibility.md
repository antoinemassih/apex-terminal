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

## Findings surfaced (follow-up investigation revised these — see honest notes)

> Update: a focused follow-up showed the gamma/strikes overlays are blank here
> because their upstream data sources are absent in this environment, not because of
> app-logic bugs. The harness's *visibility* is the win — it detects blank overlays
> and will catch a real regression when the feeds are up. The toolbar/touch-target
> UX finding (#3) is a genuine app-side issue.

### 1. Strikes overlay stays blank — but the data sources are down in THIS env
Repro: `903–905_strikes_overlay_*`. The harness correctly detects the blank overlay.
On investigation (2026-06-28, follow-up): `fetch_overlay_chain_background` tries
apex-data `get_chain` → apexib `/options/` fallback → a synthetic placeholder. The
result handler's `[overlay-chain] Loaded …` log **never fires** (0 occurrences), and
the log shows the apexib backend is unreachable here
(`http://apexibdata-api:5000/contract/SPY` resolve failures). apex-data's chain
endpoint could not be reached from outside the app to confirm whether its rows are
usable. **Conclusion: in this environment the option-chain data isn't available, so
the overlay legitimately can't populate — this is largely a data-availability issue,
NOT confirmed app-logic.** Loose end worth a real-data retest: even the synthetic
placeholder path didn't render, which I could not isolate statically. (The auto-fetch
also re-requests the chain every ~6 s while empty — a minor efficiency issue.)

### 2. Gamma overlay shows 0 levels — NOT an app bug (corrected)
Repro: `900–902_gamma_overlay_*`. **Correction to an earlier draft of this doc:** the
gamma overlay IS wired correctly. `refresh_gamma_feeds` runs every render frame
(core.rs:12013) and auto-fetches gamma for supported symbols, so there is a
render-path trigger. Two real reasons it's blank here: (a) gamma support is limited
to `QQQ,SPY` (env `APEX_GAMMA_FEED_SYMBOLS`), so the NVDA scenario is blank by
design; (b) gamma data comes from a **separate gamma feed service on port 8412**
(`fetch_gamma_from_feed`, env `APEX_GAMMA_FEED_URL`) which is not running in this
environment. So this is a data-source-availability issue, not defective wiring. No
code fix appropriate. The valuable outcome: the harness now *detects* a blank gamma
overlay, so it will catch a real regression when the feed IS up.

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
