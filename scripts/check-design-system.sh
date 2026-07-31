#!/usr/bin/env bash
# check-design-system.sh
#
# Ratchet: fails if code introduces MORE raw/off-token UI primitives than the
# recorded baseline. Existing violations are tolerated per-file until migrated;
# the ratchet only ever tightens (a file that improves updates its budget down
# on the next `--update`, and can never regress past it).
#
#   ./scripts/check-design-system.sh            # check (CI mode)
#   ./scripts/check-design-system.sh --update   # re-record the baseline
#
# WHY COUNTS, NOT LINES: the previous version stored `path:line:content` and
# compared exact lines, so ANY edit above a violation shifted its line number
# and reported it as a brand-new violation. A gate that cries wolf on every
# refactor gets its baseline blindly regenerated, which makes it decorative.
# Per-file counts are stable under refactoring and still block new drift.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE_FILE="$REPO_ROOT/scripts/.design-system-baseline.txt"
SRC_DIR="$REPO_ROOT/src-tauri/src"
MODE="${1:-check}"

# Patterns to detect.
#
# The first three guard raw egui primitives. The rest were added after the
# 2026-07-31 UI audit, which found the drift classes this gate was blind to
# (see docs/UI_AUDIT_2026-07-31.md):
#
#  * literal FONT SIZES — ~70% of the app had drifted onto 9/11px because every
#    call site picked its own size. Use font_xs()/font_sm()/font_md()/… or a
#    TextStyle tier (which now cascades through egui's text_styles table).
#  * named COLOR CONSTANTS — Color32::WHITE/BLACK/GRAY are theme-blind and break
#    on the 5 light palettes. Use the theme (t.text / t.dim / t.bull / …).
#  * literal-channel rgba — use `color_alpha(t.<role>, a)`.
#  * `CornerRadius::same(<literal>)` — use the per-style radius tokens
#    (radius_xs/sm/md/lg, r_pill) or `current().region_radius`.
#
# NOT matched (deliberate): the positional rounding arg in
# `rect_filled(rect, 4.0, col)`. It was the single biggest radius leak (155
# sites at audit time), but it is indistinguishable by grep from any other
# 3-argument call, so a regex gate would be all false positives. That one needs
# a clippy lint or an AST pass — recorded as follow-up rather than faked here.
PATTERNS=(
  "egui::Button::new("
  "egui::TextEdit::singleline"
  "Color32::from_rgb("
  "Color32::from_rgba_unmultiplied("
  "Color32::WHITE"
  "Color32::BLACK"
  "Color32::GRAY"
  "Color32::LIGHT_GRAY"
  "Color32::DARK_GRAY"
  "FontId::proportional("
  "FontId::monospace("
  "FontId::new("
  "CornerRadius::same("
)

# Files that DEFINE the design system (they must use raw primitives to build
# the tokens everything else consumes) plus dev-only surfaces.
ALLOWED_BASENAMES=(
  "style.rs"                 # token definitions (ui_kit + chart layer)
  "theme.rs"                 # ComponentTheme defaults / PortableTheme
  "theme_impl.rs"            # chart Theme -> ComponentTheme bridge
  "theme_adapter.rs"         # ColorScheme -> Theme adapter
  "builtin.rs"               # palette + style-system literals
  "color_scheme.rs"
  "design_tokens.rs"
  "design_inspector.rs"      # the token editor itself
  "theme_studio.rs"          # live theme editor
  "widget_gallery.rs"        # component demo surface
  "color_picker.rs"          # an RGB picker, by definition
  "recipe_spec.rs"
)

# Build a basename filter regex. NOTE: we deliberately do NOT use grep's
# --exclude — it silently fails to filter under the MSYS/Git-Bash grep 3.0 on
# this machine (verified: `--exclude="style.rs"` still returned style.rs hits),
# which would have quietly baselined every token-definition file as a violation.
# Filtering in the pipeline is portable and testable.
ALLOWED_RE="/($(IFS='|'; echo "${ALLOWED_BASENAMES[*]%.rs}"))\.rs$"
EXCLUDE_DIR_ARGS=(
  --exclude-dir="apex-terminal-designmode"
  --exclude-dir=".git"
  --exclude-dir="playground"   # standalone demo binary, not the product
)

# ── Collect per-file violation counts ───────────────────────────────────────
collect() {
  for pat in "${PATTERNS[@]}"; do
    grep -rn \
      "${EXCLUDE_DIR_ARGS[@]}" \
      --include="*.rs" \
      -F "$pat" \
      "$SRC_DIR" 2>/dev/null || true
  done \
  | grep -v -E ':[0-9]+:[[:space:]]*//' \
  | sed "s|^$REPO_ROOT/||" \
  | cut -d: -f1 \
  | grep -v -E "$ALLOWED_RE" \
  | grep -v -E '(^|/)tests?/' \
  | sort | uniq -c \
  | awk '{printf "%s %s\n", $1, $2}' \
  | sort -k2
}

CURRENT=$(mktemp); trap 'rm -f "$CURRENT"' EXIT
collect > "$CURRENT"

if [[ "$MODE" == "--update" ]]; then
  cp "$CURRENT" "$BASELINE_FILE"
  echo "Baseline updated: $(wc -l < "$BASELINE_FILE") files, $(awk '{s+=$1} END{print s+0}' "$BASELINE_FILE") violations."
  exit 0
fi

if [[ ! -f "$BASELINE_FILE" ]]; then
  echo "ERROR: no baseline at $BASELINE_FILE — run: $0 --update"
  exit 1
fi

# ── Compare: fail only on INCREASES ─────────────────────────────────────────
FAIL=0
REGRESSED=""
while read -r cur_count cur_file; do
  [[ -z "${cur_file:-}" ]] && continue
  base_count=$(awk -v f="$cur_file" '$2==f {print $1; found=1} END{if(!found) print 0}' "$BASELINE_FILE")
  if (( cur_count > base_count )); then
    REGRESSED+="  $cur_file: $base_count -> $cur_count (+$((cur_count - base_count)))"$'\n'
    FAIL=1
  fi
done < "$CURRENT"

CUR_TOTAL=$(awk '{s+=$1} END{print s+0}' "$CURRENT")
BASE_TOTAL=$(awk '{s+=$1} END{print s+0}' "$BASELINE_FILE")

if (( FAIL )); then
  echo "DESIGN-SYSTEM RATCHET FAILED — new off-token UI primitives:"
  echo ""
  printf '%s' "$REGRESSED"
  echo ""
  echo "Total: $BASE_TOTAL -> $CUR_TOTAL"
  echo ""
  echo "Use the design system instead of raw primitives:"
  echo "  colors  -> the theme (t.text / t.dim / t.accent / t.bull / t.bear),"
  echo "             or color_alpha(t.<role>, a) for translucency"
  echo "  fonts   -> font_xs()/font_sm()/font_md()/… or a TextStyle tier"
  echo "  radii   -> radius_xs/sm/md/lg(), r_pill(), current().region_radius"
  echo "  widgets -> ui_kit::widgets::{Button, Input, PanelListRow, …}"
  echo ""
  echo "If a violation is genuinely justified (a brand colour, a token"
  echo "definition), add the file to ALLOWED_BASENAMES with a comment saying why."
  echo "Do NOT blanket-regenerate the baseline to silence a real regression."
  exit 1
fi

if (( CUR_TOTAL < BASE_TOTAL )); then
  echo "Design-system ratchet OK — improved: $BASE_TOTAL -> $CUR_TOTAL."
  echo "Run '$0 --update' to lock in the gain."
else
  echo "Design-system ratchet OK — $CUR_TOTAL violations (baseline $BASE_TOTAL)."
fi
exit 0
