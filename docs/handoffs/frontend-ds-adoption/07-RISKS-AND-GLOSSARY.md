# 07 — Risk Register, Glossary, FAQ

---

## 1. Risk register

Ordered by expected cost. **P** = probability, **I** = impact.

### R1 — Reinventing tokens that already exist · **P: high · I: medium**

**Evidence it is real:** two of the five originally-proposed fields in this very programme
were reinventions, and one of them (`control_radius`) would have papered over a live bug
(`radius_pill`, `05` §6).

**Why it recurs:** `StyleSystem` is 955 lines with 27 `Treatments` flags and 43 `Chrome`
knobs. Nobody holds that in their head.

**Mitigation:** DS-2.0 makes reading `Treatments` + `Chrome` a ticket.
[`05-TOKEN-SURFACE-REFERENCE.md`](05-TOKEN-SURFACE-REFERENCE.md) §8 is a decision tree.
**Budget ten minutes of grepping per proposed field.**

**Detection:** any PR adding a `StyleSystem`/`ColorScheme` field without a grep in the
description.

---

### R2 — Fidelity claimed without visual verification · **P: high · I: high**

**Evidence:** the previous port of these exact six systems stalled, and its own
post-mortem named this as a root cause: *"summaries said themes were 'done/working' based
on token application, not structural comparison to source."*

Compounding factor: native windows cannot be judged by reading code, and a clean build is
not evidence of a correct render.

**Mitigation:** DS-0 blocks everything. `04-DEFINITION-OF-DONE.md` requires side-by-side
screenshots at two viewports plus pixel-sampled ramps.

**Detection:** a PR whose evidence section contains no images.

---

### R3 — Silent stale-binary testing · **P: medium · I: high**

Zombie processes lock `apex-native.exe`; `cargo build` then **silently fails to relink**
while `deps/` looks freshly built. You test an old binary and are not told. This has
burned time in this repo before.

**Mitigation:** kill stale processes before every build; make it the first line of the
DS-0.2 capture script. Consider asserting the binary mtime post-build.

**Detection:** a change that "does nothing" — check the binary before debugging the code.

---

### R4 — `TokenSnapshot` never pushed by a host · **P: medium · I: medium**

`ui_kit/style.rs` reads a thread-local snapshot. *"Hosts that don't push a snapshot get the
`DEFAULT_TOKEN_SNAPSHOT` values."* A host that never pushes silently renders defaults —
looks like a token bug, is a plumbing bug.

**Mitigation:** documented in `01` §3 and `05` §5. When a panel won't follow the theme,
**check snapshot-push before checking the token.**

**Detection:** one panel stuck on default values while every other panel themes correctly.

---

### R5 — `ShellProfile` collision · **P: medium · I: high**

`docs/migration/shell-profile.md` (Stream S6) designs `NavStyle`/`DockStyle`/`RailSide`,
is a **draft awaiting sign-off with no code written**, and `theme-authoring/README.md`
already reserves `shell_profile` as an unparsed blob. This programme's layout work is
adjacent. Two competing layout-selection mechanisms is a genuine possibility.

**Mitigation:** DS-6.0 is a blocking ticket requiring a written, signed decision referenced
from both documents.

**Detection:** anyone writing layout-selection code before DS-6.0 closes.

---

### R6 — Sacred-file pressure · **P: medium · I: very high**

`core.rs` is 13,672 lines, the hottest paint path, and it assembles the shell. Any shell
*region* change lands there. Under deadline the temptation to "just add a branch" is real,
and a mechanical sweep there can cost measurable frame rate.

**Mitigation:** `06` §8 rule 1 — **prefer views over shell regions**. A new view inside the
existing `CentralPanel` never touches `core.rs`. Where a shell change is genuinely needed:
single owner, scoped, benchmarked.

**Detection:** `core.rs` in a diff without an explicit mandate in the PR description.

---

### R7 — Frozen-struct workaround · **P: medium · I: high**

New layout state needs somewhere to live, and `Watchlist` is *right there* and already
holds `rail_col_width`, `bottom_dock_height`, panel-open flags. Adding one field is easy
and violates ADR-0001.

**Mitigation:** `state/aggregates.rs` is the destination, and new state must be mirrored in
`push_to_*` / `sync_from_*` or **it will not persist** — a silent data-loss bug.

**Detection:** new `pub` fields on `Watchlist`/`Chart` in a diff.

---

### R8 — Font weights turn out to be a font-loading project · **P: medium · I: medium**

egui selects weight by *font-family registration*, not a numeric weight axis. Shipping
600 vs 700 means registering both faces and mapping numbers to families. The DS specs
author weights per role for all six themes.

**Mitigation:** DS-2.5 is explicitly "scope before committing." Spike it; a documented
deferral is an acceptable outcome.

**Detection:** a weight token that compiles but changes nothing on screen.

---

### R9 — Siblings converge · **P: medium · I: medium**

Alto/Mariner and Lucid/Meridien are designed to be near-identical. Alto vs Mariner differ
by accent, ~10 % density, and bevel temperature — and bevel tint is currently
luminance-derived, removing one of the three. Lucid vs Meridien share a **byte-identical**
palette.

**Mitigation:** Change C (authored bevel tint) and the explicit distinguishability gate in
`04` §3 — *"a reviewer who is not told which is which identifies both from a screenshot."*

**Detection:** the gate. It is deliberately hard to fake.

---

### R10 — Light-theme regressions · **P: medium · I: medium**

Two of six targets are light (Lucid, Meridien), plus four shipped light themes. Hardcoded
black shadows and dark-assumption derivations break them, and dark-theme development does
not surface it.

**Mitigation:** `t.shadow_color` exists precisely for this; `style-mig-lint.sh` check 4
ratchets literal-black down; the DoD requires a Bauhaus walk.

**Detection:** black smudges under cards; invisible hairlines on cream.

---

### R11 — Scope inflation via multi-view discovery · **P: high · I: medium**

Each DS specifies a multi-page app — Aperture 8 tabs, Meridien 9 routes, Cadence a full
screen inventory. Reading those specs invites building all of them.

**Mitigation:** `06` §6 sequences the work so the trading views ship first and the
dashboard is additive and cuttable. Open question 3 in `06` §9 forces an explicit decision.

**Detection:** tickets appearing for `/research`, `/risk`, `/port` before any theme reaches
85 %.

---

### R12 — Ratchet erosion · **P: low · I: medium**

Three ratchets guard past cleanups. Under pressure the cheap move is to raise a baseline.
`docs/migration/README.md` is explicit: *"The ceiling can only go DOWN."*

**Mitigation:** DoD rejects a raised ratchet. `&THEMES[0]` is a hard ban at 0 — the first
new occurrence fails.

**Detection:** `baselines.toml` in a diff with a number going up.

---

### R13 — Doc drift compounding · **P: medium · I: low**

`docs/` is ~19,400 lines and `docs/styling/INDEX.md` already cites a file that does not
exist (`ui_kit/theme.rs`) and a theme count that is ~14 short. This package adds ~3,000
more lines that will drift in turn.

**Mitigation:** the drift table in `00-START-HERE.md`; DS-7.4 requires correcting or
superseding `INDEX.md`; every claim in `01` is marked `[verified]` or `[per doc]` so
readers know what to re-grep.

---

## 2. Glossary

| Term | Meaning |
|---|---|
| **Archetype** | A layout family (mosaic / dense / editorial-dashboard / trading-shell). Not a token. |
| **`.apextheme`** | Zip bundle: `manifest.json` + `colorscheme.json` + `stylesystem.json` in DTCG format. `CURRENT_SCHEMA_VERSION = 1`. |
| **Authored vs derived** | Authored = the value is stored. Derived = computed from another (`elevate()`, luminance). The programme's central theme. |
| **Bevel** | Inset highlight/shadow pair simulating a raised or sunken face. `Treatments.surface_bevel`. |
| **`ColorScheme`** | The palette axis. |
| **`ComponentTheme`** | The trait `ui_kit` widgets take instead of a concrete theme. `ui_kit/widgets/theme.rs:17`. |
| **`Chrome`** | 43 geometry/finish knobs on `StyleSystem`. |
| **DTCG** | Design Tokens Community Group JSON format. |
| **`elevate()`** | Additive luminance shift replacing gamma multiply. Achromatic. `ui_kit/style.rs:462`. |
| **Frozen** | `Watchlist` / `Chart` — no new fields (ADR-0001). |
| **`GroupEnclosure` pattern** | New treatment = new enum variant + render-site `Sx` recipe, no schema change. The canonical extension pattern. |
| **Ratchet** | A count-based lint whose ceiling only descends. |
| **Recipe / `RecipeSet`** | Per-component token overrides layered on the two axes. |
| **Resolver** | Joins `ColorScheme` × `StyleSystem` at render time. |
| **Sacred** | `chart/renderer/render/pane/core.rs` — no sweeps, single owner. |
| **`ShellProfile`** | Stream S6 draft: nav shape / dock / rail side. Adjacent to, not the same as, archetype. |
| **Split-brain** | Two token layers disagreeing on one value. See `radius_pill`. |
| **`Sx`** | Composable style recipes, `ui_kit/sx/`. |
| **`StyleSystem`** | The dimension axis: 10 sub-structs. |
| **`TokenSnapshot`** | Thread-local per-frame token bundle read by `ui_kit/style.rs`. |
| **`Treatments`** | 27 behavioural personality flags on `StyleSystem`. |

---

## 3. FAQ

**Why does the app look flat compared to the mockups even though the colours are right?**
Three compounding reasons: surface ramps are derived and achromatic (Change A); card
shadows are single-layer with no inset, so no bevel stack (Change E); and bevel tint is
luminance-derived, so warm/cool is unavailable (Change C).

**Why do two identical-looking buttons have different corner radii?**
The `radius_pill` split-brain — one comes from `foundation::shell` (preset-aware), the
other from `ui_kit::style::radius_pill()` (fixed 999.0). `05` §6.

**Why does my new token do nothing?**
In order of likelihood: (1) not in `TokenSnapshot`, or the host never pushes one (R4);
(2) not wired through `dt_*!`, so the F12 editor cannot see it; (3) call sites use
hand-passed values rather than the cascade; (4) you are running a stale binary (R3).

**Can I just add a field to `Watchlist`? It's right there.**
No — ADR-0001. And it will not persist unless mirrored in `push_to_*`/`sync_from_*`.

**Do I need to rebuild the shell for the editorial themes?**
No. `06` §1: add a **view** inside the existing workspace system. Only shell *region*
changes touch `core.rs`.

**Which theme should I do first?**
Alto or Mariner — they need no new layout, so they test the whole token contract with
nothing else confounding it. `06` §6.

**Lucid and Meridien have the same palette. Is that a bug?**
No, it is the design. They differ by type scale, radii, spacing, label treatment (mono +
uppercase) and control radius (5px vs 0px).

**Is the React port authoritative?**
No. Rank order: original apps → `design-systems/*.md` → `global.css` → the React port. When
they disagree, the original wins and `global.css` gets a correction commit.

**How do I know when a theme is done?**
`04-DEFINITION-OF-DONE.md` §3. The hardest gate is sibling distinguishability: a reviewer
who is not told which is which must identify both from a screenshot.

**What if a design-system spec contradicts the original app?**
The original app wins. File a correction against the spec.

**How much of this is actually new construction?**
Less than it first appears. Two themes are token-only. Two need only their trading view
skinned. Aperture's mosaic and the editorial dashboard are the only genuine greenfield, and
the dashboard is additive and cuttable.
