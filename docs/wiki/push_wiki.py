#!/usr/bin/env python3
"""Push Performance & Compilation Strategy + Common Pattern pages to Outline wiki."""
import requests

TOKEN = "ol_api_kVLe87UcI2WxFM4ufPxuwJ8F5VjDpdUPlYcVRY"
COL = "e7c90412-0054-440d-9c5f-eeba80e6fa06"
PARENT = "52b655ae-8471-4d3c-b189-dab9cc16ce58"
URL = "https://wiki.xllio.com/api/documents.create"
HEADERS = {"Content-Type": "application/json", "Authorization": f"Bearer {TOKEN}"}

PERF_PAGE = """# Performance & Compilation Strategy

## The Problem

Python's GIL + interpreter overhead means:
- **~100-1000x slower** than Rust for tight compute loops (indicator math)
- **~10ms+ startup latency** per call via subprocess
- **Memory overhead** -- embedded Python adds ~30MB to process

For a 60fps GPU terminal where each frame budget is **16.6ms**, Python in the hot path is not acceptable.

## Solution Tiers

### Tier 1: Rust Dynamic Libraries (Ship First)

Pure performance, zero overhead. Users write Rust, compile to `.dll`/`.so`, core loads natively.

```rust
#[apex_indicator(name = "Custom VWAP", category = "overlay")]
pub struct CustomVwap {
    #[param(default = 0)]
    anchor_bar: usize,
}

impl Indicator for CustomVwap {
    fn compute(&self, bars: &[Bar], _: &ParamValues) -> IndicatorOutput {
        // Runs at full native speed, same thread as render loop
    }
}
```

**Speed:** ~0.01ms per indicator compute. Identical to built-in indicators.

### Tier 2: Python with AOT Compilation (Ship Second)

Users **write** Python. `apex build` **compiles** to native. Development mode interprets for fast iteration.

| Mode | What runs | Speed | Use case |
|---|---|---|---|
| Dev mode | Python interpreter (subprocess) | ~10ms/call | Writing & debugging |
| Release mode | Compiled native (.pyd/.so) | ~0.01ms/call | Production trading |

#### Compilation Tools

| Tool | How it works | Compatibility |
|---|---|---|
| Nuitka | Python -> C -> native binary | Full CPython compat |
| Cython | Annotated Python -> C extensions | NumPy/SciPy use this |
| mypyc | Type-annotated Python -> C | Used by mypy itself |
| Maturin + PyO3 | Python calling Rust -> single .pyd | Best for hybrid code |

#### Build Flow

```
User writes: indicators/my_rsi.py
    v  apex build (or first load)
    v  Nuitka/Cython compiles to native
    v  my_rsi.pyd (Windows) / my_rsi.so (Linux)
    v  Core loads as dynamic library
```

This is what **Blender** does: Python scripts compiled to C extensions via Cython. **NumPy/SciPy** also use this pattern.

#### Transpilation: Python to Rust

For the constrained subset of our SDK (indicator compute = pure math on arrays), transpilation is tractable:

```python
# User writes:
@indicator(name="SMA", category="overlay")
def compute(bars: list[Bar], period: int = 20) -> IndicatorOutput:
    values = []
    for i in range(len(bars)):
        if i < period:
            values.append(bars[i].close)
        else:
            s = sum(b.close for b in bars[i-period:i])
            values.append(s / period)
    return IndicatorOutput(values=values)
```

```rust
// apex build generates:
fn compute(bars: &[Bar], period: usize) -> Vec<f32> {
    let mut values = Vec::with_capacity(bars.len());
    for i in 0..bars.len() {
        if i < period {
            values.push(bars[i].close);
        } else {
            let s: f32 = bars[i-period..i].iter().map(|b| b.close).sum();
            values.push(s / period as f32);
        }
    }
    values
}
```

**Limitations:** Only works for pure-function compute code. But indicator functions are almost always pure math.

### Tier 3: WASM Sandbox (Future - Marketplace)

Compile extensions to **WebAssembly**, run in Wasmtime/Wasmer.

- **Near-native speed** (~80-90% of bare metal)
- **Perfect sandboxing** -- WASM can't access memory/files/network by default
- **Polyglot** -- Python (RustPython), Rust, C, Go all compile to WASM
- **Portable** -- same `.wasm` runs on Windows/Mac/Linux

This is what **Figma, Envoy, Cloudflare Workers, and Shopify** use for their plugin systems.

## Recommended Rollout

| Phase | What | Speed | Who it serves |
|---|---|---|---|
| Phase 1 | Rust .dll/.so plugins | Native | Power users |
| Phase 2 | Python + Nuitka AOT | Native (compiled) | Quant traders |
| Phase 3 | WASM sandbox | ~85% native | Marketplace extensions |

## The Key Insight

> **Python is the authoring language, not the execution language.**

Just like TypeScript is authored but JavaScript runs. The user sees Python, the terminal runs native code.

## Industry Precedents

| Product | Authoring | Execution | Compilation |
|---|---|---|---|
| NumPy/SciPy | Python | C (Cython) | Build-time |
| Blender | Python | C extensions | User-triggered |
| Unity | C# | IL2CPP (native) | Build-time |
| Figma Plugins | JS | WASM (QuickJS) | Install-time |
| Cloudflare Workers | JS/Python/Rust | WASM | Deploy-time |
| **Apex Terminal** | **Python/Rust/TOML** | **Native or WASM** | **`apex build`** |
"""

PATTERN_PAGE = """# Common Architecture Pattern

## Industry Adoption

The closed-core + open-extension model is the **dominant architecture** for professional tools and platforms.

### Developer Tools

| Product | Core | Extension API | Result |
|---|---|---|---|
| VS Code | Closed Electron binary | Open JS/TS Extension API | Most successful dev tool of the decade |
| Sublime Text | Closed C++ core | Open Python plugin API | Pioneer since 2008 |
| JetBrains | Closed platform | Open Java/Kotlin SDK | Entire paid+free ecosystem |
| Figma | Closed renderer | Open JS Plugin API | Massive plugin ecosystem |
| Obsidian | Closed Electron | Open plugin API | 1000+ community plugins |

### Game Engines

| Product | Core | Extension | Revenue |
|---|---|---|---|
| Unity | Closed engine | C# scripting + Asset Store | $1B+ marketplace |
| Unreal | Source-available | Blueprint + C++ plugins | Industry standard |

### Trading Platforms

| Product | Core | Extension | Limitation |
|---|---|---|---|
| NinjaTrader | Closed | C# addon SDK | Windows only |
| Sierra Chart | Closed | C++ study DLL | Steep learning curve |
| MetaTrader 4/5 | Closed | MQL4/5 (proprietary) | Language lock-in |
| TradingView | Closed | Pine Script (proprietary) | Most restrictive |
| thinkorswim | Closed | thinkScript (proprietary) | TD Ameritrade only |
| **Apex Terminal** | **Closed** | **Python + Rust + TOML** | **No lock-in** |

## Why This Pattern Works

### 1. Core is Hard, Extensions are Creative
GPU rendering, order routing, data feeds = deep engineering. Extensions = trader domain expertise. Separation lets each side focus.

### 2. Moat Protection
Core is the competitive advantage. Opening extensions doesn't give away the secret sauce -- it makes the platform more valuable.

### 3. Community Leverage
1000 community contributors building indicators > a 5-person team trying to build everything. VS Code has 40,000+ extensions.

### 4. Revenue Models

| Model | Example |
|---|---|
| Free core + paid extensions | Unity (30% cut) |
| Free everything, adoption | VS Code |
| Freemium + premium features | JetBrains |
| Subscription + marketplace | Figma |

## What Separates Winners from Losers

| Factor | Winners | Losers |
|---|---|---|
| SDK docs | Excellent tutorials | Sparse/outdated |
| Hello world time | < 5 minutes | Hours of setup |
| Core quality | Best-in-class | "Good enough" |
| Language choice | Standard (JS, Python) | Proprietary (Pine, MQL) |
| Community | Active forums | Corporate silence |

## Our Advantages

1. **No proprietary language** -- Python + Rust, not Pine Script
2. **GPU-native** -- 60fps at 4K, not Electron
3. **Config-first** -- themes/layouts/templates with zero code
4. **Compilation path** -- Python authored, native executed
5. **Real broker integration** -- direct IBKR, not just paper
"""

for title, text in [("Performance and Compilation Strategy", PERF_PAGE), ("Common Architecture Pattern", PATTERN_PAGE)]:
    r = requests.post(URL, headers=HEADERS, json={
        "collectionId": COL,
        "parentDocumentId": PARENT,
        "title": title,
        "text": text,
        "publish": True,
    })
    d = r.json()
    doc_id = d.get("data", {}).get("id", "ERROR")
    print(f"  {title}: {doc_id[:12]}")
