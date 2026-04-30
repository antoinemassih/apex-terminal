//! Fuzzy matching, prefix parsing, expression eval, chain step resolution.

use super::Category;
use super::registry::*;
use crate::chart_renderer::gpu::Watchlist;

pub(super) fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() { return Some(0); }
    let q = query.to_lowercase();
    let t = target.to_lowercase();
    if t == q { return Some(2000); }
    if t.starts_with(&q) { return Some(1000 - t.len() as i32); }
    if t.contains(&q) { return Some(500 - t.len() as i32); }
    let mut qi = 0;
    let qb = q.as_bytes();
    for c in t.bytes() {
        if qi < qb.len() && c == qb[qi] { qi += 1; }
    }
    if qi == qb.len() { Some(100 - t.len() as i32) } else { None }
}

pub(super) fn parse_prefix(s: &str) -> (Option<Category>, String) {
    if let Some(rest) = s.strip_prefix('>') { return (Some(Category::Command), rest.trim().to_string()); }
    if let Some(rest) = s.strip_prefix('@') { return (Some(Category::Symbol),  rest.trim().to_string()); }
    if let Some(rest) = s.strip_prefix('#') { return (Some(Category::Play),    rest.trim().to_string()); }
    if let Some(rest) = s.strip_prefix('/') { return (Some(Category::Setting), rest.trim().to_string()); }
    (None, s.to_string())
}

pub(super) fn cat_from_label(lbl: &str) -> Option<Category> {
    Some(match lbl {
        "CMD" => Category::Command, "SYM" => Category::Symbol,
        "WIDGET" => Category::Widget, "OVERLAY" => Category::Overlay,
        "THEME" => Category::Theme, "TF" => Category::Timeframe,
        "LAYOUT" => Category::Layout, "PLAY" => Category::Play,
        "ALERT" => Category::Alert, "SETTING" => Category::Setting,
        "AI" => Category::Ai, "DYNAMIC" => Category::Dynamic,
        "HELP" => Category::Help, "CALC" => Category::Calc,
        "RECENT" => Category::Recent,
        _ => return None,
    })
}

/// Resolve a chain step like "5m" / "rsi-multi" / "bauhaus" / "AAPL" to an action id.
pub(super) fn resolve_chain_step(step: &str, watchlist: &Watchlist) -> Option<String> {
    let s = step.trim();
    if s.is_empty() { return None; }

    // Explicit prefixes
    let (cat, body) = parse_prefix(s);
    let body_lc = body.to_lowercase();

    // Timeframe match
    if TF_IDS.iter().any(|&tf| tf.eq_ignore_ascii_case(&body)) {
        return Some(format!("tf:{}", body));
    }
    // Layout id
    if LAYOUT_IDS.iter().any(|(id, _)| id.eq_ignore_ascii_case(&body)) {
        return Some(format!("layout:{}", body.to_uppercase()));
    }
    // Theme
    if THEME_NAMES.iter().any(|n| n.eq_ignore_ascii_case(&body)) {
        return Some(format!("theme:{}", body_lc));
    }
    // Widget id
    if widget_catalog().iter().any(|(_, id, _)| id.eq_ignore_ascii_case(&body)) {
        return Some(format!("widget:{}", body_lc));
    }
    // Overlay id
    if OVERLAY_IDS.iter().any(|(id, _)| id.eq_ignore_ascii_case(&body)) {
        return Some(format!("overlay:{}", body_lc));
    }
    // Explicit sym prefix or uppercase ticker
    if matches!(cat, Some(Category::Symbol)) || (s.len() <= 5 && s.chars().all(|c| c.is_ascii_alphabetic())) {
        return Some(format!("sym:{}", body.to_uppercase()));
    }
    // Command keyword
    if body_lc.contains("flatten") { return Some("cmd:flatten".into()); }
    if body_lc.contains("cancel")  { return Some("cmd:cancel".into()); }
    // Play by title
    if let Some(p) = watchlist.plays.iter().find(|p| p.title.to_lowercase().contains(&body_lc)) {
        return Some(format!("play:{}", p.id));
    }
    None
}

// Simple expression evaluator: supports + - * / with precedence, decimals.
pub(super) fn eval_expr(s: &str) -> Option<f64> {
    // Shunting-yard — very small, no parens
    let mut out: Vec<f64> = Vec::new();
    let mut ops: Vec<char> = Vec::new();
    let prec = |c: char| match c { '+' | '-' => 1, '*' | '/' => 2, _ => 0 };
    let mut iter = s.chars().peekable();
    let apply = |ops: &mut Vec<char>, out: &mut Vec<f64>| -> Option<()> {
        let op = ops.pop()?; let b = out.pop()?; let a = out.pop()?;
        out.push(match op { '+' => a + b, '-' => a - b, '*' => a * b, '/' => a / b, _ => return None });
        Some(())
    };
    while let Some(&c) = iter.peek() {
        if c.is_whitespace() { iter.next(); continue; }
        if c.is_ascii_digit() || c == '.' {
            let mut num = String::new();
            while let Some(&d) = iter.peek() {
                if d.is_ascii_digit() || d == '.' { num.push(d); iter.next(); } else { break; }
            }
            out.push(num.parse().ok()?);
        } else if "+-*/".contains(c) {
            while let Some(&top) = ops.last() {
                if prec(top) >= prec(c) { apply(&mut ops, &mut out)?; } else { break; }
            }
            ops.push(c); iter.next();
        } else { return None; }
    }
    while !ops.is_empty() { apply(&mut ops, &mut out)?; }
    if out.len() == 1 { Some(out[0]) } else { None }
}
