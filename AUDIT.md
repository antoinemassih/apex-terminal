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
- 2026-06-28: Phase 1 (2/4) — **ResilientWs harness** (`data/feeds/resilient_ws.rs`) DONE. Two entry points: `spawn` (simple read-only feeds: LAN-connect + jittered Backoff + idle watchdog + Shutdown + reconnect-on-signal + `send_to_charts`) and `spawn_subscribed` (subscribe-style feeds: also write-half subscribe + heartbeat (conditional/unconditional) + tick-watchdog with cooldown'd stall toast + `ConnectionState` lifecycle + metrics handles + gap-fill-on-reconnect). **All 6 secondary feeds migrated**: futures/dom/intercepts/drawings (commits 1de6b50f, 9d11a153) and signals/crypto (de5dd912 + this). Each lost its private connect/LAN/send copy; the 4 ad-hoc feeds gained backoff+shutdown they lacked; crypto/signals keep their `state_tx()`/metrics statics so the connection panel + providers are unchanged. Remaining Phase 1: provider-abstraction decision (#4).
- 2026-06-28: Phase 1 (3/4) — **endpoint config + token security**. (a) apex_data WS + replay now send the auth token via the `Authorization: Bearer` header instead of a `?token=` URL query (commit a2afcf89) — stops the secret leaking into proxy/server/app logs; needs backend header-auth (one-line revert if not). (b) New `data/endpoints.rs` centralizes the previously-hardcoded homelab hosts (crypto `192.168.1.56:30840`, OCOCO `192.168.1.60:30300`, yfin sidecar `127.0.0.1:8777`, Yahoo, legacy ibserver `127.0.0.1:5000`) behind env overrides (`APEX_CRYPTO_WS/HTTP`, `OCOCO_URL`, `YFIN_SIDECAR_URL`, `YAHOO_CHART_URL`, `APEX_IBSERVER_WS`) with the old values as defaults; updated all ~15 call sites across providers/fetch/foundation/gpu/crypto_feed/ib_ws. No more baked-in IPs.
- 2026-06-28: Phase 1 (4/4) — **provider-abstraction decision: keep as-is, documented**. Investigated the "half-wired" claim: it was a misread. The `MarketDataProvider`/`SubscriptionManager` layer is fully load-bearing — it owns (1) historical/fallback bar fetch (`bar_chain`), (2) the **live upstream subscription** (`subscribe_bars` → `ws::add_bar_sub`, i.e. what tells the firehose to stream a symbol), and (3) **gap-fill anchoring** (the apex_data frame listener + per-sub pump keep `last_seen_ts` current for `gap_fill_on_reconnect_all`, used by all 4 WS feeds). The only "dead" thing is the **fanout `broadcast::Receiver` dropped at the call site** — intentional: live bars reach the UI via the single `NATIVE_CHART_TXS` hot path, not a redundant per-frame fanout. Neither audit option was correct: deleting (A) would break live subscription + gap-fill; finishing (B) would add a redundant delivery path. Resolution: **no code change**; rewrote the stale `providers/mod.rs` header (was "Migration … is Wave 5") to document the intended split + a clarifying comment at the `fetch.rs` subscribe site, so it isn't misread/ripped out again. **Phase 1 complete.**

---

## Phase 1 — COMPLETE (2026-06-28)
Data layer hardened: per-route breakers (no cross-endpoint starvation), one `ResilientWs` harness for all 6 secondary feeds (uniform backoff/watchdog/shutdown/state), WS token off the URL, endpoints env-overridable, and the provider abstraction's real scope documented. Next up if/when desired: Phase 2 (decompose the god structs + giant files) and Phase 3 (gpu.rs split, `ui_kit→chart_renderer` inversion).
