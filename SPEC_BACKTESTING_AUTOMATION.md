# Apex Terminal — Backtesting & Automation Engine Spec

## Overview

Two interconnected systems that share a common strategy definition language:
1. **Backtest Engine** — runs strategies against historical data, produces performance analytics
2. **Automation Engine** — runs the same strategies live, triggering alerts or orders in real-time

A strategy written once can be backtested historically, then deployed live without modification.

---

## Part 1: Strategy Definition Language

### Strategy Structure

```
Strategy {
    name: String,
    universe: Universe,          // what instruments to run on
    timeframe: Timeframe,        // primary bar interval
    conditions: Vec<Condition>,  // entry/exit rules
    position_sizing: Sizing,     // how much to trade
    risk_management: Risk,       // stops, targets, max drawdown
    filters: Vec<Filter>,        // regime/market filters
    schedule: Schedule,          // when to run (market hours, specific sessions)
}
```

### Condition System (the core)

Conditions are composable boolean expressions built from:

**Price Primitives:**
- `close`, `open`, `high`, `low`, `volume`
- `close[N]` — N bars ago
- `highest(high, N)`, `lowest(low, N)` — rolling extremes
- `bar_count_since(condition)` — bars since a condition was true

**Indicator Primitives:**
- `sma(source, period)`, `ema(source, period)`, `wma(source, period)`
- `rsi(period)`, `macd(fast, slow, signal)`, `stochastic(k, d, smooth)`
- `atr(period)`, `adx(period)`, `cci(period)`, `williams_r(period)`
- `bollinger(period, std_dev)` — returns `.upper`, `.middle`, `.lower`, `.width`
- `vwap()`, `supertrend(period, multiplier)`
- `volume_sma(period)` — average volume

**ApexSignals Primitives (unique to us):**
- `trend_health()` — returns 0-100 score
- `trend_direction()` — returns -1, 0, 1
- `trend_regime()` — returns "trending", "ranging", "volatile"
- `exit_gauge()` — returns 0-100 urgency
- `precursor_active()` — boolean
- `precursor_score()` — 0-100
- `conviction()` — aggregate conviction 0-100
- `zone_strength(type)` — supply/demand zone health
- `pattern_detected(name)` — boolean for specific candlestick pattern
- `divergence_active(indicator)` — boolean
- `change_point_distance()` — bars since last regime change
- `signal_count()` — number of active signals

**Multi-Timeframe:**
- `mtf(timeframe, expression)` — evaluate any expression on a different timeframe
- Example: `mtf("1D", rsi(14)) > 50` — daily RSI above 50
- Example: `mtf("1h", close > ema(close, 20))` — hourly price above EMA

**Cross-Symbol:**
- `sym(symbol, expression)` — evaluate on another symbol
- Example: `sym("SPY", rsi(14)) < 30` — SPY RSI oversold
- Example: `sym("VIX", close) > 25` — VIX elevated

**Operators:**
- `>`, `<`, `>=`, `<=`, `==`, `!=`
- `AND`, `OR`, `NOT`
- `crosses_above(a, b)`, `crosses_below(a, b)` — crossover detection
- `is_rising(expr, bars)`, `is_falling(expr, bars)` — slope over N bars
- `percent_change(expr, bars)` — % change over N bars
- `consecutive(condition, N)` — condition true for N consecutive bars

### Entry/Exit Rules

```
Entry {
    direction: Long | Short | Both,
    conditions: Vec<Condition>,     // ALL must be true (AND logic)
    any_of: Vec<Condition>,         // at least ONE must be true (OR logic)  
    confirmation_bars: u32,         // wait N bars after signal before entering
    max_entries_per_day: u32,
    entry_type: Market | Limit(offset) | Stop(offset),
}

Exit {
    stop_loss: StopType,
    take_profit: TakeProfitType,
    trailing_stop: Option<TrailingStop>,
    time_exit: Option<TimeExit>,
    signal_exit: Vec<Condition>,    // exit when these conditions fire
    partial_exits: Vec<PartialExit>,
}
```

### Stop Types

```
StopType {
    Fixed(price_offset),           // $2 below entry
    ATR(multiplier, period),       // 2x ATR(14)
    Percent(pct),                  // 1.5% from entry
    SwingLow(lookback),            // below recent swing low
    Chandelier(atr_mult, period),  // chandelier exit
    Zone(zone_type),               // below nearest support zone (ApexSignals)
    None,                          // no stop (for scalps, time-based exits)
}
```

### Take Profit Types

```
TakeProfitType {
    Fixed(price_offset),
    ATR(multiplier, period),
    Percent(pct),
    RiskReward(ratio),             // 2:1 R:R from stop distance
    Scaled(Vec<(pct_qty, target)>), // partial exits: 50% at 1R, 25% at 2R, 25% at 3R
    TrailingActivation(trigger, trail), // activate trail after reaching trigger
    None,
}
```

### Position Sizing

```
Sizing {
    mode: FixedQty(qty) | FixedDollar(amount) | PercentEquity(pct) | RiskBased(risk_pct),
    // RiskBased: calculate qty so that if stop is hit, loss = risk_pct of equity
    max_position_pct: f32,         // never more than X% of equity in one position
    scale_with_conviction: bool,   // multiply size by conviction score / 100
}
```

### Filters (market regime guards)

```
Filter {
    name: String,
    condition: Condition,
    action: SkipEntry | ReduceSize(pct) | ExitAll,
}

// Examples:
// - Skip entries when VIX > 30
// - Reduce size by 50% when breadth < 40
// - Exit all when daily RSI < 25 (crash protection)
// - Only trade when trend_regime() == "trending"
```

---

## Part 2: Backtest Engine

### Architecture

```
BacktestEngine {
    strategy: Strategy,
    data: BarSeries,               // historical OHLCV
    initial_equity: f64,
    commission: CommissionModel,    // per-share, per-trade, or percentage
    slippage: SlippageModel,       // fixed, percentage, or volatility-based
    margin: Option<MarginModel>,   // for futures/forex leverage
}
```

### Execution Model

- **Bar-by-bar simulation** — iterate through bars chronologically
- **Intrabar execution** — for stop/limit orders, check if high/low would have triggered within the bar (not just close)
- **Order fill logic:**
  - Market orders fill at next bar open + slippage
  - Limit orders fill if price reaches limit level within the bar
  - Stop orders fill if price reaches stop level, with slippage
- **No lookahead bias** — indicators computed only with data available at that point
- **Survivorship bias warning** — flag if symbol was delisted during test period
- **Position tracking** — support multiple simultaneous positions (for portfolio strategies)

### Commission & Slippage Models

```
CommissionModel {
    PerShare(rate),        // $0.005/share
    PerTrade(flat),        // $4.95/trade
    Percentage(pct),       // 0.1% of trade value
    Tiered(Vec<(volume_threshold, rate)>), // volume-based tiers
}

SlippageModel {
    Fixed(cents),          // 1 cent per share
    Percentage(pct),       // 0.05% of price
    VolatilityBased(atr_fraction), // 0.1 * ATR
    None,                  // ideal fills (for comparison)
}
```

### Output: BacktestResult

```
BacktestResult {
    // Summary
    total_return_pct: f64,
    annualized_return_pct: f64,
    total_trades: u32,
    winning_trades: u32,
    losing_trades: u32,
    win_rate: f64,
    
    // Risk metrics
    max_drawdown_pct: f64,
    max_drawdown_duration_bars: u32,
    sharpe_ratio: f64,
    sortino_ratio: f64,
    calmar_ratio: f64,              // annual return / max drawdown
    profit_factor: f64,             // gross profit / gross loss
    
    // Trade analysis
    avg_win: f64,
    avg_loss: f64,
    avg_win_loss_ratio: f64,
    largest_win: f64,
    largest_loss: f64,
    avg_holding_period_bars: f32,
    avg_bars_in_winning_trade: f32,
    avg_bars_in_losing_trade: f32,
    max_consecutive_wins: u32,
    max_consecutive_losses: u32,
    
    // Equity curve
    equity_curve: Vec<(i64, f64)>,  // (timestamp, equity)
    drawdown_curve: Vec<(i64, f64)>,
    
    // Trade list
    trades: Vec<BacktestTrade>,
    
    // Monthly/weekly returns table
    monthly_returns: Vec<(String, f64)>,
    
    // Exposure
    time_in_market_pct: f64,
    avg_exposure_pct: f64,
    
    // Robustness
    monte_carlo: Option<MonteCarloResult>,
    walk_forward: Option<WalkForwardResult>,
}
```

### Advanced Analytics

**Monte Carlo Simulation:**
- Shuffle trade order 1000x, produce distribution of outcomes
- Show 5th/25th/50th/75th/95th percentile equity curves
- Answer: "Is this strategy robust or was the sequence lucky?"

**Walk-Forward Optimization:**
- Split data into in-sample (optimize) and out-of-sample (validate) windows
- Roll forward through time, re-optimizing at each step
- Show degradation of parameters over time
- Flag overfitting if in-sample >> out-of-sample performance

**Parameter Sensitivity (Heat Map):**
- Vary two parameters simultaneously (e.g., RSI period 5-30, MA period 10-100)
- Color-coded grid of Sharpe ratio for each combination
- Show whether the strategy works across a range or only at one specific setting

**Benchmark Comparison:**
- Compare strategy equity curve vs buy-and-hold
- Compare vs SPY, vs sector ETF
- Show alpha, beta, information ratio, tracking error

---

## Part 3: Automation Engine

### Architecture

```
AutomationEngine {
    strategies: Vec<LiveStrategy>,
    event_loop: EventLoop,         // receives bar updates, tick data, signal events
    order_router: OrderRouter,     // connects to trading backend (ApexIB)
    risk_governor: RiskGovernor,   // enforces portfolio-level risk limits
    audit_log: AuditLog,          // every decision recorded
}
```

### LiveStrategy

A strategy running in real-time:

```
LiveStrategy {
    strategy: Strategy,            // same definition as backtest
    state: StrategyState,          // current position, pending orders, P&L
    mode: Paper | Live | AlertOnly,
    
    // Paper: simulate fills locally, no real orders
    // Live: route orders to broker via ApexIB
    // AlertOnly: only fire alerts/notifications, no orders
}
```

### Event Loop

```
Events:
    BarClose(symbol, timeframe, bar)  — new bar completed
    TickUpdate(symbol, price, size)   — real-time price update
    SignalFired(signal_name, data)    — ApexSignals event
    OrderFilled(order_id, fill)      — execution confirmation
    TimerTick(interval)              — periodic check (e.g., every 30s)
    MarketOpen / MarketClose         — session events
    ManualOverride(action)           — user intervention
```

### Risk Governor (portfolio-level guardrails)

```
RiskGovernor {
    max_daily_loss: f64,           // stop all strategies if daily P&L < -$X
    max_daily_loss_pct: f64,       // or -X% of equity
    max_open_positions: u32,       // across all strategies
    max_correlated_positions: u32, // max positions in correlated symbols
    max_sector_exposure_pct: f64,  // no more than X% in one sector
    max_single_position_pct: f64,  // no more than X% in one name
    kill_switch: bool,             // manual emergency stop
    
    // Behavioral guards
    max_trades_per_hour: u32,      // prevent overtrading
    min_time_between_trades_secs: u32, // cooldown after a trade
    no_trading_after_consecutive_losses: u32, // pause after N losses in a row
}
```

### Audit Log

Every decision the engine makes is recorded:

```
AuditEntry {
    timestamp: i64,
    strategy_name: String,
    event_type: String,            // "ENTRY_SIGNAL", "ORDER_SENT", "RISK_BLOCKED", etc.
    details: String,               // human-readable explanation
    conditions_snapshot: Vec<(String, String)>, // what each condition evaluated to
    action_taken: String,
    order_id: Option<u64>,
}
```

This creates a complete audit trail: "At 10:32:15, RSI crossed below 30 (was 31.2, now 29.8), AND trend_health was 72 (> 60 threshold), AND VIX was 18.5 (< 25 filter). Entry signal generated. Position size: 150 shares (1% risk, ATR stop at $2.34). Order ID: 847291."

---

## Part 4: UI Integration

### Strategy Builder (visual)

- **Node-based editor** or **form-based builder** (not code-only)
- Drag conditions from a palette: "RSI", "EMA Cross", "Volume Spike", "ApexSignal"
- Connect with AND/OR logic
- Set parameters with sliders
- Live preview: highlight bars on the chart where signals would have fired

### Backtest Panel

- Dedicated sidebar section or modal
- **Run** button kicks off backtest (runs on background thread, not UI thread)
- Progress bar during execution
- Results displayed in tabs:
  - **Summary** — key metrics in a card grid (like the widget style)
  - **Equity Curve** — interactive chart (can overlay on price chart)
  - **Trade List** — scrollable table with entry/exit prices, P&L, duration
  - **Monthly Returns** — calendar heatmap
  - **Drawdown** — underwater chart
  - **Parameter Heatmap** — sensitivity grid
  - **Monte Carlo** — fan chart of simulated outcomes

### Automation Panel

- List of running strategies with status indicators
- Per-strategy: mode toggle (Paper/Live/AlertOnly), pause/resume, P&L
- Risk governor dashboard: daily P&L bar, position count, exposure gauges
- Audit log viewer: filterable, searchable
- **One-click deploy**: backtest result → "Deploy Live" button

### Chart Integration

- Backtest trades rendered on the chart as entry/exit markers
- Signal arrows on bars where conditions fired
- Equity curve as an overlay or separate pane
- Live strategy signals shown in real-time on the chart

---

## Part 5: Strategy Templates

Pre-built strategies users can load, modify, and backtest:

**Trend Following:**
- EMA Cross (9/21) with ADX filter
- Supertrend with volume confirmation
- Ichimoku Cloud breakout

**Mean Reversion:**
- RSI oversold bounce (RSI < 30, then crosses above 30)
- Bollinger Band squeeze + breakout
- VWAP reversion

**ApexSignals-Native:**
- Conviction > 70 with trend alignment > 80%
- Precursor alert + zone strength confirmation
- Divergence + change point within 5 bars

**Options-Aware:**
- Gamma squeeze detection (GEX flip + volume spike)
- Max pain magnet (price approaching max pain level)
- IV crush fade (sell after earnings IV collapse)

---

## Implementation Priority

1. **Strategy definition structs** — the data model
2. **Condition evaluator** — the expression engine that evaluates conditions against bars
3. **Backtest engine** — bar-by-bar simulator with the execution model
4. **Backtest UI** — results display, equity curve, trade list
5. **Strategy builder UI** — visual condition builder
6. **Automation event loop** — real-time strategy runner
7. **Risk governor** — portfolio-level guardrails
8. **Automation UI** — strategy management panel
9. **Advanced analytics** — Monte Carlo, walk-forward, parameter sensitivity
10. **Strategy templates** — pre-built strategies

Steps 1-4 can ship as v1. Steps 5-8 as v2. Steps 9-10 as v3.
