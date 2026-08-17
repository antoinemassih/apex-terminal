# Apex Terminal Design System

> **Snapshot date:** 2026-08-17
> **Migration status:** Tokens, recipes and layout migrated; the declarative
> cascading layer is in adoption. See "Future work" for pending items.

## The layers

Five, from values upward. Each is usable on its own; each is built on the one
below it. Nothing here is a parallel system — there is exactly one of each,
and the gates in `dev/` exist to keep it that way.

| Layer | Where | What it answers |
|-------|-------|-----------------|
| **Tokens** | `foundation/design_tokens.rs`, `chart/renderer/ui/style.rs` | "How big is a gap? What is the dim tone?" — `gap_md()`, `font_sm()`, `alpha_muted()`, `dt_f32!` |
| **Recipes** | `ui_kit/sx/recipe_spec.rs`, `design_system/builtin_recipes.rs` | "What does a *card* look like in this style?" — the CSS-class layer, keyed by role (`row.list`, `section.header`) |
| **Layout** | `ui_kit/layout/{flex,grid}.rs` | "Where do these go?" — Taffy-backed flexbox and grid, solved headlessly |
| **Cascade** | `ui_kit/cascade/context.rs` | "What colour is this text if nobody said?" — CSS's *inheritable* properties flowing parent → child |
| **Elements** | `ui_kit/cascade/element.rs` | "What IS this row?" — a declarative tree that solves through Layout and paints through egui |

Above them sit the **widgets** (`ui_kit/widgets/`), builder-style components —
`Button`, `Label`, `SelectableRow`, `KvRow` — each of which uses the layers
rather than reimplementing them.

---

## Authoring model: think in CSS and components

This is an **organizing** layer over egui, not a different renderer. There is
no virtual DOM, no diffing, no reconciler, no component lifecycle. A tree is
built and consumed inside one frame, exactly as immediate mode already works,
and compiles down to the same `painter.text` / `painter.rect_filled` calls the
app made before. What is borrowed from CSS and React is the *authoring* model —
declaration instead of sequence, inheritance instead of a global lookup.

### Declare the shape; do not walk a cursor

```rust
// ❌ A sequence of mutations. The gap after the icon lives inside an `if`;
//    move a line and the row silently changes.
let mut x = rect.left() + pad;
if let Some(ic) = icon {
    painter.text(pos2(x, cy), LEFT_CENTER, ic, icon_font, icon_col);
    x += icon_w + icon_gap;
}
painter.galley(pos2(x, cy - h * 0.5), label, text_col);

// ✅ A shape. The gap is a property of being siblings, so a row with no icon
//    cannot pay for a seam it does not have.
El::row()
    .pad_x(pad)
    .gap(icon_gap)
    .color(text_col)                                   // cascades to both
    .child_if(icon.is_some(), El::text_with_font(icon.unwrap_or(""), icon_font).color(icon_col))
    .child(El::text_with_font(label, label_font).grow(1.0))
    .show_in(ui, theme, rect);
```

`dev/cascade_adoption_gate.py` holds the ceiling at **zero** cursor walks in UI
and chrome. A new `x += w + gap` there fails CI.

### Inheritance is CSS's inheritance, exactly

`color`, `font-*`, `letter-spacing` and `text-align` flow down. `padding`,
`margin`, `border`, `width` and `background` do **not** — they belong to the
element that declares them. That split is mirrored rather than invented,
because the whole point is that a reader coming from CSS can predict it.

```rust
cascade::scope(Inherited::default().color(t.dim), || {
    // every unstyled text node below here is dim
});
```

**Absent means "the widget's own default", not "a global default".** With no
scope open, nothing changes — which is what makes adoption safe, and also what
makes abandonment invisible, which is why there are adoption *floors* as well
as ceilings. An explicit `.color()` on a call site always outranks an ancestor.

### Three entry points, by what the surface has

| You have | Call | The tree |
|----------|------|----------|
| `&mut Ui` | `show(ui, theme)` / `show_in(ui, theme, rect)` | solves **and paints**; buttons interact |
| `&Painter` only | `show_with(painter, theme, rect)` | solves **and paints**; buttons are unavailable (they need `interact`) |
| Either, mid-migration | `solve_in(ui, rect)` / `solve_rect(rect)` | solves only — hands back rects you paint yourself |

`solve_*` is the migration path: a surface with hard-won painting (fades, clip
invariants, morph animations) moves its *layout* first and its painting later,
or never. `El::slot("id", size)` reserves a rect for exactly that.

Prefer `El::text` with a `TextStyle` tier for anything new. `El::text_with_font`
takes an explicit `FontId` and exists for pixel-locked chrome that cannot be
re-tiered as a side effect of moving its layout — CSS has the same escape,
where `font-family` is a value and not only a class.

### Measure before you place

```rust
let w = row.intrinsic_width(ui);   // "does this fit" has an answer up front
```

Every surface that packs variable-width items — the ticker strip, the tab
strip — asks this instead of painting and discovering the overflow. Do not
re-derive it by solving into an infinite rect and reading a probe slot back.

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

Themes are `Theme` structs (defined in `chart/renderer/gpu.rs`). Style presets are named separately:

| Preset name | Notes |
|-------------|-------|
| Meridien | Default dark; `UnderlineActive` button treatment |
| Aperture | High-contrast dark |
| Octave, Cadence, Chord, Lattice, Tangent, Tempo, Contour, Relay | Additional presets |

Switch via `UiStyle::set_preset(preset)`.

---

## Widget builder reference

Prefer `widgets/` builders over the legacy free-functions in `components.rs`. The builders chain `.theme(t)` and emit the same paint.

### Frames (`ui_kit/widgets/frames.rs`)

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

### Buttons (`ui_kit/widgets/button.rs`)

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

### Inputs (`ui_kit/widgets/input.rs`)

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

### Headers (`ui_kit/widgets/header.rs`)

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

### Pills, Chips & Badges (`ui_kit/widgets/status_pill.rs`)

| Builder | Usage |
|---------|-------|
| `PillButton::new("Label")` | Toggleable filter pill. `.selected(bool).theme(t)` |
| `DisplayChip::new("DRAFT")` | Fixed status chip (read-only). `.color(c)` |
| `StatusBadge::new("ACTIVE")` | Filled status pill. `.color(status_ok())` |
| `RemovableChip::new("SPY")` | Chip with ✕ — used in watchlist multi-select |
| `KeybindChip::new("Ctrl+K")` | Keyboard shortcut hint chip |
| `BrandCtaButton::new("Upgrade")` | Promotional CTA button |

---

### Tabs (`ui_kit/widgets/tabs.rs`)

| Builder | Usage |
|---------|-------|
| `TabBar<T>` | Full tab bar widget with `Vec<(T, &str)>` tabs and selected value |
| `TabStrip` | Lower-level tab strip (renders each tab button inline) |
| `TabBarWithClose` | Tab bar where each tab has a close button |

---

### Menus (`ui_kit/widgets/menu_item.rs`)

| Builder | Usage |
|---------|-------|
| `MenuTrigger::new("File")` | Top nav button that opens a dropdown |
| `MenuItem::new("Open")` | Row inside a dropdown |
| `SidePaneAction::new("Edit")` | Mid-prominence side-pane action row |

---

### Context menus & submenus (`ui_kit/widgets/context_menu.rs`)

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

### Select / Combobox (`ui_kit/widgets/select.rs`)

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

### Modal (`ui_kit/widgets/modal.rs`)

```rust
Modal::new(id)
    .show(ctx, |ui, resp| {
        // resp: &ModalResponse — has .close field
    });
```

---

### List rows (`chart/renderer/ui/widgets/rows/`)

**`ListRow`** — generic selectable / hoverable row primitive. Wrap content in the body closure; optionally provide a trailing closure. Adoption is ongoing — use it for new panels, but expect the API to evolve.

---

### Other widget families

| Module | Contents |
|--------|----------|
| `ui_kit/widgets/label.rs` | Typed text widgets (body label, muted label, caption, monospace code, numeric display) |
| `ui_kit/widgets/polished_label.rs` | `SemanticLabel` — auto-styles based on semantic role |
| `ui_kit/widgets/indicator.rs` | Status indicator widgets |
| `ui_kit/layout/flex.rs` | Layout helpers (gap widgets, dividers) |
| `ui_kit/widgets/form_row.rs` | `FormRow` — label + input aligned in a two-column grid |
| `ui_kit/widgets/pane_grid.rs` | Pane container helpers |
| `chart/renderer/ui/components/toolbar/` | Toolbar-specific widget primitives |
| `chart/renderer/ui/widgets/trading/` | Trading-specific widgets (order entry, DOM) |
| `chart/renderer/ui/widgets/watchlist/` | Watchlist-specific row and strip widgets |
| `chart/renderer/ui/tools/drawing/` | Drawing-tool UI widgets |
| `chart/renderer/ui/foundation/` | Low-level primitives (`InputShell`, `InputState`, `InputVariant`, sizes) |
| `ui_kit/icons.rs` | Icon glyph constants |
| `chart/renderer/ui/widgets/cards/` | Card container variants |
| `ui_kit/widgets/` (removed — the perf HUD now lives in the design inspector) | Performance HUD overlay |
| `chart/renderer/ui/chrome/painter_pane.rs` | Chart-pane painter helpers |
| `foundation/design_inspector.rs` | Design-mode inspector panel (internal) |

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

Regenerated 2026-08-17. `dev/doc_accuracy_gate.py` checks every path below
still exists — the previous version of this section pointed at
the old chart_renderer widgets directory long after it became
`ui_kit/widgets/`, and nothing said so.

```
src-tauri/src/
  foundation/design_tokens.rs     — DesignTokens struct + sub-structs; dt_f32!/dt_u8!/dt_rgba!
  design_system/
    style_system.rs               — StyleSystem (the 9 styles): the shape/feel axis
    color_scheme.rs               — ColorScheme (the 22 palettes): the colour axis
    builtin.rs                    — Built-in style definitions (edit HERE to restyle)
    builtin_recipes.rs            — Authored RecipeSets per built-in style
    recipes.rs                    — Recipe resolution
    presets.rs                    — Named preset combinations
    loader.rs                     — Theme-pack loading
    hot_reload.rs                 — Live token reload (design-mode)
    export.rs                     — Token export
    baseline.rs                   — Migration baselines
    equivalence_tests.rs          — Legacy-vs-new equivalence guards
  ui_kit/
    cascade/
      mod.rs                      — The declarative cascading layer (see "Authoring model")
      context.rs                  — Inherited: CSS's inheritable properties, parent -> child
      element.rs                  — El: the declarative element tree; solve/paint entry points
    layout/
      flex.rs                     — Taffy-backed flexbox: Flex, Item, Size, FlexSlots
      grid.rs                     — Taffy-backed grid: Grid, GridItem, Track
    sx/
      recipe_spec.rs              — RecipeSpec/RecipeSet/ColorSpec — the CSS-class layer
      color.rs                    — palette_ct(), Tone
      style.rs                    — sx style helpers
    widgets/                      — 70+ builder-style components, one family per file
    style.rs                      — Token accessors (gap_*, font_*, radius_*, stroke_*, alpha_*)
    tokens.rs                     — Token helper re-exports
    text_style.rs                 — TextStyle tiers (the type ladder)
    scale.rs                      — Typed design scales (Space/Radius/Weight/Level)
    interaction.rs                — apply_interaction: the ONE hover/selected/disabled table
    icons.rs                      — Icon glyph constants
    inspect.rs                    — Widget inspection hooks (design-mode)
  chart/renderer/
    gpu.rs                        — Theme struct (semantic colour fields per theme)
    theme_impl.rs                 — impl ComponentTheme for Theme (the chart app's bridge)
    ui/
      style.rs                    — Chart-side token accessors + StyleSettings
      chart_widgets.rs            — Chart overlay panels (painter-only)
      chrome/                     — Pane chrome
      panels/                     — Side panels
      components/                 — Toolbar and component families
      lists/                      — Row families (DOM, watchlist, option chain, orders)
      foundation/                 — Interaction/text-style foundations
```

Selected widget families in `ui_kit/widgets/` (76 files; see `mod.rs` for the
full registry):

| File | Provides |
|------|----------|
| `ui_kit/widgets/button.rs` | `Button` — the one button; `show_at` for painter surfaces |
| `ui_kit/widgets/label.rs` | `Label` — text with tiers, cascade-aware colour |
| `ui_kit/widgets/kv_row.rs` | `KvRow` — label/value row for painter-only surfaces |
| `ui_kit/widgets/selectable_row.rs` | `SelectableRow` — clickable menu/dropdown row |
| `ui_kit/widgets/panel_key_value_row.rs` | `PanelKeyValueRow` — the `Ui`-based label/value row |
| `ui_kit/widgets/panel_section.rs` | `PanelSection` — collapsible titled section |
| `ui_kit/widgets/select.rs` | `Select` — the one dropdown |
| `ui_kit/widgets/modal.rs` | `Modal` — the one modal |
| `ui_kit/widgets/theme.rs` | `ComponentTheme` trait + `PortableTheme` |

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
