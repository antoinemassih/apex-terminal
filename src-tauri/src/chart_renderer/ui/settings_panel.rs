//! Settings panel — organized into Appearance, Chart, Trading, Shortcuts tabs.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Theme, Chart, THEMES};
use super::super::commands::{self, AppCommand};
use super::widgets::form::{FormRow, FormRowAlign};
use super::widgets::text::{BodyLabel, SectionLabel};

/// Build a FormRow pre-configured to match the legacy `setting_row` look:
/// 190px label gutter, left-aligned label in white_alpha(180), body right-aligned
/// with 10px inner pad, gap_xs() bottom margin.
fn srow<'a>(label: &'a str, margin: f32) -> FormRow<'a> {
    FormRow::new(label)
        .gutter(190.0)
        .leading_space(margin)
        .label_left(true)
        .label_color(egui::Color32::from_white_alpha(180))
        .alignment(FormRowAlign::Right)
        .inner_pad(10.0)
        .margins(0.0, gap_xs())
}

/// Settings tab selector.
#[derive(Clone, Copy, PartialEq)]
enum SettingsTab { Appearance, Chart, Trading, Shortcuts }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, chart: &mut Chart, t: &Theme, ap: usize) {
if !watchlist.settings_open { return; }

let screen = ctx.screen_rect();
let dialog_w = 580.0_f32;
let dialog_h = (screen.height() * 0.82).min(780.0).max(400.0);
let border = color_alpha(t.toolbar_border, alpha_active());
egui::Window::new("settings_panel".to_string())
    .fixed_pos(egui::pos2(screen.center().x - dialog_w / 2.0, screen.center().y - dialog_h / 2.0))
    .fixed_size(egui::vec2(dialog_w, dialog_h))
    .title_bar(false)
    .frame(egui::Frame::popup(&ctx.style()).fill(t.toolbar_bg).inner_margin(0.0).outer_margin(0.0)
        .stroke(egui::Stroke::new(stroke_std(), border)).corner_radius(r_lg_cr()))
    .show(ctx, |ui| {
        if super::widgets::headers::DialogHeaderWithClose::new("SETTINGS").dim(t.dim).show(ui) { watchlist.settings_open = false; }

        // ── Tab bar ──
        let tab_id = egui::Id::new("settings_active_tab");
        let mut tab: SettingsTab = ui.data_mut(|d| *d.get_temp_mut_or(tab_id, SettingsTab::Appearance));
        ui.horizontal(|ui| {
            ui.add_space(gap_lg());
            super::widgets::tabs::TabBar::new(&mut tab, &[
                (SettingsTab::Appearance, "Appearance"),
                (SettingsTab::Chart,     "Chart"),
                (SettingsTab::Trading,   "Trading"),
                (SettingsTab::Shortcuts, "Shortcuts"),
            ]).accent(t.accent).dim(t.dim).show(ui);
        });
        ui.data_mut(|d| d.insert_temp(tab_id, tab));
        separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
        ui.add_space(gap_sm());

        // ── Tab content in a scroll area ──
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_width(dialog_w - 20.0);
            let m = 10.0; // left margin

            match tab {

// ═══════════════════════════════════════════════════════════════
// APPEARANCE TAB
// ═══════════════════════════════════════════════════════════════
SettingsTab::Appearance => {
    ui.add_space(gap_sm());

    // ── Theme — big preview blocks with mini chart layout ──
    dialog_section(ui, "THEME", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    {
        let card_w = 80.0;
        let card_h = 48.0;
        let cols = 6;
        for row_start in (0..THEMES.len()).step_by(cols) {
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for i in row_start..(row_start + cols).min(THEMES.len()) {
                    let th = &THEMES[i];
                    let sel = chart.theme_idx == i;
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                    let p = ui.painter();

                    // Background fill
                    p.rect_filled(r, radius_md(), th.bg);

                    // Mini toolbar bar at top
                    let tb_h = 6.0;
                    let tb_rect = egui::Rect::from_min_size(r.min, egui::vec2(card_w, tb_h));
                    p.rect_filled(tb_rect, egui::CornerRadius { nw: radius_md() as u8, ne: radius_md() as u8, sw: 0, se: 0 },
                        egui::Color32::from_rgb(
                            th.bg.r().saturating_add(12),
                            th.bg.g().saturating_add(12),
                            th.bg.b().saturating_add(12)));

                    // Mini candles
                    let chart_top = r.top() + tb_h + 4.0;
                    let chart_bottom = r.bottom() - 12.0;
                    let chart_mid = (chart_top + chart_bottom) / 2.0;
                    let bar_w = 4.0;
                    let bar_gap = 2.0;
                    let bar_start_x = r.left() + 8.0;
                    let prices = [0.4, 0.6, 0.3, 0.7, 0.5, 0.8, 0.65, 0.45, 0.55, 0.7];
                    for (bi, &pv) in prices.iter().enumerate() {
                        let x = bar_start_x + bi as f32 * (bar_w + bar_gap);
                        if x + bar_w > r.right() - 6.0 { break; }
                        let is_bull = bi % 3 != 1; // pseudo pattern
                        let color = if is_bull { th.bull } else { th.bear };
                        let h = (chart_bottom - chart_top) * 0.6;
                        let body_top = chart_mid - h * pv + h * 0.2;
                        let body_bot = body_top + h * 0.35;
                        // Wick
                        p.line_segment(
                            [egui::pos2(x + bar_w / 2.0, body_top - 3.0),
                             egui::pos2(x + bar_w / 2.0, body_bot + 3.0)],
                            egui::Stroke::new(0.5, color_alpha(color, alpha_strong())));
                        // Body
                        p.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(x, body_top), egui::pos2(x + bar_w, body_bot)),
                            1.0, color);
                    }

                    // Accent line (like a moving average)
                    let accent_y = chart_mid - 2.0;
                    p.line_segment(
                        [egui::pos2(r.left() + 6.0, accent_y), egui::pos2(r.right() - 6.0, accent_y)],
                        egui::Stroke::new(1.0, color_alpha(th.accent, alpha_strong())));

                    // Theme name at bottom
                    p.text(
                        egui::pos2(r.center().x, r.bottom() - 6.0),
                        egui::Align2::CENTER_CENTER,
                        th.name,
                        egui::FontId::monospace(FONT_XS),
                        if sel { th.accent } else { th.dim.gamma_multiply(0.8) });

                    // Selection border
                    if sel {
                        p.rect_stroke(r, radius_md(), egui::Stroke::new(2.0, th.accent), egui::StrokeKind::Outside);
                    } else if resp.hovered() {
                        p.rect_stroke(r, radius_md(), egui::Stroke::new(1.0, color_alpha(th.accent, alpha_line())), egui::StrokeKind::Outside);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() { commands::push(AppCommand::SetThemeIdx { pane: ap, idx: i }); }
                }
            });
        }
    }
    ui.add_space(gap_lg());

    // ── Font Scale ──
    dialog_section(ui, "FONT SCALE", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    srow("Size", m).show(ui, t, |ui| {
        let display_pct = ((watchlist.font_scale - 0.96) / 0.016).round() as i32 + 60;
        let mut dp = display_pct.clamp(60, 160);
        if ui.add(egui::DragValue::new(&mut dp).range(60..=160).suffix("%").speed(1)
            .custom_formatter(|v, _| format!("{}%", v as i32))).changed() {
            watchlist.font_scale = 0.96 + (dp - 60) as f32 * 0.016;
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(m);
        for (label, ppp) in [(60, 0.96_f32), (80, 1.28), (100, 1.6), (120, 1.92), (140, 2.24), (160, 2.56)] {
            let active = (watchlist.font_scale - ppp).abs() < 0.05;
            let fg = if active { t.accent } else { t.dim.gamma_multiply(0.6) };
            let bg = if active { color_alpha(t.accent, alpha_subtle()) } else { egui::Color32::TRANSPARENT };
            if ui.add(egui::Button::new(egui::RichText::new(format!("{}%", label)).monospace().size(FONT_SM).color(fg))
                .fill(bg).corner_radius(r_sm_cr()).min_size(egui::vec2(34.0, 20.0))).clicked() {
                watchlist.font_scale = ppp;
            }
        }
    });
    ui.add_space(gap_lg());

    // ── Font Family ──
    dialog_section(ui, "FONT", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    {
        let font_names = crate::ui_kit::icons::FONT_NAMES;
        let current_idx = watchlist.font_idx.min(font_names.len() - 1);
        let card_w = 160.0;
        let card_h = 46.0;
        let cols = 3;
        let is_mono = [true, true, true, false, false, false]; // first 3 are mono

        for row_start in (0..font_names.len()).step_by(cols) {
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for i in row_start..(row_start + cols).min(font_names.len()) {
                    let name = font_names[i];
                    let sel = current_idx == i;
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());

                    let bg = if sel { color_alpha(t.accent, alpha_tint()) }
                        else if resp.hovered() { color_alpha(t.toolbar_border, alpha_subtle()) }
                        else { color_alpha(t.toolbar_border, alpha_faint()) };
                    let border_col = if sel { t.accent }
                        else if resp.hovered() { color_alpha(t.accent, alpha_line()) }
                        else { color_alpha(t.toolbar_border, alpha_muted()) };
                    ui.painter().rect_filled(r, radius_md(), bg);
                    ui.painter().rect_stroke(r, radius_md(),
                        egui::Stroke::new(if sel { 1.5 } else { 0.5 }, border_col), egui::StrokeKind::Outside);

                    // Font name
                    let name_col = if sel { t.accent } else { TEXT_PRIMARY };
                    ui.painter().text(
                        egui::pos2(r.center().x, r.top() + 14.0),
                        egui::Align2::CENTER_CENTER,
                        name, egui::FontId::monospace(FONT_SM), name_col);

                    // Type badge + sample
                    let type_label = if is_mono[i.min(is_mono.len()-1)] { "mono" } else { "sans" };
                    let type_col = t.dim.gamma_multiply(0.4);
                    ui.painter().text(
                        egui::pos2(r.left() + 8.0, r.bottom() - 12.0),
                        egui::Align2::LEFT_CENTER,
                        type_label, egui::FontId::monospace(7.0), type_col);

                    let sample_col = if sel { TEXT_PRIMARY } else { t.dim.gamma_multiply(0.7) };
                    ui.painter().text(
                        egui::pos2(r.right() - 8.0, r.bottom() - 12.0),
                        egui::Align2::RIGHT_CENTER,
                        "0123 AAPL $9.50", egui::FontId::monospace(FONT_XS), sample_col);

                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if resp.clicked() && !sel {
                        watchlist.font_idx = i;
                        crate::ui_kit::icons::init_fonts(ui.ctx(), i);
                    }
                }
            });
        }
    }
    ui.add_space(gap_lg());

    // ── Layout ──
    dialog_section(ui, "LAYOUT", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    setting_toggle(ui, m, "Compact Toolbar", t, &mut watchlist.compact_mode);
    setting_toggle(ui, m, "Auto-Hide Toolbar", t, &mut watchlist.toolbar_auto_hide);
    srow("Pane Headers", m).show(ui, t, |ui| {
        use crate::chart_renderer::PaneHeaderSize;
        let current = watchlist.pane_header_size;
        let labels = [
            (PaneHeaderSize::Compact, "Compact"),
            (PaneHeaderSize::Normal, "Normal"),
            (PaneHeaderSize::Expanded, "Expanded"),
        ];
        let active_idx = labels.iter().position(|(s, _)| *s == current).unwrap_or(0);
        let label_refs: Vec<&str> = labels.iter().map(|(_, l)| *l).collect();
        if let Some(i) = segmented_control(ui, active_idx, &label_refs,
            t.toolbar_bg, t.toolbar_border, t.accent, t.dim) {
            watchlist.pane_header_size = labels[i].0;
        }
    });
    ui.add_space(gap_lg());
}

// ═══════════════════════════════════════════════════════════════
// CHART TAB
// ═══════════════════════════════════════════════════════════════
SettingsTab::Chart => {
    ui.add_space(gap_sm());

    // ── Axes ──
    dialog_section(ui, "AXES & GRID", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    setting_toggle(ui, m, "Show X-Axis (time)", t, &mut watchlist.show_x_axis);
    setting_toggle(ui, m, "Show Y-Axis (price)", t, &mut watchlist.show_y_axis);
    setting_toggle(ui, m, "Shared X-Axis (multi-pane)", t, &mut watchlist.shared_x_axis);
    setting_toggle(ui, m, "Shared Y-Axis (multi-pane)", t, &mut watchlist.shared_y_axis);
    ui.add_space(gap_lg());

    // ── Chart Behavior ──
    dialog_section(ui, "CHART BEHAVIOR", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    setting_toggle(ui, m, "OHLC Tooltip", t, &mut chart.ohlc_tooltip);
    setting_toggle(ui, m, "Magnet Snap", t, &mut chart.magnet);
    setting_toggle(ui, m, "Log Scale", t, &mut chart.log_scale);
    setting_toggle(ui, m, "Show Volume", t, &mut chart.show_volume);
    setting_toggle(ui, m, "Show Oscillators", t, &mut chart.show_oscillators);
    ui.add_space(gap_lg());

    // ── Sessions ──
    let is_crypto = crate::data::is_crypto(&chart.symbol);
    dialog_section(ui, "SESSIONS", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    if is_crypto {
        ui.horizontal(|ui| {
            ui.add_space(m);
            ui.add(BodyLabel::new("N/A for crypto (24/7 market)").color(t.dim.gamma_multiply(0.5)));
        });
    } else {
        setting_toggle(ui, m, "Session Shading", t, &mut chart.session_shading);
        if chart.session_shading {
            srow("ETH Bar Opacity", m).show(ui, t, |ui| {
                let mut pct = (chart.eth_bar_opacity * 100.0).round() as i32;
                if ui.add(egui::DragValue::new(&mut pct).range(0..=100).suffix("%").speed(1)).changed() {
                    chart.eth_bar_opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
                }
            });
            setting_toggle(ui, m, "Background Tint", t, &mut chart.session_bg_tint);
            if chart.session_bg_tint {
                ui.horizontal(|ui| {
                    ui.add_space(m + 8.0);
                    for (label, hex) in [("Navy", "#1a1a2e"), ("Purple", "#2d1b4e"), ("Green", "#1a2e1a"), ("Red", "#2e1a1a"), ("Blue", "#1a2e3e")] {
                        let active = chart.session_bg_color == hex;
                        let c = hex_to_color(hex, 1.0);
                        let fg = if active { t.accent } else { egui::Color32::from_white_alpha(120) };
                        let bg = if active { color_alpha(c, alpha_strong()) } else { color_alpha(c, alpha_muted()) };
                        if ui.add(egui::Button::new(egui::RichText::new(label).monospace().size(8.0).color(fg))
                            .fill(bg).corner_radius(r_sm_cr()).min_size(egui::vec2(38.0, 18.0))).clicked() {
                            chart.session_bg_color = hex.to_string();
                        }
                    }
                });
                srow("Tint Opacity", m).show(ui, t, |ui| {
                    let mut pct = (chart.session_bg_opacity * 100.0).round() as i32;
                    if ui.add(egui::DragValue::new(&mut pct).range(0..=100).suffix("%").speed(1)).changed() {
                        chart.session_bg_opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
                    }
                });
            }
            setting_toggle(ui, m, "Session Break Lines", t, &mut chart.session_break_lines);
            ui.horizontal(|ui| {
                ui.add_space(m);
                let (sh, sm2, eh, em2) = (chart.rth_start_minutes / 60, chart.rth_start_minutes % 60,
                    chart.rth_end_minutes / 60, chart.rth_end_minutes % 60);
                ui.add(SectionLabel::new(&format!("RTH: {:02}:{:02} – {:02}:{:02} ET", sh, sm2, eh, em2))
                    .tiny().color(t.dim.gamma_multiply(0.4)));
            });
        }
    }
    ui.add_space(gap_lg());

    ui.add_space(gap_lg());
}

// ═══════════════════════════════════════════════════════════════
// TRADING TAB
// ═══════════════════════════════════════════════════════════════
SettingsTab::Trading => {
    ui.add_space(gap_sm());

    // ── Paper Mode ──
    dialog_section(ui, "MODE", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    {
        let was_paper = crate::chart_renderer::trading::order_manager::is_paper_mode();
        let mut paper = was_paper;
        let color = if paper { egui::Color32::from_rgb(46, 204, 113) } else { egui::Color32::from_rgb(230, 70, 70) };
        setting_toggle_with_color(ui, m, "Paper Trading", t, &mut paper, color);
        if paper != was_paper {
            crate::chart_renderer::trading::order_manager::set_paper_mode(paper);
        }
    }
    ui.horizontal(|ui| {
        ui.add_space(m + 2.0);
        let paper = crate::chart_renderer::trading::order_manager::is_paper_mode();
        let (label, color) = if paper {
            ("Paper mode — orders go to simulated account", egui::Color32::from_rgb(46, 204, 113))
        } else {
            ("LIVE mode — real money at risk", egui::Color32::from_rgb(230, 70, 70))
        };
        ui.add(SectionLabel::new(label).tiny().color(color));
    });
    ui.add_space(gap_lg());

    // ── Order Defaults ──
    dialog_section(ui, "ORDER DEFAULTS", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    srow("Stock Qty", m).show(ui, t, |ui| {
        let mut v = watchlist.default_stock_qty as i32;
        if ui.add(egui::DragValue::new(&mut v).range(1..=100_000).speed(10)
            .custom_formatter(|v, _| format!("{} shares", v as i32))).changed() {
            watchlist.default_stock_qty = v.max(1) as u32;
        }
    });
    srow("Options Qty", m).show(ui, t, |ui| {
        let mut v = watchlist.default_options_qty as i32;
        if ui.add(egui::DragValue::new(&mut v).range(1..=10_000).speed(1)
            .custom_formatter(|v, _| format!("{} contracts", v as i32))).changed() {
            watchlist.default_options_qty = v.max(1) as u32;
        }
    });
    srow("Order Type", m).show(ui, t, |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, label) in ["MKT", "LMT", "STP"].iter().enumerate() {
            let sel = watchlist.default_order_type == i;
            let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.6) };
            let bg = if sel { color_alpha(t.accent, alpha_line()) } else { color_alpha(t.toolbar_border, alpha_soft()) };
            let cr = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                else if i == 2 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                else { egui::CornerRadius::ZERO };
            if ui.add(egui::Button::new(egui::RichText::new(*label).monospace().size(FONT_SM).color(fg))
                .fill(bg).corner_radius(cr).min_size(egui::vec2(34.0, 20.0))
                .stroke(egui::Stroke::new(stroke_thin(), if sel { color_alpha(t.accent, alpha_strong()) } else { color_alpha(t.toolbar_border, alpha_muted()) })))
                .clicked() { watchlist.default_order_type = i; }
        }
    });
    srow("Time in Force", m).show(ui, t, |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, label) in ["DAY", "GTC", "IOC"].iter().enumerate() {
            let sel = watchlist.default_tif == i;
            let fg = if sel { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.6) };
            let bg = if sel { color_alpha(t.accent, alpha_line()) } else { color_alpha(t.toolbar_border, alpha_soft()) };
            let cr = if i == 0 { egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 } }
                else if i == 2 { egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 } }
                else { egui::CornerRadius::ZERO };
            if ui.add(egui::Button::new(egui::RichText::new(*label).monospace().size(FONT_SM).color(fg))
                .fill(bg).corner_radius(cr).min_size(egui::vec2(34.0, 20.0))
                .stroke(egui::Stroke::new(stroke_thin(), if sel { color_alpha(t.accent, alpha_strong()) } else { color_alpha(t.toolbar_border, alpha_muted()) })))
                .clicked() { watchlist.default_tif = i; }
        }
    });
    setting_toggle(ui, m, "Outside RTH", t, &mut watchlist.default_outside_rth);
    ui.add_space(gap_lg());

    // ── Risk Management ──
    dialog_section(ui, "RISK MANAGEMENT", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    {
        use crate::chart_renderer::trading::order_manager;
        let mut limits = order_manager::get_risk_limits();
        srow("Max Order Qty", m).show(ui, t, |ui| {
            let mut v = limits.max_order_qty as i32;
            if ui.add(egui::DragValue::new(&mut v).range(1..=100_000).speed(10)).changed() {
                limits.max_order_qty = v.max(1) as u32;
            }
        });
        srow("Max Position", m).show(ui, t, |ui| {
            let mut v = limits.max_position_qty as i32;
            if ui.add(egui::DragValue::new(&mut v).range(1..=500_000).speed(100)).changed() {
                limits.max_position_qty = v.max(1) as u32;
            }
        });
        srow("Max Notional $", m).show(ui, t, |ui| {
            let mut v = limits.max_notional as i64;
            if ui.add(egui::DragValue::new(&mut v).range(0..=10_000_000).speed(1000)
                .custom_formatter(|v, _| if v as i64 == 0 { "OFF".into() } else { format!("${}", v as i64) })).changed() {
                limits.max_notional = v.max(0) as f64;
            }
        });
        srow("Fat Finger %", m).show(ui, t, |ui| {
            let mut v = limits.fat_finger_pct;
            if ui.add(egui::DragValue::new(&mut v).range(0.0..=50.0).speed(0.5).suffix("%")
                .custom_formatter(|v, _| if v < 0.1 { "OFF".into() } else { format!("{:.1}%", v) })).changed() {
                limits.fat_finger_pct = v.max(0.0);
            }
        });
        srow("Max Open Orders", m).show(ui, t, |ui| {
            let mut v = limits.max_open_orders as i32;
            if ui.add(egui::DragValue::new(&mut v).range(1..=1000).speed(1)).changed() {
                limits.max_open_orders = v.max(1) as usize;
            }
        });
        srow("Max Daily Loss $", m).show(ui, t, |ui| {
            let mut v = limits.max_daily_loss as i64;
            if ui.add(egui::DragValue::new(&mut v).range(0..=1_000_000).speed(500)
                .custom_formatter(|v, _| if v as i64 == 0 { "OFF".into() } else { format!("${}", v as i64) })).changed() {
                limits.max_daily_loss = v.max(0) as f64;
            }
        });
        srow("Dedup Cooldown", m).show(ui, t, |ui| {
            let mut v = limits.dedup_cooldown_ms as i32;
            if ui.add(egui::DragValue::new(&mut v).range(100..=5000).speed(50).suffix("ms")).changed() {
                limits.dedup_cooldown_ms = v.max(100) as u64;
            }
        });
        order_manager::update_risk_limits(limits);
    }
    ui.add_space(gap_lg());

    // ── ApexData ─────────────────────────────────────────────────────
    dialog_section(ui, "APEX DATA", m, t.dim.gamma_multiply(0.5));
    ui.add_space(gap_sm());
    {
        let mut enabled = crate::apex_data::is_enabled();
        let prev = enabled;
        setting_toggle(ui, m, "Enabled", t, &mut enabled);
        if enabled != prev {
            crate::apex_data::set_enabled(enabled);
            if enabled { crate::apex_data::ws::start(); }
        }

        FormRow::new("Base URL")
            .gutter(190.0)
            .leading_space(m)
            .label_left(true)
            .label_color(egui::Color32::from_white_alpha(180))
            .alignment(FormRowAlign::Right)
            .inner_pad(10.0)
            .margins(0.0, gap_xs())
            .show_with_cx(ui, t, |ui, _cx| {
                let id = egui::Id::new("apex_data_url_edit");
                let mut buf: String = ui.data_mut(|d|
                    d.get_temp::<String>(id).unwrap_or_else(|| crate::apex_data::apex_url()));
                let resp = ui.add(egui::TextEdit::singleline(&mut buf).desired_width(340.0));
                if resp.changed() { ui.data_mut(|d| d.insert_temp(id, buf.clone())); }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    crate::apex_data::set_apex_url(buf.trim().to_string());
                }
            });

        FormRow::new("Auth Token")
            .gutter(190.0)
            .leading_space(m)
            .label_left(true)
            .label_color(egui::Color32::from_white_alpha(180))
            .alignment(FormRowAlign::Right)
            .inner_pad(10.0)
            .margins(0.0, gap_xs())
            .password(true)
            .hint("optional — leave blank if no token required")
            .show_with_cx(ui, t, |ui, cx| {
                let id = egui::Id::new("apex_data_token_edit");
                let mut buf: String = ui.data_mut(|d|
                    d.get_temp::<String>(id).unwrap_or_else(|| crate::apex_data::apex_token().unwrap_or_default()));
                let mut te = egui::TextEdit::singleline(&mut buf).desired_width(340.0);
                if cx.password { te = te.password(true); }
                if let Some(h) = cx.hint { te = te.hint_text(h); }
                let resp = ui.add(te);
                if resp.changed() { ui.data_mut(|d| d.insert_temp(id, buf.clone())); }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let tok = buf.trim();
                    crate::apex_data::set_apex_token(if tok.is_empty() { None } else { Some(tok.to_string()) });
                }
            });

        ui.horizontal(|ui| {
            ui.add_space(m + 2.0);
            let ws_connected = crate::apex_data::ws::is_connected();
            let (state_label, state_col) = if ws_connected {
                ("WS connected", egui::Color32::from_rgb(46, 204, 113))
            } else {
                ("WS disconnected", egui::Color32::from_rgb(230, 70, 70))
            };
            ui.add(SectionLabel::new(state_label).tiny().color(state_col));
        });
    }
    ui.add_space(gap_lg());
}

// ═══════════════════════════════════════════════════════════════
// SHORTCUTS TAB
// ═══════════════════════════════════════════════════════════════
SettingsTab::Shortcuts => {
    ui.add_space(gap_sm());
    // Column header
    ui.horizontal(|ui| {
        ui.add_space(m);
        ui.allocate_ui(egui::vec2(220.0, 16.0), |ui| {
            ui.add(SectionLabel::new("ACTION").tiny().color(t.dim.gamma_multiply(0.4)));
        });
        ui.add(SectionLabel::new("SHORTCUT").tiny().color(t.dim.gamma_multiply(0.4)));
    });
    separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
    ui.add_space(gap_xs());

    super::hotkey_editor::draw_content(ui, watchlist, t);
}

            } // end match
        }); // end scroll area
    });
}

// ─── Helpers: standard setting toggles (built atop FormRow via `srow`) ────

fn setting_toggle(ui: &mut egui::Ui, margin: f32, label: &str, t: &Theme, val: &mut bool) {
    srow(label, margin).show(ui, t, |ui| {
        ui.add(egui::Checkbox::without_text(val));
    });
}

fn setting_toggle_with_color(ui: &mut egui::Ui, margin: f32, label: &str, t: &Theme, val: &mut bool, _color: egui::Color32) {
    srow(label, margin).show(ui, t, |ui| {
        ui.add(egui::Checkbox::without_text(val));
    });
}
