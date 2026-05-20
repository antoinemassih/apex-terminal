//! Plays panel — shareable strategy cards (playbook).
//! Shows in the Feed sidebar under the "Plays" tab.
//! Each play is a polished visual card with shadow, bevel, and rich layout.
//! The editor spawns draggable play lines on the chart, synced bidirectionally.
//!
//! Migration notes (Agent Q):
//!   - Header row → `PanelSection` ("PLAYBOOK" with count + "+ New Play" action).
//!   - Empty state → `PanelEmpty` with star glyph (replaces the hand-rolled
//!     centered "No plays yet" block).
//!   - Bespoke play card visuals — DELIBERATELY preserved. The `PlayCard`
//!     widget (in `lists/cards/play_card.rs`) keeps its distinctive
//!     shadow + bevel + direction stripe look per user intent. This panel
//!     does not collapse it into `PanelCard` because that primitive's
//!     intentional layered/no-border style would lose the "premium card"
//!     showcase feel the user wants here.
//!   - Play-type chips: left untouched per Agent H's note (per-chip styling
//!     and Long/Short bull/bear tints are intentional).
//!   - There is no standalone `draw` (panel only renders as a tab body via
//!     `draw_content`), so no `SidePanelShell` wrapping is required from this
//!     file — the host (`playbook_panel`) owns the shell.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use crate::ui_kit::widgets::{Button, PanelCard, PanelEmpty, PanelKeyValueRow, PanelSection, PanelTone, RiskRewardBar, Tooltip};
use crate::ui_kit::widgets::tokens::{Variant, Size};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::widgets::Input;
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
    ui.add_space(gap_sm());

    // ── Header: PLAYBOOK section with count + "+ New Play" action ──
    let header_resp = PanelSection::new("PLAYBOOK")
        .title_color(t.accent)
        .count(watchlist.plays.len())
        .action("+ New Play", PanelTone::Accent)
        .show(ui, t, |_ui, _t| {});
    if header_resp.action_clicked {
        watchlist.play_editor_open = !watchlist.play_editor_open;
        if watchlist.play_editor_open && !panes.is_empty() {
            spawn_play_lines(watchlist, &mut panes[ap]);
        } else if !watchlist.play_editor_open && !panes.is_empty() {
            panes[ap].play_lines.clear();
            panes[ap].play_click_to_set = None;
        }
    }

    // ── Play editor (inline) ──
    if watchlist.play_editor_open {
        let chart = if !panes.is_empty() { Some(&mut panes[ap]) } else { None };
        draw_play_editor(ui, watchlist, chart, t);
        ui.add_space(gap_sm());
        separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
        ui.add_space(gap_sm());
    }

    // ── Play cards ──
    if watchlist.plays.is_empty() {
        PanelEmpty::new("No plays yet")
            .glyph(Icon::STAR)
            .hint("Create a play to share a trade idea")
            .min_height(120.0)
            .show(ui, t);
        return;
    }

    let mut remove_id: Option<String> = None;
    let mut activate_id: Option<String> = None;
    let mut display_id: Option<String> = None;

    egui::ScrollArea::vertical().id_salt("plays_scroll").show(ui, |ui| {
        for play in &watchlist.plays {
            render_play_card(ui, play, t, &mut remove_id, &mut activate_id, &mut display_id);
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
        .fill(color_alpha(t.toolbar_border, alpha_faint()))
        .inner_margin(egui::Margin::same(gap_lg() as i8))
        .corner_radius(r_lg_cr())
        .stroke(egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_muted())))
        .show(ui, |ui| {
            ui.add(super::super::components::text::SectionLabel::new("NEW PLAY").color(t.accent));
            ui.add_space(gap_xs());

            // ── Play type selector ──
            ui.horizontal_wrapped(|ui| {
                for pty in PlayType::all() {
                    let sel = pt == *pty;
                    let fg = if sel { t.accent } else { color_half(t.dim) };
                    let bg = if sel { color_alpha(t.accent, alpha_tint()) } else { egui::Color32::TRANSPARENT };
                    let label = format!("{} {}", pty.icon(), pty.label());
                    if ui.add(Button::new(label.as_str()).variant(Variant::Chrome).size(Size::Xs).fg(fg)
                        .fill(bg).corner_radius(current().r_sm as f32)
                        .stroke(egui::Stroke::new(stroke_thin(), if sel { color_alpha(t.accent, alpha_line()) } else { egui::Stroke::NONE.color }))
                        .min_size(egui::vec2(0.0, row_height_compact()))).clicked() {
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
            ui.add(super::super::components::text::MonospaceCode::new(desc).xs().color(t.dim).gamma(0.4));
            ui.add_space(gap_xs());

            // ── Direction toggle ──
            ui.horizontal(|ui| {
                for (dir, label, color) in [
                    (PlayDirection::Long, "LONG", t.bull),
                    (PlayDirection::Short, "SHORT", t.bear),
                ] {
                    let sel = watchlist.play_editor_direction == dir;
                    let fg = if sel { color } else { color_half(t.dim) };
                    let bg = if sel { color_alpha(color, alpha_tint()) } else { egui::Color32::TRANSPARENT };
                    // legacy: monospace+strong; Button uses plain text
                    if ui.add(Button::new(label).variant(Variant::Chrome).size(Size::Sm).fg(fg)
                        .fill(bg).corner_radius(current().r_sm as f32)
                        .stroke(egui::Stroke::new(stroke_thin(), if sel { color_alpha(color, alpha_line()) } else { egui::Stroke::NONE.color }))
                        .min_size(egui::vec2(56.0, row_height_default()))).clicked() {
                        watchlist.play_editor_direction = dir;
                        if let Some(ref mut c) = chart { spawn_play_lines(watchlist, c); }
                    }
                }
            });
            ui.add_space(gap_xs());

            // ── Symbol ──
            ui.horizontal(|ui| {
                dim_label(ui, "Symbol", t.dim);
                Input::new(&mut watchlist.play_editor_symbol)
                    .width(80.0).font_size(font_sm()).placeholder("AAPL").show(ui, t);
            });
            ui.add_space(gap_xs());

            // ── Price fields with click-to-set ──
            let crosshair = "\u{2295}"; // ⊕

            // Entry
            ui.horizontal(|ui| {
                dim_label(ui, "Entry", t.dim);
                let resp = Input::new(&mut watchlist.play_editor_entry)
                    .id(egui::Id::new(("play_price", PlayLineKind::Entry as u8)))
                    .width(70.0).font_size(font_sm()).placeholder("150.00").show(ui, t);
                if resp.lost_focus { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Entry))) {
                    if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Entry); }
                }
            });

            // ── Legs section ──
            ui.add_space(gap_xs());
            separator(ui, color_alpha(t.toolbar_border, alpha_faint()));
            ui.add_space(gap_xs());

            ui.horizontal(|ui| {
                crate::ui_kit::widgets::Header::section("LEGS").show(ui, t);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !watchlist.play_editor_has_t2 && pt != PlayType::Scalp {
                        if Button::small_action("+ Add Target").tint(t.accent).show(ui, t).clicked() {
                            add_target_line(watchlist, chart.as_deref_mut(), PlayLineKind::Target2);
                        }
                    } else if watchlist.play_editor_has_t2 && !watchlist.play_editor_has_t3 && pt != PlayType::Scalp {
                        if Button::small_action("+ Add T3").tint(t.accent).show(ui, t).clicked() {
                            add_target_line(watchlist, chart.as_deref_mut(), PlayLineKind::Target3);
                        }
                    }
                });
            });
            ui.add_space(gap_xs());

            // T1 — primary target with allocation
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("T1").monospace().size(font_xs()).strong().color(color_subtle(t.bull)));
                let resp = Input::new(&mut watchlist.play_editor_target)
                    .id(egui::Id::new(("play_price", PlayLineKind::Target as u8)))
                    .width(65.0).font_size(font_sm()).show(ui, t);
                if resp.lost_focus { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Target))) {
                    if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Target); }
                }
                pct_stepper(ui, &mut watchlist.play_editor_target_pct, t);
            });

            // T2
            if watchlist.play_editor_has_t2 {
                let mut remove_t2 = false;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("T2").monospace().size(font_xs()).strong().color(COLOR_T2));
                    let resp = Input::new(&mut watchlist.play_editor_t2)
                        .id(egui::Id::new(("play_price", PlayLineKind::Target2 as u8)))
                        .width(65.0).font_size(font_sm()).show(ui, t);
                    if resp.lost_focus { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                    if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Target2))) {
                        if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Target2); }
                    }
                    pct_stepper(ui, &mut watchlist.play_editor_t2_pct, t);
                    let r = ui.add(Button::icon(Icon::X).variant(Variant::Chrome).glyph_color(color_muted(t.bear)).size(Size::Sm)
                        .fill(egui::Color32::TRANSPARENT).min_size(BTN_ICON_SM).placement(IconPlacement::ListRow).tone_destructive());
                    Tooltip::new("Remove target").show(ui, &r, t);
                    if r.clicked() { remove_t2 = true; }
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
                    ui.label(egui::RichText::new("T3").monospace().size(font_xs()).strong().color(COLOR_T3));
                    let resp = Input::new(&mut watchlist.play_editor_t3)
                        .id(egui::Id::new(("play_price", PlayLineKind::Target3 as u8)))
                        .width(65.0).font_size(font_sm()).show(ui, t);
                    if resp.lost_focus { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
                    if click_to_set_btn(ui, crosshair, t, chart.as_ref().map_or(false, |c| c.play_click_to_set == Some(PlayLineKind::Target3))) {
                        if let Some(ref mut c) = chart { c.play_click_to_set = Some(PlayLineKind::Target3); }
                    }
                    pct_stepper(ui, &mut watchlist.play_editor_t3_pct, t);
                    let r = ui.add(Button::icon(Icon::X).variant(Variant::Chrome).glyph_color(color_muted(t.bear)).size(Size::Sm)
                        .fill(egui::Color32::TRANSPARENT).min_size(BTN_ICON_SM).placement(IconPlacement::ListRow).tone_destructive());
                    Tooltip::new("Remove target").show(ui, &r, t);
                    if r.clicked() { remove_t3 = true; }
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
                ui.add_space(gap_xs());
                ui.horizontal(|ui| {
                    let stop_label = egui::RichText::new("STOP").monospace().size(font_xs()).color(color_subtle(t.bear));
                    ui.label(stop_label);
                    let resp = Input::new(&mut watchlist.play_editor_stop)
                        .id(egui::Id::new(("play_price", PlayLineKind::Stop as u8)))
                        .width(70.0).font_size(font_sm()).placeholder("148.00").show(ui, t);
                    if resp.lost_focus { sync_form_to_lines(watchlist, chart.as_deref_mut()); }
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
                ui.add_space(gap_xs());
                ui.horizontal(|ui| {
                    dim_label(ui, "R:R", t.dim);
                    let rr_col = if rr >= 2.0 { t.bull } else if rr >= 1.0 { t.warn } else { t.bear };
                    ui.add(super::super::components::text::MonospaceCode::new(&format!("{:.1} : 1", rr)).sm().color(rr_col).strong(true));
                    let bar_w = ui.available_width().min(120.0);
                    crate::ui_kit::widgets::RiskRewardBar::new(risk, reward).width(bar_w).show(ui, t);
                });
            }

            ui.add_space(gap_xs());
            separator(ui, color_alpha(t.toolbar_border, alpha_faint()));
            ui.add_space(gap_xs());

            // ── Tag chips + custom input ──
            ui.horizontal_wrapped(|ui| {
                for tag in TAG_PRESETS {
                    let active = watchlist.play_editor_tags.iter().any(|t| t == tag);
                    if Button::toggle(*tag, active)
                        .corner_radius(current().r_md as f32)
                        .min_size(egui::vec2(0.0, 16.0))
                        .show(ui, t).clicked() {
                        if active { watchlist.play_editor_tags.retain(|x| x != tag); }
                        else { watchlist.play_editor_tags.push(tag.to_string()); }
                    }
                }
            });
            // Custom tag input
            ui.horizontal(|ui| {
                dim_label(ui, "+", t.dim);
                let resp = Input::new(&mut watchlist.play_editor_custom_tag)
                    .width(80.0).font_size(font_xs()).placeholder("custom tag").show(ui, t);
                if resp.lost_focus && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
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
                    let custom_tag_lbl = format!("{} \u{00D7}", ct);
                    if ui.add(Button::new(custom_tag_lbl.as_str()).variant(Variant::Chrome).size(Size::Xs).fg(t.accent)
                        .fill(color_alpha(t.accent, alpha_tint())).corner_radius(current().r_md as f32)
                        .min_size(egui::vec2(0.0, 16.0))).clicked() {
                        watchlist.play_editor_tags.retain(|x| x != ct);
                    }
                }
            });
            ui.add_space(gap_xs());

            // ── Notes ──
            Input::new(&mut watchlist.play_editor_notes)
                .multiline(true).width(ui.available_width())
                .font_size(font_sm()).placeholder("Strategy notes...").show(ui, t);
            ui.add_space(gap_sm());

            // ── Buttons ──
            ui.horizontal(|ui| {
                let can_create = !watchlist.play_editor_symbol.trim().is_empty()
                    && watchlist.play_editor_entry.parse::<f32>().is_ok();

                if Button::action("Create Play").tint(t.accent).disabled(!can_create).show(ui, t).clicked() {
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
                if Button::small_action("Cancel").tint(t.dim).show(ui, t).clicked() {
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

    if ui.add(Button::new("-").variant(Variant::Chrome).size(Size::Xs).fg(t.dim)
        .min_size(BTN_ICON_SM).fill(color_alpha(t.toolbar_border, alpha_faint()))).clicked() {
        val = val.saturating_sub(step).max(5);
    }
    Input::new(pct_str)
        .width(22.0)
        .horizontal_align(egui::Align::Center)
        .show(ui, t);
    ui.add(super::super::components::text::MonospaceCode::new("%").xs().color(t.dim).gamma(0.4));
    if ui.add(Button::new("+").variant(Variant::Chrome).size(Size::Xs).fg(t.dim)
        .min_size(BTN_ICON_SM).fill(color_alpha(t.toolbar_border, alpha_faint()))).clicked() {
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
    let fg = if active { t.accent } else { color_dim(t.dim) };
    let bg = if active { color_alpha(t.accent, alpha_tint()) } else { egui::Color32::TRANSPARENT };
    let resp = ui.add(Button::icon(icon).variant(Variant::Chrome).glyph_color(fg).size(Size::Sm)
        .fill(bg).corner_radius(current().r_sm as f32)
        .min_size(egui::vec2(18.0, row_height_dense())).placement(IconPlacement::ListRow));
    Tooltip::new("Click chart to set price").show(ui, &resp, t);
    resp.clicked()
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
    use crate::chart_renderer::trading::{OrderLevel, OrderSide, OrderStatus, OrderState};

    let entry_side = if play.direction == PlayDirection::Long { OrderSide::Buy } else { OrderSide::Sell };
    let next_id = chart.orders.iter().map(|o| o.id).max().unwrap_or(0) + 1;

    chart.orders.push(OrderLevel {
        id: next_id, side: entry_side, price: play.entry_price,
        qty: play.quantity, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None,
        option_symbol: None, option_con_id: None,
        trail_amount: None, trail_percent: None, filled_ratio: 0.0,
    });

    // Create OCO pairs for each target + stop
    if play.play_type != PlayType::Scalp && play.stop_price > 0.0 {
        let target_id = next_id + 1;
        let stop_id = next_id + 2;
        chart.orders.push(OrderLevel {
            id: target_id, side: OrderSide::OcoTarget, price: play.target_price,
            qty: play.quantity, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: Some(stop_id),
            option_symbol: None, option_con_id: None,
            trail_amount: None, trail_percent: None, filled_ratio: 0.0,
        });
        chart.orders.push(OrderLevel {
            id: stop_id, side: OrderSide::OcoStop, price: play.stop_price,
            qty: play.quantity, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: Some(target_id),
            option_symbol: None, option_con_id: None,
            trail_amount: None, trail_percent: None, filled_ratio: 0.0,
        });
    } else {
        // Scalp — target only
        chart.orders.push(OrderLevel {
            id: next_id + 1, side: OrderSide::OcoTarget, price: play.target_price,
            qty: play.quantity, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None,
            option_symbol: None, option_con_id: None,
            trail_amount: None, trail_percent: None, filled_ratio: 0.0,
        });
    }
}

/// Play card rebuilt on `ui_kit::PanelCard`.
///
/// The outer frame, direction stripe, and elevation shadow come from
/// `PanelCard`. The header row (direction pill + symbol + status + R:R),
/// tags, and notes are laid out with normal egui; the Entry/Target/Stop
/// row uses `PanelKeyValueRow` and the R:R bar uses `RiskRewardBar`.
///
/// The direction pill and status pill remain hand-painted via
/// `ui.painter()` because they are small rounded inset badges with a
/// border — there is no equivalent pill primitive in `ui_kit` yet. All
/// colours route through the `Theme`.
fn render_play_card(
    ui: &mut egui::Ui,
    play: &Play,
    t: &Theme,
    remove_id: &mut Option<String>,
    activate_id: &mut Option<String>,
    display_id: &mut Option<String>,
) {
    let is_long = play.direction == PlayDirection::Long;
    let dir_color = if is_long { t.bull } else { t.bear };

    let tone = if is_long { PanelTone::Bull } else { PanelTone::Bear };

    // Track the rect before/after so we can sense a card-body click below.
    let card_top = ui.cursor().top();

    let card_resp = PanelCard::new()
        .tone(tone)
        .stripe(true)
        .padding(gap_xs())
        .show(ui, t, |ui, t| {
            // ── Header row: direction pill · type icon · symbol · status pill · R:R ──
            let mut delete_clicked   = false;
            let mut activate_clicked = false;

            ui.horizontal(|ui| {
                // Direction pill — custom painter badge (no ui_kit pill primitive yet).
                let pill_size = egui::vec2(42.0, 16.0);
                let (pill_rect, _) = ui.allocate_exact_size(pill_size, egui::Sense::hover());
                let p = ui.painter();
                p.rect_filled(pill_rect, 3.0, color_alpha(dir_color, alpha_tint()));
                p.rect_stroke(pill_rect, 3.0, egui::Stroke::new(stroke_thin(), color_alpha(dir_color, alpha_dim())), egui::StrokeKind::Outside);
                p.text(pill_rect.center(), egui::Align2::CENTER_CENTER, play.direction.label(), mono_xs(), dir_color);

                // Type icon + symbol.
                ui.add_space(gap_xs());
                ui.label(egui::RichText::new(play.play_type.icon()).size(font_sm()).color(color_half(t.dim)));
                ui.label(egui::RichText::new(&play.symbol).monospace().size(font_lg()).color(t.text));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Delete button (always visible for discoverability, enabled on hover).
                    let del = Button::close()
                        .placement(IconPlacement::ListRow)
                        .show(ui, t);
                    if del.clicked() { delete_clicked = true; }
                    Tooltip::new("Delete play").show(ui, &del, t);

                    // R:R label.
                    if play.risk_reward > 0.0 {
                        ui.add_space(gap_xs());
                        ui.label(egui::RichText::new(format!("{:.1}R", play.risk_reward))
                            .monospace().size(font_sm()).color(t.accent));
                    }

                    // Status pill — custom painter badge (no ui_kit equivalent).
                    let status_color = match play.status {
                        PlayStatus::Draft     => t.dim,
                        PlayStatus::Published => t.accent,
                        PlayStatus::Active    => t.warn,
                        PlayStatus::Won       => t.bull,
                        PlayStatus::Lost      => t.bear,
                        _                     => color_half(t.dim),
                    };
                    let pill_size = egui::vec2(48.0, 16.0);
                    let (sr, _) = ui.allocate_exact_size(pill_size, egui::Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(sr, 3.0, color_alpha(status_color, alpha_subtle()));
                    p.text(sr.center(), egui::Align2::CENTER_CENTER, play.status.label(), mono_sm(), status_color);
                });
            });

            ui.add_space(gap_xs());

            // ── Entry / Target / Stop ──
            ui.horizontal(|ui| {
                PanelKeyValueRow::new("ENTRY", format!("${:.2}", play.entry_price))
                    .show(ui, t);
            });
            ui.horizontal(|ui| {
                PanelKeyValueRow::new("TARGET", format!("${:.2}", play.target_price))
                    .tone(PanelTone::Bull)
                    .show(ui, t);
            });
            if play.play_type != PlayType::Scalp && play.stop_price > 0.0 {
                ui.horizontal(|ui| {
                    PanelKeyValueRow::new("STOP", format!("${:.2}", play.stop_price))
                        .tone(PanelTone::Bear)
                        .show(ui, t);
                });
            }

            // ── Additional targets with allocations ──
            if play.targets.len() > 1 {
                for tgt in &play.targets {
                    ui.horizontal(|ui| {
                        PanelKeyValueRow::new(
                            tgt.label.as_str(),
                            format!("${:.2}  {}%", tgt.price, (tgt.pct * 100.0) as i32),
                        )
                        .tone(PanelTone::Bull)
                        .show(ui, t);
                    });
                }
            }

            ui.add_space(gap_xs());

            // ── R:R bar ──
            let risk   = (play.entry_price - play.stop_price).abs();
            let reward = (play.target_price - play.entry_price).abs();
            if risk > 0.001 && play.play_type != PlayType::Scalp {
                let bar_w = ui.available_width();
                RiskRewardBar::new(risk, reward).width(bar_w).show(ui, t);
            }

            // ── Tags ──
            if !play.tags.is_empty() {
                ui.add_space(gap_xs());
                ui.horizontal_wrapped(|ui| {
                    for tag in &play.tags {
                        ui.label(egui::RichText::new(format!("#{}", tag))
                            .monospace().size(font_xs()).color(color_half(t.accent)));
                    }
                });
            }

            // ── Notes ──
            if !play.notes.is_empty() {
                ui.label(egui::RichText::new(&play.notes)
                    .monospace().size(font_xs()).color(color_half(t.dim)));
            }

            // ── Activate button (Draft plays only) ──
            if play.status == PlayStatus::Draft {
                ui.add_space(gap_xs());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if Button::action("Activate").tint(t.accent).show(ui, t).clicked() {
                        activate_clicked = true;
                    }
                });
            }

            (delete_clicked, activate_clicked)
        });

    // Sense a click on the card body to display the play on chart.
    // We interact on the rendered card's rect AFTER the frame paints.
    let card_rect = egui::Rect::from_x_y_ranges(
        ui.max_rect().x_range(),
        card_top..=ui.cursor().top(),
    );
    let card_click_resp = ui.interact(
        card_rect,
        egui::Id::new(("play_card_click", play.id.as_str())),
        egui::Sense::click(),
    );
    cursor::clickable(ui, &card_click_resp);

    // Wire action signals to output.
    let (delete_clicked, activate_clicked) = card_resp;
    if delete_clicked                 { *remove_id   = Some(play.id.clone()); }
    if activate_clicked               { *activate_id = Some(play.id.clone()); }
    if card_click_resp.clicked()
        && !delete_clicked
        && !activate_clicked          { *display_id  = Some(play.id.clone()); }

    ui.add_space(gap_xs());
}

