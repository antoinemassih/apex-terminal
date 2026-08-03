#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# recipe_adoption_gate.sh — M3.6
#
# The architecture audit's finding: "There is no metric anywhere that measures
# actual Sx or RecipeSet adoption — which is precisely why the layer sat at 19
# sites while the codebase added 713 hand-painted boxes."
#
# `sx_ratchet.sh` counts a LEGACY colour pattern (a ceiling that must fall).
# This gate is its opposite: FLOORS that may only RISE. It measures whether the
# design system is actually being consumed:
#
#   1. widgets consulting the recipe layer   (get_ambient_recipes / recipes())
#   2. registered recipe keys actually authored in builtin_recipes.rs
#   3. styles shipping authored recipe data
#
# A floor that never moves is the signal the audit wanted: it makes "the layer
# is dormant" impossible to miss again.
#
# Usage:  bash scripts/recipe_adoption_gate.sh [--update]
# Exit 0 = at or above every floor. Exit 1 = adoption REGRESSED.
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2
SRC="src-tauri/src"
BASELINE="scripts/.recipe-adoption-baseline.txt"

# ── Metric 1: widgets that consult the recipe layer ──────────────────────────
# A widget "consults recipes" when it resolves a key through the ambient set.
WIDGETS_CONSULTING=$(grep -rl --include=*.rs \
    -e 'get_ambient_recipes' -e '\.recipes()' \
    "$SRC/ui_kit/widgets" 2>/dev/null | wc -l | tr -d ' ')

# ── Metric 2: distinct registered keys authored across the six styles ────────
KEYS_AUTHORED=$(grep -o '"[a-z][a-z_]*\.[a-z.]*"' \
    "$SRC/design_system/builtin_recipes.rs" 2>/dev/null \
    | sort -u | wc -l | tr -d ' ')

# ── Metric 3: styles shipping authored recipe data ───────────────────────────
# Counts the match arms in builtin_recipes() that return a real set.
STYLES_AUTHORED=$(grep -cE '^\s*"[a-z]+" => [a-z]+\(\),' \
    "$SRC/design_system/builtin_recipes.rs" 2>/dev/null | tr -d ' ')

if [[ "${1:-}" == "--update" ]]; then
    cat > "$BASELINE" <<EOF
# recipe-adoption FLOORS — may only RISE (see scripts/recipe_adoption_gate.sh)
# Recorded: $(date -u +%Y-%m-%dT%H:%MZ)
widgets_consulting_recipes=$WIDGETS_CONSULTING
registered_keys_authored=$KEYS_AUTHORED
styles_with_authored_recipes=$STYLES_AUTHORED
EOF
    echo "Recipe-adoption floors updated:"
    echo "  widgets consulting recipes : $WIDGETS_CONSULTING"
    echo "  registered keys authored   : $KEYS_AUTHORED"
    echo "  styles with recipe data    : $STYLES_AUTHORED"
    exit 0
fi

if [[ ! -f "$BASELINE" ]]; then
    echo "No baseline at $BASELINE — run: bash scripts/recipe_adoption_gate.sh --update"
    exit 2
fi

# shellcheck disable=SC1090
FLOOR_WIDGETS=$(grep '^widgets_consulting_recipes=' "$BASELINE" | cut -d= -f2)
FLOOR_KEYS=$(grep '^registered_keys_authored=' "$BASELINE" | cut -d= -f2)
FLOOR_STYLES=$(grep '^styles_with_authored_recipes=' "$BASELINE" | cut -d= -f2)

FAIL=0
echo "recipe_adoption_gate:"
printf '  widgets consulting recipes : %-4s (floor %s)\n' "$WIDGETS_CONSULTING" "$FLOOR_WIDGETS"
printf '  registered keys authored   : %-4s (floor %s)\n' "$KEYS_AUTHORED" "$FLOOR_KEYS"
printf '  styles with recipe data    : %-4s (floor %s)\n' "$STYLES_AUTHORED" "$FLOOR_STYLES"

if (( WIDGETS_CONSULTING < FLOOR_WIDGETS )); then
    echo "FAIL: fewer widgets consult the recipe layer ($WIDGETS_CONSULTING < $FLOOR_WIDGETS)."
    echo "      A widget that stopped resolving recipes silently un-themes itself."
    FAIL=1
fi
if (( KEYS_AUTHORED < FLOOR_KEYS )); then
    echo "FAIL: fewer registered keys are authored ($KEYS_AUTHORED < $FLOOR_KEYS)."
    FAIL=1
fi
if (( STYLES_AUTHORED < FLOOR_STYLES )); then
    echo "FAIL: fewer styles ship recipe data ($STYLES_AUTHORED < $FLOOR_STYLES)."
    FAIL=1
fi

if (( FAIL == 0 )); then
    echo "OK — adoption at or above every floor."
    echo "     Raise them after a genuine gain: bash scripts/recipe_adoption_gate.sh --update"
fi
exit $FAIL
