# Token Reference

Complete reference for every token in the apex-terminal design system. Derived
directly from the Rust source: `color_scheme.rs`, `style_system.rs`, and
`loader.rs` (the DTCG key names match these structs exactly).

Two axes:
- **ColorScheme** — palette axis (colors only, no dimensions)
- **StyleSystem** — dimension axis (sizes, spacing, behaviors; no colors)

Cells marked **RECIPE-CANDIDATE** are per-component tokens that can be overridden
per-key via the recipe system (see `docs/migration/recipe-keys.md`).

---

## ColorScheme tokens

DTCG file: `colorscheme.json` inside the `.apextheme` bundle.
Schema: `schema/colorscheme.schema.json`.

All values are DTCG color tokens: `{ "$type": "color", "$value": "#rrggbbaa" }`.
Optional fields (marked **optional**) are omitted from the DTCG output when `None`.
When absent in a loaded file, they fall back to `builtin_dark()` equivalents.

### Background layers

| Token key       | Type  | Default (dark)     | What it affects |
|-----------------|-------|--------------------|-----------------|
| `bg`            | color | `#121212ff`        | Deepest background — window/canvas floor |
| `surface`       | color | `#1c1c1cff`        | Elevated surface — panels, cards, toolbars |

### Text

| Token key       | Type  | Default (dark)     | What it affects |
|-----------------|-------|--------------------|-----------------|
| `text`          | color | `#dcdcdcff`        | Primary text / foreground |
| `dim`           | color | `#787878ff`        | Muted / secondary text, disabled labels, placeholders |
| `text_muted`    | color | `#aaaab4ff`        | Secondary body copy (distinct from dim) |
| `overlay_text`  | color | `#f0f0f0ff`        | Overlay / HUD foreground text |

### Structural chrome

| Token key       | Type  | Default (dark)     | What it affects |
|-----------------|-------|--------------------|-----------------|
| `border`        | color | `#373737ff`        | Borders, dividers, hairlines |
| `shadow`        | color | `#000000b4`        | Shadow tint (combined with Alphas at render time) |

### Semantic colors

| Token key            | Type  | Default (dark)     | Resolved fallback    | What it affects |
|----------------------|-------|--------------------|----------------------|-----------------|
| `accent`             | color | `#6366f1ff`        | —                    | Brand / interactive — buttons, focus rings, links |
| `bull`               | color | `#34d399ff`        | —                    | Positive price movement. Fallback for `resolved_success()` |
| `bear`               | color | `#f87171ff`        | —                    | Negative price movement. Fallback for `resolved_danger()` |
| `warn`               | color | `#fbbf24ff`        | —                    | Warning / caution. Fallback for `resolved_warning()` |
| `success` (**opt**)  | color | None               | falls back to `bull` | General success state — not price-specific. Set independently when you want a different hue from `bull` (e.g. green vs vivid teal) |
| `danger`  (**opt**)  | color | None               | falls back to `bear` | General danger / error / destructive action |
| `warning` (**opt**)  | color | None               | falls back to `warn` | General warning / caution (semantic alias) |
| `info`    (**opt**)  | color | None               | `#64a0dcff` (dark) / `#1e64b4ff` (light) | Informational — help text, info badges, links. No legacy equivalent |

> **Palette depth note (S1):** `success`, `danger`, `warning`, `info` are the
> independently-authored slots added in S1. Existing themes that leave them `None`
> are visually unchanged — the `resolved_*()` methods fall back to the
> trading-semantic equivalents. New themes may set them independently for richer UI
> states. The bull/bear aliases are: `bull` → success fallback, `bear` → danger
> fallback, `warn` → warning fallback.

### HUD / floating overlays

| Token key       | Type  | Default (dark)     | What it affects |
|-----------------|-------|--------------------|-----------------|
| `hud_bg`        | color | `#000000e6`        | Floating overlay background (premultiplied alpha) |
| `hud_border`    | color | `#32323cff`        | Floating overlay border |

### Extras (hand-authored per-theme)

| Token key             | Type  | Default (dark)     | What it affects |
|-----------------------|-------|--------------------|-----------------|
| `notification_red`    | color | `#e74c3cff`        | Alert / notification badge |
| `gold`                | color | `#ffc125ff`        | Gold accent — warm yellow for highlights |
| `pinned_row_tint`     | color | `#0000000c`        | Subtle tint behind pinned rows (premultiplied alpha) |
| `pane_gap_color` (**opt**) | color | None          | Override for gutter between adjacent panes. Absent = derive from `bg`/`border` |

### RRG quadrant colors

| Token key       | Type  | Default (dark)     | What it affects |
|-----------------|-------|--------------------|-----------------|
| `rrg_leading`   | color | `#34d399ff`        | RRG leading quadrant (strong bull) |
| `rrg_improving` | color | `#6366f1ff`        | RRG improving quadrant (trending up) |
| `rrg_weakening` | color | `#fbbf24ff`        | RRG weakening quadrant (warning, trending down) |
| `rrg_lagging`   | color | `#f87171ff`        | RRG lagging quadrant (strong bear) |

### Command palette badge colors

`cmd_palette` is a fixed-length array of 11 RGBA colors. Not yet round-tripped
through DTCG — always defaults to `CMD_PALETTE_DEFAULT` in the current loader.
Per-theme overrides can be applied by setting a different array in Rust.

---

## StyleSystem tokens

DTCG file: `stylesystem.json` inside the `.apextheme` bundle.
Schema: `schema/stylesystem.schema.json`.

All tokens are DTCG dimension/number/boolean/string tokens. Sections may be
omitted; missing tokens within a section fall back to `StyleSystem::builtin_default()`.

### Typography section (`typography.*`)

| Key                | Type   | Default     | Token function   | What it affects |
|--------------------|--------|-------------|------------------|-----------------|
| `size_xs`          | dim    | 9.0         | `font_xs()`      | Extra-small labels / annotations |
| `size_sm`          | dim    | 11.0        | `font_sm()`      | Small body / table cell text |
| `size_md`          | dim    | 13.0        | `font_md()`      | Medium body — primary reading size |
| `size_lg`          | dim    | 16.0        | `font_lg()`      | Large headings |
| `size_xl`          | dim    | 22.0        | `font_xl()`      | Extra-large display / hero headings |
| `mono_sm`          | dim    | 11.0        | `mono_sm()`      | Monospace small — code, numbers, timestamps |
| `mono_md`          | dim    | 13.0        | `mono_md()`      | Monospace medium |
| `mono_lg`          | dim    | 16.0        | `mono_lg()`      | Monospace large — price display |
| `size_section_label` | dim  | 9.0         | —                | Section / eyebrow label size (distinct from size_xs) |
| `label_tracking`   | dim    | 0.0         | —                | Letter-spacing (px) for general tracked-out labels |
| `nav_tracking`     | dim    | 0.0         | —                | Letter-spacing (px) for toolbar nav button text |
| `section_tracking` | dim    | 0.0         | —                | Letter-spacing (px) for section/eyebrow headers |
| `family_ui`        | string | `"Inter"`   | —                | UI / proportional family. Must match a loaded asset (S7) |
| `family_mono`      | string | `"JetBrains Mono"` | —         | Monospace family. Must match a loaded asset (S7) |
| `family_display`   | string | `"Inter"`   | —                | Display / hero family for size_xl headings |

### Spacing section (`spacing.*`)

| Key                 | Type | Default | Token function   | What it affects |
|---------------------|------|---------|------------------|-----------------|
| `xs`                | dim  | 4.0     | `gap_xs()`       | Tightest gap |
| `sm`                | dim  | 8.0     | `gap_sm()`       | Small gap |
| `xs_mid`            | dim  | 6.0     | `gap_xs_mid()`   | Micro-gap between xs and sm |
| `md`                | dim  | 12.0    | `gap_md()`       | Medium gap |
| `lg`                | dim  | 16.0    | `gap_lg()`       | Large gap |
| `xl`                | dim  | 20.0    | `gap_xl()`       | Extra-large gap |
| `xxl`               | dim  | 24.0    | `gap_2xl()`      | Double-extra-large gap |
| `gmd`               | dim  | 8.0     | `gap_md()` alias | Legacy alias — same value as sm |
| `cta_height`        | dim  | 28.0    | —                | Standard CTA / control height |
| `cta_padding_x`     | dim  | 12.0    | —                | Primary CTA button horizontal padding |
| `button_height`     | dim  | 24.0    | —                | Standard button height. **RECIPE-CANDIDATE** |
| `button_padding_x`  | dim  | 10.0    | —                | Standard button horizontal padding. **RECIPE-CANDIDATE** |
| `tab_height`        | dim  | 28.0    | —                | Tab strip height. **RECIPE-CANDIDATE** |

### Radii section (`radii.*`)

| Key    | Type | Default  | Token function   | What it affects |
|--------|------|----------|------------------|-----------------|
| `none` | dim  | 0.0      | —                | Sharp corners (the Meridien aesthetic) |
| `xs`   | dim  | 2.0      | `radius_xs()`    | Minimal rounding |
| `sm`   | dim  | 4.0      | `radius_sm()`    | Small rounding |
| `md`   | dim  | 6.0      | `radius_md()`    | Medium rounding |
| `lg`   | dim  | 12.0     | `radius_lg()`    | Large rounding |
| `full` | dim  | 9999.0   | —                | Conceptual maximum |
| `pill` | dim  | 99.0     | `r_pill()`       | Pill radius — 0 for sharp pills, 99 for round |
| `chip` | dim  | 0.0      | `r_chip()`       | Chip/badge corner radius. 0 = use sm. **RECIPE-CANDIDATE** |

### Strokes section (`strokes.*`)

| Key      | Type | Default | Token function      | What it affects |
|----------|------|---------|---------------------|-----------------|
| `hair`   | dim  | 0.3     | `stroke_hair()`     | Sub-pixel hairline (lightest separator) |
| `thin`   | dim  | 0.5     | `stroke_thin()`     | Sub-pixel thin border |
| `medium` | dim  | 0.8     | `stroke_medium()`   | Mid-weight border |
| `std`    | dim  | 1.0     | `stroke_std()`      | Standard 1px border |
| `bold`   | dim  | 1.5     | `stroke_bold()`     | Bold emphasis stroke |
| `thick`  | dim  | 2.0     | `stroke_thick()`    | Thick stroke |
| `md`     | dim  | 1.5     | —                   | Legacy alias for bold |
| `heavy`  | dim  | 2.0     | —                   | Legacy alias for thick |

### Alphas section (`alphas.*`)

**u8 tiers** (0–255, back `alpha_*()` token functions):

| Key         | Type | Default | Token function      | What it affects |
|-------------|------|---------|---------------------|-----------------|
| `faint`     | dim  | 10      | `alpha_faint()`     | Near-invisible overlay (hover shimmer) |
| `ghost`     | dim  | 15      | `alpha_ghost()`     | Barely-visible |
| `soft_u8`   | dim  | 20      | `alpha_soft()`      | Soft muted overlay (disabled states) |
| `subtle_u8` | dim  | 40      | `alpha_subtle()`    | Low-emphasis overlay |
| `tint`      | dim  | 48      | `alpha_tint()`      | Icon/chip accent tint |
| `muted_u8`  | dim  | 60      | `alpha_muted()`     | Primary dimming value |
| `dim`       | dim  | 60      | `alpha_dim()`       | Border/line dimming |
| `line`      | dim  | 80      | `alpha_line()`      | Structural line alpha |
| `strong_u8` | dim  | 80      | `alpha_strong()`    | Selected row fill |
| `active`    | dim  | 100     | `alpha_active()`    | Interactive element alpha |
| `heavy_u8`  | dim  | 120     | `alpha_heavy()`     | Near-opaque overlay |
| `scrim`     | dim  | 140     | `alpha_scrim()`     | Modal-backdrop / cmd-palette dimming |
| `solid`     | dim  | 200     | `alpha_solid()`     | High-opacity element |

**f32 multipliers** (0.0–1.0, resolver composites):

| Key             | Type | Default | What it affects |
|-----------------|------|---------|-----------------|
| `subtle`        | dim  | 0.04    | Hover shimmer, track backgrounds |
| `soft`          | dim  | 0.12    | Disabled states, secondary tint |
| `muted`         | dim  | 0.24    | Primary dimming |
| `mid`           | dim  | 0.48    | Ghost fills, inactive tab backgrounds |
| `strong`        | dim  | 0.72    | Selected row fill, active chip background |
| `opaque`        | dim  | 1.0     | Opaque override where transparency is unwanted |
| `header_border` | dim  | 0.18    | Header outer border alpha |

### Elevation section (`elevation.*`)

| Key  | Type | Default | What it affects |
|------|------|---------|-----------------|
| `l1` | dim  | 1.05    | Slightly raised surfaces (toolbar, column header). >1 brightens in dark themes |
| `l2` | dim  | 0.95    | Moderately raised (card, popover) |
| `l3` | dim  | 0.88    | Prominently raised (modal, tooltip) |

### Density section (`density.*`)

| Key                      | Type | Default | What it affects |
|--------------------------|------|---------|-----------------|
| `factor`                 | dim  | 1.0     | Scale factor for row heights and vertical gaps (0.8=compact, 1.2=comfortable) |
| `row_height_dense`       | dim  | 22.0    | Row height in compact list views (watchlist, scanner) |
| `row_height_comfortable` | dim  | 32.0    | Row height in comfortable list views |

### Shadows section (`shadows.*`)

Each shadow role is an object with five dimension sub-tokens:

| Shadow role | Default (blur / spread / offset_y / alpha) |
|-------------|---------------------------------------------|
| `card`      | 8 / 0 / 2 / 0.3 — subtle card lift |
| `modal`     | 24 / 0 / 8 / 0.5 — floating panel / dropdown |
| `tooltip`   | 6 / 0 / 2 / 0.4 — tooltip shadow |
| `dropdown`  | 12 / 0 / 4 / 0.4 — dropdown menu shadow |

Sub-token keys per role: `blur`, `spread`, `offset_x`, `offset_y`, `alpha`.
Color comes from `ColorScheme.shadow` at render time — not stored here.

### Treatments section (`treatments.*`)

All fields are DTCG boolean tokens except `focus_ring` (string token).

| Key                         | Type    | Default   | RECIPE-CANDIDATE | What it affects |
|-----------------------------|---------|-----------|------------------|-----------------|
| `solid_active_fills`        | boolean | false     |                  | Active/selected controls use solid fill (text-on-bg inversion) |
| `hairline_borders`          | boolean | false     |                  | Borders at thin (0.5px) instead of std (1px) |
| `uppercase_section_labels`  | boolean | false     |                  | Section/group labels rendered in uppercase |
| `segmented_filled_idle`     | boolean | false     |                  | Segmented control idle state uses filled background |
| `focus_ring`                | string  | "outline" |                  | Focus ring style: "none" / "outline" / "glow" |
| `serif_headlines`           | boolean | false     |                  | Serif family for hero numerics / display headings |
| `invert_active_fill`        | boolean | false     |                  | Invert palette on active elements |
| `vertical_group_dividers`   | boolean | false     |                  | Full-height vertical dividers between toolbar button clusters |
| `show_active_tab_underline` | boolean | true      | YES              | Accent underline shown in tab bars |
| `inactive_header_fill`      | boolean | true      |                  | Distinct recessed fill behind inactive pane headers |
| `nav_buttons_label_only`    | boolean | false     |                  | Drop icon glyphs from right-side toolbar nav buttons |
| `nav_buttons_uppercase_labels` | boolean | false  |                  | Toolbar nav button labels in ALL CAPS |
| `tab_underline_under_text`  | boolean | false     |                  | Tab underline directly under active tab text |
| `card_floating_shadow`      | boolean | false     |                  | Floating card shadow even when shadows_enabled=false |
| `shadows_enabled`           | boolean | true      |                  | Master toggle for drop shadows |
| `animations_enabled`        | boolean | true      |                  | false = snap all animations instant (reduce-motion) |

**Treatments not yet DTCG round-tripped** (default-only via from_dtcg):
`surface_bevel`, `bevel_highlight_alpha`, `bevel_shadow_alpha`,
`wl_row_side_margin`, `wl_row_corner_radius`, `wl_row_divider_alpha`,
`section_header_mono`, `wl_symbol_mono`, `panel_tab_treatment`,
`pane_active_fill_accent`, `button_treatment`. These are authored via the
internal Rust API or will be added to the DTCG spec in a future stream.

### Chrome section

`chrome` is not yet round-tripped through the DTCG file format. It is always
defaulted by `StyleSystem::from_dtcg`. Chrome values can only be set via the
internal Rust API (e.g. `StyleSystem::meridien()` or a custom built-in). A
future stream will add a `chrome` DTCG section.

Notable chrome fields for theme authors:

| Field                      | Default | What it affects |
|----------------------------|---------|-----------------|
| `toolbar_height_scale`     | 1.0     | Multiplier on toolbar height |
| `pane_gap`                 | 0.0     | Gap between adjacent panes (px). >0 = tiled card layout |
| `region_gap`               | 0.0     | Gap between major shell regions |
| `region_radius`            | 12.0    | Corner radius of shell region cards |
| `panel_footer_card`        | false   | Panel footer renders as elevated rounded card |
| `toolnav_height`           | 0.0     | Second toolbar row height. 0 = single-row chrome |
| `footer_default_open`      | false   | Bottom dock open by default for this style |
| `button_group`             | `None`  | Toolbar button-group enclosure style |
