
### Status (autonomous push 4 — same session continued)

Items 1 and 2 from the "truly remaining" list are now complete.

- **Item 1** (`79d5160c`) — **Duplicate cleanup done.** Removed the
  duplicate font / FONT / gap / GAP / stroke / radius_pill / alpha_whisper
  / alpha_hint / ELEVATION_* / color_alpha definitions from
  `chart_renderer::ui::style`. Added `pub use crate::ui_kit::style::*;`
  at the top so 50+ chart-app call sites keep working transparently.
  `ui_kit::style` is now the SINGLE source of truth for these stateless
  primitives. The stateful style machinery (`FRAME_TOKENS`,
  `STYLE_STORE`, `ACTIVE_STYLE`, `Theme`-taking helpers) stays in
  `chart_renderer::ui::style` — those depend on the chart-app's style
  preset system.

- **Item 2** (`c3a7e990`) — **Ambient theme pattern landed.** New
  `set_ambient_theme(ctx, theme)` / `get_ambient_theme(ctx)` API in
  `ui_kit::widgets::theme`. `chart_renderer::theme_impl::active_theme`
  now reads the ambient stash first and falls back to the live registry
  by idx. `chart_renderer::gpu`'s per-frame loop stashes the resolved
  Theme alongside the existing idx. All 14+ `Widget::ui` impls calling
  `active_theme(ctx)` now resolve through the portable ambient path
  with the registry-by-idx as a legacy fallback. A future doc app can
  `set_ambient_theme(ctx, its_theme)` and the same widgets work.

### Remaining for a workspace crate

3. **Physical move of `frames_widget` bodies** — `PanelFrame::build()`
   / `PopupFrame::build()` etc call `current()` (chart-app StyleSystem
   state) for `card_padding_x/y`, `shadow_alpha`, `shadow_blur`,
   `stroke_std`, `hairline_borders` flags. To move these into
   `ui_kit/widgets/frames.rs` cleanly, the bodies need to take these
   style values as explicit builder params (or via a trait the chart
   app implements). Half-day API design.

4. **Workspace crate scaffold** (`crates/apex-ui/`) requires the
   bridge files (theme.rs, frames.rs, tokens.rs) to STOP referencing
   chart_renderer. Today they re-export through it. The clean end-state
   is:
   - `theme.rs` defines a portable `Theme` struct (Phase 3 from the
     original plan, not yet started); chart_renderer's `gpu::Theme`
     becomes a *superset* implementing `ComponentTheme` (trivial).
   - `frames.rs` owns its types (item 3 above).
   - `tokens.rs` re-exports only `ui_kit::style` (no chart_renderer ref).
   - `side_panel_shell` / `split_section_panel` either physically move
     out of ui_kit (they're chart-app composites) or grow a trait
     abstraction over the kit::PanelHeader bits.

   With items 3 and the portable Theme struct done, the workspace
   move is mechanical ~1 day.

### Session totals (final)

- **Inverted imports**: 78 → 14 (all in 4 bridge files, one is a doc
  comment).
- **6 widgets** fully on `&dyn ComponentTheme`.
- **`ComponentTheme` trait** fully semantic: `success`/`danger` +
  5 surface tokens (`surface_border`, `header_surface`,
  `section_header_surface`, `panel_surface`, `header_border`).
- **`impl ComponentTheme for Theme`** in `chart_renderer::theme_impl`
  (correct dep direction).
- **`ui_kit::style`** is the sole owner of stateless token primitives
  (~40 fns/consts deduped out of `chart_renderer::ui::style`).
- **Ambient theme API** in place; the chart app stashes Theme once per
  frame; widget impls resolve through the portable path.

Items 3 + portable Theme struct + workspace scaffold are the
remaining 2–3 days of bounded work.
