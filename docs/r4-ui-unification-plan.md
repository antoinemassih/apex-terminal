# R4 UI Unification Plan

**Date:** 2026-05-02  
**Auditor:** Claude Sonnet 4.6 (read-only audit, no source edits)  
**Scope:** All UI surfaces under `src/chart_renderer/ui/**`, `src/design_inspector.rs`, `src/native_main.rs`, `src/chart_renderer/gpu.rs` (UI-chrome only), `src/chart_renderer/mod.rs`. Chart-paint engine excluded.  
**Predecessor:** R3 Wave (TopNav, ApertureOrderTicket, FloatingOrderPaneChrome, PopupFrame shadow wiring, 75 gpu.rs Color32 literals)

---

## 1. Executive Summary

**Total hardcoded sites found: ~2,495** across all measured pattern categories.

| Pattern | Count |
|---------|-------|
| `Color32::` literals (all forms) | 916 (UI surfaces only, excl. gpu.rs chart-paint) |
| `Stroke::new(...)` with literal width | 396 |
| `.size(N)` font-size literals | 246 |
| Spacing literals (`Margin::same`, `Vec2::new`, `egui::vec2`, `inner_margin`, `outer_margin`) | 673 |
| `.color(Color32::...)` inline RichText | 53 |
| `.fill(Color32::...)` / `.stroke(Stroke::new...)` | 123 |
| `CornerRadius::same(N)` / `Rounding::same(N)` | 59 |
| `Frame::default` / `Frame::none` / `Frame::new` | 29 |

**Single biggest leverage point:** Widget struct defaults — `pane.rs`, `form.rs`, `status.rs` all define their `Default`/`new()` impls with inline `Color32::from_rgb(...)` instead of reading `current()`. These color values are identical to theme tokens but never connected. Fixing these three files alone removes ~90 literals and makes widgets theme-responsive.

**Second-biggest leverage point:** `.size(N)` literals — 199 occurrences of raw `.size(10.0)` / `.size(11.0)` / `.size(12.0)` etc. Token functions `font_sm()`, `font_md()`, `font_lg()` exist in `style.rs` but are used in only 143 places vs 246 that still use literals.

**Third-biggest leverage point:** Function signatures — 56 helper functions still accept `color: Color32` parameters instead of reading `current()` internally. This is the root cause propagating literals through the call chain.

**Top 10 files by total hardcoded count:**

| Rank | File | Est. Total |
|------|------|-----------|
| 1 | `design_inspector.rs` | 257 |
| 2 | `ui/chart_widgets.rs` | 166 |
| 3 | `ui/design_preview_pane.rs` | 151 |
| 4 | `ui/watchlist_panel.rs` | 109 |
| 5 | `ui/style.rs` | 103 |
| 6 | `ui/widgets/form.rs` | 71 |
| 7 | `ui/widgets/pane.rs` | 67 |
| 8 | `ui/widgets/status.rs` | 59 |
| 9 | `ui/widgets/toolbar/top_nav.rs` | 58 |
| 10 | `ui/widgets/inputs.rs` | 46 |

> Note: `design_inspector.rs` and `design_preview_pane.rs` are dev tools — their literals are intentional showcase code. Deprioritize them unless theme-responsiveness is desired for the inspector itself.  
> Note: `style.rs` literals are mostly theme-definition constants (`pub static TEXT_PRIMARY`, `pub const COLOR_AMBER`) — these are the **source of truth**, not bugs. Only inline-computed fallbacks in `style.rs` need fixing.

---

## 2. File-by-File Table

### 2a — Panels & Panes

| File | Color32 | Stroke | Radius | Size | Frame | Spacing | Notes |
|------|---------|--------|--------|------|-------|---------|-------|
| `ui/watchlist_panel.rs` | 26 | 17 | 0 | 39 | 2 | 42 | Largest panel; spacing dominant |
| `ui/discord_panel.rs` | 20 | 0 | 0 | 11 | 0 | 13 | Many `egui::vec2` |
| `ui/dom_panel.rs` | 5 | 5 | 0 | 0 | 1 | 17 | Spacing-heavy |
| `ui/portfolio_pane.rs` | 4 | 0 | 0 | 0 | 0 | 17 | `egui::vec2` dominant |
| `ui/rrg_panel.rs` | 15 | 5 | 0 | 0 | 0 | 7 | Canvas-adjacent, some Color32 |
| `ui/spreadsheet_pane.rs` | 0 | 5 | 0 | 0 | 0 | 13 | Pure spacing |
| `ui/script_panel.rs` | 5 | 7 | 0 | 3 | 1 | 11 | Mixed |
| `ui/plays_panel.rs` | 11 | 6 | 0 | 3 | 0 | 23 | Spacing+Color32 |
| `ui/indicator_editor.rs` | 0 | 0 | 0 | 4 | 0 | 17 | Size+spacing |
| `ui/settings_panel.rs` | 4 | 5 | 0 | 0 | 1 | 10 | Light |
| `ui/screenshot_panel.rs` | 0 | 0 | 0 | 8 | 0 | 13 | Size literals |
| `ui/spread_panel.rs` | 0 | 0 | 0 | 0 | 0 | 11 | Spacing only |
| `ui/option_quick_picker.rs` | 0 | 0 | 0 | 0 | 0 | 7 | Light |
| `ui/hotkey_editor.rs` | 0 | 0 | 0 | 4 | 0 | 8 | Light |
| `ui/journal_panel.rs` | 0 | 0 | 0 | 5 | 0 | 9 | Light |
| `ui/object_tree.rs` | 7 | 0 | 0 | 13 | 1 | 7 | Size+Color32 |
| `ui/apex_diagnostics.rs` | 16 | 0 | 0 | 0 | 1 | 0 | Color32 only |
| `ui/command_palette/mod.rs` | 13 | 0 | 0 | 0 | 1 | 0 | Mostly Color32 |
| `ui/command_palette/render.rs` | 0 | 0 | 0 | 0 | 0 | 9 | Spacing |
| `ui/chart_widgets.rs` | 86 | 39 | 0 | 0 | 0 | 43 | **Massive** — mixed UI chrome + canvas-adjacent |
| `ui/style.rs` | 32 | 27 | 7 | 9 | 0 | 31 | Definitions (see note above) |

### 2b — Components & Components_Extra

| File | Color32 | Stroke | Radius | Size | Spacing | Notes |
|------|---------|--------|--------|------|---------|-------|
| `components_extra/top_nav.rs` | 12 | 8 | 2 | 0 | 13 | Superseded by `widgets/toolbar/top_nav.rs` — likely dead code |
| `components_extra/inputs.rs` | 5 | 9 | 2 | 0 | 8 | Old inputs layer |
| `components_extra/chips.rs` | 0 | 7 | 4 | 0 | 8 | Still in use? |
| `components_extra/action_button.rs` | 7 | 4 | 0 | 0 | 8 | |
| `components_extra/dom_action.rs` | 6 | 4 | 3 | 0 | 0 | |
| `components_extra/panels.rs` | 0 | 0 | 0 | 0 | 6 | Light |
| `components_extra/toasts.rs` | 0 | 0 | 0 | 0 | 5 | |
| `components/frames.rs` | 3 | 8 | 0 | 0 | 5 | Uses `current()` but still has literals |
| `components/pills.rs` | 5 | 6 | 5 | 0 | 3 | Radius-heavy |
| `components/headers.rs` | 0 | 0 | 0 | 0 | 7 | |
| `components/metrics.rs` | 0 | 0 | 0 | 4 | 1 | Size literals |
| `components/hairlines.rs` | 0 | 0 | 0 | 0 | 3 | Light |
| `components/labels.rs` | 0 | 0 | 0 | 0 | 0 | **Clean** |

### 2c — Non-UI Files (in scope)

| File | Color32 | Stroke | Size | Spacing | Notes |
|------|---------|--------|------|---------|-------|
| `design_inspector.rs` | 154 | 33 | 72 | 38 | Dev-tool — intentional literals; deprioritize |
| `native_main.rs` | 0 | 0 | 0 | 0 | **Clean** |

---

## 3. Widgets Internals — Specifically Called Out

These are extracted widgets that **still contain hardcoded literals internally**, breaking the promise of extraction.

### Core Widgets

| Widget File | Hardcoded Count | Primary Pattern | Notes |
|-------------|----------------|-----------------|-------|
| `widgets/form.rs` | **71** | Color32 (36) + Spacing (21) | Widget defaults hardcode theme colors; `unwrap_or(Color32::from_rgb(...))` should call `current().accent` etc. |
| `widgets/pane.rs` | **67** | Color32 (35) + Spacing (11) + RichColor (22) | All `struct` defaults bypass `current()`. `PaneHeader`, `OrderPane`, etc. |
| `widgets/status.rs` | **59** | Color32 (32) + Spacing (16) | `LoadingSpinner`, `ProgressBar`, `Toast` all hardcode colors in `Default` impls |
| `widgets/toolbar/top_nav.rs` | **58** | RichColor (43) + Spacing (28) | Most literals are `egui::vec2` sizing and `.color(t.dim)` — uses theme but raw `egui::vec2` |
| `widgets/inputs.rs` | **46** | Color32 (27) + Stroke (9) + Spacing (8) | `TextInput`, `NumberInput` — internal border/focus colors hardcoded |
| `widgets/toolbar/mod.rs` | **39** | Color32 (23) + Stroke (6) + Spacing (8) | Tab buttons, group separators |
| `widgets/buttons.rs` | **33** | Color32 (16) + Stroke (11) | `ChromeBtn`, `ActionBtn` — hover/active state colors hardcoded |
| `widgets/pills.rs` | **31** | Color32 (14) + Stroke (7) + Spacing (9) | Badge variants, pill colors |
| `widgets/foundation/shell.rs` | **31** | Color32 (17) + Stroke (7) + Spacing (7) | Foundation — should be cleanest |
| `widgets/frames.rs` | **30** | Stroke (12) + Fill/Stroke (9) + Color32 (10) | Some Frame builders still use literals despite using `current()` |
| `widgets/select.rs` | **28** | Color32 (21) + Spacing (2) + RichColor (21) | Dropdown rendering hardcodes all states |
| `widgets/menus.rs` | **19** | Color32 (9) + Spacing (7) | Menu items, separators |
| `widgets/tabs.rs` | **14** | Color32 (7) + RichColor (5) | Tab active/inactive states |
| `widgets/headers.rs` | **14** | Color32 (11) + RichColor (1) | Panel header widget (thin wrapper) still has literals |
| `widgets/layout.rs` | **12** | Color32 (6) + Spacing (6) | |
| `widgets/text.rs` | **10** | Color32 (9) + Size (1) | Text helpers with embedded colors |
| `widgets/foundation/variants.rs` | **8** | Color32 (8) | Variant→color mapping using literals instead of `current()` |
| `widgets/modal.rs` | **4** | Frame (1) + FillStroke (1) | Light |
| `widgets/foundation/tokens.rs` | **5** | Radius (5) | Token struct — may be intentional |
| `widgets/context_menu.rs` | **3** | FillStroke (3) | Light |
| `widgets/painter_pane.rs` | 0 (spacing) | Spacing (10) | Pure spacing literals |
| `widgets/icons.rs` | 0 | — | **Clean** |
| `widgets/mod.rs` | 0 | — | **Clean** |

### Cards

| Widget File | Hardcoded Count | Notes |
|-------------|----------------|-------|
| `cards/play_card.rs` | **15** | Color32 (5) + Spacing (10) |
| `cards/stat_card.rs` | **7** | Color32 (4) + Spacing (3) |
| `cards/signal_card.rs` | **7** | Color32 (1 from Radius) + Color32 (6) |
| `cards/trade_card.rs` | **5** | Color32 (4) |
| `cards/metric_card.rs` | **5** | Color32 (4) |
| `cards/earnings_card.rs` | **5** | RichColor (5) |
| `cards/news_card.rs` | **3** | RichColor (4) |
| `cards/event_card.rs` | **3** | RichColor (4) |
| `cards/playbook_card.rs` | **1** | Light |
| `cards/mod.rs` | 4 | Frame (2) + Color32 (4) |

### Rows

| Widget File | Hardcoded Count | Notes |
|-------------|----------------|-------|
| `rows/watchlist_row.rs` | **28** | Color32 (19) + Stroke (4) + Spacing (5) — most critical row |
| `rows/dom_row.rs` | **27** | Color32 (7) + Stroke (8) + Spacing (12) |
| `rows/table.rs` | **13** | Color32 (5) + Spacing (8) |
| `rows/news_row.rs` | **9** | Color32 (6) + RichColor (9) |
| `rows/option_chain_row.rs` | **7** | Color32 (5) |
| `rows/order_row.rs` | **6** | Color32 (4) |
| `rows/alert_row.rs` | **6** | Color32 (5) |

### Watchlist Sub-widgets

| Widget File | Hardcoded Count | Notes |
|-------------|----------------|-------|
| `watchlist/nmf_toggle.rs` | **5** | Color32 + Spacing |
| `watchlist/filter_pill.rs` | **5** | Color32 + Size |
| `watchlist/section_header.rs` | **3** | Size (2) |

---

## 4. Repeated Patterns / Extraction Candidates

These patterns appear 3+ times across separate files and are prime extraction candidates:

| Pattern | Occurrences | Files | Candidate Widget |
|---------|------------|-------|-----------------|
| `RichText::new(...).monospace().size(11.0).strong().color(accent)` — panel section title | 17 | `style.rs`, `headers.rs`, `watchlist_panel.rs`, `pane.rs`, `command_palette/render.rs`, others | `widgets::text::SectionLabel` / `widgets::text::PanelTitle` |
| `.monospace().size(10.0).color(t.dim)` — dim secondary label | 4+ | `top_nav.rs`, `watchlist_panel.rs`, `design_preview_pane.rs` | `widgets::text::DimLabel` (exists but not used everywhere) |
| `Color32::from_rgb(120, 120, 130)` — hardcoded `dim` color | 45 | `form.rs`, `pane.rs`, `status.rs`, `buttons.rs`, many | Should always be `current().dim` |
| `Color32::from_rgb(120, 140, 220)` — hardcoded `accent` color | 45 | `form.rs`, `pane.rs`, `status.rs`, `inputs.rs`, many | Should always be `current().accent` |
| `egui::vec2(32.0, 16.0)` / `egui::vec2(16.0, 16.0)` icon-button sizing | 10+ | `top_nav.rs`, `form.rs`, `pane.rs` | Constant: `BTN_ICON_SIZE` / `BTN_SM_SIZE` |
| `Stroke::new(stroke_std(), t.toolbar_border)` — standard border stroke | 20+ | `frames.rs`, `style.rs`, `inputs.rs`, `buttons.rs` | `style::border_stroke()` helper (add to style.rs) |
| `Frame::NONE.fill(t.toolbar_bg).inner_margin(...)` — panel body frame | 15+ | `watchlist_panel.rs`, `dom_panel.rs`, `discord_panel.rs` | `PanelBodyFrame` or use existing `panel_frame()` |
| `.unwrap_or(Color32::from_rgb(120, 140, 220))` — accent fallback in widget structs | 12 | `form.rs`, `pane.rs`, `status.rs` | Replace with `fn default_accent() -> Color32 { current().accent }` |
| `fn ...(accent: Color32, dim: Color32, ...)` signatures passing theme colors as params | 56 callers | `style.rs` helpers, `panels.rs`, `components_extra/*` | Refactor to read `current()` internally; zero-param versions |
| `RichText::new("SECTION").monospace().size(10.0).color(t.dim)` category header | 8+ | `top_nav.rs`, `object_tree.rs`, `watchlist_panel.rs` | `widgets::text::CategoryHeader` |

---

## 5. Prioritized Wave Plan: R4-A through R4-N

Ordered by **leverage** (fixes most sites, highest-reuse components first). Each wave is scoped to ~1 agent session.

---

### R4-A — Widget Default Constructors: `pane.rs` + `form.rs` + `status.rs`
**Sites:** ~197 total  
**Work:** Replace all `Color32::from_rgb(120, 120, 130)` with `current().dim`, `from_rgb(120, 140, 220)` with `current().accent`, `from_rgb(50–60, 50–60, 60–70)` with `current().toolbar_border`, etc. in `Default`/`new()` impls and `unwrap_or(...)` fallbacks.  
**Why first:** These three files are the root cause of the most-repeated literals (90 occurrences of just two hardcoded color values). Widgets that receive theme colors from callers will auto-heal once defaults are correct.

---

### R4-B — Function Signature Purge: `style.rs` color-param helpers
**Sites:** ~56 call-sites + ~30 function definitions  
**Work:** For each `pub fn foo(... accent: Color32, dim: Color32, ...)` in `style.rs` that simply threads colors to inner rendering, add a zero-param version that reads `current()` internally. Deprecate color-param versions or make color params `Option<Color32>` defaulting to `None → current()`.  
**Key functions:** `panel_header`, `panel_header_sub`, `dialog_header`, `dialog_separator`, `section_label`, `section_label_xs`, `dim_label`, `col_header`, `icon_btn`, `close_button`, `form_row`, `stat_row`, `status_badge`, `order_card`, `action_btn`, `tb_btn`.

---

### R4-C — Core Row Widgets: `rows/watchlist_row.rs` + `rows/dom_row.rs`
**Sites:** 55 combined  
**Work:** Replace all inline `Color32::from_rgb(...)` with `current().*` lookups. Replace `egui::vec2(N, M)` with `gap_*()` or named constants. Replace `Stroke::new(N, color)` with `Stroke::new(stroke_*(N), color)`.  
**Why high:** These rows render in every frame at high frequency; theme-correctness here also fixes visual inconsistencies in the two largest panels.

---

### R4-D — Input + Button Widgets: `inputs.rs` + `buttons.rs` + `select.rs`
**Sites:** ~107 combined  
**Work:** Internal focus/hover/border colors in `TextInput`, `NumberInput`, `ChromeBtn`, `ActionBtn`, and the `Select` dropdown all bypass theme. Wire all state-colors to `current()`. Replace `Stroke::new(literal_width, ...)` with `stroke_*()` helpers.

---

### R4-E — Toolbar Chrome: `toolbar/mod.rs` + `toolbar/top_nav.rs`
**Sites:** ~97 combined  
**Work:** `egui::vec2(N, M)` sizing constants (use named consts or `gap_*()`). `.size(N)` literals → `font_*()`. `Stroke::new(N, ...)` → `stroke_*()`. `RichText::new(...).size(N)` patterns in menus/workspace picker.  
**Key win:** `top_nav.rs` has 43 RichColor hits and 28 spacing hits — highest concentration after widget structs.

---

### R4-F — Pills, Chips, Badges: `pills.rs` + `components_extra/chips.rs` + `components/pills.rs`
**Sites:** ~66 combined  
**Work:** Radius literals → `r_sm_cr()` / `r_pill()`. Color literals → theme tokens. Introduce `style::border_stroke()` shorthand for `Stroke::new(stroke_std(), current().toolbar_border)`.

---

### R4-G — Font-Size Literal Sweep (cross-cutting)
**Sites:** ~199 occurrences  
**Work:** Mechanical replacement pass — `.size(10.0)` → `font_sm()`, `.size(11.0)` → `font_md()`, `.size(14.0)` → `font_lg()`, `.size(8.0)` / `.size(9.0)` → `font_xs()` / `font_sm()`. Targets: `watchlist_panel.rs` (39 hits), `object_tree.rs` (13), `discord_panel.rs` (11), `screenshot_panel.rs` (8), `form.rs` (8), `pane.rs` (17 via RichText), `top_nav.rs` (9).  
**Note:** Some intentional size deviations (e.g., `size(7.0)` for tiny category headers) should become `font_xs()` or a new `font_2xs()` token.

---

### R4-H — Spacing Literal Sweep: Panels
**Sites:** ~200 across panel files  
**Work:** Replace `egui::vec2(N, M)` and `Margin::same(N)` with `gap_*()` calls. Top offenders: `design_preview_pane.rs` (52), `watchlist_panel.rs` (42), `chart_widgets.rs` (43), `plays_panel.rs` (23). Focus on panels not covered by R4-C/D/E.

---

### R4-I — Frame Hand-rolls in Panels
**Sites:** ~45 `Frame::NONE` + Frame field hand-rolls  
**Work:** Ensure all `Frame::NONE.fill(t.toolbar_bg).inner_margin(...)` patterns in panels use `panel_frame()` / `panel_frame_compact()` / `PopupFrame`. Files: `watchlist_panel.rs`, `dom_panel.rs`, `discord_panel.rs`, `plays_panel.rs`, plus all panels using `Frame::none()`.

---

### R4-J — Card Widgets: all `cards/*`
**Sites:** ~51 combined  
**Work:** Wire card color literals to `current()`. Replace `unwrap_or_else` fallbacks with theme defaults. Standardize spacing via `gap_*()`. Ensure all cards use `PaneFrame` / `PopupFrame` rather than hand-rolling.

---

### R4-K — Foundation Layer: `foundation/shell.rs` + `foundation/variants.rs` + `foundation/tokens.rs`
**Sites:** ~44 combined  
**Work:** `foundation/variants.rs` maps variant names to `Color32::from_rgb(...)` — these must call `current()` instead. `shell.rs` has 31 hits including Color32, Stroke, and spacing. Fix the foundation layer so all variant consumers auto-inherit.  
**Critical:** This layer is imported by many widgets; fixing it has downstream cascade.

---

### R4-L — Remaining Panel Pass: Color32 in Mid-tier Panels
**Sites:** ~100 across remaining panels  
**Work:** `discord_panel.rs` (20), `apex_diagnostics.rs` (16), `rrg_panel.rs` (15), `command_palette/mod.rs` (13), `object_tree.rs` (11), `plays_panel.rs` (11), `dom_panel.rs` (5), `script_panel.rs` (5), plus minor panels. Replace inline `Color32::from_rgb(...)` with `current().*` or named color constants from `style.rs`.

---

### R4-M — Extract Missing Micro-Widgets
**Sites:** ~8–12 repeated patterns each  
**Work:**  
1. `SectionLabel` / `PanelTitle` — `RichText::new(text).monospace().size(font_md()).strong().color(accent)` appears 17 times. Extract to `widgets::text::SectionLabel::new(title).show(ui)`.  
2. `CategoryHeader` — `.monospace().size(font_xs()).color(t.dim)` for "SECTION" headers in nav/tree views. Extract.  
3. `border_stroke()` — `Stroke::new(stroke_std(), current().toolbar_border)` used 20+ times. Add to `style.rs`.  
4. `BTN_ICON_SM` / `BTN_ICON_MD` — `egui::vec2(16.0, 16.0)` / `egui::vec2(32.0, 24.0)` toolbar button sizes. Add named constants.

---

### R4-N — `chart_widgets.rs` UI-Chrome Layer
**Sites:** 166 total (mixed chart-paint + UI chrome)  
**Work:** This file straddles the chart/UI boundary. Identify the UI-chrome portions (toolbar strips, overlay labels, info panels) vs chart-paint (skip those). Migrate UI-chrome Color32/Stroke/spacing to tokens. Likely ~60–80 sites after excluding canvas-draw paths.  
**Note:** Do this last — requires careful per-line classification to avoid touching the chart paint engine.

---

## 6. Wave Dependencies

```
R4-A (widget defaults)
  └─► R4-C (rows use widget types)
  └─► R4-D (inputs/buttons use widget types)
  └─► R4-J (cards use widget types)

R4-B (function signature purge)
  └─► R4-I (panels call frame helpers)
  └─► R4-L (panels call section_label etc)

R4-K (foundation/variants)
  └─► R4-D (buttons/inputs import variants)
  └─► R4-F (pills/chips import variants)

R4-G (font size sweep) — independent, can run any time after R4-A
R4-H (spacing sweep) — independent, can run in parallel with R4-G
R4-E (toolbar chrome) — depends on R4-A (uses widget defaults), R4-B (uses helpers)
R4-M (extract micro-widgets) — should run after R4-B; its extractions replace the patterns R4-B fixes in helpers
R4-N (chart_widgets) — independent of widget tree, but do last to avoid merge conflicts
```

**Recommended execution order:**
1. R4-K → R4-A → R4-B (foundation first)
2. R4-C + R4-D in parallel
3. R4-E + R4-F in parallel
4. R4-G + R4-H in parallel (mechanical, can be split across sessions)
5. R4-I + R4-J in parallel
6. R4-L → R4-M → R4-N

---

## Appendix: Token Reference

Existing tokens in `style.rs` that should absorb the hardcoded literals:

| Token function | Default value | Replaces |
|----------------|--------------|---------|
| `font_xs()` | 8.0 | `.size(8.0)`, `.size(9.0)` |
| `font_sm()` | 10.0 | `.size(10.0)` |
| `font_md()` | 11.0 | `.size(11.0)` |
| `font_lg()` | 14.0 | `.size(14.0)` |
| `font_xl()` | 15.0 | `.size(15.0)` |
| `gap_xs()` | 2.0 | `Margin::same(2)`, `egui::vec2(2.0, ...)` |
| `gap_sm()` | 4.0 | `Margin::same(4)`, `egui::vec2(4.0, ...)` |
| `gap_md()` | 6.0 | `Margin::same(6)`, `egui::vec2(6.0, ...)` |
| `gap_lg()` | 8.0 | `Margin::same(8)`, `egui::vec2(8.0, ...)` |
| `gap_xl()` | 10.0 | `Margin::same(10)`, `egui::vec2(10.0, ...)` |
| `stroke_hair()` | 0.3 | `Stroke::new(0.3, ...)` |
| `stroke_thin()` | 0.5 | `Stroke::new(0.5, ...)` |
| `stroke_std()` | 1.0 | `Stroke::new(1.0, ...)` |
| `stroke_bold()` | 1.5 | `Stroke::new(1.5, ...)` |
| `stroke_thick()` | 2.0 | `Stroke::new(2.0, ...)` |
| `r_xs()` | varies | `CornerRadius::same(2)` |
| `r_sm_cr()` | varies | `CornerRadius::same(3–4)` |
| `r_md_cr()` | varies | `CornerRadius::same(5–6)` |
| `r_pill()` | varies | `CornerRadius::same(99)` |
| `current().accent` | — | `Color32::from_rgb(120, 140, 220)` ×45 |
| `current().dim` | — | `Color32::from_rgb(120, 120, 130)` ×45 |
| `current().toolbar_bg` | — | `Color32::from_rgb(18–26, 18–20, 24–32)` |
| `current().toolbar_border` | — | `Color32::from_rgb(50–80, 50–72, 60–70)` |
