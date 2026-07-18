# Apex Terminal — World-Class Execution Plan
**2026-07-18 · Derived item-by-item from `WORLD_CLASS_AUDIT_2026-07-18.md` (+appendix, +DOM second opinion)**

This is the work list. Each item is written for an implementing agent with **no
prior session context**: what to do, where, how to verify, what it depends on.
Work items in ID order within a wave unless a dependency says otherwise.

---

## 0. STANDING RULES FOR EVERY IMPLEMENTING AGENT — read before any item

1. **Never submit live orders.** All testing in paper mode (`paper_mode` defaults
   true). Never flip `armed`/live in a test. Never call the broker bridge with
   real intent.
2. **All sites or none.** If an item names N sites, land all N or land nothing.
   Partial fixes recorded as complete are this codebase's dominant defect class
   (see AUDIT_2026-07-17.md). If you cannot finish, leave the branch unmerged
   and write down exactly which sites remain.
3. **The badge pattern is law.** Any rendered data that is not real-and-live
   gets the DOM ladder's LIVE/SIMULATED treatment (`dom_panel.rs:271-313` is the
   reference implementation). Never delete a feature when a badge will do;
   never badge when the fix is cheap.
4. **Corpus-gate everything that touches the render path or state.**
   `python dev/run_corpus.py` from **repo root** (not src-tauri/). 1067
   scenarios against a real window on port 7892. Launch it **detached**
   (PowerShell `Start-Process` or `subprocess.Popen(creationflags=0x8)`) — the
   harness kills backgrounded tasks. Trust ONLY the log's `=== VERDICT` line +
   the `corpus: refreshed apex-native-corpus.exe` line (no refresh line = you
   tested a stale binary) + zero "connection could be made" errors. The verdict
   JSON file is git-tracked and can be stale. The app's process name during a
   run is `apex-native-corpus`, NOT `apex-native` — do not "fix" a run that
   looks dead without checking the right name. Progress prints every 50
   scenarios; ~40s silence is normal.
5. **Build etiquette:** kill the running exe before `cargo build` (it holds the
   file lock). Never run >2 concurrent cargo builds (build-lock saturation
   wedges the machine). A killed mid-compile build corrupts
   `target/debug/incremental` → LNK1120 on serde_json symbols → `rm -rf
   target/debug/incremental`. `cargo build --lib` does NOT compile test cfg —
   run `cargo test --lib` too. Never `taskkill node.exe` (crashes the shell).
6. **No screenshots for verification** — native windows can't be captured
   reliably; verify via build + unit tests + corpus + code review. Never
   Win32-poke (SetWindowPos/ShowWindow) a live wgpu window — it destroys it.
7. **Known threading patterns:** adding a DOM in/out signal = 3 files
   (`dom_panel.rs::draw()` signature → `ui/pane.rs` DomPaneAdapter →
   `core.rs` adapter construction ~1490). Sidebar booleans must route through
   `watchlist.update_sidebar_state(|s|…)` or the store→flat sync overwrites
   them every frame. `DomLevel` has 3 constructors (mock `dom_panel.rs:63` +
   `dom_feed.rs:142/150`) — new fields go in all three.
8. **Commit trailers** (every commit):
   `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>` and
   `Claude-Session: https://claude.ai/code/session_011KDC3NUsrPXq7VNjQA645b`.
   Push to remote `gitea`, branch `feature/automated-scenario-testing` (or a
   child branch of it). `git add` explicit paths only — a co-tenant session
   shares this repo and `git add -A` sweeps their files.
9. **Definition of done, per item:** code + unit tests for the specific defect +
   `cargo test --lib` green + (if render/state touched) corpus 1067/1067 with a
   fresh-binary refresh line + the item's own acceptance checks + audit doc
   status updated **with evidence inline** (commit hash, test names, corpus
   verdict). Never mark an item closed without the evidence sentence.

---

# WAVE 0 — CLOSE THE MONEY EDGES
*Goal: live trading is trustworthy. Nothing here is architecturally hard.*

> **PROGRESS 2026-07-18 (batch 6)** — CI/test only (no corpus needed):
> - **W0-13a DONE** `0bfa4b30` — full `cargo test --lib` now runs in CI (was
>   compile-check only; ~800 tests incl. the trading suite never ran). Also fixed
>   a Windows-only test the full run surfaced (flush_all_returns_failure_for_bad_
>   path assumed a POSIX-unwritable path). Full suite 818/0.
> - **W0-13b DEFERRED** — the strict zero-bar oracle (canvas_all_finite /
>   viewport_sane FAIL when a pane loaded a symbol but has 0 bars). With the
>   current FLAKY FEED environment, scenarios intermittently load 0 bars for
>   environmental reasons; enabling the strict oracle now would false-red every
>   corpus run and destroy the gate. Land when feeds are reliably up.
>
> **PROGRESS 2026-07-18 (batch 5)** — corpus **1067/1067** (0 real, 0 refused):
> - **W0-08 DONE** `c87b1db6` — order-path staleness gate: get_snapshot_with_age
>   + RiskLimits.max_quote_age_secs (5s default); BP snapshot fallback rejects a
>   stale quote (fail-closed, overridable). +1 test (116 total).
> - **W0-10 DONE** `16b1a638` — command palette destructive actions (flatten/
>   cancel-all/reverse/half-size) need a two-step confirm (arm → red banner →
>   second Enter); Esc cancels; chains never auto-run them.
> - **W0-14 DEFERRED** — the finding's "compact DOM ladder fabricates depth" code
>   is NOT in orders_panel (which renders positions, not a synthetic ladder).
>   Needs the exact file:line from the data-pipeline appendix before acting —
>   not guessing at a fabricated-depth fix.
>
> **PROGRESS 2026-07-18 (batch 4)** — each corpus **1067/1067** (0 real, 0 refused):
> - **W0-12 DONE** `686782b1` — footprint overlay badged "ESTIMATED · shape from
>   OHLC, not tape" with the fabricated order-flow specifics removed (delta,
>   buy/sell %, conviction, EXHAUSTION, TRAPPED, imbalance "N:1 BUY @ price");
>   only OHLC-real reads kept (direction, RVOL, wick geometry). Historical CVD:
>   per-bar `cvd_synthetic` flag → synthetic spans rendered dimmed + "dim =
>   estimated (no tape)" legend. Real tape footprint remains W2-06.
> - **W0-11 DONE** `91b8e968` — paper→LIVE now arms an explicit red "Go LIVE"
>   confirmation (toggle springs back to Paper until confirmed); disabled while
>   kill/halt engaged; armed state is a non-persisted static.
>   NOTE: first gate was self-inflicted-killed at 901/1067 (cleanup Stop-Process
>   ran before the verdict) — re-ran clean to a true 1067/1067.
>
> **PROGRESS 2026-07-18 (batch 3)** — corpus **1067/1067** (0 real, 0 refused):
> - **W0-01 DONE** `c4455441` — Spread Builder blocked from submitting a live
>   combo at conId=0 / $0 limit (validate_risk was toothless at $0). Guard in
>   submit_combo + honest UI note. +1 test (115 total).
> - **W0-09 DONE** `8867df39` — welcome-wizard daily-loss cap now applied to
>   RiskLimits.max_daily_loss on Finish (was captured then discarded); enforced
>   by the W0-05/06 breaker. Position-% honestly disclosed as Settings-configured
>   (no account-aware conversion at first run).
>
> **PROGRESS 2026-07-18 (batch 1)** — landed together, corpus **1067/1067** on the
> fresh binary (0 real failures, 0 connection-refused), 110 trading unit tests:
> - **W0-02 DONE** `6c13590b` — broker submit/cancel/modify check HTTP status.
> - **W0-03 DONE** `d01f18be` — cancel → PendingCancel (not terminal); a fill
>   racing the cancel is now adopted by reconcile, not masked. +3 tests incl.
>   the core `w0_03_pending_cancel_broker_filled_adopts_fill_not_masked`.
> - **W0-04 DONE** `d72fe80b` — rejected modify reverts optimistic price +
>   reconcile limit-price self-heal. +4 tests.
> - **W0-07 DEFERRED (needs product decision)** — market orders already hit the
>   notional `NeedsApproval` gate (validate_risk:954, uses last_price when
>   price=0); the price-deviation fat-finger is legitimately N/A for market
>   orders. A real gap exists only when `max_notional` is disabled (0). Adding a
>   second notional threshold risks a redundant/confusing gate. Decide the
>   threshold semantics (notional cap vs qty-baseline anomaly) before building.
>
> **PROGRESS 2026-07-18 (cont.)** — daily-loss hardening, corpus **1067/1067**
> (0 real, 0 refused), 114 trading unit tests:
> - **W0-05 DONE** `4aa1f40f` — daily-loss accumulator persisted via atomic
>   sidecar (daily_loss.json), restored on restart, rollover-reset. +2 tests.
> - **W0-06 DONE** `4aa1f40f` — cap counts unrealized P&L (flag, default ON) +
>   auto-halt on breach from the reconcile poll (paper-guarded). +3 tests.
>   Open positions are NOT auto-flattened by design (halt new risk only).

### W0-01 · Spread Builder: block conId=0 / $0-limit live combos  [P0 · M]
**Evidence:** spread_panel.rs builds combo legs with `conId=0` and submits with
limit price `$0.00`; `submit_combo` path applies no fat-finger/notional gate to
combos.
**Do:**
1. In the combo submit path (`order_manager.rs::submit_combo` and/or the
   spread_panel call site): reject any combo where any leg has `con_id == 0`,
   or where order_type is limit with `limit_price <= 0.0`. Reject = same
   `OrderResult::Rejected(reason)` + toast pattern as other risk rejections.
2. Route combos through `validate_risk` (notional check using a real quote —
   see W0-08 for staleness; use best-effort quote now, staleness gate lands
   separately).
3. Until W2-02 (options chain unification) supplies real conIds, the Spread
   Builder's submit button must be disabled with an explanatory tooltip when
   legs lack resolved conIds — not silently rejected after click.
**Accept:** unit tests: combo with conId=0 rejected; $0 limit rejected; valid
combo passes. No corpus impact expected (no combo scenarios) — run anyway.

### W0-02 · LiveBroker cancel/modify must check HTTP status  [P0 · S]
**Evidence:** `broker.rs:380-408` — `cancel()`/`modify()` discard status via
`.map(|_| ())`; a broker rejection reads as success.
**Do:** return `Err` on non-2xx (mirror `lookup_by_client_id` at broker.rs:410-449
which does it correctly). Then audit the two call sites' Err handling:
cancel-path must NOT leave the optimistic local `Cancelled` standing on Err
(see W0-03, do together), modify-path must revert (see W0-04, do together).
**Accept:** unit tests with a mock broker returning 4xx/5xx: cancel failure
leaves order active + reports; modify failure reverts price. All existing 103+
trading tests stay green.

### W0-03 · Optimistic local Cancelled can permanently mask a real fill  [P0 · M]
**Evidence:** `order_manager.rs:~2453` sets `Cancelled` optimistically before
the broker call resolves; the reconcile loop's absorbing terminal-state rule
then refuses to resurrect the order when the broker reports it actually FILLED
in the race window. Ok/Err match at ~2468-2470 never re-checks.
**Do:**
1. Change cancel to the same PendingX pattern the submit paths use: introduce
   `PendingCancel` (or reuse state machinery) — order stays active-looking
   ("Cancelling…") until broker Ack; on Ack → `Cancelled`; on Err → revert to
   prior state + toast.
2. In the reconcile matched-order rules (~3379-3970): a broker-reported
   `Filled` must override a locally-optimistic `Cancelled` **when the
   cancellation was never Acked**. Scope carefully: do NOT let ordinary
   broker-lag resurrect genuinely-cancelled orders — key on "local Cancelled
   without broker Ack recorded".
3. Journal both transitions.
**Accept:** unit test reproducing the race: submit → local cancel → broker
reports Filled → local state ends Filled with fill recorded, position/PnL
correct. Plus: cancel-Ack normal path, cancel-Err revert path. Corpus run
(order scenarios exercise cancel).
**Depends:** W0-02 (Err must be visible to react to).

### W0-04 · Failed order-modify permanently desyncs displayed price  [P0 · M]
**Evidence:** `order_manager.rs:2599` sets `o.price` optimistically before the
broker call; live-path Err branch (~2650) journals a Fail but never reverts;
reconcile matched-path (~3472-3567) syncs state/filled_qty/avg_fill_price but
never `price` from the broker's `limit_price`.
**Do:**
1. Capture pre-modify price; on Err (now real, per W0-02) revert `o.price`,
   clear `modify_pending_price`, toast "modify rejected: <reason>".
2. In reconcile matched-order path: sync `o.price` from broker `limit_price`
   when no modify is in flight (`!modify_inflight`) — trust broker like
   avg_fill_price already is.
**Accept:** unit tests: failed modify reverts; reconcile heals a drifted price;
in-flight modify not clobbered by reconcile (coalescing keeps working — the
`modify_version` tests must stay green). Corpus (DOM drag scenarios).
**Depends:** W0-02.

### W0-05 · Daily-loss breaker resets to $0 on restart  [P0 · M]
**Evidence:** `new()` zeroes `realized_pnl_today`/`daily_loss_date`;
`load_from_disk` restores only orders; `save_to_disk` never writes them;
`record_fill_pnl` sole call site order_manager.rs:3590.
**Do:**
1. Persist `realized_pnl_today` + `daily_loss_date` in the existing orders
   snapshot/state file (same atomic write path); restore on load; reset only
   when the restored `daily_loss_date` ≠ today (ET, using the existing
   17:00-ET boundary logic from commit bf04a9a4).
2. WAL: also journal PnL records so crash-between-snapshots recovers (replay
   should rebuild `realized_pnl_today` from today's Fill events — check
   `replay_and_recover`'s Filled branch, which currently mutates fill fields
   but never calls record_fill_pnl; route it through).
**Accept:** unit tests: save/load round-trips the two fields; date rollover
resets; WAL replay rebuilds today's realized PnL. Kill-and-restart manual test
in paper mode.

### W0-06 · Daily-loss gate: include unrealized PnL + auto-halt on breach  [P1 · M]
**Evidence:** `validate_risk` (order_manager.rs:1038-1049) tests only
`realized_pnl_today`; `AccountSummary.unrealized_pnl` is populated
(mod.rs:224,470-497) but never read by the gate; breach only rejects NEW
submits — nothing halts or flattens.
**Do:**
1. Gate on `realized_pnl_today + unrealized_pnl` (config flag
   `daily_loss_includes_unrealized`, default ON).
2. On breach: auto-engage the existing `halt_inner()` path (cancel-alls remain
   manual — halting new risk is the conservative default; flatten stays a
   user decision) + prominent toast + errors_sink Critical.
**Accept:** unit tests both flag states; breach engages halted; resume works.
**Depends:** W0-05 (persistence must be right before auto-halt keys off it).

### W0-07 · Market orders skip the fat-finger check  [P0 · S]
**Evidence:** known-open from AUDIT_2026-07-17 — fat-finger price-distance check
applies to limit orders only (~order_manager.rs:914); market orders bypass it.
**Do:** for market orders, fat-finger against qty×last_price notional (the
last_price is already on OrderIntent) with its own threshold; keep the existing
limit-price-distance check for limit/stop-limit.
**Accept:** unit tests: oversized market order rejected; normal one passes.

### W0-08 · Staleness: timestamp the quote cache + gate the order path  [P1 · S/M]
**Evidence:** live quote cache has NO timestamp; `live_state.rs:35` stores
`(Snapshot, Instant)` but `get_snapshot` (~:444) discards the Instant; the
BP/notional pre-check (order_manager.rs:965-979) consumes ageless data; DOM
ladder / watchlist / spread panel all consume ageless quotes. The correct 2s
pattern already exists (`core.rs:1456-1457`, `dom_panel.rs:662`).
**Do:**
1. Add `get_snapshot_with_age()` (returns the stored Instant); keep
   `get_snapshot` as a wrapper. Add a timestamp to the quote cache entries.
2. Order path: in `validate_risk`'s BP/notional check, if the quote is older
   than N seconds (config, default 5s) → reject with "market data stale" —
   fail-closed, overridable via the existing `override_warnings` intent flag.
3. Surface age: DOM already badges; add the same stale badge to the order
   panel's quote display and spread panel.
**Accept:** unit tests (fresh passes, stale rejects, override works). Corpus.

### W0-09 · Wizard risk limits: wire Finish to the risk gate  [P0 · S]
**Evidence:** welcome wizard captures daily loss cap / max position % then
discards them — Finish handler never calls into trading config.
**Do:** on Finish, write the captured values into the same persisted risk-limit
config `validate_risk` reads (`risk_limits`), via the existing update path
(`update_trading_*`). Show the applied values in the completion step.
**Accept:** unit test on the handler fn; manual: set in wizard → visible in
settings → a violating paper order is rejected.

### W0-10 · Command palette: confirm destructive account-wide actions  [P0 · S]
**Evidence:** palette executes cancel-all/flatten-all class actions on a single
Enter, zero confirmation.
**Do:** tag destructive palette entries; on Enter show the existing confirm
modal (ui_kit Modal/DialogWindow — see apex-dropdown-modal-unification) with
action summary; Enter-Enter (type-ahead) must not skip it.
**Accept:** corpus scenario driving the palette (`execute.rs` has palette
plumbing; add a scenario asserting order_count unchanged until confirm).

### W0-11 · Paper→Live: confirmation gate  [P1 · S]
**Evidence:** `settings_panel.rs:700-704` — single unconfirmed toggle;
`set_paper_mode` (order_manager.rs:3291-3302) blocks only PAPER-while-orders-
active; PAPER→LIVE is unguarded.
**Do:** modal on PAPER→LIVE: explicit "type LIVE to confirm" (or hold-to-
confirm) + display of account + current risk limits. Also gate LIVE with: kill
switch must be disengaged, broker connected.
**Accept:** manual + unit test on the guard fn.

### W0-12 · Footprint + historical-CVD: disclosure badges  [P0 · S]
*(The real tape-backed footprint is W2-06; this is the immediate stop-lying fix.)*
**Evidence:** `gpu.rs:4330-4379` `bar_micro_profile` fabricates per-price
volume (Gaussian around close) and buy-ratio (`0.3 + 0.5*level_pos`);
`core.rs:12308-12470` renders "ABSORPTION"/"EXHAUSTION"/"TRAPPED"/"3:1 BUY"
tags. CVD: `gpu.rs:4398-4422` backfills bars older than process start with the
same heuristic; `core.rs:5837, 12629-12660` renders synthetic+real as one line.
**Do:**
1. Footprint overlay: render an "ESTIMATED — not tape data" badge (warn tone,
   same pattern as dom_panel.rs:271-313) whenever the overlay is on; **remove
   the confidence tags** (ABSORPTION/TRAPPED/3:1) from the estimated path
   entirely — a badge does not license fake specifics.
2. CVD: render the synthetic backfill segment dashed + dimmed with a legend
   marker ("synthetic before <time>"), or clip the line to real data with a
   visible gap. The boundary index is known (first bucket in realized_delta).
**Accept:** corpus-neutral (scenarios have no live tape → verify no visual
regression via corpus); code review; screenshot-free verification per rule 6.

### W0-13 · Corpus oracles: fail (not pass) on zero bars + CI full test suite  [P0 · S]
**Evidence:** `dev_inspector/assert_engine.rs` L915-951 (`canvas_all_finite`),
L955-976 (`viewport_sane`) pass vacuously when a pane has 0 bars; capture's
n==0 fallback (price_low,high)=(0.0,1.0) makes viewport_sane pass with no
data. The 2026-07-17 GREEN run rode through a real feed outage undetected.
CI runs only ~30% of the 769 unit tests.
**Do:**
1. In both asserts: if the scenario loaded a symbol (any `SwapPaneSymbol`/
   load step) and the pane's bar_count == 0 → FAIL with "no data loaded".
   Add an explicit opt-out assert (`allow_empty_pane`) for the scenarios that
   legitimately run dataless (grep scenarios for panes that never load).
2. Add a `cargo test --lib` job (full suite, not the 229-test allowlist) to
   `.github/workflows/quality-gates.yml` (copy the existing connectivity-ci
   job shape).
**Accept:** corpus still 1067/1067 on a healthy feed; a deliberately dead-feed
run now FAILS the affected scenarios (verify once manually); CI job green.
**⚠** This may surface latent scenario failures — fix scenarios (opt-out) not
oracles.

### W0-14 · Compact DOM in order panel: stop fabricating depth  [P1 · S]
**Evidence:** the order-entry panel's compact ladder synthesizes book depth
beyond top-of-book from the single quote.
**Do:** either render only the levels actually known (top-of-book + last) with
honest empty rows, or drive it from the real dom_feed when subscribed, with
the SIMULATED badge otherwise. No synthetic depth without a badge.
**Accept:** code review + corpus.

---

# WAVE 1 — STOP THE UI LYING (durability + alerts + dead controls)

> **PROGRESS 2026-07-18 (Wave 1 started)** — corpus-unreachable (PG-persistence
> layer; corpus runs without a database), verified by unit tests + build:
> - **W1-01 DONE** `4c2cf077` — drawing persistence no longer drops 16 of 22
>   kinds. Root cause: two non-isomorphic DrawingKind enums — gpu.rs emits rich
>   strings ("hzone"/"channel"/"gannfan"/"elliott"/…), drawing_db re-parsed them
>   through a crude 13-variant enum that knew only 6. Fix: the drawing_type
>   STRING is now source-of-truth (preserved in extras JSONB, never rejected;
>   load prefers it). +3 unit tests. No schema migration.
> - **W1-02a DONE** `ad4e51fe` — drawing-persistence failures surfaced instead of
>   lost: PG-unavailable at startup → errors_sink warning (was a bare eprintln);
>   dead-letter queue now reports each drop + spills to state/drawings_dead_
>   letter.jsonl (lossless past cap 64); is_persisting() accessor added.
> - **W1-05 DONE** `bb529a3c` (corpus 1067/1067) — price alerts evaluate against
>   their OWN symbol (symbol-keyed live snapshot when the pane shows a different
>   symbol), so they no longer silently die when a pane switches symbol while
>   still shown ACTIVE. Pure alert_eval_price helper + 3 unit tests.
> - **W1-11 DONE** `739745e4` (corpus 1067/1067) — the spike-popup "[view
>   provenance]" / trade-plan "[🔍 prov]" buttons rendered live but their
>   on_open_provenance callback was never registered → dead. Now wired at startup
>   (provenance_pane::wire_provenance_buttons → request_open).
> - **W1-06 NOT STARTED (do fresh)** — hotkey wiring is a 30-site rewiring of
>   TRADING hotkeys (buy/sell/kill/flatten) where a wrong action-mapping is a
>   live-trading bug and corpus drives these via DevInput. default_hotkeys()
>   already defines all 27 actions with keys MATCHING the hardcoded sites, so the
>   fix is: a binding_pressed(ui, hotkeys, action) helper + replace each
>   hardcoded ui.input(key_pressed) site with it. Not deep-context work.
> - **W1-02b DEFERRED** — the async reconnect loop (re-establish the worker when
>   PG returns, buffer saves-while-down, drain the JSONL spill) + wiring the
>   is_persisting() status chip into the UI. Needs a fresh session (async
>   redesign, not a one-liner).

### W1-01 · Drawing persistence: unify the two DrawingKind enums  [P0 · M]
**Evidence:** two non-isomorphic DrawingKind enums; converter exists in gpu.rs
but a silent string-alias mismatch at the DB layer (`drawing_db.rs:488`
`parse_kind`) drops ~16 of 22 kinds into the dead-letter queue forever.
**Do:**
1. Make one enum canonical (the persisted one); the renderer enum maps to/from
   it EXHAUSTIVELY — replace stringly matching with a `match` that the
   compiler forces complete (no wildcard arm).
2. Round-trip test: every DrawingKind variant → serialize → parse_kind →
   identical. This is the regression net; write it FIRST (it fails today for
   16 kinds — that's the proof).
3. Migration: attempt re-parse of existing dead-lettered rows on startup once.
**Accept:** the round-trip test over all variants; manual: draw one of each,
restart, all restored.

### W1-02 · Drawing persistence: surface failure + reconnect  [P0/P1 · M]
**Evidence:** PG connect failure at startup → eprintln only, DB_TX unset,
drawings silently session-only all session; dead-letter queue (cap 64) is
write-only — never drained, never surfaced.
**Do:**
1. Route connect failure + every dead-letter event through
   `errors_sink::report()` (Warn) → toast + diagnostics panel.
2. Persistent status chip whenever DB_TX is unset: "drawings not saving".
3. Background reconnect loop (backoff, reuse resilient patterns from
   `data/connectivity`); on reconnect, drain the dead-letter queue.
4. Dead-letter overflow: spill to a local JSONL file so nothing is lost even
   past cap 64; drain that too.
**Accept:** unit test the drain; manual: start with PG down → chip visible →
bring PG up → drawings flush, chip clears.
**Depends:** W1-01 (no point flushing rows that can't parse).

### W1-03 · Watchlist persistence: fix destructive save + phantom fallback  [P0 · M]
**Evidence:** `load_watchlists()`'s comment promises a Postgres cross-machine
fallback that does not exist (`load_all()` is orphaned, zero callers); save is
destructive delete-and-reinsert — a fresh machine loads empty defaults and the
first save wipes the server copy.
**Do:**
1. Implement the fallback for real: local file missing/empty → `load_all()`
   from PG before accepting defaults.
2. Make save non-destructive: upsert by watchlist id; delete only rows the
   user explicitly deleted (tombstones or explicit delete calls).
3. Never overwrite server state with empty local defaults: if local is
   default-empty and server has data → server wins + toast.
**Accept:** unit tests: fresh-machine restore; empty-local doesn't wipe;
rename/delete round-trips.

### W1-04 · Alerts: real out-of-app delivery  [P0 · M]
**Evidence:** alert "sound" is a literal `eprintln!`; no OS notification, no
push, nothing reaches a trader who stepped away.
**Do:**
1. Real sound: bundle a short WAV, play via a tiny audio dep (e.g. `rodio`)
   or winapi PlaySound — config on/off + volume.
2. Windows toast notification (e.g. `tauri-winrt-notification`/`winrt-toast`
   class crate) with symbol/price/direction; click focuses the app window.
3. Optional webhook URL (config): POST alert JSON — this gives
   phone push via any relay the user wants without building push infra.
4. All three behind per-alert + global settings; test-fire button in the
   alerts panel.
**Accept:** manual fire in paper mode; unit test the dispatch fan-out (mock
sinks). Corpus-neutral.

### W1-05 · Alerts: decouple evaluation from pane symbol  [P0 · M]
**Evidence:** a price alert silently stops evaluating — while still listed
ACTIVE — when the pane it was created on switches symbol.
**Do:** evaluate alerts against a symbol-keyed quote source (`live_state`
snapshot per alert.symbol), NOT the owning pane's current chart. The alert
store already holds the symbol. If a symbol has no live subscription, either
auto-subscribe (bounded set, use the SubscriptionManager refcounting) or mark
the alert DEGRADED (badge) — never silently dead.
**Accept:** unit test: alert on SPY, pane switched to QQQ, SPY tick still
fires it. Corpus scenario added (alerts accumulate — use state_field_gte).

### W1-06 · Hotkey editor: dispatch from user bindings  [P0 · M]
**Evidence:** remapping is cosmetic for ~26 of 27 bindings — only the
tps_toggle boss key reads `watchlist.hotkeys`; `keyboard_shortcuts.rs`
hardcodes the rest.
**Do:** make `watchlist.hotkeys` the single dispatch source: iterate the
binding table (action → chord) in keyboard_shortcuts.rs, matching the
already-working tps_toggle pattern; delete the hardcoded matches. Conflict
detection on edit (two actions, one chord → warn). Reserved chords
(Ctrl+Shift+K kill switch) protected from remap-to-nothing: remappable but
never unbound.
**Accept:** unit test the dispatch table; corpus scenarios that fire hotkeys
via DevInput still pass (update scenarios if they assumed hardcoded keys);
manual remap round-trip.

### W1-07 · Gap-fill-on-reconnect: route to the real delivery path  [P0 · M]
**Evidence:** `SubscriptionManager.gap_fill_on_reconnect_all` is fully built +
unit-tested (refcounting, last_seen_ts, REST replay) but sends bars into a
broadcast fanout with ZERO live receivers at every production call site —
`providers/mod.rs:20-28` documents it.
**Do:** deliver replayed bars through the same path live bars take:
`NATIVE_CHART_TXS` / `send_to_native_chart` with the existing LoadBars
gen-guard semantics (bars must carry the pane's current request gen or gen=0
merge semantics — study the `LoadBars` handler dedup so backfill can't
clobber a newer symbol/tf switch; the gen-guard work from commit 43cc242c is
the reference).
**Accept:** unit test the routing; manual: kill the WS mid-session (paper),
reconnect, verify the gap bars appear. Corpus.

### W1-08 · Cold start: move PG connect off the render thread + visible failure  [P1 · S]
**Evidence:** cold-start blocks the render thread on a Postgres connect before
first frame; release builds swallow the failure invisibly.
**Do:** spawn the connect on a worker (the app already has the pattern for
feeds); first frame renders immediately; failure surfaces via W1-02's status
chip + errors_sink.
**Accept:** manual cold start with PG down: window appears fast, chip shows.
Corpus (startup scenarios).

### W1-09 · Replay: wire the on-chart overlay  [P0 · M]
**Evidence:** the replay pane's "overlay replay on live chart" checkbox is
non-functional (replay_pane.rs never calls the overlay hooks), BUT a complete
overlay render pipeline (type, API, full paint pass) already exists in
gpu.rs/core.rs — this is wiring, not building. (This was the one unverified
P0 — verify the claim first, then wire.)
**Do:** connect replay_pane's playback ticks to the existing overlay API; the
replay provider (`providers/replay.rs`) already serves bars. Play/pause/speed
drive overlay updates; exiting replay clears overlay state.
**Accept:** corpus scenario: enable replay overlay, step N ticks, assert
overlay bar count grows; disable, assert cleared.

### W1-10 · Alerts-on-drawings: wire the math  [P1 · M]
**Evidence:** the UI toggle is a fire-and-forget curl with discarded result;
the trigger math (price crossing a trendline's y-at-time) is unwired.
**Do:** evaluate drawing-anchored alerts client-side in the same evaluation
pass as W1-05 (compute the drawing's price at current time from its anchors —
the drawing geometry types already expose endpoints); fire through W1-04's
delivery. Server round-trip only for persistence.
**Accept:** unit test y-at-time math; corpus scenario with a seeded trendline
alert + GradePlaysAtPrice-style synthetic price step.
**Depends:** W1-04, W1-05.

### W1-11 · Provenance buttons: register or remove  [P1 · S]
**Evidence:** "[view provenance]"/"[🔍 prov]" render as live controls; their
callbacks are never registered — clicking does nothing.
**Do:** wire them to the provenance_pane if the data path exists; otherwise
remove the buttons (kill-or-finish rule). Decide per the appendix evidence.
**Accept:** no rendered control does nothing on click (spot-check panel).

### W1-12 · Research panel: empty sections disclose or die  [P1 · S/L]
**Evidence:** Fundamentals/Analyst/Earnings/Insider/Econ-Calendar sections are
permanently empty — no fetch path populates them.
**Do (choice, per section):** (a) wire a real fetch if the backend exists, or
(b) apply the "not connected" pattern already used by chart_widgets (honest
empty state naming the missing feed), or (c) remove the section. NO section
may render an empty table that looks like "no results today".
**Accept:** every research section either shows data or says why not.

---

# WAVE 2 — DAILY-DRIVER PARITY

### W2-01 · Paper trading: real fill/PnL/position simulation  [P0 · XL]
**Evidence:** `paper.rs` is a 19-line ACK-only stub; paper orders never fill;
paper-mode panels show the REAL IB account's positions/PnL.
**Do (staged, each stage shippable):**
1. **Stage A — fills:** a tick-driven engine: on each quote/trade for a
   symbol with Working paper orders — market fills at quote±slippage model
   (configurable bps), limit fills when price crosses (at limit price), stop
   triggers→market, stop-limit triggers→limit. Partial fills optional later.
   Write fills through the SAME path live fills take (`record_fill_pnl`,
   state transitions, journal) so every downstream consumer works unchanged.
2. **Stage B — paper account:** paper positions/PnL ledger, separate from the
   IB account snapshot; panels read paper ledger when `paper_mode` — the
   account/positions display must switch source, clearly labeled PAPER.
3. **Stage C — realism:** queue-position estimate for limits at touch,
   configurable latency, OCO/bracket sibling semantics via the same
   pair-integrity sweep.
**Accept:** unit suite per order type (fill/no-fill matrices); bracket
paper-fills trigger sibling cancels; corpus keeps `no_live_orders` green;
paper panels never show the live account.
**Depends:** W0-03/W0-04 land first (state machine stable).

### W2-02 · One real options chain + real conIds for Spread Builder  [P1 · L]
**Evidence:** a live hand-rolled chain EXISTS in `watchlist_panel.rs`
WatchlistTab::Chain (~1489-2067: strikes/bid/ask/OI/IV, click-to-add) while a
built `OptionChainRow` widget (greeks/IV/OI columns) and a tested
`IvRankWidget` sit UNUSED; Spread Builder needs resolved conIds (W0-01).
**Do:**
1. Build ONE chain panel on `OptionChainRow` + `IvRankWidget` + the existing
   `options_analytics_cached` (GEX/IV/PCR already real) + the existing
   `fetch_chain_background` data path (rows carry conIds — verify; if not,
   extend the backend fetch to include them).
2. Chain rows → "add leg to Spread Builder" carrying conId + real quote →
   unlocks W0-01's disabled submit.
3. Retire the watchlist-tab chain (or make it render the same component).
**Accept:** chain shows greeks/OI/IV for SPY 0-14DTE; Spread Builder legs have
conIds; combo submit passes the W0-01 gate in paper mode. Corpus scenarios
for the panel (presence + `dom_spread_sane`-style structure oracle).

### W2-03 · DOM: stop/stop-limit order types from the ladder  [P1 · S/M]
**Evidence:** `enum DomOrderType { Market, Limit }` (dom_panel.rs:32); combo
UI dom_panel.rs:406-423; OrderManager fully supports Stop/StopLimit/Trailing
(order_manager.rs:405).
**Do:** extend DomOrderType (Stop, StopLimit); ladder click above market =
buy-stop / below = sell-stop semantics with side-aware placement; wire to
existing submit path. Param threading rule 7 applies.
**Accept:** paper-mode manual; corpus DOM scenarios extended (SeedDraftOrder
style asserts on order_type).

### W2-04 · DOM: bracket templates + BE/trail actions  [P1 · M]
**Evidence:** Shift+click bracket hardcodes STOP_TICKS=10/TARGET_TICKS=20
(core.rs:1642-1643) while a real user-editable `BracketTemplate` system
(Tight/Normal/Wide/Scalp) exists wired to the chart context menu
(trading/mod.rs:737-741, pane_context_menu.rs:159-166) — the DOM never reads
`chart.bracket_templates`. No move-to-breakeven or one-click-trail UI exists
anywhere despite `trail_amount`/`trail_percent` being first-class fields.
**Do:**
1. DOM bracket uses the active BracketTemplate (default selectable in DOM
   header); remove the hardcoded consts.
2. Working-order chip context menu: "Move stop to BE" (modify stop leg to
   avg_fill ± offset) and "Convert to trailing" (modify with trail_amount).
   Modifies route through the existing modify pipeline (post W0-04).
**Accept:** paper-mode manual; unit test BE price calc; corpus.

### W2-05 · DOM/order-flow: session-depth tape + frame coalescing  [P1 · M]
**Evidence:** volume/absorption signals computed over the last ~100-trade
window, not the session (per-price accumulators reset by window); DOM handler
recomputes the full 6-hashmap analytics per WS message (gpu.rs:7941 drain →
3085-3179), not per frame — cost scales with market speed.
**Do:**
1. Coalesce: in the command drain, collapse a backlog of same-symbol
   DomLevels to the newest before recompute (keep the newest levels, run
   analytics once per frame).
2. Session depth: maintain per-symbol running per-price accumulators
   (volume/delta/buy/sell) accrued at trade ingestion (extend the
   `realized_delta` minute-bucket pattern in live_state to per-price), so the
   ladder reads session truth instead of re-scanning a 4000-trade window.
3. Add this scenario to the perf benchmark + an `fps_above` corpus assert.
**Accept:** analytics outputs unchanged on a quiet tape (unit-diff old vs new
on a recorded tape fixture); corpus fps assert added; visual parity.

### W2-06 · Real tape-backed footprint (replaces W0-12's estimated mode)  [P1 · L]
**Evidence:** the tape data needed already exists via the same `tape_for()`
the DOM uses; the fabrication is gpu.rs:4330-4379.
**Do:** per-bar per-price bucket aggregation from the tape (reuse the
gpu.rs:3097-3131 aggregation pattern, keyed by bar timestamp range); render
real buy/sell splits; keep the ESTIMATED badge for bars outside tape
coverage; re-enable computed tags ONLY on real segments and only with tested
threshold logic (see W2-07).
**Accept:** unit test bucketing on a fixture tape; badge only on synthetic
bars.
**Depends:** W0-12, W2-05 (session accumulators feed this).

### W2-07 · Order-flow signal thresholds: test + validate against a recording  [P2 · M]
**Evidence:** absorption/pull/big-print math (gpu.rs:3148-3173) hand-tuned,
zero unit tests, never validated live; corpus DOM scenarios assert structure
only.
**Do:** record one live session's tape to a fixture (the replay provider can
capture); build unit tests with synthetic tapes containing known
absorption/pull events asserting flags fire (and DON'T fire on quiet tape);
tune thresholds against the recording; document per-symbol scaling.
**Accept:** the new tests; a written validation note in docs/DOM_TRADER_DESIGN.md.

### W2-08 · FIFO realized PnL  [P2 · M]
**Evidence:** `record_fill_pnl` (order_manager.rs:1080-1101) picks cost basis
by most-recent opposite fill (`max_by_key(updated_at)`); comment admits
multi-lot is wrong. Feeds the daily-loss gate.
**Do:** per-symbol FIFO lot queue (open lots list; fills consume from front);
persists with the W0-05 state. Keep the conservative-bias doc comment honest.
**Accept:** unit tests: scale-in/scale-out sequences vs hand-computed PnL;
partial-lot consumption.

### W2-09 · Bracket/OCO sibling cross-cancel backstop  [P1 · M]
**Evidence:** TP/SL both pair_id→entry, never each other (submit_bracket
~1656-1682); pair-integrity sweep (~3687-3715) only cancels the order AT
pair_id → filled TP never cancels SL locally; OCO star topology (legs 2..N →
leg[0] only).
**Do:** generalize `pair_id` to `group_id` + role; sweep rule: any member of a
group going terminal-filled cancels all other active members (bracket: TP
fill cancels SL and vice versa; OCO: any fill cancels the rest). Entry-fill
must NOT cancel TP/SL (roles matter). Migrate existing persisted orders
(pair_id → group of 2) in load.
**Accept:** unit matrix: bracket TP fill → SL cancelled; SL fill → TP
cancelled; entry fill → children untouched; 3-leg OCO any-fill →
others cancelled. Existing pair tests stay green.

### W2-10 · Reconcile/account polling through the Broker trait  [P1 · L]
**Evidence:** the fill/position poller bypasses the Broker trait with
IB-hardcoded HTTP — multi-broker readiness lags the trait.
**Do:** lift the poller's HTTP calls into trait methods
(`open_orders()`, `positions()`, `account_summary()`); LiveBroker implements
with the current code; MockBroker implements for tests; reconcile consumes
the trait. Do NOT reshape conId here (that's W4-03).
**Accept:** reconcile unit tests run against MockBroker (they largely exist —
port them); no behavior change live.

### W2-11 · CI: nightly corpus + perf regression gate + binary build  [P1 · M]
**Evidence:** corpus never runs in CI; perf report is a stale May snapshot
with no regression gate; CI has never compiled apex-native (only
`cargo check --lib`).
**Do:**
1. CI job: `cargo build --bin apex-native --release` on every push (windows
   runner).
2. Nightly (self-hosted runner on the dev box — the corpus needs a GPU +
   display): detached corpus run, publish verdict as artifact, fail on <1067.
   Respect rule 4 (port 7892 contention: skip if port busy).
3. Perf: re-run the PERF_REPORT benchmark scenarios (with DOM open, per
   W2-05) and add an fps floor assert to the nightly.
**Accept:** green runs of all three; a deliberate regression (test branch)
trips the gate.

### W2-12 · Indicator golden-value oracles  [P2 · M]
**Evidence:** 17 of 20 indicator compute fns have no golden-value test
(Bollinger/SMA/Stoch have them; the rest don't).
**Do:** fixture OHLCV series + independently-computed expected values (python
reference in dev/) for EMA/RSI/MACD/ATR/VWAP/Ichimoku/etc.; tolerance asserts.
Priortize the ones with recursive state (EMA-family) — that's where silent
drift lives (see also persona-contributor finding: incremental update path
silently NaNs non-special-cased indicators — fix that here: make the
incremental path fall back to full recompute for any indicator not
special-cased, never NaN).
**Accept:** all 20 compute fns oracled; incremental-vs-full recompute parity
test.

### W2-13 · Design-system gates: revive both  [P1 · S]
**Evidence:** `scripts/.design-system-baseline.txt` references deleted paths →
design-system-check permanently red with 454 phantom violations;
`scripts/sx_ratchet.sh` exists but is not wired into CI.
**Do:** regenerate the baseline against current paths; wire sx_ratchet.sh into
quality-gates.yml; both fail-on-increase.
**Accept:** both jobs green on main, red on a test violation.

---

# WAVE 3 — THE DIFFERENTIATOR (scripting / vibe coding)
*Sequence is strict: W3-01 → W3-02 → W3-03/W3-04 → W3-05.*

### W3-01 · Indicator trait + registry  [P1 · XL]
**Evidence:** IndicatorType is a flat enum inside gpu.rs; adding one indicator
touches ~140 match sites across 6+ files (gpu.rs, core.rs, indicators_panel,
indicator_editor, persistence, compute) — measured by the contributor-persona
agent.
**Do (staged):**
1. Define `trait Indicator { fn id(&self) -> &str; fn compute(&self, &Ohlcv,
   &Params) -> IndicatorOutput; fn params(&self) -> ParamSpec; fn
   render_style(&self) -> RenderStyle; }` in a NEW module outside gpu.rs
   (start the domain-extraction direction, W4-01, without waiting for it).
2. Registry: id → factory. Port indicators one-by-one, each a separate
   corpus-gated commit; the enum delegates to the registry during migration
   (enum variant → registry id) so both paths coexist.
3. Persistence: indicators persist by string id + params (migration from enum
   ordinal); label/editor UI reads ParamSpec generically.
4. Ratchet: add an enum-match-site counter to quality_gate.py; drive down.
**Accept:** per-indicator: corpus + golden oracle (W2-12) unchanged. End
state: adding an indicator = one file implementing the trait + registration.
**⚠** render hot-path warning: keep compute out of the per-frame path exactly
as today (computed on data change, cached) — PANE_RS_SPLIT_PLAN.md rules
apply; benchmark before/after per stage.

### W3-02 · Embed Rhai: custom indicators  [P1 · L]
**Depends:** W3-01.
**Do:**
1. Add `rhai` dep. Sandbox config: no file/net/system access, op-limit +
   timeout per eval, memory cap.
2. `ScriptIndicator` implements the W3-01 trait: script defines
   `compute(bars) -> series` (+ params declaration). Compile once, eval on
   data change only — never per frame.
3. Editor: the existing script_panel becomes real for indicators — source
   persisted (per-script files under a `scripts/` user dir), errors surfaced
   inline (compile + runtime), REMOVE the SIMULATED badge only for this path
   once real.
4. Ship 3 example scripts (SMA-cross marker, session VWAP bands, custom CVD
   coloring) as templates — the presets list already exists in script_panel.
**Accept:** unit: script indicator computes on fixture and matches a native
equivalent; a deliberately-infinite script is killed by op-limit; corpus.

### W3-03 · Script hooks: signal → AppCommand (order path)  [P1 · L]
**Depends:** W3-02.
**Do:**
1. Expose a curated host API to scripts: read-only market data (bars, quote,
   tape summary, gamma levels), and an `emit(command)` that accepts a
   WHITELISTED subset of AppCommand variants (draft orders, alerts, pane ops
   — NOT live submit in v1; scripts create Draft orders the trader confirms).
2. Event loop: scripts can register on_bar / on_tick / on_timer callbacks;
   run them on the data thread with the same op-limits.
3. Risk framing: script-emitted intents carry `source: Script{id}` so
   validate_risk can apply a per-script order-rate limiter; kill switch halts
   script emission globally.
**Accept:** example strategy script creates a draft bracket on a signal in
paper mode; kill switch stops it; rate limiter test.

### W3-04 · dev_inspector: hardened release subset  [P1 · L]
**Evidence:** the real automation API (HTTP /cmd /input /assert, drivable) is
compiled out of release; ~45 AppCommand variants debug-only.
**Do:**
1. Feature-gate a release "automation" server: OFF by default, enabled in
   settings; binds 127.0.0.1 only; bearer token generated + shown in
   settings; allowlist = the same curated command subset as W3-03 (+ read
   endpoints /app-state /widget-tree). NO DevInput synthetic clicks in
   release. Rate-limited.
2. Document the API (docs/AUTOMATION_API.md) with 3 recipes (add indicator,
   set alert, export state).
**Accept:** release build: server off by default; with token, curl can add an
alert; wrong token 401; live-order commands rejected even with token.

### W3-05 · The vibe-coding loop (LLM in-app)  [P1 · L]
**Depends:** W3-02/03/04.
**Do:** wire the script_panel's AI prompt to a local LLM CLI (the sibling
supermodel project's pattern: stream a local `claude` CLI with
`--strict-mcp-config`; see its src/chat implementation for the
process-spawn/streaming shape): prompt + current script + the host-API doc as
context → generated Rhai → show diff → user applies → hot-reload → errors
loop back. Config: CLI path; graceful "no CLI found" state (honest empty
state, not a dead button).
**Accept:** with a local claude CLI: prompt → running indicator in <60s;
without: honest disabled state.

### W3-06 · Playbook sharing: surface the built backend  [P1 · M]
**Evidence:** publish/feed/fork/Discord-embed backend is complete
(playbook_store.rs) with ZERO reachable UI, compiled out of release.
**Do:** compile it into release; add Feed/Publish/Fork controls to
playbook_panel (the panel exists); Discord share behind confirm (it currently
stashes, never posts — decide: post for real with webhook config, or remove).
Scripts (W3-02) join the same share format later — design the payload with a
`kind: play|script` field now.
**Accept:** publish → appears in feed → fork round-trip in paper/dev env.

---

# WAVE 4 — PUBLIC PRODUCT

### W4-01 · Extract the domain model from gpu.rs  [P1 · XL]
**Evidence:** Chart struct (246 fields, gpu.rs:2198-2536), Watchlist (148
fields, gpu.rs:5578-5922), IndicatorType, theme registry all live inside the
renderer; 11 data/feeds files + persistence + foundation import
chart_renderer::gpu; chart↔data circular dep (2/11 feed files read renderer
config directly, e.g. drawings_feed.rs:94 gpu::auto_draw_config()).
**Do (staged, corpus-gated per slice — PANE_RS_SPLIT_PLAN.md rules):**
1. First break the 2 direct feed→gpu config reads: invert to the push pattern
   (feeds send through channels; config pushed to feeds on change).
2. Move plain-data types (SavedWatchlist, theme registry types, IndicatorType
   post-W3-01, DomLevel, alert types) into `src/domain/` — no egui/wgpu
   imports allowed there (enforce with a quality-gate grep ratchet).
3. Chart/Watchlist: extract state-only structs the render structs embed
   (composition, not a big-bang split).
4. THEN evaluate a crate split (domain / data / render) for compile times.
**Accept:** per slice: corpus 1067 + benchmark parity. Ratchet:
gpu-import-count from data/persistence/foundation must only decrease.
**⚠** Do NOT extract candle/axis core geometry (breaks rendering — historical
constraint; the deferral is documented and benchmark-justified).

### W4-02 · AppCommand adoption drive  [P1 · L]
**Evidence:** 266 direct `watchlist.field =`/`chart.field =` mutations in ui/
(quality_baseline.json, MUT_RE); watchlist_panel.rs alone has 38 with zero
AppCommand use.
**Do:** panel-by-panel migration to bus dispatch (worst offenders first:
watchlist_panel); ratchet `ui_direct_mutation` down with each PR (gate
already counts it — tighten the baseline per landing).
**Accept:** ratchet strictly decreasing; corpus per panel.

### W4-03 · Broker trait: ContractRef (de-IB the arg types)  [P1 · L]
**Evidence:** ConditionalCondition (broker.rs:161-166) + ComboLeg (202-207)
require `con_id: i64`; LiveBroker::submit (332-336) hard-fails without conId;
planned brokers (Tradier/Tasty/Schwab/Alpaca — see ApexBroker plan) trade by
symbol/OCC string.
**Do:** `enum ContractRef { IbConId(i64), Occ(String), Symbol(String) }` in
the shared arg structs; IB impl resolves internally (existing
resolve_contract); MockBroker stops fabricating conids. Land AFTER W2-10 so
the whole broker surface moves at once.
**Accept:** trait compiles with a stub Tradier impl (submit-only, feature-
gated, no live creds) proving the shape; all IB paths unchanged (unit +
paper manual).

### W4-04 · Broker endpoint: runtime config + auth  [P0(public) · L]
**Evidence:** `APEXIB_URL` compile-time const (trading/mod.rs:22) — the
live-order/cancel/modify/kill-switch wire — hardcoded to a private dev
domain, ZERO authentication; a parallel config fn
(config.rs apexib_url/set_apexib_url) is fully dead code (zero callers).
**Do:** delete the dead config fns OR make them the single source (pick one —
recommend: make config.rs's precedence chain real and used); every broker
call site reads runtime config; add bearer-token auth header (token from OS
keychain via the existing discord_keychain.rs pattern); server side of auth
is an ApexIB change — coordinate (ApexIB currently has 116 no-auth routes;
see apex-security memory).
**Accept:** no compile-time URL consts on the order path (grep gate); calls
carry auth; missing config = orders disabled with honest UI state, app
otherwise fine.

### W4-05 · Endpoint hygiene sweep  [P2 · M]
**Evidence:** xllio.com / 192.168.x defaults baked across feeds (endpoints.rs
etc.), some rendered in UI.
**Do:** one config layer (file + env override) for every endpoint; defaults
become empty-with-honest-degradation (the Yahoo bar fallback proves the
pattern); homelab values move to the owner's local config file.
**Accept:** grep gate: no 192.168./xllio literals in src (allowlist the
config-default file if kept); clean checkout with no config runs with
Yahoo bars + everything else honestly degraded.

### W4-06 · README, LICENSE, CONTRIBUTING  [P1 · S]
**Evidence:** README.md is one line; no LICENSE anywhere (blocks any public
use legally); no CONTRIBUTING.
**Do:** real README (what it is, screenshots later, build steps that a
stranger can follow, backend-optional notes, safety disclaimer — this is a
live-trading tool); LICENSE (owner's call — flag: dependency licenses were
audited permissive, so MIT/Apache-2.0 both viable); CONTRIBUTING.md pointing
at the corpus + quality gates + this plan.
**Accept:** a fresh clone on a clean Windows box builds by following README
verbatim (test it).

### W4-07 · Packaging + release pipeline  [P1 · L]
**Evidence:** zero installer/signing/release infra; build script hardcodes a
personal npm path; version 0.10.0 with no release discipline.
**Do:** cargo-dist or WiX/NSIS installer job in CI; remove the hardcoded
path; versioned GitHub/Gitea releases with changelogs; (code signing = later,
costs money — document the gap).
**Accept:** CI produces an installer artifact that installs+runs on a clean
VM.

### W4-08 · Cross-platform build check  [P2 · M]
**Evidence:** winit/wgpu/egui are portable; cfg-gating exists with
non-Windows fallbacks; but no Linux/macOS build has ever been exercised
(known Windows-only: GDI screenshot capture, Win32 window styling, PlaySound
if chosen in W1-04).
**Do:** CI matrix job `cargo build --bin apex-native` on ubuntu-latest +
macos-latest; fix what breaks behind cfg gates; do NOT chase full runtime
parity yet — compile-clean is the milestone.
**Accept:** three-platform build matrix green.

### W4-09 · Offline/demo mode  [P2 · M]
**Evidence:** bar loading already falls back to public Yahoo (5-tier
FallbackProvider); live streaming has no fallback; mock providers exist but
aren't user-reachable.
**Do:** a first-run "demo mode" toggle: mock/replay providers registered as
the live source (the provider registry supports this), demo badge globally
(badge law), bundled sample session for replay.
**Accept:** fresh install, no config, demo mode: charts + DOM + paper trading
all function with badges.

### W4-10 · Rename the crate + cosmetic identity  [P3 · S]
**Evidence:** crate `_scaffold`, authors `["you"]`, dir `src-tauri` (not
Tauri).
**Do:** rename crate to `apex-terminal` (check workspace refs + CI + corpus
launcher exe names — dev/run_corpus.py resolves exe by name!), fix authors,
optionally rename src-tauri/ → src/ LAST (touches everything; do in a quiet
window, corpus-gate).
**Accept:** build + corpus green post-rename.

---

# CONTINUOUS / POLICY ITEMS (not one-shot)

### C-01 · Ratchet additions to dev/quality_gate.py
Add counters (fail-on-increase): unbadged-fallback renders (grep
`generate_mock|synthetic` in render paths lacking badge call), IndicatorType
enum-match sites (W3-01), gpu-imports-from-lower-layers (W4-01), direct
mutations (exists — tighten per W4-02).

### C-02 · Audit-doc discipline
Every item closed in this plan gets its audit-doc entry updated **in the same
PR** with: commit hash, tests added, corpus verdict. The 2026-07-17 audit
exists because closures were recorded without evidence. Do not repeat it.

### C-03 · The kill-or-finish queue (from the product-finish honesty ledger)
For each remaining SCAFFOLD/MOCK surface in the appendix's product-finish
section not covered above: decide kill vs finish vs badge in a weekly pass.
Nothing ships rendering fake data unbadged — C-01's ratchet enforces.

---

# DEPENDENCY GRAPH (summary)

```
W0-02 ─→ W0-03 ─→ W2-01 (paper engine)
   └───→ W0-04 ─┘
W0-05 ─→ W0-06        W0-01 ─→ (unblocked fully by) W2-02
W1-01 ─→ W1-02        W1-04 ─→ W1-05 ─→ W1-10
W2-05 ─→ W2-06        W2-10 ─→ W4-03
W3-01 ─→ W3-02 ─→ W3-03 ─→ W3-05
              └──→ W3-04 ─┘
W3-01 feeds W4-01 (domain extraction)
Everything else is independent within its wave.
```

# SUGGESTED AGENT ASSIGNMENT SHAPE

- One agent per item; **never two agents in trading/ simultaneously**
  (order_manager.rs merge conflicts are certain — serialize W0-02..W0-07,
  W2-08..W2-10).
- UI items (W0-09..W0-11, W1-06, W1-11, W1-12) parallelize safely across
  different panel files.
- Max 2 concurrent cargo builds machine-wide (rule 5); corpus runs are
  exclusive (port 7892) — queue them.
- Each agent's final message must state: files touched, tests added (names),
  corpus verdict line verbatim (if run), and any site it did NOT fix.
