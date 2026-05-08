# Running the GPUI showcase

This is a standalone comparison reference. **NOT integrated with Apex Terminal in any way** — no submodule, no shared deps. Lives at a sibling path.

## What it is

[Longbridge's `gpui-component`](https://github.com/longbridge/gpui-component) gallery — a comprehensive demo of GPUI's widget capabilities, built by the team behind the Longbridge Pro brokerage desktop app. 60+ components, all relevant to a trading-app surface (data tables, charts, inputs, dock layout, code editor, markdown, virtualized lists).

- Repo: `C:\Users\USER\documents\development\gpui-component\`
- Commit pinned during this session: `2d2524d89efc47270e9a0ee18f10fa72cd573eff`
- On-disk size: ~28 MB source, ~8 GB after a release build (target dir).

## How to run

GPUI itself (the upstream Zed library) uses unstable Rust features (`round_char_boundary`), so **nightly is required**:

```bash
cd C:/Users/USER/documents/development/gpui-component
cargo +nightly build --release -p gpui-component-story
./target/release/gpui-component-story.exe
```

Stable Rust 1.89 fails with `error[E0658]: use of unstable library feature 'round_char_boundary'` inside `zed/crates/gpui/src/text_system/line_wrapper.rs`. `rustup toolchain install nightly` then `cargo +nightly` works clean.

First build is ~2 min on this machine (release, 1 min 03 s incremental). No DirectX / Windows SDK errors — DX11 backend "just works" on Windows 11.

## What you'll see

The gallery is a single window, sidebar-driven, with 60+ stories listed:

- Form primitives: `Button`, `Input`, `NumberInput`, `OtpInput`, `Checkbox`, `Radio`, `Select`, `DropdownButton`, `DatePicker`, `Slider`, `Stepper`, `Rating`, `ColorPicker`
- Surfaces: `Dialog`, `AlertDialog`, `Sheet`, `Popover`, `HoverCard`, `Tooltip`, `Notification`, `Menu`
- Data: **`DataTable` (virtualized)**, `List` (virtualized), `Chart`, `Calendar`, `DescriptionList`, `Pagination`, `Breadcrumb`
- Layout: `Sidebar`, `Resizable`, **`Dock`** (full panel/tile system), `Tiles` (freeform), `Accordion`, `Collapsible`, `GroupBox`, `Separator`, `Skeleton`
- Content: **`Editor`** (code editor with LSP + tree-sitter), `Markdown`, `Html`, `Image`, `Icon`, `Kbd`, `Clipboard`
- Misc: `Avatar`, `Badge`, `Alert`, `Progress`, `Spinner`, `Scrollbar`, `Settings`, `Form`, `Switch`, `Tag`, `Title`, `Tabs`, `Toggle`, `Tree`, `Webview`

Theme system (light/dark + multi-theme) initializes on launch; log line `Reload active theme: "Default Light"` confirms.

## What it tells us

Honest comparison vs Apex Terminal's current egui-based chrome (post-polish):

1. **Breadth of stock components is in another league.** GPUI Component ships virtualized `DataTable`, `List`, full `Editor` with LSP/tree-sitter, `Dock` panel system, charts, markdown, and a proper notification stack out of the box. egui has none of these at this fidelity — every one is something we've either built, deferred, or hacked around.

2. **The trading-app pedigree shows.** Dock + Tiles + virtualized table + chart in one widget set means the "Bloomberg-style multi-panel workspace" we keep approximating is essentially a freebie here. Longbridge Pro is shipped on this exact stack.

3. **Build cost is real but bounded.** Nightly toolchain + ~8 GB target dir + 2-min release build is heavier than egui (~600 MB, 30 s). For a single-binary desktop app this is acceptable; for CI/agent-sandbox iteration it is friction.

4. **Runtime feel (subjective, ~5 s observation):** window opens fast, fonts crisp on Windows 11 ClearType, shadows look modeled (proper blur, soft falloff) rather than the flat `Stroke + 1px alpha` approximations we have in egui. Animation isn't visible without interaction, but the sidebar story-list scroll was buttery.

5. **"Feels like Zed" rating: 8/10.** It IS Zed's UI runtime — same gpui crate from `zed-industries/zed` HEAD. The 2 lost points are: (a) it's a gallery, not a real workflow, so we can't judge sustained interaction; (b) some widgets (calendar, color picker) carry a faint shadcn-port flavor rather than the Zed-native polish you see in Zed's command palette / buffer chrome.

### Migration calculus shift

Before this exercise the assumption was "GPUI on Windows is a research project." It isn't — it builds clean on nightly, runs, looks good. The 6-month port estimate is now driven by **our** component count and Apex-specific chart engine, not by GPUI being unproven on Windows. If we ever do migrate, starting from `gpui-component` (MIT/Apache) as the chrome layer would erase 2-3 months of that estimate.

## Constraints honored

- No changes to Apex Terminal Cargo.toml or any source.
- gpui-component cloned to sibling directory, not nested.
- Nothing committed in either repo.
- Apex-native not running concurrently.
