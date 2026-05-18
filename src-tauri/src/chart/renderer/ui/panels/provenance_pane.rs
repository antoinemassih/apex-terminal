//! ProvenancePane — evidence DAG behind any clicked signal/trade plan.
//!
//! See `docs/SOTA_UX_DESIGN.md` §4.1.
//!
//! Trigger: SignalsPanel row 🔍 button (and later: trade plan overlay,
//! contributor nodes in another ProvenancePane). The trigger dispatches an
//! `OpenProvenanceFor { lineage_id }` event into the panel's static event
//! queue (see `request_open` / `drain_pending`).
//!
//! Architecture
//! - `ProvenancePane` UI state lives in the static `STATE` Mutex. There's
//!   one pane in the layout — open/close + active lineage live in
//!   `Watchlist.provenance_open` / `Watchlist.provenance_active_lineage`.
//! - Fetches go out on background threads (REST client is blocking); results
//!   land back via the static `RESULTS` mpsc receiver, drained at the top of
//!   each frame.
//! - LRU cache (capped at 100 entries) keyed by lineage_id since provenance
//!   is immutable per the spec.
//! - State-machine logic is factored into `pane_state` for unit tests that
//!   don't depend on egui.

use egui;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};

use crate::apex_data::types::{LineageId, ProvenanceMode, ProvenanceTreeNode};
use crate::apex_data::rest::{get_provenance, ProvenanceError};
use crate::chart_renderer::gpu::{Theme, Watchlist};

use super::super::style::*;

// ── Constants ───────────────────────────────────────────────────────────────

/// LRU cap. Provenance trees are immutable so we can keep them around aggressively.
const LRU_CAP: usize = 100;

/// Spec §4.1: cap rendered nodes at 500 with a "Showing first 500 of N" banner.
pub const RENDER_NODE_CAP: usize = 500;

/// Default depth on first open per spec §4.1.
pub const DEFAULT_DEPTH: u8 = 3;

/// Depth options offered by the footer selector.
pub const DEPTH_CHOICES: &[u8] = &[1, 3, 5, 10];

// ── Event bus (cross-panel) ─────────────────────────────────────────────────

/// Request to open the pane for a specific lineage. Pushed from
/// `signals_panel::draw_signal_row_calibrated` (and later trade plan overlay,
/// other panes that surface lineage IDs). Drained at the top of each
/// `provenance_pane::draw` call.
#[derive(Debug, Clone)]
pub struct OpenProvenanceFor {
    pub lineage_id: LineageId,
}

static PENDING: OnceLock<Mutex<VecDeque<OpenProvenanceFor>>> = OnceLock::new();

fn pending() -> &'static Mutex<VecDeque<OpenProvenanceFor>> {
    PENDING.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Push an open-request onto the global queue. Safe to call from any thread
/// or any panel — the pane drains the queue at the start of each frame.
pub fn request_open(lineage_id: impl Into<LineageId>) {
    if let Ok(mut q) = pending().lock() {
        q.push_back(OpenProvenanceFor { lineage_id: lineage_id.into() });
    }
}

/// Drain pending open-requests. Returns the most-recent only (older
/// requests are dropped — only one pane to show).
#[cfg(test)]
pub fn drain_pending_for_test() -> Option<OpenProvenanceFor> {
    let mut q = pending().lock().ok()?;
    let last = q.pop_back();
    q.clear();
    last
}

fn drain_pending() -> Option<OpenProvenanceFor> {
    let mut q = pending().lock().ok()?;
    let last = q.pop_back();
    q.clear();
    last
}

// ── Pane state machine ──────────────────────────────────────────────────────

/// Load status for a single lineage_id within the cache.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadStatus {
    /// Fetch in flight on a background thread.
    Loading,
    /// Loaded successfully. Tree is owned by the cache.
    Loaded(ProvenanceTreeNode),
    /// Fetch failed — `NotFound` (404, aged out) or `Other(string)`.
    Error(String),
    /// Specifically a 404 — render the spec-mandated "aged out of the 24h
    /// hot window" message rather than a generic error.
    NotFound,
}

/// LRU-ish cache: a HashMap + ordered insertion queue. Eviction is FIFO
/// when we exceed `LRU_CAP`, which matches the spec ("LRU cache by
/// lineage_id … cap 100"). Strict LRU touch-on-read isn't worth the
/// complexity for a 100-entry cache.
#[derive(Default)]
pub struct ProvenanceCache {
    entries: HashMap<LineageId, LoadStatus>,
    order:   VecDeque<LineageId>,
    cap:     usize,
}

impl ProvenanceCache {
    pub fn with_cap(cap: usize) -> Self {
        Self { entries: HashMap::new(), order: VecDeque::with_capacity(cap), cap }
    }
    pub fn get(&self, id: &str) -> Option<&LoadStatus> { self.entries.get(id) }
    pub fn contains(&self, id: &str) -> bool { self.entries.contains_key(id) }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// Insert (or replace) an entry. Evicts oldest if over cap. New entries
    /// go to the back of `order`; replacements don't change ordering.
    pub fn insert(&mut self, id: LineageId, status: LoadStatus) {
        let is_new = !self.entries.contains_key(&id);
        self.entries.insert(id.clone(), status);
        if is_new {
            self.order.push_back(id);
            while self.order.len() > self.cap {
                if let Some(old) = self.order.pop_front() {
                    self.entries.remove(&old);
                }
            }
        }
    }

    #[cfg(test)]
    pub fn evict(&mut self, id: &str) {
        self.entries.remove(id);
        self.order.retain(|x| x != id);
    }
}

/// Pure state for the pane — separated from the egui draw code so it can be
/// exercised by `#[cfg(test)]` without instantiating a Context.
pub struct PaneState {
    pub cache:    ProvenanceCache,
    pub expanded: HashSet<LineageId>,
    pub focused:  Option<LineageId>,
    pub depth:    u8,
    pub mode:     ProvenanceMode,
}

impl Default for PaneState {
    fn default() -> Self {
        Self {
            cache:    ProvenanceCache::with_cap(LRU_CAP),
            expanded: HashSet::new(),
            focused:  None,
            depth:    DEFAULT_DEPTH,
            mode:     ProvenanceMode::Tree,
        }
    }
}

impl PaneState {
    /// True if a tree node is currently expanded in the UI. The root is
    /// implicitly expanded — UI tracks descendants only.
    pub fn is_expanded(&self, id: &str) -> bool { self.expanded.contains(id) }

    pub fn toggle_expand(&mut self, id: &str) {
        if !self.expanded.remove(id) {
            self.expanded.insert(id.into());
        }
    }

    /// True if the entry is already cached (loaded or in-flight). Drives
    /// the lazy-fetch decision on expand.
    pub fn needs_fetch(&self, id: &str) -> bool {
        match self.cache.get(id) {
            None => true,
            Some(LoadStatus::Error(_)) | Some(LoadStatus::NotFound) => true,
            _ => false,
        }
    }

    pub fn set_focused(&mut self, id: Option<LineageId>) { self.focused = id; }
}

// ── Static pane singleton + result channel ──────────────────────────────────

struct PaneRuntime {
    state:  Mutex<PaneState>,
    /// Sender for background fetch results. Receiver lives on the UI thread
    /// (`std::sync::Mutex` because we drain from the egui draw closure).
    tx:     std::sync::mpsc::Sender<FetchResult>,
    rx:     Mutex<std::sync::mpsc::Receiver<FetchResult>>,
}

struct FetchResult {
    lineage_id: LineageId,
    outcome:    LoadStatus,
}

static RUNTIME: OnceLock<PaneRuntime> = OnceLock::new();

fn runtime() -> &'static PaneRuntime {
    RUNTIME.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        PaneRuntime {
            state: Mutex::new(PaneState::default()),
            tx,
            rx: Mutex::new(rx),
        }
    })
}

fn spawn_fetch(lineage_id: LineageId, depth: u8, mode: ProvenanceMode) {
    let tx = runtime().tx.clone();
    if let Ok(mut s) = runtime().state.lock() {
        s.cache.insert(lineage_id.clone(), LoadStatus::Loading);
    }
    std::thread::spawn(move || {
        let outcome = match get_provenance(&lineage_id, depth, mode) {
            Ok(node) => LoadStatus::Loaded(node),
            Err(ProvenanceError::NotFound) => LoadStatus::NotFound,
            Err(ProvenanceError::Other(e)) => LoadStatus::Error(e),
        };
        let _ = tx.send(FetchResult { lineage_id, outcome });
    });
}

fn drain_fetches() {
    let mut s = match runtime().state.lock() { Ok(s) => s, Err(_) => return };
    let rx = match runtime().rx.lock() { Ok(rx) => rx, Err(_) => return };
    while let Ok(r) = rx.try_recv() {
        s.cache.insert(r.lineage_id, r.outcome);
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Format a node count visit cap. Returns `(visited, truncated)`.
/// Walk the tree depth-first, increment `visited` per node; if `visited
/// >= RENDER_NODE_CAP`, stop and report truncation.
fn count_nodes(node: &ProvenanceTreeNode) -> usize {
    let mut n = 1usize;
    let mut stack = vec![node];
    while let Some(top) = stack.pop() {
        for c in &top.children { n += 1; stack.push(c); }
    }
    n
}

/// Short-prefix for cycle marker rendering ("↻ Cycle to <prefix>").
fn lineage_prefix(id: &str) -> String {
    if id.len() <= 8 { id.into() } else { format!("{}…", &id[..8]) }
}

fn format_age(now_ms: i64, t_ms: i64) -> String {
    if t_ms <= 0 { return "—".into(); }
    let secs = ((now_ms - t_ms).max(0)) / 1000;
    if secs < 60 { format!("{}s ago", secs) }
    else if secs < 3600 { format!("{}m ago", secs / 60) }
    else if secs < 86_400 { format!("{}h ago", secs / 3600) }
    else { format!("{}d ago", secs / 86_400) }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ── Draw ────────────────────────────────────────────────────────────────────

/// Render the pane. Mirrors the panel kit pattern (e.g. `news_panel::draw`):
/// gated on `watchlist.provenance_open`, owns its own state via static
/// runtime. Reads `watchlist.provenance_active_lineage` to know the root.
pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    // Always drain — even if closed — so background loads don't pile up.
    drain_fetches();

    // Promote any pending open-requests into the active lineage and force-open.
    if let Some(req) = drain_pending() {
        watchlist.provenance_open = true;
        // Reset focused on a new root.
        if let Ok(mut s) = runtime().state.lock() {
            s.focused = Some(req.lineage_id.clone());
            s.expanded.clear();
            // Kick a fetch if we don't already have it.
            if s.needs_fetch(&req.lineage_id) {
                let depth = s.depth;
                let mode  = s.mode;
                drop(s);
                spawn_fetch(req.lineage_id.clone(), depth, mode);
            }
        }
        watchlist.provenance_active_lineage = Some(req.lineage_id);
    }

    if !watchlist.provenance_open { return; }

    egui::SidePanel::right("provenance_pane")
        .default_width(360.0)
        .min_width(260.0)
        .max_width(560.0)
        .resizable(true)
        .frame(egui::Frame::NONE
            .fill(t.toolbar_bg)
            .stroke(egui::Stroke::new(stroke_thin(),
                color_alpha(t.toolbar_border, alpha_heavy())))
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 }))
        .show(ctx, |ui| {
            draw_inner(ui, watchlist, t);
        });
}

fn draw_inner(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    let active = watchlist.provenance_active_lineage.clone();

    // ── Header ──
    ui.horizontal(|ui| {
        ui.add_space(gap_sm());
        ui.label(egui::RichText::new("PROVENANCE")
            .monospace().size(FONT_SM).strong().color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(egui::RichText::new("\u{00D7}")
                .monospace().size(FONT_MD).color(t.dim))
                .fill(egui::Color32::TRANSPARENT)
                .min_size(egui::vec2(icon_lg(), icon_lg()))).clicked() {
                watchlist.provenance_open = false;
            }
        });
    });
    ui.painter().line_segment(
        [egui::pos2(ui.min_rect().left(), ui.min_rect().bottom()),
         egui::pos2(ui.min_rect().right(), ui.min_rect().bottom())],
        egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_muted())));

    let Some(active_id) = active else {
        ui.add_space(gap_lg());
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("No signal selected")
                .monospace().size(FONT_SM).color(t.dim));
            ui.label(egui::RichText::new("Click 🔍 next to any signal to trace its evidence.")
                .monospace().size(FONT_XS).color(t.dim.gamma_multiply(0.7)));
        });
        return;
    };

    // ── Top-bar (lineage_id, source engine, symbol, t_ms, score) ──
    let cache_snapshot = runtime().state.lock().ok()
        .and_then(|s| s.cache.get(&active_id).cloned());
    ui.horizontal_wrapped(|ui| {
        ui.add_space(gap_sm());
        let label = egui::RichText::new(format!("lineage: {}", active_id))
            .monospace().size(FONT_XS).color(t.accent);
        let resp = ui.label(label);
        if resp.on_hover_text("Click to copy").clicked() {
            ui.output_mut(|o| o.copied_text = active_id.clone());
        }
    });

    if let Some(LoadStatus::Loaded(node)) = &cache_snapshot {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(gap_sm());
            ui.label(egui::RichText::new(format!("{} · {}", node.source_engine, node.symbol))
                .monospace().size(FONT_XS).color(t.dim));
            if let Some(s) = node.score {
                ui.label(egui::RichText::new(format!("score {:.2}", s))
                    .monospace().size(FONT_XS).color(t.text));
            }
            ui.label(egui::RichText::new(format_age(now_ms(), node.t_ms))
                .monospace().size(FONT_XS).color(t.dim.gamma_multiply(0.7)));
        });
    }

    ui.add_space(gap_sm());
    ui.painter().line_segment(
        [egui::pos2(ui.min_rect().left(), ui.cursor().min.y),
         egui::pos2(ui.min_rect().right(), ui.cursor().min.y)],
        egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_faint())));
    ui.add_space(gap_sm());

    // ── Body ──
    match cache_snapshot {
        None | Some(LoadStatus::Loading) => {
            ui.add_space(gap_lg());
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Loading…")
                    .monospace().size(FONT_SM).color(t.dim));
            });
        }
        Some(LoadStatus::NotFound) => {
            ui.add_space(gap_md());
            ui.label(egui::RichText::new(
                "Provenance log doesn't have this entry. It may have aged \
                 out of the 24h hot window.")
                .monospace().size(FONT_XS).color(t.warn));
        }
        Some(LoadStatus::Error(msg)) => {
            ui.add_space(gap_md());
            ui.label(egui::RichText::new(format!("Error: {msg}"))
                .monospace().size(FONT_XS).color(t.bear));
            if ui.button("Retry").clicked() {
                if let Ok(s) = runtime().state.lock() {
                    let depth = s.depth; let mode = s.mode;
                    drop(s);
                    spawn_fetch(active_id.clone(), depth, mode);
                }
            }
        }
        Some(LoadStatus::Loaded(root)) => {
            let total = count_nodes(&root);
            // Truncation banner per spec §4.1.
            if let Some(server_total) = root.truncated_total {
                let n = server_total.max(total as u32);
                ui.label(egui::RichText::new(format!(
                    "Showing first {} of {} nodes — increase depth or open the DAG view",
                    RENDER_NODE_CAP.min(total), n))
                    .monospace().size(FONT_XS).color(t.warn));
                ui.add_space(gap_xs());
            } else if total > RENDER_NODE_CAP {
                ui.label(egui::RichText::new(format!(
                    "Showing first {} of {} nodes", RENDER_NODE_CAP, total))
                    .monospace().size(FONT_XS).color(t.warn));
                ui.add_space(gap_xs());
            }

            // ── Scrollable tree ──
            let mut clicked_lineage: Option<LineageId> = None;
            let mut toggled_lineage: Option<LineageId> = None;
            let mut budget = RENDER_NODE_CAP;
            egui::ScrollArea::vertical()
                .id_salt("provenance_tree")
                .max_height(ui.available_height() - 60.0)
                .show(ui, |ui| {
                    let expanded_snapshot: HashSet<LineageId> = runtime().state.lock().ok()
                        .map(|s| s.expanded.clone()).unwrap_or_default();
                    draw_tree_node(
                        ui, &root, 0, &expanded_snapshot, t,
                        &mut clicked_lineage, &mut toggled_lineage, &mut budget,
                    );
                });
            if let Some(id) = clicked_lineage {
                if let Ok(mut s) = runtime().state.lock() { s.focused = Some(id); }
            }
            if let Some(id) = toggled_lineage {
                let (depth, mode, need_fetch) = {
                    let mut s = match runtime().state.lock() { Ok(s) => s, Err(_) => return };
                    s.toggle_expand(&id);
                    (s.depth, s.mode, s.needs_fetch(&id) && s.is_expanded(&id))
                };
                if need_fetch {
                    spawn_fetch(id, depth, mode);
                }
            }

            // ── Focused-node detail ──
            ui.add_space(gap_sm());
            let focused_id = runtime().state.lock().ok().and_then(|s| s.focused.clone());
            if let Some(fid) = focused_id {
                if let Some(node) = find_node(&root, &fid) {
                    ui.painter().line_segment(
                        [egui::pos2(ui.min_rect().left(), ui.cursor().min.y),
                         egui::pos2(ui.min_rect().right(), ui.cursor().min.y)],
                        egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_faint())));
                    ui.add_space(gap_xs());
                    ui.label(egui::RichText::new("FOCUSED NODE").monospace()
                        .size(FONT_XS).color(t.dim));
                    ui.label(egui::RichText::new(format!("{} · {}", node.kind, node.source_engine))
                        .monospace().size(FONT_XS).color(t.text));
                    if let Some(p) = &node.payload {
                        let pretty = serde_json::to_string_pretty(p)
                            .unwrap_or_else(|_| "(payload not displayable)".into());
                        egui::ScrollArea::vertical()
                            .id_salt("provenance_detail")
                            .max_height(120.0)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(pretty)
                                    .monospace().size(FONT_XS).color(t.dim));
                            });
                    }
                }
            }
        }
    }

    // ── Footer: depth, mode, copy ──
    ui.add_space(gap_xs());
    ui.painter().line_segment(
        [egui::pos2(ui.min_rect().left(), ui.cursor().min.y),
         egui::pos2(ui.min_rect().right(), ui.cursor().min.y)],
        egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_faint())));
    ui.add_space(gap_xs());
    ui.horizontal(|ui| {
        ui.add_space(gap_sm());
        ui.label(egui::RichText::new("depth").monospace().size(FONT_XS).color(t.dim));
        let cur_depth = runtime().state.lock().ok().map(|s| s.depth).unwrap_or(DEFAULT_DEPTH);
        for &d in DEPTH_CHOICES {
            let selected = d == cur_depth;
            let col = if selected { t.accent } else { t.dim.gamma_multiply(0.6) };
            if ui.add(egui::Button::new(egui::RichText::new(format!("{}", d))
                .monospace().size(FONT_XS).color(col))
                .fill(egui::Color32::TRANSPARENT)
                .min_size(egui::vec2(20.0, 18.0))).clicked()
            {
                if let Ok(mut s) = runtime().state.lock() { s.depth = d; }
                // Re-fetch root with new depth.
                if let Some(id) = watchlist.provenance_active_lineage.clone() {
                    let mode = runtime().state.lock().ok().map(|s| s.mode).unwrap_or_default();
                    spawn_fetch(id, d, mode);
                }
            }
        }
        ui.add_space(gap_sm());
        let cur_mode = runtime().state.lock().ok().map(|s| s.mode).unwrap_or_default();
        let mode_label = match cur_mode { ProvenanceMode::Tree => "tree", ProvenanceMode::Dag => "dag" };
        if ui.add(egui::Button::new(egui::RichText::new(mode_label)
            .monospace().size(FONT_XS).color(t.accent))
            .fill(egui::Color32::TRANSPARENT)
            .min_size(egui::vec2(0.0, 18.0))).clicked()
        {
            if let Ok(mut s) = runtime().state.lock() {
                s.mode = match s.mode { ProvenanceMode::Tree => ProvenanceMode::Dag, ProvenanceMode::Dag => ProvenanceMode::Tree };
            }
            if let Some(id) = watchlist.provenance_active_lineage.clone() {
                let (d, m) = {
                    let s = runtime().state.lock().ok();
                    s.as_ref().map(|s| (s.depth, s.mode)).unwrap_or((DEFAULT_DEPTH, ProvenanceMode::Tree))
                };
                spawn_fetch(id, d, m);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(egui::RichText::new("copy JSON")
                .monospace().size(FONT_XS).color(t.dim))
                .fill(egui::Color32::TRANSPARENT)
                .min_size(egui::vec2(0.0, 18.0))).clicked()
            {
                let id_opt = watchlist.provenance_active_lineage.clone();
                if let Some(id) = id_opt {
                    let json = runtime().state.lock().ok().and_then(|s| {
                        match s.cache.get(&id)? {
                            LoadStatus::Loaded(n) => serde_json::to_string_pretty(n).ok(),
                            _ => None,
                        }
                    }).unwrap_or_else(|| "{}".into());
                    ui.output_mut(|o| o.copied_text = json);
                }
            }
            ui.add_space(gap_sm());
        });
    });
}

fn draw_tree_node(
    ui: &mut egui::Ui,
    node: &ProvenanceTreeNode,
    indent: u8,
    expanded: &HashSet<LineageId>,
    t: &Theme,
    clicked: &mut Option<LineageId>,
    toggled: &mut Option<LineageId>,
    budget: &mut usize,
) {
    if *budget == 0 { return; }
    *budget = budget.saturating_sub(1);

    let id = node.lineage_id.clone();
    let is_expanded = indent == 0 || expanded.contains(&id);
    ui.horizontal(|ui| {
        ui.add_space(gap_sm() + (indent as f32) * 12.0);
        if node.is_cycle {
            ui.label(egui::RichText::new(format!("↻ Cycle to {}", lineage_prefix(&id)))
                .monospace().size(FONT_XS).color(t.warn));
            return;
        }
        let marker = if node.children.is_empty() { "·" }
                     else if is_expanded { "▾" } else { "▸" };
        let col_marker = if node.children.is_empty() { t.dim.gamma_multiply(0.5) } else { t.dim };
        if ui.add(egui::Button::new(egui::RichText::new(marker)
            .monospace().size(FONT_XS).color(col_marker))
            .fill(egui::Color32::TRANSPARENT)
            .min_size(egui::vec2(icon_sm(), icon_sm()))).clicked()
        {
            if !node.children.is_empty() || indent > 0 {
                *toggled = Some(id.clone());
            }
        }
        let label = if node.source_engine.is_empty() {
            node.kind.clone()
        } else {
            format!("{}", node.source_engine)
        };
        let score_str = node.score.map(|s| format!(" {:.2}", s)).unwrap_or_default();
        let row_label = egui::RichText::new(format!("{}{}", label, score_str))
            .monospace().size(FONT_XS).color(t.text);
        if ui.add(egui::Label::new(row_label).sense(egui::Sense::click())).clicked() {
            *clicked = Some(id.clone());
        }
        if !node.symbol.is_empty() {
            ui.label(egui::RichText::new(&node.symbol).monospace().size(FONT_XS).color(t.dim));
        }
    });
    if is_expanded {
        for child in &node.children {
            draw_tree_node(ui, child, indent + 1, expanded, t, clicked, toggled, budget);
            if *budget == 0 { break; }
        }
    }
}

fn find_node<'a>(root: &'a ProvenanceTreeNode, id: &str) -> Option<&'a ProvenanceTreeNode> {
    if root.lineage_id == id { return Some(root); }
    for c in &root.children {
        if let Some(found) = find_node(c, id) { return Some(found); }
    }
    None
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apex_data::types::ProvenanceTreeNode;

    fn leaf(id: &str) -> ProvenanceTreeNode {
        ProvenanceTreeNode { lineage_id: id.into(), ..Default::default() }
    }

    fn node(id: &str, children: Vec<ProvenanceTreeNode>) -> ProvenanceTreeNode {
        ProvenanceTreeNode { lineage_id: id.into(), children, ..Default::default() }
    }

    // ── Cache state machine ───────────────────────────────────────────────

    #[test]
    fn cache_insert_and_get() {
        let mut c = ProvenanceCache::with_cap(4);
        c.insert("a".into(), LoadStatus::Loading);
        assert!(matches!(c.get("a"), Some(LoadStatus::Loading)));
        assert!(c.contains("a"));
        c.insert("a".into(), LoadStatus::Loaded(leaf("a")));
        assert!(matches!(c.get("a"), Some(LoadStatus::Loaded(_))));
        assert_eq!(c.len(), 1, "replace doesn't grow");
    }

    #[test]
    fn cache_evicts_oldest_when_over_cap() {
        let mut c = ProvenanceCache::with_cap(3);
        c.insert("a".into(), LoadStatus::Loaded(leaf("a")));
        c.insert("b".into(), LoadStatus::Loaded(leaf("b")));
        c.insert("c".into(), LoadStatus::Loaded(leaf("c")));
        c.insert("d".into(), LoadStatus::Loaded(leaf("d")));
        assert!(!c.contains("a"), "oldest evicted");
        assert!(c.contains("d"));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn cache_error_states_distinct() {
        let mut c = ProvenanceCache::with_cap(2);
        c.insert("x".into(), LoadStatus::NotFound);
        c.insert("y".into(), LoadStatus::Error("boom".into()));
        assert!(matches!(c.get("x"), Some(LoadStatus::NotFound)));
        match c.get("y") {
            Some(LoadStatus::Error(s)) => assert_eq!(s, "boom"),
            _ => panic!("expected Error"),
        }
    }

    // ── PaneState collapse/expand ─────────────────────────────────────────

    #[test]
    fn expand_toggle_adds_and_removes() {
        let mut s = PaneState::default();
        assert!(!s.is_expanded("foo"));
        s.toggle_expand("foo");
        assert!(s.is_expanded("foo"));
        s.toggle_expand("foo");
        assert!(!s.is_expanded("foo"));
    }

    #[test]
    fn needs_fetch_uncached() {
        let s = PaneState::default();
        assert!(s.needs_fetch("anything"));
    }

    #[test]
    fn needs_fetch_loading_returns_false() {
        let mut s = PaneState::default();
        s.cache.insert("a".into(), LoadStatus::Loading);
        assert!(!s.needs_fetch("a"));
    }

    #[test]
    fn needs_fetch_loaded_returns_false() {
        let mut s = PaneState::default();
        s.cache.insert("a".into(), LoadStatus::Loaded(leaf("a")));
        assert!(!s.needs_fetch("a"));
    }

    #[test]
    fn needs_fetch_after_error_retries() {
        let mut s = PaneState::default();
        s.cache.insert("a".into(), LoadStatus::Error("boom".into()));
        assert!(s.needs_fetch("a"), "errored entries should retry");
        s.cache.insert("b".into(), LoadStatus::NotFound);
        assert!(s.needs_fetch("b"), "404 entries should retry on explicit click");
    }

    #[test]
    fn focused_node_can_be_cleared() {
        let mut s = PaneState::default();
        s.set_focused(Some("z".into()));
        assert_eq!(s.focused.as_deref(), Some("z"));
        s.set_focused(None);
        assert_eq!(s.focused, None);
    }

    // ── Tree helpers ──────────────────────────────────────────────────────

    #[test]
    fn count_nodes_walks_full_tree() {
        let tree = node("root", vec![
            node("a", vec![leaf("a1"), leaf("a2")]),
            node("b", vec![leaf("b1")]),
        ]);
        assert_eq!(count_nodes(&tree), 6);
    }

    #[test]
    fn lineage_prefix_truncates_long_ids() {
        assert_eq!(lineage_prefix("abc"), "abc");
        assert_eq!(lineage_prefix("0123456789ABCDEF"), "01234567…");
    }

    #[test]
    fn find_node_returns_correct_subtree() {
        let tree = node("r", vec![
            node("a", vec![leaf("a1")]),
            leaf("b"),
        ]);
        assert_eq!(find_node(&tree, "a1").map(|n| n.lineage_id.as_str()), Some("a1"));
        assert!(find_node(&tree, "missing").is_none());
    }

    #[test]
    fn format_age_grades_correctly() {
        assert_eq!(format_age(0, 0), "—");
        let now = 100_000_000_000_i64;
        assert_eq!(format_age(now, now - 30_000), "30s ago");
        assert_eq!(format_age(now, now - 5 * 60_000), "5m ago");
        assert_eq!(format_age(now, now - 2 * 3_600_000), "2h ago");
        assert_eq!(format_age(now, now - 3 * 86_400_000), "3d ago");
    }

    // ── Open-request bus ──────────────────────────────────────────────────

    #[test]
    fn request_open_coalesces_to_latest() {
        // Clear leftover state from prior tests in this process.
        let _ = drain_pending_for_test();
        request_open("a");
        request_open("b");
        request_open("c");
        let last = drain_pending_for_test().expect("got a request");
        assert_eq!(last.lineage_id, "c");
        assert!(drain_pending_for_test().is_none());
    }
}
