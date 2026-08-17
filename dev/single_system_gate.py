#!/usr/bin/env python3
"""Single-styling-system gate — one design system, and no quiet second one.

WHY THIS EXISTS
---------------
This codebase accumulated EIGHT overlapping ways to answer "what colour/size
should this widget be?". None of them was ever declared obsolete, so all of
them kept working, and each new one was added beside the last rather than in
place of it. The census when this gate was written:

    ComponentTheme        432      legacy   widget-local style structs
    PortableTheme         103      legacy   serialisable theme payload
    current()              94      legacy   global StyleSettings singleton
    RecipeSpec             34      CANON    the live recipe layer
    frame_tokens()         29      CANON    per-frame TokenSnapshot
    get_theme()            28      legacy   LIVE_THEMES registry lookup
    active_style_system()   6      CANON    StyleSystem resolution
    sx::recipes             1      DELETED  a second, cva-style recipe layer

THREE OF THOSE LABELS WERE WRONG, and the table above is kept as-written on
purpose — it is the census that PRODUCED the mistake. `ComponentTheme`,
`PortableTheme` and `get_theme()` are canonical, not legacy: the first is the
widget<->theme trait, the second its default concrete impl, the third the
LIVE_THEMES accessor that replaced a compile-time const. They were classified
by counting call sites rather than by reading them, and a high count read as
"legacy sprawl" when it was wide adoption of the ONE contract.

Left as ceilings they would have failed CI for ADOPTING the design system —
the most damaging direction an instrument can be wrong in. See the notes on
each entry in SYSTEMS below for the current, corrected classification; that
block is the authority, not this table.

`sx::recipes` is the clearest illustration of the failure mode. It was a
complete parallel recipe engine — 108 lines, its own combinator API — whose
only consumer was a settings-panel gallery captioned "proof the new styling
system is wired into the app". It proved nothing except its own existence. A
reader could not tell it from the real one without counting call sites.

Tests cannot see this: every one of these systems compiles and renders. Only a
census can. So this gate keeps one.

WHAT IT CHECKS
--------------
  * FORBIDDEN systems must stay at zero. Deleting a parallel system is not
    durable unless something objects when it grows back.
  * LEGACY systems are ceilings that may only ratchet DOWN. They are still
    load-bearing, so this does not demand they vanish today — it demands they
    never grow. Adding the 104th `PortableTheme` site fails.
  * CANONICAL systems are floors. They may only grow. This is what stops a
    migration from being "solved" by abandoning the target too.

Usage:
  python dev/single_system_gate.py            # gate
  python dev/single_system_gate.py --show     # census only, never fails
  python dev/single_system_gate.py --update   # re-baseline after real progress
"""
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO, "src-tauri", "src")
BASELINE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "single_system_baseline.json")

EXCLUDE_DIRS = {"target", ".git"}

# Same comment/test stripping as token_consumer_gate.py, and for the same
# reason: this file's own prose names every system it polices, and a counter
# that reads comments would score documentation as adoption.
LINE_COMMENT_RE = re.compile(r"//[^\n]*")
BLOCK_COMMENT_RE = re.compile(r"/\*.*?\*/", re.S)
TEST_CFG_RE = re.compile(r"#\[cfg\([^\n]*\btest\b")

# system -> (kind, regex). Kinds: forbidden | legacy (ceiling) | canon (floor).
#
# Two things these patterns must get right, both learned by getting them wrong:
#
#  * `current()` is far too common a name to match bare. An unqualified pattern
#    counted `Handle::current()`, `thread::current()` and `Span::current()`
#    (tokio/tracing, nothing to do with styling) and `snapshot::current()`
#    (which is canonical), inflating the legacy ceiling with calls that can
#    never be migrated away. Only the styling paths count.
#
#  * A function is not always called. `unwrap_or_else(active_style_system)`
#    passes `active_style_system` as a VALUE — no parenthesis follows it — so a
#    `name\s*\(` pattern scored zero for the very call site that made this
#    module canonical. Canonical systems match on the identifier, not the call.
SYSTEMS = {
    # DELETED — a second recipe engine beside `RecipeSpec`.
    "sx::recipes":         ("forbidden", r"sx\s*::\s*recipes?\s*::"),

    # LEGACY — not a rival store; the cascade read at the wrong depth.
    #
    # Worth stating precisely, because "parallel system" is the wrong diagnosis
    # and would lead to the wrong fix. `begin_frame()` resolves ONE effective
    # style per frame, in precedence order:
    #
    #     hot-reload override  ->  DesignTokens (F12)  ->  StyleSettings
    #                                     |
    #                                     v
    #                              TokenSnapshot  ->  frame_tokens()
    #
    # `current()` IS the StyleSettings layer at the bottom of that chain —
    # `begin_frame` literally calls it. So it is a source feeding the single
    # resolver, not a second source of truth, and the base values agree.
    #
    # The bug is depth. A widget calling `current()` at PAINT time re-reads the
    # bottom layer directly and so silently skips the two layers above it: live
    # inspector edits and hot-reloaded theme JSON don't move it, while the
    # widget beside it does move. That is the "frozen chrome" class — an
    # element pinned to a value the cascade used to produce.
    #
    # Hence a ceiling, not a ban: using `current()` to BUILD the snapshot is
    # exactly right, and reading it at paint time is what must stop growing.
    "style::current()":    ("legacy",    r"\b(?:style|StyleSettings)\s*::\s*current\s*\(\s*\)"),

    # CANONICAL — the single system's contract, default impl, and read paths.
    #
    # An earlier version of this gate had the next three as `legacy`, which was
    # wrong in the most damaging possible direction: it would have failed CI for
    # ADOPTING the design system. They were classified from a call-site census
    # instead of by reading them. What they actually are:
    #
    #   ComponentTheme  the widget<->theme TRAIT. Widgets take
    #                   `&dyn ComponentTheme` so ui_kit can extract as a crate.
    #                   349 sites is wide adoption of the ONE contract.
    #   PortableTheme   the default concrete impl of that trait — the `T` in
    #                   `T: ComponentTheme = PortableTheme`. Not a rival; the
    #                   serialisable payload the contract is satisfied by.
    #   get_theme()     the LIVE_THEMES store accessor. The deprecated thing
    #                   here is the compile-time `THEMES` const, whose own doc
    #                   says "Do NOT add new runtime call sites against this.
    #                   Use `get_theme(idx)`" — and which is already cfg-gated
    #                   to test/design-mode, so it cannot grow in a release
    #                   build and needs no ceiling.
    "ComponentTheme":      ("canon",     r"\bComponentTheme\b"),
    "PortableTheme":       ("canon",     r"\bPortableTheme\b"),
    "get_theme()":         ("canon",     r"(?<![.\w])get_theme\s*\("),
    "RecipeSpec":          ("canon",     r"\bRecipeSpec\b"),
    "frame_tokens()":      ("canon",     r"(?<![.\w])frame_tokens\b"),
    "active_style_system": ("canon",     r"(?<![.\w])active_style_system\b"),
}


def production_text(path):
    try:
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
    except (OSError, UnicodeDecodeError):
        return ""
    cut = min((m.start() for m in TEST_CFG_RE.finditer(text)), default=-1)
    if cut != -1:
        text = text[:cut]
    return LINE_COMMENT_RE.sub("", BLOCK_COMMENT_RE.sub("", text))


def census():
    counts = {name: 0 for name in SYSTEMS}
    compiled = {n: re.compile(p) for n, (_, p) in SYSTEMS.items()}
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for f in files:
            if not f.endswith(".rs"):
                continue
            text = production_text(os.path.join(root, f))
            if not text:
                continue
            for name, rx in compiled.items():
                counts[name] += len(rx.findall(text))
    return counts


def main():
    counts = census()
    show, update = "--show" in sys.argv, "--update" in sys.argv

    if show:
        for name in sorted(counts, key=lambda n: -counts[n]):
            print(f"  {counts[name]:5d}  {name:22s} {SYSTEMS[name][0]}")
        return 0

    if update:
        with open(BASELINE, "w", encoding="utf-8") as fh:
            json.dump(counts, fh, indent=2, sort_keys=True)
            fh.write("\n")
        print(f"single-system baseline rewritten: {BASELINE}")
        return 0

    if not os.path.exists(BASELINE):
        print("single_system_gate: no baseline — run with --update once to seed it.")
        return 1
    with open(BASELINE, encoding="utf-8") as fh:
        base = json.load(fh)

    errors, wins = [], []
    for name, (kind, _) in SYSTEMS.items():
        now, was = counts[name], base.get(name)
        if was is None:
            errors.append(f"  {name}: not in the baseline — reseed with --update.")
            continue
        if kind == "forbidden" and now > 0:
            errors.append(
                f"  {name}: {now} call site(s). This is a DELETED parallel system;\n"
                f"      it was removed because a second way to do the same thing is\n"
                f"      indistinguishable from the real one. Use the canonical layer."
            )
        elif kind == "legacy" and now > was:
            errors.append(
                f"  {name}: {now} call sites, ceiling {was} (+{now - was}). This system is\n"
                f"      being retired — it may shrink, never grow. Use the canonical layer."
            )
        elif kind == "canon" and now < was:
            errors.append(
                f"  {name}: {now} call sites, floor {was} ({now - was}). This is the layer\n"
                f"      everything is migrating TO; a migration is not finished by\n"
                f"      abandoning the destination."
            )
        elif now != was:
            wins.append(f"  {name}: {was} -> {now}")

    if errors:
        print("SINGLE-SYSTEM GATE FAILED — a parallel styling system grew:\n")
        print("\n".join(errors))
        print(
            "\nThis app must have exactly ONE design/styling system. If the change is\n"
            "genuine progress, re-baseline: python dev/single_system_gate.py --update"
        )
        return 1

    print(f"single-system gate: PASS ({len(SYSTEMS)} systems censused)")
    if wins:
        print("Movement since the baseline (re-baseline to lock it in):")
        print("\n".join(wins))
    return 0


if __name__ == "__main__":
    sys.exit(main())
