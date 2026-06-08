# Handoff — Design-system sweep + Notification ticker/toasts (2026-06-08)

Branch: `main` · all work committed, working tree clean. Build: `cargo build --release --bin apex-native` (~10 min; release links fail with "Access is denied" if the app is running — Stop-Process apex-native first).

**Verify by build + code review only. Do NOT take screenshots** (4K/ppp makes captures wrong, thinkorswim sits on top of apex-native, and it burns tokens). See memory `feedback_no_screenshots`.

## Commits this session (newest first)
- `cd5b5da5` settings: toggle + route notifications (toolbar vs toasts)
- `fda6f069` toolbar: ticker type-icons + clean slide-in entrance
- `12471531` toolbar: smoother ticker + fixed 4½-slot window
- `e2774bb7` toolbar: ticker feeds from the left (newest in fixed first slot)
- `8b478dcd` toolbar: stable right-aligned ticker-tape + in-place morph box
- `7cacb643` toolbar: ticker 4-cap + notification bell + floating toast
- `24fae0b2` / `c00065d3` / `91902445` earlier ticker refinements
- `882e70ee` / `da1a4c3d` / `481cebc1` design-system unification (dropdowns / dialogs / toolbar popups)

## What's done

### Design-system unification (committed, verified building)
- **Dropdowns** → all route through `ui_kit::Select`. Chart-side `Dropdown`/`DropdownOwned` are now thin adapters over it; `Select` gained `.item_context_menu()`. Raw `egui::ComboBox` sites in `core.rs` + `properties_bar.rs` migrated.
- **Dialogs/modals** → `FrameKind::DialogWindow` (was 7 divergent hand-built frames). Fixed `news_panel` header-override bug.
- **Toolbar dropdowns** (timeframe/layout) → canonical `PopupFrame`.
- Settings/forms audited — already consistent; `Fieldset`/`FormActionBar` already exported. No changes needed.

### Notification ticker (toolbar) — file: `src-tauri/src/chart/renderer/ui/components/toolbar/alert_feed.rs`
Fully reworked `render_badge_feed`:
- **Stable fixed frame**, badges *painted* at controlled positions (no egui auto-layout). Left edge + right-pinned bell never move.
- **Feeds from the LEFT**: newest lands in a fixed first slot; existing badges slide RIGHT and clip out past the bell.
- **Fixed 4½-slot window** (`AREA_SLOTS=4.5`, `SLOT_W=150`) right-anchored before the bell — 4 full + a half (signals more).
- **Smooth eased slide** (`ease_in_out` cubic, retargets from current eased pos; `SLIDE_DUR=0.34`). New badge **slides in from one slot left** keeping exact spacing (no overlap glitch). `slide_state` in egui memory (to,from,start).
- **Type = ICON** (saves space): Signal→PULSE, PriceAlert→$, OrderFilled→check-circle, OrderRejected→x-circle, OrderPending→clock, Error→shield-warning, Warning→warning (`kind_icon`). Severity colour still conveys good/bad.
- **Bell** pinned right with live overflow count + click → **history popover** (all alerts, newest-first, Clear all).
- **Hover = in-place morph box** (foreground layer, NOT a tooltip): pill grows 2D into a card with the full wrapped message; **freeze** the running order while expanded so incoming alerts don't move it. Expand box header = icon + word.

### Toasts above footer — `top_nav.rs` + `bottom_dock.rs`
- Bottom-left toasts sit **8px above the footer**, riding up as the footer expands (`bottom_dock::current_height()` → toast `base_y`).

### Notification routing settings (NEW) — `notification.rs` + `settings_panel.rs`
- Settings → **Trading tab → NOTIFICATIONS** section:
  - Master toggles: "Toolbar ticker" / "Toasts (bottom-left)".
  - **Route by type**: Orders / Signals / Price alerts / System-Connection → Off / Toolbar / Toast / Both.
  - Defaults: trading events → toolbar; system/connection → toasts.
- Impl: `RoutingPrefs` thread-local in `notification.rs`, synced each frame from egui-persisted data via `routing_from_ctx` (called at top of `top_nav::render`). `push_pending` classifies by `category_for(source)` and delivers per prefs + master enables. Ticker + toast stack early-out when their surface is off.

## Key gotchas / context
- App display is **4K @ ~2× ppp**; egui logical coords ≠ captured physical pixels.
- `push_pending` (notification.rs) is the single routing entry point.
- `seed_placeholders()` in alert_feed.rs still pushes 3 demo badges (AAPL/SPY/NVDA) — remove once real alerts are flowing if desired.

## Possible next steps (none committed-to)
- Tune ticker feel: `SLIDE_DUR` / `AREA_SLOTS` / `SLOT_W` / icon choices.
- Burst entrance: only slot-0 slides in; multiple simultaneous arrivals appear in place (rare).
- Deferred design-system items: `object_tree.rs` FOLDER action-menu + `MeridienOrderTicket` hand-rolled form (still raw, flagged earlier).
- Consider a dedicated "Notifications" settings tab if Trading-tab placement isn't preferred.
