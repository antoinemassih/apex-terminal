# Remaining Hardcoded UI Report

**Date:** 2026-04-30 (post-R1/R2 refactor, ~233 sites migrated)
**Auditor:** Claude Sonnet 4.6 (read-only, no source edits)
**Patterns counted:** `Color32::from_rgb`, `egui::Frame::*`, `CornerRadius::same`, `.corner_radius(N` literal, `.size(N)` literal, raw `egui::Button` with inline chrome

---

## File-by-File Hardcoded Inventory

---

### `src/chart_renderer/gpu.rs` — **289 hardcoded patterns**

| Pattern | Count |
|---------|-------|
| `Color32::from_rgb` | 161 |
| `egui::Frame::` | 17 |
| `CornerRadius::same` / `.corner_radius(N)` | 27 |
| `.size(N)` literal | 84 |

#### Top 10 patterns (line numbers + snippets)

**1. Toolbar frame (line 3697) — hardcoded `Frame::NONE` with literal margins:**
```rust
.frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin { left: 8, right: 0, top: 0, bottom: 0 }))
```

**2. Toolbar auto-hide hint (line 3689):**
```rust
.frame(egui::Frame::NONE.fill(t.accent))
```

**3. Floating order pane window (lines 7593–7596) — inline `egui::Window` frame:**
```rust
.frame(egui::Frame::popup(&ctx.style())
    .fill(t.toolbar_bg).inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
    .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 100)))
    .corner_radius(4.0))
```

**4. Floating order pane header fill (line 7607):**
```rust
painter().rect_filled(..., egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, color_alpha(t.toolbar_border, 30));
```

**5. RTH toggle button (lines 1041–1045) — amber color inline:**
```rust
let rth_fg = if chart.order_outside_rth { egui::Color32::from_rgb(255, 191, 0) } ...
```
(Should use `style::COLOR_AMBER` which already exists at `style.rs:123`)

**6. Watchlist symbol hover color (line 5033):**
```rust
let dc = if hovered { egui::Color32::from_rgb(180, 180, 195) } else { t.dim.gamma_multiply(0.55) };
```

**7. Watchlist star/fav color (line 5039):**
```rust
let sc = if is_fav { egui::Color32::from_rgb(255, 200, 60) } ...
```

**8. Toolbar popup frame (line 4980–4984):**
```rust
.frame(egui::Frame::popup(&ctx.style())
    ...
    .corner_radius(6.0))
```

**9. Earnings tooltip color (line 5288):**
```rust
egui::Color32::from_rgb(255, 193, 37)
```
(Same literal exists in `watchlist_panel.rs:293` and `status.rs` — candidate for `WARN_AMBER` named constant)

**10. Aperture/Octave order entry inline buttons (lines 1110, 1164):**
```rust
.stroke(egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 90))).corner_radius(2.0)
```

**Suggested migration target:** Extract `widgets::toolbar::TopNav` component (~lines 3644–5308); extract `widgets::order::FloatingOrderPane` for floating window chrome (lines 7583–7663); migrate Aperture/Octave order entry to a second `ApertureOrderTicket` widget analogous to `MeridienOrderTicket`.

---

### `src/chart_renderer/ui/chart_widgets.rs` — **65 hardcoded patterns**

Many are chart-paint-adjacent (rendered via `Painter` onto canvas — cannot use widget tokenization). Approximately 20 are genuinely migratable UI overlays.

| Pattern | Count |
|---------|-------|
| `Color32::from_rgb` | ~30 |
| `.size(N)` literal | ~25 |
| `egui::Frame::` / corner radius | ~10 |

**Top 5 migratable patterns:**
1. Line ~65: `egui::Frame::NONE.fill(overlay_bg).inner_margin(4.0)` — use `PanelFrame`
2. Multiple: `RichText::new(text).monospace().size(9.0)` — use `TextStyle::MonoSm.as_rich()`
3. Multiple: `Color32::from_rgb(40, 200, 230)` cyan for STABLE label — should be `t.info` or named const
4. Multiple: `Color32::from_rgb(240, 160, 40)` amber for VOLATILE label — should be `COLOR_AMBER`
5. Multiple: `corner_radius(3.0)` — use `r_xs_cr()`

**Suggested migration target:** Most chart-paint paths stay inline (intentional). The ~20 migratable sites are popup/tooltip overlays — wrap in `TooltipFrame`/`PopupFrame`.

---

### `src/chart_renderer/ui/watchlist_panel.rs` — **46 hardcoded patterns**

Post-R1/R2 the `FilterPill`, `SectionHeader`, `NmfToggle` components reduced this count from ~80. Remaining hardcodes cluster around:
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

### HIGH — Extract components

#### 1. `gpu.rs` toolbar (lines 3644–5308) — ~1664 inline lines

**Description:** The entire top toolbar — nav buttons, workspace picker, layout picker, symbol search, account strip toggle, Paper-Live toggle, toasts, connection indicator, right-side icon buttons — is rendered inline in `render_toolbar()`. `ToolbarBtn` exists as a thin delegating wrapper (`widgets/toolbar.rs`), but the full toolbar orchestration remains in `gpu.rs`.

**Proposed component:** `widgets::toolbar::TopNav`
- Signature: `TopNav::new().panes(panes).active(active_pane).watchlist(watchlist).theme(t).show(ctx)`
- Internal delegation: `ToolbarBtn` (already exists), `SegmentedControl`, `SearchInput`, `AccountStrip` (already exists), `ConnectionIndicator`
- **Effort:** Large (3–5 days). Toolbar logic is tightly coupled to pane state mutations.
- **Risk:** Medium. Toolbar touches `active_pane`, `layout`, `conn_panel_open` state. Must pass `&mut` refs cleanly. Visual parity is straightforward (no chart paint).

#### 2. `gpu.rs` floating order panes (lines 7583–7663)

**Description:** Floating `egui::Window` with inline header (armed toggle, title, position indicator, expand/collapse, X) and body (delegates to `render_order_entry_body`). The header chrome is ~60 lines of raw `egui::Button` + `egui::RichText`.

**Proposed component:** `widgets::order::FloatingOrderPane`
- Use `dialog_window_themed` for the window frame
- Use `icon_btn` for close/expand controls
- Use `style::panel_header` pattern for the header row
- **Effort:** Small (0.5 day).
- **Risk:** Low. Window dragging is isolated. No chart paint.

#### 3. `gpu.rs` Aperture/Octave order entry body (lines ~999–1440)

**Description:** After the `MeridienOrderTicket` early-return at line 998, the function falls through to ~440 lines of inline Aperture/Octave order form: order type SegmentedControl, TIF SegmentedControl, RTH toggle, qty stepper, price inputs, BUY/SELL buttons. No widget component wraps this path.

**Proposed component:** `widgets::form::ApertureOrderTicket`
- Mirrors `MeridienOrderTicket` API — takes `OrderTicketState`, returns `OrderTicketOutcome`
- Internal: `SegmentedControl`, `NumericInput`, `Stepper`, `trade_btn`
- The RTH toggle inline button at line 1041–1045 replicates `COLOR_AMBER` which already exists in `style.rs:123`
- **Effort:** Medium (1–2 days).
- **Risk:** Low for Aperture style; Medium for Octave (density differences). Must match existing visual output exactly.

### MEDIUM — Cleanup passes

#### 4. `gpu.rs` floating DOM windows — bypass existing `DomRow`

**Description:** The main DOM sidebar (`dom_panel::draw` call at line 5829) uses `dom_panel.rs` correctly. However the watchlist-symbol hover tooltip at lines 5033–5039 renders inline instead of using `WatchlistRow`'s hover path. The symbol hover color (`from_rgb(180, 180, 195)` at line 5033) and star color (`from_rgb(255, 200, 60)` at line 5039) should use `t.text` and a named `GOLD_STAR: Color32` constant.

**Effort:** Small (0.5 day). **Risk:** Low.

#### 5. `gpu.rs` 161 remaining `Color32::from_rgb` patterns

Of 161 occurrences, approximately 80 fall inside the chart painter (candle paint, indicator paint, drawings — intentionally hardcoded). The remaining ~80 are in toolbar, overlays, and data labels and should be migrated:
- `from_rgb(255, 191, 0)` / `from_rgb(255, 193, 37)` amber → use `style::COLOR_AMBER` (already defined)
- `from_rgb(46, 204, 113)` bull green → use `t.bull`
- `from_rgb(231, 76, 60)` bear red → use `t.bear`
- `from_rgb(40, 200, 230)` cyan → add `t.info` or a named const

**Effort:** Medium (1 day mechanical search-replace). **Risk:** Low. No visual change; purely naming.

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

### Wave 3 (immediate — < 1 week)
1. **Fix `PopupFrame` shadow** (`widgets/frames.rs:281–285`) — replace hardcoded shadow with `st.shadow_*` fields. **Zero visual change on Aperture/Octave (shadows already match); Meridien shadow was NONE anyway.** (0.5 hr)
2. **`ALPHA_*` → `alpha_*()` in `WatchlistRow`/`DomRow`** — mechanical replace; no visual change at default design-token values. (1 hr)
3. **`COLOR_AMBER` for amber literals** — `gpu.rs` lines 1041–1045, 5039, 5288; `watchlist_panel.rs`; `status.rs` warn yellow → `COLOR_AMBER` or `t.warn`. (1 hr)
4. **`t.bull/t.bear`** for green/red literals in `apex_diagnostics.rs`, `portfolio_pane.rs`, `plays_panel.rs`. (0.5 hr)

### Wave 4 (short-sprint — 1–2 weeks)
5. **`ApertureOrderTicket` widget** — wrap Aperture/Octave order entry body (lines 999–1440 in `gpu.rs`) into `widgets::form::ApertureOrderTicket`. Mirrors `MeridienOrderTicket`. Reduces `gpu.rs` by ~440 lines.
6. **`FloatingOrderPane` widget** — wrap floating order window header/chrome (lines 7583–7663). Reduces `gpu.rs` by ~80 lines.
7. **Fix button stroke literals** — `TradeBtn`, `SimpleBtn`, `SmallActionBtn`, `ActionBtn`: `Stroke::new(1.0/1.5, ...)` → `stroke_bold()`. (2 hrs)
8. **`IconBtn` size literals** → `font_sm()/font_md()/font_lg()`. (1 hr)
9. **`watchlist_panel.rs` context menus** → `SimpleBtn`. (2 hrs)
10. **`screenshot_panel.rs` card rows** → `CardFrame`. (1 hr)

### Wave 5 (major sprint — 2–4 weeks)
11. **`TopNav` toolbar component** — extract `render_toolbar()` from `gpu.rs` into `widgets::toolbar::TopNav`. Largest single item. Reduces `gpu.rs` by ~1664 lines.
12. **`WatchlistRow`/`DomRow` painter body cleanup** — replace all `ALPHA_*` constants, `Stroke::new(1.0/2.0, ...)` literals, `FontId::monospace(7.0/9.0)` with token calls. Painter body stays inline but literals go.
13. **`discord_panel.rs` full migration** — `PanelFrame`, `SimpleBtn`, `ChromeBtn` cleanup.
14. **`object_tree.rs`** — `IconBtn` + `StatusBadge` replacements.

### Wave 6 (polish — ongoing)
15. **`TextStyle::NumericHero` literal `30.0`** — add `st.font_numeric_hero` to `StyleSettings`.
16. **`NotificationBadge` / `Skeleton` tier lift** — tie geometry to tokens.
17. **Warn yellow named constant** — `style::WARN_YELLOW: Color32` or `t.warn` field for cross-widget consistency.
18. **`CornerRadius::same(st.r_xs as u8)` truncating casts** → `Radius::Xs.corner()` in `KeybindChip`, `Stepper`, `DialogHeader`.

---

## Summary Stats

| Metric | Value |
|--------|-------|
| Total hardcoded patterns across all non-widget source files | ~500 |
| Patterns in `gpu.rs` alone | 289 |
| Patterns confirmed chart-paint-adjacent (intentional) | ~80 |
| Migratable patterns remaining | ~420 |
| Widgets at Tier 5 | 14 |
| Widgets at Tier 4 | 21 |
| Widgets at Tier 3 | 22 |
| Widgets at Tier 2 or below | 8 |
| R1/R2 new widgets (all Tier 4+) | 8 |
