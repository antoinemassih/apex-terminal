# UI / Design System — agent guide

> Read this **before** touching any UI code. The full audit lives in `docs/UI_AUDIT.md`.

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

If you find yourself adding `.fg(...)` / `.glyph_color(...)` / using `Variant::Chrome`, you probably need a new variant (see audit §6.1). Talk before reaching for Chrome — it's the escape hatch, not the default.

### 5. Don't use `ui.menu_button(...)` directly

There are 33 sites doing this with custom `RichText` styling. They drift apart over time. If you need a dropdown trigger, either:
- Use `ui_kit::Select` (for value pickers)
- Wait for `Button::menu()` to land (audit Tier 1 item)

In the meantime, copy the pattern from `top_nav.rs` so menus look the same.

### 6. Light-theme parity

We ship 15 themes. 4 are light (Bauhaus, Peach, Ivory, Newsprint). Before claiming UI work is done:
- Switch to Bauhaus, walk through the feature you touched
- Hardcoded white/black/dark-gray will be obvious there

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
