# apex-terminal — Remediation Plan

Companion to `docs/ARCHITECTURE_AUDIT.md` (2026-07-03). This turns every audit
finding into an executable program: workstreams → tickets with acceptance
criteria (AC) → verification gates → sequencing. Sizing: S (≤half day),
M (1–2 days), L (3–5 days), XL (1–2+ weeks, incremental).

---

## 0. Operating rules (non-negotiable, apply to every ticket)

1. **The app stays shippable after every commit.** Strangler pattern, never
   big-bang (per the ApexData business-critical doctrine: correctness >
   resilience > observability > latency > features).
2. **NO live order submission, ever** — every trading-path change re-asserts
   `no_live_orders`; nothing touches `PlaceAllDraftOrders`/`submit_*` semantics
   without a paper-mode proof.
3. **Verification gate per ticket:** `cargo build` clean → targeted scenarios
   pass → **full corpus (dev/run_corpus.py, currently 1,065) green** before
   merge of any behavior-adjacent change. Refactor-only tickets may run the
   targeted subset + a nightly full corpus.
4. **New pure logic ships with unit tests** in the same commit (the harness is
   the integration layer, not a substitute).
5. **Each migration is FINISHED or DELETED — never left dual.** The audit's core
   disease is half-done cutovers; this plan bans creating new ones.
6. Every extraction/refactor preserves public behavior byte-for-byte first;
   improvements land as separate commits after the move.

---

## Workstream A — P0 Safety (live-money) — total ≈ 1–2 days

| # | Ticket | Size | Acceptance criteria |
|---|---|---|---|
| A1 | Atomic writes for `orders.json` + `control_flags.json` — route `order_manager.rs:2353,2737` through `state::persistence::atomic_write` | S | kill-switch file can never be half-written; crash-during-write test (write, kill process mid-op via test seam, reload) proves old state intact |
| A2 | `PendingSubmit`-until-Ack: persist submit as `PendingSubmit`; transition to `Working` only on broker Ack (`order_manager.rs:1290`); `replay_and_recover` treats stale `PendingSubmit` as needs-reconcile, not live | M | crash-window replay test: order persisted mid-submit reloads as PendingSubmit and reconciles by `client_order_id`; scenario asserts no phantom Working order |
| A3 | Defense-in-depth paper guard inside `LiveBroker::submit/cancel/modify` — assert-or-refuse when `is_paper_mode()`; document paper-on-restart as intentional at the field | S | unit test: LiveBroker called in paper mode returns guard error, never constructs the HTTP request; `no_live_orders` scenario unchanged |
| A4 | Release panic isolation: panic hook active in release (tracing + errors_sink); `catch_unwind` wrapper for every `thread::spawn` worker body (one helper `spawn_guarded(name, f)`); migrate shared-state `std::sync::Mutex` → `parking_lot::Mutex` (already a dep, non-poisoning) for OrderManager + feed caches | M | injected panic in a feed worker: feed dies, window survives, error surfaced; grep gate: no bare `thread::spawn` in `data/`+`trading/` (must use `spawn_guarded`) |
| A5 | Guard 3 render-thread panics: `tool_previews.rs:294/367/467` `let Some(last) = … else { return }` | S | corpus green; unwrap count in `render/` drops 7→4 |
| A6 | Kill per-frame allocs in `grade_open_plays_live` (release path): cache normalized symbol on `Play` at creation; reuse thread-local scratch map | S | no HashSet/HashMap/to_uppercase alloc per frame (code review + alloc counter); grading scenarios 2550-2555 still green |

## Workstream B — CI gates & ratchets (stop regression before touching anything big) — ≈ 2–3 days

| # | Ticket | Size | Acceptance criteria |
|---|---|---|---|
| B1 | Clippy gate: workspace `[lints]` — `warn(clippy::unwrap_used, clippy::expect_used)`, `deny(clippy::todo, clippy::unimplemented, clippy::dbg_macro)`; `cargo clippy --all-targets -- -D warnings` in CI | M | CI red on new violations; existing sites annotated `#[allow]` with justification comments (ratchet baseline) |
| B2 | Ratchet script (`dev/quality_gate.py`): per-directory unwrap budget (fail on increase; `render/` hard-cap 4 after A5), file-size ceiling (warn >2,500 / fail >6,000 LOC, grandfather core.rs+gpu.rs with linked tickets), `#[allow(dead_code)]` count ratchet | M | script runs in CI; baselines committed; intentionally adding an unwrap fails the build |
| B3 | Build matrix: `--release`, `--no-default-features` (legacy render path), `--features design-mode` (3,317-LOC bit-rot risk) | S | all three compile in CI |
| B4 | Corpus as CI gate: nightly full run (run_corpus.py writes corpus_verdict.json; fail on `real > 0`); per-PR targeted tag runs | M | red nightly on any real failure; verdict artifact uploaded |
| B5 | ux_audit becomes a failing gate (16px floor, clip, overlap) in the panel snapshot scenarios | S | design regression turns CI red |

## Workstream C — The six stalled migrations (finish or delete; explicit calls)

| # | Migration | **Decision** | Plan | Size |
|---|---|---|---|---|
| C1 | `trading/types.rs` (dead 4th order-enum copy) | **DELETE** | remove module + `#[allow(dead_code)]` registration in `trading/mod.rs:8`; keep `journal` if referenced | S |
| C2 | Wave-5 widget façades (`play_card`, `playbook_card`, `signal_card`, `trade_card`, `metric_card`) + `ui/widgets/{cards,rows}` re-export shims | **DELETE façades, collapse shims** | plays_panel documents its bespoke card as deliberate → delete `play_card.rs`; rewrite the ~2 shim importers (`spread_panel.rs:11`, `script_panel.rs:32`) to `ui/lists` paths; delete shims | S–M |
| C3 | `Timeframe` enum (exists in `chart/state`, live layer uses `String`) | **ADOPT** | introduce at the `Chart.timeframe` + `ChangeTimeframe` boundary with `Display/FromStr` (string only at HTTP edge + serde); mechanical sweep | M |
| C4 | `state/persistence.rs` envelope vs ~28 loose JSONs / 46 raw writes | **MIGRATE incrementally** | order: (1) trading files (done in A1/A2), (2) alerts/watchlists/workspace state, (3) cosmetic (ui.json, hotkeys…); every migrated file gains version+migration hook; wrap any remaining raw write in `atomic_write` | L (spread out) |
| C5 | `Store<_>` dual-write (7 mirror stores, flat fields = read source) | **FREEZE, then absorb via E3** | immediate: ban new stores + new flat-field syncs (lint comment); each Watchlist sub-context extracted in E3 becomes the single owner and its store/flat pair dies together. Do NOT continue dual-write | policy + E3 |
| C6 | `chart/state/*` canonical layer (ts_ns Drawing/Annotation) — dead | **WIRE (drawings/annotations), as part of E5/E6** | the ts_ns anchor is the correct primitive for the unified line pipeline + playbook annotations (C2 of playbook design needs it anyway); until wired, mark the tree `// TARGET MODEL — see REMEDIATION E5/E6` so it stops looking like debris | with E5/E6 |

## Workstream D — Mechanical dedup (small, high-value, do early) — ≈ 2 days total

| # | Ticket | Size | AC |
|---|---|---|---|
| D1 | One `foundation::time::now_ms() -> i64`; delete the 12 copies (i64/u64 split resolved at call sites) | S | grep: zero local `fn now_ms` outside foundation |
| D2 | One `http_client()` / `http_blocking_client()` factory (pooled, UA, default timeout); order_manager broker ops reuse it — kills the fresh-Client-per-op TLS handshake (22 sites) | M | grep: zero `Client::new()`/`Client::builder()` outside the factory; broker ops verified pooled |
| D3 | One hex parser (ui_kit), one `fmt_compact` K/M/B formatter (foundation); delete duplicates | S | grep gate |
| D4 | `APEXIB_URL` + gamma `:8412` + `APEX_SIGNALS_HTTP` consts join `data/endpoints.rs`/config modules; Discord creds resolved exe-relative (not `CARGO_MANIFEST_DIR`) | S | zero endpoint env reads outside config modules; Discord works from a copied binary dir |
| D5 | One typed dev_inspector app_state serializer: `AppStateSnapshot` struct serialized once; headless fills a subset of the same struct (kills the dual-builder drift that already bit us) | M | both paths produce identical schema; harness green |

## Workstream E — Architecture decomposition (the big one; strictly ordered, corpus-guarded) — XL, incremental

**E1. Command-bus adoption ratchet (unblocks everything).** Size M then ongoing.
- Add `dev/quality_gate.py` check: count direct `watchlist.* =` / `chart.* =`
  assignments in `ui/`; fail on increase (baseline 776).
- Migrate hotspots opportunistically with each ticket below; UI emits
  `AppCommand`, reducer mutates. AC: baseline number only ever decreases.

**E2. Extract domain types out of gpu.rs.** Size L.
- Move `Chart` → `chart/model.rs`, `Watchlist` → `app/state.rs`, Theme/Layout →
  `chart/theme.rs`, the 21 persistence fns → `persistence/workspace.rs`, window
  loop + GpuCtx → `app/window_loop.rs`. Pure `git mv`-style moves + re-exports
  first (zero behavior change), then delete the `chart_renderer` alias re-exports
  once imports are rewritten. AC: gpu.rs < 2,500 LOC, contains only wgpu; corpus
  green after each move.

**E3. Split Watchlist into owned sub-contexts.** Size XL (one context at a time).
- Order (smallest coupling first): `PanelVisibility` (~40 bools) →
  `LayoutState` (splits/undo) → `ScannerState` → `RrgState` → `PlaybookState`
  (incl. the 20 editor fields) → `OptionsChainState` → `ChatState` → `Settings`.
- Each extraction: new struct owns the fields; `Watchlist` holds it; accessors
  keep call sites compiling; the corresponding `Store<_>` mirror (C5) is deleted
  in the same commit. AC per context: field count on Watchlist drops; no dual
  source remains for that domain; corpus green.

**E4. Execute PANE_RS_SPLIT_PLAN on `render_chart_pane`.** Size XL.
- Phase extraction order: `hit_test.rs` → `input.rs` (the drag/priority chain) →
  `overlays.rs` (gamma/strikes/plays) → `order_layer.rs` → `drawings.rs` →
  `axes.rs`/`candles.rs`. Each phase takes `&mut PaneRenderCtx`; behavior-
  preserving moves only. AC: no function > 400 lines in render/pane/; core.rs
  < 3,000 LOC at completion; corpus green after each phase.

**E5. Orders-as-view + unified line pipeline.** Size L–XL.
- `chart.orders` becomes a projection of `OrderManager::all_order_levels_for()`
  (delete the per-frame three-way merge + both manual insert paths).
- One `ChartLine { kind: Alert|Play|Order|Drawing|Trigger }` hit/drag/render
  pipeline replacing the five parallel ones; single priority table; shared
  badge/dash spec (fixes the affordance divergence too). AC: adding a line kind
  = one enum variant + one style row; snap/drag scenarios green.

**E6. Wire the canonical drawing/annotation model (C6).** Size L.
- Converge renderer `Drawing` onto `chart/state/drawings.rs` (ts_ns anchors);
  wire `Annotation` as the playbook per-level/callout backing store (playbook
  design §C2 dependency). One `Bar` boundary: `foundation::types::Bar` on the
  wire, explicit `From` into the GPU-packed struct at upload (kills the
  timestamps-desync class). AC: drawing_db codec and live model share one type;
  bars+timestamps zip removed.

**E7. Feed generic + UI I/O eviction.** Size M.
- `Subscription<T> { url_fn, parse_fn }` over resilient_ws collapses the
  per-feed shells (verify signals_v2 isn't a second transport); the 4 reqwest +
  8 thread-spawn sites in `ui/` move behind commands→`data/`. AC: zero
  reqwest/spawn in `ui/` (lint), one subscription engine.

## Workstream F — Observability — ≈ 2–3 days

| # | Ticket | Size | AC |
|---|---|---|---|
| F1 | Prometheus counters: per-feed connect/disconnect/stale, broker submit/reject/latency, cache hit/miss, errors_sink severity counts | M | metrics visible on :9091; Grafana panel for broker reject rate |
| F2 | `apex_data/ws.rs` 11 eprintln → errors_sink/tracing; Redis/Postgres misconfig surfaced as persistent in-app indicator (not one-time stderr) | S | zero eprintln in data/ (lint); status chip shows cache/persistence state |
| F3 | Synthetic/stale data badges: gamma synth + any placeholder chain rendered with a "SYNTHETIC" tag (copy the DOM panel's LIVE/SIMULATED pattern); fetch failures log at Warn | S | scenario: SynthGamma → capture shows synthetic=true; badge assertable |
| F4 | Daily-loss P&L labeled "advisory (local estimate)" in UI; prefer broker-reported realized P&L when present | S | UI label; no logic change |

## Workstream G — Design/UX polish — ≈ 1 week, parallelizable

| # | Ticket | Size | AC |
|---|---|---|---|
| G1 | `overlay_palette(t)` tokens; migrate core.rs's 205 hardcoded colors (start: play zone bands `:7938-7959` — lines already tokenized) | L | hardcoded-color ratchet: render/ count only decreases; theme-switch scenario shows overlays retint |
| G2 | Consolidate on ui_kit `Modal`/`Toast`: collapse 3 `dialog_window*` factories + raw `egui::Window` toasts/dialogs (≥10 sites) | M | one z-order/dismiss/backdrop authority; grep gate on new `egui::Window` in ui/ |
| G3 | `auto_chart_panel` → kit primitives (the lone raw-egui panel) | S | zero raw ui.button/checkbox in panels/ (except kit.rs) |
| G4 | Central shortcut table (`foundation/shortcuts.rs`): action→binding registry feeding palette + hotkey_editor; migrate the 21 scattered Key:: sites | M | one dispatch table; hotkey editor reads/writes it |
| G5 | Unit tests for pure engines (H4): grade_play, arm_branches, snap_price, resolve_level_expr, option_payoff_at, apply_scanner, eval_formula + proptest invariants (payoff monotonicity, snap idempotence) | M | `#[cfg(test)]` modules exist; B-gate requires them for future edits |
| G6 | Docs hygiene: date-stamp/prune stale plan docs; crate rename `_scaffold_lib`→`apex_terminal` + drop alias re-exports (do LAST — pure churn) | S–M | builds green; no functional change |

---

## Sequencing & dependency graph

```
Week 0   A1–A6 (safety)  ──►  B1–B5 (gates)          [nothing big moves before gates]
Week 1   C1,C2,C3 (quick deletes/adopts) + D1–D5 (dedup) + G5 (engine tests) + F1–F4
Week 2+  E1 ratchet on ──► E2 gpu.rs extraction ──► E3 Watchlist split (absorbs C5)
                        └► G1–G4 in parallel (design polish, independent)
Later    E4 pane split ──► E5 orders-as-view + line pipeline ──► E6 canonical model (closes C6)
         E7 feed generic
Last     G6 crate rename
```

Hard dependencies: **B before E** (no decomposition without ratchets); **E1
before E3** (can't split state that's mutated from 776 places); **E2 before E3**
(types must move before splitting); **E5/E6 after E4** (line pipeline lives in
the extracted modules); **C5 dies inside E3**; **C6 closes inside E6**.

## Effort summary

| Phase | Content | Effort |
|---|---|---|
| P0 | A (safety) | 1–2 days |
| P1 | B + C1–C3 + D + F + G5 | ~1.5–2 weeks |
| P2 | E1–E7 (+C4 spread through) | 4–8 weeks, incremental, corpus-guarded |
| P3 | G1–G4, G6 | ~1 week, parallelizable with P2 |

## Definition of done (program level)

- Zero P0/High audit findings open; kill-switch write atomic; no phantom-Working window; paper guard in broker.
- CI: clippy gate, unwrap/size/mutation/color ratchets, 3-target build matrix, nightly corpus, ux_audit failing gate — all red-on-regression.
- Exactly **one** of everything: order model, Bar, drawing model, line pipeline, app_state serializer, now_ms, http client, persistence pattern.
- No file > 6,000 LOC; no fn > 400 lines in render/pane; command-bus mutation baseline at 0 for new code.
- Chart overlays fully tokenized (theme switch retints the data surface).
- The corpus (≥1,065 scenarios) green throughout — it is the contract that none of this changed behavior.
