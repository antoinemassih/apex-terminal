# UI Workflow — the tools that make visual work fast

Built 2026-07-31, after the UI audit concluded that the real bottleneck was not
missing abstraction (there are 92 ui_kit widgets) but **a slow feedback loop and
nothing forcing the good path**. See `UI_AUDIT_2026-07-31.md` for the evidence.

## 1. See what's drawing something — `Ctrl+Shift+D`

Toggles egui's built-in widget inspector: hover any pixel and it shows the
widget rect + id that owns it, plus hit-testing and layout-expansion warnings.

This existed in egui the whole time and was never switched on. Without it,
"what is drawing this line?" meant adding a coloured stroke to a suspect,
rebuilding (~3 min), relaunching, and squinting — which is exactly how a single
unexplained 1px outline consumed hours during the audit.

Headless equivalent (drive it from the dev harness):

```bash
curl -s -X POST http://127.0.0.1:$PORT/cmd \
  -H 'Content-Type: application/json' -d '{"cmd":"SetUiDebug","on":true}'
```

Distinct from `Ctrl+Shift+I`, which is the **bug-report anchor picker** — that
one only sees regions that registered themselves. `Ctrl+Shift+D` sees every
widget, including the ones nothing registered (usually the culprit).

## 2. Tune tokens live — `cargo design`

```bash
cargo design      # dev build with the design-mode inspector compiled in
```

Then **F12** opens the token editor: sliders and colour pickers for every design
token, applied on the next repaint, saveable back to `design.toml`. The theme
watcher also hot-reloads DTCG JSON from the styles/ dir within ~1.5s.

All of this already existed but sat behind the non-default `design-mode`
feature, so a normal `cargo apex-dev` build had no live editing at all. If you
are tuning anything visual, use `cargo design` — editing a number and seeing it
immediately beats any amount of rebuild-driven guesswork.

## 3. Let the cascade set type — don't hand-pass sizes

`egui::Style` is inherited by child `Ui`s, and its `text_styles` table is a
semantic-name → `FontId` map. All 14 `TextStyle` tiers are now registered into
it every frame (`TextStyle::install`, called from `setup_theme`), so:

```rust
// preferred — size comes from the inherited table
ui.label(TextStyle::Body.as_rich_cascading("Hello", t.text));

// a subtree can override ONE tier for all its children:
ui.style_mut().text_styles.insert(TextStyle::Body.egui(), smaller_font);
```

This is the thing hand-passed `FontId`s can never do, and it is why ~70% of the
app had drifted onto 9-11px: every one of ~626 text sites independently chose a
size. `as_rich` (explicit size) still works and is unchanged — migrate call
sites to `as_rich_cascading` opportunistically.

## 4. Don't thread `&Theme` just to get a colour

The full chart `Theme` is ambient-stashed every frame, so any function with a
`ui` or `ctx` can get it:

```rust
let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
```

Threading `&Theme` through three layers is real work; typing
`Color32::from_rgb(56, 200, 120)` is not — which is precisely how the RRG panels
ended up theme-blind despite the palette carrying tuned `rrg_*` colours for all
19 themes. With the ambient accessor, the design system stops losing to the
shortcut.

## 5. The ratchet keeps it from drifting back

```bash
./scripts/check-design-system.sh            # check (also runs in CI on every push/PR)
./scripts/check-design-system.sh --update   # re-record after a genuine improvement
```

Per-file budgets for raw egui primitives, literal font sizes, named/literal
colours and `CornerRadius::same(...)`. It only ever tightens: a file may not
exceed its recorded count, and improvements can be locked in with `--update`.

Baseline at introduction: **903 violations across 127 files.**

Two deliberate design choices:

- **Counts, not line numbers.** The previous gate stored `path:line:content` and
  compared exact lines, so any edit above a violation reported it as new. A gate
  that cries wolf on every refactor gets blindly regenerated, which makes it
  decorative.
- **Token-definition files are exempt** (`style.rs`, `theme*.rs`, `builtin.rs`,
  `design_inspector.rs`, …) — they must use raw primitives to build the tokens
  everything else consumes. Note that `grep --exclude` silently does **not**
  filter under Git-Bash grep 3.0 on Windows, so the exemption is applied in the
  pipeline instead; if you change that code, re-run the self-test below.

Self-test the gate after touching it (an untested gate is a decorative gate):

```bash
./scripts/check-design-system.sh                       # expect: pass
printf '\nfn _p() -> egui::Color32 { egui::Color32::from_rgb(1,2,3) }\n' >> some_panel.rs
./scripts/check-design-system.sh                       # expect: FAIL, +1 on that file
git checkout some_panel.rs
```

## Not done: a real layout engine

Flexbox/grid would remove hand-computed alignment arithmetic (`rect.left() +
16.0`, `cy - h * 0.5`), which is where pixel drift comes from. The realistic
route is **Taffy** (the Rust flex/grid engine behind Bevy, Dioxus and Zed) via
an egui binding, scoped to panel chrome and forms — explicitly NOT chart panes
or streaming rows, which need painter-exact geometry and no per-frame solve.

This is a spike, not a afternoon: it adds a dependency, needs validation against
the pinned egui version, and immediate-mode/retained-layout has a real impedance
mismatch. Deferred deliberately rather than half-adopted.
