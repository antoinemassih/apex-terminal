#!/usr/bin/env bash
# Run EVERY gate CI runs, in one command.
#
# This exists because of a specific failure, and then immediately failed the
# same way a second time — which is why it now covers BOTH workflows.
#
#   1. Gates were being run individually from memory, and `style-mig-lint.sh`
#      was not among the ones remembered. It had been failing since
#      2026-08-16, so `Design System Lint` was RED for a whole long session
#      while local runs reported "all gates pass" — a true statement about the
#      gates that were run, and a misleading one about the gates that exist.
#
#   2. The first version of THIS script fixed that for exactly one workflow.
#      `Quality Gates` — a separate file, running `quality_gate.py` and a
#      three-way build matrix — was still red, and still unchecked, for the
#      same reason at one remove: the script was complete against the list it
#      knew about. A checklist derived from one source cannot report a gap in
#      the source it was derived from.
#
# So: every check from every workflow, and a drift check against ALL of them.
#
# Usage:
#   dev/run_all_gates.sh          fast — lint/ratchet gates only (~seconds)
#   dev/run_all_gates.sh --full   also the cargo matrix, tests and clippy
#
# `--full` is what CI actually runs. The fast tier is a convenience for a tight
# edit loop, NOT a substitute: the `--no-default-features` build was broken for
# six weeks precisely because nobody ran the config they were not editing.
set -u
cd "$(dirname "$0")/.." || exit 1

FULL=0
[[ "${1:-}" == "--full" ]] && FULL=1

# ── Tier 1: lint / ratchet gates (both workflows) ────────────────────────────
CHECKS=(
  # design-system-check.yml
  "bash scripts/check-design-system.sh"
  "bash scripts/style-mig-lint.sh"
  "bash scripts/sx_ratchet.sh"
  "bash scripts/recipe_adoption_gate.sh"
  "python scripts/radius_lint.py"
  "python scripts/control_size_lint.py"
  "python dev/token_consumer_gate.py"
  "python dev/single_system_gate.py"
  "python dev/ladder_gate.py"
  "python dev/inspector_slider_gate.py"
  "python dev/hardwire_gate.py"
  "python dev/cascade_gate.py"
  "python dev/strip_test_hits.py --selftest"
  "python dev/cascade_adoption_gate.py"
  # quality-gates.yml
  "python dev/quality_gate.py"
)

# ── Tier 2: the builds (slow; only with --full) ──────────────────────────────
# The three-way matrix is the point. `--no-default-features` selects the legacy
# egui render path, and a symbol can be cfg-gated on `gpu_chart_v2` at its
# DEFINITION while its use is not — which compiles cleanly by default and not
# at all here. That is a real break that only this configuration can see.
#
# `--manifest-path` rather than a `cd`: the crate lives in `src-tauri/`, and
# the tier-1 gates above are all repo-root-relative. Changing directory for
# half the script is how the first run of this tier reported six failures that
# were entirely "cargo was in the wrong folder".
M=src-tauri/Cargo.toml
FULL_CHECKS=(
  "cargo check  --manifest-path $M --lib"
  "cargo check  --manifest-path $M --lib --no-default-features"
  "cargo check  --manifest-path $M --lib --features design-mode"
  "cargo test   --manifest-path $M --lib"
  "cargo clippy --manifest-path $M --lib --no-deps -- -A clippy::all"
  "cargo check  --manifest-path $M --bins"
)

# Cross-check against EVERY workflow so this list cannot drift out of date.
# Counting `run:` lines is a heuristic, not a parse — it is here to make a new
# gate visible, not to validate the list.
wf_total=0
for WF in .github/workflows/design-system-check.yml .github/workflows/quality-gates.yml; do
  [[ -f "$WF" ]] || continue
  wf_total=$((wf_total + $(grep -cE "^\s+run: (bash |python |cargo )" "$WF")))
done
if [[ "$wf_total" -gt 0 ]]; then
  ours=$(( ${#CHECKS[@]} + ${#FULL_CHECKS[@]} ))
  if [[ "$wf_total" -ne "$ours" ]]; then
    echo "NOTE: workflows run ~$wf_total checks, this script runs $ours."
    echo "      Counts differ for benign reasons (matrix expansion, selftests"
    echo "      with no CI counterpart). Investigate a GROWING gap — that is a"
    echo "      gate added to CI and not here, which is how this went red twice."
    echo
  fi
fi

run_tier() {
  local -n arr=$1
  for c in "${arr[@]}"; do
    local name
    name=$(echo "$c" | sed "s|--manifest-path $M ||; s|.*/||")
    printf "%-48s " "$name"
    if out=$($c 2>&1); then
      echo "PASS"
    else
      echo "*** FAIL"
      echo "$out" | tail -8 | sed 's/^/      /'
      fails=$((fails + 1))
    fi
  done
}

fails=0
run_tier CHECKS
total=${#CHECKS[@]}
if [[ "$FULL" -eq 1 ]]; then
  echo
  echo "--- builds (this is the slow part) ---"
  run_tier FULL_CHECKS
  total=$((total + ${#FULL_CHECKS[@]}))
else
  echo
  echo "(builds skipped — run with --full before pushing)"
fi

echo
if [[ "$fails" -eq 0 ]]; then
  echo "All $total gates pass."
else
  echo "$fails gate(s) FAILED."
fi
exit "$fails"
