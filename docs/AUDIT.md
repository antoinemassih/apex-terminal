# Apex Terminal — Full Application Audit

**Date:** 2026-05-22 · **Method:** 10 parallel domain audits (read-only) ·
**Scope:** entire `src-tauri/` codebase.

Domains: security · trading/order-engine · concurrency · error-handling ·
performance · memory · data-integrity · persistence · dependencies ·
code-quality.

**Tally:** ~27 Critical · ~40 High · ~47 Medium · ~36 Low (~150 findings).

---

## Verdict

The app is feature-rich and the *networking* layer (WS reconnect, backoff,
circuit breaker) is genuinely solid. But the audit surfaces three systemic
problems that matter for a real-money trading tool:

1. **The render thread can be crashed** by ordinary data — unguarded
   `unwrap()`s on NaN option strikes, empty bar lists, and stale indices.
2. **Risk controls have holes** — the daily-loss cap is dead code, `confirm()`
   bypasses the kill switch, and live-vs-paper mode is decided by a URL
   substring match.
3. **Several displayed numbers are wrong** — multi-timeframe indicators
   compute on the wrong timeframe, the DOM is fabricated mock data shown
   unlabeled, and session shading is off by an hour for ~5 months a year.

Most of this is fixable without the big rearchitecture; the god-object /
`core.rs` items are already covered by `STATE_ROADMAP.md`.

---

## Cross-cutting themes

These root causes each surfaced in multiple independent audits:

- **`APEXIB_URL` + paper-mode-by-substring** (security H2, trading, code-quality
  H3) — live/paper is `APEXIB_URL.contains("dev")`. Change the URL, ship live
  orders. Needs an explicit `APEX_TRADING_MODE` flag.
- **Render-thread panic surface** (error-handling: 4 Critical; code-quality:
  483 `unwrap()`s) — a panic on the paint thread kills the window.
- **`get_theme()` / `style::current()` global locks** (performance H1/H2,
  concurrency H1, error-handling M2) — 30–50 `RwLock` acquisitions per frame,
  poisoning-prone, OOB-prone.
- **Unbounded growth** (memory: 3 Critical; concurrency H2) — bars, caches,
  channels, undo stacks with no cap on an all-day-running app.
- **Silent data loss** (persistence; error-handling H2) — `let _ =` on six
  `atomic_write` calls and fire-and-forget DB writes.

---

## CRITICAL (27)

### Trading / money path
- **T1 — Daily-loss cap is dead code.** `RiskLimits::max_daily_loss` ($50k
  default) is never read by `submit()`. No P&L accumulator, no rejection path.
  `order_manager.rs:463–494, 723–1075`.
- **T2 — `confirm()` bypasses kill switch, halt, and all risk gates.** A Draft
  created before the kill switch can be confirmed live afterward.
  `order_manager.rs:1078–1131`.
- **T3 — `Ctrl+Shift+Q` double-cancel race.** Cancels via `cancel_all_orders`
  *and* a separate raw `DELETE /orders` thread — racing, unlogged, un-paperable.
  `keyboard_shortcuts.rs:372–381`.
- **T4 — Live/paper mode by URL substring.** `APEXIB_URL.contains("dev")`. A
  URL change silently routes real orders with risk checks bypassed.
  `gpu.rs:3981`. (Also Security H2.)

### Render-thread crashes
- **E1 — `chart.bars.last().unwrap()`** unguarded in the paint path — empty
  pane crashes the window. `core.rs:6399` (also `:6876`).
- **E2 — `partial_cmp(..).unwrap()` ×3 on option strikes** — `NaN` from a
  zero-price/missing-IV leg panics the render thread. `core.rs:1039, 1056, 1086`.
- **E3 — `get_theme(idx)` unchecked index** — stale/too-high `theme_idx` (theme
  uninstalled, old workspace) panics on the render thread. `gpu.rs:457`.
- **E4 — `panes[active_pane]` unchecked** in `setup_theme` — empty `panes`
  panics at launch. `gpu.rs:3841`.

### Concurrency
- **CC1 — Lock-order chain** `ORDER_MANAGER → ORDERS_SNAPSHOT → WAL_LOCK` held
  nested; consistent today, but one out-of-order caller deadlocks. Publish/
  append should happen *after* the `ORDER_MANAGER` guard drops.
  `order_manager.rs:85–90`, `snapshot.rs:27`, `wal.rs:55`.
- **CC2 — `std::sync::Mutex` is not reentrant** — `submit`/`cancel` are written
  to be callable both directly and via `with_mgr`; a nested global lock
  deadlocks instantly. `order_manager.rs:4229, 4243`.

### Security
- **S1 — Discord OAuth2 has no `state` parameter** — CSRF / account-linking
  attack. `data/feeds/discord.rs:315–320`.
- **S2 — OAuth2 callback server** accepts any local request and reflects the
  error string unsanitized. `discord.rs:328–367`.
- **S3 — `image 0.24.9`** (RUSTSEC-2024-0430, OOB write) decodes untrusted
  Discord CDN PNGs — heap corruption from a crafted image. `discord.rs:511`.
  Bump to `image 0.25`.

### Data correctness
- **D1 — Multi-timeframe indicators compute on the wrong data.** A 1D EMA on a
  5m chart fetches the 1D source bars correctly, then computes against the 5m
  closes anyway. Every MTF indicator is silently wrong. `gpu.rs:2629–2630`.
- **D2 — The DOM order book is fabricated.** `generate_mock_levels` produces
  deterministic-hash bid/ask sizes every frame, shown with no "SIMULATED"
  label. `core.rs:1256`, `dom_panel.rs:49–64`.

### Persistence
- **P1 — `intern_style` non-atomic SELECT→MAX→INSERT** — on a network blip the
  retry recomputes the same id and hits a PK violation, which makes `do_save`
  return early and **silently drop the drawing**. `drawing_db.rs:296–331`.
- **P2 — `save_templates` deletes every template file, then rewrites.** A crash
  between the two loops leaves zero templates — permanent loss.
  `gpu.rs:7793–7808`.

### Memory
- **M1 — `Chart::bars` / `timestamps` grow unbounded.** Live `AppendBar` never
  evicts; >100 MB after a day on fast charts. `gpu.rs:2229`.
- **M2 — `dismissed_spikes: HashSet<String>`** never trimmed in production.
  `live_state.rs:61, 668`.
- **M3 — Per-route `Vec<UnboundedSender>`** — only `bars` prunes dead senders;
  `quotes/trades/chain` accumulate. Plus all feeds use *unbounded* channels
  with no backpressure (concurrency H2). `providers/apex_data.rs:33–36`.

### Performance
- **PF1 — Unconditional `ctx.request_repaint()`** every pane every frame →
  100% CPU even when the market is idle. `core.rs:3161`.
- **PF2 — Auto-S/R full O(n·lookback) pivot scan + O(n²) cluster every frame**
  — ~49k comparisons/frame/pane on a 5k-bar chart. `core.rs:3496–3515`.
- **PF3 — Order-sync `Mutex` lock + `Vec` alloc per pane per frame.**
  `core.rs:142`.

### Architecture (already covered by STATE_ROADMAP Phase 5)
- **A1 — `render_chart_pane` is one ~10,500-line function.** `core.rs:88`.
- **A2 — `Chart` ≈ 370 fields, `Watchlist` ≈ 160 fields** — god-objects.
  `gpu.rs:1588, 4459`.

---

## HIGH (40)

### Trading
- OCO / conditional / combo `submit_*` skip all financial risk checks
  (`max_order_qty`, position, notional). `order_manager.rs:1291–1404`.
- Broker error on `submit()` leaves a live `Working` order with no backend id
  → uncancelable for 10s. `order_manager.rs:1058–1068`.
- `paper_mode` can be toggled live→paper with orders in flight.
- OCO legs created `Draft` when not armed have no confirm path — stuck forever.

### Concurrency
- Lock-poisoning cascade: `NATIVE_CHART_TXS`, `live_themes`, `style_store`,
  `registry` use bare `.unwrap()` — one panic poisons, every later frame
  panics. Use `unwrap_or_else(|e| e.into_inner())`.
- Unbounded mpsc channels on every feed — no backpressure, OOM risk.
- A new single-thread Tokio runtime built per fetch call (22+ threads).

### Security
- Hardcoded homelab IPs / DB name `ococo` baked into the binary.
  `apex_data/config.rs:133, 156`.
- Full request URLs logged to a world-readable `/tmp` file. `rest.rs:147`.
- User search query interpolated unencoded into API URLs (path injection).
  `trendline_filter.rs:148`, `rest.rs:499`.

### Dependencies
- `tauri.conf.json` has `csp: null` — CSP disabled.
- `shell:allow-execute` + `shell:allow-kill` granted with no `scope`.
- 65 MB `ococo-api` Windows sidecar committed unsigned, unverified; the macOS
  sidecar is a 0-byte placeholder (broken build).

### Persistence
- Watchlist dual-write is DB-then-JSON; a failed JSON write leaves the
  authoritative local cache stale. `gpu.rs:7839–7879`.
- PG drawing saves are fire-and-forget, no retry — drawings created while PG is
  briefly down are lost. `drawing_db.rs:100–107`.
- `find_or_create_chart` SELECT-then-INSERT TOCTOU; no `UNIQUE` constraint →
  duplicate chart rows. `drawing_db.rs:263–284`.
- `version` field is written but never read — no migration dispatch.
  `gpu.rs:7402, 7474`.

### Data integrity
- Session shading hardcodes EDT (UTC-4) — off by 1h for ~5 months (EST).
  `core.rs:1791–1794`. Same bug in 0DTE date calc (`fetch.rs:88`).
- IB tick → `UpdateLastBar` hardcodes `"5m"` — live last-bar update broken on
  every non-5m chart. `ib_ws/mod.rs:478`.
- Option chain falls back to simulated Black-Scholes prices, user warning not
  enforced at the data layer. `fetch.rs:190–207`.
- Indicator-panel VWAP never resets per session — shows multi-week cumulative.
  `compute.rs:249`.
- ADX uses EMA smoothing, not Wilder — systematically wrong vs every platform.
  `compute.rs:605`.

### Error handling
- Order close / flatten fire-and-forget with `let _ = …send()` — network
  failure is invisible to the user. `orders_panel.rs:142, 162, 219`.
- Six `let _ = atomic_write(…)` — silent state/alert/hotkey/watchlist loss on
  disk-full. `gpu.rs:7287, 7429, 7685, 7742, 7804, 7875`.
- `pending_pts[0/1]` direct indexing not guarded against mid-frame state change.
- `OnceLock::get().unwrap()` in detached Discord threads — invisible panic.

### Memory
- `undo_stack` / `redo_stack` uncapped — ~3,600 cloned `Drawing`s per minute
  while dragging. `gpu.rs:1791`.
- `live_state` HashMaps (`quotes`, `greeks`, `snapshots`, `tape_by_symbol`, …)
  never purged — grow with every symbol ever seen.
- A new `std::thread` per projector TTL expiry — 20 threads/s with a big
  watchlist.

### Performance
- `style::current()` — `RwLock` acquired 30–50×/frame.
- `get_theme()` — `RwLock` re-read per pane per frame.
- `watched` set + `Vec<String>` of every symbol rebuilt + cloned every frame.
- Band indicators allocate fresh `Vec<Pos2>` per line per frame (egui path).
- `srgb_to_linear` redefined 6× as un-inlined inner fns.

### Code quality
- `gpu.rs` is 8,663 lines doing everything; `ui_kit` imports *from*
  `chart_renderer` — inverted dependency.
- `order_manager.rs` order lookups use `iter().find().unwrap()` — panic if the
  order is gone. `:3568–3699`.
- `render/pane/` has **0 tests** — the hottest code path is uncovered.
- `APEXIB_URL` hardcoded constant, 18 call sites, no env config.

---

## MEDIUM (47) — categories

- **Trading:** hotkey & DOM market orders pass `last_price: 0.0`, disabling
  fat-finger protection; WAL replay misses the rotated `orders.wal.1`; N-leg
  OCO pair-linkage only links to leg 0; default risk limits very large
  (10k/50k shares).
- **Concurrency:** panics in detached threads silently swallowed;
  `handle.block_on` nested-runtime panic risk; `WAL_LOCK` held across `fsync`.
- **Security:** compile-time source path baked in binary; `curl` subprocess for
  a drawing API call; `mem::transmute` lifetime-laundering in `text_engine.rs`.
- **Dependencies:** 4 `getrandom` versions; dual `reqwest` (0.12 + 0.13); raw
  `slice::from_raw_parts` GPU serialization (use `bytemuck`); NVML FFI
  `unsafe impl Send+Sync`.
- **Persistence:** supervisor has no shutdown signal; `atomic_write` ignores
  `sync_all` error and doesn't fsync the parent dir; corrupt state file loads
  defaults silently; watchlist DB/JSON can diverge with no reconciliation.
- **Data:** Ichimoku Chikou shifted the wrong direction; `(price*1000.0) as u32`
  hash UB on negative prices; RSI float-equality guard.
- **Error handling:** Tokio runtime `build().unwrap()` in feed threads;
  poison-prone `live_themes` unwraps; `unreachable!()` one variant from a crash;
  alert lookup `find().unwrap()` mid-render.
- **Memory:** `symbol_history` uncapped; `perf_log` session files never pruned;
  `remove(0)` O(n) shifts; per-order `state_history` uncapped.
- **Performance:** `selected_ids`/`hidden_groups` as `Vec<String>` with linear
  `.contains()`; option-picker strike sort every frame; `pane_rects()` allocs +
  locks every frame; 4 tab-header `Vec<String>` allocs/pane/frame.
- **Code quality:** `draw_tool: String` (108 compare sites); `u8` pseudo-enums;
  dual state systems; 483 `unwrap()`s; 51 `#[allow(dead_code)]`; 83 legacy
  `egui::Button` sites.

---

## LOW (36) — categories

DB worker `eprintln!` leaks symbols/ids · Redis URL never zeroed ·
CWD-relative `design.toml` load · `f32` order prices lose sub-cent precision ·
flatten hotkey not paper-gated · `Relaxed` atomics on ARM · dead `NATIVE_
CHART_TXS` senders · Renko volume inflated N× · incremental indicator update
appends `NaN` · range-bar OHLC tick-order approximation · `format!`-allocated
egui `Id` · `eprintln!` on the render thread · 63 TODO/FIXME markers ·
`design_inspector.rs` 3,015 lines · missing module docs · `jank.jsonl`
single-rotation data loss · no `cargo-audit` in CI · `rand 0.8` a major behind.

---

## Recommended fix order

1. **Render-thread crashes (E1–E4)** — tiny diffs, each prevents a hard crash.
2. **Money-path risk holes (T1–T4)** — wire the daily-loss cap, gate
   `confirm()`, fix the double-cancel, make trading-mode an explicit flag.
3. **Critical leaks (M1–M3)** — cap `bars`, `dismissed_spikes`, prune senders;
   bound the feed channels.
4. **`image 0.25` bump (S3)** + Discord OAuth2 `state` (S1/S2).
5. **Data correctness (D1, D2, DST)** — fix the MTF indicator source, label the
   DOM as simulated, fix the EDT/EST offset.
6. **Persistence atomicity (P1, P2)** — transaction-wrap `intern_style`, stage
   `save_templates`.
7. **Performance Criticals (PF1–PF3)** — idle-repaint guard, cache auto-S/R,
   hoist order-sync.
8. **Concurrency hardening (CC1, CC2, poisoning)**.
9. Architecture (A1/A2) — already the `STATE_ROADMAP.md` Phase 5 project.
