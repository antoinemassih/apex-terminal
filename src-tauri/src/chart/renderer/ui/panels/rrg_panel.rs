//! RRG (Relative Rotation Graph) panel — 2D scatter plot showing sector ETFs
//! rotating through 4 quadrants (Leading, Weakening, Lagging, Improving).
//!
//! X-axis: RS-Ratio (relative strength vs SPY, centered at 100)
//! Y-axis: RS-Momentum (rate of change of RS, centered at 100)
//! Sectors rotate clockwise: Improving -> Leading -> Weakening -> Lagging -> Improving

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use super::super::widgets::text::{SectionLabel, MonospaceCode};
use super::super::widgets::status::StatusDot;
use super::super::widgets::inputs::Slider;
use super::super::widgets::frames::CompactPanelFrame;

/// Fixed sector colors for the 11 SPDR sector ETFs.
const SECTOR_COLORS: &[(&str, &str, (u8, u8, u8))] = &[
    ("XLK", "Technology",      (74, 158, 255)),
    ("XLF", "Financials",      (56, 203, 137)),
    ("XLE", "Energy",          (230, 160, ALPHA_MUTED)),
    ("XLV", "Healthcare",      (0, 190, 190)),
    ("XLI", "Industrials",     (160, 160, 170)),
    ("XLP", "Staples",         (165, 120, ALPHA_STRONG)),
    ("XLU", "Utilities",       (230, 200, ALPHA_LINE)),
    ("XLY", "Consumer Disc",   (200, 80, 200)),
    ("XLC", "Communications",  (0, 200, 220)),
    ("XLRE", "Real Estate",    (140, 160, ALPHA_DIM)),
    ("XLB", "Materials",       (224, 82, 82)),
];

/// A single sector dot on the RRG.
#[derive(Clone, Debug)]
pub(crate) struct RRGSector {
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) rs_ratio: f32,
    pub(crate) rs_momentum: f32,
    pub(crate) quadrant: String,
    pub(crate) velocity: f32,
    pub(crate) volume_flow: f32,
    /// Trailing history positions (oldest first) for the tail.
    pub(crate) history: Vec<(f32, f32)>,
}

/// Generate demo sector positions with 20-point history for time slider scrubbing.
/// Each sector follows a clockwise rotation path over time.
pub(crate) fn demo_sectors() -> Vec<RRGSector> {
    // Each sector has 20 history points showing full clockwise rotation
    let demo_data: &[(&str, &str, &[(f32, f32)])] = &[
        // XLK: Improving → Leading (currently leading, strong momentum)
        ("XLK", "Technology", &[
            (97.0, 99.0), (97.5, 99.5), (98.0, 100.0), (98.5, 100.5), (99.0, 101.0),
            (99.3, 101.3), (99.6, 101.5), (99.9, 101.7), (100.2, 101.8), (100.5, 102.0),
            (100.8, 102.1), (101.0, 102.2), (101.3, 102.3), (101.6, 102.4), (101.9, 102.4),
            (102.1, 102.5), (102.4, 102.5), (102.6, 102.5), (102.9, 102.5), (103.2, 102.5),
        ]),
        // XLC: Moving into Leading
        ("XLC", "Communications", &[
            (98.5, 97.8), (98.7, 98.2), (98.9, 98.7), (99.1, 99.2), (99.3, 99.6),
            (99.5, 99.9), (99.7, 100.1), (99.9, 100.3), (100.0, 100.5), (100.2, 100.6),
            (100.3, 100.7), (100.5, 100.8), (100.7, 100.9), (100.9, 101.0), (101.0, 101.0),
            (101.2, 101.1), (101.3, 101.1), (101.5, 101.2), (101.6, 101.2), (101.8, 101.2),
        ]),
        // XLF: Leading → Weakening (peaking, momentum fading)
        ("XLF", "Financials", &[
            (100.5, 101.8), (101.0, 102.0), (101.5, 102.0), (102.0, 101.8), (102.3, 101.5),
            (102.6, 101.2), (102.8, 101.0), (103.0, 100.7), (103.2, 100.4), (103.3, 100.2),
            (103.4, 100.0), (103.5, 99.8), (103.5, 99.6), (103.5, 99.4), (103.4, 99.2),
            (103.3, 99.0), (103.2, 98.9), (103.1, 98.8), (103.0, 98.6), (102.8, 98.5),
        ]),
        // XLI: Weakening
        ("XLI", "Industrials", &[
            (100.0, 101.5), (100.3, 101.3), (100.5, 101.0), (100.7, 100.7), (100.9, 100.4),
            (101.0, 100.2), (101.1, 100.0), (101.2, 99.8), (101.2, 99.6), (101.3, 99.5),
            (101.3, 99.3), (101.3, 99.2), (101.4, 99.1), (101.4, 99.1), (101.4, 99.0),
            (101.4, 99.0), (101.5, 99.0), (101.5, 99.0), (101.5, 99.0), (101.5, 99.0),
        ]),
        // XLY: Weakening → Lagging
        ("XLY", "Consumer Disc", &[
            (101.5, 100.5), (101.4, 100.2), (101.3, 99.9), (101.2, 99.7), (101.1, 99.5),
            (101.0, 99.3), (100.9, 99.1), (100.8, 98.9), (100.7, 98.8), (100.7, 98.7),
            (100.6, 98.6), (100.6, 98.5), (100.5, 98.4), (100.5, 98.4), (100.5, 98.3),
            (100.4, 98.3), (100.4, 98.2), (100.4, 98.2), (100.5, 98.2), (100.8, 98.2),
        ]),
        // XLE: Lagging (deep underperformance)
        ("XLE", "Energy", &[
            (101.0, 100.0), (100.8, 99.5), (100.5, 99.0), (100.2, 98.6), (99.9, 98.3),
            (99.6, 98.1), (99.4, 97.9), (99.2, 97.8), (99.0, 97.7), (98.8, 97.6),
            (98.6, 97.6), (98.4, 97.5), (98.3, 97.5), (98.2, 97.5), (98.1, 97.5),
            (98.0, 97.5), (97.9, 97.5), (97.9, 97.5), (97.8, 97.5), (97.8, 97.5),
        ]),
        // XLB: Lagging
        ("XLB", "Materials", &[
            (100.5, 101.0), (100.4, 100.6), (100.2, 100.2), (100.0, 99.9), (99.8, 99.6),
            (99.6, 99.4), (99.5, 99.3), (99.3, 99.2), (99.2, 99.1), (99.0, 99.0),
            (98.9, 99.0), (98.8, 98.9), (98.7, 98.9), (98.6, 98.9), (98.5, 98.9),
            (98.4, 98.8), (98.3, 98.8), (98.3, 98.8), (98.2, 98.8), (98.2, 98.8),
        ]),
        // XLP: Lagging → Improving
        ("XLP", "Staples", &[
            (99.0, 100.5), (99.0, 100.2), (98.9, 99.9), (98.8, 99.6), (98.7, 99.4),
            (98.6, 99.2), (98.5, 99.1), (98.4, 99.0), (98.3, 98.9), (98.2, 98.9),
            (98.1, 98.9), (98.0, 98.9), (97.9, 98.9), (97.8, 98.9), (97.7, 99.0),
            (97.6, 99.0), (97.6, 99.0), (97.5, 99.0), (97.5, 99.0), (97.5, 99.0),
        ]),
        // XLU: Improving (accelerating relative strength)
        ("XLU", "Utilities", &[
            (96.0, 97.5), (96.2, 97.8), (96.4, 98.2), (96.6, 98.5), (96.8, 98.8),
            (97.0, 99.1), (97.1, 99.3), (97.2, 99.6), (97.4, 99.8), (97.5, 100.0),
            (97.7, 100.2), (97.8, 100.4), (97.9, 100.6), (98.0, 100.8), (98.1, 101.0),
            (98.2, 101.1), (98.3, 101.2), (98.4, 101.3), (98.4, 101.4), (98.5, 101.5),
        ]),
        // XLV: Improving
        ("XLV", "Healthcare", &[
            (97.0, 97.5), (97.2, 97.8), (97.4, 98.1), (97.5, 98.4), (97.7, 98.6),
            (97.8, 98.8), (97.9, 99.0), (98.0, 99.2), (98.1, 99.4), (98.2, 99.5),
            (98.3, 99.7), (98.4, 99.8), (98.5, 99.9), (98.6, 100.0), (98.7, 100.2),
            (98.8, 100.4), (98.9, 100.5), (99.0, 100.7), (99.1, 100.8), (99.2, 101.0),
        ]),
        // XLRE: Improving (just crossed into positive momentum)
        ("XLRE", "Real Estate", &[
            (97.5, 97.0), (97.7, 97.3), (97.8, 97.6), (97.9, 97.9), (98.0, 98.2),
            (98.1, 98.4), (98.2, 98.6), (98.3, 98.8), (98.4, 99.0), (98.5, 99.2),
            (98.6, 99.3), (98.7, 99.5), (98.8, 99.6), (98.9, 99.7), (99.0, 99.8),
            (99.1, 99.9), (99.2, 100.0), (99.3, 100.1), (99.4, 100.3), (99.5, 100.5),
        ]),
    ];

    demo_data.iter().map(|(sym, name, hist)| {
        let last = hist.last().unwrap_or(&(100.0, 100.0));
        let quad = match (last.0 >= 100.0, last.1 >= 100.0) {
            (true, true) => "LEADING",
            (true, false) => "WEAKENING",
            (false, false) => "LAGGING",
            (false, true) => "IMPROVING",
        };
        let vel = if hist.len() >= 2 {
            let prev = hist[hist.len() - 2];
            ((last.0 - prev.0).powi(2) + (last.1 - prev.1).powi(2)).sqrt()
        } else { 0.0 };
        RRGSector {
            symbol: sym.to_string(),
            name: name.to_string(),
            rs_ratio: last.0,
            rs_momentum: last.1,
            quadrant: quad.to_string(),
            velocity: vel,
            volume_flow: 1.0,
            history: hist.to_vec(),
        }
    }).collect()
}

/// Slice demo sectors based on time offset (0.0 = current, 1.0 = oldest).
/// Returns sectors with history truncated to the time position and tail_length.
pub(crate) fn demo_sectors_at_time(time_offset: f32, tail_length: usize) -> Vec<RRGSector> {
    let full = demo_sectors();
    full.into_iter().map(|mut s| {
        let n = s.history.len();
        if n == 0 { return s; }
        // Time offset: 0.0 = show all history up to latest, 1.0 = show from earliest
        let end_idx = ((1.0 - time_offset) * (n as f32)).round() as usize;
        let end_idx = end_idx.clamp(1, n);
        let start_idx = end_idx.saturating_sub(tail_length);
        s.history = s.history[start_idx..end_idx].to_vec();
        if let Some(last) = s.history.last() {
            s.rs_ratio = last.0;
            s.rs_momentum = last.1;
            let quad = match (last.0 >= 100.0, last.1 >= 100.0) {
                (true, true) => "LEADING",
                (true, false) => "WEAKENING",
                (false, false) => "LAGGING",
                (false, true) => "IMPROVING",
            };
            s.quadrant = quad.to_string();
        }
        s
    }).collect()
}

/// Look up sector color by symbol.
fn sector_color(symbol: &str) -> egui::Color32 {
    for (sym, _, (r, g, b)) in SECTOR_COLORS {
        if *sym == symbol {
            return egui::Color32::from_rgb(*r, *g, *b);
        }
    }
    egui::Color32::from_rgb(180, 180, 180)
}

/// Draw the RRG panel content into `ui` (used by analysis_panel as a tab).
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    // Use live data if available, otherwise demo
    let sectors: &[RRGSector] = if watchlist.rrg_sectors.is_empty() {
        &[]
    } else {
        &watchlist.rrg_sectors
    };
    let use_demo = sectors.is_empty();

    // Header
    ui.add(SectionLabel::new("RRG — Relative Rotation").xs().color(t.dim));
    super::super::style::separator(ui, t.toolbar_border);
    ui.add_space(2.0);

    // Compute the square plot area
    let avail = ui.available_size();
    let plot_size = avail.x.min(avail.y - 30.0).max(200.0); // leave room for cycle text
    let (response, painter) = ui.allocate_painter(
        egui::vec2(plot_size, plot_size),
        egui::Sense::hover(),
    );
    let rect = response.rect;

    draw_rrg_content(&painter, rect, sectors, use_demo, t,
        watchlist.rrg_time_offset, watchlist.rrg_tail_length);

    // Time slider
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("TIME").xs().color(color_alpha(t.dim, alpha_active())));
        ui.spacing_mut().slider_width = plot_size - 50.0;
        Slider::new(&mut watchlist.rrg_time_offset, 0.0..=0.95)
            .show_value(false)
            .theme(t)
            .show(ui);
    });
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("TAIL").xs().color(color_alpha(t.dim, alpha_active())));
        ui.spacing_mut().slider_width = plot_size - 50.0;
        let mut tail = watchlist.rrg_tail_length as f32;
        if Slider::new(&mut tail, 1.0..=15.0).show_value(false).step(1.0).theme(t).show(ui).changed() {
            watchlist.rrg_tail_length = tail as usize;
        }
    });

    // Cycle phase text at the bottom
    ui.add_space(2.0);
    let phase = if !watchlist.rrg_cycle_phase.is_empty() {
        watchlist.rrg_cycle_phase.as_str()
    } else {
        "LATE EXPANSION"
    };
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("CYCLE:").xs().color(color_alpha(t.dim, alpha_heavy())));
        ui.add(MonospaceCode::new(phase).xs().color(t.bull));
    });

    // Legend — compact 2-column layout
    ui.add_space(4.0);
    super::super::style::separator(ui, t.toolbar_border);
    ui.add_space(2.0);
    let legend_sectors = if use_demo { &demo_sectors()[..] } else { sectors };
    let half = (legend_sectors.len() + 1) / 2;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            for s in legend_sectors.iter().take(half) {
                let c = sector_color(&s.symbol);
                ui.add(StatusDot::new().color(c).label(&s.symbol).radius(3.0));
            }
        });
        ui.vertical(|ui| {
            for s in legend_sectors.iter().skip(half) {
                let c = sector_color(&s.symbol);
                ui.add(StatusDot::new().color(c).label(&s.symbol).radius(3.0));
            }
        });
    });
}

/// Draw the RRG panel as a side panel (matches scanner_panel, news_panel pattern).
pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    t: &Theme,
) {
    if !watchlist.rrg_open { return; }

    egui::SidePanel::right("rrg_panel")
        .default_width(320.0)
        .min_width(240.0)
        .max_width(500.0)
        .resizable(true)
        .frame(CompactPanelFrame::new(t.toolbar_bg, t.toolbar_border).build())
        .show(ctx, |ui| {
            // Add close button at the top
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if super::super::style::close_button(ui, t.dim) {
                        watchlist.rrg_open = false;
                    }
                });
            });
            draw_content(ui, watchlist, t);
        });
}

/// Draw the RRG scatter plot content into a given rect.
fn draw_rrg_content(
    painter: &egui::Painter,
    rect: egui::Rect,
    live_sectors: &[RRGSector],
    use_demo: bool,
    t: &Theme,
    time_offset: f32,
    tail_length: usize,
) {
    let demo = if use_demo { demo_sectors_at_time(time_offset, tail_length) } else { vec![] };
    let sectors = if use_demo { &demo } else { live_sectors };

    // Background
    painter.rect_filled(rect, 0.0, t.bg);

    // Compute data range: find min/max across all sector positions (including history)
    let mut min_x: f32 = 100.0;
    let mut max_x: f32 = 100.0;
    let mut min_y: f32 = 100.0;
    let mut max_y: f32 = 100.0;
    for s in sectors {
        min_x = min_x.min(s.rs_ratio);
        max_x = max_x.max(s.rs_ratio);
        min_y = min_y.min(s.rs_momentum);
        max_y = max_y.max(s.rs_momentum);
        for (hx, hy) in &s.history {
            min_x = min_x.min(*hx);
            max_x = max_x.max(*hx);
            min_y = min_y.min(*hy);
            max_y = max_y.max(*hy);
        }
    }

    // Add padding and ensure 100 is centered
    let padding = 1.5;
    min_x = (min_x - padding).min(100.0 - 2.0);
    max_x = (max_x + padding).max(100.0 + 2.0);
    min_y = (min_y - padding).min(100.0 - 2.0);
    max_y = (max_y + padding).max(100.0 + 2.0);

    // Ensure symmetric around 100
    let x_range = (100.0 - min_x).max(max_x - 100.0);
    let y_range = (100.0 - min_y).max(max_y - 100.0);
    min_x = 100.0 - x_range;
    max_x = 100.0 + x_range;
    min_y = 100.0 - y_range;
    max_y = 100.0 + y_range;

    let margin = 20.0; // px margin inside rect for axis labels
    let plot_rect = rect.shrink(margin);

    // Map data coords to screen coords
    let to_screen = |ratio: f32, momentum: f32| -> egui::Pos2 {
        let x = plot_rect.left() + (ratio - min_x) / (max_x - min_x) * plot_rect.width();
        // Y is inverted: higher momentum = higher on screen
        let y = plot_rect.bottom() - (momentum - min_y) / (max_y - min_y) * plot_rect.height();
        egui::pos2(x, y)
    };

    // ── Quadrant background tints (very faint, alpha ~8) ──
    let center = to_screen(100.0, 100.0);

    // Leading (top-right): faint green
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(center.x, plot_rect.top()), plot_rect.right_bottom().into()),
        0.0,
        color_alpha(t.rrg_leading, 8),
    );
    // Correct: Leading = top-right corner
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(center.x, plot_rect.top()), egui::pos2(plot_rect.right(), center.y)),
        0.0,
        color_alpha(t.rrg_leading, 8),
    );
    // Weakening (bottom-right): faint yellow
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(center.x, center.y), egui::pos2(plot_rect.right(), plot_rect.bottom())),
        0.0,
        color_alpha(t.rrg_weakening, 8),
    );
    // Lagging (bottom-left): faint red
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(plot_rect.left(), center.y), egui::pos2(center.x, plot_rect.bottom())),
        0.0,
        color_alpha(t.rrg_lagging, 8),
    );
    // Improving (top-left): faint blue
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(plot_rect.left(), plot_rect.top()), center),
        0.0,
        color_alpha(t.rrg_improving, 8),
    );

    // ── Axis crosshair at (100, alpha_active()) ──
    let axis_color = color_alpha(t.dim, alpha_tint());
    let axis_stroke = egui::Stroke::new(stroke_std(), axis_color);
    // Vertical line (RS-Ratio = 100)
    painter.line_segment(
        [egui::pos2(center.x, plot_rect.top()), egui::pos2(center.x, plot_rect.bottom())],
        axis_stroke,
    );
    // Horizontal line (RS-Momentum = 100)
    painter.line_segment(
        [egui::pos2(plot_rect.left(), center.y), egui::pos2(plot_rect.right(), center.y)],
        axis_stroke,
    );

    // ── Axis labels ──
    let axis_label_color = color_alpha(t.dim, alpha_dim());
    let axis_font = egui::FontId::monospace(8.0);
    // X-axis label
    painter.text(
        egui::pos2(plot_rect.center().x, rect.bottom() - 4.0),
        egui::Align2::CENTER_BOTTOM,
        "RS-Ratio",
        axis_font.clone(),
        axis_label_color,
    );
    // Y-axis label (rotated text not easy in egui, so place vertically)
    painter.text(
        egui::pos2(rect.left() + 4.0, plot_rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Mom",
        axis_font.clone(),
        axis_label_color,
    );

    // ── Axis tick marks ──
    let tick_color = color_alpha(t.dim, alpha_subtle());
    let tick_font = egui::FontId::monospace(7.0);
    // X-axis ticks
    let x_step = ((max_x - min_x) / 4.0).max(0.5);
    let mut xv = (min_x / x_step).ceil() * x_step;
    while xv <= max_x {
        let screen = to_screen(xv, min_y);
        painter.text(
            egui::pos2(screen.x, plot_rect.bottom() + 2.0),
            egui::Align2::CENTER_TOP,
            format!("{:.0}", xv),
            tick_font.clone(),
            tick_color,
        );
        if (xv - 100.0).abs() > 0.1 {
            painter.line_segment(
                [egui::pos2(screen.x, plot_rect.top()), egui::pos2(screen.x, plot_rect.bottom())],
                egui::Stroke::new(stroke_thin(), color_alpha(t.dim, 12)),
            );
        }
        xv += x_step;
    }
    // Y-axis ticks
    let y_step = ((max_y - min_y) / 4.0).max(0.5);
    let mut yv = (min_y / y_step).ceil() * y_step;
    while yv <= max_y {
        let screen = to_screen(min_x, yv);
        painter.text(
            egui::pos2(plot_rect.left() - 2.0, screen.y),
            egui::Align2::RIGHT_CENTER,
            format!("{:.0}", yv),
            tick_font.clone(),
            tick_color,
        );
        if (yv - 100.0).abs() > 0.1 {
            painter.line_segment(
                [egui::pos2(plot_rect.left(), screen.y), egui::pos2(plot_rect.right(), screen.y)],
                egui::Stroke::new(stroke_thin(), color_alpha(t.dim, 12)),
            );
        }
        yv += y_step;
    }

    // ── Quadrant labels (corners, very faded) ──
    let quad_font = egui::FontId::monospace(9.0);
    let quad_alpha = 40u8;
    // Leading (top-right)
    painter.text(
        egui::pos2(plot_rect.right() - 4.0, plot_rect.top() + 4.0),
        egui::Align2::RIGHT_TOP,
        "LEADING",
        quad_font.clone(),
        color_alpha(t.rrg_leading, quad_alpha),
    );
    // Weakening (bottom-right)
    painter.text(
        egui::pos2(plot_rect.right() - 4.0, plot_rect.bottom() - 4.0),
        egui::Align2::RIGHT_BOTTOM,
        "WEAKENING",
        quad_font.clone(),
        color_alpha(t.rrg_weakening, quad_alpha),
    );
    // Lagging (bottom-left)
    painter.text(
        egui::pos2(plot_rect.left() + 4.0, plot_rect.bottom() - 4.0),
        egui::Align2::LEFT_BOTTOM,
        "LAGGING",
        quad_font.clone(),
        color_alpha(t.rrg_lagging, quad_alpha),
    );
    // Improving (top-left)
    painter.text(
        egui::pos2(plot_rect.left() + 4.0, plot_rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        "IMPROVING",
        quad_font,
        color_alpha(t.rrg_improving, quad_alpha),
    );

    // ── Draw sector tails and dots ──
    for sector in sectors {
        let color = sector_color(&sector.symbol);

        // Draw trailing tail (polyline from oldest to newest, fading alpha)
        if sector.history.len() >= 2 {
            let n = sector.history.len();
            for i in 0..n - 1 {
                let (x0, y0) = sector.history[i];
                let (x1, y1) = sector.history[i + 1];
                let p0 = to_screen(x0, y0);
                let p1 = to_screen(x1, y1);
                // Alpha fades: oldest is very faint, newest is brighter
                let alpha = ((i as f32 + 1.0) / n as f32 * 160.0) as u8;
                let seg_color = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
                let width = 1.0 + (i as f32 / n as f32) * 1.5; // thicker toward current
                painter.line_segment([p0, p1], egui::Stroke::new(width, seg_color));
            }
        }

        // Draw the current position dot
        let pos = to_screen(sector.rs_ratio, sector.rs_momentum);
        let dot_radius = 5.0 + sector.volume_flow.clamp(0.0, 3.0) * 1.5; // 5-9.5px
        // Outer glow
        painter.circle_filled(
            pos,
            dot_radius + 2.0,
            egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha_subtle()),
        );
        // Main dot
        painter.circle_filled(pos, dot_radius, color);
        // Inner highlight
        painter.circle_filled(
            egui::pos2(pos.x - 1.0, pos.y - 1.0),
            dot_radius * 0.35,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha_line()),
        );

        // Label next to dot
        let label_offset = dot_radius + 4.0;
        // Place label to the right unless it would go off-screen
        let label_x = if pos.x + label_offset + 30.0 > plot_rect.right() {
            pos.x - label_offset
        } else {
            pos.x + label_offset
        };
        let label_align = if pos.x + label_offset + 30.0 > plot_rect.right() {
            egui::Align2::RIGHT_CENTER
        } else {
            egui::Align2::LEFT_CENTER
        };
        painter.text(
            egui::pos2(label_x, pos.y),
            label_align,
            &sector.symbol,
            egui::FontId::monospace(8.5),
            egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200),
        );
    }

    // ── Border around plot area ──
    painter.rect_stroke(
        plot_rect,
        0.0,
        egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_muted())),
        egui::StrokeKind::Outside,
    );
}
