//! Screenshot library panel — saves chart snapshots as metadata and displays them.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use super::super::widgets::text::{BodyLabel, SectionLabel};
use super::super::widgets::buttons::SimpleBtn;

/// A single screenshot entry with chart state for replay.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScreenshotEntry {
    pub id: String,
    pub symbol: String,
    pub timeframe: String,
    pub timestamp: i64,
    pub note: String,
    // Chart state for replay
    pub vs: f32,
    pub vc: u32,
}

fn screenshots_path() -> std::path::PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    p.push("apex-terminal");
    let _ = std::fs::create_dir_all(&p);
    p.push("screenshots.json");
    p
}

/// Save a new screenshot entry to disk and return it.
pub(crate) fn save_screenshot(symbol: &str, timeframe: &str, vs: f32, vc: u32) -> ScreenshotEntry {
    let mut entries = load_screenshots();
    let id = format!("ss_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());
    let entry = ScreenshotEntry {
        id,
        symbol: symbol.into(),
        timeframe: timeframe.into(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        note: String::new(),
        vs,
        vc,
    };
    entries.insert(0, entry.clone());
    // Keep max 200 entries
    entries.truncate(200);
    let _ = std::fs::write(screenshots_path(), serde_json::to_string_pretty(&entries).unwrap_or_default());
    entry
}

/// Load all screenshot entries from disk.
pub(crate) fn load_screenshots() -> Vec<ScreenshotEntry> {
    let path = screenshots_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// Save current entries back to disk.
fn persist(entries: &[ScreenshotEntry]) {
    let _ = std::fs::write(screenshots_path(), serde_json::to_string_pretty(entries).unwrap_or_default());
}

/// Format a unix timestamp into a readable date string.
fn format_timestamp(ts: i64) -> String {
    // Simple UTC-based formatting without chrono
    let secs_per_day = 86400i64;
    let days_since_epoch = ts / secs_per_day;
    let time_of_day = ts % secs_per_day;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    // Approximate date calculation (good enough for display)
    let mut y = 1970i64;
    let mut remaining = days_since_epoch;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as i64 { m = i + 1; break; }
        remaining -= md as i64;
    }
    if m == 0 { m = 12; }
    let d = remaining + 1;

    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m, d, hours, minutes)
}

/// Draw the screenshot library panel.
pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    t: &Theme,
    panes: &mut [super::super::super::gpu::Chart],
    active_pane: usize,
) {
    if !watchlist.screenshot_open { return; }

    egui::SidePanel::right("screenshot_library")
        .default_width(260.0)
        .min_width(220.0)
        .max_width(400.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 6, right: 6, top: 6, bottom: 4 })
            .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_strong()))))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.add(SectionLabel::new("SCREENSHOTS").tiny().size_px(9.0).strong(true).color(t.accent));
                ui.add(BodyLabel::new(&format!("({})", watchlist.screenshot_entries.len())).size(font_sm()).monospace(true).color(t.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.screenshot_open = false; }
                });
            });
            ui.add_space(4.0);
            draw_content(ui, watchlist, t, panes, active_pane);
        });
}

/// Tab body content (no SidePanel wrapper, no header). Used by feed_panel Screenshots tab.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    t: &Theme,
    panes: &mut [super::super::super::gpu::Chart],
    active_pane: usize,
) {
    if watchlist.screenshot_entries.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.add(BodyLabel::new("No screenshots yet").size(font_sm()).monospace(true).color(t.dim));
            ui.add_space(4.0);
            ui.add(BodyLabel::new("Press Ctrl+Shift+S to capture").size(font_sm()).monospace(true).color(t.dim.gamma_multiply(0.6)));
        });
        return;
    }

    // Scrollable list
    let mut remove_id: Option<String> = None;
    let mut navigate_entry: Option<(String, String, f32, u32)> = None;

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for entry in &watchlist.screenshot_entries {
            let card = egui::Frame::NONE
                .fill(t.bg.gamma_multiply(0.6))
                .corner_radius(r_sm_cr())
                .inner_margin(egui::Margin::same(gap_md() as i8))
                .stroke(egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_muted())));

            card.show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Symbol + timeframe
                    ui.add(BodyLabel::new(&entry.symbol).size(font_sm()).monospace(true).strong(true).color(t.accent));
                    ui.add(BodyLabel::new(&entry.timeframe).size(font_sm()).monospace(true).color(t.dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Delete button
                        if ui.add(egui::Button::new(
                            egui::RichText::new("\u{e9a8}").size(font_sm()).color(t.bear.gamma_multiply(0.5)) // X icon
                        ).frame(false).min_size(egui::vec2(14.0, 14.0))).on_hover_text("Delete").clicked() {
                            remove_id = Some(entry.id.clone());
                        }
                    });
                });

                // Timestamp
                ui.add(BodyLabel::new(&format_timestamp(entry.timestamp)).size(font_xs()).monospace(true).color(t.dim.gamma_multiply(0.7)));

                // Note (if any)
                if !entry.note.is_empty() {
                    ui.add(BodyLabel::new(&entry.note).size(font_sm()).monospace(true).color(t.dim));
                }

                // View button — navigates to the chart state
                if ui.add(SimpleBtn::new("View").color(t.accent).min_width(50.0)).on_hover_text("Navigate to this chart state").clicked() {
                    navigate_entry = Some((entry.symbol.clone(), entry.timeframe.clone(), entry.vs, entry.vc));
                }
            });
            ui.add_space(4.0);
        }
    });

    // Handle deletions
    if let Some(id) = remove_id {
        watchlist.screenshot_entries.retain(|e| e.id != id);
        persist(&watchlist.screenshot_entries);
    }

    // Handle navigation
    if let Some((symbol, timeframe, vs, vc)) = navigate_entry {
        if active_pane < panes.len() {
            let chart = &mut panes[active_pane];
            // Switch symbol/timeframe if different
            if chart.symbol != symbol || chart.timeframe != timeframe {
                chart.pending_symbol_change = Some(symbol);
                chart.pending_timeframe_change = Some(timeframe);
            }
            chart.vs = vs;
            chart.vc = vc;
            chart.vc_target = vc;
            chart.auto_scroll = false;
        }
    }
}
