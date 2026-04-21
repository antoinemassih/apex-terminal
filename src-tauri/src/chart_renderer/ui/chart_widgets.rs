//! Chart Widgets — floating info cards rendered on the chart canvas.
//! Premium infographic-style gauges and data visualizations.
//!
//! Display modes:
//!   Card    — glass card with shadow, border, title bar (full chrome)
//!   HUD     — transparent, just the data painted on the chart, click-through
//!   Minimal — no background, faint label for grab handle
//!
//! Docking:
//!   Float   — free-floating, positioned by x/y fractions
//!   Top     — auto-laid out in a horizontal strip at the top of the chart
//!   Bottom  — auto-laid out in a horizontal strip at the bottom

use egui::{self, Color32, Stroke};
use super::style::*;
use super::super::gpu::*;
use crate::chart_renderer::{ChartWidgetKind, WidgetDisplayMode, WidgetDock};
use std::f32::consts::PI;

// ─── Docking tuning ──────────────────────────────────────────────────────────
const SNAP_ZONE: f32 = 28.0;   // pixels from edge to trigger snap
const YANK_THRESHOLD: f32 = 45.0; // vertical drag needed to undock
const STRIP_PAD: f32 = 8.0;    // padding inside dock strip
const ANIM_SPEED: f32 = 0.18;  // lerp factor per frame (0=frozen, 1=instant)

/// Smooth lerp helper — moves `current` toward `target` by factor `speed`.
fn smooth(current: f32, target: f32, speed: f32) -> f32 {
    current + (target - current) * speed.clamp(0.01, 1.0)
}

/// Render all visible widgets for a chart pane.
pub(crate) fn draw_widgets(
    ui: &mut egui::Ui,
    chart: &mut Chart,
    rect: egui::Rect,
    t: &Theme,
) {
    // ── Fade during draw mode (non-interactive but still visible) ──
    let draw_faded = !chart.draw_tool.is_empty();

    let mut painter = ui.painter_at(rect);
    if draw_faded {
        painter.set_opacity(0.18);
    }
    // Cache widget data — only recompute when bar count changes
    let bar_count = chart.bars.len();
    let cache_valid = chart.widget_cache_bar_count == bar_count && bar_count > 0;
    let wd = if cache_valid {
        // Reuse cached data, just update live fields
        let mut cached = chart.widget_cache.take().unwrap_or_else(|| WidgetData::from_chart(chart));
        // Update live-changing fields
        if let Some((_, positions, _)) = crate::chart_renderer::trading::read_account_data() {
            if !positions.is_empty() {
                cached.day_pnl = positions.iter().map(|p| p.unrealized_pnl as f32).sum();
            }
        }
        cached
    } else {
        chart.widget_cache_bar_count = bar_count;
        WidgetData::from_chart(chart)
    };

    // ══════════════════════════════════════════════════════════════════════════
    // Pass 1 — Compute target positions and animate
    // ══════════════════════════════════════════════════════════════════════════

    // We need mutable access to chart.chart_widgets for animation updates,
    // so we collect target rects first, then update anim state.
    let n = chart.chart_widgets.len();
    let mut targets: Vec<(f32, f32)> = Vec::with_capacity(n); // target screen (x, y)

    for wi in 0..n {
        let w = &chart.chart_widgets[wi];
        if !w.visible { targets.push((0.0, 0.0)); continue; }

        let card_h = if w.collapsed { 26.0 } else { w.h };

        let (tx, ty) = match w.dock {
            WidgetDock::Top => {
                let dx = w.dock_x.clamp(rect.left() + STRIP_PAD, rect.right() - w.w - STRIP_PAD);
                (dx, rect.top() + STRIP_PAD)
            }
            WidgetDock::Bottom => {
                let dx = w.dock_x.clamp(rect.left() + STRIP_PAD, rect.right() - w.w - STRIP_PAD);
                (dx, rect.bottom() - card_h - STRIP_PAD)
            }
            WidgetDock::Float => {
                (rect.left() + w.x * rect.width(), rect.top() + w.y * rect.height())
            }
        };
        targets.push((tx, ty));
    }

    // Update animation state
    let mut any_animating = false;
    for wi in 0..n {
        let w = &mut chart.chart_widgets[wi];
        if !w.visible { continue; }
        let (tx, ty) = targets[wi];

        if !w.anim_init {
            // First frame: snap directly to target (no animation from 0,0)
            w.anim_x = tx;
            w.anim_y = ty;
            w.anim_init = true;
        } else {
            w.anim_x = smooth(w.anim_x, tx, ANIM_SPEED);
            w.anim_y = smooth(w.anim_y, ty, ANIM_SPEED);
            // Keep animating if not settled
            if (w.anim_x - tx).abs() > 0.5 || (w.anim_y - ty).abs() > 0.5 {
                any_animating = true;
            }
        }
    }
    if any_animating { ui.ctx().request_repaint(); }

    // ══════════════════════════════════════════════════════════════════════════
    // Draw dock strip backgrounds
    // ══════════════════════════════════════════════════════════════════════════

    let has_top = chart.chart_widgets.iter().any(|w| w.visible && w.dock == WidgetDock::Top);
    let has_bottom = chart.chart_widgets.iter().any(|w| w.visible && w.dock == WidgetDock::Bottom);

    if has_top {
        let max_h = chart.chart_widgets.iter()
            .filter(|w| w.visible && w.dock == WidgetDock::Top)
            .map(|w| if w.collapsed { 26.0 } else { w.h })
            .fold(0.0f32, f32::max);
        let strip = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), max_h + STRIP_PAD * 2.0));
        painter.rect_filled(strip, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 18));
        painter.line_segment(
            [egui::pos2(strip.left(), strip.bottom()), egui::pos2(strip.right(), strip.bottom())],
            Stroke::new(STROKE_HAIR, color_alpha(t.toolbar_border, ALPHA_MUTED)));
    }
    if has_bottom {
        let max_h = chart.chart_widgets.iter()
            .filter(|w| w.visible && w.dock == WidgetDock::Bottom)
            .map(|w| if w.collapsed { 26.0 } else { w.h })
            .fold(0.0f32, f32::max);
        let strip = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - max_h - STRIP_PAD * 2.0), rect.max);
        painter.rect_filled(strip, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 18));
        painter.line_segment(
            [egui::pos2(strip.left(), strip.top()), egui::pos2(strip.right(), strip.top())],
            Stroke::new(STROKE_HAIR, color_alpha(t.toolbar_border, ALPHA_MUTED)));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Pass 2 — Render + interact
    // ══════════════════════════════════════════════════════════════════════════

    let mut mode_toggle: Option<usize> = None;
    let mut collapse_toggle: Option<usize> = None;
    let mut popup_open: Option<usize> = None;
    let mut resize_delta: Option<(usize, egui::Vec2)> = None;
    // Deferred context menu actions (to avoid borrow conflicts)
    enum CtxAction { Lock(usize), Delete(usize), ResetSize(usize), DockTop(usize), DockBottom(usize), Undock(usize) }
    let mut ctx_action: Option<CtxAction> = None;
    let mut widget_btn_action: Option<WidgetBtnAction> = None;
    let hover_pos = ui.ctx().pointer_hover_pos();

    for wi in 0..n {
        // Copy fields we need to avoid holding a borrow across mutations
        let w = &chart.chart_widgets[wi];
        if !w.visible { continue; }

        let card_w = w.w;
        let card_h = if w.collapsed { 26.0 } else { w.h };
        let card_rect = egui::Rect::from_min_size(
            egui::pos2(w.anim_x, w.anim_y), egui::vec2(card_w, card_h));
        if !rect.intersects(card_rect) { continue; }

        let kind = w.kind;
        let mode = w.display;
        let title_h = 24.0;
        // Button hit rects for click routing (set during Card rendering, checked after resp)
        let mut card_ctx_rect: Option<egui::Rect> = None;
        let mut card_toggle_rect: Option<egui::Rect> = None;

        // Mode icon
        let mode_icon = if mode == WidgetDisplayMode::Card { "\u{25FC}" } else { "\u{25CB}" };
        let card_hovered = !draw_faded && ui.rect_contains_pointer(card_rect);

        // Card mode: solid background
        if mode == WidgetDisplayMode::Card {
            painter.rect_filled(card_rect.translate(egui::vec2(0.0, 2.0)).expand(1.0),
                RADIUS_LG + 1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 20));
            let bg = Color32::from_rgba_unmultiplied(
                t.toolbar_bg.r().saturating_add(4), t.toolbar_bg.g().saturating_add(4),
                t.toolbar_bg.b().saturating_add(6), 230);
            painter.rect_filled(card_rect, RADIUS_LG, bg);
            painter.rect_stroke(card_rect, RADIUS_LG,
                Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_LINE)),
                egui::StrokeKind::Outside);
        }

        // Widget body
        let hdr_h: f32 = if card_hovered && !w.collapsed { 26.0 } else { 0.0 };
        if !w.collapsed {
            let body = egui::Rect::from_min_max(
                egui::pos2(card_rect.left(), card_rect.top() + hdr_h),
                card_rect.max);
            let mut btns = Vec::new();
            draw_widget_body(&painter, body, kind, &wd, t, hover_pos, &mut btns);
            if ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                if let Some(pos) = hover_pos {
                    for (btn_rect, action) in &btns {
                        if btn_rect.contains(pos) { widget_btn_action = Some(*action); }
                    }
                }
            }
            if let Some(pos) = hover_pos {
                for (btn_rect, _) in &btns {
                    if btn_rect.contains(pos) { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                }
            }
            if mode == WidgetDisplayMode::Card {
                if let Some(delta) = resize_handle(ui, &painter, card_rect, wi, t) {
                    resize_delta = Some((wi, delta));
                }
            }
        } else {
            draw_mini_badge(&painter, card_rect, kind, &wd, t);
        }

        // ── Header bar — shown ABOVE body on hover, pushes body down ──
        if card_hovered && !w.collapsed {
            let hdr = egui::Rect::from_min_size(card_rect.min, egui::vec2(card_w, hdr_h));
            // Header background
            painter.rect_filled(hdr,
                egui::CornerRadius { nw: RADIUS_LG as u8, ne: RADIUS_LG as u8, sw: 0, se: 0 },
                Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 230));
            painter.line_segment(
                [egui::pos2(hdr.left() + 4.0, hdr.bottom()), egui::pos2(hdr.right() - 4.0, hdr.bottom())],
                Stroke::new(0.5, color_alpha(t.toolbar_border, ALPHA_MUTED)));
            // Label
            painter.text(egui::pos2(hdr.left() + 8.0, hdr.center().y),
                egui::Align2::LEFT_CENTER, kind.icon(), egui::FontId::proportional(FONT_MD), t.accent);
            painter.text(egui::pos2(hdr.left() + 24.0, hdr.center().y),
                egui::Align2::LEFT_CENTER, kind.label(), egui::FontId::monospace(FONT_XS), t.text);
            if w.locked {
                painter.text(egui::pos2(hdr.left() + 24.0 + kind.label().len() as f32 * 7.0 + 6.0, hdr.center().y),
                    egui::Align2::LEFT_CENTER, "\u{1F512}", egui::FontId::proportional(8.0), t.dim.gamma_multiply(0.5));
            }

            // ── Buttons — large, visible, with hover backgrounds ──
            let btn_w = 32.0;
            let btn_h = 22.0;
            let ptr = ui.ctx().pointer_hover_pos();

            // Context menu ⋯
            let ctx_rect = egui::Rect::from_center_size(
                egui::pos2(hdr.right() - btn_w - 20.0, hdr.center().y), egui::vec2(btn_w, btn_h));
            let ctx_hov = ptr.map(|p| ctx_rect.contains(p)).unwrap_or(false);
            painter.rect_filled(ctx_rect, 5.0,
                if ctx_hov { color_alpha(t.accent, 50) } else { color_alpha(t.toolbar_border, 25) });
            painter.rect_stroke(ctx_rect, 5.0,
                Stroke::new(0.5, if ctx_hov { t.accent } else { color_alpha(t.toolbar_border, ALPHA_MUTED) }),
                egui::StrokeKind::Outside);
            painter.text(ctx_rect.center(), egui::Align2::CENTER_CENTER,
                "\u{22EF}", egui::FontId::proportional(16.0),
                if ctx_hov { t.accent } else { t.dim });
            if ctx_hov { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            card_ctx_rect = Some(ctx_rect);

            // Mode toggle
            let tog_rect = egui::Rect::from_center_size(
                egui::pos2(hdr.right() - 18.0, hdr.center().y), egui::vec2(btn_w, btn_h));
            let tog_hov = ptr.map(|p| tog_rect.contains(p)).unwrap_or(false);
            painter.rect_filled(tog_rect, 5.0,
                if tog_hov { color_alpha(t.accent, 50) } else { color_alpha(t.toolbar_border, 25) });
            painter.rect_stroke(tog_rect, 5.0,
                Stroke::new(0.5, if tog_hov { t.accent } else { color_alpha(t.toolbar_border, ALPHA_MUTED) }),
                egui::StrokeKind::Outside);
            painter.text(tog_rect.center(), egui::Align2::CENTER_CENTER,
                mode_icon, egui::FontId::proportional(14.0),
                if tog_hov { t.accent } else { t.dim });
            if tog_hov { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            card_toggle_rect = Some(tog_rect);
        }

        // ══════════════════════════════════════════════════════════════════
        // Interaction
        // ══════════════════════════════════════════════════════════════════

        if !draw_faded {
        let sense = egui::Sense::click_and_drag();

        // Interact area = top strip for drag (always available)
        let interact_rect = egui::Rect::from_min_size(card_rect.min, egui::vec2(card_w, 16.0));
        let resp = ui.interact(interact_rect, egui::Id::new(("widget_drag", wi)), sense);

        if resp.dragged_by(egui::PointerButton::Primary) && !chart.chart_widgets[wi].locked {
            let d = resp.drag_delta();
            let wid = &mut chart.chart_widgets[wi];
            let pointer = ui.ctx().pointer_interact_pos().unwrap_or(card_rect.center());
            match wid.dock {
                WidgetDock::Float => {
                    wid.x += d.x / rect.width();
                    wid.y += d.y / rect.height();
                    wid.x = wid.x.clamp(0.0, 0.95);
                    wid.y = wid.y.clamp(0.0, 0.95);
                    if pointer.y - rect.top() < SNAP_ZONE {
                        wid.dock = WidgetDock::Top;
                        wid.dock_x = (pointer.x - card_w * 0.5).clamp(rect.left() + STRIP_PAD, rect.right() - card_w - STRIP_PAD);
                    } else if rect.bottom() - pointer.y < SNAP_ZONE {
                        wid.dock = WidgetDock::Bottom;
                        wid.dock_x = (pointer.x - card_w * 0.5).clamp(rect.left() + STRIP_PAD, rect.right() - card_w - STRIP_PAD);
                    }
                }
                WidgetDock::Top | WidgetDock::Bottom => {
                    wid.dock_x += d.x;
                    wid.dock_x = wid.dock_x.clamp(rect.left() + STRIP_PAD, rect.right() - card_w - STRIP_PAD);
                    let strip_center_y = match wid.dock {
                        WidgetDock::Top => rect.top() + STRIP_PAD + card_h * 0.5,
                        WidgetDock::Bottom => rect.bottom() - STRIP_PAD - card_h * 0.5,
                        _ => 0.0,
                    };
                    if (pointer.y - strip_center_y).abs() > YANK_THRESHOLD {
                        wid.dock = WidgetDock::Float;
                        wid.x = ((wid.anim_x - rect.left()) / rect.width()).clamp(0.0, 0.95);
                        wid.y = ((pointer.y - card_h * 0.5 - rect.top()) / rect.height()).clamp(0.0, 0.95);
                    }
                }
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if resp.hovered() && !card_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        // Click routing — check pointer position against button rects
        if resp.clicked() {
            if let Some(click_pos) = resp.interact_pointer_pos() {
                if card_ctx_rect.map(|r| r.contains(click_pos)).unwrap_or(false) {
                    popup_open = Some(wi);
                } else if card_toggle_rect.map(|r| r.contains(click_pos)).unwrap_or(false) {
                    mode_toggle = Some(wi);
                } else if !card_hovered {
                    // Only collapse when clicking outside the header
                    collapse_toggle = Some(wi);
                }
            }
        }
        // Also check raw pointer click on buttons (for when resp doesn't capture it)
        if ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
            if let Some(pos) = hover_pos {
                if card_ctx_rect.map(|r| r.contains(pos)).unwrap_or(false) && popup_open.is_none() {
                    popup_open = Some(wi);
                } else if card_toggle_rect.map(|r| r.contains(pos)).unwrap_or(false) && mode_toggle.is_none() {
                    mode_toggle = Some(wi);
                }
            }
        }

        // ── Snap zone glow — fades in as pointer approaches edge ──
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                let dist_top = pos.y - rect.top();
                let dist_bot = rect.bottom() - pos.y;

                if dist_top < SNAP_ZONE && chart.chart_widgets[wi].dock == WidgetDock::Float {
                    let progress = 1.0 - (dist_top / SNAP_ZONE).clamp(0.0, 1.0);
                    let h = (4.0 * progress).max(1.0);
                    let a = (ALPHA_TINT as f32 * progress) as u8;
                    painter.rect_filled(
                        egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), h)),
                        0.0, color_alpha(t.accent, a));
                } else if dist_bot < SNAP_ZONE && chart.chart_widgets[wi].dock == WidgetDock::Float {
                    let progress = 1.0 - (dist_bot / SNAP_ZONE).clamp(0.0, 1.0);
                    let h = (4.0 * progress).max(1.0);
                    let a = (ALPHA_TINT as f32 * progress) as u8;
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(rect.left(), rect.bottom() - h), rect.max),
                        0.0, color_alpha(t.accent, a));
                }
            }
        }
        } // end !draw_faded
    }

    // Apply deferred actions (no borrow conflicts here — loop is done)
    // Skip all interactions when faded (draw mode active)
    if !draw_faded {
    if let Some(wi) = collapse_toggle {
        chart.chart_widgets[wi].collapsed = !chart.chart_widgets[wi].collapsed;
    }
    if let Some(wi) = mode_toggle {
        chart.chart_widgets[wi].display = chart.chart_widgets[wi].display.cycle();
    }
    if let Some(wi) = popup_open {
        ui.memory_mut(|m| m.toggle_popup(egui::Id::new(("widget_popup", wi))));
    }
    if let Some((wi, delta)) = resize_delta {
        chart.chart_widgets[wi].w = (chart.chart_widgets[wi].w + delta.x).clamp(100.0, 400.0);
        chart.chart_widgets[wi].h = (chart.chart_widgets[wi].h + delta.y).clamp(60.0, 300.0);
    }

    // Context menu popup (rendered outside the loop to avoid borrow conflicts)
    // Check if any widget has its popup open
    for wi in 0..n {
        if !chart.chart_widgets[wi].visible { continue; }
        let popup_id = egui::Id::new(("widget_popup", wi));
        if ui.memory(|m| m.is_popup_open(popup_id)) {
            let is_locked = chart.chart_widgets[wi].locked;
            let is_docked = chart.chart_widgets[wi].dock != WidgetDock::Float;
            let anchor_rect = egui::Rect::from_center_size(
                egui::pos2(chart.chart_widgets[wi].anim_x + chart.chart_widgets[wi].w - 42.0,
                           chart.chart_widgets[wi].anim_y + 12.0),
                egui::vec2(14.0, 14.0));
            let anchor_resp = ui.interact(anchor_rect,
                egui::Id::new(("widget_ctx_anchor", wi)), egui::Sense::hover());
            egui::popup_below_widget(ui, popup_id, &anchor_resp,
                egui::PopupCloseBehavior::CloseOnClickOutside, |ui: &mut egui::Ui| {
                ui.set_min_width(120.0);
                if ui.button(if is_locked { "\u{1F513} Unlock" } else { "\u{1F512} Lock" }).clicked() {
                    ctx_action = Some(CtxAction::Lock(wi));
                    ui.memory_mut(|m| m.close_popup());
                }
                if ui.button("\u{1F5D1} Delete").clicked() {
                    ctx_action = Some(CtxAction::Delete(wi));
                    ui.memory_mut(|m| m.close_popup());
                }
                if ui.button("\u{21BB} Reset Size").clicked() {
                    ctx_action = Some(CtxAction::ResetSize(wi));
                    ui.memory_mut(|m| m.close_popup());
                }
                if ui.button("\u{2B06} Dock Top").clicked() {
                    ctx_action = Some(CtxAction::DockTop(wi));
                    ui.memory_mut(|m| m.close_popup());
                }
                if ui.button("\u{2B07} Dock Bottom").clicked() {
                    ctx_action = Some(CtxAction::DockBottom(wi));
                    ui.memory_mut(|m| m.close_popup());
                }
                if is_docked {
                    if ui.button("\u{2197} Undock").clicked() {
                        ctx_action = Some(CtxAction::Undock(wi));
                        ui.memory_mut(|m| m.close_popup());
                    }
                }
            });
            break; // only one popup can be open at a time
        }
    }

    // Apply deferred context menu action
    if let Some(action) = ctx_action {
        match action {
            CtxAction::Lock(wi) => chart.chart_widgets[wi].locked = !chart.chart_widgets[wi].locked,
            CtxAction::Delete(wi) => chart.chart_widgets[wi].visible = false,
            CtxAction::ResetSize(wi) => {
                let kind = chart.chart_widgets[wi].kind;
                let fresh = crate::chart_renderer::ChartWidget::new(kind, 0.0, 0.0);
                chart.chart_widgets[wi].w = fresh.w;
                chart.chart_widgets[wi].h = fresh.h;
            }
            CtxAction::DockTop(wi) => {
                chart.chart_widgets[wi].dock = WidgetDock::Top;
                chart.chart_widgets[wi].dock_x = chart.chart_widgets[wi].anim_x;
            }
            CtxAction::DockBottom(wi) => {
                chart.chart_widgets[wi].dock = WidgetDock::Bottom;
                chart.chart_widgets[wi].dock_x = chart.chart_widgets[wi].anim_x;
            }
            CtxAction::Undock(wi) => chart.chart_widgets[wi].dock = WidgetDock::Float,
        }
    }
    // Process widget body button actions
    if let Some(action) = widget_btn_action {
        match action {
            WidgetBtnAction::CloseAllPositions => {
                // TODO: wire to actual position close via trading module
                // For now this is a visual placeholder
            }
            WidgetBtnAction::ClosePosition(_idx) => {
                // TODO: wire to close specific position
            }
        }
    }
    } // end !draw_faded

    // Store widget data cache for next frame
    chart.widget_cache = Some(wd);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Mini badge — collapsed HUD/Minimal shows one-line key value
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_mini_badge(p: &egui::Painter, rect: egui::Rect, kind: ChartWidgetKind,
                   wd: &WidgetData, t: &Theme) {
    let cy = rect.center().y;
    let lx = rect.left() + 4.0;

    // Faint pill background
    p.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 40));

    let (label, value, color) = mini_summary(kind, wd, t);
    p.text(egui::pos2(lx, cy), egui::Align2::LEFT_CENTER,
        label, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
    p.text(egui::pos2(rect.right() - 4.0, cy), egui::Align2::RIGHT_CENTER,
        &value, egui::FontId::monospace(FONT_SM), color);
}

/// Returns (label, value_string, color) for the mini badge of each widget type.
fn mini_summary(kind: ChartWidgetKind, wd: &WidgetData, t: &Theme) -> (&'static str, String, Color32) {
    match kind {
        ChartWidgetKind::TrendStrength => {
            let s = if wd.trend_score > 0.0 { wd.trend_score } else { 72.0 };
            let c = if s > 66.0 { t.bull } else if s > 33.0 { Color32::from_rgb(255, 191, 0) } else { t.bear };
            ("TRD", format!("{:.0}", s), c)
        }
        ChartWidgetKind::Momentum => {
            let c = if wd.rsi > 70.0 { t.bull } else if wd.rsi < 30.0 { t.bear } else { Color32::from_rgb(255, 191, 0) };
            ("RSI", format!("{:.0}", wd.rsi), c)
        }
        ChartWidgetKind::Volatility => {
            ("ATR", if wd.atr > 1.0 { format!("{:.2}", wd.atr) } else { format!("{:.4}", wd.atr) }, t.accent)
        }
        ChartWidgetKind::SessionTimer => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            let day_secs = (now % 86400) as i64;
            let close_utc = 20 * 3600i64;
            let rem = if day_secs < close_utc { close_utc - day_secs } else { 86400 - day_secs + close_utc };
            let h = rem / 3600; let m = (rem % 3600) / 60;
            ("TMR", format!("{:02}:{:02}", h, m), t.text)
        }
        ChartWidgetKind::VolumeProfile => ("VOL", "profile".into(), t.dim),
        ChartWidgetKind::KeyLevels => {
            let pp = wd.price_levels[2].0;
            ("PP", format!("{:.2}", pp), t.accent)
        }
        ChartWidgetKind::OptionGreeks => ("\u{0394}", "0.45".into(), Color32::from_rgb(100, 200, 255)),
        ChartWidgetKind::RiskReward => ("R:R", "2.8:1".into(), t.bull),
        ChartWidgetKind::MarketBreadth => ("A/D", "1842/1156".into(), t.bull),
        ChartWidgetKind::Correlation => {
            let c = if wd.correlation_spy > 0.5 { t.bull } else if wd.correlation_spy > 0.0 { Color32::from_rgb(255, 191, 0) } else { t.bear };
            ("COR", format!("{:.2}", wd.correlation_spy), c)
        }
        ChartWidgetKind::DarkPool => {
            let c = if wd.dark_pool_ratio > 0.3 { t.accent } else { t.dim };
            ("DP", format!("{:.0}%", wd.dark_pool_ratio * 100.0), c)
        }
        ChartWidgetKind::PositionPnl => {
            if wd.position_qty != 0 {
                let c = if wd.position_pnl >= 0.0 { t.bull } else { t.bear };
                ("P&L", format!("{:+.0}", wd.position_pnl), c)
            } else { ("P&L", "flat".into(), t.dim) }
        }
        ChartWidgetKind::EarningsBadge => {
            if wd.earnings_days >= 0 {
                let c = if wd.earnings_days <= 3 { t.bear } else { t.accent };
                ("ERN", format!("{}d", wd.earnings_days), c)
            } else { ("ERN", "—".into(), t.dim) }
        }
        ChartWidgetKind::NewsTicker => ("NEWS", "live".into(), t.accent),
        ChartWidgetKind::ExitGauge => {
            let c = match wd.exit_gauge_urgency.as_str() {
                "exit_now" | "close" => t.bear, "partial" | "tighten" => Color32::from_rgb(255, 191, 0),
                _ => t.bull };
            ("EXIT", format!("{:.0}", wd.exit_gauge_score), c)
        }
        ChartWidgetKind::PrecursorAlert => {
            if wd.precursor_active {
                let c = if wd.precursor_dir > 0 { t.bull } else { t.bear };
                ("\u{26A1}", format!("{:.0}", wd.precursor_score), c)
            } else { ("\u{26A1}", "quiet".into(), t.dim) }
        }
        ChartWidgetKind::TradePlan => {
            if let Some((dir, _, _, _, rr, _)) = wd.trade_plan {
                let c = if dir > 0 { t.bull } else { t.bear };
                ("PLAN", format!("{:.1}R", rr), c)
            } else { ("PLAN", "—".into(), t.dim) }
        }
        ChartWidgetKind::ChangePoints => ("CP", format!("{}", wd.change_points_count), t.accent),
        ChartWidgetKind::ZoneStrength => ("ZNS", format!("{}", wd.zone_count), t.accent),
        ChartWidgetKind::PatternScanner => {
            if wd.pattern_count > 0 {
                let c = if wd.pattern_latest_bull { t.bull } else { t.bear };
                ("PAT", wd.pattern_latest.chars().take(6).collect(), c)
            } else { ("PAT", "—".into(), t.dim) }
        }
        ChartWidgetKind::VixMonitor => {
            let c = if wd.vix_spot > 25.0 { t.bear } else if wd.vix_spot > 18.0 { Color32::from_rgb(255, 191, 0) } else { t.bull };
            ("VIX", format!("{:.1}", wd.vix_spot), c)
        }
        ChartWidgetKind::SignalDashboard => ("SIG", "dash".into(), t.accent),
        ChartWidgetKind::DivergenceMonitor => ("DIV", format!("{}", wd.divergence_count), t.accent),
        ChartWidgetKind::ConvictionMeter => {
            let score = compute_conviction(wd);
            let c = if score > 70.0 { t.bull } else if score > 40.0 { Color32::from_rgb(255, 191, 0) } else { t.bear };
            ("\u{2605}", format!("{:.0}", score), c)
        }
        ChartWidgetKind::RsiMulti => {
            let avg: f32 = wd.rsi_multi.iter().sum::<f32>() / 7.0;
            let c = if avg > 60.0 { t.bull } else if avg < 40.0 { t.bear } else { egui::Color32::from_rgb(255, 191, 0) };
            ("RSI", format!("{:.0}", avg), c)
        }
        ChartWidgetKind::TrendAlign => {
            let aligned = wd.trend_grid.iter().filter(|r| r.iter().all(|&v| v)).count();
            let c = if aligned >= 5 { t.bull } else if aligned >= 3 { egui::Color32::from_rgb(255, 191, 0) } else { t.bear };
            ("TRD", format!("{}/7", aligned), c)
        }
        ChartWidgetKind::VolumeShelf => ("VOL", format!("{}", wd.vol_shelves.len()), t.accent),
        ChartWidgetKind::Confluence => ("S/R", format!("{}", wd.confluence_zones.len()), t.accent),
        ChartWidgetKind::FlowCompass => ("FLW", "—".into(), t.accent),
        ChartWidgetKind::VolRegime => ("VOL", wd.vol_regime_label.into(), t.accent),
        ChartWidgetKind::MomentumHeat => {
            let avg: f32 = wd.roc_bars.iter().sum::<f32>() / 8.0;
            let c = if avg > 0.0 { t.bull } else { t.bear };
            ("ROC", format!("{:+.1}", avg), c)
        }
        ChartWidgetKind::BreadthThermo => {
            let c = if wd.breadth_score > 60.0 { t.bull } else if wd.breadth_score < 40.0 { t.bear } else { t.dim };
            ("BRD", format!("{:.0}", wd.breadth_score), c)
        }
        ChartWidgetKind::SectorRotation => ("SEC", "radar".into(), t.accent),
        ChartWidgetKind::OptionsSentiment => ("OPT", "—".into(), t.accent),
        ChartWidgetKind::RelStrength => {
            let c = if wd.rs_rank > 70.0 { t.bull } else if wd.rs_rank < 30.0 { t.bear } else { t.dim };
            ("RS", format!("{:.0}", wd.rs_rank), c)
        }
        ChartWidgetKind::RiskDash => ("RSK", "calc".into(), t.accent),
        ChartWidgetKind::EarningsMom => ("ERN", "—".into(), t.accent),
        ChartWidgetKind::SignalRadar => {
            let active = if wd.trend_score > 0.0 { 1 } else { 0 }
                + if wd.precursor_active { 1 } else { 0 }
                + if wd.trade_plan.is_some() { 1 } else { 0 }
                + if wd.exit_gauge_score > 0.0 { 1 } else { 0 };
            ("SIG", format!("{}/10", active), t.accent)
        }
        ChartWidgetKind::CrossAssetPulse => ("MKT", "live".into(), t.accent),
        ChartWidgetKind::TapeSpeed => {
            let speed = wd.vol_ratio;
            let c = if speed > 2.0 { t.bear } else if speed > 1.2 { egui::Color32::from_rgb(255, 191, 0) } else { t.bull };
            ("SPD", format!("{:.1}x", speed), c)
        }
        ChartWidgetKind::Fundamentals => ("PE", format!("{:.1}", wd.pe_ratio), t.accent),
        ChartWidgetKind::EconCalendar => ("CAL", format!("{}", wd.econ_count), t.accent),
        ChartWidgetKind::Latency => ("LAT", "ok".into(), t.bull),
        ChartWidgetKind::PayoffChart => ("OPT", "P&L".into(), t.accent),
        ChartWidgetKind::OptionsFlow => ("FLW", "scan".into(), t.accent),
        ChartWidgetKind::LiquidityScore => {
            let c = if wd.liquidity_score > 70.0 { t.bull } else if wd.liquidity_score < 30.0 { t.bear } else { t.dim };
            ("LIQ", format!("{:.0}", wd.liquidity_score), c)
        }
        ChartWidgetKind::PositionsPanel => {
            let n = wd.all_positions.len();
            let c = if wd.day_pnl >= 0.0 { t.bull } else { t.bear };
            ("POS", format!("{} / {:+.0}", n, wd.day_pnl), c)
        }
        ChartWidgetKind::DailyPnl => {
            let c = if wd.day_pnl >= 0.0 { t.bull } else { t.bear };
            ("P&L", format!("{:+.0}", wd.day_pnl), c)
        }
        ChartWidgetKind::Custom => ("USR", "—".into(), t.dim),
    }
}

// ══════════════════════════════════════════════════════════════════��════════════
// Widget body dispatcher
// ═══════════════════════════════════════════════════════════════════════════════

/// Action from a button inside a widget body.
#[derive(Clone, Copy)]
pub(crate) enum WidgetBtnAction {
    CloseAllPositions,
    ClosePosition(usize), // index
}

/// Public entry point for rendering a widget body (used by dashboard pane).
pub(crate) fn draw_widget_body_pub(p: &egui::Painter, body: egui::Rect, kind: ChartWidgetKind,
                    wd: &WidgetData, t: &Theme, hover: Option<egui::Pos2>,
                    btns: &mut Vec<(egui::Rect, WidgetBtnAction)>) {
    draw_widget_body(p, body, kind, wd, t, hover, btns);
}

fn draw_widget_body(p: &egui::Painter, body: egui::Rect, kind: ChartWidgetKind,
                    wd: &WidgetData, t: &Theme, hover: Option<egui::Pos2>,
                    btns: &mut Vec<(egui::Rect, WidgetBtnAction)>) {
    match kind {
        ChartWidgetKind::TrendStrength => draw_trend_gauge(p, body, wd, t),
        ChartWidgetKind::Momentum      => draw_momentum_gauge(p, body, wd, t),
        ChartWidgetKind::Volatility    => draw_volatility_widget(p, body, wd, t),
        ChartWidgetKind::VolumeProfile => draw_volume_profile(p, body, wd, t),
        ChartWidgetKind::SessionTimer  => draw_session_timer(p, body, t),
        ChartWidgetKind::KeyLevels     => draw_key_levels(p, body, wd, t),
        ChartWidgetKind::OptionGreeks  => draw_option_greeks(p, body, t),
        ChartWidgetKind::RiskReward    => draw_risk_reward(p, body, wd, t),
        ChartWidgetKind::MarketBreadth => draw_market_breadth(p, body, t),
        ChartWidgetKind::Correlation   => draw_correlation(p, body, wd, t),
        ChartWidgetKind::DarkPool      => draw_dark_pool(p, body, wd, t),
        ChartWidgetKind::PositionPnl   => draw_position_pnl(p, body, wd, t),
        ChartWidgetKind::EarningsBadge => draw_earnings_badge(p, body, wd, t),
        ChartWidgetKind::NewsTicker    => draw_news_ticker(p, body, wd, t),
        ChartWidgetKind::ExitGauge     => draw_exit_gauge(p, body, wd, t),
        ChartWidgetKind::PrecursorAlert=> draw_precursor_alert(p, body, wd, t),
        ChartWidgetKind::TradePlan     => draw_trade_plan(p, body, wd, t),
        ChartWidgetKind::ChangePoints  => draw_change_points(p, body, wd, t),
        ChartWidgetKind::ZoneStrength  => draw_zone_strength(p, body, wd, t),
        ChartWidgetKind::PatternScanner=> draw_pattern_scanner(p, body, wd, t),
        ChartWidgetKind::VixMonitor    => draw_vix_monitor(p, body, wd, t),
        ChartWidgetKind::SignalDashboard=> draw_signal_dashboard(p, body, wd, t),
        ChartWidgetKind::DivergenceMonitor => draw_divergence_monitor(p, body, wd, t),
        ChartWidgetKind::ConvictionMeter=> draw_conviction_meter(p, body, wd, t),
        ChartWidgetKind::RsiMulti      => draw_rsi_multi(p, body, wd, t),
        ChartWidgetKind::TrendAlign    => draw_trend_align(p, body, wd, t),
        ChartWidgetKind::VolumeShelf   => draw_volume_shelf(p, body, wd, t),
        ChartWidgetKind::Confluence    => draw_confluence(p, body, wd, t),
        ChartWidgetKind::FlowCompass   => draw_flow_compass(p, body, wd, t),
        ChartWidgetKind::VolRegime     => draw_vol_regime(p, body, wd, t),
        ChartWidgetKind::MomentumHeat  => draw_momentum_heat(p, body, wd, t),
        ChartWidgetKind::BreadthThermo => draw_breadth_thermo(p, body, wd, t),
        ChartWidgetKind::SectorRotation=> draw_sector_rotation(p, body, wd, t),
        ChartWidgetKind::OptionsSentiment => draw_options_sentiment(p, body, wd, t),
        ChartWidgetKind::RelStrength   => draw_rel_strength(p, body, wd, t),
        ChartWidgetKind::RiskDash      => draw_risk_dash(p, body, wd, t),
        ChartWidgetKind::EarningsMom   => draw_earnings_mom(p, body, wd, t),
        ChartWidgetKind::LiquidityScore=> draw_liquidity_score(p, body, wd, t),
        ChartWidgetKind::SignalRadar   => draw_signal_radar(p, body, wd, t),
        ChartWidgetKind::CrossAssetPulse => draw_cross_asset(p, body, wd, t),
        ChartWidgetKind::TapeSpeed     => draw_tape_speed(p, body, wd, t),
        ChartWidgetKind::Fundamentals  => draw_fundamentals(p, body, wd, t),
        ChartWidgetKind::EconCalendar  => draw_econ_calendar(p, body, wd, t),
        ChartWidgetKind::Latency       => draw_latency(p, body, t),
        ChartWidgetKind::PayoffChart   => draw_payoff_chart(p, body, wd, t),
        ChartWidgetKind::OptionsFlow   => draw_options_flow(p, body, t),
        ChartWidgetKind::PositionsPanel=> draw_positions_panel(p, body, wd, t, hover, btns),
        ChartWidgetKind::DailyPnl      => draw_daily_pnl(p, body, wd, t, hover, btns),
        ChartWidgetKind::Custom        => draw_custom(p, body, t),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Live data extraction
// ═══════════════════════════════════════════════════════════════════════════════

/// Public alias for caching in Chart struct.
pub(crate) type WidgetDataCache = WidgetData;

#[derive(Clone)]
pub(crate) struct WidgetData {
    trend_score: f32,
    trend_dir: i8,
    trend_regime: String,
    rsi: f32,
    momentum: f32,
    atr: f32,
    atr_pct: f32,
    vol_ratio: f32,
    last_close: f32,
    _prev_close: f32,
    _day_change_pct: f32,
    vol_bars: [f32; 12],
    price_levels: [(f32, &'static str); 5],
    // New widget data
    symbol: String,
    correlation_spy: f32,  // -1..1 correlation with market
    dark_pool_bars: [f32; 8], // normalized unusual volume prints
    dark_pool_ratio: f32,  // dark pool % of total volume
    position_qty: i32,     // 0 = no position
    position_avg: f32,
    position_pnl: f32,
    position_pnl_pct: f32,
    earnings_days: i32,    // -1 = no upcoming earnings
    earnings_label: String,
    // ApexSignals data
    exit_gauge_score: f32,
    exit_gauge_urgency: String,
    precursor_active: bool,
    precursor_score: f32,
    precursor_dir: i8,
    precursor_desc: String,
    trade_plan: Option<(i8, f32, f32, f32, f32, f32)>, // (dir, entry, target, stop, rr, conviction)
    change_points_count: usize,
    change_points_latest: String,
    zone_count: usize,
    zone_fresh: usize,
    zone_avg_strength: f32,
    pattern_count: usize,
    pattern_latest: String,
    pattern_latest_bull: bool,
    pattern_latest_conf: f32,
    vix_spot: f32,
    vix_gap_pct: f32,
    vix_convergence: f32,
    divergence_count: usize,
    bars_loaded: bool, // false = show loading skeleton
    // Fundamentals
    pe_ratio: f32,
    eps_ttm: f32,
    market_cap_b: f32,
    dividend_yield: f32,
    revenue_growth: f32,
    profit_margin: f32,
    short_interest: f32,
    institutional_pct: f32,
    analyst_target: f32,
    analyst_buy: u8,
    analyst_hold: u8,
    analyst_sell: u8,
    econ_count: usize,
    econ_next_name: String,
    econ_next_days: i32,
    // RSI Multi-timeframe: [5m, 15m, 30m, 1h, 4h, 1d, 1w]
    rsi_multi: [f32; 7],
    // Trend alignment: 7 TFs × 4 indicators (EMA slope, MACD, price>VWAP, RSI>50)
    trend_grid: [[bool; 4]; 7],
    // Momentum ROC across 8 lookbacks
    roc_bars: [f32; 8],
    // Volume shelves: (price, volume_pct, is_support)
    vol_shelves: Vec<(f32, f32, bool)>,
    // Confluence zones: (price, count, distance_pct)
    confluence_zones: Vec<(f32, u8, f32)>,
    // Volatility regime metrics
    bb_width: f32,      // Bollinger bandwidth
    atr_percentile: f32, // ATR vs 100-bar range
    vol_regime_label: &'static str,
    // Breadth (simulated from chart data)
    breadth_score: f32,
    // Relative strength
    rs_rank: f32, // 0-100 percentile
    // Liquidity
    liquidity_score: f32,
    // All account positions (for PositionsPanel)
    all_positions: Vec<PositionRow>,
    day_pnl: f32,
}

/// One row in the positions panel.
#[derive(Clone)]
struct PositionRow {
    symbol: String,
    qty: i32,
    market_value: f64,
    unrealized_pnl: f32,
    pnl_pct: f32,
}

impl WidgetData {
    pub(crate) fn from_chart(chart: &Chart) -> Self {
        let bars = &chart.bars;
        let n = bars.len();
        let last_close = if n > 0 { bars[n - 1].close } else { 0.0 };
        let prev_close = if n > 1 { bars[n - 2].close } else { last_close };
        let day_change_pct = if prev_close > 0.0 { (last_close - prev_close) / prev_close * 100.0 } else { 0.0 };

        let rsi = compute_rsi(bars, 14);
        let momentum = if n > 10 && bars[n - 11].close > 0.0 {
            (last_close - bars[n - 11].close) / bars[n - 11].close * 100.0
        } else { 0.0 };

        let atr = compute_atr(bars, 14);
        let atr_pct = if last_close > 0.0 { atr / last_close * 100.0 } else { 0.0 };

        let vol_ratio = if n > 20 {
            let avg: f32 = bars[n-21..n-1].iter().map(|b| b.volume).sum::<f32>() / 20.0;
            if avg > 0.0 { bars[n - 1].volume / avg } else { 1.0 }
        } else { 1.0 };

        let mut vol_bars = [0.0f32; 12];
        if n >= 12 {
            let start = n - 12;
            let max_v = bars[start..n].iter().map(|b| b.volume).fold(0.0f32, f32::max).max(1.0);
            for i in 0..12 { vol_bars[i] = bars[start + i].volume / max_v; }
        }

        let (h, l) = if n > 0 {
            let recent = &bars[n.saturating_sub(20)..n];
            let hi = recent.iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max);
            let lo = recent.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
            (hi, lo)
        } else { (100.0, 90.0) };
        let pp = (h + l + last_close) / 3.0;
        let r1 = 2.0 * pp - l;
        let s1 = 2.0 * pp - h;
        let r2 = pp + (h - l);
        let s2 = pp - (h - l);

        // ── Correlation: compute from close-to-close returns (approximation) ──
        // In a real build this would correlate with SPY bars; here we use
        // autocorrelation of returns as a proxy for market coupling.
        let correlation_spy = compute_autocorrelation(bars, 20);

        // ── Dark pool: simulate unusual volume prints from volume spikes ──
        let mut dark_pool_bars = [0.0f32; 8];
        let mut dp_ratio = 0.0f32;
        if n >= 8 {
            let avg_vol: f32 = bars[n.saturating_sub(50)..n].iter().map(|b| b.volume).sum::<f32>()
                / bars[n.saturating_sub(50)..n].len() as f32;
            let start = n - 8;
            let max_dp = bars[start..n].iter()
                .map(|b| (b.volume / avg_vol.max(1.0) - 1.0).max(0.0))
                .fold(0.0f32, f32::max).max(0.01);
            for i in 0..8 {
                let spike = (bars[start + i].volume / avg_vol.max(1.0) - 1.0).max(0.0);
                dark_pool_bars[i] = spike / max_dp;
            }
            // Estimate "dark pool ratio" as fraction of volume above average
            let total_vol: f32 = bars[n.saturating_sub(20)..n].iter().map(|b| b.volume).sum();
            let above_avg: f32 = bars[n.saturating_sub(20)..n].iter()
                .map(|b| (b.volume - avg_vol).max(0.0)).sum();
            dp_ratio = if total_vol > 0.0 { above_avg / total_vol } else { 0.0 };
        }

        // ── Position P&L from ACCOUNT_DATA ──
        let (position_qty, position_avg, position_pnl, position_pnl_pct) =
            if let Some((_, positions, _)) = crate::chart_renderer::trading::read_account_data() {
                if let Some(pos) = positions.iter().find(|p| p.symbol == chart.symbol) {
                    let pnl_pct = if pos.avg_price > 0.0 {
                        (last_close - pos.avg_price) / pos.avg_price * 100.0
                            * if pos.qty < 0 { -1.0 } else { 1.0 }
                    } else { 0.0 };
                    (pos.qty, pos.avg_price, pos.unrealized_pnl as f32, pnl_pct)
                } else { (0, 0.0, 0.0, 0.0) }
            } else { (0, 0.0, 0.0, 0.0) };

        // ── Earnings from event_markers ──
        let (earnings_days, earnings_label) = chart.event_markers.iter()
            .filter(|em| em.event_type == 0) // earnings
            .min_by_key(|em| {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_secs() as i64;
                (em.time - now).abs()
            })
            .map(|em| {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_secs() as i64;
                let days = ((em.time - now) as f64 / 86400.0).ceil() as i32;
                (days, em.label.clone())
            })
            .unwrap_or((-1, String::new()));

        // ── ApexSignals data ──
        let trade_plan = chart.trade_plan.as_ref().map(|tp| (tp.0, tp.1, tp.2, tp.3, tp.5, tp.6));

        let (pattern_count, pattern_latest, pattern_latest_bull, pattern_latest_conf) =
            if chart.pattern_labels.is_empty() { (0, String::new(), false, 0.0) }
            else {
                let last = chart.pattern_labels.last().unwrap();
                (chart.pattern_labels.len(), last.label.clone(), last.bullish, last.confidence)
            };

        let zone_count = chart.signal_zones.len();
        let zone_fresh = chart.signal_zones.iter().filter(|z| z.fresh).count();
        let zone_avg_strength = if zone_count > 0 {
            chart.signal_zones.iter().map(|z| z.strength).sum::<f32>() / zone_count as f32
        } else { 0.0 };

        let change_points_count = chart.change_points.len();
        let change_points_latest = chart.change_points.last()
            .map(|(_, t, _)| t.clone()).unwrap_or_default();

        // ── All positions for PositionsPanel ──
        let (all_positions, day_pnl) = {
            let live = crate::chart_renderer::trading::read_account_data()
                .and_then(|(summary, positions, _)| {
                    if positions.is_empty() { None }
                    else {
                        let rows: Vec<PositionRow> = positions.iter().map(|p| {
                            let pnl_pct = if p.avg_price > 0.0 {
                                (p.current_price - p.avg_price) / p.avg_price * 100.0
                                    * if p.qty < 0 { -1.0 } else { 1.0 }
                            } else { 0.0 };
                            PositionRow {
                                symbol: p.symbol.clone(),
                                qty: p.qty,
                                market_value: p.market_value,
                                unrealized_pnl: p.unrealized_pnl as f32,
                                pnl_pct,
                            }
                        }).collect();
                        Some((rows, summary.daily_pnl as f32))
                    }
                });
            live.unwrap_or_else(|| {
                // Placeholder positions when no account connected
                let rows = vec![
                    PositionRow { symbol: "AAPL".into(),  qty: 100,  market_value: 21_450.0, unrealized_pnl: 325.0,  pnl_pct: 1.54 },
                    PositionRow { symbol: "NVDA".into(),  qty: 50,   market_value: 5_680.0,  unrealized_pnl: -142.0, pnl_pct: -2.44 },
                    PositionRow { symbol: "TSLA".into(),  qty: -30,  market_value: 7_920.0,  unrealized_pnl: 418.0,  pnl_pct: 5.57 },
                    PositionRow { symbol: "SPY".into(),   qty: 200,  market_value: 110_400.0,unrealized_pnl: -89.0,  pnl_pct: -0.08 },
                    PositionRow { symbol: "MSFT".into(),  qty: 75,   market_value: 31_500.0, unrealized_pnl: 210.0,  pnl_pct: 0.67 },
                ];
                let total: f32 = rows.iter().map(|r| r.unrealized_pnl).sum();
                (rows, total)
            })
        };

        WidgetData {
            trend_score: chart.trend_health_score,
            trend_dir: chart.trend_health_direction,
            trend_regime: chart.trend_health_regime.clone(),
            rsi, momentum, atr, atr_pct, vol_ratio,
            last_close, _prev_close: prev_close, _day_change_pct: day_change_pct, vol_bars,
            price_levels: [(r2, "R2"), (r1, "R1"), (pp, "PP"), (s1, "S1"), (s2, "S2")],
            symbol: chart.symbol.clone(),
            correlation_spy, dark_pool_bars, dark_pool_ratio: dp_ratio,
            position_qty, position_avg, position_pnl, position_pnl_pct,
            earnings_days, earnings_label,
            exit_gauge_score: chart.exit_gauge_score,
            exit_gauge_urgency: chart.exit_gauge_urgency.clone(),
            precursor_active: chart.precursor_active,
            precursor_score: chart.precursor_score,
            precursor_dir: chart.precursor_direction,
            precursor_desc: chart.precursor_description.clone(),
            trade_plan, change_points_count, change_points_latest,
            zone_count, zone_fresh, zone_avg_strength,
            pattern_count, pattern_latest, pattern_latest_bull, pattern_latest_conf,
            vix_spot: chart.vix_spot, vix_gap_pct: chart.vix_gap_pct,
            vix_convergence: chart.vix_convergence_score,
            divergence_count: 0, // populated when divergence overlays are active
            bars_loaded: n > 0,
            // ── Fundamental data ──
            pe_ratio: chart.fundamentals.pe_ratio,
            eps_ttm: chart.fundamentals.eps_ttm,
            market_cap_b: chart.fundamentals.market_cap as f32,
            dividend_yield: chart.fundamentals.dividend_yield,
            revenue_growth: chart.fundamentals.revenue_growth,
            profit_margin: chart.fundamentals.profit_margin,
            short_interest: chart.fundamentals.short_interest,
            institutional_pct: chart.fundamentals.institutional_pct,
            analyst_target: chart.fundamentals.analyst_target_mean,
            analyst_buy: chart.fundamentals.analyst_buy,
            analyst_hold: chart.fundamentals.analyst_hold,
            analyst_sell: chart.fundamentals.analyst_sell,
            econ_count: chart.econ_calendar.len(),
            econ_next_name: chart.econ_calendar.first().map(|e| e.name.clone()).unwrap_or_default(),
            econ_next_days: chart.econ_calendar.first().map(|e| {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
                ((e.time - now) as f64 / 86400.0).ceil() as i32
            }).unwrap_or(-1),
            // ── Computed analytics for new widgets ──
            trend_grid: compute_trend_grid(bars),
            roc_bars: compute_roc_bars(bars),
            vol_shelves: compute_vol_shelves(bars),
            confluence_zones: compute_confluence(bars, last_close),
            bb_width: compute_bb_width(bars),
            atr_percentile: compute_atr_percentile(bars),
            vol_regime_label: {
                let bbw = compute_bb_width(bars);
                let atrp = compute_atr_percentile(bars);
                if bbw < 0.03 && atrp < 30.0 { "SQUEEZE" }
                else if bbw > 0.08 || atrp > 70.0 { "EXPANSION" }
                else if atrp > 50.0 { "ELEVATED" }
                else { "NORMAL" }
            },
            breadth_score: compute_breadth(bars),
            rs_rank: compute_rs_rank(bars),
            liquidity_score: compute_liquidity(bars),
            rsi_multi: [
                compute_rsi(bars, 7),    // 5m  — fast
                compute_rsi(bars, 10),   // 15m — medium-fast
                compute_rsi(bars, 14),   // 30m — standard
                compute_rsi(bars, 21),   // 1h  — medium
                compute_rsi(bars, 42),   // 4h  — slow
                compute_rsi(bars, 70),   // 1d  — daily
                compute_rsi(bars, 140),  // 1w  — weekly
            ],
            all_positions, day_pnl,
        }
    }
}

/// Autocorrelation of returns as a proxy for market correlation.
fn compute_autocorrelation(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let n = bars.len();
    if n < period + 2 { return 0.0; }
    let mut returns: Vec<f32> = Vec::with_capacity(period);
    for i in (n - period)..n {
        if bars[i - 1].close > 0.0 {
            returns.push((bars[i].close - bars[i - 1].close) / bars[i - 1].close);
        }
    }
    if returns.len() < 4 { return 0.0; }
    let mean = returns.iter().sum::<f32>() / returns.len() as f32;
    let var: f32 = returns.iter().map(|r| (r - mean).powi(2)).sum::<f32>() / returns.len() as f32;
    if var < 1e-10 { return 0.0; }
    let mut cov = 0.0f32;
    for i in 1..returns.len() {
        cov += (returns[i] - mean) * (returns[i - 1] - mean);
    }
    cov /= (returns.len() - 1) as f32;
    (cov / var).clamp(-1.0, 1.0)
}

fn compute_rsi(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let n = bars.len();
    if n < period + 1 { return 50.0; }
    let mut gain_sum = 0.0f32;
    let mut loss_sum = 0.0f32;
    for i in (n - period)..n {
        let diff = bars[i].close - bars[i - 1].close;
        if diff > 0.0 { gain_sum += diff; } else { loss_sum += -diff; }
    }
    let avg_gain = gain_sum / period as f32;
    let avg_loss = loss_sum / period as f32;
    if avg_loss < 0.0001 { return 100.0; }
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

fn compute_atr(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let n = bars.len();
    if n < period + 1 { return 0.0; }
    let mut sum = 0.0f32;
    for i in (n - period)..n {
        let tr = (bars[i].high - bars[i].low)
            .max((bars[i].high - bars[i - 1].close).abs())
            .max((bars[i].low - bars[i - 1].close).abs());
        sum += tr;
    }
    sum / period as f32
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shared painting helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_arc(p: &egui::Painter, center: egui::Pos2, radius: f32, start: f32, end: f32,
            stroke: Stroke, segments: usize) {
    if segments < 2 { return; }
    let step = (end - start) / segments as f32;
    let points: Vec<egui::Pos2> = (0..=segments)
        .map(|i| {
            let a = start + step * i as f32;
            egui::pos2(center.x + radius * a.cos(), center.y - radius * a.sin())
        })
        .collect();
    for pair in points.windows(2) {
        p.line_segment([pair[0], pair[1]], stroke);
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color32::from_rgb(
        (a.r() as f32 * inv + b.r() as f32 * t) as u8,
        (a.g() as f32 * inv + b.g() as f32 * t) as u8,
        (a.b() as f32 * inv + b.b() as f32 * t) as u8,
    )
}

/// Hero number — large proportional display font, the focal point of every widget.
fn hero_number(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER,
        text, egui::FontId::proportional(28.0), color);
}

/// Even larger hero for primary KPIs.
fn hero_number_lg(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER,
        text, egui::FontId::proportional(36.0), color);
}

/// Small uppercase label — editorial style, tracked monospace.
fn sub_label(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER,
        text, egui::FontId::monospace(7.0),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 160));
}

/// Donut ring gauge — thick arc with value in center (infographic style).
fn donut_ring(p: &egui::Painter, center: egui::Pos2, radius: f32, thickness: f32,
              value: f32, max: f32, color: Color32, track_color: Color32) {
    let segs = 48;
    let tau = std::f32::consts::TAU;

    // Track (full circle)
    draw_arc(p, center, radius, 0.0, tau, egui::Stroke::new(thickness, track_color), segs);

    // Value arc (starting from top, going clockwise)
    let frac = (value / max).clamp(0.0, 1.0);
    // Rotate so 0 is at top: start at -PI/2, go clockwise
    let start = -std::f32::consts::FRAC_PI_2;
    for i in 0..segs {
        let t0 = i as f32 / segs as f32;
        let t1 = (i + 1) as f32 / segs as f32;
        if t0 >= frac { break; }
        let t1 = t1.min(frac);
        let a0 = start + t0 * tau;
        let a1 = start + t1 * tau;
        let p0 = egui::pos2(center.x + radius * a0.cos(), center.y + radius * a0.sin());
        let p1 = egui::pos2(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        p.line_segment([p0, p1], egui::Stroke::new(thickness, color));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Widget renderers (unchanged from premium version)
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_trend_gauge(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let score = if wd.trend_score > 0.0 { wd.trend_score } else { 72.0 };

    let color = if score > 66.0 {
        lerp_color(Color32::from_rgb(255, 191, 0), t.bull, (score - 66.0) / 34.0)
    } else if score > 33.0 {
        lerp_color(t.bear, Color32::from_rgb(255, 191, 0), (score - 33.0) / 33.0)
    } else { t.bear };

    // Donut ring gauge (infographic style)
    let gauge_cy = body.top() + 42.0;
    let r = 28.0;
    let track = color_alpha(t.toolbar_border, ALPHA_MUTED);
    donut_ring(p, egui::pos2(cx, gauge_cy), r, 5.0, score, 100.0, color, track);

    // Score in center of donut
    p.text(egui::pos2(cx, gauge_cy), egui::Align2::CENTER_CENTER,
        &format!("{:.0}", score), egui::FontId::proportional(22.0), color);

    // Regime label below donut
    let regime = if wd.trend_regime.is_empty() {
        if score > 66.0 { "STRONG" } else if score > 33.0 { "MIXED" } else { "WEAK" }
    } else { &wd.trend_regime };
    sub_label(p, egui::pos2(cx, gauge_cy + r + 14.0), regime, color);

    // Direction arrow
    let dir_icon = match wd.trend_dir { d if d > 0 => "\u{25B2}", d if d < 0 => "\u{25BC}", _ => "\u{25C6}" };
    let dir_col = match wd.trend_dir { d if d > 0 => t.bull, d if d < 0 => t.bear, _ => t.dim };
    p.text(egui::pos2(cx, gauge_cy + r + 26.0), egui::Align2::CENTER_CENTER,
        dir_icon, egui::FontId::proportional(FONT_SM), dir_col);
}

fn draw_momentum_gauge(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let rsi = wd.rsi;
    let mom = wd.momentum;

    let rsi_color = if rsi > 70.0 { t.bull }
        else if rsi < 30.0 { t.bear }
        else { Color32::from_rgb(255, 191, 0) };

    // Donut ring for RSI
    let gauge_cy = body.top() + 40.0;
    let r = 26.0;
    let track = color_alpha(t.toolbar_border, ALPHA_MUTED);
    donut_ring(p, egui::pos2(cx, gauge_cy), r, 5.0, rsi, 100.0, rsi_color, track);

    // RSI value in center
    p.text(egui::pos2(cx, gauge_cy), egui::Align2::CENTER_CENTER,
        &format!("{:.0}", rsi), egui::FontId::proportional(20.0), rsi_color);

    let zone = if rsi > 70.0 { "OVERBOUGHT" } else if rsi < 30.0 { "OVERSOLD" } else { "NEUTRAL" };
    sub_label(p, egui::pos2(cx, gauge_cy + r + 12.0), zone, rsi_color);

    // Momentum ROC at bottom
    let mom_col = if mom > 0.0 { t.bull } else { t.bear };
    let mom_sign = if mom > 0.0 { "+" } else { "" };
    p.text(egui::pos2(body.right() - 8.0, body.bottom() - 8.0), egui::Align2::RIGHT_CENTER,
        &format!("{}{:.1}%", mom_sign, mom), egui::FontId::monospace(FONT_XS), mom_col);
    p.text(egui::pos2(body.left() + 8.0, body.bottom() - 8.0), egui::Align2::LEFT_CENTER,
        "ROC", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
}

fn draw_volatility_widget(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let atr_str = if wd.atr > 1.0 { format!("{:.2}", wd.atr) } else { format!("{:.4}", wd.atr) };
    hero_number(p, egui::pos2(cx, body.top() + 18.0), &atr_str, t.accent);
    sub_label(p, egui::pos2(cx, body.top() + 36.0), "ATR (14)", t.dim);

    let bar_y = body.top() + 50.0;
    let bar_x = body.left() + 12.0;
    let bar_w = body.width() - 24.0;
    let bar_h = 6.0;

    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, bar_h)),
        3.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
    let pct = (wd.atr_pct / 5.0).clamp(0.0, 1.0);
    let vol_color = if wd.atr_pct > 3.0 { t.bear }
        else if wd.atr_pct > 1.5 { Color32::from_rgb(255, 191, 0) }
        else { t.bull };
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w * pct, bar_h)),
        3.0, vol_color);
    p.text(egui::pos2(cx, bar_y + 14.0), egui::Align2::CENTER_CENTER,
        &format!("{:.2}% of price", wd.atr_pct), egui::FontId::monospace(FONT_XS), vol_color);

    let vr_y = body.bottom() - 14.0;
    let vr_col = if wd.vol_ratio > 1.5 { t.bull } else if wd.vol_ratio > 0.7 { t.dim } else { t.bear };
    p.text(egui::pos2(body.left() + 12.0, vr_y), egui::Align2::LEFT_CENTER,
        "RVOL", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(body.right() - 12.0, vr_y), egui::Align2::RIGHT_CENTER,
        &format!("{:.1}x", wd.vol_ratio), egui::FontId::monospace(FONT_SM), vr_col);
}

fn draw_volume_profile(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let bar_x = body.left() + 10.0;
    let max_w = body.width() - 20.0;
    let n = wd.vol_bars.len();
    let total_h = body.height() - 12.0;
    let bar_h = (total_h / n as f32).min(12.0);
    let gap = ((total_h - bar_h * n as f32) / (n as f32 - 1.0).max(1.0)).max(1.0);

    let max_idx = wd.vol_bars.iter().enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i).unwrap_or(0);

    for i in 0..n {
        let y = body.top() + 6.0 + i as f32 * (bar_h + gap);
        let w = max_w * wd.vol_bars[i].max(0.03);
        let is_poc = i == max_idx;

        let color = if is_poc { t.accent } else {
            let t_val = i as f32 / n as f32;
            lerp_color(Color32::from_rgb(80, 120, 200), Color32::from_rgb(140, 80, 180), t_val)
        };
        let alpha = if is_poc { ALPHA_STRONG } else { ALPHA_DIM };
        let bar_rect = egui::Rect::from_min_size(egui::pos2(bar_x, y), egui::vec2(w, bar_h));
        p.rect_filled(bar_rect, 2.0, color_alpha(color, alpha));

        if is_poc {
            p.rect_filled(bar_rect.expand(1.0), 3.0,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 20));
            p.text(egui::pos2(bar_x + w + 4.0, y + bar_h / 2.0),
                egui::Align2::LEFT_CENTER, "POC", egui::FontId::monospace(7.0), t.accent);
        }
    }
}

fn draw_session_timer(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let cx = body.center().x;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let day_secs = (now % 86400) as i64;
    let close_utc = 20 * 3600i64;
    let remaining = if day_secs < close_utc { close_utc - day_secs } else { 86400 - day_secs + close_utc };

    let h = remaining / 3600;
    let m = (remaining % 3600) / 60;
    let s = remaining % 60;

    let ring_cy = body.top() + 22.0;
    let ring_r = 16.0;
    let total_session = 6.5 * 3600.0;
    let elapsed_frac = 1.0 - (remaining as f32 / total_session).clamp(0.0, 1.0);

    draw_arc(p, egui::pos2(cx, ring_cy), ring_r, 0.0, 2.0 * PI,
        Stroke::new(2.0, color_alpha(t.toolbar_border, ALPHA_MUTED)), 60);

    let progress_color = if elapsed_frac > 0.9 { t.bear }
        else if elapsed_frac > 0.7 { Color32::from_rgb(255, 191, 0) }
        else { t.accent };
    let sweep = elapsed_frac * 2.0 * PI;
    draw_arc(p, egui::pos2(cx, ring_cy), ring_r, PI / 2.0, PI / 2.0 - sweep,
        Stroke::new(2.5, progress_color), 40);

    let time_str = format!("{:02}:{:02}:{:02}", h, m, s);
    hero_number(p, egui::pos2(cx, body.top() + 48.0), &time_str, t.text);
    sub_label(p, egui::pos2(cx, body.top() + 66.0), "TO CLOSE", t.dim);
}

fn draw_key_levels(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let row_h = (body.height() - 8.0) / 5.0;

    for (i, (price, label)) in wd.price_levels.iter().enumerate() {
        let y = body.top() + 4.0 + i as f32 * row_h + row_h / 2.0;
        let is_pp = *label == "PP";
        let is_r = label.starts_with('R');

        let level_color = if is_pp { t.accent }
            else if is_r { Color32::from_rgb(220, 80, 80) }
            else { Color32::from_rgb(80, 180, 120) };

        let badge_w = 24.0;
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(left, y - 8.0), egui::vec2(badge_w, 16.0));
        p.rect_filled(badge_rect, 3.0, color_alpha(level_color, ALPHA_TINT));
        p.text(badge_rect.center(), egui::Align2::CENTER_CENTER,
            *label, egui::FontId::monospace(FONT_XS), level_color);

        let line_x_start = left + badge_w + 6.0;
        let line_x_end = right - 50.0;
        let dash_len = 4.0;
        let gap_len = 3.0;
        let mut x = line_x_start;
        while x < line_x_end {
            let end = (x + dash_len).min(line_x_end);
            p.line_segment([egui::pos2(x, y), egui::pos2(end, y)],
                Stroke::new(STROKE_HAIR, color_alpha(level_color, ALPHA_MUTED)));
            x += dash_len + gap_len;
        }

        let font_size = if is_pp { FONT_LG } else { FONT_SM };
        p.text(egui::pos2(right, y), egui::Align2::RIGHT_CENTER,
            &format!("{:.2}", price), egui::FontId::monospace(font_size), level_color);

        if wd.last_close > 0.0 {
            let dist = (price - wd.last_close) / wd.last_close * 100.0;
            let dist_col = if dist.abs() < 0.5 { t.accent } else { t.dim.gamma_multiply(0.4) };
            p.text(egui::pos2(right, y + 9.0), egui::Align2::RIGHT_CENTER,
                &format!("{:+.1}%", dist), egui::FontId::monospace(7.0), dist_col);
        }
    }
}

fn draw_option_greeks(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let greeks: [(&str, f32, Color32); 4] = [
        ("\u{0394} Delta", 0.45, Color32::from_rgb(100, 200, 255)),
        ("\u{0393} Gamma", 0.032, Color32::from_rgb(180, 130, 255)),
        ("\u{0398} Theta", -0.12, Color32::from_rgb(255, 140, 100)),
        ("\u{03BD} Vega",  0.085, Color32::from_rgb(100, 230, 180)),
    ];

    let row_h = (body.height() - 8.0) / 4.0;
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let bar_max_w = body.width() * 0.35;

    for (i, (name, val, color)) in greeks.iter().enumerate() {
        let y = body.top() + 4.0 + i as f32 * row_h + row_h / 2.0;
        p.text(egui::pos2(left, y), egui::Align2::LEFT_CENTER,
            *name, egui::FontId::monospace(FONT_SM), *color);
        let val_str = if val.abs() < 0.01 { format!("{:.3}", val) } else { format!("{:.2}", val) };
        p.text(egui::pos2(right, y), egui::Align2::RIGHT_CENTER,
            &val_str, egui::FontId::monospace(FONT_LG), t.text);
        let bar_x = left + 64.0;
        let bar_w = (val.abs() * bar_max_w * 2.0).min(bar_max_w);
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(bar_x, y - 3.0), egui::vec2(bar_w, 6.0));
        p.rect_filled(bar_rect, 2.0, color_alpha(*color, ALPHA_DIM));
    }
}

fn draw_risk_reward(p: &egui::Painter, body: egui::Rect, _wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let risk = 1.0f32;
    let reward = 2.8f32;
    let total = risk + reward;
    let bar_w = body.width() - 24.0;
    let bar_x = body.left() + 12.0;
    let bar_y = body.top() + 12.0;
    let bar_h = 10.0;

    let risk_w = bar_w * (risk / total);
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(risk_w, bar_h)),
        egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, color_alpha(t.bear, ALPHA_STRONG));
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x + risk_w, bar_y), egui::vec2(bar_w - risk_w, bar_h)),
        egui::CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 }, color_alpha(t.bull, ALPHA_STRONG));
    p.circle_filled(egui::pos2(bar_x + risk_w, bar_y + bar_h / 2.0), 4.0, Color32::WHITE);

    let rr_str = format!("{:.1} : 1", reward);
    let rr_col = if reward >= 2.0 { t.bull } else if reward >= 1.0 { Color32::from_rgb(255, 191, 0) } else { t.bear };
    hero_number(p, egui::pos2(cx, body.top() + 40.0), &rr_str, rr_col);

    p.text(egui::pos2(bar_x, bar_y + bar_h + 6.0), egui::Align2::LEFT_TOP,
        "RISK", egui::FontId::monospace(7.0), t.bear.gamma_multiply(0.7));
    p.text(egui::pos2(bar_x + bar_w, bar_y + bar_h + 6.0), egui::Align2::RIGHT_TOP,
        "REWARD", egui::FontId::monospace(7.0), t.bull.gamma_multiply(0.7));
    sub_label(p, egui::pos2(cx, body.top() + 58.0), "RISK / REWARD", t.dim);
}

fn draw_market_breadth(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let metrics: [(&str, &str, Color32, f32); 4] = [
        ("ADV / DEC", "1,842 / 1,156", t.bull, 0.614),
        ("NEW HI", "48", Color32::from_rgb(100, 200, 255), 0.4),
        ("NEW LO", "12", Color32::from_rgb(255, 140, 100), 0.1),
        ("VIX", "18.5", Color32::from_rgb(255, 191, 0), 0.37),
    ];

    let row_h = (body.height() - 8.0) / 4.0;
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;

    for (i, (label, value, color, bar_pct)) in metrics.iter().enumerate() {
        let y = body.top() + 4.0 + i as f32 * row_h;
        p.text(egui::pos2(left, y + 5.0), egui::Align2::LEFT_TOP,
            *label, egui::FontId::monospace(7.0),
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120));
        p.text(egui::pos2(right, y + 5.0), egui::Align2::RIGHT_TOP,
            *value, egui::FontId::monospace(FONT_SM), *color);
        let bar_y = y + 16.0;
        let bar_w = body.width() - 20.0;
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(left, bar_y), egui::vec2(bar_w, 3.0)),
            1.0, color_alpha(t.toolbar_border, ALPHA_FAINT));
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(left, bar_y), egui::vec2(bar_w * bar_pct, 3.0)),
            1.0, color_alpha(*color, ALPHA_DIM));
    }
}

fn draw_rsi_multi(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y - 4.0;
    let tf_labels = ["5m", "15m", "30m", "1h", "4h", "1D", "1W"];

    // Ring geometry: outermost = fastest (5m), innermost = slowest (1W)
    let max_r = (body.width().min(body.height()) * 0.44).min(88.0);
    let ring_gap = 2.0;
    let ring_w = ((max_r - 14.0) / 7.0 - ring_gap).max(3.0);
    let pi2 = std::f32::consts::TAU;
    let start_angle = -std::f32::consts::FRAC_PI_2; // top

    // Zone markers: thin reference arcs at RSI 30 and 70
    let os_frac = 30.0 / 100.0; // oversold
    let ob_frac = 70.0 / 100.0; // overbought

    for (i, &rsi) in wd.rsi_multi.iter().enumerate() {
        let ring_idx = i; // 0=5m(outer), 6=1W(inner)
        let r = max_r - ring_idx as f32 * (ring_w + ring_gap);
        let frac = (rsi / 100.0).clamp(0.0, 1.0);

        // Color: gradient from bear (oversold) through amber to bull (overbought)
        let color = if rsi > 70.0 {
            t.bull
        } else if rsi > 55.0 {
            lerp_color(egui::Color32::from_rgb(255, 191, 0), t.bull, (rsi - 55.0) / 15.0)
        } else if rsi > 45.0 {
            egui::Color32::from_rgb(255, 191, 0) // amber neutral
        } else if rsi > 30.0 {
            lerp_color(t.bear, egui::Color32::from_rgb(255, 191, 0), (rsi - 30.0) / 15.0)
        } else {
            t.bear
        };

        // Track ring (full circle, very faint)
        let track_alpha = if ring_idx == 0 { ALPHA_MUTED } else { ALPHA_FAINT };
        draw_arc_ring(p, egui::pos2(cx, cy), r, ring_w, 0.0, pi2,
            color_alpha(t.toolbar_border, track_alpha), 64);

        // Value arc
        let sweep = frac * pi2;
        if sweep > 0.01 {
            draw_arc_ring(p, egui::pos2(cx, cy), r, ring_w, start_angle, sweep, color, 48);
        }

        // Oversold/overbought tick marks on the track
        for zone_frac in [os_frac, ob_frac] {
            let a = start_angle + zone_frac * pi2;
            let inner = r - ring_w * 0.5 - 1.0;
            let outer = r + ring_w * 0.5 + 1.0;
            let p1 = egui::pos2(cx + inner * a.cos(), cy + inner * a.sin());
            let p2 = egui::pos2(cx + outer * a.cos(), cy + outer * a.sin());
            p.line_segment([p1, p2], egui::Stroke::new(0.5, color_alpha(t.dim, ALPHA_FAINT)));
        }

        // Timeframe label — positioned at the end of the arc
        let label_angle = start_angle + sweep + 0.15;
        let label_r = r;
        let lx = cx + label_r * label_angle.cos();
        let ly = cy + label_r * label_angle.sin();
        // Only show if there's room (sweep > 30%)
        if frac > 0.15 {
            p.text(egui::pos2(lx, ly), egui::Align2::CENTER_CENTER,
                tf_labels[i], egui::FontId::monospace(6.0),
                color.gamma_multiply(0.7));
        }

        // Small dot at arc tip
        let tip_angle = start_angle + sweep;
        let dot_x = cx + r * tip_angle.cos();
        let dot_y = cy + r * tip_angle.sin();
        p.circle_filled(egui::pos2(dot_x, dot_y), ring_w * 0.35, color);
    }

    // Center: average RSI as hero number
    let avg: f32 = wd.rsi_multi.iter().sum::<f32>() / 7.0;
    let avg_col = if avg > 60.0 { t.bull } else if avg < 40.0 { t.bear } else { egui::Color32::from_rgb(255, 191, 0) };
    p.text(egui::pos2(cx, cy - 4.0), egui::Align2::CENTER_CENTER,
        &format!("{:.0}", avg), egui::FontId::proportional(24.0), avg_col);
    p.text(egui::pos2(cx, cy + 12.0), egui::Align2::CENTER_CENTER,
        "RSI", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));

    // Legend: oversold/overbought zones at bottom
    let legend_y = body.bottom() - 10.0;
    let legend_lx = body.left() + 8.0;
    p.circle_filled(egui::pos2(legend_lx, legend_y), 3.0, t.bear);
    p.text(egui::pos2(legend_lx + 8.0, legend_y), egui::Align2::LEFT_CENTER,
        "<30", egui::FontId::monospace(6.0), t.bear.gamma_multiply(0.7));
    p.circle_filled(egui::pos2(legend_lx + 35.0, legend_y), 3.0, egui::Color32::from_rgb(255, 191, 0));
    p.text(egui::pos2(legend_lx + 43.0, legend_y), egui::Align2::LEFT_CENTER,
        "30-70", egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
    p.circle_filled(egui::pos2(legend_lx + 80.0, legend_y), 3.0, t.bull);
    p.text(egui::pos2(legend_lx + 88.0, legend_y), egui::Align2::LEFT_CENTER,
        ">70", egui::FontId::monospace(6.0), t.bull.gamma_multiply(0.7));

    // Timeframe labels on the right side of each ring (static positions)
    let label_x = body.right() - 6.0;
    for (i, label) in tf_labels.iter().enumerate() {
        let r = max_r - i as f32 * (ring_w + ring_gap);
        let ly = cy - r;
        let rsi = wd.rsi_multi[i];
        let color = if rsi > 70.0 { t.bull } else if rsi < 30.0 { t.bear } else { t.dim };
        p.text(egui::pos2(label_x, ly), egui::Align2::RIGHT_CENTER,
            &format!("{} {:.0}", label, rsi), egui::FontId::monospace(6.5), color);
    }
}

/// Draw a thick arc ring (donut segment).
// ═══════════════════════════════════════════════════════════════════════════════
// Compute helpers for analytics widgets
// ═══════════════════════════════════════════════════════════════════════════════

fn compute_trend_grid(bars: &[crate::chart_renderer::types::Bar]) -> [[bool; 4]; 7] {
    let n = bars.len();
    let periods = [7, 10, 14, 21, 42, 70, 140]; // map to 7 timeframes
    let mut grid = [[false; 4]; 7];
    for (ti, &p) in periods.iter().enumerate() {
        if n < p + 5 { continue; }
        // Col 0: EMA slope positive
        let ema_now = bars[n-1..n].iter().map(|b| b.close).sum::<f32>();
        let ema_prev = bars[n-3..n-2].iter().map(|b| b.close).sum::<f32>();
        grid[ti][0] = ema_now > ema_prev;
        // Col 1: Close > SMA
        let sma: f32 = bars[n.saturating_sub(p)..n].iter().map(|b| b.close).sum::<f32>() / p.min(n) as f32;
        grid[ti][1] = bars[n-1].close > sma;
        // Col 2: RSI > 50
        grid[ti][2] = compute_rsi(bars, p) > 50.0;
        // Col 3: Higher high
        if n > p + 1 {
            grid[ti][3] = bars[n-1].high > bars[n.saturating_sub(p/2+1)].high;
        }
    }
    grid
}

fn compute_roc_bars(bars: &[crate::chart_renderer::types::Bar]) -> [f32; 8] {
    let n = bars.len();
    let lookbacks = [1, 2, 5, 10, 20, 60, 120, 252];
    let mut roc = [0.0f32; 8];
    for (i, &lb) in lookbacks.iter().enumerate() {
        if n > lb && bars[n - lb - 1].close > 0.0 {
            roc[i] = (bars[n-1].close - bars[n-lb-1].close) / bars[n-lb-1].close * 100.0;
        }
    }
    roc
}

fn compute_vol_shelves(bars: &[crate::chart_renderer::types::Bar]) -> Vec<(f32, f32, bool)> {
    let n = bars.len();
    if n < 20 { return vec![]; }
    let recent = &bars[n.saturating_sub(100)..n];
    let lo = recent.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
    let hi = recent.iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max);
    let range = (hi - lo).max(0.01);
    let bins = 10;
    let mut vol = vec![0.0f32; bins];
    for b in recent {
        let mid = (b.high + b.low) / 2.0;
        let idx = ((mid - lo) / range * (bins - 1) as f32) as usize;
        vol[idx.min(bins - 1)] += b.volume;
    }
    let max_vol = vol.iter().cloned().fold(0.0f32, f32::max).max(1.0);
    let last = bars[n-1].close;
    let mut shelves: Vec<(f32, f32, bool)> = vol.iter().enumerate()
        .filter(|(_, &v)| v > max_vol * 0.3)
        .map(|(i, &v)| {
            let price = lo + (i as f32 + 0.5) * range / bins as f32;
            (price, v / max_vol, price < last)
        }).collect();
    shelves.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    shelves.truncate(5);
    shelves
}

fn compute_confluence(bars: &[crate::chart_renderer::types::Bar], last: f32) -> Vec<(f32, u8, f32)> {
    let n = bars.len();
    if n < 20 || last < 0.01 { return vec![]; }
    let mut levels: Vec<f32> = Vec::new();
    // SMAs
    for p in [20, 50, 100, 200] {
        if n >= p { levels.push(bars[n.saturating_sub(p)..n].iter().map(|b| b.close).sum::<f32>() / p as f32); }
    }
    // Pivots
    let (h, l) = (bars[n.saturating_sub(20)..n].iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max),
                  bars[n.saturating_sub(20)..n].iter().map(|b| b.low).fold(f32::INFINITY, f32::min));
    let pp = (h + l + last) / 3.0;
    levels.extend_from_slice(&[pp, 2.0 * pp - l, 2.0 * pp - h]);
    // Previous highs/lows
    if n > 1 { levels.push(bars[n-2].high); levels.push(bars[n-2].low); }
    // Cluster: group levels within 0.3% of each other
    levels.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut zones: Vec<(f32, u8, f32)> = Vec::new();
    let mut i = 0;
    while i < levels.len() {
        let base = levels[i];
        let mut count = 1u8;
        let mut sum = base;
        while i + (count as usize) < levels.len() && (levels[i + (count as usize)] - base).abs() / last < 0.003 {
            sum += levels[i + (count as usize)]; count += 1;
        }
        if count >= 2 {
            let avg = sum / count as f32;
            zones.push((avg, count, (avg - last).abs() / last * 100.0));
        }
        i += count as usize;
    }
    zones.sort_by(|a, b| b.1.cmp(&a.1));
    zones.truncate(5);
    zones
}

fn compute_bb_width(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 20 { return 0.05; }
    let p = 20;
    let sma: f32 = bars[n-p..n].iter().map(|b| b.close).sum::<f32>() / p as f32;
    let var: f32 = bars[n-p..n].iter().map(|b| (b.close - sma).powi(2)).sum::<f32>() / p as f32;
    let std = var.sqrt();
    if sma > 0.0 { (4.0 * std) / sma } else { 0.05 }
}

fn compute_atr_percentile(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 100 { return 50.0; }
    let current_atr = compute_atr(bars, 14);
    let mut atrs: Vec<f32> = Vec::new();
    for i in 14..n.min(100) {
        atrs.push(compute_atr(&bars[..i+1], 14));
    }
    atrs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = atrs.partition_point(|&a| a < current_atr);
    (rank as f32 / atrs.len().max(1) as f32 * 100.0).clamp(0.0, 100.0)
}

fn compute_breadth(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 50 { return 50.0; }
    // Simulate breadth from % of recent bars closing above various MAs
    let mut score = 0.0f32;
    let last = bars[n-1].close;
    for p in [10, 20, 50] {
        if n >= p {
            let sma: f32 = bars[n-p..n].iter().map(|b| b.close).sum::<f32>() / p as f32;
            if last > sma { score += 33.3; }
        }
    }
    score.clamp(0.0, 100.0)
}

fn compute_rs_rank(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 60 { return 50.0; }
    // RS approximation: relative performance vs its own history
    let ret_20 = if bars[n-21].close > 0.0 { (bars[n-1].close / bars[n-21].close - 1.0) * 100.0 } else { 0.0 };
    let ret_60 = if n > 60 && bars[n-61].close > 0.0 { (bars[n-1].close / bars[n-61].close - 1.0) * 100.0 } else { 0.0 };
    let composite = ret_20 * 0.6 + ret_60 * 0.4;
    (50.0 + composite * 5.0).clamp(0.0, 100.0)
}

fn compute_liquidity(bars: &[crate::chart_renderer::types::Bar]) -> f32 {
    let n = bars.len();
    if n < 20 { return 50.0; }
    let recent = &bars[n-20..n];
    let avg_vol: f32 = recent.iter().map(|b| b.volume).sum::<f32>() / 20.0;
    let vol_std: f32 = (recent.iter().map(|b| (b.volume - avg_vol).powi(2)).sum::<f32>() / 20.0).sqrt();
    let cv = if avg_vol > 0.0 { vol_std / avg_vol } else { 1.0 }; // coefficient of variation
    let spread_proxy = recent.iter().map(|b| (b.high - b.low) / b.close.max(0.01)).sum::<f32>() / 20.0;
    let vol_score = (avg_vol / 1_000_000.0).min(1.0) * 40.0;
    let consistency_score = (1.0 - cv).max(0.0) * 30.0;
    let spread_score = (1.0 - spread_proxy * 20.0).max(0.0) * 30.0;
    (vol_score + consistency_score + spread_score).clamp(0.0, 100.0)
}

// ═══════════════════════════════════════════════════════════════════════════════
// New widget renderers — visually inspired by design references
// ═══════════════════════════════════════════════════════════════════════════════

/// Trend Alignment — dot grid (inspired by chart9 dot matrix)
fn draw_trend_align(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let tf_labels = ["5m", "15m", "30m", "1h", "4h", "1D", "1W"];
    let ind_labels = ["EMA", "SMA", "RSI", "HH"];
    let rows = 7; let cols = 4;
    let dot_r = 4.5;
    let gap_x = (body.width() - 32.0) / cols as f32;
    let gap_y = (body.height() - 24.0) / rows as f32;
    let ox = body.left() + 28.0;
    let oy = body.top() + 18.0;

    // Column headers
    for (j, label) in ind_labels.iter().enumerate() {
        p.text(egui::pos2(ox + j as f32 * gap_x + gap_x * 0.5, oy - 6.0),
            egui::Align2::CENTER_CENTER, label, egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
    }

    let mut bull_count = 0u32;
    let total = (rows * cols) as u32;

    for (i, tf) in tf_labels.iter().enumerate() {
        // Row label
        p.text(egui::pos2(body.left() + 14.0, oy + i as f32 * gap_y + gap_y * 0.5),
            egui::Align2::CENTER_CENTER, tf, egui::FontId::monospace(6.5), t.dim.gamma_multiply(0.6));

        for j in 0..cols {
            let bullish = wd.trend_grid[i][j];
            if bullish { bull_count += 1; }
            let cx = ox + j as f32 * gap_x + gap_x * 0.5;
            let cy = oy + i as f32 * gap_y + gap_y * 0.5;
            let color = if bullish { t.bull } else { color_alpha(t.dim, ALPHA_MUTED) };
            p.circle_filled(egui::pos2(cx, cy), dot_r, color);
        }
    }

    // Alignment score bottom-right
    let pct = bull_count as f32 / total as f32 * 100.0;
    let sc = if pct > 70.0 { t.bull } else if pct > 40.0 { egui::Color32::from_rgb(255, 191, 0) } else { t.bear };
    p.text(egui::pos2(body.right() - 6.0, body.bottom() - 8.0), egui::Align2::RIGHT_CENTER,
        &format!("{:.0}%", pct), egui::FontId::proportional(16.0), sc);
}

/// Volume Shelf — horizontal bars ranked by volume (chart4 stacked bars style)
fn draw_volume_shelf(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if wd.vol_shelves.is_empty() {
        p.text(body.center(), egui::Align2::CENTER_CENTER, "NO DATA", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        return;
    }
    let row_h = (body.height() - 8.0) / wd.vol_shelves.len().min(5) as f32;
    let max_w = body.width() - 60.0;

    for (i, (price, strength, is_support)) in wd.vol_shelves.iter().enumerate() {
        let y = body.top() + 4.0 + i as f32 * row_h;
        let color = if *is_support { t.bull } else { t.bear };
        let bar_w = max_w * strength;
        let label = if *is_support { "S" } else { "R" };

        // Bar
        let bar_rect = egui::Rect::from_min_size(egui::pos2(body.left() + 50.0, y + 2.0), egui::vec2(bar_w, row_h - 6.0));
        p.rect_filled(bar_rect, 3.0, color_alpha(color, ALPHA_DIM));

        // Price label
        p.text(egui::pos2(body.left() + 6.0, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:.1}", price), egui::FontId::monospace(FONT_XS), t.text);
        // S/R label inside bar
        p.text(egui::pos2(body.left() + 52.0, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            label, egui::FontId::monospace(7.0), color);
    }
}

/// Confluence Meter — stacked level bars with count badges
fn draw_confluence(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if wd.confluence_zones.is_empty() {
        p.text(body.center(), egui::Align2::CENTER_CENTER, "NO CLUSTERS", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        return;
    }
    let row_h = (body.height() - 4.0) / wd.confluence_zones.len().min(5) as f32;

    for (i, (price, count, dist)) in wd.confluence_zones.iter().enumerate() {
        let y = body.top() + 2.0 + i as f32 * row_h;
        let bar_w = (*count as f32 / 5.0).min(1.0) * (body.width() - 70.0);
        let proximity_alpha = (1.0 - dist / 3.0).max(0.2);
        let color = color_alpha(t.accent, (proximity_alpha * 180.0) as u8);

        // Confluence bar
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(body.left() + 55.0, y + 3.0),
            egui::vec2(bar_w, row_h - 8.0)), 2.0, color);

        // Price
        p.text(egui::pos2(body.left() + 6.0, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:.1}", price), egui::FontId::monospace(FONT_XS), t.text);
        // Count badge
        p.text(egui::pos2(body.left() + 48.0, y + row_h * 0.5), egui::Align2::CENTER_CENTER,
            &format!("{}x", count), egui::FontId::monospace(7.0), t.accent);
        // Distance
        p.text(egui::pos2(body.right() - 6.0, y + row_h * 0.5), egui::Align2::RIGHT_CENTER,
            &format!("{:.1}%", dist), egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
    }
}

/// Flow Compass — circular compass with directional needle (chart12 radial tick style)
fn draw_flow_compass(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y - 6.0;
    let r = (body.width().min(body.height()) * 0.36).min(65.0);

    // Dark circle background
    p.circle_filled(egui::pos2(cx, cy), r + 2.0, color_alpha(t.toolbar_border, ALPHA_DIM));

    // Radial tick marks (chart12 style)
    let ticks = 36;
    for i in 0..ticks {
        let a = (i as f32 / ticks as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let is_major = i % 9 == 0;
        let inner = if is_major { r - 10.0 } else { r - 5.0 };
        let outer = r + 2.0;
        let p1 = egui::pos2(cx + inner * a.cos(), cy + inner * a.sin());
        let p2 = egui::pos2(cx + outer * a.cos(), cy + outer * a.sin());
        let w = if is_major { 1.5 } else { 0.5 };
        p.line_segment([p1, p2], egui::Stroke::new(w, color_alpha(t.dim, ALPHA_LINE)));
    }

    // Cardinal labels
    let bias = wd.momentum; // use momentum as flow proxy
    for (label, angle_offset) in [("BUY", 0.0f32), ("SELL", std::f32::consts::PI)] {
        let a = angle_offset - std::f32::consts::FRAC_PI_2;
        let lx = cx + (r + 14.0) * a.cos();
        let ly = cy + (r + 14.0) * a.sin();
        p.text(egui::pos2(lx, ly), egui::Align2::CENTER_CENTER, label,
            egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
    }

    // Needle
    let needle_angle = (bias / 10.0).clamp(-1.0, 1.0) * std::f32::consts::FRAC_PI_2 - std::f32::consts::FRAC_PI_2;
    let needle_end = egui::pos2(cx + (r - 14.0) * needle_angle.cos(), cy + (r - 14.0) * needle_angle.sin());
    let needle_col = if bias > 0.0 { t.bull } else { t.bear };
    p.line_segment([egui::pos2(cx, cy), needle_end], egui::Stroke::new(2.5, needle_col));
    p.circle_filled(egui::pos2(cx, cy), 4.0, needle_col);
    p.circle_filled(needle_end, 3.0, needle_col);

    // Center label
    p.text(egui::pos2(cx, body.bottom() - 8.0), egui::Align2::CENTER_CENTER,
        if bias > 2.0 { "BULLISH FLOW" } else if bias < -2.0 { "BEARISH FLOW" } else { "NEUTRAL" },
        egui::FontId::monospace(7.0), needle_col);
}

/// Volatility Regime — concentric rings (like RSI Multi but for vol metrics)
fn draw_vol_regime(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y - 8.0;
    let max_r = (body.width().min(body.height()) * 0.36).min(70.0);

    let metrics = [
        ("BBW", wd.bb_width * 1000.0, 100.0),     // Bollinger bandwidth
        ("ATR%", wd.atr_pct, 5.0),                 // ATR as % of price
        ("RVOL", wd.vol_ratio * 50.0, 100.0),      // Relative volume
        ("ATRp", wd.atr_percentile, 100.0),         // ATR percentile
    ];

    for (i, (label, val, max)) in metrics.iter().enumerate() {
        let r = max_r - i as f32 * 14.0;
        let frac = (val / max).clamp(0.0, 1.0);
        let color = if frac > 0.7 { t.bear } else if frac > 0.4 { egui::Color32::from_rgb(255, 191, 0) } else { t.bull };

        draw_arc_ring(p, egui::pos2(cx, cy), r, 5.0, 0.0, std::f32::consts::TAU,
            color_alpha(t.toolbar_border, ALPHA_FAINT), 48);
        let sweep = frac * std::f32::consts::TAU;
        draw_arc_ring(p, egui::pos2(cx, cy), r, 5.0, -std::f32::consts::FRAC_PI_2, sweep, color, 40);

        // Label at right
        p.text(egui::pos2(body.right() - 6.0, cy - r), egui::Align2::RIGHT_CENTER,
            &format!("{} {:.0}", label, val), egui::FontId::monospace(6.0), color.gamma_multiply(0.7));
    }

    // Center: regime label
    let regime_col = match wd.vol_regime_label {
        "SQUEEZE" => t.bull, "EXPANSION" => t.bear, _ => t.dim
    };
    p.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER, wd.vol_regime_label,
        egui::FontId::proportional(14.0), regime_col);
}

/// Momentum Heatmap — color strip barcode (chart9 dot grid adapted)
fn draw_momentum_heat(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let labels = ["1D", "2D", "5D", "10D", "20D", "60D", "120D", "1Y"];
    let cols = 8;
    let col_w = body.width() / cols as f32;
    let bar_h = body.height() - 20.0;
    let max_abs = wd.roc_bars.iter().map(|v| v.abs()).fold(0.0f32, f32::max).max(0.01);

    for (i, &roc) in wd.roc_bars.iter().enumerate() {
        let x = body.left() + i as f32 * col_w;
        let intensity = (roc.abs() / max_abs).clamp(0.0, 1.0);
        let color = if roc > 0.0 { t.bull } else { t.bear };
        let alpha = (intensity * 200.0 + 30.0) as u8;

        // Color block
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(x + 1.0, body.top()), egui::vec2(col_w - 2.0, bar_h)),
            2.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha));

        // ROC value
        p.text(egui::pos2(x + col_w * 0.5, body.top() + bar_h * 0.5), egui::Align2::CENTER_CENTER,
            &format!("{:+.0}", roc), egui::FontId::monospace(7.0),
            if intensity > 0.5 { egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200) } else { t.text });

        // Label
        p.text(egui::pos2(x + col_w * 0.5, body.bottom() - 6.0), egui::Align2::CENTER_CENTER,
            labels[i], egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
    }
}

/// Breadth Thermometer — dot matrix grid (chart9 purple/green inspiration)
fn draw_breadth_thermo(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let score = wd.breadth_score;
    let total_dots = 100;
    let cols = 10;
    let rows = 10;
    let dot_r = 3.5;
    let filled = (score / 100.0 * total_dots as f32) as usize;

    let grid_w = body.width() - 50.0;
    let grid_h = body.height() - 20.0;
    let gap_x = grid_w / cols as f32;
    let gap_y = grid_h / rows as f32;
    let ox = body.left() + 6.0;
    let oy = body.top() + 4.0;

    let bull_col = t.bull;
    let empty_col = color_alpha(t.dim, ALPHA_MUTED);

    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            let cx = ox + col as f32 * gap_x + gap_x * 0.5;
            let cy = oy + row as f32 * gap_y + gap_y * 0.5;
            let color = if idx < filled { bull_col } else { empty_col };
            p.circle_filled(egui::pos2(cx, cy), dot_r, color);
        }
    }

    // Score on the right
    let sc = if score > 60.0 { t.bull } else if score < 40.0 { t.bear } else { egui::Color32::from_rgb(255, 191, 0) };
    p.text(egui::pos2(body.right() - 8.0, body.center().y - 8.0), egui::Align2::RIGHT_CENTER,
        &format!("{:.0}", score), egui::FontId::proportional(24.0), sc);
    p.text(egui::pos2(body.right() - 8.0, body.center().y + 12.0), egui::Align2::RIGHT_CENTER,
        if score > 60.0 { "HEALTHY" } else if score < 40.0 { "WEAK" } else { "MIXED" },
        egui::FontId::monospace(7.0), sc);
}

/// Sector Rotation — 2x2 quadrant radar
fn draw_sector_rotation(p: &egui::Painter, body: egui::Rect, _wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y - 4.0;
    let hw = body.width() * 0.38;
    let hh = body.height() * 0.36;

    // Quadrant lines
    p.line_segment([egui::pos2(cx - hw, cy), egui::pos2(cx + hw, cy)],
        egui::Stroke::new(0.5, color_alpha(t.dim, ALPHA_MUTED)));
    p.line_segment([egui::pos2(cx, cy - hh), egui::pos2(cx, cy + hh)],
        egui::Stroke::new(0.5, color_alpha(t.dim, ALPHA_MUTED)));

    // Quadrant labels
    p.text(egui::pos2(cx + hw * 0.5, cy - hh - 4.0), egui::Align2::CENTER_CENTER, "LEADING", egui::FontId::monospace(6.0), t.bull.gamma_multiply(0.6));
    p.text(egui::pos2(cx - hw * 0.5, cy - hh - 4.0), egui::Align2::CENTER_CENTER, "IMPROVING", egui::FontId::monospace(6.0), t.accent.gamma_multiply(0.6));
    p.text(egui::pos2(cx - hw * 0.5, cy + hh + 6.0), egui::Align2::CENTER_CENTER, "LAGGING", egui::FontId::monospace(6.0), t.bear.gamma_multiply(0.6));
    p.text(egui::pos2(cx + hw * 0.5, cy + hh + 6.0), egui::Align2::CENTER_CENTER, "WEAKENING", egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));

    // Sector dots (placeholder positions)
    let sectors = [("XLK", 0.6, 0.3), ("XLF", 0.3, -0.2), ("XLE", -0.4, 0.5),
                   ("XLV", -0.2, -0.3), ("XLI", 0.4, -0.1), ("XLU", -0.5, -0.4),
                   ("XLC", 0.1, 0.4), ("XLRE", -0.3, 0.1)];
    for (label, rs, mom) in sectors {
        let sx = cx + rs * hw;
        let sy = cy - mom * hh;
        let col = if rs > 0.0 && mom > 0.0 { t.bull }
            else if rs < 0.0 && mom < 0.0 { t.bear }
            else { t.dim };
        p.circle_filled(egui::pos2(sx, sy), 4.0, col);
        p.text(egui::pos2(sx, sy - 7.0), egui::Align2::CENTER_CENTER, label,
            egui::FontId::monospace(6.0), col.gamma_multiply(0.8));
    }
}

/// Options Sentiment — donut gauge composite
fn draw_options_sentiment(p: &egui::Painter, body: egui::Rect, _wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y - 8.0;
    let r = (body.width().min(body.height()) * 0.32).min(55.0);

    // Placeholder sentiment: 62% bullish
    let sentiment = 62.0f32;
    let color = if sentiment > 60.0 { t.bull } else if sentiment < 40.0 { t.bear } else { egui::Color32::from_rgb(255, 191, 0) };

    donut_ring(p, egui::pos2(cx, cy), r, 8.0, sentiment, 100.0, color,
        color_alpha(t.toolbar_border, ALPHA_MUTED));

    p.text(egui::pos2(cx, cy - 4.0), egui::Align2::CENTER_CENTER,
        &format!("{:.0}%", sentiment), egui::FontId::proportional(22.0), color);
    p.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
        "BULLISH", egui::FontId::monospace(7.0), color);

    // Metrics below
    let my = cy + r + 16.0;
    for (i, (label, val)) in [("P/C", "0.82"), ("Skew", "-1.2"), ("GEX", "+$1.2B")].iter().enumerate() {
        let x = body.left() + 10.0 + i as f32 * 55.0;
        p.text(egui::pos2(x, my), egui::Align2::LEFT_CENTER, label, egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.4));
        p.text(egui::pos2(x, my + 10.0), egui::Align2::LEFT_CENTER, val, egui::FontId::monospace(FONT_XS), t.text);
    }
}

/// Relative Strength Radar — concentric rings for RS rank
fn draw_rel_strength(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y - 6.0;
    let max_r = (body.width().min(body.height()) * 0.35).min(65.0);

    let metrics = [
        ("vs Market", wd.rs_rank),
        ("vs Sector", (wd.rs_rank * 0.9 + 5.0).clamp(0.0, 100.0)),
        ("vs Peers", (wd.rs_rank * 1.1 - 3.0).clamp(0.0, 100.0)),
    ];

    for (i, (label, val)) in metrics.iter().enumerate() {
        let r = max_r - i as f32 * 18.0;
        let color = if *val > 70.0 { t.bull } else if *val < 30.0 { t.bear } else { egui::Color32::from_rgb(255, 191, 0) };
        donut_ring(p, egui::pos2(cx, cy), r, 6.0, *val, 100.0, color,
            color_alpha(t.toolbar_border, ALPHA_FAINT));
        p.text(egui::pos2(body.right() - 6.0, cy - r), egui::Align2::RIGHT_CENTER,
            &format!("{} {:.0}", label, val), egui::FontId::monospace(6.0), color.gamma_multiply(0.7));
    }

    // Center
    let avg = metrics.iter().map(|(_, v)| v).sum::<f32>() / 3.0;
    let ac = if avg > 60.0 { t.bull } else if avg < 40.0 { t.bear } else { t.dim };
    p.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER,
        &format!("{:.0}", avg), egui::FontId::proportional(20.0), ac);
}

/// Risk Dashboard — position sizing calculator
fn draw_risk_dash(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let left = body.left() + 8.0;
    let mut y = body.top() + 4.0;
    let account = 100_000.0f32; // placeholder
    let risk_pct = 1.0; // 1% risk
    let dollar_risk = account * risk_pct / 100.0;
    let stop_dist = wd.atr; // use ATR as stop distance
    let shares = if stop_dist > 0.0 { (dollar_risk / stop_dist).floor() } else { 0.0 };
    let notional = shares * wd.last_close;

    // Hero: suggested shares
    p.text(egui::pos2(body.center().x, y + 16.0), egui::Align2::CENTER_CENTER,
        &format!("{:.0}", shares), egui::FontId::proportional(28.0), t.accent);
    p.text(egui::pos2(body.center().x, y + 34.0), egui::Align2::CENTER_CENTER,
        "SHARES", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
    y += 48.0;

    // Stats grid
    let stats = [
        ("Risk $", format!("${:.0}", dollar_risk)),
        ("Stop", format!("${:.2}", stop_dist)),
        ("Notional", format!("${:.0}", notional)),
        ("% Acct", format!("{:.1}%", notional / account * 100.0)),
    ];
    for (i, (label, val)) in stats.iter().enumerate() {
        let x = if i % 2 == 0 { left } else { body.center().x + 4.0 };
        let row_y = y + (i / 2) as f32 * 18.0;
        p.text(egui::pos2(x, row_y), egui::Align2::LEFT_CENTER, label, egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.4));
        p.text(egui::pos2(x + 50.0, row_y), egui::Align2::LEFT_CENTER, val, egui::FontId::monospace(FONT_XS), t.text);
    }
}

/// Earnings Momentum — mini 2x2 grid (style3 card grid inspiration)
fn draw_earnings_mom(p: &egui::Painter, body: egui::Rect, _wd: &WidgetData, t: &Theme) {
    let hw = body.width() * 0.5;
    let hh = body.height() * 0.5;
    let cells = [
        ("EPS", "+12%", t.bull),
        ("REV", "+8%", t.bull),
        ("REVISIONS", "\u{2191}3", egui::Color32::from_rgb(255, 191, 0)),
        ("FWD P/E", "22.4x", t.dim),
    ];

    for (i, (label, val, color)) in cells.iter().enumerate() {
        let col = i % 2;
        let row = i / 2;
        let x = body.left() + col as f32 * hw;
        let y = body.top() + row as f32 * hh;
        let cell = egui::Rect::from_min_size(egui::pos2(x + 2.0, y + 2.0), egui::vec2(hw - 4.0, hh - 4.0));

        p.rect_filled(cell, 3.0, color_alpha(*color, ALPHA_FAINT));
        p.text(egui::pos2(cell.left() + 6.0, cell.top() + 8.0), egui::Align2::LEFT_CENTER,
            label, egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
        p.text(egui::pos2(cell.center().x, cell.center().y + 4.0), egui::Align2::CENTER_CENTER,
            val, egui::FontId::proportional(18.0), *color);
    }
}

/// Liquidity Score — single donut gauge (chart3 pie style)
fn draw_liquidity_score(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y - 4.0;
    let r = (body.width().min(body.height()) * 0.34).min(50.0);
    let score = wd.liquidity_score;
    let color = if score > 70.0 { t.bull } else if score < 30.0 { t.bear } else { egui::Color32::from_rgb(255, 191, 0) };
    let label = if score > 70.0 { "LIQUID" } else if score < 30.0 { "ILLIQUID" } else { "MODERATE" };

    donut_ring(p, egui::pos2(cx, cy), r, 8.0, score, 100.0, color,
        color_alpha(t.toolbar_border, ALPHA_MUTED));

    p.text(egui::pos2(cx, cy - 4.0), egui::Align2::CENTER_CENTER,
        &format!("{:.0}", score), egui::FontId::proportional(24.0), color);
    p.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
        label, egui::FontId::monospace(7.0), color);
}

fn draw_arc_ring(p: &egui::Painter, center: egui::Pos2, radius: f32, width: f32,
                 start: f32, sweep: f32, color: egui::Color32, segments: usize) {
    if sweep.abs() < 0.001 { return; }
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        if t0 * sweep.abs() > sweep.abs() { break; }
        let a0 = start + t0 * sweep;
        let a1 = start + t1.min(1.0) * sweep;
        let p0 = egui::pos2(center.x + radius * a0.cos(), center.y + radius * a0.sin());
        let p1 = egui::pos2(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        p.line_segment([p0, p1], egui::Stroke::new(width, color));
    }
}

/// Signal Radar — radial map of all active ApexSignals (chart26 radial inspiration)
fn draw_signal_radar(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y - 6.0;
    let r = (body.width().min(body.height()) * 0.38).min(80.0);

    // Signal definitions: (name, active, intensity)
    let signals: Vec<(&str, bool, f32)> = vec![
        ("Trend", wd.trend_score > 0.0, wd.trend_score / 100.0),
        ("Exit", wd.exit_gauge_score > 0.0, wd.exit_gauge_score / 100.0),
        ("Precur", wd.precursor_active, wd.precursor_score / 100.0),
        ("Plan", wd.trade_plan.is_some(), 0.8),
        ("Zones", wd.zone_count > 0, wd.zone_avg_strength),
        ("Pattern", wd.pattern_count > 0, wd.pattern_latest_conf),
        ("ChgPt", wd.change_points_count > 0, 0.7),
        ("VIX", wd.vix_spot > 20.0, (wd.vix_spot / 40.0).min(1.0)),
        ("Diverg", wd.divergence_count > 0, 0.6),
        ("DarkPl", wd.dark_pool_ratio > 0.2, wd.dark_pool_ratio),
    ];
    let n = signals.len();
    let active_count = signals.iter().filter(|(_, a, _)| *a).count();

    // Concentric reference rings
    for ring in [0.33, 0.66, 1.0] {
        let rr = r * ring;
        let segs = 40;
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            p.line_segment([
                egui::pos2(cx + rr * a0.cos(), cy + rr * a0.sin()),
                egui::pos2(cx + rr * a1.cos(), cy + rr * a1.sin())],
                egui::Stroke::new(0.3, color_alpha(t.toolbar_border, ALPHA_FAINT)));
        }
    }

    // Signal spokes + dots
    for (i, (name, active, intensity)) in signals.iter().enumerate() {
        let angle = (i as f32 / n as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let spoke_r = r * if *active { intensity.max(0.3) } else { 0.15 };
        let end = egui::pos2(cx + spoke_r * angle.cos(), cy + spoke_r * angle.sin());

        // Spoke line
        let spoke_col = if *active { t.accent } else { color_alpha(t.dim, ALPHA_FAINT) };
        p.line_segment([egui::pos2(cx, cy), end], egui::Stroke::new(1.0, spoke_col));

        // Dot at tip
        let dot_r = if *active { 4.0 } else { 2.0 };
        let dot_col = if *active { t.accent } else { color_alpha(t.dim, ALPHA_MUTED) };
        p.circle_filled(end, dot_r, dot_col);

        // Label
        let label_r = r + 10.0;
        let lx = cx + label_r * angle.cos();
        let ly = cy + label_r * angle.sin();
        p.text(egui::pos2(lx, ly), egui::Align2::CENTER_CENTER, name,
            egui::FontId::monospace(5.5), if *active { t.accent.gamma_multiply(0.8) } else { t.dim.gamma_multiply(0.3) });
    }

    // Center: active count
    p.text(egui::pos2(cx, cy - 4.0), egui::Align2::CENTER_CENTER,
        &format!("{}", active_count), egui::FontId::proportional(20.0), t.accent);
    p.text(egui::pos2(cx, cy + 10.0), egui::Align2::CENTER_CENTER,
        "ACTIVE", egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
}

/// Cross-Asset Pulse — compact 2x4 market grid (style3 card grid)
fn draw_cross_asset(p: &egui::Painter, body: egui::Rect, _wd: &WidgetData, t: &Theme) {
    let assets = [
        ("SPY", "+0.42%", true),  ("QQQ", "+0.68%", true),
        ("DXY", "-0.15%", false), ("VIX", "+2.3%", true),
        ("TNX", "-0.08%", false), ("GLD", "+0.31%", true),
        ("CL", "-1.2%", false),   ("BTC", "+1.8%", true),
    ];
    let cols = 4;
    let rows = 2;
    let cell_w = body.width() / cols as f32;
    let cell_h = body.height() / rows as f32;

    for (i, (sym, chg, positive)) in assets.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let x = body.left() + col as f32 * cell_w;
        let y = body.top() + row as f32 * cell_h;
        let cell = egui::Rect::from_min_size(egui::pos2(x + 1.0, y + 1.0),
            egui::vec2(cell_w - 2.0, cell_h - 2.0));

        let col_c = if *positive { t.bull } else { t.bear };
        p.rect_filled(cell, 3.0, color_alpha(col_c, ALPHA_FAINT));

        // Symbol
        p.text(egui::pos2(cell.left() + 4.0, cell.top() + 8.0), egui::Align2::LEFT_CENTER,
            sym, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
        // Change
        p.text(egui::pos2(cell.center().x, cell.center().y + 4.0), egui::Align2::CENTER_CENTER,
            chg, egui::FontId::proportional(13.0), col_c);
    }
}

/// Tape Speed — speedometer gauge showing trade velocity
fn draw_tape_speed(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y + 4.0;
    let r = (body.width().min(body.height()) * 0.34).min(50.0);
    let speed = wd.vol_ratio; // 1.0 = average, 2.0 = 2x average

    // Semi-circle track
    let pi = std::f32::consts::PI;
    let segs = 30;
    for i in 0..segs {
        let t0 = i as f32 / segs as f32;
        let t1 = (i + 1) as f32 / segs as f32;
        let a0 = pi + t0 * pi;
        let a1 = pi + t1 * pi;
        // Color gradient: blue → green → yellow → red
        let col = if t0 < 0.3 { t.accent }
            else if t0 < 0.6 { t.bull }
            else if t0 < 0.8 { egui::Color32::from_rgb(255, 191, 0) }
            else { t.bear };
        p.line_segment([
            egui::pos2(cx + r * a0.cos(), cy + r * a0.sin()),
            egui::pos2(cx + r * a1.cos(), cy + r * a1.sin())],
            egui::Stroke::new(5.0, col));
    }

    // Needle
    let needle_frac = (speed / 4.0).clamp(0.0, 1.0); // 4x = max
    let needle_a = pi + needle_frac * pi;
    let needle_end = egui::pos2(cx + (r - 10.0) * needle_a.cos(), cy + (r - 10.0) * needle_a.sin());
    let needle_col = if speed > 2.5 { t.bear } else if speed > 1.5 { egui::Color32::from_rgb(255, 191, 0) } else { t.bull };
    p.line_segment([egui::pos2(cx, cy), needle_end], egui::Stroke::new(2.0, needle_col));
    p.circle_filled(egui::pos2(cx, cy), 4.0, needle_col);

    // Speed value
    p.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
        &format!("{:.1}x", speed), egui::FontId::proportional(18.0), needle_col);

    // Labels
    p.text(egui::pos2(cx - r + 4.0, cy + 4.0), egui::Align2::LEFT_CENTER,
        "0", egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(cx + r - 4.0, cy + 4.0), egui::Align2::RIGHT_CENTER,
        "4x", egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(cx, cy - r - 4.0), egui::Align2::CENTER_CENTER,
        "TAPE", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
}

/// Fundamentals Card — key metrics in a compact grid (style2/style3 color-block inspiration)
fn draw_fundamentals(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cols = 2;
    let metrics: Vec<(&str, String, Color32)> = vec![
        ("P/E", format!("{:.1}", wd.pe_ratio), t.accent),
        ("EPS", format!("${:.2}", wd.eps_ttm), t.text),
        ("MKT CAP", format!("${:.0}B", wd.market_cap_b), t.text),
        ("DIV YIELD", format!("{:.1}%", wd.dividend_yield), if wd.dividend_yield > 2.0 { t.bull } else { t.dim }),
        ("REV GROWTH", format!("{:+.1}%", wd.revenue_growth), if wd.revenue_growth > 0.0 { t.bull } else { t.bear }),
        ("MARGIN", format!("{:.1}%", wd.profit_margin), if wd.profit_margin > 15.0 { t.bull } else { t.dim }),
        ("SHORT INT", format!("{:.1}%", wd.short_interest), if wd.short_interest > 5.0 { t.bear } else { t.dim }),
        ("INST OWN", format!("{:.0}%", wd.institutional_pct), t.dim),
    ];

    let cell_w = body.width() / cols as f32;
    let cell_h = body.height() / (metrics.len() / cols) as f32;

    for (i, (label, value, color)) in metrics.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let x = body.left() + col as f32 * cell_w;
        let y = body.top() + row as f32 * cell_h;

        // Label
        p.text(egui::pos2(x + 6.0, y + 6.0), egui::Align2::LEFT_CENTER,
            label, egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.4));
        // Value
        p.text(egui::pos2(x + 6.0, y + cell_h * 0.6), egui::Align2::LEFT_CENTER,
            value, egui::FontId::proportional(14.0), *color);
    }

    // Analyst consensus bar at bottom
    let total = (wd.analyst_buy + wd.analyst_hold + wd.analyst_sell) as f32;
    if total > 0.0 {
        let bar_y = body.bottom() - 12.0;
        let bar_w = body.width() - 12.0;
        let buy_w = bar_w * wd.analyst_buy as f32 / total;
        let hold_w = bar_w * wd.analyst_hold as f32 / total;
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(body.left() + 6.0, bar_y), egui::vec2(buy_w, 6.0)),
            2.0, t.bull);
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(body.left() + 6.0 + buy_w, bar_y), egui::vec2(hold_w, 6.0)),
            0.0, egui::Color32::from_rgb(255, 191, 0));
        let sell_w = bar_w - buy_w - hold_w;
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(body.left() + 6.0 + buy_w + hold_w, bar_y), egui::vec2(sell_w, 6.0)),
            2.0, t.bear);
        p.text(egui::pos2(body.left() + 6.0, bar_y - 4.0), egui::Align2::LEFT_BOTTOM,
            &format!("{}B {}H {}S  PT ${:.0}", wd.analyst_buy, wd.analyst_hold, wd.analyst_sell, wd.analyst_target),
            egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.4));
    }
}

/// Economic Calendar — upcoming events countdown (chart8 lollipop + color block style)
fn draw_econ_calendar(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if wd.econ_count == 0 {
        p.text(body.center(), egui::Align2::CENTER_CENTER, "NO EVENTS",
            egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        return;
    }

    // Next event hero
    if wd.econ_next_days >= 0 {
        p.text(egui::pos2(body.left() + 8.0, body.top() + 6.0), egui::Align2::LEFT_CENTER,
            "NEXT EVENT", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
        p.text(egui::pos2(body.left() + 8.0, body.top() + 26.0), egui::Align2::LEFT_CENTER,
            &format!("{}d", wd.econ_next_days), egui::FontId::proportional(28.0), t.accent);
        p.text(egui::pos2(body.left() + 55.0, body.top() + 20.0), egui::Align2::LEFT_CENTER,
            &wd.econ_next_name, egui::FontId::monospace(FONT_SM), t.text);
    }

    // Event list below
    let list_top = body.top() + 48.0;
    let row_h = 16.0;
    let events_placeholder = [
        ("FOMC", 2, 3u8), ("CPI", 5, 2), ("NFP", 8, 3),
        ("PPI", 12, 1), ("Retail", 15, 2), ("GDP", 20, 3),
    ];
    for (i, (name, days, importance)) in events_placeholder.iter().enumerate() {
        let y = list_top + i as f32 * row_h;
        if y + row_h > body.bottom() { break; }

        let imp_color = match importance {
            3 => t.bear, 2 => egui::Color32::from_rgb(255, 191, 0), _ => t.dim
        };
        // Importance dot
        p.circle_filled(egui::pos2(body.left() + 10.0, y + row_h * 0.5), 2.5, imp_color);
        // Name
        p.text(egui::pos2(body.left() + 18.0, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            name, egui::FontId::monospace(FONT_XS), t.text);
        // Days
        p.text(egui::pos2(body.right() - 8.0, y + row_h * 0.5), egui::Align2::RIGHT_CENTER,
            &format!("{}d", days), egui::FontId::monospace(FONT_XS), t.dim);
    }
}

/// Latency widget — frame time + data feed status
fn draw_latency(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let cx = body.center().x;

    // Frame time (approximate from 60fps target)
    let frame_ms = 16.7f32; // placeholder — 60fps
    let frame_col = if frame_ms < 8.0 { t.bull } else if frame_ms < 20.0 { egui::Color32::from_rgb(255, 191, 0) } else { t.bear };

    p.text(egui::pos2(body.left() + 8.0, body.top() + 6.0), egui::Align2::LEFT_CENTER,
        "RENDER", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(body.left() + 8.0, body.top() + 22.0), egui::Align2::LEFT_CENTER,
        &format!("{:.1}ms", frame_ms), egui::FontId::proportional(18.0), frame_col);
    p.text(egui::pos2(body.left() + 75.0, body.top() + 22.0), egui::Align2::LEFT_CENTER,
        "60fps", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));

    // Data feed latency
    let data_ms = 45.0f32; // placeholder
    let data_col = if data_ms < 50.0 { t.bull } else if data_ms < 200.0 { egui::Color32::from_rgb(255, 191, 0) } else { t.bear };

    p.text(egui::pos2(body.left() + 8.0, body.top() + 40.0), egui::Align2::LEFT_CENTER,
        "DATA FEED", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(body.left() + 8.0, body.top() + 56.0), egui::Align2::LEFT_CENTER,
        &format!("{:.0}ms", data_ms), egui::FontId::proportional(16.0), data_col);

    // Service dots
    let dot_y = body.top() + 74.0;
    let services = [("GPU", true), ("IB", false), ("Redis", false), ("Yahoo", true)];
    let mut dx = body.left() + 8.0;
    for (name, ok) in services {
        let col = if ok { t.bull } else { t.dim.gamma_multiply(0.3) };
        p.circle_filled(egui::pos2(dx + 3.0, dot_y), 2.5, col);
        p.text(egui::pos2(dx + 9.0, dot_y), egui::Align2::LEFT_CENTER,
            name, egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
        dx += name.len() as f32 * 5.0 + 16.0;
    }
}

/// Options Payoff Chart — P&L curve for a position
fn draw_payoff_chart(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let left = body.left() + 6.0;
    let right = body.right() - 6.0;
    let chart_top = body.top() + 18.0;
    let chart_bot = body.bottom() - 14.0;
    let chart_w = right - left;
    let chart_h = chart_bot - chart_top;

    p.text(egui::pos2(left, body.top() + 6.0), egui::Align2::LEFT_CENTER,
        "PAYOFF CURVE", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));

    // Placeholder: long call payoff curve
    let strike = wd.last_close;
    let premium = strike * 0.03; // 3% premium
    let price_low = strike * 0.90;
    let price_high = strike * 1.15;
    let range = price_high - price_low;

    // Zero line
    let zero_y = chart_top + chart_h * 0.6; // put zero at 60% down
    p.line_segment([egui::pos2(left, zero_y), egui::pos2(right, zero_y)],
        egui::Stroke::new(0.5, color_alpha(t.dim, ALPHA_MUTED)));
    p.text(egui::pos2(left - 2.0, zero_y), egui::Align2::RIGHT_CENTER,
        "0", egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.3));

    // Draw payoff curve
    let max_profit = strike * 0.10; // scale
    let points = 50;
    let mut prev_pos: Option<egui::Pos2> = None;
    for i in 0..=points {
        let frac = i as f32 / points as f32;
        let price = price_low + frac * range;
        let pnl = if price > strike { price - strike - premium } else { -premium };
        let px = left + frac * chart_w;
        let py = zero_y - (pnl / max_profit) * (chart_h * 0.35);
        let py = py.clamp(chart_top, chart_bot);
        let pos = egui::pos2(px, py);
        if let Some(prev) = prev_pos {
            let col = if pnl > 0.0 { t.bull } else { t.bear };
            p.line_segment([prev, pos], egui::Stroke::new(1.5, col));
        }
        prev_pos = Some(pos);
    }

    // Strike line
    let strike_x = left + ((strike - price_low) / range) * chart_w;
    p.line_segment([egui::pos2(strike_x, chart_top), egui::pos2(strike_x, chart_bot)],
        egui::Stroke::new(0.5, color_alpha(t.accent, 60)));
    p.text(egui::pos2(strike_x, chart_bot + 6.0), egui::Align2::CENTER_CENTER,
        &format!("${:.0}", strike), egui::FontId::monospace(6.0), t.accent.gamma_multiply(0.6));

    // Max loss label
    p.text(egui::pos2(left + 4.0, zero_y + 10.0), egui::Align2::LEFT_CENTER,
        &format!("Max Loss: -${:.0}", premium), egui::FontId::monospace(7.0), t.bear);
    // Breakeven
    let be = strike + premium;
    p.text(egui::pos2(right - 4.0, zero_y - 4.0), egui::Align2::RIGHT_BOTTOM,
        &format!("BE ${:.0}", be), egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
}

/// Options Flow — unusual activity feed
fn draw_options_flow(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    p.text(egui::pos2(body.left() + 6.0, body.top() + 6.0), egui::Align2::LEFT_CENTER,
        "UNUSUAL FLOW", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));

    let flows = [
        ("CALL", "450C 0DTE", "$2.4M", true, "sweep"),
        ("PUT",  "440P 1DTE", "$1.8M", false, "block"),
        ("CALL", "460C 5DTE", "$3.1M", true, "sweep"),
        ("CALL", "455C 0DTE", "$890K", true, "multi"),
        ("PUT",  "435P 3DTE", "$1.2M", false, "block"),
        ("CALL", "470C 10DTE", "$2.7M", true, "sweep"),
    ];

    let row_h = (body.height() - 20.0) / flows.len().min(6) as f32;
    let mut y = body.top() + 18.0;

    for (side, contract, value, bullish, flow_type) in flows {
        if y + row_h > body.bottom() { break; }
        let col = if bullish { t.bull } else { t.bear };

        // Side pill
        let pill_w = 28.0;
        let pill_rect = egui::Rect::from_min_size(egui::pos2(body.left() + 6.0, y + 2.0), egui::vec2(pill_w, row_h - 4.0));
        p.rect_filled(pill_rect, 2.0, color_alpha(col, ALPHA_TINT));
        p.text(pill_rect.center(), egui::Align2::CENTER_CENTER,
            side, egui::FontId::monospace(6.0), col);

        // Contract
        p.text(egui::pos2(body.left() + 38.0, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            contract, egui::FontId::monospace(FONT_XS), t.text);

        // Value
        p.text(egui::pos2(body.right() - 30.0, y + row_h * 0.5), egui::Align2::RIGHT_CENTER,
            value, egui::FontId::monospace(FONT_XS), col);

        // Flow type
        p.text(egui::pos2(body.right() - 6.0, y + row_h * 0.5), egui::Align2::RIGHT_CENTER,
            flow_type, egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.4));

        y += row_h;
    }
}

fn draw_positions_panel(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme,
                       hover: Option<egui::Pos2>, btns: &mut Vec<(egui::Rect, WidgetBtnAction)>) {
    let left = body.left() + 6.0;
    let right = body.right() - 6.0;
    let mut y = body.top() + 2.0;

    if wd.all_positions.is_empty() {
        // No positions
        p.text(egui::pos2(body.center().x, body.center().y - 10.0),
            egui::Align2::CENTER_CENTER, "NO POSITIONS",
            egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        p.text(egui::pos2(body.center().x, body.center().y + 8.0),
            egui::Align2::CENTER_CENTER, "Account is flat",
            egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.3));
        return;
    }

    // ── Day total P&L header ──
    let total_col = if wd.day_pnl >= 0.0 { t.bull } else { t.bear };
    p.text(egui::pos2(left, y + 5.0), egui::Align2::LEFT_CENTER,
        "DAY P&L", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
    let pnl_sign = if wd.day_pnl >= 0.0 { "+" } else { "" };
    p.text(egui::pos2(right, y + 5.0), egui::Align2::RIGHT_CENTER,
        &format!("{}${:.0}", pnl_sign, wd.day_pnl),
        egui::FontId::monospace(FONT_MD), total_col);
    y += 16.0;

    // Separator
    p.line_segment(
        [egui::pos2(left, y), egui::pos2(right, y)],
        egui::Stroke::new(0.5, color_alpha(t.toolbar_border, ALPHA_MUTED)));
    y += 4.0;

    // ── "Close All" button ──
    let btn_w = 50.0;
    let btn_rect = egui::Rect::from_min_size(
        egui::pos2(right - btn_w, y), egui::vec2(btn_w, 14.0));
    let btn_hov = hover.map(|p| btn_rect.contains(p)).unwrap_or(false);
    p.rect_filled(btn_rect, 3.0, color_alpha(t.bear, if btn_hov { 80 } else { 40 }));
    p.rect_stroke(btn_rect, 3.0, egui::Stroke::new(if btn_hov { 1.0 } else { 0.5 }, t.bear.gamma_multiply(if btn_hov { 0.9 } else { 0.5 })),
        egui::StrokeKind::Outside);
    p.text(btn_rect.center(), egui::Align2::CENTER_CENTER,
        "Close All", egui::FontId::monospace(7.0), if btn_hov { egui::Color32::WHITE } else { t.bear });
    btns.push((btn_rect, WidgetBtnAction::CloseAllPositions));

    p.text(egui::pos2(left, y + 7.0), egui::Align2::LEFT_CENTER,
        &format!("{} positions", wd.all_positions.len()),
        egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    y += 20.0;

    // ── Column headers ──
    p.text(egui::pos2(left, y + 4.0), egui::Align2::LEFT_CENTER,
        "SYMBOL", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.35));
    p.text(egui::pos2(left + 70.0, y + 4.0), egui::Align2::LEFT_CENTER,
        "QTY", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.35));
    p.text(egui::pos2(right - 40.0, y + 4.0), egui::Align2::RIGHT_CENTER,
        "P&L", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.35));
    p.text(egui::pos2(right, y + 4.0), egui::Align2::RIGHT_CENTER,
        "", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.35));
    y += 12.0;

    // ── Position rows ──
    let row_h = 20.0;
    for (pos_idx, pos) in wd.all_positions.iter().enumerate() {
        if y + row_h > body.bottom() - 2.0 { break; } // clip to body

        let pnl_col = if pos.unrealized_pnl >= 0.0 { t.bull } else { t.bear };
        let dir_col = if pos.qty > 0 { t.bull } else { t.bear };

        // Symbol
        p.text(egui::pos2(left, y + row_h * 0.35), egui::Align2::LEFT_CENTER,
            &pos.symbol, egui::FontId::monospace(FONT_SM), t.text);
        // Market value below symbol
        let mv_str = format!("${:.0}", pos.market_value);
        p.text(egui::pos2(left, y + row_h * 0.75), egui::Align2::LEFT_CENTER,
            &mv_str, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.35));

        // Qty with direction color
        let qty_label = format!("{}{}", if pos.qty > 0 { "+" } else { "" }, pos.qty);
        p.text(egui::pos2(left + 70.0, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &qty_label, egui::FontId::monospace(FONT_XS), dir_col);

        // P&L value + pct
        let pnl_str = format!("{:+.0}", pos.unrealized_pnl);
        p.text(egui::pos2(right - 40.0, y + row_h * 0.35), egui::Align2::RIGHT_CENTER,
            &pnl_str, egui::FontId::monospace(FONT_SM), pnl_col);
        let pct_str = format!("{:+.1}%", pos.pnl_pct);
        p.text(egui::pos2(right - 40.0, y + row_h * 0.75), egui::Align2::RIGHT_CENTER,
            &pct_str, egui::FontId::monospace(7.0), pnl_col);

        // Close button — interactive with hover
        let close_rect = egui::Rect::from_center_size(
            egui::pos2(right - 6.0, y + row_h * 0.5), egui::vec2(14.0, 14.0));
        let close_hov = hover.map(|p| close_rect.contains(p)).unwrap_or(false);
        if close_hov {
            p.rect_filled(close_rect, 2.0, color_alpha(t.bear, 60));
        }
        p.text(close_rect.center(), egui::Align2::CENTER_CENTER,
            "\u{00D7}", egui::FontId::proportional(FONT_SM),
            if close_hov { t.bear } else { t.dim.gamma_multiply(0.35) });
        btns.push((close_rect, WidgetBtnAction::ClosePosition(pos_idx)));

        // Subtle bottom border
        p.line_segment(
            [egui::pos2(left, y + row_h - 0.5), egui::pos2(right, y + row_h - 0.5)],
            egui::Stroke::new(0.3, color_alpha(t.toolbar_border, 20)));

        y += row_h;
    }
}

fn draw_daily_pnl(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme,
                  hover: Option<egui::Pos2>, btns: &mut Vec<(egui::Rect, WidgetBtnAction)>) {
    let pnl = wd.day_pnl;
    let col = if pnl >= 0.0 { t.bull } else { t.bear };
    let sign = if pnl >= 0.0 { "+" } else { "" };
    let label = format!("{}${:.0}", sign, pnl);

    // Hero number — proportional display font, vertically centered
    let text_y = body.center().y;
    let text_x = body.left() + 10.0;
    p.text(egui::pos2(text_x, text_y), egui::Align2::LEFT_CENTER,
        &label, egui::FontId::proportional(56.0), col);

    // "Close All" button — right side, vertically centered, interactive
    let btn_w = 60.0;
    let btn_h = 22.0;
    let btn_rect = egui::Rect::from_center_size(
        egui::pos2(body.right() - btn_w * 0.5 - 8.0, body.center().y),
        egui::vec2(btn_w, btn_h));
    let btn_hovered = hover.map(|p| btn_rect.contains(p)).unwrap_or(false);
    let btn_bg = if btn_hovered { color_alpha(t.bear, 100) } else { color_alpha(t.bear, 50) };
    let btn_border = if btn_hovered { t.bear } else { t.bear.gamma_multiply(0.6) };
    p.rect_filled(btn_rect, 4.0, btn_bg);
    p.rect_stroke(btn_rect, 4.0, egui::Stroke::new(if btn_hovered { 1.0 } else { 0.5 }, btn_border),
        egui::StrokeKind::Outside);
    p.text(btn_rect.center(), egui::Align2::CENTER_CENTER,
        "Close All", egui::FontId::monospace(FONT_SM), if btn_hovered { egui::Color32::WHITE } else { t.bear });
    btns.push((btn_rect, WidgetBtnAction::CloseAllPositions));

    // Subtle "DAY P&L" label top-left
    p.text(egui::pos2(body.left() + 10.0, body.top() + 6.0), egui::Align2::LEFT_CENTER,
        "DAY P&L", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
}

fn draw_custom(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y;
    p.text(egui::pos2(cx, cy - 6.0), egui::Align2::CENTER_CENTER,
        "\u{2699}", egui::FontId::proportional(20.0), t.dim.gamma_multiply(0.2));
    p.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
        "Drag to configure", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.3));
}

// ═══════════════════════════════════════════════════════════════════════════════
// New widget renderers
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_correlation(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let corr = wd.correlation_spy;

    // Color: green for positive, red for negative, amber near zero
    let color = if corr > 0.5 { t.bull }
        else if corr > 0.0 { lerp_color(Color32::from_rgb(255, 191, 0), t.bull, corr * 2.0) }
        else if corr > -0.5 { lerp_color(t.bear, Color32::from_rgb(255, 191, 0), (corr + 0.5) * 2.0) }
        else { t.bear };

    // Arc gauge from -1 to +1 (180° sweep)
    let gauge_cy = body.top() + 38.0;
    let r = 28.0;

    // Background track
    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI,
        Stroke::new(3.0, color_alpha(t.toolbar_border, ALPHA_MUTED)), 40);

    // Colored zones: red left, green right
    draw_arc(p, egui::pos2(cx, gauge_cy), r, PI * 0.5, PI,
        Stroke::new(2.5, color_alpha(t.bear, ALPHA_FAINT)), 15);
    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI * 0.5,
        Stroke::new(2.5, color_alpha(t.bull, ALPHA_FAINT)), 15);

    // Needle: corr maps -1..+1 to PI..0
    let needle_a = PI * 0.5 * (1.0 - corr); // 0 at right, PI at left
    let ne = egui::pos2(cx + (r - 8.0) * needle_a.cos(), gauge_cy - (r - 8.0) * needle_a.sin());
    p.line_segment([egui::pos2(cx, gauge_cy), ne], Stroke::new(1.5, Color32::WHITE));
    p.circle_filled(egui::pos2(cx, gauge_cy), 3.0, Color32::WHITE);

    // Hero correlation value
    let sign = if corr >= 0.0 { "+" } else { "" };
    hero_number(p, egui::pos2(cx, gauge_cy + 14.0), &format!("{}{:.2}", sign, corr), color);

    // Label
    let label = if corr > 0.7 { "STRONG +" } else if corr > 0.3 { "MODERATE +" }
        else if corr > -0.3 { "DECOUPLED" } else if corr > -0.7 { "MODERATE \u{2212}" }
        else { "INVERSE" };
    sub_label(p, egui::pos2(cx, gauge_cy + 32.0), label, color);

    // vs SPY label
    p.text(egui::pos2(cx, body.bottom() - 6.0), egui::Align2::CENTER_CENTER,
        "vs SPY", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.3));
}

fn draw_dark_pool(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;

    // Dark pool ratio — hero number
    let ratio_pct = wd.dark_pool_ratio * 100.0;
    let ratio_col = if ratio_pct > 40.0 { Color32::from_rgb(180, 100, 255) } // purple = heavy dark pool
        else if ratio_pct > 20.0 { t.accent }
        else { t.dim };
    hero_number(p, egui::pos2(body.center().x, body.top() + 18.0),
        &format!("{:.0}%", ratio_pct), ratio_col);
    sub_label(p, egui::pos2(body.center().x, body.top() + 36.0), "DARK POOL", t.dim);

    // Volume spike bars
    let bar_y = body.top() + 50.0;
    let bar_w = (right - left) / 8.0 - 2.0;
    let bar_max_h = body.bottom() - bar_y - 16.0;

    for i in 0..8 {
        let x = left + i as f32 * (bar_w + 2.0);
        let h = bar_max_h * wd.dark_pool_bars[i].max(0.02);

        // Gradient: low=dim, high=purple
        let intensity = wd.dark_pool_bars[i];
        let color = lerp_color(
            color_alpha(t.dim, ALPHA_MUTED),
            Color32::from_rgb(160, 80, 220), // purple
            intensity);
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(x, bar_y + bar_max_h - h), egui::vec2(bar_w, h));
        p.rect_filled(bar_rect, 2.0, color);

        // Glow on largest bar
        if intensity > 0.9 {
            p.rect_filled(bar_rect.expand(1.0), 3.0,
                Color32::from_rgba_unmultiplied(160, 80, 220, 20));
        }
    }

    // "Unusual" label if high ratio
    if ratio_pct > 30.0 {
        p.text(egui::pos2(right, body.bottom() - 6.0), egui::Align2::RIGHT_CENTER,
            "UNUSUAL", egui::FontId::monospace(7.0), Color32::from_rgb(180, 100, 255));
    }
}

fn draw_position_pnl(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;

    if wd.position_qty == 0 {
        // No position
        p.text(egui::pos2(cx, body.center().y - 4.0), egui::Align2::CENTER_CENTER,
            "NO POSITION", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        p.text(egui::pos2(cx, body.center().y + 12.0), egui::Align2::CENTER_CENTER,
            &wd.symbol, egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.3));
        return;
    }

    let pnl_col = if wd.position_pnl >= 0.0 { t.bull } else { t.bear };
    let dir = if wd.position_qty > 0 { "LONG" } else { "SHORT" };
    let dir_col = if wd.position_qty > 0 { t.bull } else { t.bear };

    // Direction + qty pill
    let pill_text = format!("{} {}x", dir, wd.position_qty.abs());
    p.text(egui::pos2(cx, body.top() + 8.0), egui::Align2::CENTER_CENTER,
        &pill_text, egui::FontId::monospace(FONT_XS), dir_col);

    // Hero P&L
    let pnl_sign = if wd.position_pnl >= 0.0 { "+" } else { "" };
    hero_number(p, egui::pos2(cx, body.top() + 30.0),
        &format!("{}${:.0}", pnl_sign, wd.position_pnl), pnl_col);

    // P&L percentage
    p.text(egui::pos2(cx, body.top() + 48.0), egui::Align2::CENTER_CENTER,
        &format!("{:+.2}%", wd.position_pnl_pct), egui::FontId::monospace(FONT_SM), pnl_col);

    // Entry line indicator
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let entry_y = body.bottom() - 10.0;
    p.text(egui::pos2(left, entry_y), egui::Align2::LEFT_CENTER,
        "ENTRY", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(right, entry_y), egui::Align2::RIGHT_CENTER,
        &format!("${:.2}", wd.position_avg), egui::FontId::monospace(FONT_SM), t.text);
}

fn draw_earnings_badge(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;

    if wd.earnings_days < 0 {
        p.text(egui::pos2(cx, body.center().y), egui::Align2::CENTER_CENTER,
            "NO EARNINGS DATA", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.4));
        return;
    }

    // Urgency color
    let urgency_col = if wd.earnings_days <= 1 { t.bear }
        else if wd.earnings_days <= 5 { Color32::from_rgb(255, 191, 0) }
        else if wd.earnings_days <= 14 { t.accent }
        else { t.dim };

    // Countdown chip
    let days_str = if wd.earnings_days == 0 { "TODAY".into() }
        else if wd.earnings_days == 1 { "TOMORROW".into() }
        else { format!("{} DAYS", wd.earnings_days) };

    hero_number(p, egui::pos2(cx, body.top() + 16.0), &days_str, urgency_col);

    // Label
    let detail = if wd.earnings_label.is_empty() { "EARNINGS".into() }
        else { wd.earnings_label.clone() };
    sub_label(p, egui::pos2(cx, body.top() + 34.0), &detail, urgency_col);

    // Expected move bar (approximation: ATR * 2 as implied move)
    if wd.atr > 0.0 && wd.last_close > 0.0 {
        let implied_move_pct = wd.atr_pct * 2.0;
        let bar_y = body.bottom() - 12.0;
        let bar_x = body.left() + 12.0;
        let bar_w = body.width() - 24.0;
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, 4.0)),
            2.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
        // Show expected range centered
        let range_w = (bar_w * (implied_move_pct / 10.0).clamp(0.0, 1.0)).max(8.0);
        let range_x = bar_x + (bar_w - range_w) * 0.5;
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(range_x, bar_y), egui::vec2(range_w, 4.0)),
            2.0, urgency_col);
        p.text(egui::pos2(cx, bar_y - 4.0), egui::Align2::CENTER_BOTTOM,
            &format!("\u{00B1}{:.1}%", implied_move_pct), egui::FontId::monospace(7.0), urgency_col);
    }
}

fn draw_news_ticker(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let left = body.left() + 8.0;
    let right = body.right() - 8.0;
    let cy = body.center().y;

    // Demo headlines — in production these would come from the news feed
    let headlines: [(&str, Color32); 3] = [
        ("Fed holds rates steady, signals patience", Color32::from_rgb(255, 191, 0)),
        (&format!("{} beats Q3 estimates, guides higher", wd.symbol), t.bull),
        ("10Y yield rises to 4.5%, markets cautious", t.bear),
    ];

    // Use a time-based index to simulate scrolling
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let idx = (now / 5) as usize % headlines.len(); // rotate every 5 seconds
    let (headline, sentiment_col) = headlines[idx];

    // Sentiment dot
    p.circle_filled(egui::pos2(left + 3.0, cy), 3.0, sentiment_col);

    // Headline text (truncated to fit)
    p.text(egui::pos2(left + 12.0, cy), egui::Align2::LEFT_CENTER,
        headline, egui::FontId::monospace(FONT_SM), t.text);

    // Timestamp
    p.text(egui::pos2(right, cy), egui::Align2::RIGHT_CENTER,
        "just now", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Resize handle — drawn on Card mode widgets, bottom-right corner
// ═══════════════════════════════════════════════════════════════════════════════

/// Draw a loading skeleton shimmer when no bar data is loaded yet.
fn draw_loading_skeleton(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y;
    // Pulsing dots
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_millis() as f32;
    for i in 0..3 {
        let phase = (now / 400.0 + i as f32 * 0.8).sin() * 0.5 + 0.5;
        let alpha = (phase * 80.0) as u8;
        let r = 2.0 + phase * 1.5;
        p.circle_filled(egui::pos2(cx - 10.0 + i as f32 * 10.0, cy),
            r, Color32::from_rgba_unmultiplied(t.dim.r(), t.dim.g(), t.dim.b(), alpha));
    }
    p.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
        "Loading\u{2026}", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.3));
}

/// Compute aggregate conviction from all signal sources (0-100).
fn compute_conviction(wd: &WidgetData) -> f32 {
    let mut score = 0.0f32;
    let mut weight = 0.0f32;

    // Trend health contributes heavily
    if wd.trend_score > 0.0 { score += wd.trend_score * 2.0; weight += 2.0; }

    // RSI extremes add conviction for reversals
    let rsi_signal = if wd.rsi > 70.0 || wd.rsi < 30.0 { 80.0 } else { 40.0 };
    score += rsi_signal; weight += 1.0;

    // Precursor adds conviction
    if wd.precursor_active { score += wd.precursor_score * 1.5; weight += 1.5; }

    // Zone strength
    if wd.zone_count > 0 { score += wd.zone_avg_strength * 10.0; weight += 1.0; }

    // Pattern confidence
    if wd.pattern_count > 0 { score += wd.pattern_latest_conf * 100.0; weight += 1.0; }

    // Exit gauge inversely correlates (high exit = low conviction to hold)
    if wd.exit_gauge_score > 0.0 { score += 100.0 - wd.exit_gauge_score; weight += 1.0; }

    if weight > 0.0 { (score / weight).clamp(0.0, 100.0) } else { 50.0 }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ApexSignals widget renderers
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_exit_gauge(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let cx = body.center().x;
    let score = wd.exit_gauge_score;

    let color = match wd.exit_gauge_urgency.as_str() {
        "exit_now" => t.bear,
        "close"    => Color32::from_rgb(220, 80, 80),
        "partial"  => Color32::from_rgb(255, 160, 60),
        "tighten"  => Color32::from_rgb(255, 191, 0),
        _          => t.bull, // "hold"
    };

    // Vertical bar gauge
    let bar_x = cx - 12.0;
    let bar_w = 24.0;
    let bar_h = body.height() - 40.0;
    let bar_y = body.top() + 8.0;

    // Track
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, bar_h)),
        4.0, color_alpha(t.toolbar_border, ALPHA_MUTED));

    // Fill from bottom
    let fill_h = bar_h * (score / 100.0).clamp(0.0, 1.0);
    p.rect_filled(egui::Rect::from_min_size(
        egui::pos2(bar_x, bar_y + bar_h - fill_h), egui::vec2(bar_w, fill_h)),
        4.0, color);

    // Score
    hero_number(p, egui::pos2(cx + 30.0, body.top() + 20.0), &format!("{:.0}", score), color);

    // Urgency label
    let label = if wd.exit_gauge_urgency.is_empty() { "HOLD" } else { &wd.exit_gauge_urgency };
    sub_label(p, egui::pos2(cx, body.bottom() - 10.0), &label.to_uppercase(), color);

    // Zone markers on the bar
    for (pct, lbl) in [(20.0, "H"), (40.0, "T"), (60.0, "P"), (80.0, "C"), (95.0, "X")] {
        let y = bar_y + bar_h * (1.0 - pct / 100.0);
        p.text(egui::pos2(bar_x - 6.0, y), egui::Align2::RIGHT_CENTER,
            lbl, egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.3));
    }
}

fn draw_precursor_alert(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let cx = body.center().x;

    if !wd.precursor_active {
        p.text(egui::pos2(cx, body.center().y - 4.0), egui::Align2::CENTER_CENTER,
            "NO ACTIVITY", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        sub_label(p, egui::pos2(cx, body.center().y + 14.0), "Smart money quiet", t.dim);
        return;
    }

    let dir_col = if wd.precursor_dir > 0 { t.bull } else { t.bear };
    let dir_label = if wd.precursor_dir > 0 { "BULLISH" } else { "BEARISH" };

    // Flash icon
    p.text(egui::pos2(cx, body.top() + 12.0), egui::Align2::CENTER_CENTER,
        "\u{26A1}", egui::FontId::proportional(18.0), dir_col);

    // Score
    hero_number(p, egui::pos2(cx, body.top() + 34.0), &format!("{:.0}", wd.precursor_score), dir_col);

    // Direction
    sub_label(p, egui::pos2(cx, body.top() + 52.0), dir_label, dir_col);

    // Description (truncated)
    if !wd.precursor_desc.is_empty() {
        let desc: String = wd.precursor_desc.chars().take(30).collect();
        p.text(egui::pos2(cx, body.bottom() - 8.0), egui::Align2::CENTER_CENTER,
            &desc, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    }
}

fn draw_trade_plan(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let cx = body.center().x;

    let Some((dir, entry, target, stop, rr, conviction)) = wd.trade_plan else {
        p.text(egui::pos2(cx, body.center().y), egui::Align2::CENTER_CENTER,
            "NO TRADE PLAN", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        return;
    };

    let dir_col = if dir > 0 { t.bull } else { t.bear };
    let dir_label = if dir > 0 { "LONG" } else { "SHORT" };

    // Direction pill
    p.text(egui::pos2(cx, body.top() + 8.0), egui::Align2::CENTER_CENTER,
        dir_label, egui::FontId::monospace(FONT_SM), dir_col);

    // Entry / Target / Stop rows
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let mut y = body.top() + 24.0;
    for (label, price, color) in [("ENTRY", entry, t.text), ("TARGET", target, t.bull), ("STOP", stop, t.bear)] {
        p.text(egui::pos2(left, y), egui::Align2::LEFT_CENTER,
            label, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
        p.text(egui::pos2(right, y), egui::Align2::RIGHT_CENTER,
            &format!("${:.2}", price), egui::FontId::monospace(FONT_SM), color);
        y += 18.0;
    }

    // R:R and conviction
    let rr_col = if rr >= 2.0 { t.bull } else if rr >= 1.0 { Color32::from_rgb(255, 191, 0) } else { t.bear };
    p.text(egui::pos2(left, y + 4.0), egui::Align2::LEFT_CENTER,
        &format!("{:.1}R", rr), egui::FontId::monospace(FONT_LG), rr_col);

    // Conviction bar
    let bar_x = left + 40.0;
    let bar_w = right - bar_x;
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, y + 2.0), egui::vec2(bar_w, 6.0)),
        2.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, y + 2.0),
        egui::vec2(bar_w * (conviction / 100.0).clamp(0.0, 1.0), 6.0)),
        2.0, dir_col);
    p.text(egui::pos2(right, y + 14.0), egui::Align2::RIGHT_CENTER,
        &format!("{:.0}% conviction", conviction), egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
}

fn draw_change_points(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let cx = body.center().x;

    hero_number(p, egui::pos2(cx, body.top() + 18.0),
        &format!("{}", wd.change_points_count), t.accent);
    sub_label(p, egui::pos2(cx, body.top() + 36.0), "REGIME SHIFTS", t.dim);

    if !wd.change_points_latest.is_empty() {
        p.text(egui::pos2(cx, body.top() + 54.0), egui::Align2::CENTER_CENTER,
            &format!("Latest: {}", wd.change_points_latest),
            egui::FontId::monospace(FONT_XS), t.accent);
    }

    // Visual: dots for each change point
    let dot_y = body.bottom() - 12.0;
    let max_dots = ((body.width() - 20.0) / 8.0) as usize;
    let count = wd.change_points_count.min(max_dots);
    let start_x = cx - (count as f32 * 8.0) / 2.0;
    for i in 0..count {
        let x = start_x + i as f32 * 8.0 + 4.0;
        p.circle_filled(egui::pos2(x, dot_y), 2.5, color_alpha(t.accent, ALPHA_DIM));
    }
}

fn draw_zone_strength(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;

    // Zone counts
    let rows = [
        ("TOTAL", format!("{}", wd.zone_count), t.accent),
        ("FRESH", format!("{}", wd.zone_fresh), t.bull),
        ("TESTED", format!("{}", wd.zone_count.saturating_sub(wd.zone_fresh)), Color32::from_rgb(255, 191, 0)),
    ];

    let mut y = body.top() + 8.0;
    for (label, value, color) in &rows {
        p.text(egui::pos2(left, y + 4.0), egui::Align2::LEFT_CENTER,
            *label, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
        p.text(egui::pos2(right, y + 4.0), egui::Align2::RIGHT_CENTER,
            value, egui::FontId::monospace(FONT_LG), *color);
        y += 22.0;
    }

    // Average strength bar
    y += 4.0;
    p.text(egui::pos2(left, y), egui::Align2::LEFT_CENTER,
        "STRENGTH", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    let bar_x = left + 56.0;
    let bar_w = right - bar_x;
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, y - 2.0), egui::vec2(bar_w, 6.0)),
        2.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
    let fill = (wd.zone_avg_strength / 10.0).clamp(0.0, 1.0);
    let str_col = if fill > 0.7 { t.bull } else if fill > 0.4 { Color32::from_rgb(255, 191, 0) } else { t.bear };
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, y - 2.0), egui::vec2(bar_w * fill, 6.0)),
        2.0, str_col);
}

fn draw_pattern_scanner(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let cx = body.center().x;

    if wd.pattern_count == 0 {
        p.text(egui::pos2(cx, body.center().y), egui::Align2::CENTER_CENTER,
            "No patterns", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        return;
    }

    let pat_col = if wd.pattern_latest_bull { t.bull } else { t.bear };

    // Latest pattern name — hero
    p.text(egui::pos2(cx, body.top() + 14.0), egui::Align2::CENTER_CENTER,
        &wd.pattern_latest, egui::FontId::monospace(FONT_LG), pat_col);

    // Confidence bar
    let bar_y = body.top() + 30.0;
    let bar_x = body.left() + 12.0;
    let bar_w = body.width() - 24.0;
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, 6.0)),
        2.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y),
        egui::vec2(bar_w * wd.pattern_latest_conf, 6.0)), 2.0, pat_col);
    p.text(egui::pos2(cx, bar_y + 14.0), egui::Align2::CENTER_CENTER,
        &format!("{:.0}% confidence", wd.pattern_latest_conf * 100.0),
        egui::FontId::monospace(FONT_XS), pat_col);

    // Total count
    p.text(egui::pos2(cx, body.bottom() - 10.0), egui::Align2::CENTER_CENTER,
        &format!("{} patterns detected", wd.pattern_count),
        egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
}

fn draw_vix_monitor(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let cx = body.center().x;

    let vix_col = if wd.vix_spot > 30.0 { t.bear }
        else if wd.vix_spot > 20.0 { Color32::from_rgb(255, 191, 0) }
        else { t.bull };

    // VIX spot hero
    hero_number(p, egui::pos2(cx, body.top() + 18.0), &format!("{:.1}", wd.vix_spot), vix_col);
    sub_label(p, egui::pos2(cx, body.top() + 36.0), "VIX SPOT", t.dim);

    // Gap % and convergence
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let y = body.top() + 52.0;

    let gap_col = if wd.vix_gap_pct.abs() > 5.0 { t.bear } else { t.dim };
    p.text(egui::pos2(left, y), egui::Align2::LEFT_CENTER,
        "GAP", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(right, y), egui::Align2::RIGHT_CENTER,
        &format!("{:+.1}%", wd.vix_gap_pct), egui::FontId::monospace(FONT_SM), gap_col);

    let conv_col = if wd.vix_convergence > 0.7 { t.bull } else if wd.vix_convergence > 0.3 { Color32::from_rgb(255, 191, 0) } else { t.bear };
    p.text(egui::pos2(left, y + 16.0), egui::Align2::LEFT_CENTER,
        "CONV", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(right, y + 16.0), egui::Align2::RIGHT_CENTER,
        &format!("{:.0}%", wd.vix_convergence * 100.0), egui::FontId::monospace(FONT_SM), conv_col);
}

fn draw_signal_dashboard(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let left = body.left() + 8.0;
    let right = body.right() - 8.0;

    // Compact signal overview — each row is a signal with status dot + name + value
    struct Row { name: &'static str, active: bool, value: String, color: Color32 }
    let rows = [
        Row { name: "Trend", active: wd.trend_score > 0.0,
            value: format!("{:.0}", wd.trend_score),
            color: if wd.trend_score > 66.0 { t.bull } else if wd.trend_score > 33.0 { Color32::from_rgb(255, 191, 0) } else { t.bear } },
        Row { name: "Exit", active: wd.exit_gauge_score > 0.0,
            value: wd.exit_gauge_urgency.chars().take(6).collect::<String>().to_uppercase(),
            color: if wd.exit_gauge_score > 60.0 { t.bear } else { t.bull } },
        Row { name: "Precursor", active: wd.precursor_active,
            value: if wd.precursor_active { format!("{:.0}", wd.precursor_score) } else { "—".into() },
            color: if wd.precursor_dir > 0 { t.bull } else if wd.precursor_active { t.bear } else { t.dim } },
        Row { name: "Zones", active: wd.zone_count > 0,
            value: format!("{} ({} fresh)", wd.zone_count, wd.zone_fresh), color: t.accent },
        Row { name: "Patterns", active: wd.pattern_count > 0,
            value: if wd.pattern_count > 0 { wd.pattern_latest.chars().take(8).collect() } else { "—".into() },
            color: if wd.pattern_latest_bull { t.bull } else if wd.pattern_count > 0 { t.bear } else { t.dim } },
        Row { name: "Changes", active: wd.change_points_count > 0,
            value: format!("{}", wd.change_points_count), color: t.accent },
        Row { name: "VIX", active: wd.vix_spot > 0.0,
            value: format!("{:.1}", wd.vix_spot),
            color: if wd.vix_spot > 25.0 { t.bear } else { t.bull } },
    ];

    let row_h = (body.height() - 4.0) / rows.len() as f32;
    for (i, row) in rows.iter().enumerate() {
        let y = body.top() + 2.0 + i as f32 * row_h + row_h * 0.5;
        // Status dot
        let dot_col = if row.active { row.color } else { color_alpha(t.dim, ALPHA_MUTED) };
        p.circle_filled(egui::pos2(left + 4.0, y), 2.5, dot_col);
        // Name
        p.text(egui::pos2(left + 14.0, y), egui::Align2::LEFT_CENTER,
            row.name, egui::FontId::monospace(FONT_XS), if row.active { t.text } else { t.dim.gamma_multiply(0.4) });
        // Value
        p.text(egui::pos2(right, y), egui::Align2::RIGHT_CENTER,
            &row.value, egui::FontId::monospace(FONT_XS), row.color);
    }
}

fn draw_divergence_monitor(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let cx = body.center().x;

    if wd.divergence_count == 0 {
        p.text(egui::pos2(cx, body.center().y - 4.0), egui::Align2::CENTER_CENTER,
            "NO DIVERGENCES", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        sub_label(p, egui::pos2(cx, body.center().y + 14.0), "RSI / MACD aligned", t.dim);
        return;
    }

    hero_number(p, egui::pos2(cx, body.top() + 18.0),
        &format!("{}", wd.divergence_count), Color32::from_rgb(255, 160, 60));
    sub_label(p, egui::pos2(cx, body.top() + 36.0), "ACTIVE DIVERGENCES", t.dim);
}

fn draw_conviction_meter(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    if !wd.bars_loaded { return draw_loading_skeleton(p, body, t); }
    let cx = body.center().x;
    let score = compute_conviction(wd);

    let color = if score > 70.0 { t.bull }
        else if score > 50.0 { lerp_color(Color32::from_rgb(255, 191, 0), t.bull, (score - 50.0) / 20.0) }
        else if score > 30.0 { lerp_color(t.bear, Color32::from_rgb(255, 191, 0), (score - 30.0) / 20.0) }
        else { t.bear };

    // Arc gauge
    let gauge_cy = body.top() + 38.0;
    let r = 28.0;
    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI,
        Stroke::new(3.0, color_alpha(t.toolbar_border, ALPHA_MUTED)), 40);
    let sweep = (score / 100.0) * PI;
    draw_arc(p, egui::pos2(cx, gauge_cy), r, PI - sweep, PI, Stroke::new(3.5, color), 30);

    // Needle
    let a = PI - (score / 100.0) * PI;
    let ne = egui::pos2(cx + (r - 8.0) * a.cos(), gauge_cy - (r - 8.0) * a.sin());
    p.line_segment([egui::pos2(cx, gauge_cy), ne], Stroke::new(1.5, Color32::WHITE));
    p.circle_filled(egui::pos2(cx, gauge_cy), 3.0, Color32::WHITE);

    hero_number(p, egui::pos2(cx, gauge_cy + 14.0), &format!("{:.0}", score), color);

    let label = if score > 75.0 { "HIGH CONVICTION" } else if score > 50.0 { "MODERATE" }
        else if score > 25.0 { "LOW" } else { "NO SIGNAL" };
    sub_label(p, egui::pos2(cx, gauge_cy + 32.0), label, color);
}

/// Draw a resize grip and handle drag interaction. Returns true if resizing occurred.
pub(crate) fn resize_handle(
    ui: &mut egui::Ui, p: &egui::Painter, card_rect: egui::Rect,
    wi: usize, t: &Theme,
) -> Option<egui::Vec2> {
    let grip_size = 12.0;
    let grip_rect = egui::Rect::from_min_size(
        egui::pos2(card_rect.right() - grip_size, card_rect.bottom() - grip_size),
        egui::vec2(grip_size, grip_size));

    // Draw grip lines (three diagonal lines)
    let gr = grip_rect;
    for i in 0..3 {
        let offset = 3.0 + i as f32 * 3.0;
        p.line_segment(
            [egui::pos2(gr.right() - offset, gr.bottom()),
             egui::pos2(gr.right(), gr.bottom() - offset)],
            Stroke::new(STROKE_THIN, color_alpha(t.dim, ALPHA_MUTED)));
    }

    let resp = ui.interact(grip_rect, egui::Id::new(("widget_resize", wi)), egui::Sense::drag());
    if resp.dragged_by(egui::PointerButton::Primary) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        Some(resp.drag_delta())
    } else {
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        }
        None
    }
}
