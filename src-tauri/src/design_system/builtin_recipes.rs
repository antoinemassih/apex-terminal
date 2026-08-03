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
//!    and light palettes.
//! 3. **Tiers over pixels.** `RadiusTier::Pill` / `PadTier::Md` track the active
//!    style's ramp. `Px(_)` is reserved for values the CSS states literally and
//!    that no tier expresses (e.g. Aperture's 1.5px outline, Cadence's 3px tab
//!    underline).
//!
//! # What is intentionally NOT here
//!
//! CSS properties with no representation in [`RecipeSpec`] are skipped rather
//! than approximated. The running list (input to the M3.2 Sx-vocabulary
//! ticket): `font-weight`, `font-family`, `text-transform`, `letter-spacing`,
//! `box-shadow` (both inset bevels and drop shadows), `linear-gradient` fills,
//! `filter: brightness`, `transform: translateY`, fixed `height`, per-side
//! borders (`border-bottom` / `border-left` / `border-top`), and `min-width`.
//!
//! # Coverage
//!
//! Six of the nine built-in styles are authored: `aperture`, `cadence`,
//! `alto`, `mariner`, `lucid`, `meridien`. The remaining three (`octave`,
//! `relay`, `glass`) return an **empty** set — an empty set resolves to the
//! widget's built-in `Sx` unchanged, so they are byte-identical to today.

use super::recipes::RecipeSet;
use crate::ui_kit::sx::recipe_spec::{
    BorderSpecRef, BorderWidthTier, ColorSpec, PadTier, RadiusTier, RecipeDelta, RecipeSpec,
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
}

impl DeltaExt for RecipeDelta {
    fn radius(mut self, r: RadiusTier) -> Self { self.radius = Some(r); self }
    fn px(mut self, p: PadTier) -> Self { self.px = Some(p); self }
    fn py(mut self, p: PadTier) -> Self { self.py = Some(p); self }
    fn fill(mut self, c: ColorSpec) -> Self { self.fill = Some(c); self }
    fn border(mut self, c: ColorSpec, w: BorderWidthTier) -> Self {
        self.border = Some(BorderSpecRef { color: c, width: Some(w) });
        self
    }
    fn no_border(mut self) -> Self {
        self.border = Some(BorderSpecRef {
            color: tint(ToneRef::Border, 0),
            width: Some(BorderWidthTier::None),
        });
        self
    }
    fn ink(mut self, t: ToneRef) -> Self { self.text = Some(TextSpec { tone: t, shade: None }); self }
    fn text_size(mut self, s: TextSizeTier) -> Self { self.text_size = Some(s); self }
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
        // accent; color: #1a1612 }` — orange block, canvas-dark ink.
        (
            "button.primary",
            spec(d().radius(RadiusTier::Pill).fill(tone(ToneRef::Accent)).ink(ToneRef::Bg).px(PadTier::Lg))
                .on_hover(d().fill(shade(ToneRef::Accent, ShadeRef::S400))),
        ),
        // `.ds-btn--secondary { border: 1.5px solid border; background:
        // transparent }`, `.is-active` → inverted block.
        (
            "button.ghost",
            spec(d().radius(RadiusTier::Pill).border(tone(ToneRef::Border), BorderWidthTier::Px(1.5)))
                .on_select(
                    d().fill(tone(ToneRef::Text))
                        .ink(ToneRef::Bg)
                        .border(tone(ToneRef::Text), BorderWidthTier::Px(1.5)),
                ),
        ),
        // `.ds-btn--chrome { padding: 0 14px }`, `.is-active { background: fg;
        // color: bg }` — the Aperture inverted-block signature.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Pill).px(PadTier::Px(14.0)))
                .on_select(d().fill(tone(ToneRef::Text)).ink(ToneRef::Bg)),
        ),
        // `.ds-tab--underline { border-radius: pill; border-bottom: none;
        // padding: 0 14px }` — tabs stop being tabs and become pills.
        (
            "tab.line",
            spec(d().radius(RadiusTier::Pill).px(PadTier::Px(14.0)).no_border()),
        ),
        // `.ds-tab--underline.is-selected { background: fg; color: bg }`.
        (
            "tab.line.active",
            spec(d().radius(RadiusTier::Pill))
                .on_select(d().fill(tone(ToneRef::Text)).ink(ToneRef::Bg)),
        ),
        // `.ds-pill { border-radius: pill; border-width: 1.5px }`,
        // `.ds-pill--md { padding: 0 14px }`.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Pill)
                    .border(tone(ToneRef::Border), BorderWidthTier::Px(1.5))
                    .px(PadTier::Px(10.0)),
            ),
        ),
        // `--ds-card-radius: radius-lg` + `--ds-card-border: none` +
        // `--ds-card-pad: 16px`. The borderless block-colour tile.
        (
            "card",
            spec(d().radius(RadiusTier::Lg).no_border().px(PadTier::Px(16.0)).py(PadTier::Px(16.0))),
        ),
        // `.ds-wl-row { border-radius: pill; padding: 0 10px; border-bottom:
        // none }`, `:hover { background: rgba(255,255,255,0.05) }`.
        (
            "row.list",
            spec(d().radius(RadiusTier::Pill).px(PadTier::Px(10.0)).no_border())
                .on_hover(d().fill(tint(ToneRef::Text, 13))),
        ),
        (
            "row.list.hover",
            spec(d()).on_hover(d().fill(tint(ToneRef::Text, 13))),
        ),
        // `.ds-panel__header { background: transparent; border-bottom: 1px
        // solid border-dim }`.
        (
            "panel.header",
            spec(d().border(tint(ToneRef::Border, 90), BorderWidthTier::Std).py(PadTier::Px(8.0))),
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
        // — THE Cadence signature. Bevel + weight are unmappable (see module
        // doc); the pill is the load-bearing half.
        (
            "button.primary",
            spec(d().radius(RadiusTier::Pill).fill(tone(ToneRef::Accent)).ink(ToneRef::Bg).px(PadTier::Lg))
                .on_hover(d().fill(shade(ToneRef::Accent, ShadeRef::S400))),
        ),
        // `.ds-btn--secondary { background: bg-surface; border-radius:
        // radius-md }`.
        (
            "button.ghost",
            spec(d().radius(RadiusTier::Md).fill(tone(ToneRef::Surface))),
        ),
        // `.ds-btn--chrome.is-active { background: bg-surface; color: fg;
        // border-bottom: 2px solid accent }`.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Pill)).on_select(
                d().fill(tone(ToneRef::Surface))
                    .ink(ToneRef::Text)
                    .border(tone(ToneRef::Accent), BorderWidthTier::Px(2.0)),
            ),
        ),
        // `.ds-tab--underline.is-selected { border-bottom-width: 3px }` — the
        // thickest underline of any style. `tabs.rs` reads the fill as the
        // indicator colour and the border width as its thickness.
        (
            "tab.line.active",
            spec(d()).on_select(
                d().fill(tone(ToneRef::Accent))
                    .border(tone(ToneRef::Accent), BorderWidthTier::Px(3.0)),
            ),
        ),
        // `.ds-pill { border-radius: pill; text-transform: uppercase }` +
        // `.ds-ind-chip { border-radius: pill; background: bg-surface;
        // border: 1px solid border }`.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Pill)
                    .fill(tone(ToneRef::Surface))
                    .border(tint(ToneRef::Text, 18), BorderWidthTier::Std)
                    .px(PadTier::Px(10.0)),
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
        // solid rgba(255,255,255,0.04); height: 32px }` — the header sits on
        // the PURE BLACK canvas, not on the panel surface. Cadence-specific.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Bg))
                    .border(tint(ToneRef::Text, 10), BorderWidthTier::Std)
                    .py(PadTier::Px(6.0)),
            ),
        ),
        // Flush rows (no gaps, no radius) — the Spotify list read.
        (
            "row.list",
            spec(d().radius(RadiusTier::None).px(PadTier::Px(8.0)).no_border()),
        ),
        // `.ds-wl-row:hover { background: color-mix(accent 8%, transparent) }`.
        (
            "row.list.hover",
            spec(d()).on_hover(d().fill(tint(ToneRef::Accent, 20))),
        ),
        // `.ds-wl-section { background: color-mix(accent 6%, bg-surface) }`.
        (
            "section.header.fill",
            spec(d().fill(tint(ToneRef::Accent, 16)).py(PadTier::Px(6.0))),
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

fn alto() -> RecipeSet {
    set(vec![
        // `.ds-btn--primary { box-shadow: inset 0 1px 0 rgba(255,255,255,.18),
        // inset 0 -1px 0 rgba(0,0,0,.25) }` — bevel unmappable; the sharp
        // radius is the transcribable half.
        ("button.primary", spec(d().radius(RadiusTier::Sm).fill(tone(ToneRef::Accent)))),
        // `.ds-btn--secondary { background: linear-gradient(bg-elevated,
        // bg-surface); border: 1px solid border }`, `:hover { → elevated }`.
        // The gradient collapses to its lower stop; hover lifts one ramp step.
        (
            "button.ghost",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
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
            spec(d()).on_select(d().fill(tone(ToneRef::Accent))),
        ),
        // `.ds-tab--inline.is-selected` / `tab.pill` share the 4px face.
        ("tab.pill", spec(d().radius(RadiusTier::Sm).fill(tone(ToneRef::Surface)))),
        // `.ds-ind-chip { border-radius: radius-xs; border: 1px solid border;
        // font-size: font-xs }` — a raised card face, not a pill.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Xs)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std)
                    .text_size(TextSizeTier::Xs),
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
        // the ledger hairline under every row. Square, flush.
        (
            "row.list",
            spec(
                d().radius(RadiusTier::None)
                    .border(tint(ToneRef::Text, 10), BorderWidthTier::Hair),
            ),
        ),
        // `.ds-panel__header { background: linear-gradient(bg-elevated,
        // bg-surface); box-shadow: inset 0 -1px 0 rgba(0,0,0,.35) }`.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
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
        // border-bottom: 1px solid border-dim }`.
        (
            "toolnav",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
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

fn mariner() -> RecipeSet {
    set(vec![
        ("button.primary", spec(d().radius(RadiusTier::Sm).fill(tone(ToneRef::Accent)))),
        // Same bevel structure as Alto; tighter vertical padding.
        (
            "button.ghost",
            spec(
                d().radius(RadiusTier::Sm)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std)
                    .py(PadTier::Px(3.0)),
            )
            .on_hover(d().fill(shade(ToneRef::Surface, ShadeRef::S400))),
        ),
        // `.ds-btn--chrome.is-active { background: color-mix(accent 14%,
        // bg-surface) }`.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Sm).px(PadTier::Px(8.0)))
                .on_select(d().fill(tint(ToneRef::Accent, 34))),
        ),
        // `.ds-tab--underline.is-selected { background: bg-surface; box-shadow:
        // none }` — Mariner explicitly drops Alto's bevel here.
        (
            "tab.line.active",
            spec(d()).on_select(d().fill(tone(ToneRef::Accent))),
        ),
        ("tab.pill", spec(d().radius(RadiusTier::Sm).fill(tone(ToneRef::Surface)))),
        // `.ds-ind-chip` — same raised face as Alto, tighter padding.
        (
            "tag",
            spec(
                d().radius(RadiusTier::Xs)
                    .fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std)
                    .px(PadTier::Px(6.0))
                    .text_size(TextSizeTier::Xs),
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
        // { border-bottom: 1px solid rgba(180,210,240,0.04) }`.
        (
            "row.list",
            spec(
                d().radius(RadiusTier::None)
                    .py(PadTier::Px(2.0))
                    .border(tint(ToneRef::Text, 10), BorderWidthTier::Hair),
            ),
        ),
        // `.ds-dom-row.is-current { background: linear-gradient(accent 18% →
        // transparent); border-left: 2px solid accent }` — the instrument
        // needle. The directional sweep flattens to a flat tint.
        (
            "row.list.selected",
            spec(d()).on_select(
                d().fill(tint(ToneRef::Accent, 46))
                    .border(tone(ToneRef::Accent), BorderWidthTier::Px(2.0)),
            ),
        ),
        // `.ds-pane-header.is-active { border-top: 1.5px solid accent }`.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std)
                    .py(PadTier::Px(4.0)),
            )
            .on_select(d().border(tone(ToneRef::Accent), BorderWidthTier::Px(1.5))),
        ),
        (
            "nav.cluster.active",
            spec(d().radius(RadiusTier::Sm))
                .on_select(d().fill(tint(ToneRef::Accent, 40)).ink(ToneRef::Accent)),
        ),
    ])
}

// ── Lucid ────────────────────────────────────────────────────────────────────
//
// global.css 901–932, 1476–1626, 1804–1807 + the `[data-ds="lucid"]` token
// block (450–464). Signature: editorial cream paper — gently rounded controls
// (5px), ink-fill primaries (there is no coloured button on paper), a hairline
// ring + soft drop on cards and NO inset bevel (light surfaces layer, they
// don't emboss).
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
        (
            "button.ghost",
            spec(d().radius(RadiusTier::Md).border(tone(ToneRef::Border), BorderWidthTier::Std)),
        ),
        // `.ds-btn--chrome { color: fg-dim }`, `:hover { background: bg-hover;
        // color: fg }`, `.is-active { background: fg; color: bg;
        // border-bottom: 2px solid accent }`.
        (
            "button.chrome",
            spec(d().radius(RadiusTier::Md).ink(ToneRef::Dim))
                .on_hover(d().fill(tint(ToneRef::Text, 10)).ink(ToneRef::Text))
                .on_select(
                    d().fill(tone(ToneRef::Text))
                        .ink(ToneRef::Bg)
                        .border(tone(ToneRef::Accent), BorderWidthTier::Px(2.0)),
                ),
        ),
        // `.ds-tab--underline.is-selected { color: fg; border-bottom-color:
        // accent; background: transparent }`.
        (
            "tab.line.active",
            spec(d()).on_select(
                d().fill(tone(ToneRef::Accent))
                    .ink(ToneRef::Text)
                    .border(tone(ToneRef::Accent), BorderWidthTier::Px(2.0)),
            ),
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
                    .border(tint(ToneRef::Border, 100), BorderWidthTier::Hair),
            ),
        ),
        // `.ds-wl-row:hover { background: color-mix(accent 8%, transparent) }`.
        (
            "row.list.hover",
            spec(d()).on_hover(d().fill(tint(ToneRef::Accent, 20))),
        ),
        // `.ds-wl-section { background: color-mix(accent 6%, bg-surface);
        // border-top: 1px solid border }`.
        (
            "section.header.fill",
            spec(d().fill(tint(ToneRef::Accent, 16))),
        ),
        // `.ds-panel__header { background: bg-surface; border-bottom: 1px solid
        // border; box-shadow: none }`.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
            ),
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
// NOTE: the mono/uppercase half of that signature is NOT expressible in the
// current schema (no font-family, no text-transform, no letter-spacing). What
// lands here is the SQUARE half plus the airier scale. See the module doc.

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
        (
            "button.ghost",
            spec(d().radius(RadiusTier::None).border(tone(ToneRef::Border), BorderWidthTier::Std)),
        ),
        // `.ds-btn--chrome { padding: 0 14px; font-size: font-xs }`,
        // `.is-active { background: fg; color: bg; border-bottom: 2px accent }`.
        (
            "button.chrome",
            spec(
                d().radius(RadiusTier::None)
                    .px(PadTier::Px(14.0))
                    .text_size(TextSizeTier::Xs),
            )
            .on_select(
                d().fill(tone(ToneRef::Text))
                    .ink(ToneRef::Bg)
                    .border(tone(ToneRef::Accent), BorderWidthTier::Px(2.0)),
            ),
        ),
        // `.ds-tab--underline { font-size: font-xs; padding: 0 14px }` — square
        // like every other Meridien control.
        (
            "tab.line",
            spec(
                d().radius(RadiusTier::None)
                    .px(PadTier::Px(14.0))
                    .text_size(TextSizeTier::Xs),
            ),
        ),
        // `.ds-tab--underline.is-selected { background: transparent; color: fg;
        // border-bottom-width: 2px }`.
        (
            "tab.line.active",
            spec(d().radius(RadiusTier::None)).on_select(
                d().fill(tone(ToneRef::Accent))
                    .ink(ToneRef::Text)
                    .border(tone(ToneRef::Accent), BorderWidthTier::Px(2.0)),
            ),
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
        (
            "card",
            spec(
                d().radius(RadiusTier::Md)
                    .px(PadTier::Px(22.0))
                    .py(PadTier::Px(22.0))
                    .border(tone(ToneRef::Border), BorderWidthTier::Std),
            ),
        ),
        // `.ds-wl-row { height: 34px; padding: 0 10px 0 14px }` — the airiest
        // row of any style.
        (
            "row.list",
            spec(d().radius(RadiusTier::None).px(PadTier::Px(12.0)).py(PadTier::Px(8.0)))
                .on_hover(d().fill(tint(ToneRef::Text, 10))),
        ),
        // `.ds-wl-section { padding: 10px 10px 5px; border-top: 2px solid
        // border; background: bg-surface }` + `.ds-section__header
        // { font-size: font-xs }`.
        (
            "section.header",
            spec(
                d().px(PadTier::Px(10.0))
                    .py(PadTier::Px(8.0))
                    .border(tone(ToneRef::Border), BorderWidthTier::Px(2.0))
                    .text_size(TextSizeTier::Xs),
            ),
        ),
        ("section.header.fill", spec(d().fill(tone(ToneRef::Surface)))),
        // `.ds-panel__header { background: bg-surface; border-bottom: 2px solid
        // border; min-height: 38px; font-size: font-sm }`.
        (
            "panel.header",
            spec(
                d().fill(tone(ToneRef::Surface))
                    .border(tone(ToneRef::Border), BorderWidthTier::Px(2.0))
                    .py(PadTier::Px(10.0))
                    .text_size(TextSizeTier::Sm),
            ),
        ),
        // `.ds-toolbar { padding: 0 20px; gap: 14px }` — magazine proportions.
        ("toolnav", spec(d().px(PadTier::Px(20.0)).py(PadTier::Px(10.0)))),
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

    // ── Guard-rails ──────────────────────────────────────────────────────────

    /// Every authored key must appear in `docs/migration/recipe-keys.md`.
    /// Authoring an unregistered key produces dead data — no widget reads it.
    #[test]
    fn every_authored_key_is_registered() {
        const REGISTERED: [&str; 28] = [
            "button.primary", "button.ghost", "button.danger", "button.success", "button.chrome",
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
    /// literal would pin a style to one palette.
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

    /// Every authored spec must actually change something — an all-`None`
    /// delta with no state blocks is an accidental no-op.
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
