# Theme System Spec — interchangeable `DesignSystem`

Status: **proposal / blueprint** — no code changes yet. This is the design
the implementation waves should follow.

## 1. Goal

Make a "theme" a complete, swappable **design system** — not just a colour
palette. The app must accept N externally-authored systems (currently 6,
authored in React, refined in Figma) and switch between them at runtime, each
carrying its own palette, type ramp, spacing, radii, stroke weights, density,
and shadows.

## 2. Current-state diagnosis

A theme today is split across three disconnected layers:

| Layer | Holds | Per-theme? |
|---|---|---|
| `gpu.rs::Theme` (×15 in `THEMES[]`) | palette colours only | yes |
| `style.rs` global fns (`font_*`, `gap_*`, `radius_*`, `stroke_*`, `alpha_*`, `elevation_*`) | type / spacing / radii / strokes / alpha | **no — global** |
| `StyleSettings` + `dt_f32!` (design-mode) | `r_sm`, `hairline_borders`, `cta_height_px`, density knobs | partially, design-mode only |

Consequence: switching theme swaps **colours only**. Type scale, spacing,
radii, density, stroke weight stay fixed. The 6 React systems differ in more
than colour, so the current architecture physically cannot express them.

In shipping builds (`design-mode` off) the `style.rs` token fns compile to
**constants** via `dt_f32!`'s `#[cfg(not(feature = "design-mode"))]` arm —
free at runtime. A data-driven theme system must preserve "effectively free"
token access (see §5).

## 3. The `DesignSystem` struct

One canonical struct. 100% data. `serde`-serializable. Nothing in globals.

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DesignSystem {
    pub meta:       Meta,
    pub palette:    Palette,
    pub typography: Typography,
    pub spacing:    Spacing,
    pub radii:      Radii,
    pub strokes:    Strokes,
    pub alphas:     Alphas,
    pub elevation:  Elevation,
    pub density:    Density,
    pub shadows:    Shadows,
}

pub struct Meta { pub id: String, pub name: String, pub is_dark: bool }

pub struct Palette {
    pub bg: Rgba, pub surface: Rgba, pub paper: Rgba,
    pub text: Rgba, pub dim: Rgba, pub border: Rgba,
    pub accent: Rgba, pub bull: Rgba, pub bear: Rgba, pub warn: Rgba,
    pub shadow: Rgba,
    // accent aliases for the top-bar picker (cobalt / green / black / amber)
    pub accent_alts: Vec<Rgba>,
}

pub struct Typography {
    pub mono_family: String,    // e.g. "JetBrains Mono"
    pub prop_family: String,    // e.g. "Inter Tight"
    pub size_2xs: f32, pub size_xs: f32, pub size_xs_plus: f32,
    pub size_sm: f32,  pub size_md: f32, pub size_md_plus: f32,
    pub size_lg: f32,  pub size_xl: f32,
    // line-height multipliers; egui has limited support — see §7
    pub line_tight: f32, pub line_normal: f32,
}

pub struct Spacing  { pub g2xs:f32, pub gxs:f32, pub gxs_mid:f32,
                      pub gsm:f32, pub gmd:f32, pub glg:f32,
                      pub gxl:f32, pub g2xl:f32, pub g3xl:f32 }

pub struct Radii    { pub xs:f32, pub sm:f32, pub md:f32, pub lg:f32, pub pill:f32 }

pub struct Strokes  { pub hair:f32, pub thin:f32, pub medium:f32,
                      pub std:f32, pub bold:f32 }

pub struct Alphas   { pub faint:u8, pub ghost:u8, pub soft:u8, pub subtle:u8,
                      pub tint:u8, pub muted:u8, pub dim:u8, pub line:u8,
                      pub strong:u8, pub active:u8, pub heavy:u8, pub solid:u8 }

// elevation tints are gamma multipliers on bg (dark-theme model); a light
// theme overrides with its own values rather than the audit's TODO gap.
pub struct Elevation { pub e1:f32, pub e2:f32, pub e3:f32 }

pub struct Density  { pub row_dense:f32, pub row_compact:f32, pub row_default:f32,
                      pub row_spacious:f32, pub row_tall:f32,
                      pub control_height:f32, pub cta_height:f32,
                      pub button_pad_x:f32, pub button_pad_y:f32 }

pub struct Shadows  { pub card:ShadowSpec, pub modal:ShadowSpec, pub tooltip:ShadowSpec }
```

`Rgba` is a plain `[u8;4]` (DTCG-friendly, no egui dependency in the schema).

The 6 React systems become **6 `DesignSystem` JSON files**. The current 15
`THEMES[]` palettes become 15 `DesignSystem`s that all share the *same*
typography/spacing/radii block (their existing behaviour) until intentionally
diverged.

## 4. Framework — four pieces

1. **Loader** — `DesignSystem::from_dtcg(json) -> DesignSystem`. Parses the
   W3C DTCG token JSON that Figma / Tokens Studio exports (§6).
2. **Registry** — `ThemeRegistry` owns all loaded `DesignSystem`s, tracks the
   active id, persists the choice through `UiSettings`/`Store<T>`.
3. **Resolver** — derived values (hover tints, `color-mix` equivalents,
   elevation surfaces, focus-ring colour) are **pure functions** of a
   `&DesignSystem`. Never stored, never a separate token.
   `fn tint(base, over, alpha) -> Rgba` is the one primitive.
4. **One trait** — widgets take `&DesignSystem` (extend or replace
   `ComponentTheme`). No widget reads a global token fn.

## 5. Performance — the load-bearing constraint

Token access must stay **effectively free** in the chart hot path.

### Rule 1 — widget code passes `&DesignSystem` by reference
Already the pattern for `&Theme`. `ds.spacing.gmd` is one in-cache pointer
deref. Identical cost to today's `&Theme` field reads. **Free.**

### Rule 2 — `style.rs` fns keep stable signatures, backed by a per-frame snapshot
`core.rs` (sacred) calls `style::font_sm()`, `style::gap_md()` etc. directly.
Those signatures **must not change** (no core.rs edits). Back them with a
`thread_local` snapshot refreshed once per frame:

```rust
thread_local! {
    static FRAME_DS: Cell<DesignSystemSnapshot> = Cell::new(DEFAULT_SNAPSHOT);
}
// render loop, once per frame, before draw_chart:
pub fn begin_frame(ds: &DesignSystem) { FRAME_DS.with(|c| c.set(ds.snapshot())); }

// style.rs — signature unchanged, body now reads the snapshot:
pub fn font_sm() -> f32 { FRAME_DS.with(|c| c.get().size_sm) }
```

`DesignSystemSnapshot` is a flat `Copy` struct of primitive token values
(no String, no Vec). A `thread_local` read of a `Copy` struct is ~1 ns and
lock-free. At ~5,000 token calls/frame that is ~5 µs — **0.03 % of a 16 ms
frame**. Imperceptible, and `core.rs` is never touched.

### Rule 3 — never a lock or map lookup per token call
No `RwLock::read()` per `font_sm()`. No `HashMap` lookup (today's design-mode
`design_tokens::get()` path is map-based — the snapshot replaces it and is
*faster* than the current design-mode path).

### Rule 4 — theme switch is a pointer swap
`registry.set_active(id)` swaps an `Arc<DesignSystem>`. Picked up by the next
frame's `begin_frame`. One-time, rare, nil cost.

**Net:** shipping-build perf is unchanged within noise. Design-mode builds get
*faster* (snapshot vs map lookup). The risk is exactly one anti-pattern —
per-call locking — which this spec forbids.

## 6. DTCG JSON shape (one worked example)

Tokens Studio exports / imports W3C DTCG. One file per `DesignSystem`:

```json
{
  "meta": { "id": "midnight", "name": "Midnight", "is_dark": true },
  "palette": {
    "bg":     { "$type": "color", "$value": "#0d0f14" },
    "accent": { "$type": "color", "$value": "#3b82f6" },
    "bull":   { "$type": "color", "$value": "#22c55e" },
    "bear":   { "$type": "color", "$value": "#ef4444" }
  },
  "typography": {
    "mono_family": { "$type": "fontFamily", "$value": "JetBrains Mono" },
    "size_sm":     { "$type": "dimension",  "$value": 11 },
    "size_md":     { "$type": "dimension",  "$value": 13 }
  },
  "spacing": { "gsm": { "$type": "dimension", "$value": 8 },
               "gmd": { "$type": "dimension", "$value": 12 } },
  "radii":   { "sm": { "$type": "dimension", "$value": 4 } }
}
```

In Figma each `DesignSystem` is one **mode** of a theme variable collection;
Tokens Studio round-trips it to this JSON unchanged. The Rust `Loader` reads
the same file. No hand-translation either direction.

## 7. Honest non-translations

Three things do not survive Figma → egui and must be flagged in the schema:

- **OpenType features** (`tnum`, `ss01`, slashed zero) — egui has no runtime
  toggle. Bake them into the shipped `.ttf`. Schema records them as metadata
  only.
- **Letter-spacing / per-style line-height** — egui can't express letter
  spacing and only coarsely supports line height. `line_tight/normal` are
  kept as best-effort; letter-spacing is dropped.
- **`color-mix()`** — computed at render via `tint()`. sRGB-blended. If the
  Figma source mixes in oklab, port `tint()` to oklab once, globally.

## 8. Migration plan — collapse three layers into one

| Wave | Work | Risk | Touches core.rs? |
|---|---|---|---|
| 1 | Define `DesignSystem` + `DesignSystemSnapshot` + `Loader` + `Registry`. No call-site changes. | Low | No |
| 2 | `begin_frame(ds)` snapshot pump; rewrite `style.rs` token fns to read the snapshot (signatures unchanged). | Med | No — signatures stable |
| 3 | Fold `StyleSettings` / `dt_f32!` into `DesignSystem`; design-mode inspector edits the active `DesignSystem`. | Med | No |
| 4 | Convert the 15 `THEMES[]` palettes → 15 `DesignSystem`s (shared type/spacing block initially). | Low | No |
| 5 | Extend `ComponentTheme` to expose the full `DesignSystem`; widgets migrate off bare palette access. | Med | No |
| 6 | Author the 6 React systems as DTCG JSON, load via `Registry`, wire the theme picker. | Low | No |

`core.rs` is never edited — Rule 2 guarantees the `style::` fn signatures it
calls stay identical. This is the same sacred-file discipline used for the
`Store<T>` state migration.

## 9. What this is NOT

- Not adopting the `rust-theme/theme.rs` scaffold from the Figma-export
  conversation — that was generated from a separate web mockup and is unaware
  of the 15 shipped themes + `ui_kit`. It would create a second, conflicting
  token system.
- Not a runtime CSS engine. No cascade, no selectors. A `DesignSystem` is
  plain data; widgets read it explicitly.
- Not recompiled per theme. A theme switch never triggers a build (see §10).

## 10. Install model — built-in `const` + installed JSON, one registry

This is how VSCode and JetBrains work, and it is the model here. Neither tool
recompiles to switch themes; a theme is always *data*, loaded at runtime. A
recompile-to-switch design would mean a 10–60 s build per colour change — and
it buys nothing: with the §5 per-frame snapshot, a compiled-`const` token and
a JSON-loaded token cost the *same* at the read site (~1 ns). Compiling a
theme in only saves a few ms of one-time startup parsing, at the cost of
making themes un-installable without a Rust rebuild.

So two theme sources feed **one `ThemeRegistry`**:

| Source | Form | Loaded | Why |
|---|---|---|---|
| **Built-in** (the final crafted set + the 15 existing) | compiled-in `const DesignSystem` | at startup, no file I/O | tamper-proof, never missing/corrupt, zero startup parse |
| **Installed / user** | DTCG JSON in a `themes/` dir | scanned at startup + on "Install theme…" | author / install without a rebuild |

"Installing" a theme = drop its DTCG JSON in the themes dir (or import via a
button) → the registry scans it → it appears in the picker. This is VSCode's
extension model minus the `.vsix` packaging. Both sources deserialize into the
same `DesignSystem`; the registry does not care which arm produced an entry.

Switching stays an instant `Arc<DesignSystem>` pointer-swap picked up by the
next frame's `begin_frame` (§5, Rule 4). No recompile, no relaunch, no UI
rebuild — just the next frame reading a new snapshot.

A built-in theme is the resolver's guaranteed fallback: if an installed JSON
fails to parse or references a missing field, the registry logs via
`errors_sink` and falls back to a named built-in rather than panicking.
