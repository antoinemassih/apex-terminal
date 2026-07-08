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


UNWRAP_RE = re.compile(r"\.unwrap\(\)")
EXPECT_RE = re.compile(r"\.expect\(")
DEAD_RE = re.compile(r"#\[allow\([^)]*dead_code")
# F2: raw stderr in the data/ layer must route through errors_sink/tracing so a
# misconfig surfaces as a persistent in-app indicator, not a one-time console
# line. Require an open paren so prose mentions in doc-comments don't count.
EPRINTLN_RE = re.compile(r"eprintln!\(")
# Direct field mutation in ui/: `wl.foo = ` / `watchlist.foo = ` / `chart.foo = `
# (crude but matches the audit's counting method; excludes ==, +=, <=, >=, !=).
MUT_RE = re.compile(r"\b(watchlist|wl|chart)\.[a-z_][a-z0-9_]*\s*=\s*[^=]")


def collect():
    counts = {
        "unwrap_by_dir": {},
        "expect_total": 0,
        "dead_code_allows": 0,
        "ui_direct_mutation": 0,
        "eprintln_in_data": 0,
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
        prod = text
        cut = text.find("#[cfg(test)]")
        if cut != -1:
            prod = text[:cut]
        n_unwrap = len(UNWRAP_RE.findall(prod))
        if n_unwrap:
            counts["unwrap_by_dir"][area] = counts["unwrap_by_dir"].get(area, 0) + n_unwrap
        counts["expect_total"] += len(EXPECT_RE.findall(prod))
        counts["dead_code_allows"] += len(DEAD_RE.findall(text))
        if r.startswith("chart/renderer/ui/") or r == "chart/renderer/gpu.rs":
            counts["ui_direct_mutation"] += len(MUT_RE.findall(text))
        if r.startswith("data/"):
            counts["eprintln_in_data"] += len(EPRINTLN_RE.findall(text))
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
    for key in ("expect_total", "dead_code_allows", "ui_direct_mutation", "eprintln_in_data"):
        c, b = cur[key], base[key]
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
