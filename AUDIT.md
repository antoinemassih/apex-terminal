# apex-terminal — Architecture & Data-Layer Audit

_Generated 2026-06-28. Scope: `src-tauri/` (447 `.rs` files, 182,242 lines). Read-only audit; this document records findings + a sequenced roadmap._

## Verdict

A capable, feature-rich app that has **accreted faster than it's been refactored**. It works, and it has a genuinely strong safety net (460-scenario dev-inspector harness, 696 unit tests, only 48 TODOs / zero FIXME/HACK). But three structural problems are now actively costing maintainability — and two of them directly explain recent runtime bugs (empty candles, endless pane load, autochart drawing nothing).

The GPU chart render path / continuous-redraw model is **performance-sacred**. Every recommendation below either avoids it or treats changes to it as pure, benchmark-gated code-moves.

---

## 1. Data-source connectivity (priority)

**Two parallel data architectures, the newer one half-wired.** A clean `MarketDataProvider` trait + `FallbackProvider`/`CachedProvider`/`SubscriptionManager` exists, but **live data bypasses it** — live `subscribe_*` streams are created and their receivers dropped (`src/chart/renderer/io/fetch.rs:1887`); live bars actually arrive via a separate listener registered in `src/lib.rs:103`. Every symbol effectively has two subscriptions and two code paths.

### Feed inventory (16 sources)
Primary: **ApexData WS** (`feeds/apex_data/ws.rs:198`, bars/snapshot/trade/quote/fmv/chain/halt/spike/regime) and **ApexData REST** (`feeds/apex_data/rest.rs`, ~40 endpoints). Plus crypto_feed (hardcoded `ws://192.168.1.56:30840`), signals_feed (`ws://localhost:8100`), dom/futures/intercepts/drawings feeds, ApexSignals drawings REST, OCOCO (`192.168.1.60:30300`), yfinance sidecar, Yahoo v8, legacy ib_ws (dormant), Redis bar cache, Discord.

### Problems
- **[HIGH] One global REST circuit breaker starves the chart.** `rest.rs:22-107` is process-wide. ~Four pollers hit many endpoints/sec; 3 consecutive **network errors** (not 404s — those are correctly excluded) open it for 30s, short-circuiting *every* REST call incl. chart-history bars (`rest.rs:141`). → root cause of "endless loading / empty candles."
- **[HIGH] Backend failure → silent empty UI.** `fetch_bars_background` on all-failed only `eprintln!`s and returns — no command, no spinner-clear, no toast (`fetch.rs:1894`). `fetch_apexsignals_drawings` swallows every error (`gpu.rs:1805`). Many REST wrappers end in `.ok()`, discarding the typed error. → root cause of "autochart draws nothing / blank chart, no explanation."
- **[HIGH] No unified feed abstraction.** dom/futures/intercepts/drawings/signals/crypto each re-implement connect-loop, LAN dial, parse, and a copy-pasted `NATIVE_CHART_TXS` send. Watchdog+toast duplicated verbatim 3×; 4 feeds have no metrics and don't drain on shutdown.
- **[MEDIUM] Hardcoded endpoints + leaky secret.** crypto/OCOCO/yfinance/ib_ws hardcoded, no env override, no LAN-resolve. WS auth token passed as a **URL query param** (`ws.rs:324`) — leakable to logs — while REST uses a bearer header.
- **[MEDIUM] Feed↔render coupling.** Feeds reach into renderer internals (`dom_feed` builds UI `DomLevel`; `intercepts_feed` writes the alert UI singleton directly), so the data layer can't move independently.
- **[MEDIUM] Inline frame→ChartCommand mapping** is one big `match` with a silent `_ => {}` (`lib.rs:105-213`); a new `Frame` variant compiles while being silently dropped.

### Target
Per-host/route breakers; a `ResilientWs` helper owning connect/backoff/watchdog/metrics/toast (each feed → ~30 lines); centralized endpoint config (runtime→env→default) + token moved to a header; one error→UX path (inline "no data / backend unreachable" + throttled toast); finish *or* formally retire the provider abstraction; split `ChartCommand` into a data-owned `DataEvent` vs UI-control, bridged at the window boundary (preserving the exact mpsc + `request_repaint` handoff).

---

## 2. Architecture & modularization

### God objects (measured)
| Struct | Location | Fields |
|---|---|--:|
| `Chart` | `gpu.rs:2453` | **242** |
| `Watchlist` | `gpu.rs:5606` | **273** (misnamed — it's the global App/Settings state: ~50 panel `*_open` bools, layout engine, palette, plays, Discord client, settings) |

### Giant files / functions
| File | Lines | | Function | ~Lines |
|---|--:|---|---|--:|
| `render/pane/core.rs` | 12,910 | | `render_chart_pane` | ~1,744 |
| `gpu.rs` | 10,552 | | `apply_draw_tool` | ~1,399 |
| `trading/order_manager.rs` | 5,178 | | `tick_simulation` | ~974 |

`render_chart_pane` interleaves rendering, hit-testing, input handling, IO triggering, and state mutation in one stack frame (110+ `// ──` markers).

### Layering & coupling
- **[HIGH] `ui_kit → chart_renderer`** (23 refs): the generic widget kit pulls style tokens + frame widgets *up* from the app (`ui_kit/tokens.rs:28`), plus `bug_anchor` calls embedded in leaf widgets. `ui_kit` can't be reused independently.
- **[HIGH] IO leaks into render:** `render_chart_pane` fires `fetch_chain_background` directly (`core.rs:1164`).
- **[MEDIUM] Command pattern half-applied:** ~100 `commands::push` sites vs ~246 direct field mutations in `core.rs` alone. Two mutation models coexist.
- **[MEDIUM] Dual-write state:** newer typed `Store<T>` aggregates mirror legacy flat fields by hand (`push_to_*`/`sync_from_*`).
- **[HIGH] `gpu.rs` is a god module** mixing wgpu/winit lifecycle, theming, the whole domain model, persistence, and ~40 small structs.

---

## 3. Code health

- **576 warnings**; ~224 (39%) auto-fixable (`cargo fix`: 133 unused imports + 73 unused vars + 18 needless `mut`). Largest real signal: **219 visibility warnings** (inconsistent `pub` surface).
- **Dead code:** 64 `#[allow(dead_code)]` + 34 file-level blanket allows.
- **Risky:** ~33 genuine non-lock `.unwrap()`s; the one that matters is `live_state.rs:364` (live feed path).
- **Cyrillic homoglyph** in `painter_pane.rs` (`PaneBtn::ClosePanе` — Cyrillic 'е').
- **Test gap:** `core.rs` (12.9k lines) has 0 unit tests (covered only via the scenario harness).

Positives: small real risky-unwrap surface; strong integration harness; clean of FIXME/HACK.

---

## Roadmap (sequenced, risk-aware)

**Sacred-path rule:** changes to `render_chart_pane`/GPU stay pure code-moves (no new allocs/locks/channels in-frame), each gated on a frame-time check.

### Phase 0 — hygiene (hours, ~zero risk)
- `cargo fix --lib` (~224 warnings).
- Fix the Cyrillic homoglyph.
- Harden `live_state.rs:364` unwrap.
- Fix `fetch_bars_background` silent-fail → emit a visible "no data / backend unreachable" state (directly fixes the blank-chart mystery).

### Phase 1 — data layer (highest ROI)
Per-host/route breakers; `ResilientWs` helper + centralized endpoint config; unified error→UX path; decide provider-abstraction fate.

### Phase 2 — decompose (behavior-preserving)
Rename `Watchlist→AppState`; collapse ~75 `*_open`/`show_*` bools into bitsets; extract overlay render sections from `render_chart_pane` into `render/pane/overlays/*` free functions; extract the input chain into `input.rs` returning `Vec<AppCommand>`.

### Phase 3 — structural
Split `gpu.rs` into `app/`/`theme/`/`model/`; group `Chart`/`AppState` fields into sub-structs; finish the `Store<T>` migration (delete dual-writes); fix the `ui_kit→chart_renderer` inversion.

---

## Progress log
- 2026-06-28: Audit created. Phase 0 started.
- 2026-06-28: Phase 0 complete (commit 7e7a80b3) — `cargo fix` sweep (576→409 warnings), Cyrillic homoglyph fixed, `live_state.rs:364` unwrap hardened, data-outage UX (`BarsUnavailable` → "no data" message instead of endless spinner).
- 2026-06-28: Phase 1 (1/4) — **per-route circuit breakers**. Replaced the single global breaker in `apex_data/rest.rs` with a `HashMap<route_group, Breaker>` keyed by the first two path segments (`/api/bars` vs `/api/snap` etc.), so a failing poller endpoint can no longer starve chart-bar history for 30s. Wired through `get`/`post_json`/`get_provenance`; `breaker_snapshot` now aggregates across groups (diagnostics panel unchanged); added `breaker_routing_tests`.
- 2026-06-28: Phase 1 (2/4) — **ResilientWs harness** (`data/feeds/resilient_ws.rs`): shared LAN-connect + jittered Backoff + idle watchdog + Shutdown registration + reconnect-on-signal + `send_to_charts`. Migrated **futures, dom, intercepts, drawings** onto it (commits 1de6b50f + this) — each loses its private connect/LAN/send copies and gains backoff/shutdown they lacked. **Deferred:** crypto + signals (they already publish `ConnectionState`/metrics to the connection panel that the helper doesn't emit yet — migrating now would regress panel visibility; do after the helper publishes state). Remaining Phase 1: centralized endpoint config + token-off-URL; provider-abstraction decision; (helper: ConnectionState/metrics + stall-toast, then migrate crypto/signals).
