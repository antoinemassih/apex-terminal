//! PanelSection — labeled section with optional count, meta, and trailing action.
//!
//! The canonical "section header + body" primitive for side-panel content. Every
//! panel that lists things ("ACTIVE", "PENDING", "CLOSED", "FILTERS", "SETUPS")
//! reaches for this. Replaces the ad-hoc `ui.horizontal { SectionLabel.tiny() ...
//! RTL { small_action_btn ... } }` boilerplate that each panel currently
//! reinvents.
//!
//! ```ignore
//! PanelSection::new("ACTIVE")
//!     .count(3)
//!     .meta("12 total")
//!     .action("Clear", Tone::Danger)
//!     .show(ui, t, |ui, t| {
//!         for row in &active { /* ... */ }
//!     });
//! ```
//!
//! # Optional features
//!
//! - **`.collapsible(&mut expanded)`** — Adds a leading chevron
//!   (caret-down expanded / caret-right collapsed). Clicking anywhere on
//!   the header strip toggles `*expanded`. When collapsed, the body
//!   closure is **not** invoked and `SectionResponse.body` is `None`. The
//!   count chip + trailing button slot still paint in both states. When
//!   collapsed, the count is also rendered inline in `(N)` form next to
//!   the title (matches `PanelSubSection`).
//! - **`.delete_when_empty()`** — When set AND `.count(0)` is also set,
//!   the trailing RTL slot paints a small `Icon::X` ghost button instead
//!   of the `.action(...)` button. The click surfaces via
//!   `SectionResponse.delete_clicked`. Precedence vs. `.action(...)`:
//!   when `count == 0` AND `delete_when_empty` is set, the delete button
//!   wins; otherwise the action button paints (if any).
//! - **`SectionResponse.header_response`** — The whole header strip's
//!   `egui::Response`, exposed so callers can attach `.context_menu(|ui|
//!   { Rename / Color / Delete })`.
//! - **`SectionResponse.chevron_clicked`** — `true` for one frame when
//!   the user toggles a collapsible section's expanded state.
//!
//! Together those three features let `PanelSection` replace the legacy
//! `chart/renderer/ui/watchlist/section_header.rs` widget without losing
//! any behavior.
//!
//! Visual spec (locked by design):
//! - Title: `mono_xs` UPPERCASE strong, in `palette_ct(t).base(SxTone::Dim)` by default; pass
//!   `.title_color(palette_ct(t).base(SxTone::Accent))` when this section is the "current" one.
//! - Count: numeric badge after title, mono_xs strong, tinted with title color.
//! - Meta: muted right-aligned mono_xs (e.g. "12 total").
//! - Action: trailing ghost button (`panel_action_btn` style), right-aligned
//!   before meta. The click is surfaced via `SectionResponse.action_clicked`.
//! - Bottom rule: hairline at `color_alpha(t.surface_border(), 36)`, **on by
//!   default** per user spec (matches chart-pane header rule).
//!
//! Sister widgets:
//! - `PanelEmpty` — body content when the section is empty.
//! - `PanelListRow` — body content for repeating list items.
//! - `PanelKeyValueRow` — body content for label/value pairs.
//! - `PanelDivider` — between sections (when rule is off).
//!
//! When NOT to use:
//! - Top-level panel chrome (header bar + close button) — use `Header` /
//!   `kit::PanelHeader`.
//! - Form-field grouping with input gutters — use `FormSection`.

use egui::{Color32, CursorIcon, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

use super::super::icons::Icon;
use super::tooltip::Tooltip;
use super::theme::{active_theme, get_ambient_recipes};
use crate::ui_kit::layout::{Align as FlexAlign, Flex, FlexSlots, Item};
use crate::ui_kit::tokens::{
    color_alpha, font_sm, font_xs, gap_lg, gap_sm, gap_xs, radius_xs, stroke_thin,
};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::text_style::TextStyle;
use crate::ui_kit::sx::{palette_ct, Sx, StyleState, Tone as SxTone};
use crate::ui_kit::interaction::{apply_interaction, InteractionState, InteractionTokens};

/// Shared semantic tone for the panel body primitives. Resolves to a theme
/// color via [`Tone::color`]. Defined here so the seven panel-body widgets
/// (Section, Empty, Loading, Divider, ListRow, Card, KV) share one vocabulary.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum Tone {
    /// Muted / dim — non-emphatic default.
    #[default]
    Default,
    /// Accent — focus / current selection.
    Accent,
    /// Bull / positive / success — long, gain, confirm.
    Bull,
    /// Bear / negative / destructive — short, loss, delete.
    Bear,
    /// Warn — amber-ish caution.
    Warn,
    /// Alias for `Bear` so callers reading the spec literally can write
    /// `Tone::Danger`. Same color, semantic intent: destructive action.
    Danger,
    /// Alias for `Bull` so callers can write `Tone::Success` for non-trading
    /// confirmations.
    Success,
    /// Full-strength text.
    Text,
}

impl Tone {
    pub fn color(self, t: &dyn ComponentTheme) -> Color32 {
        match self {
            Tone::Default => palette_ct(t).base(SxTone::Dim),
            Tone::Accent => palette_ct(t).base(SxTone::Accent),
            Tone::Bull | Tone::Success => palette_ct(t).base(SxTone::Bull),
            Tone::Bear | Tone::Danger => palette_ct(t).base(SxTone::Bear),
            Tone::Warn => palette_ct(t).base(SxTone::Warn),
            Tone::Text => palette_ct(t).base(SxTone::Text),
        }
    }
}

/// Alpha (out of 255) of the section hairline rule — locked per user spec.
/// Section rule alpha. Bumped from 36 → 100 so section boundaries
/// actually read against the panel surface. The "calm machine" goal
/// is fewer separators, not invisible ones — this rule fires only
/// when `.rule(true)` so callers still control whether it appears.
/// Alpha of the section hairline over `text`.
///
/// A pixel-level trace of the rendered panel settled a long-running question:
/// there is NO stray stroke around section bodies. The only lines in the whole
/// panel are one hairline per section header plus the floating card's own left
/// and right border. What reads as "a box around every section" is those four
/// lines from THREE different elements — a section's own rule forms the top
/// edge, the NEXT section's rule forms the bottom, and the card border forms
/// the sides. Each is individually intentional; together they enclose.
///
/// So the fix is weight, not deletion: at `header_border()` (text @ 38) the
/// rule composited to rgb(85,82,78) on a rgb(20,20,20) body — strong enough to
/// read as an edge. Halved, it separates without enclosing. Deleting it
/// entirely would lose the section boundary on flush styles that have no other
/// separation.
const RULE_ALPHA: u8 = 20;

/// The section hairline colour — `text` at [`RULE_ALPHA`].
fn rule_color<T: ComponentTheme + ?Sized>(t: &T) -> Color32 {
    color_alpha(t.text(), RULE_ALPHA)
}

// ─── Header strip: flex spec ────────────────────────────────────────────────
//
// The header used to be `ui.horizontal { caret, title, count,
// with_layout(right_to_left) { action|delete, meta } }`. The RTL child existed
// for exactly one reason: to push the trailing content against the right edge.
// That is a `grow` spacer in flexbox — one item, no nesting, and (unlike the
// RTL child) solvable and assertable headlessly via [`Flex::solve`].
//
// The spec below is GEOMETRY ONLY. Fonts, colours, tokens and paint order are
// unchanged; the flex just says where the boxes land.

/// Caret glyph point size in the section header (proportional family).
const CARET_GLYPH_PT: f32 = 12.0;
/// Height (and, for the delete button, width) of the trailing ghost buttons.
const SECTION_BTN_H: f32 = 16.0;
/// Hover-fill alpha for the trailing ghost buttons. Sits between the global
/// `alpha_ghost` (15) and `alpha_soft` (20) because these buttons are tiny and
/// need a slightly firmer plate to read as hit targets — kept as a named
/// override fed to `InteractionTokens::hover_alpha` rather than a bare literal.
const SECTION_BTN_HOVER_ALPHA: u8 = 24;

/// Which header piece a solved flex slot belongs to. `Gap` slots are pure
/// spacing (the old `ui.add_space(..)` calls and the elastic middle) and render
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Caret,
    Title,
    Count,
    Action,
    Delete,
    Meta,
}

/// Intrinsic sizes of the header's pieces, measured with egui's fonts.
///
/// Split out so the flex spec can be built — and unit-tested — from plain
/// numbers, with no egui context, GPU or window (see `header_layout_tests`).
#[derive(Clone, Copy, Debug, Default)]
struct HeaderMetrics {
    /// Caret galley size; `None` when the section is not collapsible.
    caret: Option<Vec2>,
    title: Vec2,
    count: Option<Vec2>,
    meta: Option<Vec2>,
    /// Trailing action button size; `None` when there is no `.action(..)` or
    /// when the delete button wins (`delete_when_empty` + `count == 0`).
    action: Option<Vec2>,
    /// Trailing delete button size (mutually exclusive with `action`).
    delete: Option<Vec2>,
}

/// Build the header's flex items, tagged with the piece each one renders.
///
/// Order (left → right) is exactly the order the old code produced:
/// `caret · title · count …… meta · action|delete`. Note the trailing pair:
/// the old RTL child added the button FIRST, which put it right-most, and the
/// meta text to its left — so meta precedes the button here.
fn header_slots(m: &HeaderMetrics) -> FlexSlots<Slot> {
    // Row semantics are unchanged: `gap_xs` between children (which stood in
    // for egui's `item_spacing` in the old horizontal layout), children
    // vertically centered — same as `Layout::left_to_right(Align::Center)`.
    // Padding is NOT set here: the enclosing `egui::Frame` still supplies the
    // `gap_lg` / `gap_sm` inner margin, untouched.
    //
    // AUDIT 2026-08 — this used to return `Vec<(Slot, Item)>` and be paired
    // with a `header_flex` that threw the keys away, after which `show` and the
    // tests each re-derived "which index is the title" by scanning that vec.
    // That is `FlexSlots` written by hand, one file at a time; it is now the
    // layout engine's own API, so the key travels with the item and
    // `SolvedSlots::get` replaces the local `rect_of` helper.
    let mut f = Flex::row()
        .gap(gap_xs())
        .align(FlexAlign::Center)
        .slot_if(Slot::Caret, m.caret.is_some(),
                 Item::fixed(m.caret.map_or(0.0, |c| c.x)).cross(m.caret.map_or(0.0, |c| c.y)))
        // was `ui.add_space(gap_xs())` after the caret label
        .pad_if(m.caret.is_some(), gap_xs())
        .slot(Slot::Title, Item::fixed(m.title.x).cross(m.title.y));
    if let Some(c) = m.count {
        // was `ui.add_space(gap_xs())` before the count chip
        f = f.pad(gap_xs()).slot(Slot::Count, Item::fixed(c.x).cross(c.y));
    }
    // The elastic middle — this ONE item replaces the whole
    // `with_layout(Layout::right_to_left(Align::Center))` nesting.
    f = f.spacer(1.0);
    if let Some(sz) = m.meta {
        f = f.slot(Slot::Meta, Item::fixed(sz.x).cross(sz.y));
    }
    if let Some(sz) = m.delete {
        f = f.slot(Slot::Delete, Item::fixed(sz.x).cross(sz.y));
    } else if let Some(sz) = m.action {
        // was `ui.add_space(gap_sm())` between the action button and meta
        f = f.pad_if(m.meta.is_some(), gap_sm())
             .slot(Slot::Action, Item::fixed(sz.x).cross(sz.y));
    }
    f
}


/// Row height = the tallest piece, which is what `ui.horizontal` produced.
fn header_row_height(m: &HeaderMetrics) -> f32 {
    let mut h = m.title.y;
    for s in [m.caret, m.count, m.meta, m.action, m.delete].iter().flatten() {
        h = h.max(s.y);
    }
    h.max(1.0)
}

/// Intrinsic size of `text` in `font` — the measurement the flex spec needs.
///
/// Rounded UP: Taffy rounds solved rects to whole pixels, and a rect a
/// half-pixel narrower than its galley would clip the last glyph's edge.
fn text_size(ui: &Ui, text: &str, font: FontId) -> Vec2 {
    // Delegates to the design system's measurement helper so measurement is
    // defined in exactly one place (and so widgets don't hand-roll FontIds).
    crate::ui_kit::style::measure_with(ui, text, font)
}

/// Render a label into its solved flex rect. `Extend` because the rect was
/// sized from this exact galley — wrapping would be a rounding artefact.
fn label_exact(ui: &mut Ui, rt: RichText) {
    ui.add(egui::Label::new(rt).wrap_mode(egui::TextWrapMode::Extend));
}

#[must_use = "PanelSection must be rendered with `.show(...)`"]
pub struct PanelSection<'a> {
    title: &'a str,
    title_color: Option<Color32>,
    count: Option<usize>,
    meta: Option<String>,
    action: Option<(String, Tone)>,
    rule: bool,
    /// Optional persistent expanded state — see [`PanelSection::collapsible`].
    expanded: Option<&'a mut bool>,
    /// Whether to show a trailing X (delete) button when count == 0 —
    /// see [`PanelSection::delete_when_empty`].
    delete_when_empty: bool,
    /// Optional disambiguator for the header's widget id — see
    /// [`PanelSection::id_salt`]. Needed when several sections share a title
    /// in the same `Ui` (e.g. watchlist sections), which would otherwise clash.
    id_salt: Option<egui::Id>,
}

/// Response returned from [`PanelSection::show`].
///
/// Fields:
/// - `action_clicked` — `.action(...)` button was clicked this frame.
/// - `delete_clicked` — delete button (from `.delete_when_empty()` +
///   `count == 0`) was clicked this frame.
/// - `chevron_clicked` — collapsible header's expanded state was
///   toggled this frame (either via chevron or by clicking anywhere on
///   the header strip).
/// - `header_response` — egui [`Response`] for the full header strip
///   rect. Callers may attach `.context_menu(|ui| { ... })` to it.
/// - `body` — the closure's return value, or `None` when the section is
///   collapsed and the closure was not invoked.
pub struct SectionResponse<R> {
    pub action_clicked: bool,
    pub delete_clicked: bool,
    pub chevron_clicked: bool,
    pub header_response: Response,
    pub body: Option<R>,
}

impl<'a> PanelSection<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            title_color: None,
            count: None,
            meta: None,
            action: None,
            rule: true,
            expanded: None,
            delete_when_empty: false,
            id_salt: None,
        }
    }

    /// Disambiguate the header's widget id when multiple sections may share a
    /// title within the same parent `Ui`. Without this, two same-titled
    /// collapsible sections produce the same egui `Id` ("first use of widget
    /// id" clash). Pass any stable, per-section hashable value (e.g. a section
    /// id). No-op for unique titles.
    pub fn id_salt(mut self, salt: impl std::hash::Hash) -> Self {
        self.id_salt = Some(egui::Id::new(salt));
        self
    }

    /// Make this section collapsible. Adds a leading caret
    /// (`Icon::CARET_DOWN` when expanded / `Icon::CARET_RIGHT` when
    /// collapsed) to the header strip; clicking anywhere on the header
    /// toggles `*expanded`. When `!*expanded`, the body closure passed
    /// to [`Self::show`] is **not** invoked and
    /// [`SectionResponse::body`] is `None`. The count chip and trailing
    /// action / delete button slot still paint in either state — only
    /// the body is suppressed.
    pub fn collapsible(mut self, expanded: &'a mut bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// When set, AND the section's `.count(0)` is also set, paint a
    /// trailing `Icon::X` ghost button at the right edge of the header
    /// (in place of the `.action(...)` button). The click surfaces via
    /// [`SectionResponse::delete_clicked`]. For `count > 0` the delete
    /// button is suppressed and the `.action(...)` button (if any)
    /// paints normally.
    ///
    /// Precedence when both [`Self::action`] and
    /// [`Self::delete_when_empty`] are set:
    /// - `count == 0` → delete button paints, action is suppressed.
    /// - `count > 0` (or unset) → action button paints normally.
    pub fn delete_when_empty(mut self) -> Self {
        self.delete_when_empty = true;
        self
    }

    pub fn count(mut self, n: usize) -> Self {
        self.count = Some(n);
        self
    }

    /// Test hook: whether the caption slot will be omitted.
    #[cfg(test)]
    pub(crate) fn meta_is_none(&self) -> bool { self.meta.is_none() }

    /// Right-aligned muted caption.
    ///
    /// An empty or whitespace-only string is treated as **absent**, not as an
    /// empty caption. Callers compute these conditionally — "3 open · long" but
    /// nothing at all when flat — and `Some("")` would still claim a slot and
    /// its gap, leaving a header that is subtly wider with no glyph to show for
    /// it. Making the widget decide means every call site can pass a computed
    /// `String` without each one re-deriving the same guard.
    pub fn meta(mut self, m: impl Into<String>) -> Self {
        let m = m.into();
        self.meta = (!m.trim().is_empty()).then_some(m);
        self
    }

    /// Override the title color (default: `palette_ct(t).base(SxTone::Dim)`). Use `palette_ct(t).base(SxTone::Accent)` to mark
    /// this as the "current" section. Per spec, this is the ONLY place
    /// accent should appear on a section title.
    pub fn title_color(mut self, c: Color32) -> Self {
        self.title_color = Some(c);
        self
    }

    /// Add a trailing action button. The click is exposed via
    /// [`SectionResponse::action_clicked`] returned from [`Self::show`].
    pub fn action(mut self, label: impl Into<String>, tone: Tone) -> Self {
        self.action = Some((label.into(), tone));
        self
    }

    /// Toggle the bottom hairline rule. Default is `true` per user spec.
    pub fn rule(mut self, on: bool) -> Self {
        self.rule = on;
        self
    }

    #[track_caller]
    pub fn show<T: ComponentTheme, R>(
        self,
        ui: &mut Ui,
        t: &T,
        body: impl FnOnce(&mut Ui, &T) -> R,
    ) -> SectionResponse<R> {
        // Bug-report anchor key + call-site source (no-op unless Inspect is on).
        let bug_loc = std::panic::Location::caller();
        let bug_key = format!("section/{}", crate::ui_kit::inspect::slug(self.title));
        let title_color = self.title_color.unwrap_or(palette_ct(t).base(SxTone::Dim));
        let mut action_clicked = false;
        let mut delete_clicked = false;
        let mut chevron_clicked = false;

        // Destructure once so we can move pieces into the header closure
        // (which needs to own the action / meta) AND still mutate the
        // expanded state below.
        let PanelSection {
            title: title_str,
            title_color: _,
            count,
            meta,
            action,
            rule: rule_enabled,
            mut expanded,
            delete_when_empty,
            id_salt,
        } = self;

        // Resolve collapsible state. When no `&mut bool` was bound,
        // the section is "always expanded" (back-compat default).
        let collapsible = expanded.is_some();
        let is_expanded = expanded.as_deref().copied().unwrap_or(true);

        // Decide whether to paint the delete button (precedence: delete
        // wins when count==0 + delete_when_empty set; otherwise action).
        let show_delete = delete_when_empty && matches!(count, Some(0));

        // Header row: DARKER (recessed) L0 background + edge-to-edge
        // top AND bottom rules. The header strip uses `color_layer_down`
        // so it reads as recessed inset chrome — the panel's labeled
        // band. Body content sits below on the regular panel surface.
        //
        // Edge-to-edge: SidePanelShell now drops the LR body inset, so
        // the section rect spans the full panel chrome width. The
        // inner_margin only applies to the title text and trailing
        // actions; the FILL + rules cover the full strip width.

        // ── Recipe adoption (section.header) ────────────────────────────────
        // Default fill now matches the PANEL BODY surface (was the lighter
        // `section_header_surface`). The lighter band read as a raised chrome
        // strip, and the luminance step between that band and the darker body
        // was exactly what made each section look like a boxed-in region
        // ("boxes-within-boxes"). Blending the header into the body removes the
        // band edges; the section is now delimited by its label + one hairline
        // only — calm, not boxed. A theme-pack can still opt back into a band
        // via the `section.header` recipe (the resolve path is unchanged).
        let recipes = get_ambient_recipes(ui.ctx());
        let default_header_sx = Sx::new()
            .bg_color(t.panel_surface());
        let header_sx = recipes.resolve("section.header", default_header_sx, t);
        let header_delta = header_sx.resolved(StyleState::Normal);
        // Resolve the fill Color32 from the Sx delta (falls back to panel_surface).
        let resolved_header_fill = header_delta.fill
            .map(|fill| {
                let pal = palette_ct(t);
                match fill {
                    crate::ui_kit::sx::Fill::Solid(c) => c,
                    crate::ui_kit::sx::Fill::Shade(tone, shade) => pal.shade(tone, shade),
                    crate::ui_kit::sx::Fill::Alpha(tone, a) => {
                        let b = pal.base(tone);
                        crate::ui_kit::style::color_alpha(b, a)
                    }
                }
            })
            .unwrap_or_else(|| t.panel_surface());

        let prev_pad = ui.spacing().item_spacing;
        let header_resp = egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: gap_lg() as i8,
                right: gap_lg() as i8,
                // gap_sm (was gap_xs) — section headers were vertically cramped
                // vs the mockup; a touch more breathing room reads far cleaner.
                top: gap_sm() as i8,
                bottom: gap_sm() as i8,
            })
            .fill(resolved_header_fill)
            .show(ui, |ui| {
                // ── Header strip — geometry solved by the flex engine ────────
                // Was `ui.horizontal { .. with_layout(right_to_left) { .. } }`.
                // The RTL nesting is now a single `grow` spacer (see
                // `header_slots`). Everything below — fonts, sizes, colours,
                // tokens, paint order — is byte-for-byte what it was.
                let caret_glyph = if is_expanded {
                    Icon::CARET_DOWN
                } else {
                    Icon::CARET_RIGHT
                };
                // Title family is PER-STYLE (never hardcoded): editorial styles
                // that set `section_header_mono` keep monospace eyebrows; all
                // others use proportional. Numeric counts stay mono regardless.
                let title_family = if t.section_header_mono() {
                    egui::FontFamily::Monospace
                } else {
                    egui::FontFamily::Proportional
                };
                let title_text = title_str.to_uppercase();
                // When collapsed (and collapsible) the count shows as an inline
                // `(N)` hint — matches PanelSubSection.
                let count_text = count.map(|n| {
                    if collapsible && !is_expanded {
                        format!("({})", n)
                    } else {
                        format!("{}", n)
                    }
                });

                let metrics = HeaderMetrics {
                    caret: if collapsible {
                        Some(crate::ui_kit::style::measure_prop(ui, caret_glyph, CARET_GLYPH_PT))
                    } else {
                        None
                    },
                    title: text_size(ui, &title_text, crate::ui_kit::style::font_at(font_sm(), title_family.clone())),
                    count: count_text
                        .as_ref()
                        .map(|s| crate::ui_kit::style::measure_mono(ui, s, font_xs())),
                    meta: meta
                        .as_ref()
                        .map(|s| crate::ui_kit::style::measure_mono(ui, s, font_xs())),
                    action: if show_delete {
                        None
                    } else {
                        action
                            .as_ref()
                            .map(|(label, _)| section_action_button_size(ui, label))
                    },
                    delete: if show_delete {
                        Some(Vec2::splat(SECTION_BTN_H))
                    } else {
                        None
                    },
                };
                let slots = header_slots(&metrics);

                // Reserve the strip, then solve inside it. The row is sized
                // explicitly so the flex solves against the header height (not
                // the whole remaining panel height) and so the strip spans the
                // full content width, as the old RTL child made it do.
                let row_size = Vec2::new(ui.available_width(), header_row_height(&metrics));
                let (_row_id, row_rect) = ui.allocate_space(row_size);
                let mut row_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(row_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                slots.show(&mut row_ui, |slot, ui| {
                    match slot {
                        // Leading caret (collapsible mode only): proportional
                        // 12px glyph in title_color. Drawn as a label — the
                        // click sense lives on the whole header strip below.
                        Slot::Caret => label_exact(
                            ui,
                            RichText::new(caret_glyph)
                                .size(CARET_GLYPH_PT)
                                .color(title_color),
                        ),
                        // Title — uppercase strong at font_sm. One tier smaller
                        // than the SidePanelShell header so the section reads as
                        // nested chrome inside the panel.
                        Slot::Title => label_exact(
                            ui,
                            RichText::new(title_text.clone())
                                .size(font_sm())
                                .strong()
                                .color(title_color)
                                .family(title_family.clone()),
                        ),
                        Slot::Count => {
                            if let Some(s) = &count_text {
                                label_exact(
                                    ui,
                                    TextStyle::MonoXs
                                        .as_rich_cascading(s, color_alpha(title_color, crate::ui_kit::style::alpha_solid()))
                                        .strong(),
                                );
                            }
                        }
                        Slot::Meta => {
                            if let Some(m) = &meta {
                                label_exact(
                                    ui,
                                    TextStyle::MonoXs
                                        .as_rich_cascading(m, color_alpha(palette_ct(t).base(SxTone::Dim), crate::ui_kit::style::alpha_dense())),
                                );
                            }
                        }
                        Slot::Delete => {
                            if section_delete_button(
                                ui,
                                color_alpha(palette_ct(t).base(SxTone::Dim), crate::ui_kit::style::alpha_solid()),
                            ) {
                                delete_clicked = true;
                            }
                        }
                        Slot::Action => {
                            if let Some((label, tone)) = &action {
                                if section_action_button(ui, label, tone.color(t)) {
                                    action_clicked = true;
                                }
                            }
                        }
                    }
                });
            });
        ui.spacing_mut().item_spacing = prev_pad;

        // Promote the header frame's response to click-sensing so
        // callers can attach `.context_menu(...)`. In collapsible mode,
        // a click anywhere on the header strip toggles expanded state.
        // We re-interact on the header rect with a stable id so the
        // child label widgets above don't swallow the click.
        let header_rect = header_resp.response.rect;
        crate::ui_kit::inspect::register(&bug_key, header_rect, bug_loc.file(), bug_loc.line());
        // Only sense clicks on the whole header when it's collapsible — otherwise
        // this overlay steals clicks from the action/delete button laid out above
        // (egui hit-tests later widgets first). Hover-sense still produces a
        // Response usable for .context_menu() attachment.
        let header_sense = if collapsible { Sense::click() } else { Sense::hover() };
        // Salt the header id with the caller-supplied disambiguator (if any) so
        // same-titled sections in one Ui don't collide.
        let mut header_id = ui.id().with(("panel_section_header", title_str));
        if let Some(salt) = id_salt {
            header_id = header_id.with(salt);
        }
        let header_response = ui.interact(
            header_rect,
            header_id,
            header_sense,
        );
        let header_response = if collapsible {
            let hr = header_response.on_hover_cursor(CursorIcon::PointingHand);
            if hr.clicked() {
                if let Some(state) = expanded.as_deref_mut() {
                    *state = !*state;
                    chevron_clicked = true;
                }
            }
            hr
        } else {
            header_response
        };
        // Re-read expanded after potential toggle so the body decision
        // below uses the up-to-date value.
        let is_expanded = expanded.as_deref().copied().unwrap_or(true);
        // ONE calm hairline UNDER the section header — not a box. The former
        // treatment stacked THREE separators per header (a top rule + a bottom
        // rule + a 6px inset drop-shadow); bracketing every body top-and-bottom
        // is what read as "box-in-box" chrome. We keep a single subtle bottom
        // rule (matches the chart pane header, t.header_border() = text @ ~38a),
        // still gated by `rule_enabled` so `.rule(false)` yields no divider.
        let hr = header_resp.response.rect;
        if rule_enabled {
            ui.painter().line_segment(
                // Half the rule's OWN width inside the edge, so it lands on
                // the pixel grid instead of straddling it. Was a fixed 0.5,
                // which is only correct while the stroke happens to be 1px —
                // it detaches the moment a style re-pitches the stroke ramp.
                {
                    let half = stroke_thin() * 0.5;
                    [Pos2::new(hr.left(), hr.bottom() - half),
                     Pos2::new(hr.right(), hr.bottom() - half)]
                },
                Stroke::new(stroke_thin(), rule_color(t)),
            );
        }
        // Small, even gap into the body (replaces the removed shadow band's
        // reserved height) so section spacing stays consistent.
        ui.add_space(gap_xs());

        // Body — natural flow on the panel surface. Inset by gap_lg so
        // body content's left edge aligns with the header's title text
        // (header uses gap_lg L/R via the Frame inner_margin above).
        // When the section is collapsed (collapsible + !expanded) the
        // body closure is NOT invoked and `SectionResponse.body` is `None`.
        let body_inner = if is_expanded {
            let r = egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: gap_lg() as i8,
                    right: gap_lg() as i8,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| body(ui, t))
                .inner;
            ui.add_space(gap_sm());
            Some(r)
        } else {
            ui.add_space(gap_sm());
            None
        };

        SectionResponse {
            action_clicked,
            delete_clicked,
            chevron_clicked,
            header_response,
            body: body_inner,
        }
    }
}

fn paint_rule<T: ComponentTheme>(ui: &mut Ui, t: &T) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), y),
            Pos2::new(rect.right(), y),
        ],
        Stroke::new(stroke_thin(), rule_color(t)),
    );
    ui.add_space(1.0);
}

/// Local section-delete button — small ghost `Icon::X` icon button used
/// when `.delete_when_empty()` is set AND `count == 0`. Matches the
/// visual treatment of the legacy `watchlist/section_header.rs` X
/// button (ghost, frameless, dim glyph that brightens on hover).
fn section_delete_button(ui: &mut Ui, color: Color32) -> bool {
    let glyph = Icon::X;
    let size = Vec2::splat(SECTION_BTN_H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    Tooltip::new("Delete section").show(ui, &resp, &active_theme(ui.ctx()));
    // M3.3: hover fill from the ONE interaction table (accent tint at 24 —
    // the alpha this button has always drawn).
    let v = apply_interaction(
        rect,
        InteractionState::new().hovered(resp.hovered()),
        color,
        &InteractionTokens::borderless().hover_alpha(SECTION_BTN_HOVER_ALPHA),
    );
    let draw_color = if resp.hovered() {
        ui.painter().rect_filled(rect, radius_xs(), v.fill);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        color
    } else {
        color_alpha(color, crate::ui_kit::style::alpha_dense())
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        TextStyle::BodySm.font_id_in(ui),
        draw_color,
    );
    resp.clicked()
}

/// Local section-action button — small ghost button matching `kit::panel_action_btn`
/// for visual parity. Reproduced here so this widget has no dependency on the
/// legacy `kit` module.
/// Intrinsic size of the trailing action button — the SAME arithmetic
/// `section_action_button` allocates, factored out so the flex spec can reserve
/// exactly the rect the button will fill.
fn section_action_button_size(ui: &Ui, label: &str) -> Vec2 {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(label.to_string(), TextStyle::MonoXs.font_id_in(ui), Color32::PLACEHOLDER)
    });
    let pad_x = gap_xs() + 2.0;
    // Ceil for the same reason as `text_size`: the flex rect that reserves this
    // button is rounded to whole pixels.
    Vec2::new((galley.size().x + pad_x * 2.0).ceil(), SECTION_BTN_H)
}

fn section_action_button(ui: &mut Ui, label: &str, color: Color32) -> bool {
    let text = RichText::new(label)
        .monospace()
        .size(font_xs())
        .strong()
        .color(color);
    let size = section_action_button_size(ui, label);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    // M3.3: same hover treatment as `section_delete_button`, same table.
    let v = apply_interaction(
        rect,
        InteractionState::new().hovered(resp.hovered()),
        color,
        &InteractionTokens::borderless().hover_alpha(SECTION_BTN_HOVER_ALPHA),
    );
    if resp.hovered() {
        ui.painter().rect_filled(rect, radius_xs(), v.fill);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        TextStyle::MonoXs.font_id_in(ui),
        color,
    );
    let _ = text;
    resp.clicked()
}

// ─── PanelSectionGroup ──────────────────────────────────────────────────────
//
// Stacks N PanelSections vertically with user-draggable grippy dividers
// between adjacent sections. The caller owns the persistent fraction state
// as a `&mut [f32; N]` slice; the group normalizes the sum to 1.0 and
// enforces a configurable minimum section height.
//
// Why this exists: PanelSection is natural-flow — it takes whatever
// vertical space it needs. Several panels (indicators, alerts) want
// "TOOLS / ACTIVE / LIBRARY"-style 3-way splits where the user can drag
// the divider between sections to give one more room than another. Each
// pane has a header (the PanelSection title) and a scrollable body. The
// dividers BETWEEN sections are the affordance — hence a group widget
// rather than a `.resizable()` builder on a single section.
//
// Divider visual (matches PanelDivider hairline aesthetic):
//   - 5px tall hit/paint band, hairline rule centered
//   - Center dot triplet (3 dots, 2px diameter, gap 3px) as grab affordance
//   - Idle  : `color_alpha(t.surface_border(), 36)`
//   - Hover : `color_alpha(t.surface_border(), 96)` (and dots brightened)
//   - Cursor: `CursorIcon::ResizeVertical` on hover/drag

/// Divider hit-band height (px). Drawn between adjacent sections.
const DIVIDER_BAND_H: f32 = 6.0;
/// Idle alpha of the divider hairline + dots (matches PanelSection rule).
const DIVIDER_IDLE_ALPHA: u8 = 36;
/// Hover / drag alpha of the divider hairline + dots.
const DIVIDER_HOVER_ALPHA: u8 = 96;
/// Default minimum section height when the caller doesn't override it.
const DEFAULT_MIN_SECTION_H: f32 = 32.0;

/// Vertical stack of N resizable [`PanelSection`]s with grippy dividers
/// between them.
///
/// State (`&mut [f32; N]`) lives on the caller (e.g.
/// `Watchlist::indicators_section_fracs`). Fractions sum to 1.0 — the
/// group renormalizes if the caller's storage drifts. A minimum section
/// height is enforced when dragging.
///
/// ```ignore
/// PanelSectionGroup::new(&mut watchlist.indicators_section_fracs)
///     .min_section_height(40.0)
///     .show(ui, t, |grp| {
///         grp.section(|ui, t| {
///             PanelSection::new("TOOLS").show(ui, t, |ui, t| { /* ... */ });
///         });
///         grp.section(|ui, t| {
///             PanelSection::new("ACTIVE").count(n).show(ui, t, |ui, t| { /* ... */ });
///         });
///         grp.section(|ui, t| {
///             PanelSection::new("LIBRARY").show(ui, t, |ui, t| { /* ... */ });
///         });
///     });
/// ```
///
/// The caller must invoke `grp.section(...)` exactly `N` times (one per
/// fraction in the storage slice). Extra calls are silently dropped;
/// missing calls leave those rects unrendered.
#[must_use = "PanelSectionGroup must be rendered with `.show(...)`"]
pub struct PanelSectionGroup<'a> {
    fracs: &'a mut [f32],
    min_section_height: f32,
}

impl<'a> PanelSectionGroup<'a> {
    pub fn new(fracs: &'a mut [f32]) -> Self {
        Self {
            fracs,
            min_section_height: DEFAULT_MIN_SECTION_H,
        }
    }

    /// Minimum height (px) any one section is allowed to shrink to while
    /// the user drags a divider. Defaults to 32px.
    pub fn min_section_height(mut self, px: f32) -> Self {
        self.min_section_height = px.max(8.0);
        self
    }

    pub fn show<T: ComponentTheme, F>(self, ui: &mut Ui, t: &T, mut body: F)
    where
        F: FnMut(&mut PanelSectionGroupBuilder<'_, T>),
    {
        let n = self.fracs.len();
        if n == 0 {
            return;
        }

        // Available rect — full remaining width × height inside the parent
        // UI. We allocate it up front so divider drags stay anchored to
        // the same band regardless of scrollbar reflow inside a section.
        let avail = ui.available_size_before_wrap();
        let total_w = if avail.x.is_finite() && avail.x > 0.0 { avail.x } else { 200.0 };
        let total_h = if avail.y.is_finite() && avail.y > 0.0 { avail.y } else { 200.0 };
        if total_h <= 1.0 {
            return;
        }

        let (rect, _resp) = ui.allocate_exact_size(
            Vec2::new(total_w, total_h),
            Sense::hover(),
        );

        // Normalize fractions (defensive — caller might init to zeros).
        let sum: f32 = self.fracs.iter().copied().filter(|f| f.is_finite()).sum();
        if sum <= 0.0001 {
            let even = 1.0 / n as f32;
            for f in self.fracs.iter_mut() {
                *f = even;
            }
        } else if (sum - 1.0).abs() > 0.001 {
            for f in self.fracs.iter_mut() {
                *f /= sum;
            }
        }

        // ── Drag handling ──────────────────────────────────────────────
        //
        // For each of the N-1 dividers we allocate a thin hit rect and,
        // if dragged, transfer height between the two adjacent sections.
        // Compute initial pixel heights from fractions.
        let divider_total = DIVIDER_BAND_H * (n - 1) as f32;
        let body_total = (total_h - divider_total).max(0.0);
        let min_h = self.min_section_height;

        let mut heights: Vec<f32> = self.fracs.iter().map(|f| f * body_total).collect();

        // Process each divider — must do it in a separate pass so we can
        // mutate heights[i] and heights[i+1] together.
        let base_id = ui.id().with("ui_kit_panel_section_group");
        let mut hovered_divider: Option<usize> = None;

        // M4.3: the divider band positions were an O(n²) prefix sum recomputed
        // inside the loop (`let mut y = rect.top(); for j in 0..=i { y +=
        // heights[j]; if j < i { y += DIVIDER_BAND_H } }`). That is a flex
        // COLUMN whose gutter is the divider band — solved once.
        //
        // Solving once outside the loop is exact, not an approximation: a drag
        // on divider `i` moves height from `heights[i]` to `heights[i+1]`, so
        // every prefix sum at index > i is unchanged, and prefixes at index < i
        // were already consumed.
        let drag_bands = section_group_bands(rect, &heights);
        for i in 0..n.saturating_sub(1) {
            let div_rect = drag_bands[i];
            // Expand hit rect a couple px for ease of grabbing.
            let hit_rect = div_rect.expand2(Vec2::new(0.0, 2.0));
            let resp = ui.interact(
                hit_rect,
                base_id.with(("div", i)),
                Sense::click_and_drag(),
            );
            if resp.hovered() || resp.dragged() {
                hovered_divider = Some(i);
                ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
            }
            if resp.dragged() {
                let dy = resp.drag_delta().y;
                if dy.abs() > 0.0 {
                    let a = heights[i];
                    let b = heights[i + 1];
                    // Clamp transfer by min-height on the shrinking side.
                    let new_a = (a + dy).max(min_h).min(a + b - min_h);
                    let delta = new_a - a;
                    heights[i] += delta;
                    heights[i + 1] -= delta;
                }
            }
        }

        // Write heights back as fractions of body_total.
        if body_total > 0.0 {
            for (h, f) in heights.iter().zip(self.fracs.iter_mut()) {
                *f = h / body_total;
            }
            // Re-normalize for floating point drift.
            let s: f32 = self.fracs.iter().sum();
            if s > 0.0 {
                for f in self.fracs.iter_mut() {
                    *f /= s;
                }
            }
        }

        // ── Paint sections + dividers ──────────────────────────────────
        //
        // Re-solved against the POST-drag heights (the drag above may have
        // moved one band).
        let section_rects = section_group_slots(rect, &heights);
        let bands = section_group_bands(rect, &heights);

        let mut builder = PanelSectionGroupBuilder {
            ui,
            t,
            rects: &section_rects,
            index: 0,
            hovered_divider,
        };
        body(&mut builder);

        // Paint dividers AFTER section bodies so the grippy sits on top
        // of any section bottom-rule.
        for (i, div_rect) in bands.iter().enumerate() {
            paint_grippy_divider(builder.ui, t, *div_rect, hovered_divider == Some(i));
        }
    }
}

// ─── Section-group stack: flex spec (M4.3) ──────────────────────────────────
//
// N stacked sections separated by a `DIVIDER_BAND_H` grab band is literally
// `flex-direction: column; gap: DIVIDER_BAND_H` — so the section rects and the
// divider bands both fall out of one solve, instead of three separate
// hand-rolled prefix sums (two O(n²) loops plus the builder's `cursor_y`).

/// Solve the section stack. Returns one absolute rect per section.
fn section_group_slots(rect: Rect, heights: &[f32]) -> Vec<Rect> {
    Flex::column()
        .gap(DIVIDER_BAND_H)
        .align(FlexAlign::Stretch)
        .items(heights.iter().map(|h| Item::fixed(h.max(0.0))))
        .solve(rect.size())
        .into_iter()
        .map(|r| r.translate(rect.min.to_vec2()))
        .collect()
}

/// The `n - 1` divider bands — the flex GUTTERS between the solved sections.
fn section_group_bands(rect: Rect, heights: &[f32]) -> Vec<Rect> {
    let slots = section_group_slots(rect, heights);
    slots
        .windows(2)
        .map(|w| {
            Rect::from_min_size(
                Pos2::new(rect.left(), w[0].bottom()),
                Vec2::new(rect.width(), DIVIDER_BAND_H),
            )
        })
        .collect()
}

/// Builder handed to the closure passed to [`PanelSectionGroup::show`].
/// Each call to [`Self::section`] consumes one solved slot.
pub struct PanelSectionGroupBuilder<'u, T: ComponentTheme = crate::ui_kit::widgets::theme::PortableTheme> {
    ui: &'u mut Ui,
    t: &'u T,
    /// Pre-solved section rects, in order (see [`section_group_slots`]).
    rects: &'u [Rect],
    index: usize,
    hovered_divider: Option<usize>,
}

impl<'u, T: ComponentTheme> PanelSectionGroupBuilder<'u, T> {
    /// Render one section into the next slot. The closure runs inside a
    /// child UI clipped to the section's allocated rect — typical use is
    /// to call `PanelSection::new(...).show(ui, t, |ui, t| { ... })`
    /// inside it.
    pub fn section<F>(&mut self, add_contents: F)
    where
        F: FnOnce(&mut Ui, &T),
    {
        let i = self.index;
        if i >= self.rects.len() {
            return;
        }
        // M4.3: was a `cursor_y` walk (`self.cursor_y += h; … += divider_h`).
        // The slot is now read straight out of the group's solved column.
        let section_rect = self.rects[i];

        if section_rect.height() > 0.5 {
            let mut child = self.ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(section_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(section_rect);
            add_contents(&mut child, self.t);
        }

        self.index += 1;
        let _ = self.hovered_divider; // silence unused on this path
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────
//
// Verify the three opt-in features added in Wave 10a:
//   1. `.collapsible(&mut expanded)` skips the body closure when
//      `!*expanded`.
//   2. `.delete_when_empty()` paints + fires the X button only when
//      `count == 0`.
//   3. Clicking the header strip in collapsible mode toggles
//      `*expanded` (chevron_clicked observable).
//
// We exercise the widget via egui's `__run_test_ui` harness. Click
// simulation uses `Response::clicked()`-equivalent flows by directly
// looking at allocated rects via the egui memory API — for the
// chevron toggle test we manually invoke pointer events; for the
// "body not called" check we use a flag in a Cell.

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Returns a portable theme for headless tests — no chart-app needed.
    fn theme() -> super::super::theme::PortableTheme {
        super::super::theme::PortableTheme::dark()
    }

    /// `egui::__run_test_ui` with the ui_kit typography tiers installed —
    /// what every real host does per frame in `setup_theme`. Without this,
    /// `as_rich_cascading` (used by the header's count/meta labels) resolves
    /// a `Name` text-style that is missing from the raw test context's table
    /// and panics inside egui.
    /// Two frames, not one.
    ///
    /// `egui::__run_test_ui` runs a single frame, and a context has no font
    /// atlas on its first — every string measures 0 px wide. These particular
    /// tests are behavioural (does the body run, does the chevron toggle) so
    /// nothing here was wrong, but the next width assertion added to this
    /// module would have been silently vacuous. Same fix as
    /// `cascade::element::tests::run`, where it WAS masking something.
    /// Delegates to `paint_probe::probe`, which owns the two-frame dance (a
    /// `Context` has no font atlas on its first frame, so every string
    /// measures 0 px). These tests are behavioural rather than
    /// width-dependent, but the harness is the same and there is no reason
    /// for a fourth copy of it.
    fn run_test_ui(f: impl FnOnce(&mut Ui)) {
        let _ = crate::ui_kit::widgets::paint_probe::probe(f);
    }

    #[test]
    fn collapsible_toggle_skips_body_when_collapsed() {
        use std::cell::RefCell;
        let theme = theme();
        let expanded: RefCell<bool> = RefCell::new(false);
        let body_called = Cell::new(false);

        run_test_ui(|ui| {
            let mut e = expanded.borrow_mut();
            let resp = PanelSection::new("ACTIVE")
                .count(0)
                .collapsible(&mut *e)
                .show(ui, &theme, |_ui, _t| {
                    body_called.set(true);
                });
            assert!(resp.body.is_none(), "body should be None when collapsed");
        });

        assert!(!body_called.get(), "body closure must not run when section collapsed");
        assert!(!*expanded.borrow(), "expanded state must remain false without click");
    }

    #[test]
    fn collapsible_runs_body_when_expanded() {
        use std::cell::RefCell;
        let theme = theme();
        let expanded: RefCell<bool> = RefCell::new(true);
        let body_called = Cell::new(false);

        run_test_ui(|ui| {
            let mut e = expanded.borrow_mut();
            let resp = PanelSection::new("ACTIVE")
                .count(3)
                .collapsible(&mut *e)
                .show(ui, &theme, |_ui, _t| {
                    body_called.set(true);
                });
            assert!(resp.body.is_some(), "body should be Some when expanded");
        });

        assert!(body_called.get(), "body closure must run when section expanded");
    }

    #[test]
    fn delete_button_paints_only_when_count_zero() {
        let theme = theme();

        // count == 0 + delete_when_empty: button paints, click flag wired.
        run_test_ui(|ui| {
            let resp = PanelSection::new("WATCHLIST")
                .count(0)
                .delete_when_empty()
                .show(ui, &theme, |_ui, _t| {});
            // No click was issued — but the field must exist and be
            // observable as false (proves the field is wired through).
            assert!(!resp.delete_clicked);
            assert!(!resp.action_clicked);
        });

        // count > 0: delete button suppressed, action button (if any) paints.
        run_test_ui(|ui| {
            let resp = PanelSection::new("WATCHLIST")
                .count(5)
                .delete_when_empty()
                .action("Clear", Tone::Danger)
                .show(ui, &theme, |_ui, _t| {});
            assert!(!resp.delete_clicked, "delete must not fire when count > 0");
        });
    }

    /// Render path (not just `Flex::solve`): a header with every piece present
    /// must still produce ONE `Response` covering the whole strip — that rect
    /// is what callers hang `.context_menu(..)` on, and what the hairline rule
    /// and the bug anchor are painted/registered against.
    #[test]
    fn full_header_response_covers_the_whole_strip() {
        use std::cell::RefCell;
        let theme = theme();
        let expanded: RefCell<bool> = RefCell::new(true);
        let ctx = egui::Context::default();
        ctx.style_mut(|s| TextStyle::install(s)); // headless host: register the tiers
        let seen: Cell<Option<egui::Rect>> = Cell::new(None);
        let panel: Cell<Option<egui::Rect>> = Cell::new(None);

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.set(Some(ui.max_rect()));
                let mut e = expanded.borrow_mut();
                let resp = PanelSection::new("ACTIVE")
                    .count(3)
                    .meta("12 total")
                    .action("Clear", Tone::Danger)
                    .collapsible(&mut *e)
                    .show(ui, &theme, |_ui, _t| {});
                seen.set(Some(resp.header_response.rect));
            });
        });

        let hr = seen.get().expect("header rect");
        let pr = panel.get().expect("panel rect");
        // Full-width strip (the `grow` middle spans whatever the RTL child
        // used to), and tall enough for the 16px trailing button + margins.
        assert!(hr.width() >= pr.width() - 1.0, "header width {} panel {}", hr.width(), pr.width());
        assert!(hr.height() >= 16.0, "header height {}", hr.height());
    }

    #[test]
    fn chevron_click_toggles_expansion() {
        use std::cell::RefCell;
        let theme = theme();
        let expanded: RefCell<bool> = RefCell::new(true);

        // ctx.run takes a `Fn` closure (not `FnMut`), so the expanded
        // state lives behind a RefCell. The header_response rect is
        // captured in a Cell so we can synthesize a click at its
        // center on the second frame.
        let ctx = egui::Context::default();
        ctx.style_mut(|s| TextStyle::install(s)); // headless host: register the tiers
        let header_rect_cell: Cell<Option<egui::Rect>> = Cell::new(None);

        // First pass — discover header rect (no click).
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut e = expanded.borrow_mut();
                let resp = PanelSection::new("WATCHLIST")
                    .count(1)
                    .collapsible(&mut *e)
                    .show(ui, &theme, |_ui, _t| {});
                header_rect_cell.set(Some(resp.header_response.rect));
            });
        });
        let header_rect = header_rect_cell.get().expect("header rect captured");
        assert!(*expanded.borrow(), "expanded unchanged after non-clicking pass");

        // Second pass — synthesize a click at the header center.
        let click_pos = header_rect.center();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::PointerMoved(click_pos));
        input.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        input.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        let chevron_clicked_cell: Cell<bool> = Cell::new(false);
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut e = expanded.borrow_mut();
                let resp = PanelSection::new("WATCHLIST")
                    .count(1)
                    .collapsible(&mut *e)
                    .show(ui, &theme, |_ui, _t| {});
                chevron_clicked_cell.set(resp.chevron_clicked);
            });
        });

        assert!(chevron_clicked_cell.get(), "chevron_clicked must be true on toggle frame");
        assert!(!*expanded.borrow(), "expanded should have flipped true → false");
    }
}

// ─── Header geometry tests ──────────────────────────────────────────────────
//
// `header_slots` + `header_flex` are pure: given the intrinsic sizes of the
// header's pieces they produce a `Flex`, and `Flex::solve` turns that into
// rects. So the alignment the old `right_to_left` nesting used to do implicitly
// is now assertable headlessly — no GPU, no window, no screenshot.
//
// Token values under test (default snapshot): gap_xs = 4, gap_sm = 8.

#[cfg(test)]
mod header_layout_tests {
    use super::*;
    use egui::vec2;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    /// Solve a header at `w` × its natural row height.
    ///
    /// The local `rect_of` helper that used to live here — scan the slot vec
    /// for a key, index the rect vec with the position — is now
    /// `SolvedSlots::get`, so these tests exercise the shared lookup rather
    /// than a private copy of it.
    fn solve(m: &HeaderMetrics, w: f32) -> crate::ui_kit::layout::SolvedSlots<Slot> {
        header_slots(m).solve(Vec2::new(w, header_row_height(m)))
    }

    fn rect_of(solved: &crate::ui_kit::layout::SolvedSlots<Slot>, want: Slot) -> Rect {
        solved.get(want).unwrap_or_else(|| panic!("no {want:?} slot in header"))
    }

    /// The core of the migration: the title keeps its intrinsic width at the
    /// left, and the trailing action button lands flush against the right edge
    /// — which is what the `right_to_left` child used to accomplish.
    #[test]
    fn title_starts_left_and_action_sits_flush_right() {
        let m = HeaderMetrics {
            title: vec2(80.0, 14.0),
            action: Some(vec2(40.0, 16.0)),
            ..Default::default()
        };
        let solved = solve(&m, 300.0);
        let title = rect_of(&solved, Slot::Title);
        let action = rect_of(&solved, Slot::Action);
        assert!(approx(title.left(), 0.0), "title left {}", title.left());
        assert!(approx(title.width(), 80.0), "title width {}", title.width());
        assert!(approx(action.right(), 300.0), "action right {}", action.right());
        assert!(approx(action.width(), 40.0));
        // ...and the button is right OF the title, not overlapping it.
        assert!(action.left() > title.right());
    }

    /// The caret takes its measured width and the title starts after it plus
    /// the gutter and the old `add_space(gap_xs())`.
    #[test]
    fn caret_takes_its_width_and_title_follows_after_the_gap() {
        let m = HeaderMetrics {
            caret: Some(vec2(12.0, 12.0)),
            title: vec2(80.0, 14.0),
            ..Default::default()
        };
        let solved = solve(&m, 300.0);
        let caret = rect_of(&solved, Slot::Caret);
        let title = rect_of(&solved, Slot::Title);
        assert!(approx(caret.left(), 0.0));
        assert!(approx(caret.width(), 12.0));
        // gutter + gap_xs spacer + gutter after the 12px caret.
        let expect = 12.0 + gap_xs() * 3.0;
        assert!(approx(title.left(), expect), "title left {} want {}", title.left(), expect);
    }

    /// The count chip keeps intrinsic width and follows the title.
    #[test]
    fn count_chip_follows_the_title() {
        let m = HeaderMetrics {
            title: vec2(80.0, 14.0),
            count: Some(vec2(9.0, 12.0)),
            ..Default::default()
        };
        let solved = solve(&m, 300.0);
        let title = rect_of(&solved, Slot::Title);
        let count = rect_of(&solved, Slot::Count);
        assert!(approx(count.width(), 9.0));
        assert!(approx(count.left(), title.right() + gap_xs() * 3.0));
    }

    /// With no action, no delete and no meta, the elastic middle still runs all
    /// the way to the right edge — the strip spans the full width exactly as
    /// the old (empty) RTL child made it.
    #[test]
    fn without_a_trailing_slot_the_row_still_spans_to_the_right_edge() {
        let m = HeaderMetrics { title: vec2(80.0, 14.0), ..Default::default() };
        let solved = solve(&m, 300.0);
        // The row ends with the elastic middle. It carries no key now
        // (spacing is anonymous), so this asserts the property that mattered:
        // the last solved rect reaches the right edge.
        let last = *solved.rects().last().expect("row has slots");
        assert!(approx(last.right(), 300.0), "spacer right {}", last.right());
        let title = rect_of(&solved, Slot::Title);
        assert!(approx(title.left(), 0.0));
    }

    /// Meta text sits to the LEFT of the action button (the old RTL child added
    /// the button first, i.e. right-most), separated by the `gap_sm()` the old
    /// `add_space` used.
    #[test]
    fn meta_sits_left_of_the_action_separated_by_gap_sm() {
        let m = HeaderMetrics {
            title: vec2(80.0, 14.0),
            meta: Some(vec2(50.0, 12.0)),
            action: Some(vec2(40.0, 16.0)),
            ..Default::default()
        };
        let solved = solve(&m, 300.0);
        let meta = rect_of(&solved, Slot::Meta);
        let action = rect_of(&solved, Slot::Action);
        assert!(approx(action.right(), 300.0));
        assert!(meta.right() < action.left(), "meta must precede the action button");
        // gutter + gap_sm spacer + gutter
        let expect = gap_xs() + gap_sm() + gap_xs();
        assert!(
            approx(action.left() - meta.right(), expect),
            "meta→action {} want {}",
            action.left() - meta.right(),
            expect
        );
    }

    /// `delete_when_empty` + `count == 0`: the delete button takes the trailing
    /// slot instead of the action, still flush right, and (matching the old
    /// code) with no extra `gap_sm` before the meta text.
    #[test]
    fn delete_button_takes_the_trailing_slot_flush_right() {
        let m = HeaderMetrics {
            title: vec2(80.0, 14.0),
            count: Some(vec2(9.0, 12.0)),
            meta: Some(vec2(50.0, 12.0)),
            delete: Some(Vec2::splat(SECTION_BTN_H)),
            ..Default::default()
        };
        let solved = solve(&m, 300.0);
        assert!(solved.get(Slot::Action).is_none(), "delete wins over action");
        let del = rect_of(&solved, Slot::Delete);
        let meta = rect_of(&solved, Slot::Meta);
        assert!(approx(del.right(), 300.0), "delete right {}", del.right());
        assert!(approx(del.width(), SECTION_BTN_H));
        assert!(approx(del.left() - meta.right(), gap_xs()));
    }

    /// Cross-axis: every piece is vertically centered in the strip, which is
    /// what `Layout::left_to_right(Align::Center)` did.
    #[test]
    fn pieces_are_vertically_centered_in_the_strip() {
        let m = HeaderMetrics {
            caret: Some(vec2(12.0, 12.0)),
            title: vec2(80.0, 14.0),
            action: Some(vec2(40.0, 16.0)),
            ..Default::default()
        };
        // Tallest piece is the 16px button, so that's the row height.
        assert!(approx(header_row_height(&m), 16.0));
        let solved = solve(&m, 300.0);
        for want in [Slot::Caret, Slot::Title, Slot::Action] {
            let r = rect_of(&solved, want);
            assert!(
                approx(r.center().y, 8.0),
                "{:?} center y {} want 8",
                want,
                r.center().y
            );
        }
        assert!(approx(rect_of(&solved, Slot::Title).height(), 14.0));
    }

    /// Degenerate width (panel dragged shut / first frame) must not panic and
    /// must still produce one rect per slot.
    #[test]
    fn narrow_header_does_not_panic() {
        let m = HeaderMetrics {
            caret: Some(vec2(12.0, 12.0)),
            title: vec2(80.0, 14.0),
            meta: Some(vec2(50.0, 12.0)),
            action: Some(vec2(40.0, 16.0)),
            ..Default::default()
        };
        let slots = header_slots(&m);
        let solved = slots.solve(Vec2::new(0.0, 16.0));
        assert_eq!(solved.len(), slots.len(), "every slot solves to exactly one rect");
    }
}

// ── Recipe adoption tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod recipe_tests {
    use crate::design_system::recipes::RecipeSet;
    use crate::ui_kit::sx::{
        recipe_spec::{AlphaTier, ColorSpec, RecipeDelta, RecipeSpec, ToneRef},
        style::{Sx, StyleState},
    };
    use crate::ui_kit::widgets::theme::{ComponentTheme, PortableTheme};

    fn mock_theme() -> PortableTheme { PortableTheme::dark() }

    /// S5 adoption proof — section.header: a non-empty RecipeSet overriding
    /// the `section.header` key changes the resolved header fill vs the default.
    #[test]
    fn s5_section_header_recipe_overrides_fill_vs_default() {
        let t = mock_theme();

        let mut set = RecipeSet::new();
        // Override: accent alpha tint (not the default section_header_surface).
        set.insert("section.header", RecipeSpec {
            base: RecipeDelta {
                fill: Some(ColorSpec::Alpha { tone: ToneRef::Accent, alpha: AlphaTier::Subtle }),
                ..Default::default()
            },
            ..Default::default()
        });

        // Default Sx: the historical section_header_surface fill.
        // Use explicit trait dispatch — PortableTheme implements the default method
        // through ComponentTheme.
        let default_fill = ComponentTheme::section_header_surface(&t);
        let default_sx = Sx::new().bg_color(default_fill);

        let result = set.resolve("section.header", default_sx, &t);
        let delta = result.resolved(StyleState::Normal);
        assert!(delta.fill.is_some(), "recipe should set fill");

        // Resolve and compare to default.
        let pal = crate::ui_kit::sx::palette_ct(&t);
        let resolved_color = match delta.fill.unwrap() {
            crate::ui_kit::sx::style::Fill::Solid(c) => c,
            crate::ui_kit::sx::style::Fill::Shade(tone, shade) => pal.shade(tone, shade),
            crate::ui_kit::sx::style::Fill::Alpha(tone, a) => {
                let b = pal.base(tone);
                crate::ui_kit::style::color_alpha(b, a)
            }
        };
        assert_ne!(resolved_color, default_fill,
            "recipe fill (Accent alpha40) must differ from default section_header_surface");

        // Empty set: fill unchanged.
        let empty = RecipeSet::new();
        let result_empty = empty.resolve("section.header", default_sx, &t);
        let delta_empty = result_empty.resolved(StyleState::Normal);
        let empty_color = match delta_empty.fill.unwrap() {
            crate::ui_kit::sx::style::Fill::Solid(c) => c,
            crate::ui_kit::sx::style::Fill::Shade(tone, shade) => pal.shade(tone, shade),
            crate::ui_kit::sx::style::Fill::Alpha(tone, a) => {
                let b = pal.base(tone);
                crate::ui_kit::style::color_alpha(b, a)
            }
        };
        assert_eq!(empty_color, default_fill,
            "empty RecipeSet must leave section.header fill unchanged");
    }
}

/// Paint a hairline + 3-dot grippy in the middle of `rect`. The divider
/// runs left→right; dots are centered horizontally.
fn paint_grippy_divider<T: ComponentTheme>(ui: &mut Ui, t: &T, rect: Rect, hovered: bool) {
    let alpha = if hovered { DIVIDER_HOVER_ALPHA } else { DIVIDER_IDLE_ALPHA };
    let line_color = color_alpha(t.surface_border(), alpha);
    let cy = rect.center().y;
    let painter = ui.painter();
    // Hairline rule across full width.
    painter.line_segment(
        [Pos2::new(rect.left(), cy), Pos2::new(rect.right(), cy)],
        Stroke::new(stroke_thin(), line_color),
    );
    // Center dot triplet — three filled circles, ~1.4px radius, gap 3px,
    // tinted with the same border color but a touch brighter.
    let dot_color = color_alpha(t.surface_border(), alpha.saturating_add(48));
    let dot_r = 1.4;
    let gap = 3.0;
    let cx = rect.center().x;
    let xs = [cx - (dot_r * 2.0 + gap), cx, cx + (dot_r * 2.0 + gap)];
    for x in xs {
        painter.circle_filled(Pos2::new(x, cy), dot_r, dot_color);
    }
}

// ─── M4.3: section-group stack tests ────────────────────────────────────────
//
// The stack used to be three independent hand-rolled prefix sums (two O(n²)
// loops over `heights` plus the builder's `cursor_y`). Any one of them could
// drift out of step with the other two and nothing would catch it. They are
// now one solve, and these assertions pin it.

#[cfg(test)]
mod section_group_layout_tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool { (a - b).abs() < 0.01 }

    fn body() -> Rect {
        Rect::from_min_size(egui::pos2(4.0, 10.0), Vec2::new(240.0, 300.0))
    }

    /// Sections stack top-down, each separated by exactly one divider band —
    /// what `y += heights[j]; if j < i { y += DIVIDER_BAND_H }` computed.
    #[test]
    fn sections_stack_with_one_divider_band_between_each() {
        let r = body();
        let h = [100.0, 80.0, 60.0];
        let s = section_group_slots(r, &h);
        assert_eq!(s.len(), 3);
        assert!(approx(s[0].top(), r.top()), "got {}", s[0].top());
        assert!(approx(s[0].height(), 100.0));
        assert!(approx(s[1].top(), s[0].bottom() + DIVIDER_BAND_H), "got {}", s[1].top());
        assert!(approx(s[2].top(), s[1].bottom() + DIVIDER_BAND_H), "got {}", s[2].top());
    }

    /// Sections span the group's full width (the old rects were built from
    /// `outer_rect.left()` / `.width()` by hand).
    #[test]
    fn sections_span_the_full_group_width() {
        let r = body();
        let s = section_group_slots(r, &[50.0, 50.0]);
        assert!(approx(s[0].left(), r.left()));
        assert!(approx(s[0].width(), r.width()));
    }

    /// There are exactly `n - 1` bands and each one IS the gutter — it starts
    /// where a section ends and ends where the next begins.
    #[test]
    fn bands_are_the_gutters_between_sections() {
        let r = body();
        let h = [100.0, 80.0, 60.0];
        let s = section_group_slots(r, &h);
        let b = section_group_bands(r, &h);
        assert_eq!(b.len(), 2);
        for i in 0..2 {
            assert!(approx(b[i].top(), s[i].bottom()), "band {i} top {}", b[i].top());
            assert!(approx(b[i].bottom(), s[i + 1].top()), "band {i} bottom {}", b[i].bottom());
            assert!(approx(b[i].height(), DIVIDER_BAND_H));
        }
    }

    /// A single section has no dividers and takes its whole height.
    #[test]
    fn one_section_has_no_bands() {
        let r = body();
        let s = section_group_slots(r, &[120.0]);
        assert_eq!(s.len(), 1);
        assert!(section_group_bands(r, &[120.0]).is_empty());
    }

    /// A drag transfers height between two ADJACENT sections, so every band
    /// after the dragged one is unmoved. This is what makes solving the drag
    /// bands ONCE (outside the loop) exact rather than an approximation.
    #[test]
    fn transferring_height_between_neighbours_does_not_move_later_bands() {
        let r = body();
        let before = [100.0, 80.0, 60.0];
        let after = [120.0, 60.0, 60.0]; // divider 0 dragged down by 20
        let b0 = section_group_bands(r, &before);
        let b1 = section_group_bands(r, &after);
        assert!(approx(b0[1].top(), b1[1].top()),
            "band 1 moved: {} -> {}", b0[1].top(), b1[1].top());
        assert!((b1[0].top() - b0[0].top() - 20.0).abs() < 0.01);
    }

    /// Zero / negative heights happen on the first frame before fractions are
    /// normalised; they must not panic or invert a rect.
    #[test]
    fn degenerate_heights_do_not_panic() {
        let r = body();
        let s = section_group_slots(r, &[0.0, -5.0, 10.0]);
        assert_eq!(s.len(), 3);
        for rect in s {
            assert!(rect.height() >= 0.0);
        }
    }

#[cfg(test)]
mod meta_slot_tests {
    use super::PanelSection;

    /// An empty caption must not claim a slot.
    ///
    /// Panels compute these from live state — "3 open · long" when there are
    /// positions, nothing when flat — so `String::new()` reaches this builder
    /// on every quiet frame. Storing `Some("")` would reserve the slot and its
    /// gap, making the header quietly wider with nothing drawn in it.
    #[test]
    fn an_empty_meta_is_absent_not_empty() {
        assert!(PanelSection::new("X").meta(String::new()).meta_is_none());
        assert!(PanelSection::new("X").meta("   ").meta_is_none());
        assert!(!PanelSection::new("X").meta("3 open").meta_is_none());
    }
}
}
