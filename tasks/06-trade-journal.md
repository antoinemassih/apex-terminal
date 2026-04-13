# Trade Journal Placeholder

## Summary
Add a Trade Journal panel accessible from the toolbar.
This is a placeholder page — full functionality comes later.

## Scope
- New `journal_panel.rs` UI module
- Menu item in toolbar (Icon::NOTEBOOK or similar)
- Placeholder panel with:
  - Title: "Trade Journal"
  - Coming soon message with feature preview list
  - Mock layout showing what it will look like
- Panel opens as a sidebar (like watchlist)

## Files to modify
- `src/chart_renderer/ui/mod.rs` — register journal_panel
- `src/chart_renderer/ui/journal_panel.rs` — NEW: placeholder panel
- `src/chart_renderer/gpu.rs` — add journal_open flag to Watchlist, toolbar button, wire panel

## Placeholder Content
Show a styled "coming soon" panel with planned features listed:
- Auto-log trades from IB
- Entry/exit with chart snapshot
- P&L tracking per trade
- Win rate, profit factor, expectancy
- Filter by: symbol, strategy, date range
- Tags and notes per trade
- Equity curve visualization
- Performance by day of week / time of day
