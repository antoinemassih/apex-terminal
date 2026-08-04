#!/usr/bin/env python3
"""
control_size_lint.py — the gate for CONTROL GEOMETRY.

The design-system ratchets all ask one question: *is this value a token?*
That catches a raw colour or a raw font size. It cannot catch this:

    Button::icon(..).placement(IconPlacement::Toolbar)   // 28px, a token
    Button::new("CLOSED").size(Size::Md)                 // 28px, a token
    KitButton::menu(..)                                  // 22px, ALSO a token

Three tokens, three heights, one toolbar row. Every value legal; the row
visibly ragged. The defect is never in a single expression — it is in the
RELATIONSHIP between siblings, which no vocabulary check can see.

This lint attacks the half of that problem a script can decide: a control
height written as a bare NUMBER. A literal cannot participate in any
relationship at all — it can't follow the type scale, can't follow density,
and can't be reconciled with the control beside it. Every one is a place a
theme has been locked out.

    .min_size(vec2(w, 22.0))   -> flagged; use Size::Sm.height()
    .min_size(vec2(w, h))      -> fine
    .min_size(vec2(w, Size::Md.height()))  -> fine

## Why a ratchet and not a ban

There are 33 of these. Some are genuinely not control heights (a 14x14
checkbox glyph box), so a hard ban would force noise-suppression comments.
The budget may only FALL — the same contract as `radius_lint.py` and
`check-design-system.sh`.

Usage
    python scripts/control_size_lint.py            # check against baseline
    python scripts/control_size_lint.py --update   # record current counts
    python scripts/control_size_lint.py --list     # print every site

Exit 0 = at or below every per-file budget. Exit 1 = a file grew.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SRC = REPO / "src-tauri" / "src"
BASELINE = REPO / "scripts" / ".control-size-baseline.txt"

# `.min_size(vec2(W, H))` / `.min_size(Vec2::new(W, H))` / `egui::vec2`.
# Only the SECOND argument (height) is judged — a literal width is usually a
# real column measure, while a literal height is a control being pinned.
CALL = re.compile(
    r"\.min_size\(\s*(?:egui::)?[Vv]ec2(?:::new)?\(([^()]*?),\s*([^(),]*?)\s*\)\s*\)"
)
NUMERIC = re.compile(r"^\d+(\.\d+)?(_?f\d\d)?$")

# `core.rs` is sacred (src-tauri/CLAUDE.md) and excluded from every sweep.
# The dev overlays are deliberately theme-blind.
EXEMPT_BASENAMES = {
    "core.rs",
    "design_inspector.rs",
    "design_tokens.rs",
    "inspect.rs",
    "bug_anchor.rs",
    "tps_overlay.rs",
}


def scan_file(path: Path) -> list[tuple[int, str]]:
    """Return (line_no, height_expr) for each literal control height."""
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []
    # Fixtures legitimately use literals.
    m = re.search(r"^#\[cfg\(test\)\]", text, re.M)
    if m:
        text = text[: m.start()]

    hits: list[tuple[int, str]] = []
    for mo in CALL.finditer(text):
        h = mo.group(2).strip()
        if NUMERIC.match(h):
            # 0.0 means "no floor on this axis" — that is the idiom for
            # width-only constraints, not a pinned control height.
            if float(re.sub(r"[_]?f\d\d$", "", h)) == 0.0:
                continue
            hits.append((text.count("\n", 0, mo.start()) + 1, h))
    return sorted(hits)


def collect() -> dict[str, list[tuple[int, str]]]:
    out: dict[str, list[tuple[int, str]]] = {}
    for path in SRC.rglob("*.rs"):
        if path.name in EXEMPT_BASENAMES:
            continue
        if "tests" in path.parts or "playground" in path.parts:
            continue
        hits = scan_file(path)
        if hits:
            out[str(path.relative_to(REPO)).replace("\\", "/")] = hits
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--update", action="store_true", help="record current counts")
    ap.add_argument("--list", action="store_true", help="print every site")
    args = ap.parse_args()

    found = collect()
    total = sum(len(v) for v in found.values())

    if args.list:
        for f in sorted(found):
            for line, h in found[f]:
                print(f"{f}:{line}: min_size(.., {h}) -> use Size::*.height()")
        print(f"\n{total} literal control heights in {len(found)} files")
        return 0

    if args.update:
        with BASELINE.open("w", encoding="utf-8", newline="\n") as fh:
            fh.write("# control-size budgets — may only FALL.\n")
            fh.write("# A control height written as a bare number cannot follow the\n")
            fh.write("# type scale, cannot follow density, and cannot be reconciled\n")
            fh.write("# with the control beside it. Use Size::*.height().\n")
            fh.write(f"# total={total} files={len(found)}\n")
            for f in sorted(found):
                fh.write(f"{len(found[f])} {f}\n")
        print(f"control-size baseline updated: {total} sites across {len(found)} files")
        return 0

    if not BASELINE.exists():
        print("No baseline — run: python scripts/control_size_lint.py --update")
        return 2

    budgets: dict[str, int] = {}
    for raw in BASELINE.read_text(encoding="utf-8").splitlines():
        if raw.startswith("#") or not raw.strip():
            continue
        cnt, f = raw.split(None, 1)
        budgets[f.strip()] = int(cnt)

    regressions = []
    for f, hits in sorted(found.items()):
        allowed = budgets.get(f, 0)
        if len(hits) > allowed:
            regressions.append(f"  {f}: {allowed} -> {len(hits)} (+{len(hits) - allowed})")

    base_total = sum(budgets.values())
    if regressions:
        print("CONTROL-SIZE LINT FAILED — new literal control heights:\n")
        print("\n".join(regressions))
        print(
            "\nUse the size ladder instead of a number:\n"
            "  Size::Xs/Sm/Md/Lg/Xl.height()  (themed via Density.control_*)\n"
            "  or a row/rail token: row_height_*(), rail_width_*()\n"
            "A literal here cannot follow the theme and cannot agree with the\n"
            "control next to it — which is how one toolbar ended up with four\n"
            "different heights, every one of them a legal value."
        )
        return 1

    print(f"control-size lint OK — {total} literal control heights (budget {base_total}).")
    if total < base_total:
        print("Improved — run 'python scripts/control_size_lint.py --update' to lock it in.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
