#!/usr/bin/env bash
# style-mig-lint.sh — Style-migration guardrail lints (Stream S0)
#
# Runs four ratchet checks against a recorded baseline in docs/migration/baselines.toml.
# Each check asserts that the live count is <= the baseline value.
# The baseline is lowered by hand after each migration stream lands (see docs/migration/README.md).
#
# Usage:
#   bash scripts/style-mig-lint.sh          # run from repo root
#   bash scripts/style-mig-lint.sh --verbose # print match lists as well
#
# Exit codes:
#   0  all checks pass
#   1  one or more ratchets tripped

# Note: -e is intentionally omitted — grep exits 1 when it finds no matches,
# which would abort the script prematurely.  We track failures manually.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINES_FILE="${REPO_ROOT}/docs/migration/baselines.toml"
SRC="${REPO_ROOT}/src-tauri/src"
STYLE_RS="${SRC}/chart/renderer/ui/style.rs"
UI_KIT="${SRC}/ui_kit"

VERBOSE=0
if [[ "${1:-}" == "--verbose" ]]; then
  VERBOSE=1
fi

# ── Colour helpers ─────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}PASS${NC}  $*"; }
fail() { echo -e "${RED}FAIL${NC}  $*"; }
info() { echo -e "${YELLOW}INFO${NC}  $*"; }

# ── Baseline parser (reads a simple key = value TOML, no arrays) ───────────────
# Usage: baseline_value KEY
baseline_value() {
  local key="$1"
  grep -m1 "^${key}\s*=" "${BASELINES_FILE}" | sed 's/.*=\s*//' | tr -d ' \r\n'
}

# ── Count helper: grep with no-match treated as 0 ─────────────────────────────
# Usage: grep_count <pattern> <path> [extra grep flags...]
# Runs grep -rn; if grep exits 1 (no matches) that is fine — wc -l gives 0.
grep_count() {
  local pattern="$1"; shift
  local path="$1"; shift
  { grep -rn "$@" "${pattern}" "${path}" 2>/dev/null || true; } | wc -l | tr -d ' '
}

# ── Exit tracking ──────────────────────────────────────────────────────────────
FAILURES=0
fail_check() {
  local name="$1" live="$2" limit="$3"
  fail "${name}: live=${live} exceeds baseline=${limit} — a regression was introduced"
  FAILURES=$((FAILURES + 1))
}

# ══════════════════════════════════════════════════════════════════════════════
# CHECK 1 — StyleSettings pub field count
#   Baseline: count of `pub <field>:` lines inside the StyleSettings struct.
#   Ratchet: live count must be <= baseline.  Adding a field trips the check.
#   To lower the baseline after removing a field, update baselines.toml.
# ══════════════════════════════════════════════════════════════════════════════
CHECK1_NAME="StyleSettings pub field count"
BASELINE_STYLE=$(baseline_value "style_settings_pub_fields")

# Find the line range of the StyleSettings struct body
STRUCT_START=$(grep -n "^pub struct StyleSettings" "${STYLE_RS}" | head -1 | cut -d: -f1)
if [[ -z "${STRUCT_START}" ]]; then
  fail "${CHECK1_NAME}: cannot locate 'pub struct StyleSettings' in ${STYLE_RS}"
  FAILURES=$((FAILURES + 1))
else
  # Find the closing brace: first line > STRUCT_START that is exactly "}"
  STRUCT_END=$(awk -v start="${STRUCT_START}" 'NR>start && /^\}/ {print NR; exit}' "${STYLE_RS}")
  LIVE_STYLE=$(awk -v s="${STRUCT_START}" -v e="${STRUCT_END}" \
    'NR>s && NR<e && /^[[:space:]]+pub [a-z_]/' "${STYLE_RS}" | wc -l | tr -d ' ')

  if [[ "${LIVE_STYLE}" -le "${BASELINE_STYLE}" ]]; then
    pass "${CHECK1_NAME}: ${LIVE_STYLE} <= ${BASELINE_STYLE}"
  else
    fail_check "${CHECK1_NAME}" "${LIVE_STYLE}" "${BASELINE_STYLE}"
  fi

  if [[ "${VERBOSE}" -eq 1 ]]; then
    info "  StyleSettings pub fields (${LIVE_STYLE}):"
    awk -v s="${STRUCT_START}" -v e="${STRUCT_END}" \
      'NR>s && NR<e && /^[[:space:]]+pub [a-z_]/' "${STYLE_RS}" \
      | sed 's/.*pub \([a-z_]*\):.*/    \1/'
  fi
fi

# ══════════════════════════════════════════════════════════════════════════════
# CHECK 2 — crate::chart_renderer / crate::chart::renderer refs in ui_kit/
#   Baseline: current count of cross-boundary import references.
#   These should only go DOWN as ui_kit becomes properly decoupled.
#   Ratchet: live count must be <= baseline.
# ══════════════════════════════════════════════════════════════════════════════
CHECK2_NAME="chart_renderer refs inside ui_kit/"
BASELINE_XREF=$(baseline_value "chart_renderer_refs_in_ui_kit")

LIVE_XREF=$({ grep -rn "crate::chart_renderer\|crate::chart::renderer" "${UI_KIT}" 2>/dev/null || true; } | wc -l | tr -d ' ')

if [[ "${LIVE_XREF}" -le "${BASELINE_XREF}" ]]; then
  pass "${CHECK2_NAME}: ${LIVE_XREF} <= ${BASELINE_XREF}"
else
  fail_check "${CHECK2_NAME}" "${LIVE_XREF}" "${BASELINE_XREF}"
fi

if [[ "${VERBOSE}" -eq 1 && "${LIVE_XREF}" -gt 0 ]]; then
  info "  Remaining cross-boundary refs:"
  { grep -rn "crate::chart_renderer\|crate::chart::renderer" "${UI_KIT}" 2>/dev/null || true; } \
    | sed "s|${REPO_ROOT}/||" | sed 's/^/    /'
fi

# ══════════════════════════════════════════════════════════════════════════════
# CHECK 3 — &THEMES[0] usage in code paths (not in comments)
#   Baseline: 0  (all existing grep hits are comment-only references).
#   Rule: no new code-path usage of &THEMES[0] anywhere in src-tauri/src/.
#   Comment-only lines (trimmed line starts with //) are excluded.
#   Inline comment references (pattern appears only after // on the line) are
#   also excluded.  This is effectively a HARD BAN — baseline is 0.
#   Known allowed comment hits at baseline capture:
#     src/chart/renderer/ui/chrome/pane.rs:26    (// comment)
#     src/chart/renderer/ui/inputs/form.rs:25    (// comment)
#     src/chart/renderer/ui/inputs/form.rs:401   (// comment)
#     src/chart/renderer/ui/inputs/nmf_toggle.rs:28 (// comment)
# ══════════════════════════════════════════════════════════════════════════════
CHECK3_NAME="&THEMES[0] code-path references"
BASELINE_THEMES=$(baseline_value "themes_zero_code_refs")

# Step 1: find all lines with &THEMES[0], Step 2: drop pure comment-only lines,
# Step 3: drop lines where the pattern is only in the inline comment portion.
LIVE_THEMES=$(
  { grep -rn "&THEMES\[0\]" "${SRC}" 2>/dev/null || true; } \
  | { grep -v $'^[^:]*:[0-9]*:[[:space:]]*//' || true; } \
  | { grep -v "//.*&THEMES\[0\]" || true; } \
  | wc -l | tr -d ' '
)

if [[ "${LIVE_THEMES}" -le "${BASELINE_THEMES}" ]]; then
  pass "${CHECK3_NAME}: ${LIVE_THEMES} <= ${BASELINE_THEMES} (hard ban — baseline is 0)"
else
  fail_check "${CHECK3_NAME}" "${LIVE_THEMES}" "${BASELINE_THEMES}"
fi

if [[ "${VERBOSE}" -eq 1 ]]; then
  info "  &THEMES[0] code-path hits (should be 0):"
  { grep -rn "&THEMES\[0\]" "${SRC}" 2>/dev/null || true; } \
    | { grep -v $'^[^:]*:[0-9]*:[[:space:]]*//' || true; } \
    | { grep -v "//.*&THEMES\[0\]" || true; } \
    | sed "s|${REPO_ROOT}/||" | sed 's/^/    /' || true
fi

# ══════════════════════════════════════════════════════════════════════════════
# CHECK 4 — Color32::from_rgba_unmultiplied(0, 0, 0, … literal-black shadow
#   Baseline: current count of literal-black shadow expressions.
#   These should only go DOWN as call sites are migrated to themed shadows.
#   Ratchet: live count must be <= baseline.
#   Known hits at baseline capture:
#     src/chart/renderer/ui/style.rs:1647  (bevel_shadow_alpha — token fn)
#     src/chart/renderer/ui/style.rs:1712  (doc comment)
# ══════════════════════════════════════════════════════════════════════════════
CHECK4_NAME="Color32::from_rgba_unmultiplied(0, 0, 0, literal-black shadows"
BASELINE_SHADOW=$(baseline_value "literal_black_shadow_refs")

LIVE_SHADOW=$({ grep -rn "Color32::from_rgba_unmultiplied(0, 0, 0," "${SRC}" 2>/dev/null || true; } | wc -l | tr -d ' ')

if [[ "${LIVE_SHADOW}" -le "${BASELINE_SHADOW}" ]]; then
  pass "${CHECK4_NAME}: ${LIVE_SHADOW} <= ${BASELINE_SHADOW}"
else
  fail_check "${CHECK4_NAME}" "${LIVE_SHADOW}" "${BASELINE_SHADOW}"
fi

if [[ "${VERBOSE}" -eq 1 && "${LIVE_SHADOW}" -gt 0 ]]; then
  info "  Literal-black shadow hits:"
  { grep -rn "Color32::from_rgba_unmultiplied(0, 0, 0," "${SRC}" 2>/dev/null || true; } \
    | sed "s|${REPO_ROOT}/||" | sed 's/^/    /'
fi

# ══════════════════════════════════════════════════════════════════════════════
# Summary
# ══════════════════════════════════════════════════════════════════════════════
echo ""
if [[ "${FAILURES}" -eq 0 ]]; then
  echo -e "${GREEN}All style-migration lint checks passed.${NC}"
  exit 0
else
  echo -e "${RED}${FAILURES} style-migration lint check(s) FAILED.${NC}"
  echo "  See docs/migration/README.md for how to fix regressions or lower baselines."
  exit 1
fi
