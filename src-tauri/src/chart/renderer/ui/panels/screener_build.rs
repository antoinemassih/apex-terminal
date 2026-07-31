//! Screener BUILD tab — visual condition builder + DSL text mode.
//!
//! ## Responsibilities
//!
//! This module owns only the BUILD tab body.  The outer tab shell, the open
//! flag, and `ScreenPanelState` are all owned by T-SHELL
//! (`state/aggregates.rs` + `commands.rs`).  This file reads
//! `ScreenBuilderState` (a sub-field of `ScreenPanelState` that T-SHELL
//! will add) and emits `AppCommand`s; it never touches `Watchlist` or
//! `Chart` fields directly.
//!
//! ## Two modes
//!
//! **Visual mode** — a tree of predicate cards.  Each AND/OR group renders as
//! a `PanelSubSection` with an indent.  Each predicate renders as a
//! `PanelCard` with three columns: left-operand `SearchInput`, comparator
//! `ComboBox`, right-operand `Input`.  The autocomplete operand list is fed
//! from:
//!   1. ApexSignals `GET {APEX_SIGNALS_HTTP}/admin/catalog` (signal families),
//!      with fallback to `/api/signals/families`.
//!   2. ApexData    `GET {APEX_DATA_URL}/api/scan/catalog`  (indicator/price).
//! Both are fetched once on a background thread; until they land the
//! dropdown shows a static built-in list.
//!
//! **DSL text mode** — a multi-line `TextEdit`.  On change the text is POSTed
//! to `{APEX_DATA_URL}/api/data/screens/parse`, returning
//! `{ condition, rank_expr?, errors:[{span,msg}] }`.  Errors are shown as
//! `Tag::TagTone::Warn` chips below the textarea.  Toggling DSL → Visual
//! calls `{APEX_DATA_URL}/api/data/screens/print` with the current
//! `condition` JSON to re-seed the text area with canonical DSL.
//!
//! ## State fields T-SHELL must add to `ScreenPanelState` (aggregates.rs)
//!
//! ```text
//! // On ScreenPanelState:
//! pub screen_builder: ScreenBuilderState,
//!
//! pub struct ScreenBuilderState {
//!     pub name:              String,
//!     pub tf:                String,
//!     pub class:             String,   // "stocks" | "options"
//!     pub universe:          String,
//!     pub dsl_mode:          bool,
//!     pub dsl_text:          String,
//!     pub condition_json:    Option<String>,   // round-trippable Condition JSON
//!     pub rank_by:           String,
//!     pub rank_asc:          bool,
//!     pub limit:             usize,
//!     pub alert_on_entry:    bool,
//!     pub parse_pending:     bool,
//!     pub parse_errors:      Vec<DslParseError>,
//! }
//!
//! pub struct DslParseError {
//!     pub span_start: usize,
//!     pub span_end:   usize,
//!     pub message:    String,
//! }
//! ```
//!
//! ## AppCommand variants T-SHELL must add to `commands.rs`
//!
//! (This file does NOT touch `commands.rs`.)
//!
//! ```text
//! // Screener builder commands:
//! BuilderSetDslMode   { dsl: bool },
//! BuilderSetDslText   { text: String },
//! BuilderSetName      { name: String },
//! BuilderSetTf        { tf: String },
//! BuilderSetClass     { class: String },
//! BuilderSetUniverse  { universe: String },
//! BuilderSetRankBy    { expr: String, asc: bool },
//! BuilderSetLimit     { n: usize },
//! BuilderSetAlertEntry { on: bool },
//! BuilderCommit,   // freeze condition_json into active_screen, switch to RESULTS tab
//! BuilderSave,     // POST /api/scans + persist to workspace
//! BuilderReset,    // clear all builder state
//! ```
//!
//! ## HTTP bindings
//!
//! `POST {APEX_DATA_URL}/api/data/screens/parse`
//!   Request:  `{ "text": "<dsl>" }`
//!   Response: `{ "condition": <JSON>, "rank_expr": ..., "errors": [] }`
//!            or `{ "errors": [{ "span": [start, end], "msg": "..." }] }`
//!
//! `POST {APEX_DATA_URL}/api/data/screens/print`
//!   Request:  `{ "condition": <JSON> }`
//!   Response: `{ "dsl": "<canonical dsl string>" }`
//!
//! Both use `reqwest::blocking` on a background thread (same pattern as
//! `msg_tension_panel.rs`).

#![allow(dead_code)]

use std::sync::{OnceLock, Mutex, atomic::{AtomicBool, AtomicU32, Ordering}};

use egui::{Ui, TextEdit};

use super::super::style::*;
use super::super::super::gpu::Theme;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{
    Button, Checkbox, Input, PanelCard, PanelSection, PanelSubSection,
    SearchInput, Spinner, Tag, TagTone,
};
use crate::ui_kit::widgets::tokens::{Size as KitSize, Variant};

// ── URL helpers ────────────────────────────────────────────────────────────────

fn apex_data_url() -> String {
    crate::data::feeds::apex_data::config::apex_url()
}

fn apex_signals_http() -> String {
    std::env::var("APEX_SIGNALS_HTTP")
        .unwrap_or_else(|_| "http://localhost:8100".to_string())
}

fn http_client() -> &'static reqwest::blocking::Client {
    static C: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .user_agent("apex-terminal/screener-build")
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
}

// ── Operand catalog ────────────────────────────────────────────────────────────

/// Static built-in operand list — always available, no network required.
const BUILTIN_OPERANDS: &[(&str, &str)] = &[
    ("price",       "Price (last)"),
    ("close",       "Close"),
    ("open",        "Open"),
    ("high",        "High"),
    ("low",         "Low"),
    ("volume",      "Volume"),
    ("rvol",        "Relative Volume"),
    ("day_chg_pct", "Day Change %"),
    ("gap_pct",     "Gap %"),
    ("market_cap",  "Market Cap"),
    ("sma20",       "SMA 20"),
    ("sma50",       "SMA 50"),
    ("sma200",      "SMA 200"),
    ("ema9",        "EMA 9"),
    ("ema21",       "EMA 21"),
    ("ema50",       "EMA 50"),
    ("rsi14",       "RSI 14"),
    ("atr14",       "ATR 14"),
    ("vwap",        "VWAP"),
    ("atr_pct",     "ATR %"),
];

const BUILTIN_COMPARATORS: &[&str] = &[
    ">", "<", ">=", "<=", "==", "!=", "crossed_above", "crossed_below",
];

const BUILTIN_TIMEFRAMES: &[&str] = &[
    "1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w",
];

const BUILTIN_CLASSES: &[&str] = &["stocks", "options"];

/// Operand entry from the live catalog.
#[derive(Clone, Debug)]
struct CatalogEntry {
    key:    String,   // DSL key fed to condition JSON
    label:  String,   // user-facing label
    source: String,   // "indicator" | "signal" | "price"
}

#[derive(Default)]
struct CatalogState {
    entries:  Vec<CatalogEntry>,
    fetched:  bool,
    fetching: bool,
}

fn catalog_state() -> &'static Mutex<CatalogState> {
    static C: OnceLock<Mutex<CatalogState>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(CatalogState::default()))
}

/// Spawn a one-shot background fetch of the signal + indicator catalogs.
/// Idempotent: second call is a no-op.
fn ensure_catalog_fetched() {
    {
        let mut g = catalog_state().lock().unwrap();
        if g.fetched || g.fetching { return; }
        g.fetching = true;
    }
    let data_base   = apex_data_url();
    let sigs_base   = apex_signals_http();
    std::thread::spawn(move || {
        let mut entries: Vec<CatalogEntry> = Vec::new();

        // ── ApexData scan catalog ──────────────────────────────────────
        // Expected: { "keys": [{ "key": "sma20", "label": "SMA 20",
        //                        "source": "indicator" }, ...] }
        let data_url = format!("{}/api/scan/catalog", data_base);
        if let Ok(resp) = http_client().get(&data_url).send() {
            if resp.status().is_success() {
                if let Ok(v) = resp.json::<serde_json::Value>() {
                    if let Some(arr) = v.get("keys").and_then(|x| x.as_array()) {
                        for item in arr {
                            let key    = item.get("key").and_then(|x| x.as_str())
                                .unwrap_or("").to_string();
                            let label  = item.get("label").and_then(|x| x.as_str())
                                .unwrap_or(key.as_str()).to_string();
                            let source = item.get("source").and_then(|x| x.as_str())
                                .unwrap_or("indicator").to_string();
                            if !key.is_empty() {
                                entries.push(CatalogEntry { key, label, source });
                            }
                        }
                    }
                }
            }
        }

        // ── ApexSignals family catalog ─────────────────────────────────
        // Try /admin/catalog first (full per-family field list);
        // fall back to /api/signals/families (name-only list).
        // Expected: { "families": [{ "name": "combined",
        //                           "fields": ["score", "confidence"] }, ...] }
        // or a flat array of name strings.
        let admin_url = format!("{}/admin/catalog", sigs_base);
        let fallback  = format!("{}/api/signals/families", sigs_base);
        let sig_url   = {
            let ok = http_client().get(&admin_url).send()
                .map(|r| r.status().is_success()).unwrap_or(false);
            if ok { admin_url } else { fallback }
        };
        if let Ok(resp) = http_client().get(&sig_url).send() {
            if resp.status().is_success() {
                if let Ok(v) = resp.json::<serde_json::Value>() {
                    let families = v.get("families")
                        .and_then(|x| x.as_array())
                        .or_else(|| v.as_array());
                    if let Some(fams) = families {
                        for fam in fams {
                            let name = fam.get("name")
                                .and_then(|x| x.as_str())
                                .or_else(|| fam.as_str())
                                .unwrap_or("").to_string();
                            if name.is_empty() { continue; }
                            let fields: Vec<String> = fam.get("fields")
                                .and_then(|x| x.as_array())
                                .map(|arr| arr.iter()
                                    .filter_map(|f| f.as_str())
                                    .map(|s| s.to_string())
                                    .collect())
                                .unwrap_or_else(|| vec!["score".to_string()]);
                            for field in &fields {
                                let key   = format!("signal(\"{}.{}\")", name, field);
                                let label = format!("{}.{}", name, field);
                                entries.push(CatalogEntry {
                                    key, label, source: "signal".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        let mut g = catalog_state().lock().unwrap();
        // De-dup against built-ins (built-ins win by key).
        let builtin_keys: std::collections::HashSet<&str> =
            BUILTIN_OPERANDS.iter().map(|(k, _)| *k).collect();
        g.entries = entries.into_iter()
            .filter(|e| !builtin_keys.contains(e.key.as_str()))
            .collect();
        g.fetched  = true;
        g.fetching = false;
    });
}

/// Merged autocomplete list: built-ins first, then live catalog.
fn all_operands() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = BUILTIN_OPERANDS.iter()
        .map(|(k, l)| (k.to_string(), l.to_string()))
        .collect();
    if let Ok(g) = catalog_state().try_lock() {
        for e in &g.entries {
            out.push((e.key.clone(), e.label.clone()));
        }
    }
    out
}

// ── DSL parse / print ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct ParseResult {
    /// Serialized Condition JSON — `None` if parse failed or not yet run.
    pub condition: Option<String>,
    /// Parse errors (empty → success).
    pub errors:    Vec<ParseError>,
    /// `true` while a background parse is still in-flight.
    pub pending:   bool,
}

#[derive(Clone, Debug)]
pub struct ParseError {
    pub span_start: usize,
    pub span_end:   usize,
    pub message:    String,
}

fn parse_result() -> &'static Mutex<ParseResult> {
    static C: OnceLock<Mutex<ParseResult>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(ParseResult::default()))
}

static PARSE_INFLIGHT: AtomicBool = AtomicBool::new(false);

/// Fire a background parse request.  Results land in `parse_result()`.
/// No-op if a parse is already in flight.
pub fn trigger_parse(dsl_text: String) {
    if PARSE_INFLIGHT.swap(true, Ordering::SeqCst) { return; }
    // Mark pending immediately so the spinner appears this frame.
    if let Ok(mut g) = parse_result().lock() { g.pending = true; }
    let url = format!("{}/api/data/screens/parse", apex_data_url());
    std::thread::spawn(move || {
        let body = serde_json::json!({ "text": dsl_text });
        let outcome = match http_client().post(&url).json(&body).send() {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>() {
                    Ok(v) => {
                        let condition = v.get("condition").map(|c| c.to_string());
                        let errors = extract_parse_errors(&v);
                        ParseResult { condition, errors, pending: false }
                    }
                    Err(e) => ParseResult {
                        condition: None,
                        errors: vec![ParseError {
                            span_start: 0, span_end: 0,
                            message: format!("JSON decode error: {}", e),
                        }],
                        pending: false,
                    },
                }
            }
            Ok(resp) => ParseResult {
                condition: None,
                errors: vec![ParseError {
                    span_start: 0, span_end: 0,
                    message: format!("Parse endpoint HTTP {}", resp.status().as_u16()),
                }],
                pending: false,
            },
            Err(e) => ParseResult {
                condition: None,
                errors: vec![ParseError {
                    span_start: 0, span_end: 0,
                    message: format!("Network error: {}", e),
                }],
                pending: false,
            },
        };
        if let Ok(mut g) = parse_result().lock() { *g = outcome; }
        PARSE_INFLIGHT.store(false, Ordering::SeqCst);
    });
}

fn extract_parse_errors(v: &serde_json::Value) -> Vec<ParseError> {
    v.get("errors")
        .and_then(|e| e.as_array())
        .map(|arr| arr.iter().filter_map(|e| {
            let msg = e.get("msg")
                .and_then(|x| x.as_str())
                .unwrap_or("parse error")
                .to_string();
            let span = e.get("span").and_then(|s| s.as_array());
            let (start, end) = span.map(|s| (
                s.first().and_then(|x| x.as_u64()).unwrap_or(0) as usize,
                s.get(1).and_then(|x| x.as_u64()).unwrap_or(0) as usize,
            )).unwrap_or((0, 0));
            Some(ParseError { span_start: start, span_end: end, message: msg })
        }).collect())
        .unwrap_or_default()
}

/// Synchronously POST to /api/data/screens/print to get canonical DSL from
/// a condition JSON blob.  Runs on the calling thread — only call when the
/// user explicitly triggers a visual→DSL toggle; do NOT call in a render loop.
/// Returns `None` if the endpoint is unreachable or returns an error.
pub fn fetch_print(condition_json: &str) -> Option<String> {
    let url   = format!("{}/api/data/screens/print", apex_data_url());
    let cond  = serde_json::from_str::<serde_json::Value>(condition_json).ok()?;
    let body  = serde_json::json!({ "condition": cond });
    let resp  = http_client().post(&url).json(&body).send().ok()?;
    if !resp.status().is_success() { return None; }
    let v: serde_json::Value = resp.json().ok()?;
    v.get("dsl").and_then(|d| d.as_str()).map(|s| s.to_string())
}

// ── Condition node tree ────────────────────────────────────────────────────────
// Held as local static until T-SHELL provides the full ScreenBuilderState.
// When T-SHELL wires up state/aggregates.rs, replace these statics with
// reads from ScreenPanelState::screen_builder.

/// Whether the leaf is a predicate or an AND/OR group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GroupOp { And, Or }

/// A node in the visual condition tree.
#[derive(Clone, Debug)]
pub struct ConditionNode {
    pub id:         u32,
    pub group_op:   Option<GroupOp>,  // None = leaf predicate
    pub left_op:    String,           // left operand key (e.g. "rsi14")
    pub comparator: String,           // e.g. ">"
    pub right_op:   String,           // numeric constant or operand key
    pub children:   Vec<ConditionNode>,
    pub expanded:   bool,
}

impl ConditionNode {
    fn leaf(id: u32) -> Self {
        Self {
            id, group_op: None,
            left_op: String::new(), comparator: ">".to_string(), right_op: String::new(),
            children: vec![], expanded: true,
        }
    }
    fn group(id: u32, op: GroupOp) -> Self {
        Self {
            id, group_op: Some(op),
            left_op: String::new(), comparator: String::new(), right_op: String::new(),
            children: vec![], expanded: true,
        }
    }
}

// ── Local ephemeral state (replaced by ScreenBuilderState once T-SHELL lands) ─

static NEXT_ID: AtomicU32 = AtomicU32::new(2);
fn next_id() -> u32 { NEXT_ID.fetch_add(1, Ordering::SeqCst) }

fn node_tree() -> &'static Mutex<Vec<ConditionNode>> {
    static T: OnceLock<Mutex<Vec<ConditionNode>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(vec![ConditionNode::leaf(1)]))
}

fn dsl_mode_flag() -> &'static AtomicBool {
    static F: AtomicBool = AtomicBool::new(false);
    &F
}

fn dsl_text_buf() -> &'static Mutex<String> {
    static B: OnceLock<Mutex<String>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(String::new()))
}

// Per-node operand search strings (keyed by node id).
fn left_op_buf() -> &'static Mutex<std::collections::HashMap<u32, String>> {
    static M: OnceLock<Mutex<std::collections::HashMap<u32, String>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}
fn right_op_buf() -> &'static Mutex<std::collections::HashMap<u32, String>> {
    static M: OnceLock<Mutex<std::collections::HashMap<u32, String>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

// Screen metadata (name, tf, class, universe, rank, limit, alert).
#[derive(Clone, Debug)]
struct ScreenMeta {
    name:           String,
    tf:             String,
    class:          String,
    universe:       String,
    rank_by:        String,
    rank_asc:       bool,
    limit:          usize,
    alert_on_entry: bool,
}

impl Default for ScreenMeta {
    fn default() -> Self {
        Self {
            name: String::new(),
            tf: "1d".to_string(),
            class: "stocks".to_string(),
            universe: "SP500".to_string(),
            rank_by: String::new(),
            rank_asc: false,
            limit: 50,
            alert_on_entry: false,
        }
    }
}

fn screen_meta() -> &'static Mutex<ScreenMeta> {
    static M: OnceLock<Mutex<ScreenMeta>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(ScreenMeta::default()))
}

// ── Main entry point ───────────────────────────────────────────────────────────

/// Render the BUILD tab body.
///
/// Called by T-SHELL's screener panel tab dispatcher.
/// `panel_w` = usable body width after the shell's padding is applied
/// (pre-resolved by the shell; same convention as `scanner_panel::draw_content`).
///
/// Mutations are currently applied to local statics.  Once T-SHELL wires
/// `ScreenBuilderState` into `ScreenPanelState`, the reducer for the
/// `Builder*` AppCommands listed in the module doc will replace these statics.
pub(crate) fn draw_build_tab(ui: &mut Ui, t: &Theme, panel_w: f32) {
    ensure_catalog_fetched();

    let dsl_mode = dsl_mode_flag().load(Ordering::Relaxed);

    // ── Mode toggle strip ──────────────────────────────────────────────────
    ui.horizontal(|ui| {
        let vis_r = Button::new("VISUAL")
            .variant(if !dsl_mode { Variant::Primary } else { Variant::Ghost })
            .size(KitSize::Sm)
            .show(ui, t);
        if vis_r.clicked() && dsl_mode {
            // DSL → Visual: try to print the current condition JSON to
            // canonical DSL so the text area stays in sync, then flip mode.
            // fetch_print is blocking; acceptable on a user gesture.
            if let Ok(pr) = parse_result().lock() {
                if let Some(ref json) = pr.condition {
                    // Update dsl_text_buf with canonical form if the print
                    // endpoint is reachable.
                    if let Some(canonical) = fetch_print(json) {
                        if let Ok(mut buf) = dsl_text_buf().lock() {
                            *buf = canonical;
                        }
                    }
                }
            }
            dsl_mode_flag().store(false, Ordering::Relaxed);
        }

        let dsl_r = Button::new("DSL")
            .variant(if dsl_mode { Variant::Primary } else { Variant::Ghost })
            .size(KitSize::Sm)
            .show(ui, t);
        if dsl_r.clicked() && !dsl_mode {
            // Visual → DSL: serialise the current tree to a DSL string.
            // Full round-trip serialisation is T-SHELL's responsibility;
            // here we trigger a parse of whatever is in the DSL buffer
            // so errors are up-to-date when the user sees the text editor.
            let text = dsl_text_buf().lock().ok()
                .map(|g| g.clone()).unwrap_or_default();
            if !text.is_empty() { trigger_parse(text); }
            dsl_mode_flag().store(true, Ordering::Relaxed);
        }
    });
    ui.add_space(gap_xs());

    // ── Metadata header ────────────────────────────────────────────────────
    draw_meta_header(ui, t, panel_w);
    ui.add_space(gap_sm());

    separator(ui, t.toolbar_border);
    ui.add_space(gap_xs());

    // ── Mode-specific body ─────────────────────────────────────────────────
    if dsl_mode {
        draw_dsl_body(ui, t, panel_w);
    } else {
        draw_visual_body(ui, t, panel_w);
    }

    // ── Footer ────────────────────────────────────────────────────────────
    ui.add_space(gap_md());
    draw_footer(ui, t);
}

// ── Metadata header ────────────────────────────────────────────────────────────

fn draw_meta_header(ui: &mut Ui, t: &Theme, panel_w: f32) {
    let mut m = screen_meta().lock().unwrap();

    // Name.
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(
            TextStyle::Caption.as_rich_cascading("Name", t.dim),
        ));
        ui.add_space(gap_xs());
        Input::new(&mut m.name)
            .placeholder("Unnamed Screen")
            .min_width(panel_w - 56.0)
            .size(KitSize::Xs)
            .show(ui, t);
    });

    ui.add_space(gap_2xs());

    // TF / Class / Universe row — compact combo boxes.
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(TextStyle::Caption.as_rich_cascading("TF", t.dim)));
        egui::ComboBox::from_id_salt("sb_tf")
            .selected_text(egui::RichText::new(&m.tf).monospace().size(font_xs()))
            .width(54.0)
            .show_ui(ui, |ui| {
                for &tf in BUILTIN_TIMEFRAMES {
                    ui.selectable_value(&mut m.tf, tf.to_string(),
                        egui::RichText::new(tf).monospace().size(font_xs()));
                }
            });

        ui.add_space(gap_xs());
        ui.add(egui::Label::new(TextStyle::Caption.as_rich_cascading("Class", t.dim)));
        egui::ComboBox::from_id_salt("sb_class")
            .selected_text(egui::RichText::new(&m.class).size(font_xs()))
            .width(70.0)
            .show_ui(ui, |ui| {
                for &cls in BUILTIN_CLASSES {
                    ui.selectable_value(&mut m.class, cls.to_string(),
                        egui::RichText::new(cls).size(font_xs()));
                }
            });

        ui.add_space(gap_xs());
        ui.add(egui::Label::new(TextStyle::Caption.as_rich_cascading("Univ", t.dim)));
        let universes = ["SP500", "QQQ100", "Dow30", "$universe", "All"];
        egui::ComboBox::from_id_salt("sb_univ")
            .selected_text(egui::RichText::new(&m.universe).size(font_xs()))
            .width(80.0)
            .show_ui(ui, |ui| {
                for &u in &universes {
                    ui.selectable_value(&mut m.universe, u.to_string(),
                        egui::RichText::new(u).size(font_xs()));
                }
            });
    });
}

// ── Visual condition builder ───────────────────────────────────────────────────

fn draw_visual_body(ui: &mut Ui, t: &Theme, panel_w: f32) {
    let all_ops = all_operands();
    let catalog_loaded = catalog_state().lock().ok()
        .map(|g| g.fetched).unwrap_or(false);

    // ── CONDITIONS section ─────────────────────────────────────────────────
    let mut conds_expanded = true; // T-SHELL owns this bool; local stub for now
    PanelSection::new("CONDITIONS — ALL OF")
        .collapsible(&mut conds_expanded)
        .show(ui, t, |ui, t| {
            let mut nodes     = node_tree().lock().unwrap();
            let mut lefts     = left_op_buf().lock().unwrap();
            let mut rights    = right_op_buf().lock().unwrap();
            render_node_list(ui, t, panel_w, &mut nodes, &all_ops, &mut lefts, &mut rights, 0);
            drop(nodes); drop(lefts); drop(rights);

            ui.add_space(gap_xs());
            ui.horizontal(|ui| {
                if Button::new("+ AND")
                    .variant(Variant::Ghost).size(KitSize::Xs)
                    .show(ui, t).clicked()
                {
                    node_tree().lock().unwrap().push(ConditionNode::leaf(next_id()));
                }
                if Button::new("+ OR group")
                    .variant(Variant::Ghost).size(KitSize::Xs)
                    .show(ui, t).clicked()
                {
                    node_tree().lock().unwrap().push(ConditionNode::group(next_id(), GroupOp::Or));
                }
            });

            if !catalog_loaded {
                ui.add_space(gap_2xs());
                ui.horizontal(|ui| {
                    Spinner::new().size(KitSize::Sm).show(ui, t);
                    ui.add(egui::Label::new(
                        TextStyle::Caption.as_rich_cascading("Loading catalog…", t.dim),
                    ));
                });
            }
        });

    ui.add_space(gap_sm());

    // ── RANK BY section ────────────────────────────────────────────────────
    let mut rank_expanded = true;
    PanelSection::new("RANK BY")
        .collapsible(&mut rank_expanded)
        .show(ui, t, |ui, t| {
            let mut m = screen_meta().lock().unwrap();
            ui.horizontal(|ui| {
                SearchInput::new(&mut m.rank_by)
                    .placeholder("Operand (e.g. rsi14)")
                    .width(panel_w - 90.0)
                    .size(KitSize::Xs)
                    .show(ui, t);

                let dir_label = if m.rank_asc { "ASC" } else { "DESC" };
                if Button::new(dir_label)
                    .variant(Variant::Ghost).size(KitSize::Xs)
                    .show(ui, t).clicked()
                {
                    m.rank_asc = !m.rank_asc;
                }
            });
            // Fuzzy suggestion list (no popup, just inline clickable entries).
            if !m.rank_by.is_empty() {
                let q = m.rank_by.to_lowercase();
                let matches: Vec<_> = all_ops.iter()
                    .filter(|(k, l)| k.to_lowercase().contains(&q) || l.to_lowercase().contains(&q))
                    .take(5)
                    .collect();
                for (k, l) in matches {
                    let r = ui.add(egui::Label::new(
                        TextStyle::Caption.as_rich_cascading(l.as_str(), t.accent),
                    ).sense(egui::Sense::click()));
                    if r.clicked() { m.rank_by = k.clone(); }
                }
            }
        });

    ui.add_space(gap_sm());

    // ── OPTIONS section ────────────────────────────────────────────────────
    let mut opts_expanded = true;
    PanelSection::new("OPTIONS")
        .collapsible(&mut opts_expanded)
        .show(ui, t, |ui, t| {
            let mut m = screen_meta().lock().unwrap();
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(
                    TextStyle::Caption.as_rich_cascading("Limit", t.dim),
                ));
                let mut limit_str = m.limit.to_string();
                if Input::new(&mut limit_str)
                    .min_width(50.0).size(KitSize::Xs)
                    .show(ui, t).response.changed()
                {
                    if let Ok(n) = limit_str.parse::<usize>() {
                        m.limit = n.clamp(1, 1000);
                    }
                }
                ui.add_space(gap_md());
                ui.add(egui::Label::new(
                    TextStyle::Caption.as_rich_cascading("Alert on entry", t.dim),
                ));
                Checkbox::new(&mut m.alert_on_entry).show(ui, t);
            });
        });
}

/// Render a slice of `ConditionNode`s, then prune any that were removed.
fn render_node_list(
    ui:     &mut Ui,
    t:      &Theme,
    pw:     f32,
    nodes:  &mut Vec<ConditionNode>,
    ops:    &[(String, String)],
    lefts:  &mut std::collections::HashMap<u32, String>,
    rights: &mut std::collections::HashMap<u32, String>,
    depth:  usize,
) {
    let indent = depth as f32 * (gap_md() + gap_xs());
    let mut to_remove: Vec<u32> = Vec::new();

    for node in nodes.iter_mut() {
        let nid = node.id;
        match node.group_op {
            // ── Leaf predicate ─────────────────────────────────────────────
            None => {
                if indent > 0.0 { ui.add_space(indent); }
                let rm = render_predicate_card(ui, t, pw - indent, node, ops, lefts, rights);
                if rm { to_remove.push(nid); }
            }
            // ── AND / OR group ─────────────────────────────────────────────
            Some(grp) => {
                let grp_label = match grp {
                    GroupOp::And => format!("ALL OF  ({} conditions)", node.children.len()),
                    GroupOp::Or  => format!("ANY OF  ({} conditions)", node.children.len()),
                };
                let mut exp = node.expanded;
                let id_salt = format!("sb_grp_{}", nid);
                let toggle_grp = std::cell::Cell::new(false);
                let remove_grp = std::cell::Cell::new(false);
                PanelSubSection::new(&id_salt, &grp_label)
                    .count(node.children.len())
                    .expanded(&mut exp)
                    .header_trailing(|ui, t| {
                        let label = if grp == GroupOp::And { "ANY" } else { "ALL" };
                        if Button::new(label).variant(Variant::Ghost).size(KitSize::Xs)
                            .show(ui, t).clicked()
                        {
                            toggle_grp.set(true);
                        }
                        if Button::icon(Icon::X).variant(Variant::Ghost).size(KitSize::Xs)
                            .show(ui, t).clicked()
                        {
                            remove_grp.set(true);
                        }
                    })
                    .show(ui, t, |ui, t| {
                        render_node_list(ui, t, pw - indent, &mut node.children,
                            ops, lefts, rights, depth + 1);
                        ui.add_space(gap_2xs());
                        if Button::new("+ condition").variant(Variant::Ghost).size(KitSize::Xs)
                            .show(ui, t).clicked()
                        {
                            node.children.push(ConditionNode::leaf(next_id()));
                        }
                    });
                node.expanded = exp;
                if toggle_grp.get() {
                    node.group_op = Some(if grp == GroupOp::And {
                        GroupOp::Or } else { GroupOp::And });
                }
                if remove_grp.get() { to_remove.push(nid); }
            }
        }
    }

    nodes.retain(|n| !to_remove.contains(&n.id));
}

/// Render a single leaf predicate as a `PanelCard` with three operand columns.
/// Returns `true` when the X button was clicked (caller should remove this node).
fn render_predicate_card(
    ui:     &mut Ui,
    t:      &Theme,
    pw:     f32,
    node:   &mut ConditionNode,
    ops:    &[(String, String)],
    lefts:  &mut std::collections::HashMap<u32, String>,
    rights: &mut std::collections::HashMap<u32, String>,
) -> bool {
    let mut remove = false;
    let card_w = pw - gap_sm() * 2.0;

    PanelCard::new().padding(gap_sm()).show(ui, t, |ui, t| {
        ui.set_width(card_w);
        ui.horizontal(|ui| {
            // Column widths: left_op | cmp | right_op | X button.
            let cmp_w  = 100.0_f32;
            let x_w    = 22.0_f32;
            let op_w   = ((card_w - cmp_w - x_w - gap_xs() * 3.0) / 2.0).max(60.0);

            // ── Left operand ───────────────────────────────────────────────
            let left_buf = lefts.entry(node.id)
                .or_insert_with(|| node.left_op.clone());
            let lresp = SearchInput::new(left_buf)
                .placeholder("Operand…")
                .width(op_w)
                .size(KitSize::Xs)
                .show(ui, t);
            if lresp.response.changed() {
                node.left_op = left_buf.clone();
            }
            // Inline suggestion rows beneath the card (shown when focused).
            if lresp.has_focus && !left_buf.is_empty() {
                let q = left_buf.to_lowercase();
                let hits: Vec<_> = ops.iter()
                    .filter(|(k, l)| k.to_lowercase().contains(&q)
                             || l.to_lowercase().contains(&q))
                    .take(6).collect();
                for (k, l) in &hits {
                    let sr = ui.add(egui::Label::new(
                        TextStyle::Caption.as_rich_cascading(&format!("  {}", l), t.accent),
                    ).sense(egui::Sense::click()));
                    if sr.clicked() {
                        node.left_op = k.to_string();
                        *left_buf    = l.to_string();
                    }
                }
            }

            // ── Comparator ────────────────────────────────────────────────
            egui::ComboBox::from_id_salt(egui::Id::new(("sb_cmp", node.id)))
                .selected_text(
                    egui::RichText::new(node.comparator.as_str())
                        .monospace().size(font_xs()).color(t.accent),
                )
                .width(cmp_w)
                .show_ui(ui, |ui| {
                    for &c in BUILTIN_COMPARATORS {
                        ui.selectable_value(
                            &mut node.comparator, c.to_string(),
                            egui::RichText::new(c).monospace().size(font_xs()),
                        );
                    }
                });

            // ── Right operand ─────────────────────────────────────────────
            let right_buf = rights.entry(node.id)
                .or_insert_with(|| node.right_op.clone());
            let rresp = Input::new(right_buf)
                .placeholder("Value or operand…")
                .min_width(op_w)
                .size(KitSize::Xs)
                .show(ui, t);
            if rresp.response.changed() {
                node.right_op = right_buf.clone();
            }

            // ── Remove ────────────────────────────────────────────────────
            ui.add_space(gap_xs());
            if Button::icon(Icon::X)
                .variant(Variant::Ghost).size(KitSize::Xs)
                .show(ui, t).clicked()
            {
                remove = true;
            }
        });
    });

    remove
}

// ── DSL text mode ─────────────────────────────────────────────────────────────

fn draw_dsl_body(ui: &mut Ui, t: &Theme, panel_w: f32) {
    let mut text = dsl_text_buf().lock().unwrap();
    let text_resp = ui.add(
        TextEdit::multiline(&mut *text)
            .desired_width(panel_w - gap_md() * 2.0)
            .desired_rows(8)
            .font(egui::FontId::monospace(font_xs()))
            .text_color(t.text)
            .hint_text("rsi14 < 30 and rvol > 1.5 and signal(\"combined.score\") > 0.6"),
    );
    if text_resp.changed() {
        // Clone before releasing the lock so the background thread sees a
        // stable snapshot.  Fire parse; spinner appears next frame.
        let snap = text.clone();
        drop(text); // release before spawn
        trigger_parse(snap);
        return; // re-enter next frame
    }
    drop(text);

    // ── Parse status indicator ─────────────────────────────────────────────
    let pr = parse_result().lock().unwrap().clone();
    ui.add_space(gap_xs());
    if pr.pending {
        ui.horizontal(|ui| {
            Spinner::new().size(KitSize::Sm).show(ui, t);
            ui.add(egui::Label::new(
                TextStyle::Caption.as_rich_cascading("Parsing DSL…", t.dim),
            ));
        });
    } else if pr.errors.is_empty() {
        if pr.condition.is_some() {
            ui.add(egui::Label::new(
                TextStyle::Caption.as_rich_cascading("Valid", t.bull),
            ));
        }
    } else {
        for err in &pr.errors {
            ui.horizontal(|ui| {
                // Warn tag for the error message.
                Tag::new(err.message.as_str()).tone(TagTone::Warn).show(ui, t);
                // Span annotation in dim mono.
                if err.span_end > err.span_start {
                    ui.add(egui::Label::new(
                        egui::RichText::new(
                            format!(" [{}..{}]", err.span_start, err.span_end),
                        )
                        .monospace().size(font_2xs()).color(t.dim),
                    ));
                }
            });
        }
    }

    // ── Condition JSON preview (collapsed) ────────────────────────────────
    if let Some(ref json) = pr.condition {
        ui.add_space(gap_xs());
        PanelSubSection::new("sb_cond_json", "CONDITION JSON")
            .persist_key("sb_cond_json")
            .show(ui, t, |ui, _t| {
                ui.add(egui::Label::new(
                    egui::RichText::new(json.as_str())
                        .monospace().size(font_2xs()).color(t.dim),
                ));
            });
    }
}

// ── Footer action bar ─────────────────────────────────────────────────────────

fn draw_footer(ui: &mut Ui, t: &Theme) {
    ui.separator();
    ui.add_space(gap_xs());
    ui.horizontal(|ui| {
        // "Run Now" — T-SHELL handles via AppCommand::BuilderCommit then RunScreen.
        // We record the click; T-SHELL reads it from the screener_open state toggle.
        if Button::new("Run Now")
            .variant(Variant::Primary).size(KitSize::Sm)
            .show(ui, t).clicked()
        {
            // STUB: T-SHELL must wire AppCommand::BuilderCommit here.
            // When wired: commands::push(AppCommand::BuilderCommit);
            let _ = ();
        }

        if Button::new("Save Screen")
            .variant(Variant::Secondary).size(KitSize::Sm)
            .show(ui, t).clicked()
        {
            // STUB: T-SHELL must wire AppCommand::BuilderSave here.
            let _ = ();
        }

        if Button::new("Cancel")
            .variant(Variant::Ghost).size(KitSize::Sm)
            .show(ui, t).clicked()
        {
            reset_builder();
        }
    });
}

/// Clear all local builder state (used by Cancel).
fn reset_builder() {
    *node_tree().lock().unwrap()     = vec![ConditionNode::leaf(next_id())];
    *screen_meta().lock().unwrap()   = ScreenMeta::default();
    *dsl_text_buf().lock().unwrap()  = String::new();
    if let Ok(mut g) = parse_result().lock() { *g = ParseResult::default(); }
    dsl_mode_flag().store(false, Ordering::Relaxed);
    if let Ok(mut g) = left_op_buf().lock()  { g.clear(); }
    if let Ok(mut g) = right_op_buf().lock() { g.clear(); }
}

