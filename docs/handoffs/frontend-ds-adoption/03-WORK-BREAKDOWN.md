# 03 — Work Breakdown

> ## ⚠️ SEQUENCING SUPERSEDED — [`10-MASTER-PLAN.md`](10-MASTER-PLAN.md) is the plan of record
>
> After the six-agent architecture audit ([`08`](08-ARCHITECTURE-AUDIT.md)) proved the
> platform itself was the blocker (10 sources of truth, lossy adapter, dormant recipes,
> unwired snapshot), the programme was re-planned **platform-first**: perfect the system
> (milestones M0–M5), then apply the themes as data (tracks T1–T5).
>
> **This document remains the detailed ticket reference** — file paths, acceptance
> criteria, and rationale below are still valid — but its DS-epic *ordering* no longer
> governs. `10` §6 maps every DS ticket to its new milestone. The material changes:
>
> 1. **DS-1/2/3 token-contract work lands inside M1's unified resolver**, not the legacy
>    `StyleSettings` path it would otherwise have been built against.
> 2. **DS-4 theme authoring waits for M1** — authoring ramps into a pipeline that drops
>    them at the door (the audit's central finding) is wasted work.
> 3. **DS-5.1 Aperture mosaic waits for M4's real `Grid`** instead of binary splits.
> 4. **Recipe data (the 259-rules moment) lands in M3**, after widgets can hear it.
> 5. **M0 adds a bug-fix week** the original plan didn't know it needed: black shadows,
>    radius-resolver divergence, dual ambient themes, lossy colour round-trips.

Tickets in dependency order. Sizes are **S** (≤1 day), **M** (2–4 days), **L** (1–2 weeks),
**XL** (multi-week, needs its own plan).

Every ticket's Definition of Done is in [`04-DEFINITION-OF-DONE.md`](04-DEFINITION-OF-DONE.md)
**in addition to** its own acceptance criteria.

---

## Dependency graph

```
DS-0  Verification harness ──────────────────┐  (blocks everything)
                                             │
DS-1  ColorScheme authored ramp ─────┐       │
DS-2  StyleSystem: radius fix + gaps ─┤      │
DS-3  Multi-layer shadows ───────────┤       │
                                     ▼       ▼
DS-4  Author the six schemes ────────────────┐
                                             ▼
              ┌──────────────────────────────┴───────────────────────┐
              ▼                    ▼                    ▼            ▼
   DS-5a Alto + Mariner   DS-5b Meridien/Lucid   DS-5c Aperture  DS-5d Cadence
   (TOKEN-ONLY — no       apex views (skin       mosaic          DOM + tiles
    new layout)           existing shell)        (new grid)
              └──────────────────────────────┬───────────────────────┘
                                             ▼
                        DS-6  Editorial dashboard VIEW  (additive · cuttable)
                                             ▼
                                 DS-7  Sweep and lock
```

DS-1, DS-2 and DS-3 are independent and can run in parallel by three people. DS-3 is the
only schema bump — sequence it so it does not collide.

> ### ⚠️ Resequenced 2026-08-02 — read [`06-LAYOUT-ARCHETYPES.md`](06-LAYOUT-ARCHETYPES.md) §6
>
> Reading the six DS specifications directly showed the brief's archetype grouping was
> wrong in a way that **reduces** the work:
>
> - **Alto and Mariner are trading shells in their own specs**, not editorial dashboards.
>   `mariner.md` §7.1 specifies titlebar 42 / ticker 30 / [L 240 | fluid | R 260] /
>   statusbar 24 — structurally what apex-terminal already renders. **They need zero new
>   layout: pure token work.**
> - **Meridien specifies BOTH** a `/dash` editorial dashboard *and* a `/apex` trading
>   layout (`220px 1fr 280px`, rows `1.7fr 1fr`). The trading view is reachable by skinning
>   the existing shell.
> - **The dashboard is therefore a new *view*, not a new shell** — additive inside the
>   existing workspace system, never touching sacred `core.rs`, and **cuttable without
>   blocking the other five themes.**
>
> **Do Alto + Mariner first.** They validate Changes A–E end to end with no layout work
> confounding the result, deliver two finished themes early, and are the cheapest possible
> test of the hardest acceptance gate (sibling distinguishability).

---

# EPIC DS-0 — Verification harness

> **This blocks everything.** The previous port attempt named "optimistic reporting —
> summaries said themes were 'done' based on token application, not structural comparison
> to source" as a root cause of stall. Do not start DS-1 until DS-0 ships.

### DS-0.1 — Reference capture from the originals · **S**

Capture the six original theme apps at fixed viewports.

- **Do:** `cd ApexTerminalThemes && node server.js`; script Playwright/Chrome to capture
  every page of every theme at 1440×900 and 2560×1440 → `docs/styling/screenshots/reference/<theme>/<screen>.png`
- **Note:** `docs/styling/screenshots/` already exists — check what is in it before adding.
- **Accept:** ≥1 reference PNG per theme per archetype screen, committed or in a documented
  artifact location.

### DS-0.2 — Scripted app capture · **M**

Drive `apex-terminal` headlessly and screenshot every `(theme, style, screen)` combination.

- **Do:** script over `dev_inspector`: `POST /cmd` to set `theme_idx`/`style_idx`,
  `POST /screenshot` to capture. Endpoints verified in `dev_inspector/server.rs`.
- **Watch for:** zombie processes locking `apex-native.exe` (stale binary, silent);
  concurrent `cargo build` against the corpus (phantom failures); constant widget count
  across a sweep (broken harness, not clean UI).
- **Accept:** one command produces a current-state PNG for any `(theme, screen)` pair.

### DS-0.3 — Side-by-side diff report · **S**

- **Do:** generate an HTML contact sheet — reference beside current, per theme, per screen.
- **Accept:** a reviewer can judge fidelity without running anything.

### DS-0.4 — Pixel-sampling assertions · **S**

- **Do:** use `POST /assert` (`assert_engine.rs`, 1,810 LOC) to assert *sampled pixel
  values* against the authored ramps in the design brief's spec sheets.
- **Why:** catches "the token table says `#141311` but the panel renders `#131313`" — the
  exact class of failure in design brief Law 3.
- **Accept:** ramp assertions run in CI; a wrong surface colour fails the build.

---

# EPIC DS-1 — `ColorScheme` authored ramp

Spec: [`02-TOKEN-CONTRACT.md`](02-TOKEN-CONTRACT.md) §2.

### DS-1.1 — Add the 8 `Option<Rgba>` fields · **S**

- **Files:** `design_system/color_scheme.rs`, `baseline.rs`
- **Fields:** `bg_panel` `bg_elevated` `bg_hover` `fg_xmuted` `accent_sub` `bull_alpha`
  `bear_alpha` `border_dim` — all `Option<Rgba>` + `#[serde(default)]`
- **Accept:** compiles; no schema bump; all existing schemes untouched.

### DS-1.2 — Resolved accessors · **S**

- **Do:** `resolved_bg_panel()` etc., following the existing `resolved_success()` /
  `resolved_danger()` pattern. **Call sites must never read the raw `Option`.**
- **Files:** `color_scheme.rs`
- **Accept:** every new field has an accessor; derivation fallback is byte-identical to
  today's behaviour.

### DS-1.3 — Thread through `ComponentTheme` · **M**

- **Do:** `ui_kit` widgets get authored values for free. `surface_raised()`'s 7 %-step
  heuristic default becomes "authored if present, heuristic otherwise".
- **Files:** `ui_kit/widgets/theme.rs`, `chart/renderer/theme_impl.rs`, `theme_adapter.rs`
- **Accept:** a widget taking `&dyn ComponentTheme` renders the authored panel colour with
  no call-site change.

### DS-1.4 — Pipeline plumbing · **M**

- **Files:** `snapshot.rs` · `export.rs` · `import/{model,convert,mapping}.rs` ·
  `theme_pack/validate.rs`
- **Accept:** round-trip export → import → export is lossless for authored fields.

### DS-1.5 — Design-mode wiring · **S**

- **Files:** `foundation/design_tokens.rs` (`dt_*!`), `foundation/design_inspector.rs`
- **Accept:** every new field appears in the F12 editor and edits live.
- **Note:** the most-skipped step in this epic. A token absent from the editor gets tuned
  by rebuild-and-squint.

### DS-1.6 — Tests · **S**

- **Files:** `equivalence_tests.rs`
- **Accept:** all ~22 schemes byte-identical; **no existing test modified**; new tests per
  `02-TOKEN-CONTRACT.md` §5.

---

# EPIC DS-2 — `StyleSystem`: fix the radius split-brain, then fill real gaps

Spec: [`02-TOKEN-CONTRACT.md`](02-TOKEN-CONTRACT.md) §4–§6.

> **⚠️ Revised 2026-08-02.** The first draft of this epic proposed four new fields.
> A full read of `StyleSystem` showed that **bevels, uppercase/mono label flags, serif
> headlines and a per-preset pill radius already exist** — and that one of the "missing
> fields" was hiding a genuine bug. Tickets rewritten. **Before proposing any new token,
> grep the struct.**

### DS-2.0 — Read `Treatments` and `Chrome` end to end · **S** — DO FIRST

- **Do:** read `style_system.rs` `Treatments` (27 flags) and `Chrome` (43 knobs) in full.
  `02-TOKEN-CONTRACT.md` §2 tabulates both.
- **Why:** two of five originally-proposed fields were reinventions. The doc comments in
  this file are unusually good and name specific themes (`"Meridien uses 0 (sharp pill)"`,
  `"the Zed raised button face look — Alto/Mariner"`, `"Aperture signature — orange bar"`).
- **Accept:** you can state which of Meridien's, Alto's and Aperture's signature looks are
  already expressible today.

### DS-2.1 — Unify `radius_pill()` ⚠️ **bug fix, high value** · **M**

The code documents its own defect **[verified: `chart/renderer/ui/foundation/shell.rs:18-21`]**:

> *"Pill reads `StyleSettings.r_pill` which varies per style preset (e.g. Meridien
> r_pill = 0); the ui_kit equivalent `radius_pill()` is a fixed 999.0 constant with no
> preset awareness."*

- **Impact:** every `ui_kit` widget renders a **full pill in every theme**. Meridien's
  square controls are impossible in ui_kit. Two controls side by side — one from
  `foundation::shell`, one from `ui_kit` — have **different radii in the same theme**.
  This is the "half-applied theme" signature.
- **Second defect:** `radius_xs/sm/md/lg` apply `corner_scale_override()`; `radius_pill()`
  does not. The user's "Sharp" preference squares every corner *except* pills.
- **Do:** source `radius_pill()` from `frame_tokens().radius_pill` ← `Radii.pill`; apply
  the corner-scale override; populate `Radii.pill` per style system; audit every
  `radius_pill()` and `Radius::Pill` call site.
- **⚠️ Escalate:** the source comment says unification *"requires the style-axis decision
  deferred to Phase 5."* **Do not decide it unilaterally.**
- **Accept:** `radius_pill()` varies by style system; Meridien square in both paths; Sharp
  affects pills; a screenshot shows identical radii on adjacent ui_kit and foundation
  controls.
- **Zero new fields, zero schema risk.**

### DS-2.2 — Authored bevel tint · **M**

- **Why:** `Treatments.surface_bevel` (`None`/`Raised`/`Inset`) already exists, but its
  doc comment says the tint is *"derived from palette luminance at paint time"* — i.e.
  **achromatic**. Alto and Mariner share ramp, ink, radii and families; they differ by
  accent, ~10 % density, and **bevel temperature** (warm `rgba(255,238,210,.06)` vs cool
  `rgba(190,215,245,.05)`). Luminance derivation removes one of the three.
- **Do:** add `bevel_highlight` / `bevel_shadow` as `Option<Rgba>` on **`ColorScheme`**
  (colour → palette axis); alphas stay on `Treatments` (intensity → style axis).
- **Accept:** Alto and Mariner distinguishable by bevel temperature alone with accent held
  constant; `None` reproduces today's derivation byte-for-byte.

### DS-2.3 — `NumeralTier` · **S**

- **Why:** `serif_headlines` covers Lucid's serif hero, but nothing covers **Aperture's
  hero numerals being sans** (`Inter Tight 500 @ -0.04em`) where every other theme is mono.
- **Also:** consider a `FontRole`-typed `section_header_font` superseding the boolean
  `section_header_mono` — additive: add the typed field, keep the bool, deprecate later.
- **Accept:** Aperture hero numerals render sans and negatively tracked.

### DS-2.4 — `CardRecipe` · **S**

- **Why:** `Spacing` has `cta_padding_x` and `button_padding_x` but **no card padding**,
  and there is no way to express Aperture's `--ds-card-border: none`.
- **Note:** `border_width: Option<f32>` — `None` = *no stroke at all*, not zero-width.
- **Check first:** `Chrome` has `region_radius`, `panel_footer_radius`, `nav_cluster_radius`.
  If the semantics genuinely match, extend `Chrome` rather than adding a struct.
- **Accept:** Aperture cards paint no stroke; Lucid pads 20 px vs Alto's 14 px.

### DS-2.5 — Font weights · **M** — ⚠️ scope before committing

- **Why:** `Typography` has **no weight fields at all**. The DS authors them per role
  (Aperture 700, Alto 600, Cadence 700, Meridien 600, Lucid 700).
- **⚠️ egui caveat:** egui selects weight by *font-family registration*, not a numeric
  weight axis. Shipping 600 vs 700 means registering both faces in `ui_kit/fonts/` and
  mapping the number to a registered family. **This may be a font-loading task, not a token
  task.** Spike it before estimating.
- **Accept:** a written scope decision, then either the tokens or a documented deferral.

### DS-2.6 — Type-scale depth audit · **S**

- **Why:** `Typography` has 5 UI sizes; the DS authors 7. Meridien needs 10/12/13/14/15/20.
- **Do:** audit whether 5 tiers carry all six themes. Add `size_2xs` / `size_base` **only**
  if a theme genuinely cannot be expressed.
- **Accept:** written finding; fields added only where justified.

### DS-2.7 — Text-cascade migration in touched panels · **M**

- **Why:** per-theme type scales only move if call sites use the cascade. Hand-passed
  `FontId`s will not move.
- **Do:** migrate `as_rich` → `as_rich_cascading` in the panels this programme touches.
  Opportunistic, not a global sweep. **Never in `core.rs`.**
- **Accept:** switching Lucid → Meridien changes text size in migrated panels.

### DS-2.8 — Scoped serif for Lucid's hero · **S**

- **Do:** `Treatments.serif_headlines` exists — verify it is wired to
  `Typography.family_display` and applied as a **subtree** `text_styles` override.
- **Accept:** Lucid hero price is serif; nothing else in Lucid changes.

### DS-2.9 — Plumbing + design-mode + tests · **M**

Same 10-file checklist as DS-1, for all 9 style systems. Includes the `TokenSnapshot` trap:
a host that never pushes a snapshot silently renders `DEFAULT_TOKEN_SNAPSHOT`.

---

# EPIC DS-3 — Multi-layer shadows ⚠️ schema bump

Spec: [`02-TOKEN-CONTRACT.md`](02-TOKEN-CONTRACT.md) §4. **Highest visual payoff per unit
of work in the whole programme.**

### DS-3.1 — `ShadowLayer` / `ShadowTint` types · **S**

- **Files:** `style_system.rs`
- **Accept:** `Shadows.card` is `Vec<ShadowLayer>`; `ShadowTint` is semantic, never a
  literal colour.

### DS-3.2 — Schema bump + migration · **M**

- **Files:** `theme_pack/manifest.rs` (`CURRENT_SCHEMA_VERSION` 1 → 2),
  `theme_pack/migrate.rs` (`v1_to_v2`), `theme_pack/validate.rs`
- **Accept:** a v1 `.apextheme` pack loads and renders identically; `SchemaTooNew` still
  rejects future versions.
- **⚠️ Only schema bump in the programme.** Sequence so it does not collide with DS-1/DS-2.

### DS-3.3 — Inset rendering · **M**

- **Files:** `ui_kit/widgets/shadow_pipeline.rs`, `shadow.rs`
- **Do:** outer layers use the existing drop path. Inset layers all have `blur == 0` →
  1px edge strokes clipped to the rect, painted after the fill. **Do not build a general
  inset-blur solver.**
- **Accept:** Alto's four-layer bevel renders correctly; no measurable frame-time
  regression on a card-heavy screen.

### DS-3.4 — Warm/cool highlight resolution · **S**

- **Why:** Alto `rgba(255,238,210,.06)` vs Mariner `rgba(190,215,245,.05)` is the **only**
  palette-level difference between two otherwise-identical themes.
- **Do:** resolve `ShadowTint::Highlight` from the palette.
- **Accept:** Alto and Mariner distinguishable **by bevel temperature alone**.

### DS-3.5 — Light-theme shadow audit · **S**

- **Do:** verify Bauhaus, Peach, Ivory, Newsprint, Lucid, Meridien get soft grey drops.
- **Accept:** no black smudges; `style-mig-lint.sh` check 4 does not rise.

---

# EPIC DS-4 — Author the six design-system themes

Depends on DS-1, DS-2, DS-3. Spec: design brief §5 (six spec sheets with verbatim values).

### DS-4.1 — Transcribe palettes · **M**

- **Source:** `ApexTerminalThemes/terminal/src/global.css:145-530`
- **Files:** `design_system/builtin.rs` → `builtin_color_schemes()`
- **Do:** author the full ramp for `aperture` `cadence` `alto` `mariner` `lucid`; **add
  `meridien`** (absent today — exists only as a StyleSystem).
- **⚠️ Blocked on open question:** named presets vs raw matrix (design brief §11 Q1).
  Registering `meridien` as a palette alias of Lucid is trivial; deciding the *selection
  model* is a product call. **Escalate at kickoff, do not guess.**
- **Accept:** every ramp step pixel-matches the spec sheet (DS-0.4 assertions).

### DS-4.2 — Recalibrate the 9 style systems · **M**

- **Why:** the six React-port style systems were ported when the React side was at
  15–35 % fidelity. It later reached ~90 %. **The ported values are from the wrong era.**
- **Files:** `builtin.rs` → `builtin_style_systems()`
- **Accept:** radii / spacing / heights / tracking match the spec sheets.

### DS-4.3 — Per-theme hover hue · **S**

- **Do:** Aperture `rgba(239,91,59,0.06)`, Alto `rgba(217,152,88,0.07)`,
  Mariner `rgba(110,160,200,0.07)`, Cadence `rgba(255,255,255,0.06)`,
  Lucid `rgba(20,20,15,0.04)`, Meridien `rgba(20,20,15,0.05)`.
- **Accept:** hover carries the right cast in each theme.

### DS-4.4 — Font families · **M**

- **Do:** Aperture Inter Tight / JetBrains Mono · Cadence Inter / JetBrains Mono ·
  Alto + Mariner IBM Plex Sans / IBM Plex Mono · Lucid + Meridien DM Sans / DM Mono
  (+ serif display for Lucid's hero).
- **⚠️ Open question:** ship a third bundled family or substitute (design brief §11 Q5).
- **Accept:** each theme renders in its specified families; licences checked.

---

# EPIC DS-5 — Archetypes A and B

### DS-5.1 — Aperture mosaic · **L**

- **Signature:** **one** rounded coal envelope subdivided by flush hairlines — *not* a grid
  of separate rounded cards. The outer `PaneGrid` frame owns the `radius-lg` round; leaves
  are square (`--ds-pane-radius: 0`) and share edges. `pane_gap: 7`, `pane_inset: -3`.
- **Files:** `ui_kit/widgets/pane_grid.rs`, `Chrome`
- **Accept:** ≥85 % side-by-side; the frame-owns-the-round rule is visibly correct.

### DS-5.2 — Cadence dense 3-column · **L**

- **Details** (React Wave-3 notes): continuous DOM ladder — red asks above / green bids
  below a **shared** depth-bar axis with a current-price divider; watchlist sparkline TREND
  column; T&S venue + condition codes with tick colouring; 17px dense DOM rows with the
  current row green-highlighted; green "+ Trade" topbar CTA.
- **Reuse:** `sparkline.rs` exists — wire into `panel_list_row`, do not rebuild.
- **Accept:** ≥85 % side-by-side; controls are full pills.

---

# EPIC DS-6 — Archetype C: editorial dashboard · **XL**

> The largest item. The React team measured this at **~85 % unbuilt** when approached as a
> recolour, and rebuilding it on the correct structure took all four themes from 10–35 % to
> ~90 % at once. **Needs its own plan document before any code.**

### DS-6.0 — Resolve the `ShellProfile` overlap · **S** — DO FIRST

`docs/migration/shell-profile.md` (Stream S6) already designs `NavStyle`/`DockStyle`/
`RailSide` and is **draft, awaiting sign-off, no code written**. `theme-authoring/README.md`
already reserves `shell_profile` as an unparsed forward-compatible blob.

- **Do:** decide whether archetype selection is a `ShellProfile` extension or a separate
  axis, **before** writing either.
- **Risk if skipped:** two competing layout-selection mechanisms.
- **Accept:** a written decision, signed off, referenced from both documents.

### DS-6.1 — `DashboardShell` · **L**

Four zones: hero row (large price + metric grid + area chart) → three-column (watchlist /
chart / order book) → utility row (news / heatmap / T&S / ticket) → footer (P&L stat cards).

- **⚠️ State constraint:** new layout state goes on `state/aggregates.rs` or
  `chart/state/ChartState` — **never** `Watchlist`/`Chart` (ADR-0001), and must be mirrored
  in `push_to_*`/`sync_from_*` or it will not persist.

### DS-6.2 — Missing primitives audit · **S** — DO BEFORE DS-6.3

The React "missing primitives" list (AreaChart, MetricGrid, HeatmapGrid, OrderBook,
StatCard, DashboardShell) is a **React** gap list. Rust already has `heatmap_grid.rs`,
`metric_row.rs`, `sparkline.rs`.

- **Do:** audit all six against `ui_kit/widgets/` and produce a real build list.
- **Accept:** a written list of what genuinely needs building. **Do not skip this.**

### DS-6.3 — Build the genuinely-missing primitives · **L**

Likely: `AreaChart` (line + gradient fill — *not* candles), `MetricGrid` (large-value
treatment), `OrderBook` (bid/ask/total 3-col + depth bars + spread row — distinct from the
DOM ladder), `StatCard`. Follow the `CLAUDE.md` new-widget checklist.

### DS-6.4 — Four brand shells · **L**

`lucid` cream/terracotta · `meridien` cream + dark chrome + mono caps ·
`alto` warm-dark/amber · `mariner` steel-dark/blue at 10 % tighter density.

Trading shells (Alto/Mariner) render **candles** in the centre chart while the **hero stays
an area chart**. Light shells keep area throughout.

- **Accept:** ≥85 % each; light-parity walk clean for Lucid + Meridien.

---

# EPIC DS-7 — Sweep and lock

### DS-7.1 — Retire pinned literals · **M**

Every chrome literal found en route becomes a derived token. The frozen-chrome pattern is
recurring in this repo — a value pinned to what a token *used to* produce.

### DS-7.2 — Lower the ratchets · **S**

Per `docs/migration/README.md`: confirm the live count dropped, edit `baselines.toml`,
commit with a note. **Ceilings only go down.**

### DS-7.3 — Per-theme regression snapshots · **S**

Wire DS-0 captures into CI so a palette change that breaks a theme fails the build.

### DS-7.4 — Documentation reconciliation · **S**

- Update `docs/styling/INDEX.md` (stale paths — cites `ui_kit/theme.rs`, which does not
  exist; says "8 chart themes" when there are ~22 schemes) or mark it superseded.
- Fold the outcome into `docs/DESIGN_SYSTEM.md`.
- Record decisions on the design brief's five open questions.

---

## Escalate at kickoff — do not guess

| # | Question | Blocks |
|---|---|---|
| 1 | Named presets vs raw `theme_idx × style_idx` matrix? | DS-4.1 |
| 2 | Is archetype a theme property, a user choice, or fixed pairings? | DS-6.0 |
| 3 | All four editorial themes, or Lucid first as proof? | DS-6 scope |
| 4 | Aperture `border: none` — accept, or allow a hairline fallback? | DS-2.4 |
| 5 | Bundle a serif display family, or substitute? | DS-4.4 |
| 6 | Does `ShellProfile` (S6) get signed off, and who owns the overlap? | DS-6.0 |
