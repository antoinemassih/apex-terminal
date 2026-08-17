#!/usr/bin/env python3
"""Check that the design-system docs still describe the code that exists.

`docs/DESIGN_SYSTEM.md` carried a snapshot date of 2026-05-05 and a module
table pointing at `src-tauri/src/chart_renderer/ui/widgets/` — a path that had
since become `ui_kit/widgets/`. Nothing failed. A developer following it would
have gone looking in a directory that no longer existed, and the only way to
notice was to try.

That is the same failure mode as every other one this repo has instruments for:
a thing that reads as true, that nobody re-derives, drifting quietly away from
the code. Docs are not exempt just because they are prose.

What this checks — deliberately only what a grep CAN check, so it stays honest:

* **Paths.** Every `src-tauri/src/...` or `dev/...` path mentioned in the docs
  resolves to a file or directory that exists. Brace expansion (`{flex,grid}`)
  is supported because the layer table uses it.
* **APIs.** Every `Type::method(` named inside a fenced code block resolves to
  a `fn method` on that type's module. This catches a renamed builder — the
  most common way a usage example rots.

What it does NOT check, and cannot: whether the prose is *true*. A doc can
describe the wrong reason for a correct API and pass this gate. It bounds the
rot; it does not eliminate it.
"""
import io
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "src-tauri", "src")

# The docs this gate is responsible for. Deliberately a SHORT list: a gate over
# every markdown file in the repo would fail on historical audit snapshots,
# which are supposed to describe the code as it was.
DOCS = ["docs/DESIGN_SYSTEM.md"]

# ANY backticked thing that looks like a source path: contains a `/` and ends
# in `.rs`, `.py`, `.sh`, `.md`, `.toml`, `.txt` or a directory slash.
#
# The first version anchored on a `src-tauri/src` / `dev` / `docs` prefix, and
# therefore checked NONE of the layer table — which writes `ui_kit/cascade/
# context.rs`, repo-relative to the crate root. It passed a deliberately
# corrupted path on its first mutation test. That is the fourth instrument this
# session to report success on work it was not looking at, so it now matches
# the shape of a path rather than a prefix, and resolves against both roots.
PATH_RE = re.compile(
    r"`([A-Za-z0-9_.{},*-]+(?:/[A-Za-z0-9_.{},*-]+)+/?)`"
)
# Chained builder calls inside an example — `.child_if(`, `.show_with(`.
# `Type::method(` alone missed every one of them, because a fluent API is one
# `El::row()` followed by a dozen method calls, and the dozen are exactly what
# gets renamed.
CHAIN_RE = re.compile(r"\.([a-z_][a-z0-9_]*)\(")
# Methods that belong to egui/std, not to us — a chain routinely ends in one.
NOT_OURS = {
    "unwrap", "unwrap_or", "unwrap_or_default", "unwrap_or_else", "clone",
    "to_string", "into", "is_some", "is_none", "map", "map_or", "iter",
    "collect", "len", "min", "max", "abs", "floor", "ceil", "round", "sum",
    "left", "right", "top", "bottom", "center", "width", "height", "size",
    "expect", "as_ref", "borrow", "borrow_mut", "push", "extend", "first",
    "last", "get", "insert", "contains", "filter", "enumerate", "fold",
}
FENCE_RE = re.compile(r"```(?:rust)?\n(.*?)```", re.S)
# `El::row(`, `cascade::scope(`, `Flex::row(` — a type/module path then a call.
CALL_RE = re.compile(r"\b([A-Z][A-Za-z0-9_]*|cascade|context)::([a-z_][a-z0-9_]*)\(")

# Types whose methods this gate knows how to locate. A type not listed here is
# skipped rather than guessed at — a gate that invents a lookup rule produces
# failures nobody can act on, which is how a gate gets deleted.
TYPE_HOMES = {
    "El": "ui_kit/cascade/element.rs",
    "Inherited": "ui_kit/cascade/context.rs",
    "cascade": "ui_kit/cascade/context.rs",
    "context": "ui_kit/cascade/context.rs",
    "Flex": "ui_kit/layout/flex.rs",
    "Grid": "ui_kit/layout/grid.rs",
    "Item": "ui_kit/layout/flex.rs",
    "KvRow": "ui_kit/widgets/kv_row.rs",
}


def expand_braces(p):
    """`a/{b,c}.rs` -> [`a/b.rs`, `a/c.rs`]. The layer table uses this form."""
    m = re.search(r"\{([^}]*)\}", p)
    if not m:
        return [p]
    out = []
    for opt in m.group(1).split(","):
        out.extend(expand_braces(p[: m.start()] + opt.strip() + p[m.end():]))
    return out


def looks_like_path(p):
    if "*" in p:
        return False  # prose: "one file per family"
    tail = p.rstrip("/").rsplit("/", 1)[-1]
    if p.endswith("/"):
        return True
    return "." in tail and tail.rsplit(".", 1)[-1] in {
        "rs", "py", "sh", "md", "toml", "txt", "yml", "yaml", "json",
    }


def resolve(p):
    """Repo-root-relative or crate-src-relative — the docs use both."""
    p = p.rstrip("/")
    return os.path.exists(os.path.join(ROOT, p)) or os.path.exists(
        os.path.join(SRC, p)
    )


def check_paths(text, doc, problems):
    for m in PATH_RE.finditer(text):
        raw = m.group(1)
        if not looks_like_path(raw):
            continue
        for p in expand_braces(raw):
            if not resolve(p):
                problems.append(f"{doc}: path does not exist -> {p}")


def check_calls(text, doc, problems):
    for fence in FENCE_RE.findall(text):
        # A fenced example that builds an `El` tree: every chained method in it
        # must exist on `El`. This is where a fluent API actually rots — one
        # constructor followed by a dozen builder calls, and the dozen are what
        # get renamed.
        # Per STATEMENT, not per fence. The anti-pattern examples put a ❌ and a
        # ✅ in one block, so a fence-wide scan attributed the ❌ half's
        # `painter.galley(` to `El` and failed on it. A fluent tree is one
        # expression terminated by `;`, so that is the unit.
        el_stmts = [
            st for st in fence.split(";") if re.search(r"\bEl::(?:row|column)\(\)", st)
        ]
        for stmt in el_stmts:
            home = os.path.join(SRC, TYPE_HOMES["El"])
            with io.open(home, encoding="utf-8", errors="ignore") as fh:
                body = fh.read()
            for meth in sorted(set(CHAIN_RE.findall(stmt))):
                if meth in NOT_OURS:
                    continue
                if not re.search(r"\bfn\s+" + re.escape(meth) + r"\b", body):
                    problems.append(
                        f"{doc}: `.{meth}(` is chained on an El tree in an example "
                        f"but there is no `fn {meth}` in {TYPE_HOMES['El']}"
                    )
        for m in CALL_RE.finditer(fence):
            ty, meth = m.group(1), m.group(2)
            home = TYPE_HOMES.get(ty)
            if home is None:
                continue
            path = os.path.join(SRC, home)
            if not os.path.exists(path):
                problems.append(f"{doc}: {ty} is mapped to a missing file -> {home}")
                continue
            with io.open(path, encoding="utf-8", errors="ignore") as fh:
                body = fh.read()
            # DERIVED methods have no `fn`. `Inherited::default()` is the one
            # the docs actually use, and the first version of this gate failed
            # on it — a false positive on its very first run, which is the
            # fastest way to make a gate look like noise. A derive or an
            # `impl Default for` both count as the method existing.
            derived = meth == "default" and (
                re.search(r"#\[derive\([^)]*\bDefault\b[^)]*\)\]", body)
                or re.search(r"impl\s+Default\s+for\b", body)
            )
            if derived:
                continue
            if not re.search(r"\bfn\s+" + re.escape(meth) + r"\b", body):
                problems.append(
                    f"{doc}: `{ty}::{meth}(` is in an example but there is no "
                    f"`fn {meth}` in {home}"
                )


def main():
    problems = []
    for doc in DOCS:
        path = os.path.join(ROOT, doc)
        if not os.path.exists(path):
            problems.append(f"{doc}: listed in this gate but missing from the repo")
            continue
        with io.open(path, encoding="utf-8", errors="ignore") as fh:
            text = fh.read()
        check_paths(text, doc, problems)
        check_calls(text, doc, problems)

    if problems:
        print("DOC ACCURACY GATE FAILED\n")
        for p in problems:
            print("   " + p)
        print(
            "\nThe design-system docs describe code that is not there. Either the\n"
            "doc is stale or a rename skipped it — fix whichever is wrong.\n\n"
            "This gate exists because DESIGN_SYSTEM.md pointed at\n"
            "`chart_renderer/ui/widgets/` for months after it became\n"
            "`ui_kit/widgets/`, and nothing anywhere said so."
        )
        return 1

    print(f"doc-accuracy gate: PASS ({len(DOCS)} doc(s) checked)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
