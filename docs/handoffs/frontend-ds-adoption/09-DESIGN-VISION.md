# 09 — Design Vision: One System, Like the Web

**The original vision, restated:** apex-terminal's UI should behave like a
React + Tailwind application — composable components, utility-style recipes, cascading
styles, flexbox/grid layout — because that architecture is what makes applying the six
ApexTerminalThemes design systems *mechanical* instead of artisanal.

**What the audit proved** ([`08-ARCHITECTURE-AUDIT.md`](08-ARCHITECTURE-AUDIT.md)): every
piece of that architecture has already been built in this codebase, usually well. None of
it is finished, none of the predecessors were deleted, and the pieces disagree. The
fidelity ceiling is set by **which of several coexisting resolvers a pixel routes
through**, not by anything inexpressible.

Therefore this vision is **not a build plan. It is a convergence-and-deletion plan.**
The target state is fewer systems, not more.

---

## 1. The North-Star architecture

The web stack, translated mechanism-for-mechanism into egui terms — using only components
that already exist in this repo:

```
  WEB                                APEX TARGET
  ───                                ───────────
  :root { --ds-* } tokens        →   ONE resolver:  ColorScheme × StyleSystem × RecipeSet
  [data-ds="x"] { --ds-* }       →     └─ snapshot() → DesignSnapshot        (exists, dormant)
                                        └─ per frame → TokenSnapshot         (exists, mis-fed)

  <ThemeProvider> context        →   StyleCtx { theme, tokens, recipes }     (exists, 2 adopters)
                                     threaded OR scoped-ambient — but ONE of them

  [data-ds] .ds-btn--primary     →   RecipeSet["button.primary"]             (exists, no data)
  259 structural override rules      per-theme recipe data in each pack

  color-mix(accent 18%, …)       →   Tone × Shade + alpha helpers            (exists, ADOPTED ✅)

  variant={} → pure token fn     →   tokens::Variant + widget defaults as Sx (exists, 3 adopters)

  display:flex / grid            →   ui_kit/layout::Flex + Grid(Taffy)       (flex exists; grid
                                                                              feature compiled in)
  inherited font-size / color    →   TextStyle 16-tier cascade in ui_kit     (exists, wrong crate
                                                                              layer)
```

### The five laws of the target state

1. **One resolver.** `(ColorScheme × StyleSystem × RecipeSet) → DesignSnapshot → TokenSnapshot`,
   computed once per frame. `StyleSettings`, the dual font ladders, the dual ambient
   themes, and `gpu::THEMES` are deleted at the end state. Anything a theme can author
   **must survive to the pixel** — a lossy adapter is a bug, not a shortcut.
2. **One context.** Widgets take `StyleCtx` (theme + tokens + recipes). Slot closures
   receive it. The ambient stash becomes a **scoped stack** (push/pop per pane, per
   preview) rather than a global singleton — that single change makes per-pane themes,
   Theme Studio previews, and two-densities-in-one-frame all correct by construction.
3. **One override layer.** Per-theme component restyling lives in `RecipeSet` data inside
   each theme pack — the Rust equivalent of the React port's 259 `[data-ds]` rules.
   Widgets consult recipes through `StyleCtx`; no widget hardcodes what a recipe could say.
4. **One layout language.** `Flex` (fixed) + `Grid` (new thin wrapper over the
   already-compiled Taffy feature) for chrome, panels, and forms. Painter-exact geometry
   remains legitimate exactly where it is today: chart internals and streaming rows.
   Structural dimensions (row heights, header heights, shell tracks) become tokens.
5. **Enforced, positively.** Ratchets count violations *down* AND adoption *up*. A gate
   that measures the wrong thing, or lives outside CI, is decorative — this audit found
   one of each.

### What we are explicitly NOT doing

- Not sweeping `core.rs` (sacred; 46 % of bypass lives there and stays; its colour layer
  is already healthy).
- Not building a general CSS engine, selector matching, or runtime stylesheets.
- Not adding a dependency — Taffy grid is already in the binary.
- Not renaming for its own sake — convergence deletes code; it does not churn names.

---

## 2. The fix roadmap

Phases are dependency-ordered. **Phase 0 is a week of one-liners that removes two visible
bug classes and three sources of truth.** Each item carries file:line targets from the
audit and a falsifiable acceptance test. Ratchet/gate updates ride with the phase that
makes them true.

---

### PHASE 0 — Stop the bleeding (all S-effort, independently landable)

| # | Fix | Where | Acceptance |
|---|---|---|---|
| 0.1 | **Themed shadows**: `apply_ui_style` writes popup/window shadows with hard `Color32::BLACK`, clobbering the `t.shadow_color` set in `setup_theme` | `chart/renderer/ui/style.rs:2942` | Bauhaus/Lucid popups show soft grey drops; screenshot before/after |
| 0.2 | **One radius resolver**: point `foundation::shell::Radius::corner()` and `apply_ui_style`'s corner writes at `radius_*()` so CornerScale + hot-reload apply uniformly; reconcile the 4 pill defaults | `foundation/shell.rs:25-39` · `style.rs:2911` | CornerScale=Sharp squares ui_kit, RowShell, and egui combos alike |
| 0.3 | **One ambient theme**: drop the `PortableTheme` stash (or carry `section_header_mono`/`cards_float` as real fields) | `gpu.rs:5281` · `theme_impl.rs:114-123` | `PanelCard`/`PanelSection` render identically regardless of which object type reached them |
| 0.4 | **Stop destroying authored colour**: carry `success/danger/warning/info/pane_gap_color` through `color_scheme_to_theme` and the reverse map | `theme_adapter.rs` · `theme_pack_bridge.rs:312-316` | Pack round-trip is lossless for all five fields |
| 0.5 | **Delete dead registry**: `ThemeRegistry`/`live_registry()`/`ActiveTheme` (zero external callers) — OR wire it (Phase 1 decides); delete `gpu::THEMES` reconciliation debt by re-anchoring tests on `baseline.rs` | `design_system/registry.rs` | No dead scaffolding advertising itself as the system |
| 0.6 | **9 non-core `gamma_multiply(lit)`** → direction-aware helper | `tooltip/modal/hover_card.rs`, `watchlist_panel`, `discord_panel`, `frames_widget`, `dom_action`, `dev_inspector` | Light-theme hover/pressed states correct outside canvas |
| 0.7 | **29 literal font sizes** → tokens (typography 97 → 100 %) | 13 files, ≤2 each (list in `08` §6) | Zero non-exempt literal `FontId` sizes |
| 0.8 | **Gate hygiene**: exempt `tps_overlay.rs` + `bug_anchor.rs` (intentionally theme-blind); add `Stroke::new(<lit>` and `gamma_multiply(<lit>` patterns; fix or retire `sx_ratchet.sh` (it measures neither Sx nor recipes and is red at 18 vs 4); wire all gates into CI | `scripts/` | All gates green in CI on merge; baseline 516 → ~479 honest |
| 0.9 | **Doc truth-sync**: CLAUDE.md raw-button count + phantom `Density` enum; UI_WORKFLOW Taffy tense; stale 903 figure | `src-tauri/CLAUDE.md` etc. | Contract docs match measured reality |

**Exit criterion:** two visible bug classes gone (black shadows, radius divergence),
three sources of truth removed, gates trustworthy. *Nothing in Phase 0 touches `core.rs`.*

---

### PHASE 1 — One source of truth (the resolution rebuild)

The keystone: make the documented pipeline the real one.

1. **Wire `DesignSnapshot` into the frame.** `begin_frame()` computes (or receives)
   `snapshot(&style_system, &color_scheme)` and derives `TokenSnapshot` from it — not
   from `StyleSettings`. The 1,072-line equivalence suite finally guards the *live* path.
2. **Make the adapter total — then delete it.** Every authored `StyleSystem` field
   reaches the snapshot: all 20 `Alphas`, `spacing.xs..xxl` → `gap_*()` (this single
   wire un-inerts the whitespace axis: **Meridien's airier spacing becomes authorable**),
   `mono_*`, `elevation.*`, all 4 shadow roles, `radii.none/full/chip`.
3. **Collapse the font ladders.** One store feeds both `font_*()` and the `TextStyle`
   tiers; fixes the Aperture `Body(11) < BodySm(12)` inversion permanently.
4. **Migrate the 187 `current()` call sites** to snapshot accessors, mechanically, in
   slices — each slice shrinks `StyleSettings` (its field-count ratchet already exists).
   `StyleSettings` dies by attrition, not big-bang.
5. **Fix pack activation**: all panes (or explicit target), not pane 0; hot-reload
   carries the full `StyleSystem`, not radii+strokes.

**Acceptance:** the traced examples from `08` §1.3 resolve through one path; editing any
authored token in a pack visibly changes the running app; a written test asserts
adapter totality (every `StyleSystem` field reachable from `TokenSnapshot`).

---

### PHASE 2 — The cascade reaches the components

1. **Move `TextStyle` (16 tiers) down into `ui_kit`** — the dependency direction currently
   fences all 95 ui_kit files out of the cascade. `chart` re-exports for compatibility.
2. **ui_kit text goes cascade-aware**: the 42 `RichText::new` + 130 `FontId` sites in
   ui_kit adopt tiers; widget text sizes stop being baked at construction.
3. **Scoped context**: `StyleCtx` gains a push/pop scope keyed on `Ui`/pane —
   *the* enabler for per-pane themes (fixes the half-honoured pane theming from `08`
   §1.4), Theme Studio previews without the set/restore dance, and scoped density
   (Mariner "10 % tighter" as a property, unblocking the `Density` contract promise).
4. **Unify slot signatures**: every body closure receives the context —
   `FnOnce(&mut Ui, &StyleCtx)` — killing the Modal/ToolOverlay theme-drop.
5. **Painter path**: adopt `font_id_in(ui)` in the top list/row files (485 sites total;
   start with the 10 densest).
6. **Root defaults on tokens**: `gpu.rs:5416-19` spacing literals → tokens, so the
   cascade root respects SpacingScale.

**Acceptance:** a subtree `text_styles` override restyles a `ui_kit::Button`; two panes
render two themes correctly including chrome; the playground shows two densities in one
frame.

---

### PHASE 3 — Recipes go live (the 259-rules moment)

This is the phase that directly buys theme fidelity — the Rust equivalent of the React
port's structural override layer.

1. **`StyleCtx::from_ctx` everywhere**: the ~40 parameter-less widgets stop constructing
   empty RecipeSets; Button actually consults `button.*` keys (its `DefaultButtonStyle`
   becomes the recipe *default*, per the documented resolution chain).
2. **Grow `Sx` to cover what recipes must say**: `fill_color()` helper (kills the 3×
   copy-pasted `match fill`), shadow reference (points into the Phase-1 shadow roles),
   `Focused`/`Selected` states, per-corner radius (unblocks the documented `select.rs`
   hand-roll), font role. Delete the dead `opacity`-field ambiguity.
3. **One interaction-state system**: `apply_interaction` (already the designed one)
   becomes the single hover/press/focus/selected resolver, fed by recipe state deltas;
   retire Sx-states-as-separate-system and burn down the 196 hand-rolled `.hovered()`
   sites in touched files. `button_style.rs`'s tables become Button's recipe defaults.
4. **Author the recipe data** — the actual payoff: for each of the six theme packs,
   transcribe the React port's `[data-ds]` rules into `recipes.json`
   (`button.primary` pill+bevel for Cadence, square+mono-label for Meridien, …).
   The 259 rules are sitting in `global.css` as the spec.
5. **Variant consolidation**: map the 20 private vocabularies onto `Variant` + recipe
   keys opportunistically (each widget touched in Phase 3 converts; no big-bang).
6. **New gate**: recipe-adoption count (widgets consulting recipes; keys consumed vs
   registered) — the metric that never existed.

**Acceptance:** switching packs restyles Button/Tabs/Rows/Cards *structurally* (radius,
bevel, label casing) with zero widget-code changes — demonstrated across all six themes
in the playground.

---

### PHASE 4 — Layout becomes declarative where it should be

1. **Fix intrinsic sizing** — Taffy `MeasureFunc` bridged to galley measurement. This is
   *the* adoption unblock; without it every migration is a regression in ergonomics.
2. **Fix `surface.rs:172-174`** (padding inferred from first child) before `Surface`
   adoption grows.
3. **Migrate ui_kit chrome** (~120 sites: `pane_grid`, `header`, `panel_list_row`,
   `select`, `tabs`) — the design system's own primitives stop hand-computing rects.
4. **`Grid` wrapper** (~200 lines mirroring `flex.rs`, headless-tested) over the
   already-compiled Taffy grid: track lists, spans, auto-rows. **Aperture's 12-col
   mosaic becomes expressible.** Supersedes — does not extend — the binary-split and
   uniform-tiler paths for dashboard use.
5. **Root shell solve**: one `Flex`/`Grid` over the viewport hands each region its rect;
   editorial `300px/1fr/360px` and Mariner's fixed-height rows become theme-authorable
   track lists. (Coordinates with `ShellProfile` — the DS-6.0 decision.)
6. **Structural tokens**: `row_height_*`, `HEADER_H`, `TILE_GAP`, splitter width,
   `Width::{240,300,400}` promoted to the typed scale with per-style resolution.
7. **Tree caching** keyed on `Id` if profiling demands it (one solve per container per
   frame is the budget).

**Acceptance:** Aperture mosaic renders from a track-list spec; a theme changes shell
proportions with zero layout-code changes; resize sweep shows reflow (the harness's
constant-widget-count trap is the negative test).

---

### PHASE 5 — Geometry endgame

1. The effort-ranked top-10 file list from `08` §6 (pane header → top_nav → watchlist
   pair → shared shells → DOM pair → screener buttons → 34-site stroke sweep).
2. **AST lint for positional `rect_filled(rect, 4.0, col)` radii** — the gate's admitted
   blind spot (163 sites, 106 outside core) — a ~30-line syn visitor or clippy lint.
3. Frames: raw `egui::Frame::` (63) converge on the ui_kit frame/card recipes as those
   files are touched.
4. Ratchet floors lowered after each slice; hard-ban patterns that reach zero.

**Acceptance:** off-canvas geometry-responsiveness comparable to colour (>90 %); the six
theme packs pass the `04-DEFINITION-OF-DONE` per-theme gates including sibling
distinguishability.

---

## 3. What gets DELETED (the measure of success)

Convergence is real only if the losers are removed. End-state deletions:

| Artifact | Replaced by |
|---|---|
| `StyleSettings` (99 fields, 3,807-line style.rs shrinks to helpers) | `DesignSnapshot`/`TokenSnapshot` |
| `style_system_to_style_settings` adapter | totality, then nothing |
| Dual font ladders | one store |
| `PortableTheme` ambient stash + duplicate derivation formulas | scoped `StyleCtx` |
| `gpu::THEMES` const + save-to-source text rewriting | `baseline.rs`-anchored tests |
| `ThemeRegistry`/`ActiveTheme` (if not wired in Phase 1) | — |
| Sx state-variants as a separate system | `apply_interaction` + recipe deltas |
| `shell_variants.rs` five enums · legacy header/badge/button free-fns (0 callers) | `Variant` + recipes |
| Binary-split + uniform-tiler dashboard layout paths | `Grid` |
| `sx_ratchet.sh` as currently written | real adoption metrics in CI |

A quarter of this plan's value is in this table. **Every phase review asks: what did we
delete?**

---

## 4. How this maps to the six themes

| Theme signature | Delivered by |
|---|---|
| Aperture warm ramp, orange hover, sans hero numerals | Phase 1 (authored ramp survives) + `02` Changes A/D |
| Aperture one-envelope mosaic, 12-col tiles | Phase 4 Grid + frame-owns-round |
| Cadence full-pill controls, Spotify bevel | Phase 0.2 (radius unify) + Phase 3 recipe data |
| Alto vs Mariner bevel temperature, density step | `02` Change C + Phase 2 scoped density + Phase 3 |
| Lucid non-monotonic paper ramp, serif hero, 20px card pad | Phase 1 totality + `02` Changes A/D |
| Meridien square controls, mono-caps labels, airy spacing | Phase 0.2 + Phase 1 spacing wire + Phase 3 recipes |
| All six: structural restyle without widget edits | **Phase 3 — the point of the whole plan** |

The token-contract changes in `02-TOKEN-CONTRACT.md` (rev 2) slot into Phase 1 (Change A,
ramp), Phase 0/1 (Change B, radius), Phase 3 (Changes C/D/E consumed as recipe-visible
properties).

---

## 5. Sequencing, risk, and the two rules

**Team shape:** Phases 0–1 are one owner + review (they touch the resolution spine and
must not be parallelised). Phases 2–3 parallelise by widget family. Phases 4–5
parallelise by file. `core.rs` is untouched throughout except the two shell branch points
already governed by existing rules.

**Biggest risk:** Phase 1 regressions — mitigated by the equivalence suite finally
guarding the live path, per-slice migration with screenshot diffs (the DS-0 harness), and
the existing field-count ratchet on `StyleSettings` making progress monotonic.

**Rule 1 — no new mechanism.** Every fix routes through an existing system or deletes
one. If a change adds a fifth theme path or a third font ladder, it is wrong by
definition.

**Rule 2 — no fidelity claim without a screenshot.** Unchanged from
`04-DEFINITION-OF-DONE.md`; the audit exists because token application was once mistaken
for visual correctness.
