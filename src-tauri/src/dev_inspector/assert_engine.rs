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
        // ── Dialog ───────────────────────────────────────────────────────────
        "dialog_open" => {
            // Accepts: {"dialog_open": {"dialog": "settings"}} or {"dialog_open": "settings"}
            let name = val.as_str()
                .or_else(|| val["dialog"].as_str())
                .unwrap_or("");
            let pass = state.open_dialogs.iter().any(|d| d == name || d.starts_with(&format!("{name}.")));
            ("DialogOpen".into(), pass,
             if pass { format!("dialog '{name}' is open") }
             else    { format!("dialog '{name}' is not open (open: {:?})", state.open_dialogs) })
        }
        "dialog_closed" => {
            let name = val.as_str()
                .or_else(|| val["dialog"].as_str())
                .unwrap_or("");
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
        "widget_value_equals" => {
            // {"widget_value_equals": {"id": "pane.0.symbol", "value": "AAPL"}}
            let id     = val["id"].as_str().unwrap_or("");
            let expect = val["value"].as_str().unwrap_or("");
            let w = state.widget_tree.iter().find(|w| w.id == id);
            match w {
                None => ("WidgetValueEquals".into(), false, format!("widget '{id}' not found")),
                Some(w) => {
                    let actual = w.value.as_deref().unwrap_or("");
                    let pass = actual == expect;
                    ("WidgetValueEquals".into(), pass,
                     if pass { format!("widget '{id}' value == '{expect}'") }
                     else    { format!("widget '{id}' value: expected '{expect}', got '{actual}'") })
                }
            }
        }

        // ── FPS ──────────────────────────────────────────────────────────────
        "fps_above" => {
            // Accepts scalar: {"fps_above": 30.0} or object: {"fps_above": {"min": 30.0}}
            let min = val.as_f64()
                .or_else(|| val["min"].as_f64())
                .unwrap_or(30.0) as f32;
            let pass = state.fps >= min;
            ("FpsAbove".into(), pass,
             if pass { format!("{:.1} fps >= {min}", state.fps) }
             else    { format!("{:.1} fps < min {min}", state.fps) })
        }

        // ── Domain: chart terminal ────────────────────────────────────────────
        "active_symbol_equals" => {
            // Accepts scalar: {"active_symbol_equals": "SPY"} or object form
            let sym    = val.as_str()
                .or_else(|| val["symbol"].as_str())
                .unwrap_or("");
            let actual = state.app_state["active_symbol"].as_str().unwrap_or("");
            let pass   = actual.eq_ignore_ascii_case(sym);
            ("ActiveSymbolEquals".into(), pass,
             if pass { format!("active symbol is '{sym}'") }
             else    { format!("active symbol: expected '{sym}', got '{actual}'") })
        }
        "active_timeframe_equals" => {
            let tf     = val.as_str()
                .or_else(|| val["tf"].as_str())
                .unwrap_or("");
            let actual = state.app_state["active_timeframe"].as_str().unwrap_or("");
            let pass   = actual == tf;
            ("ActiveTimeframeEquals".into(), pass,
             if pass { format!("active tf is '{tf}'") }
             else    { format!("active tf: expected '{tf}', got '{actual}'") })
        }
        "pane_count_equals" => {
            let expect = val.as_u64()
                .or_else(|| val["count"].as_u64())
                .unwrap_or(0) as usize;
            let actual = state.app_state["pane_count"].as_u64().unwrap_or(0) as usize;
            let pass   = actual == expect;
            ("PaneCountEquals".into(), pass,
             if pass { format!("pane count == {expect}") }
             else    { format!("pane count: expected {expect}, got {actual}") })
        }
        "active_pane_equals" => {
            let expect = val.as_u64()
                .or_else(|| val["pane"].as_u64())
                .unwrap_or(0) as usize;
            let actual = state.app_state["active_pane"].as_u64().unwrap_or(0) as usize;
            let pass   = actual == expect;
            ("ActivePaneEquals".into(), pass,
             if pass { format!("active pane == {expect}") }
             else    { format!("active pane: expected {expect}, got {actual}") })
        }
        "has_bars" => {
            let min = val.as_u64()
                .or_else(|| val["min"].as_u64())
                .unwrap_or(1) as usize;
            let actual = state.app_state["bar_count"].as_u64().unwrap_or(0) as usize;
            let pass = actual >= min;
            ("HasBars".into(), pass,
             if pass { format!("{actual} bars (>= {min})") }
             else    { format!("{actual} bars < min {min}") })
        }
        "bar_count_gte" => {
            // Alias for has_bars with cleaner name
            let min = val.as_u64()
                .or_else(|| val["min"].as_u64())
                .unwrap_or(1) as usize;
            let actual = state.app_state["bar_count"].as_u64().unwrap_or(0) as usize;
            let pass = actual >= min;
            ("BarCountGte".into(), pass,
             if pass { format!("{actual} bars (>= {min})") }
             else    { format!("{actual} bars < {min}") })
        }
        "indicator_count_equals" => {
            // {"indicator_count_equals": {"pane": 0, "count": 2}}
            // or scalar: {"indicator_count_equals": 2} (checks active pane)
            let (pane_idx, expect) = if let Some(n) = val.as_u64() {
                let ap = state.app_state["active_pane"].as_u64().unwrap_or(0) as usize;
                (ap, n as usize)
            } else {
                let p = val["pane"].as_u64().unwrap_or(0) as usize;
                let c = val["count"].as_u64().unwrap_or(0) as usize;
                (p, c)
            };
            let actual = state.app_state["panes"]
                .get(pane_idx)
                .and_then(|p| p["indicator_count"].as_u64())
                .unwrap_or(0) as usize;
            let pass = actual == expect;
            ("IndicatorCountEquals".into(), pass,
             if pass { format!("pane {pane_idx} indicator count == {expect}") }
             else    { format!("pane {pane_idx} indicator count: expected {expect}, got {actual}") })
        }
        "pane_type_equals" => {
            // {"pane_type_equals": {"pane": 0, "type": "Dashboard"}}
            let pane_idx = val["pane"].as_u64().unwrap_or(0) as usize;
            let expect   = val["type"].as_str()
                .or_else(|| val.as_str())
                .unwrap_or("Chart");
            let actual = state.app_state["panes"]
                .get(pane_idx)
                .and_then(|p| p["pane_type"].as_str())
                .unwrap_or("Chart");
            let pass = actual.to_lowercase() == expect.to_lowercase();
            ("PaneTypeEquals".into(), pass,
             if pass { format!("pane {pane_idx} type == '{expect}'") }
             else    { format!("pane {pane_idx} type: expected '{expect}', got '{actual}'") })
        }
        // ── Order-entry SAFETY invariant ──────────────────────────────────────
        // Proves the harness never submitted a live (or paper) order through the
        // OrderManager: paper_mode must be on AND the authoritative snapshot must
        // have zero orders in a submitted lifecycle state. Drop this into EVERY
        // order-entry scenario as belt-and-suspenders against accidental submission.
        "no_live_orders" => {
            let om = &state.app_state["order_manager"];
            let paper   = om["paper_mode"].as_bool().unwrap_or(false);
            let working = om["mgr_working_count"].as_u64().unwrap_or(0);
            let pass = paper && working == 0;
            ("NoLiveOrders".into(), pass,
             if pass { "paper_mode on, 0 orders in a submitted state".into() }
             else { format!("SAFETY: paper_mode={paper}, mgr_working_count={working} (expected true / 0)") })
        }

        // ── DOM ladder cross-field invariant ──────────────────────────────────
        // A populated ladder must have strictly-descending prices and a sane
        // spread (best ask >= best bid, both positive around the mid).
        // {"dom_spread_sane": {"pane": 0}}
        "dom_spread_sane" => {
            let pane_idx = val["pane"].as_u64().unwrap_or(0) as usize;
            let pane = &state.app_state["panes"][pane_idx];
            let levels = pane["dom_level_count"].as_u64().unwrap_or(0);
            let desc = pane["dom_prices_desc"].as_bool().unwrap_or(false);
            let bid = pane["dom_best_bid"].as_f64();
            let ask = pane["dom_best_ask"].as_f64();
            let spread_ok = match (bid, ask) {
                (Some(b), Some(a)) => a >= b && b > 0.0,
                _ => false,
            };
            let pass = levels > 0 && desc && spread_ok;
            ("DomSpreadSane".into(), pass,
             if pass { format!("pane {pane_idx} DOM ladder sane ({levels} levels, desc, bid={:?} ask={:?})", bid, ask) }
             else { format!("pane {pane_idx} DOM ladder BAD: levels={levels} desc={desc} bid={:?} ask={:?}", bid, ask) })
        }

        // ── BEHAVIORAL ORACLE: RRG quadrant classification is correct ─────────
        // For every sector, the classified quadrant must match the standard RRG
        // rule from its (rs_ratio, rs_momentum): >=100 on both axes.
        "rrg_quadrants_correct" => {
            let secs = state.app_state["rrg"]["sectors"].as_array().cloned().unwrap_or_default();
            let mut bad = vec![];
            for s in &secs {
                let r = s["rs_ratio"].as_f64().unwrap_or(0.0);
                let m = s["rs_momentum"].as_f64().unwrap_or(0.0);
                let want = match (r >= 100.0, m >= 100.0) {
                    (true, true)   => "LEADING",
                    (true, false)  => "WEAKENING",
                    (false, false) => "LAGGING",
                    (false, true)  => "IMPROVING",
                };
                let got = s["quadrant"].as_str().unwrap_or("");
                if got != want {
                    bad.push(format!("{} (r={r:.1},m={m:.1}): got {got}, want {want}",
                                     s["symbol"].as_str().unwrap_or("?")));
                }
            }
            let pass = !secs.is_empty() && bad.is_empty();
            ("RrgQuadrantsCorrect".into(), pass,
             if pass { format!("all {} RRG sectors classified correctly", secs.len()) }
             else if secs.is_empty() { "no RRG sectors present".into() }
             else { format!("{} misclassified: {}", bad.len(), bad.join("; ")) })
        }

        // ── BEHAVIORAL ORACLE: gamma wall/flip structure is sane ──────────────
        // The put wall (max negative GEX) must sit below the flip (zero-gamma),
        // which sits below the call wall (max positive GEX). {"gamma_structure_sane":{"pane":0}}
        "gamma_structure_sane" => {
            let pane_idx = val["pane"].as_u64().unwrap_or(0) as usize;
            let p = &state.app_state["panes"][pane_idx];
            let call = p["gamma_call_wall"].as_f64().unwrap_or(0.0);
            let put  = p["gamma_put_wall"].as_f64().unwrap_or(0.0);
            let zero = p["gamma_zero"].as_f64().unwrap_or(0.0);
            let n    = p["gamma_level_count"].as_u64().unwrap_or(0);
            let pass = n > 0 && put > 0.0 && call > 0.0 && put < call && put <= zero && zero <= call;
            ("GammaStructureSane".into(), pass,
             if pass { format!("pane {pane_idx} gamma structure ok (put {put:.2} <= flip {zero:.2} <= call {call:.2}, {n} levels)") }
             else { format!("pane {pane_idx} gamma structure BAD: put={put:.2} flip={zero:.2} call={call:.2} levels={n}") })
        }

        // ── BEHAVIORAL ORACLE: DOM ladder is a correct price ladder ───────────
        // Populated, strictly descending, with uniform tick spacing.
        // {"dom_ladder_correct":{"pane":0}}
        "dom_ladder_correct" => {
            let pane_idx = val["pane"].as_u64().unwrap_or(0) as usize;
            let p = &state.app_state["panes"][pane_idx];
            let n    = p["dom_level_count"].as_u64().unwrap_or(0);
            let desc = p["dom_prices_desc"].as_bool().unwrap_or(false);
            let unif = p["dom_uniform_spacing"].as_bool().unwrap_or(false);
            let pass = n >= 3 && desc && unif;
            ("DomLadderCorrect".into(), pass,
             if pass { format!("pane {pane_idx} DOM ladder correct ({n} levels, descending, uniform tick)") }
             else { format!("pane {pane_idx} DOM ladder BAD: levels={n} desc={desc} uniform={unif}") })
        }

        // ── BEHAVIORAL ORACLE: a seeded order round-trips correctly ───────────
        // The pane's order list must contain an order matching the given
        // side/price/qty. {"order_matches":{"pane":0,"side":"Buy","price":100.0,"qty":500}}
        "order_matches" => {
            let pane_idx = val["pane"].as_u64().unwrap_or(0) as usize;
            let side  = val["side"].as_str().unwrap_or("");
            let price = val["price"].as_f64().unwrap_or(f64::NAN);
            let qty   = val["qty"].as_u64().unwrap_or(0);
            let orders = state.app_state["panes"][pane_idx]["orders"].as_array().cloned().unwrap_or_default();
            let found = orders.iter().any(|o|
                o["side"].as_str().unwrap_or("").eq_ignore_ascii_case(side)
                && (o["price"].as_f64().unwrap_or(f64::NAN) - price).abs() < 1e-3
                && o["qty"].as_u64().unwrap_or(0) == qty);
            ("OrderMatches".into(), found,
             if found { format!("pane {pane_idx} has order {side} {qty}@{price}") }
             else { format!("pane {pane_idx} MISSING order {side} {qty}@{price}; have {} order(s)", orders.len()) })
        }

        // ── BEHAVIORAL ORACLE: scanner filter+sort matches an independent recompute
        // Re-derives the expected filtered/sorted/truncated result from the raw
        // pool + def[0] criteria and compares to the app's output symbol-for-symbol.
        "scanner_filter_correct" => {
            let raw = state.app_state["scanner"]["raw"].as_array().cloned().unwrap_or_default();
            let def = &state.app_state["scanner"]["def0"];
            let got: Vec<String> = state.app_state["scanner"]["filtered"].as_array().cloned().unwrap_or_default()
                .iter().map(|r| r["symbol"].as_str().unwrap_or("").to_string()).collect();
            let (mn, mx, mv, limit) = (
                def["min_change"].as_f64().unwrap_or(f64::MIN),
                def["max_change"].as_f64().unwrap_or(f64::MAX),
                def["min_volume"].as_f64().unwrap_or(0.0),
                def["limit"].as_u64().unwrap_or(usize::MAX as u64) as usize);
            let sort_by = def["sort_by"].as_str().unwrap_or("ChangeDesc");
            // This is a DELIBERATE second implementation, not a call into
            // `apply_scanner` — the whole value of the oracle is that it can
            // disagree with the app. It is written against the JSON dump, so it
            // stays independent of the Rust types.
            //
            // `change_pct` is `null` when the previous close has not arrived.
            // An unknown is excluded from any scan that constrains change (it
            // cannot be shown to satisfy `>= 0.0`, which is what made a symbol
            // with no data show up in Top Gainers AND Top Losers at once), and
            // sorts last in the scans that do not.
            let bounded = mn > -999.0 || mx < 999.0;
            let mut want: Vec<(String, Option<f64>, f64)> = raw.iter().filter_map(|r| {
                let sym = r["symbol"].as_str()?.to_string();
                let ch  = r["change_pct"].as_f64();
                let vol = r["volume"].as_f64()?;
                let px  = r["price"].as_f64().unwrap_or(0.0);
                let change_ok = match ch {
                    Some(c) => c >= mn && c <= mx,
                    None => !bounded,
                };
                (px > 0.0 && change_ok && vol >= mv).then_some((sym, ch, vol))
            }).collect();
            // `None` last in BOTH directions. Take the arguments in the
            // caller's order and flip only the Some/Some comparison — sorting
            // descending by passing them reversed would reverse the None
            // handling with it and float every unknown to the top.
            fn by_change(a: Option<f64>, b: Option<f64>, desc: bool) -> std::cmp::Ordering {
                match (a, b) {
                    (Some(x), Some(y)) => {
                        let o = x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal);
                        if desc { o.reverse() } else { o }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
            match sort_by {
                "ChangeAsc"  => want.sort_by(|a, b| by_change(a.1, b.1, false)),
                "VolumeDesc" => want.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)),
                _            => want.sort_by(|a, b| by_change(a.1, b.1, true)),
            }
            want.truncate(limit);
            let want_syms: Vec<String> = want.into_iter().map(|t| t.0).collect();
            let pass = !raw.is_empty() && got == want_syms;
            ("ScannerFilterCorrect".into(), pass,
             if pass { format!("scanner filter+sort correct ({} rows, sort={sort_by})", got.len()) }
             else { format!("scanner filter WRONG (sort={sort_by}): app={} rows, expected={} rows{}",
                            got.len(), want_syms.len(),
                            if raw.is_empty() { " [no raw pool]" } else { "" }) })
        }

        // ── BEHAVIORAL ORACLE: spreadsheet formula computes the right value ───
        // Reads the app's formula-evaluated value for a cell and compares to the
        // expected result. {"spreadsheet_cell_equals":{"pane":0,"row":0,"col":3,"value":30}}
        "spreadsheet_cell_equals" => {
            let pane_idx = val["pane"].as_u64().unwrap_or(0) as usize;
            let row = val["row"].as_u64().unwrap_or(0) as usize;
            let col = val["col"].as_u64().unwrap_or(0) as usize;
            let want = val["value"].as_f64().unwrap_or(f64::NAN);
            let tol  = val["tol"].as_f64().unwrap_or(1e-6);
            let got = state.app_state["panes"][pane_idx]["spreadsheet"]["computed"]
                .get(row).and_then(|r| r.get(col)).and_then(|v| v.as_f64());
            let pass = matches!(got, Some(g) if (g - want).abs() <= tol);
            ("SpreadsheetCellEquals".into(), pass,
             if pass { format!("cell ({row},{col}) == {want}") }
             else { format!("cell ({row},{col}): expected {want}, got {:?}", got) })
        }

        // ── BEHAVIORAL ORACLE: every play's risk/reward is computed correctly ──
        // rr must equal |target-entry| / |entry-stop| for each play. {"play_rr_correct":true}
        "play_rr_correct" => {
            let plays = state.app_state["playbook"]["plays"].as_array().cloned().unwrap_or_default();
            let mut bad = vec![];
            for p in &plays {
                let e = p["entry"].as_f64().unwrap_or(0.0);
                let t = p["target"].as_f64().unwrap_or(0.0);
                let s = p["stop"].as_f64().unwrap_or(0.0);
                let got = p["risk_reward"].as_f64().unwrap_or(-1.0);
                let want = if (e - s).abs() > 0.001 { (t - e).abs() / (e - s).abs() } else { 0.0 };
                if (got - want).abs() > 1e-3 {
                    bad.push(format!("{} e{e} t{t} s{s}: rr got {got:.3}, want {want:.3}",
                                     p["symbol"].as_str().unwrap_or("?")));
                }
            }
            let pass = !plays.is_empty() && bad.is_empty();
            ("PlayRrCorrect".into(), pass,
             if pass { format!("all {} plays have correct risk/reward", plays.len()) }
             else if plays.is_empty() { "no plays present".into() }
             else { format!("{} wrong: {}", bad.len(), bad.join("; ")) })
        }

        "watchlist_section_count_equals" => {
            let expect = val.as_u64()
                .or_else(|| val["count"].as_u64())
                .unwrap_or(0) as usize;
            let actual = state.app_state["watchlist"]["section_count"]
                .as_u64().unwrap_or(0) as usize;
            let pass = actual == expect;
            ("WatchlistSectionCountEquals".into(), pass,
             if pass { format!("watchlist section count == {expect}") }
             else    { format!("watchlist section count: expected {expect}, got {actual}") })
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

        // ── Crash / panic ─────────────────────────────────────────────────────
        // Highest-severity invariant: nothing panicked during this scenario.
        // A render-thread panic closes the window; a headless-ticker panic stalls
        // frames. Either is a bug worth surfacing. Reads the global panic log
        // (cleared at scenario start).
        "no_panic" => {
            let panics = super::panics();
            let pass = panics.is_empty();
            ("NoPanic".into(), pass,
             if pass { "no panics".into() }
             else {
                 let ps: Vec<_> = panics.iter()
                     .map(|p| format!("[{}] {} ({})", p.thread, p.message, p.location))
                     .collect();
                 format!("{} panic(s): {}", ps.len(), ps.join(" | "))
             })
        }

        // ── Widget tree sweeps ────────────────────────────────────────────────
        "all_touch_targets_ok" => {
            // Checks all widgets with role "button" or "input" meet min_size_px
            let min_px = val.as_f64()
                .or_else(|| val["min_size_px"].as_f64())
                .unwrap_or(28.0) as f32;
            let violations: Vec<_> = state.widget_tree.iter()
                .filter(|w| w.role == "button" || w.role == "input")
                .filter(|w| w.rect.area() > 0.0)
                .filter(|w| w.rect.min_side() < min_px)
                .map(|w| format!("{} ({:.0}px)", w.id, w.rect.min_side()))
                .collect();
            let pass = violations.is_empty();
            ("AllTouchTargetsOk".into(), pass,
             if pass {
                 let n = state.widget_tree.iter().filter(|w| w.role == "button" || w.role == "input").count();
                 format!("{n} button/input widgets all >= {min_px}px")
             } else {
                 format!("touch target too small: {}", violations.join(", "))
             })
        }
        "no_clipped_widgets" => {
            let violations: Vec<_> = state.widget_tree.iter()
                .filter(|w| w.is_clipped)
                .map(|w| w.id.as_str())
                .collect();
            let pass = violations.is_empty();
            ("NoClippedWidgets".into(), pass,
             if pass { "no clipped widgets".into() }
             else    { format!("clipped: {}", violations.join(", ")) })
        }
        "design_audit_clean" => {
            // Checks: no violations, no clipped widgets, all touch targets ok
            let no_viol  = state.active_violations.is_empty();
            let no_clip  = !state.widget_tree.iter().any(|w| w.is_clipped);
            let pass = no_viol && no_clip;
            let mut issues = Vec::new();
            if !no_viol { issues.push(format!("{} contract violations", state.active_violations.len())); }
            if !no_clip {
                let n = state.widget_tree.iter().filter(|w| w.is_clipped).count();
                issues.push(format!("{n} clipped widgets"));
            }
            ("DesignAuditClean".into(), pass,
             if pass { "design audit clean".into() }
             else    { issues.join("; ") })
        }

        // ── Logical combinators ───────────────────────────────────────────────
        "not" => {
            let inner = &val["assertion"];
            let (kind, pass, detail) = eval_one(inner, state);
            ("Not".into(), !pass, format!("NOT ({kind}: {detail})"))
        }
        "all_of" => {
            let items = val.as_array()
                .or_else(|| val["assertions"].as_array())
                .cloned()
                .unwrap_or_default();
            let results: Vec<_> = items.iter().map(|a| eval_one(a, state)).collect();
            let all_pass = results.iter().all(|(_, p, _)| *p);
            let detail = results.iter()
                .map(|(k, p, d)| format!("[{}{}] {}", if *p {"✓"} else {"✗"}, k, d))
                .collect::<Vec<_>>().join("; ");
            ("AllOf".into(), all_pass, detail)
        }
        "any_of" => {
            let items = val.as_array()
                .or_else(|| val["assertions"].as_array())
                .cloned()
                .unwrap_or_default();
            let results: Vec<_> = items.iter().map(|a| eval_one(a, state)).collect();
            let any_pass = results.iter().any(|(_, p, _)| *p);
            let detail = results.iter()
                .map(|(k, p, d)| format!("[{}{}] {}", if *p {"✓"} else {"✗"}, k, d))
                .collect::<Vec<_>>().join("; ");
            ("AnyOf".into(), any_pass, detail)
        }

        // ── State numeric range ───────────────────────────────────────────────
        "state_field_gte" => {
            // {"state_field_gte": {"path": "panes.0.indicator_count", "min": 2}}
            let path = val["path"].as_str().unwrap_or("");
            let min  = val["min"].as_f64().unwrap_or(0.0);
            let actual = json_path(&state.app_state, path);
            let actual_f = actual.as_f64().unwrap_or(0.0);
            let pass = actual_f >= min;
            ("StateFieldGte".into(), pass,
             if pass { format!("state.{path} ({actual_f}) >= {min}") }
             else    { format!("state.{path}: expected >= {min}, got {actual_f}") })
        }

        "state_field_lte" => {
            // {"state_field_lte": {"path": "panes.0.indicator_count", "max": 5}}
            let path = val["path"].as_str().unwrap_or("");
            let max  = val["max"].as_f64().unwrap_or(f64::MAX);
            let actual = json_path(&state.app_state, path);
            let actual_f = actual.as_f64().unwrap_or(0.0);
            let pass = actual_f <= max;
            ("StateFieldLte".into(), pass,
             if pass { format!("state.{path} ({actual_f}) <= {max}") }
             else    { format!("state.{path}: expected <= {max}, got {actual_f}") })
        }

        // ── Widget count ──────────────────────────────────────────────────────
        "widget_count_gte" => {
            // {"widget_count_gte": {"role": "button", "min": 1}}
            let role_f  = val["role"].as_str();
            let label_f = val["label"].as_str();
            let min = val["min"].as_u64()
                .or_else(|| val.as_u64())
                .unwrap_or(1) as usize;
            let count = state.widget_tree.iter().filter(|w| {
                let role_ok  = role_f.map_or(true,  |r| w.role  == r);
                let label_ok = label_f.map_or(true, |l| w.label == l);
                role_ok && label_ok
            }).count();
            let pass = count >= min;
            ("WidgetCountGte".into(), pass,
             if pass { format!("{count} widget(s) found >= {min}") }
             else    { format!("found {count} widget(s), expected >= {min} (role={role_f:?} label={label_f:?})") })
        }

        "widget_count_lte" => {
            // {"widget_count_lte": {"role": "button", "max": 10}}
            let role_f  = val["role"].as_str();
            let label_f = val["label"].as_str();
            let max = val["max"].as_u64()
                .or_else(|| val.as_u64())
                .unwrap_or(0) as usize;
            let count = state.widget_tree.iter().filter(|w| {
                let role_ok  = role_f.map_or(true,  |r| w.role  == r);
                let label_ok = label_f.map_or(true, |l| w.label == l);
                role_ok && label_ok
            }).count();
            let pass = count <= max;
            ("WidgetCountLte".into(), pass,
             if pass { format!("{count} widget(s) found <= {max}") }
             else    { format!("found {count} widget(s), expected <= {max} (role={role_f:?} label={label_f:?})") })
        }

        "widget_label_contains" => {
            // {"widget_label_contains": {"contains": "AAPL"}} or with optional role filter
            let needle  = val["contains"].as_str().or_else(|| val.as_str()).unwrap_or("");
            let role_f  = val["role"].as_str();
            let case_i  = val["case_insensitive"].as_bool().unwrap_or(true);
            let needle_cmp = if case_i { needle.to_lowercase() } else { needle.to_string() };
            let found = state.widget_tree.iter().any(|w| {
                let role_ok = role_f.map_or(true, |r| w.role == r);
                let label_cmp = if case_i { w.label.to_lowercase() } else { w.label.clone() };
                role_ok && label_cmp.contains(&needle_cmp)
            });
            ("WidgetLabelContains".into(), found,
             if found { format!("found widget with label containing '{needle}'") }
             else     { format!("no widget label contains '{needle}' (role={role_f:?})") })
        }

        // ── Performance ───────────────────────────────────────────────────────
        "frame_time_below_ms" => {
            // {"frame_time_below_ms": 33.3}
            let max_ms = val.as_f64()
                .or_else(|| val["max_ms"].as_f64())
                .unwrap_or(33.3) as f32;
            let actual = state.frame_time_ms;
            // 0.0 = headless/not-yet-measured → pass by default
            let pass = actual < max_ms || actual == 0.0;
            ("FrameTimeBelowMs".into(), pass,
             if pass { format!("frame_time {actual:.2}ms < {max_ms}ms") }
             else    { format!("frame_time {actual:.2}ms >= {max_ms}ms") })
        }

        "fps_history_min_above" => {
            // {"fps_history_min_above": 10.0}
            let min = val.as_f64()
                .or_else(|| val["min"].as_f64())
                .unwrap_or(10.0) as f32;
            let fps_min = state.fps_history.iter().cloned().fold(f32::MAX, f32::min);
            let pass = state.fps_history.is_empty() || fps_min >= min;
            ("FpsHistoryMinAbove".into(), pass,
             if state.fps_history.is_empty() {
                 "fps history empty — pass by default".into()
             } else if pass {
                 format!("fps_min {fps_min:.1} >= {min} (over {} frames)", state.fps_history.len())
             } else {
                 format!("fps_min {fps_min:.1} < {min}")
             })
        }

        // ── Contract violations count ─────────────────────────────────────────
        "violation_count_lte" => {
            // {"violation_count_lte": 0}
            let max = val.as_u64()
                .or_else(|| val["max"].as_u64())
                .unwrap_or(0) as usize;
            let actual = state.active_violations.len();
            let pass = actual <= max;
            ("ViolationCountLte".into(), pass,
             if pass { format!("{actual} violation(s) <= {max}") }
             else    { format!("{actual} violation(s) > max {max}") })
        }

        // ── Per-pane assertions ───────────────────────────────────────────────
        "pane_symbol_equals" => {
            // {"pane_symbol_equals": {"pane": 0, "symbol": "AAPL"}}
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let expect = val["symbol"].as_str()
                .or_else(|| val.as_str())
                .unwrap_or("");
            let actual = state.app_state["panes"]
                .get(pane)
                .and_then(|p| p["symbol"].as_str())
                .unwrap_or("");
            let pass = actual.to_lowercase() == expect.to_lowercase();
            ("PaneSymbolEquals".into(), pass,
             if pass { format!("pane {pane} symbol == '{expect}'") }
             else    { format!("pane {pane} symbol: expected '{expect}', got '{actual}'") })
        }

        "pane_timeframe_equals" => {
            // {"pane_timeframe_equals": {"pane": 0, "tf": "5m"}}
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let expect = val["tf"].as_str()
                .or_else(|| val["timeframe"].as_str())
                .or_else(|| val.as_str())
                .unwrap_or("");
            let actual = state.app_state["panes"]
                .get(pane)
                .and_then(|p| p["timeframe"].as_str())
                .unwrap_or("");
            let pass = actual == expect;
            ("PaneTimeframeEquals".into(), pass,
             if pass { format!("pane {pane} timeframe == '{expect}'") }
             else    { format!("pane {pane} timeframe: expected '{expect}', got '{actual}'") })
        }

        // ── Annotation count ──────────────────────────────────────────────────
        "annotation_count_equals" => {
            // {"annotation_count_equals": 2}
            let expect = val.as_u64()
                .or_else(|| val["count"].as_u64())
                .unwrap_or(0) as usize;
            let actual = state.active_annotations.len();
            let pass = actual == expect;
            ("AnnotationCountEquals".into(), pass,
             if pass { format!("annotation count == {expect}") }
             else    { format!("annotation count: expected {expect}, got {actual}") })
        }

        // ── Annotation id lookup ──────────────────────────────────────────────
        "annotation_id_exists" => {
            // {"annotation_id_exists": "ann.toolbar"}
            let id = val.as_str()
                .or_else(|| val["id"].as_str())
                .unwrap_or("");
            let pass = state.active_annotations.iter().any(|a| a.id == id);
            ("AnnotationIdExists".into(), pass,
             if pass { format!("annotation '{id}' exists") }
             else    { format!("annotation '{id}' not found (active: {})", state.active_annotations.len()) })
        }

        // ── Watchlist section title ───────────────────────────────────────────
        "watchlist_section_title_equals" => {
            // {"watchlist_section_title_equals": {"idx": 0, "title": "Stocks"}}
            let idx    = val["idx"].as_u64().unwrap_or(0) as usize;
            let expect = val["title"].as_str().unwrap_or("");
            let actual = state.app_state["watchlist"]["sections"]
                .get(idx)
                .and_then(|s| s["title"].as_str())
                .unwrap_or("");
            let pass = actual == expect;
            ("WatchlistSectionTitleEquals".into(), pass,
             if pass { format!("watchlist section {idx} title == '{expect}'") }
             else    { format!("watchlist section {idx} title: expected '{expect}', got '{actual}'") })
        }

        // ── Open dialog count ─────────────────────────────────────────────────
        "open_dialog_count_equals" => {
            // {"open_dialog_count_equals": 0}
            let expect = val.as_u64()
                .or_else(|| val["count"].as_u64())
                .unwrap_or(0) as usize;
            let actual = state.open_dialogs.len();
            let pass   = actual == expect;
            ("OpenDialogCountEquals".into(), pass,
             if pass { format!("open dialog count == {expect}") }
             else    { format!("open dialog count: expected {expect}, got {actual} ({:?})", state.open_dialogs) })
        }

        // ── Captures equality ─────────────────────────────────────────────────
        "captures_equal" => {
            // {"captures_equal": {"key": "initial_symbol", "equals": "SPY"}}
            let key    = val["key"].as_str().unwrap_or("");
            let expect = &val["equals"];
            let actual = state.captures.get(key).cloned().unwrap_or(Value::Null);
            let pass   = &actual == expect;
            ("CapturesEqual".into(), pass,
             if pass { format!("captures['{key}'] == {expect}") }
             else    { format!("captures['{key}']: expected {expect}, got {actual}") })
        }

        // ── State field string contains ───────────────────────────────────────
        "state_field_contains" => {
            // {"state_field_contains": {"path": "active_symbol", "contains": "S"}}
            let path    = val["path"].as_str().unwrap_or("");
            let needle  = val["contains"].as_str().unwrap_or("");
            let case_i  = val["case_insensitive"].as_bool().unwrap_or(true);
            let actual  = json_path(&state.app_state, path);
            let actual_s = actual.as_str().unwrap_or("");
            let pass = if case_i {
                actual_s.to_lowercase().contains(&needle.to_lowercase())
            } else {
                actual_s.contains(needle)
            };
            ("StateFieldContains".into(), pass,
             if pass { format!("state.{path} ('{actual_s}') contains '{needle}'") }
             else    { format!("state.{path} ('{actual_s}') does not contain '{needle}'") })
        }

        // ── State field not null ──────────────────────────────────────────────
        "state_field_not_null" => {
            // {"state_field_not_null": "active_symbol"} or {"state_field_not_null": {"path":"..."}}
            let path = val.as_str()
                .or_else(|| val["path"].as_str())
                .unwrap_or("");
            let actual = json_path(&state.app_state, path);
            let pass   = !actual.is_null();
            ("StateFieldNotNull".into(), pass,
             if pass { format!("state.{path} is non-null ({actual})") }
             else    { format!("state.{path} is null/missing") })
        }

        // ── State field length ────────────────────────────────────────────────
        "state_field_length_equals" => {
            // {"state_field_length_equals": {"path": "panes", "length": 2}}
            let path   = val["path"].as_str().unwrap_or("");
            let expect = val["length"].as_u64().unwrap_or(0) as usize;
            let field  = json_path(&state.app_state, path);
            let actual = match &field {
                Value::Array(a)  => a.len(),
                Value::String(s) => s.len(),
                _                => 0,
            };
            let pass = actual == expect;
            ("StateFieldLengthEquals".into(), pass,
             if pass { format!("state.{path}.len() == {expect}") }
             else    { format!("state.{path}.len(): expected {expect}, got {actual}") })
        }

        // ── State field array contains value ──────────────────────────────────
        "state_field_array_contains" => {
            // {"state_field_array_contains": {"path": "open_dialogs", "value": "order_entry"}}
            let path  = val["path"].as_str().unwrap_or("");
            let value = &val["value"];
            let field = json_path(&state.app_state, path);
            let pass = field.as_array()
                .map(|arr| arr.iter().any(|v| v == value))
                .unwrap_or(false);
            ("StateFieldArrayContains".into(), pass,
             if pass { format!("state.{path} contains {value}") }
             else    { format!("state.{path} does not contain {value} (got {field})") })
        }

        // ── State field equals ────────────────────────────────────────────────
        "state_field_equals" => {
            // {"state_field_equals": {"path": "active_symbol", "value": "SPY"}}
            let path   = val["path"].as_str().unwrap_or("");
            let expect = &val["value"];
            let actual = json_path(&state.app_state, path);
            let pass   = &actual == expect;
            ("StateFieldEquals".into(), pass,
             if pass { format!("state.{path} == {expect}") }
             else    { format!("state.{path}: expected {expect}, got {actual}") })
        }

        // ── Canvas drawing assertions ─────────────────────────────────────────
        "canvas_drawing_count_equals" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let expect = val["count"].as_u64().unwrap_or(0) as usize;
            let actual = state.canvas.panes.get(pane).map(|p| p.drawings.len()).unwrap_or(0);
            let pass   = actual == expect;
            ("CanvasDrawingCountEquals".into(), pass,
             if pass { format!("canvas pane {pane} has {actual} drawing(s)") }
             else    { format!("canvas pane {pane}: expected {expect} drawings, got {actual}") })
        }
        "canvas_drawing_exists" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            let id   = val["id"].as_str().unwrap_or("");
            let pass = state.canvas.panes.get(pane)
                .map(|p| p.drawings.iter().any(|d| d.id == id)).unwrap_or(false);
            ("CanvasDrawingExists".into(), pass,
             if pass { format!("canvas pane {pane} has drawing '{id}'") }
             else    { format!("canvas pane {pane}: no drawing '{id}'") })
        }
        "canvas_drawing_kind_count" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let kind   = val["kind"].as_str().unwrap_or("");
            let expect = val["count"].as_u64().unwrap_or(0) as usize;
            let actual = state.canvas.panes.get(pane)
                .map(|p| p.drawings.iter().filter(|d| d.kind == kind).count()).unwrap_or(0);
            let pass   = actual == expect;
            ("CanvasDrawingKindCount".into(), pass,
             if pass { format!("canvas pane {pane} has {actual} '{kind}' drawing(s)") }
             else    { format!("canvas pane {pane}: expected {expect} '{kind}' drawings, got {actual}") })
        }
        "canvas_no_drawings" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let actual = state.canvas.panes.get(pane).map(|p| p.drawings.len()).unwrap_or(0);
            let pass   = actual == 0;
            ("CanvasNoDrawings".into(), pass,
             if pass { format!("canvas pane {pane} has no drawings") }
             else    { format!("canvas pane {pane} has {actual} drawing(s) (expected 0)") })
        }

        // ── Canvas viewport assertions ────────────────────────────────────────
        "canvas_viewport_price_contains" => {
            let pane  = val["pane"].as_u64().unwrap_or(0) as usize;
            let price = val["price"].as_f64().unwrap_or(0.0) as f32;
            let pass  = state.canvas.panes.get(pane)
                .map(|p| price >= p.viewport.price_low && price <= p.viewport.price_high)
                .unwrap_or(false);
            ("CanvasViewportPriceContains".into(), pass,
             if pass { format!("canvas pane {pane} viewport contains price {price:.2}") }
             else {
                 let range = state.canvas.panes.get(pane)
                     .map(|p| format!("[{:.2}, {:.2}]", p.viewport.price_low, p.viewport.price_high))
                     .unwrap_or_else(|| "pane not found".into());
                 format!("canvas pane {pane} viewport {range} does not contain price {price:.2}")
             })
        }
        "canvas_viewport_px_per_bar_gte" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let min    = val["min"].as_f64().unwrap_or(0.0) as f32;
            let actual = state.canvas.panes.get(pane).map(|p| p.viewport.px_per_bar).unwrap_or(0.0);
            let pass   = actual >= min;
            ("CanvasViewportPxPerBarGte".into(), pass,
             if pass { format!("canvas pane {pane} px_per_bar {actual:.2} >= {min:.2}") }
             else    { format!("canvas pane {pane} px_per_bar {actual:.2} < {min:.2}") })
        }

        // ── Canvas invariants (render-data sanity) ────────────────────────────
        // No NaN/Inf anywhere in the canvas: bar OHLCV + screen coords, viewport
        // densities/range, drawing anchors, indicator values. NaN/Inf here means a
        // divide-by-zero / bad-scale math bug that paints garbage or blanks the
        // chart — exactly the class a thorough story should surface. Pass when no
        // panes (nothing to violate).
        "canvas_all_finite" => {
            let mut bad: Vec<String> = Vec::new();
            for p in &state.canvas.panes {
                let mut chk = |name: &str, v: f32| { if !v.is_finite() { bad.push(format!("pane{} {name}={v}", p.index)); } };
                chk("vp.price_low", p.viewport.price_low);
                chk("vp.price_high", p.viewport.price_high);
                chk("vp.px_per_bar", p.viewport.px_per_bar);
                chk("vp.px_per_price", p.viewport.px_per_price);
                for (bi, b) in p.bars.iter().enumerate() {
                    for (n, v) in [("o",b.open),("h",b.high),("l",b.low),("c",b.close),("v",b.volume),
                                   ("sx",b.screen_x),("soy",b.screen_open_y),("shy",b.screen_high_y),
                                   ("sly",b.screen_low_y),("scy",b.screen_close_y)] {
                        if !v.is_finite() { bad.push(format!("pane{} bar{bi}.{n}={v}", p.index)); }
                    }
                }
                for d in &p.drawings {
                    for (ai, a) in d.anchors.iter().enumerate() {
                        for (n, v) in [("price",a.price),("sx",a.screen_x),("sy",a.screen_y)] {
                            if !v.is_finite() { bad.push(format!("pane{} {}#{ai}.{n}={v}", p.index, d.id)); }
                        }
                    }
                }
                // NOTE: indicator last_value/2/3 are intentionally NOT checked here.
                // Several indicators have legitimately-NaN latest values — Ichimoku's
                // chikou span is plotted shifted into the past (its most recent `kijun`
                // bars are undefined), and long-warmup overlays may not have filled the
                // final bar yet. Indicator value sanity is covered precisely by
                // `canvas_indicator_value_in_range` on the bounded oscillators. This
                // assertion stays focused on render geometry, where NaN is unambiguous.
            }
            // Cap the detail so a fully-NaN frame doesn't produce a megabyte string.
            let pass = bad.is_empty();
            let shown: Vec<_> = bad.iter().take(12).cloned().collect();
            ("CanvasAllFinite".into(), pass,
             if pass { "all canvas values finite".into() }
             else { format!("{} non-finite value(s): {}", bad.len(), shown.join(", ")) })
        }

        // Viewport is coherent: finite, price_low < price_high, positive density.
        // A collapsed/inverted/zero-density viewport => blank or garbled chart.
        "viewport_sane" => {
            let only = val["pane"].as_u64().map(|n| n as usize);
            let mut bad: Vec<String> = Vec::new();
            for p in &state.canvas.panes {
                if let Some(idx) = only { if p.index != idx { continue; } }
                let vp = &p.viewport;
                if !vp.price_low.is_finite() || !vp.price_high.is_finite() {
                    bad.push(format!("pane{} non-finite range [{},{}]", p.index, vp.price_low, vp.price_high));
                } else if vp.price_low >= vp.price_high {
                    bad.push(format!("pane{} inverted/empty range [{}, {}]", p.index, vp.price_low, vp.price_high));
                }
                if !(vp.px_per_bar.is_finite() && vp.px_per_bar > 0.0) {
                    bad.push(format!("pane{} px_per_bar={}", p.index, vp.px_per_bar));
                }
                if !vp.px_per_price.is_finite() {
                    bad.push(format!("pane{} px_per_price={}", p.index, vp.px_per_price));
                }
            }
            let pass = bad.is_empty();
            ("ViewportSane".into(), pass,
             if pass { "viewport(s) sane".into() } else { bad.join("; ") })
        }

        // ── Canvas indicator assertions ───────────────────────────────────────
        "canvas_indicator_value_in_range" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            let kind = val["kind"].as_str().unwrap_or("");
            let min  = val["min"].as_f64().unwrap_or(f64::NEG_INFINITY) as f32;
            let max  = val["max"].as_f64().unwrap_or(f64::INFINITY) as f32;
            match state.canvas.panes.get(pane)
                .and_then(|p| p.indicators.iter().find(|i| i.kind == kind))
                .and_then(|i| i.last_value)
            {
                Some(v) => {
                    let pass = v >= min && v <= max;
                    ("CanvasIndicatorValueInRange".into(), pass,
                     if pass { format!("canvas pane {pane} {kind} last={v:.4} in [{min}, {max}]") }
                     else    { format!("canvas pane {pane} {kind} last={v:.4} outside [{min}, {max}]") })
                }
                None => ("CanvasIndicatorValueInRange".into(), false,
                         format!("canvas pane {pane} has no indicator '{kind}'"))
            }
        }
        "canvas_indicator_last_value_near" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let kind   = val["kind"].as_str().unwrap_or("");
            let expect = val["value"].as_f64().unwrap_or(0.0) as f32;
            let tol    = val["tolerance"].as_f64().unwrap_or(0.01) as f32;
            match state.canvas.panes.get(pane)
                .and_then(|p| p.indicators.iter().find(|i| i.kind == kind))
                .and_then(|i| i.last_value)
            {
                Some(v) => {
                    let pass = (v - expect).abs() <= tol;
                    ("CanvasIndicatorLastValueNear".into(), pass,
                     if pass { format!("canvas pane {pane} {kind} ≈ {expect:.4} (actual {v:.4})") }
                     else    { format!("canvas pane {pane} {kind}: expected ≈ {expect:.4}, got {v:.4} (tol {tol:.4})") })
                }
                None => ("CanvasIndicatorLastValueNear".into(), false,
                         format!("canvas pane {pane} has no indicator '{kind}'"))
            }
        }

        // ── Canvas bar assertions ─────────────────────────────────────────────
        "canvas_bar_ohlc_valid" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            let bars = state.canvas.panes.get(pane).map(|p| p.bars.as_slice()).unwrap_or(&[]);
            let invalid = bars.iter()
                .filter(|b| b.high < b.low || b.close < b.low || b.close > b.high)
                .count();
            let pass = invalid == 0;
            ("CanvasBarOhlcValid".into(), pass,
             if pass { format!("canvas pane {pane}: all {} bars have valid OHLC", bars.len()) }
             else    { format!("canvas pane {pane}: {invalid}/{} bars have invalid OHLC", bars.len()) })
        }
        "canvas_visible_bar_count_gte" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let min    = val["min"].as_u64().unwrap_or(0) as usize;
            let actual = state.canvas.panes.get(pane).map(|p| p.bars.len()).unwrap_or(0);
            let pass   = actual >= min;
            ("CanvasVisibleBarCountGte".into(), pass,
             if pass { format!("canvas pane {pane} has {actual} visible bars (>= {min})") }
             else    { format!("canvas pane {pane}: expected >= {min} visible bars, got {actual}") })
        }

        // ── Canvas drawing anchor assertions ──────────────────────────────────
        "canvas_drawing_anchor_price_near" => {
            let pane       = val["pane"].as_u64().unwrap_or(0) as usize;
            let id         = val["id"].as_str().unwrap_or("");
            let anchor_idx = val["anchor"].as_u64().unwrap_or(0) as usize;
            let expect     = val["price"].as_f64().unwrap_or(0.0) as f32;
            let tol        = val["tolerance"].as_f64().unwrap_or(0.01) as f32;
            match state.canvas.panes.get(pane)
                .and_then(|p| p.drawings.iter().find(|d| d.id == id))
                .and_then(|d| d.anchors.get(anchor_idx))
                .map(|a| a.price)
            {
                Some(price) => {
                    let pass = (price - expect).abs() <= tol;
                    ("CanvasDrawingAnchorPriceNear".into(), pass,
                     if pass { format!("canvas pane {pane} drawing '{id}' anchor {anchor_idx} price ≈ {expect:.2}") }
                     else    { format!("canvas pane {pane} drawing '{id}' anchor {anchor_idx}: expected ≈ {expect:.2}, got {price:.2}") })
                }
                None => ("CanvasDrawingAnchorPriceNear".into(), false,
                         format!("canvas pane {pane}: no drawing '{id}' or anchor {anchor_idx}"))
            }
        }

        // ── Canvas pane count ─────────────────────────────────────────────────
        "canvas_pane_count_equals" => {
            let expect = val.as_u64().unwrap_or(0) as usize;
            let actual = state.canvas.panes.len();
            let pass   = actual == expect;
            ("CanvasPaneCountEquals".into(), pass,
             if pass { format!("canvas has {actual} pane(s)") }
             else    { format!("canvas: expected {expect} panes, got {actual}") })
        }

        // ── Canvas drawing negative assertions ────────────────────────────────
        "canvas_drawing_not_exists" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            let id   = val["id"].as_str().unwrap_or("");
            let found = state.canvas.panes.get(pane)
                .map(|p| p.drawings.iter().any(|d| d.id == id)).unwrap_or(false);
            ("CanvasDrawingNotExists".into(), !found,
             if !found { format!("canvas pane {pane}: drawing '{id}' correctly absent") }
             else      { format!("canvas pane {pane}: drawing '{id}' unexpectedly exists") })
        }

        // ── Canvas indicator presence ─────────────────────────────────────────
        "canvas_no_indicators" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let actual = state.canvas.panes.get(pane)
                .map(|p| p.indicators.iter().filter(|i| i.last_value.is_some()).count())
                .unwrap_or(0);
            let pass   = actual == 0;
            ("CanvasNoIndicators".into(), pass,
             if pass { format!("canvas pane {pane} has no indicators") }
             else    { format!("canvas pane {pane} has {actual} indicator(s) (expected 0)") })
        }
        "canvas_indicator_exists" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            let kind = val["kind"].as_str().unwrap_or("");
            let pass = state.canvas.panes.get(pane)
                .map(|p| p.indicators.iter().any(|i| i.kind == kind && i.last_value.is_some()))
                .unwrap_or(false);
            ("CanvasIndicatorExists".into(), pass,
             if pass { format!("canvas pane {pane} indicator '{kind}' exists") }
             else    { format!("canvas pane {pane}: no indicator '{kind}' with a value") })
        }
        "canvas_indicator_count_equals" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let expect = val["count"].as_u64().unwrap_or(0) as usize;
            let actual = state.canvas.panes.get(pane)
                .map(|p| p.indicators.iter().filter(|i| i.last_value.is_some()).count())
                .unwrap_or(0);
            let pass   = actual == expect;
            ("CanvasIndicatorCountEquals".into(), pass,
             if pass { format!("canvas pane {pane} has {actual} indicator(s)") }
             else    { format!("canvas pane {pane}: expected {expect} indicators, got {actual}") })
        }

        // ── Canvas drawing anchor count ───────────────────────────────────────
        "canvas_drawing_anchor_count" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let id     = val["id"].as_str().unwrap_or("");
            let expect = val["count"].as_u64().unwrap_or(0) as usize;
            match state.canvas.panes.get(pane)
                .and_then(|p| p.drawings.iter().find(|d| d.id == id))
            {
                Some(d) => {
                    let actual = d.anchors.len();
                    let pass   = actual == expect;
                    ("CanvasDrawingAnchorCount".into(), pass,
                     if pass { format!("canvas pane {pane} drawing '{id}' has {actual} anchor(s)") }
                     else    { format!("canvas pane {pane} drawing '{id}': expected {expect} anchors, got {actual}") })
                }
                None => ("CanvasDrawingAnchorCount".into(), false,
                         format!("canvas pane {pane}: drawing '{id}' not found"))
            }
        }

        // ── Canvas bar volume ─────────────────────────────────────────────────
        "canvas_bar_volume_gte" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            let min  = val["min"].as_f64().unwrap_or(0.0) as f32;
            let bars = state.canvas.panes.get(pane).map(|p| p.bars.as_slice()).unwrap_or(&[]);
            let invalid = bars.iter().filter(|b| b.volume < min).count();
            let pass = invalid == 0;
            ("CanvasBarVolumeGte".into(), pass,
             if pass { format!("canvas pane {pane}: all {} bar(s) have volume >= {min:.0}", bars.len()) }
             else    { format!("canvas pane {pane}: {invalid}/{} bar(s) have volume < {min:.0}", bars.len()) })
        }

        // ── Canvas indicator auxiliary value assertions ───────────────────────
        "canvas_indicator_value2_near" => {
            let pane   = val["pane"].as_u64().unwrap_or(0) as usize;
            let kind   = val["kind"].as_str().unwrap_or("");
            let expect = val["value"].as_f64().unwrap_or(0.0) as f32;
            let tol    = val["tolerance"].as_f64().unwrap_or(0.01) as f32;
            match state.canvas.panes.get(pane)
                .and_then(|p| p.indicators.iter().find(|i| i.kind == kind))
                .and_then(|i| i.last_value2)
            {
                Some(v) => {
                    let pass = (v - expect).abs() <= tol;
                    ("CanvasIndicatorValue2Near".into(), pass,
                     if pass { format!("canvas pane {pane} {kind} value2 ≈ {expect:.4} (actual {v:.4})") }
                     else    { format!("canvas pane {pane} {kind}: value2 expected ≈ {expect:.4}, got {v:.4} (tol {tol:.4})") })
                }
                None => ("CanvasIndicatorValue2Near".into(), false,
                         format!("canvas pane {pane} indicator '{kind}' has no value2"))
            }
        }
        "canvas_indicator_value2_in_range" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            let kind = val["kind"].as_str().unwrap_or("");
            let min  = val["min"].as_f64().unwrap_or(f64::NEG_INFINITY) as f32;
            let max  = val["max"].as_f64().unwrap_or(f64::INFINITY) as f32;
            match state.canvas.panes.get(pane)
                .and_then(|p| p.indicators.iter().find(|i| i.kind == kind))
                .and_then(|i| i.last_value2)
            {
                Some(v) => {
                    let pass = v >= min && v <= max;
                    ("CanvasIndicatorValue2InRange".into(), pass,
                     if pass { format!("canvas pane {pane} {kind} value2={v:.4} in [{min:.4}, {max:.4}]") }
                     else    { format!("canvas pane {pane} {kind} value2={v:.4} outside [{min:.4}, {max:.4}]") })
                }
                None => ("CanvasIndicatorValue2InRange".into(), false,
                         format!("canvas pane {pane} indicator '{kind}' has no value2"))
            }
        }

        // ── Canvas time/geometry ordering invariants ──────────────────────────
        // Bars must march forward in time, left→right on screen. Duplicate or
        // out-of-order timestamps (a gap-fill/merge bug) paint a chart that
        // "doesn't look right day to day". Pure data — checks every visible bar.
        "canvas_bars_monotonic" => {
            let only = val["pane"].as_u64().map(|n| n as usize);
            let mut bad: Vec<String> = Vec::new();
            for p in &state.canvas.panes {
                if let Some(idx) = only { if p.index != idx { continue; } }
                for w in p.bars.windows(2) {
                    if w[1].ts_ns <= w[0].ts_ns {
                        bad.push(format!("pane{} ts not increasing: bar{} ts={} <= bar{} ts={}",
                            p.index, w[1].index, w[1].ts_ns, w[0].index, w[0].ts_ns));
                    }
                    if w[1].screen_x < w[0].screen_x {
                        bad.push(format!("pane{} screen_x regressed: bar{} x={:.1} < bar{} x={:.1}",
                            p.index, w[1].index, w[1].screen_x, w[0].index, w[0].screen_x));
                    }
                }
            }
            let pass = bad.is_empty();
            let shown: Vec<_> = bad.iter().take(8).cloned().collect();
            ("CanvasBarsMonotonic".into(), pass,
             if pass { "bars strictly time-ordered, left→right".into() }
             else { format!("{} ordering violation(s): {}", bad.len(), shown.join("; ")) })
        }

        // Candle geometry sanity: on screen, high is above (smaller y than) low,
        // and open/close sit between them. Inverted/garbled geometry = the
        // "long wicks" / broken candle class of render bug.
        "canvas_bars_screen_ordered" => {
            let only = val["pane"].as_u64().map(|n| n as usize);
            let tol  = val["tolerance"].as_f64().unwrap_or(1.0) as f32;
            let mut bad: Vec<String> = Vec::new();
            for p in &state.canvas.panes {
                if let Some(idx) = only { if p.index != idx { continue; } }
                for b in &p.bars {
                    if b.screen_high_y > b.screen_low_y + tol {
                        bad.push(format!("pane{} bar{} high_y={:.1} below low_y={:.1}",
                            p.index, b.index, b.screen_high_y, b.screen_low_y));
                    }
                    let (hi, lo) = (b.screen_high_y - tol, b.screen_low_y + tol);
                    if b.screen_open_y < hi || b.screen_open_y > lo {
                        bad.push(format!("pane{} bar{} open_y={:.1} outside wick [{:.1},{:.1}]",
                            p.index, b.index, b.screen_open_y, b.screen_high_y, b.screen_low_y));
                    }
                    if b.screen_close_y < hi || b.screen_close_y > lo {
                        bad.push(format!("pane{} bar{} close_y={:.1} outside wick [{:.1},{:.1}]",
                            p.index, b.index, b.screen_close_y, b.screen_high_y, b.screen_low_y));
                    }
                }
            }
            let pass = bad.is_empty();
            let shown: Vec<_> = bad.iter().take(8).cloned().collect();
            ("CanvasBarsScreenOrdered".into(), pass,
             if pass { "candle geometry ordered (high above low, body within wick)".into() }
             else { format!("{} geometry violation(s): {}", bad.len(), shown.join("; ")) })
        }

        // Bars must paint inside their own pane rect (+tol). Bars bleeding past
        // the pane bounds = a viewport/scale bug that draws over neighbours.
        "canvas_bars_within_pane" => {
            let only = val["pane"].as_u64().map(|n| n as usize);
            let tol  = val["tolerance"].as_f64().unwrap_or(4.0) as f32;
            let mut bad: Vec<String> = Vec::new();
            for p in &state.canvas.panes {
                if let Some(idx) = only { if p.index != idx { continue; } }
                let r = &p.screen_rect;
                if !(r.w > 0.0 && r.h > 0.0) { continue; } // pane not laid out yet
                let (xl, xr) = (r.x - tol, r.x + r.w + tol);
                let (yt, yb) = (r.y - tol, r.y + r.h + tol);
                for b in &p.bars {
                    if b.screen_x < xl || b.screen_x > xr {
                        bad.push(format!("pane{} bar{} x={:.1} outside [{:.1},{:.1}]",
                            p.index, b.index, b.screen_x, xl, xr));
                    }
                    if b.screen_high_y < yt || b.screen_low_y > yb {
                        bad.push(format!("pane{} bar{} y[{:.1},{:.1}] outside [{:.1},{:.1}]",
                            p.index, b.index, b.screen_high_y, b.screen_low_y, yt, yb));
                    }
                }
            }
            let pass = bad.is_empty();
            let shown: Vec<_> = bad.iter().take(8).cloned().collect();
            ("CanvasBarsWithinPane".into(), pass,
             if pass { "all bars paint within their pane".into() }
             else { format!("{} out-of-bounds bar(s): {}", bad.len(), shown.join("; ")) })
        }

        // ── Indicator numerical correctness (oracle) ──────────────────────────
        // Recompute a window-based indicator independently from the captured bars
        // and compare to the chart's value — verifies the number is RIGHT, not just
        // present and in-range. Only window indicators (SMA, WMA) are recomputable
        // from a visible window; recursive ones (EMA/RSI/MACD/DEMA/TEMA/Supertrend)
        // carry history and are out of scope here. Assumes the chart shows the
        // latest bars (not panned), which all harness scenarios do.
        "canvas_indicator_correct" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            let kind = val["kind"].as_str().unwrap_or("");
            let rel  = val["rel_tol"].as_f64().unwrap_or(0.01) as f32; // 1% default
            let Some(p) = state.canvas.panes.get(pane) else {
                return ("CanvasIndicatorCorrect".into(), false, format!("pane {pane} not found"));
            };
            let Some(ind) = p.indicators.iter().find(|i| i.kind == kind) else {
                return ("CanvasIndicatorCorrect".into(), false, format!("pane {pane} has no indicator '{kind}'"));
            };
            let Some(actual) = ind.last_value else {
                return ("CanvasIndicatorCorrect".into(), false, format!("pane {pane} '{kind}' has no value"));
            };
            // The chart's last_value is the indicator at the LAST bar of the full
            // series; we can only recompute it from the captured (visible) bars when
            // the last data bar is actually on screen (no right-margin / panning).
            // Otherwise skip — recomputing from a window that doesn't reach the last
            // bar would be a false positive, not an app fault.
            let last_visible_idx = p.bars.last().map(|b| b.index);
            if last_visible_idx != Some(ind.series_length.saturating_sub(1)) {
                return ("CanvasIndicatorCorrect".into(), true,
                    format!("chart not anchored to latest bar (last visible {:?} of {}); oracle skipped",
                            last_visible_idx, ind.series_length));
            }
            let period = ind.period.max(1);
            let closes: Vec<f32> = p.bars.iter().map(|b| b.close).collect();
            if closes.len() < period {
                return ("CanvasIndicatorCorrect".into(), true,
                    format!("only {} visible bars < period {period}; skipped", closes.len()));
            }
            let win = &closes[closes.len()-period..];
            // Bollinger Bands: verify the middle (SMA) AND the ±2σ bands together —
            // the band math (stddev) is what the single-value oracle can't reach.
            if kind == "BB" {
                let mid = win.iter().sum::<f32>() / period as f32;
                let var = win.iter().map(|c| (c - mid).powi(2)).sum::<f32>() / period as f32;
                let std = var.sqrt();
                let (exp_up, exp_lo) = (mid + 2.0 * std, mid - 2.0 * std);
                let tol = |e: f32| (e.abs() * rel).max(1e-3);
                let mut bad: Vec<String> = Vec::new();
                if (mid - actual).abs() > tol(mid) {
                    bad.push(format!("mid {actual:.4}≠{mid:.4}"));
                }
                match ind.last_value2 { Some(u) if (exp_up - u).abs() > tol(exp_up) =>
                    bad.push(format!("upper {u:.4}≠{exp_up:.4}")), _ => {} }
                match ind.last_value3 { Some(l) if (exp_lo - l).abs() > tol(exp_lo) =>
                    bad.push(format!("lower {l:.4}≠{exp_lo:.4}")), _ => {} }
                let pass = bad.is_empty();
                return ("CanvasIndicatorCorrect".into(), pass,
                    if pass { format!("pane {pane} BB({period}) mid/upper/lower match recompute (±{:.4})", tol(mid)) }
                    else    { format!("pane {pane} BB({period}) mismatch: {}", bad.join(", ")) });
            }
            let expected = match kind {
                "SMA" => Some(win.iter().sum::<f32>() / period as f32),
                "WMA" => {
                    let denom = (period * (period + 1) / 2) as f32;
                    Some(win.iter().enumerate().map(|(j,&c)| c * (j+1) as f32).sum::<f32>() / denom)
                }
                _ => None,
            };
            match expected {
                None => ("CanvasIndicatorCorrect".into(), true,
                    format!("'{kind}' is recursive/non-window — correctness oracle not applicable (skipped)")),
                Some(exp) => {
                    let tol = (exp.abs() * rel).max(1e-3);
                    let pass = (exp - actual).abs() <= tol;
                    ("CanvasIndicatorCorrect".into(), pass,
                     if pass { format!("pane {pane} {kind}({period}) = {actual:.4} matches recompute {exp:.4} (±{tol:.4})") }
                     else    { format!("pane {pane} {kind}({period}) = {actual:.4} but recompute = {exp:.4} (diff {:.4} > tol {tol:.4})", (exp-actual).abs()) })
                }
            }
        }

        // ── Watchlist % column (functional correctness) ────────────────────────
        // Every loaded watchlist row must carry a server-computed change_perc.
        // Catches the "watchlist % missing/blank" class.
        "watchlist_pct_present" => {
            let sections = state.app_state["watchlist"]["sections"].as_array();
            let mut loaded = 0usize; let mut missing: Vec<String> = Vec::new();
            if let Some(secs) = sections {
                for s in secs {
                    if let Some(items) = s["items"].as_array() {
                        for it in items {
                            if it["loaded"].as_bool().unwrap_or(false) {
                                loaded += 1;
                                if it["change_perc"].is_null() {
                                    missing.push(it["symbol"].as_str().unwrap_or("?").to_string());
                                }
                            }
                        }
                    }
                }
            }
            let pass = missing.is_empty();
            ("WatchlistPctPresent".into(), pass,
             if loaded == 0 { "no loaded watchlist rows yet (vacuous pass)".into() }
             else if pass { format!("all {loaded} loaded row(s) have a change_perc") }
             else { format!("{}/{loaded} loaded row(s) missing change_perc: {}", missing.len(), missing.join(", ")) })
        }
        // change_perc must be within a sane band — catches the "% completely wrong"
        // class (e.g. ±100% minute-bar artifacts, bad ref price).
        "watchlist_pct_sane" => {
            let max = val.as_f64().or_else(|| val["max_abs"].as_f64()).unwrap_or(50.0);
            let sections = state.app_state["watchlist"]["sections"].as_array();
            let mut checked = 0usize; let mut bad: Vec<String> = Vec::new();
            if let Some(secs) = sections {
                for s in secs {
                    if let Some(items) = s["items"].as_array() {
                        for it in items {
                            if let Some(p) = it["change_perc"].as_f64() {
                                checked += 1;
                                if !p.is_finite() || p.abs() > max {
                                    bad.push(format!("{}={:.2}%", it["symbol"].as_str().unwrap_or("?"), p));
                                }
                            }
                        }
                    }
                }
            }
            let pass = bad.is_empty();
            ("WatchlistPctSane".into(), pass,
             if checked == 0 { "no rows with a change_perc yet (vacuous pass)".into() }
             else if pass { format!("all {checked} change_perc value(s) within ±{max}%") }
             else { format!("{} value(s) outside ±{max}%: {}", bad.len(), bad.join(", ")) })
        }

        // ── Options/gamma overlays (functional correctness, not just no-crash) ──
        // When the gamma overlay is enabled it must actually have levels to draw.
        // This is the assertion the original "gamma levels don't appear" bug needed:
        // it fails if the flag is on but no gamma data populated.
        "gamma_overlay_active" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            match state.canvas.panes.get(pane).map(|p| &p.overlays) {
                None => ("GammaOverlayActive".into(), false, format!("pane {pane} not found")),
                Some(o) => {
                    let pass = o.show_gamma && o.gamma_level_count > 0;
                    ("GammaOverlayActive".into(), pass,
                     if pass { format!("pane {pane} gamma ON with {} level(s) (call_wall {:.2}, put_wall {:.2}, zero {:.2})",
                                        o.gamma_level_count, o.gamma_call_wall, o.gamma_put_wall, o.gamma_zero) }
                     else if !o.show_gamma { format!("pane {pane} gamma overlay is OFF") }
                     else { format!("pane {pane} gamma ON but 0 levels populated (overlay would be blank)") })
                }
            }
        }
        // When the strikes overlay is enabled it must have option-chain rows.
        "strikes_overlay_active" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            match state.canvas.panes.get(pane).map(|p| &p.overlays) {
                None => ("StrikesOverlayActive".into(), false, format!("pane {pane} not found")),
                Some(o) => {
                    let rows = o.strikes_call_count + o.strikes_put_count;
                    let pass = o.show_strikes_overlay && rows > 0;
                    ("StrikesOverlayActive".into(), pass,
                     if pass { format!("pane {pane} strikes ON with {} call / {} put row(s) [{}]",
                                        o.strikes_call_count, o.strikes_put_count, o.overlay_chain_symbol) }
                     else if !o.show_strikes_overlay { format!("pane {pane} strikes overlay is OFF") }
                     else if o.overlay_chain_loading { format!("pane {pane} strikes ON but chain still loading") }
                     else { format!("pane {pane} strikes ON but 0 chain rows (overlay would be blank)") })
                }
            }
        }

        // The strike pills must be showing REAL quotes, not a synthesized chain.
        //
        // `strikes_overlay_active` only counts rows, so it passes happily on a
        // fully fabricated chain — which is exactly the state the terminal falls
        // into when ApexData is unreachable (fetch retries, then
        // send_placeholder_chain synthesizes Black-Scholes bids/asks). On a
        // trading surface "populated" and "real" are not the same assertion.
        "strikes_are_real" => {
            let pane = val["pane"].as_u64().unwrap_or(0) as usize;
            match state.canvas.panes.get(pane).map(|p| &p.overlays) {
                None => ("StrikesAreReal".into(), false, format!("pane {pane} not found")),
                Some(o) => {
                    let rows = o.strikes_call_count + o.strikes_put_count;
                    let pass = o.show_strikes_overlay
                        && rows > 0
                        && !o.overlay_chain_placeholder;
                    let why = if pass {
                        format!("pane {pane} strikes REAL: {rows} row(s) from upstream [{}]",
                                o.overlay_chain_symbol)
                    } else if !o.show_strikes_overlay {
                        format!("pane {pane} strikes overlay is OFF")
                    } else if o.overlay_chain_placeholder {
                        format!("pane {pane} strikes are SYNTHESIZED (upstream unreachable) —                                  {rows} fabricated row(s), not quotes")
                    } else {
                        format!("pane {pane} strikes ON but 0 chain rows")
                    };
                    ("StrikesAreReal".into(), pass, why)
                }
            }
        }

        // ── UX / usability audit (one bundled check) ───────────────────────────
        // Clipping + sub-minimum touch targets + overlapping interactive widgets.
        // The widget tree lets the harness "see" usability problems a user would hit.
        "ux_audit" => {
            // 28px is a *touch* guideline; this is a mouse-driven desktop terminal
            // with intentionally dense icon/status buttons (e.g. the 20px connection
            // dot), so the default floor is 16px — flag only genuinely tiny targets.
            // Clipping and overlap remain hard checks (real defects either way).
            let min_px = val["min_touch_px"].as_f64().unwrap_or(16.0) as f32;
            let mut issues: Vec<String> = Vec::new();
            let clipped: Vec<_> = state.widget_tree.iter()
                .filter(|w| w.is_clipped).map(|w| w.id.as_str()).collect();
            if !clipped.is_empty() { issues.push(format!("clipped: {}", clipped.join(", "))); }
            let small: Vec<_> = state.widget_tree.iter()
                .filter(|w| (w.role == "button" || w.role == "input")
                         && w.rect.area() > 0.0 && w.rect.min_side() < min_px)
                .map(|w| format!("{} ({:.0}px)", w.id, w.rect.min_side())).collect();
            if !small.is_empty() { issues.push(format!("touch<{min_px}px: {}", small.join(", "))); }
            // `!w.ticker`: marquee cells slide THROUGH each other's slots during
            // the tape animation, so a transient overlap there is the design, not
            // a usability defect. Everything else is still checked.
            let btns: Vec<_> = state.widget_tree.iter()
                .filter(|w| w.role == "button" && w.layer == 0 && w.rect.area() > 0.0 && !w.ticker)
                .collect();
            let mut ov = Vec::new();
            for i in 0..btns.len() {
                for j in (i + 1)..btns.len() {
                    let (a, b) = (&btns[i].rect, &btns[j].rect);
                    if a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y {
                        ov.push(format!("{}~{}", btns[i].id, btns[j].id));
                    }
                }
            }
            if !ov.is_empty() { issues.push(format!("overlap: {}", ov.join(", "))); }
            let pass = issues.is_empty();
            ("UxAudit".into(), pass,
             if pass { format!("UX clean across {} widgets", state.widget_tree.len()) }
             else { issues.join(" | ") })
        }

        _ => ("Unknown".into(), false, format!("unknown assertion kind: '{key}'")),
    }
}

/// Navigate a dot-path like `"panes.0.symbol"` into a serde_json::Value.
fn json_path(root: &Value, path: &str) -> Value {
    let mut cur = root;
    let placeholder = Value::Null;
    for segment in path.split('.') {
        cur = if let Ok(idx) = segment.parse::<usize>() {
            cur.get(idx).unwrap_or(&placeholder)
        } else {
            cur.get(segment).unwrap_or(&placeholder)
        };
    }
    cur.clone()
}

fn widget_field(w: &WidgetRecord, field: &str) -> Value {
    match field {
        "id"         => Value::String(w.id.clone()),
        "role"       => Value::String(w.role.clone()),
        "label"      => Value::String(w.label.clone()),
        "value"      => w.value.as_ref().map(|v| Value::String(v.clone())).unwrap_or(Value::Null),
        "enabled"    => Value::Bool(w.enabled),
        "focused"    => Value::Bool(w.focused),
        "hovered"    => Value::Bool(w.hovered),
        "is_clipped" => Value::Bool(w.is_clipped),
        "style_class"=> w.style_class.as_ref().map(|s| Value::String(s.clone())).unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

// ─── Layout assertions ────────────────────────────────────────────────────────

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
            let base = edge_value(rects[0].1, edge);
            let misaligned: Vec<_> = rects.iter().filter(|(_, r)| {
                (edge_value(r, edge) - base).abs() > tol
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
            let gap = ((wb.rect.x - (wa.rect.x + wa.rect.w)).abs())
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
            let id = val["widget"].as_str()
                .or_else(|| val.as_str())
                .unwrap_or("");
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
            let id      = val["widget"].as_str()
                .or_else(|| val.as_str())
                .unwrap_or("");
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

        "toolbar_height_consistent" => {
            // All widgets with style_class=="toolbar" or id starting with "toolbar."
            // must have the same height within tolerance_px (default 2px).
            let tol = val.as_f64()
                .or_else(|| val["tolerance_px"].as_f64())
                .unwrap_or(2.0) as f32;
            let toolbar_ws: Vec<_> = widgets.iter()
                .filter(|w| {
                    let by_class = w.style_class.as_deref() == Some("toolbar");
                    let by_id    = w.id.starts_with("toolbar.");
                    (by_class || by_id) && w.rect.area() > 0.0
                })
                .collect();
            if toolbar_ws.len() < 2 {
                return ("ToolbarHeightConsistent".into(), true,
                    format!("fewer than 2 toolbar widgets ({}), nothing to check", toolbar_ws.len()));
            }
            let h0 = toolbar_ws[0].rect.h;
            let inconsistent: Vec<_> = toolbar_ws.iter()
                .filter(|w| (w.rect.h - h0).abs() > tol)
                .map(|w| format!("{} ({:.0}px)", w.id, w.rect.h))
                .collect();
            let pass = inconsistent.is_empty();
            ("ToolbarHeightConsistent".into(), pass,
             if pass { format!("{} toolbar widgets consistent height ~{h0:.0}px", toolbar_ws.len()) }
             else    { format!("toolbar height mismatch (expected ~{h0:.0}px): {}", inconsistent.join(", ")) })
        }

        "spacing_between" => {
            // Alias for gap_between with same semantics; kept separate so scenario authors
            // can express intent as "spacing" rather than "gap".
            let a_id   = val["a"].as_str().unwrap_or("");
            let b_id   = val["b"].as_str().unwrap_or("");
            let min_px = val["min_px"].as_f64().unwrap_or(0.0) as f32;
            let max_px = val["max_px"].as_f64().unwrap_or(f64::MAX) as f32;
            let (wa, wb) = match (find_widget(widgets, a_id), find_widget(widgets, b_id)) {
                (Some(a), Some(b)) => (a, b),
                _ => return ("SpacingBetween".into(), false, format!("widget '{a_id}' or '{b_id}' not found")),
            };
            let gap = ((wb.rect.x - (wa.rect.x + wa.rect.w)).abs())
                .min((wb.rect.y - (wa.rect.y + wa.rect.h)).abs());
            let pass = gap >= min_px && gap <= max_px;
            ("SpacingBetween".into(), pass,
             if pass { format!("spacing {gap:.1}px in [{min_px},{max_px}]") }
             else    { format!("spacing {gap:.1}px outside [{min_px},{max_px}]") })
        }

        "toolbar_buttons_aligned" => {
            // Convenience: all "button" role widgets share top edge within tolerance
            let tol = val.as_f64()
                .or_else(|| val["tolerance_px"].as_f64())
                .unwrap_or(2.0) as f32;
            let buttons: Vec<_> = widgets.iter()
                .filter(|w| w.role == "button" && w.rect.area() > 0.0)
                .collect();
            if buttons.len() < 2 {
                return ("ToolbarButtonsAligned".into(), true,
                    format!("fewer than 2 buttons ({}), nothing to check", buttons.len()));
            }
            let base_y = buttons[0].rect.y;
            let misaligned: Vec<_> = buttons.iter()
                .filter(|w| (w.rect.y - base_y).abs() > tol)
                .map(|w| w.id.as_str())
                .collect();
            let pass = misaligned.is_empty();
            ("ToolbarButtonsAligned".into(), pass,
             if pass { format!("{} buttons aligned (y ≈ {base_y:.0})", buttons.len()) }
             else    { format!("misaligned buttons: {}", misaligned.join(", ")) })
        }

        _ => ("Unknown".into(), false, format!("unknown layout assertion: '{key}'")),
    }
}

fn edge_value(r: &SerRect, edge: &str) -> f32 {
    match edge {
        "top"      => r.y,
        "bottom"   => r.y + r.h,
        "left"     => r.x,
        "right"    => r.x + r.w,
        "center_y" => r.y + r.h / 2.0,
        "center_x" => r.x + r.w / 2.0,
        _          => r.y,
    }
}
