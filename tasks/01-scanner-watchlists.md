# Scanner Watchlists + Market Movers

## Summary
Add a "Scanner" system that creates automated watchlists. Market Movers is a preset scanner.
Scanner watchlists are not manually managed — they auto-populate based on scan criteria.

## Scope
- New `scanner_panel.rs` UI module
- Scanner section in the watchlist sidebar (tab or toggle between Manual / Scanner watchlists)
- Market Movers preset: Top Gainers, Top Losers, Most Active (by volume), Biggest Gap Up/Down
- "Save as Watchlist" button to snapshot a scanner result into a regular saved watchlist
- Scanner definition: name, conditions (price change %, volume threshold, etc.)
- Data source: Yahoo Finance bulk quote API for now (fetch top movers)
- Scanners refresh on a timer (every 30s-60s)

## Files to modify
- `src/chart_renderer/ui/mod.rs` — register scanner_panel
- `src/chart_renderer/ui/scanner_panel.rs` — NEW: scanner UI
- `src/chart_renderer/ui/watchlist_panel.rs` — add scanner tab/toggle
- `src/chart_renderer/gpu.rs` — add scanner state to Watchlist struct, wire panel call
- `src/data.rs` — add bulk quote/movers fetch function

## Data structures
```rust
struct ScannerDef {
    name: String,
    preset: Option<String>, // "market_movers_gainers", etc. None = custom
    conditions: Vec<ScanCondition>,
    sort_by: ScanSort,
    limit: usize,
    refresh_secs: u32,
}

enum ScanCondition {
    MinChange(f32),      // % change > threshold
    MaxChange(f32),
    MinVolume(u64),
    MinPrice(f32),
    MaxPrice(f32),
    MinRvol(f32),        // relative volume
}

enum ScanSort { ChangeDesc, ChangeAsc, VolumeDesc, PriceDesc }

struct ScanResult {
    symbol: String,
    price: f32,
    change_pct: f32,
    volume: u64,
    last_updated: i64,
}
```

## Built-in Market Movers presets
1. "Top Gainers" — top 20 by % change (positive)
2. "Top Losers" — top 20 by % change (negative)  
3. "Most Active" — top 20 by volume
4. "Gap Up" — top 20 by gap % (pre-market)
5. "Gap Down" — bottom 20 by gap %

## UI Design
- Scanner tab in watchlist panel sidebar
- Each scanner shows as a collapsible section with auto-updating symbols
- Click symbol → loads chart (same as regular watchlist)
- "+" button to create custom scanner
- Preset scanners shown by default
- "Save as Watchlist" button per scanner → copies current results to a manual watchlist
