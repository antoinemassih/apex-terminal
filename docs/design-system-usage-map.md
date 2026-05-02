# Design System Usage Map

**Audit date:** 2026-05-02 (refreshed post-R5)
**Scope:** `src/chart_renderer/ui/` — all panel files + `gpu.rs`
**Excluded from hardcoded audit:** `widgets/`, `components/`, `components_extra/`, `foundation/`, `style.rs`, `design_inspector.rs`, `design_preview_pane.rs`

---

## Phase 1 — Widget Usage Inventory

### Buttons Family

#### IconBtn
| File | Usages |
|------|--------|
| `object_tree.rs` | 8 |
| `overlay_manager.rs` | 2 |
| `spread_panel.rs` | 5 |
| **Total** | **~17 usages across 3 files** |

#### TradeBtn
| File | Usages |
|------|--------|
| `gpu.rs` (order entry body) | 2 (lines 1200, 1235 — BUY/SELL in floating order entry) |
| `dom_panel.rs` | 2 (lines 208, 233) |
| `spread_panel.rs` | 1 |
| **Total** | **5 usages across 3 files** |

#### SimpleBtn
| File | Usages |
|------|--------|
| `command_palette/render.rs` | 1 |
| `connection_panel.rs` | 1 |
| `dom_panel.rs` | 5 |
| `hotkey_editor.rs` | 1 |
| `object_tree.rs` | 1 |
| `overlay_manager.rs` | 1 |
| `scanner_panel.rs` | 2 |
| `screenshot_panel.rs` | 1 |
| `spread_panel.rs` | 6 |
| `spreadsheet_pane.rs` | 2 |
| `trendline_filter.rs` | 4 |
| **Total** | **~25 usages across 11 files** |

#### ChromeBtn
| File | Usages |
|------|--------|
| `watchlist_panel.rs` | ~40 |
| `plays_panel.rs` | 8 |
| `discord_panel.rs` | 6 |
| `orders_panel.rs` | 4 |
| `alerts_panel.rs` | 2 |
| `analysis_panel.rs` | 3 |
| `news_panel.rs` | 1 |
| `indicator_editor.rs` | 6 |
| `hotkey_editor.rs` | 2 |
| `feed_panel.rs` | 2 |
| `gpu.rs` (toolbar/Paper-Live) | 2 |
| **Total** | **~76 usages across 11 files** |

#### ActionBtn / SmallActionBtn
No usages found outside widget definition files.

---

### Pills Family

#### PillButton
| File | Usages |
|------|--------|
| `widgets/pane.rs` (PaneSymbolBadge) | 1 |
| `widgets/cards/playbook_card.rs` | 1 |
| **Total** | **2 usages (internal only)** |

---

### R5 New Theme Token Usage Counts

| Token | Primary consumers | Verified usages |
|-------|------------------|-----------------|
| `t.warn` | `status.rs`, `dom_panel.rs`, `dom_action.rs`, `apex_diagnostics.rs` | 4 files (≥12 call sites) |
| `t.notification_red` | `status.rs`, `dom_action.rs`, `command_palette/mod.rs` | 3 files |
| `t.gold` | `watchlist_row.rs`, `plays_panel.rs` | 2 files |
| `t.shadow_color` | `play_card.rs`, shadow paint paths | 1 widget file |
| `t.overlay_text` | `chart_widgets.rs`, `design_preview_pane.rs` | 2 files |
| `t.rrg_leading/improving/weakening/lagging` | `rrg_panel.rs` | 19 usages (4 tokens × quadrant fills) |
| `t.cmd_palette[*]` | `command_palette/mod.rs`, `render.rs`, `execute.rs` | 52 usages in mod.rs alone |
| **Total new token usages** | Across 16 consumer files | **~102 call sites** |

> Count verified: `grep -rn "t\.warn|t\.notification_red|t\.gold|t\.shadow_color|t\.overlay_text|t\.rrg_|cmd_palette"` returned 102 hits across 16 files.

---

### R4 New Helpers and Widget Usage Counts

| Helper / Widget | Location | Usages | Wave |
|----------------|----------|--------|------|
| `style::border_stroke()` | `style.rs` | 3 call sites (replaces ~20+ `Stroke::new(stroke_std(), t.toolbar_border)` hand-rolls) | R4-M |
| `style::BTN_ICON_SM` | `style.rs` | 13 usages | R4-M |
| `style::BTN_ICON_MD` | `style.rs` | (bundled in BTN_ICON_* count) | R4-M |
| `widgets::text::CategoryHeader` | `widgets/text.rs` | 8 usages: `object_tree.rs`, `top_nav.rs`, `watchlist_panel.rs` | R4-M |
| `ft()` fallback-theme calls | widgets/panels | 161 usages across all `ui/` files | R4-A through R4-N |

### R3 New Widget Usage Counts

| Widget | File | Usages |
|--------|------|--------|
| `TopNav` | `gpu.rs` | 1 (replaces inline `render_toolbar()` — lines previously 3644–5308) |
| `ApertureOrderTicket` | `gpu.rs` | 1 (replaces inline Aperture/Octave order entry body — lines previously ~999–1440) |
| `FloatingOrderPaneChrome` | `gpu.rs` | 1 (replaces inline floating order window header — lines previously 7583–7663) |

---

### R1/R2 New Widget Usage Counts

| Widget | File | Usages |
|--------|------|--------|
| `FilterPill` | `watchlist_panel.rs` | 1 (line 388 — filter bar, per-filter pill) |
| `SectionHeader` | `watchlist_panel.rs` | 2 (lines 562, 1097 — section row for watchlist groups) |
| `NmfToggle` | `watchlist_panel.rs` | 2 (lines 1713, 1765 — NMF chain toggles) |
| `AccountStrip` | `gpu.rs` | 1 (line 4937 — `TopBottomPanel::top("account_strip")`) |
| `ColorSwatchPicker` | `indicator_editor.rs` | 4 (lines 316, 350, 360, 375 — indicator color pickers) |
| `ThicknessPicker` | `indicator_editor.rs` | 3 (lines 326, 360, 365 — indicator line thickness) |
| `IndicatorParamRow` | `indicator_editor.rs` | 3 (line 152, 192, etc.) |
| `IndicatorParamRowF` | `indicator_editor.rs` | 2 (lines 192+) |

---

### Text Family

#### TextStyle / foundation/text_style.rs
Used heavily throughout widgets. Key consumer files (post-R1/R2):
- `gpu.rs` — ~93 `TextStyle::MonoSm.as_rich(...)` calls in hover tooltips, overlays, scanner rows
- `watchlist_panel.rs` — `BodyLabel`, `TextStyle::MonoSm`, `DimLabel` throughout
- All card files in `widgets/cards/` — `TextStyle::Numeric`, `TextStyle::Body`

---

### Select / SegmentedControl

| File | Usages |
|------|--------|
| `gpu.rs` (order entry Aperture/Octave path) | 2 (lines ~1005, ~1020 — order type + TIF) |
| `widgets/form.rs` (MeridienOrderTicket) | via composition |
| `dom_panel.rs` | 1 |
| `spread_panel.rs` | 1 |
| **Total** | **~8 usages across 4 files** |

---

## Phase 2 — Per-Panel Hardcoded UI Counts (post-R4)

Patterns counted: `Color32::` literals, `Stroke::new`/`Stroke {`, `vec2(` spacing literals.

> **Note:** `design_preview_pane.rs` and `style.rs` are intentionally excluded — they define the system, not consume it. `gpu.rs` counted separately.

### Panel-level Color32 counts (verified 2026-05-02 grep, post-R5)

| File | Color32 | Notes |
|------|---------|-------|
| `chart_widgets.rs` | **82** | R5-7: 4 additional UI-chrome sites migrated; ~20 migratable remain |
| `discord_panel.rs` | 16 | Brand CTA color; intentional |
| `watchlist_panel.rs` | 15 | Context menu inline buttons remain |
| `plays_panel.rs` | 11 | Semantic colors remain |
| `object_tree.rs` | 7 | 7 literals remain |
| `rrg_panel.rs` | **6** | R5-2: all 4 quadrant fills now `t.rrg_*`; 6 structural literals remain |
| `script_panel.rs` | 5 | Medium panel |
| `indicator_editor.rs` | 4 | Reduced post-R1/R2 |
| `settings_panel.rs` | 4 | Light |
| `hotkey_editor.rs` | 3 | Light |
| `research_panel.rs` | 3 | Light |
| `signals_panel.rs` | 3 | Light |
| `analysis_panel.rs` | 3 | Light |
| `template_popup.rs` | 3 | Light |
| `command_palette/mod.rs` | **2** | R5-2: palette rows now use `t.cmd_palette[*]` |
| `dom_panel.rs` | **1** | R5-2/5: warn/notification_red migrated |
| `tape_panel.rs` | 2 | Low |
| `heatmap_pane.rs` | 2 | Intentional (heatmap domain) |
| `spread_panel.rs` | 2 | Low |
| `feed_panel.rs` | 2 | Low |
| Others (≤1 Color32) | ~11 | scanner, alerts, options, trendline, overlay_manager, etc. |
| **Totals (excl. style+preview)** | **183** | Verified grep 2026-05-02 post-R5 (was 195 post-R4) |

### `gpu.rs` (counted separately)

| Pattern | Count |
|---------|-------|
| `Color32::` literals | **324** |
| `Stroke::new` / `Stroke {` | **317** |

> gpu.rs is dominated by chart-paint paths (intentional). Approximately 80–100 Color32 literals are UI-layer and are R5 candidates.

### Widget-level Color32 counts (verified 2026-05-02 grep)

| Layer | Color32 | Stroke |
|-------|---------|--------|
| `widgets/` (all, incl. cards + rows) | **239** | **128** |
| — of which `rows/` (painter bodies) | ~48 | ~40 |
| — of which `cards/` | 7 | ~10 |
| `ui_kit/` | 4 | — |

> Row painter bodies (`WatchlistRow`, `DomRow`) account for the majority of widget-layer literals — these are canvas-adjacent and are declared R5 scope.

---

## Phase 3 — Migration Progress

### R1/R2 migrations confirmed
- All card files (`widgets/cards/`) migrated to `CardShell` (was Tier 2 → Tier 4)
- `OrderRow`, `AlertRow`, `OptionChainRow` migrated to `RowShell` (was Tier 2 → Tier 4)
- `indicator_editor.rs` — `ColorSwatchPicker`, `ThicknessPicker`, `IndicatorParamRow/F` extracted
- `watchlist_panel.rs` — `FilterPill`, `SectionHeader`, `NmfToggle` extracted
- `gpu.rs` — `AccountStrip` extracted to `widgets/pane.rs`
- ~233 call sites migrated (project claim; audit confirms directional correctness)

### R3 migrations confirmed
- `gpu.rs` toolbar → `widgets::toolbar::TopNav` (~1664 lines removed from gpu.rs)
- `gpu.rs` Aperture/Octave order entry body → `widgets::form::ApertureOrderTicket` (~270 lines removed)
- `gpu.rs` floating order pane header → `widgets::pane::FloatingOrderPaneChrome` (~80 lines removed)
- `widgets/frames.rs PopupFrame` shadow wired to `st.shadow_*` tokens (75 Color32 literals in gpu.rs migrated)

### R4 migrations confirmed (~325 sites)
- **R4-A (66 sites):** `widgets/form.rs`, `pane.rs`, `status.rs` — all `Default`/`new()` impls use `ft()` instead of `Color32::from_rgb(...)`. `FloatingOrderPaneChrome` inline stroke removed.
- **R4-C (10 sites):** `rows/watchlist_row.rs`, `rows/dom_row.rs` — inline `Color32` replaced with `current().*` lookups; `Stroke::new` → `stroke_*()`.
- **R4-D/E/F (~50 sites):** `inputs.rs`, `buttons.rs`, `select.rs`, `toolbar/mod.rs`, `pills.rs`, `chips.rs` — all state-colors wired to `ft()`. Stroke literals replaced with `stroke_bold()`/`stroke_thin()`/`stroke_hair()`. `BTN_ICON_SM/MD` constants introduced.
- **R4-G (108 sites):** Cross-cutting font-size literal sweep — `.size(N)` → `font_xs()/sm()/md()/lg()`. Panels + widgets. 6 font literals remain in widgets, 28 in panels (down from ~246).
- **R4-H (8 sites):** Spacing literal sweep — `vec2(N,M)` / `Margin::same(N)` → `gap_*()` in mid-tier panels. (Foundation/rows spacing largely intact — canvas-adjacent.)
- **R4-J (24 sites):** All `cards/*` — color literals → `ft()`. 32 `ft()` usages across cards. 7 Color32 literals remain (brand/RRG).
- **R4-L (40 sites):** Mid-tier panels (`discord_panel.rs`, `rrg_panel.rs`, `plays_panel.rs`, `apex_diagnostics.rs`, `command_palette/mod.rs`, `dom_panel.rs`, `script_panel.rs`, etc.) — `Color32::from_rgb` → `current().*` or named constants.
- **R4-M (extractions):** `border_stroke()` added to `style.rs` (3 call sites). `BTN_ICON_SM/MD` constants. `CategoryHeader` widget added to `widgets/text.rs` (8 usages). `SectionLabel`/`PanelTitle` repeated patterns consolidated.
- **R4-N (10 sites):** `chart_widgets.rs` UI-chrome layer — toolbar strips, overlay labels, info panel Color32/Stroke/spacing → tokens. Canvas-draw paths untouched.
- **R4-K / R4-I audit:** Foundation layer (`foundation/shell.rs`, `variants.rs`, `tokens.rs`) and frame hand-rolls found no actionable sites — already at desired state.

### Still inline (not yet migrated — R5 candidates)
- `watchlist_panel.rs` ChromeBtn context menu usages (~15) — replace with `SimpleBtn`
- `chart_widgets.rs` canvas-adjacent paths (86 Color32) — intentional; ~20 UI-chrome sites remain
- `gpu.rs` UI-layer `Color32` literals (~80–100 of 324) — tooltip overlays, data labels
- `rows/WatchlistRow` + `DomRow` painter bodies (48 Color32) — canvas-adjacent, high-risk
- `Skeleton` / `NotificationBadge` geometry (Tier 2) — low impact
