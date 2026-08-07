# Which fidelity source answers which question

`ApexTerminalThemes/` holds three kinds of artefact for each design system. They
do not answer the same questions, and I have now got this wrong three times on a
single feature by reaching for whichever one was closest to hand.

| artefact | path | answers | does NOT answer |
|---|---|---|---|
| **Bespoke app** | `Trading App - <Style>/` | composition — what parts a panel header has, what is numbered, what captions exist | anything about the other five styles |
| **Faithful tokens** | `faithful/<style>/tokens.full.json`, `theme.rs` | token values — colour, radius, spacing, type scale | composition |
| **Normalized harness** | `faithful/<style>/normalized.html` | how tokens look applied to a fixed layout | **composition — it renders identical markup for every style by design** |

## The trap

`normalized.html` is the most inviting of the three: it is a single file, it
renders a whole terminal, and there is one per style so it *looks* like a
per-style reference. It is not. It applies one fixed `.np-*` markup to every
theme so that colour and shape differences are isolated. Its own `HANDOFF.md`
says so:

> `normalized.html` — visual sanity-check only (token-driven; **NOT a pixel
> clone of the bespoke app**)

Because it carries `<span class="num">01</span>` for all six, it "proves" that
all six number their panels. They do not:

| style | numbers panels | evidence in the bespoke app |
|---|---|---|
| meridien | yes | `Panel num="01".."08"` |
| lucid | yes | `num="01".."08"` |
| alto | yes | `num="01".."08"` |
| mariner | yes | `num="01".."08"` |
| **aperture** | **no** | `SectionH` is `title \| sub \| right` — no numeral |
| **cadence** | **no** | `num` appears only in its design-system document |

## A second trap, inside one style

`trading app - meridien/` contains more than one composition. `styles.css` sets
`.panel-head .ttl .num { color: var(--muted) }` while the reference render shows
the numeral clearly terracotta — they belong to different screens in the same
folder.

**When two source files disagree, the render is the arbiter.** I was one edit
away from repainting a correct accent numeral muted on the strength of the
first CSS block I happened to grep.

## Rule

Before treating anything here as authoritative, ask which of the three columns
above the question falls into. Composition questions go to the bespoke app.
Token questions go to `tokens.full.json`. `normalized.html` is for looking at.

## Three findings the harness produced, all wrong

Recorded together because they are one mistake made three times, and the third
would have cost the most work.

| harness says | bespoke apps say |
|---|---|
| all six number their panels | four do; aperture and cadence do not |
| numeral is `--np-accent-ink` at 10px | true for the four that have one |
| panels are **outlined cards** (`.np-panel{border:1px solid}`) | panels are **grid rules** — `border-right` + `border-bottom` only |

The third is the clearest. `trading app - Lucid _ new _/cleanup.css`:

```css
/* Consistent panel rhythm — kill double borders */
.panel { border-top: 0 !important; border-left: 0 !important; }
.workspace > .panel:last-child { border-right: 0 !important; }
```

The bespoke design actively strips two sides to turn boxes into rules, and
drops the trailing edge so the outer frame is not doubled. Meridien's own
`styles.css` does the same by construction (`border-right` + `border-bottom`
on `.panel`, nothing else).

So "outlined panel cards", which sat on the Meridien fidelity plan as an
open item, is **not a gap** — our flush regions with edge hairlines are nearer
the design than outlined cards would have been. Implementing it would have
been a visible regression dressed as fidelity work.
