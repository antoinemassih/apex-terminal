#!/usr/bin/env bash
# check-design-system.sh
# Fails if new code introduces raw egui primitives outside the design-system modules.
# Existing violations are tracked in scripts/.design-system-baseline.txt and are tolerated
# until manually migrated.  Any line NOT in the baseline is a new violation → exit 1.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE_FILE="$REPO_ROOT/scripts/.design-system-baseline.txt"
SRC_DIR="$REPO_ROOT/src-tauri/src"

# Patterns to detect
PATTERNS=(
  "egui::Button::new("
  "egui::TextEdit::singleline"
  "Color32::from_rgb("
)

# Paths that are explicitly allowed to use raw egui primitives
ALLOWED_PATHS=(
  "chart_renderer/ui/style.rs"
  "chart_renderer/ui/components.rs"
  "chart_renderer/ui/components_extra.rs"
  "design_inspector.rs"
)

# Build the grep exclude args
EXCLUDE_ARGS=()
for p in "${ALLOWED_PATHS[@]}"; do
  EXCLUDE_ARGS+=(--exclude="*${p##*/}")   # by filename (portable)
done

# We also exclude the designmode directory entirely
EXCLUDE_DIR_ARGS=(--exclude-dir="apex-terminal-designmode" --exclude-dir=".git")

# -----------------------------------------------------------------------
# Collect all current violations (file:line:content)
# -----------------------------------------------------------------------
VIOLATIONS_TMP=$(mktemp)
trap 'rm -f "$VIOLATIONS_TMP"' EXIT

for pat in "${PATTERNS[@]}"; do
  grep -rn \
    "${EXCLUDE_ARGS[@]}" \
    "${EXCLUDE_DIR_ARGS[@]}" \
    --include="*.rs" \
    -F "$pat" \
    "$SRC_DIR" 2>/dev/null || true
done | sort -u > "$VIOLATIONS_TMP"

# Make paths relative to repo root for stable comparison across machines
# (baseline was generated the same way)
VIOLATIONS_REL_TMP=$(mktemp)
trap 'rm -f "$VIOLATIONS_TMP" "$VIOLATIONS_REL_TMP"' EXIT

while IFS= read -r line; do
  rel="${line#$REPO_ROOT/}"
  echo "$rel"
done < "$VIOLATIONS_TMP" | sort -u > "$VIOLATIONS_REL_TMP"

# -----------------------------------------------------------------------
# Compare against baseline
# -----------------------------------------------------------------------
if [[ ! -f "$BASELINE_FILE" ]]; then
  echo "WARNING: baseline file not found at $BASELINE_FILE"
  echo "         Treating all violations as new."
  BASELINE_SORTED=$(mktemp)
  touch "$BASELINE_SORTED"
  trap 'rm -f "$VIOLATIONS_TMP" "$VIOLATIONS_REL_TMP" "$BASELINE_SORTED"' EXIT
else
  BASELINE_SORTED=$(mktemp)
  trap 'rm -f "$VIOLATIONS_TMP" "$VIOLATIONS_REL_TMP" "$BASELINE_SORTED"' EXIT
  sort -u "$BASELINE_FILE" > "$BASELINE_SORTED"
fi

NEW_VIOLATIONS=$(comm -23 "$VIOLATIONS_REL_TMP" "$BASELINE_SORTED")

if [[ -z "$NEW_VIOLATIONS" ]]; then
  TOTAL=$(wc -l < "$VIOLATIONS_REL_TMP" | tr -d ' ')
  echo "Design-system check passed. ($TOTAL baseline violation(s) tolerated, 0 new)"
  exit 0
fi

# -----------------------------------------------------------------------
# Print actionable output for each new violation
# -----------------------------------------------------------------------
echo ""
echo "============================================================"
echo "  DESIGN-SYSTEM VIOLATIONS DETECTED"
echo "  These are NEW violations not present in the baseline."
echo "  Fix them before merging, or add them to the baseline with"
echo "  justification if they are truly unavoidable."
echo "============================================================"
echo ""

FAIL=0
while IFS= read -r vline; do
  [[ -z "$vline" ]] && continue
  FAIL=1

  # Determine which pattern matched to give a useful suggestion
  SUGGESTION="a design-system component from style.rs"
  if echo "$vline" | grep -qF "egui::Button::new("; then
    SUGGESTION="icon_btn / small_action_btn / pill_button / big_action_btn from style.rs"
  elif echo "$vline" | grep -qF "egui::TextEdit::singleline"; then
    SUGGESTION="text_input_field or numeric_input_field from style.rs"
  elif echo "$vline" | grep -qF "Color32::from_rgb("; then
    SUGGESTION="a token color via current().accent / current().bull / etc. from style.rs"
  fi

  echo "DESIGN-SYSTEM VIOLATION: $vline"
  echo "  -> Use $SUGGESTION instead."
  echo ""
done <<< "$NEW_VIOLATIONS"

echo "------------------------------------------------------------"
COUNT=$(echo "$NEW_VIOLATIONS" | grep -c '.' || true)
echo "  $COUNT new violation(s) found."
echo "  See docs/DESIGN_SYSTEM.md for guidance."
echo "============================================================"
echo ""

exit 1
