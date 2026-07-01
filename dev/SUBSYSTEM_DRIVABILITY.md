# Subsystem drivability map — for exhaustive scenario coverage

Discovery (2026-07-01) of every subsystem the user named: playbooks, RRG, scanners,
options chains, order entry, DOM — what's driveable via the dev_inspector command bus,
what's observable in `app_state`, and what harness extension each needs.

Pattern (same as the earlier gamma/strikes/watchlist work): a feature can **render**
yet be **UI-only** (no command to open/drive it) and **absent from `app_state`** (no
capture to assert on). Extension = add a *safe* harness command + capture field + assertion.

## SAFETY (order entry) — NON-NEGOTIABLE
- `paper_mode` defaults to **true** → nothing reaches the broker (ApexIB). `order_manager.rs:688`.
- The `AppCommand` order variants (`CancelOrder`, `CancelAllOrders`, `ClearOrderHistory`,
  `PlaceSelectedOrders`, `CancelSelectedOrders`) mutate ONLY the visual `pane.orders`
  (System A) — they never call `OrderManager`/broker. `commands.rs:352-412`.
- The ONLY live-submit path is `OrderManager::submit()` → `broker.submit()` →
  `POST {APEXIB_URL}/orders` (`broker.rs:353`). Reached only by `submit_order`/
  `submit_ib_order`/`submit_*_order`/`confirm_order` and armed+advanced Buy/Sell/REVIEW.
- Harness rules: NEVER emit `PlaceAllDraftOrders`/`PlaceAllDraftAlerts`. NEVER call any
  `submit_*`/`confirm_order`. Assert `paper_mode_on` + `no_live_orders` (mgr has 0 in
  Working/PendingSubmit/Filled) in EVERY order scenario as belt-and-suspenders proof.

## Per-subsystem

| Subsystem | Renders? | Open/drive via cmd today | Observable today | Extension needed |
|---|---|---|---|---|
| DOM ladder | yes | ❌ UI-only `dom.sidebar_open` | ❌ | `SetDomSidebar`/`SetDomOpen` cmd + capture `chart.dom` (levels/best bid-ask/is_live) + asserts. **Auto-populates 61-row mock when open — no feed needed.** |
| Order entry | yes | partial (visual-only cancel/clear) | ❌ pane.orders not captured | `SeedDraftOrder` (System A only), `SetOrderPanel` + capture order counts + mgr paper/armed/blocked + safety asserts |
| Scanner | yes | ❌ UI-only `scanner_open`; live gainers/losers only | ❌ | `SetScannerOpen`, `SeedScannerResults` (deterministic pool) + capture + `scanner_filter_correct` (recompute `apply_scanner`) |
| RRG | yes | ❌ UI-only `rrg_open` | ❌ | `SetRrgOpen`/`SetRrgTail`/`SetRrgTimeOffset` + capture effective sectors. **Demo-only but DETERMINISTIC (11 SPDR sectors)** → great assertion target |
| Heatmap pane | yes | ✅ `ChangePaneType Heatmap` | pane_type ✅ / cells ❌ | `SeedHeatmapCells` + capture `heatmap_cell_count` |
| Dashboard/Portfolio/Spreadsheet | yes | ✅ `ChangePaneType` | pane_type ✅ / internals ❌ | (optional) capture spreadsheet dims |
| Strikes overlay | yes | ✅ `SetChartFlag ShowStrikesOverlay` (auto-fetches placeholder; needs bars) | ✅ strikes_call/put_count | none — already works without feed |
| Gamma overlay | yes | ⚠️ `SetChartFlag ShowGamma` sets bool ONLY (no synth) | ✅ gamma_level_count etc | **`SynthGamma` cmd** — extract synth from `chart_controls.rs:592-617`. Fixes the 3 persistent reds (feed :8412 absent) |
| Options chain table | yes | ❌ sidebar CHAIN tab UI-only (placeholder works) | ❌ chain_0dte/far not captured | (optional) `SetChainTab` + capture chain counts. NOTE: per-strike greeks NOT implemented anywhere; only aggregate GEX is real |
| OptionsSentiment | yes (stub "not connected") | ✅ `ChangePaneType` → Dashboard | headless pane_type name | none (stub by design) |
| OptionsFlow | yes (hardcoded mock) | ✅ `ChangePaneType` → Dashboard | headless pane_type name | none (mock by design) |

## Key file anchors
- AppCommand enum + handlers: `chart/renderer/commands.rs` (order variants 143-157/352-412; SetChartFlag/ChangePaneType 586-614)
- Command bus parse: `dev_inspector/server.rs:2046-2198`
- app_state snapshot: `dev_inspector/mod.rs:909-962`
- Pane overlay capture: `dev_inspector/canvas.rs:43-58,424-432` (+ mirror `server.rs:36-52`)
- Assertions: `dev_inspector/assert_engine.rs`
- DOM: `DomPanelState` gpu.rs:2311; mock `dom_panel.rs:50`; toggle `core.rs:928`
- Orders: `order_manager.rs` (is_paper_mode 2763, is_armed 2532, is_trading_blocked 2721); `snapshot::current()` snapshot.rs:39; `OrderLevel`/`OrderStatus` trading/mod.rs:38
- Scanner: `ScanResult` gpu.rs:5596, `apply_scanner` scanner_panel.rs:30; fields gpu.rs:5966
- RRG: `RRGSector` rrg_panel.rs:34, `demo_sectors` rrg_panel.rs:48; fields gpu.rs:6003
- Gamma synth: `chart_controls.rs:573-617`
- Strikes overlay: `Chart.overlay_calls/puts` gpu.rs:2646; fetch `fetch.rs:768`
