# Audit Ledger — apex-terminal

Single source of truth for the 2026-08-02 deep-dive remediation. **Update this file in the same commit that fixes the item.**

Full evidence: [`CODEBASE_DEEP_DIVE_2026-08-02.md`](CODEBASE_DEEP_DIVE_2026-08-02.md)  
Backend-owned items: [`BACKEND_FIX_LIST_2026-08-02.md`](BACKEND_FIX_LIST_2026-08-02.md)

## Working rules

1. **One commit per `AT-###`.** Put the id in the subject: `fix(trading): AT-004 — …`.
2. **A fix is not done until a test or ratchet makes the regression impossible.** Where a test is impractical, say so in the commit.
3. **Tick the box in the same commit.** A ledger that lags the code is worse than no ledger.
4. **Work waves in order.** Later waves assume earlier guardrails exist.
5. If a finding turns out to be wrong, mark it `~~struck~~ (invalid: reason)` — do not delete it.

Verdict key: `C` confirmed · `W` weakened · `?` unverified (re-check before acting).


## Errata — commit id references (2026-08-02)

Commits `f32dfc3a`..`97dc2aab` cite some `AT-` ids that do not match this
ledger. I wrote them from memory instead of looking them up. The **ticks in
this file are correct** (they were matched by finding title, not by id), and the
in-source `AUDIT ... (AT-xxx)` comments have been corrected. The commit messages
themselves are already pushed and left as-is; use this table to read them.

| Cited in commit | Actual ledger id | Finding |
|---|---|---|
| AT-060 | **AT-079** | layer_guard glob hole |
| AT-118 | **AT-108** | bar_cache key has no range dimension |
| AT-003 | **AT-001** | ABBA lock inversion |
| AT-036 | **AT-011** | watchdog ignores pong/control frames |
| AT-011 | **AT-003** | cross-timeframe indexed in source-bar space |
| AT-012 | **AT-017** | RSI/ATR three implementations |
| AT-014 | **AT-018** | multi-timeframe RSI row labels |
| AT-013 | **AT-019** | VWAP implemented twice |
| AT-015 | **AT-035** | halt blocks position-reducing orders |
| AT-016 | **AT-036** | paper_mode not persisted |
| AT-017 | **AT-033** | paper mode adopts real broker orders |
| AT-018 | **AT-032** | confirming a single group leg |

Correct as cited: AT-002, AT-004, AT-005, AT-007, AT-008, AT-009, AT-010,
AT-026, AT-073, AT-075, AT-124.


## New findings from verification (2026-08-02) — not in the original audit

Surfaced while verifying existing items. Ids continue the AT- sequence.

- [x] **AT-142** `P1` `C` `TRADING` — `gpu.rs::tick_pane_frame` contains a LIVE synthetic
  tick/candle generator (random walk, Box-Muller) that writes invented candles into
  `chart.bars`. Gated at RUNTIME, not compile time: `!symbol_meta.is_crypto() &&
  !apex_data::is_enabled()`. So a release build with ApexData disabled fabricates
  price data on a non-crypto symbol — and unlike the chain/gamma/CVD paths it threads
  NO `placeholder`/`synthetic` flag out to a badge. Bigger accident surface than the
  four dead generators removed in this pass.
- [ ] **AT-143** `P1` `C` `DATA` — `dom_feed.rs:99` tags each inbound frame with the
  CURRENT global `ACTIVE_SYMBOL` rather than the symbol the socket was dialled with,
  so `ChartCommand::DomLevels` can carry the wrong symbol during a switch or reconnect
  storm — one instrument's ladder routed into another's DOM panel.
- [ ] **AT-144** `P2` `C` `STATE` — `persist_supervisor`'s `SHUTDOWN` is a process-global
  `OnceLock<AtomicBool>` that is never reset, so after all windows close and
  `open_window` restarts the event loop, the new supervisor reads the latched flag and
  dies immediately. Persistence is dead for the rest of the process lifetime.
- [x] **AT-145** `P2` `C` `UX` — `news_panel::draw_content` renders the headline list
  TWICE (a `NewsRow` loop outside the scroll area, then a `PanelListRow` loop inside).
  Every headline appears twice; only the first copy's click was wired.

- [ ] **AT-146** `P2` `C` `MISSED` — `cargo check --examples` has been FAILING on this
  branch (`examples/seed_watchlist_universes.rs` matches `Option` on a `Result` from
  `apex_rest::fetch_holdings`). Verified broken at HEAD~1 too, so it is not recent.
  Nothing catches it: CI is `--lib` only (AT-073) and the new `shipping-binary` job
  checks `--bins`. Either fix the example and add `--examples` to that job, or delete
  it — but a target that has never compiled should not sit in the tree pretending to.

- [ ] **AT-147** `P2` `C` `MISSED` — `tests/dev_inspector.rs` hardcodes scenario filenames
  that no longer exist (`50_chart_indicator_heavy_fps`, `51_order_open_close_fast`,
  `66_options_sentiment_pane`, and ~17 more in the 50-69 range). They were renamed or
  removed and the test was never updated, so it reports "could not load scenario" and
  the whole target fails. Invisible because CI is `--lib` only (AT-073). Either repoint
  the test at the current scenario set or have it enumerate `dev/scenarios/` rather than
  hardcode names — hardcoding is what let this rot silently.

### Corrections to original findings

- **AT-064 / ThemeRegistry** — the audit counted `registry.rs` (266) twice. Real total
  is **979 LOC** (registry.rs 266 + snapshot.rs 713), not 1,245.
- **Subpixel text** — the audit's 2,105 LOC is three whole files summed. Only
  **~1,230** is subpixel-specific; the remaining ~1,660 in `text_engine.rs` /
  `polished_label.rs` is the live grayscale cosmic-text backend with real consumers.
- **`chart/state`** — the audit's "delete it, it is unreachable" premise is **WRONG**.
  `persistence/drawing_db.rs` consumes `DrawingKind` / `DrawingFlags` / `Point` and
  `codec::db::points_packing` at ~40 call sites. Roughly 1,481 of the 2,583 LOC are in
  the live compile closure. Only the XOL/file_io/commands trio (~1,102 LOC) is
  genuinely dead.

## Progress

- Total: **141**
- P0: 10
- P1: 34
- P2: 58
- P3: 39


---

## W0 — money-path transport (DONE)  (1 items)

- [x] **AT-006** `P0` `C` `TRADING` — Bracket/OCO/options-trigger broker submits ignore HTTP status — a broker REJECTION returns Ok and paints 3 phantom "Working" legs with no backend id

---

## W1 — risk-gate integrity  (9 items)

- [x] **AT-007** `P0` `C` `TRADING` — Position caps and oversell protection read local `Filled` rows that are never persisted and are GC'd hourly — every position-based guard reads zero after a restart, while broker positions are explicitly discarded
- [x] **AT-008** `P0` `C` `TRADING` — `confirm()` marks the order Working before the broker Ack, drops the broker Err, and writes no journal event — the exact defect A2 fixed in `submit()`
- [x] **AT-009** `P0` `C` `TRADING` — `submit_bracket` bypasses `validate_risk` entirely — no position cap, notional, buying power, daily-loss, dedup, or max-open-orders check on a 3-leg live order
- [x] **AT-010** `P0` `C` `UX` — Orders panel Place All / Cancel All / per-row cancel never reach OrderManager or the broker — they flip a local field a per-frame reconcile immediately reverts
- [x] **AT-032** `P1` `C` `TRADING` — Confirming a Draft OCO or bracket leg re-submits it as a standalone order — the OCA group and bracket parent are lost, so both legs can fill
- [x] **AT-033** `P1` `C` `TRADING` — In paper mode the poller still adopts REAL broker orders and the paper fill engine then fabricates fills for them, with synthetic prices booked into realized P&L
- [ ] **AT-034** `P1` `C` `TRADING` — Kill switch, halt and resume ignore the broker's HTTP status AND their Result is discarded — a failed server-side kill reports success
- [x] **AT-035** `P1` `C` `TRADING` — Risk gates block position-REDUCING orders: once the daily-loss breaker auto-halts, Flatten/Reverse cannot close the position, and the failure is completely silent
- [x] **AT-036** `P1` `C` `TRADING` — `paper_mode` is not persisted: a restart without APEX_TRADING_MODE=live restores real live orders into paper mode, where cancel is a no-op that marks them Cancelled locally

---

## W2 — guardrails: make regressions impossible  (7 items)

- [x] **AT-026** `P1` `?` `MISSED` — dev_inspector's HTTP server is an unauthenticated, browser-reachable control plane that can synthesize real clicks into a window that may be in live-trading mode
  - **Correction:** finding overstated. The server is `#[cfg(debug_assertions)]`-gated (`native_main.rs:42`) and binds `127.0.0.1` — it does **not** exist in release builds. Real residual risk was browser CSRF against loopback; closed by rejecting requests that carry an `Origin` header.
- [ ] **AT-061** `P2` `C` `DEADCODE` — ui_kit's dead surface is hidden by six module-wide `#[allow(dead_code)]` blankets
- [x] **AT-073** `P2` `?` `MISSED` — No CI job ever compiles the shipping artifact: every job is `--lib` only, debug-only, and ubuntu-only, so both `[[bin]]` targets, the release configuration, and all 30 `cfg(windows)` sites are never type-checked
- [x] **AT-075** `P2` `?` `MISSED` — The quality-gate ratchet — the CI job whose job is to block regressions — is currently failing on committed HEAD, so it no longer distinguishes new regressions from old ones
- [x] **AT-079** `P2` `C` `OTHER` — The layer_guard ratchet has a glob-shaped hole; a real chart-layer dependency inside ui_kit currently passes as clean
- [x] **AT-124** `P3` `?` `MISSED` — The Prometheus metrics server listens on 0.0.0.0:9091 in release builds with no authentication and `Access-Control-Allow-Origin: *`
- [ ] **AT-129** `P3` `C` `OTHER` — Blanket #[allow(dead_code)] on 6 of 12 ui_kit modules disables the only automatic shelfware detector over 29,000 lines

---

## W3 — data-layer correctness  (4 items)

- [x] **AT-001** `P0` `C` `DATA` — ABBA lock inversion between ApexData ROUTES mutex and SubscriptionManager maps can deadlock the WS reader against any chart-load thread
- [x] **AT-002** `P0` `C` `DATA` — gap_fill_on_reconnect replays a FULL historical bar series into the live chart append path, appending stale out-of-order bars after the current bar
- [x] **AT-005** `P0` `C` `REDUND` — Chain cache short-circuit is keyed on underlying only — a 30/60-DTE request silently renders the nearest ≤14-DTE expiry under a "30D" label
- [x] **AT-011** `P1` `?` `DATA` — ApexData watchdog never counts pong/control frames as liveness, contradicting its own comment — a healthy but quiet feed is force-reconnected every ~30s

---

## W4 — engine correctness  (5 items)

- [x] **AT-003** `P0` `C` `ENGINES` — Cross-timeframe indicators emit a series indexed in source-bar space but rendered in chart-bar space
- [x] **AT-004** `P0` `C` `ENGINES` — Live incremental path appends NaN for 16 of 19 indicators and permanently suppresses full recompute
- [x] **AT-017** `P1` `C` `ENGINES` — RSI and ATR each exist in three implementations with three different smoothing conventions, all displayed simultaneously
- [x] **AT-018** `P1` `C` `ENGINES` — The multi-timeframe RSI widget labels seven rows 5m…1W but computes all seven on the pane's current timeframe
- [x] **AT-019** `P1` `C` `ENGINES` — VWAP is implemented twice with different session-reset rules; the σ-band version never resets on crypto or futures

---

## W5 — finish the migrations (RC-1)  (21 items)

- [ ] **AT-016** `P1` `C` `DESIGNSYS` — Two live type ladders have drifted apart: the tier scale was lifted to 9/10/12/14 but the semantic StyleSettings ladder still renders 8/10/11 — TextStyle::Caption is now BELOW the tier floor
- [ ] **AT-027** `P1` `W` `REDUND` — Broker order URL is a hardcoded dev-host const; the runtime-configurable resolver built for it is an orphan file that never compiles
- [ ] **AT-028** `P1` `C` `REDUND` — The full option chain is cloned and re-materialized twice per frame on the UI thread, in a call the codebase itself documents as too expensive to call per-frame
- [ ] **AT-029** `P1` `C` `REDUND` — Three writers into watchlist.chain.near/far; the per-frame cache-derive clobbers the command path and never clears the PLACEHOLDER flag
- [x] **AT-030** `P1` `C` `REDUND` — Two divergent RSI implementations and two divergent ATR implementations, both rendered on screen at the same time
- [ ] **AT-031** `P1` `C` `REDUND` — Two type scales run in the same frame: TextStyle is style-live, ui_kit's font_*() is frozen to literals in every shipping build
- [x] **AT-041** `P1` `C` `UX` — News panel renders every headline twice, and the second copy's click handler is a dead `// TODO: open URL`
- [ ] **AT-050** `P2` `C` `ARCH` — No single HTTP/endpoint layer: 16 files build their own reqwest client and the ApexSignals base URL is re-derived from env at 8 independent sites
- [ ] **AT-054** `P2` `?` `DATA` — IB feed bumps the gap-fill anchor with a hardcoded "5m" timeframe inside a loop that already knows the active timeframes
- [ ] **AT-081** `P2` `W` `OTHER` — Two SegmentedControl types and two Select types, both live in production, with mutually incompatible call conventions
- [ ] **AT-082** `P2` `C` `REDUND` — Hot-reload StyleSystem override maps the wrong stroke tiers, thickening every hairline the moment a theme JSON is present
- [ ] **AT-083** `P2` `C` `REDUND` — The shared HTTP client introduced to fix per-call TLS handshakes was adopted by only two files; the pre-trade margin check still builds a fresh client per call
- [ ] **AT-084** `P2` `C` `REDUND` — The strikes-overlay chain fetch is a third parallel path that neither reads nor seeds the shared chain cache
- [x] **AT-085** `P2` `C` `REDUND` — ~980 lines of a fully-built parallel theme model (ThemeRegistry / ActiveTheme / DesignSnapshot) has zero production callers, and a test comment falsely claims begin_frame uses it
- [ ] **AT-104** `P3` `W` `ARCH` — Two parallel persistence schemes coexist: a versioned `Persistable` envelope used by ~6 aggregates, and ~15 hand-rolled JSON files with no version field
- [ ] **AT-117** `P3` `C` `DEADCODE` — `chart/renderer/compute.rs` holds a second, dead copy of the drawing-tool math that `core.rs` implements inline
- [x] **AT-120** `P3` `C` `DESIGNSYS` — Two divergent definitions of the Meridien style exist; the one the ThemeRegistry defaults to is not the one the app renders
- [ ] **AT-125** `P3` `?` `MISSED` — The library declares `staticlib` and `cdylib` crate-types with no FFI consumer anywhere in the repo
- [ ] **AT-132** `P3` `W` `REDUND` — Greeks arrive from two independent sources with no reconciliation: the chain cache and a serial per-contract HTTP poller
- [ ] **AT-133** `P3` `C` `REDUND` — Verbatim-duplicated blocks across the chain stack: two JSON row parsers, four to_rows closures, two whole option quick pickers
- [ ] **AT-140** `P3` `C` `UX` — Script panel error output is never styled as an error — the error-detection prefix does not match the string the code produces

---

## W6 — design-system + ui_kit unification  (12 items)

- [ ] **AT-013** `P1` `?` `DESIGNSYS` — FROZEN CHROME: DesignTokens::default() is pinned to the pre-lift scale, so design-mode builds render a smaller type scale than shipping and RESET restores the wrong look
- [ ] **AT-014** `P1` `?` `DESIGNSYS` — Style hot-reload logs a successful StyleSystem reload but applies only radii and strokes — typography, spacing, density, treatments and chrome edits are silently dropped
- [ ] **AT-015** `P1` `C` `DESIGNSYS` — Theme-invariant categorical colours (incl. stop/target/R:R chart lines) score 1.1:1–3.0:1 contrast on all 5 light ColorSchemes
- [ ] **AT-062** `P2` `W` `DESIGNSYS` — FROZEN CHROME: FONT_* consts are pinned to the pre-lift scale and their doc comment now asserts a false equivalence; 45 live call sites render 1-2px small
- [ ] **AT-063** `P2` `C` `DESIGNSYS` — The hot-reload path remaps stroke tiers differently from the preset path, so the same StyleSystem paints borders up to 2× thicker when loaded from JSON
- [x] **AT-064** `P2` `C` `DESIGNSYS` — ThemeRegistry / DesignSnapshot (979 LOC) are documented as the canonical active-pair state but have zero references outside design_system/
- [ ] **AT-065** `P2` `C` `DESIGNSYS` — Whole StyleSystem sub-structs (alphas, elevation) and 11 further fields are inert, yet the design inspector ships sliders for them
- [ ] **AT-066** `P2` `C` `DESIGNSYS` — paint_bevel hardcodes white/black and documents itself as palette-independent, producing a one-sided dark smear on light palettes
- [ ] **AT-077** `P2` `C` `OTHER` — Label adoption is ~15%: 295 raw ui.label calls in production panel code vs 51 kit Label uses, across five competing label abstractions
- [ ] **AT-103** `P3` `C` `ARCH` — The `apex-playground` component gallery renders ui_kit widgets with default design tokens, never the ones the app ships
- [ ] **AT-106** `P3` `C` `DATA` — ApexDataProvider::unsubscribe_* removes the entire route key, dropping every other subscriber's sender for that symbol
- [ ] **AT-119** `P3` `C` `DESIGNSYS` — Five ColorScheme fields (hud_bg, hud_border, text_muted, notification_red, pinned_row_tint) are hand-authored in 21 palettes and read by nothing

---

## W7 — dead code + unwired: wire or delete  (32 items)

- [ ] **AT-012** `P1` `W` `DEADCODE` — The kill switch is one-way: `release_kill_switch()` has no caller, so engaged=true is unrecoverable in-app
- [ ] **AT-037** `P1` `W` `UNWIRED` — Screener BUILD tab: "Run Now" and "Save Screen" are literal no-op handlers, and their reducers are stubs
- [x] **AT-038** `P1` `C` `UNWIRED` — Time & Sales tape is permanently empty: the WS subscription is gated on `tape.open`, a flag no user action can set
- [ ] **AT-039** `P1` `C` `UNWIRED` — Two independent shortcut systems collide: Ctrl+Shift+R resumes halted trading AND toggles a panel in the same frame
- [ ] **AT-042** `P1` `?` `UX` — News sentiment filter chip (Any/Bull/Bear/Neut) changes its own label and active state but never filters the list — the filter function is only ever called by unit tests
- [ ] **AT-045** `P2` `C` `ARCH` — 73% of the AppCommand dispatch layer has no producer in a release build — its only callers are the debug-only dev_inspector
- [ ] **AT-058** `P2` `?` `DATA` — SubscriptionManager::check_stale() has zero callers — the documented per-subscription staleness TTL never fires
- [ ] **AT-059** `P2` `W` `DEADCODE` — Conditional orders and options-trigger orders are implemented end-to-end but have no production entry point
- [x] **AT-060** `P2` `W` `DEADCODE` — `chart/state` — 2,583 LOC of chart-storage architecture — is unreachable; its single integration point is hardcoded `None`
- [ ] **AT-074** `P2` `?` `MISSED` — The cooperative-shutdown subsystem is entirely dead: `drain_all` has zero callers, so the Postgres pool is never closed and the bug its own doc claims to fix is unfixed
- [ ] **AT-076** `P2` `C` `OTHER` — ContextMenu (507 LOC) and Popover (173 LOC) have zero production callers; the app uses raw egui context menus in 22 places
- [x] **AT-095** `P2` `C` `UNWIRED` — Command-palette Help and Calc entries execute to nothing — the dispatcher has no arm for their ids
- [x] **AT-096** `P2` `C` `UNWIRED` — Four rail panels are registered in the dispatch table but their `is_open` predicate has no writer that can ever return true
- [x] **AT-097** `P2` `C` `UNWIRED` — News headline clicks are a no-op: the URL is present and non-empty-checked, then discarded
- [ ] **AT-098** `P2` `W` `UNWIRED` — The whole ApexSignals integration — not just the three MSG panels — defaults to http://localhost:8100 with no way to configure it
- [ ] **AT-099** `P2` `W` `UNWIRED` — Three user-editable hotkeys are read by nothing, including "Halt Trading" — and the F1 cheatsheet advertises a Halt chord that does something else
- [ ] **AT-100** `P2` `C` `UNWIRED` — Two complete, tested panels have zero call sites anywhere in the tree
- [x] **AT-109** `P3` `C` `DATA` — providers/mock.rs (546 lines) and providers/replay.rs (212 lines) are compiled into production builds despite being test-only scaffolding
- [ ] **AT-110** `P3` `C` `DEADCODE` — Dead REST client surface in the ApexData feed: async auth-retry wrappers and four sync getters with no callers
- [ ] **AT-111** `P3` `W` `DEADCODE` — FMV is ingested on every frame into a map with no reader — `get_fmv()` is never called
- [ ] **AT-112** `P3` `W` `DEADCODE` — Fabricated market-data generators sit unreferenced in gpu.rs — a 100-LOC landmine in a live-money binary
- [ ] **AT-113** `P3` `W` `DEADCODE` — Halt tracking maintains two capped rings that no code reads; the comment claiming otherwise is false
- [ ] **AT-114** `P3` `W` `DEADCODE` — The `InFlightRegistry` migration stalled: entries are created and never expired, and no consumer reads it
- [ ] **AT-115** `P3` `W` `DEADCODE` — `ChainRow::display_price()` remains test-only — production still renders the raw field it was written to replace
- [ ] **AT-116** `P3` `W` `DEADCODE` — `SubscriptionManager::check_stale()` — the documented silent-stale-feed alarm — is never called
- [x] **AT-118** `P3` `C` `DEADCODE` — `design_system::registry` (266 LOC) is a competing theme source-of-truth with zero references outside its own module
- [x] **AT-126** `P3` `W` `OTHER` — 2,105 LOC of subpixel-text machinery (incl. a 986-LOC wgpu pipeline) serves exactly one production call site
- [ ] **AT-128** `P3` `C` `OTHER` — All five free-function helpers in ui_kit/widgets/mod.rs are dead, and they hand-roll raw egui inside the design system
- [ ] **AT-131** `P3` `W` `OTHER` — Thirteen exported types in the kit's public surface have no consumer anywhere outside their defining file
- [ ] **AT-137** `P3` `C` `TRADING` — `trading/config.rs` is dead and the broker URL is a hardcoded dev host — there are three competing URL mechanisms and the order path honours none of the overrides
- [ ] **AT-138** `P3` `C` `UNWIRED` — 21 AppCommand variants have reducers but zero emitters; the alert-snooze feature exists only as a reducer
- [ ] **AT-139** `P3` `W` `UNWIRED` — The command palette's top-billed "Ask Gemma" hero entry and Dynamic-UI action are acknowledged placeholders occupying prime real estate

---

## W8 — remainder (perf, coupling, arch)  (50 items)

- [ ] **AT-020** `P1` `W` `FAILSILENT` — Armed order-ticket submit discards OrderResult, so the NeedsApproval soft risk gate silently voids the order
- [ ] **AT-021** `P1` `C` `FAILSILENT` — Gamma/GEX feed outage is completely silent and stale walls are served indefinitely as live data
- [ ] **AT-022** `P1` `W` `FAILSILENT` — Kill switch / halt / resume report success before the broker call runs, discard its Result, and treat HTTP 500/404 as success
- [ ] **AT-023** `P1` `C` `FAILSILENT` — Panel Close/Half/Reverse market orders carry price=0 and last_price=0, so the max_notional risk gate silently evaluates $0 and never fires
- [ ] **AT-024** `P1` `W` `FAILSILENT` — flatten / flatten_all / halve / reverse bypass record_submit_outcome, making every rejection — including kill-engaged and rate-limit — completely invisible
- [ ] **AT-025** `P1` `?` `MISSED` — Every inbound WebSocket bar frame performs a synchronous file open/append/close on the tokio WS runtime thread
- [ ] **AT-040** `P1` `?` `UX` — "Close All" flattens every open position account-wide on a single unconfirmed click, while deleting a workspace, removing widgets, and the identical palette command all require confirmation
- [ ] **AT-043** `P1` `C` `UX` — Portfolio "NET LIQ" silently displays gross exposure when the account summary is missing, and a disconnected broker still renders a full frozen P&L board
- [ ] **AT-044** `P1` `C` `UX` — Spread Builder renders Max Profit / Max Loss / Break Even from invented option premiums and an invented payoff rule, with no badge and no contract multiplier
- [ ] **AT-046** `P2` `C` `ARCH` — Bidirectional chart ↔ data dependency: market-data feeds reach up into the renderer, and the crate root is the feed→UI adapter
- [ ] **AT-047** `P2` `W` `ARCH` — Blocking 5-second Postgres round-trip executed on the winit UI thread inside WindowEvent::RedrawRequested
- [ ] **AT-048** `P2` `W` `ARCH` — Headless scenario testing asserts against a shadow state machine that never touches Chart, Watchlist, or OrderManager
- [ ] **AT-049** `P2` `W` `ARCH` — Live order submission executes inline inside a 9,632-line function that the module doc declares frozen from refactoring
- [ ] **AT-051** `P2` `W` `ARCH` — Trading-safety bootstrapping (orphan recovery, kill-switch restore, broker watchdog) only runs if wgpu device creation succeeds
- [ ] **AT-052** `P2` `C` `ARCH` — Two divergent ApexIB base-URL resolvers; the live order path is pinned to a compiled-in dev host and ignores the env override
- [ ] **AT-053** `P2` `?` `ARCH` — `gpu.rs` is misnamed: 96% of its 10,690 lines are not GPU code, and the real GPU pipeline is a different module
- [ ] **AT-055** `P2` `C` `DATA` — Option-chain and realized-delta caches are exempt from the live_state eviction pass that covers every other per-symbol map
- [ ] **AT-056** `P2` `C` `DATA` — Per-frame ws::set_quotes at ~60Hz still costs a full SubState clone + sort + JSON serialize on the 2-thread WS runtime, even when suppressed
- [ ] **AT-057** `P2` `C` `DATA` — Redis bar cache does a blocking call behind one process-global Mutex, with no connect timeout, directly inside an async fn
- [ ] **AT-067** `P2` `C` `ENGINES` — ATR percentile ranks current volatility against the oldest bars in the buffer, not a recent lookback
- [ ] **AT-068** `P2` `C` `ENGINES` — Chart-widget data is cached on bar count alone, so RSI/ATR/price/pivots are frozen for the entire duration of the building bar
- [ ] **AT-069** `P2` `C` `ENGINES` — MA ribbon cache avoids the EMA math but deep-clones six full-length Vec<f32> every frame
- [ ] **AT-070** `P2` `C` `ENGINES` — compute_trend_grid's EMA column is identical in all 7 timeframe rows and is not an EMA
- [ ] **AT-071** `P2` `W` `FAILSILENT` — Prometheus apex_feed_state is hardwired for three of four feeds — crypto and signals can never leave Idle, ib_ws can never report Subscribed
- [ ] **AT-072** `P2` `?` `FAILSILENT` — gamma_synthetic badge is never cleared by the per-frame refresh, so real GEX data keeps painting a SYNTHETIC warning
- [ ] **AT-078** `P2` `W` `OTHER` — Settings font picker bypasses both the style-font arbitration and the FontRegistry install path
- [ ] **AT-080** `P2` `W` `OTHER` — The ui_kit verification harness (apex-playground) never installs fonts or the icon font, and runs under a different host than production
- [ ] **AT-086** `P2` `W` `STATE` — Closing ANY chart window permanently kills debounced persistence for the whole process
- [ ] **AT-087** `P2` `C` `STATE` — Every chart window creates its own 6 Stores pointed at the SAME 6 file paths, and they are never unregistered — closed windows overwrite live ones on quit
- [ ] **AT-088** `P2` `C` `STATE` — The designed state architecture covers 6 of ~193 module-level globals — the rest is ad-hoc accretion with no ownership model
- [x] **AT-089** `P2` `C` `STATE` — atomic_write uses a fixed shared `<path>.tmp` sibling, and two threads can write the same store path concurrently
- [ ] **AT-090** `P2` `C` `STATE` — dom_feed's single global ACTIVE_SYMBOL is driven per-frame from per-pane state — two open DOM ladders on different symbols cause a permanent reconnect storm
- [ ] **AT-091** `P2` `C` `TRADING` — A partially-filled order books P&L on its FULL size — `filled_qty.max(qty)` — feeding a wrong number into the daily-loss circuit breaker
- [ ] **AT-092** `P2` `C` `TRADING` — An OCO leg whose conId lookup fails is silently dropped, and the surviving legs' backend ids are then mapped positionally onto the local orders
- [ ] **AT-093** `P2` `C` `TRADING` — Orders in PendingCancel / PendingModify / Unknown are not persisted — the states where we are least sure what the broker holds are the ones dropped on restart
- [ ] **AT-094** `P2` `C` `TRADING` — `find_local_match` falls back to (symbol, side, qty) with no price or timestamp — a fill can be attributed to the wrong local order
- [ ] **AT-101** `P2` `C` `UX` — SUBMIT SPREAD can never succeed in any mode — the order manager unconditionally rejects the $0 limit the panel hardcodes — yet the button stays enabled and the disclosure says only live is blocked
- [ ] **AT-102** `P2` `C` `UX` — Spread strategy presets build strikes around hardcoded stale prices (SPY 580, NVDA 900) instead of the live underlying
- [ ] **AT-105** `P3` `C` `ARCH` — `foundation/design_inspector.rs` is 181 KB of chart UI living in the base layer, and is the sole source of the foundation→chart back-edge
- [ ] **AT-107** `P3` `W` `DATA` — Option-chain cache is invalidated on a server `resync` frame but NOT on a client-side reconnect, and a non-empty cache hard-short-circuits the REST re-seed
- [x] **AT-108** `P3` `W` `DATA` — bar_cache key has no range dimension, so a cache hit serves whatever range happened to be stored first, ignoring start_ms/end_ms/limit
- [ ] **AT-121** `P3` `C` `ENGINES` — RSI returns 99.01 instead of 100 when there are no losses in the window
- [ ] **AT-122** `P3` `C` `ENGINES` — Simulated option chain prices time in trading days while discounting at an annual rate; bs_delta is dead
- [ ] **AT-123** `P3` `W` `FAILSILENT` — Greeks poller swallows the standing 404 with an empty Err arm — the feature is permanently dead and nothing reports degraded capability
- [ ] **AT-127** `P3` `C` `OTHER` — A full Taffy flexbox engine is a hard dependency for 5 of 217 chart UI files
- [ ] **AT-130** `P3` `C` `OTHER` — Nine kit widgets are single-consumer domain code parked in the design system
- [ ] **AT-134** `P3` `W` `STATE` — CountingAlloc is installed as the global allocator in release builds — 4 contended atomic RMWs on adjacent statics for every heap allocation
- [x] **AT-135** `P3` `W` `STATE` — ORDERS_SNAPSHOT publish-after-unlock has no ordering guard — the order ledger and on-chart order lines can go permanently stale
- [ ] **AT-136** `P3` `C` `STATE` — Per-pane and per-window UI state parked in single-slot process globals — the options-chain seat set is cleared by whichever surface renders last
- [ ] **AT-141** `P3` `C` `UX` — Seasonality month attribution drifts across leap-year boundaries, misfiling early-January bars as December
- [x] **AT-148** `P2` `C` `UX` — Toolbar buttons have TWO sizing authorities; 7 of 9 styles render two different button heights in one row

  Found by running the app's own `/design-audit` across all nine styles, which
  had presumably been reporting it for a while. Aperture is clean; the other
  eight each report six `toolbar_height_consistency` failures.

  On Meridien the toolbar contains buttons at 24px and at 28px simultaneously:

      expected 24, got 28   layout_picker, settings_btn, search_btn,
                            toolnav_toggle, watchlist_toggle, timeframe_picker

  `toolbar_btn()` applies `.min_size(vec2(0, toolbar_control_h()))` AND, for
  icon-only labels, `.placement(IconPlacement::Toolbar)`. `Button::placement`
  documents itself as driving "glyph size and hit-target size (overriding
  `min_size` / `glyph_size`)". So icon buttons size from the placement and text
  chips from the token — two authorities in one row. They coincide on Aperture
  and diverge everywhere else, which is why this looks style-specific and is
  not.

  `IconPlacement::Toolbar.hit_px()` is additionally a hardcoded `24.0`, below
  the 28px touch minimum the same audit enforces — as are the other seven
  placements (PanelHeader 20, ListRow 16, TabClose 14).

  NOT FIXED DELIBERATELY. The obvious change — resolve `hit_px()` from the
  control ladder — alters icon-button sizing at 9 `Toolbar` call sites and 84
  more across the other placements, on every style. It also does not clearly
  explain the measurement: `hit_px()` reports 24 while the button measures 28,
  so the final height involves padding I have not traced. Making an app-wide
  sizing change on a model I cannot yet state precisely is how the two
  competing authorities got here in the first place.

  FIXED. Traced end to end: `paint_button` sets `desired.y = size.height()`
  (28 for `Size::Md`) and then applies placement's `hit_px` and `min_size` as
  FLOORS via `max()`. So `toolbar_btn`'s `min_size(0, toolbar_control_h())`
  did nothing whenever the style's control height was below 28 — while the
  MENU path treats the same field as an override (`min_size.or_else(placement)
  .unwrap_or(size.height())`) and shrank to it. One field, two semantics, and
  the row rendered both.

  My earlier note that `hit_px()` reports 24 where the button measures 28 was
  the clue I could not place: placement drives the GLYPH, not the height.

  * `Button::height(px)` — an EXACT height that wins over the size token, the
    placement hit target and the `min_size` floor. `toolbar_btn` uses it, so
    there is one authority for the row.
  * `MIN_TOUCH_TARGET_PX` in `ui_kit::style` — the touch minimum was hardcoded
    in the audit while the control ladder had no floor at all, so a style could
    author 24px controls and the app would render them and then report itself
    non-compliant every frame. One constant now floors `toolbar_control_h()`
    AND backs the check, so the threshold and the layout cannot drift.
  * A width floor too: `min_side` is the smaller dimension, so 28-tall buttons
    24px wide still failed (Octave). `min_size` is the right tool for that now
    that height has its own mechanism — the two uses stopped fighting once they
    stopped being the same call.
  * Badge height floored AFTER density: Octave's 0.85x scale turned 28 into
    23.8, the same failure the old raw 26.0 had, reached from the other side.
    Density may compress a control; it may not compress it out of reach.

  Verified by `/design-audit` across all nine styles: toolbar height
  consistency 6 -> 0 everywhere, touch targets 8-20 -> 4 everywhere. The
  remaining 4 are the pane-header chips (24px in a 28px header) — meeting the
  minimum there needs a TALLER header, which is a product density decision and
  is deliberately not taken here. Screenshot of Meridien confirms one uniform
  row.
- [ ] **AT-149** `P2` `C` `OTHER` — Font family has two mechanisms split by provenance; the token is decorative for every builtin style

  `style_preferred_font(style_id)` is a hardcoded index map (`1 => Some(1)`,
  `4 => Some(6)`, …) used for BUILTIN styles. Imported theme packs instead go
  through `Typography.family_ui`, read by `theme_pack_bridge`. Two answers to
  "what font is this style in", chosen by where the style came from.

  The builtins' authored `family_ui` says `"Inter"` for all three that set it
  at all — including Alto and Mariner, which actually render in IBM Plex Sans
  via the map. So the token is not merely unused for builtins, it is WRONG for
  them, and it round-trips through export/import saying so.

  NOT a mechanical migration, because of a semantic the map has and the token
  does not. `None` in the map means "this style has no opinion — honour the
  user's font picker" (Meridien, Octave). `family_ui` is always populated, so
  making it authoritative gives every style an opinion and silently overrides
  the user's choice on those two.

  It cannot be resolved by comparing against the default either: `"Inter"` is
  both the `Typography::default()` value AND Aperture's deliberate choice, so
  "differs from default" cannot distinguish "no opinion" from "explicitly
  Inter". `equivalence_tests` additionally asserts `!family_ui.is_empty()`, so
  the empty string is not available as a sentinel without changing that too.

  Needs a schema decision first — `Option<String>`, a separate
  `family_ui_is_preference` flag, or a reserved sentinel — and then the map is
  deleted and each builtin authors its true family. Font is the single most
  visible thing in the app, so this wants its own commit and a screenshot pass
  across all nine styles.
- [ ] **AT-150** `P2` `C` `UX` — The alpha ladder has holes where the app concentrates its opacity; 354 literals sit off-system

  Measured across the tree (excluding the token-definition and dev surfaces):
  634 opacity literals passed to `color_alpha` / `tint`, of which **354 match
  no rung of the alpha ladder**.

      ladder   10 faint  15 ghost  20 soft  40 subtle  48 tint  60 dim
               80 strong  100 active  120 heavy  140 scrim  200 solid

      off-ladder, by frequency
               160 x36   180 x36   30 x23   220 x22   18 x22   12 x16
               128 x16   50 x15    230 x13  150 x11   25 x10   8 x10

  The distribution is the finding. These are not scattered one-offs: 160 and
  180 appear 72 times between them, all in the gap between `scrim` (140) and
  `solid` (200), and 8/12/18/25/30 appear 81 times around and below `faint`
  (10). An unofficial second ladder grew in the holes of the real one.

  So the fix is NOT to snap 354 call sites to the nearest rung — that changes
  opacity in hundreds of places to satisfy a lint, and the call sites are
  arguably right: the app genuinely needs a tier between scrim and solid, and
  one or two below faint. The fix is to decide which recurring values deserve
  rungs, name them, and then migrate.

  Naming is the part that needs a person: what IS 160 — a heavier scrim, a
  veil, a modal backdrop? The name determines where future call sites land, so
  guessing it is worse than leaving the numbers.

  Gated meanwhile: `check-design-system.sh` now counts every literal alpha
  (on-ladder included — `alpha_dim()` says what it means and `60` does not, and
  a literal stops tracking when a style re-pitches the ramp). Baseline 740 ->
  1294; the delta IS this class.
- [ ] **AT-151** `P3` `C` `UX` — Three of the four shadow specs are authorable and ignored; modals, tooltips and dropdowns all wear the card's shadow

  `Shadows` declares four specs. Reads outside the declaration/serialisation
  files, by qualified grep:

      shadows.card       8
      shadows.modal      0
      shadows.tooltip    0
      shadows.dropdown   0

  Only `card` is wired — the adapter maps `ss.shadows.card.{blur,offset_y,
  alpha}` into the single `shadow_*` fields on `StyleSettings`, and every
  surface reads those. Modals DO paint shadows (`ui_kit/widgets/modal.rs`), just
  not their own: they inherit the card geometry.

  The styles author real differences that are being discarded — card blur 24,
  modal 36, tooltip 12, dropdown 24. A modal is supposed to sit further off the
  surface than a card; right now it cannot.

  Fix is to carry the three specs through `TokenSnapshot` and have the modal,
  tooltip and dropdown surfaces read their own. Deferred rather than done
  because shadows are purely visual and the verification needs a modal, a
  tooltip and a dropdown open on several styles — the harness interaction that
  has been unreliable in this session. Worth doing with a screenshot pass.

  CORRECTED AFTER TRACING. "Three specs ignored" understates it. There are
  THREE shadow mechanisms, and two of them declare a type called `ShadowSpec`:

      design_system::Shadows{card,modal,tooltip,dropdown}   authored per style; only `card` wired
      StyleSettings.shadow_{blur,offset_y,alpha}            derived from shadows.card; used by CardFrame
      ui_kit::widgets::shadow::ShadowSpec::{sm,md,lg}_themed  HARDCODED radius 8/16/24, alpha 64/77

  Modals do not merely miss `shadows.modal` — they use the third mechanism, a
  tiered GPU-composited shadow, and its values are literals. So this is not a
  wiring gap to fill; it is a consolidation, and the surviving mechanism has to
  be chosen rather than assumed.

  The third one is also carefully tuned. `modal.rs` explains at length why it
  uses the md tier and not lg: "lg (radius 24) routes to the GPU silhouette path
  and painted a ~48-pt halo on each side — visibly too wide. md (radius 16)
  stays in the fast CPU stacked-rect path". Replacing that with an authored
  `shadows.modal` blur of 36 would walk straight back into the halo it
  documents avoiding, and would change a CPU path into a GPU one.

  So the real work is: pick ONE shadow vocabulary, decide whether tiers
  (sm/md/lg) or roles (card/modal/tooltip/dropdown) are the right axis — they
  are different models, not two spellings of one — and note that the tier
  mechanism carries performance behaviour (CPU stacked-rect vs GPU silhouette)
  that the role mechanism has no way to express. Then rename one of the two
  `ShadowSpec` types.

  NOTE ON HOW THIS HID. `token_consumer_gate` layer 2 matches bare field names,
  and `card` / `modal` / `tooltip` / `dropdown` are widget names all over the
  app, so all four scored as consumed. Qualified matching was tried and
  reverted — see the limitation comment in the gate for why. When touching a
  group whose fields have single-word names, grep the qualified path by hand.

## AT-150 — alpha ladder holes — CLOSED (with a correction)

**Original claim:** 354 off-ladder alpha literals; the ladder has holes.

**Two measurement errors inflated that.** The ladder was extracted by regex from
`impl Default for Alphas`, matching `name: <literal>` — so `whisper` and `hint`,
which are set via `Self::default_whisper()` *function calls*, were scored as
off-ladder despite being rungs added the same day. And the count pooled chrome
with chart painting, where `color_alpha(base, 160)` is a candle body and `220` a
wick — data geometry, which was already scoped out of the layout argument for
exactly this reason and should have been scoped out here too.

**Corrected and fixed:**
- 48 chrome sites were within ±2 of an existing rung (18→20, 12→10, 8→10,
  50→48, 24→25). At 1/255 that is imperceptible, so they snapped — no new
  rungs, no naming decision needed.
- 13 sat in the one genuine gap. The ladder steps by 20 from `active` (100) to
  `heavy` (120) to `scrim` (140), then jumps 60 to `solid` (200). `dense` (160)
  and `near_solid` (180) fill it and continue the existing rhythm rather than
  inventing one.
- Chart-painting alphas are out of scope, stated in `GATES.md`.

**What the fix uncovered — AT-152.** Wiring the two new rungs showed that
`alpha_whisper` and `alpha_hint` read `al.whisper` directly in `begin_frame`,
bypassing the override and DesignTokens tiers all eleven siblings pass through.
Eleven fields across three groups (`alpha_*`, `font_*`, `gap_*`) had this, every
one of them a token I had added to satisfy the hardwire gate. See AT-152.

---

## AT-152 — snapshot fields that skip the cascade — CLOSED

A token with a `StyleSystem` field, a `TokenSnapshot` field and an accessor
looks finished. If `begin_frame` sources it as `al.whisper` rather than through
`override_style` / `dt_u8!`, it is authorable in a `.apextheme`, exports,
re-imports, round-trip asserts green — and does not move when its own inspector
slider is dragged.

Eleven fields: `alpha_whisper`, `alpha_hint`; `font_display_sm/md/lg/xl`,
`font_4xs`, `font_xs_plus`, `font_md_plus`; `gap_2xs`. Plus three more the new
gate found that a hand-scan had missed (`font_body`, `font_caption`,
`font_section_label` — my `sed` range stopped short of them).

No existing gate could see this: hardwire passes (the accessor is real),
token-consumer passes (`begin_frame` does read the field), ladder_gate asks
about scale multipliers rather than cascade tiers, and the suite passes because
an unauthored style renders byte-identically either way.

Fixed by routing all fourteen through the same three-tier expression, adding the
missing `DesignTokens` fields, and adding `dev/cascade_gate.py` (all-siblings-
or-none, the ladder gate's rule applied to the cascade). Verified to bite by
reintroducing the `alpha_whisper` bypass.

**Also found:** 14 `DesignTokens` leaves have no reader at all — invisible to
every gate, since the slider gate enumerates sliders and these have none. Four
were wired as part of this fix (`font.display`, `font.display_lg`,
`font.sm_tight`, plus `spacing.gap_2xs` added). The remaining 10 are recorded,
not yet triaged. A first pass counting only `dt_*!` reads said 48 — 34 of those
turned out to be read by direct field access, so the honest number is 14.

---
## AT-149 — font family had two mechanisms — CLOSED

`Typography.family_ui: String` could not say "no opinion". So deferral lived in
a second mechanism, `style_preferred_font(style_id) -> Option<usize>`, whose
`None` arm meant "honour the user's picker".

Two mechanisms, and they disagreed — the fifth instance of that class here:

- the map returned `None` for **Meridien** and **Octave** while both set
  `family_ui: "Inter"`;
- the map gave **Alto** and **Mariner** IBM Plex Sans while their `family_ui`
  said `"Inter"`.

The map was also keyed by style **INDEX**, the exact fragility the audit note on
`compact_adjusted` — the function directly below it — warns about: reordering
the style list silently reassigns entries.

**Fixed:** `family_ui` is `Option<String>`. `Some(name)` is a choice, `None` is
a deferral, and `"Inter"` is no longer both the default and a deliberate
selection (which is why "differs from default" could never separate them). The
map's arms were transcribed onto the styles themselves and the map deleted.
Export omits the key when a style defers — writing a name would turn a deferral
back into a choice on reload.

Guarded by two tests: one asserts every opinion the map held is still expressed,
keyed by style ID so reordering cannot break it; the other asserts a stated
family actually resolves to a registered font, since an unregistered name falls
back to the picker silently and the theme then renders in the wrong face with
nothing reporting it.

Behaviour is unchanged: Meridien/Octave → picker, Aperture/Cadence → Inter,
Alto/Mariner → IBM Plex Sans, Lucid → DM Sans. Verified in the running app.

---

## AT-151 — three shadow systems — CLOSED

Not "three systems that overlap" — three systems that **disagreed about the same
four elevations**:

| role     | `Shadows` (StyleSystem) | `shadow_preset` tokens | `ui_kit` tier |
|----------|-------------------------|------------------------|---------------|
| card     | blur 8, y2, a77         | blur 4, y2, a60        | sm r8 y2 a64  |
| modal    | blur 24, y8, a128       | blur 28, y8, sp2, a80  | lg r24 y8 a89 |
| tooltip  | blur 6, y2, a102        | blur 0, y2, a60        | sm r8 y2 a64  |
| dropdown | blur 12, y4, a102       | blur 24, y8, sp1, a40  | md r16 y4 a77 |

Whichever path happened to render won. The fourth instance of the
"two mechanisms disagree" class in this codebase.

**Resolved per the decision — tiers win, roles alias onto them:**

- `ShadowTiers { sm, md, lg, xl }` in `StyleSystem.shadows.tiers` is now the
  single authored source of depth, carried through `TokenSnapshot` as `elev_*`
  and read by `ShadowPaint::{sm,md,lg,xl}_themed`. Those constructors held
  `radius: 8.0` / `offset 0,2` / `alpha 64` as bare literals, so the elevation
  ladder was the one part of the design system no theme could author. The
  hardwire gate never saw it because it only scans `ui_kit/style.rs`.
- `shadow_modal`, `shadow_modal_themed`, `shadow_dropdown`,
  `shadow_dropdown_themed` deleted — zero call sites each. Modals and dropdowns
  already rendered from tiers (`md_themed` alone has 9 call sites), which is
  what settled the decision on evidence rather than taste.
- `shadow_preset.modal` / `.dropdown` tokens and `Shadows.dropdown` deleted with
  them; the token-consumer gate flagged `dropdown` the moment its last reader
  went, which is the gate working as intended.
- The Two-Axis editor's dead `dropdown` sliders became tier sliders, so the
  ladder is editable where it is authored.
- `ui_kit`'s `ShadowSpec` renamed `ShadowPaint`. Two different records under one
  name in one crate is a large part of how the three drifted: one is a role's
  authored geometry (blur/spread/offset_x/offset_y + 0.0–1.0 multiplier), the
  other is paint input (Gaussian radius, Vec2 offset, resolved Color32).

Guarded by: a monotonicity test (a ladder whose rungs are not ordered is not a
ladder), a `from_tier` test that fails if a constructor quietly keeps a literal,
and a round-trip assertion using an OFF-default fixture ladder — verified to
bite by deleting the export line.

---

## Chart-engine performance — measured, not assumed

Asked directly whether any of this design-system work touches the charting
engine's performance. Checked rather than reassured.

**Structural — nothing added executes per bar.** Across
`chart/renderer/render/`, `gpu.rs`, `chart/indicators/` and `chart_widgets.rs`
there are **zero** `cascade::` or `El::` call sites. Every alpha, geometry-inset
and recipe migration in this work excluded those paths by construction, because
`.left() + 6.0` there is a candle body, not chrome padding.

Only two chart-engine files changed at all across the whole effort:

* `render/pane/core.rs` — `OrderSide::color` gained an `accent` parameter (order
  LINES, not candles), and the pane-header chevron became an icon button. Both
  chrome.
* `gpu.rs` — font resolution replaced a `style_id -> index` match with the
  active style's own `family_ui`. Runs **once per frame**, behind the existing
  `LAST_FONT` atomic that already gates the expensive `set_fonts` call.

`dev_inspector::record` call sites in the render paths: **2 before this work, 2
now.** The new primitive-level instrumentation is `#[cfg(debug_assertions)]`, so
release builds carry none of it.

`begin_frame` gained one `Vec::clear` for the cascade scope stack.

**Measured A/B**, same machine, same workspace, debug + design-mode, three
samples each:

| | fps (cur / min / max) | frame_ms |
|---|---|---|
| pre-work `7b66c37f` | 27.8 / 16.3 / 60.3 | 35.9, 33.4, 50.1 |
| current HEAD | 20.0 / 13.5 / 60.2 | 50.0, 33.4, 33.5 |

Indistinguishable. Both oscillate between 33.3 ms (30 Hz) and 50 ms (20 Hz),
which is vsync quantisation rather than work, and both reach the same 60 fps
ceiling.

**Caveat, stated rather than buried:** this is a debug build and the sampler is
vsync-quantised, so it is a coarse instrument — it would not resolve a few
hundred microseconds. The stronger evidence is the structural one above: the
hot loops contain none of this code.

---

## AT-164 — the release build did not compile — FIXED

Found by trying to time a layout solve in `--release`.

`ui_kit::inspect::apply_ui_debug` reads `egui::Style::debug`, and that field is
itself `#[cfg(debug_assertions)]`. Referencing it unconditionally does not
compile in release. Now gated, with a release no-op — the overlay it drives is a
development affordance and the flags do not exist there.

**Not caused by this session's work.** The file is untouched across the whole
effort and the code is byte-identical at the commit this branch started from.

Worth stating why it went unnoticed: every build in this workflow is a debug or
`design-mode` build. The only configuration that failed is the one nobody
compiles day to day — and the one that ships. It is also why the chart-perf
answer given earlier (AT: "Chart-engine performance") had to rest on a debug,
vsync-quantised measurement with the caveat stated: release could not be built
to measure.

**A number that fell out of it.** With release building,
`flex.rs::report_solve_cost_for_a_typical_row` measures a 3-item flex solve at
**5.5 µs**. That sets a real boundary for the element-tree migration rather than
a taste-based one:

* `dom_row` paints ~40 rungs per frame by its own comment. A per-row solve there
  is 0.22 ms — **1.3 % of a 16.7 ms frame** — to replace roughly three
  multiplies. `watchlist_row` is the same shape.
* Both are now exempt from the cursor-walk ceiling **on measured cost**, with
  the measurement named in the gate so the exemption can be re-argued if the
  solver gets cheaper.

The tree earns its cost on conditional rows built once per frame — headers,
toolbars, tab strips, cards — not on fixed fractional splits painted forty times.

---

## AT-163 — the touch minimum and the control ladder contradicted each other — RESOLVED

Falling out of AT-162's 35-item queue. Every one was a HEIGHT under
`MIN_TOUCH_TARGET_PX` (28) — and the surfaces reporting them were asking for
`Size::Xs` or `Size::Sm`, whose rungs **are** 18 and 22.

So a button correctly requesting a dense rung was correctly 18 px tall and
correctly failed a flat 28 px minimum. Two parts of the design system
disagreeing, with neither misused. Holding every control to the primary minimum
would mean the dense toolbar chips this terminal is built from could never pass.

**Resolved by grading against the rung the caller asked for.** Asking for a
dense rung is a STATEMENT — the same reading `.muted()` gets in the cascade, and
the same shape as the audit's existing pane-chrome exemption by surface. `xs`
and `sm` are held to their own rung; `md` and up to the touch minimum, so a
`md` button that comes out at 20 px is still caught. The id carries the rung as
`auto.<surface>.<size>/…`.

**35 → 2.** The two survivors were real, and were exactly what the grading was
meant to isolate: `ToolBarButton::new("‹")` in the pane header — a bare glyph
with **no min width, 8.97 px wide** — while the same control's *other state*
(`identity_collapsed`) used `ToolBarButton::icon(Icon::GEAR)` and got a proper
square. One control, two states, two very different click targets. Both states
are icon buttons now.

**2 → 0.** Nothing in the app is under 12 px; the narrowest toolbar button is
25.3.

One correction worth keeping: I also made `Size`'s rung a height floor in the
button primitive, believing it caused this. It does not — no button's content
height falls below its own rung, so the floor never binds today. It closes a
hole rather than fixing a present defect, and the comment in the code says so.

---

## AT-162 — the design audit covered 2% of the app — FIXED

AT-161 found the settings modal had never been audited. The larger version of
that finding is worse.

`/design-audit` inspects widgets that call `dev_inspector::record(..)`, and
recording was the **call site's** job. A census: **24 such calls against 876
widget constructions across the panels and toolbar — 2 % coverage.** Every
"audit clean on all nine styles" reported in this ledger and in commit messages
was true and much weaker than it reads: it meant *the 2 % it could see* was
clean.

**Fixed by instrumenting the primitive.** `Button`'s shared wrapper already
derives a `bug_key` for the Bug-Inspect registry, so it now emits a
`WidgetRecord` from the same key — the id is shared rather than invented. Debug
builds only. Coverage went 451 → 490 widgets on the default screen, and **35
touch-target candidates appeared immediately** that were structurally invisible
before.

**Those 35 are reported, not graded, and that distinction is deliberate.** The
audit picks a widget's touch floor from its id prefix — `pane.` chrome is held
to `MIN_PANE_CHROME_TARGET_PX` (24), everything else to `MIN_TOUCH_TARGET_PX`
(28). An auto-emitted id carries no surface, so a pane-header chip at 24 px is
*correct* and would be failed here against 28. Grading them would mean reporting
known-good widgets as defects, which is precisely how a check stops being read
(see AT-154, AT-161).

**Attribution solved, and the queue is now specific.** `#[track_caller]` on
`Button::show` (and on the `ToolbarButton` wrapper, since attribution stops at
the first hop in a chain that lacks it) puts the CALLING FILE into the id:
`auto.<surface>/button/<slug>`. `Location::caller()` resolves at compile time,
so this costs nothing at runtime. The audit's pane-chrome exemption is a
statement about a surface, and the caller's file is the surface.

**Triage — 35 records, all heights below the 28 px floor:**

| height | count | surfaces |
|---|---|---|
| 18.0 | 6 | auto_chart_panel, plays_panel, button, mod |
| 20.7 | 2 | button, plays_panel |
| 24.0 | 8 | workspace_rail, toolbar_button |
| 25.3 | 19 | top_nav, auto_chart_panel, plays_panel, toolbar_button, button |

`min_side` conflated width and height; splitting them shows every one of these
is a **height** failure, not a narrow-button one. `toolbar_control_h()` already
floors itself at the touch minimum — these call sites are simply not using it.

Not fixed here, deliberately: raising 35 button heights across six surfaces is a
visible layout change and the 24.0 group may be legitimate chrome. It is a
follow-on pass with its own before/after, not a tail-end edit. What changed is
that it is now a specific, sized, attributed list instead of a blind spot.

---

## AT-161 — the design audit never saw a modal — FIXED, plus what it was hiding

`/design-audit` reported `clean` on all nine styles for months. It inspects
VISIBLE widgets, and the settings panel is a modal — so nothing in it had ever
been audited. Opening it while the audit ran turned up **21 touch-target
failures and 10 clipping reports at once**.

**The touch failures were real.** `const CHIP_H: f32 = 26.0` — a literal two
pixels under the app's own `MIN_TOUCH_TARGET_PX` (28.0), applied to every
preset, archetype and style chip in the panel. Now `chip_h()`, the control
ladder's `md` rung floored at the touch minimum, so it tracks a theme's density
and cannot fall back under the floor. 21 → 0, verified with the modal open and
43 settings widgets present in the tree.

**The clipping reports were the audit's own fault.** `is_clipped` asked only
"does this rect poke outside its clip rect", which is true both of a widget
truncated by an overflowing layout — the defect — and of one scrolled out of
view, which is what a scroll area is *for*. The style chips sat at y=1093.7 with
the viewport ending at y=1090: entirely below the fold, rendering perfectly,
reported ten times. The check now requires the rect to **partially overlap** its
clip rect, so truncation still fires and scrolling does not.

A check that fires on normal scrolling is one people learn to skip, which is the
same failure mode as the ratchet counting test fixtures (AT-154).

**One more real defect found on the way.** The STYLE section computes how many
chips fit per row as `(row_w + gap_xs()) / (btn_w + gap_xs())`, while the row
itself spaces with `CHIP_GAP`. 4.0 against 6.0 — so it fitted one chip more per
row than there was room for. The comment directly beneath records that the
spacing was changed from `gap_xs()` to `CHIP_GAP` for rhythm; this line was not
changed with it. Seventh instance of two mechanisms describing one value and
disagreeing.

Worth stating plainly: I fixed that arithmetic believing it caused the clipping.
It did not — the clipping was the audit's classification. The fix stands on its
own, but the diagnosis was wrong and the measurement corrected it.

---

## AT-160 — 136 edge insets were per-site literals — FIXED

The largest remaining chrome pattern, and the one AT-159 unblocked once the
"scale coupling" objection turned out to be based on a wrong model.

`rect.left() + 8.0`, `rect.bottom() - 2.0` and friends: 136 sites where a
padding inset from a rect edge was written as a number. Each is now the spacing
rung it already equalled — `gap_2xs` / `gap_xs` / `gap_xs_mid` / `gap_sm` /
`gap_md` / `gap_lg`.

**Only EDGE insets moved.** `.left()/.right()/.top()/.bottom() ± N` is padding
from an edge. Centring — `rect.center().y - 8.0`, which is half an icon — is not
padding, and a value that happens to equal a spacing rung there is coincidence.
Migrating those would have been a semantic error wearing a correct-looking
number, so the pattern was restricted to the four edges. (As it turned out the
codebase uses `.center().y` rather than `.center_y()`, so none were at risk —
but the rule is the reason, not the outcome.)

Off-rung values were left alone: 35 × `0.5` (pixel-grid alignment, which must
NOT scale — see the `panel_section` hairline fix in AT-154), 29 × `1.0`,
21 × `3.0`, and one-off widths like `236.0`.

**Verified byte-identical.** At Standard spacing every rung equals the literal it
replaced, so nothing should move — and a pixel diff of the running app across
four static chrome regions (timeframe row, top-nav, left rail, bottom tabs)
shows **0 differing pixels of 186,000**. The whole-window diff is 3%, entirely
live market data: chart bars, quotes and the alert feed.

What changes now is that a user on Tight or Loose spacing gets insets that
actually respond, which is what that setting was always supposed to mean.

Ratchet 1162 → **1066**.

---

## AT-159 — the two scale settings do NOT break containment — investigated, no defect

I reported the remaining geometry-nudge migration as blocked by "scale
coupling". That reasoning was wrong and this records why, because the wrong
version was stated confidently.

The app has two scales the user sets separately and that persist separately:
`SpacingScale` (0.75/1.0/1.25, multiplies `gap_*`) and `DensityMode`
(0.85/1.0/1.15, multiplies `control_*` and `row_*`). Type scales with neither.
The worst pairing grows padding 47% relative to the box holding it, which
*looked* like it had to break containment somewhere.

**Two models, both wrong, and the second said so loudly.** The first assumed
`control_sm` holds `font_sm` text with `gap_2xs` padding — an invented pairing;
it reported failures at numbers the app never uses. The second used `Size`'s own
`height()`/`padding()`/`font_size()` mapping and reported breakage in **40 of 45
pairings, including Standard × Standard** — the default configuration of an app
that plainly renders correctly. A model that indicts the defaults is describing
itself, not the app.

**What is actually true:** `Size::padding()` has no vertical consumer.
`Size::height()` is an exact target and text is centred inside it, so padding is
never added to height. The real relationship is just `height ≥ line box`, and it
holds at every density — though `control_xs` at Compact clears its own line box
by only **1.3 px**, thin enough that one nudge to the type scale would clip it.
That is precisely how AT-148 happened, so the relationship is now guarded, and
the guard was verified to fire by shrinking `control_xs` from 18 to 16.

**Consequence:** the geometry-nudge migration is not blocked. It remains a
consistency question (which literals are padding and should scale, which are
alignment and must not), not a containment risk.

---

## AT-158 — `ticker_strip` is built, styled, and never rendered — RECORDED

Found by migrating it. `chart/renderer/ui/components/toolbar/ticker_strip.rs`
describes itself as *"the signature element of the toolnav (second chrome row)
in the ApertureJune reference"*, and its entire public surface — `ticker_strip`,
`TickerEntry`, `TickerStripResponse` — has **zero references** outside its own
file. The only external mention is `pub mod ticker_strip;`.

That is the `sx::recipes` shape again: a complete component whose only tie to
the app is its module declaration. A census of `ui_kit/widgets`,
`toolbar` and `overlays` says it is the sole module in that state — the other
eight hits are internal helpers exposed `pub` for their own tests
(`solve_header_row`, `key_value_row_flex`), which is a different thing.

**WIRED** (decision taken by the owner). It renders in `render_toolnav`, in the
gap between the chart controls and the alert feed — exactly the space that used
to be a bare `ui.add_space(gap_w)`, so the feed still starts at the same 40 %
mark and the controls are untouched. Quotes come from loaded watchlist items;
clicking one loads it into the active pane through the same
`pending_symbol_change` path the command palette uses.

Items without a usable `prev_close` are **excluded rather than shown at 0.00 %**
— a computed-looking zero would read as "flat" when the truth is "unknown",
which is fabricated data rather than a placeholder.

Live: `QQQ 730.85 −0.17%   SPY 776.34 −0.20%`, symbol in text, price dim, change
in the bear tone.

**What was done:** its cursor walk was migrated to `Flex` anyway, which removed
a real defect in code that would ship the moment it is wired —
`if cx > rect.right() - 40.0 { break }` was a fixed guess unrelated to the quote
about to be drawn, so a quote wider than 40 px started inside the strip and ran
past its edge, where `painter_at` clipped it mid-glyph. The strip now measures
each quote and stops when one would not fit.

Two tests in `flex.rs` guard the measurement, because it relies on solving into
an INFINITE available width: one asserts that returns finite rects summing to
the content, the other that measuring and then placing agree — if they diverged,
every quote would paint at an offset from the box its click handler uses.

---

## AT-157 — four scale ladders were authorable but never cascaded — FIXED

`cascade_gate.py` reported 48 of 91 snapshot fields cascading. The other 43 sat
in 16 groups that read their `StyleSystem` value directly. The gate passed them,
correctly — its rule is all-siblings-or-none and "none" is legitimate for a
group that is deliberately snapshot-only. **What a gate cannot say is whether a
group SHOULD cascade.**

Four of those groups should, being the ladders a designer most wants to drag:
icon size (4), leading (6), row height (5), control height (5). Twenty fields,
theme-authorable, with no live control anywhere.

Now routed through the same three-tier expression as spacing and alpha, with
`IconTokens` / `LineTokens` / `RowTokens` / `ControlTokens` behind them and a
new **Scale Ladders** inspector category. 48 → **68 of 91** fields cascade, and
the slider gate holds at **0 dead of 93**.

`row_*` and `control_*` carry the density multiplier, and it has to apply on the
override branch too — otherwise the authored and overridden values disagree the
moment density leaves 1.0, the same shape as the `gap_xs_mid` defect the ladder
gate was written for.

**The first version of the test could not fail.** It asserted
`row_dense == 51.0 * dens` at the default density, where `dens` is exactly 1.0 —
so the multiplier could be deleted outright and it still passed. Confirmed by
deleting it. The test now forces density to Compact (0.85×) and drives *both*
cascade branches by installing a hot-reload override, and was re-verified to
fail when the multiplier is dropped from either one. A test that cannot fail is
worse than no test: it reports coverage it does not have.

**Still not cascading (23 fields, deliberately):** `elev_*` (documented as a
uniformly-direct group when the ladder was introduced), the four `bevel_*`, the
three `focus_*`, `rail_*`, `wl_*`, and the enum-valued treatments
(`button_treatment`, `surface_bevel`, `panel_tab_treatment`) which have no
meaningful slider. Uniform within their groups, so the gate is satisfied and
the remaining work is a judgement call rather than a defect.

---

## AT-156 — the recipe layer had tiers for everything except alpha — FIXED

`builtin_recipes.rs` is the largest single chrome entry in the ratchet, and all
of it was one thing: 56 `tint(ToneRef::X, <u8>)` calls. Its own authoring rules
say *"Tiers over pixels — `RadiusTier::Pill` / `PadTier::Md` track the active
style's ramp"*, with `RadiusTier::Px` as the documented escape hatch for a value
the source CSS states literally. Alpha had no tier at all: `ColorSpec::Alpha`
carried a bare `u8`, so 56 recipe colours were opacities no style could
re-pitch.

**The obvious fix is wrong and had already been tried.** One line read
`tint(ToneRef::Text, crate::ui_kit::style::alpha_soft())`. A `RecipeSet` is
rebuilt on **style change**, not per frame, so an accessor called there freezes
until the next style switch and stops tracking the inspector slider entirely.
Migrating 27 sites that way would have multiplied a staleness bug.

**`AlphaTier` resolves inside `ColorSpec::resolve`, which runs at paint time.**
27 call sites now name a rung and follow the live ladder; 29 keep
`AlphaTier::Raw` because the CSS states an opacity no rung expresses — the same
exemption `RadiusTier::Px` already had, but spelled `tint_raw` so it is greppable
rather than anonymous.

**The ratchet counts `tint_raw` on purpose.** Excluding it would have credited
this work with a 47-violation improvement when only 27 sites actually changed
behaviour — the other 20 would have been a function rename. The pattern was
widened and the real figure is 1211 → 1190. A number that can be moved by
renaming a function is not worth reading, which is the same argument AT-154 made.

Backward compatibility is kept by hand-written serde: `"alpha": 40` still loads
(every committed fixture used it), while a rung round-trips as `"alpha": "soft"`.
The committed Figma fixture's round-trip test caught the derived-enum version
demanding `{"raw": 40}` on the first attempt.

---

## AT-155 — the last dead controls and dead tokens — CLOSED

Two leftovers recorded by earlier findings, triaged one by one rather than
swept. The standing rule applies: delete what is genuinely useless, but a thing
that is unconnected through *oversight* gets finished instead.

**Wired (3) — the token existed and the widget hardcoded the value beside it:**
- `badge.font_size` — `badge.rs` held `let font_size: f32 = 10.0;`
- `badge.height` — `let h: f32 = 14.0;`
- `form.label_width` — `form_row.rs` held `label_width: 120.0`

Each token's default had to be moved to the literal it replaces (badge was
8.0/16.0, form was 80.0). Wiring without that is a silent resize dressed as
plumbing, and leaves the slider discontinuous at its own resting value — the
token-consumer gate caught exactly that and refused the first attempt.

**Deleted (15) — superseded, with the successor named:**
- `panel.margin_top/bottom`, `panel.compact_margin_*` — `side_panel_shell`
  builds its margins from `gap_*` tokens.
- `panel.tooltip_width_sm/md`, `panel.content_width_lg/xl` — no consumer and no
  hardcoded equivalent anywhere; never used.
- `icon_button.min_size` — `IconPlacement::*.hit_px()` owns hit sizing (AT-148).
- `font.input` — `Input` derives from `size.font_size().max(font_md())`.
- `split_divider.dot_count` — the divider computes how many dots fit its
  available height. A fixed count would be wrong, so this is superseded by
  something better rather than merely unused.
- `chart.right_pad_bars`, `chart.replay_progress_height`, `color.pane_tints`,
  `order_entry.padding` — no consumer, no equivalent.

**The inspector-slider gate's ceiling is now 0**, down from 59 when it was
written. Every slider in the design inspector moves a pixel, and the gate is a
floor rather than a budget from here: a dead control cannot be added.

---

## AT-154 — the ratchet was blind below any mid-file test module — FIXED

The design-system ratchet filtered out test-fixture hits with an awk pass that
cut each file at its FIRST `^\s*#\[cfg(test)\]` line and dropped everything
after it. Its own comment stated the assumption: *"relies on the Rust convention
that test modules sit at the END of a file."*

When the convention does not hold it fails in the dangerous direction —
**production code below a mid-file test module stops being counted at all.** It
hid 17 real violations, 11 of them in `chart/renderer/ui/style.rs`, whose test
module sits above `style_system_to_style_settings`. That is the same assumption,
in the same file, that previously made `token_consumer_gate.py` invent 37
findings by discarding the adapter below it.

The failure was unbounded, not one-off: any file that later gained a mid-file
test module would silently drop out of the count below that point, and the
ratchet would report an **improvement** for it. Adding a `#[cfg(test)]` accessor
to `panel_section.rs` during this session did exactly that.

**Fixed** by `dev/strip_test_hits.py` — brace-matched `#[cfg(test)] mod` bodies
*and* bare `#[test] fn` bodies (`design_system/equivalence_tests.rs` is declared
`pub mod` with no cfg gate and annotates each function individually).

**Newly-visible debt, and what was done with it:**
- `trading/mod.rs` decided four colours with no theme in scope — the session
  badge (`PRE` amber, `POST` blue, `CLOSED` grey) and the OCO-target purple.
  They rendered identically in all 22 palettes, light ones included. Fixed:
  `market_session()` now returns a `SessionPhase` and the call site picks
  `t.bull` / `t.warn` / `t.accent` / `t.dim`; `OrderSide::color` takes `accent`.
- `panel_section.rs` drew its header rule at `hr.bottom() - 0.5`, correct only
  while the stroke happens to be 1px. Now `stroke_thin() * 0.5` — half the
  rule's own width, so it stays on the pixel grid when a style re-pitches the
  stroke ramp.
- One `ui.add_space(1.0)` remains baselined: 1px has no rung, and minting one
  for a single call site is the ladder inflation these gates argue against.

**The filter has a self-test**, because it failed SILENTLY twice while being
built — once given git-bash MSYS paths (`/c/Users/...`) that native Windows
Python cannot open, once when an edit did not apply. Both times the only symptom
was the baseline moving by exactly the number of test literals in a few files,
which reads like real drift and invites a blind re-baseline. It runs in CI.

The headline number barely moved (1210 → 1211). That is the point: it now means
something.

---

## AT-153 — header titles overlap their own action buttons — FIXED

Found by looking at a screenshot rather than by a gate. The design inspector's
title painted straight through the RESET button beside it — an unreadable
collision *over a live click target*, not a cosmetic overrun.

**Root cause, and it was in the canonical widget too.** `ui.horizontal` with a
label followed by `with_layout(right_to_left)` does not push back when the
content exceeds the row: the right-to-left group overlaps the label. 30 sites
share that shape. And `ui_kit::Header` — the widget those sites are supposed to
migrate *to* — pinned its title `.shrink(0.0)` with a comment calling shrink "a
deliberate follow-up, not a plumbing change". That pin was correct while the
flex layer was replacing a cursor walk and had to overflow identically; kept
past that point it meant migrating to the design system would not have fixed the
bug.

Fixed in two halves, because either alone is insufficient:
- `Item::content(title_w).shrink(1.0)` — the title yields before the elastic
  middle collapses, so the close slot and trailing actions keep their width.
- The title is painted through a truncating `LayoutJob` bounded by its solved
  slot. A narrowed rect does not narrow glyphs; `painter.text` would have kept
  painting past the slot edge onto the buttons.

`an_oversized_title_overflows_rather_than_shrinking` now asserts the opposite of
what it did, and additionally that the title never overlaps the close slot.

**Residual — now CLOSED.** The inspector's five action buttons measured ~360 px
on their own, so its title elided to "…" at every allowed width. Widening the
panel was tried first and reverted, because the extra width went straight to the
buttons. The actual fix was to make the actions smaller, and both halves were
correct on their own merits before they were correct for width:

- `👁 LEFT` and `📖 Help` are icon-only. Both already carried `on_hover_text`,
  so the words were redundant width rather than information.
- **SAVE appears only when there is something to save.** It used to render
  permanently, greyed and captioned "saved", with its click guarded by
  `&& self.dirty` — a control that looks pressable and does nothing, which is
  the same class the inspector-slider gate was built for. It cost ~85 px of a
  360 px header for the entire time it could not be used.

With the actions at ~250 px the panel default moved to 420 — its own existing
`max_width`, so no more than a user could already drag to, and the panel is
design-mode only so the chart space is never a real user's. `DESIGN INSPECTOR`
now renders in full. Verified by screenshot.

---


## AT-165 — the second red-CI workflow, and three ratchets that were lying

AT-164 recorded that CI had been red for a whole session while local runs said
"all gates pass". The fix was `dev/run_all_gates.sh`. It was not sufficient, and
the way it failed is worth more than the fix itself.

`run_all_gates.sh` derived its list from `design-system-check.yml` and
cross-checked itself against that same file. It was complete, self-verifying,
and blind: `Quality Gates` is a **different workflow file**, and it was still
red. A checklist cannot report a gap in the source it was derived from. The
script now enumerates both workflows and runs the build matrix behind `--full`.

Three separate defects came out of the two failing jobs.

**1. `--no-default-features` had not compiled since 2026-07-05** (six weeks,
commit `2692ce9b`). `drawing_ppp` and `_drawing_c32` are defined under
`#[cfg(feature = "gpu_chart_v2")]` but passed to `render_order_lines`
unconditionally, so the legacy egui render path referenced values that did not
exist in that configuration. Fixed by ungating the two definitions — a float
read and an uncalled closure. This is chart-engine code and the chart engine is
sacred, so the change is a cfg correction and nothing else: no restructuring, no
migration, no design system anywhere near it.

**2. `expect_total` 63 vs baseline 61 — real, and mine.** `Grid::solve` and
`Flex::solve` called `.expect()` on four Taffy calls each. Every Taffy error
variant describes a caller mistake that cannot occur for a tree built and
dropped inside one function, which is a good argument for the panic being
unreachable and **no argument at all** for writing a panic into a trading
terminal's layout path. Both now short-circuit with `.ok()?`.

The fallback shape mattered more than the conversion. Returning an empty `Vec`
would have compiled and would have been wrong: `Flex`'s two-pass measure path
indexes `first[i]` for every hooked child, and several call sites zip the result
against their items. A short vec converts a clear panic here into an
out-of-bounds one function away — strictly worse. The fallback is one
**collapsed** rect per item, so the length is an invariant regardless of
success, and a zero-size rect fails `is_rect_visible` so a failed solve paints
nothing for one frame. `solve_arity_tests` in both files locks the arity under
zero, negative, infinite and over-subscribed space.

**3. `dead_code_allows` 52 vs baseline 49 — and the gate was counting prose.**
Three of the 52 matches were doc comments that merely *named*
`#[allow(dead_code)]` while explaining one. This is AT-154 for the third time in
this repo — the ratchet counted test fixtures, the cascade ceiling counted
comments describing migrations it had already completed, and now this. The
direction is always the same: the gate reports work that does not exist, so the
only way to make it pass is to stop writing the explanation. A gate that
penalises documenting itself is worse than no gate.

Fixing the regex was not the end of it, and this is the part that would have
been easy to skip. With comments excluded the count dropped to 49 — exactly the
baseline — and the gate went green. That would have been a false pass: the
baseline `49` had itself been measured with the buggy regex and included two
prose lines, so the honest comparison is 47-then against 49-now. There was a
genuine `+2` underneath, and the regex fix was about to bury it.

Both were mine, both in `builtin_recipes.rs`, both captioned "authoring
vocabulary". Removing them showed **one** actually suppressed anything:
`SpecExt::on_press`, unused by all nine built-in styles, kept alive by an allow
captioned "no CSS counterpart yet" where "yet" had been true for the trait's
entire life. That is `sx::recipes` in miniature — vocabulary written for a
caller that never arrived, with an allow making its deadness invisible. Deleted
the setter; the `active` field it wrote is untouched and still resolved by
`apply_over`. The other allow suppressed nothing and is simply gone.

Result: 47, below the corrected baseline, on the merits rather than on a
re-measurement. `#[allow(dead_code)]` on `pub mod cascade` also came off — the
module now has real consumers and `cargo check` reports nothing unused in it,
which is the first independent evidence that the cascade layer is not the thing
it was built to avoid becoming.

**The pattern across AT-164 and AT-165.** Every instrument in this session has
erred in the direction that flatters the work — the cursor-walk census four
times, the containment model, now the dead-code ratchet. A number that agrees
with you is the one to re-derive.

---

## AT-166 — two declarations that did nothing, and a harness that measured nothing

Three findings, each one found by asking a question the passing tests could not
answer.

**1. The element tree does not paint anything.** The adoption floor read
"El 99" and looked healthy. Broken down by node kind: 73 `slot`, 17 `row`,
9 `column`, and **0 `text`, 0 `button`, 0 `spacer`**. Zero production trees call
`show`/`show_in`. The tree is being used as a layout solver — which is real
work, and is not what it was built to be. Its component half (`El::text`,
`El::button`, `El::spacer`, and the entire `paint()` function) has no consumer
outside tests.

That is `sx::recipes` again, and the adoption gate cannot see it because it
counts `El::slot` identically to `El::text`. A floor that any node satisfies is
a floor against abandonment, not against hollowness.

**2. `text_align` and `letter_spacing` were inert.** Both sit in `Inherited`
with builders (`.align()`, `.letter_spacing()`), a module docstring explaining
that CSS inherits them, and a test asserting the fields exist. Neither was read
anywhere. A caller could declare a right-aligned subtree and get left-aligned
text with no error, no warning and no failing test — the "familiar API that
lies" that `context.rs`'s own docs warn against, shipped in the file that warns
about it.

`paint` now honours both, and `intrinsic()` accounts for tracking (measuring
without it would size a slot for untracked text and then paint tracked text
into it, misplacing every right-aligned sibling). Guarded by tests that assert
on OUTPUT — painted anchor x, measured width — because a test on the fields
would have passed the whole time. Both were mutation-checked: reverting either
consumer fails them.

**3. The test harness measured all text as 0 px wide.** `egui::__run_test_ui`
runs one frame, and a context has no font atlas on its first, so
`layout_no_wrap` returned width 0 for every string. Probed directly:
`raw_layout_width=0` for "WIDE TEXT". Every width-dependent assertion in
`element.rs` was comparing zeros to zeros — a row of text nodes laid out as if
all its children were empty, and the tests agreed.

`label.rs::truncate_tests` already had the fix and had had it for a long time:
run one throwaway frame, then measure in the next. Hoisted into
`cascade::element::tests::run` and `panel_section`'s harness. Text now measures
53.875 px where it measured 0.

**Nothing failed when the harness was fixed** — all 1163 tests still pass. The
assertions were weaker than they looked rather than wrong, which is the
uncomfortable version: there was no failure to find, and no failure to warn
anyone. `the_harness_can_actually_measure_text` now asserts the atlas exists,
so a revert to the single-frame harness fails loudly instead of going quiet.

**The pattern, third entry running.** AT-164: local gate runs were complete
against a list derived from one workflow. AT-165: the dead-code ratchet counted
prose. AT-166: the adoption floor counts placeholder nodes, and the harness
measured nothing. Every instrument built this session has erred toward
flattering the work, and every one was caught by measuring the instrument
rather than reading its output.

---

## AT-167 — the component half was unreachable, not unwanted

AT-166 recorded that 73 of 99 `El` nodes were placeholder `slot`s and that
`El::text`/`button`/`spacer` had no production callers. The obvious reading was
neglect. The actual reason was structural, and it took three attempts at
adoption to see it.

`show_in` requires a `&mut Ui`. **Most of this app paints from a bare
`&egui::Painter`** — every chart-overlay panel (`draw_positions_panel`,
`draw_trade_plan`, `draw_risk_dash`, `draw_zone_strength`), the pane chrome,
several list rows. Those surfaces could reach `solve_rect` and nothing else, so
the component half was not reachable from the majority of its intended callers.
A system that cannot be called is not adopted for the same reason a system
nobody wants is not adopted, and the adoption number looks identical.

Three additions, in the order the blockers appeared:

1. **`El::text_with_font`.** `El::text` accepts only `TextStyle` tiers; every
   existing widget paints with an explicit `FontId` (`prop_at(font_xs())`,
   `mono_at(size)`, an icon font). Pixel-locked chrome cannot be re-tiered as a
   side effect of moving its layout. CSS has the same escape — `font-family` is
   a value, not only a class.

2. **Clipping and a cascading `.color()`/`.align()`.** `show_in` now clips to
   its rect, as every widget it replaces does through `painter_at`. `.color()`
   is sugar for a `style()` delta and INHERITS, so a row states its colour once.

3. **`El::show_with(painter, theme, rect)`.** The one that mattered. Internally
   the tree's font context became a `Measure` enum — `Ui`, `Painter`, or
   `None` — replacing an `Option<&Ui>` whose two states were "measure" and
   "collapse text to zero". `Painter::fonts` lays text out, and
   `TextStyle::font_id()` already resolved a tier without a `Ui`, so the only
   real losses are per-context tier OVERRIDES (which `text_with_font` sidesteps)
   and buttons (which need `interact`). An `El::button` in a painter tree
   records its rect, paints nothing, and trips a `debug_assert` — a node that
   silently draws nothing is the fail-silent class this codebase has eleven
   documented forms of, and it was not going to be the twelfth.

Both paint paths now go through ONE `paint_text`. Two copies of "how a declared
property becomes a glyph position" is how `text_align` gets ignored again.

Result: self-painting nodes 0 → 15, across `SelectableRow`, `ticker_strip`,
`draw_zone_strength`, `draw_trade_plan` and the positions-panel column headers.

**One migration was declined, and the reason belongs here.** `PanelKeyValueRow`
looks like an ideal adopter — label left, value right, optional meta. It has a
pure `solve_key_value_row` with five exact-pixel geometry tests and no external
callers, so replacing it was available. It was not done: an `El` tree measures
text through a font stack, so those five assertions would have had to become
font-metric-dependent, and a widget's layout tests would get worse in exchange
for a number going up. Adoption that makes the tests worse is adoption theatre.

**A note on the empty statement.** The positions-panel column header ended with
`p.text(pos2(right, y + 4.0), RIGHT_CENTER, "", …)` — painting no glyphs and
reserving no space. It had been there long enough to look intentional. The tree
states the same intent with a spacer that does something.

---

## AT-168 — the design-system doc described a directory that had moved

`docs/DESIGN_SYSTEM.md` is the entry point for anyone working on this system.
It carried a snapshot date of **2026-05-05**, described the system as spanning
"three Rust modules", and its file map pointed at
`src-tauri/src/chart_renderer/ui/widgets/` — a directory that had become
`ui_kit/widgets/`. Twenty-eight of its paths did not resolve. It said nothing
about tokens-vs-recipes-vs-layout-vs-cascade-vs-elements, the five layers that
now exist, and nothing at all about the declarative layer this whole effort was
for. A developer following it would have gone looking in a directory that was
not there, and the only way to find out was to try.

Rewritten: a **layers** table (tokens → recipes → layout → cascade → elements),
an **authoring model** section covering declare-don't-walk, what inherits and
what does not, the three entry points (`show_in` for a `Ui`, `show_with` for a
bare `Painter`, `solve_*` mid-migration), and `intrinsic_width` for
measure-before-place. File map regenerated from the tree; every stale widget
path remapped to where the code actually lives, with the one file that no
longer exists anywhere (`perf_hud.rs`) said to be gone rather than pointed at.

**`dev/doc_accuracy_gate.py`** now checks it. Paths must resolve; every builder
method chained on an `El` tree in an example must exist as a `fn`. It cannot
check whether the prose is TRUE — a doc can give the wrong reason for a correct
API and pass — so it bounds the rot rather than eliminating it, and says so.

**The gate passed its own first mutation test, which was the point of running
one.** Version 1 anchored paths on a `src-tauri/src` prefix and therefore
checked none of the layer table, which is written crate-relative; and it looked
only for `Type::method(`, which misses every chained builder call — the exact
thing that rots in a fluent API. A deliberately corrupted path and a renamed
`.child_if` both sailed through. It also had to be scoped per-STATEMENT rather
than per-fence, because the anti-pattern blocks put a ❌ and a ✅ together and
the ❌ half's `painter.galley(` was being attributed to `El`.

That is the fifth instrument this session to report success on work it was not
looking at, and the second where the check was written, run, seen to pass, and
only found hollow because it was deliberately fed a lie. The passing run is not
the evidence. The failing run on known-bad input is.

---

## AT-169 — eight implementations of two colour operations

The standing rule for this design system is "there should be only a single one
and all the others removed". Colour arithmetic had **eight** implementations of
**two** operations, and every one of them compiled, rendered, and looked right.

**Multiply the RGB channels — four copies:**

| Where | Alpha | Rounding | Clamp |
|-------|-------|----------|-------|
| `interaction::brighten_color` | preserved, premultiplied | truncate | `min(255)` |
| `chart…style::darken` | preserved, premultiplied | truncate | via `1-amount` |
| `chart…style::color_shade` | STAMPED, unmultiplied | truncate | `clamp(0,255)` |
| `gpu::indicator_default_color::brighten` (a local `fn`) | DROPPED (`from_rgb`) | **round** | `min(255)` |

**Lerp two colours — four copies:**

| Where | Alpha | Rounding |
|-------|-------|----------|
| `widgets::motion` | lerped, premultiplied | round |
| `sx::style` | lerped, UNmultiplied | round |
| `overlays::kit` | ignored, forced opaque | **truncate** |
| `chart_widgets` | already delegating to `motion`, then `.to_opaque()` | round |

Consolidated onto `ui_kit::style::scale_channels` and
`widgets::motion::lerp_channels`. Everything else is now a thin wrapper stating
its own alpha rule over shared arithmetic — which is what the `chart_widgets`
copy already was, and the shape the other six should have had.

Two behaviour changes, both stated rather than buried: `gpu`'s indicator
palette and `overlays::kit`'s ramps now ROUND where they truncated, moving a
channel by at most 1/255, and `gpu`'s preserves alpha instead of forcing
opaque (a no-op in practice — theme roles there are opaque).

**`overlays::kit::lerp_color` carried `#[allow(dead_code)]` captioned "callers
migrate off chart_widgets' local copy".** It had three callers in
`viz/charts.rs`, so the allow suppressed nothing; and `chart_widgets` had
already migrated — to `motion`, not to it. A stale note on a stale allow,
describing a migration that went somewhere else. Third inert
`#[allow(dead_code)]` found this session.

**`ui_kit::layer_guard` caught the first version of the tests.** They asserted
the equivalence from `ui_kit/style.rs`, which meant ui_kit naming
`chart_renderer` — the dependency runs one way and the guard said so. Split:
ui_kit asserts its own half, the chart side asserts its wrappers against the
primitive. That guard earned its keep.

**And a test found a bad assumption of mine, not a bad function.**
`color_shade_shares_the_channels` compared `.r()` off a shade stamped with
alpha 120 against `color_scale`'s, and failed 80-vs-55.
`Color32::from_rgba_unmultiplied` premultiplies on construction, so the
readback is scaled by 120/255 — the encoding doing exactly its job. Asserted at
full alpha instead, with the stamping checked separately.

`hand_colour_math` is a new ratchet at **zero**: not "few open-coded scales are
acceptable" but "there is one implementation and everything else wraps it".
Mutation-tested — re-introducing a single `c.r() as f32 * k` fails it.

---

## AT-170 — five renderings of the same number, in a trading terminal

Follow-on from AT-169's colour sweep: the same "one operation, N
implementations" audit run over numeric formatting. Five compact-number
formatters, none agreeing.

| Where | `1_234_567` | `4_500` | `-1_234_567` |
|-------|-------------|---------|--------------|
| `trading::fmt_notional` | `$1.2M` | `$4.5K` | `$-1234567` (no compaction) |
| `command_palette::human_volume` | `1.23M` | `4.5K` | `-1234567` |
| `bottom_dock::money` | `$1.23M` | `$4.5K` | `$-1.23M` |
| `portfolio_pane::fmt_money` | `1.23M` | `4500` | `-1.23M` |
| `scanner_panel::fmt_volume` | `1.2M` | `4K` | n/a (`u64`) |

This is not a tidiness finding. A trader reading a P&L in the bottom dock and
the same figure in the portfolio pane saw `$1.23M` and `1.23M`; a volume in the
scanner and in the command palette read `1.2M` and `1.23M`. The design system's
central claim is that a value looks the same wherever it appears, and the five
formatters broke it in the place a trader looks hardest.

Two of the five thresholded on the RAW value rather than `abs`, so a negative
figure printed in full: `-1234567` where its positive twin showed `-1.23M`.
`portfolio_pane` alone used a K threshold of **10_000**, so `4_500` rendered
`4500` there and `4.5K` everywhere else — with no stated reason.

None of the five handled non-finite input. A live feed hands a panel `NaN` on
reconnect and all five printed "NaN" straight into a P&L cell, which looks like
a number. `foundation/num_format` renders an em dash.

Consolidated to one core with three named renderings that differ where the
DOMAIN differs rather than where the author did: `money` (currency symbol, two
decimals at M), `plain` (same, no symbol — for panels that label currency in a
column header), `volume` (reaches B, one decimal, because a share count's
second decimal is noise). Plus `signed_money`, which also fixes `$-1.2M`
rendering as `-$1.2M`.

Every call site's visible change is written at the delegation, not buried in a
diff.

**The duplicate-function sweep that found this also produced a negative
result worth recording.** A census of every `fn` name defined in three or more
files surfaced `show_ctx` (×40), `disabled` (×17), `selected` (×11),
`subscribe_bars` (×9) and similar — all per-type builder methods and trait
impls, which is what those SHOULD be. `now_ms` (×10) looked like a tenth copy
of a clock and turned out to be ten thin shims over one
`foundation::time::now_ms`. Colour and number formatting were the only real
duplications. The sweep is worth keeping as a periodic check, not as a gate.

---

## AT-171 — the most repeated defect class, finally asserted

"Two mechanisms compute the same value and are free to disagree" is this
codebase's most repeated defect. The instances on record:

1. The pane header's pinned `ICON_BTN_W_LAYERS = 60.0` vs the real label width
   — "LAYERS" overran on a wider face, the next button painted over it, the
   divider landed mid-word. This is what `measure_content_w` was extracted for.
2. The tab strip's fit test and paint loop disagreeing by 1px per tab.
3. `spreadsheet_pane`'s three spellings of a column offset.
4. `panel_list_row`'s `btn_gap` in the width formula and again in the advance.
5. `painter_pane`'s `gap_sm()` written twice around the +Tab affordance.
6. Four multiplicative colour scales and four colour lerps (AT-169).
7. Five compact-number formatters (AT-170).
8. `kit.rs`'s `TAB_GAP` vs a bare `+ 1.0` in `painter_pane`.

Every one was found by reading, never by a failing test — because each half is
correct on its own and only their RELATIONSHIP is wrong.

`Button` is the widget where this class first bit, and it had no test for the
contract. `fit_paint_agreement_tests` asserts it directly: **at intrinsic
width, everything the button paints lies inside the button, and no two painted
runs overlap** — across leading icon, label, kbd hint, trailing icon and their
combinations.

The trailing icon is the interesting case and the reason the test covers it
specifically. It is painted **flush right, outside the element tree**, at
`rect.right() - pad_x`, while `measure_content_w` reserves its width **inside**
`content_w` — which is also what centres the lead/label/kbd block. Those two
facts have to agree and nothing said so. They do agree; the arrangement is
correct; it was simply undefended.

**Mutation-tested, and the mutation revealed which half of the test matters.**
Dropping the trailing icon's contribution from `measure_content_w` left
`nothing_paints_outside` PASSING — the icon is anchored to the right edge, so
it stays inside the now-too-narrow button. Only `painted_runs_never_overlap`
caught it, reporting the label and the caret occupying the same pixels. A
containment check alone would have been the sixth instrument this session to
pass on broken input.

---

## AT-172 — I duplicated a test harness four times while consolidating duplicates

While AT-169 and AT-170 were merging four colour scales, four colour lerps and
five number formatters, the tests proving those merges were being written by
copy-and-paste. The same twenty lines — build a `Context`, run a throwaway
frame so the font atlas exists, run a second frame with the widget in it, walk
the layer's shapes, pull the text runs out — appeared independently in
`cascade::element` (three times), `widgets::kv_row`, `widgets::selectable_row`
and `widgets::button`. Six copies. Test code is not exempt from the rule it is
being used to enforce.

`widgets::paint_probe` is now the one harness: `probe(f) -> Vec<Run>` with
`{left, right, color}` per painted text run, plus `assert_contained`,
`assert_no_overlap` and `assert_atlas_is_built`. Every copy is gone, including
the two-frame dance in `panel_section` and the one in `label.rs` — which is
where the font-atlas fix was FIRST written, long before anything else needed
it, and where it stayed local while `cascade::element` rediscovered the problem
the hard way by measuring every string as 0 px for its entire life (AT-166).

Consolidating it also captured two mistakes worth not making twice: read a
galley's colour from `job.sections`, not `override_text_color` (`Painter::text`
bakes it into the layout job); and assert overlap separately from containment,
because the `Button` mutation that broke the fit contract left containment
passing (AT-171).

**Then the design-system ratchet flagged the new file.** `paint_probe.rs` is
declared `#[cfg(test)] pub mod paint_probe;`, so every line in it is test code —
but nothing INSIDE it says so, and `strip_test_hits.py` only knew how to skip
`#[cfg(test)]` blocks within a file. Four `Color32::PLACEHOLDER` sentinels read
as production drift, which is exactly what the cfg(test) exclusion exists to
prevent: a measuring harness must state a literal colour, and "fixing" it by
reaching for a token would make the harness depend on the system it tests.

The documented escape was an `ALLOWED_BASENAMES` entry. That was declined: a
hard-coded basename list is the thing that rots — the next test-only module
gets flagged, someone appends another name, and the list stops describing a
rule. `strip_test_hits.py` now reads the module DECLARATION, where the truth
already lives. It excludes exactly two files repo-wide, `paint_probe.rs` and
`layer_guard.rs`, both genuinely test-only; the ratchet dropped 1066 → 1064
accordingly.

The first version of that fix silently did nothing, for the third time in this
file's history: it was handed git-bash MSYS paths that native Windows Python
cannot open. `_openable` already existed for precisely that, with a docstring
saying it had failed that way before. Routed through it, and the selftest now
covers the new rule — mutation-tested, so breaking the lookup fails loudly
instead of quietly excluding nothing.

---

## AT-173 — the inheritance payoff, measured properly, is small

I said I would redo the cascade-adoption measurement because the first one was
too narrow — it counted only *consecutive same-colour siblings* and found one
run in the whole codebase. The second attempt counted, per function, how often
the same colour expression is passed to a paint call. It found 13 functions and
33 repetitions, which looked like a real opportunity.

**It was wrong, in the flattering direction, for a reason worth naming.** Most
of those repeats were BARE LOCALS — `fg`, `col`, `dark`, `icon_fg`, `text_col`.
A colour held in a local is *already* declared once and used three times. That
is not the repetition a cascade removes; the cascade removes repetition where
each leaf independently RE-RESOLVES the colour (`color_dim(t.dim)` written out
at three call sites).

Filtering to genuine re-resolutions: **17 sites, 21 removable repetitions**,
and 11 of the 17 are in one file. The first narrow measurement was closer to
right than I gave it credit for.

**The conclusion is a decision, not a number.** Inheritance is not where the
value of this system lies, and chasing `cascade::scope` adoption for its own
sake would be adoption theatre — the same mistake as counting `El::slot` nodes
and calling the component tree adopted (AT-166). What inheritance IS: a free
consequence of putting paint through the tree. `El::row().color(x)` covers
every child, so every surface migrated picks it up without a separate effort.

So the plan changed to match: migrate surfaces, take the cascade with them.
`draw_latency` is the clearest case — four resolutions of `color_dim(t.dim)`
became one declaration on the column, with only the frame-time value (the one
thing that varies with state) stating its own.

Migrated in this pass: `draw_latency`, `draw_custom`, `draw_confluence`,
`draw_options_flow`. El nodes 126 → 146, self-painting 28 → 43.

**Two pinned-inset relationships fell out.** `draw_confluence` put its count
badge at a bare `left + 48.0` while its price started at `left + 6.0` — a 42px
column width written nowhere near the thing it was clearing. `draw_options_flow`
anchored its value at `right - 30` and its flow type at `right - 6`, two pinned
insets that had to stay clear of each other by hand. Both are sibling
relationships now.

**And a census of what is left, so the remaining count is not read as a
backlog.** Twelve hand-painted label/value pairs remain, and most are not
migration candidates:

* `screener_heatmap::paint_cell` and `dom_panel`'s tape rows — per-cell and
  per-row hot paths. Same measured-cost exemption as `dom_row`/`watchlist_row`.
* `draw_tape_speed` (gauge end-labels), `draw_news_ticker` (dot + headline),
  `alert_feed::render_badge_feed` (a clipped scrolling message, an ellipsis and
  a dismiss ×) — matched by the regex, none of them a label/value row.
* `button`, `input`, `select` internals — widget-internal layouts with their
  own reasons.

Genuinely open: `msg_tension_panel::draw_ladder`, a three-column row with a bar
in the middle. Everything else in that list should stay as it is.

---

## AT-174 — text width guessed from a character count

Six surfaces sized text by multiplying its LENGTH by a pixel constant:

```rust
let qty_pill_w  = (qty_label.len() as f32 * 6.5 + 10.0).max(30.0);
let w           = (label.len()     as f32 * 6.5 + 16.0).max(48.0);
let mut ind_x   = sym_x + symbol.len() as f32 * 8.5 + 6.0;
let text_size   = Vec2::new(text.len() as f32 * st::font_sm() * 0.6, …);
p.text(pos2(hdr.left() + 24.0 + label.len() as f32 * 7.0 + 6.0, …), "🔒", …);
```

Wrong twice over. **Immediately**, for any proportional face — `W` and `i` are
not the same width, so two labels of equal length need different space, and the
constant can only ever be an average. **Eventually**, for every face, because
each constant was measured once against whatever font was current the day it
was written. That second failure is the pane-header defect that motivated
`Button::measure_content_w`: a 60px slot sized for a font that had since grown,
overrun by "LAYERS", with the next control painted on top of it.

All six now measure, with the same font the paint uses. egui caches galleys, so
measuring a string you are about to paint is a hash lookup rather than a
shaping pass — in `watchlist_row` the symbol was laid out one line above to be
drawn.

`ui_kit::style::measure_with_painter` is new: the painter-only twin of
`measure_with`. Its absence is *why* these guesses existed — most of this app's
chrome paints from a bare `&Painter` and had no way to measure at all.

`overlay_card_header` is the one fixed by declaring rather than measuring: the
lock glyph is now a sibling after the label, so it follows the label's real
extent, and the pinned `24.0` icon column that had to agree with the icon's
actual width by hand became a fixed first child.

**`watchlist_row` was fixed despite being exempt, and the distinction matters.**
Its exemption is on measured per-row cost and covers LAYOUT ARITHMETIC — it was
never a licence to guess a text width. An exemption from a migration is not an
exemption from correctness.

**The ratchet took three attempts and the first two under-reported.** Count ×
pitch (`rows.len() as f32 * 30.0`, a stack height) is correct and cannot be
told from a width guess numerically, so it is told apart by NAME. Version one
excluded any identifier ending in `s`, which silently dropped `qty_s`, `not_s`,
`status_s` and `price_s` — all strings, all exactly what the gate is for. A
ceiling that under-reports lets the thing through. Version two matched suffixes
only and missed the bare `lines` in `tooltip.rs`, where the usage is legitimate.

Baseline 17, and the number should be read carefully: **in-scope UI and chrome
is at ZERO**. Every remaining site is in `render/pane/core.rs` (the chart
engine, out of scope by standing directive) or `tps_overlay.rs` (a deliberate
Excel pastiche). The ratchet only falls, so a new guess anywhere fails it —
mutation-tested.

---

## AT-175 — five conditionals whose branches were identical, and one was a real bug

Found while migrating `heatmap_pane`: `if cell.change_pct >= 0.0 { t.text }
else { t.text }`. A census turned up five:

| Where | Written as | What it was |
|-------|-----------|-------------|
| `foundation/monitoring.rs` | `if filled >= RING_SIZE { i } else { i }` | **a real bug** |
| `chart_widgets`… `welcome.rs` | `if self.step == 3 { Primary } else { Primary }` | lost intent |
| `dom_row.rs` | `else if price > 0.0 { fg } else { fg }` | vestigial |
| `heatmap_pane.rs` | `if change_pct >= 0.0 { t.text } else { t.text }` | vestigial |
| `compute.rs` | `if price < 25.0 { 1.0 } else { 1.0 }` | vestigial rung |

**The monitoring one is a defect, not dead code.** `subsystem_stats` walks a
300-entry ring; the identical branches meant it always used the raw index, so
once the ring wrapped it read the frames out of chronological order. Sums,
maxima and counts are order-independent, which is why nothing looked wrong —
but `e.2 = *us; // last` records the LAST value seen, and "last" is whatever
the iteration order says. For every session longer than about five seconds at
60fps, the "last" column of the subsystem profiler reported whichever frame
happened to sit at the highest raw index rather than the most recent one.

Both identical branches are what makes the intent legible: somebody knew the
wrapped case needed a different index and the expression never got written.
Fixed by starting at `subsystem_ring_pos`, which is the next write slot and
therefore the oldest entry once full. `ring_order_tests` asserts it through the
public shape rather than the index arithmetic — mirroring the arithmetic would
have agreed with the bug — and is mutation-tested against the original.

**The other four were collapsed, not "fixed", and the difference is the
point.** Each could have been read as a missing second value and filled in with
a guess. None were:

* `welcome.rs` — `Primary` is right for both. "Next" and "Get Started" are each
  the one action their step asks for; inventing a `Secondary` would change how
  the wizard reads on no evidence.
* `heatmap_pane` — NOT restored to bull/bear. The cell FILL already encodes the
  sign, so tinting the label would say it twice and fight the fill for
  contrast. The label's job is to be readable on that fill.
* `dom_row` — a rung with a non-positive price is not a state the ladder
  renders; there was no second colour to restore, only a test to remove.
* `compute.rs` — no evidence of an intended third strike interval, and
  inventing one would change simulated strikes on a guess.

Neither kind survives review as written, and neither fails a test: both
branches compile and the result is reasonable. Only a census sees them, so
there is now a ratchet at zero — mutation-tested.

---

## AT-176 — layout solved repeatedly to re-derive a constant

`spreadsheet_pane` (AT-«cursor-walks») had three spellings of a column offset,
one of them nested inside a per-row loop. Looking for the same shape elsewhere
found two more, and one of them is the largest single cost in this session.

**`watchlist_panel::col_at`** ran a full Taffy solve on **every call**, and it
is called five to seven times per row. A 30-row option chain therefore paid
~150 solves a frame to place columns that are identical for every row.
`flex.rs::report_solve_cost_for_a_typical_row` times a solve at 5.5 µs in
release, so that is roughly **0.8 ms per frame — about 5% of a 16.7 ms
budget — spent re-deriving a constant**. Solved once, indexed thereafter.

**`kit.rs::solve_tabs`** ran its solve inside a closure whose only varying
input, `start_x`, is used solely by the translate afterwards. The closure is
called at least three times per panel header (twice by `fitting_tabs`, probing
with and without the ordinal prefix, and once to paint), so every header
re-derived the same tab ladder three times a frame. The solve is hoisted; the
closure now only translates.

`rail_layout`'s per-column solve was checked and left alone: each column has
different member heights, so that one is genuinely per-item.

**This retires the objection behind two exemptions.** `dom_row` and
`watchlist_row` are exempt from the element-tree migration on measured per-row
cost — the argument being that a solve per row is too expensive to replace a
few multiplies. That argument was always about solving PER ROW. Solving once
for the grid and indexing, which is what `spreadsheet_pane` and now
`watchlist_panel` do, costs one solve regardless of row count. The exemptions
stand for now because the rows also carry hard-won painting, but the cost
reason for them is gone and should not be cited again.

Also in this pass: `msg_tension_panel::draw_ladder`'s three-column split is
declared. Its `att_x0` was
`area.left() + label_w + gap_sm() + bar_w + gap_sm()` — the same walk as
`bar_origin_x` restated with one more term, free to disagree the moment
`label_w` or the seam changed. Only the COLUMNS were declared: the band, the
target marker and the labels that sit outside whichever edge the move lands on
are data geometry positioned by price, and stay.

---

## AT-177 — the chart engine, opened for migration, had paint/hit-test disagreements

The standing directive was that the chart engine is sacred. That was widened to
"fix and migrate layout too", so `render/pane/core.rs` came into scope. What
was expected: guessed text widths. What was actually there: **three surfaces
whose paint pass and hit test computed the same geometry independently, and
disagreed.**

**The order badge** — `[B/S] [QTY] [notional] [STATUS] [SEND?] [X]` — had its
width in THREE places: the paint pass, the badge-button hit test, and the
order-line drag hit test. The third carried the comment "Recompute badge
geometry to match rendering". Both hit tests hard-coded the status cell as
`if is_draft { "DRAFT" } else { "LIVE" }` while the paint pass renders the full
lifecycle (`SEND…`, `CXL…`, `MOD…`, `PART`, `FILL`, `??`), so for any order not
exactly Draft or Working the clickable region was a different width from the
badge on screen: X and SEND sat off their targets, and the drag-exclusion zone
did not match the badge it excluded. One `order_badge_metrics` now.

**The oscillator label pill** was worse, and in a way a width fix would have
hidden. Paint and hit test disagreed on WIDTH (measured vs `len() * 6.0`) and
on **Y**: the hit test stacked each pill by 16px, the paint drew every pill at
the same `osc_top + 2.0`. Oscillators share one region, so with two of them the
labels were painted on top of each other — unreadable — and the second's click
target sat 16px below, over nothing. The stacking is plainly the intent and the
paint side never got it. One `osc_label_rect` now, and the labels stack.

**The `TextNote` hit box** guessed `text.len() * font_size * 0.5` — half an em
per character — while the paint pass, ten lines away, already measured properly
with `layout_no_wrap` for its selection rect. A note reading "IIII" was
clickable well past its glyphs; "WWWW" stopped being clickable before its end.

**The near-miss is the lesson.** Converting only the paint sites to measured
widths — which is exactly what "fix the guessed widths" looks like from outside
— would have left three hit tests computing stale formulas against geometry
that had moved. I would have turned a consistent-but-wrong pair into an
inconsistent one, and nothing would have failed. Each of these was found by
asking "who else computes this?" before changing anything.

Also declared: the replay bar, which walked a `cx` through a
`|ui, cx: &mut f32, ..|` closure that advanced the caller's cursor as a side
effect of drawing a button, and guessed two of its widths.

`text_width_guess` 17 → **2**, and both survivors are in `tps_overlay.rs`,
exempt by directive as a deliberate Excel pastiche. The chart engine is clean.

---

## AT-178 — the last two exemptions retired, and why the cost argument was wrong

`dom_row` and `watchlist_row` were exempt from the element-tree migration on
MEASURED per-row cost: `flex.rs` times a solve at 5.5 µs, a DOM ladder paints
~40 rungs a frame, so a per-row solve is ~0.22 ms to replace three multiplies.
The arithmetic was right. The framing was wrong, and it took AT-176 to see it.

**The cost argument was always about solving PER ROW, and per-row was never
required.** `dom_panel` already builds one `ColumnLayout` for the whole ladder
and hands it to every rung, so `dom_row`'s hot path never needed a solve at
all — its only walk was in `show`, which paints a single row. `watchlist_panel`
was the opposite: `col_at` ran a *full solve on every call*, five to seven times
per row, ~150 solves a frame, about 5% of a 16.7 ms budget re-deriving a
constant. The exemption was protecting the file that solved once and had
nothing to say about the file that solved 150 times.

Both are now in scope and both are at zero.

**One genuinely per-row solve remains, and it is stated at the call site rather
than hidden in the gate.** `watchlist_row`'s middle columns depend on
`(s.applicable)(&item_data)` — a row with no ATR shows a different set — so
there is no single ladder to hoist. It pays ~5.5 µs × visible rows, roughly
1.3% of a frame. If that budget is ever needed back, the fix is a cached solve
keyed on the applicable-set bitmask, not a return to walking.

That migration also closed a small disagreement the walk contained: the fit test
`x + w > middle_right` used an `x` that already had the previous column's gap
added, so the test and the advance disagreed about whether a trailing gap
counts. Deciding which columns fit, then placing them, makes it one statement.

**Cursor walks across UI, chrome AND the chart engine are now zero.** The only
remaining exemption is `tps_overlay.rs` — a deliberate pastiche of Excel's
chrome, whose literals are Excel's and whose own comment says so, kept out by
directive because migrating it would make it a worse imitation.

---

## AT-179 — two widgets with no callers, and a census that had to be rebuilt twice

A survey of every widget in `ui_kit/widgets/` and `ui/lists/rows/` for
production consumers found two with none:

* **`AlertRow`** (155 LOC) — superseded, not unfinished. `alerts_panel`'s own
  module docs say it uses "`ui_kit::PanelListRow` for alert rows", and it does.
  The panel migrated to the design-system widget and this was left behind.
* **`OptionChainRow`** (147 LOC) — outgrown. The chain UI is `render_block` in
  `watchlist_panel`, which carries strike modes, NMF toggles, saved options,
  select mode and per-contract subscription publishing. `OptionChainRow` models
  bid/ask/volume/strike/delta. It cannot be wired without being rewritten into
  a different widget, at which point it is not this one.

Both were checked against the standing rule — delete what is genuinely useless,
but re-examine anything unconnected through oversight or unfinished work. One
was superseded and one was outgrown; neither is a wiring away from use.

**Both carried a file-level `#![allow(dead_code, unused_imports)]`,** which is
why 302 lines of unreachable widget sat in the tree without a single warning.
That is the fourth allow this session found hiding the thing it was suppressing
(after `builtin_recipes`, `overlays::kit::lerp_color`, and the blanket one on
`pub mod cascade`). A file-level allow is the most effective of them: it hides
the whole file.

**The census was wrong twice before it was right, in the usual direction.**
Version one took the FIRST `pub struct` per file and looked for `Type::new` —
so it reported `ModalResponse`, `PortableTheme`, `Run` and 28 others as unused,
because it had picked return types and helpers rather than widgets, and because
`PortableTheme` is built with `::dark()`. Version two checked every `pub struct`
in a file and any mention of it, which cut 31 candidates to 4 — but still
flagged `motion` and `text_engine`, which are alive through free functions
(`motion::lerp_color` has 27 consumers) rather than their structs. Only the
third reading was true. A census that reports 31 deletions when the answer is 2
does not get acted on; it gets ignored, which is worse than not running it.

Design-system ratchet 1064 → 1048.

---

## AT-180 — the dead-code ratchet was blind to the strongest form of suppression

AT-179 deleted two widgets that had sat unreachable in the tree without a single
warning, because each carried a file-level `#![allow(dead_code, unused_imports)]`.
That prompted the obvious question: how many more are there, and does the gate
that exists to measure suppression see them?

It did not. `DEAD_RE` was `#\[allow\([^)]*dead_code` — and `#![allow(` has `!`
between the `#` and the `[`, so it never matched. The ratchet had been reporting
**46** item-level allows while **41 file-level** ones stood beside it, uncounted.
The blind spot was precisely the more powerful form: an item-level allow hides
one item, a file-level allow hides a file.

**Measured, not assumed — and then re-measured, because the first measurement
was wrong in the flattering direction.** Stripping all 41 and compiling surfaces
316 warnings. I first reported "~68 named dead items" from that. That number
counted every dead-code warning in the build, most of which appear WITHOUT the
allows too — dead code in a file with no allow already warns. The honest figure
is the DELTA between the stripped build and the normal one:

> **9 files, 16 items** are genuinely hidden by the remaining file-level allows.

Sizing a problem at four times its real size is the same failure as every other
instrument in this session, just pointed at a scarier number rather than a
flattering one. Corrected here rather than left standing. See AT-181 for what
the 16 turned out to be — including one whole panel that cannot be opened.

The baseline moves 46 → 87. That is not a regression waved through — it is the
count becoming true, and it can only fall from here. Mutation-tested: adding one
file-level allow now fails the gate.

**Nine of the 41 suppress nothing at all.** `components/mod.rs`,
`components/text.rs`, `spike_popup.rs`, `state/mod.rs`, `state/codec/mod.rs`,
`ui_kit/text_style.rs`, `hover_card.rs`, `modal.rs` and `shell_variants.rs` each
carried a blanket allow over a file with no dead code in it. Removed; the build
produces zero new warnings in any of them. 87 → 78.

The remaining 32 do suppress something and are a real backlog rather than a
formality — each needs its hidden items read and either used, deleted, or
narrowed to an item-level allow with a reason. That is the shape the two deleted
widgets were found in.

---

## AT-181 — a panel that cannot be opened

AT-180's corrected measurement said the remaining file-level allows genuinely
hide 16 items across 9 files. Reading them found one that is not dead code at
all — it is a feature that cannot run.

**`trade_plan_panel` can never be shown.** Its `open_flag()` starts `false`.
`draw()` returns immediately unless it is `true`. The only functions that can
set it true — `open()` and `toggle()` — are called from **nowhere**, and so is
`close()`. `top_nav` calls `draw` every frame and it returns every frame. There
is no menu item, command-palette entry, keyboard shortcut or button anywhere
that opens it. (`W::TradePlan` in `chart_controls` is the chart OVERLAY widget,
a different surface.)

`provenance_pane` already carries a note about trade_plan_panel APIs "that were
NEVER wired", so the gap was known and never closed.

**Not deleted, and not wired to an invented trigger.** The standing rule is to
delete what is useless but re-examine what is unconnected through unfinished
work, and this is plainly the second. Which control should open it is a product
decision, not one to infer from a call graph. What changed is that the state is
now *visible*: the file-level allow is gone, the three uncallable functions
carry item-level allows naming the reason, and `draw` opens with a comment
saying the panel is unreachable and why.

**`painter_pane` lost six genuinely dead items** — `paint_icon_label_btn`,
`header_fill`, and four constants including `ICON_BTN_ICON_Y_FRAC` and
`ICON_BTN_LABEL_BOTTOM_OFFSET` that only that function used. Worth noting
against my own earlier work: `paint_icon_label_btn` is the function I examined
in AT-173 and decided not to migrate because its three `fg` uses were a
parameter rather than a repeated resolution. That analysis was correct and
irrelevant — the function was dead the whole time, and the file-level allow is
why neither of us could see it.

**`gpu::rgb` was restored after being deleted.** rustc reported it unused, but
it is only TRANSITIVELY dead: its consumers (`CMD_PALETTE_DEFAULT` and friends)
are themselves unused, so removing it broke definitions that are still present.
The chain has to go together or not at all, and deleting a colour table on a
transitive warning is not a passing change. The compile caught it; the reason is
now written at the definition.

**The metric had to be split, because narrowing an allow made the number rise.**
Converting `painter_pane`'s file-level allow into three item-level ones with
stated reasons removed 2 and added 3 — strictly smaller suppression, and the
gate fired on the improvement. `dead_code_allows_file` is now its own ceiling
and should reach zero: a blanket allow over a file is never the right shape,
because it also covers everything added to that file later. An item allow with
a reason is a decision; a file allow is the absence of one.

File-level allows **41 → 30**. Item-level 46 → 49, each addition carrying its
reason.

---

## AT-182 — file-level dead-code allows reach zero

AT-180 found 41 `#![allow(dead_code, unused_imports)]` at file scope, invisible
to the ratchet that exists to measure suppression. AT-181 cleared eleven. This
clears the rest.

**41 → 0.** Total allows 79 → 53, all of them item-level now, and
`dead_code_allows_file` is a ceiling at zero: a blanket allow over a file is
never the right shape, because it also covers everything added to that file
later.

What the 30 remaining were actually hiding, once measured as a delta against the
normal build rather than a raw warning count: **38 items, of which 32 were
unused imports.** The suppression was overwhelmingly import rot, not dead code.
Removing the allows and pruning what they hid touched 64 files.

The six real items were each decided rather than swept:

* `dom_row::price_fmt` — initialised to `"{:.2}"`, never read, no setter.
  Deleted.
* `semantic_label::Theme` — an unused type alias. Deleted.
* `chrome::pane::warn` — KEPT with a reason. It is the middle rung of a
  bull/warn/bear/dim set taken from the theme in one place; dropping it would
  leave the next caller who wants an amber state reaching past the struct, which
  is how the colour sprawl in AT-169 started.
* `screener_build::CatalogEntry::source` — KEPT. It is parsed from the catalog
  JSON, so deleting the field means the deserialiser silently stops reading a
  field the server still sends. Read it or drop it from the contract; do not
  quietly stop parsing it.
* `gpu::rgb` / `rgba_pre` — KEPT. Only transitively dead (AT-181).

**And one lying parameter.** `Dropdown::new(id_salt)` and
`DropdownOwned::new(id_salt)` took an id salt, stored it, and never read it —
`show` delegates to `ui_kit::Select`, which derives identity from `ui` like any
egui widget, so the salt became vestigial when those were migrated to delegate.
Eleven call sites were passing distinct strings ("heat_idx",
"spread_strategy_combo", `format!("group_{pane_idx}")`) in the reasonable belief
that identity depended on them. A parameter that looks load-bearing and is
ignored is the same defect class as a button that looks pressable and does
nothing. Removed from both signatures and all eleven call sites.

**Two instrument corrections on the way.**

The import pruner ran off `cargo check --lib`, which does not compile test
modules, so it removed imports that only tests use — `AlphaTier` in
`recipes.rs` and the whole import block of `gpu.rs::tab_cache_lru_tests`. The
lib built clean and `cargo test` did not. Restored; the lesson is that "unused"
from a lib-only check is not "unused".

The single-system census counted `use` lines as consumers. Removing two dead
`use ...ComponentTheme;` lines dropped it 339 → 337 and tripped the FLOOR — the
gate reporting a cleanup as abandonment of the layer everything migrates to.
Exactly the shape of `flex_rows` firing when a `Flex::row()` became an
`El::row()` (AT-173): a census that counts mentions punishes anything that
tidies the mentions. `use` lines are excluded now, which also means the numbers
finally mean what they say — `ComponentTheme` is 256 real consumers, not 337
mentions of which ~81 were imports.

---

## AT-183 — an orphaned attribute, green on Windows, 175 errors on Linux

AT-182's import prune broke CI, and the way it broke is worth the entry.

`fetch.rs` had:

```rust
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::chart_renderer::{ChartCommand, Bar};
```

The sweep removed the `CommandExt` line — correctly, it was unused — and left
the `#[cfg]` behind. Rust attaches an attribute to the NEXT item, so it
re-targeted onto the `ChartCommand, Bar` import. On Windows the cfg is true, the
import stayed, everything built. On Linux the cfg is false, the import vanished,
and 175 `cannot find type ChartCommand` errors followed.

**Every local check passed.** All three feature configurations —
default, `--no-default-features`, `design-mode` — plus `cargo test` and clippy,
all green, because all of them ran on Windows. `run_all_gates.sh --full` was
built specifically so a local run means what CI means, and it does not: it runs
on one platform and CI runs on another. Any `#[cfg(unix)]` or
`#[cfg(target_os = …)]` branch Windows skips is unverified here. The script now
says so in its own header, because a runner that claims completeness it does not
have is worse than one that admits the gap.

Two other things the prune got wrong, both restored:

* It removed `pub(crate) use core::{render_toolbar, ...}` — a **re-export**, not
  an import. `render_toolbar` is the entry point of the legacy egui render path,
  so it is unused on the default build and essential on `--no-default-features`.
  A `pub use` is API surface; its unused-ness is a statement about the current
  feature set, never about whether it is needed.
* It removed imports used only by `#[cfg(test)]` modules, because it ran off
  `cargo check --lib`, which does not compile tests (AT-182).

Three ways for "unused" to be wrong — wrong platform, wrong feature set, wrong
target — and the sweep hit all three.

**`dev/orphan_attr_gate.py`** now flags any attribute alone on a line and
separated from its item by a blank line. Deliberately narrow: it ignores
`#[serde(default)] pub x: T,` (same line), inner `#![...]` attributes, and
attributes followed by doc comments. A blank line between an attribute and its
item is nearly always the fingerprint of a deleted item. Mutation-tested against
the exact `fetch.rs` shape.

---

## AT-184 — a sub-agent fan-out, and the worst bug of the session

Six read-only census agents were run in parallel, each on a disjoint class, with
instructions to show the search that rules out a consumer and to mark anything
unprovable as UNSURE. Every finding below was verified by hand before acting.

**The headline: indicator Source was ignored on every live bar.**

`recompute_indicators` selects the price series per `ind.source` — Close, Open,
High, Low, HL2, OHLC4 — the user-facing Source control offered for moving
averages, RSI, CCI and Bollinger. `update_indicators`'s INCREMENTAL path, which
runs on every newly-closed bar, hardcoded `.close` in all three of its arms.
And `is_incrementally_extendable()` is exactly `SMA | WMA | EMA` — precisely the
indicators that expose that control.

So an SMA configured on High computed correctly, and then appended
Close-derived values for every bar that closed afterwards. It self-heals on the
next full recompute — a symbol or timeframe switch — which is what makes it hard
to notice: the number is wrong until you change something, then briefly right.

Wrong numbers on an indicator a trader reads. Fixed by extracting
`bar_source_price(bar, source)` as the ONE definition, used both to build the
full recompute's series and to extend incrementally. A multi-timeframe source
now forces a full recompute rather than extending from this chart's bars, which
would have appended values from the wrong timeframe entirely.

**Two text-width guesses my own ratchet could not see.** It required the cast to
be followed immediately by the multiply, so it matched
`tag.len() as f32 * 5.0` and missed `(tag.len() as f32) * 5.0`. `news_row`'s tag
chips used the parenthesised form, and the guess was load-bearing twice — it
sized the chip AND fed the overflow test on the next line, so an under-guess
draws a tag past the row's edge and an over-guess drops one that would have fit.
`table.rs` used it to place the sort glyph after a header label. Both measured
now; the regex accepts an optional closing paren.

That is the finding worth keeping: **the gate was green and the defect was
there.** No amount of re-running it would have helped. A ceiling only holds the
shape it can see, and a second pair of eyes found the shape.

**Also confirmed, not yet acted on** (each verified by hand, listed for the
record):

* `playbook_store::set_author_handle` — the persisting path — is dead. The
  user-facing `AppCommand::SetAuthor` routes to `set_author_handle_mem`
  instead, so the author handle is never written to `author.txt` and is lost on
  restart. The dead function's own doc comment calls itself "the real settings
  path".
* Day-change percent is computed in six places. Four fabricate `0.0` when
  `prev_close` is unknown, painting a confident green `+0.00%` for absent data;
  `fetch.rs:2282` divides by zero unguarded; and `toolnav.rs:104` filters those
  quotes out entirely, documenting the exact rule the others break — "fabricated
  data, not a placeholder". `gpu.rs::get_change_pct` already returns `Option`
  and is the right model. Not fixed in this pass: `change_pct` has ~100
  references and the correct fix changes a field type, which deserves its own
  commit.
* `hotkey_editor.rs:105` — the key-binding pill is a real `Button` whose
  `Response` is dropped, so the obvious click target for rebinding a shortcut
  does nothing; only the small separate "Edit" label works.
* `element.rs` — `paint()` and `paint_with()` retype the same Container layout
  math. The sibling `Kind::Text` arm was already consolidated into one
  `paint_text` for exactly this reason; the Container arm never was. Mine.

**On the fan-out itself.** The agents were useful precisely where predicted:
finding things. Every one of the four reports required verification before
action, and two contained specifics that did not match the code when checked.
The judgment — is this field dead weight or the middle rung of a semantic set,
is this unfinished or outgrown — stayed where it had to. What changed is that
six classes were swept in the time one would have taken.

---

## AT-185 — acting on the fan-out, including the finding that was wrong

Three of AT-184's verified findings actioned, and one corrected.

**The hotkey pill now rebinds.** `hotkey_editor.rs` painted the current binding
as a `Chrome`-variant `Button` — hover-animating like every other control — and
dropped its `Response` on the floor. In a rebinding workflow, the key you want
to change did nothing when clicked; only the small separate "Edit" label worked.

Wired rather than de-affordanced. `.sense(Sense::hover())` would also have
removed the lie, but the key is the obvious thing to click, the action already
existed two lines above, and there is only one thing a click there could mean.
This is deliberately NOT the `trade_plan_panel` case, where wiring would have
required inventing which of several surfaces should open a panel.

The `ui_direct_mutation` ratchet then caught the fix adding a second
command-bus bypass — the same assignment already existed for the "Edit" button.
Restructured so both controls set one `start_edit` flag and the state changes in
one place: two controls, one mutation, ratchet flat at 265. The gate was right
and the better shape was the one that satisfied it.

**`element.rs`'s Container arm is one definition.** `paint`, `paint_with` and
`solve` each hand-typed the padding inset, the flex build, the solve and the
translate — three copies of identical arithmetic. The sibling `Kind::Text` arm
had already been consolidated into `paint_text` on the stated grounds that "two
copies of how a declared property becomes a glyph position is how one starts
ignoring them again"; the Container arm never got the same treatment. Now
`solve_children` returns each child with its absolute rect and the three callers
differ only in what they do with it. This one was mine.

**And the correction. The author-handle "data loss" finding was wrong.**

A census agent reported that `AppCommand::SetAuthor` routes to
`set_author_handle_mem`, bypassing the persisting `set_author_handle`, so "a
user's author handle is lost on restart". The claim is precise, the evidence
cited was real, and it does not survive reading the call site: `SetAuthor` is
`#[cfg(debug_assertions)]` and carries the comment "In-memory only — never write
the user's real author.txt from a test." The memory-only routing is deliberate
and correct for a test command.

The true state is smaller and different. `author_handle()` reads `author.txt` at
startup, nothing writes it, and `set_author_handle` is the function that would —
so the handle can only be set by editing the file by hand. That is a settings
control that was never built, not a bug in the code that exists. Recorded at the
definition so the next reader gets the corrected version rather than
rediscovering the same wrong conclusion.

This is the cost of the fan-out, stated plainly: four reports, every one
requiring verification, and one headline finding that was confidently wrong in a
way that would have produced a bad change had I acted on it directly. The
searching parallelised; the judgment did not.

---

## AT-186 — The unknown day change, and the instrument that was seeing 4% of gpu.rs

Six sites computed a percent change under a `prev_close > 0.0` guard and answered
the absent case with `0.0`. That is not a neutral placeholder. It is the claim
"unchanged", and the app acted on it:

* `Top Gainers` filters `change_pct >= 0.0`. A symbol whose previous close never
  arrived scored exactly `0.0`, passed, and was listed as a gainer. `Top Losers`
  filters `<= 0.0`, so the SAME symbol was listed as a loser at the same time. A
  scanner whose entire job is "which names are moving" was including names it had
  no move data for, in both directions at once.
* A change cell colours on `>= 0.0`, so the unknown painted BULL green and read
  `+0.00%` — the most confident thing a price cell can say.

**The inverse was worse.** Three sites reconstructed the previous close back out
of the fabricated percentage (`price / (1.0 + pct / 100.0)`, falling back to
`price` itself at `0.0`). That writes `prev_close == price`, and every downstream
"is this known" check tests `prev_close > 0.0` — so the unknown passed. "Save
scan as watchlist" wrote it to disk with `loaded: true` beside it. The
fabrication crossed a persistence boundary and became permanent.

**The fix is to keep the fact and derive the display.** `ScanResult` now stores
`prev_close` — the datum the feed already sends — instead of a precomputed
`change_pct`, and `foundation::market::day_change_pct` returns `None` when there
is nothing to divide by. `Some(0.0)` (genuinely flat) and `None` (unknown) are
now different values, which is the distinction the whole defect turned on.

Also consolidated: both `ScannerPrice` command handlers — the pane router and the
winit event loop — carried a verbatim copy of the fabrication. One
`apply_scanner_price` now.

**The comparator nearly reintroduced the bug it was fixing.** The first version
sorted descending by passing the arguments reversed, `cmp(b, a)`, which silently
reversed the `None` handling too and floated every unknown to the TOP of Top
Gainers. It compiled, it read correctly, and the tests were written after it. The
replacement takes `(a, b)` in the caller's order and flips only the Some/Some
comparison. Both mutations — admitting unknowns, and the reversed sort — were
then re-applied deliberately and confirmed to fail exactly the tests written for
them.

### The larger finding: the ratchet was seeing 4% of gpu.rs

`quality_gate.py` computed "production code" by truncating each file at its first
`#[cfg(test)]`, on the Rust convention that test modules sit at the end.
`gpu.rs` is 11,139 lines and its first test module starts at line 542. Every
metric in that gate saw 4% of it — including the `.unwrap()` panic budget whose
own failure message reads "a panic in the render thread kills the window".

`strip_test_hits.py` had already solved this, brace-matched and with a selftest,
after the same assumption hid 17 violations from `check-design-system.sh` and
made `token_consumer_gate.py` invent 37 findings. Its docstring even warns that
the failure is unbounded — "any file that later gains a mid-file test module
silently drops out of the count below that point, and the ratchet reports an
improvement for it". That fix was applied to one consumer and not this one.
Third instance of the same root cause.

What the truncation was hiding, once the shared stripper was wired in:

| metric | was | actually |
|---|---|---|
| `.unwrap()` (dev_inspector) | 4 | 78 |
| `.unwrap()` (data) | 11 | 20 |
| `.unwrap()` (chart_other) | 2 | 11 |
| `.expect(` | 56 | 60 |
| `IndicatorType::` | 151 | 192 |

92 hidden `.unwrap()` calls in a gate whose stated purpose is bounding panic
risk. The baseline was re-cut upward — the numbers rose because the instrument
was repaired, not because the code got worse.

`text_width_guess` went 2 → 4 and back to 2: the two newly-visible hits were
`hist.len() as f32 * bar_w`, a histogram's bar COUNT times its bar pitch, which
is exactly the count-x-pitch case the exemption exists for. The earlier "0 in
scope" claim survives.

### New gate, and what it does NOT claim

`fabricated_ratio` counts `if base > 0.0 { a / base } else { 0.0 }`. It baselines
at 34, not 0, and the doc says why: the shape has two populations and the regex
cannot separate them. Most are fine — `plus_dm_sum / tr_sum` with no true range,
`above_avg / total_vol` with no volume: the denominator is genuinely zero and
`0.0` is the right answer. A minority are this defect in other clothes, and they
are on the trading surface:

* `(last / entry_price - 1.0) * 100.0` where `entry_price == 0.0` means there is
  NO POSITION — and renders a P&L of `0.00%`.
* `maintenance_margin / net_liq` where `net_liq == 0.0` means the account
  snapshot has not loaded — and renders 0% margin usage, which reads as "no
  margin risk".

Those are the next pass. The gate is a ceiling on the shape, in the same spirit
as `dead_code_allows` starting at 52; a passing run must not be read as "no
fabrication here".

Also removed: `chart_widgets`'s `_prev_close` / `_day_change_pct`, computed every
frame, stored, and never read — the underscore prefix was someone silencing the
warning rather than deleting the fields.

---

## AT-187 — The overlay kit centralised drawing but not placement

`overlays/kit.rs` opens by describing itself as "step 1 of the overlay system":
the shared primitives (`hero_number`, `sub_label`, `radial_gauge`,
`progress_bar`) that ~45 on-chart widget bodies draw with, instead of each one
hand-painting. That step worked. The one after it never happened, and the reason
is visible in every signature: each primitive takes an absolute `Pos2`. The kit
owns what a thing LOOKS like and nothing owns where it GOES.

So every body still computed its own placement from the raw `body` rect:

* **142 hand-written edge insets** across **26 distinct constants** — 2, 4, 6,
  8, 10, 12, 14, 18, 36, 50 and more. Two widgets side by side on the same chart
  had visibly different internal padding, picked by whoever wrote each one.
* **Eight bottom captions** at `body.bottom() - 6.0`, `- 8.0` and `- 10.0`.
  Three constants for one intention.
* **Nine widgets hand-wrote a hero-number-plus-caption stack**, five of them
  with exactly `+ 18.0` between the lines — while the kit's own `stat()` helper
  did the same thing at `+ 22.0` and had **zero callers**. The kit held one
  spelling of the relationship, the call sites held another, they disagreed by
  4px, and nothing used the kit's. A primitive that does not fit its callers is
  not a primitive; `stat()` also forced one colour on both lines, which is why
  nine bodies wrote the pair out by hand.

### This was broken, not untidy

`Settings → Density` (Tight 0.75 / Standard 1.0 / Loose 1.25) is a real,
persisted, user-facing control, and every spacing token multiplies through
`spacing_scale_override()`. A literal `8.0` does not. `chart_widgets.rs`
referenced a spacing token **once** in 3,348 lines — so changing Density
reflowed every panel in the app and left all 45 chart widgets exactly where they
were. `density_moves_the_widget_geometry` now asserts the opposite, and fails if
a literal comes back.

### What was added, and what deliberately was not

`body_content`, `body_footer`, `body_footer_text` and `body_footer_kv` hand back
token-derived geometry, so a body states its structure and never an inset.
`stat()` was rebuilt with two colours and a token gap and now has seven callers.

`body_footer_kv` is an `El` tree, because a label pushed flush right by a spacer
is structure worth solving. `stat()` deliberately is **not**: two centred lines
need a token and an addition, and a Taffy solve plus font measurement per widget
per frame on the chart's paint path would buy nothing. The system is for where
structure exists, not for everywhere.

Delta in `chart_widgets.rs`: raw `painter.text()` 104 → 96, body-edge insets
142 → 123, hand-written `hero_number` 13 → 6. `overlay_body_insets` ratchets the
rest at 124 so the population cannot grow while the remaining widgets convert.

The degenerate-body test earned its place on the first run: `body_content`
originally guarded with `.intersect(body)`, which does not un-invert a rect —
an inverted rect intersected with anything is still inverted, every downstream
`width()` goes negative, and the row paints backwards.

### The gate that counted nothing

`BODY_INSET_RE` was written with a `\b` that collapsed into a literal BACKSPACE
(0x08) before it reached the source. The pattern printed correctly, compiled
without error, and matched nothing. The metric read **0** — and 0 was one command
away from being committed as its baseline. A ceiling of 0 on a regex that can
never match is a gate that passes forever while reporting success, which is
worse than no gate at all.

`quality_gate.py --selftest` now proves every pattern in the file against a
sample it must match and one it must not, rejects any pattern containing a
control character, and the normal path refuses to report or re-baseline if the
selftest fails. Verified by reintroducing the backspace: exits 1, names the
pattern, and prints the offending repr.

That is the fourth time in this codebase's tooling that an instrument reported a
flattering number — after the truncated-file ratchet (AT-186), the parenthesised
text-width cast, and the `#[allow` regex that could not see `#![allow`. The
pattern is consistent enough to state as a rule: **a check that has never been
shown to fail has not been shown to work.**

---

## AT-188 — Converting the overlay bodies, and where the conversion stops

Continuation of AT-187. `body_header` / `body_header_text` were added as the
mirror of the footer pair — nine bodies wrote `body.top() + 6.0` or `+ 8.0` for
a header baseline, the same intention in two constants, neither on the scale.
`the_header_mirrors_the_footer` asserts the two sit the same distance from
their own edge.

`chart_widgets.rs` across AT-187 and AT-188:

| | before | after |
|---|---|---|
| raw `painter.text()` | 104 | 91 |
| body-edge insets | 142 | 64 |
| distinct inset constants | 26 | 23 |
| kit geometry calls | 0 | 76 |

### Where it stops, and why that is the right call

The remaining 64 are NOT the same defect, and sweeping them to move the number
would be the exact failure this ledger keeps recording. Two classes:

* **Genuine column positions** — `body.left() + 50.0`, `+ 52.0`, `+ 55.0` are
  the x of a label column in a row grid, not padding. Snapping them to a
  spacing token would be nonsense.
* **Row-grid origins** — `let y = body.top() + 4.0 + i as f32 * row_h`. The
  `+ 4.0` is the pad above the first row, so converting it IS correct in
  principle. But it shifts every row down by 4px in a widget that then draws
  `count * row_h` of content, and `draw_positions_panel` already carries an
  explicit `if y + row_h > body.bottom() - 2.0 { break; } // clip to body`.
  Four pixels there can drop a row off the bottom.

This app renders to a native window, so the verification rule recorded for it
is build-and-test, not screenshots. A change that can silently delete a row of
positions from a trading panel is not one to make without seeing it. The row
grids need `body_rows(body, row_h, count)` — a helper that owns the origin AND
the visible-row count together — and that is a change worth making deliberately
rather than as the tail of a padding sweep.

`overlay_body_insets` is re-baselined at 66 (the count across
`chart_widgets.rs` and `overlays/`), so the population cannot grow while that
work waits.

---

## AT-189 — A profitable short printed `+$1000.00 (-10.0%)`

Chasing the trading-surface half of the `fabricated_ratio` population (AT-186)
turned up something worse than the fabrication.

Position P&L percent had **five implementations**, and they disagreed about
which way a short moves:

| where | direction applied |
|---|---|
| `trading::Position::pnl_pct` | **no** |
| `render/pane/core.rs` chart position overlay | **no** |
| `chart_widgets` position P&L | yes |
| `chart_widgets` positions panel | yes |
| `portfolio_pane` positions table | yes |
| `journal_feed` closed trades | yes (long/short branches) |

`qty` is negative for a short. The dollar figure comes from `unrealized_pnl`,
or from `(last - entry) * qty`, so it flips correctly. The percentage in two of
those places did not.

Short 100 at $100, price now $90:

* dollars: `+$1,000.00` — correct, the short is up
* unflipped percent: `-10.0%`

`render/pane/core.rs` printed both, on one line, as
`format!("{}${:.2} ({:+.1}%)", ...)`. A trader reading a position label during
a move should not have to work out which half to believe. `Position::pnl_pct`
fed the bottom dock and the orders panel, so four surfaces showed the wrong
sign on a short and three showed the right one, simultaneously.

`the_percentage_agrees_with_the_dollar_sign` now asserts the invariant directly
across all four quadrants (long/short × up/down) rather than testing the
formula, because the formula was never the thing that mattered. Removing the
flip fails four tests and prints the contradiction verbatim:
`qty=-100 avg=100 cur=90: dollars=1000 but pct=-10`.

`pnl_pct` also returns `Option` now: `avg_price == 0.0` is a position whose
cost basis has not arrived, and it rendered `+0.00%` — a claim the position is
exactly flat. Same defect as the day change (AT-186), same fix.
`pnl_pct_at(price)` exists because the chart overlay legitimately wants the
pane's own last bar close rather than the broker snapshot's price; that was the
only real difference between the five copies, and it is now the only parameter.

Six call sites, one implementation.

---

## AT-190 — Five on-chart widgets displayed invented market data

Found while converting row grids, and it outranks everything else in this pass.

Five widget bodies rendered hardcoded literals as live market data. None of
them read `WidgetData`; three took no data parameter at all.

| widget | what it displayed |
|---|---|
| `draw_cross_asset` | `SPY +0.42%`, `QQQ +0.68%`, `DXY -0.15%`, `VIX +2.3%`, `TNX`, `GLD`, `CL`, `BTC +1.8%` |
| `draw_market_breadth` | `ADV / DEC 1,842 / 1,156`, `NEW HI 48`, `NEW LO 12`, `VIX 18.5` |
| `draw_options_flow` | six prints incl. `460C 5DTE $3.1M sweep`, `440P 1DTE $1.8M block` |
| `draw_earnings_mom` | `EPS +12%`, `REV +8%`, `REVISIONS ↑3`, `FWD P/E 22.4x` |
| `draw_risk_reward` | `risk = 1.0`, `reward = 2.8` — the bar always read 1:2.8 |

On a terminal used to place real orders, a titled panel showing a figure a
trader can act on is not a placeholder.

### The pattern already existed and was applied inconsistently

`draw_widget_no_feed(p, body, t, "<name> feed", "not connected")` was already
in the file and already used by `draw_sector_rotation`,
`draw_options_sentiment` and `draw_econ_calendar`. So the app taught the user
that "not connected" means not connected — and therefore that a panel showing
numbers means it IS connected. Inconsistency here is worse than uniform
absence, because it manufactures the confidence.

All five now use it. `WidgetData` does carry `vix_spot`, `eps_ttm` and
`revenue_growth`, so parts of breadth and fundamentals COULD be wired — but a
panel with two real rows beside two invented ones reads as fully real and is
worse than one that says nothing. Wire them when the whole panel has a feed.

### The gate

`fabricated_market_data` matches a string literal that is unambiguously a
market VALUE — a signed percentage, a currency figure, or a multiple — inside
a widget file. Labels (`"SPY"`, `"EPS"`) are bare words and do not match;
format strings are excluded by their brace. 17 hits before, 0 after, and
`quality_gate.py --selftest` proves the pattern against both known-positive and
known-negative samples.

The one false positive it did produce is worth recording: `"4x"` is the
max-end AXIS LABEL on the tape-speed gauge, whose reading is
`format!("{:.1}x", speed)` from real data. Requiring a decimal in the multiple
form separates a round scale label from a reported metric. Not airtight, and
the doc says so — a bare `"18.5"` carries no marker and would slip through.
This backs up review rather than replacing it.

### The adoption floor did its job

Deleting `draw_options_flow`'s body removed 7 `El::` nodes with it, and
`cascade_adoption_gate` failed: `el_nodes 167 -> 160`, `declared_rows 74 -> 72`,
`self_painting 46 -> 42`. That is the floor working exactly as intended — it
refused to let adoption fall silently and made the drop be justified. Every one
of the 7 removals is inside the fabricating widget (verified against the diff;
no `Flex::` removals, one `show_with`), so the floors were re-cut downward with
the reason recorded here rather than raised past the problem.

Two systems pulled in opposite directions — "keep declarative adoption up" and
"never display invented data" — and the second wins every time.

### Also in this pass

* `body_rows` divides a body into fitted rows. Five grids wrote
  `let row_h = (body.height() - 8.0) / 5.0` next to
  `let y = body.top() + 4.0 + ...`, stating the padding TWICE — doubled in the
  height, halved in the origin — with nothing connecting them. They agree
  today (8 = 2 x 4); change one and rows run past the bottom edge.
  `every_fitted_row_lands_inside_the_body` asserts the property over eight row
  counts and four body heights; breaking the link fails it immediately.
* `portfolio_pane`'s margin utilisation: `net_liq == 0.0` means the account
  snapshot has not arrived, and it rendered `0%` in BULL green with an empty
  bar — the most reassuring possible display of "we have no idea", on the one
  figure that says how close a trader is to a margin call. Now `—`, Muted, and
  NO bar (an empty bar reads as "none used"). Position weights likewise.
* Removed `body_rows_of_height` and `RowGrid::rect_of`, which I had just added.
  The former was built to replace two `break` clips; `draw_options_flow` is now
  a no-feed stub and the positions panel clips against the same `El` geometry it
  lays out with, so a separate count would REINTRODUCE the two-mechanisms
  defect. Keeping either "for when it's needed" is precisely how `stat()` sat
  unused with the wrong signature (AT-187).
* Removed kit's `metric_row` — a painter-side duplicate of `ui_kit::MetricRow`
  (4 real callers) whose only caller was the fabricating breadth widget.

---

## AT-191 — A DEMO badge, and a correction to an earlier finding

### The correction first

An earlier census recorded "the Trade Plan panel has no trigger — it can never
be opened". **That was wrong.** `signals_panel.rs:126` carries a visibility
toggle ("Trade Plan — Entry / target / stop overlay") alongside ten siblings,
and a "Start Demo" button populates it. The overlay is reachable and has been.

The real gap is narrower and different: `ChartCommand::TradePlanUpdate` is
HANDLED in `gpu.rs` and sent by nothing. Outside demo mode a trade plan can
only appear via the demo block in `core.rs`. That is a missing producer, not a
missing trigger, and it is recorded here rather than acted on.

### What a Trade Plan is

Worth stating, since the name is ambiguous. It is a suggested options play:

* `direction`, `entry`, `target`, `stop` — painted as dotted lines across the
  pane with a green zone from entry to target and a red one from entry to stop,
  plus `T` / `S` labels on the price axis.
* `contract` — a specific option, e.g. `SPY 455C 5DTE`.
* `rr` and `conviction` — rendered on a card as `R:R 2.8 | +2.0% | CVT 85`.

So: which contract to buy, where to get in, where to take profit, where to get
out, and how strongly the engine believes it.

### The badge

"Start Demo" fabricates exactly that — plus supply/demand zones, precursor
scores and a VIX expiry card — and paints it with the same lines, the same
axis labels and the same card a real plan would use. The only indication was a
button in a side panel reading "Stop Demo", which is not on screen when the
panel is collapsed.

A trader who starts a demo, closes the panel, and returns later has no way to
distinguish the plan from a live one. Same defect class as the five widgets
displaying invented quotes (AT-190), at a higher price: this one names a
contract and a stop.

`render_demo_badge` now paints a persistent "DEMO DATA" pill at the top centre
of the pane, last, so nothing can cover it.

`Chart::signal_demo_active()` is deliberately shared by the badge and the
panel's button label. The panel derived the predicate inline; a second copy is
how a badge comes to under-report, and a badge that under-reports is worse than
none. `every_demo_signal_raises_the_flag` pins each of the three sources.

### The design-system ratchet caught the badge

The first version used raw `40` / `160` alphas and a `1.0` stroke, and
`check-design-system.sh` failed with `core.rs: 515 -> 518 (+3)`. Correct call —
a new off-token primitive is a regression whoever writes it. Replaced with
`alpha_tint()`, `alpha_strong()` and `stroke_std()`; the baseline then improved
to 964 and was re-cut. Fixing the code rather than raising the ceiling is the
whole point of the ratchet, and it is worth recording that it fired on new work
in this session and not only on inherited code.

---

## AT-192 — `Select` sized itself with one font and painted with another

The component-layer pass started by looking for hand-built widgets with real
structure. `Select` topped the list — 10 text paints, 3 anchors, 6
hand-advanced x offsets — and reading its trigger turned up a live defect.

`paint_select` measured the widest option with:

```rust
let label_font = st::prop_at(font_size);   // PROPORTIONAL
```

and every paint site below it — the single label, the placeholder, the
multi-select chips, `chip_paint` — drew with `st::mono_at(font_size)`. The
trigger was SIZED from a proportional layout of the string and DREW a
monospace one. `content_right` exists but is consulted only by the multi-chip
path, so the single-mode label had no bound at all.

For wide glyphs the proportional measure is the larger of the two, the trigger
came out roomy, and nothing looked wrong. Narrow glyphs invert it: a
proportional `i` is a sliver, a monospace `i` fills a cell. Measured at
`Size::Xs`, a 24-character narrow label:

```
label   10.0 → 154.2
caret   67.0 →  71.6      <- painted ON TOP of the label
```

87 pixels of overrun, with the caret drawn over the text. `Select` backs the
option-chain expiry picker and the DOM order-type picker — by the file's own
comment, "two of the most-used dropdowns in the app" — and this is the SECOND
defect in this one function traceable to the proportional family. The first was
the chevron: Inter does not carry U+25BC, so every trigger painted a tofu box
where its caret belonged.

Fixed by measuring with the font the paint uses.

### How the first version of the test hid it

The test was originally written with `"WWWWWWWWWWWWWWWW"`, and it passed. It
passed across all five sizes. It even passed with `trigger_extra_pad` mutated
to zero. Every one of those was a green run that proved nothing, because `W` is
the glyph where the proportional measure is WIDER than the monospace paint —
the safe direction.

Only dumping the actual runs showed the tell: a 56–134px gap between label and
caret at every size, far more than any padding accounts for. A measure that
over-reports by that much is the same bug as one that under-reports; it just
does not show. Switching the fixture to narrow glyphs failed immediately.

The test now covers both glyph classes at all five sizes, and reverting the
one-line fix fails it.

### And the thing that was already there

`select.rs`'s only existing test was:

```rust
let src = include_str!("select.rs");
assert!(src.contains("cursor::focus_ring"), ...);
```

A grep of the file's own source text. It cannot see geometry, it cannot see
overlap, and it passes for any arrangement of pixels whatsoever. The widget had
a test and no coverage.

### Guard, not discovery

`Tag` got the same overlap probe — it is the same two-part shape (label plus a
close glyph) and the same failure was available to it. `tag.rs` measures and
paints both with `prop_at`, so it is currently correct; the test is a guard
against drift, and it is honest to say so. It is not vacuous: under-measuring
the label reproduces the overlap immediately.

An audit of every widget that measures and paints found only these two files
mixing families, and `input.rs`'s mix is deliberate — an explicit
`proportional` flag selects the font, and its prefix/suffix measure and paint
both with `mono_at`. `prop_at` for icon glyphs is the established convention
(`header.rs`, `selectable_row.rs`), so `Icon::X` in the tag close button is
consistent rather than a third instance.

**A check that has never been shown to fail has not been shown to work** — and
this is the first time in this session that rule caught a test of MINE rather
than an instrument I inherited.

---

## AT-193 — Measure-then-re-layout is the shape, and 29 files have it

AT-192 fixed one instance. This is the class.

Every fit/paint disagreement this codebase has produced has the same
structure: a string is laid out ONCE to decide how much room to reserve, and
laid out AGAIN to be drawn. Two code paths, two sets of arguments, and nothing
forcing them to agree. `Select` differed in the font family. The tab strip
differed in a gap. The spreadsheet had three spellings of a column offset.

`toggle_row` shows the form that cannot fail:

```rust
let g = ui.fonts(|f| f.layout_no_wrap(label.clone(), font.clone(), col));
let w = g.rect.width();
painter.galley(pos, g, col);        // the SAME galley
```

One layout, used twice. A mismatch is not merely unlikely, it is unexpressible.

Counted across `ui_kit/` and `chart/renderer/ui/`:

* **8 files reuse the measured galley** — `toggle_row`, `alert`,
  `panel_list_row`, `panel_sub_section`, `status_pill`, `tool_overlay`,
  `dom_panel`, `watchlist_columns`.
* **29 measure and then re-lay-out** — led by `tabs.rs` (8 measures),
  `painter_pane.rs` (8), `button.rs` (6), `panels/kit.rs` (5), `select.rs` (3).

The second form is not wrong and mass-refactoring 29 files on the strength of
one bug would be exactly the kind of speculative sweep this ledger keeps
arguing against. What it needs is COVERAGE, and the rule is now in
`DESIGN_SYSTEM.md` next to the measure guidance.

### Probes added

`tabs` — 24 configurations (four label sets x closable x three sizes), all
producing runs, none skipped. Verified non-vacuous by dropping `inner_gap`
from `measure_tab_width`, which reproduces a 0.6px label/close-glyph overlap.
The existing test in that file opens "We can't run a full egui render in a unit
test", which is the assumption `paint_probe` was built to disprove.

`kbd` — the chord is an alternating cap/`+` strip with every segment
pre-measured, so an under-measure accumulates across it. Currently correct
(mono both ways); shrinking the cap width makes the caps collide.

Both are guards rather than discoveries, and saying so matters — a test whose
value is "it would have caught X" should not be written up as if it found X.

### The fixture rule

Every one of these uses a NARROW-glyph case. `W` is wider proportionally than
in mono, so a test built on it passes whichever font the measure used. That is
how AT-192's first three green runs proved nothing, and it is now written down
rather than remembered.

### Checked and left alone

`toggle_row`, `input.rs` and `kbd` were each read for the `Select` defect and
none has it. `prop_at` for icon glyphs is the established convention
(`header.rs`, `selectable_row.rs`), so `Icon::X` in the tag close button is
consistent, not a fourth instance. Recorded so the next reader does not
re-derive it.

---

## AT-194 — Finishing the fabricated-ratio triage

The `fabricated_ratio` ceiling was installed at 34 with the honest caveat that
its regex cannot separate two populations. The triage is now complete: 29 sites
at the start of this pass, 26 remain, and every remaining one is the benign kind.

### Fixed (the denominator was ABSENT, not zero)

| site | what it rendered |
|---|---|
| `eps_estimate == 0.0` (`research_panel`) | a company with **no analyst estimate** shown as `0.0%` in BEAR red beside `vs $0.00` — a miss, by nothing |
| `first_px == 0.0` (`core.rs` pane chrome) | a pane with **no bars** wearing a confident `+0.00%` badge in BULL green |

Plus, earlier in this session: `entry_price == 0.0` rendering a P&L of `0.00%`
(AT-189) and `net_liq == 0.0` rendering 0% margin usage in bull green with an
empty bar (AT-190).

The EPS one is worth spelling out because the colour made it worse: `beat` was
`surprise > 0.0`, and `0.0 > 0.0` is false, so the *absence* of a forecast
resolved to the bear branch. Nobody forecast the quarter, and the panel said
the company missed.

### Consolidated

`foundation::market::pct_change(from, to)` is now the one implementation, and
`day_change_pct(price, prev_close)` is that function with its base named. They
had been separate `if base > 0.0 { .. } else { 0.0 }` expressions, which is how
the pane badge came to fabricate while the watchlist row (already fixed) did
not. A test asserts the two agree across every case including the unknown one.

### Left alone, deliberately, with the reason recorded

The 26 remaining all have a genuinely zero denominator: no true range
(ADX/DI), no deviation (CCI), no volume (`above_avg / total_vol`,
`level.total_vol / vp.max_vol`), an empty book (`cell.weight / total_cap`), a
flat set (`change_pct.abs() / max_pct`), a degenerate rect (`dashboard_pane`),
no frames yet (`1000.0 / frame_ms`). Nothing moved, and `0.0` is the correct
answer. That enumeration now lives in the gate's own doc so the next reader
inherits the triage instead of redoing it, and so a passing run is understood
as "no NEW instance of the shape" rather than "no fabrication here".

`chart.tab_changes` is a partial: the derivation goes through `pct_change` now,
but the field is an `f32` the tab strip colours on `>= 0.0`, so an unloaded tab
still reads flat-and-green. Changing that field to an `Option` is a separate
change and is noted rather than smuggled in here.

### Flagged, not fixed — "Avg R" is against an assumed risk

`journal_feed` computes the R-multiple with a documented heuristic:

```rust
// R-multiple: assume 1:1 risk if we have no stop info
// i.e., risk = 1% of position
let risk = entry_price * matched_qty as f64 * 0.01 * multiplier;
```

The journal panel then displays `Avg R` with no indication that the denominator
is an assumption rather than the trader's actual stop. The number is not
fabricated — the derivation is deliberate and commented — but the LABEL claims
more than the number knows, and a trader reading "Avg R 2.3" will take it as
2.3x their own risk. That is a wording/product decision, so it is recorded here
for the owner rather than guessed at.

---
