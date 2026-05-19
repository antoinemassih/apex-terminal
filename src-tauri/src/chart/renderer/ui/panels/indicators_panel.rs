//! Indicators panel — unified management for active indicators, library
//! browsing, and chart-tool toggles. Operates on the active pane.
//!
//! Migrated to the canonical ui_kit side-panel primitives:
//!   - `SidePanelShell` for outer chrome (was hand-rolled SidePanel + PanelFrame + PanelHeader)
//!   - `ui_kit::PanelSection` for TOOLS / ACTIVE / LIBRARY headers (default
//!     `.rule(true)` hairline, optional count badge)
//!   - `ui_kit::PanelListRow` for active rows (leading color swatch + primary
//!     label + trailing visibility / delete cluster) and library rows
//!     (selected state encodes "on", trailing slot carries the +/ON affordance)
//!   - `ui_kit::Button` for the tool toggles (`Button::toggle`), the inline
//!     close-X (`Button::close`), and the add-symbol-overlay row
//!     (`Button::simple` full-width)
//!
//! Indicator definitions (`Tg`, `LibItem`, `LIB_SECTIONS`, `bool_get/set`,
//! etc.) are unchanged — this migration only touches the chrome around them.

use egui;
use super::super::style::*;
use crate::ui_kit::widgets::{
    Button, PanelEmpty, PanelListRow, PanelSection, PanelSectionGroup, SidePanelShell, Tooltip, Width,
};
use crate::ui_kit::widgets::tokens::Size as KitSize;
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::widgets::Input;
use super::super::super::gpu::{
    Watchlist, Chart, Theme, Indicator, IndicatorType, INDICATOR_COLORS, VolumeProfileMode,
};
use crate::ui_kit::icons::Icon;

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

    let resp = SidePanelShell::new("indicators_panel", "INDICATORS")
        .width(Width::Medium)
        .pane_metrics(
            crate::chart_renderer::gpu::pane_tabs_header_h(watchlist),
            watchlist.pane_header_size.title_font(),
        )
        .show(ctx, t, |ui, t| {
            // 3-way user-resizable split (TOOLS / ACTIVE / LIBRARY). The
            // section fractions persist on the Watchlist so the user's
            // chosen ratio survives panel close / app restart.
            let tools_count = active_tools_count(&panes[ap]);
            let active_total = active_count(&panes[ap]);
            // Snapshot section fractions into a local so the group can
            // own the &mut without locking out the body closures that
            // also need &mut watchlist (for library search state).
            let mut fracs = watchlist.indicators_section_fracs;
            let chart = &mut panes[ap];
            PanelSectionGroup::new(&mut fracs)
                .min_section_height(40.0)
                .show(ui, t, |grp| {
                    // ── TOOLS ──
                    grp.section(|ui, t| {
                        PanelSection::new("TOOLS")
                            .count(tools_count)
                            .show(ui, t, |ui, t| {
                                draw_tools_section(ui, chart, t);
                            });
                    });
                    // ── ACTIVE ──
                    grp.section(|ui, t| {
                        PanelSection::new("ACTIVE")
                            .count(active_total)
                            .show(ui, t, |ui, t| {
                                egui::ScrollArea::vertical()
                                    .id_salt("indicators_active_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        draw_active_section(ui, chart, t);
                                    });
                            });
                    });
                    // ── LIBRARY ──
                    grp.section(|ui, t| {
                        PanelSection::new("LIBRARY")
                            .show(ui, t, |ui, t| {
                                egui::ScrollArea::vertical()
                                    .id_salt("indicators_library_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        draw_library_section(ui, watchlist, chart, t);
                                    });
                            });
                    });
                });
            watchlist.indicators_section_fracs = fracs;
        });

    if resp.close_clicked {
        watchlist.indicators_panel_open = false;
    }
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

// ─── Tools section ───────────────────────────────────────────────────────────

fn draw_tools_section(ui: &mut egui::Ui, chart: &mut Chart, t: &Theme) {
    // Single icon-toggle toolbar. Groups separated by hairline vertical dividers.
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

/// Compact icon-only toggle button — uses the canonical `Button::toggle`
/// preset so the active accent tint matches every other toggle chip in the app.
fn tool_btn(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart, tg: Tg, tooltip: &str) {
    let active = bool_get(chart, tg);
    // Icon-only toggle: use Button::icon() so leading_icon is populated
    // (the icon_only paint path only renders leading_icon, not label).
    // Then layer Variant::Toggle on top so the active state matches
    // every other toggle chip in the app.
    let resp = Button::icon(tool_icon(tg))
        .variant(crate::ui_kit::widgets::tokens::Variant::Toggle)
        .active(active)
        .size(KitSize::Sm)
        .min_size(egui::vec2(26.0, row_height_spacious()))
        .show(ui, t)
        .on_hover_text(format!("{}\n{}", bool_label(tg), tooltip));
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
        PanelEmpty::new("No indicators or overlays on this pane.").show(ui, t);
        ui.add_space(gap_xs());
        add_overlay_button(ui, t, chart);
        return;
    }

    // Indicators (real entries with id)
    let bar_count = chart.bars.len();
    let mut remove_id: Option<u32> = None;
    let mut edit_id: Option<u32> = None;
    for ind in chart.indicators.iter_mut() {
        active_indicator_row(ui, t, ind, bar_count, &mut remove_id, &mut edit_id);
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

    // Symbol overlays
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

    ui.add_space(gap_xs());
    add_overlay_button(ui, t, chart);
}

/// Paint a 10px color swatch into the leading slot.
fn paint_swatch(ui: &mut egui::Ui, color: egui::Color32) {
    let swatch_size = 10.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(swatch_size, swatch_size), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, color);
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
    let primary = if loading { format!("{} (loading…)", symbol) } else { symbol.clone() };
    let id_salt = format!("ov_{}", idx);
    let swatch_col = hex_to_color(&color_hex, 1.0);
    let secondary = "OV";

    let want_remove = std::cell::Cell::new(false);
    let want_edit = std::cell::Cell::new(false);
    let want_toggle_vis = std::cell::Cell::new(false);
    let r_ref = &want_remove;
    let e_ref = &want_edit;
    let v_ref = &want_toggle_vis;

    PanelListRow::new(&id_salt)
        .leading(move |ui, _t| { paint_swatch(ui, swatch_col); })
        .primary(&primary)
        .secondary(secondary)
        .trailing(move |ui, t| {
            let r = Button::close().placement(IconPlacement::ListRow).show(ui, t);
            Tooltip::new("Remove overlay").show(ui, &r, t);
            if r.clicked() { r_ref.set(true); }
            let r = Button::icon(Icon::PENCIL_LINE)
                .size(KitSize::Xs)
                .placement(IconPlacement::ListRow)
                .show(ui, t);
            Tooltip::new("Edit symbol / color").show(ui, &r, t);
            if r.clicked() { e_ref.set(true); }
            let eye = if visible { Icon::EYE } else { Icon::EYE_SLASH };
            let r = Button::icon(eye)
                .size(KitSize::Xs)
                .active(visible)
                .placement(IconPlacement::ListRow)
                .show(ui, t);
            Tooltip::new("Show / hide").show(ui, &r, t);
            if r.clicked() { v_ref.set(true); }
        })
        .dense(false)
        .show(ui, t);

    if want_remove.get() { *remove = Some(idx); }
    if want_edit.get() { *edit = Some(idx); }
    if want_toggle_vis.get() { chart.symbol_overlays[idx].visible = !visible; }
}

fn add_overlay_button(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart) {
    let label = format!("{}  Add symbol overlay", Icon::PLUS);
    let resp = Button::outline_full_width(label.as_str()).show(ui, t);
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

/// Minimum number of bars needed before the indicator produces meaningful output.
/// RSI(14) needs 14+1=15; MACD(12,26,9) needs 26+9=35; others use period+1.
fn indicator_min_bars(ind: &Indicator) -> usize {
    match ind.kind {
        IndicatorType::RSI => ind.period + 1,
        IndicatorType::MACD => 26 + 9, // slow EMA + signal smoothing
        _ => ind.period + 1,
    }
}

fn active_indicator_row(
    ui: &mut egui::Ui,
    t: &Theme,
    ind: &mut Indicator,
    bar_count: usize,
    remove_id: &mut Option<u32>,
    edit_id: &mut Option<u32>,
) {
    let id = ind.id;
    let visible = ind.visible;
    let swatch_col = hex_to_color(&ind.color, 1.0);
    let name = ind.display_name();
    let id_salt = format!("ind_{}", id);

    // Show a warning state if insufficient bars are loaded for this indicator.
    let min = indicator_min_bars(ind);
    if bar_count < min {
        let title = format!("{} needs {} bars", name, min);
        let hint = format!("{} loaded", bar_count);
        PanelEmpty::new(&title).hint(&hint).show(ui, t);
        return;
    }

    let want_remove = std::cell::Cell::new(false);
    let want_edit = std::cell::Cell::new(false);
    let want_toggle_vis = std::cell::Cell::new(false);
    let r_ref = &want_remove;
    let e_ref = &want_edit;
    let v_ref = &want_toggle_vis;

    PanelListRow::new(&id_salt)
        .leading(move |ui, _t| { paint_swatch(ui, swatch_col); })
        .primary(&name)
        .trailing(move |ui, t| {
            let r = Button::close().placement(IconPlacement::ListRow).show(ui, t);
            Tooltip::new("Remove indicator").show(ui, &r, t);
            if r.clicked() { r_ref.set(true); }
            let r = Button::icon(Icon::GEAR)
                .size(KitSize::Xs)
                .placement(IconPlacement::ListRow)
                .show(ui, t);
            Tooltip::new("Edit").show(ui, &r, t);
            if r.clicked() { e_ref.set(true); }
            let eye = if visible { Icon::EYE } else { Icon::EYE_SLASH };
            let r = Button::icon(eye)
                .size(KitSize::Xs)
                .active(visible)
                .placement(IconPlacement::ListRow)
                .show(ui, t);
            Tooltip::new("Show / hide").show(ui, &r, t);
            if r.clicked() { v_ref.set(true); }
        })
        .show(ui, t);

    if want_remove.get() { *remove_id = Some(id); }
    if want_edit.get() { *edit_id = Some(id); }
    if want_toggle_vis.get() { ind.visible = !visible; }
}

fn active_bool_row(ui: &mut egui::Ui, t: &Theme, tg: Tg, to_disable: &mut Option<Tg>) {
    let accent = color_alpha(t.accent, alpha_strong());
    let id_salt = format!("act_bool_{:?}", tg as u8 as usize);
    let want_disable = std::cell::Cell::new(false);
    let d_ref = &want_disable;
    PanelListRow::new(&id_salt)
        .leading(move |ui, _t| { paint_swatch(ui, accent); })
        .primary(bool_label(tg))
        .trailing(move |ui, t| {
            let r = Button::close().placement(IconPlacement::ListRow).show(ui, t);
            Tooltip::new("Disable").show(ui, &r, t);
            if r.clicked() { d_ref.set(true); }
        })
        .show(ui, t);
    if want_disable.get() { *to_disable = Some(tg); }
}

fn active_vp_row(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart) {
    let accent = color_alpha(t.accent, alpha_strong());
    let primary = format!("Volume profile · {}", vp_label(chart.vp_mode));
    let want_disable = std::cell::Cell::new(false);
    let d_ref = &want_disable;
    PanelListRow::new("act_vp")
        .leading(move |ui, _t| { paint_swatch(ui, accent); })
        .primary(&primary)
        .trailing(move |ui, t| {
            let r = Button::close().placement(IconPlacement::ListRow).show(ui, t);
            Tooltip::new("Disable").show(ui, &r, t);
            if r.clicked() { d_ref.set(true); }
        })
        .show(ui, t);
    if want_disable.get() {
        chart.vp_mode = VolumeProfileMode::Off; chart.vp_data = None;
    }
}

fn active_swing_row(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart) {
    let accent = color_alpha(t.accent, alpha_strong());
    let primary = format!("SwingRange · {}", swing_label(chart.swing_leg_mode));
    let want_disable = std::cell::Cell::new(false);
    let d_ref = &want_disable;
    PanelListRow::new("act_swing")
        .leading(move |ui, _t| { paint_swatch(ui, accent); })
        .primary(&primary)
        .trailing(move |ui, t| {
            let r = Button::close().placement(IconPlacement::ListRow).show(ui, t);
            Tooltip::new("Disable").show(ui, &r, t);
            if r.clicked() { d_ref.set(true); }
        })
        .show(ui, t);
    if want_disable.get() { chart.swing_leg_mode = 0; }
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
    ui.add_space(gap_xs());

    let query = watchlist.indicators_panel_search.trim().to_lowercase();
    let force_open = !query.is_empty();

    // Build visible-section list first so we know where to draw dividers.
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

        // ── Clickable category header (caret + title + count chip) ──
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
        let caret = if collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
        painter.text(
            egui::pos2(h_rect.left() + 8.0, cy),
            egui::Align2::LEFT_CENTER,
            caret,
            mono_sm(),
            t.dim,
        );
        painter.text(
            egui::pos2(h_rect.left() + 22.0, cy),
            egui::Align2::LEFT_CENTER,
            sec.title,
            mono_sm(),
            if hovered { t.text } else { color_subtle(t.text) },
        );
        let chip_text = format!("{}", matches.len());
        let galley = painter.layout_no_wrap(chip_text.clone(),
            mono_xs(), t.dim);
        let chip_w = galley.size().x + 10.0;
        let chip_h = 14.0;
        let chip_rect = egui::Rect::from_min_size(
            egui::pos2(h_rect.right() - chip_w - 6.0, cy - chip_h / 2.0),
            egui::vec2(chip_w, chip_h),
        );
        painter.rect_filled(chip_rect, 7.0, color_alpha(t.toolbar_border, alpha_subtle()));
        painter.text(chip_rect.center(), egui::Align2::CENTER_CENTER,
            chip_text, mono_xs(), t.dim);

        if h_resp.clicked() && !force_open {
            if collapsed { watchlist.indicators_lib_collapsed.remove(&key); }
            else { watchlist.indicators_lib_collapsed.insert(key); }
        }

        // ── Body — rows via PanelListRow (selected state encodes "on") ──
        if !collapsed {
            for item in matches {
                lib_row(ui, t, **item, chart, *sec_idx);
            }
        }

        // ── Inset hairline divider between visible category sections ──
        if vi + 1 < visible.len() {
            ui.add_space(gap_xs());
            let (div_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().line_segment(
                [egui::pos2(div_rect.left() + 8.0, div_rect.center().y),
                 egui::pos2(div_rect.right() - 8.0, div_rect.center().y)],
                egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_muted())),
            );
            ui.add_space(gap_xs());
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

fn lib_row(ui: &mut egui::Ui, t: &Theme, item: LibItem, chart: &mut Chart, sec_idx: usize) {
    match item {
        LibItem::Ind(k) => lib_ind_row(ui, t, k, chart, sec_idx),
        LibItem::Bool(tg) => lib_bool_row(ui, t, tg, chart, sec_idx),
        LibItem::VpMode => lib_vp_row(ui, t, chart, sec_idx),
        LibItem::SwingRange => lib_swing_row(ui, t, chart, sec_idx),
    }
}

/// Build a stable per-row id_salt that uniquely keys this library row
/// across panel sections (since the same item enum could in principle appear
/// twice).
fn lib_id(sec_idx: usize, tag: &str) -> String {
    format!("lib_{}_{}", sec_idx, tag)
}

fn lib_ind_row(ui: &mut egui::Ui, t: &Theme, kind: IndicatorType, chart: &mut Chart, sec_idx: usize) {
    let single = is_single_instance(kind);
    let count = chart.indicators.iter().filter(|i| i.kind == kind).count();
    let active = count > 0;
    // For multi-instance: never show selected state (each click adds a new one).
    let selected = single && active;
    let id_salt = lib_id(sec_idx, kind.label());
    let trail_text: String = if single {
        if active { "ON".into() } else { "+".into() }
    } else if count > 0 {
        format!("+ ({})", count)
    } else {
        "+".into()
    };
    let trail_col = if active { t.accent } else { t.dim };
    let label = kind.label();
    let long = type_long_name(kind);
    let resp = PanelListRow::new(&id_salt)
        .primary(label)
        .secondary(long)
        .selected(selected)
        .divided(true)
        .trailing(move |ui, _t| {
            ui.label(
                egui::RichText::new(&trail_text)
                    .monospace()
                    .size(font_xs())
                    .color(trail_col),
            );
        })
        .dense(false)
        .show(ui, t);

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

fn lib_bool_row(ui: &mut egui::Ui, t: &Theme, tg: Tg, chart: &mut Chart, sec_idx: usize) {
    let active = bool_get(chart, tg);
    let id_salt = lib_id(sec_idx, bool_label(tg));
    let trail_text: &str = if active { "ON" } else { "+" };
    let trail_col = if active { t.accent } else { t.dim };
    let label = bool_label(tg);
    let resp = PanelListRow::new(&id_salt)
        .primary(label)
        .selected(active)
        .divided(true)
        .trailing(move |ui, _t| {
            ui.label(
                egui::RichText::new(trail_text)
                    .monospace()
                    .size(font_xs())
                    .color(trail_col),
            );
        })
        .show(ui, t);
    if resp.clicked() { bool_set(chart, tg, !active); }
}

fn lib_vp_row(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart, sec_idx: usize) {
    let active = chart.vp_mode != VolumeProfileMode::Off;
    let id_salt = lib_id(sec_idx, "VP");
    let trail_text: String = if active { vp_label(chart.vp_mode).to_string() } else { "+".to_string() };
    let trail_col = if active { t.accent } else { t.dim };
    let resp = PanelListRow::new(&id_salt)
        .primary("Volume profile")
        .selected(active)
        .divided(true)
        .trailing(move |ui, _t| {
            ui.label(
                egui::RichText::new(&trail_text)
                    .monospace()
                    .size(font_xs())
                    .color(trail_col),
            );
        })
        .show(ui, t);
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

fn lib_swing_row(ui: &mut egui::Ui, t: &Theme, chart: &mut Chart, sec_idx: usize) {
    let active = chart.swing_leg_mode > 0;
    let id_salt = lib_id(sec_idx, "SR");
    let trail_text: String = if active { swing_label(chart.swing_leg_mode).to_string() } else { "+".to_string() };
    let trail_col = if active { t.accent } else { t.dim };
    let resp = PanelListRow::new(&id_salt)
        .primary("SwingRange")
        .selected(active)
        .divided(true)
        .trailing(move |ui, _t| {
            ui.label(
                egui::RichText::new(&trail_text)
                    .monospace()
                    .size(font_xs())
                    .color(trail_col),
            );
        })
        .show(ui, t);
    if resp.clicked() { chart.swing_leg_mode = (chart.swing_leg_mode + 1) % 3; }
}
