# Multi-Window Support — Implementation Plan

## Why this doc exists

The F12 inspector's POP button currently opens the inspector as a draggable `egui::Window` floating *inside* the main app window. Users want POP to open a REAL OS window so they can drag it to a second monitor.

The earlier wave 2.1 attempt used `ctx.show_viewport_immediate` from egui's viewport API. **That doesn't work in this codebase** — egui's viewport API requires the host (App in `gpu.rs:6629`) to implement viewport-creation plumbing. Apex-terminal is custom winit + egui-wgpu (not eframe), so when egui requests a viewport, nobody's listening and it falls back to embedded rendering (silently paints into the current window with no visible effect).

Reverted in commit `6103b34f`. POP now works via egui::Window again (inside-main-window only).

This document scopes the proper implementation. Estimated effort: **1-2 days of focused human-driven work, sacred-adjacent code**.

## Current architecture (the constraint)

```
App {
    cw: Option<ChartWindow>,     // ← exactly one
    ...
}

ChartWindow {
    win: Arc<winit::Window>,
    gpu: GpuCtx,
}

GpuCtx {
    device, queue, surface, config,    // ← one wgpu surface
    egui_ctx,                          // ← one egui::Context
    egui_state,                        // ← one egui_winit::State
    egui_renderer,                     // ← one egui_wgpu::Renderer
    ...
}

ApplicationHandler::window_event(_, wid, ev) {
    // Dispatches assuming wid == cw.win.id()
    // No HashMap, no multi-window awareness
}
```

The chart's hot paint path (`render_pane` etc. in core.rs) takes `&mut self.gpu` from this single ChartWindow — sacred-adjacent code that must not be touched as part of this work.

## Target architecture (the destination)

```
App {
    windows: HashMap<WindowId, AppWindow>,   // multi-window
    chart_wid: Option<WindowId>,             // which one is the chart
    inspector_wid: Option<WindowId>,         // which one is the inspector
    ...
}

enum AppWindow {
    Chart(ChartWindow),
    Inspector(InspectorWindow),
}

struct InspectorWindow {
    win: Arc<winit::Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,               // separate context per OS window
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}
```

Key design decisions:

1. **Separate `egui::Context` per OS window.** Sharing one Context across multiple winit windows requires egui's viewport API (which requires host plumbing — that's what we're trying to avoid by going this route). Two contexts is cleaner.

2. **Shared `DesignTokens` global state.** Already in place (`OnceLock<RwLock<DesignTokens>>`). Both contexts read/write the same tokens, so inspector edits propagate to the chart window via the normal `dt_f32!`/`dt_u8!` path.

3. **Shared `wgpu::Device` + `wgpu::Queue`.** The same logical device can drive multiple surfaces. Don't create a second adapter/device.

4. **Per-window render loop.** Each window has its own `redraw_requested` cycle. The chart window does the heavy work; the inspector window does ~3000 lines of UI per frame (cheap).

## The 5 changes

### 1. Refactor `App` struct
**File**: `src/chart/renderer/gpu.rs:6580-ish` (App struct definition).

```rust
pub struct App {
    pub event_loop_proxy: EventLoopProxy<AppEvent>,
    // BEFORE: cw: Option<ChartWindow>,
    pub windows: HashMap<WindowId, AppWindow>,
    pub chart_wid: Option<WindowId>,
    pub inspector_wid: Option<WindowId>,
    // ... other fields unchanged
}

pub enum AppWindow {
    Chart(ChartWindow),
    Inspector(InspectorWindow),
}
```

All access to `app.cw` becomes `app.chart_wid.and_then(|wid| app.windows.get_mut(&wid).and_then(|aw| match aw { AppWindow::Chart(cw) => Some(cw), _ => None }))`. Helper methods: `chart_mut(&mut self) -> Option<&mut ChartWindow>`, `inspector_mut(&mut self) -> Option<&mut InspectorWindow>`.

### 2. Implement `InspectorWindow`
**File**: NEW `src/chart/renderer/inspector_window.rs`.

```rust
pub struct InspectorWindow {
    pub win: Arc<winit::Window>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub egui_renderer: egui_wgpu::Renderer,
}

impl InspectorWindow {
    pub fn create(el: &ActiveEventLoop, device: &wgpu::Device, queue: &wgpu::Queue, adapter: &wgpu::Adapter)
        -> Result<Self, ...>;
    pub fn render(&mut self);  // Per-frame: run egui, paint, present.
    pub fn on_window_event(&mut self, ev: &WindowEvent) -> EventResponse;
}
```

The `render` method calls `egui_ctx.run(...)` with `Inspector::show_inspector_body` inside.

### 3. Wire `App::window_event` for multi-window
**File**: `src/chart/renderer/gpu.rs:6638`.

```rust
fn window_event(&mut self, _el: &ActiveEventLoop, wid: WindowId, ev: WindowEvent) {
    if Some(wid) == self.chart_wid {
        // Existing chart handling — verbatim, no changes
    } else if Some(wid) == self.inspector_wid {
        if let Some(AppWindow::Inspector(insp_win)) = self.windows.get_mut(&wid) {
            let _ = insp_win.egui_state.on_window_event(&insp_win.win, &ev);
            match ev {
                WindowEvent::CloseRequested => {
                    self.windows.remove(&wid);
                    self.inspector_wid = None;
                    // Reset inspector.is_popout = false
                    DESIGN_INSPECTOR.with(|cell| {
                        if let Some(insp) = cell.borrow_mut().as_mut() {
                            insp.is_popout = false;
                        }
                    });
                }
                WindowEvent::RedrawRequested => insp_win.render(),
                WindowEvent::Resized(s) => { /* reconfigure surface */ }
                _ => {}
            }
        }
    }
}
```

### 4. POP click handler
**File**: `src/chart/renderer/render/pane/core.rs:12030-ish` (the inspector dispatch).

Detect transition `is_popout: false → true` and send an `AppEvent::OpenInspectorWindow` through the event loop proxy. The App's `user_event` handler creates the InspectorWindow and adds to the HashMap.

```rust
// In core.rs near inspector dispatch:
let prev_popout = INSPECTOR_PREV_POPOUT.with(|c| c.replace(insp_popout));
if !prev_popout && insp_popout {
    crate::NATIVE_PROXY.with(|p| { let _ = p.send_event(AppEvent::OpenInspectorWindow); });
    // Skip inspector.show this frame — next frame it'll render in its own window
} else if !insp_popout {
    // Existing inline show
}
```

### 5. Add the inline-render branch
When `is_popout == true` and the InspectorWindow exists, skip the inline rendering in `core.rs` entirely (the inspector renders itself via its own `redraw_requested` loop).

## Verification plan

After each of the 5 changes, run:
```bash
cd src-tauri
cargo apex --features design-mode
```
And verify:

- **Step 1 (App refactor)**: App launches. Chart paints. F12 inspector still opens docked.
- **Step 2 (InspectorWindow)**: Type-checks (no runtime test yet).
- **Step 3 (window_event multi-window)**: App still launches. Chart events work.
- **Step 4 + 5 (POP handler + render branch)**: F12 → POP → real OS window appears with the inspector. Drag to second monitor. Slider edits propagate to chart window. Close inspector window → no crash, F12 reopens docked.

## Known risks

1. **Surface invalidation on resize** — winit may re-create the surface on resize; need to reconfigure correctly.
2. **Per-window scale factor** — multi-monitor with different DPIs is a footgun. egui_winit::State::set_scale_factor per window.
3. **Event loop proxy lifetime** — sending events from the inspector's wgpu thread back to the App requires a proxy clone with appropriate `Send`.
4. **macOS NSApp lifecycle** — adding a second window mid-run on macOS is reliable but make sure the second window isn't auto-closed when the first one loses focus.
5. **Windows DWM corner rounding code** in `gpu.rs:6388-6438` is for the chart window only — don't run it on the inspector window.

## Why fan-out agents are wrong here

The refactor touches:
- App struct (state shape change → every accessor changes)
- window_event (dispatch logic → if wrong, app freezes or routes events wrong)
- core.rs inspector dispatch (sacred-adjacent)
- New file (InspectorWindow)

Parallel agents would:
- Conflict on App struct edits.
- Have inconsistent assumptions about where state lives.
- Need to read 6000+ lines of gpu.rs each to understand the App architecture.
- Risk introducing a deadlock between the two wgpu render loops.

Single human-paired session = safer + faster.

## Out of scope for this work

- **N extra windows**. Solve for 1 (inspector). Generalizing to N requires the viewport API or a more sophisticated dispatcher.
- **Pop-out chart pane to its own window**. Would need to make `ChartWindow` createable multiple times — bigger refactor.
- **Pop-out settings dialog**. Same problem — would need similar plumbing for each "poppable" dialog.

After the inspector pop-out lands and is stable, generalize to a single `pub struct UtilityWindow<T: UtilityWindowContent>` pattern that any future dialog/inspector can use.

## Reference implementations to study

- **eframe** — `egui/crates/eframe/src/native/run.rs` handles viewport creation around line 200. Read for the canonical pattern.
- **rerun** — uses egui directly with multi-viewport. Their integration is the most complex production example.

## Estimated breakdown

| Step | Hours |
|---|---|
| 1. App struct refactor + helper methods | 3-4 |
| 2. InspectorWindow impl | 2-3 |
| 3. Multi-window window_event dispatch | 2-3 |
| 4. POP click handler + AppEvent wiring | 1-2 |
| 5. Inline-render skip branch | 0.5 |
| Visual verification + bugfixes | 3-4 |
| **Total** | **~12-16 hours** |

That's a focused 2-day commitment with you available for visual checks between steps.

---

*Authored 2026-05-24 after the wave 2.1 viewport_immediate approach was found incompatible with the custom winit+egui-wgpu architecture.*
