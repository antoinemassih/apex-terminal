# Apex Terminal — Agent Context Document

## What This Is

A native GPU trading terminal built in Rust/egui/wgpu. 15K+ line main renderer (`gpu.rs`), immediate-mode rendering at 60fps. Connects to ApexSignals (59-engine analysis backend) and ApexIB (Interactive Brokers gateway) for real-time trading.

## Tech Stack

- **Language:** Rust
- **UI Framework:** egui 0.31 (immediate mode)
- **GPU:** wgpu for rendering
- **Window:** winit (native, no Electron)
- **Build:** `cargo build --bin apex-native` or `cargo run --bin apex-native`
- **Also has:** Tauri scaffold (`cargo tauri dev`) but primary target is the native binary

## Project Structure

```
apex-terminal/
├── src-tauri/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                    # Tauri entry
│       ├── native.rs                  # Native wgpu entry point
│       ├── signals_feed.rs            # WebSocket client to ApexSignals
│       ├── chart_renderer/
│       │   ├── mod.rs                 # Types: Bar, ChartCommand, Drawing, SignalZone, DivergenceMarker, etc.
│       │   ├── gpu.rs                 # MAIN RENDERER — 15K+ lines. ALL rendering, state, input handling.
│       │   ├── trading/
│       │   │   ├── mod.rs             # OrderLevel, OrderSide, OrderStatus, Position, IbOrder
│       │   │   └── order_manager.rs   # Centralized OrderManager (dedup, risk, state machine)
│       │   └── ui/
│       │       ├── mod.rs             # Module index
│       │       ├── style.rs           # tb_btn, color_alpha, dashed_line, close_button, separator, status_badge
│       │       ├── dom_panel.rs       # DOM sidebar with order management
│       │       ├── scanner_panel.rs   # Market scanner
│       │       ├── rrg_panel.rs       # Relative Rotation Graph (NEW — has demo data)
│       │       ├── indicator_editor.rs # Indicator styling (Option B grouped rows)
│       │       ├── object_tree.rs     # Consolidated drawings/indicators panel
│       │       ├── settings_panel.rs  # Settings (order defaults, risk, sessions)
│       │       ├── tape_panel.rs      # Time & Sales
│       │       ├── news_panel.rs      # News feed
│       │       ├── alerts_panel.rs    # Alert management
│       │       ├── journal_panel.rs   # Trade journal
│       │       ├── script_panel.rs    # Scripting/backtesting
│       │       ├── spread_panel.rs    # Options spread builder
│       │       ├── screenshot_panel.rs # Screenshot library
│       │       ├── watchlist_panel.rs # Watchlist
│       │       ├── orders_panel.rs    # Orders
│       │       ├── connection_panel.rs # IB connection
│       │       ├── overlay_manager.rs # Chart overlays
│       │       ├── command_palette.rs # Command palette
│       │       ├── hotkey_editor.rs   # Keyboard shortcuts
│       │       ├── trendline_filter.rs # Trendline filters
│       │       └── ...
│       └── ui_kit/
│           └── icons.rs              # Phosphor icon constants
```

## Key File: gpu.rs

This is the heart of the terminal. 15K+ lines containing:

### State Structs
- **`ChartState`** (~line 1050): Per-chart-pane state. Bars, drawings, indicators, oscillators, signal state (trend_health_score, exit_gauge_score, precursor_active, signal_zones, change_points, trade_plan, signal_demo_toggle).
- **`Watchlist`** (~line 13900): Global app state. Panes, symbol lists, panel open/close flags, RRG state, account data, layout.
- **`Theme`** (~line 49): Colors (bg, bull, bear, accent, dim, toolbar_bg, toolbar_border).

### Rendering Flow
1. `render_frame()` — called every frame
2. Top toolbar rendered first (timeframes, drawing tools, panel toggles)
3. Per-pane chart rendering (candles, drawings, indicators, oscillators)
4. Signal overlays (gauges, zones, change-points, trade plan) rendered ON TOP of chart
5. Side panels (DOM, scanner, tape, RRG) rendered via egui::SidePanel
6. Floating windows (news, journal, script, settings) rendered via egui::Window

### ChartCommand System
Commands come in via `mpsc::channel` from various sources (WebSocket, HTTP, user actions). Handled in `handle_command()` (~line 1450). Key variants:
- `LoadBars`, `AppendBar`, `UpdateLastBar` — bar data
- `SetDrawing`, `RemoveDrawing` — drawing management
- `PatternLabels`, `AutoTrendlines`, `SignificanceUpdate` — from ApexSignals
- `TrendHealthUpdate`, `ExitGaugeUpdate`, `SupplyDemandZones`, `PrecursorAlert`, `ChangePointMarker`, `TradePlanUpdate`, `DivergenceOverlay`, `RotationUpdate` — signal visuals (NEW)

### Signal Visual Rendering (~line 8443)
Currently renders (toggle with "SIG" toolbar button for demo):
- **Gauges** (top-right): Trend Health pill, Exit Gauge pill (escalating colors), Precursor badge (pulsing)
- **Supply/Demand zones**: Faint fills with edge lines, right-aligned abbreviated labels
- **Change-point markers**: Small diamonds on time axis + thin line through candle body
- **Trade plan**: Subtle green/red zone fill, dotted entry/target/stop lines, price axis labels, floating card (bottom-left) with contract name, R:R, move %, conviction

### Important Patterns
- `tb_btn(ui, label, active, t)` — toolbar button helper
- `tb_btn_tip(ui, label, active, t, tooltip)` — toolbar button with tooltip
- `color_alpha(color, alpha)` — alpha blending helper
- `dashed_line(painter, a, b, stroke, LineStyle)` — dashed/dotted line
- `py(price) -> f32` — price to Y coordinate
- `bx(bar_index) -> f32` — bar index to X coordinate
- `SignalDrawing::time_to_bar(time_ms, timestamps) -> f32` — timestamp to bar position
- egui 0.31: `Rounding` uses `u8`, `rect_stroke` takes 4 args (rect, rounding, stroke, StrokeKind::Outside)
- `PENDING_TOASTS` thread-local for toast notifications

## ApexSignals Backend (what feeds the terminal)

**59,349 lines Rust, 51 engine files, 1,038 tests.**

The signal engine at `C:\Users\USER\documents\development\ApexSignals` produces signals via WebSocket at `ws://localhost:8200/ws`. The terminal subscribes via `signals_feed.rs`.

### Signal Categories (all produce WebSocket events):

**Chart:** trendlines, patterns (50 candlestick + 42 chart), channels, divergences, supply/demand zones, volume zones, volume conviction, significance scoring

**Options:** options-underlying divergence (PROPRIETARY), precursor (smart money front-running), OI estimator (real-time), MM inventory (delta/gamma), implied flow (GEX/gamma flip/pin risk), IV surface (SVI/skew/deformation)

**Advanced Quant:** Shannon entropy, Hawkes cascade, copula analysis, Bayesian change-point, HMM regime, pairs/cointegration, transfer entropy (who leads whom), price impact (Kyle's lambda)

**Market Intel:** cross-market (VIX/yields/sectors), sector rotation (RRG), ETF flow, NOI (order imbalance), dark pool flow, FMV divergence, index composition (musical chairs/breadth), seasonality, order flow, unusual activity

**Fundamentals:** short interest/squeeze, earnings setup (0DTE/IV ramp/straddle), analyst catalyst

**Time-of-Day:** premarket (4AM-9:30AM), opening auction (9:30-10AM, dip-and-rip), power hour (2-4PM, 0DTE), post-market (4-8PM)

**Microstructure:** book pressure (bid/ask depth, pulling, spoofing, iceberg)

**Output:** trade plan (contract + entry + target + stop + R:R), signal combiner (meta-engine, Thompson sampling), exit gauge, alerts

### Key Signal Combinations for UI:
1. **Dip-and-Rip**: opening_auction(absorbed) + book_pressure(bids_holding) + noi(buy) + precursor(calls) → BuyTheDip
2. **EOD Squeeze**: power_hour(squeeze>70) + mm_inventory(short_gamma) + 0DTE gamma → Buy 0DTE calls
3. **Smart Money**: precursor + entropy(low) + change_point + implied_flow → enter before move
4. **Musical Chairs**: index_composition(rotation_high) + breadth(narrowing) + anchor(weakening) → reversal imminent

## What Needs UI Work

### Already Built (rendering code exists, needs polish):
- Signal gauges (trend health, exit gauge, precursor) — working but needs styling iteration
- Supply/demand zones — working, faint fills with labels
- Change-point markers — working, diamonds on time axis
- Trade plan overlay — working, card + lines
- RRG panel — working with demo data and time slider

### Needs Building:
1. **Master Signal Panel** — the "traffic light" that distills all 51 engines into one actionable view per symbol. Green/Yellow/Red with "SETUP", "WAIT", "EXIT NOW" labels. Should be always-visible in the chart corner.
2. **Opening Auction Widget** — special widget active 9:30-10:00 showing: dip detection, absorption ratio bar, squeeze pressure gauge, morning regime label, BuyTheDip/SellTheDip/Wait signal
3. **Power Hour Widget** — active 2:00-4:00 showing: 0DTE gamma gauge, pin strike indicator, MOC imbalance, squeeze/flush pressure, countdown timer, 0DTE trade idea card
4. **Pre-Market Widget** — active before open showing: gap size, pre-market volume, VIX term structure, futures bias, verdict
5. **Index Composition / Musical Chairs View** — breadth bars, component contribution chart, rotation frequency indicator, anchor highlight, distribution phase label
6. **Divergence Lines on Chart** — draw actual divergence lines on oscillator panels connecting the diverging peaks/troughs (DivergenceOverlay ChartCommand exists, rendering TODO)
7. **Transfer Entropy Arrows** — show which symbols are leading which (could be in a separate panel or arrows on watchlist)
8. **Pairs Panel** — show active cointegrated pairs, spread z-score, entry/exit signals
9. **Book Pressure Bar** — thin bar at the bottom of the chart showing real-time bid/ask depth imbalance
10. **Signal Timeline** — horizontal timeline below chart showing when each engine fired, color-coded by type

### User's UI Preferences (from past feedback):
- Fonts should be bigger (+4pt where small)
- Everything needs tooltips
- Hover states on all buttons
- Pointer cursor on clickable elements
- Gauges: pill-shaped, dark background, colored dot + thin progress bar (not fat colored bars)
- Zones: very faint fills (alpha 8-15), thin edge lines, abbreviated right-aligned labels
- Subtle over loud — signals should inform, not overwhelm
- Trade plan: floating card is good, but clicking should expand it
- The user is an experienced options scalper — speed and clarity over flashiness

## Infrastructure

- **ApexIB:** `https://apexib-dev.xllio.com` (78 endpoints, deployed K3s)
- **ApexSignals:** `ws://localhost:8200/ws` (not deployed yet, needs Massive.com data)
- **Redis:** `redis://:monkeyxx@192.168.1.89:6379/`
- **PostgreSQL:** `postgresql://postgres:monkeyxx@192.168.1.143:5432/apex`
- **Registry:** `192.168.1.71:5000`

## How to Run

```bash
cd apex-terminal/src-tauri
cargo run --bin apex-native
```

## Demo Mode

- Click **"SIG"** button in toolbar → toggles demo signal data (gauges, zones, trade plan, change-points)
- Click **"RRG"** button → opens Relative Rotation Graph side panel with demo sectors + time slider

## Files Most Likely to Edit

1. `gpu.rs` — ALL rendering. Signal overlays start around line 8443.
2. `mod.rs` — ChartCommand enum, types (SignalZone, DivergenceMarker, PatternLabel, etc.)
3. `ui/*.rs` — Individual panels
4. `style.rs` — Shared styling helpers
5. `signals_feed.rs` — WebSocket subscription (add new channels here)
