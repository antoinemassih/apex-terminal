# Apex Terminal — Audit Fix Plan

Companion to [`AUDIT.md`](./AUDIT.md). That document is the findings; this is
the **plan to fix them** — ~150 findings grouped into 9 fix waves, sequenced by
priority and risk.

---

## Principles

1. **Incremental, always green.** Every wave builds clean (all feature configs)
   and ends with the suite passing. No wave left half-applied.
2. **Crashes and money first.** Render-thread panics and order-path risk holes
   are fixed before cosmetic or perf work.
3. **`core.rs` stays single-owner.** Hot-path / sacred-file changes are made by
   one owner, benchmark-aware, never as a mechanical sweep.
4. **The money path is verified in paper mode**, not just by a green build.
5. **Each fix cites the `AUDIT.md` finding ID it closes** — nothing is dropped.
6. New regression **tests** are added with the fix wherever the bug is
   testable (NaN sorts, ADX/VWAP math, persistence round-trips, risk gates).

---

## Wave 1 — Render-thread crash hardening
**Closes:** E1, E2, E3, E4 (Critical) + the High/Medium panic sites
(`pending_pts` indexing, alert `find().unwrap()`, order-lookup `find().unwrap()`
×5, Discord `OnceLock::get().unwrap()`, `unreachable!()` in the pane picker).
**Approach:** replace every render-thread / handler panic site with a safe
fallback — `unwrap_or`, `.get()` + `if let`, `unwrap_or(Ordering::Equal)`,
clamp-then-index. No behaviour change on the happy path.
**Files:** `core.rs` (single-owner), `gpu.rs`, `order_manager.rs`,
`discord.rs`, `tool_previews.rs`.
**Tests:** NaN-strike sort, empty-`bars` pane, stale `theme_idx`, empty `panes`.
**Risk:** Low — each diff is tiny and local. **Effort:** S. **Supervised:** no.

## Wave 2 — Money-path risk hardening
**Closes:** T1, T2, T3, T4 (Critical) + trading-High (OCO/conditional/combo skip
risk checks; broker-error uncancelable order; paper-toggle mid-flight; OCO draft
limbo) + trading-Medium (hotkey/DOM `last_price: 0.0`; WAL rotation read; N-leg
OCO pairing; default risk limits).
**Approach:**
- **T4 first** — introduce an explicit `APEX_TRADING_MODE=paper|live` config;
  remove the `APEXIB_URL.contains("dev")` heuristic everywhere.
- **T1** — add a realized-P&L accumulator (from filled orders); enforce
  `max_daily_loss` in a pre-submit check.
- Extract a single `validate_risk(intent)` helper from `submit()`; call it from
  `submit()`, `confirm()` (**T2**), `submit_oco/conditional/combo`.
- **T3** — delete the raw `DELETE /orders` thread; `cancel_all_orders` already
  routes through the broker.
- Thread real `last_price` into hotkey/DOM intents; concat `orders.wal.1` in
  WAL replay; fix N-leg OCO ring-linkage; lower default risk limits.
**Files:** `order_manager.rs`, `keyboard_shortcuts.rs`, `core.rs` (DOM intent),
config plumbing.
**Tests:** daily-loss rejection, `confirm()` blocked under kill/halt, risk
helper coverage for every submit path.
**Risk:** High — real money. **Effort:** L. **Supervised:** YES — verify in
paper mode before any live use.

## Wave 3 — Memory-leak caps
**Closes:** M1, M2, M3 (Critical) + memory-High (undo/redo cap, `live_state`
HashMap purge, projector thread pool) + memory-Medium (`symbol_history`,
`perf_log` sessions, `remove(0)`→`VecDeque`, per-order `state_history`).
**Approach:** add a cap + front-trim to every unbounded collection; bind
`live_state` maps to the watched-symbol set with a periodic evict tick; replace
unbounded feed channels with bounded + lossy-drop-oldest; share one projector
thread pool.
**Files:** `gpu.rs`, `live_state.rs`, `providers/apex_data.rs`, the feed files,
`monitoring.rs`, `perf_log.rs`.
**Risk:** Low–Medium (channel change needs care). **Effort:** M.
**Supervised:** no.

## Wave 4 — Security & dependency Criticals
**Closes:** S1, S2, S3 (Critical) + security-High (hardcoded IPs, world-readable
URL log, unencoded URL input) + dependency-High (null CSP, unscoped shell
capability, unverified sidecar) + the `transmute`/`getrandom`/`bytemuck` Mediums.
**Approach:** bump `image` 0.24→0.25; add an OAuth2 `state` nonce + sanitize the
callback; set a restrictive CSP; scope the shell capability; checksum the
sidecar; `urlencoding::encode` user input in URLs; fail-hard instead of
hardcoded IP fallbacks; log path-only; `bytemuck` for GPU POD casts; bound the
`text_engine` transmute with a `'static` signature.
**Files:** `Cargo.toml`, `discord.rs`, `tauri.conf.json`,
`capabilities/default.json`, `config.rs`, `rest.rs`, `trendline_filter.rs`,
`text_engine.rs`, GPU pipeline files.
**Risk:** Low–Medium. **Effort:** M. **Supervised:** no.

## Wave 5 — Data-correctness fixes
**Closes:** D1, D2 (Critical) + data-High (DST offset, hardcoded `"5m"`, VWAP
session reset, ADX Wilder smoothing, sim-price warning) + data-Medium (Ichimoku
Chikou direction, DOM `as u32` UB, RSI float guard) + data-Low (Renko volume,
incremental-update NaN, range-bar tick order).
**Approach:** fix the MTF indicator to read `source_bars_closes`; label the DOM
"SIMULATED" (or gate it off until a real L2 feed exists); compute the ET offset
from the date (EDT/EST) — shared helper, reuse for 0DTE; carry the real
timeframe in the IB tick command; per-session VWAP reset; Wilder (RMA) smoothing
for ADX; flip the Chikou shift; saturating/typed cast for the DOM hash.
**Files:** `gpu.rs`, `compute.rs`, `core.rs`, `ib_ws/mod.rs`, `dom_panel.rs`,
`fetch.rs`.
**Tests:** ADX vs a reference series, session-VWAP reset, MTF source selection,
the EDT/EST boundary.
**Risk:** Medium (math correctness). **Effort:** M. **Supervised:** no — but
worth eyeballing an indicator against a known-good platform.

## Wave 6 — Persistence atomicity & integrity
**Closes:** P1, P2 (Critical) + persistence-High (watchlist dual-write order, PG
drawing-save retry, `find_or_create_chart` TOCTOU + `UNIQUE`, version-field
migration) + persistence-Medium (supervisor shutdown signal, `atomic_write`
`sync_all` + parent-dir fsync, corrupt-file user notice, DB/JSON reconciliation).
**Approach:** wrap `intern_style` in a transaction (or `INSERT … ON CONFLICT`);
stage `save_templates` into a temp dir then rename; reverse the watchlist
write order; add a bounded retry buffer for PG drawing saves; add the `UNIQUE`
constraint + `ON CONFLICT`; read & dispatch on the `version` field; give the
persist supervisor a shutdown channel; check `sync_all`, fsync the parent dir;
surface a toast on load failure.
**Files:** `drawing_db.rs`, `watchlist_db.rs`, `gpu.rs`, `state/persistence.rs`,
`state/persist_supervisor.rs`.
**Risk:** Medium. **Effort:** M–L. **Supervised:** no.

## Wave 7 — Performance Criticals
**Closes:** PF1, PF2, PF3 (Critical) + perf-High (`get_theme`/`style::current()`
re-locking, watched-set rebuild, band `Vec<Pos2>` allocs, 6× `srgb_to_linear`)
+ perf-Medium (`HashSet` for `selected_ids`/`hidden_groups`, strike-sort cache,
`pane_rects` cache, tab-header `Vec` cache, auto-SR clustering).
**Approach:** guard `request_repaint()` on animation state; cache auto-S/R keyed
on `(bars.len, vs, vc)`; hoist order-sync above the pane loop; read theme/style
once per frame and pass `&` down; dirty-flag the watched set; reuse scratch
buffers; one `#[inline(always)] srgb_linear`.
**Files:** `core.rs` (sacred — single-owner, benchmark each change), `gpu.rs`,
`style.rs`.
**Risk:** Medium–High (hot path). **Effort:** L. **Supervised:** YES — measure
frame time before/after each change; eyeball idle CPU.

## Wave 8 — Concurrency hardening
**Closes:** CC1, CC2 (Critical) + concurrency-High (lock-poisoning cascade,
unbounded channels [shared with Wave 3], runtime-per-call) + concurrency-Medium
(detached-thread panic logging, nested `block_on`, `WAL_LOCK` across `fsync`).
**Approach:** return data from `with_mgr` and publish/append *after* the guard
drops; document + `debug_assert` the re-entrant-lock hazard; replace bare lock
`.unwrap()` with `unwrap_or_else(|e| e.into_inner())` on every global; one
shared `Arc<Runtime>`; move WAL writes to a dedicated I/O thread; name spawned
threads and log their panics.
**Files:** `order_manager.rs`, `snapshot.rs`, `wal.rs`, `gpu.rs`, `style.rs`,
`lib.rs`, the feed/fetch files.
**Risk:** Medium. **Effort:** M–L. **Supervised:** no.

## Wave 9 — Medium/Low cleanup & process
**Closes:** the remaining Medium/Low — `draw_tool` → enum, `u8` pseudo-enums →
typed enums, dead-field/`#[allow(dead_code)]` trim, `eprintln!`→`tracing`,
Redis-URL zeroing, `Relaxed`→`Acquire/Release` atomics, the 63 TODO triage,
missing module docs, `jank.jsonl` rotation.
**Process:** add `cargo audit` + `cargo clippy -D warnings` to CI; add a
`cargo-deny` license check; begin chipping at the ~500 compiler warnings.
**Risk:** Low. **Effort:** M (spread out). **Supervised:** no.

## Deferred — Architecture (A1, A2)
The `render_chart_pane` 10.5k-line function and the `Chart`/`Watchlist`
god-objects are **already** the subject of `STATE_ROADMAP.md` Phase 5 — not
re-planned here.

---

## Sequencing

```
Wave 1 (crashes) ──┐  do first — cheap, stops hard crashes
Wave 2 (money) ────┤  high priority — SUPERVISED (paper-mode verify)
                   ├─→ Waves 3,4,5,6,8 — largely independent, parallelizable
Wave 7 (perf) ─────┘  hot path — SUPERVISED (benchmark)
Wave 9 (cleanup) ──── ongoing, last
```

- **Waves 1–2 first** — crashes then money.
- **Waves 3, 4, 5, 6, 8** touch mostly disjoint files (memory / security /
  data-math / persistence / concurrency) — they can run in parallel.
- **Wave 7** is the hot path — sequence it deliberately, one change at a time,
  benchmarked.
- **Wave 9** runs alongside everything as cleanup.

## Autonomous vs. supervised

- **Autonomous (build + test verified):** Waves 1, 3, 4, 5, 6, 8, 9.
- **Supervised (needs live verification):** Wave 2 (paper-mode money path) and
  Wave 7 (frame-time benchmarking on the sacred hot path).

## Effort

Waves 1, 3, 4, 5, 6, 8 are a focused multi-wave push. Wave 2 and Wave 7 are the
careful ones. None of this is the multi-week scale of the state rearchitecture —
it is a large but bounded bug-fix project.
