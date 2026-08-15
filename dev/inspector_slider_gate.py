#!/usr/bin/env python3
"""Inspector-slider gate — a slider that moves nothing is a lie to the user.

WHY THIS EXISTS
---------------
The F12 design inspector presents ~120 sliders bound to `DesignTokens` fields.
Dragging one is a promise: this number controls that pixel. For 59 of them the
promise was false — the field had no `dt_*!` consumer anywhere, so the slider
moved, the value saved, and nothing on screen changed.

That is worse than a missing feature. A missing slider tells you the system
does not support something. A dead slider tells you the system supports it and
that you are somehow using it wrong, which is precisely the experience of
"we don't have proper control over styling" — you reach for the control, it
does nothing, and the conclusion is that the framework is at fault.

`stroke.heavy` was the clearest specimen: a live "heavy (2.5)" slider that
nothing read, sitting a few files away from `stroke_extra_thick()`, which
hardcoded that exact same 2.5. The number was authored in one place and
consumed from another, and neither knew about the other.

WHAT IT CHECKS
--------------
Every `drag_*`/`two_axis_*` bound to `tokens.<path>` in `design_inspector.rs`
must have at least one `dt_f32!/dt_u8!/dt_rgba!/...(<path>, ..)` consumer
somewhere in `src-tauri/src`. The count of dead sliders is a CEILING that may
only go down.

Only the `tokens.*` (DesignTokens) root is gated. `dt_*!` expands to
`design_tokens::pick_*`, and neither `pick_*` nor `DESIGN_TOKENS` has any
caller outside those macros — so for this root the consumption path is provably
complete and a zero count is trustworthy. The inspector's other root, `s.*`
(StyleSystem), reaches rendering through several routes (`ass.*` in
`begin_frame`, the `style_system_to_style_settings` adapter, theme-pack import)
and cannot be decided statically without false accusations. It is deliberately
out of scope rather than guessed at.

Usage:
  python dev/inspector_slider_gate.py            # gate
  python dev/inspector_slider_gate.py --show     # list every dead slider
  python dev/inspector_slider_gate.py --update   # re-baseline after wiring some
"""
import collections
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO, "src-tauri", "src")
INSPECTOR = os.path.join(SRC, "foundation", "design_inspector.rs")
BASELINE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "inspector_slider_baseline.json")

LINE_COMMENT_RE = re.compile(r"//[^\n]*")
BLOCK_COMMENT_RE = re.compile(r"/\*.*?\*/", re.S)

SLIDER_RE = re.compile(
    r'(?:drag_f32|drag_u8|drag_i8|drag_usize|two_axis_f32|two_axis_u8)\s*\(\s*ui\s*,\s*'
    r'"([^"]*)"\s*,\s*&mut\s+tokens\.([\w.]+)'
)
CONSUMER_RE = re.compile(r"dt_(?:f32|u8|i8|rgba|usize|bool)!\(\s*([\w.]+)\s*,")


def strip(text):
    return LINE_COMMENT_RE.sub("", BLOCK_COMMENT_RE.sub("", text))


def scan():
    with open(INSPECTOR, encoding="utf-8") as fh:
        sliders = {}
        for m in SLIDER_RE.finditer(strip(fh.read())):
            sliders.setdefault(m.group(2), m.group(1))

    consumed = collections.Counter()
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in {"target", ".git"}]
        for f in files:
            if not f.endswith(".rs"):
                continue
            try:
                with open(os.path.join(root, f), encoding="utf-8") as fh:
                    text = strip(fh.read())
            except (OSError, UnicodeDecodeError):
                continue
            for m in CONSUMER_RE.finditer(text):
                consumed[m.group(1)] += 1

    dead = sorted((p, l) for p, l in sliders.items() if consumed.get(p, 0) == 0)
    return sliders, dead


def main():
    sliders, dead = scan()
    if not sliders:
        print("inspector_slider_gate: found no sliders — the pattern is stale, not the code.")
        return 1

    if "--show" in sys.argv:
        print(f"{len(sliders)} tokens.* sliders, {len(dead)} dead:\n")
        for p, l in dead:
            print(f"   {p:36s} slider labelled {l!r}")
        return 0

    if "--update" in sys.argv:
        with open(BASELINE, "w", encoding="utf-8") as fh:
            json.dump({"dead": [p for p, _ in dead]}, fh, indent=2)
            fh.write("\n")
        print(f"inspector-slider baseline rewritten: {len(dead)} dead sliders")
        return 0

    if not os.path.exists(BASELINE):
        print("inspector_slider_gate: no baseline — run with --update once to seed it.")
        return 1
    with open(BASELINE, encoding="utf-8") as fh:
        base = set(json.load(fh)["dead"])

    now = {p for p, _ in dead}
    new = sorted(now - base)
    fixed = sorted(base - now)

    if new:
        print("INSPECTOR-SLIDER GATE FAILED — new slider(s) that control nothing:\n")
        for p in new:
            print(f"   tokens.{p}   (slider labelled {dict(dead)[p]!r})")
        print(
            "\nA slider is a promise that this number controls that pixel. Wire a\n"
            "`dt_*!(path, fallback)` consumer, or do not offer the control."
        )
        return 1

    print(f"inspector-slider gate: PASS ({len(dead)} dead of {len(sliders)}, ceiling {len(base)})")
    if fixed:
        # "no longer dead" rather than "wired": a slider leaves this list either
        # by gaining a consumer OR by being deleted, and those are very
        # different outcomes. Saying "wired" would let a deletion read as a
        # feature — the same flattery the gate exists to prevent.
        print(f"No longer dead ({len(fixed)}) — wired or removed; re-baseline to lock in:")
        for p in fixed:
            print(f"   {p}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
