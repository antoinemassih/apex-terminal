# Typography Map — `src-tauri/src/chart_renderer/`

A scan of every `FontId::monospace(N)` and `FontId::proportional(N)` call site in the
chart renderer. Goal: surface the de-facto type scale in use, expose drift, and
propose a clean canonical scale that can replace the raw float literals.

Total raw call sites scanned: **~525** across `gpu.rs`, `ui/*.rs`, and
`apex_diagnostics.rs`. The bulk lives in `gpu.rs` (~330) and
`ui/chart_widgets.rs` (~190); the rest is spread across panel files.

---

## 1. Existing constants in `ui/style.rs`

Defined at lines 35–40:

```rust
pub const FONT_XS:  f32 = 8.0;
pub const FONT_SM:  f32 = 10.0;
pub const FONT_MD:  f32 = 11.0;
pub const FONT_LG:  f32 = 12.0;
pub const FONT_XL:  f32 = 13.0;
pub const FONT_2XL: f32 = 14.0;
```

Usage counts (occurrences of bare `FONT_*` token across the renderer tree):

| Token       | Value | Direct uses (renderer) | In `FontId::*(…)` calls |
|-------------|------:|-----------------------:|------------------------:|
| `FONT_XS`   |   8.0 | ~80                    | 39 (mono) + 0 prop      |
| `FONT_SM`   |  10.0 | ~110                   | 52 (mono) + 4 prop      |
| `FONT_MD`   |  11.0 | ~6                     | 1 (mono) + 1 prop       |
| `FONT_LG`   |  12.0 | ~14                    | 5 (mono) + 0 prop       |
| `FONT_XL`   |  13.0 | ~3                     | 0                        |
| `FONT_2XL`  |  14.0 | ~1                     | 0                        |

`FONT_SM` and `FONT_XS` are doing 90% of the work; `FONT_MD/XL/2XL` are nearly
unused inside `FontId::*` calls — most "medium" sites use the literal `11.0` or
`13.0` instead of the constant.

---

## 2. Frequency table — every literal size encountered

```
108  monospace(7.0)        ← #1 by a wide margin (caption / sub-caption)
 56  monospace(9.0)        ← body-mono workhorse (text editors, badges)
 52  monospace(FONT_SM)    ← canonical body
 40  monospace(8.0)
 39  monospace(FONT_XS)    ← canonical caption
 39  monospace(10.0)       ← duplicate of FONT_SM, used inline
 35  monospace(6.0)        ← micro labels (corner tags, RRG quadrants)
 19  monospace(7.5)        ← drift between 7 and 8
  9  monospace(9.5)
  8  monospace(8.5)
  6  monospace(13.0)
  5  monospace(FONT_LG)
  5  monospace(14.0)
  6  monospace(11.0/11.5/12.0/12.5)   misc one-offs
  2  monospace(6.5), 1× monospace(5.5), 1× monospace(10.5)

  4  proportional(FONT_SM)
  4  proportional(24.0)
  4  proportional(20.0)
  4  proportional(18.0)
  4  proportional(14.0)
  4  proportional(13.0)
  4  proportional(12.0)
  3  proportional(9.0 / 28.0 / 16.0 / 11.0)
  2  proportional(8.0 / 32.0 / 22.0)
  1  proportional(56.0)   — last-price floating display (chart_widgets:2815)
  1  proportional(42.0)   — large measurement (chart_widgets:1263)
  1  proportional(34.0)   — portfolio total value (portfolio_pane:65)
  1  proportional(11.5 / 8.5 / 7.0 / FONT_MD)
```

The shape is a long tail: a few canonical sizes plus dozens of half-step variations
introduced in single widgets.

---

## 3. Sites grouped by visual purpose

### Display (huge floating numbers / hero readouts)

Sizes in use: **56, 42, 34, 32, 28, 24, 22, 20**

| Site | Size | Purpose |
|------|-----:|---------|
| `ui/chart_widgets.rs:2815` | 56.0 | Floating last-price big-number |
| `ui/chart_widgets.rs:1263` | 42.0 | Large distance-measure value |
| `ui/portfolio_pane.rs:65`  | 34.0 | "TOTAL VALUE" hero number |
| `ui/dashboard_pane.rs:34`  | 32.0 prop | Empty-state glyph |
| `ui/chart_widgets.rs:1257` | 32.0 | Large widget value |
| `ui/portfolio_pane.rs:74`  | 28.0 | Unrealized P&L |
| `ui/chart_widgets.rs:2232` | 28.0 | Shares value |
| `ui/chart_widgets.rs:2521` | 28.0 | Days-to-event |
| `ui/chart_widgets.rs:1681,2123,2291` | 24.0 | Widget primary numbers (RSI avg, score, score) |
| `ui/portfolio_pane.rs:82`  | 24.0 | Position count |
| `gpu.rs:14429`             | 22.0 | Floating overlay number |
| `ui/chart_widgets.rs:2178` | 22.0 | Sentiment % |
| `ui/chart_widgets.rs:2216,2372,*` | 20.0 | Various widget primaries |
| `ui/chart_widgets.rs:2274,2446,2485,3222`/`gpu.rs:2842` | 18–20.0 | Mid-display values |

Recommendation: collapse into **DISPLAY = 24** with a one-off **DISPLAY_HERO = 32**
for genuine hero numbers (last-price, portfolio total). Sizes 18/20/22 should fold
to TITLE.

### Title (panel headers, tab symbols, dialog titles)

Sizes: **13, 14, 12.5, 12.0, 11.5**

- `ui/command_palette.rs:357,390` → 12.0 / 13.0 (palette input + result row)
- `ui/chart_widgets.rs:2406` → 13.0 (change pill)
- `ui/dom_panel.rs:243` → 12.5 (DOM price column)
- `gpu.rs:5467,5823,5901,6248,6279,12271,12567,12682` → tab title fonts
  (`title_font_size` is dynamic but resolves around 12–14)
- `gpu.rs:7935-7949,11022` → 14.0 (drawing tool overlay labels)
- `gpu.rs:6248` → `monospace(11.5)` (single use)

Recommendation: **TITLE = 13.0** (matches existing `FONT_XL`). Use this for tab
symbols, dialog headers, prominent inline titles.

### Subtitle (mid-prominence labels)

Sizes: **11, 11.0**

- `ui/chart_widgets.rs:283` → `proportional(FONT_MD)` icon glyph
- `gpu.rs:4994,7666,15914,15871` → `proportional(11.0)` star/icons
- `gpu.rs:307` → `monospace(11.0)` price column

Very thinly populated — most sites that "should" be subtitle currently use 12.0 or
fall to body. Recommendation: **SUBTITLE = 11.0** (= `FONT_MD`).

### Body (regular text, button labels, palette rows)

Sizes: **10.0, 9.5, 9.0**

- `monospace(FONT_SM)` — 52 sites — canonical body (panels: alerts, journal,
  portfolio, plays, settings, option_quick_picker, dashboard empty state, …)
- `monospace(10.0)` — 39 sites — same value, written inline (gpu.rs trade ticket,
  alerts_panel, overlay_manager, scanner, script_panel)
- `monospace(9.0)` — 56 sites — slightly tighter body for dense chart overlays
  (gpu.rs pattern labels, level labels, BBands)
- `monospace(9.5)` — 9 sites — drift (command_palette hotkeys, gpu badges)

Recommendation: **BODY = 10.0** (= `FONT_SM`). Replace all inline `10.0` and
`9.5` with the constant; keep a separate **BODY_DENSE = 9.0** for chart-overlay
contexts where 10.0 would crowd.

### Caption (small dim helper text, axis labels, footnotes)

Sizes: **8.5, 8.0, 7.5, 7.0**

- `monospace(8.0)` — 40 sites — axis labels, BB/KC inline labels, badges
- `monospace(FONT_XS)` (8.0) — 39 sites — canonical caption
- `monospace(7.0)` — **108 sites** — by far the largest single bucket
- `monospace(7.5)` — 19 sites — fib labels, harmonic vertex labels
- `monospace(8.5)` — 8 sites — DOM micro, RRG, command palette category chip

The 108 uses of `monospace(7.0)` are mostly: section sub-labels ("ENTRY", "STOP",
"DAY P&L", "RVOL", "ROC", "BULLISH"), tag badges, and small-pill text scattered
through `chart_widgets.rs` widget cards.

Recommendation: split caption into:
- **CAPTION = 8.0** (= `FONT_XS`) — primary helper text
- **HAIR = 7.0** — micro labels used inside compact widget cards

### Tabular numerics (prices, sizes, qty)

These are predominantly `monospace(FONT_SM)` already (option_quick_picker,
portfolio rows, plays). Loud outliers:

- `gpu.rs:10658,10663` → `monospace(13.0)` for trade-ticket qty/notional
- `gpu.rs:6279` → `monospace(13.0)` badge font
- `ui/dom_panel.rs:434` → `monospace(12.0)` qty
- `ui/dom_panel.rs:243` → `monospace(12.5)` price column

These are large *because* they are the trade ticket / DOM hot zone. Recommend a
dedicated **NUM_LG = 13.0** constant so the intent reads.

### Icon glyphs (Phosphor / Unicode)

Almost all use `proportional(...)` correctly:

- `gpu.rs:5429,5454` → `proportional(12.0)` caret icons
- `gpu.rs:4994,7666,15914` → `proportional(11.0)` star
- `ui/chart_widgets.rs:283`,`gpu.rs:4458` → `proportional(FONT_MD)` icon
- `gpu.rs:5656` → `proportional(13.0)` close X
- `ui/chart_widgets.rs:288` → `proportional(8.0)` lock 🔒
- `ui/chart_widgets.rs:2842` → `proportional(20.0)` gear
- `ui/chart_widgets.rs:3222` → `proportional(18.0)` ⚡
- `gpu.rs:7243` → `proportional(7.0)` arrow

A handful of icons still slip through as `monospace` (e.g. `dom_panel.rs:104,110`
"+" and "-" buttons, `gpu.rs:10691` Icon::X) — these are minor but worth
normalising to proportional for crispness.

---

## 4. Proposed canonical scale

```rust
// src-tauri/src/chart_renderer/ui/style.rs (extension)

// Display (hero numbers — last price, portfolio total)
pub const FONT_DISPLAY_HERO: f32 = 32.0;   // 56.0/42.0/34.0 → fold here
pub const FONT_DISPLAY:      f32 = 24.0;   // 28/24/22/20 → fold here

// Structural
pub const FONT_TITLE:    f32 = 13.0;       // = FONT_XL  (tab titles, dialog heads)
pub const FONT_SUBTITLE: f32 = 11.0;       // = FONT_MD  (section heads, big icons)

// Numeric emphasis
pub const FONT_NUM_LG:   f32 = 13.0;       // trade ticket qty/notional, DOM hot row

// Body
pub const FONT_BODY:       f32 = 10.0;     // = FONT_SM   (canonical)
pub const FONT_BODY_DENSE: f32 =  9.0;     // chart overlay body

// Helper
pub const FONT_CAPTION: f32 = 8.0;         // = FONT_XS
pub const FONT_HAIR:    f32 = 7.0;         // micro labels in widget cards
```

### Migration impact (rough)

| New token | Replaces literals | Approx sites |
|-----------|-------------------|-------------:|
| `FONT_DISPLAY_HERO` | 56, 42, 34, 32 prop | ~6 |
| `FONT_DISPLAY`      | 28, 24, 22, 20, 18 prop | ~25 |
| `FONT_TITLE`        | 13, 14, 12.5 mono; 13/14 prop | ~25 |
| `FONT_SUBTITLE`     | 11, 11.5 | ~6 |
| `FONT_NUM_LG`       | 13.0 mono in trade ticket / DOM | ~5 |
| `FONT_BODY` (=SM)   | inline 10.0, 9.5 | ~50 collapsed |
| `FONT_BODY_DENSE`   | 9.0 | ~56 |
| `FONT_CAPTION` (=XS)| inline 8.0, 8.5 | ~50 collapsed |
| `FONT_HAIR`         | 7.0, 7.5, 6.5, 6.0, 5.5 | ~165 collapsed |

The single highest-leverage change is folding the 7.0/7.5/6.0/6.5/5.5 tail into
`FONT_HAIR` — that alone normalises ~165 sites.

### Drift to flag

- `monospace(9.5)` (9 sites) and `monospace(8.5)` (8 sites) are pure drift —
  they exist between body and caption with no consistent meaning.
- `monospace(11.5)` and `monospace(10.5)` each appear once.
- `proportional(8.5)`, `proportional(7.0)` are isolated outliers.
- `gpu.rs:10988` uses an inline ternary `monospace(if is_draft { 10.0 } else { 9.0 })`
  — should become `if is_draft { FONT_BODY } else { FONT_BODY_DENSE }`.
- Dynamic sizes (`title_font_size`, `font_size`, `cell_w * 0.55`) are legitimate
  responsive paths and should stay variable; only their *defaults* should map
  to constants.

### Suggested rollout order

1. Land the new constants in `style.rs` next to the existing `FONT_*` tokens.
2. Mass-replace the canonical hot paths first: `7.0 → FONT_HAIR`,
   `9.0 → FONT_BODY_DENSE`, `10.0 → FONT_BODY`, `8.0 → FONT_CAPTION`. These four
   cover ~75% of all sites.
3. Hand-migrate the display tier and title tier (smaller, more contextual).
4. Burn down half-step drift (8.5, 9.5, 11.5) by routing each to its nearer
   neighbour.
5. Audit `monospace(...)` glyph icons and switch to `proportional(...)`.

No source changes are made by this document — it is a map only.
