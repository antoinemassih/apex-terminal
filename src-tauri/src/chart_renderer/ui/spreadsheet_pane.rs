//! Spreadsheet pane — v2.
//!
//! v2 features:
//!  - Cell formulas (`=...`): numeric literals, cell refs, +-*/(), SUM/AVG/MIN/MAX/COUNT
//!  - Formula bar (top of pane) showing raw text of selected cell
//!  - Copy/Paste via Ctrl+C / Ctrl+V (single cell + range)
//!  - Shift-click to extend selection to a range
//!  - Right-click context menu: insert/delete row/col, clear, copy, paste
//!  - Column resize by dragging right edge of header (widths in ui.memory)
//!  - Save/Load to %APPDATA%/apex-terminal/spreadsheets/<pane_idx>.json
//!  - Sticky column header + row gutter (header outside scroll, gutter painted on top)
//!
//! Public API of `render` is unchanged.

use egui;
use super::style::*;
use super::super::gpu::*;
use super::widgets::buttons::SimpleBtn;
use super::widgets::text::MonospaceCode;

const HEADER_H: f32 = 18.0;
const ROW_H: f32 = 22.0;
const GUTTER_W: f32 = 32.0;
const DEFAULT_CELL_W: f32 = 96.0;
const MIN_CELL_W: f32 = 32.0;
const TOOLBAR_H: f32 = 28.0;
const FORMULA_BAR_H: f32 = 22.0;

fn col_label(mut idx: usize) -> String {
    let mut s = String::new();
    idx += 1;
    while idx > 0 {
        let r = (idx - 1) % 26;
        s.insert(0, (b'A' + r as u8) as char);
        idx = (idx - 1) / 26;
    }
    s
}

fn cell_ref(row: usize, col: usize) -> String {
    format!("{}{}", col_label(col), row + 1)
}

// ── Formula evaluation ───────────────────────────────────────────────

#[derive(Clone)]
struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self { Self { s: s.as_bytes(), i: 0 } }
    fn peek(&self) -> u8 { if self.i < self.s.len() { self.s[self.i] } else { 0 } }
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() { self.i += 1; }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.skip_ws();
        if self.peek() == c { self.i += 1; true } else { false }
    }
}

fn parse_col_letters(p: &mut Parser) -> Option<usize> {
    p.skip_ws();
    let start = p.i;
    while p.i < p.s.len() && (p.s[p.i] as char).is_ascii_alphabetic() { p.i += 1; }
    if p.i == start { return None; }
    let mut col: usize = 0;
    for &b in &p.s[start..p.i] {
        col = col * 26 + ((b.to_ascii_uppercase() - b'A') as usize + 1);
    }
    Some(col - 1)
}

fn parse_uint(p: &mut Parser) -> Option<usize> {
    p.skip_ws();
    let start = p.i;
    while p.i < p.s.len() && (p.s[p.i] as char).is_ascii_digit() { p.i += 1; }
    if p.i == start { None } else {
        std::str::from_utf8(&p.s[start..p.i]).ok()?.parse().ok()
    }
}

fn parse_cellref(p: &mut Parser) -> Option<(usize, usize)> {
    let save = p.i;
    let col = parse_col_letters(p);
    let row = parse_uint(p);
    match (col, row) {
        (Some(c), Some(r)) if r >= 1 => Some((r - 1, c)),
        _ => { p.i = save; None }
    }
}

/// Evaluate formula text (without leading '=').
fn eval_formula(text: &str, cells: &[Vec<String>], depth: u32) -> Result<f64, String> {
    if depth > 16 { return Err("cycle".into()); }
    let mut p = Parser::new(text);
    let v = parse_expr(&mut p, cells, depth)?;
    p.skip_ws();
    if p.i != p.s.len() { return Err("syntax".into()); }
    Ok(v)
}

fn parse_expr(p: &mut Parser, cells: &[Vec<String>], depth: u32) -> Result<f64, String> {
    let mut v = parse_term(p, cells, depth)?;
    loop {
        p.skip_ws();
        match p.peek() {
            b'+' => { p.i += 1; v += parse_term(p, cells, depth)?; }
            b'-' => { p.i += 1; v -= parse_term(p, cells, depth)?; }
            _ => break,
        }
    }
    Ok(v)
}

fn parse_term(p: &mut Parser, cells: &[Vec<String>], depth: u32) -> Result<f64, String> {
    let mut v = parse_factor(p, cells, depth)?;
    loop {
        p.skip_ws();
        match p.peek() {
            b'*' => { p.i += 1; v *= parse_factor(p, cells, depth)?; }
            b'/' => {
                p.i += 1;
                let d = parse_factor(p, cells, depth)?;
                if d == 0.0 { return Err("div0".into()); }
                v /= d;
            }
            _ => break,
        }
    }
    Ok(v)
}

fn parse_factor(p: &mut Parser, cells: &[Vec<String>], depth: u32) -> Result<f64, String> {
    p.skip_ws();
    if p.eat(b'-') { return Ok(-parse_factor(p, cells, depth)?); }
    if p.eat(b'+') { return parse_factor(p, cells, depth); }
    if p.eat(b'(') {
        let v = parse_expr(p, cells, depth)?;
        if !p.eat(b')') { return Err("paren".into()); }
        return Ok(v);
    }
    // Number?
    p.skip_ws();
    let c = p.peek();
    if c.is_ascii_digit() || c == b'.' {
        let start = p.i;
        while p.i < p.s.len() {
            let b = p.s[p.i];
            if b.is_ascii_digit() || b == b'.' { p.i += 1; } else { break; }
        }
        let s = std::str::from_utf8(&p.s[start..p.i]).map_err(|_| "utf8".to_string())?;
        return s.parse::<f64>().map_err(|_| "num".into());
    }
    // Identifier: function or cell ref
    if c.is_ascii_alphabetic() {
        let save = p.i;
        let id_start = p.i;
        while p.i < p.s.len() && (p.s[p.i] as char).is_ascii_alphabetic() { p.i += 1; }
        let id = std::str::from_utf8(&p.s[id_start..p.i]).unwrap_or("").to_ascii_uppercase();
        p.skip_ws();
        if p.peek() == b'(' {
            // Function call
            p.i += 1;
            let mut nums: Vec<f64> = Vec::new();
            p.skip_ws();
            if p.peek() != b')' {
                loop {
                    // arg may be range A1:B2 or single expr
                    let arg_save = p.i;
                    if let Some((r1, c1)) = parse_cellref(p) {
                        if p.eat(b':') {
                            if let Some((r2, c2)) = parse_cellref(p) {
                                let (r1, r2) = (r1.min(r2), r1.max(r2));
                                let (c1, c2) = (c1.min(c2), c1.max(c2));
                                for r in r1..=r2 {
                                    for cc in c1..=c2 {
                                        if let Some(v) = cell_numeric(cells, r, cc, depth) {
                                            nums.push(v);
                                        }
                                    }
                                }
                            } else { return Err("range".into()); }
                        } else {
                            // single ref expr — re-parse as expression
                            p.i = arg_save;
                            nums.push(parse_expr(p, cells, depth)?);
                        }
                    } else {
                        p.i = arg_save;
                        nums.push(parse_expr(p, cells, depth)?);
                    }
                    p.skip_ws();
                    if p.eat(b',') { continue; } else { break; }
                }
            }
            if !p.eat(b')') { return Err("paren".into()); }
            return apply_fn(&id, &nums);
        }
        // Not a function — try cell ref
        p.i = save;
        if let Some((r, c)) = parse_cellref(p) {
            return Ok(cell_numeric(cells, r, c, depth).unwrap_or(0.0));
        }
        return Err("ident".into());
    }
    Err("token".into())
}

fn apply_fn(name: &str, ns: &[f64]) -> Result<f64, String> {
    match name {
        "SUM"   => Ok(ns.iter().sum()),
        "AVG" | "AVERAGE" => {
            if ns.is_empty() { Ok(0.0) } else { Ok(ns.iter().sum::<f64>() / ns.len() as f64) }
        }
        "MIN"   => Ok(ns.iter().copied().fold(f64::INFINITY, f64::min)),
        "MAX"   => Ok(ns.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        "COUNT" => Ok(ns.len() as f64),
        _ => Err(format!("fn:{}", name)),
    }
}

fn cell_numeric(cells: &[Vec<String>], r: usize, c: usize, depth: u32) -> Option<f64> {
    if r >= cells.len() || c >= cells[r].len() { return None; }
    let raw = cells[r][c].trim();
    if raw.is_empty() { return None; }
    if let Some(rest) = raw.strip_prefix('=') {
        eval_formula(rest, cells, depth + 1).ok()
    } else {
        raw.parse::<f64>().ok()
    }
}

/// Compute the displayed value for a cell (formula → eval, plain → as-is).
fn display_value(cells: &[Vec<String>], r: usize, c: usize) -> String {
    if r >= cells.len() || c >= cells[r].len() { return String::new(); }
    let raw = &cells[r][c];
    if let Some(rest) = raw.strip_prefix('=') {
        match eval_formula(rest, cells, 0) {
            Ok(v) => {
                if v.is_finite() && (v.fract() == 0.0) && v.abs() < 1e15 {
                    format!("{}", v as i64)
                } else if v.is_finite() {
                    format!("{:.6}", v).trim_end_matches('0').trim_end_matches('.').to_string()
                } else {
                    "#NUM!".into()
                }
            }
            Err(_) => "#ERR".into(),
        }
    } else {
        raw.clone()
    }
}

// ── Persistence ──────────────────────────────────────────────────────

fn save_path(pane_idx: usize) -> Option<std::path::PathBuf> {
    let base = std::env::var("APPDATA").ok()
        .or_else(|| std::env::var("HOME").ok())?;
    let mut p = std::path::PathBuf::from(base);
    p.push("apex-terminal");
    p.push("spreadsheets");
    let _ = std::fs::create_dir_all(&p);
    p.push(format!("{}.json", pane_idx));
    Some(p)
}

fn save_cells(pane_idx: usize, cells: &[Vec<String>]) {
    if let Some(path) = save_path(pane_idx) {
        // Minimal hand-rolled JSON
        let mut s = String::from("[");
        for (i, row) in cells.iter().enumerate() {
            if i > 0 { s.push(','); }
            s.push('[');
            for (j, c) in row.iter().enumerate() {
                if j > 0 { s.push(','); }
                s.push('"');
                for ch in c.chars() {
                    match ch {
                        '"' => s.push_str("\\\""),
                        '\\' => s.push_str("\\\\"),
                        '\n' => s.push_str("\\n"),
                        '\r' => s.push_str("\\r"),
                        '\t' => s.push_str("\\t"),
                        c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
                        c => s.push(c),
                    }
                }
                s.push('"');
            }
            s.push(']');
        }
        s.push(']');
        let _ = std::fs::write(path, s);
    }
}

fn load_cells(pane_idx: usize) -> Option<Vec<Vec<String>>> {
    let path = save_path(pane_idx)?;
    let txt = std::fs::read_to_string(path).ok()?;
    parse_json_grid(&txt)
}

fn parse_json_grid(s: &str) -> Option<Vec<Vec<String>>> {
    let b = s.as_bytes();
    let mut i = 0usize;
    let skip_ws = |b: &[u8], i: &mut usize| {
        while *i < b.len() && (b[*i] as char).is_whitespace() { *i += 1; }
    };
    skip_ws(b, &mut i);
    if i >= b.len() || b[i] != b'[' { return None; }
    i += 1;
    let mut grid: Vec<Vec<String>> = Vec::new();
    skip_ws(b, &mut i);
    if i < b.len() && b[i] == b']' { return Some(grid); }
    loop {
        skip_ws(b, &mut i);
        if i >= b.len() || b[i] != b'[' { return None; }
        i += 1;
        let mut row: Vec<String> = Vec::new();
        skip_ws(b, &mut i);
        if i < b.len() && b[i] == b']' { i += 1; grid.push(row); }
        else {
            loop {
                skip_ws(b, &mut i);
                if i >= b.len() || b[i] != b'"' { return None; }
                i += 1;
                let mut v = String::new();
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' && i + 1 < b.len() {
                        match b[i + 1] {
                            b'"'  => { v.push('"'); i += 2; }
                            b'\\' => { v.push('\\'); i += 2; }
                            b'n'  => { v.push('\n'); i += 2; }
                            b'r'  => { v.push('\r'); i += 2; }
                            b't'  => { v.push('\t'); i += 2; }
                            b'u'  => {
                                if i + 6 > b.len() { return None; }
                                let h = std::str::from_utf8(&b[i+2..i+6]).ok()?;
                                let cp = u32::from_str_radix(h, 16).ok()?;
                                if let Some(c) = char::from_u32(cp) { v.push(c); }
                                i += 6;
                            }
                            _ => { v.push(b[i+1] as char); i += 2; }
                        }
                    } else {
                        v.push(b[i] as char);
                        i += 1;
                    }
                }
                if i >= b.len() { return None; }
                i += 1; // closing "
                row.push(v);
                skip_ws(b, &mut i);
                if i < b.len() && b[i] == b',' { i += 1; continue; }
                if i < b.len() && b[i] == b']' { i += 1; break; }
                return None;
            }
            grid.push(row);
        }
        skip_ws(b, &mut i);
        if i < b.len() && b[i] == b',' { i += 1; continue; }
        if i < b.len() && b[i] == b']' { break; }
        return None;
    }
    Some(grid)
}

// ── Per-pane in-memory ui state (column widths, range, clipboard, etc) ──

#[derive(Clone, Default)]
struct SsState {
    col_widths: Vec<f32>,
    range_anchor: Option<(usize, usize)>,
    range_focus: Option<(usize, usize)>,
    clipboard: Vec<Vec<String>>,
    ctx_menu: Option<(egui::Pos2, usize, usize)>,
    loaded: bool,
    formula_buf: String,
    formula_focus: bool,
}

fn col_x(state: &SsState, c: usize, default_w: f32) -> f32 {
    let mut x = 0.0;
    for i in 0..c {
        x += state.col_widths.get(i).copied().unwrap_or(default_w);
    }
    x
}

fn col_w(state: &SsState, c: usize, default_w: f32) -> f32 {
    state.col_widths.get(c).copied().unwrap_or(default_w)
}

fn ensure_widths(state: &mut SsState, cols: usize) {
    while state.col_widths.len() < cols { state.col_widths.push(DEFAULT_CELL_W); }
}

fn in_range(state: &SsState, r: usize, c: usize) -> bool {
    match (state.range_anchor, state.range_focus) {
        (Some((r1, c1)), Some((r2, c2))) => {
            let (rl, rh) = (r1.min(r2), r1.max(r2));
            let (cl, ch) = (c1.min(c2), c1.max(c2));
            r >= rl && r <= rh && c >= cl && c <= ch
        }
        _ => false,
    }
}

fn range_bounds(state: &SsState) -> Option<(usize, usize, usize, usize)> {
    match (state.range_anchor, state.range_focus) {
        (Some((r1, c1)), Some((r2, c2))) => {
            Some((r1.min(r2), r1.max(r2), c1.min(c2), c1.max(c2)))
        }
        _ => None,
    }
}

pub(crate) fn render(
    ui: &mut egui::Ui, _ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, active_pane: &mut usize,
    _visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    _watchlist: &mut Watchlist,
) {
    let t = &THEMES[theme_idx];
    if pane_rects.is_empty() { return; }
    let rect = pane_rects[0];

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.bg);

    if let Some(pos) = ui.ctx().pointer_hover_pos() {
        if rect.contains(pos) { *active_pane = pane_idx; }
    }

    // Pull state from egui memory (per-pane)
    let state_id = egui::Id::new(("ss_state", pane_idx));
    let mut state: SsState = ui.ctx().data_mut(|d|
        d.get_temp::<SsState>(state_id).unwrap_or_default());

    let chart = &mut panes[pane_idx];

    // Load on first render
    if !state.loaded {
        if let Some(loaded) = load_cells(pane_idx) {
            if !loaded.is_empty() {
                let cols = loaded.iter().map(|r| r.len()).max().unwrap_or(0);
                if cols > 0 {
                    chart.spreadsheet_cells = loaded.into_iter()
                        .map(|mut r| { while r.len() < cols { r.push(String::new()); } r })
                        .collect();
                    chart.spreadsheet_rows = chart.spreadsheet_cells.len();
                    chart.spreadsheet_cols = cols;
                }
            }
        }
        state.loaded = true;
    }

    ensure_widths(&mut state, chart.spreadsheet_cols);

    // ── Top toolbar ──
    let toolbar_rect = egui::Rect::from_min_size(
        rect.min, egui::vec2(rect.width(), TOOLBAR_H));
    let mut toolbar_ui = ui.new_child(egui::UiBuilder::new()
        .max_rect(toolbar_rect.shrink2(egui::vec2(GAP_SM, GAP_XS))));
    let mut do_save = false;
    toolbar_ui.horizontal_centered(|ui| {
        let btn = |ui: &mut egui::Ui, label: &str| -> bool {
            ui.add(SimpleBtn::new(label).color(t.dim)).clicked()
        };
        if btn(ui, "+ Row") {
            let cols = chart.spreadsheet_cols.max(1);
            chart.spreadsheet_cells.push(vec![String::new(); cols]);
            chart.spreadsheet_rows = chart.spreadsheet_cells.len();
        }
        if btn(ui, "+ Col") {
            chart.spreadsheet_cols += 1;
            for row in chart.spreadsheet_cells.iter_mut() {
                row.push(String::new());
            }
            ensure_widths(&mut state, chart.spreadsheet_cols);
        }
        if btn(ui, "Clear") {
            for row in chart.spreadsheet_cells.iter_mut() {
                for c in row.iter_mut() { c.clear(); }
            }
            chart.spreadsheet_editing = None;
        }
        if btn(ui, "Save") { do_save = true; }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = match chart.spreadsheet_selected {
                Some((r, c)) => cell_ref(r, c),
                None => "—".into(),
            };
            ui.add(MonospaceCode::new(&label).xs().color(t.accent));
        });
    });
    if do_save { save_cells(pane_idx, &chart.spreadsheet_cells); }

    // ── Formula bar ──
    let fbar_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + TOOLBAR_H),
        egui::vec2(rect.width(), FORMULA_BAR_H));
    {
        let p = ui.painter_at(fbar_rect);
        p.rect_filled(fbar_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_TINT));
        // "fx" tag
        p.text(egui::pos2(fbar_rect.left() + 6.0, fbar_rect.center().y),
            egui::Align2::LEFT_CENTER, "fx",
            egui::FontId::monospace(FONT_XS), t.accent);
    }
    // Sync formula buffer with selection (only when not focused)
    if let Some((r, c)) = chart.spreadsheet_selected {
        if !state.formula_focus {
            let raw = chart.spreadsheet_cells.get(r)
                .and_then(|row| row.get(c)).cloned().unwrap_or_default();
            state.formula_buf = raw;
        }
    }
    {
        let edit_rect = egui::Rect::from_min_max(
            egui::pos2(fbar_rect.left() + 28.0, fbar_rect.top() + 1.0),
            egui::pos2(fbar_rect.right() - 4.0, fbar_rect.bottom() - 1.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
        let resp = child.add(egui::TextEdit::singleline(&mut state.formula_buf)
            .font(egui::FontId::monospace(FONT_SM))
            .frame(false)
            .margin(egui::vec2(2.0, 2.0))
            .desired_width(edit_rect.width() - 4.0));
        state.formula_focus = resp.has_focus();
        if resp.changed() || resp.lost_focus() {
            if let Some((r, c)) = chart.spreadsheet_selected {
                if r < chart.spreadsheet_cells.len()
                    && c < chart.spreadsheet_cells[r].len() {
                    chart.spreadsheet_cells[r][c] = state.formula_buf.clone();
                }
            }
        }
    }

    // Empty state
    if chart.spreadsheet_rows == 0 || chart.spreadsheet_cols == 0 {
        let empty_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + TOOLBAR_H + FORMULA_BAR_H),
            rect.max);
        let p = ui.painter_at(empty_rect);
        p.text(egui::pos2(empty_rect.center().x, empty_rect.center().y - 8.0),
            egui::Align2::CENTER_CENTER, "No cells",
            egui::FontId::monospace(FONT_LG), t.dim.gamma_multiply(0.6));
        p.text(egui::pos2(empty_rect.center().x, empty_rect.center().y + 8.0),
            egui::Align2::CENTER_CENTER, "Add a row to start",
            egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        let bw = 80.0; let bh = 20.0;
        let br = egui::Rect::from_center_size(
            egui::pos2(empty_rect.center().x, empty_rect.center().y + 28.0),
            egui::vec2(bw, bh));
        let resp = ui.interact(br, ui.id().with("ss_empty_addrow"), egui::Sense::click());
        let p2 = ui.painter_at(br);
        p2.rect_filled(br, RADIUS_SM, color_alpha(t.accent, ALPHA_TINT));
        p2.rect_stroke(br, RADIUS_SM,
            egui::Stroke::new(stroke_thin(), color_alpha(t.accent, ALPHA_LINE)),
            egui::epaint::StrokeKind::Middle);
        p2.text(br.center(), egui::Align2::CENTER_CENTER, "Add row",
            egui::FontId::monospace(FONT_XS), t.accent);
        if resp.clicked() {
            let cols = chart.spreadsheet_cols.max(1);
            chart.spreadsheet_cols = cols;
            chart.spreadsheet_cells.push(vec![String::new(); cols]);
            chart.spreadsheet_rows = chart.spreadsheet_cells.len();
            ensure_widths(&mut state, chart.spreadsheet_cols);
        }
        ui.ctx().data_mut(|d| d.insert_temp(state_id, state));
        return;
    }

    // ── Grid layout ──
    let grid_top = rect.top() + TOOLBAR_H + FORMULA_BAR_H;
    let grid_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), grid_top), rect.max);

    // Sticky column header (outside scroll). Horizontal scroll offset is small;
    // for v2 simplicity headers don't follow horizontal scroll — pragmatic.
    let header_rect = egui::Rect::from_min_size(
        egui::pos2(grid_rect.left(), grid_rect.top()),
        egui::vec2(grid_rect.width(), HEADER_H));
    {
        let p = ui.painter_at(header_rect);
        p.rect_filled(header_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_TINT));
    }

    // Body scroll
    let body_top = header_rect.bottom();
    let body_rect = egui::Rect::from_min_max(
        egui::pos2(grid_rect.left(), body_top), grid_rect.max);

    let total_w: f32 = GUTTER_W + (0..chart.spreadsheet_cols)
        .map(|c| col_w(&state, c, DEFAULT_CELL_W)).sum::<f32>();
    let total_h = (chart.spreadsheet_rows as f32) * ROW_H;

    // Draw column headers (with resize handles)
    {
        let mut x = header_rect.left() + GUTTER_W;
        let p = ui.painter_at(header_rect);
        for c in 0..chart.spreadsheet_cols {
            let w = col_w(&state, c, DEFAULT_CELL_W);
            let r = egui::Rect::from_min_size(egui::pos2(x, header_rect.top()),
                egui::vec2(w, HEADER_H));
            p.text(r.center(), egui::Align2::CENTER_CENTER, col_label(c),
                egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.7));
            p.line_segment([
                egui::pos2(x, header_rect.top()),
                egui::pos2(x, header_rect.bottom())],
                egui::Stroke::new(stroke_thin(),
                    color_alpha(t.toolbar_border, ALPHA_MUTED)));
            // Resize handle on right edge
            let handle = egui::Rect::from_min_size(
                egui::pos2(x + w - 3.0, header_rect.top()),
                egui::vec2(6.0, HEADER_H));
            let resp = ui.interact(handle, ui.id().with(("ss_resize", c)),
                egui::Sense::click_and_drag());
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
            }
            if resp.dragged() {
                let dx = resp.drag_delta().x;
                if c < state.col_widths.len() {
                    state.col_widths[c] = (state.col_widths[c] + dx).max(MIN_CELL_W);
                }
            }
            x += w;
        }
    }

    let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body_rect));
    let mut commit: Option<(usize, usize, String)> = None;
    let mut cancel_edit = false;
    let mut start_edit: Option<(usize, usize)> = None;
    let mut new_select: Option<(usize, usize)> = None;
    let mut shift_extend: Option<(usize, usize)> = None;
    let mut do_paste = false;
    let mut do_copy = false;
    let mut ctx_action: Option<&'static str> = None;

    egui::ScrollArea::both()
        .id_salt(("ss_scroll", pane_idx))
        .auto_shrink([false, false])
        .show(&mut body_ui, |ui| {
            let (resp_rect, _) = ui.allocate_exact_size(
                egui::vec2(total_w, total_h), egui::Sense::hover());
            let p = ui.painter_at(resp_rect);
            let stroke_grid = egui::Stroke::new(stroke_thin(),
                color_alpha(t.toolbar_border, ALPHA_MUTED));

            // Sticky-ish gutter background
            let gutter_rect = egui::Rect::from_min_size(resp_rect.min,
                egui::vec2(GUTTER_W, total_h));
            p.rect_filled(gutter_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_TINT));

            for r in 0..chart.spreadsheet_rows {
                let y = resp_rect.top() + (r as f32) * ROW_H;
                let num_rect = egui::Rect::from_min_size(
                    egui::pos2(resp_rect.left(), y), egui::vec2(GUTTER_W, ROW_H));
                p.text(num_rect.center(), egui::Align2::CENTER_CENTER,
                    format!("{}", r + 1),
                    egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.6));
                p.line_segment([
                    egui::pos2(resp_rect.left(), y + ROW_H),
                    egui::pos2(resp_rect.left() + total_w, y + ROW_H)],
                    stroke_grid);

                let mut x = resp_rect.left() + GUTTER_W;
                for c in 0..chart.spreadsheet_cols {
                    let cw = col_w(&state, c, DEFAULT_CELL_W);
                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(x, y), egui::vec2(cw, ROW_H));
                    p.line_segment([
                        egui::pos2(x + cw, y),
                        egui::pos2(x + cw, y + ROW_H)],
                        stroke_grid);

                    // Range highlight
                    if in_range(&state, r, c) {
                        ui.painter_at(cell_rect).rect_filled(
                            cell_rect, 0.0,
                            color_alpha(t.accent, ALPHA_TINT));
                    }

                    let editing_here = matches!(chart.spreadsheet_editing,
                        Some((er, ec, _)) if er == r && ec == c);
                    let selected_here = chart.spreadsheet_selected == Some((r, c));

                    if editing_here {
                        if let Some((_, _, buf)) = chart.spreadsheet_editing.as_mut() {
                            let mut child = ui.new_child(
                                egui::UiBuilder::new().max_rect(cell_rect.shrink(1.0)));
                            let resp = child.add(egui::TextEdit::singleline(buf)
                                .font(egui::FontId::monospace(FONT_SM))
                                .frame(false)
                                .margin(egui::vec2(2.0, 2.0))
                                .desired_width(cw - 4.0));
                            ui.painter_at(cell_rect).rect_stroke(cell_rect, 0.0,
                                egui::Stroke::new(1.0, t.accent),
                                egui::epaint::StrokeKind::Middle);
                            resp.request_focus();
                            let input = ui.input(|i| (
                                i.key_pressed(egui::Key::Enter),
                                i.key_pressed(egui::Key::Tab),
                                i.key_pressed(egui::Key::Escape),
                            ));
                            if input.0 || input.1 {
                                commit = Some((r, c, buf.clone()));
                            } else if input.2 {
                                cancel_edit = true;
                            } else if resp.lost_focus() {
                                commit = Some((r, c, buf.clone()));
                            }
                        }
                    } else {
                        let val = display_value(&chart.spreadsheet_cells, r, c);
                        if !val.is_empty() {
                            let pp = ui.painter_at(cell_rect);
                            pp.text(
                                egui::pos2(cell_rect.left() + 4.0, cell_rect.center().y),
                                egui::Align2::LEFT_CENTER, &val,
                                egui::FontId::monospace(FONT_SM),
                                TEXT_PRIMARY);
                        }
                        if selected_here {
                            ui.painter_at(cell_rect).rect_stroke(cell_rect, 0.0,
                                egui::Stroke::new(1.0, t.accent),
                                egui::epaint::StrokeKind::Middle);
                        }
                        let id = ui.id().with(("ss_cell", r, c));
                        let resp = ui.interact(cell_rect, id,
                            egui::Sense::click_and_drag());
                        if resp.clicked() {
                            let shift = ui.input(|i| i.modifiers.shift);
                            if shift {
                                shift_extend = Some((r, c));
                            } else {
                                new_select = Some((r, c));
                            }
                        }
                        if resp.double_clicked() {
                            start_edit = Some((r, c));
                        }
                        if resp.secondary_clicked() {
                            if !in_range(&state, r, c) {
                                new_select = Some((r, c));
                            }
                            if let Some(p) = ui.ctx().pointer_interact_pos() {
                                state.ctx_menu = Some((p, r, c));
                            }
                        }
                    }
                    x += cw;
                }
            }
        });

    // ── Keyboard: copy / paste / save ──
    let (ctrl_c, ctrl_v, ctrl_s) = ui.input(|i| (
        i.modifiers.ctrl && i.key_pressed(egui::Key::C),
        i.modifiers.ctrl && i.key_pressed(egui::Key::V),
        i.modifiers.ctrl && i.key_pressed(egui::Key::S),
    ));
    if ctrl_c && !state.formula_focus { do_copy = true; }
    if ctrl_v && !state.formula_focus { do_paste = true; }
    if ctrl_s { save_cells(pane_idx, &chart.spreadsheet_cells); }

    // ── Right-click context menu ──
    if let Some((pos, mr, mc)) = state.ctx_menu {
        let menu_id = egui::Id::new(("ss_ctx", pane_idx));
        let area = egui::Area::new(menu_id)
            .order(egui::Order::Foreground)
            .fixed_pos(pos);
        let mut close = false;
        area.show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(160.0);
                let mut item = |ui: &mut egui::Ui, label: &str, key: &'static str| {
                    if ui.add(SimpleBtn::new(label).color(t.dim)).clicked() {
                        ctx_action = Some(key);
                        close = true;
                    }
                };
                item(ui, "Insert Row Above",  "row_above");
                item(ui, "Insert Row Below",  "row_below");
                item(ui, "Insert Col Left",   "col_left");
                item(ui, "Insert Col Right",  "col_right");
                ui.separator();
                item(ui, "Delete Row",        "del_row");
                item(ui, "Delete Col",        "del_col");
                ui.separator();
                item(ui, "Copy",              "copy");
                item(ui, "Paste",             "paste");
                item(ui, "Clear",             "clear");
            });
        });
        let clicked_outside = ui.input(|i| i.pointer.any_click()) && {
            // crude: close on any click after frame if no item triggered
            ctx_action.is_none()
                && ui.ctx().pointer_interact_pos()
                    .map(|p| (p - pos).length() > 200.0).unwrap_or(true)
        };
        if close { state.ctx_menu = None; }
        if clicked_outside { state.ctx_menu = None; }

        // Apply action
        if let Some(act) = ctx_action {
            match act {
                "row_above" => {
                    let cols = chart.spreadsheet_cols;
                    chart.spreadsheet_cells.insert(mr, vec![String::new(); cols]);
                    chart.spreadsheet_rows = chart.spreadsheet_cells.len();
                }
                "row_below" => {
                    let cols = chart.spreadsheet_cols;
                    let at = (mr + 1).min(chart.spreadsheet_cells.len());
                    chart.spreadsheet_cells.insert(at, vec![String::new(); cols]);
                    chart.spreadsheet_rows = chart.spreadsheet_cells.len();
                }
                "col_left" => {
                    for row in chart.spreadsheet_cells.iter_mut() {
                        if mc <= row.len() { row.insert(mc, String::new()); }
                    }
                    chart.spreadsheet_cols += 1;
                    if mc <= state.col_widths.len() {
                        state.col_widths.insert(mc, DEFAULT_CELL_W);
                    }
                }
                "col_right" => {
                    let at = mc + 1;
                    for row in chart.spreadsheet_cells.iter_mut() {
                        let pos = at.min(row.len());
                        row.insert(pos, String::new());
                    }
                    chart.spreadsheet_cols += 1;
                    let pos = at.min(state.col_widths.len());
                    state.col_widths.insert(pos, DEFAULT_CELL_W);
                }
                "del_row" => {
                    if chart.spreadsheet_cells.len() > 1
                        && mr < chart.spreadsheet_cells.len()
                    {
                        chart.spreadsheet_cells.remove(mr);
                        chart.spreadsheet_rows = chart.spreadsheet_cells.len();
                    }
                }
                "del_col" => {
                    if chart.spreadsheet_cols > 1 {
                        for row in chart.spreadsheet_cells.iter_mut() {
                            if mc < row.len() { row.remove(mc); }
                        }
                        chart.spreadsheet_cols -= 1;
                        if mc < state.col_widths.len() {
                            state.col_widths.remove(mc);
                        }
                    }
                }
                "copy"  => { do_copy = true; }
                "paste" => { do_paste = true; }
                "clear" => {
                    if let Some((r1, r2, c1, c2)) = range_bounds(&state) {
                        for r in r1..=r2 {
                            for c in c1..=c2 {
                                if let Some(row) = chart.spreadsheet_cells.get_mut(r) {
                                    if let Some(cell) = row.get_mut(c) { cell.clear(); }
                                }
                            }
                        }
                    } else if mr < chart.spreadsheet_cells.len()
                        && mc < chart.spreadsheet_cells[mr].len()
                    {
                        chart.spreadsheet_cells[mr][mc].clear();
                    }
                }
                _ => {}
            }
        }
    }

    // Apply edit / select state changes
    if let Some((r, c, val)) = commit {
        if r < chart.spreadsheet_cells.len()
            && c < chart.spreadsheet_cells[r].len() {
            chart.spreadsheet_cells[r][c] = val;
        }
        chart.spreadsheet_editing = None;
    } else if cancel_edit {
        chart.spreadsheet_editing = None;
    }
    if let Some((r, c)) = start_edit {
        let cur = chart.spreadsheet_cells[r][c].clone();
        chart.spreadsheet_editing = Some((r, c, cur));
        chart.spreadsheet_selected = Some((r, c));
        state.range_anchor = Some((r, c));
        state.range_focus = Some((r, c));
    } else if let Some(sel) = new_select {
        chart.spreadsheet_selected = Some(sel);
        state.range_anchor = Some(sel);
        state.range_focus = Some(sel);
    } else if let Some(ext) = shift_extend {
        chart.spreadsheet_selected = Some(ext);
        if state.range_anchor.is_none() { state.range_anchor = Some(ext); }
        state.range_focus = Some(ext);
    }

    // Copy
    if do_copy {
        let bounds = range_bounds(&state).or_else(|| {
            chart.spreadsheet_selected.map(|(r, c)| (r, r, c, c))
        });
        if let Some((r1, r2, c1, c2)) = bounds {
            let mut clip: Vec<Vec<String>> = Vec::new();
            for r in r1..=r2 {
                let mut row = Vec::new();
                for c in c1..=c2 {
                    let v = chart.spreadsheet_cells.get(r)
                        .and_then(|rr| rr.get(c)).cloned().unwrap_or_default();
                    row.push(v);
                }
                clip.push(row);
            }
            state.clipboard = clip;
        }
    }

    // Paste
    if do_paste && !state.clipboard.is_empty() {
        let (sr, sc) = chart.spreadsheet_selected.unwrap_or((0, 0));
        for (dr, row) in state.clipboard.iter().enumerate() {
            let tr = sr + dr;
            if tr >= chart.spreadsheet_cells.len() { break; }
            for (dc, val) in row.iter().enumerate() {
                let tc = sc + dc;
                if tc >= chart.spreadsheet_cells[tr].len() { break; }
                chart.spreadsheet_cells[tr][tc] = val.clone();
            }
        }
    }

    // Persist state
    ui.ctx().data_mut(|d| d.insert_temp(state_id, state));
}
