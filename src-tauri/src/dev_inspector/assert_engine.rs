//! Logical and layout assertion evaluation.
//!
//! Pure data → result. Reads `DevSharedState`, evaluates assertions, returns
//! `AssertionResult`. No mutation, no IO.

use serde_json::Value;
use crate::dev_inspector::{DevSharedState, SerRect, WidgetRecord};

// ─── Result types ─────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct AssertionResult {
    pub index:  usize,
    pub kind:   String,
    pub pass:   bool,
    pub detail: String,
}

#[derive(Debug, serde::Serialize)]
pub struct AssertionReport {
    pub passed:  usize,
    pub failed:  usize,
    pub results: Vec<AssertionResult>,
}

// ─── Logical assertions ───────────────────────────────────────────────────────

/// Evaluate a list of logical assertions against the shared state.
pub fn evaluate(assertions: &[Value], state: &DevSharedState) -> AssertionReport {
    let mut results = Vec::new();
    for (i, a) in assertions.iter().enumerate() {
        let (kind, pass, detail) = eval_one(a, state);
        results.push(AssertionResult { index: i, kind, pass, detail });
    }
    let passed = results.iter().filter(|r| r.pass).count();
    let failed = results.len() - passed;
    AssertionReport { passed, failed, results }
}

fn eval_one(assertion: &Value, state: &DevSharedState) -> (String, bool, String) {
    if let Some(obj) = assertion.as_object() {
        if let Some((key, val)) = obj.iter().next() {
            return dispatch(key.as_str(), val, assertion, state);
        }
    }
    ("Unknown".into(), false, "malformed assertion object".into())
}

fn dispatch(
    key: &str,
    val: &Value,
    full: &Value,
    state: &DevSharedState,
) -> (String, bool, String) {
    match key {
        // ── State field ──────────────────────────────────────────────────────
        "state_field_equals" => {
            let path   = val["path"].as_str().unwrap_or("");
            let expect = &val["equals"];
            let actual = json_path(&state.app_state, path);
            let pass = actual == *expect;
            (
                "StateFieldEquals".into(),
                pass,
                if pass {
                    format!("state.{path} == {expect}")
                } else {
                    format!("state.{path}: expected {expect}, got {actual}")
                },
            )
        }

        // ── Dialog ───────────────────────────────────────────────────────────
        "dialog_open" => {
            let name = val["dialog"].as_str().unwrap_or("");
            let pass = state.open_dialogs.iter().any(|d| d == name || d.starts_with(&format!("{name}.")));
            ("DialogOpen".into(), pass,
             if pass { format!("dialog '{name}' is open") }
             else    { format!("dialog '{name}' is not open") })
        }
        "dialog_closed" => {
            let name = val["dialog"].as_str().unwrap_or("");
            let open = state.open_dialogs.iter().any(|d| d == name || d.starts_with(&format!("{name}.")));
            ("DialogClosed".into(), !open,
             if !open { format!("dialog '{name}' is closed") }
             else     { format!("dialog '{name}' is open") })
        }
        "no_open_dialogs" => {
            let pass = state.open_dialogs.is_empty();
            ("NoOpenDialogs".into(), pass,
             if pass { "no open dialogs".into() }
             else    { format!("open dialogs: {:?}", state.open_dialogs) })
        }

        // ── Widget ───────────────────────────────────────────────────────────
        "widget_exists" => {
            let role  = val["role"].as_str();
            let label = val["label"].as_str();
            let id    = val["id"].as_str();
            let found = state.widget_tree.iter().find(|w| {
                let role_ok  = role.map(|r| w.role == r).unwrap_or(true);
                let label_ok = label.map(|l| w.label == l).unwrap_or(true);
                let id_ok    = id.map(|i| w.id == i).unwrap_or(true);
                role_ok && label_ok && id_ok
            });
            ("WidgetExists".into(), found.is_some(),
             match found {
                Some(w) => format!("found {} '{}' (id={})", w.role, w.label, w.id),
                None    => format!("no widget matching role={role:?} label={label:?} id={id:?}"),
             })
        }
        "widget_state" => {
            let id    = val["id"].as_str().unwrap_or("");
            let field = val["field"].as_str().unwrap_or("");
            let expect = &val["equals"];
            let widget = state.widget_tree.iter().find(|w| w.id == id);
            let Some(w) = widget else {
                return ("WidgetState".into(), false, format!("widget '{id}' not found"));
            };
            let actual = widget_field(w, field);
            let pass = actual == *expect;
            ("WidgetState".into(), pass,
             if pass { format!("widget '{id}'.{field} == {expect}") }
             else    { format!("widget '{id}'.{field}: expected {expect}, got {actual}") })
        }

        // ── FPS ──────────────────────────────────────────────────────────────
        "fps_above" => {
            let min = val["min"].as_f64().unwrap_or(30.0) as f32;
            let pass = state.fps >= min;
            ("FpsAbove".into(), pass,
             if pass { format!("{:.1} fps >= {min}", state.fps) }
             else    { format!("{:.1} fps < min {min}", state.fps) })
        }

        // ── Domain: terminal ─────────────────────────────────────────────────
        "active_symbol_equals" => {
            let sym    = val["symbol"].as_str().unwrap_or("");
            let actual = state.app_state["active_symbol"].as_str().unwrap_or("");
            let pass   = actual.eq_ignore_ascii_case(sym);
            ("ActiveSymbolEquals".into(), pass,
             if pass { format!("active symbol is '{sym}'") }
             else    { format!("active symbol: expected '{sym}', got '{actual}'") })
        }
        "active_timeframe_equals" => {
            let tf     = val["tf"].as_str().unwrap_or("");
            let actual = state.app_state["active_timeframe"].as_str().unwrap_or("");
            let pass   = actual == tf;
            ("ActiveTimeframeEquals".into(), pass,
             if pass { format!("active tf is '{tf}'") }
             else    { format!("active tf: expected '{tf}', got '{actual}'") })
        }
        "pane_count_equals" => {
            let expect = val["count"].as_u64().unwrap_or(0) as usize;
            let actual = state.app_state["pane_count"].as_u64().unwrap_or(0) as usize;
            let pass   = actual == expect;
            ("PaneCountEquals".into(), pass,
             if pass { format!("pane count == {expect}") }
             else    { format!("pane count: expected {expect}, got {actual}") })
        }
        "has_bars" => {
            let min = val["min"].as_u64().unwrap_or(1) as usize;
            let actual = state.app_state["bar_count"].as_u64().unwrap_or(0) as usize;
            let pass = actual >= min;
            ("HasBars".into(), pass,
             if pass { format!("{actual} bars (>= {min})") }
             else    { format!("{actual} bars < min {min}") })
        }
        "no_active_violations" => {
            let pass = state.active_violations.is_empty();
            ("NoActiveViolations".into(), pass,
             if pass { "no design contract violations".into() }
             else {
                 let vs: Vec<_> = state.active_violations.iter()
                     .map(|v| format!("{}: {}", v.widget_id, v.constraint))
                     .collect();
                 format!("{} violation(s): {}", vs.len(), vs.join(", "))
             })
        }

        // ── Logical combinators ───────────────────────────────────────────────
        "not" => {
            let inner = &val["assertion"];
            let (kind, pass, detail) = eval_one(inner, state);
            ("Not".into(), !pass, format!("NOT ({kind}: {detail})"))
        }
        "all_of" => {
            let items = val["assertions"].as_array().cloned().unwrap_or_default();
            let results: Vec<_> = items.iter().map(|a| eval_one(a, state)).collect();
            let all_pass = results.iter().all(|(_, p, _)| *p);
            let detail = results.iter()
                .map(|(k, p, d)| format!("[{} {}] {}", if *p {"✓"} else {"✗"}, k, d))
                .collect::<Vec<_>>().join("; ");
            ("AllOf".into(), all_pass, detail)
        }
        "any_of" => {
            let items = val["assertions"].as_array().cloned().unwrap_or_default();
            let results: Vec<_> = items.iter().map(|a| eval_one(a, state)).collect();
            let any_pass = results.iter().any(|(_, p, _)| *p);
            let detail = results.iter()
                .map(|(k, p, d)| format!("[{} {}] {}", if *p {"✓"} else {"✗"}, k, d))
                .collect::<Vec<_>>().join("; ");
            ("AnyOf".into(), any_pass, detail)
        }

        _ => ("Unknown".into(), false, format!("unknown assertion kind: '{key}'")),
    }
}

/// Navigate a dot-path like `"panes.0.symbol"` into a serde_json::Value.
fn json_path(root: &Value, path: &str) -> Value {
    let mut cur = root;
    let placeholder = Value::Null;
    for segment in path.split('.') {
        cur = if let Some(idx) = segment.parse::<usize>().ok() {
            cur.get(idx).unwrap_or(&placeholder)
        } else {
            cur.get(segment).unwrap_or(&placeholder)
        };
    }
    cur.clone()
}

fn widget_field(w: &WidgetRecord, field: &str) -> Value {
    match field {
        "id"       => Value::String(w.id.clone()),
        "role"     => Value::String(w.role.clone()),
        "label"    => Value::String(w.label.clone()),
        "value"    => w.value.as_ref().map(|v| Value::String(v.clone())).unwrap_or(Value::Null),
        "enabled"  => Value::Bool(w.enabled),
        "focused"  => Value::Bool(w.focused),
        "hovered"  => Value::Bool(w.hovered),
        "is_clipped" => Value::Bool(w.is_clipped),
        _ => Value::Null,
    }
}

// ─── Layout assertions ────────────────────────────────────────────────────────

/// Evaluate a list of geometric assertions against the widget tree.
pub fn evaluate_layout(assertions: &[Value], widgets: &[WidgetRecord]) -> AssertionReport {
    let mut results = Vec::new();
    for (i, a) in assertions.iter().enumerate() {
        let (kind, pass, detail) = eval_layout_one(a, widgets);
        results.push(AssertionResult { index: i, kind, pass, detail });
    }
    let passed = results.iter().filter(|r| r.pass).count();
    let failed = results.len() - passed;
    AssertionReport { passed, failed, results }
}

fn find_widget<'a>(widgets: &'a [WidgetRecord], id: &str) -> Option<&'a WidgetRecord> {
    widgets.iter().find(|w| w.id == id)
}

fn eval_layout_one(assertion: &Value, widgets: &[WidgetRecord]) -> (String, bool, String) {
    let Some(obj) = assertion.as_object() else {
        return ("Unknown".into(), false, "malformed layout assertion".into());
    };
    let Some((key, val)) = obj.iter().next() else {
        return ("Unknown".into(), false, "empty assertion".into());
    };

    match key.as_str() {
        "aligned" => {
            let ids: Vec<&str> = val["widgets"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let edge = val["edge"].as_str().unwrap_or("top");
            let tol  = val["tolerance_px"].as_f64().unwrap_or(2.0) as f32;
            let rects: Vec<_> = ids.iter()
                .filter_map(|id| find_widget(widgets, id).map(|w| (id, &w.rect)))
                .collect();
            if rects.len() < 2 {
                return ("Aligned".into(), false,
                    format!("fewer than 2 widgets found (got {})", rects.len()));
            }
            let base = match edge {
                "top"    => rects[0].1.y,
                "bottom" => rects[0].1.y + rects[0].1.h,
                "left"   => rects[0].1.x,
                "right"  => rects[0].1.x + rects[0].1.w,
                "center_y" => rects[0].1.y + rects[0].1.h / 2.0,
                "center_x" => rects[0].1.x + rects[0].1.w / 2.0,
                _ => rects[0].1.y,
            };
            let misaligned: Vec<_> = rects.iter().filter(|(_, r)| {
                let v = match edge {
                    "top"      => r.y,
                    "bottom"   => r.y + r.h,
                    "left"     => r.x,
                    "right"    => r.x + r.w,
                    "center_y" => r.y + r.h / 2.0,
                    "center_x" => r.x + r.w / 2.0,
                    _ => r.y,
                };
                (v - base).abs() > tol
            }).map(|(id, _)| **id).collect();
            let pass = misaligned.is_empty();
            ("Aligned".into(), pass,
             if pass { format!("{} widgets aligned on {edge}", rects.len()) }
             else    { format!("misaligned: {misaligned:?}") })
        }

        "gap_between" => {
            let a_id = val["a"].as_str().unwrap_or("");
            let b_id = val["b"].as_str().unwrap_or("");
            let min_px = val["min_px"].as_f64().unwrap_or(0.0) as f32;
            let max_px = val["max_px"].as_f64().unwrap_or(f64::MAX) as f32;
            let (wa, wb) = match (find_widget(widgets, a_id), find_widget(widgets, b_id)) {
                (Some(a), Some(b)) => (a, b),
                _ => return ("GapBetween".into(), false, format!("widget '{a_id}' or '{b_id}' not found")),
            };
            let gap = (wb.rect.x - (wa.rect.x + wa.rect.w)).abs()
                .min((wb.rect.y - (wa.rect.y + wa.rect.h)).abs());
            let pass = gap >= min_px && gap <= max_px;
            ("GapBetween".into(), pass,
             if pass { format!("gap {gap:.1}px in [{min_px},{max_px}]") }
             else    { format!("gap {gap:.1}px outside [{min_px},{max_px}]") })
        }

        "contained_in" => {
            let widget_id    = val["widget"].as_str().unwrap_or("");
            let container_id = val["container"].as_str().unwrap_or("");
            let (w, c) = match (find_widget(widgets, widget_id), find_widget(widgets, container_id)) {
                (Some(a), Some(b)) => (a, b),
                _ => return ("ContainedIn".into(), false,
                    format!("'{widget_id}' or '{container_id}' not found")),
            };
            let pass = c.rect.contains(&w.rect);
            ("ContainedIn".into(), pass,
             if pass { format!("'{widget_id}' contained in '{container_id}'") }
             else    { format!("'{widget_id}' outside '{container_id}'") })
        }

        "not_clipped" => {
            let id = val["widget"].as_str().unwrap_or("");
            let w  = match find_widget(widgets, id) {
                Some(w) => w,
                None => return ("NotClipped".into(), false, format!("widget '{id}' not found")),
            };
            let pass = !w.is_clipped;
            ("NotClipped".into(), pass,
             if pass { format!("'{id}' not clipped") }
             else    { format!("'{id}' is clipped") })
        }

        "touch_target" => {
            let id      = val["widget"].as_str().unwrap_or("");
            let min_px  = val["min_size_px"].as_f64().unwrap_or(32.0) as f32;
            let w       = match find_widget(widgets, id) {
                Some(w) => w,
                None => return ("TouchTarget".into(), false, format!("widget '{id}' not found")),
            };
            let min_side = w.rect.min_side();
            let pass = min_side >= min_px;
            ("TouchTarget".into(), pass,
             if pass { format!("'{id}' min side {min_side:.1}px >= {min_px}px") }
             else    { format!("'{id}' min side {min_side:.1}px < {min_px}px") })
        }

        "order_ltr" => {
            let left_id  = val["left"].as_str().unwrap_or("");
            let right_id = val["right"].as_str().unwrap_or("");
            let (wl, wr) = match (find_widget(widgets, left_id), find_widget(widgets, right_id)) {
                (Some(a), Some(b)) => (a, b),
                _ => return ("OrderLtr".into(), false,
                    format!("'{left_id}' or '{right_id}' not found")),
            };
            let pass = wl.rect.x < wr.rect.x;
            ("OrderLtr".into(), pass,
             if pass { format!("'{left_id}' ({:.0}) left of '{right_id}' ({:.0})", wl.rect.x, wr.rect.x) }
             else    { format!("'{left_id}' ({:.0}) NOT left of '{right_id}' ({:.0})", wl.rect.x, wr.rect.x) })
        }

        "order_ttb" => {
            let top_id    = val["top"].as_str().unwrap_or("");
            let bottom_id = val["bottom"].as_str().unwrap_or("");
            let (wt, wb) = match (find_widget(widgets, top_id), find_widget(widgets, bottom_id)) {
                (Some(a), Some(b)) => (a, b),
                _ => return ("OrderTtb".into(), false,
                    format!("'{top_id}' or '{bottom_id}' not found")),
            };
            let pass = wt.rect.y < wb.rect.y;
            ("OrderTtb".into(), pass,
             if pass { format!("'{top_id}' above '{bottom_id}'") }
             else    { format!("'{top_id}' NOT above '{bottom_id}'") })
        }

        "z_order" => {
            let above_id = val["above"].as_str().unwrap_or("");
            let below_id = val["below"].as_str().unwrap_or("");
            let (wa, wb) = match (find_widget(widgets, above_id), find_widget(widgets, below_id)) {
                (Some(a), Some(b)) => (a, b),
                _ => return ("ZOrder".into(), false,
                    format!("'{above_id}' or '{below_id}' not found")),
            };
            let pass = wa.layer > wb.layer;
            ("ZOrder".into(), pass,
             if pass { format!("'{above_id}' (layer {}) above '{below_id}' (layer {})", wa.layer, wb.layer) }
             else    { format!("'{above_id}' NOT above '{below_id}' (layers {} vs {})", wa.layer, wb.layer) })
        }

        "no_unintended_overlap" => {
            let layer = val["layer"].as_u64().unwrap_or(0) as u8;
            let layer_widgets: Vec<_> = widgets.iter()
                .filter(|w| w.layer == layer && w.rect.area() > 0.0)
                .collect();
            let mut overlapping = Vec::new();
            for i in 0..layer_widgets.len() {
                for j in (i+1)..layer_widgets.len() {
                    let a = &layer_widgets[i].rect;
                    let b = &layer_widgets[j].rect;
                    if a.x < b.x + b.w && a.x + a.w > b.x
                    && a.y < b.y + b.h && a.y + a.h > b.y {
                        overlapping.push(format!("{}↔{}",
                            layer_widgets[i].id, layer_widgets[j].id));
                    }
                }
            }
            let pass = overlapping.is_empty();
            ("NoUnintendedOverlap".into(), pass,
             if pass { format!("no overlaps on layer {layer}") }
             else    { format!("overlaps: {}", overlapping.join(", ")) })
        }

        _ => ("Unknown".into(), false, format!("unknown layout assertion: '{key}'")),
    }
}
