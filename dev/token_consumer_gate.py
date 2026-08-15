#!/usr/bin/env python3
"""Token-consumer gate — every design token must be READ by something.

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

    print(f"token-consumer gate: PASS ({len(counts)} accessors, all consumed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
