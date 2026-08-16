# Design-system gates

Twelve checks run on every push (`.github/workflows/design-system-check.yml`).
Each one exists because a specific defect shipped and nothing caught it. This
page is the short version: what each asks, the bug that motivated it, and what
to do when it fires.

**The one rule:** a gate that fires is telling you something the compiler and
the test suite cannot. Regenerating its baseline to get a green tick is the
failure mode every one of these was written to prevent — `check-design-system.sh`
says so in its own header, having been baselined blindly once already.

---

## Why so many

They are not twelve arbitrary rules. They are the answers to one question asked
at every layer a value passes through:

```
StyleSystem field          can a theme author it?          token-consumer L2
  ↓ adapter
StyleSettings / DesignTokens
  ↓ begin_frame
TokenSnapshot              does the cascade carry it?      token-consumer L1, L3
  ↓                        …by the same route as siblings? cascade
  ↓ ui_kit::style accessor is the accessor real?           hardwire
  ↓                        do siblings scale together?     ladder
call site                  is the value on-system?         design-system ratchet
  ↓                        …and only ONE system?           single-system
rendered pixel             does the control do anything?   inspector-slider
```

A value can be perfectly correct at every layer but one, and render wrong. That
is what makes these hard to find by reading code.

---

## The gates

### `token_consumer_gate.py` — three layers, one question
*Can a theme author this and have anything happen?*

1. **Accessors** — every `frame_tokens()`-backed accessor in `ui_kit::style`
   must have a call site. `splitter_width` had none while the widget hardcoded
   `6.0` under a comment defending the literal.
2. **StyleSystem fields** — the layer a `.apextheme` actually contains. A field
   here can be authored, exported, re-imported and round-trip asserted while no
   rendering code reads it. `Treatments.focus_ring` (None/Outline/Glow) was
   honoured nowhere.
3. **`dt_*` fallbacks** — `dt_f32!(radius.sm, 3.0)` against a token default of
   `4.0` means the first nudge of that slider jumps the widget 3.0 → 4.1. The
   control is discontinuous at its own resting value.

**Fires?** Wire a consumer, delete the token, or add it to the allowlist *with a
written reason*. Three allow-listed rungs is a signal; a fourth means the
ladders are defined wider than the app uses, and the fix is to narrow them.

### `hardwire_gate.py` — budget 0
`pub fn icon_md() -> f32 { 18.0 }` is a literal wearing an accessor's clothes.
Twenty-seven of these existed, and every other check passed them: the call-site
lints see a proper accessor and are satisfied, the consumer gate only enumerates
`frame_tokens()`-backed accessors, and a ladder where *every* rung is hardcoded
is perfectly consistent. Icon scale, leading and display type were unauthorable
by any theme while the system reported full compliance.

**Fires?** Back it with a `StyleSystem` field carried through `TokenSnapshot`,
or delete the accessor. Do not add a token nothing reads — that trades a
hardwire for a dead token and the consumer gate will say so.

### `ladder_gate.py` — rungs scale together
Every rung of a ladder must apply the same override multiplier, or none may.
`gap_xs_mid` sat ~240 lines from its siblings and was the only rung without
`spacing_scale_override()`. At Standard (1.0×) it was byte-identical to correct;
at Tight (0.75×) the ladder read 3.0 / 6.0 / 6.0 and the mid rung landed exactly
on the rung above it.

### `cascade_gate.py` — all siblings, or none
Fixing the hardwire gate created its own defect class. A token gets its
`StyleSystem` field, its `TokenSnapshot` field and its accessor — then is
sourced **directly** (`al.whisper`) instead of through the override and
DesignTokens tiers its siblings pass through:

```rust
alpha_scrim:   if let Some(ref ov) = override_style { ov.alphas.scrim }
               else { dt_u8!(alpha.scrim, al.scrim) },   // three tiers
alpha_whisper: al.whisper,                               // one tier
```

The short form compiles, renders byte-identically, exports, re-imports and
round-trip asserts green. What it does not do is respond to its own inspector
slider or the hot-reload file. Eleven fields across three groups had this, all
from the same habit, and **no other check here could see it**: hardwire is
satisfied (the accessor is real), token-consumer is satisfied (`begin_frame`
does read the field), and ladder_gate asks about scale multipliers rather than
cascade tiers.

A group where *nothing* cascades passes — those are legitimately snapshot-only,
and forcing token fields on all of them would be ladder inflation. A group that
is **split** is the defect: someone extended it and did not finish.

### `check-design-system.sh` — the ratchet
Per-file budgets for off-token primitives: colour, type, radius, stroke weight,
**space** and **opacity**. Counts only ever go down.

Two things to know before reading the number:

- Of the layout portion, **~59% is chart painting** (`core.rs`,
  `chart_widgets.rs`, `gpu.rs`) where `.left() + 6.0` is a candle body or a
  gauge tick. That is data geometry, not chrome layout. Driving the total to
  zero would mean mangling the renderer to satisfy a lint.
- The opacity portion was large for a reason that turned out to be mostly
  measurement error. See **AT-150** below — the corrected version is that most
  off-ladder alphas were within ±2 of a rung (imperceptible, so they snapped)
  or were chart painting (candle bodies, wick alpha — data, not chrome tiers).
  Only two values, 160 and 180, were a genuine gap.

### `single_system_gate.py` — one styling system
Censuses eight mechanisms and ratchets them. Legacy ones are ceilings (may
shrink, never grow), canonical ones are **floors** — a migration is not
finished by abandoning the destination. Deleted ones must stay at zero.

Written after `sx::recipes`, a complete second recipe engine whose only consumer
was a settings gallery captioned *"proof the new styling system is wired into
the app"*.

### `inspector_slider_gate.py` — controls that control nothing
A slider is a promise that this number moves that pixel. 59 of the inspector's
123 `tokens.*` sliders had no consumer. That is worse than a missing control: a
missing slider says the system does not support something, a dead one says it
does and that you are holding it wrong.

### The rest
- **`radius_lint.py`** — positional corner-radius literals `rect_filled(r, 4.0, c)`.
- **`control_size_lint.py`** — control heights written as bare numbers; the
  relationship gate, since three siblings can each use a legal token and still
  render three different heights.
- **`recipe_adoption_gate.sh`** — *floors* for recipe-layer adoption. The others
  are ceilings on bad patterns; this one stops the design system going dormant.
- **`sx_ratchet.sh`**, **`style-mig-lint.sh`** — legacy-tone and migration
  baselines.

---

## Adding a token

The chain has five links, and a token that stops at any one of them is
authorable in appearance only. The round-trip test has caught the export gap
three separate times.

1. `StyleSystem` group field, with a serde default.
2. `TokenSnapshot` field + `DEFAULT_TOKEN_SNAPSHOT` entry.
3. `begin_frame` — source it **the way its siblings are sourced**. If they
   cascade, so must it: `if let Some(ref ov) = override_style { ov.<g>.<f> }
   else { dt_f32!(<tok>.<f>, <src>) }`. A bare `ass.<group>.<field>` skips the
   override and DesignTokens tiers and renders identically, which is why this
   step needs its own gate.
4. `ui_kit::style` accessor, applying the override multiplier if its ladder has
   one.
5. `export.rs` **and** `loader.rs` — or it will not survive a theme pack.

Then give it a consumer, or the gate will (correctly) fail.

**Set the default to the literal you are replacing.** An unauthored style then
renders byte-identically — which is right for migration safety, and is also
precisely why these gates exist: nothing moves when a token IS wired, and
nothing breaks when it is not, so "done" and "declared and forgotten" look
identical from outside.

---

## Open decisions

None outstanding. AT-149 (font provenance), AT-150 (alpha ladder) and AT-151
(three shadow systems) are all closed — see `docs/AUDIT_LEDGER.md`.

Recorded for the record, since each needed a decision rather than a fix:

- **AT-149**
- **AT-150** — *closed, and worth recording how the framing was wrong.* The
  pitch was "354 off-ladder alphas, the ladder has holes". Two measurement
  errors inflated it. The ladder-extraction regex read `impl Default for Alphas`
  for literal values, so `whisper` and `hint` — set via `Self::default_hint()`
  function calls — were scored as off-ladder despite being rungs. And the count
  pooled chrome with chart painting, where `color_alpha(base, 160)` is a candle
  body, not a tier. Corrected: 48 chrome sites were within ±2 of a rung
  (~1/255, imperceptible) and simply snapped; 13 sat in the one real gap, where
  the ladder steps by 20 up to `scrim` (140) and then jumps 60 to `solid`;
  `dense` (160) and `near_solid` (180) fill it and continue the existing rhythm.
  Chart-painting alphas are scoped out of the ladder argument, as layout
  geometry already was.
