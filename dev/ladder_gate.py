#!/usr/bin/env python3
"""Ladder-consistency gate — every rung of a ladder must scale the same way.

WHY THIS EXISTS
---------------
The design system's size tokens come in ladders: gap_2xs..gap_3xl,
radius_xs..radius_pill, stroke_hair..stroke_rule, and so on. Several of them
multiply by a user override — `spacing_scale_override()`, `corner_scale_override
()`, `border_weight_override()` — so the whole ladder breathes together when the
user picks Tight/Loose, Sharp/Round, Hairline/Standard.

If ONE rung forgets the multiplier, nothing looks wrong at the default setting,
because every override's Standard is 1.0x. The rung only breaks away when a user
changes the setting — and then it does not merely look off, it can land on top
of its neighbour and erase a distinction the ladder exists to express.

Two real instances, both invisible to every other check because every rung was
a perfectly legal token:

  * `gap_xs_mid` was declared ~240 lines from the rest of the spacing ladder and
    was the only rung without `spacing_scale_override()`. At Tight (0.75x) the
    ladder read gap_xs 3.0 / gap_xs_mid 6.0 / gap_sm 6.0 — the mid rung landed
    exactly on the rung above it.

  * `stroke_extra_thick()` and `stroke_heavy()` were hardcoded `2.5` and `3.0`
    in a block labelled "pure constants" — not tokens at all, with 8 live call
    sites between them. At Hairline (0.5x) stroke_thick fell to 1.0 while these
    two stayed put, inverting the top of the ladder.

The Rust `ladder_ordering_tests` assert ORDER at runtime for the three ladders
that have an override. This gate is the static complement: it covers every
ladder, including the ones with no ordering test, and it names the specific rung
that disagrees rather than reporting a downstream symptom.

WHAT IT CHECKS
--------------
Groups `pub fn <prefix>_<rung>() -> f32` accessors in `ui_kit/style.rs` by
prefix, and requires every member of a group to apply the SAME override
multiplier (or none at all). Mixed = failure.

Usage:
  python dev/ladder_gate.py           # gate
  python dev/ladder_gate.py --show    # print every ladder and its multiplier
"""
import collections
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STYLE_RS = os.path.join(REPO, "src-tauri", "src", "ui_kit", "style.rs")

ACCESSOR_RE = re.compile(r"pub fn (\w+)\s*\(\s*\)\s*->\s*f32\s*\{([^}]*)\}")
MULT_RE = re.compile(r"\*\s*(\w+)\(\)")
LINE_COMMENT_RE = re.compile(r"//[^\n]*")
BLOCK_COMMENT_RE = re.compile(r"/\*.*?\*/", re.S)

# Prefixes that are not ladders — unrelated accessors that merely share a word.
# Keep this SHORT and justified; a wrong entry here hides a real defect.
NOT_A_LADDER = {
    "font",  # font_2xs..font_xl plus font_section_label/font_caption etc:
             # a mix of ladder rungs and one-off named roles, and none of them
             # take an override multiplier today.
}


def ladders():
    with open(STYLE_RS, encoding="utf-8") as fh:
        src = fh.read()
    src = LINE_COMMENT_RE.sub("", BLOCK_COMMENT_RE.sub("", src))
    groups = collections.defaultdict(list)
    for m in ACCESSOR_RE.finditer(src):
        name, body = m.group(1), m.group(2)
        mult = MULT_RE.findall(body)
        groups[name.split("_")[0]].append((name, mult[0] if mult else None))
    return {p: r for p, r in groups.items() if len(r) >= 2 and p not in NOT_A_LADDER}


def main():
    groups = ladders()
    if not groups:
        print("ladder_gate: found no ladders — the pattern is stale, not the code.")
        return 1

    if "--show" in sys.argv:
        for pre in sorted(groups):
            mults = {m for _, m in groups[pre]}
            tag = next(iter(mults)) if len(mults) == 1 else "MIXED"
            print(f"  {pre}_*  ({len(groups[pre])} rungs)  {tag or 'no multiplier'}")
        return 0

    bad = []
    for pre in sorted(groups):
        rows = groups[pre]
        if len({m for _, m in rows}) > 1:
            bad.append((pre, rows))

    if bad:
        print("LADDER GATE FAILED — a ladder's rungs do not scale together:\n")
        for pre, rows in bad:
            print(f"  {pre}_* :")
            for n, m in rows:
                mark = "  <-- disagrees" if m is None else ""
                print(f"      {n:26s} {m or '(no multiplier)'}{mark}")
        print(
            "\nEvery rung of a ladder must apply the same override multiplier, or\n"
            "none of them may. A rung that forgets it looks correct at the default\n"
            "setting (every override's Standard is 1.0x) and breaks away only once\n"
            "a user changes it — where it can land on top of its neighbour and\n"
            "erase the distinction the ladder exists to express."
        )
        return 1

    print(f"ladder gate: PASS ({len(groups)} ladders, all rungs scale together)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
