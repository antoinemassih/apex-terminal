# SOTA UX Design

> Living document. This branch (`replay-overlay-hook`) only adds the §4.2
> Replay Overlay Hook section below. Other sections will be expanded by the
> sibling `sota-terminal-replay` branch when it lands. The hook is INACTIVE
> on `main` until the `ReplayScrubber` pane is merged.

## 4. Chart Overlays

### 4.2 Replay Overlay Hook

The chart renderer (`src-tauri/src/chart/renderer/gpu.rs`) now exposes a
single, scoped hook for rendering "replay" OHLCV bars on top of the live
chart in a distinct color. This is the integration point for the
`ReplayScrubber` pane on branch `sota-terminal-replay`.

#### Types

```rust
pub struct ReplayOverlay {
    pub bars: Vec<Bar>,
    pub timestamps: Vec<i64>,   // ms since epoch, parallel to bars
    pub color: egui::Color32,   // default: orange (0xFF, 0xA5, 0x00)
    pub label: String,          // e.g. "Replay: 2026-04-15 10:30:00"
}

impl Chart {
    pub fn set_replay_overlay(&mut self, overlay: ReplayOverlay);
    pub fn append_replay_bar(&mut self, bar: Bar, t_ms: i64);
    pub fn clear_replay_overlay(&mut self);
}
```

The `Chart::replay_overlay: Option<ReplayOverlay>` field is `None` by
default and after `clear_replay_overlay()`. While `Some`, a second render
pass in `render_chart_pane` (in `src-tauri/src/chart/renderer/render/pane.rs`)
draws the overlay candles and a top-left "REPLAY MODE" badge.

#### Render contract

- Overlay bars share the live chart's time axis. Each overlay timestamp is
  matched against `chart.timestamps` via binary search; bars whose
  timestamps fall outside the live window are skipped.
- Overlay candles are drawn with the live-bar mesh primitives — no new
  render path is introduced. The live render path is untouched.
- Z-order: overlay candles render **after** live candles but **before**
  drawings and annotations, so they sit over the price history while user
  drawings remain on top.
- Color: `overlay.color` is used at alpha 160 for the body and 220 for the
  wick. Default is a distinct orange (`ReplayOverlay::DEFAULT_COLOR`).
- Label: when active, "REPLAY MODE: \<label\>" is rendered as a colored
  pill in the top-left corner of the chart region (just below the toolbar).

#### Scrubber-side usage

The `ReplayScrubber` pane should:

1. On scrub: call `chart.set_replay_overlay(ReplayOverlay { bars, timestamps, color, label })`.
2. On streaming bar over WS: call `chart.append_replay_bar(bar, t_ms)`.
3. On scrub exit / stop: call `chart.clear_replay_overlay()`.

The `TODO(overlay)` marker in `replay_pane.rs` on the
`sota-terminal-replay` branch is where these calls should go.

#### Scope guards

- Single hook point in the render loop (between live candle submission and
  the drawings pass).
- No changes to the live-bar rendering.
- Color is **per-overlay**, not hardcoded — the scrubber can pick a
  different color when comparing multiple replays.
- The `ReplayScrubber` pane is on a sibling branch and is **not** modified
  by this branch.
