# Meridien — fidelity against the design source

**Source of truth:** `ApexTerminalThemes/trading app - meridien/`
(`design-system/primitives.css`, `styles.css`, `apex.css`), reference render
`ApexTerminalThemes/meridien-terminal.png`.

**Ours:** theme 21 (`meridien` scheme) × style 0 (`meridien`) — the certified
pairing per `design_system::presets`. Captured with recipes LIVE (they were
frozen at startup until `fe96d5ca`, so no previous screenshot of Meridien ever
showed its authored recipes).

---

## 0. The headline finding

**Meridien has never been an implementation of the Meridien design source.**

`builtin.rs` says so directly, at the radii block:

> `radii + strokes aligned to the LIVE default style (style_defaults(0))`. The
> Phase B source-swap deliberately defined Meridien-the-default as the graduated
> `dt_f32!` scale to preserve the existing look — so the design_system Meridien
> matches it (equivalence test: field-exact).

So Meridien is the app's pre-existing default style wearing the Meridien name.
That is a defensible migration decision — it kept the app looking unchanged
through the swap — but it means "Meridien fidelity" had never been measured
against anything.

**The test situation, stated correctly on the third attempt.** There are TWO
equivalence suites, and I checked the wrong one twice:

| suite | file | radii |
|---|---|---|
| `equivalence_tests::style_axis_equivalence` | `design_system/equivalence_tests.rs` | `noted_differences` — reports, does not fail |
| `s2_equivalence_tests` | `chart/renderer/ui/style.rs` | **strict**, plus a golden snapshot |

First I claimed the change was blocked (I had read a code comment, not a test).
Then I claimed there was no blocker at all, having run only the `design_system`
suite. Both wrong: `s2_equivalence_tests` *does* enforce radii field-exactly,
and the radii commit broke two of its tests — `style_defaults_equivalence_id_0_
meridien` and `styles_0_to_8_token_snapshot`. I shipped that commit without
running the full suite.

Now resolved properly rather than by loosening the check:

- `RADIUS_DIVERGENCES` — a named allow-list of `(style, token, legacy, new)`.
  The three Meridien radii are recorded there with the reason. A *fourth*
  divergence, or a different value for one of these three, still fails. Proven
  by editing one entry and watching the test fail.
- The golden snapshot was updated via its own sanctioned "deliberate design
  edit" path, with a comment naming the source.

The lesson is the cheap one: `cargo test --lib`, not the tests I expect to be
relevant.

---

## 1. Token gaps (measured, source vs ours)

| token | source | ours | delta |
|---|---|---|---|
| `radius-sm` | 3px | 4.0 | +1 |
| `radius-md` | 6px | 6.0 | ✓ |
| `radius-lg` | 14px | 12.0 | −2 |
| `radius-xl` | 22px | — | absent |
| `radius-pill` | 99px | **0.0** | inverted |

`pill: 0.0` is the consequential one. Anything resolving `RadiusTier::Pill`
renders **square** in Meridien — including the `switch`, `badge` and `progress`
recipes, which I authored as Pill on the reasoning that "a track is a capsule by
definition". In Meridien they are not capsules at all.

Whether that is wrong depends on what Meridien actually does, which is worth
stating carefully:

- Radius values actually used across the source CSS: `50%` ×8 (dots/avatars),
  `0` ×4, `3px` ×3, `4px` ×1, `10px` ×1, `99px` ×1.
- The single `99px` is the **scrollbar thumb**, not a UI pill language.
- But the reference render's active nav item ("Trade") is clearly a rounded
  pill — produced by `radius-lg: 14px` on a ~28px control, not by a pill token.

So Meridien is genuinely a **sharp** system, and our `RadiusTier::None`
authoring for buttons/inputs/popovers is right in spirit. The errors are
narrower than "it should be rounded":

1. Its small controls are **3px, not 0** — Meridien is sharp, not razor-sharp.
2. `pill = 0` makes short capsule controls square, where the source would round
   them via `lg`.

---

## 2. Density — CORRECTED, and it is minor

**My first pass on this was wrong, and wrong in the flattering direction.** I
eyeballed a *downscaled* capture against a *native-resolution* reference and
reported ours as "~15px vs ~25px — roughly 60% of the source". Measuring both
at their own scales, by autocorrelation of the ink profile down the watchlist
column:

| | period | logical |
|---|---|---|
| source | 25px (autocorr 0.50) | 25px |
| ours | 45px (autocorr 0.85) | **30px** (÷1.5 DPI) |

Ours is **30 vs 25 — 20% LOOSER, not 40% tighter.** The direction was inverted.

So density is a modest gap, not the headline, and "Meridien reads as a dense
terminal that happens to be beige" was not supported by anything. Tightening
the comfortable row height from 28 → 25 would close it, which is a small
token change rather than the invasive app-wide one I described.

Recording the error rather than quietly deleting it: comparing two screenshots
means normalising for DPI and render width first, every time. The same mistake
would have sent us optimising the wrong axis.

---

## 3. Signature devices we do not implement

- **Numbered section headers.** The source labels every panel `01 WATCHLIST`,
  `02 ORDER BOOK`, `03 POSITIONS`, `04 ORDER TICKET` — accent numeral +
  uppercase mono title. It is the most recognisable thing about the layout. We
  have none of it.
- **Right-aligned panel meta.** Each header carries a muted right-side caption
  (`MEGACAPS`, `LVL 2 · 14 DEEP`, `3 OPEN`, `INTRADAY`). We have counts, not
  captions.
- **Outlined panel cards.** The source's panels are distinct hairline-bordered
  boxes separated by page background. Ours are flush regions.

---

## 4. What already matches

Worth recording so it is not "fixed" by accident:

- Paper/cream background and the warm neutral ramp.
- Sharp panel corners.
- Uppercase mono section labels with letter-spacing.
- Terracotta accent; muted sage/olive for positive, terracotta for negative.
- Mono numerals throughout the data columns.

---

## 5. Recommended order

1. ~~Decide the equivalence test.~~ **Not a blocker — I was wrong.** The test
   records radii in its `noted_differences` bucket, which reports but does not
   fail. Only directly-mapped scalar fields are strict. Verified by making the
   change: `style_axis_equivalence` still passes.
2. **Radii** — DONE. `sm 4→3`, `lg 12→14`, `pill 0→14`. Required recording
   the divergence in `RADIUS_DIVERGENCES` + the golden snapshot (see §0).
> **Superseded in part — see [SOURCES.md](SOURCES.md).** The section below
> concluded that all six styles number their panels, on the strength of
> `faithful/<style>/normalized.html`. That file is a token harness which renders
> identical markup for every style, so it cannot answer that question. The
> bespoke apps say four do (meridien, lucid, alto, mariner) and two do not
> (aperture, cadence). The code and tests now follow the bespoke apps.

### Correction: the reference set, and what numbering actually is

`ApexTerminalThemes/faithful/<style>/normalized.html` is the real fidelity
source — a normalised reference for ALL SIX systems, not just Meridien. I found
it only after building the numbered headers, by grepping for a caption string
that was not in the app JSX I had been reading.

Two things it corrects:

1. **Numbered headers are not a Meridien signature.** All six references carry
   the same `<span class="num">` panel header, every one coloured
   `--np-accent-ink`. I had authored the treatment `true` for Meridien alone and
   written a test — `only_meridien_numbers_its_section_headers` — that actively
   enforced the wrong belief. Now default `true`, with the test naming its
   evidence instead.

2. **The numeral is set below the title**: `.num` 10px against `.ttl` 11.5px.
   I painted it at full title size, which cost real width — see the yield rule
   below.

Nearly a third error: `trading app - meridien/styles.css` sets
`.panel-head .ttl .num { color: var(--muted) }`, and I was about to "fix" our
accent numeral to muted on the strength of it. The reference render shows the
numeral clearly terracotta. That CSS belongs to a different composition in the
same folder. **When source files disagree, the render is the arbiter.**

### The yield rule

On a tabbed header the ordinal is a decoration competing with controls. The
first version took its width unconditionally, on the reasoning that tabs which
no longer fit fall into the existing "»" overflow menu rather than clipping —
so nothing is lost. The Aperture capture showed `SCAN` vanishing from a header
that had been showing it. Overflow is not free; it is a hidden control.

The ordinal now counts how many tabs fit with it and without it, and drops
itself if it costs even one — without consuming an ordinal, so the panels below
do not shift by one for a numeral nobody can see. Aperture therefore shows all
four tabs and no ordinal on that panel, while its rail still reads `01`.
Meridien has room and shows both.

3. **Signature devices** — numbered headers (DONE), panel meta captions,
   outlined panel cards. With density demoted, these ARE the visible gap: they
   are what makes the reference render recognisably Meridien. New components /
   style treatments, not token changes.

   **Numbered headers — how.** `Treatments::numbered_section_labels`, authored
   `true` for Meridien alone and `false` by default, cascading through the same
   adapter as `uppercase_section_labels`. Two things are worth recording:

   - It could not ride on `style_label_case`. That is a string transform, and
     the numeral is accent-coloured while the title is not.
   - It needed no new layout. The header's leading slot — the icon slot — is
     already accent, mono, measured and flex-placed, which is exactly an
     ordinal's requirements. The ordinal takes that slot and replaces the icon
     (the source has no icons in numbered headers, and a glyph plus a numeral
     reads as two competing leading marks).

   The ordinal is frame-scoped, held in egui memory keyed by pass number so it
   self-resets on the first header of each frame. A counter reset by a
   "begin frame" hook would keep counting through any frame that skipped the
   hook, and the numbers would climb until someone noticed a panel reading 47.

   **Found on the way in:** `PanelHeader` painted every title TWICE, 0.5px
   apart, as faux-bold — citing "the same trick painter_pane symbol mode uses",
   which is the code that rendered `SPY` as `SPYY` and was deleted earlier in
   this work. The trick had been copied before it was known to be broken, so
   fixing the original left this copy behind. Every side-panel title has been
   double-drawn since. Now a single draw.
4. **Density** — 30 → 25 logical row pitch. Small, do it last, re-shoot.
