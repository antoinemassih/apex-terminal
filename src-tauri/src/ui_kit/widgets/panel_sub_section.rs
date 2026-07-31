//! PanelSubSection — collapsible category grouping nested inside a panel.
//!
//! One level down from `PanelSection`: same visual vocabulary (uppercase
//! `mono_xs` strong title in `palette_ct(t).base(Tone::Dim)`, optional count chip), but with a
//! click-to-toggle caret on the left and a `&mut bool` for persistent
//! expanded state. A hairline rule is painted below the header **always**
//! (whether expanded or collapsed) so the category boundary is visible
//! even when the body is hidden.
//!
//! Replaces the hand-rolled "caret + UPPERCASE label + count chip +
//! divider" pattern in `chart/renderer/ui/panels/indicators_panel.rs`
//! (the LIBRARY category headers) and the equivalent shape in
//! `object_tree.rs` folder rows.
//!
//! ```ignore
//! PanelSubSection::new("trend_indicators", "TREND")
//!     .count(8)
//!     .expanded(&mut group.expanded)
//!     .show(ui, t, |ui, t| {
//!         // body — only invoked when expanded
//!     });
//! ```
//!
//! Visual spec:
//! - Header row: `gap_lg()` (22px-ish) tall, full available width, clickable.
//! - Caret: `Icon::CARET_RIGHT` (collapsed) / `Icon::CARET_DOWN` (expanded),
//!   proportional 12px, painted in `palette_ct(t).base(Tone::Dim)`.
//! - Title: `font_xs()` monospace, strong, uppercase, `palette_ct(t).base(Tone::Dim)`.
//! - Count chip: monospace `font_xs()` strong in `color_alpha(palette_ct(t).base(Tone::Dim), 200)`,
//!   same treatment as `PanelSection::count`.
//! - Click anywhere on the header row toggles `*expanded`.
//! - Hover: very subtle `color_alpha(palette_ct(t).base(Tone::Text), 8)` background, `radius_sm()`.
//! - Bottom rule: `stroke_thin()` at `color_alpha(t.surface_border(), 36)` —
//!   matches `panel_divider` / `PanelSection`. Painted in both states.
//! - Body: indented from the left by `gap_md()` when expanded; nothing
//!   rendered when collapsed (early return).
//!
//! Sister widgets:
//! - `PanelSection` — top-level section header inside a panel body.
//! - `Disclosure` — generic collapsible (proportional font, `palette_ct(t).base(Tone::Text)` title);
//!   use when the row is *not* a panel category header.
//! - `panel_divider` — when you just need the hairline without a header.
//!
//! When NOT to use:
//! - As the outermost section in a panel body — use `PanelSection`.
//! - For freeform expand/collapse with a normal-case title — use
//!   `Disclosure`.

use egui::{CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use super::super::icons::Icon;
use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};
use crate::ui_kit::tokens::{
    color_alpha, font_sm, gap_2xs, gap_md, gap_xs, stroke_thin,
};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};

/// Alpha (out of 255) of the bottom hairline rule. Higher than the L2
/// surface contrast so the separator actually reads against the lifted
/// sub-section background — the previous 36 was barely visible.
const RULE_ALPHA: u8 = 80;

/// Hover background alpha (out of 255), applied to `palette_ct(t).base(Tone::Text)`. Matches
/// `PanelListRow::HOVER_BG_ALPHA` so categories feel like the rows they
/// contain.
const HOVER_BG_ALPHA: u8 = 8;

/// Caret glyph point size — proportional, matches Disclosure default.
const CARET_FONT: f32 = 12.0;

/// Header row height. Parent accordion categories deserve more vertical
/// breathing room than a list row — bumped from 22 to 30 so the parent
/// vs. child distinction reads at a glance.
const HEADER_H: f32 = 30.0;

// ─── Header strip: flex spec ────────────────────────────────────────────────
//
// The header row used to be hand-computed: `let mut x = rect.left(); … x +=
// caret_w + gap_xs(); … x += title_w;` with the trailing slot pinned by
// `rect.right() - slot_w`. That is exactly the arithmetic flexbox exists to
// delete — every one of those `+=`s is a place alignment can drift.
//
// GEOMETRY ONLY: fonts, colours and tokens below are unchanged.

/// Which header piece a solved flex slot belongs to. `Gap` is the elastic
/// middle and renders nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubSlot {
    Caret,
    Title,
    Count,
    Gap,
    Trailing,
}

/// Intrinsic sizes of the sub-header's pieces. Split out so the spec is
/// unit-testable from plain numbers — no egui context needed.
#[derive(Clone, Copy, Debug, Default)]
struct SubHeaderMetrics {
    caret: Vec2,
    title: Vec2,
    count: Option<Vec2>,
    /// Width of the reserved `header_trailing` slot; `None` when unused.
    trailing_w: Option<f32>,
}

/// Build the sub-header's flex items: `caret · title · count …… trailing`.
/// The `grow` item is the elastic middle that pins the trailing slot to the
/// right edge (it replaces `Pos2::new(rect.right() - slot_w, ..)`).
fn sub_header_slots(m: &SubHeaderMetrics) -> Vec<(SubSlot, Item)> {
    let mut v: Vec<(SubSlot, Item)> = Vec::new();
    v.push((SubSlot::Caret, Item::fixed(m.caret.x).cross(m.caret.y)));
    v.push((SubSlot::Title, Item::fixed(m.title.x).cross(m.title.y)));
    if let Some(c) = m.count {
        v.push((SubSlot::Count, Item::fixed(c.x).cross(c.y)));
    }
    v.push((SubSlot::Gap, Item::grow(1.0)));
    if let Some(w) = m.trailing_w {
        // Full-height slot (the old slot_rect spanned rect.top()..bottom()).
        v.push((SubSlot::Trailing, Item::fixed(w).align_self(FlexAlign::Stretch)));
    }
    v
}

/// One row, `gap_xs()` gutter — the exact token the old `x += … + gap_xs()`
/// arithmetic used — children vertically centered on the strip's mid-line, no
/// padding (the caret starts flush at `rect.left()`, as before).
fn sub_header_flex(slots: &[(SubSlot, Item)]) -> Flex {
    Flex::row()
        .gap(gap_xs())
        .align(FlexAlign::Center)
        .items(slots.iter().map(|(_, it)| *it))
}

/// Width reserved for the `header_trailing` slot — a generous slice so small
/// clusters of icon buttons / chips fit without the caller sizing them.
fn trailing_slot_width(avail_w: f32) -> f32 {
    (avail_w * 0.33).clamp(0.0, 160.0)
}

#[must_use = "PanelSubSection must be rendered with `.show(...)`"]
pub struct PanelSubSection<'a, T: ComponentTheme = crate::ui_kit::widgets::theme::PortableTheme> {
    id_salt: &'a str,
    title: &'a str,
    count: Option<usize>,
    expanded: Option<&'a mut bool>,
    /// Auto-persist expand/collapse state in egui memory. When set (and
    /// `expanded` is `None`), the widget reads and writes its open state
    /// under `ui.make_persistent_id(("panel_sub_section", key))`. Default
    /// open state is `true`. If both `expanded` and `persist_key` are set,
    /// `expanded` wins (explicit caller state takes precedence).
    persist_key: Option<&'a str>,
    /// Optional RTL slot painted at the right edge of the header row.
    /// Used for group-level controls (visibility toggle, opacity picker)
    /// that conceptually belong to the category as a whole. Click events
    /// inside this slot do **not** toggle `expanded` — the slot rect is
    /// excluded from the header's click sense.
    header_trailing: Option<Box<dyn FnOnce(&mut Ui, &T) + 'a>>,
}

impl<'a, T: ComponentTheme> PanelSubSection<'a, T> {
    pub fn new(id_salt: &'a str, title: &'a str) -> Self {
        Self {
            id_salt,
            title,
            count: None,
            expanded: None,
            persist_key: None,
            header_trailing: None,
        }
    }

    /// Mount a callback that renders inline in the header row's RTL slot,
    /// at the right edge between the count chip and the right margin.
    /// Used for group-level controls (visibility, opacity). Clicks landing
    /// inside the slot do NOT toggle `expanded` — the slot reserves its
    /// own rect and the header click sense excludes it.
    ///
    /// ```ignore
    /// PanelSubSection::new("trend", "TREND INDICATORS")
    ///     .count(8)
    ///     .expanded(&mut group.expanded)
    ///     .header_trailing(|ui, t| {
    ///         if Button::icon(Icon::EYE).variant(Variant::Ghost).show(ui, t).clicked() {
    ///             group.all_visible = !group.all_visible;
    ///         }
    ///     })
    ///     .show(ui, t, |ui, t| { /* body */ });
    /// ```
    pub fn header_trailing(mut self, f: impl FnOnce(&mut Ui, &T) + 'a) -> Self {
        self.header_trailing = Some(Box::new(f));
        self
    }

    /// Add a count chip after the title.
    pub fn count(mut self, n: usize) -> Self {
        self.count = Some(n);
        self
    }

    /// Bind the expanded/collapsed state. **Required** for the header to
    /// toggle on click. If omitted, the body is always rendered (and the
    /// caret is shown in the down position).
    pub fn expanded(mut self, state: &'a mut bool) -> Self {
        self.expanded = Some(state);
        self
    }

    /// Auto-persist expand/collapse state in egui memory, keyed by `key`.
    /// The state is stored under `ui.make_persistent_id(("panel_sub_section", key))`
    /// and defaults to `true` (expanded) on first use. This is an alternative
    /// to `.expanded(&mut bool)` for callers that do not want to manage the
    /// bool themselves. If `.expanded()` is also called, the explicit ref wins.
    pub fn persist_key(mut self, key: &'a str) -> Self {
        self.persist_key = Some(key);
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &T,
        body: impl FnOnce(&mut Ui, &T) -> R,
    ) -> Option<R> {
        let Self { id_salt, title, count, expanded, persist_key, header_trailing } = self;

        // Resolve current open state:
        //   1. Explicit `expanded` ref → always used if present (highest priority).
        //   2. `persist_key` → read from egui persisted memory (default: true).
        //   3. Neither → always-open fallback.
        let persist_id = persist_key.map(|k| ui.make_persistent_id(("panel_sub_section", k)));
        let is_open = if expanded.is_some() {
            expanded.as_ref().map(|b| **b).unwrap_or(true)
        } else if let Some(pid) = persist_id {
            ui.data_mut(|d| d.get_persisted::<bool>(pid).unwrap_or(true))
        } else {
            true
        };

        let avail_w = ui.available_width();
        // (Top + bottom rules are now painted on the header rect itself
        // below, so consecutive sub-sections stack as bordered bands
        // without needing a separate pre-header divider.)
        // Allocate the full header strip up-front, then split out the
        // header-trailing slot (when present) so its clicks are routed
        // separately from the header toggle.
        let (rect, _full_resp) = ui.allocate_exact_size(
            Vec2::new(avail_w, HEADER_H),
            Sense::hover(),
        );

        // Width of the header-trailing slot at the right edge (the flex spec
        // below pins it there with a `grow` middle — see `sub_header_slots`).
        // We need the width up front so the header's click sense can EXCLUDE
        // the slot: clicks on the caller's controls must not toggle the
        // sub-section. Empty slot reserves zero width.
        let slot_w = if header_trailing.is_some() {
            trailing_slot_width(avail_w)
        } else {
            0.0
        };
        // Header click area excludes the slot.
        let header_click_rect = if slot_w > 0.0 {
            Rect::from_min_max(
                rect.min,
                Pos2::new(rect.right() - slot_w - gap_xs(), rect.bottom()),
            )
        } else {
            rect
        };
        let resp = ui.interact(
            header_click_rect,
            ui.id().with(("panel_sub_section_header", id_salt)),
            Sense::click(),
        );
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

        // Toggle on click — three cases mirroring the state resolution above:
        //   1. Explicit `expanded` ref → toggle the ref and re-read.
        //   2. `persist_key` → toggle the persisted value and re-read.
        //   3. Neither → no-op (always-open).
        let mut is_open = is_open;
        if let Some(state) = expanded {
            if resp.clicked() {
                *state = !*state;
            }
            is_open = *state;
        } else if let Some(pid) = persist_id {
            if resp.clicked() {
                let new_val = !is_open;
                ui.data_mut(|d| d.insert_persisted(pid, new_val));
                is_open = new_val;
            }
        }

        // ── Header geometry — solved by the flex engine ─────────────────────
        // Measure the three text pieces, hand their intrinsic sizes to the
        // flex, and paint each galley into the rect it solved for. The old
        // running-`x` arithmetic (and the `rect.right() - slot_w` pin) is gone.
        //
        // Caret — proportional, in palette_ct(t).base(Tone::Dim), vertically
        // centered. No leading inset: the caret starts at rect.left so the
        // title text aligns with the PanelSection title above (which sits at
        // the same X via the parent section's body gap_md inset).
        let caret_glyph = if is_open { Icon::CARET_DOWN } else { Icon::CARET_RIGHT };
        let caret_color = palette_ct(t).base(Tone::Dim);
        let caret_galley = ui.fonts(|f| {
            f.layout_no_wrap(
                caret_glyph.to_string(),
                FontId::proportional(CARET_FONT),
                caret_color,
            )
        });
        // Title — uppercase mono_sm strong (same tier as PanelSection title).
        // Parent accordion categories deserve the same typographic weight as
        // the section above them so the parent/child relationship is clear by
        // hierarchy.
        let title_color = color_alpha(palette_ct(t).base(Tone::Text), 220);
        let title_galley = ui.fonts(|f| {
            f.layout_no_wrap(title.to_uppercase(), FontId::monospace(font_sm()), title_color)
        });
        // Count chip — same treatment as PanelSection.count.
        let count_color = color_alpha(palette_ct(t).base(Tone::Dim), 200);
        let count_galley = count.map(|n| {
            ui.fonts(|f| {
                f.layout_no_wrap(format!("{}", n), FontId::monospace(font_sm()), count_color)
            })
        });

        let metrics = SubHeaderMetrics {
            caret: caret_galley.rect.size(),
            title: title_galley.rect.size(),
            count: count_galley.as_ref().map(|g| g.rect.size()),
            trailing_w: if slot_w > 0.0 { Some(slot_w) } else { None },
        };
        let slots = sub_header_slots(&metrics);
        let solved = sub_header_flex(&slots).solve(Vec2::new(avail_w, HEADER_H));
        // Solved rects are container-relative — lift them onto the strip.
        let slot_rect_of = |want: SubSlot| -> Option<Rect> {
            slots
                .iter()
                .position(|(s, _)| *s == want)
                .map(|i| solved[i].translate(rect.min.to_vec2()))
        };

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            // Header-strip uses the unified header_surface token —
            // same fill as the chart pane header and the SidePanelShell
            // header. Reads as a labeled band, consistent with the
            // rest of the app's chrome family.
            painter.rect_filled(
                rect,
                CornerRadius::ZERO,
                t.header_surface(),
            );
            // Hover overlay — faint warm tint on top of the recessed base.
            if resp.hovered() {
                painter.rect_filled(
                    rect,
                    CornerRadius::ZERO,
                    color_alpha(palette_ct(t).base(Tone::Text), HOVER_BG_ALPHA),
                );
            }

            if let Some(r) = slot_rect_of(SubSlot::Caret) {
                painter.galley(r.min, caret_galley.clone(), caret_color);
            }
            if let Some(r) = slot_rect_of(SubSlot::Title) {
                painter.galley(r.min, title_galley.clone(), title_color);
            }
            if let (Some(r), Some(g)) = (slot_rect_of(SubSlot::Count), count_galley.clone()) {
                painter.galley(r.min, g, count_color);
            }

            // ONE bottom hairline — matching PanelSection. The former
            // treatment bracketed the band with BOTH a top and a bottom rule
            // (plus the 6px shadow below), which stacked 4 separators within
            // ~20px when a sub-section sat under a section header. A single
            // rule delimits it just as clearly, without the boxed-in look.
            painter.line_segment(
                [Pos2::new(rect.left(), rect.bottom() - 0.5), Pos2::new(rect.right(), rect.bottom() - 0.5)],
                Stroke::new(stroke_thin(), t.header_border()),
            );
        }

        // Header-trailing slot — render the caller's RTL closure into a
        // nested Ui scoped to the slot rect the flex reserved. Layouts
        // right-to-left so the caller can simply `add` widgets and they pack
        // from the right.
        if let (Some(slot), Some(cb)) = (slot_rect_of(SubSlot::Trailing), header_trailing) {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(slot)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            cb(&mut child, t);
        }

        // Body — only when expanded. Natural-flow, no background tint
        // (the tint lives on the HEADER strip). Just indent and add
        // breathing room. Use Frame for the inset — `horizontal {
        // vertical }` would collapse to a single-line height.
        if !is_open {
            return None;
        }

        // The 6px inset drop-shadow that used to fall from the header rule
        // into the body is gone (PanelSection dropped the identical block):
        // a fill-step + a rule + a shadow all marking the same boundary is
        // exactly the "many borders for everything" noise. The single rule
        // above carries it; keep just an even gap into the body.
        ui.add_space(gap_2xs());
        let out = egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: gap_md() as i8,
                right: gap_xs() as i8,
                top: gap_xs() as i8,
                bottom: gap_xs() as i8,
            })
            .show(ui, |ui| body(ui, t))
            .inner;
        ui.add_space(gap_xs());
        Some(out)
    }
}

// ─── Header geometry tests ──────────────────────────────────────────────────
//
// `sub_header_slots` + `sub_header_flex` are pure, so the header strip's
// alignment — previously a chain of `x += …` statements that could only be
// eyeballed — is now assertable headlessly via `Flex::solve`.
//
// Token values under test (default snapshot): gap_xs = 4, HEADER_H = 30.

#[cfg(test)]
mod header_layout_tests {
    use super::*;
    use egui::vec2;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    fn solve(m: &SubHeaderMetrics, w: f32) -> (Vec<(SubSlot, Item)>, Vec<Rect>) {
        let slots = sub_header_slots(m);
        let rects = sub_header_flex(&slots).solve(Vec2::new(w, HEADER_H));
        (slots, rects)
    }

    fn rect_of(slots: &[(SubSlot, Item)], rects: &[Rect], want: SubSlot) -> Rect {
        let i = slots
            .iter()
            .position(|(s, _)| *s == want)
            .unwrap_or_else(|| panic!("no {:?} slot in sub-header", want));
        rects[i]
    }

    fn metrics() -> SubHeaderMetrics {
        SubHeaderMetrics {
            caret: vec2(10.0, 12.0),
            title: vec2(96.0, 14.0),
            count: None,
            trailing_w: None,
        }
    }

    /// The caret keeps its measured width at the left edge and the title starts
    /// exactly one `gap_xs()` gutter later — the old `x += caret_w + gap_xs()`.
    #[test]
    fn caret_is_flush_left_and_title_follows_after_one_gap() {
        let m = metrics();
        let (slots, rects) = solve(&m, 240.0);
        let caret = rect_of(&slots, &rects, SubSlot::Caret);
        let title = rect_of(&slots, &rects, SubSlot::Title);
        assert!(approx(caret.left(), 0.0), "caret left {}", caret.left());
        assert!(approx(caret.width(), 10.0));
        assert!(
            approx(title.left(), 10.0 + gap_xs()),
            "title left {} want {}",
            title.left(),
            10.0 + gap_xs()
        );
        assert!(approx(title.width(), 96.0));
    }

    /// The count chip keeps its intrinsic width and follows the title by one
    /// gutter — the old `x += title_w; x += gap_xs()`.
    #[test]
    fn count_chip_follows_the_title_by_one_gap() {
        let m = SubHeaderMetrics { count: Some(vec2(8.0, 14.0)), ..metrics() };
        let (slots, rects) = solve(&m, 240.0);
        let title = rect_of(&slots, &rects, SubSlot::Title);
        let count = rect_of(&slots, &rects, SubSlot::Count);
        assert!(approx(count.width(), 8.0));
        assert!(
            approx(count.left(), title.right() + gap_xs()),
            "count left {} title right {}",
            count.left(),
            title.right()
        );
    }

    /// With no trailing slot, the elastic middle runs to the right edge and the
    /// title still starts hard against the caret.
    #[test]
    fn without_a_trailing_slot_the_row_spans_to_the_right_edge() {
        let m = metrics();
        let (slots, rects) = solve(&m, 240.0);
        assert_eq!(slots.last().unwrap().0, SubSlot::Gap);
        assert!(approx(rects.last().unwrap().right(), 240.0));
        assert!(
            !slots.iter().any(|(s, _)| *s == SubSlot::Trailing),
            "no trailing slot reserved when the caller didn't ask for one"
        );
    }

    /// The `header_trailing` slot is pinned flush right at its reserved width
    /// and spans the full strip height (it used to be `Rect::from_min_max(
    /// pos2(rect.right() - slot_w, rect.top()), pos2(rect.right(), bottom))`).
    #[test]
    fn trailing_slot_is_flush_right_and_full_height() {
        // 300 → a whole-pixel slot width, so the assertions are exact (Taffy
        // rounds solved rects to whole pixels).
        let w = 300.0;
        let slot_w = trailing_slot_width(w);
        let m = SubHeaderMetrics { trailing_w: Some(slot_w), ..metrics() };
        let (slots, rects) = solve(&m, w);
        let slot = rect_of(&slots, &rects, SubSlot::Trailing);
        assert!(approx(slot.right(), w), "slot right {}", slot.right());
        assert!(approx(slot.width(), slot_w), "slot width {}", slot.width());
        assert!(approx(slot.left(), w - slot_w));
        assert!(approx(slot.top(), 0.0));
        assert!(approx(slot.height(), HEADER_H), "slot height {}", slot.height());
        // ...and the title is unaffected by it.
        let title = rect_of(&slots, &rects, SubSlot::Title);
        assert!(approx(title.left(), 10.0 + gap_xs()));
    }

    /// Slot width is a third of the strip, capped at 160px.
    #[test]
    fn trailing_slot_width_is_a_third_capped() {
        assert!(approx(trailing_slot_width(240.0), 79.2));
        assert!(approx(trailing_slot_width(900.0), 160.0));
        assert!(approx(trailing_slot_width(0.0), 0.0));
    }

    /// Cross-axis: every text piece is centered on the strip's mid-line, which
    /// is what `cy - galley.height() * 0.5` did by hand.
    #[test]
    fn text_pieces_are_centered_on_the_strip_midline() {
        let m = SubHeaderMetrics { count: Some(vec2(8.0, 14.0)), ..metrics() };
        let (slots, rects) = solve(&m, 240.0);
        for want in [SubSlot::Caret, SubSlot::Title, SubSlot::Count] {
            let r = rect_of(&slots, &rects, want);
            assert!(
                approx(r.center().y, HEADER_H * 0.5),
                "{:?} center y {} want {}",
                want,
                r.center().y,
                HEADER_H * 0.5
            );
        }
        // Heights stay intrinsic (Center, not Stretch).
        assert!(approx(rect_of(&slots, &rects, SubSlot::Title).height(), 14.0));
    }

    /// Degenerate width (panel collapsed / first frame) must not panic.
    #[test]
    fn narrow_header_does_not_panic() {
        let m = SubHeaderMetrics {
            count: Some(vec2(8.0, 14.0)),
            trailing_w: Some(40.0),
            ..metrics()
        };
        let slots = sub_header_slots(&m);
        let rects = sub_header_flex(&slots).solve(Vec2::new(0.0, HEADER_H));
        assert_eq!(rects.len(), slots.len());
    }
}
