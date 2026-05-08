# SPEC: GPU Chart Canvas Refactor

**Tag:** `pre-gpu-refactor` marks the last commit before this work.
**Goal:** Replace egui-based chart rendering with a dedicated wgpu render pass for candles, indicators, and drawings. egui retains chrome (panels, menus, axes, sidebars).
**Why:** Telemetry shows the app is CPU-bound on egui layout (8.3ms/frame at p50, p99=33ms). The chart is the part that scales with bars × indicators × tick rate. Moving the chart to a custom GPU pipeline removes scaling cliffs and frees frame budget permanently.

---

## 1. Baseline (recorded at `pre-gpu-refactor`)

Sampled from `:9091/metrics` on RTX 3090, ~60s uptime, idle-to-active workload:

| Metric | Value |
|---|---|
| FPS avg | 58.9 |
| Avg frame | 16.99 ms |
| p99 frame | 33.25 ms |
| Max frame | 45.65 ms |
| Jank rate (>33ms) | 0.30% (19/6269) |
| Heap allocs/frame | 12,529 |
| Heap bytes/frame | 4,979,107 |
| GPU 0 utilization | 33% |

**Phase breakdown (avg):**

| Phase | µs | % of frame |
|---|---:|---:|
| acquire | 3,486 | 20.5% |
| layout | 8,294 | 48.8% |
| tessellate | 2,617 | 15.4% |
| upload | 781 | 4.6% |
| render | 1,555 | 9.2% |
| present | 235 | 1.4% |

**Chart subsystem timings (avg µs):**

| Subsystem | Avg | Max |
|---|---:|---:|
| chart_panes | 1,079 | 2,748 |
| chart_canvas | 238 | 927 |
| indicator_paint | 120 | 557 |
| drawings_paint | 8 | 134 |
| signal_overlays | 8 | 178 |

`chart_canvas` understates true chart cost — its egui `Shape`s flow into the global tessellate phase (2.6ms) which is not subsystem-attributed.

---

## 2. Target (post-refactor exit criteria)

| Metric | Target | Reason |
|---|---|---|
| FPS avg | ≥ 59.5 | Stable 60Hz |
| p99 frame | ≤ 17 ms | One vsync — no visible jank |
| Max frame | ≤ 25 ms | No 2-vsync stalls |
| Chart pass time | ≤ 1 ms (5k bars, 8 indicators) | New phase metric |
| Tick storm degradation | ≤ 5% frame time delta | Tick decoupled from frame |
| Heap allocs/frame | ≤ 6,000 | Halve allocator pressure |
| Bars-on-screen scaling | flat to 50k bars | GPU-instanced rendering |

Stretch: 144Hz target (≤ 6.9 ms p99) once Phase 6 lands.

---

## 3. Architecture

### 3.1 Current (pre-refactor)

```
[Chart::draw_chart]
  → egui paint commands (rects, lines, text)
    → egui tessellator (CPU)
      → egui_renderer.update_buffers (CPU→GPU copy)
        → egui render pass (GPU)
```

The chart and the chrome share one pipeline. Chart cost competes with UI cost for the same frame budget.

### 3.2 Target (post-refactor)

```
[Frame]
  ├─ Chart pass:
  │    [chart::renderer_gpu::draw_chart_pass]
  │      → custom wgpu pipelines (candles, lines, drawings)
  │        → render pass into surface texture
  │
  └─ Chrome pass:
       [egui draws panels, menus, axes, header, sidebar]
         → egui tessellator
           → egui render pass into same surface texture (on top)
```

Two render passes per frame. The chart pass owns its own pipelines, buffers, shaders, and viewport. egui no longer paints anything inside the chart rect.

### 3.3 New module layout

```
src-tauri/src/chart/renderer_gpu/        ← NEW module, separate from existing renderer/
  mod.rs                                  ← public API: draw_chart_pass, init, resize
  pipeline.rs                             ← pipeline state: shaders, layouts, buffer pool
  buffers.rs                              ← persistent vertex/instance buffers + writers
  candles/
    mod.rs                                ← candle pass entry
    shader.wgsl                           ← instanced quad shader
    instance.rs                           ← CandleInstance layout
  indicators/
    mod.rs                                ← indicator pass entry
    line.wgsl                             ← line/polyline shader
    incremental.rs                        ← O(1) update logic per indicator type
    state.rs                              ← per-indicator GPU buffer handles
  drawings/
    mod.rs                                ← drawing pass entry
    shader.wgsl
    cache.rs                              ← committed-drawing mesh cache
  axes/                                   ← OPTIONAL Phase 6 — may stay in egui
    mod.rs
    shader.wgsl
  uniforms.rs                             ← view matrix, viewport, theme uniforms
  text/
    mod.rs                                ← SDF or atlas-based text for in-canvas labels
```

The existing `chart/renderer/` (egui-based) stays untouched during Phase 1-2 and is incrementally turned off as new passes come online.

---

## 4. Phased Plan

Each phase ships independently, behind a feature flag (`gpu_chart_v2`) that defaults off until Phase 7. Existing egui chart path remains the fallback.

### Phase 0 — Instrumentation prep (1 day) ★ blocking

Before writing any new code, add the measurement scaffolding for the new pipeline.

- [ ] Add new phase timer in `monitoring.rs`: `phase_chart_pass_us` (parallel to existing six phases).
- [ ] Add subsystem timers: `gpu_candles`, `gpu_indicators`, `gpu_drawings`.
- [ ] Add `apex_chart_visible_bars` gauge.
- [ ] Add `apex_chart_pipeline_active` gauge (0=egui path, 1=gpu path) so we can A/B compare from telemetry.
- [ ] Build a small Bash script `scripts/perf_snapshot.sh` that curls `:9091/metrics` and prints a one-screen frame summary. Run before/after each phase, paste output into commit message.

**Exit criteria:** can call the script, see all current metrics, and the new (zero-valued) chart_pass timer.

### Phase 1 — wgpu pipeline foundation (3-5 days)

Goal: prove a second render pass can interleave with egui's pass on the same surface.

- [ ] Create `chart/renderer_gpu/` module skeleton.
- [ ] In `App::redraw_requested` (after egui surface acquire, before egui pass), insert a no-op chart pass that clears a sub-rect of the surface to a debug color (magenta).
- [ ] Verify chart area shows magenta with egui chrome on top.
- [ ] Wire the `gpu_chart_v2` feature flag — when off, magenta does not paint and old path runs.
- [ ] Add `phase_chart_pass_us` reporting from inside the new pass.

**Exit criteria:** With flag on, magenta renders inside chart rect; egui chrome still drawable on top; flag off restores baseline.

**Risk:** Surface format / depth attachment compatibility between two passes. Mitigation: load existing surface texture into a new render pass with `LoadOp::Load`, no depth.

### Phase 2 — Candle rendering (1 week)

Goal: candles render via instanced GPU draws; egui no longer paints them.

- [ ] Define `CandleInstance { x: f32, open: f32, high: f32, low: f32, close: f32, flags: u32 }` (24 bytes).
- [ ] Persistent instance buffer sized for 100k bars; only `queue.write_buffer` on bar append/replace.
- [ ] WGSL shader: vertex shader expands one instance to body quad (6 verts) + wick line (2-vertex line list, separate draw).
- [ ] Uniform buffer: view matrix (pan/zoom), viewport size, up/down/doji colors from active theme.
- [ ] Integrate with existing `Chart::view` state — read pan/zoom from chart, write to uniform.
- [ ] Switch egui chart-bar painting OFF when `gpu_chart_v2` enabled (gate the painter calls).
- [ ] Verify visual parity: side-by-side screenshots vs egui path.
- [ ] Tick path: on incoming tick, compute updated CandleInstance for live bar, `queue.write_buffer` at offset (no other state change, no repaint request).
- [ ] Run `perf_snapshot.sh` — record chart_pass time, allocations, frame time.

**Exit criteria:**
- Visual diff vs old path: imperceptible at all zoom levels.
- chart_pass < 0.5ms with 5k bars on screen.
- Tick storms (1000 ticks/sec) cause < 5% frame time delta.

**Risk:** color theme drift (subpixel-AA-aware bg luminance was tuned for egui glyphs, candle colors might look off). Mitigation: pass theme colors via uniform, match egui rendering output pixel-for-pixel.

### Phase 3 — Tick decoupling (3 days)

Goal: ticks stop driving repaints. Frames driven by vsync only.

- [ ] Audit all `request_repaint*` calls in tick handlers — replace with buffer writes.
- [ ] Live bar buffer slot identified by index, written on every tick.
- [ ] If a new bar starts, append to buffer; trim oldest if needed (ring buffer).
- [ ] Verify with `apex_frame_fps` — should remain steady ~60 during tick storm; old path showed dips.

**Exit criteria:** scripted tick injection at 5000/sec maintains ≥58fps. Pre-refactor baseline drops to ~45fps under same load (verify and record).

### Phase 4 — Indicators on GPU (1 week)

Goal: indicator lines rendered as GPU instances. CPU does only O(1) incremental updates.

- [ ] Per-indicator GPU buffer (Vec<f32>) sized to bar buffer.
- [ ] Move incremental compute to `indicators/incremental.rs`:
  - SMA(N): O(1) sliding window
  - EMA: O(1) recursive
  - RSI(14): O(1) Wilder smoothing
  - MACD: two EMAs + diff, O(1)
  - Bollinger: rolling mean + Welford std, O(1)
- [ ] Shader: instanced line strip from buffer of (x, y) pairs.
- [ ] Color/style from per-indicator uniform.
- [ ] Switch egui indicator_paint OFF when flag enabled.
- [ ] Multi-pass indicators (SuperTrend, fractals) deferred to Phase 4b — these may still go through egui temporarily, or use a compute shader for backfill.

**Exit criteria:** All 5 baseline indicators (SMA, EMA, RSI, MACD, BB) render via GPU. indicator_paint subsystem timer drops to ~0. Adding a 6th indicator costs <50µs/frame.

**Risk:** indicator preset / style customization (per-component colors, line widths, shading) — current egui path supports per-pixel control. Mitigation: pack style into instance data or per-indicator uniform; verify all preset combinations render identically.

### Phase 5 — Drawings (3-5 days)

Goal: 22 drawing types rendered from a cached mesh, retessellated only on edit.

- [ ] `DrawingMeshCache` keyed by drawing ID; entries are `wgpu::Buffer` handles.
- [ ] On drawing create / move / edit: tessellate to mesh, upload, store handle.
- [ ] Per frame: iterate visible drawings, issue one draw call each (or instanced batch by primitive type).
- [ ] In-progress drawing (cursor follows mouse): tessellate per frame as today — small, fine.
- [ ] Hit-testing stays on CPU (already 8µs avg).
- [ ] Significance tooltips remain in egui — they're chrome, not chart.

**Exit criteria:** 50 active drawings render with chart_pass total still <1ms. Editing a drawing (drag handle) does not slow the frame.

### Phase 6 — Axes, gridlines, crosshair (optional, 3 days)

Decision point at Phase 5 exit: do we move axes to GPU?

- **Pros:** unified pipeline, no egui in the chart rect at all, easier to push to 144/240Hz.
- **Cons:** text rendering is the hard part — would need SDF font atlas or equivalent. egui's text engine is high-quality.

**If yes:**
- [ ] SDF glyph atlas for axis labels (a single texture, generated at startup).
- [ ] Gridlines as instanced line draws.
- [ ] Crosshair as 2-line draw, position uniform.

**If no:** axes stay in egui, layered on top of chart pass. Performance acceptable; minor blur risk where text overlaps candles.

Recommendation: **defer this decision** until Phase 5 telemetry is in. Likely "no" unless chasing 144Hz.

### Phase 7 — Cutover, cleanup, default-on (3 days)

- [ ] Flip `gpu_chart_v2` default to ON.
- [ ] Remove egui chart-paint code paths (now dead).
- [ ] Delete `chart_canvas`, `indicator_paint`, `drawings_paint` egui-side instrumentation; replace with new GPU-pass equivalents.
- [ ] Update `AGENT_BRIEF.md` to describe new architecture.
- [ ] Tag `post-gpu-refactor`.

**Exit criteria:** all targets in §2 met. No feature regression in 18 indicators / 22 drawings / bar replay / Renko / Range / Tick.

---

## 5. Decisions Locked at Spec Time

These choices avoid Phase-X bikeshedding:

1. **Two render passes, not a single composited texture.** Cleaner, lower latency, no extra blit.
2. **Persistent instance buffer, not per-frame upload.** Allocations and bandwidth saved.
3. **Incremental indicator compute on CPU, rendering on GPU.** Compute shaders not justified for O(1) recursive indicators; reserved for Phase 4b multi-pass cases.
4. **Tessellate drawings once, cache mesh.** Most drawings are static after creation.
5. **Hit-testing stays on CPU.** Cheap and avoids GPU readback.
6. **Theme colors come through uniforms, not per-vertex.** Theme switches don't invalidate buffers.
7. **Feature-flagged rollout.** Old path is a fallback through Phase 7.
8. **No egui changes.** This refactor adds a peer pipeline, doesn't fork or modify egui.

---

## 6. What This Does NOT Address

To keep scope honest:

- The 8.3ms egui layout cost. Chrome layout is unchanged.
- The 12,529 allocs/frame number. Only the chart's share is removed (estimated 1-3k).
- The 3.5ms `acquire` phase. Likely vsync wait; investigate separately if it persists.
- Cosmic-text usage audit. Separate workstream.
- Animation system idle-frame audit. Separate workstream.

These are **after** this spec, not part of it. If the user says "the rest of the UI feels slow" post-refactor, that's a different project (allocation hunt + egui caching).

---

## 7. Risks and Rollback

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Visual diff vs egui path | Medium | High (user-facing) | Side-by-side screenshot tests at each phase exit; pixel-diff acceptance |
| Theme/preset breakage | Medium | Medium | Iterate all 18 indicators × 22 drawings × N themes during Phase 4-5 QA |
| Surface format conflict | Low | High | Phase 1 proves the two-pass setup works before any chart code is moved |
| Performance regression on integrated GPUs | Low | Medium | Test on Intel iGPU before Phase 7 cutover |
| Tick path race conditions | Medium | High | Tick writer is single-threaded into a per-bar slot; reads happen on render thread; test with thread sanitizer |

**Rollback:** at any phase, set `gpu_chart_v2 = false`. The old egui path continues to work through Phase 6. Phase 7 (cutover) is the point of no return — defer until §2 targets are met *and* a full QA pass is clean.

---

## 8. Tracking

- Tag `pre-gpu-refactor` — current state, this commit baseline.
- Tag at each phase exit: `gpu-chart-phase-{1..7}`.
- Each phase commit message includes a `perf_snapshot.sh` paste.
- Tag `post-gpu-refactor` at Phase 7 close.

---

## 9. Total Estimate

**4-6 weeks of focused work**, broken into 7 phases each independently shippable. Phase 1 and 2 are the highest risk; if Phase 2 is on track at end of week 2, the rest is mechanical.

If at end of Phase 2 the chart_pass timer is over 1ms or visual parity isn't clean, **stop and reconsider** — the cost model assumed for the rest of the plan no longer holds.
