# Event Markers Overlay

## Summary
Add an overlay system for displaying events on the chart:
earnings, dividends, splits, economic events.
Data integration comes later — this is the visual/display system.

## Scope
- New overlay toggle: "Events" in overlay menu
- Event types: Earnings, Dividend, Split, Economic (FOMC, CPI, NFP)
- Rendering: vertical marker lines + icon/label at the top of chart
- Color coding by event type
- Hover tooltip showing event details
- Placeholder data for testing (mock earnings dates)
- ChartCommand for receiving event data from backend

## Files to modify
- `src/chart_renderer/gpu.rs` — add EventMarker struct, event rendering, toggle
- `src/chart_renderer/mod.rs` — add ChartCommand::EventData variant
- Overlay menu — add "Events" toggle

## Data structures
```rust
struct EventMarker {
    time: i64,           // timestamp
    event_type: EventType,
    label: String,       // "Q4 Earnings", "FOMC", "$0.82 div"
    details: String,     // hover tooltip content
    impact: i8,          // -1=bearish, 0=neutral, 1=bullish (for coloring)
}

enum EventType {
    Earnings,    // icon: calendar, color: accent
    Dividend,    // icon: dollar, color: green
    Split,       // icon: split arrows, color: blue
    Economic,    // icon: flag, color: orange
}
```

## Rendering
- Vertical dashed line at event timestamp
- Small colored icon at top of chart area
- Label text below icon (rotated or horizontal)
- Earnings: "E" marker with EPS beat/miss color
- Dividend: "$" marker with amount
- Economic: flag icon with event name
- Hover any marker → tooltip with full details
- Events outside visible range: small indicator at chart edge
