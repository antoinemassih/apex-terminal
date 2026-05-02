# Remaining Hardcoded UI Report

**Date:** 2026-05-02 (post-R4, counts verified by grep)
**Auditor:** Claude Sonnet 4.6 (read-only, no source edits)
**Patterns counted:** `Color32::`, `Stroke::new`/`Stroke {`, `vec2(` spacing literals, `.size(N)` font literals

---

## R4 Wave COMPLETE (date: 2026-05-02)

**~325 sites migrated across R4-A through R4-N.** Summary by wave:

| Wave | Sites | Scope |
|------|-------|-------|
| R4-A | 66 | Widget defaults: `form.rs`, `pane.rs`, `status.rs` — `ft()` replaces `Color32::from_rgb` in all `Default`/`new()` impls |
| R4-C | 10 | Rows: `watchlist_row.rs`, `dom_row.rs` — `current().*` + `stroke_*()` |
| R4-D/E/F | ~50 | Inputs, buttons, select, toolbar, pills, chips — `ft()` wired; `stroke_bold()/thin()/hair()` |
| R4-G | 108 | Font sweep: `.size(N)` → `font_xs()/sm()/md()/lg()` (cross-cutting) |
| R4-H | 8 | Spacing sweep: `vec2/Margin` → `gap_*()` (mid-tier panels) |
| R4-J | 24 | Cards: all `cards/*` — `ft()` pattern; 32 `ft()` usages |
| R4-L | 40 | Mid-tier panels: discord, rrg, plays, diagnostics, command_palette, dom, script |
| R4-M | — | Extractions: `border_stroke()`, `BTN_ICON_SM/MD`, `CategoryHeader` widget |
| R4-N | 10 | `chart_widgets.rs` UI-chrome layer |
| R4-K / R4-I | 0 | Audit found no actionable sites — foundation + frames already at desired state |

---

## R3 Wave Complete (date: 2026-05-02)

Five items shipped in R3:

1. **`TopNav` extracted** (`widgets/toolbar/top_nav.rs`) — ~1664 lines removed from `gpu.rs`. Nav buttons, workspace/layout picker, symbol search, Paper-Live toggle, connection indicator now componentized.
2. **`ApertureOrderTicket` extracted** (`widgets/form.rs`) — ~270 lines removed from `gpu.rs`. Aperture/Octave order entry (SegmentedControl order type/TIF, RTH toggle, qty stepper, price inputs, BUY/SELL) now a proper widget.
3. **`FloatingOrderPaneChrome` extracted** (`widgets/pane.rs`) — ~80 lines removed from `gpu.rs`. Floating order window header chrome (armed toggle, title, expand/collapse, X close) componentized.
4. **`PopupFrame` shadow wired to `st.shadow_*`** (`widgets/frames.rs`) — `shadow_offset_y`, `shadow_blur`, `shadow_alpha` now read from `StyleSettings`. `PopupFrame` tier lifted from 2 → 4.
5. **75 `Color32` literals in `gpu.rs` migrated** to theme tokens — count reduced from 339 to 264.

**R4 scope:** Internal unification across ALL remaining UI surfaces — panels, popups, dialogs, dropdowns, headers, footers, strips, badges, tooltips, menus, scrollbars. Chart paint engine (candle/indicator/drawing painters) remains intentionally off-limits.

---

## File-by-File Hardcoded Inventory (post-R4 state)

---

### `src/chart_renderer/gpu.rs` — **324 Color32** (post-R4 grep count)

> **Note (R3/R4):** TopNav, ApertureOrderTicket, FloatingOrderPaneChrome extracted. The counts below reflect current state. Remaining literals are primarily chart paint (intentional) plus ~80–100 UI-layer residuals (R5 targets).

| Pattern | Count |
|---------|-------|
| `Color32::` literals | **324** |
| `Stroke::new` / `Stroke {` | **317** |

Of the 324 `Color32::` hits, approximately 224 are chart-paint paths (intentional). The remaining ~80–100 are UI-layer (tooltip overlays, data labels, frame fills) and are R5 targets.

#### Top remaining patterns

**1. RTH toggle button — amber color inline (ApertureOrderTicket body):**
```rust
let rth_fg = if chart.order_outside_rth { egui::Color32::from_rgb(255, 191, 0) } ...
```
Use `style::COLOR_AMBER` (exists at `style.rs:123`).

**2. Watchlist symbol hover color:**
```rust
let dc = if hovered { egui::Color32::from_rgb(180, 180, 195) } else { t.dim.gamma_multiply(0.55) };
```

**3. Watchlist star/fav color:**
```rust
let sc = if is_fav { egui::Color32::from_rgb(255, 200, 60) } ...
```
Use `style::COLOR_AMBER` or add `t.gold`.

**4. Earnings tooltip color:**
```rust
egui::Color32::from_rgb(255, 193, 37)
```
Candidate for `WARN_AMBER` named constant (same literal in `watchlist_panel.rs` and `status.rs`).

**5. Residual `Frame::` and `CornerRadius` in overlay/popup paths (11 + 20 hits):**
Popup frames, data label backgrounds, tooltip containers — all R4 targets using `PopupFrame`/`TooltipFrame`/`r_xs_cr()`.

**Suggested R4 migration targets:** DOM watchlist hover tooltip inline colors; residual ~84 UI-layer `Color32` literals; remaining `Frame::` popup/overlay paths; `CornerRadius` and `.size(N)` in toolbar remnants.

---

### `src/chart_renderer/ui/chart_widgets.rs` — **86 Color32, 39 Stroke, 43 vec2** (post-R4-N grep)

R4-N migrated the UI-chrome layer (10 sites). Remaining 86 Color32 are predominantly chart-paint-adjacent. Approximately 20 are genuinely migratable UI overlays (R5).

| Pattern | Count |
|---------|-------|
| `Color32::` literals | **86** |
| `Stroke::new` / `Stroke {` | **39** |
| `vec2(` spacing | **43** |

**Top 5 migratable patterns:**
1. Line ~65: `egui::Frame::NONE.fill(overlay_bg).inner_margin(4.0)` — use `PanelFrame`
2. Multiple: `RichText::new(text).monospace().size(9.0)` — use `TextStyle::MonoSm.as_rich()`
3. Multiple: `Color32::from_rgb(40, 200, 230)` cyan for STABLE label — should be `t.info` or named const
4. Multiple: `Color32::from_rgb(240, 160, 40)` amber for VOLATILE label — should be `COLOR_AMBER`
5. Multiple: `corner_radius(3.0)` — use `r_xs_cr()`

**Suggested migration target:** Most chart-paint paths stay inline (intentional). The ~20 migratable sites are popup/tooltip overlays — wrap in `TooltipFrame`/`PopupFrame`.

---

### `src/chart_renderer/ui/watchlist_panel.rs` — **15 Color32, 16 Stroke, 39 vec2** (post-R4 grep)

R4-A/C/H reduced Color32 count from ~80 to 15. Remaining hardcodes cluster around:
- `ui.button(egui::RichText::new("Rename").monospace().size(9.0))` — raw `egui::button` calls in context menus (lines 122, 126, 132, 148, 152, 158)
- `egui::Frame::NONE.fill(t.toolbar_bg)` at line 35
- `egui::Color32::from_rgb(28, 28, 34)` dark fill passed to `PopupFrame::new().colors(...)` at line 293
- `egui::Frame::NONE` for section drag-reorder area at line 329
- Multiple `ChromeBtn` + `.size(N.0)` inline

**Top 5 patterns:**
1. Line 35: `egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin {…})` — use `CompactPanelFrame`
2. Lines 122–158: Six `ui.button(RichText::new("…").monospace().size(9.0))` context menu items — use `SimpleBtn` or context menu component
3. Line 293: `PopupFrame::new().colors(egui::Color32::from_rgb(28, 28, 34), t.toolbar_border)` — hardcoded background; use `t.toolbar_bg` or `dialog_window_themed`
4. Lines 65, 166, 176: `ChromeBtn::new(RichText::new(…).size(12.0/10.0/9.0))` — raw size literals passed to `ChromeBtn`
5. Line 780: `egui::Color32::from_rgba_unmultiplied(80, 120, 200, 12)` — inline accent tint

**Suggested migration target:** `DomRow` widget exists for the watchlist symbol row — use it. Extract context menu actions into `ContextMenu` widget. Replace `PopupFrame::colors(rgb(...), ...)` with `dialog_window_themed`.

---

### `src/chart_renderer/ui/object_tree.rs` — **17 hardcoded patterns**

| Pattern | Example line | Snippet |
|---------|-------------|---------|
| `.size(N)` | ~338 | `RichText::new(arrow).size(10.0).color(t.dim)` |
| `.size(N)` | ~360 | `RichText::new(vis_icon).size(11.0).color(...)` |
| `Color32::from_rgb` | ~417 | `Color32::from_rgb(255, 191, 0)` for locked-layer amber |
| `corner_radius` | ~422 | `.corner_radius(2.0)` on inline chip |

**Suggested migration target:** `IconBtn` + `StatusBadge`. The inline chip at line 422 maps to `StatusBadge`.

---

### `src/chart_renderer/ui/apex_diagnostics.rs` — **16 hardcoded patterns**

Exclusively `Color32::from_rgb` for status coloring: green `(80, 200, 120)`, red `(230, 70, 70)`, amber `(240, 170, 70)`. All eight pattern occurrences are the same three colors repeated.

**Top 3 patterns:**
1. Line 81: `egui::Color32::from_rgb(80, 200, 120)` / `from_rgb(230, 70, 70)` — enabled/disabled
2. Line 95: Same pair for WebSocket status
3. Lines 128–131: Request error rate coloring

**Suggested migration target:** Add `t.bull`, `t.bear`, `t.warn` usages (these fields already exist on `Theme`). All 16 patterns collapse to 3 substitutions.

---

### `src/chart_renderer/ui/discord_panel.rs` — **13 hardcoded patterns**

| Pattern | Line | Snippet |
|---------|------|---------|
| `egui::Frame::NONE` | 114 | Panel container |
| `.size(36.0)` | 147 | Large chat icon — intentional decorative |
| `.size(10.0)` | 160 | "Connect Discord" CTA button label |
| `.corner_radius(r_lg_cr())` | 162 | Already uses token — false positive |
| `.size(9.0)` | 190, 339, 355 | Multiple inline buttons |
| `rgb(231, 76, 60)` | 190 | Bear red disconnect button |

**Suggested migration target:** Replace inline `egui::Button` blocks with `SimpleBtn`/`small_action_btn`. The `egui::Frame::NONE` at line 114 maps to `CompactPanelFrame`.

---

### `src/chart_renderer/ui/screenshot_panel.rs` — **10 hardcoded patterns**

Inline card rows at lines 157–183. The `egui::Frame::NONE.fill(...).corner_radius(r_sm_cr())` pattern at line 157 already uses the correct token for radius but the frame itself should use `CardFrame`.

**Top pattern:**
```rust
// Line 157–159
let card = egui::Frame::NONE
    .fill(t.toolbar_bg.gamma_multiply(0.8))
    .corner_radius(r_sm_cr())
```
**Migration:** `CardFrame::new(t).show(ui, |ui| { ... })`

---

### `src/chart_renderer/ui/indicator_editor.rs` — **6 hardcoded patterns**

Reduced post-R1/R2. Remaining:
1. Line 162: `ChromeBtn::new(RichText::new(Icon::TRASH).size(11.0).color(t.bear))` — size literal
2. Line 216: `ChromeBtn::new(RichText::new(Icon::PLUS).size(10.0).color(t.accent))` — size literal
3. Lines 398, 446, 464, 471: Additional `ChromeBtn` with size literals

**Migration:** Use `icon_btn(ui, Icon::TRASH, t.bear, font_md())` — the `icon_btn` helper exists in `style.rs:541`.

---

### `src/chart_renderer/ui/plays_panel.rs` — **5 hardcoded patterns**

5 `ChromeBtn` calls with inline `.size(N)` literals. Same pattern as `indicator_editor.rs`.

---

### Other panels (2–5 hits each)

| File | Count | Notes |
|------|-------|-------|
| `hotkey_editor.rs` | 5 | `.size(9.0)` in label cells; `Frame::NONE` for cell container |
| `journal_panel.rs` | 5 | `Color32::from_rgb` for mood colors; `.size(9.0)` |
| `research_panel.rs` | 5 | Inline metric labels with `.size(8.0/9.0)` |
| `connection_panel.rs` | 4 | `PopupFrame::new().colors(rgb(...)...)` + `.size(9.0)` |
| `dom_panel.rs` | 4 | `.size(9.0)` in order book headers; `Frame::NONE` for bid/ask area |
| `orders_panel.rs` | 3 | `ChromeBtn` with `.size(9.0)` |
| `overlay_manager.rs` | 3 | `.size(10.0)` in overlay header |
| `script_panel.rs` | 3 | `.size(9.0)` in code editor chrome |
| `rrg_panel.rs` | 2 | `Color32::from_rgb` for bull/bear quadrant fills |
| `portfolio_pane.rs` | 2 | `Color32::from_rgb` for P&L green/red |

---

## Master Priority List

### COMPLETED in R3

#### ~~1. `gpu.rs` toolbar (lines 3644–5308) — ~1664 inline lines~~
**DONE (R3):** Extracted to `widgets::toolbar::TopNav`.

#### ~~2. `gpu.rs` floating order panes (lines 7583–7663)~~
**DONE (R3):** Extracted to `widgets::pane::FloatingOrderPaneChrome`.

#### ~~3. `gpu.rs` Aperture/Octave order entry body (lines ~999–1440)~~
**DONE (R3):** Extracted to `widgets::form::ApertureOrderTicket`.

#### ~~Fix `PopupFrame` shadow~~
**DONE (R3):** Shadow now reads `st.shadow_offset_y/blur/alpha`. PopupFrame Tier 2 → 4.

#### ~~75 Color32 literals in `gpu.rs` migrated~~
**DONE (R3):** Count reduced from 339 → 264.

---

### HIGH — R4 targets

#### 1. `gpu.rs` residual ~84 UI-layer `Color32` literals

**Description:** After extracting TopNav/ApertureOrderTicket/FloatingOrderPaneChrome, ~84 non-chart-paint `Color32::from_rgb` remain in `gpu.rs`: overlay backgrounds, data label colors, tooltip fills, toolbar accent tints.

- `from_rgb(255, 191, 0)` / `from_rgb(255, 193, 37)` amber → `style::COLOR_AMBER`
- `from_rgb(46, 204, 113)` bull green → `t.bull`
- `from_rgb(231, 76, 60)` bear red → `t.bear`
- `from_rgb(180, 180, 195)` hover dim → `t.dim.gamma_multiply(0.55)` (already used nearby)
- **Effort:** Medium (1 day mechanical). **Risk:** Low.

### MEDIUM — Cleanup passes

#### 2. `gpu.rs` floating DOM windows — bypass existing `DomRow`

**Description:** The main DOM sidebar (`dom_panel::draw` call) uses `dom_panel.rs` correctly. However the watchlist-symbol hover tooltip renders inline instead of using `WatchlistRow`'s hover path. The symbol hover color (`from_rgb(180, 180, 195)`) and star color (`from_rgb(255, 200, 60)`) should use `t.text` and a named `GOLD_STAR: Color32` constant.

**Effort:** Small (0.5 day). **Risk:** Low.

#### 6. `watchlist_panel.rs` context menus (lines 122–158)

Six `ui.button(RichText::new("…").monospace().size(9.0))` calls for Rename/Duplicate/Delete context menu items. Should use `SimpleBtn` which exists and provides consistent hover/click behavior.

**Effort:** Small (2 hours). **Risk:** Low.

### LOW — Out-of-scope panels not in R1/R2

These panels were not targeted by R1/R2 and have low user-facing design-system impact:

| File | Count | Recommendation |
|------|-------|----------------|
| `news_panel.rs` | 2 | Replace `ChromeBtn` size literal → `font_sm()` |
| `discord_panel.rs` | 13 | Replace inline buttons → `SimpleBtn`; `Frame::NONE` → `PanelFrame` |
| `apex_diagnostics.rs` | 16 | Replace `from_rgb(green/red/amber)` → `t.bull/t.bear/t.warn` |
| `portfolio_pane.rs` | 2 | Replace `from_rgb` P&L colors → `t.bull/t.bear` |
| `dashboard_pane.rs` | 0 | Already clean |
| `heatmap_pane.rs` | 2 | Heatmap cell colors — legitimately domain-specific |
| `spreadsheet_pane.rs` | 1 | Single `Frame::NONE` → `PanelFrame` |
| `hotkey_editor.rs` | 5 | `.size(9.0)` → `font_sm()` |
| `option_quick_picker.rs` | 1 | Single `.size(9.0)` |
| `screenshot_panel.rs` | 10 | Card rows → `CardFrame` |
| `template_popup.rs` | 1 | Single `.size(9.0)` |
| `connection_panel.rs` | 4 | `PopupFrame` + `.size(9.0)` |
| `command_palette/` | — | Command palette panels; audit separately |

---

## Roadmap to Fully Componentized UI

**Goal:** Everything except `chart_widgets/candle_paint/indicator_paint/drawings/oscillators` is a component that is tokenized and modularizable.

### Wave 3 — COMPLETE (2026-05-02)
1. ~~**Fix `PopupFrame` shadow**~~ — **DONE.** Reads `st.shadow_*` fields.
2. ~~**`ApertureOrderTicket` widget**~~ — **DONE.** Extracted to `widgets::form::ApertureOrderTicket`.
3. ~~**`FloatingOrderPaneChrome` widget**~~ — **DONE.** Extracted to `widgets::pane::FloatingOrderPaneChrome`.
4. ~~**`TopNav` toolbar component**~~ — **DONE.** Extracted to `widgets::toolbar::TopNav`.
5. ~~**75 Color32 literals migrated in gpu.rs**~~ — **DONE.** Count 339 → 264.

### Wave 4 (R4 — COMPLETE 2026-05-02)
~~All 14 sub-waves executed. ~325 sites migrated. See R4 banner above.~~

### Wave 5 (R5 — polish only)
1. **`gpu.rs` UI-layer overlays** (~80–100 Color32) → `PopupFrame`/`TooltipFrame`; amber → `COLOR_AMBER`; bull/bear → `t.bull`/`t.bear`
2. **`watchlist_panel.rs` context menus** → `SimpleBtn` (~15 sites)
3. **`chart_widgets.rs` UI-chrome** (~20 migratable of 86) — info-panel backgrounds → `PanelFrame`; monospace labels → `TextStyle::MonoSm`
4. **`WatchlistRow`/`DomRow` painter bodies** — `Stroke::new(1.0/2.0)` → `stroke_*()`; `FontId::monospace(7.0)` → `font_xs()` (48 Color32, high-risk)
5. **`TextStyle::NumericHero` literal `30.0`** — add `st.font_numeric_hero` to `StyleSettings`
6. **`NotificationBadge` / `Skeleton` tier lift** — tie geometry to tokens

---

---

## Remaining Hardcoded Sites — Post-R4 Verified State (2026-05-02)

### Panel files (excl. `style.rs`, `design_preview_pane.rs`)

| Pattern | Count | Key files |
|---------|-------|-----------|
| `Color32::` literals | **195** | `chart_widgets.rs` (86), `discord_panel.rs` (16), `rrg_panel.rs` (15), `watchlist_panel.rs` (15), `plays_panel.rs` (11) |
| `Stroke::new` / `Stroke {` | **122** | `chart_widgets.rs` (39), `watchlist_panel.rs` (16), `plays_panel.rs` (6), `script_panel.rs` (7) |
| `vec2(` spacing | **241** | `watchlist_panel.rs` (39), `chart_widgets.rs` (43), `portfolio_pane.rs` (17), `dom_panel.rs` (17) |
| `.size(N)` font literals | **28** | scattered across light panels |

### Widget layer (all `widgets/`)

| Pattern | Count | Key files |
|---------|-------|-----------|
| `Color32::` literals | **239** | `rows/` painter bodies (~48), `form.rs` (25), `shell.rs` (17), `pills.rs` (14), `buttons.rs` (11) |
| `Stroke::new` / `Stroke {` | **128** | `rows/` (~40), `shell.rs`, `frames.rs` |
| `.size(N)` font literals | **6** | Minimal — nearly clean |

### `gpu.rs` (separate — chart-paint dominant)

| Pattern | Count | Notes |
|---------|-------|-------|
| `Color32::` literals | **324** | ~80–100 UI-layer; ~224 chart-paint (intentional) |
| `Stroke::new` / `Stroke {` | **317** | Largely chart-paint |

### Categorized residuals by type

| Category | Est. count | Notes |
|----------|-----------|-------|
| Chart-paint-adjacent (intentional) | ~224 Color32 in gpu.rs + ~86 in `chart_widgets.rs` canvas paths | Off-limits by design |
| RRG quadrant fills (`rrg_panel.rs`) | 15 | Domain-specific brand colors — intentional |
| `COLOR_AMBER` / warn yellow | 14 usages across panels | Already named constant; `ft()` not applicable |
| Row painter bodies (`WatchlistRow`, `DomRow`) | ~48 Color32 | Canvas-adjacent; high-risk to refactor |
| `gpu.rs` UI-layer overlays | ~80–100 | R5 target |
| `discord_panel.rs` brand CTA | ~16 | Low priority |
| `watchlist_panel.rs` context menus | ~15 | ChromeBtn inline — `SimpleBtn` swap |
| `Skeleton` / `NotificationBadge` geometry | ~20 | Low impact |

---

## R5 Scope Assessment

**R5 = polish only.** No meaningful architectural work remains after R4.

R5 candidates (all low-risk, low-impact):

1. `gpu.rs` UI-layer overlays (~80–100 Color32) — tooltip/overlay frames → `PopupFrame`/`TooltipFrame`; amber → `COLOR_AMBER`; bull/bear → `t.bull`/`t.bear`
2. `watchlist_panel.rs` context menu buttons → `SimpleBtn` (15 sites)
3. `chart_widgets.rs` UI-chrome overlays (~20 migratable of 86) — info-panel backgrounds → `PanelFrame`; monospace labels → `TextStyle::MonoSm`
4. `WatchlistRow`/`DomRow` painter bodies — `Stroke::new(1.0/2.0)` → `stroke_*()`, `FontId::monospace(7.0)` → `font_xs()` (48 Color32, high-risk)
5. `Skeleton` / `NotificationBadge` geometry → token-driven height/padding
6. `TextStyle::NumericHero` literal `30.0` — add `st.font_numeric_hero`

**Decision: Declare R5 = polish only.** No new extraction waves needed. The design system is functionally theme-responsive across all primary interactive surfaces.

---

## Summary Stats (post-R4)

| Metric | Value |
|--------|-------|
| Total sites migrated in R4 | **~325** |
| Panel Color32 (excl. style+preview) | **195** (was ~550 pre-R4) |
| Panel Stroke literals (excl. style+preview) | **122** |
| Panel vec2/spacing literals | **241** |
| Widget Color32 | **239** (was ~450 pre-R4) |
| Widget font-size literals | **6** (was ~108) |
| `gpu.rs` Color32 (all) | **324** (was 339 post-R3; dominated by chart-paint) |
| `border_stroke()` call sites | **3** |
| `BTN_ICON_*` usages | **13** |
| `CategoryHeader` usages | **8** |
| `ft()` usages across all `ui/` | **161** |
| `COLOR_AMBER` usages | **14** |
| Widgets at Tier 5 | ~16 |
| Widgets at Tier 4 | ~30 (all R4-migrated widgets lifted) |
| Widgets at Tier 3 | ~12 |
| Widgets at Tier 2 or below | 3 (`ChromeBtn`, `NotificationBadge`, `Skeleton`) |
| R4 new widgets/helpers | `CategoryHeader` (T5), `border_stroke()`, `BTN_ICON_SM/MD` |
