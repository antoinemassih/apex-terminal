# apex-terminal — MASTER AUDIT (2026-07, waves 1+2)

Consolidates two audit waves (14 agent-lenses total), **every P0/P1 hand-verified
against source**. Wave 1 detail lives in `AUDIT_2026-07-10.md` (trading-safety,
architecture, resilience, perf, testing, data-integrity). This master adds wave 2
(security, deps/build, feeds-deep, math, dead-code, UX, long-uptime, lifecycle)
and unifies everything into one prioritized roadmap.

**Verification discipline:** ~10 agent "CRITICAL/P0" claims did NOT survive
reading the code — they're listed in §REJECTED so they never resurface. Trust
this doc's severities over the raw agent reports.

---

## THE SHORTLIST — what actually matters (verified)

Ranked by expected-loss (money-impact × likelihood), safe-to-fix first:

1. **P0 — Positions panel submits raw market orders bypassing OrderManager**
   (`orders_panel.rs:170,190`; also `top_nav.rs:1325`, `command_palette/execute.rs:187`,
   `keyboard_shortcuts.rs:409`). No kill/risk/paper-guard/journal. *[WS-H #41]*
2. **P0 — Bracket/OCO go Working before broker Ack, no rollback on failure**
   (`order_manager.rs:1585`). Failed submit = 3 permanent phantom Working orders. *[#41]*
3. **P0 — Market-order buying-power check compares share-qty to dollars**
   (`order_manager.rs:911`). Wrong-sized orders pass; also skips fat-finger. *[#41]*
4. **P1 — Daily-loss cap resets at UTC midnight = mid-session for overnight futures**
   (`order_manager.rs:3307`, `today_day_index`). You trade NQ overnight. *[#42]*
5. **P1 — Clean-shutdown marker never fires → orphan recovery every startup +
   `drain_all` never called** (`order_manager.rs:76` Drop-on-static; `native_main.rs:108`).
   Subtle: `impl Drop for OrderManager` writes the marker, but a `OnceLock` static
   isn't dropped at exit, so it's effectively dead. *[new: #46]*
6. **P1 — 8 unbounded symbol-keyed caches leak over a scanning day**
   (`fetch.rs:198-1170`, `str_keyed_cache!` macro has zero eviction). Multi-GB creep. *[new: #47]*
7. **P1 — Kill/halt not re-checked inside spawned submit threads**
   (`order_manager.rs:1066` gate vs `:1256` async fire). *[#42]*
8. **P1 — control_flags.json fails OPEN on corrupt read** (kill silently disengaged,
   `order_manager.rs:48`). *[#42]*
9. **P1 — No feed-staleness gate before order entry**; can submit against a 45s-old
   quote with no warning (`live_state`/`subscription_manager`, no age on Quote/Trade). *[new: #48]*
10. **P1 — Bollinger Bands uses population stdev (N) not sample (N-1)**
    (`compute.rs:146`) — a REAL indicator on REAL data, ~2.6% too-tight at period 20. *[new: #49]*
11. **P1 — Dev-build CSRF: dev_inspector HTTP server unauth + `Access-Control-Allow-Origin:*`**
    on `127.0.0.1:7892`; a web page visited during `cargo run` can drive the live command
    bus (cancel orders, publish Discord) + path-traversal file read on `/run-scenario`.
    Debug-only (`#[cfg(debug_assertions)]`), NOT shipped. *[new: #50]*

Items 1-3,4,7,8 are already captured in tasks #41/#42. Items 5,6,9,10,11 are new.

---

## §A — TRADING SAFETY (order path)  [detail in AUDIT_2026-07-10 §1-2]
Verified P0s: panel bypass, bracket-Working-before-Ack + no-rollback, BP dimensional
bug. Verified P1s: UTC daily-loss rollover, kill-not-rechecked-in-spawn, control_flags
fail-open, OCO per-leg-not-aggregate risk. All in `order_manager.rs` / `orders_panel.rs`
/ `top_nav.rs`. → **WS-H (#41, #42)**. Positive: paper-guard on LiveBroker submit,
idempotency keys, WAL+recovery *are* wired (see §REJECTED for the recovery false-alarm).

## §B — LIFECYCLE / SHUTDOWN / STARTUP  [new]
- **P1 shutdown marker dead + drain_all uncalled** (shortlist #5). Fix: explicit
  close hook in the winit `CloseRequested` path that (a) appends `JournalEvent::Shutdown`,
  (b) `rt.block_on(drain_all(5s))`, (c) signals the drawing_db worker. Don't rely on
  `Drop` for a static.
- **P2 Postgres blocks the window ~5s at startup if DB down** (`native_main.rs:92`,
  5s acquire_timeout on the launch path). Move PG pool init to a post-window bg task.
- **P2 no single-instance guard** — double-launch double-binds :9091/:7892 and races
  state files (and could double-connect the broker). Add an exclusive lock file.
- **P2 state dir falls back to CWD** (`gpu.rs:8577` `data_local_dir().unwrap_or(".")`) —
  breaks read-only install dirs (Program Files). Fail loud or use a guaranteed-writable dir.
- **P3** resume-from-sleep doesn't resubscribe feeds or re-check the daily rollover;
  GPU device-loss unhandled; DPI only updates on Resize (multi-monitor drag). *(defer)*

## §C — LONG-UPTIME / MEMORY  [new]
- **P1 eight unbounded caches** (shortlist #6). One fix: add cap+LRU (or TTL sweep) to
  the `str_keyed_cache!` macro (`fetch.rs:185`) → all 8 bounded at once. ~1h, low risk.
- **P2 ProjectorCaches (news/iv_rank/iiv/corp)** TTL-only, no cap (`live_state.rs:540`);
  `realized_delta` outer map + several per-symbol maps evict on a 30s watch-set sweep
  but have no hard cap. Add a 500-key LRU ceiling.
- **Verified BOUNDED (no action):** tape/regime/spike/halt ring buffers, undo/redo
  (50/300), order state_history, dedup signatures, toast dedup, PNL_HISTORY (drain),
  subscription refcounts. Memory hygiene is mostly good; the caches are the gap.

## §D — FEEDS / CONNECTIVITY  [new, detail from feeds lens]
- **P1 no staleness gate before trading** (shortlist #9) + no `last_update_ms` on
  Quote/Trade/BarWire → UI can't show "stale" and order entry can't refuse stale.
  Fix: stamp age on push, expose `feed_age_ms`, gate order-entry + add a stale badge.
- **P1 open (still-forming) bar is cached** (`bar_cache` set-callers, no `closed` filter) —
  a second pane loads a stale current candle within TTL. Cache only closed bars.
- **P2 broadcast Lagged not uniformly handled** (`subscription_manager` fanout cap 1024):
  a slow frame under a volatile burst silently drops frames; if any `recv()` treats
  Lagged as fatal the subscription dies. Audit every `recv()`; log+seek on Lagged.
- **P2 gap-fill skipped on the FIRST reconnect** (`resilient_ws:333` guard
  `reconnect_count>0`) → a drop in the first 30s leaves a silent bar gap. Track
  "seen ≥1 frame" instead.
- **P2 multi-feed price disagreement** — same symbol via ib_ws vs futures_feed vs
  firehose, no canonical selection; panes can disagree. Define a per-symbol feed priority.
- **P3** reconnect jitter present (0.25) but verify per-feed seed to avoid lockstep;
  ib_ws MessagePack malformed-frame handling is drop-on-error (OK) but log raw bytes.
- *Calibration:* the feeds agent's "sign every signal frame" is over-engineered for a
  single-user homelab — treat signal-frame validation as P3 defensive, not P1.

## §E — QUANT / MATH CORRECTNESS  [new]
- **P1 Bollinger population vs sample stdev** (shortlist #10, `compute.rs:146`) — the one
  real-data indicator bug. One-line fix + a test.
- **P2 synthetic gamma/GEX is sine-wave noise** (`gpu.rs:2609`) — mathematically
  meaningless. Already carries the F3 "SYNTHETIC" badge (good), so it's disclosure-OK,
  but consider not drawing walls off fabricated data at all.
- **P2 synthetic footprint / micro-volume-profile are fabricated** (`gpu.rs:4232`) and
  NOT badged — unlike gamma. Add a "SIMULATED" tag or gate behind real tick data.
- **P2 hardcoded 5% risk-free rate + fabricated IV smile** in the BS synthetic chain
  (`fetch.rs:15`, `compute.rs:412`) — only matters on the synthetic fallback path; fine
  if that path is badged, but greeks beyond `bs_delta` are unimplemented (`compute.rs:392`,
  `#[allow(dead_code)]`) so any greeks display is delta-only.
- **P3 position sizing ignores futures/option multipliers** (`mod.rs:801`) — correct for
  equities; wrong notional for ES/NQ and option contracts. Wire the multiplier.
- **Verified CORRECT (no action):** RSI (Wilder), MACD, ATR/ADX (Wilder), CCI,
  Williams %R, VWAP session reset, option payoff ×100, OHLC bar aggregation, DTE/252.

## §F — SECURITY  [new]
- **P1 (dev-only) dev_inspector CSRF/exposure** (shortlist #11). Gate `init()` behind an
  explicit env opt-in (not just `debug_assertions`), add a per-run token, drop
  `Access-Control-Allow-Origin:*`, refuse order-effecting AppCommands on the interactive
  path, sanitize the `file`/`name` params (`server.rs:774`). Not shipped in release.
- **P2 ApexData defaults to plaintext http://** (`config.rs:17`) + broker has no cert
  pinning (`broker.rs`) — LAN MITM could feed fake fills/balances that drive BP+daily-loss
  checks. Homelab-acceptable today; enforce https default + pin for any wider deploy.
- **P2 unbounded broker JSON** (positions array, `trading/mod.rs:510`; HTTP body length,
  `server.rs:101`) — a rogue/compromised backend can OOM. Cap array + body sizes.
- **Verified GOOD:** workspace-name sanitization blocks path traversal; Discord token
  migrated to OS keychain; DB-cred URLs explicitly never logged; TLS validation left at
  default (no `danger_accept_invalid_certs`); all 3 `unsafe` blocks are legit Win32.

## §G — TESTING GAPS  [detail in AUDIT_2026-07-10 §3]
Order state-transition matrix, WAL rotation/replay, risk boundaries, bracket fill-linking,
conditional-trigger equality, serde round-trips — all thin/zero coverage. Unlock with an
`APEX_ORDERS_STATE_PATH` env override (mirrors `APEX_WAL_PATH`) so persistence is
disk-testable; add `/reset-state` for <500-prefix scenario hermeticity (~420 unguarded). → **#43**

## §H — PERFORMANCE  [detail in AUDIT_2026-07-10 §4]
`orders_view` per-frame clone, hit-test `format!` loop, watchlist-row `format!`,
MA-ribbon EMA recompute, VP pan hysteresis. All low-risk allocs. → **#44**

## §I — ARCHITECTURE  [detail in AUDIT_2026-07-10 §5]
core.rs 13.3k / gpu.rs 9.7k / Chart 246 fields / 7 Store dual-writes / 31 raw fs::write.
~90 of Chart's fields are safe E3-style clusters. E5 Phase-2 switch (shadow proven).
All greenlight-gated. Dead-code reclaim is tiny (~159 LOC, `generate_placeholder_*`).

## §J — UI/UX  [new, lower urgency but real mistake-risk]
- **P2 paper-vs-live is largely color-encoded** (`top_nav.rs:1307`) — add an
  unmistakable "LIVE"/"PAPER" text badge (colorblind + screenshot safety on a live-money
  tool). Highest mistake-risk UX item.
- **P2 inconsistent number/unit formatting across panels** (2dp vs 0dp, `$`/`%` present
  or not) — centralize `fmt_price/fmt_pct/fmt_qty`. Misreads → wrong size.
- **P2 error surfacing not guaranteed to reach UI** — feed drops / order rejects / save
  failures can be stderr-only. Establish "every ui/ `.send()`/`.parse()` failure →
  errors_sink/notification" (ties to the drawing-db and feed fire-and-forget gaps).
- **P3** hardcoded greek/RSI/breadth colors bypass theme (known G1 debt); raw-egui
  holdouts (a few buttons/checkboxes); 9px font floors on dense data; keyboard-shortcut
  conflict detection. *(polish track)*

---

## REJECTED / DOWNGRADED (verified false or overstated — do not re-raise)
- ❌ "`replay_and_recover()` never called" — it's spawned at `order_manager.rs:64`.
- ❌ "No `Shutdown` journal marker" — `impl Drop for OrderManager` writes it; the REAL
  (subtler) issue is Drop-on-a-static never runs at exit → see §B P1.
- ❌ "PNL_HISTORY unbounded leak" — drain at >6000 bounds it.
- ❌ "orders.json corrupt = silent total loss" — parse failure is reported; atomic_write
  + WAL replay cover the unclean-crash case. Residual: corrupt-after-clean-shutdown.
- ❌ "WAL rotation loses concurrent writes" — the WAL lock serializes append+rotate;
  needs a repro before acting (write the rotation test instead).
- ❌ "SignalDrawing/DrawingGroup/chart::state dead code" — all have live call sites.
- ⚠️ "Flatten bypasses kill-switch" — flatten is risk-reducing and kill uses the same
  endpoint; the real issue is missing journal/audit (folded into WS-H #41).
- ⚠️ "Sign every signal frame / feed cert-pin now" — over-engineered for single-user
  homelab; keep as P3 defensive.

---

## CONSOLIDATED ROADMAP (recommended order)
1. **WS-H safety round-2** (#41 P0s, #42 P1s) — corpus-gated, each lands with §G tests.
2. **New: shutdown-integrity** (#46) — clean-shutdown hook (marker + drain_all + worker
   stop); unblocks correct recovery + tidy exit. Small, high-value.
3. **New: cache bounding** (#47) — one `str_keyed_cache!` fix. ~1h, kills the uptime leak.
4. **New: feed-staleness gate + closed-bar cache** (#48) — trading-correctness.
5. **New: Bollinger N-1 + badge synthetic footprint** (#49) — correctness/disclosure.
6. **Perf quick wins** (#44) — one low-risk batch.
7. **New: dev_inspector hardening** (#50) — env-gate + token + CORS + path sanitize.
8. **UX: live/paper badge + number-format helpers + error-contract** (§J P2s).
9. **Architecture** (§I) — only with explicit per-track greenlight.

Wave-2 verification cost caught ~10 false criticals; the genuine new work is items
#46-#50 plus the already-tracked WS-H (#41-#44).
