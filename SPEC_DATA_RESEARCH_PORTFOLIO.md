# Apex Terminal — Data & Research + Portfolio Analytics Spec

## Overview

Two interconnected systems:
1. **Data & Research Layer** — fundamental data, filings, economic calendar, analyst coverage overlaid on charts and accessible via widgets/sidebar
2. **Portfolio Analytics** — portfolio-level risk analysis, scenario simulation, margin tracking, presented as a dedicated view mode

---

## Part 1: Data & Research

### 1.1 Fundamental Data Overlay

**Data Points (per symbol):**
```
FundamentalData {
    // Valuation
    pe_ratio: f64,
    forward_pe: f64,
    pb_ratio: f64,
    ps_ratio: f64,
    ev_ebitda: f64,
    peg_ratio: f64,
    
    // Profitability
    gross_margin: f64,
    operating_margin: f64,
    net_margin: f64,
    roe: f64,
    roa: f64,
    roic: f64,
    
    // Growth
    revenue_growth_yoy: f64,
    earnings_growth_yoy: f64,
    revenue_growth_qoq: f64,
    
    // Per Share
    eps_ttm: f64,
    eps_forward: f64,
    dps: f64,               // dividend per share
    dividend_yield: f64,
    payout_ratio: f64,
    
    // Balance Sheet
    debt_to_equity: f64,
    current_ratio: f64,
    quick_ratio: f64,
    cash_per_share: f64,
    
    // Ownership
    institutional_pct: f64,
    insider_pct: f64,
    short_interest_pct: f64,
    short_ratio_days: f64,   // days to cover
    float_shares: u64,
    shares_outstanding: u64,
    
    // Meta
    market_cap: f64,
    enterprise_value: f64,
    sector: String,
    industry: String,
    
    // Earnings history
    earnings_history: Vec<EarningsReport>,
    revenue_history: Vec<RevenueReport>,
}

EarningsReport {
    date: i64,
    eps_actual: f64,
    eps_estimate: f64,
    surprise_pct: f64,
    revenue_actual: f64,
    revenue_estimate: f64,
}
```

**Chart Integration:**
- **Earnings markers** on x-axis — vertical line on earnings date, colored by beat/miss, tooltip shows EPS actual vs estimate and surprise %
- **Revenue bars** — optional overlay showing quarterly revenue as background bars behind candles (like volume bars but for revenue)
- **PE band overlay** — shaded channel showing where price "should" be at historical average PE, 1 std above, 1 std below. Shows overvalued/undervalued visually
- **Analyst price targets** — horizontal lines on chart at mean, high, low analyst targets. Labeled "PT Mean $185", "PT High $210"
- **Institutional ownership changes** — markers on chart when 13F filings show major buys/sells. Arrow up for accumulation, arrow down for distribution

**Widget:**
- Fundamentals Card widget (new ChartWidgetKind) — compact card showing key metrics: PE, EPS, Revenue Growth, Margin, Short Interest, Inst Ownership
- Styled as color-blocked sections like the design references — each metric gets its own mini cell with hero number

**Sidebar:**
- "Research" tab in the Analysis sidebar split-section
- Full fundamental data in a scrollable panel
- Comparable companies table — same metrics for sector peers side by side
- Historical valuation chart — PE over time, PS over time

### 1.2 SEC Filings Scanner

**Data:**
```
Filing {
    filing_type: FilingType,     // 13F, 10-K, 10-Q, 8-K, DEF14A, SC13D, Form4
    filer: String,               // institution or insider name
    date: i64,
    symbol: String,
    
    // For 13F (institutional holdings)
    shares_held: Option<i64>,
    shares_change: Option<i64>,
    change_pct: Option<f64>,
    market_value: Option<f64>,
    portfolio_pct: Option<f64>,
    
    // For Form 4 (insider transactions)
    insider_name: Option<String>,
    insider_title: Option<String>,
    transaction_type: Option<String>,  // Buy, Sell, Grant, Exercise
    shares_transacted: Option<i64>,
    price_per_share: Option<f64>,
    shares_owned_after: Option<i64>,
    
    // For 8-K (material events)
    event_description: Option<String>,
    
    url: String,                 // link to SEC filing
}

FilingType {
    Form13F,      // quarterly institutional holdings
    Form4,        // insider buy/sell
    Form10K,      // annual report
    Form10Q,      // quarterly report
    Form8K,       // material event
    FormSC13D,    // activist stake > 5%
    FormDEF14A,   // proxy statement
}
```

**UI — Filings Feed Panel:**
- Dedicated section in Research sidebar
- Filterable by type (13F, Form4, 8K, etc.)
- Sorted by date (newest first)
- Each filing card shows: type badge, filer, date, key metric (shares changed, $ value), link to SEC
- Color-coded: green for insider buys, red for insider sells, blue for 13F, amber for 8K

**Chart Integration:**
- Insider transaction markers on chart — small person icon at the bar where transaction occurred
- Green (buy) / red (sell) colored, tooltip shows insider name, title, shares, price
- 13F filing dates shown as subtle vertical markers
- SC13D (activist) shown as prominent markers — these move stocks

### 1.3 Economic Calendar

**Data:**
```
EconomicEvent {
    date: i64,
    time: i64,               // specific time (for intraday events)
    name: String,             // "FOMC Rate Decision", "CPI", "NFP"
    country: String,          // "US", "EU", "JP"
    importance: Importance,   // Low, Medium, High
    actual: Option<f64>,      // actual value (filled after release)
    forecast: f64,            // consensus estimate
    previous: f64,            // previous period value
    
    // Impact analysis
    historical_impact: HistoricalImpact,
}

HistoricalImpact {
    avg_move_spy_pct: f64,       // average SPY move on this event
    avg_move_vix_pct: f64,       // average VIX move
    directional_bias: f64,       // -1 to 1, historical skew
    vol_expansion_factor: f64,   // how much ATR typically expands
    affected_sectors: Vec<(String, f64)>, // sector sensitivity scores
}

Importance { Low, Medium, High, Critical }
```

**UI — Economic Calendar Panel:**
- Week view with events laid out by day
- Color-coded by importance: grey (low), amber (medium), red (high), pulsing red (critical)
- Countdown to next event in header
- Past events show actual vs forecast with surprise direction arrow
- Impact forecasting: "Historical average: SPY moves ±0.8% on CPI days"

**Chart Integration:**
- Vertical lines on chart at event times
- Importance determines line style: dashed (low), solid (medium), thick (high)
- Tooltip shows event name, forecast vs previous, historical impact
- Optional: shade the expected volatility zone around the event (like vol cone but event-specific)

**Widget:**
- Next Event Countdown widget — shows the next high-importance event, countdown timer, forecast, historical impact in a compact card
- Weekly Calendar widget — mini week grid with colored dots per day

### 1.4 Analyst Ratings Tracker

**Data:**
```
AnalystRating {
    analyst_name: String,
    firm: String,
    date: i64,
    rating: Rating,           // StrongBuy, Buy, Hold, Sell, StrongSell
    previous_rating: Option<Rating>,
    price_target: f64,
    previous_price_target: Option<f64>,
}

AnalystConsensus {
    mean_target: f64,
    high_target: f64,
    low_target: f64,
    median_target: f64,
    buy_count: u32,
    hold_count: u32,
    sell_count: u32,
    total_analysts: u32,
    consensus_rating: Rating,
    
    // Revision momentum
    upgrades_30d: u32,
    downgrades_30d: u32,
    target_revisions_up_30d: u32,
    target_revisions_down_30d: u32,
}
```

**Chart Overlay:**
- **Price target lines** — three horizontal lines on chart:
  - Mean target (solid, accent color, labeled "PT $185.00")
  - High target (dashed, bull color, labeled "PT High $210.00")  
  - Low target (dashed, bear color, labeled "PT Low $155.00")
- **Rating change markers** — on the x-axis at the date of each upgrade/downgrade
  - Up arrow for upgrades, down arrow for downgrades
  - Tooltip: "Goldman Sachs: Hold → Buy, PT $180 → $195"

**Widget:**
- Analyst Consensus widget — donut showing Buy/Hold/Sell distribution, mean target as hero number, revision momentum arrows
- Target Spread widget — horizontal bar showing current price position relative to low/mean/high targets

**Sidebar:**
- Full analyst list with ratings, targets, dates
- Sortable by firm, date, target
- Historical rating changes timeline

---

## Part 2: Portfolio Analytics

### 2.1 Portfolio View (new view mode)

A dedicated view that replaces the chart layout with a portfolio dashboard. Activated via toolbar button "Portfolio" or sidebar.

**Layout:**
```
┌─────────────────────────────────────────────┐
│  PORTFOLIO SUMMARY (hero metrics bar)       │
├──────────────────────┬──────────────────────┤
│                      │  RISK BREAKDOWN      │
│  POSITIONS TABLE     │  (sector pie, beta,  │
│  (scrollable)        │   correlation matrix) │
│                      │                      │
├──────────────────────┼──────────────────────┤
│  EQUITY CURVE        │  SCENARIO SIMULATOR  │
│  (with drawdown)     │  (what-if sliders)   │
│                      │                      │
└──────────────────────┴──────────────────────┘
```

### 2.2 Portfolio-Level Analytics

**Metrics:**
```
PortfolioAnalytics {
    // Summary
    total_value: f64,
    total_cost_basis: f64,
    total_unrealized_pnl: f64,
    total_realized_pnl_today: f64,
    total_pnl_pct: f64,
    
    // Risk
    portfolio_beta: f64,           // beta-weighted vs SPY
    portfolio_delta: f64,          // net delta (including options)
    portfolio_gamma: f64,
    portfolio_theta: f64,
    portfolio_vega: f64,
    value_at_risk_95: f64,         // 95% VaR (1-day)
    value_at_risk_99: f64,
    expected_shortfall: f64,       // CVaR — average loss beyond VaR
    
    // Concentration
    sector_weights: Vec<(String, f64)>,   // sector → % of portfolio
    top_5_concentration: f64,              // % of portfolio in top 5 positions
    herfindahl_index: f64,                 // concentration measure
    
    // Correlation
    avg_pairwise_correlation: f64,
    correlation_matrix: Vec<Vec<f64>>,     // NxN matrix of position correlations
    diversification_ratio: f64,
    
    // Income
    annual_dividend_income: f64,
    dividend_yield_portfolio: f64,
    next_ex_dividend: Vec<(String, i64, f64)>, // symbol, date, amount
    
    // Margin
    margin_used: f64,
    margin_available: f64,
    margin_utilization_pct: f64,
    maintenance_margin: f64,
    margin_call_distance_pct: f64, // how far from a margin call
    buying_power: f64,
}
```

### 2.3 Positions Table

Enhanced version of the current Positions Panel widget, but full-screen:

```
Position Row:
| Symbol | Qty | Avg Price | Current | P&L $ | P&L % | % of Port | Beta | Sector | Actions |

Features:
- Sortable by any column
- Groupable by sector, strategy, account
- Inline sparkline (7-day price trend)
- Color-coded P&L (green/red gradient by magnitude)
- Click row to open chart for that symbol
- Multi-select for batch actions (close all selected, hedge selected)
- Drag to reorder
- Filter bar: search by symbol, filter by sector, P&L direction
```

### 2.4 Sector Concentration (visual)

**Treemap:**
- Each position is a rectangle sized by market value
- Colored by daily P&L (green = up, red = down, intensity = magnitude)
- Grouped by sector with sector labels
- Click to drill into sector → see individual positions
- Like Finviz heatmap but for YOUR portfolio

**Sector Donut:**
- Donut chart showing % allocation by GICS sector
- 11 sectors with standard colors
- Center: total portfolio value
- Click sector to see positions in that sector

### 2.5 What-If Scenario Simulator

**Interface:**
```
Scenario {
    name: String,
    assumptions: Vec<Assumption>,
    result: ScenarioResult,
}

Assumption {
    // Market-wide
    spy_change_pct: f64,         // "What if SPY drops 5%?"
    vix_level: f64,              // "What if VIX goes to 40?"
    rate_change_bps: i32,        // "What if rates rise 25bps?"
    
    // Sector-specific
    sector_changes: Vec<(String, f64)>,  // "What if Tech drops 8%?"
    
    // Position-specific
    symbol_changes: Vec<(String, f64)>,  // "What if NVDA drops 15%?"
}

ScenarioResult {
    portfolio_value_after: f64,
    portfolio_pnl: f64,
    portfolio_pnl_pct: f64,
    per_position_impact: Vec<(String, f64, f64)>, // symbol, $ impact, % impact
    margin_impact: f64,
    margin_call_triggered: bool,
}
```

**UI:**
- Slider controls for each assumption (drag SPY from -10% to +10%)
- Real-time recalculation as sliders move
- Results shown as:
  - Portfolio value change (hero number, red/green)
  - Impacted positions list (sorted by $ impact)
  - Margin status indicator (safe → warning → danger)
- Preset scenarios: "Black Monday (-20%)", "Flash Crash (-5%)", "Melt-Up (+3%)", "Rate Shock (+50bps)"
- Save custom scenarios for re-evaluation

### 2.6 Value at Risk (VaR)

**Computation Methods:**
```
VaRMethod {
    Historical,              // use actual return distribution
    Parametric,              // assume normal distribution
    MonteCarlo(simulations), // simulate N paths using covariance matrix
}
```

**Output:**
- 1-day VaR at 95% and 99% confidence
- 5-day VaR (scaled)
- Component VaR — each position's contribution to total VaR
- Marginal VaR — how adding one more unit of a position changes portfolio VaR
- Conditional VaR (Expected Shortfall) — average loss in the worst 5% of scenarios

**Visualization:**
- Distribution chart: histogram of simulated portfolio returns with VaR lines marked
- VaR contribution bar chart: horizontal bars showing which positions contribute most risk
- Historical VaR backtest: did actual losses ever exceed VaR? (exceptions chart)

### 2.7 Margin Utilization Tracker

**Data:**
```
MarginStatus {
    // From broker (ApexIB)
    net_liquidation: f64,
    initial_margin_req: f64,
    maintenance_margin_req: f64,
    excess_liquidity: f64,
    buying_power: f64,
    sma: f64,                    // special memorandum account
    
    // Computed
    utilization_pct: f64,        // margin used / available
    cushion_pct: f64,            // excess / net liq
    distance_to_margin_call: f64, // how much portfolio can drop before call
    
    // Per position
    position_margin: Vec<(String, f64, f64)>, // symbol, initial margin, maintenance margin
    
    // Historical
    margin_history: Vec<(i64, f64)>, // timestamp, utilization over time
}
```

**Visualization:**
- Gauge widget showing margin utilization: green (< 50%), amber (50-80%), red (> 80%)
- Stacked bar showing margin breakdown by position (who's using the most margin)
- Timeline chart showing utilization over the last 30 days
- Alert thresholds: configurable notifications at 60%, 75%, 90% utilization

### 2.8 Correlation Matrix

**Visualization:**
- NxN grid of position correlations (computed from 60-day returns)
- Color-coded: blue (negative correlation) → white (no correlation) → red (positive correlation)
- Size of cell circle proportional to correlation magnitude
- Clustered: positions with high correlation grouped together
- Click a cell to see scatter plot of the two positions' returns

**Use Case:**
- Identify hidden concentration — "My 8 positions are actually 3 bets because they're all correlated"
- Find diversification opportunities — "Adding GLD would reduce my portfolio correlation by 12%"

---

## Part 3: Data Sources

### Free Tier (local compute, no API key needed)
- Yahoo Finance API — price data, basic fundamentals (PE, market cap, dividend yield)
- SEC EDGAR RSS — latest filings (8-K, 10-K, Form 4)
- Wikipedia/static — economic calendar dates (FOMC, CPI, NFP schedules)

### Premium Tier (API key required)
- Financial Modeling Prep or Polygon.io — full fundamentals, analyst ratings, institutional holdings
- Quiver Quant — insider trading, 13F filings, congress trades
- Unusual Whales or FlowAlgo — options flow data
- Trading Economics — economic calendar with forecasts and historical impact

### Self-Hosted (ApexSignals backend)
- Precomputed fundamental scores per symbol
- Earnings surprise model
- Analyst revision momentum
- Institutional accumulation/distribution signals
- Economic event impact forecaster (ML model)

---

## Implementation Priority

### Phase 1: Fundamental Data + Portfolio View
1. Fundamental data structs and Yahoo Finance data fetching
2. Earnings markers on chart (enhanced version of existing event_markers)
3. Analyst price target lines on chart
4. Fundamentals Card widget
5. Portfolio view mode — positions table + summary metrics
6. Sector concentration donut/treemap

### Phase 2: Research Layer
7. SEC filings feed (EDGAR RSS parser)
8. Insider transaction markers on chart
9. Economic calendar panel
10. Next Event Countdown widget
11. Research tab in Analysis sidebar

### Phase 3: Risk Analytics
12. Correlation matrix computation and visualization
13. Value at Risk computation (historical method first)
14. What-if scenario simulator
15. Margin utilization tracker (from ApexIB account data)
16. VaR contribution chart

### Phase 4: Advanced
17. PE band overlay on chart
18. Revenue trend bars overlay
19. Institutional ownership change markers
20. Portfolio equity curve with drawdown
21. Monte Carlo VaR
22. Economic event impact forecasting model
