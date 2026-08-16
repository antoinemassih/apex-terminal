#!/usr/bin/env python3
"""Every sibling in a token group must traverse the SAME cascade.

The cascade has three tiers, and `begin_frame` spells them out per field:

    alpha_scrim: if let Some(ref ov) = override_style { ov.alphas.scrim }
                 else { dt_u8!(alpha.scrim, al.scrim) }

hot-reload override, then DesignTokens (the F12 inspector), then StyleSystem.
A field written as a bare `al.scrim` skips the first two: it is authorable in a
`.apextheme`, exports and re-imports correctly, round-trip asserts green — and
does not move when you drag its inspector slider or edit the hot-reload file.

That is what happened three separate times, always the same way. A token was
added to satisfy the hardwire gate (`pub fn icon_md() -> f32 { 18.0 }`), got its
StyleSystem field, its TokenSnapshot field and its accessor — five of the chain's
links — and was then sourced DIRECTLY because that compiles and renders
identically. `alpha_whisper`/`alpha_hint`, the four `font_display_*` rungs plus
`font_4xs`/`xs_plus`/`md_plus`, and `gap_2xs`: eleven fields, one habit.

No existing check could see it. The hardwire gate is satisfied (the accessor is
real). The token-consumer gate is satisfied (the field IS read — by begin_frame).
The ladder gate asks about override MULTIPLIERS on f32 ladders, not about which
cascade tiers a field passes through. The suite is satisfied because an
unauthored style renders byte-identically either way — which is the whole reason
this class hides.

So the rule is the ladder gate's rule, applied to the cascade instead of the
scale multiplier: **all siblings, or none.** A group where nothing cascades is
consistent and passes — those groups are legitimately snapshot-only, and forcing
token fields on all of them would be ladder inflation. A group where SOME
siblings cascade and others do not is the defect: it means someone extended the
group and did not finish.
"""
import os
import re
import sys
from collections import Counter, defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BEGIN_FRAME = os.path.join(
    ROOT, "src-tauri", "src", "chart", "renderer", "ui", "style.rs"
)

# Groups below this size are not a "ladder" — one or two fields sourced plainly
# is a judgement call, not an inconsistency.
MIN_GROUP = 3

# Prefixes that are NOT a cascade group: the snapshot happens to share a word.
NOT_A_GROUP = {
    # `radius_*` mixes shorthand punning (`radius_sm,`) with one computed field;
    # the shorthand IS the cascade, resolved at the binding above.
    "radius",
}


def field_lines(text):
    """`(field, value)` for each `TokenSnapshot { .. }` initialiser line."""
    fn = re.search(r"fn begin_frame\b.*?\n\}", text, re.S)
    if not fn:
        sys.exit("cascade gate: begin_frame not found — did style.rs move?")
    body = fn.group(0)
    lit = re.search(r"TokenSnapshot\s*\{(.*?)\n    \};", body, re.S)
    if lit:
        body = lit.group(1)
    out = []
    for line in body.splitlines():
        m = re.match(r"\s*(\w+):\s*(.+?),?\s*$", line)
        if m:
            out.append((m.group(1), m.group(2)))
    return out


def main():
    with open(BEGIN_FRAME, encoding="utf-8") as fh:
        text = re.sub(r"//[^\n]*", "", fh.read())

    groups = defaultdict(Counter)
    offenders = defaultdict(list)
    for field, value in field_lines(text):
        prefix = field.split("_")[0]
        if prefix in NOT_A_GROUP:
            continue
        # A bare identifier is shorthand for a local bound just above, which is
        # itself cascaded — not a bypass. Only a field ACCESS is the bypass.
        if re.fullmatch(r"\w+", value):
            continue
        cascaded = "dt_" in value
        groups[prefix]["cascade" if cascaded else "direct"] += 1
        if not cascaded:
            offenders[prefix].append(field)

    bad = []
    for prefix, counts in sorted(groups.items()):
        if sum(counts.values()) < MIN_GROUP:
            continue
        if counts["cascade"] and counts["direct"]:
            bad.append((prefix, counts, offenders[prefix]))

    if bad:
        print("CASCADE GATE FAILED — group(s) whose siblings disagree:\n")
        for prefix, counts, fields in bad:
            print(
                f"   {prefix}_*  —  {counts['cascade']} cascade, "
                f"{counts['direct']} direct"
            )
            for f in fields:
                print(f"        {f}  reads its StyleSystem field directly")
        print(
            "\nThese render correctly and export correctly. What they do NOT do is\n"
            "respond to the inspector or the hot-reload file, because they skip the\n"
            "override and DesignTokens tiers their siblings go through.\n\n"
            "Fix: give each one a DesignTokens field and write it like its\n"
            "siblings —\n"
            "    if let Some(ref ov) = override_style { ov.<group>.<field> }\n"
            "    else { dt_f32!(<tok>.<field>, <src>) }\n\n"
            "Or, if the whole group is deliberately snapshot-only, make it\n"
            "uniform: no sibling cascades. Both answers are fine. A split is not."
        )
        return 1

    total = sum(sum(c.values()) for c in groups.values())
    cascaded = sum(c["cascade"] for c in groups.values())
    print(
        f"cascade gate: PASS ({cascaded}/{total} snapshot fields cascade; "
        f"{len(groups)} groups internally consistent)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
