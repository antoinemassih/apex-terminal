# Parallel Work — fan out, converge cleanly

Multiple agent/dev sessions on this app kept causing trouble (HEAD flipping
mid-task, phantom uncommitted files, stale divergent remotes, 30 branches / 20
worktrees of debris). **Root cause: sessions sharing one git checkout.** Git has
exactly one HEAD, one index, one working tree per checkout — two writers race on
all three.

This is the workflow + tooling that fixes it. **One rule above all:**

> **One session → one worktree → one `stream/*` branch.** Never two sessions in
> the same directory or on the same branch.

---

## Fan out (starting a stream)

```bash
scripts/new-stream.sh <name>          # e.g. scripts/new-stream.sh dom-ladder
```
Creates `stream/<name>` from latest `origin/main` and a dedicated worktree at
`../apex-stream-<name>` with its **own HEAD, index, and `target/`**. The agent
works only in that directory. Branch-flips and exe-lock contention disappear
because nothing is shared but the object store.

Then, in that worktree:
- **Commit often, in small commits.** Never leave WIP uncommitted — a loose
  working tree is what nearly lost the leak-diagnostics work.
- **Push early:** `git push -u origin stream/<name>`. Durability + everyone can
  see what you're doing. (A 2-month-old unpushed fork is how `gitea/main`
  silently diverged by 281 commits.)

## Partition before you fan out
Conflicts come from two streams editing the **same file**. The figma + auto-charting
stacks merged with *zero* conflicts because they owned different subsystems.
Before splitting work, assign ownership, e.g.:
- charting stream owns `chart/renderer/render/pane/`
- design stream owns `design_system/`, `ui_kit/`
- data stream owns `data/feeds/`, `chart/renderer/io/`

Use the coordination tools you already have:
- **squidink** — `ink_list` (who's online), `ink_status` (what you're on),
  `ink_delegate` (hand off a task).
- **whalefin** — `whalefin_claim`, `whalefin_collision`, `whalefin_conflict_check`,
  `whalefin_change_scoped` — detect when two streams are about to touch the same code.

## Check in on everything
```bash
scripts/streams.sh        # every worktree: branch, ahead/behind main, dirty, tip
```

## Converge (one integrator, on a cadence)

From the **main checkout only**, one designated session/human runs:
```bash
scripts/integrate.sh                       # fold in all stream/* branches
scripts/integrate.sh stream/foo stream/bar # or specific ones
```
It does, safely: snapshot `main` to a `backup/…` branch → rebuild `integrate`
from `main` → merge each stream (conflicting ones are **aborted + reported**,
never half-merged) → **release-build the combined tree** (the real gate) →
fast-forward `main` → push to all remotes.

Useful toggles:
```bash
TEST=1  scripts/integrate.sh     # also run cargo test --lib
PRUNE=1 scripts/integrate.sh     # delete + remove worktrees of merged streams
PUSH=0  scripts/integrate.sh     # local only, no remote push
```

**Don't let every session merge into `integrate` ad hoc** — that's how it got a
mystery merge at 21:40. One integrator, one cadence.

---

## Branch layout (keep it this small)
| Branch | Meaning |
|---|---|
| `main` | single source of truth; only ever **fast-forwarded** by `integrate.sh` |
| `integrate` | scratch convergence branch; rebuilt from `main` each run (== main when idle) |
| `stream/*` | one per active work stream; deleted after it merges |
| `backup/pre-integrate-*` | auto-snapshots before each converge; delete once happy |
| `archive/*` | preserved dead lines (e.g. `archive/gitea-main-2026-04-10`) |

## Non-negotiables
- **Never** `git checkout` in another session's worktree.
- **Never** force-push a shared branch (`main`/`integrate`) without fetching and
  inspecting first — a non-ff rejection means real divergent work exists.
  Archive the other side before overwriting (see `archive/*`).
- **Never** `git add -A` blindly — `target/` is gitignored on `main`; one stray
  add re-committed 1.5 GB of build output on the old fork.
- Keep all three remotes (`origin`, `gitea`, `ococo`) aligned; `integrate.sh`
  pushes to all of them.
