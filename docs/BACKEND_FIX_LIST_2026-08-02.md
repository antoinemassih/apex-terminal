# Backend Fix List — Data Integrity Audit

**Date:** 2026-08-02 (Sunday, market closed)
**Auditor:** apex-terminal client-side trace + live endpoint probe
**Scope:** every data path feeding apex-terminal — stocks, options, orders, signals, indicators
**Goal:** zero fabricated, sampled, or placeholder data reaching a live-money UI

---

## How to reproduce every probe in this document

All ApexData v2 probes go through the LAN ingress with an explicit Host header
(there is no public DNS entry for the v2 dev host — see item **B-7**):

```bash
H="Host: apex-data-v2-dev.xllio.com"
B="http://192.168.1.71"
curl -s -H "$H" "$B/api/health/ready"
```

ApexIB is reachable by hostname:

```bash
curl -s -k https://apexib-dev.xllio.com/health
```

---

## Severity key

| | Meaning |
|---|---|
| **P0** | Live-money risk or silently-fabricated data reaching the UI. Fix now. |
| **P1** | Real feature is dark, or a health signal lies. Fix this week. |
| **P2** | Degraded / partial data. Fix when the P0-P1 queue clears. |
| **P3** | Missing capability, not a correctness defect. |

---

# ✅ What is confirmed REAL (no action needed)

Verified live on 2026-08-02. Recorded here so nobody "fixes" a working path.

| Surface | Endpoint | Evidence |
|---|---|---|
| Stock snapshots | `/api/snap/stocks/SPY` | `source:"polygon"`, last 747.03, H 748.895, L 737.68, O 744.68 |
| Intraday bars | `/api/bars/stocks/SPY/1m` | 97,057 B |
| Daily bars | `/api/bars/stocks/SPY/1d` | 158,656 B |
| **Indicators** | `/api/indicators/stocks/SPY/5m` | 22 real values: rsi14 37.65, macd −0.1532, macd_hist 0.0376, adx14 34.76, atr14 0.2461, di+ 12.19, di− 32.32, obv −5,956,940, mfi14 41.73, bb_upper/mid/lower, sma20/50/200, ema9/21/50, stoch_d 42.03, rvol 2.10 |
| Options chain | `/api/chain/SPY?dte_max=7` | 1676 rows, 750 KB, 0.58 s |
| Chain NBBO | same | bid 0.07 / ask 0.08 / mid 0.075 — 1288/1676 rows with bid > 0 |
| Chain greeks | same | delta −0.0116, gamma 0.00167, theta −0.0754, vega 0.0365, iv 0.2440 — 1487/1676 rows |
| Expected move | `/api/expected_move/SPY` | atm_strike 747, straddle_mid 4.16, EM 0.557%, expiry 2026-08-03 |
| Symbol search | `/api/search?q=AAPL` | 2,815 B |
| Bulk snapshots | `/api/stocks/snap/bulk?tickers=SPY,QQQ` | 1,782 B |

**Important:** chain greeks and `/api/indicators` are computed **server-side from
real market data**. The terminal does *not* locally model these. Any claim that
"the terminal's greeks are Black-Scholes" is false for this path — see **B-6**
for the one place a local Black-Scholes model does exist.

---

# ❌ BACKEND-OWNED DEFECTS

## B-1 — **P0** — IB is not connected; orders / positions / account are dead

**Service:** ApexIB (`apexib-dev.xllio.com`)

```bash
$ curl -s -k https://apexib-dev.xllio.com/health
{"status":"ok","role":"trading",
 "brokers":[{"name":"ib","enabled":true,
             "connected":false,          ← IB not connected
             "authenticated":false,
             "account_id":null,
             "last_error":null,          ← no error recorded either
             "last_heartbeat":null,
             "reconnect_count":0}],      ← never even retried
 "ib":{"ibConnected":false,"accountId":null,
       "circuitBreaker":{"tripped":false,"manuallyHalted":false}}}
```

Three endpoints do not answer at all — connection hangs to timeout:

```
/orders        http=000   (hang)
/positions     http=000   (hang)
/contract/SPY  http=000   (hang)
/account       http=404
```

**Impact:** the entire trading surface — order entry, working orders, positions,
P&L, buying power, contract resolution — has no real data behind it. In a
live-money application this is the single highest-risk finding.

**Note two separate bugs here:**
1. IB itself is disconnected, with `last_error: null` and `reconnect_count: 0` —
   meaning nothing tried to reconnect and nothing recorded why.
2. `/health` returns **`"status":"ok"`** while `connected:false`. The top-level
   status does not reflect broker connectivity, so any monitor watching
   `status` sees green. This is the same lie as **B-2**.

**Asks:**
- Reconnect IB / IB Gateway and confirm `account_id` populates.
- Make `reconnect_count` actually increment — a stuck-at-0 counter means the
  reconnect loop is not running.
- Populate `last_error` on failure. A null error with a false connection is
  unactionable.
- `status` must go non-`ok` when any enabled broker is disconnected.
- Find out why `/orders`, `/positions`, `/contract/{sym}` hang rather than
  returning 5xx. A hang is worse than an error — the client blocks.

---

## B-2 — **P0** — `/api/health/ready` reports `ready: true` with zero feeds connected

**Service:** ApexData v2

```bash
$ curl -s -H "$H" "$B/api/health/ready"
{"feeds_connected":0,           ← nothing connected
 "feeds_total":0,               ← nothing even registered
 "questdb":true,"redis":true,
 "ready":true,                  ← ships green regardless
 "tick_age_ms":9223372036854775807,   ← i64::MAX — no tick, ever
 "tick_fresh":false}
```

`tick_age_ms` is `i64::MAX`, i.e. the sentinel for "never received a tick", not
a large-but-real age.

Market is closed, so *quiet* feeds are expected — but `feeds_total: 0` means no
feed is **registered**, which is a different condition from "registered and
idle". A closed market should still show N registered feeds with a stale age.

**Impact:** any liveness check keying on `ready` passes while the platform has
no streaming data at all. This is the exact failure class that hid a dead
options feed for ~90 days previously.

**Asks:**
- `ready` must be false when `feeds_total == 0`, and ideally when
  `feeds_connected == 0`.
- Distinguish "no feed registered" from "feed registered, market closed".
- Consider emitting `null` rather than `i64::MAX` for a never-seen tick, so
  clients don't accidentally arithmetic on the sentinel.

---

## B-3 — **P1** — `/api/feeds` returns an empty object, so the documented health check is unimplementable

**Service:** ApexData v2

```bash
$ curl -s -H "$H" "$B/api/feeds"
{"circuits":{"polygon_rest":{"state":"closed","failures":0,
              "successes_total":4654,"failures_total":4,
              "opens_total":0,"rejections_total":0}},
 "feeds":{}}                    ← empty
```

`OPTIONS_CHAIN_GUIDE.md` explicitly instructs clients:

> *"For feed health use `GET /api/feeds` and check message age, not `connected`."*

With `feeds: {}` there is nothing to check the age of, so the prescribed check
cannot be written. The terminal currently works around this by deriving feed age
client-side from chain-row `updated_at_ms`, which is a proxy, not a health signal.

The `circuits` block *is* populated and useful (polygon_rest healthy, 4654
successes / 4 failures) — so the endpoint works, the `feeds` map specifically is
not being filled.

**Ask:** populate `feeds` with per-feed `{name, connected, last_message_ms,
message_count}`, or amend the guide to stop prescribing a check that cannot be
performed.

---

## B-4 — **P1** — Open interest is never available from any route

**Service:** ApexData v2

- `oi` is `null` on **0 of 1676** chain rows — not sparse, entirely absent.
- `volume` is likewise `null` on all rows.
- The dedicated route 404s:

```
/api/oi/O:SPY260805P00708000    http=404
```

**Impact:** OI and volume are load-bearing for options analysis (liquidity
screening, unusual-activity detection, strike selection). Their absence also
forces the terminal's placeholder path to *fabricate* OI via `sim_oi()` — a
synthetic value that then reaches the grid (see **B-6**).

**Ask:** populate `oi` / `volume` on chain rows, or restore `/api/oi/{contract}`.
If Polygon entitlement is the blocker, say so explicitly so the client can hide
the columns rather than render fabricated zeros.

---

## B-5 — **P1** — Documented routes return 404 on v2

**Service:** ApexData v2

| Route | Status | Notes |
|---|---|---|
| `/api/greeks/{contract}` | **404** | Listed in `OPTIONS_CHAIN_GUIDE.md` |
| `/api/oi/{contract}` | **404** | Listed in the guide; see **B-4** |
| `/api/price/{symbol}` | **404** | Referenced by client code |
| `/api/iv_rank/{symbol}` | **404** | Client has `get_iv_rank()` calling this |
| `/api/quote/{symbol}` | **404** | |
| `/api/trades/{symbol}` | **404** | |
| `/api/dom/{symbol}` | **404** | Blocks the DOM ladder — see **C-2** |

`/api/greeks` is currently harmless in practice because greeks arrive on the
chain rows, **but** the terminal runs a 2-second poller against it that 404s
forever and swallows the error silently (`live_state.rs:286`). That is wasted
load on your gateway, ~30 req/min per running client.

`/api/iv_rank` 404 means the IV-rank widget has no source.

**Ask:** for each route — restore it, or confirm it is retired so the client can
delete the caller. A documented route that 404s is worse than an undocumented
one, because clients are written against the doc.

---

## B-6 — **P2** — Chain fallback fabricates a full Black-Scholes chain when ApexData is unreachable

**Service:** ApexData v2 (availability); client behaviour documented for context

This is client-side, but it is *driven* by backend availability, so the backend
team should know it exists.

When `/api/chain` fails, apex-terminal synthesizes a complete option chain
locally — `bs_price()` in `compute.rs:393`, reached via `build_chain()` at
`fetch.rs:14`, from two fallback sites (`fetch.rs:685`, `fetch.rs:879`). It
fabricates bid/ask (Black-Scholes), IV (`get_iv()`), and OI (`sim_oi()`).

It is flagged `placeholder: true` and the UI paints a `PLACEHOLDER DATA — real
chain unavailable` strip — but the numbers are entirely plausible-looking, and
there is deliberately **no toast** (`fetch.rs:682`, suppressed because it spammed
on panel re-entry).

**Why this matters to you:** every minute ApexData is unreachable, traders are
looking at a Black-Scholes chain with invented open interest. Chain availability
is therefore a correctness issue, not just an uptime one.

**Ask:** none directly — this is on the client to make louder (see **C-1**). Just
be aware that chain downtime degrades to *fabricated data*, not to an empty panel.

---

## B-7 — **P1** — No DNS / proxy entry for `apex-data-v2-dev.xllio.com`

Every probe in this document needs an explicit `Host:` header against
`192.168.1.71` because the hostname does not resolve off-LAN. The Kubernetes
side is correct (the Traefik `IngressRoute` CRD exists and has been serving for
~49 days) — the gap is the NPM / openresty proxy-host entry at 192.168.1.144.

Per the split-horizon runbook, a new `*.xllio.com` host must also set
`cert_id 9` + `ssl_forced`, or off-LAN HTTPS hangs rather than failing fast.

**Ask:** add the NPM proxy-host entry for `apex-data-v2-dev.xllio.com`.

---

## B-8 — **P1** — apex-signals is up but the terminal points at `localhost:8100`

**Service:** apex-signals

```
apex-signals-dev.xllio.com    http=200    ← service is healthy
apex-signals.xllio.com        http=404
localhost:8100                http=000    ← what the client defaults to
```

Three panels — MSG RRG, MSG Influence, MSG Tension — call
`apex_signals_http()`, which reads `APEX_SIGNALS_HTTP` and falls back to
`http://localhost:8100`. That env var is unset, so all three panels are dark
against a service that is actually running.

**Ask (backend/infra):** confirm `apex-signals-dev.xllio.com` is the intended
stable hostname and that `prod` is meant to 404. Client fix tracked as **C-3**.

---

## B-9 — **P2** — GEX / gamma feed on `:8412` is unreachable

**Service:** gamma_feed_service / ApexSignals GEX

```
localhost:8412       http=000
127.0.0.1:8412       http=000
```

With the feed absent, the terminal synthesizes gamma levels
(`gpu.rs:2734` sets `gamma_synthetic = true`) and paints a `SYNTHETIC` badge.
So every gamma wall, call wall, put wall, gamma-zero and HVL currently on screen
is fabricated.

**Ask:** stand up the GEX feed, or publish its real endpoint so the client can be
pointed at it (same shape as **B-8** — it may simply be running elsewhere).

---

# ⚠️ CLIENT-OWNED DEFECTS (apex-terminal — tracked here so nothing is lost)

These are **not** backend work. Listed for completeness; the terminal team owns them.

## C-1 — **P0** — Scanner fabricates RVOL from a symbol hash, **undisclosed**

`scanner_panel.rs:426`, in the "save as watchlist" path:

```rust
let sym_hash = r.symbol.bytes().fold(0u32, |a,b| a.wrapping_mul(31).wrapping_add(b as u32));
let rvol_seed = 0.5 + (sym_hash % 40) as f32 * 0.1;
...
rvol: rvol_seed,          // fabricated from the ticker's letters
atr: 0.0,                 // hardcoded
avg_daily_range: 2.0,     // hardcoded
high_52wk: 0.0, low_52wk: 0.0,   // hardcoded
```

**This is the most dangerous finding in the audit.** Unlike every other fake in
the codebase it carries **no badge**, it is **persisted to disk** via
`watchlist.persist()`, and it is indistinguishable from measured data on reload.
A trader could screen on an RVOL that is a hash of the ticker's letters.

Fix: source real RVOL, or omit the field and let the column render "—".

## C-2 — **P1** — DOM ladder is always mock

`generate_mock_levels()` is called from `commands.rs:1168` and `core.rs:1561`.
Badged `SIMULATED` when not live — but with `/api/dom` 404 (**B-5**) and no
`/ws/dom` frames, it is *never* live, so the DOM is permanently fabricated.

## C-3 — **P1** — Set `APEX_SIGNALS_HTTP`

One-line config change to point the three MSG panels at
`apex-signals-dev.xllio.com` instead of dead `localhost:8100`. Unlocks three
finished panels for free. See **B-8**.

## C-4 — **P1** — Retire or gate the dead `/api/greeks` poller

`live_state.rs:286` polls every 2 s and silently swallows the 404 (**B-5**).
Either delete it (greeks arrive on chain rows) or gate it behind a capability
probe.

## C-5 — **P2** — Backtest results are entirely fabricated

`script_panel.rs:156` — `mock_backtest()` is a seed=42 LCG that **never reads the
script source**. Every figure (P&L, win rate, profit factor, max drawdown,
Sharpe) is invented. Called from `script_panel.rs:292`.

Badged `SIMULATED — mock data, not a real backtest`, so it is disclosed — but a
backtest panel that ignores your strategy has negative value. Recommend hiding
the panel behind a feature flag until a real engine exists.

---

# ROUND 2 — apex-signals, futures, crypto, projector routes

Added after the initial sweep. These widen the audit past SPY/equities.

## B-10 — **P0** — apex-signals is `degraded`: Redis is erroring, and Redis *is* the delivery layer

```bash
$ curl -s -H "Host: apex-signals-dev.xllio.com" http://192.168.1.71/health
{"postgres":"ok","redis":"error","service":"apex-signals",
 "status":"degraded","version":"0.1.0"}
```

This is not a peripheral dependency. `GET /engines` shows the engines publish
**exclusively over Redis pub/sub channels**:

```
apex_read              -> APEX_READ:{SYM}          master read — direction/conviction/confluence
signal_combiner        -> COMBINED:{SYM}           26-engine weighted conviction
trade_plan             -> TRADE_PLAN:{SYM}         entry/target/stop + sizing
regime_router          -> REGIME                   canonical regime label
regime_feed            -> REGIME_FEED:{SYM}        gamma flip/walls + flow
triangulation          -> TRIANGULATION:{SYM}      key-level confluence
smart_money_composite  -> SMART_MONEY_COMPOSITE:{SYM}
exit_gauge             -> EXIT_GAUGE:{SYM}         hold/trim/exit
setup_radar            -> SETUP_RADAR:{SYM}        squeeze vs trap
vol_desk               -> VOL_DESK:{SYM}
macro_tape             -> MACRO_TAPE
```

`total_engines: 256`. With Redis erroring, **the output of all 256 engines is
undeliverable** — every signal, trade plan, regime label and conviction score.

**Diagnostic already done for you:** Redis TCP is reachable from the LAN —
`192.168.1.80:6379` accepts connections. So this is **not** a network partition;
it is auth, DB-index, or client-config inside apex-signals. Check credentials
against the `infra-credentials` secret.

## B-11 — **P0** — `GET /gamma/{sym}` returns HTTP 500 "broken pipe"

```bash
$ curl -s -H "Host: apex-signals-dev.xllio.com" http://192.168.1.71/gamma/SPY
{"error":true,"message":"broken pipe"}      # http=500
```

Same for QQQ. This is the **real GEX source** referenced by `regime_feed`
("gamma flip/walls + flow — also `GET /gamma/:sym`"). Almost certainly a
downstream symptom of **B-10** (Redis read failing mid-response).

Combined with **B-9** (`:8412` unreachable), there is now **no working source of
gamma data at all**, which is why the terminal falls back to fabricated gamma
walls with a `SYNTHETIC` badge.

## B-12 — **P1** — Wave-10 projector routes were never wired (breadth / rotation / movers)

All 404 on v2:

```
/api/stocks/sector_rotation    404
/api/stocks/breadth/us         404
/api/stocks/breadth/spx        404
/api/stocks/movers/gainers     404
/api/stocks/movers/losers      404
```

There is an unresolved `TODO(wire-route)` in the client at
`data/feeds/apex_data/rest.rs:865` asking exactly this question — it was never
answered, so the routes were coded against speculatively.

**Impact:** the heatmap pane calls `live_state::get_breadth("us")` and
`get_sector_rotation()` (`heatmap_pane.rs:95-96`). Both return `None` forever, so
market breadth and the 11-sector SPDR rotation view are permanently empty.

**Ask:** confirm the real paths, or confirm the Redis-bridge design the TODO
mentions (`projector:sector_rotation`, `projector:breadth:<idx>`) — note that
bridge would also be blocked by **B-10**.

## B-13 — **P1** — Futures snapshots take exactly 10 s; futures bars 502

```
/api/snap/stocks/SPY      200   t=0.75 s
/api/snap/futures/ES      200   t=10.02 s      ← 13× slower, suspiciously exact
/api/snap/futures/NQ      200   t=10.03 s
/api/bars/futures/ES/1m   502   t=10.02 s
```

A consistent 10.0 s is a hard timeout, not load. The payload confirms the cause:

```json
{"symbol":"ES","last":7519.25,"prev_close":7472.5,"change_abs":46.75,
 "source":"ib",        ← futures come from IB
 "stale":true,         ← correctly flagged
 "ts_ms":1785626773615}   ← ~8.5 h old
```

**Root cause is B-1.** Futures are sourced from IB; IB is disconnected; the
request blocks on a 10 s IB timeout, then serves stale cache (snap) or fails
(bars). Fixing B-1 should fix this.

Credit where due: `stale: true` is set correctly and the client can act on it —
this is the *right* pattern, and the opposite of **B-2**.

**Ask:** independently of B-1, drop the IB timeout to ~1–2 s and serve cache
immediately. A 10 s blocking call in a trading UI is unacceptable even when the
data is ultimately correct.

## B-14 — **P2** — Crypto has no working route

```
/api/snap/crypto/X:BTCUSD      404
/api/bars/crypto/X:BTCUSD/1m   400
```

Consistent with the prior finding that QuestDB crypto history is broken and only
ephemeral data exists. **Ask:** confirm crypto is out of scope for now so the
client can hide the asset class rather than render errors.

## B-15 — **P2** — Engine classification is static and 8 weeks stale

`GET /engines` self-declares:

> `"classification": "static (2026-06-14); reflects data-source wiring, not live last-emit"`

So the registry cannot tell anyone which engines are *actually emitting* — only
which were wired up on 2026-06-14. `live` lists 4 data classes
(`bar, chain, quotes, tape`) and `dormant` 3 (`gated, multi_symbol, needs_feed`),
but none of it is measured.

The guidance field even says: *"Treat dormant channels as 'no data yet', not
errors"* — which means a genuinely broken engine is indistinguishable from an
idle one. This is the same fail-silent class as **B-2**.

**Ask:** emit a real `last_emit_ms` per channel so consumers can tell dead from idle.

---

# Priority queue

| # | Item | Owner | Sev |
|---|---|---|---|
| 1 | **B-1** IB disconnected; orders/positions hang; also causes B-13 | ApexIB | P0 |
| 2 | **B-10** apex-signals Redis error → 256 engines undeliverable | apex-signals | P0 |
| 3 | **C-1** Scanner fake RVOL, undisclosed + persisted | terminal | P0 |
| 4 | **B-2** `ready:true` with 0 feeds | ApexData | P0 |
| 5 | **B-11** `/gamma/{sym}` 500 broken pipe | apex-signals | P0 |
| 6 | **B-3** `/api/feeds` empty | ApexData | P1 |
| 7 | **B-5** 7 documented routes 404 | ApexData | P1 |
| 8 | **B-12** projector routes never wired (breadth/rotation/movers) | ApexData | P1 |
| 9 | **B-13** futures 10 s timeout / bars 502 | ApexData | P1 |
| 10 | **B-4** OI / volume never available | ApexData | P1 |
| 11 | **B-7** NPM entry for v2 host | infra | P1 |
| 12 | **C-3** Set `APEX_SIGNALS_HTTP` (blocked on B-10) | terminal | P1 |
| 13 | **C-4** retire dead greeks poller | terminal | P1 |
| 14 | **B-15** engine classification static since 2026-06-14 | apex-signals | P2 |
| 15 | **B-9** GEX `:8412` down (see also B-11) | signals | P2 |
| 16 | **B-14** crypto routes absent | ApexData | P2 |
| 17 | **C-2** DOM always mock (blocked on B-5 `/api/dom`) | terminal | P2 |
| 18 | **C-5** backtest engine | terminal | P2 |

## Dependency notes for sequencing

- **B-1 → B-13.** Fix IB first; futures latency and bars-502 likely resolve with it.
- **B-10 → B-11, B-12, C-3.** Fix apex-signals' Redis first; the gamma 500, the
  projector Redis-bridge option, and the client's `APEX_SIGNALS_HTTP` change are
  all downstream of it. Pointing the client at a degraded service gains nothing.
- **B-5 → C-2.** The DOM cannot stop being mock until `/api/dom` exists.

---

# Verification gate

Before any of this is called done, these must all hold **during RTH**:

```bash
# 1. Feeds registered and fresh
curl -s -H "$H" "$B/api/health/ready" | jq '.feeds_total > 0 and .tick_fresh == true'

# 2. Feed health is inspectable
curl -s -H "$H" "$B/api/feeds" | jq '.feeds | length > 0'

# 3. Broker connected
curl -s -k https://apexib-dev.xllio.com/health | jq '.brokers[0].connected == true'

# 4. Positions answer at all
curl -s -k -o /dev/null -w '%{http_code}\n' https://apexib-dev.xllio.com/positions

# 5. OI present on chain rows
curl -s -H "$H" "$B/api/chain/SPY?dte_max=7" | jq '[.rows[] | select(.oi != null)] | length'
```

**Nothing in this list is verified until it is checked with the market open.**
This audit ran on a Sunday; absence of streaming data is expected today, but
`feeds_total: 0` and `ib.connected: false` are structural and will not fix
themselves at 09:30.
