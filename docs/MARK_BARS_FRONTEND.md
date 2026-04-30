# Mark Bars — Frontend Integration

ApexData v2 ships a `Mark` candle source built from NBBO mid (`(bid+ask)/2`)
in addition to the default `Last` (trade-print) bars. Used by traders to fill
out chart density on thin-volume options the same way ToS's "Mark" mode does.

The wire spec lives in the ApexData repo at
`docs/MARK_BARS_PROTOCOL.md`. This doc is the apex-terminal-side checklist.

## Quick reference

| Surface | Change |
|---|---|
| REST `/api/bars/.../{tf}` | Accepts `?source=last\|mark`. Default `last`. |
| REST `/api/replay/.../{tf}` | Same — `?source=last\|mark`. |
| WS subscribe | New parallel array `subscribe_mark` next to `subscribe`. **Both arrays replace the full sub state per message** — always send both. |
| WS bar frame | Now carries `"source": "last" \| "mark"`. Route by source. |

## What needs to change in apex-terminal

### 1. Chart header toggle

Add a `Last | Mark` segmented control next to the timeframe picker. Per-chart
state. Default `Last`. Persist with the chart's settings struct alongside
symbol/timeframe.

### 2. On toggle (or initial load with non-default source)

```
1. Cancel current bar WS subscription state (we'll replace it below).
2. Clear the bar buffer for that chart pane.
3. Fetch initial window: GET /api/bars/{class}/{symbol}/{tf}?source={src}
4. If panning back: GET /api/replay/{class}/{symbol}/{tf}?from=...&to=...&source={src}
5. Send a new WS subscribe message with the full intended state of BOTH arrays.
```

Example WS subscribe (chart pane wants SPY on Mark, AAPL on Last):

```json
{
  "subscribe":      ["AAPL"],
  "subscribe_mark": ["SPY"]
}
```

To stop ALL mark subs while keeping last subs:
```json
{ "subscribe": ["AAPL"], "subscribe_mark": [] }
```

To stop everything:
```json
{ "subscribe": [], "subscribe_mark": [] }
```

**Critical:** the server replaces both arrays atomically per message. If you send
only `subscribe_mark`, the server treats `subscribe` as empty and drops all your
trade-bar subs. Always send both arrays.

### 3. Bar frame ingest

`BarUpdate` now has a `source` field. Route by source:

```rust
match update.source.as_str() {
    "mark" => mark_pane.push_bar(update.bar),
    _      => last_pane.push_bar(update.bar),  // default + back-compat
}
```

If a frame arrives whose source doesn't match the pane's current selection,
drop it. There's a brief race window during toggle where the previous source's
last frame may still be in flight.

### 4. Volume pane behavior on Mark

Mark bars carry `volume = 0` (no traded volume — they're synthesized from
quotes). When the pane is on Mark:

- Hide or grey the volume histogram.
- Disable indicators that need volume: VWAP, OBV, volume profile. Either
  show them as N/A or fall back to a parallel `last` subscription just for
  those indicators.

### 5. Visual hint

Render a small `MARK` badge in the chart corner when on Mark mode. Without it,
traders may confuse mark candles with trade prints — they look the same shape
but tell a different story.

## Plan tier caveat (important UX)

Polygon's plan we're on serves option NBBO history but **403s on stock NBBO
history**. So:

- **Options charts on Mark:** full history available immediately (premarket
  CronJob backfills nightly).
- **Stock charts on Mark:** no history. Mark bars only accumulate live from
  the WS quote stream during market hours. On first toggle, the pane will be
  empty.

When the user toggles a stock chart to Mark and we have no history, show:

```
Accumulating mark history since {first_quote_time_ms}
Pan-back available after several days of live data.
```

Don't error. Don't fall back to Last silently — that defeats the toggle. Just
explain the state.

## Test data already in QuestDB

(Updated whenever the cron jobs run — current snapshot below.)

| Symbol | Expiries | Strike range | History |
|---|---|---|---|
| SPY OCCs | 2026-04-27, 2026-04-28 | ATM ±2.5% | 5 days |
| QQQ OCCs | 2026-04-27, 2026-04-28 | ATM ±2.5% | 5 days |
| AAPL OCCs | 2026-04-27, 2026-04-29 | ATM ±2.5% | 5 days |
| NVDA OCCs | 2026-04-27, 2026-04-29 | ATM ±2.5% | 5 days |
| TSLA OCCs | 2026-04-27, 2026-04-29 | ATM ±2.5% | 5 days |

Quick smoke test:
```bash
curl -H "Host: apex-data-dev.xllio.com" \
  "http://192.168.1.71/api/bars/options/O:SPY260427C00714000/1m?source=mark" \
  | jq 'length'
```
Should return `1000` (L2 cache cap) with real OHLC mid prices.

## Defaults & fallbacks

- Default chart source on first open: **Last** (back-compat).
- WS clients that don't send `subscribe_mark`: get nothing on the mark stream
  (safe — old clients keep working).
- Bar frames missing `source`: treat as `"last"` (server will always populate
  it but be defensive).
