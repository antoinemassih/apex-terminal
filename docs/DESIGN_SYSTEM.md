# Apex Terminal Design System

> **Snapshot date:** 2026-05-05
> **Migration status:** Post-extraction pass — spacing, font, color, and frame literals migrated. See "Future work" for pending items.

The design system spans three Rust modules:

| File | What lives there |
|------|-----------------|
| `src-tauri/src/design_tokens.rs` | `DesignTokens` struct (all editable token values), sub-structs for each category, `dt_f32!` / `dt_u8!` / `dt_rgba!` macro accessors |
| `src-tauri/src/chart_renderer/ui/style.rs` | Token accessor functions (`gap_*`, `font_*`, `radius_*`, `stroke_*`, `alpha_*`, `status_*`, `drawing_palette()`), free-function frame helpers, legacy component helpers |
| `src-tauri/src/chart_renderer/ui/widgets/` | Builder-style widget primitives — one file per family (`frames.rs`, `buttons.rs`, `inputs.rs`, `headers.rs`, `pills.rs`, `tabs.rs`, `menus.rs`, `rows/`, `modal.rs`, `context_menu.rs`, `select.rs`, …) |

The legacy positional-arg helpers in `components.rs` / `components_extra.rs` still work and delegate to the same paint code. New code should prefer the builder API in `widgets/`.

---

## Rules

1. **Never hardcode a numeric value** (font size, padding, color RGB). Use a token accessor.
2. **Never use `egui::Button::new()` or `egui::TextEdit::singleline()` directly in panel code.** Use a design-system widget.
3. **Never construct colors with `Color32::from_rgb()`** in panel code. Use `t.<field>` from the active theme, or a `status_*()` / `drawing_palette()` token.
4. **When you touch a panel file for any other reason**, replace any raw egui calls you encounter. Don't open migrate-only PRs — migrate as you go. The baseline allow-list in `scripts/.design-system-baseline.txt` tracks legacy call-sites; new violations fail CI.

---

## Anti-patterns → correct replacements

```rust
// ❌ Hardcoded spacing
ui.add_space(6.0);
// ✅
ui.add_space(gap_md());

// ❌ Hardcoded font size
RichText::new(x).size(12.0);
// ✅
RichText::new(x).size(font_md());

// ❌ Hardcoded color
Color32::from_rgb(120, 120, 130)
// ✅
t.dim

// ❌ Raw egui popup frame
egui::Frame::popup(ui.style()).fill(...).stroke(...)
// ✅
PopupFrame::new().theme(t).ctx(ctx).build()

// ❌ Raw text edit in a form
egui::TextEdit::singleline(&mut value)
// ✅
TextInput::new(&mut value).show(ui)
```

---

## Token registry

All numeric design values come from `DesignTokens` in `design_tokens.rs`, accessed via the `dt_f32!`, `dt_u8!`, and `dt_rgba!` macros. In non-design-mode builds the compiler inlines them to their constant defaults.

### Font sizes

| Helper | Default |
|--------|---------|
| `font_xs()` | 8.0 px |
| `font_sm_tight()` | 9.0 px — between xs and sm; watchlist section headers, badge overlays |
| `font_sm()` | 10.0 px |
| `font_md()` | 11.0 px |
| `font_lg()` | 14.0 px |
| `font_xl()` | 15.0 px |
| `font_2xl()` | 15.0 px (maps to `font.xxl`) |

> Note: the old `FONT_*` constants remain for backwards compat but hold slightly different values (they predated the token system). Use the `font_*()` functions in new code.

### Spacing

| Helper | Default |
|--------|---------|
| `gap_xs()` | 2.0 px |
| `gap_sm()` | 4.0 px |
| `gap_md()` | 6.0 px |
| `gap_lg()` | 8.0 px |
| `gap_xl()` | 10.0 px |
| `gap_2xl()` | 12.0 px |
| `gap_3xl()` | 20.0 px |

### Corner radii

| Helper | Default |
|--------|---------|
| `radius_sm()` | 3.0 px |
| `radius_md()` | 4.0 px |
| `radius_lg()` | 8.0 px |

Legacy `r_xs_cr()` / `r_sm_cr()` / `r_md_cr()` / `r_lg_cr()` / `r_pill_cr()` aliases still exist in `style.rs`.

### Stroke widths

| Helper | Default |
|--------|---------|
| `stroke_hair()` | 0.3 px |
| `stroke_thin()` | 0.5 px |
| `stroke_std()` | 1.0 px |
| `stroke_bold()` | 1.5 px |
| `stroke_thick()` | 2.0 px |

### Alpha tiers

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

### Shadow tokens

| Helper | Default |
|--------|---------|
| `shadow_offset()` | 2.0 px |
| `shadow_spread()` | 4.0 px |
| `shadow_alpha()` | 60 |

### Theme colors (from `Theme` struct)

Pull from the active theme `t: &Theme`. Never construct with `Color32::from_rgb()`.

| Field | Meaning |
|-------|---------|
| `t.bg` | Canvas / deep background |
| `t.toolbar_bg` | Panel / popup fill |
| `t.toolbar_border` | Panel / popup border |
| `t.accent` | Brand accent (blue) |
| `t.bull` | Long / up / green |
| `t.bear` | Short / down / red |
| `t.dim` | Muted icon / label color |
| `t.text` | Primary text |
| `t.text_muted` | Third-tier dim text (lighter than `dim`) |
| `t.warn` | Amber / warning / R:R indicator |
| `t.gold` | Star pin, earnings pill, RVOL highlight |
| `t.notification_red` | Notification badge red (distinct from `bear`) |
| `t.shadow_color` | Drop shadow tint |
| `t.overlay_text` | High-contrast price labels (DOM), inverted |
| `t.pinned_row_tint` | Watchlist pinned-row tint |
| `t.hud_bg` / `t.hud_border` | Debug HUD overlay |
| `t.rrg_leading/improving/weakening/lagging` | RRG quadrant colors |
| `t.cmd_palette[0..11]` | Command-palette category badge colors |

### Status color tokens

```rust
status_ok()     // Green  — active / live / filled
status_warn()   // Orange — warning / pending
status_error()  // Red    — error / rejected
status_info()   // Blue   — informational
```

Backed by `StatusTokens { ok, warn, error, info }` in `DesignTokens`.

### Drawing palette

```rust
let colors: [Color32; 4] = drawing_palette();
// [blue, green, orange, purple] — link-group identity colors
```

Backed by `DrawingTokens { palette: [Rgba; 4] }` in `DesignTokens`.

---

## Theme presets

Themes are `Theme` structs (defined in `chart_renderer/gpu.rs`). Style presets are named separately:

| Preset name | Notes |
|-------------|-------|
| Meridien | Default dark; `UnderlineActive` button treatment |
| Aperture | High-contrast dark |
| Octave, Cadence, Chord, Lattice, Tangent, Tempo, Contour, Relay | Additional presets |

Switch via `UiStyle::set_preset(preset)`.

---

## Widget builder reference

Prefer `widgets/` builders over the legacy free-functions in `components.rs`. The builders chain `.theme(t)` and emit the same paint.

### Frames (`widgets/frames.rs`)

**`PopupFrame`** — context menus, dropdowns, any floating popup.
```rust
PopupFrame::new()
    .theme(t)          // pull bg + border from theme
    .ctx(ctx)          // required — passed to egui::Frame::popup
    .inner_margin(m)   // override margin (e.g. egui::Margin::ZERO)
    .no_inner_margin() // convenience: zero on all sides
    .corner_radius(r)  // override corner radius
    .border_alpha(BorderAlpha::Line)  // or ::Strong (default)
    .build()           // → egui::Frame
```

`BorderAlpha::Line` = `alpha_line()` (50) — for context menus / submenus.
`BorderAlpha::Strong` = `alpha_strong()` (80) — default for most popups.

**`DialogFrame`** — modal dialogs. Requires `.ctx(ctx)`.
```rust
DialogFrame::new().theme(t).ctx(ctx).build()
```

**`PanelFrame`** — side panels with standard margins (8/8/8/6 px).
```rust
PanelFrame::new(t.toolbar_bg, t.toolbar_border).build()
// or
PanelFrame::new(Color32::TRANSPARENT, Color32::TRANSPARENT).theme(t).build()
```

**`CompactPanelFrame`** — tighter margins for scanner, tape, etc.
```rust
CompactPanelFrame::new(t.toolbar_bg, t.toolbar_border).build()
```

**`CardFrame`** — standard framed card. Supports shadows.
```rust
CardFrame::new().theme(t).build().show(ui, |ui| { ... })
```

**`SidePanelFrame`** — flat side panel, zero inner margin.
```rust
SidePanelFrame::new().theme(t).ctx(ctx).build()
```

**`TooltipFrame`** — tooltip popup frame, corner radius from token.
```rust
TooltipFrame::new().theme(t).build()
```

**`DialogSeparator`** — inset horizontal rule between dialog sections.
```rust
DialogSeparator::new(t.toolbar_border).indent(8.0).show(ui);
```

---

### Buttons (`widgets/buttons.rs`)

**`IconBtn`** — icon-only ghost button (close, toolbar icons).
```rust
ui.add(IconBtn::new("✕").theme(t))
ui.add(IconBtn::new("⚙").color(t.accent).medium())
// sizes: .small() = 11px, .medium() = 14px (default), .large() = 18px
```

**`TradeBtn`** — BUY/SELL/Place Order action button.
```rust
ui.add(TradeBtn::new("BUY").color(t.bull).width(80.0))
ui.add(TradeBtn::new("SELL").color(t.bear).width(80.0).height(30.0))
```

**`SimpleBtn`** — low-emphasis form button (Cancel, Save, etc.).
```rust
ui.add(SimpleBtn::new("Cancel").color(t.dim).min_width(60.0))
```

**`SmallActionBtn`** — side-pane row actions (Refresh, Send, etc.).
```rust
ui.add(SmallActionBtn::new("Refresh").theme(t))
```

**`ChromeBtn`** — window chrome (minimize/maximize/close). Internal use.

**`ActionBtn`** — full-width primary/secondary action. For dialogs.

---

### Inputs (`widgets/inputs.rs`)

**`TextInput`** — single-line (or multiline) themed text edit.
```rust
let resp = TextInput::new(&mut buf)
    .placeholder("Search…")
    .width(200.0)
    .font_size(font_sm())
    .theme(t)
    .horizontal_align(egui::Align::Center)  // new in extraction pass
    .show(ui);
```

Additional knobs: `.text_color(c)`, `.background_color(c)`, `.id(id)`, `.margin(m)`, `.frameless(true)`, `.proportional(true)`, `.multiline(true)`, `.put_at(rect)`.

---

### Headers (`widgets/headers.rs`)

All headers accept `.theme(t)` or explicit `.accent(c).dim(c)`.

| Builder | Usage |
|---------|-------|
| `PanelHeader::new("Title")` | Panel title bar, no close button. `ui.add(...)` |
| `PanelHeaderWithClose::new("Title")` | Panel header with close. `.show(ui)` → `bool` (true = close clicked) |
| `DialogHeader::new("Title")` | Modal dialog header |
| `DialogHeaderWithClose::new("Title")` | Modal dialog header with close button |
| `PaneHeader::new("Title")` | Chart pane header bar |
| `PaneHeaderWithClose::new("Title")` | Chart pane header with close |

---

### Pills, Chips & Badges (`widgets/pills.rs`)

| Builder | Usage |
|---------|-------|
| `PillButton::new("Label")` | Toggleable filter pill. `.selected(bool).theme(t)` |
| `DisplayChip::new("DRAFT")` | Fixed status chip (read-only). `.color(c)` |
| `StatusBadge::new("ACTIVE")` | Filled status pill. `.color(status_ok())` |
| `RemovableChip::new("SPY")` | Chip with ✕ — used in watchlist multi-select |
| `KeybindChip::new("Ctrl+K")` | Keyboard shortcut hint chip |
| `BrandCtaButton::new("Upgrade")` | Promotional CTA button |

---

### Tabs (`widgets/tabs.rs`)

| Builder | Usage |
|---------|-------|
| `TabBar<T>` | Full tab bar widget with `Vec<(T, &str)>` tabs and selected value |
| `TabStrip` | Lower-level tab strip (renders each tab button inline) |
| `TabBarWithClose` | Tab bar where each tab has a close button |

---

### Menus (`widgets/menus.rs`)

| Builder | Usage |
|---------|-------|
| `MenuTrigger::new("File")` | Top nav button that opens a dropdown |
| `MenuItem::new("Open")` | Row inside a dropdown |
| `SidePaneAction::new("Edit")` | Mid-prominence side-pane action row |

---

### Context menus & submenus (`widgets/context_menu.rs`)

```rust
ContextMenu::new()  // top-level context menu container
MenuBuilder          // lower-level builder
MenuItem             // standard row
MenuItemWithShortcut // row + keyboard hint
MenuItemWithIcon     // row + leading icon
CheckMenuItem        // row + checkbox
RadioMenuItem<T>     // row + radio (mutually exclusive group)
Submenu<F>           // nested flyout
DangerMenuItem       // destructive-action row (red)
MenuSection          // labeled section within a menu
MenuDivider          // horizontal separator
```

---

### Select / Combobox (`widgets/select.rs`)

| Builder | Usage |
|---------|-------|
| `Dropdown<T>` | Single-select dropdown |
| `DropdownOwned<T>` | Single-select, owned items (no static lifetime) |
| `Combobox<T>` | Editable combobox (type-to-filter) |
| `MultiSelect<T>` | Multi-select with chips |
| `Autocomplete` | Text input with suggestion list |
| `SegmentedControl<T>` | Mutually-exclusive horizontal button group |
| `RadioGroup<T>` | Vertical radio button list |
| `DropdownActions` | Dropdown of action items (no bound value) |

---

### Modal (`widgets/modal.rs`)

```rust
Modal::new(id)
    .show(ctx, |ui, resp| {
        // resp: &ModalResponse — has .close field
    });
```

---

### List rows (`widgets/rows/`)

**`ListRow`** — generic selectable / hoverable row primitive. Wrap content in the body closure; optionally provide a trailing closure. Adoption is ongoing — use it for new panels, but expect the API to evolve.

---

### Other widget families

| Module | Contents |
|--------|----------|
| `widgets/text.rs` | Typed text widgets (body label, muted label, caption, monospace code, numeric display) |
| `widgets/semantic_label.rs` | `SemanticLabel` — auto-styles based on semantic role |
| `widgets/status.rs` | Status indicator widgets |
| `widgets/layout.rs` | Layout helpers (gap widgets, dividers) |
| `widgets/form.rs` | `FormRow` — label + input aligned in a two-column grid |
| `widgets/pane.rs` | Pane container helpers |
| `widgets/toolbar/` | Toolbar-specific widget primitives |
| `widgets/trading/` | Trading-specific widgets (order entry, DOM) |
| `widgets/watchlist/` | Watchlist-specific row and strip widgets |
| `widgets/drawing/` | Drawing-tool UI widgets |
| `widgets/foundation/` | Low-level primitives (`InputShell`, `InputState`, `InputVariant`, sizes) |
| `widgets/icons.rs` | Icon glyph constants |
| `widgets/cards/` | Card container variants |
| `widgets/perf_hud.rs` | Performance HUD overlay |
| `widgets/painter_pane.rs` | Chart-pane painter helpers |
| `widgets/design_mode_panel.rs` | Design-mode inspector panel (internal) |

---

## Legacy component taxonomy (components.rs / components_extra.rs)

These free functions still work. They are the migration source — builders in `widgets/` replace them incrementally. Prefer the builder equivalents for new code.

### Buttons (legacy)
`icon_btn`, `small_action_btn`, `top_nav_btn`, `top_nav_toggle`, `big_action_btn`, `side_pane_action_btn`, `pane_tab_btn`, `timeframe_selector`, `menu_trigger`, `menu_item`, `pill_button`, `tab_strip`, `segmented_control`, `numeric_stepper`, `compact_stepper`, `status_badge`

### Pills, Chips & Badges (legacy)
`status_pill`, `status_badge`, `keybind_chip`, `notification_badge`, `colored_direction_badge`

### Text (legacy)
`pane_title`, `section_label_widget`, `subheader`, `body_label`, `muted_label`, `caption_label`, `monospace_code`, `numeric_display`

### Inputs (legacy)
`text_input_field`, `numeric_input_field`, `search_input`, `toggle_row`, `toggle_switch`, `radio_button_row`

### Containers (legacy)
`pane_header_bar`, `card_frame`, `order_card`, `dialog_frame`, `panel_frame`, `toast_card`, `empty_state_panel`, `metric_value_with_label`, `metric_grid_row`, `themed_popup_frame`

### Dividers & Spacing (legacy)
`hairline`, `v_hairline` — use `gap_xs()` / `gap_sm()` / … for spacing

---

## Project file map

```
src-tauri/src/
  design_tokens.rs                — DesignTokens struct + all sub-structs; dt_f32!/dt_u8!/dt_rgba! macros
  chart_renderer/
    gpu.rs                        — Theme struct (semantic color fields per-theme)
    ui/
      style.rs                    — Token accessor functions (gap_*, font_*, radius_*, stroke_*,
                                    alpha_*, shadow_*, status_*, drawing_palette()),
                                    free-function frame + component helpers
      components/                 — Legacy positional-arg component helpers
      components_extra/           — Legacy extra component helpers
      widgets/
        mod.rs                    — Module registry
        frames.rs                 — PopupFrame, DialogFrame, PanelFrame, CardFrame,
                                    SidePanelFrame, CompactPanelFrame, TooltipFrame,
                                    DialogSeparator; BorderAlpha enum
        buttons.rs                — IconBtn, TradeBtn, SimpleBtn, SmallActionBtn,
                                    ChromeBtn, ActionBtn
        inputs.rs                 — TextInput (+ horizontal_align knob)
        headers.rs                — PanelHeader, PanelHeaderWithClose, DialogHeader,
                                    DialogHeaderWithClose, PaneHeader, PaneHeaderWithClose
        pills.rs                  — PillButton, DisplayChip, StatusBadge, RemovableChip,
                                    KeybindChip, BrandCtaButton
        tabs.rs                   — TabBar, TabStrip, TabBarWithClose
        menus.rs                  — MenuTrigger, MenuItem, SidePaneAction
        context_menu.rs           — ContextMenu, MenuBuilder, MenuItem, MenuItemWithShortcut,
                                    MenuItemWithIcon, CheckMenuItem, RadioMenuItem, Submenu,
                                    DangerMenuItem, MenuSection, MenuDivider
        modal.rs                  — Modal, ModalResponse
        select.rs                 — Dropdown, DropdownOwned, Combobox, MultiSelect,
                                    Autocomplete, SegmentedControl, RadioGroup, DropdownActions
        rows/                     — ListRow (generic selectable row)
        text.rs                   — Typed text widgets
        semantic_label.rs         — SemanticLabel
        status.rs                 — Status indicators
        layout.rs                 — Layout helpers
        form.rs                   — FormRow
        foundation/               — InputShell, InputState, InputVariant, Size, Radius
        cards/                    — Card container variants
        toolbar/                  — Toolbar primitives
        trading/                  — Order entry, DOM widgets
        watchlist/                — Watchlist row/strip widgets
        drawing/                  — Drawing-tool UI widgets
        icons.rs                  — Icon glyph constants
        pane.rs                   — Pane container helpers
        painter_pane.rs           — Chart-pane painter helpers
        perf_hud.rs               — Performance HUD overlay
        design_mode_panel.rs      — Design inspector panel (internal)
```

---

## How to add a new component

1. Pick the right family file in `widgets/` (or create a new one and register it in `mod.rs`).
2. Implement the builder struct with `pub fn new(...)` and chained setters.
3. `impl Widget for Foo` (or provide a `show(self, ui)` if multi-value return is needed).
4. Use ONLY token helpers — no hardcoded values.
5. Add it to the relevant table in this doc.

---

## Future work (not yet migrated)

| Item | Description |
|------|-------------|
| Chart-canvas pixel offsets | Sub-pixel offsets inside the GPU painter — out of UI chrome scope |
| `FormLayoutTokens` | Proposal to tokenize label widths and row heights in two-column forms |
| Numeric formatter consolidation | Multiple ad-hoc `format!("{:.2}", x)` / `format_price()` paths — needs a single formatter API |
| Color brightening/darkening helpers | `gamma_multiply` / manual RGB scaling for hover/active — needs `brighten(c, t)` / `darken(c, t)` helpers |
| Component height/min-size tokens | Button and row heights currently come from `UiStyle` struct fields, not named tokens |
| Input field width tokens | Widths passed as literals to `.width(...)` — needs semantic named widths (e.g. `input_width_sm()`) |
