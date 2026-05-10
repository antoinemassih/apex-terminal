//! Indicators panel — unified management for active indicators, library
//! browsing, and chart-tool toggles. Operates on the active pane.
//!
//! The panel is additive — every indicator and toggle that exists in the legacy
//! Indicators dropdown is reachable here without removing the old menus.

use egui;
use super::super::style::*;
use super::super::widgets as widgets;
use super::super::widgets::headers::PanelHeaderWithClose;
use super::super::super::gpu::{
    Watchlist, Chart, Theme, Indicator, IndicatorType, INDICATOR_COLORS, VolumeProfileMode,
};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::Input;
use crate::ui_kit::widgets::tokens::Size as KitSize;

// ─── Toggle / picker IDs ────────────────────────────────────────────────────
//
// Every chart-level boolean we surface in the panel gets a discriminant here.
// The `bool_get`/`bool_set` accessors below route reads and writes through a
// single match statement so the rest of the panel can treat all toggles
// uniformly.

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Tg {
    // Tools
    Magnet, OhlcTip, MeasureTip, Footprint,
    PrevClose, PatternLabels, PnlCurve,
    HideAllInd, Replay,

    // Library (volume / volatility)
    VolumeBars, DeltaVolume, Rvol, VwapBands,

    // Library (overlay/MAs)
    MaRibbon, Cvd, AutoSr,

    // Library (structure)
    VolShelves, Confluence, PriceMemory, LiquidityVoids,
    AnalystTargets, PeBand, InsiderTrades, Gamma,

    // Library (regime)
    MomentumHeat, TrendStrip, BreadthTint, VolCone, CorrRibbon,

    // Library (data)
    Events, Darkpool,

    // Library (suites)
    AutoFib,
}

fn bool_get(c: &Chart, t: Tg) -> bool {
    match t {
        Tg::Magnet => c.magnet,
        Tg::OhlcTip => c.ohlc_tooltip,
        Tg::MeasureTip => c.measure_tooltip,
        Tg::Footprint => c.show_footprint,
        Tg::PrevClose => c.show_prev_close,
        Tg::PatternLabels => c.show_pattern_labels,
        Tg::PnlCurve => c.show_pnl_curve,
        Tg::HideAllInd => c.hide_all_indicators,
        Tg::Replay => c.replay_mode,

        Tg::VolumeBars => c.show_volume,
        Tg::DeltaVolume => c.show_delta_volume,
        Tg::Rvol => c.show_rvol,
        Tg::VwapBands => c.show_vwap_bands,

        Tg::MaRibbon => c.show_ma_ribbon,
        Tg::Cvd => c.show_cvd,
        Tg::AutoSr => c.show_auto_sr,

        Tg::VolShelves => c.show_vol_shelves,
        Tg::Confluence => c.show_confluence,
        Tg::PriceMemory => c.show_price_memory,
        Tg::LiquidityVoids => c.show_liquidity_voids,
        Tg::AnalystTargets => c.show_analyst_targets,
        Tg::PeBand => c.show_pe_band,
        Tg::InsiderTrades => c.show_insider_trades,
        Tg::Gamma => c.show_gamma,

        Tg::MomentumHeat => c.show_momentum_heat,
        Tg::TrendStrip => c.show_trend_strip,
        Tg::BreadthTint => c.show_breadth_tint,
        Tg::VolCone => c.show_vol_cone,
        Tg::CorrRibbon => c.show_corr_ribbon,

        Tg::Events => c.show_events,
        Tg::Darkpool => c.show_darkpool,

        Tg::AutoFib => c.show_auto_fib,
    }
}

fn bool_set(c: &mut Chart, t: Tg, v: bool) {
    match t {
        Tg::Magnet => c.magnet = v,
        Tg::OhlcTip => c.ohlc_tooltip = v,
        Tg::MeasureTip => c.measure_tooltip = v,
        Tg::Footprint => c.show_footprint = v,
        Tg::PrevClose => c.show_prev_close = v,
        Tg::PatternLabels => c.show_pattern_labels = v,
        Tg::PnlCurve => c.show_pnl_curve = v,
        Tg::HideAllInd => c.hide_all_indicators = v,
        Tg::Replay => {
            c.replay_mode = v;
            if v {
                c.replay_bar_count = c.bars.len().min(50);
                c.replay_playing = false;
                c.indicator_bar_count = 0;
            }
        }

        Tg::VolumeBars => c.show_volume = v,
        Tg::DeltaVolume => c.show_delta_volume = v,
        Tg::Rvol => c.show_rvol = v,
        Tg::VwapBands => c.show_vwap_bands = v,

        Tg::MaRibbon => c.show_ma_ribbon = v,
        Tg::Cvd => c.show_cvd = v,
        Tg::AutoSr => c.show_auto_sr = v,

        Tg::VolShelves => c.show_vol_shelves = v,
        Tg::Confluence => c.show_confluence = v,
        Tg::PriceMemory => c.show_price_memory = v,
        Tg::LiquidityVoids => c.show_liquidity_voids = v,
        Tg::AnalystTargets => c.show_analyst_targets = v,
        Tg::PeBand => c.show_pe_band = v,
        Tg::InsiderTrades => c.show_insider_trades = v,
        Tg::Gamma => c.show_gamma = v,

        Tg::MomentumHeat => c.show_momentum_heat = v,
        Tg::TrendStrip => c.show_trend_strip = v,
        Tg::BreadthTint => c.show_breadth_tint = v,
        Tg::VolCone => c.show_vol_cone = v,
        Tg::CorrRibbon => c.show_corr_ribbon = v,

        Tg::Events => c.show_events = v,
        Tg::Darkpool => c.show_darkpool = v,

        Tg::AutoFib => c.show_auto_fib = v,
    }
}

fn bool_label(t: Tg) -> &'static str {
    match t {
        Tg::Magnet => "Magnet snap",
        Tg::OhlcTip => "OHLC tooltip",
        Tg::MeasureTip => "Measure tooltip",
        Tg::Footprint => "Footprint (hover)",
        Tg::PrevClose => "Prev close / open",
        Tg::PatternLabels => "Pattern labels",
        Tg::PnlCurve => "P&L curve",
        Tg::HideAllInd => "Hide all indicators",
        Tg::Replay => "Bar replay",

        Tg::VolumeBars => "Volume bars",
        Tg::DeltaVolume => "Delta volume",
        Tg::Rvol => "Relative volume",
        Tg::VwapBands => "VWAP + bands",

        Tg::MaRibbon => "MA ribbon (8–89)",
        Tg::Cvd => "CVD",
        Tg::AutoSr => "Auto S/R levels",

        Tg::VolShelves => "Volume shelves",
        Tg::Confluence => "S/R confluence",
        Tg::PriceMemory => "Price memory",
        Tg::LiquidityVoids => "Liquidity voids",
        Tg::AnalystTargets => "Analyst targets",
        Tg::PeBand => "PE valuation band",
        Tg::InsiderTrades => "Insider trades",
        Tg::Gamma => "Gamma levels (GEX)",

        Tg::MomentumHeat => "Momentum heatmap",
        Tg::TrendStrip => "Trend alignment strip",
        Tg::BreadthTint => "Breadth tint",
        Tg::VolCone => "Volatility cone",
        Tg::CorrRibbon => "Correlation ribbon",

        Tg::Events => "Event markers",
        Tg::Darkpool => "Dark pool prints",

        Tg::AutoFib => "Auto fibonacci",
    }
}

// ─── Tool icons (cursor + display + replay) ─────────────────────────────────

fn tool_icon(t: Tg) -> &'static str {
    match t {
        Tg::Magnet => Icon::MAGNET,
        Tg::OhlcTip => Icon::CROSSHAIR,
        Tg::MeasureTip => Icon::RULER,
        Tg::Footprint => Icon::TREE_STRUCTURE,
        Tg::PrevClose => Icon::MINUS,
        Tg::PatternLabels => Icon::SPARKLE,
        Tg::PnlCurve => Icon::CHART_LINE_UP_FILL,
        Tg::HideAllInd => Icon::EYE_SLASH,
        Tg::Replay => Icon::PLAY,
        _ => Icon::DOT,
    }
}

// ─── Library items ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum LibItem {
    /// Multi- or single-instance Indicator (lives in `chart.indicators`).
    Ind(IndicatorType),
    /// Single-instance boolean flag on Chart.
    Bool(Tg),
    /// Volume Profile enum picker — its own row that opens a sub-list of modes.
    VpMode,
    /// Swing range tri-state (Off / Vertical / Diagonal).
    SwingRange,
}

struct LibSection {
    title: &'static str,
    items: &'static [LibItem],
}

const LIB_SECTIONS: &[LibSection] = &[
    LibSection { title: "Moving Averages", items: &[
        LibItem::Ind(IndicatorType::SMA),
        LibItem::Ind(IndicatorType::EMA),
        LibItem::Ind(IndicatorType::WMA),
        LibItem::Ind(IndicatorType::DEMA),
        LibItem::Ind(IndicatorType::TEMA),
        LibItem::Bool(Tg::MaRibbon),
    ]},
    LibSection { title: "Bands & Channels", items: &[
        LibItem::Ind(IndicatorType::BollingerBands),
        LibItem::Ind(IndicatorType::KeltnerChannels),
        LibItem::Bool(Tg::VwapBands),
    ]},
    LibSection { title: "Trend", items: &[
        LibItem::Ind(IndicatorType::Ichimoku),
        LibItem::Ind(IndicatorType::ParabolicSAR),
        LibItem::Ind(IndicatorType::Supertrend),
        LibItem::Ind(IndicatorType::ADX),
        LibItem::Bool(Tg::AutoSr),
    ]},
    LibSection { title: "Oscillators", items: &[
        LibItem::Ind(IndicatorType::RSI),
        LibItem::Ind(IndicatorType::MACD),
        LibItem::Ind(IndicatorType::Stochastic),
        LibItem::Ind(IndicatorType::CCI),
        LibItem::Ind(IndicatorType::WilliamsR),
        LibItem::Bool(Tg::Cvd),
    ]},
    LibSection { title: "Volume & Volatility", items: &[
        LibItem::Ind(IndicatorType::VWAP),
        LibItem::Ind(IndicatorType::ATR),
        LibItem::Bool(Tg::VolumeBars),
        LibItem::Bool(Tg::DeltaVolume),
        LibItem::Bool(Tg::Rvol),
        LibItem::VpMode,
    ]},
    LibSection { title: "Structure", items: &[
        LibItem::Bool(Tg::VolShelves),
        LibItem::Bool(Tg::Confluence),
        LibItem::Bool(Tg::PriceMemory),
        LibItem::Bool(Tg::LiquidityVoids),
        LibItem::Bool(Tg::AnalystTargets),
        LibItem::Bool(Tg::PeBand),
        LibItem::Bool(Tg::InsiderTrades),
        LibItem::Bool(Tg::Gamma),
    ]},
    LibSection { title: "Regime", items: &[
        LibItem::Bool(Tg::MomentumHeat),
        LibItem::Bool(Tg::TrendStrip),
        LibItem::Bool(Tg::BreadthTint),
        LibItem::Bool(Tg::VolCone),
        LibItem::Bool(Tg::CorrRibbon),
    ]},
    LibSection { title: "Data", items: &[
        LibItem::Bool(Tg::Events),
        LibItem::Bool(Tg::Darkpool),
    ]},
    LibSection { title: "Suites", items: &[
        LibItem::Bool(Tg::AutoFib),
        LibItem::SwingRange,
    ]},
];

/// Whether an Indicator type is single-instance (toggle) vs multi-instance (always adds).
fn is_single_instance(k: IndicatorType) -> bool {
    matches!(
        k,
        IndicatorType::VWAP
            | IndicatorType::Ichimoku
            | IndicatorType::ParabolicSAR
            | IndicatorType::Supertrend
            | IndicatorType::ADX
            | IndicatorType::ATR
    )
}

fn type_long_name(k: IndicatorType) -> &'static str {
    match k {
        IndicatorType::SMA => "Simple Moving Average",
        IndicatorType::EMA => "Exponential Moving Average",
        IndicatorType::WMA => "Weighted Moving Average",
        IndicatorType::DEMA => "Double Exponential MA",
        IndicatorType::TEMA => "Triple Exponential MA",
        IndicatorType::VWAP => "Volume Weighted Average Price",
        IndicatorType::BollingerBands => "Bollinger Bands",
        IndicatorType::Ichimoku => "Ichimoku Cloud",
        IndicatorType::ParabolicSAR => "Parabolic SAR",
        IndicatorType::Supertrend => "Supertrend",
        IndicatorType::KeltnerChannels => "Keltner Channels",
        IndicatorType::RSI => "Relative Strength Index",
        IndicatorType::MACD => "MACD",
        IndicatorType::Stochastic => "Stochastic Oscillator",
        IndicatorType::ADX => "Average Directional Index",
        IndicatorType::CCI => "Commodity Channel Index",
        IndicatorType::WilliamsR => "Williams %R",
        IndicatorType::ATR => "Average True Range",
    }
}

fn vp_label(m: VolumeProfileMode) -> &'static str {
    match m {
        VolumeProfileMode::Off => "Off",
        VolumeProfileMode::Classic => "Classic",
        VolumeProfileMode::Heatmap => "Heatmap",
        VolumeProfileMode::Strip => "Strip",
        VolumeProfileMode::Clean => "Clean (POC/VA)",
    }
}

fn swing_label(mode: u8) -> &'static str {
    match mode { 1 => "Vertical", 2 => "Diagonal", _ => "Off" }
}

// ─── Public entry ───────────────────────────────────────────────────────────

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.indicators_panel_open || panes.is_empty() {
        return;
    }
    let ap = ap.min(panes.len() - 1);

    egui::SidePanel::right("indicators_panel")
        .default_width(300.0)
        .min_width(260.0)
        .max_width(460.0)
        .resizable(true)
        .frame(widgets::frames::PanelFrame::new(t.toolbar_bg, t.toolbar_border).theme(t).build())
        .show(ctx, |ui| {
            let closed = PanelHeaderWithClose::new("INDICATORS").theme(t).watchlist(watchlist).show(ui);
            if closed {
                watchlist.indicators_panel_open = false;
                return;
            }
            separator(ui, color_alpha(t.toolbar_border, alpha_muted()));

            // ── Three resizable sections: Tools / Active / Library ──
            let avail_h = ui.available_height();
            let header_h = 26.0_f32;
            let divider_h = 8.0_f32;
            let divider_total = 2.0 * divider_h;
            let header_total = 3.0 * header_h;
            let content_h = (avail_h - divider_total - header_total).max(60.0);

            let fracs = watchlist.indicators_section_fracs;
            let sum: f32 = fracs.iter().sum();
            let norm = if sum > 0.001 { 1.0 / sum } else { 1.0 / 3.0 };
            let h_tools   = (fracs[0] * norm * content_h).max(40.0);
            let h_active  = (fracs[1] * norm * content_h).max(40.0);
            let h_library = (fracs[2] * norm * content_h).max(60.0);

            // 1. Tools
            let tools_count = active_tools_count(&panes[ap]);
            section_header(ui, "TOOLS", Some(tools_count), t);
            section_inset_body(ui, t, h_tools, "indicators_tools_scroll", |ui| {
                draw_tools_section(ui, &mut panes[ap], t);
            });

            let d1 = grippy_divider(ui, "ind_div_0", t);
            if d1 != 0.0 {
                let delta = d1 / content_h.max(1.0);
                watchlist.indicators_section_fracs[0] = (fracs[0] + delta).max(0.06);
                watchlist.indicators_section_fracs[1] = (fracs[1] - delta).max(0.06);
            }

            // 2. Active
            let active_count_total = active_count(&panes[ap]);
            section_header(ui, "ACTIVE", Some(active_count_total), t);
            section_inset_body(ui, t, h_active, "indicators_active_scroll", |ui| {
                draw_active_section(ui, &mut panes[ap], t);
            });

            let d2 = grippy_divider(ui, "ind_div_1", t);
            if d2 != 0.0 {
                let delta = d2 / content_h.max(1.0);
                watchlist.indicators_section_fracs[1] = (fracs[1] + delta).max(0.06);
                watchlist.indicators_section_fracs[2] = (fracs[2] - delta).max(0.10);
            }

            // 3. Library
            section_header(ui, "LIBRARY", None, t);
            section_inset_body(ui, t, h_library, "indicators_library_scroll", |ui| {
                draw_library_section(ui, watchlist, &mut panes[ap], t);
            });
        });
}

/// Count of every active "thing" on the chart (indicators + toggles + special).
fn active_count(c: &Chart) -> usize {
    let toggles = library_active_toggles();
    let bool_count = toggles.iter().filter(|tg| bool_get(c, **tg)).count();
    let vp = (c.vp_mode != VolumeProfileMode::Off) as usize;
    let sw = (c.swing_leg_mode > 0) as usize;
    c.indicators.len() + bool_count + vp + sw + c.symbol_overlays.len()
}

/// Count of currently-on tool toggles (cursor + display).
fn active_tools_count(c: &Chart) -> usize {
    [Tg::Magnet, Tg::OhlcTip, Tg::MeasureTip, Tg::Footprint,
     Tg::PrevClose, Tg::PatternLabels, Tg::PnlCurve,
     Tg::HideAllInd, Tg::Replay]
        .iter().filter(|tg| bool_get(c, **tg)).count()
}

/// Polished section header: vertical accent stripe, uppercase label, optional
/// count badge on the right. Reads at-a-glance like a Figma panel header.
fn section_header(ui: &mut egui::Ui, title: &str, count: Option<usize>, t: &Theme) {
    let h = 26.0_f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
    let p = ui.painter_at(rect);

    // Accent stripe (left edge), only intrudes 2px so it reads as a tab marker.
    let stripe_x = rect.left() + 1.0;
    p.line_segment(
        [egui::pos2(stripe_x, rect.top() + 6.0), egui::pos2(stripe_x, rect.bottom() - 6.0)],
        egui::Stroke::new(stroke_thick(), color_alpha(t.accent, alpha_strong())),
    );

    // Title — uppercase, monospace small, slightly emphasised.
    p.text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::monospace(font_xs()),
        t.text,
    );

    // Count chip (subtle dim) on the right.
    if let Some(n) = count {
        let chip_text = format!("{}", n);
        let galley = p.layout_no_wrap(chip_text.clone(),
            egui::FontId::monospace(font_xs()), t.dim.gamma_multiply(0.85));
        let chip_w = galley.size().x + 10.0;
        let chip_h = 14.0;
        let chip_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - chip_w - 6.0, rect.center().y - chip_h / 2.0),
            egui::vec2(chip_w, chip_h),
        );
        p.rect_filled(chip_rect, 7.0, color_alpha(t.toolbar_border, alpha_subtle()));
        p.text(
            chip_rect.center(),
            egui::Align2::CENTER_CENTER,
            chip_text,
            egui::FontId::monospace(font_xs()),
            t.dim,
        );
    }
}

/// Inset body for a section. Subtle bg + 1px hairline border so the section
/// reads as a card inset under the header.
fn section_inset_body(
    ui: &mut egui::Ui,
    t: &Theme,
    height: f32,
    id_salt: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    let bg = color_alpha(t.toolbar_border, 22);
    ui.painter().rect_filled(rect, 5.0, bg);
    ui.painter().rect_stroke(
        rect, 5.0,
        egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_subtle())),
        egui::StrokeKind::Inside,
    );

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(6.0, 6.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink([false; 2])
        .max_height(rect.height() - 12.0)
        .show(&mut child, add_contents);
}

/// Custom resize handle between sections — Figma/Framer style 6-dot grip on
/// hover. Returns drag delta in pixels.
fn grippy_divider(ui: &mut egui::Ui, _id_salt: &str, t: &Theme) -> f32 {
    let h = 8.0_f32;
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), h),
        egui::Sense::drag(),
    );
    let active = resp.hovered() || resp.dragged();
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        // Subtle hover bg
        ui.painter().rect_filled(rect, 0.0, color_alpha(t.accent, 14));
        // 6-dot grip in the centre
        let cy = rect.center().y;
        let cx = rect.center().x;
        let dot_r = 1.5_f32;
        let dot_sp = 4.0_f32;
        let dot_col = if resp.dragged() { t.accent } else { t.dim.gamma_multiply(0.9) };
        for i in -2..=2 {
            if i == 0 { continue; }
            ui.painter().circle_filled(
                egui::pos2(cx + i as f32 * dot_sp, cy - 2.0),
                dot_r, dot_col,
            );
            ui.painter().circle_filled(
                egui::pos2(cx + i as f32 * dot_sp, cy + 2.0),
                dot_r, dot_col,
            );
        }
    } else {
        // Resting state: micro hairline at the centre, barely visible
        let cy = rect.center().y;
        ui.painter().line_segment(
            [egui::pos2(rect.left() + 12.0, cy), egui::pos2(rect.right() - 12.0, cy)],
            egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_subtle())),
        );
    }
    if resp.dragged() { resp.drag_delta().y } else { 0.0 }
}

// ─── Tools section ───────────────────────────────────────────────────────────

fn draw_tools_section(ui: &mut egui::Ui, chart: &mut Chart, t: &Theme) {
    // Single Figma-style icon toolbar. Groups are separated by a hairline
    // vertical divider. Tools wrap on narrow panels.
    let groups: &[(&[Tg], &[&str])] = &[
        // Cursor group
        (&[Tg::Magnet, Tg::OhlcTip, Tg::MeasureTip, Tg::Footprint],
         &["Snap drawings to OHLC values",
           "Show OHLC values at crosshair",
           "Show distance measurement at crosshair",
           "Show footprint on hover"]),
        // Display group
        (&[Tg::PrevClose, Tg::PatternLabels, Tg::PnlCurve],
         &["Show prior session's close + open",
           "Annotate detected chart patterns",
           "Show realised P&L curve"]),
        // Mode group
        (&[Tg::Replay, Tg::HideAllInd],
         &["Step through bars chronologically",
           "Hide every indicator on this pane"]),
    ];

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (gi, (tgs, tips)) in groups.iter().enumerate() {
            for (i, &tg) in tgs.iter().enumerate() {
                tool_btn(ui, t, chart, tg, tips[i]);
            }
            if gi + 1 < groups.len() {
                tool_group_divider(ui, t);
            }
        }
    });
}

/// Compact Figma/Framer-style tool button — icon-only, square, pill-rounded.
/// Active state: accent foreground + tinted background.
fn tool_btn(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart, tg: Tg, tooltip: &str) {
    let active = bool_get(chart, tg);
    let size = egui::vec2(26.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let hovered = resp.hovered();

    let bg = if active {
        color_alpha(t.accent, alpha_tint())
    } else if hovered {
        color_alpha(t.toolbar_border, alpha_subtle())
    } else {
        egui::Color32::TRANSPARENT
    };
    let fg = if active { t.accent } else if hovered { t.text } else { t.dim };

    ui.painter().rect_filled(rect, 4.0, bg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        tool_icon(tg),
        egui::FontId::proportional(15.0),
        fg,
    );

    let resp = resp.on_hover_text(format!("{}\n{}", bool_label(tg), tooltip));
    if resp.clicked() { bool_set(chart, tg, !active); }
}

/// Vertical hairline separating tool groups.
fn tool_group_divider(ui: &mut egui::Ui, t: &Theme) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, 24.0), egui::Sense::hover());
    let cx = rect.center().x;
    ui.painter().line_segment(
        [egui::pos2(cx, rect.top() + 5.0), egui::pos2(cx, rect.bottom() - 5.0)],
        egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_muted())),
    );
}

// ─── Active section ─────────────────────────────────────────────────────────

fn draw_active_section(ui: &mut egui::Ui, chart: &mut Chart, t: &Theme) {
    // Count active = real indicators + active boolean toggles + non-Off VP + non-Off swing
    let active_toggles = library_active_toggles();
    let active_bool_count = active_toggles.iter().filter(|tg| bool_get(chart, **tg)).count();
    let vp_active = chart.vp_mode != VolumeProfileMode::Off;
    let swing_active = chart.swing_leg_mode > 0;
    let overlay_count = chart.symbol_overlays.len();
    let total_active = chart.indicators.len()
        + active_bool_count
        + (vp_active as usize)
        + (swing_active as usize)
        + overlay_count;

    if total_active == 0 {
        ui.label(egui::RichText::new("No indicators or overlays on this pane.")
            .monospace().size(font_sm()).color(color_subtle(t.dim)));
        ui.add_space(4.0);
        add_overlay_button(ui, t, chart);
        return;
    }

    // Indicators (real entries with id)
    let mut remove_id: Option<u32> = None;
    let mut edit_id: Option<u32> = None;
    for ind in chart.indicators.iter_mut() {
        active_indicator_row(ui, t, ind, &mut remove_id, &mut edit_id);
    }
    if let Some(id) = remove_id { chart.indicators.retain(|i| i.id != id); }
    if let Some(id) = edit_id { chart.editing_indicator = Some(id); }

    // Active boolean toggles
    let mut to_disable: Option<Tg> = None;
    for tg in &active_toggles {
        if bool_get(chart, *tg) {
            active_bool_row(ui, t, *tg, &mut to_disable);
        }
    }
    if let Some(tg) = to_disable { bool_set(chart, tg, false); }

    // Volume Profile (non-Off) + Swing range (non-Off) — special multi-state rows
    if vp_active { active_vp_row(ui, t, chart); }
    if swing_active { active_swing_row(ui, t, chart); }

    // ── Symbol overlays ──────────────────────────────────────────────────
    // Each is a separate ticker overlaid on the chart. The actual add/edit
    // dialog lives in `overlay_manager.rs`; we just open it via the
    // overlay_editing/overlay_editing_idx state pair.
    let mut overlay_remove: Option<usize> = None;
    let mut overlay_edit: Option<usize> = None;
    for i in 0..chart.symbol_overlays.len() {
        active_symbol_overlay_row(ui, t, chart, i, &mut overlay_remove, &mut overlay_edit);
    }
    if let Some(i) = overlay_remove {
        chart.symbol_overlays.remove(i);
    }
    if let Some(i) = overlay_edit {
        chart.overlay_editing = true;
        chart.overlay_editing_idx = Some(i);
        chart.overlay_input = chart.symbol_overlays[i].symbol.clone();
    }

    // Add-overlay affordance — always rendered after the list.
    ui.add_space(4.0);
    add_overlay_button(ui, t, chart);
}

fn active_symbol_overlay_row(
    ui: &mut egui::Ui,
    t: &Theme,
    chart: &mut Chart,
    idx: usize,
    remove: &mut Option<usize>,
    edit: &mut Option<usize>,
) {
    let (symbol, color_hex, visible, loading) = {
        let ov = &chart.symbol_overlays[idx];
        (ov.symbol.clone(), ov.color.clone(), ov.visible, ov.loading)
    };
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        let swatch_size = 10.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(swatch_size, swatch_size), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, hex_to_color(&color_hex, 1.0));
        ui.add_space(4.0);

        let label = if loading { format!("{} (loading…)", symbol) } else { symbol.clone() };
        let txt_color = if visible { t.text } else { color_half(t.dim) };
        let label_resp = ui.label(egui::RichText::new(label).monospace().size(font_sm()).color(txt_color));
        if label_resp.double_clicked() { *edit = Some(idx); }

        // Tag the row with "OV" so users distinguish overlays from indicators
        ui.label(egui::RichText::new("OV").monospace().size(font_xs()).color(color_muted(t.dim)));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if mini_btn(ui, t, "\u{00D7}", "Remove overlay", false).clicked() {
                *remove = Some(idx);
            }
            if mini_btn(ui, t, Icon::PENCIL_LINE, "Edit symbol / color", false).clicked() {
                *edit = Some(idx);
            }
            let eye = if visible { Icon::EYE } else { Icon::EYE_SLASH };
            if mini_btn(ui, t, eye, "Show / hide", visible).clicked() {
                chart.symbol_overlays[idx].visible = !visible;
            }
        });
    });
}

fn add_overlay_button(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart) {
    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(format!("{}  Add symbol overlay", Icon::PLUS))
                .monospace().size(font_sm()).color(t.dim))
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_muted())))
            .corner_radius(3.0)
            .min_size(egui::vec2(ui.available_width(), 22.0)),
    );
    if resp.clicked() {
        chart.overlay_editing = true;
        chart.overlay_editing_idx = None;
        chart.overlay_input.clear();
    }
}

fn library_active_toggles() -> Vec<Tg> {
    // Toggles that appear in the Library and should also surface in Active when on.
    // Tools-only toggles intentionally stay out of Active to keep that section
    // focused on chart overlays / indicators.
    vec![
        Tg::VolumeBars, Tg::DeltaVolume, Tg::Rvol, Tg::VwapBands,
        Tg::MaRibbon, Tg::Cvd, Tg::AutoSr,
        Tg::VolShelves, Tg::Confluence, Tg::PriceMemory, Tg::LiquidityVoids,
        Tg::AnalystTargets, Tg::PeBand, Tg::InsiderTrades, Tg::Gamma,
        Tg::MomentumHeat, Tg::TrendStrip, Tg::BreadthTint, Tg::VolCone, Tg::CorrRibbon,
        Tg::Events, Tg::Darkpool,
        Tg::AutoFib,
    ]
}

fn active_indicator_row(
    ui: &mut egui::Ui,
    t: &Theme,
    ind: &mut Indicator,
    remove_id: &mut Option<u32>,
    edit_id: &mut Option<u32>,
) {
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        let swatch_size = 10.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(swatch_size, swatch_size), egui::Sense::hover());
        let col = hex_to_color(&ind.color, 1.0);
        ui.painter().rect_filled(rect, 2.0, col);
        ui.add_space(4.0);
        let txt_color = if ind.visible { t.text } else { color_half(t.dim) };
        ui.label(egui::RichText::new(ind.display_name()).monospace().size(font_sm()).color(txt_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if mini_btn(ui, t, "\u{00D7}", "Remove", false).clicked() { *remove_id = Some(ind.id); }
            if mini_btn(ui, t, Icon::GEAR, "Edit", false).clicked() { *edit_id = Some(ind.id); }
            let eye = if ind.visible { Icon::EYE } else { Icon::EYE_SLASH };
            if mini_btn(ui, t, eye, "Show / hide", ind.visible).clicked() { ind.visible = !ind.visible; }
        });
    });
}

fn active_bool_row(ui: &mut egui::Ui, t: &Theme, tg: Tg, to_disable: &mut Option<Tg>) {
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        // Small accent-colored pip as a "swatch" stand-in
        let swatch_size = 10.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(swatch_size, swatch_size), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color_alpha(t.accent, alpha_strong()));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(bool_label(tg)).monospace().size(font_sm()).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if mini_btn(ui, t, "\u{00D7}", "Disable", false).clicked() { *to_disable = Some(tg); }
        });
    });
}

fn active_vp_row(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart) {
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color_alpha(t.accent, alpha_strong()));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Volume profile · {}", vp_label(chart.vp_mode)))
            .monospace().size(font_sm()).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if mini_btn(ui, t, "\u{00D7}", "Disable", false).clicked() {
                chart.vp_mode = VolumeProfileMode::Off; chart.vp_data = None;
            }
        });
    });
}

fn active_swing_row(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart) {
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color_alpha(t.accent, alpha_strong()));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("SwingRange · {}", swing_label(chart.swing_leg_mode)))
            .monospace().size(font_sm()).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if mini_btn(ui, t, "\u{00D7}", "Disable", false).clicked() { chart.swing_leg_mode = 0; }
        });
    });
}

fn mini_btn(ui: &mut egui::Ui, t: &Theme, icon: &str, tip: &str, active: bool) -> egui::Response {
    let col = if active { t.text } else { color_subtle(t.dim) };
    ui.add(egui::Button::new(egui::RichText::new(icon).size(font_md()).color(col))
        .fill(egui::Color32::TRANSPARENT)
        .min_size(egui::vec2(20.0, 20.0)))
        .on_hover_text(tip)
}

// ─── Library section ────────────────────────────────────────────────────────

fn draw_library_section(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    chart: &mut Chart,
    t: &Theme,
) {
    ui.horizontal(|ui| {
        Input::new(&mut watchlist.indicators_panel_search)
            .leading_icon(Icon::MAGNIFYING_GLASS)
            .placeholder("Search…")
            .size(KitSize::Sm)
            .clearable(true)
            .full_width()
            .show(ui, t);
    });
    ui.add_space(4.0);

    let query = watchlist.indicators_panel_search.trim().to_lowercase();
    let force_open = !query.is_empty();

    // Build the visible-section list first so we know where to draw dividers
    // (only between *visible* sections, not after empty ones).
    let visible: Vec<(usize, Vec<&LibItem>)> = LIB_SECTIONS
        .iter()
        .enumerate()
        .filter_map(|(i, sec)| {
            let m: Vec<&LibItem> = sec.items.iter().filter(|item| matches_query(**item, &query)).collect();
            if m.is_empty() { None } else { Some((i, m)) }
        })
        .collect();

    for (vi, (sec_idx, matches)) in visible.iter().enumerate() {
        let sec = &LIB_SECTIONS[*sec_idx];
        let key = sec.title.to_string();
        let collapsed = !force_open && watchlist.indicators_lib_collapsed.contains(&key);

        // ── Clickable header ─────────────────────────────────────────────
        let header_h = 24.0;
        let (h_rect, h_resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), header_h),
            egui::Sense::click(),
        );
        let hovered = h_resp.hovered();
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(h_rect, 3.0, color_alpha(t.toolbar_border, alpha_subtle()));
        }
        let painter = ui.painter_at(h_rect);
        let cy = h_rect.center().y;
        // Caret: monospace + Phosphor (matches Icon constants which are
        // mapped into the monospace family in init_fonts).
        let caret = if collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
        painter.text(
            egui::pos2(h_rect.left() + 8.0, cy),
            egui::Align2::LEFT_CENTER,
            caret,
            egui::FontId::monospace(font_sm()),
            t.dim,
        );
        painter.text(
            egui::pos2(h_rect.left() + 22.0, cy),
            egui::Align2::LEFT_CENTER,
            sec.title,
            egui::FontId::monospace(font_sm()),
            if hovered { t.text } else { t.text.gamma_multiply(0.92) },
        );
        // Count chip — pill-shaped, mirrors the section-header chip.
        let chip_text = format!("{}", matches.len());
        let galley = painter.layout_no_wrap(chip_text.clone(),
            egui::FontId::monospace(font_xs()), t.dim);
        let chip_w = galley.size().x + 10.0;
        let chip_h = 14.0;
        let chip_rect = egui::Rect::from_min_size(
            egui::pos2(h_rect.right() - chip_w - 6.0, cy - chip_h / 2.0),
            egui::vec2(chip_w, chip_h),
        );
        painter.rect_filled(chip_rect, 7.0, color_alpha(t.toolbar_border, alpha_subtle()));
        painter.text(chip_rect.center(), egui::Align2::CENTER_CENTER,
            chip_text, egui::FontId::monospace(font_xs()), t.dim);

        if h_resp.clicked() && !force_open {
            if collapsed { watchlist.indicators_lib_collapsed.remove(&key); }
            else { watchlist.indicators_lib_collapsed.insert(key); }
        }

        // ── Inset body ───────────────────────────────────────────────────
        if !collapsed {
            let row_h: f32 = 22.0;
            let body_h_estimate: f32 = matches.len() as f32 * row_h + 6.0;
            let (body_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), body_h_estimate),
                egui::Sense::hover(),
            );
            // Body inset: clearly visible alpha + 1px hairline so the body
            // reads as a card under the header, not a flush continuation.
            ui.painter().rect_filled(
                body_rect, 3.0,
                color_alpha(t.toolbar_border, 28),
            );
            ui.painter().rect_stroke(
                body_rect, 3.0,
                egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_subtle())),
                egui::StrokeKind::Inside,
            );

            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(body_rect.shrink2(egui::vec2(3.0, 3.0)))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            for item in matches {
                lib_row(&mut child, t, **item, chart);
            }
        }

        // ── Divider between accordion sections ───────────────────────────
        // Inset hairline that sits between sections without touching the
        // panel edges — Figma-style architectural division.
        if vi + 1 < visible.len() {
            ui.add_space(4.0);
            let (div_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().line_segment(
                [egui::pos2(div_rect.left() + 8.0, div_rect.center().y),
                 egui::pos2(div_rect.right() - 8.0, div_rect.center().y)],
                egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_muted())),
            );
            ui.add_space(4.0);
        }
    }
}

fn matches_query(item: LibItem, q: &str) -> bool {
    if q.is_empty() { return true; }
    let (short, long) = match item {
        LibItem::Ind(k) => (k.label().to_string(), type_long_name(k).to_string()),
        LibItem::Bool(tg) => (bool_label(tg).to_string(), bool_label(tg).to_string()),
        LibItem::VpMode => ("VP".into(), "Volume profile".into()),
        LibItem::SwingRange => ("SR".into(), "Swing range".into()),
    };
    short.to_lowercase().contains(q) || long.to_lowercase().contains(q)
}

fn lib_row(ui: &mut egui::Ui, t: &Theme, item: LibItem, chart: &mut Chart) {
    match item {
        LibItem::Ind(k) => lib_ind_row(ui, t, k, chart),
        LibItem::Bool(tg) => lib_bool_row(ui, t, tg, chart),
        LibItem::VpMode => lib_vp_row(ui, t, chart),
        LibItem::SwingRange => lib_swing_row(ui, t, chart),
    }
}

fn lib_ind_row(ui: &mut egui::Ui, t: &Theme, kind: IndicatorType, chart: &mut Chart) {
    let single = is_single_instance(kind);
    let count = chart.indicators.iter().filter(|i| i.kind == kind).count();
    let active = count > 0;

    let row_h = 22.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::click());
    let hovered = resp.hovered();
    let bg = if single && active {
        color_alpha(t.accent, alpha_tint())
    } else if hovered {
        color_alpha(t.toolbar_border, alpha_subtle())
    } else { egui::Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 3.0, bg);
    if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

    let painter = ui.painter_at(rect);
    let cy = rect.center().y;
    let label_x = rect.left() + 16.0;
    let label_col = if single && active { t.accent } else { t.text };
    painter.text(egui::pos2(label_x, cy), egui::Align2::LEFT_CENTER,
        kind.label(), egui::FontId::monospace(font_sm()), label_col);
    let tag_w = 50.0;
    painter.text(egui::pos2(label_x + tag_w, cy), egui::Align2::LEFT_CENTER,
        type_long_name(kind), egui::FontId::monospace(font_xs()), t.dim);

    let right_x = rect.right() - 8.0;
    if single {
        let lbl = if active { "ON" } else { "+" };
        let col = if active { t.accent } else { t.dim };
        painter.text(egui::pos2(right_x, cy), egui::Align2::RIGHT_CENTER, lbl, egui::FontId::monospace(font_xs()), col);
    } else if count > 0 {
        painter.text(egui::pos2(right_x, cy), egui::Align2::RIGHT_CENTER,
            format!("+ ({})", count), egui::FontId::monospace(font_xs()), t.dim.gamma_multiply(0.8));
    } else {
        painter.text(egui::pos2(right_x, cy), egui::Align2::RIGHT_CENTER,
            "+", egui::FontId::monospace(font_md()), t.dim);
    }

    if resp.clicked() {
        if single && active {
            chart.indicators.retain(|i| i.kind != kind);
            chart.indicator_bar_count = 0;
        } else {
            let id = chart.next_indicator_id; chart.next_indicator_id += 1;
            let color = INDICATOR_COLORS[chart.indicators.len() % INDICATOR_COLORS.len()];
            let mut ind = Indicator::new(id, kind, kind.default_period(), color);
            ind.visible = true;
            chart.indicators.push(ind);
            chart.indicator_bar_count = 0;
        }
    }
}

fn lib_bool_row(ui: &mut egui::Ui, t: &Theme, tg: Tg, chart: &mut Chart) {
    let active = bool_get(chart, tg);
    let row_h = 22.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::click());
    let hovered = resp.hovered();
    let bg = if active { color_alpha(t.accent, alpha_tint()) }
        else if hovered { color_alpha(t.toolbar_border, alpha_subtle()) }
        else { egui::Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 3.0, bg);
    if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

    let painter = ui.painter_at(rect);
    let cy = rect.center().y;
    let label_x = rect.left() + 16.0;
    let label_col = if active { t.accent } else { t.text };
    painter.text(egui::pos2(label_x, cy), egui::Align2::LEFT_CENTER,
        bool_label(tg), egui::FontId::monospace(font_sm()), label_col);

    let right_x = rect.right() - 8.0;
    let lbl = if active { "ON" } else { "+" };
    let col = if active { t.accent } else { t.dim };
    painter.text(egui::pos2(right_x, cy), egui::Align2::RIGHT_CENTER,
        lbl, egui::FontId::monospace(font_xs()), col);

    if resp.clicked() { bool_set(chart, tg, !active); }
}

fn lib_vp_row(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart) {
    let active = chart.vp_mode != VolumeProfileMode::Off;
    let row_h = 22.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::click());
    let hovered = resp.hovered();
    let bg = if active { color_alpha(t.accent, alpha_tint()) }
        else if hovered { color_alpha(t.toolbar_border, alpha_subtle()) }
        else { egui::Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 3.0, bg);
    if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

    let painter = ui.painter_at(rect);
    let cy = rect.center().y;
    let label_x = rect.left() + 16.0;
    let label_col = if active { t.accent } else { t.text };
    painter.text(egui::pos2(label_x, cy), egui::Align2::LEFT_CENTER,
        "Volume profile", egui::FontId::monospace(font_sm()), label_col);

    let right_x = rect.right() - 8.0;
    let lbl = if active { vp_label(chart.vp_mode) } else { "+" };
    let col = if active { t.accent } else { t.dim };
    painter.text(egui::pos2(right_x, cy), egui::Align2::RIGHT_CENTER,
        lbl, egui::FontId::monospace(font_xs()), col);

    if resp.clicked() {
        // Cycle Off → Classic → Heatmap → Strip → Clean → Off
        chart.vp_mode = match chart.vp_mode {
            VolumeProfileMode::Off => VolumeProfileMode::Classic,
            VolumeProfileMode::Classic => VolumeProfileMode::Heatmap,
            VolumeProfileMode::Heatmap => VolumeProfileMode::Strip,
            VolumeProfileMode::Strip => VolumeProfileMode::Clean,
            VolumeProfileMode::Clean => VolumeProfileMode::Off,
        };
        chart.vp_data = None;
    }
}

fn lib_swing_row(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart) {
    let active = chart.swing_leg_mode > 0;
    let row_h = 22.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::click());
    let hovered = resp.hovered();
    let bg = if active { color_alpha(t.accent, alpha_tint()) }
        else if hovered { color_alpha(t.toolbar_border, alpha_subtle()) }
        else { egui::Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 3.0, bg);
    if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

    let painter = ui.painter_at(rect);
    let cy = rect.center().y;
    let label_x = rect.left() + 16.0;
    let label_col = if active { t.accent } else { t.text };
    painter.text(egui::pos2(label_x, cy), egui::Align2::LEFT_CENTER,
        "SwingRange", egui::FontId::monospace(font_sm()), label_col);

    let right_x = rect.right() - 8.0;
    let lbl = if active { swing_label(chart.swing_leg_mode) } else { "+" };
    let col = if active { t.accent } else { t.dim };
    painter.text(egui::pos2(right_x, cy), egui::Align2::RIGHT_CENTER,
        lbl, egui::FontId::monospace(font_xs()), col);

    if resp.clicked() { chart.swing_leg_mode = (chart.swing_leg_mode + 1) % 3; }
}
