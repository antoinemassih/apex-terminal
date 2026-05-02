# Design System Usage Map

**Audit date:** 2026-04-30  
**Scope:** `src/chart_renderer/ui/` — all panel files + `gpu.rs`  
**Excluded from hardcoded audit:** `widgets/`, `components/`, `components_extra/`, `foundation/`, `style.rs`, `design_inspector.rs`, `design_preview_pane.rs`

---

## Phase 1 — Widget Usage Inventory

### Buttons Family

#### IconBtn
| File | Usages |
|------|--------|
| `object_tree.rs` | 8 (`arrow`, `vis_icon`, `Icon::LOCK`, `Icon::TRASH`, `eye_icon` — lines 338, 360, 412, 417, 422, 554, 599, 662) |
| `overlay_manager.rs` | 2 (lines 61, 65) |
| `spread_panel.rs` | 4 (lines 353, 369, 371, 442, 446 — 5 calls; 2 for +/- qty controls) |
| **Total** | **17 usages across 3 files** |

#### TradeBtn
| File | Usages |
|------|--------|
| `gpu.rs` (order entry body) | 2 (lines 1200, 1235 — BUY/SELL in floating order entry) |
| `dom_panel.rs` | 2 (lines 208, 233 — BUY/SELL action buttons) |
| `spread_panel.rs` | 1 (line 455 — SUBMIT SPREAD) |
| **Total** | **5 usages across 3 files** |

#### SimpleBtn
| File | Usages |
|------|--------|
| `command_palette/render.rs` | 1 (line 27) |
| `connection_panel.rs` | 1 (line 40) |
| `dom_panel.rs` | 5 (lines 178, 191, 219, 226 — MARKET/LIMIT toggle, FLATTEN, CANCEL) |
| `hotkey_editor.rs` | 1 (line 116) |
| `object_tree.rs` | 1 (line 143) |
| `overlay_manager.rs` | 1 (line 97) |
| `scanner_panel.rs` | 2 (lines 128, 153) |
| `screenshot_panel.rs` | 1 (line 187) |
| `spread_panel.rs` | 6 (lines 365, 374, 407 + misc) |
| `spreadsheet_pane.rs` | 2 (lines 474, 789) |
| `trendline_filter.rs` | 4 (lines 59, 225, 245, 260) |
| **Total** | **25 usages across 11 files** |

#### ChromeBtn
| File | Usages |
|------|--------|
| `watchlist_panel.rs` | ~40 (header actions, section chevrons, color pickers, symbol badges, option chain controls — lines 57, 159, 169, 291, 342, 361, 383, 405, 564, 579, 602, 637, 743, 754, 1029, 1117, 1126, 1148, 1256, 1326, 1389, 1737, 1739, 1744, 1750, 1755, 1756, 1796, 1798, 1802, 1807, 1812, 1813, 1859, 2047) |
| `plays_panel.rs` | 8 (lines 210, 244, 325, 352, 417, 443, 552, 560, 576) |
| `discord_panel.rs` | 6 (lines 159, 189, 338, 354, 418, 512) |
| `orders_panel.rs` | 4 (lines 95, 117, 251 + import) |
| `alerts_panel.rs` | 2 (lines 94, 106) |
| `analysis_panel.rs` | 3 (lines 47, 99, 109) |
| `news_panel.rs` | 1 (line 34) |
| `indicator_editor.rs` | 6 (lines 162, 216, 398, 446, 464, 471) |
| `hotkey_editor.rs` | 2 (lines 101, 107) |
| `feed_panel.rs` | 2 (lines 41, 94) |
| `gpu.rs` (toolbar/Paper-Live) | 2 (lines 3783, 3920) |
| **Total** | **~76 usages across 11 files** |

#### ActionBtn
No usages found outside widget definition files.

#### SmallActionBtn
No usages found outside widget definition files.

---

### Pills Family

#### PillButton
| File | Usages |
|------|--------|
| `widgets/pane.rs` (PaneSymbolBadge internals) | 1 (line 117) |
| `widgets/cards/playbook_card.rs` | 1 (line 57) |
| **Total** | **2 usages across 2 internal widget files** |

#### BrandCtaButton
No usages found outside widget definition files.

#### RemovableChip
| File | Usages |
|------|--------|
| `widgets/pane.rs` (PaneIndicatorChip internals) | 1 (line 157) |
| **Total** | **1 usage in 1 internal widget file** |

#### DisplayChip
| File | Usages |
|------|--------|
| `widgets/pane.rs` | 2 (lines 71, 243 — pane symbol/indicator badges) |
| **Total** | **2 usages in 1 internal widget file** |

#### StatusBadge
No usages found outside widget definition files.

#### KeybindChip
| File | Usages |
|------|--------|
| `command_palette/mod.rs` | 2 (lines 193, 425) |
| **Total** | **2 usages across 1 file** |

---

### Frames Family

#### PanelFrame
| File | Usages |
|------|--------|
| `alerts_panel.rs` | 1 (line 31) |
| `feed_panel.rs` | 1 (line 33) |
| `orders_panel.rs` | 1 (line 30) |
| `signals_panel.rs` | 1 (line 29) |
| **Total** | **4 usages across 4 files** |

#### CompactPanelFrame
| File | Usages |
|------|--------|
| `scanner_panel.rs` | 1 (line 331) |
| `tape_panel.rs` | 1 (line 111) |
| **Total** | **2 usages across 2 files** |

#### PopupFrame
| File | Usages |
|------|--------|
| `apex_diagnostics.rs` | 1 (line 22) |
| `command_palette/mod.rs` | 1 (line 131) |
| `widgets/modal.rs` (internal) | 1 (line 159) |
| **Total** | **3 usages across 3 files** |

#### CardFrame
Only used internally in `widgets/cards/mod.rs` (lines 111, 112).

#### DialogFrame, SidePanelFrame, TooltipFrame, DialogSeparator
No usages found outside widget definition files.

---

### Headers Family

#### DialogHeaderWithClose
| File | Usages |
|------|--------|
| `settings_panel.rs` | 1 (line 42) |
| `widgets/modal.rs` (internal) | 1 (line 195) |
| **Total** | **2 usages across 2 files** |

#### PaneHeader
| File | Usages |
|------|--------|
| `heatmap_pane.rs` | 1 (line 42) |
| `portfolio_pane.rs` | 1 (line 46) |
| `gpu.rs` | ~20 (via `PainterPaneHeader` in `render_chart_pane`, line 5650) |
| **Total** | **~22 usages across 3 files** |

#### PanelHeader, PanelHeaderWithClose, DialogHeader, PaneHeaderWithClose
No usages outside widget/modal internals.

---

### Tabs Family

#### TabBar
| File | Usages |
|------|--------|
| `feed_panel.rs` | 1 (line 82) |
| `orders_panel.rs` | 1 (line 35) |
| `settings_panel.rs` | 1 (line 49) |
| `watchlist_panel.rs` | 1 (line 46) |
| **Total** | **4 usages across 4 files** |

#### TabStrip, TabBarWithClose
No usages found outside widget definition files.

---

### Text Family

#### MonospaceCode
| File | Usages |
|------|--------|
| `apex_diagnostics.rs` | ~22 (bulk diagnostics readouts) |
| `research_panel.rs` | ~16 (valuation/financial data fields) |
| `orders_panel.rs` | ~12 (positions, P&L, order text) |
| `rrg_panel.rs` | 1 (line 245) |
| `plays_panel.rs` | ~5 |
| `trendline_filter.rs` | 3 (lines 43, 262, 264) |
| **Total** | **~172 usages across many files (highest-used text widget)** |

#### SectionLabel
| File | Usages |
|------|--------|
| `research_panel.rs` | ~6 (section headers) |
| `plays_panel.rs` | 1 (line 200) |
| **Total** | **~46 usages (2nd most used text widget)** |

#### BodyLabel
| File | Usages |
|------|--------|
| `command_palette/mod.rs` | 5 (lines 168, 194, 200, 426, 435) |
| `command_palette/render.rs` | ~12 |
| `connection_panel.rs` | 2 (lines 81, 89) |
| `screenshot_panel.rs` | 6 (lines 123, 144, 146, 166, 167, 179, 183) |
| `hotkey_editor.rs` | 2 (lines 96, 99) |
| `discord_panel.rs` | 1 (line 182) |
| **Total** | **~18 usages across 6 files** |

#### DimLabel
| File | Usages |
|------|--------|
| Used in ~4 locations (minor usage) | |
| **Total** | **4 usages** |

#### PaneTitle, Subheader, MutedLabel, CaptionLabel, NumericDisplay
No usages found outside widget definition files.

---

### Status Family

#### StatusDot
| File | Usages |
|------|--------|
| `rrg_panel.rs` | 2 (lines 258, 264) |
| **Total** | **2 usages across 1 file** |

#### Spinner
| File | Usages |
|------|--------|
| `scanner_panel.rs` | 2 (lines 88, 175) |
| **Total** | **2 usages across 1 file** |

#### SearchPill
| File | Usages |
|------|--------|
| `gpu.rs` | 2 (line 4851) |
| **Total** | **2 usages in gpu.rs** |

#### ProgressBar, ProgressRing, Skeleton, Toast, NotificationBadge, ConnectionIndicator, TrendArrow
No usages found outside widget definition files.

---

### Rows Family

#### WatchlistRow
| File | Usages |
|------|--------|
| `watchlist_panel.rs` | 2 active (lines 495, 815) |
| `scanner_panel.rs` | 1 (line 237) |
| **Total** | **4 usages across 2 files** |

#### OrderRow
| File | Usages |
|------|--------|
| `orders_panel.rs` | 1 (line 283) |
| **Total** | **1 usage across 1 file** |

#### NewsRow
| File | Usages |
|------|--------|
| `news_panel.rs` | 1 (line 81) |
| **Total** | **1 usage across 1 file** |

#### AlertRow
| File | Usages |
|------|--------|
| `alerts_panel.rs` | 5 (lines 140, 205, 218, 253, 265) |
| **Total** | **5 usages across 1 file** |

#### DomRow
| File | Usages |
|------|--------|
| `dom_panel.rs` | 1 (line 316) |
| **Total** | **1 usage across 1 file** |

#### ListRow
| File | Usages |
|------|--------|
| `discord_panel.rs` | 2 (lines 388, 477) |
| `object_tree.rs` | 5 (lines 349, 399, 544, 591, 654) |
| `tape_panel.rs` | 1 (line 76) |
| **Total** | **8 usages across 3 files** |

#### OptionChainRow, Table
No usages found outside widget definition files.

---

### Cards Family

#### PlayCard
| File | Usages |
|------|--------|
| `plays_panel.rs` | 1 (line 661) |
| **Total** | **1 usage across 1 file** |

#### MetricCard
| File | Usages |
|------|--------|
| `spread_panel.rs` | 4 (lines 420, 423, 429, 432) |
| **Total** | **4 usages across 1 file** |

#### EarningsCard, EventCard, NewsCard, PlaybookCard, SignalCard, StatCard, TradeCard
No usages found outside widget definition files.

---

### Inputs Family

#### TextInput
| File | Usages |
|------|--------|
| `alerts_panel.rs` | 1 (line 80) |
| `command_palette/mod.rs` | 1 (line 169) |
| `command_palette/render.rs` | 1 (line 42) |
| `discord_panel.rs` | 1 (line 506) |
| `overlay_manager.rs` | 1 (line 83) |
| `plays_panel.rs` | 6 (lines 258, 269, 302, 317, 344, 371, 429, 453) |
| `scanner_panel.rs` | 2 (lines 114, 123) |
| `script_panel.rs` | 2 (lines 112, 269) |
| `spread_panel.rs` | 2 (lines 271, 379) |
| `spreadsheet_pane.rs` | 2 (lines 530, 702) |
| `template_popup.rs` | 1 (line 157) |
| `trendline_filter.rs` | 1 (line 199) |
| `watchlist_panel.rs` | 6 (lines 85, 215, 358, 402, 404, 639, 1347) |
| **Total** | **~28 usages across 13 files** |

#### Stepper
| File | Usages |
|------|--------|
| `gpu.rs` (order entry body) | 1 (line 1082) |
| **Total** | **1 usage across 1 file** |

#### SearchInput
Only used internally in `widgets/select.rs` (line 178 — inside Dropdown).

#### NumericInput, ToggleRow, CompactStepper, Slider
No usages found outside widget definition files.

---

### Selects Family

#### SegmentedControl
| File | Usages |
|------|--------|
| `gpu.rs` (order entry) | 3 (lines 1027, 1037, 1059) |
| `indicator_editor.rs` | 4 (lines 103, 120, 288, 369) |
| **Total** | **7 usages across 2 files** |

#### Dropdown
| File | Usages |
|------|--------|
| `spread_panel.rs` | 2 (lines 303, 391) |
| `watchlist_panel.rs` | 2 (lines 1313, 1847) |
| **Total** | **4 usages across 2 files** |

#### DropdownOwned
| File | Usages |
|------|--------|
| `watchlist_panel.rs` | 3 (lines 109, 1728, 1788) |
| **Total** | **3 usages across 1 file** |

#### RadioGroup, Combobox, MultiSelect, Autocomplete, DropdownActions
No usages found outside widget definition files.

---

### Modal Family

#### Modal
| File | Usages |
|------|--------|
| `connection_panel.rs` | 1 (line 23) |
| `hotkey_editor.rs` | 1 (line 59) |
| `indicator_editor.rs` | 1 (line 45) |
| `option_quick_picker.rs` | 1 (line 66) |
| `template_popup.rs` | 1 (line 34) |
| **Total** | **5 usages across 5 files** |

---

### Toolbar Family

#### ToolbarBtn
| File | Usages |
|------|--------|
| `gpu.rs` (render_toolbar) | 17 (lines 3757, 3793, 3798, 3802, 3932, 3936, 3942, 3948, 4476, 4691, 4844, 4864, 4870, 4876, 4882, 4891, 4904) |
| **Total** | **17 usages in gpu.rs only** |

#### TopNavBtn, TopNavToggle, PaneTabBtn, TimeframeSelector, PaneHeaderAction
No usages found outside widget definition files.

---

### Pane Family

All pane widgets (`PaneSymbolBadge`, `PaneTimeframeBadge`, `PaneIndicatorChip`, `PaneStatusStrip`, `PaneHeaderBar`, `PaneFooter`, `PaneHeaderActions`, `PaneDivider`) are used internally in `widgets/pane.rs` and `widgets/painter_pane.rs` only. `PaneHeader` appears in `heatmap_pane.rs`, `portfolio_pane.rs`, and `gpu.rs` (via `PainterPaneHeader`).

---

### Layout Family

#### EmptyState
| File | Usages |
|------|--------|
| `dashboard_pane.rs` | 1 (line 41) |
| `journal_panel.rs` | 2 (lines 17, 62) |
| `scanner_panel.rs` | 1 (line 178) |
| `seasonality_panel.rs` | 1 (line 25) |
| **Total** | **5 usages across 4 files** |

#### Splitter, ResizableSplit, Collapsible, Section, Accordion, Stack, Cluster, Center, Spacer
No usages found outside widget definition files.

---

### Form Family

#### FormRow
| File | Usages |
|------|--------|
| `gpu.rs` (order entry body) | 3 (lines 1130, 1139, 1148 — Limit, Stop, Trail price rows) |
| `indicator_editor.rs` | 8 (lines 176, 184, 193, 227, 236, 243, 252, 259, 266) |
| `research_panel.rs` | 3 (lines 35, 57, 80) |
| `scanner_panel.rs` | 3 (lines 113, 117, 122) |
| `settings_panel.rs` | 3 (lines 14, 495, 514) |
| **Total** | **~21 usages across 5 files** |

#### FieldSet, FormSection, LabeledControl, HelpText, ErrorText, RequiredMarker, InlineValidation
No usages found outside widget definition files.

#### MeridienOrderTicket
Used internally in `render_order_entry_body` (gpu.rs, line ~970 via the Meridien path).

---

### Menus Family

#### MenuItem
| File | Usages |
|------|--------|
| `object_tree.rs` | 2 (lines 457 `DangerMenuItem`, 471 `MenuItem`) |
| **Total** | **2 usages across 1 file** |

#### MenuTrigger, SidePaneAction
No usages found outside widget definition files.

---

## Phase 2 — Hardcoded UI Catalog

### watchlist_panel.rs

Uses widgets:
- `ChromeBtn`: lines 57, 159, 169, 291, 342, 361, 383, 405, 564, 579, 602, 637, 743, 754, 1029, 1117, 1126, 1148, 1256, 1326, 1389, 1737, 1739, 1744, 1750, 1755, 1756, 1796, 1798, 1802, 1807, 1812, 1813, 1859, 2047
- `TabBar`: line 46
- `TextInput`: lines 85, 215, 358, 402, 404, 639, 1347
- `WatchlistRow`: lines 495, 815
- `DropdownOwned`: lines 109, 1728, 1788
- `Dropdown`: lines 1313, 1847

Hardcoded UI:
- Line 29: `egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin {...})` — raw frame, should use `SidePanelFrame`
- Line 67: `egui::Stroke::new(stroke_std(), t.toolbar_border)` — hardcoded stroke on painted border
- Lines 115, 119, 125, 141, 145, 151: `ui.button(egui::RichText::new("Rename"/"Duplicate"/"Delete").monospace().size(9.0))` — bare `ui.button()` in context menus; should be `MenuItem` / `DangerMenuItem`
- Line 126: `Color32::from_rgb(224, 85, 96)` — red delete literal; should be `t.bear` or a danger token
- Line 152: same `Color32::from_rgb(224, 85, 96)` repeat
- Line 286: `egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(28, 28, 34)).corner_radius(r_sm_cr())` — raw popup frame with hardcoded dark fill `rgb(28,28,34)`; should be `PopupFrame`
- Line 322: `egui::Frame::NONE` — raw no-frame frame
- Lines 478–486: `egui::Stroke::new(stroke_std(), egui::Color32::from_rgba_unmultiplied(0,0,0, alpha_*()))` — multiple raw semi-transparent black strokes for drag handle
- Line 545: `egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_strong()))` — hardcoded strong border
- Lines 590, 612, 621, 1136, 1158, 1167: bare `ui.button(egui::RichText::new(...).monospace().size(9.0))` context menu items
- Line 621: `Color32::from_rgb(224, 85, 96)` — danger literal again
- Line 750: `Color32::from_rgb(200, 200, 210)` — dim text literal; should be `t.dim`
- Line 796: `egui::Color32::from_rgba_unmultiplied(80, 120, 200, 12)` — inline tint for drag highlight
- Line 976: `egui::Stroke::new(stroke_thick(), t.accent)` — accent stroke for selected item ring
- Line 990: `painter.rect_stroke(float_rect, 4.0, egui::Stroke::new(stroke_std(), t.accent), ...)` — hardcoded radius `4.0`; should be `r_sm_cr()`
- Line 1328: `egui::Color32::from_rgba_unmultiplied(t.accent.r(),t.accent.g(),t.accent.b(),51)` — should be `color_alpha(t.accent, 51)`
- Line 1329: `egui::Stroke::new(stroke_std(), ...)` — raw stroke
- Line 1387: `egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgb(28, 28, 34)).corner_radius(r_sm_cr())` — same `rgb(28,28,34)` hardcoded dark fill as line 286
- Lines 1509–1512: `egui::Color32::from_rgba_unmultiplied(231,76,60,...)`, `from_rgba_unmultiplied(240,160,40,...)`, etc. — IV coloring with raw RGB thresholds; no token
- Line 1526: `egui::Stroke::new(stroke_thin(), ...)` — raw stroke for option chain separator

**Total hardcoded sites: ~46**

---

### indicator_editor.rs

Uses widgets:
- `ChromeBtn`: lines 162, 216, 398, 446, 464, 471
- `SegmentedControl`: lines 103, 120, 288, 369
- `FormRow`: lines 176, 184, 193, 227, 236, 243, 252, 259, 266
- `Modal`: line 45

Hardcoded UI:
- Line 31: `egui::Frame::popup(&ctx.style()).fill(...).stroke(egui::Stroke::new(stroke_std(), ...))` — raw popup frame; should use `PopupFrame`
- Line 34: `egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_heavy()))` — hardcoded stroke on frame
- Lines 69, 150: `ui.label(egui::RichText::new(...).monospace().size(9.0).color(TEXT_PRIMARY))` — bare `ui.label()` with literal `.size(9.0)`; should be `MonospaceCode` or `BodyLabel`
- Line 311: `egui::Button::new(egui::RichText::new(label).monospace().size(9.0).color(fg)).fill(...)` — raw button for timeframe selector; should be `ChromeBtn` or `SegmentedControl`
- Line 313: `egui::Stroke::new(stroke_thin(), if sel { ... } else { ... })` — conditional stroke literal
- Line 341: `painter.rect_stroke(r, 3.0, egui::Stroke::new(stroke_std(), color), ...)` — hardcoded radius `3.0` in painter call
- Line 360: raw `egui::Button::new(...)` with `.size(8.0)` for threshold selector
- Line 391: `painter.rect_stroke(r, 2.0, ...)` — hardcoded radius `2.0`
- Line 417, 419: raw button + stroke for threshold values
- Line 440: `painter.rect_stroke(r, 2.0, ...)` — same
- Line 470: `egui::Color32::from_rgb(224, 85, 96)` — danger literal; should be theme danger token
- Line 473: `egui::Stroke::new(stroke_thin(), color_alpha(del_color, alpha_dim()))` — literal stroke for delete button border

**Total hardcoded sites: ~22**

---

### plays_panel.rs

Uses widgets:
- `ChromeBtn`: lines 210, 244, 325, 352, 417, 443, 552, 560, 576
- `TextInput`: lines 258, 269, 302, 317, 344, 371, 429, 453
- `PlayCard`: line 661
- `MonospaceCode`: lines 60, 61, 232, 393
- `SectionLabel`: line 200

Hardcoded UI:
- Line 58: `ui.label(egui::RichText::new(Icon::STAR).size(28.0).color(t.dim.gamma_multiply(0.2)))` — large icon label with literal `.size(28.0)` for empty state; should use `EmptyState`
- Lines 194–198: `egui::Frame::NONE.fill(...).stroke(egui::Stroke::new(stroke_thin(), ...))` — raw card frame; should use `CardFrame`
- Lines 212, 246: raw `egui::Button` with `.stroke(egui::Stroke::new(stroke_thin(), ...))` — tab-like buttons not using `SegmentedControl` or `TabBar`
- Line 301: `ui.label(egui::RichText::new("T1").monospace().size(7.0).strong().color(t.bull.gamma_multiply(0.7)))` — label with literal size; should use `MonospaceCode`
- Line 316: `Color32::from_rgb(26, 188, 156)` — teal literal for T2 label; no token
- Line 343: `Color32::from_rgb(52, 152, 219)` — blue literal for T3 label; no token
- Line 392: `Color32::from_rgb(255, 191, 0)` — yellow warning literal; no token
- Lines 419: `egui::Stroke::new(0.5, ...)` — literal stroke weight for tag pill border
- Lines 684–702: `painter.rect_filled(card_rect..., egui::Color32::from_rgba_unmultiplied(0,0,0,...))` — custom card shadow with raw RGBA colors; entire card paint should delegate to `CardFrame`
- Line 702: `egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_strong()))` — card border stroke
- Line 718: `egui::Stroke::new(stroke_thin(), ...)` — direction pill border
- Line 728: `Color32::from_rgb(255, 191, 0)` — yellow for `PlayStatus::Active` (same repeat)

**Total hardcoded sites: ~29**

---

### object_tree.rs

Uses widgets:
- `IconBtn`: lines 338, 360, 412, 417, 422, 554, 599, 604, 662, 667
- `ListRow`: lines 349, 399, 544, 591, 654
- `SimpleBtn`: line 143
- `MenuItem` / `DangerMenuItem`: lines 457, 471

Hardcoded UI:
- Lines 47, 49: `egui::Color32::from_rgba_unmultiplied(...)` — layer tint calculations with raw RGB math
- Lines 100–103: `Color32::from_rgb(224,85,96)`, `from_rgb(255,193,37)`, `from_rgb(81,207,102)`, `from_rgb(120,120,120)` — threat-level color literals; no tokens for severity states
- Line 114: `egui::Frame::NONE.fill(t.toolbar_bg).stroke(egui::Stroke::new(...))` — raw side-panel frame; should use `SidePanelFrame`
- Lines 151, 208, 212: `ui.menu_button(egui::RichText::new(...).monospace().size(7.0/9.0))` — raw `menu_button()` calls; should use `MenuTrigger` / `MenuItem`
- Line 255: `icon_btn(ui, Icon::TRASH, egui::Color32::from_rgb(224, 85, 96), FONT_MD)` — legacy `icon_btn()` helper call; should use `IconBtn::new(Icon::TRASH).color(...)` (already done elsewhere, inconsistent)

**Total hardcoded sites: ~22**

---

### settings_panel.rs

Uses widgets:
- `DialogHeaderWithClose`: line 42
- `TabBar` (not found — uses raw tabs)
- `FormRow`: lines 14, 495, 514
- `TextInput` (implicit via FormRow content)

Hardcoded UI:
- Line 39: `egui::Frame::popup(&ctx.style()).fill(t.toolbar_bg).inner_margin(0.0).outer_margin(0.0).stroke(egui::Stroke::new(stroke_std(), border)).corner_radius(r_lg_cr())` — detailed raw popup frame; should use `PopupFrame`
- Line 97: `Color32::from_rgb(...)` — theme-swatch color literal
- Lines 122, 134: `egui::Stroke::new(0.5, ...)` / `egui::Stroke::new(1.0, ...)` — literal stroke weights for theme card borders
- Line 146: `egui::Stroke::new(2.0, th.accent)` — bold selected border with literal weight `2.0`
- Line 148: `egui::Stroke::new(1.0, ...)` — secondary ring
- Line 211: `egui::Stroke::new(if sel { 1.5 } else { 0.5 }, ...)` — conditional literal stroke for button-style list items
- Line 320: `egui::Button::new(egui::RichText::new(label).monospace().size(8.0).color(fg))` — raw button for sound selector; should use `ChromeBtn` or `SegmentedControl`
- Lines 360, 370, 372: `Color32::from_rgb(46, 204, 113)` (green) and `from_rgb(230, 70, 70)` (red) — live/paper mode color literals; should be `t.bull` / `t.bear`
- Line 406, 421: `egui::Button::new(...)` with raw `.stroke(...)` for style selector chips
- Lines 543, 545: same green/red literals for WS connection status

**Total hardcoded sites: ~21**

---

### rrg_panel.rs

Uses widgets:
- `StatusDot`: lines 258, 264
- `MonospaceCode`: line 245

Hardcoded UI:
- Lines 184, 187: `Color32::from_rgb(*r, *g, *b)` / `from_rgb(180,180,180)` — dynamic sector color from data + fallback
- Line 245: `color: egui::Color32::from_rgb(56, 203, 137)` — RRG "improving" quadrant green literal; needs token
- Lines 364–388: `egui::Color32::from_rgba_unmultiplied(56,203,137,8)`, `(230,200,50,8)`, `(224,82,82,8)`, `(74,158,255,8)` — quadrant fill colors for RRG (Leading/Improving/Lagging/Weakening); hard literals
- Lines 393, 443, 463: `egui::Stroke::new(stroke_std()/stroke_thin(), ...)` — axis and grid strokes
- Lines 478–502: repeated `from_rgba_unmultiplied(...)` for quadrant fills with variable alpha
- Lines 519, 521: per-segment trail color derived from `color.r/g/b()` — painter code, acceptable
- Lines 532, 540: `from_rgba_unmultiplied(...)` for glow/dot fill
- Line 561: `from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200)` — high-opacity dot
- Line 569: `egui::Stroke::new(stroke_std(), ...)` — chart axis stroke

**Total hardcoded sites: ~21** (note: most are charting/painter calls, partially acceptable)

---

### script_panel.rs

Uses widgets:
- `TextInput`: lines 112, 269
- `Modal` (via `egui::Window` raw — no Modal widget used)

Hardcoded UI:
- Line 128: `egui::RichText::new(*name).monospace().size(8.0)` with `.stroke(egui::Stroke::new(stroke_thin(), ...))` — raw styled buttons for script tabs
- Line 130: `egui::Stroke::new(stroke_thin(), color_alpha(t.accent, 35))` — literal low-alpha border
- Line 148: `painter.rect_stroke(rect, 4.0, ...)` — hardcoded radius `4.0` on code block border
- Line 243: `egui::Frame::popup(&ctx.style()).fill(...).stroke(egui::Stroke::new(stroke_std(), ...))` — raw popup frame; should use `PopupFrame`
- Line 285: `painter.rect_stroke(bg_rect, 3.0, egui::Stroke::new(stroke_std(), ...))` — highlight ring
- Line 300: `egui::Stroke::new(stroke_thin(), color_alpha(t.accent, 35))` — same accent stroke as line 130
- Lines 328, 482: `painter.rect_stroke(rect, 4.0/3.0, ...)` — repeated raw radii
- Lines 627–652: raw `egui::Button::new(egui::RichText::new(...).monospace().size(9.0))` with `.stroke(...)` for Run/Stop/Clear action buttons; should use `ChromeBtn` or `ActionBtn`

**Total hardcoded sites: ~20**

---

### discord_panel.rs

Uses widgets:
- `ListRow`: lines 388, 477
- `TextInput`: line 506
- `BodyLabel`: line 182
- `ChromeBtn`: lines 159, 189, 338, 354, 418, 512

Hardcoded UI:
- Line 114: `egui::Frame::NONE.fill(t.toolbar_bg).stroke(egui::Stroke::new(stroke_std(), ...))` — raw side panel frame
- Lines 147, 332: `ui.label(egui::RichText::new(Icon::...).size(36.0/28.0).color(...))` — large icon labels with literal sizes for empty/connected states; should use `EmptyState`
- Lines 160, 339: `egui::RichText::new(...).monospace().size(10.0/9.0).strong().color(egui::Color32::WHITE)` — hardcoded `WHITE` for button text; should pass color through token
- Line 190: `egui::Color32::from_rgb(231, 76, 60)` — red for "×" in channel chip; should be `t.bear`
- Lines 380, 419, 492: `egui::RichText::new(...).size(8.0/9.0)` with literal sizes
- Line 513: `egui::RichText::new("Send").monospace().size(9.0).color(egui::Color32::WHITE)` — Send button with hardcoded WHITE

**Total hardcoded sites: ~19**

---

### screenshot_panel.rs

Uses widgets:
- `BodyLabel`: lines 123, 144, 146, 166, 167, 179, 183
- `SimpleBtn`: line 187

Hardcoded UI:
- Lines 116–118: `egui::Frame::NONE.fill(t.toolbar_bg).stroke(egui::Stroke::new(stroke_std(), ...))` — raw panel frame
- Lines 157–161: `egui::Frame::NONE.fill(...).inner_margin(...).stroke(egui::Stroke::new(stroke_thin(), ...))` — raw card frame for screenshot card; should use `CardFrame`
- Line 171: `egui::RichText::new("\u{e9a8}").size(9.0)` — literal size on close icon; should be `IconBtn`

**Total hardcoded sites: ~13**

---

### dom_panel.rs

Uses widgets:
- `TradeBtn`: lines 208, 233
- `SimpleBtn`: lines 178, 191, 219, 226
- `DomRow`: line 316

Hardcoded UI:
- Line 71: `egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_heavy()))` — resize handle border
- Line 78: `egui::Stroke::new(stroke_thick(), ...)` — hovered resize handle accent
- Line 129: `egui::Stroke::new(stroke_thin(), ...)` — separator line
- Line 136: `egui::Stroke::new(stroke_std(), egui::Color32::from_rgba_unmultiplied(0,0,0,...))` — decorative shadow lines with raw black RGBA
- Line 137: `egui::Stroke::new(stroke_thin(), ...)` — separator
- Line 162: `egui::Color32::from_rgb(220,220,230)` — quantity display color literal; should be `t.text` or `t.dim`
- Line 189: `egui::Color32::from_rgb(230,70,70)` — armed state red literal; should be `t.bear`
- Line 214: `egui::Color32::from_rgb(200,150,50)` — flatten button amber literal; no token
- Line 372: `egui::Color32::from_rgb(10, 12, 16)` — dark text on colored badge; hardcoded near-black

**Total hardcoded sites: ~10**

---

### spread_panel.rs

Uses widgets:
- `TradeBtn`: line 455
- `SimpleBtn`: lines 365, 374, 407
- `IconBtn`: lines 353, 369, 371, 442, 446
- `TextInput`: lines 271, 379
- `Dropdown`: lines 303, 391
- `MetricCard`: lines 420, 423, 429, 432

Hardcoded UI:
- Line 236: `egui::Frame::popup(&ctx.style()).fill(...).stroke(egui::Stroke::new(stroke_std(), ...))` — raw popup frame; should use `PopupFrame`
- Line 283: `egui::RichText::new(active_symbol).monospace().size(8.0).color(t.accent)` — active symbol display with literal size
- Line 286: `egui::Stroke::new(stroke_thin(), color_alpha(t.accent, alpha_muted()))` — accent border

**Total hardcoded sites: ~6**

---

### spreadsheet_pane.rs

Uses widgets:
- `TextInput`: lines 530, 702
- `SimpleBtn`: lines 474, 789

Hardcoded UI:
- Line 567: `egui::Stroke::new(stroke_thin(), color_alpha(t.accent, alpha_line()))` — selection border
- Lines 619, 657: `egui::Stroke::new(stroke_thin(), ...)` — grid lines
- Lines 709, 737: `egui::Stroke::new(1.0, t.accent)` — literal `1.0` for active cell borders
- Line 786: `egui::Frame::popup(ui.style())` — raw popup frame for formula autocomplete

**Total hardcoded sites: ~6**

---

### option_quick_picker.rs

Uses widgets:
- `Modal`: line 66

Hardcoded UI:
- Line 57: `egui::Frame::popup(&ctx.style()).fill(...).stroke(egui::Stroke::new(stroke_std(), ...))` — raw popup frame; should use `PopupFrame`
- Lines 139, 166: `egui::Stroke::new(stroke_thin(), color_alpha(t.accent, alpha_line()))` — selection ring strokes

**Total hardcoded sites: ~5**

---

### portfolio_pane.rs

Uses widgets:
- `PaneHeader`: line 46

Hardcoded UI:
- Line 116: `egui::Stroke::new(0.5, ...)` — separator line with literal weight
- Line 224: `egui::Stroke::new(10.0, color)` — thick arc/gauge stroke with literal `10.0`
- Lines 259, 299: `egui::Color32::from_rgb(255, 191, 0)` — margin utilization warning yellow; should be a token
- Lines 348, 350: `from_rgba_unmultiplied(t.bear.r()...` / `from_rgba_unmultiplied(t.accent.r()...` — risk bar colors derived from theme but reconstructed as raw RGBA

**Total hardcoded sites: ~6**

---

### orders_panel.rs

Uses widgets:
- `PanelFrame`: line 30
- `TabBar`: line 35
- `OrderRow`: line 283
- `ChromeBtn`: lines 95, 117, 251
- `MonospaceCode`: lines 90, 91, 142, 144, 146, 156, 158, 163, 195, 227

Hardcoded UI:
- Line 47: `egui::Stroke::new(1.0, color_alpha(t.toolbar_border, alpha_muted()))` — raw separator with literal `1.0`
- Lines 95, 117: `ChromeBtn` with `.size(9.0/.size(9.0)` — ChromeBtn used correctly but `size()` on RichText is a literal

**Total hardcoded sites: ~6** (relatively clean)

---

### news_panel.rs

Uses widgets:
- `ChromeBtn`: line 34
- `NewsRow`: line 81

Hardcoded UI:
- Line 19: `egui::Frame::popup(&ctx.style()).fill(...).stroke(egui::Stroke::new(stroke_std(), ...))` — raw popup frame
- Line 35: `egui::RichText::new(...).monospace().size(9.0).color(filter_col)` — literal size on filter label
- Line 38: `egui::Stroke::new(stroke_thin(), color_alpha(filter_col, alpha_muted()))` — filter chip border

**Total hardcoded sites: ~6**

---

### trendline_filter.rs

Uses widgets:
- `MonospaceCode`: lines 43, 262, 264
- `SimpleBtn`: lines 59, 225, 245, 260
- `TextInput`: line 199

Hardcoded UI:
- Line 43: `egui::Color32::from_rgb(200,200,210)` — dim text literal
- Line 193: `egui::Frame::popup(&ctx.style()).fill(...).stroke(egui::Stroke::new(stroke_thin(), ...))` — raw popup frame
- Line 244: `Color32::from_rgb(200,200,210)` — non-current symbol color literal; should be `t.dim`
- Lines 262, 264: `Color32::from_rgb(180,180,190)` / `Color32::from_rgb(100,100,120)` — name/tag muted colors; should use `t.dim` variants

**Total hardcoded sites: ~8**

---

### apex_diagnostics.rs

Uses widgets:
- `PopupFrame`: line 22
- `MonospaceCode`: ~20 usages (bulk)
- `SectionLabel`: line 25

Hardcoded UI:
- Lines 81, 95, 100, 103, 112, 128–131: `Color32::from_rgb(80,200,120)` (green=OK), `from_rgb(230,70,70)` (red=error), `from_rgb(240,170,70)` (amber=warn) — repeated status color literals across ~16 sites; should use `t.bull`, `t.bear`, and a warning token
- Lines 157–159: same three colors for data age indicators
- Lines 179–182: same pattern in `match` arms

**Total hardcoded sites: ~17** (all color-semantic literals, no token coverage)

---

### alerts_panel.rs

Uses widgets:
- `PanelFrame`: line 31
- `ChromeBtn`: lines 94, 106
- `TextInput`: line 80
- `AlertRow`: lines 140, 205, 218, 253, 265

Hardcoded UI:
- Lines 96, 108: `egui::RichText::new(...).monospace().size(9.0)` — size literals on ChromeBtn text
- Lines 99, 111: `egui::Stroke::new(stroke_thin(), color_alpha(above/below_color, alpha_line()))` — conditional strokes for above/below pill buttons

**Total hardcoded sites: ~6**

---

### connection_panel.rs

Uses widgets:
- `Modal`: line 23
- `BodyLabel`: lines 81, 89
- `SimpleBtn`: line 40

Hardcoded UI:
- Lines 18–21: `egui::Frame::popup(&_ctx.style()).fill(...).stroke(egui::Stroke::new(1.0, ...))` — raw popup frame with literal `1.0` stroke weight; should use `PopupFrame`
- Lines 81, 89: `BodyLabel` with `.size(9.0/8.0)` — literal sizes (though using widget, the size override bypasses `font_sm()`/`font_md()` tokens)

**Total hardcoded sites: ~6**

---

### overlay_manager.rs

Uses widgets:
- `IconBtn`: lines 61, 65
- `SimpleBtn`: line 97
- `TextInput`: line 83

Hardcoded UI:
- Line 22: `egui::Frame::popup(&ctx.style()).fill(...).stroke(egui::Stroke::new(stroke_std(), ...))` — raw popup frame
- Line 65: `IconBtn::new(Icon::X).size(10.0).color(t.bear.gamma_multiply(0.5))` — IconBtn used with literal `.size(10.0)` and `gamma_multiply` tint (minor)

**Total hardcoded sites: ~6**

---

### analysis_panel.rs

Uses widgets:
- `ChromeBtn`: lines 47, 99, 109

Hardcoded UI:
- Line 66: `egui::Stroke::new(1.0, ...)` — tab bottom border with literal `1.0`
- Line 120: `egui::Stroke::new(0.5, ...)` — faint separator

**Total hardcoded sites: ~2**

---

### feed_panel.rs

Uses widgets:
- `PanelFrame`: line 33
- `TabBar`: line 82
- `ChromeBtn`: lines 41, 94

Hardcoded UI:
- Line 55: `egui::Stroke::new(1.0, ...)` — tab border with literal `1.0`
- Line 104: `egui::Stroke::new(0.5, ...)` — faint separator

**Total hardcoded sites: ~2**

---

### signals_panel.rs

Uses widgets:
- `PanelFrame`: line 29

Hardcoded UI:
- Line 51: `egui::Stroke::new(1.0, ...)` — tab border
- Line 98: `egui::Stroke::new(0.5, ...)` — faint separator

**Total hardcoded sites: ~2**

---

### hotkey_editor.rs

Uses widgets:
- `Modal`: line 59
- `ChromeBtn`: lines 101, 107
- `SimpleBtn`: line 116
- `BodyLabel`: lines 96, 99

Hardcoded UI:
- Lines 96: `BodyLabel::new(...).size(9.0)` — literal `.size()` override (uses widget but bypasses token)
- Line 107: `egui::RichText::new(...).monospace().size(9.0)` — literal size inside ChromeBtn
- Lines 101, 107: `ChromeBtn` with raw `.size()` literals

**Total hardcoded sites: ~6**

---

### scanner_panel.rs

Uses widgets:
- `CompactPanelFrame`: line 331
- `Spinner`: lines 88, 175
- `WatchlistRow`: line 237
- `FormRow`: lines 113, 117, 122
- `TextInput`: lines 114, 123
- `SimpleBtn`: lines 128, 153
- `EmptyState`: line 178

Hardcoded UI:
- Line 197: `egui::RichText::new(color).monospace().size(9.0).strong()` — literal size in scanner result list item

**Total hardcoded sites: ~1** (cleanest panel)

---

### tape_panel.rs

Uses widgets:
- `CompactPanelFrame`: line 111
- `ListRow`: line 76

Hardcoded UI:
- `tape_panel.rs` has a local `const fn rgb(...)` helper at line 10 (no actual calls in audit scope)

**Total hardcoded sites: ~1**

---

### journal_panel.rs

Uses widgets:
- `EmptyState`: lines 17, 62
- `MonospaceCode` (no direct use — uses `ui.label` with RichText)

Hardcoded UI:
- Line 56: `egui::Stroke::new(1.0, ...)` — tab section border
- Lines 98, 100, 183, 184, 186: `ui.label(egui::RichText::new(...).monospace().size(7.0/34.0))` — bare `ui.label()` calls with raw size literals including a hero-sized `size(34.0)` for total P&L display; should use `NumericDisplay`

**Total hardcoded sites: ~6**

---

### template_popup.rs

Uses widgets:
- `Modal`: line 34
- `TextInput`: line 157

Hardcoded UI:
- Line 25: `egui::Frame::popup(&ctx.style()).fill(...).stroke(egui::Stroke::new(stroke_std(), ...))` — raw popup frame
- Line 75: `egui::Stroke::new(if active { stroke_thin() } else { 0.0 }, ...)` — conditional stroke with literal `0.0`

**Total hardcoded sites: ~5**

---

### seasonality_panel.rs

Uses widgets:
- `EmptyState`: line 25

Hardcoded UI:
- Line 67: `egui::Stroke::new(0.5, ...)` — grid stroke
- Lines 90–91: `egui::Stroke::new(if is_current { 2.5 } else { 1.5 }, egui::Color32::from_rgba_unmultiplied(...))` — line chart stroke with conditional literal weights and raw RGBA color

**Total hardcoded sites: ~2**

---

### heatmap_pane.rs

Uses widgets:
- `PaneHeader`: line 42

Hardcoded UI:
- Lines 106, 117: `egui::Color32::from_rgba_unmultiplied(...)` — cell background/text color derived from price delta; painter code, partially acceptable
- Line 122: `painter.rect_stroke(inset, 2.0, egui::Stroke::new(1.5, t.text), ...)` — hardcoded radius `2.0` and stroke `1.5` for selected cell

**Total hardcoded sites: ~3**

---

### research_panel.rs

Uses widgets:
- `FormRow`: lines 35, 57, 80
- `SectionLabel`: lines 20, 24, 49, 71, 94
- `MonospaceCode`: ~18 usages

Hardcoded UI:
- Line 109: `painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(255, 191, 0))` — analyst hold bar fill with literal yellow
- Line 116: `color: egui::Color32::from_rgb(255, 191, 0)` — hold count label yellow
- Line 176: `ui.label(egui::RichText::new(...).monospace().size(7.0).color(...))` — bare label with size literal
- Line 191: `Color32::from_rgb(255, 191, 0)` — economic event importance yellow
- Line 205: `.monospace().size(7.0)` literal size

**Total hardcoded sites: ~5**

---

### rrg_panel.rs — See above (21 sites, mostly painter/quadrant fills)

### dashboard_pane.rs

Uses widgets:
- `EmptyState`: line 41

Hardcoded UI: **0 sites** — cleanest panel in the app.

---

### chart_pane.rs

No widget usage found, no hardcoded UI patterns found. (Delegates entirely to `render_chart_pane` in `gpu.rs`.)

---

### playbook_panel.rs

No widget usage found in panel-level searches. (No direct calls to design-system widgets found at panel scope.)

---

## Phase 3 — Cross-Cutting Summary

### Top 10 Panels by Widget Adoption (Most Widget Calls)

| Rank | Panel | Widget Calls |
|------|-------|-------------|
| 1 | `watchlist_panel.rs` | 77 |
| 2 | `object_tree.rs` | 34 |
| 3 | `discord_panel.rs` | 30 |
| 4 | `research_panel.rs` | 30 |
| 5 | `orders_panel.rs` | 27 |
| 6 | `spread_panel.rs` | 25 |
| 7 | `plays_panel.rs` | 24 |
| 8 | `apex_diagnostics.rs` | 22 |
| 9 | `indicator_editor.rs` | 21 |
| 10 | `scanner_panel.rs` | 18 |

### Top 10 Panels by Hardcoded UI (Most Literal Sites)

| Rank | Panel | Hardcoded Sites |
|------|-------|----------------|
| 1 | `gpu.rs` (all sections) | ~716 total (chart pipeline dominant) |
| 2 | `watchlist_panel.rs` | ~46 |
| 3 | `plays_panel.rs` | ~29 |
| 4 | `indicator_editor.rs` | ~22 |
| 5 | `object_tree.rs` | ~22 |
| 6 | `apex_diagnostics.rs` | ~17 |
| 7 | `rrg_panel.rs` | ~21 |
| 8 | `script_panel.rs` | ~20 |
| 9 | `settings_panel.rs` | ~21 |
| 10 | `discord_panel.rs` | ~19 |

### Most-Used Widgets in the App

| Rank | Widget | Total Usages |
|------|--------|-------------|
| 1 | `MonospaceCode` | ~172 |
| 2 | `SectionLabel` | ~46 |
| 3 | `ChromeBtn` | ~76 |
| 4 | `TextInput` | ~28 |
| 5 | `FormRow` | ~21 |
| 6 | `SimpleBtn` | ~25 |
| 7 | `BodyLabel` | ~18 |
| 8 | `IconBtn` | ~17 |
| 9 | `ToolbarBtn` | 17 (gpu.rs only) |
| 10 | `ListRow` | 8 |

### Least-Used Widgets (Candidates for Removal / Consolidation)

These widgets have **zero usages** outside their own definition files:

- `BrandCtaButton` — defined, never called
- `StatusBadge` — defined, never called
- `TabStrip`, `TabBarWithClose` — defined, never called
- `PaneTitle`, `Subheader`, `MutedLabel`, `CaptionLabel`, `NumericDisplay` — defined, never called
- `ProgressBar`, `ProgressRing`, `Skeleton`, `Toast`, `NotificationBadge`, `TrendArrow` — defined, never called
- `TopNavBtn`, `TopNavToggle`, `PaneTabBtn`, `TimeframeSelector`, `PaneHeaderAction` — defined, never called
- `PaneSymbolBadge`, `PaneTimeframeBadge`, `PaneIndicatorChip`, `PaneStatusStrip`, `PaneHeaderBar`, `PaneFooter`, `PaneHeaderActions`, `PaneDivider` — used internally in `widgets/pane.rs` only, not called by any panel
- `OptionChainRow`, `Table` — defined, never called from panels
- `EarningsCard`, `EventCard`, `NewsCard`, `PlaybookCard`, `SignalCard`, `StatCard`, `TradeCard` — defined, never called
- `NumericInput`, `ToggleRow`, `CompactStepper` — defined, never called
- `RadioGroup`, `Combobox`, `MultiSelect`, `Autocomplete`, `DropdownActions` — defined, never called
- `MenuTrigger`, `SidePaneAction` — defined, never called
- `Splitter`, `ResizableSplit`, `Collapsible`, `Section`, `Accordion`, `Stack`, `Cluster`, `Center`, `Spacer` — defined, never called
- `FieldSet`, `FormSection`, `LabeledControl`, `HelpText`, `ErrorText`, `RequiredMarker`, `InlineValidation` — defined, never called
- `DialogFrame`, `SidePanelFrame`, `TooltipFrame`, `DialogSeparator` — defined, never called
- `PanelHeader`, `PanelHeaderWithClose`, `DialogHeader` — defined, never called

### "Hot" Hardcoded Patterns (Appearing Across Many Files)

| Pattern | Files Affected | Notes |
|---------|---------------|-------|
| `egui::Frame::popup(&ctx.style()).fill(...).stroke(Stroke::new(...))` | `connection_panel`, `indicator_editor`, `news_panel`, `option_quick_picker`, `overlay_manager`, `script_panel`, `settings_panel`, `spread_panel`, `template_popup`, `trendline_filter` | 10+ files — should use `PopupFrame` |
| `egui::Frame::NONE.fill(t.toolbar_bg).stroke(Stroke::new(...))` | `discord_panel`, `object_tree`, `screenshot_panel`, `watchlist_panel` | 4+ files — should use `SidePanelFrame` or `PanelFrame` |
| `Color32::from_rgb(224, 85, 96)` (red danger) | `indicator_editor`, `object_tree`, `watchlist_panel`, `gpu.rs` (toolbar) | 6+ sites — no `danger` token exists; `t.bear` is close but semantically different |
| `Color32::from_rgb(255, 191, 0)` (gold/warning yellow) | `plays_panel`, `portfolio_pane`, `research_panel`, `gpu.rs` | 7+ sites — no `warning` token; needs `t.warn` |
| `Color32::from_rgb(46, 204, 113)` (green success) | `apex_diagnostics`, `dom_panel`, `gpu.rs` (paper mode, account strip) | 6+ sites — `t.bull` is ≈ same; some contexts need explicit `t.bull` use |
| `Color32::from_rgb(230, 70, 70)` (error red) | `apex_diagnostics`, `settings_panel` | 4+ sites |
| `egui::Stroke::new(1.0, ...)` literal `1.0` | `analysis_panel`, `connection_panel`, `feed_panel`, `journal_panel`, `orders_panel`, `signals_panel`, `spreadsheet_pane` | 8+ files — should use `stroke_std()` or `stroke_thin()` |
| `ui.button(egui::RichText::new(...).monospace().size(9.0))` | `watchlist_panel` (context menus) | 8+ calls — should use `MenuItem` / `DangerMenuItem` |
| `egui::RichText::new(...).monospace().size(9.0)` literals | Nearly every file | 40+ sites — most should use `font_sm()` / `font_md()` tokens |
| `Color32::from_rgb(200, 200, 210)` / `(180, 180, 190)` (dim gray) | `discord_panel`, `trendline_filter`, `watchlist_panel` | 5+ sites — should be `t.dim` |

---

## Phase 4 — Special Calls in `gpu.rs`

`gpu.rs` is 20,593 lines. It contains six distinct UI regions plus the chart paint pipeline.

### A) Chart Paint Pipeline (`render_chart_pane`, lines 5378–16082)

Sacred painter calls — not UI chrome. Includes:
- OHLC candle paint, volume bars, drawings, order levels, overlays, signals, footprint, indicators
- Hardcoded sites: **~293 in floating_order_windows region (7660–11163)** + **~113 in render_chart_pane proper (5378–7660)**
- Most painter strokes and colors here are legitimate data-visualization that should remain hardcoded
- **Exception:** pane chrome sub-regions within `render_chart_pane` (header bar, border drawing) that use raw strokes instead of tokens:
  - Lines 5482: `painter.rect_stroke(pane_rect, 0.0, Stroke::new(border_width, border_color), ...)` — pane selection border
  - Lines 5543, 5560, 5572: strokes for resize handles and pane dividers
  - Line 6042: `Color32::from_rgb(220, 90, 90)` — mark entry point color literal

**Widget usage in render_chart_pane:** `PaneHeader` / `PainterPaneHeader` (~20 usages, well-adopted).

### B) Pane Chrome / Header (`PainterPaneHeader`, lines 5630–7660)

Header bar for chart panes. Built on `PainterPaneHeader` widget (correct).

Widget calls: `PainterPaneHeader` for all pane headers, `ToolbarBtn` for pane-level toggles like SIGNALS (line 4476).

Hardcoded sites within pane chrome:
- All `PainterPaneHeader` builder calls pipe through the widget system correctly
- Raw painter calls for the selection ring and resize handles remain unencapsulated

### C) Top Nav Toolbar (`render_toolbar`, lines 3644–4928)

**86 hardcoded sites** (largest non-chart region). Sub-regions:

| Sub-region | Lines | Widget Usage | Hardcoded Sites |
|-----------|-------|-------------|----------------|
| Toolbar frame | 3687–3697 | None | 2 raw `egui::Frame::NONE` |
| Bottom border | 3722 | None | `Stroke::new(1.0, t.toolbar_border)` — literal `1.0` |
| Paper mode line | 3731 | None | `Stroke::new(4.0, Color32::from_rgb(46,204,113))` — literal weight + green |
| Logo painter | 3744–3745 | None | 2 raw `Stroke::new(1.3, t.accent)` |
| Account button | 3757 | `ToolbarBtn` ✓ | CornerRadius::ZERO in hover fill |
| Paper/Live toggle | 3778–3784 | `ChromeBtn` ✓ | `Color32::from_rgb(255,165,0)` — orange literal |
| Range/Draw/Chart menus | 3835–4255 | None | ~40 `ui.menu_button()` / `ui.selectable_label()` with `.size(9.0/12.0)` literals throughout |
| Right-side toolbar buttons | 4476–4904 | `ToolbarBtn` ✓, `SearchPill` ✓ | Some raw `egui::Button` for toggles in submenus |

Notable: All 17 main toolbar buttons correctly use `ToolbarBtn`. The menu dropdown contents (Range, Draw, Chart, MAs, Osc, Vol, Overlay menus) use raw `ui.menu_button()` / `ui.selectable_label()` — no `MenuTrigger` / `MenuItem` adoption.

### D) Account Strip (`account_strip_open`, lines 4928–5040)

**21 hardcoded sites.**

- Frame: raw `egui::Frame::NONE.fill(t.toolbar_bg).stroke(Stroke::new(STROKE_THIN, ...))` — should use a dedicated `AccountStripFrame` or `CompactPanelFrame`
- All account metric labels use raw `ui.label(egui::RichText::new(...).monospace().size(11.0))` — every data label bypasses the widget system; should use `MonospaceCode` (standardized on `size_px(11.0)`)
- `egui::Button::new(...).monospace().size(9.0).strong()` for CANCEL ALL button — should be `ChromeBtn` or `ActionBtn`

### E) Order Entry Form (`render_order_entry_body`, lines 945–2430)

**14 hardcoded sites.** This is the best-maintained section.

Widget usage:
- `Stepper` (line 1082) — qty input ✓
- `SegmentedControl` (lines 1027, 1037, 1059) — order type, TIF, notional mode ✓
- `FormRow` (lines 1130, 1139, 1148) — Limit/Stop/Trail price rows ✓
- `TradeBtn` (lines 1200, 1235) — BUY/SELL ✓
- `MeridienOrderTicket` (line ~970) — full Meridien path ✓

Hardcoded sites:
- Lines 1041–1045: EXT button with `Color32::from_rgb(255,191,0)` — should use `t.accent_warm` or warn token; also raw `egui::Button` instead of `ChromeBtn`
- Line 1093: `Stroke::new(1.0, ...)` — section separator
- Lines 1099, 1119: `size(12.0)` / `size(9.0)` literals on price display labels
- Lines 1107–1110: raw `egui::Button` for order type toggle with `corner_radius(2.0)` — should be `ChromeBtn`
- Lines 1162–1164: raw `egui::Button` for bracket toggle with `corner_radius(2.0)` — should be `ChromeBtn`

### F) Floating Order Panes / DOM Window (`float_order_*`, `order_entry_*`, `dom_*`, lines 7660–11550)

**~318 combined hardcoded sites** — most are in chart painter territory (7660–11000 covers order level rendering on the chart canvas, which is legitimate painter code).

For the actual window chrome:
- `float_order_*` (7660–7760): raw `egui::Frame::popup(...)` + `Stroke::new(1.0, ...)` for window frame; no `PopupFrame` usage
- `order_entry_*` (11191–11390): raw popup frame + header with `ui.label(RichText::new(...).monospace().size(9.0/11.0))` throughout; BID/PRICE/ASK header row uses raw `Label` instead of `MonospaceCode`
- DOM ladder inside order entry (11320–11363): raw `egui::Button` calls for bid/ask size cells instead of `DomRow`; raw `Stroke::new(1.0, ...)` for selection ring
- `dom_*` window (11392–11550): same — raw popup frame, raw bid/ask cells; `confirm_toast_*` window uses raw popup frame instead of `TooltipFrame`/`DialogFrame`

### G) Modal Dialogs at Top Level

Modal dialogs (`connection_panel`, `indicator_editor`, `hotkey_editor`, `option_quick_picker`, `template_popup`) all use `Modal::new(...)` correctly. The `Modal` widget internally uses `PopupFrame` or `PaneHeaderWithClose` via `FrameKind`. These are well-encapsulated.

---

## Cross-Cutting Notes

1. **`egui::Frame::popup()` is used in 10+ files** when `PopupFrame::new()` exists and should be the canonical replacement. This is the single highest-impact migration target.

2. **Raw `ui.button()` / `ui.menu_button()` / `ui.selectable_label()` calls** with inline `RichText` styling appear extensively in toolbar dropdown menus and watchlist context menus. `MenuItem` / `DangerMenuItem` cover the context menu case; toolbar menus have no widget yet.

3. **Literal `.size(N)` on `RichText`** is the most widespread pattern (40+ sites). The tokens `font_sm()`, `font_md()`, `font_lg()` exist but adoption is incomplete.

4. **No `warning` color token exists.** `Color32::from_rgb(255,191,0)` (gold) appears in 7+ files across unrelated semantic contexts (plays T3, analyst hold, portfolio margin, economic importance). A `t.warn` token would consolidate these.

5. **`Color32::from_rgb(224,85,96)` (danger red)** appears in 6+ files. `t.bear` is semantically close but represents bearish price action, not UI danger. A `t.danger` token would be more precise.

6. **The account strip is 100% hardcoded** — no design-system widgets used in any label or metric display. All 21 sites use raw `ui.label(RichText::new(...).monospace().size(11.0))`.

7. **The DOM ladder cells** in both `order_entry_*` and `dom_*` floating windows use raw `egui::Button` instead of the `DomRow` widget that exists specifically for this purpose. `DomRow` is only called once from `dom_panel.rs`.
