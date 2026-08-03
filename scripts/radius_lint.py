#!/usr/bin/env python3
"""
radius_lint.py — M5: the design-system gate's admitted blind spot.

`scripts/check-design-system.sh` says, in its own header:

    NOT matched (deliberate): the positional rounding arg in
    `rect_filled(rect, 4.0, col)`. It was the single biggest radius leak (155
    sites at audit time), but it is indistinguishable by grep from any other
    3-argument call, so a regex gate would be all false positives. That one
    needs a clippy lint or an AST pass — recorded as follow-up rather than
    faked here.

This is that pass. It is not a full Rust parser: it finds the painter calls that
take a corner-radius argument positionally, splits their arguments with a
bracket/string/char-aware scanner (so nested calls, generics, tuples and
`'a'` literals don't fool it), and flags the ones whose radius argument is a
NUMERIC LITERAL rather than a token.

Why that is precise enough: the check is not "does this line contain a float"
(the false-positive trap the shell gate feared) but "is argument N of this
specific call a bare numeric literal". `rect_filled(r, radius_sm(), c)` and
`rect_filled(r, cr, c)` pass; only `rect_filled(r, 4.0, c)` fails.

Usage
    python scripts/radius_lint.py            # check against the baseline
    python scripts/radius_lint.py --update   # record the current counts
    python scripts/radius_lint.py --list     # print every offending site

Exit 0 = at or below every per-file budget. Exit 1 = a file grew.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SRC = REPO / "src-tauri" / "src"
BASELINE = REPO / "scripts" / ".radius-lint-baseline.txt"

# call name -> zero-based index of the corner-radius argument
RADIUS_ARG = {
    "rect_filled": 1,   # rect_filled(rect, RADIUS, color)
    "rect_stroke": 1,   # rect_stroke(rect, RADIUS, stroke, kind)
    "rect":        1,   # rect(rect, RADIUS, fill, stroke, kind)
}

# Files that legitimately use raw primitives: token definitions build the
# tokens everything else consumes, and the two dev overlays are deliberately
# theme-blind (same exemptions the shell gate applies).
EXEMPT_BASENAMES = {
    "style.rs", "tokens.rs", "theme.rs", "theme_impl.rs", "theme_adapter.rs",
    "builtin.rs", "baseline.rs", "design_inspector.rs", "design_tokens.rs",
    "inspect.rs", "scale.rs", "bug_anchor.rs", "tps_overlay.rs",
    "radius_lint.py",
}

NUMERIC = re.compile(r"^\d+(\.\d+)?(_?f\d\d)?$")

# A literal 0.0 means "square", and square is frequently the DESIGN — chart
# bars, volume columns, sparkline bars, dividers and full-bleed bands are
# square in every one of the six systems. Flagging those would reproduce the
# false-positive noise the shell gate feared, from the other direction. They
# are counted and reported, but only NON-ZERO literals are gated: a pinned
# 2.0/3.0/4.0 is a radius the theme cannot move, which is the actual leak.
GATE_ZERO = False


def _as_f32(lit: str) -> float:
    """Numeric value of a Rust float/int literal (`4.0`, `4.0f32`, `4_f32`)."""
    return float(re.sub(r"[_]?f\d\d$", "", lit.strip()))


def split_args(text: str, start: int) -> list[str] | None:
    """Split the argument list of a call whose '(' is at `start`.

    Bracket-, string- and char-literal aware, so `foo(a, bar(b, c), "x,y")`
    yields three arguments and `'('` does not open a group.
    """
    depth = 0
    args: list[str] = []
    cur: list[str] = []
    i = start
    n = len(text)
    while i < n:
        ch = text[i]
        # string literal
        if ch == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    break
                j += 1
            cur.append(text[i : j + 1])
            i = j + 1
            continue
        # char literal (but not a lifetime like 'a)
        if ch == "'" and i + 2 < n and (text[i + 2] == "'" or text[i + 1] == "\\"):
            j = text.find("'", i + 1 + (2 if text[i + 1] == "\\" else 1))
            if j != -1:
                cur.append(text[i : j + 1])
                i = j + 1
                continue
        if ch in "([{<" and not (ch == "<" and depth == 0):
            depth += 1
        elif ch in ")]}>" and not (ch == ">" and depth == 0):
            if ch == ")" and depth == 1:
                args.append("".join(cur).strip())
                return args
            depth -= 1
        if ch == "(" and depth == 1 and not cur:
            i += 1
            continue
        if ch == "," and depth == 1:
            args.append("".join(cur).strip())
            cur = []
            i += 1
            continue
        if depth >= 1:
            cur.append(ch)
        i += 1
    return None  # unbalanced (call spans past our slice)


def scan_file(path: Path) -> list[tuple[int, str, str]]:
    """Return (line_no, call_name, radius_arg) for each positional-literal hit."""
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []

    # Drop the test module: fixtures legitimately use literals.
    m = re.search(r"^#\[cfg\(test\)\]", text, re.M)
    if m:
        text = text[: m.start()]

    hits: list[tuple[int, str, str]] = []
    for name, idx in RADIUS_ARG.items():
        for m in re.finditer(r"\.\s*" + name + r"\s*\(", text):
            open_paren = text.index("(", m.start())
            args = split_args(text, open_paren)
            if not args or len(args) <= idx:
                continue
            arg = args[idx].strip()
            # strip a trailing cast so `4.0 as f32` still counts
            arg = re.sub(r"\s+as\s+\w+$", "", arg).strip()
            if NUMERIC.match(arg):
                line = text.count("\n", 0, m.start()) + 1
                hits.append((line, name, arg))
    if not GATE_ZERO:
        # Square is often the DESIGN, not a leak — drop 0.0 from the gated set.
        hits = [h for h in hits if _as_f32(h[2]) != 0.0]
    return sorted(hits)


def collect() -> dict[str, list[tuple[int, str, str]]]:
    out: dict[str, list[tuple[int, str, str]]] = {}
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
    ap.add_argument("--list", action="store_true", help="print every gated site")
    ap.add_argument("--all", action="store_true",
                    help="include square (0.0) radii, which are excluded from the gate")
    args = ap.parse_args()

    global GATE_ZERO
    if getattr(args, "all", False):
        GATE_ZERO = True
    found = collect()
    total = sum(len(v) for v in found.values())

    if args.list:
        for f in sorted(found):
            for line, name, val in found[f]:
                print(f"{f}:{line}: {name}(.., {val}, ..) -> use radius_sm()/radius_md()/r_pill()")
        print(f"\n{total} positional radius literals in {len(found)} files")
        return 0

    if args.update:
        with BASELINE.open("w", encoding="utf-8", newline="\n") as fh:
            fh.write("# radius-lint per-file budgets — may only FALL.\n")
            fh.write("# The positional corner-radius argument of painter calls\n")
            fh.write("# (rect_filled/rect_stroke/rect) must be a token, not a literal.\n")
            fh.write(f"# total={total} files={len(found)}\n")
            for f in sorted(found):
                fh.write(f"{len(found[f])} {f}\n")
        print(f"radius-lint baseline updated: {total} sites across {len(found)} files")
        return 0

    if not BASELINE.exists():
        print("No baseline — run: python scripts/radius_lint.py --update")
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
        print("RADIUS LINT FAILED — new positional radius literals:\n")
        print("\n".join(regressions))
        print(
            "\nUse the per-style radius tokens instead of a literal:\n"
            "  radius_xs()/radius_sm()/radius_md()/radius_lg(), r_pill(),\n"
            "  or CornerRadius::same(radius_sm() as u8).\n"
            "A literal here is invisible to every theme — it is exactly the leak\n"
            "check-design-system.sh documents that it cannot see."
        )
        return 1

    print(f"radius-lint OK — {total} positional radius literals (budget {base_total}).")
    if total < base_total:
        print("Improved — run 'python scripts/radius_lint.py --update' to lock it in.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
