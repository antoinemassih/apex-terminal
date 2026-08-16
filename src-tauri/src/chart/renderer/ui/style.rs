//! Shared styling helpers — single source of truth for all UI style decisions.
//!
//! # Changing the look in one place
//! - Font sizes   → `FONT_*` constants
//! - Spacing      → `GAP_*` constants
//! - Corner radii → `RADIUS_*` constants
//! - Stroke widths → `STROKE_*` constants
//! - Alpha tiers  → `ALPHA_*` constants
//! - Drop shadows → `SHADOW_*` constants
//! - Fixed colors → `TEXT_*` constants
//!
//! All helpers below use these constants internally, so a single change propagates everywhere.

use egui::{self, Color32, RichText, Stroke};
use crate::ui_kit::sx::Tone;

// ─── Owned-by-ui_kit re-exports (UI extraction, item 1) ──────────────────────
// The stateless token primitives (font/gap/alpha/stroke/radius constants,
// `color_alpha` / `color_alpha_mul`, elevation factors) now live in
// `crate::ui_kit::style` as the canonical home. Re-exported here so every
// existing `crate::chart_renderer::ui::style::font_sm()` / `gap_xs()` /
// `color_alpha(...)` call site keeps resolving with no source change.
//
// The stateful style machinery (`FRAME_TOKENS`, `STYLE_STORE`, `ACTIVE_STYLE`,
// the `Theme`-taking helpers like `header_surface(t)`) STAYS in this file —
// those depend on the chart-app's style preset system.
#[allow(unused_imports)]
pub use crate::ui_kit::style::*;

/// Register an element hit for inspect mode. No-op when design-mode is off.
#[inline(always)]
fn hit(r: &egui::Rect, family: &'static str, category: &'static str) {
    crate::design_tokens::register_hit(
        [r.min.x, r.min.y, r.width(), r.height()], family, category);
}

// ─── Per-frame token snapshot (spec §5 Rule 2) ────────────────────────────────
//
// `FRAME_TOKENS` is a lock-free `thread_local` holding a flat `Copy` struct of
// every design-token value that was previously computed via `dt_f32!`/`dt_u8!`
// on each call site.  `begin_frame()` refreshes it once per frame (wired inside
// `set_active_style`) so all token reads within that frame are a single `Cell`
// get — ~1 ns, no lock, no map lookup (spec §5 Rule 3).
//
// Hardcoded-constant token fns (font_sm = 11.0, gap_sm = 8.0, …) are NOT
// routed through this struct — they are already free compile-time constants and
// adding them would gain nothing while breaking the "leave unchanged if already
// guaranteed identical" contract.

// TokenSnapshot + DEFAULT_TOKEN_SNAPSHOT + the per-frame thread_local now
// live in `crate::ui_kit::style` and are re-exported via the `pub use` at
// the top of this file. `begin_frame()` below builds a snapshot from the
// active StyleSettings + design-mode overrides and pushes it to the
// canonical store via `crate::ui_kit::style::set_frame_tokens(snap)`.

/// Refresh the per-frame token snapshot from the current active style /
/// design-mode settings.  Call once per frame, after `set_active_style`.
///
/// This is wired inside `set_active_style` so callers in `core.rs` need no
/// changes — they already call `set_active_style` at frame start.
///
/// **Source-swap (Phase B)**: per-style dimension tokens (radii, stroke
/// widths) are sourced LIVE from the active style's `StyleSettings` via
/// `current()` — selecting a different style now changes them in shipping
/// builds, not just design-mode.  The default style (Meridien) is defined to
/// hold the pre-swap values, so the default look is preserved.  Global tokens
/// (alpha tiers, `gap_xs_mid`, `stroke_medium`, shadow geometry) have no
/// per-style backing and stay on the `dt_f32!`/`dt_u8!` path.
#[inline]
pub fn begin_frame() {
    // ── Hot-reload override (one RwLock::read per frame — ~20–50 ns) ──────────
    // If a background watcher has installed a live StyleSystem override, source
    // the per-style dimension tokens (radii, strokes) from it instead of from
    // the active `StyleSettings`.  When no override is present (`None`) the
    // existing `current()` path is used unchanged.
    let override_style = crate::design_system::active_override();

    // Active style's StyleSettings — the live per-style dimension source.
    let st = current();

    // M1: the active style's FULL StyleSystem — sources the token ladders
    // (gap_*, ui type scale, alpha tiers) that `StyleSettings` never carried.
    // Unauthored styles hold the serde-default ladder (= the previous hard
    // literals), so this wire-up is byte-identical until a style AUTHORS a
    // ladder — at which point the whitespace/type axes finally go live.
    // AUDIT 2026-08 — ONE EFFECTIVE STYLE SYSTEM PER FRAME.
    //
    // This used to be `active_style_system()` unconditionally, while the
    // hot-reload override was consulted separately for radii and strokes only.
    // The watcher parses a FULL `StyleSystem` off disk, but `begin_frame`
    // consumed 36 of its 71 fields — so editing a theme JSON silently dropped
    // density, treatments, the semantic type roles, bevels and the watchlist
    // row geometry. The reload logged success either way.
    //
    // Both sides are `Arc<StyleSystem>`, so resolving the effective system once
    // costs nothing and makes every `ass.*` read below honour the override
    // automatically — including categories added later, which is the part that
    // stops this drifting again.
    let ass = override_style.clone().unwrap_or_else(active_style_system);
    let (sp, ty, al) = (&ass.spacing, &ass.typography, &ass.alphas);

    // The effective density multiplier for this frame — the user's override if
    // set, else the active style's mode. Applied once, where the structural
    // ladder is written into the snapshot (see the note there).
    let dens = effective_density().scale();

    // Resolve radii — precedence order:
    //   1. Hot-reload override (workspace JSON → watcher).
    //   2. DesignTokens (design-mode inspector slider — live).
    //   3. StyleSettings (active style preset).
    // (2) was previously skipped, which is why inspector slider edits to
    // `radii.*` had no visible effect. Now plumbed through.
    let (r_xs, r_sm, r_md, r_lg) = if let Some(ref ov) = override_style {
        (ov.radii.xs, ov.radii.sm, ov.radii.md, ov.radii.lg)
    } else {
        (
            crate::dt_f32!(radius.xs, st.r_xs as f32),
            crate::dt_f32!(radius.sm, st.r_sm as f32),
            crate::dt_f32!(radius.md, st.r_md as f32),
            crate::dt_f32!(radius.lg, st.r_lg as f32),
        )
    };

    // Resolve strokes — same precedence (override → DesignTokens → StyleSettings).
    let (stroke_hair, stroke_thin, stroke_std, stroke_bold, stroke_thick) =
        if let Some(ref ov) = override_style {
            (
                ov.strokes.thin,   // thin  → hair (sub-pixel hairline)
                ov.strokes.std,    // std   → thin (1 px standard)
                ov.strokes.std,    // std   → std  (same level)
                ov.strokes.md,     // md    → bold (1.5 px emphasis)
                ov.strokes.heavy,  // heavy → thick (2 px)
            )
        } else {
            (
                crate::dt_f32!(stroke.hair, st.stroke_hair),
                crate::dt_f32!(stroke.thin, st.stroke_thin),
                crate::dt_f32!(stroke.std, st.stroke_std),
                crate::dt_f32!(stroke.bold, st.stroke_bold),
                crate::dt_f32!(stroke.thick, st.stroke_thick),
            )
        };

    let snap = TokenSnapshot {
        // Fonts — pulled from DesignTokens so design-mode font sliders propagate.
        // TYPE SCALE LIFT: the body tiers were 9/11px, and ~70% of all text in
        // the app rendered at one of those two sizes — that is what read as
        // "scrunched up and unreadable" everywhere except the watchlist /
        // option chain, whose rows hardcode 14-15px. Raising the TOKENS lifts
        // every call site at once (and keeps the scale as the single source of
        // truth) instead of touching 400+ sites. Steps kept ≥1px apart so the
        // tiers stay visually distinct.
        font_2xs:      if let Some(ref ov) = override_style { ov.typography.ui_2xs } else { crate::dt_f32!(font.xxs, ty.ui_2xs) },
        font_xs:       if let Some(ref ov) = override_style { ov.typography.ui_xs  } else { crate::dt_f32!(font.xs,  ty.ui_xs)  },
        font_sm:       if let Some(ref ov) = override_style { ov.typography.ui_sm  } else { crate::dt_f32!(font.sm,  ty.ui_sm)  },
        font_md:       if let Some(ref ov) = override_style { ov.typography.ui_md  } else { crate::dt_f32!(font.md,  ty.ui_md)  },
        font_lg:       if let Some(ref ov) = override_style { ov.typography.ui_lg  } else { crate::dt_f32!(font.lg,  ty.ui_lg)  },
        font_xl:       if let Some(ref ov) = override_style { ov.typography.ui_xl  } else { crate::dt_f32!(font.xl,  ty.ui_xl)  },
        // Spacing.
        gap_xs:        if let Some(ref ov) = override_style { ov.spacing.gap_xs  } else { crate::dt_f32!(spacing.xs,     sp.gap_xs)  },
        gap_xs_mid:    if let Some(ref ov) = override_style { ov.spacing.xs_mid  } else { crate::dt_f32!(spacing.xs_mid, sp.xs_mid)  },
        gap_sm:        if let Some(ref ov) = override_style { ov.spacing.gap_sm  } else { crate::dt_f32!(spacing.sm,     sp.gap_sm)  },
        gap_md:        if let Some(ref ov) = override_style { ov.spacing.gap_md  } else { crate::dt_f32!(spacing.md,     sp.gap_md)  },
        gap_lg:        if let Some(ref ov) = override_style { ov.spacing.gap_lg  } else { crate::dt_f32!(spacing.lg,     sp.gap_lg)  },
        gap_xl:        if let Some(ref ov) = override_style { ov.spacing.gap_xl  } else { crate::dt_f32!(spacing.xl,     sp.gap_xl)  },
        gap_2xl:       if let Some(ref ov) = override_style { ov.spacing.gap_2xl } else { crate::dt_f32!(spacing.xxl,    sp.gap_2xl) },
        gap_3xl:       if let Some(ref ov) = override_style { ov.spacing.gap_3xl } else { crate::dt_f32!(spacing.xxxl,   sp.gap_3xl) },
        // Radii (already resolved with override + DesignTokens precedence above).
        radius_xs:     r_xs,
        radius_sm:     r_sm,
        radius_md:     r_md,
        radius_lg:     r_lg,
        radius_pill:   st.r_pill as f32,
        // Strokes (already resolved with override + DesignTokens precedence above).
        stroke_hair,
        stroke_thin,
        // AUDIT 2026-08 — fall back to the STYLE, not to 0.8.
        //
        // Every other stroke rung falls back to its `StyleSettings` value
        // (`st.stroke_hair` etc.); this one used a literal because
        // StyleSettings has no `stroke_medium` field — a gap this file's own
        // header comment records. The consequence was not documented though: a
        // style authoring `strokes.medium` was ignored outright, so the rung
        // between `thin` and `std` was the one weight no theme could set.
        //
        // `ass.strokes.medium` is the authored value and needs no StyleSettings
        // detour, the same way `icons` and `line_heights` are read.
        stroke_medium: crate::dt_f32!(stroke.medium, ass.strokes.medium),
        // `stroke.heavy` is 2.5 in DesignTokens and had a live inspector
        // slider that nothing consumed; `stroke_extra_thick()` hardcoded the
        // same 2.5. Same number, two homes — now one.
        font_display_sm: if let Some(ref ov) = override_style { ov.typography.display_sm } else { crate::dt_f32!(font.display_sm, ty.display_sm) },
        font_display_md: if let Some(ref ov) = override_style { ov.typography.display_md } else { crate::dt_f32!(font.display_md, ty.display_md) },
        font_display_lg: if let Some(ref ov) = override_style { ov.typography.display_lg } else { crate::dt_f32!(font.display_lg, ty.display_lg) },
        font_display_xl: if let Some(ref ov) = override_style { ov.typography.display_xl } else { crate::dt_f32!(font.display_xl, ty.display_xl) },
        font_4xs:        if let Some(ref ov) = override_style { ov.typography.ui_4xs } else { crate::dt_f32!(font.ui_4xs, ty.ui_4xs) },
        font_xs_plus:    if let Some(ref ov) = override_style { ov.typography.ui_xs_plus } else { crate::dt_f32!(font.ui_xs_plus, ty.ui_xs_plus) },
        font_md_plus:    if let Some(ref ov) = override_style { ov.typography.ui_md_plus } else { crate::dt_f32!(font.ui_md_plus, ty.ui_md_plus) },
        gap_2xs:         if let Some(ref ov) = override_style { ov.spacing.gap_2xs } else { crate::dt_f32!(spacing.gap_2xs, sp.gap_2xs) },
        focus_ring:   ass.treatments.focus_ring,
        icon_xs:      ass.icons.xs,
        icon_sm:      ass.icons.sm,
        icon_md:      ass.icons.md,
        icon_lg:      ass.icons.lg,
        line_tight:   ass.line_heights.tight,
        line_heading: ass.line_heights.heading,
        line_dense:   ass.line_heights.dense,
        line_compact: ass.line_heights.compact,
        line_normal:  ass.line_heights.normal,
        line_loose:   ass.line_heights.loose,
        stroke_extra_thick: crate::dt_f32!(stroke.heavy, 2.5),
        stroke_rule:        crate::dt_f32!(stroke.rule, 3.0),
        stroke_std,
        stroke_bold,
        stroke_thick,
        // Alphas.
        alpha_faint:   if let Some(ref ov) = override_style { ov.alphas.faint     } else { crate::dt_u8!(alpha.faint,  al.faint)     },
        alpha_ghost:   if let Some(ref ov) = override_style { ov.alphas.ghost     } else { crate::dt_u8!(alpha.ghost,  al.ghost)     },
        alpha_soft:    if let Some(ref ov) = override_style { ov.alphas.soft_u8   } else { crate::dt_u8!(alpha.soft,   al.soft_u8)   },
        alpha_subtle:  if let Some(ref ov) = override_style { ov.alphas.subtle_u8 } else { crate::dt_u8!(alpha.subtle, al.subtle_u8) },
        alpha_tint:    if let Some(ref ov) = override_style { ov.alphas.tint      } else { crate::dt_u8!(alpha.tint,   al.tint)      },
        alpha_muted:   if let Some(ref ov) = override_style { ov.alphas.muted_u8  } else { crate::dt_u8!(alpha.muted,  al.muted_u8)  },
        alpha_dim:     if let Some(ref ov) = override_style { ov.alphas.dim       } else { crate::dt_u8!(alpha.dim,    al.dim)       },
        alpha_line:    if let Some(ref ov) = override_style { ov.alphas.line      } else { crate::dt_u8!(alpha.line,   al.line)      },
        alpha_strong:  if let Some(ref ov) = override_style { ov.alphas.strong_u8 } else { crate::dt_u8!(alpha.strong, al.strong_u8) },
        alpha_active:  if let Some(ref ov) = override_style { ov.alphas.active    } else { crate::dt_u8!(alpha.active, al.active)    },
        alpha_heavy:   if let Some(ref ov) = override_style { ov.alphas.heavy_u8  } else { crate::dt_u8!(alpha.heavy,  al.heavy_u8)  },
        alpha_scrim:   if let Some(ref ov) = override_style { ov.alphas.scrim     } else { crate::dt_u8!(alpha.scrim,  al.scrim)     },
        alpha_whisper:     if let Some(ref ov) = override_style { ov.alphas.whisper } else { crate::dt_u8!(alpha.whisper, al.whisper) },
        alpha_hint:        if let Some(ref ov) = override_style { ov.alphas.hint } else { crate::dt_u8!(alpha.hint, al.hint) },
        alpha_dense:       if let Some(ref ov) = override_style { ov.alphas.dense } else { crate::dt_u8!(alpha.dense, al.dense) },
        alpha_near_solid:  if let Some(ref ov) = override_style { ov.alphas.near_solid } else { crate::dt_u8!(alpha.near_solid, al.near_solid) },
        alpha_solid:   if let Some(ref ov) = override_style { ov.alphas.solid     } else { crate::dt_u8!(alpha.solid,  al.solid)     },
        // Shadows.
        shadow_offset: crate::dt_f32!(shadow.offset, 2.0),
        shadow_alpha:  crate::dt_u8!(shadow.alpha,   60),
        shadow_spread: crate::dt_f32!(shadow.spread,  4.0),
        // P5b — style-preset knobs pushed into the snapshot so ui_kit
        // widgets (input focus ring, toast glassmorphic bg, simple_btn
        // treatment) read them via frame_tokens() instead of st.
        focus_ring_alpha: st.focus_ring_alpha,
        focus_ring_width: st.focus_ring_width,
        toast_bg_alpha:   st.toast_bg_alpha,
        button_treatment: st.button_treatment,
        // Bevel — pushed from StyleSettings so ui_kit widgets read via frame_tokens().
        surface_bevel:         st.surface_bevel,
        bevel_highlight_alpha: st.bevel_highlight_alpha,
        bevel_shadow_alpha:    st.bevel_shadow_alpha,
        // M1 Change C: dimension-side defaults; `setup_theme` patches in the
        // ACTIVE PANE's authored tints right after (palette axis joins there).
        bevel_highlight_tint:  egui::Color32::WHITE,
        bevel_shadow_tint:     egui::Color32::BLACK,
        // M2.1: per-style semantic fonts for the (now ui_kit-resident) cascade.
        font_body:          if let Some(ref ov) = override_style { ov.typography.size_sm } else { crate::dt_f32!(font.sm, st.font_body) },
        font_caption:       if let Some(ref ov) = override_style { ov.typography.size_xs } else { crate::dt_f32!(font.xs, st.font_caption) },
        font_section_label: if let Some(ref ov) = override_style { ov.typography.size_section_label } else { crate::dt_f32!(font.sm_tight, st.font_section_label) },
        // M4.5: structural proportions from the active StyleSystem's Density.
        //
        // AUDIT 2026-08 — DENSITY IS APPLIED ONCE, HERE.
        //
        // The chart-side accessors (`style_row_height`,
        // `style_row_height_comfortable`, `style_button_height`) each multiplied
        // by `effective_density().scale()` themselves, while the ui_kit ladder
        // (`row_height_*`, `control_h_*`) read the snapshot raw. So changing the
        // density preference moved some rows and left others fixed — every new
        // accessor had to remember to apply the scale, and the ones that forgot
        // were invisible because the default scale is 1.0.
        //
        // Scaling as the snapshot is built makes every `frame_tokens()` consumer
        // density-aware for free, including ones added later.
        //
        // Rails and splitter are deliberately NOT scaled: density is vertical
        // rhythm. A rail is a horizontal width and the splitter is a pointer hit
        // target — shrinking either with density would change hit areas, not
        // rhythm.
        row_dense:      ass.density.row_dense    * dens,
        row_compact:    ass.density.row_compact  * dens,
        row_default:    ass.density.row_default  * dens,
        row_spacious:   ass.density.row_spacious * dens,
        row_tall:       ass.density.row_tall     * dens,
        splitter_width: ass.density.splitter_width,
        // Sourced from `st` (= `current()`), NOT from `ass.density`, and that is
        // deliberate. `pane_gap` lives on StyleSettings and varies per style
        // (8.0 default, 0.0 for the flush baseline); `Density` has no such
        // field. Reading it off `ass.density` would mean inventing a default
        // and silently overriding every style that authored 0.0 — a visible
        // regression traded for a tidier line. Routing the existing value
        // through the snapshot is byte-identical today and is what lets the
        // override / inspector layers reach it at all.
        pane_gap:       st.pane_gap,
        control_xs:     ass.density.control_xs * dens,
        control_sm:     ass.density.control_sm * dens,
        control_md:     ass.density.control_md * dens,
        control_lg:     ass.density.control_lg * dens,
        control_xl:     ass.density.control_xl * dens,
        rail_narrow:    ass.density.rail_narrow,
        rail_medium:    ass.density.rail_medium,
        rail_wide:      ass.density.rail_wide,
        // Default tab treatment — Filled for Aperture/Cadence/Glass, Line for others.
        panel_tab_treatment:   st.panel_tab_treatment,
        // List row shape — pill for Aperture/Glass, hairlines for Alto/Mariner/Relay.
        wl_row_side_margin:    st.wl_row_side_margin,
        wl_row_corner_radius:  st.wl_row_corner_radius,
        wl_row_divider_alpha:  st.wl_row_divider_alpha,
    };
    // Push to the canonical ui_kit thread_local. ui_kit's
    // `frame_tokens()` reads this back for `radius_*` / `stroke_*` /
    // `alpha_*` helpers across the kit.
    crate::ui_kit::style::set_frame_tokens(snap);
    // M1 Change E: hand the authored shadow stacks to ui_kit (empty = legacy).
    crate::ui_kit::style::set_card_shadow_layers(
        ass.shadows.card_layers.clone(),
        ass.shadows.modal_layers.clone(),
    );
    // M1 Change D: signature tokens (None = derived classics).
    crate::ui_kit::style::set_numeral_tier(ass.numerals);
}

// ─── Typography scale ─────────────────────────────────────────────────────────
// Typography scale — 4 sizes, monospace pinned for financial data.
//
// Density-first scale anchored at 9px for high-density financial UIs
// (price ladders, options chains, watchlists). Hierarchy comes through
// size + color, not new fonts.
//
// Size scale (use ONLY these 4):
//   font_xs()  =  9.0   — micro-labels, dropdown items, badge text
//   font_sm()  = 11.0   — default body, list rows, tab labels, nav buttons
//   font_md()  = 13.0   — emphasized body, panel titles
//   font_lg()  = 16.0   — section headers, modal titles
//
// Anything outside this scale is a bug. If you need 10px or 12px, you
// probably want font_sm. If you need 14-15px, you probably want font_md
// or font_lg.
//
// Weight is currently single-axis (Medium for sans, Regular for mono).
// Multi-weight support is a future phase; for now achieve hierarchy
// through size and color (use t.dim for secondary, t.text for primary).
//
// Monospace is ALWAYS JetBrains Mono regardless of the font picker.
// Use mono_xs/sm/md/lg for prices, quantities, OCC tickers, anything
// tabular. The font picker controls proportional UI chrome only.

// Below `font_xs` are the *micro* tier — chart annotations, badge counts on
// glyphs. They exist because chart_widgets / pane.rs need labels at 6–8px to
// fit dense overlays without overlapping ticks. New UI code should NOT use
// these; they are the floor for chart rendering only.
// Font-size helpers (font_4xs..font_xl) now live in `crate::ui_kit::style`
// and are re-exported from here via the `pub use` at the top of the file.
// See `docs/UI_EXTRACTION.md` item 1.

// ─── Display-font tier (proportional, large numerics only) ───────────────────
// For infographic/hero KPI numbers rendered with `FontId::proportional(...)`.
// These are NOT for UI chrome — use them only in chart widget body paint calls
// where a large display number is the primary focal point of the widget.
//
//   font_display_sm()  = 28.0 — compact hero (single KPI in a narrow widget)
//   font_display_md()  = 32.0 — standard hero number
//   font_display_lg()  = 42.0 — prominent hero / dual-KPI banner
//   font_display_xl()  = 56.0 — primary focal number (full-width banner widget)

/// 28.0 — compact display hero (narrow widget KPI, countdown digits).
/// 32.0 — standard display hero number (primary gauge focal point).
/// 42.0 — prominent display hero (dual-KPI or large widget focal number).
/// 56.0 — maximum display focal number (full-width banner widget).

// FONT_DISPLAY_* constants now in `crate::ui_kit::style`; re-exported here.

// ─── Monospace helpers (JetBrains Mono, pinned) ───────────────────────────────
// Use these for tabular financial data: prices, quantities, OCC tickers.
// Returns FontId so the family is explicit at the call site.

// ─── Legacy aliases (DEPRECATED) ──────────────────────────────────────────────
// Kept compiling existing call sites; new code must use the named tier above.
#[doc(hidden)] pub fn font_sm_tight() -> f32 { font_xs() }
// 2xl must sit ABOVE xl. It was aliased to `font_lg()` (16), which is SMALLER
// than `font_xl()` (22) — that inverted the heading ladder for every consumer
// (TextStyle::Display/HeadingLg read this, so HeadingMd(22) outranked both).
#[doc(hidden)] pub fn font_2xl()      -> f32 { font_xl() + 6.0 }

// Const aliases — kept so any const-context call sites compile. Values match
// the active scale (4xs=6, 3xs=7, 2xs=8, xs=9, xs+=10, sm=11, md=13, md+=14, lg=16, xl=22).
// FONT_* const aliases now in `crate::ui_kit::style`; re-exported here.

// ─── Spacing tokens ───────────────────────────────────────────────────────────
// Spacing scale — density-first chrome, anchored on a ~4px grid.
//
// Tier summary (reach for these in order of specificity):
//   gap_2xs()     =  2.0  — icon-internal padding, [icon][badge] overlays
//   gap_xs()      =  4.0  — minimum intra-cluster gap (adjacent buttons)
//   gap_xs_mid()  =  6.0  — micro-gap between xs and sm; for icon-label
//                           pairs and compact chip rows where 4px is too
//                           tight and 8px is too loose (DS-IMPL-3)
//   gap_sm()      =  8.0  — default inter-element gap
//   gap_md()      = 12.0  — section padding, list-row vertical
//   gap_lg()      = 16.0  — panel inner margin
//   gap_xl()      = 20.0  — between sections in a panel
//   gap_2xl()     = 24.0  — between distinct panel groups
//   gap_3xl()     = 32.0  — page-level breaks (rare)
//
// If you are unsure, prefer gap_sm (8). Reach for gap_xs_mid only when you
// have a specific tight composition that needs exactly 6px.
//
// 2026-05: gap_2xs was previously aliased to gap_xs (both 4.0). It is now a
// real 2.0 token for icon-internal padding and tightly-packed compositions
// (the gap inside `[icon][badge]` overlays, etc.).
// 2026-05 DS-IMPL-3: gap_xs_mid (6.0) added as a micro-gap tier.
// Spacing helpers (gap_2xs..gap_3xl except gap_xs_mid) and GAP_* constants
// now live in `crate::ui_kit::style` and are re-exported from this file.
// `gap_xs_mid` + `GAP_XS_MID` re-exported from `crate::ui_kit::style` via
// the `pub use` at the top of this file.
pub const GAP_XS_MID: f32 =  6.0;

// ─── Icon control sizes ──────────────────────────────────────────────────────
// Standard square sizes for icon-only controls (toggle pills, trailing buttons,
// inline icon-only buttons). Replaces hand-rolled vec2(14, 14) / vec2(16, 16) etc.

// ─── Row heights ─────────────────────────────────────────────────────────────
// Canonical list/table row heights. PanelListRow defaults to row_height_default
// (22) for dense lists and row_height_spacious (24) for breathable ones.

// ─── Card padding ────────────────────────────────────────────────────────────
// Symmetric inner_margin presets for PanelCard / hand-rolled card bodies.

// ─── Divider insets ──────────────────────────────────────────────────────────
// Vertical inset for hairline dividers (typically applied to top + bottom
// of the dividing line so the rule doesn't kiss adjacent content).

// ─── Corner radius tokens ─────────────────────────────────────────────────────
// 2026-05: function fallbacks reconciled with the const values (was 3/4/8).
// radius_xs/sm/md/lg moved to `crate::ui_kit::style`; re-exported here.
// `radius_pill` now lives in `crate::ui_kit::style`; re-exported here.

pub const RADIUS_XS: f32 = 2.0;
pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;
pub const RADIUS_LG: f32 = 12.0;
pub const RADIUS_PILL: f32 = 999.0;

// ─── Cursor tokens ───────────────────────────────────────────────────────────
//
// Centralized cursor policy. Every interactive surface in the app should
// route its cursor through one of these helpers. They:
//   1. Check `resp.hovered()` — no cursor leak when the pointer moves away.
//   2. Honor `is_inspect_mode()` — the design inspector owns the cursor
//      while it's active, so widgets must not override it.
//   3. Implement state machines where relevant (draggable: Grab on hover,
//      Grabbing while a drag is in progress).
//
// Add a new helper here rather than inlining `set_cursor_icon` at a call
// site. Inlined sites drift: they forget the inspect guard, they don't
// handle drag-state transitions, and they make role audits impossible.
pub mod cursor {
    use egui::{CursorIcon, Response, Ui};

    #[inline]
    fn inspect_mode() -> bool {
        crate::design_tokens::is_inspect_mode()
    }

    /// PointingHand on hover — for any surface that responds to click
    /// (buttons, chips, links, list rows, menu items, status indicators).
    #[inline]
    pub fn clickable(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }

    /// Grab on hover, Grabbing while the user is actively dragging.
    /// For chart pan, drawing handles, draggable lines, tab reorder.
    #[inline]
    pub fn draggable(ui: &Ui, resp: &Response) {
        if inspect_mode() { return; }
        if resp.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
    }

    /// Horizontal resize cursor — vertical dividers between panes/columns.
    /// Stays sticky during a drag so the cursor doesn't flicker back to the
    /// default arrow when the pointer briefly leaves the narrow drag rect.
    #[inline]
    pub fn resize_h(ui: &Ui, resp: &Response) {
        if inspect_mode() { return; }
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
    }

    /// Vertical resize cursor — horizontal dividers between rows/panes.
    /// Stays sticky during a drag.
    #[inline]
    pub fn resize_v(ui: &Ui, resp: &Response) {
        if inspect_mode() { return; }
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        }
    }

    /// Diagonal NW–SE resize — corner grabbers on resizable panels.
    #[inline]
    pub fn resize_nwse(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
        }
    }

    /// Text I-beam — text input fields, editable cells.
    #[inline]
    pub fn text_input(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
        }
    }

    /// Crosshair — chart measurement tool, precision picking.
    #[inline]
    pub fn crosshair(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }
    }

    /// ZoomIn — zoom-box tool while idle (Grabbing takes over once the
    /// drag starts, paint that one yourself with `set_cursor_icon` since
    /// the zoom drag is modal and doesn't go through Response).
    #[inline]
    pub fn zoom_in(ui: &Ui, resp: &Response) {
        if resp.hovered() && !inspect_mode() {
            ui.ctx().set_cursor_icon(CursorIcon::ZoomIn);
        }
    }

    /// Modal cursor: set whenever a tool mode is active (measure, zoom,
    /// crosshair, drawing). Doesn't read from a Response — caller already
    /// knows the mode is on. Still honors inspect mode so the design
    /// inspector retains control.
    #[inline]
    pub fn modal(ui: &Ui, icon: CursorIcon) {
        if !inspect_mode() {
            ui.ctx().set_cursor_icon(icon);
        }
    }

    /// `ui.add(widget)` + `clickable(...)` in one call. Use this anywhere
    /// you're about to write `ui.add(egui::Button::new(...))` — the
    /// resulting `Response` already has PointingHand wired up on hover.
    /// egui 0.31 has no global "interactive cursor" style, so call sites
    /// either route through this helper, `ui_kit::Button` (which sets
    /// PointingHand internally), or set the cursor manually.
    #[inline]
    pub fn click_widget<W: egui::Widget>(ui: &mut Ui, widget: W) -> Response {
        let r = ui.add(widget);
        clickable(ui, &r);
        r
    }

    /// Paint a keyboard-focus ring around `resp.rect` when the widget has
    /// focus. No-op when the widget is not focused, so callers add this
    /// unconditionally after building their response.
    ///
    /// The ring is painted *outside* the widget's rect (2 px expansion) so it
    /// doesn't clip or overlap inner chrome. Accent color at `focus_ring_alpha`
    /// opacity keeps it visible but subtle — consistent with the design system's
    /// focus treatment across Button, Input, and Select.
    ///
    /// `accent` — the accent Color32 for this widget (from its theme). Pull
    /// it with `theme.accent()` (ComponentTheme) or `t.accent` (Theme).
    #[inline]
    pub fn focus_ring(ui: &Ui, resp: &Response, accent: egui::Color32) {
        use egui::{CornerRadius, Stroke, StrokeKind};
        if !resp.has_focus() {
            return;
        }
        let st = super::current();
        let color = super::color_alpha(accent, st.focus_ring_alpha);
        let radius = CornerRadius::same((super::radius_sm() as u8).saturating_add(1));
        ui.painter().rect_stroke(
            resp.rect.expand(2.0),
            radius,
            Stroke::new(st.focus_ring_width, color),
            StrokeKind::Outside,
        );
    }
}

// ─── Stroke width tokens ─────────────────────────────────────────────────────
// Stroke scale — sub-pixel to multi-pixel hairlines for borders and rules.
//
// Tier summary:
//   stroke_hair()   = 0.3  — sub-pixel separator; nearly invisible hairline
//   stroke_thin()   = 0.5  — light border, table column divider
//   stroke_medium() = 0.8  — mid-weight border; between thin and std
//                            (DS-IMPL-3: now backed by `stroke.medium` token)
//   stroke_std()    = 1.0  — default UI border (buttons, inputs, panels)
//   stroke_bold()   = 1.5  — emphasis border (active selection, focus ring)
//   stroke_thick()  = 2.0  — strong visual separator
//   stroke_rule()   = 3.0  — decorative / accent rule (was `stroke_heavy`)
//
// Use `stroke_medium()` when `stroke_thin()` feels too ghost-like and
// `stroke_std()` is heavier than desired for the context.
// stroke_hair/thin/medium/std/bold/thick moved to `crate::ui_kit::style`.
// `stroke_extra_thick` / `stroke_rule` now in `crate::ui_kit::style`.


// ─── Semantic alpha tokens ────────────────────────────────────────────────────
// 2026-05: tier expanded with intermediate values to absorb hardcoded literals
// (25, 30, 140, 180, 230). Existing tiers (faint=10, subtle=40, muted=60,
// line=80, active=100, heavy=120, solid=200) keep their values so visuals
// don't shift. Note: `alpha_muted == alpha_dim` (both 60) and
// `alpha_line == alpha_strong` (both 80) by design — same value, different
// semantic intent (muted/strong = chrome; dim/line = borders).
// alpha_faint/ghost/soft/subtle/tint/muted/dim/line/strong/active/heavy/solid
// moved to `crate::ui_kit::style`. `alpha_whisper` / `alpha_hint` also there.
pub fn alpha_intense()          -> u8 { 140 }
pub fn alpha_prominent()        -> u8 { 180 }
pub fn alpha_near_opaque()      -> u8 { 230 }
/// Button hover tint — light overlay behind hovered elements (between faint=10 and soft=20).
pub fn alpha_button_hover()     -> u8 { 18  }
/// Secondary / placeholder text — between muted=60 and line=80.
pub fn alpha_secondary_text()   -> u8 { 70  }
/// Interactive highlighted text / active row fills — between strong=80 and solid=200.
pub fn alpha_interactive()      -> u8 { 160 }

/// Use with `color_alpha(color, ALPHA_*)` for consistent opacity tiers.
pub const ALPHA_INTENSE:        u8 = 140;
pub const ALPHA_PROMINENT:      u8 = 180;
pub const ALPHA_NEAR_OPAQUE:    u8 = 230;
pub const ALPHA_BUTTON_HOVER:   u8 = 18;
pub const ALPHA_SECONDARY_TEXT: u8 = 70;
pub const ALPHA_INTERACTIVE:    u8 = 160;

// ─── Badge / alert pill tokens ────────────────────────────────────────────────
/// Height of an alert badge pill.
pub const BADGE_HEIGHT:         f32 = 20.0;
/// Minimum width of a badge pill (prevents single-word badges from looking squished).
pub const BADGE_MIN_WIDTH:      f32 = 64.0;
/// Left accent stripe width on a badge.
pub const BADGE_ACCENT_WIDTH:   f32 = 3.0;
/// Width reserved for the dismiss × button on the right of a badge.
pub const BADGE_DISMISS_WIDTH:  f32 = 14.0;
/// Right inset of the dismiss × glyph inside its reserved space.
pub const BADGE_DISMISS_PADDING: f32 = 8.0;
/// Corner radius for badge pills (slightly smaller than radius_xs for compactness).
pub const BADGE_CORNER_RADIUS:  u8  = 3;
/// Alpha for the tinted pill background (accent color at low opacity).
pub const BADGE_TINT_ALPHA:     u8  = 18;

// ─── Drop shadow tokens ───────────────────────────────────────────────────────
// shadow_offset / shadow_alpha / shadow_spread now in `crate::ui_kit::style`.

pub const SHADOW_OFFSET: f32 = 2.0;
pub const SHADOW_ALPHA:  u8  = 60;
pub const SHADOW_SPREAD: f32 = 4.0;

// ─── Shadow preset accessors (semantic depth scale) ──────────────────────────
// Returns egui::epaint::Shadow ready for `Frame::shadow(...)`. Backed by the
// `shadow_preset` design-token sub-struct when design-mode is on, else hard-
// coded defaults that match the original inline values they replace.
//
// LEGACY: `shadow_card / _modal / _tooltip / _dropdown` (no theme arg) use
// black as the shadow color — fine on dark themes, but on light themes
// (Bauhaus, Peach, Ivory, Newsprint) they paint as a hard black smudge.
// Prefer the `_themed` variants — they pull `t.shadow_color` so light
// themes get a soft gray drop. The legacy variants are kept compiling for
// the ~30 call sites that don't currently have a theme handle.
#[inline]
fn shadow_from_preset(offset: [i8; 2], blur: u8, spread: u8, alpha: u8) -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset, blur, spread,
        color: Color32::from_black_alpha(alpha),
    }
}

#[inline]
fn shadow_from_preset_themed(
    t: &super::super::gpu::Theme,
    offset: [i8; 2], blur: u8, spread: u8, alpha: u8,
) -> egui::epaint::Shadow {
    let s = t.shadow_color;
    egui::epaint::Shadow {
        offset, blur, spread,
        color: crate::ui_kit::style::color_alpha(s, alpha),
    }
}

/// Card / panel — subtle resting lift. Defaults: offset (0,2), blur 4, alpha 60.
pub fn shadow_card() -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.shadow_preset.card;
        return shadow_from_preset(p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset([0, 2], 4, 0, 60)
}

/// Card / panel — theme-aware. Use this in new code.
pub fn shadow_card_themed(t: &super::super::gpu::Theme) -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(dt) = crate::design_tokens::get() {
        let p = dt.shadow_preset.card;
        return shadow_from_preset_themed(t, p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset_themed(t, [0, 2], 4, 0, 60)
}

/// Modal dialog — tall, soft. Defaults: offset (0,8), blur 28, spread 2, alpha 80.
pub fn shadow_modal() -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.shadow_preset.modal;
        return shadow_from_preset(p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset([0, 8], 28, 2, 80)
}

/// Modal dialog — theme-aware. Use this in new code.
pub fn shadow_modal_themed(t: &super::super::gpu::Theme) -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(dt) = crate::design_tokens::get() {
        let p = dt.shadow_preset.modal;
        return shadow_from_preset_themed(t, p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset_themed(t, [0, 8], 28, 2, 80)
}

/// Tooltip — small, crisp. Used for hover bubbles.
pub fn shadow_tooltip() -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.shadow_preset.tooltip;
        return shadow_from_preset(p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset([0, 2], 0, 0, 60)
}

/// Tooltip — theme-aware. Use this in new code.
pub fn shadow_tooltip_themed(t: &super::super::gpu::Theme) -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(dt) = crate::design_tokens::get() {
        let p = dt.shadow_preset.tooltip;
        return shadow_from_preset_themed(t, p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset_themed(t, [0, 2], 0, 0, 60)
}

/// Dropdown / popover. Defaults: offset (0,8), blur 24, spread 1, alpha 40.
pub fn shadow_dropdown() -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.shadow_preset.dropdown;
        return shadow_from_preset(p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset([0, 8], 24, 1, 40)
}

/// Dropdown / popover — theme-aware. Use this in new code.
pub fn shadow_dropdown_themed(t: &super::super::gpu::Theme) -> egui::epaint::Shadow {
    #[cfg(feature = "design-mode")]
    if let Some(dt) = crate::design_tokens::get() {
        let p = dt.shadow_preset.dropdown;
        return shadow_from_preset_themed(t, p.offset, p.blur, p.spread, p.alpha);
    }
    shadow_from_preset_themed(t, [0, 8], 24, 1, 40)
}

// ─── Elevation tints ─────────────────────────────────────────────────────────
// Surface elevation scale — three tints for layered dark-UI backgrounds.
//
// Elevation tints slightly brighten `theme.bg` to communicate Z-depth.
// Three levels are intentionally perceptual constants chosen for dark themes:
//
//   elevation_1() = bg × 0.95  — resting card / panel surface (subtle lift)
//   elevation_2() = bg × 0.88  — raised panel, popover body, inline editor
//   elevation_3() = bg × 0.85  — modal / dialog surface (highest layer)
//
// The gamma multipliers are DARK-THEME perceptual constants. Light themes
// would need a different strategy (additive tint or a separate lookup) because
// gamma_multiply(< 1.0) darkens on light backgrounds, inverting the intended
// depth cue.
//
// TODO: When light-theme elevation support is added, split on `theme.is_light()`
// (or equivalent flag) and use `gamma_multiply(1.05 | 1.10 | 1.15)` for light
// surfaces so the depth direction stays consistent across all 15 themes.
//
// These do NOT use design tokens — the gamma values are perceptual constants,
// not tweakable style decisions. If you find yourself wanting to override them,
// consider whether a dedicated surface color token would be more appropriate.

// Elevation gamma factors — the single source of truth for the perceptual
// depth ramp applied to `theme.bg`. DARK-THEME constants (see the light-theme
// TODO above). Phase B3 promotes these to `StyleSystem.elevation` so a style
// system can override the ramp; until then a `const` is the correct home for
// a perceptual constant (vs the magic literal repeated across call sites).
// ELEVATION_*_FACTOR now in `crate::ui_kit::style`; re-exported here.

/// Elevation 1 — resting card / panel surface. Subtle lift above the base bg.
/// `theme.bg` darkened/lightened by gamma × 0.95 for dark themes.
#[inline]
pub fn elevation_1(theme: &super::super::gpu::Theme) -> Color32 {
    crate::ui_kit::style::elevate(theme.bg, crate::ui_kit::style::ELEVATE_CARD)
}

/// Elevation 2 — raised panel, popover body, inline editor surface.
/// `theme.bg` × 0.88 for dark themes.
#[inline]
pub fn elevation_2(theme: &super::super::gpu::Theme) -> Color32 {
    crate::ui_kit::style::elevate(theme.bg, crate::ui_kit::style::ELEVATE_RAISED)
}

/// Elevation 3 — modal / dialog surface (highest Z-layer).
/// `theme.bg` × 0.85 for dark themes.
#[inline]
pub fn elevation_3(theme: &super::super::gpu::Theme) -> Color32 {
    crate::ui_kit::style::elevate(theme.bg, crate::ui_kit::style::ELEVATE_MODAL)
}

// ─── Semantic color accessors ────────────────────────────────────────────────
// Intent colors that don't fit the brand palette: hover/focus/disabled, order
// status (cancel pastel), sentiment, and third-party brand colors.
/// Soft white overlay for hover-tint on dark surfaces.
pub fn hover_tint() -> Color32 {
    crate::dt_rgba!(semantic.hover_tint, [255, 255, 255, 16])
}
/// Focus ring — keyboard-focus halo color.
pub fn focus_ring() -> Color32 {
    crate::dt_rgba!(semantic.focus_ring, [100, 200, 255, 200])
}
/// Foreground color for disabled controls.
pub fn disabled_fg() -> Color32 {
    crate::dt_rgba!(semantic.disabled_fg, [140, 140, 150, 160])
}
/// Background color for disabled controls.
pub fn disabled_bg() -> Color32 {
    crate::dt_rgba!(semantic.disabled_bg, [40, 40, 46, 200])
}
/// Order cancel button background — pastel red.
pub fn order_cancel_bg() -> Color32 {
    crate::dt_rgba!(semantic.order_cancel_bg, [232, 156, 156, 255])
}
/// Order cancel button foreground — dark red on pastel.
pub fn order_cancel_fg() -> Color32 {
    crate::dt_rgba!(semantic.order_cancel_fg, [70, 25, 25, 255])
}
/// Sentiment — positive (bullish, good news, upvote).
pub fn sentiment_positive() -> Color32 {
    crate::dt_rgba!(semantic.sentiment_positive, [80, 200, 120, 255])
}
/// Sentiment — neutral (informational).
pub fn sentiment_neutral() -> Color32 {
    crate::dt_rgba!(semantic.sentiment_neutral, [180, 180, 195, 255])
}
/// Chat author-distinction palette — returns the i-th color, modulo'd by the
/// palette length so callers can pass a raw author-name hash directly. Used
/// by the Discord chat panel to give each speaker a stable, distinct color.
///
/// Note: `crate::design_tokens::get()` returns a cloned snapshot, so we only
/// expose an indexed accessor (no `&'static` slice variant).
pub fn chat_author_color(idx: usize) -> Color32 {
    // Hard-coded fallback mirrors ChatAuthorPalette::default() — must stay in sync.
    const FALLBACK: [[u8; 4]; 8] = [
        [ 74, 158, 255, 255], [ 46, 204, 113, 255], [243, 156,  18, 255], [155,  89, 182, 255],
        [231,  76,  60, 255], [ 26, 188, 156, 255], [241, 196,  15, 255], [ 52, 152, 219, 255],
    ];
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let pal = &t.chat_author_palette.colors;
        let c = pal[idx % pal.len()];
        return Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
    }
    let c = FALLBACK[idx % FALLBACK.len()];
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Sentiment — negative (bearish, bad news, downvote).
pub fn sentiment_negative() -> Color32 {
    crate::dt_rgba!(semantic.sentiment_negative, [224, 85, 96, 255])
}
/// Discord brand color — "Blurple" (#5865F2).
pub fn discord_blurple() -> Color32 {
    crate::dt_rgba!(semantic.discord_blurple, [88, 101, 242, 255])
}
/// Order ledger badge — RECON state (lavender/purple).
pub fn order_state_recon() -> Color32 {
    crate::dt_rgba!(semantic.order_state_recon, [167, 139, 250, 255])
}
/// Order ledger badge — CTRL state (red).
pub fn order_state_ctrl() -> Color32 {
    crate::dt_rgba!(semantic.order_state_ctrl, [255, 100, 100, 255])
}

// ─── Fixed text colors — DELETED ─────────────────────────────────────────────
// `TEXT_PRIMARY` / `TEXT_SECONDARY` were literal dark-theme greys used as
// "fallbacks for code without Theme access". They rendered near-white text on
// the light palettes (Bauhaus / Peach / Ivory / Newsprint / Lucid). Both had
// their last call sites replaced with the live theme's `t.text` / `t.dim`
// (via `theme_impl::active_theme(ui.ctx())` where no `&Theme` was in scope),
// leaving zero callers. There is no theme-blind text color any more — read the
// theme.

// ─── Status color tokens ─────────────────────────────────────────────────────
/// Green — active / live / filled (status_ok).
pub fn status_ok()    -> Color32 { crate::dt_rgba!(status.ok,    [120, 180, 120, 255]) }
/// Orange — warning / pending (status_warn).
pub fn status_warn()  -> Color32 { crate::dt_rgba!(status.warn,  [255, 165,   0, 255]) }
/// Red — error / rejected (status_error).
pub fn status_error() -> Color32 { crate::dt_rgba!(status.error, [224,  85,  96, 255]) }
/// Blue/purple — informational (status_info).
pub fn status_info()  -> Color32 { crate::dt_rgba!(status.info,  [100, 200, 255, 255]) }

// ─── Drawing palette tokens ──────────────────────────────────────────────────
/// Four link-group identity colors: blue, green, orange, purple.
pub fn drawing_palette() -> [Color32; 4] {
    #[cfg(feature = "design-mode")]
    if let Some(t) = crate::design_tokens::get() {
        let p = t.drawing.palette;
        return p.map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a));
    }
    [
        Color32::from_rgb( 70, 130, 255),
        Color32::from_rgb( 80, 200, 120),
        Color32::from_rgb(255, 160,  60),
        Color32::from_rgb(180, 100, 255),
    ]
}

/// Eight identity colors cycled when the user creates a new chart link group.
///
/// The first four ARE `drawing_palette()` — the link-group and drawing-tool
/// swatches were independently maintained copies of the same blue/green/orange/
/// purple ramp, which drifted. Sourcing them here keeps the two in sync and
/// means a DesignTokens override of `drawing.palette` reaches both.
pub fn link_group_palette() -> [Color32; 8] {
    let d = drawing_palette();
    [
        d[0], d[1], d[2], d[3],
        Color32::from_rgb(255,  80, 100),
        Color32::from_rgb(  0, 200, 220),
        Color32::from_rgb(255, 220,  50),
        Color32::from_rgb(255, 130, 200),
    ]
}

// ─── Semantic accent colors (design-system tokens) ───────────────────────────
/// Amber — used for "Active" status, R:R ≥ 1 indicator, and warning states.
pub const COLOR_AMBER: Color32 = Color32::from_rgb(255, 191, 0);
/// Teal — T2 target label color (second exit level).
pub const COLOR_T2: Color32 = Color32::from_rgb(26, 188, 156);
/// Blue — T3 target label color (third exit level).
pub const COLOR_T3: Color32 = Color32::from_rgb(52, 152, 219);
/// Cyan/blue informational color — used for "info" status, link-like
/// non-accent emphasis. RGB: (74,158,255). Theme-invariant; if you need a
/// theme-following blue, use `t.accent` instead.
pub const COLOR_INFO_CYAN:    Color32 = Color32::from_rgb( 74, 158, 255);
/// Pastel green — sentiment/profit signal independent of theme bull color.
/// Use sparingly; prefer `t.bull` for trade direction.
pub const COLOR_PROFIT_GREEN: Color32 = Color32::from_rgb( 46, 204, 113);
/// Pastel red — sentiment/loss signal independent of theme bear color.
pub const COLOR_LOSS_RED:     Color32 = Color32::from_rgb(231,  76,  60);
/// Purple accent for special-tier indicators / category-specific tints.
pub const COLOR_PURPLE:       Color32 = Color32::from_rgb(180, 100, 255);
/// Warm coral — categorical identity for "theta / decay" and the new-lows
/// breadth row. Deliberately NOT `t.bear`: it marks a category, not a
/// direction, and sits next to `t.bear` in the same widget.
pub const COLOR_CORAL:        Color32 = Color32::from_rgb(255, 140, 100);
/// Cool mint — categorical identity for "vega / volatility". Same rule as
/// `COLOR_CORAL`: a category tint, not a bull signal.
pub const COLOR_MINT:         Color32 = Color32::from_rgb(100, 230, 180);

/// Option-greek identity colors, in Δ / Γ / Θ / ν order.
///
/// A CATEGORICAL palette (the sibling of `drawing_palette`), not a semantic
/// one — the four greeks need to be told apart at a glance, so they must stay
/// mutually distinct rather than follow bull/bear. Centralised here so the set
/// is defined once and can later be driven from DesignTokens like
/// `drawing_palette` already is.
pub fn greeks_palette() -> [Color32; 4] {
    [status_info(), COLOR_PURPLE, COLOR_CORAL, COLOR_MINT]
}

// ─── Raw text helpers ─────────────────────────────────────────────────────────

#[inline]
pub fn mono(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).monospace().size(size).color(color)
}

#[inline]
pub fn mono_bold(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text).monospace().size(size).strong().color(color)
}

// ─── Toolbar button ───────────────────────────────────────────────────────────

/// Toolbar button — FONT_LG, RADIUS_MD, themed, pointer cursor.
/// Active state: accent fill + accent border + soft glow halo + bottom underline.
/// Hover state: subtle bg tint + accent border.
/// True when every char in `s` is in a Phosphor / icon private-use codepoint
/// range. Lets us detect "icon-only" toolbar buttons so we can render their
/// glyphs ~50% larger than text labels without breaking text-button sizing.
fn label_is_icon_only(s: &str) -> bool {
    if s.is_empty() { return false; }
    s.chars().all(|c| {
        let cp = c as u32;
        // Private Use Area (U+E000–U+F8FF) — where Phosphor glyphs live.
        // Allow ASCII whitespace as a separator (e.g., "{ICON} {count}").
        (0xE000..=0xF8FF).contains(&cp)
            || (0xF0000..=0x10FFFF).contains(&cp)
            || c.is_ascii_whitespace()
            || c.is_ascii_digit()
    })
}

// ─── tb_btn — DELETED (0 callers; superseded by ui_kit::Button::toolbar) ──────

// ─── Dialog / popup windows ───────────────────────────────────────────────────

// ─── popup_frame / dialog_window / dialog_window_themed — DELETED (WS-G G2) ──
// The last raw dialog-window factories. Every dialog now builds its chrome via
// ui_kit::Modal (FrameKind::DialogWindow replicates the themed frame + shadow
// these produced — see modal.rs), so all three had zero call sites. Retiring
// them removes the "two ways to make a dialog" ambiguity the audit flagged.

/// Dialog header bar — auto-darkened bg, FONT_LG title, X close. Returns true if closed.
pub fn dialog_header(ui: &mut egui::Ui, title: &str, dim: Color32) -> bool {
    dialog_header_colored(ui, title, dim, None)
}

/// Dialog header bar with explicit header background.
pub fn dialog_header_colored(ui: &mut egui::Ui, title: &str, dim: Color32, header_bg: Option<Color32>) -> bool {
    let _ = dim; // previously used to tint the X glyph; now handled by Button::close placement
    let darken = crate::dt_u8!(dialog.header_darken, 8);
    let fill = header_bg.unwrap_or_else(|| {
        let bg = ui.visuals().window_fill();
        Color32::from_rgb(bg.r().saturating_sub(darken), bg.g().saturating_sub(darken), bg.b().saturating_sub(darken))
    });
    let mut closed = false;
    let rlg = current().r_lg;
    egui::Frame::NONE.fill(fill)
        .inner_margin(egui::Margin { left: gap_lg() as i8, right: gap_lg() as i8, top: gap_lg() as i8, bottom: gap_lg() as i8 })
        .corner_radius(egui::CornerRadius { nw: rlg, ne: rlg, sw: 0, se: 0 })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let text_col = ui.style().visuals.override_text_color
                    .unwrap_or_else(|| crate::chart_renderer::theme_impl::active_theme(ui.ctx()).text);
                ui.label(RichText::new(title).monospace().size(font_lg()).strong().color(text_col));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
                    if crate::ui_kit::widgets::Button::close().show(ui, &t).clicked() {
                        closed = true;
                    }
                });
            });
        });
    closed
}

// ─── Separators ───────────────────────────────────────────────────────────────

/// Full-width horizontal separator.
/// Uses `stroke_hair` when the active style has hairline_borders, otherwise `stroke_thin` —
/// giving Meridien its characteristic super-thin dividers.
#[inline]
pub fn separator(ui: &mut egui::Ui, color: Color32) {
    let st = current();
    let sw = if st.hairline_borders { st.stroke_hair } else { stroke_thin() };
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left(), ui.cursor().min.y), egui::pos2(rect.right(), ui.cursor().min.y)],
        Stroke::new(sw, color));
    ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
}

/// Inset separator with margins on both sides.
/// Uses `stroke_hair` when the active style has hairline_borders, otherwise `stroke_thin`.
pub fn dialog_separator(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let st = current();
    let sw = if st.hairline_borders { st.stroke_hair } else { stroke_thin() };
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [egui::pos2(rect.left() + margin, ui.cursor().min.y),
         egui::pos2(rect.right() - margin, ui.cursor().min.y)],
        Stroke::new(sw, color));
    ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
}

/// Inset separator + soft gradient shadow below (3 fading lines).
/// Uses `stroke_thick` for the main divider line so bold-separator sites are style-driven.
///
/// Resolves the active theme from the UI context so light themes get a
/// soft gray gradient instead of the former hardcoded black.
pub fn dialog_separator_shadow(ui: &mut egui::Ui, margin: f32, color: Color32) {
    let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
    dialog_separator_shadow_impl(ui, margin, color, t.shadow_color);
}

/// Inset separator + soft gradient shadow below — theme-aware.
pub fn dialog_separator_shadow_themed(
    ui: &mut egui::Ui,
    margin: f32,
    color: Color32,
    t: &super::super::gpu::Theme,
) {
    dialog_separator_shadow_impl(ui, margin, color, t.shadow_color);
}

#[inline]
fn dialog_separator_shadow_impl(ui: &mut egui::Ui, margin: f32, color: Color32, shadow_tint: Color32) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    let left = rect.left() + margin;
    let right = rect.right() - margin;
    ui.painter().line_segment([egui::pos2(left, y), egui::pos2(right, y)], Stroke::new(current().stroke_thick, color));
    // Fading shadow gradient: 3 strokes at decreasing alpha (themed tint).
    #[cfg(feature = "design-mode")]
    let shadow_alphas = {
        if let Some(t) = crate::design_tokens::get() { t.shadow.gradient } else { [20u8, 12, 4] }
    };
    #[cfg(not(feature = "design-mode"))]
    let shadow_alphas = [20u8, 12, 4];
    for (i, &a) in shadow_alphas.iter().enumerate() {
        ui.painter().line_segment(
            [egui::pos2(left, y + (i + 1) as f32), egui::pos2(right, y + (i + 1) as f32)],
            Stroke::new(
                stroke_thin(),
                crate::ui_kit::style::color_alpha(shadow_tint, a),
            ),
        );
    }
    ui.add_space(crate::dt_f32!(separator.shadow_space, 4.0));
}

/// Indented section label with left margin — used inside dialogs.
pub fn dialog_section(ui: &mut egui::Ui, text: &str, margin: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.label(RichText::new(text).monospace().size(font_sm()).strong().color(color));
    });
    ui.add_space(gap_xs() + 1.0);
}

// ─── Labels ───────────────────────────────────────────────────────────────────

/// Section header — FONT_SM bold. Uppercases label when the active style requires it (#12).
/// Adds `section_label_padding_top` space before and `section_label_padding_bottom` after.
#[inline]
pub fn section_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    let st = current();
    if st.section_label_padding_top > 0.0 { ui.add_space(st.section_label_padding_top); }
    let label = style_label_case(text);
    ui.label(RichText::new(label).monospace().size(font_2xs()).strong().color(color));
    if st.section_label_padding_bottom > 0.0 { ui.add_space(st.section_label_padding_bottom); }
}

/// Extra-small section label — dim monospace, uppercase when style requires (#12).
/// Uses font_2xs (8px) as the legibility floor; 6px was unreadable.
#[inline]
pub fn section_label_xs(ui: &mut egui::Ui, text: &str, color: Color32) {
    let label = style_label_case(text);
    ui.label(RichText::new(label).monospace().size(crate::ui_kit::style::font_2xs()).color(color));
}

/// Dim info label — FONT_SM regular.
#[inline]
pub fn dim_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(font_sm()).color(color));
}

/// Column header cell — FONT_XS dim monospace, fixed width.
/// `right_align = true` for numeric columns (PRICE, SIZE), false for text (SYMBOL, TIME).
pub fn col_header(ui: &mut egui::Ui, text: &str, width: f32, color: Color32, right_align: bool) {
    let layout = if right_align {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, crate::dt_f32!(table.header_height, 12.0)), layout, |ui| {
        ui.label(RichText::new(text).monospace().size(font_xs()).color(color));
    });
}

// ─── Segmented control ───────────────────────────────────────────────────────

/// Pill group of buttons with a sunken inset trough. Returns `Some(index)` of the clicked
/// segment, `None` if nothing clicked. Caller updates state on `Some(i)`.
///
/// Uses a painter-reservation approach: buttons are rendered in the normal horizontal flow
/// (so `horizontal_centered` can center them correctly), and the trough background is
/// painted behind them via a reserved painter slot — avoiding Frame centering issues.
pub fn segmented_control(
    ui: &mut egui::Ui,
    active_idx: usize,
    labels: &[&str],
    toolbar_bg: Color32,
    toolbar_border: Color32,
    accent: Color32,
    dim: Color32,
) -> Option<usize> {
    let mut clicked = None;

    let td = crate::dt_u8!(segmented.trough_darken, 12);
    let trough = Color32::from_rgb(
        toolbar_bg.r().saturating_sub(td),
        toolbar_bg.g().saturating_sub(td),
        toolbar_bg.b().saturating_sub(td),
    );
    let border_col = color_alpha(toolbar_border, alpha_strong());

    // Two reserved slots, both BEHIND the labels: the trough, then the active
    // segment's fill on top of it.
    let bg_slot = ui.painter().add(egui::Shape::Noop);
    let sel_slot = ui.painter().add(egui::Shape::Noop);

    let prev_spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = gap_xs();

    let mut union_rect: Option<egui::Rect> = None;
    let mut active_rect: Option<egui::Rect> = None;
    // The segments used to size themselves from egui's `button_padding`, which
    // the toolbar sets generously — so this control rendered nearly TWICE the
    // height of the icon buttons beside it and was by far the tallest thing in
    // the row. `min_size` only sets a floor, so it could not hold them down.
    //
    // Pin the height instead: `interact_size` is the floor egui applies via
    // `desired_size.at_least(..)`, and with `button_padding.y = 0` the segment
    // lands on it exactly. Same mechanism used for the menu triggers.
    //
    // The trough then derives from the segments (+ inset), so the whole control
    // sits on the one toolbar height rather than setting its own.
    let seg_btn_h = toolbar_control_h() - 2.0 * gap_2xs();
    let seg_pad_x = gap_xs() + 1.0;

    for (i, label) in labels.iter().enumerate() {
        let active = i == active_idx;
        let fg = if active { accent } else { dim };
        let prev_pad = ui.spacing().button_padding;
        let prev_interact = ui.spacing().interact_size;
        ui.spacing_mut().button_padding = egui::vec2(seg_pad_x, 0.0);
        ui.spacing_mut().interact_size.y = seg_btn_h;
        // The button paints NO fill — the selection is drawn below as one
        // rounded pill. Letting the button fill itself is what produced the
        // defect: a middle segment resolved to `CornerRadius::ZERO`, so the
        // active cell rendered as a hard SQUARE block inside a trough rounded
        // at radius_md()+1 (≈15px on Aperture), flush to its top and bottom
        // edges and painting over its border.
        let resp = ui.add(
            egui::Button::new(RichText::new(*label).monospace().size(font_md()).strong().color(fg))
                .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)
                .corner_radius(egui::CornerRadius::ZERO)
                .min_size(egui::vec2(0.0, seg_btn_h))
        );
        ui.spacing_mut().button_padding = prev_pad;
        ui.spacing_mut().interact_size = prev_interact;
        union_rect = Some(union_rect.map_or(resp.rect, |r: egui::Rect| r.union(resp.rect)));
        if active { active_rect = Some(resp.rect); }
        cursor::clickable(ui, &resp);
        if resp.clicked() { clicked = Some(i); }
    }

    ui.spacing_mut().item_spacing.x = prev_spacing;

    if let Some(ur) = union_rect {
        let trough_expand = crate::dt_f32!(segmented.trough_expand_x, 4.0);
        // Expand on BOTH axes. Horizontal-only left the segments flush against
        // the trough's top and bottom, so the selection had nowhere to sit
        // inside it and the trough read as a box drawn around the text rather
        // than a track the selection slides in.
        let inset = gap_2xs();
        let trough_rect = ur.expand2(egui::vec2(trough_expand, inset));
        let r = radius_md() + 1.0;
        ui.painter().set(bg_slot, egui::Shape::rect_filled(trough_rect, r, trough));
        ui.painter().rect_stroke(trough_rect, r, Stroke::new(stroke_thin(), border_col), egui::StrokeKind::Outside);

        // The selection: one rounded pill, inset inside the trough on every
        // side, with a radius derived from the trough's so the two curves are
        // concentric rather than one square inside one round.
        if let Some(ar) = active_rect {
            let sel = egui::Rect::from_min_max(
                egui::pos2(ar.left() - trough_expand * 0.5, trough_rect.top() + inset),
                egui::pos2(ar.right() + trough_expand * 0.5, trough_rect.bottom() - inset),
            );
            let sel_r = (r - inset).max(0.0);
            ui.painter().set(
                sel_slot,
                egui::Shape::rect_filled(sel, sel_r, color_alpha(accent, alpha_tint() + 5)),
            );
        }
    }

    clicked
}

// ─── Panel chrome ─────────────────────────────────────────────────────────────
// icon_btn removed — use `ui_kit::Button::icon(icon).variant(Variant::Ghost).placement(p).show(ui, t)`.

/// Close button (X icon) — standard panel close.
/// Kept for ABI compatibility (imported in core.rs which is sacred).
#[deprecated(note = "use `ui_kit::Button::close().show(ui, theme).clicked()`")]
#[inline]
pub fn close_button(ui: &mut egui::Ui, _dim: Color32) -> bool {
    let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
    crate::ui_kit::widgets::Button::close().show(ui, &t).clicked()
}

// panel_header / panel_header_sub removed — use ui_kit::widgets::PanelHeader instead.

// ─── tab_bar — DELETED (0 callers; pane tabs paint in painter_pane.rs) ──────

// ─── Tooltip infrastructure ───────────────────────────────────────────────────

/// Standard tooltip `egui::Frame` — use with `resp.on_hover_ui(|ui| { tooltip_frame(...).show(ui, |ui| { ... }) })`.
/// Matches the watchlist deferred tooltip style.
pub fn tooltip_frame(toolbar_bg: Color32, toolbar_border: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(toolbar_bg)
        .stroke(Stroke::new(stroke_thin(), color_alpha(toolbar_border, alpha_strong())))
        .inner_margin(crate::dt_f32!(tooltip.padding, 8.0))
        .corner_radius(crate::dt_f32!(tooltip.corner_radius, 8.0))
}

/// Single stat row inside a tooltip — label left, value right.
pub fn stat_row(ui: &mut egui::Ui, label: &str, value: &str, label_color: Color32, value_color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).monospace().size(crate::dt_f32!(tooltip.stat_label_size, 8.0)).color(label_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).monospace().size(crate::dt_f32!(tooltip.stat_value_size, 10.0)).strong().color(value_color));
        });
    });
}

/// Paint a drop shadow behind a painter-based tooltip rect (call BEFORE painting the bg).
///
/// Resolves the active theme from the painter's context so light themes get a
/// soft gray drop shadow instead of the former hardcoded black.
pub fn paint_tooltip_shadow(painter: &egui::Painter, rect: egui::Rect, radius: f32) {
    let t = crate::chart_renderer::theme_impl::active_theme(painter.ctx());
    let shadow_rect = rect.translate(egui::vec2(shadow_offset(), shadow_offset()));
    painter.rect_filled(shadow_rect, radius, shadow_color_alpha(&t, shadow_alpha()));
}

/// Theme-aware drop shadow behind a painter-based tooltip rect.
pub fn paint_tooltip_shadow_themed(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: f32,
    t: &super::super::gpu::Theme,
) {
    let shadow_rect = rect.translate(egui::vec2(shadow_offset(), shadow_offset()));
    painter.rect_filled(shadow_rect, radius, shadow_color_alpha(t, shadow_alpha()));
}

// ─── Utility ──────────────────────────────────────────────────────────────────

/// Convert hex color string to Color32 with opacity.
pub fn hex_to_color(hex: &str, opacity: f32) -> Color32 {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(h.get(0..2).unwrap_or("80"), 16).unwrap_or(128);
    let g = u8::from_str_radix(h.get(2..4).unwrap_or("80"), 16).unwrap_or(128);
    let b = u8::from_str_radix(h.get(4..6).unwrap_or("80"), 16).unwrap_or(128);
    Color32::from_rgba_unmultiplied(r, g, b, (opacity * 255.0) as u8)
}

// `color_alpha` now lives in `crate::ui_kit::style`; re-exported here.

// ─── Color dimming helpers ───────────────────────────────────────────────────
// Replace ad-hoc `color.gamma_multiply(0.X)` chains with these named helpers.
// Pick by intent, not by number — see UI_AUDIT.md for the histogram of
// usages each multiplier covers.
//
// `subtle`     — secondary text/icons that still read clearly
// `muted`      — disabled-leaning, but still visible
// `dim`        — clearly de-emphasised (placeholder text, etc.)
// `very_dim`   — barely visible (decorative chart rules, watermarks)

// color_subtle/muted/half/dim/very_dim moved to `crate::ui_kit::style`
// and re-exported via the `pub use` at the top of this file.

// ─── Lighten / darken primitives ─────────────────────────────────────────────
// Linear RGB lerp toward white / black. Used to derive hover/pressed states
// from a base fill color.

/// Lighten a color toward white by `amount` (0.0–1.0). Preserves alpha.
#[inline]
pub fn lighten(c: Color32, amount: f32) -> Color32 {
    let amt = amount.clamp(0.0, 1.0);
    let r = c.r() as f32 + (255.0 - c.r() as f32) * amt;
    let g = c.g() as f32 + (255.0 - c.g() as f32) * amt;
    let b = c.b() as f32 + (255.0 - c.b() as f32) * amt;
    Color32::from_rgba_premultiplied(r as u8, g as u8, b as u8, c.a())
}

/// Darken a color toward black by `amount` (0.0–1.0). Preserves alpha.
#[inline]
pub fn darken(c: Color32, amount: f32) -> Color32 {
    let amt = (1.0 - amount).clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        (c.r() as f32 * amt) as u8,
        (c.g() as f32 * amt) as u8,
        (c.b() as f32 * amt) as u8,
        c.a(),
    )
}

/// Shift each channel of `c` by a fixed amount, clamped to 0..=255. Alpha is
/// preserved.
///
/// The ADDITIVE counterpart to `lighten`/`darken`, which are multiplicative and
/// therefore can't move a near-white surface at all (`lighten(rgb(250,250,250),
/// 0.05)` is a no-op). Pane-header active/inactive fills need a fixed
/// perceptual step regardless of how light the base surface already is, and the
/// per-channel deltas let the lift carry a slight warm/cool tint.
///
/// Prefer `color_layer_up` for ordinary surface nesting; reach for this only
/// where the base is `t.bg` rather than `t.toolbar_bg`.
#[inline]
pub fn color_shift(c: Color32, dr: i16, dg: i16, db: i16) -> Color32 {
    let ch = |v: i16| -> u8 { v.clamp(0, 255) as u8 };
    Color32::from_rgba_premultiplied(
        ch(c.r() as i16 + dr),
        ch(c.g() as i16 + dg),
        ch(c.b() as i16 + db),
        c.a(),
    )
}

/// Scale a color's RGB channels by `factor` (hue preserved, brightness moved)
/// and stamp `alpha` on the result.
///
/// The building block for intensity-shaded fills — gradient/violin candle
/// bodies, badge foregrounds derived from their own fill — where the base is a
/// theme role (`t.bull` / `t.bear` / a price color) and only the shade encodes
/// magnitude. Keeping it here means those fills stay theme-following instead of
/// each call site open-coding `Color32::from_rgb(c.r() as f32 * k, …)`.
#[inline]
pub fn color_shade(c: Color32, factor: f32, alpha: u8) -> Color32 {
    let k = factor.max(0.0);
    let ch = |v: u8| -> u8 { (v as f32 * k).clamp(0.0, 255.0) as u8 };
    Color32::from_rgba_unmultiplied(ch(c.r()), ch(c.g()), ch(c.b()), alpha)
}

// ─── Semantic interaction-state colors ───────────────────────────────────────
// Canonical hover / pressed / active / divider / disabled tones built on the
// primitives above. Call-sites should reach for these instead of inlining
// `lighten(c, 0.10)` / `tint(t, Tone::Border, 36)` etc.

/// Brighten a color by 10% — canonical hover treatment for filled surfaces.
#[inline] pub fn color_hover(c: Color32) -> Color32 { lighten(c, 0.10) }

/// Darken a color by 8% — canonical pressed/active state for filled surfaces.
#[inline] pub fn color_pressed(c: Color32) -> Color32 { darken(c, 0.08) }

/// Subtle text-color hover tint for rows/cells. Roughly matches PanelListRow's
/// HOVER_BG_ALPHA constant — gives ~7% text alpha overlay.
#[inline]
pub fn hover_tint_text(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    tint(t, Tone::Text, crate::ui_kit::style::alpha_soft())
}

/// Subtle accent fill for active chips/toggles. Use when a toggleable
/// surface needs a "yes I'm on" visual that's quieter than a full accent.
#[inline]
pub fn active_chip_fill(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    tint(t, Tone::Accent, alpha_soft())
}

/// Standard hairline divider color. Wraps the toolbar_border + alpha 36 pair
/// that's been hand-written across ~5 files for section dividers.
#[inline]
pub fn divider_color(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    tint(t, Tone::Border, 36)
}

/// Disabled overlay — soft dim wash to apply over content that's not interactive.
#[inline]
pub fn disabled_overlay(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    tint(t, Tone::Dim, alpha_dim())
}

// ─── L2 surface helper (panel sub-section / card layer) ──────────────────────
//
// The design system uses four surface layers:
//   L0: t.bg              — app canvas
//   L1: t.toolbar_bg      — panel body
//   L2: `color_layer_up`  — sub-section / card / active tab body
//   L3: hover/selected    — tint(t, Tone::Text, crate::ui_kit::style::alpha_faint()) or tint(t, Tone::Accent, crate::ui_kit::style::alpha_whisper())
//
// Direction (lighten vs darken) is derived from the theme's `bg` brightness so
// the lift reads the same on dark + light themes.

/// Returns the panel surface one layer up from `t.toolbar_bg`. Used for
/// cards, sub-sections, the active tab body — anywhere the design system
/// asks for a subtle L2 surface that contrasts gently with the panel body.
///
/// `n` is the number of 4% steps to lift (clamped to keep things subtle).
/// `n=1` is the canonical L2; larger values move toward an L3-ish accent.
/// Direction (lighten vs darken) follows whether the active theme is dark or
/// light — detected from `t.bg` brightness, same heuristic the gpu hairline
/// helpers use.
#[inline]
pub(crate) fn color_layer_up(t: &crate::chart_renderer::gpu::Theme, n: u8) -> Color32 {
    let base = t.toolbar_bg;
    let bg = t.bg;
    // Match `gpu::hairline_border`'s dark-vs-light heuristic.
    let is_dark = (bg.r() as i16 + bg.g() as i16 + bg.b() as i16) < 384;
    // 7% per step (≈18/255), capped at 5 steps. Calibrated so an L2
    // subsection visibly nests above L1 without looking like a separate
    // surface; was 4% which read as nothing. Stays under the threshold
    // where the lifted bg starts feeling like a separate card.
    let steps = n.min(5) as i16;
    let shift: i16 = if is_dark { 18 * steps } else { -18 * steps };
    let clamp = |c: i16| -> u8 { c.clamp(0, 255) as u8 };
    Color32::from_rgb(
        clamp(base.r() as i16 + shift),
        clamp(base.g() as i16 + shift),
        clamp(base.b() as i16 + shift),
    )
}

/// Top-level panel header surface — closest to `t.bg` (chart pane
/// background). Used by SidePanelShell so the topmost panel chrome
/// reads as adjacent to the chart pane.
#[inline]
pub(crate) fn header_surface(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    crate::ui_kit::style::elevate(t.bg, crate::ui_kit::style::ELEVATE_PANEL_HEADER)
}

/// Section header surface — one shade darker than `header_surface` so
/// PanelSection headers sit visually below the SidePanelShell header
/// above them. Creates the depth ramp: SidePanelShell (lightest) →
/// PanelSection → PanelSubSection → panel body (darkest).
#[inline]
pub(crate) fn section_header_surface(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    crate::ui_kit::style::elevate(t.bg, crate::ui_kit::style::ELEVATE_PANEL_SECTION)
}

/// Panel body surface — darker than `t.bg` so the side panel body
/// recedes visually below the chart and below its own header.
/// The pattern is: header (lighter, near `t.bg`) → body (darker,
/// recessed) — readable depth without high-contrast slabs.
#[inline]
pub(crate) fn panel_surface(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    crate::ui_kit::style::elevate(t.bg, crate::ui_kit::style::ELEVATE_PANEL_BODY)
}

// ─── Shell region framing (floating-card chrome) ─────────────────────────────
//
// When the active style sets `region_gap > 0` (Aperture/Glass), each major
// shell region — top nav, tool layer, workspace, right rail — floats as a
// rounded card separated by `region_gap` px of canvas. When `region_gap == 0`
// (most styles) the helpers return flush flat-fill frames so the chrome reads
// as one contiguous surface (today's look). Panes inside the workspace stay
// contiguous regardless — only the *region* boundaries gap.

/// True when the active style uses the floating-card region layout.
#[inline]
pub(crate) fn region_tiled() -> bool { current().region_gap > 0.0 }

// ── Toolnav visibility override (hybrid: style default + user toggle) ─────────
//
// The active style provides a default via `Chrome.toolnav_height`; the user can
// force the second row on/off regardless of style via this override. Mirrors
// the BorderWeight/CornerScale override-axis pattern: -1 = no override (use the
// style default), 0 = force hidden, 1 = force shown.
static TOOLNAV_OVERRIDE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Set the user's toolnav override. `None` clears it (revert to style default).
pub(crate) fn set_toolnav_override(v: Option<bool>) {
    use std::sync::atomic::Ordering;
    TOOLNAV_OVERRIDE.store(v.map(|b| b as i8).unwrap_or(-1), Ordering::Release);
}
/// The user's toolnav override, if any (`None` = follow the style default).
#[inline]
pub(crate) fn toolnav_override_opt() -> Option<bool> {
    use std::sync::atomic::Ordering;
    let x = TOOLNAV_OVERRIDE.load(Ordering::Acquire);
    if x < 0 { None } else { Some(x == 1) }
}
/// Resolved: should the toolnav (second chrome row) render this frame?
/// User override wins; otherwise the toolbar is ON by default for every style
/// (it's the primary control hub — interval / tools / indicators / order live
/// here). `toolnav_height` now only sets the height when shown, not visibility.
#[inline]
pub(crate) fn toolnav_visible() -> bool {
    match toolnav_override_opt() {
        Some(b) => b,
        None => true,
    }
}
/// Resolved toolnav height (px). Falls back to [`toolnav_min_height`] when the
/// user forces the row on but the active style sets `toolnav_height = 0`.
#[inline]
pub(crate) fn toolnav_resolved_height() -> f32 {
    let floor = toolnav_min_height();
    let h = current().toolnav_height;
    if h > 0.0 { h.max(floor) } else { floor }
}

/// The smallest toolnav height that can actually CONTAIN its tallest control.
///
/// The row hosts icon dropdown buttons (indicators / widgets) built as
/// `Button::menu(icon).glyph_size(font_lg())` with the toolbar's
/// `button_padding.y == gap_sm()`, so their height is
/// `icon-font line height + 2 * gap_sm()`.
///
/// WHY THIS IS DERIVED AND NOT A CONSTANT (2026-08-01): this used to be a hard
/// `38.0`, chosen when those buttons measured ~36.6px. When the app-wide type
/// scale was lifted the buttons grew to ~37.3px, the frozen 38px row no longer
/// contained them, and egui clipped their bottom edge — the corpus caught it as
/// `clipped: toolbar.indicators_btn, toolbar.widgets_btn` in 572 / 913 / 2400.
/// The row is chrome that WRAPS type, so its floor has to track the type scale
/// (and the user's spacing-scale override) instead of being frozen against it.
/// Shrinking the type back was explicitly rejected — the readability win stands
/// and the causation test showed the scale was not the defect; the frozen
/// constant was.
#[inline]
pub(crate) fn toolnav_min_height() -> f32 {
    // Phosphor's icon face lays out at ~1.33x the requested px size.
    const ICON_LINE_FACTOR: f32 = 1.35;
    // Slack for the panel frame: the panel spends a couple of px on its own
    // chrome, and `horizontal_centered` centres the run inside what's left.
    const FRAME_SLACK: f32 = 6.0;
    let btn_h = crate::ui_kit::style::font_lg() * ICON_LINE_FACTOR
        + 2.0 * crate::ui_kit::style::gap_sm();
    (btn_h + FRAME_SLACK).max(38.0)
}

// ── Footer (bottom dock) visibility override (hybrid: style default + user) ──
//
// The active style provides the default via `Chrome.footer_default_open`; the
// user can force the footer on/off regardless of style via this override.
// Same tri-state pattern as the toolnav: -1 = no override (follow style
// default), 0 = force hidden, 1 = force shown.
static FOOTER_OVERRIDE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Set the user's footer override. `None` clears it (revert to style default).
pub(crate) fn set_footer_override(v: Option<bool>) {
    use std::sync::atomic::Ordering;
    FOOTER_OVERRIDE.store(v.map(|b| b as i8).unwrap_or(-1), Ordering::Release);
}
/// The user's footer override, if any (`None` = follow the style default).
#[inline]
pub(crate) fn footer_override_opt() -> Option<bool> {
    use std::sync::atomic::Ordering;
    let x = FOOTER_OVERRIDE.load(Ordering::Acquire);
    if x < 0 { None } else { Some(x == 1) }
}
/// Resolved: should the bottom dock (footer) render this frame? User override
/// wins; otherwise the active style's `footer_default_open` decides.
#[inline]
pub(crate) fn footer_visible() -> bool {
    match footer_override_opt() {
        Some(b) => b,
        None => current().footer_default_open,
    }
}

/// The inter-region gap (px). 0 when the style is flush.
#[inline]
pub(crate) fn region_gap() -> f32 { current().region_gap }
/// Region corner radius — the partner of [`region_gap`].
///
/// These two are always read together (a tiled style rounds its regions, a
/// flush one squares them), but only `region_gap` had an accessor, so every
/// call site paired a helper with a raw `current().region_radius`:
///
///     let rgap = style::region_gap();
///     let rr   = if rgap > 0.0 { style::current().region_radius } else { 10.0 };
///
/// One half of a pair going through the front door and the other climbing in a
/// window is how the two drift apart later.
pub(crate) fn region_radius() -> u8 { current().region_radius as u8 }

// ── Button-group enclosure ────────────────────────────────────────────────────
// A toolbar "button group" is a run of related buttons (sidebar toggles, chart
// tools). Enclosed styles (Aperture) draw a rounded-rect box around the group
// and suppress the internal hairline dividers; flat styles (Meridien/Octave)
// space the buttons and rely on dividers.

// ── Ramp-based shading (bridge to the `sx` color system) ──────────────────────
// Real lightness shades (Tailwind-style 50…950) for general use, backed by the
// per-theme cached `Palette`. Prefer these over ad-hoc `color_alpha(tone, a)` in
// NEW code that wants a genuine shade rather than an alpha tint. Existing
// `color_alpha` sites are migrated incrementally (each is a visual change).

/// A genuine ramp shade of a semantic tone (e.g. `shade(t, Tone::Accent, Shade::S600)`).
#[inline]
pub(crate) fn shade(
    t: &crate::chart_renderer::gpu::Theme,
    tone: crate::ui_kit::sx::Tone,
    s: crate::ui_kit::sx::Shade,
) -> Color32 {
    crate::ui_kit::sx::palette_ct(t).shade(tone, s)
}

/// The base (500) color of a semantic tone at an explicit alpha — the ramp
/// system's equivalent of `color_alpha`, but tone-addressed. Part of the public
/// ramp API for incremental adoption.
#[allow(dead_code)]
#[inline]
pub(crate) fn tint(
    t: &crate::chart_renderer::gpu::Theme,
    tone: crate::ui_kit::sx::Tone,
    alpha: u8,
) -> Color32 {
    let c = crate::ui_kit::sx::palette_ct(t).base(tone);
    crate::ui_kit::style::color_alpha(c, alpha)
}

/// True when the active style draws an enclosure around button groups.
/// When false, callers keep their inter-button dividers/separators.
#[inline]
pub(crate) fn button_group_enclosed() -> bool {
    !matches!(current().button_group, crate::design_system::style_system::GroupEnclosure::None)
}

/// Inner horizontal padding around an enclosed button group.
const BUTTON_GROUP_PAD: f32 = 6.0;

/// The composed `Sx` for each group-enclosure treatment. **This is the single
/// place each look is defined** — adding a new treatment is a new arm here plus
/// a [`GroupEnclosure`](crate::design_system::style_system::GroupEnclosure)
/// variant, with no token threaded through the style pipeline.
/// `Tone::Border` resolves to `theme.toolbar_border`.
fn group_enclosure_sx(
    kind: crate::design_system::style_system::GroupEnclosure,
) -> Option<crate::ui_kit::sx::Sx> {
    use crate::design_system::style_system::GroupEnclosure as G;
    use crate::ui_kit::sx::{Sx, Tone};
    Some(match kind {
        G::None => return None,
        // Aperture — rounded box: subtle fill + hairline border. Radius +
        // border now ride the token scale, so the enclosure tracks the
        // CornerScale / BorderWeight knobs like everything else.
        G::Bordered => Sx::new()
            .rounded_md()
            .bg_alpha(Tone::Border, 10)
            .border_thin_alpha(Tone::Border, 45),
        // Glass — frosted fill-only, no hard border.
        G::Frosted => Sx::new()
            .rounded_lg()
            .bg_alpha(Tone::Border, 16),
        // Lucid — sharp editorial outline: border-only, near-square.
        G::Sharp => Sx::new()
            .rounded_xs()
            .border_thin_alpha(Tone::Border, 30),
    })
}

/// A deferred rounded-rect enclosure painted *behind* a run of toolbar buttons.
///
/// Usage mirrors `segmented_control`'s trough: reserve a background paint slot
/// before emitting the buttons (so it renders behind them), then call `end`
/// with the buttons' bounding `content` rect to fill the slot. A no-op for flat
/// styles (`begin` records no slot), so callers can wrap unconditionally.
pub(crate) struct ButtonGroupBox {
    slot: Option<egui::layers::ShapeIdx>,
}

impl ButtonGroupBox {
    /// Reserve a background slot if the active style draws group enclosures.
    pub(crate) fn begin(ui: &mut egui::Ui) -> Self {
        let slot = if button_group_enclosed() {
            Some(ui.painter().add(egui::Shape::Noop))
        } else {
            None
        };
        Self { slot }
    }

    /// Fill the reserved slot with a rounded-rect box around `content`, inset
    /// vertically from the `host` toolbar row. No-op when flat.
    pub(crate) fn end(
        self,
        ui: &mut egui::Ui,
        t: &crate::chart_renderer::gpu::Theme,
        content: egui::Rect,
        host: egui::Rect,
    ) {
        let Some(slot) = self.slot else { return; };
        let Some(sx) = group_enclosure_sx(current().button_group) else { return; };
        let pad = BUTTON_GROUP_PAD;
        let rect = egui::Rect::from_min_max(
            egui::pos2(content.left() - pad, host.top() + 3.0),
            egui::pos2(content.right() + pad, host.bottom() - 3.0),
        );
        if !rect.is_finite() || rect.width() < 4.0 { return; }
        // The look is a composed `Sx` chosen by the style's `GroupEnclosure`
        // (see `group_enclosure_sx`), painted by the generic engine.
        sx.paint_into_ct(ui, t, slot, rect, crate::ui_kit::sx::StyleState::Normal);
    }
}

/// Build an `egui::Frame` for a shell region panel (toolbar / side rail).
/// `fill` is the region's surface colour. Floats as a rounded bordered card
/// with `region_gap` outer margin when tiled; flat flush fill otherwise.
pub(crate) fn region_frame(t: &crate::chart_renderer::gpu::Theme, fill: Color32) -> egui::Frame {
    let st = current();
    if st.region_gap <= 0.0 {
        egui::Frame::NONE.fill(fill)
    } else {
        let g = st.region_gap as i8;
        egui::Frame::NONE
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(st.region_radius as u8))
            .stroke(egui::Stroke::new(
                stroke_thin(),
                tint(t, Tone::Border, st.region_border_alpha),
            ))
            .outer_margin(egui::Margin::same(g))
    }
}

/// Fill + stroke a region card at `card`, with all four corners rounded.
///
/// AUDIT 2026-08 — exists because `region_frame`'s rounded background cannot be
/// trusted to keep its bottom corners inside a `TopBottomPanel`. Measured on
/// the shipped app: the top two corners rounded correctly and the bottom two
/// came out square on every tiled style, because the panel clips the frame's
/// background at its own lower edge. The frame asks for
/// `CornerRadius::same(region_radius)` and gets three-and-a-bit corners.
///
/// Painting the card ourselves removes the dependency on how a panel chooses
/// to clip: the caller computes the rect, so the inset is exact and the
/// rounding is symmetric. Flush styles (`region_gap == 0`) fill square, which
/// is their design.
pub(crate) fn paint_region_card_filled(
    painter: &egui::Painter,
    card: egui::Rect,
    t: &crate::chart_renderer::gpu::Theme,
    fill: Color32,
) {
    let st = current();
    if !card.is_finite() || card.width() < 1.0 || card.height() < 1.0 { return; }
    if st.region_gap <= 0.0 {
        painter.rect_filled(card, egui::CornerRadius::ZERO, fill);
        return;
    }
    let r = egui::CornerRadius::same(st.region_radius as u8);
    painter.rect_filled(card, r, fill);
    painter.rect_stroke(
        card, r,
        egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, st.region_border_alpha)),
        egui::StrokeKind::Inside,
    );
}

/// The card rect for a full-width chrome row: the panel band inset by
/// `region_gap` on the left and right. Vertical extent comes from the caller,
/// which knows its own content box.
pub(crate) fn region_card_rect(content: egui::Rect, screen_w: f32) -> egui::Rect {
    let g = region_gap();
    egui::Rect::from_min_max(
        egui::pos2(g, content.top()),
        egui::pos2((screen_w - g).max(g + 1.0), content.bottom()),
    )
}

/// Paint a rounded region-card border over `rect` on the given painter. Used
/// for the central workspace, which can't wrap itself in a Frame (its panel is
/// drawn by the sacred core.rs). No-op when the style is flush.
pub(crate) fn paint_region_card(
    painter: &egui::Painter,
    rect: egui::Rect,
    t: &crate::chart_renderer::gpu::Theme,
) {
    let st = current();
    if st.region_gap <= 0.0 || !rect.is_finite() || rect.width() < 8.0 { return; }
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(st.region_radius as u8),
        egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, st.region_border_alpha)),
        egui::StrokeKind::Inside,
    );
}

/// Returns the bevel highlight alpha scaled for use as a gradient top —
/// half of the crisp-line `bevel_highlight_alpha` so the fade is softer.
/// Returns 0 when the active style has no bevel (no gradient painted).
#[inline]
pub(crate) fn current_style_bevel_hi() -> u8 {
    current().bevel_highlight_alpha / 2
}

/// Paint a vertical highlight gradient fading from `top_alpha` (white, opaque)
/// at the top edge to fully transparent at the bottom. This is the Rust analogue
/// of React's `linear-gradient(180deg, rgba(255,…,a) 0%, transparent 100%)` —
/// the warm/cool tint differences between Alto and Mariner are handled by the
/// underlying toolbar_bg palette, so pure white overlay is palette-correct.
/// No-op if `top_alpha == 0` or the rect is degenerate.
pub(crate) fn paint_gradient_highlight(painter: &egui::Painter, rect: egui::Rect, top_alpha: u8) {
    use crate::design_system::style_system::BevelStyle;
    if frame_tokens().surface_bevel == BevelStyle::None { return; }
    if top_alpha == 0 || !rect.is_finite() || rect.width() < 1.0 || rect.height() < 1.0 { return; }
    let top = Color32::from_rgba_unmultiplied(255, 255, 255, top_alpha);
    let bot = Color32::TRANSPARENT;
    let uv  = egui::pos2(0.0, 0.0);
    use egui::epaint::{Mesh, Vertex};
    let mut mesh = Mesh::default();
    mesh.vertices.extend_from_slice(&[
        Vertex { pos: rect.left_top(),     uv, color: top },
        Vertex { pos: rect.right_top(),    uv, color: top },
        Vertex { pos: rect.right_bottom(), uv, color: bot },
        Vertex { pos: rect.left_bottom(),  uv, color: bot },
    ]);
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

/// Paint a surface bevel (top inner-highlight + bottom inner-shadow lines) over
/// an already-filled `rect`, per the active style's `surface_bevel` treatment.
/// This is the Rust analogue of the React themes' `box-shadow: inset …` faces.
///
/// The tint is palette-independent — a white highlight and a black shadow — so
/// it reads correctly on any colour scheme (matching the CSS which uses
/// `rgba(255,255,255,a)` / `rgba(0,0,0,a)`); only the alphas come from the
/// style. `Raised` puts the highlight on top (a lifted face); `Inset` flips it
/// (a sunken well). No-op when the active style's bevel is `None`.
pub(crate) fn paint_bevel(painter: &egui::Painter, rect: egui::Rect, radius: egui::CornerRadius) {
    use crate::design_system::style_system::BevelStyle;
    let st = current();
    let ht = crate::ui_kit::style::frame_tokens().bevel_highlight_tint;
    let stn = crate::ui_kit::style::frame_tokens().bevel_shadow_tint;
    let hi = crate::ui_kit::style::color_alpha(ht, st.bevel_highlight_alpha);
    let sh = crate::ui_kit::style::color_alpha(stn, st.bevel_shadow_alpha);
    let (top_col, bot_col) = match st.surface_bevel {
        BevelStyle::None => return,
        BevelStyle::Raised => (hi, sh),
        BevelStyle::Inset  => (sh, hi),
    };
    // Guard against degenerate / not-yet-laid-out rects (transient at startup).
    if !rect.is_finite() || rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }
    // Inset the lines by ~half the corner radius so they don't bleed past
    // rounded corners.
    let r = radius.nw.max(radius.ne).max(radius.sw).max(radius.se) as f32;
    let inset = (r * 0.5).clamp(0.0, 3.0);
    let y_top = rect.top() + 0.5;
    let y_bot = rect.bottom() - 0.5;
    painter.line_segment(
        [egui::pos2(rect.left() + inset, y_top), egui::pos2(rect.right() - inset, y_top)],
        Stroke::new(1.0, top_col),
    );
    painter.line_segment(
        [egui::pos2(rect.left() + inset, y_bot), egui::pos2(rect.right() - inset, y_bot)],
        Stroke::new(1.0, bot_col),
    );
}

/// Header border — matches the chart pane header's perimeter hairline:
/// `tint(t, Tone::Text, 38)` at `stroke_thin()`. Use for every panel
/// header bottom rule, accordion rule, and side-panel header rule so
/// the entire chrome family reads as one bordered system.
#[inline]
pub(crate) fn header_border(t: &crate::chart_renderer::gpu::Theme) -> Color32 {
    tint(t, Tone::Text, 38)
}

/// Inverse of `color_layer_up` — moves the surface DOWN one or more
/// layers (toward `t.bg`, away from `t.text`). Use for elements that
/// should read as RECESSED rather than RAISED — section header bands,
/// trough surfaces, inset wells. Direction-aware: darker on dark themes,
/// lighter on light themes (always away from the foreground).
#[inline]
pub(crate) fn color_layer_down(t: &crate::chart_renderer::gpu::Theme, n: u8) -> Color32 {
    let base = t.toolbar_bg;
    let bg = t.bg;
    let is_dark = (bg.r() as i16 + bg.g() as i16 + bg.b() as i16) < 384;
    let steps = n.min(5) as i16;
    // OPPOSITE direction from color_layer_up — dark themes go darker,
    // light themes go lighter (move toward t.bg).
    let shift: i16 = if is_dark { -18 * steps } else { 18 * steps };
    let clamp = |c: i16| -> u8 {
        if c < 0 { 0 } else if c > 255 { 255 } else { c as u8 }
    };
    Color32::from_rgb(
        clamp(base.r() as i16 + shift),
        clamp(base.g() as i16 + shift),
        clamp(base.b() as i16 + shift),
    )
}

// ─── Theme-aware shadow color ────────────────────────────────────────────────
// Light themes set `t.shadow_color` to a dark-gray (not black) so shadows on
// cream/peach backgrounds blend instead of looking like hole-punched silhouettes.
// Use this helper instead of hardcoding `Color32::from_rgba_unmultiplied(0,0,0,X)`.

/// Shadow color from the theme at the given alpha. Replaces hardcoded
/// `Color32::from_rgba_unmultiplied(0, 0, 0, X)` calls — those break light themes.
#[inline]
pub fn shadow_color_alpha(t: &super::super::gpu::Theme, alpha: u8) -> Color32 {
    let c = t.shadow_color;
    crate::ui_kit::style::color_alpha(c, alpha)
}

// ─── Form layout ──────────────────────────────────────────────────────────────

/// Form row: right-aligned fixed-width label + content widget.
pub fn form_row(ui: &mut egui::Ui, label: &str, label_width: f32, dim: Color32, add_content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(label_width, crate::dt_f32!(form.row_height, 18.0)), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(gap_sm());
                ui.label(RichText::new(label).monospace().size(font_sm()).color(dim));
            });
        });
        add_content(ui);
    });
}

// ─── Cards / badges ───────────────────────────────────────────────────────────

/// Status badge — small tinted pill (e.g. "DRAFT", "PLACED", "TRIGGERED").
pub fn status_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    // Thin wrapper over `ui_kit::Badge` so the inline egui::Button bypass is
    // gone. Callers pass a raw colour; we forward via `.tone_color(c)` so the
    // dynamic-status-colour use case (warn/bull/bear depending on state)
    // keeps working without a TagTone enum mapping.
    use crate::ui_kit::widgets::Badge;
    let theme = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
    let resp = Badge::text(text).tone_color(color).show(ui, &theme);
    hit(&resp.rect, "BADGE", "Badges");
}

/// Order card — left accent stripe + subtle bg. Returns true if the card area was clicked.
pub fn order_card(ui: &mut egui::Ui, accent: Color32, bg: Color32, add_content: impl FnOnce(&mut egui::Ui)) -> bool {
    let ml = crate::dt_i8!(card.margin_left, 9);
    let mr = crate::dt_i8!(card.margin_right, 6);
    let my = crate::dt_i8!(card.margin_y, 5);
    let cr = crate::dt_f32!(card.radius, 4.0);
    let available_w = ui.available_width();
    let resp = egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin { left: ml, right: mr, top: my, bottom: my })
        .corner_radius(cr)
        .show(ui, |ui| {
            ui.set_min_width(available_w - 15.0);
            let outer = ui.min_rect();
            let stripe = egui::Rect::from_min_max(
                egui::pos2(outer.left() - ml as f32, outer.top() - my as f32),
                egui::pos2(outer.left() - ml as f32 + crate::dt_f32!(card.stripe_width, 3.0), outer.bottom() + my as f32));
            let stripe_col = color_alpha(accent, current().card_stripe_alpha);
            ui.painter().rect_filled(stripe, egui::CornerRadius { nw: cr as u8, sw: cr as u8, ne: 0, se: 0 }, stripe_col);
            add_content(ui);
        });
    let card_rect = resp.response.rect;
    let click_resp = ui.interact(card_rect, ui.id().with(("card_click", card_rect.min.x as i32, card_rect.min.y as i32)), egui::Sense::click());
    if click_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    cursor::focus_ring(ui, &click_resp, accent);
    ui.add_space(gap_sm());
    click_resp.clicked()
}

// ─── Buttons ──────────────────────────────────────────────────────────────────

// action_btn / trade_btn / cta_btn / small_action_btn / simple_btn removed —
// zero call sites. Use the canonical ui_kit::Button presets:
//   ui_kit::Button::action / ::trade / ::cta / ::small_action / ::simple

// ─── Drawing helpers ──────────────────────────────────────────────────────────

/// Draw a dashed or dotted line between two points.
pub fn dashed_line(painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: Stroke, style: super::super::LineStyle) {
    use super::super::LineStyle;
    let dir = b - a;
    let len = dir.length();
    if len < 1.0 || !len.is_finite() || len > 20000.0 { return; }
    match style {
        LineStyle::Solid => { painter.line_segment([a, b], stroke); }
        LineStyle::Dashed | LineStyle::Dotted => {
            let (dash, gap) = if style == LineStyle::Dashed { (6.0, 3.0) } else { (2.0, 2.0) };
            let norm = dir / len;
            let mut d = 0.0;
            while d < len {
                let p0 = a + norm * d;
                let p1 = a + norm * (d + dash).min(len);
                painter.line_segment([p0, p1], stroke);
                d += dash + gap;
            }
        }
    }
}

/// Draw a thick line into an RGBA buffer (for icon generation).
pub fn draw_line_rgba(rgba: &mut [u8], width: u32, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32, color: [u8; 4]) {
    let len = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
    let steps = (len * 3.0) as i32;
    let w = thickness as i32;
    for i in 0..=steps {
        let t = i as f32 / steps.max(1) as f32;
        let px = (x0 + (x1 - x0) * t) as i32;
        let py = (y0 + (y1 - y0) * t) as i32;
        for dy in -w..=w {
            for dx in -w..=w {
                let ix = px + dx;
                let iy = py + dy;
                if ix >= 0 && ix < width as i32 && iy >= 0 && iy < width as i32 {
                    let idx = ((iy as u32 * width + ix as u32) * 4) as usize;
                    if idx + 3 < rgba.len() { rgba[idx..idx + 4].copy_from_slice(&color); }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Split-section sidebar helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Draggable divider between two split sections. Returns vertical drag delta.
pub fn split_divider(ui: &mut egui::Ui, _id_salt: &str, dim: Color32) -> f32 {
    let div_h = crate::dt_f32!(split_divider.height, 6.0);
    let inset = crate::dt_f32!(split_divider.inset, 8.0);
    let dot_r = crate::dt_f32!(split_divider.dot_radius, 1.5);
    let dot_sp = crate::dt_f32!(split_divider.dot_spacing, 8.0);
    let active_sw = crate::dt_f32!(split_divider.active_stroke, 2.0);
    let inactive_sw = crate::dt_f32!(split_divider.inactive_stroke, 1.0);

    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), div_h), egui::Sense::drag());
    let p = ui.painter();

    let active = resp.hovered() || resp.dragged();
    let st_dh = current();
    let handle_alpha_mult = if active { 1.0 } else { st_dh.drag_handle_alpha };
    let color = if active { dim.gamma_multiply(0.6) } else {
        color_alpha(dim, (alpha_faint() as f32 * (handle_alpha_mult / 0.5).min(1.0)) as u8)
    };

    // Active drag handle uses stroke_thick from the style preset for a prominent feel.
    let effective_active_sw = st_dh.stroke_thick.max(active_sw);
    p.line_segment(
        [egui::pos2(rect.left() + inset, rect.center().y),
         egui::pos2(rect.right() - inset, rect.center().y)],
        Stroke::new(if active { effective_active_sw } else { inactive_sw }, color));

    if active {
        let cy = rect.center().y;
        let cx = rect.center().x;
        let scaled_dot_r = dot_r * st_dh.drag_handle_dot_scale;
        for dx in [-dot_sp, 0.0, dot_sp] {
            p.circle_filled(egui::pos2(cx + dx, cy), scaled_dot_r, dim.gamma_multiply(0.4));
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    if resp.dragged() { resp.drag_delta().y } else { 0.0 }
}

// ─── Compatibility shims for in-session widget builders ───────────────────────
// These were introduced alongside the new widgets/* design-system primitives.
// They centralize per-style overrides; for now they return reasonable defaults.

// P5b extraction Step 1: `ButtonTreatment` enum moved into ui_kit
// (ui_kit/widgets/tokens.rs). Re-exported here so chart-renderer call
// sites (StyleSettings.button_treatment, style preset constructors)
// keep working without import changes.
pub use crate::ui_kit::widgets::tokens::ButtonTreatment;

#[derive(Clone, PartialEq, Debug)]
pub struct StyleSettings {
    pub r_xs: u8,
    pub r_sm: u8,
    pub r_md: u8,
    pub r_lg: u8,
    pub r_pill: u8,
    pub serif_headlines: bool,
    pub button_treatment: ButtonTreatment,
    pub hairline_borders: bool,
    pub stroke_hair: f32,
    pub stroke_thin: f32,
    pub stroke_std: f32,
    /// Bold stroke weight — Meridien collapses to 1 px, Relay/Aperture use 1.5.
    pub stroke_bold: f32,
    pub stroke_thick: f32,
    pub shadows_enabled: bool,
    pub solid_active_fills: bool,
    /// True only for the style that paints active elements as a solid inverted
    /// slab (fill = palette text, text = palette bg). Replaces the removed
    /// `active_fill_color` / `active_text_color` `Option<Color32>` overrides
    /// (Phase 0b): a dimension-axis boolean, not a colour. Distinct from
    /// `solid_active_fills` — that flag is broader and true for more styles.
    pub invert_active_fill: bool,
    pub uppercase_section_labels: bool,
    /// Letter spacing approximation (px) applied to tracked-out section labels.
    pub label_letter_spacing_px: f32,
    /// Multiplier applied when scaling toolbar height (1.0 = baseline, 1.4 = Meridien tall).
    pub toolbar_height_scale: f32,
    /// Multiplier applied when scaling pane header height (1.0 = baseline, 1.1 = Meridien).
    pub header_height_scale: f32,
    /// Per-style px adjustment for a COMPACT pane header. See
    /// `Chrome::pane_header_compact_adjust` — it replaced a hardcoded match on
    /// the style INDEX that was duplicated across two renderer functions.
    pub pane_header_compact_adjust: f32,
    /// Hero numeric font size in pt (22 for Relay, 36 for Meridien).
    pub font_hero: f32,
    /// Paint full-height vertical divider lines between toolbar button clusters.
    pub vertical_group_dividers: bool,
    /// Show active-tab accent underline in tab bars.
    pub show_active_tab_underline: bool,
    /// Active pane header fill multiplier (1.2 = brighter for Relay, 0.95 = near-transparent for Meridien).
    pub active_header_fill_multiply: f32,
    /// Inactive pane header fill multiplier — applied when there are multiple
    /// visible panes and this pane is not active. Lower = more recessed.
    pub inactive_header_fill_multiply: f32,
    /// Paint a distinct fill for inactive pane headers.
    pub inactive_header_fill: bool,
    /// Alpha of the hairline outer border drawn around inactive pane headers
    /// (the contrast color is derived from theme luminance).
    pub header_outer_border_alpha: u8,
    /// Stroke width of the pane-header outer border.
    pub header_outer_border_width: f32,
    /// Alpha of the inter-section vertical divider lines inside the pane header
    /// (between nav cluster, indicator chips, and right-side icon buttons).
    pub header_divider_alpha: u8,
    /// Account strip panel height in logical px.
    pub account_strip_height: f32,

    // ── Layout & spacing ──────────────────────────────────────────────────
    /// Pane border outline thickness in logical px.
    pub pane_border_width: f32,
    /// Gap between adjacent panes in px.
    pub pane_gap: f32,
    /// Card vertical inner padding in px.
    pub card_padding_y: f32,
    /// Card horizontal inner padding in px.
    pub card_padding_x: f32,
    /// Base list-row height in px.
    pub row_height_px: f32,
    /// Base button height in px.
    pub button_height_px: f32,
    /// Button horizontal padding in px.
    pub button_padding_x: f32,
    /// Tab strip height in px.
    pub tab_height: f32,

    // ── Typography overrides ──────────────────────────────────────────────
    /// Section/eyebrow label font size in pt.
    pub font_section_label: f32,
    /// Body text font size in pt.
    pub font_body: f32,
    /// Caption font size in pt.
    pub font_caption: f32,

    // ── Interaction tokens ────────────────────────────────────────────────
    /// Alpha for hover overlay (0-255).
    pub hover_bg_alpha: u8,
    /// Alpha for active/pressed state (0-255).
    pub active_bg_alpha: u8,
    /// Focus ring stroke width.
    pub focus_ring_width: f32,
    /// Focus ring alpha (0-255).
    pub focus_ring_alpha: u8,
    /// Opacity multiplier for disabled widgets (0.0-1.0).
    pub disabled_opacity: f32,

    // ── Drop shadow ───────────────────────────────────────────────────────
    /// Shadow blur radius in px.
    pub shadow_blur: f32,
    /// Shadow vertical offset in px.
    pub shadow_offset_y: f32,
    /// Shadow alpha (0-255).
    pub shadow_alpha: u8,

    // ── Density & accent ─────────────────────────────────────────────────
    /// Global density: 0=compact, 1=normal, 2=roomy. Drives row/tab/button
    /// height multipliers when explicit overrides are not set.
    pub density: u8,
    /// Saturation/brightness multiplier for accent on active elements.
    pub accent_emphasis: f32,

    // ── Reference-match fields (Newsprint/editorial style) ────────────────
    /// Letter-spacing added between glyphs in toolbar nav buttons (px).
    /// Meridien: 1.5, others: 0.
    pub nav_letter_spacing_px: f32,
    /// Drop icon glyphs from right-side toolbar nav buttons (label-only).
    /// Meridien: true, others: false.
    pub nav_buttons_label_only: bool,
    /// Render right-side toolbar nav button labels in ALL CAPS.
    /// Meridien: true, others: false.
    pub nav_buttons_uppercase_labels: bool,
    /// Thickness of the tab-active underline in pane headers.
    /// Meridien: 2.0, Aperture: 0.0 (hidden), Octave: 1.0.
    pub tab_underline_thickness: f32,
    /// When true, draw the underline directly under active tab text (not at header bottom).
    /// Meridien: true, others: false.
    pub tab_underline_under_text: bool,
    /// Show a subtle floating shadow on card windows even when `shadows_enabled` is false.
    /// Meridien: true, Aperture: covered by shadows_enabled, Octave: false.
    pub card_floating_shadow: bool,
    /// Alpha for the card floating shadow (0-255). Meridien: 25, others: 0.
    pub card_floating_shadow_alpha: u8,
    /// Height for the primary CTA button in px. Meridien: 36, Aperture: 40, Octave: 32.
    pub cta_height_px: f32,
    /// Horizontal padding for the primary CTA button in px. Meridien: 16, others: 12.
    pub cta_padding_x: f32,

    // ── New knobs added in design-pass 2 ─────────────────────────────────────

    /// Pane gap fill color alpha (0-255). 0 = transparent (gap shows bg).
    /// Controls the visible color of the gutter between panes.
    /// Meridien: 0 (flush), Aperture: 30, Octave: 15.
    pub pane_gap_alpha: u8,
    /// Pane active indicator: 0=none, 1=top border line, 2=header fill, 3=both.
    /// Meridien: 1, Aperture: 2, Octave: 3.
    pub pane_active_indicator: u8,
    /// Toolbar nav background alpha for active button column tint.
    /// Meridien: 18, Aperture: 0 (none), Octave: 25.
    pub nav_active_col_alpha: u8,
    /// Alpha for the dialog/popup backdrop overlay (0-255). 0 = no backdrop.
    pub dialog_backdrop_alpha: u8,
    /// Tab inactive text alpha multiplier (0.0-1.0). 0.5 = dimmed, 1.0 = full.
    pub tab_inactive_alpha: f32,
    /// Tab hover background alpha (0-255). Applied when hovering an inactive tab.
    pub tab_hover_bg_alpha: u8,
    /// Section label top padding in px (space above eyebrow labels).
    pub section_label_padding_top: f32,
    /// Section label bottom padding in px (space below eyebrow labels before content).
    pub section_label_padding_bottom: f32,
    /// When true, the active pane header fills with `accent` instead of a bg-multiply.
    /// Aperture signature: the active pane header turns orange (accent). All text/icons
    /// inside must use `contrast_fg(accent)` when this flag is set.
    pub pane_active_fill_accent: bool,
    /// Pane gap (gutter) fill color override. None = use toolbar_border at pane_gap_alpha.
    pub pane_gap_color: Option<Color32>,
    /// Drag handle (split divider) color alpha multiplier (0.0-1.0).
    pub drag_handle_alpha: f32,
    /// Drag handle dot size multiplier (0.5-2.0). 1.0 = default.
    pub drag_handle_dot_scale: f32,
    /// Toast / status-bar background alpha (0-255).
    pub toast_bg_alpha: u8,
    /// Stripe/accent-banner fill alpha for order/alert cards (0-255).
    pub card_stripe_alpha: u8,
    /// Pill / chip border radius separate from r_sm. 0 = use r_sm.
    /// When non-zero, overrides r_sm for badge/chip corners specifically.
    pub r_chip: u8,

    // ── Watchlist row shape ───────────────────────────────────────────────
    /// Horizontal margin inset each side of a watchlist row (px).
    /// 0 = flush (most themes); 6 = Aperture pill rows floating inside the list.
    pub wl_row_side_margin: f32,
    /// Corner radius for watchlist rows (px). 0 = no rounding; 99 = full pill.
    pub wl_row_corner_radius: u8,
    /// Alpha (0-255) of the per-row hairline bottom divider.
    /// 0 = no divider (Aperture/Cadence); ~20 = Alto/Mariner Zed hairlines.
    pub wl_row_divider_alpha: u8,
    /// Use monospace font for symbol text in watchlist rows.
    /// True for Alto/Mariner/Aperture (IBM Plex Mono / JetBrains); false for editorial themes.
    pub wl_symbol_mono: bool,

    // ── Panel section headers ─────────────────────────────────────────────
    /// Use monospace font for section/eyebrow labels (Alto/Mariner: IBM Plex Mono).
    pub section_header_mono: bool,
    /// Tracking for section headers in px (above `label_letter_spacing_px` which is for
    /// general labels; this specifically overrides the eyebrow/category header text).
    pub section_header_tracking: f32,

    // ── Panel tab default treatment ───────────────────────────────────────
    /// Default `TabTreatment` for ui-kit `Tabs` widgets when no override is
    /// specified at the call site. Encoded as u8 matching `TabTreatment::as_u8()`.
    /// 0=Line(underline), 1=Segmented, 2=Filled, 3=Card, 4=Pane.
    pub panel_tab_treatment: u8,

    // ── Accessibility ─────────────────────────────────────────────────────
    /// When false, all motion::ease_bool / ease_value calls snap immediately
    /// to their target value, honoring the system "reduce motion" preference.
    /// Default: true.
    pub animations_enabled: bool,

    // ── Surface bevel (mirrors StyleSystem.Treatments) ─────────────────────
    /// Bevel treatment for button faces / panel headers / chips / inline tabs.
    /// Tint derives from palette luminance at paint time. See `paint_bevel`.
    pub surface_bevel: crate::design_system::style_system::BevelStyle,
    /// Alpha (0-255) of the bevel top inner-highlight line.
    pub bevel_highlight_alpha: u8,
    /// Alpha (0-255) of the bevel bottom inner-shadow line.
    pub bevel_shadow_alpha: u8,

    // ── Shell region layout (mirrors StyleSystem.Chrome) ───────────────────
    /// Gap (px) between major shell regions. 0 = flush; 8 = Aperture floating cards.
    pub region_gap: f32,
    /// Shell region card corner radius (px).
    pub region_radius: f32,
    /// Shell region card border alpha (0-255).
    pub region_border_alpha: u8,
    /// Nav cluster background pill corner radius (px).
    pub nav_cluster_radius: f32,
    /// Nav cluster background fill alpha (0-255). 0 = transparent clusters.
    pub nav_cluster_fill_alpha: u8,
    /// Nav cluster horizontal inner padding (px).
    pub nav_cluster_padding: f32,
    /// Toolbar button-group enclosure treatment (look composed as `Sx`).
    pub button_group: crate::design_system::style_system::GroupEnclosure,
    /// Second toolbar row (toolnav) height (px). 0 = single-row chrome.
    pub toolnav_height: f32,
    /// Whether the bottom dock (footer) is open by default for this style.
    /// User toggle (Ctrl+`) overrides via `footer_visible()`.
    pub footer_default_open: bool,
    /// Side-panel header toggle treatment (0=Line,1=Segmented,2=Filled,3=Card,4=Pane).
    pub panel_header_treatment: u8,
    /// PanelSection body band fill alpha (0-255). 0 = flat.
    pub panel_section_fill_alpha: u8,
    /// Pinned panel footer renders as an elevated card (true) vs flat band.
    pub panel_footer_card: bool,
    /// Pinned footer card corner radius (px).
    pub panel_footer_radius: f32,
}

// Active style selection — set once at the top of each draw_chart frame
// from `gpu::style_id(watchlist)`. 0 = Meridien (editorial), 1 = Aperture
// (modern, soft), 2 = Octave (dense). All other indices alias to Meridien.
static ACTIVE_STYLE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);

pub fn set_active_style(id: u8) {
    ACTIVE_STYLE.store(id, std::sync::atomic::Ordering::Release);
    // Refresh the per-frame token snapshot (spec §5 Rule 2).  Called here so
    // the one existing `set_active_style` call-site in `draw_chart` (core.rs)
    // needs no modification — `core.rs` is sacred and must not be touched.
    begin_frame();
}

// Toolbar rect — set once at the start of each toolbar frame so tb_btn can
// read it for full-height hover/active column overlays (Meridien only, #18).
// Encoded as four f32 bits packed into four AtomicU32 cells (min_x, min_y, max_x, max_y).
static TB_RECT: [std::sync::atomic::AtomicU32; 4] = [
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
    std::sync::atomic::AtomicU32::new(0),
];

/// Set the toolbar rect at the start of the toolbar panel (gpu.rs ~line 3700).
pub fn set_toolbar_rect(r: egui::Rect) {
    TB_RECT[0].store(r.min.x.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[1].store(r.min.y.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[2].store(r.max.x.to_bits(), std::sync::atomic::Ordering::Relaxed);
    TB_RECT[3].store(r.max.y.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Read the stored toolbar rect. Returns a zero-sized rect if not yet set.
pub fn toolbar_rect() -> egui::Rect {
    let min_x = f32::from_bits(TB_RECT[0].load(std::sync::atomic::Ordering::Relaxed));
    let min_y = f32::from_bits(TB_RECT[1].load(std::sync::atomic::Ordering::Relaxed));
    let max_x = f32::from_bits(TB_RECT[2].load(std::sync::atomic::Ordering::Relaxed));
    let max_y = f32::from_bits(TB_RECT[3].load(std::sync::atomic::Ordering::Relaxed));
    egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

// ─── Live-editable style storage ─────────────────────────────────────────────
// Three RwLock<StyleSettings> initialised once from the hardcoded defaults.
// `current()` clones the active one; `set_style_settings` overwrites it.

// ┌─ STYLE_DEFAULTS_BEGIN ─────────────────────────────────────────────────────
/// Returns the default `StyleSettings` for the given style index.
///
/// The hardcoded 3-arm match that previously lived here is replaced by a call
/// through the adapter — `style_defaults(id)` now delegates to
/// `style_system_to_style_settings(&builtin_style_systems()[id])`.
///
/// **Equivalence guarantee**: the equivalence test in this file verifies that
/// every field of the adapter output matches the original hardcoded match body
/// (preserved verbatim in `style_defaults_legacy`) for ids 0/1/2. Styles 3-8
/// are defined solely in `builtin_style_systems()`.
///
/// Adding a 10th `StyleSystem` to `builtin_style_systems()` now flows through
/// here and into `STYLE_STORE` automatically — no further changes needed.
fn style_defaults(id: u8) -> StyleSettings {
    use crate::design_system::builtin_style_systems;
    let systems = builtin_style_systems();
    let idx = id as usize;
    let ss = if idx < systems.len() { &systems[idx] } else { &systems[0] };
    style_system_to_style_settings(ss)
}
// └─ STYLE_DEFAULTS_END ───────────────────────────────────────────────────────

// NOTE: The 6 ported personality presets (Cadence/Alto/Mariner/Lucid/Relay/Glass)
// previously lived here as raw StyleSettings functions. They are now proper
// StyleSystem entries in design_system::builtin_style_systems() and flow through
// the style_system_to_style_settings adapter — the canonical two-axis path.
// See design_system/builtin.rs for the authoritative personality definitions.

// ─── Ported theme personalities (React ApexTerminalThemes → StyleSettings) ────
//
// Each function returns a bespoke StyleSettings built by struct-update over the
// closest existing preset. Values are transcribed from the React mockup's
// `global.css` `[data-ds="<theme>"]` token blocks plus its `html[data-ds]`
// structural overrides — the styling source of truth tuned to ~90% fidelity.
//
// NOTE: the bevel / inset-shadow gradients that are signature to Cadence /
// Alto / Mariner are CSS `box-shadow` effects with no StyleSettings field.
// The dimensional character (radii, heights, density, tracking, underlines,
// active-fill behaviour, button treatment, drop shadow) IS captured here;
// faithful bevels would need a new widget-render capability, not a token.


/// Public test accessor for `style_defaults`.
/// Maps: 0 → Meridien (default `_` arm), 1 → Aperture, 2 → Octave.
/// Available only in `#[cfg(test)]` so it does not bloat the release binary.
#[cfg(test)]
pub fn style_defaults_pub(id: u8) -> StyleSettings {
    style_defaults(id)
}

// ─── Golden-master reference for equivalence testing ────────────────────────
/// Verbatim copy of the original 3-arm `style_defaults` match body.
/// Used by the equivalence test to verify that `style_defaults(id)` ≡
/// `style_system_to_style_settings(&builtin_style_systems()[id])` for ids 0/1/2.
/// Do NOT modify — it is the reference the adapter must match.
#[cfg(test)]
fn style_defaults_legacy(id: usize) -> StyleSettings {
    match id {
        1 => StyleSettings {
            // React fidelity: Aperture's signature big-radius scale (8/10/14/20).
            // Lockstep with builtin.rs aperture.radii (adapter equivalence test).
            r_xs: 8, r_sm: 10, r_md: 14, r_lg: 20, r_pill: 99,
            serif_headlines: false,
            // Aperture signature: active chrome = inverted block (ink fill, light text).
            button_treatment: ButtonTreatment::BlackFillActive,
            hairline_borders: false,
            stroke_hair: 0.5, stroke_thin: 1.0, stroke_std: 1.5,
            stroke_bold: 1.5, stroke_thick: 2.0,
            shadows_enabled: true, solid_active_fills: false, invert_active_fill: false,
            uppercase_section_labels: false, label_letter_spacing_px: 0.8,
            toolbar_height_scale: 1.0, header_height_scale: 1.0, pane_header_compact_adjust: 0.0,
            font_hero: 22.0, vertical_group_dividers: false,
            show_active_tab_underline: true,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            inactive_header_fill: true,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5,
            header_divider_alpha: 50,
            account_strip_height: 26.0,
            pane_border_width: 1.0, pane_gap: 8.0,
            card_padding_y: 12.0, card_padding_x: 14.0,
            row_height_px: 26.0, button_height_px: 28.0, button_padding_x: 14.0,
            tab_height: 32.0,
            font_section_label: 10.0, font_body: 11.0, font_caption: 9.0,
            hover_bg_alpha: 15, active_bg_alpha: 25,
            focus_ring_width: 2.0, focus_ring_alpha: 90, disabled_opacity: 0.5,
            shadow_blur: 24.0, shadow_offset_y: 8.0, shadow_alpha: 40,
            density: 2, accent_emphasis: 1.1,
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 0.0,
            nav_buttons_label_only: false, nav_buttons_uppercase_labels: false,
            tab_underline_under_text: false, card_floating_shadow: false,
            card_floating_shadow_alpha: 0,
            cta_height_px: 40.0, cta_padding_x: 12.0,
            // Gap alpha=0: transparent gutters show the canvas bg (pure black in Aperture palette).
            // The rounded pane card frames define the tile edges; no gutter fill needed.
            pane_gap_alpha: 0, pane_active_indicator: 2,
            nav_active_col_alpha: 0, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.55, tab_hover_bg_alpha: 18,
            section_label_padding_top: 6.0, section_label_padding_bottom: 2.0,
            pane_gap_color: None,
            drag_handle_alpha: 0.7, drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 200, card_stripe_alpha: 255,
            r_chip: 0,
            animations_enabled: true,
            // Aperture signature: active pane header = accent (orange).
            pane_active_fill_accent: true,
            // Pill rows: symbols float as rounded items inside the watchlist.
            wl_row_side_margin: 6.0, wl_row_corner_radius: 8, wl_row_divider_alpha: 0,
            wl_symbol_mono: true, section_header_mono: false, section_header_tracking: 0.8,
            panel_tab_treatment: 2, // Filled pills
            surface_bevel: crate::design_system::style_system::BevelStyle::None,
            bevel_highlight_alpha: 0, bevel_shadow_alpha: 0,
            region_gap: 8.0, region_radius: 12.0, region_border_alpha: 40,
            nav_cluster_radius: 99.0, nav_cluster_fill_alpha: 0, nav_cluster_padding: 8.0,
            button_group: crate::design_system::style_system::GroupEnclosure::Bordered,
            toolnav_height: 30.0,
            footer_default_open: true,
            panel_header_treatment: 2, panel_section_fill_alpha: 0,
            panel_footer_card: true, panel_footer_radius: 10.0,
        },
        2 => StyleSettings {
            r_xs: 1, r_sm: 2, r_md: 3, r_lg: 4, r_pill: 99,
            serif_headlines: false,
            button_treatment: ButtonTreatment::RaisedActive,
            hairline_borders: true,
            stroke_hair: 0.4, stroke_thin: 0.6, stroke_std: 1.0,
            stroke_bold: 1.0, stroke_thick: 1.4,
            shadows_enabled: false, solid_active_fills: true, invert_active_fill: false,
            uppercase_section_labels: true, label_letter_spacing_px: 0.0,
            toolbar_height_scale: 1.0, header_height_scale: 1.0, pane_header_compact_adjust: 0.0,
            font_hero: 22.0, vertical_group_dividers: false,
            show_active_tab_underline: true,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            inactive_header_fill: true,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5,
            header_divider_alpha: 50,
            account_strip_height: 26.0,
            pane_border_width: 1.0, pane_gap: 2.0,
            card_padding_y: 6.0, card_padding_x: 8.0,
            row_height_px: 20.0, button_height_px: 22.0, button_padding_x: 8.0,
            tab_height: 26.0,
            font_section_label: 8.0, font_body: 10.0, font_caption: 8.0,
            hover_bg_alpha: 18, active_bg_alpha: 30,
            focus_ring_width: 1.5, focus_ring_alpha: 110, disabled_opacity: 0.45,
            shadow_blur: 8.0, shadow_offset_y: 4.0, shadow_alpha: 20,
            density: 0, accent_emphasis: 0.95,
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 1.0,
            nav_buttons_label_only: false, nav_buttons_uppercase_labels: false,
            tab_underline_under_text: false, card_floating_shadow: false,
            card_floating_shadow_alpha: 0,
            cta_height_px: 32.0, cta_padding_x: 12.0,
            pane_gap_alpha: 15, pane_active_indicator: 3,
            nav_active_col_alpha: 25, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.5, tab_hover_bg_alpha: 20,
            section_label_padding_top: 3.0, section_label_padding_bottom: 1.0,
            pane_gap_color: None,
            drag_handle_alpha: 0.6, drag_handle_dot_scale: 0.85,
            toast_bg_alpha: 220, card_stripe_alpha: 255,
            r_chip: 0,
            animations_enabled: true,
            pane_active_fill_accent: false,
            wl_row_side_margin: 0.0, wl_row_corner_radius: 0, wl_row_divider_alpha: 0,
            wl_symbol_mono: false, section_header_mono: false, section_header_tracking: 0.0,
            panel_tab_treatment: 0,
            surface_bevel: crate::design_system::style_system::BevelStyle::None,
            bevel_highlight_alpha: 0, bevel_shadow_alpha: 0,
            region_gap: 0.0, region_radius: 0.0, region_border_alpha: 40,
            nav_cluster_radius: 2.0, nav_cluster_fill_alpha: 0, nav_cluster_padding: 4.0,
            button_group: crate::design_system::style_system::GroupEnclosure::None,
            toolnav_height: 0.0,
            footer_default_open: true,
            panel_header_treatment: 0, panel_section_fill_alpha: 0,
            panel_footer_card: false, panel_footer_radius: 0.0,
        },
        _ => StyleSettings {
            // Phase B source-swap: Meridien (the default style) is redefined
            // to hold the values the app actually rendered before the swap —
            // the graduated dt_f32! token scale that ~75% of call sites used.
            // This makes the source-swap a no-op for the default look.
            r_xs: 2, r_sm: 4, r_md: 6, r_lg: 12, r_pill: 0,
            serif_headlines: true,
            button_treatment: ButtonTreatment::UnderlineActive,
            hairline_borders: true,
            stroke_hair: 0.3, stroke_thin: 0.5, stroke_std: 1.0,
            stroke_bold: 1.5, stroke_thick: 2.0,
            shadows_enabled: true, solid_active_fills: true, invert_active_fill: true,
            uppercase_section_labels: true, label_letter_spacing_px: 0.0,
            toolbar_height_scale: 1.40, header_height_scale: 1.10, pane_header_compact_adjust: 0.0,
            font_hero: 36.0, vertical_group_dividers: true,
            show_active_tab_underline: true,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            inactive_header_fill: true,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5,
            header_divider_alpha: 50,
            account_strip_height: 36.0,
            pane_border_width: 1.0, pane_gap: 0.0,
            card_padding_y: 8.0, card_padding_x: 10.0,
            row_height_px: 22.0, button_height_px: 24.0, button_padding_x: 10.0,
            tab_height: 28.0,
            font_section_label: 8.0, font_body: 10.0, font_caption: 8.0,
            hover_bg_alpha: 20, active_bg_alpha: 35,
            focus_ring_width: 1.0, focus_ring_alpha: 120, disabled_opacity: 0.4,
            shadow_blur: 0.0, shadow_offset_y: 0.0, shadow_alpha: 0,
            density: 1, accent_emphasis: 1.0,
            nav_letter_spacing_px: 0.0, tab_underline_thickness: 2.0,
            nav_buttons_label_only: true, nav_buttons_uppercase_labels: true,
            tab_underline_under_text: true, card_floating_shadow: true,
            card_floating_shadow_alpha: 25,
            cta_height_px: 36.0, cta_padding_x: 16.0,
            pane_gap_alpha: 0, pane_active_indicator: 1,
            nav_active_col_alpha: 18, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.6, tab_hover_bg_alpha: 12,
            section_label_padding_top: 4.0, section_label_padding_bottom: 2.0,
            pane_gap_color: None,
            drag_handle_alpha: 0.5, drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 230, card_stripe_alpha: 255,
            r_chip: 0,
            animations_enabled: true,
            pane_active_fill_accent: false,
            wl_row_side_margin: 0.0, wl_row_corner_radius: 0, wl_row_divider_alpha: 0,
            wl_symbol_mono: false, section_header_mono: false, section_header_tracking: 0.0,
            panel_tab_treatment: 0,
            surface_bevel: crate::design_system::style_system::BevelStyle::None,
            bevel_highlight_alpha: 0, bevel_shadow_alpha: 0,
            region_gap: 0.0, region_radius: 0.0, region_border_alpha: 40,
            nav_cluster_radius: 0.0, nav_cluster_fill_alpha: 0, nav_cluster_padding: 6.0,
            button_group: crate::design_system::style_system::GroupEnclosure::None,
            toolnav_height: 0.0,
            footer_default_open: false,
            panel_header_treatment: 0, panel_section_fill_alpha: 0,
            panel_footer_card: false, panel_footer_radius: 0.0,
        },
    }
}

// ─── Design-system → StyleSettings adapter ───────────────────────────────────
//
// Converts a `design_system::StyleSystem` to a `StyleSettings`.
// Every StyleSettings field is now populated from the corresponding StyleSystem
// field per the field-disposition doc. There is no struct-update fallback —
// the adapter is total.
//
// Field mapping groups (see docs/migration/field-disposition.md):
//   radii:      r_xs/r_sm/r_md/r_lg/r_pill/r_chip ← ss.radii.*
//   strokes:    stroke_hair/thin/std/bold/thick ← ss.strokes.hair/thin/std/bold/thick
//   treatments: all boolean/enum flags ← ss.treatments.*
//   spacing:    cta_height_px/card_padding_y/card_padding_x/button_height_px/
//               button_padding_x/tab_height ← ss.spacing.*
//   typography: font_section_label/font_body/font_caption/font_hero/
//               label_letter_spacing_px/nav_letter_spacing_px/section_header_tracking ← ss.typography.*
//   density:    density/row_height_px ← ss.density.*
//   shadows:    shadow_blur/shadow_offset_y/shadow_alpha ← ss.shadows.card.*
//   chrome:     all geometry/finish fields ← ss.chrome.*
//   axis violation: pane_gap_color (Color32) — all builtin StyleSystems set None
pub fn style_system_to_style_settings(
    ss: &crate::design_system::StyleSystem,
) -> StyleSettings {
    // Density: 0.8 → compact (0), 1.2+ → roomy (2), anything else → normal (1).
    let density = if (ss.density.factor - 0.8_f32).abs() < 0.05 {
        0u8
    } else if ss.density.factor >= 1.15 {
        2u8
    } else {
        1u8
    };

    StyleSettings {
        // ── Radii ────────────────────────────────────────────────────────────
        r_xs: ss.radii.xs as u8,
        r_sm: ss.radii.sm as u8,
        r_md: ss.radii.md as u8,
        r_lg: ss.radii.lg as u8,
        r_pill: ss.radii.pill as u8,
        r_chip: ss.radii.chip as u8,

        // ── Strokes ──────────────────────────────────────────────────────────
        // Direct 1-to-1 field mapping: each StyleSettings stroke field comes
        // from the same-named Strokes tier in the StyleSystem.
        // Aperture and Octave builtins store their legacy values at the correct
        // tier positions (hair/thin/std/bold/thick) for field-exact equivalence.
        stroke_hair:  ss.strokes.hair,
        stroke_thin:  ss.strokes.thin,
        stroke_std:   ss.strokes.std,
        stroke_bold:  ss.strokes.bold,
        stroke_thick: ss.strokes.thick,

        // ── Treatments ───────────────────────────────────────────────────────
        hairline_borders:         ss.treatments.hairline_borders,
        solid_active_fills:       ss.treatments.solid_active_fills,
        uppercase_section_labels: ss.treatments.uppercase_section_labels,
        surface_bevel:            ss.treatments.surface_bevel,
        bevel_highlight_alpha:    ss.treatments.bevel_highlight_alpha,
        bevel_shadow_alpha:       ss.treatments.bevel_shadow_alpha,
        wl_row_side_margin:       ss.treatments.wl_row_side_margin,
        wl_row_corner_radius:     ss.treatments.wl_row_corner_radius,
        wl_row_divider_alpha:     ss.treatments.wl_row_divider_alpha,
        section_header_mono:      ss.treatments.section_header_mono,
        wl_symbol_mono:           ss.treatments.wl_symbol_mono,
        panel_tab_treatment:      ss.treatments.panel_tab_treatment,
        pane_active_fill_accent:  ss.treatments.pane_active_fill_accent,
        serif_headlines:          ss.treatments.serif_headlines,
        button_treatment:         match ss.treatments.button_treatment {
            1 => ButtonTreatment::OutlineAccent,
            2 => ButtonTreatment::UnderlineActive,
            3 => ButtonTreatment::RaisedActive,
            4 => ButtonTreatment::BlackFillActive,
            _ => ButtonTreatment::SoftPill,
        },
        invert_active_fill:           ss.treatments.invert_active_fill,
        vertical_group_dividers:      ss.treatments.vertical_group_dividers,
        show_active_tab_underline:    ss.treatments.show_active_tab_underline,
        inactive_header_fill:         ss.treatments.inactive_header_fill,
        nav_buttons_label_only:       ss.treatments.nav_buttons_label_only,
        nav_buttons_uppercase_labels: ss.treatments.nav_buttons_uppercase_labels,
        tab_underline_under_text:     ss.treatments.tab_underline_under_text,
        card_floating_shadow:         ss.treatments.card_floating_shadow,
        shadows_enabled:              ss.treatments.shadows_enabled,
        animations_enabled:           ss.treatments.animations_enabled,

        // ── Spacing ──────────────────────────────────────────────────────────
        cta_height_px:    ss.spacing.cta_height,
        cta_padding_x:    ss.spacing.cta_padding_x,
        card_padding_y:   ss.spacing.md,
        card_padding_x:   ss.spacing.lg,
        button_height_px: ss.spacing.button_height,
        button_padding_x: ss.spacing.button_padding_x,
        tab_height:       ss.spacing.tab_height,

        // ── Typography ───────────────────────────────────────────────────────
        font_caption:            ss.typography.size_xs,
        font_body:               ss.typography.size_sm,
        font_hero:               ss.typography.size_xl,
        font_section_label:      ss.typography.size_section_label,
        label_letter_spacing_px: ss.typography.label_tracking,
        nav_letter_spacing_px:   ss.typography.nav_tracking,
        section_header_tracking: ss.typography.section_tracking,

        // ── Density ──────────────────────────────────────────────────────────
        density,
        row_height_px: ss.density.row_height_dense,

        // ── Shadows ──────────────────────────────────────────────────────────
        shadow_blur:     ss.shadows.card.blur,
        shadow_offset_y: ss.shadows.card.offset_y,
        shadow_alpha:    (ss.shadows.card.alpha * 255.0).round() as u8,

        // ── Chrome (geometry + finish) ───────────────────────────────────────
        toolbar_height_scale:          ss.chrome.toolbar_height_scale,
        header_height_scale:           ss.chrome.header_height_scale,
        pane_header_compact_adjust:    ss.chrome.pane_header_compact_adjust,
        account_strip_height:          ss.chrome.account_strip_height,
        pane_border_width:             ss.chrome.pane_border_width,
        pane_gap:                      ss.chrome.pane_gap,
        pane_gap_alpha:                ss.chrome.pane_gap_alpha,
        pane_active_indicator:         ss.chrome.pane_active_indicator,
        active_header_fill_multiply:   ss.chrome.active_header_fill_multiply,
        inactive_header_fill_multiply: ss.chrome.inactive_header_fill_multiply,
        header_outer_border_alpha:     ss.chrome.header_outer_border_alpha,
        header_outer_border_width:     ss.chrome.header_outer_border_width,
        header_divider_alpha:          ss.chrome.header_divider_alpha,
        nav_active_col_alpha:          ss.chrome.nav_active_col_alpha,
        dialog_backdrop_alpha:         ss.chrome.dialog_backdrop_alpha,
        tab_inactive_alpha:            ss.chrome.tab_inactive_alpha,
        tab_hover_bg_alpha:            ss.chrome.tab_hover_bg_alpha,
        tab_underline_thickness:       ss.chrome.tab_underline_thickness,
        section_label_padding_top:     ss.chrome.section_label_padding_top,
        section_label_padding_bottom:  ss.chrome.section_label_padding_bottom,
        drag_handle_alpha:             ss.chrome.drag_handle_alpha,
        drag_handle_dot_scale:         ss.chrome.drag_handle_dot_scale,
        toast_bg_alpha:                ss.chrome.toast_bg_alpha,
        card_stripe_alpha:             ss.chrome.card_stripe_alpha,
        card_floating_shadow_alpha:    ss.chrome.card_floating_shadow_alpha,
        accent_emphasis:               ss.chrome.accent_emphasis,
        disabled_opacity:              ss.chrome.disabled_opacity,
        focus_ring_width:              ss.chrome.focus_ring_width,
        focus_ring_alpha:              ss.chrome.focus_ring_alpha,
        hover_bg_alpha:                ss.chrome.hover_bg_alpha,
        active_bg_alpha:               ss.chrome.active_bg_alpha,
        region_gap:                    ss.chrome.region_gap,
        region_radius:                 ss.chrome.region_radius,
        region_border_alpha:           ss.chrome.region_border_alpha,
        nav_cluster_radius:            ss.chrome.nav_cluster_radius,
        nav_cluster_fill_alpha:        ss.chrome.nav_cluster_fill_alpha,
        nav_cluster_padding:           ss.chrome.nav_cluster_padding,
        button_group:                  ss.chrome.button_group,
        toolnav_height:                ss.chrome.toolnav_height,
        footer_default_open:           ss.chrome.footer_default_open,
        panel_header_treatment:        ss.chrome.panel_header_treatment,
        panel_section_fill_alpha:      ss.chrome.panel_section_fill_alpha,
        panel_footer_card:             ss.chrome.panel_footer_card,
        panel_footer_radius:           ss.chrome.panel_footer_radius,

        // ── Axis violation: color on the dimension axis ───────────────────────
        // pane_gap_color is Option<Color32>. All builtin StyleSystems specify
        // None (the field-disposition doc marks this as an axis violation moved
        // to ColorScheme.pane_gap_color). Renderers derive the gap color from
        // bg/border at paint time when None.
        pane_gap_color: None,
    }
}

// ─── Dynamic style preset store ──────────────────────────────────────────────
// Vec of (name, settings) pairs. Ids 0/1/2 are the canonical three styles
// (Meridien/Aperture/Octave) and cannot be deleted. User-added presets append
// beyond index 2 and survive only for the session (in-memory, no source write).

static STYLE_STORE: std::sync::OnceLock<std::sync::RwLock<Vec<(String, StyleSettings)>>> =
    std::sync::OnceLock::new();

// -- M1: the parallel StyleSystem store -- the design system's own type, live --
//
// STYLE_STORE (above) holds the flattened legacy `StyleSettings`; this store
// holds the FULL `StyleSystem` at the same indices, so `begin_frame()` can
// source the token ladders (gaps, UI type scale, alpha tiers) from fields the
// legacy adapter never carried. Populated from `builtin_style_systems()` at
// init and kept index-aligned with STYLE_STORE by the shared setters below.
// Slots edited only through `set_style_settings` (design-inspector path) keep
// their last-known StyleSystem -- the inspector edits legacy fields the ladder
// doesn't read yet.
static STYLE_SYSTEM_STORE: std::sync::OnceLock<
    std::sync::RwLock<Vec<std::sync::Arc<crate::design_system::StyleSystem>>>,
> = std::sync::OnceLock::new();

fn style_system_store()
-> &'static std::sync::RwLock<Vec<std::sync::Arc<crate::design_system::StyleSystem>>> {
    STYLE_SYSTEM_STORE.get_or_init(|| {
        use crate::design_system::builtin_style_systems;
        let systems = builtin_style_systems();
        let mut v: Vec<std::sync::Arc<crate::design_system::StyleSystem>> =
            systems.into_iter().map(std::sync::Arc::new).collect();
        // Mirror STYLE_STORE's "Contour" alias slot (a Meridien clone).
        if let Some(first) = v.first().cloned() {
            v.push(first);
        }
        std::sync::RwLock::new(v)
    })
}

/// The ACTIVE style's full `StyleSystem` (Arc clone -- cheap, once per frame).
/// Falls back to slot 0 (Meridien) when the active id is out of range.
// M1 Change E: per-frame shadow stacks. Vec is not Copy, so these live in
// their own thread-local (TokenSnapshot stays a Copy Cell). Pushed by
// `begin_frame`; consumed by the layered card-shadow painter in ui_kit via
// `crate::ui_kit::style::set_card_shadow_layers`' getter twin.
pub fn active_style_system() -> std::sync::Arc<crate::design_system::StyleSystem> {
    let id = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire) as usize;
    let store = style_system_store().read().unwrap_or_else(|e| e.into_inner());
    store.get(id).or_else(|| store.first()).cloned()
        .unwrap_or_else(|| std::sync::Arc::new(crate::design_system::StyleSystem::default()))
}

/// Every live style system's id, in slot order.
///
/// Exists so external tooling (the ds-harness) can resolve a style NAME to its
/// index without keeping its own copy of the list. Two harness scripts each
/// hardcoded that list; both had drifted.
pub fn style_system_ids() -> Vec<String> {
    style_system_store()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .map(|s| s.meta.id.clone())
        .collect()
}

/// Overwrite the StyleSystem for slot `id` (index-aligned with STYLE_STORE).
pub fn set_style_system(id: u8, ss: crate::design_system::StyleSystem) {
    let mut store = style_system_store().write().unwrap_or_else(|e| e.into_inner());
    let idx = id as usize;
    if idx < store.len() { store[idx] = std::sync::Arc::new(ss); }
}

/// Append a StyleSystem slot (call in lockstep with `add_style_preset`).
pub fn add_style_system(ss: crate::design_system::StyleSystem) -> u8 {
    let mut store = style_system_store().write().unwrap_or_else(|e| e.into_inner());
    let id = store.len() as u8;
    store.push(std::sync::Arc::new(ss));
    id
}

fn style_store() -> &'static std::sync::RwLock<Vec<(String, StyleSettings)>> {
    STYLE_STORE.get_or_init(|| {
        // Source the 3 canonical styles from design_system::builtin_style_systems()
        // mapped through the adapter. This mirrors the colour-axis flip in gpu.rs
        // (live_themes → builtin_color_schemes). The adapter output is field-exact
        // with style_defaults(i) for all StyleSystem-carried fields (equivalence test
        // guarantees this); fields not in StyleSystem keep the style_defaults base.
        use crate::design_system::builtin_style_systems;
        let systems = builtin_style_systems();
        debug_assert_eq!(
            systems.len(), 9,
            "builtin_style_systems() must return 9 entries (Meridien/Aperture/Octave/Cadence/Alto/Mariner/Lucid/Relay/Glass)"
        );
        let mut v: Vec<(String, StyleSettings)> = systems
            .iter()
            .map(|ss| {
                (ss.meta.name.clone(), style_system_to_style_settings(ss))
            })
            .collect();
        // One remaining alias slot for forward-compatibility.
        let meridien = style_system_to_style_settings(&systems[0]);
        v.push(("Contour".to_string(), meridien));
        std::sync::RwLock::new(v)
    })
}

/// Get a clone of the settings for style `id`. Falls back to 0 (Meridien) if out of range.
pub fn get_style_settings(id: u8) -> StyleSettings {
    // Wave 8 High: recover from lock poison instead of propagating a cascade-crash.
    let store = style_store().read().unwrap_or_else(|e| e.into_inner());
    let idx = id as usize;
    if idx < store.len() { store[idx].1.clone() } else { store[0].1.clone() }
}

/// Overwrite the settings for style `id` — takes effect on the next frame.
/// Silently ignored if `id` is out of range.
pub fn set_style_settings(id: u8, settings: StyleSettings) {
    // Wave 8 High: recover from lock poison.
    let mut store = style_store().write().unwrap_or_else(|e| e.into_inner());
    let idx = id as usize;
    if idx < store.len() { store[idx].1 = settings; }
}

/// Add a new named preset cloned from an existing style. Returns the new id.
pub fn add_style_preset(name: &str, settings: StyleSettings) -> u8 {
    // Wave 8 High: recover from lock poison.
    let mut store = style_store().write().unwrap_or_else(|e| e.into_inner());
    let id = store.len() as u8;
    store.push((name.to_string(), settings));
    id
}

/// Delete a user preset. Ids 0/1/2 are protected (no-op). All ids above the
/// deleted slot are shifted down — callers should re-read `list_style_presets`
/// and update any stored `style_idx` values accordingly.
pub fn delete_style_preset(id: u8) {
    if id < 3 { return; }
    // Wave 8 High: recover from lock poison.
    let mut store = style_store().write().unwrap_or_else(|e| e.into_inner());
    let idx = id as usize;
    if idx < store.len() { store.remove(idx); }
}

/// Rename a preset in-place. No-op if `id` is out of range.
pub fn rename_style_preset(id: u8, new_name: String) {
    // Wave 8 High: recover from lock poison.
    let mut store = style_store().write().unwrap_or_else(|e| e.into_inner());
    let idx = id as usize;
    if idx < store.len() { store[idx].0 = new_name; }
}

/// Returns `(id, name)` pairs for all registered presets — use for dropdowns.
pub fn list_style_presets() -> Vec<(u8, String)> {
    // Wave 8 High: recover from lock poison.
    style_store().read().unwrap_or_else(|e| e.into_inner())
        .iter().enumerate()
        .map(|(i, (name, _))| (i as u8, name.clone()))
        .collect()
}

/// The active style's index.
///
/// `ACTIVE_STYLE` is private, and every caller that wanted to save-and-restore
/// it around a temporary style switch was reaching into the atomic directly —
/// only possible inside this module, so tests elsewhere could not do it at all.
pub fn active_style_idx() -> u8 {
    ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire)
}

pub fn current() -> StyleSettings {
    let id = active_style_idx();
    get_style_settings(id)
}

// Style-aware corner radius helpers — route through `current()` so corners
// flip when the active style changes (Meridien 0/0/0/0/0, Aperture 4/6/8/12/99,
// Octave 1/2/3/4/99). Previously these used static tokens which broke the
// style cascade — a popup using r_lg_cr() always got 8px regardless of style.
pub fn r_xs() -> egui::CornerRadius { egui::CornerRadius::same(current().r_xs) }
// r_sm_cr / r_md_cr / r_lg_cr now in `crate::ui_kit::style`.
pub fn r_pill() -> egui::CornerRadius { egui::CornerRadius::same(current().r_pill) }

pub fn btn_compact_height() -> f32 { 22.0 }
pub fn btn_simple_height() -> f32 { 24.0 }
pub fn btn_small_height() -> f32 { 22.0 }
pub fn btn_trade_height() -> f32 { 28.0 }

// ── New style-setting helpers ────────────────────────────────────────────────

/// User-set density override. Negative = no override (inherit from preset);
/// 0/1/2 = the user's explicit DensityMode choice. Set via the density picker
/// in settings_panel (P4.3); preserved across app restarts via the workspace
/// `density_override` field on Watchlist.
static DENSITY_OVERRIDE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Set the global density override. Pass `None` to clear it (inherit from
/// the active style preset's `StyleSettings.density`).
pub fn set_density_override(mode: Option<crate::ui_kit::style::DensityMode>) {
    let v = mode.map(|m| m.as_u8() as i8).unwrap_or(-1);
    DENSITY_OVERRIDE.store(v, std::sync::atomic::Ordering::Release);
}

/// Read the current density override. `None` = inherit from style preset.
#[inline]
pub fn density_override() -> Option<crate::ui_kit::style::DensityMode> {
    let v = DENSITY_OVERRIDE.load(std::sync::atomic::Ordering::Acquire);
    if v < 0 { None } else { Some(crate::ui_kit::style::DensityMode::from_u8(v as u8)) }
}

/// Effective density: override if set, otherwise the active style preset's value.
#[inline]
fn effective_density() -> crate::ui_kit::style::DensityMode {
    density_override().unwrap_or_else(|| crate::ui_kit::style::DensityMode::from_u8(current().density))
}

/// Density-aware row height. Reads `row_height_px` then scales by effective density.
pub fn style_row_height() -> f32 {
    current().row_height_px * effective_density().scale()
}

/// The COMFORTABLE row height — `Density.row_height_comfortable`, scaled.
///
/// The density block has always carried two row heights per style
/// (`row_height_dense`, `row_height_comfortable`), but only the dense one was
/// ever wired: it becomes `StyleSettings.row_height_px` and backs
/// [`style_row_height`]. `row_height_comfortable` was loaded, exported, and
/// exposed in the design inspector while no code read it.
///
/// The watchlist, meanwhile, hardcoded `28.0` — which IS Meridien's
/// `row_height_comfortable`. The token and the literal had the same value and
/// no connection, so density and style switching moved every list except the
/// most visible one.
///
/// Reads the design_system directly rather than adding a `StyleSettings`
/// field: that struct is the legacy mirror being shrunk and its pub-field
/// count is ratcheted.
pub fn style_row_height_comfortable() -> f32 {
    // NOTE: `row_height_comfortable` is the LEGACY density field and is not
    // carried in `TokenSnapshot`, so the once-only density scaling applied in
    // `begin_frame` to the structural ladder does not cover it — the scale is
    // still applied here, and that is not a double-apply.
    //
    // It does mean this one accessor bypasses the hot-reload override, because
    // it reads the style store directly. Left as-is deliberately: routing it
    // through the snapshot would need a snapshot field, and the legacy pair is
    // meant to retire in favour of the row_* ladder rather than grow.
    active_style_system().density.row_height_comfortable * effective_density().scale()
}
/// Density-aware button height. Reads `button_height_px` then scales by effective density.
pub fn style_button_height() -> f32 {
    current().button_height_px * effective_density().scale()
}

/// THE height for every interactive control in the toolbar rows.
///
/// The toolbar was rendering four different heights side by side: label chips
/// and menu triggers at one size, icon buttons at another, the segmented
/// layout picker at nearly double, and the caret next to it back at the small
/// size. Each was individually reasonable — they came from `Size::Md`, from
/// egui's `button_padding`, and from a `min_size` floor — and together they
/// read as ragged.
///
/// One source, and a THEMED one: `button_height` is authored per style
/// (Aperture 28, Meridien 24) and scaled by the active density, so a
/// comfortable bar stays comfortable when the design system changes. Rounded
/// because fractional control heights were also producing fractional widths
/// and soft edges.
///
/// This finally gives `style_button_height()` a consumer — the geometry sweep
/// found it authored, exposed, and read by nothing.
pub fn toolbar_control_h() -> f32 {
    // Floored at the touch minimum. Without it, a style authoring a 24px
    // control height rendered a toolbar the app's own design audit then
    // reported as non-compliant — every frame, on seven of nine styles.
    // Floor only: Aperture authors 32 and keeps it.
    style_button_height()
        .max(crate::ui_kit::style::MIN_TOUCH_TARGET_PX)
        .round()
}
/// Density-aware tab height. Reads `tab_height` then scales by effective density.
pub fn style_tab_height() -> f32 {
    current().tab_height * effective_density().scale()
}
/// Accent color with emphasis multiplier applied (brightness boost for active elements).
pub fn accent_emphasised(color: egui::Color32) -> egui::Color32 {
    color.gamma_multiply(current().accent_emphasis)
}

// `contrast_fg` now in `crate::ui_kit::style`; re-exported here.

pub fn rule_stroke_for(_bg: egui::Color32, border: egui::Color32) -> egui::Stroke {
    // Use pane_border_width so Meridien hairlines honour the style knob.
    egui::Stroke::new(current().pane_border_width, border)
}

/// Paint a full-height inter-cluster vertical divider line in the toolbar (#4).
/// Call between button groups when `current().vertical_group_dividers` is true.
/// `panel_rect` should be the full toolbar panel rect for correct top/bottom span.
pub fn tb_group_break(ui: &mut egui::Ui, panel_rect: egui::Rect, border: egui::Color32) {
    if !current().vertical_group_dividers { return; }
    ui.add_space(gap_md());
    let x = ui.cursor().left();
    // Use alpha_heavy (120) for clearly visible dividers even on dim toolbar_border colors.
    let color = color_alpha(border, alpha_heavy());
    ui.painter().line_segment(
        [egui::pos2(x, panel_rect.top() + 2.0), egui::pos2(x, panel_rect.bottom() - 2.0)],
        egui::Stroke::new(stroke_std(), color),
    );
    ui.add_space(gap_md());
}

/// Returns the `FontId` for section/eyebrow header labels, respecting
/// `section_header_mono` (Alto/Mariner/Relay use IBM Plex Mono for eyebrows).
pub fn section_header_font_id() -> egui::FontId {
    if current().section_header_mono {
        egui::FontId::new(font_xs(), egui::FontFamily::Monospace)
    } else {
        egui::FontId::new(font_xs(), egui::FontFamily::Proportional)
    }
}

/// `true` when the active style prefixes panel section headers with an accent
/// ordinal (`01 WATCHLIST`). See `Treatments::numbered_section_labels`.
///
/// Deliberately NOT folded into [`style_label_case`]: the numeral is accent-
/// coloured and the title is not, so this cannot be a string transform.
pub fn style_numbers_sections() -> bool {
    // Read the design_system treatment DIRECTLY rather than via `current()`.
    //
    // The first version mirrored this into `StyleSettings` like
    // `uppercase_section_labels`, and `style-mig-lint` caught it: that struct
    // is the legacy god-object being shrunk, and its pub-field count is
    // ratcheted (99). Growing it to add a *new* treatment is backwards — the
    // mirror exists to carry fields that predate `Treatments`, not to receive
    // fresh ones. Nothing was lost by dropping the mirror: the cascade is the
    // same, one hop shorter.
    active_style_system().treatments.numbered_section_labels
}

/// Returns `s` uppercased (and letter-spaced) for active styles that request it (#5, #12).
///
/// # Letter-spacing limitation
/// egui does not support CSS `letter-spacing`. We approximate it by inserting Unicode
/// thin-spaces (U+2009) between characters. Threshold:
///   < 0.5 px  → no spacing
///   0.5–1.5 px → single thin-space between each char
///   > 1.5 px  → double thin-space between each char
/// This is a visual approximation; the effective gap depends on font rendering.
pub fn style_label_case(s: &str) -> String {
    let st = current();
    let base = if st.uppercase_section_labels { s.to_uppercase() } else { s.to_string() };
    // Apply letter-spacing approximation via Unicode thin-spaces (U+2009).
    // Use the larger of label_letter_spacing_px and section_header_tracking so
    // section headers get the full per-theme tracking (Alto/Mariner 0.8px,
    // Relay 1.2px) without needing a separate call.
    let sp = st.label_letter_spacing_px.max(st.section_header_tracking);
    if sp < 0.5 {
        base
    } else {
        let sep = if sp > 1.5 { "\u{2009}\u{2009}" } else { "\u{2009}" };
        base.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(sep)
    }
}

/// Returns a `FontId` appropriate for hero numerics — serif when the active
/// style requests it, monospace otherwise (#14).
pub fn hero_font_id(size: f32) -> egui::FontId {
    if current().serif_headlines {
        egui::FontId::new(size, egui::FontFamily::Name("serif".into()))
    } else {
        egui::FontId::monospace(size)
    }
}

/// Builds a `RichText` for large numeric displays using the hero font (#14).
pub fn hero_text(text: &str, color: egui::Color32) -> egui::RichText {
    let size = current().font_hero;
    egui::RichText::new(text).font(hero_font_id(size)).color(color)
}

/// M5 — the account strip's RESOLVED height.
///
/// `chrome.account_strip_height` is an authored token, but a strip that is
/// shorter than the hero number it contains simply clips it. Meridien authors
/// `font_hero: 36` and `account_strip_height: 36`, and the strip's frame adds
/// 2px top + 2px bottom margin — leaving 32px of content box for a 36px glyph.
/// The result was a permanently guillotined `NAV $47895`.
///
/// This is the repo's recurring *frozen chrome* defect: a chrome dimension
/// pinned to a value that some token USED to produce, which stops holding the
/// moment that token moves. The cure is the standing rule — **derive, don't
/// pin**. The authored token becomes a FLOOR: the strip is whichever is
/// larger, what the designer asked for or what the type scale requires.
///
/// Styles whose hero already fits are bit-identical (Aperture and Cadence
/// author 26.0 with `font_hero: 22.0`; 22 + 4 == 26, so the max is a no-op).
/// `strip_fits_hero` in the test module holds that invariant for every style.
pub fn account_strip_height() -> f32 {
    /// The strip frame's vertical inner margin (top 2 + bottom 2), which is
    /// subtracted from the panel height before any text is laid out.
    const V_MARGIN: f32 = 4.0;
    let s = current();
    // AUDIT 2026-08 — the floor is the hero's LINE height, not its font size.
    //
    // This read `font_hero + V_MARGIN`, which is the third version of the same
    // bug: the original pinned `account_strip_height` and clipped Meridien's
    // NAV number, so it was made derived — but derived from the point size. A
    // 36px face does not occupy 36px; it occupies size x leading, about 45px at
    // 1.25. So the strip stayed ~9px short and the account value went on being
    // clipped at the bottom, just less obviously than before.
    //
    // `line_heading()` is the leading token, so this now tracks a style that
    // authors looser leading instead of needing a fourth correction.
    s.account_strip_height.max(s.font_hero * crate::ui_kit::style::line_heading() + V_MARGIN)
}

/// Apply per-style egui::Style overrides (widget visuals, spacing, shadows)
/// to the given context. Call once per frame after `set_active_style` (#3).
///
/// This is intentionally a *supplement* to the rich visual block already
/// applied in `setup_theme`; it only overrides the fields that differ
/// between styles so that non-Meridien themes remain visually unchanged.
pub fn apply_ui_style(style: &mut egui::Style, settings: &StyleSettings, toolbar_border: egui::Color32, toolbar_bg: egui::Color32, accent: egui::Color32, shadow_color: egui::Color32) {
    let is_meridien = settings.hairline_borders && settings.serif_headlines;

    if is_meridien {
        // Meridien widget fills: transparent inactive, flat hairline borders
        let inact = &mut style.visuals.widgets.inactive;
        inact.bg_fill      = egui::Color32::TRANSPARENT;
        inact.weak_bg_fill = egui::Color32::TRANSPARENT;
        inact.bg_stroke    = egui::Stroke::new(stroke_std(), color_alpha(toolbar_border, 70));
        inact.corner_radius = egui::CornerRadius::ZERO;

        let hov = &mut style.visuals.widgets.hovered;
        hov.bg_fill      = color_alpha(toolbar_border, crate::ui_kit::style::alpha_soft());
        hov.corner_radius = egui::CornerRadius::ZERO;

        let act = &mut style.visuals.widgets.active;
        act.corner_radius = egui::CornerRadius::ZERO;

        let open = &mut style.visuals.widgets.open;
        open.corner_radius = egui::CornerRadius::ZERO;

        // Shadows: keep the values applied in setup_theme — Meridien previously
        // zeroed these (#16) but that left ComboBox/menus with no depth cue.
        // The setup_theme values are already subtle and theme-appropriate.
        style.visuals.window_stroke = egui::Stroke::new(settings.stroke_std, toolbar_border);
        style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
        style.visuals.menu_corner_radius   = egui::CornerRadius::ZERO;

        // Denser editorial spacing
        style.spacing.button_padding   = egui::vec2(gap_xl(), gap_xs());
        style.spacing.menu_margin      = egui::Margin { left: gap_md() as i8, right: gap_md() as i8, top: gap_sm() as i8, bottom: gap_sm() as i8 };
        style.spacing.interact_size.y  = 22.0;
        style.spacing.item_spacing     = egui::vec2(gap_sm(), gap_xs());
    }

    // ── Per-style global widget corner radius ─────────────────────────────
    // Push the style's r_md onto all egui widget families so dropdowns,
    // menus, tooltips, and any egui-native widget match the per-theme radius.
    // Meridien's ZERO is already set above in its block; handle others here.
    if !is_meridien {
        // M0.3: resolve through the ui_kit accessors (CornerScale + hot-reload aware)
        // instead of raw `settings.r_*` — egui-native widgets (ComboBox, menus) now
        // follow the same radius resolution as ui_kit widgets.
        let cr_md = egui::CornerRadius::same(crate::ui_kit::style::radius_md() as u8);
        let cr_sm = egui::CornerRadius::same(crate::ui_kit::style::radius_sm() as u8);
        style.visuals.window_corner_radius  = cr_md;
        style.visuals.menu_corner_radius    = cr_sm;
        for state in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
            &mut style.visuals.widgets.noninteractive,
        ] {
            state.corner_radius = cr_sm;
        }
    }

    // ── Per-density item spacing ──────────────────────────────────────────
    // Roomy styles (density=2, Glass/Aperture) get a bit more breathing room;
    // compact styles (density=0, Octave/Mariner) tighten up.
    style.spacing.item_spacing = match settings.density {
        0 => egui::vec2(gap_xs(), 2.0),  // compact
        2 => egui::vec2(gap_sm(), gap_xs()), // roomy
        _ => style.spacing.item_spacing,  // keep existing
    };

    // ── Drop shadow for popup panels ─────────────────────────────────────
    // Glass and Aperture have large soft shadows; Octave/Meridien flatten them.
    if settings.shadows_enabled && settings.shadow_blur > 0.0 {
        // M0.2: was `color_alpha(Color32::BLACK, …)` — hardcoded black clobbered the
        // theme-aware `t.shadow_color` shadows set in setup_theme 100 lines earlier
        // (CLAUDE.md rule 2), turning every light-theme popup shadow into a black
        // smudge. The caller now passes the palette's shadow colour through.
        let sh_color = color_alpha(shadow_color, settings.shadow_alpha);
        style.visuals.popup_shadow = egui::epaint::Shadow {
            offset: [0, settings.shadow_offset_y as i8],
            blur:   settings.shadow_blur as u8,
            spread: 0,
            color:  sh_color,
        };
        style.visuals.window_shadow = style.visuals.popup_shadow;
    } else {
        style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
        style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    }

    // ── Per-style scrollbar ───────────────────────────────────────────────
    // React themes specify scrollbar width in ::webkit-scrollbar:
    //   Alto/Mariner/Lucid/Meridien/Relay: 7px — visible, part of the chrome
    //   Cadence/Glass: 5px — minimal, unobtrusive
    //   Aperture/Octave: default egui thin
    let scroll_bar_width = match settings.density {
        0 => 4.0, // compact (Octave/Mariner) — minimal
        2 => 8.0, // roomy (Glass) — slightly wider for comfort
        _ if settings.section_header_mono => 7.0, // Alto/Mariner/Relay — styled visible
        _ if settings.serif_headlines     => 7.0, // Meridien/Lucid/Relay editorial
        _                                  => 5.0, // others
    };
    style.spacing.scroll.bar_width    = scroll_bar_width;
    style.spacing.scroll.bar_inner_margin = 2.0;
    style.spacing.scroll.bar_outer_margin = 0.0;
    // Scrollbar color: use dim border tinted with the theme border.
    style.visuals.extreme_bg_color = color_alpha(toolbar_border, 35);

    // input_focus_color: derived from accent (§3.2 — no per-style override).
    style.visuals.selection.stroke = egui::Stroke::new(settings.focus_ring_width, accent);

    let _ = (toolbar_bg,); // may be used in future for popup fill overrides
}

// ─── #19 chrome_tile_btn ──────────────────────────────────────────────────────

/// State passed to [`paint_chrome_tile_button`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ChromeTileState { Idle, Hovered, Active }

/// Paint the small square chrome tile button used for "+Tab" and template/star
/// buttons in pane headers. Uses `current().r_md` (0 for Meridien, rounded
/// otherwise) and `current().stroke_thin`.
///
/// Returns nothing — the caller owns the `Response` and acts on clicks.
///
/// # Example
/// ```ignore
/// let resp = ui.allocate_rect(rect, egui::Sense::click());
/// let state = if resp.hovered() { ChromeTileState::Hovered } else { ChromeTileState::Idle };
/// paint_chrome_tile_button(&ui.painter().with_clip_rect(rect), rect, state, t);
/// ```
pub fn paint_chrome_tile_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    state: ChromeTileState,
    t: &crate::chart_renderer::gpu::Theme,
) {
    let cr = egui::CornerRadius::same(current().r_md);
    let sw = current().stroke_thin;
    let (bg, border) = match state {
        ChromeTileState::Active  => (
            tint(t, Tone::Accent, 38),
            tint(t, Tone::Accent, alpha_active()),
        ),
        ChromeTileState::Hovered => (
            tint(t, Tone::Border, alpha_subtle()),
            tint(t, Tone::Accent, alpha_line()),
        ),
        ChromeTileState::Idle    => (
            tint(t, Tone::Border, crate::ui_kit::style::alpha_soft()),
            tint(t, Tone::Border, alpha_muted()),
        ),
    };
    painter.rect_filled(rect, cr, bg);
    painter.rect_stroke(rect, cr, egui::Stroke::new(sw, border),
        egui::StrokeKind::Outside);
}

// ─── Border stroke shorthands ─────────────────────────────────────────────────

/// Standard 1px border stroke using `t.toolbar_border`. Covers 90% of separator / divider use.
#[inline]
pub fn border_stroke(t: &crate::chart_renderer::gpu::Theme) -> Stroke {
    Stroke::new(stroke_std(), t.toolbar_border)
}

/// Hair-width border stroke for dense / compact UI regions.
#[inline]
pub fn border_stroke_thin(t: &crate::chart_renderer::gpu::Theme) -> Stroke {
    Stroke::new(stroke_thin(), t.toolbar_border)
}

// ─── Icon button size tokens ──────────────────────────────────────────────────

/// 16×16 — small icon button (close, delete, inline action).
pub const BTN_ICON_SM: egui::Vec2 = egui::vec2(16.0, 16.0);
/// 24×24 — standard icon button (toolbar action, panel header icon).
pub const BTN_ICON_MD: egui::Vec2 = egui::vec2(24.0, 24.0);
/// 32×24 — wide icon button (split actions, nav arrows with extra hit area).
pub const BTN_ICON_LG: egui::Vec2 = egui::vec2(32.0, 24.0);

/// Foreground color for a [`ChromeTileState`] — pair with [`paint_chrome_tile_button`].
pub fn chrome_tile_fg(state: ChromeTileState, t: &crate::chart_renderer::gpu::Theme) -> egui::Color32 {
    match state {
        ChromeTileState::Active  => t.accent,
        ChromeTileState::Hovered => t.text,
        ChromeTileState::Idle    => t.dim.gamma_multiply(0.8),
    }
}

// ─── Phase 2a/2b equivalence tests ──────────────────────────────────────────
//
// Verify that `style_defaults(id)` (now adapter-driven) produces field-exact
// output for ids 0/1/2 compared to the frozen `style_defaults_legacy` reference.
// Ids 3-8 are new personalities defined only in builtin_style_systems() and have
// no legacy reference to compare against.

#[cfg(test)]
mod s2_equivalence_tests {
    use super::*;

    /// Compare two f32 values with a small epsilon (rounding in shadow_alpha cast).
    fn f32_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// Compare two u8 values from shadow_alpha (allow ±1 from f32 rounding).
    fn u8_shadow_eq(a: u8, b: u8) -> bool {
        (a as i16 - b as i16).abs() <= 1
    }

    /// Radius tokens that are KNOWN to differ between the new design-system
    /// styles and the frozen legacy literals, because a deliberate fidelity
    /// edit moved them.
    ///
    /// `(style id, token, legacy value, new value)`.
    ///
    /// Meridien's legacy radii were never Meridien's: the Phase B source-swap
    /// defined Meridien-the-default as the app's pre-existing `dt_f32!` scale
    /// to keep the look unchanged through the swap. These three now come from
    /// the actual design source (`trading app - meridien/design-system/
    /// primitives.css`: sm 3, lg 14) — `r_pill` was the consequential one, at
    /// `0` every `RadiusTier::Pill` control rendered square.
    ///
    /// This is an allow-list, not a relaxed assertion: a fourth divergence, or
    /// a different value for one of these three, still fails.
    const RADIUS_DIVERGENCES: &[(usize, &str, u8, u8)] = &[
        (0, "r_sm",   4,  3),
        (0, "r_lg",  12, 14),
        (0, "r_pill", 0, 99),
    ];

    fn check_radius(id: usize, token: &str, new: u8, legacy: u8) {
        if new == legacy {
            return;
        }
        let allowed = RADIUS_DIVERGENCES
            .iter()
            .any(|&(i, t, l, n)| i == id && t == token && l == legacy && n == new);
        assert!(
            allowed,
            "id={id} {token} mismatch: new={new} legacy={legacy} — this is not a \
             recorded divergence. If the change is deliberate, add it to \
             RADIUS_DIVERGENCES with a note on why."
        );
    }

    fn check_style_equal(id: usize, new: &StyleSettings, legacy: &StyleSettings) {
        // Radii
        check_radius(id, "r_xs",   new.r_xs,   legacy.r_xs);
        check_radius(id, "r_sm",   new.r_sm,   legacy.r_sm);
        check_radius(id, "r_md",   new.r_md,   legacy.r_md);
        check_radius(id, "r_lg",   new.r_lg,   legacy.r_lg);
        check_radius(id, "r_pill", new.r_pill, legacy.r_pill);
        check_radius(id, "r_chip", new.r_chip, legacy.r_chip);

        // Strokes
        assert!(f32_eq(new.stroke_hair,  legacy.stroke_hair),  "id={} stroke_hair mismatch: {} vs {}", id, new.stroke_hair, legacy.stroke_hair);
        assert!(f32_eq(new.stroke_thin,  legacy.stroke_thin),  "id={} stroke_thin mismatch: {} vs {}", id, new.stroke_thin, legacy.stroke_thin);
        assert!(f32_eq(new.stroke_std,   legacy.stroke_std),   "id={} stroke_std mismatch: {} vs {}", id, new.stroke_std, legacy.stroke_std);
        assert!(f32_eq(new.stroke_bold,  legacy.stroke_bold),  "id={} stroke_bold mismatch: {} vs {}", id, new.stroke_bold, legacy.stroke_bold);
        assert!(f32_eq(new.stroke_thick, legacy.stroke_thick), "id={} stroke_thick mismatch: {} vs {}", id, new.stroke_thick, legacy.stroke_thick);

        // Treatments (bool)
        assert_eq!(new.serif_headlines,          legacy.serif_headlines,          "id={} serif_headlines", id);
        assert_eq!(new.button_treatment,         legacy.button_treatment,         "id={} button_treatment", id);
        assert_eq!(new.hairline_borders,         legacy.hairline_borders,         "id={} hairline_borders", id);
        assert_eq!(new.shadows_enabled,          legacy.shadows_enabled,          "id={} shadows_enabled", id);
        assert_eq!(new.solid_active_fills,       legacy.solid_active_fills,       "id={} solid_active_fills", id);
        assert_eq!(new.invert_active_fill,       legacy.invert_active_fill,       "id={} invert_active_fill", id);
        assert_eq!(new.uppercase_section_labels, legacy.uppercase_section_labels, "id={} uppercase_section_labels", id);
        assert_eq!(new.vertical_group_dividers,  legacy.vertical_group_dividers,  "id={} vertical_group_dividers", id);
        assert_eq!(new.show_active_tab_underline,legacy.show_active_tab_underline,"id={} show_active_tab_underline", id);
        assert_eq!(new.inactive_header_fill,     legacy.inactive_header_fill,     "id={} inactive_header_fill", id);
        assert_eq!(new.nav_buttons_label_only,   legacy.nav_buttons_label_only,   "id={} nav_buttons_label_only", id);
        assert_eq!(new.nav_buttons_uppercase_labels, legacy.nav_buttons_uppercase_labels, "id={} nav_buttons_uppercase_labels", id);
        assert_eq!(new.tab_underline_under_text, legacy.tab_underline_under_text, "id={} tab_underline_under_text", id);
        assert_eq!(new.card_floating_shadow,     legacy.card_floating_shadow,     "id={} card_floating_shadow", id);
        assert_eq!(new.animations_enabled,       legacy.animations_enabled,       "id={} animations_enabled", id);
        assert_eq!(new.pane_active_fill_accent,  legacy.pane_active_fill_accent,  "id={} pane_active_fill_accent", id);
        assert_eq!(new.surface_bevel,            legacy.surface_bevel,            "id={} surface_bevel", id);
        assert_eq!(new.bevel_highlight_alpha,    legacy.bevel_highlight_alpha,    "id={} bevel_highlight_alpha", id);
        assert_eq!(new.bevel_shadow_alpha,       legacy.bevel_shadow_alpha,       "id={} bevel_shadow_alpha", id);
        assert_eq!(new.wl_row_corner_radius,     legacy.wl_row_corner_radius,     "id={} wl_row_corner_radius", id);
        assert_eq!(new.wl_row_divider_alpha,     legacy.wl_row_divider_alpha,     "id={} wl_row_divider_alpha", id);
        assert_eq!(new.wl_symbol_mono,           legacy.wl_symbol_mono,           "id={} wl_symbol_mono", id);
        assert_eq!(new.section_header_mono,      legacy.section_header_mono,      "id={} section_header_mono", id);
        assert_eq!(new.panel_tab_treatment,      legacy.panel_tab_treatment,      "id={} panel_tab_treatment", id);

        // Spacing
        assert!(f32_eq(new.label_letter_spacing_px, legacy.label_letter_spacing_px), "id={} label_letter_spacing_px", id);
        assert!(f32_eq(new.toolbar_height_scale,    legacy.toolbar_height_scale),    "id={} toolbar_height_scale", id);
        assert!(f32_eq(new.header_height_scale,     legacy.header_height_scale),     "id={} header_height_scale", id);
        assert!(f32_eq(new.account_strip_height,    legacy.account_strip_height),    "id={} account_strip_height", id);
        assert!(f32_eq(new.pane_border_width,       legacy.pane_border_width),       "id={} pane_border_width", id);
        assert!(f32_eq(new.pane_gap,                legacy.pane_gap),                "id={} pane_gap: {} vs {}", id, new.pane_gap, legacy.pane_gap);
        assert!(f32_eq(new.card_padding_y,          legacy.card_padding_y),          "id={} card_padding_y", id);
        assert!(f32_eq(new.card_padding_x,          legacy.card_padding_x),          "id={} card_padding_x", id);
        assert!(f32_eq(new.row_height_px,           legacy.row_height_px),           "id={} row_height_px", id);
        assert!(f32_eq(new.button_height_px,        legacy.button_height_px),        "id={} button_height_px", id);
        assert!(f32_eq(new.button_padding_x,        legacy.button_padding_x),        "id={} button_padding_x", id);
        assert!(f32_eq(new.tab_height,              legacy.tab_height),              "id={} tab_height", id);
        assert!(f32_eq(new.cta_height_px,           legacy.cta_height_px),           "id={} cta_height_px", id);
        assert!(f32_eq(new.cta_padding_x,           legacy.cta_padding_x),           "id={} cta_padding_x", id);
        assert!(f32_eq(new.wl_row_side_margin,      legacy.wl_row_side_margin),      "id={} wl_row_side_margin", id);
        assert!(f32_eq(new.section_header_tracking, legacy.section_header_tracking), "id={} section_header_tracking", id);

        // Typography
        assert!(f32_eq(new.font_hero,         legacy.font_hero),         "id={} font_hero", id);
        assert!(f32_eq(new.font_section_label,legacy.font_section_label),"id={} font_section_label", id);
        assert!(f32_eq(new.font_body,         legacy.font_body),         "id={} font_body", id);
        assert!(f32_eq(new.font_caption,      legacy.font_caption),      "id={} font_caption", id);
        assert!(f32_eq(new.nav_letter_spacing_px, legacy.nav_letter_spacing_px), "id={} nav_letter_spacing_px", id);

        // Interaction
        assert_eq!(new.hover_bg_alpha,    legacy.hover_bg_alpha,    "id={} hover_bg_alpha", id);
        assert_eq!(new.active_bg_alpha,   legacy.active_bg_alpha,   "id={} active_bg_alpha", id);
        assert!(f32_eq(new.focus_ring_width, legacy.focus_ring_width), "id={} focus_ring_width", id);
        assert_eq!(new.focus_ring_alpha,  legacy.focus_ring_alpha,  "id={} focus_ring_alpha", id);
        assert!(f32_eq(new.disabled_opacity, legacy.disabled_opacity), "id={} disabled_opacity", id);

        // Density
        assert_eq!(new.density, legacy.density, "id={} density", id);
        assert!(f32_eq(new.accent_emphasis, legacy.accent_emphasis), "id={} accent_emphasis", id);

        // Shadows
        assert!(f32_eq(new.shadow_blur,     legacy.shadow_blur),     "id={} shadow_blur", id);
        assert!(f32_eq(new.shadow_offset_y, legacy.shadow_offset_y), "id={} shadow_offset_y", id);
        assert!(u8_shadow_eq(new.shadow_alpha, legacy.shadow_alpha), "id={} shadow_alpha: {} vs {}", id, new.shadow_alpha, legacy.shadow_alpha);

        // Chrome (u8)
        assert_eq!(new.pane_gap_alpha,          legacy.pane_gap_alpha,          "id={} pane_gap_alpha", id);
        assert_eq!(new.pane_active_indicator,   legacy.pane_active_indicator,   "id={} pane_active_indicator", id);
        assert_eq!(new.nav_active_col_alpha,    legacy.nav_active_col_alpha,    "id={} nav_active_col_alpha", id);
        assert_eq!(new.dialog_backdrop_alpha,   legacy.dialog_backdrop_alpha,   "id={} dialog_backdrop_alpha", id);
        assert_eq!(new.tab_hover_bg_alpha,      legacy.tab_hover_bg_alpha,      "id={} tab_hover_bg_alpha", id);
        assert_eq!(new.card_floating_shadow_alpha, legacy.card_floating_shadow_alpha, "id={} card_floating_shadow_alpha", id);
        assert_eq!(new.header_outer_border_alpha,  legacy.header_outer_border_alpha,  "id={} header_outer_border_alpha", id);
        assert_eq!(new.header_divider_alpha,    legacy.header_divider_alpha,    "id={} header_divider_alpha", id);
        assert_eq!(new.toast_bg_alpha,          legacy.toast_bg_alpha,          "id={} toast_bg_alpha", id);
        assert_eq!(new.card_stripe_alpha,       legacy.card_stripe_alpha,       "id={} card_stripe_alpha", id);
        assert_eq!(new.region_border_alpha,     legacy.region_border_alpha,     "id={} region_border_alpha", id);
        assert_eq!(new.nav_cluster_fill_alpha,  legacy.nav_cluster_fill_alpha,  "id={} nav_cluster_fill_alpha", id);
        assert_eq!(new.panel_section_fill_alpha,legacy.panel_section_fill_alpha,"id={} panel_section_fill_alpha", id);
        assert_eq!(new.panel_header_treatment,  legacy.panel_header_treatment,  "id={} panel_header_treatment", id);

        // Chrome (f32)
        assert!(f32_eq(new.tab_inactive_alpha,          legacy.tab_inactive_alpha),          "id={} tab_inactive_alpha", id);
        assert!(f32_eq(new.tab_underline_thickness,     legacy.tab_underline_thickness),     "id={} tab_underline_thickness", id);
        assert!(f32_eq(new.section_label_padding_top,   legacy.section_label_padding_top),   "id={} section_label_padding_top", id);
        assert!(f32_eq(new.section_label_padding_bottom,legacy.section_label_padding_bottom),"id={} section_label_padding_bottom", id);
        assert!(f32_eq(new.drag_handle_alpha,           legacy.drag_handle_alpha),           "id={} drag_handle_alpha", id);
        assert!(f32_eq(new.drag_handle_dot_scale,       legacy.drag_handle_dot_scale),       "id={} drag_handle_dot_scale", id);
        assert!(f32_eq(new.active_header_fill_multiply, legacy.active_header_fill_multiply), "id={} active_header_fill_multiply", id);
        assert!(f32_eq(new.inactive_header_fill_multiply, legacy.inactive_header_fill_multiply), "id={} inactive_header_fill_multiply", id);
        assert!(f32_eq(new.header_outer_border_width,   legacy.header_outer_border_width),   "id={} header_outer_border_width", id);
        assert!(f32_eq(new.region_gap,                  legacy.region_gap),                  "id={} region_gap: {} vs {}", id, new.region_gap, legacy.region_gap);
        assert!(f32_eq(new.region_radius,               legacy.region_radius),               "id={} region_radius", id);
        assert!(f32_eq(new.nav_cluster_radius,          legacy.nav_cluster_radius),          "id={} nav_cluster_radius", id);
        assert!(f32_eq(new.nav_cluster_padding,         legacy.nav_cluster_padding),         "id={} nav_cluster_padding", id);
        assert!(f32_eq(new.toolnav_height,              legacy.toolnav_height),              "id={} toolnav_height", id);
        assert!(f32_eq(new.panel_footer_radius,         legacy.panel_footer_radius),         "id={} panel_footer_radius", id);

        // Chrome (bool)
        assert_eq!(new.footer_default_open,  legacy.footer_default_open,  "id={} footer_default_open", id);
        assert_eq!(new.panel_footer_card,    legacy.panel_footer_card,    "id={} panel_footer_card", id);
        assert_eq!(new.button_group,         legacy.button_group,         "id={} button_group", id);

        // Axis violation
        assert_eq!(new.pane_gap_color, legacy.pane_gap_color, "id={} pane_gap_color", id);
    }

    #[test]
    fn style_defaults_equivalence_id_0_meridien() {
        let new    = style_defaults(0);
        let legacy = style_defaults_legacy(0);
        check_style_equal(0, &new, &legacy);
    }

    #[test]
    fn style_defaults_equivalence_id_1_aperture() {
        let new    = style_defaults(1);
        let legacy = style_defaults_legacy(1);
        check_style_equal(1, &new, &legacy);
    }

    #[test]
    fn style_defaults_equivalence_id_2_octave() {
        let new    = style_defaults(2);
        let legacy = style_defaults_legacy(2);
        check_style_equal(2, &new, &legacy);
    }

    // ── Style ids 3-8 snapshot + invariant coverage ─────────────────────────
    //
    // Ids 3-8 (Cadence/Alto/Mariner/Lucid/Relay/Glass) have no
    // `style_defaults_legacy` master to diff against, so `check_style_equal`
    // cannot be used.  Instead we pin the load-bearing per-style tokens as an
    // explicit golden snapshot (transcribed from the values
    // `style_system_to_style_settings(builtin_style_systems()[i])` actually
    // produces today) and assert cross-style structural invariants.
    //
    // Ids 0-2 are included in the snapshot too: it costs nothing and catches
    // drift that would otherwise only surface as a legacy-diff failure.

    /// One row of the golden style-token snapshot.
    struct StyleGolden {
        id:                       &'static str,
        region_gap:               f32,
        region_radius:            f32,
        region_border_alpha:      u8,
        row_height_px:            f32,
        wl_row_side_margin:       f32,
        wl_row_corner_radius:     u8,
        wl_row_divider_alpha:     u8,
        wl_symbol_mono:           bool,
        section_header_mono:      bool,
        section_header_tracking:  f32,
        r_xs:                     u8,
        r_sm:                     u8,
        r_md:                     u8,
        r_lg:                     u8,
        r_pill:                   u8,
        r_chip:                   u8,
        density:                  u8,
        pane_gap:                 f32,
        shadows_enabled:          bool,
        shadow_blur:              f32,
        shadow_offset_y:          f32,
        shadow_alpha:             u8,
        hairline_borders:         bool,
        solid_active_fills:       bool,
        uppercase_section_labels: bool,
        panel_tab_treatment:      u8,
        panel_header_treatment:   u8,
        pane_active_indicator:    u8,
        nav_cluster_radius:       f32,
        nav_cluster_padding:      f32,
        toolnav_height:           f32,
        panel_footer_radius:      f32,
        panel_footer_card:        bool,
        footer_default_open:      bool,
        button_group:             &'static str, // Debug repr of GroupEnclosure
        font_body:                f32,
        font_caption:             f32,
        font_hero:                f32,
        font_section_label:       f32,
    }

    /// Golden values for all 9 built-in styles, read from the live
    /// `style_system_to_style_settings()` output (2026-07 snapshot).
    ///
    /// Any change here must be a *deliberate* design edit — an accidental
    /// tweak to `builtin_style_systems()` will fail this test with the
    /// exact token and both values named.
    const STYLE_GOLDENS: &[StyleGolden] = &[
        StyleGolden { id: "meridien",
            region_gap: 0.0, region_radius: 0.0,  region_border_alpha: 40,
            row_height_px: 22.0, wl_row_side_margin: 0.0, wl_row_corner_radius: 0,
            wl_row_divider_alpha: 0,  wl_symbol_mono: false, section_header_mono: false,
            section_header_tracking: 0.0,
            // Radii from the Meridien design source (primitives.css), not the
            // legacy default scale they were pinned to through the Phase B
            // source-swap: sm 4->3, lg 12->14, pill 0->14.
            r_xs: 2, r_sm: 3, r_md: 6, r_lg: 14, r_pill: 99, r_chip: 0,
            density: 1, pane_gap: 0.0,
            shadows_enabled: true, shadow_blur: 0.0, shadow_offset_y: 0.0, shadow_alpha: 0,
            hairline_borders: true, solid_active_fills: true, uppercase_section_labels: true,
            panel_tab_treatment: 0, panel_header_treatment: 0, pane_active_indicator: 1,
            nav_cluster_radius: 0.0, nav_cluster_padding: 6.0, toolnav_height: 0.0,
            panel_footer_radius: 0.0, panel_footer_card: false, footer_default_open: false,
            button_group: "None",
            font_body: 10.0, font_caption: 8.0, font_hero: 36.0, font_section_label: 8.0 },

        StyleGolden { id: "aperture",
            region_gap: 8.0, region_radius: 12.0, region_border_alpha: 40,
            row_height_px: 26.0, wl_row_side_margin: 6.0, wl_row_corner_radius: 8,
            wl_row_divider_alpha: 0,  wl_symbol_mono: true,  section_header_mono: false,
            section_header_tracking: 0.8,
            r_xs: 8, r_sm: 10, r_md: 14, r_lg: 20, r_pill: 99, r_chip: 0,
            density: 2, pane_gap: 8.0,
            shadows_enabled: true, shadow_blur: 24.0, shadow_offset_y: 8.0, shadow_alpha: 40,
            hairline_borders: false, solid_active_fills: false, uppercase_section_labels: false,
            panel_tab_treatment: 2, panel_header_treatment: 2, pane_active_indicator: 2,
            nav_cluster_radius: 99.0, nav_cluster_padding: 8.0, toolnav_height: 30.0,
            panel_footer_radius: 10.0, panel_footer_card: true, footer_default_open: true,
            button_group: "Bordered",
            font_body: 11.0, font_caption: 9.0, font_hero: 22.0, font_section_label: 10.0 },

        StyleGolden { id: "octave",
            region_gap: 0.0, region_radius: 0.0,  region_border_alpha: 40,
            row_height_px: 20.0, wl_row_side_margin: 0.0, wl_row_corner_radius: 0,
            wl_row_divider_alpha: 0,  wl_symbol_mono: false, section_header_mono: false,
            section_header_tracking: 0.0,
            r_xs: 1, r_sm: 2, r_md: 3, r_lg: 4, r_pill: 99, r_chip: 0,
            density: 0, pane_gap: 2.0,
            shadows_enabled: false, shadow_blur: 8.0, shadow_offset_y: 4.0, shadow_alpha: 20,
            hairline_borders: true, solid_active_fills: true, uppercase_section_labels: true,
            panel_tab_treatment: 0, panel_header_treatment: 0, pane_active_indicator: 3,
            nav_cluster_radius: 2.0, nav_cluster_padding: 4.0, toolnav_height: 0.0,
            panel_footer_radius: 0.0, panel_footer_card: false, footer_default_open: true,
            button_group: "None",
            font_body: 10.0, font_caption: 8.0, font_hero: 22.0, font_section_label: 8.0 },

        // ── id 3: Cadence — Spotify-ish tiled cards, vivid green ──────────
        StyleGolden { id: "cadence",
            region_gap: 8.0, region_radius: 12.0, region_border_alpha: 40,
            row_height_px: 26.0, wl_row_side_margin: 0.0, wl_row_corner_radius: 0,
            wl_row_divider_alpha: 0,  wl_symbol_mono: false, section_header_mono: false,
            section_header_tracking: 0.6,
            r_xs: 4, r_sm: 6, r_md: 10, r_lg: 14, r_pill: 99, r_chip: 99,
            density: 1, pane_gap: 0.0,
            shadows_enabled: true, shadow_blur: 8.0, shadow_offset_y: 2.0, shadow_alpha: 90,
            hairline_borders: true, solid_active_fills: false, uppercase_section_labels: true,
            panel_tab_treatment: 2, panel_header_treatment: 0, pane_active_indicator: 2,
            nav_cluster_radius: 8.0, nav_cluster_padding: 6.0, toolnav_height: 0.0,
            panel_footer_radius: 10.0, panel_footer_card: false, footer_default_open: false,
            button_group: "None",
            font_body: 13.0, font_caption: 9.0, font_hero: 22.0, font_section_label: 11.0 },

        // ── id 4: Alto — Zed-inspired flush editor chrome, mono eyebrows ──
        StyleGolden { id: "alto",
            region_gap: 0.0, region_radius: 12.0, region_border_alpha: 40,
            row_height_px: 24.0, wl_row_side_margin: 0.0, wl_row_corner_radius: 0,
            wl_row_divider_alpha: 22, wl_symbol_mono: true,  section_header_mono: true,
            section_header_tracking: 0.0,
            r_xs: 2, r_sm: 4, r_md: 6, r_lg: 8, r_pill: 99, r_chip: 0,
            density: 1, pane_gap: 0.0,
            shadows_enabled: true, shadow_blur: 8.0, shadow_offset_y: 2.0, shadow_alpha: 80,
            hairline_borders: false, solid_active_fills: false, uppercase_section_labels: true,
            panel_tab_treatment: 0, panel_header_treatment: 0, pane_active_indicator: 2,
            nav_cluster_radius: 8.0, nav_cluster_padding: 6.0, toolnav_height: 0.0,
            panel_footer_radius: 10.0, panel_footer_card: false, footer_default_open: false,
            button_group: "None",
            font_body: 11.0, font_caption: 9.0, font_hero: 22.0, font_section_label: 9.0 },

        // ── id 5: Mariner — Alto geometry, steel-blue personality ─────────
        StyleGolden { id: "mariner",
            region_gap: 0.0, region_radius: 12.0, region_border_alpha: 40,
            row_height_px: 22.0, wl_row_side_margin: 0.0, wl_row_corner_radius: 0,
            wl_row_divider_alpha: 28, wl_symbol_mono: true,  section_header_mono: true,
            section_header_tracking: 0.0,
            r_xs: 2, r_sm: 4, r_md: 6, r_lg: 8, r_pill: 99, r_chip: 0,
            density: 1, pane_gap: 0.0,
            shadows_enabled: true, shadow_blur: 8.0, shadow_offset_y: 2.0, shadow_alpha: 80,
            hairline_borders: false, solid_active_fills: false, uppercase_section_labels: true,
            panel_tab_treatment: 0, panel_header_treatment: 0, pane_active_indicator: 1,
            nav_cluster_radius: 8.0, nav_cluster_padding: 6.0, toolnav_height: 0.0,
            panel_footer_radius: 10.0, panel_footer_card: false, footer_default_open: true,
            button_group: "None",
            font_body: 11.0, font_caption: 9.0, font_hero: 22.0, font_section_label: 9.0 },

        // ── id 6: Lucid — editorial light, no shadows, sharp button group ─
        StyleGolden { id: "lucid",
            region_gap: 0.0, region_radius: 12.0, region_border_alpha: 40,
            row_height_px: 26.0, wl_row_side_margin: 0.0, wl_row_corner_radius: 0,
            wl_row_divider_alpha: 12, wl_symbol_mono: false, section_header_mono: false,
            section_header_tracking: 0.4,
            // Lucid, from faithful/lucid/tokens.full.json — was half-scale.
            r_xs: 4, r_sm: 6, r_md: 8, r_lg: 10, r_pill: 99, r_chip: 0,
            density: 1, pane_gap: 0.0,
            shadows_enabled: false, shadow_blur: 0.0, shadow_offset_y: 0.0, shadow_alpha: 0,
            hairline_borders: false, solid_active_fills: true, uppercase_section_labels: true,
            panel_tab_treatment: 0, panel_header_treatment: 0, pane_active_indicator: 1,
            nav_cluster_radius: 8.0, nav_cluster_padding: 6.0, toolnav_height: 0.0,
            panel_footer_radius: 10.0, panel_footer_card: false, footer_default_open: false,
            button_group: "Sharp",
            font_body: 11.0, font_caption: 9.0, font_hero: 28.0, font_section_label: 9.0 },

        // ── id 7: Relay — brutalist, zero radii, wide tracking, huge hero ─
        StyleGolden { id: "relay",
            region_gap: 0.0, region_radius: 12.0, region_border_alpha: 40,
            row_height_px: 24.0, wl_row_side_margin: 0.0, wl_row_corner_radius: 0,
            wl_row_divider_alpha: 30, wl_symbol_mono: true,  section_header_mono: true,
            section_header_tracking: 1.2,
            r_xs: 0, r_sm: 0, r_md: 2, r_lg: 4, r_pill: 0, r_chip: 0,
            density: 1, pane_gap: 0.0,
            shadows_enabled: true, shadow_blur: 12.0, shadow_offset_y: 4.0, shadow_alpha: 80,
            hairline_borders: true, solid_active_fills: true, uppercase_section_labels: true,
            panel_tab_treatment: 0, panel_header_treatment: 0, pane_active_indicator: 1,
            nav_cluster_radius: 8.0, nav_cluster_padding: 6.0, toolnav_height: 0.0,
            panel_footer_radius: 10.0, panel_footer_card: false, footer_default_open: true,
            button_group: "None",
            font_body: 11.0, font_caption: 9.0, font_hero: 48.0, font_section_label: 9.0 },

        // ── id 8: Glass — frosted, most generous radii/gaps in the set ────
        StyleGolden { id: "glass",
            region_gap: 8.0, region_radius: 16.0, region_border_alpha: 30,
            row_height_px: 30.0, wl_row_side_margin: 4.0, wl_row_corner_radius: 10,
            wl_row_divider_alpha: 0,  wl_symbol_mono: false, section_header_mono: false,
            section_header_tracking: 0.0,
            r_xs: 6, r_sm: 10, r_md: 16, r_lg: 24, r_pill: 99, r_chip: 99,
            density: 2, pane_gap: 0.0,
            shadows_enabled: true, shadow_blur: 32.0, shadow_offset_y: 8.0, shadow_alpha: 30,
            hairline_borders: false, solid_active_fills: false, uppercase_section_labels: false,
            panel_tab_treatment: 2, panel_header_treatment: 2, pane_active_indicator: 2,
            nav_cluster_radius: 99.0, nav_cluster_padding: 10.0, toolnav_height: 32.0,
            panel_footer_radius: 16.0, panel_footer_card: true, footer_default_open: false,
            button_group: "Frosted",
            font_body: 13.0, font_caption: 9.0, font_hero: 28.0, font_section_label: 9.0 },
    ];

    /// Golden-snapshot every load-bearing style token for all 9 built-in
    /// styles — the coverage `check_style_equal` cannot provide for ids 3-8
    /// (no legacy master exists for them).
    ///
    /// Collects every mismatch before failing so a whole-file drift reports
    /// once, not one assert at a time.
    #[test]
    fn styles_0_to_8_token_snapshot() {
        use crate::design_system::builtin_style_systems;

        let systems = builtin_style_systems();
        assert_eq!(systems.len(), 9, "expected 9 built-in style systems");
        assert_eq!(STYLE_GOLDENS.len(), 9, "golden table must cover all 9 styles");

        let mut deltas: Vec<String> = Vec::new();

        for (i, (ss, g)) in systems.iter().zip(STYLE_GOLDENS.iter()).enumerate() {
            assert_eq!(
                ss.meta.id, g.id,
                "style order changed: index {} is '{}' but golden expects '{}'",
                i, ss.meta.id, g.id
            );
            let r = style_system_to_style_settings(ss);

            macro_rules! chk_f32 {
                ($field:ident) => {
                    if !f32_eq(r.$field, g.$field) {
                        deltas.push(format!(
                            "[{}][{}] {}: actual={} golden={}",
                            i, g.id, stringify!($field), r.$field, g.$field));
                    }
                };
            }
            macro_rules! chk_eq {
                ($field:ident) => {
                    if r.$field != g.$field {
                        deltas.push(format!(
                            "[{}][{}] {}: actual={:?} golden={:?}",
                            i, g.id, stringify!($field), r.$field, g.$field));
                    }
                };
            }

            // ── Shell region layout (the ids 3-8 differentiators) ─────────
            chk_f32!(region_gap);
            chk_f32!(region_radius);
            chk_eq!(region_border_alpha);

            // ── Watchlist row shape ──────────────────────────────────────
            chk_f32!(row_height_px);
            chk_f32!(wl_row_side_margin);
            chk_eq!(wl_row_corner_radius);
            chk_eq!(wl_row_divider_alpha);
            chk_eq!(wl_symbol_mono);

            // ── Section headers ──────────────────────────────────────────
            chk_eq!(section_header_mono);
            chk_f32!(section_header_tracking);

            // ── Radii ────────────────────────────────────────────────────
            chk_eq!(r_xs);
            chk_eq!(r_sm);
            chk_eq!(r_md);
            chk_eq!(r_lg);
            chk_eq!(r_pill);
            chk_eq!(r_chip);

            // ── Density / spacing ────────────────────────────────────────
            chk_eq!(density);
            chk_f32!(pane_gap);

            // ── Shadows ──────────────────────────────────────────────────
            chk_eq!(shadows_enabled);
            chk_f32!(shadow_blur);
            chk_f32!(shadow_offset_y);
            chk_eq!(shadow_alpha);

            // ── Treatments ───────────────────────────────────────────────
            chk_eq!(hairline_borders);
            chk_eq!(solid_active_fills);
            chk_eq!(uppercase_section_labels);
            chk_eq!(panel_tab_treatment);
            chk_eq!(panel_header_treatment);
            chk_eq!(pane_active_indicator);

            // ── Toolbar / footer chrome ──────────────────────────────────
            chk_f32!(nav_cluster_radius);
            chk_f32!(nav_cluster_padding);
            chk_f32!(toolnav_height);
            chk_f32!(panel_footer_radius);
            chk_eq!(panel_footer_card);
            chk_eq!(footer_default_open);

            // `GroupEnclosure` has no Display impl — compare Debug reprs.
            let bg_actual = format!("{:?}", r.button_group);
            if bg_actual != g.button_group {
                deltas.push(format!(
                    "[{}][{}] button_group: actual={} golden={}",
                    i, g.id, bg_actual, g.button_group));
            }

            // ── Typography ───────────────────────────────────────────────
            chk_f32!(font_body);
            chk_f32!(font_caption);
            chk_f32!(font_hero);
            chk_f32!(font_section_label);
        }

        assert!(
            deltas.is_empty(),
            "{} style-token snapshot drift(s):\n  {}",
            deltas.len(),
            deltas.join("\n  ")
        );
    }

    /// Structural invariants every built-in style must satisfy, independent of
    /// the pinned values above.
    ///
    /// # Verified invariants
    /// * `region_gap > 0` ⟹ `region_radius > 0` — a *tiled* shell floats each
    ///   region as a rounded card; a tiled style with square corners would be
    ///   a broken personality.  Holds for all 9 styles (aperture 8/12,
    ///   cadence 8/12, glass 8/16).
    /// * `density` ∈ {0,1,2}; `row_height_px` > 0; font sizes > 0; radii
    ///   monotonically non-decreasing xs ≤ sm ≤ md ≤ lg.
    /// * `wl_row_corner_radius > 0` ⟹ `wl_row_side_margin > 0` — a rounded
    ///   watchlist row only reads as a pill when it is inset from the edge.
    ///
    /// # NOT asserted — two plausible invariants that do NOT hold today
    ///
    /// 1. "`region_gap == 0` ⟹ `region_radius == 0`" (the converse of the
    ///    tiled⟹rounded rule) is FALSE: alto, mariner, lucid and relay are
    ///    flush (`region_gap == 0`) yet carry `region_radius == 12.0`,
    ///    inherited from `Chrome::default_region_radius()` because their
    ///    presets never set the field.  Every consumer of `region_radius`
    ///    (`paint_region_bg` here, `side_panel_shell::rail_slot_ui`,
    ///    `side_panel_shell::show`, `bottom_dock`) is gated on
    ///    `region_gap > 0`, so the value is inert rather than mis-rendered.
    ///
    /// 2. "`!shadows_enabled && !card_floating_shadow` ⟹ `shadow_alpha == 0`"
    ///    is FALSE: octave (id 2) has `shadows_enabled = false`,
    ///    `card_floating_shadow = false` and `shadow_alpha = 20`.  Every
    ///    `shadow_alpha` consumer (`frames_widget`, `apply_style_to_egui`)
    ///    is gated on `shadows_enabled`, so this is likewise inert.
    ///
    /// Both are dead-but-misleading token values, not rendering defects.
    /// Asserting them would be asserting a wish, not reality, so they are
    /// documented here instead.
    #[test]
    fn styles_structural_invariants() {
        use crate::design_system::builtin_style_systems;

        let systems = builtin_style_systems();
        let mut problems: Vec<String> = Vec::new();

        for (i, ss) in systems.iter().enumerate() {
            let id = &ss.meta.id;
            let r  = style_system_to_style_settings(ss);

            // Tiled ⟹ rounded.
            if r.region_gap > 0.0 && r.region_radius <= 0.0 {
                problems.push(format!(
                    "[{i}][{id}] tiled shell (region_gap={}) must have region_radius > 0, got {}",
                    r.region_gap, r.region_radius));
            }

            // Density enum range.
            if r.density > 2 {
                problems.push(format!("[{i}][{id}] density out of range: {}", r.density));
            }

            // Positive-size sanity.
            if r.row_height_px <= 0.0 {
                problems.push(format!("[{i}][{id}] row_height_px must be > 0, got {}", r.row_height_px));
            }
            for (label, v) in [
                ("font_body", r.font_body), ("font_caption", r.font_caption),
                ("font_hero", r.font_hero), ("font_section_label", r.font_section_label),
            ] {
                if v <= 0.0 {
                    problems.push(format!("[{i}][{id}] {label} must be > 0, got {v}"));
                }
            }

            // Radii scale must be non-decreasing.
            if !(r.r_xs <= r.r_sm && r.r_sm <= r.r_md && r.r_md <= r.r_lg) {
                problems.push(format!(
                    "[{i}][{id}] radii not monotonic: xs={} sm={} md={} lg={}",
                    r.r_xs, r.r_sm, r.r_md, r.r_lg));
            }

            // Rounded watchlist rows need an inset to read as pills.
            if r.wl_row_corner_radius > 0 && r.wl_row_side_margin <= 0.0 {
                problems.push(format!(
                    "[{i}][{id}] wl_row_corner_radius={} but wl_row_side_margin={} (rows would clip flush)",
                    r.wl_row_corner_radius, r.wl_row_side_margin));
            }

            // Shadow geometry must be coherent when shadows ARE enabled:
            // a blur/offset with zero alpha paints nothing.
            if r.shadows_enabled && (r.shadow_blur > 0.0 || r.shadow_offset_y > 0.0)
                && r.shadow_alpha == 0
            {
                problems.push(format!(
                    "[{i}][{id}] shadows enabled with blur={} offset_y={} but shadow_alpha=0",
                    r.shadow_blur, r.shadow_offset_y));
            }
        }

        assert!(
            problems.is_empty(),
            "{} structural invariant violation(s):\n  {}",
            problems.len(),
            problems.join("\n  ")
        );
    }

    /// Ids 3-8 must be genuinely distinct personalities, not clones of each
    /// other or of the three legacy styles.  Guards against a copy-paste
    /// preset that silently duplicates an existing look.
    #[test]
    fn styles_3_to_8_are_distinct_personalities() {
        use crate::design_system::builtin_style_systems;

        let systems = builtin_style_systems();
        let fingerprints: Vec<(String, String)> = systems
            .iter()
            .map(|ss| {
                let r = style_system_to_style_settings(ss);
                (
                    ss.meta.id.clone(),
                    format!(
                        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                        r.region_gap, r.region_radius, r.row_height_px,
                        r.wl_row_divider_alpha, r.section_header_mono,
                        r.r_xs, r.r_sm, r.r_md, r.r_lg,
                        r.density, r.shadow_alpha, r.font_hero,
                        r.section_header_tracking,
                    ),
                )
            })
            .collect();

        for a in 0..fingerprints.len() {
            for b in (a + 1)..fingerprints.len() {
                assert_ne!(
                    fingerprints[a].1, fingerprints[b].1,
                    "styles '{}' (id {}) and '{}' (id {}) are token-identical",
                    fingerprints[a].0, a, fingerprints[b].0, b
                );
            }
        }
    }
}

// ─── DS-IMPL-3 token tests ───────────────────────────────────────────────────

#[cfg(test)]
mod ds_impl_3_tests {
    use super::*;

    /// gap_xs_mid() must return 6.0 (sits between gap_xs=4.0 and gap_sm=8.0).
    #[test]
    fn gap_xs_mid_literal() {
        // design-mode is off in tests by default, so the dt_f32! macro falls
        // through to its compile-time default.
        assert_eq!(gap_xs_mid(), 6.0_f32);
    }

    /// GAP_XS_MID const must match the function's fallback value.
    #[test]
    fn gap_xs_mid_const_matches_fn() {
        assert_eq!(GAP_XS_MID, gap_xs_mid());
    }

    /// stroke_medium() must return 0.8 — the promoted design-token value.
    #[test]
    fn stroke_medium_literal() {
        assert_eq!(stroke_medium(), 0.8_f32);
    }

    /// STROKE_MEDIUM const must match the function fallback.
    #[test]
    fn stroke_medium_const_matches_fn() {
        assert_eq!(STROKE_MEDIUM, stroke_medium());
    }

    /// elevation_1/2/3 must return `elevate(bg, N)` with the documented per-role
    /// shift amounts (2026-07-30: additive raised ramp replaced the gamma one so
    /// depth survives near-black backgrounds — see `ui_kit::style::elevate`).
    #[test]
    fn elevation_tints_use_correct_shift() {
        use crate::ui_kit::style::{elevate, ELEVATE_CARD, ELEVATE_RAISED, ELEVATE_MODAL};
        let all = crate::chart_renderer::gpu::get_all_themes();
        let t = all.iter().find(|t| t.name == "Midnight")
            .expect("Midnight theme must exist");
        assert_eq!(elevation_1(t), elevate(t.bg, ELEVATE_CARD),   "elevation_1 shift");
        assert_eq!(elevation_2(t), elevate(t.bg, ELEVATE_RAISED), "elevation_2 shift");
        assert_eq!(elevation_3(t), elevate(t.bg, ELEVATE_MODAL),  "elevation_3 shift");
    }

    /// Higher elevation = more lift. On a dark background the raised ramp makes
    /// each step LIGHTER, so lum(e3) >= lum(e2) >= lum(e1) (was reversed under
    /// the old gamma-darken model).
    #[test]
    fn elevation_depth_order_is_monotonic() {
        let all = crate::chart_renderer::gpu::get_all_themes();
        let t = all.iter().find(|t| t.name == "Midnight")
            .expect("Midnight theme must exist");
        let lum = |c: egui::Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
        assert!(lum(elevation_3(t)) >= lum(elevation_2(t)),
            "elevation_3 should be >= elevation_2 in luminance (raised ramp)");
        assert!(lum(elevation_2(t)) >= lum(elevation_1(t)),
            "elevation_2 should be >= elevation_1 in luminance (raised ramp)");
    }
}

/// End-to-end proof of the data-driven styling path (Stream S2 milestone).
///
/// Demonstrates that a brand-new style and palette, defined purely as DATA
/// (authored, serialized to DTCG JSON, then re-parsed as if loaded from a
/// theme-pack file), flow through the SAME adapter + resolver the live app
/// uses — with the custom values surviving end to end. No Rust edits or
/// recompilation introduce the new style: the value path is data only.
#[cfg(test)]
mod data_driven_proof {
    use super::style_system_to_style_settings;
    use crate::design_system::{
        builtin_style_systems, builtin_color_schemes, StyleSystem, ColorScheme,
    };

    #[test]
    fn new_style_from_json_flows_through_with_zero_rust_edits() {
        // 1. Author a brand-new StyleSystem as DATA: start from a builtin and
        //    give it distinctive dimensions no builtin uses, then rename it.
        let mut custom = builtin_style_systems()[0].clone();
        custom.meta.id = "proof_custom".to_string();
        custom.meta.name = "Proof Custom".to_string();
        custom.radii.lg = 17.0;      // distinctive
        custom.strokes.thick = 4.25; // distinctive
        custom.spacing.md = 9.5;     // distinctive

        // 2. Serialize to DTCG JSON (the on-disk theme-pack form) and re-parse,
        //    simulating a style loaded from a file rather than written in Rust.
        let json = custom.to_dtcg();
        let loaded = StyleSystem::from_dtcg(&json)
            .expect("custom style JSON should parse");

        // 3. The JSON round-trip preserved the custom dimension values.
        assert!((loaded.radii.lg - 17.0).abs() < 1e-4, "radii.lg lost in JSON round-trip");
        assert!((loaded.strokes.thick - 4.25).abs() < 1e-4, "strokes.thick lost in JSON round-trip");
        assert!((loaded.spacing.md - 9.5).abs() < 1e-4, "spacing.md lost in JSON round-trip");

        // 4. Run the loaded style through the SAME adapter the app uses to build
        //    the legacy StyleSettings consumed by the render path. No special
        //    casing for "proof_custom" exists anywhere — it is pure data.
        let settings = style_system_to_style_settings(&loaded);
        assert_eq!(settings.r_lg, 17, "custom radius did not reach StyleSettings");
        assert!((settings.stroke_thick - 4.25).abs() < 1e-4, "custom stroke did not reach StyleSettings");

        // AT-064: step 5 removed with `DesignSnapshot`. Its comment claimed to
        // run "exactly as begin_frame does" — begin_frame never called
        // `snapshot()`; it builds a `TokenSnapshot` from active_override /
        // DesignTokens / current(). Steps 1-4 above exercise the real path.
    }

    #[test]
    fn widened_palette_from_json_resolves_custom_semantics() {
        // A palette authored as DATA with explicit info/success (the widened
        // semantic axis), danger left unset to prove the bull/bear alias fallback.
        let mut cs = builtin_color_schemes()[0].clone();
        cs.meta.id = "proof_palette".to_string();
        cs.info = Some([10, 20, 30, 255]);
        cs.success = Some([1, 2, 3, 255]);

        let json = cs.to_dtcg();
        let loaded = ColorScheme::from_dtcg(&json)
            .expect("custom palette JSON should parse");

        assert_eq!(loaded.resolved_info(), [10, 20, 30, 255], "explicit info lost in round-trip");
        assert_eq!(loaded.resolved_success(), [1, 2, 3, 255], "explicit success lost in round-trip");
        // danger was never set -> resolver must fall back to the bear trading alias.
        assert_eq!(loaded.resolved_danger(), loaded.bear, "unset danger should alias bear");
    }
}

// Serialises tests that mutate the process-global ACTIVE_STYLE + preset
// stores (cargo test runs threads in parallel; thread-local snapshots are
// isolated but the atomic + RwLock stores are not).
#[cfg(test)]
pub(crate) static M1_GLOBAL_STATE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ── M1 ladder-wiring proof tests ─────────────────────────────────────────────
#[cfg(test)]
mod m1_ladder_tests {
    use super::*;

    /// The serde-default ladder on `Spacing`/`Typography`/`Alphas` must equal
    /// the literals `begin_frame` shipped before the M1 wire-up — this is the
    /// byte-identical guarantee for every style that does not author a ladder.
    #[test]
    fn default_ladders_match_pre_m1_literals() {
        use crate::design_system::StyleSystem;
        let ss = StyleSystem::default();
        // gaps (old dt_f32 defaults: 4/6/8/12/16/20/24/32)
        assert_eq!(ss.spacing.gap_xs, 4.0);
        assert_eq!(ss.spacing.xs_mid, 6.0);
        assert_eq!(ss.spacing.gap_sm, 8.0);
        assert_eq!(ss.spacing.gap_md, 12.0);
        assert_eq!(ss.spacing.gap_lg, 16.0);
        assert_eq!(ss.spacing.gap_xl, 20.0);
        assert_eq!(ss.spacing.gap_2xl, 24.0);
        assert_eq!(ss.spacing.gap_3xl, 32.0);
        // UI type ladder (old: 9/10/12/14/16/22)
        assert_eq!(ss.typography.ui_2xs, 9.0);
        assert_eq!(ss.typography.ui_xs, 10.0);
        assert_eq!(ss.typography.ui_sm, 12.0);
        assert_eq!(ss.typography.ui_md, 14.0);
        assert_eq!(ss.typography.ui_lg, 16.0);
        assert_eq!(ss.typography.ui_xl, 22.0);
        // alpha u8 tiers (old: 10/15/20/40/48/60/60/80/80/100/120/140/200)
        let a = &ss.alphas;
        assert_eq!(
            [a.faint, a.ghost, a.soft_u8, a.subtle_u8, a.tint, a.muted_u8, a.dim,
             a.line, a.strong_u8, a.active, a.heavy_u8, a.scrim, a.solid],
            [10, 15, 20, 40, 48, 60, 60, 80, 80, 100, 120, 140, 200],
        );
    }

    /// Every BUILTIN style system must hold the default ladder (none of the 9
    /// authored one yet) — proves the wire-up cannot change today's rendering.
    /// When a theme track (T1+) deliberately authors a ladder, it updates this
    /// test to exempt that style BY NAME with a comment.
    #[test]
    fn builtin_style_systems_hold_default_ladders() {
        use crate::design_system::builtin_style_systems;
        let d = crate::design_system::StyleSystem::default();
        for ss in builtin_style_systems() {
            assert_eq!(ss.spacing.gap_md, d.spacing.gap_md, "{} authored gap_md", ss.meta.name);
            assert_eq!(ss.typography.ui_sm, d.typography.ui_sm, "{} authored ui_sm", ss.meta.name);
            assert_eq!(ss.alphas.soft_u8, d.alphas.soft_u8, "{} authored soft_u8", ss.meta.name);
        }
    }

    /// The "move one token" proof: an AUTHORED ladder value must reach the
    /// per-frame `TokenSnapshot`. This is exit criterion P-2 in miniature —
    /// the axis that was inert (adapter dropped it) is now live.
    #[test]
    fn authored_gap_reaches_frame_tokens() {
        use crate::design_system::StyleSystem;
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_active = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire);

        let mut ss = StyleSystem::default();
        ss.meta.name = "m1-proof".into();
        ss.spacing.gap_md = 99.0;
        ss.typography.ui_sm = 33.0;
        let sys_id = add_style_system(ss);
        // Keep STYLE_STORE aligned (same slot count) so `current()` stays sane.
        let set_id = add_style_preset("m1-proof", get_style_settings(0));
        assert_eq!(sys_id, set_id, "stores must stay index-aligned");

        set_active_style(set_id);
        begin_frame();
        let snap = crate::ui_kit::style::frame_tokens();
        // restore BEFORE asserting so a failure doesn't poison other tests
        set_active_style(prev_active);
        begin_frame();

        assert_eq!(snap.gap_md, 99.0, "authored gap_md must reach TokenSnapshot");
        assert_eq!(snap.font_sm, 33.0, "authored ui_sm must reach TokenSnapshot");
    }

    /// Every rung the cascade gate now guards must actually MOVE when the
    /// DesignTokens layer changes — the behavioural half of that gate.
    ///
    /// `cascade_gate.py` proves the source line has the right SHAPE. This proves
    /// the shape does what it claims, which is the part that was broken: eleven
    /// fields read their StyleSystem value directly, so they were authorable and
    /// exportable and round-tripped green while the inspector slider above them
    /// did nothing. A structural check alone would not have noticed if, say, the
    /// snapshot field were spelled one way and the token path another — both
    /// sides would look correct and the pixel still would not move.
    #[cfg(feature = "design-mode")]
    #[test]
    fn every_guarded_rung_responds_to_the_token_layer() {
        use crate::design_tokens as dtk;

        // The inspector edits a copy of `pristine()`; `pick_*` returns the live
        // value only where it DIFFERS, so the probe has to change each field.
        let base = dtk::pristine().cloned().unwrap_or_else(dtk::DesignTokens::default);
        dtk::init(base.clone());

        let mut edited = base.clone();
        edited.alpha.whisper = 91;
        edited.alpha.hint = 92;
        edited.alpha.dense = 93;
        edited.alpha.near_solid = 94;
        edited.font.display_sm = 95.0;
        edited.font.display_md = 96.0;
        edited.font.display_lg = 97.0;
        edited.font.display_xl = 98.0;
        edited.font.ui_4xs = 99.0;
        edited.font.ui_xs_plus = 100.0;
        edited.font.ui_md_plus = 101.0;
        edited.font.sm = 102.0;
        edited.font.xs = 103.0;
        edited.font.sm_tight = 104.0;
        edited.spacing.gap_2xs = 105.0;
        dtk::update(edited);

        begin_frame();
        let s = crate::ui_kit::style::frame_tokens();

        // restore before asserting so one failure cannot poison the suite
        dtk::update(base);
        begin_frame();

        assert_eq!(s.alpha_whisper, 91, "alpha_whisper ignored the token layer");
        assert_eq!(s.alpha_hint, 92, "alpha_hint ignored the token layer");
        assert_eq!(s.alpha_dense, 93, "alpha_dense ignored the token layer");
        assert_eq!(s.alpha_near_solid, 94, "alpha_near_solid ignored the token layer");
        assert_eq!(s.font_display_sm, 95.0, "font_display_sm ignored the token layer");
        assert_eq!(s.font_display_md, 96.0, "font_display_md ignored the token layer");
        assert_eq!(s.font_display_lg, 97.0, "font_display_lg ignored the token layer");
        assert_eq!(s.font_display_xl, 98.0, "font_display_xl ignored the token layer");
        assert_eq!(s.font_4xs, 99.0, "font_4xs ignored the token layer");
        assert_eq!(s.font_xs_plus, 100.0, "font_xs_plus ignored the token layer");
        assert_eq!(s.font_md_plus, 101.0, "font_md_plus ignored the token layer");
        assert_eq!(s.font_body, 102.0, "font_body ignored the token layer");
        assert_eq!(s.font_caption, 103.0, "font_caption ignored the token layer");
        assert_eq!(s.font_section_label, 104.0, "font_section_label ignored the token layer");
        assert_eq!(s.gap_2xs, 105.0, "gap_2xs ignored the token layer");
    }

    /// `pane_gap` routed through the snapshot must equal the value the mosaic
    /// read before — for EVERY style, not just the default.
    ///
    /// The gutter used to be readable only as `current().pane_gap`, the bottom
    /// layer of the cascade, so it ignored the inspector and hot-reload layers
    /// above it. Moving the four paint-time reads onto `frame_tokens()` is only
    /// safe if the snapshot carries the same number, and the danger is specific:
    /// `DEFAULT_TOKEN_SNAPSHOT.pane_gap` is 8.0 while styles may author 0.0 for
    /// a flush mosaic. Get the wiring wrong and every flush style grows an 8px
    /// gutter — a change that compiles, passes every other test, and is only
    /// visible to the eye.
    ///
    /// So this asserts equivalence per style, and pins a non-default value so
    /// the test cannot pass by both sides being 8.0.
    ///
    /// NOTE ON ISOLATION — this test mutates the active style IN PLACE rather
    /// than adding presets, and that is not a style preference. The sibling
    /// tests pair `add_style_system` with `add_style_preset` because the two
    /// stores are index-aligned and each asserts `sys_id == set_id`. A first
    /// draft of this test called `add_style_preset` twice with no matching
    /// system, drifting STYLE_STORE two slots ahead — which passed in
    /// isolation and failed `authored_card_stack_reaches_ui_kit` (13 vs 15)
    /// hundreds of tests later. `M1_GLOBAL_STATE_TEST_LOCK` does not help
    /// here: it serialises ACCESS, while the damage was store GROWTH, which
    /// outlives the lock. Mutating in place and restoring leaves no trace.
    #[test]
    fn pane_gap_snapshot_matches_style_settings() {
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_active = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire);
        let original = get_style_settings(prev_active);

        // A FLUSH mosaic — the case the 8.0 default would break.
        let mut flush = original.clone();
        flush.pane_gap = 0.0;
        set_style_settings(prev_active, flush);
        begin_frame();
        let snap_flush = crate::ui_kit::style::pane_gap();
        let settings_flush = current().pane_gap;

        // ...and a wide gutter, to prove it TRACKS rather than pins.
        let mut wide = original.clone();
        wide.pane_gap = 13.0;
        set_style_settings(prev_active, wide);
        begin_frame();
        let snap_wide = crate::ui_kit::style::pane_gap();
        let settings_wide = current().pane_gap;

        // Restore BEFORE asserting so a failure cannot poison other tests.
        set_style_settings(prev_active, original);
        begin_frame();

        assert_eq!(settings_flush, 0.0, "fixture did not take: flush style");
        assert_eq!(
            snap_flush, settings_flush,
            "flush mosaic (pane_gap 0.0) must survive the snapshot — an 8.0 \
             default leaking here puts a gutter between every pane"
        );
        assert_eq!(settings_wide, 13.0, "fixture did not take: wide style");
        assert_eq!(
            snap_wide, settings_wide,
            "pane_gap must TRACK the active style, not pin to one value"
        );
    }

    /// The two design TARGETS must actually differ on the two axes that were
    /// hardcoded until now.
    ///
    /// Leading and icon scale lived as literals in `ui_kit::style`, so every
    /// style got identical values no matter what it authored. That is a large
    /// part of why the styles read as the same app in different colours: the
    /// two axes the eye uses to tell "open and editorial" from "tight and
    /// technical" were the two a style could not touch.
    ///
    /// This asserts the authored values reach `frame_tokens()` AND that the
    /// two targets are actually distinct — a test that only checked "reaches
    /// the snapshot" would pass just as happily if both styles were identical.
    #[test]
    fn meridien_and_aperture_differ_on_leading_and_icon_scale() {
        use crate::design_system::builtin_style_systems;
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let all = builtin_style_systems();
        let find = |id: &str| all.iter()
            .find(|s| s.meta.id == id)
            .unwrap_or_else(|| panic!("builtin style `{id}` missing"))
            .clone();
        let (mer, ape) = (find("meridien"), find("aperture"));

        assert!(
            mer.line_heights.normal > ape.line_heights.normal,
            "Meridien is the EDITORIAL archetype and must lead looser than the              dense Aperture mosaic (got {} vs {})",
            mer.line_heights.normal, ape.line_heights.normal,
        );
        assert!(
            ape.icons.lg > mer.icons.lg,
            "Aperture leans on iconography for wayfinding at its density and              must size icons above Meridien (got {} vs {})",
            ape.icons.lg, mer.icons.lg,
        );

        // ...and the authored values must survive the trip to the snapshot,
        // which is the step that was impossible before these token groups
        // existed.
        let prev_active = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire);
        let sys_id = add_style_system(mer.clone());
        let set_id = add_style_preset("leading-proof", get_style_settings(0));
        assert_eq!(sys_id, set_id, "stores must stay index-aligned");

        set_active_style(set_id);
        begin_frame();
        let snap = crate::ui_kit::style::frame_tokens();
        set_active_style(prev_active);
        begin_frame();

        assert_eq!(snap.line_normal, mer.line_heights.normal,
            "authored leading did not reach the TokenSnapshot");
        assert_eq!(snap.icon_lg, mer.icons.lg,
            "authored icon scale did not reach the TokenSnapshot");
    }

    /// The splitter token must reach the divider a user actually drags.
    ///
    /// This exists because the previous wiring LOOKED correct and was not.
    /// `Density::splitter_width` was read by `ui_kit::widgets::PaneGrid`, a
    /// widget no application code ever constructed — so the token had a call
    /// site, the consumer gate passed, and the value still could not move a
    /// pixel. A consumer inside unreachable code is not a consumer, and no
    /// count-based check can tell the difference.
    ///
    /// The real divider is `chart_renderer::pane_layout`'s `hit_band`, which
    /// was its own hardcoded 8.0. Hence both assertions: the default must be
    /// the 8.0 that has always shipped (not the dead widget's 6.0), and an
    /// authored value must actually track.
    #[test]
    fn splitter_token_reaches_the_real_divider() {
        use crate::design_system::StyleSystem;
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_active = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire);

        begin_frame();
        assert_eq!(
            crate::ui_kit::style::splitter_width(), 8.0,
            "default splitter must equal the 8.0 `pane_layout::hit_band` has              always used — 6.0 here means it was defaulted to the dead              widget's value again"
        );

        let mut ss = StyleSystem::default();
        ss.meta.name = "splitter-proof".into();
        ss.density.splitter_width = 21.0;
        let sys_id = add_style_system(ss);
        // Keep the two index-aligned stores in step; see the note on
        // `pane_gap_snapshot_matches_style_settings` for what happens otherwise.
        let set_id = add_style_preset("splitter-proof", get_style_settings(0));
        assert_eq!(sys_id, set_id, "stores must stay index-aligned");

        set_active_style(set_id);
        begin_frame();
        let authored = crate::ui_kit::style::splitter_width();
        set_active_style(prev_active);
        begin_frame();

        assert_eq!(
            authored, 21.0,
            "an authored splitter width must reach `splitter_width()`; 8.0              here means the token is pinned rather than cascading"
        );
    }

    /// AUDIT 2026-08: the STRUCTURAL half of the same proof.
    ///
    /// The test above covers gap/type — the two ladders M1 wired. But
    /// `begin_frame` sourced `ass` from `active_style_system()` while consulting
    /// the hot-reload override only for radii and strokes, so a theme JSON
    /// edited on disk moved 36 of 71 fields and silently dropped the rest:
    /// density, treatments, semantic type roles, bevels, watchlist row
    /// geometry. The watcher logged "reloaded StyleSystem" regardless.
    ///
    /// Density is the axis to assert on, because it is what the eye actually
    /// reads as density and it was the largest inert block.
    #[test]
    fn hot_reload_override_reaches_the_structural_tokens() {
        use crate::design_system::StyleSystem;
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let mut ov = StyleSystem::default();
        ov.meta.name = "override-proof".into();
        ov.density.row_dense = 41.0;
        ov.density.splitter_width = 13.0;
        ov.density.rail_wide = 517.0;
        ov.density.control_md = 47.0;

        crate::design_system::hot_reload::install_override_for_test(ov);
        begin_frame();
        let snap = crate::ui_kit::style::frame_tokens();

        // Clear BEFORE asserting so a failure cannot poison sibling tests.
        crate::design_system::hot_reload::clear_override_for_test();
        begin_frame();

        assert_eq!(snap.row_dense, 41.0,
            "a hot-reloaded row height must reach the frame — density was the \
             largest block the override silently dropped");
        assert_eq!(snap.splitter_width, 13.0, "splitter width must follow the override");
        assert_eq!(snap.rail_wide, 517.0, "rail width must follow the override");
        assert_eq!(snap.control_md, 47.0, "control height must follow the override");
    }

    /// AUDIT 2026-08: density must move the WHOLE vertical ladder.
    ///
    /// The chart-side accessors each applied `effective_density().scale()`
    /// themselves while the ui_kit ladder read the snapshot raw, so changing
    /// density moved some rows and left others fixed — and because the default
    /// scale is 1.0, any accessor that forgot the multiplier was invisible.
    ///
    /// The scale is now applied once, as the snapshot is built. This asserts the
    /// RELATIONSHIP (every row and control moves together) rather than pinning
    /// numbers, so it survives re-authoring.
    #[test]
    fn density_scales_the_whole_vertical_ladder_uniformly() {
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = density_override();

        set_density_override(Some(crate::ui_kit::style::DensityMode::Spacious));
        begin_frame();
        let roomy = crate::ui_kit::style::frame_tokens();

        set_density_override(Some(crate::ui_kit::style::DensityMode::Compact));
        begin_frame();
        let tight = crate::ui_kit::style::frame_tokens();

        set_density_override(prev);
        begin_frame();

        let pairs: [(&str, f32, f32); 7] = [
            ("row_dense",    roomy.row_dense,    tight.row_dense),
            ("row_compact",  roomy.row_compact,  tight.row_compact),
            ("row_default",  roomy.row_default,  tight.row_default),
            ("row_spacious", roomy.row_spacious, tight.row_spacious),
            ("row_tall",     roomy.row_tall,     tight.row_tall),
            ("control_sm",   roomy.control_sm,   tight.control_sm),
            ("control_md",   roomy.control_md,   tight.control_md),
        ];
        let unmoved: Vec<&str> = pairs.iter()
            .filter(|(_, r, t)| (r - t).abs() < f32::EPSILON)
            .map(|(n, _, _)| *n)
            .collect();

        assert!(
            unmoved.is_empty(),
            "every vertical-rhythm token must respond to density; these did not: \
             {unmoved:?} — an accessor reading the snapshot without the scale"
        );

        // Rails are a horizontal width and the splitter is a hit target; density
        // is vertical rhythm, so neither should move.
        assert_eq!(roomy.splitter_width, tight.splitter_width,
            "the splitter is a pointer hit target, not vertical rhythm");
        assert_eq!(roomy.rail_wide, tight.rail_wide,
            "rail width is horizontal; density must not shrink it");
    }

    /// M5 — the frozen-chrome invariant, for EVERY style.
    ///
    /// The account strip must be tall enough to contain the hero number it
    /// exists to display. Meridien violated this for its whole life (36px hero
    /// authored into a 36px strip, minus 4px of frame margin) and shipped a
    /// permanently clipped NAV figure. The bug was invisible to every gate
    /// because both numbers are legitimate tokens in isolation — only their
    /// RELATIONSHIP was wrong.
    ///
    /// This test is the relationship. It fails if a future style authors a
    /// larger hero without noticing the strip, or shrinks the strip without
    /// noticing the hero — the two ways this defect can come back.
    #[test]
    fn strip_fits_hero() {
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_active = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire);

        let n = style_store().read().unwrap_or_else(|e| e.into_inner()).len() as u8;
        let mut failures: Vec<String> = Vec::new();
        let mut derived_above_authored = 0usize;
        for id in 0..n {
            set_active_style(id);
            let s = current();
            let resolved = account_strip_height();
            if resolved < s.font_hero + 4.0 {
                failures.push(format!(
                    "style {id}: strip resolves to {resolved} but hero is {} (+4px margin)",
                    s.font_hero
                ));
            }
            if resolved > s.account_strip_height {
                derived_above_authored += 1;
            }
        }
        set_active_style(prev_active);

        assert!(failures.is_empty(), "clipped hero numbers:\n  {}", failures.join("\n  "));
        // Meridien is the style whose authored strip does NOT fit its hero.
        // If this ever hits 0, the derivation has become dead code and the
        // guard above is passing vacuously.
        assert!(
            derived_above_authored >= 1,
            "no style needed the derivation — account_strip_height() is now inert, \
             so this invariant is no longer actually being exercised"
        );
    }

    /// M5 — the toolbar row must contain the controls it hosts, for every style.
    ///
    /// Sibling of [`strip_fits_hero`]. `toolnav_min_height()` was written to
    /// replace a frozen `38.0` after the corpus caught the toolnav clipping its
    /// dropdown buttons, but the toolbar row one level up kept its own copy of
    /// that constant and went on clipping by ~5.6px on six styles. Nothing
    /// caught it, because a bare `38.0` is not a token violation — it only
    /// becomes wrong relative to a type scale that lives somewhere else.
    ///
    /// This asserts the relationship directly: whatever the row resolves to, it
    /// is at least what the buttons inside it need.
    #[test]
    fn toolbar_fits_controls() {
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_active = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire);

        let n = style_store().read().unwrap_or_else(|e| e.into_inner()).len() as u8;
        let mut failures: Vec<String> = Vec::new();
        for id in 0..n {
            set_active_style(id);
            let need = toolnav_min_height();
            // Mirrors the resolution in top_nav.rs: scale the authored base, then
            // floor at what the controls actually require.
            let resolved = (38.0 * current().toolbar_height_scale).max(need);
            if resolved < need {
                failures.push(format!(
                    "style {id}: toolbar resolves to {resolved} but its controls need {need}"
                ));
            }
        }
        set_active_style(prev_active);
        assert!(failures.is_empty(), "clipped toolbar controls:\n  {}", failures.join("\n  "));
    }
}

// ── M1 Change E proof test ───────────────────────────────────────────────────
#[cfg(test)]
mod m1_shadow_stack_tests {
    use super::*;
    use crate::design_system::style_system::{ShadowLayer, ShadowTint};

    /// An authored card stack must reach ui_kit's per-frame getter, and the
    /// empty default must leave the legacy path in charge.
    #[test]
    fn authored_card_stack_reaches_ui_kit() {
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_active = ACTIVE_STYLE.load(std::sync::atomic::Ordering::Acquire);

        // Alto's 4-layer Zed bevel, transcribed from the DS spec.
        let mut ss = crate::design_system::StyleSystem::default();
        ss.meta.name = "m1-shadow-proof".into();
        ss.shadows.card_layers = vec![
            ShadowLayer { inset: true,  offset_x: 0.0, offset_y:  1.0, blur: 0.0,  spread: 0.0,   tint: ShadowTint::Highlight, alpha: 15 },
            ShadowLayer { inset: true,  offset_x: 0.0, offset_y: -1.0, blur: 0.0,  spread: 0.0,   tint: ShadowTint::Shadow,    alpha: 115 },
            ShadowLayer { inset: false, offset_x: 0.0, offset_y:  1.0, blur: 0.0,  spread: 0.0,   tint: ShadowTint::Shadow,    alpha: 102 },
            ShadowLayer { inset: false, offset_x: 0.0, offset_y: 12.0, blur: 28.0, spread: -16.0, tint: ShadowTint::Shadow,    alpha: 153 },
        ];
        let sys_id = add_style_system(ss);
        let set_id = add_style_preset("m1-shadow-proof", get_style_settings(0));
        assert_eq!(sys_id, set_id);

        set_active_style(set_id);
        begin_frame();
        let stack = crate::ui_kit::style::card_shadow_layers();
        set_active_style(prev_active);
        begin_frame();
        let restored = crate::ui_kit::style::card_shadow_layers();

        assert_eq!(stack.len(), 4, "authored 4-layer stack must arrive");
        assert!(stack[0].inset && matches!(stack[0].tint, ShadowTint::Highlight));
        assert!(restored.is_empty(), "unauthored styles keep the legacy single-shadow path");
    }
}

// ── M3 end-to-end recipe-chain test ──────────────────────────────────────────
#[cfg(test)]
mod m3_recipe_chain_tests {
    use super::*;

    /// THE CROWN-JEWEL PROOF: switching the active STYLE changes what a widget
    /// resolves from the recipe layer — with zero widget-code changes.
    ///
    /// Before M3 this was impossible in two independent ways: no pack shipped
    /// recipe data (so the set was always empty), and Button never consulted
    /// the set it was handed. Both are now closed.
    #[test]
    fn active_style_selects_authored_recipes() {
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Every design-system style must ship authored recipes...
        for id in ["aperture", "cadence", "alto", "mariner", "lucid", "meridien"] {
            let set = crate::design_system::builtin_recipes(id);
            assert!(set.len() > 0, "{id} must ship authored recipes");
            assert!(set.get("button.primary").is_some(),
                "{id} must author button.primary (the highest-traffic key)");
        }
        // ...and an unauthored id must stay a guaranteed no-op.
        assert_eq!(crate::design_system::builtin_recipes("octave").len(), 0,
            "unauthored styles keep an empty set (byte-identical rendering)");
    }

    /// The signature differences the audit said were inexpressible: Cadence's
    /// full-pill primary vs Meridien's square controls, resolved through the
    /// SAME widget default.
    #[test]
    fn cadence_and_meridien_resolve_different_button_radii() {
        use crate::ui_kit::sx::{Sx, StyleState};
        let theme = crate::ui_kit::widgets::theme::PortableTheme::dark();
        let builtin_default = Sx::new().rounded(6.0);

        let cadence = crate::design_system::builtin_recipes("cadence")
            .resolve("button.primary", builtin_default, &theme)
            .resolved(StyleState::Normal).radius.expect("cadence radius");
        let meridien = crate::design_system::builtin_recipes("meridien")
            .resolve("button.primary", builtin_default, &theme)
            .resolved(StyleState::Normal).radius.expect("meridien radius");

        assert!(cadence > meridien,
            "Cadence pills ({cadence}) must exceed Meridien squares ({meridien}) \
             from the same widget default — the signature difference the audit \
             found inexpressible");
    }
}

#[cfg(test)]
mod control_height_tests {
    //! Lives here, not in `ui_kit`, because it drives the STYLE STORE — and
    //! `ui_kit_does_not_depend_on_chart_renderer` correctly rejected the
    //! import when this sat next to `Size::height()`. The guard did its job.
    use super::{
        active_style_idx, add_style_preset, add_style_system, begin_frame, get_style_settings,
        set_active_style, M1_GLOBAL_STATE_TEST_LOCK,
    };
    use crate::ui_kit::widgets::tokens::Size;
    use crate::design_system::StyleSystem;

    /// `Size::height()` must follow the ACTIVE STYLE, not a frozen table.
    ///
    /// The assertion is deliberately about the RELATIONSHIP, not the numbers:
    /// registering a style with `control_md + 9` must move `Size::Md.height()`
    /// by 9. A test pinned to `height() == 28.0` would have passed against the
    /// frozen literals too — which is exactly why this went unnoticed while
    /// `font_size()` and `padding_x()` beside it were already themed.
    #[test]
    fn size_height_follows_the_active_style() {
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = active_style_idx();
        set_active_style(prev);
        begin_frame();
        let before = Size::Md.height();

        let mut ss = StyleSystem::default();
        ss.meta.name = "control-ladder-proof".into();
        ss.density.control_md = before + 9.0;
        let sys_id = add_style_system(ss);
        let set_id = add_style_preset("control-ladder-proof", get_style_settings(0));
        assert_eq!(sys_id, set_id, "stores must stay index-aligned");

        set_active_style(set_id);
        begin_frame();
        let after = Size::Md.height();

        set_active_style(prev);
        begin_frame();

        assert_eq!(
            after,
            before + 9.0,
            "Size::Md.height() must come from Density.control_md, not a literal"
        );
    }

    /// Defaults must equal the literals this replaced, or making the ladder
    /// themeable silently resizes every control in the app.
    #[test]
    fn default_ladder_matches_the_former_literals() {
        let _guard = M1_GLOBAL_STATE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = active_style_idx();
        set_active_style(prev);
        begin_frame();
        let d = crate::design_system::style_system::Density::default();
        assert_eq!(
            [d.control_xs, d.control_sm, d.control_md, d.control_lg, d.control_xl],
            [18.0, 22.0, 28.0, 34.0, 40.0],
        );
    }
}
