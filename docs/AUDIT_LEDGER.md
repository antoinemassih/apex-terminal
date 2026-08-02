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
- [ ] **AT-032** `P1` `C` `TRADING` — Confirming a Draft OCO or bracket leg re-submits it as a standalone order — the OCA group and bracket parent are lost, so both legs can fill
- [ ] **AT-033** `P1` `C` `TRADING` — In paper mode the poller still adopts REAL broker orders and the paper fill engine then fabricates fills for them, with synthetic prices booked into realized P&L
- [ ] **AT-034** `P1` `C` `TRADING` — Kill switch, halt and resume ignore the broker's HTTP status AND their Result is discarded — a failed server-side kill reports success
- [ ] **AT-035** `P1` `C` `TRADING` — Risk gates block position-REDUCING orders: once the daily-loss breaker auto-halts, Flatten/Reverse cannot close the position, and the failure is completely silent
- [ ] **AT-036** `P1` `C` `TRADING` — `paper_mode` is not persisted: a restart without APEX_TRADING_MODE=live restores real live orders into paper mode, where cancel is a no-op that marks them Cancelled locally

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
- [ ] **AT-019** `P1` `C` `ENGINES` — VWAP is implemented twice with different session-reset rules; the σ-band version never resets on crypto or futures

---

## W5 — finish the migrations (RC-1)  (21 items)

- [ ] **AT-016** `P1` `C` `DESIGNSYS` — Two live type ladders have drifted apart: the tier scale was lifted to 9/10/12/14 but the semantic StyleSettings ladder still renders 8/10/11 — TextStyle::Caption is now BELOW the tier floor
- [ ] **AT-027** `P1` `W` `REDUND` — Broker order URL is a hardcoded dev-host const; the runtime-configurable resolver built for it is an orphan file that never compiles
- [ ] **AT-028** `P1` `C` `REDUND` — The full option chain is cloned and re-materialized twice per frame on the UI thread, in a call the codebase itself documents as too expensive to call per-frame
- [ ] **AT-029** `P1` `C` `REDUND` — Three writers into watchlist.chain.near/far; the per-frame cache-derive clobbers the command path and never clears the PLACEHOLDER flag
- [x] **AT-030** `P1` `C` `REDUND` — Two divergent RSI implementations and two divergent ATR implementations, both rendered on screen at the same time
- [ ] **AT-031** `P1` `C` `REDUND` — Two type scales run in the same frame: TextStyle is style-live, ui_kit's font_*() is frozen to literals in every shipping build
- [ ] **AT-041** `P1` `C` `UX` — News panel renders every headline twice, and the second copy's click handler is a dead `// TODO: open URL`
- [ ] **AT-050** `P2` `C` `ARCH` — No single HTTP/endpoint layer: 16 files build their own reqwest client and the ApexSignals base URL is re-derived from env at 8 independent sites
- [ ] **AT-054** `P2` `?` `DATA` — IB feed bumps the gap-fill anchor with a hardcoded "5m" timeframe inside a loop that already knows the active timeframes
- [ ] **AT-081** `P2` `W` `OTHER` — Two SegmentedControl types and two Select types, both live in production, with mutually incompatible call conventions
- [ ] **AT-082** `P2` `C` `REDUND` — Hot-reload StyleSystem override maps the wrong stroke tiers, thickening every hairline the moment a theme JSON is present
- [ ] **AT-083** `P2` `C` `REDUND` — The shared HTTP client introduced to fix per-call TLS handshakes was adopted by only two files; the pre-trade margin check still builds a fresh client per call
- [ ] **AT-084** `P2` `C` `REDUND` — The strikes-overlay chain fetch is a third parallel path that neither reads nor seeds the shared chain cache
- [ ] **AT-085** `P2` `C` `REDUND` — ~980 lines of a fully-built parallel theme model (ThemeRegistry / ActiveTheme / DesignSnapshot) has zero production callers, and a test comment falsely claims begin_frame uses it
- [ ] **AT-104** `P3` `W` `ARCH` — Two parallel persistence schemes coexist: a versioned `Persistable` envelope used by ~6 aggregates, and ~15 hand-rolled JSON files with no version field
- [ ] **AT-117** `P3` `C` `DEADCODE` — `chart/renderer/compute.rs` holds a second, dead copy of the drawing-tool math that `core.rs` implements inline
- [ ] **AT-120** `P3` `C` `DESIGNSYS` — Two divergent definitions of the Meridien style exist; the one the ThemeRegistry defaults to is not the one the app renders
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
- [ ] **AT-064** `P2` `C` `DESIGNSYS` — ThemeRegistry / DesignSnapshot (979 LOC) are documented as the canonical active-pair state but have zero references outside design_system/
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
- [ ] **AT-038** `P1` `C` `UNWIRED` — Time & Sales tape is permanently empty: the WS subscription is gated on `tape.open`, a flag no user action can set
- [ ] **AT-039** `P1` `C` `UNWIRED` — Two independent shortcut systems collide: Ctrl+Shift+R resumes halted trading AND toggles a panel in the same frame
- [ ] **AT-042** `P1` `?` `UX` — News sentiment filter chip (Any/Bull/Bear/Neut) changes its own label and active state but never filters the list — the filter function is only ever called by unit tests
- [ ] **AT-045** `P2` `C` `ARCH` — 73% of the AppCommand dispatch layer has no producer in a release build — its only callers are the debug-only dev_inspector
- [ ] **AT-058** `P2` `?` `DATA` — SubscriptionManager::check_stale() has zero callers — the documented per-subscription staleness TTL never fires
- [ ] **AT-059** `P2` `W` `DEADCODE` — Conditional orders and options-trigger orders are implemented end-to-end but have no production entry point
- [ ] **AT-060** `P2` `W` `DEADCODE` — `chart/state` — 2,583 LOC of chart-storage architecture — is unreachable; its single integration point is hardcoded `None`
- [ ] **AT-074** `P2` `?` `MISSED` — The cooperative-shutdown subsystem is entirely dead: `drain_all` has zero callers, so the Postgres pool is never closed and the bug its own doc claims to fix is unfixed
- [ ] **AT-076** `P2` `C` `OTHER` — ContextMenu (507 LOC) and Popover (173 LOC) have zero production callers; the app uses raw egui context menus in 22 places
- [ ] **AT-095** `P2` `C` `UNWIRED` — Command-palette Help and Calc entries execute to nothing — the dispatcher has no arm for their ids
- [ ] **AT-096** `P2` `C` `UNWIRED` — Four rail panels are registered in the dispatch table but their `is_open` predicate has no writer that can ever return true
- [ ] **AT-097** `P2` `C` `UNWIRED` — News headline clicks are a no-op: the URL is present and non-empty-checked, then discarded
- [ ] **AT-098** `P2` `W` `UNWIRED` — The whole ApexSignals integration — not just the three MSG panels — defaults to http://localhost:8100 with no way to configure it
- [ ] **AT-099** `P2` `W` `UNWIRED` — Three user-editable hotkeys are read by nothing, including "Halt Trading" — and the F1 cheatsheet advertises a Halt chord that does something else
- [ ] **AT-100** `P2` `C` `UNWIRED` — Two complete, tested panels have zero call sites anywhere in the tree
- [ ] **AT-109** `P3` `C` `DATA` — providers/mock.rs (546 lines) and providers/replay.rs (212 lines) are compiled into production builds despite being test-only scaffolding
- [ ] **AT-110** `P3` `C` `DEADCODE` — Dead REST client surface in the ApexData feed: async auth-retry wrappers and four sync getters with no callers
- [ ] **AT-111** `P3` `W` `DEADCODE` — FMV is ingested on every frame into a map with no reader — `get_fmv()` is never called
- [ ] **AT-112** `P3` `W` `DEADCODE` — Fabricated market-data generators sit unreferenced in gpu.rs — a 100-LOC landmine in a live-money binary
- [ ] **AT-113** `P3` `W` `DEADCODE` — Halt tracking maintains two capped rings that no code reads; the comment claiming otherwise is false
- [ ] **AT-114** `P3` `W` `DEADCODE` — The `InFlightRegistry` migration stalled: entries are created and never expired, and no consumer reads it
- [ ] **AT-115** `P3` `W` `DEADCODE` — `ChainRow::display_price()` remains test-only — production still renders the raw field it was written to replace
- [ ] **AT-116** `P3` `W` `DEADCODE` — `SubscriptionManager::check_stale()` — the documented silent-stale-feed alarm — is never called
- [ ] **AT-118** `P3` `C` `DEADCODE` — `design_system::registry` (266 LOC) is a competing theme source-of-truth with zero references outside its own module
- [ ] **AT-126** `P3` `W` `OTHER` — 2,105 LOC of subpixel-text machinery (incl. a 986-LOC wgpu pipeline) serves exactly one production call site
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
- [ ] **AT-089** `P2` `C` `STATE` — atomic_write uses a fixed shared `<path>.tmp` sibling, and two threads can write the same store path concurrently
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
- [ ] **AT-135** `P3` `W` `STATE` — ORDERS_SNAPSHOT publish-after-unlock has no ordering guard — the order ledger and on-chart order lines can go permanently stale
- [ ] **AT-136** `P3` `C` `STATE` — Per-pane and per-window UI state parked in single-slot process globals — the options-chain seat set is cleared by whichever surface renders last
- [ ] **AT-141** `P3` `C` `UX` — Seasonality month attribution drifts across leap-year boundaries, misfiling early-January bars as December
