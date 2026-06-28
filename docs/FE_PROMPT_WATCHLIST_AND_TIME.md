# Prompt — Apex Terminal: consume backend watchlist-% + time/timezone changes

You are working in the **apex-terminal** repo (Rust/egui GPU charting app, `src-tauri/`). The backend **ApexData** (`apex-data-v2`, ingress `apex-data-v2-dev.xllio.com`) has shipped server-side fixes; your job is to make the terminal consume them instead of recomputing. Two user-visible bugs are being fixed: (1) watchlist % change is wrong/stale, (2) the chart time axis shows the wrong timezone.

**Core principle:** the feed sends all timestamps as **UTC epoch** (bars in seconds, `*_ms` fields in milliseconds). Convert to market-local only at the display edge. Never assume local time on the wire.

## Backend endpoints you must use (already live)

1. `GET /api/stocks/snap/bulk?tickers=SPY,AAPL,…` — each `results[]` row now has server-computed:
   `change_perc` (signed %, session/DST-aware — USE THIS), `change_abs`, `ref_price`, `session` ("rth"|"closed"), `session_date`, `prev_session_date`, `prev_close`, `prev_close_polygon`, `prev_close_validated`, `prev_close_mismatch`. Raw `day`/`prevDay`/`lastTrade`/`min` still present. On upstream failure the whole body is last-good cache: `{ "stale": true, "served_from_cache": true, "results": [{…,"stale":true}] }`.

2. `GET /api/snap/:class/:symbol` — `class ∈ stocks|options|index|crypto|futures`. Returns `{ symbol, class, last, prev_close, change_perc, change_abs, source ("polygon"|"apexcrypto"|"ib"), ts_ms, stale? }`. HTTP `200`=data, `404`=valid class but no data yet (render `—`, NOT an error), `400`=unknown class string only.

3. `GET /api/calendar/:class[?date=YYYY-MM-DD]` — authoritative time/session metadata (use instead of hardcoding):
   `{ timezone ("America/New_York"|crypto:"UTC"), now:{ server_time_utc_ms, et_offset_min (-240 EDT/-300 EST), is_dst, session }, date, et_offset_min (per `date`!), is_dst, trading_day, prev_trading_day, next_trading_day, rth:{open_min,close_min} (ET min-from-midnight, 9:30–16:00=570–960), eth:{open_min,close_min} }`.
   Caveat: `now.session` is time-of-day only (returns "premarket" even on weekends) — AND it with `trading_day` before treating the market as live.

4. `GET /api/market_status` — live Polygon passthrough; returns **HTTP 502 on failure, never a fake "closed"**. Polygon reports `"extended-hours"` (not `"open"`) in pre/post.

5. `GET /api/bars/:class/:symbol/:tf` — `time` is **epoch SECONDS UTC**; `index` class now supported.

## Tasks

**TASK 1 — Watchlist %: use the server value (high priority).**
- Stocks rows: read `change_perc` from `/api/stocks/snap/bulk`. DELETE the client-side `(last − prevDay.close)/prevDay.close` computation and the `/api/market_status`-gated formula switch, including the `day.close == 0` pre-market branch.
- Options/crypto/futures/index rows: read `change_perc` from `/api/snap/:class/:symbol`.
- Color by sign of `change_perc`; optionally show `change_abs`/`ref_price`.
- Acceptance: QQQ shows the real session move (~−1.38%), not 0 / −0.07 / −100%, in RTH, after-hours, AND pre-market.

**TASK 2 — Stale/error handling (high priority).**
- If a response has `stale:true`/`served_from_cache:true`, still render the value (last real number); optionally a subtle "stale" indicator. Do not blank or revert.
- `/api/snap/...` `404` → render `—` (no error toast).
- `/api/market_status` non-200 → treat as "unknown, keep last known", never "closed".
- Acceptance: during a backend/Polygon blip the watchlist keeps showing last-good numbers; no error spam; no formula flip.

**TASK 3 — Time axis & sessions from `/api/calendar` (high priority).**
- The renderer label conversion (UTC→ET, DST per-bar-date) already landed in `src/chart/renderer/render/pane/core.rs` (`chart_local_ts` / `et_offset_min_for`). Your job: stop hardcoding session/offset logic — drive it from `/api/calendar`:
  - Fetch `/api/calendar/:class` once per chart (cache per session-date).
  - Use `rth.open_min`/`rth.close_min`/`eth.*` for session shading instead of hardcoded `rth_start_minutes`/`570`/`960`.
  - Use `et_offset_min` for the DISPLAYED date(s); never apply one `now()` offset across a multi-day/historical view.
  - Use `trading_day` + `prev/next_trading_day` to skip weekends/holidays in shading + "last session" logic.
  - Pick crypto axis convention (feed gives UTC; convert to user-local client-side if desired).
- Acceptance: 9:30 open candle sits at 9:30 ET; a January (EST) and a July (EDT) chart are both correct; session shading aligns with labels on historical charts; holidays not shaded as trading days.

**TASK 4 — Index rows (low/informational).**
- `/api/snap/index/I:SPX` returns `404` (no Polygon index-snapshot entitlement) — render `—`, no error. Don't block on it.

## Constraints
- `src/chart/renderer/render/pane/core.rs` is the perf-critical, "verify-in-the-running-app" file (see `src-tauri/CLAUDE.md`). Keep changes minimal, no mechanical sweeps, and verify visually in the running app.
- `Watchlist`/`Chart` structs are frozen — new state goes on `ChartState`/state aggregates (see CLAUDE.md "State: god-objects FROZEN").
- Build (`cargo build --release --bin apex-native`) before claiming compile-clean; verify the watchlist + axis visually in light theme (Bauhaus) and a non-current DST period.

## Stop / Start
| Stop | Start |
|---|---|
| Computing % client-side from prevDay.close | Read server `change_perc` |
| Gating % on `/api/market_status` | Trust server `session` + `change_perc` |
| Treating `day.close==0` as a close | Server `ref_price` handles it |
| Formatting axis labels from raw UTC ts | Market-local (done in renderer); drive sessions from `/api/calendar` |
| Hardcoding ET offset / RTH minutes / holidays | `/api/calendar` |
| Error/blank on 404 or 502 | 404=`—`; 502=keep last known |

Open questions to raise with backend if needed: (a) want a single BULK endpoint for non-stock classes? (currently per-symbol), (b) crypto axis tz preference (UTC vs user-local).
