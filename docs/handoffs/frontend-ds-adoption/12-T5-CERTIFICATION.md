# 12 — T5 Certification Record

Status of the six design systems against `04-DEFINITION-OF-DONE.md`, run
2026-08-03 after the M5 geometry endgame landed.

Captures: `docs/styling/screenshots/current/*-3840x2088.png`, taken through the
dev_inspector harness (`scripts/ds-harness/capture_app.py --port 7893 --pairs
16:1,17:3,18:4,19:5,20:6,21:0`). 6/6 captured, no crashes, no blank frames.

---

## 1. Identity — PASS 6/6

Every system is recognisable as itself without a label:

| System | Palette | Chrome signature | Verdict |
|---|---|---|---|
| Aperture | warm near-black, orange accent | sentence-case nav, soft radii | PASS |
| Cadence | near-black, green accent | pill controls, sentence-case | PASS |
| Alto | near-black, warm amber/tan | mono values, tight rows | PASS |
| Mariner | near-black, cool steel blue | mono values, precision markers | PASS |
| Lucid | cream paper, red/green data | sentence-case, rounded, airy | PASS |
| Meridien | cream paper, red/green data | MONO · UPPERCASE · SQUARE, airy | PASS |

## 2. Sibling distinguishability — PASS

The DoD's hardest visual gate: two systems that differ mainly in hue must be
unmistakable side by side.

- **Alto vs Mariner** — PASS. Alto's warm amber and Mariner's cool steel read as
  different products, not two skins. This was the T1 pair chosen precisely
  because it was the most likely to collapse into "same app, different tint".

## 3. Two-axis independence — PASS (stronger than the DoD asks)

**Lucid and Meridien ship a byte-identical palette by design.** They are
therefore a natural experiment for whether `StyleSystem` carries real weight
independently of `ColorScheme`.

They render as visibly different products: Lucid is sentence-case, rounded and
proportional; Meridien is mono, UPPERCASE and square. Same colours, different
everything else.

That is the two-axis architecture demonstrated end to end — the dimension axis
is not decorative, it is load-bearing on its own. Nothing in the DoD required
this; it fell out of the capture set and is worth keeping as a permanent
regression pair. **If Lucid and Meridien ever start looking alike, the
StyleSystem axis has gone inert** — a failure mode no colour test can see.

## 4. Defects found

### D-1 — Meridien account strip clipped the NAV hero — FIXED (4dfb792e)

Pre-existing (confirmed identical in the earliest capture, 6fc38a14). Meridien
authored `font_hero: 36.0` into `account_strip_height: 36.0` with 4px of frame
margin — 36px of glyph in a 32px box. Frozen chrome: a dimension pinned to a
value only valid for the type scale it was authored against.

Fixed by deriving — `style::account_strip_height()` treats the authored token as
a FLOOR and raises it to fit the hero. Covered by `strip_fits_hero`, which walks
every style and also asserts the derivation is still exercised, so the guard
cannot start passing vacuously.

### D-2 — Cadence tab strip overlaps — OPEN

Cadence's window tab strip renders `apex_datapex_data.r` — adjacent tab labels
painting over each other. Alto, Mariner, Aperture and Meridien show the same
tabs cleanly separated, so this is style-specific, not a universal layout bug.

Same defect CLASS as D-1 rotated 90°: a pinned horizontal extent versus themed
text metrics. Cadence is the system with the widest proportional nav type, which
is consistent with it being the one to overflow first.

### D-3 — `CLOSE` chip clipped in the watchlist section header — OPEN, all six

The `Core` section header's trailing chip renders clipped in **every** capture
(`CLOSE` losing its first and/or last glyph). Being theme-independent, this is
ordinary layout arithmetic rather than a token-relationship bug — and it is the
only defect visible in all six systems, so it is the highest-frequency one.

### D-4 — Mariner account dock has no left inset — OPEN, needs confirmation

In the Mariner capture (the only one with the bottom ACCOUNT dock expanded)
`NET LIQ` / `$47.9K` sit flush against x=0 with no gutter. Because it is the
only capture in that panel state, this may be a dock-state artifact rather than
a Mariner-specific defect. Re-capture another system with the dock expanded
before treating it as real.

## 5. Gates at certification time

All four green:

| Gate | Result |
|---|---|
| `check-design-system.sh` | 603 / baseline 603 |
| `style-mig-lint.sh` | `&THEMES[0]` code-path refs 0 / hard ban 0 |
| `recipe_adoption_gate.sh` | at or above every floor |
| `radius_lint.py` | 91 / budget 91 |

## 6. Verdict

**Identity, sibling distinguishability and two-axis independence: certified.**
The platform work (M0–M5) delivered what it promised — themes now change
colour, type, geometry, proportion and component recipes from data.

Certification is **not** complete: D-2, D-3 and D-4 are open layout defects. None
of them undermine an identity claim (all six systems are recognisably themselves
today), but D-3 is visible in every system and should close before this is
called done.

Note what the defect list has in common: **D-1 and D-2 are the same bug in two
orientations**, and neither was catchable by any existing gate, because in both
cases the two numbers involved are individually legitimate tokens and only their
RELATIONSHIP is wrong. That is the gap the gate matrix still has.
