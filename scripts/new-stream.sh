#!/usr/bin/env bash
# Create an isolated worktree + branch for a parallel work stream.
#
# Each agent/session gets its OWN directory, HEAD, index and target/ — so no
# session can flip another's branch or stomp its working tree. Worktrees share
# the object store, so converging later (scripts/integrate.sh) stays local/fast.
#
# Usage:  scripts/new-stream.sh <stream-name> [base-branch]
# Example: scripts/new-stream.sh figma-export
#          scripts/new-stream.sh dom-ladder main
set -euo pipefail

name="${1:?usage: new-stream.sh <stream-name> [base-branch]}"
base="${2:-main}"

slug="$(printf '%s' "$name" | tr '[:upper:] ' '[:lower:]-' | tr -cd 'a-z0-9-')"
branch="stream/$slug"
root="$(git rev-parse --show-toplevel)"
wt="$(dirname "$root")/apex-stream-$slug"

git -C "$root" fetch origin --quiet 2>/dev/null || true

# Prefer the freshest base: origin/<base> if it exists, else the local ref.
if git -C "$root" rev-parse --verify --quiet "origin/$base" >/dev/null 2>&1; then
  baseref="origin/$base"
else
  baseref="$base"
fi

if [ -e "$wt" ]; then
  echo "✗ worktree path already exists: $wt" >&2; exit 1
fi

if git -C "$root" show-ref --verify --quiet "refs/heads/$branch"; then
  echo "• branch $branch exists — attaching a worktree to it"
  git -C "$root" worktree add "$wt" "$branch"
else
  git -C "$root" worktree add "$wt" -b "$branch" "$baseref"
fi

cat <<EOF

✓ stream ready
  branch:   $branch   (based on $baseref)
  worktree: $wt

Next:
  cd "$wt"
  # ... work, commit often (small commits) ...
  git push -u origin $branch          # push early for durability + visibility

When this stream is done, from the MAIN checkout run:
  scripts/integrate.sh $branch
EOF
