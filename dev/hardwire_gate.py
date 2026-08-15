#!/usr/bin/env python3
"""Hardwire gate — a token accessor must be backed by a token.

WHY THIS EXISTS
---------------
`ui_kit::style` is the design system's public surface. Widgets call
`icon_md()`, `line_normal()`, `stroke_extra_thick()` and reasonably assume each
one is a token a theme can author. Twenty-seven of them were not. They looked
like this:

    pub fn icon_md()              -> f32 { 18.0 }
    pub fn line_normal()          -> f32 { 1.4 }
    pub fn stroke_extra_thick()   -> f32 { 2.5 }
    pub fn card_padding_default() -> f32 { 12.0 }

A literal wearing an accessor's clothes. It is the most durable kind of
hardwire because it is invisible to every other check in this repo:

  * `check-design-system.sh` sees a token accessor at the CALL SITE and is
    satisfied — the call site is doing everything right.
  * `token_consumer_gate.py` enumerates `frame_tokens()`-backed accessors and
    asks whether anything reads them. A literal accessor is not in that set at
    all, so it is never even considered.
  * `ladder_gate.py` compares override multipliers across a ladder. A ladder
    where EVERY rung is hardcoded is perfectly consistent, and passes.

So the system could report full compliance while icon scale, leading, and the
display type scale were unauthorable by any theme. Those are not marginal
tokens — they are most of what separates one design from another, which is why
every style read as the same app in different colours.

`stroke_extra_thick` was only caught because it happened to sit in a ladder
whose other rungs DID take a multiplier. Nothing would have caught the six
line-heights, which were uniform.

WHAT IT CHECKS
--------------
Any `pub fn <name>() -> f32|u8` in `ui_kit/style.rs` whose body is a bare
numeric literal (optionally times an override multiplier) is a hardwire. The
count is a CEILING that may only go down; it is currently 0.

Usage:
  python dev/hardwire_gate.py           # gate
  python dev/hardwire_gate.py --show    # list them
"""
import io
import re
import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STYLE_RS = os.path.join(REPO, "src-tauri", "src", "ui_kit", "style.rs")

# The ceiling. Lower it when hardwires are removed; never raise it.
BUDGET = 0

ACCESSOR_RE = re.compile(r"pub fn (\w+)\s*\(\s*\)\s*->\s*(f32|u8)\s*\{\s*([^}]*?)\s*\}")
# a bare number, optionally scaled by an override: `2.0 * spacing_scale_override().scale()`
LITERAL_RE = re.compile(r"^-?[0-9]+(?:\.[0-9]+)?\s*(?:\*\s*\w+\(\)\s*(?:\.\s*scale\(\))?)?$")


def hardwires():
    src = io.open(STYLE_RS, encoding="utf-8").read()
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    src = re.sub(r"//[^\n]*", "", src)
    out = []
    for m in ACCESSOR_RE.finditer(src):
        body = m.group(3).strip()
        if "frame_tokens" in body or "dt_" in body:
            continue
        if LITERAL_RE.match(body):
            out.append((m.group(1), m.group(2), body))
    return sorted(out)


def main():
    found = hardwires()

    if "--show" in sys.argv:
        print(f"{len(found)} hardwired accessor(s):")
        for n, t, b in found:
            print(f"   {n:28s} -> {t:3s}  {{ {b} }}")
        return 0

    if len(found) > BUDGET:
        print(f"HARDWIRE GATE FAILED — {len(found)} accessor(s) return a literal, budget {BUDGET}:\n")
        for n, t, b in found:
            print(f"   pub fn {n}() -> {t} {{ {b} }}")
        print(
            "\nAn accessor in `ui_kit::style` is a promise that a theme can author\n"
            "this value. A literal body breaks that promise while satisfying every\n"
            "call-site lint, because the CALL SITE is doing everything right.\n"
            "\n"
            "Back it with a `StyleSystem` field carried through `TokenSnapshot` and\n"
            "`begin_frame`, or delete the accessor. Do not add a token nothing reads\n"
            "just to clear this gate — that trades a hardwire for a dead token, and\n"
            "`token_consumer_gate.py` will say so."
        )
        return 1

    print(f"hardwire gate: PASS ({len(found)} literal accessors, budget {BUDGET})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
