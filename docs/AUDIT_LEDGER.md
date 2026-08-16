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

**Not wired, and not deleted.** Wiring it puts a new visible element into a
trading toolbar and needs a live quote source; that is a product decision, not a
design-system fix, and it is not mine to make. Deleting it discards work that
was clearly intended. It is recorded here for that decision.

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

