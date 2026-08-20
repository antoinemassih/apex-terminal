//! Test-only: read back what a widget actually painted.
//!
//! # Why this is one module and not six copies
//!
//! Six copies is what it was becoming. The same twenty lines — build a
//! `Context`, run a throwaway frame so the font atlas exists, run a second
//! frame with the widget in it, walk the layer's shapes and pull the text runs
//! out — were written independently in `cascade::element`, `widgets::kv_row`,
//! `widgets::selectable_row` and `widgets::button`, while the surrounding work
//! was consolidating four colour scales and five number formatters. Test code
//! is not exempt from the rule it is being used to enforce.
//!
//! # The two-frame rule
//!
//! `egui::__run_test_ui` runs ONE frame and a `Context` has no font atlas on
//! its first, so `layout_no_wrap` returns width 0 for every string. Every
//! width-dependent assertion written against that harness was comparing zeros
//! and passing (AT-166). Both entry points here run a throwaway frame first;
//! `assert_atlas_is_built` exists so a regression fails loudly rather than
//! going quietly vacuous.
//!
//! # What it can and cannot see
//!
//! Text runs only. Rects, strokes and circles are not returned, because a
//! painted rectangle rarely encodes the contract these tests are for: text
//! overrunning a slot and colliding with the next control is the defect this
//! codebase keeps producing. Add shape kinds when a test needs one, not
//! speculatively.

use egui::{Color32, Rect, Ui, Vec2};

/// One painted text run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub left: f32,
    pub right: f32,
    /// Vertical extent. Present so [`assert_no_overlap`] can tell a STACKED
    /// layout from a colliding one: the stepper paints a number inside a
    /// circle and its label directly beneath, sharing an x-range entirely by
    /// design. An x-only check called that a collision and would have had the
    /// widget "fixed" to satisfy the test.
    pub top: f32,
    pub bottom: f32,
    pub color: Color32,
}

impl Run {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }
}

/// A `Context` with the app's REAL font set installed.
///
/// A bare `egui::Context` knows only egui's default families, so any widget
/// reaching for a named family panics inside epaint:
///
/// ```text
/// FontFamily::Name("inter_semibold") is not bound to any fonts
/// ```
///
/// `ToggleRow` does exactly that for its label, and there was no way to probe
/// it at all until this existed — the harness could only see widgets that
/// happened to stay on the default families, which is a silent limit on what
/// can be tested rather than a visible one.
///
/// Uses the same `icons::init_fonts` the app calls, so a probe measures against
/// the faces that ship. Measuring against substitutes would make every width
/// assertion here a statement about the wrong font.
fn probe_ctx() -> egui::Context {
    probe_ctx_with_font(DEFAULT_PROBE_FONT)
}

/// The font preset a bare [`probe`] uses.
///
/// **0 is not neutral.** `init_fonts`'s `match font_idx` maps the default arm
/// to `"jetbrains_mono"` as the PROPORTIONAL primary, so at preset 0
/// `prop_at()` and `mono_at()` resolve to the same face and measure
/// identically. Presets 1..=6 (Inter, Plus Jakarta, Space Grotesk, DM Sans,
/// Geist, IBM Plex Sans) put a genuinely proportional face there.
///
/// A test about the DIFFERENCE between the two families must therefore choose
/// its preset explicitly — at 0 the difference is zero and the assertion is
/// vacuous. See `select.rs`, where exactly that happened.
pub const DEFAULT_PROBE_FONT: usize = 0;

/// A preset where Proportional and Monospace are genuinely different faces.
pub const PROPORTIONAL_PROBE_FONT: usize = 1;

/// [`probe`] under an explicit font preset.
pub fn probe_with_font(font_idx: usize, f: impl FnOnce(&mut Ui)) -> Vec<Run> {
    let cell = std::cell::Cell::new(Some(f));
    let out = std::cell::RefCell::new(Vec::new());
    let ctx = probe_ctx_with_font(font_idx);
    let _ = ctx.run(Default::default(), |_| {});
    let _ = ctx.run(Default::default(), |c| {
        egui::CentralPanel::default().show(c, |ui| {
            crate::ui_kit::text_style::TextStyle::install(ui.style_mut());
            if let Some(f) = cell.take() {
                f(ui);
            }
            *out.borrow_mut() = collect(ui);
        });
    });
    out.into_inner()
}

fn probe_ctx_with_font(font_idx: usize) -> egui::Context {
    let ctx = egui::Context::default();
    crate::ui_kit::icons::init_fonts(&ctx, font_idx);
    ctx
}

/// Run `f` in a `Ui` whose font atlas is built, and return every text run it
/// painted, ordered left to right.
pub fn probe(f: impl FnOnce(&mut Ui)) -> Vec<Run> {
    let cell = std::cell::Cell::new(Some(f));
    let out = std::cell::RefCell::new(Vec::new());
    let ctx = probe_ctx();
    // Frame 1 builds the atlas and is deliberately empty.
    let _ = ctx.run(Default::default(), |_| {});
    let _ = ctx.run(Default::default(), |c| {
        egui::CentralPanel::default().show(c, |ui| {
            crate::ui_kit::text_style::TextStyle::install(ui.style_mut());
            if let Some(f) = cell.take() {
                f(ui);
            }
            *out.borrow_mut() = collect(ui);
        });
    });
    out.into_inner()
}

/// How many NON-TEXT shapes the closure painted.
///
/// The module doc says to add shape kinds "when a test needs one, not
/// speculatively". One now does: the watchlist's `Week52Range` column draws a
/// range bar — a line segment and a dot — and paints no text at all, so a
/// text-only probe cannot tell "the column rendered" from "the column returned
/// early because its data was `None`". That distinction is the entire point of
/// the test, and getting it wrong reads as the column working.
///
/// Deliberately a COUNT and not a shape model. The question a caller has is
/// "did this draw anything", and a richer return would be inventing an API for
/// a need nobody has yet.
pub fn shape_count(f: impl FnOnce(&mut Ui)) -> usize {
    let cell = std::cell::Cell::new(Some(f));
    let out = std::cell::Cell::new(0usize);
    let ctx = probe_ctx();
    let _ = ctx.run(Default::default(), |_| {});
    let _ = ctx.run(Default::default(), |c| {
        egui::CentralPanel::default().show(c, |ui| {
            crate::ui_kit::text_style::TextStyle::install(ui.style_mut());
            if let Some(f) = cell.take() {
                f(ui);
            }
            let layer = ui.layer_id();
            let n = ui.ctx().graphics(|g| {
                g.get(layer)
                    .map(|l| {
                        l.all_entries()
                            .filter(|cs| !matches!(&cs.shape, egui::Shape::Text(_)))
                            .count()
                    })
                    .unwrap_or(0)
            });
            out.set(n);
        });
    });
    out.get()
}

fn collect(ui: &Ui) -> Vec<Run> {
    let layer = ui.layer_id();
    let mut runs: Vec<Run> = ui.ctx().graphics(|g| {
        g.get(layer)
            .map(|l| {
                l.all_entries()
                    .filter_map(|cs| match &cs.shape {
                        egui::Shape::Text(t) => Some(Run {
                            left: t.pos.x,
                            right: t.pos.x + t.galley.size().x,
                            top: t.pos.y,
                            bottom: t.pos.y + t.galley.size().y,
                            // `Painter::text` bakes the colour into the layout
                            // job's sections and leaves `override_text_color`
                            // as `None` — reading the override instead is a
                            // mistake that costs an afternoon.
                            color: t
                                .galley
                                .job
                                .sections
                                .first()
                                .map_or(Color32::PLACEHOLDER, |s| s.format.color),
                        }),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
    runs.sort_by(|a, b| a.left.total_cmp(&b.left));
    runs
}

/// The harness itself, under test. Call this from any module relying on
/// measured widths — if the atlas ever stops being built, every such assertion
/// silently compares zeros and only this fails.
pub fn assert_atlas_is_built() {
    let runs = probe(|ui| {
        let font = crate::ui_kit::text_style::TextStyle::Body.font_id_in(ui);
        let galley = ui.fonts(|f| {
            f.layout_no_wrap("MEASURE ME".to_string(), font, Color32::WHITE)
        });
        ui.painter().galley(egui::pos2(0.0, 0.0), galley, Color32::WHITE);
    });
    assert_eq!(runs.len(), 1, "the probe painted nothing");
    assert!(
        runs[0].width() > 20.0,
        "the test font atlas is not built — text measures {} px wide. Every \
         width assertion built on this probe is vacuous until this passes.",
        runs[0].width()
    );
}

/// Every run lies inside `rect`.
///
/// The contract a widget breaks when its measure under-reports what its paint
/// draws: a label overruns its slot and lands on the next control.
#[track_caller]
pub fn assert_contained(name: &str, rect: Rect, runs: &[Run]) {
    assert!(!runs.is_empty(), "{name}: nothing was painted");
    for r in runs {
        assert!(
            r.left >= rect.left() - 0.5,
            "{name}: a run starts at {} , left of the rect ({}). runs={runs:?}",
            r.left,
            rect.left()
        );
        assert!(
            r.right <= rect.right() + 0.5,
            "{name}: a run ends at {}, past the rect's right edge ({}) — the \
             measure under-reports what the paint path draws. runs={runs:?}",
            r.right,
            rect.right()
        );
    }
}

/// No two runs occupy the same pixels.
///
/// Worth asserting SEPARATELY from containment, and the reason is concrete: a
/// mutation that stopped `Button::measure_content_w` reserving space for the
/// trailing icon left containment PASSING — the icon is anchored to the right
/// edge, so it stayed inside the now-too-narrow button and merely landed on
/// top of the label. Only this caught it.
#[track_caller]
pub fn assert_no_overlap(name: &str, runs: &[Run]) {
    for (i, a) in runs.iter().enumerate() {
        for b in &runs[i + 1..] {
            // Two runs collide only if they overlap on BOTH axes. Comparing x
            // alone flags every stacked layout — a caption under a value, a
            // step number above its label — as a collision.
            let x = a.right > b.left + 0.5 && b.right > a.left + 0.5;
            let y = a.bottom > b.top + 0.5 && b.bottom > a.top + 0.5;
            assert!(
                !(x && y),
                "{name}: runs overlap — [{}..{}]x[{}..{}] and [{}..{}]x[{}..{}]. runs={runs:?}",
                a.left, a.right, a.top, a.bottom,
                b.left, b.right, b.top, b.bottom
            );
        }
    }
}

/// Convenience for the common shape: paint into an explicit rect, then assert
/// both contracts.
#[track_caller]
pub fn assert_paints_within(name: &str, rect: Rect, f: impl FnOnce(&mut Ui)) -> Vec<Run> {
    let runs = probe(f);
    assert_contained(name, rect, &runs);
    assert_no_overlap(name, &runs);
    runs
}

/// A rect at a non-zero origin, so a test cannot pass by accident on
/// coordinates that happen to be 0.
pub fn probe_rect(w: f32, h: f32) -> Rect {
    Rect::from_min_size(egui::pos2(30.0, 10.0), Vec2::new(w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_can_measure_text() {
        assert_atlas_is_built();
    }

    #[test]
    fn overlapping_runs_are_detected() {
        let runs = [
            Run { left: 0.0, right: 50.0, top: 0.0, bottom: 12.0, color: Color32::WHITE },
            Run { left: 40.0, right: 90.0, top: 0.0, bottom: 12.0, color: Color32::WHITE },
        ];
        let caught = std::panic::catch_unwind(|| assert_no_overlap("t", &runs)).is_err();
        assert!(caught, "assert_no_overlap must reject overlapping runs");
    }

    /// Runs that share an x-range on DIFFERENT LINES are a stack, not a
    /// collision. The stepper paints a step number inside its circle and the
    /// label directly beneath it; an x-only check reported that as an overlap,
    /// which would have had a correct widget "fixed" to satisfy the test.
    #[test]
    fn vertically_separated_runs_are_not_a_collision() {
        let runs = [
            Run { left: 0.0, right: 50.0, top: 0.0, bottom: 12.0, color: Color32::WHITE },
            Run { left: 10.0, right: 40.0, top: 20.0, bottom: 32.0, color: Color32::WHITE },
        ];
        assert_no_overlap("stacked", &runs);
    }

    #[test]
    fn a_run_past_the_right_edge_is_detected() {
        let rect = probe_rect(100.0, 20.0);
        let runs = [Run { left: 30.0, right: 200.0, top: 10.0, bottom: 22.0, color: Color32::WHITE }];
        let caught =
            std::panic::catch_unwind(move || assert_contained("t", rect, &runs)).is_err();
        assert!(caught, "assert_contained must reject an overrun");
    }
}
