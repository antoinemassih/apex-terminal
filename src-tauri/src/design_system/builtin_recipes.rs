//! Built-in per-style [`RecipeSet`]s — the Rust transcription of the React
//! port's `html[data-ds="<style>"] .ds-<component> { … }` structural overrides.
//!
//! # Why this file exists
//!
//! The palette axis ([`super::color_scheme::ColorScheme`]) owns **colour**.
//! The style axis ([`super::style_system::StyleSystem`]) owns **global scale**
//! (typography tiers, radii ramp, spacing ramp, stroke widths, elevation).
//! Neither can say *"in Cadence, `button.primary` specifically is a full pill
//! with a bold face, while `row.list` is flush and square"* — that is a
//! **per-component** statement, and it is exactly what a [`RecipeSet`] encodes.
//!
//! Without this payload, switching styles only re-scales and re-colours; with
//! it, switching styles restyles *components*.
//!
//! # Source of truth
//!
//! `ApexTerminalThemes/terminal/src/global.css` — the 259
//! `html[data-ds="…"] .ds-…` rules, plus each style's `[data-ds="…"]` token
//! block (`--ds-control-radius`, `--ds-pill-radius`, `--ds-card-radius`,
//! `--ds-card-border`, `--ds-card-pad`). Only the highest-signal rules are
//! transcribed here — the ones that make a style recognisable at a glance.
//!
//! # Authoring rules (enforced by review, not by the type system)
//!
//! 1. **Only registered keys.** See `docs/migration/recipe-keys.md`. A recipe
//!    for an unregistered key is dead data — no widget will ever read it.
//! 2. **No hex literals.** Colours are [`ColorSpec::Tone`] / [`ColorSpec::Alpha`]
//!    so a recipe survives being paired with any of the 16 palettes. The CSS
//!    says `#1a1612` for "ink on the orange bar"; the palette-agnostic spelling
//!    of that is `Tone::Bg` (the canvas colour), which is correct in both dark
//!    and light palettes. The same substitution carries the bevel rules:
//!    `rgba(255,238,210,.06)` (Alto's warm highlight) and
//!    `rgba(180,210,240,.08)` (Mariner's cool one) are both *"the text colour at
//!    a low alpha"* → `Tone::Text`; `rgba(0,0,0,.35)` is *"the canvas at a high
//!    alpha"* → `Tone::Bg`.
//! 3. **Tiers over pixels.** `RadiusTier::Pill` / `PadTier::Md` track the active
//!    style's ramp. `Px(_)` is reserved for values the CSS states literally and
//!    that no tier expresses (e.g. Aperture's 1.5px outline, Cadence's 3px tab
//!    underline).
//!
//! # What is intentionally NOT here
//!
//! CSS properties with no representation in [`RecipeSpec`] are skipped rather
//! than approximated. M3.2 closed four of them — per-side borders
//! (`border-bottom` / `border-left` / `border-top`), inset `box-shadow` bevels,
//! `font-weight`, and `gap` — and those rules are now authored below. The
//! **still-open** list (input to a future Sx-vocabulary ticket):
//! `font-family`, `text-transform`, `letter-spacing`, DROP (non-inset)
//! `box-shadow`, `linear-gradient` fills, `filter: brightness`,
//! `transform: translateY`, fixed `height`, `margin`, and `min-width`.
//!
//! Two second-order limits remain even inside the new vocabulary and are
//! called out at each site: `BorderSpecRef` carries exactly **one** edge
//! selection, so a rule that sets `border-top` *and* `border-bottom` (the DOM
//! "current price" band) lands only its dominant edge; and `BevelSpecRef` has
//! a single width, so the 0.5px chip hairlines collapse onto the 1px line.
//!
//! # Coverage
//!
//! Six of the nine built-in styles are authored: `aperture`, `cadence`,
//! `alto`, `mariner`, `lucid`, `meridien`. The remaining three (`octave`,
//! `relay`, `glass`) return an **empty** set — an empty set resolves to the
//! widget's built-in `Sx` unchanged, so they are byte-identical to today.

use super::recipes::RecipeSet;
use crate::ui_kit::sx::recipe_spec::{
    BorderSpecRef, BevelSpecRef, EdgesRef, BorderWidthTier, ColorSpec, PadTier, RadiusTier, RecipeDelta, RecipeSpec,
    ShadeRef, TextSizeTier, TextSpec, ToneRef,
};

// ── Authoring helpers ────────────────────────────────────────────────────────
//
// These exist purely to keep the recipe tables readable — one line per CSS
// declaration instead of five lines of struct-literal ceremony. They are
// private to this module.

/// A tone at its base (500) shade.
#[inline]
fn tone(t: ToneRef) -> ColorSpec {
    ColorSpec::Tone { tone: t, shade: None }
}

/// A tone at an explicit ramp shade (S400 = one step lighter, S600 = darker).
#[inline]
fn shade(t: ToneRef, s: ShadeRef) -> ColorSpec {
    ColorSpec::Tone { tone: t, shade: Some(s) }
}

/// A tone's base colour at an explicit alpha — the palette-agnostic spelling of
/// CSS `color-mix(in srgb, var(--ds-accent) N%, transparent)`.
#[inline]
fn tint(t: ToneRef, a: u8) -> ColorSpec {
    ColorSpec::Alpha { tone: t, alpha: a }
}

/// Fully transparent — the second stop of a one-sided CSS inset shadow, where
/// `bevel_raised` wants a bottom line but the CSS only declares a top one.
#[inline]
fn nothing() -> ColorSpec {
    ColorSpec::Alpha { tone: ToneRef::Bg, alpha: 0 }
}

/// Chainable setters for [`RecipeDelta`]. The struct is a plain bag of
/// `Option`s; this turns authoring into `d().radius(Pill).fill(...)`.
#[allow(dead_code)] // authoring vocabulary — not every setter is used by every style
trait DeltaExt: Sized {
    fn radius(self, r: RadiusTier) -> Self;
    fn px(self, p: PadTier) -> Self;
    fn py(self, p: PadTier) -> Self;
    fn fill(self, c: ColorSpec) -> Self;
    fn border(self, c: ColorSpec, w: BorderWidthTier) -> Self;
    /// Explicitly "no stroke" — the schema spelling of `--ds-card-border: none`.
    fn no_border(self) -> Self;
    fn ink(self, t: ToneRef) -> Self;
    fn text_size(self, s: TextSizeTier) -> Self;
    /// M3.2: border on selected EDGES only (tab underlines, ledger hairlines).
    fn border_edge(self, c: ColorSpec, w: BorderWidthTier, e: EdgesRef) -> Self;
    /// M3.2: Zed/Spotify raised face — light top line + dark bottom line.
    fn bevel_raised(self, top: ColorSpec, bottom: ColorSpec) -> Self;
    /// M3.2: a single inset marker line (Alto's `inset 0 -2px 0 accent` tab).
    fn bevel_marker(self, bottom: ColorSpec, width: f32) -> Self;
    /// M3.2: font weight (advisory until per-weight families are registered).
    fn weight(self, w: u16) -> Self;
    /// M3.2: inter-element gap (`RecipeDelta.gap`, CSS `gap:`).
    fn gap(self, g: PadTier) -> Self;
}

impl DeltaExt for RecipeDelta {
    fn radius(mut self, r: RadiusTier) -> Self { self.radius = Some(r); self }
    fn px(mut self, p: PadTier) -> Self { self.px = Some(p); self }
    fn py(mut self, p: PadTier) -> Self { self.py = Some(p); self }
    fn fill(mut self, c: ColorSpec) -> Self { self.fill = Some(c); self }
    fn border(mut self, c: ColorSpec, w: BorderWidthTier) -> Self {
        self.border = Some(BorderSpecRef { color: c, width: Some(w), edges: EdgesRef::All });
        self
    }
    fn no_border(mut self) -> Self {
        self.border = Some(BorderSpecRef {
            color: tint(ToneRef::Border, 0),
            width: Some(BorderWidthTier::None),
            edges: EdgesRef::All,
        });
        self
    }
    fn ink(mut self, t: ToneRef) -> Self { self.text = Some(TextSpec { tone: t, shade: None }); self }
    fn text_size(mut self, s: TextSizeTier) -> Self { self.text_size = Some(s); self }

    // ── M3.2 vocabulary ────────────────────────────────────────────────────
    fn border_edge(mut self, c: ColorSpec, w: BorderWidthTier, e: EdgesRef) -> Self {
        self.border = Some(BorderSpecRef { color: c, width: Some(w), edges: e });
        self
    }
    fn bevel_raised(mut self, top: ColorSpec, bottom: ColorSpec) -> Self {
        self.bevel = Some(BevelSpecRef { top: Some(top), bottom: Some(bottom), width: 1.0 });
        self
    }
    fn bevel_marker(mut self, bottom: ColorSpec, width: f32) -> Self {
        self.bevel = Some(BevelSpecRef { top: None, bottom: Some(bottom), width });
        self
    }
    fn weight(mut self, w: u16) -> Self { self.weight = Some(w); self }
    fn gap(mut self, g: PadTier) -> Self { self.gap = Some(g); self }
}

/// A fresh empty delta.
#[inline]
fn d() -> RecipeDelta { RecipeDelta::default() }

/// A spec whose base is `base` and which has no state overrides.
#[inline]
fn spec(base: RecipeDelta) -> RecipeSpec {
    RecipeSpec { base, ..Default::default() }
}

/// Chainable per-state setters for [`RecipeSpec`].
#[allow(dead_code)] // authoring vocabulary — `on_press` has no CSS counterpart yet
trait SpecExt: Sized {
    fn on_hover(self, d: RecipeDelta) -> Self;
    fn on_press(self, d: RecipeDelta) -> Self;
    /// The `.is-active` / `.is-selected` CSS state (toggled nav button, active
    /// tab, selected row). `apply_over` folds this into `StyleState::Active`.
    fn on_select(self, d: RecipeDelta) -> Self;
}

impl SpecExt for RecipeSpec {
    fn on_hover(mut self, d: RecipeDelta) -> Self { self.hover = Some(d); self }
    fn on_press(mut self, d: RecipeDelta) -> Self { self.active = Some(d); self }
    fn on_select(mut self, d: RecipeDelta) -> Self { self.selected = Some(d); self }
}

/// Build a set from a `(key, spec)` table.
fn set(rows: Vec<(&'static str, RecipeSpec)>) -> RecipeSet {
    let mut s = RecipeSet::new();
    for (k, v) in rows {
        s.insert(k, v);
    }
    s
}

// ── Public entry point ───────────────────────────────────────────────────────

/// The authored [`RecipeSet`] for a built-in style id.
///
/// Returns an **empty** set for any id that has no authored recipes (including
/// user-installed styles and the three unauthored built-ins) — an empty set is
/// a guaranteed no-op in the resolution chain.
pub fn builtin_recipes(style_id: &str) -> RecipeSet {
    match style_id {
        "aperture" => aperture(),
        "cadence" => cadence(),
        "alto" => alto(),
        "mariner" => mariner(),
        "lucid" => lucid(),
        "meridien" => meridien(),
        _ => RecipeSet::new(),
    }
}

// ── Aperture ─────────────────────────────────────────────────────────────────
//
// global.css 614–829, 1429–1464 + the `[data-ds="aperture"]` token block
// (183–222). Signature: pills EVERYWHERE, inverted-block active states
// (ink fill / canvas text), big-radius borderless tiles, chunky padding.
//   --ds-control-radius: radius-sm (10px)   --ds-pill-radius: pill
//   --ds-card-radius:    radius-lg (20px)   --ds-card-border: none
//   --ds-card-pad:       16px

fn aperture() -> RecipeSet {
    set(vec![
        // `.ds-btn { border-radius: pill }` + `.ds-btn--primary { background:
        // accent; color: #1a1612; font-weight: 600 }` — orange block,
        // canvas-dark ink, semibold face.
        (
            "button.primary",
            spec(
                d().radius(RadiusTier::Pill)
                    .fill(tone(ToneRef::Accent))
                    .ink(ToneRef::Bg)
                    .px(PadTier::Lg)
                    .weight(600),
            )
            .on_hover(d().fill(shade(ToneRef::Accent, ShadeRef::S400))),
        ),
        // `button.action` — SHAPE ONLY, for the big block controls in a
        // trading action row (DOM BUY / SELL / FLATTEN / CANCEL).
        //
        // Radius and nothing else. The first version of this key mirrored
        // `button.primary` in full, including `.fill(Accent)` — which promptly
        // repainted FLATTEN (neutral) and CANCEL (soft red) in accent orange
        // and collapsed the row's semantics, the exact defect that BUY-and-SELL
        // -are-the-same-colour was. A CONTEXT key must not decide TONE; the
        // variant does that.
        //
        // Why it exists: `button.primary`'s `Pill` resolves to min(w, h) / 2,
        // correct in general and a full ELLIPSE on a ~64x52 control. These four
        // used to escape that with a hardcoded `corner_radius_asymmetric` per
        // call site, which pinned the shape. Now each style picks it.
        (
            "button.action",
            spec(d().radius(RadiusTier::Lg)),
        ),
        // ── Form controls ────────────────────────────────────────────────
        // `input` / `select` / `checkbox` share this system's control radius.
        // Radius ONLY: fill and border stay with the widget, which already
        // computes them from state (focus, invalid, disabled, hover). A recipe
        // that also set them would flatten those states — the same mistake
        // `button.action` made on its first pass.
        (
            "input",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "select",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "checkbox",
            spec(d().radius(RadiusTier::Md)),
        ),
        // ── Surfaces & compound controls ─────────────────────────────────
        // `popover` (context menus + tool popovers), `segmented` (the trough),
        // `switch` (the track). Radius only — fills encode STATE and stay with
        // the widget. `switch` keeps a pill everywhere: a track is a capsule by
        // definition, and squaring it would read as a broken toggle rather than
        // a restyled one.
        (
            "popover",
            spec(d().radius(RadiusTier::Lg)),
        ),
        (
            "segmented",
            spec(d().radius(RadiusTier::Lg)),
        ),
        (
            "switch",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // ── Feedback surfaces & meters ───────────────────────────────────
        // `alert` / `tooltip` are surfaces; `badge` / `progress` are meters and
        // stay PILL everywhere (a badge and a progress track are capsules by
        // definition — squaring them reads as broken, not restyled).
        // Radius only in every case: tone and fill encode state.
        (
            "alert",
            spec(d().radius(RadiusTier::Lg)),
        ),
        (
            "tooltip",
            spec(d().radius(RadiusTier::Lg)),
        ),
        (
            "badge",
            spec(d().radius(RadiusTier::Pill)),
        ),
        (
            "progress",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `slider` — the track. Pill like `progress` and `switch`; its own
        // key because a Slider is interactive and carries a thumb, so a style
        // may want to separate the two.
        (
            "slider",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `.ds-btn--secondary { border: 1.5px solid border; background:
        // transparent }` + `.ds-btn { font-weight: 500 }`, `.is-active` →
        // inverted block.
        (
            "button.ghost",
            spec(
                d().radius(RadiusTier::Pill)
                    .border(tone(ToneRef::Border), BorderWidthTier::Px(1.5))
                    .weight(500),
            )
            .on_select(
                d().fill(tone(ToneRef::Text))
                    .ink(ToneRef::Bg)
                    .border(tone(ToneRef::Text), BorderWidthTier::Px(1.5)),
            ),
        ),
        // `.ds-btn--chrome { padding: 0 14px }`, `.is-active { background: fg;
        // color: bg; font-weight: 600 }` — the Aperture inverted-block signature.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Pill).px(PadTier::Px(14.0)))
                .on_select(d().fill(tone(ToneRef::Text)).ink(ToneRef::Bg).weight(600)),
        ),
        // `.ds-tab--underline { border-radius: pill; border-bottom: none;
        // padding: 0 14px }` — tabs stop being tabs and become pills.
        (
            "tab.line",
            spec(d().radius(RadiusTier::Pill).px(PadTier::Px(14.0)).no_border()),
        ),
        // `.ds-tab--underline.is-selected { background: fg; color: bg;
        // font-weight: 600 }`.
        (
            "tab.line.active",
            spec(d().radius(RadiusTier::Pill))
                .on_select(d().fill(tone(ToneRef::Text)).ink(ToneRef::Bg).weight(600)),
        ),
        // `.ds-pill { border-radius: pill; border-width: 1.5px;
        // font-weight: 500 }`, `.ds-pill--md { padding: 0 14px }`, and
        // `.ds-chip-row { gap: 6px }` — the row gap rides on the chip recipe
        // because there is no registered `chip.row` key; the container that
        // lays chips out reads `gap` from the same set.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Pill)
                    .border(tone(ToneRef::Border), BorderWidthTier::Px(1.5))
                    .px(PadTier::Px(10.0))
                    .gap(PadTier::Px(6.0))
                    .weight(500),
            ),
        ),
        // `--ds-card-radius: radius-lg` + `--ds-card-border: none` +
        // `--ds-card-pad: 16px`. The borderless block-colour tile.
        (
            "card",
            spec(d().radius(RadiusTier::Lg).no_border().px(PadTier::Px(16.0)).py(PadTier::Px(16.0))),
        ),
        // `.ds-wl-row { border-radius: pill; padding: 0 10px; border-bottom:
        // none }`, `:hover { background: rgba(255,255,255,0.05) }`, and
        // `.ds-wl-sym { font-weight: 700 }`.
        (
            "row.list",
            spec(d().radius(RadiusTier::Pill).px(PadTier::Px(10.0)).no_border().weight(700))
                .on_hover(d().fill(tint(ToneRef::Text, 13))),
        ),
        (
            "row.list.hover",
            spec(d()).on_hover(d().fill(tint(ToneRef::Text, 13))),
        ),
        // `.ds-panel__header { background: transparent; border-bottom: 1px
        // solid border-dim }` + `.ds-panel__title { font-weight: 600 }`.
        (
            "panel.header",
            spec(
                d().border_edge(tint(ToneRef::Border, 90), BorderWidthTier::Std, EdgesRef::Bottom)
                    .py(PadTier::Px(8.0))
                    .weight(600),
            ),
        ),
        // `.ds-pane-header .ds-btn--chrome { border-radius: pill }` +
        // `.is-active { background: #1a1612; color: accent }`.
        (
            "nav.cluster.active",
            spec(d().radius(RadiusTier::Pill))
                .on_select(d().fill(tone(ToneRef::Bg)).ink(ToneRef::Accent)),
        ),
    ])
}

// ── Cadence ──────────────────────────────────────────────────────────────────
//
// global.css 838–892, 1639–1662, 1745–1787 + the `[data-ds="cadence"]` token
// block (383–411). Signature: Spotify — FULL-PILL primaries with an inset
// highlight, flush square rows, hairline alpha-white borders, green tints.
//   --ds-control-radius: pill               --ds-pill-radius: pill
//   --ds-card-radius:    radius-lg (14px)   --ds-card-pad: 16px

fn cadence() -> RecipeSet {
    set(vec![
        // `.ds-btn--primary { border-radius: pill; box-shadow: inset 0 1px 0
        // rgba(255,255,255,.10), 0 2px 6px rgba(0,0,0,.4); font-weight: 700 }`
        // — THE Cadence signature, now fully authored: pill + top highlight +
        // bold face. The second (drop) shadow is still unmappable.
        (
            "button.primary",
            spec(
                d().radius(RadiusTier::Pill)
                    .fill(tone(ToneRef::Accent))
                    .ink(ToneRef::Bg)
                    .px(PadTier::Lg)
                    .bevel_raised(tint(ToneRef::Text, 26), nothing())
                    .weight(700),
            )
            .on_hover(d().fill(shade(ToneRef::Accent, ShadeRef::S400))),
        ),
        // `button.action` — SHAPE ONLY, for the big block controls in a
        // trading action row (DOM BUY / SELL / FLATTEN / CANCEL).
        //
        // Radius and nothing else. The first version of this key mirrored
        // `button.primary` in full, including `.fill(Accent)` — which promptly
        // repainted FLATTEN (neutral) and CANCEL (soft red) in accent orange
        // and collapsed the row's semantics, the exact defect that BUY-and-SELL
        // -are-the-same-colour was. A CONTEXT key must not decide TONE; the
        // variant does that.
        //
        // Why it exists: `button.primary`'s `Pill` resolves to min(w, h) / 2,
        // correct in general and a full ELLIPSE on a ~64x52 control. These four
        // used to escape that with a hardcoded `corner_radius_asymmetric` per
        // call site, which pinned the shape. Now each style picks it.
        (
            "button.action",
            spec(d().radius(RadiusTier::Lg)),
        ),
        // ── Form controls ────────────────────────────────────────────────
        // `input` / `select` / `checkbox` share this system's control radius.
        // Radius ONLY: fill and border stay with the widget, which already
        // computes them from state (focus, invalid, disabled, hover). A recipe
        // that also set them would flatten those states — the same mistake
        // `button.action` made on its first pass.
        (
            "input",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "select",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "checkbox",
            spec(d().radius(RadiusTier::Md)),
        ),
        // ── Surfaces & compound controls ─────────────────────────────────
        // `popover` (context menus + tool popovers), `segmented` (the trough),
        // `switch` (the track). Radius only — fills encode STATE and stay with
        // the widget. `switch` keeps a pill everywhere: a track is a capsule by
        // definition, and squaring it would read as a broken toggle rather than
        // a restyled one.
        (
            "popover",
            spec(d().radius(RadiusTier::Lg)),
        ),
        (
            "segmented",
            spec(d().radius(RadiusTier::Lg)),
        ),
        (
            "switch",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // ── Feedback surfaces & meters ───────────────────────────────────
        // `alert` / `tooltip` are surfaces; `badge` / `progress` are meters and
        // stay PILL everywhere (a badge and a progress track are capsules by
        // definition — squaring them reads as broken, not restyled).
        // Radius only in every case: tone and fill encode state.
        (
            "alert",
            spec(d().radius(RadiusTier::Lg)),
        ),
        (
            "tooltip",
            spec(d().radius(RadiusTier::Lg)),
        ),
        (
            "badge",
            spec(d().radius(RadiusTier::Pill)),
        ),
        (
            "progress",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `slider` — the track. Pill like `progress` and `switch`; its own
        // key because a Slider is interactive and carries a thumb, so a style
        // may want to separate the two.
        (
            "slider",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `.ds-btn--secondary { background: bg-surface; border-radius:
        // radius-md; box-shadow: inset 0 1px 0 rgba(255,255,255,0.04) }`.
        (
            "button.ghost",
            spec(
                d().radius(RadiusTier::Md)
                    .fill(tone(ToneRef::Surface))
                    .bevel_raised(tint(ToneRef::Text, 10), nothing()),
            ),
        ),
        // `.ds-btn--chrome.is-active { background: bg-surface; color: fg;
        // border-bottom: 2px solid accent; box-shadow: inset 0 -1px 0
        // rgba(0,0,0,0.3) }` — an underline PLUS a sunken bottom line.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Pill)).on_select(
                d().fill(tone(ToneRef::Surface))
                    .ink(ToneRef::Text)
                    .border_edge(tone(ToneRef::Accent), BorderWidthTier::Px(2.0), EdgesRef::Bottom)
                    .bevel_marker(tint(ToneRef::Bg, 77), 1.0),
            ),
        ),
        // `.ds-tab--underline.is-selected { border-bottom-width: 3px }` — the
        // thickest underline of any style, and now authored as an actual
        // bottom-only edge rather than a four-sided box.
        (
            "tab.line.active",
            spec(d()).on_select(
                d().fill(tone(ToneRef::Accent))
                    .border_edge(tone(ToneRef::Accent), BorderWidthTier::Px(3.0), EdgesRef::Bottom),
            ),
        ),
        // `.ds-pane-tab.is-active { box-shadow: inset 0 -2px 0 accent }` — the
        // green marker under the active symbol tab.
        (
            "tab.pill.active",
            spec(d()).on_select(d().bevel_marker(tone(ToneRef::Accent), 2.0)),
        ),
        // `.ds-pill { border-radius: pill; text-transform: uppercase;
        // font-weight: 700 }` + `.ds-pill.is-filled { box-shadow: inset 0 1px 0
        // rgba(255,255,255,0.06) }` + `.ds-ind-chip { background: bg-surface;
        // border: 1px solid border }`.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Pill)
                    .fill(tone(ToneRef::Surface))
                    .border(tint(ToneRef::Text, 18), BorderWidthTier::Std)
                    .px(PadTier::Px(10.0))
                    .bevel_raised(tint(ToneRef::Text, 15), nothing())
                    .weight(700),
            ),
        ),
        // `--ds-card-radius: radius-lg` + `--ds-card-pad: 16px` +
        // `--ds-card-border: 1px solid border` (alpha-white hairline).
        (
            "card",
            spec(
                d().radius(RadiusTier::Lg)
                    .px(PadTier::Px(16.0))
                    .py(PadTier::Px(16.0))
                    .border(tint(ToneRef::Text, 18), BorderWidthTier::Std),
            ),
        ),
        // `.ds-panel__header { background: var(--ds-bg); border-bottom: 1px
        // solid rgba(255,255,255,0.04); height: 32px }` + `.ds-panel__title
        // { font-weight: 700 }` — the header sits on the PURE BLACK canvas,
        // not on the panel surface. Cadence-specific.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Bg))
                    .border_edge(tint(ToneRef::Text, 10), BorderWidthTier::Std, EdgesRef::Bottom)
                    .py(PadTier::Px(6.0))
                    .weight(700),
            ),
        ),
        // Flush rows (no gaps, no radius) — the Spotify list read — plus
        // `.ds-wl-sym { font-weight: 700 }`.
        (
            "row.list",
            spec(d().radius(RadiusTier::None).px(PadTier::Px(8.0)).no_border().weight(700)),
        ),
        // `.ds-wl-row:hover { background: color-mix(accent 8%, transparent) }`.
        (
            "row.list.hover",
            spec(d()).on_hover(d().fill(tint(ToneRef::Accent, 20))),
        ),
        // `.ds-wl-section { background: color-mix(accent 6%, bg-surface);
        // border-top: 1px solid border }` + `.ds-wl-section span
        // { font-weight: 700 }`.
        (
            "section.header.fill",
            spec(
                d().fill(tint(ToneRef::Accent, 16))
                    .py(PadTier::Px(6.0))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Top)
                    .weight(700),
            ),
        ),
        // `.ds-pane-header .ds-btn--chrome.is-active { background:
        // color-mix(accent 16%, transparent); color: accent }`.
        (
            "nav.cluster.active",
            spec(d().radius(RadiusTier::Pill))
                .on_select(d().fill(tint(ToneRef::Accent, 40)).ink(ToneRef::Accent)),
        ),
    ])
}

// ── Alto ─────────────────────────────────────────────────────────────────────
//
// global.css 1101–1123, 1160–1260 + the `[data-ds="alto"]` token block
// (261–279). Signature: Zed warm-dark — sharp 4px controls, raised bevel on
// button faces (inset top highlight + bottom shadow), sunken "well" inputs,
// ledger hairlines under every row.
//   --ds-control-radius: radius-sm (4px)    --ds-pill-radius: radius-sm
//   --ds-card-radius:    radius-md (6px)    --ds-card-pad: 14px
//
// Alto's warm highlight `rgba(255,238,210, a)` is the palette-agnostic
// `Tone::Text` at alpha `a`; `rgba(0,0,0, a)` is `Tone::Bg` at alpha `a`.

fn alto() -> RecipeSet {
    set(vec![
        // `.ds-btn--primary { box-shadow: inset 0 1px 0 rgba(255,255,255,.18),
        // inset 0 -1px 0 rgba(0,0,0,.25) }` — the raised face, now authored.
        (
            "button.primary",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Accent))
                    .bevel_raised(tint(ToneRef::Text, 46), tint(ToneRef::Bg, 64)),
            ),
        ),
        // `button.action` — SHAPE ONLY, for the big block controls in a
        // trading action row (DOM BUY / SELL / FLATTEN / CANCEL).
        //
        // Radius and nothing else. The first version of this key mirrored
        // `button.primary` in full, including `.fill(Accent)` — which promptly
        // repainted FLATTEN (neutral) and CANCEL (soft red) in accent orange
        // and collapsed the row's semantics, the exact defect that BUY-and-SELL
        // -are-the-same-colour was. A CONTEXT key must not decide TONE; the
        // variant does that.
        //
        // Why it exists: `button.primary`'s `Pill` resolves to min(w, h) / 2,
        // correct in general and a full ELLIPSE on a ~64x52 control. These four
        // used to escape that with a hardcoded `corner_radius_asymmetric` per
        // call site, which pinned the shape. Now each style picks it.
        (
            "button.action",
            spec(d().radius(RadiusTier::Sm)),
        ),
        // ── Form controls ────────────────────────────────────────────────
        // `input` / `select` / `checkbox` share this system's control radius.
        // Radius ONLY: fill and border stay with the widget, which already
        // computes them from state (focus, invalid, disabled, hover). A recipe
        // that also set them would flatten those states — the same mistake
        // `button.action` made on its first pass.
        (
            "input",
            spec(d().radius(RadiusTier::Sm)),
        ),
        (
            "select",
            spec(d().radius(RadiusTier::Sm)),
        ),
        (
            "checkbox",
            spec(d().radius(RadiusTier::Sm)),
        ),
        // ── Surfaces & compound controls ─────────────────────────────────
        // `popover` (context menus + tool popovers), `segmented` (the trough),
        // `switch` (the track). Radius only — fills encode STATE and stay with
        // the widget. `switch` keeps a pill everywhere: a track is a capsule by
        // definition, and squaring it would read as a broken toggle rather than
        // a restyled one.
        (
            "popover",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "segmented",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "switch",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // ── Feedback surfaces & meters ───────────────────────────────────
        // `alert` / `tooltip` are surfaces; `badge` / `progress` are meters and
        // stay PILL everywhere (a badge and a progress track are capsules by
        // definition — squaring them reads as broken, not restyled).
        // Radius only in every case: tone and fill encode state.
        (
            "alert",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "tooltip",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "badge",
            spec(d().radius(RadiusTier::Pill)),
        ),
        (
            "progress",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `slider` — the track. Pill like `progress` and `switch`; its own
        // key because a Slider is interactive and carries a thumb, so a style
        // may want to separate the two.
        (
            "slider",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `.ds-btn--secondary { background: linear-gradient(bg-elevated,
        // bg-surface); border: 1px solid border; box-shadow:
        // inset 0 1px 0 rgba(255,238,210,.06), inset 0 -1px 0 rgba(0,0,0,.35) }`,
        // `:hover { → elevated }`. The gradient collapses to its lower stop;
        // hover lifts one ramp step.
        (
            "button.ghost",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std)
                    .bevel_raised(tint(ToneRef::Text, 15), tint(ToneRef::Bg, 89)),
            )
            .on_hover(d().fill(shade(ToneRef::Surface, ShadeRef::S400))),
        ),
        // `.ds-btn--chrome.is-active { background: color-mix(accent 12%,
        // bg-surface) }` — the ambient amber wash.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Sm)).on_select(d().fill(tint(ToneRef::Accent, 30))),
        ),
        // `.ds-tab--underline.is-selected { background: bg-surface; box-shadow:
        // inset 0 1px 0 rgba(255,238,210,.04) }` — a beveled face, not a bar.
        (
            "tab.line.active",
            spec(d()).on_select(
                d().fill(tone(ToneRef::Accent))
                    .bevel_raised(tint(ToneRef::Text, 10), nothing()),
            ),
        ),
        // `.ds-tab--inline.is-selected { border: 1px solid border; box-shadow:
        // inset 0 0.5px 0 rgba(255,238,210,.08), inset 0 -0.5px 0
        // rgba(0,0,0,.45) }`. NOTE: `BevelSpecRef` carries ONE width, so the
        // 0.5px pair collapses onto the 1px hairline.
        (
            "tab.pill",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Surface))
                    .bevel_raised(tint(ToneRef::Text, 20), tint(ToneRef::Bg, 115)),
            ),
        ),
        // `.ds-pane-tab.is-active { box-shadow: inset 0 -2px 0 accent }` — the
        // amber marker under the active symbol tab. A single inset line, which
        // is exactly what `bevel_marker` spells.
        (
            "tab.pill.active",
            spec(d()).on_select(d().bevel_marker(tone(ToneRef::Accent), 2.0)),
        ),
        // `.ds-ind-chip { border-radius: radius-xs; border: 1px solid border;
        // font-size: font-xs; box-shadow: inset 0 0.5px 0 rgba(255,238,210,.06),
        // inset 0 -0.5px 0 rgba(0,0,0,.40) }` — a raised card face, not a pill.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Xs)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std)
                    .text_size(TextSizeTier::Xs)
                    .bevel_raised(tint(ToneRef::Text, 15), tint(ToneRef::Bg, 102)),
            ),
        ),
        // `--ds-card-radius: radius-md` + `--ds-card-pad: 14px` + the Zed
        // bevel border.
        (
            "card",
            spec(
                d().radius(RadiusTier::Md)
                    .px(PadTier::Px(14.0))
                    .py(PadTier::Px(14.0))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
            ),
        ),
        // `.ds-dom-row { border-bottom: 1px solid rgba(255,234,210,0.04) }` —
        // the ledger hairline under every row (bottom edge ONLY, which is what
        // makes it read as a ledger rather than a grid). Plus `.ds-wl-sym
        // { font-weight: 500 }`.
        (
            "row.list",
            spec(
                d().radius(RadiusTier::None)
                    .border_edge(tint(ToneRef::Text, 10), BorderWidthTier::Hair, EdgesRef::Bottom)
                    .weight(500),
            ),
        ),
        // `.ds-dom-row.is-current { background: linear-gradient(… accent 12% …);
        // border-top: 1px solid rgba(217,152,88,.22); border-bottom: 1px solid
        // rgba(217,152,88,.22) }` + `.ds-dom-price { font-weight: 600 }`.
        // NOTE: one edge selection per border — the band's TOP rule is dropped
        // and the bottom kept, so consecutive rows still show a divider.
        (
            "row.list.selected",
            spec(d()).on_select(
                d().fill(tint(ToneRef::Accent, 31))
                    .border_edge(tint(ToneRef::Accent, 56), BorderWidthTier::Hair, EdgesRef::Bottom)
                    .weight(600),
            ),
        ),
        // `.ds-panel__header { background: linear-gradient(bg-elevated,
        // bg-surface); box-shadow: inset 0 -1px 0 rgba(0,0,0,.35) }` +
        // `.ds-pane-header { border-bottom: 1px solid border }`.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Bottom)
                    .bevel_marker(tint(ToneRef::Bg, 89), 1.0),
            ),
        ),
        // `.ds-wl-section { background: color-mix(accent 6%, bg-surface);
        // border-top: 1px solid border }` + `span { font-weight: 700 }`.
        (
            "section.header.fill",
            spec(
                d().fill(tint(ToneRef::Accent, 16))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Top)
                    .weight(700),
            ),
        ),
        // `.ds-pane-header .ds-btn--chrome.is-active { background:
        // color-mix(accent 16%, transparent); color: accent }`.
        (
            "nav.cluster.active",
            spec(d().radius(RadiusTier::Sm))
                .on_select(d().fill(tint(ToneRef::Accent, 40)).ink(ToneRef::Accent)),
        ),
        // `.ds-toolbar { background: linear-gradient(bg-elevated, bg-panel);
        // border-bottom: 1px solid border-dim; box-shadow: inset 0 1px 0
        // rgba(255,238,210,.06), 0 1px 0 rgba(0,0,0,.40) }`. The outset half of
        // that shadow is still unmappable; the inset highlight is authored.
        (
            "toolnav",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Bottom)
                    .bevel_raised(tint(ToneRef::Text, 15), nothing()),
            ),
        ),
    ])
}

// ── Mariner ──────────────────────────────────────────────────────────────────
//
// global.css 1126–1153, 1288–1402 + the `[data-ds="mariner"]` token block
// (320–345). Alto's sibling, not its clone: same Zed bones, cool-steel bevel
// highlights, ~10% tighter density (row-h 22 vs 24, toolbar-h 36 vs 40), and
// the accent used as a PRECISION MARKER (hard left edge on the current DOM
// row, 1.5px top stripe on the active pane) rather than an ambient wash.
//
// Mariner's cool highlight `rgba(180,210,240, a)` / `rgba(200,230,255, a)` is
// `Tone::Text` at alpha `a` — the same palette-agnostic spelling Alto uses for
// its warm one. The two styles differ in ALPHA, which is what the CSS says.

fn mariner() -> RecipeSet {
    set(vec![
        // `.ds-btn--primary { box-shadow: inset 0 1px 0 rgba(200,230,255,.22),
        // inset 0 -1px 0 rgba(0,0,0,.28) }` — a brighter, cooler highlight than
        // Alto's, and a shallower bottom shadow.
        (
            "button.primary",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Accent))
                    .bevel_raised(tint(ToneRef::Text, 56), tint(ToneRef::Bg, 71)),
            ),
        ),
        // `button.action` — SHAPE ONLY, for the big block controls in a
        // trading action row (DOM BUY / SELL / FLATTEN / CANCEL).
        //
        // Radius and nothing else. The first version of this key mirrored
        // `button.primary` in full, including `.fill(Accent)` — which promptly
        // repainted FLATTEN (neutral) and CANCEL (soft red) in accent orange
        // and collapsed the row's semantics, the exact defect that BUY-and-SELL
        // -are-the-same-colour was. A CONTEXT key must not decide TONE; the
        // variant does that.
        //
        // Why it exists: `button.primary`'s `Pill` resolves to min(w, h) / 2,
        // correct in general and a full ELLIPSE on a ~64x52 control. These four
        // used to escape that with a hardcoded `corner_radius_asymmetric` per
        // call site, which pinned the shape. Now each style picks it.
        (
            "button.action",
            spec(d().radius(RadiusTier::Sm)),
        ),
        // ── Form controls ────────────────────────────────────────────────
        // `input` / `select` / `checkbox` share this system's control radius.
        // Radius ONLY: fill and border stay with the widget, which already
        // computes them from state (focus, invalid, disabled, hover). A recipe
        // that also set them would flatten those states — the same mistake
        // `button.action` made on its first pass.
        (
            "input",
            spec(d().radius(RadiusTier::Sm)),
        ),
        (
            "select",
            spec(d().radius(RadiusTier::Sm)),
        ),
        (
            "checkbox",
            spec(d().radius(RadiusTier::Sm)),
        ),
        // ── Surfaces & compound controls ─────────────────────────────────
        // `popover` (context menus + tool popovers), `segmented` (the trough),
        // `switch` (the track). Radius only — fills encode STATE and stay with
        // the widget. `switch` keeps a pill everywhere: a track is a capsule by
        // definition, and squaring it would read as a broken toggle rather than
        // a restyled one.
        (
            "popover",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "segmented",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "switch",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // ── Feedback surfaces & meters ───────────────────────────────────
        // `alert` / `tooltip` are surfaces; `badge` / `progress` are meters and
        // stay PILL everywhere (a badge and a progress track are capsules by
        // definition — squaring them reads as broken, not restyled).
        // Radius only in every case: tone and fill encode state.
        (
            "alert",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "tooltip",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "badge",
            spec(d().radius(RadiusTier::Pill)),
        ),
        (
            "progress",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `slider` — the track. Pill like `progress` and `switch`; its own
        // key because a Slider is interactive and carries a thumb, so a style
        // may want to separate the two.
        (
            "slider",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `.ds-btn--secondary { box-shadow: inset 0 1px 0 rgba(180,210,240,.08),
        // inset 0 -1px 0 rgba(0,0,0,.38) }`, `:hover` lifts the highlight to
        // `.10`. Same bevel structure as Alto; tighter vertical padding.
        (
            "button.ghost",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std)
                    .py(PadTier::Px(3.0))
                    .bevel_raised(tint(ToneRef::Text, 20), tint(ToneRef::Bg, 97)),
            )
            .on_hover(
                d().fill(shade(ToneRef::Surface, ShadeRef::S400))
                    .bevel_raised(tint(ToneRef::Text, 26), tint(ToneRef::Bg, 97)),
            ),
        ),
        // `.ds-btn--chrome.is-active { background: color-mix(accent 14%,
        // bg-surface) }`.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Sm).px(PadTier::Px(8.0)))
                .on_select(d().fill(tint(ToneRef::Accent, 34))),
        ),
        // `.ds-tab--underline.is-selected { background: bg-surface; box-shadow:
        // none }` — Mariner explicitly drops Alto's bevel here, so NO bevel is
        // authored (an omitted field leaves the widget default alone).
        (
            "tab.line.active",
            spec(d()).on_select(d().fill(tone(ToneRef::Accent))),
        ),
        // `.ds-tab--inline.is-selected { box-shadow: inset 0 0.5px 0
        // rgba(180,210,240,.08), inset 0 -0.5px 0 rgba(0,0,0,.45) }`.
        (
            "tab.pill",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Surface))
                    .bevel_raised(tint(ToneRef::Text, 20), tint(ToneRef::Bg, 115)),
            ),
        ),
        // `.ds-pane-tab.is-active { box-shadow: inset 0 -2px 0 accent }` — the
        // steel-blue marker under the active symbol tab.
        (
            "tab.pill.active",
            spec(d()).on_select(d().bevel_marker(tone(ToneRef::Accent), 2.0)),
        ),
        // `.ds-ind-chip { box-shadow: inset 0 0.5px 0 rgba(180,210,240,.08),
        // inset 0 -0.5px 0 rgba(0,0,0,.40) }` — same raised face as Alto,
        // tighter padding, cooler highlight.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Xs)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std)
                    .px(PadTier::Px(6.0))
                    .text_size(TextSizeTier::Xs)
                    .bevel_raised(tint(ToneRef::Text, 20), tint(ToneRef::Bg, 102)),
            ),
        ),
        // Same radius ramp as Alto, ~10% tighter card padding (14 → 12).
        (
            "card",
            spec(
                d().radius(RadiusTier::Md)
                    .px(PadTier::Px(12.0))
                    .py(PadTier::Px(12.0))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
            ),
        ),
        // `.ds-wl-row { height: 24px }` (vs Alto's 28) + `.ds-dom-row
        // { border-bottom: 1px solid rgba(180,210,240,0.04) }` + `.ds-wl-sym
        // { font-weight: 500 }`.
        (
            "row.list",
            spec(
                d().radius(RadiusTier::None)
                    .py(PadTier::Px(2.0))
                    .border_edge(tint(ToneRef::Text, 10), BorderWidthTier::Hair, EdgesRef::Bottom)
                    .weight(500),
            ),
        ),
        // `.ds-dom-row.is-current { background: linear-gradient(accent 18% →
        // transparent); border-left: 2px solid accent }` + `.ds-dom-price
        // { font-weight: 600 }` — the instrument needle. The directional sweep
        // flattens to a flat tint; the LEFT edge is the whole point of the rule
        // and is now authored as such.
        (
            "row.list.selected",
            spec(d()).on_select(
                d().fill(tint(ToneRef::Accent, 46))
                    .border_edge(tone(ToneRef::Accent), BorderWidthTier::Px(2.0), EdgesRef::Left)
                    .weight(600),
            ),
        ),
        // `.ds-pane-header { border-bottom: 1px solid border; box-shadow:
        // inset 0 1px 0 rgba(180,210,240,.04), … }` and
        // `.ds-pane-header.is-active { border-top: 1.5px solid accent }` — the
        // nautical needle stripe, a TOP edge, which is what distinguishes it
        // from every other style's bottom underline.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Bottom)
                    .py(PadTier::Px(4.0))
                    .bevel_raised(tint(ToneRef::Text, 10), nothing()),
            )
            .on_select(d().border_edge(
                tone(ToneRef::Accent),
                BorderWidthTier::Px(1.5),
                EdgesRef::Top,
            )),
        ),
        // `.ds-wl-section { background: color-mix(accent 6%, bg-surface);
        // border-top: 1px solid border }` + `span { font-weight: 700 }`.
        (
            "section.header.fill",
            spec(
                d().fill(tint(ToneRef::Accent, 16))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Top)
                    .weight(700),
            ),
        ),
        (
            "nav.cluster.active",
            spec(d().radius(RadiusTier::Sm))
                .on_select(d().fill(tint(ToneRef::Accent, 40)).ink(ToneRef::Accent)),
        ),
        // `.ds-toolbar { background: linear-gradient(bg-elevated, bg-panel);
        // border-bottom: 1px solid border-dim; box-shadow: inset 0 1px 0
        // rgba(180,210,240,.06), 0 1px 0 rgba(0,0,0,.40) }`.
        (
            "toolnav",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Bottom)
                    .bevel_raised(tint(ToneRef::Text, 15), nothing()),
            ),
        ),
    ])
}

// ── Lucid ────────────────────────────────────────────────────────────────────
//
// global.css 901–932, 1476–1626, 1804–1807 + the `[data-ds="lucid"]` token
// block (450–464). Signature: editorial cream paper — gently rounded controls
// (5px), ink-fill primaries (there is no coloured button on paper), a hairline
// ring + soft drop on cards and NO inset bevel (light surfaces layer, they
// don't emboss — the ONE exception is the accent marker under the active pane
// tab, which is a marker rather than an emboss).
//   --ds-control-radius: radius-md (5px)    .ds-pill → radius-sm (3px)
//   --ds-card-radius:    radius-lg (8px)    --ds-card-pad: 20px

fn lucid() -> RecipeSet {
    set(vec![
        // `.ds-btn--primary { color: bg; background: fg; border: 1px solid fg }`
        // — ink block on cream. `Tone::Text` fill + `Tone::Bg` ink is the
        // palette-agnostic spelling and stays correct under a dark palette.
        (
            "button.primary",
            spec(
                d().radius(RadiusTier::Md)
                    .fill(tone(ToneRef::Text))
                    .ink(ToneRef::Bg)
                    .border(tone(ToneRef::Text), BorderWidthTier::Std),
            ),
        ),
        // `button.action` — SHAPE ONLY, for the big block controls in a
        // trading action row (DOM BUY / SELL / FLATTEN / CANCEL).
        //
        // Radius and nothing else. The first version of this key mirrored
        // `button.primary` in full, including `.fill(Accent)` — which promptly
        // repainted FLATTEN (neutral) and CANCEL (soft red) in accent orange
        // and collapsed the row's semantics, the exact defect that BUY-and-SELL
        // -are-the-same-colour was. A CONTEXT key must not decide TONE; the
        // variant does that.
        //
        // Why it exists: `button.primary`'s `Pill` resolves to min(w, h) / 2,
        // correct in general and a full ELLIPSE on a ~64x52 control. These four
        // used to escape that with a hardcoded `corner_radius_asymmetric` per
        // call site, which pinned the shape. Now each style picks it.
        (
            "button.action",
            spec(d().radius(RadiusTier::Md)),
        ),
        // ── Form controls ────────────────────────────────────────────────
        // `input` / `select` / `checkbox` share this system's control radius.
        // Radius ONLY: fill and border stay with the widget, which already
        // computes them from state (focus, invalid, disabled, hover). A recipe
        // that also set them would flatten those states — the same mistake
        // `button.action` made on its first pass.
        (
            "input",
            spec(d().radius(RadiusTier::Sm)),
        ),
        (
            "select",
            spec(d().radius(RadiusTier::Sm)),
        ),
        (
            "checkbox",
            spec(d().radius(RadiusTier::Sm)),
        ),
        // ── Surfaces & compound controls ─────────────────────────────────
        // `popover` (context menus + tool popovers), `segmented` (the trough),
        // `switch` (the track). Radius only — fills encode STATE and stay with
        // the widget. `switch` keeps a pill everywhere: a track is a capsule by
        // definition, and squaring it would read as a broken toggle rather than
        // a restyled one.
        (
            "popover",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "segmented",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "switch",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // ── Feedback surfaces & meters ───────────────────────────────────
        // `alert` / `tooltip` are surfaces; `badge` / `progress` are meters and
        // stay PILL everywhere (a badge and a progress track are capsules by
        // definition — squaring them reads as broken, not restyled).
        // Radius only in every case: tone and fill encode state.
        (
            "alert",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "tooltip",
            spec(d().radius(RadiusTier::Md)),
        ),
        (
            "badge",
            spec(d().radius(RadiusTier::Pill)),
        ),
        (
            "progress",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `slider` — the track. Pill like `progress` and `switch`; its own
        // key because a Slider is interactive and carries a thumb, so a style
        // may want to separate the two.
        (
            "slider",
            spec(d().radius(RadiusTier::Pill)),
        ),
        (
            "button.ghost",
            spec(d().radius(RadiusTier::Md).border(tone(ToneRef::Border), BorderWidthTier::Std)),
        ),
        // `.ds-btn--chrome { color: fg-dim }`, `:hover { background: bg-hover;
        // color: fg }`, `.is-active { background: fg; color: bg;
        // border-bottom: 2px solid accent }` — a terracotta underline, not a
        // terracotta box.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Md).ink(ToneRef::Dim))
                .on_hover(d().fill(tint(ToneRef::Text, 10)).ink(ToneRef::Text))
                .on_select(
                    d().fill(tone(ToneRef::Text))
                        .ink(ToneRef::Bg)
                        .border_edge(
                            tone(ToneRef::Accent),
                            BorderWidthTier::Px(2.0),
                            EdgesRef::Bottom,
                        ),
                ),
        ),
        // `.ds-tab--underline.is-selected { color: fg; border-bottom-color:
        // accent; background: transparent }`.
        (
            "tab.line.active",
            spec(d()).on_select(
                d().fill(tone(ToneRef::Accent))
                    .ink(ToneRef::Text)
                    .border_edge(tone(ToneRef::Accent), BorderWidthTier::Px(2.0), EdgesRef::Bottom),
            ),
        ),
        // `.ds-pane-header.is-active .ds-pane-tab.is-active { box-shadow:
        // inset 0 -2px 0 accent }` — the one inset line a light theme allows,
        // because it reads as a marker rather than an emboss.
        (
            "tab.pill.active",
            spec(d()).on_select(d().bevel_marker(tone(ToneRef::Accent), 2.0)),
        ),
        // `html[data-ds="lucid"] .ds-pill { border-radius: radius-sm }` — the
        // late rule that overrides `--ds-pill-radius: pill` from the token
        // block. Pills go quiet on paper.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
            ),
        ),
        // `--ds-card-radius: radius-lg` + `--ds-card-pad: 20px` +
        // `--ds-card-border: 1px solid border` + `--ds-card-shadow: 0 1px 2px …,
        // 0 6px 16px -8px …` (drop only, NO inset — the editorial paper card).
        (
            "card",
            spec(
                d().radius(RadiusTier::Lg)
                    .px(PadTier::Px(20.0))
                    .py(PadTier::Px(20.0))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
            ),
        ),
        (
            "card.floating",
            spec(
                d().radius(RadiusTier::Lg)
                    .px(PadTier::Px(20.0))
                    .py(PadTier::Px(20.0))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
            ),
        ),
        // `.ds-dom-row { border-bottom: 1px solid color-mix(border 40%,
        // transparent) }`.
        (
            "row.list",
            spec(
                d().radius(RadiusTier::Sm)
                    .border_edge(tint(ToneRef::Border, 100), BorderWidthTier::Hair, EdgesRef::Bottom),
            ),
        ),
        // `.ds-dom-row.is-current { background: color-mix(accent 14%, bg-panel);
        // border-top: 1px solid accent; border-bottom: 1px solid accent }` +
        // `.ds-dom-price { font-weight: 700 }`. One edge only — bottom kept.
        (
            "row.list.selected",
            spec(d()).on_select(
                d().fill(tint(ToneRef::Accent, 36))
                    .border_edge(tone(ToneRef::Accent), BorderWidthTier::Std, EdgesRef::Bottom)
                    .weight(700),
            ),
        ),
        // `.ds-wl-row:hover { background: color-mix(accent 8%, transparent) }`.
        (
            "row.list.hover",
            spec(d()).on_hover(d().fill(tint(ToneRef::Accent, 20))),
        ),
        // `.ds-wl-section { background: color-mix(accent 6%, bg-surface);
        // border-top: 1px solid border }` + `span { font-weight: 700 }`.
        (
            "section.header.fill",
            spec(
                d().fill(tint(ToneRef::Accent, 16))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Top)
                    .weight(700),
            ),
        ),
        // `.ds-panel__header { background: bg-surface; border-bottom: 1px solid
        // border; box-shadow: none }`.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Std, EdgesRef::Bottom),
            ),
        ),
        // `.ds-pane-header .ds-btn--chrome.is-active { color: accent;
        // background: color-mix(accent 12%, transparent); border-bottom: none }`
        // — the cluster button is the ONE chrome control that does NOT get the
        // terracotta underline.
        (
            "nav.cluster.active",
            spec(d().radius(RadiusTier::Md))
                .on_select(d().fill(tint(ToneRef::Accent, 30)).ink(ToneRef::Accent).no_border()),
        ),
    ])
}

// ── Meridien ─────────────────────────────────────────────────────────────────
//
// global.css 901–932, 942–1092, 1476–1636 + the `[data-ds="meridien"]` token
// block (506–535). Signature, stated verbatim in the CSS: "MONO · UPPERCASE ·
// SQUARE controls" — this is the single biggest differentiator from Lucid,
// which shares its palette exactly. Plus a magazine type scale and airier
// padding on every surface.
//   --ds-control-radius: 0px (pure square)  --ds-pill-radius: radius-xs (4px)
//   --ds-card-radius:    radius-md (10px)   --ds-card-pad: 22px
//
// NOTE: the mono/uppercase half of that signature is STILL not expressible
// (no font-family, no text-transform, no letter-spacing). What lands here is
// the SQUARE half, the airier scale, the editorial rules (per-side borders),
// the semibold editorial face, and the toolbar's 14px gap.

fn meridien() -> RecipeSet {
    set(vec![
        // `--ds-control-radius: 0` + `.ds-btn--primary { color: bg; background:
        // fg; border: 1px solid fg; font-size: font-sm }`.
        (
            "button.primary",
            spec(
                d().radius(RadiusTier::None)
                    .fill(tone(ToneRef::Text))
                    .ink(ToneRef::Bg)
                    .border(tone(ToneRef::Text), BorderWidthTier::Std)
                    .text_size(TextSizeTier::Sm),
            ),
        ),
        // `button.action` — SHAPE ONLY, for the big block controls in a
        // trading action row (DOM BUY / SELL / FLATTEN / CANCEL).
        //
        // Radius and nothing else. The first version of this key mirrored
        // `button.primary` in full, including `.fill(Accent)` — which promptly
        // repainted FLATTEN (neutral) and CANCEL (soft red) in accent orange
        // and collapsed the row's semantics, the exact defect that BUY-and-SELL
        // -are-the-same-colour was. A CONTEXT key must not decide TONE; the
        // variant does that.
        //
        // Why it exists: `button.primary`'s `Pill` resolves to min(w, h) / 2,
        // correct in general and a full ELLIPSE on a ~64x52 control. These four
        // used to escape that with a hardcoded `corner_radius_asymmetric` per
        // call site, which pinned the shape. Now each style picks it.
        (
            "button.action",
            spec(d().radius(RadiusTier::None)),
        ),
        // ── Form controls ────────────────────────────────────────────────
        // `input` / `select` / `checkbox` share this system's control radius.
        // Radius ONLY: fill and border stay with the widget, which already
        // computes them from state (focus, invalid, disabled, hover). A recipe
        // that also set them would flatten those states — the same mistake
        // `button.action` made on its first pass.
        (
            "input",
            spec(d().radius(RadiusTier::None)),
        ),
        (
            "select",
            spec(d().radius(RadiusTier::None)),
        ),
        (
            "checkbox",
            spec(d().radius(RadiusTier::None)),
        ),
        // ── Surfaces & compound controls ─────────────────────────────────
        // `popover` (context menus + tool popovers), `segmented` (the trough),
        // `switch` (the track). Radius only — fills encode STATE and stay with
        // the widget. `switch` keeps a pill everywhere: a track is a capsule by
        // definition, and squaring it would read as a broken toggle rather than
        // a restyled one.
        (
            "popover",
            spec(d().radius(RadiusTier::None)),
        ),
        (
            "segmented",
            spec(d().radius(RadiusTier::None)),
        ),
        (
            "switch",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // ── Feedback surfaces & meters ───────────────────────────────────
        // `alert` / `tooltip` are surfaces; `badge` / `progress` are meters and
        // stay PILL everywhere (a badge and a progress track are capsules by
        // definition — squaring them reads as broken, not restyled).
        // Radius only in every case: tone and fill encode state.
        (
            "alert",
            spec(d().radius(RadiusTier::None)),
        ),
        (
            "tooltip",
            spec(d().radius(RadiusTier::None)),
        ),
        (
            "badge",
            spec(d().radius(RadiusTier::Pill)),
        ),
        (
            "progress",
            spec(d().radius(RadiusTier::Pill)),
        ),
        // `slider` — the track. Pill like `progress` and `switch`; its own
        // key because a Slider is interactive and carries a thumb, so a style
        // may want to separate the two.
        (
            "slider",
            spec(d().radius(RadiusTier::Pill)),
        ),
        (
            "button.ghost",
            spec(d().radius(RadiusTier::None).border(tone(ToneRef::Border), BorderWidthTier::Std)),
        ),
        // `.ds-btn--chrome { padding: 0 14px; font-size: font-xs;
        // font-weight: 500 }`, `.is-active { background: fg; color: bg;
        // border-bottom: 2px accent; font-weight: 600 }`.
        (
            "button.chrome",
            spec(
                d().radius(RadiusTier::None)
                    .px(PadTier::Px(14.0))
                    .text_size(TextSizeTier::Xs)
                    .weight(500),
            )
            .on_select(
                d().fill(tone(ToneRef::Text))
                    .ink(ToneRef::Bg)
                    .border_edge(tone(ToneRef::Accent), BorderWidthTier::Px(2.0), EdgesRef::Bottom)
                    .weight(600),
            ),
        ),
        // `.ds-tab--underline { font-size: font-xs; padding: 0 14px }` — square
        // like every other Meridien control — plus `.ds-wl-header
        // .ds-tab--underline { font-weight: 600 }`.
        (
            "tab.line",
            spec(
                d().radius(RadiusTier::None)
                    .px(PadTier::Px(14.0))
                    .text_size(TextSizeTier::Xs)
                    .weight(600),
            ),
        ),
        // `.ds-tab--underline.is-selected { background: transparent; color: fg;
        // border-bottom-width: 2px }`.
        (
            "tab.line.active",
            spec(d().radius(RadiusTier::None)).on_select(
                d().fill(tone(ToneRef::Accent))
                    .ink(ToneRef::Text)
                    .border_edge(tone(ToneRef::Accent), BorderWidthTier::Px(2.0), EdgesRef::Bottom),
            ),
        ),
        // `.ds-pane-tab { padding: 0 14px 0 16px; font-size: font-base }` +
        // `.ds-pane-tab__sym { font-size: font-sm; font-weight: 700 }`.
        (
            "tab.pill",
            spec(
                d().radius(RadiusTier::None)
                    .px(PadTier::Px(14.0))
                    .text_size(TextSizeTier::Sm)
                    .weight(700),
            ),
        ),
        // `.ds-pane-header.is-active .ds-pane-tab.is-active { box-shadow:
        // inset 0 -2px 0 accent }`.
        (
            "tab.pill.active",
            spec(d()).on_select(d().bevel_marker(tone(ToneRef::Accent), 2.0)),
        ),
        // `--ds-pill-radius: radius-xs` (4px — "barely rounded") +
        // `.ds-pill { font-size: font-xs; padding: 0 10px }`.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Xs)
                    .px(PadTier::Px(10.0))
                    .text_size(TextSizeTier::Xs),
            ),
        ),
        // `--ds-card-radius: radius-md` + `--ds-card-pad: 22px` ("Meridien
        // breathes more than Lucid").
        //
        // RADIUS OVERRIDDEN vs that transcription, deliberately. `radius-md`
        // resolves to 6px on Meridien's scale, and a 6px corner contradicts the
        // system's own signature — MONO / UPPERCASE / SQUARE — which is what
        // `12-T5-CERTIFICATION` vouches for and what makes Meridien read as a
        // different product from Lucid despite sharing its palette. Squareness
        // is load-bearing here in a way the source rule did not account for.
        //
        // The PADDING stays at 22. The note above ("breathes more than Lucid",
        // which pads 20) is a deliberate relationship between two systems, and
        // nothing about squareness argues against it — dropping to 14 would
        // silently make Meridien the tightest editorial card, inverting the
        // source's intent on a point that was never in question.
        (
            "card",
            spec(
                d().radius(RadiusTier::None)
                    .px(PadTier::Px(22.0))
                    .py(PadTier::Px(22.0))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
            ),
        ),
        // `.ds-wl-row { height: 34px; padding: 0 10px 0 14px }` — the airiest
        // row of any style — plus `.ds-wl-sym { font-weight: 700 }` and the
        // shared `.ds-dom-row { border-bottom: 1px solid color-mix(border 40%,
        // transparent) }` ledger hairline.
        (
            "row.list",
            spec(
                d().radius(RadiusTier::None)
                    .px(PadTier::Px(12.0))
                    .py(PadTier::Px(8.0))
                    .border_edge(tint(ToneRef::Border, 100), BorderWidthTier::Hair, EdgesRef::Bottom)
                    .weight(700),
            )
            .on_hover(d().fill(tint(ToneRef::Text, 10))),
        ),
        // `.ds-dom-row.is-current { background: color-mix(accent 14%, bg-panel);
        // border-top: 1px solid accent; border-bottom: 1px solid accent }` +
        // `.ds-dom-price { font-weight: 700 }`. One edge only — bottom kept.
        (
            "row.list.selected",
            spec(d()).on_select(
                d().fill(tint(ToneRef::Accent, 36))
                    .border_edge(tone(ToneRef::Accent), BorderWidthTier::Std, EdgesRef::Bottom)
                    .weight(700),
            ),
        ),
        // `.ds-wl-section { padding: 10px 10px 5px; border-top: 2px solid
        // border; background: bg-surface }` + `.ds-wl-section span
        // { font-size: font-2xs; font-weight: 600 }`. The 2px TOP rule is the
        // editorial divider — it opens the section, it does not close it.
        (
            "section.header",
            spec(
                d().px(PadTier::Px(10.0))
                    .py(PadTier::Px(8.0))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Px(2.0), EdgesRef::Top)
                    .text_size(TextSizeTier::Xs)
                    .weight(600),
            ),
        ),
        ("section.header.fill", spec(d().fill(tone(ToneRef::Surface)))),
        // `.ds-panel__header { background: bg-surface; border-bottom: 2px solid
        // border; min-height: 38px; font-size: font-sm }`.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border_edge(tone(ToneRef::Border), BorderWidthTier::Px(2.0), EdgesRef::Bottom)
                    .py(PadTier::Px(10.0))
                    .text_size(TextSizeTier::Sm),
            ),
        ),
        // `.ds-toolbar { padding: 0 20px; gap: 14px }` — magazine proportions.
        // The gap is the whole reason Meridien's toolbar reads as editorial
        // rather than as a dense trading chrome bar.
        (
            "toolnav",
            spec(d().px(PadTier::Px(20.0)).py(PadTier::Px(10.0)).gap(PadTier::Px(14.0))),
        ),
        // `.ds-pane-header .ds-btn--chrome.is-active { color: accent;
        // background: color-mix(accent 12%, transparent); border-bottom: none }`.
        (
            "nav.cluster.active",
            spec(d().radius(RadiusTier::None))
                .on_select(d().fill(tint(ToneRef::Accent, 30)).ink(ToneRef::Accent).no_border()),
        ),
    ])
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::sx::style::{Sx, StyleState};
    use crate::ui_kit::widgets::theme::ComponentTheme;

    /// The six styles that carry authored recipes.
    const AUTHORED: [&str; 6] = ["aperture", "cadence", "alto", "mariner", "lucid", "meridien"];

    /// The three built-in styles that must stay byte-identical (empty set).
    const UNAUTHORED: [&str; 3] = ["octave", "relay", "glass"];

    /// Minimal `ComponentTheme` so recipes can be resolved without a GPU theme.
    struct MockTheme;

    impl ComponentTheme for MockTheme {
        fn accent(&self) -> egui::Color32 { egui::Color32::from_rgb(99, 102, 241) }
        fn bull(&self) -> egui::Color32 { egui::Color32::from_rgb(52, 211, 153) }
        fn bear(&self) -> egui::Color32 { egui::Color32::from_rgb(248, 113, 113) }
        fn warn(&self) -> egui::Color32 { egui::Color32::from_rgb(251, 191, 36) }
        fn text(&self) -> egui::Color32 { egui::Color32::from_rgb(220, 220, 220) }
        fn dim(&self) -> egui::Color32 { egui::Color32::from_rgb(120, 120, 120) }
        fn border(&self) -> egui::Color32 { egui::Color32::from_rgb(55, 55, 55) }
        fn border_variant(&self) -> egui::Color32 { egui::Color32::from_rgb(70, 70, 70) }
        fn surface(&self) -> egui::Color32 { egui::Color32::from_rgb(28, 28, 28) }
        fn bg(&self) -> egui::Color32 { egui::Color32::from_rgb(18, 18, 18) }
        fn element_hover(&self) -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12) }
        fn element_active(&self) -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20) }
        fn element_selected(&self) -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(99, 102, 241, 40) }
        fn element_disabled(&self) -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 6) }
        fn ghost_hover(&self) -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8) }
        fn ghost_active(&self) -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14) }
        fn icon(&self) -> egui::Color32 { egui::Color32::from_rgb(180, 180, 180) }
        fn icon_muted(&self) -> egui::Color32 { egui::Color32::from_rgb(100, 100, 100) }
        fn icon_disabled(&self) -> egui::Color32 { egui::Color32::from_rgb(60, 60, 60) }
        fn icon_accent(&self) -> egui::Color32 { egui::Color32::from_rgb(99, 102, 241) }
    }

    // ── (a) each authored style returns a non-empty set ──────────────────────

    #[test]
    fn authored_styles_return_non_empty_sets() {
        for id in AUTHORED {
            let s = builtin_recipes(id);
            assert!(
                !s.is_empty(),
                "style `{id}` must carry authored recipes — got an empty set"
            );
            assert!(
                s.len() >= 8,
                "style `{id}` should override at least 8 components to read as a \
                 distinct design system — got {}",
                s.len()
            );
        }
    }

    // ── (b) unknown / unauthored ids return an EMPTY set ─────────────────────

    #[test]
    fn unknown_id_returns_empty_set() {
        assert!(builtin_recipes("not-a-style").is_empty());
        assert!(builtin_recipes("").is_empty());
        // Case matters — ids are canonical lowercase.
        assert!(builtin_recipes("Aperture").is_empty());
    }

    #[test]
    fn unauthored_builtin_styles_stay_empty() {
        for id in UNAUTHORED {
            assert!(
                builtin_recipes(id).is_empty(),
                "style `{id}` has no authored recipes yet and MUST resolve to the \
                 widget defaults unchanged"
            );
        }
    }

    // ── (c) signature assertions ─────────────────────────────────────────────

    /// Cadence's single most recognisable rule:
    /// `html[data-ds="cadence"] .ds-btn--primary { border-radius: pill }`.
    #[test]
    fn cadence_button_primary_is_pill() {
        let set = builtin_recipes("cadence");
        let spec = set.get("button.primary").expect("cadence must author button.primary");
        assert!(
            matches!(spec.base.radius, Some(RadiusTier::Pill)),
            "cadence button.primary must be authored at the PILL tier, not a raw px \
             value — got {:?}",
            spec.base.radius
        );

        // And it must survive resolution over a widget default that is NOT a pill.
        let t = MockTheme;
        let resolved = set.resolve("button.primary", Sx::new().rounded_sm(), &t);
        assert_eq!(
            resolved.resolved(StyleState::Normal).radius,
            Some(RadiusTier::Pill.to_px()),
            "cadence button.primary must resolve to the pill radius"
        );
    }

    /// Meridien's stated signature: `--ds-control-radius: 0px /* pure square */`.
    #[test]
    fn meridien_controls_are_square() {
        let set = builtin_recipes("meridien");
        for key in ["button.primary", "button.ghost", "button.chrome", "tab.line"] {
            let spec = set.get(key).unwrap_or_else(|| panic!("meridien must author `{key}`"));
            assert!(
                matches!(spec.base.radius, Some(RadiusTier::None)),
                "meridien `{key}` must be square (RadiusTier::None) — got {:?}",
                spec.base.radius
            );
        }

        // Resolution proof: a rounded widget default is flattened to 0.
        let t = MockTheme;
        let resolved = set.resolve("button.primary", Sx::new().rounded_md(), &t);
        assert_eq!(
            resolved.resolved(StyleState::Normal).radius,
            Some(0.0),
            "meridien must flatten a rounded widget default to a square control"
        );
    }

    /// Aperture's `--ds-card-border: none` — flat colour-block tiles carry no
    /// stroke at all; the lift comes from the shadow (unmappable) and the
    /// big radius.
    #[test]
    fn aperture_card_has_no_border() {
        let set = builtin_recipes("aperture");
        let spec = set.get("card").expect("aperture must author card");

        let border = spec.base.border.as_ref().expect(
            "aperture card must state its border EXPLICITLY (width=None) — omitting \
             the field would inherit the widget's stroke instead of removing it",
        );
        assert!(
            matches!(border.width, Some(BorderWidthTier::None)),
            "aperture card border width must be the None tier — got {:?}",
            border.width
        );
        assert!(
            matches!(spec.base.radius, Some(RadiusTier::Lg)),
            "aperture card must use the big-radius (Lg) tile signature"
        );

        // Resolution proof: a widget default WITH a stroke ends up at width 0.
        let t = MockTheme;
        let default_sx = Sx::new()
            .rounded_sm()
            .border_thin(crate::ui_kit::sx::color::Tone::Border);
        let resolved = set.resolve("card", default_sx, &t);
        let delta = resolved.resolved(StyleState::Normal);
        assert_eq!(
            delta.border.map(|b| b.width),
            Some(0.0),
            "aperture card must erase the widget's default stroke"
        );
    }

    // ── (d) M3.4b — the newly-expressible vocabulary ─────────────────────────

    /// Per-side borders. `html[data-ds="cadence"] .ds-tab--underline.is-selected
    /// { border-bottom-width: 3px }` — the thickest underline of any style. If
    /// this ever resolves to all four edges, the tab grows a 3px BOX.
    #[test]
    fn cadence_tab_underline_is_a_bottom_edge_at_3px() {
        let set = builtin_recipes("cadence");
        let spec = set.get("tab.line.active").expect("cadence must author tab.line.active");
        let sel = spec.selected.as_ref().expect("the underline lives on the selected state");
        let border = sel.border.as_ref().expect("selected tab must carry a border");

        assert!(
            matches!(border.edges, EdgesRef::Bottom),
            "cadence's tab underline must paint the BOTTOM edge only — got {:?}",
            border.edges
        );
        assert!(
            matches!(border.width, Some(BorderWidthTier::Px(w)) if (w - 3.0).abs() < f32::EPSILON),
            "cadence's underline is 3px — the widest in the system — got {:?}",
            border.width
        );

        // Resolution proof: the edge selection survives into the render delta.
        let t = MockTheme;
        let resolved = set.resolve("tab.line.active", Sx::new(), &t);
        let edges = resolved.resolved(StyleState::Active).resolved_border_edges();
        assert!(
            edges.bottom && !edges.top && !edges.left && !edges.right,
            "the bottom-only selection must reach the render delta"
        );
    }

    /// Mariner's instrument needle is the counter-example that proves the edge
    /// axis is real: `.ds-dom-row.is-current { border-left: 2px solid accent }`
    /// — a LEFT edge where every other style uses a bottom one.
    #[test]
    fn mariner_current_row_marks_the_left_edge() {
        let set = builtin_recipes("mariner");
        let spec = set.get("row.list.selected").expect("mariner must author row.list.selected");
        let border = spec.selected.as_ref().unwrap().border.as_ref().unwrap();
        assert!(
            matches!(border.edges, EdgesRef::Left),
            "mariner's DOM needle is a LEFT edge — got {:?}",
            border.edges
        );

        // …and its active pane stripe is a TOP edge.
        let hdr = set.get("panel.header").unwrap();
        assert!(
            matches!(hdr.selected.as_ref().unwrap().border.as_ref().unwrap().edges, EdgesRef::Top),
            "mariner's active-pane stripe must be the TOP edge"
        );
    }

    /// Inset bevels. `html[data-ds="alto"] .ds-btn--primary { box-shadow:
    /// inset 0 1px 0 rgba(255,255,255,.18), inset 0 -1px 0 rgba(0,0,0,.25) }`
    /// — the Zed raised face. Both lines must be present, or the button reads
    /// flat instead of raised.
    #[test]
    fn alto_button_primary_is_a_raised_face() {
        let set = builtin_recipes("alto");
        let spec = set.get("button.primary").expect("alto must author button.primary");
        let bev = spec.base.bevel.as_ref().expect("alto's primary face must be beveled");
        assert!(bev.top.is_some(), "the raised face needs a TOP highlight line");
        assert!(bev.bottom.is_some(), "the raised face needs a BOTTOM shadow line");

        // Resolution proof: the bevel survives into the render delta.
        let t = MockTheme;
        let resolved = set.resolve("button.primary", Sx::new(), &t);
        assert!(
            resolved.resolved(StyleState::Normal).bevel_spec().is_some(),
            "the authored bevel must reach the render delta"
        );

        // Alto's tab marker is the ONE-sided form: `inset 0 -2px 0 accent`.
        let marker = set.get("tab.pill.active").expect("alto must author tab.pill.active");
        let mb = marker.selected.as_ref().unwrap().bevel.as_ref().unwrap();
        assert!(mb.top.is_none() && mb.bottom.is_some(), "a marker is bottom-only");
        assert_eq!(mb.width, 2.0, "the CSS says `inset 0 -2px 0`");
    }

    /// Font weight. `html[data-ds="cadence"] .ds-btn--primary
    /// { font-weight: 700 }` — Spotify's confident bold face.
    #[test]
    fn cadence_carries_its_bold_face() {
        let set = builtin_recipes("cadence");
        assert_eq!(
            set.get("button.primary").unwrap().base.weight,
            Some(700),
            "cadence's primary button is 700 in the CSS"
        );
        assert_eq!(
            set.get("tag").unwrap().base.weight,
            Some(700),
            "`.ds-pill` is font-weight 700 in the CSS"
        );
        assert_eq!(
            set.get("panel.header").unwrap().base.weight,
            Some(700),
            "`.ds-panel__title` is font-weight 700 in the CSS"
        );

        // Resolution proof: >= 600 renders `strong`.
        let t = MockTheme;
        let resolved = set.resolve("button.primary", Sx::new(), &t);
        assert_eq!(
            resolved.resolved(StyleState::Normal).is_strong(),
            Some(true),
            "700 must reach the render delta as a strong face"
        );
    }

    /// Gap. `html[data-ds="meridien"] .ds-toolbar { gap: 14px }` — the single
    /// property that makes Meridien's toolbar read as editorial rather than as
    /// a dense trading chrome bar.
    #[test]
    fn meridien_toolbar_carries_its_editorial_gap() {
        let set = builtin_recipes("meridien");
        let spec = set.get("toolnav").expect("meridien must author toolnav");
        assert!(
            matches!(spec.base.gap, Some(PadTier::Px(g)) if (g - 14.0).abs() < f32::EPSILON),
            "meridien's toolbar gap is 14px — got {:?}",
            spec.base.gap
        );

        let t = MockTheme;
        let resolved = set.resolve("toolnav", Sx::new(), &t);
        assert_eq!(resolved.resolved(StyleState::Normal).gap, Some(14.0));
    }

    // ── Guard-rails ──────────────────────────────────────────────────────────

    /// Every authored key must appear in `docs/migration/recipe-keys.md`.
    /// Authoring an unregistered key produces dead data — no widget reads it.
    #[test]
    fn every_authored_key_is_registered() {
        const REGISTERED: [&str; 40] = [
            "button.primary", "button.ghost", "button.danger", "button.success", "button.chrome",
            // Form controls — radius only; state colours stay with the widget.
            "input", "select", "checkbox",
            // Surfaces & compound controls — radius only.
            "popover", "segmented", "switch",
            // Feedback surfaces & meters — radius only.
            "alert", "tooltip", "badge", "progress", "slider",
            // `button.action` — large block controls in a trading action row.
            // Consumed via `Button::recipe_key("button.action")` (the DOM
            // BUY/SELL/FLATTEN/CANCEL row), not via a Variant, because it is a
            // CONTEXT ("this is an action row") rather than a semantic tone.
            "button.action",
            "tab.line", "tab.line.active", "tab.pill", "tab.pill.active",
            "row.list", "row.list.selected", "row.list.hover",
            "section.header", "section.header.fill",
            "nav.cluster", "nav.cluster.active",
            "panel.footer", "panel.header",
            "card", "card.floating",
            "toast", "toast.success", "toast.danger", "toast.warn",
            "tag", "kbd", "drag.handle", "toolnav",
        ];

        for id in AUTHORED {
            let set = builtin_recipes(id);
            for key in set.keys() {
                assert!(
                    REGISTERED.contains(&key),
                    "style `{id}` authors unregistered key `{key}` — add it to \
                     docs/migration/recipe-keys.md first, or the recipe is dead data"
                );
            }
        }
    }

    /// No recipe may hard-code a hex colour: the palette axis owns colour, and a
    /// literal would pin a style to one palette. Covers the M3.2 slots too —
    /// bevels are the most tempting place to paste `rgba(255,238,210,.06)`
    /// straight out of the CSS.
    #[test]
    fn no_recipe_hardcodes_a_hex_literal() {
        fn assert_no_literal(style: &str, key: &str, slot: &str, dlt: &RecipeDelta) {
            if let Some(c) = &dlt.fill {
                assert!(
                    !matches!(c, ColorSpec::Literal { .. }),
                    "{style}/{key}/{slot}: fill uses a hex literal — use a Tone instead"
                );
            }
            if let Some(b) = &dlt.border {
                assert!(
                    !matches!(b.color, ColorSpec::Literal { .. }),
                    "{style}/{key}/{slot}: border uses a hex literal — use a Tone instead"
                );
            }
            if let Some(b) = &dlt.bevel {
                for (line, c) in [("top", &b.top), ("bottom", &b.bottom)] {
                    if let Some(c) = c {
                        assert!(
                            !matches!(c, ColorSpec::Literal { .. }),
                            "{style}/{key}/{slot}: bevel {line} line uses a hex literal — \
                             Alto's warm highlight and Mariner's cool one are both \
                             `Tone::Text` at different alphas"
                        );
                    }
                }
            }
        }

        for id in AUTHORED {
            let set = builtin_recipes(id);
            for key in set.keys() {
                let spec = set.get(key).unwrap();
                assert_no_literal(id, key, "base", &spec.base);
                for (slot, dlt) in [
                    ("hover", &spec.hover),
                    ("active", &spec.active),
                    ("disabled", &spec.disabled),
                    ("selected", &spec.selected),
                ] {
                    if let Some(dlt) = dlt {
                        assert_no_literal(id, key, slot, dlt);
                    }
                }
            }
        }
    }

    /// Lucid and Meridien share a palette exactly — their recipe sets are the
    /// ONLY thing that keeps them apart. If these ever converge, one of the two
    /// styles has become redundant.
    #[test]
    fn lucid_and_meridien_diverge_structurally() {
        let l = builtin_recipes("lucid");
        let m = builtin_recipes("meridien");

        let l_radius = l.get("button.primary").unwrap().base.radius.clone();
        let m_radius = m.get("button.primary").unwrap().base.radius.clone();
        assert!(
            matches!(l_radius, Some(RadiusTier::Md)) && matches!(m_radius, Some(RadiusTier::None)),
            "lucid must be gently rounded and meridien square — got {l_radius:?} / {m_radius:?}"
        );

        let t = MockTheme;
        let lr = l.resolve("button.primary", Sx::new(), &t).resolved(StyleState::Normal).radius;
        let mr = m.resolve("button.primary", Sx::new(), &t).resolved(StyleState::Normal).radius;
        assert_ne!(lr, mr, "same-palette siblings must resolve to different geometry");
    }

    /// Alto and Mariner share the Zed bevel GEOMETRY but not its TEMPERATURE:
    /// the CSS gives Alto a warm 6%/18% highlight and Mariner a cool 8%/22% one.
    /// Same tone, different alpha — if the alphas ever match, one of the two
    /// bevels was copy-pasted rather than transcribed.
    #[test]
    fn alto_and_mariner_bevels_differ_in_intensity() {
        fn top_alpha(set: &RecipeSet, key: &str) -> u8 {
            match set.get(key).unwrap().base.bevel.as_ref().unwrap().top.as_ref().unwrap() {
                ColorSpec::Alpha { alpha, .. } => *alpha,
                other => panic!("bevel highlights must be tone+alpha — got {other:?}"),
            }
        }
        let a = builtin_recipes("alto");
        let m = builtin_recipes("mariner");
        assert_ne!(
            top_alpha(&a, "button.primary"),
            top_alpha(&m, "button.primary"),
            "Alto's warm highlight (.18) and Mariner's cool one (.22) are different \
             strengths in the CSS"
        );
    }

    /// Every authored spec must actually change something — an all-`None`
    /// delta with no state blocks is an accidental no-op. Includes the M3.2
    /// fields, so a bevel-only or weight-only recipe counts as real.
    #[test]
    fn no_authored_recipe_is_a_no_op() {
        for id in AUTHORED {
            let set = builtin_recipes(id);
            for key in set.keys() {
                let s = set.get(key).unwrap();
                let base_empty = s.base.radius.is_none()
                    && s.base.px.is_none()
                    && s.base.py.is_none()
                    && s.base.fill.is_none()
                    && s.base.border.is_none()
                    && s.base.text.is_none()
                    && s.base.text_size.is_none()
                    && s.base.bevel.is_none()
                    && s.base.weight.is_none()
                    && s.base.gap.is_none()
                    && s.base.opacity.is_none();
                let no_states = s.hover.is_none()
                    && s.active.is_none()
                    && s.disabled.is_none()
                    && s.selected.is_none();
                assert!(
                    !(base_empty && no_states),
                    "{id}/{key} is an empty recipe — it would resolve to the widget \
                     default, so it should be deleted rather than shipped"
                );
            }
        }
    }
}

#[cfg(test)]
mod card_duplication_tests {
    use super::*;

    /// TWO MECHANISMS DESCRIBE A CARD, and they disagree.
    ///
    /// - `StyleSystem.card: Option<CardRecipe>` (M1 Change D) — radius,
    ///   padding, border_width. **Consumed** by `ui_kit/widgets/panel_card.rs`.
    /// - the `"card"` recipe key — radius, px/py padding, border. Authored by
    ///   all six styles and **consumed by nothing**.
    ///
    /// This is the architecture audit's headline defect ("10 sources of truth")
    /// reproduced in miniature, and DS-6.0 D4 made it worse: I authored
    /// `CardRecipe` data for four styles without noticing the recipe key
    /// already carried per-theme data for all six.
    ///
    /// They agree on Aperture (radius 20 / pad 16 / no border) because both
    /// were derived from the same brief. They disagree on Meridien — the
    /// recipe says `RadiusTier::Md` (6) with 22px padding, the CardRecipe says
    /// square (0) with 14px — and neither is obviously wrong: the recipe was
    /// transcribed from the React `[data-ds]` rules, the CardRecipe followed
    /// Meridien's stated MONO/UPPERCASE/SQUARE identity.
    ///
    /// Resolving that needs the design reference, not a guess, so this test
    /// does not pick a winner. It PINS the situation: every style that
    /// authors a `card` recipe is listed, so the duplication stays visible and
    /// a new one cannot be added silently while the decision is outstanding.
    ///
    /// When the decision lands: make `panel_card` consult the recipe (the
    /// designed mechanism, authored by all six) with `CardRecipe` as fallback,
    /// delete the loser, and delete this test.
    #[test]
    fn card_is_described_by_two_competing_mechanisms() {
        let styles = ["aperture", "cadence", "alto", "mariner", "lucid", "meridien"];
        let with_recipe: Vec<&str> = styles
            .iter()
            .copied()
            .filter(|s| builtin_recipes(s).get("card").is_some())
            .collect();
        assert_eq!(
            with_recipe, styles,
            "all six styles author a `card` recipe that nothing consumes; if this \
             set changed, the duplication was touched — see this test's docs"
        );
    }
}

#[cfg(test)]
mod action_key_tests {
    use super::*;

    /// `button.action` must be authored by EVERY style and must carry a radius.
    ///
    /// The key exists so the DOM action row's corner treatment is a per-style
    /// decision instead of a hardcoded `corner_radius_asymmetric` at four call
    /// sites. If a style stops authoring it, those buttons silently fall back
    /// to `button.primary`'s `Pill` — which is what made them ellipses.
    #[test]
    fn every_style_authors_an_action_radius() {
        for id in ["aperture","cadence","alto","mariner","lucid","meridien"] {
            let set = builtin_recipes(id);
            let spec = set.get("button.action")
                .unwrap_or_else(|| panic!("style `{id}` does not author button.action"));
            let d = &spec.base;
            assert!(
                d.radius.is_some(),
                "style `{id}` authors button.action without a radius — the key \
                 exists ONLY to carry that decision"
            );
        }
    }

    /// It must carry SHAPE ONLY. The first version mirrored `button.primary`
    /// including `.fill(Accent)`, which repainted FLATTEN (neutral) and CANCEL
    /// (soft red) in accent orange — collapsing the row's semantics exactly the
    /// way BUY-and-SELL-are-the-same-colour did. A context key decides shape;
    /// the variant decides tone.
    #[test]
    fn action_key_never_decides_tone() {
        for id in ["aperture","cadence","alto","mariner","lucid","meridien"] {
            let set = builtin_recipes(id);
            if let Some(spec) = set.get("button.action") {
                let d = &spec.base;
                assert!(d.fill.is_none(), "style `{id}`: button.action must not set a fill");
                assert!(d.text.is_none(), "style `{id}`: button.action must not set ink");
            }
        }
    }
}
