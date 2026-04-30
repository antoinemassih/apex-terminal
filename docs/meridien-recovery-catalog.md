# Meridien Visual Properties Catalog

This document catalogs every Meridien-distinct visual property found in git stash `a20cf053` that does not exist on `main`. The goal is to rebuild each one properly through the design system (`UiStyle`, theme presets, widget helpers) rather than via ad-hoc `if style == 0 { ... }` branches.

Sources examined:
- `src-tauri/src/chart_renderer/gpu.rs` (diff vs main, ~7 110 lines)
- `src-tauri/src/chart_renderer/ui/**` (diff vs main, ~33 900 lines)

---

## 1. UiStyle Struct & MERIDIEN Preset

**Property:** The entire `UiStyle` struct plus the `MERIDIEN` const preset that centralizes all per-style visual tokens.

**Stash location:** `src-tauri/src/chart_renderer/ui/style.rs`, new additions near end of file (~lines 17 394–17 514 in the diff).

**Visual specification:**

| Field | Relay | Meridien |
|---|---|---|
| `r_xs / r_sm / r_md / r_lg / r_pill` | 2 / 3 / 4 / 8 / 999 | **0 / 0 / 0 / 0 / 0** (fully square) |
| `stroke_hair / thin / std / bold / thick` | 0.3 / 0.5 / 1.0 / 1.5 / 2.0 | 0.5 / 1.0 / 1.0 / 1.0 / 1.0 (all strokes collapse to 1 px) |
| `header_height_scale` | 1.0 | **1.10** |
| `toolbar_height_scale` | 1.0 | **1.40** (Bloomberg-style tall toolbar) |
| `font_hero` | 22 pt | **36 pt** |
| `solid_active_fills` | false | true |
| `uppercase_section_labels` | false | **true** |
| `label_letter_spacing_px` | 0 | **1 px** |
| `serif_headlines` | false | **true** (Source Serif 4) |
| `shadows_enabled` | true | true (soft, not suppressed) |
| `hairline_borders` | false | **true** |
| `vertical_group_dividers` | false | **true** |
| `button_treatment` | `SoftPill` | **`UnderlineActive`** |

**Style-gating:** Applies whenever `style_id(wl) == 3` (style slot index 3, labelled "Meridien").

**Suggested implementation:** The struct and preset already exist in the stash and are the canonical source of truth. Re-introduce `UiStyle`, `ButtonTreatment`, `RELAY`, `APERTURE`, `MERIDIEN`, and `current()` into `ui/style.rs`. Every visual property in subsequent sections flows from this struct; rebuilding anything else first is premature.

---

## 2. Meridien Color Themes

**Property:** Two new `Theme` entries — `"Meridien Paper"` (light) and `"Meridien Dark"` — paired specifically with the Meridien style preset.

**Stash location:** `gpu.rs`, THEMES const, light-theme section (~diff lines 165–169).

**Visual specification:**

| Token | Meridien Paper | Meridien Dark |
|---|---|---|
| `bg` | `rgb(239,233,221)` warm cream | `rgb(26,23,19)` warm near-black |
| `bull` | `rgb(90,139,78)` sage green | `rgb(120,170,104)` |
| `bear` | `rgb(194,83,42)` terracotta | `rgb(220,108,70)` |
| `dim` | `rgb(94,86,72)` muted warm gray | `rgb(150,138,118)` |
| `toolbar_bg` | same as `bg` (no contrast strip) | `rgb(18,16,14)` |
| `toolbar_border` | `rgb(88,78,62)` warm-dark hairline | `rgb(80,72,60)` |
| `accent` | `rgb(216,84,44)` deep orange | `rgb(232,118,80)` |
| `text` | `rgb(28,24,18)` near-black warm | `rgb(238,228,210)` |

**Style-gating:** Not gated by code — new theme entries usable independently. Intended to pair with Meridien style preset.

**Suggested implementation:** Add both entries to the `THEMES` const. Consider a `theme_recommended_style: Option<u8>` field on `Theme` so the UI can suggest pairing (or auto-select when the user picks a Meridien theme).

---

## 3. Global Widget Styling (egui visuals)

**Property:** A full alternative `egui::Style::visuals` configuration block that applies when `style_id == 3`. Meridien-specific overrides compared to Relay:

- **Inactive widgets:** `bg_fill = TRANSPARENT` (no fill at all), hairline `bg_stroke` at full border alpha (70–80 out of 255), versus Relay's subtle tint fill.
- **Hovered widgets:** Flat accent tint (`alpha 18–24`) + hairline accent stroke; versus Relay's bevel/contrast fill.
- **Active/pressed widgets:** Solid accent fill + contrasting cream/near-black fg (no translucency), versus Relay's translucent tint.
- **Open widgets:** Accent tint fill at `alpha 32–48`; solid accent stroke.
- **Selection:** Solid `t.accent` fill + `t.accent` stroke (no tint).
- **Popup/menu window:** `toolbar_bg` fill + `toolbar_border` stroke at `stroke_std` (1 px flat rule, no shadow blur).
- **Spacing:** `button_padding = (8, 3)`, `menu_margin = {6,6,4,4}`, `interact_size.y = 22`, `item_spacing = (4, 3)` — denser editorial grid vs Relay's `(12,6) / (10,10,8,8) / 26 / (6,4)`.
- **Popup and window shadows:** `Shadow::NONE` for both `popup_shadow` and `window_shadow`.

**Stash location:** `gpu.rs`, `setup_theme()`, `if meridien { ... } else { ... }` block (~diff lines 914–954).

**Style-gating:** Fully gated on `style_id(watchlist) == 3`.

**Suggested implementation:** Move the dual-branch `visuals` configuration into `ui::style::apply_ui_style(ctx, style, t)` — a single function called from `setup_theme`. It reads `current()` and applies the appropriate egui visuals block, keeping `gpu.rs` clean.

---

## 4. Toolbar — Height & Group Dividers

**Property (height):** Toolbar height is multiplied by `current().toolbar_height_scale`. At scale 1.40 and compact-mode baseline of 30 px, the Meridien toolbar is 42 px tall; non-compact is 53 px — a significantly taller, more editorial bar.

**Property (group dividers):** Under `hairline_borders`, `tb_group_break()` paints a full-height crisp 1 px vertical line spanning the entire toolbar panel rect between every logical button cluster. Relay uses `egui::Separator` with 4 px spacing instead.

**Property (item spacing):** Toolbar button `item_spacing.x` collapses to `0.0` under Meridien so labels sit flush against their adjacent dividers. Relay uses 4 px.

**Stash location:** `gpu.rs`, `render_toolbar()` and `tb_group_break()` function (~diff lines 1 096–1 155, 1 192–1 219).

**Style-gating:** Height scale and zero item spacing: `current().hairline_borders`. Divider rendering: `current().vertical_group_dividers` + `current().hairline_borders`.

**Visual specification:** Divider line: `rule_color_for(t.bg, t.toolbar_border)` color, 1 px width, painted from `panel.top()` to `panel.bottom()` via a `layer_painter` (so it is not clipped to inner layout bounds). 6 px breathing room (padding) around each divider.

**Suggested implementation:** `tb_group_break(ui, t)` already exists in the stash. Re-introduce it verbatim. The `toolbar_panel_rect()` thread-local that provides the full panel y-range also needs to be added to `style.rs`.

---

## 5. Toolbar — Label Uppercasing

**Property:** All toolbar button labels are uppercased under Meridien. The `tb_label(icon, text)` helper calls `text.to_uppercase()` when `current().hairline_borders` is true. Non-Meridien styles pass text through unchanged.

**Stash location:** `gpu.rs`, `tb_label()` function (~diff lines 1 103–1 114).

**Style-gating:** `current().hairline_borders`.

**Visual specification:** Labels affected: Feed, Playbook, Watchlist, Analysis, Signals, Window — all right-cluster nav buttons. Icon is dropped; only the text label is uppercased.

**Suggested implementation:** This is a direct consequence of `uppercase_section_labels: true` in the MERIDIEN preset. `tb_label()` should read `current().uppercase_section_labels` instead of `hairline_borders` for cleaner semantics.

---

## 6. Search / Command Pill (Toolbar Right Cluster)

**Property:** A flat, bespoke pill widget in the right toolbar cluster that reads "🔍 /CMD" — a clickable search/command-palette trigger. Absent in Relay (which has no dedicated search pill in the toolbar).

**Stash location:** `gpu.rs`, right-cluster section of `render_toolbar()` (~diff lines 2 234–2 260).

**Visual specification:**
- Width: 78 px; height: `panel_rect.height() - 14 px`, clamped ≥ 20 px
- Background: `t.bg.gamma_multiply(1.05)` (Meridien: slightly lighter than canvas) vs `0.92` for Relay
- Border: `rule_stroke_for(t.bg, t.toolbar_border)` — context-aware 1 px rule
- Icon color: `color_alpha(t.dim, alpha_active())`
- Label color: `color_alpha(t.dim, alpha_strong())`
- Painted by `components_extra::paint_search_command_pill()`
- Click toggles `watchlist.cmd_palette_open`

**Style-gating:** The pill is inserted unconditionally for all styles in the stash, but its background tint is `1.05` under `st.hairline_borders` vs `0.92` otherwise.

**Suggested implementation:** Expose `paint_search_command_pill` as a widget in `widgets::inputs::SearchPill`. Add `cmd_palette_open: bool` to `Watchlist`. Gate tint on `current().hairline_borders`.

---

## 7. Connection Status Button — Full-Column Hover Fill

**Property:** The small connection-status dot (IBKR ●/○) in the right toolbar cluster has an expanded hit target (28 × 20 px vs 20 × 20 px) and, when hovered, fills the full column from `panel.top()` to `panel.bottom()` with `color_alpha(t.toolbar_border, 80)`. This "full column hover" is Meridien's way of making compact frameless buttons feel interactive without adding explicit borders.

**Stash location:** `gpu.rs`, connection dot paint block inside right cluster (~diff lines 2 196–2 224).

**Style-gating:** The expanded hit target and layer-painter full-column fill is applied unconditionally in the stash, but it exists specifically to match Meridien's full-height divider aesthetic.

**Visual specification:** Fill rect spans `[rect.left(), panel.top(), rect.right(), panel.bottom()]`. Color: `color_alpha(t.toolbar_border, 80)`. Corner radius: 0 (consistent with hairline style). Cursor: `PointingHand` on hover.

**Suggested implementation:** The full-column hover fill treatment should become a helper `paint_toolbar_column_hover(painter, rect, panel_rect, color)` in `ui::style` or `ui::components`, applied to any toolbar button that opts in via the `vertical_group_dividers` style flag.

---

## 8. Pane Chrome — Hairline Border System

**Property:** Under Meridien the pane border system changes completely. The old system painted a thick accent underline under the active pane header and highlighted inactive panes with a colored rect stroke. The stash replaces this with:

1. **Top hairline** on every pane (painted via `rule_stroke_for(t.bg, t.toolbar_border)`)
2. **Left and bottom hairlines** between adjacent panes when `visible_count > 1`
3. **Accent top accent line** (1 px, `t.accent.gamma_multiply(0.55)`) inset by 1 px on the active pane's top border — the *only* active-pane indicator

**Stash location:** `gpu.rs`, `render_chart_pane()`, pane border block (~diff lines 2 675–2 730).

**Style-gating:** The new hairline system is unconditional in the stash — it replaces the old system entirely. The active-pane accent top line is gated on `is_active && visible_count > 1`.

**Visual specification:** Rule color: `rule_stroke_for(bg, toolbar_border)` — a helper that chooses 60% alpha on light, 85% alpha on dark. Active top accent: `t.accent.gamma_multiply(0.55)` at `stroke_std` (1 px). All borders are drawn with a `Painter`, not via egui widget borders.

**Suggested implementation:** Create `ui::style::paint_pane_borders(painter, pane_rect, is_active, visible_count, t)` that centralizes this logic. Let it read `current().hairline_borders` to choose between the old and new system.

---

## 9. Pane Header Background Treatment

**Property:** Under Relay/Aperture the pane header gets a distinct fill: active pane = `t.bg.gamma_multiply(1.2)`, inactive = `t.toolbar_bg` (darker). Under Meridien, headers are nearly transparent: inactive panes get no fill at all (canvas color shows through); the active pane gets a 5% darken (`t.bg.gamma_multiply(0.95)`).

The active-tab **bottom accent underline** (2 px `t.accent` line at the bottom of active tab rects) is removed entirely under Meridien — the pane border alone frames the pane.

**Stash location:** `gpu.rs`, pane header fill block (~diff lines 2 735–2 760).

**Style-gating:** Fully gated on `!st.hairline_borders` (Relay path) vs `st.hairline_borders` (Meridien path).

**Visual specification:** Meridien active header: `t.bg.gamma_multiply(0.95)`, corner radius 0. Inactive: no `rect_filled` call at all.

**Suggested implementation:** Add `UiStyle::active_header_fill_multiply: f32` (e.g. 1.2 for Relay, 0.95 for Meridien) and `UiStyle::inactive_header_fill: bool` (true = use `toolbar_bg`, false = skip). Read these in the header painting block.

---

## 10. Pane Header — Right Action Cluster

**Property:** A new right-aligned cluster of flat-text action buttons inside every pane header: **"+ Compare"**, **"Order"**, **"DOM"**, **"Options"**. These are "frameless label + vertical hairline divider" buttons with no bg fill — active state is bright text (`t.text`), inactive is dim text (`t.dim`).

**Stash location:** `gpu.rs`, `render_chart_pane()`, right-side pane action cluster block (~diff lines 2 648–2 750 of the gpu diff).

**Style-gating:** The cluster renders for all styles but the per-divider hairlines between buttons only paint when `current().hairline_borders` is true.

**Visual specification:**
- Labels: monospace, `font_md()` size
- Gap between labels: 14 px; gap around dividers: 7 px
- Dividers: `rule_color_for(t.bg, t.toolbar_border)`, 1 px, full header height
- Active label color: `t.text`; inactive: `t.dim`
- Painted by `components_extra::paint_pane_header_action()`
- Buttons move: DOM and Order Entry moved out of the top toolbar into this per-pane cluster

**Suggested implementation:** `paint_pane_header_action` should live in `ui::widgets::toolbar` or `ui::components`. The cluster itself should be a `widgets::pane::PaneHeaderActions` widget consuming a `Vec<(&str, bool)>` (label + active) and emitting click events.

---

## 11. Active-Tab Underline Removal

**Property:** Relay/Aperture paint a 2 px `t.accent` bottom underline on the active tab in the tab bar. Meridien removes this underline entirely — the tab bar relies solely on the pane's top accent hairline and the tab-background-fill contrast.

**Stash location:** `gpu.rs`, tabbed pane header, active tab rendering (~diff lines 2 835–2 845, comment: "active-tab bottom accent rule removed").

**Style-gating:** The removal is behind `// (active-tab bottom accent rule removed — pane border alone frames the pane)` — applied unconditionally in the stash when Meridien is active (since the whole pane-chrome block changed).

**Suggested implementation:** Add `UiStyle::show_active_tab_underline: bool` (true for Relay/Aperture, false for Meridien). Read in tab rendering.

---

## 12. Section Labels — Uppercase + Tracked-Out

**Property:** Section labels (e.g. "ORDER TICKET", "TYPE", "TIF", "QUANTITY", "LIMIT PRICE", "NOTIONAL", "BUYING POWER", "EST. SLIPPAGE") render as all-caps monospace with a small trailing dim at 70% alpha. This treatment is driven by `uppercase_section_labels: true` in the MERIDIEN preset. Under Relay these same labels use mixed-case.

**Stash location:** `gpu.rs`, `render_order_entry_body_meridien()` (~diff lines 247–435); also `components_extra::section_label_xs()` which respects `current().uppercase_section_labels`.

**Style-gating:** `current().uppercase_section_labels`.

**Visual specification:** Font: monospace, `font_xs()` size. Color: `t.dim.gamma_multiply(0.7)`. Rendered via `section_label_xs(ui, "LABEL", color)` helper. Letter spacing is tracked via `label_letter_spacing_px: 1.0`.

**Suggested implementation:** `section_label_xs()` (and the existing `section_label()`) should check `current().uppercase_section_labels` and call `.to_uppercase()` on the label string. `label_letter_spacing_px` should be applied via custom layouter if egui supports it, otherwise approximated by spacing.

---

## 13. Meridien Order Ticket Layout

**Property:** An entirely separate order-entry body (`render_order_entry_body_meridien`) that replaces the standard compact form when `current().hairline_borders` is true. The layout is editorial/Bloomberg: every section separated by full-width hairlines, uppercase section labels, a large quantity hero number (`font_2xl`), pill chip presets.

**Stash location:** `gpu.rs`, `render_order_entry_body_meridien()` function (~diff lines 223–496).

**Visual specification (section by section):**
1. **Header row:** "ORDER TICKET" section label (left) + symbol monospace `font_md` bold (right)
2. **Hairline rule:** full-width `color_alpha(toolbar_border, 50)`
3. **BID/LAST/ASK strip:** 3-column grid; labels at `font_xs` 60% alpha, values at `font_sm`/`font_md`. BID in `t.bear`, LAST in `t.text`, ASK in `t.bull`.
4. **BUY/SELL toggle:** Two full-width side-by-side `ActionButton` widgets: BUY = `t.bull` primary tier, SELL = `t.bear` destructive tier
5. **TYPE row:** "TYPE" section label + `interval_segmented` control
6. **TIF row:** "TIF" + `interval_segmented`
7. **Hairline + QUANTITY:** Section label, then quantity as `font_2xl` strong monospace + `Stepper` widget + preset PillButtons (100, 500, 1000 shares)
8. **LIMIT PRICE:** Section label, then `TextEdit` 96 px wide + compact stepper + BID/LAST/ASK pill presets
9. **Hairline + BRACKET checkbox:** "BRACKET — STOP + TARGET" label; if active, TP (bull) + SL (bear) TextEdits
10. **Hairline + META ROW:** 3-column: NOTIONAL `$x`, BUYING POWER `$xM`, EST. SLIPPAGE `x bp` — each with section label above value
11. **REVIEW CTA:** Full-width `ActionButton` "REVIEW BUY/SELL · qty @ price". Primary tier for BUY, Destructive for SELL.

**Style-gating:** Fully Meridien-only — entered via `if current().hairline_borders { render_order_entry_body_meridien(...); return; }`.

**Suggested implementation:** `render_order_entry_body_meridien` should become `widgets::order_entry::MeridienOrderTicket`, a self-contained widget struct that owns the layout. The `hairline_borders` guard in `render_order_entry_body` is the correct dispatch point.

---

## 14. Serif Hero Headlines

**Property:** Under `serif_headlines: true` (Meridien), large numerical displays — NAV, P&L, buying power in the account strip; hero prices in widgets; big quantity number in the order ticket — render in the registered `"serif"` font family (Source Serif 4 at `font_hero` = 36 pt) instead of monospace. This gives the Bloomberg-editorial typographic feel.

**Stash location:** `gpu.rs`, account strip `strip_value` closure (~diff lines 4 494–4 510); `ui/style.rs`, `hero_font_id()` and `hero_text()` helpers; also `gpu.rs` around line 13 019 (impact-family reference in signal widget).

**Style-gating:** `current().serif_headlines`.

**Visual specification:** `hero_font_id(size)` returns `FontId::new(size, FontFamily::Name("serif"))` when serif is on, `FontId::monospace(size)` otherwise. `hero_text(text, color)` wraps a `RichText` with the appropriate family + `font_hero` size.

**Suggested implementation:** Register Source Serif 4 in the font loader. `hero_font_id()` and `hero_text()` are already the right abstraction — call them everywhere large numerals appear instead of hardcoding `egui::FontId::monospace(...)`.

---

## 15. Account Strip — Taller Height + Serif Values

**Property:** The account strip height grows from 26 px (main) to 36 px (stash). Values (NAV, Day P&L, Unr P&L, Real P&L) render in serif at `font_lg` with `strong()`. Buying Power, Margin, Excess Liquidity render in serif at `font_lg` without strong. Labels remain monospace at `font_md` 50% alpha dim.

**Stash location:** `gpu.rs`, `account_strip` panel (~diff lines 4 383–4 415).

**Style-gating:** Height increase: unconditional in the stash (removed the 26.0 hardcode). Serif rendering: `current().serif_headlines` via `strip_value` closure.

**Visual specification:** Height: 36 px. Label font: monospace `font_md`, `t.dim.gamma_multiply(0.5)`. Value font: `font_lg`, applied via `strip_value(s, color, strong)` which delegates to `hero_text` semantics.

**Suggested implementation:** Expose `UiStyle::account_strip_height: f32` (26.0 for Relay, 36.0 for Meridien). Drive value rendering through `hero_text()`.

---

## 16. Popup / Dialog Window — Flat Hairline Border

**Property:** Meridien's modal and popup windows suppress the drop-shadow and use a crisp `toolbar_border`-colored 1 px window stroke instead. `window_shadow` and `popup_shadow` are both set to `Shadow::NONE` under `style_id == 3`.

**Stash location:** `gpu.rs`, `setup_theme()` shadow block (~diff lines 832–858).

**Style-gating:** `style_id(watchlist) == 3`.

**Visual specification:** `popup_shadow = Shadow::NONE`, `window_shadow = Shadow::NONE`. Window border: `window_stroke = Stroke::new(stroke_std, toolbar_border)` — flat, no blur, no spread.

**Suggested implementation:** Move shadow application into `apply_ui_style()`. Read `current().shadows_enabled` — false for Meridien → `Shadow::NONE`.

---

## 17. Effective Theme Overlay (Design-Mode Live Editing)

**Property:** The `effective_theme(idx)` function, only present in the stash, allows the design-mode feature flag to overlay palette edits onto any theme at runtime (so the inspector can repaint the chart live without recompilation). The `Theme` struct gains `#[derive(Clone)]`.

**Stash location:** `gpu.rs`, `effective_theme()` and `overlay_palette()` functions (~diff lines 88–132).

**Style-gating:** `#[cfg(feature = "design-mode")]` only.

**Visual specification:** Not a user-visible visual property per se — it's infrastructure for the design-mode inspector. But it gates `design_tokens::get()` → palette overlays for `bull`, `bear`, `accent`, `text`, `dim`, `bg`, `toolbar_border`, and `toolbar_bg` (the last derived as `surface_alt`).

**Suggested implementation:** Re-introduce `effective_theme()` in `gpu.rs` and `Theme: Clone`. Replace all `&THEMES[theme_idx]` calls with `effective_theme(theme_idx)`.

---

## 18. `ButtonTreatment::UnderlineActive` — General Button Treatment

**Property:** Under Meridien, interactive buttons (pill buttons, toolbar buttons) render with no fill and no border in their idle state; the active state is indicated by a bottom rule (underline) in the accent color rather than a filled background. This is the `UnderlineActive` variant of `ButtonTreatment`.

**Stash location:** `ui/style.rs`, `ButtonTreatment` enum and downstream `tb_btn` / `pill_btn` implementations (diff around line 17 444–17 460 + call sites in `style.rs`).

**Style-gating:** `current().button_treatment == ButtonTreatment::UnderlineActive`.

**Visual specification:**
- Idle: transparent bg, transparent border, `t.dim` text
- Hovered: transparent bg, `t.accent` 1 px underline, `t.text`
- Active: transparent bg, solid `t.accent` 2 px underline, `t.accent` text

Two specialized treatments also present:
- `RaisedActive` — for top-nav page-section buttons: active = lighter block fill against darker header backing
- `BlackFillActive` — for the interval segmented control: active = solid near-black fill + cream text; idle = flat dim text

**Suggested implementation:** `ButtonTreatment` is already defined in the stash. Each variant should be implemented in `tb_btn()`, `pill_btn()`, and `action_btn()` by reading `current().button_treatment`.

---

## 19. `chrome_tile_button` — Template (T) and +Tab Buttons

**Property:** The template/star button and "+Tab" button in pane headers use a dedicated `paint_chrome_tile_button()` helper that encapsulates 3-state visual logic (active / hovered / idle) in one place. All three states use rounded-rect fill + outside stroke with alpha values from the design-system constants.

**Stash location:** `gpu.rs`, `paint_chrome_tile_button()` function (~diff lines 130–170) and call sites in both tabbed and non-tabbed pane header branches.

**Style-gating:** Not Meridien-specific — `paint_chrome_tile_button` is universal across styles. It uses `current().r_md` for corner radius (which collapses to 0 under Meridien) and `current().stroke_thin` for border width.

**Visual specification:**
- Active: `color_alpha(t.accent, 38)` fill, `t.accent` fg, `color_alpha(t.accent, ALPHA_ACTIVE)` border
- Hovered: `color_alpha(t.toolbar_border, ALPHA_SUBTLE)` fill, `t.text` fg, `color_alpha(t.accent, ALPHA_LINE)` border
- Idle: `color_alpha(t.toolbar_border, 18)` fill, `t.dim.gamma_multiply(0.8)` fg, `color_alpha(t.toolbar_border, ALPHA_MUTED)` border

**Suggested implementation:** Re-introduce `paint_chrome_tile_button()` in `gpu.rs` (it's a file-local helper). Replace the duplicated inline logic at both the tabbed and non-tabbed call sites (which the stash already did).

---

## Summary

| # | Property | Meridien-Only? |
|---|---|---|
| 1 | `UiStyle` struct + `MERIDIEN` preset (all tokens) | Yes (introduces struct) |
| 2 | Meridien Paper + Dark color themes | Yes |
| 3 | Global egui widget visuals (widget states, spacing, shadows) | Yes |
| 4 | Toolbar height scale (1.4×) + vertical group dividers | Yes |
| 5 | Toolbar label uppercasing | Yes |
| 6 | Search/command pill in toolbar right cluster | Partially (tint differs) |
| 7 | Connection dot full-column hover fill | Yes |
| 8 | Pane border hairline system | Yes |
| 9 | Pane header background treatment + active-tab underline removal | Yes |
| 10 | Pane header right action cluster (Order/DOM/Options/Compare) | No (universal, but dividers gated) |
| 11 | Active-tab underline removal | Yes |
| 12 | Section labels uppercase + tracked-out | Yes |
| 13 | Meridien order ticket layout | Yes |
| 14 | Serif hero headlines | Yes |
| 15 | Account strip taller height + serif values | Yes |
| 16 | Popup/dialog flat hairline border + no shadow | Yes |
| 17 | `effective_theme()` + design-mode live palette overlay | No (feature-gated infra) |
| 18 | `ButtonTreatment::UnderlineActive` (+ RaisedActive + BlackFillActive) | Yes |
| 19 | `paint_chrome_tile_button` universal helper | No (universal) |

**Total distinct properties catalogued: 19**
