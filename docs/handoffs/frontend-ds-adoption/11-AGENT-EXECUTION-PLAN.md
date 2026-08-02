# 11 — Agent Execution Plan: Running the Master Plan with a Multi-Agent Team

**Question this answers:** can the M0–M5 / T1–T5 master plan (`10-MASTER-PLAN.md`) be
executed by an AI agent team with a human in the loop, and what does that execution
concretely look like?

**Answer:** yes — this repo has already been through multi-agent waves (agent worktrees
exist under `.claude/worktrees/`; the S0–S10 style-migration streams and the U-series
audits were agent-executed). But the master plan is **not uniformly fan-out-able**, and
pretending it is would recreate the exact failure mode the audit found: many hands, no
convergence. The topology below assigns each milestone its correct shape.

---

## 1. The three execution constraints (violate any one and the plan corrupts)

### C-1 · The spine is single-threaded
M1 (one source of truth) rewires the resolution pipeline that every other change flows
through. `10` already mandates ONE owner. In agent terms: **M1 is main-loop work by the
orchestrating agent itself** — not delegated, not parallel. Fan-out during M1 is limited
to *read-only* verification and *mechanical* migration slices that the spine owner
reviews and merges serially.

### C-2 · Build-and-verify is a serialized resource (Windows, this machine)
Known, memory-documented traps:
- **Concurrent `cargo build` produces phantom failures** (corpus contention).
- **Zombie processes lock `apex-native.exe`** → the next build silently fails to relink
  while `deps/` looks fresh → you verify a stale binary without being told.
- The dev_inspector harness drives ONE live app instance; screenshots come from it.

Therefore: agents may edit in parallel (worktrees isolate files), but **exactly one
build+screenshot gate runs at a time**, owned by the orchestrator, preceded always by a
kill-stale-processes step and a binary-mtime assertion. Agent worktrees do `cargo check`
locally at most; full builds and all visual verification happen at the gate.

### C-3 · Sacred and frozen constraints are per-agent, not just per-human
`core.rs`: no agent touches it, ever, except a single explicitly-mandated shell-branch
ticket with the orchestrator as owner. `Watchlist`/`Chart`: no new fields — verifier
agents grep for this on every diff. These rules go verbatim into every agent prompt.

---

## 2. The team topology

| Role | Who | Count | Isolation | Does |
|---|---|---|---|---|
| **Orchestrator / Spine owner** | main session (me) | 1 | repo | M1 entirely; all merges; the build+verify gate; all `core.rs`-adjacent work; decision escalation to the user |
| **Harness agent** | general-purpose | 1 (M0) | repo | Builds M0.1: capture scripts, contact sheets, pixel asserts. Becomes a *tool* everyone else calls, not a standing agent |
| **Migrator agents** | general-purpose | 2–4 per wave | worktree each | Mechanical, sharded-by-file slices: the 187 `current()` reads, ui_kit `RichText`/`FontId` cascade adoption, `.hovered()` burndown, stroke/gamma sweeps. Prompt = pattern + file list + forbidden-files list |
| **Widget-family agents** | general-purpose | 2–3 per wave | worktree each | M2/M3 semantic work sharded by widget family (forms · panels · overlays · rows): StyleCtx adoption, slot-signature unification, recipe consultation |
| **Theme-author agents** | general-purpose | 1 per theme | worktree | T1–T3: transcribe authored values + recipe data from `global.css`/DS specs into `builtin.rs`/pack JSON. Pure data work, highly parallel |
| **Verifier agents** | Explore (read-only) | 1–2 per merge wave | repo (RO) | Adversarial diff review: constraint compliance (C-3), no-new-mechanism rule, ratchet direction, "did the adapter stay total". They refute; they don't fix |
| **Layout agent** | general-purpose | 1 | worktree | M4.1/M4.4: MeasureFunc bridge and the Grid wrapper — self-contained, headless-testable, ideal solo-agent modules |

Standing rules injected into **every** prompt: never `core.rs`; never new `Watchlist`/
`Chart` fields; tokens not literals; no new mechanism (converge or delete); `cargo check`
only, no `cargo build`/run; report file:line evidence, not claims.

---

## 3. Per-milestone execution shape

### M0 — Verify + stop the bleeding *(mixed · ~2–4 sessions)*
- **Fan-out (parallel):** Harness agent builds M0.1 ‖ one migrator does M0.6+M0.7
  (gamma/font literals) ‖ one does M0.8 gate hygiene ‖ one drafts M0.10 doc-sync.
- **Inline (orchestrator):** M0.2 black shadow, M0.3 radius unification, M0.4 ambient
  dedup, M0.5 round-trip — these touch the resolution path and are the spine owner's
  warm-up; each is small but each needs the visual gate.
- **Gate:** every M0 fix gets a before/after screenshot from the new harness. The
  harness is thereby validated by its own first customers.

### M1 — One source of truth *(inline-dominant · ~6–10 sessions · the critical path)*
- **Orchestrator, serially:** M1.1 resolver wire → M1.2 adapter totality → M1.3 token
  contract (A/C/D/E incl. schema v2) → M1.4 font-ladder collapse → M1.6 pack
  activation → M1.7 design-mode wiring. Equivalence suite runs after every step.
- **Fan-out, bounded:** M1.5's 187 `current()` call sites shard to 3–4 migrators by
  directory *after* M1.1 lands (so they migrate onto the real path). Verifier agent per
  merged slice. Merge serially; ratchet must fall monotonically.
- **Why not parallelize the spine:** every M1.x changes what M1.x+1 builds against.
  Parallel spine agents would each pick a different intermediate state — that is how the
  codebase got four theme paths in the first place.
- **Gate:** the P-2 totality test + the "move one token → pixel diff" sweep, screenshot-
  verified per axis (spacing, alphas, elevation, shadows).

### M2 — Scoped context + cascade *(fan-out · ~4–6 sessions)*
- **Inline first:** M2.1 (move `TextStyle` into ui_kit — a dependency-direction change
  with wide but mechanical fallout) and M2.3 (StyleCtx scope stack).
- **Then fan out:** widget-family agents take M2.2/M2.4 by family (forms · panels ·
  overlays · rows); one migrator takes M2.5 painter `font_id_in` in the 10 densest
  files; one takes M2.6/M2.7.
- **Gate:** subtree-override-restyles-a-Button test; two-themes-two-panes screenshot;
  two-densities-one-frame in the playground.

### M3 — Recipes + states live *(the widest fan-out · ~5–7 sessions)*
- **Inline first:** M3.2 Sx vocabulary growth (one owner — it's the shared contract) and
  M3.1's Button conversion (the reference implementation others copy).
- **Fan out wide:** widget-family agents convert the ~40 parameter-less widgets to
  `StyleCtx::from_ctx` + recipe consultation; **theme-author agents (up to 6 in
  parallel)** transcribe the React `[data-ds]` rules into per-pack `recipes.json`;
  one migrator runs the `.hovered()` burndown in touched files.
- **Gate:** the crown-jewel demo — pack switch structurally restyles
  Button/Tabs/Rows/Cards across all six themes in the playground, zero widget edits.
  Adoption gate (M3.6) goes green in CI.

### M4 — Layout *(solo-agent modules + inline integration · ~5–7 sessions)*
- **Layout agent:** M4.1 MeasureFunc and M4.4 Grid — headless-testable, no app build
  needed until integration.
- **Migrators:** M4.3 ui_kit chrome migration by file (~120 sites), after M4.1 lands.
- **Inline:** M4.6 root shell solve (touches the two governed `core.rs` branch points;
  orchestrator only; **blocked on the user's `ShellProfile`/DS-6.0 decision**), M4.5
  structural tokens, M4.7 definite heights.
- **Gate:** three archetype fixtures render; resize-sweep reflow (recalling: a constant
  widget count across the sweep means the harness broke, not that the UI is clean).

### M5 — Geometry endgame *(pure fan-out · rolling)*
Migrators shard the top-10 file list; one agent writes the `rect_filled` AST lint;
verifiers hold the ratchets. Merges whenever the gate is free.

### T-track — Themes *(theme-author agents, entry-gated)*
- **T1 Alto+Mariner** (post-M1): two author agents in parallel worktrees; orchestrator
  runs the sibling-distinguishability gate personally — a verifier agent is shown the
  two screenshots *unlabeled* and must identify which is which, with reasons.
- **T2–T4** follow their entry gates per `10` §4. The editorial dashboard (T4) is the
  one XL item that may itself warrant a sub-plan before agents are assigned.
- **T5 certification:** verifier agents run the full `04` gate matrix per theme;
  results recorded with honest React-scale scores.

---

## 4. The merge-and-verify cycle (every wave, no exceptions)

```
agent completes in worktree
  → verifier agent: adversarial diff review (constraints, ratchets, no-new-mechanism)
  → orchestrator: merge to main branch
  → BUILD GATE (serialized): kill stale processes → cargo build → assert binary mtime
  → HARNESS: screenshot affected surfaces, pixel-assert ramps where applicable
  → ratchets: check-design-system / style-mig-lint / adoption gates — none may rise
  → commit (one commit per unit of work)
  → next wave
```

Rules already standing in this environment that the cycle encodes: commit every piece;
never fabricate results; screenshot or it didn't happen; a failing gate stops the wave —
it is never "noted and continued past."

---

## 5. Honest risk assessment of agent execution

| Risk | Mitigation |
|---|---|
| **Mechanical migrators "complete" a pattern wrongly at scale** (the plausible-but-wrong sweep) | Small shards (≤15 files); verifier per shard; the equivalence suite + screenshot gate catch semantic drift; ratchets catch direction |
| **Two agents converge on the same shared file** (mod.rs, style.rs re-exports) | Shard boundaries assign shared files to exactly one agent per wave; orchestrator resolves the rest at merge |
| **Agent invents a fifth mechanism under pressure** (the historical failure mode) | The no-new-mechanism rule is in every prompt AND the verifier's checklist; any new `pub` API needs orchestrator sign-off |
| **Stale-binary verification** (C-2) | Kill-step + mtime assertion are *in the gate script*, not in anyone's memory |
| **Context loss across sessions** (this is multi-week work) | The package itself is the state: `10` milestone checkboxes + per-milestone gate results appended to this doc; memory entries updated at each gate |
| **User-decision deadlocks** | The four standing decisions (selection model · ShellProfile · font-weight scope · dashboard scope) are surfaced at their gate, not discovered mid-wave |

**What agents genuinely cannot do here:** approve the taste calls. Fidelity gates end in
a human look — the user judging "yes, that *is* Meridien" against the originals at
`localhost:5173`. The plan schedules that judgment at T-gates rather than pretending a
pixel-assert replaces it.

---

## 6. Estimated envelope

| Phase | Sessions (est.) | Dominant mode |
|---|---|---|
| M0 | 2–4 | mixed |
| M1 | 6–10 | **inline spine** |
| M2 | 4–6 | fan-out |
| M3 | 5–7 | widest fan-out |
| M4 | 5–7 | solo modules + inline |
| M5 | rolling | fan-out |
| T1–T5 | 6–10 overlapped | theme authors + verify |

A "session" ≈ one focused working block ending at a gate. Total: roughly **25–40
sessions**, heavily front-loaded on inline spine work — which matches the audit's lesson:
the bottleneck was never hands, it was convergence.

---

## 7. What starting looks like

1. User gives the go (and answers nothing yet — no decisions block M0).
2. Session 1: orchestrator kills stale processes, baselines the gates, spawns the
   Harness agent (M0.1) + two migrators (M0.6/0.7, M0.8) in worktrees, and personally
   lands M0.2 (the black-shadow one-liner) as the first screenshot-verified fix.
3. Every subsequent session opens by reading `10`'s checkbox state and the last gate
   result appended here.
