# Dynamic Gemma UI — LLM-Driven Layout System

## Overview

A natural language interface for reorganizing the Apex Terminal UI in real-time using a fine-tuned small LLM (Gemma 2B). Users speak or type layout instructions, the model outputs config changes, and the app updates instantly — no recompile, no restart.

## Architecture

```
User speaks/types ──→ Fine-tuned Gemma 2B ──→ Config JSON diff ──→ Struct update in memory ──→ Next frame renders new layout
                           ↑                                              ↑
                     Current config                                 Hot-swap (16ms)
                     + schema context
```

## Performance

**Zero per-frame overhead.** The layout config is a plain Rust struct in memory. Reading `config.toolbar_height` is identical to reading a hardcoded `34.0_f32` — single CPU instruction, same machine code. The compiler doesn't distinguish between struct fields and constants.

The only cost is LLM inference (~200ms for a 2B quantized model on GPU), which only happens when the user explicitly asks for a change — not per-frame.

JSON is only used for:
- Saving layout presets to disk
- Persisting config on app exit
- LLM output parsing (one-time, on change)

During normal rendering: pure struct field reads, zero overhead vs current hardcoded approach.

## Layout Config Schema

```rust
struct LayoutConfig {
    // Toolbar
    toolbar_position: Dock,       // Top, Bottom, Hidden
    toolbar_height: f32,          // 28-42
    toolbar_sections: Vec<String>, // ["account","timeframe","drawing","layout"]
    toolbar_section_order: Vec<String>,

    // Sidebars (N sidebars, each with tabs)
    sidebars: Vec<SidebarConfig>,

    // Chart grid
    chart_grid: GridLayout,       // One, TwoH, TwoV, Three, Four, Six, Nine...

    // Floating panels
    floating_panels: Vec<FloatingPanel>,
}

struct SidebarConfig {
    dock: Dock,                   // Left, Right
    width: f32,                   // 140-500
    tabs: Vec<String>,            // ["watchlist","orders","analysis"]
    active_tab: usize,
    split: Option<f32>,           // vertical split ratio if multiple sections
}

struct FloatingPanel {
    id: String,                   // "dom", "order_entry", etc
    x: f32, y: f32,
    w: f32, h: f32,
    visible: bool,
}

enum Dock { Top, Bottom, Left, Right, Hidden }
```

## Dynamic Renderer

The render loop reads from the config struct every frame:

```rust
fn draw_chart(ctx: &egui::Context, config: &LayoutConfig, ...) {
    // Toolbar — position-aware
    match config.toolbar_position {
        Dock::Top => egui::TopBottomPanel::top("tb").exact_height(config.toolbar_height),
        Dock::Bottom => egui::TopBottomPanel::bottom("tb").exact_height(config.toolbar_height),
        Dock::Hidden => { /* skip */ },
        _ => {}
    }.show(ctx, |ui| {
        for section in &config.toolbar_sections {
            render_toolbar_section(ui, section, ...);
        }
    });

    // Sidebars — dynamic count and position
    for sidebar in &config.sidebars {
        let panel = match sidebar.dock {
            Dock::Left => egui::SidePanel::left(&sidebar.id),
            Dock::Right => egui::SidePanel::right(&sidebar.id),
            _ => continue,
        };
        panel.default_width(sidebar.width).show(ctx, |ui| {
            // Tab bar for this sidebar
            for tab in &sidebar.tabs {
                render_tab(ui, tab, ...);
            }
        });
    }

    // Floating panels
    for fp in &config.floating_panels {
        if fp.visible {
            egui::Window::new(&fp.id)
                .fixed_pos([fp.x, fp.y])
                .fixed_size([fp.w, fp.h])
                .show(ctx, |ui| { render_panel(ui, &fp.id, ...); });
        }
    }

    // Chart grid — uses remaining space
    egui::CentralPanel::default().show(ctx, |ui| {
        render_chart_grid(ui, &config.chart_grid, ...);
    });
}
```

Each UI component (watchlist, orders, DOM, analysis, etc.) is a self-contained widget that can render itself given a `Ui` reference. The layout system assigns positions; the widgets don't care where they are.

## Fine-Tuned Gemma 2B

### Why a small model works

- Input: natural language layout instruction (1 sentence)
- Output: JSON diff (~5-10 fields)
- Schema: ~30 fields with bounded value ranges
- Task: pure structured mapping, no reasoning needed
- A 2B parameter model is massively overqualified for this

### Training data generation

1. Write 50 manual examples covering core patterns:
```jsonl
{"input": "move watchlist to the left", "output": {"sidebars.0.dock": "left"}}
{"input": "make the toolbar thinner", "output": {"toolbar_height": 28}}
{"input": "hide the order entry", "output": {"floating_panels.order_entry.visible": false}}
{"input": "put orders and analysis in the same sidebar", "output": {"sidebars.0.tabs": ["orders","analysis"]}}
{"input": "float the DOM panel", "output": {"floating_panels.dom": {"visible": true, "x": 100, "y": 200, "w": 220, "h": 400}}}
{"input": "switch to 3 chart layout with watchlist on left", "output": {"chart_grid": "Three", "sidebars.0.dock": "left"}}
{"input": "make everything more compact", "output": {"toolbar_height": 28, "sidebars.0.width": 200}}
{"input": "use my scalping layout", "output": {"preset": "scalping"}}
```

2. Use Claude/GPT-4 to generate 5,000 synthetic variations (different phrasings of the same operations)
3. Fine-tune Gemma 2B on this dataset
4. The model becomes a pure "instruction → JSON diff" translator

### Inference prompt (at runtime)

```
Schema: {toolbar_height: f32, toolbar_position: "top"|"bottom"|"hidden", sidebars: [{dock: "left"|"right", width: f32, tabs: [string]}], chart_grid: "One"|"TwoH"|"TwoV"|"Three"|"Four"|"Six"|"Nine", floating_panels: [{id: string, visible: bool, x: f32, y: f32, w: f32, h: f32}]}

Current: {"toolbar_height": 34, "sidebars": [{"dock": "right", "width": 240, "tabs": ["watchlist"]}], "chart_grid": "Four"}

User: "move watchlist to left and add orders below it"

Output:
```

Model outputs: `{"sidebars": [{"dock": "left", "width": 240, "tabs": ["watchlist", "orders"], "split": 0.6}]}`

### Deployment

- **Option A: llama.cpp** — compiled as C library, called from Rust via FFI. Battle-tested, supports all quantization formats.
- **Option B: candle** — Hugging Face's pure Rust ML framework. No FFI, native Rust, but less mature.
- **Model size:** ~1.5GB quantized (Q4_K_M). Ships with the app or downloads on first use.
- **VRAM usage:** ~2GB for a 2B model. Shares GPU with wgpu chart rendering.
- **Inference time:** ~100-200ms on modern GPU for this output length.

## User interaction modes

### 1. Natural language (LLM-powered)
Text input in the app — type "move watchlist to left" → instant layout change.

### 2. Direct manipulation (no LLM)
- Drag a panel tab to another sidebar → updates config
- Drag panel edge to resize → updates config width
- Right-click panel header → "Float" / "Dock Left" / "Dock Right" / "Hide"
- Both methods update the same config struct

### 3. Layout presets
- Save current layout: stores config JSON to disk
- Load preset: one-click or voice ("switch to scalping layout")
- Share presets as JSON files between users

## Build plan

1. **Config struct + serialization** — define LayoutConfig, JSON serde, defaults (~0.5 day)
2. **Refactor renderer to be config-driven** — abstract each panel into a self-contained widget, layout engine assigns positions from config (~2-3 days, the hardest part)
3. **Hot-swap mechanism** — atomic config replacement, UI reacts next frame (~0.5 day)
4. **Drag-and-drop** — direct manipulation updates config (~1 day)
5. **LLM integration** — llama.cpp or candle, load model, inference pipeline (~1 day)
6. **Training data + fine-tuning** — generate examples, train Gemma 2B (~1 day)
7. **Text input UI** — command bar in the app, send to LLM, apply result (~0.5 day)

Total: ~6-7 days of work.

## Key decisions

- **Config struct, not JSON, at runtime** — JSON is only for disk I/O. The renderer reads from a Rust struct = zero overhead.
- **Each panel is a self-contained widget** — it renders given a `Ui`, doesn't know or care where it's placed.
- **Config changes are atomic** — swap the entire struct, not individual fields. Prevents partial/inconsistent states.
- **LLM outputs diffs, not full configs** — smaller output = faster inference, less chance of corruption.
- **Fine-tuned model, not prompt-engineered** — reliable structured output, no few-shot needed at inference, faster.
