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
    print("strip_test_hits selftest: PASS "
          "(test module, bare #[test] fn, and production code after both)")
    return 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(_selftest())
    main()
