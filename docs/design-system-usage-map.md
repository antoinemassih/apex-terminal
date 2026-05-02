# Design System Usage Map

**Audit date:** 2026-04-30 (refreshed post-R1/R2)
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

## Phase 2 — Per-Panel Hardcoded UI Counts

Patterns counted: `Color32::from_rgb`, `egui::Frame::`, `CornerRadius::same`, `.corner_radius(N`, `.size(N)` literal.

> **Note:** `design_preview_pane.rs` (70 hits) and `style.rs` (37) are intentionally excluded — they define the system, not consume it.

### HIGH priority panels

| File | Hardcoded hits | Notes |
|------|---------------|-------|
| `gpu.rs` | **289** | `from_rgb`: 161, `Frame::`: 17, `CornerRadius`: 27, `.size(N)`: 84. Largest single file. Toolbar (lines 3644–5308), floating order panes (7583–7663), and DOM sidebar are the concentration points |
| `chart_widgets.rs` | 65 | Chart UI overlays — many are intentional (chart paint adjacent); ~20 are genuinely migratable |
| `watchlist_panel.rs` | 46 | High count despite R1/R2 adding `FilterPill`/`SectionHeader`/`NmfToggle`; many `ChromeBtn` + inline buttons remain |

### MEDIUM priority panels

| File | Hardcoded hits | Notes |
|------|---------------|-------|
| `object_tree.rs` | 17 | 17 hits — icon-heavy chrome |
| `apex_diagnostics.rs` | 16 | 16 hits — low user-facing impact |
| `discord_panel.rs` | 13 | Discord panel — largely decorative hardcodes; auth buttons with inline CTA |
| `screenshot_panel.rs` | 10 | 10 hits — card row structure inline |
| `indicator_editor.rs` | 6 | 6 hits — reduced post-R1/R2 (ColorSwatchPicker/ThicknessPicker migrated) |
| `hotkey_editor.rs` | 5 | 5 hits |
| `journal_panel.rs` | 5 | 5 hits |
| `research_panel.rs` | 5 | 5 hits |
| `plays_panel.rs` | 5 | 5 hits |
| `connection_panel.rs` | 4 | 4 hits |
| `script_panel.rs` | 3 | 3 hits |
| `overlay_manager.rs` | 3 | 3 hits |
| `orders_panel.rs` | 3 | 3 hits |

### LOW priority panels (< 3 hits, likely already clean or out-of-scope)

| File | Hardcoded hits |
|------|---------------|
| `alerts_panel.rs` | 2 |
| `news_panel.rs` | 2 |
| `rrg_panel.rs` | 2 |
| `portfolio_pane.rs` | 2 |
| `spread_panel.rs` | 2 |
| `dom_panel.rs` | 4 |
| `spreadsheet_pane.rs` | 1 |
| `scanner_panel.rs` | 1 |
| `tape_panel.rs` | 1 |
| `option_quick_picker.rs` | 1 |
| `settings_panel.rs` | 1 |
| `template_popup.rs` | 1 |
| `trendline_filter.rs` | 1 |

### Panels confirmed clean (0 hits)

`feed_panel.rs`, `watchlist.rs`, `orders.rs`, `signals_panel.rs`, `playbook_panel.rs`, `picker.rs`, `toolbar.rs`, `seasonality_panel.rs`, `heatmap_pane.rs`, `dashboard_pane.rs`, `analysis_panel.rs`

---

## Phase 3 — Migration Progress

### R1/R2 migrations confirmed
- All card files (`widgets/cards/`) migrated to `CardShell` (was Tier 2 → Tier 4)
- `OrderRow`, `AlertRow`, `OptionChainRow` migrated to `RowShell` (was Tier 2 → Tier 4)
- `indicator_editor.rs` — `ColorSwatchPicker`, `ThicknessPicker`, `IndicatorParamRow/F` extracted
- `watchlist_panel.rs` — `FilterPill`, `SectionHeader`, `NmfToggle` extracted
- `gpu.rs` — `AccountStrip` extracted to `widgets/pane.rs`
- ~233 call sites migrated (project claim; audit confirms directional correctness)

### Still inline (not yet migrated)
- `gpu.rs` toolbar (lines 3644–5308): ~1664 lines of inline toolbar chrome → `widgets::toolbar::TopNav`
- `gpu.rs` floating order panes (lines 7583–7663): inline `egui::Window` + header chrome
- `gpu.rs` Aperture/Octave order entry body (lines ~999–1440): inline form fields
- `watchlist_panel.rs` ChromeBtn usages (~40) not yet replaced with `ButtonShell`/`IconBtn`
- `discord_panel.rs`, `screenshot_panel.rs` card rows inline
