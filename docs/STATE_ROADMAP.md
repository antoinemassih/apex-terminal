# Apex Terminal — State System: Enterprise-Grade Roadmap

Companion to [`STATE_SYSTEM.md`](./STATE_SYSTEM.md), which documents the state
system **as it is today**. This document is the **plan** to bring it to
enterprise grade.

---

## 1. Where we are

The state system is *uneven*. One domain — the trading/order engine — is
genuinely near enterprise grade (write-ahead log, crash recovery, lock-free
read snapshot, dedup, rate limiting, circuit breaker). Everything else is
production-prototype: two god-objects (`Watchlist` ~150 fields, `Chart` ~200
fields), ~45 global singletons, **four overlapping, each-incomplete generations**
of architecture (`Watchlist`/`Chart` direct-mutation · `AppCommand` queue at
~5% coverage · the `state/` Store layer as *mirrors* · `chart/state/ChartState`
unwired), ~80% of mutation as scattered direct `&mut` writes, and a workspace
persistence path that silently loses fields.

Per-domain grades (from the four-agent state audit):

| Domain | Today | Target |
|---|---|---|
| Orders / trading engine | A− | A |
| Data / market feeds | C+ | A− |
| Drawings | B− | A |
| Workspace persistence | D | A |
| Settings state | C | A |
| UI state | C | A− |
| Notifications | C | A− |
| User interactions (input) | C+ | A |

## 2. Target architecture (the end state)

- **One per-chart model:** `chart/state/ChartState` (SlotMap-based) — drawings,
  annotations, indicators, viewport. The `Chart` god-object is decomposed away.
- **One app model:** the `state/` aggregate layer — and the aggregates are the
  **source of truth**, not mirrors of `Watchlist` fields.
- **One mutation path:** every state change is an `AppCommand` dispatched to a
  reducer — a single auditable choke point with invariant checks.
- **One input pass:** global input handled once per frame for the active pane,
  emitting commands — never read inside the per-pane render loop.
- **One persistence path:** versioned, complete (snapshot ⊇ live state, verified
  by test), crash-recoverable for *all* durable state, not just orders.
- The other three architecture generations are **deleted**, not left standing.

## 3. Principles

1. **Incremental, always green.** No big-bang rewrite. Old and new coexist;
   new is adopted domain-by-domain. Every commit builds; every phase ends
   shippable and is relaunch-verified.
2. **Money-state is the reference standard and is never regressed.** The order
   engine is already enterprise grade — phases may change how orders are
   *triggered*, never the engine itself. Money-path changes get extra
   verification.
3. **`core.rs` stays sacred.** Per `src-tauri/CLAUDE.md`: single-owner,
   benchmark-aware changes only; no mechanical sweeps.
4. **Each phase closes specific audited findings** and has a measurable exit
   criterion.

---

## 4. Phases

### Phase 0 — Stabilize: stop data loss & correctness bugs
**Goal:** zero known correctness/data-loss bugs — a safe foundation to build on.
**Work:**
- Workspace persistence **v2 → v3 parity** — close the silent field loss
  (`chart_widgets`, option fields, `bar_source`, the settings blob). Saving and
  reloading a workspace must lose nothing.
- Finish the residual input-fan-out items: non-chart panes using the active
  pane's `theme_idx`; `deferred.rs` hard-coding `panes[0]` timeframe; stale
  `tab_changes` on inactive tabs.
- Add a `#[test]` per aggregate asserting the persisted snapshot is a **superset**
  of the live struct — fails when a field is added but not persisted.

**Closes:** workspace data loss (D); audit bugs #8/#9/#10.
**Exit:** build green; no known data-loss path. **Size:** S–M.

### Phase 1 — Decide & freeze
**Goal:** one declared target; the god-objects stop growing.
**Work:**
- Commit an Architecture Decision Record: `ChartState` = per-chart canonical;
  `state/` aggregates = app canonical; `AppCommand` = the mutation path.
- Freeze `Watchlist` / `Chart`: documented review rule — **no new fields** on
  the god-objects; new state goes to `ChartState` or an aggregate.

**Closes:** the "architecture keeps being re-started" pattern.
**Exit:** ADR committed; freeze rule in `CLAUDE.md`. **Size:** S (decision).

### Phase 2 — Input layer
**Goal:** eliminate the per-pane-loop input bug class **structurally**, not
gate-by-gate.
**Work:**
- Extract all global input handling out of `render_chart_pane` into a single
  `process_pane_input(active_pane, …)` pre-render pass, run once per frame.
- That pass emits `AppCommand`s — no direct mutation from input.
- The per-pane render loop no longer reads global keys.

**Closes:** the input fan-out class permanently (Ctrl+B-per-pane, etc. — the
session gated these by hand; this removes the hazard by design).
**Exit:** zero `key_pressed`/global-input reads inside the per-pane loop.
**Size:** M.

### Phase 3 — Persistence integrity & single path
**Goal:** one persistence path — complete, crash-recoverable.
**Work:**
- **Kill the dual-write.** The `Store<T>` aggregates become the source of
  truth; the legacy flat `Watchlist` fields become read-through accessors,
  then are removed. Every settings write goes through `update_*`.
- Migrate remaining loose persisted state into aggregates.
- Crash recovery for UI / workspace state — atomic write + recover (the
  trading WAL pattern, lighter weight).
- Extend the Phase-0 snapshot-completeness test to every aggregate.

**Closes:** settings dual-write desync; uneven crash safety.
**Exit:** one write path; a crash/restart loses nothing durable. **Size:** L.

### Phase 4 — Mutation discipline: command everything
**Goal:** every state change is one auditable command → reducer.
**Work (sub-phased by domain so each ships independently):**
- **4a** Drawings → commands (add / move / delete / restyle as `AppCommand`s).
- **4b** Viewport, pan/zoom, replay → commands.
- **4c** Toggles and per-pane UI flags → commands.
- **4d** Order *triggering* → commands (the engine is untouched; the triggers
  route through dispatch — closes the ad-hoc inline-order-submission gap).
- The reducer gains invariant assertions and a dev-mode state-transition log.

**Closes:** "no coherent mutation discipline" — yields an audit trail, a single
place to enforce invariants, testability, and undo-for-free (commands are
loggable/reversible).
**Exit:** direct `&mut` writes < ~10%, all inside the reducer. **Size:** XL —
the largest phase; ship it sub-phase by sub-phase.

### Phase 5 — Canonical model migration
**Goal:** delete the other generations.
**Work:**
- Per-chart state migrated `Chart` god-object → `ChartState` (SlotMap drawings/
  annotations/indicators, viewport). `Chart` becomes a thin handle or is absorbed.
- `Watchlist` decomposed into the aggregates + a slim root struct.
- Remove `AppCommand`-superseded inline paths and the dead scaffolding.

**Closes:** the four-generations problem → one model.
**Exit:** one per-chart model, one app model; god-objects gone. **Size:** XL.

### Phase 6 — Data layer & concurrency hardening
**Goal:** the render thread never blocks; no placeholder data.
**Work:**
- The data `STATE` (18 mutexes) → a published read snapshot (the
  `OrdersSnapshot` pattern) so render-thread reads are lock-free.
- Real DOM data — remove `generate_mock_levels`.
- Lock audit — eliminate any UI-thread stall path.

**Exit:** no render-thread lock contention; no mock data in trading surfaces.
**Size:** M–L.

### Phase 7 — Notifications, testability, observability
**Goal:** the supporting systems reach enterprise grade.
**Work:**
- A real `Notification` model — severity, source, dedup, lifecycle — replacing
  the `PENDING_TOASTS` thread-local scatter. (Mode *notices* vs event *toasts*
  are already separated by `PaneNotice`; this unifies the toast side.)
- State-logic unit + property tests on the reducers — possible once Phase 4
  lands and mutation is reducer-routed.
- Generalize the trading WAL concept into a state-transition audit log for
  dev/forensic use.

**Exit:** notifications deduped & modeled; reducers test-covered. **Size:** M.

---

## 5. Sequencing & dependencies

```
Phase 0  ──┐                              (do first — safety)
Phase 1  ──┤  (cheap decision — do early)
           ├─→ Phase 2 (input) ────┐
           ├─→ Phase 3 (persist) ──┤      (2 and 3 independent — parallel)
                                   ├─→ Phase 4 (commands) ─→ Phase 5 (model)
Phase 6  ──────────────────────────┘      (independent — slot anytime)
                                          Phase 7 (partly needs Phase 4)
```

- **Phase 0** first — it stops active data loss.
- **Phase 1** is a decision — cheap, do alongside 0.
- **Phases 2 and 3** are independent of each other — can run in parallel.
- **Phase 4** depends on 1 + 2 (commands are the input pass's target). Sub-phase
  it; each of 4a–4d ships on its own.
- **Phase 5** depends on 4 (model migration is far safer once mutation is
  command-routed).
- **Phase 6** is independent — slot it whenever.
- **Phase 7** partly depends on 4 (reducer tests need the reducer).

## 6. Effort & honesty

Phases 0–3 are a focused, achievable push — the multi-wave kind of work.
**Phases 4 and 5 are the bulk** — a sustained project, sub-phased by domain, not
a single sprint. This is a genuine rearchitecture, not a patch set. The good
news: every target component already exists (`ChartState`, the `state/` layer,
`AppCommand`, the WAL) — the work is to **finish one and remove the rest**,
not to invent.

## 7. What explicitly stays as-is

The trading order **engine** — `ORDER_MANAGER`, `OrdersSnapshot`, the WAL,
journal, circuit breaker, dedup signatures, token-bucket rate limiting, risk
limits, WAL recovery. It is already the reference standard. Phase 4d changes
only how orders are *triggered* (routing the trigger through `AppCommand`),
never the engine.
