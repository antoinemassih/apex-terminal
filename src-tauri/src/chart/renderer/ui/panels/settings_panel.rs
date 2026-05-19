//! Settings panel — organized into Appearance, Chart, Trading, Shortcuts tabs.
//!
//! Modal + HeaderStyle::Dialog chrome is canonical (per Agent O). This pass
//! fixes the BODY: kills the per-section `m = 10.0` margin literal in favor
//! of one body inner_margin derived from `gap_*()` tokens, hoists top-level
//! groups onto `PanelSection`, and uses `PanelSubSection` for the typography
//! sub-categories. FormRow / FormSection are kept for actual input rows.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme, Chart, THEMES};
use super::super::super::commands::{self, AppCommand};
use crate::ui_kit::widgets::{FormRow, FormRowAlign};
use super::super::widgets::text::{BodyLabel, SectionLabel};
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant, Size};
use crate::ui_kit::widgets::{ToggleRow, ThemePreviewCard, NumberStepper, PanelSection, PanelSubSection};
use crate::ui_kit::widgets::SegmentedControl;
use crate::ui_kit::widgets::theme_preview_card::PreviewKind;
use crate::ui_kit::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};

/// Unified body padding for the settings modal. Single source of truth —
/// every PanelSection / FormRow inside the scroll area inherits this and
/// nothing re-applies its own left indent. Picked `gap_md()` (12px) as the
/// closest token to the legacy `m = 10.0` literal so the visual delta is
/// imperceptible while gaining cross-section X alignment.
fn body_inset() -> f32 { gap_md() }

/// FormRow preset matching the legacy `srow()` look (190px label gutter,
/// muted left-aligned label, right-aligned body with 10px inner pad, xs
/// bottom margin) — but WITHOUT a per-row `leading_space`, because the
/// surrounding modal body already provides one consistent left inset.
/// Inlining this at every call site would obscure the actual content.
fn setting_form_row<'a>(label: &'a str, t: &Theme) -> FormRow<'a> {
    FormRow::new(label)
        .gutter(190.0)
        .label_left(true)
        .label_color(color_alpha(t.text, 180))
        .alignment(FormRowAlign::Right)
        .inner_pad(10.0)
        .margins(0.0, gap_xs())
}

/// Read-or-create a persistent collapse bool keyed by id_salt. Used by the
/// few PanelSubSection groupings inside Settings (TYPOGRAPHY → FONT FAMILY
/// / SIZE SCALE) so expanded state survives across modal open/close.
fn read_persisted_bool(ui: &egui::Ui, id_salt: &str, default: bool) -> bool {
    let id = ui.make_persistent_id(("settings_collapse", id_salt));
    ui.data_mut(|d| *d.get_persisted_mut_or(id, default))
}

fn write_persisted_bool(ui: &egui::Ui, id_salt: &str, value: bool) {
    let id = ui.make_persistent_id(("settings_collapse", id_salt));
    ui.data_mut(|d| d.insert_persisted(id, value));
}

/// Settings tab selector.
#[derive(Clone, Copy, PartialEq)]
enum SettingsTab { Appearance, Chart, Trading, Shortcuts }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, chart: &mut Chart, t: &Theme, ap: usize) {
if !watchlist.settings_open { return; }

let screen = ctx.screen_rect();
let dialog_w = 580.0_f32;
let dialog_h = (screen.height() * 0.82).min(780.0).max(400.0);
let dialog_pos = egui::pos2(screen.center().x - dialog_w / 2.0, screen.center().y - dialog_h / 2.0);
let frame = super::super::widgets::frames::PopupFrame::new().theme(t).ctx(ctx).build()
    .inner_margin(0.0).outer_margin(0.0);
let modal_resp = Modal::new("SETTINGS")
    .id("settings_panel")
    .ctx(ctx)
    .theme(t)
    .size(egui::vec2(dialog_w, dialog_h))
    .anchor(Anchor::Window { pos: Some(dialog_pos) })
    .header_style(HeaderStyle::Dialog)
    .frame_kind(FrameKind::Custom(frame))
    .separator(false)
    .show(|ui| {

        // ── Tab bar — `ui_kit::widgets::Tabs` (replaces legacy TabBar). ──
        const TAB_VARIANTS: &[SettingsTab] = &[
            SettingsTab::Appearance,
            SettingsTab::Chart,
            SettingsTab::Trading,
            SettingsTab::Shortcuts,
        ];
        const TAB_LABELS: &[&str] = &["Appearance", "Chart", "Trading", "Shortcuts"];
        let tab_id = egui::Id::new("settings_active_tab");
        let mut tab: SettingsTab = ui.data_mut(|d| *d.get_temp_mut_or(tab_id, SettingsTab::Appearance));
        let mut idx = TAB_VARIANTS.iter().position(|v| *v == tab).unwrap_or(0);
        ui.horizontal(|ui| {
            ui.add_space(gap_lg());
            crate::ui_kit::widgets::Tabs::new(&mut idx, TAB_LABELS)
                .treatment(crate::ui_kit::widgets::tabs::TabTreatment::Card)
                .show(ui, t);
        });
        tab = TAB_VARIANTS[idx.min(TAB_VARIANTS.len() - 1)];
        ui.data_mut(|d| d.insert_temp(tab_id, tab));
        separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
        ui.add_space(gap_sm());

        // ── Tab content in a scroll area ──
        //
        // ONE body Frame with a single inner_margin = body_inset(). Every
        // PanelSection / FormRow inside starts at the same X coordinate.
        // No per-section `ui.horizontal { ui.add_space(m); ... }` indent.
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_width(dialog_w - 20.0);
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(body_inset() as i8, gap_sm() as i8))
                .show(ui, |ui| {
                    match tab {
                        SettingsTab::Appearance => draw_appearance(ui, watchlist, chart, t, ap),
                        SettingsTab::Chart      => draw_chart(ui, watchlist, chart, t),
                        SettingsTab::Trading    => draw_trading(ui, watchlist, t),
                        SettingsTab::Shortcuts  => draw_shortcuts(ui, watchlist, t),
                    }
                });
        });
    });
    if modal_resp.closed { watchlist.settings_open = false; }
}

// ═══════════════════════════════════════════════════════════════
// APPEARANCE TAB
// ═══════════════════════════════════════════════════════════════
fn draw_appearance(ui: &mut egui::Ui, watchlist: &mut Watchlist, chart: &mut Chart, t: &Theme, ap: usize) {
    // ── THEME — big preview blocks with mini chart layout ──
    PanelSection::new("THEME").show(ui, t, |ui, t| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        ui.horizontal_wrapped(|ui| {
            for (i, preview_theme) in THEMES.iter().enumerate() {
                let resp = ThemePreviewCard::new(preview_theme.name, preview_theme)
                    .selected(chart.theme_idx == i)
                    .preview_kind(PreviewKind::Chart)
                    .show(ui, t);
                if resp.clicked() {
                    commands::push(AppCommand::SetThemeIdx { pane: ap, idx: i });
                }
            }
        });
    });

    // ── STYLE preset (Aperture / Octave / Meridien / …) ──
    PanelSection::new("STYLE").show(ui, t, |ui, t| {
        let presets = crate::chart_renderer::ui::style::list_style_presets();
        let cur_si = watchlist.style_idx.min(presets.len().saturating_sub(1));
        let btn_w: f32 = 78.0;
        let btn_h: f32 = 26.0;
        let row_w = ui.available_width().max(btn_w);
        let per_row = ((row_w + gap_xs()) / (btn_w + gap_xs())).floor().max(1.0) as usize;
        for chunk in presets.chunks(per_row) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap_xs();
                for (id, name) in chunk {
                    let id_us = *id as usize;
                    let active = id_us == cur_si;
                    if Button::toggle(name.as_str(), active)
                        .corner_radius(crate::chart_renderer::ui::style::current().r_sm as f32)
                        .min_size(egui::vec2(btn_w, btn_h))
                        .show(ui, t).clicked() {
                        commands::push(AppCommand::SetStyleIdx { idx: id_us });
                    }
                }
            });
            ui.add_space(gap_xs());
        }
    });

    // ── TYPOGRAPHY — collapsible sub-categories ──
    PanelSection::new("TYPOGRAPHY").show(ui, t, |ui, t| {
        // SIZE SCALE
        let mut size_open = read_persisted_bool(ui, "typo_size", true);
        let prev_size = size_open;
        PanelSubSection::new("typo_size", "SIZE SCALE")
            .expanded(&mut size_open)
            .show(ui, t, |ui, t| {
                setting_form_row("Size", t).show(ui, t, |ui| {
                    let display_pct = ((watchlist.font_scale - 0.96) / 0.016).round() as i32 + 60;
                    let mut dp = display_pct.clamp(60, 160);
                    if crate::ui_kit::widgets::Slider::new(&mut dp, 60..=160)
                        .step(1.0)
                        .show_value(true)
                        .show(ui, t)
                        .changed() {
                        watchlist.font_scale = 0.96 + (dp - 60) as f32 * 0.016;
                    }
                });
                ui.horizontal(|ui| {
                    for (label, ppp) in [(60, 0.96_f32), (80, 1.28), (100, 1.6), (120, 1.92), (140, 2.24), (160, 2.56)] {
                        let active = (watchlist.font_scale - ppp).abs() < 0.05;
                        let pct_label = format!("{}%", label);
                        if Button::toggle(pct_label.as_str(), active)
                            .corner_radius(crate::chart_renderer::ui::style::current().r_sm as f32)
                            .min_size(egui::vec2(34.0, row_height_compact()))
                            .show(ui, t).clicked() {
                            watchlist.font_scale = ppp;
                        }
                    }
                });
            });
        if size_open != prev_size { write_persisted_bool(ui, "typo_size", size_open); }

        // FONT FAMILY
        let mut family_open = read_persisted_bool(ui, "typo_family", true);
        let prev_family = family_open;
        PanelSubSection::new("typo_family", "FONT FAMILY")
            .expanded(&mut family_open)
            .show(ui, t, |ui, t| {
                let font_names = crate::ui_kit::icons::FONT_NAMES;
                let current_idx = watchlist.font_idx.min(font_names.len() - 1);
                let card_w = 160.0;
                let card_h = 46.0;
                let cols = 3;
                let is_mono = [true, true, true, false, false, false];

                for row_start in (0..font_names.len()).step_by(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                        for i in row_start..(row_start + cols).min(font_names.len()) {
                            let name = font_names[i];
                            let sel = current_idx == i;
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                            crate::chart_renderer::ui::style::cursor::clickable(ui, &resp);

                            let bg = if sel { color_alpha(t.accent, alpha_tint()) }
                                else if resp.hovered() { color_alpha(t.toolbar_border, alpha_subtle()) }
                                else { color_alpha(t.toolbar_border, alpha_faint()) };
                            let border_col = if sel { t.accent }
                                else if resp.hovered() { color_alpha(t.accent, alpha_line()) }
                                else { color_alpha(t.toolbar_border, alpha_muted()) };
                            ui.painter().rect_filled(r, radius_md(), bg);
                            ui.painter().rect_stroke(r, radius_md(),
                                egui::Stroke::new(if sel { stroke_bold() } else { stroke_thin() }, border_col), egui::StrokeKind::Outside);

                            let name_col = if sel { t.accent } else { TEXT_PRIMARY };
                            ui.painter().text(
                                egui::pos2(r.center().x, r.top() + 14.0),
                                egui::Align2::CENTER_CENTER,
                                name, mono_sm(), name_col);

                            let type_label = if is_mono[i.min(is_mono.len()-1)] { "mono" } else { "sans" };
                            let type_col = color_dim(t.dim);
                            ui.painter().text(
                                egui::pos2(r.left() + 8.0, r.bottom() - 12.0),
                                egui::Align2::LEFT_CENTER,
                                type_label, mono_sm(), type_col);

                            let sample_col = if sel { TEXT_PRIMARY } else { color_subtle(t.dim) };
                            ui.painter().text(
                                egui::pos2(r.right() - 8.0, r.bottom() - 12.0),
                                egui::Align2::RIGHT_CENTER,
                                "0123 AAPL $9.50", mono_xs(), sample_col);

                            if resp.clicked() && !sel {
                                watchlist.font_idx = i;
                                crate::ui_kit::icons::init_fonts(ui.ctx(), i);
                            }
                        }
                    });
                }
            });
        if family_open != prev_family { write_persisted_bool(ui, "typo_family", family_open); }
    });

    // ── LAYOUT ──
    PanelSection::new("LAYOUT").show(ui, t, |ui, t| {
        setting_toggle_described(ui, "Compact Toolbar",
            Some("Trade vertical density for visual breathing room."),
            t, &mut watchlist.compact_mode);
        setting_toggle_described(ui, "Auto-Hide Toolbar",
            Some("Hide the toolbar until the cursor approaches the top of the pane."),
            t, &mut watchlist.toolbar_auto_hide);
        setting_form_row("Pane Headers", t).show(ui, t, |ui| {
            use crate::chart_renderer::PaneHeaderSize;
            let current = watchlist.pane_header_size;
            let labels = [
                (PaneHeaderSize::Compact, "Compact"),
                (PaneHeaderSize::Normal, "Normal"),
                (PaneHeaderSize::Expanded, "Expanded"),
            ];
            let mut active_idx = labels.iter().position(|(s, _)| *s == current).unwrap_or(0);
            const PANE_HEADER_OPTS: &[(usize, &str)] = &[(0, "Compact"), (1, "Normal"), (2, "Expanded")];
            if SegmentedControl::new(&mut active_idx, PANE_HEADER_OPTS).show(ui, t).changed() {
                watchlist.pane_header_size = labels[active_idx.min(labels.len() - 1)].0;
            }
        });
    });

    // ── ONBOARDING ──
    PanelSection::new("ONBOARDING").show(ui, t, |ui, t| {
        setting_form_row("Welcome wizard", t).show(ui, t, |ui| {
            if Button::new("Show welcome again")
                .variant(crate::ui_kit::widgets::tokens::Variant::Secondary)
                .size(crate::ui_kit::widgets::tokens::Size::Sm)
                .show(ui, t)
                .clicked()
            {
                watchlist.update_ui_settings(|s| {
                    s.has_seen_welcome = false;
                    s.welcome_step_resume = 0;
                });
                watchlist.welcome_wizard = Some(
                    crate::chart_renderer::ui::welcome::WelcomeWizard::from_settings(false, 0)
                );
            }
        });
    });
}

// ═══════════════════════════════════════════════════════════════
// CHART TAB
// ═══════════════════════════════════════════════════════════════
fn draw_chart(ui: &mut egui::Ui, watchlist: &mut Watchlist, chart: &mut Chart, t: &Theme) {
    // ── AXES & GRID ──
    PanelSection::new("AXES & GRID").show(ui, t, |ui, t| {
        setting_toggle_described(ui, "Show X-Axis (time)",
            Some("Render the time axis along the bottom of every chart pane."),
            t, &mut watchlist.show_x_axis);
        setting_toggle_described(ui, "Show Y-Axis (price)",
            Some("Render the price axis along the right edge of every chart pane."),
            t, &mut watchlist.show_y_axis);
        setting_toggle_described(ui, "Shared X-Axis (multi-pane)",
            Some("Stacked panes scroll and zoom together on the time axis."),
            t, &mut watchlist.shared_x_axis);
        setting_toggle_described(ui, "Shared Y-Axis (multi-pane)",
            Some("Stacked panes share a synchronized price scale."),
            t, &mut watchlist.shared_y_axis);
    });

    // ── CHART BEHAVIOR ──
    PanelSection::new("CHART BEHAVIOR").show(ui, t, |ui, t| {
        setting_toggle_described(ui, "OHLC Tooltip",
            Some("Show open/high/low/close values for the bar under the cursor."),
            t, &mut chart.ohlc_tooltip);
        setting_toggle_described(ui, "Magnet Snap",
            Some("Snap drawings and the crosshair to the nearest OHLC level."),
            t, &mut chart.magnet);
        setting_toggle_described(ui, "Log Scale",
            Some("Use a logarithmic price axis so equal % moves render at equal heights."),
            t, &mut chart.log_scale);
        setting_toggle(ui, "Show Volume", t, &mut chart.show_volume);
        setting_toggle(ui, "Show Oscillators", t, &mut chart.show_oscillators);
    });

    // ── SESSIONS ──
    // Wave 9c: prefer registry-backed truth via `symbol_meta` so a real
    // equity ticker that happens to end in USDT doesn't get the "N/A for
    // crypto" 24/7 treatment.
    let is_crypto = chart.symbol_meta.is_crypto();
    PanelSection::new("SESSIONS").show(ui, t, |ui, t| {
        if is_crypto {
            ui.add(BodyLabel::new("N/A for crypto (24/7 market)").color(color_half(t.dim)));
        } else {
            setting_toggle_described(ui, "Session Shading",
                Some("Visually distinguish regular and extended trading hours on the chart."),
                t, &mut chart.session_shading);
            if chart.session_shading {
                setting_form_row("ETH Bar Opacity", t).show(ui, t, |ui| {
                    let mut pct = (chart.eth_bar_opacity * 100.0).round() as i32;
                    if crate::ui_kit::widgets::Slider::new(&mut pct, 0..=100)
                        .step(1.0)
                        .show_value(true)
                        .show(ui, t)
                        .changed() {
                        chart.eth_bar_opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
                    }
                });
                setting_toggle(ui, "Background Tint", t, &mut chart.session_bg_tint);
                if chart.session_bg_tint {
                    ui.horizontal(|ui| {
                        ui.add_space(gap_sm());
                        for (label, hex) in [("Navy", "#1a1a2e"), ("Purple", "#2d1b4e"), ("Green", "#1a2e1a"), ("Red", "#2e1a1a"), ("Blue", "#1a2e3e")] {
                            let active = chart.session_bg_color == hex;
                            let c = hex_to_color(hex, 1.0);
                            let fg = if active { t.accent } else { color_alpha(t.text, 120) };
                            let bg = if active { color_alpha(c, alpha_strong()) } else { color_alpha(c, alpha_muted()) };
                            if Button::new(label).variant(Variant::Chrome).size(Size::Xs).fg(fg)
                                .fill(bg).corner_radius(crate::chart_renderer::ui::style::current().r_sm as f32).min_size(egui::vec2(38.0, row_height_dense())).show(ui, t).clicked() {
                                chart.session_bg_color = hex.to_string();
                            }
                        }
                    });
                    setting_form_row("Tint Opacity", t).show(ui, t, |ui| {
                        let mut pct = (chart.session_bg_opacity * 100.0).round() as i32;
                        if crate::ui_kit::widgets::Slider::new(&mut pct, 0..=100)
                            .step(1.0)
                            .show_value(true)
                            .show(ui, t)
                            .changed() {
                            chart.session_bg_opacity = (pct as f32 / 100.0).clamp(0.0, 1.0);
                        }
                    });
                }
                setting_toggle(ui, "Session Break Lines", t, &mut chart.session_break_lines);
                let (sh, sm2, eh, em2) = (chart.rth_start_minutes / 60, chart.rth_start_minutes % 60,
                    chart.rth_end_minutes / 60, chart.rth_end_minutes % 60);
                ui.add(SectionLabel::new(&format!("RTH: {:02}:{:02} – {:02}:{:02} ET", sh, sm2, eh, em2))
                    .tiny().color(color_dim(t.dim)));
            }
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// TRADING TAB
// ═══════════════════════════════════════════════════════════════
fn draw_trading(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    // ── MODE (Paper / Live) ──
    PanelSection::new("MODE").show(ui, t, |ui, t| {
        let was_paper = crate::chart_renderer::trading::order_manager::is_paper_mode();
        let mut paper = was_paper;
        setting_toggle_described(ui, "Paper Trading",
            Some("Route orders to the simulated account instead of the live broker."),
            t, &mut paper);
        if paper != was_paper {
            crate::chart_renderer::trading::order_manager::set_paper_mode(paper);
        }
        let paper = crate::chart_renderer::trading::order_manager::is_paper_mode();
        let (label, color) = if paper {
            ("Paper mode — orders go to simulated account", t.bull)
        } else {
            ("LIVE mode — real money at risk", t.bear)
        };
        ui.add(SectionLabel::new(label).tiny().color(color));
    });

    // ── ORDER DEFAULTS ──
    // Wave 2 (state): flat fields are mutated in-place for SegmentedControl
    // compatibility; push_to_trading_defaults_store() propagates every change
    // into the Store<TradingDefaults> so the persist supervisor can write.
    let mut trading_changed = false;
    PanelSection::new("ORDER DEFAULTS").show(ui, t, |ui, t| {
        setting_form_row("Stock Qty", t).show(ui, t, |ui| {
            let mut v = watchlist.default_stock_qty as i32;
            if NumberStepper::new(&mut v).range(1..=100_000).step(10.0).suffix(" shares").integer().show(ui, t).changed() {
                watchlist.default_stock_qty = v.max(1) as u32;
                trading_changed = true;
            }
        });
        setting_form_row("Options Qty", t).show(ui, t, |ui| {
            let mut v = watchlist.default_options_qty as i32;
            if NumberStepper::new(&mut v).range(1..=10_000).step(1.0).suffix(" contracts").integer().show(ui, t).changed() {
                watchlist.default_options_qty = v.max(1) as u32;
                trading_changed = true;
            }
        });
        setting_form_row("Order Type", t).show(ui, t, |ui| {
            const ORDER_TYPES: &[(usize, &str)] = &[(0, "MKT"), (1, "LMT"), (2, "STP")];
            let before = watchlist.default_order_type;
            SegmentedControl::new(&mut watchlist.default_order_type, ORDER_TYPES).show(ui, t);
            if watchlist.default_order_type != before { trading_changed = true; }
        });
        setting_form_row("Time in Force", t).show(ui, t, |ui| {
            const TIF_OPTS: &[(usize, &str)] = &[(0, "DAY"), (1, "GTC"), (2, "IOC")];
            let before = watchlist.default_tif;
            SegmentedControl::new(&mut watchlist.default_tif, TIF_OPTS).show(ui, t);
            if watchlist.default_tif != before { trading_changed = true; }
        });
        let before_rth = watchlist.default_outside_rth;
        setting_toggle(ui, "Outside RTH", t, &mut watchlist.default_outside_rth);
        if watchlist.default_outside_rth != before_rth { trading_changed = true; }
    });
    if trading_changed { watchlist.push_to_trading_defaults_store(); }

    // ── RISK MANAGEMENT ──
    PanelSection::new("RISK MANAGEMENT").show(ui, t, |ui, t| {
        use crate::chart_renderer::trading::order_manager;
        let mut limits = order_manager::get_risk_limits();
        setting_form_row("Max Order Qty", t).show(ui, t, |ui| {
            let mut v = limits.max_order_qty as i32;
            if NumberStepper::new(&mut v).range(1..=100_000).step(10.0).integer().show(ui, t).changed() {
                limits.max_order_qty = v.max(1) as u32;
            }
        });
        setting_form_row("Max Position", t).show(ui, t, |ui| {
            let mut v = limits.max_position_qty as i32;
            if NumberStepper::new(&mut v).range(1..=500_000).step(100.0).integer().show(ui, t).changed() {
                limits.max_position_qty = v.max(1) as u32;
            }
        });
        setting_form_row("Max Notional $", t).show(ui, t, |ui| {
            let mut v = limits.max_notional as i64;
            if ui.add(egui::DragValue::new(&mut v).range(0..=10_000_000).speed(1000)
                .custom_formatter(|v, _| if v as i64 == 0 { "OFF".into() } else { format!("${}", v as i64) })).changed() {
                limits.max_notional = v.max(0) as f64;
            }
        });
        setting_form_row("Fat Finger %", t).show(ui, t, |ui| {
            let mut v = limits.fat_finger_pct;
            if ui.add(egui::DragValue::new(&mut v).range(0.0..=50.0).speed(0.5).suffix("%")
                .custom_formatter(|v, _| if v < 0.1 { "OFF".into() } else { format!("{:.1}%", v) })).changed() {
                limits.fat_finger_pct = v.max(0.0);
            }
        });
        setting_form_row("Max Open Orders", t).show(ui, t, |ui| {
            let mut v = limits.max_open_orders as i32;
            if crate::ui_kit::widgets::Slider::new(&mut v, 1..=1000)
                .step(1.0)
                .show_value(true)
                .show(ui, t)
                .changed() {
                limits.max_open_orders = v.max(1) as usize;
            }
        });
        setting_form_row("Max Daily Loss $", t).show(ui, t, |ui| {
            let mut v = limits.max_daily_loss as i64;
            if ui.add(egui::DragValue::new(&mut v).range(0..=1_000_000).speed(500)
                .custom_formatter(|v, _| if v as i64 == 0 { "OFF".into() } else { format!("${}", v as i64) })).changed() {
                limits.max_daily_loss = v.max(0) as f64;
            }
        });
        setting_form_row("Dedup Cooldown", t).show(ui, t, |ui| {
            let mut v = limits.dedup_cooldown_ms as i32;
            if crate::ui_kit::widgets::Slider::new(&mut v, 100..=5000)
                .step(50.0)
                .show_value(true)
                .show(ui, t)
                .changed() {
                limits.dedup_cooldown_ms = v.max(100) as u64;
            }
        });
        order_manager::update_risk_limits(limits);
    });

    // ── APEX DATA ──
    PanelSection::new("APEX DATA").show(ui, t, |ui, t| {
        let mut enabled = crate::apex_data::is_enabled();
        let prev = enabled;
        setting_toggle_described(ui, "Enabled",
            Some("Stream live market data from the ApexData feed."),
            t, &mut enabled);
        if enabled != prev {
            crate::apex_data::set_enabled(enabled);
            if enabled { crate::apex_data::ws::start(); }
        }

        setting_form_row("Base URL", t)
            .show_with_cx(ui, t, |ui, _cx| {
                let id = egui::Id::new("apex_data_url_edit");
                let mut buf: String = ui.data_mut(|d|
                    d.get_temp::<String>(id).unwrap_or_else(|| crate::apex_data::apex_url()));
                let resp = crate::ui_kit::widgets::Input::new(&mut buf)
                    .min_width(340.0)
                    .show(ui, t);
                if resp.response.changed() { ui.data_mut(|d| d.insert_temp(id, buf.clone())); }
                if resp.submitted {
                    crate::apex_data::set_apex_url(buf.trim().to_string());
                }
            });

        setting_form_row("Auth Token", t)
            .password(true)
            .hint("optional — leave blank if no token required")
            .show_with_cx(ui, t, |ui, cx| {
                let id = egui::Id::new("apex_data_token_edit");
                let mut buf: String = ui.data_mut(|d|
                    d.get_temp::<String>(id).unwrap_or_else(|| crate::apex_data::apex_token().unwrap_or_default()));
                let mut input = crate::ui_kit::widgets::Input::new(&mut buf).min_width(340.0);
                if cx.password { input = input.password(true); }
                if let Some(h) = cx.hint { input = input.placeholder(h); }
                let resp = input.show(ui, t);
                if resp.response.changed() { ui.data_mut(|d| d.insert_temp(id, buf.clone())); }
                if resp.submitted {
                    let tok = buf.trim();
                    crate::apex_data::set_apex_token(if tok.is_empty() { None } else { Some(tok.to_string()) });
                }
            });

        let ws_connected = crate::apex_data::ws::is_connected();
        let (state_label, state_col) = if ws_connected {
            ("WS connected", t.bull)
        } else {
            ("WS disconnected", t.bear)
        };
        ui.add(SectionLabel::new(state_label).tiny().color(state_col));
    });
}

// ═══════════════════════════════════════════════════════════════
// SHORTCUTS TAB
// ═══════════════════════════════════════════════════════════════
fn draw_shortcuts(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    // Column header (uses the body inset, no per-row indent).
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(220.0, 16.0), |ui| {
            ui.add(SectionLabel::new("ACTION").tiny().color(color_dim(t.dim)));
        });
        ui.add(SectionLabel::new("SHORTCUT").tiny().color(color_dim(t.dim)));
    });
    separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
    ui.add_space(gap_xs());

    super::super::tools::hotkey_editor::draw_content(ui, watchlist, t);
}

// ─── Helpers: standard setting toggles ─────────────────────────

fn setting_toggle(ui: &mut egui::Ui, label: &str, t: &Theme, val: &mut bool) {
    setting_toggle_described(ui, label, None, t, val);
}

/// ToggleRow wrapper with optional description. No per-row indent: the
/// outer body Frame already provides the consistent left inset.
fn setting_toggle_described(
    ui: &mut egui::Ui,
    label: &str,
    description: Option<&str>,
    t: &Theme,
    val: &mut bool,
) {
    let mut row = ToggleRow::new(val).label(label);
    if let Some(desc) = description {
        row = row.description(desc);
    }
    row.show(ui, t);
    ui.add_space(gap_xs());
}
