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
const SNAP_ZONE: f32 = 40.0;   // pixels from edge to trigger snap hint
const YANK_THRESHOLD: f32 = 50.0; // vertical drag needed to undock
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
    // ── Auto-hide during draw mode ──
    if !chart.draw_tool.is_empty() {
        if chart.chart_widgets.iter().any(|w| w.visible) {
            let p = ui.painter_at(rect);
            p.text(egui::pos2(rect.right() - 8.0, rect.top() + 12.0),
                egui::Align2::RIGHT_CENTER, "\u{25C9}",
                egui::FontId::proportional(FONT_SM), color_alpha(t.dim, ALPHA_MUTED));
        }
        return;
    }

    let painter = ui.painter_at(rect);
    let wd = WidgetData::from_chart(chart);

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
    // Deferred context menu actions (to avoid borrow conflicts)
    enum CtxAction { Lock(usize), Delete(usize), ResetSize(usize), DockTop(usize), DockBottom(usize), Undock(usize) }
    let mut ctx_action: Option<CtxAction> = None;

    for wi in 0..n {
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

        // ── Mode toggle icon: ◼ Card, ◯ HUD, ◑ Minimal ──
        let mode_icon = match mode {
            WidgetDisplayMode::Card    => "\u{25FC}", // ◼
            WidgetDisplayMode::Hud     => "\u{25CB}", // ○
            WidgetDisplayMode::Minimal => "\u{25D1}", // ◑
        };

        // Check if pointer is hovering anywhere on the card (for HUD overlay)
        let card_hovered = ui.rect_contains_pointer(card_rect);

        // ── Render based on display mode ──
        if mode == WidgetDisplayMode::Card {
            // Drop shadow
            painter.rect_filled(card_rect.translate(egui::vec2(0.0, 3.0)).expand(2.0),
                RADIUS_LG + 2.0, Color32::from_rgba_unmultiplied(0, 0, 0, 30));
            painter.rect_filled(card_rect.translate(egui::vec2(0.0, 1.5)).expand(1.0),
                RADIUS_LG + 1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 18));

            // Background
            let bg = Color32::from_rgba_unmultiplied(
                t.toolbar_bg.r().saturating_add(4), t.toolbar_bg.g().saturating_add(4),
                t.toolbar_bg.b().saturating_add(6), 230);
            painter.rect_filled(card_rect, RADIUS_LG, bg);

            // Top bevel
            painter.rect_filled(
                egui::Rect::from_min_max(card_rect.min, egui::pos2(card_rect.right(), card_rect.top() + 1.0)),
                egui::CornerRadius { nw: RADIUS_LG as u8, ne: RADIUS_LG as u8, sw: 0, se: 0 },
                Color32::from_rgba_unmultiplied(255, 255, 255, 10));

            // Border
            painter.rect_stroke(card_rect, RADIUS_LG,
                Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_LINE)),
                egui::StrokeKind::Outside);

            // Title bar
            let tr = egui::Rect::from_min_size(card_rect.min, egui::vec2(card_w, title_h));
            painter.text(egui::pos2(tr.left() + 10.0, tr.center().y),
                egui::Align2::LEFT_CENTER, kind.icon(), egui::FontId::proportional(FONT_MD), t.accent);
            painter.text(egui::pos2(tr.left() + 24.0, tr.center().y),
                egui::Align2::LEFT_CENTER, kind.label(), egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);
            let chev = if w.collapsed { "\u{25B6}" } else { "\u{25BC}" };
            painter.text(egui::pos2(tr.right() - 12.0, tr.center().y),
                egui::Align2::CENTER_CENTER, chev, egui::FontId::proportional(6.0), t.dim.gamma_multiply(0.4));

            // Lock icon (when locked)
            if w.locked {
                painter.text(egui::pos2(tr.right() - 42.0, tr.center().y),
                    egui::Align2::CENTER_CENTER, "\u{1F512}",
                    egui::FontId::proportional(7.0), t.dim.gamma_multiply(0.4));
            }

            // Context menu button (⋯)
            let ctx_rect = egui::Rect::from_center_size(
                egui::pos2(tr.right() - 42.0 - if w.locked { 14.0 } else { 0.0 }, tr.center().y),
                egui::vec2(14.0, 14.0));
            let ctx_resp = ui.interact(ctx_rect,
                egui::Id::new(("widget_ctx", wi)), egui::Sense::click());
            if ctx_resp.hovered() {
                painter.rect_filled(ctx_rect, 3.0, color_alpha(t.accent, ALPHA_GHOST));
            }
            painter.text(ctx_rect.center(), egui::Align2::CENTER_CENTER,
                "\u{22EF}", egui::FontId::proportional(FONT_SM),
                if ctx_resp.hovered() { t.accent } else { t.dim.gamma_multiply(0.4) });

            // Context menu popup (deferred actions to avoid borrow conflicts)
            if ctx_resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(egui::Id::new(("widget_popup", wi))));
            }
            let is_locked = w.locked;
            let is_docked = w.dock != WidgetDock::Float;
            egui::popup_below_widget(ui, egui::Id::new(("widget_popup", wi)), &ctx_resp,
                egui::PopupCloseBehavior::CloseOnClickOutside, |ui: &mut egui::Ui| {
                ui.set_min_width(120.0);
                if ui.button(if is_locked { "\u{1F513} Unlock" } else { "\u{1F512} Lock" }).clicked() {
                    ctx_action = Some(CtxAction::Lock(wi));
                    ui.close_menu();
                }
                if ui.button("\u{1F5D1} Delete").clicked() {
                    ctx_action = Some(CtxAction::Delete(wi));
                    ui.close_menu();
                }
                if ui.button("\u{21BB} Reset Size").clicked() {
                    ctx_action = Some(CtxAction::ResetSize(wi));
                    ui.close_menu();
                }
                if ui.button("\u{2B06} Dock Top").clicked() {
                    ctx_action = Some(CtxAction::DockTop(wi));
                    ui.close_menu();
                }
                if ui.button("\u{2B07} Dock Bottom").clicked() {
                    ctx_action = Some(CtxAction::DockBottom(wi));
                    ui.close_menu();
                }
                if is_docked {
                    if ui.button("\u{2197} Undock").clicked() {
                        ctx_action = Some(CtxAction::Undock(wi));
                        ui.close_menu();
                    }
                }
            });

            // Mode toggle button (always visible in Card)
            let toggle_rect = egui::Rect::from_center_size(
                egui::pos2(tr.right() - 28.0, tr.center().y), egui::vec2(14.0, 14.0));
            let toggle_resp = ui.interact(toggle_rect,
                egui::Id::new(("widget_mode", wi)), egui::Sense::click());
            let toggle_col = if toggle_resp.hovered() { t.accent } else { t.dim.gamma_multiply(0.5) };
            if toggle_resp.hovered() {
                painter.rect_filled(toggle_rect, 3.0, color_alpha(t.accent, ALPHA_GHOST));
            }
            painter.text(toggle_rect.center(), egui::Align2::CENTER_CENTER,
                mode_icon, egui::FontId::proportional(FONT_SM), toggle_col);
            if toggle_resp.clicked() {
                mode_toggle = Some(wi);
            }

            // Body
            if !w.collapsed {
                painter.line_segment(
                    [egui::pos2(card_rect.left() + 8.0, card_rect.top() + title_h),
                     egui::pos2(card_rect.right() - 8.0, card_rect.top() + title_h)],
                    Stroke::new(STROKE_HAIR, color_alpha(t.toolbar_border, ALPHA_MUTED)));
                let body = egui::Rect::from_min_size(
                    egui::pos2(card_rect.left(), card_rect.top() + title_h + 2.0),
                    egui::vec2(card_w, card_h - title_h - 2.0));
                draw_widget_body(&painter, body, kind, &wd, t);

                // Resize handle (bottom-right corner)
                if let Some(delta) = resize_handle(ui, &painter, card_rect, wi, t) {
                    let wid = &mut chart.chart_widgets[wi];
                    wid.w = (wid.w + delta.x).clamp(100.0, 400.0);
                    wid.h = (wid.h + delta.y).clamp(60.0, 300.0);
                }
            }
        } else if mode == WidgetDisplayMode::Hud {
            if !w.collapsed {
                draw_widget_body(&painter, card_rect, kind, &wd, t);
            } else {
                draw_mini_badge(&painter, card_rect, kind, &wd, t);
            }

            // HUD hover overlay — show mode toggle only on rollover
            if card_hovered {
                // Faint pill background at top-right
                let pill = egui::Rect::from_min_size(
                    egui::pos2(card_rect.right() - 20.0, card_rect.top()), egui::vec2(20.0, 16.0));
                painter.rect_filled(pill, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 100));
                let toggle_resp = ui.interact(pill,
                    egui::Id::new(("widget_mode", wi)), egui::Sense::click());
                let toggle_col = if toggle_resp.hovered() { t.accent } else { TEXT_PRIMARY };
                painter.text(pill.center(), egui::Align2::CENTER_CENTER,
                    mode_icon, egui::FontId::proportional(FONT_XS), toggle_col);
                if toggle_resp.clicked() {
                    mode_toggle = Some(wi);
                }
            }
        } else {
            // Minimal — faint label + mode toggle
            painter.text(egui::pos2(card_rect.left() + 4.0, card_rect.top() + 8.0),
                egui::Align2::LEFT_CENTER, kind.icon(),
                egui::FontId::proportional(FONT_XS), color_alpha(t.accent, ALPHA_MUTED));
            painter.text(egui::pos2(card_rect.left() + 16.0, card_rect.top() + 8.0),
                egui::Align2::LEFT_CENTER, kind.label(),
                egui::FontId::monospace(7.0), color_alpha(t.dim, ALPHA_MUTED));

            // Mode toggle in the label area
            let toggle_rect = egui::Rect::from_center_size(
                egui::pos2(card_rect.right() - 10.0, card_rect.top() + 8.0), egui::vec2(14.0, 14.0));
            let toggle_resp = ui.interact(toggle_rect,
                egui::Id::new(("widget_mode", wi)), egui::Sense::click());
            let toggle_col = if toggle_resp.hovered() { t.accent } else { t.dim.gamma_multiply(0.3) };
            painter.text(toggle_rect.center(), egui::Align2::CENTER_CENTER,
                mode_icon, egui::FontId::proportional(FONT_XS), toggle_col);
            if toggle_resp.clicked() {
                mode_toggle = Some(wi);
            }

            if !w.collapsed {
                let body = egui::Rect::from_min_size(
                    egui::pos2(card_rect.left(), card_rect.top() + 16.0),
                    egui::vec2(card_w, card_h - 16.0));
                draw_widget_body(&painter, body, kind, &wd, t);
            } else {
                draw_mini_badge(&painter, card_rect, kind, &wd, t);
            }
        }

        // ══════════════════════════════════════════════════════════════════
        // Interaction — magnetic dock model
        //
        // The widget is always being dragged freely with a grab cursor.
        // When the pointer enters the snap zone near an edge, vertical
        // movement locks (magnetic hold) and the widget slides along the
        // strip. If the user pulls vertically past the yank threshold,
        // the hold breaks and the widget floats free again.
        // ══════════════════════════════════════════════════════════════════

        let sense = egui::Sense::click_and_drag();

        // Interaction area: title bar for Card, thin grab strip for others
        let interact_rect = if mode == WidgetDisplayMode::Card {
            egui::Rect::from_min_size(card_rect.min, egui::vec2(card_w, title_h))
        } else {
            // HUD/Minimal: thin strip at top — visible as hover overlay in HUD
            egui::Rect::from_min_size(card_rect.min, egui::vec2(card_w, 16.0))
        };

        let resp = ui.interact(interact_rect, egui::Id::new(("widget_drag", wi)), sense);

        if resp.dragged_by(egui::PointerButton::Primary) && !chart.chart_widgets[wi].locked {
            let d = resp.drag_delta();
            let wid = &mut chart.chart_widgets[wi];
            let pointer = ui.ctx().pointer_interact_pos().unwrap_or(card_rect.center());

            match wid.dock {
                WidgetDock::Float => {
                    // ── Free drag: move both axes ──
                    wid.x += d.x / rect.width();
                    wid.y += d.y / rect.height();
                    wid.x = wid.x.clamp(0.0, 0.95);
                    wid.y = wid.y.clamp(0.0, 0.95);

                    // ── Magnetic snap: check if pointer entered a snap zone ──
                    if pointer.y - rect.top() < SNAP_ZONE {
                        wid.dock = WidgetDock::Top;
                        wid.dock_x = wid.anim_x; // dock at current visual X
                    } else if rect.bottom() - pointer.y < SNAP_ZONE {
                        wid.dock = WidgetDock::Bottom;
                        wid.dock_x = wid.anim_x;
                    }
                }
                WidgetDock::Top | WidgetDock::Bottom => {
                    // ── Magnetically held: slide X freely, Y is locked ──
                    wid.dock_x += d.x;
                    wid.dock_x = wid.dock_x.clamp(
                        rect.left() + STRIP_PAD, rect.right() - card_w - STRIP_PAD);

                    // ── Yank out: measure pull distance from strip center ──
                    let strip_center_y = match wid.dock {
                        WidgetDock::Top    => rect.top() + STRIP_PAD + card_h * 0.5,
                        WidgetDock::Bottom => rect.bottom() - STRIP_PAD - card_h * 0.5,
                        _ => 0.0,
                    };
                    let pull = (pointer.y - strip_center_y).abs();

                    if pull > YANK_THRESHOLD {
                        // Break free — place at current animated position
                        wid.dock = WidgetDock::Float;
                        wid.x = ((wid.anim_x - rect.left()) / rect.width()).clamp(0.0, 0.95);
                        wid.y = ((pointer.y - card_h * 0.5 - rect.top()) / rect.height()).clamp(0.0, 0.95);
                    }
                }
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        // Click: collapse/expand
        if resp.clicked() {
            chart.chart_widgets[wi].collapsed = !chart.chart_widgets[wi].collapsed;
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
    }

    // Apply deferred mode toggle
    if let Some(wi) = mode_toggle {
        chart.chart_widgets[wi].display = chart.chart_widgets[wi].display.cycle();
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
            ("TMR", format!("{:02}:{:02}", h, m), TEXT_PRIMARY)
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
        ChartWidgetKind::Custom => ("USR", "—".into(), t.dim),
    }
}

// ══════════════════════════════════════════════════════════════════��════════════
// Widget body dispatcher
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_widget_body(p: &egui::Painter, body: egui::Rect, kind: ChartWidgetKind,
                    wd: &WidgetData, t: &Theme) {
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
        ChartWidgetKind::Custom        => draw_custom(p, body, t),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Live data extraction
// ═══════════════════════════════════════════════════════════════════════════════

struct WidgetData {
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
}

impl WidgetData {
    fn from_chart(chart: &Chart) -> Self {
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

fn hero_number(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos + egui::vec2(0.0, 0.5), egui::Align2::CENTER_CENTER,
        text, egui::FontId::monospace(22.0),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 25));
    p.text(pos, egui::Align2::CENTER_CENTER,
        text, egui::FontId::monospace(22.0), color);
}

fn sub_label(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER,
        text, egui::FontId::monospace(FONT_XS),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 140));
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

    let gauge_cy = body.top() + 38.0;
    let r = 28.0;

    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI, Stroke::new(3.0,
        color_alpha(t.toolbar_border, ALPHA_MUTED)), 40);
    let sweep = (score / 100.0) * PI;
    draw_arc(p, egui::pos2(cx, gauge_cy), r, PI - sweep, PI, Stroke::new(3.5, color), 30);

    for pct in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let a = PI - pct * PI;
        let inner = r - 5.0;
        let outer = r + 2.0;
        let p1 = egui::pos2(cx + inner * a.cos(), gauge_cy - inner * a.sin());
        let p2 = egui::pos2(cx + outer * a.cos(), gauge_cy - outer * a.sin());
        p.line_segment([p1, p2], Stroke::new(STROKE_THIN, color_alpha(t.dim, ALPHA_DIM)));
    }

    let needle_a = PI - (score / 100.0) * PI;
    let needle_end = egui::pos2(cx + (r - 8.0) * needle_a.cos(), gauge_cy - (r - 8.0) * needle_a.sin());
    p.line_segment([egui::pos2(cx, gauge_cy), needle_end], Stroke::new(1.5, Color32::WHITE));
    p.circle_filled(egui::pos2(cx, gauge_cy), 3.0, Color32::WHITE);

    hero_number(p, egui::pos2(cx, gauge_cy + 14.0), &format!("{:.0}", score), color);

    let regime = if wd.trend_regime.is_empty() {
        if score > 66.0 { "STRONG" } else if score > 33.0 { "MIXED" } else { "WEAK" }
    } else { &wd.trend_regime };
    sub_label(p, egui::pos2(cx, gauge_cy + 32.0), regime, color);

    let dir_icon = match wd.trend_dir { d if d > 0 => "\u{25B2}", d if d < 0 => "\u{25BC}", _ => "\u{25C6}" };
    let dir_col = match wd.trend_dir { d if d > 0 => t.bull, d if d < 0 => t.bear, _ => t.dim };
    p.text(egui::pos2(cx + 30.0, gauge_cy + 14.0), egui::Align2::LEFT_CENTER,
        dir_icon, egui::FontId::proportional(FONT_SM), dir_col);
}

fn draw_momentum_gauge(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let rsi = wd.rsi;
    let mom = wd.momentum;

    let rsi_color = if rsi > 70.0 { t.bull }
        else if rsi < 30.0 { t.bear }
        else { Color32::from_rgb(255, 191, 0) };

    let gauge_cy = body.top() + 36.0;
    let r = 26.0;

    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI,
        Stroke::new(2.5, color_alpha(t.toolbar_border, ALPHA_MUTED)), 40);
    draw_arc(p, egui::pos2(cx, gauge_cy), r, PI * 0.7, PI,
        Stroke::new(2.5, color_alpha(t.bear, ALPHA_MUTED)), 10);
    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI * 0.3,
        Stroke::new(2.5, color_alpha(t.bull, ALPHA_MUTED)), 10);

    let sweep = (rsi / 100.0) * PI;
    draw_arc(p, egui::pos2(cx, gauge_cy), r, PI - sweep, PI,
        Stroke::new(3.0, rsi_color), 30);

    let a = PI - (rsi / 100.0) * PI;
    let ne = egui::pos2(cx + (r - 7.0) * a.cos(), gauge_cy - (r - 7.0) * a.sin());
    p.line_segment([egui::pos2(cx, gauge_cy), ne], Stroke::new(1.5, Color32::WHITE));
    p.circle_filled(egui::pos2(cx, gauge_cy), 2.5, Color32::WHITE);

    hero_number(p, egui::pos2(cx, gauge_cy + 12.0), &format!("{:.0}", rsi), rsi_color);

    let zone = if rsi > 70.0 { "OVERBOUGHT" } else if rsi < 30.0 { "OVERSOLD" } else { "NEUTRAL" };
    sub_label(p, egui::pos2(cx, gauge_cy + 30.0), zone, rsi_color);

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
    hero_number(p, egui::pos2(cx, body.top() + 48.0), &time_str, TEXT_PRIMARY);
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

fn draw_option_greeks(p: &egui::Painter, body: egui::Rect, _t: &Theme) {
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
            &val_str, egui::FontId::monospace(FONT_LG), TEXT_PRIMARY);
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
        &format!("${:.2}", wd.position_avg), egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);
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
        headline, egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);

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
    for (label, price, color) in [("ENTRY", entry, TEXT_PRIMARY), ("TARGET", target, t.bull), ("STOP", stop, t.bear)] {
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
            row.name, egui::FontId::monospace(FONT_XS), if row.active { TEXT_PRIMARY } else { t.dim.gamma_multiply(0.4) });
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
