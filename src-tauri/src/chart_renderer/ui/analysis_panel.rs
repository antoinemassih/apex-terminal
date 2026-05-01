//! Analysis panel — sidebar with subdivided sections, each with its own tab bar.
//! User can add/remove sections and resize them via draggable dividers.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Chart, Theme, SplitSection};
use crate::chart_renderer::AnalysisTab;
use super::widgets::text::SectionLabel;
use super::widgets::buttons::ChromeBtn;

const ALL_TABS: &[(AnalysisTab, &str)] = &[
    (AnalysisTab::Rrg, "RRG"),
    (AnalysisTab::TimeSales, "T&S"),
    (AnalysisTab::Scanner, "Scanner"),
    (AnalysisTab::Scripts, "Scripts"),
    (AnalysisTab::Seasonality, "Seasonality"),
    (AnalysisTab::Research, "Research"),
];

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.analysis_open { return; }

    let mut pending_symbol: Option<String> = None;

    egui::SidePanel::right("analysis_panel")
        .default_width(260.0)
        .min_width(220.0)
        .max_width(480.0)
        .resizable(true)
        .frame(panel_frame(t.toolbar_bg, t.toolbar_border))
        .show(ctx, |ui| {
            let panel_w = ui.available_width();

            // Header: title + add-section button + close
            let header = ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                ui.add(SectionLabel::new("ANALYSIS").color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.analysis_open = false; }
                    // Add section button
                    if ui.add(ChromeBtn::new(egui::RichText::new("+").monospace().size(FONT_SM).color(t.dim))
                        .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(20.0, 20.0))).clicked() {
                        // Pick a tab not yet used, or default to RRG
                        let used: Vec<AnalysisTab> = watchlist.analysis_splits.iter().map(|s| s.tab).collect();
                        let next = ALL_TABS.iter().find(|(tab, _)| !used.contains(tab))
                            .map(|(tab, _)| *tab).unwrap_or(AnalysisTab::Rrg);
                        // Halve the last section's frac for the new one
                        if let Some(last) = watchlist.analysis_splits.last_mut() {
                            last.frac *= 0.5;
                        }
                        let frac = watchlist.analysis_splits.last().map(|s| s.frac).unwrap_or(1.0);
                        watchlist.analysis_splits.push(SplitSection::new(next, frac));
                    }
                });
            });
            let line_y = header.response.rect.max.y;
            ui.painter().line_segment(
                [egui::pos2(ui.min_rect().left(), line_y),
                 egui::pos2(ui.min_rect().right(), line_y)],
                egui::Stroke::new(1.0, color_alpha(t.toolbar_border, alpha_muted())));

            let available_h = ui.available_height();
            let n = watchlist.analysis_splits.len();
            if n == 0 {
                watchlist.analysis_splits.push(SplitSection::new(AnalysisTab::Rrg, 1.0));
            }

            // Compute pixel heights
            let divider_total = (n.saturating_sub(1)) as f32 * 6.0;
            let tab_bar_total = n as f32 * 28.0;
            let content_h = (available_h - divider_total - tab_bar_total).max(40.0);
            let total_frac: f32 = watchlist.analysis_splits.iter().map(|s| s.frac).sum();
            let norm = if total_frac > 0.001 { 1.0 / total_frac } else { 1.0 };
            let heights: Vec<f32> = watchlist.analysis_splits.iter()
                .map(|s| (s.frac * norm * content_h).max(30.0)).collect();

            // Collect deferred actions
            let mut remove_idx: Option<usize> = None;
            let mut divider_drags: Vec<(usize, f32)> = Vec::new();

            for i in 0..n {
                let tab = watchlist.analysis_splits[i].tab;
                let h = heights[i];
                let can_close = n > 1;

                // Tab bar for this section
                ui.horizontal(|ui| {
                    ui.set_min_height(26.0);
                    // Render tabs inline
                    for (t_val, t_label) in ALL_TABS {
                        let sel = tab == *t_val;
                        let fg = if sel { t.accent } else { t.dim.gamma_multiply(0.5) };
                        if ui.add(ChromeBtn::new(egui::RichText::new(*t_label).monospace().size(FONT_XS).color(fg))
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(0.0, 22.0))).clicked() {
                            watchlist.analysis_splits[i].tab = *t_val;
                        }
                    }
                    // Close button for this section
                    if can_close {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(ChromeBtn::new(egui::RichText::new("\u{00D7}").size(FONT_SM).color(t.dim.gamma_multiply(0.4)))
                                .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(18.0, 18.0))).clicked() {
                                remove_idx = Some(i);
                            }
                        });
                    }
                });
                // Underline below active tab
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left(), ui.min_rect().bottom()),
                     egui::pos2(ui.min_rect().right(), ui.min_rect().bottom())],
                    egui::Stroke::new(0.5, color_alpha(t.toolbar_border, alpha_faint())));

                // Content
                egui::ScrollArea::vertical().id_salt(format!("analysis_sec_{}", i)).max_height(h).show(ui, |ui| {
                    match tab {
                        AnalysisTab::Rrg => super::rrg_panel::draw_content(ui, watchlist, t),
                        AnalysisTab::TimeSales => {
                            let sym = if !panes.is_empty() { panes[ap].symbol.clone() } else { String::new() };
                            super::tape_panel::draw_content(ui, watchlist, &sym, t);
                        }
                        AnalysisTab::Scanner => {
                            super::scanner_panel::draw_content(ui, watchlist, panes, ap, t, &mut pending_symbol, panel_w);
                        }
                        AnalysisTab::Scripts => super::script_panel::draw_content(ui, watchlist, t),
                        AnalysisTab::Seasonality => super::seasonality_panel::draw_content(ui, watchlist, panes, ap, t),
                        AnalysisTab::Research => super::research_panel::draw_content(ui, panes, ap, t),
                    }
                });

                // Divider between sections
                if i + 1 < n {
                    let d = split_divider(ui, &format!("adiv_{}", i), t.dim);
                    if d != 0.0 { divider_drags.push((i, d)); }
                }
            }

            // Apply deferred actions
            if let Some(idx) = remove_idx {
                let removed_frac = watchlist.analysis_splits[idx].frac;
                watchlist.analysis_splits.remove(idx);
                // Redistribute removed fraction to remaining
                if !watchlist.analysis_splits.is_empty() {
                    let share = removed_frac / watchlist.analysis_splits.len() as f32;
                    for s in &mut watchlist.analysis_splits { s.frac += share; }
                }
            }
            for (idx, delta) in divider_drags {
                if idx + 1 < watchlist.analysis_splits.len() {
                    let frac_delta = delta / available_h.max(1.0);
                    watchlist.analysis_splits[idx].frac = (watchlist.analysis_splits[idx].frac + frac_delta).clamp(0.05, 0.90);
                    watchlist.analysis_splits[idx + 1].frac = (watchlist.analysis_splits[idx + 1].frac - frac_delta).clamp(0.05, 0.90);
                }
            }
        });

    if let Some(sym) = pending_symbol {
        if let Some(p) = panes.get_mut(ap) {
            p.pending_symbol_change = Some(sym);
        }
    }
}
