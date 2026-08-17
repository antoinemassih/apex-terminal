#!/usr/bin/env python3
"""Drop `file:line:...` hits that fall inside a `#[cfg(test)]` module.

Reads grep-style lines on stdin, writes the survivors to stdout.

Replaces an awk filter in `check-design-system.sh` that cut each file at its
FIRST line matching `^\\s*#\\[cfg(test)\\]` and discarded everything below.

That heuristic is stated in its own comment: "relies on the Rust convention
that test modules sit at the END of a file." The convention does not always
hold, and when it does not the filter fails in the dangerous direction —
production code below a mid-file test module stops being counted at all. It hid
17 real violations, 11 of them in `chart/renderer/ui/style.rs`, which has a test
module above `style_system_to_style_settings`. The same assumption, in the same
file, previously made `token_consumer_gate.py` invent 37 findings by throwing
away the adapter that reads most of the StyleSystem — so this is the second
defect traceable to it.

The failure is also unbounded rather than one-off: any file that later gains a
mid-file test module silently drops out of the count below that point, and the
ratchet reports an improvement for it.

Brace matching costs a few lines and removes the assumption. Test fixtures are
still excluded, and for the right reason: a fake theme must state literal
colours (`Color32::from_rgb(1, 2, 3)` as a sentinel), and "fixing" that by
reaching for a token would make the test depend on the very system it exercises.
"""
import io
import os
import re
import sys

TEST_MOD_RE = re.compile(
    r"#\[cfg\([^\n]*\btest\b[^\n]*\)\]\s*(?:pub\s+)?mod\s+\w+\s*\{"
)

# A `#[test]` (or `#[cfg(test)]`) function that is NOT inside a test module.
# `design_system/equivalence_tests.rs` is declared `pub mod equivalence_tests;`
# with no cfg gate and marks each function individually, so a module-only scan
# reports its fixtures as production drift. `#[test]` already means "compiled
# under test", which is the only property that matters here.
TEST_FN_RE = re.compile(
    r"#\[(?:test|cfg\([^\n]*\btest\b[^\n]*\))\]\s*"
    r"(?:#\[[^\]]*\]\s*)*"          # further attributes: #[ignore], #[should_panic]
    r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+\w+[^{;]*\{"
)


MSYS_PATH_RE = re.compile(r"^/([a-zA-Z])/")


def _openable(path):
    """Translate a git-bash MSYS path (`/c/Users/...`) to a Windows one.

    The caller is a bash script, so grep emits `/c/Users/...`; the interpreter
    is native Windows Python, which cannot open that. Without this the open
    fails, `test_line_ranges` returns empty, and EVERY hit is treated as
    production code — the filter silently becomes a no-op and the ratchet
    counts test fixtures again. It failed exactly that way once, and quietly:
    the only symptom was the baseline rising by precisely the number of test
    literals in three files.
    """
    m = MSYS_PATH_RE.match(path)
    return f"{m.group(1).upper()}:/{path[3:]}" if m else path


def test_line_ranges(path):
    """1-indexed line numbers covered by `#[cfg(test)] mod .. { .. }` bodies."""
    try:
        with open(_openable(path), encoding="utf-8", errors="ignore") as fh:
            text = fh.read()
    except OSError:
        return []

    def brace_spans(rx):
        """Char spans of each `rx` match plus its brace-matched body."""
        out, i = [], 0
        while True:
            m = rx.search(text, i)
            if not m:
                return out
            depth, j = 1, m.end()
            while depth and j < len(text):
                c = text[j]
                if c == "{":
                    depth += 1
                elif c == "}":
                    depth -= 1
                j += 1
            out.append((m.start(), j))
            i = j

    char_spans = brace_spans(TEST_MOD_RE) + brace_spans(TEST_FN_RE)
    return [
        (text.count("\n", 0, a) + 1, text.count("\n", 0, b) + 1)
        for a, b in char_spans
    ]


# A whole FILE that only exists under `cfg(test)`.
#
# `paint_probe.rs` is declared `#[cfg(test)] pub mod paint_probe;` — every line
# in it is test code, but nothing INSIDE it says so, so a per-file scan reads it
# as production. It was flagged for four `Color32::PLACEHOLDER` sentinels, which
# is precisely the case the cfg(test) exclusion exists for: a measuring harness
# must state a literal colour, and "fixing" that by reaching for a token would
# make the harness depend on the system it is used to test.
#
# The documented escape was an `ALLOWED_BASENAMES` entry. A hard-coded basename
# list is the thing that rots: the next test-only module gets flagged, someone
# appends another name, and the list stops describing a rule. Reading the module
# DECLARATION generalises, because that is where the truth already lives.
MOD_DECL_RE = re.compile(
    r"#\[cfg\(test\)\]\s*(?:\n\s*(?:///[^\n]*\n\s*)*)?"
    r"(?:pub(?:\([a-z:]+\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*;"
)


def _test_only_modules(mod_rs):
    """Module names declared `#[cfg(test)] mod NAME;` in `mod_rs`."""
    try:
        with io.open(mod_rs, encoding="utf-8", errors="ignore") as fh:
            return set(MOD_DECL_RE.findall(fh.read()))
    except OSError:
        return set()


_TEST_ONLY_CACHE = {}


def is_test_only_file(path):
    """True when `path` is a module its parent declares under `cfg(test)`.

    Goes through `_openable` for the same reason `test_line_ranges` does: the
    caller is a bash script, so grep emits `/c/Users/...` and native Windows
    Python cannot open it. Without the translation this returns False for
    everything and the check silently becomes a no-op — exactly how the
    line-range filter failed once before, whose only symptom was a baseline
    rising by the number of test literals in three files.
    """
    path = _openable(path)
    key = os.path.abspath(path)
    if key in _TEST_ONLY_CACHE:
        return _TEST_ONLY_CACHE[key]
    stem = os.path.splitext(os.path.basename(path))[0]
    parent = os.path.join(os.path.dirname(path), "mod.rs")
    result = stem in _test_only_modules(parent)
    _TEST_ONLY_CACHE[key] = result
    return result


def main():
    cache = {}
    out = sys.stdout
    for raw in sys.stdin:
        line = raw.rstrip("\n")
        # grep -n output is `path:line:text`; on Windows the path carries a
        # drive letter, so split from the LEFT only twice and validate.
        parts = line.split(":")
        # find the first field that parses as a line number, allowing `C:\...`
        idx = None
        for k in range(1, min(len(parts), 4)):
            if parts[k].isdigit():
                idx = k
                break
        if idx is None:
            out.write(raw)
            continue
        path = ":".join(parts[:idx])
        try:
            lineno = int(parts[idx])
        except ValueError:
            out.write(raw)
            continue

        if is_test_only_file(path):
            continue
        if path not in cache:
            cache[path] = test_line_ranges(path)
        if any(a <= lineno <= b for a, b in cache[path]):
            continue
        out.write(raw)


def _selftest():
    """Prove the filter still filters.

    It has failed SILENTLY twice. First it was handed git-bash MSYS paths
    (`/c/Users/...`) that native Windows Python cannot open, so every lookup
    returned "no test ranges" and the filter became a no-op. Then an edit adding
    `#[test]`-function support did not apply, and nothing said so. Both times
    the only symptom was the baseline moving by exactly the number of test
    literals in a few files — which reads like real drift, and is precisely the
    kind of thing that gets baselined away.

    A filter whose failure mode is "quietly stops filtering" has to assert that
    it works. The sample is synthetic rather than a real file and line number,
    because pinning the selftest to `builtin_recipes.rs:1659` would break every
    time that file is edited and teach the next person to delete the check.
    """
    import os
    import tempfile

    sample = """
fn production_one() { let c = Color32::from_rgb(1, 2, 3); }

#[cfg(test)]
mod tests {
    fn helper() { let c = Color32::from_rgb(4, 5, 6); }
}

fn production_two() { let c = Color32::from_rgb(7, 8, 9); }

#[test]
fn a_bare_test_fn() { let c = Color32::from_rgb(10, 11, 12); }

fn production_three() { let c = Color32::from_rgb(13, 14, 15); }
""".lstrip()

    # 1-indexed: 1 prod | 3-6 test mod | 8 prod | 10-11 bare test fn | 13 prod
    want_test = [5, 11]
    want_prod = [1, 8, 13]

    fd, path = tempfile.mkstemp(suffix=".rs")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(sample)
        spans = test_line_ranges(path)
        inside = lambda n: any(a <= n <= b for a, b in spans)

        failures = []
        for n in want_test:
            if not inside(n):
                failures.append(f"line {n} is test code but was kept")
        for n in want_prod:
            if inside(n):
                failures.append(f"line {n} is PRODUCTION but was dropped")
        # the production-after-a-test-module case is the whole point
        if inside(8) or inside(13):
            failures.append("production code after a test module was dropped — "
                            "this is the exact bug the awk filter had")
    finally:
        os.unlink(path)

    if failures:
        print("strip_test_hits selftest FAILED:")
        for f in failures:
            print("   ", f)
        return 1
    # ── test-only MODULE files ───────────────────────────────────────────────
    # A file whose parent declares it `#[cfg(test)] pub mod name;` is entirely
    # test code, and nothing inside it says so. `paint_probe.rs` was flagged for
    # four `Color32::PLACEHOLDER` sentinels for exactly that reason.
    #
    # Asserted because the alternative on offer was an `ALLOWED_BASENAMES`
    # entry, and a hard-coded basename list is what rots: the next test-only
    # module gets flagged, someone appends another name, and the list stops
    # describing a rule.
    d = tempfile.mkdtemp()
    with io.open(os.path.join(d, "mod.rs"), "w", encoding="utf-8") as fh:
        fh.write("pub mod real;\n#[cfg(test)]\npub mod probe;\n")
    for name in ("real", "probe"):
        with io.open(os.path.join(d, name + ".rs"), "w", encoding="utf-8") as fh:
            fh.write("fn f() { let c = Color32::from_rgb(1, 2, 3); }\n")
    _TEST_ONLY_CACHE.clear()
    if not is_test_only_file(os.path.join(d, "probe.rs")):
        failures.append("a `#[cfg(test)] pub mod` file was not recognised as test-only")
    if is_test_only_file(os.path.join(d, "real.rs")):
        failures.append("a plain `pub mod` file was excluded - that would hide real drift")
    # A doc comment between the attribute and the declaration is normal.
    with io.open(os.path.join(d, "mod.rs"), "w", encoding="utf-8") as fh:
        fh.write("#[cfg(test)]\n/// why this is test-only\npub mod probe;\n")
    _TEST_ONLY_CACHE.clear()
    if not is_test_only_file(os.path.join(d, "probe.rs")):
        failures.append("a doc comment between `#[cfg(test)]` and `mod` defeated the match")

    if failures:
        print("strip_test_hits selftest FAILED:")
        for f in failures:
            print("   ", f)
        return 1

    print("strip_test_hits selftest: PASS "
          "(test module, bare #[test] fn, production code after both, "
          "and cfg(test)-only module files)")
    return 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(_selftest())
    main()
