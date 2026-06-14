#!/usr/bin/env bash
# Converge parallel work streams into main — safely and repeatably.
#
# Flow:  integrate := main  →  merge each stream/*  →  BUILD  →  (test)  →
#        fast-forward main  →  push  →  (prune merged streams)
#
# A pre-integrate backup branch is always made first, so any step is reversible.
# Conflicting streams are skipped (merge aborted, left untouched) and reported —
# the run never leaves a half-merged tree.
#
# Usage:
#   scripts/integrate.sh                      # auto-detect all local stream/* branches
#   scripts/integrate.sh stream/foo stream/bar
#
# Env toggles:
#   PUSH=0     skip pushing to remotes      (default: push)
#   TEST=1     also run `cargo test --lib`  (default: build only)
#   PRUNE=1    delete+remove merged streams (default: keep them)
#   REMOTES="origin gitea ococo"            (default)
set -uo pipefail

root="$(git rev-parse --show-toplevel)"; cd "$root"
REMOTES="${REMOTES:-origin gitea ococo}"
DO_PUSH="${PUSH:-1}"; DO_TEST="${TEST:-0}"; DO_PRUNE="${PRUNE:-0}"

die(){ echo "✗ $*" >&2; exit 1; }

# ── 0. preconditions ────────────────────────────────────────────────────────
cur="$(git branch --show-current)"
[ "$cur" = "main" ] || die "run from the main checkout (currently on '$cur')"
# Only TRACKED modifications block integration — untracked files (build
# artifacts, scratch json from other sessions) don't affect a merge or build.
[ -z "$(git status --porcelain --untracked-files=no)" ] \
  || die "main has uncommitted TRACKED changes — commit or stash first"

git fetch origin --quiet 2>/dev/null || true
git merge --ff-only origin/main --quiet 2>/dev/null || true

# ── 1. select streams ───────────────────────────────────────────────────────
if [ "$#" -gt 0 ]; then
  streams=("$@")
else
  mapfile -t streams < <(git for-each-ref --format='%(refname:short)' refs/heads/stream 2>/dev/null)
fi
[ "${#streams[@]}" -gt 0 ] || { echo "no stream/* branches to integrate — nothing to do"; exit 0; }
echo "streams to integrate: ${streams[*]}"

# ── 2. safety backup of current main ────────────────────────────────────────
backup="backup/pre-integrate-$(git rev-parse --short main)"
git branch -f "$backup" main >/dev/null
echo "safety backup: $backup -> $(git rev-parse --short main)  (delete later with: git branch -D $backup)"

# ── 3. build integrate from main, merge each stream ─────────────────────────
git checkout -q -B integrate main
git config rerere.enabled true   # remember conflict resolutions across runs

merged=(); skipped=(); already=()
for s in "${streams[@]}"; do
  git show-ref --verify --quiet "refs/heads/$s" || { echo "  ? $s — no such branch, skipping"; skipped+=("$s"); continue; }
  if git merge-base --is-ancestor "$s" integrate; then echo "  = $s already integrated"; already+=("$s"); continue; fi
  if git merge --no-ff --no-edit "$s" >/dev/null 2>&1; then
    echo "  + merged $s"; merged+=("$s")
  else
    echo "  ! CONFLICT merging $s — aborting that merge (resolve it manually, then re-run)"
    git merge --abort 2>/dev/null || true
    skipped+=("$s")
  fi
done

# ── 4. build the combined tree (the real gate) ──────────────────────────────
echo "building integrate (release)…"
# best-effort: release the exe lock if the app is running
( taskkill //F //IM apex-native.exe >/dev/null 2>&1 ) || true
if ! ( cd src-tauri && cargo build --release --bin apex-native ); then
  echo "✗ BUILD FAILED on integrate. main is untouched. integrate left at the failing state for inspection."
  echo "  restore clean integrate with: git checkout main && git branch -f integrate main"
  git checkout -q main
  die "integration build failed"
fi
if [ "$DO_TEST" = "1" ]; then
  echo "running cargo test --lib…"
  ( cd src-tauri && cargo test --lib ) || { git checkout -q main; die "integration tests failed"; }
fi

# ── 5. fast-forward main → integrate ────────────────────────────────────────
git checkout -q main
git merge --ff-only integrate || die "main could not fast-forward to integrate (unexpected divergence)"
echo "✓ main fast-forwarded to $(git rev-parse --short main)"

# ── 6. push ─────────────────────────────────────────────────────────────────
if [ "$DO_PUSH" = "1" ]; then
  for r in $REMOTES; do
    if git push "$r" main integrate >/dev/null 2>&1; then echo "  pushed → $r"
    else echo "  ⚠ push to $r failed (diverged? run: git fetch $r && inspect — do NOT force without checking)"; fi
  done
fi

# ── 7. prune merged streams (opt-in) ────────────────────────────────────────
if [ "$DO_PRUNE" = "1" ] && [ "${#merged[@]}" -gt 0 ]; then
  for s in "${merged[@]}" "${already[@]:-}"; do
    [ -n "$s" ] || continue
    # remove its worktree if one is attached
    wtp=$(git worktree list --porcelain | awk -v b="refs/heads/$s" '/^worktree /{w=$2} /^branch /{if($2==b) print w}')
    [ -n "$wtp" ] && git worktree remove --force "$wtp" 2>/dev/null && echo "  removed worktree $(basename "$wtp")"
    git branch -d "$s" 2>/dev/null && echo "  deleted branch $s"
    for r in $REMOTES; do git push "$r" --delete "$s" >/dev/null 2>&1 && echo "    deleted $r/$s"; done
  done
  git worktree prune
fi

# ── summary ─────────────────────────────────────────────────────────────────
echo ""
echo "════════ integrate summary ════════"
echo "  merged:        ${merged[*]:-none}"
echo "  already in:    ${already[*]:-none}"
echo "  skipped:       ${skipped[*]:-none}"
echo "  main is now:   $(git rev-parse --short main)  $(git log -1 --format=%s)"
echo "  backup branch: $backup"
[ "${#skipped[@]}" -gt 0 ] && echo "  ⚠ resolve skipped streams manually, then re-run."
exit 0
