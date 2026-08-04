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
# FIRST test module, not the last. `tail -1` assumed exactly one test
# module and it sat at the end of the file; adding a SECOND one put the
# earlier module's expected-registry array back inside the "authored"
# body, inflating the key count with test data and reporting keys as
# authored-but-dead that no theme authors at all (`panel.footer`,
# `toast.*`, `kbd`). Everything from the first test module on is test.
TEST_LINE=$(grep -n '^#\[cfg(test)\]' "$RECIPE_SRC" 2>/dev/null | head -1 | cut -d: -f1)
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

# -- Dead recipe data: keys AUTHORED by a theme that no widget ever asks for --
#
# The inverse of the M3 defect. There, a widget was handed a RecipeSet and
# ignored it. Here the data exists -- a designer wrote it per theme -- and no
# resolve("key") call site consumes it, so it is theme intent that can never
# reach a pixel. HALF the registry was in this state when this check was
# written: card, card.floating, panel.header, panel.footer, tab.line,
# toast.danger/success/warn, drag.handle, nav.cluster.
#
# Counted, not banned: wiring a widget is real work and a key may legitimately
# land before its consumer. The floor stops it getting WORSE.
AUTHORED_KEYS=$(printf '%s\n' "$KEY_LINES" | sed -E 's/.*"([^"]+)".*/\1/' | sort -u)
CONSUMED=0
DEAD_LIST=""
for k in $AUTHORED_KEYS; do
    # Match the key literal ANYWHERE in the widget layer, not just inside a
    # `resolve("...")`. Button resolves via `recipe_key_for(variant)`, which
    # returns the string from a match arm -- a `resolve(` -only grep scored
    # those as dead and would have sent someone rewiring widgets that already
    # work.
    # A state variant (`tab.pill.active`, `row.list.hover`) is reached through
    # its BASE key's delta, never looked up on its own -- so credit the base.
    BASE=$(printf '%s' "$k" | sed -E 's/\.(active|hover|selected|fill|disabled)$//')
    if grep -rqF "\"$k\"" "$SRC/ui_kit" "$SRC/chart" 2>/dev/null        || grep -rqF "\"$BASE\"" "$SRC/ui_kit" "$SRC/chart" 2>/dev/null; then
        CONSUMED=$((CONSUMED + 1))
    else
        DEAD_LIST="$DEAD_LIST $k"
    fi
done

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
keys_with_consumer=$CONSUMED
EOF
    echo "Recipe-adoption floors updated:"
    echo "  widgets consulting recipes : $WIDGETS_CONSULTING"
    echo "  distinct keys authored     : $KEYS_AUTHORED"
    echo "  styles with recipe data    : $STYLES_AUTHORED"
    echo "  authored declarations      : $DECLARATIONS"
    echo "  keys with a consumer       : $CONSUMED"
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
FLOOR_CONSUMED=$(grep '^keys_with_consumer=' "$BASELINE" | cut -d= -f2)
FLOOR_CONSUMED=${FLOOR_CONSUMED:-0}
: "${FLOOR_DECLS:=0}"

FAIL=0
echo "recipe_adoption_gate:"
printf '  widgets consulting recipes : %-4s (floor %s)\n' "$WIDGETS_CONSULTING" "$FLOOR_WIDGETS"
printf '  distinct keys authored     : %-4s (floor %s)\n' "$KEYS_AUTHORED" "$FLOOR_KEYS"
printf '  styles with recipe data    : %-4s (floor %s)\n' "$STYLES_AUTHORED" "$FLOOR_STYLES"
printf '  authored declarations      : %-4s (floor %s)\n' "$DECLARATIONS" "$FLOOR_DECLS"
printf '  keys with a consumer       : %-4s (floor %s)
' "$CONSUMED" "$FLOOR_CONSUMED"
if [ -n "$DEAD_LIST" ]; then printf '  DEAD (authored, never resolved):%s
' "$DEAD_LIST"; fi

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

if (( CONSUMED < FLOOR_CONSUMED )); then
    echo "FAIL: fewer authored recipe keys have a consumer ($CONSUMED < $FLOOR_CONSUMED)."
    echo "      A key no widget resolves is theme intent that cannot reach a pixel."
    FAIL=1
fi

if (( FAIL == 0 )); then
    echo "OK — adoption at or above every floor."
    echo "     Raise them after a genuine gain: bash scripts/recipe_adoption_gate.sh --update"
fi
exit $FAIL
