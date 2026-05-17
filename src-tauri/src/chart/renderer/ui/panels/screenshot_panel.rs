//! Screenshot library panel — saves chart snapshots as metadata and displays them.
//!
//! Migrated to canonical ui_kit side-panel primitives:
//!   - Standalone `draw` uses `SidePanelShell` (was hand-rolled
//!     `SidePanel + PanelHeaderWithClose`).
//!   - Embedded `draw_content` uses `PanelSection` + `PanelListRow` +
//!     `PanelEmpty` for the empty state.
//! Each screenshot row: symbol in the leading slot (color swatch /
//! placeholder for the thumb), filename-ish "SYM TF" as primary, date/time
//! as secondary, and a trailing delete-X + View button.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use crate::ui_kit::widgets::{
    Button, PanelEmpty, PanelListRow, PanelSection, SidePanelShell, Width,
};
use crate::ui_kit::icons::Icon;

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

fn persist(entries: &[ScreenshotEntry]) {
    let _ = std::fs::write(screenshots_path(), serde_json::to_string_pretty(entries).unwrap_or_default());
}

/// Format a unix timestamp into a readable date string.
fn format_timestamp(ts: i64) -> String {
    let secs_per_day = 86400i64;
    let days_since_epoch = ts / secs_per_day;
    let time_of_day = ts % secs_per_day;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

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

/// Draw the screenshot library panel (standalone side panel).
pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    t: &Theme,
    panes: &mut [super::super::super::gpu::Chart],
    active_pane: usize,
) {
    if !watchlist.screenshot_open { return; }

    let resp = SidePanelShell::new("screenshot_library", "SCREENSHOTS")
        .width(Width::Narrow)
        .pane_metrics(
            crate::chart_renderer::gpu::pane_tabs_header_h(watchlist),
            watchlist.pane_header_size.title_font(),
        )
        .show(ctx, t, |ui, t| {
            draw_content(ui, watchlist, t, panes, active_pane);
        });
    if resp.close_clicked { watchlist.screenshot_open = false; }
}

/// Tab body content (no SidePanel wrapper, no header). Used by feed_panel Screenshots tab.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    t: &Theme,
    panes: &mut [super::super::super::gpu::Chart],
    active_pane: usize,
) {
    let count = watchlist.screenshot_entries.len();

    let mut remove_id: Option<String> = None;
    let mut navigate_entry: Option<(String, String, f32, u32)> = None;

    PanelSection::new("LIBRARY")
        .count(count)
        .meta("Ctrl+Shift+S to capture")
        .show(ui, t, |ui, t| {
            if watchlist.screenshot_entries.is_empty() {
                PanelEmpty::new("No screenshots yet")
                    .glyph(Icon::CAMERA)
                    .hint("Press Ctrl+Shift+S to capture")
                    .show(ui, t);
                return;
            }

            egui::ScrollArea::vertical()
                .id_salt("screenshot_scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for entry in &watchlist.screenshot_entries {
                        let id_salt = format!("ss_{}", entry.id);
                        let primary = format!("{}  {}", entry.symbol, entry.timeframe);
                        let secondary = format_timestamp(entry.timestamp);
                        let entry_id = entry.id.clone();
                        let nav_data = (
                            entry.symbol.clone(),
                            entry.timeframe.clone(),
                            entry.vs,
                            entry.vc,
                        );

                        let delete = std::cell::Cell::new(false);
                        let view = std::cell::Cell::new(false);
                        let delete_ref = &delete;
                        let view_ref = &view;
                        let accent_col = t.accent;

                        PanelListRow::new(&id_salt)
                            .leading(move |ui, _t| {
                                ui.label(
                                    egui::RichText::new(Icon::CAMERA)
                                        .font(egui::FontId::proportional(font_md()))
                                        .color(accent_col),
                                );
                            })
                            .primary(&primary)
                            .secondary(&secondary)
                            .trailing(move |ui, t| {
                                if Button::close().show(ui, t).on_hover_text("Delete screenshot").clicked() {
                                    delete_ref.set(true);
                                }
                                if Button::small_action("View").tint(accent_col).show(ui, t).clicked() {
                                    view_ref.set(true);
                                }
                            })
                            .dense(false)
                            .show(ui, t);

                        if delete.get() { remove_id = Some(entry_id); }
                        if view.get() { navigate_entry = Some(nav_data); }
                    }
                });
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
