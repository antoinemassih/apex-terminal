//! RegimeTape — always-visible 4-axis regime strip at the top of the terminal.
//!
//! See `docs/SOTA_UX_DESIGN.md` §4.3.
//!
//! Renders as a 32-48 px `TopBottomPanel::top` strip with 4 cells:
//! intraday / multiday / vol / sector. Each cell shows axis label, current
//! value (with a leading shape icon for color-blind safety), and the age of
//! the latest frame. Hover tooltips list upstream tells.
//!
//! Reads from `apex_data::live_state::get_regime()` — no panel-owned state
//! beyond what the live_state already keeps. An optional transitions sub-
//! strip below the main row renders the last few axis flips from
//! `live_state::get_regime_transitions()`.

use egui;
use crate::ui_kit::sx::Tone;

use crate::apex_data::live_state;
use crate::apex_data::types::{Regime, RegimeAxis};
use crate::chart_renderer::gpu::Theme;

use super::super::style::*;

/// Tape strip height (px). Spec §4.3 calls for 32-48.
pub const TAPE_HEIGHT: f32 = 40.0;
/// Optional transitions sub-strip height (px). Hidden when no transitions.
pub const TRANSITIONS_HEIGHT: f32 = 18.0;

/// One of the four axes. Used internally for color + icon selection so the
/// label-to-color mapping is centralized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis { Intraday, Multiday, Vol, Sector }

impl Axis {
    fn label(self) -> &'static str {
        match self { Self::Intraday => "INTRADAY", Self::Multiday => "MULTIDAY",
                     Self::Vol => "VOL", Self::Sector => "SECTOR" }
    }
}

/// Pick a color + leading glyph for a regime value. Combining color + shape
/// + text satisfies the spec's color-blind-safe palette requirement.
///
/// Returns `(rgb, icon)`. Icon is a single ASCII/Unicode glyph rendered
/// alongside the value text so red/green isn't the only signal.
pub fn axis_visual(axis_label: &str, value: &str) -> ((u8, u8, u8), &'static str) {
    // Canonical mappings — keep these aligned with backend regime values.
    // Anything unknown falls through to the neutral grey + dot.
    let intraday = matches!(axis_label, "intraday" | "INTRADAY");
    let multiday = matches!(axis_label, "multiday" | "MULTIDAY");
    let vol      = matches!(axis_label, "vol"      | "VOL");
    let sector   = matches!(axis_label, "sector"   | "SECTOR");

    if intraday {
        return match value {
            "Bull"        => ((76, 175, 80),  "▲"),
            "Bear"        => ((229, 57, 53),  "▼"),
            "Pin"         => ((255, 167, 38), "◆"),
            "Chop"        => ((158, 158, 158), "≈"),
            "Mixed"       => ((171, 71, 188),  "✕"),
            "PreClassify" => ((117, 117, 117), "?"),
            _ => ((158, 158, 158), "·"),
        };
    }
    if multiday {
        return match value {
            "Trend"      => ((76, 175, 80),  "▲"),
            "DownTrend"  => ((229, 57, 53),  "▼"),
            "Range"      => ((158, 158, 158), "─"),
            "Transition" => ((255, 167, 38), "↺"),
            _ => ((158, 158, 158), "·"),
        };
    }
    if vol {
        return match value {
            "LowContango"   => ((76, 175, 80),  "↘"),
            "Normal"        => ((117, 175, 200), "○"),
            "Elevated"      => ((255, 167, 38), "△"),
            "Backwardation" => ((229, 57, 53),  "↗"),
            "Crush"         => ((171, 71, 188), "✕"),
            _ => ((158, 158, 158), "·"),
        };
    }
    if sector {
        return match value {
            "RiskOnGrowth"   => ((76, 175, 80),  "▲"),
            "RiskOnCyclical" => ((100, 200, 150), "▴"),
            "Defensive"      => ((255, 167, 38),  "■"),
            "Commodity"      => ((171, 71, 188),  "◆"),
            "Balanced"       => ((158, 158, 158), "─"),
            _ => ((158, 158, 158), "·"),
        };
    }
    ((158, 158, 158), "·")
}

/// Format an "Xm ago" / "Xs ago" / "Xh ago" string for a `t_ms` timestamp.
/// Returns "—" when `t_ms <= 0`. Public for tests.
pub fn format_age(now_ms: i64, t_ms: i64) -> String {
    if t_ms <= 0 { return "—".into(); }
    let secs = ((now_ms - t_ms).max(0)) / 1000;
    if secs < 60 { format!("{}s ago", secs) }
    else if secs < 3600 { format!("{}m ago", secs / 60) }
    else if secs < 86_400 { format!("{}h ago", secs / 3600) }
    else { format!("{}d ago", secs / 86_400) }
}

fn now_ms() -> i64 {
    crate::foundation::time::now_ms()
}

/// Render the regime cells + transitions INSIDE an arbitrary `Ui` (no
/// `TopBottomPanel` chrome). Used by the Signals sidebar's Regime tab —
/// the old top-of-terminal strip wasted persistent vertical space, so the
/// 4-cell tape now lives as a tab inside an existing side panel.
pub(crate) fn draw_in_ui(ui: &mut egui::Ui, t: &Theme) {
    let frame_data = live_state::get_regime();
    let transitions = live_state::get_regime_transitions();
    draw_inner(ui, frame_data.as_ref(), &transitions, t);
}

/// Legacy top-strip render. Retained for compatibility; no longer called.
/// Use [`draw_in_ui`] (Signals sidebar → Regime tab) instead.
#[allow(dead_code)]
pub(crate) fn draw(ctx: &egui::Context, t: &Theme) {
    let frame_data = live_state::get_regime();
    let transitions = live_state::get_regime_transitions();

    egui::TopBottomPanel::top("regime_tape")
        .exact_height(TAPE_HEIGHT
            + if transitions.is_empty() { 0.0 } else { TRANSITIONS_HEIGHT })
        .frame(egui::Frame::NONE
            .fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
            .stroke(egui::Stroke::new(stroke_thin(),
                tint(t, Tone::Border, alpha_dim()))))
        .show(ctx, |ui| {
            draw_inner(ui, frame_data.as_ref(), &transitions, t);
        });
}

fn draw_inner(
    ui: &mut egui::Ui,
    frame: Option<&crate::apex_data::types::RegimeFrame>,
    transitions: &[crate::apex_data::types::RegimeTransition],
    t: &Theme,
) {
    let now = now_ms();

    let empty_regime = Regime::default();
    let regime = frame.map(|f| &f.regime).unwrap_or(&empty_regime);
    let t_ms   = frame.map(|f| f.t_ms).unwrap_or(0);
    let age    = format_age(now, t_ms);

    // ── 4-cell row ──
    let avail = ui.available_width();
    let cell_w = (avail / 4.0).max(80.0);
    ui.allocate_ui_with_layout(
        egui::vec2(avail, TAPE_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            draw_cell(ui, Axis::Intraday, &regime.intraday, &age, cell_w, t);
            cell_divider(ui, t);
            draw_cell(ui, Axis::Multiday, &regime.multiday, &age, cell_w, t);
            cell_divider(ui, t);
            draw_cell(ui, Axis::Vol, &regime.vol, &age, cell_w, t);
            cell_divider(ui, t);
            draw_cell(ui, Axis::Sector, &regime.sector, &age, cell_w, t);
        },
    );

    // ── Optional transitions sub-strip ──
    if !transitions.is_empty() {
        ui.allocate_ui_with_layout(
            egui::vec2(avail, TRANSITIONS_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(gap_sm());
                ui.label(egui::RichText::new("TRANSITIONS")
                    .monospace().size(FONT_3XS).color(t.dim));
                for tr in transitions.iter().rev().take(4) {
                    let txt = format!(" {}: {}→{} ({}) ",
                        tr.axis, tr.from, tr.to, format_age(now, tr.t_ms));
                    ui.label(egui::RichText::new(txt)
                        .monospace().size(FONT_3XS).color(color_subtle(t.dim)));
                }
            },
        );
    }
}

fn cell_divider(ui: &mut egui::Ui, t: &Theme) {
    let h = TAPE_HEIGHT - 8.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, h), egui::Sense::hover());
    ui.painter().line_segment(
        [egui::pos2(rect.center().x, rect.top()),
         egui::pos2(rect.center().x, rect.bottom())],
        egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_muted())));
}

fn draw_cell(
    ui: &mut egui::Ui,
    axis: Axis,
    regime: &RegimeAxis,
    age: &str,
    width: f32,
    t: &Theme,
) {
    let value = if regime.value.is_empty() { "—" } else { regime.value.as_str() };
    let ((r, g, b), icon) = axis_visual(axis.label(), value);
    let col = egui::Color32::from_rgb(r, g, b);

    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(width, TAPE_HEIGHT), egui::Sense::hover());

    let painter = ui.painter_at(rect);
    // Soft tinted bar on the LEFT edge — color-blind sighted users get a
    // strong visual anchor even when the value text rolls off.
    painter.rect_filled(
        egui::Rect::from_min_max(
            rect.min, egui::pos2(rect.min.x + 3.0, rect.max.y)),
        0.0, col);

    // Axis label (small, top of cell).
    painter.text(
        egui::pos2(rect.min.x + gap_sm(), rect.min.y + 4.0),
        egui::Align2::LEFT_TOP, axis.label(),
        crate::ui_kit::style::mono_at(FONT_3XS), t.dim);

    // Icon + value (mid).
    let mid_y = rect.center().y + 1.0;
    painter.text(
        egui::pos2(rect.min.x + gap_sm(), mid_y),
        egui::Align2::LEFT_CENTER, icon,
        crate::ui_kit::style::mono_at(FONT_MD), col);
    painter.text(
        egui::pos2(rect.min.x + gap_sm() + 14.0, mid_y),
        egui::Align2::LEFT_CENTER, value,
        crate::ui_kit::style::mono_at(FONT_SM), t.text);

    // Age (small, bottom-right).
    painter.text(
        egui::pos2(rect.max.x - gap_sm(), rect.max.y - 4.0),
        egui::Align2::RIGHT_BOTTOM, age,
        crate::ui_kit::style::mono_at(FONT_3XS), color_subtle(t.dim));

    // Tooltip — list upstream tells if present.
    if !regime.upstream_tells.is_empty() {
        let tooltip = regime.upstream_tells.join(" · ");
        resp.on_hover_text(tooltip);
    } else if let Some(conf) = regime.confidence {
        resp.on_hover_text(format!("confidence: {:.2}", conf));
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_age ────────────────────────────────────────────────────────

    #[test]
    fn format_age_returns_em_dash_for_zero() {
        assert_eq!(format_age(1_000_000, 0), "—");
        assert_eq!(format_age(1_000_000, -42), "—");
    }

    #[test]
    fn format_age_grades_correctly() {
        let now = 100_000_000_000_i64;
        assert_eq!(format_age(now, now), "0s ago");
        assert_eq!(format_age(now, now - 45_000), "45s ago");
        assert_eq!(format_age(now, now - 32 * 60_000), "32m ago");
        assert_eq!(format_age(now, now - 3 * 3_600_000), "3h ago");
        assert_eq!(format_age(now, now - 2 * 86_400_000), "2d ago");
    }

    #[test]
    fn format_age_clamps_negative_deltas() {
        // Future timestamps shouldn't crash — they pin to "0s ago".
        let now = 100_000_000_i64;
        assert_eq!(format_age(now, now + 5_000), "0s ago");
    }

    // ── axis_visual ───────────────────────────────────────────────────────

    #[test]
    fn intraday_known_values_pick_distinct_colors_and_icons() {
        let bull = axis_visual("intraday", "Bull");
        let bear = axis_visual("intraday", "Bear");
        let pin  = axis_visual("intraday", "Pin");
        let chop = axis_visual("intraday", "Chop");
        // No two regimes share both color and icon.
        let seen = [bull, bear, pin, chop];
        for i in 0..seen.len() {
            for j in (i+1)..seen.len() {
                assert!(seen[i] != seen[j],
                    "regimes {} and {} collide", i, j);
            }
        }
        // Per spec: don't rely on red/green alone — every value gets an icon
        // glyph that isn't a dot.
        for (_, icon) in seen { assert!(icon != "·", "expected non-default icon"); }
    }

    #[test]
    fn multiday_uppercase_label_works() {
        // Both lower and UPPER axis labels should map.
        let (_, icon_low) = axis_visual("multiday", "Trend");
        let (_, icon_up)  = axis_visual("MULTIDAY", "Trend");
        assert_eq!(icon_low, icon_up);
        assert_eq!(icon_low, "▲");
    }

    #[test]
    fn vol_recognizes_all_states() {
        for v in ["LowContango", "Normal", "Elevated", "Backwardation", "Crush"] {
            let (_, icon) = axis_visual("vol", v);
            assert!(icon != "·", "vol={} should have a real icon", v);
        }
    }

    #[test]
    fn sector_recognizes_all_states() {
        for v in ["RiskOnGrowth", "RiskOnCyclical", "Defensive", "Commodity", "Balanced"] {
            let (_, icon) = axis_visual("sector", v);
            assert!(icon != "·", "sector={} should have a real icon", v);
        }
    }

    #[test]
    fn unknown_value_falls_back_to_neutral() {
        let ((r, g, b), icon) = axis_visual("intraday", "Mystery");
        assert_eq!(icon, "·");
        // Grey palette ish — components within 50 of each other.
        let max = r.max(g).max(b) as i32;
        let min = r.min(g).min(b) as i32;
        assert!(max - min <= 50, "expected neutral grey");
    }

    #[test]
    fn unknown_axis_falls_back_to_neutral() {
        let (_, icon) = axis_visual("not_a_real_axis", "Bull");
        assert_eq!(icon, "·");
    }
}
