# apex-terminal — World-Class DOM Trader — Design & Roadmap (2026-07-10)

Goal: make the DOM ladder **function and feel like ThinkorSwim's Active Trader**
as the baseline, then layer on the best of **NinjaTrader SuperDOM**, **Jigsaw**,
and **Bookmap** — and keep it perfectly coordinated with the chart's order lines.

Grounded in a 3-lens survey of the current code (UI, data/feed, order
coordination). TL;DR of the survey: the **order-coordination spine already
exists** (DOM and chart both drive the guarded OrderManager), and the **live
tape stream** makes ~90% of the order-flow tier buildable from data we already
have. This is an additive build, not a rewrite.

---

## 1. Current state (what we have)

**UI** — `ui/panels/dom_panel.rs` (614 L) + `ui/lists/rows/dom_row.rs` (674 L) +
`ui/components/dom_action.rs`. Per-pane `DomPanelState` (gpu.rs:2052). 3 column
modes (Δ / BID / PRICE / ASK / VOL / ORD). Working orders render as B/S+qty chips
in the ORD column; **vertical drag → `modify_order_price`**, hover-X → cancel.
Arm toggle, qty stepper, MKT/LMT, FLATTEN/CANCEL buttons, recenter (double-click
price), resizable, LIVE/SIMULATED badge, mock-depth fallback.

**Data** — `DomLevel { price, bid_size, ask_size, volume, delta }`. Feed
`data/feeds/dom_feed.rs` → `/ws/dom` (ApexIB `reqMktDepth`, **aggregated** level
sizes, 20/side). `DomLevel.volume` is **always 0** (L2 carries no traded volume).
**Live tape** exists: `TapeRow { price, qty, time, is_buy }` (Lee-Ready aggressor
+ off-exchange flag, 500-row FIFO). Best bid/ask derived from the ladder.
Staleness: `last_live_ms` + 2 s → auto LIVE→SIMULATED.

**Coordination (the spine — already solid)** — OrderManager is the single source
of truth. DOM **already** submits via `submit_order` (`OrderSource::DomLadder`),
modifies via `modify_order_price`, cancels via `cancel_order` — the SAME guarded
(paper/risk/kill/journal) path the chart uses and that WS-H just hardened. Both
read `all_order_levels_for(symbol)` → `orders_view()` reconcile each frame, with
the drag snap-back guard. `line_pipeline::LineKind` (Alert/Play/Order/Drawing/
Trigger) already unifies hit-test priorities across chart lines.

---

## 2. Gap analysis vs. world-class

| Area | Have | Missing (to be world-class) |
|---|---|---|
| Click-to-trade | click price → select + LMT, then BUY/SELL button | **direct bid/ask click = instant buy/sell limit** (TOS core feel) |
| Order chips | B/S + qty chip, vertical drag, X-cancel | order-type tag, in-place price/qty, cleaner drag ghost |
| Position | chart-only avg line | **avg-price line + size on the DOM ladder** |
| P&L | none on DOM | **PnL-at-price column** (flatten-here P&L) |
| Volume | `DomLevel.volume` = 0 | **real volume-at-price from the tape** + session profile |
| Order flow | none | **BidVol/AskVol split, cumulative delta, speed-of-tape, pulled/stacked liquidity, big-print flags** |
| Sizing | ± stepper | **quick-size presets, scroll-to-size** |
| Brackets | separate panel | **attach TP/SL (OCO) from the DOM** via `submit_bracket` |
| Context | none | **right-click price → limit/stop/stop-limit/cancel** |
| Buttons | FLATTEN == CANCEL (both cancel-all) | **CANCEL = cancel working orders; FLATTEN = close position** (use WS-H `cancel_all_working` / `flatten_symbol`) |
| Last trade | current-price border | **moving last-trade highlight + size flash** |
| Tape | 500-row buffer, not shown by DOM | **reconstructed T&S sidebar** (buy/sell color, size filter) |
| Safety | LIVE/SIMULATED badge | **stale-book trade guard** (WS-H #48 tie-in), unmistakable LIVE vs PAPER |

---

## 3. Feature matrix (by tier, with source + data-availability)

**Tier A — TOS Active Trader parity (baseline "feel"):** direct bid/ask click
(TOS), quick-size (TOS), position avg line on DOM (TOS), last-trade flash (TOS),
real volume-at-price column from tape (TOS), CANCEL vs FLATTEN split (correctness),
bracket-from-DOM (TOS), right-click context (TOS/NT), recenter/freeze (TOS),
tick-snap chart↔DOM consistency. *Data: all available.*

**Tier B — Order-flow edge (Jigsaw/Sierra/Bookmap differentiator):** BidVol/AskVol
traded split, cumulative delta (session + at-price), speed-of-tape, pulled/stacked
liquidity highlight (size-delta per level), big-print/block flags, reconstructed
tape sidebar. *Data: from the live tape + depth size-deltas — buildable now.
Iceberg/order-count needs a richer feed (flagged).*

**Tier C — NinjaTrader SuperDOM adds:** PnL-at-price column, ATM bracket
templates (one-click entry+stop+target), one-click break-even / trail-stop,
scroll-to-size, notional column. *Data: position + tick math — available.*

**Tier D — Bookmap-class (ambitious):** resting-liquidity heatmap over time,
iceberg/refill detection heuristic, fully configurable columns. *Data: partial;
heatmap needs a depth-history ring; iceberg needs order-count.*

---

## 4. Architecture

**Data model additions (new, tape-derived — no feed change needed for Tier A/B):**
- `DomAnalytics` (per-symbol, updated from the tape ring): `HashMap<price_key,
  { buy_vol, sell_vol, last_ms }>` for volume-at-price + BidVol/AskVol; running
  `cum_delta`; `trades_per_sec` (speed); `last_trade { price, size, up_tick }`.
  Computed incrementally as tape rows arrive (bounded, evicted with the session).
- `DomLevel` gains transient `prev_bid_size/prev_ask_size` (or a parallel
  per-level history) so the renderer can flash **added vs pulled** size.
- Reuse `read_account_data()` for the position (avg_price, qty) → draw on the DOM.

**Rendering:** extend `dom_row.rs` column set (config-driven): add PnL, BidVol/
AskVol, cum-delta, and a session-volume-profile bar sourced from `DomAnalytics`
(not the always-0 `DomLevel.volume`). Position avg line = a highlighted band + a
"POS ±qty @ avg" marker at its row. Last-trade flash = short-lived tint keyed on
`last_trade`. Keep 60 fps (the WS-H perf discipline; reuse `fmt_buf`, no
per-frame allocs in the ladder loop).

**Interaction (all through the existing guarded path):**
- Direct bid/ask click → `submit_order(market_order_intent-style LIMIT at row
  price)` — same `OrderSource::DomLadder` path already wired.
- Right-click → context menu → limit/stop/stop-limit/cancel at that price.
- Bracket toggle → `submit_bracket_order` (TP/SL offsets from the clicked price).
- CANCEL → `cancel_all_working()`; FLATTEN → `flatten_symbol(sym, pos_qty)`
  (the WS-H free fns — paper-guarded, journaled).
- Quick-size / scroll-to-size → set `order_panel.qty`.
- Tick-snap: when a chart order drag lands, snap to the instrument tick so the
  chart line and the DOM row agree to the tick (both already call
  `modify_order_price`; add the snap on the chart side).

**Coordination (extend the existing spine):** position line + alerts optionally
render on the DOM at their row (the `line_pipeline` already models Alert/Order;
a DOM row is just a discrete `py`). A DOM click that creates an order appears as
a chart line the same frame (shared snapshot). Nothing here bypasses OrderManager.

**Safety:** gate order entry when the book is stale (extend the existing 2 s
LIVE/SIMULATED signal into a **trade guard** — refuse/confirm a click on a
SIMULATED/stale book; this is the DOM half of WS-H #48). Make LIVE vs PAPER
unmistakable (WS-H §J).

---

## 5. Phased roadmap (each phase corpus-gated + new DOM scenarios)

- **Phase 1 — TOS parity + coordination polish (the baseline):** direct bid/ask
  click-to-trade; quick-size presets; position avg line + size on the DOM;
  last-trade flash; CANCEL/FLATTEN split via WS-H free fns; real volume-at-price
  from the tape; right-click context menu; recenter/freeze; stale-book trade
  guard; tick-snap. → the DOM now *functions and feels like* TOS Active Trader.
- **Phase 2 — Order-flow edge:** BidVol/AskVol split, cumulative delta,
  speed-of-tape, pulled/stacked-liquidity flash, big-print flags, reconstructed
  tape sidebar. → beyond TOS, into Jigsaw/Sierra territory.
- **Phase 3 — NinjaTrader adds:** PnL-at-price column, ATM bracket templates,
  one-click break-even/trail, scroll-to-size, notional column.
- **Phase 4 — Bookmap-class (ambitious):** resting-liquidity heatmap over time,
  iceberg/refill heuristic, fully configurable columns. (Heatmap + iceberg may
  need a depth-history ring / order-count feed — flagged as feed work.)

**Verification:** every phase builds clean, passes `cargo test`, and passes the
1067-scenario corpus (which already exercises the DOM); each phase adds DOM
scenarios (direct-click order, position line, bracket, stale-guard, order-flow
columns). Perf checked at 60 fps under a synthetic fast-tape.

**Feed follow-ups (not blockers for Phase 1-3):** per-level order count (iceberg,
fragmentation), server-computed volume-at-price (offload the tape aggregation),
depth-history for the heatmap.
