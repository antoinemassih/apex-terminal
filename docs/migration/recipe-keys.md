# Recipe Key Registry

**Append-only.** Keys are never renamed or removed — doing so would silently
drop any theme author's overrides for that key. To deprecate a key, add a note
and leave it in the registry.

Key convention: `<component>[.<variant>][.<state>]`

- Components are lowercase snake-case.
- Variants and states use the same casing.
- Separator is `.` (dot).

---

## Initial registry (S4)

These keys correspond to the RECIPE-CANDIDATE(S4) fields in
`docs/migration/field-disposition.md`.

### Buttons

| Key | Component | Notes |
|-----|-----------|-------|
| `button.primary` | `ui_kit::Button` Variant::Primary | Accent fill, text, radius, padding |
| `button.ghost` | `ui_kit::Button` Variant::Ghost | Border-only, text, hover ring |
| `button.danger` | `ui_kit::Button` Variant::Danger | Bear fill, destructive actions |
| `button.success` | `ui_kit::Button` implied success | Bull fill, confirm actions |
| `button.chrome` | `ui_kit::Button` Variant::Chrome | Toolbar/chrome chrome buttons |
| `button.action` | `ui_kit::Button` via `.recipe_key("button.action")` | Large block controls in a trading action row (DOM BUY/SELL/FLATTEN/CANCEL). Same treatment as `button.primary`, but each style picks a radius that survives a near-SQUARE control — `Pill` resolves to min(w,h)/2 and becomes an ellipse there. |
| `input` | `ui_kit::Input` | Text-field chrome. **Radius only** — fill/border stay with the widget, which computes them from focus / invalid / disabled state. |
| `select` | `ui_kit::Select` | Dropdown trigger chrome. Radius only, same rationale. |
| `checkbox` | `ui_kit::Checkbox` | Check box chrome. Radius only, same rationale. |
| `popover` | `ui_kit::ContextMenu`, tool popovers | Floating surface chrome. Radius only. Shared so every floating surface restyles together. |
| `segmented` | `ui_kit::SegmentedControl` | The trough. Segment fills encode selection and stay with the widget. |
| `switch` | `ui_kit::Switch` | The track. Pill in every built-in style — a track is a capsule by definition. |
| `alert` | `ui_kit::Alert` | Banner surface. Radius only — the tone encodes Info/Success/Warning/Error. |
| `tooltip` | `ui_kit::Tooltip` | Tooltip card surface. Radius only; the near-solid alpha is deliberate. |
| `badge` | `ui_kit::Badge` | Count/notification pill. Pill in every built-in style. |
| `progress` | `ui_kit::Progress` | The track. Pill in every built-in style; variant fill stays with the widget. |

### Tabs

| Key | Component | Notes |
|-----|-----------|-------|
| `tab.line` | Tab bar inactive state | Base tab geometry |
| `tab.line.active` | Active tab | Underline thickness (`tab_underline_thickness` S4 candidate) |
| `tab.pill` | Pill-style tabs (inactive) | Radius, padding |
| `tab.pill.active` | Pill-style active tab | Fill override |

### Rows / Lists

| Key | Component | Notes |
|-----|-----------|-------|
| `row.list` | `PanelListRow` default row | Radius, padding, divider alpha (`wl_row_corner_radius`, `wl_row_side_margin`, `wl_row_divider_alpha`) |
| `row.list.selected` | `PanelListRow` selected state | Tinted fill, typically `Accent alpha` |
| `row.list.hover` | `PanelListRow` hovered state | Hover bg alpha |

### Sections

| Key | Component | Notes |
|-----|-----------|-------|
| `section.header` | `PanelSection` header strip | Padding top/bottom (`section_label_padding_top/bottom`), case, font |
| `section.header.fill` | `PanelSection` with fill | Fill alpha (`panel_section_fill_alpha`) |

### Navigation

| Key | Component | Notes |
|-----|-----------|-------|
| `nav.cluster` | Nav cluster pill/group | Radius, fill alpha, padding (`nav_cluster_radius`, `nav_cluster_fill_alpha`, `nav_cluster_padding`) |
| `nav.cluster.active` | Active nav item in cluster | Accent or solid fill |

### Panels

| Key | Component | Notes |
|-----|-----------|-------|
| `panel.footer` | `PanelFooter` | Card-or-flush treatment (`panel_footer_card`, `panel_footer_radius`) |
| `panel.header` | `SidePanelShell` header | Treatment flags (`panel_header_treatment`) |

### Cards

| Key | Component | Notes |
|-----|-----------|-------|
| `card` | Generic `ui_kit::Card` | Radius, shadow, stripe alpha (`card_stripe_alpha`) |
| `card.floating` | Elevated/floating card | Shadow alpha (`card_floating_shadow_alpha`) |

### Toasts / Notifications

| Key | Component | Notes |
|-----|-----------|-------|
| `toast` | `ui_kit::Toast` | BG alpha (`toast_bg_alpha`), radius, border |
| `toast.success` | Success toast | Bull-tinted fill |
| `toast.danger` | Error toast | Bear-tinted fill |
| `toast.warn` | Warning toast | Warn-tinted fill |

### Tags / Chips

| Key | Component | Notes |
|-----|-----------|-------|
| `tag` | `ui_kit::Tag` | Pill radius, soft fill alpha, border style |

### Keyboard / Badges

| Key | Component | Notes |
|-----|-----------|-------|
| `kbd` | `ui_kit::Kbd` / keyboard shortcut display | Surface fill, border, radius, mono font |

### Drag handles

| Key | Component | Notes |
|-----|-----------|-------|
| `drag.handle` | `PanelSectionGroup` drag divider | Alpha, dot scale (`drag_handle_alpha`, `drag_handle_dot_scale`) |

### Toolnav

| Key | Component | Notes |
|-----|-----------|-------|
| `toolnav` | Bottom tool navigation bar | Height (`toolnav_height`), padding |

---

## Key discovery (how to decide if a field needs a key)

A field is a RECIPE-CANDIDATE when ALL of these are true:

1. It controls how a **specific named component** looks (not global spacing/typography).
2. Different themes want meaningfully different values (not just "follow the scale").
3. It cannot be derived from an existing semantic token (radius_sm, gap_md, etc.).
4. A theme author would reasonably want to override it independently.

Fields that just "follow the scale" (e.g. `button_height_px` → `Spacing.button_height`)
do NOT need a recipe key — they belong on `StyleSystem` and are handled by the
token tier system.

---

## Key lifecycle

Keys are **append-only**. To deprecate:

```markdown
| `old.key` | ... | ⚠️ DEPRECATED as of S12. Use `new.key` instead. Still parsed for backward compat. |
```

Consumers that read recipe files must silently ignore unknown keys — this ensures
forward compatibility when new keys are added in future streams.

---

## Figma mapping note

Recipe keys map 1-to-1 with Figma component names / variant tokens when Figma
uses the `{component}/{variant}/{state}` convention. The dot-separated key format
is equivalent to `/`-separated Figma paths with casing normalized to snake_case:

| Recipe key | Figma component path |
|------------|---------------------|
| `button.primary` | `Button/Primary` |
| `button.ghost` | `Button/Ghost` |
| `tab.line.active` | `Tab/Line/Active` |
| `row.list.selected` | `Row/List/Selected` |
| `nav.cluster.active` | `Nav/Cluster/Active` |

When exporting Figma tokens via Style Dictionary or Token Studio, the path
separator can be replaced with `.` and casing lowercased to produce valid
recipe keys directly.
