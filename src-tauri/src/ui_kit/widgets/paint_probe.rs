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
    pub color: Color32,
}

impl Run {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }
}

/// Run `f` in a `Ui` whose font atlas is built, and return every text run it
/// painted, ordered left to right.
pub fn probe(f: impl FnOnce(&mut Ui)) -> Vec<Run> {
    let cell = std::cell::Cell::new(Some(f));
    let out = std::cell::RefCell::new(Vec::new());
    let ctx = egui::Context::default();
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
    let ctx = egui::Context::default();
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
    for w in runs.windows(2) {
        assert!(
            w[0].right <= w[1].left + 0.5,
            "{name}: runs overlap — one ends at {} and the next starts at {}. runs={runs:?}",
            w[0].right,
            w[1].left
        );
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
            Run { left: 0.0, right: 50.0, color: Color32::WHITE },
            Run { left: 40.0, right: 90.0, color: Color32::WHITE },
        ];
        let caught = std::panic::catch_unwind(|| assert_no_overlap("t", &runs)).is_err();
        assert!(caught, "assert_no_overlap must reject overlapping runs");
    }

    #[test]
    fn a_run_past_the_right_edge_is_detected() {
        let rect = probe_rect(100.0, 20.0);
        let runs = [Run { left: 30.0, right: 200.0, color: Color32::WHITE }];
        let caught =
            std::panic::catch_unwind(move || assert_contained("t", rect, &runs)).is_err();
        assert!(caught, "assert_contained must reject an overrun");
    }
}
