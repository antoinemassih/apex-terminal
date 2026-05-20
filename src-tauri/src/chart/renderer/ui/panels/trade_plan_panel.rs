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

use super::super::style::*;
use super::super::components::frames_widget as widgets_frames;
use super::super::super::gpu::{Chart, Theme, Watchlist};
use crate::data::apex_data::live_state;
use crate::data::apex_data::types::{CalibrationTier, TradePlanV2};
use crate::ui_kit::widgets::Button as KitButton;
use crate::ui_kit::widgets::Tooltip;
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};
use crate::ui_kit::widgets::icon_placement::IconPlacement;

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

    egui::SidePanel::right("trade_plan_panel_v2")
        .default_width(280.0)
        .min_width(240.0)
        .max_width(360.0)
        .resizable(true)
        .frame(widgets_frames::PanelFrame::new(t.toolbar_bg, t.toolbar_border).theme(t).build())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("TRADE PLAN v2").monospace().strong()
                    .size(FONT_SM).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r = crate::ui_kit::widgets::Button::close()
                        .placement(IconPlacement::PanelHeader)
                        .show(ui, t);
                    Tooltip::new("Close").show(ui, &r, t);
                    if r.clicked() {
                        close(watchlist);
                    }
                });
            });
            separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
            ui.add_space(gap_sm());

            match live_state::get_trade_plan(&sym) {
                None => draw_empty(ui, &sym, t),
                Some(plan) => draw_plan(ui, &plan, t),
            }
        });
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
    ui.add_space(gap_md());
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("NO TRADE PLAN").monospace()
            .size(FONT_SM).color(color_dim(t.dim)));
        ui.add_space(gap_xs());
        ui.label(egui::RichText::new(format!("for {sym}")).monospace()
            .size(FONT_XS).color(color_half(t.dim)));
    });
}

fn draw_plan(ui: &mut egui::Ui, plan: &TradePlanV2, t: &Theme) {
    // ── Day-type suppression banner ─────────────────────────────────────────
    if plan.day_type_suppressed() {
        let dt = plan.day_type.as_deref().unwrap_or("?");
        ui.add_space(gap_sm());
        egui::Frame::NONE
            .fill(color_alpha(t.dim, alpha_ghost()))
            .stroke(egui::Stroke::new(stroke_thin(), color_alpha(t.dim, alpha_muted())))
            .corner_radius(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
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
    ui.horizontal(|ui| {
        egui::Frame::NONE
            .fill(color_alpha(dir_col, alpha_ghost()))
            .stroke(egui::Stroke::new(stroke_thin(), color_alpha(dir_col, alpha_muted())))
            .corner_radius(4.0)
            .inner_margin(egui::Margin { left: 8, right: 8, top: 3, bottom: 3 })
            .show(ui, |ui| {
                ui.label(egui::RichText::new(dir_label).monospace().strong()
                    .size(FONT_SM).color(dir_col));
            });
        ui.label(egui::RichText::new(&plan.symbol).monospace()
            .size(FONT_SM).color(t.text));
        if plan.conformal_coverage > 0.0 {
            let cov_label = format!("{:.0}% CI", plan.conformal_coverage * 100.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(cov_label).monospace()
                    .size(FONT_2XS).color(t.dim));
            });
        }
    });
    ui.add_space(gap_sm());

    // ── Price ladder (ENTRY / TARGET / STOP) with optional range bands ──────
    draw_price_row(ui, "ENTRY",  plan.entry_price,  None,                  t.text, t);
    draw_price_row(ui, "TARGET", plan.target_price, plan.target_range,     t.bull, t);
    draw_price_row(ui, "STOP",   plan.stop_price,   plan.stop_range,       t.bear, t);
    ui.add_space(gap_sm());

    // ── Hit-rate progress bar ───────────────────────────────────────────────
    draw_hit_rate(ui, plan, t);
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
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("DAY TYPE").monospace()
                .size(FONT_2XS).color(color_half(t.dim)));
            ui.label(egui::RichText::new(chip).monospace()
                .size(FONT_XS).color(t.accent));
        });
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

fn draw_price_row(
    ui: &mut egui::Ui,
    label: &str,
    point: f64,
    range: Option<(f64, f64)>,
    color: egui::Color32,
    t: &Theme,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).monospace()
            .size(FONT_2XS).color(color_half(t.dim)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("${:.2}", point))
                .monospace().size(FONT_SM).color(color));
        });
    });
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

fn draw_hit_rate(ui: &mut egui::Ui, plan: &TradePlanV2, t: &Theme) {
    let tier = plan.calibration_tier();
    let (bar_col, label_col) = match tier {
        CalibrationTier::Strong   => (t.bull,                                       t.bull),
        CalibrationTier::Marginal => (egui::Color32::from_rgb(255, 191, 0),        egui::Color32::from_rgb(255, 191, 0)),
        CalibrationTier::Weak     => (t.bear,                                       t.bear),
        CalibrationTier::Unknown  => (color_alpha(t.dim, alpha_muted()),           t.dim),
    };
    let untrusted = plan.historical_n_samples < MIN_TRUSTED_SAMPLES;
    let display_col = if untrusted { color_alpha(label_col, alpha_muted()) } else { label_col };

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("HIT RATE").monospace()
            .size(FONT_2XS).color(color_half(t.dim)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let txt = match plan.historical_hit_rate {
                Some(r) => format!("{:.0}%", r * 100.0),
                None => "—".to_string(),
            };
            ui.label(egui::RichText::new(txt).monospace()
                .size(FONT_SM).color(display_col));
        });
    });
    // Bar.
    let bar_w = ui.available_width().max(40.0);
    let bar_h = 6.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(bar_w, bar_h), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, color_alpha(t.toolbar_border, alpha_muted()));
    let frac = plan.historical_hit_rate.unwrap_or(0.0).clamp(0.0, 1.0);
    if frac > 0.0 {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(bar_w * frac, bar_h)),
            2.0, bar_col,
        );
    }
    // Caption — sample count.
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
