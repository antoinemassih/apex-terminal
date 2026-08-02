# 05 — Token Surface Reference

The complete token surface of `apex-terminal`, and a per-theme matrix of what is
expressible today versus what needs Change A–E from
[`02-TOKEN-CONTRACT.md`](02-TOKEN-CONTRACT.md).

Everything here was read out of `src-tauri/src/design_system/style_system.rs`,
`color_scheme.rs` and `ui_kit/style.rs` on 2026-08-02. **[verified]** throughout.

**Use this document to answer "is there already a token for X?" before writing any code.**

---

## 1. The two axes at a glance

```
ColorScheme                        StyleSystem
├─ meta (id, name, is_dark)        ├─ meta
├─ 9 core colours                  ├─ typography   (15 fields)
├─ 4 optional semantics            ├─ spacing      (13 fields)
├─ 14 special-purpose colours      ├─ radii        ( 8 fields)
└─ cmd_palette[11]                 ├─ strokes      ( 8 fields)
                                   ├─ alphas       (~14 fields)
        + RecipeSet                ├─ elevation    ( 3 fields)
     (per-component overrides)     ├─ density      ( 3 fields)
                                   ├─ shadows      ( 4 roles)
                                   ├─ treatments   (27 flags)  ← personality
                                   └─ chrome       (43 knobs)  ← geometry
```

Joined by the Resolver at render time. Contrast validation runs on `ColorScheme` alone.

---

## 2. `ColorScheme` — the palette axis

### 2.1 Core

| Field | Type | Notes |
|---|---|---|
| `meta.id` / `.name` / `.is_dark` | | `is_dark` drives every direction-aware derivation |
| `bg` | `Rgba` | canvas |
| `surface` | `Rgba` | raised surface (toolbar bg) |
| `text` | `Rgba` | primary ink |
| `dim` | `Rgba` | secondary ink |
| `text_muted` | `Rgba` | tertiary ink |
| `border` | `Rgba` | hairline |
| `accent` | `Rgba` | |
| `bull` / `bear` | `Rgba` | market direction |
| `warn` | `Rgba` | |

### 2.2 Optional semantics — alias chain

| Field | Falls back to |
|---|---|
| `success` | `bull` |
| `danger` | `bear` |
| `warning` | `warn` |
| `info` | muted blue (dark `#64a0dc`, light `#1e64b4`) |

Read via `resolved_*()`. **This is the pattern Change A follows** — every new optional
field gets a `resolved_` accessor and call sites never see the `Option`.

### 2.3 Special-purpose

`pane_gap_color` (opt) · `shadow` · `notification_red` · `gold` · `overlay_text` ·
`rrg_leading` / `rrg_improving` / `rrg_weakening` / `rrg_lagging` · `pinned_row_tint` ·
`hud_bg` · `hud_border` · `cmd_palette[11]`

> `shadow` is why "never hardcode black" is enforceable — light themes carry a soft grey.

### 2.4 ❌ Missing (Change A + C)

`bg_panel` · `bg_elevated` · `bg_hover` (hue!) · `fg_xmuted` · `accent_sub` ·
`bull_alpha` · `bear_alpha` · `border_dim` · `bevel_highlight` · `bevel_shadow`

---

## 3. `StyleSystem` — the dimension axis

### 3.1 `Typography` (15)

`size_xs` `size_sm` `size_md` `size_lg` `size_xl` · `mono_sm` `mono_md` `mono_lg` ·
`size_section_label` · `label_tracking` `nav_tracking` `section_tracking` ·
`family_ui` `family_mono` `family_display`

❌ **No weight fields at all.** ❌ 5 UI sizes vs the DS's 7. ❌ No numeral family.

### 3.2 `Spacing` (13)

`xs`(2) `sm`(4) `xs_mid`(6) `md`(8) `lg`(12) `xl`(16) `xxl`(24) `gmd` ·
`cta_height` `cta_padding_x` · `button_height` `button_padding_x` · `tab_height`

❌ **No card padding.**

### 3.3 `Radii` (8)

`none` `xs` `sm` `md` `lg` `full` · **`pill`** · **`chip`**

> `pill` doc comment: *"Pill radius as the runtime `r_pill` value (px, 0–99). Distinct from
> `full`: **Meridien uses 0 (sharp pill), Aperture/Octave use 99 (rounded pill)**."*
> `chip`: *"0 = use `sm`."*

⚠️ **See §6 — `pill` is correct in the struct and broken at one of its two consumers.**

### 3.4 `Strokes` (8)

`hair`(0.3) `thin`(0.5) `medium` `std`(1.0) `bold` `thick` `md` `heavy`

### 3.5 `Alphas` (~14)

u8 tier: `faint`(10) `ghost`(15) `tint`(48) `dim`(60) `line`(80) `active`(100)
`scrim`(140) `solid`(200)
f32 tier: `subtle`(.04) `soft`(.12) `muted`(.24) `mid`(.48) `strong`(.72) `opaque`(1.0)
plus `header_border`(.18)

### 3.6 `Elevation` (3)

`l1`(1.05) `l2`(0.95) `l3`(0.88) — *"Factor > 1 in dark themes (brightens), < 1 in light"*

> ⚠️ Legacy gamma path. Superseded for surfaces by `elevate()` (2026-07-30). Verify nothing
> still multiplies against a near-black canvas — `0 × 0.95 = 0`.

### 3.7 `Density` (3)

`factor` `row_height_dense` `row_height_comfortable`

### 3.8 `Shadows` (4 roles)

`card` `modal` `tooltip` `dropdown`, each a **single** `ShadowSpec { blur, spread,
offset_x, offset_y, alpha }` — outer only, no inset, no colour.

❌ **Change E.** Blocks light themes too: Lucid and Aperture each need **two outer layers**.

### 3.9 `Treatments` (27 flags) — the personality axis

| Flag | Type | What it buys you |
|---|---|---|
| `surface_bevel` | `BevelStyle` | `None` flat / `Raised` Zed face / `Inset` sunken well |
| `bevel_highlight_alpha` | `u8` | top highlight intensity |
| `bevel_shadow_alpha` | `u8` | bottom shadow intensity |
| `solid_active_fills` | `bool` | palette-inverted active state |
| `hairline_borders` | `bool` | `thin` vs `std` |
| `uppercase_section_labels` | `bool` | **Meridien caps** |
| `section_header_mono` | `bool` | **Meridien/Alto/Mariner mono headers** |
| `wl_symbol_mono` | `bool` | mono list symbols |
| `serif_headlines` | `bool` | **Lucid serif hero** |
| `segmented_filled_idle` | `bool` | |
| `focus_ring` | `FocusRingStyle` | `None`/`Outline`/`Glow` |
| `wl_row_side_margin` | `f32` | *"0 flush; 6 Aperture pill rows; 4 Glass"* |
| `wl_row_corner_radius` | `u8` | *"0 square; 99 full pill"* |
| `wl_row_divider_alpha` | `u8` | |
| `panel_tab_treatment` | `u8` | Line/Segmented/Filled/Card/Pane |
| `pane_active_fill_accent` | `bool` | **"Aperture signature — orange bar"** |
| `button_treatment` | `u8` | SoftPill/OutlineAccent/UnderlineActive/RaisedActive/BlackFillActive |
| `invert_active_fill` | `bool` | |
| `vertical_group_dividers` | `bool` | |
| `show_active_tab_underline` | `bool` | default true |
| `inactive_header_fill` | `bool` | default true |
| `nav_buttons_label_only` | `bool` | |
| `nav_buttons_uppercase_labels` | `bool` | |
| `tab_underline_under_text` | `bool` | |
| `card_floating_shadow` | `bool` | |
| `shadows_enabled` | `bool` | master |
| `animations_enabled` | `bool` | reduce-motion |

❌ Bevel **tint** is luminance-derived, not authored → Change C.

### 3.10 `Chrome` (43 knobs) — geometry and finish

**Regions:** `toolbar_height_scale` `header_height_scale` `account_strip_height`
`toolnav_height` `footer_default_open` `region_gap` `region_radius` `region_border_alpha`

**Panes:** `pane_border_width` `pane_gap` `pane_gap_alpha` `pane_active_indicator`
`active_header_fill_multiply` `inactive_header_fill_multiply` `header_outer_border_alpha`
`header_outer_border_width` `header_divider_alpha`

**Nav:** `nav_active_col_alpha` `nav_cluster_radius` `nav_cluster_fill_alpha`
`nav_cluster_padding` `button_group`

**Tabs:** `tab_inactive_alpha` `tab_hover_bg_alpha` `tab_underline_thickness`

**Panels:** `panel_header_treatment` `panel_section_fill_alpha` `panel_footer_card`
`panel_footer_radius` `section_label_padding_top` `section_label_padding_bottom`

**Interaction:** `hover_bg_alpha` `active_bg_alpha` `focus_ring_width` `focus_ring_alpha`
`disabled_opacity` `accent_emphasis`

**Misc:** `dialog_backdrop_alpha` `drag_handle_alpha` `drag_handle_dot_scale`
`toast_bg_alpha` `card_stripe_alpha` `card_floating_shadow_alpha`

### 3.11 Typed enums

```rust
enum PaneActiveIndicator { None, TopStripe, HeaderFill, Both }        // u8 at boundaries
enum PanelHeaderTreatment { Line, Segmented, Filled, Card, Pane }     // u8 at boundaries
enum BevelStyle          { None, Raised, Inset }
enum FocusRingStyle      { None, Outline, Glow }
enum GroupEnclosure      { None, Bordered, Frosted, Sharp }
```

> `GroupEnclosure` is the **canonical extension pattern**: *"the concrete look lives as
> composed `Sx` at the render site, not as data threaded through the style pipeline — so a
> new treatment is one new variant here plus its `Sx` recipe, with no schema change."*
> **Copy this pattern for every new visual treatment.**

---

## 4. User-level overrides — orthogonal to both axes

**[verified: `ui_kit/style.rs:191-206`]**

| Override | Values | Applies to |
|---|---|---|
| `corner_scale_override()` | Sharp 0.0× / Subtle / Standard 1.0× / Round | `radius_xs/sm/md/lg` — ⚠️ **not `radius_pill()`** |
| `border_weight_override()` | Hairline 0.5× / Standard / Bold 1.5× | all `stroke_*()` |

Sharp = 0.0× returns zero for every tier — *"square-corner Meridien aesthetic"*. A user can
therefore square a rounded theme. **Do not fight this**; test your theme at Sharp and Round.

---

## 5. `TokenSnapshot` — the per-frame delivery mechanism ⚠️

**[verified: `ui_kit/style.rs:181`]**

```rust
pub fn frame_tokens() -> TokenSnapshot { FRAME_TOKENS_LOCAL.with(|c| c.get()) }
```

Thread-local, pushed once per frame. Source comment: *"Hosts that don't push a snapshot get
the `DEFAULT_TOKEN_SNAPSHOT` values, which match the stand-alone constants below."*

**The trap:** a host that never pushes a snapshot silently renders defaults. The surface
looks half-themed; the token is fine; the *plumbing* is missing. When a panel refuses to
follow the theme, check snapshot-push before you check the token.

Every accessor (`gap_xs_mid()`, `radius_md()`, `stroke_thin()`, …) reads through this.

---

## 6. ⚠️ The `radius_pill` split-brain

**[verified: `chart/renderer/ui/foundation/shell.rs:18-21` — the code documents its own bug]**

> *"Pill reads `StyleSettings.r_pill` which varies per style preset (e.g. Meridien
> r_pill = 0); the ui_kit equivalent `radius_pill()` is a fixed 999.0 constant with no
> preset awareness. Unifying requires the style-axis decision deferred to Phase 5."*

| Consumer | Source | Preset-aware? | Corner-scale-aware? |
|---|---|---|---|
| `foundation::shell::Radius::Pill` | `StyleSettings.r_pill` | ✅ | ❓ |
| `ui_kit::style::radius_pill()` | fixed `999.0` | ❌ | ❌ |

**Consequences:**
- Meridien's square controls are **impossible** in any ui_kit widget
- Adjacent controls from the two paths render **different radii in the same theme**
- The user's Sharp preference squares everything **except** pills

This is the single clearest instance of the "half-applied theme" failure mode, it is
already known to the codebase, and fixing it needs **zero new fields**. → **DS-2.1**

---

## 7. Per-theme expressibility matrix

Can the design system's signature be expressed **today**?
✅ yes · ⚠️ partly · ❌ no (needs the named change)

| Signature | Aperture | Cadence | Alto | Mariner | Lucid | Meridien |
|---|---|---|---|---|---|---|
| Canvas + accent | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ no scheme registered |
| 4-step surface ramp | ❌ A (warm) | ❌ A | ❌ A | ❌ A | ❌ A (non-monotonic) | ❌ A |
| 4-step ink ramp | ❌ A | ❌ A | ❌ A | ❌ A | ❌ A | ❌ A |
| Hover **hue** | ❌ A (orange) | ✅ neutral | ❌ A (amber) | ❌ A (steel) | ✅ neutral | ✅ neutral |
| Radius scale | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Control radius | ⚠️ B | ⚠️ B (pill) | ⚠️ B | ⚠️ B | ⚠️ B | ❌ B (square) |
| List-row shape | ✅ `wl_row_*` | ✅ | ✅ | ✅ | ✅ | ✅ |
| Uppercase labels | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Mono labels | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Serif hero | — | — | — | — | ✅ | — |
| **Numeral family** | ❌ D2 (sans!) | ✅ mono | ✅ | ✅ | ✅ | ✅ |
| Font weight | ❌ D3 | ❌ D3 | ❌ D3 | ❌ D3 | ❌ D3 | ❌ D3 |
| Surface bevel on/off | ✅ | ✅ | ✅ | ✅ | ✅ `None` | ✅ `None` |
| **Bevel temperature** | — | ✅ neutral | ❌ C (warm) | ❌ C (cool) | — | — |
| Card radius | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Card padding | ❌ D1 | ❌ D1 | ❌ D1 | ❌ D1 | ❌ D1 (20px) | ❌ D1 |
| Card **no border** | ❌ D1 | — | — | — | — | — |
| Card shadow stack | ❌ E (2 outer) | ❌ E (2) | ❌ E (4) | ❌ E (4) | ❌ E (2 outer) | ❌ E |
| Pane gap / inset | ✅ `Chrome` | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Frame-owns-round** | ❌ DS-5.1 | — | — | — | — | — |
| Active pane = accent bar | ✅ `pane_active_fill_accent` | ✅ | ✅ | ✅ | ✅ | ✅ |
| Density step | ✅ `Density` | ✅ | ✅ | ✅ (−10 %) | ✅ | ✅ (+1 step) |
| Type scale step | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ D4 audit |
| **Layout archetype** | ❌ DS-5.1 | ❌ DS-5.2 | ❌ DS-6 | ❌ DS-6 | ❌ DS-6 | ❌ DS-6 |

### What this table says

1. **`Treatments` and `Chrome` already carry most of the personality.** Uppercase, mono,
   serif, bevel on/off, row shape, active-pane accent bar, tab treatment — all present.
2. **Colour is the deepest token gap.** Every theme needs Change A; it is the only change
   all six require.
3. **Two changes are single-theme unlocks:** D2 (Aperture's sans numerals) and B
   (Meridien's square controls).
4. **Change C is a two-theme unlock and a distinguishability gate** — without it Alto and
   Mariner converge.
5. **Change E blocks all six**, including the light ones, because even a plain two-layer
   outer drop is inexpressible.
6. **Layout is the bottom row and it is red for every theme.** No token work moves it.
   That is Law 1 of the design brief, visible as a table row.

---

## 8. Decision guide — "do I need a new token?"

```
Is it a COLOUR?
  └─ yes → ColorScheme. Optional + resolved_ accessor. Grep the 14 special-purpose
           fields first — rrg_*, hud_*, gold, pinned_row_tint already exist.

Is it a per-theme BOOLEAN or a small enum of looks?
  └─ yes → Treatments. Grep all 27 flags first. If it is a new *look* rather than a new
           *switch*, prefer a new enum variant + render-site Sx recipe (GroupEnclosure
           pattern) over a new field.

Is it a DIMENSION (px, alpha, scale)?
  └─ yes → Chrome (43 knobs — grep first) or the relevant scale struct.

Is it PER-COMPONENT rather than global?
  └─ yes → RecipeSet (design_system/recipes.rs), not a new global token.

Is it USER preference rather than theme identity?
  └─ yes → the override layer (corner_scale / border_weight), not the theme.

Does an existing token already carry it but the value is wrong or the plumbing broken?
  └─ → THIS IS THE COMMON CASE. Fix the value or the plumbing. See §5 and §6.
```

**Two of five originally-proposed fields in this programme were reinventions, and one of
those reinventions was hiding a real bug.** Budget ten minutes of grepping per proposed
field; it has already paid for itself twice.
