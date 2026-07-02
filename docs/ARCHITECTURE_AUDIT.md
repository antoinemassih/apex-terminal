# apex-terminal — Deep Architecture & Quality Audit

Date: 2026-07-03. Method: five parallel specialized audits (architecture/modularity,
duplicate & parallel systems, design system & UI consistency, backend/data layer,
code quality & robustness) over `src-tauri/src`, cross-checked against the
dev-inspector scenario suite's session findings. All claims carry file:line evidence
(verified, not guessed). Codebase: **450 files, 185,349 LOC**.

---

## 0. Executive summary

**Verdict: a well-engineered core wearing the scar tissue of six half-finished
migrations.** The money paths are disciplined (zero panicking unwraps in the broker
layer, idempotency keys, WAL + crash recovery, persisted kill-switch). The design
system is real and 90%-adopted in panels. The dev-inspector harness (1,000+
scenario suite) is rarer than anything else here. But the app's entire state lives
in two god-structs (Chart ≈ 240 fields, Watchlist ≈ 258), its entire render pass
in one **11,335-line function**, and nearly every subsystem has a dead or stalled
"correct" replacement sitting next to the live one.

### The one-sentence diagnosis

> The target architecture already exists in-tree — it was designed, partially
> built, and never cut over. The highest-leverage work is not new design; it is
> **finishing (or explicitly deleting) six stalled migrations** and adding the CI
> ratchets that stop the next regression.

### The six stalled migrations (the cross-cutting disease)

| # | Migration | The "new" system (exists, correct) | The live system (still in use) | State |
|---|---|---|---|---|
| 1 | State stores | `state/aggregates.rs` `Store<_>` (7 stores) | flat Watchlist fields | **dual-write both directions** — doc comments admit flat fields "remain the read source of truth" (`gpu.rs:6349-6402`). Worse than either end-state. |
| 2 | Canonical chart state | `chart/state/{drawings,annotations,mod}.rs` (ts_ns-anchored Drawing, markdown Annotation, `Timeframe` enum) | renderer's own `Drawing`/`DrawingKind`, `timeframe: String` | entire `chart/state/*` tree is `#[allow(dead_code)]`; only `drawing_db.rs` touches it |
| 3 | Order lifecycle types | `trading/types.rs` (Wave-3) | `trading/mod.rs` enums (byte-identical) | dead module, 4th parallel order representation |
| 4 | Widget extraction | `ui/lists/cards/*` façades (PlayCard, signal_card, …) + `ui/widgets/*` re-export shims | hand-rolled panel painters (`plays_panel::render_play_card`) | Wave-5, "no call sites migrated" |
| 5 | Persistence envelope | `state/persistence.rs` (versioned, atomic, migration hooks — textbook) | ~28 loose JSON files, 46 raw `fs::write` sites, 7 patterns | envelope adopted by **one** call site (orders shadow-write) |
| 6 | core.rs decomposition | `docs/PANE_RS_SPLIT_PLAN.md` | `render_chart_pane` = 11,335-line fn | plan written, unexecuted |

Every one of these leaves **two sources of truth** that drift independently. The
dev-inspector's dual app_state builders already caused real test failures from
exactly this pattern; the per-frame order reconcile carries a documented
"snap-back" bug workaround from the same disease.

### Scorecard

| Dimension | Grade | One-liner |
|---|---|---|
| Trading-safety architecture | **A-** | broker layer exemplary; 3 sharp last-mile gaps (below) |
| Test discipline | **B+** | 701 unit tests + 1,065-scenario harness; but the newest pure engines have zero unit tests |
| Design system | **B+** | tokens/theme-packs/rail registry excellent; chart overlays 0% themed (205 hardcoded colors in one file) |
| Resilience (feeds/net) | **B+** | resilient_ws (jittered backoff, watchdog, shutdown) genuinely good; failures often silent |
| Modularity | **D** | two god-files = 13% of codebase; 240/258-field structs; 153/450 files import the god module |
| Source-of-truth hygiene | **D** | 4 order layers, 2 Bars, 3 drawing families, 5 line pipelines, 7 persistence patterns, 12 `now_ms()` |
| Observability | **C** | Prometheus covers GPU/jank only; zero metrics on feeds/broker failures; 122 errors_sink call sites is a good chokepoint though |
| CI / quality gates | **F** | no clippy config, no lint gate, no unwrap budget, no file-size ceiling, no build matrix |

---

## 1. Critical findings (fix first)

### C1. `render_chart_pane` is an 11,335-line function
`render/pane/core.rs:140-11475`. One function contains the pane render, input,
hit-testing, drawing, order/alert dragging, overlay computation — with inline
enums defined mid-function (line 9593). No unit boundary exists below whole-pane.
**Fix:** execute the existing `PANE_RS_SPLIT_PLAN.md` — extract by frame phase
(`input.rs`, `axes.rs`, `candles.rs`, `drawings.rs`, `overlays.rs`,
`order_layer.rs`, `hit_test.rs`) around a `&mut PaneRenderCtx`. Ceiling: no fn > 400 lines.

### C2. Chart (~240 fields) and Watchlist (~258 fields) are the entire app state
`gpu.rs:2662-2995`, `gpu.rs:5935-6413`. "Watchlist" contains: watchlist (~15),
scanner (12), RRG (5), playbook + editor (~30), options chains (~30), Discord chat
(17), command palette (7), ~40 `*_open` panel flags, ~20 layout splits, workspace
mgmt, settings — plus 7 mirror `Store<_>`s (stalled migration #1).
**Fix:** split into owned sub-contexts (`ScannerState`, `PlaybookState`,
`OptionsChainState`, `ChatState`, `LayoutState`, `PanelVisibility`, `Settings`)
composed in an `AppState`; finish or revert the Store dual-write (pick one).

### C3. gpu.rs hosts ~8 unrelated responsibilities (10,976 LOC)
Chart struct + Watchlist struct + domain types + **21 persistence functions** +
winit window loop + wgpu surface + placeholder generators + sim tick + feed glue.
153/450 files (34%) import from it — the lowest-level and highest-level module at
once (dependency-inversion failure). **Fix:** `chart/model.rs`, `app/state.rs`,
`app/window_loop.rs`, `persistence/workspace.rs`, `chart/theme.rs`; `gpu/` keeps
only wgpu.

### C4. Four parallel order representations, reconciled by hand every frame
Visual `Chart.orders: Vec<OrderLevel>` (`trading/mod.rs:38`) vs authoritative
`OrderManager` (`order_manager.rs:251`) vs dead `trading/types.rs` vs the
transitional DOM insert path. The per-frame three-way merge (`core.rs:216-254`,
again at `:1497-1600`) carries a documented drag-exclusion workaround for a real
"order line snaps back" bug; manual `OrderLevel{}` construction sites mean a new
field silently defaults. **Fix:** `chart.orders` becomes a computed view of
`all_order_levels_for()`; delete `trading/types.rs`.

### C5. Chart overlays bypass theming entirely — 205 hardcoded colors in core.rs
~484 hardcoded `Color32::from_rgb` total; `core.rs` alone has 205 (VWAP
`:2903`, volume profile `:2358`, delta bars `:2298`, play zone bands
`:7938-7959`, R:R box `:7973-7997`). The 21 color schemes reskin the chrome but
**not the data surface**. Within the play overlay, lines use tokens while bands
hardcode — visible mismatch on reskin. **Fix:** `overlay_palette(t)` tokens;
migrate core.rs literals; start with play bands (lines already tokenized).

---

## 2. High-severity findings

### H1. Trading last-mile safety gaps (live-money)
The broker layer is otherwise exemplary (zero panicking unwraps, 5s timeouts,
UUID idempotency, WAL + `replay_and_recover`, persisted kill/halt, rate limiting,
dedup, fat-finger gates). Three sharp gaps:
- **Non-atomic writes of `orders.json` and `control_flags.json`**
  (`order_manager.rs:2353, 2737`) — raw `fs::write` for the *kill-switch* file,
  while a textbook `atomic_write` sits unused next door. A crash mid-write can
  corrupt the "must never silently un-engage" gate. *Trivial fix.*
- **Optimistic `Working` before broker Ack** (`order_manager.rs:1290`): the order
  is persisted `Working` with `backend_order_id=None` while the POST is in flight;
  a crash in that window restores an uncancelable phantom. Persist `PendingSubmit`
  until Ack.
- **No paper guard inside `LiveBroker`**: the paper short-circuit lives only
  upstream in `submit()` (`:1224`). Any new call path reaching `broker.submit()`
  posts live. Add a defense-in-depth assertion (or swap a `PaperBroker` object on
  mode toggle). Also: `paper_mode` resetting to true on restart is *correct* but
  undocumented-as-intentional.

### H2. No panic isolation in release
674 `.unwrap()/.expect()` total; 118 `.lock().unwrap()` (poisoning); panic hook is
dev_inspector-only; **zero `catch_unwind`**. One worker panic → poisoned mutex →
cascade panic → window death. **Fix:** release panic hook (tracing), `catch_unwind`
around every spawned worker body, migrate shared state to the already-present
`parking_lot::Mutex` (non-poisoning). Also fix the 3 unguarded
`pending_pts.last().unwrap()` in `tool_previews.rs:294/367/467` (render-thread
panics on a racy empty vec).

### H3. Command bus is ~3% adopted
`commands.rs` (100 variants, central reducer, `request_gen` staleness guard) is
enterprise-grade **design** — bypassed by 776 direct field mutations in `ui/` and
240 in `core.rs` vs ~30 dispatch calls. The reducer is decorative. This blocks C2
(state decomposition) because mutations are scattered. **Fix:** lint direct
`wl.*=`/`chart.*=` in `ui/` as failures; route through AppCommand. Highest-leverage
single refactor in the codebase.

### H4. Pure engines have zero unit tests
`grade_play`, `arm_branches`, `snap_price`, `resolve_level_expr`,
`option_payoff_at` (chart/renderer/mod.rs — **no test module at all**),
`apply_scanner`, `eval_formula` — covered only end-to-end by the scenario harness.
These decide grading, snapping, option P&L. **Fix:** table-driven `#[cfg(test)]`
next to each; `proptest` (already a dev-dep) for payoff monotonicity and snap
idempotence. The harness is the integration layer, not a substitute.

### H5. UI does I/O; layering leaks
reqwest in 4 `ui/` files (command_palette/execute.rs, top_nav.rs, orders_panel.rs,
trendline_filter.rs); thread spawns in 8 `ui/` files; `APEX_SIGNALS_HTTP` and the
gamma :8412 base configured inline in `gpu.rs`/`fetch.rs` outside the config
modules. **Fix:** UI emits commands; `data/` owns fetch + threads; endpoints join
`data/endpoints.rs`.

### H6. Observability gap for a live-money system
Prometheus (:9091) covers GPU/frame-jank only. **No metrics** for: feed
disconnect/reconnect counts, broker submit/reject rates, cache misses, staleness.
Failures route to errors_sink (good chokepoint, 122 call sites) or worse:
`apex_data/ws.rs` uses 11 raw `eprintln!` — the primary firehose's diagnostics are
invisible in-app. Gamma fetch failures silently fall back to **synthetic levels
with no visual "synthetic" badge** — operator can't distinguish real GEX walls
from fabricated ones. **Fix:** export errors_sink + per-feed counters to
Prometheus; badge synthetic/stale data (DOM already does this — "SIMULATED" —
copy that pattern); convert ws.rs eprintlns.

### H7. Persistence fragmentation
7 distinct patterns; ~28 loose JSON files; 46 raw `fs::write` sites (non-atomic,
corruptible); the versioned atomic envelope used by 1 site. No shared
versioning/migration → invisible schema drift. **Fix:** migrate home-rolled JSON
onto `state::persistence`; wrap remaining raw writes in `atomic_write`.

---

## 3. Medium-severity findings

- **M1. Two `Bar` structs** — `chart/renderer/types.rs:5` (f32, GPU-packed, no
  time; parallel `timestamps: Vec<i64>` can silently desync) vs
  `foundation/types/mod.rs:59` (f64, wire). Fix: explicit `From` at one boundary.
- **M2. Three drawing families + five draggable-line pipelines** — live
  `Drawing`/`DrawingKind`, dead canonical XOL Drawing, `SignalDrawing`; plus
  alert/play/order/drawing/trigger lines each with own hit-test, drag, render,
  teardown (priority chain hand-written `core.rs:10770-10810`). Adding a line type
  touches ~6 places. Fix: one `ChartLine` pipeline with a `kind`; converge on the
  ts_ns-anchored canonical model.
- **M3. Two dev_inspector app_state builders** (`mod.rs:570` headless vs `:1130`
  real) — shape drift already caused test failures once. Fix: one typed
  serializer.
- **M4. Stringly-typed production APIs** — `timeframe: String` in the live layer
  while a `Timeframe` enum exists in `chart/state`; `pivot_mode: String`
  ("hybrid|atr|percent"); `extend: String`. Fix: enums with Display/FromStr;
  strings only at the HTTP edge.
- **M5. Mechanical duplication** — **12 `now_ms()` definitions** (i64/u64 split!),
  **56 HTTP-client constructions** (order_manager spawns a fresh
  `blocking::Client` — new TLS handshake — per broker op, 22 spawn sites),
  2 hex parsers, 3 K/M/B formatters, duplicated `APEXIB_URL` const. Fix:
  `foundation::time::now_ms()`, `http_client()` factory, shared `fmt_compact`.
- **M6. Per-frame allocation in release** — `grade_open_plays_live`
  (commands.rs) allocates HashSet + HashMap + `to_uppercase()` strings every
  frame when ≥1 play is open. Fix: cache normalized symbol on Play; reuse scratch.
- **M7. Overlay/z-order fragmentation** — ui_kit `Modal`/`Toast` exist but ≥10
  sites use raw `egui::Window` (incl. toasts); **three** `dialog_window*`
  factories in style.rs; SidePanelShell's own docs prescribe Modal but dialogs
  don't comply. Fix: consolidate on ui_kit Modal/Toast.
- **M8. Feature/cfg risk** — `gpu_chart_v2` default feature silently swaps the
  entire render path when disabled (no CI matrix); `design_inspector.rs` (3,317
  LOC) only compiles under non-default `design-mode` (bit-rot risk). Fix: CI
  builds `--no-default-features` and `--features design-mode`.
- **M9. Keyboard shortcuts scattered across 21 files** — no dispatch table
  (hotkey_editor UI exists; `foundation/shortcuts.rs` does not). Fix: central
  action→binding table feeding the palette and the editor.
- **M10. Config sprawl (partial)** — endpoints mostly centralized in
  `data/endpoints.rs` (good), but 17 backend `env::var` sites remain; LAN IPs
  hardcoded as defaults (OCOCO `192.168.1.60`, crypto `192.168.1.56`); Discord
  creds resolved via **compile-time** `CARGO_MANIFEST_DIR` (`discord.rs:78,262`) —
  broken in any shipped binary. Fix: exe-relative resolution (pattern exists in
  orders_state_path).

## 4. Low-severity / polish

- Dead parallel widgets: `play_card.rs`, `playbook_card.rs`, `signal_card.rs` etc.
  (Wave-5 façades) — adopt or delete. 47 `#[allow(dead_code)]` sites.
- `auto_chart_panel.rs` is the lone raw-egui panel (10 raw controls) — migrate to kit.
- Line-type visual affordances diverge (play badges vs order badges vs alert
  dashes) — one `ChartLine` badge/dash spec.
- ux_audit harness exists (16px floor) but isn't enforced as a failing gate — flip it on.
- Crate named `_scaffold_lib` in a Tauri-less `src-tauri/`; legacy re-export
  aliases (`chart_renderer`) — rename when convenient.
- Daily-loss P&L is best-effort local accounting (documented) — label advisory in
  UI; prefer broker-reported realized P&L.
- Redis/Postgres misconfig → one-time stderr warn, then silent no-cache/no-persist
  operation — surface a persistent indicator.
- docs/ is plan-heavy (7 planning docs, some stale/done) — date-stamp or prune.

---

## 5. What is genuinely good (preserve these)

1. **The AppCommand bus design** — 100 documented variants, central reducer,
   `request_gen` staleness guard. Only adoption lags.
2. **The dev_inspector harness** — deterministic input injection, assertion
   engine, headless mode, 1,065-scenario behavioral-oracle suite. Rarer than
   anything else in this codebase; it's the safety net for every refactor above.
3. **The broker/risk layer** — idempotency, WAL + recovery, persisted kill/halt,
   rate limiting, dedup, documented lock-ordering fixes, zero panicking unwraps.
4. **ui_kit + design_system** — real token system (~80 fields), theme packs with
   versioned migration/validation, 90% panel adoption, the right-rail registry
   (one-line panel registration with *declared* exceptions).
5. **resilient_ws** — jittered exponential backoff, LAN-aware, idle watchdog,
   shutdown drain, unit-tested.
6. **state/persistence.rs** — textbook atomic envelope with migrations (adopt it!).
7. Test discipline overall: 701 unit tests, clean TODO tagging, zero
   `unimplemented!`, dev tooling verified compiled-out of release.

---

## 6. Remediation roadmap (prioritized)

### P0 — Safety (days; do before any live session)
1. `atomic_write` for `orders.json` + `control_flags.json` (H1) — ~1h.
2. `PendingSubmit`-until-Ack persistence (H1).
3. Paper assertion inside `LiveBroker` (H1).
4. Release panic hook + `catch_unwind` on worker bodies + parking_lot migration (H2).
5. Guard the 3 `last().unwrap()` in tool_previews (H2).
6. Fix per-frame allocs in `grade_open_plays_live` (M6).

### P1 — Stop the bleeding (1–2 weeks)
7. **CI quality gates**: clippy `-D warnings` + `warn(unwrap_used)` ratchet,
   per-directory unwrap budget, file-size ceiling (warn 2.5k / fail 6k,
   grandfathered), build matrix (release / no-default-features / design-mode),
   ux_audit as a failing gate. Without this, everything below regresses.
8. **Decide each stalled migration** — finish or delete, one by one:
   `trading/types.rs` (delete), widget façades (adopt or delete), Store dual-write
   (pick a direction), `chart/state` canonical layer (wire drawings/annotations or
   remove), persistence envelope (migrate the 28 JSONs), Timeframe enum (adopt).
9. Mechanical dedup: one `now_ms()`, one `http_client()` (fixes the per-op TLS
   handshake too), one hex parser, one formatter (M5).
10. One typed app_state serializer for dev_inspector (M3).
11. Unit tests for the pure engines + proptest invariants (H4).
12. Prometheus counters for feeds/broker/cache + synthetic-data badges (H6).

### P2 — Architecture (weeks, incremental, harness-guarded)
13. Command-bus adoption ratchet: no new direct mutations in `ui/`; migrate
    hotspots (H3). This unblocks everything below.
14. Extract domain types out of gpu.rs (`chart/model.rs`, `app/state.rs`,
    `persistence/workspace.rs`) (C3).
15. Split Watchlist into sub-contexts (C2).
16. Execute PANE_RS_SPLIT_PLAN on render_chart_pane (C1).
17. Orders-as-view consolidation (C4); unified ChartLine pipeline (M2).
18. Feed `Subscription<T>` generic over resilient_ws; UI I/O eviction (H5).

### P3 — Design/UX polish
19. Tokenize chart overlays — 205 colors in core.rs, start with play bands (C5).
20. Consolidate Modal/Toast/dialog factories (M7).
21. One ChartLine badge/dash affordance spec; shortcuts table (M9);
    auto_chart_panel kit migration; delete-or-adopt dead cards.

**Sequencing note:** every P2 item is refactoring under test — the 1,065-scenario
suite (plus the new unit tests from P1.11) is the regression net. Run the corpus
after each extraction; the harness was built for exactly this.
