# Theme Authoring Guide

This guide covers everything you need to author a `.apextheme` bundle for
apex-terminal — from concepts through a fully-worked example pack.

> **Theme Studio (S10):** The in-app Theme Studio generates these files
> interactively and exports a ready-to-install `.apextheme` bundle. This
> guide describes the underlying format so you can author packs by hand,
> build tooling, or understand what the Studio produces.

---

## Concepts

### Two-axis design system

Every theme is the product of two independent axes:

```
ColorScheme  (palette axis)   ×   StyleSystem  (dimension axis)
  colors, tints, semantics          sizes, spacing, radii, behaviors
```

They never mix until the **Resolver** joins them at render time. This means:

- A dark palette can wear any dimension style (sharp Meridien OR rounded Aperture).
- A single dimension style (e.g. monospaced Relay) can be paired with any palette.
- Validation rules (contrast, accessibility) are checked on the ColorScheme alone.

### Recipes — per-component overrides

On top of the two axes, **recipes** let you override how specific named components
look without changing the global palette or scale:

```
ColorScheme  ×  StyleSystem  +  RecipeSet
                                 button.primary  → pill radius, accent fill
                                 row.list        → no border, xs padding
                                 tab.line.active → 2px underline
```

A recipe is a set of optional field overrides for a named component. Missing
fields preserve the widget's built-in default. A missing key in the RecipeSet
leaves the widget entirely unchanged.

**Resolution chain per field:**

```
recipe override → widget built-in Sx default → no paint / transparent
```

**Resolution chain for semantic colors:**

```
success → (if None) → bull
danger  → (if None) → bear
warning → (if None) → warn
info    → (if None) → muted blue (dark: #64a0dc, light: #1e64b4)
```

This alias chain means existing themes that only set `bull`/`bear`/`warn` are
visually unchanged — the `resolved_*()` accessors fall back automatically.

### Shell profile (placeholder)

`shell_profile` is a forward-compatible JSON blob for future shell layout
configuration (panel arrangement, workspace presets). It is stored verbatim
in the `.apextheme` bundle but not yet parsed by the reader. Leave it absent.

---

## The `.apextheme` bundle layout

A `.apextheme` file is a standard zip archive with this structure:

```
mypack.apextheme
├── manifest.json          # ThemeManifest — required
├── colorscheme.json       # ColorScheme in DTCG format — required
├── stylesystem.json       # StyleSystem in DTCG format — required
├── recipes.json           # RecipeSet — optional (empty object if absent)
├── shellprofile.json      # ShellProfile blob — optional, stub
└── assets/
    ├── font/mypack-mono   # Custom font bytes (if capabilities.uses_fonts)
    └── icon/mypack-icons  # Custom icon font (if capabilities.uses_icons)
```

All JSON files are UTF-8. The zip uses standard deflate compression.

---

## JSON Schemas

Machine-readable schemas for each section (JSON Schema draft 2020-12):

| Section          | Schema file |
|------------------|-------------|
| `manifest.json`  | `schema/manifest.schema.json` |
| `colorscheme.json` | `schema/colorscheme.schema.json` |
| `stylesystem.json` | `schema/stylesystem.schema.json` |
| `recipes.json`   | `schema/recipes.schema.json` |

---

## Token reference

Every token, its type, default, and what it affects:
[token-reference.md](token-reference.md)

---

## Recipe key registry

The canonical list of all recipe keys (component names):
[`docs/migration/recipe-keys.md`](../migration/recipe-keys.md)

---

## Asset workflow (S7)

Custom fonts and icons are bundled as binary assets in `assets/`.

### Fonts

1. Prepare a `.ttf` or `.otf` file for each weight/style variant.
2. Add an entry per variant in `manifest.asset_inventory`:
   ```json
   { "key": "font/mypack-ui", "kind": "FontFace", "mime": "font/ttf" }
   { "key": "font/mypack-mono", "kind": "FontFaceMono", "mime": "font/ttf" }
   ```
3. Set `capabilities.uses_fonts = true` in the manifest.
4. Reference the family name in `stylesystem.json`:
   ```json
   "typography": {
     "family_ui":   { "$type": "string", "$value": "MyPack UI" },
     "family_mono": { "$type": "string", "$value": "MyPack Mono" }
   }
   ```

The font loader resolves `family_ui` → the registered font family name at
startup. If no asset with `kind: FontFace` is registered for that family, the
loader falls back to the compiled-in default (`Inter`).

### Icons

1. Prepare a `.ttf` icon glyph font.
2. Add an inventory entry with `kind: "IconFont"`.
3. Set `capabilities.uses_icons = true`.

---

## Validation rules (S9)

The following rules are checked when a pack is loaded. Packs that fail validation
are rejected with a descriptive error:

| Rule | What is checked |
|------|-----------------|
| Schema version | `app_schema_version` ≤ `CURRENT_SCHEMA_VERSION` (currently 1) |
| Required fields | `manifest.id`, `manifest.name`, `manifest.app_schema_version` |
| Color hex | All palette values must be valid `#rrggbb` or `#rrggbbaa` hex strings |
| Contrast (planned) | Text-on-background WCAG contrast ratio ≥ 3.0 for muted text, ≥ 4.5 for primary |
| Numeric limits | Dimension values within sane ranges (e.g. font sizes 5–80px, radii 0–9999px) |

Checksum verification (SHA-256 over non-manifest zip bytes) is stored
informational in `manifest.checksum` but not yet verified by the reader —
this is deferred to S9 finalization.

---

## Adding a theme

To add a theme to apex-terminal:

1. Author the four JSON files described below (or use the in-app Theme Studio).
2. Bundle them into a `.apextheme` zip (standard zip, deflate compression).
3. Install via the Theme Settings panel or by placing the file in the
   themes directory and restarting.

The in-app Theme Studio (S10) generates all four files interactively, previews
the result in real time, and exports a ready-to-install bundle.

---

## Fully-worked example: "Nocturne" theme

A complete minimal dark theme with a purple accent, custom font references, and
two recipe overrides.

### `manifest.json`

```json
{
  "id": "nocturne",
  "name": "Nocturne",
  "author": "example-author",
  "version": "1.0.0",
  "app_schema_version": 1,
  "is_dark": true,
  "capabilities": {
    "uses_fonts": false,
    "uses_icons": false,
    "uses_imagery": false,
    "uses_shell_profile": false
  },
  "asset_inventory": []
}
```

Key points:
- `id` is the slug used for install/uninstall — must be globally unique.
- `app_schema_version: 1` is the only currently valid value.
- `capabilities.uses_fonts: false` means no custom font assets are bundled;
  the built-in Inter/JetBrains Mono are used.

---

### `colorscheme.json`

A custom dark palette with a violet accent and independently-set success/danger colors.

```json
{
  "meta": {
    "id": "nocturne",
    "name": "Nocturne",
    "is_dark": true
  },
  "palette": {
    "bg":      { "$type": "color", "$value": "#0d0d14" },
    "surface": { "$type": "color", "$value": "#16161f" },

    "text":    { "$type": "color", "$value": "#e2e2f0" },
    "dim":     { "$type": "color", "$value": "#6a6a88" },

    "border":  { "$type": "color", "$value": "#2a2a3c" },

    "accent":  { "$type": "color", "$value": "#7c6af5" },
    "bull":    { "$type": "color", "$value": "#34d399" },
    "bear":    { "$type": "color", "$value": "#f87171" },
    "warn":    { "$type": "color", "$value": "#fbbf24" },

    "success": { "$type": "color", "$value": "#10b981" },
    "danger":  { "$type": "color", "$value": "#ef4444" },
    "warning": { "$type": "color", "$value": "#f59e0b" },
    "info":    { "$type": "color", "$value": "#6366f1" },

    "shadow":  { "$type": "color", "$value": "#000000cc" },

    "notification_red": { "$type": "color", "$value": "#ef4444ff" },
    "gold":             { "$type": "color", "$value": "#f59e0bff" },
    "overlay_text":     { "$type": "color", "$value": "#e2e2f0ff" },

    "rrg_leading":   { "$type": "color", "$value": "#10b981ff" },
    "rrg_improving": { "$type": "color", "$value": "#7c6af5ff" },
    "rrg_weakening": { "$type": "color", "$value": "#f59e0bff" },
    "rrg_lagging":   { "$type": "color", "$value": "#ef4444ff" },

    "pinned_row_tint": { "$type": "color", "$value": "#00000010" },
    "text_muted":      { "$type": "color", "$value": "#9090aaff" },
    "hud_bg":          { "$type": "color", "$value": "#000000e6" },
    "hud_border":      { "$type": "color", "$value": "#2a2a3cff" }
  }
}
```

Annotations:
- `success` / `danger` / `warning` / `info` are set **independently** from
  `bull` / `bear` / `warn`. This means a "success" badge in the UI uses
  `#10b981` (a calm green), while a positive price change uses `bull` (`#34d399`,
  a vivid teal). On themes that leave these absent, both resolve to `bull`.
- `pane_gap_color` is omitted — the renderer derives the gap color from
  `bg`/`border` automatically.

---

### `stylesystem.json`

Aperture-inspired: rounded corners, comfortable spacing, enabled shadows.

```json
{
  "meta": {
    "id": "nocturne",
    "name": "Nocturne",
    "is_dark": true
  },
  "typography": {
    "size_xs": { "$type": "dimension", "$value": 9  },
    "size_sm": { "$type": "dimension", "$value": 11 },
    "size_md": { "$type": "dimension", "$value": 13 },
    "size_lg": { "$type": "dimension", "$value": 16 },
    "size_xl": { "$type": "dimension", "$value": 22 },
    "mono_sm": { "$type": "dimension", "$value": 11 },
    "mono_md": { "$type": "dimension", "$value": 13 },
    "mono_lg": { "$type": "dimension", "$value": 16 }
  },
  "radii": {
    "none": { "$type": "dimension", "$value": 0    },
    "xs":   { "$type": "dimension", "$value": 3    },
    "sm":   { "$type": "dimension", "$value": 6    },
    "md":   { "$type": "dimension", "$value": 8    },
    "lg":   { "$type": "dimension", "$value": 14   },
    "full": { "$type": "dimension", "$value": 9999 },
    "pill": { "$type": "dimension", "$value": 99   },
    "chip": { "$type": "dimension", "$value": 6    }
  },
  "treatments": {
    "solid_active_fills":       { "$type": "boolean", "$value": false },
    "hairline_borders":         { "$type": "boolean", "$value": false },
    "uppercase_section_labels": { "$type": "boolean", "$value": false },
    "focus_ring":               { "$type": "string",  "$value": "glow" },
    "shadows_enabled":          { "$type": "boolean", "$value": true  },
    "show_active_tab_underline":{ "$type": "boolean", "$value": true  }
  },
  "shadows": {
    "card": {
      "blur":     { "$type": "dimension", "$value": 10  },
      "spread":   { "$type": "dimension", "$value": 0   },
      "offset_x": { "$type": "dimension", "$value": 0   },
      "offset_y": { "$type": "dimension", "$value": 3   },
      "alpha":    { "$type": "dimension", "$value": 0.4 }
    }
  }
}
```

Notes:
- Only the sections you want to override need to be present. Absent sections
  (`spacing`, `strokes`, `alphas`, `elevation`, `density`) use
  `StyleSystem::builtin_default()` values.
- `focus_ring: "glow"` pairs well with the violet accent.
- The `shadows.card` override tightens the default card shadow slightly.

---

### `recipes.json`

Two component overrides: pill-shaped primary buttons and a subtle selected-row tint.

```json
{
  "button.primary": {
    "radius": "pill",
    "fill":   { "kind": "tone", "tone": "accent", "shade": "s500" },
    "px":     "lg",
    "py":     "sm",
    "text":   { "tone": "bg", "shade": "s50" },
    "hover": {
      "fill": { "kind": "tone", "tone": "accent", "shade": "s400" }
    },
    "active": {
      "fill": { "kind": "tone", "tone": "accent", "shade": "s600" }
    }
  },

  "row.list.selected": {
    "fill": { "kind": "alpha", "tone": "accent", "alpha": 28 }
  }
}
```

Annotations:
- `button.primary.radius = "pill"` — overrides the global `radii.sm` with a
  full pill. This is the escape hatch for per-component radius values.
- `button.primary.fill = { "kind": "tone", "tone": "accent", "shade": "s500" }` —
  uses the palette's accent color at its base shade. The `shade` field is optional
  (defaults to `s500`).
- `row.list.selected.fill = { "kind": "alpha", "tone": "accent", "alpha": 28 }` —
  accent color at alpha 28/255 (≈11% opacity) as the selected-row tint.
- Any key not listed leaves the widget's built-in default entirely intact.

---

## Common mistakes

**Wrong: using literal RGB in recipes**

```json
"fill": "#7c6af5"
```

Recipes only accept `ColorSpec` objects (`{ "kind": "tone", ... }` or
`{ "kind": "literal", "hex": "#..." }`). A bare string is not valid.

**Wrong: setting dimension values as plain numbers in DTCG**

```json
"size_sm": 11
```

All DTCG tokens must be wrapped: `{ "$type": "dimension", "$value": 11 }`.

**Wrong: referencing an unloaded font family**

```json
"family_ui": { "$type": "string", "$value": "My Custom Font" }
```

If no asset with `kind: FontFace` is registered for `"My Custom Font"`, the
loader silently falls back to the built-in Inter. Either bundle the font file
in `assets/` and set `capabilities.uses_fonts: true`, or use a family that is
already loaded.

**Wrong: `app_schema_version` > 1**

The only currently valid value is `1`. Packs with a higher version are rejected.

---

## Design system source locations

| Question | File |
|----------|------|
| ColorScheme fields | `src-tauri/src/design_system/color_scheme.rs` |
| StyleSystem fields | `src-tauri/src/design_system/style_system.rs` |
| DTCG parse logic   | `src-tauri/src/design_system/loader.rs` |
| ThemePack bundle   | `src-tauri/src/design_system/theme_pack/` |
| RecipeSpec serde   | `src-tauri/src/ui_kit/sx/recipe_spec.rs` |
| Recipe key registry | `docs/migration/recipe-keys.md` |
| Token reference    | `docs/theme-authoring/token-reference.md` |
| JSON Schemas       | `docs/theme-authoring/schema/` |
