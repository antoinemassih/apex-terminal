# Design System Component Audit
**Date:** 2026-05-02 (refreshed post-R3)
**Scope:** `src-tauri/src/chart_renderer/ui/widgets/`, `components/`, `components_extra/`
**Auditor:** Claude Sonnet 4.6 (read-only, no source edits)

---

## Tier Definitions

| Tier | Meaning |
|------|---------|
| **5** | Fully tokenized — every visual property routed through `StyleSettings`, `TextStyle`, `Radius`, `Size`, `alpha_*()`, `gap_*()`, `font_*()`, `stroke_*()`. No literal values. |
| **4** | Mostly tokenized — 1–3 minor hardcoded literals (typically geometry calculations or documented fallbacks). |
| **3** | Partially tokenized — meaningful mix of tokens and literals. Responds to style changes but has visible exceptions. |
| **2** | Minimally tokenized — uses a few token calls but majority of visual properties are hardcoded literals. |
| **1** | No tokenization — entirely hardcoded. |

---

## Executive Summary

**Overall widget library tier: 3.5 / 5** (post-R1/R2, up from ~2.5 pre-refactor)

R1/R2 added: `FilterPill`, `SectionHeader`, `NmfToggle` (watchlist/), `ColorSwatchPicker`, `ThicknessPicker` (widgets/inputs.rs), `IndicatorParamRow`, `IndicatorParamRowF` (widgets/form.rs), `AccountStrip` (widgets/pane.rs). All new widgets are Tier 4+. The four Foundation shells (`ButtonShell`, `RowShell`, `CardShell`, `InputShell`) remain at Tier 5.

R3 added: `TopNav` (widgets/toolbar/top_nav.rs — extracted from gpu.rs; ~1664 lines removed), `ApertureOrderTicket` (widgets/form.rs — ~270 lines removed), `FloatingOrderPaneChrome` (widgets/pane.rs — ~80 lines removed). `PopupFrame` upgraded from Tier 2 to Tier 4 (shadow now reads `st.shadow_offset_y/blur/alpha`). The main remaining pathologies are in `WatchlistRow`/`DomRow` painter bodies (Tier 2) and the legacy button builders (Tier 3).

---

## Foundation Layer

### `foundation/tokens.rs`
| Item | API | Tier | Notes |
|------|-----|------|-------|
| `Radius` enum | `.corner()` | **5** | Fully routed through `current().r_*` |
| `Size` enum | `.height()`, `.padding()`, `.font()` | 4 | `Size::Xs.height()` returns literal `16.0` |
| `Density` enum | `.vscale()` | **5** | Pure scale multipliers — no token mapping needed |

### `foundation/text_style.rs`
| TextStyle | Tier | Notes |
|-----------|------|-------|
| `HeadingLg/Md`, `BodyLg`, `BodySm`, `MonoSm`, `NumericLg`, `Body`, `Caption`, `Mono`, `Numeric`, `Label`, `Eyebrow` | **5** | All route through `font_*()` or `st.font_*` fields |
| `Display` | 4 | `font_2xl() + 4.0` — literal `+4.0` offset |
| `NumericHero` | 2 | Literal `30.0` — not a `StyleSettings` field |

### `foundation/interaction.rs` — Tier **5**
### `foundation/variants.rs` — Tier **5**

### `foundation/shell.rs`
| Shell | Tier | Notes |
|-------|------|-------|
| `ButtonShell` | **5** | Reference implementation. Uses `Size`, `Radius`, `ButtonVariant`, `InteractionTokens`, `TextStyle::Body` |
| `RowShell` | **5** | `painter_mode` defaults to `style_row_height()` |
| `CardShell` | 4 | `neutral_*` fallback colors are `from_gray(N)` — documented fallbacks |
| `InputShell` | **5** | All state borders use `alpha_strong()/alpha_muted()`, widths use `stroke_bold()/stroke_thin()` |
| `ChipShell` | 4 | Uses `stroke_thin()` uniformly — could use `stroke_hair()` in `Subtle` variant |

---

## Widgets Inventory (post-R1/R2)

### `widgets/buttons.rs`

| Widget | Tier | Key Issues |
|--------|------|------------|
| `IconBtn` | 3 | `.small/.medium/.large` = `11.0/14.0/18.0` literals; `22.0` min floor not from `btn_small_height()` |
| `TradeBtn` | 3 | `Stroke::new(1.5, border)` in `OutlineAccent`; `Stroke::new(1.0, color)` underline — should be `stroke_bold()` |
| `SimpleBtn` | 3 | Same underline/outline stroke literals as `TradeBtn` |
| `SmallActionBtn` | 3 | Same underline/outline stroke literals |
| `ActionBtn` | 3 | `Stroke::new(0.5, ...)` — should be `stroke_hair()` |
| `ChromeBtn` | 2 | Intentional escape hatch. Caller supplies pre-styled `RichText`. `.padding` field stored but not applied |

**Reference:** `ButtonShell` (Tier 5). Migrate all five legacy builders onto it.

---

### `widgets/pills.rs`

| Widget | Tier | Key Issues |
|--------|------|------------|
| `PillButton` | 4 | Height literal `18.0` — should be `btn_small_height()` |
| `BrandCtaButton` | 3 | Heights `24.0/32.0/40.0` hardcoded; hover alpha literal `12` should be `alpha_ghost()` |
| `RemovableChip` | 3 | `CornerRadius { nw: 99, ... }` — literal `99` for pill radius |
| `DisplayChip` | 4 | Height `14.0` hardcoded; otherwise correct |
| `StatusBadge` | 4 | `dt_f32!(badge.*)` design-token knobs; `Radius::Pill.corner()` correct |
| `KeybindChip` | 3 | `CornerRadius::same(st.r_xs as u8)` — truncating cast; should use `Radius::Xs.corner()` |

---

### `widgets/text.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| `BodyLabel`, `MutedLabel`, `CaptionLabel`, `PaneTitle`, `Subheader`, `DimLabel` | **5** | Route through `TextStyle::as_rich()` |
| `SectionLabel` (Sm variant) | **5** | Uses `TextStyle::Label` |
| `SectionLabel` (Tiny/Xs/Md/Lg) | 3 | Fallback to raw `RichText::new(s).monospace().size(N)` — bypasses `TextStyle` |
| `NumericDisplay` (Lg) | 3 | Manual override to `font_lg()` — documented |
| `MonospaceCode` (Xs) | 3 | Manual override to `font_xs()` — documented |

---

### `widgets/frames.rs`

| Frame | Tier | Notes |
|-------|------|-------|
| `PanelFrame`, `CardFrame`, `DialogFrame`, `SidePanelFrame`, `CompactPanelFrame` | **5** | All gaps, radii, strokes, shadows via tokens |
| `TooltipFrame` | 4 | `dt_f32!(tooltip.*)` knobs — correct |
| `PopupFrame` | **4** | Shadow now reads `st.shadow_offset_y`, `st.shadow_blur`, `st.shadow_alpha` (fixed R3). One minor remaining literal: `spread: 1` (non-theme-sensitive). |

---

### `widgets/headers.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| `PanelHeaderWithClose`, `DialogHeaderWithClose` | 3 | Delegate to `style::panel_header_sub` / `style::dialog_header` — token-using |
| `PanelHeader` | 2 | `size(11.0)` literal; no `TextStyle` usage |
| `DialogHeader` | 3 | `Margin { left: 12, right: 10, top: 10, bottom: 10 }` — four hardcoded literals; `s.r_lg as u8` truncating cast |
| `PaneHeader` | 3 | Default `height: 28.0` not from token; default colors `from_rgb(...)` |

---

### `widgets/tabs.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| `TabBar` | 3 | `dt_f32!(tab.underline_thickness, 2.0)` correct; underline `rect_filled(..., 0.0, ...)` — literal corner `0.0` |
| `TabStrip` | 3 | `min_size: Vec2::new(0.0, 20.0)` literal in 2 places |
| `TabBarWithClose` | 3 | `min_size: 18.0` hardcoded |

---

### `widgets/rows/`

| Widget | Tier | Notes |
|--------|------|-------|
| `WatchlistRow` | 2 | `RowShell::painter_mode` at Tier 5, but painter body: `ALPHA_*` constants not `alpha_*()` fns; `Color32::from_rgb(255, 193, 37)` earnings; `Stroke::new(1.0/2.0, ...)` literals; `FontId::monospace(7.0)` literal |
| `DomRow` | 2 | Same painter-body pattern as `WatchlistRow` |
| `NewsRow` | 3 | Lighter painter body; uses some `ALPHA_*` constants |
| `OrderRow` | 4 | Migrated onto `RowShell` in R1/R2 |
| `AlertRow` | 4 | Migrated onto `RowShell` in R1/R2 |
| `OptionChainRow` | 4 | Migrated onto `RowShell` in R1/R2 |
| `Table` (rows/table.rs) | 4 | `style_row_height()`, `stroke_thin()` correct |

---

### `widgets/cards/`

All card files (`MetricCard`, `SignalCard`, `EarningsCard`, `EventCard`, `NewsCard`, `PlayCard`, `PlaybookCard`, `StatCard`, `TradeCard`) — migrated onto `CardShell` in R1/R2. **Tier: 4** across the board. Minor remaining issue: some use `font_*()` + raw `RichText` rather than `TextStyle::as_rich()`.

---

### `widgets/inputs.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| `TextInput` (theme path) | 4 | Uses `InputShell` |
| `TextInput` (palette path) | 3 | `Stroke::new(1.0, ...)` literal — should be `stroke_thin()` |
| `NumericInput` | 3 | Always uses palette path (no `.theme()` call) |
| `Stepper`, `CompactStepper` | 4 | `CornerRadius::same(st.r_xs as u8)` truncating cast |
| `SearchInput` | 4 | Foundation path correct; fallback has `avail - 36.0` literal |
| `ToggleRow` | 3 | No `TextStyle` usage; checkbox styled by egui visuals |
| `Slider` | 3 | Mutates egui visuals directly, not token path |
| **`ColorSwatchPicker`** (R1/R2) | **5** | New widget; fully tokenized; routed through `InputShell` |
| **`ThicknessPicker`** (R1/R2) | **5** | New widget; fully tokenized |

---

### `widgets/form.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| `FormRow` | 3 | `label_width: 120.0` hardcoded default |
| `MeridienOrderTicket` | 4 | Routed through `cta_btn`, `action_btn`, `simple_btn` — all token-using |
| **`IndicatorParamRow`** (R1/R2) | **5** | New widget; fully tokenized |
| **`IndicatorParamRowF`** (R1/R2) | **5** | New widget; fully tokenized |
| **`ApertureOrderTicket`** (R3) | 4 | New widget; Aperture/Octave order entry (~270 lines from gpu.rs). `SegmentedControl`, `NumericInput`, `Stepper`, `trade_btn`. One RTH amber literal pending `COLOR_AMBER` cleanup. |

---

### `widgets/watchlist/` (all R1/R2)

| Widget | Tier | Notes |
|--------|------|-------|
| **`FilterPill`** | **5** | New widget; `Radius::Pill`, `alpha_tint()/alpha_dim()`, `stroke_thin()`, `font_sm()` |
| **`SectionHeader`** | 4 | New widget; `style_label_case()`, `font_sm()`, `gap_md()`; one `add_space(6.0)` literal |
| **`NmfToggle`** | **5** | New widget; `r_sm_cr()`, `stroke_thin()`, `alpha_*()` |

---

### `widgets/toolbar/top_nav.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| **`TopNav`** (R3) | 3 | Extracted from gpu.rs (~1664 lines). Nav buttons, workspace picker, layout picker, symbol search, Paper-Live toggle, connection indicator. Delegates to `ToolbarBtn`, `SegmentedControl`, `SearchInput`, `AccountStrip`, `ConnectionIndicator`. Remaining hardcodes: toolbar frame margins and some inline `Color32` for toolbar accents — R4 targets. |

---

### `widgets/pane.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| `PaneFrame` | 4 | `pane_active_indicator`, `pane_border_width` — correct |
| `PaneSymbolBadge` | 4 | Uses `PillButton` internally |
| **`AccountStrip`** (R1/R2) | **5** | New widget; `account_strip_height`, `font_body/caption`, `TextStyle` |
| **`FloatingOrderPaneChrome`** (R3) | 4 | New widget (~80 lines from gpu.rs). Header chrome for floating order window: armed toggle, title, expand/collapse, X close. Uses `icon_btn`, `dialog_window_themed`. One inline stroke literal remaining. |

---

### `widgets/select.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| `SegmentedControl` | 4 | `idle_outline_color`, `segmented_idle_fill/text`, `r_pill` — correct |
| `Dropdown`, `Combobox` | 3 | `width: 140.0` hardcoded |

---

### `widgets/status.rs`

| Widget | Tier | Notes |
|--------|------|-------|
| `TrendArrow`, `SearchPill` | 4 | Mostly tokenized |
| `StatusDot`, `ConnectionIndicator` | 3 | `Color32::from_rgb(241, 196, 15)` warn yellow hardcoded (4× total in file) |
| `Spinner` | 3 | `LoadSize` px `10.0/14.0/20.0` literals |
| `ProgressBar` | 3 | Height literals `3.0/8.0/6.0` |
| `Toast` | 3 | `add_space(8.0)`, accent stripe `3.0`, `CornerRadius::same(2)` — literals |
| `Skeleton` | 2 | `rounding: 3.0`, `from_rgb(60,60,70)` base/highlight colors |
| `NotificationBadge` | 2 | All geometry hardcoded: `h=12.0`, `pad_x=4.0`, `font=7.5` |

---

### `components/` and `components_extra/`

| Module | Tier | Notes |
|--------|------|-------|
| `components/frames.rs` | **5** | Thin wrappers over `widgets/frames.rs` |
| `components/hairlines.rs` | **5** | `stroke_hair()/stroke_thin()` only |
| `components/headers.rs` | 4 | Delegates to `style::panel_header_sub` |
| `components/labels.rs` | **5** | `style_label_case()`, `font_sm()` |
| `components/metrics.rs` | 4 | `hero_text()`, `font_hero` correct |
| `components/pills.rs` | 4 | `alpha_tint()`, `r_pill()` correct |
| `components_extra/action_button.rs` | 4 | Delegates to `style::action_btn` |
| `components_extra/chips.rs` | 4 | Delegates to `StatusBadge` |
| `components_extra/dom_action.rs` | 3 | Some inline `egui::Button` with literal stroke |
| `components_extra/header_buttons.rs` | 4 | `ChromeBtn` wrappers — inherits Tier 2 of `ChromeBtn` but adds no new hardcodes |
| `components_extra/inputs.rs` | 4 | Delegates to `TextInput`/`NumericInput` |
| `components_extra/panels.rs` | 4 | `PanelFrame` wrappers |
| `components_extra/sortable_headers.rs` | 3 | Arrow icon size literal `9.0` |
| `components_extra/toasts.rs` | 3 | Inherits `Toast` tier |
| `components_extra/top_nav.rs` | 4 | Uses `ToolbarBtn` from `widgets/toolbar.rs` |

---

## Per-Family Tier Summary

| Family | Reference Widget | Avg Tier | Worst Offender |
|--------|-----------------|----------|----------------|
| Foundation | `ButtonShell` / `ChipShell` | **5** | `CardShell` (4, fallback colors) |
| Buttons | `ButtonShell` | 3 | `ChromeBtn` (2), `IconBtn` (3) |
| Pills/Chips | `ChipShell` | 3.5 | `RemovableChip` (3), `KeybindChip` (3) |
| Text | `BodyLabel` | 4.5 | `SectionLabel` non-Sm (3) |
| Frames | `CardFrame` | 4.8 | `TooltipFrame` (4) |
| Headers | `PanelHeaderWithClose` | 3 | `PanelHeader` (2), `DialogHeader` (3) |
| Tabs | `TabBar` | 3 | Shared height literals |
| Rows | `RowShell` | 2.5 | `WatchlistRow`/`DomRow` painter bodies (2) |
| Cards | `MetricCard` | 4 | All near Tier 4 |
| Inputs | `TextInput` (theme path) | 3.5 | `NumericInput` (3), `Slider` (3) |
| Form | `IndicatorParamRow` (new) | 3.5 | `FormRow` (3), `MeridienOrderTicket` (4) |
| Watchlist widgets (new) | `FilterPill`/`NmfToggle` | **5** | `SectionHeader` (4) |
| Selects | `SegmentedControl` | 3.5 | `Dropdown` (3) |
| Status | `TrendArrow`/`SearchPill` | 3 | `Skeleton`/`NotificationBadge` (2) |
| Layout | `Splitter` | 4 | — |
| components_extra | `action_button` | 3.5 | `dom_action` (3) |

---

## Top 20 Recommendations (Prioritized)

| # | Priority | Widget(s) | Issue | Fix |
|---|----------|-----------|-------|-----|
| 1 | ~~Critical~~ **DONE (R3)** | `PopupFrame` | Shadow was fully hardcoded | Fixed: now reads `st.shadow_offset_y/blur/alpha`. Tier 2 → 4. |
| 2 | High | `TradeBtn`, `SimpleBtn`, `SmallActionBtn`, `ActionBtn` | All underline strokes literal `1.0` | `Stroke::new(current().stroke_bold, ...)` |
| 3 | High | `TradeBtn`, `SimpleBtn`, `SmallActionBtn`, `ActionBtn` | Outline strokes literal `1.5` | `stroke_bold()` |
| 4 | High | `WatchlistRow`, `NewsRow` | `ALPHA_*` constants not `alpha_*()` functions | Replace all uppercase constant uses with function calls |
| 5 | High | `KeybindChip`, `Stepper`, `CompactStepper`, `DialogHeader` | `st.r_xs as u8` truncating cast | `Radius::Xs.corner()` / `Radius::Lg.corner()` |
| 6 | High | `RemovableChip` | `99` literal for pill radius | `current().r_pill.min(u8::MAX as f32) as u8` |
| 7 | High | All status widgets | `Color32::from_rgb(241, 196, 15)` warn yellow duplicated 4× | Add `t.warn` field to `Theme` or extract to `WARN_YELLOW` in `style.rs` |
| 8 | Medium | `IconBtn` | `.small/.medium/.large` literal sizes | `font_sm()/font_md()/font_lg()` |
| 9 | Medium | `PanelHeader` | `size(11.0)` literal | `font_md()` or `TextStyle::Label` |
| 10 | Medium | `DialogHeader` | `Margin { left: 12, right: 10, top: 10, bottom: 10 }` | `gap_lg()/gap_md()` |
| 11 | Medium | `BrandCtaButton` | Heights `24/32/40` hardcoded | `Size::Sm/Md/Lg.height()` |
| 12 | Medium | `BrandCtaButton` | Hover alpha literal `12` | `alpha_ghost()` |
| 13 | Medium | `TabStrip`, `TabBarWithClose` | `min_size` height `20.0/18.0` | `Size::Sm.height()` |
| 14 | Medium | `TextInput` fallback path | `Stroke::new(1.0, ...)` | `stroke_thin()` |
| 15 | Medium | `Toast` | `add_space(8.0)`, stripe `3.0`, `CornerRadius::same(2)` | `gap_lg()`, token-driven width, `Radius::Xs.corner()` |
| 16 | Medium | `SectionLabel` non-Sm variants | Bypass `TextStyle` | Add `TextStyle::LabelXs/Md/Lg` entries |
| 17 | Low | `NotificationBadge` | All geometry hardcoded | `font_xs()` for font; token for `h` |
| 18 | Low | `Spinner` | `LoadSize` px `10/14/20` | `font_sm()/font_md()/font_xl()` |
| 19 | Low | `TextStyle::NumericHero` | Literal `30.0` | Add `st.font_numeric_hero` |
| 20 | Low | `Size::Xs.height()` | Returns literal `16.0` | `btn_compact_height()` or new token |
