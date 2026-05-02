# R5 Token Promotion Summary

**Date:** 2026-05-02  
**Scope:** Apex Terminal design system — outlier-to-token promotion wave  
**Total migrations:** ~80 sites across 6 files · 10 new Theme fields · 1 dead-code deletion

---

## What R5 Did

R5 was a **token promotion wave**, not a structural refactor. The goal was to convert hardcoded `Color32::from_rgb(...)` literals that were being repeated across multiple files into first-class `Theme` struct fields, so every theme variant gets the right color automatically.

R4 had already migrated ~325 sites using `ft()` and `current().*` lookups for the core interactive surface. R5 targeted the remaining **outlier categories**: RRG quadrant fills, warn/notification states, gold/amber semantics, shadow paint, canvas overlay text, and command palette row coloring.

---

## New Theme Fields (R5-1)

Ten fields added to the `Theme` struct at `gpu.rs:83–104` and populated per-theme in the `THEMES` array:

| Field | Semantic role |
|-------|---------------|
| `warn` | Amber/yellow warning — R:R ≥ 1 indicator, active status badges, non-critical alerts |
| `notification_red` | High-urgency error/notification badge color |
| `gold` | Star/favorite icon, earnings highlight, gold accent |
| `shadow_color` | Drop-shadow base (black for dark themes, near-black for light) |
| `overlay_text` | Text rendered directly onto chart canvas / overlay backgrounds |
| `rrg_leading` | RRG Leading quadrant fill (top-right — strong RS + improving momentum) |
| `rrg_improving` | RRG Improving quadrant fill (bottom-right — improving RS) |
| `rrg_weakening` | RRG Weakening quadrant fill (top-left — weakening momentum) |
| `rrg_lagging` | RRG Lagging quadrant fill (bottom-left — weak RS + momentum) |
| `cmd_palette` | `[Color32; 11]` — command palette row colors (background, selected, highlight, category chip, divider, etc.) |

All fields are zero-maintenance: adding a new theme entry automatically inherits all 10 tokens.

---

## Sites Migrated (R5-2 through R5-7)

| Sub-wave | File(s) | Sites | What changed |
|----------|---------|-------|--------------|
| R5-2 | `status.rs` | ~8 | Warn yellow → `t.warn`; notification badge → `t.notification_red` |
| R5-2 | `rrg_panel.rs` | ~15 | All 4 quadrant fills → `t.rrg_leading/improving/weakening/lagging` |
| R5-2 | `command_palette/mod.rs` | ~12 | Palette row colors → `t.cmd_palette[*]` array |
| R5-2 | `watchlist_row.rs` | ~4 | Earnings/gold pill → `t.gold` |
| R5-2 | `dom_panel.rs` | ~2 | Warn/notification inline → `t.warn`, `t.notification_red` |
| R5-4 | `design_preview_pane.rs` | ~30 | Token preview wired to all 10 new fields |
| R5-5 | `components_extra/dom_action.rs` | ~4 | Warn/notification_red + inline stroke removed |
| R5-5 | `components_extra/inputs.rs` | ~2 | Minor cleanup |
| R5-5 | `components_extra/top_nav.rs` | — | **Deleted** (dead code — superseded by `widgets/toolbar/top_nav.rs`) |
| R5-7 | `chart_widgets.rs` | 6 | UI-chrome overlay literals → `t.warn`, `t.overlay_text` |

**R5-3 (SectionLabel):** No migrations — all candidates audited and found legitimately unique (different sizes for semantic purposes). Deferred with no action.

**R5-6 (signature purge):** Deferred — modest leverage post-R4 cleanup. Not executed in R5.

---

## Truly Intentional Remaining Literals

These are **not migration targets**. They are correct as-is:

| Pattern | Location | Why intentional |
|---------|----------|-----------------|
| Purple swatch `rgb(128,0,128)` | `design_preview_pane.rs` | Color system demo swatch — must be an absolute color |
| White toggle knobs `Color32::WHITE` | Various widget painter bodies | Semantic white (knob on dark/light track) |
| `COLOR_AMBER` const (20 usages) | `gpu.rs`, `style.rs`, `form.rs` | Named const → still correct; used as a fallback/const where `ft()` is not in scope |
| `Color32::TRANSPARENT` (≥8) | Frames, overlays | Semantic transparency — not a theme value |
| Chart-paint canvas paths (~290 Color32 in `gpu.rs` + `chart_widgets.rs`) | Chart renderer | Intentionally off-limits — candle/indicator paint is domain-specific |
| RRG structural fills (6 remaining in `rrg_panel.rs`) | Layout/overlay geometry | Background geometry color, not quadrant data fills |
| `WatchlistRow`/`DomRow` painter bodies (~48 Color32) | `rows/` | Canvas-adjacent — high-risk, out of scope |

---

## Post-R5 Verified Counts

| Metric | Post-R4 | Post-R5 | Delta |
|--------|---------|---------|-------|
| Panel Color32 (excl. style+preview) | 195 | **183** | −12 |
| Widget Color32 | 239 | **237** | −2 |
| `chart_widgets.rs` Color32 | 86 | **82** | −4 |
| `rrg_panel.rs` Color32 | 15 | **6** | −9 |
| `dom_panel.rs` Color32 | 2 | **1** | −1 |
| `COLOR_AMBER` usages | 14 | **20** | +6 (new sites using the const now that `t.warn` exists) |
| New theme token call sites | 0 | **~102** | +102 |
| Dead files | 0 | **1 deleted** | — |

---

## What Remains for R6

- `gpu.rs` UI-layer overlays (~80–100 Color32): tooltip fills, data label backgrounds → `PopupFrame`/`TooltipFrame`
- `watchlist_panel.rs` context menus (~15 ChromeBtn sites) → `SimpleBtn`
- `chart_widgets.rs` remaining ~16 migratable UI-chrome sites
- `WatchlistRow`/`DomRow` painter body sweep (high-risk, requires careful regression testing)
- `Skeleton` / `NotificationBadge` geometry tokenization
