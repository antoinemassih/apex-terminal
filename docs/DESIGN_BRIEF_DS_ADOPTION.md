# Design Brief — Adopting the ApexTerminalThemes Design Systems in `apex-terminal`

**Status:** draft for execution
**Target:** `apex-terminal/src-tauri` (Rust + egui, native)
**Source of truth:** `../ApexTerminalThemes/`
**Written:** 2026-08-02

---

## 0. What this document is

A specification for making the native Rust terminal actually *look like* the six
ApexTerminalThemes design systems — Aperture, Cadence, Alto, Mariner, Lucid, Meridien
— rather than merely wearing their colours.

It is written to be executed by someone (or several someones) who has not read the
React mockups. Every claim below is grounded in a file path. Where a number is given,
it was read out of the source, not invented.

**How to use it:** Part 2 is the diagnosis — read it first, it explains why previous
attempts produced "recoloured, not redesigned". Part 4 is the token contract and
contains the concrete gaps to close. Part 5 is six per-theme spec sheets. Part 9 is
the sequenced plan with acceptance gates. Nothing here should be started before the
Part 8 verification loop is working — without screenshots this work is unfalsifiable.

---

## 1. Sources of truth (ranked)

| Rank | Artifact | Path | Notes |
|---|---|---|---|
| 1 | **Original theme apps** | `ApexTerminalThemes/Trading App - <Theme>/` | The real design. Six standalone React/HTML apps. Serve via `node server.js` → `http://localhost:5173`. |
| 2 | **DS specifications** | `ApexTerminalThemes/design-systems/*.md` | 12,337 lines across 6 files. Full token architecture, type scales, component library, trading-specific patterns. `aperture.md` alone is 1,900 lines. |
| 3 | **React token blocks** | `ApexTerminalThemes/terminal/src/global.css` | `[data-ds="<theme>"]` blocks, lines 145–530. Already-normalised token values — the closest thing to a machine-readable port target. |
| 4 | **React fidelity audit** | `ApexTerminalThemes/terminal/FIDELITY-AUDIT.md` | The post-mortem of the *previous* port attempt. Its failure modes are the ones we must not repeat. |
| 5 | **Primitive parity map** | `ApexTerminalThemes/terminal/PRIMITIVE-PARITY.md` | React ↔ Rust widget correspondence. |

**Rule:** when 1 and 3 disagree, 1 wins, and `global.css` gets a correction commit.
The React port is a *port*, not the design.

---

## 2. Diagnosis — why this hasn't worked yet

Five findings. The first is the big one; the rest are mechanical.

### Law 1 — Recolouring is not redesigning

This is settled empirically, not theoretically. The React team ran exactly this
experiment and measured it (`FIDELITY-AUDIT.md`):

| Theme | Approach | Fidelity |
|---|---|---|
| Aperture | bespoke layout | ~80% |
| Cadence | bespoke layout, rough | ~55% |
| Alto | **default layout, recoloured** | ~35% |
| Mariner | **default layout, recoloured** | ~35% |
| Lucid | **default layout, recoloured** | ~15% |
| Meridien | **default layout, recoloured** | ~10% |

After rebuilding the four laggards on the *correct structure* (one shared
`EditorialLayout` with per-brand shells), all six landed at ~90%.

Their own root-cause line is worth quoting verbatim:

> "We theme-recoloured instead of theme-restructured for 4 of 6. Token swaps make a
> theme a different colour, not a different *design*. The sources have different
> layouts, not just palettes."

**Implication for us.** `apex-terminal` today composes a theme as
`ColorScheme × StyleSystem`. Both axes are *finishes*. Neither can move a panel.
Lucid and Meridien are **editorial dashboards** — hero price + metric grid + area
chart + sector heatmap + P&L cards. They are not a 9-pane trading grid wearing cream.
If we ship them as recoloured trading grids we will reproduce the 10–15% row above.

This does **not** mean we need six layout engines. It means we need **three layout
archetypes** (Part 6), and the theme must be able to select one.

### Law 2 — Multiply-from-black is a fixed point

Already discovered and fixed in this repo on 2026-07-30 — documented in
`src-tauri/src/ui_kit/style.rs:449-461`. Recorded here because it is the archetype
of the whole failure class:

The elevation ladder was gamma multipliers (`0.95 / 0.88 / 0.85`) over `bg`. Multiplying
only ever *darkens*. On Aperture (`bg = #000000`) every elevated surface collapses back
to black — `0 × 0.95 = 0`. Result: dead-flat panels where the mockup has panels visibly
lifting off the canvas.

`elevate(bg, amount)` replaces the multiply with an **additive luminance shift toward
the contrast direction** — dark backgrounds lighten, light backgrounds darken (at 3/5
strength so cream palettes don't go muddy).

**Action:** verify every surface actually routes through `elevate()`. The constants
(`ELEVATE_PANEL_HEADER = 30`, `ELEVATE_CARD = 22`, `ELEVATE_MODAL = 38`, …) were tuned
against Aperture's authored ramp — but see Law 3, because tuning a global constant to
one theme is itself the problem.

### Law 3 — The DS authors its ramps; we derive them

This is the unfixed structural version of Law 2.

Every design system **hand-authors a four-step background ramp and a four-step ink
ramp**. We store two or three points and synthesise the rest.

Aperture, authored:

```
--ds-bg          #000000
--ds-bg-panel    #141311   (+19 luminance, and WARM — R>G>B)
--ds-bg-surface  #1a1816   (+26, warm)
--ds-bg-elevated #1f1d1a   (+31, warm)
```

`elevate()` is achromatic — it adds the same delta to R, G and B. From `#000000` it
can produce `#131313`, never `#141311`. **The warm tint is unreachable by
construction.** Aperture's entire character is warm-neutral panels on a pure-black
canvas; we can currently render the value but not the hue.

Same story on the ink ramp — the DS authors four steps
(`--ds-fg / -dim / -muted / -xmuted`); `ColorScheme` has `text`, `dim`, `text_muted`
and no fourth.

**Fix:** make the ramp *authored with derivation as fallback* — add optional fields to
`ColorScheme` that, when `Some`, are used verbatim, and when `None`, fall back to
today's `elevate()` behaviour. Zero breakage for the 20+ existing schemes; full
fidelity for the six DS schemes. Detailed in Part 4.

### Law 4 — Personality lives in Chrome and Treatments, and is under-populated

`StyleSystem` is well designed for this. `Chrome` alone carries ~60 knobs
(`style_system.rs:683`) — `pane_gap`, `pane_active_indicator`, `tab_underline_thickness`,
`header_divider_alpha`, `focus_ring_width`, `accent_emphasis`, …

The registry currently ships **9 style systems** (`builtin.rs`, asserted at
`builtin.rs:1471`): 3 canonical (`meridien`, `aperture`, `octave`) + 6 React ports
(`cadence`, `alto`, `mariner`, `lucid`, `relay`, `glass`).

So the axis exists and is populated. The gap is **calibration** — the values were
ported before the React side reached ~90%, i.e. they were ported from the 15–35%
era. They need re-derivation from `global.css` as it stands today.

### Law 5 — Derive, don't pin (the frozen-chrome pattern)

A recurring defect in this repo: chrome dimensions get pinned to a literal that a
token *used to* produce. When the token later changes, the chrome doesn't follow, and
you get clipping or misalignment (the toolnav 38.0 floor → 0.6px clip).

Every number in Part 5 is a **token value**, not a call-site literal. If a spec says
Meridien's toolbar is 52px, that becomes `Chrome.toolbar_height_scale` against the
baseline — not `let h = 52.0;` in a render function.

---

## 3. Inventory — what already exists (do NOT rebuild)

Read this before writing any code. The infrastructure is substantially built.

### Design-system module — `src-tauri/src/design_system/` (11,094 lines)

| File | LOC | Role |
|---|---|---|
| `builtin.rs` | 1,498 | ~22 `ColorScheme`s + 9 `StyleSystem`s |
| `equivalence_tests.rs` | 1,072 | Regression cover |
| `style_system.rs` | 955 | `Typography`/`Spacing`/`Radii`/`Strokes`/`Alphas`/`Elevation`/`Density`/`Shadows`/`Treatments`/`Chrome` |
| `loader.rs` | 949 | Runtime load |
| `theme_pack/validate.rs` | 736 | Pack validation |
| `snapshot.rs` | 713 | `TokenSnapshot` |
| `recipes.rs` | 575 | Component recipes |
| `theme_pack/pack_registry.rs` | 544 | Pack registry |
| `export.rs` | 447 | Token export |
| `import/convert.rs` | 438 | **External → internal conversion** |
| `baseline.rs` | 436 | Baseline defaults |
| `color_scheme.rs` | 361 | `ColorScheme` + `Meta` |
| `hot_reload.rs` | 318 | Live reload |

Plus `theme_pack/{bundle,manifest,migrate,mod}.rs` and `import/{mapping,model,mod}.rs`.

**Already-present DS colour schemes:** `aperture`, `cadence`, `alto`, `mariner`,
`lucid` (`builtin.rs:660-792`). **`meridien` is absent as a ColorScheme** — it exists
only as a StyleSystem. That is correct in spirit (Meridien shares Lucid's palette
exactly) but means it cannot be selected as a scheme; see Part 5.6.

**Default is already Aperture** — `gpu.rs:2790`, `theme_idx: 16`, commented
"Aperture — flagship default".

### UI kit — `src-tauri/src/ui_kit/` (~95 widget files)

Comprehensive. `button`, `tabs`, `select`, `modal`, `popover`, `tooltip`,
`context_menu`, `sparkline`, `heatmap_grid`, `pane_grid`, `panel_*` family,
`risk_reward_bar`, `theme_preview_card`, `shadow_pipeline`, `text_subpixel_pipeline`,
`motion`, `guild_avatar_grid`…

**Do not add widgets before checking this list.** The React `FIDELITY-AUDIT` "missing
primitives" table (AreaChart, MetricGrid, HeatmapGrid, OrderBook, StatCard,
DashboardShell) is a **React** gap list. On the Rust side `heatmap_grid.rs`,
`metric_row.rs` and `sparkline.rs` already exist — verify before building.

### Governance already in force

`src-tauri/CLAUDE.md` is binding and covers: never `&THEMES[0]`; never hardcode black
shadows; tokens not literals; `ui_kit::Button` over `egui::Button`; light-theme parity
walk; **`render/pane/core.rs` is sacred — excluded from all design sweeps**; `Watchlist`
and `Chart` are frozen god-objects.

**This brief does not override any of that.** In particular: no mechanical token sweep
enters `core.rs`.

---

## 4. The token contract

### 4.1 Colour — `global.css` → `ColorScheme`

18 DS colour tokens. **6 have no home.**

| DS token | `ColorScheme` field | Status |
|---|---|---|
| `--ds-bg` | `bg` | ✅ |
| `--ds-bg-panel` | — | ❌ derived via `elevate(bg, 20)` |
| `--ds-bg-surface` | `surface` | ⚠️ partial |
| `--ds-bg-elevated` | — | ❌ derived via `elevate(bg, 30)` |
| `--ds-bg-hover` | — | ❌ **only an alpha exists** (`Chrome.hover_bg_alpha`) |
| `--ds-border` | `border` | ✅ |
| `--ds-border-dim` | — | ❌ derived alpha |
| `--ds-border-active` | `accent` | ⚠️ aliased |
| `--ds-fg` | `text` | ✅ |
| `--ds-fg-dim` | `dim` | ✅ |
| `--ds-fg-muted` | `text_muted` | ✅ |
| `--ds-fg-xmuted` | — | ❌ |
| `--ds-accent` | `accent` | ✅ |
| `--ds-accent-sub` | — | ❌ |
| `--ds-bull` / `--ds-bear` | `bull` / `bear` | ✅ |
| `--ds-bull-alpha` / `--ds-bear-alpha` | — | ❌ derived alpha |

**`--ds-bg-hover` deserves special attention.** Aperture's is
`rgba(239,91,59,0.06)` — *accent-tinted*. Cadence's is `rgba(255,255,255,0.06)` —
*neutral*. An alpha-only hover token cannot express that difference; every theme gets
a neutral wash. This single token is a visible, per-frame, everywhere-in-the-UI
divergence.

**Prescribed change** — additive, `Option`-typed, no migration:

```rust
pub struct ColorScheme {
    // … existing fields unchanged …

    /// Authored surface ramp. When `None`, falls back to `elevate(bg, …)`.
    /// Design-system schemes author these; classic schemes leave them `None`.
    pub bg_panel:    Option<Rgba>,
    pub bg_elevated: Option<Rgba>,
    /// Full RGBA hover wash — carries hue, not just alpha.
    pub bg_hover:    Option<Rgba>,
    /// Fourth ink step, below `text_muted`.
    pub fg_xmuted:   Option<Rgba>,
    /// Secondary accent (gradients, hover, sub-emphasis).
    pub accent_sub:  Option<Rgba>,
    /// Authored bull/bear washes (fill tints behind rows/bars).
    pub bull_alpha:  Option<Rgba>,
    pub bear_alpha:  Option<Rgba>,
    /// Dimmer hairline, distinct from `border`.
    pub border_dim:  Option<Rgba>,
}
```

Accessors resolve `authored.unwrap_or_else(|| derived())`. Existing schemes are
untouched; `#[serde(default)]` keeps pack compatibility.

> **⚠️ REVISED 2026-08-02 — read this before §4.2 and §4.3.**
>
> A full read of `StyleSystem` after this brief was first written showed that several
> "missing" tokens **already exist**. §4.1 (colour) stands unchanged and verified. §4.2 and
> §4.3 below are **superseded** by
> [`handoffs/frontend-ds-adoption/02-TOKEN-CONTRACT.md`](handoffs/frontend-ds-adoption/02-TOKEN-CONTRACT.md)
> revision 2. Corrections:
>
> | Claimed missing | Reality |
> |---|---|
> | `control_radius` | **Exists** — `Radii.pill`, whose doc comment already names *"Meridien uses 0 (sharp pill), Aperture/Octave use 99"*. The real defect is that `ui_kit::style::radius_pill()` is a **fixed 999.0 constant with no preset awareness** while `foundation::shell::Radius::Pill` reads the per-preset value — a split-brain the source comments itself at `foundation/shell.rs:18-21`. A new field would have papered over a live bug. |
> | Label font / transform | **Exist** — `Treatments.uppercase_section_labels`, `Treatments.section_header_mono`, `Treatments.wl_symbol_mono`. |
> | Serif display | **Exists** — `Treatments.serif_headlines`. |
> | Bevels "inexpressible" | **Exist** — `Treatments.surface_bevel` (`None`/`Raised`/`Inset`) + `bevel_highlight_alpha` / `bevel_shadow_alpha`. The real gap is narrower and sharper: the **tint is luminance-derived**, so Alto-warm vs Mariner-cool cannot be authored — which is exactly what makes those two themes converge. |
>
> **Still confirmed missing:** the authored colour ramp (§4.1), numeral family, font
> weights (`Typography` has none), card padding / no-border, and multi-layer shadow stacks
> (which block *light* themes too — Lucid and Aperture each need two **outer** layers and
> `Shadows` allows one).
>
> **The lesson:** this codebase is richer than it looks and its doc comments are unusually
> good. **Grep the struct before proposing a field.**

### 4.2 Shape and finish — `global.css` → `StyleSystem`

Mostly covered. The exceptions are load-bearing:

| DS token | Rust home | Status |
|---|---|---|
| `--ds-radius-*` | `Radii` | ✅ |
| `--ds-gap-*` | `Spacing` | ✅ |
| `--ds-stroke-*` | `Strokes` | ✅ |
| `--ds-font-*` | `Typography` | ✅ |
| `--ds-label-tracking` | `Typography.label_tracking` | ✅ |
| `--ds-toolbar-h` | `Chrome.toolbar_height_scale` | ⚠️ scale vs absolute |
| `--ds-pane-gap` | `Chrome.pane_gap` | ✅ |
| **`--ds-control-radius`** | — | ❌ **independent of the radius scale** |
| **`--ds-label-font`** | — | ❌ UI-vs-mono selector |
| **`--ds-label-transform`** | — | ❌ uppercase flag |
| **`--ds-label-weight`** | — | ❌ |
| **`--ds-num-display`** | — | ❌ display-numeral family |
| **`--ds-num-tracking`** | — | ❌ |
| **`--ds-card-shadow`** | `Shadows.card` | ❌ **see below** |
| `--ds-card-border` | — | ❌ (Aperture: `none`) |
| `--ds-card-pad` | — | ❌ |

Four of these are *signature* tokens — the thing a person points at when they say
"that's Meridien":

- **`--ds-control-radius` is not `radius_md`.** Cadence: `999px` (full pill) while its
  scale tops out at 14. Meridien: `0px` (pure square) while its scale runs 4/6/10/16.
  Deriving control radius from the scale gets both wrong.
- **`--ds-label-font` + `--ds-label-transform`.** Meridien's labels are
  **mono, uppercase, 0.08em**. Lucid's are **sans, sentence case, 700**. *They share an
  identical palette* — the label treatment is the entire differentiator.
- **`--ds-num-display`.** Aperture's hero numbers are `Inter Tight 500 @ -0.04em` —
  **sans, not mono.** Every other theme uses mono. Hardcoding "big numbers are mono"
  makes Aperture permanently wrong.

### 4.3 The bevel gap (highest-value single fix)

`ShadowSpec` (`style_system.rs:469`) is:

```rust
pub struct ShadowSpec { blur: f32, spread: f32, offset_x: f32, offset_y: f32, alpha: f32 }
```

Single layer. Outer only. No inset. No per-layer colour.

The design systems' card treatment is a **multi-layer stack including insets**:

```css
/* Alto — "Zed warm-dark bevel" */
--ds-card-shadow:
  inset 0  1px 0 rgba(255,238,210,.06),   /* warm top highlight  */
  inset 0 -1px 0 rgba(0,0,0,.45),         /* bottom inner shadow */
  0 1px 0 rgba(0,0,0,.4),                 /* contact line        */
  0 12px 28px -16px rgba(0,0,0,.6);       /* ambient drop        */

/* Mariner — same geometry, COOL highlight */
  inset 0 1px 0 rgba(190,215,245,.05),    /* ← the only palette-level diff */

/* Cadence — Spotify */
  inset 0 1px 0 rgba(255,255,255,.035),
  0 8px 24px -12px rgba(0,0,0,.6);

/* Lucid — editorial paper, NO inset bevel on light */
  0 1px 2px rgba(20,20,15,.05), 0 6px 16px -8px rgba(20,20,15,.12);

/* Aperture — no border at all, just lift */
--ds-card-border: none;
--ds-card-shadow: 0 1px 0 rgba(0,0,0,.45), 0 18px 36px -22px rgba(0,0,0,.65);
```

The React audit named this explicitly as a missing primitive: *"Source uses layered
inset highlight+shadow ('Zed bevels', Spotify bevels). We approximate with a couple of
CSS rules; there's no shared elevation token/util."*

**This is what makes those UIs read as crafted rather than flat.** It is currently
inexpressible in our type system.

**Prescribed change:**

```rust
pub struct ShadowLayer {
    pub inset:    bool,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur:     f32,
    pub spread:   f32,
    /// Tint applied over the theme's shadow/highlight colour. Never hardcode black
    /// (CLAUDE.md rule 2) — this modulates `ColorScheme.shadow` or a light equivalent.
    pub tint:     ShadowTint,
    pub alpha:    f32,
}

pub enum ShadowTint { Shadow, Highlight, Custom(Rgba) }

pub struct Shadows {
    pub card:  Vec<ShadowLayer>,   // was ShadowSpec
    pub modal: Vec<ShadowLayer>,
    // …
}
```

Rendering: `ui_kit/widgets/shadow_pipeline.rs` already exists and is the right home.
Inset layers paint *after* the fill, clipped to the card rect, as 1px edge strokes for
the `blur == 0` case (which covers every inset above) — cheap, no blur pass needed.

`ShadowTint::Highlight` is what carries Alto-warm vs Mariner-cool. Keep it a semantic
token, not a literal, or Law 5 bites.

---

## 5. Per-theme spec sheets

All values read from `global.css:145-530`. `#` values are verbatim.

### 5.1 Aperture — *warm brutalist, block-colour tiles*

**Archetype:** Mosaic (bespoke). **Dark.** Default theme.

| | |
|---|---|
| Canvas | `#000000` — pure black, no compromise |
| Panel ramp | `#141311` → `#1a1816` → `#1f1d1a` (**warm**: R>G>B) |
| Hover | `rgba(239,91,59,0.06)` — **accent-tinted** |
| Border | `rgba(255,255,255,0.12)` / dim `rgba(255,255,255,0.06)` |
| Ink | `#f4ece0` → `#b6ad9d` → `#76705f` → `#4a4337` (cream) |
| Accent | `#ef5b3b` orange, sub `#ff8a4c` |
| Bull/Bear | `#4ec07a` / `#d8503e` |
| Type | Inter Tight / JetBrains Mono, base **13px** (a notch up) |
| Sizes | 9 / 10 / 12 / 13 / 14 / 18 / 24 |
| Tracking | tight 0 · label 0.3 · wide 0.6 |
| **Radii** | **8 / 10 / 14 / 20 / pill — the signature big-radius scale** |
| Controls | `radius-sm` = 10px |
| Cards | `radius-lg` = **20px**, pad 16px, **`border: none`** |
| Strokes | thin 0.5px, std 1px |
| Heights | row 28 · btn 24/28/32 · toolbar 40 |
| Gaps | 10 / 14 / 18 |
| Labels | sans, **no** transform, weight **700** |
| **Numerals** | **`--ds-num-display: sans` @ `-0.04em`** — NOT mono |
| Panes | gap 7px, inset −3px, **leaf radius 0** |

**Signature:** one rounded coal envelope. The outer `PaneGrid` frame owns the
`radius-lg` round; individual panes are square and share flush hairline dividers.
Not a grid of separate rounded cards — *one* rounded card subdivided.

**Acceptance:** panels visibly warm against pure black; cards have no stroke, only
lift; hero numerals are sans and tight; hover carries an orange cast.

### 5.2 Cadence — *Spotify, pill-forward*

**Archetype:** Dense 3-column (bespoke). **Dark.**

| | |
|---|---|
| Canvas | `#000000` |
| Panel ramp | `#121212` → `#181818` → `#1f1f1f` (**neutral**) |
| Hover | `rgba(255,255,255,0.06)` — neutral |
| Border | `rgba(255,255,255,0.07)` / dim `.035` |
| Ink | `#ffffff` → `#b3b3b3` → `#7a7a7a` → `#535353` |
| Accent | `#1ed760` Spotify green, sub `#169c46` |
| Bull/Bear | `#1ed760` / `#f15e6c` |
| Type | Inter / JetBrains Mono |
| Tracking | tight 0 · label 0.2 · wide 0.5 |
| Radii | 4 / 6 / 10 / 14 / pill |
| **Controls** | **`999px` — full pill. The signature.** |
| Cards | `radius-lg` 14px, pad 16px, 1px border |
| Strokes | thin 1 · std 1 · bold 2 |
| Gaps | 4 / 6 / 8 / 12 / 16 / 24 |
| Labels | sans, no transform, weight **700** |
| Bevel | `inset 0 1px 0 rgba(255,255,255,.035)` + `0 8px 24px -12px rgba(0,0,0,.6)` |

**Signature:** full-pill buttons on a pure-black canvas with brand green. Cadence is
the only theme where `control_radius` ≫ `card_radius`.

**Structural details** (from the React Wave-3 notes): continuous DOM ladder — red asks
above / green bids below a *shared* depth-bar axis with a current-price divider;
watchlist has a sparkline TREND column; T&S carries venue + condition codes with tick
colouring; 17px dense DOM rows with the current row green-highlighted; green "+ Trade"
topbar CTA.

### 5.3 Alto — *Zed warm dark, amber*

**Archetype:** Editorial dashboard (warm-dark shell). **Dark.**

| | |
|---|---|
| Canvas | `#15120e` |
| Panel ramp | `#1c1814` → `#221d18` → `#2a241e` |
| Hover | `rgba(217,152,88,0.07)` — amber-tinted |
| Border | `#3d342b` / dim `#2f2820` (**opaque**, not alpha) |
| Ink | `#efe7d8` → `#cfc6b6` → `#9c9385` → `#6b6358` |
| Accent | `#d99858` amber, sub `#b87838` |
| Bull/Bear | `#6fbf73` / `#e25d5d` |
| Type | **IBM Plex Sans / IBM Plex Mono** |
| Tracking | tight 0 · label 0.4 · wide 0.8 |
| **Radii** | **2 / 4 / 6 / 8 — sharp, restrained** |
| Controls | 4px · Cards 6px, pad 14px, 1px border |
| Labels | sans, no transform, weight **600** |
| **Bevel** | **warm** highlight `rgba(255,238,210,.06)` |

### 5.4 Mariner — *Alto's sibling, steel blue, tighter*

**Archetype:** Editorial dashboard (steel-dark shell). **Dark.**

Inherits Alto's surface + ink ramp **exactly**. Four deliberate differences:

1. **Accent** `#6ea0c8` steel-blue (sub `#4f7ea0`) vs Alto's amber
2. **Density ~10% tighter** — row 22 (Alto ~24) · btn 20/22/25/30 · toolbar 36 (Alto 40)
3. **Bevel highlight is cool** — `rgba(190,215,245,.05)` vs Alto's `rgba(255,238,210,.06)`
4. **Accent usage** — precision markers (edge borders, directional sweeps) rather than
   Alto's ambient amber glow. A *usage* rule, not a token; encode via
   `Chrome.accent_emphasis` and recipe choices.

Hover: `rgba(110,160,200,0.07)`.

> Alto and Mariner are **siblings, not clones**. If a reviewer can't tell them apart
> at a glance, density and bevel temperature are wrong.

### 5.5 Lucid — *editorial paper, terracotta*

**Archetype:** Editorial dashboard. **LIGHT.**

| | |
|---|---|
| Canvas | `#f1ede4` cream paper |
| Panel ramp | `#f7f3ea` → `#e9e4d8` → `#e3dccd` (*inverts* — panel is LIGHTER than canvas) |
| Hover | `rgba(20,20,15,0.04)` |
| Border | `#bfb59a` / dim `#d8d0bd` |
| Ink | `#14140f` → `#2a2a23` → `#8a8675` → `#b3ad9c` |
| Accent | `#d6552b` terracotta, sub `#c43a1f` |
| Bull/Bear | `#1f6f3b` deep green / `#c43a1f` venetian |
| Type | **DM Sans / DM Mono**; DM Serif Display for the hero price |
| Tracking | tight 0 · label 0.4 · wide 0.8 |
| Radii | 2 / 3 / 5 / 8 — restrained |
| Controls | 5px · Cards 8px, **pad 20px** (editorial cards breathe) |
| Labels | **sans**, no transform, weight 700 |
| Shadow | **no inset bevel on light** — `0 1px 2px rgba(20,20,15,.05)`, `0 6px 16px -8px rgba(20,20,15,.12)` |

**Note the ramp inversion.** Panel (`#f7f3ea`) is *lighter* than canvas (`#f1ede4`),
then surface/elevated go *darker*. This is not monotonic and cannot be produced by a
single-direction `elevate()`. Authored ramp required (Part 4.1).

**Scoped serif:** the hero price uses a serif display face. Everything else is DM Sans.
Needs `Typography.family_display` wired *and scoped* — not a global switch.

### 5.6 Meridien — *Lucid's palette, magazine proportions, MONO CAPS*

**Archetype:** Editorial dashboard (dark top chrome). **LIGHT.**

Palette is **byte-identical to Lucid**. Every differentiator is shape and type.

| | Lucid | **Meridien** |
|---|---|---|
| font-2xs…lg | 9/10/12/13/14/18 | **10/12/13/14/15/20** |
| tracking tight | 0 | **−0.01em** |
| tracking label | 0.4px | **0.06em** |
| tracking wide | 0.8px | **0.10em** |
| radii | 2/3/5/8 | **4/6/10/16** |
| gaps | 4/6/8/12/16/24 | **4/6/10/12/18/24/32** |
| btn heights | 20/24/28 | **22/28/34/40** |
| row height | 24 | **32** |
| toolbar | 40 | **52** |
| **label font** | sans | **MONO** |
| **label transform** | none | **UPPERCASE** |
| **label tracking** | 0.4px | **0.08em** |
| label weight | 700 | 600 |
| **control radius** | 5px | **0px — pure square** |

> Source comment, verbatim: *"the Meridien signature: MONO · UPPERCASE · SQUARE
> controls. This is the single biggest differentiator from Lucid (same palette)."*

**Also:** Meridien has darker top chrome than Lucid despite the shared palette — a
shell-level treatment, not a palette value.

**Registry gap:** `meridien` exists as a `StyleSystem` but **not** as a `ColorScheme`
(`builtin.rs` has aperture/cadence/alto/mariner/lucid only). Either register a
`meridien` scheme aliasing Lucid's values, or make the theme selector a
`(scheme, style)` pair with a named preset list. **Recommendation: named presets** —
users pick "Meridien", not a matrix coordinate. See Part 11.

---

## 6. Layout archetypes

The Law-1 fix. **Three** archetypes serve all six themes.

### A. Mosaic — Aperture only

One rounded envelope subdivided by flush hairlines. Tile grid for the dashboard;
bespoke trade view. `pane_gap: 7`, `pane_inset: -3`, leaf radius 0, frame radius `lg`.
`Chrome` already models gap/inset; the *frame-owns-the-round* rule is new.

Reference: `design-systems/aperture.md` §8 "Layout Architecture" — App Shell (721),
Tile Mosaic Grid (770), Page Grid (796), Trade View (813).

### B. Dense 3-column — Cadence only

Spotify-style. Left library rail / centre content / right now-playing-equivalent.
Continuous DOM ladder, 17px rows.

### C. Editorial dashboard — Alto, Mariner, Lucid, Meridien

The one the React team missed and then unlocked all four at once. Zones, top to bottom:

1. **Hero row** — large price + metric grid (mkt cap, P/E, vol, range, 52w, sentiment)
   + area chart (line + gradient fill — *not* candles)
2. **Three-column** — watchlist / chart / order book
3. **Utility row** — news / sector heatmap / time & sales / order ticket
4. **Footer** — big P&L stat cards

Per-shell variation is palette + chrome only:
`lucid` cream/terracotta · `meridien` cream + dark chrome + mono caps ·
`alto` warm-dark/amber · `mariner` steel-dark/blue, 10% tighter.

**Trading shells (Alto/Mariner) render candles in the centre chart while the hero stays
an area chart.** Light shells keep area throughout.

**Implementation note:** this is a *dashboard*, not the 9-pane trading grid. It is a
sibling to the existing workspace, selected by theme archetype. Given the frozen
`Watchlist`/`Chart` god-objects (`CLAUDE.md`, ADR-0001), new layout state goes on a
`state/` aggregate, not onto those structs.

**Scope honesty:** the React team measured this as ~85% of Lucid and Meridien being
*unbuilt* when approached as a recolour. Budget accordingly. This is the single
largest work item in the brief and the only one that is genuinely new construction.

---

## 7. Primitive finish specs

1. **Multi-layer bevel** — Part 4.3. Highest value per unit of work. Unblocks the
   crafted feel on Alto, Mariner, Cadence simultaneously.
2. **Label tier** — `label_font` (ui|mono) × `transform` (none|upper) × `weight` ×
   `tracking`. One struct, six configurations, visible on every panel header.
3. **Numeral tier** — `num_display` family + `num_tracking`. Aperture sans-tight is the
   outlier that proves the token is needed.
4. **Card recipe** — `radius` / `pad` / `border` (incl. `none`) / `shadow stack`, as one
   addressable recipe rather than four independent lookups. `design_system/recipes.rs`
   is the home.
5. **Value flash** — up/down flash with decay on live numerals
   (`--ds-flash-up/-down/-decay`). `ui_kit/widgets/motion.rs` exists.
6. **Sparkline-in-row** — watchlist rows carry inline mini-charts in the editorial
   themes. `sparkline.rs` exists; wire it into `panel_list_row`.

---

## 8. Verification protocol — build this FIRST

Nothing in Parts 5–7 is falsifiable without it. The React port's own post-mortem lists
**"optimistic reporting — summaries said themes were 'done' based on token application,
not structural comparison to source"** as a root cause of stall.

### Loop

1. **Serve the originals** — `cd ApexTerminalThemes && node server.js` → `:5173`.
   Capture reference PNGs per theme per screen.
2. **Screenshot the Rust app** — the repo has a `dev_inspector`-style harness and a
   `Ctrl+Shift+D` egui widget inspector; `bug_anchor.rs` backs `Ctrl+Shift+I`. Theme
   selection is `theme_idx` (palette) × `style_idx` (style system) — sweep both.
3. **Diff side-by-side**, per theme, per archetype screen.
4. **Gate on the diff, not on the token table.**

### Known harness traps (from prior sessions)

- Native windows cannot be observed by inspection alone — screenshots are mandatory,
  and a "clean build" is not evidence of a correct render.
- Zombie processes lock `apex-native.exe`; `cargo build` then silently fails to relink
  while `deps/` looks fresh. Kill stale processes before every build.
- Concurrent `cargo build` against the corpus produces phantom failures.
- A constant widget count across a resize sweep means the harness is broken, not that
  the UI is clean.

### Per-theme acceptance gate

A theme is **not** done until:

- [ ] Side-by-side screenshot vs original at 1440×900 and 2560×1440
- [ ] Surface ramp matches authored values (sample the actual pixels — don't trust the table)
- [ ] Control radius matches (Cadence pill / Meridien square are pass-fail)
- [ ] Label treatment matches (Meridien mono-caps is pass-fail)
- [ ] Bevel present and correct temperature (Alto warm vs Mariner cool)
- [ ] Hover state carries the right hue
- [ ] Correct layout archetype
- [ ] Light-theme parity walk (`CLAUDE.md` §6) for Lucid + Meridien
- [ ] No new `&THEMES[0]`, no hardcoded black shadows, no pinned chrome literals

---

## 9. Sequenced plan

Ordered by dependency. Each phase has a gate; do not start the next before it passes.

### Phase 0 — Verification harness *(blocks everything)*
Screenshot loop, reference capture for all 6 themes, side-by-side diff.
**Gate:** one command produces a reference-vs-current pair for any `(theme, screen)`.

### Phase 1 — Token contract
Extend `ColorScheme` with 8 `Option<Rgba>` authored fields (4.1). Extend `StyleSystem`
with control-radius, label tier, numeral tier, card recipe (4.2). Replace `ShadowSpec`
with `Vec<ShadowLayer>` (4.3). Update `theme_pack` manifest + `import/convert.rs`.
**Gate:** `equivalence_tests.rs` green; all ~22 existing schemes render byte-identical
(every new field `None` ⇒ old derivation).

### Phase 2 — Author the six DS schemes
Transcribe `global.css:145-530` into `builtin.rs`. Add `meridien` as a scheme.
Recalibrate all six `StyleSystem`s from current `global.css` (they were ported from the
15–35% era).
**Gate:** pixel-sample every ramp step against the table in Part 5.

### Phase 3 — Bevel + finish primitives
Multi-layer shadow rendering in `shadow_pipeline.rs`; label tier; numeral tier; card
recipe.
**Gate:** Alto and Mariner distinguishable at a glance by bevel temperature alone.

### Phase 4 — Archetype A + B polish
Aperture mosaic (frame-owns-round). Cadence dense-3-col (pill controls, continuous DOM
ladder, sparkline TREND column).
**Gate:** ≥85% side-by-side on both.

### Phase 5 — Archetype C *(the big one)*
`EditorialDashboard` shell + four brand shells. New layout state on a `state/`
aggregate — **not** on frozen `Watchlist`/`Chart`.
**Gate:** ≥85% on all four; light-parity walk clean.

### Phase 6 — Sweep and lock
Retire pinned literals found en route. Add regression snapshots per theme.
**Gate:** full acceptance checklist green ×6.

---

## 10. Rules

**Binding** (from `src-tauri/CLAUDE.md`, restated because this brief will tempt violations):

1. `render/pane/core.rs` is **sacred** — no design sweep touches it.
2. Never `&THEMES[0]`; thread `&dyn ComponentTheme`.
3. Never hardcode black for shadows — use `t.shadow_color` / `ShadowTint`.
4. Tokens, not literals (`mono_sm()` not `FontId::monospace(11.0)`).
5. `ui_kit::Button` over `egui::Button`.
6. Walk a light theme before claiming done — **doubly** binding here, since Lucid and
   Meridien are light.
7. No new fields on frozen `Watchlist` / `Chart`.

**Brief-specific:**

8. **Derive, don't pin.** Every Part-5 number is a token value.
9. **Author, don't synthesise** — for the six DS schemes. Derivation stays the fallback
   for the classic schemes.
10. **Structure before finish.** A perfectly-tokenised wrong layout scores 15%.
11. **No fidelity claim without a screenshot diff.**
12. **Siblings must be distinguishable** — Alto/Mariner, Lucid/Meridien.

---

## 11. Open decisions — need your call

1. **Theme selection model.** Today it's `theme_idx × style_idx` (a matrix). The design
   systems are *named pairings* — Meridien is one specific scheme + one specific style.
   Do we ship **named presets** (recommended: users pick "Meridien"; the matrix stays as
   an advanced/lab mode) or keep the raw matrix?
2. **Archetype selection.** Should layout archetype be a field on the theme (theme
   drives layout), a separate user choice (any theme × any layout), or fixed pairings?
   This determines whether the editorial dashboard is a *theme* or a *workspace preset*.
3. **Scope of Phase 5.** The editorial dashboard is genuinely new construction —
   measured at ~85% unbuilt when approached as a recolour. Ship all four editorial
   themes, or land Lucid first as a proof and defer the other three?
4. **Aperture's `card-border: none`.** Aperture cards rely purely on lift. On a pure-black
   canvas with our current shadow rendering this may read as invisible. Accept as-is and
   fix via bevel quality, or allow a hairline fallback?
5. **Serif display face.** Lucid's hero price wants DM Serif Display. Do we ship a third
   bundled font family, or substitute?

---

## Appendix A — file map

| What | Where |
|---|---|
| Colour schemes | `src-tauri/src/design_system/builtin.rs:110` |
| Style systems | `src-tauri/src/design_system/builtin.rs:842` |
| `ColorScheme` | `src-tauri/src/design_system/color_scheme.rs:119` |
| `StyleSystem` | `src-tauri/src/design_system/style_system.rs:881` |
| `Chrome` | `src-tauri/src/design_system/style_system.rs:683` |
| `Treatments` | `src-tauri/src/design_system/style_system.rs:512` |
| `ShadowSpec` | `src-tauri/src/design_system/style_system.rs:469` |
| `elevate()` | `src-tauri/src/ui_kit/style.rs:462` |
| Elevation constants | `src-tauri/src/ui_kit/style.rs:478-483` |
| Shadow rendering | `src-tauri/src/ui_kit/widgets/shadow_pipeline.rs` |
| Recipes | `src-tauri/src/design_system/recipes.rs` |
| Pack import | `src-tauri/src/design_system/import/convert.rs` |
| DS token blocks | `../ApexTerminalThemes/terminal/src/global.css:145-530` |
| DS specs | `../ApexTerminalThemes/design-systems/*.md` |
| Original apps | `../ApexTerminalThemes/Trading App - */` |
| Gallery server | `../ApexTerminalThemes/server.js` (port 5173) |
| Prior post-mortem | `../ApexTerminalThemes/terminal/FIDELITY-AUDIT.md` |
