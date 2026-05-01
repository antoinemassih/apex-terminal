//! Plays panel — shareable strategy cards (playbook).
//! Shows in the Feed sidebar under the "Plays" tab.
//! Each play is a polished visual card with shadow, bevel, and rich layout.
//! The editor spawns draggable play lines on the chart, synced bidirectionally.

use egui;
use super::style::*;
use super::super::gpu::*;
use super::widgets::buttons::ChromeBtn;
use crate::chart_renderer::{Play, PlayDirection, PlayStatus, PlayType, PlayLine, PlayLineKind, PlayTarget};
use crate::ui_kit::icons::Icon;

const TAG_PRESETS: &[&str] = &["momentum", "breakout", "earnings", "scalp", "swing", "mean-rev", "gap", "squeeze"];

/// Draw the plays tab content inside the Feed sidebar.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    ui.add_space(GAP_SM);

    // ── Header with "New Play" button ──
    ui.horizontal(|ui| {
        section_label(ui, &format!("PLAYBOOK ({})", watchlist.plays.len()), t.accent);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if small_action_btn(ui, "+ New Play", t.accent) {
                watchlist.play_editor_open = !watchlist.play_editor_open;
                if watchlist.play_editor_open && !panes.is_empty() {
                    spawn_play_lines(watchlist, &mut panes[ap]);
                } else if !watchlist.play_editor_open && !panes.is_empty() {
                    panes[ap].play_lines.clear();
                    panes[ap].play_click_to_set = None;
                }
            }
        });
    });
    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Play editor (inline) ──
    if watchlist.play_editor_open {
        let chart = if !panes.is_empty() { Some(&mut panes[ap]) } else { None };
        draw_play_editor(ui, watchlist, chart, t);
        ui.add_space(GAP_SM);
        separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
        ui.add_space(GAP_SM);
    }

    // ── Play cards ──
    if watchlist.plays.is_empty() {
        ui.add_space(GAP_3XL);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(Icon::STAR).size(28.0).color(t.dim.gamma_multiply(0.2)));
            ui.add_space(GAP_SM);
            ui.add(super::widgets::text::MonospaceCode::new("No plays yet").sm().color(t.dim).gamma(0.5));
            ui.add(super::widgets::text::MonospaceCode::new("Create a play to share a trade idea").xs().color(t.dim).gamma(0.3));
        });
        return;
    }

    let mut remove_id: Option<String> = None;
    let mut activate_id: Option<String> = None;
    let mut display_id: Option<String> = None;

    egui::ScrollArea::vertical().id_salt("plays_scroll").show(ui, |ui| {
        for play in &watchlist.plays {
            draw_play_card(ui, play, t, &mut remove_id, &mut activate_id, &mut display_id);
        }
    });

    if let Some(id) = remove_id {
        watchlist.plays.retain(|p| p.id != id);
    }

    if let Some(id) = activate_id {
        if let Some(play) = watchlist.plays.iter_mut().find(|p| p.id == id) {
            play.status = PlayStatus::Active;
            if !panes.is_empty() {
                convert_play_to_orders(play, &mut panes[ap]);
            }
        }
    }

    // Display play on chart — spawn play lines from saved play data
    if let Some(id) = display_id {
        if let Some(play) = watchlist.plays.iter().find(|p| p.id == id) {
            if !panes.is_empty() {
                let chart = &mut panes[ap];
                chart.play_lines.clear();
                let mut lid = chart.next_play_line_id;

                chart.play_lines.push(PlayLine { id: lid, kind: PlayLineKind::Entry, price: play.entry_price });
                lid += 1;
                chart.play_lines.push(PlayLine { id: lid, kind: PlayLineKind::Target, price: play.target_price });
                lid += 1;

                if play.play_type != PlayType::Scalp && play.stop_price > 0.0 {
                    chart.play_lines.push(PlayLine { id: lid, kind: PlayLineKind::Stop, price: play.stop_price });
                    lid += 1;
                }

                // Additional targets from the play
                for tgt in &play.targets {
                    let kind = match tgt.label.as_str() {
                        "T2" => PlayLineKind::Target2,
                        "T3" => PlayLineKind::Target3,
                        _ => continue,
                    };
                    chart.play_lines.push(PlayLine { id: lid, kind, price: tgt.price });
                    lid += 1;
                }

                chart.next_play_line_id = lid;
            }
        }
    }
}

/// Spawn play lines on the chart at sensible default prices based on play type.
fn spawn_play_lines(watchlist: &mut Watchlist, chart: &mut Chart) {
    chart.play_lines.clear();
    let last = chart.bars.last().map(|b| b.close).unwrap_or(100.0);
    let is_long = watchlist.play_editor_direction == PlayDirection::Long;
    let sign = if is_long { 1.0 } else { -1.0 };

    let entry = last;
    let target = last * (1.0 + sign * 0.02);
    let stop = last * (1.0 - sign * 0.02);

    watchlist.play_editor_symbol = chart.symbol.clone();
    watchlist.play_editor_entry = format!("{:.2}", entry);
    watchlist.play_editor_target = format!("{:.2}", target);
    watchlist.play_editor_stop = format!("{:.2}", stop);

    let id = chart.next_play_line_id;
    chart.play_lines.push(PlayLine { id, kind: PlayLineKind::Entry, price: entry });
    chart.play_lines.push(PlayLine { id: id + 1, kind: PlayLineKind::Target, price: target });

    let pt = watchlist.play_editor_type;
    let has_stop = pt != PlayType::Scalp;
    if has_stop {
        chart.play_lines.push(PlayLine { id: id + 2, kind: PlayLineKind::Stop, price: stop });
    }

    // Swing starts with T2 by default
    if pt == PlayType::Swing {
        let t2 = last * (1.0 + sign * 0.04);
        watchlist.play_editor_has_t2 = true;
        watchlist.play_editor_t2 = format!("{:.2}", t2);
        watchlist.play_editor_t2_pct = "33".into();
        chart.play_lines.push(PlayLine { id: id + 3, kind: PlayLineKind::Target2, price: t2 });
        // Set T1 allocation
        watchlist.play_editor_target_pct = "50".into();
        chart.next_play_line_id = id + 4;
    } else {
        watchlist.play_editor_has_t2 = false;
        watchlist.play_editor_has_t3 = false;
        watchlist.play_editor_target_pct = "100".into();
        chart.next_play_line_id = id + 3;
    }
}

/// The play editor form.
fn draw_play_editor(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    mut chart: Option<&mut Chart>,
    t: &Theme,
) {
    // ── Sync play lines -> form fields (lines are source of truth while dragging) ──
    if let Some(ref chart) = chart {
        for pl in &chart.play_lines {
            let field = match pl.kind {
                PlayLineKind::Entry   => &mut watchlist.play_editor_entry,
                PlayLineKind::Target  => &mut watchlist.play_editor_target,
                PlayLineKind::Stop    => &mut watchlist.play_editor_stop,
                PlayLineKind::Target2 => &mut watchlist.play_editor_t2,
                PlayLineKind::Target3 => &mut watchlist.play_editor_t3,
            };
            let field_id = egui::Id::new(("play_price", pl.kind as u8));
            if !ui.memory(|m| m.has_focus(field_id)) {
                *field = format!("{:.2}", pl.price);
            }
        }
    }

    let pt = watchlist.play_editor_type;

    egui::Frame::NONE
        .fill(color_alpha(t.toolbar_border, ALPHA_FAINT))
        .inner_margin(egui::Margin::same(GAP_LG as i8))
        .corner_radius(r_lg_cr())
        .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_MUTED)))
        .show(ui, |ui| {
            ui.add(super::widgets::text::SectionLabel::new("NEW PLAY").color(t.accent));
            ui.add_space(GAP_XS);

            // ── Play type selector ──
            ui.horizontal_wrapped(|ui| {
                for pty in PlayType::all() {
                    let sel = pt == *pty;
                    let fg = if sel { t.accent } else { t.dim.gamma_multiply(0.5) };
                    let bg = if sel { color_alpha(t.accent, ALPHA_TINT) } else { egui::Color32::TRANSPARENT };
                    let label = format!("{} {}", pty.icon(), pty.label());
                    if ui.add(ChromeBtn::new(egui::RichText::new(&label).monospace().size(FONT_XS).color(fg))
                        .fill(bg).corner_radius(r_sm_cr())
                        .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(t.accent, ALPHA_LINE) } else { egui::Stroke::NONE.color }))
                        .min_size(egui::vec2(0.0, 20.0))).clicked() {
                        let prev = watchlist.play_editor_type;
                        watchlist.play_editor_type = *pty;
                        if prev != *pty {
                            if let Some(ref mut c) = chart { spawn_play_lines(watchlist, c); }
                        }
                    }
                }
            });

            // Type description
            let desc = match pt {
                PlayType::Directional => "Entry + target + stop",
                PlayType::Bracket     => "Entry + TP + SL bracket (auto-linked)",
                PlayType::Scalp       => "Entry + target only, no stop",
                PlayType::Swing       => "Multiple partial-exit targets",
                PlayType::Spread      => "Multi-leg strategy",
                PlayType::Event       => "Catalyst-driven with pre/post levels",
            };
            ui.add(super::widgets::text::MonospaceCode::new(desc).xs().color(t.dim).gamma(0.4));
            ui.add_space(GAP_XS);

            // ── Direction toggle ──
            ui.horizontal(|ui| {
                for (dir, label, color) in [
                    (PlayDirection::Long, "LONG", t.bull),
                    (PlayDirection::Short, "SHORT", t.bear),
                ] {
                    let sel = watchlist.play_editor_direction == dir;
                    let fg = if sel { color } else { t.dim.gamma_multiply(0.5) };
                    let bg = if sel { color_alpha(color, ALPHA_TINT) } else { egui::Color32::TRANSPARENT };
                    if ui.add(ChromeBtn::new(egui::RichText::new(label).monospace().size(FONT_SM).strong().color(fg))
                        .fill(bg).corner_radius(r_sm_cr())
                        .stroke(egui::Stroke::new(STROKE_THIN, if sel { color_alpha(color, ALPHA_LINE) } else { egui::Stroke::NONE.color }))
                        .min_size(egui::vec2(56.0, 22.0))).clicked() {
                        watchlist.play_editor_direction = dir;
                        if let Some(ref mut c) = chart { spawn_play_lines(watchlist, c); }
                    }
                }
            });
            ui.add_space(GAP_XS);

            // ── Symbol ──
            ui.horizontal(|ui| {
                dim_label(ui, "Symbol", t.dim);
                super::widgets::inputs::TextInput::new(&mut watchlist.play_editor_symbol)
                    .width(80.0).font_size(FONT_SM).placeholder("AAPL").show(ui);
            });
            ui.add_space(GAP_XS);

            // ── Price fields with click-to-set ──
            let crosshair = "\u{2295}"; // ⊕

            // Entry
            ui.horizontal(|ui| {
                dim_label(ui, "Entry", t.dim);
                let resp = ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_entry)
                    .id(egui::Id::new(("play_price", PlayLineKind::Entry as u8)))
                    .desired_width(70.0).font(egui::FontId::monospace(FONT_SM)).hint_text("150.00"));
                if resp.lost_focus() { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Entry))) {
                    if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Entry); }
                }
            });

            // ── Legs section ──
            ui.add_space(GAP_XS);
            separator(ui, color_alpha(t.toolbar_border, ALPHA_FAINT));
            ui.add_space(GAP_XS);

            ui.horizontal(|ui| {
                section_label(ui, "LEGS", t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !watchlist.play_editor_has_t2 && pt != PlayType::Scalp {
                        if small_action_btn(ui, "+ Add Target", t.accent) {
                            add_target_line(watchlist, chart.as_deref_mut(), PlayLineKind::Target2);
                        }
                    } else if watchlist.play_editor_has_t2 && !watchlist.play_editor_has_t3 && pt != PlayType::Scalp {
                        if small_action_btn(ui, "+ Add T3", t.accent) {
                            add_target_line(watchlist, chart.as_deref_mut(), PlayLineKind::Target3);
                        }
                    }
                });
            });
            ui.add_space(GAP_XS);

            // T1 — primary target with allocation
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("T1").monospace().size(7.0).strong().color(t.bull.gamma_multiply(0.7)));
                let resp = ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_target)
                    .id(egui::Id::new(("play_price", PlayLineKind::Target as u8)))
                    .desired_width(65.0).font(egui::FontId::monospace(FONT_SM)));
                if resp.lost_focus() { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Target))) {
                    if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Target); }
                }
                pct_stepper(ui, &mut watchlist.play_editor_target_pct, t);
            });

            // T2
            if watchlist.play_editor_has_t2 {
                let mut remove_t2 = false;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("T2").monospace().size(7.0).strong().color(egui::Color32::from_rgb(26, 188, 156)));
                    let resp = ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_t2)
                        .id(egui::Id::new(("play_price", PlayLineKind::Target2 as u8)))
                        .desired_width(65.0).font(egui::FontId::monospace(FONT_SM)));
                    if resp.lost_focus() { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                    if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Target2))) {
                        if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Target2); }
                    }
                    pct_stepper(ui, &mut watchlist.play_editor_t2_pct, t);
                    if ui.add(ChromeBtn::new(egui::RichText::new("\u{00D7}").size(FONT_SM).color(t.bear.gamma_multiply(0.6)))
                        .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(16.0, 16.0))).clicked() {
                        remove_t2 = true;
                    }
                });
                if remove_t2 {
                    watchlist.play_editor_has_t2 = false;
                    watchlist.play_editor_has_t3 = false;
                    if let Some(ref mut c) = chart {
                        c.play_lines.retain(|l| l.kind != PlayLineKind::Target2 && l.kind != PlayLineKind::Target3);
                    }
                }
            }

            // T3
            if watchlist.play_editor_has_t3 {
                let mut remove_t3 = false;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("T3").monospace().size(7.0).strong().color(egui::Color32::from_rgb(52, 152, 219)));
                    let resp = ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_t3)
                        .id(egui::Id::new(("play_price", PlayLineKind::Target3 as u8)))
                        .desired_width(65.0).font(egui::FontId::monospace(FONT_SM)));
                    if resp.lost_focus() { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                    if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Target3))) {
                        if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Target3); }
                    }
                    pct_stepper(ui, &mut watchlist.play_editor_t3_pct, t);
                    if ui.add(ChromeBtn::new(egui::RichText::new("\u{00D7}").size(FONT_SM).color(t.bear.gamma_multiply(0.6)))
                        .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(16.0, 16.0))).clicked() {
                        remove_t3 = true;
                    }
                });
                if remove_t3 {
                    watchlist.play_editor_has_t3 = false;
                    if let Some(ref mut c) = chart {
                        c.play_lines.retain(|l| l.kind != PlayLineKind::Target3);
                    }
                }
            }

            // Stop row (hidden for Scalp)
            if pt != PlayType::Scalp {
                ui.add_space(GAP_XS);
                ui.horizontal(|ui| {
                    let stop_label = egui::RichText::new("STOP").monospace().size(7.0).color(t.bear.gamma_multiply(0.7));
                    ui.label(stop_label);
                    let resp = ui.add(egui::TextEdit::singleline(&mut watchlist.play_editor_stop)
                        .id(egui::Id::new(("play_price", PlayLineKind::Stop as u8)))
                        .desired_width(70.0).font(egui::FontId::monospace(FONT_SM)).hint_text("148.00"));
                    if resp.lost_focus() { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                    if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Stop))) {
                        if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Stop); }
                    }
                });
            }

            // ── R:R display ──
            let entry_f = watchlist.play_editor_entry.parse::<f32>().unwrap_or(0.0);
            let target_f = watchlist.play_editor_target.parse::<f32>().unwrap_or(0.0);
            let stop_f = watchlist.play_editor_stop.parse::<f32>().unwrap_or(0.0);
            let risk = (entry_f - stop_f).abs();
            let reward = (target_f - entry_f).abs();
            if risk > 0.001 && pt != PlayType::Scalp {
                let rr = reward / risk;
                ui.add_space(GAP_XS);
                ui.horizontal(|ui| {
                    dim_label(ui, "R:R", t.dim);
                    let rr_col = if rr >= 2.0 { t.bull } else if rr >= 1.0 { egui::Color32::from_rgb(255, 191, 0) } else { t.bear };
                    ui.add(super::widgets::text::MonospaceCode::new(&format!("{:.1} : 1", rr)).sm().color(rr_col).strong(true));
                    let bar_w = ui.available_width().min(120.0);
                    let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 6.0), egui::Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(bar_rect, 2.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
                    let risk_pct = (risk / (risk + reward)).min(1.0);
                    p.rect_filled(egui::Rect::from_min_size(bar_rect.min, egui::vec2(bar_w * risk_pct, 6.0)), 2.0, color_alpha(t.bear, ALPHA_DIM));
                    p.rect_filled(egui::Rect::from_min_size(
                        egui::pos2(bar_rect.left() + bar_w * risk_pct, bar_rect.top()),
                        egui::vec2(bar_w * (1.0 - risk_pct), 6.0)), 2.0, color_alpha(t.bull, ALPHA_DIM));
                    p.circle_filled(egui::pos2(bar_rect.left() + bar_w * risk_pct, bar_rect.center().y), 3.0, t.text);
                });
            }

            ui.add_space(GAP_XS);
            separator(ui, color_alpha(t.toolbar_border, ALPHA_FAINT));
            ui.add_space(GAP_XS);

            // ── Tag chips + custom input ──
            ui.horizontal_wrapped(|ui| {
                for tag in TAG_PRESETS {
                    let active = watchlist.play_editor_tags.iter().any(|t| t == tag);
                    let fg = if active { t.accent } else { t.dim.gamma_multiply(0.4) };
                    let bg = if active { color_alpha(t.accent, ALPHA_TINT) } else { egui::Color32::TRANSPARENT };
                    if ui.add(ChromeBtn::new(egui::RichText::new(*tag).monospace().size(7.0).color(fg))
                        .fill(bg).corner_radius(r_md_cr())
                        .stroke(egui::Stroke::new(0.5, if active { color_alpha(t.accent, ALPHA_LINE) } else { color_alpha(t.toolbar_border, ALPHA_MUTED) }))
                        .min_size(egui::vec2(0.0, 16.0))).clicked() {
                        if active { watchlist.play_editor_tags.retain(|x| x != tag); }
                        else { watchlist.play_editor_tags.push(tag.to_string()); }
                    }
                }
            });
            // Custom tag input
            ui.horizontal(|ui| {
                dim_label(ui, "+", t.dim);
                let resp = super::widgets::inputs::TextInput::new(&mut watchlist.play_editor_custom_tag)
                    .width(80.0).font_size(FONT_XS).placeholder("custom tag").show(ui);
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let tag = watchlist.play_editor_custom_tag.trim().to_lowercase();
                    if !tag.is_empty() && !watchlist.play_editor_tags.contains(&tag) {
                        watchlist.play_editor_tags.push(tag);
                    }
                    watchlist.play_editor_custom_tag.clear();
                }
                // Show active custom tags (non-preset) as removable chips
                let custom: Vec<String> = watchlist.play_editor_tags.iter()
                    .filter(|tg| !TAG_PRESETS.contains(&tg.as_str()))
                    .cloned().collect();
                for ct in &custom {
                    if ui.add(ChromeBtn::new(egui::RichText::new(format!("{} \u{00D7}", ct)).monospace().size(7.0).color(t.accent))
                        .fill(color_alpha(t.accent, ALPHA_TINT)).corner_radius(r_md_cr())
                        .min_size(egui::vec2(0.0, 16.0))).clicked() {
                        watchlist.play_editor_tags.retain(|x| x != ct);
                    }
                }
            });
            ui.add_space(GAP_XS);

            // ── Notes ──
            ui.add(egui::TextEdit::multiline(&mut watchlist.play_editor_notes)
                .desired_rows(2).desired_width(ui.available_width())
                .font(egui::FontId::monospace(FONT_SM)).hint_text("Strategy notes..."));
            ui.add_space(GAP_SM);

            // ── Buttons ──
            ui.horizontal(|ui| {
                let can_create = !watchlist.play_editor_symbol.trim().is_empty()
                    && watchlist.play_editor_entry.parse::<f32>().is_ok();

                if action_btn(ui, "Create Play", t.accent, can_create) {
                    let entry = watchlist.play_editor_entry.parse::<f32>().unwrap_or(0.0);
                    let target = watchlist.play_editor_target.parse::<f32>().unwrap_or(entry * 1.02);
                    let stop = watchlist.play_editor_stop.parse::<f32>().unwrap_or(entry * 0.98);
                    let mut play = Play::new(
                        watchlist.play_editor_symbol.trim(),
                        watchlist.play_editor_direction, watchlist.play_editor_type,
                        entry, target, stop);
                    play.notes = watchlist.play_editor_notes.trim().to_string();
                    play.quantity = 1; // not important anymore
                    play.tags = watchlist.play_editor_tags.clone();

                    // Add target allocations
                    let t1_pct = watchlist.play_editor_target_pct.parse::<f32>().unwrap_or(100.0) / 100.0;
                    play.targets.push(PlayTarget { price: target, pct: t1_pct, label: "T1".into() });

                    if watchlist.play_editor_has_t2 {
                        if let Ok(p) = watchlist.play_editor_t2.parse::<f32>() {
                            let pct = watchlist.play_editor_t2_pct.parse::<f32>().unwrap_or(25.0) / 100.0;
                            play.targets.push(PlayTarget { price: p, pct, label: "T2".into() });
                        }
                    }
                    if watchlist.play_editor_has_t3 {
                        if let Ok(p) = watchlist.play_editor_t3.parse::<f32>() {
                            let pct = watchlist.play_editor_t3_pct.parse::<f32>().unwrap_or(25.0) / 100.0;
                            play.targets.push(PlayTarget { price: p, pct, label: "T3".into() });
                        }
                    }

                    watchlist.plays.push(play);
                    clear_editor(watchlist);
                    if let Some(ref mut c) = chart {
                        c.play_lines.clear();
                        c.play_click_to_set = None;
                    }
                }
                if small_action_btn(ui, "Cancel", t.dim) {
                    clear_editor(watchlist);
                    if let Some(ref mut c) = chart {
                        c.play_lines.clear();
                        c.play_click_to_set = None;
                    }
                }
            });
        });
}

fn add_target_line(watchlist: &mut Watchlist, chart: Option<&mut Chart>, kind: PlayLineKind) {
    let entry = watchlist.play_editor_entry.parse::<f32>().unwrap_or(100.0);
    let target = watchlist.play_editor_target.parse::<f32>().unwrap_or(entry * 1.02);
    let is_long = watchlist.play_editor_direction == PlayDirection::Long;
    let sign: f32 = if is_long { 1.0 } else { -1.0 };

    match kind {
        PlayLineKind::Target2 => {
            watchlist.play_editor_has_t2 = true;
            let t2 = entry + (target - entry) * 1.5 * sign;
            watchlist.play_editor_t2 = format!("{:.2}", t2);
            watchlist.play_editor_t2_pct = "33".into();
            // Adjust T1 allocation
            let t1 = watchlist.play_editor_target_pct.parse::<f32>().unwrap_or(100.0);
            if t1 > 50.0 { watchlist.play_editor_target_pct = "50".into(); }
            if let Some(c) = chart {
                let id = c.next_play_line_id;
                c.play_lines.push(PlayLine { id, kind: PlayLineKind::Target2, price: t2 });
                c.next_play_line_id += 1;
            }
        }
        PlayLineKind::Target3 => {
            watchlist.play_editor_has_t3 = true;
            let t2 = watchlist.play_editor_t2.parse::<f32>().unwrap_or(target * 1.5);
            let t3 = entry + (t2 - entry) * 1.5 * sign;
            watchlist.play_editor_t3 = format!("{:.2}", t3);
            watchlist.play_editor_t3_pct = "17".into();
            if let Some(c) = chart {
                let id = c.next_play_line_id;
                c.play_lines.push(PlayLine { id, kind: PlayLineKind::Target3, price: t3 });
                c.next_play_line_id += 1;
            }
        }
        _ => {}
    }
}

/// Compact percentage stepper: [-] value% [+]
fn pct_stepper(ui: &mut egui::Ui, pct_str: &mut String, t: &Theme) {
    let step = 5u32;
    let mut val: u32 = pct_str.parse().unwrap_or(50);

    if ui.add(ChromeBtn::new(egui::RichText::new("-").monospace().size(FONT_XS).color(t.dim))
        .min_size(egui::vec2(16.0, 16.0)).fill(color_alpha(t.toolbar_border, ALPHA_FAINT))).clicked() {
        val = val.saturating_sub(step).max(5);
    }
    ui.add(egui::TextEdit::singleline(pct_str)
        .desired_width(22.0).font(egui::FontId::monospace(FONT_XS))
        .horizontal_align(egui::Align::Center));
    ui.add(super::widgets::text::MonospaceCode::new("%").xs().color(t.dim).gamma(0.4));
    if ui.add(ChromeBtn::new(egui::RichText::new("+").monospace().size(FONT_XS).color(t.dim))
        .min_size(egui::vec2(16.0, 16.0)).fill(color_alpha(t.toolbar_border, ALPHA_FAINT))).clicked() {
        val = (val + step).min(100);
    }

    let new_val: u32 = pct_str.parse().unwrap_or(val);
    if new_val != val {
        // User typed a new value
    } else {
        *pct_str = val.to_string();
    }
}

fn click_to_set_btn(ui: &mut egui::Ui, icon: &str, t: &Theme, active: bool) -> bool {
    let fg = if active { t.accent } else { t.dim.gamma_multiply(0.4) };
    let bg = if active { color_alpha(t.accent, ALPHA_TINT) } else { egui::Color32::TRANSPARENT };
    ui.add(ChromeBtn::new(egui::RichText::new(icon).size(FONT_SM).color(fg))
        .fill(bg).corner_radius(r_sm_cr())
        .min_size(egui::vec2(18.0, 18.0))).clicked()
}

fn sync_form_to_lines(watchlist: &Watchlist, chart: Option<&mut Chart>) {
    let Some(chart) = chart else { return; };
    let fields: &[(PlayLineKind, &str)] = &[
        (PlayLineKind::Entry, &watchlist.play_editor_entry),
        (PlayLineKind::Target, &watchlist.play_editor_target),
        (PlayLineKind::Stop, &watchlist.play_editor_stop),
        (PlayLineKind::Target2, &watchlist.play_editor_t2),
        (PlayLineKind::Target3, &watchlist.play_editor_t3),
    ];
    for (kind, val) in fields {
        if let Ok(price) = val.parse::<f32>() {
            if let Some(pl) = chart.play_lines.iter_mut().find(|l| l.kind == *kind) {
                pl.price = price;
            }
        }
    }
}

fn clear_editor(watchlist: &mut Watchlist) {
    watchlist.play_editor_open = false;
    watchlist.play_editor_symbol.clear();
    watchlist.play_editor_entry.clear();
    watchlist.play_editor_target.clear();
    watchlist.play_editor_stop.clear();
    watchlist.play_editor_notes.clear();
    watchlist.play_editor_tags.clear();
    watchlist.play_editor_has_t2 = false;
    watchlist.play_editor_has_t3 = false;
    watchlist.play_editor_t2.clear();
    watchlist.play_editor_t3.clear();
    watchlist.play_editor_custom_tag.clear();
    watchlist.play_editor_target_pct = "100".into();
}

fn convert_play_to_orders(play: &Play, chart: &mut Chart) {
    use crate::chart_renderer::trading::{OrderLevel, OrderSide, OrderStatus};

    let entry_side = if play.direction == PlayDirection::Long { OrderSide::Buy } else { OrderSide::Sell };
    let next_id = chart.orders.iter().map(|o| o.id).max().unwrap_or(0) + 1;

    chart.orders.push(OrderLevel {
        id: next_id, side: entry_side, price: play.entry_price,
        qty: play.quantity, status: OrderStatus::Draft, pair_id: None,
        option_symbol: None, option_con_id: None,
        trail_amount: None, trail_percent: None,
    });

    // Create OCO pairs for each target + stop
    if play.play_type != PlayType::Scalp && play.stop_price > 0.0 {
        let target_id = next_id + 1;
        let stop_id = next_id + 2;
        chart.orders.push(OrderLevel {
            id: target_id, side: OrderSide::OcoTarget, price: play.target_price,
            qty: play.quantity, status: OrderStatus::Draft, pair_id: Some(stop_id),
            option_symbol: None, option_con_id: None,
            trail_amount: None, trail_percent: None,
        });
        chart.orders.push(OrderLevel {
            id: stop_id, side: OrderSide::OcoStop, price: play.stop_price,
            qty: play.quantity, status: OrderStatus::Draft, pair_id: Some(target_id),
            option_symbol: None, option_con_id: None,
            trail_amount: None, trail_percent: None,
        });
    } else {
        // Scalp — target only
        chart.orders.push(OrderLevel {
            id: next_id + 1, side: OrderSide::OcoTarget, price: play.target_price,
            qty: play.quantity, status: OrderStatus::Draft, pair_id: None,
            option_symbol: None, option_con_id: None,
            trail_amount: None, trail_percent: None,
        });
    }
}

/// A polished play card with shadow, direction stripe, and rich layout.
///
/// Wave 5: thin delegation to the `PlayCard` widget in the cards system.
/// All visuals + action wiring live in `widgets::cards::play_card`.
fn draw_play_card(ui: &mut egui::Ui, play: &Play, t: &Theme, remove_id: &mut Option<String>, activate_id: &mut Option<String>, display_id: &mut Option<String>) {
    use super::widgets::cards::PlayCard;
    let r = PlayCard::new(play, t).show(ui);
    if r.delete_clicked   { *remove_id   = Some(play.id.clone()); }
    if r.activate_clicked { *activate_id = Some(play.id.clone()); }
    if r.clicked          { *display_id  = Some(play.id.clone()); }
}

#[allow(dead_code)]
#[cfg(any())]
fn _draw_play_card_legacy(ui: &mut egui::Ui, play: &Play, t: &Theme, remove_id: &mut Option<String>, activate_id: &mut Option<String>, display_id: &mut Option<String>) {
    let is_long = play.direction == PlayDirection::Long;
    let dir_color = if is_long { t.bull } else { t.bear };
    let card_w = ui.available_width();
    let has_tags = !play.tags.is_empty();
    let has_targets = play.targets.len() > 1;
    let card_h = 76.0
        + if !play.notes.is_empty() { 14.0 } else { 0.0 }
        + if has_tags { 14.0 } else { 0.0 }
        + if has_targets { play.targets.len() as f32 * 10.0 } else { 0.0 };

    let (card_rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
    let p = ui.painter();

    // Shadow
    p.rect_filled(card_rect.translate(egui::vec2(0.0, 2.0)).expand(1.0), RADIUS_LG, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25));
    p.rect_filled(card_rect.translate(egui::vec2(0.0, 1.0)), RADIUS_LG, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 15));

    let bg = if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        egui::Color32::from_rgba_unmultiplied(
            t.toolbar_border.r().saturating_add(15), t.toolbar_border.g().saturating_add(15),
            t.toolbar_border.b().saturating_add(15), 255)
    } else {
        egui::Color32::from_rgb(t.toolbar_bg.r().saturating_add(8), t.toolbar_bg.g().saturating_add(8), t.toolbar_bg.b().saturating_add(8))
    };
    p.rect_filled(card_rect, RADIUS_LG, bg);

    // Top bevel highlight
    p.rect_filled(egui::Rect::from_min_max(card_rect.min, egui::pos2(card_rect.right(), card_rect.top() + 1.0)),
        egui::CornerRadius { nw: RADIUS_LG as u8, ne: RADIUS_LG as u8, sw: 0, se: 0 },
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, if t.is_light() { 40 } else { 8 }));

    p.rect_stroke(card_rect, RADIUS_LG, egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_STRONG)), egui::StrokeKind::Outside);

    // Accent stripe
    p.rect_filled(egui::Rect::from_min_max(
        egui::pos2(card_rect.left(), card_rect.top() + 4.0),
        egui::pos2(card_rect.left() + 3.0, card_rect.bottom() - 4.0)),
        egui::CornerRadius { nw: 2, sw: 2, ne: 0, se: 0 }, dir_color);

    let cx = card_rect.left() + 12.0;
    let mut cy = card_rect.top() + 10.0;

    // Row 1: Direction pill + type icon + symbol + status + R:R
    {
        let pill_w = 42.0;
        let pill_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 1.0), egui::vec2(pill_w, 16.0));
        p.rect_filled(pill_rect, 3.0, color_alpha(dir_color, ALPHA_TINT));
        p.rect_stroke(pill_rect, 3.0, egui::Stroke::new(STROKE_THIN, color_alpha(dir_color, ALPHA_DIM)), egui::StrokeKind::Outside);
        p.text(pill_rect.center(), egui::Align2::CENTER_CENTER, play.direction.label(), egui::FontId::monospace(FONT_XS), dir_color);

        p.text(egui::pos2(cx + pill_w + 6.0, cy + 6.0), egui::Align2::LEFT_CENTER,
            play.play_type.icon(), egui::FontId::proportional(FONT_SM), t.dim.gamma_multiply(0.5));
        p.text(egui::pos2(cx + pill_w + 22.0, cy + 6.0), egui::Align2::LEFT_CENTER,
            &play.symbol, egui::FontId::monospace(FONT_LG), t.text);

        let status_color = match play.status {
            PlayStatus::Draft => t.dim, PlayStatus::Published => t.accent,
            PlayStatus::Active => egui::Color32::from_rgb(255, 191, 0),
            PlayStatus::Won => t.bull, PlayStatus::Lost => t.bear, _ => t.dim.gamma_multiply(0.5),
        };
        let status_x = card_rect.right() - 60.0;
        let sr = egui::Rect::from_min_size(egui::pos2(status_x, cy - 1.0), egui::vec2(48.0, 16.0));
        p.rect_filled(sr, 3.0, color_alpha(status_color, ALPHA_SUBTLE));
        p.text(sr.center(), egui::Align2::CENTER_CENTER, play.status.label(), egui::FontId::monospace(7.0), status_color);

        if play.risk_reward > 0.0 {
            p.text(egui::pos2(status_x - 8.0, cy + 6.0), egui::Align2::RIGHT_CENTER,
                &format!("{:.1}R", play.risk_reward), egui::FontId::monospace(FONT_SM), t.accent);
        }
        cy += 22.0;
    }

    // Row 2: Entry / Target / Stop
    {
        let col_w = (card_w - 24.0) / 3.0;
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER, "ENTRY", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
        p.text(egui::pos2(cx + col_w * 0.6, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("${:.2}", play.entry_price), egui::FontId::monospace(FONT_SM), t.text);
        let tx = cx + col_w;
        p.text(egui::pos2(tx, cy + 4.0), egui::Align2::LEFT_CENTER, "TARGET", egui::FontId::monospace(7.0), t.bull.gamma_multiply(0.6));
        p.text(egui::pos2(tx + col_w * 0.6, cy + 4.0), egui::Align2::LEFT_CENTER,
            &format!("${:.2}", play.target_price), egui::FontId::monospace(FONT_SM), t.bull);
        if play.play_type != PlayType::Scalp {
            let sx = cx + col_w * 2.0;
            p.text(egui::pos2(sx, cy + 4.0), egui::Align2::LEFT_CENTER, "STOP", egui::FontId::monospace(7.0), t.bear.gamma_multiply(0.6));
            p.text(egui::pos2(sx + col_w * 0.5, cy + 4.0), egui::Align2::LEFT_CENTER,
                &format!("${:.2}", play.stop_price), egui::FontId::monospace(FONT_SM), t.bear);
        }
        cy += 20.0;
    }

    // Row 2b: Additional targets with allocations
    if has_targets {
        for tgt in &play.targets {
            p.text(egui::pos2(cx + 8.0, cy + 4.0), egui::Align2::LEFT_CENTER,
                &tgt.label, egui::FontId::monospace(7.0), t.bull.gamma_multiply(0.5));
            p.text(egui::pos2(cx + 30.0, cy + 4.0), egui::Align2::LEFT_CENTER,
                &format!("${:.2}", tgt.price), egui::FontId::monospace(FONT_XS), t.bull);
            p.text(egui::pos2(cx + 100.0, cy + 4.0), egui::Align2::LEFT_CENTER,
                &format!("{}%", (tgt.pct * 100.0) as i32), egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
            cy += 12.0;
        }
    }

    // R:R bar
    {
        let bar_x = cx;
        let bar_w = card_w - 24.0;
        let bar_rect = egui::Rect::from_min_size(egui::pos2(bar_x, cy), egui::vec2(bar_w, 4.0));
        p.rect_filled(bar_rect, 2.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
        let total_range = (play.target_price - play.stop_price).abs();
        let risk = (play.entry_price - play.stop_price).abs();
        let risk_pct = if total_range > 0.0 { (risk / total_range).min(1.0) } else { 0.5 };
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, cy), egui::vec2(bar_w * risk_pct, 4.0)), 2.0, color_alpha(t.bear, ALPHA_DIM));
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x + bar_w * risk_pct, cy), egui::vec2(bar_w * (1.0 - risk_pct), 4.0)), 2.0, color_alpha(t.bull, ALPHA_DIM));
        p.circle_filled(egui::pos2(bar_x + bar_w * risk_pct, cy + 2.0), 3.0, t.text);
        cy += 10.0;
    }

    // Tags
    if has_tags {
        let mut tx = cx;
        for tag in &play.tags {
            p.text(egui::pos2(tx, cy + 4.0), egui::Align2::LEFT_CENTER,
                &format!("#{}", tag), egui::FontId::monospace(7.0), t.accent.gamma_multiply(0.5));
            tx += tag.len() as f32 * 5.0 + 12.0;
        }
        cy += 12.0;
    }

    // Notes
    if !play.notes.is_empty() {
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            &play.notes, egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.5));
    }

    // Hover buttons
    let mut btn_clicked = false;
    if resp.hovered() {
        let del_rect = egui::Rect::from_min_size(egui::pos2(card_rect.right() - 18.0, card_rect.top() + 4.0), egui::vec2(14.0, 14.0));
        let del_resp = ui.interact(del_rect, egui::Id::new(("play_del", &play.id[..8])), egui::Sense::click());
        if del_resp.hovered() {
            p.rect_filled(del_rect, 2.0, color_alpha(t.bear, ALPHA_GHOST));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        p.text(del_rect.center(), egui::Align2::CENTER_CENTER, "\u{00D7}", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.5));
        if del_resp.clicked() { *remove_id = Some(play.id.clone()); btn_clicked = true; }

        if play.status == PlayStatus::Draft {
            let act_rect = egui::Rect::from_min_size(egui::pos2(card_rect.right() - 60.0, card_rect.bottom() - 18.0), egui::vec2(52.0, 14.0));
            let act_resp = ui.interact(act_rect, egui::Id::new(("play_act", &play.id[..8])), egui::Sense::click());
            let act_bg = if act_resp.hovered() { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.accent, ALPHA_GHOST) };
            p.rect_filled(act_rect, 3.0, act_bg);
            p.text(act_rect.center(), egui::Align2::CENTER_CENTER, "Activate", egui::FontId::monospace(7.0), t.accent);
            if act_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            if act_resp.clicked() { *activate_id = Some(play.id.clone()); btn_clicked = true; }
        }
    }

    // Click card body → display play lines on chart
    if resp.clicked() && !btn_clicked {
        *display_id = Some(play.id.clone());
    }

    ui.add_space(GAP_MD);
}
