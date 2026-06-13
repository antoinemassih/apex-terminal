# Figma ⇄ apex-terminal design-system contract

These files make `ApertureDesign` (and future Figma work) **transcribable** into the Sx
design system instead of interpreted. Three name-matched layers:

| Layer | File | Maps to code |
|---|---|---|
| **Tokens — COMPLETE** (color/dim/type/shadow/chrome/treatments) | `apex.tokens.json` | All DTCG schema fields — 339 tokens, round-trip ready |
| **Tokens — legacy partial** | `aperture.tokens.json` | Original Aperture-only subset (kept for reference) |
| **Components + variants** | `component-inventory.md` | `ui_kit/widgets/*` + their recipe enums |
| **Frames** | `component-inventory.md` | chart-side layout regions (`top_nav`, `bottom_dock`, …) |

---

## 1. Import the tokens (→ Figma Variables)

Use **`apex.tokens.json`** — the complete, engine-matched inventory. `aperture.tokens.json` is
an older partial file kept for reference only; do not import it for new work.

1. In Figma, install the **Tokens Studio (Figma Tokens)** plugin.
2. Plugin → Import → paste / upload `apex.tokens.json`.
3. Plugin → **Export to Figma Variables**. The importer will create variable collections per
   top-level group. Recommended collection mapping:
   - `color` → collection **"ColorScheme"** (Aperture values as the primary mode)
   - `typography`, `spacing`, `radii`, `strokes`, `alphas`, `elevation`, `density`, `shadows`,
     `treatments`, `chrome` → collection **"StyleSystem"** (Aperture values as the primary mode)
   - `cmdPalette` → collection **"CmdPalette"** (single mode; rarely changes)
   - `textStyle` → Figma **Text Styles** (use "Publish as text styles" in Tokens Studio)
   - `effectStyle` → Figma **Effect Styles** (use "Publish as effect styles")
4. Strip the `_comment` / `_about` / `_modes` / `_u8_comment` / `_f32_comment` /
   `_semantics` keys if the importer complains (they are doc-only meta keys, not tokens).

## 2. Wire the two axes to **Modes** (this is the whole point)

The engine has two independent theme axes. Each maps to a Figma Variable collection with modes:

### Color axis → `ColorScheme`
- Collection **"ColorScheme"**, primary mode name **`Aperture`**.
  → *a Figma mode = a `ColorScheme` in code.*
- To add **Meridien**: duplicate the mode in Figma and copy values from the `colorMeridien`
  group in `apex.tokens.json` (indigo accent, Emerald bull, near-black bg, etc.).
- Additional built-in palettes in `builtin.rs` (Nord, Dracula, Catppuccin, Gruvbox, etc.)
  can each become additional modes of this collection.

### Dimension axis → `StyleSystem`
- Collection **"StyleSystem"**, primary mode name **`Aperture`**.
  → *a Figma mode = a `StyleSystem` preset in code.*
- To add **Meridien**: duplicate the mode and copy values from `spacingMeridien`,
  `radiiMeridien`, `strokesMeridien`, `typographyMeridien`, `densityMeridien`,
  `shadowsMeridien`, `treatmentsMeridien`, `chromeMeridien`.
- Meridien key signature: sharp corners (radii.xs=2), hairline borders, tall toolbar,
  no floating-card chrome, single-row, uppercase labels, serif hero.
- Aperture key signature: large soft radii (xs=8/sm=10/md=14/lg=20), pill nav clusters,
  2-row chrome, 8px pane gaps, orange accent fills on active headers.

> Switching a Figma mode == switching a theme axis in the running app, with the **same names and values**.

### Round-trip guarantee
Every token group name and every leaf key in `apex.tokens.json` matches the DTCG schema field
names (`colorscheme.schema.json` and `stylesystem.schema.json`) exactly. A design hand-off
that specifies `spacing.cta_height = 40` maps to `StyleSystem.spacing.cta_height = 40.0` in
Rust with zero translation. The schema enforces: `typography.*`, `spacing.*`, `radii.*`,
`strokes.*`, `alphas.*`, `elevation.*`, `density.*`, `shadows.*.*`, `treatments.*`,
`chrome.*` on the dimension axis; and all `color.*` fields on the color axis.

## 3. Build components against the variables

Build each component in `component-inventory.md` with its Variant props, and **bind every fill /
corner / padding / text to a variable** — never a raw hex or px. Pills use `radius.full`; the Aperture
group look uses `ToolGroup(enclosure=bordered)` with the active-tab swatch + `spacing.tab-overlap`.

## 4. The workflow loop

```
design in Figma (named vars + named components)
      │  right-click frame → Copy link to selection (gives node-id)
      ▼
paste the link to me
      ▼
I read it via the Figma MCP → variable names tell me Tone/tier/variant
      ▼
I implement against existing widgets + set the Aperture ColorScheme / StyleSystem
```

---

## What crosses over — and what doesn't

✅ Color, ramps, overlays, radius, spacing, stroke, font sizes, type styles, shadows, component variants, layout.
❌ **Motion / easing** (hover-press timing) — no Figma concept; stays a code convention.
⚠️ **Custom-graphics** widgets (Sparkline, RiskRewardBar, sliders' tracks) — I match their *colors* from
tokens but the drawing is hand-coded; don't expect pixel-transcription there.

## The golden rule
A layer whose **fill is a named color variable**, **corner is a radius variable**, and is an instance of a
**named component with a known variant** → I transcribe 1:1. Raw hex / raw px / unnamed groups → I guess.
Minimise the second kind.
