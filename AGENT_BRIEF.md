# Apex Terminal — Agent Brief

## What is this?
Professional native GPU trading terminal built in Rust/wgpu/egui. Renders candlestick
charts at 60fps with indicators, drawings, order management, and real-time data feeds.

## Current State (v0.9.0)
- `src-tauri/src/chart_renderer/gpu.rs` — main renderer (~15K+ lines)
- `src-tauri/src/chart_renderer/trading/order_manager.rs` — centralized order management
- `src-tauri/src/chart_renderer/ui/` — 20+ extracted UI panels
- `src-tauri/src/chart_renderer/compute.rs` — indicator computation
- `src-tauri/src/chart_renderer/mod.rs` — types, ChartCommand enum

## Key Features Built
- 18+ indicators with per-component styling and presets
- 22 drawing types with significance tooltips
- Enterprise OrderManager: dedup, risk validation, full ApexIB wiring
- DOM sidebar with order management
- Scanner watchlists, spread builder, scripting framework scaffold
- Bar replay, log scale, Renko/Range/Tick charts
- Session shading, dark pool overlay, event markers
- Hit-test highlighting, pane maximize, consolidated object tree
- Paper mode, alerts panel, screenshot library

## Data Sources
- **ApexIB**: `https://apexib-dev.xllio.com` — IB broker (orders, positions, bars)
- **ApexCrypto**: `ws://192.168.1.56:30840` — crypto real-time feed
- **ApexSignals**: `http://localhost:8100` — analysis engine (gamma, patterns, alerts)
- **Yahoo Finance**: fallback bar data

## Integration Points with ApexSignals (TODO)
- Wire candlestick pattern labels to chart overlay
- Wire alert rules creation/monitoring to backend
- Wire auto trendlines as SignalDrawings
- Wire significance scores from backend (replace local estimate)
- Wire hit detection events for flash highlighting
- Wire bounce/break classification feedback

## Architecture Notes
- All order writes go through `OrderManager` with dedup + risk checks
- UI panels are separate modules in `ui/` — read `ui/mod.rs` for the list
- Chart state is in `Chart` struct, global state in `Watchlist` struct
- Drawings persist to SQLite via `drawing_db`
- State saves/loads to JSON in AppData/Local/apex-terminal/
