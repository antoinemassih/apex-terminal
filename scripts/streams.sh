#!/usr/bin/env bash
# At-a-glance status of every parallel stream: which worktree, branch, how far
# ahead/behind main, uncommitted changes, and the tip commit.
#
# Usage: scripts/streams.sh
set -uo pipefail

root="$(git rev-parse --show-toplevel)"
git -C "$root" fetch origin --quiet 2>/dev/null || true

printf "%-26s %-24s %-12s %-7s %s\n" "BRANCH" "WORKTREE" "ahead/behind" "dirty" "TIP"
printf "%-26s %-24s %-12s %-7s %s\n" "------" "--------" "------------" "-----" "---"

wt=""
git worktree list --porcelain | while IFS= read -r line; do
  case "$line" in
    worktree\ *) wt="${line#worktree }" ;;
    branch\ *)
      ref="${line#branch }"; br="${ref#refs/heads/}"
      ab=$(git -C "$root" rev-list --left-right --count "main...$br" 2>/dev/null \
            | awk '{print "+"$2"/-"$1}')
      dirty=$(git -C "$wt" status --porcelain 2>/dev/null | grep -vc "migration-patches")
      tip=$(git -C "$wt" log -1 --format='%h %s' 2>/dev/null | cut -c1-46)
      mark=""; [ "$dirty" != "0" ] && mark="*"
      printf "%-26s %-24s %-12s %-7s %s\n" "$br" "$(basename "$wt")" "${ab:- }" "${dirty}${mark}" "$tip"
      ;;
    detached) printf "%-26s %-24s %-12s %-7s %s\n" "(detached)" "$(basename "$wt")" "" "" "" ;;
  esac
done

echo ""
echo "ahead/behind = commits this branch has (+) / is missing (-) vs local main"
echo "dirty        = uncommitted files (* = needs commit/stash before integrate)"
