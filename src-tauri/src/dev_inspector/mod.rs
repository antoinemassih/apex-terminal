//! Dev Inspector — in-process testing platform, debug builds only.
//!
//! Architecture: two shared mutexes + an HTTP server on :7891.
//! The app loop calls `begin_frame()` / `end_frame()` at each render cycle.
//! An HTTP server thread reads `DevSharedState` and writes `DevQueues`.
//! External scripts, CI runners, and AI agents drive the app through the REST API.
//!
//! Zero production overhead: the entire module is compiled out by
//! `#[cfg(debug_assertions)]`.

use std::sync::{Arc, Mutex, OnceLock};
use std::cell::RefCell;

pub mod input_queue;
pub mod assert_engine;
pub mod layout;
pub mod server;

// ─── Serialisable rect ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SerRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl SerRect {
    pub fn zero() -> Self { Self::default() }
}

impl From<egui::Rect> for SerRect {
    fn from(r: egui::Rect) -> Self {
        SerRect { x: r.min.x, y: r.min.y, w: r.width(), h: r.height() }
    }
}

impl SerRect {
    pub fn to_egui(&self) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(self.x, self.y),
            egui::vec2(self.w, self.h),
        )
    }
    pub fn contains(&self, other: &SerRect) -> bool {
        self.x <= other.x && self.y <= other.y
            && self.x + self.w >= other.x + other.w
            && self.y + self.h >= other.y + other.h
    }
    pub fn area(&self) -> f32 { self.w * self.h }
    pub fn min_side(&self) -> f32 { self.w.min(self.h) }
}

// ─── Widget record ────────────────────────────────────────────────────────────

/// A record of a semantically meaningful UI element, captured each frame.
/// IDs use dot-path notation: `"toolbar.save_btn"`, `"pane.0.symbol"`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WidgetRecord {
    pub id: String,
    pub role: String,           // "button", "label", "input", "status", "header", etc.
    pub label: String,
    pub value: Option<String>,
    pub rect: SerRect,
    pub clip_rect: SerRect,
    pub layer: u8,
    pub focused: bool,
    pub hovered: bool,
    pub enabled: bool,
    pub is_clipped: bool,
}

impl WidgetRecord {
    pub fn state(id: impl Into<String>, role: &str, label: impl Into<String>) -> Self {
        WidgetRecord {
            id: id.into(), role: role.into(), label: label.into(),
            value: None, rect: SerRect::zero(), clip_rect: SerRect::zero(),
            layer: 0, focused: false, hovered: false, enabled: true, is_clipped: false,
        }
    }
    pub fn from_response(
        id: impl Into<String>, role: &str, label: impl Into<String>,
        resp: &egui::Response, ui: &egui::Ui,
    ) -> Self {
        let rect: SerRect = resp.rect.into();
        let clip_rect: SerRect = ui.clip_rect().into();
        let min_dim = rect.min_side();
        WidgetRecord {
            id: id.into(), role: role.into(), label: label.into(),
            value: None,
            is_clipped: min_dim > 0.0 && min_dim < 28.0,
            rect, clip_rect,
            layer: 0,
            focused: resp.has_focus(),
            hovered: resp.hovered(),
            enabled: ui.is_enabled(),
        }
    }
}

// ─── Design contract violation ────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContractViolation {
    pub widget_id: String,
    pub constraint: String,
    pub detail: String,
    pub frame: u64,
}

// ─── SSE event ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct SseEvent {
    pub name: String,
    pub data: serde_json::Value,
    pub seq: u64,
}

// ─── Shared state (read by HTTP thread, written by app loop) ─────────────────

pub struct DevSharedState {
    /// Frame counter from `ctx.frame_nr()`. Increments every render cycle.
    /// The scenario runner's `wait_for_next_frame()` spins on this.
    pub frame_counter: u64,
    pub fps: f32,
    /// Widget tree as of the most recently completed frame.
    pub widget_tree: Vec<WidgetRecord>,
    /// Full serialised app state snapshot.
    pub app_state: serde_json::Value,
    /// Currently open dialog names.
    pub open_dialogs: Vec<String>,
    /// Active design-contract violations.
    pub active_violations: Vec<ContractViolation>,
    /// Ring buffer of SSE events (capped at 512).
    pub sse_ring: std::collections::VecDeque<SseEvent>,
    pub sse_seq: u64,
}

impl Default for DevSharedState {
    fn default() -> Self {
        DevSharedState {
            frame_counter: 0, fps: 0.0,
            widget_tree: Vec::new(),
            app_state: serde_json::Value::Null,
            open_dialogs: Vec::new(),
            active_violations: Vec::new(),
            sse_ring: std::collections::VecDeque::with_capacity(512),
            sse_seq: 0,
        }
    }
}

// ─── Dev command variants ─────────────────────────────────────────────────────

pub enum QueuedDevCmd {
    App(crate::chart_renderer::commands::AppCommand),
    Chart(crate::chart_renderer::ChartCommand),
}

// ─── Dev queues (written by HTTP thread, drained by app loop) ────────────────

pub struct DevQueues {
    /// Queued application commands, applied at `begin_frame()`.
    pub commands: Vec<QueuedDevCmd>,
    /// Queued input events, injected into egui at `begin_frame()`.
    pub inputs: Vec<input_queue::DevInput>,
    /// When true, `begin_frame()` resets the app to a clean baseline.
    pub reset_pending: bool,
}

impl Default for DevQueues {
    fn default() -> Self {
        DevQueues { commands: Vec::new(), inputs: Vec::new(), reset_pending: false }
    }
}

// ─── Globals ─────────────────────────────────────────────────────────────────

static SHARED_STATE: OnceLock<Arc<Mutex<DevSharedState>>> = OnceLock::new();
static DEV_QUEUES:   OnceLock<Arc<Mutex<DevQueues>>>      = OnceLock::new();

/// Headless mode: invisible window, render loop still runs. Set before open_window.
static HEADLESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn set_headless(val: bool) {
    HEADLESS.store(val, std::sync::atomic::Ordering::Relaxed);
}
pub fn is_headless() -> bool {
    HEADLESS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Returns the shared state Arc for use by the server thread.
pub fn shared_state() -> Arc<Mutex<DevSharedState>> {
    SHARED_STATE.get().expect("dev_inspector not initialized").clone()
}

/// Returns the queue Arc for use by the server thread.
pub fn dev_queues() -> Arc<Mutex<DevQueues>> {
    DEV_QUEUES.get().expect("dev_inspector not initialized").clone()
}

// ─── Thread-local widget and violation accumulators ───────────────────────────

thread_local! {
    static FRAME_WIDGETS:    RefCell<Vec<WidgetRecord>>       = RefCell::new(Vec::new());
    static FRAME_VIOLATIONS: RefCell<Vec<ContractViolation>>  = RefCell::new(Vec::new());
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Initialise the inspector. Call once from `main()` before opening any window.
/// Spawns the HTTP server thread.
pub fn init() {
    let shared = Arc::new(Mutex::new(DevSharedState::default()));
    let queues  = Arc::new(Mutex::new(DevQueues::default()));

    SHARED_STATE.set(shared.clone()).ok();
    DEV_QUEUES.set(queues.clone()).ok();

    server::start(shared, queues);
    eprintln!("[dev-inspector] HTTP server on http://127.0.0.1:7891");
}

/// Drain queued inputs for injection into `raw_input` BEFORE `ctx.run()`.
/// Called from gpu.rs just before the egui run call.
pub fn drain_inputs() -> Vec<input_queue::DevInput> {
    let queues = match DEV_QUEUES.get() {
        Some(q) => q,
        None => return Vec::new(),
    };
    let mut q = queues.lock().unwrap();
    q.inputs.drain(..).collect()
}

/// Called at the TOP of `draw_chart()`, before `route_commands()`.
/// Drains queued AppCommands and ChartCommands (input injection happens earlier in gpu.rs).
pub fn begin_frame() {
    // Clear per-frame accumulators (this is the main thread).
    FRAME_WIDGETS.with(|fw| fw.borrow_mut().clear());
    FRAME_VIOLATIONS.with(|fv| fv.borrow_mut().clear());

    let queues = match DEV_QUEUES.get() {
        Some(q) => q,
        None => return,
    };

    let (reset, cmds) = {
        let mut q = queues.lock().unwrap();
        let reset = q.reset_pending;
        q.reset_pending = false;
        let cmds: Vec<_> = q.commands.drain(..).collect();
        (reset, cmds)
    };

    // Drain queued commands into the normal dispatch paths.
    for cmd in cmds {
        match cmd {
            QueuedDevCmd::App(c) => crate::chart_renderer::commands::push(c),
            QueuedDevCmd::Chart(c) => crate::send_to_native_chart(c),
        }
    }

    if reset {
        do_reset();
    }
}

/// Called at the BOTTOM of `draw_chart()`, after `drain_and_dispatch()`.
/// Captures all app state into `DevSharedState` for the HTTP thread to read.
pub fn end_frame(
    panes: &[crate::chart_renderer::gpu::Chart],
    active_pane: usize,
    watchlist: &crate::chart_renderer::gpu::Watchlist,
    ctx: &egui::Context,
) {
    let widgets    = FRAME_WIDGETS.with(|fw| fw.borrow().clone());
    let violations = FRAME_VIOLATIONS.with(|fv| fv.borrow().clone());

    // Collect open dialogs from chart/watchlist state.
    let mut open_dialogs: Vec<String> = Vec::new();
    for (i, p) in panes.iter().enumerate() {
        if p.editing_indicator.is_some() {
            open_dialogs.push(format!("indicator_editor.{i}"));
        }
        if p.pane_picker_open {
            open_dialogs.push(format!("pane_picker.{i}"));
        }
    }
    if watchlist.settings_open      { open_dialogs.push("settings".into()); }
    if watchlist.hotkey_editor_open { open_dialogs.push("hotkey_editor".into()); }
    if watchlist.chain_select_mode  { open_dialogs.push("chain_select".into()); }
    if watchlist.order_entry_open   { open_dialogs.push("order_entry".into()); }
    if watchlist.orders_panel_open  { open_dialogs.push("orders_panel".into()); }

    let active_chart = panes.get(active_pane);
    let fps  = 1.0_f32 / ctx.input(|i| i.stable_dt).max(0.001);
    // frame counter is incremented in the final write below.

    // Build per-pane JSON.
    let panes_json: Vec<serde_json::Value> = panes.iter().enumerate()
        .map(|(i, p)| serde_json::json!({
            "index":           i,
            "symbol":          p.symbol,
            "timeframe":       p.timeframe,
            "bar_count":       p.bars.len(),
            "pane_type":       format!("{:?}", p.pane_type),
            "indicator_count": p.indicators.len(),
            "drawing_count":   p.drawings.len(),
            "order_count":     p.orders.len(),
            "alert_count":     p.price_alerts.len(),
        }))
        .collect();

    // Build watchlist summary.
    let wl_sections: Vec<serde_json::Value> = watchlist.sections.iter()
        .map(|sec| serde_json::json!({
            "title":      sec.title,
            "item_count": sec.items.len(),
            "collapsed":  sec.collapsed,
        }))
        .collect();

    let app_state = serde_json::json!({
        "fps":             fps,
        "frame_counter":   0u64, // filled in during write below
        "pane_count":      panes.len(),
        "active_pane":     active_pane,
        "active_symbol":   active_chart.map(|c| c.symbol.as_str()).unwrap_or(""),
        "active_timeframe":active_chart.map(|c| c.timeframe.as_str()).unwrap_or(""),
        "bar_count":       active_chart.map(|c| c.bars.len()).unwrap_or(0),
        "open_dialogs":    open_dialogs,
        "panes":           panes_json,
        "watchlist": {
            "section_count":  watchlist.sections.len(),
            "sections":       wl_sections,
            "active_idx":     watchlist.active_watchlist_idx,
            "name":           watchlist.saved_watchlists.get(watchlist.active_watchlist_idx)
                                  .map(|wl| wl.name.as_str()).unwrap_or(""),
        },
        "total_order_count": panes.iter().map(|p| p.orders.len()).sum::<usize>(),
        "total_alert_count": panes.iter().map(|p| p.price_alerts.len()).sum::<usize>(),
    });

    // State-derived widget records (supplement egui-response-backed ones).
    let mut all_widgets = widgets;
    if let Some(ac) = active_chart {
        all_widgets.push(WidgetRecord {
            id: format!("pane.{active_pane}.symbol"),
            role: "label".into(), label: ac.symbol.clone(),
            value: Some(ac.symbol.clone()),
            ..WidgetRecord::state("", "", "")
        });
        all_widgets.push(WidgetRecord {
            id: format!("pane.{active_pane}.timeframe"),
            role: "label".into(), label: ac.timeframe.clone(),
            value: Some(ac.timeframe.clone()),
            ..WidgetRecord::state("", "", "")
        });
    }
    for (i, p) in panes.iter().enumerate() {
        all_widgets.push(WidgetRecord {
            id: format!("pane.{i}.header"),
            role: "header".into(),
            label: format!("{} {}", p.symbol, p.timeframe),
            ..WidgetRecord::state("", "", "")
        });
    }
    // Connection status widget
    all_widgets.push(WidgetRecord {
        id: "status_bar.connection".into(),
        role: "status".into(),
        label: connection_label(),
        ..WidgetRecord::state("", "", "")
    });

    let shared = match SHARED_STATE.get() {
        Some(s) => s,
        None => return,
    };
    let mut guard = shared.lock().unwrap();
    guard.frame_counter    += 1;
    let frame               = guard.frame_counter;
    // Patch frame_counter into the snapshot.
    if let Some(obj) = guard.app_state.as_object_mut() {
        // will be replaced below, but we patch here for completeness
        drop(obj);
    }
    guard.fps               = fps;
    guard.widget_tree       = all_widgets;
    // Patch the frame counter into app_state before writing.
    let mut patched_state = app_state;
    if let Some(obj) = patched_state.as_object_mut() {
        obj.insert("frame_counter".into(), serde_json::Value::Number(frame.into()));
    }
    guard.app_state         = patched_state;
    guard.open_dialogs      = open_dialogs;
    guard.active_violations = violations;
}

/// Register a widget record for the current frame.
/// Call during rendering of any semantically meaningful egui element.
#[inline]
pub fn record(w: WidgetRecord) {
    FRAME_WIDGETS.with(|fw| fw.borrow_mut().push(w));
}

/// Check a design contract for a widget. Violations accumulate in `DevSharedState`
/// and are emitted as SSE events. Never panics, never affects visible behaviour.
pub fn check_contract(widget_id: &str, rect: egui::Rect, contract: layout::Contract) {
    let violations = contract.check(widget_id, rect);
    let frame = SHARED_STATE.get()
        .and_then(|s| s.lock().ok().map(|g| g.frame_counter))
        .unwrap_or(0);
    for v in violations {
        let v = ContractViolation {
            widget_id: widget_id.into(),
            constraint: v.0,
            detail: v.1,
            frame,
        };
        FRAME_VIOLATIONS.with(|fv| fv.borrow_mut().push(v.clone()));
        emit("contract_violation", serde_json::to_value(&v).unwrap_or_default());
    }
}

/// Emit a named SSE event. Visible via `GET /events`.
pub fn emit(name: &str, data: serde_json::Value) {
    let Some(shared) = SHARED_STATE.get() else { return };
    if let Ok(mut g) = shared.lock() {
        let seq = g.sse_seq;
        g.sse_seq += 1;
        g.sse_ring.push_back(SseEvent { name: name.into(), data, seq });
        if g.sse_ring.len() > 512 {
            g.sse_ring.pop_front();
        }
    }
}

// ─── Reset ────────────────────────────────────────────────────────────────────

fn do_reset() {
    use crate::chart_renderer::commands::{push, AppCommand, ChartFlag};
    push(AppCommand::CloseAllDialogs);
    push(AppCommand::CancelAllOrders);
    push(AppCommand::SwapPaneSymbol { pane: 0, symbol: "SPY".into() });
    push(AppCommand::ChangeTimeframe { pane: 0, tf: "5m".into() });
    push(AppCommand::SetChartFlag { pane: 0, flag: ChartFlag::ShowVolume, value: true });
    push(AppCommand::SetChartFlag { pane: 0, flag: ChartFlag::LogScale,   value: false });
    // Kick off a fresh bar fetch for the reset state.
    crate::chart_renderer::gpu::fetch_bars_background_pub("SPY".into(), "5m".into());
}

// ─── Connection label (best-effort, non-blocking) ─────────────────────────────

fn connection_label() -> String {
    // Check the recent error sink: if there are recent failures the feeds are unhealthy.
    let recent = crate::data::connectivity::errors_sink::drain_recent();
    if recent.is_empty() {
        "ok".into()
    } else {
        format!("errors:{}", recent.len())
    }
}
