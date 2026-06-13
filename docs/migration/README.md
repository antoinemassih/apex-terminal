# Style Migration — Ratchet Lint Guide

This document explains the ratchet-lint workflow introduced in Stream S0
of the style-migration program.  The lint script prevents regressions from
being silently introduced on any branch while migration work is in progress.

---

## Quick start

```bash
# Run from the repo root (Git Bash or any POSIX shell)
bash scripts/style-mig-lint.sh

# Verbose mode — prints the actual matching lines for each check
bash scripts/style-mig-lint.sh --verbose
```

Exit code 0 means all checks pass.  Exit code 1 means at least one ratchet
tripped; the failing check name and its live vs. baseline count are printed.

---

## What is checked

| # | What | File / scope | Ratchet direction |
|---|------|--------------|-------------------|
| 1 | `pub` field count on `StyleSettings` struct | `src-tauri/src/chart/renderer/ui/style.rs` | DOWN — no new fields |
| 2 | `crate::chart_renderer` / `crate::chart::renderer` cross-imports inside `ui_kit/` | `src-tauri/src/ui_kit/` | DOWN — ui_kit decouples over time |
| 3 | `&THEMES[0]` in code paths (comment-only lines excluded) | `src-tauri/src/` | HARD BAN (baseline = 0) |
| 4 | `Color32::from_rgba_unmultiplied(0, 0, 0,` literal-black shadows | `src-tauri/src/` | DOWN — migrate to themed shadows |

Baselines are stored in [`docs/migration/baselines.toml`](baselines.toml).

---

## How ratchets work

A ratchet check passes when `live_count <= baseline`.  It fails when new
occurrences are added that push the count above the recorded ceiling.
The ceiling can only go DOWN — you lower it after cleaning up, it can never
be raised back up (that would be committing a regression).

Think of each baseline entry as a promise: "we have at most N of these, and
we are working toward zero."

---

## Lowering a baseline after a stream lands

1. Merge the stream's PR (the code removals are in the diff).
2. Run the lint in verbose mode and confirm the live count dropped:
   ```bash
   bash scripts/style-mig-lint.sh --verbose
   ```
3. Edit `docs/migration/baselines.toml` and set the relevant key to the
   new (lower) live count.
4. Commit `baselines.toml` alongside a short note, e.g.:
   ```
   chore(s0): lower chart_renderer_refs_in_ui_kit baseline 13 → 9 after S2 lands
   ```
5. Push.  The lint will now enforce the new, lower ceiling on every future
   branch.

Example — after Stream S2 (ui_kit decoupling) removes 4 cross-imports:

```toml
# before
chart_renderer_refs_in_ui_kit = 13

# after S2
chart_renderer_refs_in_ui_kit = 9
```

---

## When a pattern reaches zero — hard bans

When a count reaches 0 the ratchet automatically becomes a hard ban:
`live <= 0` means the first new occurrence fails the check.  No additional
configuration is needed.

For documentation clarity, update the comment in `baselines.toml` for that
key from "Ratchet direction: DOWN" to "HARD BAN — first new occurrence
fails" so reviewers understand the intent at a glance.

Similarly, update the corresponding check comment in `scripts/style-mig-lint.sh`.

Checks 3 (`&THEMES[0]` code-path refs) and eventually 2 and 4 will reach
this state as the migration completes.

---

## Adding the lint to CI

Wire the script into your CI job after `cargo check` (it is fast — pure
grep, no compilation):

```yaml
# GitHub Actions example
- name: Style migration lint
  run: bash scripts/style-mig-lint.sh
```

```yaml
# Gitea Actions example
- name: Style migration lint
  run: bash scripts/style-mig-lint.sh
```

The script is self-contained (requires only `bash`, `grep`, `awk`, `sed`,
`wc`) and runs on Linux, macOS, and Windows (Git Bash).

---

## Migration stream map

| Stream | Focus | Affects checks |
|--------|-------|----------------|
| S0 | Guardrails & CI lints (this doc) | — sets up all four |
| S1 | StyleSettings field pruning | 1 |
| S2 | ui_kit decoupling (remove chart_renderer imports) | 2 |
| S3 | Themed shadow migration (remove literal-black) | 4 |
| S4 | Final sweep + hard-ban confirmation | 1, 2, 3, 4 |

At the end of S4 all four baselines should be 0 and all four checks are
hard bans enforced in CI permanently.

---

## Non-negotiable constraints (do not modify the lint for these)

- `src-tauri/src/chart/renderer/render/pane/core.rs` is sacred — the lint
  does not grep inside it and no stream touches it.
- `Watchlist` and `Chart` structs (`chart/renderer/gpu.rs`) are frozen —
  no new fields.  This is enforced by ADR-0001, not by this lint.
- Check 3 (`&THEMES[0]`) baseline must never be raised above 0.
