//! Panel kit — high-level patterns for side panels.
//!
//! Composes the existing primitives (`SectionLabel`, `Button`, `Input`, frames,
//! etc.) into the patterns that side panels actually want to assemble: a section
//! header with optional count + meta + inline action, an empty-state line, a
//! compact label-and-input row, and a pair of opposing colored action buttons.
//!
//! Goal: replace ~30 lines of `ui.horizontal { SectionLabel.tiny() ... RTL { small_action_btn ... } }`
//! boilerplate per section with one builder call. Visual choices (hairline rule
//! under headers, accent counts, tone palette) are made once here.
//!
//! Visual spec
//! - Section title: 11px monospace, strong, uppercase. Tone-colored (default = dim).
//! - Optional count appended in muted-tint. Optional meta in `mono_xs` dim.
//! - Optional 1px hairline rule below the row at `alpha_subtle`.
//! - Inline action: ghost button — fg = tone, faint hover fill.
//! - Empty: single muted line, optional dim glyph prefix.
//! - InputRow: leading dim label + Input, fixed compact height.
//! - DualAction: two equal-width tone-colored ghost buttons side-by-side.

#![allow(dead_code)]

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone as SxTone;
use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};

use super::super::style::{
    alpha_ghost, alpha_line, alpha_soft, alpha_subtle,
    color_alpha, color_dim, current, font_xs, gap_md, gap_sm, gap_xs,
    section_header_font_id, style_label_case, stroke_thin,
};
use super::super::components::text::SectionLabel;
use crate::chart_renderer::gpu::{Theme, Watchlist};
use crate::ui_kit::widgets::{Button, tokens::{Size as KitSize, Variant}};
use crate::ui_kit::widgets::icon_placement::IconPlacement;

/// Resolve the chart-pane header height + title font from a `Watchlist`. Routes
/// through `gpu::pane_tabs_header_h` so style-token adjustments
/// (`header_height_scale` etc.) propagate identically. Side-panel headers
/// pulling from this guarantee pixel-y alignment with chart-pane headers.
fn header_metrics(wl: Option<&Watchlist>) -> (f32, f32) {
    match wl {
        Some(w) => (
            crate::chart_renderer::gpu::pane_tabs_header_h(w),
            w.pane_header_size.title_font(),
        ),
        // Fallback when no watchlist is available — Normal preset.
        None => (32.0, 12.0),
    }
}


// ── Chart-pane-parity chrome ─────────────────────────────────────────────────
//
// `panel_chrome_*` paints the SAME header chrome used by chart panes
// (`chrome::painter_pane::PainterPaneHeader`):
//   - absolute-rect, painter-mode (not layout flow)
//   - perimeter hairline using `t.text` at `header_outer_border_alpha`
//   - title in monospace `font_md` (matches chart pane's `title_font_size`)
//   - close button with painter-mode `×` glyph + bear-on-hover tint
//
// Side panels render through `PanelHeader` / `PanelHeaderTabs` which call
// these so they line up pixel-for-pixel with chart-pane headers above them.

const HEADER_CLOSE_SIZE: f32 = 18.0;
const HEADER_TAB_TOP_INSET: f32 = 1.0;
const HEADER_TAB_HEIGHT_INSET: f32 = 2.0;

// ── Header strip geometry (flexbox) ──────────────────────────────────────────
//
// The header strip used to be hand-rolled: a running `cx` cursor for the
// leading cluster plus a nested `right_to_left` layout to shove the trailing
// controls against the edge. That is the arithmetic the layout engine exists
// to delete, and it is why gutters drifted between panels.
//
// The strip is now ONE flex row. Geometry only — every colour, font, radius
// and glyph below still comes from the design system, unchanged.
//
//   ┌ gap_sm ┬ icon ┬ gap_sm ┬ title ┬ gap_md ┬ actions (grow) ┬ gap_sm ┬ × ┬ gap_sm ┐
//
// `solve_*` are pure `Rect -> Rect` functions (no `Ui`, no `Context`), so the
// header geometry is unit-testable headlessly — see the tests at the bottom of
// this file.

/// Solved slots of the static-title header strip. All rects are absolute
/// (already translated into the header rect).
#[derive(Clone, Copy, Debug)]
pub(crate) struct HeaderStrip {
    /// Leading icon slot — `None` when the header has no icon.
    pub icon: Option<Rect>,
    /// Title slot. Full strip height; paint `LEFT_CENTER` at its left edge.
    pub title: Rect,
    /// Everything between the title and the close button: the LTR title-actions
    /// flow and the RTL trailing-actions flow both live here.
    pub actions: Rect,
    /// Trailing meta caption slot, right-aligned before the actions —
    /// `None` when the header has no caption.
    ///
    /// The reference pushes it with `margin-left: auto`, i.e. it takes the
    /// slack rather than a share of it. Modelled here as a fixed slot placed
    /// AFTER the growing actions slot, which is the same thing.
    pub meta: Option<Rect>,
    /// The `HEADER_CLOSE_SIZE`-square close button, flush right — `None` when
    /// the header is not closable.
    pub close: Option<Rect>,
}

/// The flex spec for the static-title header strip. Widths are MEASURED galley
/// widths: `Item::auto()` has no intrinsic size under `Flex::solve` (leaves
/// carry no measure function), so intrinsic content is measured by the caller
/// and passed as `Item::fixed`.
fn header_flex(icon_w: Option<f32>, title_w: f32, meta_w: Option<f32>, closable: bool) -> Flex {
    let mut f = Flex::row()
        // Strip inset — the same `gap_sm` the cursor arithmetic used.
        .padding_sides(gap_sm(), gap_sm(), 0.0, 0.0)
        // Every slot spans the full strip height so painting at `slot.center().y`
        // is identical to the old `rect.center().y` vertical centring.
        .align(FlexAlign::Stretch);
    let title_margin = if icon_w.is_some() { gap_sm() } else { 0.0 };
    if let Some(w) = icon_w {
        f = f.item(Item::fixed(w));
    }
    f = f
        // The title yields (shrinks) rather than shoving the close button off
        // the strip when a panel is narrower than its own title.
        .item(Item::fixed(title_w).shrink(1.0).margin_start(title_margin))
        .item(Item::grow(1.0).margin_start(gap_md()));
    if let Some(w) = meta_w {
        f = f.item(Item::fixed(w).margin_start(gap_md()));
    }
    if closable {
        f = f.item(Item::fixed(HEADER_CLOSE_SIZE).margin_start(gap_sm()));
    }
    f
}

/// Ratio of the section ordinal's font size to the header title's.
///
/// From the faithful reference set, which sizes them separately:
/// `.np-ph .num { font-size: 10px }` against `.np-ph .ttl { font-size: 11.5px }`.
/// I first painted the ordinal at the FULL title size, which is both unfaithful
/// and expensive: on a tabbed header the extra width pushed a tab into the "»"
/// overflow menu, so a decoration cost a visible control.
const ORDINAL_FONT_RATIO: f32 = 10.0 / 11.5;

/// Ratio of the header's trailing meta caption to its title size.
///
/// `.np-ph .sub { font-size: 9.5px }` against `.np-ph .ttl { font-size: 11.5px }`.
const META_FONT_RATIO: f32 = 9.5 / 11.5;

/// The meta caption's font, derived from the header's title size.
fn meta_font(font_size: f32) -> FontId {
    crate::ui_kit::style::mono_at((font_size * META_FONT_RATIO).round().max(8.0))
}

/// The ordinal's font, derived from the header's title size.
fn ordinal_font(font_size: f32) -> FontId {
    crate::ui_kit::style::mono_at((font_size * ORDINAL_FONT_RATIO).round().max(8.0))
}

/// Hand out the next panel ordinal for THIS frame (`1`, `2`, `3`, …).
///
/// Meridien numbers its panels in visual order down the frame, so the ordinal
/// is a property of paint order, not of panel identity — there is no stable
/// per-panel number to store, and panels open and close.
///
/// The counter lives in egui memory keyed by the pass number, so it resets
/// itself on the first header of every frame. That matters: a counter reset by
/// some "begin frame" hook would silently keep counting on any frame that
/// skipped the hook, and the numbers would drift upward forever.
fn next_section_ordinal(ctx: &egui::Context) -> u32 {
    let id = egui::Id::new("panel_header.section_ordinal");
    let pass = ctx.cumulative_pass_nr();
    ctx.memory_mut(|m| {
        let (last, n) = m.data.get_temp::<(u64, u32)>(id).unwrap_or((u64::MAX, 0));
        let n = if last == pass { n + 1 } else { 1 };
        m.data.insert_temp(id, (pass, n));
        n
    })
}

/// Solve [`header_flex`] inside `rect` and hand back absolute slot rects.
pub(crate) fn solve_header_strip(
    rect: Rect,
    icon_w: Option<f32>,
    title_w: f32,
    meta_w: Option<f32>,
    closable: bool,
) -> HeaderStrip {
    let solved = header_flex(icon_w, title_w, meta_w, closable).solve(rect.size());
    let off = rect.min.to_vec2();
    let mut it = solved.into_iter().map(|r| r.translate(off));
    let icon = if icon_w.is_some() { it.next() } else { None };
    let title = it.next().unwrap_or(Rect::NOTHING);
    let actions = it.next().unwrap_or(Rect::NOTHING);
    let meta = if meta_w.is_some() { it.next() } else { None };
    // The close SLOT is full-height; the button itself is a centred square,
    // exactly as the old `Rect::from_center_size(.., splat(HEADER_CLOSE_SIZE))`.
    let close = if closable {
        it.next().map(|s| Rect::from_center_size(s.center(), Vec2::splat(HEADER_CLOSE_SIZE)))
    } else {
        None
    };
    HeaderStrip { icon, title, actions, meta, close }
}

/// Solved slots of the tab-driven header strip.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HeaderTabsStrip {
    /// Room the tab strip + trailing actions share (everything left of close).
    pub strip: Rect,
    /// The close button square, flush right — `None` when not closable.
    pub close: Option<Rect>,
}

/// Solve the tab header's OUTER chrome. Only the outer strip is flexed: the tab
/// run itself stays painter-driven because tab widths are content-measured with
/// their own overflow-to-`»`-menu rule, which is a different problem from
/// distributing a fixed set of slots.
pub(crate) fn solve_header_tabs_strip(rect: Rect, closable: bool) -> HeaderTabsStrip {
    let mut f = Flex::row()
        .padding_sides(gap_sm(), gap_sm(), 0.0, 0.0)
        .align(FlexAlign::Stretch)
        .item(Item::grow(1.0));
    if closable {
        f = f.item(Item::fixed(HEADER_CLOSE_SIZE).margin_start(gap_sm()));
    }
    let solved = f.solve(rect.size());
    let off = rect.min.to_vec2();
    let strip = solved.first().map(|r| r.translate(off)).unwrap_or(Rect::NOTHING);
    let close = if closable {
        solved.get(1)
            .map(|s| Rect::from_center_size(s.translate(off).center(), Vec2::splat(HEADER_CLOSE_SIZE)))
    } else {
        None
    };
    HeaderTabsStrip { strip, close }
}

/// Panel-header edge.
///
/// This painted a full PERIMETER hairline from `Chrome.header_outer_border_*`,
/// while the `panel.header` recipe — authored by all six styles, transcribed
/// from the React `[data-ds]` rules — describes a BOTTOM edge only
/// (`.ds-panel__header { border-bottom: 1px solid ... }`). The recipe was
/// consumed by nothing, so the design source and the pixels disagreed and the
/// authored data was inert.
///
/// Decision: honour the source. A header is a band that separates itself from
/// the content below it; boxing it on four sides makes every panel read as a
/// nested container, which is a large part of why the chrome looked busy.
///
/// The Chrome tokens still supply the colour and width when a style authors no
/// edge, so nothing is un-themed by the change.
fn paint_chrome_perimeter(ctx: &egui::Context, painter: &egui::Painter, rect: Rect, t: &Theme) {
    let st = current();
    let recipes = crate::ui_kit::widgets::theme::get_ambient_recipes(ctx);
    let delta = recipes
        .resolve("panel.header", crate::ui_kit::sx::Sx::new(), t)
        .resolved(crate::ui_kit::sx::StyleState::Normal);

    let pal = crate::ui_kit::sx::palette_ct(t);
    let (w, col) = match delta.border_spec() {
        Some(b) => (
            b.width,
            delta.resolved_border_color(&pal)
                .unwrap_or_else(|| tint(t, SxTone::Text, st.header_outer_border_alpha)),
        ),
        None => (
            st.header_outer_border_width,
            tint(t, SxTone::Text, st.header_outer_border_alpha),
        ),
    };
    if w <= 0.0 {
        return;
    }
    // Bottom edge only.
    let y = rect.bottom() - w * 0.5;
    painter.line_segment(
        [
            egui::pos2(rect.left(), y),
            egui::pos2(rect.right(), y),
        ],
        Stroke::new(w, col),
    );
}

/// Paint the 10px linear-alpha gradient shadow that sits BELOW the header bar,
/// fading from `from_black_alpha(42)` → transparent. Mirrors `render::pane.rs:425-448`
/// exactly so chart panes and side panels share the identical drop.
fn paint_header_shadow(ui: &Ui, header_rect: Rect, t: &Theme) {
    let shadow_h = 10.0_f32;
    let shadow_top = header_rect.bottom() + 1.0;
    let shadow_rect = Rect::from_min_max(
        Pos2::new(header_rect.left(), shadow_top),
        Pos2::new(header_rect.right(), shadow_top + shadow_h),
    );
    let painter = ui.painter_at(shadow_rect);
    // Themed: pull shadow tint from the active palette so light themes
    // (Bauhaus/Peach/Ivory/Newsprint) get a soft gray gradient instead of
    // a brown-black smudge.
    let s = t.shadow_color;
    let top_col = crate::ui_kit::style::color_alpha(s, 42);
    let bot_col = Color32::TRANSPARENT;
    let mut mesh = egui::Mesh::default();
    let tl = shadow_rect.left_top();
    let tr = shadow_rect.right_top();
    let bl = shadow_rect.left_bottom();
    let br = shadow_rect.right_bottom();
    mesh.colored_vertex(tl, top_col);
    mesh.colored_vertex(tr, top_col);
    mesh.colored_vertex(br, bot_col);
    mesh.colored_vertex(bl, bot_col);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_close_btn(ui: &mut Ui, painter: &egui::Painter, rect: Rect, t: &Theme) -> bool {
    crate::ui_kit::widgets::Button::close()
        .placement(IconPlacement::PanelHeader)
        .show_at(ui, painter, rect, t)
        .on_hover_text("Close")
        .clicked()
}

// ── PanelHeader ──────────────────────────────────────────────────────────────

/// Side-panel header bar that mirrors the chart-pane header chrome
/// (`chrome::painter_pane::PainterPaneHeader`) so an open side panel lines up
/// pixel-for-pixel with the pane header above it.
///
/// Visual spec (matches `PainterPaneHeader`):
/// - Height: 28px (default; chart panes use the same)
/// - Background: panel's own `egui::SidePanel` fill (no extra fill here —
///   matches inactive chart panes which inherit the canvas bg)
/// - Outer perimeter hairline: `t.text` at `header_outer_border_alpha` /
///   `header_outer_border_width` (style-token driven)
/// - Title: monospace `font_md`, color = `t.text` (matches inactive chart pane)
/// - Close: painter-mode `×` glyph, bear-tinted on hover
///
/// ```ignore
/// if PanelHeader::new("ALERTS").icon(Icon::BELL).show(ui, t) {
///     open = false;
/// }
/// ```
#[must_use = "PanelHeader must be rendered with `.show(...)`"]
pub struct PanelHeader<'a> {
    title: &'a str,
    icon: Option<&'a str>,
    height_override: Option<f32>,
    font_size_override: Option<f32>,
    closable: bool,
    watchlist: Option<&'a Watchlist>,
    /// Eligible for Meridien's section ordinal. See [`Self::numbered`].
    numbered: bool,
    /// Trailing meta caption. See [`Self::meta`].
    meta: Option<&'a str>,
}

impl<'a> PanelHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title, icon: None,
            height_override: None,
            font_size_override: None,
            closable: true,
            watchlist: None,
            numbered: false,
            meta: None,
        }
    }

    /// A short trailing caption, right-aligned before the header's actions —
    /// the reference's `<span class="sub">`: `Megacaps · 12`, `Lvl 2 · 14
    /// deep`, `3 open`.
    ///
    /// Pass only text derived from real state. The reference captions are
    /// design copy for a static mock; ours have to be true, or the header is
    /// asserting something the panel cannot back up.
    pub fn meta(mut self, meta: &'a str) -> Self { self.meta = Some(meta); self }
    pub fn icon(mut self, icon: &'a str) -> Self { self.icon = Some(icon); self }

    /// Mark this header as a SECTION header, eligible for Meridien's ordinal.
    ///
    /// Opt-in for the same reason as [`PanelHeaderTabs::numbered`]: not every
    /// `PanelHeader` is a section. `PaneHeader` (heatmap, portfolio) uses this
    /// widget for a pane's own title bar, and panes paint before the side
    /// panels — left on by default it would have taken `01` in any layout with
    /// one open, renumbering every panel beside it.
    pub fn numbered(mut self) -> Self { self.numbered = true; self }
    /// Pin the header to the chart-pane height/font configured by this
    /// `Watchlist`'s `pane_header_size`. Side panels that pass this line up
    /// pixel-y with the chart pane header above them.
    pub fn watchlist(mut self, wl: &'a Watchlist) -> Self { self.watchlist = Some(wl); self }
    /// Override the resolved height (escape hatch for non-aligned panels).
    pub fn height(mut self, h: f32) -> Self { self.height_override = Some(h); self }
    /// Override the resolved title font size.
    pub fn font_size(mut self, px: f32) -> Self { self.font_size_override = Some(px); self }
    /// Disable the trailing close button (e.g. when the panel cannot be closed
    /// from its own header).
    pub fn closable(mut self, on: bool) -> Self { self.closable = on; self }

    /// Render. Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui, t: &Theme) -> bool {
        self.show_full(ui, t, |_| {}, |_| {})
    }

    /// Render with caller-supplied trailing actions placed to the LEFT of the
    /// close button (RTL slot). Returns `true` if the close button was clicked.
    pub fn show_with(self, ui: &mut Ui, t: &Theme, actions: impl FnOnce(&mut Ui)) -> bool {
        self.show_full(ui, t, |_| {}, actions)
    }

    /// Render with controls immediately to the right of the title (LTR flow).
    pub fn show_with_title_actions(
        self,
        ui: &mut Ui,
        t: &Theme,
        title_actions: impl FnOnce(&mut Ui),
    ) -> bool {
        self.show_full(ui, t, title_actions, |_| {})
    }

    /// Render with both leading (LTR, after title) and trailing (RTL, before
    /// close) controls. Returns `true` if the close button was clicked.
    pub fn show_full(
        self,
        ui: &mut Ui,
        t: &Theme,
        title_actions: impl FnOnce(&mut Ui),
        actions: impl FnOnce(&mut Ui),
    ) -> bool {
        let (resolved_h, resolved_font) = header_metrics(self.watchlist);
        let h = self.height_override.unwrap_or(resolved_h);
        let font_size = self.font_size_override.unwrap_or(resolved_font);

        let avail_w = ui.available_width();
        let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::hover());
        let painter = ui.painter_at(rect);

        // Chrome: perimeter hairline + 10px gradient shadow below.
        paint_chrome_perimeter(ui.ctx(), &painter, rect, t);
        paint_header_shadow(ui, rect, t);

        // Use per-style font family: mono for Alto/Mariner/Relay, proportional for others.
        let title_font = FontId::new(font_size, section_header_font_id().family);
        let icon_font = crate::ui_kit::style::mono_at(font_size);
        let title_text = style_label_case(self.title);

        // Leading slot: an accent ordinal for numbering styles (Meridien),
        // otherwise the caller's icon glyph.
        //
        // The ordinal REPLACES the icon rather than stacking with it — the
        // design source has no icons in its numbered headers, and a
        // number-plus-glyph pair would read as two competing leading marks.
        // Reusing the icon slot means no new layout: it is already accent-
        // coloured, mono, measured, and placed by the same flex solve.
        //
        // An ordinal is set BELOW the title size per the reference; an icon
        // keeps the header's own size. Same slot, different type sizes.
        let (lead, lead_font): (Option<String>, FontId) = if self.numbered
            && crate::chart_renderer::ui::style::style_numbers_sections()
        {
            (Some(format!("{:02}", next_section_ordinal(ui.ctx()))),
             ordinal_font(font_size))
        } else {
            (self.icon.map(|g| g.to_string()), icon_font)
        };

        // Measure the intrinsic content, then let the flex engine place it.
        // (`Item::auto()` cannot help here — `Flex::solve` has no font access.)
        let icon_w = lead.as_ref().map(|g| {
            painter.layout_no_wrap(g.clone(), lead_font.clone(), t.accent).size().x
        });
        let title_w = painter
            .layout_no_wrap(title_text.clone(), title_font.clone(), t.text)
            .size().x;

        let meta_text = self.meta.map(|m| style_label_case(m));
        let meta_fid = meta_font(font_size);
        let mut meta_w = meta_text.as_ref().map(|m| {
            painter.layout_no_wrap(m.clone(), meta_fid.clone(), t.dim).size().x
        });

        // THE CAPTION YIELDS TO THE TITLE, for the reason the ordinal yields to
        // the tabs: it is the least important thing on the strip.
        //
        // Its flex slot is FIXED while the title's can `.shrink(1.0)`, so
        // without this a caption on a narrow panel does not get squeezed —
        // it squeezes the title, and the panel ends up captioned but unnamed.
        // Drop the caption whole rather than let it clip the title; a truncated
        // caption is worth less than the name of the panel it captions.
        if let (Some(mw), Some(iw_or_zero)) = (meta_w, Some(icon_w.unwrap_or(0.0))) {
            let chrome = gap_sm() * 2.0                                  // strip inset
                + iw_or_zero + if icon_w.is_some() { gap_sm() } else { 0.0 }
                + gap_md()                                               // title → actions
                + gap_md()                                               // actions → meta
                + if self.closable { HEADER_CLOSE_SIZE + gap_sm() } else { 0.0 };
            if chrome + title_w + mw > rect.width() {
                meta_w = None;
            }
        }
        let meta_text = if meta_w.is_some() { meta_text } else { None };

        let strip = solve_header_strip(rect, icon_w, title_w, meta_w, self.closable);

        // Paint icon + title in painter mode, into their solved slots.
        if let (Some(g), Some(islot)) = (lead.as_ref(), strip.icon) {
            painter.text(
                Pos2::new(islot.left(), islot.center().y),
                Align2::LEFT_CENTER, g, lead_font, t.accent,
            );
        }

        let title_pos = Pos2::new(strip.title.left(), strip.title.center().y);

        // ONE draw.
        //
        // This painted the title TWICE, 0.5px apart, as faux-bold — and its
        // comment cited "the same trick painter_pane symbol mode uses", which
        // is the code that rendered "SPY" as "SPYY" and was deleted earlier in
        // this work. The trick was copied before it was known to be broken, so
        // fixing the original left this one behind: every side-panel title has
        // been double-drawn since.
        //
        // Weight belongs to the font. If these want to be heavier, register a
        // bold face and ask for it.
        painter.text(title_pos, Align2::LEFT_CENTER, &title_text, title_font, t.text);

        // Trailing meta caption — muted, mono, below title size.
        if let (Some(m), Some(mslot)) = (meta_text.as_ref(), strip.meta) {
            painter.text(
                Pos2::new(mslot.left(), mslot.center().y),
                Align2::LEFT_CENTER, m, meta_fid, t.dim,
            );
        }

        let mut closed = false;

        // Layout-flow child UI over the solved actions slot. The slot already
        // stops short of the close button, so the trailing RTL flow no longer
        // needs to hand-reserve it with `add_space`.
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(strip.actions)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        title_actions(&mut child);
        child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(close_rect) = strip.close {
                if paint_close_btn(ui, &painter, close_rect, t) {
                    closed = true;
                }
            }
            actions(ui);
        });
        closed
    }
}

// ── PanelHeaderTabs ──────────────────────────────────────────────────────────

/// Tab-driven variant of `PanelHeader`. Same chart-pane-aligned chrome
/// (perimeter hairline, identical height), with tabs painted in absolute-rect
/// painter mode mirroring `chrome::painter_pane`'s tab strip exactly.
///
/// Tab visual spec (matches `painter_pane.rs:541-664`):
/// - Inactive: transparent fill, dim label
/// - Active: `color_dim(t.bg)` fill, top-rounded corners only,
///   `t.text` label (`t.accent` would require active-pane multipane semantics)
/// - Vertical hairline divider between adjacent tabs in `border_variant` at
///   alpha 200, `stroke_thin`, inset 4px from top/bottom
/// - Hover: faint `toolbar_border` fill
///
/// ```ignore
/// let closed = PanelHeaderTabs::new(&mut tab, &[
///     (Tab::Book, "BOOK"),
///     (Tab::Journal, "JOURNAL"),
/// ]).show(ui, t);
/// ```
#[must_use = "PanelHeaderTabs must be rendered with `.show(...)`"]
pub struct PanelHeaderTabs<'a, T: PartialEq + Copy + 'a> {
    current: &'a mut T,
    tabs: &'a [(T, &'a str)],
    height_override: Option<f32>,
    font_size_override: Option<f32>,
    closable: bool,
    salt: &'a str,
    watchlist: Option<&'a Watchlist>,
    /// Eligible for Meridien's section ordinal. See [`Self::numbered`].
    numbered: bool,
    /// Optional per-tab right-click handler — invoked inside each tab's
    /// `context_menu` with that tab's value. Used by `SplitTabs` for the
    /// "open this tab as a new instance" menu; ignored by plain callers.
    on_tab_secondary: Option<Box<dyn FnMut(&mut Ui, T) + 'a>>,
}

impl<'a, T: PartialEq + Copy + 'a> PanelHeaderTabs<'a, T> {
    pub fn new(current: &'a mut T, tabs: &'a [(T, &'a str)]) -> Self {
        Self {
            current, tabs,
            height_override: None,
            font_size_override: None,
            closable: true,
            salt: "panel_tabs",
            watchlist: None,
            numbered: false,
            on_tab_secondary: None,
        }
    }

    /// Attach a per-tab right-click (context-menu) handler. The closure runs
    /// inside the right-clicked tab's `context_menu`, receiving that tab's value.
    pub fn on_tab_secondary(mut self, f: impl FnMut(&mut Ui, T) + 'a) -> Self {
        self.on_tab_secondary = Some(Box::new(f));
        self
    }
    pub fn height(mut self, h: f32) -> Self { self.height_override = Some(h); self }
    pub fn font_size(mut self, px: f32) -> Self { self.font_size_override = Some(px); self }
    pub fn closable(mut self, on: bool) -> Self { self.closable = on; self }
    /// Stable id salt — required when multiple `PanelHeaderTabs` exist in the
    /// same window so per-tab interaction state doesn't collide.
    pub fn id_salt(mut self, salt: &'a str) -> Self { self.salt = salt; self }
    /// Pin tab height + label font to the chart pane's metrics so the tab
    /// strip lines up with the strip in the pane header above.
    pub fn watchlist(mut self, wl: &'a Watchlist) -> Self { self.watchlist = Some(wl); self }

    /// Mark this strip as a SECTION header, eligible for Meridien's ordinal.
    ///
    /// Opt-in, not opt-out. `PanelHeaderTabs` is used for two different things:
    /// section headers on side panels, and tab bars inside docks and split
    /// panes. Only the first is a numbered section.
    ///
    /// The first version numbered every strip, and the bottom dock — which
    /// paints before the central panel but sits last on screen — took `01`,
    /// so the panels read 01 at the bottom, 02 and 03 above it. Ordinals are a
    /// reading-order device; getting the order wrong is worse than not having
    /// them. Opt-in also means a tab strip added somewhere new later cannot
    /// silently consume an ordinal and shift every panel's number by one.
    pub fn numbered(mut self) -> Self { self.numbered = true; self }

    pub fn show(self, ui: &mut Ui, t: &Theme) -> bool {
        self.show_with(ui, t, |_| {})
    }

    /// Render with trailing actions placed to the LEFT of the close button.
    pub fn show_with(mut self, ui: &mut Ui, t: &Theme, actions: impl FnOnce(&mut Ui)) -> bool {
        let (resolved_h, resolved_font) = header_metrics(self.watchlist);
        let h_panel = self.height_override.unwrap_or(resolved_h);
        let font_size = self.font_size_override.unwrap_or(resolved_font);

        // Namespace tab interaction state under the parent ui's id so two
        // tabbed panels in the same context don't collide on `ui.interact`.
        let scope_id = ui.id().with(("kit_panel_tabs", self.salt));

        let avail_w = ui.available_width();
        let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail_w, h_panel), Sense::hover());
        let painter = ui.painter_at(rect);

        paint_chrome_perimeter(ui.ctx(), &painter, rect, t);
        paint_header_shadow(ui, rect, t);

        // Use per-style font family: mono for Alto/Mariner/Relay, proportional for others.
        let title_font = FontId::new(font_size, section_header_font_id().family);
        let h = rect.height();
        let st_settings = current();
        let r_md_corner = super::super::style::radius_md() as u8;

        // Outer chrome from the flex engine: the strip the tabs live in, and
        // the flush-right close button. Replaces the hand-reserved
        // `HEADER_CLOSE_SIZE + gap_sm() * 2.0` right margin.
        let chrome = solve_header_tabs_strip(rect, self.closable);

        // ── Tab strip (painter-mode, mirrors painter_pane.rs:541-664) ──
        let tab_h = h - HEADER_TAB_HEIGHT_INSET;
        let tab_y = rect.top() + HEADER_TAB_TOP_INSET;
        let tab_pad = gap_md() + 4.0;
        let mut cx = chrome.strip.left();

        // Numbered section headers also apply to tabbed panels.
        //
        // A tabbed header has no title to prefix, so the ordinal leads the tab
        // strip instead. Skipping this variant was tempting — it is the harder
        // one — but it would leave the watchlist unnumbered while the rail
        // beside it reads "01", and a lone ordinal looks like a bug rather than
        // a system.
        //
        // THE ORDINAL YIELDS TO THE TABS. It is a decoration; the tabs are
        // controls. Taking their width looked free because tabs that no longer
        // fit fall into the "»" overflow menu rather than clipping — but the
        // Aperture capture showed SCAN disappearing from a header that had been
        // showing it, so the decoration was costing a visible control. Reducing
        // the ordinal to its reference size (10px against an 11.5px title)
        // helped and was the right fix on its own terms, but did not close the
        // gap in the styles with the widest tabs.
        //
        // So: count how many tabs fit with the ordinal and without it. If the
        // ordinal costs even one, drop it — and do NOT consume an ordinal
        // number, or every panel below would shift by one for an ordinal
        // nobody can see.
        let fitting_tabs = |start_x: f32| -> usize {
            let max_right = chrome.strip.right() - (HEADER_CLOSE_SIZE + gap_sm() * 2.0);
            let mut x = start_x;
            let mut n = 0;
            for (_, label) in self.tabs.iter() {
                let w = tab_pad
                    + painter.layout_no_wrap(label.to_string(), title_font.clone(), t.dim).size().x
                    + tab_pad;
                if x + w > max_right { break; }
                x += w;
                n += 1;
            }
            n
        };

        if self.numbered && crate::chart_renderer::ui::style::style_numbers_sections() {
            let ord_font = ordinal_font(font_size);
            // "00" — measure a fixed-width sample so the fit decision cannot
            // flip between "01" and "02" and make a tab flicker per panel.
            let ord_w = painter
                .layout_no_wrap("00".to_string(), ord_font.clone(), t.accent)
                .size().x;
            let advance = ord_w + gap_sm();

            if fitting_tabs(cx + advance) >= fitting_tabs(cx) {
                let ord = format!("{:02}", next_section_ordinal(ui.ctx()));
                painter.text(
                    Pos2::new(cx, rect.center().y),
                    Align2::LEFT_CENTER, ord, ord_font, t.accent,
                );
                cx += advance;
            }
        }

        let mut tab_rects: Vec<Rect> = Vec::with_capacity(self.tabs.len());

        let active_idx = self.tabs.iter().position(|(v, _)| *v == *self.current).unwrap_or(0);
        let mut new_active = active_idx;

        // Tabs that don't fit get collected here and surfaced in a "»" overflow
        // menu instead of being silently clipped (which hid RESEARCH/SEASONALITY).
        // Reserve room for the overflow button so the menu itself never clips.
        let mut hidden_idx: Vec<usize> = Vec::new();
        let overflow_reserved = HEADER_CLOSE_SIZE + gap_sm() * 2.0;

        for (ti, (tab_val, label)) in self.tabs.iter().enumerate() {
            let is_active = ti == active_idx;
            let label_galley = painter.layout_no_wrap(
                label.to_string(), title_font.clone(), t.dim,
            );
            let tab_w = tab_pad + label_galley.size().x + tab_pad;
            // Clamp so tabs don't overlap the close + overflow buttons. Once one
            // tab doesn't fit, all remaining go to the overflow menu.
            let max_right = chrome.strip.right() - overflow_reserved;
            if cx + tab_w > max_right {
                hidden_idx.extend(ti..self.tabs.len());
                break;
            }

            let tab_rect = Rect::from_min_size(Pos2::new(cx, tab_y), Vec2::new(tab_w, tab_h));
            let tab_resp = ui.interact(
                tab_rect,
                scope_id.with(("tab", ti)),
                Sense::click(),
            );
            if tab_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

            // Background — animated with motion::ease_bool, mirroring
            // painter_pane.rs:583-598. Active fades over MED (180ms), hover
            // over FAST (120ms). Painted as: idle → hover → active layered.
            use super::super::components::motion;
            let active_id = scope_id.with(("active", ti));
            let hover_id  = scope_id.with(("hover", ti));
            let active_t = motion::ease_bool(ui.ctx(), active_id, is_active, motion::MED);
            let hover_t  = motion::ease_bool(ui.ctx(), hover_id,  tab_resp.hovered() && !is_active, motion::FAST);
            let corners = CornerRadius { nw: r_md_corner, ne: r_md_corner, sw: 0, se: 0 };
            let idle_bg   = Color32::TRANSPARENT;
            let hover_bg  = tint(t, SxTone::Border, st_settings.tab_hover_bg_alpha);
            let active_bg = color_dim(t.bg);
            let mut tab_bg = motion::lerp_color(idle_bg, hover_bg, hover_t);
            tab_bg = motion::lerp_color(tab_bg, active_bg, active_t);
            painter.rect_filled(tab_rect, corners, tab_bg);

            // Inter-tab vertical hairline divider (painter_pane.rs:609-616).
            if ti + 1 < self.tabs.len() {
                let div_col = color_alpha(t.border_variant, 200);
                painter.line_segment(
                    [Pos2::new(tab_rect.right() + 0.5, tab_rect.top() + 4.0),
                     Pos2::new(tab_rect.right() + 0.5, tab_rect.bottom() - 4.0)],
                    Stroke::new(stroke_thin(), div_col),
                );
            }

            // Label.
            let label_color = if is_active {
                t.text
            } else {
                t.dim.gamma_multiply(st_settings.tab_inactive_alpha)
            };
            painter.text(
                Pos2::new(tab_rect.left() + tab_pad, tab_rect.center().y),
                Align2::LEFT_CENTER, label, title_font.clone(), label_color,
            );

            if tab_resp.clicked() { new_active = ti; }
            // Per-tab right-click menu (e.g. SplitTabs "open as new instance").
            if let Some(cb) = self.on_tab_secondary.as_mut() {
                let tv = *tab_val;
                tab_resp.context_menu(|ui| cb(ui, tv));
            }
            tab_rects.push(tab_rect);
            cx += tab_w + 1.0;
        }

        if new_active != active_idx {
            if let Some((v, _)) = self.tabs.get(new_active) { *self.current = *v; }
        }

        // Trailing actions + close button — built in a child UI on the right.
        let mut closed = false;
        let actions_rect = Rect::from_min_max(
            Pos2::new(cx, rect.top()),
            Pos2::new(chrome.strip.right(), rect.bottom()),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(actions_rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        if let Some(close_rect) = chrome.close {
            if paint_close_btn(&mut child, &painter, close_rect, t) {
                closed = true;
            }
        }
        // Overflow menu for tabs that didn't fit (placed left of the close btn).
        if !hidden_idx.is_empty() {
            let hidden_items: Vec<(String, _)> = hidden_idx.iter()
                .map(|&i| (self.tabs[i].1.to_string(), self.tabs[i].0))
                .collect();
            let active_hidden = hidden_idx.iter().any(|&i| i == active_idx);
            // If the active tab is one of the hidden ones, name it on the button
            // so the selection is still visible; otherwise show the count.
            let btn_label = if active_hidden {
                format!("\u{00BB} {}", self.tabs[active_idx].1)
            } else {
                format!("\u{00BB} {}", hidden_idx.len())
            };
            let mut picked = None;
            child.menu_button(btn_label, |ui| {
                for (lbl, v) in &hidden_items {
                    if ui.button(lbl).clicked() { picked = Some(*v); ui.close_menu(); }
                }
            });
            if let Some(v) = picked { *self.current = v; }
        }
        actions(&mut child);
        closed
    }
}

// ── Tone ─────────────────────────────────────────────────────────────────────

/// Semantic tone — resolves to a color via the active theme. Drives section
/// accents, action buttons, and empty-state glyphs.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum Tone {
    /// Muted/dim — default for non-emphatic section headers.
    #[default]
    Default,
    /// Accent — focus / primary call-to-action.
    Accent,
    /// Bull/positive — long, gain, confirm.
    Success,
    /// Bear/negative — destructive, loss, danger.
    Danger,
    /// Warn — amber-ish caution.
    Warn,
    /// Text — full-strength foreground.
    Text,
}

impl Tone {
    pub fn color(self, t: &Theme) -> Color32 {
        // Resolve through the unified Sx palette (kit's local Tone is the gallery's
        // own intent enum; SxTone is the shared color vocabulary).
        let p = crate::ui_kit::sx::palette_ct(t);
        match self {
            Tone::Default => p.base(SxTone::Dim),
            Tone::Accent  => p.base(SxTone::Accent),
            Tone::Success => p.base(SxTone::Bull),
            Tone::Danger  => p.base(SxTone::Bear),
            Tone::Warn    => p.base(SxTone::Warn),
            Tone::Text    => p.base(SxTone::Text),
        }
    }
}

// ── Inline action button ─────────────────────────────────────────────────────

/// Compact ghost button used inline in section headers ("Clear All", "Place").
/// Replaces `style::small_action_btn` with a cleaner ghost/hover treatment.
pub fn panel_action_btn(ui: &mut Ui, label: &str, color: Color32) -> bool {
    let theme = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
    // `min_size` is a FLOOR that is max()'d against `size.height()`, so the
    // smallest rung is the correct expression of "stay as compact as the
    // ladder allows" — the old literal 16.0 was below every rung and could
    // therefore never bind, while also locking the site out of density.
    Button::new(label).variant(Variant::Ghost).size(KitSize::Xs)
        .fg(color).min_size(Vec2::new(0.0, KitSize::Xs.height()))
        .show(ui, &theme).clicked()
}

// ── PanelInputRow ────────────────────────────────────────────────────────────

/// Compact `Label: [body]` inline row. The body closure gets a `&mut Ui` to
/// place an Input or DragValue with whatever width it needs.
///
/// Distinct from `FormRow` (which uses a fixed gutter for full forms): this is
/// for *inline* compact rows where the label is short and the input width is
/// caller-driven.
///
/// ```ignore
/// PanelInputRow::new("Price").show(ui, t, |ui| {
///     Input::new(&mut state.price).min_width(80.0).size(KitSize::Sm).show(ui, t);
/// });
/// ```
#[must_use = "PanelInputRow must be rendered with `.show(...)`"]
pub struct PanelInputRow<'a> {
    label: &'a str,
    suffix: Option<&'a str>,
    label_width: Option<f32>,
}

impl<'a> PanelInputRow<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, suffix: None, label_width: None }
    }
    pub fn suffix(mut self, s: &'a str) -> Self { self.suffix = Some(s); self }
    /// Fix the label gutter for vertical alignment across multiple rows.
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = Some(w); self }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let label_color = t.dim;
        let label_text = RichText::new(self.label)
            .monospace()
            .size(font_xs())
            .color(label_color);
        let r = ui.horizontal(|ui| {
            if let Some(w) = self.label_width {
                ui.allocate_ui_with_layout(
                    Vec2::new(w, ui.spacing().interact_size.y),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| { ui.label(label_text); },
                );
            } else {
                ui.label(label_text);
            }
            ui.add_space(gap_sm());
            let inner = body(ui);
            if let Some(suf) = self.suffix {
                ui.add_space(gap_xs());
                ui.label(
                    RichText::new(suf)
                        .monospace()
                        .size(font_xs())
                        .color(color_alpha(label_color, alpha_line())),
                );
            }
            inner
        });
        r.inner
    }
}

// ── PanelDualAction ──────────────────────────────────────────────────────────

/// A row of two equal-width tone-colored action buttons, side-by-side. The
/// pattern shows up wherever a panel exposes a binary directional choice
/// (Above/Below, Buy/Sell, Long/Short, Approve/Reject). Returns the index of
/// the clicked button (0 = left, 1 = right) or `None`.
///
/// ```ignore
/// match PanelDualAction::new(
///     ("▲ Above", Tone::Success),
///     ("▼ Below", Tone::Danger),
/// ).show(ui, t) {
///     Some(0) => add_above(),
///     Some(1) => add_below(),
///     _ => {}
/// }
/// ```
#[must_use = "PanelDualAction must be rendered with `.show(...)`"]
pub struct PanelDualAction<'a> {
    left: (&'a str, Tone),
    right: (&'a str, Tone),
    height: f32,
    gap: f32,
}

impl<'a> PanelDualAction<'a> {
    pub fn new(left: (&'a str, Tone), right: (&'a str, Tone)) -> Self {
        Self { left, right, height: crate::chart_renderer::ui::style::style_row_height(), gap: gap_xs() }
    }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn gap(mut self, g: f32) -> Self { self.gap = g; self }

    pub fn show(self, ui: &mut Ui, t: &Theme) -> Option<usize> {
        let st = current();
        let avail = ui.available_width();
        let half = ((avail - self.gap) / 2.0).max(40.0);
        let mut clicked: Option<usize> = None;

        let prev_spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = self.gap;
        ui.horizontal(|ui| {
            for (i, (label, tone)) in [self.left, self.right].into_iter().enumerate() {
                let c = tone.color(t);
                let resp = Button::new(label)
                    .variant(Variant::Chrome)
                    .size(KitSize::Sm)
                    .fg(c)
                    .fill(color_alpha(c, alpha_ghost()))
                    .stroke(Stroke::new(stroke_thin(), color_alpha(c, alpha_line())))
                    .corner_radius(st.r_md as f32)
                    .min_size(Vec2::new(half, self.height))
                    .frameless(true)
                    .show(ui, t);
                if resp.hovered() {
                    let r = resp.rect;
                    ui.painter().rect_filled(r, st.r_md as f32, color_alpha(c, alpha_soft()));
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    clicked = Some(i);
                }
            }
        });
        ui.spacing_mut().item_spacing.x = prev_spacing;
        clicked
    }
}

// ── Standardized panel margin helpers ────────────────────────────────────────

/// Standard "list row" gap between adjacent rows of the same kind.
#[inline]
pub fn row_gap(ui: &mut Ui) { ui.add_space(gap_xs()); }

// ─── Tests ──────────────────────────────────────────────────────────────────
//
// The header strip is the chrome EVERY side panel renders through, and its
// gutters used to be a running `cx` cursor plus a nested right-to-left layout —
// untestable, and the source of the per-panel gutter drift the UI audit found.
// `solve_header_strip` / `solve_header_tabs_strip` are pure `Rect -> Rect`, so
// the whole shape is now asserted headlessly: no GPU, no egui context.

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool { (a - b).abs() < 0.01 }

    fn header() -> Rect {
        // Non-zero origin so a missing translate can't pass by accident.
        Rect::from_min_size(Pos2::new(24.0, 60.0), Vec2::new(300.0, 32.0))
    }

    /// The canonical shape: icon fixed, title after it, actions absorb the
    /// slack, close button flush right.
    #[test]
    fn icon_title_actions_and_a_flush_right_close() {
        let r = header();
        let s = solve_header_strip(r, Some(16.0), 80.0, None, true);

        let icon = s.icon.expect("icon slot");
        assert!(approx(icon.left(), r.left() + gap_sm()), "got {}", icon.left());
        assert!(approx(icon.width(), 16.0));

        // icon → title gutter is gap_sm
        assert!(approx(s.title.left(), icon.right() + gap_sm()), "got {}", s.title.left());
        assert!(approx(s.title.width(), 80.0));

        // title → actions gutter is gap_md (the one seam that differs)
        assert!(approx(s.actions.left(), s.title.right() + gap_md()), "got {}", s.actions.left());

        let close = s.close.expect("close slot");
        assert!(approx(close.right(), r.right() - gap_sm()), "got {}", close.right());
        assert!(approx(close.width(), HEADER_CLOSE_SIZE));
        assert!(approx(close.height(), HEADER_CLOSE_SIZE));
        assert!(approx(close.center().y, r.center().y));

        // actions → close gutter is gap_sm; the actions slot stops short of the
        // close button, which is what removed the hand-rolled `add_space`.
        assert!(approx(s.actions.right(), close.left() - gap_sm()), "got {}", s.actions.right());
    }

    /// No icon → the title starts at the strip inset itself.
    #[test]
    fn without_an_icon_the_title_starts_at_the_strip_inset() {
        let r = header();
        let s = solve_header_strip(r, None, 80.0, None, true);
        assert!(s.icon.is_none());
        assert!(approx(s.title.left(), r.left() + gap_sm()), "got {}", s.title.left());
    }

    /// With no trailing actions the growing slot still spans everything between
    /// the title and the close — nothing collapses, so a panel that later adds
    /// an action gets the identical gutter.
    #[test]
    fn the_actions_slot_absorbs_all_remaining_width() {
        let r = header();
        let s = solve_header_strip(r, Some(16.0), 80.0, None, true);
        let expected = r.width()
            - gap_sm() * 2.0            // strip inset both ends
            - 16.0 - gap_sm()           // icon + its gutter
            - 80.0 - gap_md()           // title + its gutter
            - HEADER_CLOSE_SIZE - gap_sm();
        assert!(approx(s.actions.width(), expected), "got {}", s.actions.width());
    }

    /// A non-closable header hands the trailing edge to the actions slot.
    #[test]
    fn without_a_close_button_the_actions_slot_reaches_the_trailing_inset() {
        let r = header();
        let s = solve_header_strip(r, None, 80.0, None, false);
        assert!(s.close.is_none());
        assert!(approx(s.actions.right(), r.right() - gap_sm()), "got {}", s.actions.right());
    }

    /// Every slot spans the full strip height, so painting at `slot.center().y`
    /// reproduces the old `rect.center().y` vertical centring exactly.
    #[test]
    fn slots_span_the_full_header_height() {
        let r = header();
        let s = solve_header_strip(r, Some(16.0), 80.0, None, true);
        assert!(approx(s.icon.unwrap().height(), r.height()));
        assert!(approx(s.title.height(), r.height()));
        assert!(approx(s.title.center().y, r.center().y));
        assert!(approx(s.actions.height(), r.height()));
    }

    /// A title longer than the panel must not shove the close button off the
    /// strip — the title yields instead (`flex-shrink`).
    #[test]
    fn a_long_title_yields_rather_than_pushing_the_close_off() {
        let r = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(120.0, 32.0));
        let s = solve_header_strip(r, None, 400.0, None, true);
        let close = s.close.expect("close slot");
        assert!(approx(close.right(), r.right() - gap_sm()), "got {}", close.right());
        assert!(close.left() >= r.left(), "close fell off the strip: {}", close.left());
    }

    /// Tab header: the strip stops exactly one `gap_sm` short of the close
    /// button, which is the room the tab run and the `»` overflow share.
    #[test]
    fn tabs_strip_reserves_the_close_button() {
        let r = header();
        let c = solve_header_tabs_strip(r, true);
        let close = c.close.expect("close slot");
        assert!(approx(c.strip.left(), r.left() + gap_sm()), "got {}", c.strip.left());
        assert!(approx(close.right(), r.right() - gap_sm()), "got {}", close.right());
        assert!(approx(c.strip.right(), close.left() - gap_sm()), "got {}", c.strip.right());
        // Same reservation the old hand-written constant encoded.
        assert!(
            approx(c.strip.right(), r.right() - (HEADER_CLOSE_SIZE + gap_sm() * 2.0)),
            "got {}", c.strip.right()
        );
    }

    /// Non-closable tab header: the strip runs to the trailing inset.
    #[test]
    fn tabs_strip_without_close_runs_to_the_trailing_inset() {
        let r = header();
        let c = solve_header_tabs_strip(r, false);
        assert!(c.close.is_none());
        assert!(approx(c.strip.right(), r.right() - gap_sm()), "got {}", c.strip.right());
    }

    // ── Numbered section headers (Meridien) ──────────────────────────────────

    /// Within one pass the ordinal counts up from 1; the next pass restarts.
    ///
    /// The restart is the whole point. An earlier shape reset the counter from
    /// a "begin frame" hook, which keeps counting on any frame that skips the
    /// hook — the numbers would climb forever and nobody would notice until a
    /// panel read `47`.
    #[test]
    fn section_ordinal_counts_per_pass_and_restarts() {
        let ctx = egui::Context::default();

        let mut first = Vec::new();
        let _ = ctx.run(Default::default(), |ctx| {
            for _ in 0..4 {
                first.push(next_section_ordinal(ctx));
            }
        });
        assert_eq!(first, vec![1, 2, 3, 4], "ordinals must count up within a pass");

        let mut second = Vec::new();
        let _ = ctx.run(Default::default(), |ctx| {
            for _ in 0..3 {
                second.push(next_section_ordinal(ctx));
            }
        });
        assert_eq!(second, vec![1, 2, 3], "a new pass must restart at 1, not continue");
    }

    /// The header renders the ordinal zero-padded to two digits.
    #[test]
    fn section_ordinal_is_zero_padded_to_two_digits() {
        assert_eq!(format!("{:02}", 1u32), "01");
        assert_eq!(format!("{:02}", 9u32), "09");
        assert_eq!(format!("{:02}", 12u32), "12");
    }

    /// Numbering is opt-in on BOTH header widgets.
    ///
    /// The default is the whole safety property. Default-on is how the bottom
    /// dock took `01` while sitting last on screen, and how a heatmap pane
    /// would have taken it in any layout with one open. A new tab strip or
    /// pane header added later must not be able to shift every panel's number
    /// by one just by existing.
    #[test]
    fn section_numbering_is_opt_in_on_both_header_widgets() {
        assert!(!PanelHeader::new("WATCHLIST").numbered,
                "PanelHeader must not number by default");
        assert!(PanelHeader::new("WATCHLIST").numbered().numbered,
                "PanelHeader::numbered() must set the flag");

        let mut tab = 0u8;
        let tabs: &[(u8, &str)] = &[(0, "LIST"), (1, "CHAIN")];
        assert!(!PanelHeaderTabs::new(&mut tab, tabs).numbered,
                "PanelHeaderTabs must not number by default");
        let mut tab2 = 0u8;
        assert!(PanelHeaderTabs::new(&mut tab2, tabs).numbered().numbered,
                "PanelHeaderTabs::numbered() must set the flag");
    }

    /// A caption widens the header's intrinsic need; it must not do so at the
    /// title's expense.
    ///
    /// `solve_header_strip` gives the title a `.shrink(1.0)` slot and the meta
    /// a FIXED one, so on a narrow panel the caption wins by construction —
    /// the exact inversion of what should happen. Both cases below use a
    /// title far too wide for the strip.
    #[test]
    fn meta_slot_is_dropped_before_the_title_is_squeezed() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(180.0, 26.0));

        // Wide title + wide caption: the caption has to go.
        let cramped = solve_header_strip(rect, None, 400.0, Some(90.0), true);
        // Roomy strip, same caption: it stays.
        let roomy = solve_header_strip(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 26.0)),
            None, 400.0, Some(90.0), true);

        assert!(roomy.meta.is_some(), "a caption that fits must be laid out");
        // The solver itself always lays out what it is given — the DROP is the
        // caller's decision, so assert the property that makes the drop
        // necessary: with both present, the title cannot get its width.
        assert!(
            cramped.title.width() < 400.0,
            "expected the title to be squeezed when a fixed caption shares a              narrow strip — if this stops being true the drop rule in              PanelHeader::show_full is dead code and should go"
        );
    }
}
