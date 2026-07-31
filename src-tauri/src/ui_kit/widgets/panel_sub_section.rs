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

        // Reserve the header-trailing slot rect at the right edge. We
        // reserve a generous slice (1/3 of available width, capped) so
        // small clusters of icon buttons / chips fit comfortably without
        // forcing the caller to size it. Empty slot reserves zero width.
        let slot_w = if header_trailing.is_some() {
            (avail_w * 0.33).clamp(0.0, 160.0)
        } else {
            0.0
        };
        let slot_rect = if slot_w > 0.0 {
            Some(Rect::from_min_max(
                Pos2::new(rect.right() - slot_w, rect.top()),
                Pos2::new(rect.right(), rect.bottom()),
            ))
        } else {
            None
        };
        // Header click area excludes the slot.
        let header_click_rect = match slot_rect {
            Some(s) => Rect::from_min_max(
                rect.min,
                Pos2::new(s.left() - gap_xs(), rect.bottom()),
            ),
            None => rect,
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

            // Caret — proportional, in palette_ct(t).base(Tone::Dim), vertically centered.
            // No leading inset — the caret starts at rect.left so the
            // title text aligns with the PanelSection title above
            // (which sits at the same X via the parent section's body
            // gap_md inset).
            let caret_glyph = if is_open { Icon::CARET_DOWN } else { Icon::CARET_RIGHT };
            let caret_font = FontId::proportional(CARET_FONT);
            let caret_galley = ui.fonts(|f| {
                f.layout_no_wrap(caret_glyph.to_string(), caret_font, palette_ct(t).base(Tone::Dim))
            });
            let cy = rect.center().y;
            let mut x = rect.left();
            painter.galley(
                Pos2::new(x, cy - caret_galley.rect.height() * 0.5),
                caret_galley.clone(),
                palette_ct(t).base(Tone::Dim),
            );
            x += caret_galley.rect.width() + gap_xs();

            // Title — uppercase mono_sm strong (same tier as
            // PanelSection title). Parent accordion categories deserve
            // the same typographic weight as the section above them so
            // the parent/child relationship is clear by hierarchy.
            let title_text = title.to_uppercase();
            let title_font = FontId::monospace(font_sm());
            let title_color = color_alpha(palette_ct(t).base(Tone::Text), 220);
            let title_galley = ui.fonts(|f| {
                f.layout_no_wrap(title_text, title_font, title_color)
            });
            painter.galley(
                Pos2::new(x, cy - title_galley.rect.height() * 0.5),
                title_galley.clone(),
                title_color,
            );
            x += title_galley.rect.width();

            // Count chip — same treatment as PanelSection.count.
            if let Some(n) = count {
                x += gap_xs();
                let count_text = format!("{}", n);
                let count_color = color_alpha(palette_ct(t).base(Tone::Dim), 200);
                let count_font = FontId::monospace(font_sm());
                let count_galley = ui.fonts(|f| {
                    f.layout_no_wrap(count_text, count_font, count_color)
                });
                painter.galley(
                    Pos2::new(x, cy - count_galley.rect.height() * 0.5),
                    count_galley,
                    count_color,
                );
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
        // nested Ui scoped to the slot rect. Layouts right-to-left so the
        // caller can simply `add` widgets and they pack from the right.
        if let (Some(slot), Some(cb)) = (slot_rect, header_trailing) {
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
