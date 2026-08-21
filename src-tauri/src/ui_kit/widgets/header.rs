//! Header — the canonical panel / section / dialog header widget.
//!
//! ## When to use
//!
//! - **Panel headers** — side panels, dock panels, modal sections.
//! - **Dialog headers** — modal title rows.
//! - **Section labels** within a panel — sub-grouping inside a single
//!   panel body.
//!
//! Three preset surfaces cover the role:
//!
//! ```ignore
//! Header::panel("ALERTS").show(ui, theme);
//! Header::dialog("Edit Order").show(ui, theme);
//! Header::section("Trigger conditions").show(ui, theme);
//! ```
//!
//! Plus a builder for custom cases:
//!
//! ```ignore
//! Header::new("WATCHLIST", HeaderVariant::Panel)
//!     .subtitle("12 symbols")
//!     .leading_icon(Icon::LIST)
//!     .trailing(|ui, t| {
//!         if ui_kit::Button::icon(Icon::PLUS).show(ui, t).clicked() { ... }
//!     })
//!     .show(ui, theme);
//! ```
//!
//! ## When NOT to use
//!
//! - For pane chrome (chart pane headers with symbol + tabs), use the
//!   in-chart `PaneHeader` in `chart/renderer/ui/chrome/painter_pane.rs`
//!   — it has GPU-aligned layout requirements this widget doesn't share.
//! - For toolbar segmented controls or top-nav, those have their own
//!   primitives in `chart/renderer/ui/components/toolbar/`.
//!
//! ## Migration target
//!
//! Replaces these legacy entry points:
//! - `chart/renderer/ui/components/headers.rs::panel_header()`
//! - `chart/renderer/ui/components/headers_widget.rs::PanelHeaderWithTabs`
//!   (keep this one for now — tabs are out of scope)
//! - `chart/renderer/ui/style.rs::dialog_header()`,
//!   `dialog_header_colored()`, `section_label()`
//! - The panel-internal `chart/renderer/ui/panels/kit.rs::PanelHeader`
//!   which has a hard `Watchlist` coupling. Use this widget for new
//!   panels that don't need that coupling.

use egui::{Align, CornerRadius, Layout, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};
use crate::ui_kit::tokens as st;
use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::Tooltip;
use super::icon_placement::IconPlacement;
use crate::ui_kit::text_style::TextStyle;

// ─── Header strip: flex spec (M4.3) ─────────────────────────────────────────
//
// The strip used to be a cursor walk:
//
//     let mut left_cursor = rect.left() + pad_x;
//     let right_edge      = rect.right() - pad_x;
//     …
//     left_cursor += g.size().x + st::gap_sm();   // after the icon
//     left_cursor += g.size().x + st::gap_md();   // after the title
//     … Rect::from_min_max(pos2(left_cursor, top), pos2(right_edge - close_w, bottom))
//
// Every `+=` there is a place alignment can drift, and the two different
// seam tokens (`gap_sm` after the icon, `gap_md` after the title) are why the
// row could not be expressed as a uniform-gutter `ui.horizontal`. Flexbox does
// both: one `grow` item replaces `right_edge - close_w`, and `margin_start`
// gives each seam its own token.
//
// GEOMETRY ONLY — fonts, colours, paint order and the close-button offsets
// below are untouched.

/// Intrinsic widths of the header's measured pieces. Split out so the strip is
/// solvable (and assertable) from plain numbers, with no egui context.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeaderMetrics {
    /// Leading-icon galley width; `None` when there is no icon.
    pub icon_w: Option<f32>,
    /// Title galley width.
    pub title_w: f32,
    /// Close-button slot width. `0.0` when the header is not closable.
    pub close_w: f32,
}

/// Solved slots of a header strip. Absolute rects.
#[derive(Clone, Copy, Debug)]
pub struct HeaderSlots {
    /// Leading icon. Paint `LEFT_CENTER` at `icon.left()`.
    pub icon: Option<Rect>,
    /// Title. Paint `LEFT_CENTER` at `title.left()`.
    pub title: Rect,
    /// The elastic middle — subtitle origin, and the rect handed to the
    /// `leading` / `trailing` callbacks. Full row height.
    pub rest: Rect,
    /// Close-button slot, flush against the right padding edge.
    pub close: Option<Rect>,
}

/// `icon · title …… close`, `pad_x` inset on both ends.
///
/// The icon is pinned `shrink(0.0)` — a glyph has no smaller form. **The title
/// shrinks**, which is the follow-up this comment used to defer.
///
/// It was pinned during the cursor-walk migration so a long title overflowed
/// exactly as the `+=` walk did, keeping that change plumbing-only. The cost of
/// keeping it is real and visible: at a 360 px side panel the design inspector's
/// own title painted straight through the RESET button beside it. A cursor walk
/// does not yield, it overflows — and overflow in a row whose right end holds
/// *buttons* is not a cosmetic overrun, it is an unreadable collision over a
/// live click target.
///
/// Shrinking is the flexbox answer and it is only half the fix: the title is
/// also painted truncated to its solved slot (see `show`), because a narrowed
/// rect does not narrow glyphs.
fn header_row_flex(m: &HeaderMetrics, pad_x: f32) -> Flex {
    let mut f = Flex::row()
        .padding_sides(pad_x, pad_x, 0.0, 0.0)
        .align(FlexAlign::Center);
    if let Some(w) = m.icon_w {
        f = f.item(Item::content(w).shrink(0.0));
    }
    f = f.item(
        Item::content(m.title_w)
            // Shrinks before the elastic middle collapses, so the close slot
            // and any trailing actions keep their width at every panel size.
            .shrink(1.0)
            // was `left_cursor += icon_w + st::gap_sm()`
            .margin_start(if m.icon_w.is_some() { st::gap_sm() } else { 0.0 }),
    );
    // The elastic middle. This ONE item replaces both `left_cursor` (its left
    // edge) and `right_edge - close_w` (its right edge).
    f = f.item(
        Item::grow(1.0)
            // was `left_cursor += title_w + st::gap_md()`
            .margin_start(st::gap_md())
            .align_self(FlexAlign::Stretch),
    );
    if m.close_w > 0.0 {
        f = f.item(Item::fixed(m.close_w).align_self(FlexAlign::Stretch));
    }
    f
}

/// Solve [`header_row_flex`] inside `rect`.
pub fn solve_header_row(rect: Rect, m: &HeaderMetrics, pad_x: f32) -> HeaderSlots {
    let solved = header_row_flex(m, pad_x).solve(rect.size());
    let off = rect.min.to_vec2();
    let mut it = solved.into_iter().map(|r| r.translate(off));
    let icon = if m.icon_w.is_some() { it.next() } else { None };
    let title = it.next().unwrap_or(Rect::NOTHING);
    let rest = it.next().unwrap_or(Rect::NOTHING);
    let close = if m.close_w > 0.0 { it.next() } else { None };
    HeaderSlots { icon, title, rest, close }
}

/// Which surface this header sits on. Drives height, font tier, padding,
/// and whether a bottom rule is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderVariant {
    /// Side / dock panel title row. 28px high, uppercase 9px caps, faint
    /// bottom rule.
    Panel,
    /// Modal dialog title row. 36px high, mixed-case 13px, no bottom rule
    /// (modal frame draws its own).
    Dialog,
    /// Sub-section inside a panel body. No background, 9px caps, generous
    /// top padding.
    Section,
}

impl HeaderVariant {
    fn height(self) -> f32 {
        match self {
            HeaderVariant::Panel => 28.0,
            HeaderVariant::Dialog => 36.0,
            HeaderVariant::Section => 22.0,
        }
    }
    fn font_size(self) -> f32 {
        match self {
            HeaderVariant::Panel | HeaderVariant::Section => st::font_xs(),
            HeaderVariant::Dialog => st::font_md(),
        }
    }
    fn upper(self) -> bool {
        matches!(self, HeaderVariant::Panel | HeaderVariant::Section)
    }
    fn bottom_rule(self) -> bool {
        matches!(self, HeaderVariant::Panel)
    }
}

/// Output of a `Header::show(...)` call.
pub struct HeaderResponse {
    /// The full header rect.
    pub rect: egui::Rect,
    /// The header row's response (useful for tooltip/menu anchoring).
    pub response: Option<Response>,
    /// True if the close button was clicked. Only fires when
    /// `.closable(true)` was set.
    pub close_clicked: bool,
}

type Trailing<'a> = Option<Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>>;
type Leading<'a>  = Option<Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>>;

#[must_use = "Header must be rendered with `.show(ui, theme)`"]
pub struct Header<'a> {
    title: &'a str,
    subtitle: Option<&'a str>,
    variant: HeaderVariant,
    leading_icon: Option<&'a str>,
    closable: bool,
    leading: Leading<'a>,
    trailing: Trailing<'a>,
}

impl<'a> Header<'a> {
    /// Build a header explicitly. Most callers should prefer the named
    /// presets (`Header::panel(...)`, `Header::dialog(...)`,
    /// `Header::section(...)`).
    pub fn new(title: &'a str, variant: HeaderVariant) -> Self {
        Self {
            title,
            subtitle: None,
            variant,
            leading_icon: None,
            closable: false,
            leading: None,
            trailing: None,
        }
    }

    /// Panel header preset.
    pub fn panel(title: &'a str) -> Self { Self::new(title, HeaderVariant::Panel) }

    /// Dialog header preset.
    pub fn dialog(title: &'a str) -> Self { Self::new(title, HeaderVariant::Dialog) }

    /// Section label preset.
    pub fn section(title: &'a str) -> Self { Self::new(title, HeaderVariant::Section) }

    /// Add a muted subtitle line below the title.
    pub fn subtitle(mut self, s: &'a str) -> Self {
        self.subtitle = Some(s);
        self
    }

    /// Phosphor icon glyph rendered before the title.
    pub fn leading_icon(mut self, icon: &'a str) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// Add a close (×) button at the trailing edge. Click state surfaces
    /// via `HeaderResponse.close_clicked`.
    pub fn closable(mut self, on: bool) -> Self {
        self.closable = on;
        self
    }

    /// Caller-drawn content immediately AFTER the title (LTR flow). Useful
    /// for status pills, counts, "(12 active)" suffixes.
    pub fn leading<F>(mut self, f: F) -> Self
    where F: FnOnce(&mut Ui, &dyn ComponentTheme) + 'a {
        self.leading = Some(Box::new(f));
        self
    }

    /// Caller-drawn content at the trailing edge, to the LEFT of the
    /// close button. Action buttons, menu triggers, etc.
    pub fn trailing<F>(mut self, f: F) -> Self
    where F: FnOnce(&mut Ui, &dyn ComponentTheme) + 'a {
        self.trailing = Some(Box::new(f));
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> HeaderResponse {
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> HeaderResponse {
        let theme = sctx.theme();
        let h = self.variant.height();
        let font_size = self.variant.font_size();
        let pad_x = st::gap_md();

        let avail_w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::hover());

        let painter = ui.painter_at(rect);
        let mut out = HeaderResponse { rect, response: Some(resp), close_clicked: false };

        // ── Measure, then solve the whole strip in one pass ──
        let icon_font = crate::ui_kit::style::prop_at(font_size + 2.0);
        let title_text = if self.variant.upper() {
            self.title.to_uppercase()
        } else {
            self.title.to_string()
        };
        let title_font = crate::ui_kit::style::mono_at(font_size);
        let title_color = palette_ct(theme).base(Tone::Text);

        let metrics = HeaderMetrics {
            icon_w: self.leading_icon.map(|icon| {
                painter
                    .layout_no_wrap(icon.to_string(), icon_font.clone(), title_color)
                    .size()
                    .x
            }),
            title_w: painter
                .layout_no_wrap(title_text.clone(), title_font.clone(), title_color)
                .size()
                .x,
            close_w: if self.closable { 22.0 } else { 0.0 },
        };
        let slots = solve_header_row(rect, &metrics, pad_x);

        // ── Leading icon ──
        if let (Some(icon), Some(slot)) = (self.leading_icon, slots.icon) {
            painter.text(
                egui::pos2(slot.left(), slot.center().y),
                egui::Align2::LEFT_CENTER,
                icon,
                icon_font,
                title_color,
            );
        }

        // ── Title (uppercase for Panel/Section, mixed-case for Dialog) ──
        //
        // Truncated to the slot the flex solved. Shrinking the rect alone does
        // not shrink glyphs — `painter.text` would happily paint the full
        // string past the slot's right edge and back onto the trailing
        // buttons, which is the collision this pair of changes removes.
        {
            let mut job = egui::text::LayoutJob::single_section(
                title_text.clone(),
                egui::TextFormat::simple(title_font, title_color),
            );
            job.wrap = egui::text::TextWrapping {
                max_width: slots.title.width().max(0.0),
                max_rows: 1,
                break_anywhere: true,
                overflow_character: Some('…'),
            };
            let galley = painter.layout_job(job);
            painter.galley(
                egui::pos2(
                    slots.title.left(),
                    slots.title.center().y - galley.size().y * 0.5,
                ),
                galley,
                title_color,
            );
        }

        // ── Subtitle (muted, right after title) ──
        if let Some(sub) = self.subtitle {
            let sub_font = TextStyle::MonoXs.font_id_in(ui);
            // Truncate the subtitle the way the TITLE above already does.
            //
            // The title builds a `LayoutJob` with `max_width`, `max_rows: 1`
            // and `overflow_character: Some('…')`. The subtitle, two statements
            // later, was a bare `painter.text(..)` with no bound — so a long
            // one was hard-clipped by the header's painter and cut mid-glyph,
            // with no ellipsis, while the title beside it ended in a tidy `…`.
            //
            // Not an overflow: the clip meant nothing escaped the header. Just
            // two pieces of text in one widget treated differently, where the
            // author had plainly already solved it once.
            let sub_room = (slots.rest.width()
                - st::gap_sm().min(slots.rest.width())).max(0.0);
            let sub_shown = st::ellipsize_to(
                &painter, sub, &sub_font,
                sub_room,
                st::color_muted(palette_ct(theme).base(Tone::Dim)),
            );
            painter.text(
                egui::pos2(slots.rest.left(), rect.center().y),
                egui::Align2::LEFT_CENTER,
                sub_shown,
                sub_font,
                st::color_muted(palette_ct(theme).base(Tone::Dim)),
            );
        }

        // ── Trailing: caller content, then close button (RTL) ──
        if let Some(slot) = slots.close {
            let close_rect = egui::Rect::from_min_size(
                egui::pos2(slot.left() + crate::ui_kit::style::gap_2xs(), rect.top() + (h - 18.0) / 2.0),
                Vec2::new(18.0, 18.0),
            );
            let close_resp = super::button::Button::close()
                .placement(IconPlacement::PanelHeader)
                .show_at(ui, &painter, close_rect, theme);
            Tooltip::new("Close").show(ui, &close_resp, theme);
            if close_resp.clicked() {
                out.close_clicked = true;
            }
        }

        if let Some(f) = self.trailing {
            // Hand the caller a child UI over the elastic middle, laid out RTL
            // so they can `add` from the close button leftward.
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(slots.rest)
                    .layout(Layout::right_to_left(Align::Center)),
            );
            f(&mut child, theme);
        }

        if let Some(f) = self.leading {
            // Re-allocate the same slot left-to-right.
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(slots.rest)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            f(&mut child, theme);
        }

        // ── Bottom rule (Panel only) ──
        if self.variant.bottom_rule() {
            let y = rect.bottom() - 0.5;
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                Stroke::new(st::stroke_thin(), st::color_alpha(palette_ct(theme).base(Tone::Border), st::alpha_muted())),
            );
        }

        let _ = CornerRadius::default(); // silence unused import if shape paths change later
        let _ = StrokeKind::Inside;
        out
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────
//
// `solve_header_row` is pure geometry, so the strip alignment that previously
// could only be eyeballed in a running window is asserted headlessly.

#[cfg(test)]
mod header_layout_tests {
    use super::*;
    use crate::ui_kit::widgets::paint_probe;

    /// The title truncates properly — a `LayoutJob` with
    /// `max_width: slots.title.width()`, `max_rows: 1` and an overflow
    /// character. The SUBTITLE, two statements below, is a bare
    /// `painter.text(pos2(slots.rest.left(), ..), LEFT_CENTER, ..)` with no
    /// bound at all, growing right through the slot that also holds the
    /// trailing actions.
    ///
    /// One widget, two pieces of text, one of them handled. That asymmetry is
    /// the finding: the author plainly knew the problem — the title is the
    /// evidence — and the subtitle was written without it.
    #[test]
    fn the_subtitle_stays_inside_the_header() {
        for width in [420.0f32, 260.0, 170.0] {
            for (title, sub) in [
                ("Positions", Some("3 open")),
                ("Positions", Some("3 open - 2 long - 1 short - $12,345.67 unrealised")),
                ("A rather long panel title that eats the row", Some("and a subtitle after it")),
                ("Positions", Some("3 open - 2 long - 1 short - $12,345.67 unrealised - 4 working orders - last sync 12:04:33")),
            ] {
                let right_edge = std::cell::Cell::new(0.0f32);
                let runs = paint_probe::probe(|ui| {
                    let t = crate::ui_kit::widgets::theme::PortableTheme::dark();
                    let rect = egui::Rect::from_min_size(
                        ui.max_rect().min, egui::vec2(width, 44.0));
                    right_edge.set(rect.right());
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    let mut h = Header::new(title, HeaderVariant::Panel);
                    if let Some(sb) = sub { h = h.subtitle(sb); }
                    h.show(&mut child, &t);
                });
                if runs.is_empty() {
                    continue;
                }
                let edge = right_edge.get();
                let name = format!("header w={width} {title:?} / {sub:?}");
                paint_probe::assert_no_overlap(&name, &runs);
                // The assertion that actually distinguishes the fix.
                //
                // The subtitle never escaped the header — it was hard-clipped
                // by the painter. So `assert_no_overlap` and the right-edge
                // check both passed while it was being cut mid-glyph. Only
                // `clipped` tells truncation-with-an-ellipsis apart from
                // truncation-by-guillotine.
                paint_probe::assert_not_clipped(&name, &runs);
                for r in &runs {
                    assert!(
                        r.right <= edge + 0.5,
                        "{name}: text reaches {} past the header's right edge {edge}; runs={runs:?}",
                        r.right
                    );
                }
            }
        }
    }


    fn approx(a: f32, b: f32) -> bool { (a - b).abs() < 0.01 }

    fn strip() -> Rect {
        Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(300.0, 28.0))
    }

    /// The bare shape: title flush against the left padding edge, elastic
    /// middle running to the right padding edge.
    #[test]
    fn title_starts_at_the_padding_edge() {
        let r = strip();
        let pad = st::gap_md();
        let s = solve_header_row(r, &HeaderMetrics { title_w: 60.0, ..Default::default() }, pad);
        assert!(s.icon.is_none());
        assert!(approx(s.title.left(), r.left() + pad), "got {}", s.title.left());
        assert!(approx(s.title.width(), 60.0), "got {}", s.title.width());
        assert!(approx(s.rest.right(), r.right() - pad), "got {}", s.rest.right());
        assert!(s.close.is_none());
    }

    /// Two different seam tokens in one row — the thing a uniform-gutter
    /// `ui.horizontal` could not express: icon→title is `gap_sm`,
    /// title→middle is `gap_md`.
    #[test]
    fn the_two_seams_use_their_own_tokens() {
        let r = strip();
        let pad = st::gap_md();
        let m = HeaderMetrics { icon_w: Some(14.0), title_w: 60.0, close_w: 0.0 };
        let s = solve_header_row(r, &m, pad);
        let icon = s.icon.expect("icon slot");
        assert!(approx(icon.left(), r.left() + pad), "got {}", icon.left());
        assert!(approx(s.title.left() - icon.right(), st::gap_sm()),
            "icon→title seam = {}", s.title.left() - icon.right());
        assert!(approx(s.rest.left() - s.title.right(), st::gap_md()),
            "title→middle seam = {}", s.rest.left() - s.title.right());
    }

    /// The close button is flush against the right padding edge and the
    /// elastic middle stops exactly at its left edge — what
    /// `right_edge - close_w` used to compute by hand.
    #[test]
    fn close_is_flush_right_and_the_middle_stops_at_it() {
        let r = strip();
        let pad = st::gap_md();
        let m = HeaderMetrics { icon_w: None, title_w: 60.0, close_w: 22.0 };
        let s = solve_header_row(r, &m, pad);
        let close = s.close.expect("close slot");
        assert!(approx(close.right(), r.right() - pad), "got {}", close.right());
        assert!(approx(close.width(), 22.0));
        assert!(approx(s.rest.right(), close.left()), "got {} vs {}", s.rest.right(), close.left());
    }

    /// The elastic middle spans the full row height — the callback rect used
    /// to be built explicitly from `rect.top()`/`rect.bottom()`.
    #[test]
    fn the_middle_spans_the_full_row_height() {
        let r = strip();
        let s = solve_header_row(r, &HeaderMetrics { title_w: 40.0, ..Default::default() }, st::gap_md());
        assert!(approx(s.rest.height(), r.height()), "got {}", s.rest.height());
        assert!(approx(s.rest.top(), r.top()));
    }

    /// An oversized title yields instead of painting over what follows it.
    ///
    /// This asserts the OPPOSITE of what it used to. The pinned-overflow
    /// behaviour was migration fidelity — the cursor walk it replaced
    /// overflowed, so the flex version overflowed identically and the change
    /// stayed plumbing-only. Kept past that point it is a bug: at a 360 px
    /// side panel the design inspector's title painted through the RESET
    /// button next to it, so the overrun landed on a live click target.
    #[test]
    fn an_oversized_title_shrinks_instead_of_overrunning_the_row() {
        let r = strip();
        let m = HeaderMetrics { icon_w: None, title_w: 500.0, close_w: 22.0 };
        let s = solve_header_row(r, &m, st::gap_md());

        assert!(
            s.title.width() < 500.0,
            "title must yield when it cannot fit; got {}",
            s.title.width()
        );
        assert!(
            s.title.right() <= r.right() + 0.5,
            "title ran past the row: {} vs {}",
            s.title.right(),
            r.right()
        );
        // The point of shrinking: the close slot keeps its full width and
        // stays inside the row, so nothing is painted over it.
        let close = s.close.expect("close slot requested");
        assert!(approx(close.width(), 22.0), "close shrank: {}", close.width());
        assert!(
            s.title.right() <= close.left() + 0.5,
            "title overlaps the close button: title ends {}, close starts {}",
            s.title.right(),
            close.left()
        );
    }

    /// Shrinking must not invert the rect when there is genuinely no room —
    /// the truncating layout job takes `max_width` from this and a negative
    /// width would be a panic in `TextWrapping`.
    #[test]
    fn a_title_squeezed_to_nothing_stays_non_negative() {
        let r = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(30.0, 28.0));
        let m = HeaderMetrics { icon_w: Some(14.0), title_w: 500.0, close_w: 22.0 };
        let s = solve_header_row(r, &m, st::gap_md());
        assert!(s.title.width() >= 0.0, "negative title width: {}", s.title.width());
    }

    /// Degenerate widths happen while a panel is being dragged; they must not
    /// panic or invert a rect.
    #[test]
    fn degenerate_width_does_not_panic() {
        let r = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(0.0, 28.0));
        let s = solve_header_row(r, &HeaderMetrics { title_w: 40.0, ..Default::default() }, st::gap_md());
        assert!(s.rest.width() >= 0.0);
    }
}
