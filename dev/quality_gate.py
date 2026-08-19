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
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import strip_test_hits

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
# BOTH forms: `#[allow(dead_code)]` on an item AND `#![allow(dead_code)]` at
# the top of a file.
#
# The regex was `#[allow`, which cannot match `#![allow` - `#` is followed by
# `!`, not `[`. So the ratchet counted 46 item-level allows and was blind to 41
# FILE-level ones, which are the more powerful kind: each suppresses an entire
# file rather than one item.
#
# That is not hypothetical. `alert_row.rs` and `option_chain_row.rs` sat in the
# tree with no callers and no warning for exactly this reason (AT-179), and
# stripping all 41 allows surfaces ~68 named dead items — 30 functions, 13
# fields, 13 constants, 4 methods, 2 structs, 2 enums — plus a wave of unused
# imports. The gate meant to measure suppression was blind to the half that
# suppresses most.
#
# The baseline rises to include them. That is not a regression being waved
# through: it is the count becoming true. It can only fall from here.
DEAD_RE = re.compile(r"#!?\[allow\([^)]*dead_code")
# Split by SCOPE, because a single count cannot express "narrower is better".
#
# Converting `painter_pane`'s file-level allow into three item-level ones,
# each carrying the reason it exists, made the suppression strictly smaller
# and the total go UP — 2 removed, 3 added. The gate fired on an improvement.
#
# So the file-level count is its own ceiling and should fall to zero: a
# blanket allow over a file is never the right shape, because it also covers
# every item added to that file afterwards. The item-level count is a ceiling
# too, but a much weaker claim — an item allow with a stated reason is a
# decision, where a file allow is the absence of one.
DEAD_FILE_RE = re.compile(r"#!\[allow\([^)]*dead_code")
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
# `hist` joined the list when the test-stripping fix (see `_production_only`)
# made 2,723 previously-invisible lines of `dev_inspector/server.rs` countable
# and surfaced `hist.len() as f32 * bar_w` — a histogram's BAR COUNT times its
# bar pitch, which is the count-x-pitch case this exemption exists for and not
# a text-width guess at all.
_COLLECTION_NAME_RE = re.compile(
    r"(?:^|_)(?:list|items|count|indices|zones|rows|lines|entries|trades|hist)$"
)
# NOTE the optional `\)` before the `*`. The first version required the cast to
# be followed IMMEDIATELY by the multiply, so it matched
# `tag.len() as f32 * 5.0` and missed `(tag.len() as f32) * 5.0` — the same
# guess with parentheses. `news_row.rs` had exactly that form, sizing a tag chip
# from its character count next to an overflow check that breaks when the guess
# is wrong, and the ratchet read the file as clean.
#
# A sub-agent census found it, which is the point worth recording: the gate was
# green and the defect was there, and no amount of re-running the gate would
# have surfaced it. A ceiling only holds the shape it can see.
TEXT_WIDTH_GUESS_RE = re.compile(
    r"\.(?:len|chars\(\)\.count)\(\)\s*as\s+f32\s*\)?\s*\*"
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
# A RATIO COMPUTED UNDER A ">0" GUARD WHOSE ELSE-BRANCH IS A BARE `0.0`.
#
#     let change_pct = if prev_close > 0.0 {
#         (price - prev_close) / prev_close * 100.0
#     } else {
#         0.0            // <-- "no previous close" rendered as "unchanged"
#     };
#
# The guard is there because the denominator may be absent. The `else` then
# answers the absent case with a NUMBER, and `0.0` is not a neutral one — it is
# the assertion "unchanged", which the rest of the app acts on:
#
#   * The scanner's "Top Gainers" preset filters `change_pct >= 0.0`. A symbol
#     with no previous close scored exactly `0.0`, passed, and was listed as a
#     gainer. "Top Losers" filters `<= 0.0`, so the SAME symbol was listed as a
#     loser at the same time.
#   * A change cell colours on `>= 0.0`, so the unknown painted BULL green and
#     read `+0.00%` — the most confident thing a price cell can say.
#   * Three sites then inverted the fabricated percentage BACK into a previous
#     close (`price / (1.0 + pct / 100.0)`, falling back to `price` at `0.0`),
#     which wrote `prev_close == price` with `loaded: true` beside it. "Save
#     scan as watchlist" committed that to disk, so the unknown became a
#     permanent, confident `0.00%`.
#
# The day-change family — six sites — is fixed: `foundation::market::
# day_change_pct` returns `None` when there is nothing to divide by, so the
# caller must decide what to show.
#
# THE BASELINE IS NOT ZERO, AND MOST OF WHAT IT COUNTS IS FINE. The shape has
# two populations and this regex cannot tell them apart:
#
#   * The denominator is genuinely ZERO. `plus_dm_sum / tr_sum` with no true
#     range, `above_avg / total_vol` with no volume, `cell.weight / total_cap`
#     with an empty book. Nothing moved, `0.0` is the right answer, and these
#     are the majority.
#   * The denominator is ABSENT. `(last / entry_price - 1.0) * 100.0` where
#     `entry_price == 0.0` means there is NO POSITION — and renders a P&L of
#     `0.00%`. `maintenance_margin / net_liq` where `net_liq == 0.0` means the
#     account snapshot has not loaded — and renders 0% margin usage, which
#     reads as "no margin risk". Those are the day-change defect wearing
#     different clothes, on the trading surface, and they are tracked
#     separately in the ledger.
#
# So this is a CEILING on the shape, in the same spirit as `dead_code_allows`
# starting at 52: it stops the population growing while the suspect subset is
# worked through. Do not read a passing run as "no fabrication here".
#
# The division inside the then-branch is load-bearing — it is what makes the
# value a RATIO, and a ratio has no meaningful zero when its base is missing.
# `else { 0.0 }` after a non-division guard is usually a genuine default and is
# deliberately not matched.
FABRICATED_RATIO_RE = re.compile(
    r">\s*0\.0\s*\{[^{}]*/[^{}]*\}\s*else\s*\{\s*0\.0\s*\}"
)


def _fabricated_ratios(prod):
    """Count the pattern over a 3-line sliding window (it is often wrapped)."""
    lines = [ln for ln in prod.splitlines() if not COMMENT_LINE_RE.match(ln)]
    seen = 0
    i = 0
    while i < len(lines):
        window = " ".join(lines[i : i + 3])
        if FABRICATED_RATIO_RE.search(window):
            seen += 1
            i += 3   # do not count one occurrence three times
        else:
            i += 1
    return seen


# A LITERAL INSET FROM A WIDGET BODY'S EDGE: `body.top() + 18.0`.
#
# The on-chart overlay widgets placed everything by hand from the raw `body`
# rect: 142 such expressions across 45 widget bodies, using TWENTY-SIX distinct
# constants (2, 4, 6, 8, 10, 12, 14, 18, 36, 50, ...). Two widgets sitting side
# by side on the same chart had visibly different internal padding, chosen by
# whoever wrote each one, and a reader could not recover why.
#
# It is also functionally broken. `Settings -> Density` (Tight 0.75 / Standard
# 1.0 / Loose 1.25) is a real, persisted, user-facing control, and every spacing
# token multiplies through `spacing_scale_override()`. A literal does not.
# `chart_widgets.rs` referenced a spacing token exactly ONCE in 3,348 lines, so
# changing Density reflowed every panel in the app and left all 45 chart widgets
# exactly where they were.
#
# The replacement is `overlays::kit::{body_content, body_footer, stat}`, which
# hand back token-derived geometry. This is a CEILING, not a floor at zero: the
# remaining expressions are being converted widget by widget, and some are
# genuine per-widget layout (a bar's height, a grid's row pitch) rather than
# padding. It exists so the population cannot grow while that work proceeds.
BODY_INSET_RE = re.compile(r"\bbody\.(?:left|right|top|bottom)\(\)\s*[-+]\s*[0-9]+\.[0-9]+")

# Only the overlay-widget surface owns a `body` rect in this sense.
BODY_INSET_FILES = ("/ui/chart_widgets.rs", "/ui/overlays/")


# A HARDCODED MARKET FIGURE IN AN ON-CHART WIDGET.
#
# Five widget bodies rendered invented data as if it were live:
#
#   draw_cross_asset    ("SPY", "+0.42%"), ("QQQ", "+0.68%"), ("BTC", "+1.8%") ...
#   draw_market_breadth ("ADV / DEC", "1,842 / 1,156"), ("VIX", "18.5")
#   draw_options_flow   ("CALL", "460C 5DTE", "$3.1M", "sweep") x6
#   draw_earnings_mom   ("EPS", "+12%"), ("REV", "+8%"), ("FWD P/E", "22.4x")
#   draw_risk_reward    risk = 1.0, reward = 2.8  (bar always read 1:2.8)
#
# None of them read `WidgetData`; three took no data parameter at all. On a
# terminal used to place real orders, a titled panel showing a figure a trader
# can act on is not a placeholder.
#
# The honest pattern already existed and was already used by three sibling
# widgets — `draw_widget_no_feed(p, body, t, "<name> feed", "not connected")`.
# It was simply applied inconsistently, which is the worst case: the user
# learns that "not connected" means not connected, and therefore that a panel
# showing numbers means it IS connected.
#
# WHAT THIS MATCHES: a string literal that is unambiguously a market VALUE —
# a signed percentage, a currency figure, or a multiple. Labels ("SPY", "EPS")
# are bare words and are not matched; format strings are excluded because they
# contain a brace. It is deliberately narrow, and therefore not exhaustive: a
# bare "18.5" carries no marker and slips through. A ceiling only holds the
# shape it can see, so this backs up review rather than replacing it.
MARKET_LITERAL_RE = re.compile(
    r'"(?:'
    r'[+-][0-9]+(?:[.][0-9]+)?%'          # +0.42%  -1.2%
    r'|[$][0-9]+(?:[.][0-9]+)?[KMB]'      # $3.1M   $890K
    r'|[0-9]+[.][0-9]+x'                  # 22.4x (a decimal is required)
    r')"'
)

# `"4x"` is the max-end AXIS LABEL on the tape-speed gauge (a 0..4x scale),
# while the reading itself is `format!("{:.1}x", speed)` from real data. A scale
# label is a round number; a reported metric carries a decimal. Not airtight,
# but a real distinction, and it removes the only false positive without hiding
# the class.
MARKET_LITERAL_FILES = ("/ui/chart_widgets.rs", "/ui/overlays/")


INDICATOR_ENUM_RE = re.compile(r"IndicatorType::")


def _production_only(path, text):
    """`text` with every test line blanked, preserving line numbering.

    This USED to truncate the file at its first `#[cfg(test)]`, on the Rust
    convention that test modules sit at the end. `gpu.rs` is 11,139 lines and
    its first test module starts at line 542, so every metric here saw 4% of
    it — including the `.unwrap()` budget whose own failure message reads "a
    panic in the render thread kills the window". The render thread's own file
    was 96% invisible to it, and the ratchet reported a comfortable number the
    entire time.

    `strip_test_hits.py` already solved this, brace-matched and with a selftest
    gate, after the same assumption hid 17 violations from
    `check-design-system.sh` and made `token_consumer_gate.py` invent 37. Its
    docstring warns that the failure is unbounded: any file that later grows a
    mid-file test module silently drops out below that point. That warning was
    written for one consumer and this one kept the bug. Third time; use the
    shared implementation.

    Blanking rather than deleting keeps line numbers aligned, which matters for
    the multi-line window scans below — deleting would splice unrelated lines
    together and invent matches that span a removed test module.
    """
    lines = text.splitlines()
    try:
        ranges = strip_test_hits.test_line_ranges(path)
    except Exception:
        # Never fail open into "everything is test code" — that would zero
        # every metric and read as a clean sweep.
        return text
    if strip_test_hits.is_test_only_file(path):
        return ""
    for a, b in ranges:
        for i in range(max(a - 1, 0), min(b, len(lines))):
            lines[i] = ""
    return chr(10).join(lines)


def collect():
    counts = {
        "unwrap_by_dir": {},
        "expect_total": 0,
        "hand_colour_math": 0,
        "text_width_guess": 0,
        "identical_branches": 0,
        "dead_code_allows": 0,
        "dead_code_allows_file": 0,
        "ui_direct_mutation": 0,
        "eprintln_in_data": 0,
        "indicator_enum_matches": 0,   # W3-01 Stage 4: IndicatorType:: sprawl
        "fabricated_ratio": 0,         # AT-186: `if base > 0.0 { a/base } else { 0.0 }`
        "overlay_body_insets": 0,      # AT-187: `body.top() + 18.0` vs a spacing token
        "fabricated_market_data": 0,   # AT-190: invented quotes in a widget body
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
        # budget, so test code is stripped before any of these are counted.
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
        prod = _production_only(path, text)
        n_unwrap = len(UNWRAP_RE.findall(prod))
        if n_unwrap:
            counts["unwrap_by_dir"][area] = counts["unwrap_by_dir"].get(area, 0) + n_unwrap
        counts["expect_total"] += len(EXPECT_RE.findall(prod))
        counts["identical_branches"] += sum(
            1
            for ln in prod.splitlines()
            if not COMMENT_LINE_RE.match(ln) and _has_identical_branches(ln)
        )
        counts["fabricated_ratio"] += _fabricated_ratios(prod)
        if any(k in "/" + r for k in BODY_INSET_FILES):
            counts["overlay_body_insets"] += len(BODY_INSET_RE.findall(prod))
        if any(k in "/" + r for k in MARKET_LITERAL_FILES):
            counts["fabricated_market_data"] += sum(
                len(MARKET_LITERAL_RE.findall(ln))
                for ln in prod.splitlines()
                if not COMMENT_LINE_RE.match(ln) and "///" not in ln
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
        counts["dead_code_allows_file"] += sum(
            len(DEAD_FILE_RE.findall(ln)) for ln in lines if not COMMENT_LINE_RE.match(ln)
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
    for key in ("expect_total", "dead_code_allows", "dead_code_allows_file", "hand_colour_math",
                "text_width_guess", "identical_branches",
                "ui_direct_mutation", "eprintln_in_data",
                "indicator_enum_matches", "fabricated_ratio",
                "overlay_body_insets", "fabricated_market_data"):
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


# Every pattern in this file, with a sample it MUST match and one it must NOT.
#
# This exists because `BODY_INSET_RE` was written with a `` that collapsed
# into a literal BACKSPACE character (0x08) on its way into the source. The
# pattern printed correctly, compiled without error, and matched nothing — so
# the metric read 0, and 0 was about to be committed as its baseline. A ceiling
# of 0 on a regex that can never match is a gate that passes forever while
# reporting success, which is worse than no gate at all.
#
# The same failure mode has now appeared four times in this codebase's tooling
# under different disguises. A regex that cannot be shown to match anything is
# not a check; it is a decoration. Every pattern here has to prove it works.
PATTERN_SELFTESTS = [
    ("MARKET_LITERAL_RE", lambda: MARKET_LITERAL_RE,
     ['("SPY", "+0.42%", true),', 'let v = "$3.1M";', '("FWD P/E", "22.4x", t.dim),'],
     ['("SPY", "spy"),', 'format!("{:+.2}%", chg)', 'let s = "not connected";',
      '"4x", mono_4xs(), color_dim(t.dim));']),
    ("BODY_INSET_RE", lambda: BODY_INSET_RE,
     ["    let bar_y = body.top() + 50.0;", "body.right() - 8.0"],
     ["let x = other.top() + 4.0;", "body.top() + gap_sm()"]),
    ("TEXT_WIDTH_GUESS_RE", lambda: TEXT_WIDTH_GUESS_RE,
     ["let w = tag.len() as f32 * 5.0;", "let w = (tag.len() as f32) * 6.5;"],
     ["let w = measure(tag);"]),
    ("FABRICATED_RATIO_RE", lambda: FABRICATED_RATIO_RE,
     ["if prev > 0.0 { (p - prev) / prev * 100.0 } else { 0.0 }"],
     ["if prev > 0.0 { compute(prev) } else { 0.0 }"]),
    ("DEAD_RE", lambda: DEAD_RE, ["#[allow(dead_code)]"], ["// #[allow(dead)]x"]),
    ("DEAD_FILE_RE", lambda: DEAD_FILE_RE, ["#![allow(dead_code)]"], ["#[allow(dead_code)]"]),
    ("UNWRAP_RE", lambda: UNWRAP_RE, ["let x = y.unwrap();"], ["let x = y.unwrap_or(3);"]),
    ("EXPECT_RE", lambda: EXPECT_RE, ['let x = y.expect("m");'], ["let x = y.unwrap();"]),
    ("INDICATOR_ENUM_RE", lambda: INDICATOR_ENUM_RE, ["IndicatorType::Sma"], ["Indicator::Sma"]),
]


def _selftest(quiet=False):
    bad = []
    for name, get, positives, negatives in PATTERN_SELFTESTS:
        try:
            rx = get()
        except NameError:
            bad.append(f"{name}: not defined")
            continue
        if chr(8) in rx.pattern or chr(12) in rx.pattern:
            bad.append(
                f"{name}: pattern contains a literal control character "
                f"({rx.pattern!r}) - a backslash escape was eaten before it "
                f"reached the source"
            )
        for sample in positives:
            if not rx.search(sample):
                bad.append(f"{name}: MUST match but does not: {sample!r}")
        for sample in negatives:
            if rx.search(sample):
                bad.append(f"{name}: must NOT match but does: {sample!r}")
    if bad:
        print("QUALITY-GATE PATTERN SELFTEST FAILED\n")
        for b in bad:
            print(f"   {b}")
        print(
            "\nA pattern that matches nothing makes its metric read 0, and a "
            "\nbaseline of 0 on a dead pattern is a gate that passes forever "
            "\nwhile reporting success."
        )
        return 1
    if not quiet:
        print(f"quality-gate pattern selftest: PASS ({len(PATTERN_SELFTESTS)} patterns)")
    return 0


def main():
    arg = sys.argv[1] if len(sys.argv) > 1 else ""

    if arg == "--selftest":
        return _selftest()

    # Never report on a broken instrument. A dead pattern silently zeroes its
    # metric, and `--update` would then bake that zero in as the baseline.
    # Quiet on success so `--show` emits parseable JSON and nothing else.
    if _selftest(quiet=True) != 0:
        return 1

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
