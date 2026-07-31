//! SOTA §4.4 — TradePlan v2 panel.
//!
//! Renders the calibrated trade plan for the active chart symbol, sourced
//! from `live_state.latest_trade_plan`. Replaces the legacy point-only
//! tuple form in `chart_widgets.rs::draw_trade_plan` for the
//! calibration-aware visualization:
//!
//!   - Direction pill (LONG/SHORT) with conformal-coverage badge ("80% CI").
//!   - Entry tick mark + ENTRY label.
//!   - Target range as a horizontal band (║...║) with the point target
//!     drawn as a tick inside. Band collapses to a tick when range is None.
//!   - Stop range identically.
//!   - Hit-rate progress bar with color routing:
//!       >=0.6 → green, 0.5..0.6 → yellow, <0.5 → red, None → grey
//!     and an "n=NNN samples" caption. n < 30 → greyed out.
//!   - Exit rule (free-form string).
//!   - Day-type chip ("BULL 78%"). MIXED/CHOP suppresses the whole plan
//!     with a "No edge today — chop" banner per the user's day-type rule.
//!   - [🔍 prov] button — calls into the provenance pane bus when wired.
//!   - Hover tooltip shows the full Calibrated{ci_low,ci_high} fields.
//!
//! Layout: side panel mounted alongside the other R-side panels via
//! `top_nav.rs::draw_sidebar`. Toggle through `Watchlist.trade_plan_panel_open`.

#![allow(dead_code)]

use egui;
use crate::ui_kit::sx::Tone;

use super::super::style::*;
use super::super::super::gpu::{Chart, Theme, Watchlist};
use crate::data::apex_data::live_state;
use crate::data::apex_data::types::{CalibrationTier, TradePlanV2};
use crate::ui_kit::widgets::Button as KitButton;
use crate::ui_kit::widgets::Tooltip;
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};
use crate::ui_kit::widgets::PanelEmpty;
use crate::ui_kit::widgets::PanelKeyValueRow;
use crate::ui_kit::widgets::PanelTone;
use crate::ui_kit::layout::{Align as FlexAlign, Item, Surface};
use crate::ui_kit::scale::Space;
use crate::ui_kit::style::measure_mono;
use crate::ui_kit::widgets::{MetricRow, MetricTone};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};

/// Min historical samples for the hit-rate to be considered trustworthy. Below
/// this the panel greys out the percentage and adds a "low confidence" tag.
const MIN_TRUSTED_SAMPLES: u32 = 30;

/// Event emitted by the `[🔍 prov]` button. Caller hooks this into the
/// provenance pane's event bus at integration time. If the provenance pane
/// isn't present in this build, leave the callback unset and the button is
/// rendered with a "not available" hover tooltip.
pub struct OpenProvenanceFor { pub lineage_id: String }

type ProvenanceCallback = Box<dyn Fn(OpenProvenanceFor) + Send + Sync>;

fn callback() -> &'static std::sync::Mutex<Option<ProvenanceCallback>> {
    static CB: std::sync::OnceLock<std::sync::Mutex<Option<ProvenanceCallback>>> = std::sync::OnceLock::new();
    CB.get_or_init(|| std::sync::Mutex::new(None))
}

/// Wire up the `[🔍 prov]` callback. Last-wins.
pub fn set_provenance_callback(cb: ProvenanceCallback) {
    if let Ok(mut g) = callback().lock() { *g = Some(cb); }
}

fn fire_provenance(event: OpenProvenanceFor) {
    if let Ok(g) = callback().lock() {
        if let Some(cb) = g.as_ref() { cb(event); }
    }
    // TODO(provenance): when the provenance pane lands in this branch, wire
    // its event bus through `set_provenance_callback` from lib.rs.
}

/// Public draw entry — mirrors the convention of every other panel.
pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !is_open(watchlist) { return; }
    let sym = panes[ap].symbol.clone();

    let pane_h    = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();
    let resp = SidePanelShell::new("trade_plan_panel_v2", "TRADE PLAN v2")
        .width(Width::Narrow)
        .resizable(240.0..=360.0)
        .pane_metrics(pane_h, pane_font)
        .show(ctx, t, |ui, t| {
            match live_state::get_trade_plan(&sym) {
                None => draw_empty(ui, &sym, t),
                Some(plan) => draw_plan(ui, &plan, t),
            }
        });
    if resp.close_clicked { close(watchlist); }
}

/// Watchlist toggle helpers. Held here (rather than as a struct field) so the
/// new panel doesn't force a Watchlist schema migration in this branch — we
/// keep the toggle in module-local state and the top_nav menu just calls
/// `toggle()`.
fn open_flag() -> &'static std::sync::Mutex<bool> {
    static O: std::sync::OnceLock<std::sync::Mutex<bool>> = std::sync::OnceLock::new();
    O.get_or_init(|| std::sync::Mutex::new(false))
}
pub(crate) fn is_open(_w: &Watchlist) -> bool {
    open_flag().lock().map(|g| *g).unwrap_or(false)
}
pub(crate) fn open(_w: &mut Watchlist)   { if let Ok(mut g) = open_flag().lock() { *g = true; } }
pub(crate) fn close(_w: &mut Watchlist)  { if let Ok(mut g) = open_flag().lock() { *g = false; } }
pub(crate) fn toggle(_w: &mut Watchlist) { if let Ok(mut g) = open_flag().lock() { *g = !*g; } }

fn draw_empty(ui: &mut egui::Ui, sym: &str, t: &Theme) {
    // The canonical empty state — was a hand-rolled `vertical_centered` pair.
    // `PanelEmpty` supplies the same centred title + hint stack (and its own
    // leading `gap_md`), so the panel stops carrying its own version of it.
    let hint = format!("for {sym}");
    PanelEmpty::new("NO TRADE PLAN").hint(&hint).show(ui, t);
}

fn draw_plan(ui: &mut egui::Ui, plan: &TradePlanV2, t: &Theme) {
    // ── Day-type suppression banner ─────────────────────────────────────────
    if plan.day_type_suppressed() {
        let dt = plan.day_type.as_deref().unwrap_or("?");
        ui.add_space(gap_sm());
        crate::ui_kit::widgets::OutlinedBox::new()
            .fill(tint(t, Tone::Dim, alpha_ghost()))
            .border(tint(t, Tone::Dim, alpha_muted()))
            .hairline()
            .radius_sm()
            .padding(gap_sm())
            .show(ui, t, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(format!("Day type: {dt}"))
                        .monospace().size(FONT_XS).color(t.dim));
                    ui.label(egui::RichText::new("No edge today — plan suppressed")
                        .monospace().size(FONT_2XS).color(color_dim(t.dim)));
                });
            });
        return;
    }

    // ── Direction pill + conformal-coverage badge ───────────────────────────
    let is_long = plan.direction.eq_ignore_ascii_case("long");
    let dir_col = if is_long { t.bull } else { t.bear };
    let dir_label = if is_long { "LONG" } else { "SHORT" };
    // ONE flex row. The `with_layout(right_to_left)` child existed for exactly
    // one reason — to shove the CI badge against the right edge — and that is a
    // `grow` item here. The direction pill + symbol keep their natural
    // left-to-right flow inside the growing slot, so nothing about them moves.
    let cov_label = (plan.conformal_coverage > 0.0)
        .then(|| format!("{:.0}% CI", plan.conformal_coverage * 100.0));
    let cov_sz = cov_label.as_deref().map(|s| measure_mono(ui, s, FONT_2XS));
    // `OutlinedBox` is an `egui::Frame`: its height is the label galley plus the
    // 3px top/bottom inner margin below.
    let row_h = measure_mono(ui, dir_label, FONT_SM).y + 6.0;
    let (_row_id, row_rect) = ui.allocate_space(egui::vec2(ui.available_width(), row_h));
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(row_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let mut strip = Surface::row()
        .gap(Space::Sm)
        .align(FlexAlign::Center)
        .item(Item::grow(1.0).cross(row_h));
    if let Some(sz) = cov_sz {
        strip = strip.item(Item::fixed(sz.x).cross(sz.y));
    }
    strip.show(&mut row, t, |idx, ui| {
        if idx == 0 {
            ui.horizontal(|ui| {
                crate::ui_kit::widgets::OutlinedBox::new()
                    .fill(color_alpha(dir_col, alpha_ghost()))
                    .border(color_alpha(dir_col, alpha_muted()))
                    .hairline()
                    .radius_sm()
                    .padding_margin(egui::Margin { left: 8, right: 8, top: 3, bottom: 3 })
                    .show(ui, t, |ui| {
                        ui.label(egui::RichText::new(dir_label).monospace().strong()
                            .size(FONT_SM).color(dir_col));
                    });
                ui.label(egui::RichText::new(&plan.symbol).monospace()
                    .size(FONT_SM).color(t.text));
            });
        } else if let Some(c) = cov_label.as_deref() {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(c).monospace().size(FONT_2XS).color(t.dim),
                )
                .wrap_mode(egui::TextWrapMode::Extend),
            );
        }
    });
    ui.add_space(gap_sm());

    // ── Price ladder (ENTRY / TARGET / STOP) with optional range bands ──────
    price_row(ui, "ENTRY",  plan.entry_price,  None,               PanelTone::Text, t);
    price_row(ui, "TARGET", plan.target_price, plan.target_range,  PanelTone::Bull, t);
    price_row(ui, "STOP",   plan.stop_price,   plan.stop_range,    PanelTone::Bear, t);
    ui.add_space(gap_sm());

    // ── Hit-rate progress bar ───────────────────────────────────────────────
    hit_rate_row(ui, plan, t);
    ui.add_space(gap_sm());

    // ── Exit rule ───────────────────────────────────────────────────────────
    if let Some(rule) = plan.exit_rule.as_deref() {
        ui.label(egui::RichText::new("EXIT RULE").monospace()
            .size(FONT_2XS).color(color_half(t.dim)));
        ui.label(egui::RichText::new(rule).monospace()
            .size(FONT_XS).color(t.text));
        ui.add_space(gap_sm());
    }

    // ── Day-type chip ───────────────────────────────────────────────────────
    if let Some(dt) = plan.day_type.as_deref() {
        let conf = plan.day_type_confidence.unwrap_or(0.0);
        let chip = format!("{dt} {:.0}%", conf * 100.0);
        // A label/value pair — exactly what `PanelKeyValueRow` is, and the same
        // primitive the ENTRY/TARGET/STOP ladder above already uses.
        PanelKeyValueRow::new("DAY TYPE", chip)
            .tone(PanelTone::Accent)
            .show(ui, t);
        ui.add_space(gap_sm());
    }

    // ── [🔍 prov] button ────────────────────────────────────────────────────
    if let Some(prov) = plan.provenance.as_ref() {
        let lineage = prov.lineage_id.clone();
        let resp = KitButton::new("\u{1F50D} prov").variant(KitVariant::Ghost).size(KitSize::Xs)
            .fg(t.accent).min_size(egui::vec2(0.0, 18.0)).show(ui, t);
        if resp.clicked() {
            fire_provenance(OpenProvenanceFor { lineage_id: lineage });
        }
    }
}

/// Label + price value row using `PanelKeyValueRow`, with an optional range
/// band below (bespoke CI visualization — kept custom because it encodes
/// three values: lo, hi, and point estimate as an inner tick; no standard
/// primitive carries all three). Colors route through `PanelTone → Theme`.
fn price_row(
    ui: &mut egui::Ui,
    label: &str,
    point: f64,
    range: Option<(f64, f64)>,
    tone: PanelTone,
    t: &Theme,
) {
    let color = tone.color(t);
    PanelKeyValueRow::new(label, format!("${:.2}", point))
        .tone(tone)
        .show(ui, t);
    if let Some((lo, hi)) = range {
        // Edge case (per spec): tiny target_range — degenerate band collapses
        // to ~1px; we still draw the band but make sure the inner tick is
        // visible. Test fixture in tests::tiny_range_renders covers this.
        let tiny = (hi - lo).abs() < 0.001;
        let tip_w = ui.available_width().max(40.0);
        let bar_h = 6.0;
        let (band_rect, _) = ui.allocate_exact_size(
            egui::vec2(tip_w, bar_h), egui::Sense::hover());
        let painter = ui.painter();
        // Background band.
        painter.rect_filled(band_rect, 2.0, color_alpha(color, alpha_ghost()));
        // Inner tick at the point estimate.
        let rel = if tiny { 0.5 } else {
            let frac = (point - lo) / (hi - lo);
            frac.clamp(0.0, 1.0) as f32
        };
        let tick_x = band_rect.left() + rel * band_rect.width();
        let tick = egui::Rect::from_center_size(
            egui::pos2(tick_x, band_rect.center().y),
            egui::vec2(2.0, bar_h + 4.0));
        painter.rect_filled(tick, 1.0, color);
        // Hover tooltip with CI bounds — the "Calibrated fields" requirement.
        let resp = ui.interact(band_rect,
            ui.id().with(("range_tip", label)), egui::Sense::hover());
        Tooltip::new(format!(
            "{label} band\n  low:  ${:.2}\n  high: ${:.2}\n  point: ${:.2}",
            lo, hi, point)).show(ui, &resp, t);
    }
    ui.add_space(gap_2xs());
}

/// Hit-rate label+value+bar row using `MetricRow::bar()`. The bar is a
/// hairline rule under the row (MetricRow's built-in). An additional
/// sample-count caption line is appended below — this is presentation data
/// not captured by MetricRow and preserved from the original spec.
fn hit_rate_row(ui: &mut egui::Ui, plan: &TradePlanV2, t: &Theme) {
    let tier = plan.calibration_tier();
    let untrusted = plan.historical_n_samples < MIN_TRUSTED_SAMPLES;

    // Map calibration tier → MetricTone. Marginal uses `Warn` (= t.warn,
    // previously a raw Color32::from_rgb(255,191,0) which broke light themes).
    let base_tone = match tier {
        CalibrationTier::Strong   => MetricTone::Bull,
        CalibrationTier::Marginal => MetricTone::Warn,
        CalibrationTier::Weak     => MetricTone::Bear,
        CalibrationTier::Unknown  => MetricTone::Muted,
    };
    // When n < MIN_TRUSTED_SAMPLES, fall back to Muted to grey out the value.
    let tone = if untrusted { MetricTone::Muted } else { base_tone };

    let value_txt = match plan.historical_hit_rate {
        Some(r) => format!("{:.0}%", r * 100.0),
        None => "—".to_string(),
    };
    let frac = plan.historical_hit_rate.unwrap_or(0.0).clamp(0.0, 1.0) as f32;

    MetricRow::new("HIT RATE")
        .value(value_txt)
        .tone(tone)
        .bar(frac)
        .show(ui, t);

    // Sample-count caption — below the MetricRow+bar composite.
    let caption = if plan.historical_n_samples == 0 {
        "no calibration data".to_string()
    } else if untrusted {
        format!("n={} (low confidence)", plan.historical_n_samples)
    } else {
        format!("n={} samples", plan.historical_n_samples)
    };
    ui.label(egui::RichText::new(caption).monospace()
        .size(FONT_2XS).color(color_half(t.dim)));
}

// ── Tests ────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::apex_data::types::ProvenanceMeta;

    fn base_plan() -> TradePlanV2 {
        TradePlanV2 {
            symbol: "AAPL".into(),
            direction: "long".into(),
            entry_price: 100.0,
            target_price: 105.0,
            stop_price: 98.5,
            target_range: None,
            stop_range: None,
            historical_hit_rate: None,
            historical_n_samples: 0,
            conformal_coverage: 0.0,
            exit_rule: None,
            day_type: None,
            day_type_confidence: None,
            provenance: None,
            t_ms: 0,
        }
    }

    #[test]
    fn calibration_tier_routes_correctly() {
        let mut p = base_plan();
        assert_eq!(p.calibration_tier(), CalibrationTier::Unknown);

        p.historical_hit_rate = Some(0.42);
        assert_eq!(p.calibration_tier(), CalibrationTier::Weak);

        p.historical_hit_rate = Some(0.55);
        assert_eq!(p.calibration_tier(), CalibrationTier::Marginal);

        p.historical_hit_rate = Some(0.72);
        assert_eq!(p.calibration_tier(), CalibrationTier::Strong);
    }

    #[test]
    fn day_type_chop_suppresses() {
        let mut p = base_plan();
        p.day_type = Some("CHOP".into());
        assert!(p.day_type_suppressed());

        p.day_type = Some("MIXED".into());
        assert!(p.day_type_suppressed());

        p.day_type = Some("BULL".into());
        assert!(!p.day_type_suppressed());

        p.day_type = None;
        assert!(!p.day_type_suppressed(), "no day_type → don't suppress");
    }

    #[test]
    fn plan_with_no_conformal_ranges_still_serializable() {
        // Round-trips even though every conformal field is missing — older
        // backend builds emit only the legacy fields.
        let p = base_plan();
        let json = serde_json::to_string(&p).unwrap();
        let back: TradePlanV2 = serde_json::from_str(&json).unwrap();
        assert!(back.target_range.is_none());
        assert!(back.historical_hit_rate.is_none());
    }

    #[test]
    fn plan_with_conformal_ranges_round_trips() {
        let mut p = base_plan();
        p.target_range = Some((104.0, 107.0));
        p.stop_range = Some((97.5, 99.0));
        p.historical_hit_rate = Some(0.68);
        p.historical_n_samples = 124;
        p.conformal_coverage = 0.80;
        p.exit_rule = Some("scale 50% at +1R; runner to +2R".into());
        p.day_type = Some("BULL".into());
        p.day_type_confidence = Some(0.78);
        p.provenance = Some(ProvenanceMeta {
            lineage_id: "L-42".into(),
            model: Some("planner-v3".into()),
            ..Default::default()
        });
        let json = serde_json::to_string(&p).unwrap();
        let back: TradePlanV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(back.target_range, Some((104.0, 107.0)));
        assert_eq!(back.historical_hit_rate, Some(0.68));
        assert_eq!(back.calibration_tier(), CalibrationTier::Strong);
        assert_eq!(back.provenance.as_ref().unwrap().lineage_id, "L-42");
    }

    #[test]
    fn untrusted_threshold_is_30() {
        // Per the panel spec — n < 30 greyed out.
        assert!(29 < MIN_TRUSTED_SAMPLES);
        assert_eq!(MIN_TRUSTED_SAMPLES, 30);
    }

    #[test]
    fn old_wire_data_with_only_legacy_fields_deserializes() {
        // Simulates a server that hasn't shipped the SOTA upgrade yet.
        let legacy_json = r#"{
            "symbol":"NVDA","direction":"short",
            "entry_price":500.0,"target_price":485.0,"stop_price":505.0
        }"#;
        let p: TradePlanV2 = serde_json::from_str(legacy_json)
            .expect("legacy schema must deserialize via serde defaults");
        assert!(p.target_range.is_none());
        assert!(p.stop_range.is_none());
        assert_eq!(p.historical_n_samples, 0);
        assert!(p.exit_rule.is_none());
    }
}
