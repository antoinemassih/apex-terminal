# StyleSettings Field Trace

Each `StyleSettings` field traced to its consumer code path and the visible UI it affects.

| Field | # Consumers | Primary consumer | Affects |
|---|---|---|---|
| `r_xs` | 16 | `style.rs r_xs()` + many painter `rect_filled` calls | Tiny chip corners, drawing handle radii, indicator badges |
| `r_sm` | 8 | tooltip frame, popup chip backgrounds | Small badges, status pills, filter chips |
| `r_md` | 20 | `r_md_cr()` + popup/dialog frames | Cards, dialog frames, button corners (most-used radius) |
| `r_lg` | 6 | `r_lg_cr()` + dialog windows | Large dialog corners, popup outer shells |
| `r_pill` | 8 | `pills.rs CornerRadius::same(s.r_pill)` | PillButton, removable chips, status pills |
| `button_treatment` | 16 | `tb_btn`, `ButtonShell::show` | Top nav active state, all button widgets' active visuals |
| `hairline_borders` | 65 | scattered everywhere | The biggest cascade — frames, separators, borders, pills, chips |
| `shadows_enabled` | 10 | dialog frames, OHLC tooltip, watchlist tooltip | Drop shadow on/off for all popups |
| `solid_active_fills` | 2 | `pills.rs`, `chips.rs` (active path) | Pills paint solid fill vs ghost on active |
| `uppercase_section_labels` | via `style_label_case` | All section labels, toolbar btn labels, dialog headers | Visible on every uppercased label |
| `serif_headlines` | 4 | `metrics.rs`, `sortable_headers.rs`, `hero_font_id` | Hero numbers font swap |
| `stroke_hair` | 4 | `pills.rs`, `style.rs split_divider` | Meridien hairline borders thinness |
| `stroke_thin` | 28 | dialog frames, separators, table grid, pane border | Most "subtle line" surfaces |
| `stroke_std` | 48 | tooltip border, popup border, drawing handle stroke | Standard borders |
| `stroke_bold` | 4 | `tb_btn` UnderlineActive bottom stripe, `ButtonShell` active | Top nav underline + button active emphasis |
| `stroke_thick` | 3 | `dialog_separator_shadow`, `split_divider` active | Thick separator lines + active-drag splitter |
| `toolbar_height_scale` | 1 | `gpu.rs:3687` top toolbar `exact_height` | Top nav bar height (1.4× = taller for Meridien) |
| `header_height_scale` | 3 | `pane_header_h`, `pane_tabs_header_h` | Pane header bars below the chart toolbar |
| `font_hero` | 2 | `hero_text()` + `widgets/form.rs:700` | Account strip NAV/Daily P&L numbers + Meridien order qty hero |
| `label_letter_spacing_px` | 1 | `style_label_case` (thin-space insertion) | Spacing between letters in uppercase labels |
| `vertical_group_dividers` | 3 | top toolbar `tb_group_break`, account-strip column hover overlay | Vertical dividers between top-nav button groups |
| `show_active_tab_underline` | 2 | pane tab strip, `tab_bar` widget | Underline under active tab on/off |
| `active_header_fill_multiply` | 1 | `gpu.rs:5523` pane header rect_filled | Tint of active pane header bg |
| `inactive_header_fill` | 1 | `gpu.rs:5524` pane header conditional fill | Whether inactive pane headers paint a fill |
| `account_strip_height` | 1 | `gpu.rs:4923` `TopBottomPanel::top("account_strip").exact_height` | Account strip height (Meridien=36, Aperture/Octave/Relay=26) |

## Visibility caveats

These fields ARE wired but only visible when their consumer is on screen:

- **`account_strip_height`** — only visible when Account toggle on (top-nav IBKR button)
- **`font_hero`** — only visible when Account strip OR Meridien order ticket is open
- **`active_header_fill_multiply` / `inactive_header_fill`** — only visible on chart panes with multiple tabs visible
- **`show_active_tab_underline`** — only visible when a pane has 2+ tabs
- **`solid_active_fills`** — visible on pills/chips that have an active state (e.g., Object Tree group headers, segmented controls in the active state)
- **`vertical_group_dividers`** — only on Meridien (gated)

## Heavy-cascade fields (instantly visible across most of the UI)

- `hairline_borders` (65 consumers — flips dialogs, popups, frames, dividers)
- `stroke_std` (48)
- `stroke_thin` (28)
- `r_md` (20 — flips most card/dialog/button corners)
- `r_xs` (16 — flips chip + handle corners)
- `button_treatment` (16 — flips every button's active visual)
- `shadows_enabled` (10 — flips all popup shadows)

## How to verify a slider does something

Some fields show effect only in specific contexts. The fastest way to verify the wiring works:

1. Open the chart with default Meridien
2. Open inspector (F12 if it's not already open)
3. Toggle `hairline_borders` — the entire UI's borders flip thickness. If this doesn't visibly change anything, the cascade is broken.
4. Toggle `shadows_enabled` — open any dialog (settings/templates/etc) and check shadow on/off
5. Slide `r_md` from 0 to 12 — cards/dialogs corners flip from square to rounded
6. Toggle `solid_active_fills` — the active pill in the side panel changes from accent fill to ghost
7. Toggle `vertical_group_dividers` — top nav divider lines appear/disappear
8. Slide `header_height_scale` from 0.8 to 1.4 — pane headers visibly shrink/grow

If any of these don't work, the wiring is genuinely broken. If they all work, every field's wiring is at least functional.
