#!/usr/bin/env python3
"""Quality-gate ratchets (audit B2).

Fail-on-increase gates that stop the codebase from regressing while the WS-C..G
remediation runs. Mirrors the repo's existing baseline-ratchet idiom
(scripts/check-design-system.sh, scripts/sx_ratchet.sh) but consolidated and
data-driven so new dimensions are one line in the baseline.

Dimensions (all counted over src-tauri/src, *.rs):
  unwrap_by_dir     — `.unwrap()` count per top-level area. The render hot path
                      is capped hard (a panic there kills the window); other
                      areas ratchet down only.
  expect_total      — `.expect(` count (single global budget).
  dead_code_allows  — `#[allow(dead_code ...)]` sites (parallel/unused surface).
  file_loc_over     — files exceeding the soft ceiling; the two god-files
                      (core.rs, gpu.rs) are grandfathered with tracked tickets.
  ui_direct_mutation — direct `watchlist.*=` / `chart.*=` assignments in ui/
                      (command-bus bypass; baseline for the WS-E ratchet).

Usage:
  python dev/quality_gate.py            # check against baseline; exit 1 on regression
  python dev/quality_gate.py --update   # rewrite baseline to current (after an intended reduction)
  python dev/quality_gate.py --show     # print current counts, no gating

Baseline lives in dev/quality_baseline.json (committed). When a count drops, the
gate PASSES and prints a nudge to run --update to lock the gain in.
"""
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO, "src-tauri", "src")
BASELINE_PATH = os.path.join(REPO, "dev", "quality_baseline.json")

# Files/dirs excluded from ALL counts (generated / vendored / not first-party).
EXCLUDE_DIRS = {"target", ".git"}

# Hard ceilings that are policy, not ratchets (fail if exceeded regardless of
# baseline). File-LOC ceiling: soft warn threshold + hard fail threshold, with
# named grandfather exceptions carrying split tickets.
FILE_LOC_SOFT = 2500
FILE_LOC_HARD = 6000
FILE_LOC_GRANDFATHER = {
    # path-suffix -> tracked remediation ticket (WS-E)
    "render/pane/core.rs": "WS-E E4 (PANE_RS_SPLIT_PLAN)",
    "gpu.rs": "WS-E E2 (extract domain types)",
    # WS-H #43 added the money-path test suite (transition matrix, WAL rotation,
    # risk boundaries, serde round-trips), pushing this over 6k. The growth is
    # tests, not production code. TODO: extract the #[cfg(test)] mod tests to a
    # sibling `order_manager/tests.rs` (file→dir) to drop the file back under 6k.
    "trading/order_manager.rs": "WS-H #43 (extract test module — follow-up)",
}

# render hot path unwrap HARD cap (audit: must trend to 0; A5 took it 7->4).
RENDER_UNWRAP_HARD_CAP = 4


def iter_rs_files():
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for f in files:
            if f.endswith(".rs"):
                yield os.path.join(root, f)


def rel(path):
    return os.path.relpath(path, SRC).replace("\\", "/")


def area_of(relpath):
    """Top-level area bucket for unwrap budgeting."""
    p = relpath
    if p.startswith("chart/renderer/render/"):
        return "render"
    if p.startswith("chart/renderer/trading/"):
        return "trading"
    if p.startswith("data/"):
        return "data"
    if p.startswith("dev_inspector/"):
        return "dev_inspector"
    if p.startswith("state/") or p.startswith("persistence/"):
        return "state"
    if p.startswith("ui_kit/"):
        return "ui_kit"
    if p.startswith("chart/renderer/ui/"):
        return "ui"
    if p.startswith("chart/"):
        return "chart_other"
    return "misc"


# Any `#[cfg(...)]` whose predicate names `test` as a whole word — covers the
# bare `#[cfg(test)]` and compound forms like `#[cfg(all(test, feature = "x"))]`.
TEST_CFG_RE = re.compile(r"#\[cfg\([^\n]*\btest\b")
UNWRAP_RE = re.compile(r"\.unwrap\(\)")
EXPECT_RE = re.compile(r"\.expect\(")
DEAD_RE = re.compile(r"#\[allow\([^)]*dead_code")
# A COMMENT LINE. `dead_code_allows` is a count of attributes, and an attribute
# written inside prose is not one — but the regex above cannot tell the
# difference, so three doc comments that merely NAME `#[allow(dead_code)]`
# while explaining it were being counted as three suppressions.
#
# This is AT-154 again, the third time in this repo: the ratchet counted test
# fixtures, the cascade ceiling counted comments describing migrations it had
# already done, and now this. The failure mode is always the same and always
# in the same direction — the gate reports work that does not exist, so the
# only way to make it pass is to stop writing the explanation. A gate that
# penalises documenting itself is worse than no gate.
COMMENT_LINE_RE = re.compile(r"^\s*(?://|\*|/\*)")
# F2: raw stderr in the data/ layer must route through errors_sink/tracing so a
# misconfig surfaces as a persistent in-app indicator, not a one-time console
# line. Require an open paren so prose mentions in doc-comments don't count.
# TEXT WIDTH GUESSED FROM A CHARACTER COUNT: `label.len() as f32 * 6.5`.
#
# Wrong twice over. Immediately, for any proportional face, where `W` and `i`
# are not the same width — so two labels of equal LENGTH need different space.
# And eventually for every face, because the constant was measured once against
# whatever font was current that day. That is the pane-header defect that
# motivated `Button::measure_content_w`: a 60px slot sized for a font that had
# since grown, overrun by "LAYERS", with the next control painted on top.
#
# `ui_kit::style::measure_with` / `measure_with_painter` measure it instead, and
# egui caches galleys, so measuring a string you are about to paint is a hash
# lookup rather than a shaping pass.
#
# The baseline is NOT zero, and the number should be read carefully: every
# remaining site is in `render/pane/core.rs` or `tps_overlay.rs`. The first is
# the chart engine, which is out of scope by standing directive; the second is
# a deliberate pastiche of Excel's chrome whose literals are Excel's. In-scope
# UI and chrome is at ZERO. The ratchet only falls, so a new guess anywhere —
# including in those two files — fails it.
#
# COUNT x PITCH is a different thing and must not be caught: `rows.len() as f32
# * 30.0` is a stack height and is correct. They cannot be told apart
# numerically, so they are told apart by NAME — a collection reads as a plural
# or a `_count`, a string does not. The list is deliberately narrow; a false
# NEGATIVE here just means one guess goes unratcheted, while a false positive
# would make the gate demand nonsense.
# EXPLICIT suffixes only. The first version of this also excluded a bare
# trailing `s`, which silently dropped `qty_s`, `not_s`, `status_s` and
# `price_s` — all of them STRINGS whose width was being guessed, i.e. exactly
# what this is for. A ceiling that under-reports lets the thing through, so the
# list names what it means instead of pattern-matching a plural.
# Matched as a whole word OR a suffix, so both `self.lines` and `pinned_items`
# read as collections. Suffix-only missed the bare `lines` in `tooltip.rs`,
# where `lines.len() * line_h()` is a stack height and entirely correct.
_COLLECTION_NAME_RE = re.compile(
    r"(?:^|_)(?:list|items|count|indices|zones|rows|lines|entries|trades)$"
)
TEXT_WIDTH_GUESS_RE = re.compile(
    r"\.(?:len|chars\(\)\.count)\(\)\s*as\s+f32\s*\*"
)


def _is_text_width_guess(line):
    """True for `<string>.len() as f32 * <n>`, false for count x pitch."""
    m = TEXT_WIDTH_GUESS_RE.search(line)
    if not m:
        return False
    before = line[: m.start()]
    ident = re.search(r"([A-Za-z_][A-Za-z0-9_]*)$", before)
    if ident and _COLLECTION_NAME_RE.search(ident.group(1)):
        return False
    return True

# A CONDITIONAL WHOSE BRANCHES ARE THE SAME: `if c { a } else { a }`.
#
# Five existed. Each was one of two things and both are worth catching:
#
#   * LOST INTENT — someone knew the two cases differed and the second value
#     never got written. `welcome.rs` chose `Primary` either way for the
#     wizard's final step; `monitoring.rs` had
#     `let idx = if filled >= RING_SIZE { i } else { i }`, where the wrapped
#     case genuinely needed a rotated index. That one was a real bug: the
#     subsystem "last" column reported whichever frame sat at the highest raw
#     index rather than the newest, for every session longer than ~5 seconds.
#
#   * VESTIGIAL — a test that once selected something and no longer does
#     (`dom_row`'s `price > 0.0`, `heatmap_pane`'s bull/bear picking one tone
#     twice, `compute.rs`'s middle strike rung).
#
# Neither survives review as written, and neither fails a test: both branches
# compile and the code does something reasonable. Only a census sees them.
IDENTICAL_BRANCH_RE = re.compile(
    r"if\s+[^{}\n]{1,80}?\s*\{\s*([^{}\n;]{1,60}?)\s*\}\s*else\s*\{\s*([^{}\n;]{1,60}?)\s*\}"
)


def _has_identical_branches(line):
    m = IDENTICAL_BRANCH_RE.search(line)
    return bool(m) and m.group(1) == m.group(2)

EPRINTLN_RE = re.compile(r"eprintln!\(")
# COLOUR ARITHMETIC written out by hand: `c.r() as f32 * k`.
#
# Four implementations of "multiply the RGB channels" existed simultaneously —
# `interaction::brighten_color`, `style::darken`, `color_shade`, and a local
# `fn brighten` hidden inside `gpu::indicator_default_color` — and they
# disagreed at the edges: one rounded where three truncated, one dropped alpha,
# one stamped a new one. Four spellings of one operation is the same shape as
# the tab strip's two spellings of a gap: not wrong today, wrong the moment one
# is edited.
#
# Consolidated onto `ui_kit::style::scale_channels`, and this holds it at ZERO.
# The number is not "few open-coded scales are acceptable" — it is that there
# is one implementation and everything else is a wrapper over it.
CHANNEL_MATH_RE = re.compile(r"\.\s*[rgb]\s*\(\s*\)\s*as\s+f32\s*\*")
# Direct field mutation in ui/: `wl.foo = ` / `watchlist.foo = ` / `chart.foo = `
# (crude but matches the audit's counting method; excludes ==, +=, <=, >=, !=).
MUT_RE = re.compile(r"\b(watchlist|wl|chart)\.[a-z_][a-z0-9_]*\s*=\s*[^=]")
# W3-01 Stage 4: `IndicatorType::` references — the ~210-site sprawl that made
# adding one indicator a cross-file chore. Stages 1-3 routed metadata, compute,
# and persistence through the registry (chart::indicators); this ratchet locks
# the count so new code goes through the registry instead of re-growing the enum
# match sites. Does NOT need to reach 0 (the enum def + its registry_id/label/
# from_persisted matches are legitimate); it just may only shrink.
INDICATOR_ENUM_RE = re.compile(r"IndicatorType::")


def collect():
    counts = {
        "unwrap_by_dir": {},
        "expect_total": 0,
        "hand_colour_math": 0,
        "text_width_guess": 0,
        "identical_branches": 0,
        "dead_code_allows": 0,
        "ui_direct_mutation": 0,
        "eprintln_in_data": 0,
        "indicator_enum_matches": 0,   # W3-01 Stage 4: IndicatorType:: sprawl
        "file_loc": {},   # relpath -> loc (only for files over soft ceiling)
    }
    for path in iter_rs_files():
        r = rel(path)
        try:
            with open(path, encoding="utf-8") as fh:
                lines = fh.readlines()
        except (OSError, UnicodeDecodeError):
            continue
        text = "".join(lines)
        area = area_of(r)
        # unwrap/expect are PANIC-RISK metrics — they only matter in production
        # code. `.unwrap()` in a #[cfg(test)] module is idiomatic (a failing
        # unwrap IS the test failure) and must not count against the release
        # budget. Rust convention puts test modules at the end of the file, so
        # truncate at the first #[cfg(test)] for these two counts.
        # AUDIT 2026-08: this used to search for the LITERAL "#[cfg(test)]" only,
        # so a test module gated on a compound predicate — e.g.
        # `#[cfg(all(test, feature = "design-mode"))]` — was never recognised as
        # tests, and every `.unwrap()`/`.expect()` inside it counted against the
        # production panic budget. A gate that mis-classifies test code as
        # production produces false failures, and a gate that cries wolf gets
        # baselined away, which is how a ratchet quietly stops ratcheting.
        #
        # Match any cfg attribute whose predicate mentions `test` as a whole
        # word, and cut at the earliest one.
        prod = text
        cut = min(
            (m.start() for m in TEST_CFG_RE.finditer(text)),
            default=-1,
        )
        if cut != -1:
            prod = text[:cut]
        n_unwrap = len(UNWRAP_RE.findall(prod))
        if n_unwrap:
            counts["unwrap_by_dir"][area] = counts["unwrap_by_dir"].get(area, 0) + n_unwrap
        counts["expect_total"] += len(EXPECT_RE.findall(prod))
        counts["identical_branches"] += sum(
            1
            for ln in prod.splitlines()
            if not COMMENT_LINE_RE.match(ln) and _has_identical_branches(ln)
        )
        counts["text_width_guess"] += sum(
            1
            for ln in prod.splitlines()
            if not COMMENT_LINE_RE.match(ln) and _is_text_width_guess(ln)
        )
        counts["hand_colour_math"] += sum(
            len(CHANNEL_MATH_RE.findall(ln))
            for ln in prod.splitlines(True)
            if not COMMENT_LINE_RE.match(ln)
        )
        counts["dead_code_allows"] += sum(
            len(DEAD_RE.findall(ln)) for ln in lines if not COMMENT_LINE_RE.match(ln)
        )
        if r.startswith("chart/renderer/ui/") or r == "chart/renderer/gpu.rs":
            counts["ui_direct_mutation"] += len(MUT_RE.findall(text))
        if r.startswith("data/"):
            counts["eprintln_in_data"] += len(EPRINTLN_RE.findall(text))
        # Count IndicatorType:: in production code only (test modules legitimately
        # reference it, e.g. the delegation/persistence guards). `prod` is already
        # truncated at the first #[cfg(test)] above.
        counts["indicator_enum_matches"] += len(INDICATOR_ENUM_RE.findall(prod))
        loc = len(lines)
        if loc > FILE_LOC_SOFT:
            counts["file_loc"][r] = loc
    return counts


def load_baseline():
    with open(BASELINE_PATH, encoding="utf-8") as fh:
        return json.load(fh)


def check(cur, base):
    failures = []
    nudges = []   # real count reductions — prompt --update to lock in
    infos = []    # informational (file-LOC watches) — not a ratchet drop

    # unwrap_by_dir: fail if any area increases; render also hard-capped.
    cur_u, base_u = cur["unwrap_by_dir"], base["unwrap_by_dir"]
    render_now = cur_u.get("render", 0)
    if render_now > RENDER_UNWRAP_HARD_CAP:
        failures.append(f"render/ unwrap {render_now} > HARD CAP {RENDER_UNWRAP_HARD_CAP} "
                        f"(a panic in the render thread kills the window)")
    for area in sorted(set(cur_u) | set(base_u)):
        c, b = cur_u.get(area, 0), base_u.get(area, 0)
        if c > b:
            failures.append(f"unwrap[{area}] {c} > baseline {b} (+{c - b})")
        elif c < b:
            nudges.append(f"unwrap[{area}] {c} < baseline {b} (-{b - c})")

    # scalar ratchets
    for key in ("expect_total", "dead_code_allows", "hand_colour_math",
                "text_width_guess", "identical_branches",
                "ui_direct_mutation", "eprintln_in_data",
                "indicator_enum_matches"):
        # A newly-added metric absent from the committed baseline seeds to its
        # current value (no spurious first-run failure); --update then locks it.
        c = cur[key]
        b = base.get(key, c)
        if c > b:
            failures.append(f"{key} {c} > baseline {b} (+{c - b})")
        elif c < b:
            nudges.append(f"{key} {c} < baseline {b} (-{b - c})")

    # file-LOC ceiling: hard-fail on non-grandfathered file over HARD; warn over SOFT.
    for r, loc in sorted(cur["file_loc"].items()):
        gf = next((t for suf, t in FILE_LOC_GRANDFATHER.items() if r.endswith(suf)), None)
        if loc > FILE_LOC_HARD and gf is None:
            failures.append(f"{r} is {loc} LOC > HARD ceiling {FILE_LOC_HARD} (split it)")
        elif loc > FILE_LOC_HARD and gf:
            infos.append(f"{r} {loc} LOC (grandfathered - {gf})")
        else:
            infos.append(f"{r} {loc} LOC > soft {FILE_LOC_SOFT} (watch)")

    return failures, nudges, infos


def main():
    arg = sys.argv[1] if len(sys.argv) > 1 else ""
    cur = collect()

    if arg == "--show":
        print(json.dumps(cur, indent=2, sort_keys=True))
        return 0

    if arg == "--update":
        with open(BASELINE_PATH, "w", encoding="utf-8") as fh:
            json.dump(cur, fh, indent=2, sort_keys=True)
            fh.write("\n")
        print(f"baseline updated -> {os.path.relpath(BASELINE_PATH, REPO)}")
        return 0

    base = load_baseline()
    failures, nudges, infos = check(cur, base)

    for n in infos:
        print(f"  ..   {n}")
    for n in nudges:
        print(f"  ok   {n}")
    if nudges:
        print("  (a count dropped - run `python dev/quality_gate.py --update` to lock the gain in)")

    if failures:
        print("\nQUALITY GATE FAILED — these ratchets regressed:")
        for f in failures:
            print(f"  FAIL {f}")
        print("\nFix the regression, or if it is a justified & unavoidable increase, "
              "update the baseline in the same commit with a reason in the message.")
        return 1

    print("\nquality gate: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
