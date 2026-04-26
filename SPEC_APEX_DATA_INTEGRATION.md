# SPEC — apex-data integration

How the Apex Terminal consumes market data from the `apex-data` service.

The primary integration is already shipped as a drop-in `DataProvider`:
`src/data/ApexDataProvider.ts`, activated in `src/main.tsx`. This spec documents
the wire contract and the deeper features the FE team should surface so the
terminal fully leverages the service.

---

## 1. Deploy & connectivity

| | Dev | Prod |
|---|---|---|
| Base URL | `http://apex-data-dev.xllio.com` | `http://apex-data.xllio.com` |
| WS URL | `ws://apex-data-dev.xllio.com/ws` | `ws://apex-data.xllio.com/ws` |
| Auth | optional | Bearer token (see §7) |

Override at runtime (no rebuild):
- `window.APEX_DATA_URL = 'http://...'` — injected by Tauri bootstrap / build
- `localStorage['apex-data-url']` — user/dev override
- `localStorage['apex-data-token']` — auth token, per user

The `ApexDataProvider` constructor accepts explicit `{ url, token }` opts if
you want to bypass those and drive it from app settings.

---

## 2. Endpoints reference

### Health / status
- `GET /api/health/live`  → `200 "ok"` (always; no auth even if token set)
- `GET /api/health/ready` → `200 {ready, redis, questdb, feeds_connected, tick_age_ms}` or `503`
- `GET /api/feeds`        → per-socket connection state + `circuits.polygon_rest`
- `GET /api/stats`        → uptime, symbol counts, version
- `GET /metrics`          → Prometheus text (always public)

### Symbols & prices
- `GET /api/symbols`          → `{stocks: [...], option_underlyings: [...]}`
- `GET /api/price/:symbol`    → `{symbol, asset_class, price}` (stocks probed first, then options)
- `GET /api/quote/:symbol`    → NBBO snapshot
- `GET /api/quote`            → all NBBOs
- `GET /api/snap/:class/:sym` → full L1 snapshot for watchlist / order ticket
  (last, bid, ask, sizes, day O/H/L/volume, session date, updated_at_ms)

### Bars (chart history)
- `GET /api/bars/:class/:symbol/:tf` — tiered (Redis → QuestDB → Polygon fallback), **L2-LRU cached 1s**, returns up to ~1000 bars with `time` **in seconds**.
  - `class ∈ {stocks, options}`
  - Chart shape: `[{time, open, high, low, close, volume}, ...]`
- `GET /api/replay/:class/:symbol/:tf?from=MS&to=MS&cursor=MS&limit=N` — cursor-paginated deep history for backtesters and chart scroll-back. **Times in ms.** `next_cursor` on the response means more is available.

### Options
- `GET /api/chain/:underlying`       → full chain hash with live last/bid/ask/mid/Greeks
- `GET /api/greeks/:contract`        → single contract IV + Δ/Γ/Θ/ν
- `GET /api/indicators/:class/:symbol/:tf` → SMA20/50/200 + EMA9/21/50 overlays (server-computed)

### Historical archive (for backtesters)
- `GET /api/cold/:table` → list of Parquet day-partitions available on disk. ApexSignals / pattern detectors glob-read `{cold_dir}/{table}/dt=*/*.parquet` directly; Terminal won't usually call this.

---

## 3. WebSocket protocol

### Handshake
Connect to `/ws`. Encoding is negotiated:
- `?format=msgpack` (default if unset) — binary frames, ~4× smaller. Use for the Tauri-native path or the high-fidelity charts.
- `?format=json` — text frames. Used by the shipped `ApexDataProvider` for debuggability in the webview. Swap to msgpack for bandwidth later.

Auth: `?token=<apex-data-api-token>` if the server has `APEX_DATA_API_TOKEN` set.

### Envelope (versioned)
Every outbound frame:
```json
{ "v": 1, "type": "<kind>", "data": { ... } }
```

### Frames

| `type`    | `data` shape | When |
|-----------|-----|------|
| `hello`    | `{v, server, encoding}` | First message after connect |
| `snapshot` | `{subscription, bar: {bar, is_closed}}` | Immediately after a specific `SYM:TF` subscribe; current in-progress bar |
| `bar`      | `{bar, is_closed}` | Every live bar tick or close |
| `trade`    | `{symbol, asset_class, price, qty, time}` | Tape subscribers only |
| `quote`    | `{symbol, asset_class, bid, ask, bid_size, ask_size, spread, time}` | Quote subscribers only |
| `resync`   | `{reason}` | Server evicted us as slow consumer. Reconnect and refetch state. |
| `error`    | `{code, message}` | Soft errors |

`bar` inner object:
```ts
{
  symbol: string
  asset_class: 'stock' | 'option'
  timeframe: '1s' | '1m' | '5m' | '15m' | '30m' | '1h' | '1d'
  time: number    // epoch MS (the Bar payload, not the TerminalBar shape)
  open, high, low, close, volume, vwap: number
  trades: number
  closed: boolean
}
```

> Note the unit split: `GET /api/bars` returns `time` in **seconds** (chart-compatible); WS `bar` frames carry **ms** inside the inner bar object. `ApexDataProvider` normalizes to seconds before forwarding as a `TickData`.

### Client → server messages

```jsonc
{ "subscribe": ["AAPL:1m", "AAPL:5m", "*:15m"] }   // exact or wildcard
{ "tape":      ["AAPL", "MSFT"] }                    // Time & Sales stream
{ "quotes":    ["O:AAPL251219C00200000"] }           // NBBO stream
{ "format":    "msgpack" }                            // switch encoding mid-session
```

Wildcards supported on bar subs: `*:1m` (all symbols one TF), `AAPL:*` (one symbol all TFs), `*` (everything).

Every new `subscribe` replaces the previous set. Send `{"subscribe": []}` to clear.

### Slow-consumer eviction
Each WS client has a 512-frame outbox. If the browser can't drain fast enough (backgrounded tab, slow network, etc.), the server emits `Frame::Resync` and disconnects. The ApexDataProvider reconnects automatically after 2s and resubscribes.

**Terminal responsibility:** when you see a `resync`, invalidate any client-side "is caught up" state and expect a fresh `snapshot` on next subscribe.

---

## 4. Typical flows

### Chart pane — 500 initial bars + live updates
```ts
const hist = await provider.getHistory({ symbol, timeframe, limit: 500 })
// render bars...
provider.subscribe(symbol, timeframe)
const off = provider.onTick((sym, tf, tick) => {
  if (sym === symbol && tf === timeframe) pane.applyTick(tick)
})
// ... later
provider.unsubscribe(symbol, timeframe)
off()
```

### Chart scroll-back — deep history paging
```ts
// user scrolls past oldest bar
const { bars, hasMore } = await provider.getHistory({
  symbol, timeframe, before: oldestBar.time, limit: 500
})
// prepend bars; disable "load more" affordance if !hasMore
```

### Watchlist row (bid/ask/last/day-change)
Don't hit `/api/quote` per row on a timer. Instead:
```ts
// Periodic snapshot (~1s) — L1 JSON with everything the row needs.
const r = await fetch(`${base}/api/snap/stocks/${sym}`, { headers })
const snap = await r.json()
// { last, bid, ask, bid_size, ask_size, spread, day_open, day_high, day_low, day_volume, trades, updated_at_ms }
```
Or (lower latency, higher bandwidth): add `{"quotes": [sym1, sym2, …]}` to your WS session and render quote frames directly.

### Options chain grid
```ts
const r = await fetch(`${base}/api/chain/AAPL`, { headers })
const { underlying, contracts, rows } = await r.json()
// rows[i] = { ticker, underlying, expiry, side: 'C'|'P', strike, last, bid, ask, mid, iv, delta, gamma, theta_per_day, vega_per_pct, updated_at_ms }
```
Poll every 1–2s for a responsive grid, or subscribe to `{"quotes": ["O:AAPL*"]}` for tick-rate updates (heavy — only for the active chain view).

### Order ticket — last + NBBO for instant staging
```ts
const [snap, q] = await Promise.all([
  fetch(`${base}/api/snap/stocks/${sym}`, { headers }).then(r => r.json()),
  fetch(`${base}/api/quote/${sym}`, { headers }).then(r => r.json()),
])
// Seed the ticket. Subscribe to {quotes: [sym]} on the WS for live refresh.
```

---

## 5. Timeframes

The server accepts: `1s`, `1m`, `5m`, `15m`, `30m`, `1h`, `1d` (plus aliases `3m`, `2h`, `4h`, `1w`). Terminal currently surfaces `1m/5m/15m/30m/1h/4h/1d/1wk`. All of those map 1:1 — no conversion needed.

**Note**: `1s` bars exist but are high-frequency; don't offer them in the chart TF picker by default.

---

## 6. Cache & consistency model

- `/api/bars` is L2-cached server-side for **1 second**. Two chart opens of AAPL:1m within a second are free to the backend.
- The live WS `bar` frame for a given `(sym, tf)` and the snapshot returned on subscribe are **authoritative** — trust them over any derived state.
- `/api/snap/:class/:sym` is refreshed per-tick on the server with a 250 ms throttle into Redis. Polling at 1 Hz misses nothing visible to a human.
- Bars that haven't closed carry `is_closed: false` and `trades: 0` at the start of a new bar period. Don't persist these as historical.

---

## 7. Auth

If the server has `APEX_DATA_API_TOKEN` set:
- REST: `Authorization: Bearer <token>` header
- WS: `?token=<token>` query param (browsers can't set headers on upgrade)
- Always public: `/api/health/live`, `/metrics`

The `ApexDataProvider` reads the token from `localStorage['apex-data-token']` or `window.APEX_DATA_TOKEN`. Add a field to your settings panel to let users / ops set it.

---

## 8. Failure modes & how the terminal should react

| Symptom | Server cause | Terminal response |
|---|---|---|
| WS closes + `resync` frame | Slow consumer, evicted | `ApexDataProvider` reconnects after 2s and resubscribes automatically. Chart state: invalidate live bar, let `snapshot` re-seed. |
| WS `close` code 1008 | Auth failure (403) | Surface "auth token invalid" to user; don't retry with same token. |
| `/api/bars` returns `[]` | Symbol unknown OR outside market hours with cold cache | Show "no data yet" overlay rather than spinning. |
| `/api/feeds` shows `circuits.polygon_rest.state == "open"` | Polygon REST is rate-limiting or down | Charts will still live-update from the WS tape. Only deep history / tier-3 fallback is affected. |
| `/api/health/ready` 503 | Redis down, QuestDB down, or no tick <60s | Show degraded-service banner. Bars page may still work from Redis even if QuestDB is offline. |

---

## 9. Performance guidance for the FE

- **Default encoding**: the shipped provider uses JSON (easier to debug in browser devtools). Swap to MessagePack (`?format=msgpack`) once stable — decodes ~4× faster, same API.
- **Don't resubscribe on every render**. The provider keeps one WS open for the whole session; `subscribe/unsubscribe` is idempotent.
- **Use wildcards** for heatmap / market-overview screens — `{"subscribe": ["*:5m"]}` delivers every symbol's 5m bar on one connection.
- **Snapshot-on-subscribe is free** — it's served from the hot Redis key with no round-trip cost. Use it for "open chart → see something immediately" UX rather than waiting on the first live tick.
- **`/api/bars` >> `/api/replay`** for the initial window. Replay is for pagination backwards.

---

## 10. Features the terminal should expose (roadmap)

The service already provides these — FE just needs to wire them:

1. **Server-computed indicators** (`/api/indicators`) — SMA/EMA overlays with no client CPU cost. Show them in chart settings.
2. **Options chain view** (`/api/chain/:underlying`) — a dense grid; Greeks inline; sortable by strike / expiry.
3. **Greeks ribbon on a contract chart** (`/api/greeks/:contract`) — stream IV/delta for a contract alongside its candlesticks.
4. **Watchlist L1** (`/api/snap/...`) — one-shot snapshot per symbol, no per-field round-trips.
5. **Health/feeds status bar** (`/api/health/ready` + `/api/feeds`) — a green/yellow/red pill in the footer with tooltip showing the feed registry. Ops and power users will want this.
6. **Replay mode** for chart playback (`/api/replay/...`) — feed the same `TickData` shape at user-controlled speed for bar-by-bar analysis.
7. **Tape (T&S) pane** — subscribe to `{"tape": [sym]}` and render a running trade tape. Separate stream from bars — doesn't interact with chart subscriptions.

---

## 11. What's not here yet (FE should NOT depend on)

- Level 2 order book — Polygon doesn't expose it via the same feed; plan separately.
- Trade execution endpoints — apex-data is read-only. Orders go through the broker path (existing IBKR/IB server) or a future `apex-orders` service.
- Cross-session state sync — apex-data has no notion of users. Per-user workspace state stays in the terminal's store.

---

## 12. Testing the integration

Run the smoke binary locally before any release:
```bash
apex-data-smoke --url http://apex-data-dev.xllio.com --wait 60
```
Exits 0 if REST + WS + Redis Streams all respond correctly.

For UI testing, point the terminal at a stubbed apex-data: `window.APEX_DATA_URL = 'http://localhost:8410'` and run `cargo run --bin apex-data` in the apex-data repo.
