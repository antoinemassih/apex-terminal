# Figma ⇄ apex-terminal design-system contract

These files make `ApertureDesign` (and future Figma work) **transcribable** into the Sx
design system instead of interpreted. Three name-matched layers:

| Layer | File | Maps to code |
|---|---|---|
| **Tokens** (color/dim/type/shadow) | `aperture.tokens.json` | Sx `Tone`, token tiers, `ComponentTheme` fields, Effect styles |
| **Components + variants** | `component-inventory.md` | `ui_kit/widgets/*` + their recipe enums |
| **Frames** | `component-inventory.md` | chart-side layout regions (`top_nav`, `bottom_dock`, …) |

---

## 1. Import the tokens (→ Figma Variables)

1. In Figma, install the **Tokens Studio (Figma Tokens)** plugin.
2. Plugin → Import → paste / upload `aperture.tokens.json`.
3. Plugin → **Export to Figma Variables**. You now have variable collections:
   `color`, `radius`, `spacing`, `stroke`, `fontSize`, plus `typography` text styles and
   `shadow` effect styles.
4. Strip the `_comment` / `_about` keys if the importer complains (they're docs only).

## 2. Wire the two axes to **Modes** (this is the whole point)

- On the **color** collection: rename its mode to **`Aperture`**. → *a Figma mode = a `ColorScheme` in code.*
  To add **MERIDIAN**: duplicate the mode, swap only the `color.*` / `colorExtended.*` values
  (Black bg, Vulcan `#11141D` surface, Emerald bull, etc.).
- On **radius/spacing/stroke/fontSize** (the dimension side): the mode = a **`StyleSystem`** preset
  (`Aperture`, `Meridien`, `Glass`, …). Duplicate + retune numbers per style.

> Switching a Figma mode == switching a theme axis in the running app, with the **same names and values**.

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
