# Apex Terminal — Dev Inspector Plan

## Current State Audit

### Infrastructure: COMPLETE
- `mod.rs` — shared state, globals, begin/end_frame hooks, record/check_contract/emit
- `server.rs` — HTTP/1.1 server on :7891, 20 endpoints
- `assert_engine.rs` — logical (13 kinds) + layout (9 kinds) assertion evaluators
- `layout.rs` — Contract builder, snapshot/diff, checkpoint I/O, scenario metadata
- `input_queue.rs` — DevInput enum, 8 event types, drain_inputs_raw injection

### Known Bugs (BLOCKING)
| Bug | Location | Effect |
|---|---|---|
| `fps_above` reads `val["min"]` but val is scalar `5.0` | assert_engine.rs:129 | Checks 30fps not provided threshold |
| `active_symbol_equals` reads `val["symbol"]` but val is `"SPY"` | assert_engine.rs:138 | Always fails (sym="") |
| `active_timeframe_equals` reads `val["tf"]` but val is `"5m"` | assert_engine.rs:145 | Always fails |
| `pane_count_equals` reads `val["count"]` but val is `1` | assert_engine.rs:153 | Always fails (expect=0) |
| `has_bars` reads `val["min"]` but val is scalar | assert_engine.rs:161 | Works by accident (default=1) |

**Scenarios 02 and 03 currently always-fail.**

### Widget Instrumentation Coverage
Only ~9 records in the tree. Rest of app is dark.

**Instrumented:**
- `pane.{i}.chart_body` — canvas rect from core.rs
- `pane.{i}.symbol`, `pane.{i}.timeframe`, `pane.{i}.header` — synthetic from end_frame
- `pane_header.symbol` — real egui Response from painter_pane.rs
- `toolbar.timeframe_picker` — real egui Response from chart_controls.rs
- `toolbar.save_workspace` — real egui Response from top_nav.rs
- `status_bar.connection` — synthetic from end_frame

**Not instrumented (priority order):**
1. Top nav buttons: theme selector, style selector, workspace dropdown, layout picker, connection panel toggle
2. Watchlist item rows (per-symbol rows in each section)
3. Pane picker dialog items
4. Indicator editor dialog fields
5. Settings / hotkey editor dialog
6. Order entry form fields
7. Orders panel rows
8. Portfolio / Dashboard / Heatmap pane bodies
9. DOM ladder cells

### Scenario Coverage
| # | Name | What it tests | Status |
|---|---|---|---|
| 01 | health_check | fps, pane_count, no_open_dialogs | ✓ passes |
| 02 | reset | reset + symbol/timeframe assertions | ✗ broken (assert bug) |
| 03 | symbol_switch | SwapPaneSymbol, ChangeTimeframe | ✗ broken (assert bug) |
| 04 | chart_flags | SetChartFlag toggles, no violations | ✓ passes |
| 05 | watchlist_edit | WatchlistAddSymbol/Remove | ~ partial |

**Never tested:** indicators, dialogs, pane types, themes, multi-pane, layout assertions, input injection, watchlist CRUD, assert_poll, snapshot/regression

---

## Implementation Plan

### Phase 0 — Fix Assertion Scalar Bug (IMMEDIATE)
**File:** `src-tauri/src/dev_inspector/assert_engine.rs`

For each domain assertion that takes a simple scalar value, accept both scalar and object form:
```rust
// Before: val["symbol"].as_str().unwrap_or("")
// After:  val.as_str().or_else(|| val["symbol"].as_str()).unwrap_or("")
```

Assertions to fix: `fps_above`, `active_symbol_equals`, `active_timeframe_equals`, `pane_count_equals`, `has_bars`

Add new logical assertions:
- `indicator_count_equals` — `panes[i].indicator_count` from app_state JSON
- `pane_type_equals` — `panes[i].pane_type`
- `active_pane_equals` — `active_pane`
- `watchlist_section_count_equals` — `watchlist.section_count`
- `all_touch_targets_ok` — sweeps button/input widgets, checks min_side >= threshold
- `no_clipped_widgets` — sweeps widget tree, fails if any is_clipped=true
- `widget_value_equals` — checks widget `value` field by id

---

### Phase 1 — Debug Annotation Overlay System
**Purpose:** Paint colored rects/labels directly on the running app from the HTTP API. Tightest possible visual debugging loop — send a rect, see it appear on the live canvas instantly.

**New type in `mod.rs`:**
```rust
pub struct DebugAnnotation {
    pub id:          String,
    pub rect:        SerRect,
    pub label:       String,
    pub color:       [u8; 4],   // RGBA
    pub border_only: bool,
    pub layer:       u8,        // 0=below widgets, 1=above widgets
}
```

**`DevQueues` addition:** `pub annotation_queue: Vec<DebugAnnotation>`
**`DevSharedState` addition:** `pub active_annotations: Vec<DebugAnnotation>`

**`begin_frame()` change:** Drain annotation_queue → active_annotations (replacing, not appending, so POST /annotations is idempotent by id)

**New public function in `mod.rs`:**
```rust
pub fn render_annotations(ctx: &egui::Context)
```
Uses `ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("dev_ann")))` to paint over everything. Called from core.rs between `drain_and_dispatch()` and `end_frame()`.

**New server routes:**
- `GET /annotations` — returns active_annotations as JSON array
- `POST /annotations` — accepts `[{id, rect, label, color, border_only}]`, merges by id
- `DELETE /annotations` — clears all
- `DELETE /annotations/{id}` — clears one

**Usage example:**
```bash
# Mark the header zone blue
curl -X POST http://localhost:7891/annotations \
  -H 'Content-Type: application/json' \
  -d '[{"id":"hdr","rect":{"x":0,"y":0,"w":1920,"h":32},"label":"Header","color":[100,180,255,60],"border_only":false,"layer":1}]'
```

---

### Phase 2 — SVG Layout Diagram (Auto-chart of the widget tree)
**Purpose:** Visual schematic of widget geometry in the live inspector report — no screenshot needed. Every widget record rendered as a colored rect at actual pixel coordinates, scaled to fit the page.

**New endpoint:** `GET /layout-svg` — returns standalone SVG

**`/report` enhancement:** Embed the SVG inline below the stats table, with:
- Widget rects colored by role: `button`=steel-blue, `label`=slate, `canvas`=forest-green, `header`=gold, `status`=dimgray, `input`=coral
- Violation rects highlighted red with dashed border
- Annotation rects shown in their specified color
- Tooltip-like text labels on each rect (truncated to fit)
- Legend box

**`/report` also adds:**
- FPS sparkline (60-frame rolling bar chart as inline SVG)
- Violation count over time
- Per-scenario pass/fail summary (if scenarios have been run)

---

### Phase 3 — Design System Audit
**Purpose:** Structured audit of the widget tree against design system constraints. The core feedback tool for the upcoming styling sprint — run this after any CSS/style change to catch regressions.

**New file:** `src-tauri/src/dev_inspector/design_audit.rs`

**DesignAuditReport:**
```rust
pub struct DesignAuditReport {
    pub touch_targets:   AuditSection,   // all button/input widgets >= 28px min side
    pub clipping:        AuditSection,   // no widget should have is_clipped=true
    pub alignment:       AuditSection,   // buttons in same row should share top edge ±2px
    pub overflow:        AuditSection,   // all widgets should be within their clip_rect
    pub empty_rects:     AuditSection,   // no widget should have zero-area rect
    pub violations:      Vec<ContractViolationSummary>,
    pub total_widgets:   usize,
    pub clean:           bool,
}
```

**New endpoint:** `GET /design-audit` returns this report as JSON

**New assertion:** `design_audit_clean` — asserts `report.clean == true`

**Design token assertions (for future styling work):**
These read from widget records' `style_class` field once we instrument it:
- `spacing_between` — gap between two named widgets is within [min,max]
- `button_height_consistent` — all toolbar buttons have same height ±2px

---

### Phase 4 — Metrics Time Series
**Purpose:** Track FPS and violation counts over time to detect performance regressions introduced by styling changes.

**`DevSharedState` additions:**
```rust
pub fps_history:       std::collections::VecDeque<f32>,   // last 300 frames
pub violation_history: std::collections::VecDeque<usize>, // last 300 frames
```
Capped at 300 entries. Updated in `end_frame()`.

**New endpoint:** `GET /metrics`
```json
{
  "fps":        { "current": 60.1, "min": 58.2, "max": 61.0, "history": [60.1, 59.8, ...] },
  "violations": { "current": 0,    "total_ever": 3,           "history": [0, 0, 1, 0, ...] }
}
```

---

### Phase 5 — Widget Instrumentation Expansion
Add `record()` calls to the following render sites. Each is a single-line change after the widget expression.

**`top_nav.rs` targets (all inside `show()`):**
- Theme dropdown button → `toolbar.theme_btn`
- Style dropdown button → `toolbar.style_btn`
- Layout picker button → `toolbar.layout_btn`
- Connection panel toggle → `toolbar.conn_toggle`
- Workspace name label → `toolbar.workspace_name`

**`chart_controls.rs` targets:**
- Symbol type dropdown → `toolbar.symbol_type`
- Indicator add button → `toolbar.add_indicator_btn`

**Watchlist render targets:**
- Per-item row → `watchlist.item.{section_idx}.{item_idx}` (synthetic from end_frame since per-item responses aren't captured)
- Section header → `watchlist.section.{idx}.header`

**Design token fields to add to WidgetRecord:**
```rust
pub style_class: Option<String>,  // "primary", "ghost", "toolbar", "header", etc.
```
Populated at each `record()` call site once we have that info available.

---

### Phase 6 — New Scenario Files
All go in `dev/scenarios/`.

| File | Name | Tests |
|---|---|---|
| `06_indicator_lifecycle.json` | indicator_lifecycle | AddIndicator (RSI), indicator_count_equals, RemoveIndicator |
| `07_dialog_lifecycle.json` | dialog_lifecycle | Trigger pane_picker, dialog_open assertion, close it |
| `08_pane_type_switch.json` | pane_type_switch | ChangePaneType Dashboard, pane_type_equals, back to Chart |
| `09_design_audit.json` | design_audit_clean | Run /design-audit via http_get, assert design_audit_clean |
| `10_annotations_demo.json` | annotations_demo | Write annotations via http_post, assert annotation count > 0 |
| `11_layout_regression.json` | layout_regression | Save snapshot as "baseline_clean", later diff to detect shifts |
| `12_watchlist_crud.json` | watchlist_crud | WatchlistCreate, WatchlistAddSection, WatchlistSwitchActive, WatchlistDelete |
| `13_theme_cycle.json` | theme_cycle | SetThemeIdx 0..4, fps_above check each, no violations |
| `14_input_injection.json` | input_injection | Click on known widget rect, assert state change |
| `15_assert_poll.json` | assert_poll | Trigger symbol switch, poll for has_bars with 5s timeout |

---

### Phase 7 — Rust Integration Test Expansion
**File:** `src-tauri/tests/dev_inspector.rs`

Add tests for:
- All 15 scenarios in sequence
- `/design-audit` returns `clean: true` after reset
- `/metrics` returns non-empty history after 10 frames
- `/annotations` round-trip (POST → GET → DELETE → GET empty)
- `/layout-diff` returns `clean: true` against just-saved baseline

---

## Execution Checklist

- [x] Audit current state
- [x] **Phase 0:** Fix 5 assertion scalar bugs in assert_engine.rs + add 8 new assertion types
- [x] **Phase 1:** DebugAnnotation type + AnnotationOp in annotations.rs, annotation routes in server.rs, render_annotations in mod.rs, hook in core.rs
- [x] **Phase 2:** build_svg_layout in server.rs, embedded in /report with FPS/violation sparklines + role color legend
- [x] **Phase 3:** build_design_audit in server.rs, GET /design-audit endpoint, design_audit_clean assertion
- [x] **Phase 4:** fps_history/violation_history in mod.rs + end_frame update + GET /metrics
- [x] **Phase 5:** record() in top_nav.rs (6 sites), chart_controls.rs (indicators_btn + widgets_btn). Note: plan listed symbol_type/add_indicator_btn which don't exist as discrete buttons; mapped to actual toolbar.indicators_btn/toolbar.widgets_btn
- [x] **Phase 6:** 10 new scenario files (06–15). Note: 07_dialog_lifecycle replaced by 07_pane_type_switch; added 14_toolbar_layout_audit
- [x] **Phase 7:** Integration test expanded to all 15 scenarios + /metrics, /design-audit, /annotations round-trip (POST/GET/DELETE one/DELETE all), /layout-svg

---

## API Reference (complete after all phases)

| Method | Path | Description |
|---|---|---|
| GET | /health | Server alive check |
| GET | /state | Full app state JSON |
| GET | /widget-tree | All WidgetRecords this frame |
| GET | /layout-snapshot | `{id: rect}` flat map |
| GET | /layout-svg | SVG schematic of widget geometry |
| GET | /layout-diff?baseline= | Diff current tree vs named snapshot |
| GET | /report | HTML dashboard with embedded SVG |
| GET | /chart | Alias for /state |
| GET | /panes | panes array from app state |
| GET | /watchlist | watchlist object from app state |
| GET | /events | SSE stream of named events |
| GET | /scenario-list?tag= | Available scenario files |
| GET | /annotations | Active debug overlays |
| GET | /metrics | FPS + violation time series |
| GET | /design-audit | Structured design system audit report |
| POST | /reset | Reset app to baseline state |
| POST | /cmd | Dispatch one AppCommand |
| POST | /input | Inject one input event |
| POST | /input/sequence | Inject event array |
| POST | /assert | Evaluate assertion array → report |
| POST | /assert-layout | Evaluate layout assertion array |
| POST | /snapshot/save | Save current widget tree as named golden |
| POST | /checkpoint/save | Save app state snapshot |
| POST | /batch | Mini-batch of {method,path,body} requests |
| POST | /run-scenario | Run scenario file or inline body |
| POST | /annotations | Add/replace debug overlays (merge by id) |
| DELETE | /annotations | Clear all overlays |
| DELETE | /annotations/{id} | Clear one overlay |
