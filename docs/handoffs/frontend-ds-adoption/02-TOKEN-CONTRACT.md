# 02 — Token Contract: The API Changes

Exact Rust changes required to make the six design systems expressible.

> **Revision 2 (2026-08-02).** Revision 1 over-stated the gaps. A full read of
> `StyleSystem` showed that bevels, mono/uppercase label flags, serif headlines and a
> per-preset pill radius **already exist**. The remaining gaps are narrower, more specific,
> and in two cases more interesting than what R1 claimed. Corrections are marked
> **[R1 WRONG]**. Trust this revision.

Claims are **[verified]** (read from source during authoring) or **[per doc]**.

---

## 0. Governing principle: additive wins

`theme_pack/migrate.rs` **[verified]**:

> "Additive changes (new optional JSON fields) do **NOT** require a version bump —
> `#[serde(default)]` handles those."

`CURRENT_SCHEMA_VERSION = 1` **[verified: `theme_pack/manifest.rs:35`]**.

| Change | Kind | Schema bump? |
|---|---|---|
| **A** — `ColorScheme` authored ramp | additive, all `Option` | ❌ none |
| **B** — Unify the split-brain radius | **bug fix**, no new fields | ❌ none |
| **C** — Authored bevel tint | additive | ❌ none |
| **D** — Card recipe + numerals + weights | additive | ❌ none |
| **E** — Multi-layer card shadows | **type change** | ✅ v1 → v2 |

---

## 1. The "adding a field" checklist

A token field is not done when the struct compiles:

| # | File | What |
|---|---|---|
| 1 | `design_system/color_scheme.rs` / `style_system.rs` | the field + doc comment |
| 2 | `design_system/baseline.rs` | default value |
| 3 | `design_system/builtin.rs` | populated for every builtin |
| 4 | `design_system/snapshot.rs` | flowed into `DesignSnapshot` / `TokenSnapshot` (`:373`) |
| 5 | `design_system/export.rs` | token export |
| 6 | `design_system/import/{model,convert,mapping}.rs` | DTCG import |
| 7 | `design_system/theme_pack/validate.rs` | validation rule |
| 8 | `foundation/design_tokens.rs` | `dt_f32!`/`dt_u8!`/`dt_i8!` wiring |
| 9 | `foundation/design_inspector.rs` | control in the F12 editor |
| 10 | `design_system/equivalence_tests.rs` | regression test |

**#4 has a trap.** `ui_kit/style.rs` reads tokens from a thread-local `TokenSnapshot`
pushed per frame **[verified: `frame_tokens()` at `ui_kit/style.rs:181`]**, and the source
comment says: *"Hosts that don't push a snapshot get the `DEFAULT_TOKEN_SNAPSHOT` values."*
**A host that never pushes a snapshot silently renders defaults** — a half-themed surface
that looks like a token bug but is a plumbing bug. Check that your field is in the
snapshot *and* that the host pushes one.

**#8 and #9 are the most-skipped.** A token absent from the F12 editor gets tuned by
rebuild-and-squint — the loop `cargo design` exists to kill.

---

## 2. What ALREADY EXISTS — do not build these

**[R1 WRONG]** Revision 1 listed several of these as missing. They are not. Read this
section before writing a line of code.

### `Treatments` — 27 behavioural flags **[verified]**

| Flag | Type | Covers |
|---|---|---|
| `surface_bevel` | `BevelStyle` | **`None` / `Raised` / `Inset`** — "the Zed raised button face look — Alto/Mariner" |
| `bevel_highlight_alpha` | `u8` | top inner-highlight line |
| `bevel_shadow_alpha` | `u8` | bottom inner-shadow line |
| `uppercase_section_labels` | `bool` | **Meridien's UPPERCASE labels** |
| `section_header_mono` | `bool` | **Meridien/Alto/Mariner mono headers** |
| `wl_symbol_mono` | `bool` | mono symbols in list rows |
| `serif_headlines` | `bool` | **Lucid's serif hero** |
| `solid_active_fills` | `bool` | palette-inverted active state |
| `hairline_borders` | `bool` | `Strokes.thin` vs `Strokes.std` |
| `segmented_filled_idle` | `bool` | segmented idle fill |
| `focus_ring` | `FocusRingStyle` | `None`/`Outline`/`Glow` |
| `wl_row_side_margin` | `f32` | "0 = flush; 6 = Aperture pill rows; 4 = Glass" |
| `wl_row_corner_radius` | `u8` | "0 = square; 99 = full pill" |
| `wl_row_divider_alpha` | `u8` | per-row hairline |
| `panel_tab_treatment` | `u8` | Line/Segmented/Filled/Card/Pane |
| `pane_active_fill_accent` | `bool` | **"Aperture signature — orange bar"** |
| `button_treatment` | `u8` | SoftPill/OutlineAccent/UnderlineActive/RaisedActive/BlackFillActive |
| `invert_active_fill` | `bool` | fill=text, text=bg |
| `vertical_group_dividers` | `bool` | toolbar cluster dividers |
| `show_active_tab_underline` | `bool` | |
| `inactive_header_fill` | `bool` | |
| `nav_buttons_label_only` | `bool` | |
| `nav_buttons_uppercase_labels` | `bool` | |
| `tab_underline_under_text` | `bool` | |
| `card_floating_shadow` | `bool` | |
| `shadows_enabled` | `bool` | master toggle |
| `animations_enabled` | `bool` | reduce-motion |

### `Chrome` — 43 geometry/finish knobs **[verified]**

`toolbar_height_scale` `header_height_scale` `account_strip_height` `pane_border_width`
`pane_gap` `pane_gap_alpha` `pane_active_indicator` `active_header_fill_multiply`
`inactive_header_fill_multiply` `header_outer_border_alpha` `header_outer_border_width`
`header_divider_alpha` `nav_active_col_alpha` `dialog_backdrop_alpha` `tab_inactive_alpha`
`tab_hover_bg_alpha` `tab_underline_thickness` `section_label_padding_top`
`section_label_padding_bottom` `drag_handle_alpha` `drag_handle_dot_scale` `toast_bg_alpha`
`card_stripe_alpha` `card_floating_shadow_alpha` `accent_emphasis` `disabled_opacity`
`focus_ring_width` `focus_ring_alpha` `hover_bg_alpha` `active_bg_alpha` `region_gap`
`region_radius` `region_border_alpha` `nav_cluster_radius` `nav_cluster_fill_alpha`
`nav_cluster_padding` `button_group` `toolnav_height` `footer_default_open`
`panel_header_treatment` `panel_section_fill_alpha` `panel_footer_card` `panel_footer_radius`

### `Radii` — includes a per-preset pill **[verified]**

```rust
pub struct Radii {
    pub none: f32,  pub xs: f32,  pub sm: f32,  pub md: f32,  pub lg: f32,
    pub full: f32,
    /// Pill radius as the runtime `r_pill` value (px, 0–99). Distinct from `full`:
    /// Meridien uses 0 (sharp pill), Aperture/Octave use 99 (rounded pill).
    pub pill: f32,
    /// Chip/badge corner radius (`r_chip`). 0 = use `sm`.
    pub chip: f32,
}
```

**[R1 WRONG]** R1 proposed a new `control_radius` field. `Radii.pill` already has exactly
those semantics, and its doc comment already names Meridien-0 and Aperture-99. **The field
is not the problem — see Change B.**

### Also present

- `Typography`: `size_{xs,sm,md,lg,xl}`, `mono_{sm,md,lg}`, `size_section_label`,
  `label_tracking`, `nav_tracking`, `section_tracking`, `family_{ui,mono,display}`
- `Spacing`: `xs sm xs_mid md lg xl xxl gmd cta_height cta_padding_x button_height button_padding_x tab_height`
- `Alphas`, `Strokes`, `Elevation`, `Density`, `Shadows` (4 roles)
- User-level overrides: `corner_scale_override()` (Sharp/Subtle/Standard/Round) and
  `border_weight_override()` (Hairline/Standard/Bold) **[verified: `ui_kit/style.rs:194-206`]**

---

## 3. Change A — `ColorScheme` authored ramp

**Unchanged from R1. This gap is real and verified.**

### Problem

Design systems hand-author a 4-step background ramp and a 4-step ink ramp. `ColorScheme`
stores 2–3 points and synthesises the rest via `elevate()` — **achromatic** and
**single-direction**.

```
Aperture:  bg #000000 → panel #141311 (warm: R>G>B)
           elevate(#000000, 20) = #141414   ← neutral. Warm tint unreachable.

Lucid:     bg #f1ede4 → panel #f7f3ea → surface #e9e4d8 → elevated #e3dccd
           panel LIGHTER than canvas, then DARKER. Non-monotonic.
           No single-direction function produces this.
```

**6 of 18 DS colour tokens have no field.** The worst is `--ds-bg-hover`: Aperture's is
`rgba(239,91,59,0.06)` (accent-tinted), Cadence's `rgba(255,255,255,0.06)` (neutral). Only
an alpha exists (`Chrome.hover_bg_alpha`), so every theme gets a neutral wash — visible on
every hover in every panel.

### Change

```rust
// design_system/color_scheme.rs

pub struct ColorScheme {
    // ── existing fields UNCHANGED ─────────────────────────────────────────

    // ── NEW: authored ramp (Stream DS-1) ──────────────────────────────────
    // DS schemes author these. Classic schemes leave them `None` and keep
    // today's derived behaviour byte-for-byte.
    // Resolution: `authored.unwrap_or_else(|| derived())`.

    /// Panel surface. DS `--ds-bg-panel`. None → `elevate(bg, ELEVATE_PANEL_BODY)`.
    #[serde(default)] pub bg_panel: Option<Rgba>,
    /// Elevated surface. DS `--ds-bg-elevated`. None → `elevate(bg, ELEVATE_RAISED)`.
    #[serde(default)] pub bg_elevated: Option<Rgba>,
    /// Hover wash carrying HUE, not just alpha. DS `--ds-bg-hover`.
    #[serde(default)] pub bg_hover: Option<Rgba>,
    /// Fourth ink step below `text_muted`. DS `--ds-fg-xmuted`.
    #[serde(default)] pub fg_xmuted: Option<Rgba>,
    /// Secondary accent. DS `--ds-accent-sub`. None → `accent`.
    #[serde(default)] pub accent_sub: Option<Rgba>,
    /// Authored bull/bear washes. DS `--ds-bull-alpha` / `--ds-bear-alpha`.
    #[serde(default)] pub bull_alpha: Option<Rgba>,
    #[serde(default)] pub bear_alpha: Option<Rgba>,
    /// Dimmer hairline distinct from `border`. DS `--ds-border-dim`.
    #[serde(default)] pub border_dim: Option<Rgba>,
}
```

### Accessor pattern — the important half

Call sites must **never** read the raw `Option`. Follow the existing
`resolved_success()` / `resolved_danger()` family:

```rust
impl ColorScheme {
    pub fn resolved_bg_panel(&self) -> Rgba {
        self.bg_panel.unwrap_or_else(|| elevate(self.bg, ELEVATE_PANEL_BODY))
    }
    pub fn resolved_bg_hover(&self, chrome: &Chrome) -> Rgba {
        self.bg_hover.unwrap_or_else(|| neutral_wash(self.bg, chrome.hover_bg_alpha))
    }
    // … one per new field
}
```

Then thread through `ComponentTheme` — its `surface_raised()` default (a
`color_layer_up(t, 1)` 7 %-step heuristic **[verified]**) becomes "authored if present,
heuristic otherwise".

### Acceptance
- All ~22 existing schemes render **byte-identical** (every new field `None`)
- `equivalence_tests.rs` green with **no test modified**
- No schema bump; existing packs load unchanged

---

## 4. Change B — Unify the split-brain radius ⚠️ **bug, not a feature**

**[R1 WRONG]** R1 proposed adding `control_radius`. The field exists. The *bug* is that
two token layers disagree, and the code says so itself.

### The evidence **[verified: `chart/renderer/ui/foundation/shell.rs:18-21`]**

> ```rust
> // Pill reads `StyleSettings.r_pill` which varies per style preset (e.g.
> // Meridien r_pill = 0); the ui_kit equivalent `radius_pill()` is a fixed
> // 999.0 constant with no preset awareness. Unifying requires the style-axis
> // decision deferred to Phase 5.
> ```

Two sources of truth for the same visual token:

| Path | Behaviour |
|---|---|
| `foundation::shell::Radius::Pill` → `st.r_pill` | ✅ per-preset. Meridien = 0 (square), Aperture = 99 |
| `ui_kit::style::radius_pill()` | ❌ **fixed 999.0, no preset awareness** |

**Consequence:** any `ui_kit` widget using `radius_pill()` renders a **full pill in every
theme**. Meridien's square controls are impossible in ui_kit; Cadence's pills are
accidentally right for the wrong reason. Two controls side by side — one from
`foundation::shell`, one from `ui_kit` — will have **different corner radii in the same
theme**. This is precisely the "half-applied theme" signature.

A second asymmetry **[verified: `ui_kit/style.rs:194-197` vs `:432`]**: `radius_xs/sm/md/lg`
multiply by `corner_scale_override()` (the user's Sharp/Subtle/Standard/Round preference);
`radius_pill()` does not. So the user's "Sharp" setting squares every corner **except**
pills.

### Change

1. `radius_pill()` reads `frame_tokens().radius_pill` from the per-frame `TokenSnapshot`,
   sourced from `Radii.pill` — like every other radius accessor.
2. Apply `corner_scale_override()` for consistency with the rest of the scale.
3. Populate `Radii.pill` per style system: Cadence 99, Meridien 0, Aperture 99, Alto 4,
   Mariner 4, Lucid 999-for-pills/5-for-controls (see the theme sheets).
4. Audit every `radius_pill()` call site and every `Radius::Pill` site; converge on one.
5. Consider deprecating `foundation::shell::Radius` in favour of the ui_kit accessor once
   they agree — **but that is the "style-axis decision deferred to Phase 5" the comment
   names. Do not unilaterally decide it; escalate.**

### Acceptance
- `radius_pill()` varies by style system
- Meridien renders square controls in **both** ui_kit and foundation paths
- The Sharp corner-scale override affects pills
- A screenshot with one ui_kit control beside one foundation control shows identical radii

### Why this is high value
Zero new fields, zero schema risk, fixes a visible inconsistency across the whole app, and
unblocks two of the six themes' signature looks.

---

## 5. Change C — Authored bevel tint

**[R1 WRONG]** R1 said bevels were "inexpressible". Surface bevels exist
(`Treatments.surface_bevel` = `None`/`Raised`/`Inset` + two alphas). The real gap is one
line in the doc comment **[verified]**:

> "The highlight/shadow *tint* is **derived from palette luminance at paint time** (light
> tint on dark themes, dark tint on light), so it works for any colour scheme."

### Problem

Luminance-derived means **achromatic** — the same failure mode as `elevate()` in Change A.
But Alto and Mariner differ by bevel temperature and *almost nothing else*:

```css
/* Alto   */ inset 0 1px 0 rgba(255,238,210,.06)   /* WARM cream highlight */
/* Mariner*/ inset 0 1px 0 rgba(190,215,245,.05)   /* COOL steel highlight */
```

They share an identical surface ramp, identical ink ramp, identical radii, identical
families. Their differences are: accent colour, ~10 % density, and **bevel temperature**.
With a luminance-derived tint, one of those three is unavailable — and the two themes
converge toward indistinguishable.

### Change

Additive, `Option`, no schema bump:

```rust
// design_system/color_scheme.rs — palette-side, because it is a colour

/// Bevel top-highlight tint. `None` → luminance-derived (today's behaviour).
/// Alto: warm cream rgba(255,238,210). Mariner: cool steel rgba(190,215,245).
#[serde(default)] pub bevel_highlight: Option<Rgba>,
/// Bevel bottom-shadow tint. `None` → luminance-derived.
#[serde(default)] pub bevel_shadow: Option<Rgba>,
```

Alphas stay on `Treatments` (`bevel_highlight_alpha` / `bevel_shadow_alpha`) — geometry and
intensity on the style axis, colour on the palette axis. That respects the two-axis split.

### Acceptance
- Alto and Mariner distinguishable **by bevel temperature alone**, accent held constant
- `None` reproduces today's luminance derivation byte-for-byte
- Light themes still get a dark-tinted bevel where `surface_bevel != None`

---

## 6. Change D — Card recipe, numerals, weights

Genuinely missing after the R2 audit. All additive.

### D1 — `CardRecipe`

`Spacing` has `cta_padding_x` and `button_padding_x` but **no card padding** **[verified]**,
and there is no way to say "no border at all".

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CardRecipe {
    pub radius:  f32,
    pub padding: f32,            // Lucid 20 · Aperture 16 · Alto 14
    /// `None` = NO stroke (Aperture `--ds-card-border: none`).
    /// `Some(w)` = hairline of width w.
    pub border_width: Option<f32>,
}

// on StyleSystem
#[serde(default)] pub card: Option<CardRecipe>,
```

> `Chrome` has `region_radius`, `panel_footer_radius`, `nav_cluster_radius` — region-level,
> not card-level. Check for overlap before adding; prefer extending `Chrome` if the
> semantics genuinely match.

### D2 — `NumeralTier`

`Treatments.serif_headlines` covers Lucid's serif hero. Nothing covers **Aperture's hero
numerals being sans** (`Inter Tight 500 @ -0.04em`) where every other theme is mono.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FontRole { #[default] Ui, Mono, Display }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumeralTier {
    pub family:   FontRole,   // Aperture: Ui (!). Others: Mono.
    pub tracking: f32,        // Aperture: -0.04 em
    pub weight:   u16,
}

#[serde(default)] pub numerals: Option<NumeralTier>,
```

Consider a `FontRole`-typed `section_header_font` to supersede the boolean
`section_header_mono` — same information, extensible to `Display`. **Additive: add the
typed field, keep the bool, deprecate later.**

### D3 — Font weights

`Typography` has **no weight fields at all** **[verified]**. The DS authors them per role:
Aperture labels 700, Alto 600, Cadence 700, Meridien 600, Lucid 700.

```rust
// Typography
#[serde(default)] pub weight_label: Option<u16>,
#[serde(default)] pub weight_body:  Option<u16>,
#[serde(default)] pub weight_display: Option<u16>,
```

> **egui caveat:** egui selects weight by *font family registration*, not a numeric weight
> axis. Shipping 600 vs 700 means registering both faces and mapping the numeric weight to
> a registered family in `ui_kit/fonts/`. **Scope this before committing** — it may be a
> font-loading task, not a token task.

### D4 — Type-scale depth

`Typography` has 5 UI sizes (`xs sm md lg xl`); the DS authors 7 (`2xs xs sm base md lg xl`).
Meridien needs 10/12/13/14/15/20. Audit whether the 5-tier scale can carry all six themes;
add `size_2xs` / `size_base` only if a theme genuinely cannot be expressed.

> **Reminder:** per-theme type scales only move if call sites use the **text cascade**
> (`TextStyle::…as_rich_cascading`). Hand-passed `FontId`s will not move. Expect call-site
> migration in the panels you touch — **never in `core.rs`**.

---

## 7. Change E — Multi-layer card shadows ⚠️ breaking

**Partially corrected.** Surface bevels exist (Change C). What does **not** exist is a
multi-layer *card shadow stack* **[verified]**:

```rust
pub struct ShadowSpec { blur: f32, spread: f32, offset_x: f32, offset_y: f32, alpha: f32 }
pub struct Shadows { card: ShadowSpec, modal: ShadowSpec, tooltip: ShadowSpec, dropdown: ShadowSpec }
```

One layer per role. Outer only. The DS card treatments are stacks:

```css
/* Alto   */ inset highlight + inset shadow + contact line + ambient drop   (4 layers)
/* Cadence*/ inset highlight + ambient drop                                  (2 layers)
/* Lucid  */ 0 1px 2px rgba(20,20,15,.05), 0 6px 16px -8px rgba(20,20,15,.12) (2 outer)
/* Aperture*/ 0 1px 0 rgba(0,0,0,.45), 0 18px 36px -22px rgba(0,0,0,.65)     (2 outer)
```

**Note:** Lucid and Aperture need only *outer* layers — but **two of them**, and `Shadows`
allows one. So even the light themes are blocked, without any inset involved.

### Change

```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShadowTint { Shadow, Highlight, Custom(Rgba) }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowLayer {
    pub inset: bool,
    pub offset_x: f32, pub offset_y: f32,
    pub blur: f32, pub spread: f32,
    pub tint: ShadowTint,   // NEVER a literal — CLAUDE.md rule 2
    pub alpha: f32,
}

pub struct Shadows {
    pub card:     Vec<ShadowLayer>,
    pub modal:    Vec<ShadowLayer>,
    pub tooltip:  Vec<ShadowLayer>,
    pub dropdown: Vec<ShadowLayer>,
}
```

`ShadowTint::Highlight` resolves against `ColorScheme.bevel_highlight` from Change C — one
temperature source, two consumers.

### Rendering

`ui_kit/widgets/shadow_pipeline.rs` (exists). Outer layers use the existing drop path, one
pass each. Inset layers all have `blur == 0`, so they are 1px edge strokes clipped to the
rect, painted after the fill. **Do not build a general inset-blur solver.**

### Migration

1. Bump `CURRENT_SCHEMA_VERSION` → `2` (`theme_pack/manifest.rs`)
2. Add `v1_to_v2` in `theme_pack/migrate.rs`, wire the match arm:

```rust
match from_version {
    1 => { v1_to_v2(self); }
    2 => { /* current */ }
    …
}
/// v1 stored one ShadowSpec per role → wrap as a single outer layer.
/// Visually identical to v1 rendering.
fn v1_to_v2(pack: &mut ThemePack) { … }
```

3. Keep `shadow_*_themed(t)` signatures stable so call sites do not churn.

### Acceptance
- A v1 `.apextheme` pack loads and renders **identically**
- `SchemaTooNew` still rejects future versions
- Lucid's two-layer outer shadow renders correctly
- No literal black (`style-mig-lint.sh` check 4 does not rise)
- No measurable frame-time regression on a card-heavy screen

---

## 8. Revised gap summary

| Gap | R1 said | R2 verdict |
|---|---|---|
| Authored colour ramp | missing | ✅ **confirmed missing** |
| `bg_hover` hue | missing | ✅ **confirmed missing** |
| Control radius | "add `control_radius`" | ❌ **field exists** (`Radii.pill`); the bug is `ui_kit::radius_pill()` is a fixed 999.0 |
| Bevels | "inexpressible" | ❌ **exist** (`Treatments.surface_bevel`); gap is the **tint is luminance-derived**, so Alto-warm vs Mariner-cool is unavailable |
| Uppercase / mono labels | missing | ❌ **exist** (`uppercase_section_labels`, `section_header_mono`) |
| Serif hero | missing | ❌ **exists** (`serif_headlines`) |
| Numeral family | missing | ✅ **confirmed missing** |
| Font weights | missing | ✅ **confirmed missing** (none in `Typography`) |
| Card padding / no-border | missing | ✅ **confirmed missing** (`Spacing` has no card padding) |
| Multi-layer shadows | missing | ✅ **confirmed missing** — and it blocks *light* themes too (2 outer layers) |

**The lesson worth carrying:** this codebase is richer than it looks and its doc comments
are unusually good. **Grep the struct before proposing a field.** Two of R1's five
proposals were reinventions, and one of those (`radius_pill`) turned out to hide a real bug
that a new field would have papered over.

---

## 9. Test strategy

`design_system/equivalence_tests.rs` (1,072 lines) is the safety net.

> **Phase-1 rule: no existing test may be modified to accommodate a new field.** If one
> needs changing, the change is not additive — stop.

```rust
#[test] fn authored_ramp_overrides_derivation()      { … }
#[test] fn none_ramp_falls_back_to_elevate()         { … }
#[test] fn aperture_panel_is_warm()                  { /* R > G > B */ }
#[test] fn lucid_ramp_is_non_monotonic()             { … }
#[test] fn lucid_and_meridien_share_palette()        { … }
#[test] fn lucid_and_meridien_differ_in_label_flags(){ … }
#[test] fn radius_pill_varies_by_style_system()      { /* Change B */ }
#[test] fn radius_pill_honours_corner_scale()        { /* Change B */ }
#[test] fn meridien_pill_is_zero()                   { … }
#[test] fn alto_and_mariner_bevel_tints_differ()     { /* Change C */ }
#[test] fn aperture_numerals_are_ui_family()         { … }
#[test] fn v1_pack_migrates_to_single_outer_layer()  { /* Change E */ }
```

Ratchets — none may rise:

```bash
./scripts/check-design-system.sh
bash scripts/style-mig-lint.sh
./scripts/sx_ratchet.sh
```

---

## 10. Out of scope here

- **Layout archetypes** — design brief §6; see the `ShellProfile` overlap in `00-START-HERE.md`
- **`ShellProfile`** — `docs/migration/shell-profile.md`, unsigned draft
- **`meridien` as a `ColorScheme`** — trivial to register; the *selection model* (named
  presets vs matrix) is a product decision
- **The `radius_pill` unification's "style-axis decision deferred to Phase 5"** — named in
  the source. Escalate; do not decide unilaterally
- **Taffy layout engine** — deliberately deferred per `docs/UI_WORKFLOW.md`
