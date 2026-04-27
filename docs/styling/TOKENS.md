# Chart Renderer — Magic-Number Inventory (Styling Tokens)

This is a static catalog of every hard-coded visual literal found under
`apex-terminal/src-tauri/src/chart_renderer/`. It is meant as the input for the
upcoming styling overhaul: any value listed here is a candidate either for
(a) replacement with an existing token in `ui/style.rs`, or (b) promotion of a
new token if it is repeated 5+ times.

The reference token table already in `ui/style.rs` is:

| Token              | Value | Token              | Value |
|--------------------|-------|--------------------|-------|
| `FONT_XS`          | 8.0   | `RADIUS_SM`        | 4.0   |
| `FONT_SM`          | 10.0  | `RADIUS_MD`        | 6.0   |
| `FONT_MD`          | 11.0  | `RADIUS_LG`        | 12.0  |
| `FONT_LG`          | 12.0  | `STROKE_HAIR`      | 0.3   |
| `FONT_XL`          | 13.0  | `STROKE_THIN`      | 0.5   |
| `FONT_2XL`         | 14.0  | `STROKE_STD`       | 1.0   |
| `GAP_XS`..`GAP_3XL`| 1/3/5/6/8/10/16 | `STROKE_BOLD` / `THICK` | 1.5 / 2.0 |
| `ALPHA_*`          | 10..120 in 10-step tiers | `SHADOW_OFFSET/ALPHA/SPREAD` | 2.0 / 60 / 4.0 |

Note: `style.rs` also exports runtime equivalents (`font_md()`, `gap_lg()`, …)
that read from `DesignTokens` when the `design-mode` feature is on. **The
overhaul should prefer the function form** so values are tunable at runtime.

---

## 1. Padding & Spacing

### 1.1 `ui.add_space(N)` — vertical/horizontal cursor advance

| Value | Count | Maps to existing token | Notes / consolidation |
|------:|------:|------------------------|-----------------------|
| `4.0` | 149 | `gap_sm()` (4.0) | Most common gap — every site should be `add_space(gap_sm())`. |
| `6.0` |  89 | `gap_md()` (6.0) | Standard "between rows" gap. |
| `2.0` |  72 | `gap_xs()` (2.0) | Used inside compact card layouts. |
| `8.0` |  42 | `gap_lg()` (8.0) | Section gap; `apex_diagnostics.rs` alone has 6+ adjacent calls (lines 39-47). |
| `10.0`|  11 | `gap_xl()` (10.0) | |
| `20.0`|   9 | `gap_3xl()` (20.0) | Large dialog separators. |
| `3.0` |   8 | `GAP_SM` (3.0) | |
| `12.0`|   7 | `gap_2xl()` (12.0) | |
| `22.0`, `24.0` | 2 ea. | — | Custom indents in `chart_widgets.rs`, `command_palette.rs`. Consider new `GAP_4XL = 24.0`. |
| `1.0`, `5.0`, `14.0`, `60.0` | 1 ea. | — | One-offs; safe to leave. |

> **Action:** mass-replace literal `add_space(N)` calls with the function
> tokens. The 5 most-used values (4.0, 6.0, 2.0, 8.0, 3.0) cover ~360 sites.

### 1.2 `egui::vec2(W, H)` used for spacing (item_spacing, button_padding, etc.)

| File:line | Value | Purpose |
|-----------|------:|---------|
| `gpu.rs:3233` | `vec2(12.0, 6.0)` | global `style.spacing.button_padding` |
| `gpu.rs:3236` | `vec2(6.0, 4.0)`  | global `style.spacing.item_spacing` |
| `gpu.rs:3192-3193` | `CornerRadius::same(4)` / `same(6)` | `widgets.*.corner_radius` global |
| `style.rs:397` | `vec2(5.0, prev_pad.y)` | segmented control `seg_pad_x` |
| `style.rs:430` | `vec2(0.0, 0.0)` | icon_btn padding zeroing (intentional) |

There are also dozens of `vec2(8.0, 4.0)` / `vec2(6.0, 4.0)` style item-spacing
overrides scattered through `gpu.rs`, `command_palette.rs`,
`indicator_editor.rs`. **Suggestion:** add `pub const PAD_BUTTON: Vec2 =
vec2(12.0, 6.0); pub const PAD_ITEM: Vec2 = vec2(6.0, 4.0);` in `style.rs`.

### 1.3 Painter `+ N.0` x/y nudge offsets

Common pattern: `egui::pos2(r.left() + 4.0, r.bottom() + 0.5)` for underline and
bevel painting (e.g. `style.rs:178-179`, `style.rs:186`, many sites in
`gpu.rs`). The recurring values are `0.5` (sub-pixel snap), `1.0`, `2.0`, `4.0`.
These are visual nudges and should usually map to `STROKE_THIN`/`gap_sm`.

---

## 2. Corner Radius

The codebase mixes three styles: `corner_radius(N.0)` (f32),
`CornerRadius::same(N)` (u8), and tuple structs. There are existing tokens —
yet very few sites use them.

### 2.1 `corner_radius(N)` literal calls (chart_renderer-wide)

| Value | Approx count | Existing token | Sites |
|------:|-------------:|---------------|-------|
| `2.0` | 18+ | none — propose `RADIUS_XS = 2.0` | `gpu.rs` toolbar cells (lines 968, 985, 991, 1016, 1024, 1055, 1115, 7761, 7778, 7786, 11139, 11154, 11172, 11626, 11630), `indicator_editor.rs` (lines 152, 214, 428, 476). |
| `3.0` | 9+  | none — propose `RADIUS_2XS = 3.0` | `style.rs:608, 622` (`action_btn`, `trade_btn`), `gpu.rs:4885, 4896, 11187, 15308`, `hotkey_editor.rs:97`, `indicator_editor.rs:493, 499`. |
| `4.0` | 13+ | `RADIUS_SM` (4.0) ✅ | `style.rs:171, 187`, `gpu.rs:3192, 7745, 11270, 15295, 18493`, `discord_panel.rs:339`, `spread_panel.rs:475`, `watchlist_panel.rs:272, 1505`. **Should use `radius_sm()`**. |
| `6.0` | 8+  | `RADIUS_MD` (6.0) ✅ | `gpu.rs:3193, 4938, 5188, 11089, 15835, 15944, 18494`, `indicator_editor.rs:38`. **Should use `radius_md()`**. |
| `7.0` | 2   | — | `apex_diagnostics.rs:70-71` pill bg+stroke. |
| `8.0` | 4   | `gap_lg` looks-alike but semantically corner | `apex_diagnostics.rs:26`, `plays_panel.rs:417, 443`, tooltip default. Consider `RADIUS_LG_TIGHT = 8.0` distinct from current `RADIUS_LG = 12.0`. |
| `9.0` | 2   | — | `command_palette.rs:333, 404` (badge). |
| `10.0`| 1   | — | `command_palette.rs:299`. |
| `12.0`| 2+  | `RADIUS_LG` (12.0) ✅ | `style.rs:227`, `gpu.rs:11242`. |

### 2.2 `CornerRadius::same(N)` (u8) sites

| File:line | N | Notes |
|-----------|--:|-------|
| `gpu.rs:3192` | 4 | `widgets.noninteractive.corner_radius` |
| `gpu.rs:3193` | 6 | `widgets.inactive` (comment: "halved from 12") |
| `gpu.rs:18493` | 3 | `r3` local |
| `gpu.rs:18494` | 6 | `r6` local |
| `gpu.rs:18496` | 4 | `menu_corner_radius` |
| `gpu.rs:15835` | 6 | popover corner |
| `gpu.rs:15944` | 6 | popover corner |
| `style.rs:391-394` | rsm (=`radius_sm() as u8`) | proper token use ✅ |
| `style.rs:587-588` | `cr as u8` | order-card stripe |
| `style.rs:251-253` | `12u8` (rlg local) | dialog header — should be `radius_lg() as u8`. |

> **Consolidation:** Add `RADIUS_XS = 2.0` and `RADIUS_2XS = 3.0` (or merge them
> with a clearer naming). 18+ sites use literal `2.0` and 9+ use literal `3.0` —
> both far past the 5+ threshold.

---

## 3. Stroke Widths

| Value | Count | Existing token | Action |
|------:|------:|----------------|--------|
| `1.0` | 151 | `STROKE_STD` ✅ | Replace literal `Stroke::new(1.0, …)` with `Stroke::new(stroke_std(), …)`. The single biggest win in the codebase. |
| `0.5` |  86 | `STROKE_THIN` ✅ | Same — replace with `stroke_thin()`. |
| `1.5` |  45 | `STROKE_BOLD` ✅ | Replace with `stroke_bold()`. |
| `0.8` |  20 | — | New `STROKE_MED = 0.8`? Used for toolbar buttons (`style.rs:171`, `chart_widgets.rs:226`) and many `gpu.rs` sites. Past threshold. |
| `2.0` |   9 | `STROKE_THICK` ✅ | Replace with `stroke_thick()`. |
| `1.2` |   8 | — | Border accents (just below threshold). |
| `0.3` |   7 | `STROKE_HAIR` ✅ | Replace with `stroke_hair()`. |
| `2.5` |   5 | — | At threshold; consider `STROKE_2XL = 2.5`. |
| `3.0`, `3.5`, `4.0`, `5.0`, `10.0` | 1-3 | — | One-offs (focus rings, screenshot border). Leave inline. |
| `1.3`, `0.6`, `0.7`, `1.8` | 1-2 | — | Drift — should be normalized to `THIN`/`STD`/`BOLD`. |

> **Action:** the stroke axis is the cleanest replacement target — every
> common width already has a token. Pure mechanical refactor.

---

## 4. Font Sizes

### 4.1 `egui::FontId::monospace(N)` (raw painter text — no `RichText`)

| Value | Count | Likely purpose | Existing token | Notes |
|------:|------:|----------------|----------------|-------|
| `7.0` | 107 | Tick labels, axis labels, sparkline labels | none — `FONT_XS = 8.0` | **By far the largest cluster.** Strong case for new `FONT_2XS = 7.0`. Heavy use in `gpu.rs` axis painting. |
| `9.0` |  53 | Inline DOM/tape rows, micro-buttons | none | Propose `FONT_XS_PLUS = 9.0` or absorb into `FONT_XS`. |
| `8.0` |  40 | Badge/pill/footer | `FONT_XS` (8.0) ✅ | Replace with `font_xs()`. |
| `10.0`|  39 | Compact body labels | `FONT_SM` (10.0) ✅ | Replace with `font_sm()`. |
| `6.0` |  35 | Footnotes, dense overlays | none | New `FONT_3XS = 6.0`. |
| `7.5` |  19 | Axis half-step | none | Consider snapping to 7 or 8. |
| `8.5` |  12 | — | none | Drift — snap to 8. |
| `9.5` |   9 | — | none | Drift — snap to 9 or 10. |
| `13.0`|   6 | Header titles | `FONT_XL` ✅ | Replace. |
| `14.0`|   5 | Big headers | `FONT_2XL` ✅ | Replace. |
| `11.0`|   2 | Body | `FONT_MD` ✅ | Replace. |

### 4.2 `egui::FontId::proportional(N)` — used for icons, glyphs, headers

| Value | Count | Purpose |
|------:|------:|---------|
| `24.0`| 4 | Empty-state large glyph |
| `20.0`| 4 | Section title proportional |
| `18.0`| 4 | Modal title |
| `14.0`| 4 | Body proportional |
| `13.0`| 4 | Body proportional |
| `12.0`| 4 | Body proportional |
| `28.0`| 3 | Empty-state heading |
| `16.0`| 3 | Sub-heading |
| `11.0`| 3 | Inline label |
| `32.0`| 2 | Big icon (`dashboard_pane.rs:34`) |
| `22.0`, `42.0`, `56.0`, `34.0`, `8.0`, `8.5`, `7.0`, `9.0` | 1-3 | one-offs |

> **Action:** introduce a parallel `DISPLAY_*` size scale for proportional
> headings (e.g. `DISPLAY_SM = 18`, `DISPLAY_MD = 24`, `DISPLAY_LG = 32`),
> distinct from the monospace `FONT_*` scale.

### 4.3 In-style.rs internal drift

`style.rs` itself contains hardcoded sizes that should consume its own tokens:

- `style.rs:170` `monospace().size(12.0)` in `tb_btn` → should be `font_lg()`.
- `style.rs:326` `section_label` is `size(7.0)` — but doc says "FONT_SM bold".
  Mismatch with intent.
- `style.rs:399` segmented button: `size(12.0)` literal → `font_lg()`.
- `style.rs:462` `panel_header`: `size(11.0)` → `font_md()`.
- `style.rs:464` subtitle: `size(9.0)` → no token.
- `style.rs:606` `action_btn`: `size(9.0)` → no token.
- `style.rs:621` `trade_btn`: `size(11.0)` → `font_md()`.

Fixing these alone cleans up many style-helper internals.

---

## 5. Hardcoded Colors

This is the noisiest category. Most should be theme-pulled (`t.bull`, `t.bear`,
`t.accent`, `t.text`, `t.dim`, `t.warning`, etc.).

### 5.1 Brand / status colors that should be theme tokens

| Literal | Count | Should be |
|---------|------:|-----------|
| `from_rgb(255, 191, 0)` | 62 | `t.warning` / `t.amber` (orange amber — used for ACTIVE alerts, draft status) |
| `from_rgb(74,158,255)` | 28 | `t.info` / `t.accent_blue` |
| `from_rgb(255, 193, 37)` | 17 | duplicate amber (drift from 191,0) — collapse to one token |
| `from_rgb(46, 204, 113)` | 13 | `t.bull` / success-green |
| `from_rgb(56, 203, 137)` | 11 | drift; collapse into bull |
| `from_rgb(230, 70, 70)`  | 11 | `t.bear` / error-red |
| `from_rgb(224,85,96)` / `(224, 85, 96)` | 17 (combined) | drift bear-red; collapse |
| `from_rgb(80, 200, 120)` | 8  | drift bull; collapse |
| `from_rgb(224, 82, 82)` | 8 | drift bear |
| `from_rgb(231, 76, 60)` | 5 | drift bear |
| `from_rgb(180, 100, 255)` | 5 | `t.purple` / AI accent |
| `from_rgb(240, 170, 70)` | 4 | drift amber |
| `from_rgb(230, 186, 57)` | 4 | drift amber |
| `from_rgb(210, 210, 220)` | 4 | `t.text` / light grey |
| `from_rgb(100, 200, 255)` | 4 | cyan info |
| `from_rgb(100, 140, 255)` | 4 | indigo info |

> **High-priority cleanup:** the bear-red family alone has at least 5 distinct
> RGB triples (`(230,70,70)`, `(224,85,96)`, `(224,82,82)`, `(231,76,60)`,
> plus tuples-with-alpha). Same for bull-green and amber. **Add `t.amber`,
> `t.info_blue`, `t.purple` to the Theme** and collapse all variants.

### 5.2 Alpha-only colors (black/white veils)

| Literal | Sites | Suggestion |
|---------|-------|-----------|
| `from_black_alpha(140)` | `command_palette.rs:286` (modal scrim) | `SCRIM_ALPHA = 140` |
| `from_black_alpha(80)`  | `style.rs:232` (dialog shadow) | `SHADOW_ALPHA` (60) is close; consider `SHADOW_ALPHA_HEAVY = 80` |
| `from_white_alpha(180)` | 5 sites | `WHITE_OVERLAY_STRONG` |
| `from_rgba_unmultiplied(255, 255, 255, 10)` | `style.rs:188` (bevel) | `BEVEL_ALPHA = 10` |
| `from_rgba_unmultiplied(0, 0, 0, 18)` | `chart_widgets.rs:130, 142` | `STRIP_SHADOW_ALPHA` |
| `from_rgba_unmultiplied(0, 0, 0, a)` for `a in [20,12,4]` | `style.rs:307` | already a `dt` token — keep |

### 5.3 Theme-derived (already correct pattern)

Sites using `from_rgba_unmultiplied(t.toolbar_bg.r(), …)` (17), `t.bull.r()` (13),
`t.bear.r()` (13), `t.accent.r()` (8) are correct in spirit but should use the
existing `color_alpha(c, alpha)` helper from `style.rs:540` rather than
hand-rolling the call. Mass-replace target.

### 5.4 Hardcoded dialog backgrounds

- `style.rs:208` — `from_rgb(26, 26, 32)` for `dialog_window` fill. Should be
  `t.dialog_bg` (new theme field) or `t.toolbar_bg.gamma_multiply(0.9)`.
- `watchlist_panel.rs:272, 1505` — `from_rgb(28, 28, 34)` for popup fill.
  Different value than dialog (drift). Same theme-token target.
- `style.rs:209` — `from_rgba_unmultiplied(60, 60, 70, 80)` default border.
  Should be `color_alpha(t.toolbar_border, 80)`.

---

## 6. Component Dimensions (`min_size` / `allocate_exact_size`)

### 6.1 Square icon/button hit targets

| Size | Count (min_size + allocate) | Purpose |
|------|----------------------------:|---------|
| `(14.0, 14.0)` | 21 + 1 | Small inline icon (logo, chevron) |
| `(16.0, 16.0)` | 11 + 3 | Standard checkbox / toggle |
| `(18.0, 18.0)` | 10 + 1 | Avatar bubble (`discord_panel.rs:475`) and toolbar icon |
| `(20.0, 20.0)` | 7 + 1   | Toolbar button |
| `(12.0, 12.0)` | 0 + 2   | Mini swatch (`indicator_editor.rs:416, 465`) |

> **Suggestion:** add a `BTN_ICON_SM = vec2(14,14)`, `BTN_ICON_MD = vec2(16,16)`,
> `BTN_ICON_LG = vec2(18,18)`, `BTN_ICON_XL = vec2(20,20)` token set, or a
> single `square(size)` helper.

### 6.2 Variable-width fixed-height action buttons (`vec2(0.0, H)`)

| Height | Count | Purpose | Existing token |
|-------:|------:|---------|----------------|
| `20.0` | 9 | `action_btn` (`style.rs:608`), `gpu.rs` actions | propose `BTN_H_SM = 20.0` |
| `16.0` | 8 | `small_action_btn` (`style.rs:644`) | propose `BTN_H_XS = 16.0` |
| `22.0` | 7 | `gpu.rs` trade rows, `style.rs:567` (badge default) | propose `BTN_H_MD = 22.0` |
| `18.0` | 3 | `gpu.rs` toolbar pills | |
| `24.0` | 2 | `tb_btn` (`style.rs:172`), `trade_btn` (`style.rs:622`) | propose `BTN_H_LG = 24.0` |

### 6.3 Pill / badge specifics

| File:line | Size | What |
|-----------|------|------|
| `apex_diagnostics.rs:69`  | `vec2(60.0, 14.0)` | Diagnostics pill |
| `apex_diagnostics.rs:199` | `vec2(62.0, 14.0)` | drift — same purpose |
| `command_palette.rs:330`  | `vec2(68.0, 18.0)` | Category badge |
| `gpu.rs:3655` | `vec2(14.0, 14.0)` | Header logo |
| `gpu.rs:4663` | `vec2(32.0, 24.0)` | Tab pill |
| `gpu.rs:4728` | `vec2(20.0, 20.0)` | Inline icon |
| `object_tree.rs:330,380,649` | `vec2(8 or 10, 16 or 18)` | Tree dot |

### 6.4 Editor mini swatches

`indicator_editor.rs` uses `vec2(22.0, 18.0)` (4×), `vec2(24.0, 12.0)` (2×),
`vec2(24.0, 22.0)` (2×). These are color-swatch buttons; consider
`SWATCH_SM = vec2(22, 18)` and `SWATCH_LG = vec2(24, 22)`.

---

## 7. Drift / Inconsistency Summary

The styling overhaul should at minimum address these clusters where the *same
visual intent* is implemented with multiple distinct values:

| Cluster | Distinct values | Recommended single token |
|---------|-----------------|--------------------------|
| Bear red | `230,70,70`, `224,85,96`, `224,82,82`, `231,76,60`, `224, 85, 96` | `t.bear` |
| Bull green | `46,204,113`, `56,203,137`, `80,200,120` | `t.bull` |
| Amber/warning | `255,191,0`, `255,193,37`, `240,170,70`, `230,186,57` | `t.warning` |
| Dialog dark bg | `26,26,32`, `28,28,34` | `t.dialog_bg` |
| Tiny font | `6.0`, `6.5`, `7.0`, `7.5`, `8.0`, `8.5` | three tokens: `FONT_3XS=6`, `FONT_2XS=7`, `FONT_XS=8` (snap halves to nearest) |
| Stroke 1.0 vs 1.2 | both used as "standard border" | `STROKE_STD=1.0` (drop 1.2) |
| Tiny radii 2.0 / 3.0 | both used for "compact" buttons | introduce `RADIUS_XS=2.0` and `RADIUS_2XS=3.0`, or pick one |

---

## 8. Files Surveyed

`chart_renderer/gpu.rs` (largest by far, ~half of all hits),
`chart_renderer/compute.rs`,
`chart_renderer/types.rs`,
`chart_renderer/trading/order_manager.rs`,
and the entire `chart_renderer/ui/` tree (47 files including `style.rs`,
`alerts_panel.rs`, `chart_widgets.rs`, `command_palette.rs`,
`apex_diagnostics.rs`, `dashboard_pane.rs`, `discord_panel.rs`,
`heatmap_pane.rs`, `hotkey_editor.rs`, `indicator_editor.rs`,
`indicators.rs`, `object_tree.rs`, `overlay_manager.rs`,
`plays_panel.rs`, `spread_panel.rs`, `watchlist_panel.rs`, etc.).

## 9. Top-Priority Refactors (mechanical wins)

1. **Stroke widths** — 295+ sites, every common value has a token. Pure search-and-replace (`Stroke::new(1.0,` → `Stroke::new(stroke_std(),`).
2. **`add_space`** — 350+ sites; ~95% of values map to `gap_*()` tokens.
3. **Color drift collapse** — eliminate the bull/bear/amber RGB variants.
4. **`corner_radius(4.0)` / `(6.0)` / `(12.0)`** — already have `RADIUS_SM/MD/LG`; just use them.
5. **Add new tokens:** `RADIUS_XS = 2.0`, `RADIUS_2XS = 3.0`, `FONT_2XS = 7.0`, `FONT_3XS = 6.0`, `STROKE_MED = 0.8`, `BTN_H_XS/SM/MD/LG = 16/20/22/24`.
6. **Theme additions:** `t.amber`, `t.info_blue`, `t.purple`, `t.dialog_bg`.

After (1)-(4) the file `style.rs` itself should be edited to consume its own
tokens (see §4.3) — currently it contains the same drift it tries to prevent.
