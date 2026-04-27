# Apex Terminal — Theme & Design-Token Audit

_Snapshot of the styling layer in `src-tauri/src/` as it stands today, plus a recommendation for the next move._

All file paths below are relative to `src-tauri/src/`. Line numbers refer to the current revision at audit time.

---

## 1. The `Theme` struct (`chart_renderer/gpu.rs`)

Defined at `chart_renderer/gpu.rs:76-81`:

```rust
pub(crate) struct Theme {
    pub(crate) name: &'static str,
    pub(crate) bg, bull, bear, dim: egui::Color32,
    pub(crate) toolbar_bg, toolbar_border, accent: egui::Color32,
    pub(crate) text: egui::Color32,
}
```

Eight colour fields plus `name`. Semantic meaning:

| Field            | Meaning                                                                                                         |
| ---------------- | --------------------------------------------------------------------------------------------------------------- |
| `bg`             | Chart canvas / app background. Also drives `Theme::is_light()` (`gpu.rs:142-146`) via luminance > 400.          |
| `bull`           | Up-candle / positive-PnL / BUY colour.                                                                          |
| `bear`           | Down-candle / negative-PnL / SELL colour.                                                                       |
| `dim`            | Secondary text + axis labels + inactive icons. Used as fallback wherever a "muted" colour is needed.            |
| `toolbar_bg`     | Toolbar fill, panel frame fill (`ui/style.rs:137`), tooltip fill (`ui/style.rs:503`), dialog fill when themed.  |
| `toolbar_border` | All panel/tooltip/dialog stroke colour, plus segmented-control trough border.                                   |
| `accent`         | Active toolbar button, active tab underline, primary CTAs, link-coloured glyphs.                                |
| `text`           | Primary text. `(220,220,230)` for all 12 dark themes, `(18-22,…)` for the three light themes.                   |

`THEMES: &[Theme]` (`gpu.rs:122-140`) is **15** entries, not 12 — the comment in the prompt is stale. 12 dark + 3 light: Midnight, Nord, Monokai, Solarized, Dracula, Gruvbox, Catppuccin, Tokyo Night, Kanagawa, Everforest, Vesper, Rosé Pine, Bauhaus, Peach, Ivory.

**Consumers of `Theme`:** every UI module. Common pattern is `let t = &THEMES[chart.theme_idx];` then `t.bg`, `t.accent`, etc. Hot reads:

- `chart_renderer/gpu.rs:3165, 5294, 16601, 16735, 16745` — render hot path.
- All `chart_renderer/ui/*.rs` panels accept `accent`/`dim`/`toolbar_bg`/`toolbar_border` parameters threaded down from a top-level `let t = &THEMES[..];`.
- `ui/style.rs` helpers (`tb_btn`, `panel_frame`, `tooltip_frame`, `dialog_window_themed`, `segmented_control`, `tab_bar`, …) take the four colour args explicitly rather than reading a global `Theme`.

**Observation:** `Theme` is plumbed as **loose colour params**, not as a `&Theme` reference. That means every call site decides which subset to pass, and any new field on `Theme` requires touching every call site.

---

## 2. `design_tokens.rs` — runtime token system

374 lines. The whole module is gated on `#[cfg(feature = "design-mode")]` — when the feature is **off** (the default release build), the entire struct doesn't compile and the macros expand to the literal default.

### Token groups

A flat `DesignTokens` struct (`design_tokens.rs:17-43`) composed of 23 sub-structs:

| Group           | Fields                                                                                                                            |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `font`          | xxs, xs, sm_tight, sm, md, input, lg, xl, xxl, display, display_lg                                                                |
| `spacing`       | xs … xxxl (7 tiers)                                                                                                               |
| `radius`        | xs, sm, md, lg                                                                                                                    |
| `stroke`        | hair, thin, std, bold, thick, heavy, xheavy                                                                                       |
| `alpha`         | faint(10) ghost(15) soft(20) subtle(25) tint(30) muted(40) line(50) dim(60) strong(80) active(100) heavy(120)                     |
| `shadow`        | offset, alpha, spread, gradient[3]                                                                                                |
| `toolbar`       | height, height_compact, btn_min_height, btn_padding_x, right_controls_width                                                       |
| `panel`         | margins (compact + normal), seven width presets, two tooltip widths, two content widths                                           |
| `dialog`        | header_darken, header_padding_x/y, section_indent                                                                                 |
| `button`        | action_height, trade_height, small_height, simple_height, trade_brightness, trade_hover_brightness                                |
| `card`          | margins, radius, stripe_width, four size presets                                                                                  |
| `badge`, `tab`, `table`, `chart`, `watchlist`, `order_entry`, `pane_header`, `segmented`, `icon_button`, `form`, `split_divider`, `tooltip`, `separator` | per-component primitives |
| `color`         | text_primary/secondary/dim/on_accent, amber, earnings, paper_orange, live_green, danger, triggered_red, dark_pool, info_blue, discord, dialog_fill, dialog_border, deep_bg, deep_bg_alt, pane_tints[4] |

Defaults at `design_tokens.rs:133-170`. TOML save/load at `:175, :360`. Global storage: `OnceLock<RwLock<DesignTokens>>` at `:185`.

### `dt_f32!` (and siblings `dt_u8!`, `dt_u32!`, `dt_usize!`, `dt_i8!`)

Defined at `design_tokens.rs:275-356`. Pattern:

```rust
dt_f32!(font.lg, 13.0)
// expands to:
//   if feature = design-mode AND tokens loaded → t.font.lg
//   else → 13.0
```

The default literal **must** match the constant elsewhere or the build will visibly drift between design-mode-on and design-mode-off. There is no compile-time check enforcing this.

### Inspect / hit-tracking subsystem

`design_tokens.rs:217-258`. Per-frame `ELEMENT_HITS` thread-local; `register_hit(rect, family, category)` is called by helpers in `ui/style.rs` (e.g. `:173, :437, :568`). Powers `design_inspector.rs`. No-op stub when feature is off.

### Are tokens used or mostly inert?

**Mostly inert in production.** Default builds compile to literal constants; only `design_inspector.rs` (4 hits) and `ui/style.rs` (67 hits) actually invoke the macro at all, plus 7 hits in `gpu.rs`. The remaining ~40 panel files in `chart_renderer/ui/*.rs` contain **zero** `dt_*!` calls — they use raw `Color32::from_rgb`, magic floats, and the all-caps `FONT_*`/`RADIUS_*` constants directly.

So the design-token system is plumbed through the shared style helpers, but barely reaches the leaves.

---

## 3. `chart_renderer/ui/style.rs` — 743 lines

The doc comment at the top (`ui/style.rs:1-12`) advertises this as the single source of truth for fonts/spacing/radius/stroke/alpha/shadow/text. In practice, it has both:

### Constants (compile-time, all-caps)

`ui/style.rs:35-114`:

- `FONT_XS=8, FONT_SM=10, FONT_MD=11, FONT_LG=12, FONT_XL=13, FONT_2XL=14`
- `GAP_XS=1, GAP_SM=3, GAP_MD=5, GAP_LG=6, GAP_XL=8, GAP_2XL=10, GAP_3XL=16`
- `RADIUS_SM=4, RADIUS_MD=6, RADIUS_LG=12`
- `STROKE_HAIR=0.3, STROKE_THIN=0.5, STROKE_STD=1.0, STROKE_BOLD=1.5, STROKE_THICK=2.0`
- `ALPHA_FAINT=10 … ALPHA_HEAVY=120` (11 tiers)
- `SHADOW_OFFSET=2, SHADOW_ALPHA=60, SHADOW_SPREAD=4`
- `TEXT_PRIMARY = (220,220,230)`, `TEXT_SECONDARY = (200,200,210)`

### Function variants (runtime, dt_*-backed)

Same names lower-cased: `font_lg()`, `radius_md()`, `alpha_dim()`, etc. These read from `DesignTokens` when `design-mode` is on.

**Conflict:** the const `FONT_LG = 12.0` but the function `font_lg()` defaults to `13.0`. The const `RADIUS_SM = 4.0` but `radius.sm` default is `3.0`. There are at least **5 such drifts** between consts and dt defaults — a refactor hazard.

### Helpers (~30 functions)

Roughly grouped:

- **Frames**: `panel_frame`, `panel_frame_compact` (`:137, :145`)
- **Buttons**: `tb_btn` (`:157`), `action_btn` (`:601`), `trade_btn` (`:615`), `small_action_btn`, `simple_btn`, `close_button`, `icon_btn` (`:427`)
- **Dialogs**: `popup_frame`, `dialog_window`, `dialog_window_themed`, `dialog_header`, `dialog_header_colored`, `dialog_separator`, `dialog_separator_shadow`, `dialog_section`
- **Labels**: `mono`, `mono_bold`, `section_label`, `dim_label`, `col_header`
- **Composites**: `segmented_control` (`:356`), `tab_bar` (`:474`), `tooltip_frame`, `stat_row`, `paint_tooltip_shadow`
- **Cards/badges**: `status_badge`, `order_card`
- **Forms**: `form_row`, `split_divider`
- **Drawing**: `dashed_line`, `draw_line_rgba`
- **Utils**: `hex_to_color`, `color_alpha`, `separator`

`gpu.rs:165` re-exports a curated subset, which is how the rest of the renderer pulls them in.

**Consumers:** all `chart_renderer/ui/*.rs` panel files (40+) plus 50+ call sites inside `gpu.rs` for the chart overlay.

---

## 4. The new `style_id()` system

Added at `chart_renderer/gpu.rs:87-120`.

- `STYLE_NAMES` (`:87`): 10 placeholder presets — Meridien, Aperture, Octave, Cadence, Chord, Lattice, Tangent, Tempo, Contour, Relay.
- `style_id(wl)` (`:95`): collapses `wl.style_idx` to `0|1|2`. Anything ≥ 3 aliases back to Meridien (0).
- `pane_header_h(wl)` (`:101`) and `pane_tabs_header_h(wl)` (`:112`): only consumers today. They tweak `PaneHeaderSize::header_h() / tabs_header_h()` by `+2` for Aperture, `-2` for Octave, when in `Compact` mode.

**Persistence:** `style_idx` is a field on `Watchlist` (`gpu.rs:17136`), defaulted to `0` (`:17363, :19881`), serialized into settings JSON (`:19833`), reloaded with bounds-check (`:20059-20060`), and exposed in the theme picker UI at `gpu.rs:4607-4652` (combined display string `"GruvBox/Meridien"`).

**Where `style_id` is read:** _only_ in `gpu.rs` (`:5352` plus the two helpers). The 40+ panel modules don't see it. Effective surface area today: pane header height in Compact mode. Nothing else.

---

## 5. Top-10 hardcoded gaps

These are spots where a literal exists and a token / Theme field obviously _should_ replace it.

1. **`dialog_window` fill = `Color32::from_rgb(26,26,32)`** (`ui/style.rs:208`). Should be `t.toolbar_bg` or `color.dialog_fill`. Light themes get a black popup on a cream chart.
2. **`dialog_window` border = `(60,60,70,80)`** (`ui/style.rs:209`). Same problem.
3. **`tb_btn` font + min size** are literal `12.0` and `(0,24)` (`ui/style.rs:170-172`) — bypass the token system entirely.
4. **`segmented_control` button height + pad** are literal `seg_btn_h = 20.0`, `seg_pad_x = 5.0` (`ui/style.rs:383-384`).
5. **`action_btn` font 9.0, height 20.0, radius 3.0, stroke 0.5** (`ui/style.rs:606-608`) — five magic numbers in one widget, despite `button.action_height` existing in the token struct.
6. **`trade_btn` font 11, height 24, radius 3, width param** (`ui/style.rs:621-622`) — same; `button.trade_height = 30.0` is in tokens but not used.
7. **`section_label` font 7.0** (`ui/style.rs:326`) — should be `font.xxs` (default 7.0); the constant doesn't even exist as `FONT_*`.
8. **`panel_header*` font 11.0, subtitle 9.0** (`ui/style.rs:462-464`) — literal.
9. **`PaneHeaderSize::{header_h, tabs_header_h, title_font}`** (`mod.rs:580-588`) hard-coded ladders (22/26/32, 28/32/38, 11/12/14). `pane_header.height_compact / height` exists in tokens but isn't read here.
10. **The 12-theme `text` colour is universally `(220,220,230)`** (`gpu.rs:123-135`) — Midnight, Dracula, Solarized all share the exact same primary text. This is a Theme-design gap: themes don't differentiate text/text_secondary/text_dim, but the token system has `text_primary/secondary/dim/on_accent`. Themes should own these, not tokens.

Honourable mentions: bull-PnL `(46,204,113)` and bear `(231,76,60)` baked into multiple panels (`chart_renderer/ui/orders.rs`, `portfolio_pane.rs`) instead of `t.bull` / `t.bear`; alert/badge palette in `color` token group is never read (the panels reach for `Color32::from_rgb` directly — see the 16 `from_rgb` hits in `apex_diagnostics.rs`).

---

## 6. Migration recommendation

**Diagnosis.** Three overlapping systems exist:

1. `Theme` — colour palette, 15 named variants, swap-at-runtime.
2. `DesignTokens` — sizes/spacing/alphas, gated on a feature flag, mostly used only via `ui/style.rs` helpers.
3. `STYLE_NAMES` / `style_id()` — _intent_ is "compositional UI feel" (density, corner shape, weight) but currently only nudges header heights.

Each picks up where the others leave off, but no module owns the combined output. Panel code receives `(accent, dim, toolbar_bg, toolbar_border)` tuples — the same four args repeated thousands of times.

**The smart next move** is to introduce a thin `theme::ui` module that resolves `(theme_idx, style_idx, design-mode-tokens)` into a single immutable `UiStyle` struct passed (or thread-local'd) through the frame. All three layers fold into one read-only view per frame.

### Sketch — `chart_renderer/ui/theme.rs`

```rust
//! Resolved per-frame UI style. Built once at frame start from
//! (THEMES[theme_idx], STYLE_NAMES[style_idx], DesignTokens::current()).
//! Read-only for the rest of the frame.

pub struct UiStyle<'a> {
    // ── Palette (from Theme) ──
    pub bg:             Color32,
    pub bull:           Color32,
    pub bear:           Color32,
    pub dim:            Color32,
    pub accent:         Color32,
    pub text:           Color32,
    pub text_secondary: Color32,
    pub text_dim:       Color32,
    pub surface:        Color32,   // was toolbar_bg
    pub border:         Color32,   // was toolbar_border

    // ── Geometry (Tokens, modulated by style preset) ──
    pub font:    FontScale,        // xs..xxxl, resolved
    pub gap:     SpacingScale,
    pub radius:  RadiusScale,
    pub stroke:  StrokeScale,
    pub alpha:   AlphaScale,
    pub shadow:  ShadowSpec,
    pub pane_header_h:      f32,
    pub pane_tabs_header_h: f32,

    // ── Style-preset modulation ──
    pub preset:  StylePreset,      // Meridien | Aperture | Octave | …
    pub density: Density,          // Compact | Normal | Expanded

    _theme: &'a Theme,             // raw access if needed
}

impl<'a> UiStyle<'a> {
    pub fn build(theme_idx: usize, wl: &Watchlist) -> Self { /* … */ }
    pub fn is_light(&self) -> bool { /* … */ }

    // Convenience colour ops
    pub fn fg_on(&self, bg: Color32) -> Color32 { /* contrast-aware */ }
    pub fn tint(&self, c: Color32, a: AlphaTier) -> Color32 { /* … */ }
}

// The four scale structs hold pre-resolved f32s (no macro indirection at use site).
pub struct FontScale  { pub xs: f32, pub sm: f32, pub md: f32, pub lg: f32, pub xl: f32, pub xxl: f32 }
pub struct SpacingScale { /* … */ }
pub struct RadiusScale  { pub sm: f32, pub md: f32, pub lg: f32 }
// etc.

#[derive(Copy, Clone)] pub enum StylePreset { Meridien, Aperture, Octave }
#[derive(Copy, Clone)] pub enum Density     { Compact, Normal, Expanded }
```

### Migration in three passes

1. **Build the module + adapter.** Add `UiStyle::build()` that fills from `THEMES`, `dt_*`, and `style_id`. Existing `panel_frame(toolbar_bg, toolbar_border)` becomes `panel_frame(s: &UiStyle)`. Helpers in `ui/style.rs` lose their colour-tuple arguments.
2. **Sweep panel files.** Each panel signature `fn show(..., accent, dim, toolbar_bg, toolbar_border)` becomes `fn show(..., s: &UiStyle)`. Mostly mechanical. ~40 files. Each `Color32::from_rgb(26,26,32)` becomes `s.surface`, etc.
3. **Wire `StylePreset` to real visual differences.** Aperture → larger radii, more spacing, lighter strokes. Octave → tighter density, square corners, heavier alphas. Meridien stays as the current baseline. Concretely, `UiStyle::build` post-processes the resolved `radius/gap/stroke` scales by preset.

After pass 2, the `dt_*!` macros become an internal detail of `UiStyle::build()`, and the per-frame style is a single allocation. After pass 3, `style_idx` actually does something visible across the whole UI rather than nudging two header heights.

### Why this is the smart move

- **One ownership boundary.** Today, "what does the UI look like?" is answered by reading `Theme`, then `dt_f32!` lookups, then `style_id` branches. After: read `UiStyle`. Done.
- **Zero feature-flag drift.** `UiStyle::build()` is the only place that branches on `cfg(feature = "design-mode")`. Constants and macro defaults can no longer disagree silently.
- **Style presets become real.** The `STYLE_NAMES` list stops being decorative. Adding "Cadence" means adding one match arm in `build()`.
- **Theme additions are cheap.** Adding `text_dim` to `Theme` no longer touches every panel — only `UiStyle`.
- **Inspect-mode unchanged.** `register_hit()` still works; `UiStyle` is a read layer above the existing `DesignTokens` storage.

The cost is one mechanical sweep of ~40 panel files. The payoff is that the next change to the design system happens in one file.

---

_Audit ends._
