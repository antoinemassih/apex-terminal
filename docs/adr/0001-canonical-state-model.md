# ADR 0001 — Canonical State Model

**Status:** Accepted · **Date:** 2026-05-22 · **Supersedes:** —

## Context

The Apex Terminal state system is **four overlapping, each-incomplete
generations** of architecture (see `docs/STATE_SYSTEM.md` for the full audit):

1. `Watchlist` (~150 fields) + `Chart` (~200 fields) god-objects, mutated by
   direct `&mut` field writes — ~80% of all mutation.
2. The `AppCommand` queue + reducer — a real command system, but at ~5% coverage.
3. The `state/` aggregate layer (`Store<T>` + supervisor + 7 `Persistable`
   aggregates) — well-designed, but the aggregates *mirror* `Watchlist` fields
   rather than own them.
4. `chart/state/ChartState` — a `SlotMap`-based canonical chart model — fully
   designed but **not wired** at runtime.

Each generation was started and never finished. The cost: no single source of
truth, no mutation discipline, a class of per-pane input-fan-out bugs, and
persistence desync. The full plan to resolve this is `docs/STATE_ROADMAP.md`.

## Decision

There is **one** target architecture. All future state work moves toward it;
the other three generations are migration debt to be removed, not extended.

- **Per-chart state → `chart/state/ChartState`.** The `SlotMap`-based model is
  canonical for drawings, annotations, indicators, and viewport.
- **App / global state → the `state/` aggregate layer.** Aggregates are the
  **source of truth**, not mirrors. Persistence flows through `Store<T>`.
- **All mutation → `AppCommand` → reducer.** Every state change is one
  auditable command. Direct `&mut` field writes are legacy.
- **Input → one per-frame pass** for the active pane, emitting commands —
  never read inside the per-pane render loop.

## Freeze

Effective immediately, the god-objects are **frozen**:

- **Do not add new fields to `Watchlist` or `Chart`.** New per-chart state goes
  on `ChartState`; new app/UI state goes on a `state/` aggregate.
- New mutation goes through `AppCommand`, not a direct `&mut` write.
- Exceptions require a follow-up ADR.

This freeze is also recorded in `src-tauri/CLAUDE.md`.

## Consequences

- A migration project (`STATE_ROADMAP.md`, phases 2–5) decomposes the
  god-objects and routes mutation through the reducer.
- Short-term, all four generations still coexist — the freeze stops the
  divergence widening while the migration proceeds.
- The trading order **engine** (`ORDER_MANAGER`, WAL, snapshot, circuit
  breaker) is already at the target standard and is explicitly out of scope of
  the decomposition — only how orders are *triggered* changes.
