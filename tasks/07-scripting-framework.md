# Scripting / Backtesting / Scanner AI Framework

## Summary
Scaffold the infrastructure for an AI-driven scripting system.
Users describe indicators/strategies in natural language → AI generates and executes them.
This is the FRAMEWORK only — full AI integration comes later.

## Scope
- New `script_panel.rs` UI module — code editor panel
- Script engine scaffold: parse simple indicator expressions
- Strategy definition structure (entry/exit rules)
- Backtest runner scaffold (iterate bars, apply rules, collect results)
- Results display: equity curve, stats table
- Integration point for AI: prompt input → generated script
- Custom indicator from script: compute values per bar, display on chart

## Files to modify
- `src/chart_renderer/ui/mod.rs` — register script_panel
- `src/chart_renderer/ui/script_panel.rs` — NEW: script editor + results panel
- `src/chart_renderer/gpu.rs` — add script state, wire panel
- `src/chart_renderer/compute.rs` — add script indicator evaluation

## Architecture
```
User describes in natural language
  → (future: AI generates ApexScript)
  → Script parsed into AST
  → Evaluated per bar against OHLCV data
  → Results: indicator values OR strategy signals
  → Display: overlay on chart OR backtest report

ApexScript (simple expression language):
  close > sma(close, 20) AND rsi(close, 14) < 30
  → evaluates to bool per bar (for scanners/strategies)
  → or float per bar (for custom indicators)
```

## Data structures
```rust
struct Script {
    id: u32,
    name: String,
    source: String,        // ApexScript source code
    script_type: ScriptType,
    compiled: Option<CompiledScript>,
}

enum ScriptType {
    Indicator,  // outputs f32 per bar
    Strategy,   // outputs buy/sell signals
    Scanner,    // outputs bool per symbol
}

struct BacktestResult {
    trades: Vec<BacktestTrade>,
    equity_curve: Vec<f32>,
    total_pnl: f32,
    win_rate: f32,
    profit_factor: f32,
    max_drawdown: f32,
    sharpe: f32,
}

struct BacktestTrade {
    entry_bar: usize,
    exit_bar: usize,
    side: i8,      // 1=long, -1=short
    entry_price: f32,
    exit_price: f32,
    pnl: f32,
}
```

## UI Layout
- Split panel: left = code editor, right = results
- Toolbar: Run, Backtest, Save, AI Generate
- Results tabs: Chart Overlay | Backtest Report | Scanner Results
- Code editor: monospace, syntax highlighting (basic keyword coloring)
- AI input: text field at top "Describe your indicator or strategy..."
