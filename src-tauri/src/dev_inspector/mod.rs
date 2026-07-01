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
pub mod canvas;
pub mod layout;
pub mod server;
pub mod annotations;

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
    pub role: String,             // "button", "label", "input", "status", "header", etc.
    pub label: String,
    pub value: Option<String>,
    pub rect: SerRect,
    pub clip_rect: SerRect,
    pub layer: u8,
    pub focused: bool,
    pub hovered: bool,
    pub enabled: bool,
    pub is_clipped: bool,
    pub style_class: Option<String>, // "primary", "ghost", "toolbar", "header", etc.
}

impl WidgetRecord {
    pub fn state(id: impl Into<String>, role: &str, label: impl Into<String>) -> Self {
        WidgetRecord {
            id: id.into(), role: role.into(), label: label.into(),
            value: None, rect: SerRect::zero(), clip_rect: SerRect::zero(),
            layer: 0, focused: false, hovered: false, enabled: true, is_clipped: false,
            style_class: None,
        }
    }
    pub fn from_response(
        id: impl Into<String>, role: &str, label: impl Into<String>,
        resp: &egui::Response, ui: &egui::Ui,
    ) -> Self {
        let rect: SerRect = resp.rect.into();
        let clip_rect: SerRect = ui.clip_rect().into();
        // True geometric clipping: the widget's rect is not fully contained within
        // its clip rect (egui will visually cut it off). Previously this checked
        // min-side < 28px, which mislabels every small icon button as "clipped" —
        // a touch-target concern, not clipping (see `all_touch_targets_ok`).
        const TOL: f32 = 0.5;
        let is_clipped = rect.w > 0.0 && rect.h > 0.0 && (
            rect.x          < clip_rect.x - TOL ||
            rect.y          < clip_rect.y - TOL ||
            rect.x + rect.w > clip_rect.x + clip_rect.w + TOL ||
            rect.y + rect.h > clip_rect.y + clip_rect.h + TOL
        );
        WidgetRecord {
            id: id.into(), role: role.into(), label: label.into(),
            value: None,
            is_clipped,
            rect, clip_rect,
            layer: 0,
            focused: resp.has_focus(),
            hovered: resp.hovered(),
            enabled: ui.is_enabled(),
            style_class: None,
        }
    }
    pub fn with_style(mut self, class: impl Into<String>) -> Self {
        self.style_class = Some(class.into()); self
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
    /// Frame counter. Increments every render cycle; `wait_for_next_frame()` spins on this.
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
    /// Active debug annotation overlays (set via POST /annotations, rendered each frame).
    pub active_annotations: Vec<annotations::DebugAnnotation>,
    /// Rolling FPS history (last 300 frames).
    pub fps_history: std::collections::VecDeque<f32>,
    /// Rolling violation-count history (last 300 frames).
    pub violation_history: std::collections::VecDeque<usize>,
    /// Last frame time in milliseconds (1000 / fps, from unstable_dt).
    pub frame_time_ms: f32,
    /// Last completed scenario result (JSON), queryable via GET /last-run.
    pub last_run: Option<serde_json::Value>,
    /// Named captures set by the `capture` step action, queryable via GET /captures.
    pub captures: std::collections::HashMap<String, serde_json::Value>,
    /// Per-frame canvas snapshot: drawings, indicators, visible bars with world+screen coords.
    pub canvas: canvas::CanvasSnapshot,
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
            active_annotations: Vec::new(),
            fps_history: std::collections::VecDeque::with_capacity(300),
            violation_history: std::collections::VecDeque::with_capacity(300),
            frame_time_ms: 0.0,
            last_run: None,
            captures: std::collections::HashMap::new(),
            canvas: canvas::CanvasSnapshot::default(),
        }
    }
}

// ─── Dev command variants ─────────────────────────────────────────────────────

pub enum QueuedDevCmd {
    App(crate::chart_renderer::commands::AppCommand),
    Chart(crate::chart_renderer::ChartCommand),
    /// Headless-only: resize the synthetic pane array.
    HeadlessLayout { cols: usize },
    /// Headless-only: override a pane's display type string without going through PaneType enum.
    HeadlessPaneType { pane: usize, name: String },
    /// Headless-only: open or close a named dialog directly (no AppCommand equivalent).
    HeadlessDialog { name: String, open: bool },
    // ── Canvas / drawing commands (headless simulation) ───────────────────────
    /// Add or replace a synthetic drawing on a pane.
    HeadlessAddDrawing {
        pane: usize, id: String, kind: String,
        price_a: f64, price_b: Option<f64>,
    },
    /// Remove a synthetic drawing by ID.
    HeadlessRemoveDrawing { pane: usize, id: String },
    /// Remove all synthetic drawings from a pane.
    HeadlessClearDrawings { pane: usize },
    /// Override the visible price range for a pane.
    HeadlessSetViewport { pane: usize, price_low: f64, price_high: f64 },
    /// Set the last-computed output value for a named indicator on a pane.
    HeadlessSetIndicatorOutput { pane: usize, kind: String, value: f64, value2: Option<f64> },
    /// Remove a synthetic indicator by kind name.
    HeadlessRemoveIndicator { pane: usize, kind: String },
}

// ─── Dev queues (written by HTTP thread, drained by app loop) ────────────────

pub struct DevQueues {
    /// Queued application commands, applied at `begin_frame()`.
    pub commands: Vec<QueuedDevCmd>,
    /// Queued input events, injected into egui at `begin_frame()`.
    pub inputs: Vec<input_queue::DevInput>,
    /// When true, `begin_frame()` resets the app to a clean baseline.
    pub reset_pending: bool,
    /// Pending annotation mutations. Drained into DevSharedState.active_annotations each frame.
    pub annotation_ops: Vec<annotations::AnnotationOp>,
}

impl Default for DevQueues {
    fn default() -> Self {
        DevQueues {
            commands: Vec::new(),
            inputs: Vec::new(),
            reset_pending: false,
            annotation_ops: Vec::new(),
        }
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
/// Spawns the HTTP server thread and (in headless mode) the frame ticker.
pub fn init() {
    let shared = Arc::new(Mutex::new(DevSharedState::default()));
    let queues  = Arc::new(Mutex::new(DevQueues::default()));

    SHARED_STATE.set(shared.clone()).ok();
    DEV_QUEUES.set(queues.clone()).ok();

    install_panic_hook();
    server::start(shared.clone(), queues.clone());
    eprintln!("[dev-inspector] HTTP server on http://127.0.0.1:7892");

    if is_headless() {
        start_headless_ticker(shared, queues);
    }
}

// ─── Panic capture ────────────────────────────────────────────────────────────
// A panic on the render thread (interactive) closes the window; a panic on the
// headless ticker stalls frames. Either way the scenario harness needs to KNOW —
// a crash is the highest-severity bug a story can surface. We chain a panic hook
// that records every panic into a global log (it survives the panicking thread's
// unwind, and the HTTP server thread stays alive to report it). The `no_panic`
// assertion + the suite bug-report read this. Debug-only (this whole module is
// `#[cfg(debug_assertions)]`).

/// One captured panic.
#[derive(Clone, Debug, serde::Serialize)]
pub struct PanicReport {
    pub message: String,
    pub location: String,
    pub thread: String,
}

static PANIC_LOG: OnceLock<Mutex<Vec<PanicReport>>> = OnceLock::new();
fn panic_log() -> &'static Mutex<Vec<PanicReport>> {
    PANIC_LOG.get_or_init(|| Mutex::new(Vec::new()))
}

/// Snapshot all captured panics (newest last).
pub fn panics() -> Vec<PanicReport> {
    panic_log().lock().map(|g| g.clone()).unwrap_or_default()
}

/// Number of panics captured since startup.
pub fn panic_count() -> usize {
    panic_log().lock().map(|g| g.len()).unwrap_or(0)
}

/// Clear the panic log (called on `reset` so each scenario starts clean).
pub fn clear_panics() {
    if let Ok(mut g) = panic_log().lock() { g.clear(); }
}

// ─── Screenshot requests (fulfilled on the render thread, which owns the HWND) ──
// The HTTP/scenario thread can't touch the window; it queues a filename here and
// gpu.rs drains it each frame, capturing the live window to dev/screenshots/<name>.png.
static SCREENSHOT_REQS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
fn screenshot_reqs() -> &'static Mutex<Vec<String>> {
    SCREENSHOT_REQS.get_or_init(|| Mutex::new(Vec::new()))
}
/// Queue a full-window screenshot to `dev/screenshots/<name>.png`.
pub fn request_screenshot(name: impl Into<String>) {
    if let Ok(mut g) = screenshot_reqs().lock() { g.push(name.into()); }
}
/// Drain pending screenshot filenames (called by the render thread).
pub fn take_screenshot_reqs() -> Vec<String> {
    screenshot_reqs().lock().map(|mut g| std::mem::take(&mut *g)).unwrap_or_default()
}

fn install_panic_hook() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    if INSTALLED.set(()).is_err() { return; }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = match info.payload().downcast_ref::<&str>() {
            Some(s) => s.to_string(),
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => s.clone(),
                None => "<non-string panic payload>".to_string(),
            },
        };
        let location = info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let thread = std::thread::current().name().unwrap_or("<unnamed>").to_string();
        if let Ok(mut g) = panic_log().lock() {
            g.push(PanicReport { message, location, thread });
        }
        prev(info); // keep the default hook (backtrace to stderr)
    }));
}

// ─── Headless simulation ──────────────────────────────────────────────────────
// When the app is spawned via CreateProcessW (e.g. by cargo test), Windows DWM
// may not deliver WM_PAINT events, so the real GPU render loop never fires.
// The headless ticker runs at 60 fps on a background thread and:
//   1. Increments `frame_counter` so `wait_for_next_frame()` succeeds.
//   2. Processes queued AppCommands against a lightweight synthetic state machine.
//   3. Writes synthetic `app_state` JSON so scenario assertions pass.
// Only active when `is_headless()` is true — zero impact on interactive builds.

struct DrawingSim {
    id:       String,
    kind:     String,
    /// (bar_offset_from_right, price). offset=0 → current rightmost bar.
    anchor_a: (f64, f64),
    anchor_b: Option<(f64, f64)>,
    visible:  bool,
    selected: bool,
}

struct IndicatorOutputSim {
    id:          u32,
    kind:        String,
    period:      usize,
    visible:     bool,
    last_value:  Option<f64>,
    last_value2: Option<f64>,
}

struct ViewportSim {
    price_low:     f64,
    price_high:    f64,
    bars_visible:  u32,
}

impl Default for ViewportSim {
    fn default() -> Self {
        ViewportSim { price_low: 430.0, price_high: 470.0, bars_visible: 100 }
    }
}

struct PaneSim {
    symbol:          String,
    timeframe:       String,
    indicator_count: usize,
    pane_type:       String,
    // Canvas state
    drawings:        Vec<DrawingSim>,
    viewport:        ViewportSim,
    indicators:      Vec<IndicatorOutputSim>,
    bar_count:       usize,
}

struct SectionSim {
    title:      String,
    item_count: usize,
    collapsed:  bool,
}

struct HeadlessState {
    panes:                 Vec<PaneSim>,
    active_pane:           usize,
    open_dialogs:          Vec<String>,
    watchlist_sections:    Vec<SectionSim>,
    watchlist_active_idx:  usize,
    watchlist_count:       usize,
    next_indicator_id:     u32,
}

impl PaneSim {
    fn new_default(symbol: &str) -> Self {
        PaneSim {
            symbol:          symbol.into(),
            timeframe:       "5m".into(),
            indicator_count: 0,
            pane_type:       "Chart".into(),
            drawings:        Vec::new(),
            viewport:        ViewportSim::default(),
            indicators:      Vec::new(),
            bar_count:       100,
        }
    }
}

impl Default for HeadlessState {
    fn default() -> Self {
        HeadlessState {
            panes: vec![PaneSim::new_default("SPY")],
            active_pane:          0,
            open_dialogs:         Vec::new(),
            watchlist_sections:   vec![SectionSim { title: "Stocks".into(), item_count: 0, collapsed: false }],
            watchlist_active_idx: 0,
            watchlist_count:      1,
            next_indicator_id:    1,
        }
    }
}

fn apply_headless_cmd(
    hs: &mut HeadlessState,
    cmd: crate::chart_renderer::commands::AppCommand,
) {
    use crate::chart_renderer::commands::AppCommand;
    match cmd {
        AppCommand::SwapPaneSymbol { pane, symbol } => {
            if let Some(p) = hs.panes.get_mut(pane) { p.symbol = symbol; }
        }
        AppCommand::ChangeTimeframe { pane, tf } => {
            if let Some(p) = hs.panes.get_mut(pane) { p.timeframe = tf; }
        }
        AppCommand::AddIndicator { pane, .. } => {
            if let Some(p) = hs.panes.get_mut(pane) { p.indicator_count += 1; }
        }
        AppCommand::RemoveIndicator { pane, .. } => {
            if let Some(p) = hs.panes.get_mut(pane) {
                p.indicator_count = p.indicator_count.saturating_sub(1);
            }
        }
        AppCommand::ChangePaneType { pane, kind } => {
            if let Some(p) = hs.panes.get_mut(pane) {
                p.pane_type = format!("{kind:?}");
            }
        }
        AppCommand::WatchlistAddSection { title } |
        AppCommand::WatchlistAddOptionSection { title } => {
            hs.watchlist_sections.push(SectionSim { title, item_count: 0, collapsed: false });
        }
        AppCommand::WatchlistRemoveSection { idx } => {
            if idx < hs.watchlist_sections.len() {
                hs.watchlist_sections.remove(idx);
            }
        }
        AppCommand::WatchlistAddSymbol { .. } => {
            if let Some(s) = hs.watchlist_sections.last_mut() {
                s.item_count += 1;
            }
        }
        AppCommand::WatchlistRemoveSymbol { symbol } => {
            for sec in &mut hs.watchlist_sections {
                // Best-effort: decrement count if any symbol matches (no per-item tracking).
                let _ = symbol.as_str();  // suppress unused warning
                if sec.item_count > 0 { sec.item_count -= 1; break; }
            }
        }
        AppCommand::WatchlistToggleSectionCollapse { idx } => {
            if let Some(s) = hs.watchlist_sections.get_mut(idx) {
                s.collapsed = !s.collapsed;
            }
        }
        AppCommand::OpenIndicatorEditor { pane, .. } => {
            let key = format!("indicator_editor.{pane}");
            if !hs.open_dialogs.contains(&key) { hs.open_dialogs.push(key); }
        }
        AppCommand::CloseIndicatorEditor { pane } => {
            let key = format!("indicator_editor.{pane}");
            hs.open_dialogs.retain(|d| d != &key);
        }
        AppCommand::CloseAllDialogs => {
            hs.open_dialogs.clear();
        }
        AppCommand::WatchlistSwitchActive { idx } => {
            let bound = hs.watchlist_sections.len().max(hs.watchlist_count);
            if bound > 0 {
                hs.watchlist_active_idx = idx.min(bound - 1);
            }
        }
        AppCommand::WatchlistCreate { .. } => {
            hs.watchlist_count += 1;
        }
        AppCommand::WatchlistDelete { idx } => {
            if hs.watchlist_count > 1 {
                hs.watchlist_count -= 1;
                if hs.watchlist_active_idx >= hs.watchlist_count {
                    hs.watchlist_active_idx = hs.watchlist_count - 1;
                }
                // If we deleted the active watchlist (or one before it), step back.
                if idx <= hs.watchlist_active_idx && hs.watchlist_active_idx > 0 {
                    hs.watchlist_active_idx -= 1;
                }
            }
        }
        // Commands with no observable effect on the synthetic state:
        AppCommand::SetChartFlag { .. }
        | AppCommand::SetThemeIdx { .. }
        | AppCommand::SetStyleIdx { .. }
        | AppCommand::RecomputeIndicators { .. }
        | AppCommand::CancelAllOrders
        | AppCommand::ClearOrderHistory
        | AppCommand::PlaceAllDraftOrders
        | AppCommand::PlaceAllDraftAlerts
        | AppCommand::WatchlistRenameActive { .. }
        | AppCommand::WatchlistDuplicate { .. }
        | _ => {}
    }
}

fn headless_to_json(hs: &HeadlessState) -> serde_json::Value {
    let panes_json: Vec<serde_json::Value> = hs.panes.iter().enumerate()
        .map(|(i, p)| serde_json::json!({
            "index":           i,
            "symbol":          p.symbol,
            "timeframe":       p.timeframe,
            "bar_count":       100,
            "pane_type":       p.pane_type,
            "indicator_count": p.indicator_count,
            "drawing_count":   0,
            "order_count":     0,
            "alert_count":     0,
        }))
        .collect();

    let wl_sections: Vec<serde_json::Value> = hs.watchlist_sections.iter()
        .map(|s| serde_json::json!({
            "title":      s.title,
            "item_count": s.item_count,
            "collapsed":  s.collapsed,
        }))
        .collect();

    let active = hs.panes.get(hs.active_pane);

    serde_json::json!({
        "fps":              60.0,
        "frame_counter":    0,   // patched in by the ticker after locking shared state
        "pane_count":       hs.panes.len(),
        "active_pane":      hs.active_pane,
        "active_symbol":    active.map(|p| p.symbol.as_str()).unwrap_or(""),
        "active_timeframe": active.map(|p| p.timeframe.as_str()).unwrap_or(""),
        "bar_count":        100,
        "open_dialogs":     hs.open_dialogs,
        "panes":            panes_json,
        "watchlist": {
            "section_count":  hs.watchlist_sections.len(),
            "sections":       wl_sections,
            "active_idx":     hs.watchlist_active_idx,
            "watchlist_count": hs.watchlist_count,
            "name":           "Default",
        },
        "total_order_count": 0,
        "total_alert_count": 0,
    })
}

fn start_headless_ticker(
    shared: Arc<Mutex<DevSharedState>>,
    queues: Arc<Mutex<DevQueues>>,
) {
    std::thread::Builder::new()
        .name("dev-inspector-headless-ticker".into())
        .spawn(move || {
            let mut hs = HeadlessState::default();
            const TICK_MS: u64 = 16; // ~60 fps

            loop {
                std::thread::sleep(std::time::Duration::from_millis(TICK_MS));

                // Drain all pending mutations from DevQueues.
                let (reset, cmds, inputs, ann_ops) = {
                    let mut q = queues.lock().unwrap();
                    let reset = q.reset_pending;
                    q.reset_pending = false;
                    let cmds:    Vec<_> = q.commands.drain(..).collect();
                    let inputs:  Vec<_> = q.inputs.drain(..).collect();
                    let ann_ops: Vec<_> = q.annotation_ops.drain(..).collect();
                    (reset, cmds, inputs, ann_ops)
                };

                if reset {
                    hs = HeadlessState::default();
                }

                for cmd in cmds {
                    match cmd {
                        QueuedDevCmd::App(app_cmd) => apply_headless_cmd(&mut hs, app_cmd),
                        QueuedDevCmd::HeadlessLayout { cols } => {
                            let target = cols.max(1);
                            while hs.panes.len() < target {
                                hs.panes.push(PaneSim::new_default("SPY"));
                            }
                            if hs.panes.len() > target {
                                hs.panes.truncate(target);
                                if hs.active_pane >= target { hs.active_pane = 0; }
                            }
                        }
                        QueuedDevCmd::HeadlessPaneType { pane, name } => {
                            if let Some(p) = hs.panes.get_mut(pane) { p.pane_type = name; }
                        }
                        QueuedDevCmd::HeadlessDialog { name, open } => {
                            if open {
                                if !hs.open_dialogs.contains(&name) { hs.open_dialogs.push(name); }
                            } else {
                                hs.open_dialogs.retain(|d| d != &name);
                            }
                        }
                        // ── Canvas commands ───────────────────────────────────
                        QueuedDevCmd::HeadlessAddDrawing { pane, id, kind, price_a, price_b } => {
                            if let Some(p) = hs.panes.get_mut(pane) {
                                p.drawings.retain(|d| d.id != id); // replace if exists
                                p.drawings.push(DrawingSim {
                                    id, kind,
                                    anchor_a: (0.0, price_a),
                                    anchor_b: price_b.map(|pr| (10.0, pr)),
                                    visible: true, selected: false,
                                });
                            }
                        }
                        QueuedDevCmd::HeadlessRemoveDrawing { pane, id } => {
                            if let Some(p) = hs.panes.get_mut(pane) {
                                p.drawings.retain(|d| d.id != id);
                            }
                        }
                        QueuedDevCmd::HeadlessClearDrawings { pane } => {
                            if let Some(p) = hs.panes.get_mut(pane) { p.drawings.clear(); }
                        }
                        QueuedDevCmd::HeadlessSetViewport { pane, price_low, price_high } => {
                            if let Some(p) = hs.panes.get_mut(pane) {
                                p.viewport.price_low  = price_low;
                                p.viewport.price_high = price_high;
                            }
                        }
                        QueuedDevCmd::HeadlessSetIndicatorOutput { pane, kind, value, value2 } => {
                            if let Some(p) = hs.panes.get_mut(pane) {
                                if let Some(ind) = p.indicators.iter_mut().find(|i| i.kind == kind) {
                                    ind.last_value  = Some(value);
                                    ind.last_value2 = value2;
                                } else {
                                    let id = hs.next_indicator_id;
                                    hs.next_indicator_id += 1;
                                    p.indicator_count += 1;
                                    p.indicators.push(IndicatorOutputSim {
                                        id, kind, period: 14,
                                        visible: true, last_value: Some(value), last_value2: value2,
                                    });
                                }
                            }
                        }
                        QueuedDevCmd::HeadlessRemoveIndicator { pane, kind } => {
                            if let Some(p) = hs.panes.get_mut(pane) {
                                let before = p.indicators.len();
                                p.indicators.retain(|i| i.kind != kind);
                                let removed = before - p.indicators.len();
                                p.indicator_count = p.indicator_count.saturating_sub(removed);
                            }
                        }
                        QueuedDevCmd::Chart(_) => {} // chart data irrelevant in headless
                    }
                }

                // Process injected key events: Escape closes all dialogs; others no-op in headless.
                for input in inputs {
                    if let input_queue::DevInput::Key { key } = &input {
                        let k = key.to_lowercase();
                        if k == "escape" || k == "esc" {
                            hs.open_dialogs.clear();
                        }
                    }
                }

                // Apply annotation ops to shared state (these use real shared state).
                if !ann_ops.is_empty() {
                    if let Ok(mut g) = shared.lock() {
                        for op in ann_ops {
                            annotations::apply_op(&mut g.active_annotations, op);
                        }
                    }
                }

                // Build canvas snapshot from headless state before locking shared.
                let pane_drawings: Vec<Vec<canvas::HeadlessDrawing>> = hs.panes.iter().map(|p| {
                    p.drawings.iter().map(|d| canvas::HeadlessDrawing {
                        id: d.id.clone(), kind: d.kind.clone(),
                        anchor_a: d.anchor_a, anchor_b: d.anchor_b,
                        visible: d.visible, selected: d.selected,
                    }).collect()
                }).collect();
                let pane_indicators: Vec<Vec<canvas::HeadlessIndicator>> = hs.panes.iter().map(|p| {
                    p.indicators.iter().map(|ind| canvas::HeadlessIndicator {
                        id: ind.id, kind: ind.kind.clone(), period: ind.period,
                        visible: ind.visible, last_value: ind.last_value, last_value2: ind.last_value2,
                    }).collect()
                }).collect();
                let canvas_hp: Vec<canvas::HeadlessPane<'_>> = hs.panes.iter().enumerate().map(|(i, p)| {
                    canvas::HeadlessPane {
                        index: i,
                        symbol: &p.symbol, timeframe: &p.timeframe, pane_type: &p.pane_type,
                        visible_bar_count: p.viewport.bars_visible,
                        price_low: p.viewport.price_low, price_high: p.viewport.price_high,
                        drawings: &pane_drawings[i], indicators: &pane_indicators[i],
                        bar_count: p.bar_count,
                    }
                }).collect();
                let canvas_input = canvas::HeadlessCanvasInput {
                    panes: &canvas_hp, active_pane: hs.active_pane,
                };
                let mut canvas_snap = canvas::from_headless(&canvas_input);

                // Write synthetic app state and advance frame counter.
                let app_state = headless_to_json(&hs);

                if let Ok(mut g) = shared.lock() {
                    g.frame_counter += 1;
                    let frame = g.frame_counter;
                    g.fps           = 60.0;
                    g.frame_time_ms = TICK_MS as f32;
                    g.open_dialogs  = hs.open_dialogs.clone();
                    g.active_violations.clear();

                    g.fps_history.push_back(60.0);
                    if g.fps_history.len() > 300 { g.fps_history.pop_front(); }
                    g.violation_history.push_back(0);
                    if g.violation_history.len() > 300 { g.violation_history.pop_front(); }

                    let mut patched = app_state;
                    if let Some(obj) = patched.as_object_mut() {
                        obj.insert("frame_counter".into(), frame.into());
                    }
                    g.app_state = patched;
                    canvas_snap.captured_at_frame = frame;
                    g.canvas = canvas_snap;
                }
            }
        })
        .expect("failed to spawn headless ticker thread");
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

    // In headless mode the headless ticker owns DevQueues exclusively.
    // begin_frame() must not drain commands or trigger do_reset() — the ticker
    // processes both. The frame accumulators above are cleared so end_frame()
    // writes an empty widget tree (also a no-op since end_frame() returns early too).
    if is_headless() { return; }

    let queues = match DEV_QUEUES.get() {
        Some(q) => q,
        None => return,
    };

    let (reset, cmds, ann_ops) = {
        let mut q = queues.lock().unwrap();
        let reset = q.reset_pending;
        q.reset_pending = false;
        let cmds: Vec<_> = q.commands.drain(..).collect();
        let ann_ops: Vec<_> = q.annotation_ops.drain(..).collect();
        (reset, cmds, ann_ops)
    };

    // Drain queued commands into the normal dispatch paths.
    for cmd in cmds {
        match cmd {
            QueuedDevCmd::App(c)   => crate::chart_renderer::commands::push(c),
            QueuedDevCmd::Chart(c) => crate::send_to_native_chart(c),
            // All Headless* variants are owned by the headless ticker — ignore in live mode.
            QueuedDevCmd::HeadlessLayout { .. }
            | QueuedDevCmd::HeadlessPaneType { .. }
            | QueuedDevCmd::HeadlessDialog { .. }
            | QueuedDevCmd::HeadlessAddDrawing { .. }
            | QueuedDevCmd::HeadlessRemoveDrawing { .. }
            | QueuedDevCmd::HeadlessClearDrawings { .. }
            | QueuedDevCmd::HeadlessSetViewport { .. }
            | QueuedDevCmd::HeadlessSetIndicatorOutput { .. }
            | QueuedDevCmd::HeadlessRemoveIndicator { .. } => {}
        }
    }

    // Apply annotation mutations to shared state.
    if !ann_ops.is_empty() {
        if let Some(shared) = SHARED_STATE.get() {
            if let Ok(mut g) = shared.lock() {
                for op in ann_ops {
                    annotations::apply_op(&mut g.active_annotations, op);
                }
            }
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
    // In headless mode the headless ticker writes all DevSharedState fields.
    // end_frame() must not overwrite them — the ticker's synthetic state is
    // authoritative during scenario execution.
    if is_headless() { return; }

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
    let raw_dt       = ctx.input(|i| i.unstable_dt);
    let fps          = 1.0_f32 / raw_dt.max(0.001);
    let frame_time_ms = raw_dt * 1000.0;
    // frame counter is incremented in the final write below.

    // Build per-pane JSON.
    use crate::chart_renderer::trading::OrderStatus;
    let now_epoch_ms: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let panes_json: Vec<serde_json::Value> = panes.iter().enumerate()
        .map(|(i, p)| {
            // Order status breakdown (visual System-A list — never the broker).
            let (mut n_draft, mut n_placed, mut n_exec, mut n_cancel) = (0u32, 0u32, 0u32, 0u32);
            for o in &p.orders {
                match o.status {
                    OrderStatus::Draft     => n_draft  += 1,
                    OrderStatus::Placed    => n_placed += 1,
                    OrderStatus::Executed  => n_exec   += 1,
                    OrderStatus::Cancelled => n_cancel += 1,
                }
            }
            // DOM ladder is sorted high→low. Best bid/ask are defined relative
            // to the mid (center) price, not by side flags — the mock ladder
            // carries bid+ask size on every level, so a size-only rule is
            // meaningless. best_ask = lowest price >= mid; best_bid = highest
            // price <= mid ⇒ ask >= bid always holds for a well-formed ladder.
            let dom_mid = if p.dom.center_price > 0.0 {
                p.dom.center_price
            } else {
                p.dom.levels.get(p.dom.levels.len() / 2).map(|l| l.price).unwrap_or(0.0)
            };
            let dom_best_ask = p.dom.levels.iter().filter(|l| l.price >= dom_mid)
                .map(|l| l.price).fold(f32::NAN, f32::min);
            let dom_best_bid = p.dom.levels.iter().filter(|l| l.price <= dom_mid)
                .map(|l| l.price).fold(f32::NAN, f32::max);
            let dom_best_ask = if dom_best_ask.is_finite() { Some(dom_best_ask) } else { None };
            let dom_best_bid = if dom_best_bid.is_finite() { Some(dom_best_bid) } else { None };
            // Real correctness invariant: prices strictly descend down the ladder.
            let dom_prices_desc = p.dom.levels.windows(2).all(|w| w[0].price > w[1].price);
            let dom_is_live  = p.dom.last_live_ms != 0
                && (now_epoch_ms.saturating_sub(p.dom.last_live_ms)) < 2000;
            serde_json::json!({
                "index":           i,
                "symbol":          p.symbol,
                "timeframe":       p.timeframe,
                "bar_count":       p.bars.len(),
                "pane_type":       format!("{:?}", p.pane_type),
                "indicator_count": p.indicators.len(),
                "drawing_count":   p.drawings.len(),
                "order_count":     p.orders.len(),
                "alert_count":     p.price_alerts.len(),
                // Order-entry (visual) breakdown + panel state.
                "order_draft_count":     n_draft,
                "order_placed_count":    n_placed,
                "order_executed_count":  n_exec,
                "order_cancelled_count": n_cancel,
                "order_panel_collapsed": p.order_panel.collapsed,
                // DOM ladder.
                "dom_sidebar_open": p.dom.sidebar_open,
                "dom_open":         p.dom.open,
                "dom_level_count":  p.dom.levels.len(),
                "dom_best_bid":     dom_best_bid,
                "dom_best_ask":     dom_best_ask,
                "dom_prices_desc":  dom_prices_desc,
                "dom_is_live":      dom_is_live,
                // Gamma / strikes overlays.
                "show_gamma":         p.show_gamma,
                "gamma_level_count":  p.gamma_levels.len(),
                "gamma_call_wall":    p.gamma_call_wall,
                "gamma_put_wall":     p.gamma_put_wall,
                "show_strikes_overlay": p.show_strikes_overlay,
                "strikes_call_count":   p.overlay_calls.len(),
                "strikes_put_count":    p.overlay_puts.len(),
            })
        })
        .collect();

    // Build watchlist summary.
    let wl_sections: Vec<serde_json::Value> = watchlist.sections.iter()
        .map(|sec| serde_json::json!({
            "title":      sec.title,
            "item_count": sec.items.len(),
            "collapsed":  sec.collapsed,
            // Expose each row so the harness can verify the % column is populated
            // and sane (the original "watchlist % completely wrong" bug). change_perc
            // is the server-computed value; null until the row loads.
            "items": sec.items.iter().map(|it| serde_json::json!({
                "symbol":      it.symbol,
                "last":        it.price,
                "prev_close":  it.prev_close,
                "change_perc": it.change_perc,
                "loaded":      it.loaded,
            })).collect::<Vec<_>>(),
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
        // ── Subsystem observability (scanner / RRG / heatmap) ──────────────
        "scanner": {
            "open":         watchlist.scanner_open,
            "result_count": watchlist.scanner_results.len(),
            "def_count":    watchlist.scanner_defs.len(),
            // Filtered count for the first def — proves apply_scanner() ran on
            // the seeded pool (the harness re-derives this to assert correctness).
            "first_def_filtered_count": watchlist.scanner_defs.first()
                .map(|d| crate::chart_renderer::ui::panels::scanner_panel::apply_scanner(d, &watchlist.scanner_results).len())
                .unwrap_or(0),
        },
        "rrg": {
            "open":        watchlist.rrg_open,
            "tail_length": watchlist.rrg_tail_length,
            // Effective sector count: real if populated, else the deterministic
            // demo set the panel falls back to (always 11 SPDR sectors).
            "sector_count": if watchlist.rrg_sectors.is_empty() {
                crate::chart_renderer::ui::panels::rrg_panel::demo_sectors().len()
            } else {
                watchlist.rrg_sectors.len()
            },
        },
        "heatmap": {
            "cell_count": watchlist.heatmap_cells.len(),
        },
        // ── Order-manager safety telemetry (proves no live submission) ─────
        "order_manager": {
            "paper_mode":      crate::chart_renderer::trading::order_manager::is_paper_mode(),
            "armed":           crate::chart_renderer::trading::order_manager::is_armed(),
            "trading_blocked": crate::chart_renderer::trading::order_manager::is_trading_blocked(),
            // Authoritative (System-B) order counts from the lock-free snapshot.
            // Non-zero working/pending/filled would indicate a real submit path
            // fired — the harness asserts these stay 0.
            "mgr_working_count": crate::chart_renderer::trading::snapshot::current().orders.iter()
                .filter(|o| matches!(o.state,
                    crate::chart_renderer::trading::OrderState::Working
                    | crate::chart_renderer::trading::OrderState::PendingSubmit
                    | crate::chart_renderer::trading::OrderState::PartialFill
                    | crate::chart_renderer::trading::OrderState::Filled)).count(),
        },
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
    guard.fps               = fps;
    guard.frame_time_ms     = frame_time_ms;
    guard.widget_tree       = all_widgets;
    guard.open_dialogs      = open_dialogs;
    guard.active_violations = violations;

    // Rolling history (cap at 300).
    guard.fps_history.push_back(fps);
    if guard.fps_history.len() > 300 { guard.fps_history.pop_front(); }
    let viol_count = guard.active_violations.len();
    guard.violation_history.push_back(viol_count);
    if guard.violation_history.len() > 300 { guard.violation_history.pop_front(); }

    // Patch the frame counter into app_state before writing.
    let mut patched_state = app_state;
    if let Some(obj) = patched_state.as_object_mut() {
        obj.insert("frame_counter".into(), serde_json::Value::Number(frame.into()));
    }
    guard.app_state = patched_state;

    // Capture canvas snapshot — reads already-computed chart state, no GPU work.
    let mut snap = canvas::capture(panes, active_pane);
    snap.captured_at_frame = frame;
    guard.canvas = snap;
}

/// Paint active debug annotations as egui overlay (called from core.rs, after draw_chart).
/// Uses the Tooltip layer so annotations appear above all chart content.
pub fn render_annotations(ctx: &egui::Context) {
    let shared = match SHARED_STATE.get() { Some(s) => s, None => return };
    let annotations = match shared.lock() {
        Ok(g) => g.active_annotations.clone(),
        Err(_) => return,
    };
    if annotations.is_empty() { return; }

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("dev_annotations"),
    ));

    for ann in &annotations {
        let rect   = ann.rect.to_egui();
        let color  = egui::Color32::from_rgba_unmultiplied(
            ann.color[0], ann.color[1], ann.color[2], ann.color[3],
        );
        if ann.border_only {
            painter.rect_stroke(rect, egui::CornerRadius::ZERO,
                egui::Stroke::new(ann.border_width.unwrap_or(1.0), color),
                egui::StrokeKind::Outside);
        } else {
            painter.rect_filled(rect, egui::CornerRadius::ZERO, color);
            if ann.border_width.unwrap_or(0.0) > 0.0 {
                let border_color = egui::Color32::from_rgba_unmultiplied(
                    ann.color[0], ann.color[1], ann.color[2],
                    ann.color[3].saturating_add(80),
                );
                painter.rect_stroke(rect, egui::CornerRadius::ZERO,
                    egui::Stroke::new(ann.border_width.unwrap_or(1.0), border_color),
                    egui::StrokeKind::Outside);
            }
        }
        if !ann.label.is_empty() {
            let label_color = if ann.color[3] > 100 {
                egui::Color32::WHITE
            } else {
                color.gamma_multiply(3.0)
            };
            painter.text(
                rect.min + egui::vec2(3.0, 2.0),
                egui::Align2::LEFT_TOP,
                &ann.label,
                egui::FontId::monospace(10.0),
                label_color,
            );
        }
    }
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
    use crate::chart_renderer::gpu::PaneType;
    push(AppCommand::CloseAllDialogs);
    push(AppCommand::CancelAllOrders);
    // Clean slate for EVERY possible pane slot, not just 0/1. A previous scenario
    // can leave the shared session with extra layout panes and/or a non-Chart pane
    // (Spreadsheet/Dashboard) still holding a stale symbol. Because bar-load results
    // route by symbol across all panes, that makes the routing ambiguous and lets a
    // stale load land on pane 0. Forcing every pane back to a plain Chart on SPY/5m
    // (no indicators) removes the ambiguity. Commands for non-existent panes no-op.
    // SwapPaneSymbol/ChangeTimeframe/ClearIndicators use get_mut and safely no-op
    // for slots that don't exist, so we can sweep a generous range.
    for pane in 0..8usize {
        push(AppCommand::ClearIndicators { pane });
        push(AppCommand::SwapPaneSymbol { pane, symbol: "SPY".into() });
        push(AppCommand::ChangeTimeframe { pane, tf: "5m".into() });
    }
    // ChangePaneType debug_asserts on out-of-bounds indices, so only issue it for
    // the standard two panes (covers a leftover Spreadsheet/Dashboard on pane 1).
    push(AppCommand::ChangePaneType { pane: 0, kind: PaneType::Chart });
    push(AppCommand::ChangePaneType { pane: 1, kind: PaneType::Chart });
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
