# apex-terminal — Full Codebase Deep Dive

**Date:** 2026-08-02  
**Method:** 26-agent multi-dimension audit with adversarial verification  
**Scope:** 443 files / 196,575 lines across chart, ui_kit, data, design_system, foundation, state, dev_inspector, persistence

## Method

12 dimensions were audited in parallel (architecture, dead code, unwired features, redundancy, ui_kit, design system, UX, central engines, data layer, trading path, state management, fail-silent patterns). Each dimension's findings were then handed to a **separate adversarial verifier** instructed to *refute* them by opening the cited `file:line`. A synthesis agent and a completeness critic ran last.

Every finding below carries a verdict:

- **CONFIRMED** — verifier opened the code and the claim held
- **WEAKENED** — real but overstated (severity cut, or true for fewer call sites than claimed)
- **UNVERIFIED** — verifier returned no matching verdict; treat as unconfirmed
- **REFUTED** — dropped, listed at the end for the record

## Results

| | Count |
|---|---|
| Raw findings | 147 |
| Refuted and dropped | 6 |
| **Surviving** | **141** |
| — confirmed | 89 |
| — weakened | 36 |
| — unverified | 16 |

**By severity**

| Severity | Count |
|---|---|
| P0 | 10 |
| P1 | 34 |
| P2 | 58 |
| P3 | 39 |

**By category**

| Category | Count |
|---|---|
| correctness | 33 |
| fail-silent | 19 |
| dead-code | 16 |
| unwired | 16 |
| architecture | 15 |
| redundant | 10 |
| inconsistency | 9 |
| design-system | 8 |
| perf | 7 |
| ux | 5 |
| coupling | 3 |

---

# P0 — Critical

These are live-money or data-integrity defects. Ordered as returned.

## [CONFIRMED] Bracket/OCO/options-trigger broker submits ignore HTTP status — a broker REJECTION returns Ok and paints 3 phantom "Working" legs with no backend id

**Category:** fail-silent

**Evidence**

broker.rs:620-633 (`submit_bracket`), 665-677 (`submit_oco`), 734-743 (`submit_options_trigger`) go `client.post(...).send().map_err(...)?` then straight to `resp.json()`. They never call `Self::error_for_status`, which W0-02 added and which IS used on the single-order paths (broker.rs:400 submit, 420 cancel, 445 modify). A 4xx/5xx body parses fine, `pick("parentOrderId")` yields `None`, and the function returns `Ok(BracketSubmitResponse{None,None,None})`. The manager's Ok branch (order_manager.rs:2117-2147) then runs the `if !paper` block that transitions eid/tid/sid `PendingSubmit -> Working` unconditionally, leaving `backend_order_id = None`. Same shape for OCO at order_manager.rs:2300-2326. The Err branch that would mark them Rejected (2149-2171) is unreachable for any HTTP-level rejection.

**Impact**

After a broker-side rejection (margin fail, risk breach, IB disconnect, 500) the trader sees an entry + take-profit + stop-loss all displayed as live Working orders that do not exist at the broker. Clicking cancel takes `issue_cancel`'s `(_, None)` arm (order_manager.rs:2920-2926) and marks them Cancelled locally without ever contacting the broker, so nothing ever corrects the lie. The trader believes a stop is protecting a position that has no stop — or no position at all.

**Fix**

Route all five multi-leg submits through `Self::error_for_status(resp, "bracket")?` exactly like `submit()` does, and additionally treat an all-`None` id response as `Err` ("broker returned no order ids") so the legs land in Rejected rather than Working.

**Verifier note:** Line-exact. broker.rs:620-633 (submit_bracket), 665-677 (submit_oco), 734-743 (submit_options_trigger) all do `.send().map_err(...)?` then `resp.json()` with no `Self::error_for_status`. error_for_status is defined at broker.rs:335 and applied ONLY at 400/420/445 (submit/cancel/modify) — grep confirms exactly 3 call sites. The Ok branch at order_manager.rs:2117-2147 wires `if let Some(oid)` (all None on a rejection body) and then unconditionally runs `if !paper { ... PendingSubmit -> Working }` at 2131-2144; the Rejected branch is at 2149-2171 as claimed. Identical shape for OCO at 2300-2326. Cancel impact also holds: a leg with backend_order_id=None and paper=false falls to the `(_, None)` arm at order_manager.rs:2920-2926, transitioning straight to Cancelled with no HTTP DELETE. One precondition the auditor asserts rather than proves: the 4xx/5xx body must be valid JSON for `resp.json()` to succeed — a non-JSON error page would Err and land correctly in Rejected. FastAPI-shaped ApexIB errors are JSON, so the failure mode is real. Note the fix text says "all five multi-leg submits" but submit_conditional (broker.rs:708-712) and submit_combo already fail closed via `extract_order_id().ok_or_else(...)`; only the three named are affected.

---

## [CONFIRMED] `submit_bracket` bypasses `validate_risk` entirely — no position cap, notional, buying power, daily-loss, dedup, or max-open-orders check on a 3-leg live order

**Category:** correctness

**Evidence**

order_manager.rs:1978-2000: `submit_bracket` checks only `kill_engaged`, `halted`, the rate-limit token, `qty == 0` and `max_order_qty`, then pushes three ManagedOrders. `validate_risk` is called by every other path — submit (1620), confirm (1907), submit_oco (2208), submit_conditional (2391), submit_options_trigger (2556), submit_combo (2721) — and by grep it has exactly those six call sites. Bracket is the only submit path with no call. Dedup (`OrderSignature`/`recent_signatures`, 1598-1606) and the `max_open_orders` check (1612-1616) are also absent. Reachable live from render/pane/core.rs:1761 and io/fetch.rs:1593 (`submit_ib_order`, called from a background thread).

**Impact**

A bracket is the highest-notional gesture in the app (3 legs, entry + exposure both sides) and is the one path with no buying-power pre-check, no daily-loss cap, no max-position cap and no oversell guard. Double-clicking the bracket button fires two independent brackets because there is no dedup window; the rate limiter allows 20 in a burst.

**Fix**

Call `self.validate_risk(&intent, paper)?` and the dedup + `max_open_orders` checks at the top of `submit_bracket`, sizing the notional check against `qty × entry_price` for the entry leg (the TP/SL legs are exit legs and should be exempt from the buying-power arm).

**Verifier note:** order_manager.rs:1978-2000 contains exactly and only: kill_engaged (1979), halted (1980), try_consume_submit_token (1983), qty==0 (1995), max_order_qty (1998). No validate_risk, no OrderSignature dedup, no max_open_orders. Grep for `validate_risk` returns the definition at 1075 plus call sites 1620 (submit), 1907 (confirm), 2208 (submit_oco), 2391, 2556, 2721 — bracket is absent, exactly as claimed. Dedup at 1596-1606 and max_open_orders at 1612-1616 confirmed present in submit() only. Reachability confirmed: render/pane/core.rs:1761 (DOM ladder bracket click) and io/fetch.rs:1593. Partial mitigation the auditor did not mention: when NOT armed and NOT a market order, legs are created Draft (2007-2011) and each subsequent confirm() DOES run validate_risk at 1907 — so the unchecked window is armed-mode brackets and market brackets (initial_state is PendingSubmit unconditionally for Market at 2008). Market brackets and armed live trading are the primary use, so P0 stands.

---

## [CONFIRMED] Position caps and oversell protection read local `Filled` rows that are never persisted and are GC'd hourly — every position-based guard reads zero after a restart, while broker positions are explicitly discarded

**Category:** correctness

**Evidence**

validate_risk computes `net_position` by summing `self.orders` filtered on `o.state == OrderState::Filled` (order_manager.rs:1092-1097), and the oversell guard repeats that scan at 1627-1648. `save_to_disk` persists ONLY `Working | PendingSubmit | PartialFill` (order_manager.rs:3238-3241) — `Filled` is never written. `gc()` (3351-3360) drops terminal orders older than one hour once the book exceeds 600. Meanwhile the broker's real position list is available and thrown away: `Some((summary, _positions, _ib))` at order_manager.rs:1183, and `reconcile_positions` (4500-4545) only logs drift, with a comment that it deliberately does not auto-correct.

**Impact**

Restart the app while holding 500 shares: `net_position` is 0, so `max_position_qty` lets you add another full 500, and the oversell guard (`is_sell && net_position > 0`) never fires, so a sell of any size passes. The same happens without a restart once GC prunes fills. The two guards that exist specifically to stop over-sizing and naked shorting are inert in exactly the situation they were written for.

**Fix**

Seed `net_position` from the broker's `positions` snapshot (already fetched every 5s and already passed into `validate_risk` as `_positions`), using local Filled rows only as an in-session delta on top of broker truth.

**Verifier note:** All four cited spots are line-exact. net_position sums `o.state == OrderState::Filled` at order_manager.rs:1092-1097; the oversell/overbuy guards repeat the same scan at 1626-1648. save_to_disk at 3238-3241 filters `matches!(o.state, Working | PendingSubmit | PartialFill)` with a doc comment at 3236 stating "Filled / Cancelled / Rejected are history and not saved" — so Filled is provably never persisted. gc() at 3351-3360 retains terminal orders only if updated within 3_600_000ms, gated on `orders.len() > 600`. Broker truth is discarded: validate_risk step 5 matches `Some((summary, _positions, _ib))` at 1183, and reconcile_positions (4500-4545) computes drift and only reports/toasts, with the doc comment "does NOT auto-correct". Minor title imprecision: GC is size-triggered (>600 orders), not a periodic hourly sweep — the one-hour figure is the retention window inside it. The restart failure mode is fully proven.

---

## [CONFIRMED] `confirm()` marks the order Working before the broker Ack, drops the broker Err, and writes no journal event — the exact defect A2 fixed in `submit()`

**Category:** correctness

**Evidence**

order_manager.rs:1914-1962. Line 1916 sets `PendingSubmit`, then line 1919 immediately sets `o.state = OrderState::Working` — synchronously, before the spawned thread runs. The spawned closure at 1953 is `if let Ok(ib_oid) = broker.submit(&args)`: the `Err` case is silently discarded, so a rejected order stays Working forever with `backend_order_id = None`. Contrast `submit()` at 1799-1844, whose comment reads "transition PendingSubmit -> Working ONLY now, on the real broker Ack" and which has a full Err → Rejected branch. Also, grepping lines 1867-1970 for `journal`/`deferred_journal` returns nothing: confirm emits no `Attempt`, no `Ack`, no `Fail`, and never calls `transition()` so not even a `StateChg`.

**Impact**

Two failures at once. (1) A broker-rejected confirm shows as a live Working order with no backend id, is persisted as Working by `save_to_disk`, restores as Working on the next start, and cancel silently no-ops it. (2) Because no `Attempt` is journalled, `find_orphan_attempts` / `replay_and_recover` are structurally blind to every order sent via the SEND-badge path — the WAL durability story does not cover it at all.

**Fix**

Make `confirm()` mirror `submit()`: leave the order in `PendingSubmit`, push a `JournalEvent::Attempt` before spawning, and add the `Ok` → guarded Working + `Ack` / `Err` → Rejected + `Fail` branches.

**Verifier note:** Line-exact to the character. order_manager.rs:1916 `o.state = OrderState::PendingSubmit;` then 1919 `o.state = OrderState::Working;` — both synchronous, inside the same `if let Some(o)` block opened at 1914, before spawn_guarded at 1937. 1953 is `if let Ok(ib_oid) = broker.submit(&args) {` with no else — the Err is discarded. Contrast confirmed at submit() 1797-1844: `match broker.submit(&args)` with the A2 comment "transition PendingSubmit -> Working ONLY now, on the real broker Ack", a guarded `if o.state == OrderState::PendingSubmit`, journal::append(Ack), and a full Err -> Rejected + journal Fail branch. Journal claim verified independently: grep for `JournalEvent::Attempt` returns 1736, 2842, 2975, 3067 (plus tests) — nothing in the 1867-1970 confirm body — and confirm mutates o.state directly rather than calling transition(), so no StateChg either. Both stated impacts follow: save_to_disk persists Working (3238-3241) and restored_state_for (222-227) maps non-PendingSubmit to Working on reload.

---

## [CONFIRMED] gap_fill_on_reconnect replays a FULL historical bar series into the live chart append path, appending stale out-of-order bars after the current bar

**Category:** correctness

**Evidence**

subscription_manager.rs:585-588 (bars call), :601 (send_to_native_chart); gpu.rs:3089-3093

**Impact**

On every WS reconnect (including the ~30s watchdog-forced ones), each active bar subscription appends hundreds of stale historical bars onto the END of the live series. `timestamps` becomes non-monotonic and duplicated, so anything doing an ordered/bisect lookup over it (crosshair time, indicator windows, x-axis labels, history pagination) reads wrong data, and the chart draws a fake re-run of the session at the right edge with historical prices. The actual gap is never filled, yet ws.rs:502-507 reports "replayed {n} bars after reconnect" — a green log for work that filled nothing. This is a live-money trading chart.

**Fix**

Route gap-fill through a ranged fetch (`rest::get_replay(class, sym, tf, last_ts, now_ms, ..)`) and honor `start_ms/end_ms/limit` in `ApexDataProvider::bars`; make `CachedProvider` bypass the cache (or key on the range) whenever `start_ms != 0`; and add a monotonicity guard in the `AppendBar` handler that drops/ignores any `timestamp < self.timestamps.last()` instead of pushing it.

**Verifier note:** Every link in the chain holds. subscription_manager.rs:585-588 calls `self.provider.bars(symbol, timeframe, last_ts, now_ms, None)`; line 601 pushes EVERY returned bar via `crate::send_to_native_chart(bar_wire_to_append_cmd(&b, is_mark))`. ApexDataProvider::bars binds `_start_ms/_end_ms/_limit` as unused (apex_data.rs:196-198) and calls `rest::get_bars` which has no range params (rest.rs:407-423); the ranged `get_replay` at rest.rs:426 is indeed unused here. CachedProvider::bars short-circuits on a range-less key (cached.rs:70-74). I checked the provider composition: registry.rs:36-60 puts CryptoProvider (crypto_only, returns NotSupported for stocks) ahead of Cached(ApexData), so the chain does resolve to the two implicated providers. The AppendBar handler at gpu.rs:3078-3105 dedupes ONLY `self.timestamps.last() == Some(&timestamp)` (line 3089); everything else falls to the unconditional `self.bars.push(bar); self.timestamps.push(timestamp)` at line 3093. I looked for a caller-side guard and found none: send_to_native_chart (lib.rs:66-77) broadcasts to all panes and the only filter in the handler is symbol/timeframe equality (line 3084). Both trigger paths are live (ws.rs:497-510, ib_ws/mod.rs:281-295) and I verified last_seen_ts is actually populated — the frame listener at apex_data.rs:72 bumps it, and real bar subs exist via fetch.rs:604-605, 1857, 1867, 2017-2018. Nothing refutes this.

---

## [CONFIRMED] ABBA lock inversion between ApexData ROUTES mutex and SubscriptionManager maps can deadlock the WS reader against any chart-load thread

**Category:** correctness

**Evidence**

Order A (WS runtime thread, per inbound frame): the frame listener registered in `ensure_listener` takes `routes().lock()` at data/providers/apex_data.rs:66 and, while that guard `r` is still alive (used at line 74), calls `mgr.bump_last_seen_bar(...)` at line 72, which acquires `SubscriptionManager.bars` at data/providers/subscription_manager.rs:413. So: ROUTES → bars. Order B (chart-load / fetch thread): `SubscriptionManager::subscribe_bars_with_source` acquires `self.bars.lock()` at subscription_manager.rs:200 and holds it (explicit `drop(map)` only at line 220) across `self.provider.subscribe_bars(symbol, timeframe)?` at line 208, which flows FallbackProvider → CachedProvider → `ApexDataProvider::subscribe_bars` (apex_data.rs:234-244) which takes `routes().lock()` at line 237. So: bars → ROUTES. The same inversion exists on all four streams: quotes (subscription_manager.rs:265+270 vs apex_data.rs:87+256), trades (317 vs 93+276), and on the unsubscribe side (`unsubscribe_bars_with_source` holds the map guard across `self.provider.unsubscribe_bars` at subscription_manager.rs:248-254). Both orders are live in production: the listener runs on the `apex-data-ws` runtime, and `subscribe_bars`/`subscribe_bars_with_source` are called from `spawn_guarded` OS threads at chart/renderer/io/fetch.rs:604, 1856, 1866 and 2017.

**Impact**

A window as small as one HashMap probe is enough: if the WS thread grabs ROUTES while a fetch thread holds `bars`, both block forever (`std::sync::Mutex` and `parking_lot::Mutex`, neither reentrant nor timed). The market-data reader thread and the chart-load thread hang permanently — prices freeze on-screen with no error, the watchdog can't help because it only forces a reconnect of a loop that is already blocked, and the terminal keeps rendering last-known prices while the user trades against them.

**Fix**

Never hold `SubscriptionManager`'s map guard across a `provider.*` call: compute the key, drop the guard, call the provider, then re-acquire to insert (with a re-check for a concurrent inserter). Equivalently, snapshot the needed sender vec out of ROUTES and drop that guard before calling `bump_last_seen_*`. Document a single global lock order for the data layer.

**Verifier note:** Both orders verified line-by-line. Order A: apex_data.rs:66 `let mut r = match routes().lock()`, guard still alive when line 72 calls `mgr.bump_last_seen_bar(...)` (which locks `self.bars` at subscription_manager.rs:413) and is used again at line 74 — so the guard is provably not dropped early. ROUTES -> bars. Order B: subscription_manager.rs:200 `let mut map = self.bars.lock()`, held across line 208 `self.provider.subscribe_bars(...)`, with the only `drop(map)` at line 220; that call reaches ApexDataProvider::subscribe_bars which locks routes at apex_data.rs:237. bars -> ROUTES. I specifically tried to refute this via the FallbackProvider hop: `first_realtime()` (fallback.rs:45-49) picks the first provider with `realtime: true` — CryptoProvider is `realtime: false` (crypto.rs:134) and HttpFallbackProvider is `realtime: false` (http_fallback.rs:161), so it resolves to CachedProvider, which forwards verbatim (cached.rs:91). The inversion path is real, not broken by the chain. Same shape on quotes (200/265+270 vs 87+256), trades (317 vs 93+276) and unsubscribe (246-254). Order B runs on spawn_guarded OS threads (fetch.rs:604, 1857, 1867, 2017); Order A on the apex-data-ws runtime. Neither parking_lot::Mutex nor std::sync::Mutex is timed or reentrant. Latent race rather than an observed hang, but the inversion is unambiguous.

---

## [CONFIRMED] Orders panel Place All / Cancel All / per-row cancel never reach OrderManager or the broker — they flip a local field a per-frame reconcile immediately reverts

**Category:** unwired

**Evidence**

cancel_order_with_pair is trading/mod.rs:194-206; PLAY_ID_BASE at plays_panel.rs:629

**Impact**

A trader with working orders clicks the X on a row, or "Cancel All", to get out of the market. Nothing is cancelled at the broker. The row briefly greys and then snaps back to PLACED on the next frame — which reads as a UI glitch, not as "your cancel did not happen". Conversely "Place All (N)" appears to arm N drafts; for a NeedsConfirmation order the manager still holds it as Draft and it is never submitted. For play-derived phantom orders (plays_panel.rs:616-660, ids >= 0x8000_0000, deliberately never registered with OrderManager) the local flip is NOT reverted because they are absent from `mgr_orders`, so they display PLACED indefinitely with nothing behind them. Both directions are live-money exposure errors: believing you are flat when you are not, and believing you are in when you are not.

**Fix**

Route all four commands through order_manager: PlaceAllDraftOrders/PlaceSelectedOrders → `confirm_order(id)`; CancelAllOrders/CancelSelectedOrders/CancelOrder → `cancel_order(id)` (mirroring order_ledger_panel.rs:285/487). Keep the local status write only as an optimistic hint and let orders_view reconcile it. Separately, either exclude play-derived phantom ids (>= PLAY_ID_BASE) from the Orders panel's draft count and Place All, or badge them distinctly so they cannot be mistaken for broker-known drafts.

**Verifier note:** Every link in the chain checks out. orders_panel.rs:278/285/312/315/400 push the four AppCommands. commands.rs:772-778 (PlaceAllDraftOrders) and :780-788 (CancelAllOrders) mutate only `p.orders` status; :796-813 (PlaceSelectedOrders) likewise; :749-768 (CancelOrder) and :816-824 (CancelSelectedOrders) call cancel_order_with_pair, whose full body (trading/mod.rs:194-206) only sets `o.status = OrderStatus::Cancelled` on the Vec. A repo-wide grep for these four variants returns only commands.rs, orders_panel.rs and dev_inspector — no order_manager call anywhere on the path. core.rs:357 does run `chart.orders = trading::orders_view(&chart.orders, &mgr_orders, dragging)` every frame, and orders_view (trading/mod.rs:76-96) unconditionally does `local.status = mo.status` for any id the manager knows, so the local flip is reverted next frame. The correct path exists and is used at order_ledger_panel.rs:285 (`order_manager::cancel_order`) and :487 (`cancel_all_for_symbol`). The phantom-order claim also holds: plays_panel.rs:616-644 allocates ids from PLAY_ID_BASE 0x8000_0000 and its own comment says 'this fn deliberately never touches OrderManager', so those rows are absent from mgr_orders and the local PLACED flip persists forever.

---

## [CONFIRMED] Live incremental path appends NaN for 16 of 19 indicators and permanently suppresses full recompute

**Category:** correctness

**Evidence**

gpu.rs:3752 (guard), 3759 (counter set), 3764/3777 (SMA-WMA/EMA arms), 3787-3790 (NaN catch-all), 4938 (sole call site)

**Impact**

On any live-data chart, every oscillator and band indicator except SMA/WMA/EMA freezes at the bar count present when the pane last loaded, and stays frozen for the whole session. The line renders correctly over history and then just stops at the right edge — the exact region a trader is reading to make an entry. Bollinger bands, Supertrend direction and MACD histogram desync from their primary line. Nothing logs or warns.

**Fix**

Delete the incremental branch entirely and always call `recompute_indicators()` when `n != indicator_bar_count` — it is already O(n) per new bar and the volume-analytics path at `gpu.rs:4629` does exactly that without a problem. If profiling later shows it matters, make the incremental path go through `IndicatorType::spec().compute()` over the tail window rather than hand-rolling per-kind arms.

**Verifier note:** gpu.rs:3787-3790 is exactly the `_ => { ind.values.push(f32::NAN); }` catch-all; only SMA/WMA (3764) and EMA (3777) are extended. Guard at 3752 is verbatim as quoted and 3759 sets indicator_bar_count = n on every incremental pass. I independently enumerated all 30 `indicator_bar_count = 0` sites (grep across tree): LoadBars 2971, PrependBars 3062, IndicatorSourceBars 3202, plus UI/keyboard/context-menu/workspace-restore sites — none on the AppendBar (3093) or UpdateLastBar new-minute (3118) path, both of which push exactly one bar. update_indicators() has exactly one call site, gpu.rs:4938 inside update_simulation, which iterates every pane every frame. I also checked for a periodic history refetch that could reset the counter: fetch_bars_background is only invoked on symbol/timeframe change (gpu.rs:4996, 5044, 8902, core.rs:986) — nothing polls. Multi-lane claim also holds: the incremental loop touches only ind.values, never values2..5/histogram/supertrend_dir/divergences. Nothing logs. No refutation available.

---

## [CONFIRMED] Cross-timeframe indicators emit a series indexed in source-bar space but rendered in chart-bar space

**Category:** correctness

**Evidence**

gpu.rs:3642-3643, 3679, 3702-3712; compute.rs:247 (keltner mixes series); source_timestamps written gpu.rs:3200, zero readers

**Impact**

A user who sets an indicator's source to a higher timeframe (a documented, shipped feature with a TF badge drawn at core.rs:4048) gets a line anchored to the left edge of the chart at the wrong timestamps and truncated early — or, for Keltner/Supertrend/ADX/Ichimoku, a line computed from two mismatched bar series. It looks like a plausible indicator, which is the dangerous failure mode.

**Fix**

After computing in source space, resample back to chart-bar space using `ind.source_timestamps` and `self.timestamps` (forward-fill: for each chart bar, take the last source value whose timestamp <= the chart bar's). `compute::time_to_bar_index` (compute.rs:587) already does the binary search. Until that exists, gate the source-TF picker off rather than shipping a wrong line.

**Verifier note:** gpu.rs:3642-3643 builds source_closes_owned from ind.source_bars; 3679 makes it base_source; the reg::Ohlcv at ~3702-3712 passes chart_highs/chart_lows/chart_volumes alongside it, so OHLC-consuming indicators genuinely mix two bar series (compute_keltner at compute.rs:247 calls compute_ema(source_closes) and compute_atr(chart_highs, chart_lows, source_closes)). Output length is closes.len() in every compute fn I checked (compute.rs:143 bollinger, :264 rsi, :120 atr). Render indexes by chart bar index (core.rs:3888-3893 Ichimoku lanes via `ind.values*.get(i as usize)` with bx(i)). Decisive check: `source_timestamps` is written at gpu.rs:3200, cleared at 3024/5033/8880, and read NOWHERE — grep across all 443 files returns only writes/clears. So no remap exists. Feature is reachable (indicator_editor.rs:275-281 TF picker) and persisted (workspace_persist.rs:28, 228, 523) and badged (core.rs:4038-4048). ADDITIONAL, strengthening: when source_bars.len() > chart bars.len() (e.g. 5m source on a 1d chart), compute_atr's loop `for i in (period+1)..n` with n = closes.len() indexes highs[i] past the end → index-out-of-bounds panic, not just a wrong line. Only mitigation vs the finding: it is opt-in, not default-on.

---

## [CONFIRMED] Chain cache short-circuit is keyed on underlying only — a 30/60-DTE request silently renders the nearest ≤14-DTE expiry under a "30D" label

**Category:** correctness

**Evidence**

chart/renderer/io/fetch.rs:546-561 (short-circuit) + chart/renderer/render/pane/core.rs:10266 (frame-path re-derive with far_dte) — both must be fixed

**Impact**

A trader selects 30D or 60D, the picker header keeps printing `dte_label(current_dte)` = "30D"/"60D" (option_quick_picker.rs:87-89), and the grid shows ~14-DTE contracts. The OCC ticker carried in each row is the REAL nearest-expiry contract (fetch.rs:790 `r.ticker.clone()`), so a click routes a live order for a contract weeks earlier than the one displayed. Wrong-expiry fills on a live-money terminal.

**Fix**

Key the short-circuit on (underlying, expiry-coverage), not underlying alone: before returning from the cache, verify the chosen expiry is within a tolerance of the requested target (e.g. abs(chosen - target) <= 3 days) and fall through to the dte-specific REST query otherwise. Separately, have `apex_data_chain_to_tuples` return the chosen expiry so callers can label the grid with the real date instead of the requested DTE.

**Verifier note:** Every cited mechanism verified. fetch.rs:471 `dte_max: Some(14)` + merge_chain_delta (fetch.rs:474) is exactly as claimed; fetch.rs:546-555 short-circuits on any non-empty `live_state::get_chain(&symbol)` and returns before the dte-specific query at fetch.rs:557-561; apex_data_chain_to_tuples picks `expiries.min_by_key(|d| (*d - target).num_days().abs())` at fetch.rs:714-715 with no tolerance and no way for the caller to learn which expiry was chosen. DTE_LIST with 30/60 confirmed at option_quick_picker.rs:16 and pane/core.rs:1229. Bootstrap at pane/core.rs:10228 uses dte=0 (so dte_max = max(0,14) = 14) and the 6s path at pane/core.rs:10232 is refresh_chain_rest — so the cache genuinely only ever holds ≤14-DTE rows for a warmed underlying. The defect is in fact WORSE than described: the frame path independently re-derives `watchlist.chain.far` with `apex_data_chain_to_tuples(&cached, far_dte, ns, hint)` (pane/core.rs:10266), so fixing only the fetch.rs short-circuit would not fix it. Two corrections that do not change the verdict: (a) dte_label(30)/dte_label(60) render "1M"/"2M", not "30D"/"60D" (option_quick_picker.rs:19-29); (b) the impact wording is slightly off — the OCC ticker, bid/ask, strike and IV in each row all belong to the SAME real ≤14-DTE contract, so a click routes an order for the contract actually displayed. The defect is that OptionRow carries no expiry (gpu.rs OptionRow has 9 fields, none of them a date), so the header label is the only expiry signal the trader has, and it lies.

---

# P1 — High

## [CONFIRMED] Confirming a Draft OCO or bracket leg re-submits it as a standalone order — the OCA group and bracket parent are lost, so both legs can fill

**Category:** correctness

**Evidence**

When not armed and not a market order, `submit_bracket` (order_manager.rs:2008-2012) and `submit_oco` (2226) create their legs as `Draft` and skip the multi-leg broker call entirely (`if initial_state == PendingSubmit` at 2064; `if self.armed` at 2272). The UI's SEND badge then calls `confirm_order(id)` and, for a paired leg, `confirm_order(pair_id)` (render/pane/core.rs:9333-9341). `confirm()` submits through `broker.submit(&SubmitArgs{...})` (order_manager.rs:1941-1953) → `POST /orders` with `conId/side/quantity/orderType/idempotencyKey` (broker.rs:364-393). There is no `ocaGroup` field and no bracket parent in that body; `POST /orders/oco` and `POST /orders/bracket` are never reached.

**Impact**

An unarmed OCO placed as "take-profit OR stop" goes to the broker as two unlinked live orders — a gap-through can fill BOTH, flipping the trader into a doubled opposite position. For a bracket, SEND on the entry confirms only the entry (its `pair_id` is None), so the trader gets a naked entry with no stop; SEND on the TP confirms TP+entry and leaves the stop as a Draft that was never sent.

**Fix**

Tag drafts with their group (`oca_group` / bracket parent id) on the ManagedOrder and have `confirm()` dispatch the whole group through `broker.submit_oco` / `submit_bracket` once, rather than per-leg through the single-order endpoint. Until then, block `confirm()` for orders whose `source` is `Oco` or `Bracket`.

---

## [CONFIRMED] In paper mode the poller still adopts REAL broker orders and the paper fill engine then fabricates fills for them, with synthetic prices booked into realized P&L

**Category:** correctness

**Evidence**

`start_account_poller` (trading/mod.rs:394-444) runs unconditionally — it is not paper-gated — and feeds `reconcile_with_ib`. Rule 5 (order_manager.rs:3894-3971) adopts any unmatched broker order into `mgr.orders` as a real ManagedOrder with `state` from the broker. `apply_paper_fills` (1404-1443) filters only on `o.symbol == symbol && o.state.is_active() && o.state != Draft` — it does not check `source`, `backend_order_id`, or whether the order was locally created — and for every match sets `state = Filled`, `filled_qty = o.qty`, `avg_fill_price = Price::from_f32(fp)` where `fp` comes from `paper_fill_price` against a live quote, then calls `record_fill_pnl` (1436) and toasts "PAPER FILLED". It is invoked at the top of `reconcile_with_ib_inner` (3872-3882) on every poll.

**Impact**

This is the fabricated-fill path. With a real IB account reachable and the app in paper mode (the default — see gpu.rs:5358, paper unless `APEX_TRADING_MODE=live`), a genuinely resting live order at the broker is adopted, then declared Filled locally at a price the broker never gave, its P&L is added to `realized_pnl_today`, and that number drives the daily-loss circuit breaker and the P&L panel. The order is still live at the broker.

**Fix**

Restrict `apply_paper_fills` to locally-originated paper orders (`backend_order_id` starting with `paper:`), and skip Rule 5 adoption entirely while `paper_mode` is true — a broker order has no business entering the paper book.

---

## [CONFIRMED] `paper_mode` is not persisted: a restart without APEX_TRADING_MODE=live restores real live orders into paper mode, where cancel is a no-op that marks them Cancelled locally

**Category:** fail-silent

**Evidence**

`OrderManager::new()` sets `paper_mode: true` (order_manager.rs:957). `save_control_flags` persists only `kill_engaged` and `halted` (3759-3768); `save_to_disk` persists orders but not the mode. `load_from_disk` (3279-3319) restores Working orders on startup. gpu.rs:5350-5366 then calls `set_paper_mode(is_paper)` with `is_paper = true` unless the env var says "live" — and the T4 guard at order_manager.rs:3779 only fires `if paper && !mgr.paper_mode`, which is false because the manager already defaulted to paper. In paper mode `issue_cancel` takes the `(true, Some(bid))` arm (2913-2919): `paper::cancel_paper` is an empty function body (paper.rs:14) and the order transitions straight to `Cancelled` with no HTTP DELETE. Rule 4's re-cancel is also paper-gated to the same no-op (4236-4245).

**Impact**

Launch live from a shell, crash or exit, relaunch from Explorer (no env var): every restored live order is now uncancelable through the UI. Clicking cancel shows "Cancelled" while the order stays resting at IB, and the Rule 4 safety net that exists precisely to catch "local cancelled but broker still working" is disarmed by the same flag. The T4 guard documents this exact hazard for the runtime toggle but does not cover the restart path.

**Fix**

Persist `paper_mode` in control_flags.json alongside kill/halt and restore it before `load_from_disk`; if restored orders exist and the mode is ambiguous, fail closed to live (cancels reach the broker) rather than to paper (cancels are no-ops).

---

## [CONFIRMED] Kill switch, halt and resume ignore the broker's HTTP status AND their Result is discarded — a failed server-side kill reports success

**Category:** fail-silent

**Evidence**

broker.rs:571-591: `kill`, `halt`, `resume` are `client.post(...).send().map(|_| ()).map_err(...)`. `.map(|_| ())` throws away the `Response`, so HTTP 401/500/503 all become `Ok(())` — `error_for_status`, which exists in the same impl and is used at lines 400/420/445, is not applied. The callers then discard even that: `let _ = broker.kill();` (order_manager.rs:1501), `let _ = broker.halt();` (1517), `let _ = broker.resume();` (1527), each inside `spawn_guarded` with no result channel. `engage_kill` unconditionally returns `Ok(())` (1504) and `kill_switch()` discards it too (3713).

**Impact**

The kill switch is the last-resort control on a live-money app. If ApexIB is down, unauthenticated, or 500s, the local gate flips and the UI shows KILL ENGAGED, but the server-side kill (which is what stops broker-side automation and pulls resting orders at IB) never happened and nothing anywhere says so. `release_kill` is even documented as "local only — no broker endpoint" (1509), so the states can silently diverge in both directions.

**Fix**

Apply `error_for_status` in kill/halt/resume, propagate the Err back through `engage_kill`/`halt_inner`, and surface a Critical toast plus a persistent banner when the broker-side control call fails, so the operator knows the local gate is the only thing holding.

---

## [CONFIRMED] Risk gates block position-REDUCING orders: once the daily-loss breaker auto-halts, Flatten/Reverse cannot close the position, and the failure is completely silent

**Category:** ux

**Evidence**

`OrderManager::flatten` (order_manager.rs:3175-3200) calls `self.submit(...)` with a market close intent. `submit` rejects on `self.halted` at line 1573 — and `check_daily_loss_and_halt` (1294-1324) sets exactly that flag from the reconcile poll, toasting "open positions NOT auto-flattened". `validate_risk` then applies the daily-loss cap (1273-1282) and the buying-power check (1181-1256) to the closing order with no side/reduce-only distinction — a sell-to-close is compared against `buying_power` as if it consumed margin. The result is dropped: `flatten` ignores `submit`'s return, and `flatten_symbol`/`flatten_all`/`reverse_all` (3433-3484) call `mgr.flatten(...)` directly, bypassing `record_submit_outcome` (3370-3382), so nothing is toasted and nothing reaches errors_sink.

**Impact**

The moment the daily-loss circuit breaker trips — the moment you most need to get flat — "Flatten All" cancels the working orders and then silently fails to submit the closing market order. Same for a position whose notional exceeds remaining buying power. The button appears to work; the position stays open and keeps bleeding.

**Fix**

Add a `reduce_only` flag to OrderIntent; have `flatten`/`halve`/`reverse` set it, and in `submit`/`validate_risk` let reduce-only intents through the halt, daily-loss and buying-power gates (keeping kill, qty=0 and dedup). At minimum, route these through `submit_order` so a rejection is reported and toasted instead of vanishing.

---

## [UNVERIFIED] ApexData watchdog never counts pong/control frames as liveness, contradicting its own comment — a healthy but quiet feed is force-reconnected every ~30s

**Category:** correctness

**Evidence**

data/feeds/apex_data/ws.rs:545-552 sends a Ping every 30s with the comment "server's pong arrives back through rx_ws and is counted in LAST_MESSAGE_AT_MS via the default Message arm (no-op)". The default arm is line 560: `Some(Ok(_))  => {}` — it does nothing. `LAST_MESSAGE_AT_MS` is only stored in `handle_text` (ws.rs:703) and `handle_binary` (ws.rs:715), i.e. only for Text/Binary data frames. The watchdog (ws.rs:632-663) trips `FORCE_RECONNECT` whenever `now - last > STALE_TIMEOUT_MS` (30_000). The sibling IB feed, written from the same template, gets this right: data/feeds/ib_ws/mod.rs:512-514 is `Some(Ok(_)) => { LAST_MESSAGE_AT_MS.store(now_ms(), Ordering::Relaxed); }` with the comment "Ping/pong/text — refresh liveness so the watchdog sees the pong from our own ping when no data flows."

**Impact**

Any period where ApexData emits no data frames for 30s — overnight/pre-market, a halted symbol, or a watchlist of illiquid names — tears down a perfectly healthy socket and reconnects. Each such reconnect also triggers `gap_fill_on_reconnect_all()` (ws.rs:497), compounding finding #1, and emits a user-facing "ApexData feed silent for >Ns — reconnecting" toast (ws.rs:652-657) that misdescribes a live connection.

**Fix**

Store `LAST_MESSAGE_AT_MS` in the `Some(Ok(_))` arm at ws.rs:560, exactly as ib_ws/mod.rs:513 does.

---

## [UNVERIFIED] "Close All" flattens every open position account-wide on a single unconfirmed click, while deleting a workspace, removing widgets, and the identical palette command all require confirmation

**Category:** ux

**Evidence**

orders_panel.rs:125 `.action("Close All", PanelTone::Bear)` and orders_panel.rs:222-225 `if resp.action_clicked && has_positions { order_manager::flatten_all(); }` — one click, no gate. order_manager.rs:3440-3446 `flatten_all()` immediately submits a flattening order for every non-zero position. The per-position X (orders_panel.rs:160-170 → `flatten_symbol`) and the ½ button (:171-178 → `halve_position`) are likewise unguarded. This is inconsistent with every comparable control in the app: command_palette/mod.rs:205-223 explicitly gates `cmd:flatten` / `cmd:cancel` / `cmd:reverse` / `cmd:halfsize` behind a second Enter with the label "Flatten ALL open positions?"; workspace_rail.rs:288-306 puts a ConfirmDialog on deleting a saved workspace; chart_controls.rs:318-333 confirms "Remove all widgets?"; settings_panel.rs:714-748 gates the paper→LIVE toggle.

**Impact**

"Close All" sits in the same PanelSection action slot as the benign "Clear All" in alerts_panel.rs:135 and "Dismiss All" at :170, so it is one mis-aimed click away from every routine action in that panel. The consequence is a market-order liquidation of the entire book. The app already decided a workspace file deserves a confirmation dialog; the position book does not have one.

**Fix**

Wrap Close All (and reasonably the per-position X) in the existing `ConfirmDialog` with `ConfirmTone::Danger`, matching workspace_rail.rs:291-306, and state the position count and net notional in the body. Reuse `destructive_confirm_label("cmd:flatten")` wording so the palette and the panel say the same thing.

---

## [CONFIRMED] Portfolio "NET LIQ" silently displays gross exposure when the account summary is missing, and a disconnected broker still renders a full frozen P&L board

**Category:** correctness

**Evidence**

portfolio_pane.rs:86, 96; also :151 and :231 consume net_liq

**Impact**

In a margin account gross exposure routinely runs several multiples of net liquidation value, so the NET LIQ card can read 3-4x the trader's actual equity with no visual difference from the real figure — directly corrupting position sizing. And after a broker ingress drop (a known failure mode for this stack) the pane keeps presenting yesterday's P&L, unrealized, and buying power as current numbers; the trader has to notice a small grey pill to know none of it is live.

**Fix**

Never relabel a different quantity: when `summary.nav <= 0.0`, render NET LIQ as "—" (or as gross with an explicit "gross" sublabel), not as net liq. Thread a `last_updated_ms` alongside `account_data` and dim/badge the whole metric row with an age when disconnected or when the snapshot exceeds a staleness threshold — the RegimeTape already establishes the "age of the latest frame" convention (regime_tape.rs:5-13).

---

## [CONFIRMED] News panel renders every headline twice, and the second copy's click handler is a dead `// TODO: open URL`

**Category:** redundant

**Evidence**

news_panel.rs:216-227 and 228-255; empty click arm at 251-253

**Impact**

The headline list is visibly doubled — each story appears once outside the scroll area and once inside it. Clicking a headline works or does nothing depending on which of the two identical-looking copies the trader hit, and there is no way to tell them apart. On a news-driven move that is a duplicated, half-inert feed at exactly the moment it needs to be trustworthy.

**Fix**

Delete one of the two loops. Keep the ScrollArea/PanelListRow version (it is the one the module docstring at :10-12 describes) and port the working `open::that(&news.url)` call from :225 into :251.

---

## [UNVERIFIED] News sentiment filter chip (Any/Bull/Bear/Neut) changes its own label and active state but never filters the list — the filter function is only ever called by unit tests

**Category:** unwired

**Evidence**

news_panel.rs:137-143 renders the chip and sets `cycle_sentiment`; :152-154 advances `watchlist.news.sentiment_filter` via `next_sentiment_filter`. The list body, news_panel.rs:201-203, filters on symbol only: `.filter(|n| !watchlist.news.filter_symbol || n.symbol == active_symbol)` — `sentiment_filter` is never read there. The function that actually applies it, `filtered()` at news_panel.rs:170-181, is called from exactly five places, all inside `mod tests` (:280, :287, :295, :303, :311). A repo-wide grep for `sentiment_filter` returns only watchlist_state.rs:418-428 (the field), the chip, the cycler, and test names — no production consumer.

**Impact**

Classic tested-but-unwired: five green unit tests assert the sentiment filter works while the shipped control does nothing. A trader who sets the chip to "Bear" to scan for negative catalysts sees an unfiltered list that now looks filtered (the chip renders `.active(true)`), and will read neutral/bullish headlines as bearish hits.

**Fix**

Replace the inline filter at news_panel.rs:201-203 with `filtered(&watchlist.news.items, watchlist.news.filter_symbol, active_symbol, watchlist.news.sentiment_filter)` — the function already exists and is tested. Note draw_content currently has no access to `sentiment_filter` semantics; pass it in or read it off `watchlist.news`.

---

## [CONFIRMED] Spread Builder renders Max Profit / Max Loss / Break Even from invented option premiums and an invented payoff rule, with no badge and no contract multiplier

**Category:** ux

**Evidence**

spread_panel.rs:149-160 (leg_price), :202-203 (3x rule), :419 compute_spread_metrics(&legs, active_symbol), :424-438 (cards)

**Impact**

These four cards are exactly the numbers an options trader sizes on. With the chain unwired every one of them is derived from a 0.5%-of-strike guess, presented in the same typography as the app's real metrics. The 3x rule reports a bounded max profit for structures that have none (a naked long call). And a "Max Loss $2.50" on 10 contracts is a $2,500 risk — understated by 1000x with no unit label to hint otherwise.

**Fix**

Track whether every leg priced from a live quote; when any leg fell back to the synthetic price, badge the metric block "ESTIMATED — no live chain" using the same treatment as script_panel.rs:439-454. Return `None` rather than `net_premium * 3.0` for unsupported leg counts and render "—". Multiply by 100 and `combo_qty`, or label the cards "per share".

---

## [WEAKENED] Kill switch / halt / resume report success before the broker call runs, discard its Result, and treat HTTP 500/404 as success

**Category:** fail-silent

**Evidence**

order_manager.rs:1496-1531 (engage_kill 1496, release_kill 1507, halt_inner 1513, resume_inner 1523); broker.rs:571-591; error_for_status broker.rs:335-348; mitigation not accounted for: order_manager.rs:1497 -> cancel_all at :2972-3004 (journaled broker cancel_all + per-order hardened cancel)

**Impact**

ApexIB is unreachable or returns 500. The trader hits the kill switch, sees a red "KILL SWITCH — orders cancelled, trading halted" banner plus a `kill: ENGAGED (local + broker)` toast, and believes the account is flat and frozen at the broker. Only the local in-process gate actually flipped. Resting orders at IB stay working, and any other client on the same account keeps trading. Nothing in the error ring, the toast pipeline, or Prometheus records that `/risk/kill` failed. This is the single control whose whole purpose is to be trustworthy under failure, and it is the one that cannot report failure.

**Fix**

Route kill/halt/resume through the existing `Self::error_for_status(resp, op)` helper so non-2xx becomes `Err`. Make `engage_kill`/`halt_inner`/`resume_inner` propagate the broker outcome instead of returning `Ok(())` unconditionally — either block on the call, or move the `report(...)` into the spawned thread so success is only claimed after the POST returns 2xx, and emit `report(ErrorLevel::Critical, "kill", "broker_failed", e)` on the error path. The keyboard handler must render "LOCAL ONLY — broker kill FAILED" when the broker leg errors.

---

## [WEAKENED] Armed order-ticket submit discards OrderResult, so the NeedsApproval soft risk gate silently voids the order

**Category:** fail-silent

**Evidence**

fetch.rs:1569-1600 (last_price: 0.0 set at :1584); gpu.rs:1913-1923 and :1851-1867 bypass; fat-finger gate order_manager.rs:1120-1136 is UNREACHABLE from this path (requires last_price > 0.0); only max_notional :1138-1147 applies

**Impact**

In armed+advanced mode — the mode a trader uses to fire fast — a limit order more than 5% off last, or any order above $50k notional, is silently voided. No order is created, no approval modal appears, no toast, no error-ring entry. The trader clicks BUY and nothing happens, with zero explanation, and may re-click or assume a UI lag while the market moves. The same button in unarmed mode surfaces a proper "Override and submit" dialog, so the behaviour is inconsistent in the more dangerous direction.

**Fix**

Have `submit_ib_order` return `OrderResult` (and the bracket tuple) rather than discarding it, and match on it: `NeedsApproval { reason, .. } => enqueue_approval(reason, intent)` — `enqueue_approval` takes the global manager lock so it is safe from the spawned thread. At minimum, upgrade `record_submit_outcome`'s `NeedsApproval` arm from `count()` to `report(ErrorLevel::Warn, ...)` so no call site can swallow a soft-gate block.

---

## [WEAKENED] flatten / flatten_all / halve / reverse bypass record_submit_outcome, making every rejection — including kill-engaged and rate-limit — completely invisible

**Category:** fail-silent

**Evidence**

order_manager.rs:3175-3199 (flatten), :3433-3446 (flatten_symbol/flatten_all) — confirmed silent. :3456-3461 (halve_position) and :3474-3487 (reverse_all) route through submit_order at :3385-3390 -> record_submit_outcome :3373 report(Warn) — NOT silent on Rejected

**Impact**

Concrete sequence: operator hits the kill switch (sets `kill_engaged`), then hits "Flatten All" to exit. Every submit returns `Rejected("kill switch engaged")` and is discarded — no position is closed, and the UI says nothing at all. Second sequence: flattening more than 20 positions at once drains the 20-token submit bucket; the remaining symbols are rejected with "submit rate limit exceeded" and left open, silently. In both cases the trader believes they are flat while carrying full exposure.

**Fix**

Route `OrderManager::flatten` through the reporting path (or call `record_submit_outcome` on its result inside `flatten`), and have `flatten_all`/`halve_all`/`reverse_all` accumulate per-symbol outcomes and surface a summary toast: "Flattened 18 of 25 — 7 rejected (rate limit)". Replace the `let _ =` at `:3460` and `:3481` with a match that at minimum `report()`s non-Accepted results.

---

## [CONFIRMED] Gamma/GEX feed outage is completely silent and stale walls are served indefinitely as live data

**Category:** fail-silent

**Evidence**

fetch.rs:114-149 (fetch), :296-313 (spawn), :319-345 (refresh, staleness used only at :339), :205-225 (evict at cap only); call site core.rs:10461

**Impact**

The `:8412` gamma feed dies mid-session. The chart keeps painting the call wall, put wall, zero-gamma flip, and PPE from the last successful poll — potentially hours old — with no SYNTHETIC badge (that flag is only set by `populate_gamma`, not by this path), no staleness indicator, no toast, and no error-ring entry. These levels drive discretionary entries and exits. Trading against yesterday's or this-morning's gamma structure as if it were live is a direct path to real losses, and there is no signal anywhere that would let the trader or an operator notice.

**Fix**

Give `fetch_gamma_from_feed` a typed error and `report(ErrorLevel::Warn, "gamma_feed", "unreachable", ...)` on failure (rate-limited, e.g. once per minute). In `refresh_gamma_feeds`, apply the cached snapshot only when `at.elapsed() < STALE_LIMIT`; past that, either clear `p.gamma_levels` or set a new `gamma_stale` flag and paint a "STALE {n}m" badge next to the existing SYNTHETIC badge site at `core.rs:11436`.

---

## [CONFIRMED] Panel Close/Half/Reverse market orders carry price=0 and last_price=0, so the max_notional risk gate silently evaluates $0 and never fires

**Category:** fail-silent

**Evidence**

order_manager.rs:3421-3429, :3184/:3193, gate :1138-1155, backstop :722-737, bp recovery :1196-1246, paper early-return :1083

**Impact**

`reverse_all` opens a brand-new full-size position on the opposite side via `market_order_intent`, and the $50k per-order notional cap — the fat-finger sizing backstop — is bypassed because the gate is evaluated against a fabricated $0. The gate reports nothing when it skips, so a review of the risk configuration would show the cap as enabled and enforced. Live mode only (`validate_risk` returns early in paper, `:1083`), which is precisely where it matters.

**Fix**

Populate `last_price` in `market_order_intent` and `OrderManager::flatten` from `apex_data::live_state::get_snapshot_with_age` (the same source the buying-power check already uses at `:1206`) so the notional gate has a real number. If no price is available, `report(ErrorLevel::Warn, "risk", "notional_check_skipped", ...)` — mirroring the existing `bp_market_no_price` warning — so a skipped gate is never silent.

---

## [CONFIRMED] VWAP is implemented twice with different session-reset rules; the σ-band version never resets on crypto or futures

**Category:** inconsistency

**Evidence**

compute.rs:324-357; gpu.rs:4676-4706 (gap>14400 only at 4682-4683); render core.rs:11667-11671

**Impact**

On a BTC or NQ chart the σ-band VWAP anchor drifts to wherever the loaded history happens to start, so the ±1σ/±2σ bands are meaningless and the VWAP level a trader fades or targets is wrong by an arbitrary amount. On US equities the two implementations agree, which is why this survives casual inspection.

**Fix**

Delete the inline loop at gpu.rs:4676-4706 and have `compute_volume_analytics` call `compute::compute_vwap` for the mean, computing only the σ accumulators alongside it. Then replace the UTC-day heuristic in `compute_vwap` with an explicit session-open rule per asset class (equities 09:30 ET / extended 04:00 ET, CME futures 18:00 ET, crypto 00:00 UTC), reusing the existing `rth_start_minutes` (gpu.rs:2625) and the ET offset logic already at core.rs:2289-2300.

---

## [CONFIRMED] RSI and ATR each exist in three implementations with three different smoothing conventions, all displayed simultaneously

**Category:** redundant

**Evidence**

chart_widgets.rs:905 (rsi), :910 (atr) — not :1045; rest of citations accurate

**Impact**

The RSI widget and the RSI indicator pane on the same chart show different numbers for the same setting, and the widget's 70/30 colour thresholds (chart_widgets.rs:547) fire at different moments than the indicator's overbought/oversold lines. The volatility cone's projected range is built on a non-Wilder ATR that differs from every ATR the user can add as an indicator. A trader cross-checking two readouts of "RSI 14" has no way to know which is authoritative.

**Fix**

Make `ui/overlays/indicators.rs::compute_rsi/compute_atr` thin wrappers that build the `&[f32]` slices and call `compute::compute_rsi`/`compute_atr`, returning the last non-NaN element. Same for the inline cone ATR at core.rs:6667. That is a strictly-behaviour-changing fix, so land it with a note that widget values will shift to match the indicator pane — which is the point.

---

## [CONFIRMED] The multi-timeframe RSI widget labels seven rows 5m…1W but computes all seven on the pane's current timeframe

**Category:** correctness

**Evidence**

chart_widgets.rs:739-740 confirm both widgets are dispatched (not dead)

**Impact**

A trader reading "1W RSI = 68" on a 5-minute chart is actually seeing RSI(140) on 5-minute bars — roughly an 11-hour lookback, not a week. On a daily chart the same row means RSI(140) on daily bars ≈ 7 months. The widget is presenting fabricated timeframe attribution as multi-timeframe confirmation, which is exactly what such a widget is used to justify sizing up a trade.

**Fix**

Either relabel the rows by their true periods ("RSI 7", "RSI 10", …) — a one-line honest fix — or wire real multi-timeframe bars through the same fetch path `fetch_indicator_source` uses and compute RSI(14) on each. Do not ship timeframe labels over single-timeframe data.

---

## [WEAKENED] The kill switch is one-way: `release_kill_switch()` has no caller, so engaged=true is unrecoverable in-app

**Category:** unwired

**Evidence**

chart/renderer/trading/order_manager.rs:3725 (dead fn); chart/renderer/ui/components/toolbar/top_nav.rs:316-352 (banner that already exists, refuting the "silent/invisible" claim)

**Impact**

After one Ctrl+Shift+K — or one corrupt/truncated control_flags.json after a crash — the terminal silently rejects every order forever, across restarts, with a green "Trading RESUMED" toast actively lying about the state. The only recovery is hand-editing a JSON file next to the exe. On a live-money terminal this is the difference between being flat and being unable to exit a position.

**Fix**

Wire `release_kill_switch()` to Ctrl+Shift+R (call it alongside `resume_trading()`), and gate the "Trading RESUMED" toast on `!is_trading_blocked()` so the notification can never contradict the gate. Add a visible KILL-ENGAGED banner driven by `is_trading_blocked()` so the state is never invisible.

---

## [WEAKENED] Screener BUILD tab: "Run Now" and "Save Screen" are literal no-op handlers, and their reducers are stubs

**Category:** unwired

**Evidence**

screener_build.rs:998-1013 (verified verbatim); screener_panel.rs:707-710 is the Build dispatch (finding cited :516 for draw_build_tab, actual definition is screener_build.rs:516)

**Impact**

A trader composes a multi-condition screen (visual tree or DSL), clicks Save Screen, and nothing happens — no error, no toast, no persisted screen. The work is lost on the next Cancel or restart. The LIBRARY empty state directs users into this dead end. This is the largest single unwired surface found.

**Fix**

Either (a) wire the two buttons to `commands::push(AppCommand::BuilderCommit)` / `BuilderSave`, move the builder's module statics into `ScreenPanelState::screen_builder` so the reducers operate on the same store, and implement the REST POST in the `BuilderSave` reducer; or (b) if the backend is not ready, disable both buttons (`.disabled(true)`) and label the tab PREVIEW so the UI stops lying about what it can do.

---

## [CONFIRMED] Time & Sales tape is permanently empty: the WS subscription is gated on `tape.open`, a flag no user action can set

**Category:** unwired

**Evidence**

core.rs:10372/10379, ws.rs:297-300, tape_panel.rs:150 — all verbatim; no dev_inspector writer for tape_open

**Impact**

Tape reading is a core discretionary-trading input. The T&S tab renders an empty list forever for stocks and options (crypto still works — crypto_feed.rs:82 pushes `ChartCommand::TapeEntry` directly, bypassing the ApexData subscription), and the per-frame empty set silently unsubscribes anything the provider API subscribed. A trader would reasonably conclude the venue has no prints.

**Fix**

Drive the gate from what is actually visible, not from the dead rail flag: set `tape_syms` when the Analysis panel is open AND its active tab is `TimeSales` (or when `watchlist.tape.open`), and merge rather than replace so `providers::apex_data::subscribe_trades` subscriptions survive the per-frame push.

---

## [CONFIRMED] Two independent shortcut systems collide: Ctrl+Shift+R resumes halted trading AND toggles a panel in the same frame

**Category:** correctness

**Evidence**

top_nav.rs:1822-1830 (MSG toggles), keyboard_shortcuts.rs:441 adds a `!ctx.wants_keyboard_input()` guard the finding did not mention — narrows but does not remove the collision

**Impact**

After a kill switch (Ctrl+Shift+K halts trading, keyboard_shortcuts.rs:446-457), a trader who presses Ctrl+Shift+R intending to open the MSG RRG panel silently RESUMES live trading. That is a safety-interlock bypass in a live-money app. The Ctrl+Shift+S collision is merely annoying (a stray screenshot entry + an OS overlay popping over the terminal).

**Fix**

Route the top_nav toggles through `binding_pressed`/`watchlist.hotkeys` (or the `foundation::shortcuts` registry) instead of raw `ctx.input`, and add a startup assertion/test that no two registered actions share a chord — the existing `defaults_reproduce_the_old_hardcoded_bindings` test (keyboard_shortcuts.rs:482) checks drift but not collisions.

---

## [CONFIRMED] Theme-invariant categorical colours (incl. stop/target/R:R chart lines) score 1.1:1–3.0:1 contrast on all 5 light ColorSchemes

**Category:** design-system

**Evidence**

chart/renderer/ui/style.rs:827-849; chart/renderer/render/pane/core.rs:5194,5195,5214,11947,11953

**Impact**

A trader who selects any of the 5 light palettes cannot read their own stop, target, or risk-reward levels on the chart, and order/status badges become invisible. The palette axis is presented as 21 freely-selectable options; 5 of them silently degrade trading-critical readouts. This is a money-losing failure mode, not a cosmetic one.

**Fix**

Two-tier the categorical palette: keep the hue identity but derive lightness from `cs.meta.is_dark` (the mechanism already exists — `chart/renderer/ui/theme_studio.rs:810-815` correctly switches WHITE↔BLACK on `is_dark`). Add a `categorical: [Rgba; N]` block to `ColorScheme` so each palette ships its own readable ramp, make `COLOR_*` resolve through it, and add a CI test that asserts ≥3:1 for every categorical colour × every builtin ColorScheme bg. Route the ~135 literal call sites through the same accessor.

---

## [UNVERIFIED] Style hot-reload logs a successful StyleSystem reload but applies only radii and strokes — typography, spacing, density, treatments and chrome edits are silently dropped

**Category:** fail-silent

**Evidence**

`design_system/hot_reload.rs:137-139` documents the watcher as reloading "`StyleSystem` overrides (radii, strokes, typography, density, treatments)", and `hot_reload.rs:158-161` prints `[theme-watcher] reloaded StyleSystem '{}' from {:?}` on every change before calling `install_override(style)`. The override slot has exactly one consumer: `chart/renderer/ui/style.rs:76` `let override_style = crate::design_system::active_override();`. That consumer reads `ov.radii.xs/sm/md/lg` (`style.rs:88`) and `ov.strokes.thin/std/md/heavy` (`style.rs:101-107`) and nothing else — `ov.typography`, `ov.spacing`, `ov.density`, `ov.treatments`, `ov.chrome`, `ov.shadows`, `ov.alphas`, `ov.elevation` are never touched. `grep active_override()` across the tree returns only `style.rs:76` plus the definition and its own tests.

**Impact**

This is the live design-iteration loop. An author edits `themes/styles/*.json`, sees a confirmation line in the terminal saying the style reloaded, and concludes their typography/density/treatment change had no visual effect — i.e. the tool lies about success. Design sessions get burned chasing a change that was never applied.

**Fix**

Route the override through the same total adapter the preset path uses: replace the ad-hoc radii/stroke extraction in `begin_frame` with `style_system_to_style_settings(&ov)` (`chart/renderer/ui/style.rs:2491`) pushed into the active `StyleSettings` slot, so the override and preset paths are literally the same code. If that is too large a change now, at minimum change the watcher log to name exactly which sub-structs were applied and warn on the ones ignored.

---

## [CONFIRMED] Two live type ladders have drifted apart: the tier scale was lifted to 9/10/12/14 but the semantic StyleSettings ladder still renders 8/10/11 — TextStyle::Caption is now BELOW the tier floor

**Category:** inconsistency

**Evidence**

chart/renderer/ui/style.rs:126-131 (lift), :2566-2569 (adapter), :2378 + :2436 (live style_defaults), foundation/text_style.rs:59-70

**Impact**

The fix for "scrunched up and unreadable" text was applied to only one of the two ladders. Every surface migrated to the newer, preferred `TextStyle::*` API still renders at the old, rejected sizes, and adjacent widgets in the same panel differ by 2px for what is nominally the same tier. Switching styles also silently changes body text between 10px and 13px with no corresponding tier change.

**Fix**

Pick one ladder. The cheapest coherent move is to define the semantic roles as derived from the tier tokens (`font_caption = font_2xs()`, `font_body = font_sm()`, `font_section_label = font_xs()`) with per-style offsets rather than absolute px, then re-baseline every builtin `Typography` in `design_system/builtin.rs`. Add a test asserting `font_caption >= font_2xs()` and `font_body >= font_xs()` for every builtin style so the ladder can never invert again.

---

## [UNVERIFIED] FROZEN CHROME: DesignTokens::default() is pinned to the pre-lift scale, so design-mode builds render a smaller type scale than shipping and RESET restores the wrong look

**Category:** design-system

**Evidence**

`foundation/design_tokens.rs:310-312` reads: "Aligned to the canonical `ui_kit::style` constant fallbacks so RESET restores the app's shipped look, not a different tier" followed by `font: FontTokens { xxs: 8.0, xs: 9.0, sm_tight: 10.0, sm: 11.0, md: 13.0, ... }`. The shipped look is 9/10/12/14 (`ui_kit/style.rs:137`, `chart/renderer/ui/style.rs:126-130`). The macro `dt_f32!` (`foundation/design_tokens.rs:473-486`) returns `t.font.sm` whenever `design_tokens::get()` is `Some`, and `native_main.rs:79-83` unconditionally calls `design_tokens::init(tokens)` with `unwrap_or_default()` when the `design-mode` feature is on. So a design-mode build with no/partial `design.toml` renders the app at 8/9/11/13. `Typography::default()` (`design_system/style_system.rs:170-192`) carries the identical stale claim — "P2.2: aligned to TokenSnapshot DEFAULT (the values the live frame renders with via frame_tokens())" — with `size_xs: 9.0, size_sm: 11.0, size_md: 13.0`, again the pre-lift values.

**Impact**

`cargo design` (the documented live-token / F12 inspector workflow for this repo) shows a different type scale than the shipping binary, and the inspector's "reset to defaults" pushes the rejected old scale into the live frame. Every UI judgement made in the design tool is made against the wrong rendering. The two "aligned to…" comments make the drift invisible to review.

**Fix**

Make `DesignTokens::default()` construct its font block from `DEFAULT_TOKEN_SNAPSHOT` rather than restating literals, and do the same for `Typography::default()`. Add an equivalence test (`design_system/equivalence_tests.rs` is the natural home) asserting `DesignTokens::default().font.sm == DEFAULT_TOKEN_SNAPSHOT.font_sm` and `Typography::default().size_sm == DEFAULT_TOKEN_SNAPSHOT.font_sm`.

---

## [WEAKENED] Broker order URL is a hardcoded dev-host const; the runtime-configurable resolver built for it is an orphan file that never compiles

**Category:** unwired

**Evidence**

chart/renderer/trading/config.rs (orphan, no `mod config` anywhere) + trading/mod.rs:22 + gpu.rs:1771-1773

**Impact**

Two consequences, both live-money. First, order routing is pinned at compile time to a host literally named `apexib-dev` with no supported way to repoint it. Second, `APEXIB_HTTP` is a real, documented knob that only moves market data: set it to a production ApexIB and the terminal shows prod chains and prices while every order still goes to dev — a data/execution split-brain with no visible indication. The env var name is also inconsistent between the two live paths (`APEXIB_HTTP`) and the dead one (`APEXIB_URL`).

**Fix**

Delete chart/renderer/trading/config.rs or wire it (`pub(crate) mod config;`) and route ALL 41 `APEXIB_URL` sites plus the 8 `apexib_url()` sites through the single resolver. Settle on one env var name. Until then, at minimum make broker.rs call `gpu::apexib_url()` so data and orders cannot diverge, and surface the resolved broker host in the connection panel.

---

## [CONFIRMED] Two divergent RSI implementations and two divergent ATR implementations, both rendered on screen at the same time

**Category:** inconsistency

**Evidence**

chart/renderer/ui/chart_widgets.rs:28 (glob import is what selects the SMA variant — no competing `compute::` import exists in that file)

**Impact**

The RSI(14) plotted in the indicator pane and the RSI(14) in the widget readout / MTF grid are different numbers for the same bars — after a sharp move they can straddle the 30/70 thresholds in opposite directions. Same for ATR, which the widget converts to `atr_pct` (chart_widgets.rs:911) and traders use for stop sizing. Two contradictory readings of the same named indicator on one screen undermines trust in every other number shown.

**Fix**

Delete `compute_rsi`/`compute_atr` from ui/overlays/indicators.rs and have the widget layer call `compute::compute_rsi(&closes, p)` / `compute::compute_atr(...)` and take the last valid value. If a scalar convenience is wanted, add `compute::rsi_last(&closes, p) -> f32` that delegates to the Wilder series so there is exactly one algorithm.

---

## [CONFIRMED] Three writers into watchlist.chain.near/far; the per-frame cache-derive clobbers the command path and never clears the PLACEHOLDER flag

**Category:** architecture

**Evidence**

chart/renderer/render/pane/core.rs:10272-10273 (writer 3); gpu.rs:4801-4825 / 8305-8322

**Impact**

Once `fetch_chain_background` falls through to `send_placeholder_chain` (fetch.rs:505-521, placeholder=true) and the cache later fills from the WS prime subscription, writer 3 fills the grid with REAL rows while the flag stays true — the panel keeps showing the banner whose stated purpose is "so the user doesn't trade off fake bids" (watchlist_panel.rs:1524-1527) on top of genuine data. Conversely all of writer 1's work (num_strikes trimming, spot resolution, InFlight completion) is discarded on the next frame. Two of the three writers also disagree about whether `underlying_price` should be applied.

**Fix**

Pick one owner. Either delete the per-frame derive and make `refresh_chain_rest` emit a `ChainData` command like the other fetchers, or delete the command handlers and have the frame path own the placeholder flags too (setting them false whenever it writes cache-derived rows). The gpu.rs:8305 duplicate should call the same helper as gpu.rs:4801 rather than being a hand-copied variant.

---

## [CONFIRMED] The full option chain is cloned and re-materialized twice per frame on the UI thread, in a call the codebase itself documents as too expensive to call per-frame

**Category:** perf

**Evidence**

chart/renderer/render/pane/core.rs:10206 (get_chain, ungated) + :10256-10273 (double materialization); live_state.rs:500-510

**Impact**

At 60 fps this is roughly 100k row clones/second plus ~200k String allocations/second on the render thread, for data that changes at most every 6 seconds. This is exactly the work the command path (`fetch_chain_background`) was built to do off-thread; the duplicate frame path undoes that, and the cost lands on the same thread that draws the price ladder and order entry.

**Fix**

Materialize near/far only when the cache generation changes. `merge_chain_delta`/`seed_chain` already stamp `chain_touched` (live_state.rs:495-497) — store the last-seen Instant on `watchlist.chain` and skip the whole block when it hasn't advanced, or add a `get_chain_generation(underlying) -> u64` counter.

---

## [CONFIRMED] Two type scales run in the same frame: TextStyle is style-live, ui_kit's font_*() is frozen to literals in every shipping build

**Category:** design-system

**Evidence**

ui_kit/style.rs:375-381 and chart/renderer/ui/style.rs:126-131 (line numbers shift a few from the report; content is as claimed)

**Impact**

Switching style preset resizes text drawn through TextStyle but leaves every ui_kit widget (buttons, inputs, dropdowns, toasts) at 10/12/14/16px — a style change produces a half-restyled UI. The scales also disagree at rest: Meridien's `font_caption`/`font_section_label` are 8.0 (design_system/baseline.rs:102) while `ui_kit::font_xs()` is 10.0. And `StyleSystem::Typography`/`Spacing`/`Alphas`/`Shadows` — authored per-style in builtin.rs, DTCG round-tripped, validated by theme_pack/validate.rs (736 lines) — are inert in shipping builds for everything except radii and strokes.

**Fix**

Source TokenSnapshot's font/gap/alpha/shadow fields from `current()` (StyleSettings already carries them via the adapter) with `dt_f32!` layered on top as the design-mode override, rather than using `dt_f32!` as the sole source. That makes one snapshot the single scale and lets style presets actually move type and spacing in release builds.

---

## [UNVERIFIED] dev_inspector's HTTP server is an unauthenticated, browser-reachable control plane that can synthesize real clicks into a window that may be in live-trading mode

**Category:** correctness

**Evidence**

`dev_inspector/server.rs:85-120` binds `127.0.0.1:<7892>` and spawns a thread per connection. `read_request` (server.rs:132-185) parses only the request line and Content-Length — it never inspects `Origin`, `Host`, `Referer`, or `Content-Type`, and `parse_body` (server.rs:220-222) runs `serde_json::from_slice` on the body regardless of declared type. There is no token, no auth header, no allowlist anywhere in the 2,662-line file (grep for origin/host/auth returns only the OPTIONS block). The comment at server.rs:235-236 claims "omitting the header prevents web-page CSRF" — that is wrong: absent CORS headers block a browser from *reading* the response, not from *sending* a simple cross-origin POST with `Content-Type: text/plain`, which reaches the handler and executes. `POST /input` (server.rs:338-348) and `POST /input/sequence` (server.rs:349-363) push `DevInput` into a queue; `input_queue.rs:26-31 drain_inputs_raw` converts them into `egui::Event::PointerButton`/`Key`/`Text` and `gpu.rs:7521-7525` splices them into the real window's `raw_input` immediately before `egui_ctx.run(...)` — the module doc (input_queue.rs:1) says they are "indistinguishable from real user input". `POST /cmd` (server.rs:320-337) dispatches arbitrary AppCommands. The module is `#[cfg(debug_assertions)]` (lib.rs:12-13) — but live trading is gated only on an env var, not the build profile: `gpu.rs:5358-5361` reads `APEX_TRADING_MODE`, so a debug build with `APEX_TRADING_MODE=live` is a real-money window. Cargo.toml:143-152 deliberately tunes the dev profile to "perform near-release", i.e. the debug build is the daily driver.

**Impact**

Any web page open in a browser on the trading workstation (or any local process, or any attacker with a DNS-rebinding foothold) can drive the trading UI of a live-money terminal: move the pointer, click Place Order / Close All / Flatten, and type into the order ticket. There is no credential to steal and no log entry distinguishing injected input from the trader's own.

**Fix**

Require a per-process bearer token (write it to a file the harness reads) on every mutating route; reject any request carrying an `Origin` or `Referer` header, and any request whose `Content-Type` is not `application/json`; correct the false comment at server.rs:235. Optionally gate the whole server behind an explicit `APEX_DEV_INSPECTOR=1` opt-in and hard-refuse to start when `APEX_TRADING_MODE=live`.

---

## [UNVERIFIED] Every inbound WebSocket bar frame performs a synchronous file open/append/close on the tokio WS runtime thread

**Category:** perf

**Evidence**

`lib.rs:114-115` calls `crate::apex_log!("ws.bar", ...)` inside the `Frame::Bar | Frame::Snapshot` arm of the `subscribe_to_frames` callback — i.e. once per inbound bar/snapshot frame, for every subscribed symbol. `lib.rs:163` does the same for every `Frame::ChainDelta` (option-chain deltas). The macro (`data/feeds/apex_data/debug_log.rs:54-58`) expands to `debug_log::write(tag, &format!(...))`, so the `format!` allocation is unconditional — there is no level check. `write` (debug_log.rs:31-45) then does, per call: a `tracing::info!` emit, plus `OpenOptions::new().create(true).append(true).open(...)` + `writeln!` + implicit close — three syscalls, blocking, no buffering (the doc at debug_log.rs:4-6 states the file is deliberately "closed between writes" so nothing is buffered). This runs on the WS reader task: `data/feeds/apex_data/ws.rs:700 handle_text` → `dispatch` → `ws.rs:696-699 broadcast` invokes every listener synchronously, and the WS runtime is the 2-worker `tokio` runtime. Separately, the 10 MB truncation guard (debug_log.rs:22-27) sits inside the `OnceLock` initializer, so it is evaluated exactly once per process — the file is unbounded for the whole session.

**Impact**

Blocking disk I/O inside an async worker on the market-data hot path: with dozens of subscribed symbols the WS runtime spends its budget in the filesystem instead of dispatching frames, delaying live bars and chain deltas for every symbol and every consumer. It also writes an unbounded log to %TEMP% during a trading session despite a size cap the code advertises but never re-checks.

**Fix**

Drop the direct file write from `debug_log::write` and let the already-installed non-blocking `tracing_appender` layer own the file (telemetry.rs:41-42), or at minimum hold the `File` open in a `OnceLock<Mutex<File>>` and re-check size periodically. Then demote `lib.rs:114` and `lib.rs:163` to `tracing::trace!` (or gate them on an env flag) so the per-frame `format!` disappears in normal operation.

---

# P2 / P3 — Medium and low

Condensed. Full evidence for these is in the workflow journal.


## P2

- **[CONFIRMED]** A partially-filled order books P&L on its FULL size — `filled_qty.max(qty)` — feeding a wrong number into the daily-loss circuit breaker — `correctness`
- **[CONFIRMED]** `find_local_match` falls back to (symbol, side, qty) with no price or timestamp — a fill can be attributed to the wrong local order — `correctness`
- **[CONFIRMED]** An OCO leg whose conId lookup fails is silently dropped, and the surviving legs' backend ids are then mapped positionally onto the local orders — `fail-silent`
- **[CONFIRMED]** Orders in PendingCancel / PendingModify / Unknown are not persisted — the states where we are least sure what the broker holds are the ones dropped on restart — `correctness`
- **[CONFIRMED]** Redis bar cache does a blocking call behind one process-global Mutex, with no connect timeout, directly inside an async fn — `perf`
- **[UNVERIFIED]** SubscriptionManager::check_stale() has zero callers — the documented per-subscription staleness TTL never fires — `unwired`
- **[CONFIRMED]** Option-chain and realized-delta caches are exempt from the live_state eviction pass that covers every other per-symbol map — `perf`
- **[CONFIRMED]** Per-frame ws::set_quotes at ~60Hz still costs a full SubState clone + sort + JSON serialize on the 2-thread WS runtime, even when suppressed — `perf`
- **[UNVERIFIED]** IB feed bumps the gap-fill anchor with a hardcoded "5m" timeframe inside a loop that already knows the active timeframes — `inconsistency`
- **[WEAKENED]** Closing ANY chart window permanently kills debounced persistence for the whole process — `correctness`
- **[CONFIRMED]** Every chart window creates its own 6 Stores pointed at the SAME 6 file paths, and they are never unregistered — closed windows overwrite live ones on quit — `architecture`
- **[CONFIRMED]** dom_feed's single global ACTIVE_SYMBOL is driven per-frame from per-pane state — two open DOM ladders on different symbols cause a permanent reconnect storm — `coupling`
- **[CONFIRMED]** atomic_write uses a fixed shared `<path>.tmp` sibling, and two threads can write the same store path concurrently — `correctness`
- **[CONFIRMED]** The designed state architecture covers 6 of ~193 module-level globals — the rest is ad-hoc accretion with no ownership model — `architecture`
- **[CONFIRMED]** SUBMIT SPREAD can never succeed in any mode — the order manager unconditionally rejects the $0 limit the panel hardcodes — yet the button stays enabled and the disclosure says only live is blocked — `ux`
- **[CONFIRMED]** Spread strategy presets build strikes around hardcoded stale prices (SPY 580, NVDA 900) instead of the live underlying — `correctness`
- **[WEAKENED]** Prometheus apex_feed_state is hardwired for three of four feeds — crypto and signals can never leave Idle, ib_ws can never report Subscribed — `fail-silent`
- **[UNVERIFIED]** gamma_synthetic badge is never cleared by the per-frame refresh, so real GEX data keeps painting a SYNTHETIC warning — `fail-silent`
- **[CONFIRMED]** compute_trend_grid's EMA column is identical in all 7 timeframe rows and is not an EMA — `correctness`
- **[CONFIRMED]** ATR percentile ranks current volatility against the oldest bars in the buffer, not a recent lookback — `correctness`
- **[CONFIRMED]** Chart-widget data is cached on bar count alone, so RSI/ATR/price/pivots are frozen for the entire duration of the building bar — `fail-silent`
- **[CONFIRMED]** MA ribbon cache avoids the EMA math but deep-clones six full-length Vec<f32> every frame — `perf`
- **[WEAKENED]** Conditional orders and options-trigger orders are implemented end-to-end but have no production entry point — `unwired`
- **[WEAKENED]** `chart/state` — 2,583 LOC of chart-storage architecture — is unreachable; its single integration point is hardcoded `None` — `dead-code`
- **[CONFIRMED]** ui_kit's dead surface is hidden by six module-wide `#[allow(dead_code)]` blankets — `dead-code`
- **[WEAKENED]** Three user-editable hotkeys are read by nothing, including "Halt Trading" — and the F1 cheatsheet advertises a Halt chord that does something else — `fail-silent`
- **[WEAKENED]** The whole ApexSignals integration — not just the three MSG panels — defaults to http://localhost:8100 with no way to configure it — `fail-silent`
- **[CONFIRMED]** Two complete, tested panels have zero call sites anywhere in the tree — `unwired`
- **[CONFIRMED]** Command-palette Help and Calc entries execute to nothing — the dispatcher has no arm for their ids — `unwired`
- **[CONFIRMED]** Four rail panels are registered in the dispatch table but their `is_open` predicate has no writer that can ever return true — `unwired`
- **[CONFIRMED]** News headline clicks are a no-op: the URL is present and non-empty-checked, then discarded — `unwired`
- **[WEAKENED]** Trading-safety bootstrapping (orphan recovery, kill-switch restore, broker watchdog) only runs if wgpu device creation succeeds — `architecture`
- **[WEAKENED]** Live order submission executes inline inside a 9,632-line function that the module doc declares frozen from refactoring — `architecture`
- **[WEAKENED]** Blocking 5-second Postgres round-trip executed on the winit UI thread inside WindowEvent::RedrawRequested — `architecture`
- **[WEAKENED]** Headless scenario testing asserts against a shadow state machine that never touches Chart, Watchlist, or OrderManager — `architecture`
- **[CONFIRMED]** Two divergent ApexIB base-URL resolvers; the live order path is pinned to a compiled-in dev host and ignores the env override — `correctness`
- **[UNVERIFIED]** `gpu.rs` is misnamed: 96% of its 10,690 lines are not GPU code, and the real GPU pipeline is a different module — `architecture`
- **[CONFIRMED]** Bidirectional chart ↔ data dependency: market-data feeds reach up into the renderer, and the crate root is the feed→UI adapter — `coupling`
- **[CONFIRMED]** 73% of the AppCommand dispatch layer has no producer in a release build — its only callers are the debug-only dev_inspector — `unwired`
- **[CONFIRMED]** No single HTTP/endpoint layer: 16 files build their own reqwest client and the ApexSignals base URL is re-derived from env at 8 independent sites — `inconsistency`
- **[WEAKENED]** FROZEN CHROME: FONT_* consts are pinned to the pre-lift scale and their doc comment now asserts a false equivalence; 45 live call sites render 1-2px small — `design-system`
- **[CONFIRMED]** The hot-reload path remaps stroke tiers differently from the preset path, so the same StyleSystem paints borders up to 2× thicker when loaded from JSON — `correctness`
- **[CONFIRMED]** ThemeRegistry / DesignSnapshot (979 LOC) are documented as the canonical active-pair state but have zero references outside design_system/ — `dead-code`
- **[CONFIRMED]** Whole StyleSystem sub-structs (alphas, elevation) and 11 further fields are inert, yet the design inspector ships sliders for them — `unwired`
- **[CONFIRMED]** paint_bevel hardcodes white/black and documents itself as palette-independent, producing a one-sided dark smear on light palettes — `design-system`
- **[WEAKENED]** Settings font picker bypasses both the style-font arbitration and the FontRegistry install path — `correctness`
- **[WEAKENED]** The ui_kit verification harness (apex-playground) never installs fonts or the icon font, and runs under a different host than production — `fail-silent`
- **[CONFIRMED]** The layer_guard ratchet has a glob-shaped hole; a real chart-layer dependency inside ui_kit currently passes as clean — `architecture`
- **[WEAKENED]** Two SegmentedControl types and two Select types, both live in production, with mutually incompatible call conventions — `redundant`
- **[CONFIRMED]** ContextMenu (507 LOC) and Popover (173 LOC) have zero production callers; the app uses raw egui context menus in 22 places — `dead-code`
- **[CONFIRMED]** Label adoption is ~15%: 295 raw ui.label calls in production panel code vs 51 kit Label uses, across five competing label abstractions — `design-system`
- **[CONFIRMED]** ~980 lines of a fully-built parallel theme model (ThemeRegistry / ActiveTheme / DesignSnapshot) has zero production callers, and a test comment falsely claims begin_frame uses it — `dead-code`
- **[CONFIRMED]** Hot-reload StyleSystem override maps the wrong stroke tiers, thickening every hairline the moment a theme JSON is present — `inconsistency`
- **[CONFIRMED]** The strikes-overlay chain fetch is a third parallel path that neither reads nor seeds the shared chain cache — `redundant`
- **[CONFIRMED]** The shared HTTP client introduced to fix per-call TLS handshakes was adopted by only two files; the pre-trade margin check still builds a fresh client per call — `redundant`
- **[UNVERIFIED]** The cooperative-shutdown subsystem is entirely dead: `drain_all` has zero callers, so the Postgres pool is never closed and the bug its own doc claims to fix is unfixed — `dead-code`
- **[UNVERIFIED]** No CI job ever compiles the shipping artifact: every job is `--lib` only, debug-only, and ubuntu-only, so both `[[bin]]` targets, the release configuration, and all 30 `cfg(windows)` sites are never type-checked — `architecture`
- **[UNVERIFIED]** The quality-gate ratchet — the CI job whose job is to block regressions — is currently failing on committed HEAD, so it no longer distinguishes new regressions from old ones — `inconsistency`

## P3

- **[CONFIRMED]** `trading/config.rs` is dead and the broker URL is a hardcoded dev host — there are three competing URL mechanisms and the order path honours none of the overrides — `dead-code`
- **[WEAKENED]** Option-chain cache is invalidated on a server `resync` frame but NOT on a client-side reconnect, and a non-empty cache hard-short-circuits the REST re-seed — `correctness`
- **[WEAKENED]** bar_cache key has no range dimension, so a cache hit serves whatever range happened to be stored first, ignoring start_ms/end_ms/limit — `correctness`
- **[CONFIRMED]** providers/mock.rs (546 lines) and providers/replay.rs (212 lines) are compiled into production builds despite being test-only scaffolding — `dead-code`
- **[CONFIRMED]** ApexDataProvider::unsubscribe_* removes the entire route key, dropping every other subscriber's sender for that symbol — `design-system`
- **[WEAKENED]** ORDERS_SNAPSHOT publish-after-unlock has no ordering guard — the order ledger and on-chart order lines can go permanently stale — `correctness`
- **[WEAKENED]** CountingAlloc is installed as the global allocator in release builds — 4 contended atomic RMWs on adjacent statics for every heap allocation — `perf`
- **[CONFIRMED]** Per-pane and per-window UI state parked in single-slot process globals — the options-chain seat set is cleared by whichever surface renders last — `coupling`
- **[CONFIRMED]** Script panel error output is never styled as an error — the error-detection prefix does not match the string the code produces — `inconsistency`
- **[CONFIRMED]** Seasonality month attribution drifts across leap-year boundaries, misfiling early-January bars as December — `correctness`
- **[WEAKENED]** Greeks poller swallows the standing 404 with an empty Err arm — the feature is permanently dead and nothing reports degraded capability — `fail-silent`
- **[CONFIRMED]** RSI returns 99.01 instead of 100 when there are no losses in the window — `correctness`
- **[CONFIRMED]** Simulated option chain prices time in trading days while discounting at an annual rate; bs_delta is dead — `correctness`
- **[WEAKENED]** `SubscriptionManager::check_stale()` — the documented silent-stale-feed alarm — is never called — `unwired`
- **[WEAKENED]** FMV is ingested on every frame into a map with no reader — `get_fmv()` is never called — `fail-silent`
- **[WEAKENED]** The `InFlightRegistry` migration stalled: entries are created and never expired, and no consumer reads it — `unwired`
- **[WEAKENED]** Fabricated market-data generators sit unreferenced in gpu.rs — a 100-LOC landmine in a live-money binary — `dead-code`
- **[WEAKENED]** `ChainRow::display_price()` remains test-only — production still renders the raw field it was written to replace — `fail-silent`
- **[WEAKENED]** Halt tracking maintains two capped rings that no code reads; the comment claiming otherwise is false — `dead-code`
- **[CONFIRMED]** `chart/renderer/compute.rs` holds a second, dead copy of the drawing-tool math that `core.rs` implements inline — `redundant`
- **[CONFIRMED]** `design_system::registry` (266 LOC) is a competing theme source-of-truth with zero references outside its own module — `architecture`
- **[CONFIRMED]** Dead REST client surface in the ApexData feed: async auth-retry wrappers and four sync getters with no callers — `dead-code`
- **[WEAKENED]** The command palette's top-billed "Ask Gemma" hero entry and Dynamic-UI action are acknowledged placeholders occupying prime real estate — `ux`
- **[CONFIRMED]** 21 AppCommand variants have reducers but zero emitters; the alert-snooze feature exists only as a reducer — `dead-code`
- **[CONFIRMED]** The `apex-playground` component gallery renders ui_kit widgets with default design tokens, never the ones the app ships — `design-system`
- **[CONFIRMED]** `foundation/design_inspector.rs` is 181 KB of chart UI living in the base layer, and is the sole source of the foundation→chart back-edge — `architecture`
- **[WEAKENED]** Two parallel persistence schemes coexist: a versioned `Persistable` envelope used by ~6 aggregates, and ~15 hand-rolled JSON files with no version field — `redundant`
- **[CONFIRMED]** Five ColorScheme fields (hud_bg, hud_border, text_muted, notification_red, pinned_row_tint) are hand-authored in 21 palettes and read by nothing — `dead-code`
- **[CONFIRMED]** Two divergent definitions of the Meridien style exist; the one the ThemeRegistry defaults to is not the one the app renders — `inconsistency`
- **[WEAKENED]** 2,105 LOC of subpixel-text machinery (incl. a 986-LOC wgpu pipeline) serves exactly one production call site — `dead-code`
- **[CONFIRMED]** Blanket #[allow(dead_code)] on 6 of 12 ui_kit modules disables the only automatic shelfware detector over 29,000 lines — `architecture`
- **[CONFIRMED]** All five free-function helpers in ui_kit/widgets/mod.rs are dead, and they hand-roll raw egui inside the design system — `dead-code`
- **[WEAKENED]** Thirteen exported types in the kit's public surface have no consumer anywhere outside their defining file — `dead-code`
- **[CONFIRMED]** A full Taffy flexbox engine is a hard dependency for 5 of 217 chart UI files — `architecture`
- **[CONFIRMED]** Nine kit widgets are single-consumer domain code parked in the design system — `architecture`
- **[WEAKENED]** Greeks arrive from two independent sources with no reconciliation: the chain cache and a serial per-contract HTTP poller — `redundant`
- **[CONFIRMED]** Verbatim-duplicated blocks across the chain stack: two JSON row parsers, four to_rows closures, two whole option quick pickers — `redundant`
- **[UNVERIFIED]** The Prometheus metrics server listens on 0.0.0.0:9091 in release builds with no authentication and `Access-Control-Allow-Origin: *` — `correctness`
- **[UNVERIFIED]** The library declares `staticlib` and `cdylib` crate-types with no FFI consumer anywhere in the repo — `redundant`

---

# Cross-Cutting Synthesis — apex-terminal

## 1. Root causes

Seven root causes account for essentially all ~100 surviving findings. They are ranked by how much of the finding set each explains.

---

### RC-1 — Migrations that ship the new half and never delete the old half
**~30 findings, 8 dimensions. The dominant structural fact of this codebase.**

Every one of these is "two implementations, both live, no owner":

| Concern | Old half | New half |
|---|---|---|
| Indicator math | `ui/overlays/indicators.rs:27,43` (SMA-smoothed) + `pane/core.rs:6667` | `compute.rs:264,120` (Wilder) — both painted on one screen |
| VWAP | `gpu.rs:4676-4706` (no calendar reset) | `compute.rs:324` (day-boundary reset) |
| Chain fetch | `refresh_chain_rest`, `fetch_overlay_chain_background` | `fetch_chain_background` — 3 entry points, 3 writers to `watchlist.chain` |
| Type scale | `ui_kit/style.rs:389-398 FONT_*` (8/9/11/13), `DesignTokens::default()`, `Typography::default()` | `frame_tokens()` (9/10/12/14) |
| StyleSystem→tokens | ad-hoc remap at `ui/style.rs:99-107` | `style_system_to_style_settings` (`ui/style.rs:2491`) |
| Theme state | `LIVE_THEMES` + `STYLE_STORE` + 6 more | `ThemeRegistry`/`DesignSnapshot` (979 LOC, zero callers) |
| Persistence | 15 hand-rolled `*_path()`+save/load in `gpu.rs:9029-9520` | `state/persistence.rs` `Persistable` envelope (6 aggregates) |
| Broker URL | `trading/mod.rs:22` const | `gpu.rs:1772 apexib_url()` + orphan `trading/config.rs` (never declared as a module) |
| HTTP client | 16 ad-hoc `Client::builder` incl. `order_manager.rs:3703` | `foundation/http.rs:19 blocking_client()` (2 adopters) |
| Widgets | `ui/inputs/select.rs:215 SegmentedControl`, raw `.context_menu()` ×17 | `ui_kit::SegmentedControl`, `ui_kit::ContextMenu` (0 callers) |
| Order UI | `orders_panel.rs` (local-only) | `order_ledger_panel.rs:285` (`order_manager::cancel_order`) |
| Chart storage | drawing_db | `chart/state/` (2,583 LOC, integration point hardcoded `None`) |

This is not cosmetic debt. It directly produces the P0 at `fetch.rs:546-561` (30/60-DTE silently rendering a ≤14-DTE chain) and the P1 at `chart_widgets.rs:28` (a glob import silently selecting the non-Wilder RSI for the on-chart readout while the indicator pane uses the Wilder one).

---

### RC-2 — The outcome is produced and then dropped at the boundary
**~18 findings. Concentrated on the live-money path.**

Uniform signature: something returns a `Result`/`OrderResult`/`Response` and the caller discards it with `let _ =`, `.map(|_| ())`, an empty match arm, an `eprintln!`, or an empty `if` body.

- `broker.rs:620,665,734` — bracket/OCO/options-trigger `.send()` then `.json()` with no `error_for_status`. **I verified: `error_for_status` (defined `:335`) has exactly three call sites — `:400`, `:420`, `:445`.** Three of six broker POSTs are hardened; three are not.
- `broker.rs:571-591` — kill/halt/resume are `.map(|_| ())`; callers are `let _ = broker.kill()` at `order_manager.rs:1501/1517/1527`; `report("ENGAGED (local + broker)")` fires *before* the POST.
- `order_manager.rs:1953` — `if let Ok(ib_oid) = broker.submit(...)` in `confirm()`; no else.
- `fetch.rs:1569-1600` — `let _ = submit_order(intent)` on the armed path; `NeedsApproval` vanishes.
- `core.rs:1721-1723` and 5 more sites — `Rejected(reason) => eprintln!(...)` in a `windows_subsystem` app; `Duplicate => {}`.
- `screener_build.rs:998-1013` — `let _ = ();` behind "Run Now" and "Save Screen".
- `news_panel.rs:251-253` — `if row.clicked() && !news.url.is_empty() { // TODO }`.
- `live_state.rs:286` — `Err(Http{status:404,..}) => {}`.
- `hot_reload.rs:158` logs "reloaded StyleSystem" then applies 2 of 8 sub-structs.
- `ws.rs:502` logs "replayed {n} bars after reconnect" for work that filled no gap.

The last three are the nastier variant: **the discard is paired with a success message.**

---

### RC-3 — Cache/identity keys that are a strict prefix of the real identity
**7 findings, including 2 P0s. The single most mechanically checkable class.**

- `live_state::get_chain(symbol)` — no expiry dimension → 30D label over ≤14-DTE contracts (`fetch.rs:546`, and again at `core.rs:10266`).
- `bar_cache::key = "apex:bars:{sym}:{tf}"` (`bar_cache.rs:63`) — no range → `gap_fill_on_reconnect` replays the full stale series into `AppendBar`, which has no monotonicity guard (`gpu.rs:3078-3105`).
- chain cache invalidated on server `resync` (`lib.rs:202`) but not on client reconnect — missing the "connect epoch" dimension.
- `find_local_match` Rule 1b (`order_manager.rs:3850`) — (symbol, side, qty), no price, no time → fill attributed to the wrong ladder rung.
- OCO `leg_backend_ids` zipped **positionally** (`order_manager.rs:2302`) after `broker.rs:642` silently `continue`s a leg.
- widget cache keyed on `bar_count` alone (`chart_widgets.rs:74`) → RSI/ATR/last-price frozen for the whole building bar.
- `COLOR_*` categorical palette has no `is_dark` dimension (`ui/style.rs:827-849`) → stop/target/R:R lines at 1.1–3.0:1 on 5 light schemes.

---

### RC-4 — Process-global singletons under a multi-instance runtime
**~10 findings.** The app grew multi-window (`top_nav.rs:1254` → `gpu::open_window`) and nine-pane layouts (`gpu.rs:9609-9616`) on top of state written for one of each.

- `persist_supervisor.rs:19` SHUTDOWN is a process-global `OnceLock`; `gpu.rs:8192` calls it on *any* window close, before the `windows.is_empty()` guard at `:8204`.
- Six `Store`s per window, all pointing at the same six paths (`gpu.rs:6324-6353`), `StoreRegistry` has no `unregister`, `flush_all` ignores `needs_persist()`.
- `pane/core.rs:97 VISIBLE_CHAIN_CONTRACTS` — one set, cleared per watchlist-panel render.
- `dom_feed.rs:20 ACTIVE_SYMBOL` driven per-frame from per-pane state (`core.rs:1525`).
- `ui_kit/style.rs:168 FRAME_TOKENS_LOCAL` written only from inside `draw_chart` → the playground never gets real tokens.
- 193 module-level statics vs. 6 governed aggregates.
- The ABBA inversion (`apex_data.rs:66` ROUTES→bars vs `subscription_manager.rs:200` bars→ROUTES) is the same class: one global lock reached from two directions.

---

### RC-5 — `#[allow(dead_code)]` as ambient policy
**~15 findings, and it is the *mechanism* behind most of RC-1.**

Six module-wide allows in `ui_kit/mod.rs:5,7,17,22,30,32` cover 26.5k LOC. `#![allow(dead_code)]` covers all 2,583 LOC of `chart/state/`. `pub use` re-exports suppress warnings for `providers/mock.rs`. The compiler is structurally unable to report shelfware, so shelfware only surfaces via an audit like this one — and the shelf contains safety code: `release_kill_switch()` (`order_manager.rs:3725`), `SubscriptionManager::check_stale()` (`:677`), the WAL orphan reporter, `ChainRow::display_price()`.

---

### RC-6 — No composition root; lifecycle wired as a side effect
**~6 findings.** Startup is scattered across `native_main.rs`, `GpuCtx::new`, `Watchlist::new`, and lazy statics.

- Trading recovery (WAL replay, kill-switch restore, orphan reconciliation, broker watchdog) is lazily triggered by `manager()`, whose only startup path is `start_account_poller()` at `gpu.rs:7421` — **inside `GpuCtx::new`, after `request_adapter`**. A DX12 failure returns at `gpu.rs:7922` with no window *and* no trading recovery.
- A blocking 5s Postgres `recv_timeout` inside `WindowEvent::RedrawRequested` (`gpu.rs:8257`).
- `init_live_feeds` is a 170-line feed→UI adapter living at the crate root with business logic in it (`lib.rs:99-270`).
- `foundation/design_inspector.rs` (181 KB of chart UI) is the sole source of the foundation→chart back-edge.

---

### RC-7 — Harnesses that verify a model of the app, not the app
**~8 findings. This is why RC-1 through RC-6 survived.**

- `dev_inspector/mod.rs:436-522` asserts against `HeadlessState`, a hand-written shadow state machine; `:843-853` hardcodes `fps = 60.0` and clears `active_violations` — perf and design assertions **cannot fail**.
- 89 of 122 `AppCommand` variants are producible only from `dev_inspector`, which is `#[cfg(debug_assertions)]` — scenario tests validate paths no user can reach.
- `apex-playground` runs under eframe with default tokens, no fonts, no icon font.
- Tested-but-unwired: `news_panel::filtered()` (5 green tests, never called in production), `display_price()`, `check_stale()`, snapshot goldens anchored to `StyleSystem::meridien()` — a Meridien the app never renders.
- `ui/style.rs:3783` — a test comment that claims to exercise `begin_frame`'s resolver and does not.

---

## 2. The single highest-leverage fix

There are two answers and they are different; do the risk one first.

**Highest risk-per-line — do this: make the outcome un-discardable on the money path.** Two rules, ~15 edit sites:

1. Every `.send()` in `broker.rs` goes through `Self::error_for_status(resp, op)`. Currently 3 of 6 do (verified: `:400`, `:420`, `:445`).
2. Every function that produces an `OrderResult` terminates in `record_submit_outcome` (`order_manager.rs:3370`), and `record_submit_outcome` upgrades `Rejected`/`Duplicate`/`NeedsApproval` from `count()` to `report()` + toast — the pipeline already exists (`errors_sink.rs:89`, and `order_manager.rs:4141` already toasts broker-side rejects).

This alone closes or materially reduces: trading#1 (P0), trading#4 (P0), ux#2 (P0), trading#8, trading#9, trading#13, fail-silent#1, fail-silent#2, fail-silent#3, dead-code#1. Three P0s and six P1s, from two invariants and no architectural change.

**Highest finding-count — schedule for Wave 2: route every token consumer through `style_system_to_style_settings` (`ui/style.rs:2491`).** It is already written, already documented as total, and is the correct half of four separate migrations. Making it the *only* StyleSystem→tokens mapping closes ~14 findings across `design-system`, `redundancy`, `ui-kit` and `architecture`. Zero money risk, so it does not go first.

---

## 3. How it got here, and the process change

**This is not organic growth and it is not copy-paste sloppiness.** The evidence says the opposite: this team finds real defects, fixes them precisely, and documents the fix well — and then never sweeps the sibling call sites. The pattern repeats verbatim:

- **W0-02** added `error_for_status` → applied to 3 of 6 broker POSTs.
- **A2** fixed Working-before-Ack in `submit()` with an explicit comment (`order_manager.rs:1799`) → `confirm()`, 100 lines away, still does it (`:1916-1919`).
- **Wave 5** fixed a hardcoded `"5m"` at `ib_ws/mod.rs:459-462` *with a comment naming the bug* → the identical bug 65 lines earlier at `:394` is untouched.
- **foundation/http.rs** was written specifically to stop per-call `Client::new()` on the broker path → the pre-trade margin check still does `Client::new()` per call (`order_manager.rs:3703`).
- **TYPE SCALE LIFT** raised the tier tokens → the semantic ladder, `FONT_*`, `DesignTokens::default()` and `Typography::default()` were all left behind, three of them still carrying comments asserting they are aligned.
- **CC1** (publish-after-unlock) fixed a lock-hold and introduced an unordered-publish race.

The tell is the comments. This repo is unusually well-annotated, and a large fraction of the annotations are now false: *"Values match the active scale"* (`ui/style.rs:259`), *"aligned to TokenSnapshot DEFAULT"* (`style_system.rs:170`), *"so it reads correctly on any colour scheme"* (`ui/style.rs:1702`), *"Cache in recent_halts for the heat/scanner panels"* (`lib.rs:165`), *"Run through the per-frame resolver exactly as begin_frame does"* (`ui/style.rs:3783`), *"not used by any existing hotkey block"* (`top_nav.rs:1817`). **A comment asserting an invariant is the highest-density defect signal in this codebase.**

There are 42 TODOs and exactly one enforced architectural invariant: `ui_kit/layer_guard.rs`. And it is the one boundary that held — pinned at 4, with both new-violation detection (`:142-155`) *and* stale-ratchet detection (`:174`) so progress must be recorded.

**Three process changes:**

1. **Fix the class, not the site.** Every defect PR states the grep that characterises the class and the count `fixed / found`. If sites remain, the PR adds a ratchet entry — never a TODO.
2. **An invariant comment is a test or it is deleted.** Any comment of the form "matches / same as / aligned to / equivalent" becomes an assertion. ~12 confirmed findings are literally a false invariant comment.
3. **Generalise `layer_guard.rs` into an `invariants/` module.** The mechanism exists, is proven, and is in-tree. Seed it with:

| Ratchet | Start | Catches |
|---|---|---|
| R1 `#[allow(dead_code)]` count/module | 68 | RC-5 |
| R2 module-level interior-mutable statics/file | 193 | RC-4 |
| R3 `data/`→`chart_renderer::` refs; `foundation/`→`chart::` | 26 / 17 | RC-6 |
| R4 `let _ =` / `.map(\|_\| ())` on any Result-returning fn in `trading/` | → 0 | RC-2 |
| R5 every `.send()` in `broker.rs` followed by `error_for_status` | → 100% | RC-2 |
| **R6 pub fns whose only callers are inside `#[cfg(test)]`** | → 0 | **8 findings alone** — `release_kill_switch`, `check_stale`, `display_price`, `filtered()`, 13 ui_kit types |
| R7 token equality: `FONT_SM == font_sm()`, `DesignTokens::default().font.sm == DEFAULT_TOKEN_SNAPSHOT.font_sm`, adapter totality over every `StyleSystem` field | → pass | RC-1 frozen chrome |
| R8 contrast ≥3:1 for every categorical colour × every builtin `ColorScheme` bg | → pass | design-system#1 |
| R9 no duplicate `fn compute_rsi` / `fn compute_atr` / `struct SegmentedControl` in tree | → 1 each | RC-1 |

R6 is the standout: one mechanical check, eight findings.

---

## 4. Dependency-ordered remediation plan

### Wave 0 — Money-path integrity (no dependencies; days)
Everything here is local and independently shippable.

1. `broker.rs:620,665,734` → `error_for_status`; treat an all-`None` id response as `Err`. *(trading#1, P0)*
2. `submit_bracket` (`order_manager.rs:1978`) → add `validate_risk` + `OrderSignature` dedup + `max_open_orders`. *(trading#2, P0)*
3. `validate_risk` → seed `net_position` from the broker `positions` already bound as `_positions` at `:1183`; local `Filled` rows become an in-session delta. *(trading#3, P0)*
4. `confirm()` (`:1914-1962`) → mirror `submit()`: stay `PendingSubmit`, journal `Attempt`/`Ack`/`Fail`, `Err`→`Rejected`. *(trading#4, P0)*
5. `record_submit_outcome` → `report()` + toast for Rejected/Duplicate/NeedsApproval; delete the 6 `eprintln` arms; route `flatten`/`flatten_all`/`flatten_symbol` through `submit_order`. *(ux#2 P0, fail-silent#3)*
6. kill/halt/resume → `error_for_status`, move `report(...)` into the spawned thread, Critical toast on broker-leg failure. *(trading#8, fail-silent#1)*
7. Wire `release_kill_switch()` to Ctrl+Shift+R; gate the "Trading RESUMED" toast on `!is_trading_blocked()`; resolve the Ctrl+Shift+R double-fire (`top_nav.rs:1822` vs `gpu.rs:7213`). *(dead-code#1, unwired#3)*
8. Orders panel Place/Cancel → `order_manager::confirm_order`/`cancel_order`, mirroring `order_ledger_panel.rs:285/487`. *(ux#1, P0)*
9. Persist `paper_mode` in `control_flags.json`, fail closed to live; restrict `apply_paper_fills` to `paper:`-prefixed backend ids; skip Rule 5 adoption while paper. *(trading#5, trading#6)*
10. Add `reduce_only` to `OrderIntent`; exempt it from halt/daily-loss/buying-power. *(trading#10)*

**Unblocks:** every later wave, by guaranteeing a refactor cannot land on top of a broken money path.

### Wave 1 — Numbers on screen are true (parallel with Wave 0)
1. Delete the incremental branch at `gpu.rs:3752-3790`; always recompute when `n != indicator_bar_count`. *(engines#1, P0)*
2. `ws.rs:560` → `Some(Ok(_)) => LAST_MESSAGE_AT_MS.store(...)`. **One line; do it first in this wave** — it stops the 30s reconnect storm that multiplies #3. *(data-layer#4)*
3. Ranged gap-fill: `rest::get_replay`, honour `start_ms/end_ms/limit` in `ApexDataProvider::bars`, `CachedProvider` bypass when `start_ms != 0`, monotonicity guard in `AppendBar`. *(data-layer#1, P0)*
4. ABBA: never hold a `SubscriptionManager` map guard across a `provider.*` call; snapshot senders out of `ROUTES` before `bump_last_seen_*`. Document one global data-layer lock order. *(data-layer#2, P0)*
5. Chain expiry: key the short-circuit on expiry coverage at **both** `fetch.rs:546` and `core.rs:10266`; return the chosen expiry and label the header with it. *(redundancy#1, P0)*
6. Gate the cross-timeframe indicator source picker off until resample-to-chart-space exists — note the OOB panic when source bars outnumber chart bars. *(engines#2, P0)*
7. Derive categorical colour lightness from `cs.meta.is_dark` (mechanism already at `theme_studio.rs:810`); add ratchet R8. *(design-system#1)*

**Unblocks:** any perf or refactor work on the chart — you now know the pixels are right.

### Wave 2 — One writer per concern (depends on 0+1)
1. **Indicators:** `overlays/indicators.rs::compute_rsi/compute_atr` become wrappers over `compute.rs`; same for `core.rs:6667` cone ATR and the second VWAP at `gpu.rs:4676`. Relabel the MTF RSI rows honestly or fetch real bars. Ship with a note that widget values will shift to match the pane — that is the point. *(engines#3–7, redundancy#3)*
2. **Chain:** one owner. Delete the per-frame derive at `core.rs:10272` (also kills the 60 Hz full-chain clone) or make it own the placeholder flags. Fold 4 `to_rows` and 2 `parse_rows` copies into one helper each. Overlay reads `live_state::get_chain` and `seed_chain`s its REST result. *(redundancy#2,#4,#9,#10)*
3. **Style:** route the hot-reload override through `style_system_to_style_settings`; source `TokenSnapshot` font/gap/alpha/shadow from `current()` with `dt_f32!` as override-only; delete `FONT_*`; derive `DesignTokens::default()`/`Typography::default()` from `DEFAULT_TOKEN_SNAPSHOT`; delete `StyleSystem::meridien()`; delete or `cfg(test)` `ThemeRegistry`/`DesignSnapshot`. Add ratchet R7. *(design-system#2–8, redundancy#5,#7, ui-kit#1)*
4. **HTTP:** one `apexib_url()` and one `apex_signals_http()` in `data/endpoints.rs`; delete `trading/mod.rs:22` and the orphan `trading/config.rs`; route the 16 ad-hoc clients + `order_manager.rs:3703` through `foundation::http::blocking_client()`; log the resolved broker base URL once at startup; make the `apexib_curl` fallback logged rather than a silent degrade to synthesized bids. Add ratchet R5. *(architecture#5,#9, redundancy#2,#8, trading#15, unwired#5)*

### Wave 3 — Ownership (depends on 2 for the token/HTTP singletons)
1. Per-supervisor shutdown flag, gated on last window; `StoreRegistry::unregister`; `flush_all` respects `needs_persist()`; unique `.tmp` names in `atomic_write`. *(state#1,#3,#4)*
2. Generation counter on `ORDERS_SNAPSHOT` publish. *(state#2)*
3. `VISIBLE_CHAIN_CONTRACTS` keyed by window; DOM feed multi-symbol (or drive `set_symbol` from open/close transitions, not per-frame). *(state#5,#7)*
4. Panel-state globals (`replay_pane`, `screener_panel`, `screener_race`, `provenance_pane`) onto their owning structs. Add ratchet R2.

### Wave 4 — Boundary extraction (last; now low-risk)
1. Extract the order-action blocks (`core.rs:1697-1815`, `:7017`, `:9327`) into `trading/pane_actions.rs`. **This is a boundary extraction, not a paint refactor** — the SACRED charter does not cover it.
2. Explicit composition root: `order_manager::init()` + `start_account_poller()` in `native_main.rs` before `open_window`. *(architecture#1, P1)*
3. Non-blocking `ReloadWatchlistsFromDb`. *(architecture#3)*
4. Move `design_inspector` into `chart/`; move `init_live_feeds` into `data/adapters/`. Add ratchet R3 at 0 / 26. *(architecture#6,#10)*
5. Split `gpu.rs` — biggest, lowest risk-adjusted value, do it last or not at all. *(architecture#4)*

### Wave 5 — Make the compiler the reviewer (start in parallel, land after 4)
1. Remove the 6 module-wide allows in `ui_kit/mod.rs` and `#![allow(dead_code)]` on `chart/state/`; delete confirmed-dead; demote the 13 types to `pub(crate)`. Add ratchet R1.
2. Fix the layer_guard glob hole: replace `ui_kit/tokens.rs:28`'s `pub use ...::*` with an explicit re-export list; move `discord_blurple` into `ui_kit`.
3. Delete the `dev_inspector` shadow state machine; make headless a rendering mode so `begin_frame`/`end_frame` capture real `Chart`/`Watchlist`; emit `null` for fps/violations instead of literals. Split `AppCommand` / `DevCommand`.
4. Playground: `init_icons(&cc.egui_ctx)` + `set_frame_tokens` + a style picker.
5. Add ratchets R4, R6, R9.

---

## 5. HEALTHY — do not touch

This list matters as much as the fix list; a cleanup sweep is the most likely way these get damaged.

1. **`chart/renderer/compute.rs` pure math.** Bollinger uses sample stdev N-1 (`:155`) with a dedicated test; RSI/ATR/ADX use true Wilder smoothing; NaN-aware seeding is deliberate and tested. Black-Scholes `bs_price`/`normal_cdf` are textbook-correct. **Consolidate onto it; do not rewrite it.**
2. **`chart/indicators/mod.rs` W3-01 registry.** Genuinely load-bearing — `recompute_indicators` routes every kind through `spec().compute()`. This is the model the other registries should imitate.
3. **`ui_kit/layer_guard.rs`.** The only enforced architectural invariant in the repo, with both new-violation *and* stale-ratchet detection. Fix its glob hole; never weaken or delete it.
4. **The single-order submit path** (`order_manager.rs:1620-1844`). No optimistic Working-before-Ack, HTTP status checked, kill re-check inside the spawned thread, WAL Attempt/Ack/Fail, `PendingCancel` instead of optimistic `Cancelled`, atomic state writes, and no UI site bypasses OrderManager with raw HTTP. **Wave 0 is literally "make the siblings look like this."**
5. **`foundation/http.rs`, `data/endpoints.rs`, `errors_sink`, `foundation::guard::spawn_guarded`, the panic-containment harness, `state/` (Store/PersistableStore/persist_supervisor/Persistable).** All correct designs; the fault is scope and adoption. **Adopt, don't redesign.**
6. **The prior lying-buttons sweep.** DOM ladder gating click-to-trade on a live feed + SIMULATED badge; MSG panels' SAMPLE DATA badges; backtest SIMULATED badge; the PLACEHOLDER DATA strip at `watchlist_panel.rs:1524-1544`; theo freshness markers at `core.rs:3273-3295`; the KILL SWITCH ENGAGED banner at `top_nav.rs:316-352`; command-palette / workspace-rail / LIVE-toggle confirm gates. **Several dimension audits initially mis-read these as fail-silent and the verification pass refuted them** — that is exactly how a cleanup would remove them by accident.
7. **The ui_kit widget contract itself** — uniform `show(self, ui, theme: &dyn ComponentTheme) -> Response` across ~60 widgets, `sx::Tone` in 64 files. The problem is adoption, not design.
8. **The trading module's documented CC1/CC2 lock ordering**, the WAL journal + `replay_and_recover`, and the `state/` test suite. Correct and tested; the fault is scope, not quality.
9. **Font-size call-site discipline** — 39 hardcoded literals across 196k lines. The colour axis has no discipline; typography call sites do. Don't spend effort here.
10. **`render_chart_pane`'s paint hot loop.** The SACRED designation is legitimate *for paint*. Wave 4 removes the order-action blocks and nothing else; do not let it become a paint refactor.

---

**One meta-observation worth acting on.** A large share of findings were downgraded in verification because the auditor asserted "no signal reaches the user" when a badge, banner or toast already existed — but far from the defect site (`top_nav.rs:316` for the kill switch, `ws.rs:632-663` for feed staleness, `watchlist_panel.rs:1527` for placeholder chains). The honest-degradation surfaces are real and **undiscoverable by reading the code that degrades**. That argues for a single `foundation::capability` registry — one place where "greeks unavailable", "gamma stale 47m", "chain placeholder", "broker kill failed" are declared and one place they are rendered — rather than continuing to add per-site badges nobody can find.

---

# Refuted findings (dropped)

Recorded so they are not re-raised. The adversarial verifier opened the cited code and found these did not hold.

- ~~with_mgr's re-entrancy guard is not panic-safe — one panicking closure permanently disables it for that thread~~
  - Why refuted: The code observation is literally true — order_manager.rs:157 sets IN_WITH_MGR true, 162 runs f, 173 clears it, with no RAII guard, so an unwind past 173 leaves the thread-local true. But the failure scenario and the evidence offered for it do not hold. The finding argues 'the order-manager threads are all launched via spawn_guarded, which implies panic catching' — I read guard.rs:49-66 and spawn_guarded's catch_unwind wraps the ENTIRE worker closure (`panic::catch_unwind(AssertUnwindSafe(f))` at line 56), so a panic inside a with_mgr closure unwinds the worker's whole loop, reports 'worker died from a panic (contained)', and the THREAD EXITS. It never calls with_mgr again, so the stuck flag is unobservable. I grepped the full tree for catch_unwind: guard.rs:56 is the only occurrence (the other two hits are doc comments at guard.rs:7 and 46). There is no catch_unwind anywhere on the render/winit thread either, so a panic there unwinds out of the event loop rather than returning to steady-state frame rendering. Consequently there is no live path on which a thread survives a panic inside f and re-enters with_mgr, and the claimed debug cascade / silent loss of the release-mode deadlock detector cannot occur. Remains a defensible hygiene fix (the RAII guard is two lines) but it is inert today, not a P3 fail-silent defect.
- ~~Locally rejected order submits produce no UI feedback at all — six call sites eprintln to a stderr no windowed user can see; Duplicate is silently swallowed~~
  - Why refuted: The central claim is false. errors_sink::report (data/connectivity/errors_sink.rs:89-136) does far more than feed apex_diagnostics: after ring-buffering and tracing, lines 122-135 map ErrorLevel::Warn to severity 1 and call `live_state::push_toast_with_severity(format!("{source}: {message}"), sev)` (live_state.rs:735-745). gpu.rs:5343-5349 drains that queue every frame via `live_state::drain_toasts()` and pushes each into the notification pipeline with decoded severity. record_submit_outcome (order_manager.rs:3370-3382) routes OrderResult::Rejected through exactly that `report(Warn, "broker", "order_rejected", reason)` — so kill switch, halt, 'submit rate limit exceeded', the $0-limit guard and InsufficientBuyingPower ALL raise a visible yellow warning toast today. And record_submit_outcome is invoked by every global entry point the six cited sites use: submit_order (order_manager.rs:2675-2680), submit_bracket_order (:3628-3633), submit_oco_order (:3636-3641), submit_conditional_order (:3644-3648). The auditor quoted the doc comment at :3364-3366 ('A rejection is surfaced through the visible errors_sink') and then asserted the opposite of what it says. The eprintlns are redundant, not the sole channel. Two narrow sub-claims survive: OrderResult::Duplicate uses `count(...)` (errors_sink.rs:82-88, counter-only, no toast) so it is genuinely silent, and the bracket NeedsApproval arm at core.rs:1779-1781 only counts + eprintlns while the plain-order path at core.rs:1724-1729 and gpu.rs:1941-1946 correctly calls enqueue_approval. That residue is a P3 gap, not a P0 'the UI does absolutely nothing'.
- ~~WAL orphan detection and manual WAL snapshot exist but are never invoked, so unknown-state orders are never surfaced~~
  - Why refuted: The headline claim — "unknown-state orders are never surfaced", "the recovery tooling was written and then never connected" — is FALSE. The live orphan detector is `journal::find_orphan_attempts()` (journal/mod.rs:70), called from `replay_and_recover()` (order_manager.rs:4344), which is spawned at startup via `spawn_guarded` at order_manager.rs:77 (gated on `last_event_was_shutdown()` at :72). It queries the broker per orphaned client_order_id (`query_broker_by_client_id`) and reconciles into Working/Filled/Cancelled with Reconcile+Ack journal events (4356-4430), reporting "orphan_query_start" at ErrorLevel::Info. broker.rs:237-246 documents the `order_by_client_id` trait method as existing specifically for this path. `report_orphans_to_stderr()` (journal/mod.rs:159) is a redundant log-only duplicate of that scan, correctly annotated `#[allow(dead_code)]`; `snapshot_wal_now()` (:116) is a sync wrapper over `snapshot_wal()`, which the live hourly backup thread (:98, called from order_manager.rs:81) already runs. Both are genuinely unreferenced, but the safety property is delivered by a different, wired path — this is a P3 duplicate-helper cleanup, not a recovery gap.
- ~~The entire RecipeSet / RecipeSpec theme-override layer (1,272 LOC) is inert — theme packs are parsed, hot-reloaded and never read~~
  - Why refuted: The load-bearing claim — '`get_ambient_recipes()` has ZERO call sites' — is false. `grep -rn get_ambient_recipes` returns FOUR live widget call sites outside ctx.rs, and each one feeds a real paint call: tag.rs:143 gets the ambient set, :151 does `recipes.resolve("tag", default_chip_sx, theme)` and the resolved fill/border are painted at :160+ (`painter.rect_filled(rect, chip_cr, fill_color)`); tabs.rs:272 resolves `tab.line.active` (:278) and `tab.pill` (:295) into the underline colour and pill radius; panel_section.rs:448 resolves `section.header` (:451) into the header fill; panel_list_row.rs:565 resolves `row.list` (:566) and `row.list.selected` (:580) into the row corner radius and selected-bg colour. So a theme pack shipping recipes for the keys `tag`, `tab.line.active`, `tab.pill`, `section.header`, `row.list`, `row.list.selected` DOES change pixels — 'not one pixel changes' is wrong. The narrow sub-claims are true (StyleCtx::from_ctx has no non-doc callers; `.recipes()` returns 0 hits; button.rs/tabs.rs show_ctx use from_theme with empty_recipe_arc), but that describes an unfinished second adoption path, not an inert layer. The auditor's grep was evidently scoped to the StyleCtx plumbing and missed the direct-resolve adoption pattern the widgets actually use. Recommended fix ('delete recipes.rs + recipe_spec.rs') would break 4 shipping widgets.
- ~~Fourteen competing header abstractions; the canonical ui_kit::Header has 2 call sites~~
  - Why refuted: Both load-bearing claims are false. (1) 'ui_kit::Header has exactly two production call sites' — Header is also the render path for every dialog-style Modal: dialog_header.rs:67 calls `Header::dialog`, modal.rs:340 calls `DialogHeader::new(title)`, and there are 12 `Modal::new` sites in chart/. layer_guard.rs:211-217 actively asserts that Modal must route through ui_kit's DialogHeader and not the legacy `DialogHeaderWithClose`, i.e. the codebase already has a tested consolidation on Header. So Header is not a 2-call-site orphan. (2) The fix rests on 'panels/kit.rs::PanelHeader (47 uses) is the de-facto standard'. That number is a raw substring count — `grep -rno "PanelHeader"` in chart/ returns 75, but that string is a prefix of PanelHeaderWithClose (10) and PanelHeaderTabs. Counting actual constructor calls: `PanelHeader::new` = 4 and `PanelHeaderTabs::new` = 5. Picking PanelHeader as the winner-by-usage is therefore unsupported, and acting on the recommendation would promote a 4-call-site type over the one the layer guard already blessed. The remaining counts do check out (PainterPaneHeader 14, PaneHeaderBar 8, PaneHeaderActions 5, pane_header_bar 12, panel_header 4, PanelHeaderWithClose 10, DialogHeaderWithClose 4, PaneHeaderWithClose 4, PaneHeaderAction 6), so header-type proliferation is real — but as a P3 tidiness item, not a P1 with this evidence.
- ~~38% of Button call chains override raw colors, defeating the token/theme system Button exists to enforce~~
  - Why refuted: The impact claim — 'A theme change cannot reach 112 call sites; those buttons stay whatever color someone typed... same class as the frozen-chrome defect' — is refuted by reading the arguments rather than counting the method names. Sorting all 80 `.fg(` arguments in chart/ shows they are overwhelmingly THEME-DERIVED: `t.dim`, `t.accent`, `t.text`, `t.bear`, or locals computed from the theme. Exactly 3 are hard literals (`egui::Color32::WHITE`). A theme change propagates through `t.*` by construction, so these are not pinned values and this is not the frozen-chrome pattern. Two smaller errors: `Button::cta` and `Button::trade` have ZERO call sites in chart/ (the auditor says 1 each — those hits are in the playground/gallery), and toolbar/simple have 1 each, not 2. What remains true is only that Button exposes many escape hatches and 14 constructors, several unused — an API-surface tidiness item, P3.
