# Front-End Spec — Watchlist % Change, Per-Class Snapshots, Time/Timezone

**Audience:** Apex Terminal front-end team
**Backend:** ApexData (`apex-data-v2`, deployed on `apexdatav2`)
**Status:** backend changes are LIVE; this spec is the FE work to consume them.

---

## 0. Why

Two recurring problems traced back to the FE recomputing things the backend should own:

1. **Watchlist % change was wrong/stale** — the terminal computed % itself from raw snapshot fields, gated on a flaky `/api/market_status` poll, and broke in pre-market (`day.close == 0` → −100%) and when the Polygon REST egress blipped.
2. **Chart time axis showed the wrong timezone** — labels were formatted straight from the UTC bar timestamp (9:30 ET rendered as 13:30/14:30), and the DST offset was derived from `now()` (so historical/cross-DST charts were an hour off).

The backend now does the hard parts (session-aware % per class, DST/holiday-correct calendar). The FE should **consume the server values instead of recomputing**.

> **Core principle:** the feed always sends timestamps as **UTC epoch** on the wire. The client converts to market-local for display using `/api/calendar`. Never put local time on the wire.

---

## 1. Endpoint reference (new / changed)

### 1.1 `GET /api/stocks/snap/bulk?tickers=SPY,AAPL,…` — stocks watchlist (bulk)
Per-result fields now include **server-computed change**:
```jsonc
{
  "count": 3,
  "session": "rth" | "closed",          // which formula the server used
  "session_date": "2026-06-26",
  "prev_session_date": "2026-06-25",
  "computed_at_ms": 1782650427902,
  "results": [
    {
      "ticker": "QQQ",
      "change_perc": -1.3763,            // ← USE THIS (signed %, session/DST aware)
      "change_abs":  -9.86,
      "ref_price":   706.52,             // the price change_perc was computed from
      "session": "closed",
      "session_date": "2026-06-26",
      "prev_session_date": "2026-06-25",
      "prev_close": 716.38,              // authoritative prev close (self-healed)
      "prev_close_polygon": 716.38,      // Polygon's raw (dateless) prevDay
      "prev_close_validated": true,      // we cross-checked it against grouped-daily
      "prev_close_mismatch": false,      // true = Polygon's prevDay was wrong; we corrected it
      // raw passthrough still present: day, prevDay, lastTrade, min, todaysChangePerc
    }
  ]
}
```
On upstream failure the body is the **last-good cache**:
```jsonc
{ "stale": true, "served_from_cache": true, "error": "...", "count": N, "results": [ { …, "stale": true } ] }
```

### 1.2 `GET /api/snap/:class/:symbol` — per-symbol snapshot, ALL classes
`class ∈ stocks | options | index | crypto | futures`. Unified shape with `change_perc`:
```jsonc
{
  "symbol": "BTCUSDT", "class": "crypto",
  "last": 60167.77, "prev_close": 60029.0,
  "change_perc": 0.2312, "change_abs": 138.77,
  "source": "polygon" | "apexcrypto" | "ib",
  "ts_ms": 1782650427956,
  "stale": true          // present only when served from last-good cache
}
```
- **stocks / options / index** → Polygon universal snapshot.
- **crypto** → ApexCrypto Redis (24/7).
- **futures** → IB daily bars.
- **HTTP codes:** `200` = data, `404` = valid class but no data yet (NOT an error — render blank/"—"), `400` = genuinely unknown class string only.

### 1.3 `GET /api/calendar/:class[?date=YYYY-MM-DD]` — authoritative time/session metadata (NEW)
The single source of truth for timezone, DST offset, sessions, and holidays — use this instead of hardcoding offsets/sessions client-side:
```jsonc
{
  "class": "stocks",
  "timezone": "America/New_York",        // crypto → "UTC"
  "now": {
    "server_time_utc_ms": 1782650427902,
    "et_offset_min": -240,                // -240 EDT / -300 EST (DST-aware)
    "is_dst": true,
    "session": "premarket" | "rth" | "postmarket" | "gth" | "closed" | "open"(crypto)
  },
  "date": "2026-06-26",
  "et_offset_min": -240,                  // offset for `date` (per-date DST!)
  "is_dst": true,
  "trading_day": true,
  "prev_trading_day": "2026-06-25",
  "next_trading_day": "2026-06-29",
  "rth": { "open_min": 570, "close_min": 960 },     // ET minutes from midnight (9:30–16:00)
  "eth": { "open_min": 240, "close_min": 1200 }      // 04:00–20:00 (futures: overnight; crypto: null)
}
```
Caveat: `now.session` is time-of-day only — it returns `"premarket"` even on a weekend/holiday. **AND it with `trading_day`** before treating the market as live.

### 1.4 `GET /api/market_status`
Polygon passthrough, fetched live (no cache). On upstream failure it returns **HTTP 502 — never a fake `"closed"`**. Polygon reports `"extended-hours"` (not `"open"`) during pre/post.

### 1.5 `GET /api/bars/:class/:symbol/:tf`
- `time` is **epoch SECONDS, UTC**.
- `index` class now supported (`I:SPX` etc.); daily history is current again.

---

## 2. FE tasks

### TASK 1 — Watchlist %: use the server value, drop client-side recompute  *(priority: high)*
- **Stocks rows:** read `change_perc` from `/api/stocks/snap/bulk`. Stop computing `(last − prevDay.close)/prevDay.close` on the client and stop gating the formula on `/api/market_status`.
- **Options / crypto / futures / index rows:** read `change_perc` from `/api/snap/:class/:symbol`.
  - (Open question for backend: if you want a single bulk call for non-stock classes too, say so — currently per-symbol.)
- Display `change_abs` / `ref_price` if useful; color by sign of `change_perc`.
- **Remove** the `day.close == 0` pre-market path entirely — the server handles it.

**Acceptance:** QQQ shows the real session move (e.g. −1.38%), not 0 / −0.07 / −100%, in RTH, after-hours, **and** pre-market.

### TASK 2 — Stale / error handling  *(priority: high)*
- If a snapshot response has `"stale": true` / `"served_from_cache": true`, still **render the value** (it's the last real number) — optionally show a subtle "stale" dot. Do **not** blank the row or revert to an older cache.
- `/api/snap/...` → `404` means "no data yet for this class/symbol" — render `—`, **not** an error toast.
- `/api/market_status` non-200 (e.g. 502) → treat as **"unknown, keep last known"**, never as "closed". (This was the trigger for the wrong-formula bug.)

**Acceptance:** during a backend/Polygon blip the watchlist keeps showing last-good numbers; no error spam; no flip to close-to-close.

### TASK 3 — Time axis & sessions: render in market-local via `/api/calendar`  *(priority: high)*
The chart-renderer label fix (UTC→ET, DST per-bar-date) has already landed in the terminal. The FE work here is to **stop hardcoding** session/offset logic and drive it from `/api/calendar`:
- Fetch `/api/calendar/:class` once per chart (cache per session-date) and use:
  - `timezone` to label the axis / pick ET vs UTC (crypto → UTC).
  - `rth.open_min` / `rth.close_min` / `eth.*` for **session shading** instead of hardcoded `rth_start_minutes` / `570/960`.
  - `et_offset_min` **for the displayed date(s)** when converting — do not use a single `now()` offset across a multi-day/historical view.
  - `trading_day` + `prev/next_trading_day` to skip weekends/holidays in shading and the "last session" logic.
- Decide crypto axis convention: currently UTC. If you want user-local for crypto, do the conversion client-side (the feed gives UTC).

**Acceptance:** intraday labels read in ET (9:30 open candle at 9:30); a January (EST) chart and a July (EDT) chart are both correct; session shading aligns with labels on historical charts; holidays aren't shaded as trading days.

### TASK 4 — Index rows  *(priority: low / informational)*
`I:SPX`, `I:VIX`, `I:NDX` return **404** from `/api/snap/index/...` — the Polygon plan has **no index-snapshot entitlement**, so `change_perc` isn't available for indices. Render `—` (no error). If index % is required, it needs a Polygon indices subscription or an alternate source (raise with backend).

---

## 3. Quick reference — what to stop doing

| Stop | Start |
|---|---|
| Computing % from `(last − prevDay.close)` client-side | Read `change_perc` from snap / snap/bulk |
| Gating the % formula on `/api/market_status` | Trust the server `session` + `change_perc` |
| Treating `day.close == 0` as a valid close (pre-market) | Server already handles it (`ref_price`) |
| Formatting axis labels from the raw UTC timestamp | Convert to market-local (ET DST-aware) — done in renderer; drive sessions from `/api/calendar` |
| Hardcoding ET offset / RTH minutes / holidays | `/api/calendar` (`et_offset_min` per date, `rth`/`eth`, `trading_day`) |
| Showing an error/blank on 404 or 502 | 404 = no data (`—`); 502 = keep last known |

---

## 4. Notes
- All timestamps on the wire are **UTC epoch** (bars: seconds; `*_ms` fields: milliseconds). Convert at the edge for display only.
- `prev_close_mismatch: true` is a useful debug signal — it means the backend corrected a wrong Polygon prevDay; safe to log.
- Backend contacts: the ApexData `apexdatav2` deployment serves all of the above; ingress host `apex-data-v2-dev.xllio.com`.
