#!/usr/bin/env python3
"""Token-consumer gate — every design token must be READ by something.

Two layers, one question: can a theme author this and have anything happen?

  1. `ui_kit::style` ACCESSORS backed by `frame_tokens()` (the original check).
  2. `StyleSystem` FIELDS — the thing a `.apextheme` file actually contains.
  3. `dt_*!` FALLBACKS — the value rendered until a token is authored.

Layer 2 was unguarded, and it is the layer a theme author edits. A field there
can be authored, exported, re-imported and round-trip asserted while no
rendering code ever reads it.

WHY THIS EXISTS
---------------
The design-system audit found the same defect over and over: a token is added
to `StyleSystem`, surfaced as an accessor in `ui_kit::style`, given a slider in
the design inspector, marked done in the plan — and then nothing ever reads it.

It is invisible for a specific reason. New token defaults are deliberately set
equal to the literal they replace, so an unauthored style renders
byte-identically. That is the right call for migration safety, but it means:

    nothing moves when a token IS wired, and nothing breaks when it ISN'T.

So "✅ done" and "declared and forgotten" look identical from the outside.
Confirmed instances at the time of writing:

  * `splitter_width`          — 0 consumers; caller hardcoded 6.0 with a comment
                                defending the literal. Default was also 6.0.
  * `row_height_comfortable`  — 0 consumers for ~4 days AFTER its milestone was
                                marked complete; the watchlist rendered 28/34.
  * `rail_narrow/medium/wide` — 0 consumers for a full milestone; the plan's own
                                note admits `Width` "was their only consumer,
                                ignoring them".

A test cannot catch this — the code compiles perfectly. Only a call-site count
can, which is what this does.

WHAT IT CHECKS
--------------
For every `pub fn <name>() -> T { frame_tokens().<field> }` accessor in
`ui_kit/style.rs`, count call sites of `<name>(` across `src-tauri/src`,
excluding the defining file and any `#[cfg(...test...)]` module. Zero is a
failure.

Usage:
  python dev/token_consumer_gate.py            # gate; exit 1 if any token is unread
  python dev/token_consumer_gate.py --show     # list every token with its count
"""
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO, "src-tauri", "src")
STYLE_RS = os.path.join(SRC, "ui_kit", "style.rs")

EXCLUDE_DIRS = {"target", ".git"}

# `pub fn foo() -> f32 { frame_tokens().bar }` — with or without #[inline].
ACCESSOR_RE = re.compile(
    r"pub\s+fn\s+(\w+)\s*\(\s*\)\s*->\s*[\w:<>\[\] ]+\s*\{[^}]*frame_tokens\(\)\s*\.\s*(\w+)",
)

# Any cfg attribute naming `test` as a whole word — bare or compound.
TEST_CFG_RE = re.compile(r"#\[cfg\([^\n]*\btest\b")

# Tokens allowed to have zero consumers, each with a reason. Keep this SHORT —
# every entry is a token the design system cannot actually deliver.
# ── Layer 2: StyleSystem fields ────────────────────────────────────────────
#
# Files that DECLARE or SERIALISE the style system. A read in one of these is
# not a consumer: round-tripping a value through JSON proves it survives, not
# that it does anything.
SS_DECL_FILES = {
    "style_system.rs", "export.rs", "loader.rs", "baseline.rs", "builtin.rs",
    "design_inspector.rs", "convert.rs", "model.rs",
}

# Fields with no reader today, each a deliberate carry rather than an oversight.
# The count is a CEILING; adding to it needs a reason written here.
SS_ALLOWED_UNREAD = {
    # ShellSpec: `archetype` is wired; these three are DECLARED FEATURES that
    # were never built. `NavStyle::IconRail` means a vertical strip replacing
    # the top bar — a structural change, not a flag. The live nav appearance
    # comes from `Treatments.nav_buttons_*`, which is a different axis (label
    # rendering), so these are not duplicates of a working mechanism; they are
    # a roadmap sitting in data. Kept so the intent survives, listed so nobody
    # believes authoring them does something.
    "nav", "dock", "rail",
    # Elevation: the whole group, and it is NOT a simple oversight — the code
    # argues both sides in one comment block. `chart/renderer/ui/style.rs` says
    #
    #   "These do NOT use design tokens — the gamma values are perceptual
    #    constants, not tweakable style decisions."
    #
    # eight lines above
    #
    #   "Phase B3 promotes these to `StyleSystem.elevation` so a style system
    #    can override the ramp."
    #
    # So the struct exists because of the second sentence and is unread because
    # of the first. Wiring it would contradict a deliberate decision; deleting
    # it would discard a planned one. Left inert and listed HERE rather than
    # resolved by whoever next runs this gate, because the question is a design
    # call — is surface depth a perceptual constant or a style axis? — and not
    # a wiring bug. Whoever answers it should delete this entry either way.
    "elevation", "l1", "l2", "l3",
    # Typography: superseded by the M1 `ui_*` ladder, which is what
    # `begin_frame` reads. These are the pre-M1 names, kept for theme-pack
    # compatibility so older `.apextheme` files still parse.
    "size_md", "size_lg", "mono_sm", "mono_md", "mono_lg",
    # Assorted rungs declared alongside used ones and never called for:
    # a zero radius, a full radius, a fully-opaque alpha, one spacing rung,
    # a segmented-control idle treatment, and numeral tracking.
    "none", "full", "opaque", "gmd", "segmented_filled_idle", "tracking",
    # Spacing.xl / .xxl — superseded by the gap_* ladder. `begin_frame` reads
    # `sp.gap_xl` / `sp.gap_2xl`; these two are the pre-ladder names, kept so
    # older `.apextheme` files still parse. Same situation as the pre-M1
    # typography names above.
    "xl", "xxl",
}

ALLOWED_UNREAD = {
    # The leading ladder is tight/heading/dense/compact/normal/loose. Five of
    # the six are assigned to a `TextStyle` tier in `ui_kit/text_style.rs`;
    # `loose` is not, because no text tier calls for generous leading yet.
    #
    # Kept rather than deleted: it is a coherent rung of a ladder whose other
    # rungs are live, and it is the axis the editorial (Aperture) target needs
    # — deleting it would mean re-adding it to build that style. Listed here
    # rather than given a token consumer nobody asked for, so it reads as
    # deliberately pending instead of forgotten.
    "line_loose",
    # Top rung of the display scale. sm/md/lg have 4/5/3 consumers; xl has none
    # yet, and its own doc names the intended one: "primary focal number
    # (full-width banner widget)" — a widget that does not exist.
    #
    # Kept for the same reason as `line_loose`: a ladder whose other rungs are
    # live, with a documented pending consumer. Note the trap it set — grepping
    # `font_display_xl()` returns one hit, which is that comment. This gate
    # strips comments precisely so documentation cannot satisfy a wiring check,
    # and it was right where the grep was not.
    "font_display_xl",
    # Top rung of the icon ladder (14/16/18/20). xs/sm/md are consumed; lg lost
    # its only caller when the connection dot stopped sizing its HIT TARGET
    # from the icon ladder — a click box is a control, not a glyph, so that use
    # was wrong even though it kept this gate green.
    #
    # Kept rather than deleted, for the same reason as the two above: a ladder
    # with a hole in it is worse than one with an unused top rung, and a theme
    # authoring `icons.lg` should not silently do nothing when the next large
    # glyph arrives. Worth noticing that this is the THIRD allow-listed rung —
    # if a fourth appears, the ladders are being defined wider than the app
    # actually uses, and the right fix is to narrow them rather than keep
    # adding entries here.
    "icon_lg",
}


_CALL_CACHE = {}


def call_re(name):
    """Match `name(` as a FREE function call only.

    A plain `text.count(name + "(")` is not good enough, and getting this wrong
    is the very defect this gate exists to catch. `splitter_width` is both a
    token accessor in `ui_kit::style` AND a builder method on `PaneGrid`, so a
    naive substring count saw the builder's own definition and its call sites
    and reported the token as consumed — while the widget still hardcoded 6.0.

    Requiring that the name is not preceded by `.` (a method call) or a word
    character (a longer identifier) keeps the count honest. A qualified path
    like `style::splitter_width()` still matches, because `::` ends in `:`.
    """
    if name not in _CALL_CACHE:
        _CALL_CACHE[name] = re.compile(r"(?<![.\w])" + re.escape(name) + r"\s*\(")
    return _CALL_CACHE[name]


def iter_rs_files():
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for f in files:
            if f.endswith(".rs"):
                yield os.path.join(root, f)


LINE_COMMENT_RE = re.compile(r"//[^\n]*")
BLOCK_COMMENT_RE = re.compile(r"/\*.*?\*/", re.S)


def strip_comments(text):
    """Remove // and /* */ comments.

    Necessary because this file's own prose says things like
    "`splitter_width()` had zero consumers" — and a counter that reads comments
    would score that mention as a consumer and permanently mask the token it is
    describing. Documentation must never satisfy a wiring check.
    """
    return LINE_COMMENT_RE.sub("", BLOCK_COMMENT_RE.sub("", text))


def production_text(path):
    """File contents with test modules and comments removed."""
    try:
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
    except (OSError, UnicodeDecodeError):
        return ""
    cut = min((m.start() for m in TEST_CFG_RE.finditer(text)), default=-1)
    if cut != -1:
        text = text[:cut]
    return strip_comments(text)


DEF_PREFIX_RE = re.compile(r"fn\s+$")


def count_calls(text, name):
    """Count real call sites of free function `name` in `text`.

    Skips definitions: `pub fn splitter_width(mut self, ..)` is a builder METHOD
    that happens to share the token's name, and counting it made the gate report
    the token as consumed while the widget still hardcoded its value. That is
    precisely the failure mode this gate exists to prevent, so it must not
    commit it itself.
    """
    n = 0
    for m in call_re(name).finditer(text):
        if DEF_PREFIX_RE.search(text[max(0, m.start() - 8):m.start()]):
            continue
        n += 1
    return n


_SS_READ_CACHE = {}


def SS_READ_RE(name):
    """`.field` as a real field access, cached.

    Built here rather than inline because the inline version was written
    through a shell heredoc, which turned `\\b` into a literal backspace and
    `\\.` into an escaped backslash. The regex then matched nothing, every
    StyleSystem field looked unread, and the gate reported 195 false findings —
    including fields wired and pixel-verified minutes earlier. grep showed the
    line as correct, because a backspace is invisible.
    """
    if name not in _SS_READ_CACHE:
        _SS_READ_CACHE[name] = re.compile(r"\.\s*" + re.escape(name) + r"\b")
    return _SS_READ_CACHE[name]


# `dt_f32!(group.field, ..)` etc. A dt_* macro reads DESIGN TOKENS. It can
# never be a read of a StyleSystem field, but its argument looks exactly like
# one — so `Treatments.focus_ring` was scored as consumed on the strength of
# `dt_rgba!(semantic.focus_ring, ..)`, a different token that merely shares a
# leaf name. Stripping these before counting is exact rather than heuristic.
# Strips ONLY the token path — `dt_f32!(stroke.medium, ..)` -> `dt_f32!(..)`.
#
# The first version removed the whole macro call, which was too much: a dt_*
# FALLBACK is frequently a real StyleSystem read (`dt_f32!(stroke.medium,
# ass.strokes.medium)`), and deleting the call deleted that read too. It
# reported `Strokes.medium` as unread seconds after I had wired it — the gate
# contradicting a change I had just made, which is the cheapest possible signal
# that the gate is wrong.
DT_MACRO_PATH_RE = re.compile(
    r"(dt_(?:f32|u8|i8|usize|rgba|bool)!\()\s*[\w.]+\s*,"
)


TEST_MOD_RE = re.compile(r"#\[cfg\([^\n]*\btest\b[^\n]*\)\]\s*(?:pub\s+)?mod\s+\w+\s*\{")


def strip_test_modules(text):
    """Remove `#[cfg(test)] mod .. { .. }` bodies, keeping the rest of the file.

    Brace-matched rather than truncated. Truncating at the first test module
    threw away everything below it, which in `chart/renderer/ui/style.rs` is the
    adapter that reads most of the StyleSystem — and produced 37 confident,
    wrong "inert field" findings. Leaving tests in is the opposite error: a
    round-trip `assert_eq!` then counts as a consumer, and a test proving a
    value SURVIVES serialisation is not evidence that anything reads it.
    """
    out, i = [], 0
    while True:
        m = TEST_MOD_RE.search(text, i)
        if not m:
            out.append(text[i:])
            return "".join(out)
        out.append(text[i:m.start()])
        depth, j = 1, m.end()
        while depth and j < len(text):
            c = text[j]
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
            j += 1
        i = j


SS_STRUCT_RE = re.compile(r"pub struct (\w+)\s*\{(.*?)\n\}", re.S)
SS_FIELD_RE = re.compile(r"pub (\w+)\s*:")


def style_system_fields():
    """Every `pub` field name declared in a StyleSystem struct."""
    path = os.path.join(SRC, "design_system", "style_system.rs")
    try:
        with open(path, encoding="utf-8") as fh:
            src = LINE_COMMENT_RE.sub("", fh.read())
    except OSError:
        return {}
    out = {}
    for m in SS_STRUCT_RE.finditer(src):
        for f in SS_FIELD_RE.finditer(m.group(2)):
            out.setdefault(f.group(1), set()).add(m.group(1))
    return out


def unread_style_system_fields():
    """StyleSystem fields nothing outside declaration/serialisation reads.

    NOTE: this deliberately does NOT truncate files at `#[cfg(test)]`. An
    earlier version did, and `chart/renderer/ui/style.rs` has a test module
    ABOVE `style_system_to_style_settings` — so the adapter that reads most of
    these was cut off and 37 fields were reported inert that are read every
    frame. Over-counting a read is a missed warning; under-counting one is a
    fabricated finding.
    """
    fields = style_system_fields()
    if not fields:
        return {}, {}
    seen = {n: 0 for n in fields}
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for f in files:
            if not f.endswith(".rs") or f in SS_DECL_FILES:
                continue
            try:
                with open(os.path.join(root, f), encoding="utf-8") as fh:
                    text = DT_MACRO_PATH_RE.sub(
                        r"\1", strip_test_modules(strip_comments(fh.read()))
                    )
            except (OSError, UnicodeDecodeError):
                continue
            for name in seen:
                seen[name] += len(re.findall(SS_READ_RE(name), text))
    unread = {n: fields[n] for n, c in seen.items()
              if c == 0 and n not in SS_ALLOWED_UNREAD}
    return fields, unread



# ── Layer 3: dt_* fallbacks vs token defaults ───────────────────────────────
#
# `dt_f32!(radius.sm, 3.0)` renders `3.0` until someone moves the `radius.sm`
# slider, at which point it renders whatever that slider says. If the token's
# DEFAULT is 4.0, the first 0.1 nudge jumps the widget from 3.0 to 4.1 — the
# control has a discontinuity at its own resting position.
#
# It is invisible from either side. Reading the call site, 3.0 looks like a
# considered choice. Reading the token table, 4.0 looks like the value in use.
# Only comparing them shows that the app renders one number and the inspector
# is calibrated to another.
#
# Four instances of this shape turned up in one session — `splitter_width`
# (6 vs 8), `toolbar.height` (36 vs 38), `pane_header.height` (36 vs 28) and
# `radius.sm` (4 vs 3) — three of them found only by tracing an unrelated bug.
# Every NUMERIC dt_* variant, not just f32. The four instances that motivated
# this check were all f32, and gating only f32 would have left the identical
# defect ungated for the 19 `dt_u8!` / `dt_i8!` / `dt_usize!` sites — alphas and
# insets drift exactly the same way sizes do. Checked at the time of writing:
# all 19 agree with their token defaults, which is worth keeping true rather
# than discovering later.
#
# `dt_rgba!` is excluded: its fallback is a colour expression, not a literal
# this can compare.
DT_CALL_RE = re.compile(
    r"dt_(?:f32|u8|i8|usize)!\(\s*([\w.]+)\s*,\s*(-?[0-9]+(?:\.[0-9]+)?)\s*\)"
)
DT_GROUP_RE = re.compile(r"(\w+):\s*(\w+Tokens)\s*\{([^}]*)\}")
DT_FIELD_RE = re.compile(r"(\w+):\s*(-?[0-9]+(?:\.[0-9]+)?)")


def dt_token_defaults():
    """`group.field` -> default value, from the `DesignTokens` Default impl."""
    path = os.path.join(SRC, "foundation", "design_tokens.rs")
    try:
        with open(path, encoding="utf-8") as fh:
            src = LINE_COMMENT_RE.sub("", fh.read())
    except OSError:
        return {}
    out = {}
    for g in DT_GROUP_RE.finditer(src):
        for f in DT_FIELD_RE.finditer(g.group(3)):
            out[f"{g.group(1)}.{f.group(1)}"] = float(f.group(2))
    return out


def dt_fallback_mismatches():
    """Call sites whose literal fallback differs from the token's default."""
    defaults = dt_token_defaults()
    if not defaults:
        return []
    out = []
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for f in files:
            if not f.endswith(".rs"):
                continue
            p = os.path.join(root, f)
            try:
                with open(p, encoding="utf-8") as fh:
                    text = LINE_COMMENT_RE.sub("", fh.read())
            except (OSError, UnicodeDecodeError):
                continue
            for m in DT_CALL_RE.finditer(text):
                path, fb = m.group(1), float(m.group(2))
                if path in defaults and abs(defaults[path] - fb) > 1e-6:
                    out.append((path, defaults[path], fb,
                                os.path.relpath(p, SRC).replace("\\", "/")))
    return sorted(out)


def main():
    show = "--show" in sys.argv

    with open(STYLE_RS, encoding="utf-8") as fh:
        style_src = fh.read()
    accessors = {name: field for name, field in ACCESSOR_RE.findall(style_src)}
    if not accessors:
        print("token_consumer_gate: found no accessors — the pattern is stale, not the code.")
        return 1

    counts = {name: 0 for name in accessors}
    for path in iter_rs_files():
        if os.path.abspath(path) == os.path.abspath(STYLE_RS):
            continue  # the definition site is not a consumer
        text = production_text(path)
        if not text:
            continue
        for name in counts:
            counts[name] += count_calls(text, name)

    unread = sorted(n for n, c in counts.items() if c == 0 and n not in ALLOWED_UNREAD)

    if show:
        for name in sorted(counts, key=lambda n: (counts[n], n)):
            print(f"  {counts[name]:4d}  {name}  (StyleSystem.{accessors[name]})")
        print(f"\n{len(counts)} token accessors, {len(unread)} unread")
        return 0

    if unread:
        print("TOKEN-CONSUMER GATE FAILED — these tokens are authorable but nothing reads them:\n")
        for name in unread:
            print(f"  {name}()  ->  StyleSystem.{accessors[name]}")
        print(
            "\nA theme can author these and the app will silently ignore them.\n"
            "Either wire a consumer, or delete the token. If it is genuinely\n"
            "pending, add it to ALLOWED_UNREAD with a written reason so the next\n"
            "person knows it is deliberate rather than forgotten."
        )
        return 1

    all_fields, ss_unread = unread_style_system_fields()
    if ss_unread:
        print("TOKEN-CONSUMER GATE FAILED — StyleSystem fields nothing reads:\n")
        for name in sorted(ss_unread):
            print(f"  {name}  (in {', '.join(sorted(ss_unread[name]))})")
        print(
            "\nA theme can author these, export them, and round-trip them, and no\n"
            "rendering code will ever look at them. Wire a reader, delete the field,\n"
            "or add it to SS_ALLOWED_UNREAD with a reason."
        )
        return 1

    dt_bad = dt_fallback_mismatches()
    if dt_bad:
        print("TOKEN-CONSUMER GATE FAILED — dt_* fallback disagrees with the "
              "token default:\n")
        for path, default, fb, where in dt_bad:
            print(f"  {path}: token default {default}, call-site fallback {fb}")
            print(f"      {where}")
        print(
            "\nThe app renders the FALLBACK until the token is authored, so these\n"
            "two numbers are the same pixel measured from two places. While they\n"
            "differ, the inspector slider is discontinuous at its resting value:\n"
            "the first nudge jumps to a different baseline. Make them agree, or\n"
            "use the `ui_kit::style` accessor and stop reading the token store\n"
            "directly."
        )
        return 1

    print(f"token-consumer gate: PASS ({len(counts)} accessors, "
          f"{len(all_fields)} StyleSystem fields, "
          f"{len(dt_token_defaults())} dt_* defaults, all consistent)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
