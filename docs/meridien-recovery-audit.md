# Meridien Recovery Audit

Audit of the 19 properties listed in `docs/meridien-recovery-catalog.md` against the current design system in `src-tauri/src/chart_renderer/ui/`.

Sources checked:
- `ui/style.rs` — `StyleSettings` struct + `current()` (lines 749–868)
- `ui/widgets/foundation/{shell,variants,tokens,interaction,text_style}.rs`

---

## 1. UiStyle Struct & MERIDIEN Preset

- **In StyleSettings?** Partially. `StyleSettings` exists and has most token fields: `r_xs/sm/md/lg/r_pill`, `stroke_hair/thin/std/thick`, `hairline_borders`, `serif_headlines`, `uppercase_section_labels`, `solid_active_fills`, `shadows_enabled`, `button_treatment`. Missing fields: `header_height_scale`, `toolbar_height_scale`, `font_hero`, `label_letter_spacing_px`, `vertical_group_dividers`, `stroke_bold`. The `Meridien` preset (style id 0 / `_ =>` arm) also incorrectly sets `r_sm/md/lg` to runtime `radius_*()` values (3/4/8) rather than 0, and `serif_headlines: false` rather than `true`.
- **Consumed by which widgets?** `ButtonShell::show()` reads `current().button_treatment`; `CardShell::show()` reads `hairline_borders` and `shadows_enabled`. The `Radius` token enum in `tokens.rs` maps Sm/Md/Lg to `radius_sm()/radius_md()/radius_lg()` (design-token helpers) rather than to `current().r_*` fields, so style-switching has no effect on radii via shells.
- **Gap** — `StyleSettings` is partially in place but (a) is missing 6 fields the catalog requires, (b) the Meridien arm sets wrong radius values (`radius_sm()` = 3 instead of 0) and `serif_headlines: false`, and (c) the `Radius` enum in `tokens.rs` bypasses `current().r_*` entirely.
- **Suggested fix:**
  - Add missing fields: `header_height_scale: f32`, `toolbar_height_scale: f32`, `font_hero: f32`, `label_letter_spacing_px: f32`, `vertical_group_dividers: bool`, `stroke_bold: f32`.
  - Fix Meridien arm: set `r_xs/sm/md/lg/r_pill = 0/0/0/0/0` and `serif_headlines = true`.
  - Update `Radius::corner()` in `tokens.rs` to read `current().r_sm` etc. instead of `radius_sm()`.
- **Priority** — High (all other properties depend on this being correct).

---

## 2. Meridien Color Themes

- **In StyleSettings?** No. `StyleSettings` has no color fields — themes live in `gpu.rs` `THEMES` const. No "Meridien Paper" or "Meridien Dark" entries exist.
- **Consumed by which widgets?** Not applicable — themes are passed by reference as `&Theme` to every shell, but the two new themes don't exist to be selected.
- **Gap** — Two theme entries and their color values are completely absent from `gpu.rs`. No `theme_recommended_style` field on `Theme` either.
- **Suggested fix:**
  - Add `MeridienPaper` and `MeridienDark` entries to the `THEMES` const in `gpu.rs`.
  - Optionally add `recommended_style: Option<u8>` to `Theme` struct to support auto-pairing.
- **Priority** — High (visual identity is invisible without these themes).

---

## 3. Global Widget Styling (egui visuals)

- **In StyleSettings?** Partially — `hairline_borders`, `shadows_enabled`, and `solid_active_fills` are present. The egui `visuals` block itself (widget fill/stroke states, spacing, `button_padding`, `interact_size`, `item_spacing`, `popup_shadow`, `window_shadow`) is not driven by `StyleSettings` at all; no `apply_ui_style(ctx, style, t)` function exists.
- **Consumed by which widgets?** `CardShell` reads `shadows_enabled`. No shell reads `solid_active_fills`. The toolbar's `item_spacing` and popup shadows are not gated on any `StyleSettings` field in the current code.
- **Gap** — No `apply_ui_style()` function bridges `StyleSettings` → egui `Context` visuals. The full Meridien egui visuals block (transparent inactive fills, hairline strokes, denser spacing, `Shadow::NONE`) is absent.
- **Suggested fix:**
  - Introduce `pub fn apply_ui_style(ctx: &egui::Context, settings: &StyleSettings, t: &Theme)` in `ui/style.rs` that sets `ctx.set_style(...)` with the appropriate `visuals` block.
  - Call it from `setup_theme()` in `gpu.rs` each frame.
- **Priority** — High (affects every egui widget across the whole UI).

---

## 4. Toolbar — Height & Group Dividers

- **In StyleSettings?** No. `toolbar_height_scale` and `vertical_group_dividers` are absent from `StyleSettings`. `hairline_borders` is present but there is no `tb_group_break()` helper or `toolbar_panel_rect` thread-local.
- **Consumed by which widgets?** None — no toolbar shell reads height scale or vertical dividers.
- **Gap** — No mechanism for the 1.40× toolbar height scale or full-height inter-cluster hairlines. The `tb_btn()` helper in `style.rs` is independent of both missing fields.
- **Suggested fix:**
  - Add `toolbar_height_scale: f32` and `vertical_group_dividers: bool` to `StyleSettings`.
  - Re-introduce `tb_group_break(ui, t)` helper and `toolbar_panel_rect` thread-local in `style.rs` or `ui/components.rs`.
  - Update toolbar layout in `gpu.rs` to multiply height by `current().toolbar_height_scale` and call `tb_group_break` when `vertical_group_dividers`.
- **Priority** — High (immediately visible Bloomberg-style toolbar look).

---

## 5. Toolbar — Label Uppercasing

- **In StyleSettings?** Yes — `uppercase_section_labels: bool` is present in `StyleSettings` and set `true` in the Meridien arm.
- **Consumed by which widgets?** `style_label_case(s)` helper exists at line 868 and unconditionally returns `s.to_uppercase()`. However, the toolbar button helper `tb_btn()` does **not** call `style_label_case` — it passes `label` directly to `RichText::new(label)`. No widget reads `uppercase_section_labels` dynamically.
- **Gap** — `uppercase_section_labels` is in `StyleSettings` but `tb_btn()` and any direct call sites don't gate uppercasing on it. `style_label_case` is also unconditional (always uppercases), ignoring the flag.
- **Suggested fix:**
  - Update `style_label_case(s)` to read `current().uppercase_section_labels` and return `s.to_uppercase()` only when true.
  - Update `tb_btn()` (and any toolbar label helpers) to apply `style_label_case` to `label` before building `RichText`.
- **Priority** — Medium (small cosmetic detail but semantically tied to Meridien identity).

---

## 6. Search / Command Pill (Toolbar Right Cluster)

- **In StyleSettings?** No — no `SearchPill` widget and no `cmd_palette_open` state field exist. `hairline_borders` (which gates the tint value) is present.
- **Consumed by which widgets?** None — the search/command pill does not exist in the current codebase.
- **Gap** — Entire widget missing: `paint_search_command_pill()` helper, `widgets::inputs::SearchPill`, and `cmd_palette_open: bool` on `Watchlist`.
- **Suggested fix:**
  - Create `widgets::inputs::SearchPill` struct with width/height params and tint logic gated on `current().hairline_borders`.
  - Add `cmd_palette_open: bool` to the watchlist/app state.
  - Insert the pill into the toolbar right cluster.
- **Priority** — Medium (functional feature, not merely aesthetic).

---

## 7. Connection Status Button — Full-Column Hover Fill

- **In StyleSettings?** No — `vertical_group_dividers` (the intended gate) is absent from `StyleSettings`. `hairline_borders` is present but no helper implements the full-column hover fill.
- **Consumed by which widgets?** None — the full-column hover fill is not implemented.
- **Gap** — No `paint_toolbar_column_hover()` helper and no widget opts in. The connection dot renders as a standard inline icon button.
- **Suggested fix:**
  - Add `pub fn paint_toolbar_column_hover(painter: &Painter, rect: Rect, panel_rect: Rect, color: Color32)` to `style.rs` or `ui/components.rs`.
  - Gate it on `current().vertical_group_dividers` at the toolbar call site.
- **Priority** — Low (subtle micro-interaction).

---

## 8. Pane Chrome — Hairline Border System

- **In StyleSettings?** Partially — `hairline_borders: bool` is present. `rule_stroke_for()` helper also exists at line 864. But there is no `paint_pane_borders()` function and no pane-border dispatch calling `hairline_borders`.
- **Consumed by which widgets?** `CardShell` reads `hairline_borders` for its own stroke, but pane-level border painting is done inline in `gpu.rs` with the old system.
- **Gap** — No `paint_pane_borders(painter, pane_rect, is_active, visible_count, t)` abstraction; the hairline top/left/bottom + accent top-line system is absent.
- **Suggested fix:**
  - Add `pub fn paint_pane_borders(painter: &egui::Painter, pane_rect: Rect, is_active: bool, visible_count: usize, t: &Theme)` to `style.rs`.
  - Have it dispatch on `current().hairline_borders` to choose old vs new system.
  - Call it from `render_chart_pane()` in `gpu.rs`.
- **Priority** — High (defines the entire pane layout visual language).

---

## 9. Pane Header Background Treatment

- **In StyleSettings?** No — `active_header_fill_multiply` and `inactive_header_fill` fields are absent from `StyleSettings`. `hairline_borders` is present but not consumed by pane header painting.
- **Consumed by which widgets?** None — pane header painting is inline in `gpu.rs` with the Relay path only.
- **Gap** — Missing `StyleSettings` fields and no style-dispatch in the pane header fill block. The Meridien near-transparent header (0.95 multiply, no inactive fill) is absent.
- **Suggested fix:**
  - Add `active_header_fill_multiply: f32` (1.2 Relay, 0.95 Meridien) and `inactive_header_fill: bool` (true Relay, false Meridien) to `StyleSettings`.
  - Read these in the pane header fill block in `gpu.rs`.
- **Priority** — High (directly affects every pane's visual frame).

---

## 10. Pane Header — Right Action Cluster

- **In StyleSettings?** Partially — `hairline_borders` (which gates per-divider hairlines between buttons) is present. No `PaneHeaderActions` widget or `paint_pane_header_action()` exists.
- **Consumed by which widgets?** None — the cluster does not exist.
- **Gap** — The entire right-action cluster (+ Compare / Order / DOM / Options) is absent. No widget, no helper, no state.
- **Suggested fix:**
  - Create `widgets::pane::PaneHeaderActions` accepting `Vec<(&str, bool)>` (label + active).
  - Implement `paint_pane_header_action()` helper with hairline dividers gated on `current().hairline_borders`.
  - Insert into `render_chart_pane()` header bar in `gpu.rs`.
- **Priority** — Medium (new UX feature, not purely aesthetic).

---

## 11. Active-Tab Underline Removal

- **In StyleSettings?** No — `show_active_tab_underline: bool` field is absent from `StyleSettings`.
- **Consumed by which widgets?** The `tab_bar()` helper in `style.rs` (line 473) unconditionally paints the 2 px accent bottom underline on the active tab. No flag gates it.
- **Gap** — `tab_bar()` always paints the underline; no way to suppress it for Meridien without adding the field and a conditional.
- **Suggested fix:**
  - Add `show_active_tab_underline: bool` to `StyleSettings` (true for Relay/Aperture, false for Meridien).
  - In `tab_bar()`, wrap the underline paint block in `if current().show_active_tab_underline { ... }`.
- **Priority** — Medium (visible on every tabbed pane).

---

## 12. Section Labels — Uppercase + Tracked-Out

- **In StyleSettings?** Partially — `uppercase_section_labels: bool` is present. `label_letter_spacing_px` is absent. No `section_label_xs()` helper exists (only `section_label()` at line 325, which hardcodes size 7.0 and never reads `uppercase_section_labels`).
- **Consumed by which widgets?** `section_label()` in `style.rs` ignores `uppercase_section_labels`. `style_label_case()` at line 868 always uppercases regardless of the flag. No widget applies letter spacing.
- **Gap** — `section_label()` does not call `style_label_case()`. `section_label_xs()` doesn't exist. `label_letter_spacing_px` field is missing from `StyleSettings`.
- **Suggested fix:**
  - Add `label_letter_spacing_px: f32` to `StyleSettings`.
  - Fix `section_label()` to call `style_label_case(text)` and apply spacing approximation.
  - Create `section_label_xs(ui, text, color)` mirroring the stash helper.
- **Priority** — Medium (affects order ticket, panels, all section headers).

---

## 13. Meridien Order Ticket Layout

- **In StyleSettings?** Partially — `hairline_borders` (the dispatch gate) is present. No `MeridienOrderTicket` widget or `render_order_entry_body_meridien()` function exists.
- **Consumed by which widgets?** None — only the standard compact order-entry form exists.
- **Gap** — The entire Meridien order ticket layout (10-section editorial form, `font_2xl` quantity hero, preset pill chips, BID/LAST/ASK strip, bracket section, meta row) is absent. This is the largest single missing widget.
- **Suggested fix:**
  - Create `widgets::order_entry::MeridienOrderTicket` struct owning the full layout.
  - Dispatch from `render_order_entry_body()` using `if current().hairline_borders`.
  - Depends on: section labels, `ActionButton`, `Stepper`, `PillButton` preset chips (some partially present via `action_btn`/`trade_btn`).
- **Priority** — High (core trading workflow widget, entirely absent).

---

## 14. Serif Hero Headlines

- **In StyleSettings?** Partially — `serif_headlines: bool` is present but set to `false` in the Meridien arm (bug: should be `true`). No `hero_font_id()` or `hero_text()` helper exists in `style.rs`. `TextStyle::NumericHero` in `text_style.rs` hardcodes `monospace: true` and size 30.0, ignoring `serif_headlines`.
- **Consumed by which widgets?** `TextStyle::NumericHero` in `text_style.rs` ignores `serif_headlines`. No widget switches to the serif family.
- **Gap** — `serif_headlines` field is present but `false` for Meridien and no code path checks it. `hero_font_id()` / `hero_text()` helpers are absent. Serif font family not registered.
- **Suggested fix:**
  - Fix Meridien arm to set `serif_headlines: true`.
  - Add `pub fn hero_font_id(size: f32) -> egui::FontId` and `pub fn hero_text(text: &str, color: Color32) -> RichText` to `style.rs`, dispatching on `current().serif_headlines`.
  - Register Source Serif 4 in the font loader.
  - Update `TextStyle::NumericHero.spec()` to check `serif_headlines` for family choice.
- **Priority** — Medium (headline visual feature but requires font registration).

---

## 15. Account Strip — Taller Height + Serif Values

- **In StyleSettings?** No — `account_strip_height: f32` is absent from `StyleSettings`. `serif_headlines` is present (but wrong value; see #14).
- **Consumed by which widgets?** None — account strip height is hardcoded in `gpu.rs`. Serif rendering is not yet possible (see #14).
- **Gap** — Missing `account_strip_height` field and no style dispatch in the account strip panel. Serif value rendering depends on resolving #14 first.
- **Suggested fix:**
  - Add `account_strip_height: f32` to `StyleSettings` (26.0 Relay, 36.0 Meridien).
  - Read it in `account_strip` panel sizing in `gpu.rs`.
  - Drive value rendering through `hero_text()` once #14 is resolved.
- **Priority** — Medium (layout change; depends on #14 for full effect).

---

## 16. Popup / Dialog Window — Flat Hairline Border

- **In StyleSettings?** Partially — `shadows_enabled: bool` is present and set `true` in Meridien arm. However `dialog_window_themed()` in `style.rs` (line 217) unconditionally applies a drop shadow (`blur: 28, spread: 2, color: black_alpha(80)`) regardless of `shadows_enabled`. `popup_frame()` uses `egui::Frame::popup(&ctx.style())` which inherits whatever egui's `popup_shadow` is — not controlled by `shadows_enabled` either.
- **Consumed by which widgets?** `CardShell` reads `shadows_enabled` (for `CardVariant::Elevated` only). `dialog_window_themed()` ignores it.
- **Gap** — `dialog_window_themed()` hard-codes shadow. No `apply_ui_style()` sets `popup_shadow`/`window_shadow` to `Shadow::NONE` when `!shadows_enabled`. `window_stroke` is also not set from `StyleSettings`.
- **Suggested fix:**
  - In `dialog_window_themed()`, read `current().shadows_enabled` and skip the shadow block when false.
  - In `apply_ui_style()` (see #3), set `visuals.popup_shadow = Shadow::NONE` and `visuals.window_shadow = Shadow::NONE` when `!settings.shadows_enabled`.
  - Set `visuals.window_stroke = Stroke::new(stroke_std(), t.toolbar_border)` for Meridien.
- **Priority** — Medium (affects all popups, modals, order ticket).

---

## 17. Effective Theme Overlay (Design-Mode Live Editing)

- **In StyleSettings?** No — this is `#[cfg(feature = "design-mode")]` infrastructure. Not a `StyleSettings` field.
- **Consumed by which widgets?** Not applicable — it's a `gpu.rs` function replacing `&THEMES[idx]` accesses. The `design_tokens` module exists in the codebase (referenced in `style.rs`), but `effective_theme()` and `overlay_palette()` are absent.
- **Gap** — `effective_theme()` / `overlay_palette()` functions and `Theme: Clone` derive are absent from `gpu.rs`. All `THEMES[idx]` accesses are direct.
- **Suggested fix:**
  - Add `#[derive(Clone)]` to `Theme` in `gpu.rs`.
  - Re-introduce `effective_theme(idx: usize) -> Theme` and `overlay_palette()` under `#[cfg(feature = "design-mode")]`.
  - Replace all `&THEMES[idx]` with `effective_theme(idx)` references.
- **Priority** — Low (developer tooling only, no user-visible effect).

---

## 18. `ButtonTreatment::UnderlineActive` — General Button Treatment

- **In StyleSettings?** Yes — `ButtonTreatment` enum (all 5 variants: `SoftPill`, `OutlineAccent`, `UnderlineActive`, `RaisedActive`, `BlackFillActive`) is defined in `style.rs` (line 750) and `button_treatment` is a field on `StyleSettings`. Meridien arm sets `UnderlineActive`. ✓
- **Consumed by which widgets?** `ButtonShell::show()` reads `current().button_treatment` and implements all 5 variants including `UnderlineActive` (paints a bottom stripe). The legacy `tb_btn()` and `action_btn()` helpers in `style.rs` do **not** read `button_treatment` — they use hard-coded styling.
- **Gap** — The `ButtonShell` is fully correct. But `tb_btn()`, `action_btn()`, `trade_btn()`, `simple_btn()`, `small_action_btn()` all bypass `button_treatment`. Any toolbar or order-entry button not ported to `ButtonShell` will ignore the treatment entirely.
- **Suggested fix:**
  - Port `tb_btn()` to read `current().button_treatment` and dispatch rendering.
  - Port `action_btn()` similarly, or migrate call sites to `ButtonShell`.
- **Priority** — Medium (ButtonShell already works; legacy helpers are the gap).

---

## 19. `paint_chrome_tile_button` — Template (T) and +Tab Buttons

- **In StyleSettings?** Not a `StyleSettings` field — it uses `current().r_md` (which maps to `StyleSettings.r_md`) and `current().stroke_thin`. Both fields exist but `r_md` is set to `radius_md() as u8` (≈ 4–6) for Meridien rather than 0.
- **Consumed by which widgets?** No `paint_chrome_tile_button()` helper exists in the codebase. The chrome tile button logic is inlined at both the tabbed and non-tabbed pane header call sites.
- **Gap** — Helper is absent; logic is duplicated inline. For Meridien, corner radius would be wrong (non-zero) because `StyleSettings.r_md` is currently set to `radius_md() as u8` instead of 0 in the Meridien arm.
- **Suggested fix:**
  - Re-introduce `paint_chrome_tile_button(painter, rect, panel_rect, state, t)` as a local helper in `gpu.rs`.
  - Fix Meridien `r_md = 0` (blocked by #1).
  - Replace duplicated inline logic at both call sites.
- **Priority** — Low (visual consistency detail; blocked by #1 radius fix).

---

## Summary

**Total cataloged properties:** 19

| Status | Count | Items |
|--------|-------|-------|
| Already covered (field + consumption correct) | 0 | — |
| Field exists, consumption gap only (shell extension needed) | 5 | #5, #11, #16, #18, #19 |
| Field partially wrong / missing, no new widget needed | 5 | #1, #3, #12, #14, #15 |
| Field missing + new widget needed | 6 | #4, #6, #8, #9, #10, #13 |
| Infrastructure only (no StyleSettings field needed) | 2 | #2, #17 |
| Incorrect field value (bug fix only) | 1 | #14 (serif_headlines: false → true in Meridien arm) |

Simplified breakdown per the requested categories:
- **Already covered end-to-end:** 0
- **Need shell extension / helper wiring only:** 5 (#5, #11, #16, #18, #19)
- **Need new widget (+ field additions):** 6 (#4, #6, #8, #9, #10, #13)
- **Need both field + shell extension:** 5 (#1, #3, #12, #14, #15)
- **Infrastructure only:** 3 (#2, #7, #17)

### High-priority items (top 5 — fix first)

1. **#1 — UiStyle/StyleSettings correctness** — All radius fields wrong for Meridien, `serif_headlines` wrong, 6 fields missing. Everything else is blocked on this.
2. **#3 — Global egui visuals / `apply_ui_style()`** — Without this, inactive fills, hover states, spacing, and shadows are all wrong in every egui widget.
3. **#8 — Pane chrome hairline border system** — The most visible structural difference between Relay and Meridien; needs `paint_pane_borders()` abstraction.
4. **#13 — Meridien Order Ticket Layout** — The largest missing widget; the entire trading workflow for Meridien is absent.
5. **#4 — Toolbar height scale + group dividers** — The Bloomberg-style tall toolbar + inter-cluster hairlines are immediately visible and define the Meridien toolbar identity.
