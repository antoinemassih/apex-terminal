# Frontend Handoff — Design-System Adoption

**Package:** `docs/handoffs/frontend-ds-adoption/`
**Created:** 2026-08-02
**Audience:** engineers joining the apex-terminal UI layer to make it look like the ApexTerminalThemes design systems.

---

## The one-paragraph version

`apex-terminal` is a 197,621-line Rust/egui native trading terminal. It already has a
mature two-axis design system — `ColorScheme × StyleSystem` — with ~22 palettes, 9 style
systems, a 60-knob `Chrome` struct, theme packs, hot reload and an in-app token editor.
Despite that, it does not look like the six design systems it is supposed to embody
(Aperture, Cadence, Alto, Mariner, Lucid, Meridien). **The reason is not missing
infrastructure.** It is (a) a handful of specific expressiveness gaps in the token
types, and (b) the fact that four of the six designs are a *different layout*, and no
amount of palette swapping produces a different layout. This package tells you exactly
what to build.

---

## Read in this order

| # | Document | Why | Time |
|---|---|---|---|
| 1 | **This file** | Orientation, environment, doc map | 10 min |
| 2 | [`01-UI-ARCHITECTURE.md`](01-UI-ARCHITECTURE.md) | How a frame renders; where theme flows; what you may not touch | 45 min |
| 3 | [`../../DESIGN_BRIEF_DS_ADOPTION.md`](../../DESIGN_BRIEF_DS_ADOPTION.md) | **The design spec.** Diagnosis + six per-theme spec sheets. ⚠️ §4.2/4.3 superseded — see the revision banner | 60 min |
| 4 | [`05-TOKEN-SURFACE-REFERENCE.md`](05-TOKEN-SURFACE-REFERENCE.md) | **Complete verified token inventory** + per-theme expressibility matrix. *Read before proposing any new token* | 40 min |
| 5 | [`02-TOKEN-CONTRACT.md`](02-TOKEN-CONTRACT.md) | Exact Rust API changes (rev 2), migration, tests | 45 min |
| 6 | [`06-LAYOUT-ARCHETYPES.md`](06-LAYOUT-ARCHETYPES.md) | **Layout — the part no token fixes.** Revises the brief's §6 | 45 min |
| 7 | [`03-WORK-BREAKDOWN.md`](03-WORK-BREAKDOWN.md) | Tickets: files, acceptance, dependency order | 30 min |
| 8 | [`04-DEFINITION-OF-DONE.md`](04-DEFINITION-OF-DONE.md) | Gates, verification protocol, review checklist | 15 min |
| 9 | [`07-RISKS-AND-GLOSSARY.md`](07-RISKS-AND-GLOSSARY.md) | Risk register, glossary, FAQ | 20 min |
| 10 | [`08-ARCHITECTURE-AUDIT.md`](08-ARCHITECTURE-AUDIT.md) | **Six-agent deep audit** of the design-system architecture vs the React/Tailwind reference — 10 sources of truth, dormant recipe layer, unwired snapshot. Corrects several earlier docs (incl. the 903 figure) | 45 min |
| 11 | [`09-DESIGN-VISION.md`](09-DESIGN-VISION.md) | **The convergence vision.** North-star architecture (one resolver, one context, recipes live, Grid, cascade in ui_kit) + deletion list | 45 min |
| 12 | [`10-MASTER-PLAN.md`](10-MASTER-PLAN.md) | ⭐ **THE PLAN OF RECORD.** Definition of "perfect" (14 measurable exit criteria) · platform milestones M0–M5 · theme tracks T1–T5 · absorption map for every older ticket. Supersedes `03`'s sequencing | 45 min |
| 13 | [`11-AGENT-EXECUTION-PLAN.md`](11-AGENT-EXECUTION-PLAN.md) | How an AI agent team executes `10`: roles/topology, what fans out vs what stays single-owner, the serialized build+verify gate, per-milestone shapes, session estimates | 30 min |

**If you read only two documents:** `08` (what is broken and proven how) and `10` (what
we do about it, in what order, with what gates).

Then, before your first commit, read `src-tauri/CLAUDE.md` end to end. It is binding.

### Two revisions you must not miss

This package was revised on 2026-08-02 after a deeper read of the source. Both revisions
**reduce** scope:

1. **The token gaps are narrower than first written.** Bevels, uppercase/mono label flags,
   serif headlines and a per-preset pill radius **already exist**. One "missing field" was
   hiding a real bug (`radius_pill` — see `05` §6). `02` rev 2 and `05` are authoritative;
   the brief's §4.2/4.3 are superseded.
2. **Two themes need no new layout at all.** Alto and Mariner are trading shells in their
   own specs, not editorial dashboards. They are pure token work. `06` §6 has a
   lower-risk sequence that front-loads them.

**Net effect: less to build, and the fastest path to two finished themes is token work you
were going to do anyway.**

---

## Day-one environment

### Build and run

```bash
cd apex-terminal/src-tauri

cargo apex          # RELEASE build — 122 fps. Use this to LOOK at the app.
cargo apex-dev      # dev build — ~59 fps, fast incremental. Use while iterating.
cargo design        # dev build + design-mode feature → F12 live token editor
```

`cargo design` is the one you want for visual work. See "Tooling" below.

### Run the reference designs side by side

```bash
cd ../ApexTerminalThemes
node server.js      # → http://localhost:5173   the six ORIGINAL theme apps
```

Sidebar = theme; tabs = Pages / Design System / Screenshots; each theme has its own
sub-palette switcher. **This is ground truth.** Keep it open on a second monitor.

There is also a React port at `ApexTerminalThemes/terminal/` (`npm install && npm run dev`
→ `:5175`, routes `/`, `/kit`, `/original`). It is a *port*, not the design — useful for
normalised token values, not for judging fidelity.

> Vite there binds IPv6-only; use `localhost`, not `127.0.0.1`.

### Tooling you must know before you start

| Tool | Invocation | What it gives you |
|---|---|---|
| **egui widget inspector** | `Ctrl+Shift+D` in-app | Hover any pixel → the widget rect + id that owns it. **This is how you answer "what is drawing this line?"** without a 3-minute rebuild cycle. |
| **Headless equivalent** | `POST /cmd {"cmd":"SetUiDebug","on":true}` | Same, driven from the dev harness |
| **Live token editor** | `cargo design`, then `F12` | Sliders + colour pickers for every token, applied next repaint, saveable to `design.toml` |
| **Theme hot reload** | automatic under `cargo design` | DTCG JSON in `styles/` reloads within ~1.5 s |
| **Bug-report anchor** | `Ctrl+Shift+I` | Different from `Ctrl+Shift+D` — only sees regions that registered themselves |
| **Design-system ratchet** | `./scripts/check-design-system.sh` | Per-file budgets for raw primitives / literal sizes / literal colours. Runs in CI. |
| **Style-migration ratchet** | `bash scripts/style-mig-lint.sh` | `&THEMES[0]` hard ban, literal-black shadows, ui_kit decoupling |
| **Sx ratchet** | `./scripts/sx_ratchet.sh` | Sx recipe adoption |

### The dev inspector (headless harness)

`src-tauri/src/dev_inspector/` — 2,684-line HTTP server. **This is your verification loop.**
Verified endpoints:

```
GET  /health  /state  /report  /chart  /panes  /watchlist  /canvas
GET  /metrics /captures /stories /coverage /events /annotations
POST /screenshot  /cmd  /input  /assert  /batch  /reset  /annotations
DELETE /captures /annotations
```

`POST /screenshot` is the single most important one in this package — see
[`04-DEFINITION-OF-DONE.md`](04-DEFINITION-OF-DONE.md). No fidelity claim is
acceptable without it.

---

## Documentation map — what is authoritative, what has drifted

`docs/` holds ~19,400 lines. Much of it is excellent; some of it is stale. **Trust this
table over any individual document's self-description.**

### Authoritative — read and follow

| Doc | Status |
|---|---|
| `src-tauri/CLAUDE.md` | **Binding.** Hard rules, sacred files, frozen structs. |
| `docs/adr/0001-canonical-state-model.md` | **Binding.** Why `Watchlist`/`Chart` are frozen. |
| `docs/UI_WORKFLOW.md` | Current (2026-07-31). The tooling doc. |
| `docs/theme-authoring/README.md` + `schema/` + `token-reference.md` | Current. `.apextheme` bundle format. |
| `docs/migration/README.md` + `baselines.toml` | Current. Ratchet workflow. |
| `docs/migration/shell-profile.md` | **Draft awaiting sign-off.** See "Overlap" below. |
| `docs/UI_AUDIT_2026-07-31.md` | Current audit. |
| `docs/DESIGN_BRIEF_DS_ADOPTION.md` | This programme's design spec. |
| `docs/CODEBASE_DEEP_DIVE_2026-08-02.md` | Newest codebase survey. |

### Known drift — useful but verify before trusting

| Doc | Drift |
|---|---|
| `docs/styling/INDEX.md` | Excellent "where-is-X" map, but **file paths are stale**. It cites `ui_kit/theme.rs` (does not exist — the trait lives at `ui_kit/widgets/theme.rs:17`) and says "8 chart themes" (there are ~22 colour schemes in `design_system/builtin.rs`). Line numbers in `gpu.rs` will have moved. Treat as a *map of concepts*, re-grep for exact locations. |
| `docs/DESIGN_SYSTEM.md` | Predates the `design_system/` module maturity. |
| Older audits (`AUDIT*.md`, `WORLD_CLASS_AUDIT_2026-07-18*`) | Historical record. Useful for "why is it like this", not for current state. |

> **Note on module paths:** `crate::chart_renderer::…` is a module alias that resolves to
> the `chart/renderer/` directory. Both appear in docs. The *directory* is `chart/renderer/`.

### Overlap you must resolve before starting Phase 5

`docs/migration/shell-profile.md` (Stream S6, Wave 1) already designs a `ShellProfile`
struct with `NavStyle` / `DockStyle` / `RailSide` variants. It is **draft, awaiting user
sign-off, no code written**.

It solves a *different but adjacent* problem to this package's layout archetypes:

| | `ShellProfile` (S6) | Layout archetypes (this package) |
|---|---|---|
| Scope | Chrome regions — nav shape, dock placement, rail side | Central content composition |
| Question | "Where does the toolbar live?" | "Is this a 9-pane trading grid or an editorial dashboard?" |
| Status | Design drafted, unsigned | Specified in the design brief, unbuilt |

They compose — an editorial dashboard could run under any `NavStyle` — but **they must be
designed together or you will build two competing layout-selection mechanisms.** Flag this
at kickoff. Note also that `theme-authoring/README.md` already reserves `shell_profile` as
"a forward-compatible JSON blob… stored verbatim… but not yet parsed by the reader."

---

## What already exists — do not rebuild

Read this twice. The single most expensive mistake available to you is reimplementing
something that is already here.

### `src-tauri/src/design_system/` — 11,094 lines

Two-axis system with recipes:

```
ColorScheme (palette)  ×  StyleSystem (dimensions)  +  RecipeSet (per-component overrides)
```

Joined by a **Resolver** at render time; they never mix before that. Resolution chains:

```
field:  recipe override → widget built-in Sx default → no paint
colour: success → bull  |  danger → bear  |  warning → warn  |  info → muted blue
```

| File | LOC | Contains |
|---|---|---|
| `builtin.rs` | 1,498 | ~22 `ColorScheme`s (incl. `aperture`/`cadence`/`alto`/`mariner`/`lucid`) + 9 `StyleSystem`s (`meridien`/`aperture`/`octave` + 6 React ports) |
| `style_system.rs` | 955 | `Typography` `Spacing` `Radii` `Strokes` `Alphas` `Elevation` `Density` `Shadows` `Treatments` `Chrome` |
| `color_scheme.rs` | 361 | `ColorScheme`, `Meta` |
| `snapshot.rs` | 713 | `DesignSnapshot`, `snapshot(style, colors)` at `:373` |
| `recipes.rs` | 575 | Per-component recipes |
| `registry.rs` | 266 | Theme registry |
| `loader.rs` | 949 | Runtime loading |
| `hot_reload.rs` | 318 | Live reload |
| `import/` | 814 | External → internal conversion (`convert.rs`, `mapping.rs`, `model.rs`) |
| `export.rs` | 447 | Token export |
| `theme_pack/` | 1,178 | `.apextheme` bundle, manifest, validate, migrate, registry |
| `equivalence_tests.rs` | 1,072 | Regression cover — **your safety net for Phase 1** |
| `baseline.rs` | 436 | Baseline defaults |

### `src-tauri/src/ui_kit/` — ~95 widget files

`button` `tabs` `select` `modal` `popover` `tooltip` `context_menu` `sparkline`
`heatmap_grid` `metric_row` `pane_grid` `risk_reward_bar` `theme_preview_card`
`shadow_pipeline` `text_engine` `text_subpixel_pipeline` `motion` `skeleton`
`stepper` `pagination` `hover_card` `panel_*` (14 files) `form_*` `input_*` …

Plus `sx/` (composable style recipes: `color.rs`, `recipe.rs`, `recipe_spec.rs`,
`recipes.rs`, `style.rs`), `layout/` (`flex.rs`, `surface.rs`), `tokens.rs`, `scale.rs`,
`icons.rs`, `symbols.rs`, `inspect.rs`, `layer_guard.rs`.

> The React port's "missing primitives" list (AreaChart, MetricGrid, HeatmapGrid,
> OrderBook, StatCard, DashboardShell) is a **React** gap list. On the Rust side
> `heatmap_grid.rs`, `metric_row.rs` and `sparkline.rs` already exist. **Check before
> you build.**

### Other relevant modules

| Module | Role |
|---|---|
| `dev_inspector/` (6,120 LOC) | Headless HTTP harness — screenshots, assertions, stories, coverage |
| `foundation/design_inspector.rs` (3,303 LOC) | In-app F12 token editor |
| `foundation/design_tokens.rs` | `dt_f32!` / `dt_u8!` / `dt_i8!` design-mode override macros |
| `playground/designer.rs` | Design playground |
| `chart/renderer/ui/theme_studio.rs` | In-app Theme Studio (S10) — exports `.apextheme` |
| `chart/renderer/ui/components/design_mode_panel.rs` | Design-mode panel |
| `state/aggregates.rs` (2,337 LOC) | Where new state goes (NOT the frozen god-objects) |

---

## Non-negotiables (full detail in `01-UI-ARCHITECTURE.md`)

1. **`chart/renderer/render/pane/core.rs` is sacred.** 13,672 lines, hottest paint path.
   No design sweeps, no "cleanup" refactors, no helper extraction. Single owner for any
   change. Ratchet scripts do not grep inside it.
2. **`Watchlist` and `Chart` (`gpu.rs`) are frozen.** No new fields. New state goes on
   `state/aggregates.rs` or `chart/state/ChartState`, and must be mirrored in
   `push_to_*` / `sync_from_*` or it will not persist.
3. **Never `&THEMES[0]`.** Hard ban at baseline 0 in `style-mig-lint.sh`.
4. **Never hardcode black for shadows.** Breaks all light themes — and two of your six
   targets (Lucid, Meridien) are light.
5. **Tokens, not literals.**
6. **Walk a light theme before claiming done.**

---

## First-week checklist

- [ ] `cargo apex` runs; you have seen the app
- [ ] `node server.js` in ApexTerminalThemes; you have seen all six originals
- [ ] `cargo design` + F12 — you have moved a token and watched it change
- [ ] `Ctrl+Shift+D` — you have identified what draws one specific pixel
- [ ] `POST /screenshot` returns a PNG
- [ ] All three ratchet scripts pass on a clean checkout
- [ ] You have read `src-tauri/CLAUDE.md` and ADR-0001
- [ ] You can name, without looking: the three sacred/frozen constraints
- [ ] You have opened `design_system/builtin.rs` and found the `aperture` ColorScheme
- [ ] You have raised the `ShellProfile` ↔ layout-archetype overlap at kickoff

---

## Known traps (each has cost someone a day)

- **Zombie processes lock `apex-native.exe`.** `cargo build` then silently fails to
  relink while `deps/` looks freshly built. You will be testing a stale binary and will
  not be told. Kill stale processes before every build.
- **Concurrent `cargo build` against the corpus** produces phantom test failures.
- **A constant widget count across a resize sweep** means your harness is broken, not
  that the UI is clean.
- **Native windows cannot be judged by code reading.** A clean build is not evidence of
  a correct render. Screenshot or it did not happen.
- **`grep --exclude` silently does not filter under Git-Bash grep 3.0 on Windows** —
  which is why `check-design-system.sh` applies exemptions in the pipeline instead. If
  you touch that script, run its self-test.
- **The frozen-chrome pattern.** Chrome dimensions get pinned to a literal that a token
  *used to* produce; the token later changes and the chrome does not follow. Derive,
  never pin.
