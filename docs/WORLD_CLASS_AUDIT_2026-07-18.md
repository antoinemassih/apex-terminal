# Apex Terminal — World-Class Product Audit
**2026-07-18 · 17 dimensions · 77 agents · 93 adversarially-verified claims**

## Method

Seventeen Sonnet digger agents, one per product dimension, each with a scoped brief,
hard evidence discipline (file:line for every claim, REAL/PARTIAL/SCAFFOLD/MOCK
classification for every feature touched), and a standing warning about this
codebase's dominant defect class: **partial implementations that read as complete**.
Every P0/P1 claim was then re-checked by an independent adversarial verifier whose
default posture was to refute it. Results: **64 CONFIRMED, 26 ADJUSTED (evidence or
severity corrected), 3 REFUTED**. Severities below are **post-verification**.
Refuted claims are listed in §8 — they do not appear in the ledgers.

Full per-dimension detail (verdicts, strengths, all findings with evidence and
verification notes): `WORLD_CLASS_AUDIT_2026-07-18_APPENDIX.md`.

---

## 1. Executive verdict

Apex Terminal is **a genuinely capable native GPU trading engine wearing the
scaffolding of a much bigger product**. The parts that touch real money at the
core — the order state machine, WAL crash recovery, the reconcile loop, the
transport/resilience layer, the GPU chart pipeline — are unusually mature, in
places better-engineered than what retail platforms ship. The parts that
surround them fall into three failure classes, in order of danger:

1. **Live-money edges that are still soft** (§4): a Spread Builder that submits
   live combo orders at $0 limit with conId=0 past every risk gate; broker
   cancel/modify calls that read rejection as success; a daily-loss breaker that
   resets to zero on restart; paper trading that never fills anything.
2. **Convincing UI over dead wires** (§4, §5): a welcome wizard that captures
   risk limits and discards them; a hotkey editor that remaps nothing; alerts
   that show ACTIVE after they've silently died; drawings that silently stop
   persisting; a footprint chart that fabricates order-flow from bar shape.
3. **The differentiator does not exist** (§6): the product's stated identity —
   free, extensible, vibe-codeable — has zero scripting runtime, a closed
   19-member indicator enum requiring ~140 edit sites to extend, and its one
   real automation API compiled out of release builds.

None of this is fatal. The audit's biggest positive surprise is how much
**honest infrastructure** already exists to build on: the LIVE/SIMULATED badge
discipline, fetch-real-first-badge-sample-on-fallback panels, a real command
palette, a real design system at ~82% adoption, a 1,067-scenario behavioral
corpus, and working extension seams (provider trait, AppCommand bus, expression
evaluator) that prove the team knows the right patterns. The path to world-class
is not a rewrite — it is: **harden the money edges (weeks), wire the dead UI
(weeks), then build the scripting layer on seams that already exist (the one
XL investment that creates the moat)**.

### Scorecard

| Dimension | Distance | One-line verdict |
|---|---|---|
| Trading core | FAR | Order state machine excellent; paper trading is a stub, 4 verified P0 money edges |
| Data pipeline | FAR | Transport layer world-class; gap-fill is dead code, caches are ageless |
| Charting engine | FAR | Rendering NEAR world-class; drawings silently don't persist, replay stub |
| DOM & order-flow | FAR | Tape signals real & honest; footprint fabricates, no depth history |
| UI & design system | **NEAR** | Real system, ~82% adoption; both quality gates currently dead |
| UX & usability | **NEAR** | Real wizard/palette/hotkey UI; several are shells over unread state |
| Extensibility & scripting | FAR | The differentiator: zero runtime, but the seams to build on exist |
| Features vs competitors | FAR | Portfolio/Journal/News/GEX real (some ahead of rivals); options chain fragmented; Spread Builder dangerous |
| Quality & testing | FAR | 769 tests + corpus + ratchets; corpus oracles vacuous when data absent, CI runs ~30% |
| Performance | **NEAR** | GPU pipeline matches its docs; DOM handler not frame-coalesced; benchmark stale |
| Persistence & state | **NEAR** | Order WAL excellent; drawing/watchlist stores silently lossy |
| Product finish | FAR | Honest-labeling discipline is real; a sweep of silent no-ops remains |
| Safety & security | FAR | Keychain/TLS/kill-switch solid; live-order path hardcoded + unauthenticated |
| OSS readiness | **ABSENT** | 1-line README, no LICENSE, CI has never built the shipping binary |
| Persona: day trader | **NEAR** | Mechanics trustworthy; alert delivery gap is the disqualifier |
| Persona: contributor | FAR | Core-team-only today; indicator trait is the tractable first boundary |
| Architecture | FAR | Domain model trapped in gpu.rs (246-field Chart struct); real seams exist |

---

## 2. What is already world-class — protect these

- **Order persistence & recovery**: fsync'd rotating WAL, orphan detection,
  fail-closed kill-switch restore on corrupt flags, shutdown marker handling the
  "statics don't Drop" gotcha, PendingSubmit→Unknown demotion on restart with a
  user toast. This is the best subsystem in the codebase.
- **Transport resilience**: jittered backoff, per-route circuit breakers,
  `resilient_ws`, subscription dedup/refcount — heavily unit-tested.
- **GPU chart pipeline**: pre-sized buffers, reserved per-frame Vecs, bounded
  caches with eviction tests; matches its own perf report.
- **Honesty patterns**: DOM LIVE/SIMULATED badge + stale-book trade guard;
  msg_* panels fetch-real-first with badged SAMPLE fallback; chart widgets that
  say "not connected" instead of fabricating. **This is the pattern every
  remaining silent no-op must adopt.**
- **The corpus + quality ratchets**: 1,067 behavioral scenarios driving a real
  window, file-size/unwrap/dead-code/bus-bypass ratchets in CI, and
  `PANE_RS_SPLIT_PLAN.md`'s benchmark-gated refusal to split the hot render fn —
  the right way to say no to a refactor.
- **Working extension seams**: `MarketDataProvider` trait + registry (mock,
  replay, fallback, IB, apex_data implementations), the 128-variant AppCommand
  bus, the playbook expression evaluator, the `ComponentTheme` dependency
  inversion. The scripting layer builds on these, not from scratch.
- **UX infrastructure**: 4-step welcome wizard, 1,291-line fuzzy command palette
  with chaining, hotkey-remap UI with live key capture, unified errors_sink→toast
  pipeline, real multi-window. These are shells to *wire*, not rebuild.

---

## 3. The P0 ledger — verified live-money & deception issues

Every item below survived adversarial verification (or was adjusted **up** to P0
by the verifier). Effort: S=hours, M=days, L=week+, XL=multi-week.

### Money edges
| # | Finding | Effort | Evidence anchor |
|---|---|---|---|
| 1 | **Spread Builder submits live combo orders with conId=0 and $0.00 limit**, bypassing fat-finger and notional gates | M | spread_panel → submit_combo path |
| 2 | **LiveBroker cancel/modify never check HTTP status** — broker rejection reads as success; compounds with optimistic local Cancelled | S | broker.rs:380-408 `.map(\|_\|())` |
| 3 | **Optimistic Cancelled + absorbing terminal-state rule can permanently mask a real fill** | M | order_manager.rs ~2453 + reconcile rules |
| 4 | **Daily-loss circuit breaker resets to $0 on every restart** — fields never persisted; restart clears the day's loss | M | new()/load_from_disk/save_to_disk omit realized_pnl_today |
| 5 | **Paper trading is an ACK-only stub** — no fills, no PnL, no positions; paper panels show the real IB account | XL | paper.rs (19 lines) |
| 6 | **Paper→Live is a single unconfirmed toggle click** (P1, listed here for adjacency) | S | settings_panel.rs:700-704 |

### Deception class (UI that lies)
| # | Finding | Effort | Evidence anchor |
|---|---|---|---|
| 7 | **Footprint chart fabricates per-price buy/sell volume from OHLC bar shape**, not tape — renders "ABSORPTION"/"3:1 BUY @ price" tags from a hardcoded Gaussian + `0.3 + 0.5*level_pos` formula, zero disclosure | M | gpu.rs:4330-4379 `bar_micro_profile`; core.rs:12308-12470 |
| 7b | **Historical CVD is silently backfilled with the same fabricated heuristic** — `realized_delta_in()` only covers trades since process start (~1 session); older bars fall back to the close-position formula, and both segments render as ONE continuous line with no visual break. A multi-day CVD divergence is mostly synthetic and unlabeled | S-M | gpu.rs:4398-4422; core.rs:5837, 12629-12660 *(second-opinion agent)* |
| 8 | **Welcome-wizard risk limits (daily loss cap, max position %) are captured then silently discarded** — never reach the risk gate | S | wizard Finish handler |
| 9 | **Hotkey editor is cosmetic for ~26 of 27 bindings** — remapping changes nothing; only the boss key dispatches from state | M | keyboard_shortcuts.rs vs watchlist.hotkeys |
| 10 | **Price alerts silently die — still shown ACTIVE — when the owning pane switches symbol** | M | alert evaluation is pane-symbol-coupled |
| 11 | **Alerts have zero out-of-app delivery; "sound" is a literal eprintln!** | M | alert fire path |
| 12 | **Drawing persistence silently drops ~16 of 22 drawing kinds** — two non-isomorphic DrawingKind enums, string-mapping mismatch at the DB layer, dead-lettered forever | M | drawing_db.rs:488 parse_kind |
| 13 | **Drawing persistence silently degrades to session-only when Postgres is unreachable at startup** — eprintln only, no UI signal, no reconnect | M | drawing_db connect path |
| 14 | **Watchlist save is destructive delete-and-reinsert with a phantom fallback** — code comment promises a Postgres cross-machine fallback that doesn't exist; a new machine loads empty defaults and the first save wipes the server copy | M | load_watchlists / save path |
| 15 | **Command-palette destructive account-wide actions execute on a single Enter, zero confirmation** | S | palette action dispatch |
| 16 | **Corpus invariant oracles pass vacuously when a pane never loads bars** — the 2026-07-17 GREEN run rode through a real feed outage undetected | S | assert_engine.rs L915-976 |
| 17 | **Gap-fill-on-reconnect is architecturally dead** — replayed bars go into a broadcast channel with zero live receivers | M | providers/mod.rs:20-28 admits it |
| 18 | **Replay's "overlay on live chart" checkbox is fully non-functional** *(unverified — the one P0 that missed the verify wave; the render pipeline for it exists in core.rs, so wiring is tractable)* | M | replay_pane.rs |

### Public-release blocker (P0 for the product vision, not for today's owner-use)
| # | Finding | Effort |
|---|---|---|
| 19 | **Live order/cancel/modify + kill-switch path is hardcoded at compile time to a private dev domain with zero authentication** (`APEXIB_URL` const). Must become runtime-required URL + bearer token before any second user exists | L |

---

## 4. The P1 ledger — daily-driver & world-class blockers

**Trading**: reconcile/account polling bypasses the Broker trait (IB-hardcoded HTTP);
bracket TP/SL legs pair-link only to entry — no local defense if broker OCA breaks;
realized PnL uses most-recent-fill cost basis, not FIFO (feeds the daily-loss gate).

**Data**: the live quote cache has **no timestamp at all** and silently drives the
DOM ladder, watchlist, and spread panel (closes with known-open #48 — do together);
the order-entry panel's compact DOM ladder **fabricates book depth** beyond
top-of-book; provider pluggability for anyone off the owner's LAN is effectively
absent (though a 5-tier fallback ending at Yahoo does serve historical bars).

**Charting**: alerts-on-drawings math unwired (toggle is a fire-and-forget curl);
adding one indicator touches ~140 sites across 6+ files including both god files;
incremental indicator updates silently NaN any indicator not special-cased.

**DOM**: volume/absorption signals computed over last ~100 trades, not the session;
DOM bracket hardcoded 10t/20t while a real user-editable `BracketTemplate` system
(Tight/Normal/Wide/Scalp) already exists and is wired into the chart context menu
(trading/mod.rs:737-741, pane_context_menu.rs:159-166) — the DOM just never reads
it, so this fix is S-M not a build; **the DOM cannot place Stop/Stop-Limit orders
at all** (`enum DomOrderType { Market, Limit }`, dom_panel.rs:32) despite
OrderManager fully supporting them — a TOS/SuperDOM table-stakes gap; no
move-to-breakeven or one-click-trail UI anywhere despite `trail_amount`/
`trail_percent` being first-class OrderManager fields; no depth history (the
Bookmap heatmap gap — needs a DomHistory ring, currently the book is
frame-overwritten); iceberg detection is structurally impossible on IBKR's
aggregated MBP feed — needs Databento/Rithmic/dxFeed as an alternate DOM feed,
or should be cut from the roadmap rather than left perpetually "flagged".

**UX**: Postgres failure at startup invisible in release builds; palette/wizard
findings above.

**Quality**: CI executes ~30% of the unit suite; the corpus never runs in CI on
any schedule; 17 of 20 indicator compute fns have no golden-value oracle.

**Performance**: DOM handler recomputes full order-flow analytics per WS message
instead of coalescing to frame cadence — the one hot path that scales with market
speed, not frame budget; cold-start blocks the render thread on a Postgres
connect before first frame.

**Extensibility**: playbook sharing (publish/feed/fork/Discord) has a complete
backend and **zero reachable UI**, compiled out of release; dev_inspector (the
real automation API) compiled out of release; indicators are a closed enum.

**Architecture**: domain model (246-field Chart, 148-field Watchlist,
IndicatorType) trapped inside gpu.rs; AppCommand adoption shallow (266 direct
mutations baselined; watchlist_panel.rs alone has 38 with zero bus use);
chart↔data circular dependency (2/11 feed files still read renderer config
directly) blocks any crate split.

**OSS**: README is one line; no LICENSE anywhere; CI has never compiled
`apex-native`; zero packaging/signing/release story; no cross-platform build
ever exercised.

---

## 5. The differentiator plan — extensibility & vibe coding

This is the product's stated identity and it currently **does not exist as a
surface**. But the audit found the load-bearing pieces already built:

- a real expression evaluator (playbook `resolve_level_expr`: named refs
  entry/stop/target/atr/vwap/callwall/putwall, arithmetic, R-multiples)
- a real spreadsheet formula engine (no market-data hooks yet)
- a 128-variant AppCommand bus (the natural script→action surface)
- dev_inspector's HTTP server (the natural LLM-in-the-loop surface — currently
  debug-only)
- a provider trait proving the registry pattern works here

**Sequenced build (the consensus of the extensibility, contributor, charting,
and architecture agents):**

1. **Indicator trait first** (the tractable boundary): replace the
   IndicatorType enum-and-match sprawl with
   `trait Indicator { compute, params, render }` + registry. This collapses the
   ~140-site contribution cost, is prerequisite for user-defined indicators, and
   is the first real plugin boundary. (XL, but pays for every later step)
2. **Embed Rhai** (fits the egui frame loop; no async runtime needed) with three
   host bindings in order: **(a)** custom indicators via
   `IndicatorType::Custom{expr}` reusing the expression-evaluator patterns;
   **(b)** signal→order hooks where scripts emit the same AppCommand variants
   the UI dispatches (inheriting every risk gate for free); **(c)** data access
   (bars, tape, gamma levels) read-only first.
3. **Ship a hardened, opt-in subset of dev_inspector in release** — bearer
   token, localhost default, allowlisted commands — as the automation API. This
   is what makes "vibe coding" real: an LLM writes a Rhai script or drives the
   HTTP API, the user watches the result live. The sibling-project patterns
   (supermodel's local-Claude-CLI chat, graphcoder's NL→verified-formula
   synthesis) port directly.
4. **Surface the playbook sharing backend** (publish/feed/fork already built) as
   the community layer — scripts and playbooks shared the same way.

---

## 6. Sequenced roadmap

**Wave 0 — close the money edges (S/M items, ~1-2 weeks)**
P0s #1-4, #6, #8, #15, #16 + the quote-cache timestamp/staleness gate (#48).
Nothing here is architecturally hard; it is the list that makes live trading
trustworthy. Gate: corpus + new unit tests per fix.

**Wave 1 — stop the UI lying (M items, ~2-3 weeks)**
P0s #7, #9-14, #17, #18. Drawing persistence chain end-to-end (enum unification
→ dead-letter surfacing → PG failure banner → reconnect loop); alert delivery
(OS notification + real sound) + pane-decoupling; hotkey wiring; footprint
grounding-or-badge; gap-fill routing to the real delivery path.

**Wave 2 — daily-driver parity (L items, ~4-6 weeks)**
Paper fill engine (the XL that unblocks strategy validation); options chain
unification (wire the built-but-unused OptionChainRow + IvRankWidget into one
real chain panel — also gives Spread Builder real conIds); replay overlay
wiring; DOM session-depth + ATM templates; alerts-on-drawings; FIFO PnL;
CI running the full test suite + nightly corpus.

**Wave 3 — the differentiator (XL, ~6-10 weeks)**
§5 in order: indicator trait → Rhai + three bindings → release automation API →
sharing surface. This is the moat; nothing in Waves 0-2 competes with it for
priority once the money edges are closed.

**Wave 4 — public product**
Domain extraction from gpu.rs + break the chart↔data cycle (prereq for crate
split); Broker trait de-IB-ification (ContractRef); runtime-config endpoints +
auth (#19); README/LICENSE/CI-builds-the-binary/installer; offline demo mode
(the Yahoo fallback already proves bars work without the homelab).

---

## 7. Cross-cutting patterns the fixes must follow

1. **The badge pattern is law**: any data that isn't real-and-live gets the
   DOM's LIVE/SIMULATED treatment. The footprint, compact-DOM depth, and any
   future fallback rendering all inherit it.
2. **All sites or none**: the WS-H lesson — fixes that land on a subset of the
   named sites while docs read "closed" are the origin of half this ledger.
3. **State the UI writes must be state the app reads**: wizard limits, hotkeys,
   replay checkbox — before adding any settings surface, wire the read side first.
4. **Ratchet everything**: the quality-gate pattern works (bus-bypass count,
   file size, unwraps). Add ratchets for: direct-mutation count (drive to zero),
   unbadged-fallback renders, enum-match sites for IndicatorType (drive down as
   the trait lands).

---

## 8. Refuted claims (the verification pass earning its keep)

- **"No options chain view exists"** — REFUTED: a live chain (strikes, bid/ask,
  OI, IV color-coding, ITM tint, click-to-add) exists in
  `watchlist_panel.rs` WatchlistTab::Chain (~1489-2067). The real finding is
  *fragmentation*: it's a parallel hand-rolled implementation, not the built
  OptionChainRow widget.
- **"Offline shows no chart data"** — REFUTED: bar loading is a 5-tier
  FallbackProvider ending at public Yahoo Finance; historical charts load with
  zero config (after ~10s of cascading timeouts). The narrower true gap: live
  streaming has no fallback.
- **"Most Active scanner preset silently no-ops"** — recorded REFUTED, but the
  verifier's note was malformed; treat as *unresolved* and re-check before
  relying on it.

Plus 26 ADJUSTED claims — mostly severity corrections in both directions (three
adjusted **up** to P0: daily-loss reset, cancel/modify status, watchlist wipe).
Details inline in the appendix.

---

*Method note: dimension briefs, agent outputs, and verification notes are
reproduced in full in `WORLD_CLASS_AUDIT_2026-07-18_APPENDIX.md`. P2/P3 findings
were not adversarially verified — treat those as leads, not conclusions.*
