//! Data story — Table, Sparkline, HeatmapGrid, RiskRewardBar, Calendar, DatePicker, TimePicker.
//!
//! Widgets shown (~20 instances):
//!   Table: 5 rows × 4 columns, sortable header (1)
//!   Sparkline: uptrend, sideways, V-shape (3)
//!   HeatmapGrid: 5×5 = 25 cells, 5 cols (1)
//!   RiskRewardBar: 1:2 ratio, 1:3 ratio (2)
//!   Calendar: single-select month view (1)
//!   DatePicker: single trigger (1)
//!   TimePicker: trigger (1)

use _scaffold_lib::ui_kit::widgets::{
    Calendar, ColAlign, Column, DatePicker, HeatmapCell, HeatmapGrid, RiskRewardBar, Sparkline,
    Table, TimePicker,
    theme::ComponentTheme,
};
use egui::Ui;

use super::super::stories::DataState;

// ── Sample data ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MarketRow {
    symbol: &'static str,
    last: f32,
    change: f32,
    volume: &'static str,
}

fn sample_rows() -> Vec<MarketRow> {
    vec![
        MarketRow { symbol: "AAPL",  last: 182.45, change:  1.20, volume: "72.4M" },
        MarketRow { symbol: "TSLA",  last: 248.10, change: -2.35, volume: "131.2M" },
        MarketRow { symbol: "NVDA",  last: 875.62, change:  4.88, volume: "48.9M" },
        MarketRow { symbol: "MSFT",  last: 415.33, change:  0.54, volume: "22.1M" },
        MarketRow { symbol: "GOOGL", last: 171.80, change: -0.90, volume: "18.3M" },
    ]
}

fn heatmap_cells() -> Vec<HeatmapCell> {
    let data = [
        ("AAPL",  1.2), ("MSFT",  0.5), ("NVDA",  4.9), ("GOOGL", -0.9), ("AMZN",  2.1),
        ("TSLA", -2.4), ("META",  1.8), ("NFLX", -1.1), ("AMD",    3.5), ("INTC", -0.3),
        ("CRM",   0.7), ("ORCL",  1.4), ("ADBE",  2.2), ("QCOM",  -1.7), ("TXN",   0.8),
        ("AVGO",  1.9), ("MU",   -2.8), ("AMAT",  3.1), ("LRCX",  -0.5), ("KLAC",  2.6),
        ("ON",    1.1), ("MRVL",  2.3), ("SWKS", -1.3), ("QRVO",   0.4), ("MPWR",  1.7),
    ];
    data.iter().map(|(sym, pct)| HeatmapCell {
        symbol: sym.to_string(),
        change_pct: *pct as f32,
    }).collect()
}

// ── Story entry point ─────────────────────────────────────────────────────────

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme, state: &mut DataState) {
    // ── Table ─────────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Table");
    ui.add_space(6.0);

    let rows = sample_rows();
    let cols = [
        Column::new("Symbol").min_width(70.0).sortable(true),
        Column::new("Last").width(80.0).align(ColAlign::Right).sortable(true),
        Column::new("Change").width(80.0).align(ColAlign::Right).sortable(true),
        Column::new("Volume").width(80.0).align(ColAlign::Right),
    ];

    let bull = theme.bull();
    let bear = theme.bear();
    let text_col = theme.text();
    let dim_col = theme.dim();

    Table::new(&cols, &rows, &mut state.table_state)
        .row_height(22.0)
        .alternate_rows(true)
        .hover_row(true)
        .row_render(move |ui, _theme, row, col_idx, _rect| {
            match col_idx {
                0 => {
                    ui.label(egui::RichText::new(row.symbol).color(text_col).size(11.0).monospace());
                }
                1 => {
                    ui.label(
                        egui::RichText::new(format!("${:.2}", row.last))
                            .color(text_col)
                            .size(11.0)
                            .monospace(),
                    );
                }
                2 => {
                    let color = if row.change >= 0.0 { bull } else { bear };
                    let sign  = if row.change >= 0.0 { "+" } else { "" };
                    ui.label(
                        egui::RichText::new(format!("{}{:.2}%", sign, row.change))
                            .color(color)
                            .size(11.0)
                            .monospace(),
                    );
                }
                3 => {
                    ui.label(egui::RichText::new(row.volume).color(dim_col).size(11.0));
                }
                _ => {}
            }
        })
        .show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Sparkline ─────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Sparkline");
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Uptrend / Sideways / V-shape").color(theme.dim()).size(10.0));
    ui.add_space(4.0);

    let uptrend:   &[f32] = &[10.0, 12.0, 11.5, 14.0, 13.5, 15.0, 17.0, 18.5, 19.0, 21.0];
    let sideways:  &[f32] = &[15.0, 15.5, 14.8, 15.2, 15.0, 15.3, 14.9, 15.1, 15.0, 15.2];
    let v_shape:   &[f32] = &[20.0, 17.0, 14.0, 11.0,  8.0,  9.0, 12.0, 15.0, 18.0, 21.0];

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Up").color(theme.dim()).size(9.0));
            Sparkline::new(uptrend)
                .color(theme.bull())
                .size(80.0, 28.0)
                .show(ui, theme);
        });
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Side").color(theme.dim()).size(9.0));
            Sparkline::new(sideways)
                .color(theme.dim())
                .size(80.0, 28.0)
                .show(ui, theme);
        });
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("V").color(theme.dim()).size(9.0));
            Sparkline::new(v_shape)
                .color(theme.accent())
                .size(80.0, 28.0)
                .show(ui, theme);
        });
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Bars").color(theme.dim()).size(9.0));
            Sparkline::new(uptrend)
                .bars()
                .color(theme.bull())
                .size(80.0, 28.0)
                .show(ui, theme);
        });
    });

    ui.add_space(12.0);
    rule(ui, theme);

    // ── HeatmapGrid ───────────────────────────────────────────────────────────
    story_heading(ui, theme, "HeatmapGrid");
    ui.add_space(6.0);
    ui.label(egui::RichText::new("5 × 5 grid — green = bull, red = bear").color(theme.dim()).size(10.0));
    ui.add_space(4.0);

    let cells = heatmap_cells();
    HeatmapGrid::new(&cells).num_cols(5).show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── RiskRewardBar ─────────────────────────────────────────────────────────
    story_heading(ui, theme, "RiskRewardBar");
    ui.add_space(6.0);

    let avail = (ui.available_width() * 0.5).min(260.0);

    ui.label(egui::RichText::new("1:2 ratio (risk $50 → reward $100)").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    RiskRewardBar::new(50.0, 100.0).width(avail).show_label(true).show(ui, theme);

    ui.add_space(8.0);
    ui.label(egui::RichText::new("1:3 ratio (risk $30 → reward $90)").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    RiskRewardBar::new(30.0, 90.0).width(avail).show_label(true).show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── Calendar ──────────────────────────────────────────────────────────────
    story_heading(ui, theme, "Calendar");
    ui.add_space(6.0);
    Calendar::new(&mut state.cal_date).show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── DatePicker ────────────────────────────────────────────────────────────
    story_heading(ui, theme, "DatePicker");
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Click the field below to open the calendar popover:").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    DatePicker::new(&mut state.date_val)
        .placeholder("Pick a date…")
        .show(ui, theme);

    ui.add_space(12.0);
    rule(ui, theme);

    // ── TimePicker ────────────────────────────────────────────────────────────
    story_heading(ui, theme, "TimePicker");
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Click the field below to open the time picker:").color(theme.dim()).size(10.0));
    ui.add_space(4.0);
    TimePicker::new(&mut state.time_val)
        .placeholder("09:30")
        .use_24h(true)
        .id_salt("data_story_time")
        .show(ui, theme);

    ui.add_space(8.0);
}

fn story_heading(ui: &mut Ui, theme: &dyn ComponentTheme, title: &str) {
    ui.label(egui::RichText::new(title).color(theme.text()).size(12.0).strong());
}

fn rule(ui: &mut Ui, theme: &dyn ComponentTheme) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(
            theme.border().r(), theme.border().g(), theme.border().b(), 80,
        )),
    );
    ui.add_space(12.0);
}
