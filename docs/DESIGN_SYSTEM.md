# Apex Terminal Design System

The design system currently lives in `src-tauri/src/chart_renderer/ui/style.rs`.
All reusable components, token helpers, and theme presets are defined there.
> Audit snapshot (2026-04-28): 34 reusable helpers, 883 raw `Button::new` call-sites,
> 121 panel files. Migration is ongoing — see the baseline allow-list in
> `scripts/.design-system-baseline.txt`.

---

## Module map

| File | What lives there |
|------|-----------------|
| `src-tauri/src/chart_renderer/ui/style.rs` | Token helpers (`font_*`, `gap_*`, `r_*_cr`, `alpha_*`, `stroke_*`), all reusable component functions, and theme presets (Relay / Aperture / Meridien) |
| `src-tauri/src/design_inspector.rs` | Design-mode inspector overlay — the one legitimate place that inspects raw egui internals |
| `apex-terminal-designmode/` | Standalone design-mode tool — also allowed to use raw egui primitives |

Token values are defined as runtime-editable fields in a `DesignTokens` struct and accessed
via `dt_f32!` / `dt_u8!` macros. The `current()` helper returns the active `UiStyle` preset.

---

## Rules

**Do NOT use raw `egui::Button::new()` or `egui::TextEdit::singleline()` directly in panel
code.** Use a design-system component, or add one to `style.rs` if nothing fits.

**Do NOT hardcode `Color32::from_rgb(...)`, font sizes, padding, or corner radii in panel
code.** Use token helpers from `style.rs`.

**When you touch a panel file for any other reason**, replace any raw `Button::new` calls you
encounter with the appropriate component below. Do not open migrate-only PRs — migrate as you go.

---

## Component taxonomy

### Buttons

- `IconButton` (`icon_btn`) — icon-only, ghost. Use for: small toolbar actions, X close buttons.
- `SmallActionButton` (`small_action_btn`) — text label, minimal frame. Use for: side-pane row actions.
- `TopNavButton` (`top_nav_btn`) — top app bar menu items (File / Edit / View). Variants: Raised, Underline, SoftPill.
- `TopNavToggle` (`top_nav_toggle`) — top bar icon toggles (magnet, snap, lock). Sizes: Small, Medium.
- `BigActionButton` (`big_action_btn`) — Buy / Sell / Place Order / Flatten. Tiers: Primary, Destructive, Secondary. Sizes: Small, Medium, Large.
- `SidePaneActionButton` (`side_pane_action_btn`) — mid-prominence actions in side panes (Refresh, Save, Send).
- `PaneTabButton` (`pane_tab_btn`) — top-of-pane tabs (Order / Compare / DOM / Options / Indicators). Styles: Underline, Filled, Border.
- `TimeframeSelector` (`timeframe_selector`) — `1m / 5m / 15m / 1h / 1d / 1wk` selector at top of chart panes.
- `MenuTrigger` (`menu_trigger`) — opens a dropdown.
- `MenuItem` (`menu_item`) — row inside a dropdown. Variants: Default, Submenu, Checkbox, Separator.
- `PillButton` (`pill_button`) — toggleable pill. Replaces deprecated `pill_btn` and `filter_chip`.
- `TabStrip` (`tab_strip`) — horizontal tab bar. Tabs imply "current view"; use `SegmentedControl` for exclusive options.
- `SegmentedControl` (`segmented_control`) — multi-button mutually-exclusive selector.
- `Stepper` (`numeric_stepper`, `compact_stepper`) — `[-] value [+]` increment/decrement.
- `StatusBadgeButton` (`status_badge`) — fixed status chip (DRAFT, ACTIVE, FILLED).

### Pills, Chips & Badges

- `StatusPill` (`status_pill`) — fixed status indicator.
- `StatusBadge` (`status_badge`) — filled pill for status labels.
- `KeybindChip` (`keybind_chip`) — keyboard-shortcut hint.
- `NotificationBadge` (`notification_badge`) — count indicator (e.g., "99+").
- `DirectionBadge` (`colored_direction_badge`) — ▲/▼ + price, bull/bear colored.

### Text

- `PaneTitle` (`pane_title`) — pane heading.
- `SectionHeader` (`section_label_widget`) — subdivision within a pane.
- `Subheader` (`subheader`) — smaller heading.
- `BodyLabel` (`body_label`) — default UI text.
- `MutedLabel` (`muted_label`) — dim secondary text.
- `Caption` (`caption_label`) — micro-text (timestamps, helpers).
- `MonospaceCode` (`monospace_code`) — tickers, prices in monospace.
- `NumericDisplay` (`numeric_display`) — large price / P&L display.

### Inputs

- `TextInput` (`text_input_field`) — themed text edit.
- `NumericInput` (`numeric_input_field`) — text edit that parses to f32.
- `SearchInput` (`search_input`) — text edit with magnifier icon.
- `ToggleRow` (`toggle_row`) — label + checkbox inline.
- `ToggleSwitch` (`toggle_switch`) — modern slide toggle.
- `RadioButtonRow` (`radio_button_row`) — mutually-exclusive group row.
- `Checkbox` — use `egui::Checkbox` directly (no wrapper needed).

### Containers

- `PaneHeaderBar` (`pane_header_bar`) — pane top bar.
- `Card` (`card_frame`) — standard framed group.
- `OrderCard` (`order_card`) — left accent stripe + frame.
- `DialogFrame` (`dialog_frame`) — modal dialog.
- `PanelFrame` (`panel_frame`) — side panel container.
- `ToastCard` (`toast_card`) — toast notification.
- `EmptyState` (`empty_state_panel`) — placeholder for "No data".
- `MetricCard` (`metric_value_with_label`) — label + large value.
- `MetricGridRow` (`metric_grid_row`) — multi-column metric row.
- `ThemedPopupFrame` (`themed_popup_frame`) — popup window frame.

### Dividers & Spacing

- `Hairline` (`hairline`) — horizontal rule.
- `VHairline` (`v_hairline`) — vertical divider.
- `Spacing` (`gap_xs` / `gap_sm` / `gap_md` / `gap_lg` / `gap_xl` / `gap_2xl`) — gap helpers.

---

## Token registry

All numeric design values come from the `DesignTokens` struct in `style.rs`,
accessed via the `dt_f32!` and `dt_u8!` macros. Never hardcode these values.

### Fonts

| Helper | Value |
|--------|-------|
| `font_xs()` | 8 px |
| `font_sm()` | 10 px |
| `font_md()` | 11 px |
| `font_lg()` | 13 px |
| `font_xl()` | 14 px |
| `font_2xl()` | 15 px |

### Spacing

| Helper | Value |
|--------|-------|
| `gap_xs()` | 2 px |
| `gap_sm()` | 4 px |
| `gap_md()` | 6 px |
| `gap_lg()` | 8 px |
| `gap_xl()` | 10 px |
| `gap_2xl()` | 12 px |
| `gap_3xl()` | 20 px |

### Corner radii

| Helper | Value |
|--------|-------|
| `r_xs_cr()` | 2 px |
| `r_sm_cr()` | 3 px |
| `r_md_cr()` | 4 px |
| `r_lg_cr()` | 8 px |
| `r_pill_cr()` | pill (large) |

### Strokes

| Helper | Value |
|--------|-------|
| `stroke_hair()` | 0.3 px |
| `stroke_thin()` | 0.5 px |
| `stroke_std()` | 1.0 px |
| `stroke_bold()` | 1.5 px |
| `stroke_thick()` | 2.0 px |

### Alphas

| Helper | Value |
|--------|-------|
| `alpha_faint()` | 10 |
| `alpha_ghost()` | 15 |
| `alpha_soft()` | 20 |
| `alpha_subtle()` | 25 |
| `alpha_tint()` | 30 |
| `alpha_muted()` | 40 |
| `alpha_line()` | 50 |
| `alpha_dim()` | 60 |
| `alpha_strong()` | 80 |
| `alpha_active()` | 100 |
| `alpha_heavy()` | 120 |

### Semantic colors

Pull from `current()` (the active `UiStyle` preset). Key fields:
`accent`, `bull`, `bear`, `dim`, `text_primary`, `text_secondary`,
`toolbar_bg`, `deep_bg`, and more. Never construct colors with `Color32::from_rgb()`.

---

## Theme presets

Three themes ship in `style.rs`:

| Name | Description |
|------|-------------|
| `Relay` | Default dark theme |
| `Aperture` | High-contrast dark |
| `Meridien` | Slightly warmer dark |

Switch via `UiStyle::set_preset(preset)`.

---

## How to add a new component

1. Decide which family it belongs to (Buttons, Inputs, Containers, …).
2. Add the function to `style.rs`.
3. Use ONLY token helpers — no hardcoded values.
4. Update the Component taxonomy table in this doc.
5. The lint guard will enforce rule 2 automatically.

## How to migrate a one-off button

When you touch a panel file for any reason, replace any raw `egui::Button::new(...)` calls
with the appropriate component from the taxonomy above. Don't open migrate-only PRs — too
noisy. The baseline allow-list in `scripts/.design-system-baseline.txt` tracks the 525+
legacy call-sites that predate this doc; new violations fail CI immediately.
