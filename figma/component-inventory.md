# apex-terminal ui_kit — Complete Component Inventory
<!-- AUTO-GENERATED — do not hand-edit; regenerate via agent task -->
<!-- Last generated: 2026-06-13 -->

## Overview

All widgets live under `src-tauri/src/ui_kit/widgets/`. Every visual widget
accepts `&dyn ComponentTheme` (or reads `active_theme(ctx)` internally) and
resolves colors from the active `PortableTheme`. Dimension tokens come from
`frame_tokens()` / `crate::ui_kit::tokens::*`. Recipe adoption is the marker
for Figma-driven per-component overrides.

### Master Component Table

| Component | File | Recipe Adopted | Variants | Sizes |
|---|---|---|---|---|
| Button | `button.rs` | PENDING | Primary Secondary Ghost Danger Link Chrome Chip Tab InlineClose MutedIcon NeutralAction TextOnly Toggle DynamicTint | Xs Sm Md Lg Xl |
| Tag | `tag.rs` | **YES** (`tag`) | Neutral Accent Bull Bear Warn + outline/dot/closable | Xs Sm |
| Badge | `badge.rs` | No | Count Dot Text × Neutral Accent Bull Bear Warn | — (auto) |
| StatusPill | `status_pill.rs` | No | Default Accent Bull Bear Warn Danger Success Text | Xs Sm |
| CountChip | `count_chip.rs` | No | Muted Accent Bull Bear Warn | — (auto) |
| Kbd | `kbd.rs` | PENDING (`kbd`) | — (key sequence) | Xs Sm |
| Alert | `alert.rs` | No | Info Success Warning Error | — |
| Toast | `toast.rs` | PENDING (`toast`/`toast.success`/`toast.danger`/`toast.warn`) | Info Success Warning Danger | — |
| Tabs | `tabs.rs` | **YES** (`tab.line.active`, `tab.pill`) | Line Segmented Filled Card Pane | Sm Md |
| SegmentedControl | `segmented_control.rs` | No | connected / separated | Xs Sm Md Lg |
| ToggleGroup | `toggle_group.rs` | No (delegates to Button/Chip) | — | Sm (default) |
| Switch | `switch.rs` | No | on / off | Sm Md |
| Checkbox | `checkbox.rs` | No | Off On Indeterminate | Sm Md |
| Radio | `radio.rs` | No | Off On | Sm Md |
| Input | `input.rs` | No | default hover focus disabled error warning | Sm Md Lg |
| SearchInput | `search_input.rs` | No | — (Input preset) | — |
| TextArea | `text_area.rs` | No | — (multiline Input) | — |
| Select | `select.rs` | No | Single Multi | — |
| NumberStepper | `number_stepper.rs` | No | — | Sm (default) |
| Slider | `slider.rs` | No | Primary Danger | Xs Sm Md Lg |
| RangeSlider | `range_slider.rs` | No | Primary | Xs Sm Md Lg |
| DatePicker | `date_picker.rs` | No | Single Range | — |
| TimePicker | `time_picker.rs` | No | — | — |
| ColorPicker | `color_picker.rs` | No | compact inline | — |
| Tooltip | `tooltip.rs` | No | Text Rich | — |
| HoverCard | `hover_card.rs` | No | — | — |
| Popover | `popover.rs` | No | — | — |
| ContextMenu | `context_menu.rs` | No | — | — |
| Modal | `modal.rs` | No | Pane Dialog Panel None (header style) | — |
| Sheet | `sheet.rs` | No | Left Right Top Bottom (side) | Fixed Percent Auto |
| Header | `header.rs` | No | Panel Dialog Section | — |
| PanelSection | `panel_section.rs` | **YES** (`section.header`) | Default Accent Bull Bear Warn Danger Success Text | — |
| PanelListRow | `panel_list_row.rs` | **YES** (`row.list`) | default selected hover | — |
| PanelCard | `panel_card.rs` | No | Default Accent Bull Bear Warn Danger Success Text | — |
| PanelKeyValueRow | `panel_key_value_row.rs` | No | — | — |
| PanelToolbar | `panel_toolbar.rs` | No | — | — |
| PanelEmpty | `panel_empty.rs` | No | — | — |
| PanelLoading | `panel_loading.rs` | No | — | — |
| PanelError | `panel_error.rs` | No | — | — |
| PanelSubSection | `panel_sub_section.rs` | No | — | — |
| MetricRow | `metric_row.rs` | No | Default Muted Accent Bull Bear Warn | — |
| SelectableRow | `selectable_row.rs` | No | default selected disabled | Xs Sm Md Lg Xl |
| OutlinedBox | `outlined_box.rs` | No | None Hairline Standard Bold (border tier) | — |
| Separator | `separator.rs` | No | Horizontal Vertical (+ label option) | — |
| Sparkline | `sparkline.rs` | No | Line Bars | — |
| Progress | `progress.rs` | No | Primary Danger × Linear Circular | — |
| Spinner | `spinner.rs` | No | — | Sm (default) |
| Skeleton | `skeleton.rs` | No | rect text lines circle | — |
| RiskRewardBar | `risk_reward_bar.rs` | No | — | — |
| Indicator | `indicator.rs` | No | Neutral Accent Bull Bear Warn Custom × Dot Ring Pulsing | — |
| Table | `table.rs` | No | — | — |
| TableHeader | `table_header.rs` | No | — | — |
| Tree | `tree.rs` | No | — | — |
| Stepper | `stepper.rs` | No | — | — |
| Sidebar | `sidebar.rs` | No | — | — |
| Label | `label.rs` | No | — | — |
| Link | `link.rs` | No | — | — |
| TagInput | `tag_input.rs` | No | — | — |
| ToggleRow | `toggle_row.rs` | No | — | — |
| TradeCard | `trade_card.rs` | No | — | — |
| ConfirmDialog | `confirm_dialog.rs` | No | — | — |
| FormField | `form_field.rs` | No | — | — |
| FormRow | `form_row.rs` | No | — | — |
| FormSection | `form_section.rs` | No | — | — |
| FormActionBar | `form_action_bar.rs` | No | — | — |
| Fieldset | `fieldset.rs` | No | — | — |
| ToolbarButton | `toolbar_button.rs` | No | — | — |
| ToolOverlay | `tool_overlay.rs` | No | — | — |
| ToolPopover | `tool_popover.rs` | No | — | — |
| GuildAvatarGrid | `guild_avatar_grid.rs` | No | — | — |
| HeatmapGrid | `heatmap_grid.rs` | No | — | — |
| Calendar | `calendar.rs` | No | Single Range | — |
| OpacityPicker | `opacity_picker.rs` | No | — | — |
| ThemePreviewCard | `theme_preview_card.rs` | No | — | — |
| ScrollArea | `scroll_area.rs` | No | — | — |

---

## Section 1 — Primitive / Ink

### Button
**File:** `src-tauri/src/ui_kit/widgets/button.rs`
**Recipe keys:** `button.primary` · `button.ghost` · `button.danger` · `button.success` · `button.chrome` (all PENDING — not yet hooked via `resolve()`)

#### Dimension table

| Variant | Size | Height | Corner radius | Notes |
|---|---|---|---|---|
| Primary / Secondary / Danger / NeutralAction | Xs | 18 px | 4 px | |
| Primary / Secondary / Danger / NeutralAction | Sm | 22 px | 4 px | |
| Primary / Secondary / Danger / NeutralAction | Md | 28 px | 4 px | default |
| Primary / Secondary / Danger / NeutralAction | Lg | 34 px | 4 px | |
| Primary / Secondary / Danger / NeutralAction | Xl | 40 px | 4 px | |
| Ghost / MutedIcon / InlineClose / DynamicTint | any | same as above | 2 px | |
| Link / TextOnly / Tab | any | same as above | 0 px | frameless |
| Chip / Toggle | any | same as above | 99 px | pill |
| InlineClose | Sm | 18 px sq | 2 px | icon-only; hit area = height |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Primary fill | `theme.accent()` | full opacity |
| Primary fg | `theme.text()` contrast over accent | |
| Secondary fill | transparent | border only |
| Secondary border | `theme.border()` at `stroke_thin()` | |
| Ghost fill | transparent until hover | hover = `color_alpha(text, alpha_faint())` |
| Danger fill | `theme.bear()` | |
| Link / TextOnly fg | `theme.accent()` (default) or caller `.fg()` | underline on hover for Link |
| Chip inactive fill | transparent | |
| Chip inactive fg | `color_alpha(dim, 0.5)` | |
| Chip active fill | soft accent | |
| Chip active fg | accent | |
| Toggle active fill | accent-tinted bg | accent border |
| DynamicTint fg/hover | caller `.tint(color)` | falls back to Ghost if unset |
| Disabled overlay | `alpha_ghost()` | all variants |
| Loading spinner | accent | replaces label |
| Buy tint | `color_alpha(bull, ...)` | `.buy()` preset |
| Sell tint | `color_alpha(bear, ...)` | `.sell()` preset |

#### Named presets

| Preset | Variant | Size | Notes |
|---|---|---|---|
| `Button::buy()` | Primary | Md | bull tint fill |
| `Button::sell()` | Primary | Md | bear tint fill |
| `Button::action()` | Secondary | Sm | |
| `Button::trade()` | Primary | Md | |
| `Button::cta()` | Primary | Lg | |
| `Button::small_action()` | Ghost | Xs | |
| `Button::toolbar()` | Ghost | Sm | icon-only |
| `Button::menu()` | Ghost | Md | full-width left-aligned |
| `Button::close()` | InlineClose | — | |
| `Button::toggle(active)` | Toggle | Sm | active state bound |

#### Figma variant matrix guidance

Figma props: `Variant` (Primary/Secondary/Ghost/Danger/Link/Chrome/Chip/Tab/InlineClose/MutedIcon/NeutralAction/TextOnly/Toggle/DynamicTint) × `Size` (Xs/Sm/Md/Lg/Xl) × `State` (Default/Hover/Pressed/Active/Disabled/Loading) × `IconOnly` (bool) × `HasIcon` (bool).

---

### Tag
**File:** `src-tauri/src/ui_kit/widgets/tag.rs`
**Recipe key:** `tag` — **ADOPTED** via `recipes.resolve("tag", default_chip_sx, theme)`

#### Dimension table

| Size | Height | Corner | Font |
|---|---|---|---|
| Xs | ~16 px | pill (h × 0.5) | `font_xs` |
| Sm | ~20 px | pill (h × 0.5) | `font_sm` |
| Md+ | clamped to Sm | — | — |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Fill (standard) | `color_alpha(tone_color, 32)` | tone: Neutral→Dim / Accent / Bull / Bear / Warn |
| Fill (outline) | transparent | `.outline(true)` |
| Border (outline) | `color_alpha(tone_color, 200)` at `stroke_thin()` | |
| Text | `tone_color` | |
| Dot | `tone_color` · 6 px filled circle left of label | optional `.dot(true)` |
| Close btn fg | `color_alpha(tone_color, 180)` | |
| Disabled | `alpha_ghost()` overlay | |

#### Painter-level helpers (also in tag.rs)

`paint_pill(painter, rect, style, color)` — PillStyle: Soft / Subtle / Solid / Outline.
`paint_badge(painter, rect, tone, text)` — used by CountChip and Badge internally.

#### Figma variant matrix guidance

Figma props: `Tone` (Neutral/Accent/Bull/Bear/Warn) × `Size` (Xs/Sm) × `Outline` (bool) × `Dot` (bool) × `Closable` (bool) × `Disabled` (bool).

---

### Badge
**File:** `src-tauri/src/ui_kit/widgets/badge.rs`

#### Dimension table

| Kind | Size | Notes |
|---|---|---|
| Dot | 8 px circle | no text |
| Count | auto (pill) | mono 10 px; truncates at max_count with `{n}+` |
| Text | auto (pill) | mono 10 px |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Fill | `tone_color` solid | high contrast |
| Text | `contrast_fg(tone_color)` | black or white for legibility |
| tone_color_override | caller-supplied Color32 | overrides tone enum |

#### Figma variant matrix guidance

Figma props: `Kind` (Count/Dot/Text) × `Tone` (Neutral/Accent/Bull/Bear/Warn) × `MaxCount` (number).

---

### StatusPill
**File:** `src-tauri/src/ui_kit/widgets/status_pill.rs`

#### Dimension table

| Size | Corner |
|---|---|
| Xs | `rounded_sm` |
| Sm | `rounded_md` |
| Md+ | clamped to Sm |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Fill | `color_alpha(tone, alpha_hint())` | |
| Border | `color_alpha(tone, alpha_muted())` at `stroke_thin()` | |
| Text | tone color | |
| Dot | tone color · 6 px circle | optional `.dot(true)` |

#### Figma variant matrix guidance

Figma props: `Tone` (Default/Accent/Bull/Bear/Warn/Danger/Success/Text) × `Size` (Xs/Sm) × `Dot` (bool).

---

### CountChip
**File:** `src-tauri/src/ui_kit/widgets/count_chip.rs`

#### Dimension table

| Attribute | Value |
|---|---|
| Fixed height | 14 px |
| Font | mono_xs |
| Corner | `radius_sm` |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Fill | `color_alpha(fill_base, 38)` | fill_base from tone |
| Border | `color_alpha(fg, 60)` at `stroke_thin()` | |
| Text | tone fg | |

`paint_at(painter, center, ...)` — painter-mode for HUD overlays.

#### Figma variant matrix guidance

Figma props: `Tone` (Muted/Accent/Bull/Bear/Warn) × `MaxCount` (number, default 99).

---

### Kbd
**File:** `src-tauri/src/ui_kit/widgets/kbd.rs`
**Recipe key:** `kbd` (PENDING)

#### Dimension table

| Size | Font |
|---|---|
| Xs | 9 px (intentionally small for keycaps — documented exception to type scale) |
| Sm | 10 px |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Key cap fill | `Sx::new().rounded_sm().bg_alpha(Tone::Surface, 200)` | |
| Key cap border | `Tone::Border` at `stroke_std()` | |
| Separator | dim text | `+` between keys |

#### Figma variant matrix guidance

Figma props: `Size` (Xs/Sm) × `Keys` (array of key strings). Renders as sequence of caps with `+` separators.

---

## Section 2 — Feedback / Notification

### Alert
**File:** `src-tauri/src/ui_kit/widgets/alert.rs`

#### Token table

| Part | Token | Notes |
|---|---|---|
| Box fill | `Sx::new().rounded_md().bg_alpha(box_tone, 32)` | |
| Box border | `border_alpha(box_tone, 200, stroke_std())` | |
| Icon | 18 px; tone color | Info→accent / Success→bull / Warning→warn / Error→bear |
| Title | `font_sm` semibold via `polished_label` | |
| Body | `font_sm` dim | |
| Close btn | InlineClose | optional |

#### Figma variant matrix guidance

Figma props: `Variant` (Info/Success/Warning/Error) × `HasTitle` (bool) × `HasBody` (bool) × `Closable` (bool).

---

### Toast
**File:** `src-tauri/src/ui_kit/widgets/toast.rs`
**Recipe keys:** `toast` · `toast.success` · `toast.danger` · `toast.warn` (all PENDING)

#### Dimension table

| Attribute | Value |
|---|---|
| Default width | 280 px |
| Corner | `r_md_cr()` |
| Padding | `gap_lg()` all sides |
| Accent stripe | 3 px wide, left edge, height = `font_md + font_sm + 6` |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Background | `color_alpha(surface, toast_bg_alpha)` | toast_bg_alpha from `frame_tokens()` |
| Border | `color_alpha(border, alpha_strong())` at `stroke_thin()` | |
| Accent stripe | variant color (accent/bull/warn/bear) | |
| Title | `font_md` monospace strong, theme text color | |
| Body | `font_sm` monospace, `color_alpha(text, alpha_dim())` | |
| Entrance | slide 12 px from right + fade over `motion::FAST` | |

#### Figma variant matrix guidance

Figma props: `Variant` (Info/Success/Warning/Danger) × `HasBody` (bool) × `AutoDismiss` (bool) × `Width` (number, default 280).

---

### Indicator
**File:** `src-tauri/src/ui_kit/widgets/indicator.rs`

#### Token table

| Style | Rendering |
|---|---|
| Dot | filled circle, `size_px` diameter |
| Ring | hollow circle, `stroke_thin()` |
| Pulsing | Dot + outer ring that animates 0→1 opacity over 1500 ms loop |

#### Figma variant matrix guidance

Figma props: `Tone` (Neutral/Accent/Bull/Bear/Warn/Custom) × `Style` (Dot/Ring/Pulsing) × `SizePx` (number, default 6) × `Label` (string, optional).

---

### Progress / Spinner / Skeleton
**Files:** `progress.rs`, `spinner.rs`, `skeleton.rs`

#### Progress token table

| Part | Token |
|---|---|
| Track | `color_alpha(surface, alpha_soft())` |
| Fill (Primary) | accent |
| Fill (Danger) | bear |
| Indeterminate | sliding accent segment |

#### Skeleton token table

| Part | Token |
|---|---|
| Base | `color_alpha(surface, alpha_subtle())` |
| Shimmer | animated lighter band via palette |

**Shapes:** `rect(w,h)` · `text(w)` (font_sm height) · `lines(count, w)` · `circle(d)`

**Spinner** — thin wrapper: `Progress::circular_indeterminate()`, default Size Sm.

---

## Section 3 — Inputs

### Switch
**File:** `src-tauri/src/ui_kit/widgets/switch.rs`

#### Dimension table

| Size | Track W | Track H | Thumb diam |
|---|---|---|---|
| Sm | 26 px | 14 px | 10 px |
| Md | 32 px | 20 px | 12 px |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Track off | `color_alpha(dim, 64)` | |
| Track on | `theme.accent()` | |
| Thumb | white (contrast) | `ease_out_back` overshoot animation |
| Focus ring | `st::cursor::focus_ring` | |

#### Figma variant matrix guidance

Figma props: `State` (Off/On/Disabled) × `Size` (Sm/Md) × `HasLabel` (bool).

---

### Checkbox
**File:** `src-tauri/src/ui_kit/widgets/checkbox.rs`

#### Dimension table

| Size | Box side |
|---|---|
| Sm | 14 px |
| Md | 16 px |

#### Token table

| State | Fill | Border |
|---|---|---|
| Off | transparent | `theme.border()` |
| On | `theme.accent()` | accent |
| Indeterminate | `theme.accent()` | accent |

Checkmark: white `✓`. Indeterminate: white horizontal dash.

#### Figma variant matrix guidance

Figma props: `State` (Off/On/Indeterminate/Disabled) × `Size` (Sm/Md) × `HasLabel` (bool).

---

### Radio
**File:** `src-tauri/src/ui_kit/widgets/radio.rs`

#### Dimension table

| Size | Diameter |
|---|---|
| Sm | 14 px |
| Md | 16 px |

#### Token table

| State | Fill | Inner dot |
|---|---|---|
| Off | transparent | — |
| On | `theme.accent()` | white dot radius = `(d-6)*0.5` |

Uses `ease_bool` animations.

#### Figma variant matrix guidance

Figma props: `State` (Off/On/Disabled) × `Size` (Sm/Md) × `HasLabel` (bool).

---

### Input
**File:** `src-tauri/src/ui_kit/widgets/input.rs`

#### Token table

| Part | Token | Notes |
|---|---|---|
| Border default | `theme.border()` at `stroke_thin()` | |
| Border hover | accent-tinted | |
| Border focus | `theme.accent()` full | |
| Border error | `theme.bear()` | `.invalid(true)` |
| Border warning | `theme.warn()` | `.warning(true)` |
| Fill | `theme.surface()` | |
| Fill disabled | `color_alpha(surface, alpha_soft())` | |
| Text | `theme.text()` | |
| Placeholder | `color_alpha(text, alpha_muted())` | |
| Leading/trailing icon | `color_alpha(text, alpha_dim())` | |
| Clear btn | InlineClose variant | |

`Input::number()` — right-aligned, frameless preset.

Returns `InputResponse` with `editor_id` for focus control.

#### Figma variant matrix guidance

Figma props: `State` (Default/Hover/Focus/Disabled/Error/Warning) × `Size` (Sm/Md/Lg) × `HasLeadingIcon` (bool) × `HasTrailingIcon` (bool) × `HasPrefix` (bool) × `HasSuffix` (bool) × `Clearable` (bool) × `Password` (bool) × `Multiline` (bool) × `Frameless` (bool).

---

### SearchInput
**File:** `src-tauri/src/ui_kit/widgets/search_input.rs`

Input preset with magnifier leading icon and clear button. Same states as Input.

---

### TextArea
**File:** `src-tauri/src/ui_kit/widgets/text_area.rs`

Multiline variant of Input. Same token set; height is variable (rows or auto-grow).

---

### Select
**File:** `src-tauri/src/ui_kit/widgets/select.rs`

#### Token table

| Part | Token |
|---|---|
| Trigger | same as Input (border/fill/text) |
| Dropdown fill | `theme.surface()` |
| Dropdown border | `theme.border()` at `stroke_thin()` |
| Item hover | `color_alpha(text, alpha_faint())` |
| Item selected | `color_alpha(accent, alpha_soft())` |
| Search input | embedded Input (frameless) |

#### Figma variant matrix guidance

Figma props: `Mode` (Single/Multi) × `State` (Default/Open/Disabled) × `Searchable` (bool) × `CompactTrigger` (bool).

---

### NumberStepper
**File:** `src-tauri/src/ui_kit/widgets/number_stepper.rs`

Thin wrapper around `egui::DragValue`. Drag up/down to change value. Same border/fill tokens as Input. Default Size Sm.

---

### Slider
**File:** `src-tauri/src/ui_kit/widgets/slider.rs`

#### Token table

| Part | Token |
|---|---|
| Track | `color_alpha(border, alpha_subtle())` |
| Fill (Primary) | `theme.accent()` |
| Fill (Danger) | `theme.bear()` |
| Thumb | white / accent |
| Tick marks | `color_alpha(dim, alpha_line())` |

#### Figma variant matrix guidance

Figma props: `Variant` (Primary/Danger) × `Size` (Xs/Sm/Md/Lg) × `ShowValue` (bool) × `Disabled` (bool) × `Ticks` (bool).

---

### RangeSlider
**File:** `src-tauri/src/ui_kit/widgets/range_slider.rs`

Two-thumb slider. Filled segment between thumbs uses `color_alpha(accent, alpha_active())`.

---

### DatePicker / TimePicker
**Files:** `date_picker.rs` / `time_picker.rs`

Both open a popover. DatePicker uses Calendar internally. TimePicker has hour:minute(:second) columns.

---

### ColorPicker / OpacityPicker
**Files:** `color_picker.rs` / `opacity_picker.rs`

ColorPicker: SV area 180 px sq, hue rail 16 px H, preset swatches 20 px. With-alpha mode adds opacity rail.

---

### TagInput
**File:** `src-tauri/src/ui_kit/widgets/tag_input.rs`

Comma-separated tag entry. Each confirmed tag renders as a Tag (Neutral, closable). Input renders inline after last tag.

---

## Section 4 — Navigation

### Tabs
**File:** `src-tauri/src/ui_kit/widgets/tabs.rs`
**Recipe keys:** `tab.line.active` · `tab.pill` — **ADOPTED**

#### Dimension table

| Treatment | Tab height | Underline | Corner |
|---|---|---|---|
| Line | 31 px (ZED_TAB_HEIGHT) | animated sliding 2 px bar | 0 |
| Segmented | Md: 28 px / Sm: 22 px | — | `radius_sm` outer, `radius_xs` inner |
| Filled | same as Segmented | — | `radius_sm` |
| Card | 28 px | — | `radius_sm` |
| Pane | 28 px | — | `radius_sm` |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Line underline color | `recipes.resolve("tab.line.active", ...)` → accent | Figma-overridable |
| Segmented/Filled radius | `recipes.resolve("tab.pill", ...)` | Figma-overridable |
| Active tab fill (Segmented) | `color_alpha(accent, alpha_tint())` | |
| Active tab text | accent | |
| Inactive tab text | `color_alpha(text, alpha_dim())` | |
| Badge on tab | CountChip | |
| Modified dot | 6 px bull dot | |
| Close btn | InlineClose | |

#### Figma variant matrix guidance

Figma props: `Treatment` (Line/Segmented/Filled/Card/Pane) × `Size` (Sm/Md) × `State` (Default/Active/Hover/Disabled) × `HasIcon` (bool) × `HasBadge` (bool) × `Closable` (bool) × `Modified` (bool).

---

### SegmentedControl
**File:** `src-tauri/src/ui_kit/widgets/segmented_control.rs`

#### Dimension table

| Size | Height | Corner (outer) |
|---|---|---|
| Xs | 18 px | 4 px |
| Sm | 22 px | 4 px |
| Md | 28 px | 4 px |
| Lg | 34 px | 4 px |

Connected mode: one outer container with hairline dividers between segments.
Separated mode: individual pill buttons with gaps.

#### Token table

| Part | Token |
|---|---|
| Container bg | `color_alpha(surface, alpha_subtle())` |
| Container border | `theme.border()` at `stroke_thin()` |
| Active segment fill | `color_alpha(accent, alpha_tint())` |
| Active segment text | accent |
| Divider | `color_alpha(border, alpha_line())` |

---

### ToggleGroup
**File:** `src-tauri/src/ui_kit/widgets/toggle_group.rs`

Row of `Variant::Chip` buttons. Delegates all styling to Button/Chip. Returns merged Response with `.changed()`.

---

### Sidebar
**File:** `src-tauri/src/ui_kit/widgets/sidebar.rs`

Navigation sidebar panel. Same surface/border tokens as Panel.

---

## Section 5 — Panels & Layout

### Header
**File:** `src-tauri/src/ui_kit/widgets/header.rs`

#### Dimension table

| Variant | Height | Typography |
|---|---|---|
| Panel | 28 px | `font_xs` uppercase |
| Dialog | 36 px | `font_md` mixed-case |
| Section | 22 px | `font_xs` uppercase |

#### Token table

| Part | Token | Notes |
|---|---|---|
| Panel bottom rule | `stroke_thin()` at `alpha_faint()` | |
| Panel bg | `theme.surface()` faint | |
| Dialog | no rule, no bg fill | |
| Section | no rule, no fill | |
| Leading icon | 16 px | |
| Trailing | caller closure | |

Returns `HeaderResponse` with rect + response.

---

### PanelSection
**File:** `src-tauri/src/ui_kit/widgets/panel_section.rs`
**Recipe key:** `section.header` — **ADOPTED**

#### Token table

| Part | Token |
|---|---|
| Header bg | `recipes.resolve("section.header", ...)` |
| Count chip | CountChip (Muted tone) |
| Action btn | Button Ghost Xs |
| Delete btn | Button Danger Xs |
| Chevron | 12 px icon, rotates 0↔90° |
| Divider rule | `stroke_thin()` at `alpha_line()` |

Returns `SectionResponse` with action_clicked / delete_clicked / header_response / chevron_clicked.

---

### PanelListRow
**File:** `src-tauri/src/ui_kit/widgets/panel_list_row.rs`
**Recipe key:** `row.list` — **ADOPTED**

#### Dimension table

| Density | Height |
|---|---|
| Dense | 22 px |
| Comfortable | 32 px |

#### Token table

| State | Fill | Notes |
|---|---|---|
| Default | transparent | |
| Hover | `color_alpha(Text, 8)` | |
| Selected | `color_alpha(Accent, 24)` fill + 2 px accent left stripe | |

Text columns: Primary = `mono_sm` Text · Secondary = `mono_xs` Dim muted.
TrailingBtn tones: Default / Accent / Bull / Bear / Warn / Muted.

---

### PanelCard
**File:** `src-tauri/src/ui_kit/widgets/panel_card.rs`

#### Token table

| Part | Token |
|---|---|
| Fill | `t.color_layer_up(1)` (L2 surface) |
| Corner | `radius_md` |
| Border | none by default |
| Shadow | present when tone ≠ Default |
| Stripe | 2 px left bar in tone color (optional) |
| Padding | `gap_md` default |

---

### PanelKeyValueRow
**File:** `src-tauri/src/ui_kit/widgets/panel_key_value_row.rs`

| Part | Token |
|---|---|
| Height | `gap_lg` (16 px) |
| Label | `mono_xs` muted Dim |
| Value | `mono_sm` in tone color or text |
| Meta | `mono_xs` very muted |

---

### PanelToolbar
**File:** `src-tauri/src/ui_kit/widgets/panel_toolbar.rs`

| Part | Token |
|---|---|
| Height | 22 px |
| Background | `color_alpha(surface_border, alpha_ghost())` |
| Bottom hairline | `stroke_thin()` at alpha 60 |
| Label (left) | `mono_xs` dim, `gap_md` padding |

---

### PanelEmpty / PanelLoading / PanelError
**Files:** `panel_empty.rs` / `panel_loading.rs` / `panel_error.rs`

All are vertically-centered state-feedback panels. Min height 64 px.

| Widget | Contents |
|---|---|
| PanelEmpty | glyph `font_xl` muted + title `mono_sm` dim + hint `mono_xs` muted |
| PanelLoading | Spinner + optional reason text `mono_xs` muted |
| PanelError | Icon::WARNING bear color + message `mono_sm` dim + hint + optional retry button |

---

### PanelSubSection
**File:** `src-tauri/src/ui_kit/widgets/panel_sub_section.rs`

| Part | Value |
|---|---|
| Header height | 30 px (HEADER_H constant) |
| Caret | 12 px proportional, rotates on expand |
| Hover bg | HOVER_BG_ALPHA = 8 |
| Divider rule | RULE_ALPHA = 80 |
| Body indent | `gap_md` when expanded |

---

### OutlinedBox
**File:** `src-tauri/src/ui_kit/widgets/outlined_box.rs`

Container with configurable border tier (None / Hairline / Standard / Bold), fill, corner radius, and padding. Wraps arbitrary egui UI closure.

---

### Separator
**File:** `src-tauri/src/ui_kit/widgets/separator.rs`

| Style | Color |
|---|---|
| Standard | `pal.base(Tone::Border)` at `alpha_line()` |
| Faint | `pal.base(Tone::Border)` at `alpha_muted()` |

Optional inline label in `font_xs` dim. Orientation: Horizontal / Vertical.

---

## Section 6 — Overlays

### Tooltip
**File:** `src-tauri/src/ui_kit/widgets/tooltip.rs`

| Attribute | Value |
|---|---|
| Delay | `motion::DELAY_TOOLTIP × 1000 ms` |
| Placement | Side::Top (default) |
| Shadow | `shadow_tooltip_themed(theme)` |
| Entrance | `motion::ease_bool` over `motion::FAST` |
| Content | `Content::Text(&str)` or `Content::Rich(FnOnce)` |

---

### HoverCard
**File:** `src-tauri/src/ui_kit/widgets/hover_card.rs`

| Attribute | Value |
|---|---|
| Delay | `motion::DELAY_HOVER_CARD × 1000 ms` |
| Placement | Side::Bottom (default) |
| Sticky | stays open when pointer moves from trigger to card |

---

### Popover
**File:** `src-tauri/src/ui_kit/widgets/popover.rs`

| Attribute | Token |
|---|---|
| Frame | `theme.surface()` fill + `theme.border()` border |
| Corner | `radius_sm` |
| Padding | `gap_sm` |
| Modal option | backdrop overlay |

---

### ContextMenu
**File:** `src-tauri/src/ui_kit/widgets/context_menu.rs`

Constructed via `ContextMenu::new(theme)` — documented API variance (not `show(ui, theme)`). MenuTheme snapshot (Copy): accent / dim / bg / fg / danger / shadow. Anchor: Pos or Response.

---

### Modal
**File:** `src-tauri/src/ui_kit/widgets/modal.rs`

Constructed via builder, takes `ctx + theme` — documented API variance from standard `show(ui, theme)` pattern.

| HeaderStyle | Description |
|---|---|
| Pane | floating pane header with close |
| Dialog | dialog header with close |
| Panel `{title_size, title_size_px, title_monospace, leading_space, trailing_space}` | panel header variant |
| None | no header |

Anchor: `Window{pos}` or `Area{pos}`.

---

### Sheet
**File:** `src-tauri/src/ui_kit/widgets/sheet.rs`

| Attribute | Options |
|---|---|
| Side | Left / Right / Top / Bottom |
| Size | Fixed(f32) / Percent(f32) / Auto |
| Animation | slide-in from edge |
| Options | close_on_backdrop / close_on_escape / modal / title |

---

### ConfirmDialog
**File:** `src-tauri/src/ui_kit/widgets/confirm_dialog.rs`

Modal with confirm/cancel actions. Uses Danger button for destructive confirm.

---

## Section 7 — Data Visualization

### Sparkline
**File:** `src-tauri/src/ui_kit/widgets/sparkline.rs`

| Attribute | Default |
|---|---|
| Default size | 32 × 12 px |
| Style | Line / Bars |
| API | `show(ui, theme)` or `paint(painter, rect)` |

Colors: caller-supplied `color` (line) or `bar_color` callback per bar.

---

### MetricRow
**File:** `src-tauri/src/ui_kit/widgets/metric_row.rs`

| Part | Token |
|---|---|
| Label | `mono_xs` Dim muted |
| Value | `mono_sm` in tone color (Default/Muted/Accent/Bull/Bear/Warn) |
| Delta | optional; positive → bull color, negative → bear color |

---

### RiskRewardBar
**File:** `src-tauri/src/ui_kit/widgets/risk_reward_bar.rs`

| Attribute | Default |
|---|---|
| Default width | 200 px |
| Height | 6 px |
| Risk segment | bear color (left) |
| Reward segment | bull color (right) |

---

### SelectableRow
**File:** `src-tauri/src/ui_kit/widgets/selectable_row.rs`

#### Dimension table

| Size | Height |
|---|---|
| Xs | 18 px |
| Sm / Md | `gap_2xl` (24 px) |
| Lg / Xl | 28 px |

#### Token table

| State | Fill | Text |
|---|---|---|
| Default | transparent | `theme.text()` |
| Hover | `color_alpha(Text, alpha_faint())` | `theme.text()` |
| Selected | `color_alpha(Accent, alpha_soft())` | accent |
| Disabled | `alpha_ghost()` overlay | dim |

---

### Table / TableHeader
**Files:** `table.rs` / `table_header.rs`

Data table with sortable columns. TableHeader uses the panel_toolbar token set (22 px height, same bg/border/font tokens).

---

### HeatmapGrid
**File:** `src-tauri/src/ui_kit/widgets/heatmap_grid.rs`

Grid visualization. Cell colors are caller-supplied (bull/bear/neutral scale).

---

## Section 8 — Form System

| Widget | File | Notes |
|---|---|---|
| FormField | `form_field.rs` | label + input + error/hint text row |
| FormRow | `form_row.rs` | horizontal layout for inline fields |
| FormSection | `form_section.rs` | grouped fields with optional title |
| FormActionBar | `form_action_bar.rs` | submit/cancel buttons docked to bottom |
| Fieldset | `fieldset.rs` | groups related FormFields with a legend |

Form fields follow the same border/fill/error tokens as Input.

---

## Section 9 — Structural / Internal

| Widget | File | Notes |
|---|---|---|
| ButtonStyle | `button_style.rs` | ButtonStyle trait + DefaultButtonStyle + ButtonState enum |
| Calendar | `calendar.rs` | Used by DatePicker; not typically used standalone |
| StyleCtx | `ctx.rs` | S5 opt-in entry point for Sx-based styling |
| Frames | `frames.rs` | DialogHeaderWithClose / PaneHeaderWithClose / PanelHeaderWithClose / PopupFrame / SectionLabelSize |
| IconPlacement | `icon_placement.rs` | Toolbar/PanelHeader/TabClose/Modal/etc — glyph_px/hit_px/hover_bg/interactive |
| InputGroup | `input_group.rs` | Composes multiple Inputs into a joined group |
| Label | `label.rs` | Themed text label |
| Link | `link.rs` | Underline-on-hover text link |
| MenuItem | `menu_item.rs` | Item for ContextMenu |
| Motion | `motion.rs` | ease_bool / ease_value / lerp_color / ease_out_back / FAST / MED / DELAY_TOOLTIP / DELAY_HOVER_CARD |
| PaneGrid | `pane_grid.rs` | Tiling pane layout |
| Placement | `placement.rs` | Side (Top/Bottom/Left/Right) + Align positioning |
| PolishedLabel | `polished_label.rs` | Subpixel-rendered label via cosmic-text |
| Resizable | `resizable.rs` | Resizable panel primitive |
| ScrollArea | `scroll_area.rs` | Themed egui ScrollArea |
| Shadow / ShadowPipeline | `shadow.rs` / `shadow_pipeline.rs` | Shadow rendering system |
| ShellVariants | `shell_variants.rs` | Shell variant enums |
| Stepper | `stepper.rs` | Multi-step workflow progress indicator |
| TextEngine / TextSubpixelPipeline | `text_engine.rs` / `text_subpixel_pipeline.rs` | Text rendering internals |
| Theme | `theme.rs` | ComponentTheme trait / PortableTheme / active_theme() / get_ambient_recipes() |
| ThemePreviewCard | `theme_preview_card.rs` | Live theme preview card |
| ToggleRow | `toggle_row.rs` | Toggle embedded in a row context |
| ToolOverlay | `tool_overlay.rs` | Drawing tool overlay |
| ToolbarButton | `toolbar_button.rs` | Toolbar-specific button preset |
| ToolPopover | `tool_popover.rs` | Tool popover for drawing/chart tools |
| TradeCard | `trade_card.rs` | Trade summary card |
| Tree | `tree.rs` | Hierarchical tree widget |
| GuildAvatarGrid | `guild_avatar_grid.rs` | Avatar grid (watchlist/leaderboard) |

---

## Frames (Screen Regions)

```
TradingScreen
├── TopNav                    (nav.cluster / nav.cluster.active recipe keys)
│   ├── Brand / Logo
│   ├── Primary nav items     (Tab treatment or Button/Tab variant)
│   └── Trailing actions      (Button Ghost Sm)
├── MainContent
│   ├── HeaderStrip           (PanelToolbar token set, 22 px)
│   ├── Toolbar               (Button Ghost Sm icon-only / ToolbarButton)
│   ├── ChartArea             (canvas — colors only cross over from design)
│   └── Panel rail (right)    (PanelSection / PanelListRow / PanelCard stacked)
├── BottomDock                (panel.footer recipe key)
│   ├── Trade input forms     (Input / Select / NumberStepper / Slider)
│   └── Order confirmation    (Button Primary / Danger)
└── RightRail                 (PanelCard / PanelKeyValueRow / PanelSubSection)
    ├── PositionCard          (PanelCard + StatusPill + MetricRow)
    └── WatchlistRows         (PanelListRow + Sparkline + CountChip)
```

---

## Recipe System Coverage

| Recipe key | Status | Widget | What it controls |
|---|---|---|---|
| `button.primary` | PENDING | Button | Primary fill / radius |
| `button.ghost` | PENDING | Button | Ghost hover overlay |
| `button.danger` | PENDING | Button | Danger fill |
| `button.success` | PENDING | Button | Success fill |
| `button.chrome` | PENDING | Button | Chrome free-form |
| `tab.line` | PENDING | Tabs | Line tab container |
| `tab.line.active` | **ADOPTED** | Tabs | Active underline color |
| `tab.pill` | **ADOPTED** | Tabs | Segmented/Filled inner radius |
| `row.list` | **ADOPTED** | PanelListRow | Row bg / selected style |
| `row.list.selected` | PENDING | PanelListRow | Selected state override |
| `row.list.hover` | PENDING | PanelListRow | Hover state override |
| `section.header` | **ADOPTED** | PanelSection | Section header bg |
| `section.header.fill` | PENDING | PanelSection | Fill variant |
| `nav.cluster` | PENDING | TopNav | Nav item default |
| `nav.cluster.active` | PENDING | TopNav | Nav item active |
| `panel.footer` | PENDING | BottomDock | Footer bg |
| `panel.header` | PENDING | Header | Panel header |
| `card` | PENDING | PanelCard | Card fill/radius/shadow |
| `card.floating` | PENDING | PanelCard | Floating card elevation |
| `toast` | PENDING | Toast | Toast base |
| `toast.success` | PENDING | Toast | Success variant |
| `toast.danger` | PENDING | Toast | Danger variant |
| `toast.warn` | PENDING | Toast | Warning variant |
| `tag` | **ADOPTED** | Tag | Tag fill/corner/border |
| `kbd` | PENDING | Kbd | Keycap fill/border |
| `drag.handle` | PENDING | Resizable | Drag handle |
| `toolnav` | PENDING | ToolOverlay | Tool nav strip |

**Adoption count: 4 widgets / 5 resolve() call sites out of 27 registered keys.**

---

## What Crosses Over (Figma → Code)

| Crosses over | Does NOT cross over |
|---|---|
| Color tokens (accent/bull/bear/warn/dim/text/surface/border/bg/hud_*) | Motion / easing curves (defined in `motion.rs`) |
| Radius tokens (xs/sm/md/lg/full/pill/chip) | Chart canvas rendering (colors only) |
| Spacing tokens (xs/sm/xs_mid/md/lg/xl/xxl) | Shadow blur/spread raw values (themed helpers wrap them) |
| Stroke weights (hair/thin/medium/std/bold/thick) | Internal layout math (row heights, thumb sizes, etc.) |
| Font sizes (xs/sm/md/lg/xl — 9/11/13/16/22 px) | |
| Alpha tiers (faint/ghost/soft/subtle/tint/muted/dim/line/strong/active/heavy/scrim/solid) | |
| Component variant props (Figma Variant prop = Rust Variant enum value) | |
| Component size props (Figma Size prop = Rust Size enum value) | |
| Recipe key overrides (per-component Sx style customization) | |

---

## Coverage Report

- **Total widget .rs files found:** ~90
- **Core visual widgets documented in detail:** 55
- **Structural / internal / pipeline files:** ~35
- **Recipe keys registered in recipe-keys.md:** 27
- **Recipe keys actively adopted (resolve() hooked in widget code):** 5 call sites across 4 widgets — `tag`, `tab.line.active`, `tab.pill`, `row.list`, `section.header`
- **Recipe keys PENDING adoption:** 22
- **Widgets with no size tiers (auto-size):** Badge, CountChip, Toast, Alert, Separator, Sparkline, RiskRewardBar, Indicator, Tooltip, HoverCard, Popover
- **API variance (not `show(ui, theme)`):** Modal (builder with ctx+theme), ContextMenu (constructor takes theme, not show)

### Surprises

- `button.rs` is 1533 lines — largest file; contains ButtonStyle trait, all 14 Variant values, and named presets
- `tabs.rs` is 1272 lines — defines 5 TabTreatments including the `ZED_TAB_HEIGHT = 31` constant
- `tag.rs` contains painter-level `paint_pill` and `paint_badge` helpers consumed by other widgets
- `count_chip.rs` has a `paint_at(painter, center)` painter-mode API for HUD overlays
- Kbd font sizes (9/10 px) are documented exceptions to the type scale — intentionally smaller for keycap realism
- `panel.rs` defines only a Rust trait (`Panel`), not a visual widget — has no egui rendering of its own
- Form system is a full sub-library: FormField / FormRow / FormSection / FormActionBar / Fieldset
- `guild_avatar_grid.rs` and `heatmap_grid.rs` exist as specialized visualizations not present in the original partial inventory
- `polished_label.rs` uses cosmic-text for subpixel rendering — applies only when the style preset opts into polished typography
