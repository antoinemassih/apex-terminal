# Alternative Chart Types (Renko, Range, Tick)

## Summary
Add non-time-based chart types: Renko, Range bars, and Tick bars.
These are the 3 most popular alternative chart types across all platforms.

## Scope
- Add to CandleMode enum: Renko, RangeBar, TickBar
- Add to chart type dropdown in toolbar
- Settings per type: Renko brick size, Range bar size, Tick count
- Recompute bars from raw tick/OHLC data into the alternative format
- Render with appropriate visuals

## Files to modify
- `src/chart_renderer/gpu.rs` — add CandleMode variants, rendering, recomputation
- Toolbar dropdown — add new chart type options

## Chart Type Specs

### Renko
- Fixed-size bricks (e.g., $1.00 per brick)
- New brick when price moves brick_size from current brick close
- Up bricks = bull color, Down bricks = bear color
- No wicks, no time axis (bars are evenly spaced)
- Settings: brick_size (auto-calculate from ATR or user-specified)
- Rendering: filled rectangles, each brick_size tall

### Range Bars  
- New bar forms when price range (high-low) reaches threshold
- Each bar has exactly the same height
- Time axis is irregular (more bars during volatile periods)
- Settings: range_size (points/dollars)
- Rendering: standard OHLC candles but all same height

### Tick Bars
- New bar every N trades (e.g., every 500 trades)
- Volume-normalized view of price action
- Settings: tick_count (trades per bar)
- For now: approximate using volume-based splitting of existing OHLC bars
- Rendering: standard OHLC candles

## Data structures (add to Chart)
```rust
renko_brick_size: f32,    // 0.0 = auto (ATR-based)
range_bar_size: f32,      // 0.0 = auto
tick_bar_count: u32,      // default 500
alt_bars: Vec<Bar>,       // recomputed bars for alt chart types
alt_timestamps: Vec<i64>, // timestamps for alt bars
```

## Recomputation
- When chart type changes to Renko/Range/Tick, recompute from source bars
- Store in alt_bars/alt_timestamps
- Rendering uses alt_bars when mode is Renko/Range/Tick
- Indicators recompute on alt_bars
