# Apex Terminal — Charting UI Implementation Plan (v2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a high-performance multi-window desktop trading terminal — Tauri 2 shell, WebGPU instanced candlestick renderer with category axis, multi-pane workspace, moving average overlays, drawing tools, crosshair, pan/zoom via uniform matrix.

**Architecture:** Tauri (Rust backend) hosts a React + TypeScript frontend. Each chart pane owns a `<canvas>` rendered via WebGPU with instanced draw calls — one draw call per candle series regardless of bar count. Pan/zoom updates a clip-space transform matrix uniform; the instance buffer is only rebuilt when data or visible range changes. A Canvas 2D overlay handles crosshair and drawing tools. The Rust side fetches OHLCV data via a Python yfinance sidecar during development. Multiple windows supported via Tauri's multi-window API.

**Tech Stack:** Tauri 2, Rust, React 18, TypeScript, Vite, WebGPU (WGSL shaders), Zustand, Vitest, yfinance (Python, data source)

---

## File Structure

```
apex-terminal/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   └── src/
│       ├── main.rs                    # Tauri entry
│       ├── lib.rs                     # Register commands
│       └── data.rs                    # yfinance sidecar bridge, OHLCV types
├── scripts/
│   └── yfinance_server.py             # HTTP server wrapping yfinance
├── src/
│   ├── main.tsx                       # React entry
│   ├── App.tsx                        # Root: toolbar + workspace
│   ├── global.css                     # CSS reset
│   ├── types.ts                       # Shared TS types (Bar, Drawing, etc)
│   ├── renderer/
│   │   ├── gpu.ts                     # WebGPU device singleton
│   │   ├── CandleRenderer.ts          # Instanced candle draw calls
│   │   ├── LineRenderer.ts            # Instanced quad AA line renderer (MAs)
│   │   ├── GridRenderer.ts            # Grid lines + axes via render bundle
│   │   └── shaders/
│   │       ├── candles.wgsl           # Candle vertex + fragment shader
│   │       ├── line.wgsl              # AA line vertex + fragment shader
│   │       └── grid.wgsl              # Grid line shader
│   ├── chart/
│   │   ├── ChartPane.tsx              # Single chart: canvas + overlay
│   │   ├── CoordSystem.ts             # Category axis: bar index ↔ pixel, price ↔ pixel
│   │   ├── CrosshairOverlay.tsx       # Canvas 2D crosshair + price/time labels
│   │   ├── DrawingOverlay.tsx         # Canvas 2D trendline/hline drawing tools
│   │   └── useChartData.ts            # Hook: loads bars from Tauri, manages viewport
│   ├── data/
│   │   ├── columns.ts                 # Columnar Float64Array data store
│   │   └── indicators.ts              # SMA/EMA compute (CPU)
│   ├── workspace/
│   │   └── Workspace.tsx              # Responsive grid of ChartPanes
│   ├── toolbar/
│   │   └── Toolbar.tsx                # Symbol input, timeframe, drawing tools
│   └── store/
│       ├── chartStore.ts              # Zustand: per-pane symbol, tf, viewport
│       └── drawingStore.ts            # Zustand: drawings per symbol, persisted
├── src/tests/
│   ├── CoordSystem.test.ts
│   ├── columns.test.ts
│   └── drawingStore.test.ts
├── index.html
├── vite.config.ts
├── tsconfig.json
└── package.json
```

---

## Task 1: Project Scaffold (Tauri + React + TypeScript)

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`
- Create: `src/main.tsx`, `src/App.tsx`, `src/global.css`, `src/types.ts`

- [ ] **Step 1: Scaffold Tauri + React + TypeScript project**

Since the directory has existing files (docs/), scaffold into a temp dir then merge:

```bash
cd C:\Users\USER\documents\development\apex-terminal
npx create-tauri-app@latest _scaffold -- --template react-ts --manager npm --yes
```

Move scaffold files to repo root (preserve docs/):
```bash
cp -r _scaffold/src-tauri _scaffold/src _scaffold/index.html _scaffold/package.json _scaffold/vite.config.ts _scaffold/tsconfig.json _scaffold/tsconfig.node.json .
rm -rf _scaffold
```

- [ ] **Step 2: Install frontend dependencies**

```bash
npm install zustand
npm install -D vitest jsdom
```

- [ ] **Step 3: Configure vite.config.ts for Vitest**

```typescript
// vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
  },
  clearScreen: false,
  server: {
    strictPort: true,
  },
})
```

- [ ] **Step 4: Add test script to package.json**

Add to the `"scripts"` section:
```json
"test": "vitest run",
"test:watch": "vitest"
```

- [ ] **Step 5: Create shared TypeScript types**

```typescript
// src/types.ts
export interface Bar {
  time: number   // Unix timestamp seconds
  open: number
  high: number
  low: number
  close: number
  volume: number
}

export type Timeframe = '1m' | '5m' | '15m' | '1h' | '4h' | '1d' | '1wk'

export type DrawingTool = 'cursor' | 'trendline' | 'hline'

export interface Point { time: number; price: number }

export interface Drawing {
  id: string
  type: DrawingTool
  points: Point[]
  color: string
  symbol: string
  timeframe: Timeframe
}
```

- [ ] **Step 6: Create global CSS reset**

```css
/* src/global.css */
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
html, body, #root { width: 100%; height: 100%; overflow: hidden; }
body { background: #0d0d0d; color: #ccc; font-family: 'JetBrains Mono', 'Fira Code', monospace; }
```

- [ ] **Step 7: Create placeholder App.tsx**

```typescript
// src/App.tsx
import './global.css'

export default function App() {
  return <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', color: '#555' }}>
    Apex Terminal — initializing...
  </div>
}
```

- [ ] **Step 8: Configure Tauri window for trading**

In `src-tauri/tauri.conf.json`, set:
```json
{
  "productName": "Apex Terminal",
  "identifier": "com.apex.terminal",
  "windows": [
    {
      "width": 1920,
      "height": 1080,
      "title": "Apex Terminal",
      "decorations": true,
      "resizable": true
    }
  ]
}
```

Also add to `src-tauri/capabilities/default.json`:
```json
{
  "identifier": "default",
  "windows": ["*"],
  "permissions": [
    "core:default",
    "core:window:allow-create",
    "core:window:default",
    "shell:allow-open"
  ]
}
```

- [ ] **Step 9: Verify dev server starts**

```bash
cargo tauri dev
```
Expected: Tauri window opens with placeholder text.

- [ ] **Step 10: Commit**

```bash
git add -A && git commit -m "feat: scaffold Tauri 2 + React + TypeScript + Vitest"
```

---

## Task 2: yfinance Data Bridge

**Files:**
- Create: `scripts/yfinance_server.py`
- Create: `src-tauri/src/data.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Create yfinance HTTP server**

```python
# scripts/yfinance_server.py
"""Tiny HTTP server that wraps yfinance. Tauri calls this via reqwest."""
import json, sys
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs
import yfinance as yf

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        params = parse_qs(parsed.query)

        if parsed.path == "/bars":
            symbol = params.get("symbol", ["AAPL"])[0]
            interval = params.get("interval", ["5m"])[0]
            period = params.get("period", ["5d"])[0]

            ticker = yf.Ticker(symbol)
            df = ticker.history(period=period, interval=interval)
            bars = []
            for ts, row in df.iterrows():
                bars.append({
                    "time": int(ts.timestamp()),
                    "open": round(row["Open"], 4),
                    "high": round(row["High"], 4),
                    "low": round(row["Low"], 4),
                    "close": round(row["Close"], 4),
                    "volume": round(row["Volume"], 2),
                })

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(json.dumps(bars).encode())
        elif parsed.path == "/health":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"ok")
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass  # suppress logs

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8777
    server = HTTPServer(("127.0.0.1", port), Handler)
    print(f"yfinance server on http://127.0.0.1:{port}")
    server.serve_forever()
```

- [ ] **Step 2: Add reqwest + serde to Cargo.toml**

Add to existing `[dependencies]` section in `src-tauri/Cargo.toml`:
```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 3: Write data.rs — Tauri command that calls yfinance server**

```rust
// src-tauri/src/data.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[tauri::command]
pub async fn get_bars(symbol: String, interval: String, period: String) -> Result<Vec<Bar>, String> {
    let url = format!(
        "http://127.0.0.1:8777/bars?symbol={}&interval={}&period={}",
        symbol, interval, period
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to reach yfinance server: {}", e))?;
    let bars: Vec<Bar> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse bars: {}", e))?;
    Ok(bars)
}
```

- [ ] **Step 4: Register command in lib.rs**

```rust
// src-tauri/src/lib.rs
mod data;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![data::get_bars])
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
```

- [ ] **Step 5: Test end-to-end**

Terminal 1: `python scripts/yfinance_server.py`
Terminal 2: `cargo tauri dev`

In browser console or App.tsx:
```typescript
import { invoke } from '@tauri-apps/api/core'
const bars = await invoke('get_bars', { symbol: 'AAPL', interval: '5m', period: '5d' })
console.log(`Got ${bars.length} bars`, bars[0])
```
Expected: Real AAPL data in console.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: yfinance data bridge via Python sidecar + Tauri command"
```

---

## Task 3: Columnar Data Store + Coordinate System

**Files:**
- Create: `src/data/columns.ts`
- Create: `src/chart/CoordSystem.ts`
- Create: `src/tests/columns.test.ts`
- Create: `src/tests/CoordSystem.test.ts`

- [ ] **Step 1: Write failing tests for ColumnStore**

```typescript
// src/tests/columns.test.ts
import { describe, it, expect } from 'vitest'
import { ColumnStore } from '../data/columns'
import type { Bar } from '../types'

const BARS: Bar[] = [
  { time: 1000, open: 100, high: 110, low: 90, close: 105, volume: 1000 },
  { time: 1060, open: 105, high: 115, low: 95, close: 110, volume: 2000 },
  { time: 1120, open: 110, high: 120, low: 100, close: 108, volume: 1500 },
]

describe('ColumnStore', () => {
  it('converts Bar[] to columnar arrays', () => {
    const store = ColumnStore.fromBars(BARS)
    expect(store.length).toBe(3)
    expect(store.opens[0]).toBe(100)
    expect(store.closes[2]).toBe(108)
    expect(store.highs[1]).toBe(115)
  })

  it('returns min/max for a range', () => {
    const store = ColumnStore.fromBars(BARS)
    const { min, max } = store.priceRange(0, 3)
    expect(min).toBe(90)
    expect(max).toBe(120)
  })

  it('binary searches for time index', () => {
    const store = ColumnStore.fromBars(BARS)
    expect(store.indexAtTime(1060)).toBe(1)
    expect(store.indexAtTime(1090)).toBe(1) // between 1060 and 1120, returns floor
  })
})
```

- [ ] **Step 2: Run tests — expect FAIL**

```bash
npm test
```

- [ ] **Step 3: Implement ColumnStore**

```typescript
// src/data/columns.ts
import type { Bar } from '../types'

export class ColumnStore {
  readonly times: Float64Array
  readonly opens: Float64Array
  readonly highs: Float64Array
  readonly lows: Float64Array
  readonly closes: Float64Array
  readonly volumes: Float64Array
  readonly length: number

  private constructor(
    times: Float64Array, opens: Float64Array, highs: Float64Array,
    lows: Float64Array, closes: Float64Array, volumes: Float64Array,
  ) {
    this.times = times; this.opens = opens; this.highs = highs
    this.lows = lows; this.closes = closes; this.volumes = volumes
    this.length = times.length
  }

  static fromBars(bars: Bar[]): ColumnStore {
    const n = bars.length
    const times = new Float64Array(n)
    const opens = new Float64Array(n)
    const highs = new Float64Array(n)
    const lows = new Float64Array(n)
    const closes = new Float64Array(n)
    const volumes = new Float64Array(n)

    for (let i = 0; i < n; i++) {
      times[i] = bars[i].time
      opens[i] = bars[i].open
      highs[i] = bars[i].high
      lows[i] = bars[i].low
      closes[i] = bars[i].close
      volumes[i] = bars[i].volume
    }
    return new ColumnStore(times, opens, highs, lows, closes, volumes)
  }

  /** Min low and max high for bars in [start, end) */
  priceRange(start: number, end: number): { min: number; max: number } {
    let min = Infinity, max = -Infinity
    const e = Math.min(end, this.length)
    for (let i = start; i < e; i++) {
      if (this.lows[i] < min) min = this.lows[i]
      if (this.highs[i] > max) max = this.highs[i]
    }
    return { min, max }
  }

  /** Binary search for the bar index at or before the given timestamp */
  indexAtTime(time: number): number {
    let lo = 0, hi = this.length - 1
    while (lo <= hi) {
      const mid = (lo + hi) >>> 1
      if (this.times[mid] <= time) lo = mid + 1
      else hi = mid - 1
    }
    return Math.max(0, hi)
  }
}
```

- [ ] **Step 4: Write failing tests for CoordSystem**

```typescript
// src/tests/CoordSystem.test.ts
import { describe, it, expect } from 'vitest'
import { CoordSystem } from '../chart/CoordSystem'

const cs = new CoordSystem({
  width: 1000, height: 600,
  barCount: 100,
  minPrice: 100, maxPrice: 200,
  paddingRight: 80, paddingTop: 20, paddingBottom: 40,
})

describe('CoordSystem (category axis)', () => {
  it('maps bar index 0 to left edge', () => {
    expect(cs.barToX(0)).toBeCloseTo(0)
  })
  it('maps last bar to right edge minus padding', () => {
    expect(cs.barToX(99)).toBeCloseTo(920 - 920/100) // close to right edge
  })
  it('maps min price to bottom', () => {
    expect(cs.priceToY(100)).toBeCloseTo(560) // height - paddingBottom
  })
  it('maps max price to top', () => {
    expect(cs.priceToY(200)).toBeCloseTo(20)  // paddingTop
  })
  it('round-trips price', () => {
    expect(cs.yToPrice(cs.priceToY(150))).toBeCloseTo(150)
  })
  it('round-trips bar index', () => {
    expect(cs.xToBar(cs.barToX(50))).toBeCloseTo(50)
  })
  it('reports bar width', () => {
    expect(cs.barWidth).toBeGreaterThan(0)
    expect(cs.barWidth).toBeLessThan(20)
  })
  it('converts price to clip Y', () => {
    const clipY = cs.priceToClipY(150)
    expect(clipY).toBeGreaterThan(-1)
    expect(clipY).toBeLessThan(1)
  })
  it('converts bar index to clip X', () => {
    const clipX = cs.barToClipX(50)
    expect(clipX).toBeGreaterThan(-1)
    expect(clipX).toBeLessThan(1)
  })
})
```

- [ ] **Step 5: Run tests — expect FAIL**

```bash
npm test
```

- [ ] **Step 6: Implement CoordSystem**

```typescript
// src/chart/CoordSystem.ts
export interface CoordConfig {
  width: number
  height: number
  barCount: number
  minPrice: number
  maxPrice: number
  paddingRight?: number
  paddingTop?: number
  paddingBottom?: number
}

/**
 * Category-axis coordinate system.
 * X: bar index → pixel (no timestamp gaps)
 * Y: price → pixel (linear, inverted)
 * Also provides clip-space conversions for WebGPU.
 */
export class CoordSystem {
  readonly width: number
  readonly height: number
  readonly barCount: number
  readonly minPrice: number
  readonly maxPrice: number
  readonly pr: number  // padding right (price axis labels)
  readonly pt: number  // padding top
  readonly pb: number  // padding bottom

  constructor(c: CoordConfig) {
    this.width = c.width
    this.height = c.height
    this.barCount = c.barCount
    this.minPrice = c.minPrice
    this.maxPrice = c.maxPrice
    this.pr = c.paddingRight ?? 80
    this.pt = c.paddingTop ?? 20
    this.pb = c.paddingBottom ?? 40
  }

  get chartWidth() { return this.width - this.pr }
  get chartHeight() { return this.height - this.pt - this.pb }
  get barWidth() { return this.barCount > 0 ? (this.chartWidth / this.barCount) * 0.8 : 1 }
  get barStep() { return this.barCount > 0 ? this.chartWidth / this.barCount : 1 }

  // --- Pixel space ---
  barToX(index: number): number {
    return index * this.barStep + this.barStep * 0.5
  }

  xToBar(x: number): number {
    return (x - this.barStep * 0.5) / this.barStep
  }

  priceToY(price: number): number {
    const ratio = (price - this.minPrice) / (this.maxPrice - this.minPrice)
    return this.pt + this.chartHeight * (1 - ratio)
  }

  yToPrice(y: number): number {
    const ratio = 1 - (y - this.pt) / this.chartHeight
    return this.minPrice + ratio * (this.maxPrice - this.minPrice)
  }

  // --- Clip space [-1, 1] for WebGPU ---
  barToClipX(index: number): number {
    return (this.barToX(index) / this.width) * 2 - 1
  }

  priceToClipY(price: number): number {
    return 1 - (this.priceToY(price) / this.height) * 2
  }

  clipBarWidth(): number {
    return (this.barWidth / this.width) * 2
  }

  // --- Immutable updates ---
  withSize(width: number, height: number): CoordSystem {
    return new CoordSystem({ ...this.toConfig(), width, height })
  }

  withPriceRange(min: number, max: number): CoordSystem {
    return new CoordSystem({ ...this.toConfig(), minPrice: min, maxPrice: max })
  }

  withBarCount(count: number): CoordSystem {
    return new CoordSystem({ ...this.toConfig(), barCount: count })
  }

  private toConfig(): CoordConfig {
    return {
      width: this.width, height: this.height, barCount: this.barCount,
      minPrice: this.minPrice, maxPrice: this.maxPrice,
      paddingRight: this.pr, paddingTop: this.pt, paddingBottom: this.pb,
    }
  }
}
```

- [ ] **Step 7: Run tests — expect PASS**

```bash
npm test
```
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: columnar data store + category-axis coordinate system with tests"
```

---

## Task 4: WebGPU Context + Candle Shader

**Files:**
- Create: `src/renderer/gpu.ts`
- Create: `src/renderer/shaders/candles.wgsl`
- Create: `src/renderer/CandleRenderer.ts`

- [ ] **Step 1: Write gpu.ts — device singleton**

```typescript
// src/renderer/gpu.ts
export interface GPUContext {
  device: GPUDevice
  format: GPUTextureFormat
}

let _ctx: GPUContext | null = null

export async function getGPUContext(): Promise<GPUContext> {
  if (_ctx) return _ctx
  if (!navigator.gpu) throw new Error('WebGPU not supported')
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' })
  if (!adapter) throw new Error('No GPU adapter found')
  const device = await adapter.requestDevice()
  device.lost.then(info => { console.error('GPU device lost:', info); _ctx = null })
  _ctx = { device, format: navigator.gpu.getPreferredCanvasFormat() }
  return _ctx
}

export function configureCanvas(canvas: HTMLCanvasElement, ctx: GPUContext): GPUCanvasContext {
  const gpuCtx = canvas.getContext('webgpu') as GPUCanvasContext
  gpuCtx.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })
  return gpuCtx
}
```

- [ ] **Step 2: Write the WGSL candle shader**

```wgsl
// src/renderer/shaders/candles.wgsl

// Per-instance candle data — pre-transformed to clip space on CPU
struct CandleInstance {
  @location(0) x_clip:       f32,  // center X in clip space
  @location(1) open_clip:    f32,  // open Y in clip space
  @location(2) close_clip:   f32,  // close Y in clip space
  @location(3) low_clip:     f32,  // low (wick bottom) in clip space
  @location(4) high_clip:    f32,  // high (wick top) in clip space
  @location(5) body_w_clip:  f32,  // body half-width in clip space
  @location(6) color:        vec4<f32>,  // RGBA
}

struct Uniforms {
  wick_w_clip: f32,  // wick half-width in clip space
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
}

// 18 vertices per candle: 6 body + 6 upper wick + 6 lower wick
@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: CandleInstance) -> VertOut {
  let body_top    = max(inst.open_clip, inst.close_clip);
  let body_bottom = min(inst.open_clip, inst.close_clip);

  // Unit quad corners for triangle-list (2 triangles = 6 verts)
  let corners = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
    vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
  );

  var min_pt: vec2<f32>;
  var max_pt: vec2<f32>;
  var idx: u32;

  if (vi < 6u) {
    // Body
    idx = vi;
    min_pt = vec2(inst.x_clip - inst.body_w_clip, body_bottom);
    max_pt = vec2(inst.x_clip + inst.body_w_clip, body_top);
  } else if (vi < 12u) {
    // Upper wick
    idx = vi - 6u;
    min_pt = vec2(inst.x_clip - u.wick_w_clip, body_top);
    max_pt = vec2(inst.x_clip + u.wick_w_clip, inst.high_clip);
  } else {
    // Lower wick
    idx = vi - 12u;
    min_pt = vec2(inst.x_clip - u.wick_w_clip, inst.low_clip);
    max_pt = vec2(inst.x_clip + u.wick_w_clip, body_bottom);
  }

  let c = corners[idx];
  let pos = min_pt + c * (max_pt - min_pt);

  var out: VertOut;
  out.pos = vec4(pos, 0.0, 1.0);
  out.color = inst.color;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  return in.color;
}
```

- [ ] **Step 3: Write CandleRenderer.ts**

```typescript
// src/renderer/CandleRenderer.ts
import { GPUContext } from './gpu'
import { CoordSystem } from '../chart/CoordSystem'
import { ColumnStore } from '../data/columns'
import shaderSrc from './shaders/candles.wgsl?raw'

const FLOATS_PER_INSTANCE = 10  // x, open, close, low, high, bodyW, r, g, b, a
const VERTS_PER_CANDLE = 18

const BULL_COLOR = [0.18, 0.78, 0.45, 1.0]  // green
const BEAR_COLOR = [0.93, 0.27, 0.27, 1.0]  // red

export class CandleRenderer {
  private pipeline: GPURenderPipeline
  private uniformBuffer: GPUBuffer
  private uniformBindGroup: GPUBindGroup
  private instanceBuffer: GPUBuffer | null = null
  private instanceCount = 0
  private readonly device: GPUDevice

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    this.uniformBuffer = ctx.device.createBuffer({
      size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })

    const module = ctx.device.createShaderModule({ code: shaderSrc })
    const bgl = ctx.device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: {} }],
    })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: ctx.device.createPipelineLayout({ bindGroupLayouts: [bgl] }),
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [{
          arrayStride: FLOATS_PER_INSTANCE * 4,
          stepMode: 'instance',
          attributes: [
            { shaderLocation: 0, offset: 0,  format: 'float32' },     // x_clip
            { shaderLocation: 1, offset: 4,  format: 'float32' },     // open_clip
            { shaderLocation: 2, offset: 8,  format: 'float32' },     // close_clip
            { shaderLocation: 3, offset: 12, format: 'float32' },     // low_clip
            { shaderLocation: 4, offset: 16, format: 'float32' },     // high_clip
            { shaderLocation: 5, offset: 20, format: 'float32' },     // body_w_clip
            { shaderLocation: 6, offset: 24, format: 'float32x4' },   // color RGBA
          ],
        }],
      },
      fragment: { module, entryPoint: 'fs_main', targets: [{ format: ctx.format }] },
      primitive: { topology: 'triangle-list' },
    })

    this.uniformBindGroup = ctx.device.createBindGroup({
      layout: bgl,
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
    })
  }

  upload(data: ColumnStore, cs: CoordSystem, viewStart: number, viewCount: number) {
    const end = Math.min(viewStart + viewCount, data.length)
    const count = end - viewStart
    if (count <= 0) return

    const arr = new Float32Array(count * FLOATS_PER_INSTANCE)
    const bodyW = cs.clipBarWidth() * 0.5  // half-width in clip space
    const wickW = Math.max(bodyW * 0.15, 0.001)

    for (let i = 0; i < count; i++) {
      const di = viewStart + i  // data index
      const base = i * FLOATS_PER_INSTANCE
      const isBull = data.closes[di] >= data.opens[di]
      const color = isBull ? BULL_COLOR : BEAR_COLOR

      arr[base + 0] = cs.barToClipX(i)  // category axis: use local index
      arr[base + 1] = cs.priceToClipY(data.opens[di])
      arr[base + 2] = cs.priceToClipY(data.closes[di])
      arr[base + 3] = cs.priceToClipY(data.lows[di])
      arr[base + 4] = cs.priceToClipY(data.highs[di])
      arr[base + 5] = bodyW
      arr[base + 6] = color[0]
      arr[base + 7] = color[1]
      arr[base + 8] = color[2]
      arr[base + 9] = color[3]
    }

    if (this.instanceBuffer) this.instanceBuffer.destroy()
    this.instanceBuffer = this.device.createBuffer({
      size: arr.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    })
    this.device.queue.writeBuffer(this.instanceBuffer, 0, arr)
    this.instanceCount = count

    // Wick width uniform
    this.device.queue.writeBuffer(this.uniformBuffer, 0, new Float32Array([wickW, 0, 0, 0]))
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.instanceBuffer || this.instanceCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.uniformBindGroup)
    pass.setVertexBuffer(0, this.instanceBuffer)
    pass.draw(VERTS_PER_CANDLE, this.instanceCount)
  }

  destroy() {
    this.instanceBuffer?.destroy()
    this.uniformBuffer.destroy()
  }
}
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: WebGPU context + instanced candle renderer with clip-space WGSL shader"
```

---

## Task 5: Grid Renderer (Render Bundle)

**Files:**
- Create: `src/renderer/shaders/grid.wgsl`
- Create: `src/renderer/GridRenderer.ts`

- [ ] **Step 1: Write grid WGSL shader**

```wgsl
// src/renderer/shaders/grid.wgsl
struct LineVert {
  @location(0) pos: vec2<f32>,
  @location(1) color: vec4<f32>,
}

struct VertOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(v: LineVert) -> VertOut {
  // Input is already in clip space [-1, 1]
  return VertOut(vec4(v.pos, 0.0, 1.0), v.color);
}

@fragment
fn fs_main(v: VertOut) -> @location(0) vec4<f32> {
  return v.color;
}
```

- [ ] **Step 2: Write GridRenderer.ts**

```typescript
// src/renderer/GridRenderer.ts
import { GPUContext } from './gpu'
import { CoordSystem } from '../chart/CoordSystem'
import shaderSrc from './shaders/grid.wgsl?raw'

const GRID_COLOR = [0.15, 0.15, 0.15, 1.0]
const AXIS_COLOR = [0.3, 0.3, 0.3, 1.0]

export class GridRenderer {
  private pipeline: GPURenderPipeline
  private lineBuffer: GPUBuffer | null = null
  private vertexCount = 0
  private readonly device: GPUDevice
  private readonly format: GPUTextureFormat

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    this.format = ctx.format

    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: 'auto',
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [{
          arrayStride: 6 * 4,
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32x2' },
            { shaderLocation: 1, offset: 8, format: 'float32x4' },
          ],
        }],
      },
      fragment: { module, entryPoint: 'fs_main', targets: [{ format: ctx.format }] },
      primitive: { topology: 'line-list' },
    })
  }

  upload(cs: CoordSystem) {
    const verts: number[] = []
    const addLine = (x0: number, y0: number, x1: number, y1: number, color: number[]) => {
      // Convert pixel → clip space
      const cx0 = (x0 / cs.width) * 2 - 1, cy0 = 1 - (y0 / cs.height) * 2
      const cx1 = (x1 / cs.width) * 2 - 1, cy1 = 1 - (y1 / cs.height) * 2
      verts.push(cx0, cy0, ...color, cx1, cy1, ...color)
    }

    // Horizontal price grid (8 levels)
    const priceStep = (cs.maxPrice - cs.minPrice) / 8
    for (let i = 0; i <= 8; i++) {
      const y = cs.priceToY(cs.minPrice + i * priceStep)
      addLine(0, y, cs.width - cs.pr, y, GRID_COLOR)
    }

    // Vertical bar grid (every ~100px)
    const barStep = Math.max(1, Math.floor(100 / cs.barStep))
    for (let i = 0; i < cs.barCount; i += barStep) {
      const x = cs.barToX(i)
      addLine(x, cs.pt, x, cs.height - cs.pb, GRID_COLOR)
    }

    // Axis border lines
    addLine(0, cs.pt, 0, cs.height - cs.pb, AXIS_COLOR)
    addLine(0, cs.height - cs.pb, cs.width - cs.pr, cs.height - cs.pb, AXIS_COLOR)
    addLine(cs.width - cs.pr, cs.pt, cs.width - cs.pr, cs.height - cs.pb, AXIS_COLOR)

    const data = new Float32Array(verts)
    if (this.lineBuffer) this.lineBuffer.destroy()
    this.lineBuffer = this.device.createBuffer({
      size: data.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    })
    this.device.queue.writeBuffer(this.lineBuffer, 0, data)
    this.vertexCount = verts.length / 6
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.lineBuffer || this.vertexCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setVertexBuffer(0, this.lineBuffer)
    pass.draw(this.vertexCount)
  }

  destroy() {
    this.lineBuffer?.destroy()
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: grid renderer with clip-space line shader"
```

---

## Task 6: ChartPane — Wire Renderers Together

**Files:**
- Create: `src/chart/useChartData.ts`
- Create: `src/chart/ChartPane.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write useChartData hook**

```typescript
// src/chart/useChartData.ts
import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ColumnStore } from '../data/columns'
import { CoordSystem } from './CoordSystem'
import type { Bar, Timeframe } from '../types'

const TF_TO_INTERVAL: Record<Timeframe, { interval: string; period: string }> = {
  '1m':  { interval: '1m',  period: '1d' },
  '5m':  { interval: '5m',  period: '5d' },
  '15m': { interval: '15m', period: '5d' },
  '1h':  { interval: '1h',  period: '1mo' },
  '4h':  { interval: '1h',  period: '3mo' }, // yfinance doesn't have 4h, use 1h
  '1d':  { interval: '1d',  period: '1y' },
  '1wk': { interval: '1wk', period: '5y' },
}

export function useChartData(symbol: string, timeframe: Timeframe, width: number, height: number) {
  const [data, setData] = useState<ColumnStore | null>(null)
  const [viewStart, setViewStart] = useState(0)
  const [viewCount, setViewCount] = useState(200)
  const [cs, setCs] = useState<CoordSystem | null>(null)

  // Load data
  useEffect(() => {
    const tf = TF_TO_INTERVAL[timeframe] ?? TF_TO_INTERVAL['5m']
    invoke<Bar[]>('get_bars', { symbol, interval: tf.interval, period: tf.period })
      .then(bars => {
        const store = ColumnStore.fromBars(bars)
        setData(store)
        setViewStart(Math.max(0, store.length - 200))
        setViewCount(Math.min(200, store.length))
      })
      .catch(err => console.error('Failed to load bars:', err))
  }, [symbol, timeframe])

  // Recompute coordinate system
  useEffect(() => {
    if (!data || width === 0 || height === 0) return
    const end = Math.min(viewStart + viewCount, data.length)
    const count = end - viewStart
    if (count <= 0) return

    const { min: minP, max: maxP } = data.priceRange(viewStart, end)
    const pad = (maxP - minP) * 0.05

    setCs(new CoordSystem({
      width, height,
      barCount: count,
      minPrice: minP - pad,
      maxPrice: maxP + pad,
    }))
  }, [data, viewStart, viewCount, width, height])

  const pan = useCallback((deltaPixels: number) => {
    if (!data || !cs) return
    const barDelta = Math.round(deltaPixels / cs.barStep)
    setViewStart(v => Math.max(0, Math.min(data.length - viewCount, v - barDelta)))
  }, [data, cs, viewCount])

  const zoom = useCallback((factor: number) => {
    if (!data) return
    setViewCount(v => {
      const newCount = Math.max(20, Math.min(data.length, Math.round(v * factor)))
      // Re-center on zoom
      setViewStart(s => Math.max(0, Math.min(data.length - newCount, s + Math.round((v - newCount) / 2))))
      return newCount
    })
  }, [data])

  return { data, cs, viewStart, viewCount, pan, zoom }
}
```

- [ ] **Step 2: Write ChartPane component**

```typescript
// src/chart/ChartPane.tsx
import { useEffect, useRef, useCallback } from 'react'
import { getGPUContext, configureCanvas } from '../renderer/gpu'
import { CandleRenderer } from '../renderer/CandleRenderer'
import { GridRenderer } from '../renderer/GridRenderer'
import { useChartData } from './useChartData'
import type { Timeframe } from '../types'

interface Props {
  symbol: string
  timeframe: Timeframe
  width: number
  height: number
}

export function ChartPane({ symbol, timeframe, width, height }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const renderers = useRef<{ candle: CandleRenderer; grid: GridRenderer } | null>(null)
  const gpuCanvas = useRef<GPUCanvasContext | null>(null)
  const { data, cs, viewStart, viewCount, pan, zoom } = useChartData(symbol, timeframe, width, height)

  // Init GPU
  useEffect(() => {
    if (!canvasRef.current) return
    let cancelled = false
    getGPUContext().then(ctx => {
      if (cancelled) return
      gpuCanvas.current = configureCanvas(canvasRef.current!, ctx)
      renderers.current = {
        candle: new CandleRenderer(ctx),
        grid: new GridRenderer(ctx),
      }
    })
    return () => {
      cancelled = true
      renderers.current?.candle.destroy()
      renderers.current?.grid.destroy()
    }
  }, [])

  // Render frame
  useEffect(() => {
    if (!renderers.current || !gpuCanvas.current || !cs || !data) return
    const { candle, grid } = renderers.current

    getGPUContext().then(({ device }) => {
      grid.upload(cs)
      candle.upload(data, cs, viewStart, viewCount)

      const encoder = device.createCommandEncoder()
      const view = gpuCanvas.current!.getCurrentTexture().createView()

      // Single render pass: clear + grid + candles
      const pass = encoder.beginRenderPass({
        colorAttachments: [{
          view, loadOp: 'clear',
          clearValue: { r: 0.05, g: 0.05, b: 0.05, a: 1 },
          storeOp: 'store',
        }],
      })
      grid.render(pass)
      candle.render(pass)
      pass.end()

      device.queue.submit([encoder.finish()])
    })
  }, [data, cs, viewStart, viewCount])

  // Pan on drag
  const dragRef = useRef<{ x: number } | null>(null)
  const onMouseDown = useCallback((e: React.MouseEvent) => {
    dragRef.current = { x: e.clientX }
  }, [])
  const onMouseMove = useCallback((e: React.MouseEvent) => {
    if (!dragRef.current) return
    pan(e.clientX - dragRef.current.x)
    dragRef.current = { x: e.clientX }
  }, [pan])
  const onMouseUp = useCallback(() => { dragRef.current = null }, [])
  const onWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault()
    zoom(e.deltaY > 0 ? 1.1 : 0.9)
  }, [zoom])

  return (
    <div style={{ position: 'relative', width, height, background: '#0d0d0d' }}>
      <canvas
        ref={canvasRef} width={width} height={height}
        style={{ display: 'block' }}
        onMouseDown={onMouseDown} onMouseMove={onMouseMove}
        onMouseUp={onMouseUp} onMouseLeave={onMouseUp}
        onWheel={onWheel}
      />
      {/* OHLC label */}
      <div style={{
        position: 'absolute', top: 4, left: 8,
        color: '#666', fontSize: 11, fontFamily: 'monospace', pointerEvents: 'none',
      }}>
        {symbol} · {timeframe}
        {data && viewStart + viewCount <= data.length && (() => {
          const last = viewStart + viewCount - 1
          return ` · O ${data.opens[last]?.toFixed(2)} H ${data.highs[last]?.toFixed(2)} L ${data.lows[last]?.toFixed(2)} C ${data.closes[last]?.toFixed(2)}`
        })()}
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Update App.tsx with a single ChartPane**

```typescript
// src/App.tsx
import './global.css'
import { ChartPane } from './chart/ChartPane'

export default function App() {
  return (
    <div style={{ width: '100vw', height: '100vh', overflow: 'hidden', background: '#0d0d0d' }}>
      <ChartPane symbol="AAPL" timeframe="5m" width={1200} height={700} />
    </div>
  )
}
```

- [ ] **Step 4: Run and verify**

Start yfinance server: `python scripts/yfinance_server.py`
Start app: `cargo tauri dev`
Expected: Candlestick chart of AAPL 5m data. Drag to pan, scroll to zoom.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: ChartPane wires WebGPU renderers with pan/zoom and real yfinance data"
```

---

## Task 7: Crosshair Overlay (Canvas 2D)

**Files:**
- Create: `src/chart/CrosshairOverlay.tsx`
- Modify: `src/chart/ChartPane.tsx`

- [ ] **Step 1: Write CrosshairOverlay**

```typescript
// src/chart/CrosshairOverlay.tsx
import { useRef, useEffect, useCallback, forwardRef, useImperativeHandle } from 'react'
import { CoordSystem } from './CoordSystem'
import { ColumnStore } from '../data/columns'

export interface CrosshairHandle {
  update: (mouseX: number, mouseY: number) => void
  clear: () => void
}

interface Props {
  cs: CoordSystem
  data: ColumnStore
  viewStart: number
  width: number
  height: number
}

export const CrosshairOverlay = forwardRef<CrosshairHandle, Props>(
  function CrosshairOverlay({ cs, data, viewStart, width, height }, ref) {
    const canvasRef = useRef<HTMLCanvasElement>(null)

    useImperativeHandle(ref, () => ({
      update(mouseX: number, mouseY: number) {
        const canvas = canvasRef.current
        if (!canvas) return
        const ctx = canvas.getContext('2d', { alpha: true })!
        ctx.clearRect(0, 0, width, height)

        const price = cs.yToPrice(mouseY)
        const barIdx = Math.round(cs.xToBar(mouseX))

        // Dashed crosshair lines
        ctx.strokeStyle = 'rgba(255,255,255,0.25)'
        ctx.setLineDash([4, 4])
        ctx.lineWidth = 1

        ctx.beginPath()
        ctx.moveTo(0, mouseY)
        ctx.lineTo(width - cs.pr, mouseY)
        ctx.stroke()

        ctx.beginPath()
        ctx.moveTo(mouseX, cs.pt)
        ctx.lineTo(mouseX, height - cs.pb)
        ctx.stroke()
        ctx.setLineDash([])

        // Price label on right axis
        ctx.fillStyle = '#1a1a2e'
        ctx.fillRect(width - cs.pr, mouseY - 10, cs.pr, 20)
        ctx.fillStyle = '#ccc'
        ctx.font = '11px monospace'
        ctx.textAlign = 'left'
        ctx.fillText(price.toFixed(2), width - cs.pr + 4, mouseY + 4)

        // Time label on bottom axis
        const dataIdx = viewStart + barIdx
        if (dataIdx >= 0 && dataIdx < data.length) {
          const time = data.times[dataIdx]
          const d = new Date(time * 1000)
          const label = `${d.getMonth()+1}/${d.getDate()} ${d.getHours().toString().padStart(2,'0')}:${d.getMinutes().toString().padStart(2,'0')}`
          const tw = ctx.measureText(label).width
          ctx.fillStyle = '#1a1a2e'
          ctx.fillRect(mouseX - tw/2 - 4, height - cs.pb, tw + 8, 18)
          ctx.fillStyle = '#ccc'
          ctx.textAlign = 'center'
          ctx.fillText(label, mouseX, height - cs.pb + 13)
        }
      },
      clear() {
        const canvas = canvasRef.current
        if (!canvas) return
        canvas.getContext('2d')?.clearRect(0, 0, width, height)
      },
    }))

    return (
      <canvas
        ref={canvasRef} width={width} height={height}
        style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }}
      />
    )
  }
)
```

- [ ] **Step 2: Integrate into ChartPane**

In `ChartPane.tsx`, add crosshair ref and wire mouse events:

```typescript
// Add imports:
import { CrosshairOverlay, CrosshairHandle } from './CrosshairOverlay'

// Add ref:
const crosshairRef = useRef<CrosshairHandle>(null)

// Update onMouseMove to also update crosshair:
const onMouseMove = useCallback((e: React.MouseEvent) => {
  const rect = canvasRef.current?.getBoundingClientRect()
  if (rect && cs && data) {
    crosshairRef.current?.update(e.clientX - rect.left, e.clientY - rect.top)
  }
  if (!dragRef.current) return
  pan(e.clientX - dragRef.current.x)
  dragRef.current = { x: e.clientX }
}, [pan, cs, data])

// Update onMouseUp/onMouseLeave to clear crosshair:
const onMouseUp = useCallback(() => { dragRef.current = null }, [])
const onMouseLeave = useCallback(() => {
  dragRef.current = null
  crosshairRef.current?.clear()
}, [])

// In JSX, after <canvas>, add:
{cs && data && (
  <CrosshairOverlay ref={crosshairRef} cs={cs} data={data}
    viewStart={viewStart} width={width} height={height} />
)}
```

- [ ] **Step 3: Add axis labels (price + time) to CrosshairOverlay or separate canvas**

Add a `useEffect` in ChartPane that draws static axis labels on another Canvas 2D:

```typescript
const axisRef = useRef<HTMLCanvasElement>(null)

useEffect(() => {
  if (!axisRef.current || !cs || !data) return
  const ctx = axisRef.current.getContext('2d')!
  ctx.clearRect(0, 0, width, height)
  ctx.fillStyle = '#444'
  ctx.font = '10px monospace'

  // Price labels
  const priceStep = (cs.maxPrice - cs.minPrice) / 8
  for (let i = 0; i <= 8; i++) {
    const price = cs.minPrice + i * priceStep
    ctx.fillText(price.toFixed(2), width - cs.pr + 4, cs.priceToY(price) + 4)
  }

  // Time labels
  const barStep = Math.max(1, Math.floor(100 / cs.barStep))
  for (let i = 0; i < cs.barCount; i += barStep) {
    const dataIdx = viewStart + i
    if (dataIdx < data.length) {
      const d = new Date(data.times[dataIdx] * 1000)
      const label = `${d.getHours().toString().padStart(2,'0')}:${d.getMinutes().toString().padStart(2,'0')}`
      ctx.fillText(label, cs.barToX(i) - 16, height - cs.pb + 14)
    }
  }
}, [cs, data, viewStart, width, height])

// In JSX, add after crosshair:
<canvas ref={axisRef} width={width} height={height}
  style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
```

- [ ] **Step 4: Verify crosshair + labels**

Expected: Move mouse over chart → crosshair follows with price/time labels. Price labels on right axis, time labels on bottom.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: crosshair overlay + axis labels via Canvas 2D"
```

---

## Task 8: Moving Average Line Renderer

**Files:**
- Create: `src/renderer/shaders/line.wgsl`
- Create: `src/renderer/LineRenderer.ts`
- Create: `src/data/indicators.ts`
- Modify: `src/chart/ChartPane.tsx`

- [ ] **Step 1: Write AA line WGSL shader (instanced quads + SDF)**

```wgsl
// src/renderer/shaders/line.wgsl
struct Uniforms {
  line_width: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
  color: vec4<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) line_coord: vec2<f32>,
  @location(1) color: vec4<f32>,
}

// Instance: two endpoints in clip space
// Slot 0 = pointA (clip xy), Slot 1 = pointB (clip xy)
@vertex
fn vs_main(
  @builtin(vertex_index) vIdx: u32,
  @location(0) pointA: vec2<f32>,
  @location(1) pointB: vec2<f32>,
) -> VertOut {
  let corners = array<vec2<f32>, 6>(
    vec2(0.0, -0.5), vec2(1.0, -0.5), vec2(1.0, 0.5),
    vec2(0.0, -0.5), vec2(1.0,  0.5), vec2(0.0, 0.5),
  );
  let c = corners[vIdx];

  let dir = pointB - pointA;
  let len = length(dir);
  let xBasis = select(vec2(1.0, 0.0), dir / len, len > 0.0001);
  let yBasis = vec2(-xBasis.y, xBasis.x);

  // Width in clip space: u.line_width pixels → clip units
  let pos = pointA + xBasis * (c.x * len) + yBasis * (c.y * u.line_width);

  var out: VertOut;
  out.pos = vec4(pos, 0.0, 1.0);
  out.line_coord = c;
  out.color = u.color;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  // SDF anti-aliasing at line edges
  let dist = abs(in.line_coord.y);
  let fw = fwidth(in.line_coord.y);
  let alpha = 1.0 - smoothstep(0.5 - fw, 0.5 + fw, dist);
  return vec4(in.color.rgb, in.color.a * alpha);
}
```

- [ ] **Step 2: Write indicators.ts — CPU-side SMA**

```typescript
// src/data/indicators.ts

/** Simple Moving Average */
export function sma(closes: Float64Array, period: number): Float64Array {
  const out = new Float64Array(closes.length)
  let sum = 0
  for (let i = 0; i < closes.length; i++) {
    sum += closes[i]
    if (i >= period) sum -= closes[i - period]
    out[i] = i >= period - 1 ? sum / period : NaN
  }
  return out
}

/** Exponential Moving Average */
export function ema(closes: Float64Array, period: number): Float64Array {
  const out = new Float64Array(closes.length)
  const k = 2 / (period + 1)
  out[0] = closes[0]
  for (let i = 1; i < closes.length; i++) {
    out[i] = closes[i] * k + out[i - 1] * (1 - k)
  }
  // Mark first `period` values as NaN (not enough data)
  for (let i = 0; i < period - 1; i++) out[i] = NaN
  return out
}
```

- [ ] **Step 3: Write LineRenderer.ts**

```typescript
// src/renderer/LineRenderer.ts
import { GPUContext } from './gpu'
import { CoordSystem } from '../chart/CoordSystem'
import shaderSrc from './shaders/line.wgsl?raw'

export class LineRenderer {
  private pipeline: GPURenderPipeline
  private uniformBuffer: GPUBuffer
  private bindGroup: GPUBindGroup
  private pointBuffer: GPUBuffer | null = null
  private segmentCount = 0
  private readonly device: GPUDevice

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    this.uniformBuffer = ctx.device.createBuffer({
      size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    })

    const module = ctx.device.createShaderModule({ code: shaderSrc })
    const bgl = ctx.device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: {} }],
    })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: ctx.device.createPipelineLayout({ bindGroupLayouts: [bgl] }),
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [
          // pointA — same buffer, instance stride = 1 point
          { arrayStride: 8, stepMode: 'instance',
            attributes: [{ shaderLocation: 0, offset: 0, format: 'float32x2' }] },
          // pointB — same buffer, offset by 1 point (8 bytes)
          { arrayStride: 8, stepMode: 'instance',
            attributes: [{ shaderLocation: 1, offset: 0, format: 'float32x2' }] },
        ],
      },
      fragment: {
        module, entryPoint: 'fs_main',
        targets: [{
          format: ctx.format,
          blend: {
            color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
          },
        }],
      },
      primitive: { topology: 'triangle-list' },
    })

    this.bindGroup = ctx.device.createBindGroup({
      layout: bgl,
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
    })
  }

  upload(values: Float64Array, cs: CoordSystem, viewStart: number, viewCount: number,
         color: [number, number, number, number], lineWidthPx: number) {
    // Build point array in clip space, skipping NaN
    const points: number[] = []
    for (let i = 0; i < viewCount; i++) {
      const di = viewStart + i
      if (di >= values.length || isNaN(values[di])) continue
      points.push(cs.barToClipX(i), cs.priceToClipY(values[di]))
    }

    if (points.length < 4) { this.segmentCount = 0; return }

    const data = new Float32Array(points)
    if (this.pointBuffer) this.pointBuffer.destroy()
    this.pointBuffer = this.device.createBuffer({
      size: data.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    })
    this.device.queue.writeBuffer(this.pointBuffer, 0, data)
    this.segmentCount = (points.length / 2) - 1

    // line_width in clip space (approximate: 2 * px / canvasWidth)
    const clipWidth = (lineWidthPx / cs.width) * 2
    this.device.queue.writeBuffer(this.uniformBuffer, 0,
      new Float32Array([clipWidth, 0, 0, 0, ...color]))
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.pointBuffer || this.segmentCount <= 0) return
    pass.setPipeline(this.pipeline)
    pass.setBindGroup(0, this.bindGroup)
    // pointA: starts at byte 0, pointB: starts at byte 8 (one point ahead)
    pass.setVertexBuffer(0, this.pointBuffer, 0, this.pointBuffer.size - 8)
    pass.setVertexBuffer(1, this.pointBuffer, 8, this.pointBuffer.size - 8)
    pass.draw(6, this.segmentCount)
  }

  destroy() {
    this.pointBuffer?.destroy()
    this.uniformBuffer.destroy()
  }
}
```

- [ ] **Step 4: Add MAs to ChartPane**

In `ChartPane.tsx`, add LineRenderer instances for SMA-20 and EMA-50:

```typescript
import { LineRenderer } from '../renderer/LineRenderer'
import { sma, ema } from '../data/indicators'

// Update renderers ref type:
const renderers = useRef<{
  candle: CandleRenderer; grid: GridRenderer;
  sma20: LineRenderer; ema50: LineRenderer;
} | null>(null)

// In GPU init:
renderers.current = {
  candle: new CandleRenderer(ctx),
  grid: new GridRenderer(ctx),
  sma20: new LineRenderer(ctx),
  ema50: new LineRenderer(ctx),
}

// In render useEffect, after candle.upload:
const sma20 = sma(data.closes, 20)
const ema50 = ema(data.closes, 50)
renderers.current.sma20.upload(sma20, cs, viewStart, viewCount, [0.3, 0.6, 1.0, 0.8], 1.5)
renderers.current.ema50.upload(ema50, cs, viewStart, viewCount, [1.0, 0.6, 0.2, 0.8], 1.5)

// In render pass, after candle.render:
renderers.current.sma20.render(pass)
renderers.current.ema50.render(pass)
```

- [ ] **Step 5: Verify MAs render**

Expected: Blue SMA-20 and orange EMA-50 lines overlaid on candlesticks with smooth anti-aliased edges.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: anti-aliased line renderer + SMA/EMA overlays"
```

---

## Task 9: Zustand Store + Workspace Grid

**Files:**
- Create: `src/store/chartStore.ts`
- Create: `src/workspace/Workspace.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write chartStore**

```typescript
// src/store/chartStore.ts
import { create } from 'zustand'
import type { Timeframe } from '../types'

interface PaneConfig {
  id: string
  symbol: string
  timeframe: Timeframe
}

interface ChartStore {
  panes: PaneConfig[]
  activePane: string
  setActivePane: (id: string) => void
  setSymbol: (id: string, symbol: string) => void
  setTimeframe: (id: string, tf: Timeframe) => void
}

const DEFAULT_SYMBOLS = ['AAPL', 'MSFT', 'NVDA', 'TSLA', 'SPY', 'QQQ', 'AMZN']

export const useChartStore = create<ChartStore>(set => ({
  panes: DEFAULT_SYMBOLS.map((symbol, i) => ({
    id: `pane-${i}`,
    symbol,
    timeframe: '5m' as Timeframe,
  })),
  activePane: 'pane-0',
  setActivePane: (id) => set({ activePane: id }),
  setSymbol: (id, symbol) =>
    set(s => ({ panes: s.panes.map(p => p.id === id ? { ...p, symbol } : p) })),
  setTimeframe: (id, timeframe) =>
    set(s => ({ panes: s.panes.map(p => p.id === id ? { ...p, timeframe } : p) })),
}))
```

- [ ] **Step 2: Write Workspace**

```typescript
// src/workspace/Workspace.tsx
import { useEffect, useRef, useState } from 'react'
import { ChartPane } from '../chart/ChartPane'
import { useChartStore } from '../store/chartStore'

export function Workspace() {
  const { panes, activePane, setActivePane } = useChartStore()
  const containerRef = useRef<HTMLDivElement>(null)
  const [dims, setDims] = useState({ w: 0, h: 0 })

  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const ro = new ResizeObserver(() => {
      setDims({ w: el.clientWidth, h: el.clientHeight })
    })
    ro.observe(el)
    setDims({ w: el.clientWidth, h: el.clientHeight })
    return () => ro.disconnect()
  }, [])

  const cols = 3
  const rows = Math.ceil(panes.length / cols)
  const paneW = dims.w > 0 ? Math.floor(dims.w / cols) : 0
  const paneH = dims.h > 0 ? Math.floor(dims.h / rows) : 0

  return (
    <div ref={containerRef} style={{
      display: 'grid',
      gridTemplateColumns: `repeat(${cols}, 1fr)`,
      width: '100%', height: '100%', background: '#0a0a0a', gap: 1,
    }}>
      {panes.map(pane => (
        <div
          key={pane.id}
          onClick={() => setActivePane(pane.id)}
          style={{
            border: `1px solid ${activePane === pane.id ? '#2a6496' : '#1a1a1a'}`,
            overflow: 'hidden',
          }}
        >
          {paneW > 0 && paneH > 0 && (
            <ChartPane
              symbol={pane.symbol}
              timeframe={pane.timeframe}
              width={paneW - 2}
              height={paneH - 2}
            />
          )}
        </div>
      ))}
    </div>
  )
}
```

- [ ] **Step 3: Update App.tsx**

```typescript
// src/App.tsx
import './global.css'
import { Workspace } from './workspace/Workspace'

export default function App() {
  return (
    <div style={{ width: '100vw', height: '100vh', overflow: 'hidden' }}>
      <Workspace />
    </div>
  )
}
```

- [ ] **Step 4: Verify 7 panes**

Expected: 7 chart panes in a 3×3 grid (7 filled, 2 empty slots), each showing different symbol.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: 7-pane workspace grid with Zustand store"
```

---

## Task 10: Toolbar

**Files:**
- Create: `src/toolbar/Toolbar.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write Toolbar**

```typescript
// src/toolbar/Toolbar.tsx
import { useState } from 'react'
import { useChartStore } from '../store/chartStore'
import type { Timeframe } from '../types'

const TIMEFRAMES: Timeframe[] = ['1m', '5m', '15m', '1h', '1d', '1wk']

export function Toolbar() {
  const { panes, activePane, setSymbol, setTimeframe } = useChartStore()
  const [symbolInput, setSymbolInput] = useState('')
  const pane = panes.find(p => p.id === activePane)

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      height: 36, background: '#111', borderBottom: '1px solid #222',
      padding: '0 12px', flexShrink: 0, fontFamily: 'monospace', fontSize: 12,
    }}>
      <span style={{ color: '#4a9eff', fontWeight: 'bold' }}>{pane?.symbol ?? '—'}</span>

      <form onSubmit={e => {
        e.preventDefault()
        if (symbolInput.trim() && activePane) {
          setSymbol(activePane, symbolInput.trim().toUpperCase())
          setSymbolInput('')
        }
      }} style={{ display: 'flex' }}>
        <input
          value={symbolInput}
          onChange={e => setSymbolInput(e.target.value)}
          placeholder="Symbol..."
          style={{
            background: '#1a1a1a', color: '#ccc', border: '1px solid #333',
            padding: '2px 8px', width: 80, fontSize: 12, fontFamily: 'monospace',
          }}
        />
      </form>

      <div style={{ display: 'flex', gap: 2 }}>
        {TIMEFRAMES.map(tf => (
          <button key={tf}
            onClick={() => activePane && setTimeframe(activePane, tf)}
            style={{
              background: pane?.timeframe === tf ? '#2a6496' : '#1a1a1a',
              color: '#ccc', border: '1px solid #333',
              padding: '2px 8px', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace',
            }}
          >{tf}</button>
        ))}
      </div>

      <div style={{ marginLeft: 'auto', color: '#333', fontSize: 10, letterSpacing: 2 }}>
        APEX TERMINAL
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Add Toolbar to App.tsx**

```typescript
// src/App.tsx
import './global.css'
import { Toolbar } from './toolbar/Toolbar'
import { Workspace } from './workspace/Workspace'

export default function App() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', width: '100vw', height: '100vh', overflow: 'hidden' }}>
      <Toolbar />
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <Workspace />
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Verify**

Expected: Click a pane to select it (blue border). Type symbol + Enter → chart changes. Click timeframe → chart reloads.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: toolbar with symbol input and timeframe selector"
```

---

## Task 11: Drawing Overlay (Trendlines + Horizontal Lines)

**Files:**
- Create: `src/store/drawingStore.ts`
- Create: `src/chart/DrawingOverlay.tsx`
- Create: `src/tests/drawingStore.test.ts`
- Modify: `src/chart/ChartPane.tsx`
- Modify: `src/toolbar/Toolbar.tsx`

- [ ] **Step 1: Install uuid**

```bash
npm install uuid && npm install -D @types/uuid
```

- [ ] **Step 2: Write failing tests for drawingStore**

```typescript
// src/tests/drawingStore.test.ts
import { describe, it, expect, beforeEach } from 'vitest'
import { useDrawingStore } from '../store/drawingStore'

describe('drawingStore', () => {
  beforeEach(() => useDrawingStore.getState().clear())

  it('adds a drawing', () => {
    useDrawingStore.getState().addDrawing({
      id: '1', type: 'trendline', points: [], color: '#fff', symbol: 'AAPL', timeframe: '5m'
    })
    expect(useDrawingStore.getState().drawings).toHaveLength(1)
  })

  it('removes a drawing', () => {
    useDrawingStore.getState().addDrawing({
      id: '1', type: 'trendline', points: [], color: '#fff', symbol: 'AAPL', timeframe: '5m'
    })
    useDrawingStore.getState().removeDrawing('1')
    expect(useDrawingStore.getState().drawings).toHaveLength(0)
  })

  it('filters by symbol and timeframe', () => {
    const s = useDrawingStore.getState()
    s.addDrawing({ id: '1', type: 'trendline', points: [], color: '#fff', symbol: 'AAPL', timeframe: '5m' })
    s.addDrawing({ id: '2', type: 'trendline', points: [], color: '#fff', symbol: 'MSFT', timeframe: '5m' })
    expect(s.drawingsFor('AAPL', '5m')).toHaveLength(1)
  })
})
```

- [ ] **Step 3: Run tests — expect FAIL**

- [ ] **Step 4: Implement drawingStore**

```typescript
// src/store/drawingStore.ts
import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { Drawing, DrawingTool, Timeframe } from '../types'

interface DrawingStore {
  drawings: Drawing[]
  activeTool: DrawingTool
  setActiveTool: (tool: DrawingTool) => void
  addDrawing: (d: Drawing) => void
  removeDrawing: (id: string) => void
  drawingsFor: (symbol: string, tf: Timeframe) => Drawing[]
  clear: () => void
}

export const useDrawingStore = create<DrawingStore>()(
  persist(
    (set, get) => ({
      drawings: [],
      activeTool: 'cursor',
      setActiveTool: tool => set({ activeTool: tool }),
      addDrawing: d => set(s => ({ drawings: [...s.drawings, d] })),
      removeDrawing: id => set(s => ({ drawings: s.drawings.filter(d => d.id !== id) })),
      drawingsFor: (symbol, tf) => get().drawings.filter(d => d.symbol === symbol && d.timeframe === tf),
      clear: () => set({ drawings: [] }),
    }),
    { name: 'apex-drawings' }
  )
)
```

- [ ] **Step 5: Run tests — expect PASS**

- [ ] **Step 6: Write DrawingOverlay**

```typescript
// src/chart/DrawingOverlay.tsx
import { useRef, useCallback, useState, useEffect } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { CoordSystem } from './CoordSystem'
import type { Point, Timeframe } from '../types'
import { v4 as uuid } from 'uuid'

interface Props {
  symbol: string
  timeframe: Timeframe
  cs: CoordSystem
  width: number
  height: number
  viewStart: number
}

export function DrawingOverlay({ symbol, timeframe, cs, width, height, viewStart }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { activeTool, drawingsFor, addDrawing } = useDrawingStore()
  const [inProgress, setInProgress] = useState<Point | null>(null)
  const mouseRef = useRef({ x: 0, y: 0 })

  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')!
    ctx.clearRect(0, 0, width, height)

    const drawings = drawingsFor(symbol, timeframe)
    ctx.lineWidth = 1.5

    for (const d of drawings) {
      ctx.strokeStyle = d.color
      if (d.type === 'trendline' && d.points.length === 2) {
        ctx.beginPath()
        ctx.moveTo(cs.barToX(d.points[0].time - viewStart), cs.priceToY(d.points[0].price))
        ctx.lineTo(cs.barToX(d.points[1].time - viewStart), cs.priceToY(d.points[1].price))
        ctx.stroke()
      }
      if (d.type === 'hline' && d.points.length >= 1) {
        const y = cs.priceToY(d.points[0].price)
        ctx.beginPath()
        ctx.moveTo(0, y)
        ctx.lineTo(width - cs.pr, y)
        ctx.stroke()
      }
    }

    // In-progress trendline
    if (inProgress && activeTool === 'trendline') {
      ctx.strokeStyle = 'rgba(74,158,255,0.6)'
      ctx.setLineDash([4, 4])
      ctx.beginPath()
      ctx.moveTo(cs.barToX(inProgress.time - viewStart), cs.priceToY(inProgress.price))
      ctx.lineTo(mouseRef.current.x, mouseRef.current.y)
      ctx.stroke()
      ctx.setLineDash([])
    }
  }, [cs, symbol, timeframe, drawingsFor, activeTool, inProgress, width, height, viewStart])

  useEffect(() => { draw() }, [draw])

  const onClick = useCallback((e: React.MouseEvent) => {
    if (activeTool === 'cursor') return
    const rect = canvasRef.current!.getBoundingClientRect()
    const x = e.clientX - rect.left
    const y = e.clientY - rect.top
    const barIdx = Math.round(cs.xToBar(x)) + viewStart
    const price = cs.yToPrice(y)

    if (activeTool === 'trendline') {
      if (!inProgress) {
        setInProgress({ time: barIdx, price })
      } else {
        addDrawing({
          id: uuid(), type: 'trendline',
          points: [inProgress, { time: barIdx, price }],
          color: '#4a9eff', symbol, timeframe,
        })
        setInProgress(null)
      }
    }
    if (activeTool === 'hline') {
      addDrawing({
        id: uuid(), type: 'hline',
        points: [{ time: barIdx, price }],
        color: '#4a9eff', symbol, timeframe,
      })
    }
  }, [activeTool, inProgress, cs, addDrawing, symbol, timeframe, viewStart])

  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const rect = canvasRef.current!.getBoundingClientRect()
    mouseRef.current = { x: e.clientX - rect.left, y: e.clientY - rect.top }
    if (inProgress) draw()
  }, [inProgress, draw])

  return (
    <canvas
      ref={canvasRef} width={width} height={height}
      style={{
        position: 'absolute', top: 0, left: 0,
        cursor: activeTool !== 'cursor' ? 'crosshair' : 'default',
        pointerEvents: activeTool === 'cursor' ? 'none' : 'auto',
      }}
      onClick={onClick}
      onMouseMove={onMouseMove}
    />
  )
}
```

- [ ] **Step 7: Add DrawingOverlay to ChartPane**

In ChartPane.tsx, after CrosshairOverlay:
```typescript
import { DrawingOverlay } from './DrawingOverlay'

// In JSX:
{cs && data && (
  <DrawingOverlay symbol={symbol} timeframe={timeframe} cs={cs}
    width={width} height={height} viewStart={viewStart} />
)}
```

- [ ] **Step 8: Add drawing tool buttons to Toolbar**

```typescript
import { useDrawingStore } from '../store/drawingStore'

// Inside Toolbar component:
const { activeTool, setActiveTool } = useDrawingStore()

// After timeframe buttons:
<div style={{ display: 'flex', gap: 2, marginLeft: 12, borderLeft: '1px solid #333', paddingLeft: 12 }}>
  {(['cursor', 'trendline', 'hline'] as const).map(tool => (
    <button key={tool}
      onClick={() => setActiveTool(tool)}
      style={{
        background: activeTool === tool ? '#2a6496' : '#1a1a1a',
        color: '#ccc', border: '1px solid #333',
        padding: '2px 8px', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace',
      }}
    >{tool}</button>
  ))}
</div>
```

- [ ] **Step 9: Verify drawings**

Expected: Select trendline tool → click twice on chart → line drawn. Select hline → click → horizontal line. Persisted across refreshes.

- [ ] **Step 10: Commit**

```bash
git add -A && git commit -m "feat: drawing overlay with trendline and hline tools, persisted to localStorage"
```

---

## Task 12: Multi-Window Support

**Files:**
- Modify: `src/toolbar/Toolbar.tsx`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Add "New Window" button to Toolbar**

```typescript
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

// In Toolbar, add a new window button after the APEX TERMINAL label:
<button
  onClick={async () => {
    const label = `chart-${Date.now()}`
    new WebviewWindow(label, {
      title: 'Apex Terminal',
      width: 1920,
      height: 1080,
      decorations: true,
    })
  }}
  style={{
    background: '#1a1a1a', color: '#555', border: '1px solid #333',
    padding: '2px 8px', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace',
    marginLeft: 8,
  }}
>+ Window</button>
```

- [ ] **Step 2: Install Tauri API package if needed**

```bash
npm install @tauri-apps/api
```

- [ ] **Step 3: Update Tauri capabilities for multi-window**

Ensure `src-tauri/capabilities/default.json` has:
```json
{
  "identifier": "default",
  "windows": ["*"],
  "permissions": [
    "core:default",
    "core:window:allow-create",
    "core:window:default",
    "core:webview:allow-create-webview-window"
  ]
}
```

- [ ] **Step 4: Verify multi-window**

Expected: Click "+ Window" → new Tauri window opens with its own 7-pane workspace. Each window runs independently.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: multi-window support via Tauri webview window API"
```

---

## Done — What You Have

- 7 chart panes per window, each with independent symbol + timeframe
- WebGPU instanced candle rendering (clip-space, 18 verts/candle, one draw call)
- Category axis (no weekend/holiday gaps)
- Anti-aliased SMA-20 + EMA-50 line overlays (instanced quads + SDF)
- Grid lines + price/time axis labels
- Crosshair with live price/time readout
- Drag to pan, scroll to zoom
- Trendline + horizontal line drawing tools, persisted to localStorage
- Toolbar: symbol input, timeframe selector, drawing tool picker
- Multi-window support (open unlimited chart windows)
- Real market data from yfinance

**Next phase** (not in this plan): real-time WebSocket tick feed, volume bars, Fibonacci tool, alert system, order entry.
