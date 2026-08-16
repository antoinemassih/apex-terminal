#!/usr/bin/env bash
# Run EVERY gate CI runs, in one command.
#
# This exists because of a specific failure: gates were being run individually
# from memory, and `style-mig-lint.sh` was not among the ones remembered. It had
# been failing since 2026-08-16, so `Design System Lint` was RED in CI for the
# whole of a long session while local runs reported "all gates pass" — a true
# statement about the gates that were run, and a misleading one about the gates
# that exist.
#
# The list below is derived from the workflow itself, so a gate added to CI and
# not here shows up as a diff rather than as a silent gap.
set -u
cd "$(dirname "$0")/.." || exit 1

CHECKS=(
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
)

# Cross-check against the workflow so this list cannot drift out of date.
WF=.github/workflows/design-system-check.yml
if [[ -f "$WF" ]]; then
  wf_count=$(grep -cE "^\s+run: (bash |python )" "$WF")
  if [[ "$wf_count" -ne "${#CHECKS[@]}" ]]; then
    echo "WARNING: workflow runs $wf_count checks, this script runs ${#CHECKS[@]}."
    echo "         A gate was added to CI without being added here — that is the"
    echo "         exact gap this script was written to close."
  fi
fi

fails=0
for c in "${CHECKS[@]}"; do
  name=$(echo "$c" | sed 's|.*/||')
  printf "%-44s " "$name"
  if out=$($c 2>&1); then
    echo "PASS"
  else
    echo "*** FAIL"
    echo "$out" | tail -6 | sed 's/^/      /'
    fails=$((fails + 1))
  fi
done

echo
if [[ "$fails" -eq 0 ]]; then
  echo "All ${#CHECKS[@]} gates pass."
else
  echo "$fails gate(s) FAILED."
fi
exit "$fails"
