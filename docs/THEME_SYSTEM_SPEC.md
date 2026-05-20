# Theme System Spec — two-axis `StyleSystem × ColorScheme`

Status: **proposal / blueprint** — no code changes yet. This is the design
the implementation phases should follow.

> **Revision (2026-05-20):** the monolithic `DesignSystem` of the first draft
> is replaced by a **two-axis model** — a theme is a `StyleSystem` (all
> dimensions) crossed with a `ColorScheme` (the palette), each selectable
> independently. See §3. This revision also adds **Phase 0 — Untangle &
> Unify** (§8), a prerequisite the monolithic model did not need.

## 1. Goal

Make a "theme" two orthogonal, swappable axes — not just a colour palette:

- **`StyleSystem`** — the structural design language: typography, spacing,
  radii, stroke weights, corner sharpness, density, elevation factors, shadow
  geometry, alpha values. **No colour.** (e.g. "Meridien", the 6 React systems.)
- **`ColorScheme`** — the palette only: bg, surface, text, accent, bull, bear,
  etc. (e.g. "Solarized", "Dracula", "Gruvbox", the ~20 existing `THEMES[]`.)

Picked independently → "Meridien in Dracula", "Meridien in Solarized" — same
layout language, different palette. N styles × M colorschemes = N·M valid
combinations, switchable at runtime, no recompile.

**This is making explicit what the app already half-has.** Today there is
`Theme` (the ~20 palettes — the colour axis) *and* `StyleSettings` + `style_id`
(Meridien is already a "style") — two things, tangled together and not
independently pickable. The refactor separates and orthogonalises them.

## 2. Current-state diagnosis

A theme today is split across three disconnected layers:

| Layer | Holds | Per-theme? |
|---|---|---|
| `gpu.rs::Theme` (×~20 in `THEMES[]`) | palette colours only | yes |
| `style.rs` global fns (`font_*`, `gap_*`, `radius_*`, `stroke_*`, `alpha_*`, `elevation_*`) | type / spacing / radii / strokes / alpha | **no — global** |
| `StyleSettings` + `dt_f32!` (design-mode) | `r_sm`, `hairline_borders`, `cta_height_px`, density knobs | partially, design-mode only |

Consequence: switching theme swaps **colours only**. Type scale, spacing,
radii, density, stroke weight stay fixed. The style axis cannot move.

In shipping builds (`design-mode` off) the `style.rs` token fns compile to
**constants** via `dt_f32!`'s `#[cfg(not(feature = "design-mode"))]` arm —
free at runtime. A data-driven theme system must preserve "effectively free"
token access (see §5).

### 2.1 Entanglement diagnosis (the two-axis blocker)

The final audit (2026-05-20) found the codebase is **not split-ready**: colour
and dimension are fused in **18 discrete sites** across 4 files. Until these
are untangled, "Meridien in Dracula" is physically impossible.

| Category | Count | Where |
|---|---|---|
| `Option<Color32>` fields living inside the *style* struct `StyleSettings` | 8 fields | `style.rs:1753–1815` — `active_fill_color`, `active_text_color`, `idle_outline_color`, `input_focus_color`, `pane_gap_color`, `segmented_idle_fill`, `segmented_idle_text`, `header_outer_border_alpha` |
| Elevation factors hardcoded as literals, not tokened | 6 sites | `style.rs:575,582,589,1412,1421,1430` — `0.95 / 0.88 / 0.85` |
| `Stroke::new(width, literal_color)` — dimension + colour fused | 2 sites | `spike_popup.rs:227,341` |
| Legacy black-shadow path | 1 site | `style.rs:1251` `paint_tooltip_shadow` |
| Parallel `Variant` enum (two divergent button colour-lookup paths) | 1 structural | `ui_kit/widgets/tokens.rs` vs `chart/renderer/ui/foundation/variants.rs` |

The `gpu.rs::Theme` struct itself is **clean** — pure colour, zero non-colour
fields. The `DesignTokens` struct is correctly segregated. The structural debt
is concentrated in `StyleSettings`, which was built as a style struct but
accumulated colour overrides over time.

## 3. The two structs

Two canonical structs. 100% data. `serde`-serializable. Nothing in globals.

```rust
/// Axis 2 — the palette. Pure colour. (Was `gpu.rs::Theme`.)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ColorScheme {
    pub meta:   Meta,                 // id, name, is_dark
    pub bg: Rgba, pub surface: Rgba, pub paper: Rgba,
    pub text: Rgba, pub dim: Rgba, pub border: Rgba,
    pub accent: Rgba, pub bull: Rgba, pub bear: Rgba, pub warn: Rgba,
    pub shadow: Rgba,
    pub accent_alts: Vec<Rgba>,
}

/// Axis 1 — the design language. Pure dimension. No colour.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StyleSystem {
    pub meta:       Meta,
    pub typography: Typography,
    pub spacing:    Spacing,
    pub radii:      Radii,
    pub strokes:    Strokes,
    pub alphas:     Alphas,      // alpha *values* — dimension-axis
    pub elevation:  Elevation,   // gamma *factors* — dimension-axis
    pub density:    Density,
    pub shadows:    Shadows,     // blur/offset/spread *geometry* — no colour
    pub treatments: Treatments,  // booleans/enums: solid_active_fills, hairline_borders, …
}
```

`Typography`, `Spacing`, `Radii`, `Strokes`, `Alphas`, `Elevation`, `Density`,
`Shadows` are unchanged from the first draft (see git history of this file).
`Treatments` is new — it absorbs the *non-colour* `StyleSettings` booleans
(`solid_active_fills`, `hairline_borders`, `uppercase_section_labels`, …).

`Rgba` is a plain `[u8;4]` (DTCG-friendly, no egui dependency in the schema).

### 3.1 The active theme is a *pair*

```rust
pub struct ActiveTheme { pub style: Arc<StyleSystem>, pub colors: Arc<ColorScheme> }
```

A token like "muted border" resolves as `colors.border` at
`style.alphas.muted` — the Resolver (§4) combines the two axes at render time.
Colour and dimension never meet until the resolver joins them.

### 3.2 Style fields specify *roles*, never literal colours

The fix for the 8 entangled `Option<Color32>` fields: a `StyleSystem` field
must never name a colour. It names a **role or a derivation**, and the
`ColorScheme` provides the pixels.

| Entangled field today | Two-axis replacement |
|---|---|
| `active_fill_color: Some(BLACK)` (Meridien) | `treatments.solid_active_fills: bool` — when true, active fill = `colors.text`, active text = `colors.bg` (palette inversion). Works for *any* colorscheme. |
| `active_text_color: Some(WHITE)` | (same — derived inversion) |
| `idle_outline_color: Some(rgb(60,56,44))` | derive from `colors.border` (optionally tinted via the resolver) |
| `input_focus_color`, `pane_gap_color`, `segmented_idle_fill`, `segmented_idle_text` | drop the override; derive from the palette role (`accent`, `surface`, …) |
| `header_outer_border_alpha: u8` | already an alpha — keep on the style axis; pairs with `colors.text` at render |

Result: Meridien × Dracula inverts Dracula's palette for active elements;
Meridien × Solarized inverts Solarized's. The style says *"invert for active"*;
the colorscheme says *"with these colours."*

## 4. Framework — four pieces

1. **Loader** — `StyleSystem::from_dtcg(json)` and `ColorScheme::from_dtcg(json)`.
   Parses W3C DTCG token JSON (§6). Two file *kinds*, one parser family.
2. **Registry** — `ThemeRegistry` owns all loaded `StyleSystem`s **and**
   `ColorScheme`s as two separate lists, tracks the active *pair*, persists the
   choice through `UiSettings`/`Store<T>`.
3. **Resolver** — derived values (hover tints, elevation surfaces, focus-ring
   colour, the active-element inversion of §3.2) are **pure functions** of
   `(&StyleSystem, &ColorScheme)`. Never stored. `fn tint(base, over, alpha)`
   is the one primitive.
4. **One trait** — widgets take `&ActiveTheme` (extend `ComponentTheme`
   additively). No widget reads a global token fn.

## 5. Performance — the load-bearing constraint

Token access must stay **effectively free** in the chart hot path.
**Confirmed by audit #198 (2026-05-20):** the main candle loop in `core.rs`
(the only path running ×200–500/frame) contains **zero** named token-fn calls;
the GPU pipeline (`renderer_gpu/`) is fully insulated (colours arrive as
resolved `[f32;4]`). Net frame-time change of the refactor: **≈ 0 µs**.

### Rule 1 — widget code passes `&ActiveTheme` by reference
`active.style.spacing.gmd` is one in-cache pointer deref. Identical cost to
today's `&Theme` field reads. **Free.**

### Rule 2 — `style.rs` fns keep stable signatures, backed by a per-frame snapshot
`core.rs` (sacred) calls `style::font_sm()`, `style::gap_md()` etc. directly.
Those signatures **must not change** (no core.rs edits). Back them with a
`thread_local` snapshot refreshed once per frame:

```rust
thread_local! {
    static FRAME: Cell<DesignSnapshot> = Cell::new(DEFAULT_SNAPSHOT);
}
// render loop, once per frame, before draw_chart:
pub fn begin_frame(t: &ActiveTheme) { FRAME.with(|c| c.set(t.snapshot())); }

// style.rs — signature unchanged, body now reads the snapshot:
pub fn font_sm() -> f32 { FRAME.with(|c| c.get().size_sm) }
```

`DesignSnapshot` is a flat `Copy` struct of the *resolved* pair's primitive
token values (no String, no Vec). A `thread_local` read is ~1 ns, lock-free.
Audit #198 measured the net added cost at ~30–120 ns/frame for the O(1)
setup-zone token calls; `style::current()` actually gets *faster* (RwLock
clone → Cell copy). `core.rs` is never touched.

### Rule 3 — never a lock or map lookup per token call
No `RwLock::read()` per `font_sm()`. No `HashMap` lookup.

### Rule 4 — a theme switch is two pointer swaps
`registry.set_style(id)` / `registry.set_colors(id)` swap an `Arc`. Picked up
by the next frame's `begin_frame`. One-time, rare, nil cost.

### Hoist list (audit #198) — 7 token reads in O(D≤50) loops
`core.rs:3290, 4220, 4223, 8780, 8781, 8805, 8847…8873` — token calls inside
the drawings / tooltip loops. Worth hoisting to locals before the loop. Even
unhoisted the overhead is <10 µs/frame; hoisting is hygiene, not correctness.

## 6. DTCG JSON shape

Tokens Studio exports / imports W3C DTCG. **Two file kinds**, one per axis:

```json
// colorscheme.dracula.json
{ "meta": { "id": "dracula", "name": "Dracula", "is_dark": true },
  "palette": {
    "bg":     { "$type": "color", "$value": "#282a36" },
    "accent": { "$type": "color", "$value": "#bd93f9" },
    "bull":   { "$type": "color", "$value": "#50fa7b" },
    "bear":   { "$type": "color", "$value": "#ff5555" } } }
```

```json
// style.meridien.json
{ "meta": { "id": "meridien", "name": "Meridien", "is_dark": true },
  "typography": { "size_sm": { "$type": "dimension", "$value": 11 } },
  "spacing":    { "gmd": { "$type": "dimension", "$value": 12 } },
  "radii":      { "sm": { "$type": "dimension", "$value": 0 } },
  "treatments": { "solid_active_fills": { "$type": "boolean", "$value": true } } }
```

In Figma each axis is its own variable collection; Tokens Studio round-trips
each to its JSON unchanged. No hand-translation either direction.

## 7. Honest non-translations

Three things do not survive Figma → egui and are flagged in the schema as
metadata only: **OpenType features** (`tnum`, `ss01`, slashed zero — bake into
the `.ttf`); **letter-spacing / per-style line-height** (egui can't express
letter spacing, coarse line height); **`color-mix()`** (computed at render via
`tint()`; port to oklab once if the Figma source needs it).

## 8. Phase 0 — Untangle & Unify (prerequisite)

The two-axis split cannot begin until colour and dimension are separable.
This phase did not exist in the monolithic draft.

| # | Work |
|---|---|
| 0a | **Unify the two `Variant` enums** → one canonical `ui_kit/widgets/tokens.rs`; migrate `lists/rows/*` off `foundation/variants.rs`; delete the legacy enum. |
| 0b | **Untangle the 8 `Option<Color32>` fields** out of `StyleSettings` — replace with the role/derivation model of §3.2. |
| 0c | **Token the elevation factors** — `0.95/0.88/0.85` (6 hardcoded sites) → `StyleSystem.elevation` fields. |
| 0d | Fix `spike_popup.rs:227,341` fused strokes + the legacy black shadow at `style.rs:1251`. |

## 9. The full phased plan

### Phase A — Styleability + Componentization
| # | Work |
|---|---|
| A1 | Build the **one** genuinely-missing primitive `Sparkline` (absorbs the hand-rolled impls in `perf_hud.rs:26` + `watchlist_columns.rs:100`). Adopt the 4 zero-consumer primitives where applicable: `StatusPill`, `Toast`, `TagInput`, `TimePicker`. *NOTE: audit #5 found `PriceBadge`/`StatTile`/`SectionDivider` are NOT needed — `MetricRow`/`PanelKeyValueRow`/`Separator` already cover those patterns.* |
| A2 | Migrate the ~75 `Variant::Chrome` leaks → named variants (`Chip`/`InlineClose`/`Tab`/`MutedIcon`, which exist but are unadopted); fix `PanelListRow` height drift (5 values: 14/18/22/28/52px); finish the `PanelSection` `trailing_buttons` migration. Consolidate the **3 duplicate widget clusters** (audit #5): text — `components/text.rs` + `semantic_label.rs` → `ui_kit::Label`; headers — `headers_widget.rs` → `ui_kit::Header`; buttons — `action_button.rs` / `header_buttons.rs` → `Button::` presets. Verify the `components/motion.rs` (190 sites) vs `ui_kit/motion.rs` overlap before retiring either. |
| A3 | Rebuild the hand-rolled UI on primitives. Audit #4's complete 138-file census is the authoritative roster — ~46 `hand-rolled`/`mixed` files, including the entire `components/` subtree, `command_palette/`, `lists/cards/`, `inputs/form.rs`, `top_nav.rs` — all missed by earlier lens-based audits. Worst two: `chart_widgets.rs` (103 hardcoded values) and `style.rs` (token infra — its 38 colours / 24 strokes are addressed by Phase B). Fold the ~48 private mini-widgets into primitives. |
| A4 | Wire remaining hardcoded sites to the theme — `shadow.rs` (active light-theme bug), `button.rs`, `form.rs` mini-`Theme`, ui_kit literals, `chart_widgets`. |
| A5 | `core.rs` sacred wire-ups — vol-delta→`t.bull/bear`, gold→`t.warn`, 211 strokes→tokens. Single-owner. **Perf-confirmed safe** (audit #198). |

### Phase B — The two-axis theme system
| Wave | Work | Touches core.rs? |
|---|---|---|
| B1 | Define `StyleSystem` + `ColorScheme` + `DesignSnapshot` + `Loader` + `Registry`. No call-site changes. | No |
| B2 | `begin_frame` snapshot pump; rewrite `style.rs` token fns to read the snapshot (signatures unchanged). | No — Rule 2 |
| B3 | Fold the 62 clean `StyleSettings` dimension fields → `StyleSystem`; design-mode inspector edits the active pair. | No |
| B4 | Convert the ~20 `THEMES[]` palettes → ~20 `ColorScheme`s. | No |
| B5 | Extend `ComponentTheme` additively to expose `&ActiveTheme`; widgets migrate off bare palette access. | No |
| B6 | Author the 6 React systems as DTCG `StyleSystem` JSON + decompose into style/colorscheme pairs; load via `Registry`; wire the two-dropdown theme picker. | No |

`core.rs` is never edited in Phase B — Rule 2 guarantees the `style::` fn
signatures stay identical. Phase A5 is the *only* sanctioned `core.rs` work,
done as a single verified owner.

## 10. Audits — complete & converged (2026-05-21)

Five audits run. **Audit #5's verdict: the inventory has converged — start Phase 0.**

- **#1 styleability** — hardcoded values; **#198 perf** — chart hot-path safety
  (net ≈ 0 µs/frame); **#199 consistency** — usage drift + two-axis readiness.
- **#4 exhaustive consumer census** — all 138 UI files classified; found ~25
  hand-rolled files the lens-based audits missed (the `components/` subtree,
  `chart_widgets.rs`, `inputs/form.rs`, `command_palette/`, `lists/cards/`).
- **#5 library audit** — 60+ primitives catalogued; 4 zero-consumer primitives
  (`TagInput`, `TimePicker`, `Toast`, `StatusPill`); 3 duplicate widget
  clusters; the two `Variant` enums fully characterised; `ComponentTheme` is
  23 methods (not 66), all colour-returning, correctly shaped for B5; only
  `Sparkline` genuinely missing. Found exactly **one** new file
  (`foundation/design_inspector.rs`, a dev tool) — convergence confirmed.

No further audits needed. The roster is stable.

## 11. What this is NOT

- Not adopting the `rust-theme/theme.rs` scaffold from the Figma-export
  conversation — it is unaware of the shipped themes + `ui_kit` and would
  create a second, conflicting token system.
- Not a runtime CSS engine. No cascade, no selectors. Plain data.
- Not recompiled per theme. A switch never triggers a build (see §12).

## 12. Install model — built-in `const` + installed JSON, one registry

VSCode / JetBrains model: a theme is always *data*, loaded at runtime, never
recompiled. With the §5 per-frame snapshot a compiled-`const` token and a
JSON-loaded token cost the *same* at the read site (~1 ns).

Two sources feed **one `ThemeRegistry`** (per axis):

| Source | Form | Loaded |
|---|---|---|
| **Built-in** (the crafted set + the ~20 existing) | compiled-in `const` | at startup, no file I/O |
| **Installed / user** | DTCG JSON in a `themes/` dir | scanned at startup + on "Install…" |

"Installing" = drop a DTCG JSON in the themes dir → the registry scans it → it
appears in the relevant axis of the picker. Switching is two `Arc` pointer
swaps picked up by the next frame's `begin_frame`. A built-in theme is the
resolver's guaranteed fallback if an installed JSON fails to parse.
