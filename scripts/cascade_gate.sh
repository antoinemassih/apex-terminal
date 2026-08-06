#!/usr/bin/env bash
# cascade_gate.sh — the StyleCtx cascade must stay honest and keep growing.
#
# Two rules, both learned the hard way:
#
#  1. NO `StyleCtx::from_theme` inside a `show(ui, theme)` wrapper.
#     `from_theme` fills the recipe set with `empty_recipe_arc()`. A `show`
#     wrapper has a `ui` and can therefore reach the ambient set, so using the
#     shim there makes `StyleCtx` advertise a `recipes()` accessor that returns
#     nothing on the path every call site takes. Use `StyleCtx::from_ui`.
#
#  2. Adoption ratchets. `show_ctx` widgets and recipe-consuming widgets may
#     only go UP. Floors live in scripts/.cascade_floors.
set -uo pipefail
cd "$(dirname "$0")/.."
W=src-tauri/src/ui_kit/widgets
FLOORS=scripts/.cascade_floors

fail=0

# ── Rule 1 ────────────────────────────────────────────────────────────────────
bad=$(grep -rn "StyleCtx::from_theme" $W/*.rs 2>/dev/null \
      | grep -v "ctx.rs" | grep -v ':[0-9]*: *//' || true)
if [ -n "$bad" ]; then
  echo "CASCADE GATE FAILED — from_theme (EMPTY recipes) used in a widget:"
  echo "$bad"
  echo "Use StyleCtx::from_ui(theme, ui) — a show() wrapper has the Ui."
  fail=1
fi

# ── Rule 2 ────────────────────────────────────────────────────────────────────
ctx_n=$(grep -rl "fn show_ctx" $W/*.rs 2>/dev/null | wc -l | tr -d ' ')
rec_n=$(grep -rl "get_ambient_recipes\|recipes()\.resolve\|resolve_cached\|resolve_control_chrome" $W/*.rs 2>/dev/null | grep -v "/theme.rs\|/ctx.rs" | wc -l | tr -d ' ')

if [ "${1:-}" = "--update" ]; then
  printf "show_ctx=%s\nrecipes=%s\n" "$ctx_n" "$rec_n" > $FLOORS
  echo "cascade floors updated: show_ctx=$ctx_n recipes=$rec_n"
  exit 0
fi

ctx_floor=$(grep '^show_ctx=' $FLOORS 2>/dev/null | cut -d= -f2); ctx_floor=${ctx_floor:-0}
rec_floor=$(grep '^recipes='  $FLOORS 2>/dev/null | cut -d= -f2); rec_floor=${rec_floor:-0}

printf "  widgets with show_ctx      : %-4s (floor %s)\n" "$ctx_n" "$ctx_floor"
printf "  widgets consuming recipes  : %-4s (floor %s)\n" "$rec_n" "$rec_floor"

[ "$ctx_n" -lt "$ctx_floor" ] && { echo "REGRESSION: show_ctx $ctx_n < $ctx_floor"; fail=1; }
[ "$rec_n" -lt "$rec_floor" ] && { echo "REGRESSION: recipes $rec_n < $rec_floor"; fail=1; }

[ $fail -eq 0 ] && echo "cascade gate OK" || exit 1
