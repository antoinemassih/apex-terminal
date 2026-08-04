# 10 — MASTER PLAN: Perfecting the System, Then the Themes

**Status:** Plan of record. Supersedes the *sequencing* of `03-WORK-BREAKDOWN.md` and
operationalises `09-DESIGN-VISION.md`. Individual ticket detail in `02`/`03` remains valid
where referenced; §6 maps every old ticket to its new home.

**The strategy in one sentence:** bring the platform to a measurably "perfect" level —
one resolver, one context, live recipes, real layout, enforced — *then* the six design
systems become data (tokens + recipe files + track lists), and applying them is
mechanical.

**Why platform-first is now justified:** the audit (`08`) proved the failure mode of
every previous attempt was theming against a fractured platform — 10 sources of truth,
a lossy adapter, dormant recipes. Authoring Aperture's ramp into a pipeline that drops
it at the door is wasted work. The platform must stop lying before the themes can start
telling the truth.

---

## 1. Definition of PERFECT — the exit criteria

"Perfect" is not aesthetic judgment; it is this table. Each criterion is a test, a trace,
or a count that CI can hold. The platform is done when all rows are green.

| # | Criterion | Measured by |
|---|---|---|
| P-1 | **One resolver.** `(ColorScheme × StyleSystem × RecipeSet) → DesignSnapshot → TokenSnapshot`, once per frame. Zero production reads of `StyleSettings::current()` | grep count = 0 (from 187); trace test on the 3 examples from `08` §1.3 |
| P-2 | **Adapter totality.** Every authored field of `ColorScheme` + `StyleSystem` is reachable at a pixel | a written totality test: for each field, a synthetic theme that moves only it produces a pixel diff |
| P-3 | **Round-trip lossless.** Pack → activate → export → identical | byte-compare test (today it destroys 5 colour fields + most dimensions) |
| P-4 | **One context, scoped.** `StyleCtx` push/pop keyed on `Ui`/pane; ambient singleton and `PortableTheme` deleted | two panes render two full themes (incl. chrome/popups) in one screenshot; playground shows 2 densities in one frame |
| P-5 | **Cascade reaches components.** `TextStyle` lives in `ui_kit`; a subtree override restyles a `ui_kit::Button` | the override test renders; ui_kit `RichText` baked-`.size()` count → 0 |
| P-6 | **One font ladder.** `Body < BodySm` inversion impossible | ladder-monotonicity test runs against the *live* path for all 9 style systems |
| P-7 | **Recipes live.** ≥ 90 % of ui_kit widgets resolve through `StyleCtx` recipes; Button consults `button.*` | adoption gate (new): widgets-consulting / keys-consumed counters in CI |
| P-8 | **One interaction system.** hover/press/focus/selected resolve via `apply_interaction` + recipe deltas | hand-rolled `.hovered()` styling in touched files → 0; count ratchets down from 196 |
| P-9 | **One radius/stroke/shadow resolution.** CornerScale, BorderWeight, hot-reload apply uniformly; themed shadows everywhere | Sharp-scale screenshot: ui_kit + foundation + egui-native all square; zero literal-black shadows |
| P-10 | **Layout expressible.** `Grid` (tracks/spans/auto-rows) + intrinsic sizing (`MeasureFunc`) + definite-height columns; structural dims (row/header heights, shell tracks) are tokens | headless solve tests; the 3 archetype fixtures render (12-col mosaic, 300/1fr/360, fixed-height shell) |
| P-11 | **Per-pane + preview correctness.** Pack activation targets all panes; Theme Studio previews without global mutation | mixed-theme screenshot; preview test |
| P-12 | **Enforced, positively.** All gates green in CI; adoption metrics exist (recipes, cascade, flex); `sx_ratchet.sh` fixed or retired; AST lint for positional radii | CI config + gate self-tests |
| P-13 | **Deletion ledger complete.** `StyleSettings`, dual font ladders, `PortableTheme`, `gpu::THEMES`, `ThemeRegistry` (or wired), Sx-states-as-system, `shell_variants` enums, binary-split dashboard path | the code is gone; `09` §3 table checked off |
| P-14 | **Docs truthful.** CLAUDE.md, UI_WORKFLOW, styling/INDEX match measured reality | doc-drift review vs `08` §7 |

**Capability ⇒ theme mapping** (why each row exists):

| Theme requirement | Exit criteria that deliver it |
|---|---|
| Authored 4-step ramps, non-monotonic Lucid, warm Aperture | P-1, P-2 |
| Hue-carrying hover (Aperture orange / Alto amber / Mariner steel) | P-2, P-8 |
| Alto-warm vs Mariner-cool bevel | P-2 (tints), P-9 |
| Multi-layer card shadows (all six) | P-2, P-9 |
| Cadence 999 px pills / Meridien 0 px squares — everywhere | P-9 |
| Meridien mono-CAPS labels reaching every widget | P-5, P-7 |
| Aperture sans hero numerals | P-2 (NumeralTier), P-6 |
| Meridien airy / Mariner tight spacing as authored data | P-2 (Spacing→gap wire) |
| Per-theme type scale that actually moves | P-5, P-6 |
| Structural restyle per theme with zero widget edits | **P-7 — the crown jewel** |
| Aperture 12-col mosaic; editorial 300/1fr/360; Mariner shell rows | P-10 |
| Mariner "10 % tighter" as a scoped property | P-4 |
| Six themes coexisting in preview/panes without bleed | P-4, P-11 |
| Trustworthy "done" claims | P-12 + the DS-0 screenshot harness |

---

## 2. Structure: two tracks, one graph

```
PLATFORM TRACK                                   THEME TRACK
──────────────                                   ───────────
M0  Verify + stop the bleeding  (≈1.5 wk)
 │
M1  One source of truth         (≈3–4 wk)  ──►  T1  Alto + Mariner        (token-only,
 │        (token contract A/C/D/E land here)         platform validation)
M2  Scoped context + cascade    (≈2–3 wk)  ──►  T2  Lucid + Meridien
 │                                                   trading views (skin)
M3  Recipes + states live       (≈3 wk)    ──►  T3  Recipe data ×6 →
 │                                                   Cadence chrome
M4  Layout declarative          (≈3–4 wk)  ──►  T4  Aperture mosaic ·
 │                                                   Editorial dashboard
M5  Geometry endgame            (≈2 wk, rolling)
                                                T5  Six-pack DoD gates +
                                                     CI snapshots
```

- Platform milestones are **strictly sequential** (each consumes the previous).
- Theme milestones **interleave**: each starts the moment its platform dependency lands,
  and each is a live validation of that milestone (T1 exists to prove M1 with real
  stakes).
- **Effort:** platform ≈ 11–14 engineer-weeks serial; M2/M3 partially parallelise by
  widget family, M4/M5 by file → ≈ 8–10 calendar weeks with 2–3 engineers. Theme track
  adds ≈ 6–8 weeks, largely overlapped.
- `core.rs` is untouched throughout, except the pre-existing shell branch points under
  their existing governance.

---

## 3. PLATFORM TRACK

### M0 — Verify + stop the bleeding (≈1.5 wk, one owner)

The harness first — nothing downstream is falsifiable without it — then the one-liners
from `09` Phase 0.

| ID | Item | From | Accept |
|---|---|---|---|
| M0.1 | Screenshot harness: reference capture (originals @1440/2560) + scripted app capture over `dev_inspector` (`POST /cmd` theme/style sweep, `POST /screenshot`) + side-by-side contact sheet + pixel-sample `POST /assert` ramps | `03` DS-0.1–0.4 | one command → reference-vs-current pair for any (theme, screen); ramp asserts in CI |
| M0.2 | **Black-shadow fix** (`style.rs:2942` clobbers `t.shadow_color`) | `09` 0.1 | light-theme popup screenshot |
| M0.3 | **Radius resolver unification** (`shell::Radius::corner()` + `apply_ui_style` → `radius_*()`; reconcile 4 pill defaults) | `09` 0.2 / `02` Change B | Sharp squares everything uniformly |
| M0.4 | **Single ambient theme** (drop `PortableTheme` stash or make fields real) | `09` 0.3 | PanelCard/PanelSection identical via either path |
| M0.5 | **Lossless colour round-trip** (5 dropped fields) | `09` 0.4 | byte-compare pack round-trip |
| M0.6 | 9 non-core `gamma_multiply(lit)` → direction-aware | `09` 0.6 | light-theme hover correct off-canvas |
| M0.7 | 29 literal font sizes → tokens | `09` 0.7 | typography 100 % |
| M0.8 | Gate hygiene: exempt `tps_overlay`/`bug_anchor`; add `Stroke::new(<lit>` + `gamma_multiply(<lit>` patterns; fix-or-retire `sx_ratchet.sh`; **all gates into CI** | `09` 0.8 | gates green in CI; honest baseline ≈479 |
| M0.9 | Registry decision: delete `ThemeRegistry`/`ActiveTheme` **or** commit to wiring in M1 (write the decision) | `09` 0.5 | ✅ **DECIDED (2026-08-02): DELETE in M1.** `begin_frame()` will call `design_system::snapshot::snapshot()` directly — the registry adds an indirection layer with zero external consumers. `DesignSnapshot` itself IS wired (M1.1); `ThemeRegistry`/`live_registry()`/`ActiveTheme` are deleted as part of M1.5 cleanup. |
| M0.10 | Doc truth-sync (CLAUDE.md buttons/Density; UI_WORKFLOW Taffy; 903 figure) | `09` 0.9 | P-14 partial |

**Gate to M1:** harness demo + all M0 fixes screenshot-verified + CI green.

> **EXECUTION STATUS (2026-08-03, session 4):**
> **M3 ✅ FULLY COMPLETE** — M3.4b re-authored everything M3.2 unblocked:
> **80 recipe declarations** across the six styles (+28 per-side borders,
> +23 inset bevels, +27 weights, +2 gaps). All four originally-blocked classes
> are CLOSED. Mariner's accent finally reads as a *precision marker* (LEFT edge
> on the DOM needle, TOP stripe on the active pane) instead of a four-sided box;
> Cadence's Spotify signature (pill + inset highlight + 700 weight) fully lands.
> Visual gate re-run: sibling test still passes with the full layer live.
> **M4 in progress:**
> · M4.1 `Size::Content` + measuring constructors — the adoption unblock ✅
> · M4.2 `Surface` padding-inference bug fixed ✅
> · M4.4 **`Grid`** ✅ — Taffy's compiled-but-unused grid feature now wrapped.
>   **Aperture's 12-col × 92px mosaic solves exactly** (4-col hero = 436px,
>   2-row span = 196px), as does the editorial 300px/1fr/360px dashboard and
>   its 1.1/1.0/0.9 row weights. The layout the audit called inexpressible.
> · M4.5 structural tokens ✅ — row heights, splitter width and the rail
>   presets move from hard literals onto `Density`, so **themes can change
>   PROPORTIONS, not just gutters** (the audit's stated gap). Defaults equal
>   the former literals.
> · M4.3 chrome migration: agent in flight.
> **Gate-metric fix:** the adoption gate's key pattern required a dot, so
> DOTLESS keys (`card`, `tag`, `toolnav`) were invisible; it also counted test
> strings. Now tuple-position only, test module excluded, plus a fourth floor
> (total declarations) because distinct-key count is blind to breadth —
> M3.4b added 14 declarations while distinct keys held constant.
>
> **EXECUTION STATUS (2026-08-03, session 3):**
> **M3 ✅ COMPLETE.**
> · M3.2 Sx vocabulary closed the top-3 evidence-ranked blockers: per-side
>   borders (`Edges`/`EdgesRef`, ~35 CSS rules — every tab underline and ledger
>   hairline), inset bevels (`BevelSpec`/`BevelSpecRef`, ~25 rules — Alto/
>   Mariner's raised-face identity), font-weight (~30 rules, advisory pending
>   per-weight families), plus `gap` on `RecipeDelta` (the free win). All
>   additive; `edges` defaults ALL so existing recipes are byte-identical.
> · M3.6 **recipe-adoption gate** — the metric the audit said never existed.
>   Three FLOORS that may only rise (widgets consulting 7, keys authored 23,
>   styles with data 6), self-tested and in CI. Every other gate is a ceiling
>   on a bad pattern; this one makes "the layer went dormant" a build failure.
> **M4 started:**
> · M4.1 **the layout adoption unblock** — `Size::Content(px)` gives real
>   intrinsic sizing (CSS flex-basis: holds its width, may shrink where `Fixed`
>   never does) plus `Item::{content,galley,text,text_tier}` so callers stop
>   hand-measuring. This is why adoption stalled at 10 sites; the cost of
>   migrating a header is now lower than the arithmetic it replaces.
> · M4.2 `Surface` padding fixed — it inferred pad from the first child's left
>   edge (wrong under `Justify::Center` and for columns) while discarding the
>   authored `Space` it already held.
> Sweep: ui_kit 213, design_system 91, four gates green.
>
> **EXECUTION STATUS (2026-08-03, session 2):**
> **M2 ✅ COMPLETE** — M2.3/M2.7 landed: `ThemeScope` + `TokenScope` RAII
> guards give the palette and tokens CSS-subtree semantics. Wired into the pane
> render path (an inactive pane's ui_kit widgets now use ITS palette, not the
> active pane's) and Theme Studio (its hand-rolled preview/restore dance is
> gone). `TokenScope::density(f)` delivers the per-component Density knob
> CLAUDE.md documented but that only existed as a process global.
> **M3 ✅ CORE COMPLETE — THE CROWN JEWEL IS PROVEN.**
> · M3.1 Button consults recipes (295 call sites; `DefaultButtonStyle` is now
>   the recipe default). `SxDelta::fill_color`/`resolved_border_color` kill the
>   5×-copy-pasted `match fill` boilerplate.
> · M3.3 the designed interaction system promoted into ui_kit; 13 hand-rolled
>   hover sites converted; disabled now dominates.
> · M3.4 **68 authored recipes across the six design systems**, transcribed
>   from the React `[data-ds]` rules with CSS line citations, 100% semantic
>   colour (a test bans hex literals).
> · HOST WIRING: `setup_theme` installed an EMPTY set unless a pack shipped
>   one — and none ships. It now installs the authored set for the ACTIVE
>   STYLE. **This was the last link; the recipe layer is live.**
> · PROOF: Cadence resolves a larger button radius than Meridien from the SAME
>   widget default — the pill-vs-square signature the audit called
>   inexpressible, now data-driven with zero widget-code changes.
> · Found + fixed a live bug in M3.1 via the agent's registry cross-check:
>   `Variant::Chrome` returned None, inerting ~12 authored chrome recipes.
> **Remaining in M3:** M3.2 (Sx vocabulary — the agent produced a ranked
> "unmappable, needs schema" list: inset bevels ~25 rules, per-side borders
> ~35, font-weight ~30, gradients, font-family, uppercase, tracking, fixed
> heights, `gap` on RecipeDelta = cheapest win); M3.5 variant consolidation;
> M3.6 adoption gate.
> Sweep: ui_kit 201, design_system 91, gates 000, baseline 623.
>
> **EXECUTION STATUS (2026-08-02, session 1):**
> **M0 ✅ COMPLETE** — harness live (18 reference PNGs + app captures via port
> 7892), black-shadow/radius/ambient/round-trip bugs fixed, literal sweeps done,
> gates hardened + in CI, docs truth-synced. 5 commits.
> **M1 core ✅ LANDED** — M1.1 STYLE_SYSTEM_STORE + gap/type/alpha ladders LIVE
> (authored values reach TokenSnapshot; proof tests); M1.3 Changes A/C/D/E all
> landed (authored ramp fields end-to-end incl. DTCG; bevel tints joined per
> frame; NumeralTier+CardRecipe consumed by PanelCard/TextStyle; multi-layer
> shadow stacks ADDITIVE — no schema bump needed, v1 packs load unchanged).
> M1.6 verified NO-OP (handler already all-panes — audit claim corrected).
> **Remaining in M1:** M1.4 folded into M2.1 (ladder collapse rides the
> TextStyle move); M1.5 (187 current() reads) = the M2-wave fan-out, gated on
> Chrome-into-snapshot; M1.7 partial (ladders live-editable via dt defaults;
> new ColorScheme fields lack F12 controls — debt).
> 8 commits on feature/ds-adoption-m0. Sweep: 126+ tests green, gates 000.

### M1 — One source of truth (≈3–4 wk, ONE owner — the resolution spine is not parallelisable)

| ID | Item | Accept |
|---|---|---|
| M1.1 | Wire the resolver: `begin_frame()` derives `TokenSnapshot` from `snapshot(&style_system, &color_scheme)`; equivalence suite now guards the live path | trace test P-1 (partial) |
| M1.2 | **Adapter totality**: all 20 Alphas · `spacing.xs..xxl → gap_*()` (un-inerts the whitespace axis) · `mono_*` · `elevation.*` · all 4 shadow roles · `radii.none/full/chip` · the 50 Chrome fields into the snapshot | totality test P-2 |
| M1.3 | **Token contract lands here, in the new path** — `02` rev 2: **A** authored ramp (8 `Option<Rgba>` + `resolved_*`) · **C** bevel tints · **D** CardRecipe/NumeralTier/weight-spike/scale-depth-audit · **E** `Vec<ShadowLayer>` + schema v2 migration + inset rendering in `shadow_pipeline` | `02`'s per-change gates; v1 pack renders identically |
| M1.4 | Collapse the font ladders (one store feeds `font_*()` and TextStyle tiers) | P-6 inversion test |
| M1.5 | Migrate 187 `current()` sites in slices; `StyleSettings` field ratchet monotonic ↓ | count → 0 over M1–M3 |
| M1.6 | Pack activation: all panes; hot-reload carries full `StyleSystem` | P-3, P-11 partial |
| M1.7 | F12/`dt_*` wiring for every new+rescued token (the perennially skipped step) | every token live-editable |

**Gate to M2 / T1 entry:** P-2 + P-3 + P-6 green; a synthetic "move one token" sweep
shows pixel diffs for spacing, alphas, elevation, shadows — the axes that were inert.

### M2 — Scoped context + cascade down (≈2–3 wk, parallelise by widget family)

| ID | Item | Accept |
|---|---|---|
| M2.1 | Move `TextStyle` (16 tiers) into `ui_kit`; chart re-exports | ui_kit imports it |
| M2.2 | ui_kit text cascade-aware (42 `RichText` + 130 `FontId` sites) | baked-size count → 0 in ui_kit |
| M2.3 | `StyleCtx` scope stack (push/pop keyed on `Ui`/pane) replacing the ambient singleton | P-4 two-theme screenshot |
| M2.4 | Slot unification: body closures → `FnOnce(&mut Ui, &StyleCtx)` (Modal/ToolOverlay first) | no theme-drop in dialogs |
| M2.5 | Painter cascade: `font_id_in(ui)` in the 10 densest list/row files | painter coverage 3 %→ measurable ↑ |
| M2.6 | Root defaults on tokens (`gpu.rs:5416-19`) | SpacingScale reaches the root |
| M2.7 | Scoped density (delivers the promised `Density` contract) | 2 densities, 1 frame |

**Gate:** P-4 + P-5 green.

### M3 — Recipes + states live (≈3 wk, parallelise by widget family)

| ID | Item | Accept |
|---|---|---|
| M3.1 | `StyleCtx::from_ctx` in the ~40 parameter-less widgets; Button consults `button.*` (its `DefaultButtonStyle` becomes the recipe default) | Button restyles from pack data |
| M3.2 | Sx gap closure: `fill_color()` helper · shadow reference · `Focused`/`Selected` · per-corner radius (unblocks `select.rs`) · font role; delete dead `opacity` ambiguity | recipe vocabulary sufficient for the 259 rules |
| M3.3 | One interaction system: `apply_interaction` + recipe state deltas; `button_style.rs` tables become defaults; burn `.hovered()` sites in touched files | P-8 ratchet ↓ |
| M3.4 | **Author recipe data ×6** — transcribe the React `[data-ds]` rules (`global.css`) into each pack's `recipes.json` (Cadence pill+bevel; Meridien square+mono; …) | the 259-rules moment |
| M3.5 | Variant consolidation (opportunistic: touched widgets map private vocabularies → `Variant`+keys) | vocab count ↓ |
| M3.6 | **Adoption gate** (new): widgets-consulting-recipes / keys-consumed counters in CI | P-7, P-12 |

**Gate:** switching packs restyles Button/Tabs/Rows/Cards structurally with zero widget
edits, across all six themes, in the playground. **This is the plan's crown jewel.**

### M4 — Layout declarative (≈3–4 wk, parallelise by file)

| ID | Item | Accept |
|---|---|---|
| M4.1 | **Intrinsic sizing**: Taffy `MeasureFunc` ↔ galley bridge (THE adoption unblock) | `Size::Auto` measures content; test |
| M4.2 | Fix `surface.rs:172-174` padding-inference bug | justify/column correct |
| M4.3 | Migrate ui_kit chrome (~120 sites: `pane_grid`, `header`, `panel_list_row`, `select`, `tabs`) | ui_kit manual-geometry 385 → <100 |
| M4.4 | **`Grid` wrapper** (~200 lines over compiled-in Taffy grid: tracks, spans, auto-rows, headless-tested) | 12-col × 92 px fixture solves |
| M4.5 | Structural tokens: `row_height_*`, `HEADER_H`, `TILE_GAP`, splitter, `Width::{240,300,400}` → typed scale, per-style | themes change proportions |
| M4.6 | ✅ **DONE** (2026-08-03). DS-6.0 decided (`13-DS-6.0-DECISION.md`), so `ShellSpec` gave the shell the owner the audit said it lacked. Rails now read `Density.rail_*` — the tokens existed since M4.5 and `Width` was their only consumer, ignoring them. The 300/1fr/360 solve itself landed in **DS-6.1** as a workspace view (`dashboard_layout.rs`), NOT a shell mode, so sacred `core.rs` is untouched. Open follow-up: no style differentiates its rails yet — `06` §4 says widths are content-derived, so that is a measurement task. | rails follow the active style (test asserts the RELATIONSHIP, not the numbers) |
| M4.7 | Definite-height propagation (fixes the silent `MaxContent` collapse) | Mariner shell fixture |

**Gate:** P-10 green; resize sweep shows reflow (constant-widget-count = broken harness).

### M5 — Geometry endgame (≈2 wk, rolling)

Top-10 file list from `08` §6 (pane header → top_nav → watchlist pair → shared shells →
DOM pair → screener buttons → 34-site stroke sweep) · AST lint for positional
`rect_filled` radii (163 sites, the gate's admitted blind spot) · frames converge on
recipes (63 raw → helper) · ratchet floors lowered, zeros hard-banned.

**Gate:** off-canvas geometry-responsiveness comparable to colour (>90 %); P-13 deletion
ledger checked; P-1 fully green (`current()` = 0).

---

## 4. THEME TRACK

Each theme milestone names its **entry criteria** (platform gates) and its **DoD** (always
`04-DEFINITION-OF-DONE.md` per-theme gates: side-by-sides @2 viewports, pixel-sampled
ramps, sibling distinguishability, light-parity, honest score).

### T1 — Alto + Mariner *(entry: M1 · ≈1.5 wk)*
Pure token work; zero new layout (their specs are trading shells — `06` §5). Author full
ramps + hover hues + bevel tints + densities from the `05`/design-brief value tables into
`builtin.rs` **through the new resolver**. Recalibrate their StyleSystems from current
`global.css` (ported values are from the 15–35 % era).
**Purpose beyond shipping two themes:** first end-to-end validation of M1 with real
stakes — if Alto/Mariner can't hit ~90 % here, M1 isn't done, whatever its tests say.
**DoD extra:** the sibling test — bevel temperature + density + accent alone must
distinguish them.

### T2 — Lucid + Meridien trading views *(entry: M1, better after M2 · ≈1.5 wk)*
Register `meridien` as a ColorScheme (palette = Lucid; **selection-model question — named
presets vs matrix — must be answered here**, escalated not guessed). Author both packs.
Skin the existing shell per `meridien.md` §9.3 `/apex` (220/1fr/280) and Lucid's apex
layout. Serif hero via scoped display override (M2). **DoD extra:** Lucid/Meridien
distinguishable on an identical palette (type scale, radii, labels, control radius).

### T3 — Recipe data ×6 + Cadence *(entry: M3 · ≈2 wk)*
M3.4's authored recipes reviewed against the originals per theme. Then Cadence: pills
(M0.3 + recipes), continuous DOM ladder (shared depth axis, 17 px rows, current-row
highlight), sparkline TREND column (`sparkline.rs` exists — wire, don't rebuild), T&S
venue/condition codes, green CTA.

### T4 — Aperture mosaic + Editorial dashboard *(entry: M4 · ≈3 wk)*
Aperture: 12-col × 92 px tile grid on M4.4 `Grid`; one-envelope
frame-owns-round via M4.6; trade view 220/1fr. Editorial dashboard as a **workspace
view** (per `06` §1 — never a shell rewrite): hero row (price + `MetricGrid` + new
`AreaChart`) → 3-col → utility row (heatmap exists) → P&L footer; four brand shells.
Run the missing-primitives audit (`03` DS-6.2) before building — the React gap list is
not the Rust gap list. New widgets follow the CLAUDE.md checklist.
**Cuttable:** the dashboard can slip without blocking T1–T3 themes.

### T5 — Six-pack certification *(entry: all · ≈1 wk)*
All six through the full `04` gate matrix; per-theme regression snapshots wired into CI
(the M0.1 harness becomes permanent); fidelity scores recorded honestly on the React
port's scale; remaining open questions (design brief §11, `06` §9) answered in writing.

---

## 5. Dependency graph (full)

```
M0 ──► M1 ──► M2 ──► M3 ──► M4 ──► M5
        │      │      │      │
        ▼      ▼      ▼      ▼
        T1     T2     T3     T4 ──► T5 (needs all)
```
Hard edges only; T-work may *begin* speculatively earlier (e.g. transcribing token values
is safe any time) but may not *claim fidelity* before its entry gate.

---

## 6. Absorption map — where every old ticket went

| Old (`03-WORK-BREAKDOWN`) | New home |
|---|---|
| DS-0.1–0.4 harness | **M0.1** (verbatim, first) |
| DS-1.* ColorScheme ramp | **M1.3-A** (lands in the *new* resolver, not the legacy path — this is the material change) |
| DS-2.0 read Treatments/Chrome | standing rule (`05` §8 decision tree) |
| DS-2.1 radius split-brain | **M0.3** (promoted: bug-fix week) |
| DS-2.2 bevel tint | **M1.3-C** |
| DS-2.3/2.4 NumeralTier/CardRecipe | **M1.3-D** |
| DS-2.5 font weights spike | **M1.3-D** (spike; may defer with a written decision) |
| DS-2.6 scale-depth audit | **M1.3-D** |
| DS-2.7 cascade migration | **M2.2/M2.5** (systematised, no longer opportunistic-only) |
| DS-2.8 scoped serif | **M2** (subtree override) + T2 |
| DS-2.9 plumbing/design-mode | **M1.7** |
| DS-3.* multi-layer shadows | **M1.3-E** (schema v2 rides the M1 bump window) |
| DS-4.1–4.4 author six schemes | **T1/T2/T3** (post-M1, through the real pipeline) |
| DS-5.1 Aperture mosaic | **T4** (needs M4.4 Grid — the old plan had no grid to build on) |
| DS-5.2 Cadence | **T3** |
| DS-6.0 ShellProfile decision | ✅ RESOLVED 2026-08-03 — see `13-DS-6.0-DECISION.md`; M4.6 unblocked and done |
| DS-6.1–6.4 editorial dashboard | **T4** |
| DS-7.* sweep and lock | **M5 + T5** |

**What the re-plan changes materially:** (1) token-contract work lands in the *unified*
resolver instead of the legacy path it would otherwise have been built against; (2) theme
authoring waits for M1 so authored values actually survive to pixels; (3) Aperture's
mosaic waits for a real Grid instead of being hacked onto binary splits; (4) recipes get
data only after widgets can hear them.

---

## 7. Governance

1. **No new mechanism** (`09` Rule 1). Every item converges or deletes; a fifth theme
   path or third font ladder is wrong by definition.
2. **No fidelity claim without a screenshot** (`09` Rule 2 / `04`). The M0.1 harness is
   the first deliverable for exactly this reason.
3. **The deletion ledger is an exit criterion** (P-13), reviewed at every milestone:
   *what did we delete?*
4. **One owner for the spine** (M1); parallelism only where the graph allows.
5. **Escalate, don't guess**: the standing open questions — theme selection model (T2),
   `ShellProfile` ownership (M4.6), font-weight scope (M1.3-D), dashboard scope (T4) —
   have named decision points; passing one without a written answer blocks the gate.
6. **Sacred/frozen unchanged**: `core.rs` untouched; no `Watchlist`/`Chart` fields; new
   state on aggregates with `push_to_*`/`sync_from_*` mirrors.
