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
  # gamma_multiply with a literal factor darkens/brightens a colour outside the
  # token system — the result tracks NO theme role. Use color_alpha / tint or a
  # dedicated token instead. Fixed-string on the "0." prefix catches the literal
  # forms (gamma_multiply(0.5) etc.) without matching variable factors.
  "gamma_multiply(0."
  # Literal stroke WIDTHS: Stroke::new(0.5/1.0/1.5/2.0/3.0, …) hardcodes line
  # weight per call site instead of using the stroke-width tokens. grep -F, so
  # each common literal is its own fixed string (regex would need -E and would
  # false-positive on computed widths).
  "Stroke::new(0.5,"
  "Stroke::new(1.0,"
  "Stroke::new(1.5,"
  "Stroke::new(2.0,"
  "Stroke::new(3.0,"
)

# ── Regex patterns (grep -E) ─────────────────────────────────────────────────
#
# SPACE. Every pattern above governs colour, type, radius or stroke weight.
# Nothing governed layout — and a measurement during the 2026-08 audit found
# 997 hardcoded spacing/positioning literals across 86 files. That is the
# largest ungoverned surface left in the design system, and it is the one the
# eye reads as "this screen was assembled rather than laid out": every gap is
# individually plausible and no two agree.
#
# These need -E rather than -F because the offending number varies; as fixed
# strings each would need one entry per literal.
#
#   .left() + 6.0        positioning by arithmetic on a rect edge
#   add_space(8.0)       vertical rhythm chosen per call site
#   (`.shrink(N)` is deliberately NOT matched: `Rect::shrink` is a pixel
#    inset but `Item::shrink` is CSS flex-shrink, and a regex cannot tell them
#    apart. 17 of the 29 matches were flex-shrink factors on layout Items —
#    correct, design-system code. A gate that counts the right answer as a
#    violation teaches people to avoid the engine it is meant to promote.)
#   Margin::same(6)      frame padding outside the token system
#
# The tokens already exist (gap_2xs..gap_3xl and the density ladder); these
# call sites simply predate them. Anything computed FROM a token call does not
# match, so migrating a site removes it from the count.
#
# HOW TO READ THE LAYOUT PORTION OF THIS NUMBER. Of 746 at the 2026-08 audit,
# 443 (59%) are in chart PAINTING — core.rs, chart_widgets.rs, gpu.rs — where
# `.left() + 6.0` is a candle body, a gauge tick or a sparkline vertex. That is
# data geometry, not chrome layout, and running it through a flexbox solver
# would be slower and wrong. Those are not a migration backlog.
#
# The layout engine addresses the other 303: panels and chrome (109), ui_kit
# widgets (86), lists and inputs (43), toolbar (38), misc (27). Driving the
# total to zero is not the goal and would mean mangling the chart renderer to
# satisfy a lint.
REGEX_PATTERNS=(
  # ALPHA. `color_alpha(t.text, 60)` and `tint(t, Tone::Dim, 160)` pass an
  # opacity as a bare number.
  #
  # The original note here claimed 354 of 634 literals were off-ladder and that
  # 160/180 formed "an unofficial second ladder". That count was wrong twice
  # over: it read the ladder with a regex that missed `whisper` (25) and `hint`
  # (30) because those are set via function calls rather than literals, and it
  # pooled chrome with CHART PAINTING, where `color_alpha(base, 160)` is a
  # candle body and `220` a wick — data geometry, not a chrome tier. See
  # AT-150, now closed: most off-ladder chrome values were within +-2 of a rung
  # and snapped; only 160 and 180 were a real gap, now `dense` and `near_solid`.
  #
  # Still matched whether or not the value is on-ladder, because `alpha_dim()`
  # says what it means and `60` does not, and a literal stops tracking the
  # moment a style re-pitches the ramp.
  "(color_alpha|tint)\([^)]*,[[:space:]]*[0-9]{1,3}[[:space:]]*\)"
  "\.(left|right|top|bottom|center_x|center_y)\(\)[[:space:]]*[-+][[:space:]]*[0-9]+\.[0-9]+"
  "add_space\([[:space:]]*[0-9]+\.[0-9]+[[:space:]]*\)"
  "Margin::(same|symmetric)\([[:space:]]*[0-9]+"
  "item_spacing[[:space:]]*=[[:space:]]*egui::vec2\([[:space:]]*[0-9]+\.[0-9]+"
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
  "text_style.rs"            # DEFINES the 16 type tiers. font_id()/font_id_at()
                             # must build FontIds — that is the whole point, and
                             # centralising construction HERE is what keeps call
                             # sites from doing it. Same rationale as scale.rs.
  "scale.rs"                 # DEFINES the typed Space/Radius/Weight/Level scales
                             # — Radius::cr() must build a CornerRadius, that is
                             # its whole job. Exempt for the same reason style.rs
                             # is: it is the front door, not a consumer.
  "inspect.rs"               # Ctrl+Shift+D debug OVERLAY -- its highlight must
                             # NOT follow the theme; dev chrome has to contrast
                             # with whatever palette is active to stay usable.
  "design_inspector.rs"      # the token editor itself
  "theme_studio.rs"          # live theme editor
  "widget_gallery.rs"        # component demo surface
  "color_picker.rs"          # an RGB picker, by definition
  "recipe_spec.rs"
  "tps_overlay.rs"           # fake-Excel boss-key overlay — theme-blindness IS the feature
  "bug_anchor.rs"            # Ctrl+Shift+I dev bug-reporter overlay; same rationale as inspect.rs
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
# Drop hits inside a `#[cfg(test)]` module. Test fixtures legitimately use
# literal colours/fonts (`Color32::from_rgb(1, 2, 3)` as a sentinel), and
# counting those as drift trains people to ignore the gate — worse, "fixing"
# one by reaching for a token would make the test depend on the very system it
# exercises.
#
# This used to be an awk filter that cut each file at its FIRST
# `^\s*#\[cfg(test)\]` line, on the stated assumption that "test modules sit
# at the END of a file". When that does not hold it fails in the DANGEROUS
# direction: production code below a mid-file test module stopped being counted
# at all. It hid 17 real violations, 11 of them in
# `chart/renderer/ui/style.rs`, whose test module sits above
# `style_system_to_style_settings`. The failure was also unbounded — any file
# that later gained a mid-file test module would silently drop out of the count
# below that point and the ratchet would report an improvement for it.
#
# Brace matching removes the assumption. See `dev/strip_test_hits.py`.
drop_test_module_hits() {
  python dev/strip_test_hits.py
}

collect() {
  {
    for pat in "${PATTERNS[@]}"; do
      grep -rn \
        "${EXCLUDE_DIR_ARGS[@]}" \
        --include="*.rs" \
        -F "$pat" \
        "$SRC_DIR" 2>/dev/null || true
    done
    for pat in "${REGEX_PATTERNS[@]}"; do
      grep -rnE \
        "${EXCLUDE_DIR_ARGS[@]}" \
        --include="*.rs" \
        "$pat" \
        "$SRC_DIR" 2>/dev/null || true
    done
  } \
  | grep -v -E ':[0-9]+:[[:space:]]*//' \
  | drop_test_module_hits \
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
