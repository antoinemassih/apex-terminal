# `pane.rs` Split Plan

## State today

`src/chart/renderer/render/pane.rs` is 12,482 lines. The bulk — lines 88 to ~11,372 — is **one function**, `render_chart_pane(...)`. Five small helpers (most unused / dead code) follow. The remaining ~200 lines are entry points (`render_toolbar`, `draw_chart`).

This file is the single biggest blocker for multi-agent parallel work: any two agents touching it will conflict constantly.

## Why this can't be a mechanical sweep

`render_chart_pane()` holds **deep shared local state** that's threaded through every section:

- `chart: &mut Chart` — mutably borrowed across most segments
- `event_consumed: bool` — guards drag/click priority across PRIORITY 1 → 7
- Geometry: `rect`, `cw`, `ch`, `pt`, `pr`, `vs`, `vc`, `bs`, `n`, `min_p`, `max_p`
- Closures: `py`, `bx`, `pos_to_bar`, `pos_to_price` (close over rect/scale)
- Hit-test state: `hover_hit`, `hover_order`, `hover_play_line`
- Pointer state: `hover_pos`, `current_zone`, `in_chart_body`, `in_xaxis`, `in_yaxis`, `shift_held`

A naive function extraction means passing 12+ arguments to every extracted function and re-deriving borrow ordering at every cut point. Compile breaks are guaranteed if done in one pass.

## Constraint

Visual behavior must be **bit-identical** before and after each step. The chart paint is performance-critical; refactor regressions would manifest as flicker, mis-aligned overlays, or dropped frame rate.

## Strategy — five waves, each independently shippable

### Wave 0 — Set up the destination

Create `src/chart/renderer/render/pane/` directory with `mod.rs` re-exporting `render_chart_pane` from a new `pane/core.rs` (which initially just contains the function unchanged). All call sites still use `pane::render_chart_pane`. Zero behavior change.

**Verification:** build clean, app launches, all 9 panes render.

### Wave 1 — Extract free helpers (no shared state)

Move out:
- `drawing_icon`, `drawing_label`, `drawing_is_active`, `apply_draw_tool` → `pane/drawing_helpers.rs`
- `handle_deferred` → `pane/deferred.rs`
- `DRAW_CATEGORIES` const → `pane/drawing_helpers.rs`

These take no shared local state from `render_chart_pane`. Truly safe.

**Verification:** build clean. Smoke-test drawing tool picker.

### Wave 2 — Extract pure paint sub-systems (leaf calls)

Within `render_chart_pane`, identify segments that match the pattern:
```rust
// Pure paint: writes to `painter`, reads `chart` / `geometry`, no mutation,
// no event consumption, no closure escape.
painter.text(...);
painter.line_segment(...);
```

Candidates (roughly, from the audit):
- `paint_axis_labels(painter, chart, geom, t)` — lines ~1625-1745 (price + time axis)
- `paint_volume_bars(painter, chart, geom, t)` — lines ~1500-1620
- `paint_session_shading(painter, chart, geom, t)` — line ~1750-1900
- `paint_extended_hours(painter, chart, geom, t)` — line ~1900-2000

Each becomes a free function in `pane/paint.rs` taking `(&Painter, &Chart, &PaneGeometry, &Theme)`. The `PaneGeometry` struct collects `rect, cw, ch, pt, pr, bs, vs, vc, min_p, max_p` plus the `py / bx` closures lifted into methods.

**Verification after EACH extraction:** build, launch, eyeball the 9-pane grid in dark + light themes (Bauhaus). Diff the GPU draw call count if available.

### Wave 3 — Extract indicator overlays

Each of ~20 indicator overlays (SMA, EMA, RSI, MACD, Bollinger, Ichimoku, SAR, SuperTrend, Keltner, ADX, CCI, Williams %R, VWAP, etc.) is roughly 50-150 lines. They share the pattern: iterate visible indicators, compute, paint. Most read `chart.bars` immutably and write to `painter`.

Move each to `pane/indicators/<name>.rs`. They take `(&Painter, &Chart, &PaneGeometry, &Theme, &IndicatorSettings)`.

**Verification:** toggle each indicator on/off and visually confirm.

### Wave 4 — Extract drawing renderer

Drawing rendering (~1,800 lines, line ~3,936 onward) is the largest single section. It paints all drawings (trendlines, rectangles, fibs, etc.) AND handles hit-testing AND endpoint selection.

Split into:
- `pane/drawings/paint.rs` — pure paint per `DrawingKind`
- `pane/drawings/hit_test.rs` — `hit_drawing(pos, drawings) -> Option<(String, i32)>`
- `pane/drawings/interaction.rs` — drag/edit/select interaction (this stays in `render_chart_pane` initially because it mutates chart + consumes events)

**Verification:** select / drag / edit / delete a trendline. Test undo/redo.

### Wave 5 — Extract orders + options overlays

Same shape: pure paint goes out, mutating interaction stays in core.

- `pane/orders/paint.rs` — pending/filled/ghost order lines
- `pane/orders/dialogs.rs` — order entry / edit dialogs
- `pane/options/strikes.rs` — strike grid
- `pane/options/gamma.rs` — zero-gamma, HVL, vol smile

**Verification:** open order panel, place test order in paper mode, edit, cancel.

## Final shape

```
chart/renderer/render/
├── mod.rs
└── pane/
    ├── mod.rs                  (re-exports render_chart_pane, draw_chart)
    ├── core.rs                 (the now-shrunk main function, ~2,000 lines)
    ├── geometry.rs             (PaneGeometry struct + py/bx methods)
    ├── paint.rs                (axes, volume, session shading)
    ├── drawing_helpers.rs      (Wave 1)
    ├── deferred.rs             (Wave 1)
    ├── indicators/
    │   ├── mod.rs
    │   ├── sma.rs ema.rs rsi.rs macd.rs ...
    ├── drawings/
    │   ├── mod.rs
    │   ├── paint.rs
    │   ├── hit_test.rs
    │   └── interaction.rs
    ├── orders/
    │   ├── mod.rs
    │   ├── paint.rs
    │   └── dialogs.rs
    └── options/
        ├── mod.rs
        ├── strikes.rs
        └── gamma.rs
```

Estimated final `core.rs` size: ~2,000 lines (the interaction state machines + event-consumption flow). Down from 11,284.

## Cadence + ownership

This is a **dedicated agent task** — one agent owns the split, one wave per session, build + visual verification gate between waves. Five waves × ~half a day each = 2-3 days focused work.

Do NOT parallelize the waves — each wave touches `render_chart_pane` and its shape changes between waves. Sequential, with the verification gate, or it diverges.

## What unlocks once it's done

- Indicator team can add new indicators in their own files without touching the core
- Drawing tool team owns `drawings/` end-to-end
- Order team owns `orders/`
- Chart paint optimizations live in `paint.rs` with clear contract
- Performance profiling is per-module (currently the entire chart render is one trace span)

## Risks

- **Drift between waves.** If a non-split PR lands inside `render_chart_pane` between waves, the next wave's diff explodes. Mitigation: branch protection, wave PRs land same-day.
- **Closure capture.** `py` / `bx` close over `vs`, `vc`, `bs`, `rect`. Lifting to methods on `PaneGeometry` changes the call shape. Touch all ~400 call sites in one pass per wave to avoid mixed call shapes.
- **Borrow checker.** `chart: &mut Chart` is split-borrowed in places (e.g., reading `chart.bars` while mutating `chart.drawings`). Extractions need to preserve those split borrows by passing field-level references, not the whole `chart`.

## When to start

After the design-system pre-work lands (button consolidation, Header widget, Panel trait, token additions). Those are merged. This is the next foundational task — and once it's done, the multi-agent fleet can actually fan out.
