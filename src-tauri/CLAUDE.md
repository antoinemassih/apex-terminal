# UI / Design System — agent guide

> Read this **before** touching any UI code. The full audit lives in `docs/UI_AUDIT.md`.

## 🔒 Sacred code: `src/chart/renderer/render/pane/core.rs`

This file contains the GPU-optimized chart paint pipeline — hottest
code path in the app. **Do NOT touch it as part of any design-system
sweep, token migration, button consolidation, or "for cleanliness"
refactor.** Function-call overhead, lost inlining, and parameter
passing can manifest as measurable frame drops.

Rules:
- No mechanical sweeps inside `core.rs`. Literals stay until a
  performance-conscious owner replaces them with benchmark cover.
- No extraction of helpers "for organization." If you think you need
  one, write a doc explaining why and benchmark it before merging.
- One owner at a time. Multi-agent fanout does NOT cover this file.
- Visual tweaks (color/spacing/layout the user requests) are fine, but
  land them with a single owner who can verify in the running app.

See `docs/PANE_RS_SPLIT_PLAN.md` for context. The plan is deferred
indefinitely — the perf risk outweighs the organizational wins.

## Hard rules

### 1. Never hardcode `&THEMES[0]`

```rust
// ❌ Don't
let theme = &crate::chart_renderer::gpu::THEMES[0];

// ❌ Don't (the `fn ft()` helper)
fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }

// ✅ Do — accept theme as a parameter
pub fn my_widget(ui: &mut Ui, theme: &Theme) { ... }
// or, in ui_kit/, take the trait
pub fn my_widget(ui: &mut Ui, theme: &dyn ComponentTheme) { ... }
```

There are 56 known offenders (Widget impls in `ui_kit/widgets/*.rs` and free `fn ft()` helpers). Don't add a 57th.

### 2. Never hardcode black for shadows

```rust
// ❌ Breaks all 4 light themes (Bauhaus, Peach, Ivory, Newsprint)
Color32::from_rgba_unmultiplied(0, 0, 0, 60)

// ✅ Use the theme's shadow color
let s = t.shadow_color;
Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 60)
```

### 3. Use design tokens, not literals

| Don't write | Write |
|---|---|
| `FontId::monospace(11.0)` | `mono_sm()` |
| `FontId::monospace(13.0)` | `mono_md()` |
| `vec2(4.0, 4.0)` (button_padding etc.) | `vec2(gap_xs(), gap_xs())` |
| `Stroke::new(0.5, c)` | `Stroke::new(stroke_thin(), c)` |
| `Stroke::new(1.0, c)` | `Stroke::new(stroke_std(), c)` |
| `from_rgba_unmultiplied(_,_,_, 60)` | `color_alpha(c, alpha_muted())` |
| `c.gamma_multiply(0.6)` | `color_muted(c)` *(once added — see §5.3 of audit)* |
| `CornerRadius::same(4)` | `CornerRadius::same(radius_sm() as u8)` |

Token sources:
- `src/chart/renderer/ui/style.rs` — fonts, spacing, strokes, alpha, radii, shadows, semantic colors
- `src/ui_kit/widgets/tokens.rs` — `Size`, `Variant`, `Density` enums (use these in new components)
- `src/chart/renderer/gpu.rs::Theme` — palette (use `t.accent`, `t.bull`, `t.bear`, `t.dim`, `t.text`, `t.warn` — not raw RGB)

### 4. Prefer `ui_kit::Button` over `egui::Button`

```rust
// ❌ Don't roll your own
egui::Button::new(label).fill(t.toolbar_bg).stroke(...)

// ✅ Use the design-system button
use crate::ui_kit::widgets::Button;
Button::new(label)
    .variant(Variant::Ghost)   // Primary | Secondary | Ghost | Danger | Link | Chrome
    .size(Size::Sm)            // Xs | Sm | Md | Lg
    .show(ui, t)
```

There are 83 hand-rolled `egui::Button` calls in `chart/renderer/ui/`. Don't add an 84th — convert one when you're nearby.

`ui_kit::Button` has named role presets for the common cases — use them instead of the deprecated free functions in `chart/renderer/ui/style.rs`:

| You used to call | Now use |
|---|---|
| `tb_btn(ui, label, active, ...)` | `Button::toolbar(label).active(b).show(ui, t)` |
| `action_btn(ui, label, color, en)` | `Button::action(label).tint(color).enabled(en).show(ui, t)` |
| `trade_btn(ui, label, color, w)` | `Button::trade(label).tint(color).min_size((w, 24.0)).show(ui, t)` |
| `cta_btn(ui, label, color, en)` | `Button::cta(label).tint(color).enabled(en).show(ui, t)` |
| `simple_btn(ui, label, color, w)` | `Button::simple(label).tint(color).min_width(w).show(ui, t)` |
| `small_action_btn(ui, label, c)` | `Button::small_action(label).tint(c).show(ui, t)` |
| `close_button(ui, dim)` | `Button::close().show(ui, t).clicked()` |
| `ui.add(egui::Button::new(label).fill(tint_active).stroke(...).fg(...))` for toggle chips | `Button::toggle(label, active).tint(color).show(ui, t)` |
| `Button::simple(label).variant(Variant::Ghost).min_size(vec2(avail, 22.0))` (full-width add-row) | `Button::outline_full_width(label).show(ui, t)` |

The free functions are `#[deprecated]`. Migrate when you're nearby.

If you find yourself adding `.fg(...)` / `.glyph_color(...)` / using `Variant::Chrome`, you probably need a new variant (see audit §6.1). Talk before reaching for Chrome — it's the escape hatch, not the default.

For chips / pills / tags / badges: prefer `ui_kit::Tag` (label with `TagTone::Normal | Muted | Success | Warning`) and `ui_kit::Badge` (count / notification). The legacy modules `chart/renderer/ui/components/{chips,pills,pills_widget}.rs` are `#[deprecated]`.

For panel headers: prefer `ui_kit::Header` (`Header::panel(title) | Header::dialog(title) | Header::section(title)`). The legacy `panel_header()` / `dialog_header()` / `section_label()` free functions in `style.rs` will be retired.

For shadows: use the `_themed` variants (`shadow_card_themed(t)`, `shadow_modal_themed(t)`, `shadow_tooltip_themed(t)`, `shadow_dropdown_themed(t)`). They pull `t.shadow_color` so light themes get soft gray drops instead of hardcoded black smudges. The non-`_themed` variants are kept for legacy compatibility but should not be used in new code.

For new panels: implement `ui_kit::Panel` (`fn id(&self)`, `fn render(&mut self, ctx: PanelCtx) -> PanelResponse`) so the host can route close / focus / dirty signals through a common contract.

### 5. Don't use `ui.menu_button(...)` directly

There are 33 sites doing this with custom `RichText` styling. They drift apart over time. If you need a dropdown trigger, either:
- Use `ui_kit::Select` (for value pickers)
- Wait for `Button::menu()` to land (audit Tier 1 item)

In the meantime, copy the pattern from `top_nav.rs` so menus look the same.

### 6. Light-theme parity

We ship 15 themes. 4 are light (Bauhaus, Peach, Ivory, Newsprint). Before claiming UI work is done:
- Switch to Bauhaus, walk through the feature you touched
- Hardcoded white/black/dark-gray will be obvious there

## Side panel primitives

For new side panels, use:
- `SidePanelShell::new(id, title)` for the outer shell (replaces hand-rolled SidePanel + PanelHeader + PanelFrame chrome).
- `SidePanelShell::tabs(id, &mut state, tabs)` for tab-driven panels.
- `SplitSectionPanel::new(id, &mut splits)` for feed/signals/analysis multi-pane patterns.
- `PanelFooter::new()` for bottom action bars.

Width presets: `Width::Narrow` (240px), `Width::Medium` (300px), `Width::Wide` (400px). All resizable.

Floating panels (settings, news, connection): use `Header::dialog` + `Modal`, not SidePanelShell.

Body primitives (PanelSection/PanelEmpty/PanelLoading/PanelListRow/PanelCard/PanelKeyValueRow/PanelDivider) — see Agent K's foundation PR.

### `PanelSectionGroup` — user-resizable section stack

When a panel body wants N stacked `PanelSection`s with user-draggable
dividers between them (TOOLS / ACTIVE / LIBRARY style), use
`PanelSectionGroup`. The caller owns the persistent fraction state as a
`&mut [f32; N]` (typically a field on `Watchlist`). Each entry is the
section's fraction of the available vertical body height; the sum is
renormalized to 1.0 internally and a min-section-height is enforced
while dragging.

```rust
PanelSectionGroup::new(&mut watchlist.indicators_section_fracs)
    .min_section_height(40.0)
    .show(ui, t, |grp| {
        grp.section(|ui, t| {
            PanelSection::new("TOOLS").show(ui, t, |ui, t| { /* ... */ });
        });
        grp.section(|ui, t| {
            PanelSection::new("ACTIVE").count(n).show(ui, t, |ui, t| { /* ... */ });
        });
        grp.section(|ui, t| {
            PanelSection::new("LIBRARY").show(ui, t, |ui, t| { /* ... */ });
        });
    });
```

Divider visual: 6px hit-band, hairline rule at
`color_alpha(t.toolbar_border, 36)` with a center 3-dot triplet
(1.4px radius, gap 3px). Hover brightens both to alpha 96. Cursor
becomes `ResizeVertical` on hover/drag. Plain `PanelSection` stays
the right primitive when the body is natural-flow; reach for the
group only when the user benefit of dragging the split is real.

### `PanelSubSection` — collapsible category grouping

Nested INSIDE a `PanelSection` body (or anywhere a panel needs
sub-grouping). Caret + uppercase title + count chip header with a
hairline rule below (painted in both expanded and collapsed states).
Persistent expanded state via `&mut bool`. Clicking anywhere on the
header toggles. Body indents `gap_md()` when expanded.

```rust
PanelSubSection::new("trend", "TREND INDICATORS")
    .count(8)
    .expanded(&mut group.expanded)
    .show(ui, t, |ui, t| { /* body */ });
```

Optional `.header_trailing(|ui, t| ...)` slot for group-level controls
(visibility toggle, opacity picker) that conceptually belong to the
category rather than any single body row. The slot is laid out
right-to-left at the right edge of the header (between the count chip
and the right margin). Clicks inside the slot do NOT toggle `expanded` —
the toggle's click sense is reserved to the caret/title/chip region:

```rust
PanelSubSection::new("trend", "TREND INDICATORS")
    .count(8)
    .expanded(&mut group.expanded)
    .header_trailing(|ui, t| {
        if Button::icon(Icon::EYE).variant(Variant::Ghost).show(ui, t).clicked() {
            group.all_visible = !group.all_visible;
        }
    })
    .show(ui, t, |ui, t| { /* body */ });
```

| Hand-rolled pattern | Replace with |
|---|---|
| `indicators_panel.rs` LIBRARY category headers (caret + UPPER title + chip + inter-category divider) | `PanelSubSection` |
| `object_tree.rs` folder rows | `PanelSubSection` |
| `PanelSubSection` body's first row being the group-level toggle / opacity picker | `.header_trailing(|ui, t| { ... })` |
| Generic non-categorical disclosure with proportional title in `t.text` | `Disclosure` (unchanged) |

### `PanelListRow::columns(&[Column...])` — free-form column layout

Alternate row layout for streaming-data panels (tape T&S, scanner T&S,
journal). Switches the row to a free-form N-column layout, ignoring
`.primary()` / `.secondary()` / `.leading()` / `.trailing()`. Selected
+ hover backgrounds still apply. Slice-based API — no per-cell heap
allocations (important: tape can emit ~200 rows/frame).

```rust
PanelListRow::new("print_123")
    .columns(&[
        Column::left("09:31:42.123").color(t.dim),
        Column::right("$145.32").color(t.bull),
        Column::right("250").color(t.text),
    ])
    .show(ui, t);
```

Re-exported as `PanelColumn` / `PanelColAlign` from `ui_kit::widgets`
to avoid clashing with `Table`'s `Column` / `ColAlign`.

| Hand-rolled pattern | Replace with |
|---|---|
| `tape_panel.rs` free-form 3-column print rows | `PanelListRow::columns(&[...])` |
| `scanner_panel.rs` T&S column rows | `PanelListRow::columns(&[...])` |
| Journal table rows (date / pnl / R / notes) | `PanelListRow::columns(&[...])` |

### `PanelListRow::trailing_buttons(&[TrailingBtn...])` — typed icon strip

When a row needs 2–4 inline icon buttons (delete / lock / visibility /
reset), the closure-style `.trailing(|ui, t| ...)` slot forces callers
to wire each click through a `Cell<bool>` trampoline because the
`FnOnce` is consumed by `.show()`. The typed-slice form eliminates the
boilerplate. Use `.show_full(ui, t)` to read which button fired:

```rust
let resp = PanelListRow::new("drawing_42")
    .leading(|ui, _| paint_swatch(ui, color))
    .primary("Trendline AAPL")
    .trailing_buttons(&[
        TrailingBtn::icon(Icon::TRASH).tone(TrailingTone::Bear).tooltip("Delete"),
        TrailingBtn::icon(if locked { Icon::LOCK } else { Icon::LOCK_OPEN })
            .active(locked).tooltip("Lock"),
        TrailingBtn::icon(if visible { Icon::EYE } else { Icon::EYE_SLASH })
            .active(visible).tooltip("Visibility"),
    ])
    .show_full(ui, t);
if resp.clicked { /* row body click */ }
match resp.trailing_clicked_idx {
    Some(0) => delete(),
    Some(1) => toggle_lock(),
    Some(2) => toggle_visible(),
    _ => {}
}
```

API notes:
- `TrailingTone` variants: `Default | Accent | Bull | Bear | Warn | Muted`
  — resolve to a `Theme` color so call sites stay theme-aware (no raw RGB).
- `.active(true)` paints a subtle bg fill at `color_alpha(tone, alpha_soft())`.
- Idle glyph paints tone-at-`t.dim`-opacity; hover snaps to full tone color.
- `.trailing_buttons(&[...])` and `.trailing(|ui, t| ...)` are mutually
  exclusive — last setter wins.
- Clicks are reported as `Option<usize>` (only one can fire per frame
  since egui consumes pointer presses on the topmost interactable rect).
- The slice is borrowed for one frame; no per-button heap allocations.
- `.show()` still returns plain `Response` for backwards compatibility —
  use `.show_full()` when you need the clicked index.

| Hand-rolled pattern | Replace with |
|---|---|
| `PanelListRow` trailing closure with `Cell<bool>` trampolines for multiple icon buttons | `.trailing_buttons(&[TrailingBtn::icon(...).tone().tooltip().active(), ...])` + `.show_full()` |
| Single inline trailing button or a custom non-icon widget (price, dropdown) | `.trailing(|ui, t| ...)` (unchanged) |

### Shell API contract (open flag, pane alignment)

`SidePanelShell::show` / `SidePanelShell::tabs::show` / `SplitSectionPanel::show`
do NOT take `&mut open`. The caller does its own early-return and writes its
flag back from the returned `SidePanelShellResponse.close_clicked`. This
removes the borrow conflict that arises when the body closure also needs
`&mut watchlist` (where the open flag lives).

```rust
if watchlist.alerts_panel_open {
    let resp = SidePanelShell::new("alerts", "ALERTS")
        .icon(Icon::BELL)
        .width(Width::Narrow)
        .show(ctx, t, |ui, t| { /* body, &mut watchlist OK */ });
    if resp.close_clicked { watchlist.alerts_panel_open = false; }
}
```

For pane-header alignment, prefer `.pane_aligned(&watchlist)` when the body
closure does NOT borrow `watchlist`. When it does, pre-resolve and pass via
`.pane_metrics(height, title_font)`:

```rust
let pane_h    = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
let pane_font = watchlist.pane_header_size.title_font();
SidePanelShell::new(...)
    .pane_metrics(pane_h, pane_font)
    .show(ctx, t, |ui, t| { /* uses &mut watchlist freely */ });
```

### `PanelSection::action` — no closure

`PanelSection::action(label, tone)` takes only the label and tone. The click
surfaces via `SectionResponse.action_clicked` from `.show(...)`:

```rust
let resp = PanelSection::new("ACTIVE")
    .count(n)
    .action("Clear All", PanelTone::Danger)
    .show(ui, t, |ui, t| { /* body */ });
if resp.action_clicked { /* handle */ }
```

### `PanelSection` opt-in features (Wave 10a)

`PanelSection` now supports three opt-in builders that bring it to
feature parity with the legacy `chart/renderer/ui/watchlist/section_header.rs`
widget (so that wrapper can be retired):

- **`.collapsible(&mut expanded)`** — leading caret + click-anywhere-on-header
  toggle. Body closure is skipped when collapsed (`SectionResponse.body` is
  `None`); count chip is rendered inline as `(N)` next to the title.
- **`.delete_when_empty()`** — when set AND `.count(0)` is set, the
  trailing slot paints an `Icon::X` ghost button; click surfaces via
  `SectionResponse.delete_clicked`. Precedence: when `count == 0` the
  delete button wins over `.action(...)`; for `count > 0` the action
  button paints normally.
- **`SectionResponse.header_response`** — the full header-strip
  `egui::Response`, exposed so callers can attach
  `.context_menu(|ui| { Rename / Color / Delete })`. The frame's
  `chevron_clicked` flag fires once on the frame `*expanded` flipped.

```rust
let resp = PanelSection::new("WATCHLIST")
    .count(items.len())
    .collapsible(&mut section.expanded)
    .delete_when_empty()
    .show(ui, t, |ui, t| { /* body */ });
if resp.delete_clicked { delete_section(section); }
resp.header_response.context_menu(|ui| {
    if ui.button("Rename").clicked() { /* … */ }
    if ui.button("Color").clicked()  { /* … */ }
    if ui.button("Delete").clicked() { /* … */ }
});
```

## Where to find things

| Question | File |
|---|---|
| Add a token | `src/chart/renderer/ui/style.rs` |
| Add a widget | `src/ui_kit/widgets/` (use `Button` as the template) |
| Add a theme palette field | `src/chart/renderer/gpu.rs::Theme` *(and update all 15 entries in `THEMES`)* |
| Add an icon | `src/ui_kit/icons.rs` |
| What does a token do? | `docs/DESIGN_SYSTEM.md` |
| Audit / punch list | `docs/UI_AUDIT.md` |

## Adding a new component — checklist

1. Builder lives in `src/ui_kit/widgets/<name>.rs`
2. `pub struct <Name><'a> { ... }` with builder-style methods returning `Self`
3. `pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response`
4. Use `Size`/`Variant` from `ui_kit::widgets::tokens` for sizing/states
5. Pull colors from the `theme: &dyn ComponentTheme` argument — **never** from `&THEMES[0]`
6. Add a 5–10 line module doc explaining: what it does, what variants it has, when to use it, when to use a different widget
7. Re-export from `src/ui_kit/widgets/mod.rs`

## Before you commit a UI change

- [ ] No new `&THEMES[0]` references
- [ ] No new `Color32::from_rgba_unmultiplied(0, 0, 0, …)` shadows
- [ ] Font sizes use `font_*()` / `mono_*()` helpers
- [ ] Spacing uses `gap_*()` helpers
- [ ] Strokes use `stroke_*()` helpers
- [ ] Theme colors come from a threaded `Theme` / `&dyn ComponentTheme`, not raw RGB
- [ ] Walked the feature in a light theme (Bauhaus) at least once
