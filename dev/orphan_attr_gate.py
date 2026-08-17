#!/usr/bin/env python3
"""Catch an attribute that has been separated from the item it applies to.

Rust attaches `#[...]` to the NEXT item, whatever that turns out to be. Delete
the item and leave the attribute, and it silently re-targets — the code still
compiles, and it compiles WRONG.

That happened here, and the way it failed is the reason this gate exists.
`fetch.rs` had:

    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    use crate::chart_renderer::{ChartCommand, Bar};

An unused-import sweep removed the `CommandExt` line — correctly, it was unused
— and left the `#[cfg]` behind. It then applied to the `ChartCommand, Bar`
import instead. On Windows the cfg is TRUE, so the import stayed and everything
built. On Linux the cfg is FALSE, the import vanished, and 175 errors followed.

Local checks passed on all three feature configurations because they all ran on
Windows. Only CI, on Linux, could see it. A defect that is invisible on the
developer's platform and fatal on the build platform is exactly what a cheap
static check is for.

WHAT IT FLAGS
-------------
An attribute ALONE on its line, followed by a blank line, followed by an item.
That blank line is the signal: `#[derive(...)]`, `#[inline]` and friends sit
flush against what they annotate. A gap means something that used to be there
is gone.

WHAT IT DOES NOT FLAG
---------------------
* `#[serde(default)] pub field: T,` — attribute and item on one line.
* An attribute followed by doc comments and then an item; comments between the
  two are normal and the attribute still lands correctly.
* `#[default]` above an enum variant, and every other flush-attached case.

The check is deliberately narrow. A blank line between an attribute and its
item is nearly always an accident, and the cost of the rare deliberate one is a
comment saying so.
"""
import io
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "src-tauri", "src")

ATTR_ALONE = re.compile(r"#!?\[[^\]]*\]")


def offenders():
    out = []
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in {"target", ".git"}]
        for f in files:
            if not f.endswith(".rs"):
                continue
            path = os.path.join(root, f)
            with io.open(path, encoding="utf-8", errors="ignore") as fh:
                lines = fh.read().split("\n")
            for i, line in enumerate(lines):
                s = line.strip()
                if not ATTR_ALONE.fullmatch(s):
                    continue
                # Inner attributes (`#![...]`) apply to the enclosing module, not
                # to a following item, so a blank line after one is meaningless.
                if s.startswith("#!["):
                    continue
                if i + 1 >= len(lines) or lines[i + 1].strip():
                    continue
                j = i + 1
                while j < len(lines) and not lines[j].strip():
                    j += 1
                if j >= len(lines):
                    continue
                target = lines[j].strip()
                # A trailing attribute at the end of a block annotates nothing.
                if target in ("}", "};", ")", "],"):
                    continue
                rel = os.path.relpath(path, ROOT).replace("\\", "/")
                out.append((rel, i + 1, s, target[:70]))
    return out


def main():
    bad = offenders()
    if not bad:
        print("orphan-attribute gate: PASS (no attribute is separated from its item)")
        return 0
    print("ORPHAN-ATTRIBUTE GATE FAILED\n")
    for rel, ln, attr, target in bad:
        print(f"   {rel}:{ln}")
        print(f"       {attr}")
        print(f"       is separated by a blank line from: {target}")
    print(
        "\nRust attaches an attribute to the NEXT item. A blank line between the\n"
        "two usually means the item it belonged to was deleted and the attribute\n"
        "was left behind — where it now silently applies to something else.\n\n"
        "This is not hypothetical: a `#[cfg(target_os = \"windows\")]` orphaned\n"
        "this way re-targeted onto a `use crate::chart_renderer::{ChartCommand,\n"
        "Bar};`, which kept building on Windows and failed on Linux with 175\n"
        "errors. Every local check passed because they all ran on Windows.\n\n"
        "Either delete the attribute or move it flush against its item."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
