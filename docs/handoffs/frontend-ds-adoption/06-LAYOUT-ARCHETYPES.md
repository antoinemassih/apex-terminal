# 06 — Layout Architecture

The part no token can fix. Design brief Law 1: *"Token swaps make a theme a different
colour, not a different design."*

> **⚠️ This document revises the design brief's §6.** Reading the six DS specifications
> directly (12,337 lines in `ApexTerminalThemes/design-systems/`) shows the React port's
> "one EditorialLayout serves four themes" was a **mockup-side simplification**, not what
> the design systems actually specify. The corrected picture is better news for us. See §2.

All measurements **[verified]** from `design-systems/*.md`.

---

## 1. The finding that reframes the work

**Each design system is a multi-page application, not a layout.**

| Theme | Views specified |
|---|---|
| **Aperture** | Trade `T` · Live tiles `W` · Dashboard `D` · Portfolio `P` · Research `R` · History `H` · Design · Settings |
| **Meridien** | `/dash` `/apex` `/chart` `/trade` `/tiles` `/port` `/research` `/risk` `/options` |
| **Lucid** | dashboard workspace · apex layout · pages/routes (§2135) |
| **Cadence** | full screen inventory (§11): dashboard tiles · screener · plays · news · alerts · portfolio |
| **Alto** | dual-mode architecture (§1947) — terminal *and* editorial "The Daily" |
| **Mariner** | app shell + panes (trading-terminal shaped) |

The React port collapsed this to one layout per theme because a mockup only needs to look
right in a screenshot. **A real terminal cannot.**

### Why this is good news

The design brief warned that Lucid and Meridien were *"~85 % unbuilt"* because they are
editorial dashboards rather than trading grids. Reading the specs shows something more
tractable:

**Meridien specifies BOTH.** `/dash` is the editorial dashboard **and** `/apex` is a
trading layout:

```css
/* Meridien §9.3 — Apex Trading Layout */
.apex { grid-template-columns: 220px 1fr 280px;
        grid-template-rows: 1.7fr 1fr; }
```

That is a left DOM ladder, a centre chart, a right watchlist and a lower panel row —
**structurally what apex-terminal already renders.**

And Mariner's shell (§7.1) is a trading terminal outright:

```
┌────────────────────────────────────────┐
│  Title bar             height: 42px    │
├────────────────────────────────────────┤
│  Ticker strip          height: 30px    │
├────────────────────────────────────────┤
│ Sidebar L  │   Main pane  │ Sidebar R  │
│  240px     │   fluid      │  260px     │
├────────────────────────────────────────┤
│  Status bar            height: 24px    │
└────────────────────────────────────────┘
```

Compare to apex-terminal's actual shell: top-nav → toolnav → account strip → workspace rail
→ [central] → right rail → bottom dock. **Same skeleton.**

### The corrected strategy

> **Do not replace the workspace. Add a Dashboard view alongside it.**

apex-terminal already has a workspace/view system and a workspace switcher rail. The
editorial dashboard is **a new view within that system**, not a competing shell.

This changes the DS-6 risk profile completely:

| | Brief's framing | Corrected framing |
|---|---|---|
| Work | Replace the shell for 4 themes | Add one new view; re-skin the existing shell for the rest |
| Risk | Competes with `ShellProfile`; touches sacred `core.rs` | New view registers in the existing workspace system |
| State | New layout state, frozen-struct pressure | A view is already a workspace concept |
| Fallback | All-or-nothing | Ship the trading shell skinned per theme; add the dashboard incrementally |

**Each theme's trading view is reachable by skinning what exists.** The dashboard is
additive. That is a fundamentally different project from "rebuild the shell four times."

---

## 2. Archetype A — Aperture: tile mosaic

### Shell **[verified: `aperture.md` §8]**

```css
.app {
  height: 100vh;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 250px;
  grid-template-rows: auto auto 1fr;
  padding: 12px;   gap: 10px;
}
```

| Zone | Height | Width |
|---|---|---|
| Topbar | 48px | full span |
| Subnav | 36px | full span |
| Body | `1fr` | `1fr` |
| Right pane | `1fr` | 250px |

**Minimum width 980px** — pages clamp, horizontal scroll below.

> Note `padding: 12px` + `gap: 10px` on the app root. The whole shell is inset from the
> window edge — this is the tiled/floating-card feel, and `Chrome.region_gap` +
> `Chrome.pane_gap` already model it.

### Tile mosaic (Live tiles, Dashboard)

```css
.tile-grid {
  grid-template-columns: repeat(12, minmax(0, 1fr));
  grid-auto-rows: 92px;
  gap: 12px;          /* tiles use 12px; pages use 10px */
  min-width: 980px;
}
```

Column spans **1, 2, 3, 4, 5, 6, 7, 8, 12** · row spans **1–4**.

| Shape | Use |
|---|---|
| 1×1 | index pill, KPI |
| 2×1 | small KPI |
| 2×2 | KPI, ring |
| 3×2 | chart, ladder |
| 4×2 | hero P&L / chart |
| 6×2 | watchlist, news |
| 12×1 | full-width action strip |

### Page grid (Portfolio, Research, History, Settings)

Same 12 columns, **`grid-auto-rows: minmax(64px, auto)`**, row height 110px vs the mosaic's
92px, gap 10px, `align-content: start`.

### Trade view

```css
.trade { grid-template-columns: 220px 1fr; gap: 10px; }
```

220px DOM ladder | chart card. Optional watch/chain cards on a rail inside the chart area.

### The signature — do not get this wrong

**One rounded coal envelope subdivided by flush hairlines** — not a grid of separate
rounded cards. The outer `PaneGrid` frame owns the `radius-lg` round; leaves are square
(`--ds-pane-radius: 0`) and share edges. `--ds-pane-gap: 7px`, `--ds-pane-inset: -3px`.

**Mapping:** `Chrome.pane_gap` / `pane_gap_alpha` / `region_radius` / `region_gap` exist.
The frame-owns-the-round rule is the new part → `ui_kit/widgets/pane_grid.rs`.

---

## 3. Archetype B — Cadence: dense screens

Spotify-derived. Full screen inventory at `cadence.md` §11: dashboard tiles, screener,
plays, news, alerts, portfolio.

**Signature details** (React Wave-3 notes + `cadence.md` §8.27/§8.44):

- **Continuous DOM ladder** — red asks above / green bids below a **shared** depth-bar
  axis, with a current-price divider. Not two separate half-ladders.
- **17px dense DOM rows**, current row green-highlighted
- **Watchlist sparkline TREND column**
- **T&S with venue + condition codes**, tick-coloured
- **Green "+ Trade" topbar CTA**
- **Widget Tile** (§8.27) and **Big Number** (§8.44) dashboard components
- **Full-pill controls** — `Radii.pill = 99`, blocked today by the `radius_pill` split-brain
  (see [`05-TOKEN-SURFACE-REFERENCE.md`](05-TOKEN-SURFACE-REFERENCE.md) §6)

**Reuse:** `sparkline.rs` exists — wire into `panel_list_row`, do not rebuild.

---

## 4. Archetype C — Editorial dashboard: Lucid + Meridien

### The grid — identical in both **[verified]**

```css
/* lucid.md §8 "Dashboard workspace"  ≡  meridien.md §9.2 ".dash" */
grid-template-columns: 300px 1fr 360px;
grid-template-rows:    auto minmax(0,1.1fr) minmax(0,1fr) minmax(0,0.9fr);
```

- **Left 300px** — watchlist, order book
- **Centre 1fr** — main chart + data
- **Right 360px** — news, portfolio summary
- **Rows** — auto (topbar) → 1.1 (main) → 1.0 → 0.9 (bottom)

### Shell

```
.shell
  .topbar      52px   grid 3-col
  .ticker      36px editorial / 32px Lucid
  [page]       flex: 1
  .statusbar   32px
```

Topbar: `grid-template-columns: var(--sidebar-w) 1fr auto` (sidebar mode) or
`auto 1fr auto` (editorial). `border-bottom: 1px solid var(--b2)`, background
`color-mix(in srgb, var(--bg) 80%, var(--paper))` + `backdrop-filter: blur(8px)`.

> The blur is decorative and expensive in egui. **Approximate with a solid mix; do not
> build a backdrop-blur pass for this.**

### Two panel modes — this is the Lucid/Meridien split

```css
/* Editorial mode — borders, no gaps, flush */
.panel { border-right: 1px solid var(--rule);
         border-bottom: 1px solid var(--rule);
         background: var(--bg); }

/* Lucid card mode — gaps, radius, shadow */
.workspace { gap: var(--s-3); padding: var(--s-3); background: var(--bg-2); }
.panel { border: 1px solid var(--b2); background: var(--paper);
         border-radius: var(--r-4);
         box-shadow: 0 1px 0 rgba(0,0,0,0.03), 0 0 0 1px var(--b2); }
```

**Meridien's variant is the sharper idea** (§9.4):

```css
.workspace { display: grid; gap: 1px; background: var(--rule); }
```

**The 1px gap *is* the border.** Panels carry no border of their own; the workspace
background shows through the gutters. `Chrome.pane_gap` + `pane_gap_alpha` +
`ColorScheme.pane_gap_color` already model exactly this — `pane_gap: 1`,
`pane_gap_alpha: 255`, `pane_gap_color: rule`.

> Note the symmetry: Aperture's "frame owns the round" and Meridien's "gap is the border"
> are the same architectural move — **the container paints the separation, not the leaves.**
> Solve it once in `pane_grid.rs` and both themes benefit.

### Ticker tape

Continuous scroll, `animation: tick 90s linear infinite`. Items separated by a 1px
pseudo-element rule. Lucid: 11px sans, 7px/16px padding. Editorial: mono, 10px/22px.

> apex-terminal has no scrolling ticker. This is a genuinely new widget — but it is a
> *widget*, not a layout. Scope it separately.

### Meridien `/apex` — the trading page

```css
.apex { grid-template-columns: 220px 1fr 280px;
        grid-template-rows: 1.7fr 1fr; }
```

**This is the bridge.** 220px DOM | 1fr chart | 280px watchlist, over a lower panel row.
apex-terminal renders this shape today. **Skin it and Meridien has a real trading view
without any new layout work.**

---

## 5. Archetype D — Alto / Mariner: the trading shell

**[R1 WRONG]** The design brief grouped these with the editorial dashboards. The specs say
otherwise.

### Mariner shell **[verified: `mariner.md` §7.1]**

```
Title bar    42px          z: 20
Ticker strip 30px
Sidebar L 240px │ Main fluid │ Sidebar R 260px
Status bar   24px          z: 20
```

`.app { height: 100vh; display: flex; flex-direction: column; background: var(--bg-0) }`
`html, body { height: 100%; overflow: hidden }`

### Standard pane widths **[verified: §7.2]**

| Pane | Width | Rationale given in spec |
|---|---|---|
| Watchlist (left) | 200–240px | fits a 4-col mono row at 12.5px |
| Order book (right) | 220–280px | fits PRICE/SIZE/TOTAL at 11px mono |
| Order ticket | 260–320px | two-column field stack |
| Modal narrow | 440px | confirm, settings |
| Modal wide | 640px | order ticket, multi-leg |
| Popover | 240–320px | indicator picker |
| Tooltip | auto, max 280px | |
| Command palette | 380–440px | |

> Widths are **derived from content** — "fits a 4-col mono row at 12.5px". That is
> derive-don't-pin stated by the source designer. Our `Width::Narrow/Medium/Wide`
> (240/300/400) is close; check each against its content rather than adopting blindly.

### Component heights **[verified: §7.3]**

| Component | Height | Token name in spec |
|---|---|---|
| Title bar | 42px | `component.titlebar.height` |
| Ticker strip | 30px | `component.ticker-cell.height` |
| Status bar | 24px | `component.statusbar.height` |
| Panel header | min 34px | `component.panel-header.height` |
| Watchlist row | 36px | `component.row-watchlist.height` |
| DOM row | 20px | `component.row-dom.height` |
| Button | 28px | `component.button.height` |
| Segmented | 28px (caps 24px) | `component.segmented.height` |
| Symbol search | 26px | `component.search.height` |
| TF tabs | 26px | `component.tabs-time.height` |
| Screen tabs | 24px | `component.tabs-screen.height` |
| Keycap | 32px | `component.keycap.height` |

**Every one of these maps to an existing `Chrome` knob or `Spacing` field.** Alto and
Mariner need **no new layout** — they need correct values in tokens that already exist,
plus Change A (ramp), Change B (radius) and Change C (bevel temperature).

> **Alto has a second mode** — `alto.md` §1947 "Dual-Mode Architecture": the terminal *and*
> "The Daily" editorial view. Treat The Daily as an optional extra view, not a requirement.

---

## 6. Revised archetype map

| Theme | Trading view | Dashboard view | New layout needed? |
|---|---|---|---|
| **Aperture** | `.trade` 220px + chart | 12-col × 92px mosaic | **Yes** — mosaic + frame-owns-round |
| **Cadence** | dense 3-col | widget tiles | **Partly** — DOM ladder + tiles |
| **Alto** | shell (240/fluid/260) | *(The Daily, optional)* | **No** — skin existing |
| **Mariner** | shell (240/fluid/260) | — | **No** — skin existing |
| **Lucid** | apex layout | 300/1fr/360 × 4 rows | **Dashboard only** |
| **Meridien** | `/apex` 220/1fr/280 | `/dash` 300/1fr/360 | **Dashboard only** |

### What this means for sequencing

**Two themes (Alto, Mariner) need zero new layout.** They are pure token work — the
fastest path to two themes at ~90 %, and the best early proof that the token contract is
right.

**Recommended re-ordering of DS-5/DS-6:**

1. **Alto + Mariner first** — token-only. Validates Changes A–E end to end and gives two
   finished themes early. *Also the cheapest possible test of sibling
   distinguishability, which is the hardest acceptance gate in the programme.*
2. **Meridien `/apex` + Lucid apex** — skin the existing shell. Two more trading views.
3. **Aperture mosaic** — new tile grid, genuinely new but self-contained.
4. **Cadence** — DOM ladder rework + widget tiles.
5. **Editorial dashboard view** — the only true greenfield. Additive; ships last;
   **cuttable without blocking the other five themes.**

That is a materially lower-risk sequence than the brief's, and it front-loads the evidence
that the token work is correct.

---

## 7. Shared primitives across archetypes

| Primitive | Used by | Status |
|---|---|---|
| **Container-paints-separation** (frame-owns-round / gap-is-border) | Aperture, Meridien | `pane_grid.rs` + `Chrome.pane_gap*` — **extend** |
| **12-col span grid** | Aperture mosaic, Cadence tiles | New. `ui_kit/layout/flex.rs` exists as a base |
| **Widget tile** (span-aware card) | Aperture, Cadence | Compose from `panel_card.rs` |
| **Big number / KPI** | all six | `metric_row.rs` exists — needs large-value treatment |
| **Sparkline in row** | Cadence, editorial | `sparkline.rs` exists — wire into `panel_list_row` |
| **Heatmap** | editorial | `heatmap_grid.rs` **exists** |
| **Area chart** (line + gradient fill) | editorial hero | **Genuinely missing** |
| **Order book** (bid/ask/total + depth + spread) | editorial, Cadence | Distinct from the DOM ladder — audit first |
| **Scrolling ticker tape** | Lucid, Meridien, Mariner | **Genuinely missing** |
| **Status bar** | Lucid, Meridien, Mariner | Check against the bottom dock |

> Run **DS-6.2 (missing-primitives audit)** before building any of these. The React
> "missing" list is a React gap list; Rust already has `heatmap_grid`, `metric_row`,
> `sparkline`, `pane_grid`, `panel_card`.

---

## 8. Constraints that bind every layout decision

1. **`core.rs` is sacred.** The shell is assembled there (`draw_chart()` at `core.rs:10026`).
   Adding a *view* inside the existing `CentralPanel` does not touch it. Adding a *shell
   region* does. **Prefer views.**
2. **`Watchlist` / `Chart` are frozen.** Layout state → `state/aggregates.rs` or
   `chart/state/ChartState`, mirrored in `push_to_*` / `sync_from_*` or it will not persist.
3. **`ShellProfile` (Stream S6) overlaps.** It owns nav shape / dock / rail side. This
   document owns central content. **Resolve ownership before writing either** — DS-6.0.
4. **egui panel registration order is layout order**, and `SidePanel`s must be registered
   before the `CentralPanel` in a stable per-frame order.
5. **No backdrop blur.** Approximate with a solid colour mix.
6. **Derive, don't pin.** Every measurement here becomes a token. Mariner's spec derives
   widths from content ("fits a 4-col mono row at 12.5px") — match that discipline.

---

## 9. Open questions — escalate, do not guess

| # | Question | Blocks |
|---|---|---|
| 1 | Is the dashboard a **workspace view** (recommended) or a shell mode? | DS-6.0 |
| 2 | Does archetype follow the theme, or is it a free user choice? | DS-6.0 |
| 3 | Do we adopt the multi-view model (Aperture's 8 tabs, Meridien's 9 routes) or map onto existing workspaces? | DS-5, DS-6 |
| 4 | Ticker tape and status bar — in scope, or deferred? | DS-6.3 |
| 5 | Alto's "The Daily" dual mode — in scope? | DS-6 scope |
| 6 | Is 980px minimum width acceptable, or must we support narrower? | all |
