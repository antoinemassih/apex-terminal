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

## 3b. Capture artifact — NOT an app defect (read this before filing a bug)

Every reference capture is 3840×2088, and **3840 = 2 × 1920**: the window spans
two monitors. A vertical seam is baked into the screenshots at x≈1920 — two thin
lines running the full height, plus black blocks at the very top and bottom, and
a stray `55` near the bottom edge.

It reads convincingly like a group divider painting through the "Indicators"
label (which is a real bug this repo has had before, and is what
`Button::intrinsic_width` was written to fix). It is not. It is present in
identical form in captures taken before and after unrelated changes, sits at
exactly the monitor boundary, and crosses chrome that has no shared geometry —
toolbar, account strip, chart body and bottom dock alike.

Do not chase it. If you want captures without it, run the harness on a
single-monitor window.

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

### D-2 — alert-feed text escaped the strip — FIXED (8477da60)

Initially recorded as "Cadence tab strip overlaps". Wrong on both counts: they
are not tabs (they are the alert feed's error pills — `apex_data.rest`,
`crypto_feed`) and it is not Cadence-specific.

The message painter was built from `ui.painter()` rather than from the one
carrying `badge_clip`, and `with_clip_rect` REPLACES the clip rather than
intersecting it — so the strip bounds were silently dropped. A badge animating
out past the strip's left edge kept its pill clipped but shed its label onto the
bare toolbar, escaping the edge fade too.

Repro is animation-state dependent, so this was fixed by reading rather than by
a before/after capture; the two captures on hand have different alert sets and
are not comparable evidence either way.

### D-3 — watchlist header: three widgets overlapping — FIXED (5fef3f63)

Recorded as a clipped `CLOSE` chip. Cropping the pixels at 3× showed it was not
clipping at all but **three widgets painting on the same pixels** — the
selector's caret, the `+` button and the session badge, stacked into ~30px,
reading `▼CLOSED+`.

Both branches of the header row pinned their reservation
(`available_width() - 60.0` / `- 50.0`) for a cluster whose real width is themed
and about 85px. Fixed by measuring with `Button::intrinsic_width`, which shares
`measure_content_w` with the paint path so a measurement cannot drift from the
widget it measures.

Lesson worth keeping: the certification record's first reading of this was
wrong, and only a 3× crop settled it. Diagnose overlap from pixels, not from a
downscaled full-frame screenshot.

### D-4 — dock content had no left gutter — FIXED (7029a1f4)

Real, and not Mariner-specific. The dock's frame is `Frame::NONE`, whose inner
margin is zero, so its content never had a gutter. Tiling styles inset the whole
card via the OUTER margin and mask it; Mariner does not tile, so `NET LIQ` sat
at x=0.

Applied to the tab's content rather than the frame — the frame also wraps the
resize grip and the SplitTabs strip, which span the full width deliberately.

### D-5 — toolbar row kept a frozen 38.0 — FIXED (2780e926)

Not from the capture pass; found by a codebase-wide sweep for the D-1 defect
class. `toolnav_min_height()` exists only because this constant was wrong, yet
the toolbar row one level up kept its own copy and clipped its dropdown buttons
by 5.6px on six styles. Covered by a new `toolbar_fits_controls` invariant.

### D-6 — auto-hide hit test disagreed with the toolbar — FIXED (de535c86)

Consequence of D-5: the auto-hide logic had its own pinned `28.0 / 36.0` notion
of the bar's height, so the toolbar could hide out from under the cursor.
Raising the floor widened that gap, which is what forced the fix.

### D-7 — DOM rung floor and order-row band — FIXED (6de786a2)

Also from the sweep. The DOM rung's floor measured `mono_sm()` while the row
paints `MonoMd`; the order row's qty@price band reserved a pinned 84px against a
right-anchored status label and centred its text on the row rather than on the
band, clipping early and asymmetrically.

## 5. Gates at certification time

All four green:

| Gate | Result |
|---|---|
| `check-design-system.sh` | 603 / baseline 603 |
| `style-mig-lint.sh` | `&THEMES[0]` code-path refs 0 / hard ban 0 |
| `recipe_adoption_gate.sh` | at or above every floor |
| `radius_lint.py` | 91 / budget 91 |

## 6. Verdict

**Certified.** Identity 6/6, sibling distinguishability, and two-axis
independence all pass; D-1 through D-7 are closed. The platform work (M0–M5)
delivered what it promised — themes now change colour, type, geometry,
proportion and component recipes from data.

## 7. The pattern behind every defect on this page

Seven defects, one shape. In each case **two numbers were individually
legitimate and only their RELATIONSHIP was wrong**:

| Defect | Pinned | Themed thing it had to hold |
|---|---|---|
| D-1 | `account_strip_height 36` | `font_hero 36` + 4px margin |
| D-2 | text clip built from the wrong painter | the strip's own bounds |
| D-3 | `available_width() - 60` | badge text + 2 icon hit targets |
| D-4 | `Frame::NONE` (zero inner margin) | any content at all |
| D-5 | `38.0` | `font_lg()*1.35 + 2*gap_sm()` |
| D-6 | `28.0 / 36.0` hover zone | the toolbar's resolved height |
| D-7 | `84.0` reserve; `mono_sm()` floor | status label; `MonoMd` |

**No ratchet could have caught any of them.** Every gate we run asks "is this
value a token?" — and in all seven cases the answer was yes, or the literal was
in a position no lint inspects. The design-system ratchet, style-mig lint,
recipe-adoption floors and radius lint are all *vocabulary* checks. These were
*agreement* failures.

Three rules fall out, in decreasing order of how often they would have helped:

1. **Derive, don't pin.** A chrome dimension that contains type must be computed
   from that type, with the authored value as a FLOOR rather than a fixed size.
2. **One measures, the other asks.** When two places need the same dimension,
   never let both guess — D-5 and D-6 were the same number computed twice.
3. **Measure the thing you actually paint.** D-7's floor was derived, just from
   the wrong tier — being derived is not sufficient if it derives from the wrong
   source.

The invariant tests `strip_fits_hero` and `toolbar_fits_controls` are the first
two gates of a new kind: they assert *relationships across styles* rather than
token vocabulary. Both walk every style, and `strip_fits_hero` additionally
asserts its own derivation is still exercised so it cannot start passing
vacuously. **Adding one such invariant per pinned chrome dimension is the
highest-value follow-up available** — it is the only gate class that could have
caught this page.

## 8. Known-remaining, deliberately not fixed

- `alert_feed.rs` `SLOT_W = 150.0` — a pinned pixel slot holding a label
  truncated by CHARACTER count. That equivalence holds for exactly one font
  metric. It degrades to overrun WITHIN the strip rather than outside it, so it
  is less severe than D-2 and wants its own change.
- `ui_kit/widgets/tokens.rs` `Size::height()` returns frozen `18/22/28/34/40`
  while `Size::font_size()` reads live tokens. Unguarded, and survives only
  because the `ui_*` ladder is currently identical across all nine styles — and
  external theme packs cannot author it (`loader.rs` exposes no `ui_*` key).
  **This is the single largest latent instance of the pattern above**: the day
  that ladder becomes themeable, every `Size`-based height goes stale at once.
- `style_button_height()`, `style_tab_height()`, `Spacing.cta_height` and
  `Chrome.nav_cluster_padding` have zero consumers — themed alternatives exist
  beside ~30 literal-height boxes that never adopted them.
- Cadence renders the dropdown caret as tofu (missing glyph in its proportional
  face) — a font-coverage issue, not layout.
