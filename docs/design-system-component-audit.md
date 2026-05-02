# Design System Component Audit
**Date:** 2026-04-30  
**Scope:** `src-tauri/src/chart_renderer/ui/widgets/`, `components/`, `components_extra/`  
**Auditor:** Claude Sonnet 4.6 (read-only, no source edits)

---

## Executive Summary

**Overall Consistency Tier: 3.5 / 5**

The codebase has a genuine, well-designed token system (`style::*`, `StyleSettings`, `InteractionTokens`, `Size/Radius/Density`, `TextStyle`, the four Foundation shells). However, only a minority of widgets fully exploit it. The main pathologies are:

1. **Parallel resolution paths** — every widget family has a "foundation path" (uses `InputShell`/`ButtonShell`/`CardShell`/`RowShell`) and a "legacy fallback path" (hand-rolled `egui::Frame`/`egui::Button` with inline literals). Both paths coexist in the same widget. The fallback is reached whenever a caller passes `palette()` instead of `theme()`. This is the largest single source of inconsistency.
2. **Hardcoded pixel constants in hot paths** — `WatchlistRow`, `DomRow`, `NewsRow`, `MetricCard`, and all six `StatusBadge`/`NotificationBadge`/`Skeleton` widgets contain literal `f32` sizes that are never threaded through `font_*()` or `gap_*()`.
3. **Duplicate ALPHA_* constants** — `style.rs` exports both `ALPHA_MUTED = 40` (const) and `alpha_muted() -> u8` (fn). `WatchlistRow` and `NewsRow` use the raw uppercase constants (`ALPHA_GHOST`, `ALPHA_ACTIVE`, `ALPHA_MUTED`, `ALPHA_STRONG`) instead of the token functions. These shadow each other and create silent disagreements when the token function is overridden at runtime via `dt_u8!`.
4. **Missing `TextStyle` adoption in several families** — `PanelHeader`, `DialogHeader`, `SectionLabel` (non-Sm variants), `TabBar`/`TabStrip`/`TabBarWithClose`, and `Toast` all construct `RichText` manually with raw `font_*()` calls rather than routing through `TextStyle::as_rich()`.
5. **`IconBtn` size hardcodes** — `.small()` = 11.0, `.medium()` = 14.0, `.large()` = 18.0 are literal floats inside `buttons.rs`; none are driven by `font_*()` tokens.
6. **`TradeBtn` underline stroke** — literal `Stroke::new(1.0, color)` at line 154; every other underline uses `current().stroke_bold`.
7. **`PopupFrame` shadow is fully hardcoded** — `offset: [0, 8]`, `blur: 24`, `spread: 1`, `color: from_black_alpha(40)` at `frames.rs:281–285`. `CardFrame` and `DialogFrame` use `st.shadow_*` knobs; `PopupFrame` ignores them.
8. **`KeybindChip` corner radius cast** — `CornerRadius::same(st.r_xs as u8)` at `pills.rs:469` silently truncates a `f32` to `u8`. All other tokens use `Radius::Xs.corner()` from `foundation/tokens.rs` which calls `CornerRadius::same(st.r_xs)` — the correct `u8`-safe path.
9. **`NotificationBadge` font size** — literal `7.5` at `status.rs:604`. Should use `font_xs()` = 8.0.
10. **`StatusBadge` uses `crate::dt_f32!(badge.font_size, 8.0)` and `crate::dt_f32!(badge.height, 16.0)`** — this is correct token usage but the default `8.0` disagrees with `font_xs()` = 8.0 (matches), while `16.0` height is not tied to `btn_compact_height()`.

---

## Foundation Layer (Reference)

### `foundation/tokens.rs`
| Item | API | Token usage | Tier |
|------|-----|-------------|------|
| `Size` enum (Xs/Sm/Md/Lg/Xl) | `.height()`, `.padding()`, `.font()` | All three delegate to `current()` fields or `gap_*()` / `font_*()` / `btn_*_height()`. Xs.height = literal `16.0` and Xs.padding.x = `gap_sm()` — only `16.0` is hardcoded. | 4 |
| `Density` enum | `.vscale()` | Literal floats `0.65 / 1.0 / 1.4` — acceptable since these are pure scale multipliers with no design-token mapping. | 4 |
| `Radius` enum | `.corner()` | Fully routed through `current().r_*`. | **5** |

**Inconsistency:** `Size::Xs.height()` returns literal `16.0` (not a token). All other `Size` heights use style knobs.

---

### `foundation/text_style.rs`
| TextStyle | Size source | Tier |
|-----------|-------------|------|
| `Display` | `font_2xl() + 4.0` — literal `+4.0` offset | 4 |
| `HeadingLg/Md`, `BodyLg`, `BodySm`, `MonoSm`, `NumericLg` | `font_*()` tokens | **5** |
| `Body`, `Caption`, `Mono`, `Numeric`, `Label`, `Eyebrow` | `st.font_body`, `st.font_caption`, `st.font_section_label` | **5** |
| `NumericHero` | Literal `30.0` | 2 |

**Inconsistency:** `NumericHero` at `30.0` is not a token. The `Display` `+4.0` addend is reasonable but could be a StyleSettings knob.

---

### `foundation/interaction.rs`
Fully tokenized. All alpha values read from `current()` or `InteractionTokens` fields. **Tier 5.**

### `foundation/variants.rs`
Fully tokenized. Every color derives from `t.*` fields and `alpha_*()` functions. **Tier 5.**

### `foundation/shell.rs`
| Shell | Tier | Notes |
|-------|------|-------|
| `ButtonShell` | **5** | Uses `Size`, `Radius`, `ButtonVariant`, `InteractionTokens`, `TextStyle::Body`. One cosmetic literal: `1.0` at `UnderlineActive` y-offset (same as tricky `resp.rect.bottom() - 1.0` geometry, not a token concern). |
| `RowShell` | **5** | `painter_mode` defaults height to `style_row_height()`. Everything else routed through Foundation. |
| `CardShell` | 4 | `neutral_fg = Color32::from_gray(220)`, `neutral_dim = from_gray(150)`, `neutral_bg = from_gray(28)`, `neutral_border = from_gray(60)` in the themeless fallback path. These are documented fallbacks so tier 4 is fair. Shadow uses `shadow_offset()/shadow_spread()/shadow_alpha()`. |
| `InputShell` | **5** | All state-dependent borders use `alpha_strong()/alpha_muted()`. Stroke widths use `stroke_bold()/stroke_thin()`. |
| `ChipShell` | 4 | Uses `stroke_thin()` uniformly — could switch to `stroke_hair()` in `Subtle` variant to match pill aesthetics elsewhere. Otherwise fully tokenized. |

---

## Buttons Family

### `buttons.rs`

#### `IconBtn`
- **Tier: 3**
- `.small()` = `11.0`, `.medium()` = `14.0`, `.large()` = `18.0` — **all hardcoded literals** (should be `font_sm()/font_md()/font_lg()`).
- Default fallback color `Color32::from_rgb(120, 120, 130)` (not a theme field).
- `min_size: side = (size + 8.0).max(22.0)` — literal `8.0` addend and `22.0` floor. `22.0` happens to equal `btn_small_height()` but is not read from the token.
- Hover overlay correctly uses `radius_sm()`, `alpha_ghost()`, `stroke_thin()`, `alpha_muted()`. **Tokens used partially.**
- **Drift vs. ButtonShell**: `ButtonShell` reads size from `Size::*` enum; `IconBtn` reads hardcoded floats. Font sizes diverge at runtime when design tokens change.

#### `TradeBtn`
- **Tier: 3**
- `bright` via `dt_f32!(button.trade_brightness, 0.55)` — correct.
- `h = btn_trade_height()` — correct.
- `Stroke::new(1.5, border)` in `OutlineAccent` — should be `stroke_bold()`.
- `Stroke::new(1.0, color)` underline at line 154 — **literal**. Every other underline in the family (`SimpleBtn:233`, `SmallActionBtn:306`, `ActionBtn:437`) uses the same literal `1.0`. The `ButtonShell` uses `current().stroke_bold`. **All four buttons should use `stroke_bold()`.**
- `r_xs()`, `r_sm_cr()`, `r_md_cr()` used correctly — but `current().r_md` (raw field, not CornerRadius) used at line 140 (`rect_filled(resp.rect, current().r_md, color)`) — this is wrong; `current().r_md` is `f32` and `rect_filled` expects `CornerRadius`. Silently coerced via `impl Into<CornerRadius>`.

#### `SimpleBtn`
- **Tier: 3**
- `Stroke::new(1.5, ...)` in `OutlineAccent` — should be `stroke_bold()`.
- `Stroke::new(1.0, color)` underline — should be `stroke_bold()`.
- `r_xs()`, `r_sm_cr()`, `r_md_cr()` — correct.
- Default color `Color32::from_rgb(120, 120, 130)` — not a theme field.

#### `SmallActionBtn`
- **Tier: 3**
- Same issues as `SimpleBtn`. `Stroke::new(1.5, ...)` and `Stroke::new(1.0, ...)` literals.
- `font_sm().strong()` via `RichText::new(...).monospace().size(font_sm()).strong()` — bypasses `TextStyle`.

#### `ChromeBtn`
- **Tier: 2**
- Intentionally "escape hatch" — caller supplies pre-styled `RichText` and explicit colors. No token usage at all except `set_cursor_icon`. Acceptable by design but tier 2 by measurement.
- **Note:** `.padding` field stored but not applied (comment at line 359 explains egui limitation).

#### `ActionBtn`
- **Tier: 3**
- `Stroke::new(0.5, ...)` and `Stroke::new(1.5, ...)` literals — should be `stroke_hair()/stroke_bold()`.
- `Stroke::new(1.0, ...)` underline — should be `stroke_bold()`.
- `btn_simple_height()` — correct token usage.

**Reference widget in Buttons family:** `ButtonShell` (foundation/shell.rs) — **Tier 5**. All five legacy button builders should migrate onto it.

**Recommended unification (Buttons):**
1. Replace `IconBtn`'s `.small/.medium/.large` literals with `font_sm()/font_md()/font_lg()`.
2. Replace all `Stroke::new(1.0, color)` underline strokes with `Stroke::new(current().stroke_bold, color)`.
3. Replace `Stroke::new(1.5, ...)` with `stroke_bold()`.
4. Replace `Stroke::new(0.5, ...)` with `stroke_hair()`.
5. Have `TradeBtn`/`SimpleBtn`/`SmallActionBtn`/`ActionBtn` compose `ButtonShell` for their common paint logic.

---

## Pills/Chips Family

### `widgets/pills.rs`

#### `PillButton`
- **Tier: 4**
- Correctly maps to `ChipVariant::Solid`/`Subtle`. Colors via `alpha_muted()/alpha_active()/alpha_dim()`. Radius via `Radius::Pill.corner()`. Height literal `18.0` — should be a token (compare `StatusBadge` using `dt_f32!(badge.height, 16.0)`). `font_sm()` correct.
- `pad_x = gap_md()` correct; `pad_y = prev_pad_y` (inherits egui button padding) — acceptable.

#### `BrandCtaButton`
- **Tier: 3**
- Heights `24.0/32.0/40.0` for Sm/Md/Lg — **hardcoded literals**. Should map through `Size::Sm/Md/Lg.height()`.
- `gap_xl()` for x-padding — correct.
- `Radius::Md.corner()` — correct.
- `stroke_thin()` — correct.
- Hover overlay `color_alpha(Color32::WHITE, 12)` — literal alpha `12` bypasses `alpha_ghost()` = 15.

#### `RemovableChip`
- **Tier: 3**
- `font_sm()` — correct. `stroke_thin()` — correct. `gap_md()` — correct.
- `egui::CornerRadius { nw: 99, sw: 99, ne: 0, se: 0 }` and `{ nw: 0, sw: 0, ne: 99, se: 99 }` — **hardcoded literal `99`** for pill radius. Should be `current().r_pill.min(u8::MAX as f32) as u8` like `SearchPill` does.
- Min size `18.0` — hardcoded. Should use `Size::Sm.height()` = `btn_small_height()`.

#### `DisplayChip`
- **Tier: 4**
- `alpha_tint()/alpha_dim()` — correct. `Radius::Pill.corner()` — correct. `font_xs()` — correct. Height `14.0` — hardcoded (acceptable for non-interactive chip but should be `Size::Xs.height()`).

#### `StatusBadge`
- **Tier: 4**
- `dt_f32!(badge.font_size, 8.0)` and `dt_f32!(badge.height, 16.0)` — runtime design-token knobs. `Radius::Pill.corner()` — correct. `alpha_subtle()` — correct.
- `s.uppercase_section_labels` checked for case — correct style-setting usage.
- Spacing: inherits button_padding without reset (no `prev_pad` guard), unlike `BrandCtaButton` and `KeybindChip`.

#### `KeybindChip`
- **Tier: 3**
- `CornerRadius::same(st.r_xs as u8)` — **truncating cast** from `f32` to `u8`. Should use `Radius::Xs.corner()`.
- `st.stroke_std/stroke_thin` — correct.
- `alpha_strong()/alpha_muted()` — correct.
- Height `14.0` — hardcoded.

**Reference widget:** `ChipShell` (foundation). `DisplayChip` is closest at Tier 4.

**Recommended unification (Pills):**
1. Fix `RemovableChip`'s `99` radius literals → `current().r_pill as u8`.
2. Fix `KeybindChip` cast → `Radius::Xs.corner()`.
3. Replace hardcoded heights (`14.0`, `18.0`, `24.0/32.0/40.0`) with `Size::Xs/Sm/Md/Lg.height()`.
4. Fix `BrandCtaButton` hover alpha literal `12` → `alpha_ghost()`.

---

## Text Family

### `widgets/text.rs`

All eight text widgets (`PaneTitle`, `Subheader`, `BodyLabel`, `MutedLabel`, `CaptionLabel`, `MonospaceCode`, `NumericDisplay`, `SectionLabel`, `DimLabel`) route through `TextStyle::as_rich()` for their primary render path. **Tier: 4–5.**

#### Inconsistencies:
- **`SectionLabel` non-Sm variants** (Tiny, Xs, Md, Lg): fallback to raw `RichText::new(s).monospace().size(7.0/font_xs()/font_md()/font_lg()).strong()` — bypasses `TextStyle`. Only `SectionLabelSize::Sm` uses `TextStyle::Label`.
  - `Tiny` uses literal `7.0` — sub-`font_xs()`. This is intentional (legacy match) but not in `TextStyle`.
- **`MonospaceCode::Xs`**: manually overrides to `font_xs()` with raw `RichText` because `TextStyle::MonoSm` resolves to `font_sm()` (not `font_xs()`). This hack is documented inline but means `Xs` bypasses the TextStyle path.
- **`NumericDisplay::Lg`**: manually overrides to `font_lg()` because `TextStyle::Numeric` uses `st.font_body`. Documented but creates a TextStyle escape hatch.
- **Default colors are hardcoded**: all constructors use `Color32::from_rgb(...)` as defaults. Callers are expected to pass `.color(t.text)` / `.color(t.dim)`. This is correct by design but means theme-less instantiation gives wrong colors silently.

**Reference widget:** `BodyLabel` — Tier 5. Routes through `TextStyle::Body.as_rich()` cleanly.

---

## Frames Family

### `widgets/frames.rs`

| Frame | Tier | Notes |
|-------|------|-------|
| `PanelFrame` | **5** | `gap_xl()/gap_lg()`, `r_md_cr()`, `stroke_std`, `alpha_heavy()`. Shadow absent (panels don't shadow). |
| `CardFrame` | **5** | `st.card_padding_y/x`, `r_md_cr()`, `stroke_std/thin`, `alpha_strong()/muted()`, `st.shadow_*`. |
| `DialogFrame` | **5** | `r_lg_cr()`, `gap_xl()`, `stroke_std/thin`, `alpha_strong()`, `st.shadow_*` with `+40`/`*1.2` scaling. |
| `PopupFrame` | **2** | Shadow hardcoded: `offset: [0, 8]`, `blur: 24`, `spread: 1`, `alpha: 40` — ignores `st.shadow_offset_y`, `st.shadow_blur`, `st.shadow_alpha`. All siblings use style knobs. |
| `SidePanelFrame` | **5** | `stroke_std/thin`, `alpha_strong()`, `r_md_cr()`. No shadow (correct for side panels). |
| `TooltipFrame` | 4 | `dt_f32!(tooltip.corner_radius, 8.0)` and `dt_f32!(tooltip.padding, 8.0)` — design-token knobs, correct. `alpha_strong()` — correct. `stroke_std/thin` — correct. Corner radius `0` in hairline mode — intentional. |
| `CompactPanelFrame` | **5** | `gap_lg()/gap_md()`, `r_sm_cr()`, `stroke_std`, `alpha_heavy()`. |

**Critical bug: `PopupFrame` shadow** — `frames.rs:281–285`:
```rust
frame = frame.shadow(egui::epaint::Shadow {
    offset: [0, 8],   // should be [0, st.shadow_offset_y as i8]
    blur:   24,       // should be st.shadow_blur as u8
    spread: 1,        // should be 0 (CardFrame uses 0)
    color:  Color32::from_black_alpha(40), // should be from_black_alpha(st.shadow_alpha)
});
```
All other frames correctly use `st.shadow_*`. This causes `PopupFrame` shadows to not respond to the style inspector's shadow knobs.

---

## Headers Family

### `widgets/headers.rs`

#### `PanelHeader`
- **Tier: 2**
- `RichText::new(title_text).monospace().size(11.0).strong().color(accent)` — **literal `11.0`** bypasses `font_md()` = 11.0 (numerically equal but not token-linked). Should use `TextStyle::Label.as_rich()` or `font_md()`.
- No `TextStyle` usage.

#### `PanelHeaderWithClose`
- **Tier: 3**
- Delegates to `style::panel_header_sub` — inherits that function's token usage (not audited inline here; assumed to use `font_sm()/style_label_case()`).

#### `DialogHeader`
- **Tier: 3**
- `darken` via `dt_u8!(dialog.header_darken, 8)` — correct design-token usage.
- `Margin { left: 12, right: 10, top: 10, bottom: 10 }` — **four hardcoded literals**. Should use `gap_lg()/gap_md()` variants.
- `RichText::new(title_text).monospace().size(font_lg()).strong()` — bypasses `TextStyle`. Should be `TextStyle::HeadingMd.as_rich()` or `TextStyle::Label`.
- `s.r_lg as u8` — same truncating-cast issue as `KeybindChip` (though `r_lg` is typically small enough to fit in u8).
- `stroke_std`, `alpha_muted()` — correct.

#### `PaneHeader`
- **Tier: 3**
- Hardcoded defaults: `title_color: from_rgb(120, 140, 220)`, `bg: from_rgb(20, 20, 28)`, `border: from_rgb(50, 50, 60)`, `height: 28.0`.
- Delegates to `components::pane_header_bar` + `section_label_widget` — both use token functions.
- The `28.0` default height is not driven by a token; should use `style_tab_height()` or a dedicated `pane_header_height()` token.

#### `PaneHeaderWithClose`
- **Tier: 3** (inherits `components::panel_header`).

#### `DialogHeaderWithClose`
- **Tier: 3** (delegates to `style::dialog_header`).

**Reference widget:** `PanelHeaderWithClose` and `DialogHeaderWithClose` — they delegate entirely to legacy functions that use tokens. `DialogHeader` is the worst offender.

**Recommended unification (Headers):**
1. Replace `PanelHeader`'s literal `11.0` with `font_md()` or `TextStyle::Label`.
2. Replace `DialogHeader`'s margin literals with `gap_lg()/gap_md()`.
3. Replace `DialogHeader`'s `RichText` construction with `TextStyle::HeadingMd.as_rich()`.
4. Replace `PaneHeader`'s `28.0` default with a token (`style_tab_height()` + offset or dedicated `pane_header_height()`).

---

## Tabs Family

### `widgets/tabs.rs`

#### `TabBar`
- **Tier: 3**
- In knob-override path: `font_lg()` default — correct. `style_tab_height()` as min height — correct. `dt_f32!(tab.underline_thickness, 2.0)` — design-token knob.
- `Color32::TRANSPARENT` fill, `Stroke::NONE` stroke for frameless buttons — correct.
- `tab_ul` underline painted as `rect_filled(..., 0.0, ...)` — literal corner radius `0.0`. Should be `Radius::None.corner()`.

#### `TabStrip`
- **Tier: 3**
- `font_md()`, `gap_md()`, `alpha_tint()`, `r_pill()`, `st.hairline_borders`, `st.stroke_std` — all tokenized.
- `min_size: Vec2::new(0.0, 20.0)` — **literal `20.0`** in two places. Should use `Size::Sm.height()` = `btn_small_height()`.

#### `TabBarWithClose`
- **Tier: 3**
- Same issues as `TabStrip`. `min_size: 18.0` — hardcoded. `font_sm()`, `alpha_tint()`, `r_pill()` — correct.
- `gap_xs()` item spacing — correct.

**Reference widget:** `TabBar` (with knob path active) is closest to ideal.

**Recommended unification (Tabs):**
1. Replace `TabStrip`/`TabBarWithClose` height literals with `Size::Sm.height()`.
2. Replace `TabBar` underline `0.0` corner with `Radius::None.corner()`.

---

## Rows Family

### `widgets/rows/watchlist_row.rs` — `WatchlistRow`
- **Tier: 2**
- Correctly uses `RowShell::painter_mode` — the shell layer is fully tokenized (Tier 5).
- **Inside the painter body:** extensive hardcoded literals:
  - `2.5` active-stripe width, `1.0` corner — not tokens.
  - `4.0/3.0/2.0` RVOL strip widths — not tokens.
  - `Color32::from_rgba_unmultiplied(46, 204, 113, ALPHA_GHOST)` for bull tint — uses `ALPHA_GHOST` constant, not `alpha_ghost()` function. Same for `ALPHA_ACTIVE`, `ALPHA_MUTED`, `ALPHA_STRONG`.
  - `Color32::from_rgb(255, 193, 37)` for earnings pill — not a theme field.
  - `egui::FontId::monospace(7.0)` — literal size, not a token.
  - `6.0` earnings pill corner radius, `12.0` height — hardcoded.
  - `Stroke::new(1.0, color_alpha(border, ALPHA_MUTED))` — literal `1.0` stroke.
  - `Stroke::new(2.0, ...)` for range bar and 52-week bar — not tokenized.
  - `egui::FontId::proportional(9.0)` for drag-handle and star icons — literal.
  - `Stroke::new(STROKE_THIN, ...)` for separator — uses CONST instead of `stroke_thin()`.

#### `DomRow`
- **Tier: 2** (same pattern — painter mode body contains extensive hardcoded geometry).

#### `NewsRow`
- **Tier: 3** — uses `RowShell::painter_mode`, literal sizes in body but generally lighter.

**Recommended unification (Rows):**
1. Move all RVOL/earnings/alert hardcoded colors into theme fields or named constants.
2. Replace `ALPHA_*` constant usage with `alpha_*()` function calls throughout all painter bodies.
3. Replace literal font sizes (`7.0`, `9.0`) with `font_xs()` or new `font_2xs()` token.
4. Replace literal stroke widths in painter bodies with `stroke_thin()/stroke_std()`.

---

## Cards Family

### `widgets/cards/metric_card.rs` — `MetricCard`
- **Tier: 4**
- Migrated onto `CardShell`. `gap_lg()` — correct. `font_xs()/font_xl()/font_sm()` — correct. `RichText::new(...).monospace().size(font_*())` — bypasses `TextStyle` but uses tokens. Near-ideal.

### `widgets/cards/signal_card.rs` — `SignalCard`
- **Tier: 4**
- Migrated onto `CardShell`. `TextStyle::Numeric` for title — correct. `font_xl()/font_md()/font_sm()` — correct. `radius_sm() as u8` corner radius at `status.rs:411` — correct path. `stroke_thin()/stroke_std()` — correct. `alpha_muted()` — correct.
- `add_space(gap_xs())` — correct.

### Other card files (`earnings_card.rs`, `event_card.rs`, `news_card.rs`, `play_card.rs`, `playbook_card.rs`, `stat_card.rs`, `trade_card.rs`)
- Pattern mirrors `MetricCard`/`SignalCard` (all migrated onto `CardShell`). Assumed Tier 4.

---

## Inputs Family

### `widgets/inputs.rs`

#### `TextInput`
- **Tier: 4**
- Foundation path (`.theme()` called): uses `InputShell` — Tier 5.
- Fallback path (`.palette()` called): `Stroke::new(1.0, border_color)` — **literal `1.0`**, should be `stroke_thin()`. `radius_sm()` — correct. `gap_sm()` — correct.
- `font_size` defaults to `font_sm()` — correct.

#### `NumericInput`
- **Tier: 3**
- Delegates to `TextInput` via `.palette()` — always uses the fallback path (no `.theme()` call). Tier 3 by inheritance.

#### `Stepper`
- **Tier: 4**
- `CornerRadius::same(st.r_xs as u8)` — same truncating-cast issue as `KeybindChip`. Should use `Radius::Xs.corner()`.
- `stroke_std/thin`, `alpha_strong()/muted()` — correct. `gap_xs()`, `font_xs()/font_sm()` — correct.

#### `CompactStepper`
- **Tier: 4** — same as `Stepper`.

#### `ToggleRow`
- **Tier: 3**
- `font_sm()`, `style_label_case()` — correct. Checkbox styled by egui visuals not reset — acceptable.
- No `TextStyle` usage for label.

#### `SearchInput`
- **Tier: 4**
- Foundation path uses `InputShell` correctly.
- Fallback: `r_sm_cr()`, `stroke_std/thin`, `alpha_strong()/muted()`, `gap_md()/gap_xs()` — all correct. `avail - 36.0` — literal `36.0` for icon width reservation.

#### `Slider`
- **Tier: 3**
- Sets `visuals.selection.bg_fill` — mutates egui visuals directly, not a token path. Comment notes `_handle_r = st.r_md` is stored but unused (egui Slider doesn't expose handle radius). Label uses raw `RichText`+`font_sm()`.

---

## Select Family

### `widgets/select.rs`

#### `Dropdown`
- **Tier: 3**
- Uses `egui::ComboBox` — minimal token opportunity. `font_sm()`, `alpha_*` — used where applicable. Width default `140.0` — hardcoded.

#### `Combobox`
- **Tier: 3** — same pattern as `Dropdown`.

(Remaining selects: `DropdownOwned`, `DropdownActions`, `MultiSelect`, `Autocomplete`, `SegmentedControl`, `RadioGroup` — not individually read but follow same pattern.)

---

## Status Family

### `widgets/status.rs`

#### `StatusDot`
- **Tier: 3**
- `font_sm()`, `gap_sm()`, `alpha_dim()` — correct.
- `radius: 3.5` default — **hardcoded**. Should be `current().r_xs / 2.0` or similar.
- Label width estimation `s.len() as f32 * font_sm() * 0.6` — character-width approximation with literal `0.6`, unavoidable for pre-measurement.
- Warning color `Color32::from_rgb(241, 196, 15)` hardcoded in both `StatusDot` and `ConnectionIndicator` and `Toast`. Should be a named token or `t.warn` field.

#### `Spinner`
- **Tier: 3**
- `alpha_soft()/alpha_active()` — correct. `LoadSize` px values `10.0/14.0/20.0` — hardcoded. Should map through `font_sm()/font_md()/font_xl()` or new `icon_size_*` tokens.

#### `ProgressBar`
- **Tier: 3**
- `font_xs()` — correct. `h` values `3.0/8.0/6.0` for variants — hardcoded. `stripe_w = 6.0` — hardcoded. `alpha_soft()` — correct.

#### `ProgressRing`
- **Tier: 3**
- `font_xs()` — correct. `stroke_w = (d * 0.10).max(2.0)` — literal factors.

#### `Skeleton`
- **Tier: 2**
- `rounding: 3.0` default — hardcoded. `highlight/base` colors `from_rgb(60,60,70)/from_rgb(110,110,125)` — should use `theme()`. `alpha_subtle()` — correct.

#### `Toast`
- **Tier: 3**
- `stroke_thin()`, `alpha_strong()`, `r_md_cr()`, `gap_lg()` — correct.
- `current().toast_bg_alpha` — correct style-settings usage.
- Accent stripe `Vec2::new(3.0, font_md() + font_sm() + 6.0)` — literal `3.0` width and `6.0` height addend. `CornerRadius::same(2)` — literal.
- `ui.add_space(8.0)` — literal, should be `gap_lg()`.
- `RichText` construction bypasses `TextStyle`.
- Warning color `from_rgb(241, 196, 15)` — hardcoded (third occurrence across status.rs).
- Width default `280.0` — hardcoded.

#### `NotificationBadge`
- **Tier: 2**
- `h = 12.0`, `pad_x = 4.0`, `font size = 7.5` — **all hardcoded**.
- Default color `Color32::from_rgb(231, 76, 60)` — not a theme field.
- `CornerRadius::same((h * 0.5) as u8)` — correct pill calculation but `12.0` base is hardcoded.

#### `ConnectionIndicator`
- **Tier: 3**
- Composes `StatusDot` — inherits its tier.
- `gap_sm()`, `font_xs()`, `alpha_dim()` — correct.
- Warning color `from_rgb(241, 196, 15)` — **fourth hardcoded occurrence** of the same literal.

#### `SearchPill`
- **Tier: 4**
- `current().hairline_borders` check — correct. `rule_stroke_for(...)` — correct helper. `alpha_active()/alpha_strong()` — correct. `current().r_pill.min(8)` — correct (uses `r_pill` but with hardcoded floor `8`). `stroke_thin()` — correct. `font_sm()/font_xs()` — correct.
- Defaults use `from_rgb(...)` not theme fields — correct for a default-only constructor.

#### `TrendArrow`
- **Tier: 4**
- `gap_xs()`, `font_sm()` — correct. Default colors are fallbacks, not hardcodes in usage. Uses unicode glyphs not icon constants — acceptable.

---

## Layout Family

### `widgets/layout.rs`

#### `Splitter`
- **Tier: 4**
- `dt_f32!(split_divider.*)` design-token knobs — correct. `color_alpha(dim, alpha_faint())` — correct. `gamma_multiply(0.6)` — literal factor.

(Remaining layout builders — `EmptyState`, `Stack`, `Cluster`, etc. — delegate to legacy helpers that use tokens.)

---

## Form Family

### `widgets/form.rs`

#### `FormRow`
- **Tier: 3**
- `font_sm()`, `style_label_case()` — correct.
- `label_width: 120.0` default — **hardcoded**.
- No token usage for internal spacing or separators.

---

## Cross-Family Inconsistency Tables

### Font Usage

| Pattern | Widgets | Recommended |
|---------|---------|-------------|
| `TextStyle::as_rich()` | `BodyLabel`, `MutedLabel`, `CaptionLabel`, `PaneTitle`, `Subheader`, `DimLabel`, `ButtonShell`, `RowShell`, `ChipShell`, `SignalCard` | **Gold standard** |
| `font_*()` + raw `RichText` | `SectionLabel` (non-Sm), `MetricCard`, `TabBar/Strip/WithClose`, `PanelHeader`, `DialogHeader`, `TradeBtn`, `SimpleBtn`, `SmallActionBtn`, `ActionBtn`, `Toast`, `ToggleRow` | Migrate to `TextStyle` |
| Literal float sizes | `IconBtn` (11/14/18), `PanelHeader` (11.0), `WatchlistRow` (7.0/9.0/14.0/15.0), `NotificationBadge` (7.5), `Spinner` (10/14/20) | Replace with `font_*()` tokens |

### Padding

| Pattern | Widgets |
|---------|---------|
| `Size::*.padding()` → `Margin` | `ButtonShell`, `RowShell`, `CardShell`, `InputShell`, `ChipShell` |
| `gap_*()` in `Margin` fields | `PanelFrame`, `CardFrame`, `DialogFrame`, `PopupFrame`, `CompactPanelFrame`, `Toast` |
| Literal `i8` Margin values | `DialogHeader` (12/10/10/10), `FormRow` uses default `label_width: 120.0` |

### Borders / Strokes

| Pattern | Widgets |
|---------|---------|
| `stroke_thin()/stroke_bold()/stroke_hair()` | Most frames, shells, most chips |
| Literal `1.0` | `TradeBtn`, `SimpleBtn`, `SmallActionBtn`, `ActionBtn` (underlines); `TextInput` fallback |
| Literal `1.5` | `TradeBtn`, `SimpleBtn`, `SmallActionBtn`, `ActionBtn` (outline strokes) |
| Literal `2.0` | `WatchlistRow` (range bar/week52), `ProgressBar` |

### Corner Radii

| Pattern | Widgets |
|---------|---------|
| `Radius::*.corner()` | `ButtonShell`, `ChipShell`, `RowShell`, `PillButton`, `BrandCtaButton`, `DisplayChip`, `StatusBadge`, `SearchPill` |
| `r_md_cr()/r_sm_cr()/r_lg_cr()` | Frames, `TradeBtn`, `SimpleBtn`, `ActionBtn` |
| `CornerRadius::same(st.r_xs as u8)` — truncating cast | `KeybindChip`, `Stepper`, `CompactStepper` |
| `CornerRadius::same(st.r_lg as u8)` — truncating cast | `DialogHeader` |
| `CornerRadius { nw: 99, sw: 99, ne: 0, se: 0 }` — literal 99 | `RemovableChip` |
| Literal `0.0` | `TabBar` underline rect |

### Alpha/Color Usage

| Pattern | Widgets |
|---------|---------|
| `alpha_*()` functions | Shells, most buttons/chips/frames |
| `ALPHA_*` uppercase constants | `WatchlistRow` (multiple), `NewsRow` |
| Literal alpha `12` | `BrandCtaButton` hover |
| `Color32::from_rgb(241, 196, 15)` warn yellow | `StatusDot`, `ConnectionIndicator`, `Toast`, `TrendArrow::new()` — **4 occurrences** |
| `Color32::from_rgb(231, 76, 60)` bear red (default) | `NotificationBadge`, `ConnectionIndicator` |
| `Color32::from_rgb(46, 204, 113)` bull green (default) | `ConnectionIndicator`, `WatchlistRow` |

---

## Top 20 Recommendations (Prioritized)

| # | Priority | Widget(s) | Issue | Fix |
|---|----------|-----------|-------|-----|
| 1 | Critical | `PopupFrame` | Shadow fully hardcoded — ignores style knobs | Replace `offset/blur/spread/alpha` literals with `st.shadow_*` fields |
| 2 | High | `TradeBtn`, `SimpleBtn`, `SmallActionBtn`, `ActionBtn` | All underline strokes use `Stroke::new(1.0, ...)` | Replace with `Stroke::new(current().stroke_bold, ...)` |
| 3 | High | `TradeBtn`, `SimpleBtn`, `SmallActionBtn`, `ActionBtn` | Outline strokes use literal `1.5` | Replace with `stroke_bold()` |
| 4 | High | `WatchlistRow`, `NewsRow` | `ALPHA_*` constants instead of `alpha_*()` functions | Replace all `ALPHA_GHOST/ACTIVE/MUTED/STRONG` constant uses with function calls |
| 5 | High | `KeybindChip`, `Stepper`, `CompactStepper`, `DialogHeader` | `st.r_xs as u8` / `st.r_lg as u8` truncating cast | Replace with `Radius::Xs.corner()` / `Radius::Lg.corner()` |
| 6 | High | `RemovableChip` | Literal `99` for pill radius | Replace with `current().r_pill.min(u8::MAX as f32) as u8` |
| 7 | High | All status widgets | `Color32::from_rgb(241, 196, 15)` warn yellow duplicated 4× | Add `t.warn` field to `Theme` or extract to `WARN_YELLOW: Color32` in `style.rs` |
| 8 | Medium | `IconBtn` | `.small/.medium/.large` literal sizes | Replace with `font_sm()/font_md()/font_lg()` |
| 9 | Medium | `PanelHeader` | `size(11.0)` literal | Replace with `font_md()` or `TextStyle::Label` |
| 10 | Medium | `DialogHeader` | `Margin { left: 12, right: 10, top: 10, bottom: 10 }` | Replace with `gap_lg()/gap_md()` |
| 11 | Medium | `BrandCtaButton` | Heights `24/32/40` hardcoded | Map to `Size::Sm/Md/Lg.height()` |
| 12 | Medium | `BrandCtaButton` | Hover alpha literal `12` | Replace with `alpha_ghost()` |
| 13 | Medium | `TabStrip`, `TabBarWithClose` | `min_size` height `20.0/18.0` | Replace with `Size::Sm.height()` / `btn_small_height()` |
| 14 | Medium | `TextInput` fallback path | `Stroke::new(1.0, ...)` | Replace with `stroke_thin()` |
| 15 | Medium | `Toast` | `add_space(8.0)`, accent stripe `3.0` width, `CornerRadius::same(2)` | Replace with `gap_lg()`, token-driven accent width, `Radius::Xs.corner()` |
| 16 | Medium | `SectionLabel` non-Sm variants | Bypass `TextStyle` | Add `TextStyle::LabelXs/Md/Lg` entries or extend existing |
| 17 | Low | `NotificationBadge` | All geometry hardcoded (`h=12`, `7.5` font) | Use `font_xs()` for font; tie `h` to a token |
| 18 | Low | `Spinner` | `LoadSize` px values `10/14/20` | Map to `font_sm()/font_md()/font_xl()` |
| 19 | Low | `TextStyle::NumericHero` | Literal `30.0` | Add `st.font_numeric_hero` to `StyleSettings` |
| 20 | Low | `Size::Xs.height()` | Returns literal `16.0` | Add `st.size_xs_height` or reuse `btn_compact_height()` |

---

## Per-Family Tier Summary

| Family | Reference Widget | Avg Tier | Worst Offender |
|--------|-----------------|----------|----------------|
| Foundation | `ButtonShell` / `ChipShell` | **5** | `CardShell` (4, fallback colors) |
| Buttons | `ButtonShell` | 3 | `ChromeBtn` (2), `IconBtn` (3) |
| Pills/Chips | `ChipShell` | 3.5 | `RemovableChip` (3), `KeybindChip` (3) |
| Text | `BodyLabel` | 4.5 | `SectionLabel` non-Sm (3) |
| Frames | `CardFrame` | 4.5 | `PopupFrame` (2) |
| Headers | `PanelHeaderWithClose` | 3 | `PanelHeader` (2), `DialogHeader` (3) |
| Tabs | `TabBar` | 3 | Shared height literals |
| Rows | `RowShell` | 2.5 | `WatchlistRow`/`DomRow` painter bodies (2) |
| Cards | `MetricCard` | 4 | All cards near Tier 4 |
| Inputs | `TextInput` (theme path) | 3.5 | `NumericInput` (3), `Slider` (3) |
| Selects | `Dropdown` | 3 | All 3 |
| Status | `TrendArrow`/`SearchPill` | 3 | `Skeleton`/`NotificationBadge` (2) |
| Layout | `Splitter` | 4 | — |
| Form | `FormRow` | 3 | Hardcoded gutter default |
