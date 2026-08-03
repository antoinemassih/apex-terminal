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
# NOTE: the key set includes DOTLESS keys (`card`, `tag`, `toolnav`, `kbd`).
# The first pattern here required a dot and silently skipped them — it
# under-reported AND would not have noticed those keys disappearing. Keys are
# matched in tuple position (indented, quoted, comma-terminated), and the test
# module is excluded so assertions never inflate the count.
RECIPE_SRC="$SRC/design_system/builtin_recipes.rs"
TEST_LINE=$(grep -n '^#\[cfg(test)\]' "$RECIPE_SRC" 2>/dev/null | tail -1 | cut -d: -f1)
: "${TEST_LINE:=999999}"
AUTHORED_BODY=$(head -n $((TEST_LINE - 1)) "$RECIPE_SRC" 2>/dev/null)
KEY_LINES=$(printf '%s\n' "$AUTHORED_BODY" | grep -oE '^[[:space:]]{8,}"[a-z][a-z_.]*",')

KEYS_AUTHORED=$(printf '%s\n' "$KEY_LINES" | sed 's/[[:space:]]*//;s/,$//' \
    | sort -u | wc -l | tr -d ' ')

# ── Metric 4: TOTAL authored declarations ────────────────────────────────────
# Distinct-key count alone is blind to BREADTH: authoring an existing key for
# three more themes is real adoption but leaves metric 2 unchanged (exactly
# what happened in M3.4b — 66 -> 80 declarations, 23 distinct keys throughout).
DECLARATIONS=$(printf '%s\n' "$KEY_LINES" | grep -c . | tr -d ' ')

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
authored_declarations=$DECLARATIONS
EOF
    echo "Recipe-adoption floors updated:"
    echo "  widgets consulting recipes : $WIDGETS_CONSULTING"
    echo "  distinct keys authored     : $KEYS_AUTHORED"
    echo "  styles with recipe data    : $STYLES_AUTHORED"
    echo "  authored declarations      : $DECLARATIONS"
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
FLOOR_DECLS=$(grep '^authored_declarations=' "$BASELINE" | cut -d= -f2)
: "${FLOOR_DECLS:=0}"

FAIL=0
echo "recipe_adoption_gate:"
printf '  widgets consulting recipes : %-4s (floor %s)\n' "$WIDGETS_CONSULTING" "$FLOOR_WIDGETS"
printf '  distinct keys authored     : %-4s (floor %s)\n' "$KEYS_AUTHORED" "$FLOOR_KEYS"
printf '  styles with recipe data    : %-4s (floor %s)\n' "$STYLES_AUTHORED" "$FLOOR_STYLES"
printf '  authored declarations      : %-4s (floor %s)\n' "$DECLARATIONS" "$FLOOR_DECLS"

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
if (( DECLARATIONS < FLOOR_DECLS )); then
    echo "FAIL: fewer authored declarations ($DECLARATIONS < $FLOOR_DECLS)."
    echo "      A key removed from a theme un-themes that component there."
    FAIL=1
fi

if (( FAIL == 0 )); then
    echo "OK — adoption at or above every floor."
    echo "     Raise them after a genuine gain: bash scripts/recipe_adoption_gate.sh --update"
fi
exit $FAIL
