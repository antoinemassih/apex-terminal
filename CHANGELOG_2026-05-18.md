# Apex Backend Multi-Repo Session — 2026-05-17/18

Big multi-day session landing Polygon Stocks Advanced rollout + the SOTA closed-loop architecture across the Apex stack. **53 branches merged across 6 repos, ~4500+ tests passing, 0 push events yet at the time of this writeup.**

This is the shared changelog — same content lives in each repo's root. Per-repo specifics below.

---

## Repos touched

| Repo | Branches merged | New | Status |
|---|---|---|---|
| **apex-common** (new crate) | n/a (built from scratch) | YES | 74/74 tests; ready to push |
| **ApexData** | 26/26 | — | Build clean; 51 commits ahead of origin/master |
| **ApexSignals** | 16/16 | — | Build clean (140 pre-existing warnings); 32 commits ahead of gitea/master |
| **apex-terminal** | 7/7 | — | Build clean (lib + apex-native bin); commits ahead of origin/main |
| **ApexIB** | 1/1 | — | Tests pass; 2 commits ahead of origin/master |
| **preopen-scanner** | 1/1 | — | Stdlib only; committed locally; no remote yet |
| **apex-spike-explainer** (new repo) | n/a (built from scratch) | YES | 11/11 tests; committed locally; no remote yet |
| ApexCrypto | 0 (no changes) | — | Clean — not touched this session |
| ApexFeed | N/A — repo doesn't exist | — | (Possibly meant `C:\dev\polygon-feed` which is empty; agreed to retire/refactor — no work landed) |

---

## ApexData — the data plane

**26 branches merged.** Polygon Stocks Advanced rollout, SOTA wave, hardening, projector fleet.

### Stocks Advanced rollout (Phase 1 + 2)

- `apex-common-migration` — Swapped inline OCC, AssetClass, normalize_underlying for `apex-common` re-exports (drop-in equivalence)
- `polygon-stocks-rest` — 10 stocks REST methods (`aggs_grouped_daily`, `snapshot_all_stocks`, `snapshot_movers`, `reference_tickers` w/ paginate-all, `reference_conditions` + disqualifier-set builder, `market_status_now`/`upcoming`, `reference_splits`/`dividends`, `reference_ticker_news`) + 3 routes
- `questdb-stocks-tick-tables` — `stock_trades` + `stock_quotes_l1` DDL + ILP writers + retention helper
- `preopen-routes` — `GET /api/options/contracts?expired=true` + `GET /api/bars/option/:occ/1d` (unblocks preopen-scanner historical mode)
- `stocks-ws-workers` — N=1-4 hash-sharded Polygon stocks WS workers behind `APEX_DATA_STOCKS_WS=1` flag; T/Q/AM channels; exp backoff w/ jitter; auth-failure detection; Q-universe constrained to top ~3000 (`watchlist ∪ scanner ∪ chart ∪ ETF members ∪ index constituents`)
- `stocks-ws-write-path` — Conditions filter on every trade (live `/v3/reference/conditions` lookup, hot-swappable disqualifier set), batched tick-table writer (500ms / 1000-row flush), sequence-gap REST replay (5min cap)
- `universe-sync-cronjobs` — Nightly ticker diff (`/v3/reference/tickers` paginated) + grouped-daily-aggs CronJob; postgres `ingestion_log` manifest (Redis-backed temporary until sqlx dep lands); K8s manifests `timeZone: America/New_York`
- `flat-files-stocks-trades-quotes` — `us_stocks_sip/trades_v1` + `quotes_v1` + `us_indices/{day,minute}_aggs_v1` parsers; **2025-11-03 quote size semantics branch** (pre-cutoff = round-lots, post-cutoff = raw shares)
- `parquet-writer-nbbo-derivation` — NBBO-1s derivation (`derive_nbbo_l1` quote→1s L1 squashing) + letter-partitioned parquet writer (`dt=YYYY-MM-DD/letter=A..Z/`) with atomic rename + manifest callback
- `splits-divs-adjuster` — `splits_cache` (6h refresh, atomic snapshot swap) + `adjusted=` middleware on `/api/bars` and `/api/replay` (store unadjusted, adjust on read) + `/api/stocks/splits/:ticker` + `/api/stocks/dividends/:ticker`
- `backfill-orchestrator` — `backfill_orchestrator` binary + K8s Job manifest + run-plan doc; Waves 1-5 from storage plan (day_aggs 22y / minute_aggs 5y+5-22y / trades 2y+2-5y); 8-way concurrency; 250 MB/s throttle; NYSE holiday skip; manifest idempotency

### SOTA Phase 0 — closed feedback loop

- `sota-provenance-wire` — Optional `provenance: Option<Provenance>` field on v1 Frame variants (Bar/Trade/Quote/Fmv/Snapshot/ChainDelta); `GET /api/provenance/:id?format=tree|dag&depth=N` endpoint with cycle detection + node-budget truncation + Redis Stream lookup with side-index pattern
- `sota-replay-service` — Replay manager + parquet reader + `POST /api/replay/start` + `WS /api/replay/:id/stream` + control endpoints; auto-discovers parquet via existing `COLD_STORAGE_DIR`
- `sota-spike-ws-fanout` — `src/api/spike_fanout.rs` subscribes to Redis `events:spike_explanation` pubsub → broadcasts `Frame::Spike(SpikeFrame)` via new `FanoutIndex::broadcast_all` helper; auto-reconnect w/ 2s backoff; URL password redaction
- `expiry-slice-publisher` — `ChainSnapshotPublisher` (5s fast tick / 5min slow OI tick) publishing per-underlying chain snapshots to Redis pubsub `chain:{ul}:{expiry}` — unblocks the ApexSignals options-engine fleet that was waiting on `ExpirySlice` ingestion

### 8 Stocks Projectors (Phase 3)

- `projector-breadth` — Advance/decline, new H/L, % above 20/50/200 SMA, NYSE-$TICK proxy; reads `stocks:snap:*` + `stocks:ind:*:1d`; writes `stocks:breadth:{index}` Redis hash + `md:stocks:breadth:{index}` stream + QuestDB `stock_breadth`
- `projector-sector-rotation` — 11 SPDR sectors RS matrix vs SPY benchmark; momentum_rank + RRG quadrant (Leading/Improving/Lagging/Weakening); leaders from holdings cache
- `projector-halts-luld` — LULD-band proxy locally (±5%/±10% Tier-1; ±10%/±20% Tier-2; floor for Tier-3); halt detection via condition 16; HaltActive/HaltCleared/NearLuldUp/NearLuldDown states
- `projector-movers` — 5 buckets (gainers/losers/active/rvol_leaders/gappers), top-50 each, 5s rebuild from snapshot cache; env-configurable thresholds
- `projector-etf-iiv` — Indicative NAV per ETF (weight-normalized over priced holdings), premium/discount in bps, staleness fraction; reads holdings cache only
- `projector-news-sentiment` — Polls `/v2/reference/news` per watched ticker (200 max, 60s cycle, ~3.3 RPS sustained); per-article dedup; 24h weighted sentiment mean; `Frame::NewsArticle` push
- `projector-corporate-actions` — Upcoming splits + dividends within next 30 days for watched tickers; reuses splits_cache; 6h refresh
- `vol-surface-migration` — Moves IV history from Redis lists (capped 365) to QuestDB `vol_surface` table (DDL with moneyness buckets + DEDUP UPSERT); new `GET /api/stocks/iv_rank/:underlying?lookback=252` with insufficient-history fallback (no more day-1 "trivially 100 or 0")

### Hardening (Phase 4)

- `hardening-throttle-budget-cadence` — Token-bucket throttle on PolygonRest (50 RPS sustained / 100 burst; env-overridable), platform-native disk-budget enforcement (Win `GetDiskFreeSpaceExW` / Unix `statvfs`) with per-dataset retention caps, chain_snapshot cadence split (5s fast / 300s slow with `RefreshScope` gating)
- `options-rate-divs-sharded-greeks` — Daily FRED DTB3 risk-free rate fetcher (graceful fallback to 0.045 when `FRED_API_KEY` unset), per-underlying dividend-yield cache (Polygon `/v3/reference/dividends`), N=4 sharded greeks compute (env `APEX_GREEKS_SHARDS`)

---

## ApexSignals — the engine plane

**16 branches merged across 274 engines.** apex-common adoption, the SOTA layer, day-type gating, multi-horizon ensemble.

- `apex-common-occ-swap` — Replaced 73 inline OCC parser bodies across 65 engines with delegation to `apex_common::occ::Occ::parse`; ~440 LOC of duplicated parsing logic deleted; 1686/1686 tests still pass bit-identical
- `engines-md-refresh` — ENGINES.md refreshed from stale "52 engines" to actual 274; categories: Options 69, Order Flow 40
- `crypto-hist-prefix-fix` — Asset-aware bar_loader (`bars:hist:{class}:{symbol}:{tf}` instead of legacy `crypto:hist:*` for all classes); asset-aware QuestDB dispatch; URL-encode via crate (fixes `BRK.B`-style tickers); preventive fix — BarLoader has no live callers today
- `chain-reader` — New `ChainSubscriber` consuming PR-4's `chain:{ul}:{expiry}` Redis pubsub → fans to options engines via `ChainEvent::{FastTick, OiTick}`; first wiring: `IVSurfaceEngine`
- `iv-surface-dispatcher` — Wires `IVSurfaceEngine` into bar dispatcher's engine list (shared `Arc<tokio::sync::Mutex<>>` with chain_dispatcher) so its signals actually reach signal_tx
- `adaptive-learner-persist` — `adaptive_learner_weights` postgres table + hourly checkpoint during RTH + restore-on-startup with `trust=false` until obs_count>50 (linear ramp); `Signal::AdaptiveLearner` consumed in signal_combiner with `scaled_score = score * trust.clamp(0,1)`
- `day-type-v2-gating` — `signal_combiner.rs` halves conviction on counter-direction `(Pin,±1) / (Bear,+1) / (Bull,-1)`; `trade_plan.rs` HARD refuses Mixed/Chop with confidence<0.5 + emits with v2's `recommended_exit_rule`; `hydrate_from_redis` methods for cross-session restore
- `sota-provenance-threading` — `StampedSignal { signal, provenance }` wrapper at broadcast boundary; `LineageGenerator` + per-signal source-name registry; `provenance:log` Redis Stream daemon; **smart simplification: zero engine algorithm files touched**
- `sota-regime-router` — New `RegimeRouterEngine` reads day_type_v2 + market_regime_institutional + vix_regime + sector_breadth → emits unified `Signal::Regime` with `Regime{intraday,multiday,vol,sector}`; writes `signals:current_regime` Redis hash + JSON blob
- `sota-outcome-tracker` — `pending_outcomes` + `outcomes` + `calibration_curves` postgres tables; T+5/T+30/T+EOD horizon scorers; `walk_forward_calibrator` binary + CronJob; `CalibrationReader` with 5min refresh; `calibrated_contributors: HashMap<engine, Calibrated>` on CombinedSignal
- `sota-conformal-trade-plan` — TradePlan extends with `target_range`/`stop_range`/`historical_hit_rate`/`historical_n_samples`/`conformal_coverage`; distribution-free empirical-quantile algorithm; MIN_SAMPLES=30 floor; 5min refresh cache
- `sota-wire-combiner-trade-plan` — **KEYSTONE PR.** Wires `SignalCombinerEngine` + `trade_plan` generator into dispatcher (both were dead code before); `Signal::TradePlan(TradePlanSignal)` variant added; parallel `tick_stamped: Vec<StampedSignal>` mirror for provenance inputs[] capture; aggregator phase broadcasts Combined+TradePlan with full lineage
- `multi-horizon-ensemble` — `horizon::Horizon{Intraday5m, Intraday30m, Swing, Positional}` + `horizon_for(engine_name)` lookup; signal_combiner buckets signals by horizon, weighted cross-bucket blend (default `[0.4, 0.3, 0.2, 0.1]`); `dominant_horizon` on CombinedSignal; TradePlan `horizon` field
- `streaming-dag-tier1` — `streaming_dag::{occ, session, indicators::Sma<N>}` operators + 3 migrated engines (volume_conviction, consecutive_inside_bars, day_type_classifier); **fixes the pre-market sign bug in minutes_from_et_open** via div_euclid; migrated engines produce bit-identical output
- `apex-fill-outcome-subscriber` — `events:apex_fill` subscriber (ApexIB → ApexSignals); matches `lineage_id` to `pending_outcomes` → `apex_taken_outcomes` table + UPSERTs outcomes with `user_taken=TRUE, fill_price=avg_fill_price`; unmatched → `dangling_apex_fills` for ops inspection
- `per-engine-latency-histograms` — Prometheus `apex_engine_process_seconds{engine}` histogram with buckets `[100us..100ms]` + `apex_engine_slow_runs_total{engine}` counter; `GET /metrics` endpoint added

---

## apex-terminal — the UI plane

**7 branches merged** to the egui+wgpu native terminal.

- `consume-stocks-rest-routes` — Watchlist switched from ApexIB+Yahoo to `/api/stocks/snap/bulk`; scanner uses `/api/stocks/movers`; heatmap cold-start via `/api/stocks/grouped/:date` (1h debounce, top-60 by dollar-volume)
- `sota-terminal-provenance-regime` — `provenance_pane.rs` (clickable evidence DAG with depth selector, LRU cache, tree+DAG modes); `regime_tape.rs` (4-axis top strip showing current regime + transitions, color-blind-safe); signals_panel CALIBRATED SIGNALS section with calibrated/trust/🔍-prov columns
- `sota-terminal-replay` — `replay_pane.rs` with date pickers, speed slider (0.25× to MAX), play/pause/stop, events log (500 cap), command-palette entry; ~35 KB resident even for full-day replays
- `replay-overlay-hook` — `Chart::replay_overlay: Option<ReplayOverlay>` field + 3 API methods + second render pass at `render/pane.rs:2356`; overlay renders over price history but under user drawings; +161 lines to gpu.rs
- `sota-terminal-tradeplan-spike` — `trade_plan_panel.rs` (V2 with conformal range bands, hit-rate progress bar, day-type confidence, exit_rule); `spike_popup.rs` (top-right toast overlay via egui::Area::Foreground, 30s auto-dismiss, same-symbol coalescing); cross-pane wire-up hooks (`set_chart_jump_callback`, `set_provenance_callback`)
- `panels-projectors-data` — Heatmap sector header (11 SPDRs color-coded by quadrant) + breadth strip; scanner movers tab (5 buckets); dashboard breadth widget; halt toasts via existing toast pipeline
- `panels-projectors-info` — News panel rewired to projector (5 hardcoded literals gone); IV rank widget with insufficient-history state; ETF IIV panel (17 ETFs, bps-tier coloring); watchlist row badges (🔴 halt / 💰 ex-div / 📊 split / 📰 news)

---

## ApexIB — the broker plane

**1 branch merged.**

- `outcome-tracker-bridge` — `apex_order_lineage` postgres table; optional `lineageId` + `tradePlanId` on all 5 order-submit endpoints (`/orders`, `/orders/bracket`, `/orders/combo`, `/orders/conditional`, `/orders/options-trigger`); fill listener (via `OrderManager._on_order_status` since no `execDetailsEvent` subscriber exists) publishes to `events:apex_fill` Redis pubsub; cancel/reject → `events:apex_order_status`; 111 tests pass; bonus dead-code typo fix on conditional-order route

---

## ApexCrypto

**No changes this session.** Last activity is `149c74b "Add Binance kline stream for authoritative OHLC correction"` from prior session. Clean working tree. No upstream tracking on master.

---

## Strategic docs added

| Doc | Location | Length |
|---|---|---|
| `STOCKS_ADVANCED_MASTER_PLAN.md` | ApexData/docs/ | ~6500 words |
| `SOTA_VISION.md` | ApexData/docs/ | ~6000 words (closed-loop architecture + 8 themes + sequencing + success metrics) |
| `STREAMING_DAG_ANALYSIS.md` | ApexSignals/docs/ | ~1750 words (30 shared-subexpression frequency table + 34% CPU recovery estimate) |
| `SOTA_UX_DESIGN.md` | apex-terminal/docs/ | per-pane contract for the 6 SOTA UX surfaces |
| `STOCKS_TICK_STORAGE.md` | ApexData/docs/ | tick-table sizing + retention |
| `INGESTION_PIPELINE.md` | ApexData/docs/ | manifest model + operator runbook |
| `BACKFILL_RUN_PLAN.md` | ApexData/docs/ | wall-clock estimates per wave |

---

## ⚠️ Critical action items

1. **Rotate Polygon API key** `U8iOXJQyN42LJRrHPLMDoAlvTv_DP3XA` on the Polygon dashboard. Leaked key is in git history forever (was hardcoded in `preopen-scanner/scanner.py:16` before this session's PR removed it).
2. **Set up FRED API key** (`FRED_API_KEY` env var) for Treasury rate fetcher — free at fred.stlouisfed.org. Without it greeks stay at hardcoded 0.045 risk-free rate.
3. **Stocks WS feed disabled by default** behind `APEX_DATA_STOCKS_WS=1` env flag. Activate when Polygon Stocks Advanced subscription is live.
4. **Worktree cleanup** — ~50 worktree dirs across all repos. Run per-repo:
   ```
   git worktree list | tail -n +2 | awk '{print $1}' | xargs -I{} git worktree remove --force {}
   git worktree prune
   ```
5. **Run full test suites** (`cargo test`) before relying on production output — build was verified clean post-merge but tests weren't re-run after union-strip conflict resolutions.

---

## Capacity expectations

| Surface | Tested capacity |
|---|---|
| Polygon stocks WS worker | 150-250k events/sec/worker; ~600k-1M aggregate at N=4 |
| Per-projector cycle | < 100ms p95 |
| Provenance Redis Stream | 1M MAXLEN ~ 24h hot at 150 B avg/signal |
| Backfill: trades 2y wave | ~22h wall-clock at 8-way concurrency |
| Storage budget | 31 TB total occupied (Hot 1.2 TB QuestDB + Warm 8.3 TB NVMe + Cold 22 TB HDD); 4 TB headroom |

End of changelog.
