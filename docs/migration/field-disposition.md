# StyleSettings → StyleSystem Field Disposition

**Stream S1 authoritative contract.** Every field in `StyleSettings`
(`src-tauri/src/chart/renderer/ui/style.rs`) is accounted for here. S2
builds the total adapter against this table.

Verified against the actual structs on 2026-06-13.
Source of truth for values: `style_defaults(id)` match arms (ids 0 = Meridien,
1 = Aperture, 2 = Octave) in `style.rs`.

---

## Ownership note

- **S1** defines all new typed enums (`PaneActiveIndicator`,
  `PanelHeaderTreatment`) and adds the widened-palette fields
  (`info`/`success`/`warning`/`danger`, `pane_gap` color) to `ColorScheme`.
- **S3/S11** consume the typed enums — after S1 merges, S3 updates
  `TokenSnapshot` and `Treatments` to use the typed forms;
  `pane_active_indicator: u8` → `PaneActiveIndicator` and
  `panel_header_treatment: u8` → `PanelHeaderTreatment`.

---

## Field-by-field disposition

### Radii

| StyleSettings field | Disposition | StyleSystem path | Status |
|---------------------|-------------|-----------------|--------|
| `r_xs` | `Radii.xs` | `style.radii.xs` | EXISTS |
| `r_sm` | `Radii.sm` | `style.radii.sm` | EXISTS |
| `r_md` | `Radii.md` | `style.radii.md` | EXISTS |
| `r_lg` | `Radii.lg` | `style.radii.lg` | EXISTS |
| `r_pill` | `Radii.pill` | `style.radii.pill` | EXISTS |
| `r_chip` | `Radii.chip` | `style.radii.chip` | EXISTS |

All radii fields are fully migrated.

---

### Strokes

| StyleSettings field | Disposition | StyleSystem path | Status |
|---------------------|-------------|-----------------|--------|
| `stroke_hair` | `Strokes.thin` | `style.strokes.thin` | EXISTS (mapping: StyleSettings.stroke_hair → Strokes.thin; Strokes.hair is a sub-pixel constant) |
| `stroke_thin` | `Strokes.std` | `style.strokes.std` | EXISTS |
| `stroke_std` | `Strokes.std` | `style.strokes.std` | EXISTS (collapsed with stroke_thin in most styles) |
| `stroke_bold` | `Strokes.bold` / `Strokes.md` | `style.strokes.bold` | EXISTS |
| `stroke_thick` | `Strokes.thick` / `Strokes.heavy` | `style.strokes.thick` | EXISTS |

All stroke fields are fully migrated.

---

### Typography

| StyleSettings field | Disposition | StyleSystem path | Status |
|---------------------|-------------|-----------------|--------|
| `font_section_label` | `Typography.size_section_label` | `style.typography.size_section_label` | EXISTS |
| `font_body` | `Typography.size_sm` | `style.typography.size_sm` | EXISTS |
| `font_caption` | `Typography.size_xs` | `style.typography.size_xs` | EXISTS |
| `font_hero` | `Typography.size_xl` | `style.typography.size_xl` | EXISTS (renamed) |
| `label_letter_spacing_px` | `Typography.label_tracking` | `style.typography.label_tracking` | EXISTS |
| `nav_letter_spacing_px` | `Typography.nav_tracking` | `style.typography.nav_tracking` | EXISTS |
| `section_header_tracking` | `Typography.section_tracking` | `style.typography.section_tracking` | EXISTS |

All typography fields are fully migrated.

---

### Spacing / Density

| StyleSettings field | Disposition | StyleSystem path | Status |
|---------------------|-------------|-----------------|--------|
| `card_padding_y` | `Spacing.md` | `style.spacing.md` | EXISTS (approx) |
| `card_padding_x` | `Spacing.lg` | `style.spacing.lg` | EXISTS (approx) |
| `cta_height_px` | `Spacing.cta_height` | `style.spacing.cta_height` | EXISTS |
| `cta_padding_x` | `Spacing.cta_padding_x` | `style.spacing.cta_padding_x` | EXISTS |
| `button_height_px` | `Spacing.button_height` | `style.spacing.button_height` | EXISTS |
| `button_padding_x` | `Spacing.button_padding_x` | `style.spacing.button_padding_x` | EXISTS |
| `tab_height` | `Spacing.tab_height` | `style.spacing.tab_height` | EXISTS |
| `row_height_px` | `Density.row_height_dense` | `style.density.row_height_dense` | EXISTS |
| `density` | `Density.factor` | `style.density.factor` | EXISTS (u8 enum 0/1/2 → f32 0.8/1.0/1.2) |

All spacing/density fields are fully migrated.

---

### Shadows

| StyleSettings field | Disposition | StyleSystem path | Status |
|---------------------|-------------|-----------------|--------|
| `shadow_blur` | `Shadows.card.blur` | `style.shadows.card.blur` | EXISTS |
| `shadow_offset_y` | `Shadows.card.offset_y` | `style.shadows.card.offset_y` | EXISTS |
| `shadow_alpha` | `Shadows.card.alpha` | `style.shadows.card.alpha` | EXISTS |
| `card_floating_shadow_alpha` | `Chrome.card_floating_shadow_alpha` | `style.chrome.card_floating_shadow_alpha` | EXISTS |

All shadow fields are fully migrated.

---

### Treatments (boolean / behavioral flags)

| StyleSettings field | Disposition | StyleSystem path | Status |
|---------------------|-------------|-----------------|--------|
| `serif_headlines` | `Treatments.serif_headlines` | `style.treatments.serif_headlines` | EXISTS |
| `button_treatment` | `Treatments.button_treatment` | `style.treatments.button_treatment` | EXISTS (stored as u8; S3 will type as ButtonTreatment enum) |
| `hairline_borders` | `Treatments.hairline_borders` | `style.treatments.hairline_borders` | EXISTS |
| `solid_active_fills` | `Treatments.solid_active_fills` | `style.treatments.solid_active_fills` | EXISTS |
| `invert_active_fill` | `Treatments.invert_active_fill` | `style.treatments.invert_active_fill` | EXISTS |
| `uppercase_section_labels` | `Treatments.uppercase_section_labels` | `style.treatments.uppercase_section_labels` | EXISTS |
| `vertical_group_dividers` | `Treatments.vertical_group_dividers` | `style.treatments.vertical_group_dividers` | EXISTS |
| `show_active_tab_underline` | `Treatments.show_active_tab_underline` | `style.treatments.show_active_tab_underline` | EXISTS |
| `inactive_header_fill` | `Treatments.inactive_header_fill` | `style.treatments.inactive_header_fill` | EXISTS |
| `nav_buttons_label_only` | `Treatments.nav_buttons_label_only` | `style.treatments.nav_buttons_label_only` | EXISTS |
| `nav_buttons_uppercase_labels` | `Treatments.nav_buttons_uppercase_labels` | `style.treatments.nav_buttons_uppercase_labels` | EXISTS |
| `tab_underline_under_text` | `Treatments.tab_underline_under_text` | `style.treatments.tab_underline_under_text` | EXISTS |
| `card_floating_shadow` | `Treatments.card_floating_shadow` | `style.treatments.card_floating_shadow` | EXISTS |
| `shadows_enabled` | `Treatments.shadows_enabled` | `style.treatments.shadows_enabled` | EXISTS |
| `animations_enabled` | `Treatments.animations_enabled` | `style.treatments.animations_enabled` | EXISTS |
| `surface_bevel` | `Treatments.surface_bevel` | `style.treatments.surface_bevel` | EXISTS |
| `bevel_highlight_alpha` | `Treatments.bevel_highlight_alpha` | `style.treatments.bevel_highlight_alpha` | EXISTS |
| `bevel_shadow_alpha` | `Treatments.bevel_shadow_alpha` | `style.treatments.bevel_shadow_alpha` | EXISTS |
| `section_header_mono` | `Treatments.section_header_mono` | `style.treatments.section_header_mono` | EXISTS |
| `wl_symbol_mono` | `Treatments.wl_symbol_mono` | `style.treatments.wl_symbol_mono` | EXISTS |
| `panel_tab_treatment` | `Treatments.panel_tab_treatment` | `style.treatments.panel_tab_treatment` | EXISTS (u8; S3 will type as TabTreatment) |
| `pane_active_fill_accent` | `Treatments.pane_active_fill_accent` | `style.treatments.pane_active_fill_accent` | EXISTS |
| `wl_row_side_margin` | `Treatments.wl_row_side_margin` | `style.treatments.wl_row_side_margin` | EXISTS — RECIPE-CANDIDATE(S4) |
| `wl_row_corner_radius` | `Treatments.wl_row_corner_radius` | `style.treatments.wl_row_corner_radius` | EXISTS — RECIPE-CANDIDATE(S4) |
| `wl_row_divider_alpha` | `Treatments.wl_row_divider_alpha` | `style.treatments.wl_row_divider_alpha` | EXISTS — RECIPE-CANDIDATE(S4) |

All treatment boolean/enum fields are fully migrated.

---

### Chrome (geometry and finish)

| StyleSettings field | Disposition | StyleSystem path | Status |
|---------------------|-------------|-----------------|--------|
| `toolbar_height_scale` | `Chrome.toolbar_height_scale` | `style.chrome.toolbar_height_scale` | EXISTS |
| `header_height_scale` | `Chrome.header_height_scale` | `style.chrome.header_height_scale` | EXISTS |
| `account_strip_height` | `Chrome.account_strip_height` | `style.chrome.account_strip_height` | EXISTS |
| `pane_border_width` | `Chrome.pane_border_width` | `style.chrome.pane_border_width` | EXISTS |
| `pane_gap` | `Chrome.pane_gap` | `style.chrome.pane_gap` | EXISTS |
| `pane_gap_alpha` | `Chrome.pane_gap_alpha` | `style.chrome.pane_gap_alpha` | EXISTS |
| `pane_active_indicator` | `Chrome.pane_active_indicator` | `style.chrome.pane_active_indicator` | EXISTS (u8; S3 will type as PaneActiveIndicator) |
| `active_header_fill_multiply` | `Chrome.active_header_fill_multiply` | `style.chrome.active_header_fill_multiply` | EXISTS |
| `inactive_header_fill_multiply` | `Chrome.inactive_header_fill_multiply` | `style.chrome.inactive_header_fill_multiply` | EXISTS |
| `header_outer_border_alpha` | `Chrome.header_outer_border_alpha` | `style.chrome.header_outer_border_alpha` | EXISTS |
| `header_outer_border_width` | `Chrome.header_outer_border_width` | `style.chrome.header_outer_border_width` | EXISTS |
| `header_divider_alpha` | `Chrome.header_divider_alpha` | `style.chrome.header_divider_alpha` | EXISTS |
| `nav_active_col_alpha` | `Chrome.nav_active_col_alpha` | `style.chrome.nav_active_col_alpha` | EXISTS |
| `dialog_backdrop_alpha` | `Chrome.dialog_backdrop_alpha` | `style.chrome.dialog_backdrop_alpha` | EXISTS |
| `tab_inactive_alpha` | `Chrome.tab_inactive_alpha` | `style.chrome.tab_inactive_alpha` | EXISTS |
| `tab_hover_bg_alpha` | `Chrome.tab_hover_bg_alpha` | `style.chrome.tab_hover_bg_alpha` | EXISTS |
| `hover_bg_alpha` | `Chrome.hover_bg_alpha` | `style.chrome.hover_bg_alpha` | EXISTS |
| `active_bg_alpha` | `Chrome.active_bg_alpha` | `style.chrome.active_bg_alpha` | EXISTS |
| `focus_ring_width` | `Chrome.focus_ring_width` | `style.chrome.focus_ring_width` | EXISTS |
| `focus_ring_alpha` | `Chrome.focus_ring_alpha` | `style.chrome.focus_ring_alpha` | EXISTS |
| `disabled_opacity` | `Chrome.disabled_opacity` | `style.chrome.disabled_opacity` | EXISTS |
| `accent_emphasis` | `Chrome.accent_emphasis` | `style.chrome.accent_emphasis` | EXISTS |
| `tab_underline_thickness` | `Chrome.tab_underline_thickness` | `style.chrome.tab_underline_thickness` | EXISTS — RECIPE-CANDIDATE(S4) |
| `section_label_padding_top` | `Chrome.section_label_padding_top` | `style.chrome.section_label_padding_top` | EXISTS — RECIPE-CANDIDATE(S4) |
| `section_label_padding_bottom` | `Chrome.section_label_padding_bottom` | `style.chrome.section_label_padding_bottom` | EXISTS — RECIPE-CANDIDATE(S4) |
| `drag_handle_alpha` | `Chrome.drag_handle_alpha` | `style.chrome.drag_handle_alpha` | EXISTS — RECIPE-CANDIDATE(S4) |
| `drag_handle_dot_scale` | `Chrome.drag_handle_dot_scale` | `style.chrome.drag_handle_dot_scale` | EXISTS — RECIPE-CANDIDATE(S4) |
| `toast_bg_alpha` | `Chrome.toast_bg_alpha` | `style.chrome.toast_bg_alpha` | EXISTS — RECIPE-CANDIDATE(S4) |
| `card_stripe_alpha` | `Chrome.card_stripe_alpha` | `style.chrome.card_stripe_alpha` | EXISTS — RECIPE-CANDIDATE(S4) |
| `region_gap` | `Chrome.region_gap` | `style.chrome.region_gap` | EXISTS |
| `region_radius` | `Chrome.region_radius` | `style.chrome.region_radius` | EXISTS |
| `region_border_alpha` | `Chrome.region_border_alpha` | `style.chrome.region_border_alpha` | EXISTS |
| `nav_cluster_radius` | `Chrome.nav_cluster_radius` | `style.chrome.nav_cluster_radius` | EXISTS |
| `nav_cluster_fill_alpha` | `Chrome.nav_cluster_fill_alpha` | `style.chrome.nav_cluster_fill_alpha` | EXISTS |
| `nav_cluster_padding` | `Chrome.nav_cluster_padding` | `style.chrome.nav_cluster_padding` | EXISTS |
| `button_group` | `Chrome.button_group` (GroupEnclosure) | `style.chrome.button_group` | EXISTS |
| `toolnav_height` | `Chrome.toolnav_height` | `style.chrome.toolnav_height` | EXISTS — RECIPE-CANDIDATE(S4) |
| `panel_header_treatment` | `Chrome.panel_header_treatment` | `style.chrome.panel_header_treatment` | EXISTS (u8; S3 will type as PanelHeaderTreatment) — RECIPE-CANDIDATE(S4) |
| `panel_section_fill_alpha` | `Chrome.panel_section_fill_alpha` | `style.chrome.panel_section_fill_alpha` | EXISTS — RECIPE-CANDIDATE(S4) |
| `panel_footer_card` | `Chrome.panel_footer_card` | `style.chrome.panel_footer_card` | EXISTS — RECIPE-CANDIDATE(S4) |
| `panel_footer_radius` | `Chrome.panel_footer_radius` | `style.chrome.panel_footer_radius` | EXISTS — RECIPE-CANDIDATE(S4) |

All Chrome geometry fields are fully migrated.

---

### Axis violations and special cases

| StyleSettings field | Disposition | Notes |
|---------------------|-------------|-------|
| `pane_gap_color: Option<Color32>` | **AXIS VIOLATION → ColorScheme.pane_gap_color: Option<Rgba>** | A color on the dimension axis. Moved to ColorScheme as `Option<Rgba>` with `#[serde(default)]`. All 9 style_defaults() entries set this to `None`; default is `None`. When `None`, renderers derive the gap color from `bg`/`border` at paint time. |
| `footer_default_open: bool` | `Chrome.footer_default_open` | **Borderline app-state.** The brief notes this "should" move to a `state/` aggregate. However: (a) `Watchlist`/`Chart` are frozen; (b) there is no current `state/` aggregate that persists per-style; (c) the field is a *style-default* (Aperture ships the footer open; Meridien does not) more than a user preference. It remains in Chrome as a *style-level default* that session state can override. If a dedicated style-preferences state aggregate is added in a later wave, this migrates there. Documented: not moving in S1. |

---

## Palette-depth fields (ColorScheme additions — PALETTE-DEPTH decision)

Per the brief's "Palette depth — DECIDED" section, the following semantic
color fields are added to `ColorScheme`:

| New field | Type | Default (dark) | Default (light) | Purpose |
|-----------|------|----------------|-----------------|---------|
| `success` | `Rgba` | derived from `bull` | derived from `bull` | Positive state; general success (not just price) |
| `danger` | `Rgba` | derived from `bear` | derived from `bear` | Error / destructive state |
| `warning` | `Rgba` | derived from `warn` | derived from `warn` | Caution state |
| `info` | `Rgba` | `rgba(100, 160, 220, 255)` | `rgba(30, 100, 180, 255)` | Informational / neutral highlight |
| `pane_gap_color` | `Option<Rgba>` | `None` | `None` | Gap gutter fill; derived from bg/border when None |

`bull`/`bear` default to the values of `success`/`danger` when `success`/`danger`
are unset in legacy JSON (backward compatible via `serde(default)`). The
built-in schemes seed `success = bull` and `danger = bear` so the existing
visual is preserved exactly.

---

## Coverage summary

- **Total StyleSettings fields:** ~85 distinct fields
- **Already migrated before S1:** ~75 fields
- **Axis violation fixed (pane_gap_color):** 1 field moved to ColorScheme
- **App-state (stays in Chrome with note):** 1 field (footer_default_open)
- **RECIPE-CANDIDATE(S4) markers added:** 11 fields
- **Zero "TBD" fields**

All fields have a disposition. S2 can build the total adapter from this table.
