# Apex Terminal — Closed Source / Open Source & Extensibility Framework

## Overview

Apex Terminal follows the **VS Code model**: a closed-source high-performance core engine distributed as a compiled binary, with a fully open-source extension API and plugin system that lets developers customize and extend every aspect of the terminal using Python, Rust, or simple config files.

---

## Architecture

```
+-----------------------------------------------------+
|  OPEN SOURCE -- apex-terminal/extensions/             |
|                                                       |
|  +-----------+ +-----------+ +-----------+           |
|  | Themes    | |Indicators | | Widgets   |  <- TOML/ |
|  | .toml     | | .py/.rs   | | .py/.rs   |    Python/|
|  +-----------+ +-----------+ +-----------+    Rust    |
|       |             |              |                  |
|  +----+-------------+--------------+----------------+ |
|  |          Extension API (public crate)             | |
|  |  apex_sdk::                                       | |
|  |    Theme, Indicator, Widget, Overlay,             | |
|  |    Automation, Template, Layout, Command           | |
|  +---------------------------------------------------+ |
|                      |                                |
|                      | trait impls + FFI               |
+----------------------|--------------------------------+
|  CLOSED SOURCE       |  (distributed as binary)       |
|                      v                                |
|  +---------------------------------------------------+ |
|  |            Core Engine (apex-core)                 | |
|  |  GPU renderer, order manager, data feeds,          | |
|  |  chart math, egui host, persistence                | |
|  +---------------------------------------------------+ |
+-------------------------------------------------------+
```

---

## Two Crates

### 1. `apex-core` (closed, binary distribution)

What's inside:
- GPU chart renderer (wgpu/egui-based, 60fps candlestick rendering)
- Order manager + risk engine (dedup, validation, IBKR wiring)
- Data feed connections (IBKR, crypto WebSocket, Yahoo Finance)
- Drawing system + PostgreSQL persistence
- Pane/layout/tab system
- The egui host render loop
- Style/design token system

**Distributed as:** compiled binary (like VS Code's `code` binary or Sublime Text's engine)

### 2. `apex-sdk` (open source, published crate)

The public API that extensions program against:

```rust
// apex_sdk/src/lib.rs

/// Register a custom indicator
pub trait Indicator: Send + Sync {
    fn name(&self) -> &str;
    fn category(&self) -> IndicatorCategory;
    fn params(&self) -> Vec<Param>;           // UI auto-generates editor
    fn compute(&self, bars: &[Bar], params: &ParamValues) -> IndicatorOutput;
    fn render_hint(&self) -> RenderHint;      // overlay vs oscillator, colors, line style
}

/// Register a custom theme
pub trait ThemeProvider {
    fn themes(&self) -> Vec<ThemeDef>;        // colors, can reference design tokens
}

/// Register a custom widget (side panel, floating window, toolbar section)
pub trait Widget: Send + Sync {
    fn name(&self) -> &str;
    fn location(&self) -> WidgetLocation;     // SidePanel, FloatingWindow, ToolbarSection
    fn ui(&mut self, ctx: &WidgetContext, ui: &mut egui::Ui);
}

/// Register an overlay (painted on the chart canvas)
pub trait Overlay: Send + Sync {
    fn name(&self) -> &str;
    fn paint(&self, ctx: &ChartContext, painter: &egui::Painter);
}

/// Register an automation (event -> action pipeline)
pub trait Automation: Send + Sync {
    fn name(&self) -> &str;
    fn triggers(&self) -> Vec<Trigger>;       // price cross, time, indicator signal
    fn execute(&mut self, event: &TriggerEvent, ctx: &mut ActionContext);
}
```

---

## Extension Types

### A. Config-based (zero code, TOML/JSON)

**Themes** -- `extensions/themes/midnight-blue.toml`:
```toml
[theme]
name = "Midnight Blue"
bg = "#0a0e1a"
bull = "#3e78b4"
bear = "#b4413a"
dim = "#646973"
accent = "#3e78b4"
toolbar_bg = "#080c15"
toolbar_border = "#1c2028"
```

**Templates** -- already working, formalized JSON schema for indicator/toggle presets

**Layouts** -- predefined pane arrangements:
```toml
[layout]
name = "Scalper 6-Pack"
grid = "3x2"
panes = [
  { symbol = "ES", timeframe = "1m", template = "Scalping" },
  { symbol = "NQ", timeframe = "1m", template = "Scalping" },
]
```

### B. Python Extensions (most accessible)

Using PyO3 or subprocess-based approach:

```python
# extensions/indicators/vwap_anchored.py
from apex_sdk import indicator, Bar, IndicatorOutput

@indicator(name="Anchored VWAP", category="overlay")
def compute(bars: list[Bar], anchor_bar: int = 0) -> IndicatorOutput:
    cum_vol, cum_pv = 0.0, 0.0
    values = []
    for bar in bars[anchor_bar:]:
        cum_vol += bar.volume
        cum_pv += bar.close * bar.volume
        values.append(cum_pv / cum_vol if cum_vol > 0 else bar.close)
    return IndicatorOutput(values=values, color="#4a9eff")
```

```python
# extensions/automations/trailing_stop.py
from apex_sdk import automation, Trigger, Action

@automation(name="Trailing Stop")
def on_price_update(price, position, params):
    trail_pct = params.get("trail_pct", 1.0)
    if position.side == "long" and price < position.high * (1 - trail_pct/100):
        return Action.close_position(position)
```

### C. Rust Extensions (highest performance)

```rust
// extensions/indicators/custom_rsi/src/lib.rs
use apex_sdk::prelude::*;

#[apex_indicator(name = "Smoothed RSI", category = "oscillator")]
pub struct SmoothedRsi {
    #[param(default = 14, min = 2, max = 200)]
    period: usize,
    #[param(default = 3, min = 1, max = 20)]
    smoothing: usize,
}

impl Indicator for SmoothedRsi {
    fn compute(&self, bars: &[Bar], _params: &ParamValues) -> IndicatorOutput {
        let rsi = compute_rsi(bars, self.period);
        let smoothed = compute_ema(&rsi, self.smoothing);
        IndicatorOutput::oscillator(smoothed)
            .with_levels(vec![30.0, 70.0])
            .with_color("#9b59b6")
    }
}
```

### D. Widget Extensions

```python
# extensions/widgets/order_flow_heatmap.py
from apex_sdk import widget, WidgetLocation

@widget(name="Order Flow Heatmap", location=WidgetLocation.SIDE_PANEL)
class OrderFlowHeatmap:
    def ui(self, ctx, ui):
        ui.label("Order Flow Heatmap")
        for level in ctx.dom_levels:
            bid_pct = level.bid_size / ctx.max_size
            ask_pct = level.ask_size / ctx.max_size
            ui.horizontal(lambda: (
                ui.colored_bar(bid_pct, color="green"),
                ui.label(f"${level.price:.2f}"),
                ui.colored_bar(ask_pct, color="red"),
            ))
```

---

## Extension Discovery & Loading

```
~/.apex-terminal/
├── extensions/
│   ├── themes/           <- .toml files, hot-reloaded
│   ├── indicators/       <- .py or Rust .so/.dll
│   ├── widgets/          <- .py or Rust .so/.dll
│   ├── overlays/         <- .py or Rust .so/.dll
│   ├── automations/      <- .py or Rust .so/.dll
│   └── templates/        <- .json files (already exists!)
├── registry.toml         <- enabled/disabled extensions
└── extension-cache/      <- compiled Rust extensions
```

**Loading flow:**
1. On startup, core scans `extensions/` directories
2. TOML configs parsed and registered immediately
3. Python scripts loaded via embedded Python or subprocess
4. Rust crates loaded as dynamic libraries (`.dll`/`.so`)
5. Each extension gets a sandboxed `ExtensionContext` with:
   - Read-only access to market data
   - Read-only access to chart state
   - Write access only through the Action API (place order, draw overlay, show notification)

---

## Key Design Principles

1. **Core owns the render loop** -- extensions can't block the GPU thread. Python runs in a worker thread; results are sent via channel.

2. **Declarative params** -- indicator parameters are declared via attributes/decorators. The core auto-generates the editor UI (slider, color picker, dropdown). No UI code needed in extensions.

3. **Hot-reload for configs** -- themes, templates, layouts reload on file change. Code extensions require restart (or explicit reload command).

4. **Marketplace-ready** -- each extension is a directory with a `manifest.toml` (name, version, author, dependencies). Could support `apex install <extension>` from a registry.

5. **Security sandbox** -- Python extensions can't access filesystem or network directly. They go through the SDK's vetted API. Rust extensions are trusted (user compiles them).

---

## Closed vs Open Boundary

| Closed (apex-core binary) | Open (apex-sdk + extensions) |
|---|---|
| GPU chart renderer | Custom indicators |
| Order manager + risk engine | Custom themes |
| Data feed adapters | Custom widgets/panels |
| Drawing system | Custom overlays |
| Pane/tab/layout engine | Automations/bots |
| Persistence layer | Templates/layouts |
| egui host + style system | Keyboard shortcuts |
| Signal processing | Alert conditions |

---

## Comparison to Existing Models

| Product | Model | Our Approach |
|---|---|---|
| VS Code | Closed Electron core + open extension API | Same model, GPU-native instead of Electron |
| TradingView | Closed platform + Pine Script | Similar but with Python/Rust instead of custom lang |
| Sublime Text | Closed core + Python plugin API | Similar, but we also support Rust plugins |
| Bloomberg Terminal | Fully closed | We open the extension layer |
| thinkorswim | Closed + thinkScript | We use standard languages (Python/Rust) |

---

## Implementation Phases

### Phase 1: Foundation (config-based)
- TOML theme loading from `extensions/themes/`
- JSON template loading (already done via `templates/`)
- Layout presets from TOML

### Phase 2: Rust SDK
- Publish `apex-sdk` crate with trait definitions
- Dynamic library loading for Rust indicator extensions
- Auto-generated parameter editor UI

### Phase 3: Python SDK
- Embedded Python via PyO3 or subprocess bridge
- `apex_sdk` Python package wrapping the Rust traits
- Indicator + automation decorators

### Phase 4: Marketplace
- Extension manifest format (`manifest.toml`)
- CLI: `apex install`, `apex list`, `apex update`
- Optional hosted registry for community extensions
