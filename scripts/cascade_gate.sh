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

# ── Rule 3 ────────────────────────────────────────────────────────────────────
# Every widget that paints CHROME must either resolve a recipe key or appear on
# this exclusion list with a reason. Without this, the layer silently stops
# growing the moment someone adds a widget — which is how it stalled at 6.
#
# Exclusions are NOT "not done yet". Each is a deliberate answer to "what would
# a style have to say about this?":
#
#   data-viz — paints DATA, not chrome. A style has no opinion on a sparkline's
#   bars or a heatmap's cells; a key there would be dead data.
#     guild_avatar_grid heatmap_grid opacity_picker pane_grid risk_reward_bar
#     skeleton sparkline
#
#   modal            — paints only the SCRIM (full viewport, radius 0). Its
#                      panel chrome comes from PopupFrame.
#   shadow           — a shadow-painting utility, not a surface.
#   theme_preview_card — renders a PREVIEW OF ANOTHER THEME. It must NOT follow
#                      the ambient recipes or every swatch would look like the
#                      active style instead of the one being previewed.
EXCLUDE="guild_avatar_grid heatmap_grid modal opacity_picker pane_grid risk_reward_bar shadow skeleton sparkline theme_preview_card"

unwired=""
for f in $W/*.rs; do
  b=$(basename "$f" .rs)
  case " theme ctx tokens mod " in *" $b "*) continue;; esac
  echo "$EXCLUDE" | tr ' ' '
' | grep -qx "$b" && continue
  grep -q "get_ambient_recipes\|resolve_control_chrome\|resolve_sx\|resolve_cached" "$f" && continue
  if grep -q "rect_filled\|rect_stroke" "$f"; then unwired="$unwired $b"; fi
done
if [ -n "$unwired" ]; then
  echo "CASCADE GATE FAILED — widget paints chrome but resolves no recipe key:"
  for u in $unwired; do echo "    $u"; done
  echo "Give it a key (reuse a sibling's where one exists), or add it to"
  echo "EXCLUDE in this script WITH a reason."
  fail=1
fi

[ $fail -eq 0 ] && echo "cascade gate OK" || exit 1
