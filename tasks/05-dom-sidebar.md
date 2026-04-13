# DOM Full Sidebar Mode

## Summary
Enhance the existing trading ladder/DOM and add a full sidebar mode 
that opens as a panel on the left side of the chart.

## Scope
- New `dom_panel.rs` UI module for the full sidebar DOM
- Toggle: compact (current in-chart) vs full sidebar mode
- Full mode: left sidebar showing price ladder with bid/ask depth, volume at price, trade flow
- Level 2 depth visualization: bid/ask bars at each price level
- Current price highlighted, scrollable ladder
- One-click trading from DOM: click bid side to sell, ask side to buy
- Volume at price column (from volume profile data)
- Delta column (buy volume - sell volume per level)
- Cumulative delta running total
- Imbalance highlighting (when bid/ask ratio exceeds threshold)

## Files to modify
- `src/chart_renderer/ui/mod.rs` — register dom_panel
- `src/chart_renderer/ui/dom_panel.rs` — NEW: full DOM sidebar
- `src/chart_renderer/gpu.rs` — add dom_sidebar_open flag, layout adjustment for sidebar
- `src/chart_renderer/mod.rs` — add ChartCommand for depth data if needed

## Data structures
```rust
struct DomLevel {
    price: f32,
    bid_size: u32,
    ask_size: u32,
    volume: u64,       // total traded at this level
    delta: i64,        // buy_vol - sell_vol
    last_trade_side: i8, // 1=buy, -1=sell
}

// In Chart struct:
dom_sidebar_open: bool,
dom_levels: Vec<DomLevel>,
dom_tick_size: f32,     // price increment between levels
dom_center_price: f32,  // auto-centers on last price
```

## Layout
- When dom_sidebar_open: chart area shrinks, DOM takes ~180px on left
- DOM columns: [Delta] [Bid Size] [Price] [Ask Size] [Volume]
- Price ladder centered on current price, ~30 levels visible
- Color coding: bid=bull, ask=bear, large sizes highlighted
- Current price row highlighted with accent color
