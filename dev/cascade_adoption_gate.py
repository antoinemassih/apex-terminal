#!/usr/bin/env python3
"""Adoption floors for the declarative cascading layer, and a ceiling on what it replaces.

Every other gate here is a CEILING on a bad pattern. Those keep a system from
getting worse; they cannot keep one from being built and then ignored.

`sx::recipes` is the cautionary case: a complete second recipe engine whose only
consumer was a settings gallery captioned "proof the new styling system is wired
into the app". It passed every ceiling in the repo, because a system nobody uses
introduces no violations. It was deleted.

`ui_kit/cascade` is at exactly the same risk and for the same reason — it is
additive by design. With no scope open nothing changes, which is what makes
adoption safe and also what makes abandonment invisible. So:

* **Floors** — `El::` nodes, `cascade::scope`/`resolved` call sites, and
  `Flex::` rows may not fall. A migration is not finished by abandoning the
  destination.
* **Ceiling** — cursor walks (`x += …`) in chrome may not rise. That is the
  thing the element tree replaces, and it is how ~80 of them accumulated: each
  one was locally reasonable.

Chart painting is excluded from both. Candles, wicks and axis ticks are data
geometry, not a component tree; `.left() + 6.0` there is a bar body.
"""
import io
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "src-tauri", "src")
BASELINE = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                        "cascade_adoption_baseline.json")

# Data geometry, not chrome. See the module docstring.
CHART = ("/render/", "chart_widgets", "gpu.rs", "/indicators/")

PATTERNS = {
    "el_nodes":      r"\bEl::(?:row|column|text|spacer|button|slot)\(",
    "cascade_sites": r"cascade::(?:scope|resolved)\(",
    "flex_rows":     r"Flex::(?:row|column)\(\)",
    # A BARE local accumulator. The `(?<![.\w])` guard matters: without it this
    # matched `chart.order_panel.pos.x += delta.x`, which is a panel DRAG and
    # not a layout walk — four of them in `order_entry_panel` alone. Counting
    # those would demand a "migration" of code with nothing to migrate.
    "cursor_walks":  r"(?<![.\w])(?:x|y|cx|cy|cursor|left_cursor|top_cursor)\s*\+=",
}

# Which way each metric is allowed to move.
FLOORS = ("el_nodes", "cascade_sites", "flex_rows")
CEILINGS = ("cursor_walks",)

TEST_MOD = re.compile(r"#\[cfg\(test\)\]")
LINE_COMMENT = re.compile(r"//.*")
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)


def census():
    counts = {k: 0 for k in PATTERNS}
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in {"target", ".git", "playground"}]
        for f in files:
            if not f.endswith(".rs"):
                continue
            path = os.path.join(root, f)
            if any(c in path.replace("\\", "/") for c in CHART):
                continue
            with io.open(path, encoding="utf-8", errors="ignore") as fh:
                text = fh.read()
            # Tests are not adoption, and a test that exercises the tree should
            # not be able to raise a floor it does not really meet.
            m = TEST_MOD.search(text)
            if m:
                text = text[: m.start()]
            # Comments are not code. Migrated call sites routinely document the
            # walk they REPLACED — `panel_sub_section` alone carries four such
            # notes — and counting that prose meant the ceiling reported walks
            # that no longer exist. It is the same mistake as the ratchet
            # counting test fixtures (AT-154): a number nobody can act on.
            text = BLOCK_COMMENT.sub("", LINE_COMMENT.sub("", text))
            for name, rx in PATTERNS.items():
                counts[name] += len(re.findall(rx, text))
    return counts


def main():
    now = census()

    if "--update" in sys.argv:
        with io.open(BASELINE, "w", encoding="utf-8") as fh:
            json.dump(now, fh, indent=2, sort_keys=True)
        print(f"cascade-adoption baseline written: {now}")
        return 0

    if not os.path.exists(BASELINE):
        print("cascade_adoption_gate: no baseline — run with --update once to seed it.")
        return 0

    with io.open(BASELINE, encoding="utf-8") as fh:
        base = json.load(fh)

    failures = []
    for k in FLOORS:
        if now.get(k, 0) < base.get(k, 0):
            failures.append(
                f"   {k}: {base[k]} -> {now.get(k, 0)}  (FLOOR — adoption went backwards)"
            )
    for k in CEILINGS:
        if now.get(k, 0) > base.get(k, 0):
            failures.append(
                f"   {k}: {base[k]} -> {now.get(k, 0)}  (CEILING — a new one was added)"
            )

    if failures:
        print("CASCADE-ADOPTION GATE FAILED\n")
        print("\n".join(failures))
        print(
            "\nFloors exist because a design system does not die by being wrong,\n"
            "it dies by being unused — `sx::recipes` passed every ceiling in this\n"
            "repo right up until it was deleted, because a system nobody calls\n"
            "produces no violations.\n\n"
            "The ceiling on cursor walks is the other half: `x += w + gap` is what\n"
            "the element tree replaces, and ~80 of them accumulated one locally\n"
            "reasonable line at a time.\n\n"
            "Raise the floors after a genuine migration:\n"
            "  python dev/cascade_adoption_gate.py --update"
        )
        return 1

    gained = {k: now[k] - base[k] for k in FLOORS if now[k] > base[k]}
    dropped = {k: base[k] - now[k] for k in CEILINGS if now[k] < base[k]}
    print(
        f"cascade-adoption gate: PASS "
        f"(El {now['el_nodes']}, cascade {now['cascade_sites']}, "
        f"Flex {now['flex_rows']}, cursor walks {now['cursor_walks']})"
    )
    if gained or dropped:
        print(f"Improved — re-baseline to lock in: gained {gained}, walks removed {dropped}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
