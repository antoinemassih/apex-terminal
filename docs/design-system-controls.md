# Apex Terminal Design Controls — what each parameter does

**Refreshed:** 2026-05-02 (post-R3)  
**Field count:** 77 fields in `StyleSettings` (struct at `style.rs:912`)  
**Styles:** Meridien (0), Aperture (1), Octave (2) + 7 aliases (Cadence, Chord, Lattice, Tangent, Tempo, Contour, Relay)

Open the Design Inspector (F12) → **Design** tab → **Style Editor** to live-edit all parameters below. Changes take effect on the next frame with no rebuild.

---

## Corner Radii (6 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `r_xs` | u8 | 0 | 4 | 1 | Tiny chip/tag corners | Indicators, tiny labels |
| `r_sm` | u8 | 0 | 6 | 2 | Button corners, small card corners | All action buttons |
| `r_md` | u8 | 0 | 8 | 3 | Card/dialog/chrome tile corners | Cards, dialogs, pane chrome btns |
| `r_lg` | u8 | 0 | 12 | 4 | Modal/overlay/large card corners | Popups, large modals |
| `r_pill` | u8 | 0 | 99 | 99 | Pill buttons and segmented controls | SegmentedControl |
| `r_chip` | u8 | 0 | 0 | 0 | Badge/chip corner radius (0 = use r_sm) | status_badge() calls |

## Borders & Strokes (7 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `hairline_borders` | bool | true | false | true | Applies hairline (sub-pixel) borders to frames, panes, separators | All bordered elements |
| `stroke_hair` | f32 | 0.5 | 0.5 | 0.4 | Thinnest border weight (used when hairline_borders) | Separators, dividers |
| `stroke_thin` | f32 | 1.0 | 1.0 | 0.6 | Subtle borders, inactive outlines | Card edges, panel frames |
| `stroke_std` | f32 | 1.0 | 1.5 | 1.0 | Standard borders | Dialog windows, pane borders |
| `stroke_bold` | f32 | 1.0 | 1.5 | 1.0 | Active/hover borders, tab underlines | Hover states, active underlines |
| `stroke_thick` | f32 | 1.0 | 2.0 | 1.4 | Emphasis borders, active drag handles | Chart annotations, split dividers |
| `pane_border_width` | f32 | 0.5 | 1.0 | 0.6 | Stroke width of the border drawn around each pane tile | Any multi-pane layout |

## Layout (10 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `pane_gap` | f32 | 0.0 | 8.0 | 2.0 | Pixel gap between adjacent pane tiles (the gutter width) | Multi-pane layouts |
| `pane_gap_alpha` | u8 | 0 | 30 | 15 | Alpha of the gutter fill between panes — 0 = transparent, 255 = full `pane_gap_color` | Multi-pane layouts |
| `pane_gap_color` | Color? | None | None | None | Override color for pane gutter (None = `toolbar_border` at `pane_gap_alpha`) | Multi-pane layouts |
| `pane_active_indicator` | u8 | 1 | 2 | 3 | 0=none, 1=accent top-border line on active pane, 2=brightened header fill, 3=both | Multi-pane layouts |
| `account_strip_height` | f32 | 36.0 | 26.0 | 26.0 | Height of the account summary strip at the top | Account strip visible |
| `row_height_px` | f32 | 22.0 | 26.0 | 20.0 | Base height for table/list rows (scales with `density`) | Orders, Scanner, DOM |
| `density` | u8 | 1 | 2 | 0 | 0=compact, 1=normal, 2=spacious — scales row/tab/button heights | Entire app |
| `toolbar_height_scale` | f32 | 1.40 | 1.0 | 1.0 | Multiplier on the baseline toolbar height | Top nav bar |
| `header_height_scale` | f32 | 1.10 | 1.0 | 1.0 | Multiplier on pane header heights | Pane chrome headers |
| `card_padding_y` | f32 | 8.0 | 12.0 | 6.0 | Vertical inner padding inside cards | Order/alert/play cards |
| `card_padding_x` | f32 | 10.0 | 14.0 | 8.0 | Horizontal inner padding inside cards | Order/alert/play cards |

## Typography (10 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `font_hero` | f32 | 36.0 | 22.0 | 22.0 | Font size for large price/P&L numerics | Chart price display, portfolio totals |
| `font_section_label` | f32 | 8.0 | 10.0 | 8.0 | Size of section eyebrow labels (ORDERS, POSITIONS…) | Panel section headers |
| `font_body` | f32 | 10.0 | 11.0 | 10.0 | Body and table text size | All data tables, lists |
| `font_caption` | f32 | 8.0 | 9.0 | 8.0 | Caption, badge, secondary text size | Badges, subtitles, timestamps |
| `serif_headlines` | bool | true | false | false | Toggles serif font for hero numerics | Price, P&L large displays |
| `uppercase_section_labels` | bool | true | false | true | Uppercases all section eyebrow labels | Panel section headers |
| `label_letter_spacing_px` | f32 | 0.0 | 0.0 | 0.0 | Letter-spacing in section labels (via Unicode thin-spaces; < 0.5 = off) | Section labels when uppercase |
| `nav_letter_spacing_px` | f32 | 0.0 | 0.0 | 0.0 | Letter-spacing in toolbar nav buttons (same approximation) | Toolbar nav buttons |
| `section_label_padding_top` | f32 | 4.0 | 6.0 | 3.0 | Space above section eyebrow labels in px | All section_label() calls in panels |
| `section_label_padding_bottom` | f32 | 2.0 | 2.0 | 1.0 | Space below section eyebrow labels before content | All section_label() calls in panels |

## Color (8 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `active_fill_color` | Color? | Black | None | None | Fill color for active buttons/segments (None = theme accent) | Toolbar active buttons, CTA |
| `active_text_color` | Color? | White | None | None | Text color on active buttons (None = contrast-auto) | Same as above |
| `idle_outline_color` | Color? | Near-black | None | None | Border color for idle pill segments (None = toolbar_border) | SegmentedControl in select widget |
| `segmented_idle_fill` | Color? | None | None | None | Background fill for idle segments in SegmentedControl | SegmentedControl (new widgets) |
| `segmented_idle_text` | Color? | None | None | None | Text color for idle segments (None = dim) | SegmentedControl (new widgets) |
| `input_focus_color` | Color? | None | None | None | Focus ring color on text inputs (None = accent) | Any focused TextEdit |
| `pane_gap_color` | Color? | None | None | None | Custom pane gutter color (None = toolbar_border) | Multi-pane gutter |
| `accent_emphasis` | f32 | 1.0 | 1.1 | 0.95 | Brightness multiplier on accent for active elements | Active buttons, tab underlines |

## Buttons (9 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `button_treatment` | enum | UnderlineActive | SoftPill | RaisedActive | Visual treatment for toolbar/nav button active state | Top toolbar navigation |
| `solid_active_fills` | bool | true | false | true | Solid vs. tinted fills on active/selected items | Pills, segmented controls |
| `button_height_px` | f32 | 24.0 | 28.0 | 22.0 | Standard action button height (scales with density) | Order entry buttons |
| `button_padding_x` | f32 | 10.0 | 14.0 | 8.0 | Horizontal padding inside action buttons | All action/simple buttons |
| `cta_height_px` | f32 | 36.0 | 40.0 | 32.0 | Primary CTA button height (REVIEW BUY, PLACE ORDER) | Order ticket bottom |
| `cta_padding_x` | f32 | 16.0 | 12.0 | 12.0 | Horizontal padding in CTA button | Order ticket CTA |
| `nav_active_col_alpha` | u8 | 18 | 0 | 25 | Column tint alpha behind active toolbar nav button | Toolbar (Meridien/Octave style) |
| `hover_bg_alpha` | u8 | 20 | 15 | 18 | Background tint alpha on hover | All interactive widgets |
| `active_bg_alpha` | u8 | 35 | 25 | 30 | Background alpha when pressed/active | Interactive widget presses |

## Inputs (4 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `focus_ring_width` | f32 | 1.0 | 2.0 | 1.5 | Width of keyboard focus ring on text inputs | Text fields when focused |
| `focus_ring_alpha` | u8 | 120 | 90 | 110 | Opacity of focus ring | Focused text inputs |
| `disabled_opacity` | f32 | 0.4 | 0.5 | 0.45 | Opacity multiplier for disabled widgets | Disabled buttons/inputs |
| `input_focus_color` | Color? | None | None | None | Focus ring color override (None = accent) | Focused text inputs |

## Cards (5 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `card_stripe_alpha` | u8 | 255 | 255 | 255 | Opacity of left-edge accent stripe on order/alert cards | Orders, Alerts panels |
| `card_floating_shadow` | bool | true | false | false | Floating drop shadow on card windows | Card popups, dialogs |
| `card_floating_shadow_alpha` | u8 | 25 | 0 | 0 | Opacity of floating card shadow | Card popups |
| `card_padding_y` | f32 | 8.0 | 12.0 | 6.0 | Card vertical inner padding | Order/alert/play cards |
| `card_padding_x` | f32 | 10.0 | 14.0 | 8.0 | Card horizontal inner padding | Order/alert/play cards |

## Tabs (6 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `tab_height` | f32 | 28.0 | 32.0 | 26.0 | Tab bar item height (scales with density) | New-widgets tabs |
| `show_active_tab_underline` | bool | true | true | true | Underline accent line on active tab | All tab bars |
| `tab_underline_thickness` | f32 | 2.0 | 0.0 | 1.0 | Thickness of active tab underline | Tab bars, pane headers |
| `tab_underline_under_text` | bool | true | false | false | Position underline under text vs at header bottom | Pane header tab underline |
| `tab_inactive_alpha` | f32 | 0.6 | 0.55 | 0.5 | Opacity multiplier for inactive tab text | Multi-tab pane headers |
| `tab_hover_bg_alpha` | u8 | 12 | 18 | 20 | Background tint alpha on hovered inactive tab | Multi-tab pane headers (hover) |

## Navigation (4 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `vertical_group_dividers` | bool | true | false | false | Full-height vertical dividers between toolbar clusters | Toolbar (Meridien only) |
| `active_header_fill_multiply` | f32 | 0.95 | 1.2 | 1.2 | Brightness multiplier for active pane header fill | Multi-pane header chrome |
| `inactive_header_fill` | bool | false | true | true | Draw a fill on inactive pane headers | Multi-pane headers |
| `pane_active_indicator` | u8 | 1 | 2 | 3 | 0=none 1=top-accent-line 2=header-fill 3=both | Multi-pane active pane |

## Interaction (5 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `drag_handle_alpha` | f32 | 0.5 | 0.7 | 0.6 | Opacity of drag handles at rest | Split dividers between panels |
| `drag_handle_dot_scale` | f32 | 1.0 | 1.0 | 0.85 | Size multiplier for drag handle center dots | Split dividers (on hover) |
| `disabled_opacity` | f32 | 0.4 | 0.5 | 0.45 | Opacity multiplier for disabled widgets | Disabled buttons/inputs |
| `hover_bg_alpha` | u8 | 20 | 15 | 18 | Background tint alpha on hover | Interactive widgets |
| `active_bg_alpha` | u8 | 35 | 25 | 30 | Background alpha when pressed | Interactive widget presses |

## Shadow (4 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `shadows_enabled` | bool | true | true | false | Drop shadows on cards and modals | Popups, card widgets |
| `shadow_blur` | f32 | 0.0 | 24.0 | 8.0 | Blur radius of drop shadows | Cards, dialogs, `PopupFrame` |
| `shadow_offset_y` | f32 | 0.0 | 8.0 | 4.0 | Vertical offset of drop shadows | Cards, dialogs, `PopupFrame` |
| `shadow_alpha` | u8 | 0 | 40 | 20 | Opacity of drop shadows | Cards, dialogs, `PopupFrame` |

## Overlays & Toasts (2 fields)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `dialog_backdrop_alpha` | u8 | 0 | 0 | 0 | Alpha of overlay behind modal dialogs (0 = none) | Modal dialogs |
| `toast_bg_alpha` | u8 | 230 | 200 | 220 | Background opacity of toast notifications | Toast/notification widgets |

## Density (1 field)

| Field | Type | Meridien | Aperture | Octave | Visible Effect | Where to See It |
|---|---|---|---|---|---|---|
| `density` | u8 | 1 | 2 | 0 | 0=compact, 1=normal, 2=spacious — scales row/button/tab heights | All density-aware widgets |

---

## Field Count Summary

| Category | Count |
|----------|-------|
| Corner Radii | 6 |
| Borders & Strokes | 7 |
| Layout | 11 |
| Typography | 10 |
| Color | 8 |
| Buttons | 9 |
| Inputs | 4 |
| Cards | 5 |
| Tabs | 6 |
| Navigation | 4 |
| Interaction (deduplicated) | 2 |
| Shadow | 4 |
| Overlays & Toasts | 2 |
| Density | 1 |
| **Total (struct fields)** | **77** |

---

## Consumer References

| Helper / location | Field(s) consumed |
|---|---|
| `style::tb_btn` | `r_sm`, `active_fill_color`, `active_text_color`, `button_treatment`, `nav_letter_spacing_px`, `uppercase_section_labels`, `nav_active_col_alpha`, `tab_underline_thickness`, `stroke_bold`, `vertical_group_dividers` |
| `style::apply_ui_style` | `hairline_borders`, `serif_headlines`, `stroke_std`, `stroke_std`, `focus_ring_width`, `input_focus_color` |
| `style::dialog_window_themed` | `r_lg`, `shadows_enabled`, `card_floating_shadow`, `card_floating_shadow_alpha`, `stroke_std` |
| `style::cta_btn` | `active_fill_color`, `active_text_color`, `cta_height_px`, `cta_padding_x`, `r_sm` |
| `style::order_card` | `card_padding_y/x`, `r_md`, `card_stripe_alpha` |
| `style::section_label` | `uppercase_section_labels`, `section_label_padding_top/bottom` |
| `style::split_divider` | `stroke_thick`, `drag_handle_alpha`, `drag_handle_dot_scale` |
| `widgets/pane.rs AccountStrip` | `account_strip_height`, `font_body`, `font_caption` |
| `widgets/select.rs SegmentedControl` | `idle_outline_color`, `segmented_idle_fill`, `segmented_idle_text`, `r_sm`, `r_pill` |
| `widgets/foundation/shell.rs ButtonShell` | `r_sm/md/lg`, `stroke_bold/thin`, `hover_bg_alpha`, `active_bg_alpha`, `button_treatment`, `tab_underline_thickness` |
| `widgets/foundation/tokens.rs Radius` | `r_xs/sm/md/lg/pill` |
| `widgets/frames.rs PopupFrame` | `shadow_offset_y`, `shadow_blur`, `shadow_alpha`, `shadows_enabled` |
| `widgets/foundation/tokens.rs Size` | `button_height_px`, `row_height_px`, `tab_height`, `density` |

---

## How to Test (10 Most-Impactful Operations)

1. **`pane_gap` + `pane_gap_alpha`** — Open a 2-pane layout. Drag `pane_gap` from 0→12 and `pane_gap_alpha` from 0→80 to see the gutter appear with color.
2. **`pane_active_indicator`** — Switch values 0/1/2/3 to see none/top-line/fill/both on the active pane header.
3. **`r_sm` / `r_md`** — Drag from 0→12 with Meridien style to see all buttons/cards round up from square.
4. **`font_hero` + `serif_headlines`** — Set `font_hero` to 48, enable `serif_headlines`: the price display changes to large serif text.
5. **`label_letter_spacing_px`** — Set to 2.0 with `uppercase_section_labels` true: section headers gain tracked-out spacing.
6. **`tab_inactive_alpha`** — Open a multi-tab pane; drag from 1.0 → 0.2 to dim inactive tab labels to near-invisible.
7. **`tab_hover_bg_alpha`** — Hover over inactive tabs; drag from 0 → 60 to see hover background appear.
8. **`card_stripe_alpha`** — Open Orders panel; drag from 255 → 0 to fade out the left accent stripe on order cards.
9. **`toast_bg_alpha`** — Trigger any toast notification; drag from 230 → 80 for a glassmorphic semi-transparent look.
10. **`section_label_padding_top`** — Open any panel with section labels; drag from 0 → 16 to see labels get more breathing room.

---

## Visibility Caveats

- `account_strip_height` — Only visible when the account strip is enabled in settings
- `idle_outline_color` — Only fires in `widgets/select.rs` SegmentedControl (new widget system)
- `segmented_idle_fill` / `segmented_idle_text` — Same as above; only new-widget SegmentedControl
- `tab_underline_under_text` — Only visible in pane headers that are active with `show_active_tab_underline` true
- `dialog_backdrop_alpha` — Reserved; backdrop must be painted explicitly by dialog callers (currently 0 everywhere)
- `nav_active_col_alpha` — Only fires in `UnderlineActive` button treatment (Meridien style toolbar)
- `pane_gap_alpha` / `pane_gap_color` — Only visible when `pane_gap` > 0 and `visible_count` > 1
