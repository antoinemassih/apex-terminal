# 04 — Definition of Done

Applies to **every** ticket in [`03-WORK-BREAKDOWN.md`](03-WORK-BREAKDOWN.md), in addition
to that ticket's own acceptance criteria.

---

## 1. The one rule

> **No fidelity claim without a screenshot diff.**

The previous port attempt of these exact six design systems stalled, and its own
post-mortem named the cause:

> "Optimistic reporting. Earlier summaries said themes were 'done/working' based on token
> application, not structural comparison to source."

A clean `cargo build` is not evidence of a correct render. Native windows cannot be judged
by reading code. **Screenshot or it did not happen.**

---

## 2. Universal checklist — every PR

### Correctness

- [ ] `cargo check` and `cargo test` pass
- [ ] `design_system/equivalence_tests.rs` green
- [ ] **No existing test modified to accommodate a new field** (if one needed changing,
      the change was not additive — stop and reconsider)
- [ ] Feature verified in the **running app**, not just in tests

### Ratchets — none may rise

```bash
./scripts/check-design-system.sh     # raw primitives, literal sizes/colours, radii
bash scripts/style-mig-lint.sh       # &THEMES[0], black shadows, StyleSettings, ui_kit imports
./scripts/sx_ratchet.sh              # Sx recipe adoption
```

- [ ] All three pass
- [ ] If a count went **down**, `baselines.toml` lowered in the same PR (ceilings only
      descend — `docs/migration/README.md`)

### Hard rules (`src-tauri/CLAUDE.md` — binding)

- [ ] No new `&THEMES[0]` — hard ban at baseline 0
- [ ] No new `Color32::from_rgba_unmultiplied(0, 0, 0, …)` shadows
- [ ] No new `pub` fields on `StyleSettings`
- [ ] No new `crate::chart_renderer` imports inside `ui_kit/`
- [ ] Font sizes via `font_*()` / `mono_*()`; spacing via `gap_*()`; strokes via `stroke_*()`
- [ ] Colours from a threaded `&Theme` / `&dyn ComponentTheme`, never raw RGB
- [ ] `ui_kit::Button` (not `egui::Button`), `ui_kit::Tag`/`Badge` (not the deprecated
      chips/pills modules), `ui_kit::Header` (not the free-fn panel/dialog headers)
- [ ] Shadows use the `_themed(t)` variants

### Boundaries

- [ ] **No changes to `chart/renderer/render/pane/core.rs`** — unless this is an
      explicitly-scoped, single-owner, benchmarked shell-structure change
- [ ] **No new fields on `Watchlist` or `Chart`** (ADR-0001). New state on
      `state/aggregates.rs` / `chart/state/ChartState`, mirrored in `push_to_*` /
      `sync_from_*`
- [ ] New mutation goes through `AppCommand`, not direct `&mut`
- [ ] No global input read inside `render_chart_pane` (fans out to every pane)

### Design-system hygiene

- [ ] Any new token is wired through `foundation/design_tokens.rs` (`dt_*!`) **and** appears
      in the F12 editor — *the most-skipped step in the programme*
- [ ] New visual treatments are **enum variant + render-site `Sx` recipe**, not new threaded
      fields (the `GroupEnclosure` pattern)
- [ ] No chrome dimension pinned to a literal a token used to produce — **derive, don't pin**

### Light-theme parity

Two of the six targets are light (Lucid, Meridien), plus four shipped light themes
(Bauhaus, Peach, Ivory, Newsprint).

- [ ] Walked the touched feature in **Bauhaus**
- [ ] Walked it in **Lucid** if the change touches surfaces, shadows or borders
- [ ] No black smudges; no invisible hairlines; no white-on-cream text

---

## 3. Per-theme acceptance gate

A theme is **not done** until every box is ticked. Attach evidence to the PR.

- [ ] **Side-by-side screenshot** vs the original at **1440×900** and **2560×1440**
- [ ] **Surface ramp pixel-sampled** and matching the authored values — sample the actual
      rendered pixels, do not trust the token table (design brief Law 3 is precisely the
      case where the table is right and the render is wrong)
- [ ] **Ink ramp** — all four steps distinguishable and matching
- [ ] **Control radius** correct — Cadence full pill / Meridien pure square are pass-fail
- [ ] **Label treatment** correct — Meridien mono-UPPERCASE is pass-fail
- [ ] **Numerals** correct — Aperture hero numerals sans and negatively tracked
- [ ] **Bevel** present and the right temperature — Alto warm vs Mariner cool
- [ ] **Hover** carries the right hue (accent-tinted vs neutral)
- [ ] **Card treatment** — Aperture paints no stroke; per-theme padding differs
- [ ] **Correct layout archetype** (not a recoloured default)
- [ ] **Type scale** moves between Lucid and Meridien
- [ ] Light-parity walk clean (Lucid, Meridien)
- [ ] Universal checklist above, all green

### Sibling distinguishability — explicit gate

Two pairs are designed to be confusable and must not be:

| Pair | Must differ by | Test |
|---|---|---|
| **Alto / Mariner** | accent (amber vs steel-blue), ~10 % density, bevel temperature (warm vs cool), accent *usage* (ambient glow vs precision markers) | A reviewer who is not told which is which identifies both from a screenshot |
| **Lucid / Meridien** | *identical palette* — type scale, radii, spacing, label font/transform, control radius | Same test, with the palette held constant |

If a reviewer cannot tell them apart, the theme is not done regardless of the checklist.

---

## 4. Fidelity scoring

Use the same scale as the React port so numbers are comparable across the two efforts.

| Score | Meaning |
|---|---|
| **~90 %** | Correct archetype, correct tokens, correct finish. **Target.** |
| ~80 % | Correct archetype and tokens; finish details missing (bevels, tracking, hover hue) |
| ~55 % | Correct archetype, rough execution |
| **10–35 %** | **Recoloured default layout — the failure mode.** Not shippable. |

Report honestly. A theme at 55 % reported as done is worse than a theme at 55 % reported
as 55 %, because it stops the work that would have fixed it.

---

## 5. Review protocol

### Author provides

1. Side-by-side screenshots (reference | current) at both viewports
2. Pixel-sample output for the ramps
3. Ratchet output (all three scripts)
4. A one-line honest fidelity score with the gap named
5. For DS-3: proof a v1 `.apextheme` pack still loads and renders identically

### Reviewer checks

1. **Look at the screenshots first.** Before the diff. If it does not look right, the code
   being clean is irrelevant.
2. Sibling distinguishability, if applicable
3. Boundaries — `core.rs` untouched, no frozen-struct fields
4. Ratchets did not rise
5. New tokens appear in the F12 editor
6. Light-theme evidence
7. The fidelity claim matches the screenshots

### Reviewer rejects when

- Screenshots absent or from one viewport only
- Fidelity claimed from token application rather than visual comparison
- Ratchet raised instead of the underlying violation fixed
- An existing test modified to make a "purely additive" change pass
- New chrome literal pinned rather than derived
- `core.rs` touched without a scoped, single-owner mandate

---

## 6. Escalate rather than guess

Stop and ask when you hit any of these:

- One of the six open questions in `03-WORK-BREAKDOWN.md` blocks you
- A "purely additive" change turns out to require a schema bump
- A design-system value contradicts the original app (**the original wins**, and
  `global.css` gets a correction commit)
- Work appears to require touching `core.rs` or a frozen struct
- A ratchet cannot be satisfied without a genuine regression
- Two design systems become indistinguishable and the spec does not say how to separate them

---

## 7. Programme-level done

The programme is done when:

- [ ] All six themes at ≥85 % with evidence
- [ ] All three layout archetypes implemented and selectable
- [ ] Aperture / Cadence / Alto / Mariner / Lucid / Meridien all authored, `meridien`
      registered, selection model decided and implemented
- [ ] Multi-layer shadows shipped; schema v2 migration proven against a v1 pack
- [ ] Every new token in the F12 editor and in the `.apextheme` round-trip
- [ ] Per-theme regression snapshots in CI
- [ ] All ratchets lowered to their new floors
- [ ] `docs/styling/INDEX.md` corrected or superseded; `docs/DESIGN_SYSTEM.md` updated
- [ ] All five design-brief open questions answered in writing
- [ ] The `ShellProfile` overlap resolved and recorded in both documents
