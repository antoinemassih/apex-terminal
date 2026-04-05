# Trading UX Enhancements — Spec

## 1. Watchlist Filter System

### Filter Types
- **Movement**: Filter symbols by daily change % (e.g., >2%, <-3%)
- **Volume**: Filter by relative volume (RVOL > 2x, > 3x)
- **Price**: Filter by price range ($10-$50)
- **Indicator**: Filter by RSI > 70 (overbought), RSI < 30 (oversold)
- **Sector**: Filter by GICS sector
- **Custom expression**: e.g., `change > 2 AND rvol > 1.5`

### Preset Views
Save filter combinations as named presets:
- "Movers" — |change%| > 2
- "High Volume" — RVOL > 2.5
- "Overbought" — RSI(14) > 70
- "Oversold" — RSI(14) < 30
- "Earnings This Week" — has_earnings within 5 days

### UI
- Filter bar at top of watchlist with dropdown for preset views
- Active filters shown as removable pills
- "Save View" button to create new presets
- Results update in real-time as prices move

### Data Requirements
- Need per-symbol RSI, RVOL, ATR, sector from ApexSignals or computed locally
- Daily change% and volume already available from watchlist price updates

---

## 2. Options IV Visual Indicators

### IV Rank / IV Percentile
Show where current IV is relative to the past year:
- **IV Rank**: (Current IV - 52wk Low IV) / (52wk High IV - 52wk Low IV) × 100
- **IV Percentile**: % of days in past year where IV was lower than today

### Visual Representation
For each options-eligible symbol in the watchlist/chain:

**IV Gauge**: A small semicircular gauge (like a speedometer):
- 0-30: Green (low IV — good for buying options)
- 30-50: Yellow (moderate)
- 50-70: Orange (elevated)
- 70-100: Red (high IV — good for selling options)

**HV vs IV**: Inline bar showing:
```
HV ████████░░░░ IV
   20%          35%
```
When IV > HV: options are "expensive" (amber tint)
When IV < HV: options are "cheap" (green tint)

### Unusual Activity Badge
On the options chain, highlight strikes with:
- Volume > 3x average daily volume: 🔥 badge
- OI change > 20% in one day: ⚡ badge
- Volume > OI (opening trades): 🆕 badge

### Data Requirements
- Historical IV data from ApexSignals (52-week range)
- Historical volatility (HV) computed from close prices
- Average daily volume per strike from ApexSignals
- Daily OI change tracking

---

## 3. Heatmap View (New Tab)

### Layout
A new tab after "Chain" in the watchlist sidebar:
- Grid of colored tiles
- Each tile = one symbol from the watchlist
- Tile size proportional to volume or market cap
- Color = daily change% (green = up, red = down, intensity = magnitude)

### Tile Content
Each tile shows:
```
┌─────────┐
│  AAPL   │
│ +1.23%  │
│ $195.42 │
└─────────┘
```

### Sorting/Grouping
- Group by sector (Technology, Healthcare, etc.)
- Sort by change%, volume, market cap
- Click a tile to switch the chart to that symbol

### Data
- Uses existing watchlist data (price, change%)
- Market cap / sector from ApexIB search endpoint

---

## 4. Full Order Book Panel

### Layout
A dedicated panel (toggle from toolbar) showing:

```
┌─ ORDERS ────────────────────────────────────┐
│ ACTIVE                                       │
│ ┌──────┬───────┬─────┬──────┬──────┬──────┐ │
│ │ Side │ Qty   │Price│ Type │Status│ Time │ │
│ ├──────┼───────┼─────┼──────┼──────┼──────┤ │
│ │ BUY  │ 100   │595.5│ LMT  │ WORK │ 10:30│ │
│ │ SELL │ 50    │600.0│ STP  │ WORK │ 10:31│ │
│ └──────┴───────┴─────┴──────┴──────┴──────┘ │
│                                               │
│ FILLED                                        │
│ │ BUY  │ 100   │594.8│ MKT  │ FILL │ 10:28│ │
│ │ SELL │ 100   │596.2│ LMT  │ FILL │ 10:45│ │
│                                               │
│ P&L: +$140.00 (+0.24%)                       │
└───────────────────────────────────────────────┘
```

### Features
- Active orders with modify/cancel buttons
- Filled orders with fill price, commission
- Cancelled orders (collapsed by default)
- Running P&L calculation
- Click an order to highlight it on the chart
- Right-click to modify/cancel

### Data
- Polled from ApexIB account endpoint (already running)
- Orders stored in Chart.orders for chart rendering

---

## 5. Comprehensive Hotkey Management Interface

### UI Dialog
Full-screen dialog with editable keybinding table:

```
┌─ HOTKEYS ──────────────────────────────────────────┐
│                                                      │
│ TRADING                                              │
│ Buy Market          [Ctrl] + [B]        [Edit]      │
│ Sell Market         [Ctrl] + [Shift] + [B]  [Edit]  │
│ Flatten All         [Ctrl] + [Shift] + [F]  [Edit]  │
│ Cancel All Orders   [Ctrl] + [Shift] + [Q]  [Edit]  │
│ Buy Limit @ Price   [Ctrl] + [L]        [Edit]      │
│                                                      │
│ DRAWING                                              │
│ Trendline           [T]                 [Edit]       │
│ H-Line              [H]                 [Edit]       │
│ Fibonacci           [F]                 [Edit]       │
│ Channel             [C]                 [Edit]       │
│                                                      │
│ NAVIGATION                                           │
│ Next Timeframe      [.]                 [Edit]       │
│ Prev Timeframe      [,]                 [Edit]       │
│ Toggle Magnet       [M]                 [Edit]       │
│                                                      │
│ CUSTOM                                               │
│ [+ Add Hotkey]                                       │
│                                                      │
│ [Import] [Export] [Reset to Defaults]                │
└──────────────────────────────────────────────────────┘
```

### Edit Mode
Click [Edit] → the row enters capture mode:
- "Press any key combination..."
- Captures the next key press as the new binding
- Shows conflicts if the key is already used
- [Save] [Cancel] buttons

### Storage
- JSON file: `hotkeys.json` alongside state file
- Import/Export for sharing between machines
- Reset to defaults button

### Categories
- **Trading**: Buy/Sell/Flatten/Cancel/Bracket
- **Drawing**: All 22 drawing tools
- **Navigation**: Timeframes, scroll, zoom, pane switching
- **Indicators**: Toggle specific indicators
- **Custom**: User-defined command strings

### Programmable Keyboard Support
- Support arbitrary key codes (for programmable keyboards with extra keys)
- MIDI input support (for control surfaces) — future
- Multiple keys can map to the same action (aliases)

---

## 6. Notional Order Entry (Enhanced)

### Real-Time Calculation
The current implementation calculates contracts from a dollar amount. Enhance:

- **Bid/Ask awareness**: Use the ask for buys, bid for sells
- **Slippage estimate**: Show "Est. fill: $X ± $Y" based on spread
- **Max contracts display**: "Max N contracts within budget"
- **Partial fill warning**: "Budget allows 3.7 contracts → 3 (leaving $XX unused)"
- **Greeks summary**: For options, show the total delta/theta exposure for the calculated position

### Quick Notional Buttons
Pre-set dollar amount buttons: [$500] [$1000] [$2000] [$5000] [$10000]
Click to instantly set the notional amount.

---

## Implementation Priority

1. ✅ Market session badge (done)
2. ✅ Notional order entry (done)
3. ✅ Trading hotkeys (done)
4. ✅ Bracket templates (done)
5. Watchlist filter system — next sprint
6. Options IV indicators — needs ApexSignals data
7. Order book panel — needs ApexIB order polling
8. Hotkey management interface — UI-heavy, standalone dialog
9. Heatmap view — new tab, standalone widget
10. Enhanced notional entry — iterative improvement
