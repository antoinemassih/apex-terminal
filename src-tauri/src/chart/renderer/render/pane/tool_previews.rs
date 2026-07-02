//! Drawing tool previews and custom cursors for the chart pane.
//!
//! Extracted from `core.rs` — interaction-gated (only renders when pointer
//! is inside the pane and a draw tool is active), zero per-frame render cost
//! on frames with no active tool.

#![allow(unused_imports)]

use crate::chart_renderer::gpu::*;
use crate::chart_renderer::ui::style::{
    color_alpha, mono_2xs, mono_xs, mono_3xs,
    stroke_thin, stroke_medium, stroke_std, stroke_bold,
    COLOR_PURPLE, COLOR_PROFIT_GREEN, COLOR_LOSS_RED,
};

/// Render drawing tool preview overlays and set custom cursors.
///
/// Called only when `pointer_in_pane` is true and a hover position exists.
///
/// Generic parameters:
/// - `Py` — price→screen-y mapping closure `|p: f32| -> f32`
/// - `Bx` — bar-index→screen-x mapping closure `|i: f32| -> f32`
///
/// All other parameters are scalars / references read from the enclosing
/// `render_chart_pane` scope:
/// - `ui`            — pane `Ui` (for cursor icon, layout_no_wrap)
/// - `painter`       — pane painter
/// - `t`             — resolved `Theme`
/// - `chart`         — active `Chart` (read-only in preview path)
/// - `rect`          — full pane rect
/// - `cw`/`pt`/`ch`  — chart-body geometry (width, header height, chart height)
/// - `min_p`/`max_p` — visible price range
/// - `off`/`bs`/`vs` — scroll offset, bar size, view-start (for fibtimezone anchor)
/// - `pos`           — current hover position
pub(super) fn render_tool_previews<Py, Bx>(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    t: &Theme,
    chart: &Chart,
    rect: egui::Rect,
    cw: f32,
    pt: f32,
    ch: f32,
    min_p: f32,
    max_p: f32,
    off: f32,
    bs: f32,
    vs: f32,
    pos: egui::Pos2,
    py: &Py,
    bx: &Bx,
) where
    Py: Fn(f32) -> f32,
    Bx: Fn(f32) -> f32,
{
    let blue = egui::Color32::from_rgb(70, 130, 255);
    let clamp_pt = |p: egui::Pos2| -> egui::Pos2 {
        let margin = 10000.0;
        egui::pos2(p.x.clamp(-margin, margin), p.y.clamp(-margin, margin))
    };

    // Tool status notice — rendered ungated from core.rs via pane/notice.rs.
    if chart.draw_tool == "trendline" {
        // Accent-colored crosshair + dashed preview line from first click
        if let Some((b0, p0)) = chart.pending_pt {
            let start = egui::pos2(bx(b0), py(p0));
            let dir = pos - start;
            let len = dir.length();
            if len > 2.0 {
                let dash_len = 6.0; let gap_len = 4.0; let step = dash_len + gap_len;
                let norm = dir / len;
                let mut d = 0.0;
                while d < len {
                    let a = start + norm * d;
                    let b = start + norm * (d + dash_len).min(len);
                    painter.line_segment([a, b], egui::Stroke::new(stroke_bold(), color_alpha(t.accent, 200)));
                    d += step;
                }
            }
            painter.circle_filled(start, 3.0, t.accent);
            painter.circle_filled(pos, 3.0, t.accent);
        }
        // Accent crosshair
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), t.accent));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), t.accent));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "hline" {
        // Blue horizontal line preview following mouse
        if pos.y >= rect.top()+pt && pos.y < rect.top()+pt+ch {
            painter.line_segment(
                [egui::pos2(rect.left(), pos.y), egui::pos2(rect.left()+cw, pos.y)],
                egui::Stroke::new(stroke_std(), color_alpha(blue, 160)),
            );
            // Price label on right edge
            let hp = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
            let price_label = format!("{:.2}", hp);
            painter.text(egui::pos2(rect.left() + cw + 3.0, pos.y), egui::Align2::LEFT_CENTER, &price_label, mono_2xs(), blue);
        }
        // Blue crosshair
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), blue));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), blue));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "hzone" {
        if let Some((_b0, p0)) = chart.pending_pt {
            // Rectangle preview from first click to cursor
            let y0 = py(p0);
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(rect.left(), y0.min(pos.y)), egui::pos2(rect.left()+cw, y0.max(pos.y))),
                0.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15));
            painter.line_segment([egui::pos2(rect.left(),y0),egui::pos2(rect.left()+cw,y0)], egui::Stroke::new(stroke_std(), color_alpha(t.text,120)));
            painter.line_segment([egui::pos2(rect.left(),pos.y),egui::pos2(rect.left()+cw,pos.y)], egui::Stroke::new(stroke_std(), color_alpha(t.text,120)));
        }
        // Rectangle cursor — uses contrast_fg so it stays visible on both
        // dark (white) and light (black) chart backgrounds.
        let sz = 6.0;
        let cursor_col = crate::ui_kit::style::contrast_fg(t.bg);
        painter.rect_stroke(
            egui::Rect::from_center_size(pos, egui::vec2(sz * 2.0, sz * 2.0)),
            1.0, egui::Stroke::new(stroke_std(), cursor_col), egui::StrokeKind::Outside);
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "fibonacci" {
        // Fibonacci preview: retracement + extension levels
        let fib_color = egui::Color32::from_rgb(255, 193, 37); // gold
        if let Some((b0, p0)) = chart.pending_pt {
            let price_cursor = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
            let x0 = bx(b0); let x1 = pos.x;
            let xl = x0.min(x1); let xr = x0.max(x1);
            let range = price_cursor - p0;
            // Retracement levels
            let retrace = [0.0_f32, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];
            // Extension levels
            let extensions = [-0.272_f32, -0.618, 1.272, 1.414, 1.618, 2.0, 2.618, 3.146];
            for &lv in retrace.iter() {
                let lp = p0 + range * lv;
                let y = py(lp);
                if y.is_finite() && y.abs() < 50000.0 {
                    painter.line_segment([egui::pos2(xl, y), egui::pos2(xr, y)],
                        egui::Stroke::new(stroke_medium(), color_alpha(fib_color, 140)));
                    painter.text(egui::pos2(xr + 4.0, y), egui::Align2::LEFT_CENTER,
                        &format!("{:.1}% {:.2}", lv * 100.0, lp), mono_2xs(), color_alpha(fib_color, 200));
                }
            }
            for &lv in extensions.iter() {
                let lp = p0 + range * lv;
                let y = py(lp);
                if y.is_finite() && y.abs() < 50000.0 {
                    // Extensions: dashed, dimmer
                    let dash_len = 4.0; let gap_len = 4.0;
                    let a = egui::pos2(xl, y); let b_pt = egui::pos2(xr, y);
                    let dir = b_pt - a; let len = dir.length();
                    if len > 0.0 {
                        let norm = dir / len; let mut dd = 0.0;
                        while dd < len {
                            let s = a + norm * dd;
                            let e = a + norm * (dd + dash_len).min(len);
                            painter.line_segment([s, e], egui::Stroke::new(stroke_thin(), color_alpha(fib_color, 80)));
                            dd += dash_len + gap_len;
                        }
                    }
                    painter.text(egui::pos2(xr + 4.0, y), egui::Align2::LEFT_CENTER,
                        &format!("{:.1}% {:.2}", lv * 100.0, lp), mono_2xs(), color_alpha(fib_color, 130));
                }
            }
            // Shaded golden zone (38.2%-61.8%)
            let y382 = py(p0 + range * 0.382);
            let y618 = py(p0 + range * 0.618);
            if y382.is_finite() && y618.is_finite() {
                painter.rect_filled(egui::Rect::from_min_max(
                    egui::pos2(xl, y382.min(y618)), egui::pos2(xr, y382.max(y618))),
                    0.0, egui::Color32::from_rgba_unmultiplied(255, 193, 37, 12));
            }
        }
        // Gold crosshair
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), fib_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), fib_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "channel" || chart.draw_tool == "fibchannel" {
        // Channel / Fib-channel preview
        let is_fib = chart.draw_tool == "fibchannel";
        let chan_color = if is_fib { egui::Color32::from_rgb(196, 163, 90) } else { egui::Color32::from_rgb(130, 220, 180) };
        if let Some((b0, p0)) = chart.pending_pt {
            if let Some((b1, p1)) = chart.pending_pt2 {
                let cursor_price = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
                let base_mid_price = (p0 + p1) / 2.0;
                let offset_price = cursor_price - base_mid_price;
                let sx0 = bx(b0); let sx1 = bx(b1);
                let sy0 = py(p0); let sy1 = py(p1);
                let oy0 = py(p0 + offset_price); let oy1 = py(p1 + offset_price);
                // Fill
                let pts = vec![egui::pos2(sx0, sy0), egui::pos2(sx1, sy1), egui::pos2(sx1, oy1), egui::pos2(sx0, oy0)];
                painter.add(egui::Shape::convex_polygon(pts, color_alpha(chan_color, 15), egui::Stroke::NONE));
                // Base + parallel
                painter.line_segment([egui::pos2(sx0, sy0), egui::pos2(sx1, sy1)], egui::Stroke::new(stroke_bold(), color_alpha(chan_color, 200)));
                painter.line_segment([egui::pos2(sx0, oy0), egui::pos2(sx1, oy1)], egui::Stroke::new(stroke_bold(), color_alpha(chan_color, 200)));
                // Midline
                let my0 = (sy0 + oy0) / 2.0; let my1 = (sy1 + oy1) / 2.0;
                painter.line_segment([egui::pos2(sx0, my0), egui::pos2(sx1, my1)], egui::Stroke::new(stroke_medium(), color_alpha(chan_color, 80)));
                // Fib internal lines preview
                if is_fib {
                    for &ratio in &[0.236_f32, 0.382, 0.618, 0.786] {
                        let fy0 = sy0 + (oy0 - sy0) * ratio;
                        let fy1 = sy1 + (oy1 - sy1) * ratio;
                        painter.line_segment([egui::pos2(sx0, fy0), egui::pos2(sx1, fy1)],
                            egui::Stroke::new(stroke_thin(), color_alpha(chan_color, 60)));
                    }
                }
            } else {
                let start = egui::pos2(bx(b0), py(p0));
                painter.line_segment([start, pos], egui::Stroke::new(stroke_bold(), color_alpha(chan_color, 180)));
                painter.circle_filled(start, 3.0, chan_color);
                painter.circle_filled(pos, 3.0, chan_color);
            }
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), chan_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), chan_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "barmarker" {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    } else if chart.draw_tool == "pitchfork" {
        let fork_color = egui::Color32::from_rgb(126, 207, 207);
        let n_pts = chart.pending_pts.len();
        if n_pts == 0 {
            // First click hint
        } else if n_pts == 1 {
            let p0 = egui::pos2(bx(chart.pending_pts[0].0), py(chart.pending_pts[0].1));
            painter.line_segment([p0, pos], egui::Stroke::new(stroke_bold(), color_alpha(fork_color, 180)));
            painter.circle_filled(p0, 3.0, fork_color);
        } else if n_pts == 2 {
            let p0 = egui::pos2(bx(chart.pending_pts[0].0), py(chart.pending_pts[0].1));
            let p1 = egui::pos2(bx(chart.pending_pts[1].0), py(chart.pending_pts[1].1));
            painter.line_segment([p0, pos], egui::Stroke::new(stroke_bold(), color_alpha(fork_color, 180)));
            painter.circle_filled(p0, 3.0, fork_color);
            painter.circle_filled(p1, 3.0, fork_color);
            painter.line_segment([p1, pos], egui::Stroke::new(stroke_medium(), color_alpha(fork_color, 100)));
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), fork_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), fork_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "gannfan" {
        let fan_color = egui::Color32::from_rgb(232, 201, 107);
        if let Some((b0, p0)) = chart.pending_pt {
            let origin = egui::pos2(bx(b0), py(p0));
            let chart_right = rect.left() + cw;
            let ref_dx = pos.x - origin.x; let ref_dy = pos.y - origin.y;
            if ref_dx.abs() > 1.0 {
                let fans: &[(f32, u8)] = &[(8.0,50),(4.0,60),(3.0,70),(2.0,90),(1.0,200),(0.5,90),(1.0/3.0,70),(0.25,60),(0.125,50)];
                for &(ratio, alpha) in fans {
                    let slope = ref_dy / ref_dx * ratio;
                    let end = egui::pos2(chart_right, origin.y + slope * (chart_right - origin.x));
                    painter.line_segment([origin, clamp_pt(end)], egui::Stroke::new(if (ratio-1.0).abs()<0.01 {stroke_bold()} else {stroke_medium()}, color_alpha(fan_color, alpha)));
                }
            }
            painter.circle_filled(origin, 3.0, fan_color);
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), fan_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), fan_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "regression" {
        let reg_color = egui::Color32::from_rgb(180, 128, 232);
        if let Some((b0, _)) = chart.pending_pt {
            let x0 = bx(b0); let x1 = pos.x;
            painter.line_segment([egui::pos2(x0, rect.top()+pt), egui::pos2(x0, rect.top()+pt+ch)],
                egui::Stroke::new(stroke_std(), color_alpha(reg_color, 120)));
            painter.line_segment([egui::pos2(x1, rect.top()+pt), egui::pos2(x1, rect.top()+pt+ch)],
                egui::Stroke::new(stroke_std(), color_alpha(reg_color, 120)));
            painter.rect_filled(egui::Rect::from_x_y_ranges(x0.min(x1)..=x0.max(x1), (rect.top()+pt)..=(rect.top()+pt+ch)),
                0.0, egui::Color32::from_rgba_unmultiplied(180, 128, 232, 15));
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), reg_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), reg_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "xabcd" {
        let xabcd_color = egui::Color32::from_rgb(255, 159, 67);
        let labels = ["X","A","B","C","D"];
        for i in 0..chart.pending_pts.len().saturating_sub(1) {
            let pa = egui::pos2(bx(chart.pending_pts[i].0), py(chart.pending_pts[i].1));
            let pb = egui::pos2(bx(chart.pending_pts[i+1].0), py(chart.pending_pts[i+1].1));
            painter.line_segment([pa, pb], egui::Stroke::new(stroke_bold(), color_alpha(xabcd_color, 200)));
        }
        for (i, &(b, p)) in chart.pending_pts.iter().enumerate() {
            let pt = egui::pos2(bx(b), py(p));
            painter.circle_filled(pt, 4.0, xabcd_color);
            painter.text(pt + egui::vec2(0.0, -10.0), egui::Align2::CENTER_CENTER,
                labels.get(i).copied().unwrap_or("?"), mono_xs(), xabcd_color);
        }
        if let Some(last) = chart.pending_pts.last() {
            let lp = egui::pos2(bx(last.0), py(last.1));
            painter.line_segment([lp, pos], egui::Stroke::new(stroke_bold(), color_alpha(xabcd_color, 160)));
            let next_label = labels.get(chart.pending_pts.len()).copied().unwrap_or("?");
            painter.text(pos + egui::vec2(0.0, -10.0), egui::Align2::CENTER_CENTER,
                next_label, mono_xs(), xabcd_color);
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), xabcd_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), xabcd_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "vline" {
        let vl_color = egui::Color32::from_rgb(100, 160, 255);
        let x = pos.x;
        painter.line_segment([egui::pos2(x, rect.top()+pt), egui::pos2(x, rect.top()+pt+ch)],
            egui::Stroke::new(stroke_std(), color_alpha(vl_color, 160)));
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), vl_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), vl_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "ray" {
        let ray_color = egui::Color32::from_rgb(100, 200, 255);
        if let Some((b0, p0)) = chart.pending_pt {
            let start = egui::pos2(bx(b0), py(p0));
            let chart_right = rect.left() + cw;
            painter.line_segment([start, pos], egui::Stroke::new(stroke_bold(), color_alpha(ray_color, 200)));
            // Extended preview
            let dx = pos.x - start.x;
            if dx.abs() > 0.001 {
                let slope = (pos.y - start.y) / dx;
                let ext_y = pos.y + slope * (chart_right - pos.x);
                painter.line_segment([pos, clamp_pt(egui::pos2(chart_right, ext_y))], egui::Stroke::new(stroke_std(), color_alpha(ray_color, 120)));
            }
            painter.circle_filled(start, 3.0, ray_color);
            painter.circle_filled(pos, 3.0, ray_color);
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), ray_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), ray_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "fibext" {
        let fibext_color = egui::Color32::from_rgb(255, 215, 0);
        let n_pts = chart.pending_pts.len();
        let labels = ["A","B","C"];
        for i in 0..n_pts.saturating_sub(1) {
            let pa = egui::pos2(bx(chart.pending_pts[i].0), py(chart.pending_pts[i].1));
            let pb = egui::pos2(bx(chart.pending_pts[i+1].0), py(chart.pending_pts[i+1].1));
            painter.line_segment([pa, pb], egui::Stroke::new(stroke_bold(), color_alpha(fibext_color, 180)));
        }
        for (i, &(b, p)) in chart.pending_pts.iter().enumerate() {
            let pt = egui::pos2(bx(b), py(p));
            painter.circle_filled(pt, 4.0, fibext_color);
            painter.text(pt + egui::vec2(0.0, -10.0), egui::Align2::CENTER_CENTER, labels.get(i).copied().unwrap_or("?"), mono_xs(), fibext_color);
        }
        if n_pts == 2 {
            // Show extension levels preview
            let (b0, p0) = chart.pending_pts[0]; let (_, p1) = chart.pending_pts[1];
            let ab_range = p1 - p0;
            let cursor_price = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
            let dir: f32 = if ab_range >= 0.0 { 1.0 } else { -1.0 };
            let chart_right = rect.left() + cw;
            let cx = bx(chart.pending_pts[1].0.max(pos.x as f32));
            for &(ratio, label) in &[(0.0_f32,"0%"),(0.618,"61.8%"),(1.0,"100%"),(1.618,"161.8%"),(2.618,"261.8%")] {
                let lp = cursor_price + dir * ratio * ab_range.abs();
                let y = py(lp);
                if y.is_finite() && y.abs() < 50000.0 {
                    painter.line_segment([egui::pos2(pos.x, y), egui::pos2(chart_right, y)], egui::Stroke::new(stroke_medium(), color_alpha(fibext_color, 140)));
                    painter.text(egui::pos2(chart_right + 2.0, y), egui::Align2::LEFT_CENTER, &format!("{} {:.2}", label, lp), mono_3xs(), color_alpha(fibext_color, 180));
                }
            }
            let _ = (b0, cx);
        }
        if let Some(last) = chart.pending_pts.last() {
            let lp = egui::pos2(bx(last.0), py(last.1));
            painter.line_segment([lp, pos], egui::Stroke::new(stroke_bold(), color_alpha(fibext_color, 140)));
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), fibext_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), fibext_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "fibtimezone" {
        let ftz_color = egui::Color32::from_rgb(255, 193, 37);
        let anchor_bar = (pos.x - rect.left() + off - bs*0.5) / bs + vs;
        let fib_nums: &[u32] = &[1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];
        let chart_right = rect.left() + cw;
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for (idx, &fib) in fib_nums.iter().enumerate() {
            if seen.contains(&fib) { continue; } seen.insert(fib);
            let x = bx(anchor_bar + fib as f32);
            if x < rect.left() || x > chart_right { continue; }
            let alpha = (180_u8).saturating_sub((idx as u8) * 16);
            painter.line_segment([egui::pos2(x, rect.top()+pt), egui::pos2(x, rect.top()+pt+ch)],
                egui::Stroke::new(stroke_medium(), color_alpha(ftz_color, alpha)));
        }
        painter.line_segment([pos, egui::pos2(pos.x, rect.top()+pt+ch)], egui::Stroke::new(stroke_bold(), color_alpha(ftz_color, 200)));
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), ftz_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), ftz_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "fibarc" {
        let farc_color = egui::Color32::from_rgb(255, 193, 37);
        if let Some((b0, p0)) = chart.pending_pt {
            let center = egui::pos2(bx(b0), py(p0));
            let dist = center.distance(pos);
            for &ratio in &[0.236_f32, 0.382, 0.5, 0.618, 0.786, 1.0] {
                let r = dist * ratio;
                let alpha = if ratio >= 0.618 { 180u8 } else { 100 };
                let n_seg = 30;
                let mut arc_pts: Vec<egui::Pos2> = Vec::with_capacity(n_seg + 1);
                for k in 0..=n_seg {
                    let angle = std::f32::consts::PI * (k as f32 / n_seg as f32) + std::f32::consts::FRAC_PI_2;
                    arc_pts.push(clamp_pt(egui::pos2(pos.x + r * angle.cos(), pos.y + r * angle.sin())));
                }
                if arc_pts.len() > 1 { painter.add(egui::Shape::line(arc_pts, egui::Stroke::new(stroke_medium(), color_alpha(farc_color, alpha)))); }
            }
            painter.circle_filled(center, 3.0, farc_color);
            painter.line_segment([center, pos], egui::Stroke::new(stroke_medium(), color_alpha(farc_color, 100)));
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), farc_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), farc_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "gannbox" {
        let gb_color = egui::Color32::from_rgb(232, 201, 107);
        if let Some((_b0, _p0)) = chart.pending_pt {
            let start = egui::pos2(bx(_b0), py(_p0));
            let xl = start.x.min(pos.x); let xr = start.x.max(pos.x);
            let yt = start.y.min(pos.y); let yb = start.y.max(pos.y);
            painter.rect_stroke(egui::Rect::from_min_max(egui::pos2(xl,yt), egui::pos2(xr,yb)),
                0.0, egui::Stroke::new(stroke_bold(), color_alpha(gb_color, 200)), egui::StrokeKind::Outside);
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(xl,yt), egui::pos2(xr,yb)),
                0.0, color_alpha(gb_color, 12));
            // Diagonals
            painter.line_segment([egui::pos2(xl,yt), egui::pos2(xr,yb)], egui::Stroke::new(stroke_medium(), color_alpha(gb_color, 120)));
            painter.line_segment([egui::pos2(xl,yb), egui::pos2(xr,yt)], egui::Stroke::new(stroke_medium(), color_alpha(gb_color, 120)));
            painter.circle_filled(start, 3.0, gb_color);
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), gb_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), gb_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "elliott_impulse" || chart.draw_tool == "elliott_corrective"
           || chart.draw_tool == "elliott_wxy" || chart.draw_tool == "elliott_wxyxz"
           || chart.draw_tool == "elliott_sub_impulse" || chart.draw_tool == "elliott_sub_corrective" {
        let wave_color = egui::Color32::from_rgb(78, 205, 196);
        let impulse_labels = ["1","2","3","4","5"];
        let corrective_labels = ["A","B","C"];
        let wxy_labels = ["W","X","Y"];
        let wxyxz_labels = ["W","X","Y","X","Z"];
        let sub_impulse_labels = ["i","ii","iii","iv","v"];
        let sub_corrective_labels = ["a","b","c"];
        let labels: &[&str] = match chart.draw_tool.as_str() {
            "elliott_impulse" => &impulse_labels,
            "elliott_wxy" => &wxy_labels,
            "elliott_wxyxz" => &wxyxz_labels,
            "elliott_sub_impulse" => &sub_impulse_labels,
            "elliott_sub_corrective" => &sub_corrective_labels,
            _ => &corrective_labels,
        };
        for i in 0..chart.pending_pts.len().saturating_sub(1) {
            let pa = egui::pos2(bx(chart.pending_pts[i].0), py(chart.pending_pts[i].1));
            let pb = egui::pos2(bx(chart.pending_pts[i+1].0), py(chart.pending_pts[i+1].1));
            painter.line_segment([pa, pb], egui::Stroke::new(stroke_bold(), color_alpha(wave_color, 200)));
        }
        for (i, &(b, p)) in chart.pending_pts.iter().enumerate() {
            let pt = egui::pos2(bx(b), py(p));
            painter.circle_filled(pt, 7.0, color_alpha(wave_color, 80));
            painter.circle_stroke(pt, 7.0, egui::Stroke::new(stroke_std(), wave_color));
            // contrast_fg picks BLACK on the light teal/wave circle fill, WHITE on dark.
            painter.text(pt, egui::Align2::CENTER_CENTER, labels.get(i).copied().unwrap_or("?"), mono_3xs(), crate::ui_kit::style::contrast_fg(wave_color));
        }
        if let Some(last) = chart.pending_pts.last() {
            let lp = egui::pos2(bx(last.0), py(last.1));
            painter.line_segment([lp, pos], egui::Stroke::new(stroke_bold(), color_alpha(wave_color, 160)));
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), wave_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), wave_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "avwap" {
        let av_color = COLOR_PURPLE;
        painter.line_segment([egui::pos2(pos.x, rect.top()+pt), egui::pos2(pos.x, rect.top()+pt+ch)],
            egui::Stroke::new(stroke_std(), color_alpha(av_color, 120)));
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), av_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), av_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "pricerange" {
        let pr_color = egui::Color32::from_rgb(116, 185, 255);
        if let Some((_b0, p0)) = chart.pending_pt {
            let y0 = py(p0);
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(chart.pending_pt.map(|_| pos.x).unwrap_or(pos.x), y0.min(pos.y)),
                                         egui::pos2(pos.x.max(chart.pending_pt.map(|_| pos.x).unwrap_or(pos.x)), y0.max(pos.y))),
                0.0, egui::Color32::from_rgba_unmultiplied(116, 185, 255, 20));
            painter.line_segment([egui::pos2(rect.left(), y0), egui::pos2(rect.left()+cw, y0)],
                egui::Stroke::new(stroke_std(), color_alpha(pr_color, 160)));
            painter.line_segment([egui::pos2(rect.left(), pos.y), egui::pos2(rect.left()+cw, pos.y)],
                egui::Stroke::new(stroke_std(), color_alpha(pr_color, 160)));
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), pr_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), pr_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    } else if chart.draw_tool == "riskreward" {
        let rr_color = COLOR_PROFIT_GREEN;
        let n_pts = chart.pending_pts.len();
        if n_pts == 0 {
            // waiting for entry click
        } else if n_pts == 1 {
            // entry placed, waiting for stop
            let entry_y = py(chart.pending_pts[0].1);
            let chart_right = rect.left() + cw;
            let ex = bx(chart.pending_pts[0].0);
            painter.line_segment([egui::pos2(ex, entry_y), egui::pos2(chart_right, entry_y)],
                egui::Stroke::new(stroke_bold(), color_alpha(rr_color, 200)));
            painter.line_segment([egui::pos2(ex, pos.y), egui::pos2(chart_right, pos.y)],
                egui::Stroke::new(stroke_std(), color_alpha(COLOR_LOSS_RED, 160)));
            let risk = (py(chart.pending_pts[0].1) - pos.y).abs();
            let stop_side = pos.y.min(entry_y);
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(ex, stop_side), egui::pos2(chart_right, stop_side + risk)),
                0.0, color_alpha(t.bear, 20));
        } else if n_pts == 2 {
            // entry + stop placed, waiting for target
            let chart_right = rect.left() + cw;
            let ex = bx(chart.pending_pts[0].0);
            let entry_y = py(chart.pending_pts[0].1);
            let stop_y  = py(chart.pending_pts[1].1);
            let risk_h  = (entry_y - stop_y).abs();
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(ex, entry_y.min(stop_y)), egui::pos2(chart_right, entry_y.max(stop_y))),
                0.0, color_alpha(t.bear, 25));
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(ex, entry_y.min(pos.y)), egui::pos2(chart_right, entry_y.max(pos.y))),
                0.0, color_alpha(t.bull, 25));
            painter.line_segment([egui::pos2(ex, entry_y), egui::pos2(chart_right, entry_y)], egui::Stroke::new(stroke_bold(), color_alpha(rr_color, 200)));
            painter.line_segment([egui::pos2(ex, stop_y), egui::pos2(chart_right, stop_y)], egui::Stroke::new(stroke_std(), color_alpha(COLOR_LOSS_RED, 160)));
            painter.line_segment([egui::pos2(ex, pos.y), egui::pos2(chart_right, pos.y)], egui::Stroke::new(stroke_std(), color_alpha(rr_color, 160)));
            if risk_h > 0.0 {
                let reward_h = (entry_y - pos.y).abs();
                let rr = reward_h / risk_h;
                painter.text(egui::pos2(chart_right - 4.0, entry_y.min(pos.y) + reward_h * 0.5),
                    egui::Align2::RIGHT_CENTER, &format!("{:.2}:1", rr), mono_xs(), rr_color);
            }
        }
        let ch_len = 8.0;
        painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(stroke_std(), rr_color));
        painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(stroke_std(), rr_color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    }
}
