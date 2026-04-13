# Dark Pool Overlay Scaffold

## Summary
Add visual infrastructure for displaying dark pool / off-exchange prints on the chart.
Data will come from ApexSignals later. This is the display scaffold only.

## Scope
- New overlay type: "Dark Pool" in the overlay menu (under Gamma)
- Visual: large circle markers at price levels where dark pool prints occur
- Size proportional to block size, color-coded (buy=green, sell=red, unknown=gray)
- Horizontal level lines at significant dark pool price levels
- Summary tooltip on hover showing: price, size, time, exchange
- Toggle in overlay menu
- Placeholder data generator for testing (random large prints near current price)

## Files to modify
- `src/chart_renderer/gpu.rs` — add DarkPoolPrint struct, rendering in overlay section, toggle flag
- `src/chart_renderer/mod.rs` — add ChartCommand::DarkPoolData variant
- `src/chart_renderer/ui/watchlist_panel.rs` or overlay menu — add "Dark Pool" toggle

## Data structures
```rust
struct DarkPoolPrint {
    price: f32,
    size: u64,        // share count
    time: i64,
    side: i8,         // 1=buy, -1=sell, 0=unknown
    exchange: String,  // "DARK", "FINRA", etc.
}
```

## Rendering
- Circle at (bar_x, price_y) with radius = log(size) scaled
- Color: bull color for buys, bear color for sells, dim for unknown
- Alpha proportional to recency (fade older prints)
- Horizontal dashed line at prices with aggregate dark pool volume > threshold
- Label showing total dark pool volume at that level
