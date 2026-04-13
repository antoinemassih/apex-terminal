# Session Shading (Pre/Post Market)

## Summary
Add appearance controls for out-of-hours session visualization.
Users can dim bars outside regular trading hours and shade the background.

## Scope
- New section in Settings panel: "Sessions"
- Controls: RTH bar opacity (0-100%), pre-market bar opacity, after-hours bar opacity
- Background shading: toggle + color picker for pre/post market background tint
- Session time definitions (default US equities: RTH 9:30-16:00 ET)
- Visual: bars outside RTH rendered with reduced opacity, background tint behind those bars
- Vertical session break lines (dashed) at market open/close

## Files to modify
- `src/chart_renderer/gpu.rs` — add session fields to Chart struct, rendering logic in candle drawing
- `src/chart_renderer/ui/settings_panel.rs` — add Sessions section with controls
- `src/chart_renderer/gpu.rs` save_state/load_state — persist session settings

## Data structures (add to Chart)
```rust
// Session settings
session_shading: bool,           // master toggle
rth_start_hour: u8,              // 9 (9:30 = 9*60+30 minutes)
rth_start_min: u8,               // 30
rth_end_hour: u8,                // 16
rth_end_min: u8,                 // 0
pre_market_bar_opacity: f32,     // 0.3 default
after_hours_bar_opacity: f32,    // 0.3 default
session_bg_tint: bool,           // shade background
session_bg_color: String,        // "#1a1a2e" default
session_bg_opacity: f32,         // 0.15 default
session_break_lines: bool,       // vertical lines at open/close
```

## Rendering
- When drawing candles, check if bar timestamp falls within RTH
- If outside RTH: multiply candle colors by pre/after opacity
- If session_bg_tint enabled: draw filled rect behind out-of-hours regions
- If session_break_lines: draw vertical dashed lines at session boundaries
- For crypto: no session shading (24/7 market) — auto-detect via is_crypto()
